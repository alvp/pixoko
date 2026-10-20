//! Editor de color dentro del panel de paletas: deslizadores, campo hexadecimal y avisos
//! (specs/hito-2-editar-color.md). La lógica vive en [`crate::color_edit`] y la geometría en
//! [`crate::edit_layout`]; aquí solo se traducen a egui.
//!
//! Todo cae en píxeles físicos enteros: las medidas son `u` (`n` píxeles físicos) desde `top_px`,
//! que es múltiplo de `n`, y las posiciones dentro de una pista salen de `slider_x`.

use egui::{
    Align2, Event, FontFamily, FontId, Frame, Id, Key, Margin, PointerButton, Pos2, Rect, Stroke,
    StrokeKind, TextEdit, Ui, vec2,
};
use px_core::{Mode, Rgba};

use crate::color_edit::{
    ColorEditor, Commit, EditTarget, HexOutcome, Resolved, control_value, controls, hex_notice,
    hex_text, slider_value_at, slider_x, stale_notice, value_text,
};
use crate::edit_layout::{
    BUTTON_TEXT_X_U, BUTTON_TEXT_Y_U, FIELD_H_U, FIELD_PAD_U, FIELD_W_U, FIELD_X_U, HEADER_X_U,
    HEADER_Y_U, LABEL_X_U, PaletteButton, ROW_H_U, SEPARATOR_Y_U, THUMB_H_U, THUMB_W_U, TRACK_H_U,
    TRACK_W_U, TRACK_X_U, VALUE_X_U, button_rect_u, control_row_u, field_y_u, notice_y_u,
    slider_hit,
};
use crate::palette_grid::{PANEL_WIDTH_U, TEXT_WIDTH_U};
use crate::palette_ops::blocked_reason;
use crate::status::{bar_text, columns};
use crate::text::{font_size_points, ui_unit};
use crate::theme::{
    ACCENT, NOTICE_PREFIX, SURFACE_0, SURFACE_2, SURFACE_3, TEXT, TEXT_DIM, WARNING,
};

/// Dónde está el editor: escala, esquina del panel y `top_px` desde su borde superior (negativo
/// si el panel se desplazó). Lo pintado y los clics se limitan a `panel_rect`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Geometry {
    pub ppp: f32,
    pub n: u32,
    pub panel_min: Pos2,
    pub panel_rect: Rect,
    pub top_px: i64,
    pub mode: Mode,
}

impl Geometry {
    fn n_px(&self) -> i64 {
        i64::from(self.n)
    }

    /// Píxeles físicos a puntos.
    fn pt(&self, px: i64) -> f32 {
        px as f32 / self.ppp
    }

    /// Esquina del editor, en puntos.
    fn origin(&self) -> Pos2 {
        self.panel_min + vec2(0.0, self.pt(self.top_px))
    }

    /// Rectángulo en `u` desde la esquina del editor, en puntos.
    fn rect_u(&self, x: i64, y: i64, width: i64, height: i64) -> Rect {
        let n = self.n_px();
        Rect::from_min_size(
            self.origin() + vec2(self.pt(x * n), self.pt(y * n)),
            vec2(self.pt(width * n), self.pt(height * n)),
        )
    }

    /// Un punto de egui, en píxeles físicos desde la esquina del editor.
    pub(crate) fn rel_px(&self, pos: Pos2) -> (i64, i64) {
        (
            ((pos.x - self.panel_min.x) * self.ppp).round() as i64,
            ((pos.y - self.panel_min.y) * self.ppp).round() as i64 - self.top_px,
        )
    }

    /// Valor de `control` bajo la `x` (relativa al editor) de un punto.
    fn value_at(&self, x_px: i64, max: u32) -> u32 {
        slider_value_at(x_px - TRACK_X_U * self.n_px(), TRACK_W_U * self.n_px(), max)
    }
}

pub(crate) fn is_escape(event: &Event) -> bool {
    matches!(
        event,
        Event::Key { key: Key::Escape, pressed: true, modifiers, .. } if modifiers.is_none()
    )
}

/// Identificador del campo hexadecimal (el mismo cada fotograma).
pub(crate) fn hex_field_id() -> Id {
    Id::unique("pixoko-campo-hex")
}

fn is_enter(event: &Event) -> bool {
    matches!(
        event,
        Event::Key {
            key: Key::Enter,
            pressed: true,
            ..
        }
    )
}

/// Ratón y teclado de los deslizadores, en el orden de los eventos del fotograma. Devuelve el
/// color a aplicar si se soltó un arrastre que cambió el color. `primary_down` es lo que egui
/// cree del botón primario (red de seguridad si se perdió un soltar).
pub(crate) fn interact(
    events: &[Event],
    geo: &Geometry,
    editor: &mut ColorEditor,
    enabled: bool,
    primary_down: bool,
) -> Option<Commit> {
    let mut commit = None;
    for event in events {
        match event {
            Event::PointerButton {
                pos,
                button: PointerButton::Primary,
                pressed: true,
                ..
            } if enabled && !editor.dragging() && geo.panel_rect.contains(*pos) => {
                let (x, y) = geo.rel_px(*pos);
                if let Some(control) = slider_hit(geo.mode, x, y, geo.n) {
                    editor.begin(control, geo.value_at(x, control.max()));
                }
            }
            Event::PointerMoved(pos) => {
                if let Some(control) = editor.dragging_control() {
                    editor.drag(geo.value_at(geo.rel_px(*pos).0, control.max()));
                }
            }
            Event::PointerButton {
                pos,
                button: PointerButton::Primary,
                pressed: false,
                ..
            } => {
                if let Some(control) = editor.dragging_control() {
                    editor.drag(geo.value_at(geo.rel_px(*pos).0, control.max()));
                    commit = editor.release().or(commit);
                }
            }
            Event::WindowFocused(false) => editor.cancel(),
            event if is_escape(event) => editor.cancel(),
            _ => {}
        }
    }
    if editor.dragging() && !primary_down {
        commit = editor.release().or(commit);
    }
    commit
}

/// Qué botones de la paleta hay que pintar y cómo (specs/hito-2-paleta-operaciones.md).
#[derive(Clone, Copy, Debug)]
pub(crate) struct ButtonsView {
    /// Hay una quita pendiente: la fila 1 es `Confirmar` y `Cancelar`.
    pub removing: bool,
    /// Entradas de la paleta.
    pub len: usize,
    /// La quita pendiente tiene destino.
    pub has_destination: bool,
    /// Rótulo del botón de la fila 4 (`Fijar transparente` o `Quitar transparente`).
    pub transparent_label: &'static str,
}

/// Lo que el editor pinta además de los deslizadores y el campo.
#[derive(Clone, Debug, Default)]
pub(crate) struct Extras {
    /// Botones de la paleta (solo indexadas).
    pub buttons: Option<ButtonsView>,
    /// Las dos líneas de una quita pendiente, en lugar del aviso del editor.
    pub prompt: Option<[String; 2]>,
}

fn button_label(button: PaletteButton) -> &'static str {
    match button {
        PaletteButton::Add => "Añadir",
        PaletteButton::Remove => "Quitar",
        PaletteButton::Confirm => "Confirmar",
        PaletteButton::Cancel => "Cancelar",
        PaletteButton::RotateLeft => "Rotar a la izquierda",
        PaletteButton::RotateRight => "Rotar a la derecha",
        PaletteButton::Transparent => "Fijar transparente",
    }
}

/// Lo que pidió el editor en un fotograma.
#[derive(Debug, Default)]
pub(crate) struct Output {
    pub commit: Option<Commit>,
    /// Aviso de 5 s para la barra de estado.
    pub notice: Option<String>,
}

/// Pinta el editor y atiende el campo hexadecimal. `transparent`: el destino es la entrada
/// transparente de la paleta.
#[allow(clippy::too_many_arguments)]
pub(crate) fn show(
    ui: &mut Ui,
    geo: &Geometry,
    editor: &mut ColorEditor,
    target: EditTarget,
    transparent: bool,
    events: &[Event],
    stored: &dyn Fn(EditTarget) -> Option<Rgba>,
    extras: &Extras,
) -> Output {
    let mut output = Output::default();
    let Some((rgba, hsl)) = editor.shown() else {
        return output;
    };
    let ctx = ui.ctx().clone();
    let mode = geo.mode;
    let n = geo.n_px();
    let u = ui_unit(geo.ppp);
    let cols = columns((TEXT_WIDTH_U * n) as f32, geo.ppp);
    let font = FontId::new(font_size_points(geo.ppp), FontFamily::Proportional);
    let painter = ui.painter_at(geo.panel_rect);
    let text = |x: i64, y: i64, content: &str, color| {
        let shown = bar_text(&ctx, content, false, cols);
        painter.text(
            geo.rect_u(x, y, 0, ROW_H_U).min,
            Align2::LEFT_TOP,
            shown,
            font.clone(),
            color,
        );
    };

    painter.rect_filled(
        geo.rect_u(0, SEPARATOR_Y_U, PANEL_WIDTH_U, 1),
        0.0,
        SURFACE_3,
    );
    text(HEADER_X_U, HEADER_Y_U, &target.header(mode), TEXT);

    for &control in controls(mode) {
        let Some(row) = control_row_u(mode, control) else {
            continue;
        };
        let value = control_value(rgba, hsl, control);
        text(LABEL_X_U, row, &control.label().to_string(), TEXT);
        let track = geo.rect_u(
            TRACK_X_U,
            row + (ROW_H_U - TRACK_H_U) / 2,
            TRACK_W_U,
            TRACK_H_U,
        );
        painter.rect_filled(track, 0.0, SURFACE_3);
        let at = slider_x(value, TRACK_W_U * n, control.max());
        let filled = Rect::from_min_size(track.min, vec2(geo.pt(at), track.height()));
        painter.rect_filled(filled, 0.0, TEXT_DIM);
        let thumb = Rect::from_min_size(
            geo.rect_u(TRACK_X_U, row + (ROW_H_U - THUMB_H_U) / 2, 0, THUMB_H_U)
                .min
                + vec2(geo.pt(at - n), 0.0),
            vec2(geo.pt(THUMB_W_U * n), geo.pt(THUMB_H_U * n)),
        );
        painter.rect_filled(thumb, 0.0, ACCENT);
        text(VALUE_X_U, row, &value_text(value), TEXT);
    }

    // Campo hexadecimal: fondo y borde nuestros; el `TextEdit` va dentro y sin marco, porque los
    // márgenes de egui son enteros en puntos y no caen en píxeles físicos enteros a 1,5×.
    let id = hex_field_id();
    let field_y = field_y_u(mode);
    let focused = ui.memory(|memory| memory.has_focus(id));
    painter.rect(
        geo.rect_u(FIELD_X_U, field_y, FIELD_W_U, FIELD_H_U),
        0.0,
        SURFACE_0,
        Stroke::new(u, if focused { ACCENT } else { SURFACE_3 }),
        StrokeKind::Inside,
    );
    let inner = geo.rect_u(
        FIELD_X_U + FIELD_PAD_U,
        field_y + (FIELD_H_U - ROW_H_U) / 2,
        FIELD_W_U - 2 * FIELD_PAD_U,
        ROW_H_U,
    );
    let mut content = editor.hex_display().unwrap_or_else(|| hex_text(rgba));
    let before = content.clone();
    let response = ui.put(
        inner,
        TextEdit::singleline(&mut content)
            .id(id)
            .font(font.clone())
            .text_color(TEXT)
            .frame(Frame::NONE)
            .margin(Margin::ZERO)
            .desired_width(inner.width())
            .clip_text(true),
    );
    if response.changed() && content != before {
        editor.type_hex(target, rgba, content);
    }
    if response.lost_focus() {
        if events.iter().any(is_escape) {
            editor.escape_hex();
        } else {
            let enter = events.iter().any(is_enter);
            match editor.confirm_hex_checked(mode, !enter, stored) {
                Resolved::Done(outcome) => match outcome {
                    HexOutcome::Commit(commit) => output.commit = Some(commit),
                    HexOutcome::Invalid(_) if enter => response.request_focus(),
                    HexOutcome::Invalid(error) => {
                        output.notice = Some(hex_notice(&error).to_string());
                    }
                    HexOutcome::Nothing | HexOutcome::Unchanged => {}
                },
                Resolved::Stale(reason) => output.notice = Some(stale_notice(reason).to_string()),
            }
        }
    }

    // Aviso de dos líneas, o las de la quita pendiente.
    let notice = notice_y_u(mode);
    if let Some([first, second]) = &extras.prompt {
        text(HEADER_X_U, notice, first, TEXT);
        let warning = second.starts_with(NOTICE_PREFIX);
        let color = if warning { WARNING } else { TEXT_DIM };
        text(HEADER_X_U, notice + ROW_H_U, second, color);
    } else {
        let [first, second] = editor.notice(transparent);
        text(HEADER_X_U, notice, &first, WARNING);
        text(HEADER_X_U, notice + ROW_H_U, &second, WARNING);
    }

    // Botones de la paleta.
    if let Some(view) = extras.buttons {
        let pointer = ui.input(|input| input.pointer.hover_pos());
        let (first, second) = if view.removing {
            (PaletteButton::Confirm, PaletteButton::Cancel)
        } else {
            (PaletteButton::Add, PaletteButton::Remove)
        };
        for button in [
            first,
            second,
            PaletteButton::RotateLeft,
            PaletteButton::RotateRight,
            PaletteButton::Transparent,
        ] {
            let Some((x, y, width, height)) = button_rect_u(mode, button) else {
                continue;
            };
            let rect = geo.rect_u(x, y, width, height);
            let disabled = blocked_reason(button, view.len, view.has_destination).is_some();
            let hover = !disabled
                && pointer.is_some_and(|p| geo.panel_rect.contains(p) && rect.contains(p));
            let (fill, edge) = if hover {
                (SURFACE_3, ACCENT)
            } else {
                (SURFACE_2, SURFACE_3)
            };
            painter.rect(rect, 0.0, fill, Stroke::new(u, edge), StrokeKind::Inside);
            let color = if disabled { TEXT_DIM } else { TEXT };
            text(
                x + BUTTON_TEXT_X_U,
                y + BUTTON_TEXT_Y_U,
                if button == PaletteButton::Transparent {
                    view.transparent_label
                } else {
                    button_label(button)
                },
                color,
            );
        }
    }

    output
}
