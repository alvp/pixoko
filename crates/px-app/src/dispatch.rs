//! `dispatch`: la capa de acciones de la app (specs/hito-2-menu-herramientas.md, «Piezas»).
//!
//! El menú, la columna de herramientas, los botones de zoom y los atajos producen la misma
//! [`AppAction`]; aquí se comprueba el bloqueo y se aplica lo que no necesita ventana. Lo que sí la
//! necesita (diálogos, cerrar, el zoom del lienzo, el panel de licencias) se devuelve como
//! [`Dispatched`] y lo ejecuta `App`. Sin egui: se prueba sin ventana.

use px_ui::actions::{AppAction, Locks, Origin, is_blocked};
use px_ui::background::CanvasBackground;
use px_ui::color::key_choice;
use px_ui::help::HelpTarget;
use px_ui::shortcuts::FileAction;
use px_ui::tool::{ToolAction, ToolState};
use px_ui::view::ViewOp;

use crate::keys::select_tool;
use crate::merge_flow::MergeKind;
use crate::session::{LayerReport, Session};

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
    /// `F1` o `Ayuda → Ayuda de la herramienta`: `App` abre la ayuda de ese destino.
    Help(HelpTarget),
    /// `Ayuda → Atajos de teclado…`: `App` abre la lista de atajos.
    Shortcuts,
    /// Una acción del panel de capas que es de la ventana: lo muestra, lo oculta o lleva el foco.
    LayerPanel(LayerPanelOp),
    /// `Nueva capa` o `Duplicar` en gris: el aviso ofrece pasar a gris con alfa; repetir la orden en
    /// 5 s abre `Convertir modo…` (la regla de la repetición la lleva `App`).
    OfferGrayAlpha { notice: String, action: AppAction },
    /// Combinar abajo, combinar visibles o aplanar: la ejecuta `App` (reglas de rechazo, espera de
    /// «Combinando…» / «Aplanando…» y confirmación de Aplanar).
    Merge(MergeKind),
    /// `Archivo → Exportar PNG aplanado…`: la ejecuta `App` (diálogo y escritura).
    ExportFlat,
}

/// Lo que se le pide al panel de capas.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayerPanelOp {
    Toggle,
    /// `Renombrar`: abre el panel y lleva el foco al nombre.
    FocusName,
    /// `Propiedades`: abre el panel y lo deja a la vista.
    ShowProperties,
}

/// Lo que `dispatch` puede tocar, con los bloqueos de este fotograma.
pub struct Targets<'a> {
    pub session: &'a mut Session,
    pub tools: &'a mut ToolState,
    pub locks: Locks,
}

/// El resultado de una operación de capa: refresco, aviso y, en gris, la oferta de convertir.
fn layer_done(action: AppAction, report: LayerReport) -> Dispatched {
    match (report.offer_convert, report.notice) {
        (true, Some(notice)) => Dispatched::OfferGrayAlpha { notice, action },
        (_, notice) => Dispatched::Done {
            refresh: report.changed,
            notice,
        },
    }
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
        AppAction::ShowHelp(target) => Dispatched::Help(target),
        AppAction::ShowShortcuts => Dispatched::Shortcuts,
        AppAction::ToggleLayerPanel => Dispatched::LayerPanel(LayerPanelOp::Toggle),
        AppAction::RenameLayer => Dispatched::LayerPanel(LayerPanelOp::FocusName),
        AppAction::LayerProperties => Dispatched::LayerPanel(LayerPanelOp::ShowProperties),
        AppAction::NewLayer => layer_done(action, session.new_layer()),
        AppAction::DuplicateLayer => layer_done(action, session.duplicate_layer()),
        AppAction::RemoveLayer => layer_done(action, session.remove_layer()),
        AppAction::LayerUp => layer_done(action, session.move_layer(1)),
        AppAction::LayerDown => layer_done(action, session.move_layer(-1)),
        AppAction::MergeLayerDown => Dispatched::Merge(MergeKind::Down),
        AppAction::MergeVisibleLayers => Dispatched::Merge(MergeKind::Visible),
        AppAction::FlattenLayers => Dispatched::Merge(MergeKind::Flatten),
        AppAction::ExportFlat => Dispatched::ExportFlat,
        AppAction::SelectTool(tool) => {
            let image = Some(session.document.canvas_image());
            Dispatched::Done {
                refresh: false,
                notice: select_tool(tools, ToolAction::Select(tool), false, image),
            }
        }
        AppAction::ColorNext => {
            session.cycle_choice(1);
            done(false)
        }
        AppAction::ColorPrev => {
            session.cycle_choice(-1);
            done(false)
        }
        AppAction::BackgroundNext => {
            session.cycle_background(1);
            done(false)
        }
        AppAction::BackgroundPrev => {
            session.cycle_background(-1);
            done(false)
        }
        AppAction::ClearPage => match session.clear_page() {
            Ok(changed) => done(changed),
            Err(error) => Dispatched::Done {
                refresh: false,
                notice: Some(error.to_string()),
            },
        },
        AppAction::CenterOnCursor => Dispatched::View(ViewOp::CenterOnCursor),
        AppAction::SelectColor(slot) => {
            let image = session.document.canvas_image();
            if let Some(choice) = key_choice(image, slot) {
                session.set_choice_revealing(choice);
            }
            done(false)
        }
    }
}

#[cfg(test)]
mod ayuda_tests;
#[cfg(test)]
mod teclado_tests;
#[cfg(test)]
mod tests;
