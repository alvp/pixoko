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

const PLATAFORMAS: [Platform; 3] = [Platform::Linux, Platform::Windows, Platform::Mac];

const ACCIONES: [(AppAction, &str); 8] = [
    (AppAction::ToggleLayerPanel, "layer.panel"),
    (AppAction::NewLayer, "layer.new"),
    (AppAction::DuplicateLayer, "layer.duplicate"),
    (AppAction::RemoveLayer, "layer.remove"),
    (AppAction::LayerUp, "layer.up"),
    (AppAction::LayerDown, "layer.down"),
    (AppAction::RenameLayer, "layer.rename"),
    (AppAction::LayerProperties, "layer.properties"),
];

fn pulsa(c: char) -> Press {
    Press {
        key: Some(KeyId::Char(c)),
        command: false,
        shift: false,
        alt: false,
        other_ctrl: false,
        repeat: false,
    }
}

fn texto_de_la_zona(clave: &str) -> String {
    let spec = help_form_for(clave, &Keymap::default_scheme(), Platform::Linux).expect(clave);
    let campo = spec
        .fields
        .iter()
        .find(|f| f.id == HELP_ZONE)
        .expect("zona de texto");
    match &campo.kind {
        px_ui::dialog::FieldKind::ReadOnly { text, .. } => text.clone(),
        otro => panic!("{otro:?}"),
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 3
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_3_las_cuatro_teclas_dan_sus_acciones_y_la_tabla_no_tiene_conflictos() {
    let tabla = Keymap::default_scheme();
    assert_eq!(
        tabla.conflicts(),
        vec![],
        "la propiedad de conflictos de I.1 e I.5"
    );
    assert_eq!(tabla.rows().len(), 35);
    assert_eq!(tabla.rows().iter().filter(|f| f.command).count(), 9);
    for platform in PLATAFORMAS {
        for (c, accion) in [
            ('\'', AppAction::ToggleLayerPanel),
            (';', AppAction::NewLayer),
            (')', AppAction::LayerUp),
            ('(', AppAction::LayerDown),
        ] {
            assert_eq!(
                tabla.action_for(&pulsa(c), platform),
                Some(accion),
                "{c} en {platform:?}"
            );
            // Un símbolo no cuenta `Mayús` ni `Alt` (como `<` y `[`), pero sí `Ctrl`/`Cmd`.
            let mut con_mayus = pulsa(c);
            con_mayus.shift = true;
            assert_eq!(tabla.action_for(&con_mayus, platform), Some(accion));
            let mut con_command = pulsa(c);
            con_command.command = true;
            assert_eq!(tabla.action_for(&con_command, platform), None);
        }
        // Las demás cuatro no tienen tecla por defecto y `l` / `L` siguen sin asignar (I.1).
        for accion in [
            AppAction::DuplicateLayer,
            AppAction::RemoveLayer,
            AppAction::RenameLayer,
            AppAction::LayerProperties,
        ] {
            assert!(tabla.shown(accion, platform).is_none(), "{accion:?}");
        }
        for c in ['l', 'L', 'p', 'i', 'e'] {
            assert_eq!(tabla.action_for(&pulsa(c), platform), None, "{c}");
        }
    }
}

#[test]
fn fila_3_las_ocho_acciones_tienen_clave_texto_valido_y_son_editables() {
    assert_eq!(help_table().len(), 33);
    for (accion, clave) in ACCIONES {
        assert_eq!(help_key(accion), Some(clave));
        assert_eq!(action_of_key(clave), Some(accion));
        assert!(is_editable(accion), "{clave}");
        let entrada = help_entry(clave).expect(clave);
        assert!(entrada.name.chars().count() <= 20, "{clave}: nombre largo");
        assert!(
            wrap_text(entrada.description, ZONE_COLS).len() <= 8,
            "{clave}"
        );
        assert!(entrada.options.is_empty(), "{clave}");
        for texto in [entrada.name, entrada.description] {
            for c in texto.chars() {
                assert!(
                    INTERFACE_CHARSET.contains(c),
                    "{clave}: {c:?} fuera del juego"
                );
            }
        }
    }
}

#[test]
fn fila_3_la_ayuda_de_las_cuatro_teclas_dice_como_teclearlas_en_un_teclado_espanol() {
    // Condición 4: `'` es una tecla normal; `;` con Mayús+coma; `(` y `)` con Mayús+8 y Mayús+9.
    assert!(texto_de_la_zona("layer.panel").contains('\''));
    assert!(texto_de_la_zona("layer.new").contains("Mayús+coma"));
    assert!(texto_de_la_zona("layer.up").contains("Mayús+9"));
    assert!(texto_de_la_zona("layer.down").contains("Mayús+8"));
}

#[test]
fn fila_3_la_ayuda_del_candado_dice_que_protege_pixeles_y_que_paleta_y_modo_afectan_a_todas() {
    // Condición 3: el candado protege los píxeles de las herramientas de dibujo; los cambios de
    // paleta y de modo afectan a todas las capas.
    let texto = texto_de_la_zona("layer.properties");
    for buscado in ["candado", "píxeles", "paleta", "modo", "todas las capas"] {
        assert!(texto.contains(buscado), "falta {buscado:?} en {texto:?}");
    }
}

#[test]
fn fila_3_f1_sobre_cada_control_del_panel_da_la_ayuda_de_su_accion() {
    let esperado = [
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
    for (golpe, ayuda) in esperado {
        assert_eq!(layer_help_for(golpe), ayuda, "{golpe:?}");
    }
    let acciones = [
        (LayerHelp::Panel, AppAction::ToggleLayerPanel),
        (LayerHelp::New, AppAction::NewLayer),
        (LayerHelp::Duplicate, AppAction::DuplicateLayer),
        (LayerHelp::Remove, AppAction::RemoveLayer),
        (LayerHelp::Up, AppAction::LayerUp),
        (LayerHelp::Down, AppAction::LayerDown),
        (LayerHelp::Rename, AppAction::RenameLayer),
        (LayerHelp::Properties, AppAction::LayerProperties),
    ];
    for (ayuda, accion) in acciones {
        let destino = HelpTarget::Layer(ayuda);
        assert_eq!(destino.action(Tool::Pencil), accion);
        for platform in PLATAFORMAS {
            let spec = help_form(destino, Tool::Pencil, &Keymap::default_scheme(), platform);
            assert_eq!(
                spec.title,
                help_entry(help_key(accion).expect("clave"))
                    .expect("texto")
                    .name
            );
        }
    }
}

#[test]
fn fila_3_con_un_gesto_o_un_campo_de_texto_solo_el_panel_se_deja_abrir() {
    let gesto = Locks {
        gesture: true,
        stroke: true,
        ..Locks::default()
    };
    assert!(!is_blocked(AppAction::ToggleLayerPanel, gesto, Origin::Key));
    assert!(!is_blocked(
        AppAction::ToggleLayerPanel,
        gesto,
        Origin::Menu
    ));
    for (accion, _) in ACCIONES.iter().skip(1) {
        assert!(is_blocked(*accion, gesto, Origin::Key), "{accion:?}");
        assert!(is_blocked(*accion, gesto, Origin::Button), "{accion:?}");
    }
    let texto = Locks {
        text_focus: true,
        ..Locks::default()
    };
    for (accion, _) in ACCIONES {
        assert!(is_blocked(accion, texto, Origin::Key), "{accion:?}");
    }
    let modal = Locks {
        modal: true,
        ..Locks::default()
    };
    for (accion, _) in ACCIONES {
        assert!(is_blocked(accion, modal, Origin::Menu), "{accion:?}");
    }
    for (accion, _) in ACCIONES {
        assert!(
            !is_blocked(accion, Locks::default(), Origin::Key),
            "{accion:?}"
        );
    }
}

#[test]
fn fila_3_teclear_en_el_campo_del_nombre_no_dispara_ninguna_accion() {
    let eventos: Vec<Event> = "a;b'c()"
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
    assert!(presses(&gate_keys(&eventos, true)).is_empty());
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
fn fila_4_las_acciones_de_capa_se_habilitan_segun_la_pila() {
    // Con una capa: no se quita, ni sube ni baja.
    let una = nav(1, 0);
    assert!(!layer_enabled(AppAction::RemoveLayer, &una));
    assert!(!layer_enabled(AppAction::LayerUp, &una));
    assert!(!layer_enabled(AppAction::LayerDown, &una));
    assert!(layer_enabled(AppAction::NewLayer, &una));
    assert!(layer_enabled(AppAction::DuplicateLayer, &una));
    // Con tres: en medio todo; arriba no sube; abajo no baja.
    let medio = nav(3, 1);
    for (accion, _) in ACCIONES {
        assert!(layer_enabled(accion, &medio), "{accion:?}");
    }
    assert!(!layer_enabled(AppAction::LayerUp, &nav(3, 2)));
    assert!(layer_enabled(AppAction::LayerDown, &nav(3, 2)));
    assert!(!layer_enabled(AppAction::LayerDown, &nav(3, 0)));
    assert!(layer_enabled(AppAction::LayerUp, &nav(3, 0)));
    // Con 256: ni nueva ni duplicar.
    let llena = nav(256, 10);
    assert!(!layer_enabled(AppAction::NewLayer, &llena));
    assert!(!layer_enabled(AppAction::DuplicateLayer, &llena));
    assert!(layer_enabled(AppAction::RemoveLayer, &llena));
    // Renombrar, propiedades y el panel no dependen de la pila.
    for n in [una, medio, llena] {
        for accion in [
            AppAction::RenameLayer,
            AppAction::LayerProperties,
            AppAction::ToggleLayerPanel,
        ] {
            assert!(layer_enabled(accion, &n), "{accion:?}");
        }
    }
    // Lo que no es de capas no lo toca, y el valor por defecto lo deja todo habilitado.
    assert!(layer_enabled(AppAction::Undo, &una));
    for (accion, _) in ACCIONES {
        assert!(layer_enabled(accion, &LayerNav::default()), "{accion:?}");
    }
}
