//! Geometría del editor de color, sin tipos de egui (specs/hito-2-editar-color.md).
//!
//! Las medidas son constantes en «píxeles de interfaz» `u` (`n = text_scale(ppp)` píxeles físicos
//! cada una) desde la esquina superior izquierda del editor, que empieza en `top_px` (anclado al
//! fondo del panel). Las funciones trabajan en píxeles físicos con `n`.

use px_core::Mode;

use crate::color_edit::{Control, controls};

/// Franja de 1 u en la parte superior del editor.
pub const SEPARATOR_Y_U: i64 = 0;
/// Cabecera: origen del texto.
pub const HEADER_X_U: i64 = 4;
pub const HEADER_Y_U: i64 = 4;
/// Alto de una fila de texto y de deslizador.
pub const ROW_H_U: i64 = 14;
/// Primera fila de deslizadores.
pub const FIRST_ROW_Y_U: i64 = 22;
/// Hueco entre grupos de deslizadores (RGB | HSL | A).
pub const GROUP_GAP_U: i64 = 4;
/// Dentro de una fila: letra, pista, marca, valor y zona activa.
pub const LABEL_X_U: i64 = 4;
pub const LABEL_W_U: i64 = 7;
pub const TRACK_X_U: i64 = 15;
pub const TRACK_W_U: i64 = 184;
pub const TRACK_H_U: i64 = 4;
pub const THUMB_W_U: i64 = 3;
pub const THUMB_H_U: i64 = 12;
pub const VALUE_X_U: i64 = 203;
pub const VALUE_W_U: i64 = 21;
/// Zona activa de la fila: de `HIT_X0_U` (incluido) a `HIT_X1_U` (excluido).
pub const HIT_X0_U: i64 = 13;
pub const HIT_X1_U: i64 = 201;
/// Campo hexadecimal.
pub const FIELD_X_U: i64 = 4;
pub const FIELD_W_U: i64 = 69;
pub const FIELD_H_U: i64 = 18;
pub const FIELD_PAD_U: i64 = 3;
/// Hueco entre bloques (controles, campo y aviso) y margen inferior.
pub const BLOCK_GAP_U: i64 = 4;
/// Líneas del aviso.
pub const NOTICE_LINES: i64 = 2;
/// Cabecera: texto de aviso de «ventana baja».
pub const EDITOR_HIDDEN_NOTICE: &str = "ventana baja: editor oculto";

pub fn editor_height_u(mode: Mode) -> i64 {
    match mode {
        Mode::Gray => 94,
        Mode::GrayAlpha => 112,
        _ => 186,
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EditorLayout {
    pub top_px: i64,
    pub reserved_px: i64,
}
pub fn editor_layout(mode: Mode, panel_h_px: i64, n: u32) -> Option<EditorLayout> {
    let n = i64::from(n.max(1));
    if panel_h_px < (41 + 14 + editor_height_u(mode)) * n {
        return None;
    }
    let top = (panel_h_px - editor_height_u(mode) * n).div_euclid(n) * n;
    Some(EditorLayout {
        top_px: top,
        reserved_px: panel_h_px - top,
    })
}
pub fn control_row_u(mode: Mode, control: Control) -> Option<i64> {
    let idx = controls(mode).iter().position(|c| *c == control)?;
    let mut y = FIRST_ROW_Y_U;
    for (i, c) in controls(mode).iter().enumerate().take(idx + 1) {
        if i > 0 {
            let prev = controls(mode)[i - 1];
            let gap = matches!(
                (prev, *c),
                (Control::Blue, Control::Hue)
                    | (Control::Lightness, Control::Alpha)
                    | (Control::Gray, Control::Alpha)
            );
            y += ROW_H_U + if gap { GROUP_GAP_U } else { 0 };
        }
    }
    Some(y)
}
pub fn slider_hit(mode: Mode, x_px: i64, y_px: i64, n: u32) -> Option<Control> {
    let n = i64::from(n.max(1));
    if x_px < HIT_X0_U * n || x_px >= HIT_X1_U * n {
        return None;
    }
    controls(mode).iter().copied().find(|c| {
        control_row_u(mode, *c).is_some_and(|r| y_px >= r * n && y_px < (r + ROW_H_U) * n)
    })
}
pub fn field_y_u(mode: Mode) -> i64 {
    let last_row = controls(mode)
        .last()
        .and_then(|last| control_row_u(mode, *last))
        .unwrap_or(FIRST_ROW_Y_U);
    last_row + ROW_H_U + BLOCK_GAP_U
}
pub fn notice_y_u(mode: Mode) -> i64 {
    field_y_u(mode) + FIELD_H_U + BLOCK_GAP_U
}
pub fn edges_u(mode: Mode) -> Vec<i64> {
    let mut e = vec![
        0,
        1,
        HEADER_X_U,
        HEADER_Y_U,
        HEADER_Y_U + ROW_H_U,
        TRACK_X_U,
        TRACK_X_U + TRACK_W_U,
        VALUE_X_U,
        VALUE_X_U + VALUE_W_U,
        FIELD_X_U,
        FIELD_X_U + FIELD_W_U,
        field_y_u(mode),
        field_y_u(mode) + FIELD_H_U,
        notice_y_u(mode),
        notice_y_u(mode) + ROW_H_U,
        notice_y_u(mode) + 2 * ROW_H_U,
        editor_height_u(mode),
    ];
    for row in controls(mode)
        .iter()
        .filter_map(|c| control_row_u(mode, *c))
    {
        e.extend([row, row + ROW_H_U, row + 5, row + 9, row + 1, row + 13]);
    }
    e
}
