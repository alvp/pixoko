//! Altura mínima de la ventana (specs/hito-2-panel-ventana-baja.md): con ella el panel siempre
//! cabe sin desplazarse. Sin egui: se prueba sin ventana.

use px_ui::edit_layout::min_window_height_pt;

/// Variable de entorno solo para pruebas: `PIXOKO_SIN_MINIMO=1` desactiva el mínimo (para probar
/// el desplazamiento del panel sin un gestor de ventanas en mosaico).
pub const ENV_NO_MIN: &str = "PIXOKO_SIN_MINIMO";

/// Ancho mínimo, en puntos: ninguna restricción.
const MIN_WIDTH_PT: f32 = 1.0;

/// El mínimo está activo salvo con el valor `1` de [`ENV_NO_MIN`].
pub fn min_enabled(value: Option<&str>) -> bool {
    value != Some("1")
}

/// Tamaño interior mínimo `[ancho, alto]` en píxeles lógicos para `ppp`, o `None` si está
/// desactivado.
pub fn min_inner_size(ppp: f32, enabled: bool) -> Option<[f32; 2]> {
    enabled.then(|| [MIN_WIDTH_PT, min_window_height_pt(ppp)])
}

#[cfg(test)]
mod tests;
