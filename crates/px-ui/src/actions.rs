//! Single action layer: the menu, the toolbox column, the zoom buttons and the keyboard shortcuts
//! produce the same [`AppAction`] and obey the same blocking rules
//! (specs/hito-2-menu-herramientas.md, decisions 2 to 5). All of it is pure logic, without a window.

use egui::{Key, Modifiers};

use crate::background::CanvasBackground;
use crate::help::HelpTarget;
use crate::key_events::press_from_key;
use crate::keymap::default_keymap;
use crate::tool::Tool;
use crate::view::Zoom;

/// What can be asked of the app, wherever it comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppAction {
    /// `Archivo → Nueva image_2…` (specs/hito-2-nueva-imagen.md).
    NewImage,
    /// `Image → Convert mode…` (specs/hito-2-convertir-modo.md): no shortcut.
    ConvertMode,
    Open,
    Save,
    SaveAs,
    Quit,
    Undo,
    Redo,
    ZoomIn,
    ZoomOut,
    /// Fit to the window.
    ZoomFit,
    /// 1:1.
    ZoomActualSize,
    ShowLicenses,
    CloseLicenses,
    SelectTool(Tool),
    /// Color slot, from 1 to 9 (no default key: assignable in milestone 8).
    SelectColor(u8),
    /// Next color (`]`): the entry in indexed images, the slot in the other modes.
    ColorNext,
    /// Color anterior (`[`).
    ColorPrev,
    /// Centers the area under the cursor (`n`): it needs the pointer, it has no menu entry.
    CenterOnCursor,
    /// Next background color (`}` and `Shift`+`Page Down`), like `ColorNext` but for the background.
    BackgroundNext,
    /// Previous background color (`{` and `Shift`+`Page Up`).
    BackgroundPrev,
    /// `Edit → Clear page` (`K`): fills the whole image with the background color, in one step.
    ClearPage,
    /// Background under the transparency of the canvas: a preference of the session, not of the document.
    CanvasBackground(CanvasBackground),
    /// `View → Canvas background → Color…` (specs/hito-2-fondo-color.md): opens a dialog.
    BackgroundColor,
    /// `F1` or `Help → Tool help` (specs/interfaz-ayuda.md): opens the help of the
    /// target (the active tool or the toolbox button under the pointer).
    ShowHelp(HelpTarget),
    /// `Help → Keyboard shortcuts…`: opens the list of shortcuts.
    ShowShortcuts,
    /// `Layer → Show panel`: shows or hides the layers panel (specs/hito-3-capas-interfaz.md).
    ToggleLayerPanel,
    /// `View → Color editor` (`p`): opens or closes the color editor (specs/ui-composition.md).
    ToggleColorEditor,
    /// `Layer → New layer`: an empty layer above the active one.
    NewLayer,
    /// `Layer → Duplicate`: the copy goes right above the active one.
    DuplicateLayer,
    /// `Layer → Remove`.
    RemoveLayer,
    /// `Layer → Subir`: un puesto.
    LayerUp,
    /// `Layer → Bajar`: un puesto.
    LayerDown,
    /// `Layer → Rename`: takes the focus to the name in the panel.
    RenameLayer,
    /// `Layer → Properties`: takes the focus to the options of the layer in the panel.
    LayerProperties,
    /// `Layer → Merge down`: joins the active one with the one below (specs/hito-3-combinar.md).
    MergeLayerDown,
    /// `Layer → Merge visible`: joins all the visible ones into the lowest visible one.
    MergeVisibleLayers,
    /// `Layer → Flatten`: a single layer with what is seen; the hidden ones are removed.
    FlattenLayers,
    /// `File → Export flattened PNG…`: saves what is seen without touching the state of the document.
    ExportFlat,
    /// `Editar → Intercambiar colores`: swaps the foreground and the background (specs/ui-color-block.md);
    /// session state, no shortcut by default.
    SwapColors,
    /// `Imagen → Añadir entrada`: copies the chosen color to the end of the palette (no default key).
    AddPaletteEntry,
    /// `Imagen → Quitar entrada`: removes the chosen entry (no default key).
    RemovePaletteEntry,
    /// `Imagen → Tamaño de la paleta…`: opens a dialog with the exact length (no default key).
    PaletteSize,
}

impl AppAction {
    /// The file shortcuts and `Quit` do not repeat with the key held down: they would confirm by themselves.
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

    /// The actions that open a modal dialog (licenses, new image, convert mode, the background
    /// color, the help and the list of shortcuts): they do not open with a gesture in progress nor with a pending removal.
    pub fn opens_dialog(self) -> bool {
        matches!(
            self,
            AppAction::ShowLicenses
                | AppAction::NewImage
                | AppAction::ConvertMode
                | AppAction::BackgroundColor
                | AppAction::ShowHelp(_)
                | AppAction::ShowShortcuts
                | AppAction::PaletteSize
        )
    }

    /// The actions of the `Imagen` menu that edit the palette (C3): they are blocked like any
    /// change of the document and also with a pending removal or resize.
    pub fn changes_palette(self) -> bool {
        matches!(
            self,
            AppAction::AddPaletteEntry | AppAction::RemovePaletteEntry | AppAction::PaletteSize
        )
    }

    /// The four zoom actions, which do not modify the document.
    pub fn is_zoom(self) -> bool {
        matches!(
            self,
            AppAction::ZoomIn | AppAction::ZoomOut | AppAction::ZoomFit | AppAction::ZoomActualSize
        )
    }

    /// The view actions: zoom and centering under the cursor. They do not modify the document.
    pub fn is_view(self) -> bool {
        self.is_zoom() || self == AppAction::CenterOnCursor
    }

    /// Those that change one of the two colors: they are blocked with a pending removal and not with a
    /// stroke in progress (specs/interfaz-dos-colores.md, decision 6).
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

/// Where an action comes from. It does not change the effect; it only counts for the blocking of the zoom with a
/// focused text field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    Menu,
    Button,
    Key,
}

/// Operating system, for the shortcuts and their text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Platform {
    Linux,
    Windows,
    Mac,
}

impl Platform {
    /// The system it was compiled for.
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

/// Why an action may be blocked in this frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Locks {
    /// A stroke of a tool, the slider of the editor or the drag of a swatch.
    pub gesture: bool,
    /// The gesture in progress is only a stroke of a tool (there is no slider or swatch drag):
    /// the color actions work with the button pressed and affect the next stroke.
    pub stroke: bool,
    /// An open file dialog.
    pub dialog: bool,
    /// A text field with the focus.
    pub text_focus: bool,
    /// A palette removal pending confirmation.
    pub removal: bool,
    /// An open modal dialog.
    pub modal: bool,
}

/// `true` if `action`, asked from `origin`, must not run with these locks.
///
/// In this order: (1) `CloseLicenses` never; (2) with `modal`, everything but `Quit`; (3) with
/// `dialog`, everything; (4) the view actions (zoom, centering under the cursor, the layers panel
/// and the color editor) are only blocked with `text_focus` and from the keyboard (they do not
/// modify the document: a gesture or a pending removal does not stop them); (5) the color ones,
/// with `text_focus`, with `removal` or with a `gesture` that is not just a stroke (`stroke`); (6)
/// the rest, with `gesture` or `text_focus`, and with `removal` only those that open a dialog; (7)
/// otherwise, nothing. The canvas background (`CanvasBackground`) does not touch the document
/// either: after (1) to (3) only `text_focus` blocks it, with any origin; a gesture or a pending
/// removal does not.
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
    if action.is_view()
        || action == AppAction::ToggleLayerPanel
        || action == AppAction::ToggleColorEditor
    {
        return locks.text_focus && origin == Origin::Key;
    }
    if action.changes_color() {
        // With a stroke of a tool in progress the color can be changed: the stroke keeps the one it had
        // when pressed. With the slider of the editor or a swatch drag, it cannot.
        return locks.text_focus || locks.removal || (locks.gesture && !locks.stroke);
    }
    if locks.gesture || locks.text_focus {
        return true;
    }
    locks.removal && (action.opens_dialog() || action.changes_palette())
}

/// Whether a modal dialog can be opened: neither a gesture in progress, nor a pending removal, nor a file dialog,
/// nor a focused text field, nor another modal.
pub fn can_open_dialog(locks: Locks) -> bool {
    !(locks.gesture || locks.dialog || locks.text_focus || locks.removal || locks.modal)
}

/// The layer stack, what the menu and the panel need to enable their actions (it is not part of
/// [`UiState`]: the app passes it to `show_menu_full`). The default leaves everything enabled.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LayerNav {
    /// Number of layers.
    pub count: usize,
    /// Position of the active one from the bottom (0 is the bottom).
    pub active_pos: usize,
    /// One more layer fits (`count < 256`).
    pub can_add: bool,
}

impl Default for LayerNav {
    fn default() -> LayerNav {
        LayerNav {
            count: 3,
            active_pos: 1,
            can_add: true,
        }
    }
}

/// Whether the layer action is enabled with this stack; what is not a layer action always is.
pub fn layer_enabled(action: AppAction, nav: &LayerNav) -> bool {
    match action {
        AppAction::RemoveLayer => nav.count > 1,
        AppAction::LayerUp => nav.active_pos + 1 < nav.count,
        AppAction::LayerDown => nav.active_pos > 0,
        AppAction::NewLayer | AppAction::DuplicateLayer => nav.can_add,
        AppAction::MergeLayerDown => nav.active_pos > 0,
        AppAction::MergeVisibleLayers | AppAction::FlattenLayers => nav.count > 1,
        _ => true,
    }
}

/// What the menu needs to know about the image to enable the palette actions (it is not part of
/// [`UiState`]: the app passes it to `show_menu_full`). The default leaves everything enabled.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PaletteNav {
    /// The image is indexed: the only mode with a palette to edit.
    pub indexed: bool,
}

impl Default for PaletteNav {
    fn default() -> PaletteNav {
        PaletteNav { indexed: true }
    }
}

/// Whether the palette action is enabled for this image; what is not a palette action always is.
pub fn palette_enabled(action: AppAction, nav: &PaletteNav) -> bool {
    !action.changes_palette() || nav.indexed
}

/// What the interface needs to know to enable or disable the menu and the column.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UiState {
    pub can_undo: bool,
    pub can_redo: bool,
    /// Zoom of the canvas, or `None` if there is no image to show.
    pub zoom: Option<Zoom>,
    pub tool: Tool,
    pub locks: Locks,
}

/// Whether the menu entry or the button (which act with `Origin::Menu`) is enabled.
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
        AppAction::ZoomActualSize => state.zoom.is_some_and(|zoom| zoom != Zoom::ONE),
        _ => true,
    }
}

/// Translates a pressed key and its modifiers (without text: the key is taken as typed) with
/// the default table. It is [`crate::keymap::Keymap::action_for`] over
/// [`press_from_key`]; the events of the window go through `key_events::presses`.
///
/// `Escape` is not an action: the canvas orders it with the stroke events.
pub fn action_for_shortcut(
    key: Key,
    modifiers: Modifiers,
    platform: Platform,
) -> Option<AppAction> {
    default_keymap().action_for(&press_from_key(key, modifiers), platform)
}
