//! Tamaño mínimo de la ventana (specs/hito-2-panel-ventana-baja.md): con la altura mínima el
//! panel siempre cabe sin desplazarse, y con el ancho mínimo queda sitio para el lienzo. Sin egui:
//! se prueba sin ventana.

use px_ui::edit_layout::{min_window_height_pt, min_window_width_pt};

/// Variable de entorno solo para pruebas: `PIXOKO_NO_MIN_SIZE=1` desactiva el mínimo, de alto y de
/// ancho (para probar el desplazamiento del panel sin un gestor de ventanas en mosaico).
pub const ENV_NO_MIN: &str = "PIXOKO_NO_MIN_SIZE";

/// El mínimo está activo salvo con el valor `1` de [`ENV_NO_MIN`].
pub fn min_enabled(value: Option<&str>) -> bool {
    value != Some("1")
}

/// Tamaño interior mínimo `[ancho, alto]` en píxeles lógicos para `ppp`, o `None` si está
/// desactivado.
pub fn min_inner_size(ppp: f32, enabled: bool) -> Option<[f32; 2]> {
    enabled.then(|| [min_window_width_pt(ppp), min_window_height_pt(ppp)])
}

#[cfg(test)]
mod tests;
