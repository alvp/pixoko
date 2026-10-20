//! Geometría del menú y de la caja de herramientas, sin tipos de egui
//! (specs/hito-2-menu-herramientas.md, «Geometría»; specs/interfaz-caja-derecha.md: la caja y el
//! panel van a la derecha del lienzo y la caja lleva el recuadro de opciones).
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
/// Recuadro fijo de opciones de la herramienta activa, en `u` desde la esquina de la caja: 4 u bajo
/// el último botón de zoom (termina en 189), del ancho de un botón, con borde de 1 u (2 u con el
/// foco) y relleno; su alto no cambia con la herramienta ni con la ventana.
pub const OPTIONS_X_U: i64 = TOOLBAR_MARGIN_U;
pub const OPTIONS_Y_U: i64 = 193;
pub const OPTIONS_W_U: i64 = 99;
pub const OPTIONS_H_U: i64 = 138;
/// Grosor del borde del recuadro, sin y con el foco.
pub const OPTIONS_BORDER_U: i64 = 1;
pub const OPTIONS_FOCUS_BORDER_U: i64 = 2;
/// Título (nombre de la herramienta) y línea del atajo, de 14 u, con 4 u de margen en el recuadro.
pub const OPTIONS_TITLE_X_U: i64 = OPTIONS_X_U + 4;
pub const OPTIONS_TITLE_Y_U: i64 = OPTIONS_Y_U + 4;
pub const OPTIONS_KEY_Y_U: i64 = OPTIONS_TITLE_Y_U + 14;
/// Cuerpo: 13 columnas de 7 u por 7 filas de 14 u. Con más filas de las que caben, se desplaza por
/// filas enteras.
pub const OPTIONS_ROW_H_U: i64 = 14;
pub const OPTIONS_BODY_ROWS: i64 = 7;
pub const OPTIONS_BODY_X_U: i64 = OPTIONS_TITLE_X_U;
pub const OPTIONS_BODY_Y_U: i64 = OPTIONS_KEY_Y_U + 14 + 4;
pub const OPTIONS_BODY_W_U: i64 = OPTIONS_W_U - 8;
pub const OPTIONS_BODY_H_U: i64 = OPTIONS_BODY_ROWS * OPTIONS_ROW_H_U;
/// Alto del contenido de la caja: el recuadro de opciones y un margen inferior de 4 u. Cabe en el
/// panel mínimo (351 u): una prueba lo guarda (I.4 liberará altura y habrá que reevaluarlo).
pub const TOOLBAR_CONTENT_H_U: i64 = OPTIONS_Y_U + OPTIONS_H_U + 4;

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

/// Origen del lienzo en la ventana, en píxeles físicos: en el borde izquierdo, bajo el menú (la
/// caja y el panel van a su derecha).
pub fn canvas_origin_px(n: u32) -> (i64, i64) {
    (0, MENU_H_U * scale(n))
}

/// Reparte una ventana de `window_px` píxeles físicos: menú arriba y barra de estado abajo (todo el
/// ancho) y, en la franja central, de izquierda a derecha, el lienzo (el resto), el panel de paletas
/// y la caja de herramientas en el borde derecho. Es la **única** función que conoce ese orden. Los cinco
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
        toolbar: rect(w - toolbar_w, menu_h, toolbar_w, band),
        panel: rect(w - toolbar_w - panel_w, menu_h, panel_w, band),
        canvas: rect(0, menu_h, w - toolbar_w - panel_w, band),
        status: rect(0, h - bar_h, w, bar_h),
    }
}
