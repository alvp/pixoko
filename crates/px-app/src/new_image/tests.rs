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
//! - Ids de campo: los literales de D11 (`width`, `height`, `mode_2`, `fondo_indexado`, `fondo_rgba`,
//!   `color_rgba`, `fondo_gris_alfa`, `color_gris_alfa`, `fondo_gris`, `color_gris`, `report`); de
//!   las constantes solo se nombra `FIELD_WIDTH`.
//! - Orden de las opciones de `mode_2`: Indexado (0), RGBA (1), Gris con alfa (2), Gris (3), y por
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
//! - `NewImageError::Memory { bytes }`: `bytes` es `memory_estimate(w, h, mode_2).ram_bytes` (la
//!   «estimación» de la spec es la de RAM, la que da la cifra en MiB del aviso). La spec no da una
//!   función para el aviso «sin memoria para una imagen de {w}×{h} (~{n} MiB)»: no se prueba aquí
//!   (queda para el cableado de `App`).
//! - `describe_new`: modos con los nombres de `describe_open` (`indexed_2`, `RGBA`, `gray`,
//!   `gray with alpha_2`); solo el indexado lleva ` · 32 colors`.
//! - El tamaño «grande» de la fila 5 es 1000 × 600 (esquinas y centro).
//! - El recorrido de `App` (D8) se escribe aquí como el ayudante `creates` (el cableado es del último
//!   commit de la unidad): `Unsaved` antes de construir nada, luego `build_new_image`, luego
//!   `Session::new_image` y `Canvas::set_image` con la imagen del documento. Se prueban los contratos de las tres piezas.
//! - `new_image_form` con un lado de textura menor que 64: el valor por defecto es `min(64, máximo)`
//!   (`validate` rechaza un defecto fuera de rango); con 1, el defecto es 1.
//! - La línea de estado del informe de la propiedad 3 se compara solo en lo que dice la spec
//!   (≤ 4 líneas, ≤ 42 caracteres, dentro del juego, empieza por `TestMemory:`).

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

/// Los modos en el orden del campo `mode_2` de D11.
const TEST_MODES: [Mode; 4] = [Mode::Indexed, Mode::Rgba, Mode::GrayAlpha, Mode::Gray];

const GRAY_OPAQUE: Rgba = Rgba::opaque(0x80, 0x80, 0x80);
const GRAY_MIDDLE: Rgba = Rgba::new(0x80, 0x80, 0x80, 0x80);
const TEST_RED: Rgba = Rgba::opaque(0xff, 0, 0);
const RED_MIDDLE: Rgba = Rgba::new(0xff, 0, 0, 0x80);

const TRANSPARENT: Background = Background::Transparent;
const WHITE: Background = Background::White;
const TEST_BLACK: Background = Background::Black;

/// Los cuatro tipos de opción de un fondo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Type {
    Transparent,
    White,
    Black,
    Color,
}

fn index_of_mode(mode: Mode) -> usize {
    TEST_MODES.iter().position(|m| *m == mode).expect("modo")
}

fn id_background(mode: Mode) -> &'static str {
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

/// El índice de la opción de `kind` en el grupo de fondo de `mode` (D1 y D11).
fn index_of_background(mode: Mode, kind: Type) -> Option<usize> {
    match (mode, kind) {
        (Mode::Gray, Type::Transparent) => None,
        (Mode::Gray, Type::White) => Some(0),
        (Mode::Gray, Type::Black) => Some(1),
        (Mode::Gray, Type::Color) => Some(2),
        (Mode::Indexed, Type::Color) => None,
        (_, Type::Transparent) => Some(0),
        (_, Type::White) => Some(1),
        (_, Type::Black) => Some(2),
        (_, Type::Color) => Some(3),
    }
}

fn type_of(background_2: Background) -> Type {
    match background_2 {
        Background::Transparent => Type::Transparent,
        Background::White => Type::White,
        Background::Black => Type::Black,
        Background::Color(_) => Type::Color,
    }
}

/// Los fondos que un modo admite (de la tabla D1), con los colores de la fila 3.
fn backgrounds_valid(mode: Mode) -> Vec<Background> {
    let color = Background::Color;
    match mode {
        Mode::Rgba => vec![
            TRANSPARENT,
            WHITE,
            TEST_BLACK,
            color(GRAY_OPAQUE),
            color(RED_MIDDLE),
            color(GRAY_MIDDLE),
        ],
        Mode::GrayAlpha => vec![
            TRANSPARENT,
            WHITE,
            TEST_BLACK,
            color(GRAY_OPAQUE),
            color(GRAY_MIDDLE),
        ],
        Mode::Gray => vec![WHITE, TEST_BLACK, color(GRAY_OPAQUE)],
        Mode::Indexed => vec![TRANSPARENT, WHITE, TEST_BLACK],
    }
}

/// Los seis fondos de la fila 3.
fn backgrounds_of_row_3() -> [Background; 6] {
    [
        TRANSPARENT,
        WHITE,
        TEST_BLACK,
        Background::Color(GRAY_OPAQUE),
        Background::Color(RED_MIDDLE),
        Background::Color(GRAY_MIDDLE),
    ]
}

fn dialog(max_side: usize) -> Dialog {
    Dialog::new(DialogKind::NewImage, new_image_form(max_side)).expect("formulario válido")
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

fn writes_color(d: &mut Dialog, field: &'static str, text: &str) {
    assert_eq!(
        d.event(Event::Hex {
            field,
            text: text.to_string()
        }),
        Reaction::None
    );
}

fn confirms_color(d: &mut Dialog, field: &'static str) {
    assert_eq!(
        d.event(Event::HexConfirm { field, blur: false }),
        Reaction::None
    );
}

/// Elige `mode` y, en su grupo, `background_2` (con su hexadecimal confirmado si es un color).
fn put_background(d: &mut Dialog, mode: Mode, background_2: Background) {
    chooses(d, "modo", index_of_mode(mode));
    let index_2 =
        index_of_background(mode, type_of(background_2)).expect("el modo admite ese fondo");
    chooses(d, id_background(mode), index_2);
    if let Background::Color(color) = background_2 {
        let id = id_color(mode).expect("el modo tiene color");
        writes_color(d, id, &color.to_string());
        confirms_color(d, id);
    }
}

fn spec_de(d: &Dialog) -> Option<NewImageSpec> {
    spec_from_values(&d.values()?)
}

/// El contenido de una `NewImageSpec`, sin suponer sus derives.
fn fields(spec: &NewImageSpec) -> (u32, u32, Mode, Background) {
    (spec.width, spec.height, spec.mode, spec.background)
}

fn spec_new(width: u32, height: u32, mode: Mode, background: Background) -> NewImageSpec {
    NewImageSpec {
        width,
        height,
        mode,
        background,
    }
}

fn in_the_set(text_2: &str) -> bool {
    text_2.chars().all(|c| INTERFACE_CHARSET.contains(c))
}

fn invalid(text_2: &str) -> Option<Validity> {
    Some(Validity::Invalid(text_2.to_string()))
}

/// El texto de un campo `ReadOnly`, leído de la spec del diálogo.
fn text_of_the_report(d: &Dialog) -> String {
    d.spec()
        .fields
        .iter()
        .find_map(|f| match &f.kind {
            FieldKind::ReadOnly { text, .. } if f.id == "informe" => Some(text.clone()),
            _ => None,
        })
        .expect("el formulario tiene el informe")
}

fn color_visible(mode: Mode, background_2: Background) -> Option<Rgba> {
    Some(match background_pixel(mode, background_2)? {
        Pixel::Rgba(color) => color,
        Pixel::GrayAlpha { gray, alpha } => Rgba::new(gray, gray, gray, alpha),
        Pixel::Gray(gray) => Rgba::opaque(gray, gray, gray),
        Pixel::Indexed(index) => initial_palette().resolved(index).expect("índice válido"),
    })
}

/// `Ok` o el motivo en texto (para no suponer `Debug` en `NewImageError`).
fn builds(spec: &NewImageSpec, alloc: &mut dyn ImageAlloc) -> Result<Image, String> {
    match build_new_image(spec, alloc) {
        Ok(image) => Ok(image),
        Err(NewImageError::Size(text_2)) => Err(format!("Size({text_2})")),
        Err(NewImageError::Memory { bytes }) => Err(format!("Memory({bytes})")),
    }
}

fn builds_real(spec: &NewImageSpec) -> Image {
    builds(spec, &mut SystemAlloc).unwrap_or_else(|e| {
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
fn row_1_the_form_with_side_16384_and_8192_asks_for_width_and_height_from_1_to_8192_with_64_by_default()
 {
    assert_eq!(FIELD_WIDTH, "ancho");
    assert_eq!((MAX_SIDE, DEFAULT_SIDE), (8192, 64));
    for max_side in [16384usize, 8192] {
        let spec = new_image_form(max_side);
        assert_eq!(spec.title, "Nueva imagen");
        assert_eq!(spec.accept, "Crear");
        assert_eq!(spec.cancel, Some("Cancelar"));
        spec.validate().expect("spec válida");
        for (id, label) in [("ancho", "Ancho"), ("alto", "Alto")] {
            let field_2 = spec
                .fields
                .iter()
                .find(|f| f.id == id)
                .unwrap_or_else(|| panic!("falta {id}"));
            assert_eq!(field_2.label, label);
            assert_eq!(
                field_2.kind,
                FieldKind::Number {
                    min: 1,
                    max: 8192,
                    default: 64
                },
                "max_side {max_side}: {id}"
            );
            assert!(field_2.shown_when.is_none());
        }
    }
    // Con 16384 el máximo sigue siendo 8192 y no hay nada que cambie respecto a 8192.
    assert_eq!(new_image_form(16384), new_image_form(8192));
}

#[test]
fn row_1_by_default_64_by_64_is_valid_and_the_initial_focus_is_the_width() {
    for max_side in [16384usize, 8192] {
        let d = dialog(max_side);
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
fn row_1_each_width_and_height_value_gives_its_reason_with_16384_and_8192() {
    for max_side in [16384usize, 8192] {
        for (field_2, label_2) in [("ancho", "Ancho"), ("alto", "Alto")] {
            for (text_2, expected_2) in [
                ("0", invalid("mínimo 1")),
                ("1", Some(Validity::Valid)),
                ("64", Some(Validity::Valid)),
                ("8192", Some(Validity::Valid)),
                ("8193", invalid("máximo 8192")),
                ("", invalid("escribe un número")),
                ("abc", invalid("solo dígitos 0-9")),
            ] {
                let mut d = dialog(max_side);
                number(&mut d, field_2, text_2);
                assert_eq!(
                    d.validity(field_2),
                    expected_2,
                    "max_side {max_side}, {field_2} = {text_2:?}"
                );
                match expected_2 {
                    Some(Validity::Invalid(reason_2)) => {
                        assert!(!d.is_valid());
                        assert_eq!(d.values(), None);
                        assert_eq!(d.reason(), Some(format!("{label_2}: {reason_2}")));
                        assert!(
                            !reason_2.contains("tarjeta"),
                            "el límite de 8192 no es de la GPU: {reason_2}"
                        );
                        // En línea: `error: ` y el motivo.
                        assert!(format!("{ERROR_PREFIX}{reason_2}").len() <= 44);
                    }
                    _ => assert!(d.is_valid(), "{:?}", d.reason()),
                }
            }
        }
    }
}

#[test]
fn row_1_the_reason_of_smaller_and_larger_side_says_exactly_error_minimum_1_and_error_maximum_8192()
{
    let mut d = dialog(16384);
    number(&mut d, "ancho", "0");
    assert_eq!(
        format!("{ERROR_PREFIX}{}", reason_of(&d, "ancho")),
        "error: mínimo 1"
    );
    number(&mut d, "ancho", "8193");
    assert_eq!(
        format!("{ERROR_PREFIX}{}", reason_of(&d, "ancho")),
        "error: máximo 8192"
    );
}

fn reason_of(d: &Dialog, field_2: &'static str) -> String {
    match d.validity(field_2) {
        Some(Validity::Invalid(reason_2)) => reason_2,
        other_2 => panic!("{field_2} debía ser inválido y es {other_2:?}"),
    }
}

const TEXT_OF_SIZE: &str = "ancho × alto: máx. 67108864 píxeles";

#[test]
fn row_1_size_error_gives_none_at_8192_by_8192_in_all_modes() {
    for mode in TEST_MODES {
        assert_eq!(size_error(8192, 8192, mode), None, "{mode:?}");
        assert_eq!(size_error(1, 1, mode), None, "{mode:?}");
        assert_eq!(size_error(64, 64, mode), None, "{mode:?}");
        assert_eq!(size_error(8192, 1, mode), None, "{mode:?}");
    }
}

#[test]
fn row_1_size_error_gives_the_d5_text_in_the_rest_without_overflowing() {
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
        for mode in TEST_MODES {
            let text_2 = size_error(width, height, mode)
                .unwrap_or_else(|| panic!("{width}×{height} {mode:?}: debía dar error"));
            assert_eq!(text_2, TEXT_OF_SIZE, "{width}×{height} {mode:?}");
        }
    }
}

#[test]
fn row_1_the_size_error_text_fits_in_the_reason_and_is_in_the_set() {
    assert_eq!(TEXT_OF_SIZE.chars().count(), 35);
    let text_2 = size_error(8193, 8192, Mode::Rgba).expect("error");
    assert!(text_2.chars().count() <= 37, "{text_2}");
    assert!(in_the_set(&text_2), "{text_2}");
}

#[test]
fn row_1_the_form_check_does_not_fire_with_valid_sides_and_uses_size_error() {
    let spec = new_image_form(16384);
    let check = spec.check.expect("el formulario tiene un check cruzado");
    // Con cualquier valor que el formulario deje pasar, el check no da motivo.
    let mut d = dialog(16384);
    for (width, height) in [("1", "1"), ("64", "64"), ("8192", "8192"), ("8192", "1")] {
        number(&mut d, "ancho", width);
        number(&mut d, "alto", height);
        let values = d.values().unwrap_or_else(|| panic!("{width}×{height}"));
        assert_eq!(check(&values), None, "{width}×{height}");
        assert_eq!(d.reason(), None, "{width}×{height}");
    }
}

// ---------------------------------------------------------------------------------------
// Fila 2 (parte pura): el lado de textura de la GPU.
// ---------------------------------------------------------------------------------------

fn maximum_expected(max_side: usize) -> u32 {
    u32::try_from(max_side.clamp(1, 8192)).expect("cabe")
}

#[test]
fn row_2_the_field_maximum_is_max_1_min_8192_side_for_each_texture_side() {
    for max_side in [8191usize, 4096, 2048, 100, 1, 8192, 16384] {
        let max = maximum_expected(max_side);
        let spec = new_image_form(max_side);
        spec.validate()
            .unwrap_or_else(|e| panic!("max_side {max_side}: {e:?}"));
        for id in ["ancho", "alto"] {
            let field_2 = spec.fields.iter().find(|f| f.id == id).expect("campo");
            let FieldKind::Number {
                min,
                max: cap,
                default,
            } = field_2.kind
            else {
                panic!("{id} no es numérico");
            };
            assert_eq!(min, 1, "max_side {max_side}: {id}");
            assert_eq!(cap, max, "max_side {max_side}: {id}");
            assert_eq!(
                default,
                64.min(max),
                "max_side {max_side}: {id} (por defecto)"
            );
        }
    }
}

#[test]
fn row_2_the_maximum_is_valid_and_the_next_gives_maximum_n_without_a_note_in_the_plain_form() {
    for max_side in [8191usize, 4096, 2048, 100, 1, 8192, 16384] {
        let max = maximum_expected(max_side);
        for field_2 in ["ancho", "alto"] {
            let mut d = dialog(max_side);
            number(&mut d, field_2, &max.to_string());
            assert_eq!(
                d.validity(field_2),
                Some(Validity::Valid),
                "{max_side} {field_2}"
            );
            number(&mut d, field_2, &(max + 1).to_string());
            assert_eq!(
                d.validity(field_2),
                invalid(&format!("máximo {max}")),
                "{max_side} {field_2}: sin set_limit_note el texto es el de siempre"
            );
        }
    }
}

#[test]
fn row_2_zero_and_eleven_digits_give_their_usual_text_with_any_texture_side() {
    for max_side in [8191usize, 4096, 2048, 100, 1, 8192, 16384] {
        for field_2 in ["ancho", "alto"] {
            let mut d = dialog(max_side);
            number(&mut d, field_2, "0");
            assert_eq!(d.validity(field_2), invalid("mínimo 1"), "{max_side}");
            number(&mut d, field_2, "99999999999");
            assert_eq!(
                d.validity(field_2),
                invalid("máximo 10 cifras"),
                "{max_side}"
            );
            number(&mut d, field_2, "5000");
            let expected_2 = if maximum_expected(max_side) >= 5000 {
                Some(Validity::Valid)
            } else {
                invalid(&format!("máximo {}", maximum_expected(max_side)))
            };
            assert_eq!(
                d.validity(field_2),
                expected_2,
                "{max_side} {field_2} = 5000"
            );
        }
    }
}

#[test]
fn row_2_the_graphics_card_note_is_the_spec_one() {
    assert_eq!(GPU_LIMIT_NOTE, "tarjeta gráfica");
    assert!(in_the_set(GPU_LIMIT_NOTE));
}

// ---------------------------------------------------------------------------------------
// Fila 3: fondo por modo.
// ---------------------------------------------------------------------------------------

fn rgba_px(r: u8, g: u8, b: u8, a: u8) -> Option<Pixel> {
    Some(Pixel::Rgba(Rgba::new(r, g, b, a)))
}

fn gray_alpha_px(gray: u8, alpha: u8) -> Option<Pixel> {
    Some(Pixel::GrayAlpha { gray, alpha })
}

#[test]
fn row_3_background_pixel_in_rgba() {
    let expected_2 = [
        rgba_px(0, 0, 0, 0),
        rgba_px(255, 255, 255, 255),
        rgba_px(0, 0, 0, 255),
        rgba_px(0x80, 0x80, 0x80, 0xff),
        rgba_px(0xff, 0, 0, 0x80),
        rgba_px(0x80, 0x80, 0x80, 0x80),
    ];
    for (background_2, expected_2) in backgrounds_of_row_3().into_iter().zip(expected_2) {
        assert_eq!(
            background_pixel(Mode::Rgba, background_2),
            expected_2,
            "{background_2:?}"
        );
    }
}

#[test]
fn row_3_background_pixel_in_gray_alpha() {
    let expected_2 = [
        gray_alpha_px(0, 0),
        gray_alpha_px(255, 255),
        gray_alpha_px(0, 255),
        gray_alpha_px(0x80, 0xff),
        None, // el rojo no es gris
        gray_alpha_px(0x80, 0x80),
    ];
    for (background_2, expected_2) in backgrounds_of_row_3().into_iter().zip(expected_2) {
        assert_eq!(
            background_pixel(Mode::GrayAlpha, background_2),
            expected_2,
            "{background_2:?}"
        );
    }
}

#[test]
fn row_3_background_pixel_in_gray_admits_neither_transparent_nor_alpha() {
    let expected_2 = [
        None, // sin transparente
        Some(Pixel::Gray(255)),
        Some(Pixel::Gray(0)),
        Some(Pixel::Gray(0x80)),
        None, // no es gris
        None, // gris con alfa < 255
    ];
    for (background_2, expected_2) in backgrounds_of_row_3().into_iter().zip(expected_2) {
        assert_eq!(
            background_pixel(Mode::Gray, background_2),
            expected_2,
            "{background_2:?}"
        );
    }
}

#[test]
fn row_3_background_pixel_in_indexed_uses_the_palette_indices_and_admits_no_color() {
    let expected_2 = [
        Some(Pixel::Indexed(0)),
        Some(Pixel::Indexed(31)),
        Some(Pixel::Indexed(25)),
        None,
        None,
        None,
    ];
    for (background_2, expected_2) in backgrounds_of_row_3().into_iter().zip(expected_2) {
        assert_eq!(
            background_pixel(Mode::Indexed, background_2),
            expected_2,
            "{background_2:?}"
        );
    }
    assert_eq!((BLACK_INDEX, WHITE_INDEX), (25, 31));
}

#[test]
fn row_3_the_default_background_is_white_in_gray_and_transparent_in_the_others() {
    assert_eq!(default_background(Mode::Gray), WHITE);
    for mode in [Mode::Indexed, Mode::Rgba, Mode::GrayAlpha] {
        assert_eq!(default_background(mode), TRANSPARENT, "{mode:?}");
    }
    // Y el de por defecto de cada modo lo admite el modo.
    for mode in TEST_MODES {
        assert!(background_pixel(mode, default_background(mode)).is_some());
    }
}

#[test]
fn row_3_the_form_has_a_background_group_per_mode_with_the_options_of_the_table() {
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
    let field_2 = |id: &str| spec.fields.iter().find(|f| f.id == id).expect("campo");
    let options_2 = |id: &str| match field_2(id).kind {
        FieldKind::Choice { options, default } => (options.to_vec(), default),
        ref other_2 => panic!("{id}: {other_2:?}"),
    };
    assert_eq!(field_2("modo").label, "Modo");
    assert_eq!(
        options_2("modo"),
        (vec!["Indexado", "RGBA", "Gris con alfa", "Gris"], 0)
    );
    let transparent_white_black_color = vec!["Transparente", "Blanco", "Negro", "Color…"];
    assert_eq!(
        options_2("fondo_indexado"),
        (vec!["Transparente", "Blanco", "Negro"], 0)
    );
    assert_eq!(
        options_2("fondo_rgba"),
        (transparent_white_black_color.clone(), 0)
    );
    assert_eq!(
        options_2("fondo_gris_alfa"),
        (transparent_white_black_color, 0)
    );
    assert_eq!(
        options_2("fondo_gris"),
        (vec!["Blanco", "Negro", "Color…"], 0)
    );

    // Cada grupo se ve solo con su modo y cada hexadecimal solo con «Color…» (y con su modo).
    for (background_2, mode_2, color_id, color_en) in [
        ("fondo_indexado", 0, None, 0),
        ("fondo_rgba", 1, Some("color_rgba"), 3),
        ("fondo_gris_alfa", 2, Some("color_gris_alfa"), 3),
        ("fondo_gris", 3, Some("color_gris"), 2),
    ] {
        assert_eq!(field_2(background_2).label, "Fondo");
        assert_eq!(
            field_2(background_2).shown_when,
            Some(ShowWhen {
                field: "modo",
                choice: mode_2
            }),
            "{background_2}"
        );
        if let Some(color_id) = color_id {
            assert_eq!(field_2(color_id).label, "Color");
            assert_eq!(
                field_2(color_id).shown_when,
                Some(ShowWhen {
                    field: background_2,
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
            field_2(id).kind,
            FieldKind::Hex {
                mode,
                default: GRAY_OPAQUE
            },
            "{id}"
        );
    }
    // El informe: solo lectura, cuatro filas.
    assert!(matches!(
        field_2("informe").kind,
        FieldKind::ReadOnly {
            rows: Rows::Fixed(4),
            ..
        }
    ));
}

#[test]
fn row_3_spec_from_values_reads_the_mode_group_with_each_valid_background() {
    for mode in TEST_MODES {
        for background_2 in backgrounds_valid(mode) {
            let mut d = dialog(8192);
            put_background(&mut d, mode, background_2);
            let spec = spec_de(&d)
                .unwrap_or_else(|| panic!("{mode:?} {background_2:?}: spec_from_values dio None"));
            assert_eq!(
                fields(&spec),
                (64, 64, mode, background_2),
                "{mode:?} {background_2:?}"
            );
        }
    }
}

#[test]
fn row_3_spec_from_values_reads_width_and_height() {
    let mut d = dialog(8192);
    number(&mut d, "ancho", "5");
    number(&mut d, "alto", "3");
    let spec = spec_de(&d).expect("spec");
    assert_eq!(fields(&spec), (5, 3, Mode::Indexed, TRANSPARENT));
}

#[test]
fn row_3_spec_from_values_gives_none_if_visible_fields_are_missing() {
    assert!(spec_from_values(&Values::default()).is_none());
}

#[test]
fn row_3_spec_from_values_does_not_read_the_hidden_groups_and_they_keep_their_value() {
    let mut d = dialog(8192);
    // RGBA con Negro; luego Gris con su grupo por defecto (Blanco); luego Indexado.
    put_background(&mut d, Mode::Rgba, TEST_BLACK);
    assert_eq!(
        fields(&spec_de(&d).expect("rgba")),
        (64, 64, Mode::Rgba, TEST_BLACK)
    );
    chooses(&mut d, "modo", index_of_mode(Mode::Gray));
    assert_eq!(
        fields(&spec_de(&d).expect("gris")),
        (64, 64, Mode::Gray, WHITE),
        "no lee el Negro oculto de RGBA"
    );
    chooses(&mut d, "modo", index_of_mode(Mode::Indexed));
    assert_eq!(
        fields(&spec_de(&d).expect("indexado")),
        (64, 64, Mode::Indexed, TRANSPARENT)
    );
    chooses(&mut d, "modo", index_of_mode(Mode::Rgba));
    assert_eq!(
        fields(&spec_de(&d).expect("rgba otra vez")),
        (64, 64, Mode::Rgba, TEST_BLACK),
        "el grupo oculto conservaba su elección"
    );
}

#[test]
fn row_3_in_gray_a_non_gray_color_or_with_alpha_gives_the_hex_notice_reason_and_is_invalid() {
    for (text_2, error) in [
        ("#ff0000", HexError::NotGray),
        ("#80808080", HexError::NoAlpha),
    ] {
        let mut d = dialog(8192);
        chooses(&mut d, "modo", index_of_mode(Mode::Gray));
        chooses(&mut d, "fondo_gris", 2);
        writes_color(&mut d, "color_gris", text_2);
        let reason_2 = hex_notice(&error);
        assert_eq!(d.validity("color_gris"), invalid(reason_2), "{text_2}");
        assert!(!d.is_valid(), "{text_2}");
        assert_eq!(d.values(), None, "{text_2}");
        let reason = d.reason().expect("hay motivo");
        assert!(reason.starts_with("Color: "), "{reason}");
        assert!(reason.chars().count() <= 37, "{reason}");
        assert!(spec_de(&d).is_none());
    }
}

#[test]
fn row_3_in_gray_alpha_a_non_gray_color_is_invalid_and_in_rgba_it_is_valid() {
    let mut d = dialog(8192);
    chooses(&mut d, "modo", index_of_mode(Mode::GrayAlpha));
    chooses(&mut d, "fondo_gris_alfa", 3);
    writes_color(&mut d, "color_gris_alfa", "#ff0000");
    assert_eq!(
        d.validity("color_gris_alfa"),
        invalid(hex_notice(&HexError::NotGray))
    );
    writes_color(&mut d, "color_gris_alfa", "#80808080");
    assert_eq!(d.validity("color_gris_alfa"), Some(Validity::Valid));

    let mut d = dialog(8192);
    chooses(&mut d, "modo", index_of_mode(Mode::Rgba));
    chooses(&mut d, "fondo_rgba", 3);
    writes_color(&mut d, "color_rgba", "#ff0000");
    assert_eq!(d.validity("color_rgba"), Some(Validity::Valid));
}

#[test]
fn row_3_a_short_hexadecimal_or_with_bad_letters_is_invalid_in_any_mode() {
    for mode in [Mode::Rgba, Mode::GrayAlpha, Mode::Gray] {
        for text_2 in ["#12", "zz"] {
            let mut d = dialog(8192);
            chooses(&mut d, "modo", index_of_mode(mode));
            chooses(
                &mut d,
                id_background(mode),
                index_of_background(mode, Type::Color).expect("color"),
            );
            let id = id_color(mode).expect("color");
            writes_color(&mut d, id, text_2);
            assert!(
                matches!(d.validity(id), Some(Validity::Invalid(_))),
                "{mode:?} {text_2}"
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
enum TestBox {
    Equal,
    Resets(Background, &'static str),
}

use TestBox::{Equal, Resets};

const NOTICE_GRAY_WITHOUT_TRANSPARENT: &str = "Gris sin Transparente: pasa a Blanco";
const NOTICE_INDEXED_WITHOUT_COLOR: &str = "Indexado sin Color: pasa a Transparente";
const NOTICE_COLOR_WITH_ALPHA: &str = "Color con alfa: pasa a Blanco";
const NOTICE_NON_GRAY_TO_GRAY_ALPHA: &str = "Color no gris: pasa a Transparente";
const NOTICE_NON_GRAY_TO_GRAY: &str = "Color no gris: pasa a Blanco";

const NOTICES: [&str; 5] = [
    NOTICE_GRAY_WITHOUT_TRANSPARENT,
    NOTICE_INDEXED_WITHOUT_COLOR,
    NOTICE_COLOR_WITH_ALPHA,
    NOTICE_NON_GRAY_TO_GRAY_ALPHA,
    NOTICE_NON_GRAY_TO_GRAY,
];

/// Destinos de las columnas de la tabla D1b, en su orden.
const TARGETS: [Mode; 4] = [Mode::Rgba, Mode::GrayAlpha, Mode::Gray, Mode::Indexed];

/// La tabla D1b, copiada tal cual: (nombre, fondo de origen, modos en los que puede existir ese
/// fondo, casillas A RGBA / A Gris con alfa / A Gris / A Indexado).
fn table_d_1_b() -> Vec<(&'static str, Background, Vec<Mode>, [TestBox; 4])> {
    let all_2 = vec![Mode::Rgba, Mode::GrayAlpha, Mode::Gray, Mode::Indexed];
    let color = Background::Color;
    vec![
        (
            "transparente",
            TRANSPARENT,
            vec![Mode::Rgba, Mode::GrayAlpha, Mode::Indexed],
            [
                Equal,
                Equal,
                Resets(WHITE, NOTICE_GRAY_WITHOUT_TRANSPARENT),
                Equal,
            ],
        ),
        ("blanco", WHITE, all_2.clone(), [Equal, Equal, Equal, Equal]),
        ("negro", TEST_BLACK, all_2, [Equal, Equal, Equal, Equal]),
        (
            "color gris con alfa 255",
            color(GRAY_OPAQUE),
            vec![Mode::Rgba, Mode::GrayAlpha, Mode::Gray],
            [
                Equal,
                Equal,
                Equal,
                Resets(TRANSPARENT, NOTICE_INDEXED_WITHOUT_COLOR),
            ],
        ),
        (
            "color gris con alfa < 255",
            color(GRAY_MIDDLE),
            vec![Mode::Rgba, Mode::GrayAlpha],
            [
                Equal,
                Equal,
                Resets(WHITE, NOTICE_COLOR_WITH_ALPHA),
                Resets(TRANSPARENT, NOTICE_INDEXED_WITHOUT_COLOR),
            ],
        ),
        (
            "color no gris",
            color(TEST_RED),
            vec![Mode::Rgba],
            [
                Equal,
                Resets(TRANSPARENT, NOTICE_NON_GRAY_TO_GRAY_ALPHA),
                Resets(WHITE, NOTICE_NON_GRAY_TO_GRAY),
                Resets(TRANSPARENT, NOTICE_INDEXED_WITHOUT_COLOR),
            ],
        ),
        (
            "color no gris con alfa",
            color(RED_MIDDLE),
            vec![Mode::Rgba],
            [
                Equal,
                Resets(TRANSPARENT, NOTICE_NON_GRAY_TO_GRAY_ALPHA),
                Resets(WHITE, NOTICE_NON_GRAY_TO_GRAY),
                Resets(TRANSPARENT, NOTICE_INDEXED_WITHOUT_COLOR),
            ],
        ),
    ]
}

/// Lo que dio `carry_background`, sin suponer derives de `Carry`.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Seen {
    Kept(Background),
    Reset(Background, String),
}

fn translates(from: Mode, background_2: Background, to: Mode) -> Seen {
    match carry_background(from, background_2, to) {
        Carry::Kept(b) => Seen::Kept(b),
        Carry::Reset { to, notice } => Seen::Reset(to, notice.to_string()),
    }
}

fn expected_2(box_value: TestBox, background_2: Background) -> Seen {
    match box_value {
        Equal => Seen::Kept(background_2),
        Resets(a, notice) => Seen::Reset(a, notice.to_string()),
    }
}

#[test]
fn row_4_carry_background_is_exactly_the_table_d1b() {
    for (name_2, background_2, sources, boxes_2) in table_d_1_b() {
        for from in sources {
            for (to, box_value) in TARGETS.into_iter().zip(boxes_2) {
                if from == to {
                    continue;
                }
                assert_eq!(
                    translates(from, background_2, to),
                    expected_2(box_value, background_2),
                    "{name_2}: de {from:?} a {to:?}"
                );
            }
        }
    }
}

#[test]
fn row_4_with_the_same_mode_the_background_is_kept() {
    for mode in TEST_MODES {
        for background_2 in backgrounds_valid(mode) {
            assert_eq!(
                translates(mode, background_2, mode),
                Seen::Kept(background_2),
                "{mode:?} {background_2:?}"
            );
        }
    }
}

#[test]
fn row_4_white_and_black_are_never_reset() {
    for from in TEST_MODES {
        for to in TEST_MODES {
            for background_2 in [WHITE, TEST_BLACK] {
                assert_eq!(
                    translates(from, background_2, to),
                    Seen::Kept(background_2),
                    "{from:?} → {to:?} {background_2:?}"
                );
            }
        }
    }
}

#[test]
fn row_4_what_is_kept_is_admitted_by_the_new_mode_and_what_is_reset_goes_to_the_default() {
    for (name_2, background_2, sources, _) in table_d_1_b() {
        for from in sources {
            for to in TEST_MODES {
                match translates(from, background_2, to) {
                    Seen::Kept(b) => assert!(
                        background_pixel(to, b).is_some(),
                        "{name_2}: {from:?} → {to:?}: conserva un fondo que el modo no admite"
                    ),
                    Seen::Reset(a, notice) => {
                        assert_eq!(a, default_background(to), "{name_2}: {from:?} → {to:?}");
                        assert!(background_pixel(to, a).is_some());
                        assert!(!notice.is_empty());
                    }
                }
            }
        }
    }
}

#[test]
fn row_4_the_notices_say_the_background_there_was_and_which_one_it_goes_back_to_and_fit() {
    let mut seen = HashSet::new();
    for (_, background_2, sources, _) in table_d_1_b() {
        for from in sources {
            for to in TEST_MODES {
                if let Seen::Reset(a, notice) = translates(from, background_2, to) {
                    seen.insert(notice.clone());
                    assert!(notice.chars().count() <= 42, "{notice}");
                    assert!(in_the_set(&notice), "{notice}");
                    // Qué fondo había: Transparente o Color (con el origen).
                    let was = match background_2 {
                        Background::Transparent => "Transparente",
                        Background::Color(_) => "Color",
                        other_2 => panic!("{other_2:?} nunca se reinicia"),
                    };
                    assert!(notice.contains(was), "{notice} no dice que había {was}");
                    // A cuál vuelve.
                    let returns = match a {
                        Background::Transparent => "pasa a Transparente",
                        Background::White => "pasa a Blanco",
                        other_2 => panic!("{other_2:?} no es un fondo de reinicio"),
                    };
                    assert!(notice.ends_with(returns), "{notice} no acaba en {returns}");
                }
            }
        }
    }
    let expected_2_2_2: HashSet<String> = NOTICES.iter().map(|s| s.to_string()).collect();
    assert_eq!(
        seen, expected_2_2_2,
        "salen los cinco avisos de la tabla y ninguno más"
    );
}

#[test]
fn row_4_a_non_gray_color_with_alpha_says_non_gray_color() {
    assert_eq!(
        translates(Mode::Rgba, Background::Color(RED_MIDDLE), Mode::Gray),
        Seen::Reset(WHITE, NOTICE_NON_GRAY_TO_GRAY.to_string())
    );
    assert_eq!(
        translates(Mode::Rgba, Background::Color(RED_MIDDLE), Mode::GrayAlpha),
        Seen::Reset(TRANSPARENT, NOTICE_NON_GRAY_TO_GRAY_ALPHA.to_string())
    );
}

// --- sync_form: la tabla aplicada en el diálogo (D8) ---

fn step(d: &mut Dialog, sync: &mut FormSync, event_2: Event) {
    assert_eq!(d.event(event_2), Reaction::None);
    sync_form(d, sync);
}

/// Un diálogo con el modo `mode` y su fondo `background_2`, como lo dejaría el usuario (un evento y un
/// `sync_form` por paso), y su `FormSync`.
fn dialog_synchronized(mode: Mode, background_2: Background) -> (Dialog, FormSync) {
    let mut d = dialog(8192);
    let mut sync = FormSync::default();
    sync_form(&mut d, &mut sync);
    step(
        &mut d,
        &mut sync,
        Event::Choice {
            field: "modo",
            index: index_of_mode(mode),
        },
    );
    step(
        &mut d,
        &mut sync,
        Event::Choice {
            field: id_background(mode),
            index: index_of_background(mode, type_of(background_2)).expect("fondo del modo"),
        },
    );
    if let Background::Color(color) = background_2 {
        let id = id_color(mode).expect("color");
        step(
            &mut d,
            &mut sync,
            Event::Hex {
                field: id,
                text: color.to_string(),
            },
        );
        step(
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
        step(
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

fn lines_of_the_report(d: &Dialog) -> Vec<String> {
    text_of_the_report(d).lines().map(str::to_string).collect()
}

#[test]
fn row_4_sync_form_applies_the_table_when_changing_mode_and_warns_in_the_report() {
    for (name_2, background_2, sources, boxes_2) in table_d_1_b() {
        for from in sources {
            for (to, box_value) in TARGETS.into_iter().zip(boxes_2) {
                if from == to {
                    continue;
                }
                let (mut d, mut sync) = dialog_synchronized(from, background_2);
                assert_eq!(
                    fields(&spec_de(&d).expect("antes")),
                    (64, 64, from, background_2),
                    "{name_2}: partida en {from:?}"
                );
                assert!(
                    NOTICES.iter().all(|a| !text_of_the_report(&d).contains(*a)),
                    "{name_2}: sin avisos antes de cambiar"
                );

                step(
                    &mut d,
                    &mut sync,
                    Event::Choice {
                        field: "modo",
                        index: index_of_mode(to),
                    },
                );
                let spec = spec_de(&d)
                    .unwrap_or_else(|| panic!("{name_2}: {from:?} → {to:?}: sin valores válidos"));
                let (background_expected, notice) = match box_value {
                    Equal => (background_2, None),
                    Resets(a, notice) => (a, Some(notice)),
                };
                assert_eq!(
                    fields(&spec),
                    (64, 64, to, background_expected),
                    "{name_2}: de {from:?} a {to:?}"
                );
                // El informe es el de `report_text` con el aviso (o sin él).
                assert_eq!(
                    text_of_the_report(&d),
                    report_text(Some((64, 64)), to, notice),
                    "{name_2}: de {from:?} a {to:?}"
                );
                // El hexadecimal del grupo nuevo lleva el color que viaja.
                if let (Background::Color(color), Some(id)) = (background_expected, id_color(to)) {
                    assert_eq!(d.text(id), Some(color.to_string()), "{name_2}");
                }

                // El aviso dura hasta que cambia otro valor del formulario.
                assert!(
                    !sync_form(&mut d, &mut sync),
                    "sin cambios no hay nada que pintar"
                );
                if let Some(notice) = notice {
                    assert!(lines_of_the_report(&d).iter().any(|l| l == notice));
                    step(
                        &mut d,
                        &mut sync,
                        Event::Step {
                            field: "ancho",
                            delta: 1,
                        },
                    );
                    assert!(
                        NOTICES.iter().all(|a| !text_of_the_report(&d).contains(*a)),
                        "{name_2}: el aviso no sobrevive a otro cambio"
                    );
                    assert_eq!(
                        text_of_the_report(&d),
                        report_text(Some((65, 64)), to, None)
                    );
                }
            }
        }
    }
}

#[test]
fn row_4_sync_form_returns_true_when_something_to_paint_changes_and_false_otherwise() {
    let mut d = dialog(8192);
    let mut sync = FormSync::default();
    sync_form(&mut d, &mut sync);
    assert!(!sync_form(&mut d, &mut sync), "sin eventos");
    // Un cambio de tamaño cambia la memoria del informe.
    number(&mut d, "ancho", "4097");
    number(&mut d, "alto", "4096");
    assert!(sync_form(&mut d, &mut sync), "el informe cambió");
    assert!(!sync_form(&mut d, &mut sync));
    // Un cambio de modo traslada el fondo y cambia el informe (RGBA no lleva la línea de la paleta).
    chooses(&mut d, "modo", index_of_mode(Mode::Rgba));
    assert!(sync_form(&mut d, &mut sync));
    assert!(!sync_form(&mut d, &mut sync));
}

#[test]
fn row_4_sync_form_with_invalid_width_or_height_says_memory_dash() {
    let mut d = dialog(8192);
    let mut sync = FormSync::default();
    sync_form(&mut d, &mut sync);
    number(&mut d, "ancho", "abc");
    sync_form(&mut d, &mut sync);
    assert_eq!(
        text_of_the_report(&d),
        report_text(None, Mode::Indexed, None),
        "ancho sin valor"
    );
    assert!(text_of_the_report(&d).starts_with("Memoria: —"));
    number(&mut d, "ancho", "10");
    number(&mut d, "alto", "");
    sync_form(&mut d, &mut sync);
    assert_eq!(
        text_of_the_report(&d),
        report_text(None, Mode::Indexed, None),
        "alto sin valor"
    );
    number(&mut d, "alto", "20");
    sync_form(&mut d, &mut sync);
    assert_eq!(
        text_of_the_report(&d),
        report_text(Some((10, 20)), Mode::Indexed, None)
    );
}

// ---------------------------------------------------------------------------------------
// Fila 5: paleta inicial e imagen creada.
// ---------------------------------------------------------------------------------------

/// Los 32 valores literales de D2 (la entrada 0 es `#00000000`).
const PALETTE_D_2: [&str; 32] = [
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

fn palette_literal() -> Vec<Rgba> {
    PALETTE_D_2
        .iter()
        .map(|h| Rgba::from_hex(h).unwrap_or_else(|e| panic!("{h}: {e:?}")))
        .collect()
}

#[test]
fn row_5_the_initial_palette_has_the_32_literal_values_of_d2() {
    let palette = initial_palette();
    assert_eq!(PALETTE_LEN, 32);
    assert_eq!(palette.len(), PALETTE_LEN);
    assert_eq!(palette.colors(), palette_literal().as_slice());
    assert_eq!(
        palette.get(0),
        Ok(Rgba::new(0, 0, 0, 0)),
        "la entrada 0 es #00000000"
    );
}

#[test]
fn row_5_entry_0_is_the_transparent_index_and_black_and_white_are_at_25_and_31() {
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
fn row_5_the_32_entries_are_different() {
    let palette = initial_palette();
    let different: HashSet<Rgba> = palette.colors().iter().copied().collect();
    assert_eq!(different.len(), 32);
}

#[test]
fn row_5_two_calls_give_the_same_palette_and_from_colors_accepts_it() {
    let a = initial_palette();
    let b = initial_palette();
    assert_eq!(a, b);
    assert_eq!(a.colors(), b.colors());
    let rebuilt = Palette::from_colors(a.colors().iter().copied()).expect("la acepta");
    assert_eq!(rebuilt.colors(), a.colors());
    // Y con ella se puede crear una imagen indexada.
    Image::new_indexed(2, 2, a).expect("imagen indexada");
}

#[test]
fn row_5_entries_1_to_24_follow_the_hsl_rule_and_25_to_31_the_integer_one() {
    let palette = initial_palette();
    for (row, l) in [0.25f32, 0.50, 0.75].into_iter().enumerate() {
        for k in 0..8usize {
            let expected_2 = Hsl::new(45.0 * k as f32, 1.0, l).to_rgba(255);
            let index_2 = 1 + 8 * row + k;
            assert_eq!(
                palette.colors()[index_2],
                expected_2,
                "entrada {index_2} (fila {row}, k {k})"
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
fn row_5_the_created_image_has_the_requested_size_and_mode_and_all_the_background_pixels() {
    for (width, height) in [(5u32, 3u32), (64, 64)] {
        for mode in TEST_MODES {
            for background_2 in backgrounds_valid(mode) {
                let spec = spec_new(width, height, mode, background_2);
                let image = builds_real(&spec);
                let expected_2 = background_pixel(mode, background_2).expect("fondo válido");
                assert_eq!((image.width(), image.height()), (width, height));
                assert_eq!(image.mode(), mode);
                for y in 0..height {
                    for x in 0..width {
                        assert_eq!(
                            image.get_pixel(x, y),
                            Ok(expected_2),
                            "{width}×{height} {mode:?} {background_2:?} en ({x}, {y})"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn row_5_in_the_large_image_the_background_the_four_corners_and_the_center_are_valid() {
    let (width, height) = (1000u32, 600u32);
    for mode in TEST_MODES {
        for background_2 in backgrounds_valid(mode) {
            let spec = spec_new(width, height, mode, background_2);
            let image = builds_real(&spec);
            let expected_2 = background_pixel(mode, background_2).expect("fondo válido");
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
                    Ok(expected_2),
                    "{mode:?} {background_2:?} en ({x}, {y})"
                );
            }
        }
    }
}

#[test]
fn row_5_the_indexed_one_carries_the_d2_palette_with_white_black_and_transparent_at_their_indices()
{
    for background_2 in [TRANSPARENT, WHITE, TEST_BLACK] {
        let image = builds_real(&spec_new(7, 5, Mode::Indexed, background_2));
        assert_eq!(
            image.palette(),
            Some(&initial_palette()),
            "{background_2:?}"
        );
        let index_2 = match background_2 {
            Background::Transparent => 0,
            Background::White => 31,
            Background::Black => 25,
            Background::Color(_) => unreachable!(),
        };
        assert_eq!(
            image.get_pixel(3, 2),
            Ok(Pixel::Indexed(index_2)),
            "{background_2:?}"
        );
        let palette = image.palette().expect("paleta");
        let visible = palette.resolved(index_2).expect("índice");
        let expected_2 = match background_2 {
            Background::Transparent => Rgba::new(0, 0, 0, 0),
            Background::White => Rgba::opaque(255, 255, 255),
            _ => Rgba::opaque(0, 0, 0),
        };
        assert_eq!(visible, expected_2, "{background_2:?}");
    }
    // Los demás modos no llevan paleta.
    for mode in [Mode::Rgba, Mode::GrayAlpha, Mode::Gray] {
        let image = builds_real(&spec_new(2, 2, mode, default_background(mode)));
        assert!(image.palette().is_none(), "{mode:?}");
    }
}

#[test]
fn row_5_wrapped_in_document_the_image_has_no_path_or_history() {
    // Antes (unidades 19 a 14a-1) se afirmaba también `document == canvas`: la segunda copia, la
    // del lienzo, desaparece (decisión 10 de la 14a-2).
    for mode in TEST_MODES {
        let image = builds_real(&spec_new(9, 4, mode, default_background(mode)));
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

fn memory(width: u32, height: u32, mode: Mode) -> (u64, u64) {
    let m = memory_estimate(width, height, mode);
    (m.ram_bytes, m.gpu_bytes)
}

#[test]
fn row_6_memory_estimate_is_n_times_bpp_plus_4_in_ram_and_4_n_in_gpu() {
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
        for mode in TEST_MODES {
            let n = u64::from(width) * u64::from(height);
            assert_eq!(
                memory(width, height, mode),
                (n * (bpp(mode) + 4), 4 * n),
                "{width}×{height} {mode:?}"
            );
        }
    }
}

#[test]
fn row_6_memory_estimate_gives_the_figures_of_the_spec() {
    // RAM = N · (bytes por píxel + 4) desde la 14a-2 (antes N · (2 · bytes por píxel + 4)).
    assert_eq!(memory(1024, 1024, Mode::Rgba), (8 * MIB, 4 * MIB));
    assert_eq!(memory(4096, 4096, Mode::Rgba), (128 * MIB, 64 * MIB));
    assert_eq!(memory(8192, 8192, Mode::Rgba), (512 * MIB, 256 * MIB));
    assert_eq!(memory(8192, 8192, Mode::GrayAlpha), (384 * MIB, 256 * MIB));
    assert_eq!(memory(8192, 8192, Mode::Gray), (320 * MIB, 256 * MIB));
    assert_eq!(memory(8192, 8192, Mode::Indexed), (320 * MIB, 256 * MIB));
    assert_eq!(memory(4097, 4096, Mode::Rgba), (134_250_496, 67_125_248));
}

const NOTICE_OF_SIZE: &str = "aviso: más de 16777216 px; puede ir lento";
const LINE_OF_PALETTE: &str = "Paleta: 32 colores, el 0 transparente";

fn report(size: Option<(u32, u32)>, mode: Mode, notice: Option<&str>) -> String {
    report_text(size, mode, notice)
}

#[test]
fn row_6_report_text_less_than_1_mib_at_64_and_256_squared() {
    assert_eq!(
        report(Some((64, 64)), Mode::Rgba, None),
        "Memoria: menos de 1 MiB"
    );
    // 256² RGBA son 0,5 MiB de RAM: todavía menos de 1.
    assert_eq!(
        report(Some((256, 256)), Mode::Rgba, None),
        "Memoria: menos de 1 MiB"
    );
    assert_eq!(
        report(Some((64, 64)), Mode::Indexed, None),
        format!("Memoria: menos de 1 MiB\n{LINE_OF_PALETTE}")
    );
}

#[test]
fn row_6_report_text_gives_the_exact_texts_of_d4() {
    assert_eq!(
        report(Some((1024, 1024)), Mode::Rgba, None),
        "Memoria: ~8 MiB + 4 MiB de GPU"
    );
    assert_eq!(
        report(Some((4096, 4096)), Mode::Rgba, None),
        "Memoria: ~128 MiB + 64 MiB de GPU",
        "4096² no avisa"
    );
    assert_eq!(
        report(Some((8192, 8192)), Mode::Rgba, None),
        format!("Memoria: ~512 MiB + 256 MiB de GPU\n{NOTICE_OF_SIZE}")
    );
    assert_eq!(
        report(Some((8192, 8192)), Mode::GrayAlpha, None),
        format!("Memoria: ~384 MiB + 256 MiB de GPU\n{NOTICE_OF_SIZE}")
    );
    assert_eq!(
        report(Some((8192, 8192)), Mode::Gray, None),
        format!("Memoria: ~320 MiB + 256 MiB de GPU\n{NOTICE_OF_SIZE}")
    );
    assert_eq!(
        report(Some((8192, 8192)), Mode::Indexed, None),
        format!("Memoria: ~320 MiB + 256 MiB de GPU\n{LINE_OF_PALETTE}\n{NOTICE_OF_SIZE}")
    );
}

#[test]
fn row_6_the_rounding_is_upward() {
    // 4097 × 4096 RGBA: 128,01 MiB de RAM y 64,004 de GPU.
    assert_eq!(
        report(Some((4097, 4096)), Mode::Rgba, None),
        format!("Memoria: ~129 MiB + 65 MiB de GPU\n{NOTICE_OF_SIZE}")
    );
}

#[test]
fn row_6_with_width_or_height_without_a_value_it_says_memory_dash_and_does_not_warn_about_size() {
    assert_eq!(report(None, Mode::Rgba, None), "Memoria: —");
    assert_eq!(
        report(None, Mode::Indexed, None),
        format!("Memoria: —\n{LINE_OF_PALETTE}")
    );
    assert!(!report(None, Mode::Gray, None).contains("aviso"));
}

#[test]
fn row_6_the_size_notice_appears_only_with_more_than_2_to_the_24_pixels() {
    assert_eq!(WARN_PIXELS, 16_777_216);
    for ((width, height), warns) in [
        ((4096u32, 4096u32), false),
        ((8192, 2048), false),
        ((4097, 4096), true),
        ((8192, 2049), true),
        ((8192, 8192), true),
        ((1, 1), false),
    ] {
        for mode in TEST_MODES {
            let text_2 = report(Some((width, height)), mode, None);
            assert_eq!(
                text_2.lines().any(|l| l == NOTICE_OF_SIZE),
                warns,
                "{width}×{height} {mode:?}: {text_2}"
            );
        }
    }
}

#[test]
fn row_6_the_size_notice_does_not_prevent_creating() {
    let mut d = dialog(8192);
    number(&mut d, "ancho", "4097");
    number(&mut d, "alto", "4096");
    assert!(d.is_valid(), "{:?}", d.reason());
    let spec = spec_de(&d).expect("se puede crear");
    assert_eq!((spec.width, spec.height), (4097, 4096));
    assert_eq!(size_error(spec.width, spec.height, spec.mode), None);
    // Y el formulario lo acepta con `Crear` (Accept).
    assert!(matches!(d.event(Event::Accept), Reaction::Accepted(_)));
}

#[test]
fn row_6_with_a_background_notice_the_report_puts_it_last() {
    let text_2 = report(
        Some((4097, 4096)),
        Mode::Indexed,
        Some(NOTICE_INDEXED_WITHOUT_COLOR),
    );
    let lines: Vec<&str> = text_2.lines().collect();
    assert_eq!(lines.len(), 4, "{text_2}");
    assert!(lines[0].starts_with("Memoria: "));
    assert_eq!(lines[1], LINE_OF_PALETTE);
    assert_eq!(lines[2], NOTICE_OF_SIZE);
    assert_eq!(lines[3], NOTICE_INDEXED_WITHOUT_COLOR);

    // Sin la línea de la paleta ni el aviso de tamaño, el aviso de fondo va tras la memoria.
    assert_eq!(
        report(
            Some((64, 64)),
            Mode::Rgba,
            Some(NOTICE_GRAY_WITHOUT_TRANSPARENT)
        ),
        format!("Memoria: menos de 1 MiB\n{NOTICE_GRAY_WITHOUT_TRANSPARENT}")
    );
    assert_eq!(
        report(None, Mode::Gray, Some(NOTICE_COLOR_WITH_ALPHA)),
        format!("Memoria: —\n{NOTICE_COLOR_WITH_ALPHA}")
    );
}

#[test]
fn row_6_the_report_has_at_most_4_lines_of_42_characters_inside_the_set() {
    let sizes = [
        None,
        Some((1u32, 1u32)),
        Some((64, 64)),
        Some((256, 256)),
        Some((1024, 1024)),
        Some((4096, 4096)),
        Some((4097, 4096)),
        Some((8192, 8192)),
    ];
    let mut notices: Vec<Option<&str>> = vec![None];
    notices.extend(NOTICES.iter().map(|a| Some(*a)));
    for size in sizes {
        for mode in TEST_MODES {
            for notice in &notices {
                let text_2 = report(size, mode, *notice);
                let lines: Vec<&str> = text_2.lines().collect();
                assert!(lines.len() <= 4, "{size:?} {mode:?} {notice:?}: {text_2}");
                for line in &lines {
                    assert!(
                        line.chars().count() <= 42,
                        "{size:?} {mode:?} {notice:?}: {line:?}"
                    );
                }
                assert!(
                    lines.iter().all(|l| in_the_set(l)),
                    "{size:?} {mode:?} {notice:?}: {text_2}"
                );
                // El orden de D11: memoria, paleta (solo indexado), tamaño, fondo.
                assert!(lines[0].starts_with("Memoria: "), "{text_2}");
                let palette = lines.iter().position(|l| *l == LINE_OF_PALETTE);
                assert_eq!(palette.is_some(), mode == Mode::Indexed, "{text_2}");
                if let Some(p) = palette {
                    assert_eq!(p, 1, "{text_2}");
                }
                if let Some(t) = lines.iter().position(|l| *l == NOTICE_OF_SIZE) {
                    assert_eq!(t, if mode == Mode::Indexed { 2 } else { 1 }, "{text_2}");
                }
                if let Some(a) = notice {
                    assert_eq!(lines.last().copied(), Some(*a), "{text_2}");
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
enum Call {
    Image2,
    Probe(usize),
}

/// Dónde falla el falso.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Failure {
    TestNone,
    /// Falla la reserva de la imagen del documento.
    Image2,
    /// Falla la reserva de la subida a baldosas (`probe`). Desde la 14a-2 ya no hay una segunda
    /// imagen (la copia del lienzo): quedan estos dos pasos.
    Probe,
}

const FAILURES: [Failure; 2] = [Failure::Image2, Failure::Probe];

/// Un `ImageAlloc` que cuenta las llamadas, delega en el real y falla donde se le dice.
struct False {
    failure: Failure,
    calls: Vec<Call>,
}

impl False {
    fn new_2(failure: Failure) -> False {
        False {
            failure,
            calls: Vec::new(),
        }
    }
}

impl ImageAlloc for False {
    fn image(&mut self, spec: &NewImageSpec) -> Result<Image, ImageError> {
        self.calls.push(Call::Image2);
        if self.failure == Failure::Image2 {
            return Err(ImageError::TooLarge {
                width: spec.width,
                height: spec.height,
            });
        }
        SystemAlloc.image(spec)
    }

    fn probe(&mut self, bytes: usize) -> bool {
        self.calls.push(Call::Probe(bytes));
        self.failure != Failure::Probe
    }
}

fn calls_expected(failure: Failure, bytes_of_the_probe: usize) -> Vec<Call> {
    match failure {
        Failure::Image2 => vec![Call::Image2],
        Failure::Probe | Failure::TestNone => vec![Call::Image2, Call::Probe(bytes_of_the_probe)],
    }
}

#[test]
fn row_7_without_failure_the_fake_sees_an_image_and_a_probe_of_4_n_bytes_and_gives_ok() {
    for mode in TEST_MODES {
        let spec = spec_new(100, 50, mode, default_background(mode));
        let mut false_value = False::new_2(Failure::TestNone);
        let image = builds(&spec, &mut false_value).expect("sin fallo da Ok");
        assert_eq!(
            false_value.calls,
            calls_expected(Failure::TestNone, 4 * 100 * 50),
            "{mode:?}"
        );
        assert_eq!((image.width(), image.height()), (100, 50));
    }
}

#[test]
fn row_7_each_failure_gives_err_memory_with_the_bytes_of_the_estimate_and_nothing_else_is_called() {
    for mode in TEST_MODES {
        for failure in FAILURES {
            let spec = spec_new(100, 50, mode, default_background(mode));
            let mut false_value = False::new_2(failure);
            let result = build_new_image(&spec, &mut false_value);
            match result {
                Err(NewImageError::Memory { bytes }) => assert_eq!(
                    bytes,
                    memory_estimate(100, 50, mode).ram_bytes,
                    "{mode:?} {failure:?}"
                ),
                Err(NewImageError::Size(text_2)) => panic!("{mode:?} {failure:?}: Size({text_2})"),
                Ok(_) => panic!("{mode:?} {failure:?}: debía fallar y dio Ok (ninguna imagen)"),
            }
            assert_eq!(
                false_value.calls,
                calls_expected(failure, 4 * 100 * 50),
                "{mode:?} {failure:?}: orden y nada más tras el fallo"
            );
        }
    }
}

#[test]
fn row_7_the_probe_receives_the_bytes_of_the_conversion_to_rgba_of_the_upload() {
    for (width, height) in [(1u32, 1u32), (7, 3), (300, 200)] {
        let spec = spec_new(width, height, Mode::Gray, default_background(Mode::Gray));
        let mut false_value = False::new_2(Failure::TestNone);
        builds(&spec, &mut false_value).expect("Ok");
        let bytes = 4 * width as usize * height as usize;
        assert_eq!(false_value.calls.last(), Some(&Call::Probe(bytes)));
    }
}

#[test]
fn row_7_with_the_real_one_at_8192_by_8192_rgba_it_gives_ok() {
    // Una imagen de 256 MiB y un `probe` de 256 MiB: lo más pesado de la unidad. Fondo
    // transparente (ceros): no hace falta rellenar.
    let spec = spec_new(8192, 8192, Mode::Rgba, TRANSPARENT);
    let image = builds(&spec, &mut SystemAlloc).expect("con el real, Ok");
    assert_eq!((image.width(), image.height()), (8192, 8192));
    assert_eq!(
        image.get_pixel(8191, 8191),
        Ok(Pixel::Rgba(Rgba::TRANSPARENT))
    );
}

// --- Sesión y lienzo intactos tras un fallo ---

/// Un banco mínimo de egui para el lienzo (sin ventana ni GPU): ejecuta fotogramas y anota lo que
/// el lienzo dejó.
struct Canvas2 {
    ctx: Context,
    canvas: Canvas,
}

const SCREEN: eframe::egui::Vec2 = vec2(800.0, 600.0);

impl Canvas2 {
    fn new_2(image_2: Image) -> Canvas2 {
        let mut canvas_2 = Canvas2 {
            ctx: Context::default(),
            canvas: Canvas::new(),
        };
        canvas_2.canvas.set_owned_image(image_2);
        canvas_2.frame(vec![]);
        canvas_2
    }

    /// Un fotograma: la información del lienzo, la textura y si se subió una en este fotograma.
    fn frame(&mut self, events: Vec<InputEvent>) -> TestSnapshot {
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SCREEN)),
            events,
            ..RawInput::default()
        };
        let mut info = None;
        let mut output = self.ctx.run_ui(input, |ui| {
            info = Some(self.canvas.show(ui));
        });
        let texture_2 = self.canvas.texture_id();
        let uploaded = texture_2.is_some_and(|id| output.textures_delta.set.contains_key(&id));
        output.textures_delta.clear();
        TestSnapshot {
            info: info.expect("el lienzo se dibuja"),
            texture_2,
            uploaded,
        }
    }

    /// La rueda sobre el centro: cambia el zoom (la vista deja de ser la de ajuste).
    fn zooms_out(&mut self) {
        let center = pos2(400.0, 300.0);
        self.frame(vec![InputEvent::PointerMoved(center)]);
        self.frame(vec![InputEvent::MouseWheel {
            unit: MouseWheelUnit::Line,
            delta: vec2(0.0, -1.0),
            phase: TouchPhase::Move,
            source: MouseWheelSource::Unknown,
            modifiers: Modifiers::NONE,
        }]);
    }
}

#[derive(Clone, Debug, PartialEq)]
struct TestSnapshot {
    info: CanvasInfo,
    texture_2: Option<eframe::egui::TextureId>,
    uploaded: bool,
}

/// Todo lo observable de la sesión que un fallo no debe tocar.
#[derive(Clone, Debug, PartialEq)]
struct SnapshotOfSession {
    image: Image,
    state_id: u64,
    can_undo: bool,
    can_redo: bool,
    path: Option<std::path::PathBuf>,
    modified: bool,
    choice: ColorChoice,
}

fn snapshot_of(session: &Session) -> SnapshotOfSession {
    SnapshotOfSession {
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

/// Una sesión con un documento RGBA 6×4 y, si `modified`, un paso de historia sin guardar.
fn session_2(modified: bool) -> Session {
    let image = Image::new(6, 4, Mode::Rgba).expect("imagen");
    let mut session = Session::sample(Document::new(image));
    if modified {
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
enum NotCreate {
    Unsaved,
    TestMemory(u64),
}

/// El recorrido de `App` (D8) sin egui: con cambios y sin el descarte recordado, `Unsaved` antes de
/// construir nada; si no, `build_new_image` y, solo si salió bien, `Session::new_image` y el
/// lienzo con la imagen del documento (`Canvas::set_image`; ya no hay una copia aparte, 14a-2).
fn creates(
    session: &mut Session,
    canvas_2: &mut Canvas2,
    spec: &NewImageSpec,
    alloc: &mut dyn ImageAlloc,
    discard: bool,
) -> Result<(), NotCreate> {
    if session.modified() && !discard {
        return Err(NotCreate::Unsaved);
    }
    let image = match build_new_image(spec, alloc) {
        Ok(image) => image,
        Err(NewImageError::Memory { bytes }) => return Err(NotCreate::TestMemory(bytes)),
        Err(NewImageError::Size(text_2)) => panic!("Size({text_2})"),
    };
    match session.new_image(image, discard) {
        Ok(()) => {
            let of_the_document = session
                .document
                .image(ImageId::from_raw(0))
                .expect("imagen");
            canvas_2.canvas.set_image(of_the_document);
            Ok(())
        }
        Err(FileError::Unsaved) => Err(NotCreate::Unsaved),
        Err(other_2) => panic!("{other_2:?}"),
    }
}

#[test]
fn row_7_a_failure_leaves_the_session_and_the_canvas_equal() {
    for failure in FAILURES {
        let mut session = session_2(false);
        let mut canvas_2 = Canvas2::new_2(
            session
                .document
                .image(ImageId::from_raw(0))
                .expect("imagen")
                .clone(),
        );
        canvas_2.zooms_out();
        let before_session = snapshot_of(&session);
        let before_canvas = canvas_2.frame(vec![]);

        let spec = spec_new(30, 20, Mode::Indexed, TEST_BLACK);
        let mut false_value = False::new_2(failure);
        let result = creates(&mut session, &mut canvas_2, &spec, &mut false_value, false);
        assert_eq!(
            result,
            Err(NotCreate::TestMemory(
                memory_estimate(30, 20, Mode::Indexed).ram_bytes
            )),
            "{failure:?}"
        );

        assert_eq!(snapshot_of(&session), before_session, "{failure:?}: sesión");
        let after_canvas = canvas_2.frame(vec![]);
        assert_eq!(after_canvas, before_canvas, "{failure:?}: lienzo");
        assert!(
            !after_canvas.uploaded,
            "{failure:?}: no se volvió a subir nada"
        );
    }
}

#[test]
fn row_7_on_success_session_and_canvas_are_replaced_once() {
    let mut session = session_2(false);
    let mut canvas_2 = Canvas2::new_2(
        session
            .document
            .image(ImageId::from_raw(0))
            .expect("imagen")
            .clone(),
    );
    let old_2 = canvas_2.frame(vec![]).texture_2;
    let spec = spec_new(30, 20, Mode::Indexed, TEST_BLACK);
    let mut false_value = False::new_2(Failure::TestNone);
    assert_eq!(
        creates(&mut session, &mut canvas_2, &spec, &mut false_value, false),
        Ok(())
    );
    assert_eq!(
        false_value.calls.len(),
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
    let first = canvas_2.frame(vec![]);
    assert!(first.uploaded, "el lienzo sube la imagen nueva");
    assert_ne!(first.texture_2, old_2);
    assert!(!canvas_2.frame(vec![]).uploaded, "una sola subida");
}

// ---------------------------------------------------------------------------------------
// Fila 8: el aviso de 5 s tras crear.
// ---------------------------------------------------------------------------------------

#[test]
fn row_8_describe_new_says_size_mode_and_the_32_colors_only_for_the_indexed_one() {
    assert_eq!(
        describe_new(&spec_new(64, 64, Mode::Indexed, TRANSPARENT)),
        "imagen nueva 64×64 · indexado · 32 colores"
    );
    assert_eq!(
        describe_new(&spec_new(64, 64, Mode::Rgba, TRANSPARENT)),
        "imagen nueva 64×64 · RGBA"
    );
    assert_eq!(
        describe_new(&spec_new(5, 3, Mode::GrayAlpha, TEST_BLACK)),
        "imagen nueva 5×3 · gris con alfa"
    );
    assert_eq!(
        describe_new(&spec_new(8192, 4097, Mode::Gray, WHITE)),
        "imagen nueva 8192×4097 · gris"
    );
    for mode in TEST_MODES {
        let text_2 = describe_new(&spec_new(10, 20, mode, default_background(mode)));
        assert!(in_the_set(&text_2), "{text_2}");
    }
}

// ---------------------------------------------------------------------------------------
// Propiedad 1: construir es exacto.
// ---------------------------------------------------------------------------------------

/// Los fondos válidos de un modo con el gris y el alfa dados (para el «Color…» de los que lo
/// admiten).
fn backgrounds_with_color(mode: Mode, color: [u8; 4]) -> Vec<Background> {
    let [r, g, b, a] = color;
    match mode {
        Mode::Rgba => vec![
            TRANSPARENT,
            WHITE,
            TEST_BLACK,
            Background::Color(Rgba::new(r, g, b, a)),
        ],
        Mode::GrayAlpha => vec![
            TRANSPARENT,
            WHITE,
            TEST_BLACK,
            Background::Color(Rgba::new(g, g, g, a)),
        ],
        Mode::Gray => vec![WHITE, TEST_BLACK, Background::Color(Rgba::opaque(g, g, g))],
        Mode::Indexed => vec![TRANSPARENT, WHITE, TEST_BLACK],
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Para cualquier lado de 1 a 24, modo y fondo válido: `Ok`, la imagen mide lo pedido y cada
    /// píxel es `background_pixel`; con la paleta de D2 en indexado; y
    /// `memory_estimate` crece con el área.
    #[test]
    fn property_1_building_is_exact(
        width in 1u32..=24,
        height in 1u32..=24,
        width2 in 1u32..=24,
        height2 in 1u32..=24,
        mode_2 in 0usize..4,
        background_2 in 0usize..8,
        color in any::<[u8; 4]>(),
    ) {
        let mode = TEST_MODES[mode_2];
        let candidates = backgrounds_with_color(mode, color);
        let background_2 = candidates[background_2 % candidates.len()];
        let spec = spec_new(width, height, mode, background_2);
        let image = builds(&spec, &mut SystemAlloc)
            .map_err(TestCaseError::fail)?;
        let pixel = background_pixel(mode, background_2);
        prop_assert!(pixel.is_some(), "{:?} {:?}: fondo no válido en la prueba", mode, background_2);
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
fn backgrounds_of_source() -> Vec<Background> {
    let mut backgrounds = vec![TRANSPARENT, WHITE, TEST_BLACK];
    for color in [
        GRAY_OPAQUE,
        GRAY_MIDDLE,
        TEST_RED,
        RED_MIDDLE,
        Rgba::new(0x40, 0x40, 0x40, 0),
        Rgba::new(0, 0, 0, 0),
        Rgba::opaque(0, 0, 0),
        Rgba::opaque(255, 255, 255),
        Rgba::new(255, 255, 255, 254),
        Rgba::opaque(1, 2, 3),
    ] {
        backgrounds.push(Background::Color(color));
    }
    backgrounds
}

#[test]
fn property_2_translating_never_loses_color_silently() {
    let mut checked = 0;
    for from in TEST_MODES {
        for background_2 in backgrounds_of_source() {
            // Solo los fondos que existen en el modo de origen.
            let Some(visible) = color_visible(from, background_2) else {
                continue;
            };
            for to in TEST_MODES {
                checked += 1;
                match carry_background(from, background_2, to) {
                    Carry::Kept(new_2) => {
                        assert_eq!(
                            color_visible(to, new_2),
                            Some(visible),
                            "{from:?} {background_2:?} → {to:?}: conservó otro color"
                        );
                        assert_eq!(new_2, background_2, "{from:?} {background_2:?} → {to:?}");
                    }
                    Carry::Reset { to: new_2, notice } => {
                        assert_eq!(
                            new_2,
                            default_background(to),
                            "{from:?} {background_2:?} → {to:?}: se reinicia al de por defecto"
                        );
                        assert!(
                            !notice.is_empty(),
                            "{from:?} {background_2:?} → {to:?}: sin aviso"
                        );
                        assert!(
                            color_visible(to, new_2).is_some(),
                            "{from:?} {background_2:?} → {to:?}: el reinicio no vale en el modo nuevo"
                        );
                    }
                }
            }
        }
    }
    assert!(checked > 100, "se recorrieron {checked} combinaciones");
}

#[test]
fn property_2_carry_background_is_total_also_with_sources_that_the_mode_does_not_admit() {
    // No debe entrar en pánico con ninguna combinación, aunque el fondo no exista en el modo de
    // origen (por ejemplo Transparente en Gris o un Color en Indexado).
    for from in TEST_MODES {
        for background_2 in backgrounds_of_source() {
            for to in TEST_MODES {
                match carry_background(from, background_2, to) {
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
enum TestGesture {
    Width(usize),
    Height(usize),
    StepWidth(bool),
    StepHeight(bool),
    TestMode(usize),
    TestBackground(usize),
    Hex(usize),
    Confirms,
}

const TEXTS_NUMERIC: [&str; 11] = [
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

const TEXTS_HEX: [&str; 10] = [
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

fn gesture() -> impl Strategy<Value = TestGesture> {
    prop_oneof![
        (0..TEXTS_NUMERIC.len()).prop_map(TestGesture::Width),
        (0..TEXTS_NUMERIC.len()).prop_map(TestGesture::Height),
        any::<bool>().prop_map(TestGesture::StepWidth),
        any::<bool>().prop_map(TestGesture::StepHeight),
        (0usize..4).prop_map(TestGesture::TestMode),
        (0usize..4).prop_map(TestGesture::TestBackground),
        (0..TEXTS_HEX.len()).prop_map(TestGesture::Hex),
        Just(TestGesture::Confirms),
    ]
}

/// El diálogo sin su informe, para saber si un evento cambió algo del formulario.
fn without_report(d: &Dialog) -> Dialog {
    let mut copy = d.clone();
    assert!(copy.set_report("informe", String::new()));
    copy
}

fn options_of_the_background(mode: Mode) -> usize {
    match mode {
        Mode::Indexed | Mode::Gray => 3,
        Mode::Rgba | Mode::GrayAlpha => 4,
    }
}

/// Aplica un gesto al diálogo y devuelve el modo en que queda (el modelo de la prueba).
fn applies(d: &mut Dialog, mode_2: usize, gesture: &TestGesture) -> usize {
    let mode = TEST_MODES[mode_2];
    match gesture {
        TestGesture::Width(i) => {
            d.event(Event::Number {
                field: "ancho",
                text: TEXTS_NUMERIC[*i].to_string(),
            });
        }
        TestGesture::Height(i) => {
            d.event(Event::Number {
                field: "alto",
                text: TEXTS_NUMERIC[*i].to_string(),
            });
        }
        TestGesture::StepWidth(more) => {
            d.event(Event::Step {
                field: "ancho",
                delta: if *more { 1 } else { -1 },
            });
        }
        TestGesture::StepHeight(more) => {
            d.event(Event::Step {
                field: "alto",
                delta: if *more { 1 } else { -1 },
            });
        }
        TestGesture::TestMode(i) => {
            d.event(Event::Choice {
                field: "modo",
                index: *i,
            });
            return *i;
        }
        TestGesture::TestBackground(i) => {
            d.event(Event::Choice {
                field: id_background(mode),
                index: i % options_of_the_background(mode),
            });
        }
        TestGesture::Hex(i) => {
            if let Some(id) = id_color(mode).filter(|&id| d.is_shown(id)) {
                d.event(Event::Hex {
                    field: id,
                    text: TEXTS_HEX[*i].to_string(),
                });
            }
        }
        TestGesture::Confirms => {
            if let Some(id) = id_color(mode).filter(|&id| d.is_shown(id)) {
                d.event(Event::HexConfirm {
                    field: id,
                    blur: false,
                });
            }
        }
    }
    mode_2
}

fn has_a_background_notice(d: &Dialog) -> bool {
    let text_2 = text_of_the_report(d);
    NOTICES.iter().any(|a| text_2.contains(*a))
}

/// Las invariantes de la propiedad 3 tras un gesto y su `sync_form`.
fn checks_form(d: &Dialog, mode_2: usize, ctx: &str) -> Result<(), TestCaseError> {
    let mode = TEST_MODES[mode_2];
    // Solo es visible el grupo de fondo del modo actual; un hexadecimal, solo con su modo.
    for other_2 in TEST_MODES {
        prop_assert_eq!(
            d.is_shown(id_background(other_2)),
            other_2 == mode,
            "{}: grupo de fondo de {:?}",
            ctx,
            other_2
        );
        if let Some(id) = id_color(other_2) {
            prop_assert!(
                !d.is_shown(id) || other_2 == mode,
                "{}: el color de {:?} se ve con el modo {:?}",
                ctx,
                other_2,
                mode
            );
        }
    }
    // Lo oculto no sale en los valores; si hay valores, hay especificación y el tamaño vale.
    if let Some(values) = d.values() {
        for other_2 in TEST_MODES.into_iter().filter(|m| *m != mode) {
            prop_assert!(
                values.get(id_background(other_2)).is_none(),
                "{}: {:?} sale en los valores",
                ctx,
                other_2
            );
            if let Some(id) = id_color(other_2) {
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
    let text_2 = text_of_the_report(d);
    let lines: Vec<&str> = text_2.lines().collect();
    prop_assert!(
        !lines.is_empty() && lines.len() <= 4,
        "{}: {:?}",
        ctx,
        text_2
    );
    for line in &lines {
        prop_assert!(line.chars().count() <= 42, "{}: {:?}", ctx, line);
    }
    prop_assert!(lines.iter().all(|l| in_the_set(l)), "{}: {:?}", ctx, text_2);
    prop_assert!(lines[0].starts_with("Memoria:"), "{}: {:?}", ctx, text_2);
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Para secuencias de 0 a 48 eventos con `sync_form` tras cada uno: sin pánico, solo es visible
    /// el grupo de fondo del modo actual, los fondos ocultos no salen en `Values`, `values()` es
    /// `Some` ⇒ `spec_from_values` da `Some` y `size_error` da `None`, el informe cabe y el aviso de
    /// D1b no sobrevive a un cambio de otro campo.
    #[test]
    fn property_3_the_form_state_is_not_corrupted(
        gestures in prop::collection::vec(gesture(), 0..=48),
    ) {
        let mut d = dialog(8192);
        let mut sync = FormSync::default();
        sync_form(&mut d, &mut sync);
        let mut mode_2 = 0usize; // Indexado al abrir
        checks_form(&d, mode_2, "al abrir")?;

        for (i, g) in gestures.iter().enumerate() {
            let ctx = format!("paso {i}: {g:?}");
            let before = without_report(&d);
            let mode_previous = mode_2;
            mode_2 = applies(&mut d, mode_2, g);
            let change = without_report(&d) != before;
            sync_form(&mut d, &mut sync);

            checks_form(&d, mode_2, &ctx)?;
            // Una segunda sincronización sin eventos no tiene nada que pintar.
            prop_assert!(!sync_form(&mut d, &mut sync), "{}: sync_form no es estable", ctx);
            // El aviso de D1b solo puede salir de un cambio de modo.
            if change && mode_2 == mode_previous {
                prop_assert!(!has_a_background_notice(&d), "{}: el aviso sobrevive", ctx);
            }
        }
    }
}

// ---------------------------------------------------------------------------------------
// Propiedad 4: fallar no cambia nada (exhaustiva).
// ---------------------------------------------------------------------------------------

#[test]
fn property_4_failing_changes_nothing() {
    for mode in TEST_MODES {
        let spec = spec_new(16, 8, mode, default_background(mode));
        for modified in [false, true] {
            // Con cambios sin guardar, el descarte ya confirmado (el recorrido llega a construir).
            let discard = modified;
            for failure in FAILURES {
                let mut session = session_2(modified);
                let mut canvas_2 = Canvas2::new_2(
                    session
                        .document
                        .image(ImageId::from_raw(0))
                        .expect("imagen")
                        .clone(),
                );
                canvas_2.zooms_out();
                let before_session = snapshot_of(&session);
                let before_canvas = canvas_2.frame(vec![]);

                let mut false_value = False::new_2(failure);
                let result = creates(
                    &mut session,
                    &mut canvas_2,
                    &spec,
                    &mut false_value,
                    discard,
                );

                let context = format!("{mode:?}, {failure:?}, con cambios {modified}");
                assert_eq!(
                    result,
                    Err(NotCreate::TestMemory(
                        memory_estimate(16, 8, mode).ram_bytes
                    )),
                    "{context}"
                );
                assert_eq!(snapshot_of(&session), before_session, "{context}: sesión");
                let after_2 = canvas_2.frame(vec![]);
                assert_eq!(after_2, before_canvas, "{context}: lienzo");
                assert!(!after_2.uploaded, "{context}: el lienzo no subió nada");
            }

            // Con éxito se reemplazan exactamente una vez.
            let mut session = session_2(modified);
            let mut canvas_2 = Canvas2::new_2(
                session
                    .document
                    .image(ImageId::from_raw(0))
                    .expect("imagen")
                    .clone(),
            );
            let old_2 = canvas_2.frame(vec![]).texture_2;
            let mut false_value = False::new_2(Failure::TestNone);
            let context = format!("{mode:?}, éxito, con cambios {modified}");
            assert_eq!(
                creates(
                    &mut session,
                    &mut canvas_2,
                    &spec,
                    &mut false_value,
                    discard
                ),
                Ok(()),
                "{context}"
            );
            assert_eq!(
                false_value.calls.len(),
                2,
                "{context}: se construyó una vez"
            );
            let image = session
                .document
                .image(ImageId::from_raw(0))
                .expect("imagen");
            assert_eq!(
                (image.width(), image.height(), image.mode()),
                (16, 8, mode),
                "{context}"
            );
            assert!(!session.modified(), "{context}");
            assert!(!session.document.can_undo(), "{context}");
            assert!(session.path().is_none(), "{context}");
            let first = canvas_2.frame(vec![]);
            assert!(first.uploaded, "{context}: el lienzo sube la nueva");
            assert_ne!(first.texture_2, old_2, "{context}");
            assert!(!canvas_2.frame(vec![]).uploaded, "{context}: una sola vez");
        }

        // Con `Unsaved` no se construye nada y todo queda igual.
        let mut session = session_2(true);
        let mut canvas_2 = Canvas2::new_2(
            session
                .document
                .image(ImageId::from_raw(0))
                .expect("imagen")
                .clone(),
        );
        canvas_2.zooms_out();
        let before_session = snapshot_of(&session);
        let before_canvas = canvas_2.frame(vec![]);
        let mut false_value = False::new_2(Failure::TestNone);
        assert_eq!(
            creates(&mut session, &mut canvas_2, &spec, &mut false_value, false),
            Err(NotCreate::Unsaved),
            "{mode:?}"
        );
        assert!(
            false_value.calls.is_empty(),
            "{mode:?}: con Unsaved no se construye nada"
        );
        assert_eq!(snapshot_of(&session), before_session, "{mode:?}: sesión");
        assert_eq!(canvas_2.frame(vec![]), before_canvas, "{mode:?}: lienzo");
    }
}

#[test]
fn property_4_session_new_image_with_unsaved_does_not_change_the_session_or_the_canvas() {
    // La misma garantía vista desde `Session::new_image`: aunque la imagen ya esté construida, con
    // cambios sin guardar y sin descarte la sesión no cambia.
    for mode in TEST_MODES {
        let mut session = session_2(true);
        let before = snapshot_of(&session);
        let image = builds_real(&spec_new(16, 8, mode, default_background(mode)));
        assert_eq!(
            session.new_image(image, false),
            Err(FileError::Unsaved),
            "{mode:?}"
        );
        assert_eq!(snapshot_of(&session), before, "{mode:?}");
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
// - «Líneas de aviso» del informe son las que no empiezan por `TestMemory:`, `Paleta:` ni `notice:`.
// - El aviso de 5 s del título no se puede observar aquí: `sync_form` no recibe ningún medio de
//   avisar. Se fija la firma (A7), que es lo que garantiza que el descarte no lance ninguno.
// ---------------------------------------------------------------------------------------

use px_ui::color_edit::{hex_text, parse_hex};
use px_ui::dialog::resolve_hex;

const SOURCES: [Mode; 3] = [Mode::Rgba, Mode::GrayAlpha, Mode::Gray];
const NOT_VALID: &str = "Color no válido: pasa a ";
const GRAY_LIGHT: Rgba = Rgba::opaque(0xaa, 0xaa, 0xaa);
const BLUE_OPAQUE: Rgba = Rgba::opaque(0x33, 0x66, 0x99);

/// `#rrggbbaa` siempre con ocho dígitos (la forma del aviso nuevo).
fn hex8(c: Rgba) -> String {
    format!("#{:02x}{:02x}{:02x}{:02x}", c.r, c.g, c.b, c.a)
}

fn notice_discarded(c: Rgba) -> String {
    format!("{NOT_VALID}{}", hex8(c))
}

/// Los últimos confirmados que admite cada modo de origen (A2).
fn confirmed_of(from: Mode) -> Vec<Rgba> {
    match from {
        Mode::Rgba => vec![GRAY_LIGHT, GRAY_MIDDLE, BLUE_OPAQUE],
        Mode::GrayAlpha => vec![GRAY_LIGHT, GRAY_MIDDLE],
        Mode::Gray => vec![GRAY_LIGHT],
        Mode::Indexed => vec![],
    }
}

/// Textos que no valen en `from` (`#ff0000` solo vale en RGBA).
fn invalid_of(from: Mode) -> Vec<&'static str> {
    let mut texts = vec!["zz", "#12"];
    if from != Mode::Rgba {
        texts.push("#ff0000");
    }
    for text_2 in &texts {
        assert!(
            parse_hex(text_2, from, GRAY_OPAQUE).is_err(),
            "{from:?} {text_2}"
        );
    }
    texts
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

fn lines_of_notice(d: &Dialog) -> Vec<String> {
    lines_of_the_report(d)
        .into_iter()
        .filter(|l| {
            !l.starts_with("Memoria:") && !l.starts_with("Paleta:") && !l.starts_with("aviso:")
        })
        .collect()
}

/// Un diálogo en `from` con «Color…» elegido, `confirmed` (si lo hay) confirmado y `text_2` (si lo
/// hay) escrito sin confirmar; un evento y un `sync_form` por paso, como el usuario.
fn source_2(from: Mode, confirmed: Option<Rgba>, text_2: Option<&str>) -> (Dialog, FormSync) {
    let mut d = dialog(8192);
    let mut sync = FormSync::default();
    sync_form(&mut d, &mut sync);
    step(
        &mut d,
        &mut sync,
        Event::Choice {
            field: "modo",
            index: index_of_mode(from),
        },
    );
    let id = id_color(from).expect("el modo tiene «Color…»");
    step(
        &mut d,
        &mut sync,
        Event::Choice {
            field: id_background(from),
            index: index_of_background(from, Type::Color).expect("«Color…»"),
        },
    );
    if let Some(c) = confirmed {
        step(
            &mut d,
            &mut sync,
            Event::Hex {
                field: id,
                text: hex8(c),
            },
        );
        step(
            &mut d,
            &mut sync,
            Event::HexConfirm {
                field: id,
                blur: false,
            },
        );
        assert_eq!(d.hex_committed(id), Some(c), "{from:?}: confirmado");
    }
    if let Some(text_2) = text_2 {
        step(
            &mut d,
            &mut sync,
            Event::Hex {
                field: id,
                text: text_2.to_string(),
            },
        );
        assert!(d.hex_pending(id));
    }
    assert!(d.is_shown(id), "{from:?}: el hexadecimal se ve");
    assert!(
        lines_of_notice(&d).is_empty(),
        "{from:?}: la preparación no deja avisos; hay {:?}",
        lines_of_notice(&d)
    );
    (d, sync)
}

fn rest(d: &mut Dialog, sync: &mut FormSync) {
    for _ in 0..2 {
        sync_form(d, sync);
    }
}

/// Los dos caminos de la spec hasta el fotograma del cambio de modo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TestPath {
    /// (a) escribir y hacer clic en el modo: el clic quita el foco al pulsar y el campo se revierte.
    Click,
    /// (b) `Intro` con el texto inválido (se queda) y luego el clic.
    Intro,
}

const PATHS: [TestPath; 2] = [TestPath::Click, TestPath::Intro];

fn until_the_change(d: &mut Dialog, sync: &mut FormSync, from: Mode, path: TestPath) {
    let id = id_color(from).expect("color");
    rest(d, sync);
    match path {
        TestPath::Click => {
            step(
                d,
                sync,
                Event::HexConfirm {
                    field: id,
                    blur: true,
                },
            );
            assert!(!d.hex_pending(id), "el clic revierte el campo");
        }
        TestPath::Intro => {
            step(
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
    rest(d, sync);
}

fn changes_of_mode(d: &mut Dialog, sync: &mut FormSync, to: Mode) {
    step(
        d,
        sync,
        Event::Choice {
            field: "modo",
            index: index_of_mode(to),
        },
    );
}

/// El fondo y el aviso al trasladar un fondo válido (el de D1b o ninguno).
fn after_translate(from: Mode, background_2: Background, to: Mode) -> (Background, Option<String>) {
    match translates(from, background_2, to) {
        Seen::Kept(b) => (b, None),
        Seen::Reset(b, notice) => (b, Some(notice)),
    }
}

/// El fondo y el aviso al descartar un texto inválido con `last_2_2` confirmado (A1-1 y A1-4): si
/// viaja, el aviso nuevo; si no, solo el de D1b de ese color.
fn after_discard(from: Mode, last_2_2: Rgba, to: Mode) -> (Background, String) {
    match translates(from, Background::Color(last_2_2), to) {
        Seen::Kept(b) => (b, notice_discarded(last_2_2)),
        Seen::Reset(b, notice) => (b, notice),
    }
}

/// El estado del diálogo tras un traslado: informe, fondo, hexadecimal nuevo y ninguno viejo.
fn checks_translation(
    d: &Dialog,
    from: Mode,
    to: Mode,
    background_2: Background,
    notice: Option<&str>,
    ctx: &str,
) {
    assert_eq!(
        text_of_the_report(d),
        report_text(Some((64, 64)), to, notice),
        "{ctx}: informe"
    );
    let lines = lines_of_notice(d);
    assert_eq!(
        lines,
        notice.map(str::to_string).into_iter().collect::<Vec<_>>(),
        "{ctx}: líneas de aviso"
    );
    for line in lines_of_the_report(d) {
        assert!(line.chars().count() <= 42, "{ctx}: {line:?}");
        assert!(in_the_set(&line), "{ctx}: {line:?}");
    }
    assert!(
        d.is_valid(),
        "{ctx}: el formulario sale válido; {:?}",
        d.reason()
    );
    let spec = spec_de(d).unwrap_or_else(|| panic!("{ctx}: sin valores válidos"));
    assert_eq!(fields(&spec), (64, 64, to, background_2), "{ctx}");
    // El hexadecimal del modo viejo no se ve.
    if let Some(old) = id_color(from) {
        assert!(!d.is_shown(old), "{ctx}: el hexadecimal viejo se ve");
        assert_eq!(
            d.validity(old),
            None,
            "{ctx}: el viejo no cuenta en la validez"
        );
    }
    // El nuevo (si lo hay y es el fondo) muestra el color que viaja, válido y confirmado.
    let values = d.values().expect("válido");
    for other_2 in TEST_MODES {
        if let Some(id) = id_color(other_2).filter(|_| other_2 != to) {
            assert_eq!(values.get(id), None, "{ctx}: {id} en Values");
        }
    }
    if let (Background::Color(color), Some(id)) = (background_2, id_color(to)) {
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
fn create_color(d: &Dialog, from: Mode) -> Rgba {
    match d.clone().event(Event::Accept) {
        Reaction::Accepted(v) => v
            .color(id_color(from).expect("color"))
            .expect("el hexadecimal visible sale en los valores"),
        other_2 => panic!("se esperaba Accepted y salió {other_2:?}"),
    }
}

// --- A1: sin válido previo ---

#[test]
fn row_a1_invalid_text_without_a_previous_valid_one_travels_as_808080ff_and_warns() {
    for from in SOURCES {
        for to in SOURCES.into_iter().filter(|m| *m != from) {
            for text_2 in invalid_of(from) {
                for path in PATHS {
                    let ctx = format!("{from:?} → {to:?} {text_2:?} {path:?}");
                    let (mut d, mut sync) = source_2(from, None, Some(text_2));
                    until_the_change(&mut d, &mut sync, from, path);
                    changes_of_mode(&mut d, &mut sync, to);

                    let notice = notice_discarded(GRAY_OPAQUE);
                    assert_eq!(notice, "Color no válido: pasa a #808080ff");
                    assert_eq!(notice.chars().count(), 33);
                    checks_translation(
                        &d,
                        from,
                        to,
                        Background::Color(GRAY_OPAQUE),
                        Some(&notice),
                        &ctx,
                    );
                    let id = id_color(to).expect("color");
                    assert_eq!(d.text(id), Some("#808080".to_string()), "{ctx}");
                    // Y el aviso dura hasta que cambia otro valor.
                    assert!(!sync_form(&mut d, &mut sync), "{ctx}: estable");
                    assert!(lines_of_notice(&d).contains(&notice), "{ctx}");
                }
            }
        }
    }
}

#[test]
fn row_a1_invalid_text_without_a_previous_valid_one_to_indexed_says_indexed_without_color() {
    for from in SOURCES {
        for text_2 in invalid_of(from) {
            for path in PATHS {
                let ctx = format!("{from:?} → Indexado {text_2:?} {path:?}");
                let (mut d, mut sync) = source_2(from, None, Some(text_2));
                until_the_change(&mut d, &mut sync, from, path);
                changes_of_mode(&mut d, &mut sync, Mode::Indexed);
                checks_translation(
                    &d,
                    from,
                    Mode::Indexed,
                    TRANSPARENT,
                    Some(NOTICE_INDEXED_WITHOUT_COLOR),
                    &ctx,
                );
                assert!(!text_of_the_report(&d).contains(NOT_VALID), "{ctx}");
            }
        }
    }
}

// --- A2: con válido previo ---

#[test]
fn row_a1_with_a_previous_valid_one_the_last_confirmed_travels_or_only_the_d1b_notice_comes_out() {
    for from in SOURCES {
        for last_2_2 in confirmed_of(from) {
            for to in TEST_MODES.into_iter().filter(|m| *m != from) {
                for text_2 in invalid_of(from) {
                    for path in PATHS {
                        let ctx = format!("{from:?} {last_2_2:?} → {to:?} {text_2:?} {path:?}");
                        let (mut d, mut sync) = source_2(from, Some(last_2_2), Some(text_2));
                        until_the_change(&mut d, &mut sync, from, path);
                        changes_of_mode(&mut d, &mut sync, to);

                        let (background_2, notice) = after_discard(from, last_2_2, to);
                        checks_translation(&d, from, to, background_2, Some(&notice), &ctx);
                        if last_2_2 != GRAY_OPAQUE {
                            assert!(
                                !text_of_the_report(&d).contains("#808080ff"),
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
fn row_a1_with_a_previous_valid_one_the_literal_examples_of_the_spec() {
    // (modo de origen, último confirmado, destino, aviso esperado)
    let cases = [
        (
            Mode::Rgba,
            GRAY_LIGHT,
            Mode::Gray,
            "Color no válido: pasa a #aaaaaaff",
        ),
        (
            Mode::Rgba,
            GRAY_LIGHT,
            Mode::GrayAlpha,
            "Color no válido: pasa a #aaaaaaff",
        ),
        (
            Mode::Rgba,
            GRAY_MIDDLE,
            Mode::GrayAlpha,
            "Color no válido: pasa a #80808080",
        ),
        (
            Mode::Rgba,
            GRAY_MIDDLE,
            Mode::Gray,
            "Color con alfa: pasa a Blanco",
        ),
        (
            Mode::Rgba,
            BLUE_OPAQUE,
            Mode::GrayAlpha,
            "Color no gris: pasa a Transparente",
        ),
        (
            Mode::Rgba,
            BLUE_OPAQUE,
            Mode::Gray,
            "Color no gris: pasa a Blanco",
        ),
        (
            Mode::Rgba,
            BLUE_OPAQUE,
            Mode::Indexed,
            "Indexado sin Color: pasa a Transparente",
        ),
        (
            Mode::GrayAlpha,
            GRAY_MIDDLE,
            Mode::Rgba,
            "Color no válido: pasa a #80808080",
        ),
        (
            Mode::GrayAlpha,
            GRAY_MIDDLE,
            Mode::Gray,
            "Color con alfa: pasa a Blanco",
        ),
        (
            Mode::Gray,
            GRAY_LIGHT,
            Mode::Rgba,
            "Color no válido: pasa a #aaaaaaff",
        ),
        (
            Mode::Gray,
            GRAY_LIGHT,
            Mode::Indexed,
            "Indexado sin Color: pasa a Transparente",
        ),
    ];
    for (from, last_2_2, to, notice) in cases {
        let (mut d, mut sync) = source_2(from, Some(last_2_2), Some("zz"));
        until_the_change(&mut d, &mut sync, from, TestPath::Click);
        changes_of_mode(&mut d, &mut sync, to);
        assert_eq!(
            lines_of_notice(&d),
            vec![notice.to_string()],
            "{from:?} {last_2_2:?} → {to:?}"
        );
    }
}

// --- A5: texto válido sin confirmar, y el alfa ---

#[test]
fn row_a1_unconfirmed_valid_text_travels_with_the_alpha_that_create_would_give_and_without_a_new_notice()
 {
    let with_alpha = Rgba::new(0xaa, 0xaa, 0xaa, 0xcc);
    // (modo de origen, último confirmado, texto, color que daría Crear)
    let mut cases = Vec::new();
    for from in [Mode::Rgba, Mode::GrayAlpha] {
        for (text_2, expected_2) in [
            ("bbbbbb", Rgba::new(0xbb, 0xbb, 0xbb, 0xcc)),
            ("bbb", Rgba::new(0xbb, 0xbb, 0xbb, 0xcc)),
            ("#bbbbbbdd", Rgba::new(0xbb, 0xbb, 0xbb, 0xdd)),
            ("#bbb8", Rgba::new(0xbb, 0xbb, 0xbb, 0x88)),
        ] {
            cases.push((from, Some(with_alpha), text_2, expected_2));
        }
        // Sin confirmar nada antes: el alfa de partida es 255.
        cases.push((from, None, "bbbbbb", Rgba::opaque(0xbb, 0xbb, 0xbb)));
    }
    for (text_2, expected_2) in [
        ("bbbbbb", Rgba::opaque(0xbb, 0xbb, 0xbb)),
        ("bbb", Rgba::opaque(0xbb, 0xbb, 0xbb)),
        ("#bbbbbbff", Rgba::opaque(0xbb, 0xbb, 0xbb)),
    ] {
        cases.push((Mode::Gray, Some(GRAY_LIGHT), text_2, expected_2));
    }
    for (from, confirmed, text_2, expected_2) in cases {
        for to in TEST_MODES.into_iter().filter(|m| *m != from) {
            let ctx = format!("{from:?} {confirmed:?} {text_2:?} → {to:?}");
            let (mut d, mut sync) = source_2(from, confirmed, Some(text_2));
            // Es lo que daría `Crear` en el modo de origen.
            assert_eq!(create_color(&d, from), expected_2, "{ctx}: Crear");
            rest(&mut d, &mut sync);
            changes_of_mode(&mut d, &mut sync, to);

            let (background_2, notice) = after_translate(from, Background::Color(expected_2), to);
            checks_translation(&d, from, to, background_2, notice.as_deref(), &ctx);
            assert!(!text_of_the_report(&d).contains(NOT_VALID), "{ctx}");
        }
    }
}

#[test]
fn row_a1_the_alpha_of_the_last_confirmed_travels_in_the_new_field_with_six_digits() {
    // `#aaaaaacc` confirmado y `bbbbbb` escrito: el campo de Gris con alfa dice `#bbbbbbcc`.
    let (mut d, mut sync) = source_2(
        Mode::Rgba,
        Some(Rgba::new(0xaa, 0xaa, 0xaa, 0xcc)),
        Some("bbbbbb"),
    );
    changes_of_mode(&mut d, &mut sync, Mode::GrayAlpha);
    assert_eq!(d.text("color_gris_alfa"), Some("#bbbbbbcc".to_string()));
    assert!(lines_of_notice(&d).is_empty());
    // A Gris, el alfa < 255 reinicia con el aviso de siempre.
    let (mut d, mut sync) = source_2(
        Mode::Rgba,
        Some(Rgba::new(0xaa, 0xaa, 0xaa, 0xcc)),
        Some("bbbbbb"),
    );
    changes_of_mode(&mut d, &mut sync, Mode::Gray);
    assert_eq!(
        lines_of_notice(&d),
        vec![NOTICE_COLOR_WITH_ALPHA.to_string()]
    );
    // Y de vuelta a RGBA con `Crear`: el mismo valor que viajó.
    let (mut d, mut sync) = source_2(
        Mode::Rgba,
        Some(Rgba::new(0xaa, 0xaa, 0xaa, 0xcc)),
        Some("bbbbbb"),
    );
    changes_of_mode(&mut d, &mut sync, Mode::GrayAlpha);
    changes_of_mode(&mut d, &mut sync, Mode::Rgba);
    assert_eq!(
        create_color(&d, Mode::Rgba),
        Rgba::new(0xbb, 0xbb, 0xbb, 0xcc)
    );
}

// --- A6: anchos ---

#[test]
fn row_a1_the_notices_fit_in_42_characters_and_are_in_the_set() {
    for c in [
        Rgba::opaque(255, 255, 255),
        Rgba::new(0, 0, 0, 0),
        GRAY_MIDDLE,
        GRAY_OPAQUE,
        BLUE_OPAQUE,
        Rgba::new(0xff, 0xff, 0xff, 0xfe),
    ] {
        let notice = notice_discarded(c);
        assert_eq!(notice.chars().count(), 33, "{notice}");
        assert!(in_the_set(&notice), "{notice}");
    }
    for notice in NOTICES {
        assert!(notice.chars().count() <= 42, "{notice}");
        assert!(in_the_set(notice), "{notice}");
    }
}

#[test]
fn row_a1_the_report_with_a_size_that_warns_and_indexed_fits_in_4_lines_of_42() {
    let reviews = |d: &Dialog, to: Mode, ctx: &str| {
        let lines = lines_of_the_report(d);
        assert!(lines.len() <= 4, "{ctx}: {lines:?}");
        assert!(lines[0].starts_with("Memoria:"), "{ctx}: {lines:?}");
        assert!(
            lines.iter().any(|l| l.starts_with("aviso: más de")),
            "{ctx}: el tamaño avisa; {lines:?}"
        );
        assert_eq!(
            lines.iter().any(|l| l.starts_with("Paleta:")),
            to == Mode::Indexed,
            "{ctx}"
        );
        assert_eq!(lines_of_notice(d).len(), 1, "{ctx}: {lines:?}");
        for line in &lines {
            assert!(line.chars().count() <= 42, "{ctx}: {line:?}");
            assert!(in_the_set(line), "{ctx}: {line:?}");
        }
        // El aviso de fondo va el último, como en la fila 6.
        assert_eq!(lines.last(), lines_of_notice(d).last(), "{ctx}");
    };
    for from in SOURCES {
        for last_2_2 in confirmed_of(from) {
            for to in TEST_MODES.into_iter().filter(|m| *m != from) {
                let ctx = format!("{from:?} {last_2_2:?} → {to:?} con 5000 × 5000");
                let (mut d, mut sync) = source_2(from, Some(last_2_2), Some("zz"));
                for field_2 in ["ancho", "alto"] {
                    step(
                        &mut d,
                        &mut sync,
                        Event::Number {
                            field: field_2,
                            text: "5000".to_string(),
                        },
                    );
                }
                until_the_change(&mut d, &mut sync, from, TestPath::Intro);
                changes_of_mode(&mut d, &mut sync, to);
                reviews(&d, to, &ctx);
            }
        }
    }
    // Transparente a Gris (el quinto texto de D1b) con el tamaño que avisa.
    let (mut d, mut sync) = dialog_synchronized(Mode::Rgba, TRANSPARENT);
    for field_2 in ["ancho", "alto"] {
        step(
            &mut d,
            &mut sync,
            Event::Number {
                field: field_2,
                text: "5000".to_string(),
            },
        );
    }
    changes_of_mode(&mut d, &mut sync, Mode::Gray);
    reviews(&d, Mode::Gray, "Transparente → Gris con 5000 × 5000");
}

// --- A7: vida, hueco único y silencio ---

#[test]
fn row_a1_the_new_notice_goes_away_when_another_value_changes_like_those_of_d1b() {
    type Change = (&'static str, fn(&mut Dialog, &mut FormSync));
    let changes_2: [Change; 5] = [
        ("ancho +", |d, s| {
            step(
                d,
                s,
                Event::Step {
                    field: "ancho",
                    delta: 1,
                },
            );
        }),
        ("alto 10", |d, s| {
            step(
                d,
                s,
                Event::Number {
                    field: "alto",
                    text: "10".to_string(),
                },
            );
        }),
        ("fondo Blanco", |d, s| {
            step(
                d,
                s,
                Event::Choice {
                    field: "fondo_gris_alfa",
                    index: 1,
                },
            );
        }),
        ("hexadecimal nuevo", |d, s| {
            step(
                d,
                s,
                Event::Hex {
                    field: "color_gris_alfa",
                    text: "#444444".to_string(),
                },
            );
        }),
        ("modo", |d, s| {
            step(
                d,
                s,
                Event::Choice {
                    field: "modo",
                    index: 1,
                },
            );
        }),
    ];
    for (name_2, changes) in changes_2 {
        let (mut d, mut sync) = source_2(Mode::Rgba, None, Some("zz"));
        until_the_change(&mut d, &mut sync, Mode::Rgba, TestPath::Click);
        changes_of_mode(&mut d, &mut sync, Mode::GrayAlpha);
        assert_eq!(
            lines_of_notice(&d),
            vec![notice_discarded(GRAY_OPAQUE)],
            "{name_2}"
        );
        // Dura mientras no cambia nada.
        rest(&mut d, &mut sync);
        assert_eq!(
            lines_of_notice(&d),
            vec![notice_discarded(GRAY_OPAQUE)],
            "{name_2}"
        );
        changes(&mut d, &mut sync);
        // Ni el nuevo ni el de D1b (el traslado de modo de un fondo válido no avisa).
        assert!(
            lines_of_notice(&d).is_empty(),
            "{name_2}: {:?}",
            lines_of_notice(&d)
        );
        assert!(!text_of_the_report(&d).contains(NOT_VALID), "{name_2}");
    }
}

#[test]
fn row_a1_if_the_last_confirmed_does_not_travel_only_the_d1b_notice_comes_out_on_one_line() {
    for (to, notice) in [
        (Mode::GrayAlpha, NOTICE_NON_GRAY_TO_GRAY_ALPHA),
        (Mode::Gray, NOTICE_NON_GRAY_TO_GRAY),
        (Mode::Indexed, NOTICE_INDEXED_WITHOUT_COLOR),
    ] {
        for path in PATHS {
            let (mut d, mut sync) = source_2(Mode::Rgba, Some(BLUE_OPAQUE), Some("zz"));
            until_the_change(&mut d, &mut sync, Mode::Rgba, path);
            changes_of_mode(&mut d, &mut sync, to);
            assert_eq!(
                lines_of_notice(&d),
                vec![notice.to_string()],
                "{to:?} {path:?}: un solo aviso, el de D1b"
            );
            assert!(
                !text_of_the_report(&d).contains(NOT_VALID),
                "{to:?} {path:?}"
            );
            assert!(lines_of_the_report(&d).len() <= 4);
        }
    }
}

#[test]
fn row_a1_a_hidden_invalid_text_does_not_warn() {
    for from in SOURCES {
        let options_2: &[Type] = match from {
            Mode::Gray => &[Type::White, Type::Black],
            _ => &[Type::Transparent, Type::White, Type::Black],
        };
        for &kind in options_2 {
            for to in TEST_MODES.into_iter().filter(|m| *m != from) {
                let ctx = format!("{from:?} {kind:?} → {to:?}");
                let (mut d, mut sync) = source_2(from, None, Some("zz"));
                let id = id_color(from).expect("color");
                // La opción deja de ser «Color…»: el hexadecimal se oculta con basura dentro.
                step(
                    &mut d,
                    &mut sync,
                    Event::Choice {
                        field: id_background(from),
                        index: index_of_background(from, kind).expect("opción"),
                    },
                );
                assert!(!d.is_shown(id), "{ctx}");
                assert!(d.hex_pending(id), "{ctx}: la basura sigue escrita");
                rest(&mut d, &mut sync);
                changes_of_mode(&mut d, &mut sync, to);

                let background_2 = match kind {
                    Type::Transparent => TRANSPARENT,
                    Type::White => WHITE,
                    _ => TEST_BLACK,
                };
                let (expected_2, notice) = after_translate(from, background_2, to);
                assert_eq!(
                    lines_of_notice(&d),
                    notice.clone().into_iter().collect::<Vec<_>>(),
                    "{ctx}"
                );
                assert!(!text_of_the_report(&d).contains(NOT_VALID), "{ctx}");
                assert_eq!(
                    fields(&spec_de(&d).expect("válido")),
                    (64, 64, to, expected_2),
                    "{ctx}"
                );
            }
        }
    }
}

#[test]
fn row_a1_the_transfers_of_valid_values_of_d1b_stay_the_same_and_without_the_new_notice() {
    for (name_2, background_2, sources, boxes_2) in table_d_1_b() {
        for from in sources {
            for (to, box_value) in TARGETS.into_iter().zip(boxes_2) {
                if from == to {
                    continue;
                }
                let (mut d, mut sync) = dialog_synchronized(from, background_2);
                changes_of_mode(&mut d, &mut sync, to);
                let (background_expected, notice) = match box_value {
                    Equal => (background_2, None),
                    Resets(a, notice) => (a, Some(notice)),
                };
                let ctx = format!("{name_2}: {from:?} → {to:?}");
                assert!(!text_of_the_report(&d).contains(NOT_VALID), "{ctx}");
                assert_eq!(
                    text_of_the_report(&d),
                    report_text(Some((64, 64)), to, notice),
                    "{ctx}"
                );
                assert_eq!(
                    fields(&spec_de(&d).expect("válido")),
                    (64, 64, to, background_expected),
                    "{ctx}"
                );
            }
        }
    }
}

#[test]
fn row_a1_sync_form_cannot_launch_the_5_s_notice_because_it_does_not_receive_how() {
    // El aviso del título (`App::notify`) queda bajo el velo del modal (A1-4): el descarte solo
    // puede decirse en el informe. `sync_form` solo recibe el diálogo y su estado y devuelve si hay
    // algo que pintar; si ganase un parámetro para avisar, esta prueba dejaría de compilar.
    let _: fn(&mut Dialog, &mut FormSync) -> bool = sync_form;
    let (mut d, mut sync) = source_2(Mode::Rgba, None, Some("zz"));
    until_the_change(&mut d, &mut sync, Mode::Rgba, TestPath::Click);
    let change = {
        step(
            &mut d,
            &mut sync,
            Event::Choice {
                field: "modo",
                index: index_of_mode(Mode::Gray),
            },
        );
        sync_form(&mut d, &mut sync)
    };
    assert!(!change, "tras el traslado no queda nada por pintar");
    assert_eq!(lines_of_notice(&d), vec![notice_discarded(GRAY_OPAQUE)]);
}

// --- Propiedades ---

const TEXTS_A_1: [&str; 17] = [
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
    fn property_a1_1_it_is_never_replaced_silently(
        from_2 in 0usize..3,
        jump in 1usize..4,
        with_confirmed in any::<bool>(),
        color in any::<[u8; 4]>(),
        text_2 in 0..=TEXTS_A_1.len(),
        click in any::<bool>(),
    ) {
        let from = SOURCES[from_2];
        let to = TEST_MODES[(index_of_mode(from) + jump) % 4];
        let confirmed = with_confirmed.then(|| confirmable(from, color));
        let last_2_2 = confirmed.unwrap_or(GRAY_OPAQUE);
        let text_2 = TEXTS_A_1.get(text_2).copied();
        let ctx = format!("{from:?} {confirmed:?} {text_2:?} → {to:?} clic {click}");
        let (mut d, mut sync) = source_2(from, confirmed, text_2);
        let reading = match text_2 {
            Some(text_2) => parse_hex(text_2, from, last_2_2).ok(),
            None => Some(last_2_2),
        };
        match reading {
            None => {
                let path = if click { TestPath::Click } else { TestPath::Intro };
                until_the_change(&mut d, &mut sync, from, path);
                changes_of_mode(&mut d, &mut sync, to);
                let (background_2, notice) = after_discard(from, last_2_2, to);
                checks_translation(&d, from, to, background_2, Some(&notice), &ctx);
            }
            Some(color) => {
                rest(&mut d, &mut sync);
                changes_of_mode(&mut d, &mut sync, to);
                let (background_2, notice) = after_translate(from, Background::Color(color), to);
                checks_translation(&d, from, to, background_2, notice.as_deref(), &ctx);
                prop_assert!(!text_of_the_report(&d).contains(NOT_VALID), "{}", ctx);
            }
        }
    }
}

/// Un gesto de la propiedad 3 de la 19 o uno de los nuevos: `Intro`, la pérdida del foco y el
/// cambio de modo con clic (que antes quita el foco al campo visible).
#[derive(Clone, Debug)]
enum GestureA1 {
    Base(TestGesture),
    Intro,
    LosesFocus,
    ClickMode(usize),
}

fn gesture_a_1() -> impl Strategy<Value = GestureA1> {
    prop_oneof![
        4 => gesture().prop_map(GestureA1::Base),
        1 => Just(GestureA1::Intro),
        1 => Just(GestureA1::LosesFocus),
        2 => (0usize..4).prop_map(GestureA1::ClickMode),
    ]
}

/// Aplica un gesto y devuelve el modo en que queda; el clic en el modo hace su `sync_form` del
/// fotograma de la pulsación entre la pérdida del foco y el cambio.
fn applies_a_1(d: &mut Dialog, sync: &mut FormSync, mode_2: usize, gesture: &GestureA1) -> usize {
    let visible = id_color(TEST_MODES[mode_2]).filter(|&id| d.is_shown(id));
    match gesture {
        GestureA1::Base(g) => applies(d, mode_2, g),
        GestureA1::Intro => {
            if let Some(id) = visible {
                d.event(Event::HexConfirm {
                    field: id,
                    blur: false,
                });
            }
            mode_2
        }
        GestureA1::LosesFocus => {
            if let Some(id) = visible {
                d.event(Event::HexConfirm {
                    field: id,
                    blur: true,
                });
            }
            mode_2
        }
        GestureA1::ClickMode(i) => {
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
    fn property_a1_2_the_form_stays_healthy(
        gestures in prop::collection::vec(gesture_a_1(), 0..=48),
    ) {
        let mut d = dialog(8192);
        let mut sync = FormSync::default();
        sync_form(&mut d, &mut sync);
        let mut mode_2 = 0usize;
        for (i, g) in gestures.iter().enumerate() {
            let ctx = format!("paso {i}: {g:?}");
            let before = without_report(&d);
            let mode_previous = mode_2;
            mode_2 = applies_a_1(&mut d, &mut sync, mode_2, g);
            let change = without_report(&d) != before;
            sync_form(&mut d, &mut sync);

            checks_form(&d, mode_2, &ctx)?;
            prop_assert!(!sync_form(&mut d, &mut sync), "{}: sync_form no es estable", ctx);
            let notices = lines_of_notice(&d);
            prop_assert!(notices.len() <= 1, "{}: {:?}", ctx, notices);
            for notice in &notices {
                prop_assert!(
                    NOTICES.contains(&notice.as_str())
                        || (notice.starts_with(NOT_VALID) && notice.chars().count() == 33),
                    "{}: aviso desconocido {:?}",
                    ctx,
                    notice
                );
            }
            // Ningún texto inválido en los valores: el hexadecimal visible es válido y sale.
            if let Some(values) = d.values()
                && let Some(id) = id_color(TEST_MODES[mode_2]).filter(|&id| d.is_shown(id))
            {
                prop_assert_eq!(d.validity(id), Some(Validity::Valid), "{}", ctx);
                prop_assert!(values.color(id).is_some(), "{}: {} sin color", ctx, id);
            }
            if change && mode_2 == mode_previous {
                prop_assert!(notices.is_empty(), "{}: el aviso sobrevive: {:?}", ctx, notices);
            }
        }
    }
}

const TEXTS_P_3: [&str; 24] = [
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
    fn property_a1_3_transfer_and_create_read_the_same(
        from_2 in 0usize..3,
        jump in 1usize..4,
        with_confirmed in any::<bool>(),
        color in any::<[u8; 4]>(),
        text_2 in 0..TEXTS_P_3.len(),
    ) {
        let from = SOURCES[from_2];
        let to = TEST_MODES[(index_of_mode(from) + jump) % 4];
        let confirmed = with_confirmed.then(|| confirmable(from, color));
        let last_2_2 = confirmed.unwrap_or(GRAY_OPAQUE);
        let text_2 = TEXTS_P_3[text_2];
        let ctx = format!("{from:?} {confirmed:?} {text_2:?} → {to:?}");
        let (mut d, mut sync) = source_2(from, confirmed, Some(text_2));
        let id = id_color(from).expect("color");
        prop_assert_eq!(d.hex_committed(id), Some(last_2_2), "{}", ctx);
        match resolve_hex(Some(text_2), from, last_2_2) {
            Ok(c) => {
                prop_assert_eq!(create_color(&d, from), c, "{}: Crear", ctx);
                prop_assert_eq!(
                    read_background(&d, from),
                    Some(Background::Color(c)),
                    "{}: lectura del traslado",
                    ctx
                );
                rest(&mut d, &mut sync);
                changes_of_mode(&mut d, &mut sync, to);
                let (background_2, notice) = after_translate(from, Background::Color(c), to);
                checks_translation(&d, from, to, background_2, notice.as_deref(), &ctx);
                prop_assert!(!text_of_the_report(&d).contains(NOT_VALID), "{}", ctx);
            }
            Err(_) => {
                let rejected = matches!(d.clone().event(Event::Accept), Reaction::Rejected(_));
                prop_assert!(rejected, "{}: Crear debía rechazarse", ctx);
                prop_assert_eq!(
                    read_background(&d, from),
                    Some(Background::Color(last_2_2)),
                    "{}: lectura del traslado",
                    ctx
                );
                until_the_change(&mut d, &mut sync, from, TestPath::Intro);
                changes_of_mode(&mut d, &mut sync, to);
                let (background_2, notice) = after_discard(from, last_2_2, to);
                checks_translation(&d, from, to, background_2, Some(&notice), &ctx);
            }
        }
    }
}
