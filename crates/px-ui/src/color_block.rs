//! Geometry and pure logic of the color block (specs/ui-color-block.md), without egui types: the
//! two bars under the grid with the swap button, and the pieces of the editor (the saturation and
//! brightness square, the hue and alpha bars and the RGB / HSL / HSV selector).
//!
//! The measures are constants in "interface pixels" `u` (`n = text_scale(ppp)` physical pixels
//! each). The bars are measured from the top edge of the tail under the grid; the pieces of the
//! editor, from the top-left corner of the editor. The functions work in physical pixels with `n`.

use px_core::Rgba;

use crate::color::Role;
use crate::view::ScreenRect;

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
pub fn bar_cell_rect(_role: Role, _cols: i64, _n: u32) -> ScreenRect {
    todo!("C4: bar_cell_rect")
}

/// Fill of the bar of `role`, in physical pixels from the corner of the tail.
pub fn bar_fill_rect(_role: Role, _cols: i64, _n: u32) -> ScreenRect {
    todo!("C4: bar_fill_rect")
}

/// The swap button, in physical pixels from the corner of the tail.
pub fn swap_rect(_cols: i64, _n: u32) -> ScreenRect {
    todo!("C4: swap_rect")
}

/// What is under `(x_px, y_px)`, in physical pixels from the corner of the tail.
pub fn bar_hit(_cols: i64, _x_px: i64, _y_px: i64, _n: u32) -> Option<BarHit> {
    todo!("C4: bar_hit")
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
pub fn piece_rect(_piece: Piece, _n: u32) -> ScreenRect {
    todo!("C4: piece_rect")
}

/// Rectangle of option `index` (0 RGB, 1 HSL, 2 HSV) of the selector, in physical pixels from the
/// corner of the editor.
pub fn selector_rect(_index: usize, _n: u32) -> ScreenRect {
    todo!("C4: selector_rect")
}

/// Saturation and brightness under `(x_px, y_px)` (from the corner of the editor), in `[0, 1]`:
/// clamped to the square, the four corners reached exactly.
pub fn square_value_at(_x_px: i64, _y_px: i64, _n: u32) -> (f32, f32) {
    todo!("C4: square_value_at")
}

/// Value of the hue bar (`0..=359`, 0 at the top) or of the alpha bar (`255..=0`, 255 at the top)
/// under `y_px` (from the corner of the editor). `Piece::Square` gives 0.
pub fn bar_value_at(_piece: Piece, _y_px: i64, _n: u32) -> u32 {
    todo!("C4: bar_value_at")
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
pub fn square_key(_saturation: u32, _brightness: u32, _arrow: Arrow, _shift: bool) -> (u32, u32) {
    todo!("C4: square_key")
}

/// Value of a bar after an arrow: the hue bar `↑` −1 and `↓` +1 (clamped to `0..=359`), the alpha
/// bar `↑` +1 and `↓` −1 (clamped to `0..=255`); 10 with `Shift`. Left and right do nothing.
pub fn bar_key(_piece: Piece, _value: u32, _arrow: Arrow, _shift: bool) -> u32 {
    todo!("C4: bar_key")
}

/// Texel `(x, y)` of the square of whole degrees of hue `hue`: `Hsv(hue, x / 111, 1 − y / 111)`.
pub fn sv_texel(_hue: u32, _x: u32, _y: u32) -> Rgba {
    todo!("C4: sv_texel")
}

/// The seven vertices of the hue bar, top to bottom: position along the bar in `[0, 1]` and the
/// color at S = V = 1, every 60 degrees.
pub fn hue_mesh() -> [(f32, Rgba); 7] {
    todo!("C4: hue_mesh")
}

/// The two ends of the alpha bar, top to bottom: `(r, g, b, 255)` and transparent.
pub fn alpha_mesh(_color: Rgba) -> [(f32, Rgba); 2] {
    todo!("C4: alpha_mesh")
}
