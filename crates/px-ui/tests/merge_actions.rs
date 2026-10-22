//! Acciones, menú, ayuda y teclas de combinar, aplanar y exportar (specs/hito-3-combinar.md,
//! unidad 3.5, fila 10, decisiones D10 y D11 y condición (d)). En ROJO hasta la lógica:
//! `AppAction::{MergeLayerDown, MergeVisibleLayers, FlattenLayers, ExportFlat}` no existen todavía.
//!
//! - Tres acciones de capa con clave de ayuda (`layer.merge-down`, `layer.merge-visible`,
//!   `layer.flatten`), texto válido y editables; `Exportar PNG aplanado…` es de archivo: fija y
//!   sin clave de ayuda.
//! - Ninguna de las cuatro tiene tecla por defecto; la tabla sigue en 36 filas sin conflictos.
//! - `layer_enabled`: `Combinar below` ⇔ hay capa debajo; `Combinar visible_2` y `Aplanar` ⇔ hay
//!   más de una capa; `Exportar` siempre.
//! - `is_blocked`: como el resto de acciones de documento (un trazo, un campo de texto o un
//!   modal las paran).
//! - El menú: `Layer` y `Archivo` con sus entradas nuevas y contraste de sus rótulos.

use px_ui::actions::{AppAction, LayerNav, Locks, Origin, Platform, is_blocked, layer_enabled};
use px_ui::dialog_layout::ZONE_COLS;
use px_ui::help_texts::{action_of_key, help_entry, help_key, help_table, is_editable};
use px_ui::keymap::Keymap;
use px_ui::licenses_view::wrap_text;
use px_ui::menu::MENUS;
use px_ui::text::INTERFACE_CHARSET;

const PLATFORMS: [Platform; 3] = [Platform::Linux, Platform::Windows, Platform::Mac];

const ACTIONS_OF_LAYER: [(AppAction, &str); 3] = [
    (AppAction::MergeLayerDown, "layer.merge-down"),
    (AppAction::MergeVisibleLayers, "layer.merge-visible"),
    (AppAction::FlattenLayers, "layer.flatten"),
];

const THE_FOUR: [AppAction; 4] = [
    AppAction::MergeLayerDown,
    AppAction::MergeVisibleLayers,
    AppAction::FlattenLayers,
    AppAction::ExportFlat,
];

fn stack(count: usize, active_pos: usize) -> LayerNav {
    LayerNav {
        count,
        active_pos,
        can_add: count < 256,
    }
}

#[test]
fn row_10_the_three_layer_actions_have_a_key_valid_text_and_are_editable() {
    assert_eq!(help_table().len(), 34);
    for (action_2, key) in ACTIONS_OF_LAYER {
        assert_eq!(help_key(action_2), Some(key));
        assert_eq!(action_of_key(key), Some(action_2));
        assert!(is_editable(action_2), "{key}");
        let input_2 = help_entry(key).expect(key);
        assert!(input_2.name.chars().count() <= 20, "{key}: nombre largo");
        assert!(
            wrap_text(input_2.description, ZONE_COLS).len() <= 8,
            "{key}: más de 8 líneas"
        );
        for text_2 in [input_2.name, input_2.description] {
            for c in text_2.chars() {
                assert!(
                    INTERFACE_CHARSET.contains(c),
                    "{key}: {c:?} fuera del juego"
                );
            }
        }
        // Sin tecla por defecto, y la ayuda lo dice (D10).
        assert!(
            input_2.description.contains("No tiene atajo por defecto"),
            "{key}"
        );
    }
    // `Exportar PNG aplanado…` es una acción de archivo: ni clave de ayuda ni editor.
    assert_eq!(help_key(AppAction::ExportFlat), None);
    assert!(!is_editable(AppAction::ExportFlat));
}

#[test]
fn row_10_none_of_the_four_has_a_default_key_and_the_table_does_not_change() {
    let table = Keymap::default_scheme();
    assert_eq!(table.conflicts(), vec![]);
    assert_eq!(table.rows().len(), 36);
    assert_eq!(table.rows().iter().filter(|f| f.command).count(), 9);
    for platform in PLATFORMS {
        for action_2 in THE_FOUR {
            assert!(table.shown(action_2, platform).is_none(), "{action_2:?}");
        }
    }
}

#[test]
fn row_10_layer_enabled_follows_the_layer_stack() {
    // Una capa: nada que combinar ni aplanar; `Exportar` siempre.
    let a = stack(1, 0);
    assert!(!layer_enabled(AppAction::MergeLayerDown, &a));
    assert!(!layer_enabled(AppAction::MergeVisibleLayers, &a));
    assert!(!layer_enabled(AppAction::FlattenLayers, &a));
    assert!(layer_enabled(AppAction::ExportFlat, &a));
    // Tres capas con la activa abajo: no hay capa debajo, pero sí más de una capa.
    let below = stack(3, 0);
    assert!(!layer_enabled(AppAction::MergeLayerDown, &below));
    assert!(layer_enabled(AppAction::MergeVisibleLayers, &below));
    assert!(layer_enabled(AppAction::FlattenLayers, &below));
    // La activa en medio y arriba.
    for position in [1, 2] {
        let nav = stack(3, position);
        assert!(layer_enabled(AppAction::MergeLayerDown, &nav), "{position}");
        assert!(layer_enabled(AppAction::MergeVisibleLayers, &nav));
        assert!(layer_enabled(AppAction::FlattenLayers, &nav));
    }
    // Con 256 capas combinar sigue valiendo (solo reduce).
    let full = stack(256, 255);
    assert!(layer_enabled(AppAction::MergeLayerDown, &full));
    assert!(layer_enabled(AppAction::FlattenLayers, &full));
}

#[test]
fn row_10_the_locks_are_those_of_the_document_actions() {
    let free = Locks::default();
    let with = |f: fn(&mut Locks)| {
        let mut locks = Locks::default();
        f(&mut locks);
        locks
    };
    for action_2 in THE_FOUR {
        for source_2 in [Origin::Menu, Origin::Key, Origin::Button] {
            assert!(!is_blocked(action_2, free, source_2), "{action_2:?}");
            for locks in [
                with(|l| l.gesture = true),
                with(|l| l.text_focus = true),
                with(|l| l.modal = true),
                with(|l| l.dialog = true),
            ] {
                assert!(
                    is_blocked(action_2, locks, source_2),
                    "{action_2:?} {locks:?}"
                );
            }
        }
    }
}

#[test]
fn row_10_the_menu_has_the_new_entries_in_their_place() {
    let menu = |title: &str| {
        MENUS
            .iter()
            .find(|m| m.title == title)
            .unwrap_or_else(|| panic!("menú {title}"))
    };
    let file: Vec<AppAction> = menu("Archivo").items.iter().map(|i| i.action).collect();
    let save_as = file
        .iter()
        .position(|a| *a == AppAction::SaveAs)
        .expect("Guardar como…");
    assert_eq!(file[save_as + 1], AppAction::ExportFlat);
    assert_eq!(file[save_as + 2], AppAction::Quit);
    let export = menu("Archivo")
        .items
        .iter()
        .find(|i| i.action == AppAction::ExportFlat)
        .expect("Exportar");
    assert_eq!(export.label, "Exportar PNG aplanado…");
    assert!(!export.separator_before);

    let layer_2: Vec<(&str, AppAction)> = menu("Capa")
        .items
        .iter()
        .map(|i| (i.label, i.action))
        .collect();
    let lower = layer_2
        .iter()
        .position(|(_, a)| *a == AppAction::LayerDown)
        .expect("Bajar");
    assert_eq!(
        layer_2[lower + 1..lower + 4],
        [
            ("Combinar abajo", AppAction::MergeLayerDown),
            ("Combinar visibles", AppAction::MergeVisibleLayers),
            ("Aplanar", AppAction::FlattenLayers),
        ]
    );
    // Sin separador nuevo: el único sigue siendo el de `Salir`.
    for menu in MENUS.iter() {
        for item in menu.items {
            assert_eq!(item.separator_before, item.action == AppAction::Quit);
        }
    }
    // Todo texto de las entradas nuevas está en el juego de caracteres de la interfaz.
    for item in menu("Capa")
        .items
        .iter()
        .chain(menu("Archivo").items.iter())
    {
        for c in item.label.chars() {
            assert!(INTERFACE_CHARSET.contains(c), "{}: {c:?}", item.label);
        }
    }
}
