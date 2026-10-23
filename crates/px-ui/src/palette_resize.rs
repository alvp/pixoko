//! The handle of the palette and the resize of its length, without egui types
//! (specs/ui-palette-handle.md): where the handle is, the cell under the pointer while it is
//! dragged, the state machine of the drag, what is previewed, the pending cut (with its destination
//! and its texts) and the pixels a cut reassigns.

use px_core::{Image, Mode, Rgba};

use crate::help_texts_palette_es as texts;
use crate::palette_grid::{CELL_U, DRAG_START_U, total_rows_cols};
use crate::theme::SURFACE_1;
use crate::view::ScreenRect;

/// Longest palette.
pub const MAX_LEN: usize = 256;
/// Milliseconds of the frame clock per row that the grid scrolls on its own while the handle is
/// held in the first or the last half row of the zone.
pub const AUTOSCROLL_MS: u64 = 120;
/// Columns of the status bar a text of this module has to fit in (the minimum window).
pub const STATUS_COLUMNS: usize = 66;

/// What the status line says while the handle is held at the longest palette.
pub const MAX_ENTRIES_HINT: &str = texts::MAX_ENTRIES_HINT;

/// What the status line says over the handle.
pub const HANDLE_HINT: &str = texts::HANDLE_HINT;

/// The cell of the handle: the one after the last entry, in indexed images only.
pub fn handle_index(image: &Image) -> Option<usize> {
    (image.mode() == Mode::Indexed)
        .then(|| image.palette().map(|palette| palette.len()))
        .flatten()
}

/// Cells of the grid while the handle is dragged and the preview has `preview_len` entries: one
/// full row more than the ones the preview (entries and handle) occupies, so that there is always
/// somewhere to stretch to, up to the row of cell 256.
pub fn drag_zone_cells(preview_len: usize, columns: usize) -> usize {
    let columns = columns.max(1);
    let rows = (total_rows_cols(preview_len.min(MAX_LEN) + 1, columns) + 1)
        .min(total_rows_cols(MAX_LEN + 1, columns));
    rows * columns
}

/// Rows the grid scrolls in `elapsed_ms` with the pointer in a band of the zone: `band` is `-1`
/// in the first half row, `1` in the last one and `0` elsewhere (one row every [`AUTOSCROLL_MS`]).
pub fn autoscroll_rows(elapsed_ms: u64, band: i32) -> i32 {
    let rows = i32::try_from(elapsed_ms / AUTOSCROLL_MS).unwrap_or(i32::MAX);
    band.signum().saturating_mul(rows)
}

/// Color of an entry that would be cut: 25 % of its color over `SURFACE_1`.
pub fn dimmed(color: Rgba) -> Rgba {
    let mix = |c: u8, s: u8| ((u16::from(c) + 3 * u16::from(s)) / 4) as u8;
    Rgba::opaque(
        mix(color.r, SURFACE_1.r()),
        mix(color.g, SURFACE_1.g()),
        mix(color.b, SURFACE_1.b()),
    )
}

/// The two bars of the handle (2 × 8 u, 2 u apart) centered in its cell, in the coordinates of the
/// cell.
pub fn handle_bars(cell: ScreenRect, n: u32) -> [ScreenRect; 2] {
    let k = i64::from(n.max(1));
    let bar = |dx: i64| ScreenRect {
        x: cell.x + (CELL_U - 6) / 2 * k + dx * k,
        y: cell.y + (CELL_U - 8) / 2 * k,
        width: 2 * k,
        height: 8 * k,
    };
    [bar(0), bar(4)]
}

/// Pixels that use an entry from `len` on, in all the images (the cells of all the layers): the
/// ones a cut to `len` entries reassigns.
pub fn reassigned_pixels<'a>(images: impl IntoIterator<Item = &'a Image>, len: usize) -> u64 {
    images
        .into_iter()
        .filter(|image| image.mode() == Mode::Indexed)
        .map(|image| {
            image
                .bytes()
                .iter()
                .filter(|&&byte| usize::from(byte) >= len)
                .count() as u64
        })
        .sum()
}

/// How the press of the handle ends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HandleEnd {
    /// The pointer did not go past the threshold: it is a click and does nothing.
    Click,
    /// It was dragged but there is nothing to do (same length or no destination).
    Nothing,
    /// Released over a cell that asks for a different length.
    Resize { len: u16 },
}

/// A press of the handle that may end in a drag. Positions in physical pixels.
#[derive(Clone, Copy, Debug)]
pub struct HandleDrag {
    original: usize,
    start: (i64, i64),
    dragging: bool,
    target: Option<u16>,
    held_ms: u64,
}

impl HandleDrag {
    /// Pressing the handle of a palette of `len` entries.
    pub fn press(len: usize, at: (i64, i64)) -> HandleDrag {
        HandleDrag {
            original: len,
            start: at,
            dragging: false,
            target: None,
            held_ms: 0,
        }
    }

    /// The pointer is at `at` and the cell under it asks for `target` entries (`None`: outside the
    /// zone). It starts to drag after travelling `4n` pixels (Euclidean distance) and never goes
    /// back to being a click.
    pub fn update(&mut self, at: (i64, i64), n: u32, target: Option<u16>) {
        if !self.dragging {
            let dx = i128::from(at.0) - i128::from(self.start.0);
            let dy = i128::from(at.1) - i128::from(self.start.1);
            let limit = i128::from(DRAG_START_U) * i128::from(n.max(1));
            self.dragging = dx * dx + dy * dy >= limit * limit;
        }
        if self.dragging {
            self.target = target;
        }
    }

    pub fn dragging(&self) -> bool {
        self.dragging
    }

    /// The length of the palette when it was pressed.
    pub fn original_len(&self) -> usize {
        self.original
    }

    /// The length the preview shows: the one under the pointer, or the original if the pointer is
    /// outside the zone (or still not dragging).
    pub fn preview_len(&self) -> usize {
        match (self.dragging, self.target) {
            (true, Some(len)) => usize::from(len),
            _ => self.original,
        }
    }

    /// Adds `dt_ms` of the frame clock while the pointer is in `band` (see [`autoscroll_rows`]) and
    /// returns the rows to scroll now. Leaving the band forgets the time held.
    pub fn autoscroll(&mut self, dt_ms: u64, band: i32) -> i32 {
        if band == 0 || !self.dragging {
            self.held_ms = 0;
            return 0;
        }
        self.held_ms = self.held_ms.saturating_add(dt_ms);
        let rows = autoscroll_rows(self.held_ms, band);
        self.held_ms %= AUTOSCROLL_MS;
        rows
    }

    /// The button went up.
    pub fn release(self) -> HandleEnd {
        if !self.dragging {
            return HandleEnd::Click;
        }
        match self.target {
            Some(len) if usize::from(len) != self.original => HandleEnd::Resize { len },
            _ => HandleEnd::Nothing,
        }
    }
}

/// A cut waiting for its destination and for `Confirmar`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PendingResize {
    /// The new length.
    pub len: u16,
    /// The destination of the pixels, chosen by pressing a kept swatch (never by default).
    pub into: Option<u8>,
    /// Pixels of all the layers that use an entry that is cut.
    pub pixels: u64,
    /// The transparent index of the palette before the cut.
    pub transparent: Option<u8>,
}

/// What the cut warns about, if anything.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Warning {
    LosesTransparent,
    PixelsOpaque,
    DestinationTransparent,
}

impl PendingResize {
    /// The transparent entry is among the ones that are cut.
    pub fn loses_transparent(&self) -> bool {
        self.transparent.is_some_and(|t| u16::from(t) >= self.len)
    }

    /// There are pixels to move and no destination yet: `Confirmar` waits.
    pub fn needs_destination(&self) -> bool {
        self.pixels > 0 && self.into.is_none()
    }

    fn warning(&self) -> Option<Warning> {
        if self.loses_transparent() {
            Some(if self.pixels == 0 {
                Warning::LosesTransparent
            } else {
                Warning::PixelsOpaque
            })
        } else if self.into.is_some() && self.into == self.transparent {
            Some(Warning::DestinationTransparent)
        } else {
            None
        }
    }
}

/// What the status line says while a cut is pending. It fits in [`STATUS_COLUMNS`] columns: if the
/// full text with the warning does not, it is shortened keeping the length, the pixels, the
/// destination and the warning.
pub fn resize_prompt(pending: &PendingResize) -> String {
    let (len, pixels) = (pending.len, pending.pixels);
    let warning = pending.warning();
    let full = {
        let base = match (pixels, pending.into) {
            (0, _) => format!("recortar a {len}"),
            (n, None) => format!(
                "recortar a {len}: pulsa un destino · {}",
                if n == 1 {
                    "1 píxel se reasigna".to_string()
                } else {
                    format!("{n} píxeles se reasignan")
                }
            ),
            (n, Some(d)) => format!(
                "recortar a {len}; destino {d} · {}",
                if n == 1 {
                    format!("1 píxel pasa a la {d}")
                } else {
                    format!("{n} píxeles pasan a la {d}")
                }
            ),
        };
        match warning {
            Some(Warning::LosesTransparent) => {
                format!("{base} · {}", texts::WARN_LOSES_TRANSPARENT)
            }
            Some(Warning::PixelsOpaque) => format!("{base} · {}", texts::WARN_PIXELS_OPAQUE),
            Some(Warning::DestinationTransparent) => {
                format!("{base} · {}", texts::WARN_DESTINATION_TRANSPARENT)
            }
            None => base,
        }
    };
    if full.chars().count() <= STATUS_COLUMNS {
        return full;
    }
    let base = match (pixels, pending.into) {
        (0, _) => format!("recortar a {len}"),
        (n, None) => format!("recortar a {len}: sin destino · {n} px"),
        (n, Some(d)) => format!("recortar a {len}; destino {d} · {n} px"),
    };
    match warning {
        Some(Warning::LosesTransparent) => format!("{base} · {}", texts::WARN_LOSES_TRANSPARENT),
        Some(Warning::PixelsOpaque) => format!("{base} · {}", texts::WARN_PIXELS_OPAQUE_SHORT),
        Some(Warning::DestinationTransparent) => {
            format!("{base} · {}", texts::WARN_DESTINATION_TRANSPARENT_SHORT)
        }
        None => base,
    }
}

/// What the status line says while the handle is dragged.
pub fn drag_status(original: usize, preview: usize) -> String {
    format!("paleta de {preview} colores (antes {original})")
}

/// The 5 s notice of a resize that was applied. `old_len` and `len` are the lengths before and
/// after; `moved`: some pixel changed index; `into`: where they went; `lost_transparent`: the
/// transparent entry was cut.
pub fn resize_summary(
    old_len: usize,
    len: usize,
    into: Option<u8>,
    moved: bool,
    lost_transparent: bool,
) -> String {
    if len > old_len {
        return drag_status(old_len, len);
    }
    match (moved, into) {
        (true, Some(into)) => {
            format!("paleta recortada a {len} colores; píxeles pasan a la {into}")
        }
        (false, _) if lost_transparent => {
            format!("paleta recortada a {len} colores; se pierde la transparente")
        }
        _ => format!("paleta recortada a {len} colores (sin píxeles)"),
    }
}
