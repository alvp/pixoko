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
//!   del formulario. El resto de textos fijados son el título `Fondo del lienzo` y los botones
//!   `Aplicar` y `Cancelar`.
//! - «`Intro` en el campo» es lo que hace `dialog_view` con `key_action` (`Enter` con el foco en el
//!   campo): `ConfirmHex` con texto pendiente y `Accept` sin él; `Escape`, `RevertHex` con texto
//!   pendiente y `Cancel` sin él.
//! - `initial_color` de un `Custom` construido a mano con alfa < 255 no está fijado y no se prueba.
//! - `Dialog: Clone + PartialEq` (derives de `px-ui`) sirve para comprobar que un rechazo no cambia
//!   nada.
//! - Lo que `App` hace al aceptar (D8) se escribe aquí como `Mundo::evento`: `Accepted` →
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

const GRIS_POR_DEFECTO: Rgba = Rgba::opaque(0x80, 0x80, 0x80);
const AZUL_ACERO: Rgba = Rgba::opaque(0x33, 0x66, 0x99);
const AZUL_OSCURO: Rgba = Rgba::opaque(0x11, 0x22, 0x33);
const BLANCO: Rgba = Rgba::opaque(0xff, 0xff, 0xff);
const NEGRO: Rgba = Rgba::opaque(0, 0, 0);
const ROJO: Rgba = Rgba::opaque(0xff, 0, 0);

/// Los cinco fondos previos y el color con el que debe nacer el diálogo desde cada uno (el del
/// sólido, `#808080` con el damero).
const PREVIOS: [(CanvasBackground, Rgba); 5] = [
    (CanvasBackground::Checker, GRIS_POR_DEFECTO),
    (CanvasBackground::Black, NEGRO),
    (CanvasBackground::Gray, Rgba::opaque(40, 40, 40)),
    (CanvasBackground::White, BLANCO),
    (CanvasBackground::Custom(AZUL_OSCURO), AZUL_OSCURO),
];

/// Cada bloqueo, solo.
fn bloqueos_de_uno_en_uno() -> [(&'static str, Locks); 5] {
    let con = |f: fn(&mut Locks)| {
        let mut locks = Locks::default();
        f(&mut locks);
        locks
    };
    [
        ("gesture", con(|l| l.gesture = true)),
        ("dialog", con(|l| l.dialog = true)),
        ("text_focus", con(|l| l.text_focus = true)),
        ("removal", con(|l| l.removal = true)),
        ("modal", con(|l| l.modal = true)),
    ]
}

fn bloqueos(bits: u8) -> Locks {
    Locks {
        gesture: bits & 1 != 0,
        stroke: false,
        dialog: bits & 2 != 0,
        text_focus: bits & 4 != 0,
        removal: bits & 8 != 0,
        modal: bits & 16 != 0,
    }
}

fn en_el_juego(texto: &str) -> bool {
    texto.chars().all(|c| INTERFACE_CHARSET.contains(c))
}

/// El diálogo recién abierto desde `previo`.
fn abierto(previo: CanvasBackground) -> Modal {
    let mut modal = Modal::default();
    assert!(
        open_background_color(&mut modal, previo, Locks::default()),
        "{previo:?}: el diálogo se abre"
    );
    modal
}

fn dialogo(modal: &Modal) -> &Dialog {
    match modal {
        Modal::Open(d) => d,
        Modal::Closed => panic!("el modal está cerrado"),
    }
}

fn escribe(modal: &mut Modal, texto: &str) {
    assert_eq!(
        modal.event(Event::Hex {
            field: FIELD_COLOR,
            text: texto.to_string()
        }),
        Reaction::None,
        "escribir {texto:?}"
    );
}

/// `Intro` sobre un texto pendiente: confirma el campo.
fn confirma(modal: &mut Modal) {
    assert_eq!(
        modal.event(Event::HexConfirm {
            field: FIELD_COLOR,
            blur: false
        }),
        Reaction::None
    );
}

fn pierde_el_foco(modal: &mut Modal) {
    assert_eq!(
        modal.event(Event::HexConfirm {
            field: FIELD_COLOR,
            blur: true
        }),
        Reaction::None
    );
}

fn acepta(modal: &mut Modal) -> Reaction {
    modal.event(Event::Accept)
}

fn texto_del_campo(modal: &Modal) -> String {
    dialogo(modal).text(FIELD_COLOR).expect("el campo existe")
}

fn confirmado(modal: &Modal) -> Rgba {
    dialogo(modal)
        .hex_committed(FIELD_COLOR)
        .expect("el campo es hexadecimal")
}

fn motivo(modal: &Modal) -> Option<String> {
    dialogo(modal).reason()
}

const EN_EL_CAMPO: Option<FocusTarget> = Some(FocusTarget::Field(FIELD_COLOR));

/// Una tecla con el foco en `foco`: lo que decide `key_action` y el evento que `dialog_view` le
/// aplica al diálogo.
fn pulsa(modal: &mut Modal, foco: Option<FocusTarget>, tecla: DialogKey) -> Reaction {
    let accion = key_action(dialogo(modal), foco, tecla, false);
    match accion {
        KeyAction::Accept => modal.event(Event::Accept),
        KeyAction::Cancel => modal.event(Event::Cancel),
        KeyAction::ConfirmHex(field) => modal.event(Event::HexConfirm { field, blur: false }),
        KeyAction::RevertHex(field) => modal.event(Event::HexRevert { field }),
        KeyAction::Nothing => Reaction::None,
    }
}

/// El color que debe salir de un texto que vale (con alfa `ff`).
fn opaco(r: u8, g: u8, b: u8) -> Rgba {
    Rgba::opaque(r, g, b)
}

/// Textos que valen desde un diálogo recién abierto (el confirmado es opaco): con y sin `#`, de 3,
/// 4, 6 y 8 dígitos, con alfa `ff`.
fn textos_que_valen() -> Vec<(&'static str, Rgba)> {
    vec![
        ("fff", BLANCO),
        ("#fff", BLANCO),
        ("ffffff", BLANCO),
        ("#ffffff", BLANCO),
        ("ffff", BLANCO),
        ("#ffff", BLANCO),
        ("#ff0000ff", ROJO),
        ("ff0000ff", ROJO),
        ("f00", ROJO),
        ("#f00", ROJO),
        ("#f00f", ROJO),
        ("336699", AZUL_ACERO),
        ("#336699", AZUL_ACERO),
        ("#336699FF", AZUL_ACERO),
        ("  #336699  ", AZUL_ACERO),
        ("#abc", opaco(0xaa, 0xbb, 0xcc)),
    ]
}

/// Textos de 4 u 8 dígitos con alfa < `ff`: el campo es válido y el conjunto no.
const TEXTOS_CON_ALFA: [&str; 12] = [
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
fn fila_4_las_constantes_son_las_de_piezas() {
    assert_eq!(FIELD_COLOR, "color");
    assert_eq!(OPAQUE_REASON, "el fondo debe ser opaco (alfa ff)");
    assert_eq!(OPAQUE_REASON.chars().count(), 33);
    assert!(OPAQUE_REASON.chars().count() <= REASON_MAX_CHARS);
    assert_eq!(REASON_MAX_CHARS, 37);
    assert!(en_el_juego(OPAQUE_REASON));
    // Con el prefijo `error: ` cabe en las 44 columnas del pie y no se recorta.
    assert!(ERROR_PREFIX.chars().count() + OPAQUE_REASON.chars().count() <= FOOTER_COLUMNS);
}

#[test]
fn fila_4_background_color_form_valida_y_lleva_titulo_botones_un_hexadecimal_rgba_y_un_check() {
    for color in [
        GRIS_POR_DEFECTO,
        NEGRO,
        BLANCO,
        AZUL_ACERO,
        Rgba::opaque(40, 40, 40),
    ] {
        let spec: FormSpec = background_color_form(color);
        assert_eq!(spec.validate(), Ok(()), "{color:?}");
        assert_eq!(spec.title, "Fondo del lienzo");
        assert_eq!(spec.accept, "Aplicar");
        assert_eq!(spec.cancel, Some("Cancelar"));
        assert_eq!(spec.fields.len(), 1, "un solo campo");
        let campo = &spec.fields[0];
        assert_eq!(campo.id, FIELD_COLOR);
        assert!(campo.shown_when.is_none());
        assert_eq!(
            campo.kind,
            FieldKind::Hex {
                mode: Mode::Rgba,
                default: color
            },
            "hexadecimal en modo RGBA con el color inicial por defecto"
        );
        assert!(campo.label.chars().count() <= 12 && !campo.label.is_empty());
        assert!(
            spec.check.is_some(),
            "el check de opacidad es lo que rechaza el alfa"
        );
        // El formulario se puede abrir (`Dialog::new` valida) como el diálogo de color.
        assert!(Dialog::new(DialogKind::BackgroundColor, spec).is_ok());
    }
}

#[test]
fn fila_4_initial_color_es_el_del_solido_el_gris_80_con_el_damero_y_el_del_custom() {
    for (previo, esperado) in PREVIOS {
        assert_eq!(initial_color(previo), esperado, "{previo:?}");
        assert_eq!(initial_color(previo).a, 255, "{previo:?}: siempre opaco");
    }
    // Otros colores propios, con canales distintos.
    for c in [
        AZUL_ACERO,
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
fn fila_4_open_background_color_abre_el_dialogo_de_color_con_el_color_del_fondo_actual() {
    for (previo, color) in PREVIOS {
        let modal = abierto(previo);
        assert_eq!(modal.kind(), Some(DialogKind::BackgroundColor));
        let d = dialogo(&modal);
        assert_eq!(d.kind(), DialogKind::BackgroundColor);
        assert_eq!(d.spec().title, "Fondo del lienzo");
        // El defecto es válido y `Aplicar` está habilitado.
        assert_eq!(d.validity(FIELD_COLOR), Some(Validity::Valid), "{previo:?}");
        assert!(d.is_valid(), "{previo:?}");
        assert_eq!(d.reason(), None, "{previo:?}");
        assert_eq!(d.text(FIELD_COLOR), Some(color.to_string()), "{previo:?}");
        assert_eq!(d.hex_committed(FIELD_COLOR), Some(color), "{previo:?}");
        assert!(!d.hex_pending(FIELD_COLOR));
        let valores = d.values().expect("el defecto vale");
        assert_eq!(valores.color(FIELD_COLOR), Some(color));
        assert_eq!(
            background_from_values(&valores),
            Some(CanvasBackground::Custom(color)),
            "{previo:?}: aceptar sin tocar nada da el color de partida"
        );
    }
}

#[test]
fn fila_4_el_defecto_se_acepta_tal_cual_y_da_custom_con_alfa_ff() {
    for (previo, color) in PREVIOS {
        let mut modal = abierto(previo);
        let Reaction::Accepted(valores) = acepta(&mut modal) else {
            panic!("{previo:?}: el defecto se acepta");
        };
        assert!(!modal.is_open());
        assert_eq!(valores.color(FIELD_COLOR), Some(color));
        assert_eq!(
            background_from_values(&valores),
            Some(CanvasBackground::Custom(color))
        );
    }
}

#[test]
fn fila_4_con_un_bloqueo_o_con_otro_dialogo_open_background_color_da_false_y_no_cambia_nada() {
    for (previo, _) in PREVIOS {
        for (nombre, locks) in bloqueos_de_uno_en_uno() {
            let mut modal = Modal::default();
            assert!(
                !open_background_color(&mut modal, previo, locks),
                "{previo:?}, {nombre}"
            );
            assert_eq!(modal, Modal::Closed, "{previo:?}, {nombre}");
        }
        // Con otro modal abierto, tampoco, y el abierto no cambia.
        let mut modal = abierto(CanvasBackground::Black);
        let antes = modal.clone();
        assert!(!open_background_color(&mut modal, previo, Locks::default()));
        assert_eq!(modal, antes);
    }
}

#[test]
fn fila_4_el_foco_inicial_es_el_campo_y_el_orden_de_tabulacion_campo_aplicar_cancelar() {
    for (previo, _) in PREVIOS {
        let mut modal = abierto(previo);
        let d = dialogo(&modal);
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
        // no: solo el campo y `Cancelar`.
        for texto in ["#fff8", "zz", ""] {
            escribe(&mut modal, texto);
            assert_eq!(
                focus_order(dialogo(&modal)),
                vec![FocusTarget::Field(FIELD_COLOR), FocusTarget::Cancel],
                "{previo:?}: {texto:?}"
            );
        }
        escribe(&mut modal, "#fff");
        assert_eq!(
            focus_order(dialogo(&modal)),
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
fn fila_4_las_cuatro_longitudes_con_y_sin_almohadilla_valen_con_alfa_ff_desde_cada_fondo_previo() {
    for (previo, _) in PREVIOS {
        for (texto, color) in textos_que_valen() {
            let contexto = format!("{previo:?}: {texto:?}");
            let mut modal = abierto(previo);
            escribe(&mut modal, texto);
            let d = dialogo(&modal);
            assert_eq!(d.validity(FIELD_COLOR), Some(Validity::Valid), "{contexto}");
            assert_eq!(d.reason(), None, "{contexto}");
            assert!(d.is_valid(), "{contexto}");
            assert_eq!(
                d.values().and_then(|v| v.color(FIELD_COLOR)),
                Some(color),
                "{contexto}"
            );
            assert_eq!(
                focus_order(d),
                vec![
                    FocusTarget::Field(FIELD_COLOR),
                    FocusTarget::Accept,
                    FocusTarget::Cancel
                ],
                "{contexto}: `Aplicar` habilitado"
            );
            // Aceptar con el texto sin confirmar lo confirma y da el color.
            let Reaction::Accepted(valores) = acepta(&mut modal) else {
                panic!("{contexto}: se acepta");
            };
            assert!(!modal.is_open(), "{contexto}");
            assert_eq!(
                background_from_values(&valores),
                Some(CanvasBackground::Custom(color)),
                "{contexto}"
            );
        }
    }
}

#[test]
fn fila_4_con_alfa_menor_que_ff_el_campo_vale_pero_se_rechaza_con_el_motivo_y_el_dialogo_sigue() {
    for (previo, _) in PREVIOS {
        for texto in TEXTOS_CON_ALFA {
            let contexto = format!("{previo:?}: {texto:?}");
            let mut modal = abierto(previo);
            escribe(&mut modal, texto);
            // El campo es válido (no hay error en línea); el motivo es el del conjunto.
            let d = dialogo(&modal);
            assert_eq!(d.validity(FIELD_COLOR), Some(Validity::Valid), "{contexto}");
            assert_eq!(d.reason(), Some(OPAQUE_REASON.to_string()), "{contexto}");
            assert!(!d.is_valid(), "{contexto}");
            assert_eq!(d.values(), None, "{contexto}");
            assert_eq!(
                focus_order(d),
                vec![FocusTarget::Field(FIELD_COLOR), FocusTarget::Cancel],
                "{contexto}: `Aplicar` atenuado"
            );
            // `Accept` da `Rejected` con el motivo y no cambia nada.
            let antes = dialogo(&modal).clone();
            assert_eq!(
                acepta(&mut modal),
                Reaction::Rejected(OPAQUE_REASON.to_string()),
                "{contexto}"
            );
            assert!(modal.is_open(), "{contexto}: el diálogo sigue abierto");
            assert_eq!(dialogo(&modal), &antes, "{contexto}: no cambia nada");
            // El mismo resultado tras `Intro` (que solo confirma el campo).
            confirma(&mut modal);
            assert_eq!(
                motivo(&modal),
                Some(OPAQUE_REASON.to_string()),
                "{contexto}"
            );
            assert_eq!(dialogo(&modal).values(), None, "{contexto}");
            assert_eq!(
                acepta(&mut modal),
                Reaction::Rejected(OPAQUE_REASON.to_string()),
                "{contexto}: tras Intro"
            );
            assert!(modal.is_open(), "{contexto}");
        }
    }
}

#[test]
fn fila_4_el_motivo_de_opacidad_es_el_del_pie_con_su_prefijo_y_cabe_sin_recortar() {
    let mut modal = abierto(CanvasBackground::Checker);
    escribe(&mut modal, "#ff000080");
    let motivo = motivo(&modal).expect("motivo");
    assert_eq!(motivo, OPAQUE_REASON, "no se recorta");
    assert!(motivo.chars().count() <= REASON_MAX_CHARS);
    let pie = format!("{ERROR_PREFIX}{motivo}");
    assert!(pie.chars().count() <= FOOTER_COLUMNS, "{pie:?}");
    assert!(en_el_juego(&pie));
}

#[test]
fn fila_4_zz_y_12_dan_el_error_en_linea_de_hex_notice_y_no_el_de_opacidad() {
    let casos: [(&str, HexError); 7] = [
        ("zz", HexError::Length(2)),
        ("#12", HexError::Length(2)),
        ("", HexError::Empty),
        ("   ", HexError::Empty),
        ("#", HexError::Empty),
        ("#12345", HexError::Length(5)),
        ("#gggggg", HexError::Digit),
    ];
    for (previo, _) in PREVIOS {
        for (texto, error) in &casos {
            let contexto = format!("{previo:?}: {texto:?}");
            let mut modal = abierto(previo);
            escribe(&mut modal, texto);
            let d = dialogo(&modal);
            let aviso = hex_notice(error).to_string();
            assert_eq!(
                d.validity(FIELD_COLOR),
                Some(Validity::Invalid(aviso.clone())),
                "{contexto}: el error en línea es el de `hex_notice`"
            );
            // El motivo del pie es el del campo (con su rótulo), no el de opacidad.
            let etiqueta = d.spec().fields[0].label;
            let esperado: String = format!("{etiqueta}: {aviso}")
                .chars()
                .take(REASON_MAX_CHARS)
                .collect();
            assert_eq!(d.reason(), Some(esperado.clone()), "{contexto}");
            assert_ne!(d.reason(), Some(OPAQUE_REASON.to_string()), "{contexto}");
            assert_eq!(d.values(), None, "{contexto}");
            assert_eq!(
                focus_order(d),
                vec![FocusTarget::Field(FIELD_COLOR), FocusTarget::Cancel],
                "{contexto}"
            );
            assert_eq!(
                acepta(&mut modal),
                Reaction::Rejected(esperado),
                "{contexto}"
            );
            assert!(modal.is_open(), "{contexto}");
        }
    }
}

#[test]
fn fila_4_intro_confirma_un_texto_valido_y_conserva_el_invalido_con_su_error() {
    for (previo, _) in PREVIOS {
        // Un texto válido: `Intro` lo confirma, el campo lo muestra canónico y `Intro` otra vez acepta.
        let mut modal = abierto(previo);
        escribe(&mut modal, "336699");
        assert!(dialogo(&modal).hex_pending(FIELD_COLOR));
        assert_eq!(
            key_action(dialogo(&modal), EN_EL_CAMPO, DialogKey::Enter, false),
            KeyAction::ConfirmHex(FIELD_COLOR)
        );
        assert_eq!(
            pulsa(&mut modal, EN_EL_CAMPO, DialogKey::Enter),
            Reaction::None
        );
        assert!(!dialogo(&modal).hex_pending(FIELD_COLOR));
        assert_eq!(confirmado(&modal), AZUL_ACERO);
        assert_eq!(texto_del_campo(&modal), "#336699");
        assert_eq!(
            key_action(dialogo(&modal), EN_EL_CAMPO, DialogKey::Enter, false),
            KeyAction::Accept
        );
        let Reaction::Accepted(valores) = pulsa(&mut modal, EN_EL_CAMPO, DialogKey::Enter) else {
            panic!("{previo:?}: el segundo Intro acepta");
        };
        assert_eq!(
            background_from_values(&valores),
            Some(CanvasBackground::Custom(AZUL_ACERO))
        );

        // Un texto inválido: `Intro` no lo descarta ni confirma, y el error sigue.
        let mut modal = abierto(previo);
        let antes = confirmado(&modal);
        escribe(&mut modal, "zz");
        assert_eq!(
            pulsa(&mut modal, EN_EL_CAMPO, DialogKey::Enter),
            Reaction::None
        );
        assert!(dialogo(&modal).hex_pending(FIELD_COLOR));
        assert_eq!(texto_del_campo(&modal), "zz");
        assert_eq!(confirmado(&modal), antes);
        assert!(matches!(
            dialogo(&modal).validity(FIELD_COLOR),
            Some(Validity::Invalid(_))
        ));
    }
}

#[test]
fn fila_4_la_perdida_de_foco_confirma_un_texto_valido_y_revierte_el_invalido() {
    for (previo, color) in PREVIOS {
        // Válida: confirma.
        let mut modal = abierto(previo);
        escribe(&mut modal, "#ff8000");
        pierde_el_foco(&mut modal);
        assert!(!dialogo(&modal).hex_pending(FIELD_COLOR));
        assert_eq!(confirmado(&modal), Rgba::opaque(0xff, 0x80, 0));
        assert_eq!(texto_del_campo(&modal), "#ff8000");

        // Inválida: revierte al último confirmado y el diálogo sigue válido.
        let mut modal = abierto(previo);
        escribe(&mut modal, "zz");
        pierde_el_foco(&mut modal);
        assert!(!dialogo(&modal).hex_pending(FIELD_COLOR), "{previo:?}");
        assert_eq!(texto_del_campo(&modal), color.to_string(), "{previo:?}");
        assert_eq!(confirmado(&modal), color);
        assert!(dialogo(&modal).is_valid());

        // Con alfa: es válida y se confirma (y el motivo sigue visible).
        let mut modal = abierto(previo);
        escribe(&mut modal, "#ff000080");
        pierde_el_foco(&mut modal);
        assert_eq!(confirmado(&modal), Rgba::new(0xff, 0, 0, 0x80));
        assert_eq!(motivo(&modal), Some(OPAQUE_REASON.to_string()));
    }
}

#[test]
fn fila_4_escape_con_texto_pendiente_invalido_lo_revierte_y_el_siguiente_escape_cancela() {
    for (previo, color) in PREVIOS {
        let mut modal = abierto(previo);
        escribe(&mut modal, "zz");
        assert_eq!(
            key_action(dialogo(&modal), EN_EL_CAMPO, DialogKey::Escape, false),
            KeyAction::RevertHex(FIELD_COLOR)
        );
        // El primer `Escape` revierte y el diálogo sigue abierto.
        assert_eq!(
            pulsa(&mut modal, EN_EL_CAMPO, DialogKey::Escape),
            Reaction::None
        );
        assert!(modal.is_open(), "{previo:?}");
        assert!(!dialogo(&modal).hex_pending(FIELD_COLOR));
        assert_eq!(texto_del_campo(&modal), color.to_string());
        assert!(dialogo(&modal).is_valid());
        // El siguiente cancela.
        assert_eq!(
            key_action(dialogo(&modal), EN_EL_CAMPO, DialogKey::Escape, false),
            KeyAction::Cancel
        );
        assert_eq!(
            pulsa(&mut modal, EN_EL_CAMPO, DialogKey::Escape),
            Reaction::Cancelled
        );
        assert!(!modal.is_open());
    }
}

#[test]
fn fila_4_escape_con_texto_pendiente_valido_o_con_alfa_tambien_lo_revierte_antes_de_cancelar() {
    for texto in ["#336699", "#ff000080", "fff"] {
        let mut modal = abierto(CanvasBackground::Black);
        escribe(&mut modal, texto);
        assert_eq!(
            pulsa(&mut modal, EN_EL_CAMPO, DialogKey::Escape),
            Reaction::None
        );
        assert!(modal.is_open(), "{texto:?}");
        assert!(!dialogo(&modal).hex_pending(FIELD_COLOR));
        assert_eq!(texto_del_campo(&modal), "#000000");
        assert!(
            dialogo(&modal).is_valid(),
            "el motivo desaparece con el texto"
        );
        assert_eq!(
            pulsa(&mut modal, EN_EL_CAMPO, DialogKey::Escape),
            Reaction::Cancelled
        );
    }
    // Sin texto pendiente, `Escape` cancela a la primera.
    let mut modal = abierto(CanvasBackground::White);
    assert_eq!(
        pulsa(&mut modal, EN_EL_CAMPO, DialogKey::Escape),
        Reaction::Cancelled
    );
}

#[test]
fn fila_4_la_secuencia_del_alfa_confirmado_se_arrastra_hasta_escribir_ocho_digitos_con_ff() {
    for (previo, _) in PREVIOS {
        let contexto = |paso: &str| format!("{previo:?}: {paso}");
        let mut modal = abierto(previo);
        // 1. Escribir `#ff000080`: el campo es válido y el conjunto no.
        escribe(&mut modal, "#ff000080");
        assert_eq!(
            motivo(&modal),
            Some(OPAQUE_REASON.to_string()),
            "{}",
            contexto("#ff000080 sin confirmar")
        );
        // 2. `Intro`: el campo confirma ese valor y el motivo queda visible.
        confirma(&mut modal);
        assert_eq!(
            texto_del_campo(&modal),
            "#ff000080",
            "{}",
            contexto("tras Intro")
        );
        assert_eq!(
            confirmado(&modal),
            Rgba::new(0xff, 0, 0, 0x80),
            "{}",
            contexto("el confirmado lleva alfa 80")
        );
        assert!(!dialogo(&modal).hex_pending(FIELD_COLOR));
        assert_eq!(motivo(&modal), Some(OPAQUE_REASON.to_string()));
        assert_eq!(
            acepta(&mut modal),
            Reaction::Rejected(OPAQUE_REASON.to_string()),
            "{}",
            contexto("Aplicar tras #ff000080")
        );
        assert!(modal.is_open());

        // 3. `#fff` y `#ffffff`, cada una con `Intro`: el alfa que se toma es el `80` confirmado.
        for texto in ["#fff", "#ffffff", "fff", "ffffff"] {
            escribe(&mut modal, texto);
            assert_eq!(
                motivo(&modal),
                Some(OPAQUE_REASON.to_string()),
                "{}",
                contexto(&format!("{texto} sin confirmar"))
            );
            assert_eq!(dialogo(&modal).values(), None);
            confirma(&mut modal);
            assert_eq!(
                texto_del_campo(&modal),
                "#ffffff80",
                "{}",
                contexto(&format!("{texto} + Intro"))
            );
            assert_eq!(confirmado(&modal), Rgba::new(0xff, 0xff, 0xff, 0x80));
            assert_eq!(motivo(&modal), Some(OPAQUE_REASON.to_string()));
            assert_eq!(
                acepta(&mut modal),
                Reaction::Rejected(OPAQUE_REASON.to_string()),
                "{}",
                contexto(&format!("Aplicar tras {texto}"))
            );
            assert!(modal.is_open());
        }

        // 4. `#ffffffff` vale (alfa `ff`): `Aplicar` acepta y da `Custom(#ffffff)`.
        escribe(&mut modal, "#ffffffff");
        assert_eq!(motivo(&modal), None, "{}", contexto("#ffffffff"));
        assert_eq!(
            focus_order(dialogo(&modal)),
            vec![
                FocusTarget::Field(FIELD_COLOR),
                FocusTarget::Accept,
                FocusTarget::Cancel
            ]
        );
        let Reaction::Accepted(valores) = acepta(&mut modal) else {
            panic!("{}", contexto("#ffffffff se acepta"));
        };
        assert!(!modal.is_open());
        assert_eq!(
            background_from_values(&valores),
            Some(CanvasBackground::Custom(BLANCO)),
            "{}",
            contexto("Custom(#ffffff)")
        );
    }
}

#[test]
fn fila_4_confirmar_ocho_digitos_con_ff_devuelve_al_alfa_opaco_para_los_de_tres_y_seis() {
    // La salida de la secuencia sin cerrar el diálogo: confirmar `#ffffffff` y seguir con `#fff`.
    let mut modal = abierto(CanvasBackground::Checker);
    escribe(&mut modal, "#ff000080");
    confirma(&mut modal);
    assert!(!dialogo(&modal).is_valid());
    escribe(&mut modal, "#336699ff");
    confirma(&mut modal);
    assert_eq!(confirmado(&modal), AZUL_ACERO);
    assert_eq!(motivo(&modal), None);
    escribe(&mut modal, "#fff");
    assert_eq!(motivo(&modal), None, "con el confirmado opaco, `#fff` vale");
    confirma(&mut modal);
    assert_eq!(texto_del_campo(&modal), "#ffffff");
}

#[test]
fn fila_4_cancelar_y_reabrir_nace_con_alfa_ff_aunque_antes_hubiera_alfa_confirmado() {
    for (previo, color) in PREVIOS {
        let mut modal = abierto(previo);
        escribe(&mut modal, "#ff000080");
        confirma(&mut modal);
        assert_eq!(confirmado(&modal).a, 0x80);
        // `Cancelar` cierra sin respuesta de aceptación.
        assert_eq!(modal.event(Event::Cancel), Reaction::Cancelled);
        assert!(!modal.is_open());
        // Reabrir: el confirmado vuelve a ser el color del fondo, opaco; `#fff` vale.
        let mut modal = abierto(previo);
        assert_eq!(confirmado(&modal), color);
        assert_eq!(confirmado(&modal).a, 255);
        assert_eq!(motivo(&modal), None);
        escribe(&mut modal, "#fff");
        assert_eq!(motivo(&modal), None, "{previo:?}: `#fff` vale al reabrir");
        // Con `Escape` (sin texto pendiente, tras confirmar) pasa lo mismo.
        let mut modal = abierto(previo);
        escribe(&mut modal, "#ff000080");
        confirma(&mut modal);
        assert_eq!(
            pulsa(&mut modal, EN_EL_CAMPO, DialogKey::Escape),
            Reaction::Cancelled
        );
        let modal = abierto(previo);
        assert_eq!(confirmado(&modal), color);
    }
}

#[test]
fn fila_4_background_from_values_da_custom_con_el_color_opaco_de_los_valores() {
    for (texto, color) in textos_que_valen() {
        let mut modal = abierto(CanvasBackground::Checker);
        escribe(&mut modal, texto);
        let valores = dialogo(&modal).values().expect("valen");
        let fondo = background_from_values(&valores).expect("un fondo");
        assert_eq!(fondo, CanvasBackground::Custom(color), "{texto:?}");
        let CanvasBackground::Custom(c) = fondo else {
            unreachable!()
        };
        assert_eq!(c.a, 255, "{texto:?}: el color del fondo es opaco");
        // El valor que sale en el menú es el mismo color.
        assert_eq!(
            background_value_text(fondo),
            Some(format!("#{:02x}{:02x}{:02x}", color.r, color.g, color.b))
        );
    }
}

// --- Textos y contraste ---

#[test]
fn fila_4_todo_texto_fijo_y_todo_motivo_caben_en_37_caracteres_y_estan_en_el_juego() {
    for (previo, color) in PREVIOS {
        let spec = background_color_form(initial_color(previo));
        for texto in [spec.title, spec.accept, spec.cancel.expect("hay Cancelar")] {
            assert!(en_el_juego(texto), "{texto:?}");
            assert!(texto.chars().count() <= 44, "{texto:?}");
        }
        for campo in &spec.fields {
            assert!(en_el_juego(campo.label), "{:?}", campo.label);
        }
        assert!(en_el_juego(&color.to_string()));
        // Todos los motivos que puede dar el diálogo: opacidad y los errores del campo.
        let mut modal = abierto(previo);
        for texto in [
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
            escribe(&mut modal, texto);
            if let Some(motivo) = motivo(&modal) {
                assert!(en_el_juego(&motivo), "{motivo:?}");
                assert!(
                    motivo.chars().count() <= REASON_MAX_CHARS,
                    "{motivo:?} mide {}",
                    motivo.chars().count()
                );
                let pie = format!("{ERROR_PREFIX}{motivo}");
                assert!(pie.chars().count() <= FOOTER_COLUMNS, "{pie:?}");
            }
            if let Some(Validity::Invalid(aviso)) = dialogo(&modal).validity(FIELD_COLOR) {
                assert!(en_el_juego(&aviso), "{aviso:?}");
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
        assert!(en_el_juego(hex_notice(&error)));
    }
}

/// Un texto pintado, en orden de pintado.
#[derive(Clone, Debug)]
struct TextoPintado {
    texto: String,
    rect: Rect,
    clip: Rect,
    colores: Vec<Color32>,
    opacidad: f32,
    /// El color del primer vértice de glifo: lo que sale por pantalla (con el atenuado aplicado).
    glifo: Option<Color32>,
}

#[derive(Clone, Debug)]
enum Pintado {
    Texto(TextoPintado),
    Relleno {
        rect: Rect,
        fill: Color32,
        clip: Rect,
    },
}

fn aplana(shape: &Shape, clip: Rect, fuera: &mut Vec<Pintado>) {
    match shape {
        Shape::Text(t) => {
            let job = &t.galley.job;
            let colores = job
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
            let glifo = t.galley.rows.iter().find_map(|fila| {
                let visuales = &fila.row.visuals;
                visuales.mesh.vertices[visuales.glyph_vertex_range.clone()]
                    .first()
                    .map(|v| v.color)
            });
            fuera.push(Pintado::Texto(TextoPintado {
                texto: t.galley.text().to_owned(),
                rect: Rect::from_min_size(t.pos, t.galley.size()),
                clip,
                colores,
                opacidad: t.opacity_factor,
                glifo,
            }));
        }
        Shape::Rect(r) => fuera.push(Pintado::Relleno {
            rect: r.rect,
            fill: r.fill,
            clip,
        }),
        Shape::Vec(formas) => {
            for forma in formas {
                aplana(forma, clip, fuera);
            }
        }
        _ => {}
    }
}

fn contexto_de(ppp: f32) -> Context {
    let ctx = Context::default();
    ctx.set_pixels_per_point(ppp);
    ctx.set_missing_glyph_policy(MissingGlyphPolicy::Panic);
    install_fonts(&ctx);
    ctx.set_visuals_of(eframe::egui::Theme::Dark, visuals(ppp));
    for _ in 0..2 {
        let entrada = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(100.0, 100.0))),
            ..RawInput::default()
        };
        let salida = ctx.run_ui(entrada, |ui| apply_scale(ui.ctx()));
        salida.drop_without_applying_deltas();
    }
    ctx
}

/// `src` (premultiplicado, como egui) por encima de `dst` opaco.
fn encima(dst: Color32, src: Color32) -> Color32 {
    let k = 1.0 - f32::from(src.a()) / 255.0;
    let canal = |s: u8, d: u8| (f32::from(s) + f32::from(d) * k).round().clamp(0.0, 255.0) as u8;
    Color32::from_rgb(
        canal(src.r(), dst.r()),
        canal(src.g(), dst.g()),
        canal(src.b(), dst.b()),
    )
}

/// Pinta `dialog` unos fotogramas (con el reloj avanzando, para que acabe cualquier animación) a la
/// escala `ppp` y una ventana de 960 × 720 puntos, y devuelve lo pintado del último, en orden.
fn pinta_el_dialogo(ppp: f32, dialog: &mut Dialog) -> Vec<Pintado> {
    let ctx = contexto_de(ppp);
    let mut foco = None;
    let mut ultimo = Vec::new();
    let pantalla: Vec2 = vec2(960.0, 720.0);
    for fotograma in 0..6u32 {
        let entrada = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, pantalla)),
            time: Some(f64::from(fotograma) * 0.25),
            ..RawInput::default()
        };
        let mut salida = ctx.run_ui(entrada, |ui| {
            apply_scale(ui.ctx());
            let reaccion = show_dialog(ui.ctx(), dialog, &mut foco);
            assert_eq!(reaccion, Reaction::None, "sin eventos no hay respuesta");
        });
        salida.textures_delta.clear();
        let formas: Vec<ClippedShape> = std::mem::take(&mut salida.shapes);
        ultimo.clear();
        for clipped in &formas {
            aplana(&clipped.shape, clipped.clip_rect, &mut ultimo);
        }
    }
    ultimo
}

/// Los fallos de contraste de los textos no atenuados y cuántos se comprobaron.
fn fallos_de_contraste(pintado: &[Pintado]) -> (Vec<String>, usize) {
    let mut fallos = Vec::new();
    let mut comprobados = 0;
    for (i, forma) in pintado.iter().enumerate() {
        let Pintado::Texto(t) = forma else { continue };
        let visible = t.rect.intersect(t.clip);
        if t.texto.trim().is_empty() || !visible.is_positive() {
            continue;
        }
        let atenuado = t.opacidad < 1.0
            || t.colores.iter().any(|c| *c == TEXT_DISABLED || c.a() < 255)
            || t.glifo.is_some_and(|c| c.a() < 255 || c == TEXT_DISABLED);
        if atenuado {
            continue;
        }
        let sonda = visible.shrink2(vec2(0.0, visible.height() * 0.25));
        let mut fondo = BACKGROUND;
        let mut hay_relleno = false;
        for anterior in &pintado[..i] {
            if let Pintado::Relleno { rect, fill, clip } = anterior
                && rect.intersect(*clip).expand(0.01).contains_rect(sonda)
            {
                hay_relleno = true;
                fondo = encima(fondo, *fill);
            }
        }
        if !hay_relleno {
            fallos.push(format!("el texto {:?} no tiene relleno debajo", t.texto));
            continue;
        }
        let color = t
            .glifo
            .or_else(|| t.colores.first().copied())
            .unwrap_or(Color32::PLACEHOLDER);
        comprobados += 1;
        let ratio = contrast_ratio(color, fondo);
        if ratio < 4.5 {
            fallos.push(format!(
                "el texto {:?} ({color:?}) sobre {fondo:?} da {ratio:.2} (< 4,5)",
                t.texto
            ));
        }
    }
    (fallos, comprobados)
}

fn textos_pintados(pintado: &[Pintado]) -> Vec<&str> {
    pintado
        .iter()
        .filter_map(|f| match f {
            Pintado::Texto(t) => Some(t.texto.as_str()),
            Pintado::Relleno { .. } => None,
        })
        .collect()
}

#[test]
fn fila_4_el_dialogo_pinta_sus_textos_con_contraste_al_menos_4_5_y_el_motivo_visible() {
    for ppp in PPPS {
        // (texto escrito, confirmado con Intro, lo que debe verse además)
        let estados: [(&str, bool, &str); 4] = [
            ("", false, "Fondo del lienzo"),
            (
                "#ff000080",
                true,
                "error: el fondo debe ser opaco (alfa ff)",
            ),
            ("#fff8", false, "error: el fondo debe ser opaco (alfa ff)"),
            ("zz", false, "usa 3, 4, 6 u 8 dígitos"),
        ];
        for (texto, intro, visible) in estados {
            let mut modal = abierto(CanvasBackground::Checker);
            if !texto.is_empty() {
                escribe(&mut modal, texto);
            }
            if intro {
                confirma(&mut modal);
            }
            let Modal::Open(dialog) = &mut modal else {
                unreachable!()
            };
            let pintado = pinta_el_dialogo(ppp, dialog);
            let etiqueta = format!("ppp {ppp}, texto {texto:?}");
            let textos = textos_pintados(&pintado);
            for esperado in ["Fondo del lienzo", "Cancelar", "Aplicar", visible] {
                assert!(
                    textos.iter().any(|t| t.contains(esperado)),
                    "{etiqueta}: falta {esperado:?}; se pinta {textos:?}"
                );
            }
            // El motivo de opacidad nunca va cortado y el error en línea lleva su prefijo.
            if visible.contains("opaco") {
                assert!(
                    textos
                        .iter()
                        .any(|t| *t == format!("{ERROR_PREFIX}{OPAQUE_REASON}")),
                    "{etiqueta}: el motivo se ve entero en el pie; se pinta {textos:?}"
                );
            }
            let (fallos, comprobados) = fallos_de_contraste(&pintado);
            assert!(
                comprobados >= 3,
                "{etiqueta}: solo se comprobaron {comprobados} textos; {textos:?}"
            );
            assert!(fallos.is_empty(), "{etiqueta}: {fallos:#?}");
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 5: sesión y lienzo, sin ventana
// ---------------------------------------------------------------------------------------------

static CONTADOR: AtomicU64 = AtomicU64::new(0);

/// Directorio temporal propio del test; el guardián lo borra al soltarse.
struct DirTemporal {
    ruta: PathBuf,
}

impl DirTemporal {
    fn nuevo() -> DirTemporal {
        let n = CONTADOR.fetch_add(1, Ordering::SeqCst);
        let ruta = std::env::temp_dir().join(format!(
            "pixoko-px-app-fondo-color-{}-{n}",
            std::process::id()
        ));
        fs::create_dir_all(&ruta).expect("crear el directorio temporal");
        DirTemporal { ruta }
    }
}

impl Drop for DirTemporal {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.ruta);
    }
}

fn imagen_rgba() -> Image {
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
fn edicion(image: ImageId, i: u32) -> Command {
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
struct Foto {
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
struct Mundo {
    session: Session,
    canvas: Canvas,
    modal: Modal,
}

impl Mundo {
    /// Una RGBA de 8×6 con dos pasos de historia y uno deshecho. Con `modificada`, una edición
    /// posterior sin guardar (el redo se pierde).
    fn nuevo(fondo: CanvasBackground, modificada: bool) -> Mundo {
        let mut document = Document::new(imagen_rgba());
        let id = document.main_image();
        for i in 0..2 {
            document.apply(edicion(id, i)).expect("paso");
        }
        document.undo().expect("deshacer");
        let mut session = Session::sample(document);
        if modificada {
            session.document.apply(edicion(id, 9)).expect("edición");
            assert!(session.modified());
        }
        let mut canvas = Canvas::new();
        canvas.set_image(session.document.image(id).expect("imagen"));
        canvas.set_background(fondo);
        Mundo {
            session,
            canvas,
            modal: Modal::default(),
        }
    }

    fn foto(&self) -> Foto {
        let image = self
            .session
            .document
            .image(self.session.document.main_image())
            .expect("imagen")
            .clone();
        Foto {
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

    fn imagen(&self) -> Image {
        self.foto().image
    }

    fn fondo(&self) -> CanvasBackground {
        self.canvas.background()
    }

    /// `Ver → Fondo del lienzo → Color…` con estos bloqueos.
    fn abre(&mut self, locks: Locks) -> bool {
        let fondo = self.canvas.background();
        open_background_color(&mut self.modal, fondo, locks)
    }

    /// Un evento al diálogo y, si se acepta, lo que hace `App` (D8): `Accepted` →
    /// `background_from_values` → `Canvas::set_background`.
    fn evento(&mut self, event: Event) -> Reaction {
        let reaccion = self.modal.event(event);
        self.aplica(&reaccion);
        reaccion
    }

    fn aplica(&mut self, reaccion: &Reaction) {
        if let Reaction::Accepted(valores) = reaccion
            && let Some(fondo) = background_from_values(valores)
        {
            self.canvas.set_background(fondo);
        }
    }

    fn escribe(&mut self, texto: &str) {
        let r = self.evento(Event::Hex {
            field: FIELD_COLOR,
            text: texto.to_string(),
        });
        assert_eq!(r, Reaction::None);
    }

    fn pulsa(&mut self, tecla: DialogKey) -> Reaction {
        let reaccion = pulsa(&mut self.modal, EN_EL_CAMPO, tecla);
        self.aplica(&reaccion);
        reaccion
    }
}

const MODIFICADA: [bool; 2] = [false, true];

#[test]
fn fila_5_la_sesion_de_la_prueba_se_puede_deshacer_y_rehacer_o_esta_modificada() {
    // Control: sin esto las comparaciones de `Foto` probarían poco.
    let m = Mundo::nuevo(CanvasBackground::Checker, false);
    assert!(m.session.document.can_undo() && m.session.document.can_redo());
    assert!(!m.session.modified());
    assert!(m.session.document.history_bytes() > 0);
    let m = Mundo::nuevo(CanvasBackground::Checker, true);
    assert!(m.session.modified() && m.session.document.can_undo());
}

#[test]
fn fila_5_aceptar_336699_da_custom_en_el_lienzo_el_menu_y_el_valor_desde_cada_fondo_previo() {
    for (previo, _) in PREVIOS {
        for modificada in MODIFICADA {
            let contexto = format!("{previo:?}, modificada {modificada}");
            let mut m = Mundo::nuevo(previo, modificada);
            let antes = m.foto();
            assert!(m.abre(Locks::default()), "{contexto}");
            assert_eq!(m.modal.kind(), Some(DialogKind::BackgroundColor));
            // Mientras está abierto el fondo no cambia (sin vista previa en vivo).
            m.escribe("#336699");
            assert_eq!(m.fondo(), previo, "{contexto}: sin vista previa");
            assert_eq!(
                m.pulsa(DialogKey::Enter),
                Reaction::None,
                "{contexto}: confirma"
            );
            assert_eq!(m.fondo(), previo, "{contexto}: confirmar tampoco aplica");
            let Reaction::Accepted(_) = m.pulsa(DialogKey::Enter) else {
                panic!("{contexto}: el segundo Intro acepta");
            };
            assert!(!m.modal.is_open());
            // El lienzo y el menú lo dicen.
            assert_eq!(
                m.fondo(),
                CanvasBackground::Custom(AZUL_ACERO),
                "{contexto}"
            );
            assert!(background_checked(AppAction::BackgroundColor, m.fondo()));
            for fijo in [
                CanvasBackground::Checker,
                CanvasBackground::Black,
                CanvasBackground::Gray,
                CanvasBackground::White,
            ] {
                assert!(!background_checked(
                    AppAction::CanvasBackground(fijo),
                    m.fondo()
                ));
            }
            assert_eq!(
                background_value_text(m.fondo()),
                Some("#336699".to_string()),
                "{contexto}"
            );
            // Es solo visualización: la sesión, igual.
            assert_eq!(
                m.foto(),
                antes,
                "{contexto}: ningún paso de fondo toca la sesión"
            );
        }
    }
}

#[test]
fn fila_5_aceptar_con_el_boton_aplicar_y_con_el_defecto_tambien_da_custom() {
    for (previo, color) in PREVIOS {
        let mut m = Mundo::nuevo(previo, false);
        let antes = m.foto();
        assert!(m.abre(Locks::default()));
        let Reaction::Accepted(_) = m.evento(Event::Accept) else {
            panic!("{previo:?}: Aplicar");
        };
        assert_eq!(m.fondo(), CanvasBackground::Custom(color), "{previo:?}");
        assert_eq!(m.foto(), antes);
    }
}

#[test]
fn fila_5_cancelar_deja_el_fondo_previo_sea_cual_sea() {
    for (previo, _) in PREVIOS {
        for modificada in MODIFICADA {
            let mut m = Mundo::nuevo(previo, modificada);
            let antes = m.foto();
            assert!(m.abre(Locks::default()));
            m.escribe("#336699");
            assert_eq!(m.evento(Event::Cancel), Reaction::Cancelled);
            assert!(!m.modal.is_open());
            assert_eq!(m.fondo(), previo, "{previo:?}: cancelar");
            assert_eq!(m.foto(), antes, "{previo:?}");
            // También con un texto confirmado (y con alfa) antes de cancelar.
            assert!(m.abre(Locks::default()));
            m.escribe("#ff000080");
            m.pulsa(DialogKey::Enter);
            assert_eq!(m.evento(Event::Cancel), Reaction::Cancelled);
            assert_eq!(m.fondo(), previo, "{previo:?}: cancelar tras confirmar");
            assert_eq!(m.foto(), antes);
        }
    }
}

#[test]
fn fila_5_escape_deja_el_fondo_previo_sea_cual_sea() {
    for (previo, _) in PREVIOS {
        let mut m = Mundo::nuevo(previo, false);
        let antes = m.foto();
        // Sin texto pendiente, `Escape` cancela a la primera.
        assert!(m.abre(Locks::default()));
        assert_eq!(m.pulsa(DialogKey::Escape), Reaction::Cancelled);
        assert!(!m.modal.is_open());
        assert_eq!(m.fondo(), previo);
        // Con texto pendiente, el primero lo descarta y el segundo cancela.
        assert!(m.abre(Locks::default()));
        m.escribe("#336699");
        assert_eq!(m.pulsa(DialogKey::Escape), Reaction::None);
        assert!(m.modal.is_open());
        assert_eq!(m.fondo(), previo);
        assert_eq!(m.pulsa(DialogKey::Escape), Reaction::Cancelled);
        assert_eq!(m.fondo(), previo, "{previo:?}");
        assert_eq!(m.foto(), antes);
    }
}

#[test]
fn fila_5_quit_abandona_el_dialogo_y_deja_el_fondo_previo_sea_cual_sea() {
    for (previo, _) in PREVIOS {
        let mut m = Mundo::nuevo(previo, true);
        let antes = m.foto();
        assert!(m.abre(Locks::default()));
        m.escribe("#336699");
        m.pulsa(DialogKey::Enter);
        assert!(m.modal.is_open());
        // `Quit` (y el cierre de la ventana): `Modal::abandon`, sin reacción.
        assert!(m.modal.abandon());
        assert!(!m.modal.is_open());
        assert_eq!(m.fondo(), previo, "{previo:?}");
        assert_eq!(m.foto(), antes);
        // Y no queda nada: otro `abandon` no hace nada y se puede reabrir con el color del fondo.
        assert!(!m.modal.abandon());
        assert!(m.abre(Locks::default()));
        assert_eq!(confirmado(&m.modal), initial_color(previo));
    }
}

#[test]
fn fila_5_con_cada_bloqueo_open_background_color_da_false_y_no_cambia_nada() {
    for (previo, _) in PREVIOS {
        for (nombre, locks) in bloqueos_de_uno_en_uno() {
            let mut m = Mundo::nuevo(previo, false);
            let antes = m.foto();
            assert!(!m.abre(locks), "{previo:?}, {nombre}");
            assert!(!m.modal.is_open(), "{previo:?}, {nombre}");
            assert_eq!(m.fondo(), previo);
            assert_eq!(m.foto(), antes);
        }
        // Con los cinco a la vez, también.
        let mut m = Mundo::nuevo(previo, false);
        assert!(!m.abre(bloqueos(31)));
        // Un segundo `Color…` con el diálogo abierto no hace nada.
        assert!(m.abre(Locks::default()));
        m.escribe("#336699");
        let antes = m.modal.clone();
        assert!(!m.abre(Locks::default()));
        assert_eq!(m.modal, antes);
    }
}

type Operacion = fn(&mut Mundo, &DirTemporal);

fn op_set_image(m: &mut Mundo, _: &DirTemporal) {
    let imagen = m.imagen();
    m.canvas.set_image(&imagen);
}

fn op_set_owned_image(m: &mut Mundo, _: &DirTemporal) {
    m.canvas.set_owned_image(imagen_rgba());
}

fn op_nueva_imagen(m: &mut Mundo, _: &DirTemporal) {
    let imagen = Image::new(5, 4, Mode::Rgba).expect("imagen");
    m.session.new_image(imagen.clone(), true).expect("nueva");
    m.canvas.set_owned_image(imagen);
}

fn op_abrir_un_png(m: &mut Mundo, dir: &DirTemporal) {
    let ruta = dir.ruta.join("abierta.png");
    fs::write(&ruta, write_png(&imagen_rgba()).expect("PNG")).expect("escribir");
    m.session.open(&ruta, 16384, true).expect("abrir");
    let imagen = m.imagen();
    m.canvas.set_image(&imagen);
}

fn op_convertir_de_modo(m: &mut Mundo, _: &DirTemporal) {
    let permitido = Allowed {
        luma: true,
        drop_alpha: Some([0, 0, 0]),
    };
    assert_eq!(m.session.convert_mode(Mode::Gray, permitido), Ok(true));
    let imagen = m.imagen();
    m.canvas.set_image(&imagen);
}

fn op_deshacer(m: &mut Mundo, _: &DirTemporal) {
    m.session.undo().expect("hay algo que deshacer");
}

fn op_rehacer(m: &mut Mundo, _: &DirTemporal) {
    m.session.redo().expect("hay algo que rehacer");
}

/// Las operaciones de documento de la fila 5, como las ejecuta `App`: cada una cambia el documento
/// y deja el lienzo con la imagen nueva (`set_image`, `set_owned_image`), pero no toca el fondo.
fn operaciones_de_documento() -> [(&'static str, Operacion); 7] {
    [
        ("set_image", op_set_image),
        ("set_owned_image", op_set_owned_image),
        ("Nueva imagen…", op_nueva_imagen),
        ("abrir un PNG", op_abrir_un_png),
        ("convertir de modo", op_convertir_de_modo),
        ("deshacer", op_deshacer),
        ("rehacer", op_rehacer),
    ]
}

#[test]
fn fila_5_el_fondo_sobrevive_a_set_image_set_owned_image_nueva_imagen_abrir_un_png_y_convertir() {
    for (previo, _) in PREVIOS {
        for (nombre, operacion) in operaciones_de_documento() {
            let dir = DirTemporal::nuevo();
            let mut m = Mundo::nuevo(previo, false);
            operacion(&mut m, &dir);
            assert_eq!(m.fondo(), previo, "{previo:?} tras {nombre}");
        }
    }
}

#[test]
fn fila_5_un_color_aceptado_sobrevive_a_las_operaciones_de_documento() {
    for (previo, _) in PREVIOS {
        for (nombre, operacion) in operaciones_de_documento() {
            let dir = DirTemporal::nuevo();
            let mut m = Mundo::nuevo(previo, false);
            assert!(m.abre(Locks::default()));
            m.escribe("#336699");
            m.pulsa(DialogKey::Enter);
            m.pulsa(DialogKey::Enter);
            assert_eq!(m.fondo(), CanvasBackground::Custom(AZUL_ACERO));
            operacion(&mut m, &dir);
            assert_eq!(
                m.fondo(),
                CanvasBackground::Custom(AZUL_ACERO),
                "{previo:?} tras {nombre}"
            );
            assert_eq!(
                background_value_text(m.fondo()),
                Some("#336699".to_string()),
                "{previo:?} tras {nombre}: el menú sigue diciéndolo"
            );
        }
    }
}

#[test]
fn fila_5_cada_paso_de_fondo_deja_la_sesion_igual_a_antes() {
    // Una sola sesión recorre todos los pasos de fondo seguidos: `Foto` igual tras cada uno.
    for modificada in MODIFICADA {
        let mut m = Mundo::nuevo(CanvasBackground::Checker, modificada);
        let antes = m.foto();
        for (previo, _) in PREVIOS {
            m.canvas.set_background(previo);
            assert_eq!(m.foto(), antes, "elegir {previo:?}");
            assert!(m.abre(Locks::default()));
            assert_eq!(m.foto(), antes, "abrir desde {previo:?}");
            m.escribe("#336699");
            assert_eq!(m.foto(), antes, "teclear");
            m.pulsa(DialogKey::Enter);
            assert_eq!(m.foto(), antes, "confirmar");
            m.pulsa(DialogKey::Enter);
            assert_eq!(m.foto(), antes, "aceptar desde {previo:?}");
            assert_eq!(m.fondo(), CanvasBackground::Custom(AZUL_ACERO));
        }
        assert_eq!(m.foto(), antes);
    }
}

#[test]
fn fila_5_un_segundo_color_con_custom_activo_nace_con_ese_color() {
    for (previo, _) in PREVIOS {
        let mut m = Mundo::nuevo(previo, false);
        assert!(m.abre(Locks::default()));
        m.escribe("#336699");
        m.pulsa(DialogKey::Enter);
        m.pulsa(DialogKey::Enter);
        assert_eq!(m.fondo(), CanvasBackground::Custom(AZUL_ACERO));
        // El segundo `Color…` nace con el color del fondo actual, el que se eligió.
        assert!(m.abre(Locks::default()));
        assert_eq!(texto_del_campo(&m.modal), "#336699", "{previo:?}");
        assert_eq!(confirmado(&m.modal), AZUL_ACERO);
        assert_eq!(motivo(&m.modal), None);
        // Cancelarlo no cambia nada, y tras `Negro` nace en `#000000` (no recuerda el propio).
        assert_eq!(m.evento(Event::Cancel), Reaction::Cancelled);
        assert_eq!(m.fondo(), CanvasBackground::Custom(AZUL_ACERO));
        m.canvas.set_background(CanvasBackground::Black);
        assert!(m.abre(Locks::default()));
        assert_eq!(texto_del_campo(&m.modal), "#000000", "{previo:?}");
    }
}

#[test]
fn fila_5_aceptar_dos_colores_seguidos_deja_el_ultimo() {
    let mut m = Mundo::nuevo(CanvasBackground::Checker, false);
    for texto in ["#336699", "#ff8000", "#010203"] {
        assert!(m.abre(Locks::default()));
        m.escribe(texto);
        m.pulsa(DialogKey::Enter);
        m.pulsa(DialogKey::Enter);
        let color = Rgba::from_hex(texto).expect("color");
        assert_eq!(m.fondo(), CanvasBackground::Custom(color), "{texto}");
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 8, parte de `px-app`: el fichero guardado no lleva el fondo
// ---------------------------------------------------------------------------------------------

fn es_gris_del_damero(c: &[u8; 4]) -> bool {
    c[3] == 255
        && [CHECK_A, CHECK_B]
            .iter()
            .any(|g| (g.r, g.g, g.b) == (c[0], c[1], c[2]))
}

/// Un fotograma sin ventana en el orden de la app: `interact` → `set_overlay` → `sync` → `paint`.
fn fotograma_del_lienzo(
    ctx: &Context,
    canvas: &mut Canvas,
    documento: &Image,
    overlay: &[(u32, u32, Rgba)],
) {
    let entrada = RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(200.0, 150.0))),
        ..RawInput::default()
    };
    let mut salida = ctx.run_ui(entrada, |ui| {
        canvas.interact_roles(ui);
        canvas.set_overlay(ui.ctx(), documento, overlay);
        canvas.sync(ui.ctx(), documento).expect("sync");
        canvas.paint(ui);
    });
    salida.textures_delta.clear();
}

#[test]
fn fila_8_write_png_da_los_mismos_bytes_con_cualquier_fondo_y_la_lectura_es_la_original() {
    // Alfa 0 en las columnas 0 y 1, 128 en las 2 y 3 y opacas el resto; ningún píxel opaco con un
    // color de los fondos que se prueban ni con un gris del damero.
    let original = imagen_rgba();
    let doc = Document::new(original.clone());
    let documento = doc.image(doc.main_image()).expect("imagen del documento");
    let state_id = doc.state_id();
    let bytes_rgba = to_rgba8(documento);
    let png_antes = write_png(documento).expect("PNG antes");

    let ctx = Context::default();
    let mut canvas = Canvas::new();
    canvas.set_image(documento);
    let overlay = [
        (0, 0, Rgba::new(0, 0, 0, 0)),
        (3, 2, Rgba::new(255, 0, 0, 128)),
        (6, 4, Rgba::opaque(255, 0, 0)),
    ];
    let fondos = [
        CanvasBackground::Custom(AZUL_ACERO),
        CanvasBackground::Custom(Rgba::opaque(0xff, 0x80, 0)),
        CanvasBackground::Custom(Rgba::opaque(1, 2, 3)),
        CanvasBackground::Custom(Rgba::opaque(40, 40, 40)),
        CanvasBackground::Checker,
        CanvasBackground::Black,
        CanvasBackground::Gray,
        CanvasBackground::White,
    ];
    for fondo in fondos {
        canvas.set_background(fondo);
        for superposicion in [&[][..], &overlay[..]] {
            for _ in 0..2 {
                fotograma_del_lienzo(&ctx, &mut canvas, documento, superposicion);
            }
            // Durante los fotogramas, el documento, `to_rgba8` y el `state_id` no cambian.
            assert_eq!(doc.state_id(), state_id, "{fondo:?}");
            assert_eq!(to_rgba8(documento), bytes_rgba, "{fondo:?}");
            // Y `write_png` da los mismos bytes con cada fondo.
            assert_eq!(
                write_png(documento).expect("PNG"),
                png_antes,
                "{fondo:?}: mismos bytes del PNG"
            );
        }
        canvas.clear_overlay();
    }
    // La lectura de vuelta es la imagen original, con alfa 0 donde era 0 y sin el color elegido ni
    // los grises del damero.
    let leido = read_png(&png_antes).expect("el PNG se lee");
    assert!(leido.is_exact(), "sin conversiones al leer");
    assert_eq!(leido.image, original);
    let leidos = to_rgba8(&leido.image);
    assert_eq!(leidos, bytes_rgba);
    for (i, pixel) in leidos.as_chunks::<4>().0.iter().enumerate() {
        assert!(
            !es_gris_del_damero(pixel),
            "el píxel {i} es un gris del damero"
        );
        for fondo in fondos {
            if let CanvasBackground::Custom(c) = fondo {
                assert!(
                    !(pixel[3] == 255 && (pixel[0], pixel[1], pixel[2]) == (c.r, c.g, c.b)),
                    "el píxel {i} del PNG tiene el color del fondo {fondo:?}"
                );
            }
        }
    }
    for y in 0..6usize {
        for x in 0..2usize {
            assert_eq!(
                leidos[(y * 8 + x) * 4 + 3],
                0,
                "({x}, {y}) sigue transparente"
            );
        }
    }
    assert_eq!(*documento, original);
    assert_eq!(doc.state_id(), state_id);
    assert!(!doc.can_undo());
}

// ---------------------------------------------------------------------------------------------
// Propiedad 1: el formulario acepta exactamente los opacos
// ---------------------------------------------------------------------------------------------

/// Texto de un hexadecimal: de 0 a 10 dígitos, con y sin `#`, con espacios, con algún carácter que no
/// es hexadecimal, o uno de los textos fijos de las filas.
fn texto_hexadecimal() -> impl Strategy<Value = String> {
    let fijos: Vec<String> = textos_que_valen()
        .into_iter()
        .map(|(t, _)| t.to_string())
        .chain(TEXTOS_CON_ALFA.iter().map(|t| t.to_string()))
        .chain(["zz", "#12", "", " ", "#", "#gggggg", "12 34"].map(String::from))
        .collect();
    let aleatorio = (
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
        .prop_map(|(almohadilla, digitos, antes, despues, malo)| {
            let mut cuerpo: Vec<char> = digitos.into_iter().map(char::from).collect();
            if let Some((i, c)) = malo {
                let i = i % (cuerpo.len() + 1);
                cuerpo.insert(i, c);
            }
            let mut texto = " ".repeat(antes);
            if almohadilla {
                texto.push('#');
            }
            texto.extend(cuerpo);
            texto.push_str(&" ".repeat(despues));
            texto
        });
    prop_oneof![
        3 => aleatorio,
        1 => prop::sample::select(fijos),
    ]
}

fn color_cualquiera() -> impl Strategy<Value = Rgba> {
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
    /// llega si y solo si `resolve_hex(texto, Mode::Rgba, confirmado)` da `Ok(c)` con `c.a == 255`, y
    /// entonces `background_from_values` es `Custom(c)`; en los demás casos hay `Rejected`, el
    /// diálogo sigue abierto y no cambia nada.
    #[test]
    fn propiedad_1_el_formulario_acepta_exactamente_los_opacos(
        previo in 0usize..5,
        confirmado_antes in color_cualquiera(),
        texto in prop::option::of(texto_hexadecimal()),
    ) {
        let (fondo, _) = PREVIOS[previo];
        let mut modal = abierto(fondo);
        // El último confirmado, con su alfa: se escribe entero (8 dígitos) y se confirma.
        escribe(
            &mut modal,
            &format!(
                "#{:02x}{:02x}{:02x}{:02x}",
                confirmado_antes.r, confirmado_antes.g, confirmado_antes.b, confirmado_antes.a
            ),
        );
        confirma(&mut modal);
        prop_assert_eq!(confirmado(&modal), confirmado_antes);
        if let Some(t) = &texto {
            escribe(&mut modal, t);
        }
        let esperado = resolve_hex(texto.as_deref(), Mode::Rgba, confirmado_antes);
        let antes = dialogo(&modal).clone();
        let reaccion = acepta(&mut modal);
        match esperado {
            Ok(c) if c.a == 255 => {
                let Reaction::Accepted(valores) = reaccion else {
                    return Err(TestCaseError::fail(format!("se esperaba Accepted y hay {reaccion:?}")));
                };
                prop_assert!(!modal.is_open());
                prop_assert_eq!(valores.color(FIELD_COLOR), Some(c));
                prop_assert_eq!(
                    background_from_values(&valores),
                    Some(CanvasBackground::Custom(c))
                );
            }
            Ok(_) => {
                // Alfa < 255: el campo vale y el conjunto no.
                prop_assert_eq!(reaccion, Reaction::Rejected(OPAQUE_REASON.to_string()));
                prop_assert!(modal.is_open());
                prop_assert_eq!(dialogo(&modal), &antes);
                prop_assert_eq!(dialogo(&modal).validity(FIELD_COLOR), Some(Validity::Valid));
                prop_assert_eq!(dialogo(&modal).values(), None);
            }
            Err(error) => {
                // Inválido: el error en línea es el de `hex_notice` y el conjunto no vale.
                prop_assert!(matches!(reaccion, Reaction::Rejected(_)), "{:?}", reaccion);
                prop_assert!(modal.is_open());
                prop_assert_eq!(dialogo(&modal), &antes);
                prop_assert_eq!(
                    dialogo(&modal).validity(FIELD_COLOR),
                    Some(Validity::Invalid(hex_notice(&error).to_string()))
                );
                prop_assert_eq!(dialogo(&modal).values(), None);
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Propiedad 2: el fondo es un modelo trivial «el último aplicado»
// ---------------------------------------------------------------------------------------------

/// Los textos que se teclean en la propiedad 2.
const TECLEADOS: [&str; 9] = [
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
enum Paso {
    /// Elegir uno de los cinco fondos (`Dispatched::Background` o el que dejó un `Color…` anterior).
    Elegir(usize),
    /// Abrir el diálogo con estos bloqueos (los cinco bits).
    Abrir(u8),
    Teclear(usize),
    Intro,
    Escape,
    Aceptar,
    Cancelar,
    Quit,
    SetImage,
    NuevaImagen,
    Deshacer,
    Rehacer,
    Trazo(u32),
}

fn un_paso() -> impl Strategy<Value = Paso> {
    prop_oneof![
        2 => (0usize..5).prop_map(Paso::Elegir),
        3 => Just(Paso::Abrir(0)),
        2 => (1u8..32).prop_map(Paso::Abrir),
        3 => (0usize..TECLEADOS.len()).prop_map(Paso::Teclear),
        2 => Just(Paso::Intro),
        2 => Just(Paso::Escape),
        2 => Just(Paso::Aceptar),
        1 => Just(Paso::Cancelar),
        1 => Just(Paso::Quit),
        1 => Just(Paso::SetImage),
        1 => Just(Paso::NuevaImagen),
        1 => Just(Paso::Deshacer),
        1 => Just(Paso::Rehacer),
        1 => (0u32..48).prop_map(Paso::Trazo),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// `canvas.background()` es siempre el del modelo («el último fondo aplicado»); cancelar,
    /// `Escape`, `Quit` y los rechazos no lo cambian; `open_background_color` solo da `true` con los
    /// cinco bloqueos en falso y sin otro modal; ningún paso de fondo cambia `state_id`, `modified`,
    /// la historia, la imagen ni la ruta.
    #[test]
    fn propiedad_2_el_fondo_es_el_ultimo_aplicado_y_los_pasos_de_fondo_no_tocan_la_sesion(
        pasos in prop::collection::vec(un_paso(), 0..=32),
    ) {
        let fondos: [CanvasBackground; 5] = [
            CanvasBackground::Checker,
            CanvasBackground::Black,
            CanvasBackground::Gray,
            CanvasBackground::White,
            CanvasBackground::Custom(AZUL_OSCURO),
        ];
        let mut m = Mundo::nuevo(CanvasBackground::Checker, false);
        let mut modelo = CanvasBackground::Checker;
        for (n, paso) in pasos.into_iter().enumerate() {
            let antes = m.foto();
            let abierto = m.modal.is_open();
            let mut de_fondo = true;
            match &paso {
                Paso::Elegir(i) => {
                    // Con el diálogo abierto, el menú está bloqueado.
                    if !abierto {
                        m.canvas.set_background(fondos[*i]);
                        modelo = fondos[*i];
                    }
                }
                Paso::Abrir(bits) => {
                    let locks = bloqueos(*bits);
                    let ninguno = !(locks.gesture || locks.dialog || locks.text_focus
                        || locks.removal || locks.modal);
                    let hecho = m.abre(locks);
                    prop_assert_eq!(hecho, ninguno && !abierto, "paso {}: {:?}", n, paso);
                    if hecho {
                        prop_assert_eq!(m.modal.kind(), Some(DialogKind::BackgroundColor));
                        prop_assert_eq!(confirmado(&m.modal), initial_color(modelo));
                    }
                }
                Paso::Teclear(i) => {
                    if abierto {
                        m.escribe(TECLEADOS[*i]);
                    }
                }
                Paso::Intro => {
                    if abierto {
                        let r = m.pulsa(DialogKey::Enter);
                        if let Reaction::Accepted(valores) = &r {
                            modelo = background_from_values(valores)
                                .expect("un Accepted siempre trae un fondo opaco");
                        }
                    }
                }
                Paso::Escape => {
                    if abierto {
                        let r = m.pulsa(DialogKey::Escape);
                        prop_assert!(!matches!(r, Reaction::Accepted(_)), "Escape no acepta");
                    }
                }
                Paso::Aceptar => {
                    if abierto {
                        match m.evento(Event::Accept) {
                            Reaction::Accepted(valores) => {
                                modelo = background_from_values(&valores)
                                    .expect("un Accepted siempre trae un fondo opaco");
                            }
                            Reaction::Rejected(_) => prop_assert!(m.modal.is_open()),
                            otra => prop_assert!(false, "Accept dio {:?}", otra),
                        }
                    }
                }
                Paso::Cancelar => {
                    if abierto {
                        prop_assert_eq!(m.evento(Event::Cancel), Reaction::Cancelled);
                    }
                }
                Paso::Quit => {
                    prop_assert_eq!(m.modal.abandon(), abierto);
                }
                Paso::SetImage => {
                    if !abierto {
                        let imagen = m.imagen();
                        m.canvas.set_image(&imagen);
                        de_fondo = false;
                    }
                }
                Paso::NuevaImagen => {
                    if !abierto {
                        let imagen = Image::new(5, 4, Mode::Rgba).expect("imagen");
                        m.session.new_image(imagen.clone(), true).expect("nueva");
                        m.canvas.set_owned_image(imagen);
                        de_fondo = false;
                    }
                }
                Paso::Deshacer => {
                    if !abierto {
                        let _ = m.session.undo();
                        de_fondo = false;
                    }
                }
                Paso::Rehacer => {
                    if !abierto {
                        let _ = m.session.redo();
                        de_fondo = false;
                    }
                }
                Paso::Trazo(i) => {
                    if !abierto {
                        let id = m.session.document.main_image();
                        let (ancho, alto) = {
                            let imagen = m.imagen();
                            (imagen.width(), imagen.height())
                        };
                        let _ = m.session.document.apply(Command::SetPixels {
                            image: id,
                            pixels: vec![PixelEdit {
                                x: i % ancho,
                                y: i / ancho % alto,
                                pixel: Pixel::Rgba(Rgba::opaque(9, 9, (i % 200) as u8)),
                            }],
                        });
                        de_fondo = false;
                    }
                }
            }
            // El lienzo dice siempre lo mismo que el modelo.
            prop_assert_eq!(m.canvas.background(), modelo, "paso {}: {:?}", n, paso);
            // Si hay un diálogo, es el de color y su estado es coherente.
            if let Some(kind) = m.modal.kind() {
                prop_assert_eq!(kind, DialogKind::BackgroundColor);
            }
            // Un paso de fondo no toca la sesión.
            if de_fondo {
                prop_assert_eq!(m.foto(), antes, "paso {}: {:?}", n, paso);
            }
        }
    }
}
