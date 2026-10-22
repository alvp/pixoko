//! Tests del estado puro de los diálogos `px_ui::dialog` (specs/hito-2-dialogos-modales.md, unidad
//! 18, decisiones 1, 3, 4 y 6, «Casos» filas 3 y 4 y propiedad 2). Sin egui: ni ventana ni banco.
//! **En rojo** hasta que exista el módulo.
//!
//! - Fila 3: validez de cada campo y del conjunto con el formulario de los cinco tipos
//!   (`dialog_forms::all_kinds`), los motivos, el `check` con su contador de llamadas,
//!   `FormSpec::validate` con cada `SpecError` y que `Dialog::new` y `Modal::open` la rechazan.
//! - Fila 4: `Modal` cerrado y abierto, eventos válidos e inválidos, `Accept`, `Cancel`,
//!   `abandon`, `close_kind`, las reglas del hexadecimal (decisión 6) y `take_focus_request`.
//! - Propiedad 2: el estado nunca se corrompe, con formularios y secuencias de eventos al azar
//!   (`dialog_random`) y un modelo escrito aparte.
//!
//! Derives que se suponen (la spec no los lista): `Dialog`, `Values`, `Value`, `Validity`,
//! `Reaction`, `Event` y `SpecError` son `Clone + Debug + PartialEq`; `DialogKind` es
//! `Clone + Copy + Debug + PartialEq + Eq` (`Dialog::kind` lo devuelve por valor);
//! `FocusTarget`, `Clone + Copy + Debug + PartialEq + Eq`. `Modal` solo se supone `Default`.
//!
//! Decisiones donde la spec no fija un detalle:
//! - `SpecError` no tiene variantes nombradas en la spec: cada causa se prueba con
//!   `validate().is_err()`, y `Dialog::new` y `Modal::open` con el mismo formulario.
//! - Solo existe `DialogKind::Licenses` (`#[non_exhaustive]` y «la 19 y la 20 añaden las suyas»):
//!   el «segundo `open` de otro tipo» y «`close_kind` solo cierra el tipo pedido» no se pueden
//!   probar con dos tipos; se prueba el segundo `open` del mismo tipo y que `close_kind` cierra
//!   el tipo abierto y da `false` con el diálogo cerrado.
//! - El recorte del texto del `check` es a **37** caracteres (decisión 8, «Piezas» y la frase
//!   «todo motivo mide ≤ 37» de la propia fila 3); la fila dice «44» en un sitio, que se lee como
//!   un descuido. Se comprueba que el motivo mide ≤ 37 y es un prefijo del texto del `check`.
//! - `Validity::Invalid(m)` lleva el motivo; `reason()` es `label_2: reason_2`. Para no fijar si
//!   `m` lleva o no la etiqueta, solo se exige que `reason()` empiece por la etiqueta y contenga
//!   `m`. `validity` de un campo que no es numérico ni hexadecimal puede ser `Valid` o `None`
//!   (en las propiedades se exige solo que no sea `Invalid`); de un id desconocido, `None`.
//! - Eventos sobre un campo **oculto**: la spec no dice si se aplican; las pruebas fijas no los
//!   envían y la propiedad no comprueba el campo tocado (solo que no se corrompe nada).
//! - `Values` de un hexadecimal con un pendiente válido sin confirmar: la spec no dice si trae el
//!   color confirmado o el pendiente; `Accept` confirma el pendiente antes de entregar. En la
//!   propiedad 2 los hexadecimales con pendiente no se comparan entre `values()` y `Accepted`.
//! - Un id desconocido: `is_shown` da `false`, `validity` y `text` dan `None`, `hex_pending`
//!   `false`.
//! - `content_h_u` sigue las medidas de «Geometría»: `16` de relleno más los bloques (numérico y
//!   hexadecimal 32, casilla 16, grupo `14 + 16·n + 2·(n−1)`, zona `6 + 14·min(rows_2, líneas)`)
//!   más 4 entre bloques; un formulario vacío mide 16; con `Fill`, 330 (el contenido natural
//!   de la spec, `16 + 6 + 22·14`).
//! - El orden de `check`: se evalúa solo con todos los campos visibles válidos.

use std::collections::HashSet;

use proptest::prelude::*;
use px_core::{Mode, Rgba};
use px_ui::actions::Locks;
use px_ui::color_edit::{HexError, hex_notice, parse_hex};
use px_ui::dialog::{
    Dialog, DialogKind, Event, FieldKind, FieldSpec, FormSpec, Modal, Reaction, Rows, ShowWhen,
    Validity, Value, Values, resolve_hex,
};
use px_ui::dialog_keys::{FocusTarget, initial_focus};
use px_ui::licenses_view::{LICENSES_FIELD, licenses_form};
use px_ui::number::{bounded_text, parse_number, step_number};
use px_ui::text::INTERFACE_CHARSET;

#[path = "common/dialog_forms.rs"]
mod dialog_forms;
#[path = "common/dialog_random.rs"]
mod dialog_random;

use dialog_forms::{all_kinds, check_calls, huge, open, reset_check_calls};
use dialog_random::{
    EventRecipe, FormRecipe, IDS, KIND_ABANDON, LABELS, SUM_ODD, build, event_of, event_recipe,
    form_recipe, target_id,
};

// ---------------------------------------------------------------------------------------------
// Ayudas
// ---------------------------------------------------------------------------------------------

fn dialog() -> Dialog {
    open(all_kinds())
}

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

fn hex(d: &mut Dialog, field: &'static str, text: &str) {
    assert_eq!(
        d.event(Event::Hex {
            field,
            text: text.to_string()
        }),
        Reaction::None
    );
}

fn confirms(d: &mut Dialog, field: &'static str, blur: bool) {
    assert_eq!(d.event(Event::HexConfirm { field, blur }), Reaction::None);
}

fn in_the_set(text_2: &str) -> bool {
    text_2.chars().all(|c| INTERFACE_CHARSET.contains(c))
}

fn field_2(id: &'static str, label: &'static str, kind: FieldKind) -> FieldSpec {
    FieldSpec {
        id,
        label,
        kind,
        shown_when: None,
    }
}

fn numeric(id: &'static str, label: &'static str, min: u32, max: u32, default: u32) -> FieldSpec {
    field_2(id, label, FieldKind::Number { min, max, default })
}

fn spec_with(fields: Vec<FieldSpec>) -> FormSpec {
    FormSpec {
        title: "Prueba",
        accept: "Aceptar",
        cancel: Some("Cancelar"),
        fields,
        check: None,
    }
}

/// Un formulario con un solo hexadecimal «color».
fn hex_form(mode: Mode, default: Rgba) -> FormSpec {
    spec_with(vec![field_2(
        "color",
        "Color",
        FieldKind::Hex { mode, default },
    )])
}

/// El color confirmado, leído sin cerrar el diálogo (con `Accept` sobre una copia).
fn color_accepted(d: &Dialog) -> Rgba {
    match d.clone().event(Event::Accept) {
        Reaction::Accepted(v) => v
            .color("color")
            .expect("el hexadecimal sale en los valores"),
        r => panic!("se esperaba Accepted y salió {r:?}"),
    }
}

fn dialog_of(modal: &Modal) -> &Dialog {
    match modal {
        Modal::Open(d) => d,
        Modal::Closed => panic!("el modal está cerrado"),
    }
}

fn modal_open(spec: FormSpec) -> Modal {
    let mut modal = Modal::default();
    assert!(modal.open(DialogKind::Licenses, spec, Locks::default()));
    modal
}

fn checks_rejected(cause: &str, spec: FormSpec) {
    assert!(spec.validate().is_err(), "{cause}: validate la acepta");
}

/// Como `checks_rejected`, y además `Dialog::new` y `Modal::open` la rechazan.
fn rejects(cause: &str, mut does: impl FnMut() -> FormSpec) {
    checks_rejected(cause, does());
    assert!(
        Dialog::new(DialogKind::Licenses, does()).is_err(),
        "{cause}: Dialog::new la acepta"
    );
    let mut modal = Modal::default();
    assert!(
        !modal.open(DialogKind::Licenses, does(), Locks::default()),
        "{cause}: Modal::open la acepta"
    );
    assert!(!modal.is_open(), "{cause}: el modal se abrió");
    assert!(matches!(modal, Modal::Closed));
}

// ---------------------------------------------------------------------------------------------
// Fila 3: validez de cada campo y del conjunto
// ---------------------------------------------------------------------------------------------

#[test]
fn row_3_the_spec_of_the_five_kinds_valid() {
    assert!(all_kinds().validate().is_ok());
    assert!(huge(10).validate().is_ok());
    assert!(huge(0).validate().is_ok());
    assert!(spec_with(vec![]).validate().is_ok());
}

#[test]
fn row_3_by_default_it_is_valid_and_the_values_are_those_of_the_visible_fields() {
    let d = dialog();
    assert!(d.is_valid());
    assert_eq!(d.reason(), None);
    let v = d.values().expect("válido: hay valores");
    assert_eq!(v.number("ancho"), Some(64));
    assert_eq!(v.number("alto"), Some(64));
    assert_eq!(v.choice("modo"), Some(0));
    assert_eq!(v.check("opaco"), Some(false));
    assert_eq!(v.get("ancho"), Some(&Value::Number(64)));
    assert_eq!(v.get("modo"), Some(&Value::Choice(0)));
    assert_eq!(v.get("opaco"), Some(&Value::Check(false)));
    // El hexadecimal está oculto (la opción es la 0) y el informe no es un valor.
    assert_eq!(v.get("color"), None);
    assert_eq!(v.color("color"), None);
    assert_eq!(v.get("informe"), None);
    assert_eq!(v.get("nada"), None);
    // Un getter de otro tipo no devuelve nada.
    assert_eq!(v.choice("ancho"), None);
    assert_eq!(v.number("modo"), None);
    assert_eq!(v.check("alto"), None);
    assert_eq!(v.color("opaco"), None);
}

#[test]
fn row_3_the_basic_accessors() {
    let d = dialog();
    assert_eq!(d.kind(), DialogKind::Licenses);
    assert_eq!(d.spec().title, "Prueba");
    assert_eq!(d.spec().accept, "Aceptar");
    assert_eq!(d.spec().cancel, Some("Cancelar"));
    assert_eq!(d.spec().fields.len(), 6);
    assert!(d.is_shown("ancho"));
    assert!(d.is_shown("modo"));
    assert!(d.is_shown("opaco"));
    assert!(d.is_shown("informe"));
    assert!(!d.is_shown("color"));
    assert!(!d.is_shown("nada"));
    assert_eq!(d.text("ancho"), Some("64".to_string()));
    assert_eq!(d.text("alto"), Some("64".to_string()));
    assert_eq!(d.validity("ancho"), Some(Validity::Valid));
    assert_eq!(d.validity("alto"), Some(Validity::Valid));
    assert_eq!(d.validity("nada"), None);
    assert_eq!(d.text("nada"), None);
    assert!(!d.hex_pending("color"));
    assert!(!d.hex_pending("nada"));
    assert!(!d.is_fill());
}

#[test]
fn row_3_an_empty_numeric_gives_the_reason_with_the_label() {
    let mut d = dialog();
    number(&mut d, "ancho", "");
    assert!(!d.is_valid());
    assert_eq!(d.reason(), Some("Ancho: escribe un número".to_string()));
    assert_eq!(d.values().map(|_| ()), None);
    assert_eq!(d.text("ancho"), Some(String::new()));
    let Some(Validity::Invalid(reason_2)) = d.validity("ancho") else {
        panic!("el campo vacío es inválido");
    };
    assert!(d.reason().unwrap().contains(&reason_2));
    assert_eq!(d.validity("alto"), Some(Validity::Valid));
}

#[test]
fn row_3_the_reasons_of_each_numeric_error() {
    let cases = [
        ("", "Ancho: escribe un número"),
        ("abc", "Ancho: solo dígitos 0-9"),
        (" 5", "Ancho: solo dígitos 0-9"),
        ("-5", "Ancho: solo dígitos 0-9"),
        ("0", "Ancho: mínimo 1"),
        ("8193", "Ancho: máximo 8192"),
        ("99999999999", "Ancho: máximo 10 cifras"),
        ("4294967295", "Ancho: máximo 8192"),
    ];
    for (text_2, expected_2) in cases {
        let mut d = dialog();
        number(&mut d, "ancho", text_2);
        assert_eq!(d.reason().as_deref(), Some(expected_2), "texto {text_2:?}");
    }
}

#[test]
fn row_3_two_invalid_at_once_the_reason_is_that_of_the_first_in_field_order() {
    let mut d = dialog();
    number(&mut d, "alto", "abc");
    number(&mut d, "ancho", "");
    assert_eq!(d.reason(), Some("Ancho: escribe un número".to_string()));
    // Se arregla el primero: sale el segundo.
    number(&mut d, "ancho", "10");
    assert_eq!(d.reason(), Some("Alto: solo dígitos 0-9".to_string()));
    // Y al arreglar el segundo, todo vale.
    number(&mut d, "alto", "10");
    assert!(d.is_valid());
    assert_eq!(d.reason(), None);
    assert!(d.values().is_some());
}

#[test]
fn row_3_the_hidden_hexadecimal_does_not_count_and_when_shown_it_does_and_recovers_what_was_written()
 {
    let mut d = dialog();
    // Se muestra con la opción 2 y se escribe algo inválido.
    chooses(&mut d, "modo", 2);
    assert!(d.is_shown("color"));
    hex(&mut d, "color", "zz");
    assert!(!d.is_valid());
    let reason_2 = d.reason().expect("hay motivo");
    assert!(reason_2.starts_with("Color: "), "{reason_2}");
    // Se oculta: ya no cuenta y no sale en los valores, pero conserva lo escrito.
    chooses(&mut d, "modo", 0);
    assert!(!d.is_shown("color"));
    assert!(d.is_valid());
    assert_eq!(d.reason(), None);
    let v = d.values().expect("válido");
    assert_eq!(v.get("color"), None);
    assert_eq!(d.text("color"), Some("zz".to_string()));
    assert!(d.hex_pending("color"));
    // Al mostrarse otra vez recupera lo escrito y vuelve a ser inválido, con el mismo motivo.
    chooses(&mut d, "modo", 2);
    assert!(!d.is_valid());
    assert_eq!(d.reason(), Some(reason_2.clone()));
    assert_eq!(d.text("color"), Some("zz".to_string()));
    // Y de nuevo oculto.
    chooses(&mut d, "modo", 1);
    assert!(d.is_valid());
    assert_eq!(d.text("color"), Some("zz".to_string()));
    // Con la opción 2 y el color por defecto sale en los valores.
    let mut d = dialog();
    chooses(&mut d, "modo", 2);
    let v = d.values().expect("válido");
    assert_eq!(v.color("color"), Some(Rgba::opaque(0x33, 0x66, 0x99)));
    assert_eq!(
        v.get("color"),
        Some(&Value::Color(Rgba::opaque(0x33, 0x66, 0x99)))
    );
    assert_eq!(d.text("color"), Some("#336699".to_string()));
}

#[test]
fn row_3_the_hidden_numerics_do_not_count_either() {
    let mut spec = spec_with(vec![
        field_2(
            "modo",
            "Modo",
            FieldKind::Choice {
                options: &["A", "B"],
                default: 0,
            },
        ),
        numeric("n", "Cuanto", 1, 10, 5),
    ]);
    spec.fields[1].shown_when = Some(ShowWhen {
        field: "modo",
        choice: 1,
    });
    let mut d = open(spec);
    assert!(!d.is_shown("n"));
    assert_eq!(d.values().expect("válido").number("n"), None);
    chooses(&mut d, "modo", 1);
    number(&mut d, "n", "");
    assert_eq!(d.reason(), Some("Cuanto: escribe un número".to_string()));
    chooses(&mut d, "modo", 0);
    assert!(d.is_valid());
    assert_eq!(d.text("n"), Some(String::new()));
}

#[test]
fn row_3_the_hexadecimal_reasons_are_those_of_hex_notice() {
    let def = Rgba::opaque(0x33, 0x66, 0x99);
    for text_2 in ["#12", "zz", "zzz", "", "#12345", "#ggg", "#12 3"] {
        let mut d = open(hex_form(Mode::Rgba, def));
        hex(&mut d, "color", text_2);
        let error = parse_hex(text_2, Mode::Rgba, def).expect_err(text_2);
        assert!(!d.is_valid(), "{text_2:?}");
        assert_eq!(
            d.reason(),
            Some(format!("Color: {}", hex_notice(&error))),
            "{text_2:?}"
        );
        assert!(d.hex_pending("color"), "{text_2:?}");
    }
    // En el formulario de los cinco tipos, con `#12`.
    let mut d = dialog();
    chooses(&mut d, "modo", 2);
    hex(&mut d, "color", "#12");
    assert_eq!(
        d.reason(),
        Some("Color: usa 3, 4, 6 u 8 dígitos".to_string())
    );
}

#[test]
fn row_3_hexadecimal_in_gray_mode() {
    let gray_2 = Rgba::opaque(0x80, 0x80, 0x80);
    let mut d = open(hex_form(Mode::Gray, gray_2));
    hex(&mut d, "color", "#ff0000");
    assert_eq!(
        d.reason(),
        Some("Color: en modo gris: r = g = b".to_string())
    );
    assert!(!d.is_valid());
    hex(&mut d, "color", "#80808080");
    assert_eq!(
        d.reason(),
        Some("Color: este modo no tiene alfa".to_string())
    );
    hex(&mut d, "color", "#aaa");
    assert!(d.is_valid());
    assert_eq!(d.reason(), None);
    confirms(&mut d, "color", false);
    assert_eq!(color_accepted(&d), Rgba::opaque(0xaa, 0xaa, 0xaa));
    // GrayAlpha admite alfa pero no color.
    let mut d = open(hex_form(Mode::GrayAlpha, Rgba::new(0x80, 0x80, 0x80, 0xff)));
    hex(&mut d, "color", "#ff000080");
    assert_eq!(
        d.reason(),
        Some("Color: en modo gris: r = g = b".to_string())
    );
    hex(&mut d, "color", "#80808040");
    assert!(d.is_valid());
}

#[test]
fn row_3_the_check_is_only_called_with_everything_valid() {
    // Con un campo inválido, ni al crear el estado ni al consultarlo se llama.
    let mut d = dialog();
    reset_check_calls();
    number(&mut d, "ancho", "");
    let _ = (d.is_valid(), d.reason(), d.values(), d.validity("ancho"));
    let _ = (d.validity("alto"), d.text("ancho"), d.is_shown("color"));
    assert_eq!(
        check_calls(),
        0,
        "el check no se llama con un campo inválido"
    );
    // Con todo válido sí se llama.
    reset_check_calls();
    number(&mut d, "ancho", "10");
    let _ = (d.is_valid(), d.reason(), d.values());
    assert!(check_calls() >= 1, "el check se llama con todo válido");
}

#[test]
fn row_3_the_check_that_fails_with_everything_valid_invalidates_the_set() {
    let mut d = dialog();
    number(&mut d, "ancho", "2000");
    assert!(d.is_valid(), "2000 × 64 cabe");
    reset_check_calls();
    number(&mut d, "alto", "2000");
    assert!(!d.is_valid());
    assert_eq!(d.reason(), Some("demasiado grande".to_string()));
    assert!(d.values().is_none());
    assert!(check_calls() >= 1);
    // El motivo del check no se atribuye a ningún campo.
    assert_eq!(d.validity("ancho"), Some(Validity::Valid));
    assert_eq!(d.validity("alto"), Some(Validity::Valid));
    // Un campo inválido gana al check: ya no se llama.
    reset_check_calls();
    number(&mut d, "alto", "");
    assert_eq!(d.reason(), Some("Alto: escribe un número".to_string()));
    assert!(d.values().is_none());
    assert_eq!(
        check_calls(),
        0,
        "con un campo inválido no se llama al check"
    );
}

#[test]
fn row_3_the_check_limit_is_a_million_pixels() {
    let mut d = dialog();
    number(&mut d, "ancho", "1000");
    number(&mut d, "alto", "1000");
    assert!(d.is_valid(), "1000 × 1000 = 1 000 000 cabe");
    number(&mut d, "alto", "1001");
    assert!(!d.is_valid());
    number(&mut d, "ancho", "8192");
    number(&mut d, "alto", "8192");
    assert_eq!(d.reason(), Some("demasiado grande".to_string()));
}

fn check_length(_: &Values) -> Option<String> {
    Some("x".repeat(100))
}

fn check_multibyte(_: &Values) -> Option<String> {
    Some("é".repeat(100))
}

fn check_never(_: &Values) -> Option<String> {
    None
}

#[test]
fn row_3_the_check_text_is_clipped_to_37_characters() {
    let mut spec = spec_with(vec![numeric("n", "Cuanto", 1, 10, 5)]);
    spec.check = Some(check_length);
    let d = open(spec);
    assert!(!d.is_valid());
    let reason_2 = d.reason().expect("el check falla");
    assert!(reason_2.chars().count() <= 37, "{reason_2:?}");
    assert!(!reason_2.is_empty());
    assert!("x".repeat(100).starts_with(&reason_2), "{reason_2:?}");

    let mut spec = spec_with(vec![numeric("n", "Cuanto", 1, 10, 5)]);
    spec.check = Some(check_multibyte);
    let d = open(spec);
    let reason_2 = d.reason().expect("el check falla");
    assert!(reason_2.chars().count() <= 37, "{reason_2:?}");
    assert!("é".repeat(100).starts_with(&reason_2));

    let mut spec = spec_with(vec![numeric("n", "Cuanto", 1, 10, 5)]);
    spec.check = Some(check_never);
    assert!(open(spec).is_valid());
}

#[test]
fn row_3_every_reason_measures_37_or_less_and_is_in_the_character_set() {
    let texts = [
        "",
        "abc",
        " 5",
        "0",
        "99999999999",
        "4294967295",
        "#12",
        "zz",
        "#ff0000",
        "#80808080",
        "#abcde",
    ];
    // Rótulos de 12 caracteres, el máximo.
    let label_2 = "ABCDEFGHIJKL";
    assert_eq!(label_2.chars().count(), 12);
    let mut greater = 0;
    for text_2 in texts {
        let mut d = open(spec_with(vec![numeric("n", label_2, 1, 8192, 64)]));
        number(&mut d, "n", text_2);
        if let Some(r) = d.reason() {
            greater = greater.max(r.chars().count());
            assert!(r.chars().count() <= 37, "{r:?}");
            assert!(in_the_set(&r), "{r:?}");
        }
        let mut d = open(hex_form_with_label(label_2));
        hex(&mut d, "color", text_2);
        if let Some(r) = d.reason() {
            greater = greater.max(r.chars().count());
            assert!(r.chars().count() <= 37, "{r:?}");
            assert!(in_the_set(&r), "{r:?}");
        }
    }
    // El peor caso (12 + «: » + 23) llega exactamente a 37.
    let mut d = open(hex_form_with_label(label_2));
    hex(&mut d, "color", "#ff0000");
    assert_eq!(d.reason().expect("modo gris").chars().count(), 37);
    assert_eq!(greater, 37);
}

fn hex_form_with_label(label: &'static str) -> FormSpec {
    spec_with(vec![field_2(
        "color",
        label,
        FieldKind::Hex {
            mode: Mode::Gray,
            default: Rgba::opaque(0x80, 0x80, 0x80),
        },
    )])
}

#[test]
fn row_3_the_reason_of_any_state_of_the_five_kinds_fits_in_the_line() {
    // `error: ` (7) + motivo (≤ 37) = 44 columnas.
    for text_2 in ["", "x", "0", "8193", "99999999999"] {
        let mut d = dialog();
        number(&mut d, "alto", text_2);
        if let Some(r) = d.reason() {
            assert!(7 + r.chars().count() <= 44, "{r:?}");
            assert!(in_the_set(&r));
        }
    }
}

#[test]
fn row_3_validate_accepts_the_exact_limits() {
    // Rótulos de 12 (numérico y hexadecimal) y 42 (casilla); min = max; valores por defecto en
    // los extremos; la última opción por defecto.
    let spec = spec_with(vec![
        numeric("a", "ABCDEFGHIJKL", 5, 5, 5),
        numeric("b", "B", 0, u32::MAX, 0),
        numeric("c", "C", 0, u32::MAX, u32::MAX),
        field_2(
            "d",
            "ABCDEFGHIJKL",
            FieldKind::Hex {
                mode: Mode::Rgba,
                default: Rgba::TRANSPARENT,
            },
        ),
        field_2("e", "x", FieldKind::Check { default: true }),
        field_2(
            "f",
            "Opciones",
            FieldKind::Choice {
                options: &["A", "B", "C"],
                default: 2,
            },
        ),
    ]);
    assert!(spec.validate().is_ok());
    let box_long: &'static str = Box::leak("x".repeat(42).into_boxed_str());
    let spec = spec_with(vec![field_2(
        "g",
        box_long,
        FieldKind::Check { default: false },
    )]);
    assert!(spec.validate().is_ok());
    // El texto del informe no es fijo: pasa por `printable` al pintar y no se valida contra el
    // juego de caracteres.
    let spec = spec_with(vec![field_2(
        "h",
        "Informe",
        FieldKind::ReadOnly {
            text: "日✓ 🙂".to_string(),
            rows: Rows::Fixed(3),
        },
    )]);
    assert!(spec.validate().is_ok());
    // Un `Fill` solo es válido.
    let spec = spec_with(vec![field_2(
        "i",
        "Zona",
        FieldKind::ReadOnly {
            text: "x".to_string(),
            rows: Rows::Fill,
        },
    )]);
    assert!(spec.validate().is_ok());
    // `shown_when` válido: un `Choice` anterior y una opción que existe.
    let mut spec = spec_with(vec![
        field_2(
            "modo",
            "Modo",
            FieldKind::Choice {
                options: &["A", "B"],
                default: 0,
            },
        ),
        field_2("c", "Casilla", FieldKind::Check { default: false }),
    ]);
    spec.fields[1].shown_when = Some(ShowWhen {
        field: "modo",
        choice: 1,
    });
    assert!(spec.validate().is_ok());
}

#[test]
fn row_3_spec_error_repeated_id() {
    rejects("id repetido", || {
        spec_with(vec![
            field_2("a", "Uno", FieldKind::Check { default: false }),
            field_2("a", "Dos", FieldKind::Check { default: true }),
        ])
    });
    rejects("id repetido entre tipos distintos", || {
        spec_with(vec![
            numeric("a", "Uno", 1, 2, 1),
            field_2(
                "b",
                "Dos",
                FieldKind::Choice {
                    options: &["x"],
                    default: 0,
                },
            ),
            field_2("a", "Tres", FieldKind::Check { default: false }),
        ])
    });
}

#[test]
fn row_3_spec_error_min_greater_than_max() {
    rejects("min > max", || spec_with(vec![numeric("n", "N", 10, 5, 7)]));
    rejects("min > max por uno", || {
        spec_with(vec![numeric("n", "N", 6, 5, 5)])
    });
}

#[test]
fn row_3_spec_error_default_value_outside() {
    rejects("default por debajo", || {
        spec_with(vec![numeric("n", "N", 1, 10, 0)])
    });
    rejects("default por encima", || {
        spec_with(vec![numeric("n", "N", 1, 10, 11)])
    });
    rejects("opción por defecto = len", || {
        spec_with(vec![field_2(
            "m",
            "M",
            FieldKind::Choice {
                options: &["a", "b", "c"],
                default: 3,
            },
        )])
    });
    rejects("opción por defecto enorme", || {
        spec_with(vec![field_2(
            "m",
            "M",
            FieldKind::Choice {
                options: &["a"],
                default: usize::MAX,
            },
        )])
    });
}

#[test]
fn row_3_spec_error_empty_options() {
    rejects("opciones vacías", || {
        spec_with(vec![field_2(
            "m",
            "M",
            FieldKind::Choice {
                options: &[],
                default: 0,
            },
        )])
    });
}

#[test]
fn row_3_spec_error_long_label() {
    rejects("rótulo numérico de 13", || {
        spec_with(vec![numeric("n", "ABCDEFGHIJKLM", 1, 2, 1)])
    });
    rejects("rótulo hexadecimal de 13", || {
        spec_with(vec![field_2(
            "c",
            "ABCDEFGHIJKLM",
            FieldKind::Hex {
                mode: Mode::Rgba,
                default: Rgba::TRANSPARENT,
            },
        )])
    });
    let length_2: &'static str = Box::leak("x".repeat(43).into_boxed_str());
    rejects("rótulo de casilla de 43", || {
        spec_with(vec![field_2(
            "c",
            length_2,
            FieldKind::Check { default: false },
        )])
    });
}

#[test]
fn row_3_spec_error_text_outside_the_set() {
    rejects("título", || {
        let mut s = spec_with(vec![]);
        s.title = "日本";
        s
    });
    rejects("botón de aceptar", || {
        let mut s = spec_with(vec![]);
        s.accept = "✓";
        s
    });
    rejects("botón de cancelar", || {
        let mut s = spec_with(vec![]);
        s.cancel = Some("日");
        s
    });
    rejects("rótulo de un campo", || {
        spec_with(vec![field_2(
            "c",
            "Año日",
            FieldKind::Check { default: false },
        )])
    });
    rejects("opción de un grupo", || {
        spec_with(vec![field_2(
            "m",
            "M",
            FieldKind::Choice {
                options: &["bien", "mal✗"],
                default: 0,
            },
        )])
    });
    // Los acentos y la eñe del español sí están.
    let mut s = spec_with(vec![field_2(
        "c",
        "Año ¿sí?",
        FieldKind::Check { default: false },
    )]);
    s.title = "Información — ñandú · ×…";
    assert!(s.validate().is_ok());
}

#[test]
fn row_3_spec_error_shown_when_invalid() {
    let mode_2 = || {
        field_2(
            "modo",
            "Modo",
            FieldKind::Choice {
                options: &["A", "B"],
                default: 0,
            },
        )
    };
    rejects("shown_when de un campo que no existe", || {
        let mut s = spec_with(vec![
            mode_2(),
            field_2("c", "C", FieldKind::Check { default: false }),
        ]);
        s.fields[1].shown_when = Some(ShowWhen {
            field: "nada",
            choice: 0,
        });
        s
    });
    rejects("shown_when de un campo que no es Choice", || {
        let mut s = spec_with(vec![
            field_2("k", "K", FieldKind::Check { default: false }),
            field_2("c", "C", FieldKind::Check { default: false }),
        ]);
        s.fields[1].shown_when = Some(ShowWhen {
            field: "k",
            choice: 0,
        });
        s
    });
    rejects("shown_when de un numérico", || {
        let mut s = spec_with(vec![
            numeric("n", "N", 1, 2, 1),
            field_2("c", "C", FieldKind::Check { default: false }),
        ]);
        s.fields[1].shown_when = Some(ShowWhen {
            field: "n",
            choice: 0,
        });
        s
    });
    rejects("shown_when con una opción que no existe", || {
        let mut s = spec_with(vec![
            mode_2(),
            field_2("c", "C", FieldKind::Check { default: false }),
        ]);
        s.fields[1].shown_when = Some(ShowWhen {
            field: "modo",
            choice: 2,
        });
        s
    });
}

#[test]
fn row_3_spec_error_fill_with_other_visible_fields() {
    let zone = |id: &'static str| {
        field_2(
            id,
            "Zona",
            FieldKind::ReadOnly {
                text: "texto".to_string(),
                rows: Rows::Fill,
            },
        )
    };
    rejects("Fill con una casilla", || {
        spec_with(vec![
            field_2("c", "C", FieldKind::Check { default: false }),
            zone("z"),
        ])
    });
    rejects("Fill con un numérico después", || {
        spec_with(vec![zone("z"), numeric("n", "N", 1, 2, 1)])
    });
    rejects("dos Fill", || spec_with(vec![zone("z"), zone("y")]));
}

#[test]
fn row_3_dialog_new_and_modal_open_accept_a_valid_spec() {
    assert!(Dialog::new(DialogKind::Licenses, all_kinds()).is_ok());
    let mut modal = Modal::default();
    assert!(modal.open(DialogKind::Licenses, all_kinds(), Locks::default()));
    assert!(modal.is_open());
}

// ---------------------------------------------------------------------------------------------
// Geometría que expone el estado: content_h_u, is_fill, scroll y primera línea
// ---------------------------------------------------------------------------------------------

#[test]
fn row_3_content_h_u_follows_the_measures_of_the_geometry() {
    // Con la opción 0: numérico 32 + numérico 32 + grupo (14 + 3·16 + 2·2 = 66) + casilla 16 +
    // informe (6 + 4·14 = 62) = 208, más 4 huecos de 4 y 16 de relleno.
    let mut d = dialog();
    assert_eq!(d.content_h_u(), 208 + 16 + 16);
    // Con la opción 2 se suma el hexadecimal (32) y un hueco.
    chooses(&mut d, "modo", 2);
    assert_eq!(d.content_h_u(), 240 + 32 + 4);
    // Vacío: solo el relleno.
    assert_eq!(open(spec_with(vec![])).content_h_u(), 16);
    // N casillas: 16 de relleno + 16·N + 4·(N − 1).
    for n in [1usize, 2, 10, 64] {
        assert_eq!(
            open(huge(n)).content_h_u(),
            (16 + 16 * n + 4 * (n - 1)) as i64,
            "{n} casillas"
        );
    }
    // Un informe con menos líneas que filas mide las líneas que tiene.
    let spec = spec_with(vec![field_2(
        "z",
        "Z",
        FieldKind::ReadOnly {
            text: "una\ndos".to_string(),
            rows: Rows::Fixed(4),
        },
    )]);
    assert_eq!(open(spec).content_h_u(), 16 + 6 + 2 * 14);
}

#[test]
fn row_3_a_typical_form_of_the_19_measures_228() {
    // Dos numéricos, un grupo de 4 opciones, una casilla y un hexadecimal visible.
    let spec = spec_with(vec![
        numeric("ancho", "Ancho", 1, 8192, 64),
        numeric("alto", "Alto", 1, 8192, 64),
        field_2(
            "modo",
            "Modo",
            FieldKind::Choice {
                options: &["A", "B", "C", "D"],
                default: 0,
            },
        ),
        field_2("opaco", "Opaco", FieldKind::Check { default: false }),
        field_2(
            "color",
            "Color",
            FieldKind::Hex {
                mode: Mode::Rgba,
                default: Rgba::opaque(0, 0, 0),
            },
        ),
    ]);
    assert_eq!(open(spec).content_h_u(), 228);
}

#[test]
fn row_3_a_form_with_fill_is_fill_and_measures_330() {
    let d = open(licenses_form("texto"));
    assert!(d.is_fill());
    assert_eq!(d.content_h_u(), 330);
    assert!(!dialog().is_fill());
}

#[test]
fn row_3_the_scroll_and_the_first_line_are_stored_and_the_events_do_not_touch_them() {
    let mut d = dialog();
    assert_eq!(d.scroll_u(), 0);
    assert_eq!(d.first_line("informe"), 0);
    d.set_scroll_u(100);
    d.set_first_line("informe", 7);
    assert_eq!(d.scroll_u(), 100);
    assert_eq!(d.first_line("informe"), 7);
    number(&mut d, "ancho", "10");
    chooses(&mut d, "modo", 1);
    assert_eq!(d.scroll_u(), 100);
    assert_eq!(d.first_line("informe"), 7);
    // Guardar el desplazamiento no cambia los valores.
    let v = d.values().expect("válido");
    assert_eq!(v.number("ancho"), Some(10));
}

// ---------------------------------------------------------------------------------------------
// Fila 4: Modal y eventos
// ---------------------------------------------------------------------------------------------

#[test]
fn row_4_a_closed_modal_does_not_react() {
    let mut modal = Modal::default();
    assert!(!modal.is_open());
    assert_eq!(modal.kind(), None);
    assert_eq!(modal.event(Event::Accept), Reaction::None);
    assert_eq!(modal.event(Event::Cancel), Reaction::None);
    assert_eq!(
        modal.event(Event::Number {
            field: "ancho",
            text: "5".into()
        }),
        Reaction::None
    );
    assert!(!modal.abandon());
    assert!(!modal.close_kind(DialogKind::Licenses));
    assert!(matches!(modal, Modal::Closed));
}

#[test]
fn row_4_open_opens_and_the_second_open_is_false_and_does_not_touch_the_values() {
    let mut modal = Modal::default();
    assert!(modal.open(DialogKind::Licenses, all_kinds(), Locks::default()));
    assert!(modal.is_open());
    assert_eq!(modal.kind(), Some(DialogKind::Licenses));
    assert_eq!(
        modal.event(Event::Number {
            field: "ancho",
            text: "100".into()
        }),
        Reaction::None
    );
    // Un segundo open con uno abierto (la 19 y la 20 añadirán otros tipos; hoy solo hay uno).
    assert!(!modal.open(DialogKind::Licenses, all_kinds(), Locks::default()));
    assert!(!modal.open(DialogKind::Licenses, licenses_form("x"), Locks::default()));
    assert!(modal.is_open());
    assert_eq!(dialog_of(&modal).text("ancho"), Some("100".to_string()));
    assert_eq!(dialog_of(&modal).spec().title, "Prueba");
    assert_eq!(
        dialog_of(&modal).values().unwrap().number("ancho"),
        Some(100)
    );
}

#[test]
fn row_4_a_numeric_changes_only_its_field() {
    let mut d = dialog();
    let before_height = d.text("alto");
    number(&mut d, "ancho", "100");
    assert_eq!(d.text("ancho"), Some("100".to_string()));
    assert_eq!(d.text("alto"), before_height);
    let v = d.values().expect("válido");
    assert_eq!(v.number("ancho"), Some(100));
    assert_eq!(v.number("alto"), Some(64));
    assert_eq!(v.choice("modo"), Some(0));
    assert_eq!(v.check("opaco"), Some(false));
}

#[test]
fn row_4_the_numeric_text_is_stored_bounded_to_eleven_characters() {
    let mut d = dialog();
    number(&mut d, "ancho", "123456789012345");
    assert_eq!(d.text("ancho"), Some("12345678901".to_string()));
    assert_eq!(d.reason(), Some("Ancho: máximo 10 cifras".to_string()));
    let length_2 = "9".repeat(1_000_000);
    number(&mut d, "ancho", &length_2);
    let text_2 = d.text("ancho").expect("hay texto");
    assert_eq!(text_2, bounded_text(&length_2));
    assert!(text_2.chars().count() <= 11);
    // Multibyte: se corta en frontera de carácter.
    number(&mut d, "alto", &"é".repeat(13));
    assert_eq!(d.text("alto"), Some("é".repeat(11)));
}

#[test]
fn row_4_step_adds_subtracts_and_stops_at_the_limits() {
    let mut d = dialog();
    assert_eq!(
        d.event(Event::Step {
            field: "ancho",
            delta: 1
        }),
        Reaction::None
    );
    assert_eq!(d.text("ancho"), Some("65".to_string()));
    assert_eq!(
        d.event(Event::Step {
            field: "ancho",
            delta: -1
        }),
        Reaction::None
    );
    assert_eq!(
        d.event(Event::Step {
            field: "ancho",
            delta: -1
        }),
        Reaction::None
    );
    assert_eq!(d.text("ancho"), Some("63".to_string()));
    // Con el texto inválido, el valor por defecto en los dos sentidos.
    for delta in [1i8, -1] {
        number(&mut d, "ancho", "abc");
        d.event(Event::Step {
            field: "ancho",
            delta,
        });
        assert_eq!(d.text("ancho"), Some("64".to_string()), "delta {delta}");
    }
    // En los límites.
    number(&mut d, "ancho", "8192");
    d.event(Event::Step {
        field: "ancho",
        delta: 1,
    });
    assert_eq!(d.text("ancho"), Some("8192".to_string()));
    number(&mut d, "ancho", "1");
    d.event(Event::Step {
        field: "ancho",
        delta: -1,
    });
    assert_eq!(d.text("ancho"), Some("1".to_string()));
    // Sin ceros a la izquierda.
    number(&mut d, "ancho", "007");
    d.event(Event::Step {
        field: "ancho",
        delta: 1,
    });
    assert_eq!(d.text("ancho"), Some(step_number("007", 1, 8192, 64, 1)));
    assert_eq!(d.text("ancho"), Some("8".to_string()));
    // El `Step` deja el otro numérico como estaba.
    assert_eq!(d.text("alto"), Some("64".to_string()));
}

#[test]
fn row_4_choice_and_check_change_only_their_field() {
    let mut d = dialog();
    chooses(&mut d, "modo", 1);
    assert_eq!(d.values().unwrap().choice("modo"), Some(1));
    assert!(!d.is_shown("color"));
    chooses(&mut d, "modo", 2);
    assert!(d.is_shown("color"));
    assert_eq!(
        d.event(Event::Check {
            field: "opaco",
            on: true
        }),
        Reaction::None
    );
    let v = d.values().unwrap();
    assert_eq!(v.check("opaco"), Some(true));
    assert_eq!(v.choice("modo"), Some(2));
    assert_eq!(v.number("ancho"), Some(64));
    assert_eq!(
        d.event(Event::Check {
            field: "opaco",
            on: false
        }),
        Reaction::None
    );
    assert_eq!(d.values().unwrap().check("opaco"), Some(false));
}

#[test]
fn row_4_invalid_events_are_ignored_without_panic() {
    let base = dialog();
    let invalid_2 = [
        // Opción fuera de rango.
        Event::Choice {
            field: "modo",
            index: 3,
        },
        Event::Choice {
            field: "modo",
            index: usize::MAX,
        },
        // Campo desconocido, de cada clase de evento.
        Event::Number {
            field: "nada",
            text: "5".into(),
        },
        Event::Step {
            field: "nada",
            delta: 1,
        },
        Event::Choice {
            field: "nada",
            index: 0,
        },
        Event::Check {
            field: "nada",
            on: true,
        },
        Event::Hex {
            field: "nada",
            text: "#fff".into(),
        },
        Event::HexConfirm {
            field: "nada",
            blur: false,
        },
        Event::HexConfirm {
            field: "nada",
            blur: true,
        },
        Event::HexRevert { field: "nada" },
        // `Step` sobre un campo que no es numérico.
        Event::Step {
            field: "modo",
            delta: 1,
        },
        Event::Step {
            field: "opaco",
            delta: -1,
        },
        Event::Step {
            field: "informe",
            delta: 1,
        },
        // Un evento de otra clase de campo.
        Event::Number {
            field: "modo",
            text: "1".into(),
        },
        Event::Number {
            field: "opaco",
            text: "1".into(),
        },
        Event::Choice {
            field: "ancho",
            index: 0,
        },
        Event::Choice {
            field: "opaco",
            index: 0,
        },
        Event::Check {
            field: "ancho",
            on: true,
        },
        Event::Check {
            field: "modo",
            on: true,
        },
        Event::Hex {
            field: "ancho",
            text: "#fff".into(),
        },
        Event::HexConfirm {
            field: "ancho",
            blur: false,
        },
        Event::HexRevert { field: "ancho" },
        Event::Number {
            field: "informe",
            text: "1".into(),
        },
    ];
    for event in invalid_2 {
        let mut d = base.clone();
        assert_eq!(d.event(event.clone()), Reaction::None, "{event:?}");
        assert_eq!(d, base, "el evento {event:?} cambió el estado");
    }
}

#[test]
fn row_4_accept_valid_delivers_what_was_confirmed_and_closes() {
    let mut modal = modal_open(all_kinds());
    modal.event(Event::Number {
        field: "ancho",
        text: "100".into(),
    });
    modal.event(Event::Choice {
        field: "modo",
        index: 1,
    });
    modal.event(Event::Check {
        field: "opaco",
        on: true,
    });
    let before = dialog_of(&modal).values().expect("válido");
    let Reaction::Accepted(v) = modal.event(Event::Accept) else {
        panic!("se esperaba Accepted");
    };
    assert_eq!(v, before);
    assert_eq!(v.number("ancho"), Some(100));
    assert_eq!(v.number("alto"), Some(64));
    assert_eq!(v.choice("modo"), Some(1));
    assert_eq!(v.check("opaco"), Some(true));
    // El hexadecimal estaba oculto: no se entrega.
    assert_eq!(v.get("color"), None);
    assert!(!modal.is_open());
    assert!(matches!(modal, Modal::Closed));
    assert_eq!(modal.kind(), None);
}

#[test]
fn row_4_accept_does_not_deliver_hidden_fields_even_if_they_have_something_written() {
    let mut modal = modal_open(all_kinds());
    modal.event(Event::Choice {
        field: "modo",
        index: 2,
    });
    modal.event(Event::Hex {
        field: "color",
        text: "#abc".into(),
    });
    modal.event(Event::HexConfirm {
        field: "color",
        blur: false,
    });
    modal.event(Event::Choice {
        field: "modo",
        index: 0,
    });
    let Reaction::Accepted(v) = modal.event(Event::Accept) else {
        panic!("se esperaba Accepted");
    };
    assert_eq!(v.get("color"), None);
    assert_eq!(v.choice("modo"), Some(0));
}

#[test]
fn row_4_accept_with_a_visible_hexadecimal_gives_the_canonical_color() {
    let mut modal = modal_open(all_kinds());
    modal.event(Event::Choice {
        field: "modo",
        index: 2,
    });
    let Reaction::Accepted(v) = modal.event(Event::Accept) else {
        panic!("se esperaba Accepted");
    };
    assert_eq!(v.color("color"), Some(Rgba::opaque(0x33, 0x66, 0x99)));
}

#[test]
fn row_4_accept_invalid_gives_rejected_with_the_reason_without_changing_or_closing() {
    let mut modal = modal_open(all_kinds());
    modal.event(Event::Number {
        field: "ancho",
        text: "".into(),
    });
    let before = dialog_of(&modal).clone();
    let reason_2 = before.reason().expect("inválido");
    assert_eq!(modal.event(Event::Accept), Reaction::Rejected(reason_2));
    assert!(modal.is_open());
    assert_eq!(dialog_of(&modal), &before);
    // También con el `check` que falla.
    let mut modal = modal_open(all_kinds());
    modal.event(Event::Number {
        field: "ancho",
        text: "8192".into(),
    });
    modal.event(Event::Number {
        field: "alto",
        text: "8192".into(),
    });
    assert_eq!(
        modal.event(Event::Accept),
        Reaction::Rejected("demasiado grande".to_string())
    );
    assert!(modal.is_open());
}

#[test]
fn row_4_cancel_gives_cancelled_and_never_values() {
    let mut modal = modal_open(all_kinds());
    modal.event(Event::Number {
        field: "ancho",
        text: "100".into(),
    });
    assert_eq!(modal.event(Event::Cancel), Reaction::Cancelled);
    assert!(!modal.is_open());
    // Con el formulario inválido también.
    let mut modal = modal_open(all_kinds());
    modal.event(Event::Number {
        field: "ancho",
        text: "".into(),
    });
    assert_eq!(modal.event(Event::Cancel), Reaction::Cancelled);
    assert!(!modal.is_open());
}

#[test]
fn row_4_after_closing_everything_gives_none() {
    for close in [Event::Accept, Event::Cancel] {
        let mut modal = modal_open(all_kinds());
        let first = modal.event(close.clone());
        assert!(matches!(first, Reaction::Accepted(_) | Reaction::Cancelled));
        assert!(!modal.is_open());
        for event in [
            Event::Accept,
            Event::Cancel,
            Event::Number {
                field: "ancho",
                text: "5".into(),
            },
            Event::Step {
                field: "ancho",
                delta: 1,
            },
            Event::Choice {
                field: "modo",
                index: 1,
            },
            Event::Check {
                field: "opaco",
                on: true,
            },
            Event::Hex {
                field: "color",
                text: "#fff".into(),
            },
            Event::HexConfirm {
                field: "color",
                blur: true,
            },
            Event::HexRevert { field: "color" },
        ] {
            assert_eq!(modal.event(event.clone()), Reaction::None, "{event:?}");
            assert!(!modal.is_open());
        }
        assert!(!modal.abandon());
        assert!(!modal.close_kind(DialogKind::Licenses));
    }
}

#[test]
fn row_4_abandon_gives_true_and_then_false_without_a_reaction() {
    let mut modal = modal_open(all_kinds());
    modal.event(Event::Number {
        field: "ancho",
        text: "100".into(),
    });
    assert!(modal.abandon());
    assert!(!modal.is_open());
    assert!(!modal.abandon());
    assert_eq!(modal.event(Event::Accept), Reaction::None);
}

#[test]
fn row_4_it_can_be_reopened_after_closing_with_a_clean_state() {
    let mut modal = modal_open(all_kinds());
    modal.event(Event::Number {
        field: "ancho",
        text: "100".into(),
    });
    assert!(modal.abandon());
    assert!(modal.open(DialogKind::Licenses, all_kinds(), Locks::default()));
    assert_eq!(dialog_of(&modal).text("ancho"), Some("64".to_string()));
}

#[test]
fn row_4_close_kind_closes_the_open_kind() {
    let mut modal = modal_open(licenses_form("texto"));
    assert!(modal.close_kind(DialogKind::Licenses));
    assert!(!modal.is_open());
    assert!(!modal.close_kind(DialogKind::Licenses));
}

#[test]
fn row_4_hexadecimal_writing_does_not_confirm() {
    let def = Rgba::new(0x33, 0x66, 0x99, 0x80);
    let mut d = open(hex_form(Mode::Rgba, def));
    assert_eq!(d.text("color"), Some("#33669980".to_string()));
    assert!(!d.hex_pending("color"));
    hex(&mut d, "color", "#abc");
    assert_eq!(d.text("color"), Some("#abc".to_string()));
    assert!(d.hex_pending("color"));
    // El pendiente es válido: el conjunto lo es.
    assert!(d.is_valid());
    assert_eq!(d.reason(), None);
}

#[test]
fn row_4_hexadecimal_enter_valid_confirms_and_shows_hex_text() {
    let def = Rgba::new(0x33, 0x66, 0x99, 0x80);
    let mut d = open(hex_form(Mode::Rgba, def));
    hex(&mut d, "color", "#abc");
    confirms(&mut d, "color", false);
    // 3 dígitos: conserva el alfa confirmado (0x80).
    assert_eq!(d.text("color"), Some("#aabbcc80".to_string()));
    assert!(!d.hex_pending("color"));
    assert!(d.is_valid());
    assert_eq!(color_accepted(&d), Rgba::new(0xaa, 0xbb, 0xcc, 0x80));
}

#[test]
fn row_4_hexadecimal_enter_invalid_keeps_the_text_and_the_error() {
    let def = Rgba::new(0x33, 0x66, 0x99, 0x80);
    let mut d = open(hex_form(Mode::Rgba, def));
    hex(&mut d, "color", "zz");
    let reason_2 = d.reason();
    assert!(reason_2.is_some());
    confirms(&mut d, "color", false);
    assert_eq!(d.text("color"), Some("zz".to_string()));
    assert!(d.hex_pending("color"));
    assert!(!d.is_valid());
    assert_eq!(d.reason(), reason_2);
    // Y sigue sin ser válido el conjunto: Aceptar se rechaza.
    assert!(matches!(d.event(Event::Accept), Reaction::Rejected(_)));
}

#[test]
fn row_4_hexadecimal_blur_valid_confirms() {
    let def = Rgba::new(0x33, 0x66, 0x99, 0x80);
    let mut d = open(hex_form(Mode::Rgba, def));
    hex(&mut d, "color", "#abc");
    confirms(&mut d, "color", true);
    assert_eq!(d.text("color"), Some("#aabbcc80".to_string()));
    assert!(!d.hex_pending("color"));
    assert_eq!(color_accepted(&d), Rgba::new(0xaa, 0xbb, 0xcc, 0x80));
}

#[test]
fn row_4_hexadecimal_blur_invalid_reverts_to_the_last_confirmed() {
    let def = Rgba::new(0x33, 0x66, 0x99, 0x80);
    let mut d = open(hex_form(Mode::Rgba, def));
    hex(&mut d, "color", "zz");
    confirms(&mut d, "color", true);
    assert_eq!(d.text("color"), Some("#33669980".to_string()));
    assert!(!d.hex_pending("color"));
    assert!(d.is_valid());
    assert_eq!(d.reason(), None);
    assert_eq!(color_accepted(&d), def);
    // Revierte al último **confirmado**, no al valor por defecto.
    hex(&mut d, "color", "#123456");
    confirms(&mut d, "color", false);
    assert_eq!(d.text("color"), Some("#12345680".to_string()));
    hex(&mut d, "color", "nope");
    confirms(&mut d, "color", true);
    assert_eq!(d.text("color"), Some("#12345680".to_string()));
}

#[test]
fn row_4_hexadecimal_revert_returns_to_the_confirmed() {
    let def = Rgba::new(0x33, 0x66, 0x99, 0x80);
    for text_2 in ["zz", "#abc", ""] {
        let mut d = open(hex_form(Mode::Rgba, def));
        hex(&mut d, "color", text_2);
        assert_eq!(d.event(Event::HexRevert { field: "color" }), Reaction::None);
        assert_eq!(d.text("color"), Some("#33669980".to_string()), "{text_2:?}");
        assert!(!d.hex_pending("color"), "{text_2:?}");
        assert!(d.is_valid(), "{text_2:?}");
    }
}

#[test]
fn row_4_hexadecimal_three_or_six_digits_keep_the_alpha_and_four_or_eight_the_written_one() {
    let def = Rgba::new(0x33, 0x66, 0x99, 0x80);
    let cases = [
        ("#f00", Rgba::new(0xff, 0, 0, 0x80)),
        ("#ff0000", Rgba::new(0xff, 0, 0, 0x80)),
        ("f00", Rgba::new(0xff, 0, 0, 0x80)),
        ("#f008", Rgba::new(0xff, 0, 0, 0x88)),
        ("#ff000040", Rgba::new(0xff, 0, 0, 0x40)),
        ("#ff0000ff", Rgba::new(0xff, 0, 0, 0xff)),
    ];
    for (text_2, expected_2) in cases {
        let mut d = open(hex_form(Mode::Rgba, def));
        hex(&mut d, "color", text_2);
        confirms(&mut d, "color", false);
        assert_eq!(color_accepted(&d), expected_2, "{text_2:?}");
        assert_eq!(d.text("color"), Some(expected_2.to_string()), "{text_2:?}");
    }
    // El alfa que se conserva es el **confirmado**, no el de partida.
    let mut d = open(hex_form(Mode::Rgba, def));
    hex(&mut d, "color", "#f008");
    confirms(&mut d, "color", false);
    hex(&mut d, "color", "#0f0");
    confirms(&mut d, "color", false);
    assert_eq!(color_accepted(&d), Rgba::new(0, 0xff, 0, 0x88));
    hex(&mut d, "color", "#00ff00");
    confirms(&mut d, "color", true);
    assert_eq!(color_accepted(&d), Rgba::new(0, 0xff, 0, 0x88));
    // Un color opaco se muestra sin los dígitos del alfa.
    let mut d = open(hex_form(Mode::Rgba, Rgba::opaque(1, 2, 3)));
    hex(&mut d, "color", "#f00");
    confirms(&mut d, "color", false);
    assert_eq!(d.text("color"), Some("#ff0000".to_string()));
}

#[test]
fn row_4_accept_with_a_valid_pending_one_confirms_it_before_delivering() {
    let def = Rgba::new(0x33, 0x66, 0x99, 0x80);
    let mut d = open(hex_form(Mode::Rgba, def));
    hex(&mut d, "color", "#abc");
    let Reaction::Accepted(v) = d.event(Event::Accept) else {
        panic!("se esperaba Accepted");
    };
    assert_eq!(v.color("color"), Some(Rgba::new(0xaa, 0xbb, 0xcc, 0x80)));
}

#[test]
fn row_4_accept_with_an_invalid_pending_one_is_rejected_without_changing_anything() {
    let def = Rgba::new(0x33, 0x66, 0x99, 0x80);
    let mut d = open(hex_form(Mode::Rgba, def));
    hex(&mut d, "color", "zz");
    let before = d.clone();
    let reason_2 = d.reason().expect("inválido");
    assert_eq!(d.event(Event::Accept), Reaction::Rejected(reason_2));
    assert_eq!(d, before);
    assert_eq!(d.text("color"), Some("zz".to_string()));
    assert!(d.hex_pending("color"));
    // Lo mismo a través del `Modal`: sigue abierto con el pendiente.
    let mut modal = modal_open(hex_form(Mode::Rgba, def));
    modal.event(Event::Hex {
        field: "color",
        text: "zz".into(),
    });
    assert!(matches!(modal.event(Event::Accept), Reaction::Rejected(_)));
    assert!(modal.is_open());
    assert_eq!(dialog_of(&modal).text("color"), Some("zz".to_string()));
}

#[test]
fn row_4_writing_in_a_hexadecimal_does_not_touch_the_other_fields() {
    let mut d = dialog();
    chooses(&mut d, "modo", 2);
    number(&mut d, "ancho", "10");
    hex(&mut d, "color", "#123");
    assert_eq!(d.text("ancho"), Some("10".to_string()));
    assert_eq!(d.text("alto"), Some("64".to_string()));
    assert_eq!(d.values().expect("válido").choice("modo"), Some(2));
}

// ---------------------------------------------------------------------------------------------
// take_focus_request
// ---------------------------------------------------------------------------------------------

#[test]
fn row_4_take_focus_request_gives_some_once_after_new_and_then_none() {
    let mut d = dialog();
    let initial = initial_focus(&d);
    assert_eq!(d.take_focus_request(), Some(initial));
    assert_eq!(d.take_focus_request(), None);
    assert_eq!(d.take_focus_request(), None);
    // Los eventos no vuelven a pedirlo.
    number(&mut d, "ancho", "5");
    chooses(&mut d, "modo", 2);
    assert_eq!(d.take_focus_request(), None);
    // Un diálogo nuevo vuelve a pedirlo.
    let mut other_2 = dialog();
    assert!(other_2.take_focus_request().is_some());
}

#[test]
fn row_4_the_requested_initial_focus_is_the_first_field_or_the_area_in_licenses() {
    let mut d = dialog();
    assert_eq!(d.take_focus_request(), Some(FocusTarget::Field("ancho")));
    let mut l = open(licenses_form("texto"));
    assert_eq!(
        l.take_focus_request(),
        Some(FocusTarget::Zone(LICENSES_FIELD))
    );
    assert_eq!(l.take_focus_request(), None);
}

#[test]
fn row_4_take_focus_request_through_the_modal() {
    let mut modal = modal_open(all_kinds());
    let Modal::Open(d) = &mut modal else {
        panic!("abierto");
    };
    assert_eq!(d.take_focus_request(), Some(FocusTarget::Field("ancho")));
    assert_eq!(d.take_focus_request(), None);
}

// ---------------------------------------------------------------------------------------------
// Propiedad 2: el estado nunca se corrompe
// ---------------------------------------------------------------------------------------------

/// Lo que el modelo, escrito aparte, sabe del diálogo. Se indexa por posición del campo.
struct Model {
    choices: Vec<usize>,
    checks: Vec<bool>,
    numbers: Vec<String>,
    /// Campos a los que llegó un evento mientras estaban ocultos: la spec no dice qué pasa.
    tainted: Vec<bool>,
}

impl Model {
    fn new(spec: &FormSpec) -> Model {
        let mut m = Model {
            choices: vec![0; spec.fields.len()],
            checks: vec![false; spec.fields.len()],
            numbers: vec![String::new(); spec.fields.len()],
            tainted: vec![false; spec.fields.len()],
        };
        for (i, f) in spec.fields.iter().enumerate() {
            match &f.kind {
                FieldKind::Number { default, .. } => m.numbers[i] = default.to_string(),
                FieldKind::Choice { default, .. } => m.choices[i] = *default,
                FieldKind::Check { default } => m.checks[i] = *default,
                _ => {}
            }
        }
        m
    }

    fn visible(&self, spec: &FormSpec, i: usize) -> bool {
        match &spec.fields[i].shown_when {
            None => true,
            Some(sw) => {
                let j = spec
                    .fields
                    .iter()
                    .position(|f| f.id == sw.field)
                    .expect("shown_when válido");
                self.choices[j] == sw.choice
            }
        }
    }

    fn visible_2(&self, spec: &FormSpec) -> Vec<bool> {
        (0..spec.fields.len())
            .map(|i| self.visible(spec, i))
            .collect()
    }

    /// Aplica el evento al modelo. Devuelve `(field_2 touched, debe ignorarse)`.
    fn applies(&mut self, spec: &FormSpec, event: &Event) -> (Option<usize>, bool) {
        let idx = |field: &str| spec.fields.iter().position(|f| f.id == field);
        match event {
            Event::Number { field, text } => match idx(field) {
                Some(i) if matches!(spec.fields[i].kind, FieldKind::Number { .. }) => {
                    if self.visible(spec, i) {
                        self.numbers[i] = bounded_text(text);
                    } else {
                        self.tainted[i] = true;
                    }
                    (Some(i), false)
                }
                other => (other, true),
            },
            Event::Step { field, delta } => match idx(field) {
                Some(i) => match spec.fields[i].kind {
                    FieldKind::Number { min, max, default } => {
                        if self.visible(spec, i) {
                            self.numbers[i] =
                                step_number(&self.numbers[i], min, max, default, *delta);
                        } else {
                            self.tainted[i] = true;
                        }
                        (Some(i), false)
                    }
                    _ => (Some(i), true),
                },
                None => (None, true),
            },
            Event::Choice { field, index } => match idx(field) {
                Some(i) => match spec.fields[i].kind {
                    FieldKind::Choice { options, .. } if *index < options.len() => {
                        if self.visible(spec, i) {
                            self.choices[i] = *index;
                        } else {
                            self.tainted[i] = true;
                        }
                        (Some(i), false)
                    }
                    _ => (Some(i), true),
                },
                None => (None, true),
            },
            Event::Check { field, on } => match idx(field) {
                Some(i) if matches!(spec.fields[i].kind, FieldKind::Check { .. }) => {
                    if self.visible(spec, i) {
                        self.checks[i] = *on;
                    } else {
                        self.tainted[i] = true;
                    }
                    (Some(i), false)
                }
                other => (other, true),
            },
            Event::Hex { field, .. }
            | Event::HexConfirm { field, .. }
            | Event::HexRevert { field } => match idx(field) {
                Some(i) if matches!(spec.fields[i].kind, FieldKind::Hex { .. }) => (Some(i), false),
                other => (other, true),
            },
            Event::Accept | Event::Cancel | Event::Press { .. } => (None, false),
        }
    }
}

/// Lo que tiene que cumplir un diálogo abierto tras cualquier secuencia de eventos.
fn checks_state(d: &Dialog, spec: &FormSpec, m: &Model) -> Result<(), TestCaseError> {
    let n = spec.fields.len();
    let vis = m.visible_2(spec);
    let mut first_invalid: Option<(usize, String)> = None;
    let mut sum: u64 = 0;
    for i in 0..n {
        let id = IDS[i];
        prop_assert_eq!(d.is_shown(id), vis[i], "is_shown de {}", id);
        let validity = d.validity(id);
        let invalid = match &validity {
            Some(Validity::Invalid(reason_2)) => Some(reason_2.clone()),
            _ => None,
        };
        match &spec.fields[i].kind {
            FieldKind::Number { min, max, .. } => {
                let text_2 = d.text(id).expect("un numérico siempre tiene texto");
                prop_assert!(text_2.chars().count() <= 11, "texto numérico {:?}", text_2);
                if !m.tainted[i] {
                    prop_assert_eq!(&text_2, &m.numbers[i], "texto de {}", id);
                }
                let analysis = parse_number(&text_2, *min, *max);
                if vis[i] {
                    prop_assert_eq!(analysis.is_err(), invalid.is_some(), "validez de {}", id);
                    if let Ok(v) = analysis {
                        sum += u64::from(v);
                    }
                }
            }
            FieldKind::Hex { .. } => {
                prop_assert!(d.text(id).is_some());
                if vis[i] && !d.hex_pending(id) {
                    prop_assert!(invalid.is_none(), "un hexadecimal sin pendiente es válido");
                }
            }
            _ => {
                if vis[i] {
                    prop_assert!(invalid.is_none());
                }
            }
        }
        if vis[i]
            && first_invalid.is_none()
            && let Some(reason_2) = invalid
        {
            first_invalid = Some((i, reason_2));
        }
    }
    // Los ids que no existen nunca son nada.
    for id in IDS.iter().skip(n).chain(["zz"].iter()).copied() {
        prop_assert!(!d.is_shown(id));
        prop_assert!(d.validity(id).is_none());
        prop_assert!(d.text(id).is_none());
        prop_assert!(!d.hex_pending(id));
    }

    let check_fails = spec.check.is_some() && first_invalid.is_none() && sum % 2 == 1;
    let expected_valid = first_invalid.is_none() && !check_fails;
    prop_assert_eq!(d.is_valid(), expected_valid);
    let reason = d.reason();
    prop_assert_eq!(reason.is_none(), expected_valid);
    let values = d.values();
    prop_assert_eq!(values.is_some(), expected_valid);
    if let Some(r) = &reason {
        prop_assert!(r.chars().count() <= 37, "motivo largo {:?}", r);
        prop_assert!(in_the_set(r), "motivo fuera del juego {:?}", r);
    }
    if let Some((i, reason_2)) = &first_invalid {
        let r = reason.expect("hay un campo inválido");
        prop_assert!(
            r.starts_with(LABELS[*i]),
            "{:?} no empieza por {:?}",
            r,
            LABELS[*i]
        );
        prop_assert!(
            r.contains(reason_2.as_str()),
            "{:?} no contiene {:?}",
            r,
            reason_2
        );
    } else if check_fails {
        prop_assert_eq!(reason, Some(SUM_ODD.to_string()));
    }

    if let Some(v) = values {
        for i in 0..n {
            let id = IDS[i];
            let there_is = vis[i] && !matches!(spec.fields[i].kind, FieldKind::ReadOnly { .. });
            prop_assert_eq!(v.get(id).is_some(), there_is, "{} en los valores", id);
            match &spec.fields[i].kind {
                FieldKind::Choice { options, .. } if vis[i] => {
                    let c = v.choice(id).expect("la opción sale en los valores");
                    prop_assert!(c < options.len(), "opción {} de {}", c, options.len());
                    if !m.tainted[i] {
                        prop_assert_eq!(c, m.choices[i]);
                    }
                }
                FieldKind::Check { .. } if vis[i] && !m.tainted[i] => {
                    prop_assert_eq!(v.check(id), Some(m.checks[i]));
                }
                FieldKind::Number { min, max, .. } if vis[i] => {
                    let expected_2 = parse_number(&d.text(id).unwrap(), *min, *max).ok();
                    prop_assert_eq!(v.number(id), expected_2);
                }
                FieldKind::Hex { .. } if vis[i] => {
                    prop_assert!(v.color(id).is_some());
                }
                _ => {}
            }
        }
        for id in IDS.iter().skip(n).copied() {
            prop_assert!(v.get(id).is_none());
        }
    }
    Ok(())
}

/// Los valores de dos `Values` son iguales salvo en los hexadecimales con pendiente.
fn values_equal(a: &Values, b: &Values, pending: &[bool]) -> bool {
    IDS.iter()
        .enumerate()
        .all(|(i, id)| pending.get(i).copied().unwrap_or(false) || a.get(id) == b.get(id))
}

fn runs(recipe: &FormRecipe, events: &[EventRecipe]) -> Result<(), TestCaseError> {
    let spec = build(recipe);
    prop_assert!(spec.validate().is_ok(), "la spec al azar no valida");
    let n = spec.fields.len();
    let mut model = Model::new(&spec);
    let mut modal = Modal::default();
    prop_assert!(modal.open(DialogKind::Licenses, build(recipe), Locks::default()));
    checks_state(dialog_of(&modal), &spec, &model)?;

    let mut open_2_2 = true;
    for r in events {
        let event = event_of(r);
        if !open_2_2 {
            // Tras cerrar, todo da `None`.
            match event {
                Some(e) => prop_assert_eq!(modal.event(e), Reaction::None),
                None => prop_assert!(!modal.abandon()),
            }
            prop_assert!(!modal.is_open());
            continue;
        }
        let before = dialog_of(&modal).clone();
        let valid_before = before.is_valid();
        let reason_before = before.reason();
        let values_before = before.values();
        let texts_before: Vec<Option<String>> = IDS.iter().map(|id| before.text(id)).collect();
        let pending_before: Vec<bool> = IDS
            .iter()
            .take(n)
            .map(|id| before.hex_pending(id))
            .collect();
        let visible_before = model.visible_2(&spec);

        let Some(event) = event else {
            prop_assert_eq!(r.kind, KIND_ABANDON);
            prop_assert!(modal.abandon());
            prop_assert!(!modal.is_open());
            prop_assert!(!modal.abandon());
            open_2_2 = false;
            continue;
        };

        let (touched, ignored) = model.applies(&spec, &event);
        let reaction = modal.event(event.clone());
        match (&event, reaction) {
            (Event::Accept, Reaction::Accepted(v)) => {
                prop_assert!(valid_before, "Accepted con el conjunto inválido");
                let before_v = values_before.expect("válido: había valores");
                prop_assert!(values_equal(&v, &before_v, &pending_before));
                for i in 0..n {
                    let there_is = visible_before[i]
                        && !matches!(spec.fields[i].kind, FieldKind::ReadOnly { .. });
                    prop_assert_eq!(
                        v.get(IDS[i]).is_some(),
                        there_is,
                        "{} en lo aceptado",
                        IDS[i]
                    );
                }
                prop_assert!(!modal.is_open());
                open_2_2 = false;
            }
            (Event::Accept, Reaction::Rejected(reason_2)) => {
                prop_assert!(!valid_before, "Rejected con el conjunto válido");
                prop_assert_eq!(Some(reason_2), reason_before);
                prop_assert!(modal.is_open());
                prop_assert_eq!(dialog_of(&modal), &before);
            }
            (Event::Cancel, Reaction::Cancelled) => {
                prop_assert!(!modal.is_open());
                open_2_2 = false;
            }
            (Event::Accept | Event::Cancel, other) => {
                prop_assert!(false, "{:?} dio {:?}", event, other);
            }
            (_, Reaction::None) => {
                prop_assert!(modal.is_open());
                if ignored {
                    prop_assert_eq!(
                        dialog_of(&modal),
                        &before,
                        "un evento ignorado cambió el estado"
                    );
                }
                // Un evento solo cambia el texto de su campo.
                let d = dialog_of(&modal);
                for (i, id) in IDS.iter().copied().enumerate() {
                    if Some(i) != touched {
                        prop_assert_eq!(
                            d.text(id),
                            texts_before[i].clone(),
                            "{} cambió sin evento",
                            id
                        );
                    }
                }
            }
            (_, other) => {
                prop_assert!(false, "{:?} dio {:?}", event, other);
            }
        }
        if open_2_2 {
            checks_state(dialog_of(&modal), &spec, &model)?;
        }
    }

    // Al final, si sigue abierto: abandonar no entrega nada y luego todo da `None`.
    if open_2_2 {
        prop_assert!(modal.abandon());
    }
    prop_assert!(!modal.abandon());
    prop_assert_eq!(modal.event(Event::Accept), Reaction::None);
    prop_assert_eq!(modal.event(Event::Cancel), Reaction::None);
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn property_2_the_state_is_never_corrupted(
        recipe in form_recipe(),
        events in prop::collection::vec(event_recipe(), 0..=64),
    ) {
        runs(&recipe, &events)?;
    }

    /// `target_id` y `build` son coherentes entre sí (guarda de la propia prueba): los ids de la
    /// spec son los de `IDS` y no se repiten.
    #[test]
    fn property_2_random_forms_are_valid(recipe in form_recipe()) {
        let spec = build(&recipe);
        prop_assert!(spec.validate().is_ok());
        let ids: HashSet<_> = spec.fields.iter().map(|f| f.id).collect();
        prop_assert_eq!(ids.len(), spec.fields.len());
        prop_assert!(Dialog::new(DialogKind::Licenses, spec).is_ok());
        prop_assert_eq!(target_id(&EventRecipe { kind: 0, id: 99, text: 0, index: 0, flag: false }), "zz");
    }
}

/// Los campos ocultos conservan lo escrito aunque se cambie la opción varias veces.
#[test]
fn property_2_the_hidden_ones_keep_what_was_written_in_a_fixed_sequence() {
    let mut d = dialog();
    chooses(&mut d, "modo", 2);
    hex(&mut d, "color", "#12345");
    for index in [0, 1, 2, 0, 1, 0] {
        chooses(&mut d, "modo", index);
        assert_eq!(
            d.text("color"),
            Some("#12345".to_string()),
            "opción {index}"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Unidad 19 (specs/hito-2-nueva-imagen.md, D9 y fila 12): tres ampliaciones aditivas del motor
// ---------------------------------------------------------------------------------------------
//
// - E1: `shown_when` transitivo (un campo con el control oculto está oculto aunque el control
//   conserve la opción que lo muestra).
// - E2: `Dialog::set_report(id, text_2) -> bool`, el texto de un campo `ReadOnly`.
// - E3: `Dialog::set_limit_note(id, nota) -> bool`, el `Above { max }` de un numérico pasa a decir
//   `máximo N (nota)`.
// - Más `DialogKind::NewImage`.
//
// Ninguna prueba anterior de este fichero cambia (esta sección solo añade). Suposición: el texto
// de un campo `ReadOnly` se lee de `Dialog::spec()` (el `FieldKind::ReadOnly { text, .. }` del
// campo): `set_report` lo cambia ahí, que es donde `content_h_u` lo mide. Se usan los ayudantes de
// arriba (`open`, `field_2`, `numeric`, `spec_with`, `number`, `chooses`, `hex`).

const MODES_19: &[&str] = &["Uno", "Dos", "Tres"];
const BACKGROUNDS_19: &[&str] = &["Transparente", "Blanco", "Negro", "Color…"];

fn field_if(
    id: &'static str,
    label: &'static str,
    kind: FieldKind,
    control: &'static str,
    option: usize,
) -> FieldSpec {
    FieldSpec {
        id,
        label,
        kind,
        shown_when: Some(ShowWhen {
            field: control,
            choice: option,
        }),
    }
}

/// `mode_2` (por defecto la opción 1) → `background_2` (se ve con `mode_2` = 1) → `color` (se ve con
/// `background_2` = 3) → `report`: la cadena de dos niveles del formulario de nueva imagen.
fn spec_in_string() -> FormSpec {
    spec_with(vec![
        field_2(
            "modo",
            "Modo",
            FieldKind::Choice {
                options: MODES_19,
                default: 1,
            },
        ),
        field_if(
            "fondo",
            "Fondo",
            FieldKind::Choice {
                options: BACKGROUNDS_19,
                default: 0,
            },
            "modo",
            1,
        ),
        field_if(
            "color",
            "Color",
            FieldKind::Hex {
                mode: Mode::Rgba,
                default: Rgba::opaque(0x80, 0x80, 0x80),
            },
            "fondo",
            3,
        ),
        field_2(
            "informe",
            "Informe",
            FieldKind::ReadOnly {
                text: "uno".to_string(),
                rows: Rows::Fixed(4),
            },
        ),
    ])
}

fn in_string() -> Dialog {
    open(spec_in_string())
}

// --- E1 ---

#[test]
fn row_12_e1_the_example_string_is_a_valid_spec() {
    spec_in_string().validate().expect("spec válida");
    assert!(Dialog::new(DialogKind::NewImage, spec_in_string()).is_ok());
}

#[test]
fn row_12_e1_a_field_with_the_control_hidden_is_hidden_even_if_the_control_is_worth_the_option() {
    let mut d = in_string();
    // Inicio: modo = 1 → fondo se ve; fondo = 0 → color no.
    assert!(d.is_shown("fondo"));
    assert!(!d.is_shown("color"));
    chooses(&mut d, "fondo", 3);
    assert!(d.is_shown("color"), "fondo = 3 con el modo que lo muestra");

    // Se oculta el control (`background_2`): `color` también, aunque `background_2` siga valiendo 3.
    chooses(&mut d, "modo", 0);
    assert!(!d.is_shown("fondo"));
    assert!(
        !d.is_shown("color"),
        "el control de color (fondo) está oculto: color también"
    );

    // Y vuelven juntos al volver el modo.
    chooses(&mut d, "modo", 1);
    assert!(d.is_shown("fondo"));
    assert!(d.is_shown("color"), "fondo conservaba el 3");
}

#[test]
fn row_12_e1_the_visibility_of_each_combination_of_mode_and_background() {
    for mode_2 in 0..3 {
        for background_2 in 0..4 {
            let mut d = in_string();
            chooses(&mut d, "modo", mode_2);
            chooses(&mut d, "fondo", background_2);
            assert!(d.is_shown("modo"), "modo {mode_2}, fondo {background_2}");
            assert!(d.is_shown("informe"), "modo {mode_2}, fondo {background_2}");
            assert_eq!(
                d.is_shown("fondo"),
                mode_2 == 1,
                "modo {mode_2}, fondo {background_2}"
            );
            assert_eq!(
                d.is_shown("color"),
                mode_2 == 1 && background_2 == 3,
                "modo {mode_2}, fondo {background_2}"
            );
        }
    }
}

#[test]
fn row_12_e1_a_field_hidden_by_transitivity_keeps_what_was_written() {
    let mut d = in_string();
    chooses(&mut d, "fondo", 3);
    hex(&mut d, "color", "#12345");
    for mode_2 in [0, 2, 1, 0, 1] {
        chooses(&mut d, "modo", mode_2);
        assert_eq!(
            d.text("color"),
            Some("#12345".to_string()),
            "modo {mode_2}: lo escrito se conserva oculto"
        );
    }
    assert!(d.hex_pending("color"));
}

#[test]
fn row_12_e1_what_is_hidden_by_transitivity_does_not_count_in_validity_values_or_focus() {
    let mut d = in_string();
    chooses(&mut d, "fondo", 3);
    hex(&mut d, "color", "zz");
    assert!(!d.is_valid(), "con el hexadecimal visible e inválido");
    assert!(d.any_hex_pending());
    let height_visible = d.content_h_u();

    chooses(&mut d, "modo", 0);
    // Ya no cuenta: ni la validez, ni el pendiente, ni el motivo.
    assert!(d.is_valid(), "{:?}", d.reason());
    assert_eq!(d.reason(), None);
    assert!(!d.any_hex_pending());
    assert_eq!(d.validity("fondo"), None);
    assert_eq!(d.validity("color"), None);

    // Ni los valores, ni lo que entrega `Accept`.
    let values = d.values().expect("valores");
    assert_eq!(values.choice("modo"), Some(0));
    assert_eq!(values.get("fondo"), None);
    assert_eq!(values.get("color"), None);
    match d.clone().event(Event::Accept) {
        Reaction::Accepted(v) => {
            assert_eq!(v.get("fondo"), None);
            assert_eq!(v.get("color"), None);
        }
        r => panic!("se esperaba Accepted y salió {r:?}"),
    }

    // Ni el orden de foco, ni el alto del contenido.
    let order = px_ui::dialog_keys::focus_order(&d);
    for &goal in &order {
        match goal {
            FocusTarget::Field(id)
            | FocusTarget::Option(id, _)
            | FocusTarget::Minus(id)
            | FocusTarget::Plus(id)
            | FocusTarget::Zone(id)
            | FocusTarget::Button(id)
            | FocusTarget::ButtonAside(id) => {
                assert!(d.is_shown(id), "{goal:?} está oculto y está en el orden")
            }
            FocusTarget::Accept | FocusTarget::Cancel => {}
        }
    }
    assert!(!order.contains(&FocusTarget::Field("color")));
    assert!(!order.contains(&FocusTarget::Option("fondo", 0)));
    assert!(d.content_h_u() < height_visible);
}

#[test]
fn row_12_e1_a_chain_of_four_levels_is_hidden_whole_with_the_first_control() {
    const SI_NO: &[&str] = &["No", "Si"];
    let choice_2 = |default| FieldKind::Choice {
        options: SI_NO,
        default,
    };
    let spec = spec_with(vec![
        field_2("a", "A", choice_2(1)),
        field_if("b", "B", choice_2(1), "a", 1),
        field_if("c", "C", choice_2(1), "b", 1),
        field_if("d", "D", choice_2(1), "c", 1),
    ]);
    spec.validate().expect("spec válida");
    let mut d = open(spec);
    for id in ["a", "b", "c", "d"] {
        assert!(d.is_shown(id), "todo a 1: {id} se ve");
    }
    chooses(&mut d, "a", 0);
    assert!(d.is_shown("a"));
    for id in ["b", "c", "d"] {
        assert!(!d.is_shown(id), "a = 0 oculta {id} aunque valga 1");
    }
    chooses(&mut d, "a", 1);
    chooses(&mut d, "c", 0);
    assert!(d.is_shown("b") && d.is_shown("c"));
    assert!(!d.is_shown("d"), "c = 0 oculta d");
}

#[test]
fn row_12_e1_with_the_control_visible_everything_stays_as_before() {
    // El caso de un solo nivel de la unidad 18 no cambia: oculto solo si la opción no vale.
    let mut d = dialog();
    assert!(!d.is_shown("color"));
    chooses(&mut d, "modo", 2);
    assert!(d.is_shown("color"));
    chooses(&mut d, "modo", 1);
    assert!(!d.is_shown("color"));
}

#[test]
fn row_12_dialog_kind_new_image_opens_a_dialog_and_closes_by_its_kind() {
    let d = Dialog::new(DialogKind::NewImage, spec_in_string()).expect("spec válida");
    assert_eq!(d.kind(), DialogKind::NewImage);
    assert_ne!(DialogKind::NewImage, DialogKind::Licenses);
    assert_ne!(DialogKind::NewImage, DialogKind::Demo);

    let mut modal = Modal::default();
    assert!(modal.open(DialogKind::NewImage, spec_in_string(), Locks::default()));
    assert_eq!(modal.kind(), Some(DialogKind::NewImage));
    assert!(!modal.close_kind(DialogKind::Licenses), "es de otro tipo");
    assert!(modal.is_open());
    assert!(modal.close_kind(DialogKind::NewImage));
    assert!(!modal.is_open());
}

// --- E2 ---

/// El texto de un campo `ReadOnly`, leído de la spec del diálogo.
fn text_of_the_report_of(d: &Dialog, id: &str) -> Option<String> {
    d.spec().fields.iter().find_map(|f| match &f.kind {
        FieldKind::ReadOnly { text, .. } if f.id == id => Some(text.clone()),
        _ => None,
    })
}

/// Un informe con `lines` líneas «linea N».
fn lines(n: usize) -> String {
    (1..=n)
        .map(|i| format!("linea {i}"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn row_12_e2_set_report_changes_the_text_of_a_read_only() {
    let mut d = in_string();
    assert_eq!(
        text_of_the_report_of(&d, "informe"),
        Some("uno".to_string())
    );
    assert!(d.set_report("informe", "otro texto".to_string()));
    assert_eq!(
        text_of_the_report_of(&d, "informe"),
        Some("otro texto".to_string())
    );
    assert!(d.set_report("informe", String::new()));
    assert_eq!(text_of_the_report_of(&d, "informe"), Some(String::new()));
}

#[test]
fn row_12_e2_set_report_changes_the_content_height_with_the_number_of_lines() {
    let mut d = in_string();
    let a = d.content_h_u();
    // 14 u por fila hasta las 4 de `Rows::Fixed(4)`; una vacía cuenta como una línea.
    for (n, extra) in [(1, 0), (2, 14), (3, 28), (4, 42), (5, 42), (9, 42)] {
        assert!(d.set_report("informe", lines(n)));
        assert_eq!(d.content_h_u(), a + extra, "{n} líneas");
    }
    assert!(d.set_report("informe", String::new()));
    assert_eq!(d.content_h_u(), a, "vacío mide como una línea");
    assert!(d.set_report("informe", lines(3)));
    assert!(d.set_report("informe", "uno".to_string()));
    assert_eq!(d.content_h_u(), a, "vuelve al alto de una línea");
}

#[test]
fn row_12_e2_set_report_does_not_change_values_validity_or_the_other_fields() {
    let mut d = in_string();
    chooses(&mut d, "fondo", 3);
    hex(&mut d, "color", "#abc");
    let values = d.values();
    let reason_2 = d.reason();
    let color = d.text("color");
    let visible_2: Vec<bool> = ["modo", "fondo", "color", "informe"]
        .iter()
        .map(|id| d.is_shown(id))
        .collect();
    assert!(d.set_report("informe", lines(3)));
    assert_eq!(d.values(), values);
    assert_eq!(d.reason(), reason_2);
    assert_eq!(d.text("color"), color);
    let now: Vec<bool> = ["modo", "fondo", "color", "informe"]
        .iter()
        .map(|id| d.is_shown(id))
        .collect();
    assert_eq!(now, visible_2);
}

#[test]
fn row_12_e2_set_report_gives_false_if_the_field_does_not_exist_or_is_not_read_only_and_changes_nothing()
 {
    let mut d = open(spec_with(vec![
        numeric("ancho", "Ancho", 1, 100, 64),
        field_2(
            "modo",
            "Modo",
            FieldKind::Choice {
                options: MODES_19,
                default: 0,
            },
        ),
        field_2("opaco", "Opaco", FieldKind::Check { default: false }),
        field_2(
            "color",
            "Color",
            FieldKind::Hex {
                mode: Mode::Rgba,
                default: Rgba::opaque(1, 2, 3),
            },
        ),
        field_2(
            "informe",
            "Informe",
            FieldKind::ReadOnly {
                text: "uno".to_string(),
                rows: Rows::Fixed(2),
            },
        ),
    ]));
    let before = d.clone();
    for id in ["inexistente", "ancho", "modo", "opaco", "color", ""] {
        assert!(!d.set_report(id, "x".to_string()), "{id:?}");
        assert_eq!(d, before, "{id:?}: no cambió nada");
    }
    assert!(d.set_report("informe", "x".to_string()), "el ReadOnly sí");
    assert_ne!(d, before);
}

// --- E3 ---

fn two_numeric() -> Dialog {
    open(spec_with(vec![
        numeric("ancho", "Ancho", 1, 4096, 64),
        numeric("alto", "Alto", 1, 4096, 64),
        field_2(
            "modo",
            "Modo",
            FieldKind::Choice {
                options: MODES_19,
                default: 0,
            },
        ),
    ]))
}

fn invalid(text_2: &str) -> Option<Validity> {
    Some(Validity::Invalid(text_2.to_string()))
}

#[test]
fn row_12_e3_set_limit_note_changes_the_text_from_above_to_maximum_n_note() {
    let mut d = two_numeric();
    number(&mut d, "ancho", "4097");
    assert_eq!(d.validity("ancho"), invalid("máximo 4096"), "sin nota");
    assert_eq!(d.reason(), Some("Ancho: máximo 4096".to_string()));

    assert!(d.set_limit_note("ancho", Some("tarjeta gráfica")));
    assert_eq!(
        d.validity("ancho"),
        invalid("máximo 4096 (tarjeta gráfica)")
    );
    assert_eq!(
        d.reason(),
        Some("Ancho: máximo 4096 (tarjeta gráfica)".to_string())
    );

    assert!(d.set_limit_note("ancho", Some("otra nota")));
    assert_eq!(d.validity("ancho"), invalid("máximo 4096 (otra nota)"));
}

#[test]
fn row_12_e3_none_returns_the_usual_text_and_restores_the_dialog() {
    let original = two_numeric();
    let mut d = original.clone();
    assert!(d.set_limit_note("ancho", Some("tarjeta gráfica")));
    assert_ne!(d, original, "la nota se guarda");
    number(&mut d, "ancho", "4097");
    assert_eq!(
        d.validity("ancho"),
        invalid("máximo 4096 (tarjeta gráfica)")
    );
    assert!(d.set_limit_note("ancho", None));
    assert_eq!(d.validity("ancho"), invalid("máximo 4096"));
    number(&mut d, "ancho", "64");
    assert_eq!(
        d, original,
        "sin nota y con el texto inicial es el de partida"
    );
}

#[test]
fn row_12_e3_it_only_affects_the_requested_field() {
    let mut d = two_numeric();
    assert!(d.set_limit_note("ancho", Some("tarjeta gráfica")));
    number(&mut d, "alto", "4097");
    assert_eq!(
        d.validity("alto"),
        invalid("máximo 4096"),
        "alto no tiene nota"
    );
    assert!(d.set_limit_note("alto", Some("tarjeta gráfica")));
    assert_eq!(d.validity("alto"), invalid("máximo 4096 (tarjeta gráfica)"));
    assert_eq!(
        d.reason(),
        Some("Alto: máximo 4096 (tarjeta gráfica)".to_string())
    );
}

#[test]
fn row_12_e3_the_other_errors_do_not_change_with_a_note() {
    let mut d = two_numeric();
    assert!(d.set_limit_note("ancho", Some("tarjeta gráfica")));
    for (text_2, expected_2) in [
        ("0", invalid("mínimo 1")),
        ("", invalid("escribe un número")),
        ("abc", invalid("solo dígitos 0-9")),
        ("12345678901", invalid("máximo 10 cifras")),
        ("1", Some(Validity::Valid)),
        ("4096", Some(Validity::Valid)),
        ("4097", invalid("máximo 4096 (tarjeta gráfica)")),
        ("99999999", invalid("máximo 4096 (tarjeta gráfica)")),
    ] {
        number(&mut d, "ancho", text_2);
        assert_eq!(d.validity("ancho"), expected_2, "texto {text_2:?}");
    }
}

#[test]
fn row_12_e3_the_note_survives_the_field_events() {
    let mut d = two_numeric();
    assert!(d.set_limit_note("ancho", Some("tarjeta gráfica")));
    number(&mut d, "ancho", "5000");
    assert_eq!(
        d.validity("ancho"),
        invalid("máximo 4096 (tarjeta gráfica)")
    );
    // `+` y `−` desde un valor inválido dejan el valor por defecto (válido).
    assert_eq!(
        d.event(Event::Step {
            field: "ancho",
            delta: 1
        }),
        Reaction::None
    );
    assert_eq!(d.validity("ancho"), Some(Validity::Valid));
    number(&mut d, "ancho", "5000");
    assert_eq!(
        d.validity("ancho"),
        invalid("máximo 4096 (tarjeta gráfica)"),
        "la nota seguía puesta"
    );
}

#[test]
fn row_12_e3_it_does_not_change_the_height_or_the_values() {
    let mut d = two_numeric();
    let height_2 = d.content_h_u();
    let values = d.values();
    assert!(d.set_limit_note("ancho", Some("tarjeta gráfica")));
    assert_eq!(d.content_h_u(), height_2);
    assert_eq!(d.values(), values);
}

#[test]
fn row_12_e3_set_limit_note_gives_false_if_the_field_does_not_exist_or_is_not_numeric() {
    let mut d = open(spec_with(vec![
        numeric("ancho", "Ancho", 1, 4096, 64),
        field_2(
            "modo",
            "Modo",
            FieldKind::Choice {
                options: MODES_19,
                default: 0,
            },
        ),
        field_2("opaco", "Opaco", FieldKind::Check { default: false }),
        field_2(
            "color",
            "Color",
            FieldKind::Hex {
                mode: Mode::Rgba,
                default: Rgba::opaque(1, 2, 3),
            },
        ),
        field_2(
            "informe",
            "Informe",
            FieldKind::ReadOnly {
                text: "uno".to_string(),
                rows: Rows::Fixed(2),
            },
        ),
    ]));
    let before = d.clone();
    for id in ["inexistente", "modo", "opaco", "color", "informe", ""] {
        assert!(!d.set_limit_note(id, Some("nota")), "{id:?} con nota");
        assert!(!d.set_limit_note(id, None), "{id:?} sin nota");
        assert_eq!(d, before, "{id:?}: no cambió nada");
    }
    assert!(d.set_limit_note("ancho", Some("nota")), "el numérico sí");
}

// ---------------------------------------------------------------------------------------------
// Unidad 20 (specs/hito-2-convertir-modo.md, D6 y fila 12): dos ampliaciones aditivas del motor
// ---------------------------------------------------------------------------------------------
//
// - E1: `Dialog::set_block(Option<String>)` y `Dialog::block() -> Option<&str>`: un motivo externo,
//   recortado a 37 caracteres, que hace inválido el conjunto **después** de los campos y del
//   `check` (`reason()` los da en ese orden; `is_valid` y `values()` lo respetan; `Accept` con
//   motivo da `Rejected`; sin motivo, todo es como hoy).
// - E2: `Dialog::checked(id) -> Option<bool>` pasa de `pub(crate)` a `pub`.
// - Más `DialogKind::ConvertMode`.
//
// Ninguna prueba anterior de este fichero cambia (esta sección solo añade). Suposición: `block()`
// devuelve el motivo ya recortado a 37 caracteres (la spec dice «un motivo externo, recortado a
// 37»), y `set_block(None)` deja el diálogo `==` al de antes (el motivo forma parte de su estado).

fn check_20_forbids_the_seven(values: &Values) -> Option<String> {
    (values.number("ancho") == Some(7)).then(|| "el ancho no puede ser 7".to_string())
}

/// `width_2` (1 a 100, por defecto 64), una casilla `opaque`, un grupo `mode_2` y un `check` que
/// rechaza el ancho 7.
fn spec_20() -> FormSpec {
    FormSpec {
        check: Some(check_20_forbids_the_seven),
        ..spec_with(vec![
            numeric("ancho", "Ancho", 1, 100, 64),
            field_2("opaco", "Opaco", FieldKind::Check { default: false }),
            field_2(
                "modo",
                "Modo",
                FieldKind::Choice {
                    options: MODES_19,
                    default: 0,
                },
            ),
        ])
    }
}

fn dialog_20() -> Dialog {
    Dialog::new(DialogKind::ConvertMode, spec_20()).expect("spec válida")
}

fn box_value(d: &mut Dialog, field: &'static str, on: bool) {
    assert_eq!(d.event(Event::Check { field, on }), Reaction::None);
}

// --- E1: set_block y block ---

#[test]
fn row_12_set_block_without_a_reason_the_valid_dialog_has_no_block() {
    let d = dialog_20();
    assert_eq!(d.block(), None);
    assert_eq!(d.reason(), None);
    assert!(d.is_valid());
    assert!(d.values().is_some());
}

#[test]
fn row_12_set_block_with_a_reason_makes_a_valid_dialog_invalid() {
    let mut d = dialog_20();
    d.set_block(Some("no cabe".to_string()));
    assert_eq!(d.block(), Some("no cabe"));
    assert_eq!(d.reason(), Some("no cabe".to_string()));
    assert!(!d.is_valid());
    assert_eq!(d.values(), None);
}

#[test]
fn row_12_set_block_none_leaves_everything_as_before() {
    let before = dialog_20();
    let values = before.values();
    let mut d = before.clone();
    d.set_block(Some("no cabe".to_string()));
    assert!(!d.is_valid());
    d.set_block(None);
    assert_eq!(d.block(), None);
    assert_eq!(d.reason(), None);
    assert!(d.is_valid());
    assert_eq!(d.values(), values);
    assert_eq!(d, before, "el diálogo vuelve a ser el de antes");
    // Y `None` sobre un diálogo sin motivo no cambia nada.
    let mut other_2 = before.clone();
    other_2.set_block(None);
    assert_eq!(other_2, before);
}

#[test]
fn row_12_set_block_can_be_changed_and_the_last_reason_wins() {
    let mut d = dialog_20();
    d.set_block(Some("uno".to_string()));
    d.set_block(Some("dos".to_string()));
    assert_eq!(d.block(), Some("dos"));
    assert_eq!(d.reason(), Some("dos".to_string()));
}

#[test]
fn row_12_set_block_clips_to_37_characters() {
    for (text_2, expected_2) in [
        ("x".repeat(36), "x".repeat(36)),
        ("x".repeat(37), "x".repeat(37)),
        ("x".repeat(38), "x".repeat(37)),
        ("x".repeat(200), "x".repeat(37)),
        // Se cuentan caracteres, no bytes.
        ("ñ".repeat(50), "ñ".repeat(37)),
        ("…".repeat(80), "…".repeat(37)),
    ] {
        let mut d = dialog_20();
        d.set_block(Some(text_2.clone()));
        let reason_2 = d.reason().expect("hay motivo");
        assert_eq!(
            reason_2,
            expected_2,
            "texto de {} caracteres",
            text_2.chars().count()
        );
        assert_eq!(reason_2.chars().count(), expected_2.chars().count());
        assert!(reason_2.chars().count() <= 37);
        assert_eq!(
            d.block(),
            Some(expected_2.as_str()),
            "block() lo da recortado"
        );
        assert!(!d.is_valid());
    }
}

#[test]
fn row_12_reason_gives_first_the_invalid_field_then_the_check_and_last_the_external_reason() {
    let mut d = dialog_20();
    d.set_block(Some("motivo externo".to_string()));

    // El campo inválido gana al check y al motivo externo.
    number(&mut d, "ancho", "0");
    assert_eq!(d.reason(), Some("Ancho: mínimo 1".to_string()));
    assert_eq!(d.values(), None);

    // Con el campo válido pero con el ancho que rechaza el `check`: gana el `check`.
    number(&mut d, "ancho", "7");
    assert_eq!(d.validity("ancho"), Some(Validity::Valid));
    assert_eq!(d.reason(), Some("el ancho no puede ser 7".to_string()));
    assert_eq!(d.values(), None);

    // Con el `check` satisfecho queda el motivo externo.
    number(&mut d, "ancho", "8");
    assert_eq!(d.reason(), Some("motivo externo".to_string()));
    assert_eq!(d.values(), None);

    // Sin el motivo externo, todo vale.
    d.set_block(None);
    assert_eq!(d.reason(), None);
    assert!(d.values().is_some());
}

#[test]
fn row_12_the_external_reason_does_not_hide_the_error_of_a_field_or_the_check_one_when_removed() {
    let mut d = dialog_20();
    number(&mut d, "ancho", "7");
    d.set_block(Some("motivo externo".to_string()));
    assert_eq!(d.reason(), Some("el ancho no puede ser 7".to_string()));
    d.set_block(None);
    assert_eq!(d.reason(), Some("el ancho no puede ser 7".to_string()));
}

#[test]
fn row_12_accept_with_an_external_reason_gives_rejected_and_does_not_touch_the_state() {
    let mut d = dialog_20();
    number(&mut d, "ancho", "50");
    box_value(&mut d, "opaco", true);
    d.set_block(Some("no cabe".to_string()));
    let before = d.clone();
    assert_eq!(
        d.event(Event::Accept),
        Reaction::Rejected("no cabe".to_string())
    );
    assert_eq!(d, before, "un Accept rechazado no cambia nada");
    // Mientras haya motivo, sigue rechazando.
    assert_eq!(
        d.event(Event::Accept),
        Reaction::Rejected("no cabe".to_string())
    );
    // `Cancel` no depende del motivo.
    assert_eq!(d.event(Event::Cancel), Reaction::Cancelled);
}

#[test]
fn row_12_accept_without_an_external_reason_delivers_the_values_as_today() {
    let mut d = dialog_20();
    number(&mut d, "ancho", "50");
    d.set_block(Some("no cabe".to_string()));
    d.set_block(None);
    let expected_2 = d.values().expect("valores");
    match d.event(Event::Accept) {
        Reaction::Accepted(values) => {
            assert_eq!(values, expected_2);
            assert_eq!(values.number("ancho"), Some(50));
        }
        other => panic!("se esperaba Accepted y salió {other:?}"),
    }
}

#[test]
fn row_12_accept_with_a_reason_does_not_confirm_the_pending_hexadecimal() {
    let mut d = open(spec_with(vec![field_2(
        "color",
        "Color",
        FieldKind::Hex {
            mode: Mode::Rgba,
            default: Rgba::opaque(1, 2, 3),
        },
    )]));
    hex(&mut d, "color", "#ff0000");
    assert!(d.hex_pending("color"));
    d.set_block(Some("no cabe".to_string()));
    assert_eq!(
        d.event(Event::Accept),
        Reaction::Rejected("no cabe".to_string())
    );
    assert!(d.hex_pending("color"), "el texto sigue pendiente");
}

#[test]
fn row_12_set_block_does_not_change_the_height_or_the_fields_or_the_values_of_the_fields() {
    let mut d = dialog_20();
    let height_2 = d.content_h_u();
    let spec = d.spec().clone();
    let text_2 = d.text("ancho");
    d.set_block(Some("no cabe".to_string()));
    assert_eq!(d.content_h_u(), height_2);
    assert_eq!(d.spec(), &spec);
    assert_eq!(d.text("ancho"), text_2);
    assert_eq!(d.validity("ancho"), Some(Validity::Valid));
}

#[test]
fn row_12_set_block_on_an_open_modal_works_and_accept_rejects_it() {
    let mut modal = Modal::default();
    assert!(modal.open(DialogKind::ConvertMode, spec_20(), Locks::default()));
    match &mut modal {
        Modal::Open(d) => d.set_block(Some("calculando el informe".to_string())),
        Modal::Closed => panic!("abierto"),
    }
    assert_eq!(
        modal.event(Event::Accept),
        Reaction::Rejected("calculando el informe".to_string())
    );
    assert!(modal.is_open(), "un Accept rechazado no cierra");
    match &mut modal {
        Modal::Open(d) => d.set_block(None),
        Modal::Closed => panic!("abierto"),
    }
    assert!(matches!(modal.event(Event::Accept), Reaction::Accepted(_)));
    assert!(!modal.is_open());
}

// --- E2: checked pública ---

#[test]
fn row_12_checked_is_public_and_reads_the_boxes() {
    let mut d = dialog_20();
    assert_eq!(d.checked("opaco"), Some(false));
    box_value(&mut d, "opaco", true);
    assert_eq!(d.checked("opaco"), Some(true));
    box_value(&mut d, "opaco", false);
    assert_eq!(d.checked("opaco"), Some(false));
}

#[test]
fn row_12_checked_gives_none_if_it_does_not_exist_or_is_not_a_box() {
    let d = dialog_20();
    for id in ["ancho", "modo", "inexistente", ""] {
        assert_eq!(d.checked(id), None, "{id:?}");
    }
}

#[test]
fn row_12_checked_reads_the_boxes_of_an_invalid_dialog() {
    let mut d = dialog_20();
    box_value(&mut d, "opaco", true);
    // Campo inválido.
    number(&mut d, "ancho", "0");
    assert!(!d.is_valid());
    assert_eq!(d.values(), None);
    assert_eq!(d.checked("opaco"), Some(true));
    // `check` que falla.
    number(&mut d, "ancho", "7");
    assert!(!d.is_valid());
    assert_eq!(d.checked("opaco"), Some(true));
    // Motivo externo.
    number(&mut d, "ancho", "8");
    d.set_block(Some("no cabe".to_string()));
    assert!(!d.is_valid());
    assert_eq!(d.checked("opaco"), Some(true));
    box_value(&mut d, "opaco", false);
    assert_eq!(d.checked("opaco"), Some(false));
}

// --- DialogKind::ConvertMode ---

#[test]
fn row_12_dialog_kind_convert_mode_is_a_new_kind() {
    let d = dialog_20();
    assert_eq!(d.kind(), DialogKind::ConvertMode);
    for other_2 in [DialogKind::Licenses, DialogKind::Demo, DialogKind::NewImage] {
        assert_ne!(DialogKind::ConvertMode, other_2);
    }
    let mut modal = Modal::default();
    assert!(modal.open(DialogKind::ConvertMode, spec_20(), Locks::default()));
    assert_eq!(modal.kind(), Some(DialogKind::ConvertMode));
    assert!(
        !modal.close_kind(DialogKind::NewImage),
        "solo cierra el tipo pedido"
    );
    assert!(modal.is_open());
    assert!(modal.close_kind(DialogKind::ConvertMode));
    assert!(!modal.is_open());
}

// ---------------------------------------------------------------------------------------------
// Unidad 20b (specs/hito-2-motivo-y-marcadores.md, D3-A y fila 6): tres ampliaciones aditivas del
// motor. En ROJO hasta que existan `Dialog::problems`, `Dialog::set_summary` y `Dialog::summary`.
// ---------------------------------------------------------------------------------------------
//
// - `problems() -> Vec<String>`: los motivos de los campos visibles inválidos y del `check`, en el
//   orden de hoy y **sin recortar**; no incluye el motivo externo ni el resumen.
// - `set_summary(Option<String>)` y `summary() -> Option<&str>`: con resumen, `reason()` lo da
//   **antes que todo** (campos, `check` y `set_block`), el conjunto es inválido y `Accept` da
//   `Rejected`. Recorte: 37 caracteres, o 44 si el texto empieza por `falta: ` (también en
//   `set_block`). `set_summary(None)` deja el diálogo `==` al de antes. `set_block` sigue como está.
//
// Suposiciones (la spec no las fija): `summary()` devuelve el texto ya recortado (como `block()`);
// con dos campos inválidos `problems()` los lista a los dos, en el orden de los campos; con un
// campo inválido y un `check` que falla a la vez no se prueba si el `check` entra en la lista (hoy
// no se llama con un campo inválido); `set_block(None)` no quita el resumen; el recorte a 44 solo
// se prueba en `set_block` y `set_summary`, no en el texto del `check` ni en el del campo.

const MISSING_TWO: &str = "falta: perder el color; fondo del alfa";

fn problems(d: &Dialog) -> Vec<String> {
    d.problems()
}

/// Dos numéricos válidos por defecto, un grupo que oculta un hexadecimal y un `check` opcional.
fn spec_20b(check: Option<fn(&Values) -> Option<String>>) -> FormSpec {
    FormSpec {
        check,
        ..spec_with(vec![
            numeric("ancho", "Ancho", 1, 100, 64),
            numeric("alto", "Alto", 1, 100, 64),
            field_2(
                "modo",
                "Modo",
                FieldKind::Choice {
                    options: MODES_19,
                    default: 0,
                },
            ),
            FieldSpec {
                id: "color",
                label: "Color",
                kind: FieldKind::Hex {
                    mode: Mode::Rgba,
                    default: Rgba::opaque(0x80, 0x80, 0x80),
                },
                shown_when: Some(ShowWhen {
                    field: "modo",
                    choice: 1,
                }),
            },
        ])
    }
}

fn dialog_20_b(check: Option<fn(&Values) -> Option<String>>) -> Dialog {
    Dialog::new(DialogKind::ConvertMode, spec_20b(check)).expect("spec válida")
}

fn check_length_20_b(_: &Values) -> Option<String> {
    Some("x".repeat(100))
}

#[test]
fn row_6_problems_of_a_valid_dialog_is_empty() {
    assert_eq!(problems(&dialog_20_b(None)), Vec::<String>::new());
    assert_eq!(problems(&dialog()), Vec::<String>::new());
    // Un `check` que no falla tampoco añade nada.
    assert!(problems(&dialog_20_b(Some(check_never))).is_empty());
}

#[test]
fn row_6_problems_with_an_invalid_field_gives_its_reason_the_same_as_reason() {
    let mut d = dialog_20_b(None);
    number(&mut d, "ancho", "0");
    assert_eq!(problems(&d), vec!["Ancho: mínimo 1".to_string()]);
    assert_eq!(d.reason(), Some("Ancho: mínimo 1".to_string()));
    // Un hexadecimal visible inválido.
    let mut d = dialog_20_b(None);
    chooses(&mut d, "modo", 1);
    hex(&mut d, "color", "zz");
    let list = problems(&d);
    assert_eq!(list.len(), 1, "{list:?}");
    assert!(list[0].starts_with("Color: "), "{list:?}");
    assert_eq!(Some(list[0].clone()), d.reason());
}

#[test]
fn row_6_problems_lists_all_the_invalid_fields_in_the_field_order() {
    let mut d = dialog_20_b(None);
    chooses(&mut d, "modo", 1);
    hex(&mut d, "color", "zz");
    number(&mut d, "alto", "0");
    number(&mut d, "ancho", "0");
    let list = problems(&d);
    assert_eq!(list.len(), 3, "{list:?}");
    assert_eq!(list[0], "Ancho: mínimo 1");
    assert_eq!(list[1], "Alto: mínimo 1");
    assert!(list[2].starts_with("Color: "), "{list:?}");
    // El primero es el que da `reason()` sin resumen.
    assert_eq!(d.reason(), Some(list[0].clone()));
}

#[test]
fn row_6_problems_does_not_count_a_hidden_field_even_if_it_has_something_invalid_written() {
    let mut d = dialog_20_b(None);
    chooses(&mut d, "modo", 1);
    hex(&mut d, "color", "zz");
    assert_eq!(problems(&d).len(), 1);
    chooses(&mut d, "modo", 0);
    assert!(!d.is_shown("color"));
    assert!(problems(&d).is_empty());
    assert!(d.is_valid());
}

#[test]
fn row_6_problems_gives_the_check_reason_without_clipping() {
    let d = dialog_20_b(Some(check_length_20_b));
    assert_eq!(problems(&d), vec!["x".repeat(100)]);
    // `reason()` sigue recortándolo a 37 como hoy.
    assert_eq!(d.reason(), Some("x".repeat(37)));
}

#[test]
fn row_6_problems_does_not_include_the_external_reason_or_the_summary() {
    let mut d = dialog_20_b(None);
    d.set_block(Some("calculando el informe".to_string()));
    d.set_summary(Some(MISSING_TWO.to_string()));
    assert!(problems(&d).is_empty());
    // Con un campo inválido, solo el del campo.
    number(&mut d, "ancho", "0");
    assert_eq!(problems(&d), vec!["Ancho: mínimo 1".to_string()]);
}

#[test]
fn row_6_problems_does_not_change_the_dialog() {
    let mut d = dialog_20_b(None);
    number(&mut d, "ancho", "0");
    let before = d.clone();
    let _ = problems(&d);
    assert_eq!(d, before);
}

#[test]
fn row_6_summary_without_setting_anything_is_none_and_set_summary_stores_it() {
    let mut d = dialog_20_b(None);
    assert_eq!(d.summary(), None);
    d.set_summary(Some(MISSING_TWO.to_string()));
    assert_eq!(d.summary(), Some(MISSING_TWO));
    d.set_summary(Some("falta: otra cosa".to_string()));
    assert_eq!(d.summary(), Some("falta: otra cosa"), "el último gana");
}

#[test]
fn row_6_with_a_summary_reason_is_the_summary_and_the_dialog_is_invalid() {
    let mut d = dialog_20_b(None);
    d.set_summary(Some(MISSING_TWO.to_string()));
    assert_eq!(d.reason(), Some(MISSING_TWO.to_string()));
    assert!(!d.is_valid());
    assert_eq!(d.values(), None);
    // El resumen no es un motivo externo.
    assert_eq!(d.block(), None);
}

#[test]
fn row_6_the_summary_goes_before_the_invalid_field_the_check_and_the_external_reason() {
    // Campo inválido.
    let mut d = dialog_20_b(None);
    d.set_block(Some("calculando el informe".to_string()));
    number(&mut d, "ancho", "0");
    assert_eq!(d.reason(), Some("Ancho: mínimo 1".to_string()));
    d.set_summary(Some(MISSING_TWO.to_string()));
    assert_eq!(d.reason(), Some(MISSING_TWO.to_string()));
    // Y el motivo del campo sigue estando en `problems()`.
    assert_eq!(problems(&d), vec!["Ancho: mínimo 1".to_string()]);
    d.set_summary(None);
    assert_eq!(d.reason(), Some("Ancho: mínimo 1".to_string()));

    // `check` que falla.
    let mut d = dialog_20_b(Some(check_length_20_b));
    d.set_block(Some("motivo externo".to_string()));
    assert_eq!(d.reason(), Some("x".repeat(37)));
    d.set_summary(Some(MISSING_TWO.to_string()));
    assert_eq!(d.reason(), Some(MISSING_TWO.to_string()));

    // Solo motivo externo.
    let mut d = dialog_20_b(None);
    d.set_block(Some("motivo externo".to_string()));
    d.set_summary(Some(MISSING_TWO.to_string()));
    assert_eq!(d.reason(), Some(MISSING_TWO.to_string()));
    assert_eq!(d.block(), Some("motivo externo"));
}

#[test]
fn row_6_accept_with_a_summary_gives_rejected_with_the_summary_and_does_not_touch_the_state() {
    let mut d = dialog_20_b(None);
    number(&mut d, "ancho", "50");
    d.set_summary(Some(MISSING_TWO.to_string()));
    let before = d.clone();
    assert_eq!(
        d.event(Event::Accept),
        Reaction::Rejected(MISSING_TWO.to_string())
    );
    assert_eq!(d, before, "un Accept rechazado no cambia nada");
    // Con un campo inválido a la vez, el resumen sigue ganando.
    number(&mut d, "ancho", "0");
    assert_eq!(
        d.event(Event::Accept),
        Reaction::Rejected(MISSING_TWO.to_string())
    );
    // Sin resumen y con todo válido, se acepta.
    number(&mut d, "ancho", "50");
    d.set_summary(None);
    assert!(matches!(d.event(Event::Accept), Reaction::Accepted(_)));
}

#[test]
fn row_6_accept_with_a_summary_through_the_modal_does_not_close() {
    let mut modal = Modal::default();
    assert!(modal.open(DialogKind::ConvertMode, spec_20b(None), Locks::default()));
    match &mut modal {
        Modal::Open(d) => d.set_summary(Some(MISSING_TWO.to_string())),
        Modal::Closed => panic!("abierto"),
    }
    assert_eq!(
        modal.event(Event::Accept),
        Reaction::Rejected(MISSING_TWO.to_string())
    );
    assert!(modal.is_open());
    match &mut modal {
        Modal::Open(d) => d.set_summary(None),
        Modal::Closed => panic!("abierto"),
    }
    assert!(matches!(modal.event(Event::Accept), Reaction::Accepted(_)));
}

#[test]
fn row_6_set_summary_none_leaves_the_dialog_the_same_as_before() {
    // Sobre un diálogo virgen.
    let before = dialog_20_b(None);
    let mut d = before.clone();
    d.set_summary(None);
    assert_eq!(d, before);
    // Tras poner y quitar un resumen.
    d.set_summary(Some(MISSING_TWO.to_string()));
    assert_ne!(d, before, "el resumen forma parte del estado");
    d.set_summary(None);
    assert_eq!(d, before);
    assert_eq!(d.summary(), None);
    assert_eq!(d.reason(), None);
    assert_eq!(d.values(), before.values());
    // Con un campo inválido y un motivo externo de por medio.
    let mut before = dialog_20_b(None);
    number(&mut before, "alto", "0");
    before.set_block(Some("calculando el informe".to_string()));
    let mut d = before.clone();
    d.set_summary(Some("falta: algo".to_string()));
    d.set_summary(None);
    assert_eq!(d, before);
}

#[test]
fn row_6_set_block_stays_as_it_is_with_and_without_a_summary() {
    let mut d = dialog_20_b(None);
    d.set_summary(Some(MISSING_TWO.to_string()));
    // Poner y cambiar el motivo externo no toca el resumen y el resumen no toca `block()`.
    d.set_block(Some("no cabe".to_string()));
    assert_eq!(d.block(), Some("no cabe"));
    assert_eq!(d.summary(), Some(MISSING_TWO));
    assert_eq!(d.reason(), Some(MISSING_TWO.to_string()));
    // Quitar el resumen deja el motivo externo.
    d.set_summary(None);
    assert_eq!(d.block(), Some("no cabe"));
    assert_eq!(d.reason(), Some("no cabe".to_string()));
    // Quitar el motivo externo con un resumen puesto no quita el resumen.
    d.set_summary(Some(MISSING_TWO.to_string()));
    d.set_block(None);
    assert_eq!(d.block(), None);
    assert_eq!(d.summary(), Some(MISSING_TWO));
    assert_eq!(d.reason(), Some(MISSING_TWO.to_string()));
    d.set_summary(None);
    assert_eq!(d.reason(), None);
}

#[test]
fn row_6_the_summary_is_clipped_to_37_or_to_44_if_it_starts_with_missing() {
    for (text_2, expected_2) in [
        // Sin el prefijo `falta: `, como hoy: 37.
        ("x".repeat(36), "x".repeat(36)),
        ("x".repeat(37), "x".repeat(37)),
        ("x".repeat(38), "x".repeat(37)),
        ("x".repeat(200), "x".repeat(37)),
        ("ñ".repeat(50), "ñ".repeat(37)),
        // Con `falta: `, 44 en total (el pie lleva ese prefijo en lugar de `error: `).
        (
            format!("falta: {}", "x".repeat(30)),
            format!("falta: {}", "x".repeat(30)),
        ),
        (
            format!("falta: {}", "x".repeat(37)),
            format!("falta: {}", "x".repeat(37)),
        ),
        (
            format!("falta: {}", "x".repeat(38)),
            format!("falta: {}", "x".repeat(37)),
        ),
        (
            format!("falta: {}", "x".repeat(200)),
            format!("falta: {}", "x".repeat(37)),
        ),
        (
            format!("falta: {}", "ñ".repeat(60)),
            format!("falta: {}", "ñ".repeat(37)),
        ),
        // La forma con cuenta (`faltan N: `) también es una lista: 44 en total.
        (
            format!("faltan 3: {}", "x".repeat(60)),
            format!("faltan 3: {}", "x".repeat(34)),
        ),
        // `falta:` sin espacio no es el prefijo: 37.
        (
            format!("falta:{}", "x".repeat(60)),
            format!("falta:{}", "x".repeat(31)),
        ),
        (
            format!("Falta: {}", "x".repeat(60)),
            format!("Falta: {}", "x".repeat(30)),
        ),
    ] {
        let mut d = dialog_20_b(None);
        d.set_summary(Some(text_2.clone()));
        let reason_2 = d.reason().expect("hay motivo");
        assert_eq!(
            reason_2,
            expected_2,
            "texto de {} caracteres",
            text_2.chars().count()
        );
        assert_eq!(
            d.summary(),
            Some(expected_2.as_str()),
            "summary() lo da recortado"
        );
        assert!(!d.is_valid());
    }
}

#[test]
fn row_6_set_block_also_clips_to_44_if_it_starts_with_missing() {
    for (text_2, expected_2) in [
        (
            format!("falta: {}", "x".repeat(30)),
            format!("falta: {}", "x".repeat(30)),
        ),
        (
            format!("falta: {}", "x".repeat(37)),
            format!("falta: {}", "x".repeat(37)),
        ),
        (
            format!("falta: {}", "x".repeat(38)),
            format!("falta: {}", "x".repeat(37)),
        ),
        (
            format!("falta: {}", "ñ".repeat(60)),
            format!("falta: {}", "ñ".repeat(37)),
        ),
        // Como hoy sin el prefijo.
        ("x".repeat(38), "x".repeat(37)),
        (
            format!("faltan 3: {}", "x".repeat(60)),
            format!("faltan 3: {}", "x".repeat(34)),
        ),
    ] {
        let mut d = dialog_20_b(None);
        d.set_block(Some(text_2.clone()));
        assert_eq!(d.block(), Some(expected_2.as_str()), "{text_2:?}");
        assert_eq!(d.reason(), Some(expected_2.clone()));
        assert!(expected_2.chars().count() <= 44);
    }
}

#[test]
fn row_6_the_texts_of_the_spec_list_fit_without_clipping() {
    for text_2 in [
        "falta: permitir perder el color",
        "falta: elegir el fondo del alfa",
        "falta: perder el color; fondo del alfa",
        "falta: perder el color; fondo válido",
        "falta: fondo del alfa; fondo válido",
        "faltan 3: color, fondo, valor",
    ] {
        let mut d = dialog_20_b(None);
        d.set_block(Some(text_2.to_string()));
        assert_eq!(d.reason(), Some(text_2.to_string()), "block {text_2:?}");
        let mut d = dialog_20_b(None);
        d.set_summary(Some(text_2.to_string()));
        assert_eq!(d.reason(), Some(text_2.to_string()), "summary {text_2:?}");
        assert_eq!(d.summary(), Some(text_2));
    }
}

// ---------------------------------------------------------------------------------------------
// Apéndice A1 de la unidad 19 (specs/hito-2-nueva-imagen.md): `Dialog::hex_committed` y
// `resolve_hex`. En ROJO hasta que existan las dos piezas (aditivas, sin cambiar nada de la 18).
// ---------------------------------------------------------------------------------------------

const TEST_DEFAULT: Rgba = Rgba::new(0x33, 0x66, 0x99, 0x80);

#[test]
fn row_a1_hex_committed_is_born_at_the_default_value_of_the_field() {
    for mode in [Mode::Rgba, Mode::GrayAlpha, Mode::Gray] {
        let default = match mode {
            Mode::Rgba => TEST_DEFAULT,
            Mode::GrayAlpha => Rgba::new(0x80, 0x80, 0x80, 0x40),
            _ => Rgba::opaque(0x80, 0x80, 0x80),
        };
        let d = open(hex_form(mode, default));
        assert_eq!(d.hex_committed("color"), Some(default), "{mode:?}");
    }
    // Un campo oculto existe y tiene su valor por defecto (el del formulario de la fila 3).
    let d = dialog();
    assert!(!d.is_shown("color"));
    assert_eq!(
        d.hex_committed("color"),
        Some(Rgba::opaque(0x33, 0x66, 0x99))
    );
}

#[test]
fn row_a1_hex_committed_follows_the_last_confirmed() {
    let mut d = open(hex_form(Mode::Rgba, TEST_DEFAULT));
    // Intro con un válido de 6 dígitos: toma el alfa del confirmado.
    hex(&mut d, "color", "#123456");
    confirms(&mut d, "color", false);
    assert_eq!(
        d.hex_committed("color"),
        Some(Rgba::new(0x12, 0x34, 0x56, 0x80))
    );
    // Pérdida del foco con un válido de 8 dígitos: el suyo.
    hex(&mut d, "color", "#abcdef11");
    confirms(&mut d, "color", true);
    assert_eq!(
        d.hex_committed("color"),
        Some(Rgba::new(0xab, 0xcd, 0xef, 0x11))
    );
    // Tres dígitos: el alfa del último confirmado (0x11), no el de partida.
    hex(&mut d, "color", "f00");
    confirms(&mut d, "color", false);
    assert_eq!(d.hex_committed("color"), Some(Rgba::new(0xff, 0, 0, 0x11)));
    // `Accept` confirma el pendiente válido antes de entregarlo.
    hex(&mut d, "color", "#00ff00ee");
    assert!(matches!(d.event(Event::Accept), Reaction::Accepted(_)));
    assert_eq!(d.hex_committed("color"), Some(Rgba::new(0, 0xff, 0, 0xee)));
}

#[test]
fn row_a1_hex_committed_with_pending_text_follows_the_confirmed_one() {
    let confirmed = Rgba::new(0x12, 0x34, 0x56, 0x80);
    let mut d = open(hex_form(Mode::Rgba, TEST_DEFAULT));
    hex(&mut d, "color", "#123456");
    confirms(&mut d, "color", false);
    assert_eq!(d.hex_committed("color"), Some(confirmed));
    // Pendiente inválido (escrito, con Intro, con pérdida del foco que revierte, con Escape).
    hex(&mut d, "color", "zz");
    assert!(d.hex_pending("color"));
    assert_eq!(d.hex_committed("color"), Some(confirmed), "zz pendiente");
    confirms(&mut d, "color", false);
    assert!(d.hex_pending("color"));
    assert_eq!(d.hex_committed("color"), Some(confirmed), "zz con Intro");
    confirms(&mut d, "color", true);
    assert!(!d.hex_pending("color"));
    assert_eq!(d.hex_committed("color"), Some(confirmed), "zz revertido");
    d.event(Event::HexRevert { field: "color" });
    assert_eq!(d.hex_committed("color"), Some(confirmed), "tras Escape");
    // Pendiente válido sin confirmar: sigue el confirmado, no el escrito.
    hex(&mut d, "color", "#abcdef");
    assert!(d.hex_pending("color"));
    assert_eq!(
        d.hex_committed("color"),
        Some(confirmed),
        "válido sin confirmar"
    );
    // El campo oculto conserva el suyo aunque se oculte con basura escrita.
    let mut d = dialog();
    chooses(&mut d, "modo", 2);
    hex(&mut d, "color", "#445566");
    confirms(&mut d, "color", false);
    hex(&mut d, "color", "zz");
    chooses(&mut d, "modo", 0);
    assert!(!d.is_shown("color"));
    assert_eq!(
        d.hex_committed("color"),
        Some(Rgba::opaque(0x44, 0x55, 0x66))
    );
}

#[test]
fn row_a1_hex_committed_gives_none_if_the_field_does_not_exist() {
    let d = dialog();
    for id in ["", "nada", "Color", "color ", "color_rgba"] {
        assert_eq!(d.hex_committed(id), None, "{id:?}");
    }
    let d = open(hex_form(Mode::Rgba, TEST_DEFAULT));
    assert_eq!(d.hex_committed("modo"), None);
}

#[test]
fn row_a1_hex_committed_gives_none_if_the_field_is_not_hexadecimal() {
    let d = dialog();
    for id in ["ancho", "alto", "modo", "opaco", "informe"] {
        assert!(d.spec().fields.iter().any(|f| f.id == id), "{id} existe");
        assert_eq!(d.hex_committed(id), None, "{id}");
    }
}

#[test]
fn row_a1_hex_committed_does_not_change_the_dialog() {
    let mut d = open(hex_form(Mode::Rgba, TEST_DEFAULT));
    hex(&mut d, "color", "zz");
    let before = d.clone();
    let _ = d.hex_committed("color");
    let _ = d.hex_committed("nada");
    assert_eq!(d, before);
}

const TEXTS_RESOLVE: [&str; 22] = [
    "#abc",
    "abc",
    "#abcd",
    "abcd",
    "#aabbcc",
    "aabbcc",
    "#aabbccdd",
    "aabbccdd",
    "  #aabbcc  ",
    " abc",
    "#ABC",
    "#fff",
    "#808080",
    "#80808080",
    "#ff0000",
    "#ff000080",
    "",
    "#",
    "zz",
    "#12",
    "#12345",
    "#aabbccddee",
];

const CONFIRMED_RESOLVE: [Rgba; 4] = [
    Rgba::new(0x80, 0x80, 0x80, 0xff),
    Rgba::new(0x33, 0x66, 0x99, 0xcc),
    Rgba::new(0x80, 0x80, 0x80, 0x80),
    Rgba::new(0, 0, 0, 0),
];

#[test]
fn row_a1_resolve_hex_without_text_gives_the_confirmed_one() {
    for mode in [Mode::Rgba, Mode::GrayAlpha, Mode::Gray, Mode::Indexed] {
        for committed in CONFIRMED_RESOLVE {
            assert_eq!(
                resolve_hex(None, mode, committed),
                Ok(committed),
                "{mode:?}"
            );
        }
    }
}

#[test]
fn row_a1_resolve_hex_with_text_is_parse_hex_with_the_confirmed_one() {
    for mode in [Mode::Rgba, Mode::GrayAlpha, Mode::Gray] {
        for committed in CONFIRMED_RESOLVE {
            for text_2 in TEXTS_RESOLVE {
                assert_eq!(
                    resolve_hex(Some(text_2), mode, committed),
                    parse_hex(text_2, mode, committed),
                    "{mode:?} {committed:?} {text_2:?}"
                );
            }
        }
    }
}

#[test]
fn row_a1_resolve_hex_three_and_six_digits_take_the_alpha_of_the_confirmed_one_and_four_and_eight_their_own()
 {
    let confirmed = Rgba::new(0x33, 0x66, 0x99, 0xcc);
    for (text_2, expected_2) in [
        ("bbb", Rgba::new(0xbb, 0xbb, 0xbb, 0xcc)),
        ("#bbb", Rgba::new(0xbb, 0xbb, 0xbb, 0xcc)),
        ("bbbbbb", Rgba::new(0xbb, 0xbb, 0xbb, 0xcc)),
        ("#bbbbbb", Rgba::new(0xbb, 0xbb, 0xbb, 0xcc)),
        ("#bbb8", Rgba::new(0xbb, 0xbb, 0xbb, 0x88)),
        ("bbb8", Rgba::new(0xbb, 0xbb, 0xbb, 0x88)),
        ("#bbbbbbdd", Rgba::new(0xbb, 0xbb, 0xbb, 0xdd)),
        ("bbbbbbdd", Rgba::new(0xbb, 0xbb, 0xbb, 0xdd)),
        ("  #bbbbbb  ", Rgba::new(0xbb, 0xbb, 0xbb, 0xcc)),
        ("#BBBBBBDD", Rgba::new(0xbb, 0xbb, 0xbb, 0xdd)),
    ] {
        assert_eq!(
            resolve_hex(Some(text_2), Mode::Rgba, confirmed),
            Ok(expected_2),
            "RGBA {text_2:?}"
        );
        assert_eq!(
            resolve_hex(Some(text_2), Mode::GrayAlpha, confirmed),
            Ok(expected_2),
            "Gris con alfa {text_2:?}"
        );
    }
}

#[test]
fn row_a1_resolve_hex_the_invalid_ones_give_their_error() {
    let c = Rgba::opaque(0x80, 0x80, 0x80);
    for (text_2, error) in [
        ("", HexError::Empty),
        ("#", HexError::Empty),
        ("   ", HexError::Empty),
        ("zz", HexError::Length(2)),
        ("#12", HexError::Length(2)),
        ("#12345", HexError::Length(5)),
        ("#aabbcc1", HexError::Length(7)),
        ("#ggg", HexError::Digit),
        ("#aabbcg", HexError::Digit),
    ] {
        for mode in [Mode::Rgba, Mode::GrayAlpha, Mode::Gray] {
            assert_eq!(
                resolve_hex(Some(text_2), mode, c),
                Err(error.clone()),
                "{mode:?} {text_2:?}"
            );
        }
    }
}

#[test]
fn row_a1_resolve_hex_applies_the_rules_of_gray_and_gray_alpha() {
    let c = Rgba::new(0x80, 0x80, 0x80, 0xcc);
    // Un color no gris no vale ni en Gris ni en Gris con alfa, y sí en RGBA.
    assert_eq!(
        resolve_hex(Some("#ff0000"), Mode::Rgba, c),
        Ok(Rgba::new(0xff, 0, 0, 0xcc))
    );
    assert_eq!(
        resolve_hex(Some("#ff0000"), Mode::GrayAlpha, c),
        Err(HexError::NotGray)
    );
    assert_eq!(
        resolve_hex(Some("#ff0000"), Mode::Gray, c),
        Err(HexError::NotGray)
    );
    // Un gris con alfa vale en Gris con alfa y no en Gris (sin alfa).
    assert_eq!(
        resolve_hex(Some("#80808080"), Mode::GrayAlpha, c),
        Ok(Rgba::new(0x80, 0x80, 0x80, 0x80))
    );
    assert_eq!(
        resolve_hex(Some("#80808080"), Mode::Gray, c),
        Err(HexError::NoAlpha)
    );
    assert_eq!(
        resolve_hex(Some("#808080ff"), Mode::Gray, c),
        Ok(Rgba::opaque(0x80, 0x80, 0x80))
    );
    // Con `None` no se mira el modo: el confirmado tal cual.
    assert_eq!(resolve_hex(None, Mode::Gray, c), Ok(c));
}

#[test]
fn row_a1_resolve_hex_is_the_rule_used_by_validity_and_values_of_the_dialog() {
    // El dialogo y la función dan el mismo color y el mismo error para cada texto, con el último
    // confirmado distinto del de partida.
    for mode in [Mode::Rgba, Mode::GrayAlpha, Mode::Gray] {
        for text_2 in TEXTS_RESOLVE {
            let mut d = open(hex_form(mode, Rgba::opaque(0x80, 0x80, 0x80)));
            hex(&mut d, "color", "#aaaaaa");
            confirms(&mut d, "color", false);
            let confirmed = d.hex_committed("color").expect("hexadecimal");
            hex(&mut d, "color", text_2);
            match resolve_hex(Some(text_2), mode, confirmed) {
                Ok(color) => {
                    assert_eq!(
                        d.validity("color"),
                        Some(Validity::Valid),
                        "{mode:?} {text_2:?}"
                    );
                    assert_eq!(color_accepted(&d), color, "{mode:?} {text_2:?}");
                }
                Err(error) => {
                    assert_eq!(
                        d.validity("color"),
                        Some(Validity::Invalid(hex_notice(&error).to_string())),
                        "{mode:?} {text_2:?}"
                    );
                    assert!(matches!(
                        d.clone().event(Event::Accept),
                        Reaction::Rejected(_)
                    ));
                    // Con Intro el texto inválido se queda; con la pérdida del foco, revierte.
                    confirms(&mut d, "color", false);
                    assert_eq!(d.hex_committed("color"), Some(confirmed));
                    assert!(d.hex_pending("color"), "{mode:?} {text_2:?}");
                    confirms(&mut d, "color", true);
                    assert!(!d.hex_pending("color"), "{mode:?} {text_2:?}");
                    assert_eq!(d.hex_committed("color"), Some(confirmed));
                }
            }
        }
    }
}
