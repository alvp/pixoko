//! Tests de `new_image` en su parte sin egui (specs/hito-2-nueva-imagen.md, unidad 19): filas 1 a 8
//! (menos lo que es de banco de egui, que va en `dialog_tests.rs`), propiedades 1, 2, 3 y 4. En
//! ROJO hasta la implementación: `new_image` solo declara estos módulos de prueba.
//!
//! - Fila 1: límites del formulario (`new_image_form`, `size_error`).
//! - Fila 2 (parte pura): máximo de cada campo según el lado de textura de la GPU.
//! - Fila 3: `background_pixel`, `default_background`, `spec_from_values` y los motivos del hexadecimal.
//! - Fila 4: `carry_background` (tabla D1b) y `sync_form` aplicándola en el diálogo.
//! - Fila 5: `initial_palette` (los 32 valores literales de D2) y `build_new_image` con el real.
//! - Fila 6: `memory_estimate` y `report_text`.
//! - Fila 7: `ImageAlloc` falso que falla en la imagen o en el `probe` (la 2.ª imagen, la copia del
//!   lienzo, desapareció en la 14a-2); el real a 8192².
//! - Fila 8 (`describe_new`): el aviso de 5 s tras crear.
//! - Propiedades 1 (construir es exacto), 2 (trasladar nunca pierde color en silencio, exhaustiva),
//!   3 (el estado del formulario no se corrompe) y 4 (fallar no cambia nada, exhaustiva).
//!
//! **Suposiciones y ambigüedades** (la spec no fija un detalle; se elige lo más literal):
//! - Ids de campo: los literales de D11 (`ancho`, `alto`, `modo`, `fondo_indexado`, `fondo_rgba`,
//!   `color_rgba`, `fondo_gris_alfa`, `color_gris_alfa`, `fondo_gris`, `color_gris`, `informe`); de
//!   las constantes solo se nombra `FIELD_WIDTH`.
//! - Orden de las opciones de `modo`: Indexado (0), RGBA (1), Gris con alfa (2), Gris (3), y por
//!   defecto Indexado. Opciones del fondo: RGBA y Gris con alfa Transparente, Blanco, Negro, Color…;
//!   Gris Blanco, Negro, Color…; Indexado Transparente, Blanco, Negro (la primera es la de defecto).
//! - Derives no listados: `NewImageSpec`, `NewImageError`, `Carry` y `Memory` no se
//!   comparan ni se imprimen (se miran sus campos y se casan sus variantes); solo se suponen los de
//!   `Background` (los de la spec).
//! - `FormSync` se construye con `FormSync::default()` (la spec no da constructor). La primera
//!   llamada a `sync_form` toma como modo anterior el que tiene el diálogo (Indexado al abrir).
//! - `sync_form` también necesita leer la opción elegida de un grupo aunque el formulario sea
//!   inválido; en `px-ui` `Dialog::chosen` es `pub(crate)`: hará falta un acceso público. Las
//!   pruebas no lo usan: llevan su propio modelo del modo elegido.
//! - Texto del informe (`report_text`): líneas unidas con `\n`; el informe del diálogo se lee de
//!   `dialog.spec()` (el `ReadOnly` que cambia `set_report`).
//! - Segunda llamada a `sync_form` sin eventos: `false` («true si cambió algo que pintar»).
//! - `NewImageError::Memory { bytes }`: `bytes` es `memory_estimate(w, h, modo).ram_bytes` (la
//!   «estimación» de la spec es la de RAM, la que da la cifra en MiB del aviso). La spec no da una
//!   función para el aviso «sin memoria para una imagen de {w}×{h} (~{n} MiB)»: no se prueba aquí
//!   (queda para el cableado de `App`).
//! - `describe_new`: modos con los nombres de `describe_open` (`indexado`, `RGBA`, `gris`,
//!   `gris con alfa`); solo el indexado lleva ` · 32 colores`.
//! - El tamaño «grande» de la fila 5 es 1000 × 600 (esquinas y centro).
//! - El recorrido de `App` (D8) se escribe aquí como el ayudante `crea` (el cableado es del último
//!   commit de la unidad): `Unsaved` antes de construir nada, luego `build_new_image`, luego
//!   `Session::new_image` y `Canvas::set_image` con la imagen del documento. Se prueban los contratos de las tres piezas.
//! - `new_image_form` con un lado de textura menor que 64: el valor por defecto es `min(64, máximo)`
//!   (`validate` rechaza un defecto fuera de rango); con 1, el defecto es 1.
//! - La línea de estado del informe de la propiedad 3 se compara solo en lo que dice la spec
//!   (≤ 4 líneas, ≤ 42 caracteres, dentro del juego, empieza por `Memoria:`).

use super::*;
use px_core::ImageId;

use std::collections::HashSet;
use std::path::Path;

use eframe::egui::{Context, Event as InputEvent, MouseWheelUnit, Pos2, RawInput, Rect};
use eframe::egui::{Modifiers, MouseWheelSource, TouchPhase, pos2, vec2};
use proptest::prelude::*;
use px_core::{Command, Document, Hsl, Image, ImageError, Mode, Palette, Pixel, PixelEdit, Rgba};
use px_ui::canvas::{Canvas, CanvasInfo};
use px_ui::color::ColorChoice;
use px_ui::color_edit::{HexError, hex_notice};
use px_ui::dialog::{Dialog, DialogKind, Event, FieldKind, Reaction, Rows, ShowWhen};
use px_ui::dialog::{Validity, Values};
use px_ui::dialog_keys::{FocusTarget, initial_focus};
use px_ui::dialog_view::ERROR_PREFIX;
use px_ui::text::INTERFACE_CHARSET;

use crate::files::FileError;
use crate::session::Session;

// ---------------------------------------------------------------------------------------
// Ayudantes: modos, fondos, formulario y lectura del diálogo.
// ---------------------------------------------------------------------------------------

/// Los modos en el orden del campo `modo` de D11.
const MODOS: [Mode; 4] = [Mode::Indexed, Mode::Rgba, Mode::GrayAlpha, Mode::Gray];

const GRIS_OPACO: Rgba = Rgba::opaque(0x80, 0x80, 0x80);
const GRIS_MEDIO: Rgba = Rgba::new(0x80, 0x80, 0x80, 0x80);
const ROJO: Rgba = Rgba::opaque(0xff, 0, 0);
const ROJO_MEDIO: Rgba = Rgba::new(0xff, 0, 0, 0x80);

const TRANSPARENTE: Background = Background::Transparent;
const BLANCO: Background = Background::White;
const NEGRO: Background = Background::Black;

/// Los cuatro tipos de opción de un fondo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tipo {
    Transparente,
    Blanco,
    Negro,
    Color,
}

fn indice_de_modo(mode: Mode) -> usize {
    MODOS.iter().position(|m| *m == mode).expect("modo")
}

fn id_fondo(mode: Mode) -> &'static str {
    match mode {
        Mode::Indexed => "fondo_indexado",
        Mode::Rgba => "fondo_rgba",
        Mode::GrayAlpha => "fondo_gris_alfa",
        Mode::Gray => "fondo_gris",
    }
}

fn id_color(mode: Mode) -> Option<&'static str> {
    match mode {
        Mode::Indexed => None,
        Mode::Rgba => Some("color_rgba"),
        Mode::GrayAlpha => Some("color_gris_alfa"),
        Mode::Gray => Some("color_gris"),
    }
}

/// El índice de la opción de `tipo` en el grupo de fondo de `mode` (D1 y D11).
fn indice_de_fondo(mode: Mode, tipo: Tipo) -> Option<usize> {
    match (mode, tipo) {
        (Mode::Gray, Tipo::Transparente) => None,
        (Mode::Gray, Tipo::Blanco) => Some(0),
        (Mode::Gray, Tipo::Negro) => Some(1),
        (Mode::Gray, Tipo::Color) => Some(2),
        (Mode::Indexed, Tipo::Color) => None,
        (_, Tipo::Transparente) => Some(0),
        (_, Tipo::Blanco) => Some(1),
        (_, Tipo::Negro) => Some(2),
        (_, Tipo::Color) => Some(3),
    }
}

fn tipo_de(fondo: Background) -> Tipo {
    match fondo {
        Background::Transparent => Tipo::Transparente,
        Background::White => Tipo::Blanco,
        Background::Black => Tipo::Negro,
        Background::Color(_) => Tipo::Color,
    }
}

/// Los fondos que un modo admite (de la tabla D1), con los colores de la fila 3.
fn fondos_validos(mode: Mode) -> Vec<Background> {
    let color = Background::Color;
    match mode {
        Mode::Rgba => vec![
            TRANSPARENTE,
            BLANCO,
            NEGRO,
            color(GRIS_OPACO),
            color(ROJO_MEDIO),
            color(GRIS_MEDIO),
        ],
        Mode::GrayAlpha => vec![
            TRANSPARENTE,
            BLANCO,
            NEGRO,
            color(GRIS_OPACO),
            color(GRIS_MEDIO),
        ],
        Mode::Gray => vec![BLANCO, NEGRO, color(GRIS_OPACO)],
        Mode::Indexed => vec![TRANSPARENTE, BLANCO, NEGRO],
    }
}

/// Los seis fondos de la fila 3.
fn fondos_de_la_fila_3() -> [Background; 6] {
    [
        TRANSPARENTE,
        BLANCO,
        NEGRO,
        Background::Color(GRIS_OPACO),
        Background::Color(ROJO_MEDIO),
        Background::Color(GRIS_MEDIO),
    ]
}

fn dialogo(max_side: usize) -> Dialog {
    Dialog::new(DialogKind::NewImage, new_image_form(max_side)).expect("formulario válido")
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

fn escribe_color(d: &mut Dialog, field: &'static str, text: &str) {
    assert_eq!(
        d.event(Event::Hex {
            field,
            text: text.to_string()
        }),
        Reaction::None
    );
}

fn confirma_color(d: &mut Dialog, field: &'static str) {
    assert_eq!(
        d.event(Event::HexConfirm { field, blur: false }),
        Reaction::None
    );
}

/// Elige `mode` y, en su grupo, `fondo` (con su hexadecimal confirmado si es un color).
fn pon_fondo(d: &mut Dialog, mode: Mode, fondo: Background) {
    elige(d, "modo", indice_de_modo(mode));
    let indice = indice_de_fondo(mode, tipo_de(fondo)).expect("el modo admite ese fondo");
    elige(d, id_fondo(mode), indice);
    if let Background::Color(color) = fondo {
        let id = id_color(mode).expect("el modo tiene color");
        escribe_color(d, id, &color.to_string());
        confirma_color(d, id);
    }
}

fn spec_de(d: &Dialog) -> Option<NewImageSpec> {
    spec_from_values(&d.values()?)
}

/// El contenido de una `NewImageSpec`, sin suponer sus derives.
fn campos(spec: &NewImageSpec) -> (u32, u32, Mode, Background) {
    (spec.width, spec.height, spec.mode, spec.background)
}

fn spec_nueva(width: u32, height: u32, mode: Mode, background: Background) -> NewImageSpec {
    NewImageSpec {
        width,
        height,
        mode,
        background,
    }
}

fn en_el_juego(texto: &str) -> bool {
    texto.chars().all(|c| INTERFACE_CHARSET.contains(c))
}

fn invalido(texto: &str) -> Option<Validity> {
    Some(Validity::Invalid(texto.to_string()))
}

/// El texto de un campo `ReadOnly`, leído de la spec del diálogo.
fn texto_del_informe(d: &Dialog) -> String {
    d.spec()
        .fields
        .iter()
        .find_map(|f| match &f.kind {
            FieldKind::ReadOnly { text, .. } if f.id == "informe" => Some(text.clone()),
            _ => None,
        })
        .expect("el formulario tiene el informe")
}

fn color_visible(mode: Mode, fondo: Background) -> Option<Rgba> {
    Some(match background_pixel(mode, fondo)? {
        Pixel::Rgba(color) => color,
        Pixel::GrayAlpha { gray, alpha } => Rgba::new(gray, gray, gray, alpha),
        Pixel::Gray(gray) => Rgba::opaque(gray, gray, gray),
        Pixel::Indexed(index) => initial_palette().resolved(index).expect("índice válido"),
    })
}

/// `Ok` o el motivo en texto (para no suponer `Debug` en `NewImageError`).
fn construye(spec: &NewImageSpec, alloc: &mut dyn ImageAlloc) -> Result<Image, String> {
    match build_new_image(spec, alloc) {
        Ok(image) => Ok(image),
        Err(NewImageError::Size(texto)) => Err(format!("Size({texto})")),
        Err(NewImageError::Memory { bytes }) => Err(format!("Memory({bytes})")),
    }
}

fn construye_real(spec: &NewImageSpec) -> Image {
    construye(spec, &mut SystemAlloc).unwrap_or_else(|e| {
        panic!(
            "{:?} {}×{} {:?}: {e}",
            spec.mode, spec.width, spec.height, spec.background
        )
    })
}

// ---------------------------------------------------------------------------------------
// Fila 1: límites.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_1_el_formulario_con_lado_16384_y_8192_pide_ancho_y_alto_de_1_a_8192_con_64_por_defecto() {
    assert_eq!(FIELD_WIDTH, "ancho");
    assert_eq!((MAX_SIDE, DEFAULT_SIDE), (8192, 64));
    for max_side in [16384usize, 8192] {
        let spec = new_image_form(max_side);
        assert_eq!(spec.title, "Nueva imagen");
        assert_eq!(spec.accept, "Crear");
        assert_eq!(spec.cancel, Some("Cancelar"));
        spec.validate().expect("spec válida");
        for (id, label) in [("ancho", "Ancho"), ("alto", "Alto")] {
            let campo = spec
                .fields
                .iter()
                .find(|f| f.id == id)
                .unwrap_or_else(|| panic!("falta {id}"));
            assert_eq!(campo.label, label);
            assert_eq!(
                campo.kind,
                FieldKind::Number {
                    min: 1,
                    max: 8192,
                    default: 64
                },
                "max_side {max_side}: {id}"
            );
            assert!(campo.shown_when.is_none());
        }
    }
    // Con 16384 el máximo sigue siendo 8192 y no hay nada que cambie respecto a 8192.
    assert_eq!(new_image_form(16384), new_image_form(8192));
}

#[test]
fn fila_1_por_defecto_64_por_64_es_valido_y_el_foco_inicial_es_el_ancho() {
    for max_side in [16384usize, 8192] {
        let d = dialogo(max_side);
        assert!(d.is_valid(), "{:?}", d.reason());
        assert_eq!(d.reason(), None);
        let values = d.values().expect("valores");
        assert_eq!(values.number("ancho"), Some(64));
        assert_eq!(values.number("alto"), Some(64));
        assert_eq!(initial_focus(&d), FocusTarget::Field("ancho"));
        assert!(!d.is_shown("fondo_rgba"), "por defecto es Indexado");
    }
}

#[test]
fn fila_1_cada_valor_de_ancho_y_alto_da_su_motivo_con_16384_y_8192() {
    for max_side in [16384usize, 8192] {
        for (campo, etiqueta) in [("ancho", "Ancho"), ("alto", "Alto")] {
            for (texto, esperado) in [
                ("0", invalido("mínimo 1")),
                ("1", Some(Validity::Valid)),
                ("64", Some(Validity::Valid)),
                ("8192", Some(Validity::Valid)),
                ("8193", invalido("máximo 8192")),
                ("", invalido("escribe un número")),
                ("abc", invalido("solo dígitos 0-9")),
            ] {
                let mut d = dialogo(max_side);
                numero(&mut d, campo, texto);
                assert_eq!(
                    d.validity(campo),
                    esperado,
                    "max_side {max_side}, {campo} = {texto:?}"
                );
                match esperado {
                    Some(Validity::Invalid(motivo)) => {
                        assert!(!d.is_valid());
                        assert_eq!(d.values(), None);
                        assert_eq!(d.reason(), Some(format!("{etiqueta}: {motivo}")));
                        assert!(
                            !motivo.contains("tarjeta"),
                            "el límite de 8192 no es de la GPU: {motivo}"
                        );
                        // En línea: `error: ` y el motivo.
                        assert!(format!("{ERROR_PREFIX}{motivo}").len() <= 44);
                    }
                    _ => assert!(d.is_valid(), "{:?}", d.reason()),
                }
            }
        }
    }
}

#[test]
fn fila_1_el_motivo_de_lado_menor_y_mayor_dice_exactamente_error_minimo_1_y_error_maximo_8192() {
    let mut d = dialogo(16384);
    numero(&mut d, "ancho", "0");
    assert_eq!(
        format!("{ERROR_PREFIX}{}", motivo_de(&d, "ancho")),
        "error: mínimo 1"
    );
    numero(&mut d, "ancho", "8193");
    assert_eq!(
        format!("{ERROR_PREFIX}{}", motivo_de(&d, "ancho")),
        "error: máximo 8192"
    );
}

fn motivo_de(d: &Dialog, campo: &'static str) -> String {
    match d.validity(campo) {
        Some(Validity::Invalid(motivo)) => motivo,
        otro => panic!("{campo} debía ser inválido y es {otro:?}"),
    }
}

const TEXTO_DE_TAMANO: &str = "ancho × alto: máx. 67108864 píxeles";

#[test]
fn fila_1_size_error_da_none_en_8192_por_8192_en_todos_los_modos() {
    for mode in MODOS {
        assert_eq!(size_error(8192, 8192, mode), None, "{mode:?}");
        assert_eq!(size_error(1, 1, mode), None, "{mode:?}");
        assert_eq!(size_error(64, 64, mode), None, "{mode:?}");
        assert_eq!(size_error(8192, 1, mode), None, "{mode:?}");
    }
}

#[test]
fn fila_1_size_error_da_el_texto_de_d5_en_el_resto_sin_desbordar() {
    for (width, height) in [
        (8193, 8192),
        (8192, 8193),
        (16384, 16384),
        (u32::MAX, 1),
        (u32::MAX, u32::MAX),
        (0, 5),
        (5, 0),
        (0, 0),
    ] {
        for mode in MODOS {
            let texto = size_error(width, height, mode)
                .unwrap_or_else(|| panic!("{width}×{height} {mode:?}: debía dar error"));
            assert_eq!(texto, TEXTO_DE_TAMANO, "{width}×{height} {mode:?}");
        }
    }
}

#[test]
fn fila_1_el_texto_de_size_error_cabe_en_el_motivo_y_esta_en_el_juego() {
    assert_eq!(TEXTO_DE_TAMANO.chars().count(), 35);
    let texto = size_error(8193, 8192, Mode::Rgba).expect("error");
    assert!(texto.chars().count() <= 37, "{texto}");
    assert!(en_el_juego(&texto), "{texto}");
}

#[test]
fn fila_1_el_check_del_formulario_no_salta_con_lados_validos_y_usa_size_error() {
    let spec = new_image_form(16384);
    let check = spec.check.expect("el formulario tiene un check cruzado");
    // Con cualquier valor que el formulario deje pasar, el check no da motivo.
    let mut d = dialogo(16384);
    for (ancho, alto) in [("1", "1"), ("64", "64"), ("8192", "8192"), ("8192", "1")] {
        numero(&mut d, "ancho", ancho);
        numero(&mut d, "alto", alto);
        let values = d.values().unwrap_or_else(|| panic!("{ancho}×{alto}"));
        assert_eq!(check(&values), None, "{ancho}×{alto}");
        assert_eq!(d.reason(), None, "{ancho}×{alto}");
    }
}

// ---------------------------------------------------------------------------------------
// Fila 2 (parte pura): el lado de textura de la GPU.
// ---------------------------------------------------------------------------------------

fn maximo_esperado(max_side: usize) -> u32 {
    u32::try_from(max_side.clamp(1, 8192)).expect("cabe")
}

#[test]
fn fila_2_el_maximo_del_campo_es_max_1_min_8192_lado_para_cada_lado_de_textura() {
    for max_side in [8191usize, 4096, 2048, 100, 1, 8192, 16384] {
        let max = maximo_esperado(max_side);
        let spec = new_image_form(max_side);
        spec.validate()
            .unwrap_or_else(|e| panic!("max_side {max_side}: {e:?}"));
        for id in ["ancho", "alto"] {
            let campo = spec.fields.iter().find(|f| f.id == id).expect("campo");
            let FieldKind::Number {
                min,
                max: tope,
                default,
            } = campo.kind
            else {
                panic!("{id} no es numérico");
            };
            assert_eq!(min, 1, "max_side {max_side}: {id}");
            assert_eq!(tope, max, "max_side {max_side}: {id}");
            assert_eq!(
                default,
                64.min(max),
                "max_side {max_side}: {id} (por defecto)"
            );
        }
    }
}

#[test]
fn fila_2_el_maximo_vale_y_el_siguiente_da_maximo_n_sin_nota_en_el_formulario_a_secas() {
    for max_side in [8191usize, 4096, 2048, 100, 1, 8192, 16384] {
        let max = maximo_esperado(max_side);
        for campo in ["ancho", "alto"] {
            let mut d = dialogo(max_side);
            numero(&mut d, campo, &max.to_string());
            assert_eq!(
                d.validity(campo),
                Some(Validity::Valid),
                "{max_side} {campo}"
            );
            numero(&mut d, campo, &(max + 1).to_string());
            assert_eq!(
                d.validity(campo),
                invalido(&format!("máximo {max}")),
                "{max_side} {campo}: sin set_limit_note el texto es el de siempre"
            );
        }
    }
}

#[test]
fn fila_2_cero_y_once_cifras_dan_su_texto_de_siempre_con_cualquier_lado_de_textura() {
    for max_side in [8191usize, 4096, 2048, 100, 1, 8192, 16384] {
        for campo in ["ancho", "alto"] {
            let mut d = dialogo(max_side);
            numero(&mut d, campo, "0");
            assert_eq!(d.validity(campo), invalido("mínimo 1"), "{max_side}");
            numero(&mut d, campo, "99999999999");
            assert_eq!(
                d.validity(campo),
                invalido("máximo 10 cifras"),
                "{max_side}"
            );
            numero(&mut d, campo, "5000");
            let esperado = if maximo_esperado(max_side) >= 5000 {
                Some(Validity::Valid)
            } else {
                invalido(&format!("máximo {}", maximo_esperado(max_side)))
            };
            assert_eq!(d.validity(campo), esperado, "{max_side} {campo} = 5000");
        }
    }
}

#[test]
fn fila_2_la_nota_de_la_tarjeta_grafica_es_la_de_la_spec() {
    assert_eq!(GPU_LIMIT_NOTE, "tarjeta gráfica");
    assert!(en_el_juego(GPU_LIMIT_NOTE));
}

// ---------------------------------------------------------------------------------------
// Fila 3: fondo por modo.
// ---------------------------------------------------------------------------------------

fn rgba_px(r: u8, g: u8, b: u8, a: u8) -> Option<Pixel> {
    Some(Pixel::Rgba(Rgba::new(r, g, b, a)))
}

fn gris_alfa_px(gray: u8, alpha: u8) -> Option<Pixel> {
    Some(Pixel::GrayAlpha { gray, alpha })
}

#[test]
fn fila_3_background_pixel_en_rgba() {
    let esperado = [
        rgba_px(0, 0, 0, 0),
        rgba_px(255, 255, 255, 255),
        rgba_px(0, 0, 0, 255),
        rgba_px(0x80, 0x80, 0x80, 0xff),
        rgba_px(0xff, 0, 0, 0x80),
        rgba_px(0x80, 0x80, 0x80, 0x80),
    ];
    for (fondo, esperado) in fondos_de_la_fila_3().into_iter().zip(esperado) {
        assert_eq!(background_pixel(Mode::Rgba, fondo), esperado, "{fondo:?}");
    }
}

#[test]
fn fila_3_background_pixel_en_gris_con_alfa() {
    let esperado = [
        gris_alfa_px(0, 0),
        gris_alfa_px(255, 255),
        gris_alfa_px(0, 255),
        gris_alfa_px(0x80, 0xff),
        None, // el rojo no es gris
        gris_alfa_px(0x80, 0x80),
    ];
    for (fondo, esperado) in fondos_de_la_fila_3().into_iter().zip(esperado) {
        assert_eq!(
            background_pixel(Mode::GrayAlpha, fondo),
            esperado,
            "{fondo:?}"
        );
    }
}

#[test]
fn fila_3_background_pixel_en_gris_no_admite_transparente_ni_alfa() {
    let esperado = [
        None, // sin transparente
        Some(Pixel::Gray(255)),
        Some(Pixel::Gray(0)),
        Some(Pixel::Gray(0x80)),
        None, // no es gris
        None, // gris con alfa < 255
    ];
    for (fondo, esperado) in fondos_de_la_fila_3().into_iter().zip(esperado) {
        assert_eq!(background_pixel(Mode::Gray, fondo), esperado, "{fondo:?}");
    }
}

#[test]
fn fila_3_background_pixel_en_indexado_usa_los_indices_de_la_paleta_y_no_admite_color() {
    let esperado = [
        Some(Pixel::Indexed(0)),
        Some(Pixel::Indexed(31)),
        Some(Pixel::Indexed(25)),
        None,
        None,
        None,
    ];
    for (fondo, esperado) in fondos_de_la_fila_3().into_iter().zip(esperado) {
        assert_eq!(
            background_pixel(Mode::Indexed, fondo),
            esperado,
            "{fondo:?}"
        );
    }
    assert_eq!((BLACK_INDEX, WHITE_INDEX), (25, 31));
}

#[test]
fn fila_3_el_fondo_por_defecto_es_blanco_en_gris_y_transparente_en_los_demas() {
    assert_eq!(default_background(Mode::Gray), BLANCO);
    for mode in [Mode::Indexed, Mode::Rgba, Mode::GrayAlpha] {
        assert_eq!(default_background(mode), TRANSPARENTE, "{mode:?}");
    }
    // Y el de por defecto de cada modo lo admite el modo.
    for mode in MODOS {
        assert!(background_pixel(mode, default_background(mode)).is_some());
    }
}

#[test]
fn fila_3_el_formulario_tiene_un_grupo_de_fondo_por_modo_con_las_opciones_de_la_tabla() {
    let spec = new_image_form(8192);
    let ids: Vec<&str> = spec.fields.iter().map(|f| f.id).collect();
    assert_eq!(
        ids,
        [
            "ancho",
            "alto",
            "modo",
            "fondo_indexado",
            "fondo_rgba",
            "color_rgba",
            "fondo_gris_alfa",
            "color_gris_alfa",
            "fondo_gris",
            "color_gris",
            "informe"
        ]
    );
    let campo = |id: &str| spec.fields.iter().find(|f| f.id == id).expect("campo");
    let opciones = |id: &str| match campo(id).kind {
        FieldKind::Choice { options, default } => (options.to_vec(), default),
        ref otro => panic!("{id}: {otro:?}"),
    };
    assert_eq!(campo("modo").label, "Modo");
    assert_eq!(
        opciones("modo"),
        (vec!["Indexado", "RGBA", "Gris con alfa", "Gris"], 0)
    );
    let transparente_blanco_negro_color = vec!["Transparente", "Blanco", "Negro", "Color…"];
    assert_eq!(
        opciones("fondo_indexado"),
        (vec!["Transparente", "Blanco", "Negro"], 0)
    );
    assert_eq!(
        opciones("fondo_rgba"),
        (transparente_blanco_negro_color.clone(), 0)
    );
    assert_eq!(
        opciones("fondo_gris_alfa"),
        (transparente_blanco_negro_color, 0)
    );
    assert_eq!(
        opciones("fondo_gris"),
        (vec!["Blanco", "Negro", "Color…"], 0)
    );

    // Cada grupo se ve solo con su modo y cada hexadecimal solo con «Color…» (y con su modo).
    for (fondo, modo, color_id, color_en) in [
        ("fondo_indexado", 0, None, 0),
        ("fondo_rgba", 1, Some("color_rgba"), 3),
        ("fondo_gris_alfa", 2, Some("color_gris_alfa"), 3),
        ("fondo_gris", 3, Some("color_gris"), 2),
    ] {
        assert_eq!(campo(fondo).label, "Fondo");
        assert_eq!(
            campo(fondo).shown_when,
            Some(ShowWhen {
                field: "modo",
                choice: modo
            }),
            "{fondo}"
        );
        if let Some(color_id) = color_id {
            assert_eq!(campo(color_id).label, "Color");
            assert_eq!(
                campo(color_id).shown_when,
                Some(ShowWhen {
                    field: fondo,
                    choice: color_en
                }),
                "{color_id}"
            );
        }
    }
    // El hexadecimal nace en gris medio con alfa 255: válido en los tres modos.
    for (id, mode) in [
        ("color_rgba", Mode::Rgba),
        ("color_gris_alfa", Mode::GrayAlpha),
        ("color_gris", Mode::Gray),
    ] {
        assert_eq!(
            campo(id).kind,
            FieldKind::Hex {
                mode,
                default: GRIS_OPACO
            },
            "{id}"
        );
    }
    // El informe: solo lectura, cuatro filas.
    assert!(matches!(
        campo("informe").kind,
        FieldKind::ReadOnly {
            rows: Rows::Fixed(4),
            ..
        }
    ));
}

#[test]
fn fila_3_spec_from_values_lee_el_grupo_del_modo_con_cada_fondo_valido() {
    for mode in MODOS {
        for fondo in fondos_validos(mode) {
            let mut d = dialogo(8192);
            pon_fondo(&mut d, mode, fondo);
            let spec = spec_de(&d)
                .unwrap_or_else(|| panic!("{mode:?} {fondo:?}: spec_from_values dio None"));
            assert_eq!(campos(&spec), (64, 64, mode, fondo), "{mode:?} {fondo:?}");
        }
    }
}

#[test]
fn fila_3_spec_from_values_lee_ancho_y_alto() {
    let mut d = dialogo(8192);
    numero(&mut d, "ancho", "5");
    numero(&mut d, "alto", "3");
    let spec = spec_de(&d).expect("spec");
    assert_eq!(campos(&spec), (5, 3, Mode::Indexed, TRANSPARENTE));
}

#[test]
fn fila_3_spec_from_values_da_none_si_faltan_campos_visibles() {
    assert!(spec_from_values(&Values::default()).is_none());
}

#[test]
fn fila_3_spec_from_values_no_lee_los_grupos_ocultos_y_estos_conservan_su_valor() {
    let mut d = dialogo(8192);
    // RGBA con Negro; luego Gris con su grupo por defecto (Blanco); luego Indexado.
    pon_fondo(&mut d, Mode::Rgba, NEGRO);
    assert_eq!(
        campos(&spec_de(&d).expect("rgba")),
        (64, 64, Mode::Rgba, NEGRO)
    );
    elige(&mut d, "modo", indice_de_modo(Mode::Gray));
    assert_eq!(
        campos(&spec_de(&d).expect("gris")),
        (64, 64, Mode::Gray, BLANCO),
        "no lee el Negro oculto de RGBA"
    );
    elige(&mut d, "modo", indice_de_modo(Mode::Indexed));
    assert_eq!(
        campos(&spec_de(&d).expect("indexado")),
        (64, 64, Mode::Indexed, TRANSPARENTE)
    );
    elige(&mut d, "modo", indice_de_modo(Mode::Rgba));
    assert_eq!(
        campos(&spec_de(&d).expect("rgba otra vez")),
        (64, 64, Mode::Rgba, NEGRO),
        "el grupo oculto conservaba su elección"
    );
}

#[test]
fn fila_3_en_gris_un_color_no_gris_o_con_alfa_da_el_motivo_de_hex_notice_y_es_invalido() {
    for (texto, error) in [
        ("#ff0000", HexError::NotGray),
        ("#80808080", HexError::NoAlpha),
    ] {
        let mut d = dialogo(8192);
        elige(&mut d, "modo", indice_de_modo(Mode::Gray));
        elige(&mut d, "fondo_gris", 2);
        escribe_color(&mut d, "color_gris", texto);
        let motivo = hex_notice(&error);
        assert_eq!(d.validity("color_gris"), invalido(motivo), "{texto}");
        assert!(!d.is_valid(), "{texto}");
        assert_eq!(d.values(), None, "{texto}");
        let reason = d.reason().expect("hay motivo");
        assert!(reason.starts_with("Color: "), "{reason}");
        assert!(reason.chars().count() <= 37, "{reason}");
        assert!(spec_de(&d).is_none());
    }
}

#[test]
fn fila_3_en_gris_con_alfa_un_color_no_gris_es_invalido_y_en_rgba_vale() {
    let mut d = dialogo(8192);
    elige(&mut d, "modo", indice_de_modo(Mode::GrayAlpha));
    elige(&mut d, "fondo_gris_alfa", 3);
    escribe_color(&mut d, "color_gris_alfa", "#ff0000");
    assert_eq!(
        d.validity("color_gris_alfa"),
        invalido(hex_notice(&HexError::NotGray))
    );
    escribe_color(&mut d, "color_gris_alfa", "#80808080");
    assert_eq!(d.validity("color_gris_alfa"), Some(Validity::Valid));

    let mut d = dialogo(8192);
    elige(&mut d, "modo", indice_de_modo(Mode::Rgba));
    elige(&mut d, "fondo_rgba", 3);
    escribe_color(&mut d, "color_rgba", "#ff0000");
    assert_eq!(d.validity("color_rgba"), Some(Validity::Valid));
}

#[test]
fn fila_3_un_hexadecimal_corto_o_con_letras_malas_es_invalido_en_cualquier_modo() {
    for mode in [Mode::Rgba, Mode::GrayAlpha, Mode::Gray] {
        for texto in ["#12", "zz"] {
            let mut d = dialogo(8192);
            elige(&mut d, "modo", indice_de_modo(mode));
            elige(
                &mut d,
                id_fondo(mode),
                indice_de_fondo(mode, Tipo::Color).expect("color"),
            );
            let id = id_color(mode).expect("color");
            escribe_color(&mut d, id, texto);
            assert!(
                matches!(d.validity(id), Some(Validity::Invalid(_))),
                "{mode:?} {texto}"
            );
            assert!(!d.is_valid());
        }
    }
}

// ---------------------------------------------------------------------------------------
// Fila 4: trasladar el fondo.
// ---------------------------------------------------------------------------------------

/// Lo que se espera de una casilla de la tabla D1b.
#[derive(Clone, Copy, Debug)]
enum Casilla {
    Igual,
    Reinicia(Background, &'static str),
}

use Casilla::{Igual, Reinicia};

const AVISO_GRIS_SIN_TRANSPARENTE: &str = "Gris sin Transparente: pasa a Blanco";
const AVISO_INDEXADO_SIN_COLOR: &str = "Indexado sin Color: pasa a Transparente";
const AVISO_COLOR_CON_ALFA: &str = "Color con alfa: pasa a Blanco";
const AVISO_NO_GRIS_A_GRIS_ALFA: &str = "Color no gris: pasa a Transparente";
const AVISO_NO_GRIS_A_GRIS: &str = "Color no gris: pasa a Blanco";

const AVISOS: [&str; 5] = [
    AVISO_GRIS_SIN_TRANSPARENTE,
    AVISO_INDEXADO_SIN_COLOR,
    AVISO_COLOR_CON_ALFA,
    AVISO_NO_GRIS_A_GRIS_ALFA,
    AVISO_NO_GRIS_A_GRIS,
];

/// Destinos de las columnas de la tabla D1b, en su orden.
const DESTINOS: [Mode; 4] = [Mode::Rgba, Mode::GrayAlpha, Mode::Gray, Mode::Indexed];

/// La tabla D1b, copiada tal cual: (nombre, fondo de origen, modos en los que puede existir ese
/// fondo, casillas A RGBA / A Gris con alfa / A Gris / A Indexado).
fn tabla_d1b() -> Vec<(&'static str, Background, Vec<Mode>, [Casilla; 4])> {
    let todos = vec![Mode::Rgba, Mode::GrayAlpha, Mode::Gray, Mode::Indexed];
    let color = Background::Color;
    vec![
        (
            "transparente",
            TRANSPARENTE,
            vec![Mode::Rgba, Mode::GrayAlpha, Mode::Indexed],
            [
                Igual,
                Igual,
                Reinicia(BLANCO, AVISO_GRIS_SIN_TRANSPARENTE),
                Igual,
            ],
        ),
        (
            "blanco",
            BLANCO,
            todos.clone(),
            [Igual, Igual, Igual, Igual],
        ),
        ("negro", NEGRO, todos, [Igual, Igual, Igual, Igual]),
        (
            "color gris con alfa 255",
            color(GRIS_OPACO),
            vec![Mode::Rgba, Mode::GrayAlpha, Mode::Gray],
            [
                Igual,
                Igual,
                Igual,
                Reinicia(TRANSPARENTE, AVISO_INDEXADO_SIN_COLOR),
            ],
        ),
        (
            "color gris con alfa < 255",
            color(GRIS_MEDIO),
            vec![Mode::Rgba, Mode::GrayAlpha],
            [
                Igual,
                Igual,
                Reinicia(BLANCO, AVISO_COLOR_CON_ALFA),
                Reinicia(TRANSPARENTE, AVISO_INDEXADO_SIN_COLOR),
            ],
        ),
        (
            "color no gris",
            color(ROJO),
            vec![Mode::Rgba],
            [
                Igual,
                Reinicia(TRANSPARENTE, AVISO_NO_GRIS_A_GRIS_ALFA),
                Reinicia(BLANCO, AVISO_NO_GRIS_A_GRIS),
                Reinicia(TRANSPARENTE, AVISO_INDEXADO_SIN_COLOR),
            ],
        ),
        (
            "color no gris con alfa",
            color(ROJO_MEDIO),
            vec![Mode::Rgba],
            [
                Igual,
                Reinicia(TRANSPARENTE, AVISO_NO_GRIS_A_GRIS_ALFA),
                Reinicia(BLANCO, AVISO_NO_GRIS_A_GRIS),
                Reinicia(TRANSPARENTE, AVISO_INDEXADO_SIN_COLOR),
            ],
        ),
    ]
}

/// Lo que dio `carry_background`, sin suponer derives de `Carry`.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Visto {
    Kept(Background),
    Reset(Background, String),
}

fn traslada(from: Mode, fondo: Background, to: Mode) -> Visto {
    match carry_background(from, fondo, to) {
        Carry::Kept(b) => Visto::Kept(b),
        Carry::Reset { to, notice } => Visto::Reset(to, notice.to_string()),
    }
}

fn esperado(casilla: Casilla, fondo: Background) -> Visto {
    match casilla {
        Igual => Visto::Kept(fondo),
        Reinicia(a, aviso) => Visto::Reset(a, aviso.to_string()),
    }
}

#[test]
fn fila_4_carry_background_es_exactamente_la_tabla_d1b() {
    for (nombre, fondo, origenes, casillas) in tabla_d1b() {
        for from in origenes {
            for (to, casilla) in DESTINOS.into_iter().zip(casillas) {
                if from == to {
                    continue;
                }
                assert_eq!(
                    traslada(from, fondo, to),
                    esperado(casilla, fondo),
                    "{nombre}: de {from:?} a {to:?}"
                );
            }
        }
    }
}

#[test]
fn fila_4_con_el_mismo_modo_el_fondo_se_conserva() {
    for mode in MODOS {
        for fondo in fondos_validos(mode) {
            assert_eq!(
                traslada(mode, fondo, mode),
                Visto::Kept(fondo),
                "{mode:?} {fondo:?}"
            );
        }
    }
}

#[test]
fn fila_4_blanco_y_negro_nunca_se_reinician() {
    for from in MODOS {
        for to in MODOS {
            for fondo in [BLANCO, NEGRO] {
                assert_eq!(
                    traslada(from, fondo, to),
                    Visto::Kept(fondo),
                    "{from:?} → {to:?} {fondo:?}"
                );
            }
        }
    }
}

#[test]
fn fila_4_lo_que_se_conserva_lo_admite_el_modo_nuevo_y_lo_que_se_reinicia_va_al_de_por_defecto() {
    for (nombre, fondo, origenes, _) in tabla_d1b() {
        for from in origenes {
            for to in MODOS {
                match traslada(from, fondo, to) {
                    Visto::Kept(b) => assert!(
                        background_pixel(to, b).is_some(),
                        "{nombre}: {from:?} → {to:?}: conserva un fondo que el modo no admite"
                    ),
                    Visto::Reset(a, aviso) => {
                        assert_eq!(a, default_background(to), "{nombre}: {from:?} → {to:?}");
                        assert!(background_pixel(to, a).is_some());
                        assert!(!aviso.is_empty());
                    }
                }
            }
        }
    }
}

#[test]
fn fila_4_los_avisos_dicen_el_fondo_que_habia_y_a_cual_vuelve_y_caben() {
    let mut vistos = HashSet::new();
    for (_, fondo, origenes, _) in tabla_d1b() {
        for from in origenes {
            for to in MODOS {
                if let Visto::Reset(a, aviso) = traslada(from, fondo, to) {
                    vistos.insert(aviso.clone());
                    assert!(aviso.chars().count() <= 42, "{aviso}");
                    assert!(en_el_juego(&aviso), "{aviso}");
                    // Qué fondo había: Transparente o Color (con el origen).
                    let habia = match fondo {
                        Background::Transparent => "Transparente",
                        Background::Color(_) => "Color",
                        otro => panic!("{otro:?} nunca se reinicia"),
                    };
                    assert!(aviso.contains(habia), "{aviso} no dice que había {habia}");
                    // A cuál vuelve.
                    let vuelve = match a {
                        Background::Transparent => "pasa a Transparente",
                        Background::White => "pasa a Blanco",
                        otro => panic!("{otro:?} no es un fondo de reinicio"),
                    };
                    assert!(aviso.ends_with(vuelve), "{aviso} no acaba en {vuelve}");
                }
            }
        }
    }
    let esperados: HashSet<String> = AVISOS.iter().map(|s| s.to_string()).collect();
    assert_eq!(
        vistos, esperados,
        "salen los cinco avisos de la tabla y ninguno más"
    );
}

#[test]
fn fila_4_un_color_no_gris_con_alfa_dice_color_no_gris() {
    assert_eq!(
        traslada(Mode::Rgba, Background::Color(ROJO_MEDIO), Mode::Gray),
        Visto::Reset(BLANCO, AVISO_NO_GRIS_A_GRIS.to_string())
    );
    assert_eq!(
        traslada(Mode::Rgba, Background::Color(ROJO_MEDIO), Mode::GrayAlpha),
        Visto::Reset(TRANSPARENTE, AVISO_NO_GRIS_A_GRIS_ALFA.to_string())
    );
}

// --- sync_form: la tabla aplicada en el diálogo (D8) ---

fn paso(d: &mut Dialog, sync: &mut FormSync, evento: Event) {
    assert_eq!(d.event(evento), Reaction::None);
    sync_form(d, sync);
}

/// Un diálogo con el modo `mode` y su fondo `fondo`, como lo dejaría el usuario (un evento y un
/// `sync_form` por paso), y su `FormSync`.
fn dialogo_sincronizado(mode: Mode, fondo: Background) -> (Dialog, FormSync) {
    let mut d = dialogo(8192);
    let mut sync = FormSync::default();
    sync_form(&mut d, &mut sync);
    paso(
        &mut d,
        &mut sync,
        Event::Choice {
            field: "modo",
            index: indice_de_modo(mode),
        },
    );
    paso(
        &mut d,
        &mut sync,
        Event::Choice {
            field: id_fondo(mode),
            index: indice_de_fondo(mode, tipo_de(fondo)).expect("fondo del modo"),
        },
    );
    if let Background::Color(color) = fondo {
        let id = id_color(mode).expect("color");
        paso(
            &mut d,
            &mut sync,
            Event::Hex {
                field: id,
                text: color.to_string(),
            },
        );
        paso(
            &mut d,
            &mut sync,
            Event::HexConfirm {
                field: id,
                blur: false,
            },
        );
    }
    // El modo de partida pudo dejar un aviso (p. ej. Indexado → Gris): dos cambios reales de otro
    // campo lo borran y dejan el ancho como estaba.
    for delta in [1, -1] {
        paso(
            &mut d,
            &mut sync,
            Event::Step {
                field: "ancho",
                delta,
            },
        );
    }
    (d, sync)
}

fn lineas_del_informe(d: &Dialog) -> Vec<String> {
    texto_del_informe(d).lines().map(str::to_string).collect()
}

#[test]
fn fila_4_sync_form_aplica_la_tabla_al_cambiar_de_modo_y_avisa_en_el_informe() {
    for (nombre, fondo, origenes, casillas) in tabla_d1b() {
        for from in origenes {
            for (to, casilla) in DESTINOS.into_iter().zip(casillas) {
                if from == to {
                    continue;
                }
                let (mut d, mut sync) = dialogo_sincronizado(from, fondo);
                assert_eq!(
                    campos(&spec_de(&d).expect("antes")),
                    (64, 64, from, fondo),
                    "{nombre}: partida en {from:?}"
                );
                assert!(
                    AVISOS.iter().all(|a| !texto_del_informe(&d).contains(*a)),
                    "{nombre}: sin avisos antes de cambiar"
                );

                paso(
                    &mut d,
                    &mut sync,
                    Event::Choice {
                        field: "modo",
                        index: indice_de_modo(to),
                    },
                );
                let spec = spec_de(&d)
                    .unwrap_or_else(|| panic!("{nombre}: {from:?} → {to:?}: sin valores válidos"));
                let (fondo_esperado, aviso) = match casilla {
                    Igual => (fondo, None),
                    Reinicia(a, aviso) => (a, Some(aviso)),
                };
                assert_eq!(
                    campos(&spec),
                    (64, 64, to, fondo_esperado),
                    "{nombre}: de {from:?} a {to:?}"
                );
                // El informe es el de `report_text` con el aviso (o sin él).
                assert_eq!(
                    texto_del_informe(&d),
                    report_text(Some((64, 64)), to, aviso),
                    "{nombre}: de {from:?} a {to:?}"
                );
                // El hexadecimal del grupo nuevo lleva el color que viaja.
                if let (Background::Color(color), Some(id)) = (fondo_esperado, id_color(to)) {
                    assert_eq!(d.text(id), Some(color.to_string()), "{nombre}");
                }

                // El aviso dura hasta que cambia otro valor del formulario.
                assert!(
                    !sync_form(&mut d, &mut sync),
                    "sin cambios no hay nada que pintar"
                );
                if let Some(aviso) = aviso {
                    assert!(lineas_del_informe(&d).iter().any(|l| l == aviso));
                    paso(
                        &mut d,
                        &mut sync,
                        Event::Step {
                            field: "ancho",
                            delta: 1,
                        },
                    );
                    assert!(
                        AVISOS.iter().all(|a| !texto_del_informe(&d).contains(*a)),
                        "{nombre}: el aviso no sobrevive a otro cambio"
                    );
                    assert_eq!(texto_del_informe(&d), report_text(Some((65, 64)), to, None));
                }
            }
        }
    }
}

#[test]
fn fila_4_sync_form_devuelve_true_al_cambiar_algo_que_pintar_y_false_si_no() {
    let mut d = dialogo(8192);
    let mut sync = FormSync::default();
    sync_form(&mut d, &mut sync);
    assert!(!sync_form(&mut d, &mut sync), "sin eventos");
    // Un cambio de tamaño cambia la memoria del informe.
    numero(&mut d, "ancho", "4097");
    numero(&mut d, "alto", "4096");
    assert!(sync_form(&mut d, &mut sync), "el informe cambió");
    assert!(!sync_form(&mut d, &mut sync));
    // Un cambio de modo traslada el fondo y cambia el informe (RGBA no lleva la línea de la paleta).
    elige(&mut d, "modo", indice_de_modo(Mode::Rgba));
    assert!(sync_form(&mut d, &mut sync));
    assert!(!sync_form(&mut d, &mut sync));
}

#[test]
fn fila_4_sync_form_con_ancho_o_alto_invalidos_dice_memoria_sin_valor() {
    let mut d = dialogo(8192);
    let mut sync = FormSync::default();
    sync_form(&mut d, &mut sync);
    numero(&mut d, "ancho", "abc");
    sync_form(&mut d, &mut sync);
    assert_eq!(
        texto_del_informe(&d),
        report_text(None, Mode::Indexed, None),
        "ancho sin valor"
    );
    assert!(texto_del_informe(&d).starts_with("Memoria: —"));
    numero(&mut d, "ancho", "10");
    numero(&mut d, "alto", "");
    sync_form(&mut d, &mut sync);
    assert_eq!(
        texto_del_informe(&d),
        report_text(None, Mode::Indexed, None),
        "alto sin valor"
    );
    numero(&mut d, "alto", "20");
    sync_form(&mut d, &mut sync);
    assert_eq!(
        texto_del_informe(&d),
        report_text(Some((10, 20)), Mode::Indexed, None)
    );
}

// ---------------------------------------------------------------------------------------
// Fila 5: paleta inicial e imagen creada.
// ---------------------------------------------------------------------------------------

/// Los 32 valores literales de D2 (la entrada 0 es `#00000000`).
const PALETA_D2: [&str; 32] = [
    "#00000000",
    // fila 0 (1–8)
    "#800000",
    "#806000",
    "#408000",
    "#008020",
    "#008080",
    "#002080",
    "#400080",
    "#800060",
    // fila 1 (9–16)
    "#ff0000",
    "#ffbf00",
    "#80ff00",
    "#00ff40",
    "#00ffff",
    "#0040ff",
    "#8000ff",
    "#ff00bf",
    // fila 2 (17–24)
    "#ff8080",
    "#ffdf80",
    "#bfff80",
    "#80ff9f",
    "#80ffff",
    "#809fff",
    "#bf80ff",
    "#ff80df",
    // grises (25–31)
    "#000000",
    "#2b2b2b",
    "#555555",
    "#808080",
    "#aaaaaa",
    "#d5d5d5",
    "#ffffff",
];

fn paleta_literal() -> Vec<Rgba> {
    PALETA_D2
        .iter()
        .map(|h| Rgba::from_hex(h).unwrap_or_else(|e| panic!("{h}: {e:?}")))
        .collect()
}

#[test]
fn fila_5_la_paleta_inicial_tiene_los_32_valores_literales_de_d2() {
    let palette = initial_palette();
    assert_eq!(PALETTE_LEN, 32);
    assert_eq!(palette.len(), PALETTE_LEN);
    assert_eq!(palette.colors(), paleta_literal().as_slice());
    assert_eq!(
        palette.get(0),
        Ok(Rgba::new(0, 0, 0, 0)),
        "la entrada 0 es #00000000"
    );
}

#[test]
fn fila_5_la_entrada_0_es_el_indice_transparente_y_el_negro_y_el_blanco_estan_en_25_y_31() {
    let palette = initial_palette();
    assert_eq!(palette.transparent_index(), Some(0));
    assert_eq!(BLACK_INDEX, 25);
    assert_eq!(WHITE_INDEX, 31);
    assert_eq!(palette.get(BLACK_INDEX), Ok(Rgba::opaque(0, 0, 0)));
    assert_eq!(palette.get(WHITE_INDEX), Ok(Rgba::opaque(255, 255, 255)));
    // Todas las demás son opacas.
    for (i, color) in palette.colors().iter().enumerate().skip(1) {
        assert_eq!(color.a, 255, "entrada {i}");
    }
}

#[test]
fn fila_5_las_32_entradas_son_distintas() {
    let palette = initial_palette();
    let distintas: HashSet<Rgba> = palette.colors().iter().copied().collect();
    assert_eq!(distintas.len(), 32);
}

#[test]
fn fila_5_dos_llamadas_dan_la_misma_paleta_y_from_colors_la_acepta() {
    let a = initial_palette();
    let b = initial_palette();
    assert_eq!(a, b);
    assert_eq!(a.colors(), b.colors());
    let reconstruida = Palette::from_colors(a.colors().iter().copied()).expect("la acepta");
    assert_eq!(reconstruida.colors(), a.colors());
    // Y con ella se puede crear una imagen indexada.
    Image::new_indexed(2, 2, a).expect("imagen indexada");
}

#[test]
fn fila_5_las_entradas_1_a_24_siguen_la_regla_de_hsl_y_las_25_a_31_la_de_enteros() {
    let palette = initial_palette();
    for (fila, l) in [0.25f32, 0.50, 0.75].into_iter().enumerate() {
        for k in 0..8usize {
            let esperado = Hsl::new(45.0 * k as f32, 1.0, l).to_rgba(255);
            let indice = 1 + 8 * fila + k;
            assert_eq!(
                palette.colors()[indice],
                esperado,
                "entrada {indice} (fila {fila}, k {k})"
            );
        }
    }
    for j in 0..7u32 {
        let g = ((255 * j + 3) / 6) as u8;
        assert_eq!(
            palette.colors()[25 + j as usize],
            Rgba::opaque(g, g, g),
            "gris {j}"
        );
    }
}

#[test]
fn fila_5_la_imagen_creada_tiene_el_tamano_y_el_modo_pedidos_y_todos_los_pixeles_del_fondo() {
    for (width, height) in [(5u32, 3u32), (64, 64)] {
        for mode in MODOS {
            for fondo in fondos_validos(mode) {
                let spec = spec_nueva(width, height, mode, fondo);
                let image = construye_real(&spec);
                let esperado = background_pixel(mode, fondo).expect("fondo válido");
                assert_eq!((image.width(), image.height()), (width, height));
                assert_eq!(image.mode(), mode);
                for y in 0..height {
                    for x in 0..width {
                        assert_eq!(
                            image.get_pixel(x, y),
                            Ok(esperado),
                            "{width}×{height} {mode:?} {fondo:?} en ({x}, {y})"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn fila_5_en_la_imagen_grande_valen_el_fondo_las_cuatro_esquinas_y_el_centro() {
    let (width, height) = (1000u32, 600u32);
    for mode in MODOS {
        for fondo in fondos_validos(mode) {
            let spec = spec_nueva(width, height, mode, fondo);
            let image = construye_real(&spec);
            let esperado = background_pixel(mode, fondo).expect("fondo válido");
            assert_eq!((image.width(), image.height()), (width, height));
            assert_eq!(image.mode(), mode);
            for (x, y) in [
                (0, 0),
                (width - 1, 0),
                (0, height - 1),
                (width - 1, height - 1),
                (width / 2, height / 2),
            ] {
                assert_eq!(
                    image.get_pixel(x, y),
                    Ok(esperado),
                    "{mode:?} {fondo:?} en ({x}, {y})"
                );
            }
        }
    }
}

#[test]
fn fila_5_el_indexado_lleva_la_paleta_de_d2_con_blanco_negro_y_transparente_en_sus_indices() {
    for fondo in [TRANSPARENTE, BLANCO, NEGRO] {
        let image = construye_real(&spec_nueva(7, 5, Mode::Indexed, fondo));
        assert_eq!(image.palette(), Some(&initial_palette()), "{fondo:?}");
        let indice = match fondo {
            Background::Transparent => 0,
            Background::White => 31,
            Background::Black => 25,
            Background::Color(_) => unreachable!(),
        };
        assert_eq!(
            image.get_pixel(3, 2),
            Ok(Pixel::Indexed(indice)),
            "{fondo:?}"
        );
        let palette = image.palette().expect("paleta");
        let visible = palette.resolved(indice).expect("índice");
        let esperado = match fondo {
            Background::Transparent => Rgba::new(0, 0, 0, 0),
            Background::White => Rgba::opaque(255, 255, 255),
            _ => Rgba::opaque(0, 0, 0),
        };
        assert_eq!(visible, esperado, "{fondo:?}");
    }
    // Los demás modos no llevan paleta.
    for mode in [Mode::Rgba, Mode::GrayAlpha, Mode::Gray] {
        let image = construye_real(&spec_nueva(2, 2, mode, default_background(mode)));
        assert!(image.palette().is_none(), "{mode:?}");
    }
}

#[test]
fn fila_5_envuelta_en_document_la_imagen_no_tiene_ruta_ni_historia() {
    // Antes (unidades 19 a 14a-1) se afirmaba también `document == canvas`: la segunda copia, la
    // del lienzo, desaparece (decisión 10 de la 14a-2).
    for mode in MODOS {
        let image = construye_real(&spec_nueva(9, 4, mode, default_background(mode)));
        let document = Document::new(image);
        assert!(!document.can_undo(), "{mode:?}");
        assert!(!document.can_redo(), "{mode:?}");
        assert_eq!(document.state_id(), 0, "{mode:?}");
        let session = Session::sample(document);
        assert!(session.path().is_none(), "{mode:?}");
        assert!(!session.modified(), "{mode:?}");
    }
}

// ---------------------------------------------------------------------------------------
// Fila 6: memoria.
// ---------------------------------------------------------------------------------------

const MIB: u64 = 1 << 20;

fn bpp(mode: Mode) -> u64 {
    match mode {
        Mode::Indexed | Mode::Gray => 1,
        Mode::GrayAlpha => 2,
        Mode::Rgba => 4,
    }
}

fn memoria(width: u32, height: u32, mode: Mode) -> (u64, u64) {
    let m = memory_estimate(width, height, mode);
    (m.ram_bytes, m.gpu_bytes)
}

#[test]
fn fila_6_memory_estimate_es_n_por_bpp_mas_4_en_ram_y_4_n_en_gpu() {
    for (width, height) in [
        (1u32, 1u32),
        (64, 64),
        (256, 256),
        (1024, 1024),
        (4096, 4096),
        (4097, 4096),
        (8192, 8192),
        (8192, 1),
        (3, 7),
    ] {
        for mode in MODOS {
            let n = u64::from(width) * u64::from(height);
            assert_eq!(
                memoria(width, height, mode),
                (n * (bpp(mode) + 4), 4 * n),
                "{width}×{height} {mode:?}"
            );
        }
    }
}

#[test]
fn fila_6_memory_estimate_da_las_cifras_de_la_spec() {
    // RAM = N · (bytes por píxel + 4) desde la 14a-2 (antes N · (2 · bytes por píxel + 4)).
    assert_eq!(memoria(1024, 1024, Mode::Rgba), (8 * MIB, 4 * MIB));
    assert_eq!(memoria(4096, 4096, Mode::Rgba), (128 * MIB, 64 * MIB));
    assert_eq!(memoria(8192, 8192, Mode::Rgba), (512 * MIB, 256 * MIB));
    assert_eq!(memoria(8192, 8192, Mode::GrayAlpha), (384 * MIB, 256 * MIB));
    assert_eq!(memoria(8192, 8192, Mode::Gray), (320 * MIB, 256 * MIB));
    assert_eq!(memoria(8192, 8192, Mode::Indexed), (320 * MIB, 256 * MIB));
    assert_eq!(memoria(4097, 4096, Mode::Rgba), (134_250_496, 67_125_248));
}

const AVISO_DE_TAMANO: &str = "aviso: más de 16777216 px; puede ir lento";
const LINEA_DE_PALETA: &str = "Paleta: 32 colores, el 0 transparente";

fn informe(size: Option<(u32, u32)>, mode: Mode, notice: Option<&str>) -> String {
    report_text(size, mode, notice)
}

#[test]
fn fila_6_report_text_menos_de_1_mib_en_64_y_256_al_cuadrado() {
    assert_eq!(
        informe(Some((64, 64)), Mode::Rgba, None),
        "Memoria: menos de 1 MiB"
    );
    // 256² RGBA son 0,5 MiB de RAM: todavía menos de 1.
    assert_eq!(
        informe(Some((256, 256)), Mode::Rgba, None),
        "Memoria: menos de 1 MiB"
    );
    assert_eq!(
        informe(Some((64, 64)), Mode::Indexed, None),
        format!("Memoria: menos de 1 MiB\n{LINEA_DE_PALETA}")
    );
}

#[test]
fn fila_6_report_text_da_los_textos_exactos_de_d4() {
    assert_eq!(
        informe(Some((1024, 1024)), Mode::Rgba, None),
        "Memoria: ~8 MiB + 4 MiB de GPU"
    );
    assert_eq!(
        informe(Some((4096, 4096)), Mode::Rgba, None),
        "Memoria: ~128 MiB + 64 MiB de GPU",
        "4096² no avisa"
    );
    assert_eq!(
        informe(Some((8192, 8192)), Mode::Rgba, None),
        format!("Memoria: ~512 MiB + 256 MiB de GPU\n{AVISO_DE_TAMANO}")
    );
    assert_eq!(
        informe(Some((8192, 8192)), Mode::GrayAlpha, None),
        format!("Memoria: ~384 MiB + 256 MiB de GPU\n{AVISO_DE_TAMANO}")
    );
    assert_eq!(
        informe(Some((8192, 8192)), Mode::Gray, None),
        format!("Memoria: ~320 MiB + 256 MiB de GPU\n{AVISO_DE_TAMANO}")
    );
    assert_eq!(
        informe(Some((8192, 8192)), Mode::Indexed, None),
        format!("Memoria: ~320 MiB + 256 MiB de GPU\n{LINEA_DE_PALETA}\n{AVISO_DE_TAMANO}")
    );
}

#[test]
fn fila_6_el_redondeo_es_hacia_arriba() {
    // 4097 × 4096 RGBA: 128,01 MiB de RAM y 64,004 de GPU.
    assert_eq!(
        informe(Some((4097, 4096)), Mode::Rgba, None),
        format!("Memoria: ~129 MiB + 65 MiB de GPU\n{AVISO_DE_TAMANO}")
    );
}

#[test]
fn fila_6_con_ancho_o_alto_sin_valor_dice_memoria_raya_y_no_avisa_del_tamano() {
    assert_eq!(informe(None, Mode::Rgba, None), "Memoria: —");
    assert_eq!(
        informe(None, Mode::Indexed, None),
        format!("Memoria: —\n{LINEA_DE_PALETA}")
    );
    assert!(!informe(None, Mode::Gray, None).contains("aviso"));
}

#[test]
fn fila_6_el_aviso_de_tamano_solo_aparece_con_mas_de_2_a_la_24_pixeles() {
    assert_eq!(WARN_PIXELS, 16_777_216);
    for ((width, height), avisa) in [
        ((4096u32, 4096u32), false),
        ((8192, 2048), false),
        ((4097, 4096), true),
        ((8192, 2049), true),
        ((8192, 8192), true),
        ((1, 1), false),
    ] {
        for mode in MODOS {
            let texto = informe(Some((width, height)), mode, None);
            assert_eq!(
                texto.lines().any(|l| l == AVISO_DE_TAMANO),
                avisa,
                "{width}×{height} {mode:?}: {texto}"
            );
        }
    }
}

#[test]
fn fila_6_el_aviso_de_tamano_no_impide_crear() {
    let mut d = dialogo(8192);
    numero(&mut d, "ancho", "4097");
    numero(&mut d, "alto", "4096");
    assert!(d.is_valid(), "{:?}", d.reason());
    let spec = spec_de(&d).expect("se puede crear");
    assert_eq!((spec.width, spec.height), (4097, 4096));
    assert_eq!(size_error(spec.width, spec.height, spec.mode), None);
    // Y el formulario lo acepta con `Crear` (Accept).
    assert!(matches!(d.event(Event::Accept), Reaction::Accepted(_)));
}

#[test]
fn fila_6_con_aviso_de_fondo_el_informe_lo_pone_el_ultimo() {
    let texto = informe(
        Some((4097, 4096)),
        Mode::Indexed,
        Some(AVISO_INDEXADO_SIN_COLOR),
    );
    let lineas: Vec<&str> = texto.lines().collect();
    assert_eq!(lineas.len(), 4, "{texto}");
    assert!(lineas[0].starts_with("Memoria: "));
    assert_eq!(lineas[1], LINEA_DE_PALETA);
    assert_eq!(lineas[2], AVISO_DE_TAMANO);
    assert_eq!(lineas[3], AVISO_INDEXADO_SIN_COLOR);

    // Sin la línea de la paleta ni el aviso de tamaño, el aviso de fondo va tras la memoria.
    assert_eq!(
        informe(
            Some((64, 64)),
            Mode::Rgba,
            Some(AVISO_GRIS_SIN_TRANSPARENTE)
        ),
        format!("Memoria: menos de 1 MiB\n{AVISO_GRIS_SIN_TRANSPARENTE}")
    );
    assert_eq!(
        informe(None, Mode::Gray, Some(AVISO_COLOR_CON_ALFA)),
        format!("Memoria: —\n{AVISO_COLOR_CON_ALFA}")
    );
}

#[test]
fn fila_6_el_informe_tiene_como_mucho_4_lineas_de_42_caracteres_dentro_del_juego() {
    let tamanos = [
        None,
        Some((1u32, 1u32)),
        Some((64, 64)),
        Some((256, 256)),
        Some((1024, 1024)),
        Some((4096, 4096)),
        Some((4097, 4096)),
        Some((8192, 8192)),
    ];
    let mut avisos: Vec<Option<&str>> = vec![None];
    avisos.extend(AVISOS.iter().map(|a| Some(*a)));
    for size in tamanos {
        for mode in MODOS {
            for aviso in &avisos {
                let texto = informe(size, mode, *aviso);
                let lineas: Vec<&str> = texto.lines().collect();
                assert!(lineas.len() <= 4, "{size:?} {mode:?} {aviso:?}: {texto}");
                for linea in &lineas {
                    assert!(
                        linea.chars().count() <= 42,
                        "{size:?} {mode:?} {aviso:?}: {linea:?}"
                    );
                }
                assert!(
                    lineas.iter().all(|l| en_el_juego(l)),
                    "{size:?} {mode:?} {aviso:?}: {texto}"
                );
                // El orden de D11: memoria, paleta (solo indexado), tamaño, fondo.
                assert!(lineas[0].starts_with("Memoria: "), "{texto}");
                let paleta = lineas.iter().position(|l| *l == LINEA_DE_PALETA);
                assert_eq!(paleta.is_some(), mode == Mode::Indexed, "{texto}");
                if let Some(p) = paleta {
                    assert_eq!(p, 1, "{texto}");
                }
                if let Some(t) = lineas.iter().position(|l| *l == AVISO_DE_TAMANO) {
                    assert_eq!(t, if mode == Mode::Indexed { 2 } else { 1 }, "{texto}");
                }
                if let Some(a) = aviso {
                    assert_eq!(lineas.last().copied(), Some(*a), "{texto}");
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------------------
// Fila 7: reserva y fallo.
// ---------------------------------------------------------------------------------------

/// Lo que el falso ve, en el orden en que ocurre.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Llamada {
    Imagen,
    Probe(usize),
}

/// Dónde falla el falso.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Fallo {
    Ninguno,
    /// Falla la reserva de la imagen del documento.
    Imagen,
    /// Falla la reserva de la subida a baldosas (`probe`). Desde la 14a-2 ya no hay una segunda
    /// imagen (la copia del lienzo): quedan estos dos pasos.
    Probe,
}

const FALLOS: [Fallo; 2] = [Fallo::Imagen, Fallo::Probe];

/// Un `ImageAlloc` que cuenta las llamadas, delega en el real y falla donde se le dice.
struct Falso {
    fallo: Fallo,
    llamadas: Vec<Llamada>,
}

impl Falso {
    fn nuevo(fallo: Fallo) -> Falso {
        Falso {
            fallo,
            llamadas: Vec::new(),
        }
    }
}

impl ImageAlloc for Falso {
    fn image(&mut self, spec: &NewImageSpec) -> Result<Image, ImageError> {
        self.llamadas.push(Llamada::Imagen);
        if self.fallo == Fallo::Imagen {
            return Err(ImageError::TooLarge {
                width: spec.width,
                height: spec.height,
            });
        }
        SystemAlloc.image(spec)
    }

    fn probe(&mut self, bytes: usize) -> bool {
        self.llamadas.push(Llamada::Probe(bytes));
        self.fallo != Fallo::Probe
    }
}

fn llamadas_esperadas(fallo: Fallo, bytes_del_probe: usize) -> Vec<Llamada> {
    match fallo {
        Fallo::Imagen => vec![Llamada::Imagen],
        Fallo::Probe | Fallo::Ninguno => vec![Llamada::Imagen, Llamada::Probe(bytes_del_probe)],
    }
}

#[test]
fn fila_7_sin_fallo_el_falso_ve_una_imagen_y_probe_de_4_n_bytes_y_da_ok() {
    for mode in MODOS {
        let spec = spec_nueva(100, 50, mode, default_background(mode));
        let mut falso = Falso::nuevo(Fallo::Ninguno);
        let image = construye(&spec, &mut falso).expect("sin fallo da Ok");
        assert_eq!(
            falso.llamadas,
            llamadas_esperadas(Fallo::Ninguno, 4 * 100 * 50),
            "{mode:?}"
        );
        assert_eq!((image.width(), image.height()), (100, 50));
    }
}

#[test]
fn fila_7_cada_fallo_da_err_memory_con_los_bytes_de_la_estimacion_y_no_se_llama_a_nada_mas() {
    for mode in MODOS {
        for fallo in FALLOS {
            let spec = spec_nueva(100, 50, mode, default_background(mode));
            let mut falso = Falso::nuevo(fallo);
            let resultado = build_new_image(&spec, &mut falso);
            match resultado {
                Err(NewImageError::Memory { bytes }) => assert_eq!(
                    bytes,
                    memory_estimate(100, 50, mode).ram_bytes,
                    "{mode:?} {fallo:?}"
                ),
                Err(NewImageError::Size(texto)) => panic!("{mode:?} {fallo:?}: Size({texto})"),
                Ok(_) => panic!("{mode:?} {fallo:?}: debía fallar y dio Ok (ninguna imagen)"),
            }
            assert_eq!(
                falso.llamadas,
                llamadas_esperadas(fallo, 4 * 100 * 50),
                "{mode:?} {fallo:?}: orden y nada más tras el fallo"
            );
        }
    }
}

#[test]
fn fila_7_el_probe_recibe_los_bytes_de_la_conversion_a_rgba_de_la_subida() {
    for (width, height) in [(1u32, 1u32), (7, 3), (300, 200)] {
        let spec = spec_nueva(width, height, Mode::Gray, default_background(Mode::Gray));
        let mut falso = Falso::nuevo(Fallo::Ninguno);
        construye(&spec, &mut falso).expect("Ok");
        let bytes = 4 * width as usize * height as usize;
        assert_eq!(falso.llamadas.last(), Some(&Llamada::Probe(bytes)));
    }
}

#[test]
fn fila_7_con_el_real_a_8192_por_8192_rgba_da_ok() {
    // Una imagen de 256 MiB y un `probe` de 256 MiB: lo más pesado de la unidad. Fondo
    // transparente (ceros): no hace falta rellenar.
    let spec = spec_nueva(8192, 8192, Mode::Rgba, TRANSPARENTE);
    let image = construye(&spec, &mut SystemAlloc).expect("con el real, Ok");
    assert_eq!((image.width(), image.height()), (8192, 8192));
    assert_eq!(
        image.get_pixel(8191, 8191),
        Ok(Pixel::Rgba(Rgba::TRANSPARENT))
    );
}

// --- Sesión y lienzo intactos tras un fallo ---

/// Un banco mínimo de egui para el lienzo (sin ventana ni GPU): ejecuta fotogramas y anota lo que
/// el lienzo dejó.
struct Lienzo {
    ctx: Context,
    canvas: Canvas,
}

const PANTALLA: eframe::egui::Vec2 = vec2(800.0, 600.0);

impl Lienzo {
    fn nuevo(imagen: Image) -> Lienzo {
        let mut lienzo = Lienzo {
            ctx: Context::default(),
            canvas: Canvas::new(),
        };
        lienzo.canvas.set_owned_image(imagen);
        lienzo.fotograma(vec![]);
        lienzo
    }

    /// Un fotograma: la información del lienzo, la textura y si se subió una en este fotograma.
    fn fotograma(&mut self, events: Vec<InputEvent>) -> Foto {
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, PANTALLA)),
            events,
            ..RawInput::default()
        };
        let mut info = None;
        let mut salida = self.ctx.run_ui(input, |ui| {
            info = Some(self.canvas.show(ui));
        });
        let textura = self.canvas.texture_id();
        let subio = textura.is_some_and(|id| salida.textures_delta.set.contains_key(&id));
        salida.textures_delta.clear();
        Foto {
            info: info.expect("el lienzo se dibuja"),
            textura,
            subio,
        }
    }

    /// La rueda sobre el centro: cambia el zoom (la vista deja de ser la de ajuste).
    fn aleja(&mut self) {
        let centro = pos2(400.0, 300.0);
        self.fotograma(vec![InputEvent::PointerMoved(centro)]);
        self.fotograma(vec![InputEvent::MouseWheel {
            unit: MouseWheelUnit::Line,
            delta: vec2(0.0, -1.0),
            phase: TouchPhase::Move,
            source: MouseWheelSource::Unknown,
            modifiers: Modifiers::NONE,
        }]);
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Foto {
    info: CanvasInfo,
    textura: Option<eframe::egui::TextureId>,
    subio: bool,
}

/// Todo lo observable de la sesión que un fallo no debe tocar.
#[derive(Clone, Debug, PartialEq)]
struct FotoDeSesion {
    image: Image,
    state_id: u64,
    can_undo: bool,
    can_redo: bool,
    path: Option<std::path::PathBuf>,
    modified: bool,
    choice: ColorChoice,
}

fn foto_de(session: &Session) -> FotoDeSesion {
    FotoDeSesion {
        image: session
            .document
            .image(ImageId::from_raw(0))
            .expect("imagen")
            .clone(),
        state_id: session.document.state_id(),
        can_undo: session.document.can_undo(),
        can_redo: session.document.can_redo(),
        path: session.path().map(Path::to_path_buf),
        modified: session.modified(),
        choice: session.choice,
    }
}

/// Una sesión con un documento RGBA 6×4 y, si `modificada`, un paso de historia sin guardar.
fn sesion(modificada: bool) -> Session {
    let image = Image::new(6, 4, Mode::Rgba).expect("imagen");
    let mut session = Session::sample(Document::new(image));
    if modificada {
        let id = ImageId::from_raw(0);
        session
            .document
            .apply(Command::SetPixels {
                image: id,
                pixels: vec![PixelEdit {
                    x: 1,
                    y: 1,
                    pixel: Pixel::Rgba(Rgba::opaque(9, 8, 7)),
                }],
            })
            .expect("editar");
        assert!(session.modified());
    }
    session
}

/// Por qué no se creó la imagen.
#[derive(Clone, Debug, PartialEq, Eq)]
enum NoSeCreo {
    Unsaved,
    Memoria(u64),
}

/// El recorrido de `App` (D8) sin egui: con cambios y sin el descarte recordado, `Unsaved` antes de
/// construir nada; si no, `build_new_image` y, solo si salió bien, `Session::new_image` y el
/// lienzo con la imagen del documento (`Canvas::set_image`; ya no hay una copia aparte, 14a-2).
fn crea(
    session: &mut Session,
    lienzo: &mut Lienzo,
    spec: &NewImageSpec,
    alloc: &mut dyn ImageAlloc,
    discard: bool,
) -> Result<(), NoSeCreo> {
    if session.modified() && !discard {
        return Err(NoSeCreo::Unsaved);
    }
    let image = match build_new_image(spec, alloc) {
        Ok(image) => image,
        Err(NewImageError::Memory { bytes }) => return Err(NoSeCreo::Memoria(bytes)),
        Err(NewImageError::Size(texto)) => panic!("Size({texto})"),
    };
    match session.new_image(image, discard) {
        Ok(()) => {
            let del_documento = session
                .document
                .image(ImageId::from_raw(0))
                .expect("imagen");
            lienzo.canvas.set_image(del_documento);
            Ok(())
        }
        Err(FileError::Unsaved) => Err(NoSeCreo::Unsaved),
        Err(otro) => panic!("{otro:?}"),
    }
}

#[test]
fn fila_7_un_fallo_deja_la_sesion_y_el_lienzo_iguales() {
    for fallo in FALLOS {
        let mut session = sesion(false);
        let mut lienzo = Lienzo::nuevo(
            session
                .document
                .image(ImageId::from_raw(0))
                .expect("imagen")
                .clone(),
        );
        lienzo.aleja();
        let antes_sesion = foto_de(&session);
        let antes_lienzo = lienzo.fotograma(vec![]);

        let spec = spec_nueva(30, 20, Mode::Indexed, NEGRO);
        let mut falso = Falso::nuevo(fallo);
        let resultado = crea(&mut session, &mut lienzo, &spec, &mut falso, false);
        assert_eq!(
            resultado,
            Err(NoSeCreo::Memoria(
                memory_estimate(30, 20, Mode::Indexed).ram_bytes
            )),
            "{fallo:?}"
        );

        assert_eq!(foto_de(&session), antes_sesion, "{fallo:?}: sesión");
        let despues_lienzo = lienzo.fotograma(vec![]);
        assert_eq!(despues_lienzo, antes_lienzo, "{fallo:?}: lienzo");
        assert!(
            !despues_lienzo.subio,
            "{fallo:?}: no se volvió a subir nada"
        );
    }
}

#[test]
fn fila_7_con_exito_se_reemplazan_sesion_y_lienzo_una_vez() {
    let mut session = sesion(false);
    let mut lienzo = Lienzo::nuevo(
        session
            .document
            .image(ImageId::from_raw(0))
            .expect("imagen")
            .clone(),
    );
    let antigua = lienzo.fotograma(vec![]).textura;
    let spec = spec_nueva(30, 20, Mode::Indexed, NEGRO);
    let mut falso = Falso::nuevo(Fallo::Ninguno);
    assert_eq!(
        crea(&mut session, &mut lienzo, &spec, &mut falso, false),
        Ok(())
    );
    assert_eq!(
        falso.llamadas.len(),
        2,
        "una imagen y un probe, una sola vez"
    );

    let image = session
        .document
        .image(ImageId::from_raw(0))
        .expect("imagen");
    assert_eq!(
        (image.width(), image.height(), image.mode()),
        (30, 20, Mode::Indexed)
    );
    let primera = lienzo.fotograma(vec![]);
    assert!(primera.subio, "el lienzo sube la imagen nueva");
    assert_ne!(primera.textura, antigua);
    assert!(!lienzo.fotograma(vec![]).subio, "una sola subida");
}

// ---------------------------------------------------------------------------------------
// Fila 8: el aviso de 5 s tras crear.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_8_describe_new_dice_tamano_modo_y_los_32_colores_solo_del_indexado() {
    assert_eq!(
        describe_new(&spec_nueva(64, 64, Mode::Indexed, TRANSPARENTE)),
        "imagen nueva 64×64 · indexado · 32 colores"
    );
    assert_eq!(
        describe_new(&spec_nueva(64, 64, Mode::Rgba, TRANSPARENTE)),
        "imagen nueva 64×64 · RGBA"
    );
    assert_eq!(
        describe_new(&spec_nueva(5, 3, Mode::GrayAlpha, NEGRO)),
        "imagen nueva 5×3 · gris con alfa"
    );
    assert_eq!(
        describe_new(&spec_nueva(8192, 4097, Mode::Gray, BLANCO)),
        "imagen nueva 8192×4097 · gris"
    );
    for mode in MODOS {
        let texto = describe_new(&spec_nueva(10, 20, mode, default_background(mode)));
        assert!(en_el_juego(&texto), "{texto}");
    }
}

// ---------------------------------------------------------------------------------------
// Propiedad 1: construir es exacto.
// ---------------------------------------------------------------------------------------

/// Los fondos válidos de un modo con el gris y el alfa dados (para el «Color…» de los que lo
/// admiten).
fn fondos_con_color(mode: Mode, color: [u8; 4]) -> Vec<Background> {
    let [r, g, b, a] = color;
    match mode {
        Mode::Rgba => vec![
            TRANSPARENTE,
            BLANCO,
            NEGRO,
            Background::Color(Rgba::new(r, g, b, a)),
        ],
        Mode::GrayAlpha => vec![
            TRANSPARENTE,
            BLANCO,
            NEGRO,
            Background::Color(Rgba::new(g, g, g, a)),
        ],
        Mode::Gray => vec![BLANCO, NEGRO, Background::Color(Rgba::opaque(g, g, g))],
        Mode::Indexed => vec![TRANSPARENTE, BLANCO, NEGRO],
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Para cualquier lado de 1 a 24, modo y fondo válido: `Ok`, la imagen mide lo pedido y cada
    /// píxel es `background_pixel`; con la paleta de D2 en indexado; y
    /// `memory_estimate` crece con el área.
    #[test]
    fn propiedad_1_construir_es_exacto(
        width in 1u32..=24,
        height in 1u32..=24,
        width2 in 1u32..=24,
        height2 in 1u32..=24,
        modo in 0usize..4,
        fondo in 0usize..8,
        color in any::<[u8; 4]>(),
    ) {
        let mode = MODOS[modo];
        let candidatos = fondos_con_color(mode, color);
        let fondo = candidatos[fondo % candidatos.len()];
        let spec = spec_nueva(width, height, mode, fondo);
        let image = construye(&spec, &mut SystemAlloc)
            .map_err(TestCaseError::fail)?;
        let pixel = background_pixel(mode, fondo);
        prop_assert!(pixel.is_some(), "{:?} {:?}: fondo no válido en la prueba", mode, fondo);
        let pixel = pixel.unwrap();
        prop_assert_eq!((image.width(), image.height()), (width, height));
        prop_assert_eq!(image.mode(), mode);
        for y in 0..height {
            for x in 0..width {
                prop_assert_eq!(image.get_pixel(x, y), Ok(pixel), "({}, {})", x, y);
            }
        }
        if mode == Mode::Indexed {
            prop_assert_eq!(image.palette(), Some(&initial_palette()));
        } else {
            prop_assert!(image.palette().is_none());
        }

        // `memory_estimate` crece con el área (y no decrece nunca).
        let a = memory_estimate(width, height, mode);
        let b = memory_estimate(width2, height2, mode);
        let area = u64::from(width) * u64::from(height);
        let area2 = u64::from(width2) * u64::from(height2);
        if area2 > area {
            prop_assert!(b.ram_bytes > a.ram_bytes);
            prop_assert!(b.gpu_bytes > a.gpu_bytes);
        } else if area2 == area {
            prop_assert_eq!((b.ram_bytes, b.gpu_bytes), (a.ram_bytes, a.gpu_bytes));
        } else {
            prop_assert!(b.ram_bytes < a.ram_bytes);
            prop_assert!(b.gpu_bytes < a.gpu_bytes);
        }
    }
}

// ---------------------------------------------------------------------------------------
// Propiedad 2: trasladar nunca pierde color en silencio (exhaustiva).
// ---------------------------------------------------------------------------------------

/// Los fondos que se recorren como origen: los de la fila 3 y unos cuantos colores más (grises con
/// todos los alfas relevantes, no grises, y el blanco y el negro como «Color…»).
fn fondos_de_origen() -> Vec<Background> {
    let mut fondos = vec![TRANSPARENTE, BLANCO, NEGRO];
    for color in [
        GRIS_OPACO,
        GRIS_MEDIO,
        ROJO,
        ROJO_MEDIO,
        Rgba::new(0x40, 0x40, 0x40, 0),
        Rgba::new(0, 0, 0, 0),
        Rgba::opaque(0, 0, 0),
        Rgba::opaque(255, 255, 255),
        Rgba::new(255, 255, 255, 254),
        Rgba::opaque(1, 2, 3),
    ] {
        fondos.push(Background::Color(color));
    }
    fondos
}

#[test]
fn propiedad_2_trasladar_nunca_pierde_color_en_silencio() {
    let mut comprobados = 0;
    for from in MODOS {
        for fondo in fondos_de_origen() {
            // Solo los fondos que existen en el modo de origen.
            let Some(visible) = color_visible(from, fondo) else {
                continue;
            };
            for to in MODOS {
                comprobados += 1;
                match carry_background(from, fondo, to) {
                    Carry::Kept(nuevo) => {
                        assert_eq!(
                            color_visible(to, nuevo),
                            Some(visible),
                            "{from:?} {fondo:?} → {to:?}: conservó otro color"
                        );
                        assert_eq!(nuevo, fondo, "{from:?} {fondo:?} → {to:?}");
                    }
                    Carry::Reset { to: nuevo, notice } => {
                        assert_eq!(
                            nuevo,
                            default_background(to),
                            "{from:?} {fondo:?} → {to:?}: se reinicia al de por defecto"
                        );
                        assert!(!notice.is_empty(), "{from:?} {fondo:?} → {to:?}: sin aviso");
                        assert!(
                            color_visible(to, nuevo).is_some(),
                            "{from:?} {fondo:?} → {to:?}: el reinicio no vale en el modo nuevo"
                        );
                    }
                }
            }
        }
    }
    assert!(
        comprobados > 100,
        "se recorrieron {comprobados} combinaciones"
    );
}

#[test]
fn propiedad_2_carry_background_es_total_tambien_con_origenes_que_el_modo_no_admite() {
    // No debe entrar en pánico con ninguna combinación, aunque el fondo no exista en el modo de
    // origen (por ejemplo Transparente en Gris o un Color en Indexado).
    for from in MODOS {
        for fondo in fondos_de_origen() {
            for to in MODOS {
                match carry_background(from, fondo, to) {
                    Carry::Kept(_) => {}
                    Carry::Reset { notice, .. } => assert!(!notice.is_empty()),
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------------------
// Propiedad 3: el estado del formulario no se corrompe.
// ---------------------------------------------------------------------------------------

/// Un evento del usuario sobre el formulario, en abstracto: se resuelve contra el modo actual
/// (como la interfaz, que solo envía eventos a lo que se ve).
#[derive(Clone, Debug)]
enum Gesto {
    Ancho(usize),
    Alto(usize),
    PasoAncho(bool),
    PasoAlto(bool),
    Modo(usize),
    Fondo(usize),
    Hex(usize),
    Confirma,
}

const TEXTOS_NUMERICOS: [&str; 11] = [
    "0",
    "1",
    "64",
    "8192",
    "8193",
    "",
    "abc",
    "4097",
    "4096",
    "12345678901",
    "007",
];

const TEXTOS_HEX: [&str; 10] = [
    "#12",
    "zz",
    "#ff0000",
    "#808080",
    "#80808080",
    "#ff000080",
    "#fff",
    "",
    "#12345",
    "#00ff00ff",
];

fn gesto() -> impl Strategy<Value = Gesto> {
    prop_oneof![
        (0..TEXTOS_NUMERICOS.len()).prop_map(Gesto::Ancho),
        (0..TEXTOS_NUMERICOS.len()).prop_map(Gesto::Alto),
        any::<bool>().prop_map(Gesto::PasoAncho),
        any::<bool>().prop_map(Gesto::PasoAlto),
        (0usize..4).prop_map(Gesto::Modo),
        (0usize..4).prop_map(Gesto::Fondo),
        (0..TEXTOS_HEX.len()).prop_map(Gesto::Hex),
        Just(Gesto::Confirma),
    ]
}

/// El diálogo sin su informe, para saber si un evento cambió algo del formulario.
fn sin_informe(d: &Dialog) -> Dialog {
    let mut copia = d.clone();
    assert!(copia.set_report("informe", String::new()));
    copia
}

fn opciones_del_fondo(mode: Mode) -> usize {
    match mode {
        Mode::Indexed | Mode::Gray => 3,
        Mode::Rgba | Mode::GrayAlpha => 4,
    }
}

/// Aplica un gesto al diálogo y devuelve el modo en que queda (el modelo de la prueba).
fn aplica(d: &mut Dialog, modo: usize, gesto: &Gesto) -> usize {
    let mode = MODOS[modo];
    match gesto {
        Gesto::Ancho(i) => {
            d.event(Event::Number {
                field: "ancho",
                text: TEXTOS_NUMERICOS[*i].to_string(),
            });
        }
        Gesto::Alto(i) => {
            d.event(Event::Number {
                field: "alto",
                text: TEXTOS_NUMERICOS[*i].to_string(),
            });
        }
        Gesto::PasoAncho(mas) => {
            d.event(Event::Step {
                field: "ancho",
                delta: if *mas { 1 } else { -1 },
            });
        }
        Gesto::PasoAlto(mas) => {
            d.event(Event::Step {
                field: "alto",
                delta: if *mas { 1 } else { -1 },
            });
        }
        Gesto::Modo(i) => {
            d.event(Event::Choice {
                field: "modo",
                index: *i,
            });
            return *i;
        }
        Gesto::Fondo(i) => {
            d.event(Event::Choice {
                field: id_fondo(mode),
                index: i % opciones_del_fondo(mode),
            });
        }
        Gesto::Hex(i) => {
            if let Some(id) = id_color(mode).filter(|&id| d.is_shown(id)) {
                d.event(Event::Hex {
                    field: id,
                    text: TEXTOS_HEX[*i].to_string(),
                });
            }
        }
        Gesto::Confirma => {
            if let Some(id) = id_color(mode).filter(|&id| d.is_shown(id)) {
                d.event(Event::HexConfirm {
                    field: id,
                    blur: false,
                });
            }
        }
    }
    modo
}

fn tiene_aviso_de_fondo(d: &Dialog) -> bool {
    let texto = texto_del_informe(d);
    AVISOS.iter().any(|a| texto.contains(*a))
}

/// Las invariantes de la propiedad 3 tras un gesto y su `sync_form`.
fn comprueba_formulario(d: &Dialog, modo: usize, ctx: &str) -> Result<(), TestCaseError> {
    let mode = MODOS[modo];
    // Solo es visible el grupo de fondo del modo actual; un hexadecimal, solo con su modo.
    for otro in MODOS {
        prop_assert_eq!(
            d.is_shown(id_fondo(otro)),
            otro == mode,
            "{}: grupo de fondo de {:?}",
            ctx,
            otro
        );
        if let Some(id) = id_color(otro) {
            prop_assert!(
                !d.is_shown(id) || otro == mode,
                "{}: el color de {:?} se ve con el modo {:?}",
                ctx,
                otro,
                mode
            );
        }
    }
    // Lo oculto no sale en los valores; si hay valores, hay especificación y el tamaño vale.
    if let Some(values) = d.values() {
        for otro in MODOS.into_iter().filter(|m| *m != mode) {
            prop_assert!(
                values.get(id_fondo(otro)).is_none(),
                "{}: {:?} sale en los valores",
                ctx,
                otro
            );
            if let Some(id) = id_color(otro) {
                prop_assert!(
                    values.get(id).is_none(),
                    "{}: {} sale en los valores",
                    ctx,
                    id
                );
            }
        }
        let spec = spec_from_values(&values);
        prop_assert!(
            spec.is_some(),
            "{}: valores válidos sin especificación",
            ctx
        );
        let spec = spec.unwrap();
        prop_assert_eq!(spec.mode, mode, "{}", ctx);
        prop_assert!(
            background_pixel(spec.mode, spec.background).is_some(),
            "{}: fondo que el modo no admite",
            ctx
        );
        prop_assert_eq!(
            size_error(spec.width, spec.height, spec.mode),
            None,
            "{}",
            ctx
        );
    }
    // El informe: ≤ 4 líneas de ≤ 42 caracteres, dentro del juego y con la memoria primero.
    let texto = texto_del_informe(d);
    let lineas: Vec<&str> = texto.lines().collect();
    prop_assert!(
        !lineas.is_empty() && lineas.len() <= 4,
        "{}: {:?}",
        ctx,
        texto
    );
    for linea in &lineas {
        prop_assert!(linea.chars().count() <= 42, "{}: {:?}", ctx, linea);
    }
    prop_assert!(
        lineas.iter().all(|l| en_el_juego(l)),
        "{}: {:?}",
        ctx,
        texto
    );
    prop_assert!(lineas[0].starts_with("Memoria:"), "{}: {:?}", ctx, texto);
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Para secuencias de 0 a 48 eventos con `sync_form` tras cada uno: sin pánico, solo es visible
    /// el grupo de fondo del modo actual, los fondos ocultos no salen en `Values`, `values()` es
    /// `Some` ⇒ `spec_from_values` da `Some` y `size_error` da `None`, el informe cabe y el aviso de
    /// D1b no sobrevive a un cambio de otro campo.
    #[test]
    fn propiedad_3_el_estado_del_formulario_no_se_corrompe(
        gestos in prop::collection::vec(gesto(), 0..=48),
    ) {
        let mut d = dialogo(8192);
        let mut sync = FormSync::default();
        sync_form(&mut d, &mut sync);
        let mut modo = 0usize; // Indexado al abrir
        comprueba_formulario(&d, modo, "al abrir")?;

        for (i, g) in gestos.iter().enumerate() {
            let ctx = format!("paso {i}: {g:?}");
            let antes = sin_informe(&d);
            let modo_anterior = modo;
            modo = aplica(&mut d, modo, g);
            let cambio = sin_informe(&d) != antes;
            sync_form(&mut d, &mut sync);

            comprueba_formulario(&d, modo, &ctx)?;
            // Una segunda sincronización sin eventos no tiene nada que pintar.
            prop_assert!(!sync_form(&mut d, &mut sync), "{}: sync_form no es estable", ctx);
            // El aviso de D1b solo puede salir de un cambio de modo.
            if cambio && modo == modo_anterior {
                prop_assert!(!tiene_aviso_de_fondo(&d), "{}: el aviso sobrevive", ctx);
            }
        }
    }
}

// ---------------------------------------------------------------------------------------
// Propiedad 4: fallar no cambia nada (exhaustiva).
// ---------------------------------------------------------------------------------------

#[test]
fn propiedad_4_fallar_no_cambia_nada() {
    for mode in MODOS {
        let spec = spec_nueva(16, 8, mode, default_background(mode));
        for modificada in [false, true] {
            // Con cambios sin guardar, el descarte ya confirmado (el recorrido llega a construir).
            let discard = modificada;
            for fallo in FALLOS {
                let mut session = sesion(modificada);
                let mut lienzo = Lienzo::nuevo(
                    session
                        .document
                        .image(ImageId::from_raw(0))
                        .expect("imagen")
                        .clone(),
                );
                lienzo.aleja();
                let antes_sesion = foto_de(&session);
                let antes_lienzo = lienzo.fotograma(vec![]);

                let mut falso = Falso::nuevo(fallo);
                let resultado = crea(&mut session, &mut lienzo, &spec, &mut falso, discard);

                let contexto = format!("{mode:?}, {fallo:?}, con cambios {modificada}");
                assert_eq!(
                    resultado,
                    Err(NoSeCreo::Memoria(memory_estimate(16, 8, mode).ram_bytes)),
                    "{contexto}"
                );
                assert_eq!(foto_de(&session), antes_sesion, "{contexto}: sesión");
                let despues = lienzo.fotograma(vec![]);
                assert_eq!(despues, antes_lienzo, "{contexto}: lienzo");
                assert!(!despues.subio, "{contexto}: el lienzo no subió nada");
            }

            // Con éxito se reemplazan exactamente una vez.
            let mut session = sesion(modificada);
            let mut lienzo = Lienzo::nuevo(
                session
                    .document
                    .image(ImageId::from_raw(0))
                    .expect("imagen")
                    .clone(),
            );
            let antigua = lienzo.fotograma(vec![]).textura;
            let mut falso = Falso::nuevo(Fallo::Ninguno);
            let contexto = format!("{mode:?}, éxito, con cambios {modificada}");
            assert_eq!(
                crea(&mut session, &mut lienzo, &spec, &mut falso, discard),
                Ok(()),
                "{contexto}"
            );
            assert_eq!(falso.llamadas.len(), 2, "{contexto}: se construyó una vez");
            let image = session
                .document
                .image(ImageId::from_raw(0))
                .expect("imagen");
            assert_eq!(
                (image.width(), image.height(), image.mode()),
                (16, 8, mode),
                "{contexto}"
            );
            assert!(!session.modified(), "{contexto}");
            assert!(!session.document.can_undo(), "{contexto}");
            assert!(session.path().is_none(), "{contexto}");
            let primera = lienzo.fotograma(vec![]);
            assert!(primera.subio, "{contexto}: el lienzo sube la nueva");
            assert_ne!(primera.textura, antigua, "{contexto}");
            assert!(!lienzo.fotograma(vec![]).subio, "{contexto}: una sola vez");
        }

        // Con `Unsaved` no se construye nada y todo queda igual.
        let mut session = sesion(true);
        let mut lienzo = Lienzo::nuevo(
            session
                .document
                .image(ImageId::from_raw(0))
                .expect("imagen")
                .clone(),
        );
        lienzo.aleja();
        let antes_sesion = foto_de(&session);
        let antes_lienzo = lienzo.fotograma(vec![]);
        let mut falso = Falso::nuevo(Fallo::Ninguno);
        assert_eq!(
            crea(&mut session, &mut lienzo, &spec, &mut falso, false),
            Err(NoSeCreo::Unsaved),
            "{mode:?}"
        );
        assert!(
            falso.llamadas.is_empty(),
            "{mode:?}: con Unsaved no se construye nada"
        );
        assert_eq!(foto_de(&session), antes_sesion, "{mode:?}: sesión");
        assert_eq!(lienzo.fotograma(vec![]), antes_lienzo, "{mode:?}: lienzo");
    }
}

#[test]
fn propiedad_4_session_new_image_con_unsaved_no_cambia_la_sesion_ni_el_lienzo() {
    // La misma garantía vista desde `Session::new_image`: aunque la imagen ya esté construida, con
    // cambios sin guardar y sin descarte la sesión no cambia.
    for mode in MODOS {
        let mut session = sesion(true);
        let antes = foto_de(&session);
        let image = construye_real(&spec_nueva(16, 8, mode, default_background(mode)));
        assert_eq!(
            session.new_image(image, false),
            Err(FileError::Unsaved),
            "{mode:?}"
        );
        assert_eq!(foto_de(&session), antes, "{mode:?}");
    }
}

// ---------------------------------------------------------------------------------------
// Apéndice A1 (specs/hito-2-nueva-imagen.md): hexadecimal inválido al cambiar de modo.
// Filas A1, A2, A5, A6 y A7 y propiedades 1, 2 y 3 (A3 y A4 son de banco y van en
// `dialog_tests.rs`). En ROJO hasta la implementación (`Dialog::hex_committed`, `resolve_hex` y el
// aviso nuevo de `sync_form`).
//
// Decisiones donde el apéndice no fija un detalle:
// - El aviso nuevo es `Color no válido: pasa a #rrggbbaa` con **ocho dígitos siempre** (33
//   caracteres, A1-1 y A6), también con alfa `ff`: `hex8`. El campo, en cambio, muestra el color
//   con `hex_text` (`Rgba: Display`: seis dígitos si es opaco), como en toda la unidad; el apéndice
//   escribe `#808080ff` para las dos cosas. Si la implementación usa `hex_text` en el aviso, solo
//   cambia `hex8`.
// - El camino (a) de la spec (el clic quita el foco al pulsar y el campo se revierte antes del
//   fotograma del cambio) se modela con `HexConfirm { blur: true }` y su `sync_form`; el camino (b)
//   (`Intro` con el texto inválido, que se queda), con `HexConfirm { blur: false }`. En los dos
//   hay `sync_form` de reposo antes y después, como los fotogramas reales.
// - «Líneas de aviso» del informe son las que no empiezan por `Memoria:`, `Paleta:` ni `aviso:`.
// - El aviso de 5 s del título no se puede observar aquí: `sync_form` no recibe ningún medio de
//   avisar. Se fija la firma (A7), que es lo que garantiza que el descarte no lance ninguno.
// ---------------------------------------------------------------------------------------

use px_ui::color_edit::{hex_text, parse_hex};
use px_ui::dialog::resolve_hex;

const ORIGENES: [Mode; 3] = [Mode::Rgba, Mode::GrayAlpha, Mode::Gray];
const NO_VALIDO: &str = "Color no válido: pasa a ";
const GRIS_CLARO: Rgba = Rgba::opaque(0xaa, 0xaa, 0xaa);
const AZUL_OPACO: Rgba = Rgba::opaque(0x33, 0x66, 0x99);

/// `#rrggbbaa` siempre con ocho dígitos (la forma del aviso nuevo).
fn hex8(c: Rgba) -> String {
    format!("#{:02x}{:02x}{:02x}{:02x}", c.r, c.g, c.b, c.a)
}

fn aviso_descartado(c: Rgba) -> String {
    format!("{NO_VALIDO}{}", hex8(c))
}

/// Los últimos confirmados que admite cada modo de origen (A2).
fn confirmados_de(from: Mode) -> Vec<Rgba> {
    match from {
        Mode::Rgba => vec![GRIS_CLARO, GRIS_MEDIO, AZUL_OPACO],
        Mode::GrayAlpha => vec![GRIS_CLARO, GRIS_MEDIO],
        Mode::Gray => vec![GRIS_CLARO],
        Mode::Indexed => vec![],
    }
}

/// Textos que no valen en `from` (`#ff0000` solo vale en RGBA).
fn invalidos_de(from: Mode) -> Vec<&'static str> {
    let mut textos = vec!["zz", "#12"];
    if from != Mode::Rgba {
        textos.push("#ff0000");
    }
    for texto in &textos {
        assert!(
            parse_hex(texto, from, GRIS_OPACO).is_err(),
            "{from:?} {texto}"
        );
    }
    textos
}

/// Un color que `from` admite, a partir de cuatro bytes cualesquiera.
fn confirmable(from: Mode, color: [u8; 4]) -> Rgba {
    let [r, g, b, a] = color;
    match from {
        Mode::Rgba => Rgba::new(r, g, b, a),
        Mode::GrayAlpha => Rgba::new(g, g, g, a),
        Mode::Gray => Rgba::opaque(g, g, g),
        Mode::Indexed => panic!("Indexado no tiene «Color…»"),
    }
}

fn lineas_de_aviso(d: &Dialog) -> Vec<String> {
    lineas_del_informe(d)
        .into_iter()
        .filter(|l| {
            !l.starts_with("Memoria:") && !l.starts_with("Paleta:") && !l.starts_with("aviso:")
        })
        .collect()
}

/// Un diálogo en `from` con «Color…» elegido, `confirmado` (si lo hay) confirmado y `texto` (si lo
/// hay) escrito sin confirmar; un evento y un `sync_form` por paso, como el usuario.
fn origen(from: Mode, confirmado: Option<Rgba>, texto: Option<&str>) -> (Dialog, FormSync) {
    let mut d = dialogo(8192);
    let mut sync = FormSync::default();
    sync_form(&mut d, &mut sync);
    paso(
        &mut d,
        &mut sync,
        Event::Choice {
            field: "modo",
            index: indice_de_modo(from),
        },
    );
    let id = id_color(from).expect("el modo tiene «Color…»");
    paso(
        &mut d,
        &mut sync,
        Event::Choice {
            field: id_fondo(from),
            index: indice_de_fondo(from, Tipo::Color).expect("«Color…»"),
        },
    );
    if let Some(c) = confirmado {
        paso(
            &mut d,
            &mut sync,
            Event::Hex {
                field: id,
                text: hex8(c),
            },
        );
        paso(
            &mut d,
            &mut sync,
            Event::HexConfirm {
                field: id,
                blur: false,
            },
        );
        assert_eq!(d.hex_committed(id), Some(c), "{from:?}: confirmado");
    }
    if let Some(texto) = texto {
        paso(
            &mut d,
            &mut sync,
            Event::Hex {
                field: id,
                text: texto.to_string(),
            },
        );
        assert!(d.hex_pending(id));
    }
    assert!(d.is_shown(id), "{from:?}: el hexadecimal se ve");
    assert!(
        lineas_de_aviso(&d).is_empty(),
        "{from:?}: la preparación no deja avisos; hay {:?}",
        lineas_de_aviso(&d)
    );
    (d, sync)
}

fn reposo(d: &mut Dialog, sync: &mut FormSync) {
    for _ in 0..2 {
        sync_form(d, sync);
    }
}

/// Los dos caminos de la spec hasta el fotograma del cambio de modo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Camino {
    /// (a) escribir y hacer clic en el modo: el clic quita el foco al pulsar y el campo se revierte.
    Clic,
    /// (b) `Intro` con el texto inválido (se queda) y luego el clic.
    Intro,
}

const CAMINOS: [Camino; 2] = [Camino::Clic, Camino::Intro];

fn hasta_el_cambio(d: &mut Dialog, sync: &mut FormSync, from: Mode, camino: Camino) {
    let id = id_color(from).expect("color");
    reposo(d, sync);
    match camino {
        Camino::Clic => {
            paso(
                d,
                sync,
                Event::HexConfirm {
                    field: id,
                    blur: true,
                },
            );
            assert!(!d.hex_pending(id), "el clic revierte el campo");
        }
        Camino::Intro => {
            paso(
                d,
                sync,
                Event::HexConfirm {
                    field: id,
                    blur: false,
                },
            );
            assert!(d.hex_pending(id), "Intro con un inválido lo deja pendiente");
        }
    }
    reposo(d, sync);
}

fn cambia_de_modo(d: &mut Dialog, sync: &mut FormSync, to: Mode) {
    paso(
        d,
        sync,
        Event::Choice {
            field: "modo",
            index: indice_de_modo(to),
        },
    );
}

/// El fondo y el aviso al trasladar un fondo válido (el de D1b o ninguno).
fn tras_trasladar(from: Mode, fondo: Background, to: Mode) -> (Background, Option<String>) {
    match traslada(from, fondo, to) {
        Visto::Kept(b) => (b, None),
        Visto::Reset(b, aviso) => (b, Some(aviso)),
    }
}

/// El fondo y el aviso al descartar un texto inválido con `ultimo` confirmado (A1-1 y A1-4): si
/// viaja, el aviso nuevo; si no, solo el de D1b de ese color.
fn tras_descartar(from: Mode, ultimo: Rgba, to: Mode) -> (Background, String) {
    match traslada(from, Background::Color(ultimo), to) {
        Visto::Kept(b) => (b, aviso_descartado(ultimo)),
        Visto::Reset(b, aviso) => (b, aviso),
    }
}

/// El estado del diálogo tras un traslado: informe, fondo, hexadecimal nuevo y ninguno viejo.
fn comprueba_traslado(
    d: &Dialog,
    from: Mode,
    to: Mode,
    fondo: Background,
    aviso: Option<&str>,
    ctx: &str,
) {
    assert_eq!(
        texto_del_informe(d),
        report_text(Some((64, 64)), to, aviso),
        "{ctx}: informe"
    );
    let lineas = lineas_de_aviso(d);
    assert_eq!(
        lineas,
        aviso.map(str::to_string).into_iter().collect::<Vec<_>>(),
        "{ctx}: líneas de aviso"
    );
    for linea in lineas_del_informe(d) {
        assert!(linea.chars().count() <= 42, "{ctx}: {linea:?}");
        assert!(en_el_juego(&linea), "{ctx}: {linea:?}");
    }
    assert!(
        d.is_valid(),
        "{ctx}: el formulario sale válido; {:?}",
        d.reason()
    );
    let spec = spec_de(d).unwrap_or_else(|| panic!("{ctx}: sin valores válidos"));
    assert_eq!(campos(&spec), (64, 64, to, fondo), "{ctx}");
    // El hexadecimal del modo viejo no se ve.
    if let Some(viejo) = id_color(from) {
        assert!(!d.is_shown(viejo), "{ctx}: el hexadecimal viejo se ve");
        assert_eq!(
            d.validity(viejo),
            None,
            "{ctx}: el viejo no cuenta en la validez"
        );
    }
    // El nuevo (si lo hay y es el fondo) muestra el color que viaja, válido y confirmado.
    let values = d.values().expect("válido");
    for otro in MODOS {
        if let Some(id) = id_color(otro).filter(|_| otro != to) {
            assert_eq!(values.get(id), None, "{ctx}: {id} en Values");
        }
    }
    if let (Background::Color(color), Some(id)) = (fondo, id_color(to)) {
        assert!(d.is_shown(id), "{ctx}: {id} se ve");
        assert_eq!(d.validity(id), Some(Validity::Valid), "{ctx}");
        assert!(
            !d.hex_pending(id),
            "{ctx}: el campo nuevo no conserva texto"
        );
        assert_eq!(d.hex_committed(id), Some(color), "{ctx}: confirmado");
        assert_eq!(d.text(id), Some(hex_text(color)), "{ctx}: texto del campo");
        assert_eq!(values.color(id), Some(color), "{ctx}: Values");
    } else if let Some(id) = id_color(to) {
        assert!(!d.is_shown(id), "{ctx}: {id} no debía verse");
    }
}

/// `Crear` en el modo de origen: el color que entrega `Accept` sobre una copia.
fn crear_color(d: &Dialog, from: Mode) -> Rgba {
    match d.clone().event(Event::Accept) {
        Reaction::Accepted(v) => v
            .color(id_color(from).expect("color"))
            .expect("el hexadecimal visible sale en los valores"),
        otro => panic!("se esperaba Accepted y salió {otro:?}"),
    }
}

// --- A1: sin válido previo ---

#[test]
fn fila_a1_texto_invalido_sin_valido_previo_viaja_como_808080ff_y_avisa() {
    for from in ORIGENES {
        for to in ORIGENES.into_iter().filter(|m| *m != from) {
            for texto in invalidos_de(from) {
                for camino in CAMINOS {
                    let ctx = format!("{from:?} → {to:?} {texto:?} {camino:?}");
                    let (mut d, mut sync) = origen(from, None, Some(texto));
                    hasta_el_cambio(&mut d, &mut sync, from, camino);
                    cambia_de_modo(&mut d, &mut sync, to);

                    let aviso = aviso_descartado(GRIS_OPACO);
                    assert_eq!(aviso, "Color no válido: pasa a #808080ff");
                    assert_eq!(aviso.chars().count(), 33);
                    comprueba_traslado(
                        &d,
                        from,
                        to,
                        Background::Color(GRIS_OPACO),
                        Some(&aviso),
                        &ctx,
                    );
                    let id = id_color(to).expect("color");
                    assert_eq!(d.text(id), Some("#808080".to_string()), "{ctx}");
                    // Y el aviso dura hasta que cambia otro valor.
                    assert!(!sync_form(&mut d, &mut sync), "{ctx}: estable");
                    assert!(lineas_de_aviso(&d).contains(&aviso), "{ctx}");
                }
            }
        }
    }
}

#[test]
fn fila_a1_texto_invalido_sin_valido_previo_a_indexado_dice_indexado_sin_color() {
    for from in ORIGENES {
        for texto in invalidos_de(from) {
            for camino in CAMINOS {
                let ctx = format!("{from:?} → Indexado {texto:?} {camino:?}");
                let (mut d, mut sync) = origen(from, None, Some(texto));
                hasta_el_cambio(&mut d, &mut sync, from, camino);
                cambia_de_modo(&mut d, &mut sync, Mode::Indexed);
                comprueba_traslado(
                    &d,
                    from,
                    Mode::Indexed,
                    TRANSPARENTE,
                    Some(AVISO_INDEXADO_SIN_COLOR),
                    &ctx,
                );
                assert!(!texto_del_informe(&d).contains(NO_VALIDO), "{ctx}");
            }
        }
    }
}

// --- A2: con válido previo ---

#[test]
fn fila_a1_con_valido_previo_viaja_el_ultimo_confirmado_o_sale_solo_el_aviso_de_d1b() {
    for from in ORIGENES {
        for ultimo in confirmados_de(from) {
            for to in MODOS.into_iter().filter(|m| *m != from) {
                for texto in invalidos_de(from) {
                    for camino in CAMINOS {
                        let ctx = format!("{from:?} {ultimo:?} → {to:?} {texto:?} {camino:?}");
                        let (mut d, mut sync) = origen(from, Some(ultimo), Some(texto));
                        hasta_el_cambio(&mut d, &mut sync, from, camino);
                        cambia_de_modo(&mut d, &mut sync, to);

                        let (fondo, aviso) = tras_descartar(from, ultimo, to);
                        comprueba_traslado(&d, from, to, fondo, Some(&aviso), &ctx);
                        if ultimo != GRIS_OPACO {
                            assert!(
                                !texto_del_informe(&d).contains("#808080ff"),
                                "{ctx}: nunca #808080ff si había un último confirmado"
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn fila_a1_con_valido_previo_los_ejemplos_literales_de_la_spec() {
    // (modo de origen, último confirmado, destino, aviso esperado)
    let casos = [
        (
            Mode::Rgba,
            GRIS_CLARO,
            Mode::Gray,
            "Color no válido: pasa a #aaaaaaff",
        ),
        (
            Mode::Rgba,
            GRIS_CLARO,
            Mode::GrayAlpha,
            "Color no válido: pasa a #aaaaaaff",
        ),
        (
            Mode::Rgba,
            GRIS_MEDIO,
            Mode::GrayAlpha,
            "Color no válido: pasa a #80808080",
        ),
        (
            Mode::Rgba,
            GRIS_MEDIO,
            Mode::Gray,
            "Color con alfa: pasa a Blanco",
        ),
        (
            Mode::Rgba,
            AZUL_OPACO,
            Mode::GrayAlpha,
            "Color no gris: pasa a Transparente",
        ),
        (
            Mode::Rgba,
            AZUL_OPACO,
            Mode::Gray,
            "Color no gris: pasa a Blanco",
        ),
        (
            Mode::Rgba,
            AZUL_OPACO,
            Mode::Indexed,
            "Indexado sin Color: pasa a Transparente",
        ),
        (
            Mode::GrayAlpha,
            GRIS_MEDIO,
            Mode::Rgba,
            "Color no válido: pasa a #80808080",
        ),
        (
            Mode::GrayAlpha,
            GRIS_MEDIO,
            Mode::Gray,
            "Color con alfa: pasa a Blanco",
        ),
        (
            Mode::Gray,
            GRIS_CLARO,
            Mode::Rgba,
            "Color no válido: pasa a #aaaaaaff",
        ),
        (
            Mode::Gray,
            GRIS_CLARO,
            Mode::Indexed,
            "Indexado sin Color: pasa a Transparente",
        ),
    ];
    for (from, ultimo, to, aviso) in casos {
        let (mut d, mut sync) = origen(from, Some(ultimo), Some("zz"));
        hasta_el_cambio(&mut d, &mut sync, from, Camino::Clic);
        cambia_de_modo(&mut d, &mut sync, to);
        assert_eq!(
            lineas_de_aviso(&d),
            vec![aviso.to_string()],
            "{from:?} {ultimo:?} → {to:?}"
        );
    }
}

// --- A5: texto válido sin confirmar, y el alfa ---

#[test]
fn fila_a1_texto_valido_sin_confirmar_viaja_con_el_alfa_que_daria_crear_y_sin_aviso_nuevo() {
    let con_alfa = Rgba::new(0xaa, 0xaa, 0xaa, 0xcc);
    // (modo de origen, último confirmado, texto, color que daría Crear)
    let mut casos = Vec::new();
    for from in [Mode::Rgba, Mode::GrayAlpha] {
        for (texto, esperado) in [
            ("bbbbbb", Rgba::new(0xbb, 0xbb, 0xbb, 0xcc)),
            ("bbb", Rgba::new(0xbb, 0xbb, 0xbb, 0xcc)),
            ("#bbbbbbdd", Rgba::new(0xbb, 0xbb, 0xbb, 0xdd)),
            ("#bbb8", Rgba::new(0xbb, 0xbb, 0xbb, 0x88)),
        ] {
            casos.push((from, Some(con_alfa), texto, esperado));
        }
        // Sin confirmar nada antes: el alfa de partida es 255.
        casos.push((from, None, "bbbbbb", Rgba::opaque(0xbb, 0xbb, 0xbb)));
    }
    for (texto, esperado) in [
        ("bbbbbb", Rgba::opaque(0xbb, 0xbb, 0xbb)),
        ("bbb", Rgba::opaque(0xbb, 0xbb, 0xbb)),
        ("#bbbbbbff", Rgba::opaque(0xbb, 0xbb, 0xbb)),
    ] {
        casos.push((Mode::Gray, Some(GRIS_CLARO), texto, esperado));
    }
    for (from, confirmado, texto, esperado) in casos {
        for to in MODOS.into_iter().filter(|m| *m != from) {
            let ctx = format!("{from:?} {confirmado:?} {texto:?} → {to:?}");
            let (mut d, mut sync) = origen(from, confirmado, Some(texto));
            // Es lo que daría `Crear` en el modo de origen.
            assert_eq!(crear_color(&d, from), esperado, "{ctx}: Crear");
            reposo(&mut d, &mut sync);
            cambia_de_modo(&mut d, &mut sync, to);

            let (fondo, aviso) = tras_trasladar(from, Background::Color(esperado), to);
            comprueba_traslado(&d, from, to, fondo, aviso.as_deref(), &ctx);
            assert!(!texto_del_informe(&d).contains(NO_VALIDO), "{ctx}");
        }
    }
}

#[test]
fn fila_a1_el_alfa_del_ultimo_confirmado_viaja_en_el_campo_nuevo_con_seis_digitos() {
    // `#aaaaaacc` confirmado y `bbbbbb` escrito: el campo de Gris con alfa dice `#bbbbbbcc`.
    let (mut d, mut sync) = origen(
        Mode::Rgba,
        Some(Rgba::new(0xaa, 0xaa, 0xaa, 0xcc)),
        Some("bbbbbb"),
    );
    cambia_de_modo(&mut d, &mut sync, Mode::GrayAlpha);
    assert_eq!(d.text("color_gris_alfa"), Some("#bbbbbbcc".to_string()));
    assert!(lineas_de_aviso(&d).is_empty());
    // A Gris, el alfa < 255 reinicia con el aviso de siempre.
    let (mut d, mut sync) = origen(
        Mode::Rgba,
        Some(Rgba::new(0xaa, 0xaa, 0xaa, 0xcc)),
        Some("bbbbbb"),
    );
    cambia_de_modo(&mut d, &mut sync, Mode::Gray);
    assert_eq!(lineas_de_aviso(&d), vec![AVISO_COLOR_CON_ALFA.to_string()]);
    // Y de vuelta a RGBA con `Crear`: el mismo valor que viajó.
    let (mut d, mut sync) = origen(
        Mode::Rgba,
        Some(Rgba::new(0xaa, 0xaa, 0xaa, 0xcc)),
        Some("bbbbbb"),
    );
    cambia_de_modo(&mut d, &mut sync, Mode::GrayAlpha);
    cambia_de_modo(&mut d, &mut sync, Mode::Rgba);
    assert_eq!(
        crear_color(&d, Mode::Rgba),
        Rgba::new(0xbb, 0xbb, 0xbb, 0xcc)
    );
}

// --- A6: anchos ---

#[test]
fn fila_a1_los_avisos_caben_en_42_caracteres_y_estan_en_el_juego() {
    for c in [
        Rgba::opaque(255, 255, 255),
        Rgba::new(0, 0, 0, 0),
        GRIS_MEDIO,
        GRIS_OPACO,
        AZUL_OPACO,
        Rgba::new(0xff, 0xff, 0xff, 0xfe),
    ] {
        let aviso = aviso_descartado(c);
        assert_eq!(aviso.chars().count(), 33, "{aviso}");
        assert!(en_el_juego(&aviso), "{aviso}");
    }
    for aviso in AVISOS {
        assert!(aviso.chars().count() <= 42, "{aviso}");
        assert!(en_el_juego(aviso), "{aviso}");
    }
}

#[test]
fn fila_a1_el_informe_con_tamano_que_avisa_e_indexado_cabe_en_4_lineas_de_42() {
    let revisa = |d: &Dialog, to: Mode, ctx: &str| {
        let lineas = lineas_del_informe(d);
        assert!(lineas.len() <= 4, "{ctx}: {lineas:?}");
        assert!(lineas[0].starts_with("Memoria:"), "{ctx}: {lineas:?}");
        assert!(
            lineas.iter().any(|l| l.starts_with("aviso: más de")),
            "{ctx}: el tamaño avisa; {lineas:?}"
        );
        assert_eq!(
            lineas.iter().any(|l| l.starts_with("Paleta:")),
            to == Mode::Indexed,
            "{ctx}"
        );
        assert_eq!(lineas_de_aviso(d).len(), 1, "{ctx}: {lineas:?}");
        for linea in &lineas {
            assert!(linea.chars().count() <= 42, "{ctx}: {linea:?}");
            assert!(en_el_juego(linea), "{ctx}: {linea:?}");
        }
        // El aviso de fondo va el último, como en la fila 6.
        assert_eq!(lineas.last(), lineas_de_aviso(d).last(), "{ctx}");
    };
    for from in ORIGENES {
        for ultimo in confirmados_de(from) {
            for to in MODOS.into_iter().filter(|m| *m != from) {
                let ctx = format!("{from:?} {ultimo:?} → {to:?} con 5000 × 5000");
                let (mut d, mut sync) = origen(from, Some(ultimo), Some("zz"));
                for campo in ["ancho", "alto"] {
                    paso(
                        &mut d,
                        &mut sync,
                        Event::Number {
                            field: campo,
                            text: "5000".to_string(),
                        },
                    );
                }
                hasta_el_cambio(&mut d, &mut sync, from, Camino::Intro);
                cambia_de_modo(&mut d, &mut sync, to);
                revisa(&d, to, &ctx);
            }
        }
    }
    // Transparente a Gris (el quinto texto de D1b) con el tamaño que avisa.
    let (mut d, mut sync) = dialogo_sincronizado(Mode::Rgba, TRANSPARENTE);
    for campo in ["ancho", "alto"] {
        paso(
            &mut d,
            &mut sync,
            Event::Number {
                field: campo,
                text: "5000".to_string(),
            },
        );
    }
    cambia_de_modo(&mut d, &mut sync, Mode::Gray);
    revisa(&d, Mode::Gray, "Transparente → Gris con 5000 × 5000");
}

// --- A7: vida, hueco único y silencio ---

#[test]
fn fila_a1_el_aviso_nuevo_se_va_al_cambiar_otro_valor_como_los_de_d1b() {
    type Cambio = (&'static str, fn(&mut Dialog, &mut FormSync));
    let cambios: [Cambio; 5] = [
        ("ancho +", |d, s| {
            paso(
                d,
                s,
                Event::Step {
                    field: "ancho",
                    delta: 1,
                },
            );
        }),
        ("alto 10", |d, s| {
            paso(
                d,
                s,
                Event::Number {
                    field: "alto",
                    text: "10".to_string(),
                },
            );
        }),
        ("fondo Blanco", |d, s| {
            paso(
                d,
                s,
                Event::Choice {
                    field: "fondo_gris_alfa",
                    index: 1,
                },
            );
        }),
        ("hexadecimal nuevo", |d, s| {
            paso(
                d,
                s,
                Event::Hex {
                    field: "color_gris_alfa",
                    text: "#444444".to_string(),
                },
            );
        }),
        ("modo", |d, s| {
            paso(
                d,
                s,
                Event::Choice {
                    field: "modo",
                    index: 1,
                },
            );
        }),
    ];
    for (nombre, cambia) in cambios {
        let (mut d, mut sync) = origen(Mode::Rgba, None, Some("zz"));
        hasta_el_cambio(&mut d, &mut sync, Mode::Rgba, Camino::Clic);
        cambia_de_modo(&mut d, &mut sync, Mode::GrayAlpha);
        assert_eq!(
            lineas_de_aviso(&d),
            vec![aviso_descartado(GRIS_OPACO)],
            "{nombre}"
        );
        // Dura mientras no cambia nada.
        reposo(&mut d, &mut sync);
        assert_eq!(
            lineas_de_aviso(&d),
            vec![aviso_descartado(GRIS_OPACO)],
            "{nombre}"
        );
        cambia(&mut d, &mut sync);
        // Ni el nuevo ni el de D1b (el traslado de modo de un fondo válido no avisa).
        assert!(
            lineas_de_aviso(&d).is_empty(),
            "{nombre}: {:?}",
            lineas_de_aviso(&d)
        );
        assert!(!texto_del_informe(&d).contains(NO_VALIDO), "{nombre}");
    }
}

#[test]
fn fila_a1_si_el_ultimo_confirmado_no_viaja_sale_solo_el_aviso_de_d1b_en_una_linea() {
    for (to, aviso) in [
        (Mode::GrayAlpha, AVISO_NO_GRIS_A_GRIS_ALFA),
        (Mode::Gray, AVISO_NO_GRIS_A_GRIS),
        (Mode::Indexed, AVISO_INDEXADO_SIN_COLOR),
    ] {
        for camino in CAMINOS {
            let (mut d, mut sync) = origen(Mode::Rgba, Some(AZUL_OPACO), Some("zz"));
            hasta_el_cambio(&mut d, &mut sync, Mode::Rgba, camino);
            cambia_de_modo(&mut d, &mut sync, to);
            assert_eq!(
                lineas_de_aviso(&d),
                vec![aviso.to_string()],
                "{to:?} {camino:?}: un solo aviso, el de D1b"
            );
            assert!(
                !texto_del_informe(&d).contains(NO_VALIDO),
                "{to:?} {camino:?}"
            );
            assert!(lineas_del_informe(&d).len() <= 4);
        }
    }
}

#[test]
fn fila_a1_un_texto_invalido_oculto_no_avisa() {
    for from in ORIGENES {
        let opciones: &[Tipo] = match from {
            Mode::Gray => &[Tipo::Blanco, Tipo::Negro],
            _ => &[Tipo::Transparente, Tipo::Blanco, Tipo::Negro],
        };
        for &tipo in opciones {
            for to in MODOS.into_iter().filter(|m| *m != from) {
                let ctx = format!("{from:?} {tipo:?} → {to:?}");
                let (mut d, mut sync) = origen(from, None, Some("zz"));
                let id = id_color(from).expect("color");
                // La opción deja de ser «Color…»: el hexadecimal se oculta con basura dentro.
                paso(
                    &mut d,
                    &mut sync,
                    Event::Choice {
                        field: id_fondo(from),
                        index: indice_de_fondo(from, tipo).expect("opción"),
                    },
                );
                assert!(!d.is_shown(id), "{ctx}");
                assert!(d.hex_pending(id), "{ctx}: la basura sigue escrita");
                reposo(&mut d, &mut sync);
                cambia_de_modo(&mut d, &mut sync, to);

                let fondo = match tipo {
                    Tipo::Transparente => TRANSPARENTE,
                    Tipo::Blanco => BLANCO,
                    _ => NEGRO,
                };
                let (esperado, aviso) = tras_trasladar(from, fondo, to);
                assert_eq!(
                    lineas_de_aviso(&d),
                    aviso.clone().into_iter().collect::<Vec<_>>(),
                    "{ctx}"
                );
                assert!(!texto_del_informe(&d).contains(NO_VALIDO), "{ctx}");
                assert_eq!(
                    campos(&spec_de(&d).expect("válido")),
                    (64, 64, to, esperado),
                    "{ctx}"
                );
            }
        }
    }
}

#[test]
fn fila_a1_los_traslados_de_valores_validos_de_d1b_siguen_igual_y_sin_el_aviso_nuevo() {
    for (nombre, fondo, origenes, casillas) in tabla_d1b() {
        for from in origenes {
            for (to, casilla) in DESTINOS.into_iter().zip(casillas) {
                if from == to {
                    continue;
                }
                let (mut d, mut sync) = dialogo_sincronizado(from, fondo);
                cambia_de_modo(&mut d, &mut sync, to);
                let (fondo_esperado, aviso) = match casilla {
                    Igual => (fondo, None),
                    Reinicia(a, aviso) => (a, Some(aviso)),
                };
                let ctx = format!("{nombre}: {from:?} → {to:?}");
                assert!(!texto_del_informe(&d).contains(NO_VALIDO), "{ctx}");
                assert_eq!(
                    texto_del_informe(&d),
                    report_text(Some((64, 64)), to, aviso),
                    "{ctx}"
                );
                assert_eq!(
                    campos(&spec_de(&d).expect("válido")),
                    (64, 64, to, fondo_esperado),
                    "{ctx}"
                );
            }
        }
    }
}

#[test]
fn fila_a1_sync_form_no_puede_lanzar_el_aviso_de_5_s_porque_no_recibe_como() {
    // El aviso del título (`App::notify`) queda bajo el velo del modal (A1-4): el descarte solo
    // puede decirse en el informe. `sync_form` solo recibe el diálogo y su estado y devuelve si hay
    // algo que pintar; si ganase un parámetro para avisar, esta prueba dejaría de compilar.
    let _: fn(&mut Dialog, &mut FormSync) -> bool = sync_form;
    let (mut d, mut sync) = origen(Mode::Rgba, None, Some("zz"));
    hasta_el_cambio(&mut d, &mut sync, Mode::Rgba, Camino::Clic);
    let cambio = {
        paso(
            &mut d,
            &mut sync,
            Event::Choice {
                field: "modo",
                index: indice_de_modo(Mode::Gray),
            },
        );
        sync_form(&mut d, &mut sync)
    };
    assert!(!cambio, "tras el traslado no queda nada por pintar");
    assert_eq!(lineas_de_aviso(&d), vec![aviso_descartado(GRIS_OPACO)]);
}

// --- Propiedades ---

const TEXTOS_A1: [&str; 17] = [
    "zz",
    "#12",
    "",
    "#ff0000",
    "#808080",
    "#80808080",
    "#ff000080",
    "bbb",
    "#bbb8",
    "bbbbbb",
    "#bbbbbbdd",
    " #bbbbbb ",
    "#aabbcc",
    "#12345",
    "#ggg",
    "#fff",
    "bbbb",
];

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Nunca se sustituye en silencio: con un texto inválido en un «Color…» visible, el informe
    /// lleva exactamente un aviso (el nuevo o el de D1b), el campo nuevo es válido y vale el último
    /// confirmado o su traslado de D1b; con un texto válido (o ninguno) no hay aviso nuevo y el
    /// fondo es el de `Crear`. Los caminos (a) y (b) valen igual.
    #[test]
    fn propiedad_a1_1_nunca_se_sustituye_en_silencio(
        desde in 0usize..3,
        salto in 1usize..4,
        con_confirmado in any::<bool>(),
        color in any::<[u8; 4]>(),
        texto in 0..=TEXTOS_A1.len(),
        clic in any::<bool>(),
    ) {
        let from = ORIGENES[desde];
        let to = MODOS[(indice_de_modo(from) + salto) % 4];
        let confirmado = con_confirmado.then(|| confirmable(from, color));
        let ultimo = confirmado.unwrap_or(GRIS_OPACO);
        let texto = TEXTOS_A1.get(texto).copied();
        let ctx = format!("{from:?} {confirmado:?} {texto:?} → {to:?} clic {clic}");
        let (mut d, mut sync) = origen(from, confirmado, texto);
        let lectura = match texto {
            Some(texto) => parse_hex(texto, from, ultimo).ok(),
            None => Some(ultimo),
        };
        match lectura {
            None => {
                let camino = if clic { Camino::Clic } else { Camino::Intro };
                hasta_el_cambio(&mut d, &mut sync, from, camino);
                cambia_de_modo(&mut d, &mut sync, to);
                let (fondo, aviso) = tras_descartar(from, ultimo, to);
                comprueba_traslado(&d, from, to, fondo, Some(&aviso), &ctx);
            }
            Some(color) => {
                reposo(&mut d, &mut sync);
                cambia_de_modo(&mut d, &mut sync, to);
                let (fondo, aviso) = tras_trasladar(from, Background::Color(color), to);
                comprueba_traslado(&d, from, to, fondo, aviso.as_deref(), &ctx);
                prop_assert!(!texto_del_informe(&d).contains(NO_VALIDO), "{}", ctx);
            }
        }
    }
}

/// Un gesto de la propiedad 3 de la 19 o uno de los nuevos: `Intro`, la pérdida del foco y el
/// cambio de modo con clic (que antes quita el foco al campo visible).
#[derive(Clone, Debug)]
enum GestoA1 {
    Base(Gesto),
    Intro,
    PierdeFoco,
    ClicModo(usize),
}

fn gesto_a1() -> impl Strategy<Value = GestoA1> {
    prop_oneof![
        4 => gesto().prop_map(GestoA1::Base),
        1 => Just(GestoA1::Intro),
        1 => Just(GestoA1::PierdeFoco),
        2 => (0usize..4).prop_map(GestoA1::ClicModo),
    ]
}

/// Aplica un gesto y devuelve el modo en que queda; el clic en el modo hace su `sync_form` del
/// fotograma de la pulsación entre la pérdida del foco y el cambio.
fn aplica_a1(d: &mut Dialog, sync: &mut FormSync, modo: usize, gesto: &GestoA1) -> usize {
    let visible = id_color(MODOS[modo]).filter(|&id| d.is_shown(id));
    match gesto {
        GestoA1::Base(g) => aplica(d, modo, g),
        GestoA1::Intro => {
            if let Some(id) = visible {
                d.event(Event::HexConfirm {
                    field: id,
                    blur: false,
                });
            }
            modo
        }
        GestoA1::PierdeFoco => {
            if let Some(id) = visible {
                d.event(Event::HexConfirm {
                    field: id,
                    blur: true,
                });
            }
            modo
        }
        GestoA1::ClicModo(i) => {
            if let Some(id) = visible {
                d.event(Event::HexConfirm {
                    field: id,
                    blur: true,
                });
                sync_form(d, sync);
            }
            d.event(Event::Choice {
                field: "modo",
                index: *i,
            });
            *i
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// El formulario sigue sano con las secuencias de la propiedad 3 de la 19 ampliadas con `Intro`,
    /// la pérdida del foco y el cambio de modo con clic: el informe cabe (≤ 4 líneas de ≤ 42
    /// caracteres dentro del juego), a lo sumo un aviso de fondo, ningún texto inválido en los
    /// valores, y el aviso no sobrevive a un cambio de otro campo.
    #[test]
    fn propiedad_a1_2_el_formulario_sigue_sano(
        gestos in prop::collection::vec(gesto_a1(), 0..=48),
    ) {
        let mut d = dialogo(8192);
        let mut sync = FormSync::default();
        sync_form(&mut d, &mut sync);
        let mut modo = 0usize;
        for (i, g) in gestos.iter().enumerate() {
            let ctx = format!("paso {i}: {g:?}");
            let antes = sin_informe(&d);
            let modo_anterior = modo;
            modo = aplica_a1(&mut d, &mut sync, modo, g);
            let cambio = sin_informe(&d) != antes;
            sync_form(&mut d, &mut sync);

            comprueba_formulario(&d, modo, &ctx)?;
            prop_assert!(!sync_form(&mut d, &mut sync), "{}: sync_form no es estable", ctx);
            let avisos = lineas_de_aviso(&d);
            prop_assert!(avisos.len() <= 1, "{}: {:?}", ctx, avisos);
            for aviso in &avisos {
                prop_assert!(
                    AVISOS.contains(&aviso.as_str())
                        || (aviso.starts_with(NO_VALIDO) && aviso.chars().count() == 33),
                    "{}: aviso desconocido {:?}",
                    ctx,
                    aviso
                );
            }
            // Ningún texto inválido en los valores: el hexadecimal visible es válido y sale.
            if let Some(values) = d.values()
                && let Some(id) = id_color(MODOS[modo]).filter(|&id| d.is_shown(id))
            {
                prop_assert_eq!(d.validity(id), Some(Validity::Valid), "{}", ctx);
                prop_assert!(values.color(id).is_some(), "{}: {} sin color", ctx, id);
            }
            if cambio && modo == modo_anterior {
                prop_assert!(avisos.is_empty(), "{}: el aviso sobrevive: {:?}", ctx, avisos);
            }
        }
    }
}

const TEXTOS_P3: [&str; 24] = [
    "bbb",
    "#bbb",
    "bbbb",
    "#bbb8",
    "bbbbbb",
    "#bbbbbb",
    "bbbbbbdd",
    "#bbbbbbdd",
    "  bbb ",
    " #bbbbbb  ",
    "  #bbbbbbdd",
    "#808080",
    "#80808080",
    "#ff0000",
    "#ff000080",
    "#fff",
    "#f00",
    "",
    "#",
    "zz",
    "#12",
    "#12345",
    "#ggg",
    "bbbbbbbbb",
];

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// El traslado y `Crear` leen lo mismo: si `resolve_hex` da `Ok(c)`, `Crear` da `c` y el
    /// traslado lee `c`; si da `Err`, `Crear` se rechaza y el traslado lee el último confirmado y
    /// avisa.
    #[test]
    fn propiedad_a1_3_traslado_y_crear_leen_lo_mismo(
        desde in 0usize..3,
        salto in 1usize..4,
        con_confirmado in any::<bool>(),
        color in any::<[u8; 4]>(),
        texto in 0..TEXTOS_P3.len(),
    ) {
        let from = ORIGENES[desde];
        let to = MODOS[(indice_de_modo(from) + salto) % 4];
        let confirmado = con_confirmado.then(|| confirmable(from, color));
        let ultimo = confirmado.unwrap_or(GRIS_OPACO);
        let texto = TEXTOS_P3[texto];
        let ctx = format!("{from:?} {confirmado:?} {texto:?} → {to:?}");
        let (mut d, mut sync) = origen(from, confirmado, Some(texto));
        let id = id_color(from).expect("color");
        prop_assert_eq!(d.hex_committed(id), Some(ultimo), "{}", ctx);
        match resolve_hex(Some(texto), from, ultimo) {
            Ok(c) => {
                prop_assert_eq!(crear_color(&d, from), c, "{}: Crear", ctx);
                prop_assert_eq!(
                    read_background(&d, from),
                    Some(Background::Color(c)),
                    "{}: lectura del traslado",
                    ctx
                );
                reposo(&mut d, &mut sync);
                cambia_de_modo(&mut d, &mut sync, to);
                let (fondo, aviso) = tras_trasladar(from, Background::Color(c), to);
                comprueba_traslado(&d, from, to, fondo, aviso.as_deref(), &ctx);
                prop_assert!(!texto_del_informe(&d).contains(NO_VALIDO), "{}", ctx);
            }
            Err(_) => {
                let rechazado = matches!(d.clone().event(Event::Accept), Reaction::Rejected(_));
                prop_assert!(rechazado, "{}: Crear debía rechazarse", ctx);
                prop_assert_eq!(
                    read_background(&d, from),
                    Some(Background::Color(ultimo)),
                    "{}: lectura del traslado",
                    ctx
                );
                hasta_el_cambio(&mut d, &mut sync, from, Camino::Intro);
                cambia_de_modo(&mut d, &mut sync, to);
                let (fondo, aviso) = tras_descartar(from, ultimo, to);
                comprueba_traslado(&d, from, to, fondo, Some(&aviso), &ctx);
            }
        }
    }
}
