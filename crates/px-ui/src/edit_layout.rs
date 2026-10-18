//! Geometría del editor de color, sin tipos de egui (specs/hito-2-editar-color.md).
//!
//! Las medidas son constantes en «píxeles de interfaz» `u` (`n = text_scale(ppp)` píxeles físicos
//! cada una) desde la esquina superior izquierda del editor, que empieza en `top_px` (anclado al
//! fondo del panel). Las funciones trabajan en píxeles físicos con `n`.

use px_core::Mode;

use crate::color_edit::{Control, controls};
use crate::palette_grid::{GRID_Y_U, PANEL_WIDTH_U, PITCH_U, visible_rows_reserved};
use crate::status::bar_height_px;
use crate::text::text_scale;

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
/// Anchura mínima del lienzo en la ventana, en píxeles de interfaz
/// (specs/hito-2-panel-ventana-baja.md).
pub const MIN_CANVAS_WIDTH_U: i64 = 128;
/// Líneas del aviso.
pub const NOTICE_LINES: i64 = 2;
/// Modos de imagen: la altura del editor depende de ellos.
const MODES: [Mode; 4] = [Mode::Rgba, Mode::Indexed, Mode::Gray, Mode::GrayAlpha];

/// Alto del editor del modo, en `u`: hasta el final del aviso más el margen inferior. Es el
/// **único** sitio donde se calcula (specs/hito-2-panel-ventana-baja.md): todo lo demás cuelga de
/// ella.
pub fn editor_height_u(mode: Mode) -> i64 {
    notice_y_u(mode) + NOTICE_LINES * ROW_H_U + BLOCK_GAP_U
}

/// El mayor alto de editor entre los modos.
pub fn editor_height_u_max() -> i64 {
    MODES
        .iter()
        .map(|mode| editor_height_u(*mode))
        .max()
        .unwrap_or(0)
}

fn scale(n: u32) -> i64 {
    i64::from(n.max(1))
}

/// Alto del bloque `[texto · una fila de rejilla · editor]` del modo, en píxeles físicos: el
/// alto del panel a partir del cual no hay desplazamiento.
pub fn block_height_px(mode: Mode, n: u32) -> i64 {
    (GRID_Y_U + PITCH_U + editor_height_u(mode)) * scale(n)
}

/// Alto mínimo del panel, en píxeles físicos, para cualquier modo.
pub fn min_panel_height_px(n: u32) -> i64 {
    (GRID_Y_U + PITCH_U + editor_height_u_max()) * scale(n)
}

/// Alto mínimo de la ventana en píxeles lógicos (puntos de egui): la barra de estado más el
/// panel mínimo.
pub fn min_window_height_pt(ppp: f32) -> f32 {
    let ppp = if ppp.is_finite() && ppp > 0.0 {
        ppp
    } else {
        1.0
    };
    let n = text_scale(ppp);
    let physical = i64::from(bar_height_px(n)) + min_panel_height_px(n);
    physical as f32 / ppp
}

/// Ancho mínimo de la ventana en píxeles de interfaz: el panel más el lienzo mínimo.
pub fn min_window_width_u() -> i64 {
    PANEL_WIDTH_U + MIN_CANVAS_WIDTH_U
}

/// Ancho mínimo de la ventana en píxeles lógicos (puntos de egui).
pub fn min_window_width_pt(ppp: f32) -> f32 {
    let ppp = if ppp.is_finite() && ppp > 0.0 {
        ppp
    } else {
        1.0
    };
    let n = text_scale(ppp);
    (min_window_width_u() * scale(n)) as f32 / ppp
}

/// Reparto vertical del panel: dónde va el editor y cuántas filas ve la rejilla.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PanelLayout {
    /// El panel es más bajo que su bloque y se desplaza entero.
    pub scrolling: bool,
    /// Desplazamiento aplicado: 0 si no hay, si no un múltiplo de `n` en `0..=max_offset_px`.
    pub offset_px: i64,
    pub max_offset_px: i64,
    /// Borde superior del editor desde el del panel, en píxeles físicos (puede ser negativo).
    pub top_px: i64,
    pub visible_rows: usize,
}

/// Reparto del panel de `panel_h_px` con el editor del modo. Con alto suficiente, el editor se
/// ancla al fondo y la rejilla usa el resto; con menos, el bloque se desplaza `offset_px` (se
/// recorta a lo que cabe) y la rejilla enseña una fila. Con `offset_px = 0` y `alto = bloque`
/// los dos repartos coinciden.
pub fn panel_layout(mode: Mode, panel_h_px: i64, n: u32, offset_px: i64) -> PanelLayout {
    let n = scale(n);
    let height = editor_height_u(mode) * n;
    let block = block_height_px(mode, n as u32);
    if panel_h_px >= block {
        let top = panel_h_px.saturating_sub(height).div_euclid(n) * n;
        return PanelLayout {
            scrolling: false,
            offset_px: 0,
            max_offset_px: 0,
            top_px: top,
            visible_rows: visible_rows_reserved(panel_h_px, panel_h_px - top, n as u32),
        };
    }
    let max = block.saturating_sub(panel_h_px).div_euclid(n) * n;
    let offset = offset_px.clamp(0, max).div_euclid(n) * n;
    PanelLayout {
        scrolling: true,
        offset_px: offset,
        max_offset_px: max,
        top_px: (GRID_Y_U + PITCH_U) * n - offset,
        visible_rows: 1,
    }
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
