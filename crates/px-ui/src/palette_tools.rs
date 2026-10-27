//! Pure helpers of the palette operations of unit 4.6 (specs/hito-4-paleta.md): selection with
//! `Shift` and `Ctrl`, gradient, default palette, transposition and replacing or swapping a color
//! in the whole image.

use px_core::{Command, Document, DocumentError, Image, ImageError, Pixel, Rgba};

/// The selected entries in increasing order: those that are in the range between the active entry
/// and `range_end` XOR in `extras`, plus the active one (always selected).
pub fn selected_entries(active: u8, range_end: Option<u8>, extras: &[u8]) -> Vec<u8> {
    let (low, high) = range_end.map_or((active, active), |end| (active.min(end), active.max(end)));
    (0..=u8::MAX)
        .filter(|&entry| {
            let in_range = range_end.is_some() && (low..=high).contains(&entry);
            entry == active || (in_range != extras.contains(&entry))
        })
        .collect()
}

/// The `n + 1` colors from `a` to `b`: channel `i` is `(a·(n − i) + b·i + n/2) / n`.
pub fn gradient_colors(a: Rgba, b: Rgba, n: usize) -> Vec<Rgba> {
    let mix = |x: u8, y: u8, i: usize| -> u8 {
        let value = (usize::from(x) * (n - i) + usize::from(y) * i + n / 2) / n.max(1);
        u8::try_from(value).unwrap_or(u8::MAX)
    };
    (0..=n)
        .map(|i| {
            Rgba::new(
                mix(a.r, b.r, i),
                mix(a.g, b.g, i),
                mix(a.b, b.b, i),
                mix(a.a, b.a, i),
            )
        })
        .collect()
}

/// The default palette of 256 entries: 16 text colors, a 6×6×6 cube and 24 grays.
pub fn default_palette() -> Vec<Rgba> {
    const BASE: [(u8, u8, u8); 16] = [
        (0, 0, 0),
        (0, 0, 170),
        (0, 170, 0),
        (0, 170, 170),
        (170, 0, 0),
        (170, 0, 170),
        (170, 85, 0),
        (170, 170, 170),
        (85, 85, 85),
        (85, 85, 255),
        (85, 255, 85),
        (85, 255, 255),
        (255, 85, 85),
        (255, 85, 255),
        (255, 255, 85),
        (255, 255, 255),
    ];
    let level = |k: u8| if k == 0 { 0 } else { 55 + 40 * k };
    let mut colors: Vec<Rgba> = BASE
        .iter()
        .map(|&(r, g, b)| Rgba::opaque(r, g, b))
        .collect();
    colors.extend((0..216u16).map(|k| {
        let (r, g, b) = ((k / 36) as u8, (k / 6 % 6) as u8, (k % 6) as u8);
        Rgba::opaque(level(r), level(g), level(b))
    }));
    colors.extend((0..24u8).map(|k| Rgba::opaque(8 + 10 * k, 8 + 10 * k, 8 + 10 * k)));
    colors
}

/// `PermutePalette::order` that swaps entries `a` and `b` of a palette of `len` entries.
pub fn swap_order(len: usize, a: u8, b: u8) -> Vec<u8> {
    let mut order: Vec<u8> = (0..len).map(|i| i as u8).collect();
    order.swap(usize::from(a), usize::from(b));
    order
}

/// A copy of every cell with `map` applied to each pixel; `None` if no pixel changed anywhere.
fn remap(doc: &Document, map: impl Fn(Pixel) -> Pixel) -> Result<Option<Command>, DocumentError> {
    let mut images: Vec<Option<Image>> = Vec::new();
    images
        .try_reserve_exact(doc.layers().len())
        .map_err(|_| DocumentError::OutOfMemory)?;
    let mut changed = false;
    for layer in doc.layers() {
        let Some(image) = doc.image(layer.cel()) else {
            images.push(None);
            continue;
        };
        let mut copy = image.try_clone().map_err(image_error)?;
        for y in 0..image.height() {
            for x in 0..image.width() {
                let before = image.get_pixel(x, y).map_err(image_error)?;
                let after = map(before);
                if after != before {
                    copy.set_pixel(x, y, after).map_err(image_error)?;
                    changed = true;
                }
            }
        }
        images.push(Some(copy));
    }
    Ok(changed.then(|| Command::ReplaceImages {
        width: doc.width(),
        height: doc.height(),
        images,
        based_on: doc.state_id(),
    }))
}

fn image_error(_: ImageError) -> DocumentError {
    DocumentError::OutOfMemory
}

/// The command that changes every pixel equal to `from` into `to` in all cells (`None`: no pixel
/// has that value, or both are the same).
pub fn replace_color_command(
    doc: &Document,
    from: Pixel,
    to: Pixel,
) -> Result<Option<Command>, DocumentError> {
    if from == to {
        return Ok(None);
    }
    remap(doc, |pixel| if pixel == from { to } else { pixel })
}

/// The command that swaps the pixels equal to `a` and to `b` in all cells.
pub fn swap_colors_command(
    doc: &Document,
    a: Pixel,
    b: Pixel,
) -> Result<Option<Command>, DocumentError> {
    if a == b {
        return Ok(None);
    }
    remap(doc, |pixel| {
        if pixel == a {
            b
        } else if pixel == b {
            a
        } else {
            pixel
        }
    })
}

/// The actions of the `Paleta` menu (unit 4.6), all without a default key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaletteTool {
    Copy,
    Paste,
    Swap,
    Gradient,
    Delete,
    Default,
    Load,
    LoadAt,
    Save,
    ReplaceColor,
    SwapColors,
}

impl PaletteTool {
    /// All of them, in menu order.
    pub const ALL: [PaletteTool; 11] = [
        PaletteTool::Copy,
        PaletteTool::Paste,
        PaletteTool::Swap,
        PaletteTool::Gradient,
        PaletteTool::Delete,
        PaletteTool::Default,
        PaletteTool::Load,
        PaletteTool::LoadAt,
        PaletteTool::Save,
        PaletteTool::ReplaceColor,
        PaletteTool::SwapColors,
    ];

    /// The stable help key.
    pub fn key(self) -> &'static str {
        match self {
            PaletteTool::Copy => "palette.copy",
            PaletteTool::Paste => "palette.paste",
            PaletteTool::Swap => "palette.swap",
            PaletteTool::Gradient => "palette.gradient",
            PaletteTool::Delete => "palette.delete",
            PaletteTool::Default => "palette.default",
            PaletteTool::Load => "palette.load",
            PaletteTool::LoadAt => "palette.load-at",
            PaletteTool::Save => "palette.save",
            PaletteTool::ReplaceColor => "palette.replace-color",
            PaletteTool::SwapColors => "palette.swap-colors",
        }
    }

    /// The tool of a help key.
    pub fn from_key(key: &str) -> Option<PaletteTool> {
        PaletteTool::ALL.into_iter().find(|tool| tool.key() == key)
    }

    /// Only indexed images have a palette to work on; replacing or swapping a color in the image works
    /// in every mode.
    pub fn needs_indexed(self) -> bool {
        !matches!(self, PaletteTool::ReplaceColor | PaletteTool::SwapColors)
    }
}
