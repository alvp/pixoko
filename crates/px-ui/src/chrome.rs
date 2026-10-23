//! Geometría del menú, la columna única y la caja de herramientas, sin tipos de egui
//! (specs/hito-2-menu-herramientas.md, «Geometría»; specs/ui-single-column.md: una sola columna a
//! la derecha del lienzo, con la caja arriba y la paleta debajo, de ancho ajustable).
//!
//! Las medidas son enteros de «píxeles de interfaz» `u` (`n = text_scale(ppp)` píxeles físicos
//! cada una), igual que el panel de paletas y el editor de color.
//!
//! La columna mide `W(k) = 14·k + 9` u con `k` columnas de muestras de paleta (4 u de margen, `14k`
//! de rejilla, 5 u de pista y línea). La caja es una rejilla de botones de icono de 24 × 24 u con 1 u
//! de hueco, de `c(k) = ⌊(14k + 2)/25⌋` por fila; con `r` filas de herramientas mide `25r + 58` u
//! y la paleta empieza en `head = 25r + 59` u.

use crate::icon_button::{BUTTON_U, GAP_U, PITCH_U};
use crate::status::bar_height_px;
use crate::view::ScreenRect;

/// Alto del menú, en `u`.
pub const MENU_H_U: i64 = 16;

/// Margen de la columna a cada lado de los botones, en `u`.
pub const TOOLBAR_MARGIN_U: i64 = 4;
/// Margen del texto del nivel de zoom respecto del botón, en `u`.
pub const BUTTON_PAD_U: i64 = 4;
/// Botones de la caja: cinco herramientas y cuatro de zoom.
pub const TOOLBOX_BUTTONS: usize = 9;
/// `y` de la primera fila de herramientas.
pub const TOOLS_Y_U: i64 = TOOLBAR_MARGIN_U;

/// Columnas de muestras mínimas, por defecto y las que necesita el editor de color (233 u).
pub const COLUMN_MIN_COLS: i64 = 8;
pub const COLUMN_DEFAULT_COLS: i64 = 16;
pub const COLUMN_EDITOR_COLS: i64 = 16;
/// Zona de arrastre del borde izquierdo de la columna, dentro de ella, en `u`.
pub const COLUMN_GRIP_U: i64 = 4;
/// Ancho de la barra de menús: la suma de sus seis títulos (`chars·7 + 12` u cada uno).
pub const MENU_BAR_W_U: i64 = 289;

/// Rótulos de los botones de herramienta (sin la letra del atajo), en el orden de la columna.
pub const TOOL_LABELS: [&str; 5] = ["Lápiz", "Línea", "Relleno", "Cuentagotas", "Borrador"];

fn scale(n: u32) -> i64 {
    i64::from(n.max(1))
}

/// Ancho de la columna con `cols` columnas de muestras, en `u`: `14·cols + 9`.
pub fn column_width_u(cols: i64) -> i64 {
    14 * cols + 9
}

/// Mayor número de columnas de muestras que deja a una ventana de `window_w_px` píxeles físicos su
/// lienzo mínimo (128 u): `max(8, ⌊(w − 137·n)/(14·n)⌋)`.
pub fn max_cols(window_w_px: i64, n: u32) -> i64 {
    let n = scale(n);
    ((window_w_px - 137 * n).div_euclid(14 * n)).max(COLUMN_MIN_COLS)
}

/// Las columnas con que se pinta la columna: lo elegido (`wanted`) recortado a `[8, max_cols]`. Lo
/// elegido no se toca: al ensanchar la ventana vuelve el ancho elegido. Es la única función que
/// recorta.
pub fn column_cols(window_w_px: i64, n: u32, wanted: i64) -> i64 {
    wanted.clamp(COLUMN_MIN_COLS, max_cols(window_w_px, n))
}

/// Lo elegido, ensanchado a las columnas que necesita el editor de color mientras esté abierto o haya
/// una quita pendiente (`wide`).
pub fn wide_cols(wanted: i64, wide: bool) -> i64 {
    if wide {
        wanted.max(COLUMN_EDITOR_COLS)
    } else {
        wanted
    }
}

/// Botones de 24 u con 1 u de hueco que caben en una fila de una columna de `cols` columnas, con 4 u
/// de margen a cada lado: `⌊(14·cols + 2)/25⌋`.
pub fn columns_per_row(cols: i64) -> i64 {
    ((14 * cols + 2) / (BUTTON_U + GAP_U)).max(1)
}

/// Filas de herramientas que ocupan `tools` botones con `cols` columnas.
pub fn tool_rows(tools: usize, cols: i64) -> i64 {
    let per_row = columns_per_row(cols);
    ((tools as i64 + per_row - 1) / per_row).max(1)
}

/// Rectángulo `(x, y, ancho, alto)` del botón `index` de la caja, en `u` desde su esquina: los
/// primeros `tools` son las herramientas, en filas de `c(k)`, y los cuatro siguientes el zoom, en
/// una sola fila debajo.
///
/// # Panics
/// Si `index` es `tools + 4` o más.
pub fn toolbox_button_rect_u_at(index: usize, tools: usize, cols: i64) -> (i64, i64, i64, i64) {
    assert!(index < tools + 4, "la caja tiene {} botones", tools + 4);
    let (x, y) = if index < tools {
        let per_row = columns_per_row(cols);
        let i = index as i64;
        (
            TOOLBAR_MARGIN_U + PITCH_U * (i % per_row),
            TOOLS_Y_U + PITCH_U * (i / per_row),
        )
    } else {
        (
            TOOLBAR_MARGIN_U + PITCH_U * (index - tools) as i64,
            zoom_row_y_u(tools, cols),
        )
    };
    (x, y, BUTTON_U, BUTTON_U)
}

/// `y` del separador de 1 u entre las herramientas y el zoom: `25r + 7`.
pub fn toolbox_separator_y_u(tools: usize, cols: i64) -> i64 {
    PITCH_U * tool_rows(tools, cols) + 7
}

/// `y` del texto del nivel de zoom (14 u de alto): `25r + 12`.
pub fn zoom_label_y_u(tools: usize, cols: i64) -> i64 {
    PITCH_U * tool_rows(tools, cols) + 12
}

/// `y` de la fila de botones de zoom: `25r + 30`.
pub fn zoom_row_y_u(tools: usize, cols: i64) -> i64 {
    PITCH_U * tool_rows(tools, cols) + 30
}

/// Alto de la caja, con su margen inferior de 4 u: `25r + 58`.
pub fn toolbox_height_u(tools: usize, cols: i64) -> i64 {
    PITCH_U * tool_rows(tools, cols) + 58
}

/// Dónde empieza la paleta, bajo un separador de 1 u: `25r + 59`.
pub fn head_u(tools: usize, cols: i64) -> i64 {
    toolbox_height_u(tools, cols) + 1
}

/// El botón de la caja bajo un punto en píxeles físicos, relativo a la esquina de la caja, con
/// `tools` herramientas y `cols` columnas.
pub fn toolbox_hit_at(x_px: i64, y_px: i64, n: u32, tools: usize, cols: i64) -> Option<usize> {
    let n = scale(n);
    (0..tools + 4).find(|&index| {
        let (x, y, w, h) = toolbox_button_rect_u_at(index, tools, cols);
        x_px >= x * n && x_px < (x + w) * n && y_px >= y * n && y_px < (y + h) * n
    })
}

/// Rectángulo del botón `index` de la caja de hoy (cinco herramientas y 16 columnas).
///
/// # Panics
/// Si `index` es 9 o más.
pub fn toolbox_button_rect_u(index: usize) -> (i64, i64, i64, i64) {
    assert!(
        index < TOOLBOX_BUTTONS,
        "la caja tiene {TOOLBOX_BUTTONS} botones"
    );
    toolbox_button_rect_u_at(index, TOOL_LABELS.len(), COLUMN_DEFAULT_COLS)
}

/// El botón de la caja de hoy bajo un punto en píxeles físicos, relativo a la esquina de la caja.
pub fn toolbox_hit(x_px: i64, y_px: i64, n: u32) -> Option<usize> {
    toolbox_hit_at(x_px, y_px, n, TOOL_LABELS.len(), COLUMN_DEFAULT_COLS)
}

/// Rectángulos de las cinco piezas de la ventana, en píxeles físicos enteros. `toolbar` y `panel`
/// son las dos partes apiladas de la columna única: la caja arriba y la paleta debajo, con el mismo
/// `x` y el mismo ancho.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WindowLayout {
    pub menu: ScreenRect,
    pub toolbar: ScreenRect,
    pub panel: ScreenRect,
    pub canvas: ScreenRect,
    pub status: ScreenRect,
}

/// Origen del lienzo en la ventana, en píxeles físicos: en el borde izquierdo, bajo el menú (la
/// columna va a su derecha).
pub fn canvas_origin_px(n: u32) -> (i64, i64) {
    (0, MENU_H_U * scale(n))
}

/// Como [`window_layout_cols`] con las 16 columnas de hoy (233 u).
pub fn window_layout(window_px: (i64, i64), n: u32) -> WindowLayout {
    window_layout_cols(window_px, n, COLUMN_DEFAULT_COLS)
}

/// Reparte una ventana de `window_px` píxeles físicos: menú arriba y barra de estado abajo (todo el
/// ancho) y, en la franja central, el lienzo (el resto) y la columna única en el borde derecho, de
/// `column_cols(wanted)` columnas, con la caja arriba y la paleta debajo. Es la **única** función
/// que conoce ese orden. Los cinco rectángulos recubren la ventana sin huecos ni solapes (con una
/// ventana menor que el mínimo, los tamaños no negativos se recortan a lo que queda).
pub fn window_layout_cols(window_px: (i64, i64), n: u32, wanted: i64) -> WindowLayout {
    let k = scale(n);
    let (w, h) = window_px;
    let menu_h = MENU_H_U * k;
    let bar_h = i64::from(bar_height_px(n));
    let band = (h - menu_h - bar_h).max(0);
    let cols = column_cols(w, n, wanted);
    let column_w = (column_width_u(cols) * k).min(w.max(0));
    let head = (head_u(TOOL_LABELS.len(), cols) * k).min(band);
    let rect = |x, y, width, height| ScreenRect {
        x,
        y,
        width,
        height,
    };
    WindowLayout {
        menu: rect(0, 0, w, menu_h),
        toolbar: rect(w - column_w, menu_h, column_w, head),
        panel: rect(w - column_w, menu_h + head, column_w, band - head),
        canvas: rect(0, menu_h, w - column_w, band),
        status: rect(0, h - bar_h, w, bar_h),
    }
}
