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
//! - `Validity::Invalid(m)` lleva el motivo; `reason()` es `etiqueta: motivo`. Para no fijar si
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
//!   hexadecimal 32, casilla 16, grupo `14 + 16·n + 2·(n−1)`, zona `6 + 14·min(filas, líneas)`)
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
    EventRecipe, FormRecipe, IDS, KIND_ABANDON, LABELS, SUMA_IMPAR, build, event_of, event_recipe,
    form_recipe, target_id,
};

// ---------------------------------------------------------------------------------------------
// Ayudas
// ---------------------------------------------------------------------------------------------

fn dialog() -> Dialog {
    open(all_kinds())
}

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

fn hex(d: &mut Dialog, field: &'static str, text: &str) {
    assert_eq!(
        d.event(Event::Hex {
            field,
            text: text.to_string()
        }),
        Reaction::None
    );
}

fn confirma(d: &mut Dialog, field: &'static str, blur: bool) {
    assert_eq!(d.event(Event::HexConfirm { field, blur }), Reaction::None);
}

fn en_el_juego(texto: &str) -> bool {
    texto.chars().all(|c| INTERFACE_CHARSET.contains(c))
}

fn campo(id: &'static str, label: &'static str, kind: FieldKind) -> FieldSpec {
    FieldSpec {
        id,
        label,
        kind,
        shown_when: None,
    }
}

fn numerico(id: &'static str, label: &'static str, min: u32, max: u32, default: u32) -> FieldSpec {
    campo(id, label, FieldKind::Number { min, max, default })
}

fn spec_con(fields: Vec<FieldSpec>) -> FormSpec {
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
    spec_con(vec![campo(
        "color",
        "Color",
        FieldKind::Hex { mode, default },
    )])
}

/// El color confirmado, leído sin cerrar el diálogo (con `Accept` sobre una copia).
fn color_aceptado(d: &Dialog) -> Rgba {
    match d.clone().event(Event::Accept) {
        Reaction::Accepted(v) => v
            .color("color")
            .expect("el hexadecimal sale en los valores"),
        r => panic!("se esperaba Accepted y salió {r:?}"),
    }
}

fn dialogo_de(modal: &Modal) -> &Dialog {
    match modal {
        Modal::Open(d) => d,
        Modal::Closed => panic!("el modal está cerrado"),
    }
}

fn modal_abierto(spec: FormSpec) -> Modal {
    let mut modal = Modal::default();
    assert!(modal.open(DialogKind::Licenses, spec, Locks::default()));
    modal
}

fn comprueba_rechazada(causa: &str, spec: FormSpec) {
    assert!(spec.validate().is_err(), "{causa}: validate la acepta");
}

/// Como `comprueba_rechazada`, y además `Dialog::new` y `Modal::open` la rechazan.
fn rechaza(causa: &str, mut hace: impl FnMut() -> FormSpec) {
    comprueba_rechazada(causa, hace());
    assert!(
        Dialog::new(DialogKind::Licenses, hace()).is_err(),
        "{causa}: Dialog::new la acepta"
    );
    let mut modal = Modal::default();
    assert!(
        !modal.open(DialogKind::Licenses, hace(), Locks::default()),
        "{causa}: Modal::open la acepta"
    );
    assert!(!modal.is_open(), "{causa}: el modal se abrió");
    assert!(matches!(modal, Modal::Closed));
}

// ---------------------------------------------------------------------------------------------
// Fila 3: validez de cada campo y del conjunto
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_3_la_spec_de_los_cinco_tipos_valida() {
    assert!(all_kinds().validate().is_ok());
    assert!(huge(10).validate().is_ok());
    assert!(huge(0).validate().is_ok());
    assert!(spec_con(vec![]).validate().is_ok());
}

#[test]
fn fila_3_por_defecto_es_valido_y_los_valores_son_los_de_los_campos_visibles() {
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
fn fila_3_los_accesores_basicos() {
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
fn fila_3_un_numerico_vacio_da_el_motivo_con_la_etiqueta() {
    let mut d = dialog();
    numero(&mut d, "ancho", "");
    assert!(!d.is_valid());
    assert_eq!(d.reason(), Some("Ancho: escribe un número".to_string()));
    assert_eq!(d.values().map(|_| ()), None);
    assert_eq!(d.text("ancho"), Some(String::new()));
    let Some(Validity::Invalid(motivo)) = d.validity("ancho") else {
        panic!("el campo vacío es inválido");
    };
    assert!(d.reason().unwrap().contains(&motivo));
    assert_eq!(d.validity("alto"), Some(Validity::Valid));
}

#[test]
fn fila_3_los_motivos_de_cada_error_numerico() {
    let casos = [
        ("", "Ancho: escribe un número"),
        ("abc", "Ancho: solo dígitos 0-9"),
        (" 5", "Ancho: solo dígitos 0-9"),
        ("-5", "Ancho: solo dígitos 0-9"),
        ("0", "Ancho: mínimo 1"),
        ("8193", "Ancho: máximo 8192"),
        ("99999999999", "Ancho: máximo 10 cifras"),
        ("4294967295", "Ancho: máximo 8192"),
    ];
    for (texto, esperado) in casos {
        let mut d = dialog();
        numero(&mut d, "ancho", texto);
        assert_eq!(d.reason().as_deref(), Some(esperado), "texto {texto:?}");
    }
}

#[test]
fn fila_3_dos_invalidos_a_la_vez_el_motivo_es_el_del_primero_en_orden_de_campos() {
    let mut d = dialog();
    numero(&mut d, "alto", "abc");
    numero(&mut d, "ancho", "");
    assert_eq!(d.reason(), Some("Ancho: escribe un número".to_string()));
    // Se arregla el primero: sale el segundo.
    numero(&mut d, "ancho", "10");
    assert_eq!(d.reason(), Some("Alto: solo dígitos 0-9".to_string()));
    // Y al arreglar el segundo, todo vale.
    numero(&mut d, "alto", "10");
    assert!(d.is_valid());
    assert_eq!(d.reason(), None);
    assert!(d.values().is_some());
}

#[test]
fn fila_3_el_hexadecimal_oculto_no_cuenta_y_al_mostrarse_si_y_recupera_lo_escrito() {
    let mut d = dialog();
    // Se muestra con la opción 2 y se escribe algo inválido.
    elige(&mut d, "modo", 2);
    assert!(d.is_shown("color"));
    hex(&mut d, "color", "zz");
    assert!(!d.is_valid());
    let motivo = d.reason().expect("hay motivo");
    assert!(motivo.starts_with("Color: "), "{motivo}");
    // Se oculta: ya no cuenta y no sale en los valores, pero conserva lo escrito.
    elige(&mut d, "modo", 0);
    assert!(!d.is_shown("color"));
    assert!(d.is_valid());
    assert_eq!(d.reason(), None);
    let v = d.values().expect("válido");
    assert_eq!(v.get("color"), None);
    assert_eq!(d.text("color"), Some("zz".to_string()));
    assert!(d.hex_pending("color"));
    // Al mostrarse otra vez recupera lo escrito y vuelve a ser inválido, con el mismo motivo.
    elige(&mut d, "modo", 2);
    assert!(!d.is_valid());
    assert_eq!(d.reason(), Some(motivo.clone()));
    assert_eq!(d.text("color"), Some("zz".to_string()));
    // Y de nuevo oculto.
    elige(&mut d, "modo", 1);
    assert!(d.is_valid());
    assert_eq!(d.text("color"), Some("zz".to_string()));
    // Con la opción 2 y el color por defecto sale en los valores.
    let mut d = dialog();
    elige(&mut d, "modo", 2);
    let v = d.values().expect("válido");
    assert_eq!(v.color("color"), Some(Rgba::opaque(0x33, 0x66, 0x99)));
    assert_eq!(
        v.get("color"),
        Some(&Value::Color(Rgba::opaque(0x33, 0x66, 0x99)))
    );
    assert_eq!(d.text("color"), Some("#336699".to_string()));
}

#[test]
fn fila_3_los_numericos_ocultos_tampoco_cuentan() {
    let mut spec = spec_con(vec![
        campo(
            "modo",
            "Modo",
            FieldKind::Choice {
                options: &["A", "B"],
                default: 0,
            },
        ),
        numerico("n", "Cuanto", 1, 10, 5),
    ]);
    spec.fields[1].shown_when = Some(ShowWhen {
        field: "modo",
        choice: 1,
    });
    let mut d = open(spec);
    assert!(!d.is_shown("n"));
    assert_eq!(d.values().expect("válido").number("n"), None);
    elige(&mut d, "modo", 1);
    numero(&mut d, "n", "");
    assert_eq!(d.reason(), Some("Cuanto: escribe un número".to_string()));
    elige(&mut d, "modo", 0);
    assert!(d.is_valid());
    assert_eq!(d.text("n"), Some(String::new()));
}

#[test]
fn fila_3_los_motivos_del_hexadecimal_son_los_de_hex_notice() {
    let def = Rgba::opaque(0x33, 0x66, 0x99);
    for texto in ["#12", "zz", "zzz", "", "#12345", "#ggg", "#12 3"] {
        let mut d = open(hex_form(Mode::Rgba, def));
        hex(&mut d, "color", texto);
        let error = parse_hex(texto, Mode::Rgba, def).expect_err(texto);
        assert!(!d.is_valid(), "{texto:?}");
        assert_eq!(
            d.reason(),
            Some(format!("Color: {}", hex_notice(&error))),
            "{texto:?}"
        );
        assert!(d.hex_pending("color"), "{texto:?}");
    }
    // En el formulario de los cinco tipos, con `#12`.
    let mut d = dialog();
    elige(&mut d, "modo", 2);
    hex(&mut d, "color", "#12");
    assert_eq!(
        d.reason(),
        Some("Color: usa 3, 4, 6 u 8 dígitos".to_string())
    );
}

#[test]
fn fila_3_hexadecimal_en_modo_gris() {
    let gris = Rgba::opaque(0x80, 0x80, 0x80);
    let mut d = open(hex_form(Mode::Gray, gris));
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
    confirma(&mut d, "color", false);
    assert_eq!(color_aceptado(&d), Rgba::opaque(0xaa, 0xaa, 0xaa));
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
fn fila_3_el_check_solo_se_llama_con_todo_valido() {
    // Con un campo inválido, ni al crear el estado ni al consultarlo se llama.
    let mut d = dialog();
    reset_check_calls();
    numero(&mut d, "ancho", "");
    let _ = (d.is_valid(), d.reason(), d.values(), d.validity("ancho"));
    let _ = (d.validity("alto"), d.text("ancho"), d.is_shown("color"));
    assert_eq!(
        check_calls(),
        0,
        "el check no se llama con un campo inválido"
    );
    // Con todo válido sí se llama.
    reset_check_calls();
    numero(&mut d, "ancho", "10");
    let _ = (d.is_valid(), d.reason(), d.values());
    assert!(check_calls() >= 1, "el check se llama con todo válido");
}

#[test]
fn fila_3_el_check_que_falla_con_todo_valido_invalida_el_conjunto() {
    let mut d = dialog();
    numero(&mut d, "ancho", "2000");
    assert!(d.is_valid(), "2000 × 64 cabe");
    reset_check_calls();
    numero(&mut d, "alto", "2000");
    assert!(!d.is_valid());
    assert_eq!(d.reason(), Some("demasiado grande".to_string()));
    assert!(d.values().is_none());
    assert!(check_calls() >= 1);
    // El motivo del check no se atribuye a ningún campo.
    assert_eq!(d.validity("ancho"), Some(Validity::Valid));
    assert_eq!(d.validity("alto"), Some(Validity::Valid));
    // Un campo inválido gana al check: ya no se llama.
    reset_check_calls();
    numero(&mut d, "alto", "");
    assert_eq!(d.reason(), Some("Alto: escribe un número".to_string()));
    assert!(d.values().is_none());
    assert_eq!(
        check_calls(),
        0,
        "con un campo inválido no se llama al check"
    );
}

#[test]
fn fila_3_el_limite_del_check_es_un_millon_de_pixeles() {
    let mut d = dialog();
    numero(&mut d, "ancho", "1000");
    numero(&mut d, "alto", "1000");
    assert!(d.is_valid(), "1000 × 1000 = 1 000 000 cabe");
    numero(&mut d, "alto", "1001");
    assert!(!d.is_valid());
    numero(&mut d, "ancho", "8192");
    numero(&mut d, "alto", "8192");
    assert_eq!(d.reason(), Some("demasiado grande".to_string()));
}

fn check_largo(_: &Values) -> Option<String> {
    Some("x".repeat(100))
}

fn check_multibyte(_: &Values) -> Option<String> {
    Some("é".repeat(100))
}

fn check_nunca(_: &Values) -> Option<String> {
    None
}

#[test]
fn fila_3_el_texto_del_check_se_recorta_a_37_caracteres() {
    let mut spec = spec_con(vec![numerico("n", "Cuanto", 1, 10, 5)]);
    spec.check = Some(check_largo);
    let d = open(spec);
    assert!(!d.is_valid());
    let motivo = d.reason().expect("el check falla");
    assert!(motivo.chars().count() <= 37, "{motivo:?}");
    assert!(!motivo.is_empty());
    assert!("x".repeat(100).starts_with(&motivo), "{motivo:?}");

    let mut spec = spec_con(vec![numerico("n", "Cuanto", 1, 10, 5)]);
    spec.check = Some(check_multibyte);
    let d = open(spec);
    let motivo = d.reason().expect("el check falla");
    assert!(motivo.chars().count() <= 37, "{motivo:?}");
    assert!("é".repeat(100).starts_with(&motivo));

    let mut spec = spec_con(vec![numerico("n", "Cuanto", 1, 10, 5)]);
    spec.check = Some(check_nunca);
    assert!(open(spec).is_valid());
}

#[test]
fn fila_3_todo_motivo_mide_37_o_menos_y_esta_en_el_juego_de_caracteres() {
    let textos = [
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
    let etiqueta = "ABCDEFGHIJKL";
    assert_eq!(etiqueta.chars().count(), 12);
    let mut mayor = 0;
    for texto in textos {
        let mut d = open(spec_con(vec![numerico("n", etiqueta, 1, 8192, 64)]));
        numero(&mut d, "n", texto);
        if let Some(r) = d.reason() {
            mayor = mayor.max(r.chars().count());
            assert!(r.chars().count() <= 37, "{r:?}");
            assert!(en_el_juego(&r), "{r:?}");
        }
        let mut d = open(hex_form_con_etiqueta(etiqueta));
        hex(&mut d, "color", texto);
        if let Some(r) = d.reason() {
            mayor = mayor.max(r.chars().count());
            assert!(r.chars().count() <= 37, "{r:?}");
            assert!(en_el_juego(&r), "{r:?}");
        }
    }
    // El peor caso (12 + «: » + 23) llega exactamente a 37.
    let mut d = open(hex_form_con_etiqueta(etiqueta));
    hex(&mut d, "color", "#ff0000");
    assert_eq!(d.reason().expect("modo gris").chars().count(), 37);
    assert_eq!(mayor, 37);
}

fn hex_form_con_etiqueta(label: &'static str) -> FormSpec {
    spec_con(vec![campo(
        "color",
        label,
        FieldKind::Hex {
            mode: Mode::Gray,
            default: Rgba::opaque(0x80, 0x80, 0x80),
        },
    )])
}

#[test]
fn fila_3_el_motivo_de_cualquier_estado_de_los_cinco_tipos_cabe_en_la_linea() {
    // `error: ` (7) + motivo (≤ 37) = 44 columnas.
    for texto in ["", "x", "0", "8193", "99999999999"] {
        let mut d = dialog();
        numero(&mut d, "alto", texto);
        if let Some(r) = d.reason() {
            assert!(7 + r.chars().count() <= 44, "{r:?}");
            assert!(en_el_juego(&r));
        }
    }
}

#[test]
fn fila_3_validate_acepta_los_limites_exactos() {
    // Rótulos de 12 (numérico y hexadecimal) y 42 (casilla); min = max; valores por defecto en
    // los extremos; la última opción por defecto.
    let spec = spec_con(vec![
        numerico("a", "ABCDEFGHIJKL", 5, 5, 5),
        numerico("b", "B", 0, u32::MAX, 0),
        numerico("c", "C", 0, u32::MAX, u32::MAX),
        campo(
            "d",
            "ABCDEFGHIJKL",
            FieldKind::Hex {
                mode: Mode::Rgba,
                default: Rgba::TRANSPARENT,
            },
        ),
        campo("e", "x", FieldKind::Check { default: true }),
        campo(
            "f",
            "Opciones",
            FieldKind::Choice {
                options: &["A", "B", "C"],
                default: 2,
            },
        ),
    ]);
    assert!(spec.validate().is_ok());
    let casilla_larga: &'static str = Box::leak("x".repeat(42).into_boxed_str());
    let spec = spec_con(vec![campo(
        "g",
        casilla_larga,
        FieldKind::Check { default: false },
    )]);
    assert!(spec.validate().is_ok());
    // El texto del informe no es fijo: pasa por `printable` al pintar y no se valida contra el
    // juego de caracteres.
    let spec = spec_con(vec![campo(
        "h",
        "Informe",
        FieldKind::ReadOnly {
            text: "日✓ 🙂".to_string(),
            rows: Rows::Fixed(3),
        },
    )]);
    assert!(spec.validate().is_ok());
    // Un `Fill` solo es válido.
    let spec = spec_con(vec![campo(
        "i",
        "Zona",
        FieldKind::ReadOnly {
            text: "x".to_string(),
            rows: Rows::Fill,
        },
    )]);
    assert!(spec.validate().is_ok());
    // `shown_when` válido: un `Choice` anterior y una opción que existe.
    let mut spec = spec_con(vec![
        campo(
            "modo",
            "Modo",
            FieldKind::Choice {
                options: &["A", "B"],
                default: 0,
            },
        ),
        campo("c", "Casilla", FieldKind::Check { default: false }),
    ]);
    spec.fields[1].shown_when = Some(ShowWhen {
        field: "modo",
        choice: 1,
    });
    assert!(spec.validate().is_ok());
}

#[test]
fn fila_3_spec_error_id_repetido() {
    rechaza("id repetido", || {
        spec_con(vec![
            campo("a", "Uno", FieldKind::Check { default: false }),
            campo("a", "Dos", FieldKind::Check { default: true }),
        ])
    });
    rechaza("id repetido entre tipos distintos", || {
        spec_con(vec![
            numerico("a", "Uno", 1, 2, 1),
            campo(
                "b",
                "Dos",
                FieldKind::Choice {
                    options: &["x"],
                    default: 0,
                },
            ),
            campo("a", "Tres", FieldKind::Check { default: false }),
        ])
    });
}

#[test]
fn fila_3_spec_error_min_mayor_que_max() {
    rechaza("min > max", || spec_con(vec![numerico("n", "N", 10, 5, 7)]));
    rechaza("min > max por uno", || {
        spec_con(vec![numerico("n", "N", 6, 5, 5)])
    });
}

#[test]
fn fila_3_spec_error_valor_por_defecto_fuera() {
    rechaza("default por debajo", || {
        spec_con(vec![numerico("n", "N", 1, 10, 0)])
    });
    rechaza("default por encima", || {
        spec_con(vec![numerico("n", "N", 1, 10, 11)])
    });
    rechaza("opción por defecto = len", || {
        spec_con(vec![campo(
            "m",
            "M",
            FieldKind::Choice {
                options: &["a", "b", "c"],
                default: 3,
            },
        )])
    });
    rechaza("opción por defecto enorme", || {
        spec_con(vec![campo(
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
fn fila_3_spec_error_opciones_vacias() {
    rechaza("opciones vacías", || {
        spec_con(vec![campo(
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
fn fila_3_spec_error_rotulo_largo() {
    rechaza("rótulo numérico de 13", || {
        spec_con(vec![numerico("n", "ABCDEFGHIJKLM", 1, 2, 1)])
    });
    rechaza("rótulo hexadecimal de 13", || {
        spec_con(vec![campo(
            "c",
            "ABCDEFGHIJKLM",
            FieldKind::Hex {
                mode: Mode::Rgba,
                default: Rgba::TRANSPARENT,
            },
        )])
    });
    let largo: &'static str = Box::leak("x".repeat(43).into_boxed_str());
    rechaza("rótulo de casilla de 43", || {
        spec_con(vec![campo("c", largo, FieldKind::Check { default: false })])
    });
}

#[test]
fn fila_3_spec_error_texto_fuera_del_juego() {
    rechaza("título", || {
        let mut s = spec_con(vec![]);
        s.title = "日本";
        s
    });
    rechaza("botón de aceptar", || {
        let mut s = spec_con(vec![]);
        s.accept = "✓";
        s
    });
    rechaza("botón de cancelar", || {
        let mut s = spec_con(vec![]);
        s.cancel = Some("日");
        s
    });
    rechaza("rótulo de un campo", || {
        spec_con(vec![campo(
            "c",
            "Año日",
            FieldKind::Check { default: false },
        )])
    });
    rechaza("opción de un grupo", || {
        spec_con(vec![campo(
            "m",
            "M",
            FieldKind::Choice {
                options: &["bien", "mal✗"],
                default: 0,
            },
        )])
    });
    // Los acentos y la eñe del español sí están.
    let mut s = spec_con(vec![campo(
        "c",
        "Año ¿sí?",
        FieldKind::Check { default: false },
    )]);
    s.title = "Información — ñandú · ×…";
    assert!(s.validate().is_ok());
}

#[test]
fn fila_3_spec_error_shown_when_invalido() {
    let modo = || {
        campo(
            "modo",
            "Modo",
            FieldKind::Choice {
                options: &["A", "B"],
                default: 0,
            },
        )
    };
    rechaza("shown_when de un campo que no existe", || {
        let mut s = spec_con(vec![
            modo(),
            campo("c", "C", FieldKind::Check { default: false }),
        ]);
        s.fields[1].shown_when = Some(ShowWhen {
            field: "nada",
            choice: 0,
        });
        s
    });
    rechaza("shown_when de un campo que no es Choice", || {
        let mut s = spec_con(vec![
            campo("k", "K", FieldKind::Check { default: false }),
            campo("c", "C", FieldKind::Check { default: false }),
        ]);
        s.fields[1].shown_when = Some(ShowWhen {
            field: "k",
            choice: 0,
        });
        s
    });
    rechaza("shown_when de un numérico", || {
        let mut s = spec_con(vec![
            numerico("n", "N", 1, 2, 1),
            campo("c", "C", FieldKind::Check { default: false }),
        ]);
        s.fields[1].shown_when = Some(ShowWhen {
            field: "n",
            choice: 0,
        });
        s
    });
    rechaza("shown_when con una opción que no existe", || {
        let mut s = spec_con(vec![
            modo(),
            campo("c", "C", FieldKind::Check { default: false }),
        ]);
        s.fields[1].shown_when = Some(ShowWhen {
            field: "modo",
            choice: 2,
        });
        s
    });
}

#[test]
fn fila_3_spec_error_fill_con_otros_campos_visibles() {
    let zona = |id: &'static str| {
        campo(
            id,
            "Zona",
            FieldKind::ReadOnly {
                text: "texto".to_string(),
                rows: Rows::Fill,
            },
        )
    };
    rechaza("Fill con una casilla", || {
        spec_con(vec![
            campo("c", "C", FieldKind::Check { default: false }),
            zona("z"),
        ])
    });
    rechaza("Fill con un numérico después", || {
        spec_con(vec![zona("z"), numerico("n", "N", 1, 2, 1)])
    });
    rechaza("dos Fill", || spec_con(vec![zona("z"), zona("y")]));
}

#[test]
fn fila_3_dialog_new_y_modal_open_aceptan_una_spec_valida() {
    assert!(Dialog::new(DialogKind::Licenses, all_kinds()).is_ok());
    let mut modal = Modal::default();
    assert!(modal.open(DialogKind::Licenses, all_kinds(), Locks::default()));
    assert!(modal.is_open());
}

// ---------------------------------------------------------------------------------------------
// Geometría que expone el estado: content_h_u, is_fill, scroll y primera línea
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_3_content_h_u_sigue_las_medidas_de_la_geometria() {
    // Con la opción 0: numérico 32 + numérico 32 + grupo (14 + 3·16 + 2·2 = 66) + casilla 16 +
    // informe (6 + 4·14 = 62) = 208, más 4 huecos de 4 y 16 de relleno.
    let mut d = dialog();
    assert_eq!(d.content_h_u(), 208 + 16 + 16);
    // Con la opción 2 se suma el hexadecimal (32) y un hueco.
    elige(&mut d, "modo", 2);
    assert_eq!(d.content_h_u(), 240 + 32 + 4);
    // Vacío: solo el relleno.
    assert_eq!(open(spec_con(vec![])).content_h_u(), 16);
    // N casillas: 16 de relleno + 16·N + 4·(N − 1).
    for n in [1usize, 2, 10, 64] {
        assert_eq!(
            open(huge(n)).content_h_u(),
            (16 + 16 * n + 4 * (n - 1)) as i64,
            "{n} casillas"
        );
    }
    // Un informe con menos líneas que filas mide las líneas que tiene.
    let spec = spec_con(vec![campo(
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
fn fila_3_un_formulario_tipico_de_la_19_mide_228() {
    // Dos numéricos, un grupo de 4 opciones, una casilla y un hexadecimal visible.
    let spec = spec_con(vec![
        numerico("ancho", "Ancho", 1, 8192, 64),
        numerico("alto", "Alto", 1, 8192, 64),
        campo(
            "modo",
            "Modo",
            FieldKind::Choice {
                options: &["A", "B", "C", "D"],
                default: 0,
            },
        ),
        campo("opaco", "Opaco", FieldKind::Check { default: false }),
        campo(
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
fn fila_3_un_formulario_con_fill_es_fill_y_mide_330() {
    let d = open(licenses_form("texto"));
    assert!(d.is_fill());
    assert_eq!(d.content_h_u(), 330);
    assert!(!dialog().is_fill());
}

#[test]
fn fila_3_el_desplazamiento_y_la_primera_linea_se_guardan_y_los_eventos_no_los_tocan() {
    let mut d = dialog();
    assert_eq!(d.scroll_u(), 0);
    assert_eq!(d.first_line("informe"), 0);
    d.set_scroll_u(100);
    d.set_first_line("informe", 7);
    assert_eq!(d.scroll_u(), 100);
    assert_eq!(d.first_line("informe"), 7);
    numero(&mut d, "ancho", "10");
    elige(&mut d, "modo", 1);
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
fn fila_4_un_modal_cerrado_no_reacciona() {
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
fn fila_4_open_abre_y_el_segundo_open_es_false_y_no_toca_los_valores() {
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
    assert_eq!(dialogo_de(&modal).text("ancho"), Some("100".to_string()));
    assert_eq!(dialogo_de(&modal).spec().title, "Prueba");
    assert_eq!(
        dialogo_de(&modal).values().unwrap().number("ancho"),
        Some(100)
    );
}

#[test]
fn fila_4_un_numerico_cambia_solo_su_campo() {
    let mut d = dialog();
    let antes_alto = d.text("alto");
    numero(&mut d, "ancho", "100");
    assert_eq!(d.text("ancho"), Some("100".to_string()));
    assert_eq!(d.text("alto"), antes_alto);
    let v = d.values().expect("válido");
    assert_eq!(v.number("ancho"), Some(100));
    assert_eq!(v.number("alto"), Some(64));
    assert_eq!(v.choice("modo"), Some(0));
    assert_eq!(v.check("opaco"), Some(false));
}

#[test]
fn fila_4_el_texto_numerico_se_guarda_acotado_a_once_caracteres() {
    let mut d = dialog();
    numero(&mut d, "ancho", "123456789012345");
    assert_eq!(d.text("ancho"), Some("12345678901".to_string()));
    assert_eq!(d.reason(), Some("Ancho: máximo 10 cifras".to_string()));
    let largo = "9".repeat(1_000_000);
    numero(&mut d, "ancho", &largo);
    let texto = d.text("ancho").expect("hay texto");
    assert_eq!(texto, bounded_text(&largo));
    assert!(texto.chars().count() <= 11);
    // Multibyte: se corta en frontera de carácter.
    numero(&mut d, "alto", &"é".repeat(13));
    assert_eq!(d.text("alto"), Some("é".repeat(11)));
}

#[test]
fn fila_4_step_suma_resta_y_se_para_en_los_limites() {
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
        numero(&mut d, "ancho", "abc");
        d.event(Event::Step {
            field: "ancho",
            delta,
        });
        assert_eq!(d.text("ancho"), Some("64".to_string()), "delta {delta}");
    }
    // En los límites.
    numero(&mut d, "ancho", "8192");
    d.event(Event::Step {
        field: "ancho",
        delta: 1,
    });
    assert_eq!(d.text("ancho"), Some("8192".to_string()));
    numero(&mut d, "ancho", "1");
    d.event(Event::Step {
        field: "ancho",
        delta: -1,
    });
    assert_eq!(d.text("ancho"), Some("1".to_string()));
    // Sin ceros a la izquierda.
    numero(&mut d, "ancho", "007");
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
fn fila_4_choice_y_check_cambian_solo_su_campo() {
    let mut d = dialog();
    elige(&mut d, "modo", 1);
    assert_eq!(d.values().unwrap().choice("modo"), Some(1));
    assert!(!d.is_shown("color"));
    elige(&mut d, "modo", 2);
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
fn fila_4_los_eventos_invalidos_se_ignoran_sin_panico() {
    let base = dialog();
    let invalidos = [
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
    for evento in invalidos {
        let mut d = base.clone();
        assert_eq!(d.event(evento.clone()), Reaction::None, "{evento:?}");
        assert_eq!(d, base, "el evento {evento:?} cambió el estado");
    }
}

#[test]
fn fila_4_accept_valido_entrega_lo_confirmado_y_cierra() {
    let mut modal = modal_abierto(all_kinds());
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
    let antes = dialogo_de(&modal).values().expect("válido");
    let Reaction::Accepted(v) = modal.event(Event::Accept) else {
        panic!("se esperaba Accepted");
    };
    assert_eq!(v, antes);
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
fn fila_4_accept_no_entrega_campos_ocultos_aunque_tengan_algo_escrito() {
    let mut modal = modal_abierto(all_kinds());
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
fn fila_4_accept_con_hexadecimal_visible_da_el_color_canonico() {
    let mut modal = modal_abierto(all_kinds());
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
fn fila_4_accept_invalido_da_rejected_con_el_motivo_sin_cambiar_ni_cerrar() {
    let mut modal = modal_abierto(all_kinds());
    modal.event(Event::Number {
        field: "ancho",
        text: "".into(),
    });
    let antes = dialogo_de(&modal).clone();
    let motivo = antes.reason().expect("inválido");
    assert_eq!(modal.event(Event::Accept), Reaction::Rejected(motivo));
    assert!(modal.is_open());
    assert_eq!(dialogo_de(&modal), &antes);
    // También con el `check` que falla.
    let mut modal = modal_abierto(all_kinds());
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
fn fila_4_cancel_da_cancelled_y_nunca_valores() {
    let mut modal = modal_abierto(all_kinds());
    modal.event(Event::Number {
        field: "ancho",
        text: "100".into(),
    });
    assert_eq!(modal.event(Event::Cancel), Reaction::Cancelled);
    assert!(!modal.is_open());
    // Con el formulario inválido también.
    let mut modal = modal_abierto(all_kinds());
    modal.event(Event::Number {
        field: "ancho",
        text: "".into(),
    });
    assert_eq!(modal.event(Event::Cancel), Reaction::Cancelled);
    assert!(!modal.is_open());
}

#[test]
fn fila_4_tras_cerrar_todo_da_none() {
    for cierre in [Event::Accept, Event::Cancel] {
        let mut modal = modal_abierto(all_kinds());
        let primera = modal.event(cierre.clone());
        assert!(matches!(
            primera,
            Reaction::Accepted(_) | Reaction::Cancelled
        ));
        assert!(!modal.is_open());
        for evento in [
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
            assert_eq!(modal.event(evento.clone()), Reaction::None, "{evento:?}");
            assert!(!modal.is_open());
        }
        assert!(!modal.abandon());
        assert!(!modal.close_kind(DialogKind::Licenses));
    }
}

#[test]
fn fila_4_abandon_da_true_y_luego_false_sin_reaccion() {
    let mut modal = modal_abierto(all_kinds());
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
fn fila_4_se_puede_reabrir_tras_cerrar_con_el_estado_limpio() {
    let mut modal = modal_abierto(all_kinds());
    modal.event(Event::Number {
        field: "ancho",
        text: "100".into(),
    });
    assert!(modal.abandon());
    assert!(modal.open(DialogKind::Licenses, all_kinds(), Locks::default()));
    assert_eq!(dialogo_de(&modal).text("ancho"), Some("64".to_string()));
}

#[test]
fn fila_4_close_kind_cierra_el_tipo_abierto() {
    let mut modal = modal_abierto(licenses_form("texto"));
    assert!(modal.close_kind(DialogKind::Licenses));
    assert!(!modal.is_open());
    assert!(!modal.close_kind(DialogKind::Licenses));
}

#[test]
fn fila_4_hexadecimal_escribir_no_confirma() {
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
fn fila_4_hexadecimal_intro_valido_confirma_y_muestra_hex_text() {
    let def = Rgba::new(0x33, 0x66, 0x99, 0x80);
    let mut d = open(hex_form(Mode::Rgba, def));
    hex(&mut d, "color", "#abc");
    confirma(&mut d, "color", false);
    // 3 dígitos: conserva el alfa confirmado (0x80).
    assert_eq!(d.text("color"), Some("#aabbcc80".to_string()));
    assert!(!d.hex_pending("color"));
    assert!(d.is_valid());
    assert_eq!(color_aceptado(&d), Rgba::new(0xaa, 0xbb, 0xcc, 0x80));
}

#[test]
fn fila_4_hexadecimal_intro_invalido_conserva_el_texto_y_el_error() {
    let def = Rgba::new(0x33, 0x66, 0x99, 0x80);
    let mut d = open(hex_form(Mode::Rgba, def));
    hex(&mut d, "color", "zz");
    let motivo = d.reason();
    assert!(motivo.is_some());
    confirma(&mut d, "color", false);
    assert_eq!(d.text("color"), Some("zz".to_string()));
    assert!(d.hex_pending("color"));
    assert!(!d.is_valid());
    assert_eq!(d.reason(), motivo);
    // Y sigue sin ser válido el conjunto: Aceptar se rechaza.
    assert!(matches!(d.event(Event::Accept), Reaction::Rejected(_)));
}

#[test]
fn fila_4_hexadecimal_blur_valido_confirma() {
    let def = Rgba::new(0x33, 0x66, 0x99, 0x80);
    let mut d = open(hex_form(Mode::Rgba, def));
    hex(&mut d, "color", "#abc");
    confirma(&mut d, "color", true);
    assert_eq!(d.text("color"), Some("#aabbcc80".to_string()));
    assert!(!d.hex_pending("color"));
    assert_eq!(color_aceptado(&d), Rgba::new(0xaa, 0xbb, 0xcc, 0x80));
}

#[test]
fn fila_4_hexadecimal_blur_invalido_revierte_al_ultimo_confirmado() {
    let def = Rgba::new(0x33, 0x66, 0x99, 0x80);
    let mut d = open(hex_form(Mode::Rgba, def));
    hex(&mut d, "color", "zz");
    confirma(&mut d, "color", true);
    assert_eq!(d.text("color"), Some("#33669980".to_string()));
    assert!(!d.hex_pending("color"));
    assert!(d.is_valid());
    assert_eq!(d.reason(), None);
    assert_eq!(color_aceptado(&d), def);
    // Revierte al último **confirmado**, no al valor por defecto.
    hex(&mut d, "color", "#123456");
    confirma(&mut d, "color", false);
    assert_eq!(d.text("color"), Some("#12345680".to_string()));
    hex(&mut d, "color", "nope");
    confirma(&mut d, "color", true);
    assert_eq!(d.text("color"), Some("#12345680".to_string()));
}

#[test]
fn fila_4_hexadecimal_revert_vuelve_al_confirmado() {
    let def = Rgba::new(0x33, 0x66, 0x99, 0x80);
    for texto in ["zz", "#abc", ""] {
        let mut d = open(hex_form(Mode::Rgba, def));
        hex(&mut d, "color", texto);
        assert_eq!(d.event(Event::HexRevert { field: "color" }), Reaction::None);
        assert_eq!(d.text("color"), Some("#33669980".to_string()), "{texto:?}");
        assert!(!d.hex_pending("color"), "{texto:?}");
        assert!(d.is_valid(), "{texto:?}");
    }
}

#[test]
fn fila_4_hexadecimal_tres_o_seis_digitos_conservan_el_alfa_y_cuatro_u_ocho_el_escrito() {
    let def = Rgba::new(0x33, 0x66, 0x99, 0x80);
    let casos = [
        ("#f00", Rgba::new(0xff, 0, 0, 0x80)),
        ("#ff0000", Rgba::new(0xff, 0, 0, 0x80)),
        ("f00", Rgba::new(0xff, 0, 0, 0x80)),
        ("#f008", Rgba::new(0xff, 0, 0, 0x88)),
        ("#ff000040", Rgba::new(0xff, 0, 0, 0x40)),
        ("#ff0000ff", Rgba::new(0xff, 0, 0, 0xff)),
    ];
    for (texto, esperado) in casos {
        let mut d = open(hex_form(Mode::Rgba, def));
        hex(&mut d, "color", texto);
        confirma(&mut d, "color", false);
        assert_eq!(color_aceptado(&d), esperado, "{texto:?}");
        assert_eq!(d.text("color"), Some(esperado.to_string()), "{texto:?}");
    }
    // El alfa que se conserva es el **confirmado**, no el de partida.
    let mut d = open(hex_form(Mode::Rgba, def));
    hex(&mut d, "color", "#f008");
    confirma(&mut d, "color", false);
    hex(&mut d, "color", "#0f0");
    confirma(&mut d, "color", false);
    assert_eq!(color_aceptado(&d), Rgba::new(0, 0xff, 0, 0x88));
    hex(&mut d, "color", "#00ff00");
    confirma(&mut d, "color", true);
    assert_eq!(color_aceptado(&d), Rgba::new(0, 0xff, 0, 0x88));
    // Un color opaco se muestra sin los dígitos del alfa.
    let mut d = open(hex_form(Mode::Rgba, Rgba::opaque(1, 2, 3)));
    hex(&mut d, "color", "#f00");
    confirma(&mut d, "color", false);
    assert_eq!(d.text("color"), Some("#ff0000".to_string()));
}

#[test]
fn fila_4_accept_con_un_pendiente_valido_lo_confirma_antes_de_entregar() {
    let def = Rgba::new(0x33, 0x66, 0x99, 0x80);
    let mut d = open(hex_form(Mode::Rgba, def));
    hex(&mut d, "color", "#abc");
    let Reaction::Accepted(v) = d.event(Event::Accept) else {
        panic!("se esperaba Accepted");
    };
    assert_eq!(v.color("color"), Some(Rgba::new(0xaa, 0xbb, 0xcc, 0x80)));
}

#[test]
fn fila_4_accept_con_un_pendiente_invalido_se_rechaza_sin_cambiar_nada() {
    let def = Rgba::new(0x33, 0x66, 0x99, 0x80);
    let mut d = open(hex_form(Mode::Rgba, def));
    hex(&mut d, "color", "zz");
    let antes = d.clone();
    let motivo = d.reason().expect("inválido");
    assert_eq!(d.event(Event::Accept), Reaction::Rejected(motivo));
    assert_eq!(d, antes);
    assert_eq!(d.text("color"), Some("zz".to_string()));
    assert!(d.hex_pending("color"));
    // Lo mismo a través del `Modal`: sigue abierto con el pendiente.
    let mut modal = modal_abierto(hex_form(Mode::Rgba, def));
    modal.event(Event::Hex {
        field: "color",
        text: "zz".into(),
    });
    assert!(matches!(modal.event(Event::Accept), Reaction::Rejected(_)));
    assert!(modal.is_open());
    assert_eq!(dialogo_de(&modal).text("color"), Some("zz".to_string()));
}

#[test]
fn fila_4_escribir_en_un_hexadecimal_no_toca_los_demas_campos() {
    let mut d = dialog();
    elige(&mut d, "modo", 2);
    numero(&mut d, "ancho", "10");
    hex(&mut d, "color", "#123");
    assert_eq!(d.text("ancho"), Some("10".to_string()));
    assert_eq!(d.text("alto"), Some("64".to_string()));
    assert_eq!(d.values().expect("válido").choice("modo"), Some(2));
}

// ---------------------------------------------------------------------------------------------
// take_focus_request
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_4_take_focus_request_da_some_una_vez_tras_new_y_luego_none() {
    let mut d = dialog();
    let inicial = initial_focus(&d);
    assert_eq!(d.take_focus_request(), Some(inicial));
    assert_eq!(d.take_focus_request(), None);
    assert_eq!(d.take_focus_request(), None);
    // Los eventos no vuelven a pedirlo.
    numero(&mut d, "ancho", "5");
    elige(&mut d, "modo", 2);
    assert_eq!(d.take_focus_request(), None);
    // Un diálogo nuevo vuelve a pedirlo.
    let mut otro = dialog();
    assert!(otro.take_focus_request().is_some());
}

#[test]
fn fila_4_el_foco_inicial_pedido_es_el_primer_campo_o_la_zona_en_licencias() {
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
fn fila_4_take_focus_request_a_traves_del_modal() {
    let mut modal = modal_abierto(all_kinds());
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
struct Modelo {
    choices: Vec<usize>,
    checks: Vec<bool>,
    numbers: Vec<String>,
    /// Campos a los que llegó un evento mientras estaban ocultos: la spec no dice qué pasa.
    tainted: Vec<bool>,
}

impl Modelo {
    fn new(spec: &FormSpec) -> Modelo {
        let mut m = Modelo {
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

    fn visibles(&self, spec: &FormSpec) -> Vec<bool> {
        (0..spec.fields.len())
            .map(|i| self.visible(spec, i))
            .collect()
    }

    /// Aplica el evento al modelo. Devuelve `(campo tocado, debe ignorarse)`.
    fn aplica(&mut self, spec: &FormSpec, event: &Event) -> (Option<usize>, bool) {
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
fn comprueba_estado(d: &Dialog, spec: &FormSpec, m: &Modelo) -> Result<(), TestCaseError> {
    let n = spec.fields.len();
    let vis = m.visibles(spec);
    let mut primero_invalido: Option<(usize, String)> = None;
    let mut suma: u64 = 0;
    for i in 0..n {
        let id = IDS[i];
        prop_assert_eq!(d.is_shown(id), vis[i], "is_shown de {}", id);
        let validity = d.validity(id);
        let invalido = match &validity {
            Some(Validity::Invalid(motivo)) => Some(motivo.clone()),
            _ => None,
        };
        match &spec.fields[i].kind {
            FieldKind::Number { min, max, .. } => {
                let texto = d.text(id).expect("un numérico siempre tiene texto");
                prop_assert!(texto.chars().count() <= 11, "texto numérico {:?}", texto);
                if !m.tainted[i] {
                    prop_assert_eq!(&texto, &m.numbers[i], "texto de {}", id);
                }
                let analisis = parse_number(&texto, *min, *max);
                if vis[i] {
                    prop_assert_eq!(analisis.is_err(), invalido.is_some(), "validez de {}", id);
                    if let Ok(v) = analisis {
                        suma += u64::from(v);
                    }
                }
            }
            FieldKind::Hex { .. } => {
                prop_assert!(d.text(id).is_some());
                if vis[i] && !d.hex_pending(id) {
                    prop_assert!(invalido.is_none(), "un hexadecimal sin pendiente es válido");
                }
            }
            _ => {
                if vis[i] {
                    prop_assert!(invalido.is_none());
                }
            }
        }
        if vis[i]
            && primero_invalido.is_none()
            && let Some(motivo) = invalido
        {
            primero_invalido = Some((i, motivo));
        }
    }
    // Los ids que no existen nunca son nada.
    for id in IDS.iter().skip(n).chain(["zz"].iter()).copied() {
        prop_assert!(!d.is_shown(id));
        prop_assert!(d.validity(id).is_none());
        prop_assert!(d.text(id).is_none());
        prop_assert!(!d.hex_pending(id));
    }

    let check_falla = spec.check.is_some() && primero_invalido.is_none() && suma % 2 == 1;
    let esperado_valido = primero_invalido.is_none() && !check_falla;
    prop_assert_eq!(d.is_valid(), esperado_valido);
    let reason = d.reason();
    prop_assert_eq!(reason.is_none(), esperado_valido);
    let values = d.values();
    prop_assert_eq!(values.is_some(), esperado_valido);
    if let Some(r) = &reason {
        prop_assert!(r.chars().count() <= 37, "motivo largo {:?}", r);
        prop_assert!(en_el_juego(r), "motivo fuera del juego {:?}", r);
    }
    if let Some((i, motivo)) = &primero_invalido {
        let r = reason.expect("hay un campo inválido");
        prop_assert!(
            r.starts_with(LABELS[*i]),
            "{:?} no empieza por {:?}",
            r,
            LABELS[*i]
        );
        prop_assert!(
            r.contains(motivo.as_str()),
            "{:?} no contiene {:?}",
            r,
            motivo
        );
    } else if check_falla {
        prop_assert_eq!(reason, Some(SUMA_IMPAR.to_string()));
    }

    if let Some(v) = values {
        for i in 0..n {
            let id = IDS[i];
            let hay = vis[i] && !matches!(spec.fields[i].kind, FieldKind::ReadOnly { .. });
            prop_assert_eq!(v.get(id).is_some(), hay, "{} en los valores", id);
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
                    let esperado = parse_number(&d.text(id).unwrap(), *min, *max).ok();
                    prop_assert_eq!(v.number(id), esperado);
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
fn valores_iguales(a: &Values, b: &Values, pendiente: &[bool]) -> bool {
    IDS.iter()
        .enumerate()
        .all(|(i, id)| pendiente.get(i).copied().unwrap_or(false) || a.get(id) == b.get(id))
}

fn ejecuta(recipe: &FormRecipe, events: &[EventRecipe]) -> Result<(), TestCaseError> {
    let spec = build(recipe);
    prop_assert!(spec.validate().is_ok(), "la spec al azar no valida");
    let n = spec.fields.len();
    let mut model = Modelo::new(&spec);
    let mut modal = Modal::default();
    prop_assert!(modal.open(DialogKind::Licenses, build(recipe), Locks::default()));
    comprueba_estado(dialogo_de(&modal), &spec, &model)?;

    let mut abierto = true;
    for r in events {
        let evento = event_of(r);
        if !abierto {
            // Tras cerrar, todo da `None`.
            match evento {
                Some(e) => prop_assert_eq!(modal.event(e), Reaction::None),
                None => prop_assert!(!modal.abandon()),
            }
            prop_assert!(!modal.is_open());
            continue;
        }
        let antes = dialogo_de(&modal).clone();
        let valido_antes = antes.is_valid();
        let motivo_antes = antes.reason();
        let valores_antes = antes.values();
        let textos_antes: Vec<Option<String>> = IDS.iter().map(|id| antes.text(id)).collect();
        let pendiente_antes: Vec<bool> =
            IDS.iter().take(n).map(|id| antes.hex_pending(id)).collect();
        let visibles_antes = model.visibles(&spec);

        let Some(evento) = evento else {
            prop_assert_eq!(r.kind, KIND_ABANDON);
            prop_assert!(modal.abandon());
            prop_assert!(!modal.is_open());
            prop_assert!(!modal.abandon());
            abierto = false;
            continue;
        };

        let (tocado, ignorado) = model.aplica(&spec, &evento);
        let reaccion = modal.event(evento.clone());
        match (&evento, reaccion) {
            (Event::Accept, Reaction::Accepted(v)) => {
                prop_assert!(valido_antes, "Accepted con el conjunto inválido");
                let antes_v = valores_antes.expect("válido: había valores");
                prop_assert!(valores_iguales(&v, &antes_v, &pendiente_antes));
                for i in 0..n {
                    let hay = visibles_antes[i]
                        && !matches!(spec.fields[i].kind, FieldKind::ReadOnly { .. });
                    prop_assert_eq!(v.get(IDS[i]).is_some(), hay, "{} en lo aceptado", IDS[i]);
                }
                prop_assert!(!modal.is_open());
                abierto = false;
            }
            (Event::Accept, Reaction::Rejected(motivo)) => {
                prop_assert!(!valido_antes, "Rejected con el conjunto válido");
                prop_assert_eq!(Some(motivo), motivo_antes);
                prop_assert!(modal.is_open());
                prop_assert_eq!(dialogo_de(&modal), &antes);
            }
            (Event::Cancel, Reaction::Cancelled) => {
                prop_assert!(!modal.is_open());
                abierto = false;
            }
            (Event::Accept | Event::Cancel, otra) => {
                prop_assert!(false, "{:?} dio {:?}", evento, otra);
            }
            (_, Reaction::None) => {
                prop_assert!(modal.is_open());
                if ignorado {
                    prop_assert_eq!(
                        dialogo_de(&modal),
                        &antes,
                        "un evento ignorado cambió el estado"
                    );
                }
                // Un evento solo cambia el texto de su campo.
                let d = dialogo_de(&modal);
                for (i, id) in IDS.iter().copied().enumerate() {
                    if Some(i) != tocado {
                        prop_assert_eq!(
                            d.text(id),
                            textos_antes[i].clone(),
                            "{} cambió sin evento",
                            id
                        );
                    }
                }
            }
            (_, otra) => {
                prop_assert!(false, "{:?} dio {:?}", evento, otra);
            }
        }
        if abierto {
            comprueba_estado(dialogo_de(&modal), &spec, &model)?;
        }
    }

    // Al final, si sigue abierto: abandonar no entrega nada y luego todo da `None`.
    if abierto {
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
    fn propiedad_2_el_estado_nunca_se_corrompe(
        recipe in form_recipe(),
        events in prop::collection::vec(event_recipe(), 0..=64),
    ) {
        ejecuta(&recipe, &events)?;
    }

    /// `target_id` y `build` son coherentes entre sí (guarda de la propia prueba): los ids de la
    /// spec son los de `IDS` y no se repiten.
    #[test]
    fn propiedad_2_los_formularios_al_azar_son_validos(recipe in form_recipe()) {
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
fn propiedad_2_los_ocultos_conservan_lo_escrito_en_una_secuencia_fija() {
    let mut d = dialog();
    elige(&mut d, "modo", 2);
    hex(&mut d, "color", "#12345");
    for index in [0, 1, 2, 0, 1, 0] {
        elige(&mut d, "modo", index);
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
// - E2: `Dialog::set_report(id, texto) -> bool`, el texto de un campo `ReadOnly`.
// - E3: `Dialog::set_limit_note(id, nota) -> bool`, el `Above { max }` de un numérico pasa a decir
//   `máximo N (nota)`.
// - Más `DialogKind::NewImage`.
//
// Ninguna prueba anterior de este fichero cambia (esta sección solo añade). Suposición: el texto
// de un campo `ReadOnly` se lee de `Dialog::spec()` (el `FieldKind::ReadOnly { text, .. }` del
// campo): `set_report` lo cambia ahí, que es donde `content_h_u` lo mide. Se usan los ayudantes de
// arriba (`open`, `campo`, `numerico`, `spec_con`, `numero`, `elige`, `hex`).

const MODOS_19: &[&str] = &["Uno", "Dos", "Tres"];
const FONDOS_19: &[&str] = &["Transparente", "Blanco", "Negro", "Color…"];

fn campo_si(
    id: &'static str,
    label: &'static str,
    kind: FieldKind,
    control: &'static str,
    opcion: usize,
) -> FieldSpec {
    FieldSpec {
        id,
        label,
        kind,
        shown_when: Some(ShowWhen {
            field: control,
            choice: opcion,
        }),
    }
}

/// `modo` (por defecto la opción 1) → `fondo` (se ve con `modo` = 1) → `color` (se ve con
/// `fondo` = 3) → `informe`: la cadena de dos niveles del formulario de nueva imagen.
fn spec_en_cadena() -> FormSpec {
    spec_con(vec![
        campo(
            "modo",
            "Modo",
            FieldKind::Choice {
                options: MODOS_19,
                default: 1,
            },
        ),
        campo_si(
            "fondo",
            "Fondo",
            FieldKind::Choice {
                options: FONDOS_19,
                default: 0,
            },
            "modo",
            1,
        ),
        campo_si(
            "color",
            "Color",
            FieldKind::Hex {
                mode: Mode::Rgba,
                default: Rgba::opaque(0x80, 0x80, 0x80),
            },
            "fondo",
            3,
        ),
        campo(
            "informe",
            "Informe",
            FieldKind::ReadOnly {
                text: "uno".to_string(),
                rows: Rows::Fixed(4),
            },
        ),
    ])
}

fn en_cadena() -> Dialog {
    open(spec_en_cadena())
}

// --- E1 ---

#[test]
fn fila_12_e1_la_cadena_de_ejemplo_es_una_spec_valida() {
    spec_en_cadena().validate().expect("spec válida");
    assert!(Dialog::new(DialogKind::NewImage, spec_en_cadena()).is_ok());
}

#[test]
fn fila_12_e1_un_campo_con_el_control_oculto_esta_oculto_aunque_el_control_valga_la_opcion() {
    let mut d = en_cadena();
    // Inicio: modo = 1 → fondo se ve; fondo = 0 → color no.
    assert!(d.is_shown("fondo"));
    assert!(!d.is_shown("color"));
    elige(&mut d, "fondo", 3);
    assert!(d.is_shown("color"), "fondo = 3 con el modo que lo muestra");

    // Se oculta el control (`fondo`): `color` también, aunque `fondo` siga valiendo 3.
    elige(&mut d, "modo", 0);
    assert!(!d.is_shown("fondo"));
    assert!(
        !d.is_shown("color"),
        "el control de color (fondo) está oculto: color también"
    );

    // Y vuelven juntos al volver el modo.
    elige(&mut d, "modo", 1);
    assert!(d.is_shown("fondo"));
    assert!(d.is_shown("color"), "fondo conservaba el 3");
}

#[test]
fn fila_12_e1_la_visibilidad_de_cada_combinacion_de_modo_y_fondo() {
    for modo in 0..3 {
        for fondo in 0..4 {
            let mut d = en_cadena();
            elige(&mut d, "modo", modo);
            elige(&mut d, "fondo", fondo);
            assert!(d.is_shown("modo"), "modo {modo}, fondo {fondo}");
            assert!(d.is_shown("informe"), "modo {modo}, fondo {fondo}");
            assert_eq!(d.is_shown("fondo"), modo == 1, "modo {modo}, fondo {fondo}");
            assert_eq!(
                d.is_shown("color"),
                modo == 1 && fondo == 3,
                "modo {modo}, fondo {fondo}"
            );
        }
    }
}

#[test]
fn fila_12_e1_un_campo_oculto_por_transitividad_conserva_lo_escrito() {
    let mut d = en_cadena();
    elige(&mut d, "fondo", 3);
    hex(&mut d, "color", "#12345");
    for modo in [0, 2, 1, 0, 1] {
        elige(&mut d, "modo", modo);
        assert_eq!(
            d.text("color"),
            Some("#12345".to_string()),
            "modo {modo}: lo escrito se conserva oculto"
        );
    }
    assert!(d.hex_pending("color"));
}

#[test]
fn fila_12_e1_lo_oculto_por_transitividad_no_cuenta_en_validez_valores_ni_foco() {
    let mut d = en_cadena();
    elige(&mut d, "fondo", 3);
    hex(&mut d, "color", "zz");
    assert!(!d.is_valid(), "con el hexadecimal visible e inválido");
    assert!(d.any_hex_pending());
    let alto_visible = d.content_h_u();

    elige(&mut d, "modo", 0);
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
    let orden = px_ui::dialog_keys::focus_order(&d);
    for &objetivo in &orden {
        match objetivo {
            FocusTarget::Field(id)
            | FocusTarget::Option(id, _)
            | FocusTarget::Minus(id)
            | FocusTarget::Plus(id)
            | FocusTarget::Zone(id)
            | FocusTarget::Button(id)
            | FocusTarget::ButtonAside(id) => {
                assert!(
                    d.is_shown(id),
                    "{objetivo:?} está oculto y está en el orden"
                )
            }
            FocusTarget::Accept | FocusTarget::Cancel => {}
        }
    }
    assert!(!orden.contains(&FocusTarget::Field("color")));
    assert!(!orden.contains(&FocusTarget::Option("fondo", 0)));
    assert!(d.content_h_u() < alto_visible);
}

#[test]
fn fila_12_e1_una_cadena_de_cuatro_niveles_se_oculta_entera_con_el_primer_control() {
    const SI_NO: &[&str] = &["No", "Si"];
    let eleccion = |default| FieldKind::Choice {
        options: SI_NO,
        default,
    };
    let spec = spec_con(vec![
        campo("a", "A", eleccion(1)),
        campo_si("b", "B", eleccion(1), "a", 1),
        campo_si("c", "C", eleccion(1), "b", 1),
        campo_si("d", "D", eleccion(1), "c", 1),
    ]);
    spec.validate().expect("spec válida");
    let mut d = open(spec);
    for id in ["a", "b", "c", "d"] {
        assert!(d.is_shown(id), "todo a 1: {id} se ve");
    }
    elige(&mut d, "a", 0);
    assert!(d.is_shown("a"));
    for id in ["b", "c", "d"] {
        assert!(!d.is_shown(id), "a = 0 oculta {id} aunque valga 1");
    }
    elige(&mut d, "a", 1);
    elige(&mut d, "c", 0);
    assert!(d.is_shown("b") && d.is_shown("c"));
    assert!(!d.is_shown("d"), "c = 0 oculta d");
}

#[test]
fn fila_12_e1_con_el_control_visible_todo_sigue_como_antes() {
    // El caso de un solo nivel de la unidad 18 no cambia: oculto solo si la opción no vale.
    let mut d = dialog();
    assert!(!d.is_shown("color"));
    elige(&mut d, "modo", 2);
    assert!(d.is_shown("color"));
    elige(&mut d, "modo", 1);
    assert!(!d.is_shown("color"));
}

#[test]
fn fila_12_dialog_kind_new_image_abre_un_dialogo_y_se_cierra_por_su_tipo() {
    let d = Dialog::new(DialogKind::NewImage, spec_en_cadena()).expect("spec válida");
    assert_eq!(d.kind(), DialogKind::NewImage);
    assert_ne!(DialogKind::NewImage, DialogKind::Licenses);
    assert_ne!(DialogKind::NewImage, DialogKind::Demo);

    let mut modal = Modal::default();
    assert!(modal.open(DialogKind::NewImage, spec_en_cadena(), Locks::default()));
    assert_eq!(modal.kind(), Some(DialogKind::NewImage));
    assert!(!modal.close_kind(DialogKind::Licenses), "es de otro tipo");
    assert!(modal.is_open());
    assert!(modal.close_kind(DialogKind::NewImage));
    assert!(!modal.is_open());
}

// --- E2 ---

/// El texto de un campo `ReadOnly`, leído de la spec del diálogo.
fn texto_del_informe_de(d: &Dialog, id: &str) -> Option<String> {
    d.spec().fields.iter().find_map(|f| match &f.kind {
        FieldKind::ReadOnly { text, .. } if f.id == id => Some(text.clone()),
        _ => None,
    })
}

/// Un informe con `lineas` líneas «linea N».
fn lineas(n: usize) -> String {
    (1..=n)
        .map(|i| format!("linea {i}"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn fila_12_e2_set_report_cambia_el_texto_de_un_read_only() {
    let mut d = en_cadena();
    assert_eq!(texto_del_informe_de(&d, "informe"), Some("uno".to_string()));
    assert!(d.set_report("informe", "otro texto".to_string()));
    assert_eq!(
        texto_del_informe_de(&d, "informe"),
        Some("otro texto".to_string())
    );
    assert!(d.set_report("informe", String::new()));
    assert_eq!(texto_del_informe_de(&d, "informe"), Some(String::new()));
}

#[test]
fn fila_12_e2_set_report_cambia_el_alto_del_contenido_con_el_numero_de_lineas() {
    let mut d = en_cadena();
    let una = d.content_h_u();
    // 14 u por fila hasta las 4 de `Rows::Fixed(4)`; una vacía cuenta como una línea.
    for (n, extra) in [(1, 0), (2, 14), (3, 28), (4, 42), (5, 42), (9, 42)] {
        assert!(d.set_report("informe", lineas(n)));
        assert_eq!(d.content_h_u(), una + extra, "{n} líneas");
    }
    assert!(d.set_report("informe", String::new()));
    assert_eq!(d.content_h_u(), una, "vacío mide como una línea");
    assert!(d.set_report("informe", lineas(3)));
    assert!(d.set_report("informe", "uno".to_string()));
    assert_eq!(d.content_h_u(), una, "vuelve al alto de una línea");
}

#[test]
fn fila_12_e2_set_report_no_cambia_valores_validez_ni_los_demas_campos() {
    let mut d = en_cadena();
    elige(&mut d, "fondo", 3);
    hex(&mut d, "color", "#abc");
    let valores = d.values();
    let motivo = d.reason();
    let color = d.text("color");
    let visibles: Vec<bool> = ["modo", "fondo", "color", "informe"]
        .iter()
        .map(|id| d.is_shown(id))
        .collect();
    assert!(d.set_report("informe", lineas(3)));
    assert_eq!(d.values(), valores);
    assert_eq!(d.reason(), motivo);
    assert_eq!(d.text("color"), color);
    let ahora: Vec<bool> = ["modo", "fondo", "color", "informe"]
        .iter()
        .map(|id| d.is_shown(id))
        .collect();
    assert_eq!(ahora, visibles);
}

#[test]
fn fila_12_e2_set_report_da_false_si_el_campo_no_existe_o_no_es_read_only_y_no_cambia_nada() {
    let mut d = open(spec_con(vec![
        numerico("ancho", "Ancho", 1, 100, 64),
        campo(
            "modo",
            "Modo",
            FieldKind::Choice {
                options: MODOS_19,
                default: 0,
            },
        ),
        campo("opaco", "Opaco", FieldKind::Check { default: false }),
        campo(
            "color",
            "Color",
            FieldKind::Hex {
                mode: Mode::Rgba,
                default: Rgba::opaque(1, 2, 3),
            },
        ),
        campo(
            "informe",
            "Informe",
            FieldKind::ReadOnly {
                text: "uno".to_string(),
                rows: Rows::Fixed(2),
            },
        ),
    ]));
    let antes = d.clone();
    for id in ["inexistente", "ancho", "modo", "opaco", "color", ""] {
        assert!(!d.set_report(id, "x".to_string()), "{id:?}");
        assert_eq!(d, antes, "{id:?}: no cambió nada");
    }
    assert!(d.set_report("informe", "x".to_string()), "el ReadOnly sí");
    assert_ne!(d, antes);
}

// --- E3 ---

fn dos_numericos() -> Dialog {
    open(spec_con(vec![
        numerico("ancho", "Ancho", 1, 4096, 64),
        numerico("alto", "Alto", 1, 4096, 64),
        campo(
            "modo",
            "Modo",
            FieldKind::Choice {
                options: MODOS_19,
                default: 0,
            },
        ),
    ]))
}

fn invalido(texto: &str) -> Option<Validity> {
    Some(Validity::Invalid(texto.to_string()))
}

#[test]
fn fila_12_e3_set_limit_note_cambia_el_texto_de_above_a_maximo_n_nota() {
    let mut d = dos_numericos();
    numero(&mut d, "ancho", "4097");
    assert_eq!(d.validity("ancho"), invalido("máximo 4096"), "sin nota");
    assert_eq!(d.reason(), Some("Ancho: máximo 4096".to_string()));

    assert!(d.set_limit_note("ancho", Some("tarjeta gráfica")));
    assert_eq!(
        d.validity("ancho"),
        invalido("máximo 4096 (tarjeta gráfica)")
    );
    assert_eq!(
        d.reason(),
        Some("Ancho: máximo 4096 (tarjeta gráfica)".to_string())
    );

    assert!(d.set_limit_note("ancho", Some("otra nota")));
    assert_eq!(d.validity("ancho"), invalido("máximo 4096 (otra nota)"));
}

#[test]
fn fila_12_e3_none_devuelve_el_texto_de_siempre_y_restaura_el_dialogo() {
    let original = dos_numericos();
    let mut d = original.clone();
    assert!(d.set_limit_note("ancho", Some("tarjeta gráfica")));
    assert_ne!(d, original, "la nota se guarda");
    numero(&mut d, "ancho", "4097");
    assert_eq!(
        d.validity("ancho"),
        invalido("máximo 4096 (tarjeta gráfica)")
    );
    assert!(d.set_limit_note("ancho", None));
    assert_eq!(d.validity("ancho"), invalido("máximo 4096"));
    numero(&mut d, "ancho", "64");
    assert_eq!(
        d, original,
        "sin nota y con el texto inicial es el de partida"
    );
}

#[test]
fn fila_12_e3_solo_afecta_al_campo_pedido() {
    let mut d = dos_numericos();
    assert!(d.set_limit_note("ancho", Some("tarjeta gráfica")));
    numero(&mut d, "alto", "4097");
    assert_eq!(
        d.validity("alto"),
        invalido("máximo 4096"),
        "alto no tiene nota"
    );
    assert!(d.set_limit_note("alto", Some("tarjeta gráfica")));
    assert_eq!(
        d.validity("alto"),
        invalido("máximo 4096 (tarjeta gráfica)")
    );
    assert_eq!(
        d.reason(),
        Some("Alto: máximo 4096 (tarjeta gráfica)".to_string())
    );
}

#[test]
fn fila_12_e3_los_demas_errores_no_cambian_con_nota() {
    let mut d = dos_numericos();
    assert!(d.set_limit_note("ancho", Some("tarjeta gráfica")));
    for (texto, esperado) in [
        ("0", invalido("mínimo 1")),
        ("", invalido("escribe un número")),
        ("abc", invalido("solo dígitos 0-9")),
        ("12345678901", invalido("máximo 10 cifras")),
        ("1", Some(Validity::Valid)),
        ("4096", Some(Validity::Valid)),
        ("4097", invalido("máximo 4096 (tarjeta gráfica)")),
        ("99999999", invalido("máximo 4096 (tarjeta gráfica)")),
    ] {
        numero(&mut d, "ancho", texto);
        assert_eq!(d.validity("ancho"), esperado, "texto {texto:?}");
    }
}

#[test]
fn fila_12_e3_la_nota_sobrevive_a_los_eventos_del_campo() {
    let mut d = dos_numericos();
    assert!(d.set_limit_note("ancho", Some("tarjeta gráfica")));
    numero(&mut d, "ancho", "5000");
    assert_eq!(
        d.validity("ancho"),
        invalido("máximo 4096 (tarjeta gráfica)")
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
    numero(&mut d, "ancho", "5000");
    assert_eq!(
        d.validity("ancho"),
        invalido("máximo 4096 (tarjeta gráfica)"),
        "la nota seguía puesta"
    );
}

#[test]
fn fila_12_e3_no_cambia_el_alto_ni_los_valores() {
    let mut d = dos_numericos();
    let alto = d.content_h_u();
    let valores = d.values();
    assert!(d.set_limit_note("ancho", Some("tarjeta gráfica")));
    assert_eq!(d.content_h_u(), alto);
    assert_eq!(d.values(), valores);
}

#[test]
fn fila_12_e3_set_limit_note_da_false_si_el_campo_no_existe_o_no_es_numerico() {
    let mut d = open(spec_con(vec![
        numerico("ancho", "Ancho", 1, 4096, 64),
        campo(
            "modo",
            "Modo",
            FieldKind::Choice {
                options: MODOS_19,
                default: 0,
            },
        ),
        campo("opaco", "Opaco", FieldKind::Check { default: false }),
        campo(
            "color",
            "Color",
            FieldKind::Hex {
                mode: Mode::Rgba,
                default: Rgba::opaque(1, 2, 3),
            },
        ),
        campo(
            "informe",
            "Informe",
            FieldKind::ReadOnly {
                text: "uno".to_string(),
                rows: Rows::Fixed(2),
            },
        ),
    ]));
    let antes = d.clone();
    for id in ["inexistente", "modo", "opaco", "color", "informe", ""] {
        assert!(!d.set_limit_note(id, Some("nota")), "{id:?} con nota");
        assert!(!d.set_limit_note(id, None), "{id:?} sin nota");
        assert_eq!(d, antes, "{id:?}: no cambió nada");
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

fn check_20_prohibe_el_siete(values: &Values) -> Option<String> {
    (values.number("ancho") == Some(7)).then(|| "el ancho no puede ser 7".to_string())
}

/// `ancho` (1 a 100, por defecto 64), una casilla `opaco`, un grupo `modo` y un `check` que
/// rechaza el ancho 7.
fn spec_20() -> FormSpec {
    FormSpec {
        check: Some(check_20_prohibe_el_siete),
        ..spec_con(vec![
            numerico("ancho", "Ancho", 1, 100, 64),
            campo("opaco", "Opaco", FieldKind::Check { default: false }),
            campo(
                "modo",
                "Modo",
                FieldKind::Choice {
                    options: MODOS_19,
                    default: 0,
                },
            ),
        ])
    }
}

fn dialogo_20() -> Dialog {
    Dialog::new(DialogKind::ConvertMode, spec_20()).expect("spec válida")
}

fn casilla(d: &mut Dialog, field: &'static str, on: bool) {
    assert_eq!(d.event(Event::Check { field, on }), Reaction::None);
}

// --- E1: set_block y block ---

#[test]
fn fila_12_set_block_sin_motivo_el_dialogo_valido_no_tiene_block() {
    let d = dialogo_20();
    assert_eq!(d.block(), None);
    assert_eq!(d.reason(), None);
    assert!(d.is_valid());
    assert!(d.values().is_some());
}

#[test]
fn fila_12_set_block_con_motivo_hace_invalido_un_dialogo_valido() {
    let mut d = dialogo_20();
    d.set_block(Some("no cabe".to_string()));
    assert_eq!(d.block(), Some("no cabe"));
    assert_eq!(d.reason(), Some("no cabe".to_string()));
    assert!(!d.is_valid());
    assert_eq!(d.values(), None);
}

#[test]
fn fila_12_set_block_none_deja_todo_como_antes() {
    let antes = dialogo_20();
    let valores = antes.values();
    let mut d = antes.clone();
    d.set_block(Some("no cabe".to_string()));
    assert!(!d.is_valid());
    d.set_block(None);
    assert_eq!(d.block(), None);
    assert_eq!(d.reason(), None);
    assert!(d.is_valid());
    assert_eq!(d.values(), valores);
    assert_eq!(d, antes, "el diálogo vuelve a ser el de antes");
    // Y `None` sobre un diálogo sin motivo no cambia nada.
    let mut otro = antes.clone();
    otro.set_block(None);
    assert_eq!(otro, antes);
}

#[test]
fn fila_12_set_block_se_puede_cambiar_y_el_ultimo_motivo_gana() {
    let mut d = dialogo_20();
    d.set_block(Some("uno".to_string()));
    d.set_block(Some("dos".to_string()));
    assert_eq!(d.block(), Some("dos"));
    assert_eq!(d.reason(), Some("dos".to_string()));
}

#[test]
fn fila_12_set_block_recorta_a_37_caracteres() {
    for (texto, esperado) in [
        ("x".repeat(36), "x".repeat(36)),
        ("x".repeat(37), "x".repeat(37)),
        ("x".repeat(38), "x".repeat(37)),
        ("x".repeat(200), "x".repeat(37)),
        // Se cuentan caracteres, no bytes.
        ("ñ".repeat(50), "ñ".repeat(37)),
        ("…".repeat(80), "…".repeat(37)),
    ] {
        let mut d = dialogo_20();
        d.set_block(Some(texto.clone()));
        let motivo = d.reason().expect("hay motivo");
        assert_eq!(
            motivo,
            esperado,
            "texto de {} caracteres",
            texto.chars().count()
        );
        assert_eq!(motivo.chars().count(), esperado.chars().count());
        assert!(motivo.chars().count() <= 37);
        assert_eq!(
            d.block(),
            Some(esperado.as_str()),
            "block() lo da recortado"
        );
        assert!(!d.is_valid());
    }
}

#[test]
fn fila_12_reason_da_primero_el_campo_invalido_luego_el_check_y_por_ultimo_el_motivo_externo() {
    let mut d = dialogo_20();
    d.set_block(Some("motivo externo".to_string()));

    // El campo inválido gana al check y al motivo externo.
    numero(&mut d, "ancho", "0");
    assert_eq!(d.reason(), Some("Ancho: mínimo 1".to_string()));
    assert_eq!(d.values(), None);

    // Con el campo válido pero con el ancho que rechaza el `check`: gana el `check`.
    numero(&mut d, "ancho", "7");
    assert_eq!(d.validity("ancho"), Some(Validity::Valid));
    assert_eq!(d.reason(), Some("el ancho no puede ser 7".to_string()));
    assert_eq!(d.values(), None);

    // Con el `check` satisfecho queda el motivo externo.
    numero(&mut d, "ancho", "8");
    assert_eq!(d.reason(), Some("motivo externo".to_string()));
    assert_eq!(d.values(), None);

    // Sin el motivo externo, todo vale.
    d.set_block(None);
    assert_eq!(d.reason(), None);
    assert!(d.values().is_some());
}

#[test]
fn fila_12_el_motivo_externo_no_oculta_el_error_de_un_campo_ni_el_del_check_al_quitarlo() {
    let mut d = dialogo_20();
    numero(&mut d, "ancho", "7");
    d.set_block(Some("motivo externo".to_string()));
    assert_eq!(d.reason(), Some("el ancho no puede ser 7".to_string()));
    d.set_block(None);
    assert_eq!(d.reason(), Some("el ancho no puede ser 7".to_string()));
}

#[test]
fn fila_12_accept_con_motivo_externo_da_rejected_y_no_toca_el_estado() {
    let mut d = dialogo_20();
    numero(&mut d, "ancho", "50");
    casilla(&mut d, "opaco", true);
    d.set_block(Some("no cabe".to_string()));
    let antes = d.clone();
    assert_eq!(
        d.event(Event::Accept),
        Reaction::Rejected("no cabe".to_string())
    );
    assert_eq!(d, antes, "un Accept rechazado no cambia nada");
    // Mientras haya motivo, sigue rechazando.
    assert_eq!(
        d.event(Event::Accept),
        Reaction::Rejected("no cabe".to_string())
    );
    // `Cancelar` no depende del motivo.
    assert_eq!(d.event(Event::Cancel), Reaction::Cancelled);
}

#[test]
fn fila_12_accept_sin_motivo_externo_entrega_los_valores_como_hoy() {
    let mut d = dialogo_20();
    numero(&mut d, "ancho", "50");
    d.set_block(Some("no cabe".to_string()));
    d.set_block(None);
    let esperado = d.values().expect("valores");
    match d.event(Event::Accept) {
        Reaction::Accepted(valores) => {
            assert_eq!(valores, esperado);
            assert_eq!(valores.number("ancho"), Some(50));
        }
        otra => panic!("se esperaba Accepted y salió {otra:?}"),
    }
}

#[test]
fn fila_12_accept_con_motivo_no_confirma_el_hexadecimal_pendiente() {
    let mut d = open(spec_con(vec![campo(
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
fn fila_12_set_block_no_cambia_el_alto_ni_los_campos_ni_los_valores_de_los_campos() {
    let mut d = dialogo_20();
    let alto = d.content_h_u();
    let spec = d.spec().clone();
    let texto = d.text("ancho");
    d.set_block(Some("no cabe".to_string()));
    assert_eq!(d.content_h_u(), alto);
    assert_eq!(d.spec(), &spec);
    assert_eq!(d.text("ancho"), texto);
    assert_eq!(d.validity("ancho"), Some(Validity::Valid));
}

#[test]
fn fila_12_set_block_sobre_un_modal_abierto_funciona_y_accept_lo_rechaza() {
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
fn fila_12_checked_es_publica_y_lee_las_casillas() {
    let mut d = dialogo_20();
    assert_eq!(d.checked("opaco"), Some(false));
    casilla(&mut d, "opaco", true);
    assert_eq!(d.checked("opaco"), Some(true));
    casilla(&mut d, "opaco", false);
    assert_eq!(d.checked("opaco"), Some(false));
}

#[test]
fn fila_12_checked_da_none_si_no_existe_o_no_es_una_casilla() {
    let d = dialogo_20();
    for id in ["ancho", "modo", "inexistente", ""] {
        assert_eq!(d.checked(id), None, "{id:?}");
    }
}

#[test]
fn fila_12_checked_lee_las_casillas_de_un_dialogo_invalido() {
    let mut d = dialogo_20();
    casilla(&mut d, "opaco", true);
    // Campo inválido.
    numero(&mut d, "ancho", "0");
    assert!(!d.is_valid());
    assert_eq!(d.values(), None);
    assert_eq!(d.checked("opaco"), Some(true));
    // `check` que falla.
    numero(&mut d, "ancho", "7");
    assert!(!d.is_valid());
    assert_eq!(d.checked("opaco"), Some(true));
    // Motivo externo.
    numero(&mut d, "ancho", "8");
    d.set_block(Some("no cabe".to_string()));
    assert!(!d.is_valid());
    assert_eq!(d.checked("opaco"), Some(true));
    casilla(&mut d, "opaco", false);
    assert_eq!(d.checked("opaco"), Some(false));
}

// --- DialogKind::ConvertMode ---

#[test]
fn fila_12_dialog_kind_convert_mode_es_un_tipo_nuevo() {
    let d = dialogo_20();
    assert_eq!(d.kind(), DialogKind::ConvertMode);
    for otro in [DialogKind::Licenses, DialogKind::Demo, DialogKind::NewImage] {
        assert_ne!(DialogKind::ConvertMode, otro);
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

const FALTA_DOS: &str = "falta: perder el color; fondo del alfa";

fn problemas(d: &Dialog) -> Vec<String> {
    d.problems()
}

/// Dos numéricos válidos por defecto, un grupo que oculta un hexadecimal y un `check` opcional.
fn spec_20b(check: Option<fn(&Values) -> Option<String>>) -> FormSpec {
    FormSpec {
        check,
        ..spec_con(vec![
            numerico("ancho", "Ancho", 1, 100, 64),
            numerico("alto", "Alto", 1, 100, 64),
            campo(
                "modo",
                "Modo",
                FieldKind::Choice {
                    options: MODOS_19,
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

fn dialogo_20b(check: Option<fn(&Values) -> Option<String>>) -> Dialog {
    Dialog::new(DialogKind::ConvertMode, spec_20b(check)).expect("spec válida")
}

fn check_largo_20b(_: &Values) -> Option<String> {
    Some("x".repeat(100))
}

#[test]
fn fila_6_problems_de_un_dialogo_valido_esta_vacio() {
    assert_eq!(problemas(&dialogo_20b(None)), Vec::<String>::new());
    assert_eq!(problemas(&dialog()), Vec::<String>::new());
    // Un `check` que no falla tampoco añade nada.
    assert!(problemas(&dialogo_20b(Some(check_nunca))).is_empty());
}

#[test]
fn fila_6_problems_con_un_campo_invalido_da_su_motivo_el_mismo_de_reason() {
    let mut d = dialogo_20b(None);
    numero(&mut d, "ancho", "0");
    assert_eq!(problemas(&d), vec!["Ancho: mínimo 1".to_string()]);
    assert_eq!(d.reason(), Some("Ancho: mínimo 1".to_string()));
    // Un hexadecimal visible inválido.
    let mut d = dialogo_20b(None);
    elige(&mut d, "modo", 1);
    hex(&mut d, "color", "zz");
    let lista = problemas(&d);
    assert_eq!(lista.len(), 1, "{lista:?}");
    assert!(lista[0].starts_with("Color: "), "{lista:?}");
    assert_eq!(Some(lista[0].clone()), d.reason());
}

#[test]
fn fila_6_problems_lista_todos_los_campos_invalidos_en_el_orden_de_los_campos() {
    let mut d = dialogo_20b(None);
    elige(&mut d, "modo", 1);
    hex(&mut d, "color", "zz");
    numero(&mut d, "alto", "0");
    numero(&mut d, "ancho", "0");
    let lista = problemas(&d);
    assert_eq!(lista.len(), 3, "{lista:?}");
    assert_eq!(lista[0], "Ancho: mínimo 1");
    assert_eq!(lista[1], "Alto: mínimo 1");
    assert!(lista[2].starts_with("Color: "), "{lista:?}");
    // El primero es el que da `reason()` sin resumen.
    assert_eq!(d.reason(), Some(lista[0].clone()));
}

#[test]
fn fila_6_problems_no_cuenta_un_campo_oculto_aunque_tenga_algo_invalido_escrito() {
    let mut d = dialogo_20b(None);
    elige(&mut d, "modo", 1);
    hex(&mut d, "color", "zz");
    assert_eq!(problemas(&d).len(), 1);
    elige(&mut d, "modo", 0);
    assert!(!d.is_shown("color"));
    assert!(problemas(&d).is_empty());
    assert!(d.is_valid());
}

#[test]
fn fila_6_problems_da_el_motivo_del_check_sin_recortar() {
    let d = dialogo_20b(Some(check_largo_20b));
    assert_eq!(problemas(&d), vec!["x".repeat(100)]);
    // `reason()` sigue recortándolo a 37 como hoy.
    assert_eq!(d.reason(), Some("x".repeat(37)));
}

#[test]
fn fila_6_problems_no_incluye_el_motivo_externo_ni_el_resumen() {
    let mut d = dialogo_20b(None);
    d.set_block(Some("calculando el informe".to_string()));
    d.set_summary(Some(FALTA_DOS.to_string()));
    assert!(problemas(&d).is_empty());
    // Con un campo inválido, solo el del campo.
    numero(&mut d, "ancho", "0");
    assert_eq!(problemas(&d), vec!["Ancho: mínimo 1".to_string()]);
}

#[test]
fn fila_6_problems_no_cambia_el_dialogo() {
    let mut d = dialogo_20b(None);
    numero(&mut d, "ancho", "0");
    let antes = d.clone();
    let _ = problemas(&d);
    assert_eq!(d, antes);
}

#[test]
fn fila_6_summary_sin_poner_nada_es_none_y_set_summary_lo_guarda() {
    let mut d = dialogo_20b(None);
    assert_eq!(d.summary(), None);
    d.set_summary(Some(FALTA_DOS.to_string()));
    assert_eq!(d.summary(), Some(FALTA_DOS));
    d.set_summary(Some("falta: otra cosa".to_string()));
    assert_eq!(d.summary(), Some("falta: otra cosa"), "el último gana");
}

#[test]
fn fila_6_con_resumen_reason_es_el_resumen_y_el_dialogo_es_invalido() {
    let mut d = dialogo_20b(None);
    d.set_summary(Some(FALTA_DOS.to_string()));
    assert_eq!(d.reason(), Some(FALTA_DOS.to_string()));
    assert!(!d.is_valid());
    assert_eq!(d.values(), None);
    // El resumen no es un motivo externo.
    assert_eq!(d.block(), None);
}

#[test]
fn fila_6_el_resumen_va_antes_que_el_campo_invalido_el_check_y_el_motivo_externo() {
    // Campo inválido.
    let mut d = dialogo_20b(None);
    d.set_block(Some("calculando el informe".to_string()));
    numero(&mut d, "ancho", "0");
    assert_eq!(d.reason(), Some("Ancho: mínimo 1".to_string()));
    d.set_summary(Some(FALTA_DOS.to_string()));
    assert_eq!(d.reason(), Some(FALTA_DOS.to_string()));
    // Y el motivo del campo sigue estando en `problems()`.
    assert_eq!(problemas(&d), vec!["Ancho: mínimo 1".to_string()]);
    d.set_summary(None);
    assert_eq!(d.reason(), Some("Ancho: mínimo 1".to_string()));

    // `check` que falla.
    let mut d = dialogo_20b(Some(check_largo_20b));
    d.set_block(Some("motivo externo".to_string()));
    assert_eq!(d.reason(), Some("x".repeat(37)));
    d.set_summary(Some(FALTA_DOS.to_string()));
    assert_eq!(d.reason(), Some(FALTA_DOS.to_string()));

    // Solo motivo externo.
    let mut d = dialogo_20b(None);
    d.set_block(Some("motivo externo".to_string()));
    d.set_summary(Some(FALTA_DOS.to_string()));
    assert_eq!(d.reason(), Some(FALTA_DOS.to_string()));
    assert_eq!(d.block(), Some("motivo externo"));
}

#[test]
fn fila_6_accept_con_resumen_da_rejected_con_el_resumen_y_no_toca_el_estado() {
    let mut d = dialogo_20b(None);
    numero(&mut d, "ancho", "50");
    d.set_summary(Some(FALTA_DOS.to_string()));
    let antes = d.clone();
    assert_eq!(
        d.event(Event::Accept),
        Reaction::Rejected(FALTA_DOS.to_string())
    );
    assert_eq!(d, antes, "un Accept rechazado no cambia nada");
    // Con un campo inválido a la vez, el resumen sigue ganando.
    numero(&mut d, "ancho", "0");
    assert_eq!(
        d.event(Event::Accept),
        Reaction::Rejected(FALTA_DOS.to_string())
    );
    // Sin resumen y con todo válido, se acepta.
    numero(&mut d, "ancho", "50");
    d.set_summary(None);
    assert!(matches!(d.event(Event::Accept), Reaction::Accepted(_)));
}

#[test]
fn fila_6_accept_con_resumen_a_traves_del_modal_no_cierra() {
    let mut modal = Modal::default();
    assert!(modal.open(DialogKind::ConvertMode, spec_20b(None), Locks::default()));
    match &mut modal {
        Modal::Open(d) => d.set_summary(Some(FALTA_DOS.to_string())),
        Modal::Closed => panic!("abierto"),
    }
    assert_eq!(
        modal.event(Event::Accept),
        Reaction::Rejected(FALTA_DOS.to_string())
    );
    assert!(modal.is_open());
    match &mut modal {
        Modal::Open(d) => d.set_summary(None),
        Modal::Closed => panic!("abierto"),
    }
    assert!(matches!(modal.event(Event::Accept), Reaction::Accepted(_)));
}

#[test]
fn fila_6_set_summary_none_deja_el_dialogo_igual_al_de_antes() {
    // Sobre un diálogo virgen.
    let antes = dialogo_20b(None);
    let mut d = antes.clone();
    d.set_summary(None);
    assert_eq!(d, antes);
    // Tras poner y quitar un resumen.
    d.set_summary(Some(FALTA_DOS.to_string()));
    assert_ne!(d, antes, "el resumen forma parte del estado");
    d.set_summary(None);
    assert_eq!(d, antes);
    assert_eq!(d.summary(), None);
    assert_eq!(d.reason(), None);
    assert_eq!(d.values(), antes.values());
    // Con un campo inválido y un motivo externo de por medio.
    let mut antes = dialogo_20b(None);
    numero(&mut antes, "alto", "0");
    antes.set_block(Some("calculando el informe".to_string()));
    let mut d = antes.clone();
    d.set_summary(Some("falta: algo".to_string()));
    d.set_summary(None);
    assert_eq!(d, antes);
}

#[test]
fn fila_6_set_block_sigue_como_esta_con_y_sin_resumen() {
    let mut d = dialogo_20b(None);
    d.set_summary(Some(FALTA_DOS.to_string()));
    // Poner y cambiar el motivo externo no toca el resumen y el resumen no toca `block()`.
    d.set_block(Some("no cabe".to_string()));
    assert_eq!(d.block(), Some("no cabe"));
    assert_eq!(d.summary(), Some(FALTA_DOS));
    assert_eq!(d.reason(), Some(FALTA_DOS.to_string()));
    // Quitar el resumen deja el motivo externo.
    d.set_summary(None);
    assert_eq!(d.block(), Some("no cabe"));
    assert_eq!(d.reason(), Some("no cabe".to_string()));
    // Quitar el motivo externo con un resumen puesto no quita el resumen.
    d.set_summary(Some(FALTA_DOS.to_string()));
    d.set_block(None);
    assert_eq!(d.block(), None);
    assert_eq!(d.summary(), Some(FALTA_DOS));
    assert_eq!(d.reason(), Some(FALTA_DOS.to_string()));
    d.set_summary(None);
    assert_eq!(d.reason(), None);
}

#[test]
fn fila_6_el_resumen_se_recorta_a_37_o_a_44_si_empieza_por_falta() {
    for (texto, esperado) in [
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
        let mut d = dialogo_20b(None);
        d.set_summary(Some(texto.clone()));
        let motivo = d.reason().expect("hay motivo");
        assert_eq!(
            motivo,
            esperado,
            "texto de {} caracteres",
            texto.chars().count()
        );
        assert_eq!(
            d.summary(),
            Some(esperado.as_str()),
            "summary() lo da recortado"
        );
        assert!(!d.is_valid());
    }
}

#[test]
fn fila_6_set_block_tambien_recorta_a_44_si_empieza_por_falta() {
    for (texto, esperado) in [
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
        let mut d = dialogo_20b(None);
        d.set_block(Some(texto.clone()));
        assert_eq!(d.block(), Some(esperado.as_str()), "{texto:?}");
        assert_eq!(d.reason(), Some(esperado.clone()));
        assert!(esperado.chars().count() <= 44);
    }
}

#[test]
fn fila_6_los_textos_de_la_lista_de_la_spec_caben_sin_recorte() {
    for texto in [
        "falta: permitir perder el color",
        "falta: elegir el fondo del alfa",
        "falta: perder el color; fondo del alfa",
        "falta: perder el color; fondo válido",
        "falta: fondo del alfa; fondo válido",
        "faltan 3: color, fondo, valor",
    ] {
        let mut d = dialogo_20b(None);
        d.set_block(Some(texto.to_string()));
        assert_eq!(d.reason(), Some(texto.to_string()), "block {texto:?}");
        let mut d = dialogo_20b(None);
        d.set_summary(Some(texto.to_string()));
        assert_eq!(d.reason(), Some(texto.to_string()), "summary {texto:?}");
        assert_eq!(d.summary(), Some(texto));
    }
}

// ---------------------------------------------------------------------------------------------
// Apéndice A1 de la unidad 19 (specs/hito-2-nueva-imagen.md): `Dialog::hex_committed` y
// `resolve_hex`. En ROJO hasta que existan las dos piezas (aditivas, sin cambiar nada de la 18).
// ---------------------------------------------------------------------------------------------

const POR_DEFECTO: Rgba = Rgba::new(0x33, 0x66, 0x99, 0x80);

#[test]
fn fila_a1_hex_committed_nace_en_el_valor_por_defecto_del_campo() {
    for mode in [Mode::Rgba, Mode::GrayAlpha, Mode::Gray] {
        let default = match mode {
            Mode::Rgba => POR_DEFECTO,
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
fn fila_a1_hex_committed_sigue_al_ultimo_confirmado() {
    let mut d = open(hex_form(Mode::Rgba, POR_DEFECTO));
    // Intro con un válido de 6 dígitos: toma el alfa del confirmado.
    hex(&mut d, "color", "#123456");
    confirma(&mut d, "color", false);
    assert_eq!(
        d.hex_committed("color"),
        Some(Rgba::new(0x12, 0x34, 0x56, 0x80))
    );
    // Pérdida del foco con un válido de 8 dígitos: el suyo.
    hex(&mut d, "color", "#abcdef11");
    confirma(&mut d, "color", true);
    assert_eq!(
        d.hex_committed("color"),
        Some(Rgba::new(0xab, 0xcd, 0xef, 0x11))
    );
    // Tres dígitos: el alfa del último confirmado (0x11), no el de partida.
    hex(&mut d, "color", "f00");
    confirma(&mut d, "color", false);
    assert_eq!(d.hex_committed("color"), Some(Rgba::new(0xff, 0, 0, 0x11)));
    // `Accept` confirma el pendiente válido antes de entregarlo.
    hex(&mut d, "color", "#00ff00ee");
    assert!(matches!(d.event(Event::Accept), Reaction::Accepted(_)));
    assert_eq!(d.hex_committed("color"), Some(Rgba::new(0, 0xff, 0, 0xee)));
}

#[test]
fn fila_a1_hex_committed_con_texto_pendiente_sigue_el_confirmado() {
    let confirmado = Rgba::new(0x12, 0x34, 0x56, 0x80);
    let mut d = open(hex_form(Mode::Rgba, POR_DEFECTO));
    hex(&mut d, "color", "#123456");
    confirma(&mut d, "color", false);
    assert_eq!(d.hex_committed("color"), Some(confirmado));
    // Pendiente inválido (escrito, con Intro, con pérdida del foco que revierte, con Escape).
    hex(&mut d, "color", "zz");
    assert!(d.hex_pending("color"));
    assert_eq!(d.hex_committed("color"), Some(confirmado), "zz pendiente");
    confirma(&mut d, "color", false);
    assert!(d.hex_pending("color"));
    assert_eq!(d.hex_committed("color"), Some(confirmado), "zz con Intro");
    confirma(&mut d, "color", true);
    assert!(!d.hex_pending("color"));
    assert_eq!(d.hex_committed("color"), Some(confirmado), "zz revertido");
    d.event(Event::HexRevert { field: "color" });
    assert_eq!(d.hex_committed("color"), Some(confirmado), "tras Escape");
    // Pendiente válido sin confirmar: sigue el confirmado, no el escrito.
    hex(&mut d, "color", "#abcdef");
    assert!(d.hex_pending("color"));
    assert_eq!(
        d.hex_committed("color"),
        Some(confirmado),
        "válido sin confirmar"
    );
    // El campo oculto conserva el suyo aunque se oculte con basura escrita.
    let mut d = dialog();
    elige(&mut d, "modo", 2);
    hex(&mut d, "color", "#445566");
    confirma(&mut d, "color", false);
    hex(&mut d, "color", "zz");
    elige(&mut d, "modo", 0);
    assert!(!d.is_shown("color"));
    assert_eq!(
        d.hex_committed("color"),
        Some(Rgba::opaque(0x44, 0x55, 0x66))
    );
}

#[test]
fn fila_a1_hex_committed_da_none_si_el_campo_no_existe() {
    let d = dialog();
    for id in ["", "nada", "Color", "color ", "color_rgba"] {
        assert_eq!(d.hex_committed(id), None, "{id:?}");
    }
    let d = open(hex_form(Mode::Rgba, POR_DEFECTO));
    assert_eq!(d.hex_committed("modo"), None);
}

#[test]
fn fila_a1_hex_committed_da_none_si_el_campo_no_es_hexadecimal() {
    let d = dialog();
    for id in ["ancho", "alto", "modo", "opaco", "informe"] {
        assert!(d.spec().fields.iter().any(|f| f.id == id), "{id} existe");
        assert_eq!(d.hex_committed(id), None, "{id}");
    }
}

#[test]
fn fila_a1_hex_committed_no_cambia_el_dialogo() {
    let mut d = open(hex_form(Mode::Rgba, POR_DEFECTO));
    hex(&mut d, "color", "zz");
    let antes = d.clone();
    let _ = d.hex_committed("color");
    let _ = d.hex_committed("nada");
    assert_eq!(d, antes);
}

const TEXTOS_RESOLVE: [&str; 22] = [
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

const CONFIRMADOS_RESOLVE: [Rgba; 4] = [
    Rgba::new(0x80, 0x80, 0x80, 0xff),
    Rgba::new(0x33, 0x66, 0x99, 0xcc),
    Rgba::new(0x80, 0x80, 0x80, 0x80),
    Rgba::new(0, 0, 0, 0),
];

#[test]
fn fila_a1_resolve_hex_sin_texto_da_el_confirmado() {
    for mode in [Mode::Rgba, Mode::GrayAlpha, Mode::Gray, Mode::Indexed] {
        for committed in CONFIRMADOS_RESOLVE {
            assert_eq!(
                resolve_hex(None, mode, committed),
                Ok(committed),
                "{mode:?}"
            );
        }
    }
}

#[test]
fn fila_a1_resolve_hex_con_texto_es_parse_hex_con_el_confirmado() {
    for mode in [Mode::Rgba, Mode::GrayAlpha, Mode::Gray] {
        for committed in CONFIRMADOS_RESOLVE {
            for texto in TEXTOS_RESOLVE {
                assert_eq!(
                    resolve_hex(Some(texto), mode, committed),
                    parse_hex(texto, mode, committed),
                    "{mode:?} {committed:?} {texto:?}"
                );
            }
        }
    }
}

#[test]
fn fila_a1_resolve_hex_tres_y_seis_digitos_toman_el_alfa_del_confirmado_y_cuatro_y_ocho_el_suyo() {
    let confirmado = Rgba::new(0x33, 0x66, 0x99, 0xcc);
    for (texto, esperado) in [
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
            resolve_hex(Some(texto), Mode::Rgba, confirmado),
            Ok(esperado),
            "RGBA {texto:?}"
        );
        assert_eq!(
            resolve_hex(Some(texto), Mode::GrayAlpha, confirmado),
            Ok(esperado),
            "Gris con alfa {texto:?}"
        );
    }
}

#[test]
fn fila_a1_resolve_hex_los_invalidos_dan_su_error() {
    let c = Rgba::opaque(0x80, 0x80, 0x80);
    for (texto, error) in [
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
                resolve_hex(Some(texto), mode, c),
                Err(error.clone()),
                "{mode:?} {texto:?}"
            );
        }
    }
}

#[test]
fn fila_a1_resolve_hex_aplica_las_reglas_de_gris_y_gris_con_alfa() {
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
fn fila_a1_resolve_hex_es_la_regla_que_usan_validez_y_valores_del_dialogo() {
    // El dialogo y la función dan el mismo color y el mismo error para cada texto, con el último
    // confirmado distinto del de partida.
    for mode in [Mode::Rgba, Mode::GrayAlpha, Mode::Gray] {
        for texto in TEXTOS_RESOLVE {
            let mut d = open(hex_form(mode, Rgba::opaque(0x80, 0x80, 0x80)));
            hex(&mut d, "color", "#aaaaaa");
            confirma(&mut d, "color", false);
            let confirmado = d.hex_committed("color").expect("hexadecimal");
            hex(&mut d, "color", texto);
            match resolve_hex(Some(texto), mode, confirmado) {
                Ok(color) => {
                    assert_eq!(
                        d.validity("color"),
                        Some(Validity::Valid),
                        "{mode:?} {texto:?}"
                    );
                    assert_eq!(color_aceptado(&d), color, "{mode:?} {texto:?}");
                }
                Err(error) => {
                    assert_eq!(
                        d.validity("color"),
                        Some(Validity::Invalid(hex_notice(&error).to_string())),
                        "{mode:?} {texto:?}"
                    );
                    assert!(matches!(
                        d.clone().event(Event::Accept),
                        Reaction::Rejected(_)
                    ));
                    // Con Intro el texto inválido se queda; con la pérdida del foco, revierte.
                    confirma(&mut d, "color", false);
                    assert_eq!(d.hex_committed("color"), Some(confirmado));
                    assert!(d.hex_pending("color"), "{mode:?} {texto:?}");
                    confirma(&mut d, "color", true);
                    assert!(!d.hex_pending("color"), "{mode:?} {texto:?}");
                    assert_eq!(d.hex_committed("color"), Some(confirmado));
                }
            }
        }
    }
}
