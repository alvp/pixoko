//! Palette panel: grid of swatches to see and choose the active color
//! (specs/hito-2-panel-paletas.md). It does not modify the document.
//!
//! The geometry and the constants live in [`crate::palette_grid`]; here they are only translated to
//! egui points (`u` = `ui_unit(ppp)` points = `n` physical pixels) and the events are read.

use egui::{
    Color32, Event, FontFamily, FontId, Frame, Label, MouseWheelUnit, Panel, PointerButton, Pos2,
    Rect, RichText, Sense, Ui, vec2,
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
use crate::color_editor::{
    self, ButtonsView, Extras, Geometry, hex_field_id, is_escape, show_icon_row,
};
use crate::edit_layout::{
    PaletteButton, PanelLayout, block_height_closed_px, block_height_px, button_hit, panel_layout,
    panel_layout_closed,
};
use crate::palette_grid::{
    COLUMNS, GRID_X_U, GRID_Y_U, Grid, IND_BORDER_EDIT_U, IND_BORDER_U, INDICATOR_H_U,
    PANEL_WIDTH_U, PITCH_U, PanelView, RIGHT_LINE_X_U, RING_ACTIVE_U, RING_BACKGROUND_ARM_U,
    RING_BACKGROUND_U, RING_DEST_U, RING_HOVER_U, RING_RANGE_U, SEPARATOR_Y_U, Scroll,
    TEXT_WIDTH_U, TRACK_W_U, TRACK_X_U, block_parts, corner_rects, drop_mark_rect, indicator_hit,
    indicator_rects, panel_thumb, ring_rects, swatch_parts, thumb, total_rows,
    visible_rows_reserved,
};
use crate::palette_ops::{
    DragEnd, PaletteOp, PaletteOps, blocked_reason, default_destination, drag_labels,
    removal_prompt, removal_warning, rotation_range, transparent_toggle, uses_entry,
};
use crate::status::columns;
use crate::text::{font_size_points, printable, text_scale, ui_unit};
use crate::theme::{
    ACCENT, DROP_MARK, RING_ACTIVE, RING_BACKGROUND, RING_DEST, RING_HOVER, RING_RANGE, SURFACE_1,
    SURFACE_2, SURFACE_3, TEXT, TEXT_DIM,
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
    /// Top-left corner of the block of the panel, in points: the panel's minus the
    /// scroll of the whole panel when it does not fit.
    min: Pos2,
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
    /// The text the status line shows instead of its usual one, by priority: a swatch drag, a
    /// pending removal, the swatch under the cursor. `None`: the usual line.
    pub status_hint: Option<String>,
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
struct PanelRoles {
    foreground: ColorChoice,
    background: Option<ColorChoice>,
    editing: Role,
}

impl PanelRoles {
    fn legacy(choice: ColorChoice) -> PanelRoles {
        PanelRoles {
            foreground: choice,
            background: None,
            editing: Role::Foreground,
        }
    }

    /// The choice the editor edits and on which the palette buttons act.
    fn edited(&self) -> ColorChoice {
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
    mut editor: Option<&mut ColorEditor>,
    mut ops: Option<&mut PaletteOps>,
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
            let height_px = (rect.height() * ppp).round() as i64;
            let n = text_scale(ppp);
            let mut layout = Layout {
                ppp,
                n,
                rect,
                min: rect.min,
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
            let place = |offset_px: i64| {
                if !has_editor {
                    PanelLayout {
                        scrolling: false,
                        offset_px: 0,
                        max_offset_px: 0,
                        top_px: height_px,
                        icons_top_px: height_px,
                        visible_rows: visible_rows_reserved(height_px, 0, n),
                    }
                } else if shown_target.is_some() {
                    panel_layout(mode, height_px, n, offset_px)
                } else {
                    panel_layout_closed(mode, height_px, n, offset_px)
                }
            };
            let block_px = if shown_target.is_some() || !has_editor {
                block_height_px(mode, n)
            } else {
                block_height_closed_px(mode, n)
            };
            let mut placed = place(view.offset_px);
            view.offset_px = placed.offset_px;
            if !placed.scrolling {
                view.rest_px = 0;
            }
            layout.min = rect.min - vec2(0.0, placed.offset_px as f32 / ppp);
            let mut output = PanelOutput::default();
            let count = cell_count(image);
            let total = total_rows(count);
            let visible = placed.visible_rows;
            scroll.clamp(total, visible);
            let changed = view.reveal || view.last_visible.is_some_and(|last| last != visible);
            if changed && let Some(cell) = cell_of_choice(image, choice) {
                scroll.reveal(cell / COLUMNS, total, visible);
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
                top_px: placed.icons_top_px,
                mode,
            };
            let phys = |p: Pos2| ((p.x * ppp).round() as i64, (p.y * ppp).round() as i64);

            let pointer = ui
                .input(|i| i.pointer.hover_pos())
                .filter(|p| rect.contains(*p));
            let events = ui.input(|i| i.events.clone());
            let mut picked = None;
            for event in &events {
                match event {
                    Event::MouseWheel {
                        unit,
                        delta,
                        modifiers,
                        ..
                    } if !modifiers.command && delta.y != 0.0 && pointer.is_some() => {
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
                                && (0..PANEL_WIDTH_U * i64::from(layout.n)).contains(&bx)
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
                        let grid = Grid::new(layout.n, count, visible, scroll.first_row);
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
                            let removing = ops.as_deref().is_some_and(|o| o.removal().is_some());
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
                        let grid = Grid::new(layout.n, count, visible, scroll.first_row);
                        let (x, y) = layout.grid_px(*pos);
                        if let Some(cell) = grid.index_at(x, y) {
                            let cell_u8 = u8::try_from(cell).ok();
                            match (ops.as_deref_mut(), cell_u8) {
                                (Some(ops), Some(cell_u8)) if ops_on => {
                                    if ops.removal().is_some() {
                                        // With a pending removal, pressing is choosing the destination.
                                        ops.set_destination(cell_u8, count);
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
                        }
                    }
                    Event::WindowFocused(false) => {
                        if let Some(ops) = ops.as_deref_mut() {
                            ops.cancel_drag();
                        }
                    }
                    event if is_escape(event) && !text_focused => {
                        // Priority of `Escape`: the text field, the drawing gesture or the
                        // slider (they do not get here), the swatch drag, the pending removal and, last, closing the editor.
                        let mut claimed = false;
                        if let Some(ops) = ops.as_deref_mut() {
                            if ops.drag_state().is_some() {
                                ops.cancel_drag();
                                claimed = true;
                            } else if ops.removal().is_some() && enabled && !busy_editing {
                                ops.cancel_removal();
                                claimed = true;
                            }
                        }
                        // Last in the chain: nothing else claimed `Escape`, so it closes the editor.
                        if !claimed && enabled && !busy_editing && shown_target.is_some() {
                            output.close_editor = true;
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
            placed = place(view.offset_px);
            view.offset_px = placed.offset_px;
            layout.min = rect.min - vec2(0.0, placed.offset_px as f32 / ppp);

            let mut preview = None;
            if let Some(editor) = editor.as_mut() {
                match shown_target {
                    Some(target) => {
                        let geometry = Geometry {
                            ppp,
                            n: layout.n,
                            panel_min: rect.min,
                            panel_rect: rect,
                            top_px: placed.top_px,
                            mode,
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
                        let transparent = matches!(target, EditTarget::Entry(index)
                            if image.palette().and_then(|p| p.transparent_index()) == Some(index));
                        let removal = ops.as_deref().and_then(|o| o.removal());
                        let extras = Extras {
                            prompt: removal.map(|r| {
                                let warn = image
                                    .palette()
                                    .and_then(|p| removal_warning(p, r.index, r.into));
                                removal_prompt(r.index, r.into, warn)
                            }),
                        };
                        let shown = color_editor::show(
                            ui,
                            &geometry,
                            editor,
                            target,
                            transparent,
                            &events,
                            &stored,
                            &extras,
                        );
                        output.commit = shown.commit.or(output.commit);
                        output.notice = shown.notice.or(output.notice.take());
                    }
                    None => {
                        // The field disappears (editor closed or nothing to edit): its pending
                        // text is resolved as when it loses the focus (egui does not tell about a
                        // field that is no longer painted).
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
                let icons_geo = Geometry {
                    top_px: placed.icons_top_px,
                    ..pre_geometry
                };
                show_icon_row(
                    ui,
                    &icons_geo,
                    ButtonsView {
                        removing: removal.is_some(),
                        len: count,
                        has_destination: removal.is_some_and(|r| r.into.is_some()),
                        transparent_active: matches!(
                            (image.palette(), active_entry),
                            (Some(palette), Some(active))
                                if palette.transparent_index() == Some(active)
                        ),
                        icons: view.icons,
                    },
                );
            }

            let grid = Grid::new(layout.n, count, visible, scroll.first_row);
            let hover = pointer.and_then(|p| {
                let (x, y) = layout.grid_px(p);
                grid.index_at(x, y)
            });
            let panel_bar = placed
                .scrolling
                .then(|| panel_thumb(placed.offset_px, height_px, block_px, n));
            if let Some(ops) = ops.as_deref_mut() {
                ops.set_text_focus(ctx.text_edit_focused());
            }
            let marks = Marks::new(ops.as_deref(), active_entry, ops_on);
            let edit_mark = shown_target.map(|_| roles.editing);
            paint(
                ui, &layout, image, slots, roles, &grid, hover, preview, scroll, total, visible,
                panel_bar, &marks, edit_mark,
            );

            // The information of the swatch: what the two fixed lines used to paint now goes to
            // the tooltip and, by priority, to the status line.
            let active = cell_of_choice(image, roles.foreground);
            let background = roles
                .background
                .and_then(|choice| cell_of_choice(image, choice));
            let info = swatch_info(
                image, slots, &roles, hover, active, background, &marks, ppp, n,
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
            let status_hint = if marks.drag_text.is_some() {
                Some(joined(&info))
            } else if let Some(lines) = &removal_text {
                Some(joined(lines))
            } else if hover.is_some() {
                Some(joined(&info))
            } else {
                None
            };
            ctx.data_mut(|d| {
                d.insert_temp(
                    panel_info_id(),
                    PanelInfo {
                        lines: info.clone(),
                        status_hint,
                    },
                );
            });

            // Tooltips: the swatch under the cursor (unless something is being dragged or removed)
            // and each rectangle of the indicator.
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
            let calm = marks.drag_text.is_none() && removal_text.is_none() && !busy_editing;
            if hover.is_some() && calm {
                let pitch = PITCH_U * i64::from(n);
                let cells = layout
                    .grid_rect(ScreenRect {
                        x: 0,
                        y: 0,
                        width: COLUMNS as i64 * pitch,
                        height: visible as i64 * pitch,
                    })
                    .intersect(rect);
                if cells.is_positive() {
                    let response =
                        ui.interact(cells, ui.unique_id().with("swatch-tooltip"), Sense::hover());
                    response.on_hover_ui(|ui| tip(ui, &[&info[0], &info[1]]));
                }
            }
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
        })
        .inner
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
) -> [String; 2] {
    if let Some(lines) = &marks.drag_text {
        return lines.clone();
    }
    let shown = hover.or(active);
    let line1 = shown
        .and_then(|cell| cell_label_with(image, cell, slots))
        .unwrap_or_default();
    let cols = columns((TEXT_WIDTH_U * i64::from(n)) as f32, ppp);
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
        layout.u_rect(0, SEPARATOR_Y_U, PANEL_WIDTH_U, 1),
        0.0,
        SURFACE_3,
    );
    // Two 1 u lines: the left one separates the panel from the canvas and the right one, from the
    // toolbox (the left one falls in the 4 u margin of the grid).
    for x_u in [0, RIGHT_LINE_X_U] {
        let line = layout.u_rect(x_u, 0, 1, 0);
        painter.rect_filled(
            Rect::from_min_size(
                Pos2::new(line.min.x, layout.rect.min.y),
                vec2(line.width(), layout.rect.height()),
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
            let x = layout.u_rect(TRACK_X_U, 0, 0, 0).min.x;
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
            layout.u_rect(TRACK_X_U, GRID_Y_U, TRACK_W_U, track_h),
            0.0,
            SURFACE_2,
        );
        let top = layout.u_rect(TRACK_X_U, GRID_Y_U, TRACK_W_U, 0).min;
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
            dest: ops.removal().and_then(|r| r.into).map(usize::from),
            drop: drag
                .and_then(|d| d.preview())
                .map(|(from, to)| (usize::from(from), usize::from(to))),
            drag_text: drag.map(|d| drag_labels(d.from(), d.target())),
        }
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
    let grid = Grid::new(layout.n, count, visible, first_row);
    let (x, y) = layout.grid_px(pos);
    grid.drop_index(x, y)
        .and_then(|index| u8::try_from(index).ok())
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
    if let Some(reason) = blocked_reason(
        button,
        count,
        removal.is_some_and(|removal| removal.into.is_some()),
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
            }
        }
        PaletteButton::Cancel => ops.cancel_removal(),
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
