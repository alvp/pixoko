//! The contextual help and the list of shortcuts (specs/interfaz-ayuda.md, decisions 2, 3 and 12), without
//! egui: the target of `F1`, the forms of the help, of the list and of the confirmation.

use crate::actions::{AppAction, Platform};
use crate::chrome::{
    COLUMN_DEFAULT_COLS, TOOL_LABELS, TOOLBOX_BUTTONS, toolbox_hit_at, window_layout_cols,
};
use crate::dialog::{Dialog, FieldId, FieldKind, FieldSpec, FormSpec, Rows};
use crate::help_texts::{
    HelpEntry, action_of_key, help_entry, help_key, help_table, is_editable, ui,
};
use crate::keymap::Keymap;
use crate::layers_panel::PanelHit;
use crate::shortcut_editor::{fixed_chords, slots_of};
use crate::tool::Tool;

/// Fields of the help window.
pub const HELP_ZONE: FieldId = "texto";
pub const HELP_PRIMARY: FieldId = "principal";
pub const HELP_SECONDARY: FieldId = "secundario";
/// The `Reset all` button of the list of shortcuts.
pub const SHORTCUTS_RESET: FieldId = "restablecer-todo";
/// The zone of the `Reset all` confirmation.
pub const RESET_TEXT: FieldId = "texto";

/// What `F1` asks help about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HelpTarget {
    /// The active tool.
    Active,
    Tool(Tool),
    ZoomIn,
    ZoomOut,
    ZoomFit,
    ZoomActualSize,
    /// A control of the layers panel (`F1` with the pointer over it).
    Layer(LayerHelp),
    /// One of the two rectangles of the color indicator (`F1` with the pointer over it).
    Indicator,
}

/// The control of the layers panel help is asked about: one per layer action.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayerHelp {
    Panel,
    New,
    Duplicate,
    Remove,
    Up,
    Down,
    Rename,
    Properties,
}

/// The help of control `hit` of the panel: the five buttons, the name field (rename), the rows and
/// the other options (properties) and the rest of the panel (the panel itself).
pub fn layer_help_for(hit: PanelHit) -> LayerHelp {
    use crate::layers_panel::PanelButton;
    match hit {
        PanelHit::Button(PanelButton::New) => LayerHelp::New,
        PanelHit::Button(PanelButton::Duplicate) => LayerHelp::Duplicate,
        PanelHit::Button(PanelButton::Remove) => LayerHelp::Remove,
        PanelHit::Button(PanelButton::Up) => LayerHelp::Up,
        PanelHit::Button(PanelButton::Down) => LayerHelp::Down,
        PanelHit::NameField => LayerHelp::Rename,
        PanelHit::Eye(_)
        | PanelHit::Lock(_)
        | PanelHit::Name(_)
        | PanelHit::OpacitySlider
        | PanelHit::OpacityField
        | PanelHit::BlendButton => LayerHelp::Properties,
        PanelHit::Title
        | PanelHit::Close
        | PanelHit::Scrollbar
        | PanelHit::Memory
        | PanelHit::Count
        | PanelHit::Body => LayerHelp::Panel,
    }
}

impl HelpTarget {
    /// The action of this target; `Active` is the one of the `active` tool.
    pub fn action(self, active: Tool) -> AppAction {
        match self {
            HelpTarget::Active => AppAction::SelectTool(active),
            HelpTarget::Tool(tool) => AppAction::SelectTool(tool),
            HelpTarget::ZoomIn => AppAction::ZoomIn,
            HelpTarget::ZoomOut => AppAction::ZoomOut,
            HelpTarget::ZoomFit => AppAction::ZoomFit,
            HelpTarget::ZoomActualSize => AppAction::ZoomActualSize,
            HelpTarget::Indicator => AppAction::ToggleColorEditor,
            HelpTarget::Layer(help) => match help {
                LayerHelp::Panel => AppAction::ToggleLayerPanel,
                LayerHelp::New => AppAction::NewLayer,
                LayerHelp::Duplicate => AppAction::DuplicateLayer,
                LayerHelp::Remove => AppAction::RemoveLayer,
                LayerHelp::Up => AppAction::LayerUp,
                LayerHelp::Down => AppAction::LayerDown,
                LayerHelp::Rename => AppAction::RenameLayer,
                LayerHelp::Properties => AppAction::LayerProperties,
            },
        }
    }
}

/// The target of button `index` of the toolbox (the five tools and the four zoom ones, in the
/// order of `chrome::toolbox_hit`).
pub fn help_target_for_button(index: usize) -> Option<HelpTarget> {
    const TOOLS: [Tool; 5] = [
        Tool::Pencil,
        Tool::Line,
        Tool::Fill,
        Tool::Dropper,
        Tool::Eraser,
    ];
    if index >= TOOLBOX_BUTTONS {
        return None;
    }
    Some(match index {
        0..=4 => HelpTarget::Tool(TOOLS[index]),
        5 => HelpTarget::ZoomIn,
        6 => HelpTarget::ZoomOut,
        7 => HelpTarget::ZoomFit,
        _ => HelpTarget::ZoomActualSize,
    })
}

/// The target of `F1` with the pointer at `point_px` (physical pixels of the window) if it falls inside the
/// layers panel, which is at `panel` (its rectangle in this window): the help of the control below.
/// `None` outside the panel.
pub fn help_target_in_panel(
    point_px: (i64, i64),
    panel: crate::view::ScreenRect,
    n: u32,
) -> Option<HelpTarget> {
    crate::layers_panel::hit(point_px.0 - panel.x, point_px.1 - panel.y, n)
        .map(|hit| HelpTarget::Layer(layer_help_for(hit)))
}

/// The target of `F1` with the pointer at `point_px` (physical window pixels) if it falls on one of
/// the rectangles of the color indicator. `None` elsewhere.
pub fn help_target_in_indicator(
    point_px: (i64, i64),
    window_px: (i64, i64),
    n: u32,
) -> Option<HelpTarget> {
    help_target_in_indicator_cols(point_px, window_px, n, COLUMN_DEFAULT_COLS)
}

/// Like [`help_target_in_indicator`] with the column of `wanted` columns of swatches (the width that
/// was chosen; the window cuts it).
pub fn help_target_in_indicator_cols(
    point_px: (i64, i64),
    window_px: (i64, i64),
    n: u32,
    wanted: i64,
) -> Option<HelpTarget> {
    let panel = window_layout_cols(window_px, n, wanted).panel;
    crate::palette_grid::indicator_hit(point_px.0 - panel.x, point_px.1 - panel.y, n)
        .map(|_| HelpTarget::Indicator)
}

/// The target of `F1` with the pointer at `point_px` if it falls inside the window of options, which is
/// at `window` (its rectangle in this window of the app): the help of the active tool. `None` outside.
pub fn help_target_in_options(
    point_px: (i64, i64),
    window: crate::view::ScreenRect,
) -> Option<HelpTarget> {
    let inside = point_px.0 >= window.x
        && point_px.0 < window.x + window.width
        && point_px.1 >= window.y
        && point_px.1 < window.y + window.height;
    inside.then_some(HelpTarget::Active)
}

/// The target of `F1` with the pointer at `point_px` (physical pixels of the window): the toolbox
/// button below, also dimmed; anywhere else, the active tool.
pub fn help_target_at(point_px: (i64, i64), window_px: (i64, i64), n: u32) -> HelpTarget {
    help_target_at_cols(point_px, window_px, n, COLUMN_DEFAULT_COLS)
}

/// Like [`help_target_at`] with the column of `wanted` columns of swatches.
pub fn help_target_at_cols(
    point_px: (i64, i64),
    window_px: (i64, i64),
    n: u32,
    wanted: i64,
) -> HelpTarget {
    let layout = window_layout_cols(window_px, n, wanted);
    let toolbar = layout.toolbar;
    let cols = crate::chrome::column_cols(window_px.0, n, wanted);
    let (x, y) = (point_px.0 - toolbar.x, point_px.1 - toolbar.y);
    if x < 0 || y < 0 || x >= toolbar.width || y >= toolbar.height {
        return HelpTarget::Active;
    }
    toolbox_hit_at(x, y, n, TOOL_LABELS.len(), cols)
        .and_then(help_target_for_button)
        .unwrap_or(HelpTarget::Active)
}

fn join(chords: impl IntoIterator<Item = String>) -> String {
    chords.into_iter().collect::<Vec<_>>().join(", ")
}

/// The text of the help zone: the description, the clicks, the options and the fixed shortcuts.
fn zone_text(entry: &HelpEntry, action: AppAction, keymap: &Keymap, platform: Platform) -> String {
    let strings = ui();
    let mut blocks = vec![entry.description.to_string()];
    let mut mouse = Vec::new();
    if let Some(left) = entry.mouse_left {
        mouse.push(format!("{}{left}", strings.left_click));
    }
    if let Some(right) = entry.mouse_right {
        mouse.push(format!("{}{right}", strings.right_click));
    }
    if matches!(action, AppAction::SelectTool(_)) {
        mouse.push(strings.tool_button_right_click.to_string());
    }
    if !mouse.is_empty() {
        blocks.push(mouse.join("\n"));
    }
    if !entry.options.is_empty() {
        blocks.push(
            entry
                .options
                .iter()
                .map(|(name, text)| format!("{}{name}: {text}", strings.options_prefix))
                .collect::<Vec<_>>()
                .join("\n"),
        );
    }
    let fixed = fixed_chords(keymap, action, platform);
    let mut shortcuts = Vec::new();
    if is_editable(action) {
        if !fixed.is_empty() {
            shortcuts.push(format!(
                "{}{}",
                strings.fixed_prefix,
                join(fixed.iter().map(|c| c.text(platform)))
            ));
        }
        let extras: Vec<_> = slots_of(keymap, action).slots.into_iter().skip(2).collect();
        if !extras.is_empty() {
            shortcuts.push(format!(
                "{}{}",
                strings.also_prefix,
                join(extras.iter().map(|c| c.text(platform)))
            ));
        }
    } else {
        let text = if fixed.is_empty() {
            strings.no_shortcut.to_string()
        } else {
            join(fixed.iter().map(|c| c.text(platform)))
        };
        shortcuts.push(format!("{}{text}", strings.fixed_single_prefix));
    }
    if !shortcuts.is_empty() {
        blocks.push(shortcuts.join("\n"));
    }
    blocks.join("\n\n")
}

/// The help form of the action with key `key`: the title is the name; a scrolling text zone and,
/// for an editable action, the `Principal` and `Secundario` buttons with their shortcut. `None` if
/// the key does not exist.
pub fn help_form_for(key: &str, keymap: &Keymap, platform: Platform) -> Option<FormSpec> {
    let strings = ui();
    let entry = help_entry(key)?;
    let action = action_of_key(key)?;
    let mut fields = vec![FieldSpec {
        id: HELP_ZONE,
        label: "",
        kind: FieldKind::ReadOnly {
            text: zone_text(entry, action, keymap, platform),
            rows: Rows::Fixed(9),
        },
        shown_when: None,
    }];
    if is_editable(action) {
        let slots = slots_of(keymap, action).slots;
        for (id, label, index) in [
            (HELP_PRIMARY, strings.primary, 0usize),
            (HELP_SECONDARY, strings.secondary, 1usize),
        ] {
            fields.push(FieldSpec {
                id,
                label,
                kind: FieldKind::Button {
                    text: slots
                        .get(index)
                        .map_or(strings.no_shortcut.to_string(), |c| c.text(platform)),
                    aside: None,
                },
                shown_when: None,
            });
        }
    }
    Some(FormSpec {
        title: entry.name,
        accept: strings.close,
        cancel: None,
        fields,
        check: None,
    })
}

/// The help form of a target; `active` is the active tool (for `Active`).
pub fn help_form(
    target: HelpTarget,
    active: Tool,
    keymap: &Keymap,
    platform: Platform,
) -> FormSpec {
    let key = help_key(target.action(active))
        .expect("toda herramienta y todo botón de zoom tienen ayuda");
    help_form_for(key, keymap, platform).expect("la clave existe")
}

/// The line of the list for an action: `{name:<20}{shortcuts}` (44 columns at most), with the
/// `conflicto` mark if the action lost a shortcut in a clash.
fn list_line(
    entry: &HelpEntry,
    action: AppAction,
    keymap: &Keymap,
    platform: Platform,
    conflicted: bool,
) -> String {
    let fixed = fixed_chords(keymap, action, platform);
    let mut chords: Vec<String> = if is_editable(action) {
        slots_of(keymap, action)
            .slots
            .iter()
            .map(|c| c.text(platform))
            .collect()
    } else {
        Vec::new()
    };
    chords.extend(fixed.iter().map(|c| c.text(platform)));
    let chords = if chords.is_empty() {
        ui().no_shortcut.to_string()
    } else {
        chords.join(", ")
    };
    if conflicted {
        let short: String = chords.chars().take(14).collect();
        format!("{:<20}{:<14} {}", entry.name, short, ui().conflict_mark)
    } else {
        let short: String = chords.chars().take(24).collect();
        format!("{:<20}{short}", entry.name)
    }
}

/// The list of shortcuts: one button per action (`{name:<20}{shortcuts}`), `Reset all` at the end and
/// `Cerrar`. `conflicted` are the actions with the `conflicto` mark. The dialog has to go through
/// [`prepare_shortcut_list`] to dim the fixed actions.
pub fn shortcuts_form(keymap: &Keymap, platform: Platform, conflicted: &[AppAction]) -> FormSpec {
    let strings = ui();
    let mut fields: Vec<FieldSpec> = help_table()
        .iter()
        .filter_map(|entry| {
            let action = action_of_key(entry.key)?;
            Some(FieldSpec {
                id: entry.key,
                label: "",
                kind: FieldKind::Button {
                    text: list_line(
                        entry,
                        action,
                        keymap,
                        platform,
                        conflicted.contains(&action),
                    ),
                    aside: None,
                },
                shown_when: None,
            })
        })
        .collect();
    fields.push(FieldSpec {
        id: SHORTCUTS_RESET,
        label: "",
        kind: FieldKind::Button {
            text: strings.reset_all.to_string(),
            aside: None,
        },
        shown_when: None,
    });
    FormSpec {
        title: strings.list_title,
        accept: strings.close,
        cancel: None,
        fields,
        check: None,
    }
}

/// Dims in the list the fixed actions (their combinations are seen but do not open the editor).
pub fn prepare_shortcut_list(dialog: &mut Dialog) {
    for entry in help_table() {
        if action_of_key(entry.key).is_some_and(|action| !is_editable(action)) {
            dialog.set_button_disabled(entry.key, true, false);
        }
    }
}

/// The `Reset all` confirmation.
pub fn confirm_reset_form() -> FormSpec {
    let strings = ui();
    FormSpec {
        title: strings.reset_title,
        accept: strings.accept,
        cancel: Some(strings.cancel),
        fields: vec![FieldSpec {
            id: RESET_TEXT,
            label: "",
            kind: FieldKind::ReadOnly {
                text: strings.reset_all_text.to_string(),
                rows: Rows::Fixed(2),
            },
            shown_when: None,
        }],
        check: None,
    }
}
