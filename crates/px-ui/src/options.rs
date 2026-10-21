//! Panel fijo de opciones de la herramienta activa (specs/interfaz-caja-derecha.md, decisiones 4 a
//! 6), sin tipos de egui: el texto de hoy, el desplazamiento por filas y el foco.
//!
//! Hoy ninguna herramienta tiene opciones: el recuadro enseña el nombre, el atajo de la tabla y
//! `sin opciones`. El hito 4 amplía [`options_row_count`] (y el cuerpo) sin tocar el resto.

use crate::actions::{AppAction, Platform};
use crate::chrome::OPTIONS_BODY_ROWS;
use crate::keymap::Keymap;
use crate::tool::Tool;

/// Aviso del cuerpo cuando la herramienta no tiene opciones.
pub const NO_OPTIONS: &str = "sin opciones";

/// Filas del cuerpo que desplaza una muesca de la rueda.
pub const SCROLL_ROWS_PER_NOTCH: i64 = 3;

/// Filas de opciones de `tool`: 0 para las cinco herramientas de hoy. Es el único sitio que el
/// hito 4 amplía.
pub fn options_row_count(_tool: Tool) -> usize {
    0
}

/// Mayor desplazamiento del cuerpo, en filas, con `rows` filas de opciones.
pub fn options_max_scroll(rows: usize) -> usize {
    rows.saturating_sub(OPTIONS_BODY_ROWS as usize)
}

/// Título del recuadro: el nombre de la herramienta, el mismo rótulo que su botón.
pub fn title(tool: Tool) -> &'static str {
    match tool {
        Tool::Pencil => "Lápiz",
        Tool::Line => "Línea",
        Tool::Fill => "Relleno",
        Tool::Dropper => "Cuentagotas",
        Tool::Eraser => "Borrador",
    }
}

/// Línea del atajo, leída de la tabla: `tecla d`, o `sin tecla` si la tabla no le da ninguno.
pub fn key_line(tool: Tool, keymap: &Keymap, platform: Platform) -> String {
    match keymap.shown_text(AppAction::SelectTool(tool), platform) {
        Some(text) => format!("tecla {text}"),
        None => "sin tecla".to_string(),
    }
}

/// Estado del recuadro: el foco (un clic derecho en una herramienta lo da) y el desplazamiento del
/// cuerpo en filas. Sin controles, el foco no captura el teclado.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OptionsState {
    pub focused: bool,
    pub scroll_rows: usize,
    tool: Tool,
}

impl OptionsState {
    /// Da el foco al recuadro.
    pub fn focus(&mut self) {
        self.focused = true;
    }

    /// Quita el foco (un diálogo que se abre).
    pub fn blur(&mut self) {
        self.focused = false;
    }

    /// Una pulsación fuera del recuadro quita el foco. En el fotograma de un clic derecho en una
    /// herramienta se aplica **antes** de `focus`.
    pub fn press_outside(&mut self) {
        self.focused = false;
    }

    /// Anota la herramienta activa: si cambió (por cualquier vía) el desplazamiento vuelve a 0 y
    /// el foco se pierde. El foco del clic derecho se da **después** de llamar a esto.
    pub fn sync_tool(&mut self, tool: Tool) {
        if tool != self.tool {
            self.tool = tool;
            self.scroll_rows = 0;
            self.focused = false;
        }
    }

    /// Desplaza el cuerpo `delta` filas (positivo = hacia abajo), recortado a `0..=max` con `rows`
    /// filas de opciones.
    pub fn scroll_by(&mut self, delta: i64, rows: usize) {
        let max = options_max_scroll(rows) as i64;
        self.scroll_rows = (self.scroll_rows as i64 + delta).clamp(0, max) as usize;
    }
}
