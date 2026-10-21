//! Geometría del menú y de la caja de herramientas, sin tipos de egui
//! (specs/hito-2-menu-herramientas.md, «Geometría»; specs/interfaz-caja-derecha.md: la caja y el
//! panel van a la derecha del lienzo y la caja lleva el recuadro de opciones).
//!
//! Las medidas son enteros de «píxeles de interfaz» `u` (`n = text_scale(ppp)` píxeles físicos
//! cada una), igual que el panel de paletas y el editor de color.
//!
//! La caja es una rejilla de botones de icono de 24 × 24 u en cuatro columnas con 1 u de hueco
//! (specs/interfaz-iconos.md): `4·24 + 3·1 = 99 u`, el ancho que tenía el botón de texto, así que
//! ni la caja (107 u) ni el recuadro de opciones (99 u) ni los mínimos de ancho cambian.

use crate::icon_button::{BUTTON_U, GAP_U, PITCH_U};
use crate::palette_grid::PANEL_WIDTH_U;
use crate::status::bar_height_px;
use crate::view::ScreenRect;

/// Alto del menú, en `u`.
pub const MENU_H_U: i64 = 16;

/// Margen de la columna a cada lado de los botones, en `u`.
pub const TOOLBAR_MARGIN_U: i64 = 4;
/// Margen del texto del nivel de zoom respecto del botón, en `u`.
pub const BUTTON_PAD_U: i64 = 4;
/// Columnas de la rejilla de botones de la caja.
pub const TOOLBOX_COLUMNS: i64 = 4;
/// Filas de herramientas y casillas que dan (8: cinco herramientas y sitio para el pincel del
/// hito 4, sin casilla vacía visible).
pub const TOOL_ROWS: i64 = 2;
pub const TOOL_SLOTS: usize = (TOOLBOX_COLUMNS * TOOL_ROWS) as usize;
/// Botones de la caja: cinco herramientas y cuatro de zoom.
pub const TOOLBOX_BUTTONS: usize = 9;
/// `y` de la primera fila de herramientas (`4 + 25·fila`; la última termina en 53).
pub const TOOLS_Y_U: i64 = TOOLBAR_MARGIN_U;
/// `y` del separador de 1 u entre las herramientas y el zoom: 4 u bajo la última fila.
pub const TOOLBAR_SEPARATOR_Y_U: i64 = TOOLS_Y_U + TOOL_ROWS * PITCH_U - GAP_U + 4;
/// `y` del texto del nivel de zoom (14 u de alto): 4 u bajo el separador.
pub const ZOOM_LABEL_Y_U: i64 = TOOLBAR_SEPARATOR_Y_U + 1 + 4;
/// `y` de la fila de botones de zoom: 4 u bajo el texto (termina en 104).
pub const ZOOM_Y_U: i64 = ZOOM_LABEL_Y_U + 14 + 4;
/// Recuadro fijo de opciones de la herramienta activa, en `u` desde la esquina de la caja: 4 u bajo
/// la fila de zoom (termina en 104), del ancho de la rejilla de botones, con borde de 1 u (2 u con el
/// foco) y relleno; su alto no cambia con la herramienta ni con la ventana.
pub const OPTIONS_X_U: i64 = TOOLBAR_MARGIN_U;
pub const OPTIONS_Y_U: i64 = ZOOM_Y_U + BUTTON_U + 4;
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
/// Alto del contenido de la caja: el recuadro de opciones y un margen inferior de 4 u (250 u). Cabe
/// en el panel mínimo (305 u) con 55 u de holgura: una prueba lo guarda, y con cinco filas de
/// herramientas (17 a 20) dejaría de caber (hito 4).
pub const TOOLBAR_CONTENT_H_U: i64 = OPTIONS_Y_U + OPTIONS_H_U + 4;

/// Rótulos de los botones de herramienta (sin la letra del atajo), en el orden de la columna.
pub const TOOL_LABELS: [&str; 5] = ["Lápiz", "Línea", "Relleno", "Cuentagotas", "Borrador"];

/// Ancho de la rejilla de botones de la columna, en `u`: las columnas, sus botones y los huecos
/// entre ellos (99 u).
pub fn toolbar_button_width_u() -> i64 {
    TOOLBOX_COLUMNS * BUTTON_U + (TOOLBOX_COLUMNS - 1) * GAP_U
}

/// Esquina `(x, y)` de la casilla `slot` de la rejilla de herramientas, en `u` desde la esquina de
/// la caja.
pub fn tool_slot_u(slot: usize) -> (i64, i64) {
    let slot = slot as i64;
    (
        TOOLBAR_MARGIN_U + PITCH_U * (slot % TOOLBOX_COLUMNS),
        TOOLS_Y_U + PITCH_U * (slot / TOOLBOX_COLUMNS),
    )
}

/// Rectángulo `(x, y, ancho, alto)` del botón `index` de la caja, en `u` desde su esquina: del 0 al
/// 4 las herramientas (casillas 0 a 4) y del 5 al 8 el zoom (una fila).
///
/// # Panics
/// Si `index` es 9 o más.
pub fn toolbox_button_rect_u(index: usize) -> (i64, i64, i64, i64) {
    assert!(index < TOOLBOX_BUTTONS, "la caja tiene {TOOLBOX_BUTTONS} botones");
    let (x, y) = if index < TOOL_LABELS.len() {
        tool_slot_u(index)
    } else {
        (
            TOOLBAR_MARGIN_U + PITCH_U * (index - TOOL_LABELS.len()) as i64,
            ZOOM_Y_U,
        )
    };
    (x, y, BUTTON_U, BUTTON_U)
}

/// El botón de la caja bajo un punto en píxeles físicos, relativo a la esquina de la caja.
pub fn toolbox_hit(x_px: i64, y_px: i64, n: u32) -> Option<usize> {
    let n = scale(n);
    (0..TOOLBOX_BUTTONS).find(|&index| {
        let (x, y, w, h) = toolbox_button_rect_u(index);
        x_px >= x * n && x_px < (x + w) * n && y_px >= y * n && y_px < (y + h) * n
    })
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
