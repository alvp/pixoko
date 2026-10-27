//! Acciones, tabla de teclado, ayuda y menú de las capas (specs/hito-3-capas-interfaz.md, unidad
//! 3.4: filas 3 y 4 y condiciones 3 y 4). En ROJO hasta la lógica: las ocho `AppAction` de capa,
//! `LayerNav`, `layer_enabled`, `HelpTarget::Layer`, `LayerHelp` y `layers_panel::PanelHit` no
//! existen todavía.
//!
//! - Fila 3: las ocho acciones, las cuatro filas de la tabla (`'` `;` `)` `(`), la ayuda de cada
//!   una (con cómo teclearla en un teclado español), el texto del candado y `F1` sobre el panel.
//! - Fila 4: `layer_enabled` con 1, 3 y 256 capas y la activa arriba y abajo.

use egui::{Event, Key, Modifiers};
use px_ui::actions::{AppAction, LayerNav, Locks, Origin, Platform, is_blocked, layer_enabled};
use px_ui::dialog_layout::ZONE_COLS;
use px_ui::help::{HELP_ZONE, HelpTarget, LayerHelp, help_form, help_form_for, layer_help_for};
use px_ui::help_texts::{action_of_key, help_entry, help_key, help_table, is_editable};
use px_ui::key_events::presses;
use px_ui::keymap::{KeyId, Keymap, Press};
use px_ui::layers_panel::{PanelButton, PanelHit};
use px_ui::licenses_view::wrap_text;
use px_ui::shortcuts::gate_keys;
use px_ui::text::INTERFACE_CHARSET;
use px_ui::tool::Tool;

const PLATFORMS: [Platform; 3] = [Platform::Linux, Platform::Windows, Platform::Mac];

const ACTIONS: [(AppAction, &str); 8] = [
    (AppAction::ToggleLayerPanel, "layer.panel"),
    (AppAction::NewLayer, "layer.new"),
    (AppAction::DuplicateLayer, "layer.duplicate"),
    (AppAction::RemoveLayer, "layer.remove"),
    (AppAction::LayerUp, "layer.up"),
    (AppAction::LayerDown, "layer.down"),
    (AppAction::RenameLayer, "layer.rename"),
    (AppAction::LayerProperties, "layer.properties"),
];

fn presses_2(c: char) -> Press {
    Press {
        key: Some(KeyId::Char(c)),
        command: false,
        shift: false,
        alt: false,
        other_ctrl: false,
        repeat: false,
    }
}

fn text_of_the_area(key: &str) -> String {
    let spec = help_form_for(key, &Keymap::default_scheme(), Platform::Linux).expect(key);
    let field_2 = spec
        .fields
        .iter()
        .find(|f| f.id == HELP_ZONE)
        .expect("zona de texto");
    match &field_2.kind {
        px_ui::dialog::FieldKind::ReadOnly { text, .. } => text.clone(),
        other_2 => panic!("{other_2:?}"),
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 3
// ---------------------------------------------------------------------------------------------

#[test]
fn row_3_the_four_keys_give_their_actions_and_the_table_has_no_conflicts() {
    let table = Keymap::default_scheme();
    assert_eq!(
        table.conflicts(),
        vec![],
        "la propiedad de conflictos de I.1 e I.5"
    );
    assert_eq!(table.rows().len(), 38);
    assert_eq!(table.rows().iter().filter(|f| f.command).count(), 9);
    for platform in PLATFORMS {
        for (c, action_2) in [
            ('\'', AppAction::ToggleLayerPanel),
            (';', AppAction::NewLayer),
            (')', AppAction::LayerUp),
            ('(', AppAction::LayerDown),
        ] {
            assert_eq!(
                table.action_for(&presses_2(c), platform),
                Some(action_2),
                "{c} en {platform:?}"
            );
            // Un símbolo no cuenta `Mayús` ni `Alt` (como `<` y `[`), pero sí `Ctrl`/`Cmd`.
            let mut with_shift = presses_2(c);
            with_shift.shift = true;
            assert_eq!(table.action_for(&with_shift, platform), Some(action_2));
            let mut with_command = presses_2(c);
            with_command.command = true;
            assert_eq!(table.action_for(&with_command, platform), None);
        }
        // Las demás cuatro no tienen tecla por defecto y `l` / `L` siguen sin asignar (I.1).
        for action_2 in [
            AppAction::DuplicateLayer,
            AppAction::RemoveLayer,
            AppAction::RenameLayer,
            AppAction::LayerProperties,
        ] {
            assert!(table.shown(action_2, platform).is_none(), "{action_2:?}");
        }
        for c in ['l', 'L', 'i', 'e'] {
            assert_eq!(table.action_for(&presses_2(c), platform), None, "{c}");
        }
    }
}

#[test]
fn row_3_the_eight_actions_have_a_key_valid_text_and_are_editable() {
    assert_eq!(help_table().len(), 51);
    for (action_2, key) in ACTIONS {
        assert_eq!(help_key(action_2), Some(key));
        assert_eq!(action_of_key(key), Some(action_2));
        assert!(is_editable(action_2), "{key}");
        let input_2 = help_entry(key).expect(key);
        assert!(input_2.name.chars().count() <= 20, "{key}: nombre largo");
        assert!(
            wrap_text(input_2.description, ZONE_COLS).len() <= 8,
            "{key}"
        );
        assert!(input_2.options.is_empty(), "{key}");
        for text_2 in [input_2.name, input_2.description] {
            for c in text_2.chars() {
                assert!(
                    INTERFACE_CHARSET.contains(c),
                    "{key}: {c:?} fuera del juego"
                );
            }
        }
    }
}

#[test]
fn row_3_the_help_of_the_four_keys_says_how_to_type_them_on_a_spanish_keyboard() {
    // Condición 4: `'` es una tecla normal; `;` con Mayús+coma; `(` y `)` con Mayús+8 y Mayús+9.
    assert!(text_of_the_area("layer.panel").contains('\''));
    assert!(text_of_the_area("layer.new").contains("Mayús+coma"));
    assert!(text_of_the_area("layer.up").contains("Mayús+9"));
    assert!(text_of_the_area("layer.down").contains("Mayús+8"));
}

#[test]
fn row_3_the_lock_help_says_it_protects_pixels_and_that_palette_and_mode_affect_all_of_them() {
    // Condición 3: el candado protege los píxeles de las herramientas de dibujo; los cambios de
    // paleta y de modo afectan a todas las capas.
    let text_2 = text_of_the_area("layer.properties");
    for searched in ["candado", "píxeles", "paleta", "modo", "todas las capas"] {
        assert!(
            text_2.contains(searched),
            "falta {searched:?} en {text_2:?}"
        );
    }
}

#[test]
fn row_3_f1_over_each_control_of_the_panel_gives_the_help_of_its_action() {
    let expected_2 = [
        (PanelHit::Button(PanelButton::New), LayerHelp::New),
        (
            PanelHit::Button(PanelButton::Duplicate),
            LayerHelp::Duplicate,
        ),
        (PanelHit::Button(PanelButton::Remove), LayerHelp::Remove),
        (PanelHit::Button(PanelButton::Up), LayerHelp::Up),
        (PanelHit::Button(PanelButton::Down), LayerHelp::Down),
        (PanelHit::Title, LayerHelp::Panel),
        (PanelHit::Close, LayerHelp::Panel),
        (PanelHit::Scrollbar, LayerHelp::Panel),
        (PanelHit::Memory, LayerHelp::Panel),
        (PanelHit::Count, LayerHelp::Panel),
        (PanelHit::Body, LayerHelp::Panel),
        (PanelHit::NameField, LayerHelp::Rename),
        (PanelHit::Eye(0), LayerHelp::Properties),
        (PanelHit::Lock(2), LayerHelp::Properties),
        (PanelHit::Name(1), LayerHelp::Properties),
        (PanelHit::OpacitySlider, LayerHelp::Properties),
        (PanelHit::OpacityField, LayerHelp::Properties),
        (PanelHit::BlendButton, LayerHelp::Properties),
    ];
    for (hit_2, help) in expected_2 {
        assert_eq!(layer_help_for(hit_2), help, "{hit_2:?}");
    }
    let actions = [
        (LayerHelp::Panel, AppAction::ToggleLayerPanel),
        (LayerHelp::New, AppAction::NewLayer),
        (LayerHelp::Duplicate, AppAction::DuplicateLayer),
        (LayerHelp::Remove, AppAction::RemoveLayer),
        (LayerHelp::Up, AppAction::LayerUp),
        (LayerHelp::Down, AppAction::LayerDown),
        (LayerHelp::Rename, AppAction::RenameLayer),
        (LayerHelp::Properties, AppAction::LayerProperties),
    ];
    for (help, action_2) in actions {
        let target_2 = HelpTarget::Layer(help);
        assert_eq!(target_2.action(Tool::Pencil), action_2);
        for platform in PLATFORMS {
            let spec = help_form(target_2, Tool::Pencil, &Keymap::default_scheme(), platform);
            assert_eq!(
                spec.title,
                help_entry(help_key(action_2).expect("clave"))
                    .expect("texto")
                    .name
            );
        }
    }
}

#[test]
fn row_3_with_a_gesture_or_a_text_field_only_the_panel_can_be_opened() {
    let gesture_2 = Locks {
        gesture: true,
        stroke: true,
        ..Locks::default()
    };
    assert!(!is_blocked(
        AppAction::ToggleLayerPanel,
        gesture_2,
        Origin::Key
    ));
    assert!(!is_blocked(
        AppAction::ToggleLayerPanel,
        gesture_2,
        Origin::Menu
    ));
    for (action_2, _) in ACTIONS.iter().skip(1) {
        assert!(
            is_blocked(*action_2, gesture_2, Origin::Key),
            "{action_2:?}"
        );
        assert!(
            is_blocked(*action_2, gesture_2, Origin::Button),
            "{action_2:?}"
        );
    }
    let text_2 = Locks {
        text_focus: true,
        ..Locks::default()
    };
    for (action_2, _) in ACTIONS {
        assert!(is_blocked(action_2, text_2, Origin::Key), "{action_2:?}");
    }
    let modal = Locks {
        modal: true,
        ..Locks::default()
    };
    for (action_2, _) in ACTIONS {
        assert!(is_blocked(action_2, modal, Origin::Menu), "{action_2:?}");
    }
    for (action_2, _) in ACTIONS {
        assert!(
            !is_blocked(action_2, Locks::default(), Origin::Key),
            "{action_2:?}"
        );
    }
}

#[test]
fn row_3_typing_in_the_name_field_fires_no_action() {
    let events_2: Vec<Event> = "a;b'c()"
        .chars()
        .flat_map(|c| {
            [
                Event::Key {
                    key: Key::A,
                    physical_key: Some(Key::A),
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers::NONE,
                },
                Event::Text(c.to_string()),
            ]
        })
        .collect();
    assert!(presses(&gate_keys(&events_2, true)).is_empty());
}

// ---------------------------------------------------------------------------------------------
// Fila 4
// ---------------------------------------------------------------------------------------------

fn nav(count: usize, active_pos: usize) -> LayerNav {
    LayerNav {
        count,
        active_pos,
        can_add: count < 256,
    }
}

#[test]
fn row_4_the_layer_actions_are_enabled_according_to_the_stack() {
    // Con una capa: no se quita, ni sube ni baja.
    let a = nav(1, 0);
    assert!(!layer_enabled(AppAction::RemoveLayer, &a));
    assert!(!layer_enabled(AppAction::LayerUp, &a));
    assert!(!layer_enabled(AppAction::LayerDown, &a));
    assert!(layer_enabled(AppAction::NewLayer, &a));
    assert!(layer_enabled(AppAction::DuplicateLayer, &a));
    // Con tres: en medio todo; arriba no sube; abajo no baja.
    let middle = nav(3, 1);
    for (action_2, _) in ACTIONS {
        assert!(layer_enabled(action_2, &middle), "{action_2:?}");
    }
    assert!(!layer_enabled(AppAction::LayerUp, &nav(3, 2)));
    assert!(layer_enabled(AppAction::LayerDown, &nav(3, 2)));
    assert!(!layer_enabled(AppAction::LayerDown, &nav(3, 0)));
    assert!(layer_enabled(AppAction::LayerUp, &nav(3, 0)));
    // Con 256: ni nueva ni duplicar.
    let full = nav(256, 10);
    assert!(!layer_enabled(AppAction::NewLayer, &full));
    assert!(!layer_enabled(AppAction::DuplicateLayer, &full));
    assert!(layer_enabled(AppAction::RemoveLayer, &full));
    // Renombrar, propiedades y el panel no dependen de la pila.
    for n in [a, middle, full] {
        for action_2 in [
            AppAction::RenameLayer,
            AppAction::LayerProperties,
            AppAction::ToggleLayerPanel,
        ] {
            assert!(layer_enabled(action_2, &n), "{action_2:?}");
        }
    }
    // Lo que no es de capas no lo toca, y el valor por defecto lo deja todo habilitado.
    assert!(layer_enabled(AppAction::Undo, &a));
    for (action_2, _) in ACTIONS {
        assert!(
            layer_enabled(action_2, &LayerNav::default()),
            "{action_2:?}"
        );
    }
}
