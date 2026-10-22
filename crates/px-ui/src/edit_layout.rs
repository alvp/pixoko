//! Geometry of the color editor, without egui types (specs/hito-2-editar-color.md), and of the row of
//! palette icons (specs/ui-composition.md).
//!
//! The measures are constants in "interface pixels" `u` (`n = text_scale(ppp)` physical pixels
//! each) from the top-left corner of the editor, which starts at `top_px` (anchored to the bottom of
//! the panel). The functions work in physical pixels with `n`.

use px_core::Mode;

use crate::chrome::{MENU_H_U, toolbar_width_u};
use crate::color_edit::{Control, controls};
use crate::icon_button::{BUTTON_U, GAP_U};
use crate::icons::Icon;
use crate::palette_grid::{GRID_Y_U, PANEL_WIDTH_U, PITCH_U, TEXT_WIDTH_U, visible_rows_reserved};
use crate::status::bar_height_px;
use crate::text::text_scale;

/// 1 u strip at the top of the editor.
pub const SEPARATOR_Y_U: i64 = 0;
/// Header: origin of the text.
pub const HEADER_X_U: i64 = 4;
pub const HEADER_Y_U: i64 = 4;
/// Height of a row of text and of a slider.
pub const ROW_H_U: i64 = 14;
/// First row of sliders.
pub const FIRST_ROW_Y_U: i64 = 22;
/// Gap between groups of sliders (RGB | HSL | A).
pub const GROUP_GAP_U: i64 = 4;
/// Inside a row: letter, track, thumb, value and active zone.
pub const LABEL_X_U: i64 = 4;
pub const LABEL_W_U: i64 = 7;
pub const TRACK_X_U: i64 = 15;
pub const TRACK_W_U: i64 = 184;
pub const TRACK_H_U: i64 = 4;
pub const THUMB_W_U: i64 = 3;
pub const THUMB_H_U: i64 = 12;
pub const VALUE_X_U: i64 = 203;
pub const VALUE_W_U: i64 = 21;
/// Active zone of the row: from `HIT_X0_U` (included) to `HIT_X1_U` (excluded).
pub const HIT_X0_U: i64 = 13;
pub const HIT_X1_U: i64 = 201;
/// Hexadecimal field.
pub const FIELD_X_U: i64 = 4;
pub const FIELD_W_U: i64 = 69;
pub const FIELD_H_U: i64 = 18;
pub const FIELD_PAD_U: i64 = 3;
/// Gap between blocks (controls, field and notice) and bottom margin.
pub const BLOCK_GAP_U: i64 = 4;
/// Minimum width of the canvas in the window, in interface pixels
/// (specs/hito-2-panel-ventana-baja.md).
pub const MIN_CANVAS_WIDTH_U: i64 = 128;
/// Lines of the notice.
pub const NOTICE_LINES: i64 = 2;
/// Buttons of the palette (indexed only; specs/interfaz-iconos.md, specs/ui-composition.md): a row
/// of five 24 u icons (1 u gap inside a group and 8 u between groups: add and remove | rotate to one
/// side and the other | transparent) or, with a pending removal, `Confirmar` and `Cancel` as text in
/// their place (two 110 u buttons and a 5 u gap). Rows, height, origin, width of the two text
/// buttons and gap between them, margin of the text inside the button and gap between groups of icons.
pub const BUTTON_ROWS: i64 = 1;
/// The icon row sits under the grid, outside the editor (so it is visible with the editor closed):
/// a 2 u gap above the buttons and 2 u below them.
pub const ICON_ROW_PAD_U: i64 = 2;
pub const ICON_ROW_H_U: i64 = BUTTON_ROWS * BUTTON_U + 2 * ICON_ROW_PAD_U;
pub const BUTTON_H_U: i64 = BUTTON_U;
pub const BUTTON_X_U: i64 = 4;
pub const BUTTON_HALF_W_U: i64 = 110;
pub const BUTTON_COL_GAP_U: i64 = 5;
pub const BUTTON_FULL_W_U: i64 = TEXT_WIDTH_U;
pub const BUTTON_TEXT_X_U: i64 = 6;
/// The label (14 u high) centered in the height of the button.
pub const BUTTON_TEXT_Y_U: i64 = (BUTTON_H_U - ROW_H_U) / 2;
pub const ICON_GROUP_GAP_U: i64 = 8;
/// Height of a **text** button (the rows of the menus and the buttons of the dialogs) and top
/// margin of its label, in `u`: they do not change with the icons.
pub const TEXT_BUTTON_H_U: i64 = 16;
pub const TEXT_BUTTON_TEXT_Y_U: i64 = 1;
/// Image modes: the height of the editor depends on them.
const MODES: [Mode; 4] = [Mode::Rgba, Mode::Indexed, Mode::Gray, Mode::GrayAlpha];

/// Height of the editor of the mode, in `u`: up to the end of the notice plus the bottom margin.
/// It is the **only** place where it is worked out (specs/hito-2-panel-ventana-baja.md):
/// everything else hangs from it.
pub fn editor_height_u(mode: Mode) -> i64 {
    notice_y_u(mode) + NOTICE_LINES * ROW_H_U + BLOCK_GAP_U
}

/// Height of the icon row of the mode under the grid, in `u`: 0 if the mode has none.
pub fn icons_height_u(mode: Mode) -> i64 {
    if mode == Mode::Indexed {
        ICON_ROW_H_U
    } else {
        0
    }
}

/// Height of the buttons of the mode, in `u`: 0 if it has none.
pub fn buttons_height_u(mode: Mode) -> i64 {
    if mode == Mode::Indexed {
        BUTTON_ROWS * BUTTON_H_U
    } else {
        0
    }
}

/// Buttons of the palette. `Confirm` and `Cancel` (text) replace the whole icon row while there is
/// a pending removal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaletteButton {
    Add,
    Remove,
    Confirm,
    Cancel,
    RotateLeft,
    RotateRight,
    /// Set or remove the transparent entry (the fifth icon, specs/hito-2-fijar-transparente.md).
    Transparent,
}

/// The icon of a button of the row; `None` for `Confirm` and `Cancel`, which are text.
pub fn palette_icon(button: PaletteButton) -> Option<Icon> {
    match button {
        PaletteButton::Add => Some(Icon::Add),
        PaletteButton::Remove => Some(Icon::Remove),
        PaletteButton::RotateLeft => Some(Icon::RotateLeft),
        PaletteButton::RotateRight => Some(Icon::RotateRight),
        PaletteButton::Transparent => Some(Icon::Transparent),
        PaletteButton::Confirm | PaletteButton::Cancel => None,
    }
}

/// Rectangle `(x, y, width, height)` of the button in `u` from the top-left corner of the icon row.
/// `None` in the modes without buttons.
pub fn button_rect_u(mode: Mode, button: PaletteButton) -> Option<(i64, i64, i64, i64)> {
    if buttons_height_u(mode) == 0 {
        return None;
    }
    let top = ICON_ROW_PAD_U;
    let remove_x = BUTTON_X_U + BUTTON_U + GAP_U;
    let rotate_left_x = remove_x + BUTTON_U + ICON_GROUP_GAP_U;
    let rotate_right_x = rotate_left_x + BUTTON_U + GAP_U;
    let transparent_x = rotate_right_x + BUTTON_U + ICON_GROUP_GAP_U;
    let icon = |x| (x, top, BUTTON_U, BUTTON_H_U);
    Some(match button {
        PaletteButton::Add => icon(BUTTON_X_U),
        PaletteButton::Remove => icon(remove_x),
        PaletteButton::RotateLeft => icon(rotate_left_x),
        PaletteButton::RotateRight => icon(rotate_right_x),
        PaletteButton::Transparent => icon(transparent_x),
        PaletteButton::Confirm => (BUTTON_X_U, top, BUTTON_HALF_W_U, BUTTON_H_U),
        PaletteButton::Cancel => (
            BUTTON_X_U + BUTTON_HALF_W_U + BUTTON_COL_GAP_U,
            top,
            BUTTON_HALF_W_U,
            BUTTON_H_U,
        ),
    })
}

/// Button under a point in physical pixels, relative to the top-left corner of the icon row.
/// `removing`: there is a pending removal (only `Confirm` and `Cancel` hit; the icons are not seen).
pub fn button_hit(
    mode: Mode,
    removing: bool,
    x_px: i64,
    y_px: i64,
    n: u32,
) -> Option<PaletteButton> {
    let n = scale(n);
    let visible: &[PaletteButton] = if removing {
        &[PaletteButton::Confirm, PaletteButton::Cancel]
    } else {
        &[
            PaletteButton::Add,
            PaletteButton::Remove,
            PaletteButton::RotateLeft,
            PaletteButton::RotateRight,
            PaletteButton::Transparent,
        ]
    };
    visible.iter().copied().find(|button| {
        button_rect_u(mode, *button).is_some_and(|(x, y, w, h)| {
            x_px >= x * n && x_px < (x + w) * n && y_px >= y * n && y_px < (y + h) * n
        })
    })
}

/// The largest editor height among the modes.
pub fn editor_height_u_max() -> i64 {
    MODES
        .iter()
        .map(|mode| editor_height_u(*mode))
        .max()
        .unwrap_or(0)
}

/// The tallest icon row plus editor among the modes, in `u`: what the minimum panel holds below the
/// first grid row with the editor open.
fn open_tail_u_max() -> i64 {
    MODES
        .iter()
        .map(|mode| icons_height_u(*mode) + editor_height_u(*mode))
        .max()
        .unwrap_or(0)
}

fn scale(n: u32) -> i64 {
    i64::from(n.max(1))
}

/// Height of the block `[band · one grid row · icons · editor]` of the mode with the editor open,
/// in physical pixels: the height of the panel from which there is no scrolling.
pub fn block_height_px(mode: Mode, n: u32) -> i64 {
    (GRID_Y_U + PITCH_U + icons_height_u(mode) + editor_height_u(mode)) * scale(n)
}

/// Like [`block_height_px`] with the editor closed: `[band · one grid row · icons]`.
pub fn block_height_closed_px(mode: Mode, n: u32) -> i64 {
    (GRID_Y_U + PITCH_U + icons_height_u(mode)) * scale(n)
}

/// Minimum height of the panel, in physical pixels, for any mode (with the editor open, which is
/// the largest: closed it needs less, and the floor is set by the layers panel, 277 u, which this
/// number equals).
pub fn min_panel_height_px(n: u32) -> i64 {
    (GRID_Y_U + PITCH_U + open_tail_u_max()) * scale(n)
}

/// Minimum height of the window in logical pixels (egui points): the menu, the status bar and
/// the minimum panel.
pub fn min_window_height_pt(ppp: f32) -> f32 {
    let ppp = if ppp.is_finite() && ppp > 0.0 {
        ppp
    } else {
        1.0
    };
    let n = text_scale(ppp);
    let physical = i64::from(bar_height_px(n)) + MENU_H_U * scale(n) + min_panel_height_px(n);
    physical as f32 / ppp
}

/// Minimum width of the window in interface pixels: the toolbox column, the panel and the
/// minimum canvas.
pub fn min_window_width_u() -> i64 {
    toolbar_width_u() + PANEL_WIDTH_U + MIN_CANVAS_WIDTH_U
}

/// Minimum width of the window in logical pixels (egui points).
pub fn min_window_width_pt(ppp: f32) -> f32 {
    let ppp = if ppp.is_finite() && ppp > 0.0 {
        ppp
    } else {
        1.0
    };
    let n = text_scale(ppp);
    (min_window_width_u() * scale(n)) as f32 / ppp
}

/// Vertical split of the panel: where the icon row and the editor go and how many rows the grid sees.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PanelLayout {
    /// The panel is lower than its block and scrolls as a whole.
    pub scrolling: bool,
    /// Applied scroll: 0 if none, else a multiple of `n` in `0..=max_offset_px`.
    pub offset_px: i64,
    pub max_offset_px: i64,
    /// Top edge of the editor from the panel's, in physical pixels (may be negative). With the
    /// editor closed it is the bottom edge of the icon row.
    pub top_px: i64,
    /// Lowest position of the top edge of the icon row from the panel's, in physical pixels: right
    /// above the editor (or the bottom edge) when the grid fills the space (equal to `top_px` in
    /// the modes without icons). With a shorter grid the row hangs higher: see
    /// [`PanelLayout::icon_row_top_px`].
    pub icons_top_px: i64,
    pub visible_rows: usize,
}

impl PanelLayout {
    /// Top edge of the icon row for a grid of `grid_rows` rows, in physical pixels: the smaller of
    /// the bottom edge of the grid and the bottom edge of the space (the panel, or the top of the
    /// editor when it is open) minus the height of the row. A short grid leaves the row right under
    /// it; a grid taller than the space leaves it anchored to the bottom of the space. When the
    /// panel scrolls as a whole the row is where the block puts it.
    pub fn icon_row_top_px(&self, grid_rows: usize, n: u32) -> i64 {
        if self.scrolling {
            return self.icons_top_px;
        }
        let grid_bottom = i64::try_from(grid_rows)
            .unwrap_or(i64::MAX)
            .saturating_mul(PITCH_U)
            .saturating_add(GRID_Y_U)
            .saturating_mul(scale(n));
        grid_bottom.min(self.icons_top_px)
    }
}

/// Split of a panel of `panel_h_px` with the editor of the mode open. With enough height, the editor
/// is anchored to the bottom, the icon row goes right above it and the grid takes the rest; with
/// less, the block scrolls `offset_px` (clamped to what fits) and the grid shows one row. With
/// `offset_px = 0` and `height = block` both splits coincide.
pub fn panel_layout(mode: Mode, panel_h_px: i64, n: u32, offset_px: i64) -> PanelLayout {
    layout(mode, panel_h_px, n, offset_px, true)
}

/// Like [`panel_layout`] with the editor closed: the icon row is anchored to the bottom and the grid
/// takes the rest.
pub fn panel_layout_closed(mode: Mode, panel_h_px: i64, n: u32, offset_px: i64) -> PanelLayout {
    layout(mode, panel_h_px, n, offset_px, false)
}

fn layout(mode: Mode, panel_h_px: i64, n: u32, offset_px: i64, editor_open: bool) -> PanelLayout {
    let n = scale(n);
    let editor = if editor_open {
        editor_height_u(mode) * n
    } else {
        0
    };
    let icons = icons_height_u(mode) * n;
    let block = if editor_open {
        block_height_px(mode, n as u32)
    } else {
        block_height_closed_px(mode, n as u32)
    };
    if panel_h_px >= block {
        let top = panel_h_px.saturating_sub(editor + icons).div_euclid(n) * n;
        // The editor (or the bottom edge) closes the panel; the icon row sits right above it.
        let editor_top = top + icons;
        return PanelLayout {
            scrolling: false,
            offset_px: 0,
            max_offset_px: 0,
            top_px: editor_top,
            icons_top_px: top,
            visible_rows: visible_rows_reserved(panel_h_px, panel_h_px - top, n as u32),
        };
    }
    let max = block.saturating_sub(panel_h_px).div_euclid(n) * n;
    let offset = offset_px.clamp(0, max).div_euclid(n) * n;
    let icons_top = (GRID_Y_U + PITCH_U) * n - offset;
    PanelLayout {
        scrolling: true,
        offset_px: offset,
        max_offset_px: max,
        top_px: icons_top + icons,
        icons_top_px: icons_top,
        visible_rows: 1,
    }
}
pub fn control_row_u(mode: Mode, control: Control) -> Option<i64> {
    let idx = controls(mode).iter().position(|c| *c == control)?;
    let mut y = FIRST_ROW_Y_U;
    for (i, c) in controls(mode).iter().enumerate().take(idx + 1) {
        if i > 0 {
            let prev = controls(mode)[i - 1];
            let gap = matches!(
                (prev, *c),
                (Control::Blue, Control::Hue)
                    | (Control::Lightness, Control::Alpha)
                    | (Control::Gray, Control::Alpha)
            );
            y += ROW_H_U + if gap { GROUP_GAP_U } else { 0 };
        }
    }
    Some(y)
}
pub fn slider_hit(mode: Mode, x_px: i64, y_px: i64, n: u32) -> Option<Control> {
    let n = i64::from(n.max(1));
    if x_px < HIT_X0_U * n || x_px >= HIT_X1_U * n {
        return None;
    }
    controls(mode).iter().copied().find(|c| {
        control_row_u(mode, *c).is_some_and(|r| y_px >= r * n && y_px < (r + ROW_H_U) * n)
    })
}
pub fn field_y_u(mode: Mode) -> i64 {
    let last_row = controls(mode)
        .last()
        .and_then(|last| control_row_u(mode, *last))
        .unwrap_or(FIRST_ROW_Y_U);
    last_row + ROW_H_U + BLOCK_GAP_U
}
pub fn notice_y_u(mode: Mode) -> i64 {
    field_y_u(mode) + FIELD_H_U + BLOCK_GAP_U
}
pub fn edges_u(mode: Mode) -> Vec<i64> {
    let mut e = vec![
        0,
        1,
        HEADER_X_U,
        HEADER_Y_U,
        HEADER_Y_U + ROW_H_U,
        TRACK_X_U,
        TRACK_X_U + TRACK_W_U,
        VALUE_X_U,
        VALUE_X_U + VALUE_W_U,
        FIELD_X_U,
        FIELD_X_U + FIELD_W_U,
        field_y_u(mode),
        field_y_u(mode) + FIELD_H_U,
        notice_y_u(mode),
        notice_y_u(mode) + ROW_H_U,
        notice_y_u(mode) + 2 * ROW_H_U,
        editor_height_u(mode),
    ];
    for row in controls(mode)
        .iter()
        .filter_map(|c| control_row_u(mode, *c))
    {
        e.extend([row, row + ROW_H_U, row + 5, row + 9, row + 1, row + 13]);
    }
    for button in [
        PaletteButton::Add,
        PaletteButton::Remove,
        PaletteButton::Confirm,
        PaletteButton::Cancel,
        PaletteButton::RotateLeft,
        PaletteButton::RotateRight,
        PaletteButton::Transparent,
    ] {
        if let Some((x, y, w, h)) = button_rect_u(mode, button) {
            // Face, relief (1 u), icon 4 u from the edge (5 u when sunken) and label of the text ones.
            e.extend([
                x,
                x + 1,
                x + 4,
                x + 5,
                x + w - 5,
                x + w - 4,
                x + w - 1,
                x + w,
                x + BUTTON_TEXT_X_U,
                y,
                y + 1,
                y + 4,
                y + 5,
                y + h - 5,
                y + h - 4,
                y + h - 1,
                y + h,
                y + BUTTON_TEXT_Y_U,
                y + BUTTON_TEXT_Y_U + ROW_H_U,
            ]);
        }
    }
    e
}
