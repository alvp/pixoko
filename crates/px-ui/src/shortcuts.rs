//! Atajos de teclado como función pura (specs/hito-2-lapiz-deshacer.md, sección 5).

use egui::{Key, Modifiers};

/// Lo que pide un atajo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Undo,
    Redo,
    /// Ranura de color, de 1 a 9.
    SelectColor(u8),
}

/// Traduce una tecla pulsada y sus modificadores. `command` es Ctrl en Linux y Windows y Cmd
/// en macOS (egui lo resuelve).
///
/// - `command` + Z (sin Mayús ni Alt): deshacer.
/// - `command` + Y, o `command` + Mayús + Z (sin Alt): rehacer.
/// - Teclas 1 a 9 sin ningún modificador: elegir esa ranura de color.
/// - Todo lo demás: nada.
pub fn action_for_key(key: Key, modifiers: Modifiers) -> Option<Action> {
    if modifiers.command {
        if modifiers.alt {
            return None;
        }
        return match (key, modifiers.shift) {
            (Key::Z, false) => Some(Action::Undo),
            (Key::Z, true) | (Key::Y, false) => Some(Action::Redo),
            _ => None,
        };
    }
    if !modifiers.is_none() {
        return None;
    }
    let slot = match key {
        Key::Num1 => 1,
        Key::Num2 => 2,
        Key::Num3 => 3,
        Key::Num4 => 4,
        Key::Num5 => 5,
        Key::Num6 => 6,
        Key::Num7 => 7,
        Key::Num8 => 8,
        Key::Num9 => 9,
        _ => return None,
    };
    Some(Action::SelectColor(slot))
}

/// Lo que pide un atajo de fichero.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileAction {
    Save,
    SaveAs,
}

/// `command` + S (sin Alt): guardar; con Mayús, guardar como. `command` es Ctrl en Linux y
/// Windows y Cmd en macOS (como en `action_for_key`; en macOS, Ctrl + S no hace nada). Función
/// aparte de `action_for_key` para no alterar su contrato (specs/hito-2-abrir-guardar.md).
pub fn file_action_for_key(key: Key, modifiers: Modifiers) -> Option<FileAction> {
    if key != Key::S || !modifiers.command || modifiers.alt {
        return None;
    }
    Some(if modifiers.shift {
        FileAction::SaveAs
    } else {
        FileAction::Save
    })
}

/// Eventos que pueden ver los atajos de teclado: todos, o ninguno si un campo de texto tiene el
/// foco (`wants_keyboard`), para que teclear en él no cambie de herramienta, de color ni de
/// vista (specs/hito-2-editar-color.md, decisión 11). Conserva el orden.
pub fn gate_keys(events: &[egui::Event], wants_keyboard: bool) -> Vec<egui::Event> {
    if wants_keyboard {
        Vec::new()
    } else {
        events.to_vec()
    }
}
