//! `dispatch`: la capa de acciones de la app (specs/hito-2-menu-herramientas.md, «Piezas»).
//!
//! El menú, la columna de herramientas, los botones de zoom y los atajos producen la misma
//! [`AppAction`]; aquí se comprueba el bloqueo y se aplica lo que no necesita ventana. Lo que sí la
//! necesita (diálogos, cerrar, el zoom del lienzo, el panel de licencias) se devuelve como
//! [`Dispatched`] y lo ejecuta `App`. Sin egui: se prueba sin ventana.

use px_ui::actions::{AppAction, Locks, Origin, is_blocked};
use px_ui::background::CanvasBackground;
use px_ui::color::key_choice;
use px_ui::shortcuts::FileAction;
use px_ui::tool::{ToolAction, ToolState};
use px_ui::view::ViewOp;

use crate::keys::select_tool;
use crate::session::Session;

/// Lo que dejó una acción despachada.
#[derive(Clone, Debug, PartialEq)]
pub enum Dispatched {
    /// Bloqueada por [`is_blocked`]: no cambió nada.
    Blocked,
    /// Aplicada. `refresh`: el documento cambió y hay que regenerar la textura. `notice`: aviso
    /// para la barra (un error o el aviso del borrador).
    Done {
        refresh: bool,
        notice: Option<String>,
    },
    /// Una acción de fichero: la ejecuta `App::file_action` (diálogo, confirmación…).
    File(FileAction),
    /// Cerrar la ventana: pasa por la regla de cierre por repetición de `handle_close`.
    Quit,
    /// Una operación de zoom para el lienzo.
    View(ViewOp),
    /// Abrir (`true`) o cerrar (`false`) el panel de licencias.
    Licenses(bool),
    /// `Nueva imagen…`: la ejecuta `App` (confirmación, diálogo y creación).
    NewImage,
    /// `Convertir modo…`: la ejecuta `App` (diálogo, informe y conversión).
    ConvertMode,
    /// Elegir el fondo bajo la transparencia del lienzo: preferencia de la sesión, no toca el
    /// documento.
    Background(CanvasBackground),
    /// `Color…` del fondo del lienzo: la ejecuta `App` (abre el diálogo).
    BackgroundColor,
}

/// Lo que `dispatch` puede tocar, con los bloqueos de este fotograma.
pub struct Targets<'a> {
    pub session: &'a mut Session,
    pub tools: &'a mut ToolState,
    pub locks: Locks,
}

fn done(refresh: bool) -> Dispatched {
    Dispatched::Done {
        refresh,
        notice: None,
    }
}

/// Aplica `action`, pedida desde `origin`, salvo que esté bloqueada.
pub fn dispatch(action: AppAction, origin: Origin, targets: &mut Targets) -> Dispatched {
    if is_blocked(action, targets.locks, origin) {
        return Dispatched::Blocked;
    }
    let Targets { session, tools, .. } = targets;
    match action {
        AppAction::NewImage => Dispatched::NewImage,
        AppAction::ConvertMode => Dispatched::ConvertMode,
        AppAction::Open => Dispatched::File(FileAction::Open),
        AppAction::Save => Dispatched::File(FileAction::Save),
        AppAction::SaveAs => Dispatched::File(FileAction::SaveAs),
        AppAction::Quit => Dispatched::Quit,
        AppAction::Undo if !session.document.can_undo() => done(false),
        AppAction::Undo => match session.undo() {
            Ok(()) => done(true),
            Err(error) => Dispatched::Done {
                refresh: false,
                notice: Some(error.to_string()),
            },
        },
        AppAction::Redo if !session.document.can_redo() => done(false),
        AppAction::Redo => match session.redo() {
            Ok(()) => done(true),
            Err(error) => Dispatched::Done {
                refresh: false,
                notice: Some(error.to_string()),
            },
        },
        AppAction::ZoomIn => Dispatched::View(ViewOp::ZoomIn),
        AppAction::ZoomOut => Dispatched::View(ViewOp::ZoomOut),
        AppAction::ZoomFit => Dispatched::View(ViewOp::Fit),
        AppAction::ZoomActual => Dispatched::View(ViewOp::Actual),
        AppAction::ShowLicenses => Dispatched::Licenses(true),
        AppAction::CloseLicenses => Dispatched::Licenses(false),
        AppAction::CanvasBackground(background) => Dispatched::Background(background),
        AppAction::BackgroundColor => Dispatched::BackgroundColor,
        AppAction::SelectTool(tool) => {
            let image = session.document.image(session.document.main_image());
            Dispatched::Done {
                refresh: false,
                notice: select_tool(tools, ToolAction::Select(tool), false, image),
            }
        }
        AppAction::SelectColor(slot) => {
            let image = session.document.image(session.document.main_image());
            if let Some(choice) = image.and_then(|image| key_choice(image, slot)) {
                session.set_choice_revealing(choice);
            }
            done(false)
        }
    }
}

#[cfg(test)]
mod teclado_tests;
#[cfg(test)]
mod tests;
