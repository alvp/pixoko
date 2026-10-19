//! Capa de acciones única: el menú, la columna de herramientas, los botones de zoom y los atajos
//! de teclado producen la misma [`AppAction`] y obedecen las mismas reglas de bloqueo
//! (specs/hito-2-menu-herramientas.md, decisiones 2 a 5). Todo es lógica pura, sin ventana.

use egui::{Key, Modifiers};

use crate::background::CanvasBackground;
use crate::shortcuts::{Action, FileAction, action_for_key, file_action_for_key};
use crate::tool::{Tool, ToolAction, tool_action_for_key};
use crate::view::Zoom;

/// Lo que se puede pedir a la app, venga de donde venga.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppAction {
    /// `Archivo → Nueva imagen…` (specs/hito-2-nueva-imagen.md).
    NewImage,
    Open,
    Save,
    SaveAs,
    Quit,
    Undo,
    Redo,
    ZoomIn,
    ZoomOut,
    /// Ajustar a la ventana.
    ZoomFit,
    /// 1:1.
    ZoomActual,
    ShowLicenses,
    CloseLicenses,
    SelectTool(Tool),
    /// Ranura de color, de 1 a 9.
    SelectColor(u8),
    /// Fondo bajo la transparencia del lienzo: preferencia de la sesión, no del documento.
    CanvasBackground(CanvasBackground),
}

impl AppAction {
    /// Los atajos de fichero y `Salir` no repiten con la tecla mantenida: confirmarían solos.
    pub fn ignores_repeat(self) -> bool {
        matches!(
            self,
            AppAction::NewImage
                | AppAction::Open
                | AppAction::Save
                | AppAction::SaveAs
                | AppAction::Quit
        )
    }

    /// Las acciones que abren un diálogo modal (las licencias y nueva imagen; la 20 añade la
    /// suya): no se abren con un gesto en curso ni con una quita pendiente.
    pub fn opens_dialog(self) -> bool {
        matches!(self, AppAction::ShowLicenses | AppAction::NewImage)
    }

    /// Las cuatro acciones de zoom, que no modifican el documento.
    pub fn is_zoom(self) -> bool {
        matches!(
            self,
            AppAction::ZoomIn | AppAction::ZoomOut | AppAction::ZoomFit | AppAction::ZoomActual
        )
    }
}

/// De dónde viene una acción. No cambia el efecto; solo cuenta para el bloqueo del zoom con un
/// campo de texto enfocado.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    Menu,
    Button,
    Key,
}

/// Sistema operativo, para los atajos y su texto.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Platform {
    Linux,
    Windows,
    Mac,
}

impl Platform {
    /// El sistema para el que se compiló.
    pub fn current() -> Platform {
        if cfg!(target_os = "macos") {
            Platform::Mac
        } else if cfg!(target_os = "windows") {
            Platform::Windows
        } else {
            Platform::Linux
        }
    }
}

/// Por qué una acción puede estar bloqueada en este fotograma.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Locks {
    /// Un trazo de una herramienta, el deslizador del editor o el arrastre de una muestra.
    pub gesture: bool,
    /// Un diálogo de archivo abierto.
    pub dialog: bool,
    /// Un campo de texto con el foco.
    pub text_focus: bool,
    /// Una quita de la paleta pendiente de confirmar.
    pub removal: bool,
    /// Un diálogo modal abierto.
    pub modal: bool,
}

/// `true` si `action`, pedida desde `origin`, no debe ejecutarse con estos bloqueos.
///
/// En este orden: (1) `CloseLicenses` nunca; (2) con `modal`, todo salvo `Quit`; (3) con `dialog`,
/// todo; (4) el zoom solo se bloquea con `text_focus` y desde el teclado (no modifica el
/// documento: un gesto o una quita pendiente no lo paran); (5) el resto, con `gesture` o
/// `text_focus`, y con `removal` solo `SelectColor` y las que abren un diálogo; (6) si no, nada. El fondo del lienzo
/// (`CanvasBackground`) tampoco toca el documento: tras (1) a (3) solo lo bloquea `text_focus`, con
/// cualquier origen; un gesto o una quita pendiente no.
pub fn is_blocked(action: AppAction, locks: Locks, origin: Origin) -> bool {
    if action == AppAction::CloseLicenses {
        return false;
    }
    if locks.modal && action != AppAction::Quit {
        return true;
    }
    if locks.dialog {
        return true;
    }
    if matches!(action, AppAction::CanvasBackground(_)) {
        return locks.text_focus;
    }
    if action.is_zoom() {
        return locks.text_focus && origin == Origin::Key;
    }
    if locks.gesture || locks.text_focus {
        return true;
    }
    locks.removal && (matches!(action, AppAction::SelectColor(_)) || action.opens_dialog())
}

/// Si se puede abrir un diálogo modal: ni un gesto en curso, ni una quita pendiente, ni un diálogo
/// de archivo, ni un campo de texto enfocado, ni otro modal.
pub fn can_open_dialog(locks: Locks) -> bool {
    !(locks.gesture || locks.dialog || locks.text_focus || locks.removal || locks.modal)
}

/// Lo que la interfaz necesita saber para habilitar o deshabilitar el menú y la columna.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UiState {
    pub can_undo: bool,
    pub can_redo: bool,
    /// Zoom del lienzo, o `None` si no hay imagen que mostrar.
    pub zoom: Option<Zoom>,
    pub tool: Tool,
    pub locks: Locks,
}

/// Si la entrada del menú o el botón (que actúan con `Origin::Menu`) está habilitada.
pub fn enabled(action: AppAction, state: &UiState) -> bool {
    if is_blocked(action, state.locks, Origin::Menu) {
        return false;
    }
    match action {
        AppAction::Undo => state.can_undo,
        AppAction::Redo => state.can_redo,
        AppAction::ZoomIn => state.zoom.is_some_and(|zoom| zoom < Zoom::MAX),
        AppAction::ZoomOut => state.zoom.is_some_and(|zoom| zoom > Zoom::MIN),
        AppAction::ZoomFit => state.zoom.is_some(),
        AppAction::ZoomActual => state.zoom.is_some_and(|zoom| zoom != Zoom::ONE),
        _ => true,
    }
}

/// Traduce una tecla pulsada y sus modificadores. Compone `file_action_for_key`, las teclas de
/// herramienta, `action_for_key` y las nuevas:
///
/// - `command` + `1` (sin Mayús ni Alt): 1:1. `command` + `Q` (sin Mayús ni Alt): salir, **solo en
///   Linux**: en Windows `Alt`+`F4` llega como petición de cierre y en macOS `Cmd`+`Q` lo gestiona
///   el sistema.
/// - `+`, `=`, `-` y `Inicio` sin `command` (con cualquier otro modificador): zoom.
///
/// `Escape` no es una acción: la ordena el lienzo con los eventos de trazo.
pub fn action_for_shortcut(
    key: Key,
    modifiers: Modifiers,
    platform: Platform,
) -> Option<AppAction> {
    if let Some(action) = file_action_for_key(key, modifiers) {
        return Some(match action {
            FileAction::Open => AppAction::Open,
            FileAction::Save => AppAction::Save,
            FileAction::SaveAs => AppAction::SaveAs,
        });
    }
    if let Some(ToolAction::Select(tool)) = tool_action_for_key(key, modifiers) {
        return Some(AppAction::SelectTool(tool));
    }
    if let Some(action) = action_for_key(key, modifiers) {
        return Some(match action {
            Action::Undo => AppAction::Undo,
            Action::Redo => AppAction::Redo,
            Action::SelectColor(slot) => AppAction::SelectColor(slot),
        });
    }
    if modifiers.command {
        if modifiers.alt || modifiers.shift {
            return None;
        }
        return match key {
            Key::Num1 => Some(AppAction::ZoomActual),
            Key::N => Some(AppAction::NewImage),
            Key::Q if platform == Platform::Linux => Some(AppAction::Quit),
            _ => None,
        };
    }
    match key {
        Key::Plus | Key::Equals => Some(AppAction::ZoomIn),
        Key::Minus => Some(AppAction::ZoomOut),
        Key::Home => Some(AppAction::ZoomFit),
        _ => None,
    }
}
