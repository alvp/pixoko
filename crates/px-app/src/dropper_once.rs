//! El cuentagotas del indicador de color es de un solo uso (specs/interfaz-dos-colores.md, visto
//! bueno 6): el clic en el indicador lo elige, y al soltar su primer gesto vuelve la herramienta
//! anterior. Elegir otra herramienta antes del gesto lo cancela. Lógica pura, sin ventana.

use px_ui::color::Role;
use px_ui::tool::Tool;

/// Estado del cuentagotas de un solo uso.
#[derive(Debug, Default)]
pub struct DropperOnce {
    /// La herramienta a la que se vuelve; `None`: no hay retorno pendiente.
    previous: Option<Tool>,
    /// El primer gesto del cuentagotas ya empezó.
    started: bool,
    /// The role of the bar that was pressed (specs/ui-color-block.md, decision 7): while this use
    /// lasts, the chosen color goes to that role with either button.
    role: Option<Role>,
}

impl DropperOnce {
    pub fn new() -> DropperOnce {
        DropperOnce::default()
    }

    /// The bar of `role` was pressed with `current` as the active tool. With the dropper already
    /// chosen the previous tool, if any, is not lost (and if there was none, it stays where it is).
    pub fn arm(&mut self, current: Tool, role: Role) {
        if current != Tool::Dropper {
            self.previous = Some(current);
        }
        self.started = false;
        self.role = Some(role);
    }

    /// The role of the bar that armed the dropper, while that use lasts.
    #[allow(dead_code)] // C4: wired in the logic commit
    pub fn role(&self) -> Option<Role> {
        self.role
    }

    /// The role that receives the chosen color: the bar's if a use is armed and, if not, the one of
    /// the button that was pressed.
    #[allow(dead_code)] // C4: wired in the logic commit
    pub fn role_for(&self, _button: Role) -> Role {
        todo!("C4: role_for")
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
