//! Tests de `background_color` (specs/hito-2-fondo-color.md, unidad 19b): las filas 4 y 5, la parte de
//! `write_png` de la fila 8 y las propiedades 1 y 2. En ROJO hasta la implementación: no compilan sin
//! `CanvasBackground::Custom`, `DialogKind::BackgroundColor` ni el módulo `background_color`.
//! (El despacho de la fila 3 está en `dispatch/tests.rs`; lo de `px-ui`, en `px-ui/tests/`.)
//!
//! - Fila 4: `background_color_form`, `initial_color`, `open_background_color`, el campo hexadecimal con
//!   las cuatro longitudes con y sin `#`, el rechazo del alfa con `OPAQUE_REASON`, **la secuencia**
//!   `#ff000080` + `Intro`, `#fff`, `#ffffff`, `#ffffffff`, los errores en línea, `Intro`, la pérdida de
//!   foco, `Escape`, el foco inicial, el orden de tabulación, los textos (37 caracteres, juego de
//!   caracteres y contraste con un banco de egui sin ventana).
//! - Fila 5: sesión y lienzo sin ventana (`Session` y `Canvas`): aceptar, cancelar, `Escape`, `Quit`,
//!   los bloqueos, las operaciones de documento y deshacer/rehacer, con cada fondo previo.
//! - Fila 8 (parte de `px-app`): `write_png` da los mismos bytes con cualquier fondo.
//! - Propiedad 1: el formulario acepta exactamente los colores opacos (256 casos).
//! - Propiedad 2: el fondo es un modelo trivial «el último aplicado» (256 casos).
//!
//! **Suposiciones y ambigüedades** (la spec no fija un detalle; se elige lo más literal):
//! - `FIELD_COLOR = "color"` y `OPAQUE_REASON` son los de «Piezas». El rótulo del campo no está fijado
//!   (solo que cabe en 12 caracteres, lo que ya comprueba `FormSpec::validate`): las pruebas lo leen
//!   del formulario. El resto de textos fijados son el título `TestBackground del canvas` y los botones
//!   `Aplicar` y `Cancel`.
//! - «`Intro` en el campo» es lo que hace `dialog_view` con `key_action` (`Enter` con el foco en el
//!   campo): `ConfirmHex` con texto pendiente y `Accept` sin él; `Escape`, `RevertHex` con texto
//!   pendiente y `Cancel` sin él.
//! - `initial_color` de un `Custom` construido a mano con alfa < 255 no está fijado y no se prueba.
//! - `Dialog: Clone + PartialEq` (derives de `px-ui`) sirve para comprobar que un rechazo no cambia
//!   nada.
//! - Lo que `App` hace al aceptar (D8) se escribe aquí como `World::event_2`: `Accepted` →
//!   `background_from_values` → `Canvas::set_background`; el resto de reacciones no toca el lienzo.
//! - La sesión de la fila 5 es una RGBA de 8×6 con dos pasos de historia y uno deshecho (se puede
//!   deshacer y rehacer) y, en la variante «modificada», una edición posterior sin guardar. No se
//!   comprueba el título de la ventana (es de `title.rs` y no depende del fondo).
//! - Cada prueba que escribe un PNG usa su propio directorio temporal y nunca escribe en `golden/`.
//! - NO PROBADO: que el motivo de alfa se vea en el pie del diálogo con el rótulo `error: ` más el
//!   motivo a las cinco escalas y a la ventana mínima (se mira a las cinco escalas con la ventana
//!   grande: el reparto del pie a la ventana mínima es de la unidad 18).

use super::*;

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use eframe::egui::epaint::{ClippedShape, Shape};
use eframe::egui::{Color32, Context, MissingGlyphPolicy, Pos2, RawInput, Rect, Vec2, vec2};
use proptest::prelude::*;
use px_core::{Allowed, Command, Document, Image, ImageId, Mode, Pixel, PixelEdit, Rgba};
use px_formats::{read_png, write_png};
use px_ui::actions::{AppAction, Locks};
use px_ui::background::{CanvasBackground, background_value_text};
use px_ui::canvas::{BACKGROUND, Canvas};
use px_ui::color::ColorChoice;
use px_ui::color_edit::{HexError, hex_notice};
use px_ui::dialog::{
    Dialog, DialogKind, Event, FOOTER_COLUMNS, FieldKind, FormSpec, Modal, REASON_MAX_CHARS,
    Reaction, Validity, resolve_hex,
};
use px_ui::dialog_keys::{
    DialogKey, FocusTarget, KeyAction, focus_order, initial_focus, key_action,
};
use px_ui::dialog_view::{ERROR_PREFIX, show_dialog};
use px_ui::menu::background_checked;
use px_ui::palette_grid::{CHECK_A, CHECK_B};
use px_ui::text::{INTERFACE_CHARSET, apply_scale, install_fonts};
use px_ui::texture::to_rgba8;
use px_ui::theme::{TEXT_DISABLED, contrast_ratio, visuals};

use crate::session::Session;

// ---------------------------------------------------------------------------------------------
// Datos y ayudantes
// ---------------------------------------------------------------------------------------------

/// Las cinco escalas de pantalla de la spec.
const PPPS: [f32; 5] = [1.0, 1.25, 1.5, 2.0, 3.0];

const GRAY_DEFAULT: Rgba = Rgba::opaque(0x80, 0x80, 0x80);
const BLUE_STEEL: Rgba = Rgba::opaque(0x33, 0x66, 0x99);
const BLUE_DARK: Rgba = Rgba::opaque(0x11, 0x22, 0x33);
const WHITE: Rgba = Rgba::opaque(0xff, 0xff, 0xff);
const TEST_BLACK: Rgba = Rgba::opaque(0, 0, 0);
const TEST_RED: Rgba = Rgba::opaque(0xff, 0, 0);

/// Los cinco fondos previos y el color con el que debe nacer el diálogo desde cada uno (el del
/// sólido, `#808080` con el damero).
const PREVIOUS: [(CanvasBackground, Rgba); 5] = [
    (CanvasBackground::Checker, GRAY_DEFAULT),
    (CanvasBackground::Black, TEST_BLACK),
    (CanvasBackground::Gray, Rgba::opaque(40, 40, 40)),
    (CanvasBackground::White, WHITE),
    (CanvasBackground::Custom(BLUE_DARK), BLUE_DARK),
];

/// Cada bloqueo, solo.
fn locks_one_by_one() -> [(&'static str, Locks); 5] {
    let with = |f: fn(&mut Locks)| {
        let mut locks = Locks::default();
        f(&mut locks);
        locks
    };
    [
        ("gesture", with(|l| l.gesture = true)),
        ("dialog", with(|l| l.dialog = true)),
        ("text_focus", with(|l| l.text_focus = true)),
        ("removal", with(|l| l.removal = true)),
        ("modal", with(|l| l.modal = true)),
    ]
}

fn locks_2(bits: u8) -> Locks {
    Locks {
        gesture: bits & 1 != 0,
        stroke: false,
        dialog: bits & 2 != 0,
        text_focus: bits & 4 != 0,
        removal: bits & 8 != 0,
        modal: bits & 16 != 0,
    }
}

fn in_the_set(text_2: &str) -> bool {
    text_2.chars().all(|c| INTERFACE_CHARSET.contains(c))
}

/// El diálogo recién abierto desde `previous`.
fn open(previous: CanvasBackground) -> Modal {
    let mut modal = Modal::default();
    assert!(
        open_background_color(&mut modal, previous, Locks::default()),
        "{previous:?}: el diálogo se abre"
    );
    modal
}

fn dialog(modal: &Modal) -> &Dialog {
    match modal {
        Modal::Open(d) => d,
        Modal::Closed => panic!("el modal está cerrado"),
    }
}

fn writes(modal: &mut Modal, text_2: &str) {
    assert_eq!(
        modal.event(Event::Hex {
            field: FIELD_COLOR,
            text: text_2.to_string()
        }),
        Reaction::None,
        "escribir {text_2:?}"
    );
}

/// `Intro` sobre un texto pendiente: confirma el campo.
fn confirms(modal: &mut Modal) {
    assert_eq!(
        modal.event(Event::HexConfirm {
            field: FIELD_COLOR,
            blur: false
        }),
        Reaction::None
    );
}

fn loses_the_focus(modal: &mut Modal) {
    assert_eq!(
        modal.event(Event::HexConfirm {
            field: FIELD_COLOR,
            blur: true
        }),
        Reaction::None
    );
}

fn accepts(modal: &mut Modal) -> Reaction {
    modal.event(Event::Accept)
}

fn text_of_the_field(modal: &Modal) -> String {
    dialog(modal).text(FIELD_COLOR).expect("el campo existe")
}

fn confirmed(modal: &Modal) -> Rgba {
    dialog(modal)
        .hex_committed(FIELD_COLOR)
        .expect("el campo es hexadecimal")
}

fn reason_2(modal: &Modal) -> Option<String> {
    dialog(modal).reason()
}

const IN_THE_FIELD: Option<FocusTarget> = Some(FocusTarget::Field(FIELD_COLOR));

/// Una tecla con el foco en `focus`: lo que decide `key_action` y el evento que `dialog_view` le
/// aplica al diálogo.
fn presses(modal: &mut Modal, focus: Option<FocusTarget>, key_2: DialogKey) -> Reaction {
    let action = key_action(dialog(modal), focus, key_2, false);
    match action {
        KeyAction::Accept => modal.event(Event::Accept),
        KeyAction::Cancel => modal.event(Event::Cancel),
        KeyAction::ConfirmHex(field) => modal.event(Event::HexConfirm { field, blur: false }),
        KeyAction::RevertHex(field) => modal.event(Event::HexRevert { field }),
        KeyAction::Nothing => Reaction::None,
    }
}

/// El color que debe salir de un texto que vale (con alfa `ff`).
fn opaque(r: u8, g: u8, b: u8) -> Rgba {
    Rgba::opaque(r, g, b)
}

/// Textos que valen desde un diálogo recién abierto (el confirmado es opaco): con y sin `#`, de 3,
/// 4, 6 y 8 dígitos, con alfa `ff`.
fn texts_that_are() -> Vec<(&'static str, Rgba)> {
    vec![
        ("fff", WHITE),
        ("#fff", WHITE),
        ("ffffff", WHITE),
        ("#ffffff", WHITE),
        ("ffff", WHITE),
        ("#ffff", WHITE),
        ("#ff0000ff", TEST_RED),
        ("ff0000ff", TEST_RED),
        ("f00", TEST_RED),
        ("#f00", TEST_RED),
        ("#f00f", TEST_RED),
        ("336699", BLUE_STEEL),
        ("#336699", BLUE_STEEL),
        ("#336699FF", BLUE_STEEL),
        ("  #336699  ", BLUE_STEEL),
        ("#abc", opaque(0xaa, 0xbb, 0xcc)),
    ]
}

/// Textos de 4 u 8 dígitos con alfa < `ff`: el campo es válido y el conjunto no.
const TEXTS_WITH_ALPHA: [&str; 12] = [
    "fff8",
    "#fff8",
    "ffffff80",
    "#ffffff80",
    "#ff000080",
    "#0000",
    "#00000000",
    "#336699fe",
    "336699fe",
    "  #fff8  ",
    "#f00e",
    "#abcdef01",
];

// ---------------------------------------------------------------------------------------------
// Fila 4: el formulario
// ---------------------------------------------------------------------------------------------

#[test]
fn row_4_the_constants_are_those_of_pieces() {
    assert_eq!(FIELD_COLOR, "color");
    assert_eq!(OPAQUE_REASON, "el fondo debe ser opaco (alfa ff)");
    assert_eq!(OPAQUE_REASON.chars().count(), 33);
    assert!(OPAQUE_REASON.chars().count() <= REASON_MAX_CHARS);
    assert_eq!(REASON_MAX_CHARS, 37);
    assert!(in_the_set(OPAQUE_REASON));
    // Con el prefijo `error: ` cabe en las 44 columnas del pie y no se recorta.
    assert!(ERROR_PREFIX.chars().count() + OPAQUE_REASON.chars().count() <= FOOTER_COLUMNS);
}

#[test]
fn row_4_background_color_form_valid_and_carries_title_buttons_a_hexadecimal_rgba_and_a_check() {
    for color in [
        GRAY_DEFAULT,
        TEST_BLACK,
        WHITE,
        BLUE_STEEL,
        Rgba::opaque(40, 40, 40),
    ] {
        let spec: FormSpec = background_color_form(color);
        assert_eq!(spec.validate(), Ok(()), "{color:?}");
        assert_eq!(spec.title, "Fondo del lienzo");
        assert_eq!(spec.accept, "Aplicar");
        assert_eq!(spec.cancel, Some("Cancelar"));
        assert_eq!(spec.fields.len(), 1, "un solo campo");
        let field_2 = &spec.fields[0];
        assert_eq!(field_2.id, FIELD_COLOR);
        assert!(field_2.shown_when.is_none());
        assert_eq!(
            field_2.kind,
            FieldKind::Hex {
                mode: Mode::Rgba,
                default: color
            },
            "hexadecimal en modo RGBA con el color inicial por defecto"
        );
        assert!(field_2.label.chars().count() <= 12 && !field_2.label.is_empty());
        assert!(
            spec.check.is_some(),
            "el check de opacidad es lo que rechaza el alfa"
        );
        // El formulario se puede abrir (`Dialog::new` valida) como el diálogo de color.
        assert!(Dialog::new(DialogKind::BackgroundColor, spec).is_ok());
    }
}

#[test]
fn row_4_initial_color_is_that_of_the_solid_the_gray_80_with_the_checker_and_that_of_custom() {
    for (previous, expected_2) in PREVIOUS {
        assert_eq!(initial_color(previous), expected_2, "{previous:?}");
        assert_eq!(
            initial_color(previous).a,
            255,
            "{previous:?}: siempre opaco"
        );
    }
    // Otros colores propios, con canales distintos.
    for c in [
        BLUE_STEEL,
        Rgba::opaque(0xff, 0x80, 0),
        Rgba::opaque(1, 2, 3),
    ] {
        assert_eq!(initial_color(CanvasBackground::Custom(c)), c);
    }
    // Con el damero, el `DEFAULT_COLOR` de la 19.
    assert_eq!(
        initial_color(CanvasBackground::Checker),
        Rgba::opaque(0x80, 0x80, 0x80)
    );
}

#[test]
fn row_4_open_background_color_opens_the_color_dialog_with_the_color_of_the_current_background() {
    for (previous, color) in PREVIOUS {
        let modal = open(previous);
        assert_eq!(modal.kind(), Some(DialogKind::BackgroundColor));
        let d = dialog(&modal);
        assert_eq!(d.kind(), DialogKind::BackgroundColor);
        assert_eq!(d.spec().title, "Fondo del lienzo");
        // El defecto es válido y `Aplicar` está habilitado.
        assert_eq!(
            d.validity(FIELD_COLOR),
            Some(Validity::Valid),
            "{previous:?}"
        );
        assert!(d.is_valid(), "{previous:?}");
        assert_eq!(d.reason(), None, "{previous:?}");
        assert_eq!(d.text(FIELD_COLOR), Some(color.to_string()), "{previous:?}");
        assert_eq!(d.hex_committed(FIELD_COLOR), Some(color), "{previous:?}");
        assert!(!d.hex_pending(FIELD_COLOR));
        let values_2 = d.values().expect("el defecto vale");
        assert_eq!(values_2.color(FIELD_COLOR), Some(color));
        assert_eq!(
            background_from_values(&values_2),
            Some(CanvasBackground::Custom(color)),
            "{previous:?}: aceptar sin tocar nada da el color de partida"
        );
    }
}

#[test]
fn row_4_the_default_is_accepted_as_is_and_gives_custom_with_alpha_ff() {
    for (previous, color) in PREVIOUS {
        let mut modal = open(previous);
        let Reaction::Accepted(values_2) = accepts(&mut modal) else {
            panic!("{previous:?}: el defecto se acepta");
        };
        assert!(!modal.is_open());
        assert_eq!(values_2.color(FIELD_COLOR), Some(color));
        assert_eq!(
            background_from_values(&values_2),
            Some(CanvasBackground::Custom(color))
        );
    }
}

#[test]
fn row_4_with_a_lock_or_with_another_dialog_open_background_color_gives_false_and_changes_nothing()
{
    for (previous, _) in PREVIOUS {
        for (name_2, locks) in locks_one_by_one() {
            let mut modal = Modal::default();
            assert!(
                !open_background_color(&mut modal, previous, locks),
                "{previous:?}, {name_2}"
            );
            assert_eq!(modal, Modal::Closed, "{previous:?}, {name_2}");
        }
        // Con otro modal abierto, tampoco, y el abierto no cambia.
        let mut modal = open(CanvasBackground::Black);
        let before = modal.clone();
        assert!(!open_background_color(
            &mut modal,
            previous,
            Locks::default()
        ));
        assert_eq!(modal, before);
    }
}

#[test]
fn row_4_the_initial_focus_is_the_field_and_the_tab_order_field_apply_cancel() {
    for (previous, _) in PREVIOUS {
        let mut modal = open(previous);
        let d = dialog(&modal);
        assert_eq!(initial_focus(d), FocusTarget::Field(FIELD_COLOR));
        assert_eq!(
            focus_order(d),
            vec![
                FocusTarget::Field(FIELD_COLOR),
                FocusTarget::Accept,
                FocusTarget::Cancel
            ]
        );
        // Con todo válido, `Aplicar` está en el orden; con el alfa rechazado o un texto inválido,
        // no: solo el campo y `Cancel`.
        for text_2 in ["#fff8", "zz", ""] {
            writes(&mut modal, text_2);
            assert_eq!(
                focus_order(dialog(&modal)),
                vec![FocusTarget::Field(FIELD_COLOR), FocusTarget::Cancel],
                "{previous:?}: {text_2:?}"
            );
        }
        writes(&mut modal, "#fff");
        assert_eq!(
            focus_order(dialog(&modal)),
            vec![
                FocusTarget::Field(FIELD_COLOR),
                FocusTarget::Accept,
                FocusTarget::Cancel
            ]
        );
        // El foco se pide una sola vez y es el del campo.
        let Modal::Open(d) = &mut modal else {
            unreachable!()
        };
        assert_eq!(
            d.take_focus_request(),
            Some(FocusTarget::Field(FIELD_COLOR))
        );
        assert_eq!(d.take_focus_request(), None);
    }
}

#[test]
fn row_4_the_four_lengths_with_and_without_hash_are_valid_with_alpha_ff_from_each_previous_background()
 {
    for (previous, _) in PREVIOUS {
        for (text_2, color) in texts_that_are() {
            let context = format!("{previous:?}: {text_2:?}");
            let mut modal = open(previous);
            writes(&mut modal, text_2);
            let d = dialog(&modal);
            assert_eq!(d.validity(FIELD_COLOR), Some(Validity::Valid), "{context}");
            assert_eq!(d.reason(), None, "{context}");
            assert!(d.is_valid(), "{context}");
            assert_eq!(
                d.values().and_then(|v| v.color(FIELD_COLOR)),
                Some(color),
                "{context}"
            );
            assert_eq!(
                focus_order(d),
                vec![
                    FocusTarget::Field(FIELD_COLOR),
                    FocusTarget::Accept,
                    FocusTarget::Cancel
                ],
                "{context}: `Aplicar` habilitado"
            );
            // Aceptar con el texto sin confirmar lo confirma y da el color.
            let Reaction::Accepted(values_2) = accepts(&mut modal) else {
                panic!("{context}: se acepta");
            };
            assert!(!modal.is_open(), "{context}");
            assert_eq!(
                background_from_values(&values_2),
                Some(CanvasBackground::Custom(color)),
                "{context}"
            );
        }
    }
}

#[test]
fn row_4_with_alpha_less_than_ff_the_field_is_valid_but_rejected_with_the_reason_and_the_dialog_stays()
 {
    for (previous, _) in PREVIOUS {
        for text_2 in TEXTS_WITH_ALPHA {
            let context = format!("{previous:?}: {text_2:?}");
            let mut modal = open(previous);
            writes(&mut modal, text_2);
            // El campo es válido (no hay error en línea); el motivo es el del conjunto.
            let d = dialog(&modal);
            assert_eq!(d.validity(FIELD_COLOR), Some(Validity::Valid), "{context}");
            assert_eq!(d.reason(), Some(OPAQUE_REASON.to_string()), "{context}");
            assert!(!d.is_valid(), "{context}");
            assert_eq!(d.values(), None, "{context}");
            assert_eq!(
                focus_order(d),
                vec![FocusTarget::Field(FIELD_COLOR), FocusTarget::Cancel],
                "{context}: `Aplicar` atenuado"
            );
            // `Accept` da `Rejected` con el motivo y no cambia nada.
            let before = dialog(&modal).clone();
            assert_eq!(
                accepts(&mut modal),
                Reaction::Rejected(OPAQUE_REASON.to_string()),
                "{context}"
            );
            assert!(modal.is_open(), "{context}: el diálogo sigue abierto");
            assert_eq!(dialog(&modal), &before, "{context}: no cambia nada");
            // El mismo resultado tras `Intro` (que solo confirma el campo).
            confirms(&mut modal);
            assert_eq!(
                reason_2(&modal),
                Some(OPAQUE_REASON.to_string()),
                "{context}"
            );
            assert_eq!(dialog(&modal).values(), None, "{context}");
            assert_eq!(
                accepts(&mut modal),
                Reaction::Rejected(OPAQUE_REASON.to_string()),
                "{context}: tras Intro"
            );
            assert!(modal.is_open(), "{context}");
        }
    }
}

#[test]
fn row_4_the_opacity_reason_is_the_footer_one_with_its_prefix_and_fits_without_clipping() {
    let mut modal = open(CanvasBackground::Checker);
    writes(&mut modal, "#ff000080");
    let reason_2 = reason_2(&modal).expect("motivo");
    assert_eq!(reason_2, OPAQUE_REASON, "no se recorta");
    assert!(reason_2.chars().count() <= REASON_MAX_CHARS);
    let footer = format!("{ERROR_PREFIX}{reason_2}");
    assert!(footer.chars().count() <= FOOTER_COLUMNS, "{footer:?}");
    assert!(in_the_set(&footer));
}

#[test]
fn row_4_zz_and_12_give_the_inline_error_of_hex_notice_and_not_the_opacity_one() {
    let cases: [(&str, HexError); 7] = [
        ("zz", HexError::Length(2)),
        ("#12", HexError::Length(2)),
        ("", HexError::Empty),
        ("   ", HexError::Empty),
        ("#", HexError::Empty),
        ("#12345", HexError::Length(5)),
        ("#gggggg", HexError::Digit),
    ];
    for (previous, _) in PREVIOUS {
        for (text_2, error) in &cases {
            let context = format!("{previous:?}: {text_2:?}");
            let mut modal = open(previous);
            writes(&mut modal, text_2);
            let d = dialog(&modal);
            let notice = hex_notice(error).to_string();
            assert_eq!(
                d.validity(FIELD_COLOR),
                Some(Validity::Invalid(notice.clone())),
                "{context}: el error en línea es el de `hex_notice`"
            );
            // El motivo del pie es el del campo (con su rótulo), no el de opacidad.
            let label_2 = d.spec().fields[0].label;
            let expected_2: String = format!("{label_2}: {notice}")
                .chars()
                .take(REASON_MAX_CHARS)
                .collect();
            assert_eq!(d.reason(), Some(expected_2.clone()), "{context}");
            assert_ne!(d.reason(), Some(OPAQUE_REASON.to_string()), "{context}");
            assert_eq!(d.values(), None, "{context}");
            assert_eq!(
                focus_order(d),
                vec![FocusTarget::Field(FIELD_COLOR), FocusTarget::Cancel],
                "{context}"
            );
            assert_eq!(
                accepts(&mut modal),
                Reaction::Rejected(expected_2),
                "{context}"
            );
            assert!(modal.is_open(), "{context}");
        }
    }
}

#[test]
fn row_4_enter_confirms_a_valid_text_and_keeps_the_invalid_one_with_its_error() {
    for (previous, _) in PREVIOUS {
        // Un texto válido: `Intro` lo confirma, el campo lo muestra canónico y `Intro` otra vez acepta.
        let mut modal = open(previous);
        writes(&mut modal, "336699");
        assert!(dialog(&modal).hex_pending(FIELD_COLOR));
        assert_eq!(
            key_action(dialog(&modal), IN_THE_FIELD, DialogKey::Enter, false),
            KeyAction::ConfirmHex(FIELD_COLOR)
        );
        assert_eq!(
            presses(&mut modal, IN_THE_FIELD, DialogKey::Enter),
            Reaction::None
        );
        assert!(!dialog(&modal).hex_pending(FIELD_COLOR));
        assert_eq!(confirmed(&modal), BLUE_STEEL);
        assert_eq!(text_of_the_field(&modal), "#336699");
        assert_eq!(
            key_action(dialog(&modal), IN_THE_FIELD, DialogKey::Enter, false),
            KeyAction::Accept
        );
        let Reaction::Accepted(values_2) = presses(&mut modal, IN_THE_FIELD, DialogKey::Enter)
        else {
            panic!("{previous:?}: el segundo Intro acepta");
        };
        assert_eq!(
            background_from_values(&values_2),
            Some(CanvasBackground::Custom(BLUE_STEEL))
        );

        // Un texto inválido: `Intro` no lo descarta ni confirma, y el error sigue.
        let mut modal = open(previous);
        let before = confirmed(&modal);
        writes(&mut modal, "zz");
        assert_eq!(
            presses(&mut modal, IN_THE_FIELD, DialogKey::Enter),
            Reaction::None
        );
        assert!(dialog(&modal).hex_pending(FIELD_COLOR));
        assert_eq!(text_of_the_field(&modal), "zz");
        assert_eq!(confirmed(&modal), before);
        assert!(matches!(
            dialog(&modal).validity(FIELD_COLOR),
            Some(Validity::Invalid(_))
        ));
    }
}

#[test]
fn row_4_losing_focus_confirms_a_valid_text_and_reverts_the_invalid_one() {
    for (previous, color) in PREVIOUS {
        // Válida: confirma.
        let mut modal = open(previous);
        writes(&mut modal, "#ff8000");
        loses_the_focus(&mut modal);
        assert!(!dialog(&modal).hex_pending(FIELD_COLOR));
        assert_eq!(confirmed(&modal), Rgba::opaque(0xff, 0x80, 0));
        assert_eq!(text_of_the_field(&modal), "#ff8000");

        // Inválida: revierte al último confirmado y el diálogo sigue válido.
        let mut modal = open(previous);
        writes(&mut modal, "zz");
        loses_the_focus(&mut modal);
        assert!(!dialog(&modal).hex_pending(FIELD_COLOR), "{previous:?}");
        assert_eq!(text_of_the_field(&modal), color.to_string(), "{previous:?}");
        assert_eq!(confirmed(&modal), color);
        assert!(dialog(&modal).is_valid());

        // Con alfa: es válida y se confirma (y el motivo sigue visible).
        let mut modal = open(previous);
        writes(&mut modal, "#ff000080");
        loses_the_focus(&mut modal);
        assert_eq!(confirmed(&modal), Rgba::new(0xff, 0, 0, 0x80));
        assert_eq!(reason_2(&modal), Some(OPAQUE_REASON.to_string()));
    }
}

#[test]
fn row_4_escape_with_invalid_pending_text_reverts_it_and_the_next_escape_cancels() {
    for (previous, color) in PREVIOUS {
        let mut modal = open(previous);
        writes(&mut modal, "zz");
        assert_eq!(
            key_action(dialog(&modal), IN_THE_FIELD, DialogKey::Escape, false),
            KeyAction::RevertHex(FIELD_COLOR)
        );
        // El primer `Escape` revierte y el diálogo sigue abierto.
        assert_eq!(
            presses(&mut modal, IN_THE_FIELD, DialogKey::Escape),
            Reaction::None
        );
        assert!(modal.is_open(), "{previous:?}");
        assert!(!dialog(&modal).hex_pending(FIELD_COLOR));
        assert_eq!(text_of_the_field(&modal), color.to_string());
        assert!(dialog(&modal).is_valid());
        // El siguiente cancela.
        assert_eq!(
            key_action(dialog(&modal), IN_THE_FIELD, DialogKey::Escape, false),
            KeyAction::Cancel
        );
        assert_eq!(
            presses(&mut modal, IN_THE_FIELD, DialogKey::Escape),
            Reaction::Cancelled
        );
        assert!(!modal.is_open());
    }
}

#[test]
fn row_4_escape_with_valid_pending_text_or_with_alpha_also_reverts_it_before_cancelling() {
    for text_2 in ["#336699", "#ff000080", "fff"] {
        let mut modal = open(CanvasBackground::Black);
        writes(&mut modal, text_2);
        assert_eq!(
            presses(&mut modal, IN_THE_FIELD, DialogKey::Escape),
            Reaction::None
        );
        assert!(modal.is_open(), "{text_2:?}");
        assert!(!dialog(&modal).hex_pending(FIELD_COLOR));
        assert_eq!(text_of_the_field(&modal), "#000000");
        assert!(
            dialog(&modal).is_valid(),
            "el motivo desaparece con el texto"
        );
        assert_eq!(
            presses(&mut modal, IN_THE_FIELD, DialogKey::Escape),
            Reaction::Cancelled
        );
    }
    // Sin texto pendiente, `Escape` cancela a la primera.
    let mut modal = open(CanvasBackground::White);
    assert_eq!(
        presses(&mut modal, IN_THE_FIELD, DialogKey::Escape),
        Reaction::Cancelled
    );
}

#[test]
fn row_4_the_confirmed_alpha_sequence_is_dragged_until_writing_eight_digits_with_ff() {
    for (previous, _) in PREVIOUS {
        let context = |step: &str| format!("{previous:?}: {step}");
        let mut modal = open(previous);
        // 1. Escribir `#ff000080`: el campo es válido y el conjunto no.
        writes(&mut modal, "#ff000080");
        assert_eq!(
            reason_2(&modal),
            Some(OPAQUE_REASON.to_string()),
            "{}",
            context("#ff000080 sin confirmar")
        );
        // 2. `Intro`: el campo confirma ese valor y el motivo queda visible.
        confirms(&mut modal);
        assert_eq!(
            text_of_the_field(&modal),
            "#ff000080",
            "{}",
            context("tras Intro")
        );
        assert_eq!(
            confirmed(&modal),
            Rgba::new(0xff, 0, 0, 0x80),
            "{}",
            context("el confirmado lleva alfa 80")
        );
        assert!(!dialog(&modal).hex_pending(FIELD_COLOR));
        assert_eq!(reason_2(&modal), Some(OPAQUE_REASON.to_string()));
        assert_eq!(
            accepts(&mut modal),
            Reaction::Rejected(OPAQUE_REASON.to_string()),
            "{}",
            context("Aplicar tras #ff000080")
        );
        assert!(modal.is_open());

        // 3. `#fff` y `#ffffff`, cada una con `Intro`: el alfa que se toma es el `80` confirmado.
        for text_2 in ["#fff", "#ffffff", "fff", "ffffff"] {
            writes(&mut modal, text_2);
            assert_eq!(
                reason_2(&modal),
                Some(OPAQUE_REASON.to_string()),
                "{}",
                context(&format!("{text_2} sin confirmar"))
            );
            assert_eq!(dialog(&modal).values(), None);
            confirms(&mut modal);
            assert_eq!(
                text_of_the_field(&modal),
                "#ffffff80",
                "{}",
                context(&format!("{text_2} + Intro"))
            );
            assert_eq!(confirmed(&modal), Rgba::new(0xff, 0xff, 0xff, 0x80));
            assert_eq!(reason_2(&modal), Some(OPAQUE_REASON.to_string()));
            assert_eq!(
                accepts(&mut modal),
                Reaction::Rejected(OPAQUE_REASON.to_string()),
                "{}",
                context(&format!("Aplicar tras {text_2}"))
            );
            assert!(modal.is_open());
        }

        // 4. `#ffffffff` vale (alfa `ff`): `Aplicar` acepta y da `Custom(#ffffff)`.
        writes(&mut modal, "#ffffffff");
        assert_eq!(reason_2(&modal), None, "{}", context("#ffffffff"));
        assert_eq!(
            focus_order(dialog(&modal)),
            vec![
                FocusTarget::Field(FIELD_COLOR),
                FocusTarget::Accept,
                FocusTarget::Cancel
            ]
        );
        let Reaction::Accepted(values_2) = accepts(&mut modal) else {
            panic!("{}", context("#ffffffff se acepta"));
        };
        assert!(!modal.is_open());
        assert_eq!(
            background_from_values(&values_2),
            Some(CanvasBackground::Custom(WHITE)),
            "{}",
            context("Custom(#ffffff)")
        );
    }
}

#[test]
fn row_4_confirming_eight_digits_with_ff_returns_to_opaque_alpha_for_the_three_and_six_digit_ones()
{
    // La salida de la secuencia sin cerrar el diálogo: confirmar `#ffffffff` y seguir con `#fff`.
    let mut modal = open(CanvasBackground::Checker);
    writes(&mut modal, "#ff000080");
    confirms(&mut modal);
    assert!(!dialog(&modal).is_valid());
    writes(&mut modal, "#336699ff");
    confirms(&mut modal);
    assert_eq!(confirmed(&modal), BLUE_STEEL);
    assert_eq!(reason_2(&modal), None);
    writes(&mut modal, "#fff");
    assert_eq!(
        reason_2(&modal),
        None,
        "con el confirmado opaco, `#fff` vale"
    );
    confirms(&mut modal);
    assert_eq!(text_of_the_field(&modal), "#ffffff");
}

#[test]
fn row_4_cancel_and_reopen_is_born_with_alpha_ff_even_if_there_was_a_confirmed_alpha_before() {
    for (previous, color) in PREVIOUS {
        let mut modal = open(previous);
        writes(&mut modal, "#ff000080");
        confirms(&mut modal);
        assert_eq!(confirmed(&modal).a, 0x80);
        // `Cancel` cierra sin respuesta de aceptación.
        assert_eq!(modal.event(Event::Cancel), Reaction::Cancelled);
        assert!(!modal.is_open());
        // Reabrir: el confirmado vuelve a ser el color del fondo, opaco; `#fff` vale.
        let mut modal = open(previous);
        assert_eq!(confirmed(&modal), color);
        assert_eq!(confirmed(&modal).a, 255);
        assert_eq!(reason_2(&modal), None);
        writes(&mut modal, "#fff");
        assert_eq!(
            reason_2(&modal),
            None,
            "{previous:?}: `#fff` vale al reabrir"
        );
        // Con `Escape` (sin texto pendiente, tras confirmar) pasa lo mismo.
        let mut modal = open(previous);
        writes(&mut modal, "#ff000080");
        confirms(&mut modal);
        assert_eq!(
            presses(&mut modal, IN_THE_FIELD, DialogKey::Escape),
            Reaction::Cancelled
        );
        let modal = open(previous);
        assert_eq!(confirmed(&modal), color);
    }
}

#[test]
fn row_4_background_from_values_gives_custom_with_the_opaque_color_of_the_values() {
    for (text_2, color) in texts_that_are() {
        let mut modal = open(CanvasBackground::Checker);
        writes(&mut modal, text_2);
        let values_2 = dialog(&modal).values().expect("valen");
        let background_2 = background_from_values(&values_2).expect("un fondo");
        assert_eq!(background_2, CanvasBackground::Custom(color), "{text_2:?}");
        let CanvasBackground::Custom(c) = background_2 else {
            unreachable!()
        };
        assert_eq!(c.a, 255, "{text_2:?}: el color del fondo es opaco");
        // El valor que sale en el menú es el mismo color.
        assert_eq!(
            background_value_text(background_2),
            Some(format!("#{:02x}{:02x}{:02x}", color.r, color.g, color.b))
        );
    }
}

// --- Textos y contraste ---

#[test]
fn row_4_all_fixed_text_and_every_reason_fit_in_37_characters_and_are_in_the_set() {
    for (previous, color) in PREVIOUS {
        let spec = background_color_form(initial_color(previous));
        for text_2 in [spec.title, spec.accept, spec.cancel.expect("hay Cancelar")] {
            assert!(in_the_set(text_2), "{text_2:?}");
            assert!(text_2.chars().count() <= 44, "{text_2:?}");
        }
        for field_2 in &spec.fields {
            assert!(in_the_set(field_2.label), "{:?}", field_2.label);
        }
        assert!(in_the_set(&color.to_string()));
        // Todos los motivos que puede dar el diálogo: opacidad y los errores del campo.
        let mut modal = open(previous);
        for text_2 in [
            "#ff000080",
            "zz",
            "#12",
            "",
            "   ",
            "#12345",
            "#gggggg",
            "#fff8",
            "123456789",
        ] {
            writes(&mut modal, text_2);
            if let Some(reason_2) = reason_2(&modal) {
                assert!(in_the_set(&reason_2), "{reason_2:?}");
                assert!(
                    reason_2.chars().count() <= REASON_MAX_CHARS,
                    "{reason_2:?} mide {}",
                    reason_2.chars().count()
                );
                let footer = format!("{ERROR_PREFIX}{reason_2}");
                assert!(footer.chars().count() <= FOOTER_COLUMNS, "{footer:?}");
            }
            if let Some(Validity::Invalid(notice)) = dialog(&modal).validity(FIELD_COLOR) {
                assert!(in_the_set(&notice), "{notice:?}");
            }
        }
    }
    for error in [
        HexError::Empty,
        HexError::Length(2),
        HexError::Digit,
        HexError::NotGray,
        HexError::NoAlpha,
    ] {
        assert!(in_the_set(hex_notice(&error)));
    }
}

/// Un texto pintado, en orden de pintado.
#[derive(Clone, Debug)]
struct TextPainted {
    text_2: String,
    rect: Rect,
    clip: Rect,
    colors: Vec<Color32>,
    opacity_2: f32,
    /// El color del primer vértice de glifo: lo que sale por pantalla (con el atenuado aplicado).
    glyph: Option<Color32>,
}

#[derive(Clone, Debug)]
enum Painted {
    Text(TextPainted),
    Fill {
        rect: Rect,
        fill: Color32,
        clip: Rect,
    },
}

fn flattens(shape: &Shape, clip: Rect, outside: &mut Vec<Painted>) {
    match shape {
        Shape::Text(t) => {
            let job = &t.galley.job;
            let colors = job
                .sections
                .iter()
                .map(|s| {
                    t.override_text_color
                        .unwrap_or(if s.format.color == Color32::PLACEHOLDER {
                            t.fallback_color
                        } else {
                            s.format.color
                        })
                })
                .collect();
            let glyph = t.galley.rows.iter().find_map(|row| {
                let visual = &row.row.visuals;
                visual.mesh.vertices[visual.glyph_vertex_range.clone()]
                    .first()
                    .map(|v| v.color)
            });
            outside.push(Painted::Text(TextPainted {
                text_2: t.galley.text().to_owned(),
                rect: Rect::from_min_size(t.pos, t.galley.size()),
                clip,
                colors,
                opacity_2: t.opacity_factor,
                glyph,
            }));
        }
        Shape::Rect(r) => outside.push(Painted::Fill {
            rect: r.rect,
            fill: r.fill,
            clip,
        }),
        Shape::Vec(shapes) => {
            for shape_2 in shapes {
                flattens(shape_2, clip, outside);
            }
        }
        _ => {}
    }
}

fn context_of(ppp: f32) -> Context {
    let ctx = Context::default();
    ctx.set_pixels_per_point(ppp);
    ctx.set_missing_glyph_policy(MissingGlyphPolicy::Panic);
    install_fonts(&ctx);
    ctx.set_visuals_of(eframe::egui::Theme::Dark, visuals(ppp));
    for _ in 0..2 {
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(100.0, 100.0))),
            ..RawInput::default()
        };
        let output = ctx.run_ui(input, |ui| apply_scale(ui.ctx()));
        output.drop_without_applying_deltas();
    }
    ctx
}

/// `src` (premultiplicado, como egui) por encima de `dst` opaco.
fn above(dst: Color32, src: Color32) -> Color32 {
    let k = 1.0 - f32::from(src.a()) / 255.0;
    let channel = |s: u8, d: u8| (f32::from(s) + f32::from(d) * k).round().clamp(0.0, 255.0) as u8;
    Color32::from_rgb(
        channel(src.r(), dst.r()),
        channel(src.g(), dst.g()),
        channel(src.b(), dst.b()),
    )
}

/// Pinta `dialog` unos fotogramas (con el reloj avanzando, para que acabe cualquier animación) a la
/// escala `ppp` y una ventana de 960 × 720 puntos, y devuelve lo pintado del último, en orden.
fn paints_the_dialog(ppp: f32, dialog: &mut Dialog) -> Vec<Painted> {
    let ctx = context_of(ppp);
    let mut focus = None;
    let mut last_2_2 = Vec::new();
    let screen: Vec2 = vec2(960.0, 720.0);
    for frame in 0..6u32 {
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, screen)),
            time: Some(f64::from(frame) * 0.25),
            ..RawInput::default()
        };
        let mut output = ctx.run_ui(input, |ui| {
            apply_scale(ui.ctx());
            let reaction = show_dialog(ui.ctx(), dialog, &mut focus);
            assert_eq!(reaction, Reaction::None, "sin eventos no hay respuesta");
        });
        output.textures_delta.clear();
        let shapes: Vec<ClippedShape> = std::mem::take(&mut output.shapes);
        last_2_2.clear();
        for clipped in &shapes {
            flattens(&clipped.shape, clipped.clip_rect, &mut last_2_2);
        }
    }
    last_2_2
}

/// Los fallos de contraste de los textos no atenuados y cuántos se comprobaron.
fn failures_of_contrast(painted: &[Painted]) -> (Vec<String>, usize) {
    let mut failures = Vec::new();
    let mut checked = 0;
    for (i, shape_2) in painted.iter().enumerate() {
        let Painted::Text(t) = shape_2 else { continue };
        let visible = t.rect.intersect(t.clip);
        if t.text_2.trim().is_empty() || !visible.is_positive() {
            continue;
        }
        let dimmed = t.opacity_2 < 1.0
            || t.colors.iter().any(|c| *c == TEXT_DISABLED || c.a() < 255)
            || t.glyph.is_some_and(|c| c.a() < 255 || c == TEXT_DISABLED);
        if dimmed {
            continue;
        }
        let probe = visible.shrink2(vec2(0.0, visible.height() * 0.25));
        let mut background_2 = BACKGROUND;
        let mut there_is_fill = false;
        for previous in &painted[..i] {
            if let Painted::Fill { rect, fill, clip } = previous
                && rect.intersect(*clip).expand(0.01).contains_rect(probe)
            {
                there_is_fill = true;
                background_2 = above(background_2, *fill);
            }
        }
        if !there_is_fill {
            failures.push(format!("el texto {:?} no tiene relleno debajo", t.text_2));
            continue;
        }
        let color = t
            .glyph
            .or_else(|| t.colors.first().copied())
            .unwrap_or(Color32::PLACEHOLDER);
        checked += 1;
        let ratio = contrast_ratio(color, background_2);
        if ratio < 4.5 {
            failures.push(format!(
                "el texto {:?} ({color:?}) sobre {background_2:?} da {ratio:.2} (< 4,5)",
                t.text_2
            ));
        }
    }
    (failures, checked)
}

fn texts_painted(painted: &[Painted]) -> Vec<&str> {
    painted
        .iter()
        .filter_map(|f| match f {
            Painted::Text(t) => Some(t.text_2.as_str()),
            Painted::Fill { .. } => None,
        })
        .collect()
}

#[test]
fn row_4_the_dialog_paints_its_texts_with_contrast_of_at_least_4_5_and_the_reason_visible() {
    for ppp in PPPS {
        // (texto escrito, confirmado con Intro, lo que debe verse además)
        let states: [(&str, bool, &str); 4] = [
            ("", false, "Fondo del lienzo"),
            (
                "#ff000080",
                true,
                "error: el fondo debe ser opaco (alfa ff)",
            ),
            ("#fff8", false, "error: el fondo debe ser opaco (alfa ff)"),
            ("zz", false, "usa 3, 4, 6 u 8 dígitos"),
        ];
        for (text_2, intro, visible) in states {
            let mut modal = open(CanvasBackground::Checker);
            if !text_2.is_empty() {
                writes(&mut modal, text_2);
            }
            if intro {
                confirms(&mut modal);
            }
            let Modal::Open(dialog) = &mut modal else {
                unreachable!()
            };
            let painted = paints_the_dialog(ppp, dialog);
            let label_2 = format!("ppp {ppp}, texto {text_2:?}");
            let texts = texts_painted(&painted);
            for expected_2 in ["Fondo del lienzo", "Cancelar", "Aplicar", visible] {
                assert!(
                    texts.iter().any(|t| t.contains(expected_2)),
                    "{label_2}: falta {expected_2:?}; se pinta {texts:?}"
                );
            }
            // El motivo de opacidad nunca va cortado y el error en línea lleva su prefijo.
            if visible.contains("opaco") {
                assert!(
                    texts
                        .iter()
                        .any(|t| *t == format!("{ERROR_PREFIX}{OPAQUE_REASON}")),
                    "{label_2}: el motivo se ve entero en el pie; se pinta {texts:?}"
                );
            }
            let (failures, checked) = failures_of_contrast(&painted);
            assert!(
                checked >= 3,
                "{label_2}: solo se comprobaron {checked} textos; {texts:?}"
            );
            assert!(failures.is_empty(), "{label_2}: {failures:#?}");
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 5: sesión y lienzo, sin ventana
// ---------------------------------------------------------------------------------------------

static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Directorio temporal propio del test; el guardián lo borra al soltarse.
struct TempDir {
    path_2: PathBuf,
}

impl TempDir {
    fn new_2() -> TempDir {
        let n = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let path_2 = std::env::temp_dir().join(format!(
            "pixoko-px-app-fondo-color-{}-{n}",
            std::process::id()
        ));
        fs::create_dir_all(&path_2).expect("crear el directorio temporal");
        TempDir { path_2 }
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path_2);
    }
}

fn image_rgba() -> Image {
    let mut image = Image::new(8, 6, Mode::Rgba).expect("imagen RGBA");
    for y in 0..6 {
        for x in 0..8 {
            let color = match x {
                0..=1 => Rgba::new(0, 0, 0, 0),
                2..=3 => Rgba::new(200, 10, 10, 128),
                _ => Rgba::opaque(10 + x as u8, 20 + y as u8, 30),
            };
            image.set_pixel(x, y, Pixel::Rgba(color)).expect("dentro");
        }
    }
    image
}

/// Una edición de un píxel, distinta cada vez (`i`), sobre la imagen `image` del documento.
fn editing(image: ImageId, i: u32) -> Command {
    Command::SetPixels {
        image,
        pixels: vec![PixelEdit {
            x: i % 8,
            y: i / 8 % 6,
            pixel: Pixel::Rgba(Rgba::opaque(7, 8, 9 + i as u8)),
        }],
    }
}

/// Lo que un paso de fondo no debe tocar de la sesión.
#[derive(Clone, Debug, PartialEq)]
struct TestSnapshot {
    state_id: u64,
    modified: bool,
    can_undo: bool,
    can_redo: bool,
    history_bytes: usize,
    image: Image,
    path: Option<PathBuf>,
    choice: ColorChoice,
}

/// La sesión, el lienzo y el estado de los diálogos: lo que cablea `main.rs`.
struct World {
    session: Session,
    canvas: Canvas,
    modal: Modal,
}

impl World {
    /// Una RGBA de 8×6 con dos pasos de historia y uno deshecho. Con `modified`, una edición
    /// posterior sin guardar (el redo se pierde).
    fn new_2(background_2: CanvasBackground, modified: bool) -> World {
        let mut document = Document::new(image_rgba());
        let id = ImageId::from_raw(0);
        for i in 0..2 {
            document.apply(editing(id, i)).expect("paso");
        }
        document.undo().expect("deshacer");
        let mut session = Session::sample(document);
        if modified {
            session.document.apply(editing(id, 9)).expect("edición");
            assert!(session.modified());
        }
        let mut canvas = Canvas::new();
        canvas.set_image(session.document.image(id).expect("imagen"));
        canvas.set_background(background_2);
        World {
            session,
            canvas,
            modal: Modal::default(),
        }
    }

    fn snapshot(&self) -> TestSnapshot {
        let image = self
            .session
            .document
            .image(ImageId::from_raw(0))
            .expect("imagen")
            .clone();
        TestSnapshot {
            state_id: self.session.document.state_id(),
            modified: self.session.modified(),
            can_undo: self.session.document.can_undo(),
            can_redo: self.session.document.can_redo(),
            history_bytes: self.session.document.history_bytes(),
            image,
            path: self.session.path().map(PathBuf::from),
            choice: self.session.choice,
        }
    }

    fn image_2(&self) -> Image {
        self.snapshot().image
    }

    fn background_2(&self) -> CanvasBackground {
        self.canvas.background()
    }

    /// `Ver → TestBackground del canvas → Color…` con estos bloqueos.
    fn opens(&mut self, locks: Locks) -> bool {
        let background_2 = self.canvas.background();
        open_background_color(&mut self.modal, background_2, locks)
    }

    /// Un evento al diálogo y, si se acepta, lo que hace `App` (D8): `Accepted` →
    /// `background_from_values` → `Canvas::set_background`.
    fn event_2(&mut self, event: Event) -> Reaction {
        let reaction = self.modal.event(event);
        self.applies(&reaction);
        reaction
    }

    fn applies(&mut self, reaction: &Reaction) {
        if let Reaction::Accepted(values_2) = reaction
            && let Some(background_2) = background_from_values(values_2)
        {
            self.canvas.set_background(background_2);
        }
    }

    fn writes(&mut self, text_2: &str) {
        let r = self.event_2(Event::Hex {
            field: FIELD_COLOR,
            text: text_2.to_string(),
        });
        assert_eq!(r, Reaction::None);
    }

    fn presses(&mut self, key_2: DialogKey) -> Reaction {
        let reaction = presses(&mut self.modal, IN_THE_FIELD, key_2);
        self.applies(&reaction);
        reaction
    }
}

const MODIFIED: [bool; 2] = [false, true];

#[test]
fn row_5_the_test_session_can_be_undone_and_redone_or_is_modified() {
    // Control: sin esto las comparaciones de `TestSnapshot` probarían poco.
    let m = World::new_2(CanvasBackground::Checker, false);
    assert!(m.session.document.can_undo() && m.session.document.can_redo());
    assert!(!m.session.modified());
    assert!(m.session.document.history_bytes() > 0);
    let m = World::new_2(CanvasBackground::Checker, true);
    assert!(m.session.modified() && m.session.document.can_undo());
}

#[test]
fn row_5_accepting_336699_gives_custom_in_the_canvas_the_menu_and_the_value_from_each_previous_background()
 {
    for (previous, _) in PREVIOUS {
        for modified in MODIFIED {
            let context = format!("{previous:?}, modificada {modified}");
            let mut m = World::new_2(previous, modified);
            let before = m.snapshot();
            assert!(m.opens(Locks::default()), "{context}");
            assert_eq!(m.modal.kind(), Some(DialogKind::BackgroundColor));
            // Mientras está abierto el fondo no cambia (sin vista previa en vivo).
            m.writes("#336699");
            assert_eq!(m.background_2(), previous, "{context}: sin vista previa");
            assert_eq!(
                m.presses(DialogKey::Enter),
                Reaction::None,
                "{context}: confirma"
            );
            assert_eq!(
                m.background_2(),
                previous,
                "{context}: confirmar tampoco aplica"
            );
            let Reaction::Accepted(_) = m.presses(DialogKey::Enter) else {
                panic!("{context}: el segundo Intro acepta");
            };
            assert!(!m.modal.is_open());
            // El lienzo y el menú lo dicen.
            assert_eq!(
                m.background_2(),
                CanvasBackground::Custom(BLUE_STEEL),
                "{context}"
            );
            assert!(background_checked(
                AppAction::BackgroundColor,
                m.background_2()
            ));
            for fixed in [
                CanvasBackground::Checker,
                CanvasBackground::Black,
                CanvasBackground::Gray,
                CanvasBackground::White,
            ] {
                assert!(!background_checked(
                    AppAction::CanvasBackground(fixed),
                    m.background_2()
                ));
            }
            assert_eq!(
                background_value_text(m.background_2()),
                Some("#336699".to_string()),
                "{context}"
            );
            // Es solo visualización: la sesión, igual.
            assert_eq!(
                m.snapshot(),
                before,
                "{context}: ningún paso de fondo toca la sesión"
            );
        }
    }
}

#[test]
fn row_5_accepting_with_the_apply_button_and_with_the_default_also_gives_custom() {
    for (previous, color) in PREVIOUS {
        let mut m = World::new_2(previous, false);
        let before = m.snapshot();
        assert!(m.opens(Locks::default()));
        let Reaction::Accepted(_) = m.event_2(Event::Accept) else {
            panic!("{previous:?}: Aplicar");
        };
        assert_eq!(
            m.background_2(),
            CanvasBackground::Custom(color),
            "{previous:?}"
        );
        assert_eq!(m.snapshot(), before);
    }
}

#[test]
fn row_5_cancel_leaves_the_previous_background_whichever_it_is() {
    for (previous, _) in PREVIOUS {
        for modified in MODIFIED {
            let mut m = World::new_2(previous, modified);
            let before = m.snapshot();
            assert!(m.opens(Locks::default()));
            m.writes("#336699");
            assert_eq!(m.event_2(Event::Cancel), Reaction::Cancelled);
            assert!(!m.modal.is_open());
            assert_eq!(m.background_2(), previous, "{previous:?}: cancelar");
            assert_eq!(m.snapshot(), before, "{previous:?}");
            // También con un texto confirmado (y con alfa) antes de cancelar.
            assert!(m.opens(Locks::default()));
            m.writes("#ff000080");
            m.presses(DialogKey::Enter);
            assert_eq!(m.event_2(Event::Cancel), Reaction::Cancelled);
            assert_eq!(
                m.background_2(),
                previous,
                "{previous:?}: cancelar tras confirmar"
            );
            assert_eq!(m.snapshot(), before);
        }
    }
}

#[test]
fn row_5_escape_leaves_the_previous_background_whichever_it_is() {
    for (previous, _) in PREVIOUS {
        let mut m = World::new_2(previous, false);
        let before = m.snapshot();
        // Sin texto pendiente, `Escape` cancela a la primera.
        assert!(m.opens(Locks::default()));
        assert_eq!(m.presses(DialogKey::Escape), Reaction::Cancelled);
        assert!(!m.modal.is_open());
        assert_eq!(m.background_2(), previous);
        // Con texto pendiente, el primero lo descarta y el segundo cancela.
        assert!(m.opens(Locks::default()));
        m.writes("#336699");
        assert_eq!(m.presses(DialogKey::Escape), Reaction::None);
        assert!(m.modal.is_open());
        assert_eq!(m.background_2(), previous);
        assert_eq!(m.presses(DialogKey::Escape), Reaction::Cancelled);
        assert_eq!(m.background_2(), previous, "{previous:?}");
        assert_eq!(m.snapshot(), before);
    }
}

#[test]
fn row_5_quit_abandons_the_dialog_and_leaves_the_previous_background_whichever_it_is() {
    for (previous, _) in PREVIOUS {
        let mut m = World::new_2(previous, true);
        let before = m.snapshot();
        assert!(m.opens(Locks::default()));
        m.writes("#336699");
        m.presses(DialogKey::Enter);
        assert!(m.modal.is_open());
        // `Quit` (y el cierre de la ventana): `Modal::abandon`, sin reacción.
        assert!(m.modal.abandon());
        assert!(!m.modal.is_open());
        assert_eq!(m.background_2(), previous, "{previous:?}");
        assert_eq!(m.snapshot(), before);
        // Y no queda nada: otro `abandon` no hace nada y se puede reabrir con el color del fondo.
        assert!(!m.modal.abandon());
        assert!(m.opens(Locks::default()));
        assert_eq!(confirmed(&m.modal), initial_color(previous));
    }
}

#[test]
fn row_5_with_each_lock_open_background_color_gives_false_and_changes_nothing() {
    for (previous, _) in PREVIOUS {
        for (name_2, locks) in locks_one_by_one() {
            let mut m = World::new_2(previous, false);
            let before = m.snapshot();
            assert!(!m.opens(locks), "{previous:?}, {name_2}");
            assert!(!m.modal.is_open(), "{previous:?}, {name_2}");
            assert_eq!(m.background_2(), previous);
            assert_eq!(m.snapshot(), before);
        }
        // Con los cinco a la vez, también.
        let mut m = World::new_2(previous, false);
        assert!(!m.opens(locks_2(31)));
        // Un segundo `Color…` con el diálogo abierto no hace nada.
        assert!(m.opens(Locks::default()));
        m.writes("#336699");
        let before = m.modal.clone();
        assert!(!m.opens(Locks::default()));
        assert_eq!(m.modal, before);
    }
}

type Operation = fn(&mut World, &TempDir);

fn op_set_image(m: &mut World, _: &TempDir) {
    let image_2 = m.image_2();
    m.canvas.set_image(&image_2);
}

fn op_set_owned_image(m: &mut World, _: &TempDir) {
    m.canvas.set_owned_image(image_rgba());
}

fn op_new_image(m: &mut World, _: &TempDir) {
    let image_2 = Image::new(5, 4, Mode::Rgba).expect("imagen");
    m.session.new_image(image_2.clone(), true).expect("nueva");
    m.canvas.set_owned_image(image_2);
}

fn op_open_a_png(m: &mut World, dir: &TempDir) {
    let path_2 = dir.path_2.join("abierta.png");
    fs::write(&path_2, write_png(&image_rgba()).expect("PNG")).expect("escribir");
    m.session.open(&path_2, 16384, true).expect("abrir");
    let image_2 = m.image_2();
    m.canvas.set_image(&image_2);
}

fn op_convert_mode(m: &mut World, _: &TempDir) {
    let allowed = Allowed {
        luma: true,
        drop_alpha: Some([0, 0, 0]),
    };
    assert_eq!(m.session.convert_mode(Mode::Gray, allowed), Ok(true));
    let image_2 = m.image_2();
    m.canvas.set_image(&image_2);
}

fn op_undo(m: &mut World, _: &TempDir) {
    m.session.undo().expect("hay algo que deshacer");
}

fn op_redo(m: &mut World, _: &TempDir) {
    m.session.redo().expect("hay algo que rehacer");
}

/// Las operaciones de documento de la fila 5, como las ejecuta `App`: cada una cambia el documento
/// y deja el lienzo con la imagen nueva (`set_image`, `set_owned_image`), pero no toca el fondo.
fn operations_of_document() -> [(&'static str, Operation); 7] {
    [
        ("set_image", op_set_image),
        ("set_owned_image", op_set_owned_image),
        ("Nueva imagen…", op_new_image),
        ("abrir un PNG", op_open_a_png),
        ("convertir de modo", op_convert_mode),
        ("deshacer", op_undo),
        ("rehacer", op_redo),
    ]
}

#[test]
fn row_5_the_background_survives_set_image_set_owned_image_new_image_opening_a_png_and_converting()
{
    for (previous, _) in PREVIOUS {
        for (name_2, operation) in operations_of_document() {
            let dir = TempDir::new_2();
            let mut m = World::new_2(previous, false);
            operation(&mut m, &dir);
            assert_eq!(m.background_2(), previous, "{previous:?} tras {name_2}");
        }
    }
}

#[test]
fn row_5_an_accepted_color_survives_the_document_operations() {
    for (previous, _) in PREVIOUS {
        for (name_2, operation) in operations_of_document() {
            let dir = TempDir::new_2();
            let mut m = World::new_2(previous, false);
            assert!(m.opens(Locks::default()));
            m.writes("#336699");
            m.presses(DialogKey::Enter);
            m.presses(DialogKey::Enter);
            assert_eq!(m.background_2(), CanvasBackground::Custom(BLUE_STEEL));
            operation(&mut m, &dir);
            assert_eq!(
                m.background_2(),
                CanvasBackground::Custom(BLUE_STEEL),
                "{previous:?} tras {name_2}"
            );
            assert_eq!(
                background_value_text(m.background_2()),
                Some("#336699".to_string()),
                "{previous:?} tras {name_2}: el menú sigue diciéndolo"
            );
        }
    }
}

#[test]
fn row_5_each_background_step_leaves_the_session_equal_to_before() {
    // Una sola sesión recorre todos los pasos de fondo seguidos: `TestSnapshot` igual tras cada uno.
    for modified in MODIFIED {
        let mut m = World::new_2(CanvasBackground::Checker, modified);
        let before = m.snapshot();
        for (previous, _) in PREVIOUS {
            m.canvas.set_background(previous);
            assert_eq!(m.snapshot(), before, "elegir {previous:?}");
            assert!(m.opens(Locks::default()));
            assert_eq!(m.snapshot(), before, "abrir desde {previous:?}");
            m.writes("#336699");
            assert_eq!(m.snapshot(), before, "teclear");
            m.presses(DialogKey::Enter);
            assert_eq!(m.snapshot(), before, "confirmar");
            m.presses(DialogKey::Enter);
            assert_eq!(m.snapshot(), before, "aceptar desde {previous:?}");
            assert_eq!(m.background_2(), CanvasBackground::Custom(BLUE_STEEL));
        }
        assert_eq!(m.snapshot(), before);
    }
}

#[test]
fn row_5_a_second_color_with_custom_active_is_born_with_that_color() {
    for (previous, _) in PREVIOUS {
        let mut m = World::new_2(previous, false);
        assert!(m.opens(Locks::default()));
        m.writes("#336699");
        m.presses(DialogKey::Enter);
        m.presses(DialogKey::Enter);
        assert_eq!(m.background_2(), CanvasBackground::Custom(BLUE_STEEL));
        // El segundo `Color…` nace con el color del fondo actual, el que se eligió.
        assert!(m.opens(Locks::default()));
        assert_eq!(text_of_the_field(&m.modal), "#336699", "{previous:?}");
        assert_eq!(confirmed(&m.modal), BLUE_STEEL);
        assert_eq!(reason_2(&m.modal), None);
        // Cancelarlo no cambia nada, y tras `Black` nace en `#000000` (no recuerda el propio).
        assert_eq!(m.event_2(Event::Cancel), Reaction::Cancelled);
        assert_eq!(m.background_2(), CanvasBackground::Custom(BLUE_STEEL));
        m.canvas.set_background(CanvasBackground::Black);
        assert!(m.opens(Locks::default()));
        assert_eq!(text_of_the_field(&m.modal), "#000000", "{previous:?}");
    }
}

#[test]
fn row_5_accepting_two_colors_in_a_row_leaves_the_last() {
    let mut m = World::new_2(CanvasBackground::Checker, false);
    for text_2 in ["#336699", "#ff8000", "#010203"] {
        assert!(m.opens(Locks::default()));
        m.writes(text_2);
        m.presses(DialogKey::Enter);
        m.presses(DialogKey::Enter);
        let color = Rgba::from_hex(text_2).expect("color");
        assert_eq!(
            m.background_2(),
            CanvasBackground::Custom(color),
            "{text_2}"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 8, parte de `px-app`: el fichero guardado no lleva el fondo
// ---------------------------------------------------------------------------------------------

fn is_checker_gray(c: &[u8; 4]) -> bool {
    c[3] == 255
        && [CHECK_A, CHECK_B]
            .iter()
            .any(|g| (g.r, g.g, g.b) == (c[0], c[1], c[2]))
}

/// Un fotograma sin ventana en el orden de la app: `interact` → `set_overlay` → `sync` → `paint`.
fn frame_of_the_canvas(
    ctx: &Context,
    canvas: &mut Canvas,
    document: &Image,
    overlay: &[(u32, u32, Rgba)],
) {
    let input = RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(200.0, 150.0))),
        ..RawInput::default()
    };
    let mut output = ctx.run_ui(input, |ui| {
        canvas.interact_roles(ui);
        canvas.set_overlay(ui.ctx(), document, overlay);
        canvas.sync(ui.ctx(), document).expect("sync");
        canvas.paint(ui);
    });
    output.textures_delta.clear();
}

#[test]
fn row_8_write_png_gives_the_same_bytes_with_any_background_and_the_reading_is_the_original() {
    // Alfa 0 en las columnas 0 y 1, 128 en las 2 y 3 y opacas el resto; ningún píxel opaco con un
    // color de los fondos que se prueban ni con un gris del damero.
    let original = image_rgba();
    let doc = Document::new(original.clone());
    let document = doc
        .image(ImageId::from_raw(0))
        .expect("imagen del documento");
    let state_id = doc.state_id();
    let bytes_rgba = to_rgba8(document);
    let png_before = write_png(document).expect("PNG antes");

    let ctx = Context::default();
    let mut canvas = Canvas::new();
    canvas.set_image(document);
    let overlay = [
        (0, 0, Rgba::new(0, 0, 0, 0)),
        (3, 2, Rgba::new(255, 0, 0, 128)),
        (6, 4, Rgba::opaque(255, 0, 0)),
    ];
    let backgrounds = [
        CanvasBackground::Custom(BLUE_STEEL),
        CanvasBackground::Custom(Rgba::opaque(0xff, 0x80, 0)),
        CanvasBackground::Custom(Rgba::opaque(1, 2, 3)),
        CanvasBackground::Custom(Rgba::opaque(40, 40, 40)),
        CanvasBackground::Checker,
        CanvasBackground::Black,
        CanvasBackground::Gray,
        CanvasBackground::White,
    ];
    for background_2 in backgrounds {
        canvas.set_background(background_2);
        for overlay_2 in [&[][..], &overlay[..]] {
            for _ in 0..2 {
                frame_of_the_canvas(&ctx, &mut canvas, document, overlay_2);
            }
            // Durante los fotogramas, el documento, `to_rgba8` y el `state_id` no cambian.
            assert_eq!(doc.state_id(), state_id, "{background_2:?}");
            assert_eq!(to_rgba8(document), bytes_rgba, "{background_2:?}");
            // Y `write_png` da los mismos bytes con cada fondo.
            assert_eq!(
                write_png(document).expect("PNG"),
                png_before,
                "{background_2:?}: mismos bytes del PNG"
            );
        }
        canvas.clear_overlay();
    }
    // La lectura de vuelta es la imagen original, con alfa 0 donde era 0 y sin el color elegido ni
    // los grises del damero.
    let read_2 = read_png(&png_before).expect("el PNG se lee");
    assert!(read_2.is_exact(), "sin conversiones al leer");
    assert_eq!(read_2.image, original);
    let read_2_2 = to_rgba8(&read_2.image);
    assert_eq!(read_2_2, bytes_rgba);
    for (i, pixel) in read_2_2.as_chunks::<4>().0.iter().enumerate() {
        assert!(
            !is_checker_gray(pixel),
            "el píxel {i} es un gris del damero"
        );
        for background_2 in backgrounds {
            if let CanvasBackground::Custom(c) = background_2 {
                assert!(
                    !(pixel[3] == 255 && (pixel[0], pixel[1], pixel[2]) == (c.r, c.g, c.b)),
                    "el píxel {i} del PNG tiene el color del fondo {background_2:?}"
                );
            }
        }
    }
    for y in 0..6usize {
        for x in 0..2usize {
            assert_eq!(
                read_2_2[(y * 8 + x) * 4 + 3],
                0,
                "({x}, {y}) sigue transparente"
            );
        }
    }
    assert_eq!(*document, original);
    assert_eq!(doc.state_id(), state_id);
    assert!(!doc.can_undo());
}

// ---------------------------------------------------------------------------------------------
// Propiedad 1: el formulario acepta exactamente los opacos
// ---------------------------------------------------------------------------------------------

/// Texto de un hexadecimal: de 0 a 10 dígitos, con y sin `#`, con espacios, con algún carácter que no
/// es hexadecimal, o uno de los textos fijos de las filas.
fn text_hexadecimal() -> impl Strategy<Value = String> {
    let fixed_2: Vec<String> = texts_that_are()
        .into_iter()
        .map(|(t, _)| t.to_string())
        .chain(TEXTS_WITH_ALPHA.iter().map(|t| t.to_string()))
        .chain(["zz", "#12", "", " ", "#", "#gggggg", "12 34"].map(String::from))
        .collect();
    let random = (
        any::<bool>(),
        prop::collection::vec(
            prop::sample::select(b"0123456789abcdefABCDEF".to_vec()),
            0..=10,
        ),
        0usize..3,
        0usize..3,
        prop::option::of((
            0usize..10,
            prop::sample::select(vec!['z', 'g', '-', ' ', 'ñ']),
        )),
    )
        .prop_map(|(hash, digits, before, after_2, bad)| {
            let mut body: Vec<char> = digits.into_iter().map(char::from).collect();
            if let Some((i, c)) = bad {
                let i = i % (body.len() + 1);
                body.insert(i, c);
            }
            let mut text_2 = " ".repeat(before);
            if hash {
                text_2.push('#');
            }
            text_2.extend(body);
            text_2.push_str(&" ".repeat(after_2));
            text_2
        });
    prop_oneof![
        3 => random,
        1 => prop::sample::select(fixed_2),
    ]
}

fn color_any() -> impl Strategy<Value = Rgba> {
    (
        any::<u8>(),
        any::<u8>(),
        any::<u8>(),
        prop_oneof![Just(255u8), any::<u8>()],
    )
        .prop_map(|(r, g, b, a)| Rgba::new(r, g, b, a))
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Para cualquier texto (o ninguno) y cualquier último confirmado (alfa incluido): `Accepted`
    /// llega si y solo si `resolve_hex(text_2, Mode::Rgba, confirmed)` da `Ok(c)` con `c.a == 255`, y
    /// entonces `background_from_values` es `Custom(c)`; en los demás casos hay `Rejected`, el
    /// diálogo sigue abierto y no cambia nada.
    #[test]
    fn property_1_the_form_accepts_exactly_the_opaque_ones(
        previous in 0usize..5,
        confirmed_before in color_any(),
        text_2 in prop::option::of(text_hexadecimal()),
    ) {
        let (background_2, _) = PREVIOUS[previous];
        let mut modal = open(background_2);
        // El último confirmado, con su alfa: se escribe entero (8 dígitos) y se confirma.
        writes(
            &mut modal,
            &format!(
                "#{:02x}{:02x}{:02x}{:02x}",
                confirmed_before.r, confirmed_before.g, confirmed_before.b, confirmed_before.a
            ),
        );
        confirms(&mut modal);
        prop_assert_eq!(confirmed(&modal), confirmed_before);
        if let Some(t) = &text_2 {
            writes(&mut modal, t);
        }
        let expected_2 = resolve_hex(text_2.as_deref(), Mode::Rgba, confirmed_before);
        let before = dialog(&modal).clone();
        let reaction = accepts(&mut modal);
        match expected_2 {
            Ok(c) if c.a == 255 => {
                let Reaction::Accepted(values_2) = reaction else {
                    return Err(TestCaseError::fail(format!("se esperaba Accepted y hay {reaction:?}")));
                };
                prop_assert!(!modal.is_open());
                prop_assert_eq!(values_2.color(FIELD_COLOR), Some(c));
                prop_assert_eq!(
                    background_from_values(&values_2),
                    Some(CanvasBackground::Custom(c))
                );
            }
            Ok(_) => {
                // Alfa < 255: el campo vale y el conjunto no.
                prop_assert_eq!(reaction, Reaction::Rejected(OPAQUE_REASON.to_string()));
                prop_assert!(modal.is_open());
                prop_assert_eq!(dialog(&modal), &before);
                prop_assert_eq!(dialog(&modal).validity(FIELD_COLOR), Some(Validity::Valid));
                prop_assert_eq!(dialog(&modal).values(), None);
            }
            Err(error) => {
                // Inválido: el error en línea es el de `hex_notice` y el conjunto no vale.
                prop_assert!(matches!(reaction, Reaction::Rejected(_)), "{:?}", reaction);
                prop_assert!(modal.is_open());
                prop_assert_eq!(dialog(&modal), &before);
                prop_assert_eq!(
                    dialog(&modal).validity(FIELD_COLOR),
                    Some(Validity::Invalid(hex_notice(&error).to_string()))
                );
                prop_assert_eq!(dialog(&modal).values(), None);
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Propiedad 2: el fondo es un modelo trivial «el último aplicado»
// ---------------------------------------------------------------------------------------------

/// Los textos que se teclean en la propiedad 2.
const TYPED: [&str; 9] = [
    "#336699",
    "fff",
    "#ff000080",
    "zz",
    "#12",
    "",
    "#fff8",
    "#ffffffff",
    "336699",
];

/// Un paso de la propiedad 2.
#[derive(Clone, Debug)]
enum TestStep {
    /// Elegir uno de los cinco fondos (`Dispatched::Background` o el que dejó un `Color…` anterior).
    Choose(usize),
    /// Abrir el diálogo con estos bloqueos (los cinco bits).
    Open(u8),
    Type(usize),
    Intro,
    Escape,
    Accept,
    Cancel,
    Quit,
    SetImage,
    NewImage,
    Undo,
    Redo,
    Stroke(u32),
}

fn a_step() -> impl Strategy<Value = TestStep> {
    prop_oneof![
        2 => (0usize..5).prop_map(TestStep::Choose),
        3 => Just(TestStep::Open(0)),
        2 => (1u8..32).prop_map(TestStep::Open),
        3 => (0usize..TYPED.len()).prop_map(TestStep::Type),
        2 => Just(TestStep::Intro),
        2 => Just(TestStep::Escape),
        2 => Just(TestStep::Accept),
        1 => Just(TestStep::Cancel),
        1 => Just(TestStep::Quit),
        1 => Just(TestStep::SetImage),
        1 => Just(TestStep::NewImage),
        1 => Just(TestStep::Undo),
        1 => Just(TestStep::Redo),
        1 => (0u32..48).prop_map(TestStep::Stroke),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// `canvas.background()` es siempre el del modelo («el último fondo aplicado»); cancelar,
    /// `Escape`, `Quit` y los rechazos no lo cambian; `open_background_color` solo da `true` con los
    /// cinco bloqueos en falso y sin otro modal; ningún paso de fondo cambia `state_id`, `modified`,
    /// la historia, la imagen ni la ruta.
    #[test]
    fn property_2_the_background_is_the_last_applied_and_the_background_steps_do_not_touch_the_session(
        steps_2 in prop::collection::vec(a_step(), 0..=32),
    ) {
        let backgrounds: [CanvasBackground; 5] = [
            CanvasBackground::Checker,
            CanvasBackground::Black,
            CanvasBackground::Gray,
            CanvasBackground::White,
            CanvasBackground::Custom(BLUE_DARK),
        ];
        let mut m = World::new_2(CanvasBackground::Checker, false);
        let mut model = CanvasBackground::Checker;
        for (n, step) in steps_2.into_iter().enumerate() {
            let before = m.snapshot();
            let open = m.modal.is_open();
            let mut of_background = true;
            match &step {
                TestStep::Choose(i) => {
                    // Con el diálogo abierto, el menú está bloqueado.
                    if !open {
                        m.canvas.set_background(backgrounds[*i]);
                        model = backgrounds[*i];
                    }
                }
                TestStep::Open(bits) => {
                    let locks = locks_2(*bits);
                    let none = !(locks.gesture || locks.dialog || locks.text_focus
                        || locks.removal || locks.modal);
                    let done = m.opens(locks);
                    prop_assert_eq!(done, none && !open, "paso {}: {:?}", n, step);
                    if done {
                        prop_assert_eq!(m.modal.kind(), Some(DialogKind::BackgroundColor));
                        prop_assert_eq!(confirmed(&m.modal), initial_color(model));
                    }
                }
                TestStep::Type(i) => {
                    if open {
                        m.writes(TYPED[*i]);
                    }
                }
                TestStep::Intro => {
                    if open {
                        let r = m.presses(DialogKey::Enter);
                        if let Reaction::Accepted(values_2) = &r {
                            model = background_from_values(values_2)
                                .expect("un Accepted siempre trae un fondo opaco");
                        }
                    }
                }
                TestStep::Escape => {
                    if open {
                        let r = m.presses(DialogKey::Escape);
                        prop_assert!(!matches!(r, Reaction::Accepted(_)), "Escape no acepta");
                    }
                }
                TestStep::Accept => {
                    if open {
                        match m.event_2(Event::Accept) {
                            Reaction::Accepted(values_2) => {
                                model = background_from_values(&values_2)
                                    .expect("un Accepted siempre trae un fondo opaco");
                            }
                            Reaction::Rejected(_) => prop_assert!(m.modal.is_open()),
                            other => prop_assert!(false, "Accept dio {:?}", other),
                        }
                    }
                }
                TestStep::Cancel => {
                    if open {
                        prop_assert_eq!(m.event_2(Event::Cancel), Reaction::Cancelled);
                    }
                }
                TestStep::Quit => {
                    prop_assert_eq!(m.modal.abandon(), open);
                }
                TestStep::SetImage => {
                    if !open {
                        let image_2 = m.image_2();
                        m.canvas.set_image(&image_2);
                        of_background = false;
                    }
                }
                TestStep::NewImage => {
                    if !open {
                        let image_2 = Image::new(5, 4, Mode::Rgba).expect("imagen");
                        m.session.new_image(image_2.clone(), true).expect("nueva");
                        m.canvas.set_owned_image(image_2);
                        of_background = false;
                    }
                }
                TestStep::Undo => {
                    if !open {
                        let _ = m.session.undo();
                        of_background = false;
                    }
                }
                TestStep::Redo => {
                    if !open {
                        let _ = m.session.redo();
                        of_background = false;
                    }
                }
                TestStep::Stroke(i) => {
                    if !open {
                        let id = ImageId::from_raw(0);
                        let (width, height) = {
                            let image_2 = m.image_2();
                            (image_2.width(), image_2.height())
                        };
                        let _ = m.session.document.apply(Command::SetPixels {
                            image: id,
                            pixels: vec![PixelEdit {
                                x: i % width,
                                y: i / width % height,
                                pixel: Pixel::Rgba(Rgba::opaque(9, 9, (i % 200) as u8)),
                            }],
                        });
                        of_background = false;
                    }
                }
            }
            // El lienzo dice siempre lo mismo que el modelo.
            prop_assert_eq!(m.canvas.background(), model, "paso {}: {:?}", n, step);
            // Si hay un diálogo, es el de color y su estado es coherente.
            if let Some(kind) = m.modal.kind() {
                prop_assert_eq!(kind, DialogKind::BackgroundColor);
            }
            // Un paso de fondo no toca la sesión.
            if of_background {
                prop_assert_eq!(m.snapshot(), before, "paso {}: {:?}", n, step);
            }
        }
    }
}
