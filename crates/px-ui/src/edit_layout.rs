//! Geometry of the color editor, without egui types (specs/hito-2-editar-color.md), of the tail under
//! the grid (color bar, hexadecimal field and row of palette icons; specs/ui-palette-handle.md) and
//! of the minimums of the window.
//!
//! The measures are constants in "interface pixels" `u` (`n = text_scale(ppp)` physical pixels
//! each). The editor (separator and sliders) is measured from its top-left corner, which starts at
//! `top_px` (anchored to the bottom of the panel); the bar, the field and the icons, from the top
//! edge of the tail (`tail_top_px`), which hangs under the grid. The functions work in physical
//! pixels with `n`.

use px_core::Mode;

use crate::chrome::{COLUMN_MIN_COLS, MENU_BAR_W_U, MENU_H_U, column_width_u, head_u};
use crate::color_block::{
    ALPHA_BAR_X_U, BARS_H_U, HUE_BAR_X_U, SELECTOR_H_U, SELECTOR_W_U, SELECTOR_X_U, SELECTOR_Y_U,
    SQUARE_SIDE_U, SQUARE_X_U, SQUARE_Y_U, VBAR_W_U,
};
use crate::color_edit::{Control, SliderMode, controls, visible_controls};
use crate::dialog_layout::{DIALOG_W_U, MARGIN_U};
use crate::icon_button::{BUTTON_U, GAP_U};
use crate::icons::Icon;
use crate::layers_panel::PANEL_H_U as LAYERS_PANEL_H_U;
use crate::palette_grid::{GRID_Y_U, PITCH_U, TEXT_WIDTH_U, visible_rows_reserved};
use crate::status::bar_height_px;
use crate::text::text_scale;

/// 1 u strip at the top of the editor.
pub const SEPARATOR_Y_U: i64 = 0;
/// Height of a row of text and of a slider.
pub const ROW_H_U: i64 = 14;
/// First row of sliders of the gray modes: the editor has no header, only the separator above them.
pub const FIRST_ROW_Y_U: i64 = 4;
/// First row of sliders of indexed and RGBA images: under the square (or the bars) and the selector.
pub const COLOR_FIRST_ROW_Y_U: i64 = SELECTOR_Y_U + SELECTOR_H_U + 4;
/// Gap between the three sliders of a model and the alpha one.
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
/// Hexadecimal field, from the top edge of the tail: 2 u under the two bars (40 u, specs/ui-color-block.md).
pub const FIELD_X_U: i64 = 4;
pub const FIELD_Y_U: i64 = BARS_H_U + 2;
pub const FIELD_W_U: i64 = 69;
pub const FIELD_H_U: i64 = 18;
pub const FIELD_PAD_U: i64 = 3;
/// Bottom margin of the editor.
pub const BLOCK_GAP_U: i64 = 4;
/// Minimum width of the canvas in the window, in interface pixels
/// (specs/hito-2-panel-ventana-baja.md).
pub const MIN_CANVAS_WIDTH_U: i64 = 128;
/// Buttons of the palette (indexed only; specs/interfaz-iconos.md, specs/ui-composition.md): a row
/// of four 24 u icons (1 u gap inside a group and 8 u between groups: remove | rotate to one side
/// and the other | transparent) or, with a pending removal or resize, `Confirmar` and `Cancel` as
/// text in their place (two 110 u buttons and a 5 u gap). Rows, height, origin, width of the two
/// text buttons and gap between them, margin of the text inside the button and gap between groups
/// of icons.
pub const BUTTON_ROWS: i64 = 1;
/// The icon row sits under the field, outside the editor (so it is visible with the editor closed):
/// a 2 u gap above the buttons (after the field) and 2 u below them.
pub const ICON_ROW_PAD_U: i64 = 2;
pub const ICON_ROW_Y_U: i64 = FIELD_Y_U + FIELD_H_U + ICON_ROW_PAD_U;
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

/// Height of the editor of the mode, in `u`: the separator, the sliders and the bottom margin. It
/// is the **only** place where it is worked out (specs/hito-2-panel-ventana-baja.md): everything
/// else hangs from it.
pub fn editor_height_u(mode: Mode) -> i64 {
    let last_row = controls(mode)
        .last()
        .and_then(|last| control_row_u(mode, *last))
        .unwrap_or(FIRST_ROW_Y_U);
    last_row + ROW_H_U + BLOCK_GAP_U
}

/// Height of the tail under the grid, in `u`: the bar and the field in every mode and, in indexed
/// images, the icon row (64 u; 38 u in the others).
pub fn tail_height_u(mode: Mode) -> i64 {
    if mode == Mode::Indexed {
        ICON_ROW_Y_U + BUTTON_ROWS * BUTTON_U + ICON_ROW_PAD_U
    } else {
        FIELD_Y_U + FIELD_H_U + ICON_ROW_PAD_U
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
/// a pending removal or resize. `Add` names the add operation (for `blocked_reason` and the menu
/// action `Imagen → Añadir entrada`): since C3 it is not in the row.
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

/// Rectangle `(x, y, width, height)` of the button in `u` from the top-left corner of the tail
/// (the icons are at y = 38). `None` in the modes without buttons and for `Add`, which is not in
/// the row.
pub fn button_rect_u(mode: Mode, button: PaletteButton) -> Option<(i64, i64, i64, i64)> {
    if buttons_height_u(mode) == 0 {
        return None;
    }
    let top = ICON_ROW_Y_U;
    let remove_x = BUTTON_X_U;
    let rotate_left_x = remove_x + BUTTON_U + ICON_GROUP_GAP_U;
    let rotate_right_x = rotate_left_x + BUTTON_U + GAP_U;
    let transparent_x = rotate_right_x + BUTTON_U + ICON_GROUP_GAP_U;
    let icon = |x| (x, top, BUTTON_U, BUTTON_H_U);
    Some(match button {
        PaletteButton::Add => return None,
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

/// Button under a point in physical pixels, relative to the top-left corner of the tail.
/// `removing`: there is a pending removal or resize (only `Confirm` and `Cancel` hit; the icons are
/// not seen).
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

/// Tools the toolbox must hold at the minimum column in the minimum window (specs/ui-single-column.md,
/// capacity rule): 20 tools take four rows at 10 columns.
pub const TOOLBOX_CAPACITY: usize = 20;

fn scale(n: u32) -> i64 {
    i64::from(n.max(1))
}

/// Height of the block `[band · one grid row · tail · editor]` of the mode with the editor open,
/// in physical pixels: the height of the panel from which there is no scrolling.
pub fn block_height_px(mode: Mode, n: u32) -> i64 {
    (GRID_Y_U + PITCH_U + tail_height_u(mode) + editor_height_u(mode)) * scale(n)
}

/// Like [`block_height_px`] with the editor closed: `[band · one grid row · tail]`.
pub fn block_height_closed_px(mode: Mode, n: u32) -> i64 {
    (GRID_Y_U + PITCH_U + tail_height_u(mode)) * scale(n)
}

/// Minimum height of the panel (the column under the menu), in physical pixels: the larger of the
/// layers panel and the capacity of the toolbox (`TOOLBOX_CAPACITY` tools at the minimum column
/// plus the closed block of the tallest palette). The open block of every mode is lower.
pub fn min_panel_height_px(n: u32) -> i64 {
    let closed = MODES
        .iter()
        .map(|mode| GRID_Y_U + PITCH_U + tail_height_u(*mode))
        .max()
        .unwrap_or(0);
    let toolbox = head_u(TOOLBOX_CAPACITY, COLUMN_MIN_COLS) + closed;
    toolbox.max(LAYERS_PANEL_H_U) * scale(n)
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

/// Columns of the status bar that the minimum window keeps (66, each 7 u wide) and the padding of
/// the bar at each side (3 u).
const STATUS_BAR_COLUMNS: i64 = 66;
const STATUS_ADVANCE_U: i64 = 7;
const STATUS_PADDING_U: i64 = 3;

/// The four terms of the minimum width of the window, in interface pixels: the narrowest column
/// plus the minimum canvas, the menu bar and its margin, the dialog with its two margins and the 66
/// columns of the status bar. The window is as wide as the widest of them.
pub fn min_width_terms_u() -> [i64; 4] {
    [
        column_width_u(COLUMN_MIN_COLS) + MIN_CANVAS_WIDTH_U,
        MENU_BAR_W_U + 4,
        DIALOG_W_U + 2 * MARGIN_U,
        STATUS_BAR_COLUMNS * STATUS_ADVANCE_U + 2 * STATUS_PADDING_U,
    ]
}

/// Minimum width of the window in interface pixels: the widest of [`min_width_terms_u`] (468).
pub fn min_window_width_u() -> i64 {
    min_width_terms_u().into_iter().max().unwrap_or(0)
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
    /// [`PanelLayout::tail_top_for`].
    pub tail_top_px: i64,
    pub visible_rows: usize,
}

impl PanelLayout {
    /// Top edge of the icon row for a grid of `grid_rows` rows, in physical pixels: the smaller of
    /// the bottom edge of the grid and the bottom edge of the space (the panel, or the top of the
    /// editor when it is open) minus the height of the row. A short grid leaves the row right under
    /// it; a grid taller than the space leaves it anchored to the bottom of the space. When the
    /// panel scrolls as a whole the row is where the block puts it.
    pub fn tail_top_for(&self, grid_rows: usize, n: u32) -> i64 {
        self.tail_top_for_at(grid_rows, n, 0)
    }

    /// Like [`PanelLayout::tail_top_for`] for a palette that starts `head_px` physical pixels
    /// below the top of the column (the toolbox is above it).
    pub fn tail_top_for_at(&self, grid_rows: usize, n: u32, head_px: i64) -> i64 {
        if self.scrolling {
            return self.tail_top_px;
        }
        let grid_bottom = i64::try_from(grid_rows)
            .unwrap_or(i64::MAX)
            .saturating_mul(PITCH_U)
            .saturating_add(GRID_Y_U)
            .saturating_mul(scale(n))
            .saturating_add(head_px);
        grid_bottom.min(self.tail_top_px)
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
    column_layout(mode, panel_h_px, n, offset_px, 0, editor_open)
}

/// Split of the column of `column_h_px` with the toolbox of `head_px` on top and the palette (band ·
/// grid · icon row · editor, the editor open or not) under it. The block is the toolbox and the
/// palette: with enough height the palette is anchored to the bottom as in [`panel_layout`] and the
/// grid takes the rest; with less, the whole block, the toolbox included, scrolls `offset_px`
/// (clamped to what fits) and the grid shows one row. All the positions are in physical pixels from
/// the top of the **column**. With `head_px = 0` it is [`panel_layout`] (or [`panel_layout_closed`]).
pub fn column_layout(
    mode: Mode,
    column_h_px: i64,
    n: u32,
    offset_px: i64,
    head_px: i64,
    editor_open: bool,
) -> PanelLayout {
    let n = scale(n);
    let head = head_px.max(0);
    let editor = if editor_open {
        editor_height_u(mode) * n
    } else {
        0
    };
    let icons = tail_height_u(mode) * n;
    let block = head
        + if editor_open {
            block_height_px(mode, n as u32)
        } else {
            block_height_closed_px(mode, n as u32)
        };
    if column_h_px >= block {
        let top = column_h_px.saturating_sub(editor + icons).div_euclid(n) * n;
        // The editor (or the bottom edge) closes the column; the icon row sits right above it.
        let editor_top = top + icons;
        return PanelLayout {
            scrolling: false,
            offset_px: 0,
            max_offset_px: 0,
            top_px: editor_top,
            tail_top_px: top,
            visible_rows: visible_rows_reserved(column_h_px - head, column_h_px - top, n as u32),
        };
    }
    let max = block.saturating_sub(column_h_px).div_euclid(n) * n;
    let offset = offset_px.clamp(0, max).div_euclid(n) * n;
    let tail_top = head + (GRID_Y_U + PITCH_U) * n - offset;
    PanelLayout {
        scrolling: true,
        offset_px: offset,
        max_offset_px: max,
        top_px: tail_top + icons,
        tail_top_px: tail_top,
        visible_rows: 1,
    }
}

/// Row (`y`, in `u` from the corner of the editor) of the slider of `control`, if the mode has it.
/// Indexed and RGBA images show three sliders of the chosen model and then the alpha one, whatever
/// the model: the three controls of a position (`R`, `H` and `H` of HSV; `G`, `S`, `S`; `B`, `L`,
/// `V`) share its row. The gray modes keep their rows from 4 u.
pub fn control_row_u(mode: Mode, control: Control) -> Option<i64> {
    if matches!(mode, Mode::Rgba | Mode::Indexed) {
        let position = match control {
            Control::Red | Control::Hue | Control::HueV => 0,
            Control::Green | Control::Saturation | Control::SaturationV => 1,
            Control::Blue | Control::Lightness | Control::Value => 2,
            Control::Alpha => 3,
            Control::Gray => return None,
        };
        let gap = if position == 3 { GROUP_GAP_U } else { 0 };
        return Some(COLOR_FIRST_ROW_Y_U + position * ROW_H_U + gap);
    }
    let idx = controls(mode).iter().position(|c| *c == control)?;
    let mut y = FIRST_ROW_Y_U;
    for (i, c) in controls(mode).iter().enumerate().take(idx + 1) {
        if i > 0 {
            let prev = controls(mode)[i - 1];
            let gap = matches!((prev, *c), (Control::Gray, Control::Alpha));
            y += ROW_H_U + if gap { GROUP_GAP_U } else { 0 };
        }
    }
    Some(y)
}

/// Slider under a point in physical pixels from the corner of the editor, among the ones the
/// selector shows (`slider_mode`).
pub fn slider_hit(
    mode: Mode,
    slider_mode: SliderMode,
    x_px: i64,
    y_px: i64,
    n: u32,
) -> Option<Control> {
    let n = i64::from(n.max(1));
    if x_px < HIT_X0_U * n || x_px >= HIT_X1_U * n {
        return None;
    }
    visible_controls(mode, slider_mode)
        .iter()
        .copied()
        .find(|c| {
            control_row_u(mode, *c).is_some_and(|r| y_px >= r * n && y_px < (r + ROW_H_U) * n)
        })
}

pub fn edges_u(mode: Mode) -> Vec<i64> {
    let mut e = vec![
        0,
        1,
        FIELD_X_U,
        FIELD_X_U + FIELD_W_U,
        FIELD_Y_U,
        FIELD_Y_U + FIELD_H_U,
        TRACK_X_U,
        TRACK_X_U + TRACK_W_U,
        VALUE_X_U,
        VALUE_X_U + VALUE_W_U,
        tail_height_u(mode),
        editor_height_u(mode),
    ];
    if matches!(mode, Mode::Rgba | Mode::Indexed) {
        for x in [SQUARE_X_U, HUE_BAR_X_U, ALPHA_BAR_X_U] {
            e.push(x);
        }
        e.extend([
            SQUARE_X_U + SQUARE_SIDE_U,
            HUE_BAR_X_U + VBAR_W_U,
            ALPHA_BAR_X_U + VBAR_W_U,
            SQUARE_Y_U,
            SQUARE_Y_U + SQUARE_SIDE_U,
            SELECTOR_Y_U,
            SELECTOR_Y_U + SELECTOR_H_U,
        ]);
        for x in SELECTOR_X_U {
            e.extend([x, x + SELECTOR_W_U]);
        }
    }
    for control in controls(mode) {
        if let Some(row) = control_row_u(mode, *control) {
            e.extend([row, row + ROW_H_U, row + 5, row + 9, row + 1, row + 13]);
        }
    }
    for button in [
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
