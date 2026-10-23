//! Geometry and pure logic of the color block (specs/ui-color-block.md), without egui types: the
//! two bars under the grid with the swap button, and the pieces of the editor (the saturation and
//! brightness square, the hue and alpha bars and the RGB / HSL / HSV selector).
//!
//! The measures are constants in "interface pixels" `u` (`n = text_scale(ppp)` physical pixels
//! each). The bars are measured from the top edge of the tail under the grid; the pieces of the
//! editor, from the top-left corner of the editor. The functions work in physical pixels with `n`.

use px_core::Rgba;

use crate::color::Role;
use crate::color_edit::slider_value_at;
use crate::color_hsv::Hsv;
use crate::palette_grid::panel_width_u;
use crate::view::ScreenRect;

fn scale(n: u32) -> i64 {
    i64::from(n.max(1))
}

fn rect(x: i64, y: i64, width: i64, height: i64) -> ScreenRect {
    ScreenRect {
        x,
        y,
        width,
        height,
    }
}

/// Height of the cell of a bar, in `u`: a 16 u fill and a 2 u border outside it.
pub const BAR_CELL_H_U: i64 = 20;
/// The cell starts 2 u from the left of the column; its fill, 4 u.
pub const BAR_CELL_X_U: i64 = 2;
pub const BAR_FILL_X_U: i64 = 4;
/// The fill is 2 u under the top edge of its cell and 16 u high.
pub const BAR_FILL_INSET_U: i64 = 2;
pub const BAR_FILL_H_U: i64 = 16;
/// Thickness of the border of the bar the hexadecimal field writes to (`ACCENT`) and of the other
/// one (`SURFACE_3`).
pub const BAR_BORDER_EDIT_U: i64 = 2;
pub const BAR_BORDER_U: i64 = 1;
/// Height of the two bars together.
pub const BARS_H_U: i64 = 2 * BAR_CELL_H_U;
/// The swap button: 24 u square, centered vertically between both bars, 2 u from the right edge
/// of the column.
pub const SWAP_SIZE_U: i64 = 24;
pub const SWAP_Y_U: i64 = (BARS_H_U - SWAP_SIZE_U) / 2;
pub const SWAP_MARGIN_U: i64 = 2;

/// What the pointer is over in the block of the bars.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BarHit {
    Bar(Role),
    Swap,
}

/// Cell of the bar of `role` (the click zone), in physical pixels from the corner of the tail, in a
/// column of `cols` columns of swatches.
pub fn bar_cell_rect(role: Role, cols: i64, n: u32) -> ScreenRect {
    let k = scale(n);
    let width = panel_width_u(cols) - 30;
    rect(
        BAR_CELL_X_U * k,
        role_row(role) * BAR_CELL_H_U * k,
        width * k,
        BAR_CELL_H_U * k,
    )
}

/// 0 for the foreground (the bar on top) and 1 for the background.
fn role_row(role: Role) -> i64 {
    match role {
        Role::Foreground => 0,
        Role::Background => 1,
    }
}

/// Fill of the bar of `role`, in physical pixels from the corner of the tail.
pub fn bar_fill_rect(role: Role, cols: i64, n: u32) -> ScreenRect {
    let k = scale(n);
    let width = panel_width_u(cols) - 34;
    rect(
        BAR_FILL_X_U * k,
        (role_row(role) * BAR_CELL_H_U + BAR_FILL_INSET_U) * k,
        width * k,
        BAR_FILL_H_U * k,
    )
}

/// The swap button, in physical pixels from the corner of the tail.
pub fn swap_rect(cols: i64, n: u32) -> ScreenRect {
    let k = scale(n);
    rect(
        (panel_width_u(cols) - SWAP_SIZE_U - SWAP_MARGIN_U) * k,
        SWAP_Y_U * k,
        SWAP_SIZE_U * k,
        SWAP_SIZE_U * k,
    )
}

/// What is under `(x_px, y_px)`, in physical pixels from the corner of the tail.
pub fn bar_hit(cols: i64, x_px: i64, y_px: i64, n: u32) -> Option<BarHit> {
    let inside =
        |r: ScreenRect| x_px >= r.x && x_px < r.x + r.width && y_px >= r.y && y_px < r.y + r.height;
    if inside(swap_rect(cols, n)) {
        return Some(BarHit::Swap);
    }
    [Role::Foreground, Role::Background]
        .into_iter()
        .find(|role| inside(bar_cell_rect(*role, cols, n)))
        .map(BarHit::Bar)
}

/// The pieces of the editor that are dragged and have their own keyboard focus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Piece {
    Square,
    HueBar,
    AlphaBar,
}

/// Side of the square and length of the two bars, in `u`; the square is at `(4, 4)`.
pub const SQUARE_SIDE_U: i64 = 112;
pub const SQUARE_X_U: i64 = 4;
pub const SQUARE_Y_U: i64 = 4;
/// Width and position of the hue and alpha bars.
pub const VBAR_W_U: i64 = 12;
pub const HUE_BAR_X_U: i64 = 120;
pub const ALPHA_BAR_X_U: i64 = 136;
/// Row of the RGB / HSL / HSV selector, its height and the `x` of its three options (39 u each).
pub const SELECTOR_Y_U: i64 = 120;
pub const SELECTOR_H_U: i64 = 14;
pub const SELECTOR_X_U: [i64; 3] = [4, 51, 98];
pub const SELECTOR_W_U: i64 = 39;

/// Rectangle of `piece`, in physical pixels from the corner of the editor.
pub fn piece_rect(piece: Piece, n: u32) -> ScreenRect {
    let k = scale(n);
    let x = match piece {
        Piece::Square => SQUARE_X_U,
        Piece::HueBar => HUE_BAR_X_U,
        Piece::AlphaBar => ALPHA_BAR_X_U,
    };
    let width = match piece {
        Piece::Square => SQUARE_SIDE_U,
        Piece::HueBar | Piece::AlphaBar => VBAR_W_U,
    };
    rect(x * k, SQUARE_Y_U * k, width * k, SQUARE_SIDE_U * k)
}

/// Rectangle of option `index` (0 RGB, 1 HSL, 2 HSV) of the selector, in physical pixels from the
/// corner of the editor.
pub fn selector_rect(index: usize, n: u32) -> ScreenRect {
    let k = scale(n);
    let x = SELECTOR_X_U[index.min(SELECTOR_X_U.len() - 1)];
    rect(x * k, SELECTOR_Y_U * k, SELECTOR_W_U * k, SELECTOR_H_U * k)
}

/// Saturation and brightness under `(x_px, y_px)` (from the corner of the editor), in `[0, 1]`:
/// clamped to the square, the four corners reached exactly.
pub fn square_value_at(x_px: i64, y_px: i64, n: u32) -> (f32, f32) {
    let k = scale(n);
    let side = SQUARE_SIDE_U * k;
    let span = (side - 1) as f32;
    let dx = (x_px - SQUARE_X_U * k).clamp(0, side - 1);
    let dy = (y_px - SQUARE_Y_U * k).clamp(0, side - 1);
    (dx as f32 / span, 1.0 - dy as f32 / span)
}

/// Value of the hue bar (`0..=359`, 0 at the top) or of the alpha bar (`255..=0`, 255 at the top)
/// under `y_px` (from the corner of the editor). `Piece::Square` gives 0.
pub fn bar_value_at(piece: Piece, y_px: i64, n: u32) -> u32 {
    let k = scale(n);
    let dy = y_px - SQUARE_Y_U * k;
    let side = SQUARE_SIDE_U * k;
    match piece {
        Piece::Square => 0,
        Piece::HueBar => slider_value_at(dy, side, 359),
        Piece::AlphaBar => 255 - slider_value_at(dy, side, 255),
    }
}

/// The arrow keys.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Arrow {
    Left,
    Right,
    Up,
    Down,
}

/// Saturation and brightness in percent after an arrow: `←` `→` saturation −1 / +1, `↓` `↑`
/// brightness −1 / +1 (10 with `Shift`), clamped to `0..=100`.
pub fn square_key(saturation: u32, brightness: u32, arrow: Arrow, shift: bool) -> (u32, u32) {
    let step = if shift { 10 } else { 1 };
    let up = |value: u32| (value + step).min(100);
    let down = |value: u32| value.saturating_sub(step);
    match arrow {
        Arrow::Left => (down(saturation), brightness),
        Arrow::Right => (up(saturation), brightness),
        Arrow::Down => (saturation, down(brightness)),
        Arrow::Up => (saturation, up(brightness)),
    }
}

/// Value of a bar after an arrow: the hue bar `↑` −1 and `↓` +1 (clamped to `0..=359`), the alpha
/// bar `↑` +1 and `↓` −1 (clamped to `0..=255`); 10 with `Shift`. Left and right do nothing.
pub fn bar_key(piece: Piece, value: u32, arrow: Arrow, shift: bool) -> u32 {
    let step = if shift { 10 } else { 1 };
    match (piece, arrow) {
        (Piece::HueBar, Arrow::Up) => value.saturating_sub(step),
        (Piece::HueBar, Arrow::Down) => (value + step).min(359),
        (Piece::AlphaBar, Arrow::Up) => (value + step).min(255),
        (Piece::AlphaBar, Arrow::Down) => value.saturating_sub(step),
        _ => value,
    }
}

/// Texel `(x, y)` of the square of whole degrees of hue `hue`: `Hsv(hue, x / 111, 1 − y / 111)`.
pub fn sv_texel(hue: u32, x: u32, y: u32) -> Rgba {
    let last = (SQUARE_SIDE_U - 1) as f32;
    Hsv::new(hue as f32, x as f32 / last, 1.0 - y as f32 / last).to_rgba(255)
}

/// The seven vertices of the hue bar, top to bottom: position along the bar in `[0, 1]` and the
/// color at S = V = 1, every 60 degrees.
pub fn hue_mesh() -> [(f32, Rgba); 7] {
    std::array::from_fn(|i| {
        (
            i as f32 / 6.0,
            Hsv::new(60.0 * i as f32, 1.0, 1.0).to_rgba(255),
        )
    })
}

/// The two ends of the alpha bar, top to bottom: `(r, g, b, 255)` and transparent.
pub fn alpha_mesh(color: Rgba) -> [(f32, Rgba); 2] {
    [
        (0.0, Rgba::opaque(color.r, color.g, color.b)),
        (1.0, Rgba::TRANSPARENT),
    ]
}

/// Which of the two tones of a mark a rectangle is painted with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarkTone {
    /// `TEXT`.
    Light,
    /// `SURFACE_0`.
    Edge,
}

/// Whole `u` of the mark of the square: its box is 9 u, centered on the texel of `(s, v)`.
pub const SQUARE_MARK_U: i64 = 9;

/// Rectangles of the mark of the square for saturation `s` and brightness `v` (both in `[0, 1]`),
/// in physical pixels from the corner of the editor: a ring of 9 u with 1 u of `TEXT` between two
/// of 1 u of `SURFACE_0`, so that with any color of the square at least one has 3:1.
pub fn square_mark(s: f32, v: f32, n: u32) -> Vec<(ScreenRect, MarkTone)> {
    let k = scale(n);
    let last = (SQUARE_SIDE_U - 1) as f32;
    let tx = (s.clamp(0.0, 1.0) * last).round() as i64;
    let ty = ((1.0 - v.clamp(0.0, 1.0)) * last).round() as i64;
    let origin_x = (SQUARE_X_U + tx - SQUARE_MARK_U / 2) * k;
    let origin_y = (SQUARE_Y_U + ty - SQUARE_MARK_U / 2) * k;
    let mut parts = Vec::new();
    for (inset, tone) in [
        (0, MarkTone::Edge),
        (1, MarkTone::Light),
        (2, MarkTone::Edge),
    ] {
        let side = (SQUARE_MARK_U - 2 * inset) * k;
        let (x, y) = (origin_x + inset * k, origin_y + inset * k);
        for part in [
            rect(x, y, side, k),
            rect(x, y + side - k, side, k),
            rect(x, y + k, k, side - 2 * k),
            rect(x + side - k, y + k, k, side - 2 * k),
        ] {
            parts.push((part, tone));
        }
    }
    parts
}

/// Rectangles of the mark of a bar at `value` (the hue, 0 at the top, or the alpha, 255 at the
/// top), in physical pixels from the corner of the editor: a strip 4 u high (1 u `SURFACE_0`, 2 u
/// `TEXT`, 1 u `SURFACE_0`) that sticks out 1 u on each side, inside the 4 u gaps.
pub fn bar_mark(piece: Piece, value: u32, n: u32) -> Vec<(ScreenRect, MarkTone)> {
    let k = scale(n);
    let bar = piece_rect(piece, n);
    let (length, max, from_top) = match piece {
        Piece::HueBar => (bar.height, 359, value.min(359)),
        _ => (bar.height, 255, 255 - value.min(255)),
    };
    let at = crate::color_edit::slider_x(from_top, length, max);
    let top = bar.y + at - 2 * k;
    let (x, width) = (bar.x - k, bar.width + 2 * k);
    vec![
        (rect(x, top, width, k), MarkTone::Edge),
        (rect(x, top + k, width, 2 * k), MarkTone::Light),
        (rect(x, top + 3 * k, width, k), MarkTone::Edge),
    ]
}
