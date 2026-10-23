//! The single column on the right edge of the window (specs/ui-single-column.md): the toolbox on top
//! and under it the palette (color indicator, grid, icon row and, on demand, the color editor) in one
//! `egui::Panel`. Its width is chosen by dragging its left edge, in steps of one column of swatches
//! (14 u); the width is cut in [`crate::chrome::column_cols`] and what was chosen is kept.
//!
//! The toolbox and the palette are the two parts of one block: when the window is lower than the
//! block (the editor open on a low window) the **whole block** scrolls with the wheel and the toolbox
//! goes up with it. Opening the editor takes the scroll to the maximum; closing it brings back the
//! value before. The editor, the icon row and the buttons of a removal are wider than a narrow
//! column, so while the editor is open or a removal is pending the column has at least 16 columns.

use egui::{CursorIcon, Event, Frame, Key, Panel, Pos2, Rect, Sense, Ui, vec2};
use px_core::Image;

use crate::actions::Platform;
use crate::chrome::{
    COLUMN_GRIP_U, COLUMN_MIN_COLS, TOOL_LABELS, column_cols, column_width_u, head_u, max_cols,
    wide_cols,
};
use crate::color::SlotColors;
use crate::color_edit::{ColorEditor, EditTarget};
use crate::keymap::Keymap;
use crate::palette_grid::{PanelView, Scroll};
use crate::palette_ops::PaletteOps;
use crate::palette_panel::{PanelOutput, PanelRoles, Placement, Roles, panel_body};
use crate::text::{text_scale, ui_unit};
use crate::theme::{SURFACE_1, SURFACE_3};
use crate::toolbar::{ToolboxOutput, ToolboxPlace, ToolboxView, paint_toolbox};

/// What the column needs to be painted.
pub struct ColumnView<'a> {
    pub toolbox: ToolboxView<'a>,
    pub image: &'a Image,
    pub roles: Roles,
    pub slots: &'a SlotColors,
    /// `false` with a gesture in course, a dialog or a modal open: the palette is seen but does not
    /// choose and the edge does not drag.
    pub enabled: bool,
    /// Columns of swatches chosen by the user (`App.column_cols`): the column is cut to what the
    /// window allows, but this is not touched when the window narrows.
    pub cols: i64,
}

/// What happened in the column in a frame.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ColumnOutput {
    pub toolbox: ToolboxOutput,
    pub panel: PanelOutput,
    /// The new chosen width after a drag of the left edge, if it changed in this frame.
    pub resized: Option<i64>,
    /// The columns of swatches the column was painted with in this frame.
    pub cols: i64,
}

/// A drag of the left edge in course, kept in the egui memory between frames.
#[derive(Clone, Copy, Debug)]
struct EdgeDrag {
    /// `x` of the pointer when it was pressed, in points.
    press_x: f32,
    /// The chosen width when it began.
    wanted0: i64,
    /// The columns shown when it began: the `k₀` of `k = k₀ + round(Δx / 14n)`.
    shown0: i64,
    /// `Esc` was pressed: the width went back and the drag does nothing until it is released.
    cancelled: bool,
}

fn drag_id() -> egui::Id {
    egui::Id::unique("pixoko-column-edge-drag")
}

/// Paints the column in the right edge of `ui` and returns what happened in it. It has to be called
/// **after** the menu and the status bar and **before** the canvas, which takes what is left.
#[allow(clippy::too_many_arguments)]
pub fn show_column(
    ui: &mut Ui,
    view: &ColumnView,
    keymap: &Keymap,
    platform: Platform,
    scroll: &mut Scroll,
    panel_view: &mut PanelView,
    editor: &mut ColorEditor,
    ops: &mut PaletteOps,
) -> ColumnOutput {
    let ctx = ui.ctx().clone();
    let ppp = ctx.pixels_per_point();
    let u = ui_unit(ppp);
    let n = text_scale(ppp);
    let window_w_px = (ctx.content_rect().width() * ppp).round() as i64;
    let roles = PanelRoles::of(view.roles);
    // The editor is open when the app opened it and there is something to edit.
    let editor_shown =
        panel_view.editor_open && EditTarget::of(view.image, roles.edited()).is_some();
    let wide = editor_shown || ops.removal().is_some();
    let cols = column_cols(window_w_px, n, wide_cols(view.cols, wide));
    let k_max = max_cols(window_w_px, n);
    let tools = TOOL_LABELS.len();
    let head_px = head_u(tools, cols) * i64::from(n);
    let mut output = ColumnOutput {
        cols,
        ..ColumnOutput::default()
    };
    Panel::right("columna")
        .exact_size(column_width_u(cols) as f32 * u)
        .resizable(false)
        .show_separator_line(false)
        .frame(Frame::new().fill(SURFACE_1))
        .show(ui, |ui| {
            let (rect, _) = ui.allocate_exact_size(ui.available_size(), Sense::hover());
            // The palette first: it settles the scroll of the block, which the toolbox follows.
            output.panel = panel_body(
                ui,
                &Placement {
                    rect,
                    head_px,
                    cols,
                },
                view.image,
                roles,
                scroll,
                panel_view,
                view.enabled,
                view.slots,
                Some(editor),
                Some(ops),
            );
            let offset_pt = panel_view.offset_px as f32 / ppp;
            let visible = Rect::from_min_max(
                rect.min,
                Pos2::new(
                    rect.max.x,
                    (rect.min.y + head_px as f32 / ppp - offset_pt).min(rect.max.y),
                ),
            );
            if visible.is_positive() {
                output.toolbox = paint_toolbox(
                    ui,
                    &view.toolbox,
                    keymap,
                    platform,
                    &ToolboxPlace {
                        origin: rect.min - vec2(0.0, offset_pt),
                        clip: visible,
                        cols,
                    },
                );
                // The 1 u lines at the two sides, over the toolbox part (the palette paints its own).
                let painter = ui.painter().with_clip_rect(visible);
                for x in [0.0, (column_width_u(cols) - 1) as f32 * u] {
                    painter.rect_filled(
                        Rect::from_min_size(
                            Pos2::new(rect.min.x + x, visible.min.y),
                            vec2(u, visible.height()),
                        ),
                        0.0,
                        SURFACE_3,
                    );
                }
            }
            edge_drag(ui, rect, view, &mut output, cols, k_max);
        });
    output
}

/// The left edge of the column: a zone of 4 u inside it that drags, in steps of 14 u, the width.
/// The resize cursor is only there when the drag can have an effect (enabled and another width
/// to go to).
fn edge_drag(
    ui: &mut Ui,
    rect: Rect,
    view: &ColumnView,
    output: &mut ColumnOutput,
    cols: i64,
    k_max: i64,
) {
    let ctx = ui.ctx().clone();
    let ppp = ctx.pixels_per_point();
    let u = ui_unit(ppp);
    let n = i64::from(text_scale(ppp));
    let state: Option<EdgeDrag> = ctx.data(|d| d.get_temp(drag_id()));
    let may = view.enabled && k_max > COLUMN_MIN_COLS;
    if !may {
        // Disabled (or nothing to go to): no cursor and no drag; one in course ends.
        if state.is_some() {
            ctx.data_mut(|d| d.remove::<EdgeDrag>(drag_id()));
        }
        return;
    }
    let grip = Rect::from_min_size(rect.min, vec2(COLUMN_GRIP_U as f32 * u, rect.height()));
    let response = ui.interact(grip, ui.unique_id().with("borde-columna"), Sense::drag());
    // Egui also hovers a widget that is near the pointer (its interaction radius): the zone is
    // exactly the 4 u, so the pointer has to be inside it.
    let on_zone = response.hovered()
        && ctx
            .input(|i| i.pointer.hover_pos())
            .is_some_and(|p| grip.contains(p));
    let events = ctx.input(|i| i.events.clone());
    let down = ctx.input(|i| i.pointer.primary_down());
    let mut drag = state;
    // A press on the zone begins the drag.
    for event in &events {
        if let Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: true,
            ..
        } = event
            && drag.is_none()
            && grip.contains(*pos)
            && on_zone
        {
            drag = Some(EdgeDrag {
                press_x: pos.x,
                wanted0: view.cols,
                shown0: cols,
                cancelled: false,
            });
        }
    }
    if let Some(mut d) = drag {
        let escape = events.iter().any(|e| {
            matches!(
                e,
                Event::Key {
                    key: Key::Escape,
                    pressed: true,
                    ..
                }
            )
        });
        let released = !down
            || events.iter().any(|e| {
                matches!(
                    e,
                    Event::PointerButton {
                        button: egui::PointerButton::Primary,
                        pressed: false,
                        ..
                    }
                )
            });
        if escape && !d.cancelled {
            d.cancelled = true;
            if d.wanted0 != view.cols {
                output.resized = Some(d.wanted0);
            }
            // The `Esc` was the drag's: it does not close anything else in this frame.
            output.panel.escape_free = false;
            output.panel.close_editor = false;
        }
        if !d.cancelled
            && !released
            && let Some(now) = ctx.input(|i| i.pointer.latest_pos())
        {
            let travelled = ((d.press_x - now.x) * ppp).round() as i64;
            let step = 14 * n;
            let steps = (travelled + step / 2).div_euclid(step);
            let wanted = (d.shown0 + steps).clamp(COLUMN_MIN_COLS, k_max.max(COLUMN_MIN_COLS));
            if wanted != view.cols {
                output.resized = Some(wanted);
            }
        }
        if released {
            ctx.data_mut(|d| d.remove::<EdgeDrag>(drag_id()));
        } else {
            ctx.data_mut(|data| data.insert_temp(drag_id(), d));
            ctx.set_cursor_icon(CursorIcon::ResizeHorizontal);
        }
    } else if on_zone {
        ctx.set_cursor_icon(CursorIcon::ResizeHorizontal);
    }
}
