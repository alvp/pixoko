//! El panel de capas pintado con egui (specs/hito-3-capas-interfaz.md, decisiones 1 a 4): un
//! `egui::Area` de exactamente el rectángulo del panel, así que fuera de él todo llega al lienzo.
//! No toca el documento: devuelve [`PanelEvent`] y lo aplica quien llama. La geometría, la lista y
//! las reglas de los campos están en `layers_panel` (sin egui).

use egui::{
    Align2, Area, Context, Event, FontFamily, FontId, Frame, Id, Key, Margin, MouseWheelUnit,
    Order, PointerButton, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, TextEdit, Ui, vec2,
};
use px_core::{BlendMode, Document, LayerId, Mode};

use crate::actions::{AppAction, layer_enabled};
use crate::help_texts::{help_entry, help_key};
use crate::icon_button::{look, paint_icon, paint_text};
use crate::icons::{Icon, IconSet};
use crate::layers_panel::{
    BLEND_LABELS, LabelRow, NAME_EDIT_CHARS, NameCommit, PANEL_H_U, PANEL_W_U, PanelButton,
    PanelState, RectU, VISIBLE_ROWS, blend_label, blend_u, button_u, close_u, commit_name, count_u,
    eye_u, label_pos_u, layer_nav, lock_u, max_scroll, memory_line, memory_u, name_field_u, name_u,
    opacity_field_u, opacity_slider_u, row_u, scroll_for, scrollbar_u, title_u, visible_rows,
};
use crate::number::parse_number;
use crate::text::{font_size_points, printable, text_scale, ui_unit};
use crate::theme::{
    ACCENT, SURFACE_0, SURFACE_1, SURFACE_2, SURFACE_3, TEXT, TEXT_DIM, TEXT_DISABLED, WARNING,
};

/// Lo que el panel le pide a quien lo muestra.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PanelEvent {
    /// La `x` de la barra de título.
    Close,
    /// Clic en el nombre de una fila: es la capa activa.
    Select(LayerId),
    ToggleVisible(LayerId),
    ToggleLocked(LayerId),
    /// Uno de los cinco botones (Nueva, Duplicar, Quitar, Subir, Bajar).
    Button(PanelButton),
    /// Doble clic en el nombre de una fila: `Renombrar`.
    StartRename(LayerId),
    /// El campo del nombre confirmado con un nombre válido y distinto.
    Rename(LayerId, String),
    /// Opacidad soltada (el deslizador, al soltar) o confirmada en el campo.
    Opacity(LayerId, u8),
    Blend(LayerId, BlendMode),
    /// Un aviso (por ejemplo, un nombre vacío rechazado).
    Notice(String),
}

/// Lo que el panel necesita para pintarse.
pub struct PanelView<'a> {
    pub doc: &'a Document,
    pub active: LayerId,
    /// `false` con un diálogo modal (o una conversión o un guardado en espera) abierto: el panel se
    /// pinta pero no responde a nada.
    pub enabled: bool,
    /// Las texturas de los iconos (vacío: los botones se pintan con un rótulo corto).
    pub icons: &'a IconSet,
}

fn fallback_label(icon: Icon) -> &'static str {
    match icon {
        Icon::Eye => "oj",
        Icon::EyeClosed => "oc",
        Icon::Lock => "ca",
        Icon::LockOpen => "ab",
        Icon::Duplicate => "2x",
        Icon::Up => "^",
        Icon::Down => "v",
        Icon::Add => "+",
        _ => "x",
    }
}

fn button_action(button: PanelButton) -> AppAction {
    match button {
        PanelButton::New => AppAction::NewLayer,
        PanelButton::Duplicate => AppAction::DuplicateLayer,
        PanelButton::Remove => AppAction::RemoveLayer,
        PanelButton::Up => AppAction::LayerUp,
        PanelButton::Down => AppAction::LayerDown,
    }
}

fn button_icon(button: PanelButton) -> Icon {
    match button {
        PanelButton::New => Icon::Add,
        PanelButton::Duplicate => Icon::Duplicate,
        PanelButton::Remove => Icon::Remove,
        PanelButton::Up => Icon::Up,
        PanelButton::Down => Icon::Down,
    }
}

/// Lo que se necesita para pintar y dar sentido a las posiciones en `u`.
struct Frame2<'a> {
    ctx: &'a Context,
    origin: Pos2,
    u: f32,
    font: FontId,
    enabled: bool,
}

impl Frame2<'_> {
    fn rect(&self, r: RectU) -> Rect {
        Rect::from_min_size(
            self.origin + vec2(r.x as f32 * self.u, r.y as f32 * self.u),
            vec2(r.w as f32 * self.u, r.h as f32 * self.u),
        )
    }

    fn sense(&self, sense: Sense) -> Sense {
        if self.enabled { sense } else { Sense::hover() }
    }
}

/// Un botón de icono (o de rótulo corto sin textura) de la lista y de la fila de acciones.
fn icon_button(
    ui: &mut Ui,
    f: &Frame2,
    id: Id,
    rect: Rect,
    (icon, textures): (Icon, &IconSet),
    on: bool,
    tooltip: &str,
) -> bool {
    let response = ui.interact(
        rect,
        id,
        f.sense(if on { Sense::click() } else { Sense::hover() }),
    );
    let look = look(false, on, response.hovered());
    match textures.get(icon) {
        Some(texture) => paint_icon(ui.painter(), rect, f.u, Some(texture), &look),
        None => paint_text(
            ui.painter(),
            rect,
            f.u,
            rect.min + vec2(5.0 * f.u, 5.0 * f.u),
            fallback_label(icon),
            &f.font,
            &look,
        ),
    }
    let text = tooltip.to_string();
    let ctx = f.ctx.clone();
    let font = f.font.clone();
    response.clone().on_hover_ui(|ui| {
        ui.add(
            egui::Label::new(RichText::new(printable(&ctx, &text)).font(font).color(TEXT)).extend(),
        );
    });
    on && f.enabled && response.clicked()
}

fn tooltip_of(action: AppAction) -> String {
    help_key(action)
        .and_then(help_entry)
        .map_or_else(String::new, |entry| entry.name.to_string())
}

/// Pinta el panel (si está abierto) y devuelve lo que pasó en él en este fotograma.
/// `window_px` es el tamaño de la ventana en píxeles físicos.
pub fn show_layer_panel(
    ctx: &Context,
    view: &PanelView,
    state: &mut PanelState,
    window_px: (i64, i64),
) -> Vec<PanelEvent> {
    let mut events = Vec::new();
    if !state.open {
        return events;
    }
    let ppp = ctx.pixels_per_point();
    let n = text_scale(ppp);
    let u = ui_unit(ppp);
    let panel_px = state.rect(window_px, n);
    let origin = Pos2::new(panel_px.x as f32 / ppp, panel_px.y as f32 / ppp);
    let size = vec2(PANEL_W_U as f32 * u, PANEL_H_U as f32 * u);
    let f = Frame2 {
        ctx,
        origin,
        u,
        font: FontId::new(font_size_points(ppp), FontFamily::Proportional),
        enabled: view.enabled,
    };
    let doc = view.doc;
    let layers = doc.layers();
    let count = layers.len();
    // La lista lleva la activa a la vista cuando cambia, y no se pasa de los extremos.
    if state.seen_active != Some(view.active) {
        let pos = layers
            .iter()
            .position(|l| l.id() == view.active)
            .unwrap_or(0);
        state.scroll = scroll_for(count, pos, state.scroll);
        state.seen_active = Some(view.active);
    }
    state.scroll = state.scroll.min(max_scroll(count));
    let nav = layer_nav(doc, view.active);
    Area::new(Id::unique("pixoko-layers-panel"))
        .order(Order::Middle)
        .fixed_pos(origin)
        .interactable(view.enabled)
        .constrain(false)
        .fade_in(false)
        .show(ctx, |ui| {
            ui.allocate_exact_size(size, Sense::hover());
            draw_panel(ui, &f, view, state, window_px, nav, &mut events);
        });
    events
}

fn draw_panel(
    ui: &mut Ui,
    f: &Frame2,
    view: &PanelView,
    state: &mut PanelState,
    _window_px: (i64, i64),
    nav: crate::actions::LayerNav,
    events: &mut Vec<PanelEvent>,
) {
    let ppp = f.ctx.pixels_per_point();
    let n = text_scale(ppp);
    let u = f.u;
    let painter = ui.painter().clone();
    let base = Id::unique("pixoko-layers-panel-widgets");
    let whole = f.rect(RectU {
        x: 0,
        y: 0,
        w: PANEL_W_U,
        h: PANEL_H_U,
    });
    painter.rect(
        whole,
        0.0,
        SURFACE_1,
        Stroke::new(u, SURFACE_3),
        StrokeKind::Inside,
    );
    let text = |pos: Pos2, s: &str, color| {
        painter.text(
            pos,
            Align2::LEFT_TOP,
            printable(f.ctx, s),
            f.font.clone(),
            color,
        );
    };

    // Barra de título: arrastra el panel.
    let title = f.rect(title_u());
    text(title.min + vec2(4.0 * u, 1.0 * u), "Capas", TEXT);
    let drag = ui.interact(title, base.with("title"), f.sense(Sense::drag()));
    if drag.drag_started() {
        state.drag_from = state.pos_u;
    }
    if drag.dragged()
        && let Some(from) = state.drag_from
        && let (Some(press), Some(now)) = f
            .ctx
            .input(|i| (i.pointer.press_origin(), i.pointer.interact_pos()))
    {
        let dx = ((now.x - press.x) * ppp).round() as i64;
        let dy = ((now.y - press.y) * ppp).round() as i64;
        state.drag_total(from, dx, dy, n);
    }
    if drag.drag_stopped() {
        state.drag_from = None;
    }
    // Cerrar.
    let close = f.rect(close_u());
    let close_response = ui.interact(close, base.with("close"), f.sense(Sense::click()));
    paint_text(
        &painter,
        close,
        u,
        close.min + vec2(5.0 * u, 1.0 * u),
        "x",
        &f.font,
        &look(false, f.enabled, close_response.hovered()),
    );
    if f.enabled && close_response.clicked() {
        events.push(PanelEvent::Close);
    }

    // Lista.
    let rows = visible_rows(view.doc, view.active, state.scroll);
    for (i, row) in rows.iter().enumerate() {
        let eye_icon = if row.visible {
            Icon::Eye
        } else {
            Icon::EyeClosed
        };
        let lock_icon = if row.locked {
            Icon::Lock
        } else {
            Icon::LockOpen
        };
        if icon_button(
            ui,
            f,
            base.with(("eye", i)),
            f.rect(eye_u(i)),
            (eye_icon, view.icons),
            true,
            if row.visible {
                "Ocultar capa"
            } else {
                "Mostrar capa"
            },
        ) {
            events.push(PanelEvent::ToggleVisible(row.id));
        }
        if icon_button(
            ui,
            f,
            base.with(("lock", i)),
            f.rect(lock_u(i)),
            (lock_icon, view.icons),
            true,
            if row.locked {
                "Desbloquear capa"
            } else {
                "Bloquear capa"
            },
        ) {
            events.push(PanelEvent::ToggleLocked(row.id));
        }
        let rect = f.rect(name_u(i));
        let response = ui.interact(rect, base.with(("name", i)), f.sense(Sense::click()));
        let (fill, color) = if row.active {
            (ACCENT, SURFACE_0)
        } else if response.hovered() && f.enabled {
            (SURFACE_3, if row.visible { TEXT } else { TEXT_DIM })
        } else {
            (SURFACE_2, if row.visible { TEXT } else { TEXT_DIM })
        };
        painter.rect_filled(rect, 0.0, fill);
        text(rect.min + vec2(3.0 * u, 5.0 * u), &row.name, color);
        if f.enabled && response.double_clicked() {
            events.push(PanelEvent::Select(row.id));
            events.push(PanelEvent::StartRename(row.id));
        } else if f.enabled && response.clicked() {
            events.push(PanelEvent::Select(row.id));
        }
        let _ = row_u(i);
    }
    // Barra de desplazamiento y rueda.
    let track = f.rect(scrollbar_u());
    painter.rect_filled(track, 0.0, SURFACE_0);
    let count = view.doc.layers().len();
    let max = max_scroll(count);
    if max > 0 {
        let thumb_h = (track.height() * VISIBLE_ROWS as f32 / count as f32).max(8.0 * u);
        let free = track.height() - thumb_h;
        let top = track.min.y + free * state.scroll as f32 / max as f32;
        painter.rect_filled(
            Rect::from_min_size(Pos2::new(track.min.x, top), vec2(track.width(), thumb_h)),
            0.0,
            SURFACE_3,
        );
        let response = ui.interact(
            track,
            base.with("scrollbar"),
            f.sense(Sense::click_and_drag()),
        );
        if f.enabled
            && (response.dragged() || response.is_pointer_button_down_on())
            && let Some(pos) = response.interact_pointer_pos()
            && free > 0.0
        {
            let ratio = ((pos.y - track.min.y - thumb_h / 2.0) / free).clamp(0.0, 1.0);
            state.scroll = ((ratio * max as f32).round() as usize).min(max);
        }
    }
    if f.enabled {
        let list = Rect::from_min_max(f.rect(row_u(0)).min, track.max);
        let row_h = 25.0 * u;
        f.ctx.input(|input| {
            let hover = input.pointer.hover_pos();
            for event in &input.events {
                if let Event::MouseWheel { unit, delta, .. } = event
                    && hover.is_some_and(|p| list.contains(p))
                {
                    let rows = match unit {
                        MouseWheelUnit::Line | MouseWheelUnit::Page => -delta.y * 3.0,
                        MouseWheelUnit::Point => -delta.y / row_h,
                    };
                    state.scroll_rows(rows.round() as i64, count);
                }
            }
        });
    }

    // Botones de acción y contador.
    for button in PanelButton::ALL {
        let action = button_action(button);
        let on = f.enabled && layer_enabled(action, &nav);
        if icon_button(
            ui,
            f,
            base.with(("button", button as u8)),
            f.rect(button_u(button)),
            (button_icon(button), view.icons),
            on,
            &tooltip_of(action),
        ) {
            events.push(PanelEvent::Button(button));
        }
    }
    let counter = f.rect(count_u());
    painter.text(
        counter.right_top() + vec2(-u, 5.0 * u),
        Align2::RIGHT_TOP,
        printable(f.ctx, &format!("{count} capas")),
        f.font.clone(),
        TEXT_DIM,
    );

    // Opciones de la capa activa.
    options(ui, f, view, state, base, events);

    // Pie de memoria.
    let (line, warn) = memory_line(view.doc.materialized_bytes(), view.doc.cells_limit());
    text(
        f.rect(memory_u()).min + vec2(0.0, 1.0 * u),
        &line,
        if warn { WARNING } else { TEXT_DIM },
    );
}

fn options(
    ui: &mut Ui,
    f: &Frame2,
    view: &PanelView,
    state: &mut PanelState,
    base: Id,
    events: &mut Vec<PanelEvent>,
) {
    let u = f.u;
    let painter = ui.painter().clone();
    let Some(layer) = view.doc.layer(view.active) else {
        return;
    };
    let indexed = view.doc.mode() == Mode::Indexed;
    let label = |row: LabelRow, s: &str, color| {
        let (x, y) = label_pos_u(row);
        painter.text(
            f.origin + vec2(x as f32 * u, (y + 1) as f32 * u),
            Align2::LEFT_TOP,
            printable(f.ctx, s),
            f.font.clone(),
            color,
        );
    };
    let dim = |on: bool| if on { TEXT_DIM } else { TEXT_DISABLED };
    label(LabelRow::Name, "Nombre", TEXT_DIM);
    label(LabelRow::Opacity, "Opacidad", dim(!indexed));
    label(LabelRow::Blend, "Mezcla", dim(!indexed));

    // Nombre.
    let field = f.rect(name_field_u());
    let id = base.with("name-field");
    painter.rect(
        field,
        0.0,
        SURFACE_0,
        Stroke::new(u, SURFACE_3),
        StrokeKind::Inside,
    );
    if state.focus_name && f.enabled {
        f.ctx.memory_mut(|m| m.request_focus(id));
        state.focus_name = false;
    }
    let focused = f.ctx.memory(|m| m.has_focus(id));
    if state.name_for != Some(layer.id()) || (!focused && state.name != layer.name()) {
        state.name = layer.name().to_string();
        state.name_for = Some(layer.id());
    }
    let inner = Rect::from_min_size(
        field.min + vec2(3.0 * u, 1.0 * u),
        vec2(field.width() - 6.0 * u, 14.0 * u),
    );
    if f.enabled {
        let response = ui.put(
            inner,
            TextEdit::singleline(&mut state.name)
                .id(id)
                .font(f.font.clone())
                .text_color(TEXT)
                .frame(Frame::NONE)
                .margin(Margin::ZERO)
                .desired_width(inner.width())
                .char_limit(NAME_EDIT_CHARS)
                .clip_text(true),
        );
        if response.lost_focus() {
            if f.ctx.input(|i| i.key_pressed(Key::Escape)) {
                state.name = layer.name().to_string();
            } else if state.name != layer.name() {
                match commit_name(&state.name, layer.name()) {
                    NameCommit::Rename(name) => events.push(PanelEvent::Rename(layer.id(), name)),
                    NameCommit::Empty => {
                        events.push(PanelEvent::Notice(
                            "el nombre de la capa no puede estar vacío".to_string(),
                        ));
                        state.name = layer.name().to_string();
                    }
                    NameCommit::Unchanged => {}
                }
            }
        }
    } else {
        painter.text(
            inner.min,
            Align2::LEFT_TOP,
            printable(f.ctx, &state.name),
            f.font.clone(),
            TEXT_DIM,
        );
    }

    // Opacidad: deslizador (se aplica al soltar) y campo numérico.
    let on = f.enabled && !indexed;
    let slider = f.rect(opacity_slider_u());
    let shown = state.opacity_drag.unwrap_or_else(|| layer.opacity());
    let track = Rect::from_min_size(
        slider.min + vec2(0.0, 6.0 * u),
        vec2(slider.width(), 4.0 * u),
    );
    painter.rect_filled(track, 0.0, SURFACE_0);
    let fill_w = track.width() * f32::from(shown) / 255.0;
    painter.rect_filled(
        Rect::from_min_size(track.min, vec2(fill_w, track.height())),
        0.0,
        if on { ACCENT } else { TEXT_DISABLED },
    );
    let response = ui.interact(
        slider,
        base.with("opacity"),
        if on {
            Sense::click_and_drag()
        } else {
            Sense::hover()
        },
    );
    if on && response.is_pointer_button_down_on() {
        if let Some(pos) = response.interact_pointer_pos() {
            let ratio = ((pos.x - slider.min.x) / slider.width()).clamp(0.0, 1.0);
            state.opacity_drag = Some((ratio * 255.0).round() as u8);
        }
    } else if let Some(value) = state.opacity_drag.take()
        && on
        && value != layer.opacity()
    {
        events.push(PanelEvent::Opacity(layer.id(), value));
    }
    let number = f.rect(opacity_field_u());
    let nid = base.with("opacity-field");
    painter.rect(
        number,
        0.0,
        SURFACE_0,
        Stroke::new(u, SURFACE_3),
        StrokeKind::Inside,
    );
    let inner = Rect::from_min_size(
        number.min + vec2(3.0 * u, 1.0 * u),
        vec2(number.width() - 6.0 * u, 14.0 * u),
    );
    if on {
        let mut buffer = state
            .opacity_text
            .clone()
            .unwrap_or_else(|| shown.to_string());
        let response = ui.put(
            inner,
            TextEdit::singleline(&mut buffer)
                .id(nid)
                .font(f.font.clone())
                .text_color(TEXT)
                .frame(Frame::NONE)
                .margin(Margin::ZERO)
                .desired_width(inner.width())
                .char_limit(3)
                .clip_text(true),
        );
        if response.changed() {
            state.opacity_text = Some(buffer);
        }
        if response.lost_focus()
            && let Some(text) = state.opacity_text.take()
            && !f.ctx.input(|i| i.key_pressed(Key::Escape))
            && let Ok(value) = parse_number(&text, 0, 255)
            && value as u8 != layer.opacity()
        {
            events.push(PanelEvent::Opacity(layer.id(), value as u8));
        }
    } else {
        painter.text(
            inner.min,
            Align2::LEFT_TOP,
            printable(f.ctx, &shown.to_string()),
            f.font.clone(),
            TEXT_DISABLED,
        );
    }

    // Mezcla: un botón con el modo y un menú de los ocho.
    let blend = f.rect(blend_u());
    let response = ui.interact(
        blend,
        base.with("blend"),
        f.sense(if on { Sense::click() } else { Sense::hover() }),
    );
    paint_text(
        &painter,
        blend,
        u,
        blend.min + vec2(4.0 * u, 1.0 * u),
        blend_label(layer.blend()),
        &f.font,
        &look(false, on, response.hovered()),
    );
    if on {
        egui::Popup::menu(&response).show(|ui| {
            for (mode, name) in BLEND_LABELS {
                if ui
                    .selectable_label(layer.blend() == mode, printable(f.ctx, name))
                    .clicked()
                {
                    if mode != layer.blend() {
                        events.push(PanelEvent::Blend(layer.id(), mode));
                    }
                    ui.close();
                }
            }
        });
    } else if indexed && f.enabled {
        let (ctx, font) = (f.ctx.clone(), f.font.clone());
        response.on_hover_ui(|ui| {
            ui.add(
                egui::Label::new(
                    RichText::new(printable(&ctx, "convierte a RGBA"))
                        .font(font)
                        .color(TEXT),
                )
                .extend(),
            );
        });
    }
    let _ = PointerButton::Primary;
}
