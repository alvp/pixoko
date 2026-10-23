//! Barra de estado: una línea de texto abajo de la ventana (specs/hito-2-texto.md).
//!
//! Alto fijo de `20·n` píxeles físicos (14·n de línea y 3·n de margen arriba y abajo), el texto
//! que no cabe se recorta con nuestro `fit_text` y todo lo que viene de fuera pasa por
//! `replace_missing`.

use egui::{Align2, Context, FontFamily, FontId, Frame, Panel, Rect, Sense, Ui, pos2, vec2};

use crate::text::{
    fit_text, font_size_points, replace_missing, text_scale, ui_unit, with_glyph_check,
};
use crate::theme::{NOTICE_PREFIX, SURFACE_1, SURFACE_3, TEXT, WARNING};

/// Píxeles físicos de avance de un carácter en la escala 1 (la fuente es monoespaciada).
pub const ADVANCE_PX: u32 = 7;
/// Píxeles físicos de alto de una línea en la escala 1.
const LINE_PX: u32 = 14;
/// Margen de la barra, en píxeles de interfaz.
const PADDING_UI_PX: u32 = 3;

/// Alto de la barra en píxeles físicos con el factor `n`: `20·n`.
pub fn bar_height_px(n: u32) -> u32 {
    (LINE_PX + 2 * PADDING_UI_PX) * n
}

/// Caracteres que caben en `width_px` píxeles físicos: `floor(width / (7·n))`. Un ancho no
/// finito o negativo da 0.
pub fn columns(width_px: f32, ppp: f32) -> usize {
    if !(width_px.is_finite() && width_px > 0.0) {
        return 0;
    }
    let advance = (ADVANCE_PX * text_scale(ppp)) as f32;
    (width_px / advance).floor() as usize
}

/// Caracteres que caben en la barra de estado de una ventana de `width_pt` puntos de ancho (la
/// barra ocupa todo el ancho y deja un margen a cada lado).
pub fn bar_columns(width_pt: f32, ppp: f32) -> usize {
    let pad = PADDING_UI_PX as f32 * ui_unit(ppp);
    columns((width_pt - 2.0 * pad) * ppp, ppp)
}

/// Texto de la barra: con aviso, antepone `NOTICE_PREFIX`; lo recorta a `columns` caracteres
/// (`…` si la fuente lo tiene, `...` si no) y sustituye lo que la fuente no dibuja.
pub fn bar_text(ctx: &Context, text: &str, is_notice: bool, columns: usize) -> String {
    let full = if is_notice {
        format!("{NOTICE_PREFIX}{text}")
    } else {
        text.to_string()
    };
    with_glyph_check(ctx, |has| {
        replace_missing(&fit_text(&full, columns, has), has)
    })
}

/// Pinta la barra en el borde inferior de `ui`. Hay que llamarla **antes** de lo que ocupa el
/// resto de la ventana (el lienzo se queda con el rectángulo que sobra).
pub fn show(ui: &mut Ui, text: &str, is_notice: bool) {
    let ctx = ui.ctx().clone();
    let ppp = ctx.pixels_per_point();
    let n = text_scale(ppp);
    let u = ui_unit(ppp);
    let height = bar_height_px(n) as f32 / ppp;
    let pad = PADDING_UI_PX as f32 * u;
    let color = if is_notice { WARNING } else { TEXT };
    Panel::bottom("estado")
        .exact_size(height)
        .resizable(false)
        .show_separator_line(false)
        .frame(Frame::new().fill(SURFACE_1))
        .show(ui, |ui| {
            let (rect, _) =
                ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
            let shown = bar_text(
                &ctx,
                text,
                is_notice,
                columns((rect.width() - 2.0 * pad) * ppp, ppp),
            );
            let painter = ui.painter();
            painter.rect_filled(
                Rect::from_min_size(rect.min, vec2(rect.width(), u)),
                0.0,
                SURFACE_3,
            );
            painter.text(
                pos2(rect.min.x + pad, rect.min.y + pad),
                Align2::LEFT_TOP,
                shown,
                FontId::new(font_size_points(ppp), FontFamily::Proportional),
                color,
            );
        });
}
