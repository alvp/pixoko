//! Geometría del menú y de la columna de herramientas, sin tipos de egui
//! (specs/hito-2-menu-herramientas.md, «Geometría»).
//!
//! Las medidas son enteros de «píxeles de interfaz» `u` (`n = text_scale(ppp)` píxeles físicos
//! cada una), igual que el panel de paletas y el editor de color.

use crate::edit_layout::BUTTON_H_U;
use crate::palette_grid::PANEL_WIDTH_U;
use crate::status::{ADVANCE_PX, bar_height_px};
use crate::view::ScreenRect;

/// Alto del menú, en `u`.
pub const MENU_H_U: i64 = 16;

/// Margen de la columna a cada lado de los botones, en `u`.
pub const TOOLBAR_MARGIN_U: i64 = 4;
/// Margen del texto dentro de un botón de la columna, en `u`, a cada lado.
pub const BUTTON_PAD_U: i64 = 4;
/// Paso vertical entre botones de la columna: el alto del botón y 2 u de hueco.
pub const TOOLBAR_STEP_U: i64 = BUTTON_H_U + 2;
/// `y` del primer botón de herramienta (`4 + 18·i`; el último termina en 92).
pub const TOOLS_Y_U: i64 = TOOLBAR_MARGIN_U;
/// `y` del separador de 1 u entre las herramientas y el zoom.
pub const TOOLBAR_SEPARATOR_Y_U: i64 = 96;
/// `y` del texto del nivel de zoom (14 u de alto).
pub const ZOOM_LABEL_Y_U: i64 = 101;
/// `y` del primer botón de zoom (`119 + 18·i`; el último termina en 189).
pub const ZOOM_Y_U: i64 = 119;
/// Alto del contenido de la columna: hasta el último botón de zoom y un margen inferior de 4 u.
pub const TOOLBAR_CONTENT_H_U: i64 = 193;

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

/// Rectángulos de las cinco piezas de la ventana, en píxeles físicos enteros.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WindowLayout {
    pub menu: ScreenRect,
    pub toolbar: ScreenRect,
    pub panel: ScreenRect,
    pub canvas: ScreenRect,
    pub status: ScreenRect,
}

fn scale(n: u32) -> i64 {
    i64::from(n.max(1))
}

/// Origen del lienzo en la ventana, en píxeles físicos: tras la columna y el panel, bajo el menú.
pub fn canvas_origin_px(n: u32) -> (i64, i64) {
    let n = scale(n);
    ((toolbar_width_u() + PANEL_WIDTH_U) * n, MENU_H_U * n)
}

/// Reparte una ventana de `window_px` píxeles físicos: menú arriba y barra de estado abajo (todo el
/// ancho) y, en la franja central, la columna, el panel de paletas y el lienzo (el resto). Los cinco
/// rectángulos recubren la ventana sin huecos ni solapes (con una ventana menor que el mínimo, los
/// tamaños no negativos se recortan a lo que queda).
pub fn window_layout(window_px: (i64, i64), n: u32) -> WindowLayout {
    let k = scale(n);
    let (w, h) = window_px;
    let menu_h = MENU_H_U * k;
    let bar_h = i64::from(bar_height_px(n));
    let band = (h - menu_h - bar_h).max(0);
    let toolbar_w = toolbar_width_u() * k;
    let panel_w = PANEL_WIDTH_U * k;
    let rect = |x, y, width, height| ScreenRect {
        x,
        y,
        width,
        height,
    };
    WindowLayout {
        menu: rect(0, 0, w, menu_h),
        toolbar: rect(0, menu_h, toolbar_w, band),
        panel: rect(toolbar_w, menu_h, panel_w, band),
        canvas: rect(toolbar_w + panel_w, menu_h, w - toolbar_w - panel_w, band),
        status: rect(0, h - bar_h, w, bar_h),
    }
}
