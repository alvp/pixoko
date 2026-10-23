//! Geometry of the palette panel, without egui types (specs/hito-2-panel-paletas.md;
//! specs/ui-composition.md: big indicator, compact grid and no fixed text).
//!
//! The measures are constants in "interface pixels" `u`: `u` is `n = text_scale(ppp)` physical
//! pixels, always an integer, so everything falls on whole physical pixels. The functions work in
//! physical pixels (`i64`) with `n` and with positions relative to the origin of the grid (the
//! top-left corner of the first swatch).

use px_core::Rgba;

use crate::color::Role;
use crate::icons::IconSet;

use crate::view::ScreenRect;

/// Swatches per row by default (the width of the column is chosen by the user, 8 or more).
pub const COLUMNS: usize = 16;
/// Side of a swatch.
pub const CELL_U: i64 = 12;
/// Gap between swatches: the ring of the chosen one fits exactly.
pub const GAP_U: i64 = 2;
/// Step between swatches: the height of a line of text.
pub const PITCH_U: i64 = CELL_U + GAP_U;
/// Side of a checker square (four per swatch).
pub const CHECK_U: i64 = 6;
/// Width of the panel with the default 16 columns: 4 of margin, 16 steps, 2 of gap, 2 of track and
/// 1 of line. With `k` columns it is [`panel_width_u`].
pub const PANEL_WIDTH_U: i64 = 233;
/// Origin of the grid in the panel: under the indicator band, a 1 u separator and 4 u of gap
/// for the rings.
pub const GRID_X_U: i64 = 4;
pub const GRID_Y_U: i64 = INDICATOR_H_U + 1 + 4;
/// Band of the color indicator, above the grid (specs/ui-composition.md, decision 4): 4 of
/// margin, 36 of indicator and 4 of margin. Everything else in the panel hangs from it.
pub const INDICATOR_H_U: i64 = 44;
/// Small rectangle of the indicator (foreground) and large one (background, under the small one):
/// origin and size, from the corner of the panel.
pub const IND_FG_X_U: i64 = 4;
pub const IND_FG_Y_U: i64 = 4;
pub const IND_FG_W_U: i64 = 40;
pub const IND_FG_H_U: i64 = 24;
pub const IND_BG_X_U: i64 = 20;
pub const IND_BG_Y_U: i64 = 8;
pub const IND_BG_W_U: i64 = 64;
pub const IND_BG_H_U: i64 = 32;
/// 1 u border (`SURFACE_3`) around each rectangle of the indicator; the one of the role the
/// open editor edits measures 2 u and is `ACCENT`.
pub const IND_BORDER_U: i64 = 1;
pub const IND_BORDER_EDIT_U: i64 = 2;
/// Text of the editors: origin and usable width.
pub const TEXT_X_U: i64 = 4;
pub const TEXT_WIDTH_U: i64 = 225;
/// 1 u strip between the band and the grid.
pub const SEPARATOR_Y_U: i64 = INDICATOR_H_U;
/// Track of the scroll indicator.
pub const TRACK_X_U: i64 = 230;
pub const TRACK_W_U: i64 = 2;
/// Right line of the panel (1 u).
pub const RIGHT_LINE_X_U: i64 = 232;
/// Thickness of the rings of the chosen swatch and of the one under the cursor.
pub const RING_ACTIVE_U: i64 = 2;
pub const RING_HOVER_U: i64 = 1;
/// Distance the pointer must travel (in `u`) for pressing a swatch to go from click to
/// drag (specs/hito-2-paleta-operaciones.md).
pub const DRAG_START_U: i64 = 4;
/// Half the gap between swatches: the capture of a drag's destination reaches that far.
pub const DROP_HALF_GAP_U: i64 = GAP_U / 2;
/// Thickness and length of the arms of the corners of the background ring.
pub const RING_BACKGROUND_U: i64 = 2;
pub const RING_BACKGROUND_ARM_U: i64 = 5;
/// Ring of the swatches of the rotation range and of the destination of a pending removal.
pub const RING_RANGE_U: i64 = 1;
pub const RING_DEST_U: i64 = 2;
/// Width of the bar that marks the destination of a drag.
pub const DROP_MARK_W_U: i64 = 2;
/// Checker of the swatches with alpha.
pub const CHECK_A: Rgba = Rgba::opaque(0x4a, 0x4a, 0x4a);
pub const CHECK_B: Rgba = Rgba::opaque(0x6e, 0x6e, 0x6e);

/// Rows (and columns) that `edges_u` walks: those of the largest palette (256 entries).
const MAX_ROWS: i64 = 16;

fn scale(n: u32) -> i64 {
    i64::from(n.max(1))
}

/// Width of the panel (the column) with `cols` columns of swatches, in `u`: `14·cols + 9`.
pub fn panel_width_u(cols: i64) -> i64 {
    14 * cols + 9
}

/// `x` of the track of the scroll indicator, in `u`: 3 u from the right edge of the panel.
pub fn track_x_u(cols: i64) -> i64 {
    panel_width_u(cols) - 3
}

/// `x` of the right line of the panel (1 u), in `u`: its last column.
pub fn right_line_x_u(cols: i64) -> i64 {
    panel_width_u(cols) - 1
}

/// Usable width of the texts of the panel, in `u`: 4 u of margin on each side.
pub fn text_width_u(cols: i64) -> i64 {
    panel_width_u(cols) - 8
}

/// All the coordinates, `x` and `y`, in `u` from the corner of the panel, that the layout uses:
/// panel edges, texts, separator, track and line, and the edges of each swatch (with its checker
/// and its rings) of a 256-entry palette.
pub fn edges_u() -> Vec<i64> {
    let mut edges = vec![
        0,
        PANEL_WIDTH_U,
        TEXT_X_U,
        TEXT_X_U + TEXT_WIDTH_U,
        SEPARATOR_Y_U,
        SEPARATOR_Y_U + 1,
        TRACK_X_U,
        TRACK_X_U + TRACK_W_U,
        RIGHT_LINE_X_U,
        RIGHT_LINE_X_U + 1,
        INDICATOR_H_U,
        IND_FG_X_U - IND_BORDER_U,
        IND_FG_X_U,
        IND_FG_X_U + IND_FG_W_U,
        IND_FG_X_U + IND_FG_W_U + IND_BORDER_U,
        IND_FG_X_U - IND_BORDER_EDIT_U,
        IND_FG_X_U + IND_FG_W_U + IND_BORDER_EDIT_U,
        IND_FG_Y_U - IND_BORDER_U,
        IND_FG_Y_U - IND_BORDER_EDIT_U,
        IND_FG_Y_U,
        IND_FG_Y_U + IND_FG_H_U,
        IND_FG_Y_U + IND_FG_H_U + IND_BORDER_U,
        IND_FG_Y_U + IND_FG_H_U + IND_BORDER_EDIT_U,
        IND_BG_X_U - IND_BORDER_U,
        IND_BG_X_U - IND_BORDER_EDIT_U,
        IND_BG_X_U,
        IND_BG_X_U + IND_BG_W_U,
        IND_BG_X_U + IND_BG_W_U + IND_BORDER_U,
        IND_BG_X_U + IND_BG_W_U + IND_BORDER_EDIT_U,
        IND_BG_Y_U - IND_BORDER_U,
        IND_BG_Y_U - IND_BORDER_EDIT_U,
        IND_BG_Y_U,
        IND_BG_Y_U + IND_BG_H_U,
        IND_BG_Y_U + IND_BG_H_U + IND_BORDER_U,
        IND_BG_Y_U + IND_BG_H_U + IND_BORDER_EDIT_U,
    ];
    for step in 0..MAX_ROWS {
        for origin in [GRID_X_U, GRID_Y_U] {
            let start = origin + step * PITCH_U;
            edges.extend([
                start - RING_ACTIVE_U - 1,
                start - RING_ACTIVE_U,
                start - RING_HOVER_U,
                start,
                start + CHECK_U,
                start + CELL_U,
                start + CELL_U + RING_HOVER_U,
                start + CELL_U + RING_ACTIVE_U,
                start + CELL_U + RING_ACTIVE_U + 1,
            ]);
        }
    }
    edges.sort_unstable();
    edges.dedup();
    edges
}

/// Rows taken by `count` swatches in the default 16 columns.
pub fn total_rows(count: usize) -> usize {
    total_rows_cols(count, COLUMNS)
}

/// Rows taken by `count` swatches in `columns` columns.
pub fn total_rows_cols(count: usize, columns: usize) -> usize {
    count.div_ceil(columns.max(1))
}

/// Whole rows that fit in a panel of `panel_h_px` physical pixels of height:
/// `floor((height - GRID_Y_U·n) / (14·n))`, 0 if negative. Each row counts its trailing gap.
pub fn visible_rows(panel_h_px: i64, n: u32) -> usize {
    let n = scale(n);
    let available = panel_h_px.saturating_sub(GRID_Y_U * n);
    if available < 0 {
        return 0;
    }
    usize::try_from(available / (PITCH_U * n)).unwrap_or(usize::MAX)
}

/// Like [`visible_rows`], but with `reserved_px` physical pixels at the bottom of the panel that the
/// grid does not use (icon row and color editor). `visible_rows(h, n)` is `visible_rows_reserved(h, 0, n)`.
pub fn visible_rows_reserved(panel_h_px: i64, reserved_px: i64, n: u32) -> usize {
    visible_rows(panel_h_px.saturating_sub(reserved_px), n)
}

/// Mayor primera fila posible: `total − max(visible, 1)`, saturado.
pub fn max_first_row(total: usize, visible: usize) -> usize {
    total.saturating_sub(visible.max(1))
}

/// Visible grid: which swatch each rectangle holds. Positions in physical pixels and
/// relative to the origin of the grid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Grid {
    n: i64,
    columns: usize,
    count: usize,
    visible_rows: usize,
    first_row: usize,
}

impl Grid {
    /// The grid of the default 16 columns.
    pub fn new(n: u32, count: usize, visible_rows: usize, first_row: usize) -> Grid {
        Grid::with_columns(n, COLUMNS, count, visible_rows, first_row)
    }

    /// The grid of `columns` columns (the column of the window is as wide as the user chose).
    pub fn with_columns(
        n: u32,
        columns: usize,
        count: usize,
        visible_rows: usize,
        first_row: usize,
    ) -> Grid {
        Grid {
            n: scale(n),
            columns: columns.max(1),
            count,
            visible_rows,
            first_row,
        }
    }

    /// Rectangle of swatch `index`, or `None` if it does not exist or its row is not seen.
    pub fn cell_rect(&self, index: usize) -> Option<ScreenRect> {
        if index >= self.count {
            return None;
        }
        let (row, col) = (index / self.columns, index % self.columns);
        let shown = row.checked_sub(self.first_row)?;
        if shown >= self.visible_rows {
            return None;
        }
        let pitch = PITCH_U * self.n;
        Some(ScreenRect {
            x: i64::try_from(col).ok()? * pitch,
            y: i64::try_from(shown).ok()?.checked_mul(pitch)?,
            width: CELL_U * self.n,
            height: CELL_U * self.n,
        })
    }

    /// Destination of a drag under the point `(x, y)`: the index the moved entry will have. Each
    /// swatch captures its whole step (with half a gap on each side: no dead gaps); past the end of the
    /// last row the last index holds; outside the zone, `None`.
    pub fn drop_index(&self, x: i64, y: i64) -> Option<usize> {
        let half = DROP_HALF_GAP_U * self.n;
        let pitch = PITCH_U * self.n;
        let (x, y) = (x.checked_add(half)?, y.checked_add(half)?);
        if x < 0 || y < 0 {
            return None;
        }
        let col = usize::try_from(x / pitch).ok()?;
        let shown = usize::try_from(y / pitch).ok()?;
        let rows_left = total_rows_cols(self.count, self.columns).saturating_sub(self.first_row);
        if col >= self.columns || shown >= self.visible_rows.min(rows_left) {
            return None;
        }
        let index = self
            .first_row
            .checked_add(shown)?
            .checked_mul(self.columns)?
            .checked_add(col)?;
        Some(index.min(self.count.checked_sub(1)?))
    }

    /// New length of the palette that the point `(x, y)` asks for when the handle is dragged: the
    /// index of the cell under it (the cell of the handle is the one after the last entry),
    /// clamped to `1..=256`. Like [`Grid::drop_index`] each cell captures its whole step (half a
    /// gap on each side), but the zone is **every visible row of the grid** (`count` cells, with
    /// or without a swatch in them) and the index is not clamped to the last cell. Outside the
    /// zone, `None`.
    pub fn len_at(&self, x: i64, y: i64) -> Option<u16> {
        let half = DROP_HALF_GAP_U * self.n;
        let pitch = PITCH_U * self.n;
        let (x, y) = (x.checked_add(half)?, y.checked_add(half)?);
        if x < 0 || y < 0 {
            return None;
        }
        let col = usize::try_from(x / pitch).ok()?;
        let shown = usize::try_from(y / pitch).ok()?;
        let rows_left = total_rows_cols(self.count, self.columns).saturating_sub(self.first_row);
        if col >= self.columns || shown >= self.visible_rows.min(rows_left) {
            return None;
        }
        let index = self
            .first_row
            .checked_add(shown)?
            .checked_mul(self.columns)?
            .checked_add(col)?;
        u16::try_from(index.clamp(1, 256)).ok()
    }

    /// Like [`Grid::len_at`] for a drag that is alive: a point outside the zone asks for the
    /// length of the nearest cell (the column and the row are clamped to the zone). `None` only if
    /// the zone has no visible row.
    pub fn len_nearest(&self, x: i64, y: i64) -> Option<u16> {
        let half = DROP_HALF_GAP_U * self.n;
        let pitch = PITCH_U * self.n;
        let rows_left = total_rows_cols(self.count, self.columns).saturating_sub(self.first_row);
        let rows = self.visible_rows.min(rows_left);
        if rows == 0 {
            return None;
        }
        let col = (x.saturating_add(half).div_euclid(pitch)).clamp(0, self.columns as i64 - 1);
        let shown = (y.saturating_add(half).div_euclid(pitch)).clamp(0, rows as i64 - 1);
        let index = (self.first_row + usize::try_from(shown).ok()?)
            .checked_mul(self.columns)?
            .checked_add(usize::try_from(col).ok()?)?;
        u16::try_from(index.clamp(1, 256)).ok()
    }

    /// Swatch under the point `(x, y)`, or `None` in a gap, outside the grid, in a row
    /// that is not seen or without a swatch.
    pub fn index_at(&self, x: i64, y: i64) -> Option<usize> {
        if x < 0 || y < 0 {
            return None;
        }
        let (pitch, cell) = (PITCH_U * self.n, CELL_U * self.n);
        if x % pitch >= cell || y % pitch >= cell {
            return None;
        }
        let col = usize::try_from(x / pitch).ok()?;
        let shown = usize::try_from(y / pitch).ok()?;
        if col >= self.columns || shown >= self.visible_rows {
            return None;
        }
        let index = self
            .first_row
            .checked_add(shown)?
            .checked_mul(self.columns)?
            .checked_add(col)?;
        (index < self.count).then_some(index)
    }
}

/// Vertical scroll: first visible row and accumulated physical pixels of the next row
/// (`0 ≤ rest < 14·n`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Scroll {
    pub first_row: usize,
    pub rest: i64,
}

impl Scroll {
    /// Scrolls `delta_px` physical pixels (positive: later rows). Adds saturating to
    /// `rest`, advances `div_euclid(14·n)` rows and clamps to `0..=max_first_row`; `rest` ends at
    /// 0 if it was clamped or if `first_row` is the maximum (there is nothing more to accumulate).
    pub fn by(&mut self, delta_px: i64, n: u32, total: usize, visible: usize) {
        let pitch = PITCH_U * scale(n);
        let pending = self.rest.saturating_add(delta_px);
        let target = i128::try_from(self.first_row)
            .unwrap_or(i128::MAX)
            .saturating_add(i128::from(pending.div_euclid(pitch)));
        let max = max_first_row(total, visible);
        match usize::try_from(target) {
            Ok(row) if row < max => {
                self.first_row = row;
                self.rest = pending.rem_euclid(pitch);
            }
            Ok(_) => {
                self.first_row = max;
                self.rest = 0;
            }
            Err(_) => {
                self.first_row = 0;
                self.rest = 0;
            }
        }
        if self.first_row == max {
            self.rest = 0;
        }
    }

    /// Keeps `first_row` in `0..=max_first_row` (the window may grow).
    pub fn clamp(&mut self, total: usize, visible: usize) {
        let max = max_first_row(total, visible);
        if self.first_row >= max {
            self.first_row = max;
            self.rest = 0;
        }
    }
}

impl Scroll {
    /// Moves `first_row` the minimum for `row` to be visible and clamps it to
    /// `0..=max_first_row`; `rest` ends at 0 if it moved. Nothing with `row ≥ total`.
    pub fn reveal(&mut self, row: usize, total: usize, visible: usize) {
        if row >= total {
            return;
        }
        let visible = visible.max(1);
        let before = self.first_row;
        if row < self.first_row {
            self.first_row = row;
        } else if row >= self.first_row.saturating_add(visible) {
            self.first_row = row + 1 - visible;
        }
        self.first_row = self.first_row.min(max_first_row(total, visible));
        if self.first_row != before {
            self.rest = 0;
        }
    }
}

/// State of the panel between frames, besides the one of the grid ([`Scroll`]): the scroll
/// of the whole panel when it does not fit (specs/hito-2-panel-ventana-baja.md) and what is needed
/// to show the chosen row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PanelView {
    /// Scroll of the whole panel, in physical pixels (a multiple of `n`).
    pub offset_px: i64,
    /// Pixels that do not reach `n` yet.
    pub rest_px: i64,
    /// Visible rows of the grid in the previous frame.
    pub last_visible: Option<usize>,
    /// The chosen row has to be shown (it changed by keyboard, undo or redo).
    pub reveal: bool,
    /// The textures of the icons of the palette buttons (empty: painted without icon).
    pub icons: IconSet,
    /// The color editor is open. `true` by default (the panel as it was before the editor became
    /// on demand); the app drives it from `Session::editor_open` (specs/ui-composition.md).
    pub editor_open: bool,
    /// The state of `editor_open` in the previous frame, to know when it opens or closes.
    pub last_editor_open: bool,
    /// The scroll before the editor opened, brought back when it closes.
    pub saved_offset_px: Option<i64>,
}

impl Default for PanelView {
    fn default() -> PanelView {
        PanelView {
            offset_px: 0,
            rest_px: 0,
            last_visible: None,
            reveal: false,
            icons: IconSet::default(),
            editor_open: true,
            last_editor_open: true,
            saved_offset_px: None,
        }
    }
}

impl PanelView {
    /// Follows the opening and the closing of the editor (`open`) in the scroll of the whole block:
    /// opening takes it to `max_open_px`, the maximum of the open block, so the editor is in sight;
    /// closing brings back the value it had before opening. Called once per frame; it does nothing
    /// while `open` does not change.
    pub fn sync_editor(&mut self, open: bool, max_open_px: i64) {
        if open == self.last_editor_open {
            return;
        }
        self.last_editor_open = open;
        if open {
            self.saved_offset_px = Some(self.offset_px);
            self.offset_px = max_open_px.max(0);
            self.rest_px = 0;
        } else if let Some(saved) = self.saved_offset_px.take() {
            self.offset_px = saved;
            self.rest_px = 0;
        }
    }

    /// Scrolls `delta_px` (positive: further down) in steps of `n`, keeping the rest, and clamps to
    /// `0..=max_offset_px` (with the rest at 0 when clamping and at the maximum; at 0 it accumulates,
    /// so that jumps of less than `n` come from the top).
    pub fn scroll_by(&mut self, delta_px: i64, n: u32, max_offset_px: i64) {
        let step = scale(n);
        let pending = self.rest_px.saturating_add(delta_px);
        let target = self
            .offset_px
            .saturating_add(pending.div_euclid(step).saturating_mul(step));
        let max = max_offset_px.max(0);
        self.rest_px = pending.rem_euclid(step);
        self.offset_px = target.clamp(0, max);
        if self.offset_px != target || self.offset_px == max {
            self.rest_px = 0;
        }
    }
}

/// Indicator of the scroll of the whole panel: `(y, height)` relative to the track, which measures
/// `panel_h_px`. Minimum height `2·n`. `None` if the panel fits (`block_h_px ≤ panel_h_px`).
pub fn panel_thumb(offset_px: i64, panel_h_px: i64, block_h_px: i64, n: u32) -> Option<(i64, i64)> {
    if panel_h_px <= 0 || block_h_px <= panel_h_px {
        return None;
    }
    let (track, block) = (i128::from(panel_h_px), i128::from(block_h_px));
    let height = (track * track / block).max(i128::from(2 * scale(n)));
    let max_offset = block - track;
    let y = (track - height) * i128::from(offset_px.clamp(0, i64::try_from(max_offset).ok()?))
        / max_offset;
    Some((
        i64::try_from(y.max(0)).ok()?,
        i64::try_from(height.min(track)).ok()?,
    ))
}

/// Scroll indicator: `(y, height)` relative to the track, which measures `visible·14·n`.
/// Minimum height `2·n`. `None` if not needed (`total ≤ visible`) or no row is seen.
pub fn thumb(first_row: usize, visible: usize, total: usize, n: u32) -> Option<(i64, i64)> {
    if visible == 0 || total <= visible {
        return None;
    }
    let n = scale(n);
    let (visible, total) = (i128::try_from(visible).ok()?, i128::try_from(total).ok()?);
    let track = visible * i128::from(PITCH_U * n);
    let height = (track * visible / total).max(i128::from(2 * n));
    let first = i128::try_from(first_row).ok()?;
    let y = (track * first / total).min(track - height);
    Some((i64::try_from(y).ok()?, i64::try_from(height).ok()?))
}

fn rect(x: i64, y: i64, width: i64, height: i64) -> ScreenRect {
    ScreenRect {
        x,
        y,
        width,
        height,
    }
}

/// Rectangles of a swatch, relative to its corner and in painting order: alpha 255, the whole
/// color; alpha 0, only the checker (four squares of `CHECK_U·n`); in between, the checker and on top
/// the color with its alpha.
pub fn swatch_parts(n: u32, color: Rgba) -> Vec<(ScreenRect, Rgba)> {
    let n = scale(n);
    let whole = rect(0, 0, CELL_U * n, CELL_U * n);
    if color.a == 255 {
        return vec![(whole, color)];
    }
    let check = CHECK_U * n;
    let mut parts: Vec<(ScreenRect, Rgba)> = [
        (0, 0, CHECK_A),
        (check, 0, CHECK_B),
        (0, check, CHECK_B),
        (check, check, CHECK_A),
    ]
    .into_iter()
    .map(|(x, y, tone)| (rect(x, y, check, check), tone))
    .collect();
    if color.a > 0 {
        parts.push((whole, color));
    }
    parts
}

/// Bar that marks the destination of a drag next to `cell`: in the gap on its left
/// (`after` false) or on its right, centered in it, with the height of a step.
pub fn drop_mark_rect(cell: ScreenRect, after: bool, n: u32) -> ScreenRect {
    let n = scale(n);
    let x = if after {
        cell.x + cell.width + (GAP_U - DROP_MARK_W_U) / 2 * n
    } else {
        cell.x - (GAP_U + DROP_MARK_W_U) / 2 * n
    };
    rect(
        x,
        cell.y - DROP_HALF_GAP_U * n,
        DROP_MARK_W_U * n,
        PITCH_U * n,
    )
}

fn indicator_rect(x: i64, y: i64, width: i64, height: i64, n: u32) -> ScreenRect {
    let k = scale(n);
    rect(x * k, y * k, width * k, height * k)
}

/// The two rectangles of the indicator in physical pixels from the corner of the panel:
/// `(foreground, background)`. The small one is painted on top of the large one.
pub fn indicator_rects(n: u32) -> (ScreenRect, ScreenRect) {
    (
        indicator_rect(IND_FG_X_U, IND_FG_Y_U, IND_FG_W_U, IND_FG_H_U, n),
        indicator_rect(IND_BG_X_U, IND_BG_Y_U, IND_BG_W_U, IND_BG_H_U, n),
    )
}

fn contains(r: &ScreenRect, x: i64, y: i64) -> bool {
    x >= r.x && x < r.x + r.width && y >= r.y && y < r.y + r.height
}

/// Role of the indicator rectangle under `(x, y)` (pixels from the corner of the panel): where they
/// overlap the small one wins; the empty corners of the box that contains them belong to nobody.
pub fn indicator_hit(x: i64, y: i64, n: u32) -> Option<Role> {
    let (fg, bg) = indicator_rects(n);
    if contains(&fg, x, y) {
        Some(Role::Foreground)
    } else if contains(&bg, x, y) {
        Some(Role::Background)
    } else {
        None
    }
}

/// Parts of a rectangle of `width × height` pixels with color `color`, in painting order and
/// relative to its corner: alpha 255, one rectangle; alpha 0, the checker of squares of
/// `square` pixels (clipped to the edge) without color; alpha 1 to 254, the checker and on top the
/// rectangle with its alpha. It is `swatch_parts` for any size.
pub fn block_parts(width: i64, height: i64, square: i64, color: Rgba) -> Vec<(ScreenRect, Rgba)> {
    let solid = (rect(0, 0, width, height), color);
    if color.a == 255 {
        return vec![solid];
    }
    let mut parts = Vec::new();
    let square = square.max(1);
    let (mut j, mut y) = (0, 0);
    while y < height {
        let (mut i, mut x) = (0, 0);
        while x < width {
            let tone = if (i + j) % 2 == 0 { CHECK_A } else { CHECK_B };
            parts.push((
                rect(x, y, square.min(width - x), square.min(height - y)),
                tone,
            ));
            i += 1;
            x += square;
        }
        j += 1;
        y += square;
    }
    if color.a > 0 {
        parts.push(solid);
    }
    parts
}

/// Ring of the background: four L-shaped corners (eight disjoint rectangles) of `thickness` pixels
/// and arms of `arm` pixels, at `offset` pixels from the swatch (0: attached to it). Order: by
/// corner (top left, top right, bottom left, bottom right), the horizontal arm and then the
/// vertical one.
pub fn corner_rects(cell: ScreenRect, offset: i64, thickness: i64, arm: i64) -> [ScreenRect; 8] {
    let t = thickness;
    let (x0, y0) = (cell.x - offset - t, cell.y - offset - t);
    let (x1, y1) = (
        cell.x + cell.width + offset + t,
        cell.y + cell.height + offset + t,
    );
    let tall = (arm - t).max(0);
    [
        rect(x0, y0, arm, t),
        rect(x0, y0 + t, t, tall),
        rect(x1 - arm, y0, arm, t),
        rect(x1 - t, y0 + t, t, tall),
        rect(x0, y1 - t, arm, t),
        rect(x0, y1 - arm, t, tall),
        rect(x1 - arm, y1 - t, arm, t),
        rect(x1 - t, y1 - arm, t, tall),
    ]
}

/// Ring of `thickness` pixels around `cell`: top, bottom, left and right. The top and bottom
/// strips carry the corners.
pub fn ring_rects(cell: ScreenRect, thickness: i64) -> [ScreenRect; 4] {
    let t = thickness;
    [
        rect(cell.x - t, cell.y - t, cell.width + 2 * t, t),
        rect(cell.x - t, cell.y + cell.height, cell.width + 2 * t, t),
        rect(cell.x - t, cell.y, t, cell.height),
        rect(cell.x + cell.width, cell.y, t, cell.height),
    ]
}
