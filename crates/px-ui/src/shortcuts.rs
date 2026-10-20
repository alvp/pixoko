//! Atajos de teclado como función pura (specs/hito-2-lapiz-deshacer.md, sección 5).

use egui::{Key, Modifiers};

use crate::actions::{AppAction, Platform};
use crate::key_events::press_from_key;
use crate::keymap::default_keymap;

/// Lo que pide un atajo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Undo,
    Redo,
    /// Ranura de color, de 1 a 9 (sin tecla por defecto desde la unidad I.1).
    SelectColor(u8),
    /// Color siguiente (`]` y `Av Pág`).
    ColorNext,
    /// Color anterior (`[` y `Re Pág`).
    ColorPrev,
}

// TODO(hito 8): `action_for_key`, `file_action_for_key` (y `tool::tool_action_for_key`) son vistas
// de la tabla de teclas (`keymap::Keymap`) que conservan su firma; eliminarlas cuando los
// llamadores migren a `Keymap::action_for`.

/// La acción de la tabla por defecto para una tecla tecleada (sin texto).
fn por_defecto(key: Key, modifiers: Modifiers) -> Option<AppAction> {
    default_keymap().action_for(&press_from_key(key, modifiers), Platform::Linux)
}

/// Vista de la tabla de teclas por defecto: deshacer (`command` + Z o `u`), rehacer (`command` + Y,
/// o `command` + Mayús + Z) y color siguiente y anterior (`]` y `Av Pág`, `[` y `Re Pág`). Un `Ctrl`
/// que no es `command` (macOS) no activa nada. `command` es Ctrl en Linux y Windows y Cmd en macOS
/// (egui lo resuelve). Todo lo demás: nada.
pub fn action_for_key(key: Key, modifiers: Modifiers) -> Option<Action> {
    if !modifiers.command && (modifiers.ctrl || modifiers.mac_cmd) {
        return None;
    }
    match por_defecto(key, modifiers)? {
        AppAction::Undo => Some(Action::Undo),
        AppAction::Redo => Some(Action::Redo),
        AppAction::ColorNext => Some(Action::ColorNext),
        AppAction::ColorPrev => Some(Action::ColorPrev),
        _ => None,
    }
}

/// Lo que pide un atajo de fichero.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileAction {
    Open,
    Save,
    SaveAs,
}

/// Vista de la tabla de teclas por defecto: `command` + S (sin Alt): guardar; con Mayús, guardar
/// como. `command` + O (sin Mayús ni Alt): abrir. `command` es Ctrl en Linux y Windows y Cmd en
/// macOS (en macOS, Ctrl + S y Ctrl + O no hacen nada).
pub fn file_action_for_key(key: Key, modifiers: Modifiers) -> Option<FileAction> {
    match por_defecto(key, modifiers)? {
        AppAction::Open => Some(FileAction::Open),
        AppAction::Save => Some(FileAction::Save),
        AppAction::SaveAs => Some(FileAction::SaveAs),
        _ => None,
    }
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
