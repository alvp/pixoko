//! Panel de licencias de terceros (specs/hito-2-menu-herramientas.md, decisión 7): un texto de
//! solo lectura que sustituye al lienzo, con rueda para desplazarlo y un botón `Cerrar`.
//!
//! El ajuste del texto a las columnas que caben es una función pura; el panel solo lo pinta.

use egui::{Align2, Event, FontFamily, FontId, Key, Rect, Sense, Ui, pos2, vec2};

use crate::chrome::BUTTON_PAD_U;
use crate::edit_layout::{BUTTON_H_U, BUTTON_TEXT_Y_U, ROW_H_U};
use crate::status::{ADVANCE_PX, columns};
use crate::text::{font_size_points, printable, text_scale, ui_unit};
use crate::theme::{SURFACE_0, SURFACE_2, SURFACE_3, TEXT};

/// Lo que devuelve el panel en un fotograma.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LicensesOutput {
    /// Se pidió cerrar el panel (`Escape` o el botón `Cerrar`).
    pub close: bool,
}

/// Margen del panel, en `u`.
const MARGIN_U: i64 = 4;
/// Líneas que avanza una muesca de la rueda.
const LINES_PER_NOTCH: f32 = 3.0;
const CLOSE_LABEL: &str = "Cerrar";

/// Ajusta `text` a `columns` caracteres por línea. Corta por el último espacio de la línea si lo
/// hay (con algo delante) y, si no, en seco. Conserva las líneas vacías. Con `columns = 0` no hay
/// sitio y da vacío.
pub fn wrap_text(text: &str, columns: usize) -> Vec<String> {
    let mut out = Vec::new();
    if columns == 0 {
        return out;
    }
    for line in text.lines() {
        let mut rest: Vec<char> = line.chars().collect();
        if rest.is_empty() {
            out.push(String::new());
            continue;
        }
        while rest.len() > columns {
            let cut = (1..=columns)
                .rev()
                .find(|&i| rest[i] == ' ' && rest[..i].iter().any(|c| *c != ' '));
            match cut {
                Some(i) => {
                    let piece: String = rest[..i].iter().collect();
                    out.push(piece.trim_end().to_string());
                    let skip = rest[i..].iter().take_while(|c| **c == ' ').count();
                    rest.drain(..i + skip);
                }
                None => {
                    out.push(rest[..columns].iter().collect());
                    rest.drain(..columns);
                }
            }
        }
        if !rest.is_empty() {
            out.push(rest.iter().collect());
        }
    }
    out
}

/// Recorta la primera línea visible a `0..=total − visibles`.
pub fn clamp_first_line(first: usize, total: usize, visible: usize) -> usize {
    first.min(total.saturating_sub(visible))
}

/// Pinta el panel sobre todo el espacio libre de `ui` y devuelve si hay que cerrarlo.
/// `first_line` es la primera línea envuelta que se ve: la rueda la mueve y se recorta a lo que
/// queda. Todo lo pintado se recorta al rectángulo del panel.
pub fn show_licenses(ui: &mut Ui, text: &str, first_line: &mut usize) -> LicensesOutput {
    let ctx = ui.ctx().clone();
    let ppp = ctx.pixels_per_point();
    let n = i64::from(text_scale(ppp));
    let u = ui_unit(ppp);
    let font = FontId::new(font_size_points(ppp), FontFamily::Proportional);
    let rect = ui.available_rect_before_wrap();
    let response = ui.allocate_rect(rect, Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, SURFACE_0);

    let at = |x_u: i64, y_u: i64| rect.min + vec2(x_u as f32 * u, y_u as f32 * u);
    let mut output = LicensesOutput::default();

    // Botón `Cerrar`, arriba a la izquierda.
    let label_chars = CLOSE_LABEL.chars().count() as i64;
    let button_w = 2 * BUTTON_PAD_U + label_chars * i64::from(ADVANCE_PX);
    let button = Rect::from_min_size(
        at(MARGIN_U, MARGIN_U),
        vec2(button_w as f32 * u, BUTTON_H_U as f32 * u),
    );
    let click = ui.interact(button, response.id.with("cerrar"), Sense::click());
    painter.rect_filled(
        button,
        0.0,
        if click.hovered() {
            SURFACE_3
        } else {
            SURFACE_2
        },
    );
    painter.text(
        pos2(
            button.min.x + BUTTON_PAD_U as f32 * u,
            button.min.y + BUTTON_TEXT_Y_U as f32 * u,
        ),
        Align2::LEFT_TOP,
        CLOSE_LABEL,
        font.clone(),
        TEXT,
    );
    if click.clicked() {
        output.close = true;
    }

    // Texto envuelto a las columnas que caben.
    let text_top_u = 2 * MARGIN_U + BUTTON_H_U;
    let width_px = rect.width() * ppp - (2 * MARGIN_U * n) as f32;
    let lines = wrap_text(text, columns(width_px, ppp));
    let height_px = (rect.height() * ppp).round() as i64;
    let visible = ((height_px - (text_top_u + MARGIN_U) * n) / (ROW_H_U * n)).max(0) as usize;

    let (hovering, close_key, notches) = ui.input(|input| {
        let hovering = input.pointer.hover_pos().is_some_and(|p| rect.contains(p));
        let close_key = input.events.iter().any(|event| {
            matches!(
                event,
                Event::Key { key: Key::Escape, pressed: true, modifiers, .. } if modifiers.is_none()
            )
        });
        let notches: f32 = input
            .events
            .iter()
            .filter_map(|event| match event {
                Event::MouseWheel { unit, delta, .. } => Some(match unit {
                    egui::MouseWheelUnit::Line => -delta.y * LINES_PER_NOTCH,
                    egui::MouseWheelUnit::Point => -delta.y * ppp / (ROW_H_U * n) as f32,
                    egui::MouseWheelUnit::Page => -delta.y * visible as f32,
                }),
                _ => None,
            })
            .sum();
        (hovering, close_key, notches)
    });
    if close_key {
        output.close = true;
    }
    if hovering && notches != 0.0 {
        let moved = notches.round() as i64;
        let moved = if moved == 0 {
            notches.signum() as i64
        } else {
            moved
        };
        *first_line = if moved >= 0 {
            first_line.saturating_add(moved as usize)
        } else {
            first_line.saturating_sub(moved.unsigned_abs() as usize)
        };
    }
    *first_line = clamp_first_line(*first_line, lines.len(), visible);

    for (row, line) in lines.iter().skip(*first_line).take(visible).enumerate() {
        painter.text(
            at(MARGIN_U, text_top_u + ROW_H_U * row as i64),
            Align2::LEFT_TOP,
            printable(&ctx, line),
            font.clone(),
            TEXT,
        );
    }
    output
}
