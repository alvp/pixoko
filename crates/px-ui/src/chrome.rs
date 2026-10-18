//! Geometría del menú y de la columna de herramientas, sin tipos de egui
//! (specs/hito-2-menu-herramientas.md, «Geometría»).
//!
//! Las medidas son enteros de «píxeles de interfaz» `u` (`n = text_scale(ppp)` píxeles físicos
//! cada una), igual que el panel de paletas y el editor de color.

use crate::status::ADVANCE_PX;

/// Alto del menú, en `u`.
pub const MENU_H_U: i64 = 16;

/// Margen de la columna a cada lado de los botones, en `u`.
const TOOLBAR_MARGIN_U: i64 = 4;
/// Margen del texto dentro de un botón de la columna, en `u`, a cada lado.
const BUTTON_PAD_U: i64 = 4;

/// Rótulos de los botones de herramienta (sin la letra del atajo), en el orden de la columna.
pub const TOOL_LABELS: [&str; 5] = ["Lápiz", "Línea", "Relleno", "Cuentagotas", "Borrador"];

/// Ancho de un botón de la columna, en `u`: margen, el rótulo más largo, un hueco y la letra del
/// atajo (a `ADVANCE_PX` de avance por carácter) y margen.
pub fn toolbar_button_width_u() -> i64 {
    let longest = TOOL_LABELS
        .iter()
        .map(|label| label.chars().count())
        .max()
        .unwrap_or(0) as i64;
    BUTTON_PAD_U + (longest + 2) * i64::from(ADVANCE_PX) + BUTTON_PAD_U
}

/// Ancho de la columna de herramientas, en `u`: el botón más un margen a cada lado.
pub fn toolbar_width_u() -> i64 {
    2 * TOOLBAR_MARGIN_U + toolbar_button_width_u()
}
