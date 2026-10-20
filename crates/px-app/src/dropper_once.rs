//! El cuentagotas del indicador de color es de un solo uso (specs/interfaz-dos-colores.md, visto
//! bueno 6): el clic en el indicador lo elige, y al soltar su primer gesto vuelve la herramienta
//! anterior. Elegir otra herramienta antes del gesto lo cancela. Lógica pura, sin ventana.

use px_ui::tool::Tool;

/// Estado del cuentagotas de un solo uso.
#[derive(Debug, Default)]
pub struct DropperOnce {
    /// La herramienta a la que se vuelve; `None`: no hay retorno pendiente.
    previous: Option<Tool>,
    /// El primer gesto del cuentagotas ya empezó.
    started: bool,
}

impl DropperOnce {
    pub fn new() -> DropperOnce {
        DropperOnce::default()
    }

    /// Se pulsó el indicador con `current` como herramienta activa. Con el cuentagotas ya elegido
    /// no se pierde la anterior que hubiera (y si no había, se queda donde está).
    pub fn arm(&mut self, current: Tool) {
        if current != Tool::Dropper {
            self.previous = Some(current);
        }
        self.started = false;
    }

    /// Olvida el retorno pendiente.
    pub fn cancel(&mut self) {
        *self = DropperOnce::default();
    }

    /// Se llama cada fotograma con la herramienta activa y si hay un gesto en curso. Devuelve la
    /// herramienta a la que hay que volver, una sola vez, al soltar el primer gesto.
    pub fn update(&mut self, tool: Tool, busy: bool) -> Option<Tool> {
        let previous = self.previous?;
        if tool != Tool::Dropper {
            self.cancel();
            return None;
        }
        if busy {
            self.started = true;
            return None;
        }
        if self.started {
            self.cancel();
            return Some(previous);
        }
        None
    }
}

#[cfg(test)]
mod tests;
