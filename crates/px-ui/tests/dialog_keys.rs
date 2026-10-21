//! Tests del teclado y el foco de los diálogos `px_ui::dialog_keys` (specs/hito-2-dialogos-
//! modales.md, unidad 18, decisiones 5, 7 y 8, «Casos» filas 5 y 9 —parte pura— y propiedad 4).
//! Sin ventana ni banco de egui: **la parte del banco de la fila 9** (cada texto y cada marca
//! pintados frente a su fondo) es de `dialog_view.rs`. **En rojo** hasta que exista el módulo.
//!
//! - Fila 5: `key_action` con cada foco (`Field` numérico, hexadecimal con y sin pendiente,
//!   `Option`, casilla, `Minus`, `Plus`, `Zone`, `Aceptar`, `Cancelar` y sin foco) × `Intro` y
//!   `Escape` × válido e inválido × `repeat`, contra una tabla escrita a mano y contra un oráculo
//!   aparte; un diálogo sin `Cancelar` (`licenses_form`); `dialog_key` con y sin modificadores.
//! - Fila 9 (pura): `initial_focus`, `focus_order` y `next_focus` del formulario de los cinco
//!   tipos (con la opción 2 y sin ella, con un campo inválido y con todo válido) y de
//!   `licenses_form`; `CONTRAST_PAIRS` con `contrast_ratio` y las cifras de «Lo comprobado».
//! - Propiedad 4: foco y teclado coherentes con formularios y estados al azar (`dialog_random`).
//!
//! Derives que se suponen (la spec no los lista): `Dialog`, `Values`, `Value`, `Validity`,
//! `Reaction`, `Event`, `NumberError` y `SpecError` son `Clone + Debug + PartialEq`;
//! `FocusTarget`, `DialogKey` y `KeyAction` son `Clone + Copy + Debug + PartialEq + Eq`;
//! `DialogKind`, `Clone + Copy + Debug + PartialEq + Eq`.
//!
//! Decisiones donde la spec no fija un detalle:
//! - `next_focus` con `current = None` se trata como «un foco que ya no existe»: hacia delante da
//!   el primero y hacia atrás el último.
//! - Un `Choice` es una parada por opción (`Option(id, i)`); el foco inicial de un grupo no se
//!   fija (solo que sea una opción de ese grupo). Se prueba con `matches!`.
//! - `Minus` y `Plus` están siempre en el orden, también con el botón deshabilitado en el
//!   límite: la spec los llama «paradas de tabulación» sin excepción y solo exceptúa `Aceptar`.
//! - `Intro` con un foco que no existe o con un hexadecimal oculto no se prueba (la spec no lo
//!   dice); tampoco `Cancelar` como foco en un diálogo sin `Cancelar`.
//! - `Accept` queda fuera del orden con el `check` que falla aunque todos los campos sean
//!   válidos (el conjunto es inválido).
//! - Un formulario vacío tiene como foco inicial `Aceptar`; su orden es `Aceptar` (y `Cancelar`
//!   si lo hay).
//! - De `CONTRAST_PAIRS` se exige: no vacía; el mínimo de cada par es 4,5 o 3,0 (con un texto
//!   —`TEXT`, `TEXT_DIM`, `WARNING` como primer color— el mínimo es 4,5); cada par cumple su
//!   mínimo; contiene los pares de «Lo comprobado» (sus cifras con tolerancia 0,01, y los de
//!   texto con 4,5 y los del anillo con 3,0); ninguno menciona `TEXT_DISABLED`.

use egui::{Key, Modifiers};
use proptest::prelude::*;
use px_ui::dialog::{Dialog, DialogKind, Event, FieldKind, FieldSpec, FormSpec, Reaction, Rows};
use px_ui::dialog_keys::{
    DialogKey, FocusTarget, KeyAction, dialog_key, focus_order, initial_focus, key_action,
    next_focus,
};
use px_ui::dialog_view::CONTRAST_PAIRS;
use px_ui::licenses_view::{LICENSES_FIELD, licenses_form};
use px_ui::theme::{
    ACCENT, SURFACE_0, SURFACE_1, SURFACE_2, SURFACE_3, TEXT, TEXT_DIM, TEXT_DISABLED, WARNING,
    contrast_ratio,
};

mod common;
use common::modificadores;
#[path = "common/dialog_forms.rs"]
mod dialog_forms;
#[path = "common/dialog_random.rs"]
mod dialog_random;

use dialog_forms::{all_kinds, open};
use dialog_random::{FormRecipe, build, event_of, event_recipe, form_recipe};

// ---------------------------------------------------------------------------------------------
// Ayudas
// ---------------------------------------------------------------------------------------------

fn numero(d: &mut Dialog, field: &'static str, text: &str) {
    assert_eq!(
        d.event(Event::Number {
            field,
            text: text.to_string()
        }),
        Reaction::None
    );
}

fn elige(d: &mut Dialog, field: &'static str, index: usize) {
    assert_eq!(d.event(Event::Choice { field, index }), Reaction::None);
}

fn hex(d: &mut Dialog, text: &str) {
    assert_eq!(
        d.event(Event::Hex {
            field: "color",
            text: text.to_string()
        }),
        Reaction::None
    );
}

/// Los cinco tipos con el hexadecimal visible (opción 2) y todo válido.
fn valido() -> Dialog {
    let mut d = open(all_kinds());
    elige(&mut d, "modo", 2);
    assert!(d.is_valid());
    d
}

/// Un numérico vacío: el conjunto es inválido.
fn invalido() -> Dialog {
    let mut d = valido();
    numero(&mut d, "ancho", "");
    assert!(!d.is_valid());
    d
}

/// Un pendiente válido en el hexadecimal (`#abc`): el conjunto sigue siendo válido.
fn pendiente_valido() -> Dialog {
    let mut d = valido();
    hex(&mut d, "#abc");
    assert!(d.hex_pending("color"));
    assert!(d.is_valid());
    d
}

/// Un pendiente inválido (`zz`): el conjunto es inválido.
fn pendiente_invalido() -> Dialog {
    let mut d = valido();
    hex(&mut d, "zz");
    assert!(d.hex_pending("color"));
    assert!(!d.is_valid());
    d
}

fn es_hex(d: &Dialog, id: &str) -> bool {
    d.spec()
        .fields
        .iter()
        .any(|f| f.id == id && matches!(f.kind, FieldKind::Hex { .. }))
}

/// Los focos que hay en `valido()`: uno de cada clase y ninguno.
fn focos() -> Vec<(&'static str, Option<FocusTarget>)> {
    vec![
        ("sin foco", None),
        ("numérico", Some(FocusTarget::Field("ancho"))),
        ("hexadecimal", Some(FocusTarget::Field("color"))),
        ("opción", Some(FocusTarget::Option("modo", 1))),
        ("casilla", Some(FocusTarget::Field("opaco"))),
        ("menos", Some(FocusTarget::Minus("ancho"))),
        ("más", Some(FocusTarget::Plus("ancho"))),
        ("zona", Some(FocusTarget::Zone("informe"))),
        ("aceptar", Some(FocusTarget::Accept)),
        ("cancelar", Some(FocusTarget::Cancel)),
    ]
}

/// El oráculo de la decisión 5, escrito aparte.
fn oraculo(d: &Dialog, focus: Option<FocusTarget>, key: DialogKey, repeat: bool) -> KeyAction {
    if repeat {
        return KeyAction::Nothing;
    }
    let hex_pendiente = match focus {
        Some(FocusTarget::Field(id)) if es_hex(d, id) && d.hex_pending(id) => Some(id),
        _ => None,
    };
    match key {
        DialogKey::Escape => match hex_pendiente {
            Some(id) => KeyAction::RevertHex(id),
            None => KeyAction::Cancel,
        },
        DialogKey::Enter => match focus {
            Some(FocusTarget::Field(id)) if hex_pendiente == Some(id) => KeyAction::ConfirmHex(id),
            Some(
                FocusTarget::Zone(_)
                | FocusTarget::Minus(_)
                | FocusTarget::Plus(_)
                | FocusTarget::Accept
                | FocusTarget::Cancel,
            ) => KeyAction::Nothing,
            _ => {
                if d.is_valid() {
                    KeyAction::Accept
                } else {
                    KeyAction::Nothing
                }
            }
        },
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 5: dialog_key
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_5_dialog_key_reconoce_intro_y_escape_sin_modificadores() {
    assert_eq!(
        dialog_key(Key::Enter, Modifiers::NONE),
        Some(DialogKey::Enter)
    );
    assert_eq!(
        dialog_key(Key::Escape, Modifiers::NONE),
        Some(DialogKey::Escape)
    );
}

#[test]
fn fila_5_dialog_key_con_cualquier_modificador_da_none() {
    for bits in 1u8..32 {
        let m = modificadores(bits);
        assert_eq!(dialog_key(Key::Enter, m), None, "Intro con {m:?}");
        assert_eq!(dialog_key(Key::Escape, m), None, "Escape con {m:?}");
    }
    // Los tres que pide la spec, por separado.
    let ctrl = Modifiers {
        ctrl: true,
        ..Modifiers::NONE
    };
    let alt = Modifiers {
        alt: true,
        ..Modifiers::NONE
    };
    let mayus = Modifiers {
        shift: true,
        ..Modifiers::NONE
    };
    for m in [ctrl, alt, mayus, Modifiers::COMMAND, Modifiers::ALT] {
        assert_eq!(dialog_key(Key::Enter, m), None);
        assert_eq!(dialog_key(Key::Escape, m), None);
    }
}

#[test]
fn fila_5_dialog_key_solo_reconoce_esas_dos_teclas() {
    for &key in Key::ALL {
        let esperado = match key {
            Key::Enter => Some(DialogKey::Enter),
            Key::Escape => Some(DialogKey::Escape),
            _ => None,
        };
        assert_eq!(dialog_key(key, Modifiers::NONE), esperado, "{key:?}");
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 5: key_action
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_5_escape_cancela_con_cualquier_foco_si_no_hay_pendiente() {
    for dialogo in [valido(), invalido()] {
        for (nombre, foco) in focos() {
            assert_eq!(
                key_action(&dialogo, foco, DialogKey::Escape, false),
                KeyAction::Cancel,
                "foco {nombre}"
            );
        }
    }
}

#[test]
fn fila_5_escape_en_un_hexadecimal_con_pendiente_revierte_y_el_siguiente_cancela() {
    for mut dialogo in [pendiente_valido(), pendiente_invalido()] {
        let foco = Some(FocusTarget::Field("color"));
        assert_eq!(
            key_action(&dialogo, foco, DialogKey::Escape, false),
            KeyAction::RevertHex("color")
        );
        // El campo se revierte (lo hace quien recibe `RevertHex`) y el siguiente `Escape` cancela.
        assert_eq!(
            dialogo.event(Event::HexRevert { field: "color" }),
            Reaction::None
        );
        assert!(!dialogo.hex_pending("color"));
        assert_eq!(
            key_action(&dialogo, foco, DialogKey::Escape, false),
            KeyAction::Cancel
        );
    }
}

#[test]
fn fila_5_escape_con_pendiente_pero_el_foco_en_otro_sitio_cancela() {
    for dialogo in [pendiente_valido(), pendiente_invalido()] {
        for (nombre, foco) in focos() {
            if foco == Some(FocusTarget::Field("color")) {
                continue;
            }
            assert_eq!(
                key_action(&dialogo, foco, DialogKey::Escape, false),
                KeyAction::Cancel,
                "foco {nombre}"
            );
        }
    }
}

#[test]
fn fila_5_intro_en_un_hexadecimal_con_pendiente_confirma_el_campo_sea_valido_o_no() {
    for dialogo in [pendiente_valido(), pendiente_invalido()] {
        assert_eq!(
            key_action(
                &dialogo,
                Some(FocusTarget::Field("color")),
                DialogKey::Enter,
                false
            ),
            KeyAction::ConfirmHex("color")
        );
    }
}

#[test]
fn fila_5_intro_acepta_solo_si_todo_es_valido_en_campo_opcion_casilla_hex_sin_pendiente_y_sin_foco()
{
    let con_accept = [
        ("sin foco", None),
        ("numérico", Some(FocusTarget::Field("ancho"))),
        (
            "hexadecimal sin pendiente",
            Some(FocusTarget::Field("color")),
        ),
        ("opción", Some(FocusTarget::Option("modo", 0))),
        ("opción 2", Some(FocusTarget::Option("modo", 2))),
        ("casilla", Some(FocusTarget::Field("opaco"))),
    ];
    let bueno = valido();
    let malo = invalido();
    for (nombre, foco) in con_accept {
        assert_eq!(
            key_action(&bueno, foco, DialogKey::Enter, false),
            KeyAction::Accept,
            "válido, foco {nombre}"
        );
        assert_eq!(
            key_action(&malo, foco, DialogKey::Enter, false),
            KeyAction::Nothing,
            "inválido, foco {nombre}"
        );
    }
    // Con un pendiente válido en otro campo el conjunto es válido: acepta desde el numérico.
    assert_eq!(
        key_action(
            &pendiente_valido(),
            Some(FocusTarget::Field("ancho")),
            DialogKey::Enter,
            false
        ),
        KeyAction::Accept
    );
    // Con un pendiente inválido en otro campo, el conjunto no lo es.
    assert_eq!(
        key_action(
            &pendiente_invalido(),
            Some(FocusTarget::Field("ancho")),
            DialogKey::Enter,
            false
        ),
        KeyAction::Nothing
    );
}

#[test]
fn fila_5_intro_no_hace_nada_en_la_zona_y_en_los_cuatro_botones() {
    let botones = [
        Some(FocusTarget::Zone("informe")),
        Some(FocusTarget::Minus("ancho")),
        Some(FocusTarget::Plus("ancho")),
        Some(FocusTarget::Minus("alto")),
        Some(FocusTarget::Plus("alto")),
        Some(FocusTarget::Accept),
        Some(FocusTarget::Cancel),
    ];
    for dialogo in [
        valido(),
        invalido(),
        pendiente_valido(),
        pendiente_invalido(),
    ] {
        for foco in botones {
            assert_eq!(
                key_action(&dialogo, foco, DialogKey::Enter, false),
                KeyAction::Nothing,
                "{foco:?}"
            );
        }
    }
}

#[test]
fn fila_5_con_repeat_siempre_nothing() {
    for dialogo in [
        valido(),
        invalido(),
        pendiente_valido(),
        pendiente_invalido(),
    ] {
        for (nombre, foco) in focos() {
            for key in [DialogKey::Enter, DialogKey::Escape] {
                assert_eq!(
                    key_action(&dialogo, foco, key, true),
                    KeyAction::Nothing,
                    "foco {nombre}, {key:?}"
                );
            }
        }
    }
}

#[test]
fn fila_5_la_tabla_completa_coincide_con_el_oraculo() {
    let estados: [(&str, Dialog); 4] = [
        ("válido", valido()),
        ("inválido", invalido()),
        ("pendiente válido", pendiente_valido()),
        ("pendiente inválido", pendiente_invalido()),
    ];
    for (estado, dialogo) in &estados {
        for (nombre, foco) in focos() {
            for key in [DialogKey::Enter, DialogKey::Escape] {
                for repeat in [false, true] {
                    assert_eq!(
                        key_action(dialogo, foco, key, repeat),
                        oraculo(dialogo, foco, key, repeat),
                        "{estado}, foco {nombre}, {key:?}, repeat {repeat}"
                    );
                }
            }
        }
    }
}

#[test]
fn fila_5_la_zona_y_los_botones_no_son_un_hexadecimal_aunque_haya_pendiente() {
    // Escape en un botón con un pendiente en el hexadecimal: cancela, no revierte.
    let d = pendiente_invalido();
    assert_eq!(
        key_action(&d, Some(FocusTarget::Accept), DialogKey::Escape, false),
        KeyAction::Cancel
    );
    assert_eq!(
        key_action(
            &d,
            Some(FocusTarget::Zone("informe")),
            DialogKey::Escape,
            false
        ),
        KeyAction::Cancel
    );
}

fn licencias() -> Dialog {
    open(licenses_form("x"))
}

#[test]
fn fila_5_un_dialogo_sin_cancelar_se_cierra_con_escape() {
    let d = licencias();
    assert_eq!(d.spec().cancel, None);
    for foco in [
        None,
        Some(FocusTarget::Zone(LICENSES_FIELD)),
        Some(FocusTarget::Accept),
    ] {
        assert_eq!(
            key_action(&d, foco, DialogKey::Escape, false),
            KeyAction::Cancel,
            "{foco:?}"
        );
    }
}

#[test]
fn fila_5_en_licencias_intro_en_la_zona_y_en_el_boton_no_hace_nada_y_sin_foco_acepta() {
    let d = licencias();
    assert_eq!(
        key_action(
            &d,
            Some(FocusTarget::Zone(LICENSES_FIELD)),
            DialogKey::Enter,
            false
        ),
        KeyAction::Nothing
    );
    assert_eq!(
        key_action(&d, Some(FocusTarget::Accept), DialogKey::Enter, false),
        KeyAction::Nothing
    );
    // Sin foco y todo válido, `Intro` acepta (cierra el diálogo).
    assert_eq!(
        key_action(&d, None, DialogKey::Enter, false),
        KeyAction::Accept
    );
}

#[test]
fn fila_5_en_licencias_con_repeat_siempre_nothing() {
    let d = licencias();
    for foco in [
        None,
        Some(FocusTarget::Zone(LICENSES_FIELD)),
        Some(FocusTarget::Accept),
    ] {
        for key in [DialogKey::Enter, DialogKey::Escape] {
            assert_eq!(key_action(&d, foco, key, true), KeyAction::Nothing);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 9 (pura): initial_focus, focus_order y next_focus
// ---------------------------------------------------------------------------------------------

fn campos_base() -> Vec<FocusTarget> {
    vec![
        FocusTarget::Field("ancho"),
        FocusTarget::Minus("ancho"),
        FocusTarget::Plus("ancho"),
        FocusTarget::Field("alto"),
        FocusTarget::Minus("alto"),
        FocusTarget::Plus("alto"),
        FocusTarget::Option("modo", 0),
        FocusTarget::Option("modo", 1),
        FocusTarget::Option("modo", 2),
    ]
}

#[test]
fn fila_9_el_orden_con_la_opcion_0_y_todo_valido() {
    let d = open(all_kinds());
    let mut esperado = campos_base();
    esperado.extend([
        FocusTarget::Field("opaco"),
        FocusTarget::Zone("informe"),
        FocusTarget::Accept,
        FocusTarget::Cancel,
    ]);
    assert_eq!(focus_order(&d), esperado);
    // Con la opción 1 es lo mismo: el hexadecimal sigue oculto.
    let mut d = open(all_kinds());
    elige(&mut d, "modo", 1);
    assert_eq!(focus_order(&d), esperado);
}

#[test]
fn fila_9_el_orden_con_la_opcion_2_incluye_el_hexadecimal_tras_las_opciones() {
    let d = valido();
    let mut esperado = campos_base();
    esperado.extend([
        FocusTarget::Field("color"),
        FocusTarget::Field("opaco"),
        FocusTarget::Zone("informe"),
        FocusTarget::Accept,
        FocusTarget::Cancel,
    ]);
    assert_eq!(focus_order(&d), esperado);
}

#[test]
fn fila_9_con_un_campo_invalido_no_hay_aceptar_pero_si_cancelar() {
    let d = invalido();
    let mut esperado = campos_base();
    esperado.extend([
        FocusTarget::Field("color"),
        FocusTarget::Field("opaco"),
        FocusTarget::Zone("informe"),
        FocusTarget::Cancel,
    ]);
    assert_eq!(focus_order(&d), esperado);
    // El campo inválido sigue siendo una parada de tabulación.
    assert!(focus_order(&d).contains(&FocusTarget::Field("ancho")));
    // Con la opción 0 también.
    let mut d = open(all_kinds());
    numero(&mut d, "alto", "x");
    assert!(!focus_order(&d).contains(&FocusTarget::Accept));
    assert_eq!(focus_order(&d).last(), Some(&FocusTarget::Cancel));
}

#[test]
fn fila_9_con_el_check_que_falla_tampoco_hay_aceptar() {
    let mut d = open(all_kinds());
    numero(&mut d, "ancho", "8192");
    numero(&mut d, "alto", "8192");
    assert!(!d.is_valid());
    let orden = focus_order(&d);
    assert!(!orden.contains(&FocusTarget::Accept));
    assert_eq!(orden.last(), Some(&FocusTarget::Cancel));
}

#[test]
fn fila_9_un_pendiente_valido_deja_aceptar_y_uno_invalido_lo_quita() {
    assert!(focus_order(&pendiente_valido()).contains(&FocusTarget::Accept));
    assert!(!focus_order(&pendiente_invalido()).contains(&FocusTarget::Accept));
    // Revertir el pendiente lo devuelve.
    let mut d = pendiente_invalido();
    d.event(Event::HexRevert { field: "color" });
    assert!(focus_order(&d).contains(&FocusTarget::Accept));
}

#[test]
fn fila_9_el_orden_de_licencias() {
    let d = licencias();
    assert_eq!(
        focus_order(&d),
        vec![FocusTarget::Zone(LICENSES_FIELD), FocusTarget::Accept]
    );
}

#[test]
fn fila_9_el_orden_de_un_formulario_vacio_y_de_uno_sin_cancelar() {
    let vacio = |cancel| FormSpec {
        title: "Vacío",
        accept: "Aceptar",
        cancel,
        fields: vec![],
        check: None,
    };
    assert_eq!(
        focus_order(&open(vacio(Some("Cancelar")))),
        vec![FocusTarget::Accept, FocusTarget::Cancel]
    );
    assert_eq!(focus_order(&open(vacio(None))), vec![FocusTarget::Accept]);
}

#[test]
fn fila_9_el_foco_inicial_es_el_primer_campo() {
    assert_eq!(
        initial_focus(&open(all_kinds())),
        FocusTarget::Field("ancho")
    );
    assert_eq!(initial_focus(&valido()), FocusTarget::Field("ancho"));
    // También con el primero inválido: sigue siendo el primer campo editable.
    assert_eq!(initial_focus(&invalido()), FocusTarget::Field("ancho"));
}

#[test]
fn fila_9_el_foco_inicial_de_licencias_es_la_zona() {
    assert_eq!(
        initial_focus(&licencias()),
        FocusTarget::Zone(LICENSES_FIELD)
    );
}

fn zona(id: &'static str) -> FieldSpec {
    FieldSpec {
        id,
        label: "Zona",
        kind: FieldKind::ReadOnly {
            text: "uno\ndos".to_string(),
            rows: Rows::Fixed(2),
        },
        shown_when: None,
    }
}

fn casilla(id: &'static str) -> FieldSpec {
    FieldSpec {
        id,
        label: "Casilla",
        kind: FieldKind::Check { default: false },
        shown_when: None,
    }
}

fn formulario(fields: Vec<FieldSpec>) -> FormSpec {
    FormSpec {
        title: "Prueba",
        accept: "Aceptar",
        cancel: Some("Cancelar"),
        fields,
        check: None,
    }
}

#[test]
fn fila_9_el_foco_inicial_salta_la_zona_y_va_al_primer_campo_editable() {
    let d = open(formulario(vec![zona("z"), casilla("c")]));
    assert_eq!(initial_focus(&d), FocusTarget::Field("c"));
    // El orden sigue el de los campos: la zona primero.
    assert_eq!(
        focus_order(&d),
        vec![
            FocusTarget::Zone("z"),
            FocusTarget::Field("c"),
            FocusTarget::Accept,
            FocusTarget::Cancel
        ]
    );
}

#[test]
fn fila_9_sin_campos_editables_el_foco_inicial_es_la_zona_y_si_no_aceptar() {
    let d = open(formulario(vec![zona("z")]));
    assert_eq!(initial_focus(&d), FocusTarget::Zone("z"));
    let d = open(formulario(vec![zona("z"), zona("y")]));
    assert_eq!(initial_focus(&d), FocusTarget::Zone("z"));
    let d = open(formulario(vec![]));
    assert_eq!(initial_focus(&d), FocusTarget::Accept);
}

#[test]
fn fila_9_el_foco_inicial_de_un_grupo_es_una_de_sus_opciones() {
    let d = open(formulario(vec![
        FieldSpec {
            id: "modo",
            label: "Modo",
            kind: FieldKind::Choice {
                options: &["A", "B"],
                default: 1,
            },
            shown_when: None,
        },
        casilla("c"),
    ]));
    let inicial = initial_focus(&d);
    assert!(
        matches!(inicial, FocusTarget::Option("modo", i) if i < 2),
        "{inicial:?}"
    );
    assert!(focus_order(&d).contains(&inicial));
}

fn orden_de_prueba() -> Vec<FocusTarget> {
    vec![
        FocusTarget::Field("a"),
        FocusTarget::Minus("a"),
        FocusTarget::Plus("a"),
        FocusTarget::Option("m", 0),
        FocusTarget::Zone("z"),
        FocusTarget::Accept,
        FocusTarget::Cancel,
    ]
}

#[test]
fn fila_9_next_focus_es_ciclico_hacia_delante() {
    let orden = orden_de_prueba();
    for (i, &f) in orden.iter().enumerate() {
        assert_eq!(
            next_focus(&orden, Some(f), false),
            Some(orden[(i + 1) % orden.len()]),
            "tras {f:?}"
        );
    }
    // Da la vuelta del último al primero.
    assert_eq!(
        next_focus(&orden, Some(FocusTarget::Cancel), false),
        Some(FocusTarget::Field("a"))
    );
}

#[test]
fn fila_9_next_focus_hacia_atras_es_el_inverso() {
    let orden = orden_de_prueba();
    for (i, &f) in orden.iter().enumerate() {
        assert_eq!(
            next_focus(&orden, Some(f), true),
            Some(orden[(i + orden.len() - 1) % orden.len()]),
            "antes de {f:?}"
        );
        let delante = next_focus(&orden, Some(f), false);
        assert_eq!(next_focus(&orden, delante, true), Some(f));
        let detras = next_focus(&orden, Some(f), true);
        assert_eq!(next_focus(&orden, detras, false), Some(f));
    }
    assert_eq!(
        next_focus(&orden, Some(FocusTarget::Field("a")), true),
        Some(FocusTarget::Cancel)
    );
}

#[test]
fn fila_9_len_pasos_vuelven_al_principio() {
    let orden = orden_de_prueba();
    for backwards in [false, true] {
        let mut actual = Some(orden[2]);
        for _ in 0..orden.len() {
            actual = next_focus(&orden, actual, backwards);
        }
        assert_eq!(actual, Some(orden[2]), "backwards {backwards}");
    }
}

#[test]
fn fila_9_un_foco_que_ya_no_existe_da_el_primero_o_el_ultimo_hacia_atras() {
    let orden = orden_de_prueba();
    for desaparecido in [
        FocusTarget::Field("color"),
        FocusTarget::Option("m", 7),
        FocusTarget::Zone("nada"),
        FocusTarget::Minus("z"),
    ] {
        assert_eq!(
            next_focus(&orden, Some(desaparecido), false),
            Some(FocusTarget::Field("a")),
            "{desaparecido:?}"
        );
        assert_eq!(
            next_focus(&orden, Some(desaparecido), true),
            Some(FocusTarget::Cancel),
            "{desaparecido:?}"
        );
    }
    // Sin foco: igual que un foco que no existe.
    assert_eq!(
        next_focus(&orden, None, false),
        Some(FocusTarget::Field("a"))
    );
    assert_eq!(next_focus(&orden, None, true), Some(FocusTarget::Cancel));
}

#[test]
fn fila_9_un_orden_vacio_da_none() {
    for current in [
        None,
        Some(FocusTarget::Accept),
        Some(FocusTarget::Field("a")),
    ] {
        for backwards in [false, true] {
            assert_eq!(next_focus(&[], current, backwards), None);
        }
    }
}

#[test]
fn fila_9_un_orden_de_uno_da_siempre_ese_elemento() {
    let orden = [FocusTarget::Accept];
    for current in [None, Some(FocusTarget::Accept), Some(FocusTarget::Cancel)] {
        for backwards in [false, true] {
            assert_eq!(
                next_focus(&orden, current, backwards),
                Some(FocusTarget::Accept)
            );
        }
    }
}

#[test]
fn fila_9_next_focus_recorre_el_orden_real_del_dialogo() {
    let d = valido();
    let orden = focus_order(&d);
    let mut actual = None;
    let mut visitados = Vec::new();
    for _ in 0..orden.len() {
        actual = next_focus(&orden, actual, false);
        visitados.push(actual.expect("hay orden"));
    }
    assert_eq!(visitados, orden);
    assert_eq!(next_focus(&orden, actual, false), Some(orden[0]));
}

// ---------------------------------------------------------------------------------------------
// Fila 9 (pura): CONTRAST_PAIRS
// ---------------------------------------------------------------------------------------------

/// Las cifras de «Lo comprobado»: primer color, segundo, contraste y mínimo.
fn cifras() -> Vec<(&'static str, egui::Color32, egui::Color32, f64, f64)> {
    vec![
        ("TEXT sobre SURFACE_1", TEXT, SURFACE_1, 11.76, 4.5),
        ("TEXT_DIM sobre SURFACE_1", TEXT_DIM, SURFACE_1, 6.15, 4.5),
        ("WARNING sobre SURFACE_1", WARNING, SURFACE_1, 6.36, 4.5),
        ("TEXT sobre SURFACE_0", TEXT, SURFACE_0, 13.18, 4.5),
        ("TEXT sobre SURFACE_2", TEXT, SURFACE_2, 10.00, 4.5),
        ("TEXT sobre SURFACE_3", TEXT, SURFACE_3, 7.61, 4.5),
        ("ACCENT sobre SURFACE_0", ACCENT, SURFACE_0, 8.98, 3.0),
        ("ACCENT sobre SURFACE_1", ACCENT, SURFACE_1, 8.01, 3.0),
        ("ACCENT sobre SURFACE_2", ACCENT, SURFACE_2, 6.81, 3.0),
        ("ACCENT sobre SURFACE_3", ACCENT, SURFACE_3, 5.19, 3.0),
    ]
}

#[test]
fn fila_9_las_cifras_de_lo_comprobado() {
    for (nombre, a, b, cifra, _) in cifras() {
        let real = contrast_ratio(a, b);
        assert!(
            (real - cifra).abs() <= 0.01,
            "{nombre}: {real:.3} y la spec dice {cifra}"
        );
    }
    // Los dos que no llegan a 4,5 sobre `SURFACE_3` (por eso no se pintan ahí).
    assert!((contrast_ratio(TEXT_DIM, SURFACE_3) - 3.98).abs() <= 0.01);
    assert!((contrast_ratio(WARNING, SURFACE_3) - 4.11).abs() <= 0.01);
    assert!(contrast_ratio(TEXT_DIM, SURFACE_3) < 4.5);
    assert!(contrast_ratio(WARNING, SURFACE_3) < 4.5);
    // El texto deshabilitado queda exento (decisión 6).
    assert!((contrast_ratio(TEXT_DISABLED, SURFACE_2) - 2.48).abs() <= 0.01);
}

#[test]
fn fila_9_cada_par_de_la_tabla_cumple_su_minimo() {
    assert!(!CONTRAST_PAIRS.is_empty());
    for &(nombre, primero, segundo, minimo) in CONTRAST_PAIRS.iter() {
        assert!(!nombre.is_empty());
        assert!(
            minimo == 4.5 || minimo == 3.0,
            "{nombre}: el mínimo es {minimo}"
        );
        if [TEXT, TEXT_DIM, WARNING].contains(&primero) {
            assert_eq!(minimo, 4.5, "{nombre}: un texto exige 4,5");
        }
        let real = contrast_ratio(primero, segundo);
        assert!(real >= minimo, "{nombre}: {real:.3} no llega a {minimo}");
    }
}

#[test]
fn fila_9_la_tabla_contiene_los_pares_de_lo_comprobado_con_sus_cifras() {
    for (nombre, a, b, cifra, minimo) in cifras() {
        let par = CONTRAST_PAIRS
            .iter()
            .find(|&&(_, primero, segundo, _)| primero == a && segundo == b)
            .unwrap_or_else(|| panic!("falta el par {nombre}"));
        assert_eq!(par.3, minimo, "{nombre}: mínimo");
        let real = contrast_ratio(par.1, par.2);
        assert!(
            (real - cifra).abs() <= 0.01,
            "{nombre}: {real:.3} frente a {cifra}"
        );
    }
}

#[test]
fn fila_9_ningun_par_menciona_el_texto_deshabilitado() {
    for &(nombre, primero, segundo, _) in CONTRAST_PAIRS.iter() {
        assert_ne!(primero, TEXT_DISABLED, "{nombre}");
        assert_ne!(segundo, TEXT_DISABLED, "{nombre}");
    }
}

#[test]
fn fila_9_ni_texto_tenue_ni_aviso_sobre_el_boton_con_el_raton() {
    // `TEXT_DIM` y `WARNING` no llegan a 4,5 sobre `SURFACE_3`: no se pintan ahí.
    for &(nombre, primero, segundo, _) in CONTRAST_PAIRS.iter() {
        assert!(
            !((primero == TEXT_DIM || primero == WARNING) && segundo == SURFACE_3),
            "{nombre}"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Propiedad 4: foco y teclado coherentes
// ---------------------------------------------------------------------------------------------

/// El orden de la decisión 7, escrito aparte a partir de la spec y del estado.
fn orden_esperado(d: &Dialog) -> Vec<FocusTarget> {
    let mut v = Vec::new();
    for f in &d.spec().fields {
        if !d.is_shown(f.id) {
            continue;
        }
        match &f.kind {
            FieldKind::Number { .. } => {
                v.push(FocusTarget::Field(f.id));
                v.push(FocusTarget::Minus(f.id));
                v.push(FocusTarget::Plus(f.id));
            }
            FieldKind::Choice { options, .. } => {
                for i in 0..options.len() {
                    v.push(FocusTarget::Option(f.id, i));
                }
            }
            FieldKind::Check { .. } | FieldKind::Hex { .. } => v.push(FocusTarget::Field(f.id)),
            FieldKind::ReadOnly { .. } => v.push(FocusTarget::Zone(f.id)),
            FieldKind::Button { .. } => {
                unreachable!("los formularios aleatorios no llevan botones")
            }
        }
    }
    if d.is_valid() {
        v.push(FocusTarget::Accept);
    }
    if d.spec().cancel.is_some() {
        v.push(FocusTarget::Cancel);
    }
    v
}

/// El foco inicial de la decisión 7, escrito aparte.
fn comprueba_inicial(d: &Dialog, inicial: FocusTarget) -> Result<(), TestCaseError> {
    let primer_editable = d
        .spec()
        .fields
        .iter()
        .find(|f| d.is_shown(f.id) && !matches!(f.kind, FieldKind::ReadOnly { .. }));
    let primera_zona = d
        .spec()
        .fields
        .iter()
        .find(|f| d.is_shown(f.id) && matches!(f.kind, FieldKind::ReadOnly { .. }));
    match (primer_editable, primera_zona) {
        (Some(f), _) => match f.kind {
            FieldKind::Choice { options, .. } => {
                prop_assert!(
                    matches!(inicial, FocusTarget::Option(id, i) if id == f.id && i < options.len()),
                    "{:?}",
                    inicial
                );
            }
            _ => prop_assert_eq!(inicial, FocusTarget::Field(f.id)),
        },
        (None, Some(z)) => prop_assert_eq!(inicial, FocusTarget::Zone(z.id)),
        (None, None) => prop_assert_eq!(inicial, FocusTarget::Accept),
    }
    Ok(())
}

fn comprueba_estado(d: &Dialog) -> Result<(), TestCaseError> {
    let orden = focus_order(d);
    // Es exactamente el orden de la spec: solo campos visibles, `Aceptar` solo si todo es válido.
    prop_assert_eq!(&orden, &orden_esperado(d));
    // Sin repeticiones.
    for (i, a) in orden.iter().enumerate() {
        for b in &orden[i + 1..] {
            prop_assert!(a != b, "{:?} repetido", a);
        }
    }
    prop_assert_eq!(orden.contains(&FocusTarget::Accept), d.is_valid());
    prop_assert_eq!(
        orden.contains(&FocusTarget::Cancel),
        d.spec().cancel.is_some()
    );

    // `next_focus`: cíclico y su inverso.
    if orden.is_empty() {
        for backwards in [false, true] {
            prop_assert_eq!(next_focus(&orden, None, backwards), None);
        }
    } else {
        for backwards in [false, true] {
            let mut actual = Some(orden[0]);
            for _ in 0..orden.len() {
                actual = next_focus(&orden, actual, backwards);
            }
            prop_assert_eq!(actual, Some(orden[0]));
        }
        for &f in &orden {
            let delante = next_focus(&orden, Some(f), false);
            prop_assert!(delante.is_some());
            prop_assert_eq!(next_focus(&orden, delante, true), Some(f));
            let detras = next_focus(&orden, Some(f), true);
            prop_assert_eq!(next_focus(&orden, detras, false), Some(f));
        }
    }

    // `initial_focus`: en el orden, o un botón si no hay otra cosa.
    let inicial = initial_focus(d);
    prop_assert!(
        orden.contains(&inicial) || matches!(inicial, FocusTarget::Accept | FocusTarget::Cancel),
        "{:?} no está en el orden",
        inicial
    );
    comprueba_inicial(d, inicial)?;

    // `key_action` es total y coherente.
    let mut focos: Vec<Option<FocusTarget>> = vec![None];
    focos.extend(orden.iter().copied().map(Some));
    for foco in focos {
        for repeat in [false, true] {
            let escape = key_action(d, foco, DialogKey::Escape, repeat);
            let intro = key_action(d, foco, DialogKey::Enter, repeat);
            prop_assert_ne!(escape, KeyAction::Accept, "Escape nunca acepta");
            if !d.is_valid() {
                prop_assert_ne!(
                    intro,
                    KeyAction::Accept,
                    "Intro no acepta con algo inválido"
                );
            }
            if repeat {
                prop_assert_eq!(escape, KeyAction::Nothing);
                prop_assert_eq!(intro, KeyAction::Nothing);
            }
            prop_assert_eq!(escape, oraculo(d, foco, DialogKey::Escape, repeat));
            prop_assert_eq!(intro, oraculo(d, foco, DialogKey::Enter, repeat));
        }
    }
    Ok(())
}

fn ejecuta(
    recipe: &FormRecipe,
    events: &[dialog_random::EventRecipe],
) -> Result<(), TestCaseError> {
    let mut d = Dialog::new(DialogKind::Licenses, build(recipe)).expect("spec válida");
    comprueba_estado(&d)?;
    for r in events {
        let Some(evento) = event_of(r) else {
            continue;
        };
        // `Accept` y `Cancel` no cambian un diálogo (que no es un `Modal`): se sigue con él.
        let _ = d.event(evento);
        comprueba_estado(&d)?;
    }
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn propiedad_4_foco_y_teclado_coherentes(
        recipe in form_recipe(),
        events in prop::collection::vec(event_recipe(), 0..=32),
    ) {
        ejecuta(&recipe, &events)?;
    }
}
