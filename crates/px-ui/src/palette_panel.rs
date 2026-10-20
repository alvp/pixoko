//! Panel de paletas: rejilla de muestras para ver y elegir el color activo
//! (specs/hito-2-panel-paletas.md). No modifica el documento.
//!
//! La geometría y las constantes viven en [`crate::palette_grid`]; aquí solo se traducen a
//! puntos de egui (`u` = `ui_unit(ppp)` puntos = `n` píxeles físicos) y se leen los eventos.

use egui::{
    Align2, Color32, Event, FontFamily, FontId, Frame, MouseWheelUnit, Panel, PointerButton, Pos2,
    Rect, Sense, Ui, vec2,
};
use px_core::{Image, Mode, Rgba};

use crate::color::{
    ColorChoice, SlotColors, cell_color_with, cell_count, cell_label_with, cell_of_choice,
    choice_for_cell, info_flags_alpha0, is_alpha0_cell_with, is_transparent_cell_with,
};
use crate::color_edit::{
    ColorEditor, Commit, EditTarget, HexOutcome, Resolved, hex_notice, stale_notice,
};
use crate::color_editor::{self, ButtonsView, Extras, Geometry, hex_field_id, is_escape};
use crate::edit_layout::{PaletteButton, PanelLayout, block_height_px, button_hit, panel_layout};
use crate::palette_grid::{
    COLUMNS, GRID_X_U, GRID_Y_U, Grid, LINE_H_U, LINE1_Y_U, LINE2_Y_U, PANEL_WIDTH_U, PITCH_U,
    PanelView, RIGHT_LINE_X_U, RING_ACTIVE_U, RING_DEST_U, RING_HOVER_U, RING_RANGE_U,
    SEPARATOR_Y_U, Scroll, TEXT_WIDTH_U, TEXT_X_U, TRACK_W_U, TRACK_X_U, drop_mark_rect,
    panel_thumb, ring_rects, swatch_parts, thumb, total_rows, visible_rows_reserved,
};
use crate::palette_ops::{
    DragEnd, PaletteOp, PaletteOps, blocked_reason, default_destination, drag_labels,
    removal_prompt, removal_warning, rotation_range, transparent_button_label, transparent_toggle,
    uses_entry,
};
use crate::status::{bar_text, columns};
use crate::text::{font_size_points, text_scale, ui_unit};
use crate::theme::{
    DROP_MARK, RING_ACTIVE, RING_DEST, RING_HOVER, RING_RANGE, SURFACE_1, SURFACE_2, SURFACE_3,
    TEXT, TEXT_DIM,
};
use crate::view::ScreenRect;

/// Filas que desplaza una «línea» de la rueda.
const WHEEL_LINE_ROWS: f32 = 3.0;

/// Datos de un fotograma: escala y origen de la rejilla en el panel.
struct Layout {
    ppp: f32,
    n: u32,
    /// El panel, en puntos.
    rect: Rect,
    /// Esquina superior izquierda del bloque del panel, en puntos: la del panel menos el
    /// desplazamiento de todo el panel cuando no cabe.
    min: Pos2,
}

impl Layout {
    /// Un punto de egui sobre el panel, en píxeles físicos desde el origen de la rejilla.
    fn grid_px(&self, p: Pos2) -> (i64, i64) {
        let u = ui_unit(self.ppp);
        let origin = self.min + vec2(GRID_X_U as f32 * u, GRID_Y_U as f32 * u);
        (
            ((p.x - origin.x) * self.ppp).round() as i64,
            ((p.y - origin.y) * self.ppp).round() as i64,
        )
    }

    /// Un rectángulo en píxeles físicos desde el origen de la rejilla, en puntos de egui.
    fn grid_rect(&self, r: ScreenRect) -> Rect {
        let u = ui_unit(self.ppp);
        let origin = self.min + vec2(GRID_X_U as f32 * u, GRID_Y_U as f32 * u);
        Rect::from_min_size(
            origin + vec2(r.x as f32 / self.ppp, r.y as f32 / self.ppp),
            vec2(r.width as f32 / self.ppp, r.height as f32 / self.ppp),
        )
    }

    /// Un rectángulo en `u` desde la esquina del panel, en puntos de egui.
    fn u_rect(&self, x: i64, y: i64, width: i64, height: i64) -> Rect {
        let u = ui_unit(self.ppp);
        Rect::from_min_size(
            self.min + vec2(x as f32 * u, y as f32 * u),
            vec2(width as f32 * u, height as f32 * u),
        )
    }
}

/// Lo que pidió el panel con el editor en un fotograma.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PanelOutput {
    /// Elección pedida por un clic en una muestra.
    pub choice: Option<ColorChoice>,
    /// Color a aplicar (soltar un deslizador o confirmar el hexadecimal).
    pub commit: Option<Commit>,
    /// Aviso de 5 s para la barra de estado (hexadecimal inválido al perder el foco).
    pub notice: Option<String>,
    /// Edición de la paleta pedida (botones o soltar un arrastre de muestra); la aplica la sesión.
    pub op: Option<PaletteOp>,
    /// Con `op` = `Remove`: ningún píxel usaba la entrada (se quita sin pedir destino).
    pub op_unused: bool,
}

/// Pinta el panel a la izquierda de `ui` y devuelve la elección que pidió un clic (botón
/// izquierdo pulsado sobre una muestra). `enabled` falso (gesto en curso): se ve y se
/// desplaza, pero no elige. Hay que llamarla **después** de la barra de estado y **antes** del
/// lienzo, que se queda con el rectángulo que sobra. Sin editor de color: [`show_editing`] lo
/// añade.
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
        choice,
        scroll,
        &mut PanelView::default(),
        enabled,
        &slots,
        None,
        None,
    )
    .choice
}

/// Como [`show`], con el editor de color anclado al fondo del panel (specs/hito-2-editar-color.md).
/// La rejilla usa la altura que queda. El editor se pone al día antes (`ColorEditor::sync`); el
/// panel no toca el documento: devuelve el color a aplicar. Sin memoria entre fotogramas: ni
/// desplazamiento del panel ni «mostrar la elegida» (para eso, [`show_editing_view`]).
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

/// Como [`show_editing`], con el estado del panel entre fotogramas (`view`). Si el panel no
/// cabe (`edit_layout::panel_layout`), el bloque `[texto · una fila · editor]` se desplaza
/// entero con la rueda y una barra visible; la elegida se muestra al cambiar el número de filas
/// visibles o si `view.reveal`, nunca por la rueda (specs/hito-2-panel-ventana-baja.md).
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

/// Como [`show_editing_view`], con los botones de añadir, quitar y rotar, el arrastre de muestras,
/// el rango y la quita pendiente de las indexadas (`ops`, specs/hito-2-paleta-operaciones.md).
/// Las ediciones salen en `PanelOutput::op`; el panel no toca el documento.
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
        choice,
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
    choice: ColorChoice,
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
    Panel::left("paleta")
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
            let target = editor.as_ref().and_then(|_| EditTarget::of(image, choice));
            // Reparto vertical: con destino, el del editor (que puede desplazar todo el panel);
            // sin él, la rejilla usa todo el alto.
            let place = |offset_px: i64| match target {
                Some(_) => panel_layout(mode, height_px, n, offset_px),
                None => PanelLayout {
                    scrolling: false,
                    offset_px: 0,
                    max_offset_px: 0,
                    top_px: height_px,
                    visible_rows: visible_rows_reserved(height_px, 0, n),
                },
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
            // Operaciones de paleta: solo indexadas, con editor y con una entrada activa.
            let active_entry = match choice {
                ColorChoice::Entry(index) => Some(index),
                ColorChoice::Slot(_) => None,
            };
            let ops_on = mode == Mode::Indexed
                && target.is_some()
                && ops.is_some()
                && active_entry.is_some();
            let text_focused =
                ctx.text_edit_focused() || ops.as_deref().is_some_and(|ops| ops.text_focus());
            let stored = |target: EditTarget| target.stored(image, slots);
            let primary_down = ui.input(|i| i.pointer.primary_down());
            let pre_geometry = Geometry {
                ppp,
                n: layout.n,
                panel_min: rect.min,
                panel_rect: rect,
                top_px: placed.top_px,
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
                        button: PointerButton::Primary,
                        pressed: true,
                        modifiers,
                    } if enabled && !busy_editing && rect.contains(*pos) => {
                        if ops_on {
                            let removing = ops.as_deref().is_some_and(|o| o.removal().is_some());
                            let (rx, ry) = pre_geometry.rel_px(*pos);
                            if let Some(button) = button_hit(mode, removing, rx, ry, layout.n) {
                                // Pulsar un botón es salir del campo: su texto pendiente se
                                // resuelve antes de la operación (el `Commit` llega primero).
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
                                        // Con una quita pendiente, pulsar es elegir el destino.
                                        ops.set_destination(cell_u8, count);
                                    } else if modifiers.shift {
                                        // `Mayús` + clic: fija el rango, sin elegir ni arrastrar.
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
                        // Prioridad de `Escape`: el campo de texto, el gesto de dibujo o el
                        // deslizador (no llegan aquí), el arrastre de muestra y la quita pendiente.
                        if let Some(ops) = ops.as_deref_mut() {
                            if ops.drag_state().is_some() {
                                ops.cancel_drag();
                            } else if ops.removal().is_some() && enabled && !busy_editing {
                                ops.cancel_removal();
                            }
                        }
                    }
                    _ => {}
                }
            }
            if let Some(ops) = ops.as_deref_mut()
                && ops.drag_state().is_some()
            {
                // Tras la rueda o un cambio de diseño el destino puede haber cambiado; y si
                // egui ya no ve el botón (se perdió el soltar), es un soltar.
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
            placed = place(view.offset_px);
            view.offset_px = placed.offset_px;
            layout.min = rect.min - vec2(0.0, placed.offset_px as f32 / ppp);

            let mut preview = None;
            if let Some(editor) = editor.as_mut() {
                match target {
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
                            buttons: ops_on.then_some(ButtonsView {
                                removing: removal.is_some(),
                                len: count,
                                has_destination: removal.is_some_and(|r| r.into.is_some()),
                                transparent_label: match (image.palette(), active_entry) {
                                    (Some(palette), Some(active)) => {
                                        transparent_button_label(palette, active)
                                    }
                                    _ => "Fijar transparente",
                                },
                            }),
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
                        // El campo desaparece: su texto pendiente se resuelve como al perder el
                        // foco (egui no avisa de un campo que ya no se pinta).
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

            let grid = Grid::new(layout.n, count, visible, scroll.first_row);
            let hover = pointer.and_then(|p| {
                let (x, y) = layout.grid_px(p);
                grid.index_at(x, y)
            });
            let panel_bar = placed
                .scrolling
                .then(|| panel_thumb(placed.offset_px, height_px, block_height_px(mode, n), n));
            if let Some(ops) = ops.as_deref_mut() {
                ops.set_text_focus(ctx.text_edit_focused());
            }
            let marks = Marks::new(ops.as_deref(), active_entry, ops_on);
            paint(
                ui, &layout, image, slots, choice, &grid, hover, preview, scroll, total, visible,
                panel_bar, &marks,
            );
            output
        })
        .inner
}

#[allow(clippy::too_many_arguments)]
fn paint(
    ui: &Ui,
    layout: &Layout,
    image: &Image,
    slots: &SlotColors,
    choice: ColorChoice,
    grid: &Grid,
    hover: Option<usize>,
    preview: Option<(EditTarget, Rgba)>,
    scroll: &Scroll,
    total: usize,
    visible: usize,
    panel_bar: Option<Option<(i64, i64)>>,
    marks: &Marks,
) {
    let ctx = ui.ctx();
    let (n, ppp) = (layout.n, layout.ppp);
    let n_px = i64::from(n);
    let painter = ui.painter_at(layout.rect);
    let active = cell_of_choice(image, choice);

    // Trazos: bajo el texto y a la derecha.
    painter.rect_filled(
        layout.u_rect(0, SEPARATOR_Y_U, PANEL_WIDTH_U, 1),
        0.0,
        SURFACE_3,
    );
    let line = layout.u_rect(RIGHT_LINE_X_U, 0, 1, 0);
    painter.rect_filled(
        Rect::from_min_size(
            Pos2::new(line.min.x, layout.rect.min.y),
            vec2(line.width(), layout.rect.height()),
        ),
        0.0,
        SURFACE_3,
    );

    // Información: la muestra bajo el cursor o, si no hay, la elegida.
    let shown = hover.or(active);
    let mut line1 = shown
        .and_then(|cell| cell_label_with(image, cell, slots))
        .unwrap_or_default();
    let mut line2 = shown
        .map(|cell| {
            info_flags_alpha0(
                hover.is_some(),
                Some(cell) == active,
                marks.in_range(cell),
                is_transparent_cell_with(image, cell, slots),
                is_alpha0_cell_with(image, cell, slots),
            )
        })
        .unwrap_or_default();
    // Durante un arrastre, el panel dice qué se mueve y cómo cancelarlo.
    if let Some([first, second]) = &marks.drag_text {
        line1 = first.clone();
        line2 = second.clone();
    }
    let cols = columns((TEXT_WIDTH_U * n_px) as f32, ppp);
    let font = FontId::new(font_size_points(ppp), FontFamily::Proportional);
    for (y, text, color) in [(LINE1_Y_U, line1, TEXT), (LINE2_Y_U, line2, TEXT_DIM)] {
        let text = bar_text(ctx, &text, false, cols);
        let top_left = layout.u_rect(TEXT_X_U, y, 0, LINE_H_U).min;
        painter.text(top_left, Align2::LEFT_TOP, text, font.clone(), color);
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
            // La muestra editada se pinta con el color provisional (resuelto: la entrada
            // transparente sigue con alfa 0).
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

    // Anillos: los del rango de rotación, el del cursor (si no es la elegida), el de la elegida y
    // el destino de una quita pendiente.
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
            hover.filter(|cell| Some(*cell) != active),
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

    if let Some(rect) = marks.dest.and_then(|cell| grid.cell_rect(cell)) {
        for ring in ring_rects(rect, RING_DEST_U * n_px) {
            painter.rect_filled(layout.grid_rect(ring), 0.0, RING_DEST);
        }
    }
    // Barra del destino de un arrastre.
    if let Some((from, to)) = marks.drop
        && let Some(rect) = grid.cell_rect(to)
    {
        let bar = drop_mark_rect(rect, to > from, n);
        painter.rect_filled(layout.grid_rect(bar), 0.0, DROP_MARK);
    }

    // Pista e indicador, solo si hace falta desplazar: los de todo el panel si no cabe; si no,
    // los de la rejilla.
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

/// Lo que las operaciones de paleta añaden al pintado de la rejilla.
#[derive(Default)]
struct Marks {
    /// Muestras del rango de rotación (extremos incluidos).
    range: Option<(usize, usize)>,
    /// Destino elegido de la quita pendiente.
    dest: Option<usize>,
    /// Arrastre con destino: `(from, to)`.
    drop: Option<(usize, usize)>,
    /// Líneas de información mientras se arrastra.
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

/// Destino de un arrastre bajo un punto de egui: el índice final de la entrada movida.
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

/// Resuelve el texto pendiente del campo hexadecimal como al perder el foco.
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

/// Termina el arrastre: soltar en otra posición pide un `Move`.
fn finish_drag(ops: &mut PaletteOps, output: &mut PanelOutput) {
    if let Some(DragEnd::Move { from, to }) = ops.end_drag() {
        output.op = Some(PaletteOp::Move { from, to });
    }
}

/// Pulsar un botón de la paleta: o pide una edición, o cambia la quita pendiente, o avisa de por
/// qué no se puede.
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
                // Nadie la usa: el destino da igual (la transparente no tiene por defecto).
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

/// Muestra del panel que corresponde al destino de la edición.
fn edited_cell(target: EditTarget) -> usize {
    match target {
        EditTarget::Entry(index) => usize::from(index),
        EditTarget::Slot(slot) => usize::from(slot).saturating_sub(1),
    }
}

fn to_color32(color: Rgba) -> Color32 {
    Color32::from_rgba_unmultiplied(color.r, color.g, color.b, color.a)
}
