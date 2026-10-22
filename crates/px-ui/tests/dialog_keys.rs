//! Tests del teclado y el foco de los diálogos `px_ui::dialog_keys` (specs/hito-2-dialogos-
//! modales.md, unidad 18, decisiones 5, 7 y 8, «Casos» filas 5 y 9 —parte pura— y propiedad 4).
//! Sin ventana ni banco de egui: **la parte del banco de la fila 9** (cada texto y cada marca
//! pintados frente a su fondo) es de `dialog_view.rs`. **En rojo** hasta que exista el módulo.
//!
//! - Fila 5: `key_action` con cada foco (`Field` numérico, hexadecimal con y sin pendiente,
//!   `Option`, casilla, `Minus`, `Plus`, `Zone`, `Accept`, `Cancel` y sin foco) × `Intro` y
//!   `Escape` × válido e inválido × `repeat`, contra una tabla escrita a mano y contra un oráculo
//!   aparte; un diálogo sin `Cancel` (`licenses_form`); `dialog_key` con y sin modificadores.
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
//!   límite: la spec los llama «paradas de tabulación» sin excepción y solo exceptúa `Accept`.
//! - `Intro` con un foco que no existe o con un hexadecimal oculto no se prueba (la spec no lo
//!   dice); tampoco `Cancel` como foco en un diálogo sin `Cancel`.
//! - `Accept` queda fuera del orden con el `check` que falla aunque todos los campos sean
//!   válidos (el conjunto es inválido).
//! - Un formulario vacío tiene como foco inicial `Accept`; su orden es `Accept` (y `Cancel`
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
use common::modifiers_2;
#[path = "common/dialog_forms.rs"]
mod dialog_forms;
#[path = "common/dialog_random.rs"]
mod dialog_random;

use dialog_forms::{all_kinds, open};
use dialog_random::{FormRecipe, build, event_of, event_recipe, form_recipe};

// ---------------------------------------------------------------------------------------------
// Ayudas
// ---------------------------------------------------------------------------------------------

fn number(d: &mut Dialog, field: &'static str, text: &str) {
    assert_eq!(
        d.event(Event::Number {
            field,
            text: text.to_string()
        }),
        Reaction::None
    );
}

fn chooses(d: &mut Dialog, field: &'static str, index: usize) {
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
fn valid() -> Dialog {
    let mut d = open(all_kinds());
    chooses(&mut d, "modo", 2);
    assert!(d.is_valid());
    d
}

/// Un numérico vacío: el conjunto es inválido.
fn invalid() -> Dialog {
    let mut d = valid();
    number(&mut d, "ancho", "");
    assert!(!d.is_valid());
    d
}

/// Un pendiente válido en el hexadecimal (`#abc`): el conjunto sigue siendo válido.
fn pending_valid() -> Dialog {
    let mut d = valid();
    hex(&mut d, "#abc");
    assert!(d.hex_pending("color"));
    assert!(d.is_valid());
    d
}

/// Un pendiente inválido (`zz`): el conjunto es inválido.
fn pending_invalid() -> Dialog {
    let mut d = valid();
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

/// Los focos que hay en `valid()`: uno de cada clase y ninguno.
fn foci() -> Vec<(&'static str, Option<FocusTarget>)> {
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
fn oracle(d: &Dialog, focus: Option<FocusTarget>, key: DialogKey, repeat: bool) -> KeyAction {
    if repeat {
        return KeyAction::Nothing;
    }
    let hex_pending = match focus {
        Some(FocusTarget::Field(id)) if es_hex(d, id) && d.hex_pending(id) => Some(id),
        _ => None,
    };
    match key {
        DialogKey::Escape => match hex_pending {
            Some(id) => KeyAction::RevertHex(id),
            None => KeyAction::Cancel,
        },
        DialogKey::Enter => match focus {
            Some(FocusTarget::Field(id)) if hex_pending == Some(id) => KeyAction::ConfirmHex(id),
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
fn row_5_dialog_key_recognizes_enter_and_escape_without_modifiers() {
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
fn row_5_dialog_key_with_any_modifier_gives_none() {
    for bits in 1u8..32 {
        let m = modifiers_2(bits);
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
    let shift_2 = Modifiers {
        shift: true,
        ..Modifiers::NONE
    };
    for m in [ctrl, alt, shift_2, Modifiers::COMMAND, Modifiers::ALT] {
        assert_eq!(dialog_key(Key::Enter, m), None);
        assert_eq!(dialog_key(Key::Escape, m), None);
    }
}

#[test]
fn row_5_dialog_key_only_recognizes_those_two_keys() {
    for &key in Key::ALL {
        let expected_2 = match key {
            Key::Enter => Some(DialogKey::Enter),
            Key::Escape => Some(DialogKey::Escape),
            _ => None,
        };
        assert_eq!(dialog_key(key, Modifiers::NONE), expected_2, "{key:?}");
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 5: key_action
// ---------------------------------------------------------------------------------------------

#[test]
fn row_5_escape_cancels_with_any_focus_if_there_is_no_pending_one() {
    for dialog_2 in [valid(), invalid()] {
        for (name_2, focus) in foci() {
            assert_eq!(
                key_action(&dialog_2, focus, DialogKey::Escape, false),
                KeyAction::Cancel,
                "foco {name_2}"
            );
        }
    }
}

#[test]
fn row_5_escape_in_a_hexadecimal_with_a_pending_one_reverts_and_the_next_one_cancels() {
    for mut dialog_2 in [pending_valid(), pending_invalid()] {
        let focus = Some(FocusTarget::Field("color"));
        assert_eq!(
            key_action(&dialog_2, focus, DialogKey::Escape, false),
            KeyAction::RevertHex("color")
        );
        // El campo se revierte (lo hace quien recibe `RevertHex`) y el siguiente `Escape` cancela.
        assert_eq!(
            dialog_2.event(Event::HexRevert { field: "color" }),
            Reaction::None
        );
        assert!(!dialog_2.hex_pending("color"));
        assert_eq!(
            key_action(&dialog_2, focus, DialogKey::Escape, false),
            KeyAction::Cancel
        );
    }
}

#[test]
fn row_5_escape_with_a_pending_one_but_the_focus_elsewhere_cancels() {
    for dialog_2 in [pending_valid(), pending_invalid()] {
        for (name_2, focus) in foci() {
            if focus == Some(FocusTarget::Field("color")) {
                continue;
            }
            assert_eq!(
                key_action(&dialog_2, focus, DialogKey::Escape, false),
                KeyAction::Cancel,
                "foco {name_2}"
            );
        }
    }
}

#[test]
fn row_5_enter_in_a_hexadecimal_with_a_pending_one_confirms_the_field_valid_or_not() {
    for dialog_2 in [pending_valid(), pending_invalid()] {
        assert_eq!(
            key_action(
                &dialog_2,
                Some(FocusTarget::Field("color")),
                DialogKey::Enter,
                false
            ),
            KeyAction::ConfirmHex("color")
        );
    }
}

#[test]
fn row_5_enter_accepts_only_if_everything_is_valid_in_field_option_box_hex_without_a_pending_one_and_without_focus()
 {
    let with_accept = [
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
    let good = valid();
    let bad = invalid();
    for (name_2, focus) in with_accept {
        assert_eq!(
            key_action(&good, focus, DialogKey::Enter, false),
            KeyAction::Accept,
            "válido, foco {name_2}"
        );
        assert_eq!(
            key_action(&bad, focus, DialogKey::Enter, false),
            KeyAction::Nothing,
            "inválido, foco {name_2}"
        );
    }
    // Con un pendiente válido en otro campo el conjunto es válido: acepta desde el numérico.
    assert_eq!(
        key_action(
            &pending_valid(),
            Some(FocusTarget::Field("ancho")),
            DialogKey::Enter,
            false
        ),
        KeyAction::Accept
    );
    // Con un pendiente inválido en otro campo, el conjunto no lo es.
    assert_eq!(
        key_action(
            &pending_invalid(),
            Some(FocusTarget::Field("ancho")),
            DialogKey::Enter,
            false
        ),
        KeyAction::Nothing
    );
}

#[test]
fn row_5_enter_does_nothing_in_the_area_and_on_the_four_buttons() {
    let buttons = [
        Some(FocusTarget::Zone("informe")),
        Some(FocusTarget::Minus("ancho")),
        Some(FocusTarget::Plus("ancho")),
        Some(FocusTarget::Minus("alto")),
        Some(FocusTarget::Plus("alto")),
        Some(FocusTarget::Accept),
        Some(FocusTarget::Cancel),
    ];
    for dialog_2 in [valid(), invalid(), pending_valid(), pending_invalid()] {
        for focus in buttons {
            assert_eq!(
                key_action(&dialog_2, focus, DialogKey::Enter, false),
                KeyAction::Nothing,
                "{focus:?}"
            );
        }
    }
}

#[test]
fn row_5_with_repeat_always_nothing() {
    for dialog_2 in [valid(), invalid(), pending_valid(), pending_invalid()] {
        for (name_2, focus) in foci() {
            for key in [DialogKey::Enter, DialogKey::Escape] {
                assert_eq!(
                    key_action(&dialog_2, focus, key, true),
                    KeyAction::Nothing,
                    "foco {name_2}, {key:?}"
                );
            }
        }
    }
}

#[test]
fn row_5_the_complete_table_matches_the_oracle() {
    let states: [(&str, Dialog); 4] = [
        ("válido", valid()),
        ("inválido", invalid()),
        ("pendiente válido", pending_valid()),
        ("pendiente inválido", pending_invalid()),
    ];
    for (state_2, dialog_2) in &states {
        for (name_2, focus) in foci() {
            for key in [DialogKey::Enter, DialogKey::Escape] {
                for repeat in [false, true] {
                    assert_eq!(
                        key_action(dialog_2, focus, key, repeat),
                        oracle(dialog_2, focus, key, repeat),
                        "{state_2}, foco {name_2}, {key:?}, repeat {repeat}"
                    );
                }
            }
        }
    }
}

#[test]
fn row_5_the_area_and_the_buttons_are_not_a_hexadecimal_even_if_there_is_a_pending_one() {
    // Escape en un botón con un pendiente en el hexadecimal: cancela, no revierte.
    let d = pending_invalid();
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

fn licenses() -> Dialog {
    open(licenses_form("x"))
}

#[test]
fn row_5_a_dialog_without_cancel_closes_with_escape() {
    let d = licenses();
    assert_eq!(d.spec().cancel, None);
    for focus in [
        None,
        Some(FocusTarget::Zone(LICENSES_FIELD)),
        Some(FocusTarget::Accept),
    ] {
        assert_eq!(
            key_action(&d, focus, DialogKey::Escape, false),
            KeyAction::Cancel,
            "{focus:?}"
        );
    }
}

#[test]
fn row_5_in_licenses_enter_in_the_area_and_on_the_button_does_nothing_and_without_focus_it_accepts()
{
    let d = licenses();
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
fn row_5_in_licenses_with_repeat_always_nothing() {
    let d = licenses();
    for focus in [
        None,
        Some(FocusTarget::Zone(LICENSES_FIELD)),
        Some(FocusTarget::Accept),
    ] {
        for key in [DialogKey::Enter, DialogKey::Escape] {
            assert_eq!(key_action(&d, focus, key, true), KeyAction::Nothing);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 9 (pura): initial_focus, focus_order y next_focus
// ---------------------------------------------------------------------------------------------

fn fields_base() -> Vec<FocusTarget> {
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
fn row_9_the_order_with_option_0_and_everything_valid() {
    let d = open(all_kinds());
    let mut expected_2 = fields_base();
    expected_2.extend([
        FocusTarget::Field("opaco"),
        FocusTarget::Zone("informe"),
        FocusTarget::Accept,
        FocusTarget::Cancel,
    ]);
    assert_eq!(focus_order(&d), expected_2);
    // Con la opción 1 es lo mismo: el hexadecimal sigue oculto.
    let mut d = open(all_kinds());
    chooses(&mut d, "modo", 1);
    assert_eq!(focus_order(&d), expected_2);
}

#[test]
fn row_9_the_order_with_option_2_includes_the_hexadecimal_after_the_options() {
    let d = valid();
    let mut expected_2 = fields_base();
    expected_2.extend([
        FocusTarget::Field("color"),
        FocusTarget::Field("opaco"),
        FocusTarget::Zone("informe"),
        FocusTarget::Accept,
        FocusTarget::Cancel,
    ]);
    assert_eq!(focus_order(&d), expected_2);
}

#[test]
fn row_9_with_an_invalid_field_there_is_no_accept_but_there_is_cancel() {
    let d = invalid();
    let mut expected_2 = fields_base();
    expected_2.extend([
        FocusTarget::Field("color"),
        FocusTarget::Field("opaco"),
        FocusTarget::Zone("informe"),
        FocusTarget::Cancel,
    ]);
    assert_eq!(focus_order(&d), expected_2);
    // El campo inválido sigue siendo una parada de tabulación.
    assert!(focus_order(&d).contains(&FocusTarget::Field("ancho")));
    // Con la opción 0 también.
    let mut d = open(all_kinds());
    number(&mut d, "alto", "x");
    assert!(!focus_order(&d).contains(&FocusTarget::Accept));
    assert_eq!(focus_order(&d).last(), Some(&FocusTarget::Cancel));
}

#[test]
fn row_9_with_the_failing_check_there_is_no_accept_either() {
    let mut d = open(all_kinds());
    number(&mut d, "ancho", "8192");
    number(&mut d, "alto", "8192");
    assert!(!d.is_valid());
    let order = focus_order(&d);
    assert!(!order.contains(&FocusTarget::Accept));
    assert_eq!(order.last(), Some(&FocusTarget::Cancel));
}

#[test]
fn row_9_a_valid_pending_one_lets_accept_through_and_an_invalid_one_removes_it() {
    assert!(focus_order(&pending_valid()).contains(&FocusTarget::Accept));
    assert!(!focus_order(&pending_invalid()).contains(&FocusTarget::Accept));
    // Revertir el pendiente lo devuelve.
    let mut d = pending_invalid();
    d.event(Event::HexRevert { field: "color" });
    assert!(focus_order(&d).contains(&FocusTarget::Accept));
}

#[test]
fn row_9_the_order_of_licenses() {
    let d = licenses();
    assert_eq!(
        focus_order(&d),
        vec![FocusTarget::Zone(LICENSES_FIELD), FocusTarget::Accept]
    );
}

#[test]
fn row_9_the_order_of_an_empty_form_and_of_one_without_cancel() {
    let empty = |cancel| FormSpec {
        title: "Vacío",
        accept: "Aceptar",
        cancel,
        fields: vec![],
        check: None,
    };
    assert_eq!(
        focus_order(&open(empty(Some("Cancelar")))),
        vec![FocusTarget::Accept, FocusTarget::Cancel]
    );
    assert_eq!(focus_order(&open(empty(None))), vec![FocusTarget::Accept]);
}

#[test]
fn row_9_the_initial_focus_is_the_first_field() {
    assert_eq!(
        initial_focus(&open(all_kinds())),
        FocusTarget::Field("ancho")
    );
    assert_eq!(initial_focus(&valid()), FocusTarget::Field("ancho"));
    // También con el primero inválido: sigue siendo el primer campo editable.
    assert_eq!(initial_focus(&invalid()), FocusTarget::Field("ancho"));
}

#[test]
fn row_9_the_initial_focus_of_licenses_is_the_area() {
    assert_eq!(
        initial_focus(&licenses()),
        FocusTarget::Zone(LICENSES_FIELD)
    );
}

fn zone(id: &'static str) -> FieldSpec {
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

fn box_value(id: &'static str) -> FieldSpec {
    FieldSpec {
        id,
        label: "Casilla",
        kind: FieldKind::Check { default: false },
        shown_when: None,
    }
}

fn form(fields: Vec<FieldSpec>) -> FormSpec {
    FormSpec {
        title: "Prueba",
        accept: "Aceptar",
        cancel: Some("Cancelar"),
        fields,
        check: None,
    }
}

#[test]
fn row_9_the_initial_focus_skips_the_area_and_goes_to_the_first_editable_field() {
    let d = open(form(vec![zone("z"), box_value("c")]));
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
fn row_9_without_editable_fields_the_initial_focus_is_the_area_and_if_not_accept() {
    let d = open(form(vec![zone("z")]));
    assert_eq!(initial_focus(&d), FocusTarget::Zone("z"));
    let d = open(form(vec![zone("z"), zone("y")]));
    assert_eq!(initial_focus(&d), FocusTarget::Zone("z"));
    let d = open(form(vec![]));
    assert_eq!(initial_focus(&d), FocusTarget::Accept);
}

#[test]
fn row_9_the_initial_focus_of_a_group_is_one_of_its_options() {
    let d = open(form(vec![
        FieldSpec {
            id: "modo",
            label: "Modo",
            kind: FieldKind::Choice {
                options: &["A", "B"],
                default: 1,
            },
            shown_when: None,
        },
        box_value("c"),
    ]));
    let initial = initial_focus(&d);
    assert!(
        matches!(initial, FocusTarget::Option("modo", i) if i < 2),
        "{initial:?}"
    );
    assert!(focus_order(&d).contains(&initial));
}

fn order_of_test() -> Vec<FocusTarget> {
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
fn row_9_next_focus_is_cyclic_forward() {
    let order = order_of_test();
    for (i, &f) in order.iter().enumerate() {
        assert_eq!(
            next_focus(&order, Some(f), false),
            Some(order[(i + 1) % order.len()]),
            "tras {f:?}"
        );
    }
    // Da la vuelta del último al primero.
    assert_eq!(
        next_focus(&order, Some(FocusTarget::Cancel), false),
        Some(FocusTarget::Field("a"))
    );
}

#[test]
fn row_9_next_focus_backwards_is_the_inverse() {
    let order = order_of_test();
    for (i, &f) in order.iter().enumerate() {
        assert_eq!(
            next_focus(&order, Some(f), true),
            Some(order[(i + order.len() - 1) % order.len()]),
            "antes de {f:?}"
        );
        let front = next_focus(&order, Some(f), false);
        assert_eq!(next_focus(&order, front, true), Some(f));
        let behind = next_focus(&order, Some(f), true);
        assert_eq!(next_focus(&order, behind, false), Some(f));
    }
    assert_eq!(
        next_focus(&order, Some(FocusTarget::Field("a")), true),
        Some(FocusTarget::Cancel)
    );
}

#[test]
fn row_9_len_steps_go_back_to_the_start() {
    let order = order_of_test();
    for backwards in [false, true] {
        let mut actual = Some(order[2]);
        for _ in 0..order.len() {
            actual = next_focus(&order, actual, backwards);
        }
        assert_eq!(actual, Some(order[2]), "backwards {backwards}");
    }
}

#[test]
fn row_9_a_focus_that_no_longer_exists_gives_the_first_or_the_last_backwards() {
    let order = order_of_test();
    for disappeared in [
        FocusTarget::Field("color"),
        FocusTarget::Option("m", 7),
        FocusTarget::Zone("nada"),
        FocusTarget::Minus("z"),
    ] {
        assert_eq!(
            next_focus(&order, Some(disappeared), false),
            Some(FocusTarget::Field("a")),
            "{disappeared:?}"
        );
        assert_eq!(
            next_focus(&order, Some(disappeared), true),
            Some(FocusTarget::Cancel),
            "{disappeared:?}"
        );
    }
    // Sin foco: igual que un foco que no existe.
    assert_eq!(
        next_focus(&order, None, false),
        Some(FocusTarget::Field("a"))
    );
    assert_eq!(next_focus(&order, None, true), Some(FocusTarget::Cancel));
}

#[test]
fn row_9_an_empty_order_gives_none() {
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
fn row_9_an_order_of_one_always_gives_that_element() {
    let order = [FocusTarget::Accept];
    for current in [None, Some(FocusTarget::Accept), Some(FocusTarget::Cancel)] {
        for backwards in [false, true] {
            assert_eq!(
                next_focus(&order, current, backwards),
                Some(FocusTarget::Accept)
            );
        }
    }
}

#[test]
fn row_9_next_focus_walks_the_real_order_of_the_dialog() {
    let d = valid();
    let order = focus_order(&d);
    let mut actual = None;
    let mut visited = Vec::new();
    for _ in 0..order.len() {
        actual = next_focus(&order, actual, false);
        visited.push(actual.expect("hay orden"));
    }
    assert_eq!(visited, order);
    assert_eq!(next_focus(&order, actual, false), Some(order[0]));
}

// ---------------------------------------------------------------------------------------------
// Fila 9 (pura): CONTRAST_PAIRS
// ---------------------------------------------------------------------------------------------

/// Las cifras de «Lo comprobado»: primer color, segundo, contraste y mínimo.
fn figures() -> Vec<(&'static str, egui::Color32, egui::Color32, f64, f64)> {
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
fn row_9_the_figures_of_what_was_checked() {
    for (name_2, a, b, figure, _) in figures() {
        let real = contrast_ratio(a, b);
        assert!(
            (real - figure).abs() <= 0.01,
            "{name_2}: {real:.3} y la spec dice {figure}"
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
fn row_9_each_pair_of_the_table_meets_its_minimum() {
    assert!(!CONTRAST_PAIRS.is_empty());
    for &(name_2, first, second, minimum_2) in CONTRAST_PAIRS.iter() {
        assert!(!name_2.is_empty());
        assert!(
            minimum_2 == 4.5 || minimum_2 == 3.0,
            "{name_2}: el mínimo es {minimum_2}"
        );
        if [TEXT, TEXT_DIM, WARNING].contains(&first) {
            assert_eq!(minimum_2, 4.5, "{name_2}: un texto exige 4,5");
        }
        let real = contrast_ratio(first, second);
        assert!(
            real >= minimum_2,
            "{name_2}: {real:.3} no llega a {minimum_2}"
        );
    }
}

#[test]
fn row_9_the_table_contains_the_pairs_of_what_was_checked_with_their_figures() {
    for (name_2, a, b, figure, minimum_2) in figures() {
        let par = CONTRAST_PAIRS
            .iter()
            .find(|&&(_, first, second, _)| first == a && second == b)
            .unwrap_or_else(|| panic!("falta el par {name_2}"));
        assert_eq!(par.3, minimum_2, "{name_2}: mínimo");
        let real = contrast_ratio(par.1, par.2);
        assert!(
            (real - figure).abs() <= 0.01,
            "{name_2}: {real:.3} frente a {figure}"
        );
    }
}

#[test]
fn row_9_no_pair_mentions_the_disabled_text() {
    for &(name_2, first, second, _) in CONTRAST_PAIRS.iter() {
        assert_ne!(first, TEXT_DISABLED, "{name_2}");
        assert_ne!(second, TEXT_DISABLED, "{name_2}");
    }
}

#[test]
fn row_9_neither_faint_text_nor_a_notice_over_the_button_with_the_mouse() {
    // `TEXT_DIM` y `WARNING` no llegan a 4,5 sobre `SURFACE_3`: no se pintan ahí.
    for &(name_2, first, second, _) in CONTRAST_PAIRS.iter() {
        assert!(
            !((first == TEXT_DIM || first == WARNING) && second == SURFACE_3),
            "{name_2}"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Propiedad 4: foco y teclado coherentes
// ---------------------------------------------------------------------------------------------

/// El orden de la decisión 7, escrito aparte a partir de la spec y del estado.
fn order_expected(d: &Dialog) -> Vec<FocusTarget> {
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
fn checks_initial(d: &Dialog, initial: FocusTarget) -> Result<(), TestCaseError> {
    let first_editable = d
        .spec()
        .fields
        .iter()
        .find(|f| d.is_shown(f.id) && !matches!(f.kind, FieldKind::ReadOnly { .. }));
    let first_zone = d
        .spec()
        .fields
        .iter()
        .find(|f| d.is_shown(f.id) && matches!(f.kind, FieldKind::ReadOnly { .. }));
    match (first_editable, first_zone) {
        (Some(f), _) => match f.kind {
            FieldKind::Choice { options, .. } => {
                prop_assert!(
                    matches!(initial, FocusTarget::Option(id, i) if id == f.id && i < options.len()),
                    "{:?}",
                    initial
                );
            }
            _ => prop_assert_eq!(initial, FocusTarget::Field(f.id)),
        },
        (None, Some(z)) => prop_assert_eq!(initial, FocusTarget::Zone(z.id)),
        (None, None) => prop_assert_eq!(initial, FocusTarget::Accept),
    }
    Ok(())
}

fn checks_state(d: &Dialog) -> Result<(), TestCaseError> {
    let order = focus_order(d);
    // Es exactamente el orden de la spec: solo campos visibles, `Accept` solo si todo es válido.
    prop_assert_eq!(&order, &order_expected(d));
    // Sin repeticiones.
    for (i, a) in order.iter().enumerate() {
        for b in &order[i + 1..] {
            prop_assert!(a != b, "{:?} repetido", a);
        }
    }
    prop_assert_eq!(order.contains(&FocusTarget::Accept), d.is_valid());
    prop_assert_eq!(
        order.contains(&FocusTarget::Cancel),
        d.spec().cancel.is_some()
    );

    // `next_focus`: cíclico y su inverso.
    if order.is_empty() {
        for backwards in [false, true] {
            prop_assert_eq!(next_focus(&order, None, backwards), None);
        }
    } else {
        for backwards in [false, true] {
            let mut actual = Some(order[0]);
            for _ in 0..order.len() {
                actual = next_focus(&order, actual, backwards);
            }
            prop_assert_eq!(actual, Some(order[0]));
        }
        for &f in &order {
            let front = next_focus(&order, Some(f), false);
            prop_assert!(front.is_some());
            prop_assert_eq!(next_focus(&order, front, true), Some(f));
            let behind = next_focus(&order, Some(f), true);
            prop_assert_eq!(next_focus(&order, behind, false), Some(f));
        }
    }

    // `initial_focus`: en el orden, o un botón si no hay otra cosa.
    let initial = initial_focus(d);
    prop_assert!(
        order.contains(&initial) || matches!(initial, FocusTarget::Accept | FocusTarget::Cancel),
        "{:?} no está en el orden",
        initial
    );
    checks_initial(d, initial)?;

    // `key_action` es total y coherente.
    let mut foci: Vec<Option<FocusTarget>> = vec![None];
    foci.extend(order.iter().copied().map(Some));
    for focus in foci {
        for repeat in [false, true] {
            let escape = key_action(d, focus, DialogKey::Escape, repeat);
            let intro = key_action(d, focus, DialogKey::Enter, repeat);
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
            prop_assert_eq!(escape, oracle(d, focus, DialogKey::Escape, repeat));
            prop_assert_eq!(intro, oracle(d, focus, DialogKey::Enter, repeat));
        }
    }
    Ok(())
}

fn runs(recipe: &FormRecipe, events: &[dialog_random::EventRecipe]) -> Result<(), TestCaseError> {
    let mut d = Dialog::new(DialogKind::Licenses, build(recipe)).expect("spec válida");
    checks_state(&d)?;
    for r in events {
        let Some(event) = event_of(r) else {
            continue;
        };
        // `Accept` y `Cancel` no cambian un diálogo (que no es un `Modal`): se sigue con él.
        let _ = d.event(event);
        checks_state(&d)?;
    }
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn property_4_focus_and_keyboard_consistent(
        recipe in form_recipe(),
        events in prop::collection::vec(event_recipe(), 0..=32),
    ) {
        runs(&recipe, &events)?;
    }
}
