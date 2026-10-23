//! Palette panel: grid of swatches to see and choose the active color
//! (specs/hito-2-panel-paletas.md). It does not modify the document.
//!
//! The geometry and the constants live in [`crate::palette_grid`]; here they are only translated to
//! egui points (`u` = `ui_unit(ppp)` points = `n` physical pixels) and the events are read.

use egui::{
    Color32, CursorIcon, Event, FontFamily, FontId, Frame, Label, MouseWheelUnit, Panel,
    PointerButton, Pos2, Rect, RichText, Sense, Ui, vec2,
};
use px_core::{Image, Mode, Rgba};

use crate::color::{
    ColorChoice, Role, RoleFlags, SlotColors, cell_color_with, cell_count, cell_label_with,
    cell_of_choice, choice_for_cell, info_flags_alpha0, info_flags_roles, is_alpha0_cell_with,
    is_transparent_cell_with,
};
use crate::color_edit::{
    ColorEditor, Commit, EditTarget, HexOutcome, Resolved, hex_notice, stale_notice,
};
use crate::color_editor::{self, ButtonsView, Geometry, hex_field_id, is_escape, show_icon_row};
use crate::edit_layout::{
    FIELD_H_U, FIELD_Y_U, PaletteButton, PanelLayout, block_height_closed_px, block_height_px,
    button_hit, column_layout,
};
use crate::help::HelpTarget;
use crate::palette_grid::{
    COLUMNS, GRID_X_U, GRID_Y_U, Grid, IND_BORDER_EDIT_U, IND_BORDER_U, INDICATOR_H_U,
    PANEL_WIDTH_U, PITCH_U, PanelView, RING_ACTIVE_U, RING_BACKGROUND_ARM_U, RING_BACKGROUND_U,
    RING_DEST_U, RING_HOVER_U, RING_RANGE_U, SEPARATOR_Y_U, Scroll, TRACK_W_U, block_parts,
    corner_rects, drop_mark_rect, indicator_hit, indicator_rects, panel_thumb, panel_width_u,
    right_line_x_u, ring_rects, swatch_parts, text_width_u, thumb, total_rows_cols, track_x_u,
    visible_rows_reserved,
};
use crate::palette_ops::{
    DragEnd, PaletteOp, PaletteOps, blocked_reason, default_destination, drag_labels,
    removal_prompt, removal_warning, rotation_range, transparent_toggle, uses_entry,
};
use crate::palette_resize::{
    HANDLE_HINT, HandleEnd, PendingResize, dimmed, drag_status, drag_zone_cells, handle_bars,
    handle_index, resize_prompt,
};
use crate::status::columns;
use crate::text::{font_size_points, printable, text_scale, ui_unit};
use crate::theme::{
    ACCENT, DROP_MARK, RING_ACTIVE, RING_BACKGROUND, RING_DEST, RING_HOVER, RING_RANGE, SURFACE_1,
    SURFACE_2, SURFACE_3, TEXT, TEXT_DIM, TEXT_DISABLED,
};
use crate::view::ScreenRect;

/// Temporary mark of "take the focus to the hexadecimal field" (right click on the indicator).
fn hex_focus_pending() -> egui::Id {
    egui::Id::unique("pixoko-hex-focus-pending")
}

/// Rows scrolled by one wheel "line".
const WHEEL_LINE_ROWS: f32 = 3.0;

/// Data of a frame: scale and origin of the grid in the panel.
struct Layout {
    ppp: f32,
    n: u32,
    /// The panel, in points.
    rect: Rect,
    /// Top-left corner of the block of the panel, in points: the panel's, under the toolbox (`head`)
    /// and minus the scroll of the whole block when it does not fit.
    min: Pos2,
    /// Columns of swatches of the grid.
    cols: i64,
}

impl Layout {
    /// An egui point over the panel, in physical pixels from the origin of the grid.
    fn grid_px(&self, p: Pos2) -> (i64, i64) {
        let u = ui_unit(self.ppp);
        let origin = self.min + vec2(GRID_X_U as f32 * u, GRID_Y_U as f32 * u);
        (
            ((p.x - origin.x) * self.ppp).round() as i64,
            ((p.y - origin.y) * self.ppp).round() as i64,
        )
    }

    /// A rectangle in physical pixels from the origin of the grid, in egui points.
    fn grid_rect(&self, r: ScreenRect) -> Rect {
        let u = ui_unit(self.ppp);
        let origin = self.min + vec2(GRID_X_U as f32 * u, GRID_Y_U as f32 * u);
        Rect::from_min_size(
            origin + vec2(r.x as f32 / self.ppp, r.y as f32 / self.ppp),
            vec2(r.width as f32 / self.ppp, r.height as f32 / self.ppp),
        )
    }

    /// An egui point over the panel, in physical pixels from the corner of the block of the panel
    /// (where the indicator band starts).
    fn band_px(&self, p: Pos2) -> (i64, i64) {
        (
            ((p.x - self.min.x) * self.ppp).round() as i64,
            ((p.y - self.min.y) * self.ppp).round() as i64,
        )
    }

    /// A rectangle in `u` from the corner of the panel, in egui points.
    fn u_rect(&self, x: i64, y: i64, width: i64, height: i64) -> Rect {
        let u = ui_unit(self.ppp);
        Rect::from_min_size(
            self.min + vec2(x as f32 * u, y as f32 * u),
            vec2(width as f32 * u, height as f32 * u),
        )
    }
}

/// What the panel asked for in a frame.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PanelOutput {
    /// Choice asked for by a click on a swatch.
    pub choice: Option<ColorChoice>,
    /// Color to apply (releasing a slider or confirming the hexadecimal).
    pub commit: Option<Commit>,
    /// 5 s notice for the status bar (invalid hexadecimal on losing the focus).
    pub notice: Option<String>,
    /// Palette edit asked for (buttons or releasing a swatch drag); the session applies it.
    pub op: Option<PaletteOp>,
    /// With `op` = `Remove`: no pixel used the entry (it is removed without asking for a destination).
    pub op_unused: bool,
    /// Choice asked for by a right click on a swatch: the background color.
    pub background: Option<ColorChoice>,
    /// What a click on the color indicator asked for.
    pub indicator: Option<Indicator>,
    /// `Esc` with nothing else claiming it: the app closes the color editor.
    pub close_editor: bool,
    /// `Esc` that nothing in the palette claimed (not a text field, a swatch drag, a pending removal
    /// nor a disabled panel), whether the editor is open or not: the app uses it to close first the
    /// window of options and then the editor (specs/ui-single-column.md, decision 9).
    pub escape_free: bool,
}

/// What the panel says about the swatch under the pointer (specs/ui-composition.md, decision 7),
/// kept in the egui context after each frame: [`panel_info`] reads it. It is not part of
/// [`PanelOutput`] so that the outputs of the panel stay comparable between frames.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PanelInfo {
    /// What the two fixed text lines of the panel used to say: the swatch under the cursor or, if
    /// none, the chosen one; while a swatch is dragged, what is being moved. They are the
    /// tooltip's two lines.
    pub lines: [String; 2],
    /// The text the status line shows instead of its usual one, by priority: a drag of the handle
    /// or of a swatch, a pending removal or resize, the handle or the swatch under the cursor.
    /// `None`: the usual line.
    pub status_hint: Option<String>,
    /// What `F1` asks help about with the pointer where it is: the handle, or the bar and the
    /// field. `None` elsewhere in the panel.
    pub help: Option<HelpTarget>,
}

fn panel_info_id() -> egui::Id {
    egui::Id::unique("pixoko-panel-info")
}

/// The information the panel left in `ctx` in its last frame (empty before the first one).
pub fn panel_info(ctx: &egui::Context) -> PanelInfo {
    ctx.data(|d| d.get_temp(panel_info_id()))
        .unwrap_or_default()
}

/// What a click on the color indicator asks for (specs/interfaz-dos-colores.md, decision 5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Indicator {
    /// Left click: activate the dropper.
    Dropper,
    /// Right click: the app opens the color editor on this role's color, switches to it if the
    /// editor is open on the other role, or closes it if it is open on this role.
    FocusEditor(Role),
}

/// The two colors and which one the editor edits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Roles {
    pub foreground: ColorChoice,
    pub background: ColorChoice,
    pub editing: Role,
}

/// What `show_panel` needs from the colors: the background is `None` in the old entry points (which
/// do not paint the ring of the background nor say `foreground`/`background`).
#[derive(Clone, Copy)]
pub(crate) struct PanelRoles {
    pub(crate) foreground: ColorChoice,
    pub(crate) background: Option<ColorChoice>,
    pub(crate) editing: Role,
}

impl PanelRoles {
    /// The roles of the column (the two colors and which one the editor edits).
    pub(crate) fn of(roles: Roles) -> PanelRoles {
        PanelRoles {
            foreground: roles.foreground,
            background: Some(roles.background),
            editing: roles.editing,
        }
    }

    fn legacy(choice: ColorChoice) -> PanelRoles {
        PanelRoles {
            foreground: choice,
            background: None,
            editing: Role::Foreground,
        }
    }

    /// The choice the editor edits and on which the palette buttons act.
    pub(crate) fn edited(&self) -> ColorChoice {
        match (self.editing, self.background) {
            (Role::Background, Some(background)) => background,
            _ => self.foreground,
        }
    }
}

/// Paints the panel on the left of `ui` and returns the choice a click asked for (left button
/// pressed over a swatch). `enabled` false (gesture in progress): it is seen and scrolls, but does
/// not choose. It has to be called **after** the status bar and **before** the canvas, which takes
/// the rectangle that is left. Without color editor: [`show_editing`] adds it.
pub fn show(
    ui: &mut Ui,
    image: &Image,
    choice: ColorChoice,
    scroll: &mut Scroll,
    enabled: bool,
) -> Option<ColorChoice> {
    let slots = SlotColors::defaults(image.mode());
    show_panel(
        ui,
        image,
        PanelRoles::legacy(choice),
        scroll,
        &mut PanelView::default(),
        enabled,
        &slots,
        None,
        None,
    )
    .choice
}

/// Like [`show`], with the color editor anchored to the bottom of the panel (specs/hito-2-editar-color.md).
/// The grid uses the height that is left. The editor is brought up to date before (`ColorEditor::sync`);
/// the panel does not touch the document: it returns the color to apply. No memory between frames:
/// neither scroll of the panel nor "show the chosen one" (for that, [`show_editing_view`]).
pub fn show_editing(
    ui: &mut Ui,
    image: &Image,
    choice: ColorChoice,
    scroll: &mut Scroll,
    enabled: bool,
    slots: &SlotColors,
    editor: &mut ColorEditor,
) -> PanelOutput {
    show_editing_view(
        ui,
        image,
        choice,
        scroll,
        &mut PanelView::default(),
        enabled,
        slots,
        editor,
    )
}

/// Like [`show_editing`], with the state of the panel between frames (`view`). If the panel does not
/// fit (`edit_layout::panel_layout`), the block `[band · a row of grid · icons · editor]` scrolls as a
/// whole with the wheel and a visible bar; the chosen row is shown when the number of visible rows
/// changes or if `view.reveal`, never by the wheel (specs/hito-2-panel-ventana-baja.md).
#[allow(clippy::too_many_arguments)]
pub fn show_editing_view(
    ui: &mut Ui,
    image: &Image,
    choice: ColorChoice,
    scroll: &mut Scroll,
    view: &mut PanelView,
    enabled: bool,
    slots: &SlotColors,
    editor: &mut ColorEditor,
) -> PanelOutput {
    show_editing_ops(
        ui,
        image,
        choice,
        scroll,
        view,
        enabled,
        slots,
        editor,
        &mut PaletteOps::default(),
    )
}

/// Like [`show_editing_view`], with the buttons to add, remove and rotate, the swatch drag, the range
/// and the pending removal of the indexed images (`ops`, specs/hito-2-paleta-operaciones.md). The
/// edits come out in `PanelOutput::op`; the panel does not touch the document.
#[allow(clippy::too_many_arguments)]
pub fn show_editing_ops(
    ui: &mut Ui,
    image: &Image,
    choice: ColorChoice,
    scroll: &mut Scroll,
    view: &mut PanelView,
    enabled: bool,
    slots: &SlotColors,
    editor: &mut ColorEditor,
    ops: &mut PaletteOps,
) -> PanelOutput {
    show_panel(
        ui,
        image,
        PanelRoles::legacy(choice),
        scroll,
        view,
        enabled,
        slots,
        Some(editor),
        Some(ops),
    )
}

/// Like [`show_editing_ops`] with the two colors (specs/interfaz-dos-colores.md): the indicator
/// heads the panel; a right click on a swatch asks for the background; the editor and the palette
/// buttons act on the color of `roles.editing`.
#[allow(clippy::too_many_arguments)]
pub fn show_roles(
    ui: &mut Ui,
    image: &Image,
    roles: Roles,
    scroll: &mut Scroll,
    view: &mut PanelView,
    enabled: bool,
    slots: &SlotColors,
    editor: &mut ColorEditor,
    ops: &mut PaletteOps,
) -> PanelOutput {
    show_panel(
        ui,
        image,
        PanelRoles {
            foreground: roles.foreground,
            background: Some(roles.background),
            editing: roles.editing,
        },
        scroll,
        view,
        enabled,
        slots,
        Some(editor),
        Some(ops),
    )
}

#[allow(clippy::too_many_arguments)]
fn show_panel(
    ui: &mut Ui,
    image: &Image,
    roles: PanelRoles,
    scroll: &mut Scroll,
    view: &mut PanelView,
    enabled: bool,
    slots: &SlotColors,
    editor: Option<&mut ColorEditor>,
    ops: Option<&mut PaletteOps>,
) -> PanelOutput {
    let ctx = ui.ctx().clone();
    let ppp = ctx.pixels_per_point();
    let u = ui_unit(ppp);
    let width = PANEL_WIDTH_U as f32 * u;
    Panel::right("paleta")
        .exact_size(width)
        .resizable(false)
        .show_separator_line(false)
        .frame(Frame::new().fill(SURFACE_1))
        .show(ui, |ui| {
            let (rect, _) = ui.allocate_exact_size(ui.available_size(), Sense::hover());
            let place = Placement {
                rect,
                head_px: 0,
                cols: COLUMNS as i64,
            };
            panel_body(
                ui, &place, image, roles, scroll, view, enabled, slots, editor, ops,
            )
        })
        .inner
}

/// Where the body of the panel is painted: the rectangle of the **column** (the panel is the part
/// under the toolbox), how many physical pixels the toolbox takes at the top (`head_px`, 0 when the
/// panel is alone) and the columns of swatches of the grid.
pub(crate) struct Placement {
    pub rect: Rect,
    pub head_px: i64,
    pub cols: i64,
}

/// The body of the panel: events, layout and painting, in `place.rect`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn panel_body(
    ui: &mut Ui,
    place: &Placement,
    image: &Image,
    roles: PanelRoles,
    scroll: &mut Scroll,
    view: &mut PanelView,
    enabled: bool,
    slots: &SlotColors,
    mut editor: Option<&mut ColorEditor>,
    mut ops: Option<&mut PaletteOps>,
) -> PanelOutput {
    let ctx = ui.ctx().clone();
    let ppp = ctx.pixels_per_point();
    let rect = place.rect;
    let (head_px, cols) = (place.head_px, place.cols);
    let panel_cols = usize::try_from(cols).unwrap_or(COLUMNS).max(1);
    {
        {
            let height_px = (rect.height() * ppp).round() as i64;
            let n = text_scale(ppp);
            let mut layout = Layout {
                ppp,
                n,
                rect,
                min: rect.min + vec2(0.0, head_px as f32 / ppp),
                cols,
            };
            let mode = image.mode();
            let choice = roles.foreground;
            let edited = roles.edited();
            let target = editor.as_ref().and_then(|_| EditTarget::of(image, edited));
            // The editor is shown when the app opened it and there is something to edit.
            let shown_target = target.filter(|_| view.editor_open);
            let active_entry = match edited {
                ColorChoice::Entry(index) => Some(index),
                ColorChoice::Slot(_) => None,
            };
            // Palette operations (the icon row under the grid): indexed images with an editor
            // state and an entry chosen, whether the editor is open or closed.
            let ops_on = mode == Mode::Indexed
                && editor.is_some()
                && ops.is_some()
                && active_entry.is_some();
            // Vertical split. With an editor state: the open layout (editor anchored to the
            // bottom, icons right above) or the closed one (icons at the bottom); both may scroll
            // the whole panel. Without one (the old entry points) the grid takes all the height.
            let has_editor = editor.is_some();
            let slot = |offset_px: i64| {
                if !has_editor {
                    PanelLayout {
                        scrolling: false,
                        offset_px: 0,
                        max_offset_px: 0,
                        top_px: height_px,
                        tail_top_px: height_px,
                        visible_rows: visible_rows_reserved(height_px - head_px, 0, n),
                    }
                } else {
                    column_layout(
                        mode,
                        height_px,
                        n,
                        offset_px,
                        head_px,
                        shown_target.is_some(),
                    )
                }
            };
            let block_px = head_px
                + if shown_target.is_some() || !has_editor {
                    block_height_px(mode, n)
                } else {
                    block_height_closed_px(mode, n)
                };
            // Opening the editor takes the scroll of the block to the maximum of the open block (the
            // editor in sight); closing it brings back the value it had before.
            if has_editor {
                let open_max = column_layout(mode, height_px, n, 0, head_px, true).max_offset_px;
                view.sync_editor(shown_target.is_some(), open_max);
            }
            let mut placed = slot(view.offset_px);
            view.offset_px = placed.offset_px;
            if !placed.scrolling {
                view.rest_px = 0;
            }
            layout.min = rect.min + vec2(0.0, (head_px - placed.offset_px) as f32 / ppp);
            let mut output = PanelOutput::default();
            let count = cell_count(image);
            // The handle: the cell after the last entry of an indexed image (wherever the preview
            // or the pending cut puts it), only where the panel keeps the state to drag it.
            let handle_on = ops.is_some() && handle_index(image).is_some();
            let HandleView {
                handle_pos, cells, ..
            } = handle_view(handle_on, count, panel_cols, ops.as_deref());
            let total = total_rows_cols(cells, panel_cols);
            let visible = placed.visible_rows;
            // The icon row hangs under a short grid and is anchored below a tall one.
            let tail_top_px = placed.tail_top_for_at(total, n, head_px);
            scroll.clamp(total, visible);
            let changed = view.reveal || view.last_visible.is_some_and(|last| last != visible);
            if changed && let Some(cell) = cell_of_choice(image, choice) {
                scroll.reveal(cell / panel_cols, total, visible);
            }
            view.reveal = false;
            view.last_visible = Some(visible);
            let busy_editing = editor.as_ref().is_some_and(|editor| editor.dragging());
            let text_focused =
                ctx.text_edit_focused() || ops.as_deref().is_some_and(|ops| ops.text_focus());
            let stored = |target: EditTarget| target.stored(image, slots);
            let primary_down = ui.input(|i| i.pointer.primary_down());
            let pre_geometry = Geometry {
                ppp,
                n: layout.n,
                panel_min: rect.min,
                panel_rect: rect,
                top_px: tail_top_px,
                mode,
                cols,
            };
            let phys = |p: Pos2| ((p.x * ppp).round() as i64, (p.y * ppp).round() as i64);

            let pointer = ui
                .input(|i| i.pointer.hover_pos())
                .filter(|p| rect.contains(*p));
            // The part of the column under the toolbox: what the grid and the tooltips see.
            let pointer_in_panel = pointer.filter(|p| p.y >= layout.min.y);
            let events = ui.input(|i| i.events.clone());
            let mut picked = None;
            for event in &events {
                match event {
                    Event::MouseWheel {
                        unit,
                        delta,
                        modifiers,
                        ..
                    } if !modifiers.command
                        && delta.y != 0.0
                        && (if placed.scrolling {
                            pointer.is_some()
                        } else {
                            pointer_in_panel.is_some()
                        }) =>
                    {
                        let pitch_px = (PITCH_U * i64::from(layout.n)) as f32;
                        let down = -delta.y;
                        let page = if placed.scrolling {
                            height_px as f32
                        } else {
                            visible as f32 * pitch_px
                        };
                        let pixels = match unit {
                            MouseWheelUnit::Point => down * ppp,
                            MouseWheelUnit::Line => down * WHEEL_LINE_ROWS * pitch_px,
                            MouseWheelUnit::Page => down * page,
                        };
                        let pixels = pixels.round() as i64;
                        if !placed.scrolling {
                            scroll.by(pixels, layout.n, total, visible);
                        } else if !busy_editing {
                            view.scroll_by(pixels, layout.n, placed.max_offset_px);
                        }
                    }
                    Event::PointerButton {
                        pos,
                        button: button @ (PointerButton::Primary | PointerButton::Secondary),
                        pressed: true,
                        ..
                    } if roles.background.is_some()
                        && enabled
                        && !busy_editing
                        && rect.contains(*pos)
                        && {
                            let (bx, by) = layout.band_px(*pos);
                            (0..INDICATOR_H_U * i64::from(layout.n)).contains(&by)
                                && (0..panel_width_u(cols) * i64::from(layout.n)).contains(&bx)
                        } =>
                    {
                        let (bx, by) = layout.band_px(*pos);
                        if let Some(role) = indicator_hit(bx, by, layout.n) {
                            output.indicator = Some(if *button == PointerButton::Primary {
                                Indicator::Dropper
                            } else {
                                // A second right click on the rectangle being edited closes the
                                // editor: nothing to focus then.
                                let closing = view.editor_open && roles.editing == role;
                                if editor.is_some() && !closing {
                                    // The focus is requested in a frame without pointer: a click
                                    // takes the focus away from any widget that is not hovered.
                                    ctx.data_mut(|d| d.insert_temp(hex_focus_pending(), true));
                                    ctx.request_repaint();
                                }
                                Indicator::FocusEditor(role)
                            });
                        }
                    }
                    Event::PointerButton {
                        pos,
                        button: PointerButton::Secondary,
                        pressed: true,
                        ..
                    } if roles.background.is_some()
                        && enabled
                        && !busy_editing
                        && rect.contains(*pos)
                        && ops.as_deref().is_none_or(|ops| ops.removal().is_none()) =>
                    {
                        let grid = Grid::with_columns(
                            layout.n,
                            panel_cols,
                            count,
                            visible,
                            scroll.first_row,
                        );
                        let (x, y) = layout.grid_px(*pos);
                        if let Some(cell) = grid.index_at(x, y) {
                            output.background = choice_for_cell(image, cell);
                        }
                    }
                    Event::PointerButton {
                        pos,
                        button: PointerButton::Primary,
                        pressed: true,
                        modifiers,
                    } if enabled && !busy_editing && rect.contains(*pos) => {
                        if ops_on {
                            let removing = ops.as_deref().is_some_and(|o| o.confirming());
                            let (rx, ry) = pre_geometry.rel_px(*pos);
                            if let Some(button) = button_hit(mode, removing, rx, ry, layout.n) {
                                // Pressing a button is leaving the field: its pending text is
                                // resolved before the operation (the `Commit` comes first).
                                if let Some(editor) = editor.as_mut() {
                                    resolve_pending(editor, mode, &stored, &mut output);
                                    ctx.memory_mut(|m| m.surrender_focus(hex_field_id()));
                                }
                                if let (Some(ops), Some(active)) =
                                    (ops.as_deref_mut(), active_entry)
                                {
                                    press_button(button, image, active, count, ops, &mut output);
                                }
                                continue;
                            }
                        }
                        let grid = Grid::with_columns(
                            layout.n,
                            panel_cols,
                            count,
                            visible,
                            scroll.first_row,
                        );
                        let (x, y) = layout.grid_px(*pos);
                        // Pressing the handle arms its drag (it does not choose anything).
                        if handle_on
                            && handle_pos == Some(count)
                            && let Some(ops) = ops.as_deref_mut()
                            && !ops.confirming()
                            && Grid::with_columns(
                                layout.n,
                                panel_cols,
                                cells,
                                visible,
                                scroll.first_row,
                            )
                            .index_at(x, y)
                                == Some(count)
                        {
                            ops.begin_handle(count, phys(*pos));
                            continue;
                        }
                        if let Some(cell) = grid.index_at(x, y) {
                            let cell_u8 = u8::try_from(cell).ok();
                            match (ops.as_deref_mut(), cell_u8) {
                                (Some(ops), Some(cell_u8)) if ops_on => {
                                    if ops.removal().is_some() {
                                        // With a pending removal, pressing is choosing the destination.
                                        ops.set_destination(cell_u8, count);
                                    } else if ops.resize_pending().is_some() {
                                        // With a pending cut, the destination is a swatch that is
                                        // kept; the ones that are cut off do not answer.
                                        ops.set_resize_destination(cell_u8);
                                    } else if modifiers.shift {
                                        // `Shift` + click: sets the range, without choosing or dragging.
                                        ops.set_range_end(cell_u8);
                                    } else {
                                        picked = choice_for_cell(image, cell).or(picked);
                                        ops.clear_range();
                                        ops.begin_drag(cell_u8, phys(*pos));
                                    }
                                }
                                _ => picked = choice_for_cell(image, cell).or(picked),
                            }
                        }
                    }
                    Event::PointerMoved(pos) => {
                        if let Some(ops) = ops.as_deref_mut() {
                            let first_row = scroll.first_row;
                            let target = drop_target(&layout, count, visible, first_row, *pos);
                            if let Some(drag) = ops.drag() {
                                drag.update(phys(*pos), layout.n, target);
                            }
                            let zone = zone_cells(cells, count, ops, panel_cols);
                            let len = handle_target(&layout, zone, visible, first_row, *pos);
                            if let Some(handle) = ops.handle() {
                                handle.update(phys(*pos), layout.n, len);
                            }
                        }
                    }
                    Event::PointerButton {
                        pos,
                        button: PointerButton::Primary,
                        pressed: false,
                        ..
                    } => {
                        if let Some(ops) = ops.as_deref_mut() {
                            let first_row = scroll.first_row;
                            let target = drop_target(&layout, count, visible, first_row, *pos);
                            if let Some(drag) = ops.drag() {
                                drag.update(phys(*pos), layout.n, target);
                            }
                            finish_drag(ops, &mut output);
                            let zone = zone_cells(cells, count, ops, panel_cols);
                            let len = handle_target(&layout, zone, visible, first_row, *pos);
                            if let Some(handle) = ops.handle() {
                                handle.update(phys(*pos), layout.n, len);
                            }
                            finish_handle(ops, &mut output);
                        }
                    }
                    Event::WindowFocused(false) => {
                        if let Some(ops) = ops.as_deref_mut() {
                            ops.cancel_drag();
                            ops.cancel_handle();
                        }
                    }
                    event if is_escape(event) && !text_focused => {
                        // Priority of `Escape`: the text field, the drawing gesture or the
                        // slider (they do not get here), the swatch drag, the pending removal and, last, closing the editor.
                        let mut claimed = false;
                        if let Some(ops) = ops.as_deref_mut() {
                            if ops.drag_state().is_some() || ops.handle_state().is_some() {
                                ops.cancel_drag();
                                ops.cancel_handle();
                                claimed = true;
                            } else if ops.removal().is_some() && enabled && !busy_editing {
                                ops.cancel_removal();
                                claimed = true;
                            } else if ops.resize_pending().is_some() && enabled && !busy_editing {
                                ops.cancel_resize();
                                claimed = true;
                            }
                        }
                        // Last in the chain: nothing in the palette claimed `Escape`. It is free for the
                        // app (the window of options first) and, if the editor is open, it closes it.
                        if !claimed && enabled && !busy_editing {
                            output.escape_free = true;
                            if shown_target.is_some() {
                                output.close_editor = true;
                            }
                        }
                    }
                    _ => {}
                }
            }
            if let Some(ops) = ops.as_deref_mut()
                && ops.drag_state().is_some()
            {
                // After the wheel or a layout change the destination may have changed; and if
                // egui no longer sees the button (the release was lost), it is a release.
                if let Some(pos) = ui.input(|i| i.pointer.latest_pos()) {
                    let first_row = scroll.first_row;
                    let target = drop_target(&layout, count, visible, first_row, pos);
                    if let Some(drag) = ops.drag() {
                        drag.update(phys(pos), layout.n, target);
                    }
                }
                if !primary_down {
                    finish_drag(ops, &mut output);
                }
            }
            if let Some(ops) = ops.as_deref_mut()
                && ops.handle_state().is_some()
            {
                // The same for the handle, plus the scroll of the rows on its own when the pointer
                // is held in the first or the last half row of the zone.
                if let Some(pos) = ui.input(|i| i.pointer.latest_pos()) {
                    let zone = zone_cells(cells, count, ops, panel_cols);
                    let band = autoscroll_band(&layout, zone, visible, scroll.first_row, pos);
                    let dt_ms = (ui.input(|i| i.stable_dt) * 1000.0).round() as u64;
                    let rows = ops.handle().map_or(0, |h| h.autoscroll(dt_ms, band));
                    if rows != 0 {
                        let pitch_px = PITCH_U * i64::from(layout.n);
                        scroll.by(i64::from(rows) * pitch_px, layout.n, total, visible);
                    }
                    if band != 0 {
                        ctx.request_repaint();
                    }
                    let first_row = scroll.first_row;
                    let len = handle_target(&layout, zone, visible, first_row, pos);
                    if let Some(handle) = ops.handle() {
                        handle.update(phys(pos), layout.n, len);
                    }
                }
                if !primary_down {
                    finish_handle(ops, &mut output);
                }
            }
            output.choice = picked;
            let pending: bool = ctx
                .data(|d| d.get_temp(hex_focus_pending()))
                .unwrap_or(false);
            let pointer_active =
                ctx.input(|input| input.pointer.any_click() || input.pointer.any_pressed());
            if pending && !pointer_active {
                if ctx.memory(|m| m.has_focus(hex_field_id())) {
                    ctx.data_mut(|d| d.insert_temp(hex_focus_pending(), false));
                } else {
                    ctx.memory_mut(|m| m.request_focus(hex_field_id()));
                    ctx.request_repaint();
                }
            }
            placed = slot(view.offset_px);
            view.offset_px = placed.offset_px;
            layout.min = rect.min + vec2(0.0, (head_px - placed.offset_px) as f32 / ppp);

            // What the events of this frame did to the handle (the preview, a cut that was
            // asked for) is what is painted now.
            let HandleView {
                preview_len,
                pending_resize,
                handle_pos,
                cells,
            } = handle_view(handle_on, count, panel_cols, ops.as_deref());
            let total = total_rows_cols(cells, panel_cols);

            // The tail under the grid, where it falls now (after the wheel of this frame).
            let paint_tail_top_px = placed.tail_top_for_at(total, n, head_px);
            let tail_geo = Geometry {
                ppp,
                n: layout.n,
                panel_min: rect.min,
                panel_rect: rect,
                top_px: paint_tail_top_px,
                mode,
                cols,
            };
            let mut preview = None;
            if let Some(editor) = editor.as_mut() {
                match target {
                    Some(target) => {
                        // The bar and the field are always seen (the editor open or closed).
                        let transparent = matches!(target, EditTarget::Entry(index)
                            if image.palette().and_then(|p| p.transparent_index()) == Some(index));
                        let shown = color_editor::show_tail(
                            ui,
                            &tail_geo,
                            editor,
                            target,
                            transparent,
                            &events,
                            &stored,
                        );
                        output.commit = shown.commit.or(output.commit);
                        output.notice = shown.notice.or(output.notice.take());
                        if shown_target.is_some() {
                            let geometry = Geometry {
                                top_px: placed.top_px,
                                ..tail_geo
                            };
                            output.commit = color_editor::interact(
                                &events,
                                &geometry,
                                editor,
                                enabled,
                                primary_down,
                            )
                            .or(output.commit);
                            preview = editor.preview();
                            color_editor::show(ui, &geometry, editor);
                        }
                    }
                    None => {
                        // The field disappears (nothing to edit): its pending text is resolved as
                        // when it loses the focus (egui does not tell about a field that is no
                        // longer painted).
                        match editor.confirm_hex_checked(mode, true, stored) {
                            Resolved::Done(HexOutcome::Commit(commit)) => {
                                output.commit = Some(commit);
                            }
                            Resolved::Done(HexOutcome::Invalid(error)) => {
                                output.notice = Some(hex_notice(&error).to_string());
                            }
                            Resolved::Done(_) => {}
                            Resolved::Stale(reason) => {
                                output.notice = Some(stale_notice(reason).to_string());
                            }
                        }
                        editor.cancel();
                    }
                }
            }

            if ops_on {
                let removal = ops.as_deref().and_then(|o| o.removal());
                show_icon_row(
                    ui,
                    &tail_geo,
                    ButtonsView {
                        removing: ops.as_deref().is_some_and(|o| o.confirming()),
                        len: count,
                        has_destination: removal.is_some_and(|r| r.into.is_some())
                            || pending_resize.is_some_and(|p| !p.needs_destination()),
                        transparent_active: matches!(
                            (image.palette(), active_entry),
                            (Some(palette), Some(active))
                                if palette.transparent_index() == Some(active)
                        ),
                        icons: view.icons,
                    },
                );
            }

            let grid = Grid::with_columns(layout.n, panel_cols, count, visible, scroll.first_row);
            let zone_grid =
                Grid::with_columns(layout.n, panel_cols, cells, visible, scroll.first_row);
            let hover = pointer_in_panel.and_then(|p| {
                let (x, y) = layout.grid_px(p);
                grid.index_at(x, y)
            });
            // The pointer is over the handle (or dragging it): grab cursor and its hint.
            let dragging_handle = preview_len.is_some();
            let over_handle = enabled
                && !busy_editing
                && handle_pos.is_some()
                && pointer_in_panel.is_some_and(|p| {
                    let (x, y) = layout.grid_px(p);
                    zone_grid.index_at(x, y) == handle_pos
                })
                && ops
                    .as_deref()
                    .is_some_and(|o| !o.confirming() || dragging_handle);
            if dragging_handle {
                ctx.set_cursor_icon(CursorIcon::Grabbing);
            } else if over_handle {
                ctx.set_cursor_icon(CursorIcon::Grab);
            }
            let panel_bar = placed
                .scrolling
                .then(|| panel_thumb(placed.offset_px, height_px, block_px, n));
            if let Some(ops) = ops.as_deref_mut() {
                ops.set_text_focus(ctx.text_edit_focused());
            }
            let mut marks = Marks::new(ops.as_deref(), active_entry, ops_on);
            if let Some(pos) = handle_pos {
                if dragging_handle {
                    marks.dimmed = (pos < count).then_some((pos, count));
                    marks.added = (pos > count).then_some((count, pos));
                } else if let Some(pending) = pending_resize {
                    marks.dimmed = Some((usize::from(pending.len), count));
                }
                marks.handle = Some(HandleMark {
                    cell: pos,
                    hot: dragging_handle || over_handle,
                    inert: pending_resize.is_some(),
                });
            }
            let edit_mark = shown_target.map(|_| roles.editing);
            paint(
                ui, &layout, image, slots, roles, &zone_grid, hover, preview, scroll, total,
                visible, panel_bar, &marks, edit_mark,
            );

            // The information of the swatch: what the two fixed lines used to paint now goes to
            // the tooltip and, by priority, to the status line.
            let active = cell_of_choice(image, roles.foreground);
            let background = roles
                .background
                .and_then(|choice| cell_of_choice(image, choice));
            let info = swatch_info(
                image, slots, &roles, hover, active, background, &marks, ppp, n, cols,
            );
            let removal_text = ops
                .as_deref()
                .and_then(|o| o.removal())
                .filter(|_| ops_on)
                .map(|r| {
                    let warn = image
                        .palette()
                        .and_then(|p| removal_warning(p, r.index, r.into));
                    removal_prompt(r.index, r.into, warn)
                });
            let joined = |lines: &[String; 2]| {
                if lines[1].is_empty() {
                    lines[0].clone()
                } else {
                    format!("{} · {}", lines[0], lines[1])
                }
            };
            let status_hint = if let Some(len) = preview_len {
                Some(drag_status(count, len))
            } else if marks.drag_text.is_some() {
                Some(joined(&info))
            } else if let Some(lines) = &removal_text {
                Some(joined(lines))
            } else if let Some(pending) = pending_resize {
                Some(resize_prompt(&pending))
            } else if over_handle {
                Some(HANDLE_HINT.to_string())
            } else if hover.is_some() {
                Some(joined(&info))
            } else {
                None
            };
            // `F1` over the handle or over the bar and the field opens their help.
            let over_bar = target.is_some()
                && editor.is_some()
                && pointer.is_some_and(|p| {
                    let (x, y) = tail_geo.rel_px(p);
                    (0..panel_width_u(cols) * i64::from(layout.n)).contains(&x)
                        && (0..(FIELD_Y_U + FIELD_H_U) * i64::from(layout.n)).contains(&y)
                });
            let help = if over_handle || dragging_handle {
                Some(HelpTarget::PaletteHandle)
            } else if over_bar {
                Some(HelpTarget::ColorBar)
            } else {
                None
            };
            ctx.data_mut(|d| {
                d.insert_temp(
                    panel_info_id(),
                    PanelInfo {
                        lines: info.clone(),
                        status_hint,
                        help,
                    },
                );
            });

            // Tooltip of each rectangle of the indicator (the swatches have none: their text goes
            // to the status line only).
            let font = FontId::new(font_size_points(ppp), FontFamily::Proportional);
            let tip = |ui: &mut Ui, lines: &[&str]| {
                for (i, line) in lines.iter().enumerate().filter(|(_, l)| !l.is_empty()) {
                    let tone = if i == 0 { TEXT } else { TEXT_DIM };
                    ui.add(
                        Label::new(
                            RichText::new(printable(&ctx, line))
                                .font(font.clone())
                                .color(tone),
                        )
                        .extend(),
                    );
                }
            };
            if roles.background.is_some() {
                // One area over both rectangles; the pointer says which one (the small one wins
                // where they overlap, as for the clicks).
                let (fg, bg) = indicator_rects(n);
                let area = panel_rect_of(&layout, fg)
                    .union(panel_rect_of(&layout, bg))
                    .intersect(rect);
                let over = pointer.and_then(|p| {
                    let (x, y) = layout.band_px(p);
                    indicator_hit(x, y, n)
                });
                if area.is_positive()
                    && let Some(role) = over
                {
                    let name = match role {
                        Role::Foreground => "Primer plano",
                        Role::Background => "Fondo",
                    };
                    let response = ui.interact(
                        area,
                        ui.unique_id().with("indicator-tooltip"),
                        Sense::hover(),
                    );
                    response.on_hover_ui(|ui| tip(ui, &[name, INDICATOR_TIP]));
                }
            }
            output
        }
    }
}

/// Tooltip text of the indicator rectangles (second line).
pub const INDICATOR_TIP: &str = "clic: cuentagotas; clic derecho: editar este color";

/// What the two fixed lines of the panel said: the swatch under the cursor or, if none, the chosen
/// one; while a swatch is dragged, what is being moved.
#[allow(clippy::too_many_arguments)]
fn swatch_info(
    image: &Image,
    slots: &SlotColors,
    roles: &PanelRoles,
    hover: Option<usize>,
    active: Option<usize>,
    background: Option<usize>,
    marks: &Marks,
    ppp: f32,
    n: u32,
    swatch_cols: i64,
) -> [String; 2] {
    if let Some(lines) = &marks.drag_text {
        return lines.clone();
    }
    let shown = hover.or(active);
    let line1 = shown
        .and_then(|cell| cell_label_with(image, cell, slots))
        .unwrap_or_default();
    let cols = columns((text_width_u(swatch_cols) * i64::from(n)) as f32, ppp);
    let line2 = shown
        .map(|cell| {
            if roles.background.is_some() {
                info_flags_roles(
                    RoleFlags {
                        hover: hover.is_some(),
                        foreground: Some(cell) == active,
                        background: Some(cell) == background,
                        in_range: marks.in_range(cell),
                        transparent: is_transparent_cell_with(image, cell, slots),
                        alpha0: is_alpha0_cell_with(image, cell, slots),
                    },
                    cols,
                )
            } else {
                info_flags_alpha0(
                    hover.is_some(),
                    Some(cell) == active,
                    marks.in_range(cell),
                    is_transparent_cell_with(image, cell, slots),
                    is_alpha0_cell_with(image, cell, slots),
                )
            }
        })
        .unwrap_or_default();
    [line1, line2]
}

#[allow(clippy::too_many_arguments)]
fn paint(
    ui: &Ui,
    layout: &Layout,
    image: &Image,
    slots: &SlotColors,
    roles: PanelRoles,
    grid: &Grid,
    hover: Option<usize>,
    preview: Option<(EditTarget, Rgba)>,
    scroll: &Scroll,
    total: usize,
    visible: usize,
    panel_bar: Option<Option<(i64, i64)>>,
    marks: &Marks,
    edit_mark: Option<Role>,
) {
    let (n, ppp) = (layout.n, layout.ppp);
    let n_px = i64::from(n);
    let painter = ui.painter_at(layout.rect);
    let active = cell_of_choice(image, roles.foreground);
    let background = roles
        .background
        .and_then(|choice| cell_of_choice(image, choice));

    // Lines: under the band and on the right.
    painter.rect_filled(
        layout.u_rect(0, SEPARATOR_Y_U, panel_width_u(layout.cols), 1),
        0.0,
        SURFACE_3,
    );
    // Two 1 u lines over the part of the column that is the panel (under the toolbox): the left one
    // separates it from the canvas (it falls in the 4 u margin of the grid) and the right one is the
    // last column of the panel.
    let top = layout.min.y.max(layout.rect.min.y);
    for x_u in [0, right_line_x_u(layout.cols)] {
        let line = layout.u_rect(x_u, 0, 1, 0);
        painter.rect_filled(
            Rect::from_min_size(
                Pos2::new(line.min.x, top),
                vec2(line.width(), (layout.rect.max.y - top).max(0.0)),
            ),
            0.0,
            SURFACE_3,
        );
    }

    if let Some(background_choice) = roles.background {
        paint_band(
            &painter,
            layout,
            image,
            slots,
            roles,
            background_choice,
            edit_mark,
        );
    }

    // Muestras visibles.
    for index in 0..cell_count(image) {
        let Some(cell) = grid.cell_rect(index) else {
            continue;
        };
        let mut color = cell_color_with(image, index, slots).unwrap_or(Rgba::TRANSPARENT);
        if marks.is_dimmed(index) {
            color = dimmed(color);
        }
        if let Some((target, provisional)) = preview
            && edited_cell(target) == index
        {
            // The edited swatch is painted with the provisional color (resolved: the transparent
            // entry stays at alpha 0).
            let transparent = is_transparent_cell_with(image, index, slots);
            color = Rgba {
                a: if transparent { 0 } else { provisional.a },
                ..provisional
            };
        }
        for (part, tone) in swatch_parts(n, color) {
            let part = ScreenRect {
                x: cell.x + part.x,
                y: cell.y + part.y,
                ..part
            };
            painter.rect_filled(layout.grid_rect(part), 0.0, to_color32(tone));
        }
    }

    // The entries that dragging the handle past the last one would add: opaque black with a 1 u ring.
    if let Some((from, to)) = marks.added {
        for index in from..to {
            let Some(cell) = grid.cell_rect(index) else {
                continue;
            };
            for (part, tone) in swatch_parts(n, Rgba::opaque(0, 0, 0)) {
                let part = ScreenRect {
                    x: cell.x + part.x,
                    y: cell.y + part.y,
                    ..part
                };
                painter.rect_filled(layout.grid_rect(part), 0.0, to_color32(tone));
            }
            for ring in ring_rects(cell, RING_RANGE_U * n_px) {
                painter.rect_filled(layout.grid_rect(ring), 0.0, RING_RANGE);
            }
        }
    }

    // Rings: those of the rotation range, the one of the cursor (if it is not the chosen one), the one
    // of the chosen one and the destination of a pending removal.
    if let Some((low, high)) = marks.range {
        for cell in (low..=high).filter(|cell| Some(*cell) != active) {
            let Some(rect) = grid.cell_rect(cell) else {
                continue;
            };
            for ring in ring_rects(rect, RING_RANGE_U * n_px) {
                painter.rect_filled(layout.grid_rect(ring), 0.0, RING_RANGE);
            }
        }
    }
    let rings = [
        (
            hover.filter(|cell| Some(*cell) != active && Some(*cell) != background),
            RING_HOVER_U,
            RING_HOVER,
        ),
        (active, RING_ACTIVE_U, RING_ACTIVE),
    ];
    for (cell, thickness, color) in rings {
        let Some(rect) = cell.and_then(|cell| grid.cell_rect(cell)) else {
            continue;
        };
        for ring in ring_rects(rect, thickness * n_px) {
            painter.rect_filled(layout.grid_rect(ring), 0.0, color);
        }
    }

    // The background: four L-shaped corners; with the same swatch as the foreground, outside its
    // ring.
    if let Some(rect) = background.and_then(|cell| grid.cell_rect(cell)) {
        let offset = if background == active {
            RING_ACTIVE_U * n_px
        } else {
            0
        };
        for corner in corner_rects(
            rect,
            offset,
            RING_BACKGROUND_U * n_px,
            RING_BACKGROUND_ARM_U * n_px,
        ) {
            painter.rect_filled(layout.grid_rect(corner), 0.0, RING_BACKGROUND);
        }
    }

    if let Some(rect) = marks.dest.and_then(|cell| grid.cell_rect(cell)) {
        for ring in ring_rects(rect, RING_DEST_U * n_px) {
            painter.rect_filled(layout.grid_rect(ring), 0.0, RING_DEST);
        }
    }
    // The handle: two bars on a face with a 1 u border, over the entry it falls on, if any.
    if let Some(mark) = &marks.handle
        && let Some(cell) = grid.cell_rect(mark.cell)
    {
        let (face, bars) = if mark.inert {
            (SURFACE_1, TEXT_DISABLED)
        } else if mark.hot {
            (SURFACE_3, ACCENT)
        } else {
            (SURFACE_2, TEXT_DIM)
        };
        painter.rect_filled(layout.grid_rect(cell), 0.0, SURFACE_3);
        let inner = ScreenRect {
            x: cell.x + n_px,
            y: cell.y + n_px,
            width: cell.width - 2 * n_px,
            height: cell.height - 2 * n_px,
        };
        painter.rect_filled(layout.grid_rect(inner), 0.0, face);
        for bar in handle_bars(cell, n) {
            painter.rect_filled(layout.grid_rect(bar), 0.0, bars);
        }
    }
    // Bar of the destination of a drag.
    if let Some((from, to)) = marks.drop
        && let Some(rect) = grid.cell_rect(to)
    {
        let bar = drop_mark_rect(rect, to > from, n);
        painter.rect_filled(layout.grid_rect(bar), 0.0, DROP_MARK);
    }

    // Track and indicator, only if scrolling is needed: those of the whole panel if it does not fit;
    // otherwise, those of the grid.
    if let Some(bar) = panel_bar {
        if let Some((y, height)) = bar {
            let x = layout.u_rect(track_x_u(layout.cols), 0, 0, 0).min.x;
            let width = TRACK_W_U as f32 * ui_unit(ppp);
            let top = layout.rect.min.y;
            painter.rect_filled(
                Rect::from_min_size(Pos2::new(x, top), vec2(width, layout.rect.height())),
                0.0,
                SURFACE_2,
            );
            painter.rect_filled(
                Rect::from_min_size(
                    Pos2::new(x, top + y as f32 / ppp),
                    vec2(width, height as f32 / ppp),
                ),
                0.0,
                TEXT_DIM,
            );
        }
    } else if let Some((y, height)) = thumb(scroll.first_row, visible, total, n) {
        let track_h = visible as i64 * PITCH_U;
        painter.rect_filled(
            layout.u_rect(track_x_u(layout.cols), GRID_Y_U, TRACK_W_U, track_h),
            0.0,
            SURFACE_2,
        );
        let top = layout
            .u_rect(track_x_u(layout.cols), GRID_Y_U, TRACK_W_U, 0)
            .min;
        let bar = Rect::from_min_size(
            top + vec2(0.0, y as f32 / ppp),
            vec2(TRACK_W_U as f32 * ui_unit(ppp), height as f32 / ppp),
        );
        painter.rect_filled(bar, 0.0, TEXT_DIM);
    }
}

/// Rectangle of a [`ScreenRect`] given from the panel's corner, in egui points.
fn panel_rect_of(layout: &Layout, r: ScreenRect) -> Rect {
    Rect::from_min_size(
        layout.min + vec2(r.x as f32 / layout.ppp, r.y as f32 / layout.ppp),
        vec2(r.width as f32 / layout.ppp, r.height as f32 / layout.ppp),
    )
}

/// The top band: the color indicator (the small one, the foreground, on top of the large one, the
/// background). `edit_mark` is the role the open editor edits: its border is 2 u of `ACCENT`
/// instead of 1 u of `SURFACE_3`.
fn paint_band(
    painter: &egui::Painter,
    layout: &Layout,
    image: &Image,
    slots: &SlotColors,
    roles: PanelRoles,
    background: ColorChoice,
    edit_mark: Option<Role>,
) {
    let n = i64::from(layout.n);
    let color_of = |choice: ColorChoice| {
        cell_of_choice(image, choice)
            .and_then(|cell| cell_color_with(image, cell, slots))
            .unwrap_or(Rgba::TRANSPARENT)
    };
    let (fg, bg) = indicator_rects(layout.n);
    for (role, rect, choice) in [
        (Role::Background, bg, background),
        (Role::Foreground, fg, roles.foreground),
    ] {
        let (thickness, tone) = if edit_mark == Some(role) {
            (IND_BORDER_EDIT_U, ACCENT)
        } else {
            (IND_BORDER_U, SURFACE_3)
        };
        for border in ring_rects(rect, thickness * n) {
            painter.rect_filled(panel_rect_of(layout, border), 0.0, tone);
        }
        for (part, tone) in block_parts(rect.width, rect.height, 5 * n, color_of(choice)) {
            let part = ScreenRect {
                x: rect.x + part.x,
                y: rect.y + part.y,
                ..part
            };
            painter.rect_filled(panel_rect_of(layout, part), 0.0, to_color32(tone));
        }
    }
}

/// What the palette operations add to the painting of the grid.
#[derive(Default)]
struct Marks {
    /// Swatches of the rotation range (ends included).
    range: Option<(usize, usize)>,
    /// Chosen destination of the pending removal.
    dest: Option<usize>,
    /// Drag with destination: `(from, to)`.
    drop: Option<(usize, usize)>,
    /// Lines of information while dragging.
    drag_text: Option<[String; 2]>,
    /// Entries `[from, to)` painted dimmed: the ones a drag of the handle or a pending cut would
    /// cut.
    dimmed: Option<(usize, usize)>,
    /// New entries `[from, to)` that the handle dragged past the last entry would add.
    added: Option<(usize, usize)>,
    /// The handle, if the image has one.
    handle: Option<HandleMark>,
}

/// How the handle is painted.
#[derive(Clone, Copy, Debug)]
struct HandleMark {
    cell: usize,
    /// The pointer is over it or dragging it.
    hot: bool,
    /// A cut is pending: it neither answers nor changes.
    inert: bool,
}

impl Marks {
    fn new(ops: Option<&PaletteOps>, active: Option<u8>, on: bool) -> Marks {
        let Some(ops) = ops.filter(|_| on) else {
            return Marks::default();
        };
        let range = active
            .zip(ops.range_end())
            .map(|(a, e)| (usize::from(a.min(e)), usize::from(a.max(e))));
        let drag = ops.drag_state().filter(|drag| drag.dragging());
        Marks {
            range,
            dest: ops
                .removal()
                .and_then(|r| r.into)
                .or_else(|| ops.resize_pending().and_then(|r| r.into))
                .map(usize::from),
            drop: drag
                .and_then(|d| d.preview())
                .map(|(from, to)| (usize::from(from), usize::from(to))),
            drag_text: drag.map(|d| drag_labels(d.from(), d.target())),
            ..Marks::default()
        }
    }

    fn is_dimmed(&self, cell: usize) -> bool {
        self.dimmed
            .is_some_and(|(from, to)| (from..to).contains(&cell))
    }

    fn in_range(&self, cell: usize) -> bool {
        self.range
            .is_some_and(|(low, high)| (low..=high).contains(&cell))
    }
}

/// Destination of a drag under an egui point: the final index of the moved entry.
fn drop_target(
    layout: &Layout,
    count: usize,
    visible: usize,
    first_row: usize,
    pos: Pos2,
) -> Option<u8> {
    let columns = usize::try_from(layout.cols).unwrap_or(COLUMNS).max(1);
    let grid = Grid::with_columns(layout.n, columns, count, visible, first_row);
    let (x, y) = layout.grid_px(pos);
    grid.drop_index(x, y)
        .and_then(|index| u8::try_from(index).ok())
}

/// How the handle looks in the state of `ops`: where it is, the length it previews while it is
/// dragged, the cut that is pending, and the cells the grid needs for them.
struct HandleView {
    preview_len: Option<usize>,
    pending_resize: Option<PendingResize>,
    handle_pos: Option<usize>,
    cells: usize,
}

fn handle_view(
    handle_on: bool,
    count: usize,
    columns: usize,
    ops: Option<&PaletteOps>,
) -> HandleView {
    let preview_len = ops
        .and_then(|o| o.handle_state())
        .filter(|h| h.dragging())
        .map(|h| h.preview_len());
    let pending_resize = ops.and_then(|o| o.resize_pending());
    let handle_pos = handle_on.then(|| match (pending_resize, preview_len) {
        (Some(pending), _) => usize::from(pending.len),
        (None, Some(len)) => len,
        (None, None) => count,
    });
    // Cells of the grid: with the handle dragged, the zone has one row more than the preview
    // occupies; otherwise the entries and the handle.
    let cells = match (handle_on, preview_len) {
        (false, _) => count,
        (true, Some(preview)) => drag_zone_cells(preview, columns)
            .max(total_rows_cols(count.max(preview) + 1, columns) * columns),
        (true, None) => count.max(handle_pos.map_or(0, |pos| pos + 1)),
    };
    HandleView {
        preview_len,
        pending_resize,
        handle_pos,
        cells,
    }
}

/// Cells of the grid for the hit test of the handle at this moment: while the handle is pressed the
/// zone is the one of its preview; otherwise the one of the frame.
fn zone_cells(cells: usize, count: usize, ops: &PaletteOps, columns: usize) -> usize {
    match ops.handle_state() {
        Some(handle) => {
            let preview = handle.preview_len();
            drag_zone_cells(preview, columns)
                .max(total_rows_cols(count.max(preview) + 1, columns) * columns)
        }
        None => cells,
    }
}

/// The new length that the handle asks for under an egui point: the cell under it, in the zone.
fn handle_target(
    layout: &Layout,
    cells: usize,
    visible: usize,
    first_row: usize,
    pos: Pos2,
) -> Option<u16> {
    let columns = usize::try_from(layout.cols).unwrap_or(COLUMNS).max(1);
    let grid = Grid::with_columns(layout.n, columns, cells, visible, first_row);
    let (x, y) = layout.grid_px(pos);
    grid.len_at(x, y)
}

/// `-1` if the pointer is in the first half row of the zone, `1` in the last half row and `0`
/// elsewhere (and outside the columns of the grid): where the rows scroll on their own.
fn autoscroll_band(
    layout: &Layout,
    cells: usize,
    visible: usize,
    first_row: usize,
    pos: Pos2,
) -> i32 {
    let columns = usize::try_from(layout.cols).unwrap_or(COLUMNS).max(1);
    let n = i64::from(layout.n);
    let (x, y) = layout.grid_px(pos);
    let rows_left = total_rows_cols(cells, columns).saturating_sub(first_row);
    let shown = i64::try_from(visible.min(rows_left)).unwrap_or(0);
    let half = PITCH_U * n / 2;
    if x < 0 || x >= i64::try_from(columns).unwrap_or(0) * PITCH_U * n || shown == 0 {
        0
    } else if (0..half).contains(&y) {
        -1
    } else if (shown * PITCH_U * n - half..shown * PITCH_U * n).contains(&y) {
        1
    } else {
        0
    }
}

/// Ends the drag of the handle: releasing over a cell that asks for another length asks for it.
fn finish_handle(ops: &mut PaletteOps, output: &mut PanelOutput) {
    if let Some(HandleEnd::Resize { len }) = ops.end_handle() {
        output.op = Some(PaletteOp::ResizeTo { len });
    }
}

/// Resolves the pending text of the hexadecimal field as when it loses the focus.
fn resolve_pending(
    editor: &mut ColorEditor,
    mode: Mode,
    stored: &dyn Fn(EditTarget) -> Option<Rgba>,
    output: &mut PanelOutput,
) {
    match editor.confirm_hex_checked(mode, true, stored) {
        Resolved::Done(HexOutcome::Commit(commit)) => output.commit = Some(commit),
        Resolved::Done(HexOutcome::Invalid(error)) => {
            output.notice = Some(hex_notice(&error).to_string());
        }
        Resolved::Done(_) => {}
        Resolved::Stale(reason) => output.notice = Some(stale_notice(reason).to_string()),
    }
}

/// Ends the drag: releasing at another position asks for a `Move`.
fn finish_drag(ops: &mut PaletteOps, output: &mut PanelOutput) {
    if let Some(DragEnd::Move { from, to }) = ops.end_drag() {
        output.op = Some(PaletteOp::Move { from, to });
    }
}

/// Pressing a palette button: it either asks for an edit, or changes the pending removal, or says why
/// it cannot be done.
fn press_button(
    button: PaletteButton,
    image: &Image,
    active: u8,
    count: usize,
    ops: &mut PaletteOps,
    output: &mut PanelOutput,
) {
    let removal = ops.removal();
    let resize = ops.resize_pending();
    if let Some(reason) = blocked_reason(
        button,
        count,
        removal.is_some_and(|removal| removal.into.is_some())
            || resize.is_some_and(|pending| !pending.needs_destination()),
    ) {
        output.notice = Some(reason.to_string());
        return;
    }
    match button {
        PaletteButton::Add => output.op = Some(PaletteOp::Add),
        PaletteButton::Remove => {
            let default = image
                .palette()
                .and_then(|palette| default_destination(palette, active));
            if uses_entry(image, active) {
                ops.begin_removal(active, default);
            } else {
                // Nobody uses it: the destination does not matter (the transparent one has no default).
                let into = default.unwrap_or(u8::from(active == 0));
                output.op = Some(PaletteOp::Remove {
                    index: active,
                    into,
                });
                output.op_unused = true;
            }
        }
        PaletteButton::Confirm => {
            if let Some(removal) = removal
                && let Some(into) = removal.into
            {
                output.op = Some(PaletteOp::Remove {
                    index: removal.index,
                    into,
                });
                ops.cancel_removal();
            } else if let Some(pending) = resize {
                // The session applies it and closes the pending cut (it may still ask to confirm
                // the loss of the transparent entry).
                output.op = Some(PaletteOp::Resize {
                    len: pending.len,
                    into: pending.into,
                });
            }
        }
        PaletteButton::Cancel => {
            ops.cancel_removal();
            ops.cancel_resize();
        }
        PaletteButton::Transparent => {
            if let Some(palette) = image.palette() {
                output.op = Some(PaletteOp::SetTransparent {
                    index: transparent_toggle(palette, active),
                });
            }
        }
        PaletteButton::RotateLeft | PaletteButton::RotateRight => {
            let (start, len) = rotation_range(count, active, ops.range_end());
            let steps = if button == PaletteButton::RotateLeft {
                -1
            } else {
                1
            };
            output.op = Some(PaletteOp::Rotate { start, len, steps });
        }
    }
}

/// Swatch of the panel that corresponds to the target of the edit.
fn edited_cell(target: EditTarget) -> usize {
    match target {
        EditTarget::Entry(index) => usize::from(index),
        EditTarget::Slot(slot) => usize::from(slot).saturating_sub(1),
    }
}

fn to_color32(color: Rgba) -> Color32 {
    Color32::from_rgba_unmultiplied(color.r, color.g, color.b, color.a)
}
