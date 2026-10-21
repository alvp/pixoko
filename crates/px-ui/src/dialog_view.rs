//! Pintado de los diálogos modales con egui (specs/hito-2-dialogos-modales.md): no decide nada, lee
//! el estado puro (`Dialog`), pinta lo que dicta `dialog_layout`, recoge los clics y las teclas y los
//! traduce a eventos.
//!
//! Va sobre `egui::Modal`, que pone un velo que consume clic, arrastre y rueda, y limita el foco a
//! su capa. Todo se coloca en píxeles físicos enteros (`u = n` píxeles); el texto fijo y el
//! externo pasan por `printable`.

use egui::{
    Align2, Area, AreaState, Color32, Context, Event as InputEvent, EventFilter, FontFamily,
    FontId, Id, Key, LayerId, Margin, Modal, MouseWheelUnit, Order, Painter, Pos2, Rect, Sense,
    Stroke, StrokeKind, TextEdit, Ui, UiBuilder, UiKind, pos2, vec2,
};

use crate::dialog::{Dialog, Event, FieldKind, Reaction, Rows, Validity};
use crate::dialog_keys::{DialogKey, FocusTarget, KeyAction, dialog_key, key_action};
use crate::dialog_layout::{
    BLOCK_GAP_U, BUTTON_W_U, CHOICE_GAP_U, CHOICE_ROW_H_U, CONTENT_W_U, DialogLayout, FIELD_H_U,
    PAD_U, SCROLLBAR_GAP_U, SCROLLBAR_W_U, TITLE_H_U, ZONE_COLS, ZONE_FRAME_U, dialog_layout,
    ensure_visible, zone_rows,
};
use crate::edit_layout::ROW_H_U;
use crate::licenses_view::{clamp_first_line, wrap_text};
use crate::number::{MAX_NUMBER_CHARS, step_enabled};
use crate::status::ADVANCE_PX;
use crate::text::{font_size_points, printable, text_scale};
use crate::theme::{
    ACCENT, SURFACE_0, SURFACE_1, SURFACE_2, SURFACE_3, TEXT, TEXT_DIM, TEXT_DISABLED, WARNING,
};

/// Prefijo de los errores de validación: no dependen solo del color.
pub const ERROR_PREFIX: &str = "error: ";

pub use crate::dialog::MISSING_PREFIX;

/// El texto del pie: una lista de requisitos pendientes (`falta: …`, `faltan N: …`) va tal cual y
/// cualquier otro motivo, con `error: ` delante. Nunca pasa de 44 columnas.
pub fn footer_text(reason: &str) -> String {
    if crate::dialog::is_missing_list(reason) {
        reason.to_string()
    } else {
        format!("{ERROR_PREFIX}{reason}")
    }
}

/// Fondo del diálogo.
pub const DIALOG_FILL: Color32 = SURFACE_1;
/// Fondo de los campos y de las zonas de texto.
pub const FIELD_FILL: Color32 = SURFACE_0;
/// Fondo de un botón.
pub const BUTTON_FILL: Color32 = SURFACE_2;
/// Fondo de un botón con el ratón encima.
pub const BUTTON_HOVER_FILL: Color32 = SURFACE_3;
/// Borde de un campo, un botón o una marca.
pub const EDGE: Color32 = SURFACE_3;
/// Aro de una opción y borde de una casilla (≥ 3:1 sobre el campo; `EDGE` solo da 1,73).
pub const TOGGLE_EDGE: Color32 = TEXT_DIM;
/// Anillo del foco y marca de lo elegido.
pub const FOCUS_RING: Color32 = ACCENT;
/// Color de los errores de validación.
pub const ERROR_FG: Color32 = WARNING;
/// Velo sobre el resto de la ventana.
pub const VEIL: Color32 = Color32::from_black_alpha(128);

/// Mínimo de contraste del texto (WCAG 1.4.3).
const MIN_TEXT: f64 = 4.5;
/// Mínimo de contraste del anillo y de las marcas (WCAG 1.4.11).
const MIN_MARK: f64 = 3.0;

/// Los pares (nombre, color, fondo, mínimo) que se pintan en un diálogo: de aquí salen los colores
/// que usa el pintado. Solo el texto deshabilitado (`TEXT_DISABLED`) queda exento.
pub const CONTRAST_PAIRS: [(&str, Color32, Color32, f64); 11] = [
    ("texto sobre el diálogo", TEXT, DIALOG_FILL, MIN_TEXT),
    (
        "texto tenue sobre el diálogo",
        TEXT_DIM,
        DIALOG_FILL,
        MIN_TEXT,
    ),
    ("error sobre el diálogo", ERROR_FG, DIALOG_FILL, MIN_TEXT),
    ("texto sobre un campo", TEXT, FIELD_FILL, MIN_TEXT),
    ("texto sobre un botón", TEXT, BUTTON_FILL, MIN_TEXT),
    (
        "texto sobre un botón con el ratón",
        TEXT,
        BUTTON_HOVER_FILL,
        MIN_TEXT,
    ),
    ("anillo sobre un campo", FOCUS_RING, FIELD_FILL, MIN_MARK),
    // `TEXT_DIM` pide 4,5 (regla de la 18 para ese color); sobre el campo da 6,9.
    (
        "borde de opción y casilla sobre un campo",
        TOGGLE_EDGE,
        FIELD_FILL,
        MIN_TEXT,
    ),
    ("anillo sobre el diálogo", FOCUS_RING, DIALOG_FILL, MIN_MARK),
    ("anillo sobre un botón", FOCUS_RING, BUTTON_FILL, MIN_MARK),
    (
        "anillo sobre un botón con el ratón",
        FOCUS_RING,
        BUTTON_HOVER_FILL,
        MIN_MARK,
    ),
];

/// Paso de la rueda sobre el cuerpo: 3 filas de 14 u.
const BODY_NOTCH_U: f32 = 42.0;
/// Líneas que avanza una muesca de la rueda sobre una zona de texto.
const ZONE_LINES_PER_NOTCH: f32 = 3.0;
/// Ancho del campo numérico y del hexadecimal, y su `x` (tras la columna de la etiqueta).
const FIELD_X_U: i64 = 110;
const FIELD_W_U: i64 = 69;
const STEP_BUTTON_U: i64 = 18;
const MINUS_X_U: i64 = 183;
const PLUS_X_U: i64 = 203;
const RANGE_X_U: i64 = 225;
/// Botón de campo: ancho solo y con un segundo botón (`x` y ancho de cada uno).
const BUTTON_FIELD_W_U: i64 = 206;
const BUTTON_PAIR_MAIN_W_U: i64 = 125;
const BUTTON_ASIDE_X_U: i64 = 239;
const BUTTON_ASIDE_W_U: i64 = 69;
/// Marca de una casilla o una opción: caja de 10 u y marca de 6 u.
const BOX_U: i64 = 10;
const MARK_U: i64 = 6;

/// Escala de este fotograma: puntos y píxeles.
#[derive(Clone, Copy)]
struct Scale {
    ppp: f32,
    n: i64,
}

impl Scale {
    fn pt(self, px: i64) -> f32 {
        px as f32 / self.ppp
    }

    fn rect(self, x: i64, y: i64, w: i64, h: i64) -> Rect {
        Rect::from_min_max(
            pos2(self.pt(x), self.pt(y)),
            pos2(self.pt(x + w), self.pt(y + h)),
        )
    }

    fn rect_of(self, r: crate::view::ScreenRect) -> Rect {
        self.rect(r.x, r.y, r.width, r.height)
    }

    fn u(self) -> f32 {
        self.n as f32 / self.ppp
    }
}

/// Intentos de pedir el foco inicial en fotogramas sin puntero antes de rendirse.
const MAX_FOCUS_TRIES: u8 = 3;

/// El foco inicial pedido y aún sin confirmar por egui.
#[derive(Clone, Copy)]
struct PendingFocus {
    target: FocusTarget,
    tries: u8,
}

/// Un elemento al que se puede dar el foco, con dónde está (para el foco y el desplazamiento).
struct Item {
    target: FocusTarget,
    id: Id,
    /// Extensión vertical en el contenido del cuerpo, en `u`; `None` si está en el pie.
    span_u: Option<(i64, i64)>,
}

/// Un bloque del cuerpo: el campo, dónde empieza y cuánto mide, en `u` del contenido.
struct Block {
    index: usize,
    y_u: i64,
    h_u: i64,
}

fn widget_id(target: FocusTarget) -> Id {
    match target {
        FocusTarget::Field(id) => Id::unique(("pixoko-dialog", id)),
        FocusTarget::Option(id, i) => Id::unique(("pixoko-dialog-option", id, i)),
        FocusTarget::Minus(id) => Id::unique(("pixoko-dialog-minus", id)),
        FocusTarget::Plus(id) => Id::unique(("pixoko-dialog-plus", id)),
        FocusTarget::Zone(id) => Id::unique(("pixoko-dialog-zone", id)),
        FocusTarget::Button(id) => Id::unique(("pixoko-dialog-button", id)),
        FocusTarget::ButtonAside(id) => Id::unique(("pixoko-dialog-button-aside", id)),
        FocusTarget::Accept => Id::unique("pixoko-dialog-accept"),
        FocusTarget::Cancel => Id::unique("pixoko-dialog-cancel"),
    }
}

fn blocks(dialog: &Dialog, body_h_u: i64) -> Vec<Block> {
    let mut y_u = PAD_U;
    let mut out = Vec::new();
    for index in dialog.shown_indices() {
        let h_u = if dialog.is_fill() {
            (body_h_u - 2 * PAD_U).max(0)
        } else {
            dialog.block_h_u(index)
        };
        out.push(Block { index, y_u, h_u });
        y_u += h_u + BLOCK_GAP_U;
    }
    out
}

/// Líneas ajustadas de una zona de texto y filas que se ven.
fn zone_metrics(dialog: &Dialog, block: &Block) -> (Vec<String>, usize) {
    let FieldKind::ReadOnly { text, rows } = &dialog.spec().fields[block.index].kind else {
        return (Vec::new(), 0);
    };
    let lines = wrap_text(text, ZONE_COLS);
    let visible = match rows {
        Rows::Fixed(r) => (*r).min(lines.len().max(1)),
        Rows::Fill => zone_rows(block.h_u),
    };
    (lines, visible)
}

/// Líneas que mueve la rueda (con signo, hacia abajo positivo) según la unidad del evento.
fn wheel_amount(
    unit: MouseWheelUnit,
    delta_y: f32,
    line: f32,
    point_to_unit: f32,
    page: f32,
) -> f32 {
    match unit {
        MouseWheelUnit::Line => -delta_y * line,
        MouseWheelUnit::Point => -delta_y * point_to_unit,
        MouseWheelUnit::Page => -delta_y * page,
    }
}

fn round_away(amount: f32) -> i64 {
    let moved = amount.round() as i64;
    if moved == 0 && amount != 0.0 {
        amount.signum() as i64
    } else {
        moved
    }
}

/// Pinta el diálogo y devuelve su respuesta de este fotograma (`None` casi siempre).
///
/// `focus` es el elemento enfocado: lo que se leyó al final del fotograma anterior (con él se
/// deciden `Intro` y `Escape`) y se actualiza al final de este. Un diálogo recién abierto pide a
/// egui su foco inicial (`Dialog::take_focus_request`).
pub fn show_dialog(
    ctx: &Context,
    dialog: &mut Dialog,
    focus: &mut Option<FocusTarget>,
) -> Reaction {
    let ppp = ctx.pixels_per_point();
    let n = i64::from(text_scale(ppp));
    let scale = Scale { ppp, n };
    let screen = ctx.content_rect();
    let window_px = (
        (screen.width() * ppp).round() as i64,
        (screen.height() * ppp).round() as i64,
    );
    let layout = dialog_layout(
        window_px,
        n as u32,
        dialog.content_h_u(),
        dialog.is_fill(),
        dialog.spec().cancel.is_some(),
    );
    let body_h_u = layout.body.height / n;
    let blocks = blocks(dialog, body_h_u);

    // Foco inicial: un clic o una pulsación en el fotograma le quita el foco a todo widget que no
    // está bajo el puntero justo al crearlo, y un diálogo abierto con un clic se pinta por
    // primera vez en el fotograma de ese clic. Por eso la petición queda pendiente y se repite
    // en los fotogramas sin puntero hasta que egui confirma el foco (al final del fotograma).
    let pending_id = Id::unique("pixoko-dialog-initial-focus");
    if let Some(target) = dialog.take_focus_request() {
        ctx.data_mut(|data| data.insert_temp(pending_id, Some(PendingFocus { target, tries: 0 })));
    }
    let focus_pending: Option<PendingFocus> = ctx.data(|data| data.get_temp(pending_id)).flatten();
    let pointer_active =
        ctx.input(|input| input.pointer.any_click() || input.pointer.any_pressed());
    if let Some(PendingFocus { target, .. }) = focus_pending
        && !pointer_active
    {
        ctx.memory_mut(|memory| memory.request_focus(widget_id(target)));
    }

    // Teclas del diálogo con el foco del principio del fotograma.
    let events: Vec<InputEvent> = ctx.input(|input| input.raw.events.clone());
    let mut reaction = Reaction::None;
    for event in &events {
        let InputEvent::Key {
            key,
            pressed: true,
            repeat,
            modifiers,
            ..
        } = event
        else {
            continue;
        };
        let Some(dialog_key): Option<DialogKey> = dialog_key(*key, *modifiers) else {
            continue;
        };
        let applied = match key_action(dialog, *focus, dialog_key, *repeat) {
            KeyAction::Accept => dialog.event(Event::Accept),
            KeyAction::Cancel => dialog.event(Event::Cancel),
            KeyAction::ConfirmHex(field) => dialog.event(Event::HexConfirm { field, blur: false }),
            KeyAction::RevertHex(field) => dialog.event(Event::HexRevert { field }),
            KeyAction::Nothing => Reaction::None,
        };
        if matches!(applied, Reaction::Accepted(_) | Reaction::Cancelled) {
            reaction = applied;
            break;
        }
    }

    scroll_inputs(ctx, dialog, &layout, &blocks, scale, &events);

    let mut items: Vec<Item> = Vec::new();
    let mut pending: Vec<Event> = Vec::new();
    let mut pressed_accept = false;
    let mut pressed_cancel = false;

    let dialog_rect = scale.rect_of(layout.dialog);
    // Un área nueva se pinta invisible en su primer fotograma (para medirla). Como ya se cuánto
    // mide, se le da ese estado de antemano y se ve desde el primero.
    let area_id = Id::unique("pixoko-dialog-area");
    if AreaState::load(ctx, area_id).is_none() {
        ctx.memory_mut(|memory| {
            memory.areas_mut().set_state(
                LayerId::new(Order::Foreground, area_id),
                AreaState {
                    pivot_pos: Some(dialog_rect.min),
                    pivot: Align2::LEFT_TOP,
                    size: Some(dialog_rect.size()),
                    interactable: true,
                    last_became_visible_at: None,
                },
            );
        });
    }
    let area = Area::new(area_id)
        .fade_in(false)
        .kind(UiKind::Modal)
        .sense(Sense::hover())
        .order(Order::Foreground)
        .interactable(true)
        .fixed_pos(dialog_rect.min);
    Modal::new(Id::unique("pixoko-dialog-modal"))
        .area(area)
        .frame(egui::Frame::NONE)
        .backdrop_color(VEIL)
        .show(ctx, |ui| {
            ui.allocate_exact_size(dialog_rect.size(), Sense::hover());
            let mut frame_ui = ui.new_child(UiBuilder::new().max_rect(dialog_rect));
            frame_ui.shrink_clip_rect(dialog_rect);
            let painter = frame_ui.painter().clone();
            let u = scale.u();
            painter.rect(
                dialog_rect,
                0.0,
                DIALOG_FILL,
                Stroke::new(u, TEXT_DIM),
                StrokeKind::Inside,
            );
            let font = FontId::new(font_size_points(ppp), FontFamily::Proportional);

            // Título.
            let title = scale.rect_of(layout.title);
            painter.text(
                title.min + vec2(PAD_U as f32 * u, 3.0 * u),
                Align2::LEFT_TOP,
                printable(ctx, dialog.spec().title),
                font.clone(),
                TEXT,
            );
            painter.rect_filled(
                scale.rect(
                    layout.title.x,
                    layout.title.y + (TITLE_H_U - 1) * n,
                    layout.title.width,
                    n,
                ),
                0.0,
                EDGE,
            );

            // Cuerpo.
            let body_rect = scale.rect_of(layout.body);
            let mut body_ui = frame_ui.new_child(UiBuilder::new().max_rect(body_rect));
            body_ui.shrink_clip_rect(body_rect);
            let body_painter = body_ui.painter().clone();
            let origin = Origin {
                x_px: layout.dialog.x,
                y_px: layout.body.y - dialog.scroll_u() * n,
                scale,
            };
            for block in &blocks {
                let spec = &dialog.spec().fields[block.index];
                match &spec.kind {
                    FieldKind::Number { min, max, .. } => {
                        draw_number(
                            ctx,
                            &mut body_ui,
                            &body_painter,
                            &font,
                            &origin,
                            block,
                            dialog,
                            (*min, *max),
                            &mut items,
                            &mut pending,
                        );
                    }
                    FieldKind::Hex { .. } => {
                        draw_hex(
                            ctx,
                            &mut body_ui,
                            &body_painter,
                            &font,
                            &origin,
                            block,
                            dialog,
                            &events,
                            &mut items,
                            &mut pending,
                        );
                    }
                    FieldKind::Check { .. } => {
                        draw_check(
                            ctx,
                            &mut body_ui,
                            &body_painter,
                            &font,
                            &origin,
                            block,
                            dialog,
                            &mut items,
                            &mut pending,
                        );
                    }
                    FieldKind::Choice { .. } => {
                        draw_choice(
                            ctx,
                            &mut body_ui,
                            &body_painter,
                            &font,
                            &origin,
                            block,
                            dialog,
                            &mut items,
                            &mut pending,
                        );
                    }
                    FieldKind::ReadOnly { .. } => {
                        draw_zone(
                            ctx,
                            &mut body_ui,
                            &body_painter,
                            &font,
                            &origin,
                            block,
                            dialog,
                            &mut items,
                        );
                    }
                    FieldKind::Button { .. } => {
                        draw_button_field(
                            ctx,
                            &mut body_ui,
                            &body_painter,
                            &font,
                            &origin,
                            block,
                            dialog,
                            &mut items,
                            &mut pending,
                        );
                    }
                }
            }
            draw_scrollbar(&body_painter, &layout, scale, dialog.scroll_u());

            // Pie: línea, motivo y botones.
            painter.rect_filled(
                scale.rect(layout.footer.x, layout.footer.y, layout.footer.width, n),
                0.0,
                EDGE,
            );
            if let Some(reason) = dialog.reason() {
                painter.text(
                    scale.rect_of(layout.reason).min,
                    Align2::LEFT_TOP,
                    printable(ctx, &footer_text(&reason)),
                    font.clone(),
                    ERROR_FG,
                );
            }
            let accept_enabled = dialog.is_valid();
            pressed_accept = draw_button(
                ctx,
                &mut frame_ui,
                &painter,
                &font,
                scale,
                layout.accept,
                dialog.spec().accept,
                accept_enabled,
                FocusTarget::Accept,
                &mut items,
            );
            if let (Some(rect), Some(label)) = (layout.cancel, dialog.spec().cancel) {
                pressed_cancel = draw_button(
                    ctx,
                    &mut frame_ui,
                    &painter,
                    &font,
                    scale,
                    rect,
                    label,
                    true,
                    FocusTarget::Cancel,
                    &mut items,
                );
            }
        });

    for event in pending {
        let applied = dialog.event(event);
        if matches!(applied, Reaction::Pressed { .. }) && matches!(reaction, Reaction::None) {
            reaction = applied;
        }
    }
    if matches!(reaction, Reaction::None) && pressed_accept {
        reaction = dialog.event(Event::Accept);
    }
    if matches!(reaction, Reaction::None) && pressed_cancel {
        reaction = dialog.event(Event::Cancel);
    }
    if matches!(reaction, Reaction::Rejected(_)) {
        reaction = Reaction::None;
    }

    // El foco de este fotograma y, si cambió, el elemento a la vista.
    let focused_id = ctx.memory(|memory| memory.focused());
    let new_focus = focused_id.and_then(|id| items.iter().find(|i| i.id == id));
    if let Some(PendingFocus { target, tries }) = focus_pending {
        // Cumplida si algún elemento del diálogo tiene el foco (el pedido o el que el usuario eligió
        // antes); si no, se reintenta en el siguiente fotograma, como mucho `MAX_FOCUS_TRIES` veces.
        let tries = tries + u8::from(!pointer_active);
        if new_focus.is_some() || tries >= MAX_FOCUS_TRIES {
            ctx.data_mut(|data| data.insert_temp(pending_id, None::<PendingFocus>));
        } else {
            ctx.data_mut(|data| data.insert_temp(pending_id, Some(PendingFocus { target, tries })));
        }
    }
    let new_target = new_focus.map(|item| item.target);
    if new_target != *focus {
        if let Some(Item {
            span_u: Some((top, bottom)),
            ..
        }) = new_focus
        {
            let max = layout.max_scroll_u;
            let scroll = ensure_visible(dialog.scroll_u(), *top, *bottom, body_h_u).clamp(0, max);
            if scroll != dialog.scroll_u() {
                dialog.set_scroll_u(scroll);
                ctx.request_repaint();
            }
        }
        *focus = new_target;
    }
    reaction
}

/// La rueda sobre una zona de texto o sobre el cuerpo, y las teclas de la zona enfocada.
fn scroll_inputs(
    ctx: &Context,
    dialog: &mut Dialog,
    layout: &DialogLayout,
    blocks: &[Block],
    scale: Scale,
    events: &[InputEvent],
) {
    let n = scale.n;
    let body_h_u = layout.body.height / n;
    let pointer = ctx
        .input(|input| input.pointer.hover_pos())
        .map(|p| (p.x * scale.ppp, p.y * scale.ppp));
    let in_rect = |px: (f32, f32), x: i64, y: i64, w: i64, h: i64| {
        px.0 >= x as f32 && px.0 < (x + w) as f32 && px.1 >= y as f32 && px.1 < (y + h) as f32
    };
    let over_body = pointer.is_some_and(|p| {
        in_rect(
            p,
            layout.body.x,
            layout.body.y,
            layout.body.width,
            layout.body.height,
        )
    });

    // Cuánto mueve la rueda, en líneas de zona y en `u` del cuerpo.
    let (mut zone_notches, mut body_notches_u) = (0.0f32, 0.0f32);
    for event in events {
        if let InputEvent::MouseWheel { unit, delta, .. } = event {
            zone_notches += wheel_amount(
                *unit,
                delta.y,
                ZONE_LINES_PER_NOTCH,
                scale.ppp / (ROW_H_U * n) as f32,
                1.0,
            );
            body_notches_u += wheel_amount(
                *unit,
                delta.y,
                BODY_NOTCH_U,
                scale.ppp / n as f32,
                body_h_u as f32,
            );
        }
    }

    let mut zone_taken = false;
    let content_y = layout.body.y - dialog.scroll_u() * n;
    for block in blocks {
        let FieldKind::ReadOnly { .. } = &dialog.spec().fields[block.index].kind else {
            continue;
        };
        let id = dialog.spec().fields[block.index].id;
        let (lines, visible) = zone_metrics(dialog, block);
        let mut first = dialog.first_line(id);
        let over_zone = pointer.is_some_and(|p| {
            over_body
                && in_rect(
                    p,
                    layout.dialog.x + PAD_U * n,
                    content_y + block.y_u * n,
                    CONTENT_W_U * n,
                    block.h_u * n,
                )
        });
        if over_zone && zone_notches != 0.0 {
            let moved = round_away(zone_notches);
            first = if moved >= 0 {
                first.saturating_add(moved as usize)
            } else {
                first.saturating_sub(moved.unsigned_abs() as usize)
            };
            zone_taken = true;
        }
        if ctx.memory(|memory| memory.has_focus(widget_id(FocusTarget::Zone(id)))) {
            for event in events {
                let InputEvent::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } = event
                else {
                    continue;
                };
                if !modifiers.is_none() {
                    continue;
                }
                let page = visible.saturating_sub(1).max(1);
                first = match key {
                    Key::ArrowDown => first.saturating_add(1),
                    Key::ArrowUp => first.saturating_sub(1),
                    Key::PageDown => first.saturating_add(page),
                    Key::PageUp => first.saturating_sub(page),
                    Key::Home => 0,
                    Key::End => usize::MAX / 2,
                    _ => first,
                };
            }
        }
        dialog.set_first_line(id, clamp_first_line(first, lines.len(), visible));
    }

    let mut scroll = dialog.scroll_u();
    if over_body && !zone_taken && body_notches_u != 0.0 {
        scroll += i64::from(round_away(body_notches_u) as i32);
    }
    dialog.set_scroll_u(scroll.clamp(0, layout.max_scroll_u));
}

/// Dónde está el contenido del cuerpo en píxeles físicos.
struct Origin {
    x_px: i64,
    y_px: i64,
    scale: Scale,
}

impl Origin {
    /// Rectángulo de `u` dentro del contenido (`x` desde el borde del diálogo).
    fn rect_u(&self, x_u: i64, y_u: i64, w_u: i64, h_u: i64) -> Rect {
        let n = self.scale.n;
        self.scale
            .rect(self.x_px + x_u * n, self.y_px + y_u * n, w_u * n, h_u * n)
    }

    fn pos_u(&self, x_u: i64, y_u: i64) -> Pos2 {
        let n = self.scale.n;
        pos2(
            self.scale.pt(self.x_px + x_u * n),
            self.scale.pt(self.y_px + y_u * n),
        )
    }
}

fn ring(painter: &Painter, rect: Rect, scale: Scale) {
    painter.rect_stroke(
        rect,
        0.0,
        Stroke::new(2.0 * scale.u(), FOCUS_RING),
        StrokeKind::Inside,
    );
}

fn draw_label(
    ctx: &Context,
    painter: &Painter,
    font: &FontId,
    origin: &Origin,
    y_u: i64,
    label: &str,
) {
    painter.text(
        origin.pos_u(PAD_U, y_u),
        Align2::LEFT_TOP,
        printable(ctx, label),
        font.clone(),
        TEXT,
    );
}

fn draw_error(
    ctx: &Context,
    painter: &Painter,
    font: &FontId,
    origin: &Origin,
    y_u: i64,
    validity: Option<Validity>,
) {
    if let Some(Validity::Invalid(reason)) = validity {
        painter.text(
            origin.pos_u(PAD_U, y_u),
            Align2::LEFT_TOP,
            printable(ctx, &format!("{ERROR_PREFIX}{reason}")),
            font.clone(),
            ERROR_FG,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_step_button(
    ctx: &Context,
    ui: &mut Ui,
    painter: &Painter,
    font: &FontId,
    origin: &Origin,
    x_u: i64,
    y_u: i64,
    label: &str,
    enabled: bool,
    target: FocusTarget,
    span_u: (i64, i64),
    items: &mut Vec<Item>,
) -> bool {
    let scale = origin.scale;
    let rect = origin.rect_u(x_u, y_u, STEP_BUTTON_U, STEP_BUTTON_U);
    let id = widget_id(target);
    let response = ui.interact(rect, id, Sense::click());
    items.push(Item {
        target,
        id,
        span_u: Some(span_u),
    });
    let fill = if enabled && response.hovered() {
        BUTTON_HOVER_FILL
    } else {
        BUTTON_FILL
    };
    painter.rect(
        rect,
        0.0,
        fill,
        Stroke::new(scale.u(), EDGE),
        StrokeKind::Inside,
    );
    let color = if enabled { TEXT } else { TEXT_DISABLED };
    let text_x = origin.x_px
        + x_u * scale.n
        + (STEP_BUTTON_U * scale.n - i64::from(ADVANCE_PX) * scale.n) / 2;
    painter.text(
        pos2(scale.pt(text_x), origin.pos_u(0, y_u + 2).y),
        Align2::LEFT_TOP,
        printable(ctx, label),
        font.clone(),
        color,
    );
    if response.has_focus() {
        ring(painter, rect, scale);
    }
    enabled && response.clicked()
}

#[allow(clippy::too_many_arguments)]
fn draw_number(
    ctx: &Context,
    ui: &mut Ui,
    painter: &Painter,
    font: &FontId,
    origin: &Origin,
    block: &Block,
    dialog: &Dialog,
    (min, max): (u32, u32),
    items: &mut Vec<Item>,
    pending: &mut Vec<Event>,
) {
    let scale = origin.scale;
    let spec = &dialog.spec().fields[block.index];
    let field = spec.id;
    let y = block.y_u;
    let span = (y, y + block.h_u);
    draw_label(ctx, painter, font, origin, y + 2, spec.label);

    let text = dialog.text(field).unwrap_or_default();
    let box_rect = origin.rect_u(FIELD_X_U, y, FIELD_W_U, FIELD_H_U);
    painter.rect(
        box_rect,
        0.0,
        FIELD_FILL,
        Stroke::new(scale.u(), EDGE),
        StrokeKind::Inside,
    );
    let inner = origin.rect_u(FIELD_X_U + 3, y + 2, FIELD_W_U - 6, ROW_H_U);
    let mut edited = text.clone();
    let id = widget_id(FocusTarget::Field(field));
    let response = ui.put(
        inner,
        TextEdit::singleline(&mut edited)
            .id(id)
            .font(font.clone())
            .text_color(TEXT)
            .frame(egui::Frame::NONE)
            .margin(Margin::ZERO)
            .desired_width(inner.width())
            .clip_text(true)
            .char_limit(MAX_NUMBER_CHARS + 1),
    );
    items.push(Item {
        target: FocusTarget::Field(field),
        id,
        span_u: Some(span),
    });
    if response.changed() && edited != text {
        pending.push(Event::Number {
            field,
            text: edited,
        });
    }
    if response.has_focus() {
        ring(painter, box_rect, scale);
    }

    for (x_u, label, delta) in [(MINUS_X_U, "-", -1i8), (PLUS_X_U, "+", 1i8)] {
        let target = if delta < 0 {
            FocusTarget::Minus(field)
        } else {
            FocusTarget::Plus(field)
        };
        let enabled = step_enabled(&text, min, max, delta);
        if draw_step_button(
            ctx, ui, painter, font, origin, x_u, y, label, enabled, target, span, items,
        ) {
            pending.push(Event::Step { field, delta });
        }
    }
    painter.text(
        origin.pos_u(RANGE_X_U, y + 2),
        Align2::LEFT_TOP,
        printable(ctx, &format!("{min}-{max}")),
        font.clone(),
        TEXT_DIM,
    );
    draw_error(
        ctx,
        painter,
        font,
        origin,
        y + FIELD_H_U,
        dialog.validity(field),
    );
}

#[allow(clippy::too_many_arguments)]
fn draw_hex(
    ctx: &Context,
    ui: &mut Ui,
    painter: &Painter,
    font: &FontId,
    origin: &Origin,
    block: &Block,
    dialog: &Dialog,
    events: &[InputEvent],
    items: &mut Vec<Item>,
    pending: &mut Vec<Event>,
) {
    let scale = origin.scale;
    let spec = &dialog.spec().fields[block.index];
    let field = spec.id;
    let y = block.y_u;
    draw_label(ctx, painter, font, origin, y + 2, spec.label);

    let text = dialog.text(field).unwrap_or_default();
    let box_rect = origin.rect_u(FIELD_X_U, y, FIELD_W_U, FIELD_H_U);
    painter.rect(
        box_rect,
        0.0,
        FIELD_FILL,
        Stroke::new(scale.u(), EDGE),
        StrokeKind::Inside,
    );
    let inner = origin.rect_u(FIELD_X_U + 3, y + 2, FIELD_W_U - 6, ROW_H_U);
    let mut edited = text.clone();
    let id = widget_id(FocusTarget::Field(field));
    let response = ui.put(
        inner,
        TextEdit::singleline(&mut edited)
            .id(id)
            .font(font.clone())
            .text_color(TEXT)
            .frame(egui::Frame::NONE)
            .margin(Margin::ZERO)
            .desired_width(inner.width())
            .clip_text(true),
    );
    items.push(Item {
        target: FocusTarget::Field(field),
        id,
        span_u: Some((y, y + block.h_u)),
    });
    let edited_id = id.with("editado");
    if response.changed() && edited != text {
        pending.push(Event::Hex {
            field,
            text: edited,
        });
        ctx.data_mut(|data| data.insert_temp(edited_id, true));
    }
    if response.lost_focus() {
        let typed = ctx
            .data(|data| data.get_temp::<bool>(edited_id))
            .unwrap_or(false);
        let enter_or_escape = events.iter().any(|event| {
            matches!(
                event,
                InputEvent::Key {
                    key: Key::Enter | Key::Escape,
                    pressed: true,
                    ..
                }
            )
        });
        let enter = events.iter().any(|event| {
            matches!(
                event,
                InputEvent::Key {
                    key: Key::Enter,
                    pressed: true,
                    ..
                }
            )
        });
        if enter && dialog.hex_pending(field) {
            // Intro con un texto inválido: se queda en el campo, como en 11c.
            response.request_focus();
        } else if typed && !enter_or_escape {
            pending.push(Event::HexConfirm { field, blur: true });
        }
        ctx.data_mut(|data| data.insert_temp(edited_id, false));
    }
    if response.has_focus() {
        ring(painter, box_rect, scale);
    }
    draw_error(
        ctx,
        painter,
        font,
        origin,
        y + FIELD_H_U,
        dialog.validity(field),
    );
}

/// Una fila con una caja de 10 u, su marca y el rótulo; devuelve si se pulsó.
#[allow(clippy::too_many_arguments)]
fn draw_toggle(
    ctx: &Context,
    ui: &mut Ui,
    painter: &Painter,
    font: &FontId,
    origin: &Origin,
    y_u: i64,
    label: &str,
    marked: bool,
    target: FocusTarget,
    span_u: (i64, i64),
    round: bool,
    items: &mut Vec<Item>,
) -> bool {
    let scale = origin.scale;
    let chars = label.chars().count() as i64;
    let width_u = BOX_U + 4 + chars * i64::from(ADVANCE_PX) + 4;
    let rect = origin.rect_u(PAD_U, y_u, width_u, CHOICE_ROW_H_U);
    let id = widget_id(target);
    let response = ui.interact(rect, id, Sense::click());
    items.push(Item {
        target,
        id,
        span_u: Some(span_u),
    });
    let marker = origin.rect_u(PAD_U + 2, y_u + 3, BOX_U, BOX_U);
    if round {
        // Una opción excluyente: aro de 10 u de diámetro exterior y, si es la elegida, un punto.
        let radius = (BOX_U as f32 / 2.0 - 0.5) * scale.u();
        painter.circle(
            marker.center(),
            radius,
            FIELD_FILL,
            Stroke::new(scale.u(), TOGGLE_EDGE),
        );
        if marked {
            painter.circle_filled(marker.center(), MARK_U as f32 / 2.0 * scale.u(), FOCUS_RING);
        }
    } else {
        painter.rect(
            marker,
            0.0,
            FIELD_FILL,
            Stroke::new(scale.u(), TOGGLE_EDGE),
            StrokeKind::Inside,
        );
        if marked {
            painter.rect_filled(
                origin.rect_u(
                    PAD_U + 2 + (BOX_U - MARK_U) / 2,
                    y_u + 3 + (BOX_U - MARK_U) / 2,
                    MARK_U,
                    MARK_U,
                ),
                0.0,
                FOCUS_RING,
            );
        }
    }
    painter.text(
        origin.pos_u(PAD_U + 2 + BOX_U + 4, y_u + 1),
        Align2::LEFT_TOP,
        printable(ctx, label),
        font.clone(),
        TEXT,
    );
    if response.has_focus() {
        ring(painter, rect, scale);
    }
    response.clicked()
}

#[allow(clippy::too_many_arguments)]
fn draw_check(
    ctx: &Context,
    ui: &mut Ui,
    painter: &Painter,
    font: &FontId,
    origin: &Origin,
    block: &Block,
    dialog: &Dialog,
    items: &mut Vec<Item>,
    pending: &mut Vec<Event>,
) {
    let spec = &dialog.spec().fields[block.index];
    let on = dialog.checked(spec.id).unwrap_or(false);
    let span = (block.y_u, block.y_u + block.h_u);
    if draw_toggle(
        ctx,
        ui,
        painter,
        font,
        origin,
        block.y_u,
        spec.label,
        on,
        FocusTarget::Field(spec.id),
        span,
        false,
        items,
    ) {
        pending.push(Event::Check {
            field: spec.id,
            on: !on,
        });
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_choice(
    ctx: &Context,
    ui: &mut Ui,
    painter: &Painter,
    font: &FontId,
    origin: &Origin,
    block: &Block,
    dialog: &Dialog,
    items: &mut Vec<Item>,
    pending: &mut Vec<Event>,
) {
    let spec = &dialog.spec().fields[block.index];
    let FieldKind::Choice { options, .. } = &spec.kind else {
        return;
    };
    let selected = dialog.chosen(spec.id).unwrap_or(0);
    draw_label(ctx, painter, font, origin, block.y_u, spec.label);
    for (i, option) in options.iter().enumerate() {
        let y = block.y_u + ROW_H_U + i as i64 * (CHOICE_ROW_H_U + CHOICE_GAP_U);
        if draw_toggle(
            ctx,
            ui,
            painter,
            font,
            origin,
            y,
            option,
            i == selected,
            FocusTarget::Option(spec.id, i),
            (y, y + CHOICE_ROW_H_U),
            true,
            items,
        ) {
            pending.push(Event::Choice {
                field: spec.id,
                index: i,
            });
        }
    }
}

/// Un campo `Button`: el rótulo a la izquierda, el botón principal y, si lo hay, el segundo. Un
/// botón atenuado no se enfoca ni responde.
#[allow(clippy::too_many_arguments)]
fn draw_button_field(
    ctx: &Context,
    ui: &mut Ui,
    painter: &Painter,
    font: &FontId,
    origin: &Origin,
    block: &Block,
    dialog: &Dialog,
    items: &mut Vec<Item>,
    pending: &mut Vec<Event>,
) {
    let spec = &dialog.spec().fields[block.index];
    let FieldKind::Button { text, aside } = &spec.kind else {
        return;
    };
    let y = block.y_u;
    let span = (y, y + block.h_u);
    draw_label(ctx, painter, font, origin, y + 2, spec.label);
    // Sin rótulo ni segundo botón (la lista de atajos) el botón ocupa toda la fila, con el texto a
    // la izquierda.
    let wide = spec.label.is_empty() && aside.is_none();
    let (main_x, main_w) = if wide {
        (PAD_U, CONTENT_W_U)
    } else if aside.is_some() {
        (FIELD_X_U, BUTTON_PAIR_MAIN_W_U)
    } else {
        (FIELD_X_U, BUTTON_FIELD_W_U)
    };
    let mut buttons = vec![(FocusTarget::Button(spec.id), main_x, main_w, text.as_str(), false)];
    if let Some(label) = aside {
        buttons.push((
            FocusTarget::ButtonAside(spec.id),
            BUTTON_ASIDE_X_U,
            BUTTON_ASIDE_W_U,
            label,
            true,
        ));
    }
    for (target, x_u, w_u, label, is_aside) in buttons {
        let enabled = dialog.button_enabled(spec.id, is_aside);
        if draw_field_button(
            ctx,
            ui,
            painter,
            font,
            origin,
            (x_u, y, w_u),
            label,
            (enabled, wide),
            target,
            span,
            items,
        ) {
            pending.push(Event::Press {
                field: spec.id,
                aside: is_aside,
            });
        }
    }
}

/// Un botón de campo con el texto centrado. Devuelve si se pulsó.
#[allow(clippy::too_many_arguments)]
fn draw_field_button(
    ctx: &Context,
    ui: &mut Ui,
    painter: &Painter,
    font: &FontId,
    origin: &Origin,
    (x_u, y_u, w_u): (i64, i64, i64),
    label: &str,
    (enabled, left): (bool, bool),
    target: FocusTarget,
    span_u: (i64, i64),
    items: &mut Vec<Item>,
) -> bool {
    let scale = origin.scale;
    let rect = origin.rect_u(x_u, y_u, w_u, FIELD_H_U);
    let id = widget_id(target);
    let response = enabled.then(|| ui.interact(rect, id, Sense::click()));
    if response.is_some() {
        items.push(Item {
            target,
            id,
            span_u: Some(span_u),
        });
    }
    let hovered = response.as_ref().is_some_and(|r| r.hovered());
    painter.rect(
        rect,
        0.0,
        if hovered {
            BUTTON_HOVER_FILL
        } else {
            BUTTON_FILL
        },
        Stroke::new(scale.u(), EDGE),
        StrokeKind::Inside,
    );
    let shown = printable(ctx, label);
    let chars = shown.chars().count() as i64;
    let text_x = origin.x_px
        + x_u * scale.n
        + if left {
            4 * scale.n
        } else {
            ((w_u * scale.n - chars * i64::from(ADVANCE_PX) * scale.n) / 2).max(0)
        };
    painter.text(
        pos2(scale.pt(text_x), origin.pos_u(0, y_u + 2).y),
        Align2::LEFT_TOP,
        shown,
        font.clone(),
        if enabled { TEXT } else { TEXT_DISABLED },
    );
    let Some(response) = response else {
        return false;
    };
    if response.has_focus() {
        ring(painter, rect, scale);
    }
    response.clicked()
}

#[allow(clippy::too_many_arguments)]
fn draw_zone(
    ctx: &Context,
    ui: &mut Ui,
    painter: &Painter,
    font: &FontId,
    origin: &Origin,
    block: &Block,
    dialog: &Dialog,
    items: &mut Vec<Item>,
) {
    let scale = origin.scale;
    let spec = &dialog.spec().fields[block.index];
    let (lines, visible) = zone_metrics(dialog, block);
    let first = clamp_first_line(dialog.first_line(spec.id), lines.len(), visible);
    let box_h = ZONE_FRAME_U + ROW_H_U * visible as i64;
    let box_rect = origin.rect_u(PAD_U, block.y_u, CONTENT_W_U, box_h);
    let target = FocusTarget::Zone(spec.id);
    let id = widget_id(target);
    let response = ui.interact(box_rect, id, Sense::click());
    items.push(Item {
        target,
        id,
        span_u: Some((block.y_u, block.y_u + box_h)),
    });
    // Con la zona enfocada se quedan las flechas (en egui moverían el foco).
    if response.has_focus() {
        ctx.memory_mut(|memory| {
            memory.set_focus_lock_filter(
                id,
                EventFilter {
                    horizontal_arrows: true,
                    vertical_arrows: true,
                    ..EventFilter::default()
                },
            );
        });
    }
    painter.rect(
        box_rect,
        0.0,
        FIELD_FILL,
        Stroke::new(scale.u(), EDGE),
        StrokeKind::Inside,
    );
    let clip = painter.with_clip_rect(box_rect.intersect(painter.clip_rect()));
    for (row, line) in lines.iter().skip(first).take(visible).enumerate() {
        clip.text(
            origin.pos_u(PAD_U + 5, block.y_u + 3 + ROW_H_U * row as i64),
            Align2::LEFT_TOP,
            printable(ctx, line),
            font.clone(),
            TEXT,
        );
    }
    if lines.len() > visible && visible > 0 {
        let track_h = ROW_H_U * visible as i64;
        let thumb_h = (track_h * visible as i64 / lines.len() as i64).max(4);
        let travel = track_h - thumb_h;
        let max_first = (lines.len() - visible) as i64;
        let thumb_y = if max_first == 0 {
            0
        } else {
            travel * first as i64 / max_first
        };
        let bar_x = PAD_U + 4 + (ZONE_COLS as i64) * i64::from(ADVANCE_PX) + SCROLLBAR_GAP_U;
        clip.rect_filled(
            origin.rect_u(bar_x, block.y_u + 3 + thumb_y, SCROLLBAR_W_U, thumb_h),
            0.0,
            TEXT_DIM,
        );
    }
    if response.has_focus() {
        ring(painter, box_rect, scale);
    }
}

/// La barra del cuerpo, a la derecha del contenido, cuando hay que desplazar.
fn draw_scrollbar(painter: &Painter, layout: &DialogLayout, scale: Scale, scroll_u: i64) {
    if layout.max_scroll_u <= 0 {
        return;
    }
    let n = scale.n;
    let body_h = layout.body.height / n;
    let content_h = body_h + layout.max_scroll_u;
    let thumb_h = (body_h * body_h / content_h).max(4);
    let travel = body_h - thumb_h;
    let thumb_y = travel * scroll_u / layout.max_scroll_u;
    let x = layout.dialog.x + (PAD_U + CONTENT_W_U + SCROLLBAR_GAP_U) * n;
    painter.rect_filled(
        scale.rect(x, layout.body.y, SCROLLBAR_W_U * n, body_h * n),
        0.0,
        FIELD_FILL,
    );
    painter.rect_filled(
        scale.rect(
            x,
            layout.body.y + thumb_y * n,
            SCROLLBAR_W_U * n,
            thumb_h * n,
        ),
        0.0,
        TEXT_DIM,
    );
}

/// Un botón del pie. Deshabilitado no se enfoca ni responde. Devuelve si se pulsó.
#[allow(clippy::too_many_arguments)]
fn draw_button(
    ctx: &Context,
    ui: &mut Ui,
    painter: &Painter,
    font: &FontId,
    scale: Scale,
    rect_px: crate::view::ScreenRect,
    label: &str,
    enabled: bool,
    target: FocusTarget,
    items: &mut Vec<Item>,
) -> bool {
    let rect = scale.rect_of(rect_px);
    let id = widget_id(target);
    let response = enabled.then(|| ui.interact(rect, id, Sense::click()));
    if response.is_some() {
        items.push(Item {
            target,
            id,
            span_u: None,
        });
    }
    let hovered = response.as_ref().is_some_and(|r| r.hovered());
    painter.rect(
        rect,
        0.0,
        if hovered {
            BUTTON_HOVER_FILL
        } else {
            BUTTON_FILL
        },
        Stroke::new(scale.u(), EDGE),
        StrokeKind::Inside,
    );
    let chars = label.chars().count() as i64;
    let text_x = rect_px.x + (BUTTON_W_U * scale.n - chars * i64::from(ADVANCE_PX) * scale.n) / 2;
    painter.text(
        pos2(scale.pt(text_x), scale.pt(rect_px.y + scale.n)),
        Align2::LEFT_TOP,
        printable(ctx, label),
        font.clone(),
        if enabled { TEXT } else { TEXT_DISABLED },
    );
    let Some(response) = response else {
        return false;
    };
    if response.has_focus() {
        ring(painter, rect, scale);
    }
    response.clicked()
}
