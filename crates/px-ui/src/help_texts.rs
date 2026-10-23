//! The help table (specs/interfaz-ayuda.md, decision 1): one entry per action of the keyboard table,
//! with a stable **key** independent of the language (it is also the one of the shortcuts file), and
//! the functions that relate actions, keys and texts. The texts live in `help_texts_es` (data only,
//! ready to translate in milestone 8); without egui.

use crate::actions::AppAction;
use std::sync::LazyLock;

use crate::help_texts_color_es::HELP_COLOR_ES;
use crate::help_texts_es::{HELP_ES, UI_ES};
use crate::help_texts_layers_es::HELP_LAYERS_ES;
use crate::help_texts_merge_es::HELP_MERGE_ES;
use crate::help_texts_palette_es::HELP_PALETTE_ES;
use crate::tool::Tool;

/// The help text of an action.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HelpEntry {
    /// Clave estable (`tool.pencil`, `view.zoom-in`…).
    pub key: &'static str,
    /// Nombre (≤ 20 caracteres).
    pub name: &'static str,
    /// Short description (≤ 8 lines of 42 columns).
    pub description: &'static str,
    /// What the left click does, if anything.
    pub mouse_left: Option<&'static str>,
    /// What the right click does, if anything.
    pub mouse_right: Option<&'static str>,
    /// Options: name and description of each. Empty today (no tool has them).
    pub options: &'static [(&'static str, &'static str)],
}

/// The fixed text of the help and shortcuts windows.
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
    /// The line that the help of a tool adds: what the right click on its button does.
    pub tool_button_right_click: &'static str,
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

/// The table of help texts (today in Spanish: 22 entries, the 8 of layers, the 3 of merge, the one
/// of the color editor and the 5 of the palette panel, in this order).
pub fn help_table() -> &'static [HelpEntry] {
    static TABLE: LazyLock<Vec<HelpEntry>> = LazyLock::new(|| {
        HELP_ES
            .iter()
            .chain(HELP_LAYERS_ES.iter())
            .chain(HELP_MERGE_ES.iter())
            .chain(HELP_COLOR_ES.iter())
            .chain(HELP_PALETTE_ES.iter())
            .copied()
            .collect()
    });
    TABLE.as_slice()
}

/// The fixed text of the interface (today in Spanish).
pub fn ui() -> &'static UiStrings {
    &UI_ES
}

/// The entry with key `key`.
pub fn help_entry(key: &str) -> Option<&'static HelpEntry> {
    help_table().iter().find(|entry| entry.key == key)
}

/// The help key of `action`: `None` for what has no help (the color slots, the canvas backgrounds,
/// the dialog actions…).
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
        AppAction::ToggleColorEditor => "color.editor",
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
        AppAction::AddPaletteEntry => "palette.add_entry",
        AppAction::RemovePaletteEntry => "palette.remove_entry",
        AppAction::PaletteSize => "palette.size",
        _ => return None,
    })
}

/// The action of a help key: the inverse of [`help_key`].
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
        "color.editor" => AppAction::ToggleColorEditor,
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
        "palette.add_entry" => AppAction::AddPaletteEntry,
        "palette.remove_entry" => AppAction::RemovePaletteEntry,
        "palette.size" => AppAction::PaletteSize,
        _ => return None,
    })
}

/// Whether the user can edit the shortcuts of `action`: those of the help table, except the five file
/// ones (new, open, save, save as and quit), which keep their modern combinations.
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
