//! Geometría de los diálogos modales, sin tipos de egui (specs/hito-2-dialogos-modales.md,
//! «Geometría»). Las medidas son enteros de «píxeles de interfaz» `u` (`n = text_scale(ppp)`
//! píxeles físicos cada una); el resultado, rectángulos en píxeles físicos enteros.

use crate::edit_layout::{ROW_H_U, TEXT_BUTTON_H_U};
use crate::view::ScreenRect;

/// Ancho del diálogo: 8 de relleno + 308 de contenido (44 columnas) + 2 de hueco + 4 de barra de
/// desplazamiento + 8.
pub const DIALOG_W_U: i64 = 330;
/// Relleno interior, a cada lado.
pub const PAD_U: i64 = 8;
/// Contenido (texto) del diálogo: 44 columnas de 7 u.
pub const CONTENT_W_U: i64 = 308;
/// Hueco entre el contenido y la barra de desplazamiento.
pub const SCROLLBAR_GAP_U: i64 = 2;
/// Ancho de la barra de desplazamiento.
pub const SCROLLBAR_W_U: i64 = 4;
/// Margen mínimo entre el diálogo y la ventana, a cada lado.
pub const MARGIN_U: i64 = 8;
/// Título: texto de 14 u desde `y = 3` y 1 u de línea al pie.
pub const TITLE_H_U: i64 = 20;
/// Pie: 1 de línea + 4 + motivo (14) + 2 + botones (16) + 8.
pub const FOOTER_H_U: i64 = 45;
/// `y` del motivo dentro del pie (la línea y el hueco).
pub const REASON_Y_U: i64 = 5;
/// `y` de los botones dentro del pie.
pub const BUTTONS_Y_U: i64 = 21;
/// Ancho de un botón: 8 caracteres de 7 u y 6 u de margen a cada lado.
pub const BUTTON_W_U: i64 = 68;
/// Hueco entre `Accept` y `Cancel`.
pub const BUTTON_GAP_U: i64 = 4;
/// Alto del campo numérico y del hexadecimal.
pub const FIELD_H_U: i64 = 18;
/// Alto de la línea de error bajo un campo.
pub const ERROR_H_U: i64 = 14;
/// Hueco entre dos bloques del cuerpo.
pub const BLOCK_GAP_U: i64 = 4;
/// Alto de una casilla o de una opción.
pub const CHOICE_ROW_H_U: i64 = 16;
/// Hueco entre dos opciones de un grupo.
pub const CHOICE_GAP_U: i64 = 2;
/// Columnas de texto de una zona de solo lectura.
pub const ZONE_COLS: usize = 42;
/// Relleno vertical y borde de una zona: `2·1` de borde y `2·2` de relleno.
pub const ZONE_FRAME_U: i64 = 6;
/// Filas del contenido natural de una zona `Fill`.
pub const FILL_ROWS: i64 = 22;
/// Contenido natural de un diálogo con una zona `Fill`: relleno, marco y 22 filas.
pub const FILL_CONTENT_H_U: i64 = 2 * PAD_U + ZONE_FRAME_U + FILL_ROWS * ROW_H_U;

/// Los rectángulos del diálogo, en píxeles físicos enteros.
#[derive(Clone, Debug, PartialEq)]
pub struct DialogLayout {
    pub dialog: ScreenRect,
    pub title: ScreenRect,
    pub body: ScreenRect,
    pub footer: ScreenRect,
    /// La línea del motivo, en el pie.
    pub reason: ScreenRect,
    pub accept: ScreenRect,
    pub cancel: Option<ScreenRect>,
    /// Desplazamiento máximo del cuerpo, en `u` (0 si cabe o si es una zona `Fill`).
    pub max_scroll_u: i64,
}

/// Coloca el diálogo, centrado, en una ventana de `window_px` píxeles físicos. `content_h_u` es el
/// alto del contenido del cuerpo (`Dialog::content_h_u`); con `fill`, el natural. El ancho es
/// `min(330, width_2 de la window at u − 16)` y el alto `min(65 + content, height_2 at u − 16)`: cabe
/// siempre y, si el contenido no cabe, se desplaza solo el cuerpo.
pub fn dialog_layout(
    window_px: (i64, i64),
    n: u32,
    content_h_u: i64,
    fill: bool,
    has_cancel: bool,
) -> DialogLayout {
    let k = i64::from(n.max(1));
    let (window_w, window_h) = (window_px.0.max(0), window_px.1.max(0));
    let content = if fill {
        FILL_CONTENT_H_U
    } else {
        content_h_u.max(0)
    };
    let width_u = DIALOG_W_U.min((window_w / k - 2 * MARGIN_U).max(0));
    let natural_u = TITLE_H_U + content + FOOTER_H_U;
    let height_u = natural_u.min((window_h / k - 2 * MARGIN_U).max(0));
    let (width, height) = (width_u * k, height_u * k);
    let x = (window_w - width).div_euclid(2);
    let y = (window_h - height).div_euclid(2);
    let rect = |x, y, width, height| ScreenRect {
        x,
        y,
        width,
        height,
    };

    let title_u = TITLE_H_U.min(height_u);
    let footer_u = FOOTER_H_U.min(height_u - title_u);
    let body_u = height_u - title_u - footer_u;
    let title = rect(x, y, width, title_u * k);
    let body = rect(x, y + title_u * k, width, body_u * k);
    let footer = rect(x, y + (title_u + body_u) * k, width, footer_u * k);

    let reason_w = (width_u - 2 * PAD_U).clamp(0, CONTENT_W_U);
    let reason = rect(
        x + PAD_U * k,
        footer.y + REASON_Y_U * k,
        reason_w * k,
        ROW_H_U * k,
    );
    let buttons_y = footer.y + BUTTONS_Y_U * k;
    let right = x + width - PAD_U * k;
    let button = |left: i64| rect(left, buttons_y, BUTTON_W_U * k, TEXT_BUTTON_H_U * k);
    let (accept, cancel) = if has_cancel {
        let cancel = button(right - BUTTON_W_U * k);
        let accept = button(cancel.x - (BUTTON_GAP_U + BUTTON_W_U) * k);
        (accept, Some(cancel))
    } else {
        (button(right - BUTTON_W_U * k), None)
    };

    DialogLayout {
        dialog: rect(x, y, width, height),
        title,
        body,
        footer,
        reason,
        accept,
        cancel,
        max_scroll_u: if fill { 0 } else { (content - body_u).max(0) },
    }
}

/// Filas de texto que caben en una caja de zona de `box_h_u` (el alto menos el marco de 6 u).
pub fn zone_rows(box_h_u: i64) -> usize {
    if box_h_u < ZONE_FRAME_U {
        0
    } else {
        ((box_h_u - ZONE_FRAME_U) / ROW_H_U) as usize
    }
}

/// El desplazamiento mínimo que deja a la vista el elemento `[top_u, bottom_u)` (coordenadas del
/// contenido) en una vista de `view_h_u` que empieza en `scroll_u`. Si el elemento es más alto que
/// la vista, su borde superior.
pub fn ensure_visible(scroll_u: i64, top_u: i64, bottom_u: i64, view_h_u: i64) -> i64 {
    if bottom_u - top_u >= view_h_u || top_u < scroll_u {
        top_u
    } else if bottom_u > scroll_u + view_h_u {
        bottom_u - view_h_u
    } else {
        scroll_u
    }
}
