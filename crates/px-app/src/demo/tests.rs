//! Pruebas de la fila 12 de specs/hito-2-dialogos-modales.md que tocan a `px-app`: el diálogo de
//! prueba (`PIXOKO_TEST_DIALOG=1`), es decir `demo_form() -> FormSpec` y
//! `demo_notice(&Reaction) -> String`. En ROJO hasta la implementación: ninguna de las dos existe.
//!
//! - `demo_form`: `validate()` correcto y los cinco tipos de campo (dos numéricos, opciones —cuatro
//!   en total—, una casilla, un hexadecimal con `shown_when` sobre un grupo de opciones y un informe
//!   con `Rows::Fixed`), con `Cancel` (el aviso dice «cancelado», así que tiene que poder
//!   cancelarse); abre con `Dialog::new`, es válido por defecto y el hexadecimal solo se muestra con
//!   su opción. Los rangos de la prueba manual (`1-8192` y hasta `4294967295`) van en su propia
//!   prueba, derivada del texto de «Prueba manual», por si la implementación elige otros.
//! - `demo_notice`: función pura, **sin sesión** (la firma lo impide), con textos en
//!   `INTERFACE_CHARSET`, de una sola línea y no vacíos.
//!
//! Lo que la spec no fija y aquí se espera de `demo_notice` (anotado para quien lo implemente):
//! - `Cancelled`: contiene «cancelado» (minúsculas) y no «aceptado».
//! - `Accepted(values)`: contiene «aceptado» y, de cada campo **visible** con valor, su rótulo y su
//!   valor: el número en decimal, el texto de la opción elegida, `sí` o `no` para la casilla y el
//!   color con `hex_text`. Un formato natural es `accepted: Width 64, Height 64, …`. Los campos ocultos
//!   (`Values` no los trae) no salen: con el hexadecimal oculto no sale el texto de su valor por
//!   defecto.
//! - `Rejected(reason_2)`: contiene el motivo.
//! - `None`: un texto no vacío cualquiera (la barra no debe quedar con un texto engañoso: no dice
//!   «aceptado» ni «cancelado»).
//! - Todo texto: sin saltos de línea, sin espacios al principio ni al final y en
//!   `INTERFACE_CHARSET`.
//!
//! Suposiciones sobre las firmas de «Piezas»: `Reaction` y `Values`: `Clone + Debug + PartialEq`.

use super::*;

use px_ui::color_edit::hex_text;
use px_ui::dialog::{
    Dialog, DialogKind, Event, FieldKind, FieldSpec, FormSpec, Reaction, Rows, Values,
};
use px_ui::text::INTERFACE_CHARSET;

/// El `FormSpec` por su firma exacta (que `demo_notice` no recibe sesión lo fija la firma).
const _FORM: fn() -> FormSpec = demo_form;
const _NOTICE: fn(&Reaction) -> String = demo_notice;

fn opens() -> Dialog {
    Dialog::new(DialogKind::Licenses, demo_form()).expect("la spec de prueba es válida")
}

fn fields_of<'a>(
    spec: &'a FormSpec,
    wants: impl Fn(&FieldKind) -> bool + 'a,
) -> Vec<&'a FieldSpec> {
    spec.fields.iter().filter(|f| wants(&f.kind)).collect()
}

fn numeric(spec: &FormSpec) -> Vec<&FieldSpec> {
    fields_of(spec, |k| matches!(k, FieldKind::Number { .. }))
}

fn groups(spec: &FormSpec) -> Vec<&FieldSpec> {
    fields_of(spec, |k| matches!(k, FieldKind::Choice { .. }))
}

fn boxes_2(spec: &FormSpec) -> Vec<&FieldSpec> {
    fields_of(spec, |k| matches!(k, FieldKind::Check { .. }))
}

fn hex_2(spec: &FormSpec) -> Vec<&FieldSpec> {
    fields_of(spec, |k| matches!(k, FieldKind::Hex { .. }))
}

fn reports(spec: &FormSpec) -> Vec<&FieldSpec> {
    fields_of(spec, |k| matches!(k, FieldKind::ReadOnly { .. }))
}

fn options_2(field_2: &FieldSpec) -> &'static [&'static str] {
    match &field_2.kind {
        FieldKind::Choice { options, .. } => options,
        other_2 => panic!("{:?} no es un grupo de opciones: {other_2:?}", field_2.id),
    }
}

/// Lo aceptado con el diálogo en el estado actual: `Accepted(values)` de verdad.
fn accepted(dialog: &mut Dialog) -> Reaction {
    let reaction = dialog.event(Event::Accept);
    assert!(
        matches!(reaction, Reaction::Accepted(_)),
        "el diálogo de prueba es aceptable: {reaction:?}"
    );
    reaction
}

fn checks_text_valid(text_2: &str, context: &str) {
    assert!(!text_2.is_empty(), "{context}: texto vacío");
    assert_eq!(
        text_2.trim(),
        text_2,
        "{context}: espacios sobrantes: {text_2:?}"
    );
    assert!(
        !text_2.contains('\n') && !text_2.contains('\r'),
        "{context}: {text_2:?}"
    );
    for c in text_2.chars() {
        assert!(
            INTERFACE_CHARSET.contains(c),
            "{context}: {c:?} no está en INTERFACE_CHARSET: {text_2:?}"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// demo_form
// ---------------------------------------------------------------------------------------------

#[test]
fn row_12_demo_form_is_valid() {
    let spec = demo_form();
    assert!(spec.validate().is_ok());
    // Y la acepta un `Dialog` (la única `DialogKind` hoy es `Licenses`).
    assert!(Dialog::new(DialogKind::Licenses, demo_form()).is_ok());
    // Siempre da la misma spec (función pura de nada).
    let other = demo_form();
    assert_eq!(spec.title, other.title);
    assert_eq!(spec.accept, other.accept);
    assert_eq!(spec.cancel, other.cancel);
    assert_eq!(spec.fields.len(), other.fields.len());
}

#[test]
fn row_12_demo_form_contains_the_five_field_types() {
    let spec = demo_form();
    assert_eq!(numeric(&spec).len(), 2, "dos numéricos");
    let groups = groups(&spec);
    assert!(!groups.is_empty(), "al menos un grupo de opciones");
    let total_options: usize = groups.iter().map(|g| options_2(g).len()).sum();
    assert_eq!(total_options, 4, "cuatro opciones en total");
    assert_eq!(boxes_2(&spec).len(), 1, "una casilla");
    assert_eq!(hex_2(&spec).len(), 1, "un hexadecimal");
    let reports = reports(&spec);
    assert_eq!(reports.len(), 1, "un informe");
    assert!(
        matches!(
            reports[0].kind,
            FieldKind::ReadOnly {
                rows: Rows::Fixed(_),
                ..
            }
        ),
        "el informe es de filas fijas (con `Fill` no puede haber otros campos visibles)"
    );
}

#[test]
fn row_12_demo_form_has_cancel_and_labels_in_the_character_set() {
    let spec = demo_form();
    assert!(spec.cancel.is_some(), "tiene que poder cancelarse");
    checks_text_valid(spec.title, "título");
    checks_text_valid(spec.accept, "aceptar");
    checks_text_valid(spec.cancel.expect("hay Cancelar"), "cancelar");
    for field_2 in &spec.fields {
        checks_text_valid(field_2.label, field_2.id);
        if let FieldKind::Choice { options, .. } = &field_2.kind {
            for option in *options {
                checks_text_valid(option, field_2.id);
            }
        }
    }
}

#[test]
fn row_12_demo_form_is_valid_by_default_and_the_hexadecimal_shows_only_with_its_option() {
    let spec = demo_form();
    let hex = hex_2(&spec)[0];
    let condition = hex
        .shown_when
        .as_ref()
        .expect("el hexadecimal está condicionado a una opción");
    let group = *groups(&spec)
        .iter()
        .find(|g| g.id == condition.field)
        .expect("la condición apunta a un grupo de opciones");
    assert!(condition.choice < options_2(group).len());

    let mut dialog = opens();
    assert!(
        dialog.is_valid(),
        "válido por defecto: {:?}",
        dialog.reason()
    );
    assert_eq!(dialog.reason(), None);
    assert!(dialog.values().is_some());

    for k in 0..options_2(group).len() {
        dialog.event(Event::Choice {
            field: group.id,
            index: k,
        });
        assert_eq!(
            dialog.is_shown(hex.id),
            k == condition.choice,
            "opción {k}: el hexadecimal solo se muestra con la opción {}",
            condition.choice
        );
        assert!(dialog.is_valid(), "opción {k}");
        let values = dialog.values().expect("válido");
        assert_eq!(values.choice(group.id), Some(k));
        assert_eq!(values.color(hex.id).is_some(), k == condition.choice);
    }
}

#[test]
fn row_12_demo_form_offers_the_ranges_of_the_manual_trial() {
    // De «Prueba manual», punto 5: un numérico de 1 a 8192 (`0` da «mínimo 1» y `8193`
    // «máximo 8192») y otro que llega a `4294967295`.
    let spec = demo_form();
    let ranges: Vec<(u32, u32)> = numeric(&spec)
        .iter()
        .map(|f| match f.kind {
            FieldKind::Number { min, max, .. } => (min, max),
            _ => unreachable!(),
        })
        .collect();
    assert!(
        ranges.contains(&(1, 8192)),
        "un numérico de 1 a 8192: {ranges:?}"
    );
    assert!(
        ranges.iter().any(|&(_, max)| max == u32::MAX),
        "un numérico que llega a u32::MAX: {ranges:?}"
    );
}

// ---------------------------------------------------------------------------------------------
// demo_notice
// ---------------------------------------------------------------------------------------------

#[test]
fn row_12_demo_notice_with_cancelled_says_cancelled() {
    let text_2 = demo_notice(&Reaction::Cancelled);
    checks_text_valid(&text_2, "Cancelled");
    assert!(text_2.contains("cancelado"), "{text_2:?}");
    assert!(!text_2.contains("aceptado"), "{text_2:?}");
}

#[test]
fn row_12_demo_notice_with_accepted_lists_what_was_accepted() {
    let spec = demo_form();
    let group = groups(&spec)[0];
    let hex = hex_2(&spec)[0];
    let box_value = boxes_2(&spec)[0];

    // Todos los valores por defecto y, a continuación, cada opción con la casilla en los dos
    // estados; los numéricos con valores distintos de los de por defecto.
    for k in 0..options_2(group).len() {
        for marked in [false, true] {
            let mut dialog = opens();
            dialog.event(Event::Choice {
                field: group.id,
                index: k,
            });
            dialog.event(Event::Check {
                field: box_value.id,
                on: marked,
            });
            for (i, n) in numeric(&spec).iter().enumerate() {
                dialog.event(Event::Number {
                    field: n.id,
                    text: format!("{}", 300 + 111 * i),
                });
            }
            let reaction = accepted(&mut dialog);
            let Reaction::Accepted(values) = &reaction else {
                unreachable!()
            };
            let text_2 = demo_notice(&reaction);
            checks_text_valid(&text_2, "Accepted");
            assert!(text_2.contains("aceptado"), "{text_2:?}");
            assert!(!text_2.contains("cancelado"), "{text_2:?}");

            // Cada campo visible con valor: su rótulo y su valor.
            for n in numeric(&spec) {
                assert!(
                    text_2.contains(n.label),
                    "falta el rótulo {:?}: {text_2:?}",
                    n.label
                );
                let value = values.number(n.id).expect("el numérico se entrega");
                assert!(
                    text_2.contains(&value.to_string()),
                    "falta el valor {value} de {:?}: {text_2:?}",
                    n.label
                );
            }
            assert!(text_2.contains(group.label), "{text_2:?}");
            assert!(
                text_2.contains(options_2(group)[k]),
                "falta la opción elegida {:?}: {text_2:?}",
                options_2(group)[k]
            );
            assert!(text_2.contains(box_value.label), "{text_2:?}");
            assert!(
                text_2.contains(if marked { "sí" } else { "no" }),
                "la casilla ({marked}) se dice con sí o no: {text_2:?}"
            );
            // El hexadecimal solo sale si se muestra.
            let FieldKind::Hex { default, .. } = &hex.kind else {
                unreachable!()
            };
            match values.color(hex.id) {
                Some(color) => {
                    assert!(text_2.contains(hex.label), "{text_2:?}");
                    assert!(
                        text_2.contains(&hex_text(color)),
                        "falta el color: {text_2:?}"
                    );
                }
                None => assert!(
                    !text_2.contains(&hex_text(*default)),
                    "un campo oculto no sale: {text_2:?}"
                ),
            }
        }
    }
}

#[test]
fn row_12_demo_notice_with_accepted_and_a_different_color_lists_it() {
    let spec = demo_form();
    let group = groups(&spec)[0];
    let hex = hex_2(&spec)[0];
    let condition = hex.shown_when.as_ref().expect("condicionado");
    let mut dialog = opens();
    dialog.event(Event::Choice {
        field: group.id,
        index: condition.choice,
    });
    // Un color que vale en todos los modos (gris opaco): se escribe y se confirma.
    dialog.event(Event::Hex {
        field: hex.id,
        text: "#777777".into(),
    });
    dialog.event(Event::HexConfirm {
        field: hex.id,
        blur: false,
    });
    let reaction = accepted(&mut dialog);
    let Reaction::Accepted(values) = &reaction else {
        unreachable!()
    };
    let color = values.color(hex.id).expect("el hexadecimal se muestra");
    assert_eq!((color.r, color.g, color.b), (0x77, 0x77, 0x77));
    let text_2 = demo_notice(&reaction);
    checks_text_valid(&text_2, "Accepted con color");
    assert!(text_2.contains(&hex_text(color)), "{text_2:?}");
}

#[test]
fn row_12_demo_notice_with_rejected_says_the_reason() {
    for reason_2 in [
        "Ancho: escribe un número",
        "demasiado grande",
        "a",
        "Color: usa 3, 4, 6 u 8 dígitos",
    ] {
        let text_2 = demo_notice(&Reaction::Rejected(reason_2.to_string()));
        checks_text_valid(&text_2, "Rejected");
        assert!(
            text_2.contains(reason_2),
            "falta el motivo {reason_2:?}: {text_2:?}"
        );
        assert!(!text_2.contains("aceptado"), "{text_2:?}");
    }
}

#[test]
fn row_12_demo_notice_with_none_gives_a_reasonable_text() {
    let text_2 = demo_notice(&Reaction::None);
    checks_text_valid(&text_2, "None");
    assert!(!text_2.contains("aceptado"), "{text_2:?}");
    assert!(!text_2.contains("cancelado"), "{text_2:?}");
}

#[test]
fn row_12_demo_notice_is_pure_and_does_not_consume_the_reaction() {
    let mut dialog = opens();
    let reaction = accepted(&mut dialog);
    let reactions = [
        Reaction::None,
        Reaction::Cancelled,
        Reaction::Rejected("Ancho: máximo 8192".into()),
        reaction.clone(),
    ];
    for r in &reactions {
        let copy = r.clone();
        let a = demo_notice(r);
        let b = demo_notice(r);
        assert_eq!(a, b, "misma entrada, misma salida");
        assert_eq!(r, &copy, "la reacción no cambia");
    }
    // Tampoco toca el diálogo del que salió.
    let values_2: Option<Values> = dialog.values();
    demo_notice(&reaction);
    assert_eq!(dialog.values(), values_2);
}
