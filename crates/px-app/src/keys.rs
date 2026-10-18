//! Teclas de herramienta con el estado de la app (specs/hito-2-cuentagotas-borrador.md,
//! decisiones 2 y 5). Sin egui: se prueba sin ventana.

use px_core::Image;
use px_ui::pick::eraser_pixel;
use px_ui::tool::{Tool, ToolAction, ToolState};

/// Aplica la acción de una tecla de herramienta. `busy` es `App::busy()` completo (gesto, deslizador
/// del editor, arrastre de muestra o diálogo): con él, o con un gesto de la herramienta en curso,
/// no hace nada. Si la herramienta pasa a ser el borrador y la imagen no admite borrar, devuelve
/// el aviso de 5 s (la herramienta queda elegida igualmente); en cualquier otro caso, `None`.
pub fn select_tool(
    tools: &mut ToolState,
    action: ToolAction,
    busy: bool,
    image: Option<&Image>,
) -> Option<String> {
    if busy || tools.busy() {
        return None;
    }
    tools.apply_action(action);
    match (action, image) {
        (ToolAction::Select(Tool::Eraser), Some(image)) => {
            eraser_pixel(image).err().map(|error| error.to_string())
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests;
