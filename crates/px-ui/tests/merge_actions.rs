//! Acciones, menú, ayuda y teclas de combinar, aplanar y exportar (specs/hito-3-combinar.md,
//! unidad 3.5, fila 10, decisiones D10 y D11 y condición (d)). En ROJO hasta la lógica:
//! `AppAction::{MergeLayerDown, MergeVisibleLayers, FlattenLayers, ExportFlat}` no existen todavía.
//!
//! - Tres acciones de capa con clave de ayuda (`layer.merge-down`, `layer.merge-visible`,
//!   `layer.flatten`), texto válido y editables; `Exportar PNG aplanado…` es de archivo: fija y
//!   sin clave de ayuda.
//! - Ninguna de las cuatro tiene tecla por defecto; la tabla sigue en 35 filas sin conflictos.
//! - `layer_enabled`: `Combinar abajo` ⇔ hay capa debajo; `Combinar visibles` y `Aplanar` ⇔ hay
//!   más de una capa; `Exportar` siempre.
//! - `is_blocked`: como el resto de acciones de documento (un trazo, un campo de texto o un
//!   modal las paran).
//! - El menú: `Capa` y `Archivo` con sus entradas nuevas y contraste de sus rótulos.

use px_ui::actions::{AppAction, LayerNav, Locks, Origin, Platform, is_blocked, layer_enabled};
use px_ui::dialog_layout::ZONE_COLS;
use px_ui::help_texts::{action_of_key, help_entry, help_key, help_table, is_editable};
use px_ui::keymap::Keymap;
use px_ui::licenses_view::wrap_text;
use px_ui::menu::MENUS;
use px_ui::text::INTERFACE_CHARSET;

const PLATAFORMAS: [Platform; 3] = [Platform::Linux, Platform::Windows, Platform::Mac];

const ACCIONES_DE_CAPA: [(AppAction, &str); 3] = [
    (AppAction::MergeLayerDown, "layer.merge-down"),
    (AppAction::MergeVisibleLayers, "layer.merge-visible"),
    (AppAction::FlattenLayers, "layer.flatten"),
];

const LAS_CUATRO: [AppAction; 4] = [
    AppAction::MergeLayerDown,
    AppAction::MergeVisibleLayers,
    AppAction::FlattenLayers,
    AppAction::ExportFlat,
];

fn pila(count: usize, active_pos: usize) -> LayerNav {
    LayerNav {
        count,
        active_pos,
        can_add: count < 256,
    }
}

#[test]
fn fila_10_las_tres_acciones_de_capa_tienen_clave_texto_valido_y_son_editables() {
    assert_eq!(help_table().len(), 33);
    for (accion, clave) in ACCIONES_DE_CAPA {
        assert_eq!(help_key(accion), Some(clave));
        assert_eq!(action_of_key(clave), Some(accion));
        assert!(is_editable(accion), "{clave}");
        let entrada = help_entry(clave).expect(clave);
        assert!(entrada.name.chars().count() <= 20, "{clave}: nombre largo");
        assert!(
            wrap_text(entrada.description, ZONE_COLS).len() <= 8,
            "{clave}: más de 8 líneas"
        );
        for texto in [entrada.name, entrada.description] {
            for c in texto.chars() {
                assert!(
                    INTERFACE_CHARSET.contains(c),
                    "{clave}: {c:?} fuera del juego"
                );
            }
        }
        // Sin tecla por defecto, y la ayuda lo dice (D10).
        assert!(
            entrada.description.contains("No tiene atajo por defecto"),
            "{clave}"
        );
    }
    // `Exportar PNG aplanado…` es una acción de archivo: ni clave de ayuda ni editor.
    assert_eq!(help_key(AppAction::ExportFlat), None);
    assert!(!is_editable(AppAction::ExportFlat));
}

#[test]
fn fila_10_ninguna_de_las_cuatro_tiene_tecla_por_defecto_y_la_tabla_no_cambia() {
    let tabla = Keymap::default_scheme();
    assert_eq!(tabla.conflicts(), vec![]);
    assert_eq!(tabla.rows().len(), 35);
    assert_eq!(tabla.rows().iter().filter(|f| f.command).count(), 9);
    for platform in PLATAFORMAS {
        for accion in LAS_CUATRO {
            assert!(tabla.shown(accion, platform).is_none(), "{accion:?}");
        }
    }
}

#[test]
fn fila_10_layer_enabled_sigue_la_pila_de_capas() {
    // Una capa: nada que combinar ni aplanar; `Exportar` siempre.
    let una = pila(1, 0);
    assert!(!layer_enabled(AppAction::MergeLayerDown, &una));
    assert!(!layer_enabled(AppAction::MergeVisibleLayers, &una));
    assert!(!layer_enabled(AppAction::FlattenLayers, &una));
    assert!(layer_enabled(AppAction::ExportFlat, &una));
    // Tres capas con la activa abajo: no hay capa debajo, pero sí más de una capa.
    let abajo = pila(3, 0);
    assert!(!layer_enabled(AppAction::MergeLayerDown, &abajo));
    assert!(layer_enabled(AppAction::MergeVisibleLayers, &abajo));
    assert!(layer_enabled(AppAction::FlattenLayers, &abajo));
    // La activa en medio y arriba.
    for posicion in [1, 2] {
        let nav = pila(3, posicion);
        assert!(layer_enabled(AppAction::MergeLayerDown, &nav), "{posicion}");
        assert!(layer_enabled(AppAction::MergeVisibleLayers, &nav));
        assert!(layer_enabled(AppAction::FlattenLayers, &nav));
    }
    // Con 256 capas combinar sigue valiendo (solo reduce).
    let llena = pila(256, 255);
    assert!(layer_enabled(AppAction::MergeLayerDown, &llena));
    assert!(layer_enabled(AppAction::FlattenLayers, &llena));
}

#[test]
fn fila_10_los_bloqueos_son_los_de_las_acciones_de_documento() {
    let libre = Locks::default();
    let con = |f: fn(&mut Locks)| {
        let mut locks = Locks::default();
        f(&mut locks);
        locks
    };
    for accion in LAS_CUATRO {
        for origen in [Origin::Menu, Origin::Key, Origin::Button] {
            assert!(!is_blocked(accion, libre, origen), "{accion:?}");
            for locks in [
                con(|l| l.gesture = true),
                con(|l| l.text_focus = true),
                con(|l| l.modal = true),
                con(|l| l.dialog = true),
            ] {
                assert!(is_blocked(accion, locks, origen), "{accion:?} {locks:?}");
            }
        }
    }
}

#[test]
fn fila_10_el_menu_tiene_las_entradas_nuevas_en_su_sitio() {
    let menu = |titulo: &str| {
        MENUS
            .iter()
            .find(|m| m.title == titulo)
            .unwrap_or_else(|| panic!("menú {titulo}"))
    };
    let archivo: Vec<AppAction> = menu("Archivo").items.iter().map(|i| i.action).collect();
    let guardar_como = archivo
        .iter()
        .position(|a| *a == AppAction::SaveAs)
        .expect("Guardar como…");
    assert_eq!(archivo[guardar_como + 1], AppAction::ExportFlat);
    assert_eq!(archivo[guardar_como + 2], AppAction::Quit);
    let exportar = menu("Archivo")
        .items
        .iter()
        .find(|i| i.action == AppAction::ExportFlat)
        .expect("Exportar");
    assert_eq!(exportar.label, "Exportar PNG aplanado…");
    assert!(!exportar.separator_before);

    let capa: Vec<(&str, AppAction)> = menu("Capa")
        .items
        .iter()
        .map(|i| (i.label, i.action))
        .collect();
    let bajar = capa
        .iter()
        .position(|(_, a)| *a == AppAction::LayerDown)
        .expect("Bajar");
    assert_eq!(
        capa[bajar + 1..bajar + 4],
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
