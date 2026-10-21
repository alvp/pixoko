//! Capa de acciones única: el menú, la columna de herramientas, los botones de zoom y los atajos
//! de teclado producen la misma [`AppAction`] y obedecen las mismas reglas de bloqueo
//! (specs/hito-2-menu-herramientas.md, decisiones 2 a 5). Todo es lógica pura, sin ventana.

use egui::{Key, Modifiers};

use crate::background::CanvasBackground;
use crate::help::HelpTarget;
use crate::key_events::press_from_key;
use crate::keymap::default_keymap;
use crate::tool::Tool;
use crate::view::Zoom;

/// Lo que se puede pedir a la app, venga de donde venga.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppAction {
    /// `Archivo → Nueva imagen…` (specs/hito-2-nueva-imagen.md).
    NewImage,
    /// `Imagen → Convertir modo…` (specs/hito-2-convertir-modo.md): sin atajo.
    ConvertMode,
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
    /// Ranura de color, de 1 a 9 (sin tecla por defecto: asignable en el hito 8).
    SelectColor(u8),
    /// Color siguiente (`]`): la entrada en indexado, la ranura en los demás modos.
    ColorNext,
    /// Color anterior (`[`).
    ColorPrev,
    /// Centra el área bajo el cursor (`n`): necesita el puntero, no tiene entrada de menú.
    CenterOnCursor,
    /// Color de fondo siguiente (`}` y `Mayús`+`Av Pág`), como `ColorNext` pero para el fondo.
    BackgroundNext,
    /// Color de fondo anterior (`{` y `Mayús`+`Re Pág`).
    BackgroundPrev,
    /// `Editar → Borrar página` (`K`): rellena toda la imagen con el color de fondo, en un paso.
    ClearPage,
    /// Fondo bajo la transparencia del lienzo: preferencia de la sesión, no del documento.
    CanvasBackground(CanvasBackground),
    /// `Ver → Fondo del lienzo → Color…` (specs/hito-2-fondo-color.md): abre un diálogo.
    BackgroundColor,
    /// `F1` o `Ayuda → Ayuda de la herramienta` (specs/interfaz-ayuda.md): abre la ayuda del
    /// destino (la herramienta activa o el botón de la caja bajo el puntero).
    ShowHelp(HelpTarget),
    /// `Ayuda → Atajos de teclado…`: abre la lista de atajos.
    ShowShortcuts,
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
                | AppAction::ShowHelp(_)
        )
    }

    /// Las acciones que abren un diálogo modal (las licencias, nueva imagen, convertir modo, el
    /// color del fondo, la ayuda y la lista de atajos): no se abren con un gesto en curso ni con una quita pendiente.
    pub fn opens_dialog(self) -> bool {
        matches!(
            self,
            AppAction::ShowLicenses
                | AppAction::NewImage
                | AppAction::ConvertMode
                | AppAction::BackgroundColor
                | AppAction::ShowHelp(_)
                | AppAction::ShowShortcuts
        )
    }

    /// Las cuatro acciones de zoom, que no modifican el documento.
    pub fn is_zoom(self) -> bool {
        matches!(
            self,
            AppAction::ZoomIn | AppAction::ZoomOut | AppAction::ZoomFit | AppAction::ZoomActual
        )
    }

    /// Las acciones de vista: el zoom y centrar bajo el cursor. No modifican el documento.
    pub fn is_view(self) -> bool {
        self.is_zoom() || self == AppAction::CenterOnCursor
    }

    /// Las que cambian uno de los dos colores: se bloquean con una quita pendiente y no con un
    /// trazo en curso (specs/interfaz-dos-colores.md, decisión 6).
    fn changes_color(self) -> bool {
        matches!(
            self,
            AppAction::SelectColor(_)
                | AppAction::ColorNext
                | AppAction::ColorPrev
                | AppAction::BackgroundNext
                | AppAction::BackgroundPrev
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
    /// El gesto en curso es solo un trazo de una herramienta (no hay deslizador ni arrastre de
    /// muestra): las acciones de color valen con el botón pulsado y afectan al trazo siguiente.
    pub stroke: bool,
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
/// todo; (4) las acciones de vista (el zoom y centrar bajo el cursor) solo se bloquean con
/// `text_focus` y desde el teclado (no modifican el documento: un gesto o una quita pendiente no
/// las paran); (5) las de color, con `text_focus`, con `removal` o con un `gesture` que no sea solo
/// un trazo (`stroke`); (6) el resto, con `gesture` o `text_focus`, y con `removal` solo las que
/// abren un diálogo; (7) si no, nada. El fondo del lienzo
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
    if action.is_view() {
        return locks.text_focus && origin == Origin::Key;
    }
    if action.changes_color() {
        // Con un trazo de una herramienta en curso el color se puede cambiar: el trazo conserva el
        // que tenía al pulsar. Con el deslizador del editor o un arrastre de muestra, no.
        return locks.text_focus || locks.removal || (locks.gesture && !locks.stroke);
    }
    if locks.gesture || locks.text_focus {
        return true;
    }
    locks.removal && action.opens_dialog()
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
        AppAction::ZoomFit | AppAction::CenterOnCursor => state.zoom.is_some(),
        AppAction::ZoomActual => state.zoom.is_some_and(|zoom| zoom != Zoom::ONE),
        _ => true,
    }
}

/// Traduce una tecla pulsada y sus modificadores (sin texto: la tecla se toma como tecleada) con
/// la tabla por defecto. Es [`crate::keymap::Keymap::action_for`] sobre
/// [`press_from_key`]; los eventos de la ventana pasan por `key_events::presses`.
///
/// `Escape` no es una acción: la ordena el lienzo con los eventos de trazo.
pub fn action_for_shortcut(
    key: Key,
    modifiers: Modifiers,
    platform: Platform,
) -> Option<AppAction> {
    default_keymap().action_for(&press_from_key(key, modifiers), platform)
}
