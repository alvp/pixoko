//! La tabla de ayuda (specs/interfaz-ayuda.md, decisión 1): una entrada por acción de la tabla de
//! teclado, con una **clave** estable e independiente del idioma (es también la del fichero de
//! atajos), y las funciones que relacionan acciones, claves y textos. Los textos viven en
//! `help_texts_es` (solo datos, preparados para traducir en el hito 8); sin egui.

use crate::actions::AppAction;
use std::sync::LazyLock;

use crate::help_texts_es::{HELP_ES, UI_ES};
use crate::help_texts_layers_es::HELP_LAYERS_ES;
use crate::help_texts_merge_es::HELP_MERGE_ES;
use crate::tool::Tool;

/// El texto de ayuda de una acción.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HelpEntry {
    /// Clave estable (`tool.pencil`, `view.zoom-in`…).
    pub key: &'static str,
    /// Nombre (≤ 20 caracteres).
    pub name: &'static str,
    /// Descripción corta (≤ 8 líneas de 42 columnas).
    pub description: &'static str,
    /// Qué hace el clic izquierdo, si algo.
    pub mouse_left: Option<&'static str>,
    /// Qué hace el clic derecho, si algo.
    pub mouse_right: Option<&'static str>,
    /// Opciones: nombre y descripción de cada una. Vacío hoy (ninguna herramienta las tiene).
    pub options: &'static [(&'static str, &'static str)],
}

/// El texto fijo de las ventanas de ayuda y de atajos.
#[derive(Clone, Copy, Debug)]
pub struct UiStrings {
    pub primary: &'static str,
    pub secondary: &'static str,
    pub no_shortcut: &'static str,
    pub close: &'static str,
    pub accept: &'static str,
    pub cancel: &'static str,
    pub remove: &'static str,
    pub reassign: &'static str,
    pub keep: &'static str,
    pub conflict_label: &'static str,
    pub reset_label: &'static str,
    pub reset: &'static str,
    pub reset_all: &'static str,
    pub reset_all_text: &'static str,
    pub editor_title: &'static str,
    pub list_title: &'static str,
    pub reset_title: &'static str,
    pub action_prefix: &'static str,
    pub fixed_prefix: &'static str,
    pub also_prefix: &'static str,
    pub fixed_single_prefix: &'static str,
    pub left_click: &'static str,
    pub right_click: &'static str,
    pub options_prefix: &'static str,
    pub capture_prompt: &'static str,
    pub armed: &'static str,
    pub same_action: &'static str,
    pub resolve_conflict: &'static str,
    pub conflict_mark: &'static str,
    pub already_belongs: &'static str,
    pub not_a_key: &'static str,
    pub no_command: &'static str,
    pub no_alt_letter: &'static str,
    pub no_space: &'static str,
    pub f1_reserved: &'static str,
}

/// La tabla de textos de ayuda (hoy en español: 22 entradas, las 8 de capas y las 3 de combinar, en este orden).
pub fn help_table() -> &'static [HelpEntry] {
    static TABLE: LazyLock<Vec<HelpEntry>> = LazyLock::new(|| {
        HELP_ES
            .iter()
            .chain(HELP_LAYERS_ES.iter())
            .chain(HELP_MERGE_ES.iter())
            .copied()
            .collect()
    });
    TABLE.as_slice()
}

/// El texto fijo de la interfaz (hoy en español).
pub fn ui() -> &'static UiStrings {
    &UI_ES
}

/// La entrada de clave `key`.
pub fn help_entry(key: &str) -> Option<&'static HelpEntry> {
    help_table().iter().find(|entry| entry.key == key)
}

/// La clave de ayuda de `action`: `None` para lo que no tiene ayuda (las ranuras de color, los
/// fondos del lienzo, las acciones de diálogo…).
pub fn help_key(action: AppAction) -> Option<&'static str> {
    Some(match action {
        AppAction::SelectTool(Tool::Pencil) => "tool.pencil",
        AppAction::SelectTool(Tool::Line) => "tool.line",
        AppAction::SelectTool(Tool::Fill) => "tool.fill",
        AppAction::SelectTool(Tool::Dropper) => "tool.dropper",
        AppAction::SelectTool(Tool::Eraser) => "tool.eraser",
        AppAction::ZoomIn => "view.zoom-in",
        AppAction::ZoomOut => "view.zoom-out",
        AppAction::ZoomFit => "view.zoom-fit",
        AppAction::ZoomActualSize => "view.zoom-actual",
        AppAction::CenterOnCursor => "view.center",
        AppAction::Undo => "edit.undo",
        AppAction::Redo => "edit.redo",
        AppAction::ClearPage => "edit.clear-page",
        AppAction::ColorNext => "color.next",
        AppAction::ColorPrev => "color.prev",
        AppAction::BackgroundNext => "color.bg-next",
        AppAction::BackgroundPrev => "color.bg-prev",
        AppAction::NewImage => "file.new",
        AppAction::Open => "file.open",
        AppAction::Save => "file.save",
        AppAction::SaveAs => "file.save-as",
        AppAction::Quit => "file.quit",
        AppAction::ToggleLayerPanel => "layer.panel",
        AppAction::NewLayer => "layer.new",
        AppAction::DuplicateLayer => "layer.duplicate",
        AppAction::RemoveLayer => "layer.remove",
        AppAction::LayerUp => "layer.up",
        AppAction::LayerDown => "layer.down",
        AppAction::RenameLayer => "layer.rename",
        AppAction::LayerProperties => "layer.properties",
        AppAction::MergeLayerDown => "layer.merge-down",
        AppAction::MergeVisibleLayers => "layer.merge-visible",
        AppAction::FlattenLayers => "layer.flatten",
        _ => return None,
    })
}

/// La acción de una clave de ayuda: la inversa de [`help_key`].
pub fn action_of_key(key: &str) -> Option<AppAction> {
    Some(match key {
        "tool.pencil" => AppAction::SelectTool(Tool::Pencil),
        "tool.line" => AppAction::SelectTool(Tool::Line),
        "tool.fill" => AppAction::SelectTool(Tool::Fill),
        "tool.dropper" => AppAction::SelectTool(Tool::Dropper),
        "tool.eraser" => AppAction::SelectTool(Tool::Eraser),
        "view.zoom-in" => AppAction::ZoomIn,
        "view.zoom-out" => AppAction::ZoomOut,
        "view.zoom-fit" => AppAction::ZoomFit,
        "view.zoom-actual" => AppAction::ZoomActualSize,
        "view.center" => AppAction::CenterOnCursor,
        "edit.undo" => AppAction::Undo,
        "edit.redo" => AppAction::Redo,
        "edit.clear-page" => AppAction::ClearPage,
        "color.next" => AppAction::ColorNext,
        "color.prev" => AppAction::ColorPrev,
        "color.bg-next" => AppAction::BackgroundNext,
        "color.bg-prev" => AppAction::BackgroundPrev,
        "file.new" => AppAction::NewImage,
        "file.open" => AppAction::Open,
        "file.save" => AppAction::Save,
        "file.save-as" => AppAction::SaveAs,
        "file.quit" => AppAction::Quit,
        "layer.panel" => AppAction::ToggleLayerPanel,
        "layer.new" => AppAction::NewLayer,
        "layer.duplicate" => AppAction::DuplicateLayer,
        "layer.remove" => AppAction::RemoveLayer,
        "layer.up" => AppAction::LayerUp,
        "layer.down" => AppAction::LayerDown,
        "layer.rename" => AppAction::RenameLayer,
        "layer.properties" => AppAction::LayerProperties,
        "layer.merge-down" => AppAction::MergeLayerDown,
        "layer.merge-visible" => AppAction::MergeVisibleLayers,
        "layer.flatten" => AppAction::FlattenLayers,
        _ => return None,
    })
}

/// Si el usuario puede editar los atajos de `action`: las de la tabla de ayuda, salvo las cinco de
/// archivo (nueva, abrir, guardar, guardar como y salir), que conservan sus combinaciones modernas.
pub fn is_editable(action: AppAction) -> bool {
    help_key(action).is_some()
        && !matches!(
            action,
            AppAction::NewImage
                | AppAction::Open
                | AppAction::Save
                | AppAction::SaveAs
                | AppAction::Quit
        )
}
