//! La ayuda contextual y la lista de atajos (specs/interfaz-ayuda.md, decisiones 2, 3 y 12), sin
//! egui: el destino de `F1`, los formularios de la ayuda, de la lista y de la confirmación.

use crate::actions::{AppAction, Platform};
use crate::chrome::{TOOLBOX_BUTTONS, toolbox_hit, window_layout};
use crate::dialog::{Dialog, FieldId, FieldKind, FieldSpec, FormSpec, Rows};
use crate::help_texts::{
    HelpEntry, action_of_key, help_entry, help_key, help_table, is_editable, ui,
};
use crate::keymap::Keymap;
use crate::layers_panel::PanelHit;
use crate::shortcut_editor::{fixed_chords, slots_of};
use crate::tool::Tool;

/// Campos de la ventana de ayuda.
pub const HELP_ZONE: FieldId = "texto";
pub const HELP_PRIMARY: FieldId = "principal";
pub const HELP_SECONDARY: FieldId = "secundario";
/// El botón `Reset all` de la lista de atajos.
pub const SHORTCUTS_RESET: FieldId = "restablecer-todo";
/// La zona de la confirmación de `Reset all`.
pub const RESET_TEXT: FieldId = "texto";

/// De qué pide ayuda `F1`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HelpTarget {
    /// La herramienta activa.
    Active,
    Tool(Tool),
    ZoomIn,
    ZoomOut,
    ZoomFit,
    ZoomActual,
    /// Un control del panel de capas (`F1` con el puntero encima).
    Layer(LayerHelp),
}

/// El control del panel de capas del que se pide ayuda: una por acción de capa.
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

/// La ayuda del control `hit` del panel: los cinco botones, el campo del nombre (renombrar), las
/// filas y las demás opciones (propiedades) y el resto del panel (el propio panel).
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
    /// La acción de este destino; `Active` es la de la herramienta `active`.
    pub fn action(self, active: Tool) -> AppAction {
        match self {
            HelpTarget::Active => AppAction::SelectTool(active),
            HelpTarget::Tool(tool) => AppAction::SelectTool(tool),
            HelpTarget::ZoomIn => AppAction::ZoomIn,
            HelpTarget::ZoomOut => AppAction::ZoomOut,
            HelpTarget::ZoomFit => AppAction::ZoomFit,
            HelpTarget::ZoomActual => AppAction::ZoomActual,
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

/// El destino del botón `index` de la caja (las cinco herramientas y los cuatro de zoom, en el
/// orden de `chrome::toolbox_hit`).
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
        _ => HelpTarget::ZoomActual,
    })
}

/// El destino de `F1` con el puntero en `point_px` (píxeles físicos de la ventana) si cae dentro del
/// panel de capas, que está en `panel` (su rectángulo en esta ventana): la ayuda del control de
/// debajo. `None` fuera del panel.
pub fn help_target_in_panel(
    point_px: (i64, i64),
    panel: crate::view::ScreenRect,
    n: u32,
) -> Option<HelpTarget> {
    crate::layers_panel::hit(point_px.0 - panel.x, point_px.1 - panel.y, n)
        .map(|hit| HelpTarget::Layer(layer_help_for(hit)))
}

/// El destino de `F1` con el puntero en `point_px` (píxeles físicos de la ventana): el botón de la
/// caja que haya debajo, también atenuado; en cualquier otro sitio, la herramienta activa.
pub fn help_target_at(point_px: (i64, i64), window_px: (i64, i64), n: u32) -> HelpTarget {
    let toolbar = window_layout(window_px, n).toolbar;
    let (x, y) = (point_px.0 - toolbar.x, point_px.1 - toolbar.y);
    if x < 0 || y < 0 || x >= toolbar.width || y >= toolbar.height {
        return HelpTarget::Active;
    }
    toolbox_hit(x, y, n)
        .and_then(help_target_for_button)
        .unwrap_or(HelpTarget::Active)
}

fn join(chords: impl IntoIterator<Item = String>) -> String {
    chords.into_iter().collect::<Vec<_>>().join(", ")
}

/// El texto de la zona de ayuda: la descripción, los clics, las opciones y los atajos fijos.
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

/// El formulario de ayuda de la acción de clave `key`: el título es el nombre; una zona de texto
/// con desplazamiento y, para una acción editable, los botones `Principal` y `Secundario` con su
/// atajo. `None` si la clave no existe.
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

/// El formulario de ayuda de un destino; `active` es la herramienta activa (para `Active`).
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

/// La línea de la lista de una acción: `{name_2:<20}{shortcuts}` (44 columnas como mucho), con la
/// marca `conflicto` si la acción perdió un atajo en un choque.
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

/// La lista de atajos: un botón por acción (`{name_2:<20}{shortcuts}`), `Reset all` al final y
/// `Cerrar`. `conflicted` son las acciones con la marca `conflicto`. Hay que pasar el diálogo por
/// [`prepare_shortcut_list`] para atenuar las acciones fijas.
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

/// Atenúa en la lista las acciones fijas (sus combinaciones se ven pero no abren el editor).
pub fn prepare_shortcut_list(dialog: &mut Dialog) {
    for entry in help_table() {
        if action_of_key(entry.key).is_some_and(|action| !is_editable(action)) {
            dialog.set_button_disabled(entry.key, true, false);
        }
    }
}

/// La confirmación de `Reset all`.
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
