//! `dispatch`: the action layer of the app (specs/hito-2-menu-herramientas.md, "Pieces").
//!
//! The menu, the toolbox column, the zoom buttons and the shortcuts produce the same
//! [`AppAction`]; here the blocking is checked and what does not need a window is applied. What does
//! need one (dialogs, closing, the zoom of the canvas, the licenses panel) is returned as
//! [`Dispatched`] and `App` runs it. Without egui: it is tested without a window.

use px_ui::actions::{AppAction, Locks, Origin, is_blocked};
use px_ui::background::CanvasBackground;
use px_ui::color::key_choice;
use px_ui::help::HelpTarget;
use px_ui::shortcuts::FileAction;
use px_ui::tool::{ToolAction, ToolState};
use px_ui::view::ViewOp;

use crate::keys::select_tool;
use crate::merge_flow::MergeKind;
use crate::session::{LayerReport, PaletteOutcome, Session};
use px_core::DocumentError;

/// What a dispatched action left.
#[derive(Clone, Debug, PartialEq)]
pub enum Dispatched {
    /// Blocked by [`is_blocked`]: nothing changed.
    Blocked,
    /// Applied. `refresh`: the document changed and the texture has to be regenerated. `notice`: notice
    /// for the bar (an error or the eraser's notice).
    Done {
        refresh: bool,
        notice: Option<String>,
    },
    /// A file action: `App::file_action` runs it (dialog, confirmation…).
    File(FileAction),
    /// Close the window: it goes through the close-by-repetition rule of `handle_close`.
    Quit,
    /// A zoom operation for the canvas.
    View(ViewOp),
    /// Open (`true`) or close (`false`) the licenses panel.
    Licenses(bool),
    /// `New image…`: `App` runs it (confirmation, dialog and creation).
    NewImage,
    /// `Convert mode…`: `App` runs it (dialog, report and conversion).
    ConvertMode,
    /// Choose the background under the transparency of the canvas: a preference of the session, it does not
    /// touch the document.
    Background(CanvasBackground),
    /// `Color…` of the canvas background: `App` runs it (opens the dialog).
    BackgroundColor,
    /// `Image → Palette size…`: `App` opens the dialog with the current length.
    PaletteSize,
    /// `F1` or `Help → Tool help`: `App` opens the help of that target.
    Help(HelpTarget),
    /// `Help → Keyboard shortcuts…`: `App` opens the list of shortcuts.
    Shortcuts,
    /// An action of the layers panel that belongs to the window: it shows it, hides it or takes the focus.
    LayerPanel(LayerPanelOp),
    /// `New layer` or `Duplicate` in gray: the notice offers going to gray with alpha; repeating the order in
    /// 5 s opens `Convert mode…` (the repetition rule is carried by `App`).
    OfferGrayAlpha { notice: String, action: AppAction },
    /// Merge down, merge visible or flatten: `App` runs it (rejection rules, wait of
    /// "Combinando…" / "Aplanando…" and confirmation of Flatten).
    Merge(MergeKind),
    /// `File → Export flattened PNG…`: `App` runs it (dialog and writing).
    ExportFlat,
}

/// What is asked of the layers panel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayerPanelOp {
    Toggle,
    /// `Rename`: opens the panel and takes the focus to the name.
    FocusName,
    /// `Properties`: opens the panel and leaves it in view.
    ShowProperties,
}

/// What `dispatch` may touch, with the locks of this frame.
pub struct Targets<'a> {
    pub session: &'a mut Session,
    pub tools: &'a mut ToolState,
    pub locks: Locks,
}

/// The result of a layer operation: refresh, notice and, in gray, the offer to convert.
fn layer_done(action: AppAction, report: LayerReport) -> Dispatched {
    match (report.offer_convert, report.notice) {
        (true, Some(notice)) => Dispatched::OfferGrayAlpha { notice, action },
        (_, notice) => Dispatched::Done {
            refresh: report.changed,
            notice,
        },
    }
}

/// The result of a palette action of the `Imagen` menu: the refresh and the notice, or the error.
fn palette_done(result: Result<PaletteOutcome, DocumentError>) -> Dispatched {
    match result {
        Ok(outcome) => Dispatched::Done {
            refresh: outcome.changed,
            notice: outcome.notice,
        },
        Err(error) => Dispatched::Done {
            refresh: false,
            notice: Some(error.to_string()),
        },
    }
}

fn done(refresh: bool) -> Dispatched {
    Dispatched::Done {
        refresh,
        notice: None,
    }
}

/// Applies `action`, asked from `origin`, unless it is blocked.
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
        AppAction::ZoomActualSize => Dispatched::View(ViewOp::ActualSize),
        AppAction::ShowLicenses => Dispatched::Licenses(true),
        AppAction::CloseLicenses => Dispatched::Licenses(false),
        AppAction::CanvasBackground(background) => Dispatched::Background(background),
        AppAction::BackgroundColor => Dispatched::BackgroundColor,
        AppAction::ShowHelp(target) => Dispatched::Help(target),
        AppAction::ShowShortcuts => Dispatched::Shortcuts,
        AppAction::ToggleLayerPanel => Dispatched::LayerPanel(LayerPanelOp::Toggle),
        AppAction::ToggleColorEditor => {
            session.toggle_editor();
            Dispatched::Done {
                refresh: false,
                notice: None,
            }
        }
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
        AppAction::AddPaletteEntry => palette_done(session.menu_add_entry()),
        AppAction::RemovePaletteEntry => palette_done(session.menu_remove_entry()),
        AppAction::PaletteSize if session.document.palette().is_none() => done(false),
        AppAction::PaletteSize => Dispatched::PaletteSize,
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
mod help_tests;
#[cfg(test)]
mod keyboard_tests;
#[cfg(test)]
mod tests;
