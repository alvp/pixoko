//! The mechanism that the floating windows share (specs/hito-3-capas-interfaz.md, 3.4, and
//! specs/ui-single-column.md, decision 10): the strip where they can be, how a position in `u` is
//! placed inside it and how a drag moves it. Pure, without egui: the layers panel and the window of
//! options differ only in their size and in their content.
//!
//! A window is an `egui::Area` of exactly its rectangle (outside it everything reaches the canvas),
//! with a title bar that drags it. Its position is kept in `u` from the corner of the central strip
//! (between the menu and the status bar), so a change of the size of the window or of the scale
//! keeps it.

use crate::chrome::window_layout;
use crate::view::ScreenRect;

/// The central strip of the window (between the menu and the status bar), in physical pixels.
pub fn band(window_px: (i64, i64), n: u32) -> ScreenRect {
    let layout = window_layout(window_px, n);
    ScreenRect {
        x: 0,
        y: layout.menu.height,
        width: window_px.0.max(0),
        height: (layout.status.y - layout.menu.height).max(0),
    }
}

/// Where a window of `size_u` falls if its corner is at `pos_u` (`u` from the corner of the central
/// strip): the position is clamped so that it fits whole in the strip and aligned to the grid of
/// `u`. With a window smaller than the window it is anchored at the top left.
pub fn place(pos_u: (i64, i64), size_u: (i64, i64), window_px: (i64, i64), n: u32) -> ScreenRect {
    let k = i64::from(n.max(1));
    let band = band(window_px, n);
    let max_x = ((band.width - size_u.0 * k) / k).max(0);
    let max_y = ((band.height - size_u.1 * k) / k).max(0);
    let x_u = pos_u.0.clamp(0, max_x);
    let y_u = pos_u.1.clamp(0, max_y);
    ScreenRect {
        x: x_u * k,
        y: band.y + y_u * k,
        width: size_u.0 * k,
        height: size_u.1 * k,
    }
}

/// The position in `u` of a window placed at `rect`, from the corner of the central strip.
pub fn pos_of(rect: ScreenRect, window_px: (i64, i64), n: u32) -> (i64, i64) {
    let k = i64::from(n.max(1));
    (rect.x / k, (rect.y - band(window_px, n).y) / k)
}

/// The position `from` (in `u`) moved `dx_px`, `dy_px` physical pixels (in steps of `u`: rounded).
pub fn dragged(from: (i64, i64), dx_px: i64, dy_px: i64, n: u32) -> (i64, i64) {
    let k = n.max(1) as f64;
    (
        from.0 + (dx_px as f64 / k).round() as i64,
        from.1 + (dy_px as f64 / k).round() as i64,
    )
}
