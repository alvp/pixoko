//! Tests de `convert_mode` en su parte sin egui (specs/hito-2-convertir-modo.md, unidad 20): filas 1 a
//! 6 y 9, propiedades 1, 2 y 4 y la medición `#[ignore]` de la fila 12. En ROJO hasta la
//! implementación: `convert_mode` solo declara estos módulos de prueba.
//!
//! - Fila 1: `targets`, `convert_form` por origen y `open_convert_mode`.
//! - Fila 2: `analyze` y `classify` (el informe sale de `convert_image(.., NOTHING)`).
//! - Fila 3: `needs` y `block_reason` (cobertura y motivo), con un oráculo escrito aparte.
//! - Fila 4: `allowed_of`, `plan_from_values` y el `check` del formulario.
//! - Fila 5: `report_text` y `history_warning`.
//! - Fila 6: `sync_convert` con un `Analyzer` que cuenta (cuándo se calcula).
//! - Fila 9: `start_convert`, `PendingConvert::tick` y `CONVERTING_NOTICE`.
//! - Propiedades: 1 (el informe y `Accept` dicen la verdad), 2 (el estado del formulario no se
//!   corrompe) y 4 (calcular solo al cambiar de destino).
//! - Fila 12 (medición): `#[ignore]`, `cargo test --release -p px-app -- --ignored --nocapture`.
//!
//! **Suposiciones y ambigüedades** (la spec no fija un detalle; se elige lo más literal y se prueba
//! lo demás por propiedades):
//! - Rótulos de las opciones de `target_2`: la spec solo dice «los modos en el orden Indexado · RGBA ·
//!   Gris con alfa · Gris». Se supone que cada opción se llama como el modo, igual que en la 19
//!   (`Indexado`, `RGBA`, `Gris with alpha_2`, `Gris`). Es lo que el banco de egui pulsa.
//! - `#ffff` es el 4 dígitos `rgba` con `a = f` → `Rgba::from_hex` lo lee como `(255,255,255,255)`:
//!   **opaco** (la spec lo aclara); el de alfa menor es `#fff8`.
//! - Derives no listados: `AlphaChoice`, `Permissions`, `Needs`, `PaletteInfo` e `History` solo se
//!   construyen y se les leen los campos (nunca se comparan ni se imprimen enteros, salvo
//!   `PaletteInfo` dentro de `Analysis`, que es `PartialEq` y `Debug`). `SystemAnalyzer` es una
//!   estructura sin campos (`SystemAnalyzer`), como `SystemAlloc` de la 19.
//! - «Líneas» del informe: `report_text` devuelve texto con saltos de línea; se mide con
//!   `wrap_text(text_2, 42)` (lo que pinta la zona de solo lectura: ≤ 7 filas de ≤ 42 caracteres) y
//!   los textos literales de D3 y D4 se buscan en el texto con los saltos pasados a espacios (un
//!   envoltorio previo de `report_text` o el de la zona no los parte).
//! - Singular y plural: la spec dice que existen y solo da el plural. Se exige que con N = 1 no
//!   salga «1 píxeles» ni «1 colores» y que salga «1 píxel» y «1 color» (español correcto).
//! - Aviso de tamaño: solo depende de `size` (no de las dimensiones de `image`); el de historia,
//!   solo de `image` y `history`. Así se prueban 8192² y 4097 × 4096 sin reservarlas.
//! - Con el informe pendiente (`analysis = None`) el texto empieza por `Calculando el report…`; si
//!   sale además el aviso de tamaño no se dice (la spec no lo fija) y el de historia no sale.
//! - Los textos de `TooLarge` en el informe no están en la spec: solo se exige que haya una primera
//!   línea distinta de las demás y que respete juego y longitud.
//! - Un fondo hexadecimal o activo con alfa < 255 y con alfa en juego: `block_reason` no tiene texto
//!   fijado para el hexadecimal (el `check` del formulario lo rechaza antes): no se prueba.
//! - Un fondo «color activo» no opaco o sin color se rechaza siempre que esté elegido, se pierda el
//!   alfa o no (la spec lo aclara: no se deja `Accept` activo sin `Plan`).
//! - Durante la espera de un cálculo, el valor que devuelve `sync_convert` en los fotogramas
//!   intermedios no se prueba (solo el del fotograma del cambio y el del cálculo).

#![allow(clippy::needless_range_loop)]

use super::*;
use px_core::ImageId;

use std::sync::OnceLock;
use std::time::Instant;

use proptest::prelude::*;
use px_core::{
    Allowed, Command, ConversionReport, ConvertError, Document, Image, Mode, Palette, Pixel, Rgba,
    convert_image,
};
use px_ui::actions::Locks;
use px_ui::color_edit::parse_hex;
use px_ui::dialog::{
    Dialog, DialogKind, Event, FieldKind, FieldSpec, Modal, Reaction, Rows, ShowWhen, Values,
};
use px_ui::dialog_keys::{FocusTarget, initial_focus};
use px_ui::licenses_view::wrap_text;
use px_ui::text::INTERFACE_CHARSET;

use crate::new_image::WARN_PIXELS;

// ---------------------------------------------------------------------------------------
// Ayudantes: modos, imágenes de la spec 9, textos y formulario.
// ---------------------------------------------------------------------------------------

const TEST_MODES: [Mode; 4] = [Mode::Indexed, Mode::Rgba, Mode::GrayAlpha, Mode::Gray];
const MIB: usize = 1 << 20;

/// Los destinos de `from` en el orden de D2: Indexado · RGBA · Gris con alfa · Gris sin el actual.
fn targets_2(from: Mode) -> [Mode; 3] {
    let v: Vec<Mode> = TEST_MODES.into_iter().filter(|m| *m != from).collect();
    [v[0], v[1], v[2]]
}

fn index_of_target(from: Mode, target: Mode) -> usize {
    targets_2(from)
        .iter()
        .position(|m| *m == target)
        .expect("destino de ese origen")
}

/// Nombre del modo en las opciones de `target_2` (suposición: el de la 19).
fn name_of_mode(mode: Mode) -> &'static str {
    match mode {
        Mode::Indexed => "Indexado",
        Mode::Rgba => "RGBA",
        Mode::GrayAlpha => "Gris con alfa",
        Mode::Gray => "Gris",
    }
}

const ALL_THE_FIELDS: [&str; 6] = [
    "destino",
    "informe",
    "luma_gris_alfa",
    "luma_gris",
    "alfa_gris",
    "fondo_hex",
];

/// Los ids que tiene el formulario de cada origen (tabla de D2).
fn ids_of_form(from: Mode) -> Vec<&'static str> {
    match from {
        Mode::Rgba | Mode::Indexed => ALL_THE_FIELDS.to_vec(),
        Mode::GrayAlpha => vec!["destino", "informe", "alfa_gris", "fondo_hex"],
        Mode::Gray => vec!["destino", "informe"],
    }
}

/// Los ids visibles que corresponden a un destino (índice) y, con destino Gris y alfa, a la opción
/// del grupo del alfa.
fn ids_visible(from: Mode, target_2: usize, alpha_2: usize) -> Vec<&'static str> {
    let target = targets_2(from)[target_2];
    let mut v = vec!["destino", "informe"];
    match (from, target) {
        (Mode::Rgba | Mode::Indexed, Mode::GrayAlpha) => v.push("luma_gris_alfa"),
        (Mode::Rgba | Mode::Indexed, Mode::Gray) => {
            v.push("luma_gris");
            v.push("alfa_gris");
            if alpha_2 == 4 {
                v.push("fondo_hex");
            }
        }
        (Mode::GrayAlpha, Mode::Gray) => {
            v.push("alfa_gris");
            if alpha_2 == 4 {
                v.push("fondo_hex");
            }
        }
        _ => {}
    }
    v
}

fn dialog(from: Mode) -> Dialog {
    Dialog::new(DialogKind::ConvertMode, convert_form(from)).expect("formulario válido")
}

fn chooses(d: &mut Dialog, field: &'static str, index: usize) {
    assert_eq!(d.event(Event::Choice { field, index }), Reaction::None);
}

fn mark(d: &mut Dialog, field: &'static str, on: bool) {
    assert_eq!(d.event(Event::Check { field, on }), Reaction::None);
}

fn writes_hex(d: &mut Dialog, text: &str) {
    assert_eq!(
        d.event(Event::Hex {
            field: "fondo_hex",
            text: text.to_string()
        }),
        Reaction::None
    );
}

fn confirms_hex(d: &mut Dialog) {
    assert_eq!(
        d.event(Event::HexConfirm {
            field: "fondo_hex",
            blur: false
        }),
        Reaction::None
    );
}

fn in_the_set(text_2: &str) -> bool {
    text_2.chars().all(|c| INTERFACE_CHARSET.contains(c))
}

fn field_of<'a>(d: &'a Dialog, id: &str) -> &'a FieldSpec {
    d.spec()
        .fields
        .iter()
        .find(|f| f.id == id)
        .unwrap_or_else(|| panic!("falta el campo {id}"))
}

fn has_field(d: &Dialog, id: &str) -> bool {
    d.spec().fields.iter().any(|f| f.id == id)
}

/// El texto del informe del diálogo, leído de su spec (el `ReadOnly` que cambia `set_report`).
fn report_of(d: &Dialog) -> String {
    match &field_of(d, "informe").kind {
        FieldKind::ReadOnly { text, .. } => text.clone(),
        other_2 => panic!("el informe no es de solo lectura: {other_2:?}"),
    }
}

/// Un texto con los saltos de línea pasados a espacios: para buscar un literal aunque el informe
/// se envuelva.
fn normalized(text_2: &str) -> String {
    text_2
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn lines_of_zone(text_2: &str) -> Vec<String> {
    wrap_text(text_2, 42)
}

fn checks_report(text_2: &str) {
    let lines = lines_of_zone(text_2);
    assert!(
        !lines.is_empty() && lines.len() <= 7,
        "{} líneas en {text_2:?}",
        lines.len()
    );
    for line in &lines {
        assert!(line.chars().count() <= 42, "{line:?}");
    }
    assert!(
        in_the_set(&text_2.replace('\n', " ")),
        "fuera del juego: {text_2:?}"
    );
}

// --- Imágenes (las de la spec 9, «Casos límite») ---

fn rgba_of(pixels: &[Rgba]) -> Image {
    let mut image = Image::new(pixels.len() as u32, 1, Mode::Rgba).expect("imagen RGBA");
    for (x, p) in pixels.iter().enumerate() {
        image
            .set_pixel(x as u32, 0, Pixel::Rgba(*p))
            .expect("píxel");
    }
    image
}

fn gray_of(values: &[u8]) -> Image {
    let mut image = Image::new(values.len() as u32, 1, Mode::Gray).expect("imagen Gray");
    for (x, v) in values.iter().enumerate() {
        image
            .set_pixel(x as u32, 0, Pixel::Gray(*v))
            .expect("píxel");
    }
    image
}

fn gray_alpha_of(values: &[(u8, u8)]) -> Image {
    let mut image = Image::new(values.len() as u32, 1, Mode::GrayAlpha).expect("imagen GrayAlpha");
    for (x, (gray, alpha)) in values.iter().enumerate() {
        image
            .set_pixel(
                x as u32,
                0,
                Pixel::GrayAlpha {
                    gray: *gray,
                    alpha: *alpha,
                },
            )
            .expect("píxel");
    }
    image
}

fn indexed_of(colors: &[Rgba], transparent: Option<u8>, pixels: &[u8]) -> Image {
    let mut palette = Palette::from_colors(colors.iter().copied()).expect("paleta");
    palette
        .set_transparent_index(transparent)
        .expect("índice transparente");
    let mut image = Image::new_indexed(pixels.len() as u32, 1, palette).expect("imagen indexada");
    for (x, i) in pixels.iter().enumerate() {
        image
            .set_pixel(x as u32, 0, Pixel::Indexed(*i))
            .expect("píxel");
    }
    image
}

/// Fila 1: Gray 3×1 `[0, 127, 255]`.
fn gray_row_1() -> Image {
    gray_of(&[0, 127, 255])
}

/// Fila 2: GrayAlpha 3×1 `[(10,0), (10,255), (200,128)]`.
fn gray_alpha_row_2() -> Image {
    gray_alpha_of(&[(10, 0), (10, 255), (200, 128)])
}

/// Fila 7: Rgba 5×1.
fn rgba_row_7() -> Image {
    rgba_of(&[
        Rgba::opaque(5, 5, 5),
        Rgba::new(9, 9, 9, 0),
        Rgba::opaque(5, 5, 5),
        Rgba::new(1, 2, 3, 0),
        Rgba::opaque(7, 7, 7),
    ])
}

/// Fila 9: `c(0)..c(255)` y un `(0,0,0,0)` más (257×1): 257 colores con el transparente.
fn rgba_257_colors() -> Image {
    let mut colors: Vec<Rgba> = (0..256u32)
        .map(|i| Rgba::opaque((i % 256) as u8, (i / 256) as u8, 0))
        .collect();
    colors.push(Rgba::TRANSPARENT);
    rgba_of(&colors)
}

/// Fila 11 de la 9: paleta `[(10,10,10), (200,0,0), (5,5,5)]`, transparente el índice 2.
fn indexed_row_11(pixels: &[u8]) -> Image {
    indexed_of(
        &[
            Rgba::opaque(10, 10, 10),
            Rgba::opaque(200, 0, 0),
            Rgba::opaque(5, 5, 5),
        ],
        Some(2),
        pixels,
    )
}

fn three_reds() -> Image {
    rgba_of(&[Rgba::opaque(255, 0, 0); 3])
}

/// Una imagen cualquiera, pequeña, de cada modo.
fn image_of_mode(mode: Mode) -> Image {
    match mode {
        Mode::Rgba => rgba_of(&[
            Rgba::opaque(255, 0, 0),
            Rgba::new(0, 255, 0, 128),
            Rgba::opaque(7, 7, 7),
            Rgba::new(0, 0, 255, 128),
        ]),
        Mode::Indexed => indexed_row_11(&[0, 1, 1, 2, 2]),
        Mode::GrayAlpha => gray_alpha_of(&[(10, 0), (10, 255), (200, 128), (30, 0)]),
        Mode::Gray => gray_row_1(),
    }
}

/// Una imagen gris de `w × h` a ceros (1 byte por píxel).
fn gray_empty(w: u32, h: u32) -> Image {
    Image::new(w, h, Mode::Gray).expect("imagen Gray")
}

fn lost(color: u64, alpha: u64) -> Analysis {
    Analysis::Lossy(ConversionReport {
        color_lost: color,
        alpha_lost: alpha,
    })
}

fn exact() -> Analysis {
    Analysis::Exact { palette: None }
}

fn exact_with_palette(colors: usize, transparent: bool) -> Analysis {
    Analysis::Exact {
        palette: Some(PaletteInfo {
            colors,
            transparent,
        }),
    }
}

// --- Permisos ---

const NAMES_OF_ALPHA: [&str; 5] = ["Keep", "White", "Black", "Active", "Hex"];

/// Las cinco opciones del grupo del alfa, en el orden del formulario; `hex` es el color de `Hex`.
fn alpha_of(index: usize, hex: Rgba) -> AlphaChoice {
    match index {
        0 => AlphaChoice::Keep,
        1 => AlphaChoice::White,
        2 => AlphaChoice::Black,
        3 => AlphaChoice::Active,
        _ => AlphaChoice::Hex(hex),
    }
}

fn permissions(luma: bool, alpha_2: usize, hex: Rgba) -> Permissions {
    Permissions {
        luma,
        alpha: alpha_of(alpha_2, hex),
    }
}

const TEST_RED: Rgba = Rgba::opaque(255, 0, 0);
const ACTIVE_OPAQUE: Rgba = Rgba::opaque(0x12, 0x34, 0x56);
const ACTIVE_WITH_ALPHA: Rgba = Rgba::new(0xff, 0, 0, 0x80);
const ACTIVE_TRANSPARENT: Rgba = Rgba::TRANSPARENT;

/// Los colores activos de la fila 3: opaco, con alfa, transparente y sin color.
fn active_2_2() -> [Option<Rgba>; 4] {
    [
        Some(ACTIVE_OPAQUE),
        Some(ACTIVE_WITH_ALPHA),
        Some(ACTIVE_TRANSPARENT),
        None,
    ]
}

// --- Textos literales de la spec ---

const PENDING: &str = "calculando el informe";
const WITHOUT_QUANTIZER: &str = "más de 256 colores: sin cuantizador";
const NOT_FITS: &str = "la imagen convertida no cabe";
const MISSING_COLOR: &str = "falta: permitir perder el color";
const CHOOSES_BACKGROUND: &str = "falta: elegir el fondo del alfa";
const MISSING_BOTH: &str = "falta: perder el color; fondo del alfa";
const MISSING_COLOR_AND_VALUE: &str = "falta: perder el color; fondo válido";

const REPORT_COMPUTING: &str = "Calculando el informe…";
const REPORT_EXACT: &str = "Sin pérdidas: la conversión es exacta.";
const REPORT_QUANTIZER: &str = "Esta imagen tiene más de 256 colores y una paleta indexada no los admite. Más de 256 colores: el cuantizador llega en el hito 4; hasta entonces no se puede pasar a Indexado.";
const NOTICE_SIZE: &str = "aviso: más de 16777216 px; puede tardar";
const NOTICE_HISTORY_ALL: &str = "aviso: convertir descarta el historial anterior; solo se podrá deshacer hasta tu próxima edición";
const NOTICE_HISTORY_PART: &str = "aviso: convertir descarta parte del historial anterior";

fn history(limit: usize, used: usize) -> History {
    History { limit, used }
}

/// Bytes de los píxeles de una imagen (lo que cuesta un paso de conversión, sin el margen).
fn bytes_of_pixels(image: &Image) -> usize {
    Image::byte_len(image.width(), image.height(), image.mode()).expect("tamaño válido")
}

/// El aviso de historia que da la spec (D7), calculado aparte.
fn expected_history_notice(image: &Image, limit: usize, used: usize) -> Option<&'static str> {
    let c = bytes_of_pixels(image) as u64 + HISTORY_MARGIN;
    let (limit, used) = (limit as u64, used as u64);
    if c > limit {
        Some(NOTICE_HISTORY_ALL)
    } else if 2 * c > limit && used + c > limit {
        Some(NOTICE_HISTORY_PART)
    } else {
        None
    }
}

// ---------------------------------------------------------------------------------------
// Constantes.
// ---------------------------------------------------------------------------------------

#[test]
fn the_constants_of_the_spec() {
    assert_eq!(SYNC_PIXELS, 1 << 22);
    assert_eq!(SYNC_PIXELS, 4_194_304);
    assert_eq!(WAIT_FRAMES, 2);
    assert_eq!(HISTORY_MARGIN, 2048);
    assert_eq!(WARN_PIXELS, 1 << 24, "el aviso de tamaño es el de la 19");
    assert_eq!(FIELD_TARGET, "destino");
    assert_eq!(FIELD_REPORT, "informe");
    assert_eq!(FIELD_LUMA_GRAY_ALPHA, "luma_gris_alfa");
    assert_eq!(FIELD_LUMA_GRAY, "luma_gris");
    assert_eq!(FIELD_ALPHA, "alfa_gris");
    assert_eq!(FIELD_ALPHA_HEX, "fondo_hex");
}

// ---------------------------------------------------------------------------------------
// Fila 1: formulario por origen.
// ---------------------------------------------------------------------------------------

#[test]
fn row_1_targets_gives_the_other_three_modes_in_the_order_of_decision_2_and_never_the_current() {
    assert_eq!(
        targets(Mode::Indexed),
        [Mode::Rgba, Mode::GrayAlpha, Mode::Gray]
    );
    assert_eq!(
        targets(Mode::Rgba),
        [Mode::Indexed, Mode::GrayAlpha, Mode::Gray]
    );
    assert_eq!(
        targets(Mode::GrayAlpha),
        [Mode::Indexed, Mode::Rgba, Mode::Gray]
    );
    assert_eq!(
        targets(Mode::Gray),
        [Mode::Indexed, Mode::Rgba, Mode::GrayAlpha]
    );
    for from in TEST_MODES {
        assert_eq!(targets(from), targets_2(from), "{from:?}");
        assert!(!targets(from).contains(&from), "{from:?}");
    }
}

#[test]
fn row_1_convert_form_is_valid_with_its_title_and_buttons_for_the_four_sources() {
    for from in TEST_MODES {
        let spec = convert_form(from);
        assert_eq!(spec.title, "Convertir modo", "{from:?}");
        assert_eq!(spec.accept, "Aceptar");
        assert_eq!(spec.cancel, Some("Cancelar"));
        spec.validate()
            .unwrap_or_else(|e| panic!("{from:?}: {e:?}"));
        assert!(spec.check.is_some(), "{from:?}: lleva el check del fondo");
        assert!(
            Dialog::new(DialogKind::ConvertMode, spec).is_ok(),
            "{from:?}"
        );
    }
}

#[test]
fn row_1_the_target_is_the_first_field_a_group_of_three_modes_different_from_the_current() {
    for from in TEST_MODES {
        let d = dialog(from);
        let first_2 = &d.spec().fields[0];
        assert_eq!(first_2.id, FIELD_TARGET, "{from:?}");
        assert_eq!(first_2.label, "Modo de destino", "{from:?}");
        assert!(first_2.shown_when.is_none());
        let FieldKind::Choice { options, default } = &first_2.kind else {
            panic!("{from:?}: destino no es un grupo");
        };
        assert_eq!(options.len(), 3, "{from:?}: solo los tres modos distintos");
        assert_eq!(
            *default, 0,
            "{from:?}: la primera opción es la de por defecto"
        );
        let expected_2: Vec<&str> = targets_2(from).into_iter().map(name_of_mode).collect();
        assert_eq!(options.to_vec(), expected_2, "{from:?}");
        assert!(
            !options.contains(&name_of_mode(from)),
            "{from:?}: el mismo modo no se ofrece"
        );
    }
}

#[test]
fn row_1_the_report_is_read_only_with_seven_rows_and_goes_second() {
    for from in TEST_MODES {
        let d = dialog(from);
        let second = &d.spec().fields[1];
        assert_eq!(second.id, FIELD_REPORT, "{from:?}");
        assert!(second.shown_when.is_none());
        match &second.kind {
            FieldKind::ReadOnly { text, rows } => {
                assert_eq!(*rows, Rows::Fixed(7), "{from:?}");
                checks_report(text);
            }
            other_2 => panic!("{from:?}: el informe es {other_2:?}"),
        }
    }
}

#[test]
fn row_1_there_are_only_the_fields_of_the_table_of_decision_2_per_source() {
    for from in TEST_MODES {
        let d = dialog(from);
        let mut real_2: Vec<&str> = d.spec().fields.iter().map(|f| f.id).collect();
        let mut expected_2_2_2 = ids_of_form(from);
        real_2.sort_unstable();
        expected_2_2_2.sort_unstable();
        assert_eq!(real_2, expected_2_2_2, "{from:?}");
    }
}

#[test]
fn row_1_each_field_shows_only_with_the_target_and_the_option_that_apply_to_it() {
    for from in TEST_MODES {
        for target_2 in 0..3 {
            for alpha_2 in 0..5 {
                let mut d = dialog(from);
                chooses(&mut d, "destino", target_2);
                if has_field(&d, "alfa_gris") {
                    // El grupo del alfa puede cambiarse mientras es visible (destino Gris) o,
                    // como estado conservado, estando oculto: se fija desde el destino Gris.
                    let gray = index_of_target(from, Mode::Gray);
                    chooses(&mut d, "destino", gray);
                    chooses(&mut d, "alfa_gris", alpha_2);
                    chooses(&mut d, "destino", target_2);
                }
                let expected_2_2_2 = ids_visible(from, target_2, alpha_2);
                for id in ALL_THE_FIELDS {
                    assert_eq!(
                        d.is_shown(id),
                        expected_2_2_2.contains(&id),
                        "{from:?}, destino {target_2}, alfa {alpha_2}: {id}"
                    );
                }
            }
        }
    }
}

#[test]
fn row_1_background_hex_is_transitive_if_the_alpha_group_is_hidden_so_is_the_hexadecimal() {
    for from in [Mode::Rgba, Mode::Indexed, Mode::GrayAlpha] {
        let mut d = dialog(from);
        let gray = index_of_target(from, Mode::Gray);
        chooses(&mut d, "destino", gray);
        assert!(d.is_shown("alfa_gris"), "{from:?}");
        assert!(
            !d.is_shown("fondo_hex"),
            "{from:?}: con «No perder el alfa»"
        );
        chooses(&mut d, "alfa_gris", 4);
        assert!(d.is_shown("fondo_hex"), "{from:?}");
        // Se oculta el grupo (otro destino): el hexadecimal también, aunque conserve la opción 4.
        chooses(&mut d, "destino", 0);
        assert!(!d.is_shown("alfa_gris"), "{from:?}");
        assert!(!d.is_shown("fondo_hex"), "{from:?}: el control está oculto");
        // Y vuelven juntos.
        chooses(&mut d, "destino", gray);
        assert!(
            d.is_shown("alfa_gris") && d.is_shown("fondo_hex"),
            "{from:?}"
        );
    }
}

#[test]
fn row_1_the_shown_when_only_point_back_and_to_the_options_of_decision_2() {
    for from in [Mode::Rgba, Mode::Indexed, Mode::GrayAlpha] {
        let d = dialog(from);
        let gray = index_of_target(from, Mode::Gray);
        let ShowWhen { field, choice } = field_of(&d, "alfa_gris")
            .shown_when
            .expect("depende del destino");
        assert_eq!((field, choice), ("destino", gray), "{from:?}");
        let ShowWhen { field, choice } = field_of(&d, "fondo_hex")
            .shown_when
            .expect("depende del alfa");
        assert_eq!((field, choice), ("alfa_gris", 4), "{from:?}");
    }
    for from in [Mode::Rgba, Mode::Indexed] {
        let d = dialog(from);
        let ShowWhen { field, choice } = field_of(&d, "luma_gris_alfa")
            .shown_when
            .expect("depende del destino");
        assert_eq!(
            (field, choice),
            ("destino", index_of_target(from, Mode::GrayAlpha)),
            "{from:?}"
        );
        let ShowWhen { field, choice } = field_of(&d, "luma_gris")
            .shown_when
            .expect("depende del destino");
        assert_eq!(
            (field, choice),
            ("destino", index_of_target(from, Mode::Gray)),
            "{from:?}"
        );
    }
}

#[test]
fn row_1_the_labels_and_options_are_the_literals_of_decision_2() {
    let d = dialog(Mode::Rgba);
    for id in ["luma_gris_alfa", "luma_gris"] {
        let field_2 = field_of(&d, id);
        assert_eq!(
            field_2.label, "Permitir perder el color (pasar a gris)",
            "{id}"
        );
        assert_eq!(field_2.kind, FieldKind::Check { default: false }, "{id}");
    }
    let alpha_2 = field_of(&d, "alfa_gris");
    assert_eq!(alpha_2.label, "Si hay alfa, componer sobre:");
    match &alpha_2.kind {
        FieldKind::Choice { options, default } => {
            assert_eq!(
                options.to_vec(),
                [
                    "No perder el alfa",
                    "Fondo blanco",
                    "Fondo negro",
                    "Fondo: el color activo",
                    "Fondo: un color hexadecimal",
                ]
            );
            assert_eq!(*default, 0, "«No perder el alfa» es la de por defecto");
        }
        other_2 => panic!("el alfa no es un grupo: {other_2:?}"),
    }
    let hex = field_of(&d, "fondo_hex");
    assert_eq!(hex.label, "Color");
    assert_eq!(
        hex.kind,
        FieldKind::Hex {
            mode: Mode::Rgba,
            default: Rgba::new(0x80, 0x80, 0x80, 0xff)
        }
    );
}

#[test]
fn row_1_all_fixed_text_is_in_the_set_and_the_box_and_option_labels_fit() {
    for from in TEST_MODES {
        let spec = convert_form(from);
        let mut texts: Vec<&str> = vec![spec.title, spec.accept];
        texts.extend(spec.cancel);
        for f in &spec.fields {
            texts.push(f.label);
            match &f.kind {
                FieldKind::Choice { options, .. } => {
                    for o in options.iter().copied() {
                        assert!(o.chars().count() <= 42, "{from:?}: {o:?}");
                        texts.push(o);
                    }
                }
                FieldKind::Check { .. } => {
                    assert!(f.label.chars().count() <= 42, "{from:?}: {:?}", f.label);
                }
                _ => {}
            }
        }
        for text_2 in texts {
            assert!(in_the_set(text_2), "{from:?}: {text_2:?} fuera del juego");
        }
    }
}

#[test]
fn row_1_on_opening_there_is_no_reason_no_box_checked_and_the_alpha_on_do_not_lose() {
    for from in TEST_MODES {
        let mut d = dialog(from);
        assert_eq!(d.reason(), None, "{from:?}");
        assert!(d.is_valid(), "{from:?}");
        assert!(d.values().is_some(), "{from:?}");
        assert_eq!(d.chosen("destino"), Some(0), "{from:?}");
        // Los valores por defecto de la spec: casillas sin marcar y el alfa en la opción 0.
        for f in &d.spec().fields {
            match &f.kind {
                FieldKind::Check { default } => assert!(!default, "{from:?}: {}", f.id),
                FieldKind::Choice { default, .. } => assert_eq!(*default, 0, "{from:?}: {}", f.id),
                _ => {}
            }
        }
        // Y las casillas visibles lo leen sin marcar.
        for target_2 in 0..3 {
            chooses(&mut d, "destino", target_2);
            for id in ["luma_gris_alfa", "luma_gris"] {
                if d.is_shown(id) {
                    assert_eq!(d.checked(id), Some(false), "{from:?}: {id}");
                }
            }
            if d.is_shown("alfa_gris") {
                assert_eq!(d.chosen("alfa_gris"), Some(0), "{from:?}");
            }
        }
    }
}

#[test]
fn row_1_open_convert_mode_opens_the_dialog_with_the_computing_reason_and_the_focus_on_the_target()
{
    for from in TEST_MODES {
        let mut modal = Modal::default();
        assert!(
            open_convert_mode(&mut modal, from, Locks::default()),
            "{from:?}"
        );
        assert_eq!(modal.kind(), Some(DialogKind::ConvertMode));
        let Modal::Open(d) = &modal else {
            panic!("{from:?}: cerrado");
        };
        assert_eq!(d.spec().title, "Convertir modo");
        assert_eq!(d.block(), Some(PENDING), "{from:?}");
        assert_eq!(d.reason(), Some(PENDING.to_string()), "{from:?}");
        assert!(!d.is_valid(), "{from:?}: Aceptar atenuado hasta calcular");
        assert_eq!(
            initial_focus(d),
            FocusTarget::Option("destino", 0),
            "{from:?}"
        );
    }
}

#[test]
fn row_1_open_convert_mode_gives_false_with_a_lock_or_with_another_dialog_open() {
    for locks in [
        Locks {
            gesture: true,
            ..Locks::default()
        },
        Locks {
            dialog: true,
            ..Locks::default()
        },
        Locks {
            text_focus: true,
            ..Locks::default()
        },
        Locks {
            removal: true,
            ..Locks::default()
        },
        Locks {
            modal: true,
            ..Locks::default()
        },
    ] {
        let mut modal = Modal::default();
        assert!(
            !open_convert_mode(&mut modal, Mode::Rgba, locks),
            "{locks:?}"
        );
        assert!(!modal.is_open(), "{locks:?}");
    }
    let mut modal = Modal::default();
    assert!(open_convert_mode(&mut modal, Mode::Rgba, Locks::default()));
    let before = modal.clone();
    assert!(!open_convert_mode(&mut modal, Mode::Gray, Locks::default()));
    assert_eq!(modal, before, "no cambia el diálogo abierto");
}

// ---------------------------------------------------------------------------------------
// Fila 2: del informe de `convert_image` a `Analysis`.
// ---------------------------------------------------------------------------------------

#[test]
fn row_2_three_reds_to_gray_loses_the_color_in_three_pixels() {
    assert_eq!(analyze(&three_reds(), Mode::Gray), lost(3, 0));
}

#[test]
fn row_2_semitransparent_red_to_gray_loses_color_and_alpha_in_one_pixel() {
    let image = rgba_of(&[Rgba::new(255, 0, 0, 128)]);
    assert_eq!(analyze(&image, Mode::Gray), lost(1, 1));
}

#[test]
fn row_2_gray_alpha_of_row_2_to_gray_loses_the_alpha_in_two_pixels() {
    assert_eq!(analyze(&gray_alpha_row_2(), Mode::Gray), lost(0, 2));
}

#[test]
fn row_2_gray_to_rgba_and_to_gray_alpha_is_exact_without_a_palette() {
    for target in [Mode::Rgba, Mode::GrayAlpha] {
        assert_eq!(analyze(&gray_row_1(), target), exact(), "{target:?}");
    }
}

#[test]
fn row_2_gray_to_indexed_is_exact_with_a_new_palette_of_three_colors_without_transparent() {
    assert_eq!(
        analyze(&gray_row_1(), Mode::Indexed),
        exact_with_palette(3, false)
    );
}

#[test]
fn row_2_rgba_of_row_7_to_indexed_gives_three_colors_with_zero_transparent() {
    assert_eq!(
        analyze(&rgba_row_7(), Mode::Indexed),
        exact_with_palette(3, true)
    );
}

#[test]
fn row_2_rgba_of_257_colors_to_indexed_is_too_many_colors() {
    assert_eq!(
        analyze(&rgba_257_colors(), Mode::Indexed),
        Analysis::TooManyColors
    );
}

#[test]
fn row_2_indexed_of_row_11_with_ones_and_zero_to_gray_loses_the_color_in_two_pixels() {
    assert_eq!(analyze(&indexed_row_11(&[0, 1, 1]), Mode::Gray), lost(2, 0));
}

#[test]
fn row_2_classify_of_each_result() {
    let report = ConversionReport {
        color_lost: 4,
        alpha_lost: 5,
    };
    assert_eq!(
        classify(Err(ConvertError::LossNotAllowed { report })),
        Analysis::Lossy(report)
    );
    assert_eq!(
        classify(Err(ConvertError::TooManyColors { at_least: 257 })),
        Analysis::TooManyColors
    );
    assert_eq!(
        classify(Err(ConvertError::TooLarge {
            width: 16385,
            height: 16384
        })),
        Analysis::TooLarge
    );
    // `Ok`: exacta; a indexado se lee la paleta de la conversión.
    let a_gray_alpha = convert_image(&gray_row_1(), Mode::GrayAlpha, Allowed::NOTHING);
    assert_eq!(classify(a_gray_alpha), exact());
    let a_indexed = convert_image(&rgba_row_7(), Mode::Indexed, Allowed::NOTHING);
    assert_eq!(classify(a_indexed), exact_with_palette(3, true));
    let a_indexed = convert_image(&gray_row_1(), Mode::Indexed, Allowed::NOTHING);
    assert_eq!(classify(a_indexed), exact_with_palette(3, false));
}

#[test]
fn row_2_classify_of_too_large_does_not_reserve_memory_for_any_dimension() {
    for (width, height) in [(1u32, 1u32), (16385, 16384), (u32::MAX, u32::MAX)] {
        assert_eq!(
            classify(Err(ConvertError::TooLarge { width, height })),
            Analysis::TooLarge
        );
    }
}

#[test]
fn row_2_analyze_matches_classify_of_convert_image_without_permissions() {
    let cases: Vec<(Image, Mode)> = vec![
        (three_reds(), Mode::Gray),
        (gray_alpha_row_2(), Mode::Gray),
        (gray_alpha_row_2(), Mode::Indexed),
        (gray_row_1(), Mode::Indexed),
        (rgba_row_7(), Mode::Indexed),
        (rgba_257_colors(), Mode::Indexed),
        (indexed_row_11(&[0, 1, 1]), Mode::Gray),
        (indexed_row_11(&[0, 1, 1]), Mode::Rgba),
    ];
    for (image, target) in cases {
        assert_eq!(
            analyze(&image, target),
            classify(convert_image(&image, target, Allowed::NOTHING)),
            "{:?} a {target:?}",
            image.mode()
        );
    }
}

#[test]
fn row_2_analyze_does_not_touch_the_document() {
    let document = Document::new(rgba_row_7());
    let id = ImageId::from_raw(0);
    let before = document.image(id).expect("imagen").clone();
    let state_id = document.state_id();
    for target in [Mode::Indexed, Mode::Gray, Mode::GrayAlpha] {
        let _ = analyze(document.image(id).expect("imagen"), target);
    }
    assert_eq!(document.state_id(), state_id);
    assert!(!document.can_undo());
    assert!(!document.can_redo());
    assert_eq!(document.image(id), Some(&before));
}

#[test]
fn row_2_the_report_does_not_depend_on_the_permissions_with_nothing_it_comes_out_whole() {
    // `LossNotAllowed` lleva los dos contadores completos: con cualquier `Allowed` el informe de
    // `analyze` es el mismo (analyze usa siempre NOTHING).
    let image = rgba_of(&[
        Rgba::opaque(255, 0, 0),
        Rgba::new(0, 255, 0, 128),
        Rgba::new(1, 2, 3, 0),
        Rgba::opaque(9, 9, 9),
    ]);
    assert_eq!(analyze(&image, Mode::Gray), lost(2, 2));
    assert_eq!(analyze(&image, Mode::GrayAlpha), lost(2, 0));
}

// ---------------------------------------------------------------------------------------
// Fila 3: cobertura y motivo.
// ---------------------------------------------------------------------------------------

/// Las siete formas de `Analysis` que interesan: sin pérdida, con paleta, solo color, solo alfa,
/// las dos cosas y los dos imposibles.
fn analysis_of_row_3() -> Vec<(&'static str, Analysis)> {
    vec![
        ("Exact", exact()),
        ("Exact con paleta", exact_with_palette(5, true)),
        ("Lossy 2/0", lost(2, 0)),
        ("Lossy 0/3", lost(0, 3)),
        ("Lossy 2/3", lost(2, 3)),
        ("TooManyColors", Analysis::TooManyColors),
        ("TooLarge", Analysis::TooLarge),
    ]
}

/// El motivo que da la spec (D4), calculado aparte y en el orden de D4.
fn reason_expected(
    analysis: Option<&Analysis>,
    luma: bool,
    alpha_2: usize,
    active_2: Option<Rgba>,
) -> Option<String> {
    let Some(analysis) = analysis else {
        return Some(PENDING.to_string());
    };
    let report = match analysis {
        Analysis::TooManyColors => return Some(WITHOUT_QUANTIZER.to_string()),
        Analysis::TooLarge => return Some(NOT_FITS.to_string()),
        Analysis::Blocked(block) => return Some(block.reason().to_string()),
        Analysis::Exact { .. } => ConversionReport::default(),
        Analysis::Lossy(report) => *report,
    };
    let missing_color = report.color_lost > 0 && !luma;
    let missing_background = report.alpha_lost > 0 && alpha_2 == 0;
    // Un fondo «color activo» sin color o con alfa es un valor inválido.
    let value_bad = alpha_2 == 3 && active_2.is_none_or(|c| c.a < 255);
    if missing_color && missing_background {
        return Some(MISSING_BOTH.to_string());
    }
    if missing_color && value_bad {
        return Some(MISSING_COLOR_AND_VALUE.to_string());
    }
    if missing_color {
        return Some(MISSING_COLOR.to_string());
    }
    if missing_background {
        return Some(CHOOSES_BACKGROUND.to_string());
    }
    // Un fondo «color activo» no opaco se rechaza siempre que esté elegido, haga falta el alfa o no.
    if alpha_2 == 3 {
        match active_2 {
            None => return Some("no hay color activo".to_string()),
            Some(c) if c.a < 255 => {
                return Some(format!(
                    "el color activo #{:02x}{:02x}{:02x}{:02x} tiene alfa",
                    c.r, c.g, c.b, c.a
                ));
            }
            Some(_) => {}
        }
    }
    None
}

#[test]
fn row_3_needs_asks_for_color_and_alpha_only_when_there_are_lost_pixels() {
    for (name_2, analysis, color, alpha_2) in [
        ("Exact", exact(), false, false),
        (
            "Exact con paleta",
            exact_with_palette(3, false),
            false,
            false,
        ),
        ("Lossy 3/0", lost(3, 0), true, false),
        ("Lossy 0/2", lost(0, 2), false, true),
        ("Lossy 1/1", lost(1, 1), true, true),
        ("TooManyColors", Analysis::TooManyColors, false, false),
        ("TooLarge", Analysis::TooLarge, false, false),
    ] {
        let n = needs(&analysis);
        assert_eq!((n.luma, n.alpha), (color, alpha_2), "{name_2}");
    }
}

#[test]
fn row_3_exact_has_no_reason_with_any_permission_checked_except_a_non_opaque_active_one() {
    for (name_2, analysis) in analysis_of_row_3().into_iter().take(2) {
        for luma in [false, true] {
            for alpha_2 in 0..5 {
                for active_2 in active_2_2() {
                    assert_eq!(
                        block_reason(
                            Some(&analysis),
                            &permissions(luma, alpha_2, TEST_RED),
                            active_2
                        ),
                        // Solo un fondo «color activo» no opaco estorba, haga falta el alfa o no.
                        reason_expected(Some(&analysis), luma, alpha_2, active_2),
                        "{name_2}, luma {luma}, alfa {}, activo {active_2:?}",
                        NAMES_OF_ALPHA[alpha_2]
                    );
                }
            }
        }
    }
}

#[test]
fn row_3_lossy_of_color_only_asks_for_the_box_and_nothing_else() {
    let analysis = lost(2, 0);
    for alpha_2 in 0..5 {
        for active_2 in active_2_2() {
            // Un fondo «color activo» sin color o con alfa se suma como valor inválido.
            let expected_2 = if alpha_2 == 3 && active_2.is_none_or(|c| c.a < 255) {
                MISSING_COLOR_AND_VALUE
            } else {
                MISSING_COLOR
            };
            assert_eq!(
                block_reason(
                    Some(&analysis),
                    &permissions(false, alpha_2, TEST_RED),
                    active_2
                ),
                Some(expected_2.to_string()),
                "alfa {}",
                NAMES_OF_ALPHA[alpha_2]
            );
            assert_eq!(
                block_reason(
                    Some(&analysis),
                    &permissions(true, alpha_2, TEST_RED),
                    active_2
                ),
                // El fondo no hace falta, salvo que sea un «color activo» no opaco.
                reason_expected(Some(&analysis), true, alpha_2, active_2),
                "alfa {}: el fondo no hace falta",
                NAMES_OF_ALPHA[alpha_2]
            );
        }
    }
}

#[test]
fn row_3_lossy_of_alpha_only_asks_for_the_background_and_an_opaque_background_covers_it() {
    let analysis = lost(0, 3);
    for luma in [false, true] {
        // Sin fondo: «No perder el alfa».
        assert_eq!(
            block_reason(
                Some(&analysis),
                &permissions(luma, 0, TEST_RED),
                Some(ACTIVE_OPAQUE)
            ),
            Some(CHOOSES_BACKGROUND.to_string()),
            "luma {luma}"
        );
        // Blanco, negro, hexadecimal opaco y activo opaco cubren el alfa.
        for alpha_2 in 1..5 {
            assert_eq!(
                block_reason(
                    Some(&analysis),
                    &permissions(luma, alpha_2, TEST_RED),
                    Some(ACTIVE_OPAQUE)
                ),
                None,
                "luma {luma}, alfa {}",
                NAMES_OF_ALPHA[alpha_2]
            );
        }
    }
}

#[test]
fn row_3_lossy_of_color_and_alpha_lists_both_and_each_permission_removes_its_own() {
    let analysis = lost(2, 3);
    // Ni casilla ni fondo: salen los dos, en el orden color y fondo.
    assert_eq!(
        block_reason(
            Some(&analysis),
            &permissions(false, 0, TEST_RED),
            Some(ACTIVE_OPAQUE)
        ),
        Some(MISSING_BOTH.to_string())
    );
    // Con la casilla y sin fondo: el fondo.
    assert_eq!(
        block_reason(
            Some(&analysis),
            &permissions(true, 0, TEST_RED),
            Some(ACTIVE_OPAQUE)
        ),
        Some(CHOOSES_BACKGROUND.to_string())
    );
    // Con fondo y sin casilla: el color.
    assert_eq!(
        block_reason(
            Some(&analysis),
            &permissions(false, 1, TEST_RED),
            Some(ACTIVE_OPAQUE)
        ),
        Some(MISSING_COLOR.to_string())
    );
    // Con las dos cosas: ninguno.
    assert_eq!(
        block_reason(
            Some(&analysis),
            &permissions(true, 2, TEST_RED),
            Some(ACTIVE_OPAQUE)
        ),
        None
    );
}

#[test]
fn row_3_the_active_color_with_alpha_gives_its_resolved_value_and_without_color_it_says_there_is_none()
 {
    let analysis = lost(0, 1);
    let reason_2 = block_reason(
        Some(&analysis),
        &permissions(false, 3, TEST_RED),
        Some(Rgba::new(0xff, 0, 0, 0x80)),
    )
    .expect("hay motivo");
    assert_eq!(reason_2, "el color activo #ff000080 tiene alfa");
    assert_eq!(reason_2.chars().count(), 36);
    // La entrada transparente: `#00000000`.
    assert_eq!(
        block_reason(
            Some(&analysis),
            &permissions(false, 3, TEST_RED),
            Some(Rgba::TRANSPARENT)
        ),
        Some("el color activo #00000000 tiene alfa".to_string())
    );
    assert_eq!(
        block_reason(Some(&analysis), &permissions(false, 3, TEST_RED), None),
        Some("no hay color activo".to_string())
    );
    // Con el color activo opaco, sin motivo.
    assert_eq!(
        block_reason(
            Some(&analysis),
            &permissions(false, 3, TEST_RED),
            Some(ACTIVE_OPAQUE)
        ),
        None
    );
}

#[test]
fn row_3_a_non_opaque_active_color_blocks_even_if_the_alpha_is_not_lost() {
    // Elegido «color activo» como fondo, un color sin alfa opaco se rechaza siempre (no se deja
    // `Accept` activo sin `Plan`); con un color opaco no estorba.
    for analysis in [exact(), lost(2, 0)] {
        for active_2 in active_2_2() {
            assert_eq!(
                block_reason(Some(&analysis), &permissions(true, 3, TEST_RED), active_2),
                reason_expected(Some(&analysis), true, 3, active_2),
                "activo {active_2:?}"
            );
        }
        assert_eq!(
            block_reason(
                Some(&analysis),
                &permissions(true, 3, TEST_RED),
                Some(ACTIVE_OPAQUE)
            ),
            None
        );
        assert_eq!(
            block_reason(
                Some(&analysis),
                &permissions(true, 3, TEST_RED),
                Some(ACTIVE_WITH_ALPHA)
            ),
            Some("el color activo #ff000080 tiene alfa".to_string())
        );
        // Con otra opción del grupo, el color activo no cuenta.
        assert_eq!(
            block_reason(
                Some(&analysis),
                &permissions(true, 1, TEST_RED),
                Some(ACTIVE_WITH_ALPHA)
            ),
            None
        );
    }
}

#[test]
fn row_3_the_impossible_ones_always_have_a_reason_with_any_permission() {
    for (analysis, expected_2) in [
        (Analysis::TooManyColors, WITHOUT_QUANTIZER),
        (Analysis::TooLarge, NOT_FITS),
    ] {
        for luma in [false, true] {
            for alpha_2 in 0..5 {
                for active_2 in active_2_2() {
                    assert_eq!(
                        block_reason(
                            Some(&analysis),
                            &permissions(luma, alpha_2, TEST_RED),
                            active_2
                        ),
                        Some(expected_2.to_string()),
                        "luma {luma}, alfa {}, activo {active_2:?}",
                        NAMES_OF_ALPHA[alpha_2]
                    );
                }
            }
        }
    }
}

#[test]
fn row_3_pending_says_computing_the_report_with_any_permission() {
    for luma in [false, true] {
        for alpha_2 in 0..5 {
            for active_2 in active_2_2() {
                assert_eq!(
                    block_reason(None, &permissions(luma, alpha_2, TEST_RED), active_2),
                    Some(PENDING.to_string()),
                    "luma {luma}, alfa {}",
                    NAMES_OF_ALPHA[alpha_2]
                );
            }
        }
    }
}

#[test]
fn row_3_the_reason_texts_are_the_literals_and_measure_at_most_37() {
    assert_eq!(PENDING.chars().count(), 21);
    assert_eq!(WITHOUT_QUANTIZER.chars().count(), 35);
    for text_2 in [
        PENDING,
        WITHOUT_QUANTIZER,
        NOT_FITS,
        MISSING_COLOR,
        CHOOSES_BACKGROUND,
    ] {
        assert!(text_2.chars().count() <= 37, "{text_2:?}");
        assert!(in_the_set(text_2), "{text_2:?}");
    }
}

#[test]
fn row_3_full_matrix_against_the_oracle_of_the_order_of_d4() {
    let mut checked = 0;
    for (name_2, analysis) in analysis_of_row_3() {
        for luma in [false, true] {
            for alpha_2 in 0..5 {
                for active_2 in active_2_2() {
                    let real = block_reason(
                        Some(&analysis),
                        &permissions(luma, alpha_2, TEST_RED),
                        active_2,
                    );
                    let expected_2 = reason_expected(Some(&analysis), luma, alpha_2, active_2);
                    assert_eq!(
                        real, expected_2,
                        "{name_2}, luma {luma}, alfa {}, activo {active_2:?}",
                        NAMES_OF_ALPHA[alpha_2]
                    );
                    if let Some(text_2) = real {
                        let limit_2 = if text_2.starts_with("falta: ") {
                            44
                        } else {
                            37
                        };
                        assert!(text_2.chars().count() <= limit_2, "{text_2:?}");
                        assert!(in_the_set(&text_2), "{text_2:?}");
                    }
                    checked += 1;
                }
            }
        }
    }
    // Y con el informe pendiente.
    for luma in [false, true] {
        for alpha_2 in 0..5 {
            for active_2 in active_2_2() {
                assert_eq!(
                    block_reason(None, &permissions(luma, alpha_2, TEST_RED), active_2),
                    reason_expected(None, luma, alpha_2, active_2)
                );
                checked += 1;
            }
        }
    }
    assert_eq!(checked, 7 * 2 * 5 * 4 + 2 * 5 * 4);
}

// ---------------------------------------------------------------------------------------
// Fila 4: del formulario a `Allowed`.
// ---------------------------------------------------------------------------------------

#[test]
fn row_4_allowed_of_gives_the_background_of_each_option_and_the_color_box() {
    let active_2 = Some(Rgba::opaque(0x12, 0x34, 0x56));
    for luma in [false, true] {
        let cases: [(AlphaChoice, Option<Allowed>); 5] = [
            (
                AlphaChoice::Keep,
                Some(Allowed {
                    luma,
                    drop_alpha: None,
                }),
            ),
            (
                AlphaChoice::White,
                Some(Allowed {
                    luma,
                    drop_alpha: Some([255, 255, 255]),
                }),
            ),
            (
                AlphaChoice::Black,
                Some(Allowed {
                    luma,
                    drop_alpha: Some([0, 0, 0]),
                }),
            ),
            (
                AlphaChoice::Active,
                Some(Allowed {
                    luma,
                    drop_alpha: Some([0x12, 0x34, 0x56]),
                }),
            ),
            (
                AlphaChoice::Hex(Rgba::opaque(0xff, 0x00, 0x00)),
                Some(Allowed {
                    luma,
                    drop_alpha: Some([255, 0, 0]),
                }),
            ),
        ];
        for (alpha, expected_2) in cases {
            let perms = Permissions { luma, alpha };
            assert_eq!(allowed_of(&perms, active_2), expected_2, "luma {luma}");
        }
    }
}

#[test]
fn row_4_allowed_of_gives_none_if_the_background_is_not_opaque() {
    let active_with_alpha = Some(ACTIVE_WITH_ALPHA);
    for luma in [false, true] {
        for (name_2, perms, active_2) in [
            (
                "hex con alfa",
                Permissions {
                    luma,
                    alpha: AlphaChoice::Hex(Rgba::new(0xff, 0, 0, 0x80)),
                },
                None,
            ),
            (
                "activo con alfa",
                Permissions {
                    luma,
                    alpha: AlphaChoice::Active,
                },
                active_with_alpha,
            ),
            (
                "activo transparente",
                Permissions {
                    luma,
                    alpha: AlphaChoice::Active,
                },
                Some(Rgba::TRANSPARENT),
            ),
            (
                "sin activo",
                Permissions {
                    luma,
                    alpha: AlphaChoice::Active,
                },
                None,
            ),
        ] {
            assert_eq!(allowed_of(&perms, active_2), None, "{name_2}, luma {luma}");
        }
    }
}

/// Un formulario de `from` en el que se han marcado las casillas y el grupo del alfa **mientras
/// eran visibles** y se deja en el destino final (así una casilla de otro destino queda oculta
/// pero marcada).
fn prepared(
    from: Mode,
    target_final: usize,
    luma_gray_alpha: bool,
    luma_gray: bool,
    alpha_2: usize,
    hex: Option<&str>,
) -> Dialog {
    let mut d = dialog(from);
    if has_field(&d, "luma_gris_alfa") {
        chooses(&mut d, "destino", index_of_target(from, Mode::GrayAlpha));
        mark(&mut d, "luma_gris_alfa", luma_gray_alpha);
    }
    if has_field(&d, "luma_gris") {
        chooses(&mut d, "destino", index_of_target(from, Mode::Gray));
        mark(&mut d, "luma_gris", luma_gray);
    }
    if has_field(&d, "alfa_gris") {
        chooses(&mut d, "destino", index_of_target(from, Mode::Gray));
        chooses(&mut d, "alfa_gris", alpha_2);
        if let Some(text_2) = hex.filter(|_| alpha_2 == 4) {
            writes_hex(&mut d, text_2);
        }
    }
    chooses(&mut d, "destino", target_final);
    d
}

/// El `Allowed` esperado para un destino, con el modelo de la spec escrito aparte. `None` si el
/// fondo elegido no vale (activo sin color opaco).
fn allowed_expected(
    from: Mode,
    target_2: usize,
    luma_gray_alpha: bool,
    luma_gray: bool,
    alpha_2: usize,
    hex_opaque: Option<Rgba>,
    active_2: Option<Rgba>,
) -> Option<Allowed> {
    let target = targets_2(from)[target_2];
    let luma = match (from, target) {
        (Mode::Rgba | Mode::Indexed, Mode::GrayAlpha) => luma_gray_alpha,
        (Mode::Rgba | Mode::Indexed, Mode::Gray) => luma_gray,
        _ => false,
    };
    let with_group = target == Mode::Gray && from != Mode::Gray;
    let drop_alpha = if with_group {
        match alpha_2 {
            0 => None,
            1 => Some([255, 255, 255]),
            2 => Some([0, 0, 0]),
            3 => {
                let c = active_2.filter(|c| c.a == 255)?;
                Some([c.r, c.g, c.b])
            }
            _ => {
                let c = hex_opaque?;
                Some([c.r, c.g, c.b])
            }
        }
    } else {
        None
    };
    Some(Allowed { luma, drop_alpha })
}

#[test]
fn row_4_plan_from_values_in_all_combinations_of_boxes_and_alpha() {
    let active_2 = Some(Rgba::opaque(0x0a, 0x0b, 0x0c));
    let mut checked = 0;
    for from in TEST_MODES {
        for target_2 in 0..3 {
            for luma_ga in [false, true] {
                for luma_g in [false, true] {
                    for alpha_2 in 0..4 {
                        let d = prepared(from, target_2, luma_ga, luma_g, alpha_2, None);
                        let values = d.values().expect("todo válido");
                        let expected_2 = allowed_expected(
                            from, target_2, luma_ga, luma_g, alpha_2, None, active_2,
                        )
                        .map(|allowed| Plan {
                            target: targets_2(from)[target_2],
                            allowed,
                        });
                        let plan = plan_from_values(&values, from, active_2);
                        assert_eq!(
                            plan, expected_2,
                            "{from:?}, destino {target_2}, ga {luma_ga}, g {luma_g}, alfa {alpha_2}"
                        );
                        if let Some(plan) = plan {
                            assert_ne!(plan.target, from, "nunca el modo actual");
                            assert_eq!(plan.target, targets(from)[target_2]);
                        }
                        checked += 1;
                    }
                }
            }
        }
    }
    assert_eq!(checked, 4 * 3 * 2 * 2 * 4);
}

#[test]
fn row_4_the_box_of_a_hidden_target_checked_does_not_enter_allowed() {
    for from in [Mode::Rgba, Mode::Indexed] {
        // Marcada la de «Gris» y elegido «Gris con alfa» (su propia casilla sin marcar).
        let d = prepared(
            from,
            index_of_target(from, Mode::GrayAlpha),
            false,
            true,
            0,
            None,
        );
        assert!(!d.is_shown("luma_gris"));
        assert_eq!(d.checked("luma_gris"), Some(true));
        let plan = plan_from_values(&d.values().expect("válido"), from, None).expect("plan");
        assert_eq!(plan.target, Mode::GrayAlpha);
        assert!(!plan.allowed.luma, "{from:?}: la casilla oculta no cuenta");
        assert_eq!(plan.allowed.drop_alpha, None);

        // Y al revés.
        let d = prepared(
            from,
            index_of_target(from, Mode::Gray),
            true,
            false,
            0,
            None,
        );
        assert!(!d.is_shown("luma_gris_alfa"));
        assert_eq!(d.checked("luma_gris_alfa"), Some(true));
        let plan = plan_from_values(&d.values().expect("válido"), from, None).expect("plan");
        assert_eq!(plan.target, Mode::Gray);
        assert!(!plan.allowed.luma, "{from:?}: la casilla oculta no cuenta");

        // A un destino sin casilla (Indexado, RGBA): nunca hay `luma`, aunque las dos estén marcadas.
        for target in [Mode::Indexed, Mode::Rgba] {
            let target_2 = targets_2(from).iter().position(|m| *m == target);
            let Some(target_2) = target_2 else { continue };
            let d = prepared(from, target_2, true, true, 4, Some("#ff0000"));
            let plan = plan_from_values(&d.values().expect("válido"), from, None).expect("plan");
            assert_eq!(plan.target, target);
            assert_eq!(
                plan.allowed,
                Allowed {
                    luma: false,
                    drop_alpha: None
                },
                "{from:?} a {target:?}"
            );
        }
    }
}

#[test]
fn row_4_the_alpha_of_the_hidden_group_does_not_enter_allowed_for_a_target_that_keeps_the_alpha() {
    for from in [Mode::Rgba, Mode::Indexed, Mode::GrayAlpha] {
        // Fondo blanco elegido con destino Gris y luego destino Indexado.
        let d = prepared(from, 0, false, false, 1, None);
        let plan = plan_from_values(&d.values().expect("válido"), from, None).expect("plan");
        assert_eq!(plan.allowed.drop_alpha, None, "{from:?}");
    }
}

#[test]
fn row_4_the_hexadecimals_of_the_spec_give_the_background_or_the_alpha_error() {
    for from in [Mode::Rgba, Mode::Indexed, Mode::GrayAlpha] {
        let gray = index_of_target(from, Mode::Gray);
        // (texto, RGB del fondo si vale)
        for (text_2, background_2) in [
            ("#ff0000", Some([255u8, 0, 0])),
            ("#fff", Some([255, 255, 255])),
            // `#ffff` son cuatro dígitos `rgba` con `a = f`: opaco.
            ("#ffff", Some([255, 255, 255])),
            ("#ff000080", None),
            ("#fff8", None),
        ] {
            let d = prepared(from, gray, true, true, 4, Some(text_2));
            match background_2 {
                Some(rgb) => {
                    assert_eq!(d.reason(), None, "{from:?}: {text_2}");
                    let plan = plan_from_values(&d.values().expect("válido"), from, None)
                        .expect("hay plan");
                    assert_eq!(plan.target, Mode::Gray);
                    assert_eq!(plan.allowed.drop_alpha, Some(rgb), "{from:?}: {text_2}");
                    assert_eq!(
                        plan.allowed.luma,
                        from != Mode::GrayAlpha,
                        "{from:?}: {text_2}"
                    );
                }
                None => {
                    assert_eq!(
                        d.reason(),
                        Some("Color: el fondo no lleva alfa".to_string()),
                        "{from:?}: {text_2}"
                    );
                    assert_eq!(d.values(), None, "{from:?}: {text_2}: no hay Plan");
                }
            }
        }
    }
}

#[test]
fn row_4_the_hexadecimal_with_hidden_alpha_does_not_get_in_the_way() {
    for from in [Mode::Rgba, Mode::Indexed, Mode::GrayAlpha] {
        // Con el destino Gris y el hexadecimal con alfa: error. Cambiando de destino, desaparece.
        let gray = index_of_target(from, Mode::Gray);
        let mut d = prepared(from, gray, false, false, 4, Some("#ff000080"));
        assert_eq!(
            d.reason(),
            Some("Color: el fondo no lleva alfa".to_string()),
            "{from:?}"
        );
        chooses(&mut d, "destino", 0);
        assert_eq!(d.reason(), None, "{from:?}: el hexadecimal está oculto");
        assert!(d.values().is_some());
    }
}

#[test]
fn row_4_an_unreadable_hexadecimal_invalidates_the_form_with_the_reason_of_the_field() {
    for text_2 in ["zz", "#12", "", "#gg0000"] {
        let d = prepared(Mode::Rgba, 2, true, true, 4, Some(text_2));
        let reason_2 = d.reason().expect("hay motivo");
        assert!(reason_2.starts_with("Color: "), "{text_2:?}: {reason_2}");
        assert_eq!(d.values(), None, "{text_2:?}");
    }
}

#[test]
fn row_4_the_active_color_without_a_value_or_with_alpha_gives_no_plan_and_the_opaque_one_does() {
    for from in [Mode::Rgba, Mode::Indexed, Mode::GrayAlpha] {
        let d = prepared(from, index_of_target(from, Mode::Gray), true, true, 3, None);
        let values = d.values().expect("el formulario es válido");
        for (active_2, expected_2) in [
            (Some(Rgba::opaque(1, 2, 3)), Some([1u8, 2, 3])),
            (Some(ACTIVE_WITH_ALPHA), None),
            (Some(Rgba::TRANSPARENT), None),
            (None, None),
        ] {
            let plan = plan_from_values(&values, from, active_2);
            match expected_2 {
                Some(rgb) => {
                    let plan = plan.expect("hay plan");
                    assert_eq!(plan.allowed.drop_alpha, Some(rgb), "{from:?}");
                }
                None => assert!(plan.is_none(), "{from:?}: activo {active_2:?}"),
            }
        }
    }
}

#[test]
fn row_4_without_visible_fields_that_are_missing_the_plan_is_none() {
    // Unos `Values` sin ningún campo (no hay destino): sin plan, para los cuatro orígenes.
    for from in TEST_MODES {
        assert_eq!(
            plan_from_values(&Values::default(), from, None),
            None,
            "{from:?}"
        );
        assert_eq!(
            plan_from_values(&Values::default(), from, Some(ACTIVE_OPAQUE)),
            None,
            "{from:?}"
        );
    }
}

#[test]
fn row_4_the_plan_target_is_targets_of_from_at_the_chosen_position_never_the_current() {
    for from in TEST_MODES {
        for i in 0..3 {
            let d = prepared(from, i, false, false, 0, None);
            let plan = plan_from_values(&d.values().expect("válido"), from, None).expect("plan");
            assert_eq!(plan.target, targets(from)[i], "{from:?}, {i}");
            assert_ne!(plan.target, from, "{from:?}, {i}");
        }
    }
}

#[test]
fn row_4_enter_in_the_dialog_delivers_the_values_with_the_hexadecimal_confirmed() {
    // `Accept` confirma el texto pendiente antes de entregar: el fondo sale en `Accepted`.
    let mut d = prepared(Mode::Rgba, 2, true, true, 4, Some("#102030"));
    match d.event(Event::Accept) {
        Reaction::Accepted(values) => {
            let plan = plan_from_values(&values, Mode::Rgba, None).expect("plan");
            assert_eq!(
                plan,
                Plan {
                    target: Mode::Gray,
                    allowed: Allowed {
                        luma: true,
                        drop_alpha: Some([0x10, 0x20, 0x30])
                    }
                }
            );
        }
        other => panic!("se esperaba Accepted y salió {other:?}"),
    }
}

#[test]
fn row_4_confirming_the_hexadecimal_of_a_valid_color_leaves_it_as_the_background() {
    let mut d = prepared(Mode::Indexed, 2, false, false, 4, Some("#336699"));
    confirms_hex(&mut d);
    assert!(!d.hex_pending("fondo_hex"));
    let plan = plan_from_values(&d.values().expect("válido"), Mode::Indexed, None).expect("plan");
    assert_eq!(plan.allowed.drop_alpha, Some([0x33, 0x66, 0x99]));
}

// ---------------------------------------------------------------------------------------
// Fila 5: texto del informe y aviso de historia.
// ---------------------------------------------------------------------------------------

/// El límite por defecto (64 MiB) y nada usado.
fn without_history() -> History {
    History {
        limit: 64 * MIB,
        used: 0,
    }
}

/// `report_text` con permisos y color activo por defecto (sin fondo) y sin aviso de historia.
fn report(analysis: Option<&Analysis>, size: (u32, u32)) -> String {
    let image = gray_empty(4, 4);
    report_text(
        analysis,
        size,
        &permissions(false, 0, TEST_RED),
        None,
        &image,
        without_history(),
    )
}

fn report_with(analysis: Option<&Analysis>, perms: &Permissions, active_2: Option<Rgba>) -> String {
    let image = gray_empty(4, 4);
    report_text(analysis, (4, 4), perms, active_2, &image, without_history())
}

fn contains(text_2: &str, searched: &str) -> bool {
    normalized(text_2).contains(searched)
}

#[test]
fn row_5_pending_says_computing_the_report_and_brings_no_history_notice() {
    let image = gray_empty(100, 100);
    let text_2 = report_text(
        None,
        (100, 100),
        &permissions(false, 0, TEST_RED),
        None,
        &image,
        history(0, 0),
    );
    assert!(
        normalized(&text_2).starts_with(REPORT_COMPUTING),
        "{text_2:?}"
    );
    assert!(!contains(&text_2, "historial"), "{text_2:?}");
    checks_report(&text_2);
}

#[test]
fn row_5_exact_without_a_palette_says_the_conversion_is_exact() {
    let text_2 = report(Some(&exact()), (4, 4));
    assert!(contains(&text_2, REPORT_EXACT), "{text_2:?}");
    assert!(!contains(&text_2, "Pierde"), "{text_2:?}");
    checks_report(&text_2);
}

#[test]
fn row_5_exact_to_indexed_says_the_entries_of_the_new_palette() {
    let without_transparent = report(Some(&exact_with_palette(3, false)), (4, 4));
    assert!(
        contains(
            &without_transparent,
            "Sin pérdidas. Paleta nueva de 3 colores."
        ),
        "{without_transparent:?}"
    );
    assert!(
        !contains(&without_transparent, "transparente"),
        "{without_transparent:?}"
    );
    let with_transparent = report(Some(&exact_with_palette(3, true)), (4, 4));
    assert!(
        contains(
            &with_transparent,
            "Sin pérdidas. Paleta nueva de 3 colores, el 0 transparente."
        ),
        "{with_transparent:?}"
    );
    let large = report(Some(&exact_with_palette(256, true)), (4, 4));
    assert!(
        contains(
            &large,
            "Sin pérdidas. Paleta nueva de 256 colores, el 0 transparente."
        ),
        "{large:?}"
    );
    for text_2 in [&without_transparent, &with_transparent, &large] {
        checks_report(text_2);
    }
}

#[test]
fn row_5_the_palette_of_one_color_goes_in_the_singular() {
    let text_2 = report(Some(&exact_with_palette(1, false)), (4, 4));
    assert!(contains(&text_2, "Paleta nueva de 1 color"), "{text_2:?}");
    assert!(!contains(&text_2, "1 colores"), "{text_2:?}");
    let text_2 = report(Some(&exact_with_palette(1, true)), (4, 4));
    assert!(!contains(&text_2, "1 colores"), "{text_2:?}");
}

#[test]
fn row_5_with_loss_it_says_only_the_losses_that_are_greater_than_zero() {
    let only_color = report(Some(&lost(3, 0)), (4, 4));
    assert!(
        contains(&only_color, "Pierde el color en 3 píxeles."),
        "{only_color:?}"
    );
    assert!(!contains(&only_color, "alfa"), "{only_color:?}");
    assert!(!contains(&only_color, "Sin pérdidas"), "{only_color:?}");

    let only_alpha = report(Some(&lost(0, 2)), (4, 4));
    assert!(
        contains(&only_alpha, "Pierde el alfa en 2 píxeles."),
        "{only_alpha:?}"
    );
    assert!(!contains(&only_alpha, "Pierde el color"), "{only_alpha:?}");

    let the_two = report(Some(&lost(3, 2)), (4, 4));
    let plane = normalized(&the_two);
    let color = plane.find("Pierde el color en 3 píxeles.").expect("color");
    let alpha_2 = plane.find("Pierde el alfa en 2 píxeles.").expect("alfa");
    assert!(color < alpha_2, "primero el color: {the_two:?}");

    for text_2 in [&only_color, &only_alpha, &the_two] {
        checks_report(text_2);
    }
}

#[test]
fn row_5_with_one_lost_pixel_it_goes_in_the_singular() {
    let text_2 = report(Some(&lost(1, 1)), (4, 4));
    assert!(
        contains(&text_2, "Pierde el color en 1 píxel"),
        "{text_2:?}"
    );
    assert!(contains(&text_2, "Pierde el alfa en 1 píxel"), "{text_2:?}");
    assert!(!contains(&text_2, "1 píxeles"), "{text_2:?}");
}

#[test]
fn row_5_the_too_many_colors_text_explains_the_quantizer_and_milestone_4_without_blaming() {
    let text_2 = report(Some(&Analysis::TooManyColors), (4, 4));
    assert!(contains(&text_2, REPORT_QUANTIZER), "{text_2:?}");
    assert!(contains(&text_2, "cuantizador"), "{text_2:?}");
    assert!(contains(&text_2, "hito 4"), "{text_2:?}");
    assert!(contains(&text_2, "más de 256 colores"), "{text_2:?}");
    checks_report(&text_2);
}

#[test]
fn row_5_too_large_has_its_first_line_and_respects_the_set_and_length() {
    let text_2 = report(Some(&Analysis::TooLarge), (4, 4));
    assert!(!text_2.trim().is_empty());
    for other_2 in [REPORT_EXACT, REPORT_COMPUTING, "Pierde el"] {
        assert!(!contains(&text_2, other_2), "{text_2:?}");
    }
    checks_report(&text_2);
}

#[test]
fn row_5_the_alpha_background_comes_out_only_with_an_opaque_background_chosen_and_alpha_in_play() {
    // Alfa en juego: Lossy con `alpha_lost > 0`.
    let with_alpha = lost(1, 1);
    let only_color = lost(2, 0);
    let yellow = Rgba::opaque(0xff, 0xcc, 0x00);
    for (name_2, perms, active_2, expected_2) in [
        (
            "blanco",
            permissions(true, 1, TEST_RED),
            None,
            Some("Fondo del alfa: #ffffff"),
        ),
        (
            "negro",
            permissions(true, 2, TEST_RED),
            None,
            Some("Fondo del alfa: #000000"),
        ),
        (
            "hexadecimal",
            permissions(true, 4, yellow),
            None,
            Some("Fondo del alfa: #ffcc00"),
        ),
        (
            "activo opaco",
            permissions(true, 3, TEST_RED),
            Some(Rgba::opaque(1, 2, 3)),
            Some("Fondo del alfa: #010203"),
        ),
        (
            "activo con alfa",
            permissions(true, 3, TEST_RED),
            Some(ACTIVE_WITH_ALPHA),
            None,
        ),
        (
            "activo sin color",
            permissions(true, 3, TEST_RED),
            None,
            None,
        ),
        (
            "no perder el alfa",
            permissions(true, 0, TEST_RED),
            Some(ACTIVE_OPAQUE),
            None,
        ),
    ] {
        let text_2 = report_with(Some(&with_alpha), &perms, active_2);
        match expected_2 {
            Some(line) => assert!(contains(&text_2, line), "{name_2}: {text_2:?}"),
            None => assert!(!contains(&text_2, "Fondo del alfa"), "{name_2}: {text_2:?}"),
        }
        checks_report(&text_2);
    }
    // Sin alfa en juego (solo color, exacta, pendiente): no sale aunque haya fondo elegido.
    for (name_2, analysis) in [
        ("solo color", Some(&only_color)),
        ("exacta", Some(&exact())),
        ("pendiente", None),
    ] {
        let text_2 = report_with(
            analysis,
            &permissions(true, 1, TEST_RED),
            Some(ACTIVE_OPAQUE),
        );
        assert!(!contains(&text_2, "Fondo del alfa"), "{name_2}: {text_2:?}");
    }
}

#[test]
fn row_5_the_order_is_state_background_size_and_history() {
    let image = gray_empty(8, 8);
    let perms = permissions(true, 2, TEST_RED);
    let text_2 = report_text(
        Some(&lost(2, 3)),
        (4097, 4096),
        &perms,
        None,
        &image,
        history(100, 0),
    );
    let plane = normalized(&text_2);
    let positions: Vec<usize> = [
        "Pierde el color en 2 píxeles.",
        "Pierde el alfa en 3 píxeles.",
        "Fondo del alfa: #000000",
        NOTICE_SIZE,
        NOTICE_HISTORY_ALL,
    ]
    .iter()
    .map(|t| {
        plane
            .find(t)
            .unwrap_or_else(|| panic!("falta {t:?} en {plane:?}"))
    })
    .collect();
    assert!(positions.windows(2).all(|w| w[0] < w[1]), "{positions:?}");
}

#[test]
fn row_5_the_size_notice_comes_out_only_with_more_than_2_to_the_24_pixels() {
    for (size, warns) in [
        ((2048u32, 2048u32), false),
        ((4096, 4096), false),
        ((4097, 4096), true),
        ((4096, 4097), true),
        ((8192, 8192), true),
        ((1, 1), false),
        ((16_777_216, 1), false),
        ((16_777_217, 1), true),
    ] {
        for analysis in [
            exact(),
            exact_with_palette(3, false),
            lost(2, 0),
            lost(0, 1),
        ] {
            let text_2 = report(Some(&analysis), size);
            assert_eq!(
                contains(&text_2, NOTICE_SIZE),
                warns,
                "{size:?}: {text_2:?}"
            );
            checks_report(&text_2);
        }
        // Los imposibles y el pendiente nunca lo traen por debajo del umbral.
        if !warns {
            for analysis in [
                Some(&Analysis::TooManyColors),
                Some(&Analysis::TooLarge),
                None,
            ] {
                let text_2 = report(analysis, size);
                assert!(!contains(&text_2, "puede tardar"), "{size:?}: {text_2:?}");
            }
        }
    }
    // El texto es el de la spec y cabe.
    assert_eq!(NOTICE_SIZE.chars().count(), 39);
}

#[test]
fn row_5_the_history_notice_follows_the_d7_rule_with_small_limits() {
    // Gray 100 × 100: 10 000 bytes de píxeles, un paso cuesta c = 12 048.
    let image = gray_empty(100, 100);
    assert_eq!(bytes_of_pixels(&image), 10_000);
    let c = 10_000 + HISTORY_MARGIN as usize;
    for (limit_2, used_2, expected_2) in [
        // c > límite: el primero (también con el límite 0).
        (0, 0, Some(NOTICE_HISTORY_ALL)),
        (c - 1, 0, Some(NOTICE_HISTORY_ALL)),
        (c - 1, 5_000, Some(NOTICE_HISTORY_ALL)),
        // c == límite: no es el primero; el segundo solo si hay historia que no cabe.
        (c, 0, None),
        (c, 1, Some(NOTICE_HISTORY_PART)),
        // límite/2 < c ≤ límite.
        (2 * c - 1, 0, None),
        (2 * c - 1, c, Some(NOTICE_HISTORY_PART)),
        (20_000, 0, None),
        (20_000, 7_952, None), // h + c == límite: cabe
        (20_000, 7_953, Some(NOTICE_HISTORY_PART)),
        (20_000, 20_000, Some(NOTICE_HISTORY_PART)),
        // c == límite/2: no pasa de la mitad (estrictamente): ninguno, con cualquier `h`.
        (2 * c, 0, None),
        (2 * c, 2 * c, None),
        (2 * c, 100_000, None),
        // c < límite/2.
        (64 * MIB, 0, None),
        (64 * MIB, 63 * MIB, None),
    ] {
        let text_2 = report_text(
            Some(&exact()),
            (100, 100),
            &permissions(false, 0, TEST_RED),
            None,
            &image,
            history(limit_2, used_2),
        );
        match expected_2 {
            Some(notice) => assert!(
                contains(&text_2, notice),
                "límite {limit_2}, usado {used_2}: falta {notice:?} en {text_2:?}"
            ),
            None => assert!(
                !contains(&text_2, "historial"),
                "límite {limit_2}, usado {used_2}: {text_2:?}"
            ),
        }
        // Y el que da `history_warning` es el mismo.
        assert_eq!(
            history_warning(&image, history(limit_2, used_2)).as_deref(),
            expected_2,
            "límite {limit_2}, usado {used_2}"
        );
        assert_eq!(
            expected_history_notice(&image, limit_2, used_2),
            expected_2,
            "el oráculo de la prueba"
        );
        checks_report(&text_2);
    }
}

#[test]
fn row_5_the_two_history_texts_are_those_of_d7_and_do_not_come_out_together() {
    let image = gray_empty(100, 100);
    let all = history_warning(&image, history(0, 0)).expect("primer aviso");
    assert_eq!(all, NOTICE_HISTORY_ALL);
    let part = history_warning(&image, history(20_000, 20_000)).expect("segundo aviso");
    assert_eq!(part, NOTICE_HISTORY_PART);
    for text_2 in [&all, &part] {
        assert!(in_the_set(text_2), "{text_2:?}");
    }
    assert_ne!(all, part);
    // Cada texto cabe en la zona de siete filas (con el resto del informe se mide en el peor caso).
    assert!(lines_of_zone(&all).len() <= 4);
    assert!(lines_of_zone(&part).len() <= 2);
}

#[test]
fn row_5_the_history_notice_only_comes_out_with_exact_or_lossy() {
    let image = gray_empty(100, 100);
    for (name_2, analysis, leaves) in [
        ("Exact", Some(exact()), true),
        ("Exact con paleta", Some(exact_with_palette(3, true)), true),
        ("Lossy", Some(lost(2, 0)), true),
        ("TooManyColors", Some(Analysis::TooManyColors), false),
        ("TooLarge", Some(Analysis::TooLarge), false),
        ("pendiente", None, false),
    ] {
        // Límite 0: el primer aviso saldría con cualquier informe posible.
        let text_2 = report_text(
            analysis.as_ref(),
            (100, 100),
            &permissions(true, 1, TEST_RED),
            Some(ACTIVE_OPAQUE),
            &image,
            history(0, 0),
        );
        assert_eq!(
            contains(&text_2, "historial"),
            leaves,
            "{name_2}: {text_2:?}"
        );
        checks_report(&text_2);
    }
}

#[test]
fn row_5_gray_of_4096_squared_with_the_default_limit_does_not_warn_with_any_history() {
    // 16 MiB de píxeles: c = 16 MiB + 2048 ≤ 32 MiB (la mitad del límite): ninguno, con cualquier `h`.
    let image = gray_empty(4096, 4096);
    for used_2 in [0, 32 * MIB, 63 * MIB, 64 * MIB] {
        assert_eq!(
            history_warning(&image, history(64 * MIB, used_2)),
            None,
            "h = {used_2}"
        );
    }
}

#[test]
#[ignore = "reserva 64 MiB y 34 MiB: cargo test -p px-app -- --ignored"]
fn row_5_rgba_of_4096_and_of_3000_with_the_default_limit_and_h_of_0_and_of_63_mib() {
    // RGBA 4096²: 64 MiB de píxeles > límite → el primer aviso, con cualquier `h`.
    let large = Image::new(4096, 4096, Mode::Rgba).expect("RGBA 4096²");
    for used_2 in [0, 63 * MIB] {
        assert_eq!(
            history_warning(&large, history(64 * MIB, used_2)).as_deref(),
            Some(NOTICE_HISTORY_ALL),
            "h = {used_2}"
        );
    }
    drop(large);
    // RGBA 3000²: 34,3 MiB (más de la mitad del límite y menos que el límite).
    let median = Image::new(3000, 3000, Mode::Rgba).expect("RGBA 3000²");
    assert_eq!(
        history_warning(&median, history(64 * MIB, 63 * MIB)).as_deref(),
        Some(NOTICE_HISTORY_PART)
    );
    assert_eq!(history_warning(&median, history(64 * MIB, 0)), None);
}

#[test]
fn row_5_with_limit_zero_the_notice_is_the_first_with_any_image() {
    for image in [
        gray_empty(1, 1),
        image_of_mode(Mode::Rgba),
        image_of_mode(Mode::Indexed),
    ] {
        for used_2 in [0, 1, 1000] {
            assert_eq!(
                history_warning(&image, history(0, used_2)).as_deref(),
                Some(NOTICE_HISTORY_ALL),
                "{:?}, h {used_2}",
                image.mode()
            );
        }
    }
}

#[test]
fn row_5_the_cost_of_a_step_counts_the_pixel_bytes_of_each_mode() {
    // 10 × 10 en cada modo: 100, 100, 200 y 400 bytes. Con límite = c − 1 avisa y con c no.
    for (mode, bytes) in [
        (Mode::Gray, 100usize),
        (Mode::GrayAlpha, 200),
        (Mode::Rgba, 400),
    ] {
        let image = Image::new(10, 10, mode).expect("imagen");
        let c = bytes + HISTORY_MARGIN as usize;
        assert_eq!(
            history_warning(&image, history(c - 1, 0)).as_deref(),
            Some(NOTICE_HISTORY_ALL),
            "{mode:?}"
        );
        assert_eq!(history_warning(&image, history(c, 0)), None, "{mode:?}");
    }
    let palette =
        Palette::from_colors([Rgba::opaque(1, 2, 3), Rgba::opaque(4, 5, 6)]).expect("paleta");
    let indexed = Image::new_indexed(10, 10, palette).expect("indexada");
    let c = 100 + HISTORY_MARGIN as usize;
    assert_eq!(
        history_warning(&indexed, history(c - 1, 0)).as_deref(),
        Some(NOTICE_HISTORY_ALL)
    );
    assert_eq!(history_warning(&indexed, history(c, 0)), None);
}

#[test]
fn row_5_the_worst_case_fits_in_seven_lines_of_42_characters_and_in_the_set() {
    // RGBA a Gris con color, alfa, fondo, tamaño e historia (primera forma).
    let image = gray_empty(8, 8);
    for perms in [
        permissions(true, 1, TEST_RED),
        permissions(true, 2, TEST_RED),
        permissions(true, 4, TEST_RED),
    ] {
        let text_2 = report_text(
            Some(&lost(123_456_789, 987_654_321)),
            (8192, 8192),
            &perms,
            None,
            &image,
            history(0, 0),
        );
        checks_report(&text_2);
        let plane = normalized(&text_2);
        for part in [
            "Pierde el color en 123456789 píxeles.",
            "Pierde el alfa en 987654321 píxeles.",
            "Fondo del alfa: #",
            NOTICE_SIZE,
            NOTICE_HISTORY_ALL,
        ] {
            assert!(plane.contains(part), "falta {part:?} en {plane:?}");
        }
        assert!(lines_of_zone(&text_2).len() <= 7);
    }
    // El mismo caso con el segundo aviso de historia.
    let text_2 = report_text(
        Some(&lost(2, 3)),
        (4097, 4096),
        &permissions(true, 1, TEST_RED),
        None,
        &gray_empty(100, 100),
        history(20_000, 20_000),
    );
    checks_report(&text_2);
    assert!(contains(&text_2, NOTICE_HISTORY_PART));
}

#[test]
fn row_5_every_possible_report_respects_the_set_and_seven_lines() {
    let image = gray_empty(100, 100);
    let mut checked = 0;
    for (_, analysis) in analysis_of_row_3() {
        for pending_2 in [false, true] {
            for size in [(4u32, 4u32), (4097, 4096), (8192, 8192)] {
                for (limit_2, used_2) in [(0, 0), (20_000, 20_000), (64 * MIB, 0)] {
                    for alpha_2 in 0..5 {
                        for active_2 in active_2_2() {
                            let text_2 = report_text(
                                if pending_2 { None } else { Some(&analysis) },
                                size,
                                &permissions(true, alpha_2, TEST_RED),
                                active_2,
                                &image,
                                history(limit_2, used_2),
                            );
                            checks_report(&text_2);
                            checked += 1;
                        }
                    }
                }
            }
        }
    }
    assert!(checked > 1000);
}

#[test]
fn row_5_describe_converted_says_the_mode_and_only_the_losses_greater_than_zero() {
    for (target, mode_2) in [
        (Mode::Indexed, "indexado"),
        (Mode::Rgba, "RGBA"),
        (Mode::GrayAlpha, "gris con alfa"),
        (Mode::Gray, "gris"),
    ] {
        let without = format!("imagen convertida a {mode_2}");
        assert_eq!(describe_converted(target, None), without);
        assert_eq!(describe_converted(target, Some(&exact())), without);
        assert_eq!(
            describe_converted(target, Some(&exact_with_palette(3, false))),
            without
        );
        assert_eq!(
            describe_converted(target, Some(&lost(3, 0))),
            format!("{without} · perdido: color 3 px")
        );
        assert_eq!(
            describe_converted(target, Some(&lost(0, 2))),
            format!("{without} · perdido: alfa 2 px")
        );
        assert_eq!(
            describe_converted(target, Some(&lost(3, 2))),
            format!("{without} · perdido: color 3 px, alfa 2 px")
        );
    }
    // El de la spec.
    assert_eq!(
        describe_converted(Mode::GrayAlpha, Some(&lost(3, 0))),
        "imagen convertida a gris con alfa · perdido: color 3 px"
    );
    assert!(in_the_set(&describe_converted(
        Mode::GrayAlpha,
        Some(&lost(3, 2))
    )));
}

// ---------------------------------------------------------------------------------------
// Fila 6: cuándo se calcula (`sync_convert` con un `Analyzer` que cuenta).
// ---------------------------------------------------------------------------------------

/// El informe que da el analizador falso, según el destino (sin tocar píxeles).
fn analysis_false(target: Mode) -> Analysis {
    match target {
        Mode::Gray => lost(2, 1),
        Mode::GrayAlpha => lost(2, 0),
        Mode::Indexed => exact_with_palette(3, false),
        Mode::Rgba => exact(),
    }
}

/// Un `Analyzer` falso: anota cada llamada (ancho, alto y modo de la imagen, y el destino) y da
/// `analysis_false`.
#[derive(Default)]
struct Counter {
    calls: Vec<(u32, u32, Mode, Mode)>,
}

impl Analyzer for Counter {
    fn analyze(&mut self, image: &Image, target: Mode) -> Analysis {
        self.calls
            .push((image.width(), image.height(), image.mode(), target));
        analysis_false(target)
    }
}

/// Un diálogo abierto con `open_convert_mode`, su `ConvertSync` y un analizador que cuenta.
struct Setup {
    modal: Modal,
    sync: ConvertSync,
    counter: Counter,
    image: Image,
    state_id: u64,
    limit: usize,
    used: usize,
}

impl Setup {
    fn new_2(from: Mode, image: Image) -> Setup {
        let mut modal = Modal::default();
        assert!(open_convert_mode(&mut modal, from, Locks::default()));
        Setup {
            modal,
            sync: ConvertSync::new(from, 0, Some(ACTIVE_OPAQUE)),
            counter: Counter::default(),
            image,
            state_id: 0,
            limit: 64 * MIB,
            used: 0,
        }
    }

    fn dialog(&self) -> &Dialog {
        match &self.modal {
            Modal::Open(d) => d,
            Modal::Closed => panic!("el diálogo está cerrado"),
        }
    }

    fn dialog_mut(&mut self) -> &mut Dialog {
        match &mut self.modal {
            Modal::Open(d) => d,
            Modal::Closed => panic!("el diálogo está cerrado"),
        }
    }

    /// Un fotograma de `sync_convert`: `true` si hay algo que repintar.
    fn synchronizes(&mut self) -> bool {
        let Setup {
            modal,
            sync,
            counter,
            image,
            state_id,
            limit,
            used,
            ..
        } = self;
        let Modal::Open(dialog) = modal else {
            panic!("el diálogo está cerrado");
        };
        sync_convert(
            dialog,
            sync,
            image,
            *state_id,
            History {
                limit: *limit,
                used: *used,
            },
            counter,
        )
    }

    fn calls(&self) -> usize {
        self.counter.calls.len()
    }

    fn target_of_the_last_call(&self) -> Mode {
        self.counter.calls.last().expect("hubo una llamada").3
    }

    fn report(&self) -> String {
        report_of(self.dialog())
    }

    /// Cambia el destino (índice) con un evento del diálogo.
    fn chooses_target(&mut self, index: usize) {
        chooses(self.dialog_mut(), "destino", index);
    }

    /// `true` si el diálogo está en espera de un cálculo (motivo de «calculando»).
    fn pending_2(&self) -> bool {
        self.dialog().block() == Some(PENDING)
    }

    /// `Accept` sobre una copia: `Rejected` mientras haya motivo.
    fn accept_rejected(&self) -> bool {
        matches!(
            self.dialog().clone().event(Event::Accept),
            Reaction::Rejected(_)
        )
    }
}

#[test]
fn row_6_before_the_first_frame_there_is_no_current_report() {
    let sync = ConvertSync::new(Mode::Rgba, 0, None);
    assert!(sync.analysis().is_none());
}

#[test]
fn row_6_when_opening_with_a_small_image_there_is_one_call_with_the_first_target() {
    for from in TEST_MODES {
        let image = image_of_mode(from);
        let (w, h) = (image.width(), image.height());
        let mut m = Setup::new_2(from, image);
        assert!(m.pending_2(), "{from:?}: al abrir, calculando");
        assert_eq!(m.calls(), 0, "{from:?}: abrir no calcula");
        assert!(m.synchronizes(), "{from:?}: el primer fotograma repinta");
        assert_eq!(m.calls(), 1, "{from:?}");
        assert_eq!(
            m.counter.calls[0],
            (w, h, from, targets(from)[0]),
            "{from:?}: la imagen y el destino por defecto"
        );
        assert_eq!(
            m.sync.analysis(),
            Some(&analysis_false(targets(from)[0])),
            "{from:?}"
        );
        assert!(!m.pending_2(), "{from:?}");
        // Sin más eventos no hay nada que hacer.
        for _ in 0..3 {
            assert!(!m.synchronizes(), "{from:?}: sin cambios no repinta");
        }
        assert_eq!(m.calls(), 1, "{from:?}: ni se vuelve a calcular");
    }
}

#[test]
fn row_6_changing_target_makes_one_call_with_the_chosen_target() {
    for from in TEST_MODES {
        let mut m = Setup::new_2(from, image_of_mode(from));
        m.synchronizes();
        let mut expected_2_2 = 1;
        for index in [1, 2, 0, 2, 1, 0] {
            m.chooses_target(index);
            assert!(m.synchronizes(), "{from:?}: el cambio repinta");
            expected_2_2 += 1;
            assert_eq!(m.calls(), expected_2_2, "{from:?}, destino {index}");
            assert_eq!(
                m.target_of_the_last_call(),
                targets(from)[index],
                "{from:?}"
            );
            assert_eq!(
                m.sync.analysis(),
                Some(&analysis_false(targets(from)[index])),
                "{from:?}, destino {index}"
            );
            // Sin más eventos, nada.
            assert!(!m.synchronizes());
            assert_eq!(m.calls(), expected_2_2);
        }
    }
}

#[test]
fn row_6_choosing_the_same_target_does_not_call() {
    let mut m = Setup::new_2(Mode::Rgba, image_of_mode(Mode::Rgba));
    m.synchronizes();
    m.chooses_target(0);
    assert!(!m.synchronizes());
    assert_eq!(m.calls(), 1);
}

#[test]
fn row_6_going_back_to_the_previous_target_is_another_change_and_another_call() {
    let mut m = Setup::new_2(Mode::Rgba, image_of_mode(Mode::Rgba));
    m.synchronizes();
    for (index, expected_2_2) in [(1, 2), (0, 3), (1, 4)] {
        m.chooses_target(index);
        m.synchronizes();
        assert_eq!(m.calls(), expected_2_2, "destino {index}");
    }
}

#[test]
fn row_6_changing_boxes_background_or_hexadecimal_does_not_call_the_analyzer() {
    let mut m = Setup::new_2(Mode::Rgba, image_of_mode(Mode::Rgba));
    m.synchronizes();

    // Destino «Gris con alfa» (perdida 2/0): la casilla cubre el color.
    m.chooses_target(1);
    m.synchronizes();
    let base = m.calls();
    assert_eq!(m.dialog().block(), Some(MISSING_COLOR));
    assert!(contains(&m.report(), "Pierde el color en 2 píxeles."));
    mark(m.dialog_mut(), "luma_gris_alfa", true);
    assert!(m.synchronizes(), "la casilla cambia el motivo");
    assert_eq!(m.calls(), base, "marcar la casilla no calcula");
    assert_eq!(m.dialog().block(), None);
    assert!(m.dialog().is_valid());
    mark(m.dialog_mut(), "luma_gris_alfa", false);
    assert!(m.synchronizes());
    assert_eq!(m.calls(), base, "desmarcarla tampoco");
    assert_eq!(m.dialog().block(), Some(MISSING_COLOR));

    // Destino «Gris» (perdida 2/1): color y fondo.
    m.chooses_target(2);
    m.synchronizes();
    let base = m.calls();
    assert_eq!(base, 3);
    assert_eq!(m.dialog().block(), Some(MISSING_BOTH));
    mark(m.dialog_mut(), "luma_gris", true);
    assert!(m.synchronizes());
    assert_eq!(m.dialog().block(), Some(CHOOSES_BACKGROUND));
    for (option, background_2) in [
        (1, "Fondo del alfa: #ffffff"),
        (2, "Fondo del alfa: #000000"),
        (3, "Fondo del alfa: #123456"),
        (4, "Fondo del alfa: #808080"),
    ] {
        chooses(m.dialog_mut(), "alfa_gris", option);
        assert!(m.synchronizes(), "opción {option}");
        assert_eq!(m.calls(), base, "opción {option}: no calcula");
        assert_eq!(m.dialog().block(), None, "opción {option}");
        assert!(
            contains(&m.report(), background_2),
            "opción {option}: {:?}",
            m.report()
        );
    }
    // Un hexadecimal confirmado, válido y con alfa, y uno ilegible.
    writes_hex(m.dialog_mut(), "#ff0000");
    confirms_hex(m.dialog_mut());
    m.synchronizes();
    assert_eq!(m.calls(), base);
    assert!(
        contains(&m.report(), "Fondo del alfa: #ff0000"),
        "{:?}",
        m.report()
    );
    writes_hex(m.dialog_mut(), "zz");
    m.synchronizes();
    assert_eq!(m.calls(), base);
    assert!(!m.dialog().is_valid());
    writes_hex(m.dialog_mut(), "#ff000080");
    confirms_hex(m.dialog_mut());
    m.synchronizes();
    assert_eq!(m.calls(), base, "ni con el alfa que el formulario rechaza");
    assert_eq!(
        m.dialog().reason(),
        Some("Color: el fondo no lleva alfa".to_string())
    );
    assert!(!contains(&m.report(), "Fondo del alfa"), "{:?}", m.report());
    // «No perder el alfa» vuelve a pedir el fondo, sin llamar.
    chooses(m.dialog_mut(), "alfa_gris", 0);
    m.synchronizes();
    assert_eq!(m.calls(), base);
    assert_eq!(m.dialog().block(), Some(CHOOSES_BACKGROUND));
}

#[test]
fn row_6_with_2_to_the_22_pixels_it_is_computed_in_the_same_frame() {
    let mut m = Setup::new_2(Mode::Gray, gray_empty(2048, 2048));
    assert_eq!(
        2048u64 * 2048,
        SYNC_PIXELS,
        "la imagen de la prueba mide justo SYNC_PIXELS"
    );
    assert!(m.synchronizes());
    assert_eq!(m.calls(), 1, "hasta 2²² píxeles: inmediato");
    assert!(!m.pending_2());
    assert!(m.sync.analysis().is_some());
}

#[test]
fn row_6_above_2_to_the_22_the_call_arrives_wait_frames_frames_later() {
    let mut m = Setup::new_2(Mode::Gray, gray_empty(2048, 2049));
    const { assert!(2048u64 * 2049 > SYNC_PIXELS) };
    let mut frames = 0usize;
    // Del fotograma del cambio (el primero) al anterior al cálculo: «Calculando».
    for frame in 0..WAIT_FRAMES {
        m.synchronizes();
        frames += 1;
        assert_eq!(m.calls(), 0, "fotograma {frame}: aún sin llamar");
        assert!(m.pending_2(), "fotograma {frame}");
        assert_eq!(m.dialog().reason(), Some(PENDING.to_string()));
        assert!(
            !m.dialog().is_valid(),
            "fotograma {frame}: Aceptar atenuado"
        );
        assert!(m.accept_rejected(), "fotograma {frame}: Accept da Rejected");
        assert!(
            contains(&m.report(), REPORT_COMPUTING),
            "fotograma {frame}: {:?}",
            m.report()
        );
    }
    // Exactamente `WAIT_FRAMES` fotogramas después del primero: la llamada.
    let repaints = m.synchronizes();
    frames += 1;
    assert_eq!(frames, usize::from(WAIT_FRAMES) + 1);
    assert!(repaints, "el informe ya está: hay que repintar");
    assert_eq!(
        m.calls(),
        1,
        "la llamada llega WAIT_FRAMES fotogramas después"
    );
    assert_eq!(m.target_of_the_last_call(), Mode::Indexed);
    assert!(!m.pending_2());
    assert_eq!(m.dialog().block(), None);
    assert!(m.dialog().is_valid());
    assert!(
        contains(&m.report(), "Sin pérdidas. Paleta nueva de 3 colores."),
        "{:?}",
        m.report()
    );
    // Y después no se repite ni repinta.
    for _ in 0..3 {
        assert!(!m.synchronizes());
        assert_eq!(m.calls(), 1);
    }
}

#[test]
fn row_6_with_2048_by_2049_changing_target_waits_again() {
    let mut m = Setup::new_2(Mode::Gray, gray_empty(2048, 2049));
    for _ in 0..=WAIT_FRAMES {
        m.synchronizes();
    }
    assert_eq!(m.calls(), 1);
    m.chooses_target(1);
    assert!(m.synchronizes(), "el cambio repinta con el aviso");
    assert_eq!(m.calls(), 1, "el fotograma del cambio no calcula");
    assert!(m.pending_2());
    assert!(contains(&m.report(), REPORT_COMPUTING));
    assert!(m.accept_rejected());
    for _ in 1..WAIT_FRAMES {
        m.synchronizes();
        assert_eq!(m.calls(), 1);
        assert!(m.pending_2());
    }
    assert!(m.synchronizes());
    assert_eq!(m.calls(), 2);
    assert_eq!(m.target_of_the_last_call(), Mode::Rgba);
    assert!(!m.pending_2());
}

#[test]
fn row_6_changing_target_twice_during_the_wait_restarts_it_without_calling() {
    let mut m = Setup::new_2(Mode::Gray, gray_empty(2048, 2049));
    for _ in 0..=WAIT_FRAMES {
        m.synchronizes();
    }
    assert_eq!(m.calls(), 1);

    // Primer cambio y espera hasta justo antes del fotograma del cálculo.
    m.chooses_target(1);
    m.synchronizes();
    for _ in 1..WAIT_FRAMES {
        m.synchronizes();
        assert_eq!(m.calls(), 1);
    }
    // En el fotograma en que habría llegado la llamada, el usuario cambia otra vez: se reinicia.
    m.chooses_target(2);
    m.synchronizes();
    assert_eq!(m.calls(), 1, "el cambio reinicia la espera sin calcular");
    assert!(m.pending_2(), "sigue calculando");
    for _ in 1..WAIT_FRAMES {
        m.synchronizes();
        assert_eq!(m.calls(), 1, "la espera nueva aún no ha terminado");
        assert!(m.pending_2());
    }
    m.synchronizes();
    assert_eq!(
        m.calls(),
        2,
        "WAIT_FRAMES fotogramas después del último cambio"
    );
    // Nunca se calculó el destino intermedio.
    let targets_computed: Vec<Mode> = m.counter.calls.iter().map(|l| l.3).collect();
    assert_eq!(targets_computed, [Mode::Indexed, Mode::GrayAlpha]);
    assert!(!m.pending_2());
}

#[test]
fn row_6_a_different_state_id_discards_the_report_and_recomputes() {
    // Imagen pequeña: se recalcula en el mismo fotograma.
    let mut m = Setup::new_2(Mode::Rgba, image_of_mode(Mode::Rgba));
    m.synchronizes();
    assert_eq!(m.calls(), 1);
    m.state_id = 7;
    // El informe sale igual: puede que no haya nada que repintar (no se asegura el valor).
    m.synchronizes();
    assert_eq!(m.calls(), 2);
    assert_eq!(m.target_of_the_last_call(), Mode::Indexed);
    assert!(!m.synchronizes());
    assert_eq!(m.calls(), 2, "con el mismo state_id no vuelve a calcular");

    // Imagen grande: pasa por la espera de siempre.
    let mut m = Setup::new_2(Mode::Gray, gray_empty(2048, 2049));
    for _ in 0..=WAIT_FRAMES {
        m.synchronizes();
    }
    assert_eq!(m.calls(), 1);
    m.state_id = 7;
    m.synchronizes();
    assert_eq!(m.calls(), 1);
    assert!(m.pending_2(), "el informe vigente se descartó");
    for _ in 1..WAIT_FRAMES {
        m.synchronizes();
        assert_eq!(m.calls(), 1);
    }
    m.synchronizes();
    assert_eq!(m.calls(), 2);
    assert!(!m.pending_2());
}

#[test]
fn row_6_sync_convert_returns_true_only_if_there_is_something_to_repaint() {
    let mut m = Setup::new_2(Mode::Rgba, image_of_mode(Mode::Rgba));
    assert!(m.synchronizes(), "primer fotograma");
    for _ in 0..3 {
        assert!(!m.synchronizes(), "reposo");
    }
    m.chooses_target(1);
    assert!(m.synchronizes(), "otro destino");
    assert!(!m.synchronizes());
    mark(m.dialog_mut(), "luma_gris_alfa", true);
    assert!(m.synchronizes(), "la casilla cambia el motivo");
    assert!(!m.synchronizes());
    // La misma casilla marcada otra vez: sin cambios.
    mark(m.dialog_mut(), "luma_gris_alfa", true);
    assert!(!m.synchronizes(), "nada cambia");
    mark(m.dialog_mut(), "luma_gris_alfa", false);
    assert!(m.synchronizes(), "desmarcar vuelve a pedir el permiso");
    assert!(!m.synchronizes());
}

#[test]
fn row_6_the_analyzer_receives_the_image_and_never_a_modified_copy() {
    let image = image_of_mode(Mode::Indexed);
    let copy = image.clone();
    let mut m = Setup::new_2(Mode::Indexed, image);
    m.synchronizes();
    m.chooses_target(2);
    m.synchronizes();
    assert_eq!(m.image, copy, "el diálogo no modifica la imagen");
    for call in &m.counter.calls {
        assert_eq!((call.0, call.1, call.2), (5, 1, Mode::Indexed));
    }
}

// ---------------------------------------------------------------------------------------
// Fila 9: aceptar en dos tiempos (funciones puras).
// ---------------------------------------------------------------------------------------

fn plan_of_test() -> Plan {
    Plan {
        target: Mode::Gray,
        allowed: Allowed {
            luma: true,
            drop_alpha: Some([1, 2, 3]),
        },
    }
}

#[test]
fn row_9_up_to_2_to_the_22_pixels_it_is_converted_in_the_same_frame() {
    let plan = plan_of_test();
    for pixels in [1u64, 100, SYNC_PIXELS - 1, SYNC_PIXELS, 2048 * 2048] {
        assert_eq!(start_convert(plan, pixels), Start::Now(plan), "{pixels}");
    }
}

#[test]
fn row_9_above_2_to_the_22_it_is_later_and_tick_gives_the_plan_exactly_at_the_wait_frames_th() {
    let plan = plan_of_test();
    for pixels in [
        SYNC_PIXELS + 1,
        2048 * 2048 + 1,
        2048 * 2049,
        4096 * 4096,
        4097 * 4096,
        8192 * 8192,
        u64::MAX,
    ] {
        let Start::Later(mut pending_2) = start_convert(plan, pixels) else {
            panic!("{pixels}: debía esperar");
        };
        assert_eq!(pending_2.plan(), plan, "{pixels}");
        let mut deliveries = 0;
        for tick in 1..=usize::from(WAIT_FRAMES) + 6 {
            let result = pending_2.tick();
            if tick == usize::from(WAIT_FRAMES) {
                assert_eq!(result, Some(plan), "{pixels}: tick {tick}");
                deliveries += 1;
            } else {
                assert_eq!(result, None, "{pixels}: tick {tick}");
            }
        }
        assert_eq!(deliveries, 1, "{pixels}: el plan se entrega una sola vez");
    }
}

#[test]
fn row_9_the_plan_that_comes_out_is_the_one_that_went_in_with_its_allowed_included() {
    for allowed in [
        Allowed::NOTHING,
        Allowed {
            luma: true,
            drop_alpha: None,
        },
        Allowed {
            luma: false,
            drop_alpha: Some([255, 255, 255]),
        },
        Allowed {
            luma: true,
            drop_alpha: Some([0, 0, 0]),
        },
    ] {
        for target in TEST_MODES {
            let plan = Plan { target, allowed };
            let Start::Later(mut pending_2) = start_convert(plan, 8192 * 8192) else {
                panic!("debía esperar");
            };
            let mut delivered = None;
            for _ in 0..WAIT_FRAMES {
                delivered = pending_2.tick().or(delivered);
            }
            assert_eq!(delivered, Some(plan));
        }
    }
}

#[test]
fn row_9_after_the_wait_frames_th_tick_it_does_not_repeat() {
    let plan = plan_of_test();
    let Start::Later(mut pending_2) = start_convert(plan, 4096 * 4096) else {
        panic!("debía esperar");
    };
    for _ in 0..WAIT_FRAMES {
        pending_2.tick();
    }
    for _ in 0..20 {
        assert_eq!(pending_2.tick(), None);
    }
}

#[test]
fn row_9_start_convert_is_deterministic_and_pending_convert_is_cloned_with_its_count() {
    let plan = plan_of_test();
    assert_eq!(
        start_convert(plan, 8192 * 8192),
        start_convert(plan, 8192 * 8192)
    );
    let Start::Later(original) = start_convert(plan, 8192 * 8192) else {
        panic!("debía esperar");
    };
    let mut copy = original.clone();
    assert_eq!(copy, original);
    copy.tick();
    assert_ne!(
        copy, original,
        "la copia lleva su propia cuenta de fotogramas"
    );
    let mut a = original.clone();
    let mut b = original;
    for _ in 0..WAIT_FRAMES {
        assert_eq!(a.tick(), b.tick());
    }
}

#[test]
fn row_9_the_notice_is_converting_and_is_in_the_set() {
    assert_eq!(CONVERTING_NOTICE, "Convirtiendo…");
    assert!(in_the_set(CONVERTING_NOTICE));
    assert!(CONVERTING_NOTICE.chars().count() <= 44);
}

#[test]
fn row_9_the_accept_threshold_is_the_same_as_the_report_one() {
    // 2048² + 1 px: el informe espera y el aceptar espera; 2048²: los dos son inmediatos.
    let plan = plan_of_test();
    assert!(matches!(start_convert(plan, SYNC_PIXELS), Start::Now(_)));
    assert!(matches!(
        start_convert(plan, SYNC_PIXELS + 1),
        Start::Later(_)
    ));
    let mut immediate = Setup::new_2(Mode::Gray, gray_empty(2048, 2048));
    immediate.synchronizes();
    assert_eq!(immediate.calls(), 1);
    let mut with_waits = Setup::new_2(Mode::Gray, gray_empty(2048, 2049));
    with_waits.synchronizes();
    assert_eq!(with_waits.calls(), 0);
    // Y la espera es de `WAIT_FRAMES` en los dos.
    let Start::Later(mut pending_2) = start_convert(plan, SYNC_PIXELS + 1) else {
        panic!("debía esperar");
    };
    let mut ticks = 0;
    while pending_2.tick().is_none() {
        ticks += 1;
        assert!(ticks < 100);
    }
    let mut frames = 0;
    while with_waits.calls() == 0 {
        with_waits.synchronizes();
        frames += 1;
        assert!(frames < 100);
    }
    assert_eq!(ticks + 1, usize::from(WAIT_FRAMES));
    assert_eq!(frames, usize::from(WAIT_FRAMES));
}

// ---------------------------------------------------------------------------------------
// Propiedad 1: el informe y `Accept` dicen la verdad.
// ---------------------------------------------------------------------------------------

/// Los valores de canal con los que se generan los colores: fuerzan grises, alfa 0, alfa 255 y
/// repeticiones.
const VALUES: [u8; 5] = [0, 1, 127, 128, 255];

/// Todo lo que hace falta para construir una imagen de hasta 6 × 6 en cualquier modo.
#[derive(Clone, Debug)]
struct Seed {
    mode_2: usize,
    width: u32,
    height: u32,
    /// 36 píxeles, cada uno con cuatro índices en `VALUES`.
    pixels_2: Vec<[usize; 4]>,
    /// 6 colores de paleta, cada uno con cuatro índices en `VALUES`.
    colors: Vec<[usize; 4]>,
    /// Entradas de la paleta (1 a 6).
    n: usize,
    /// Índice transparente: `6` es «ninguno».
    transparent: usize,
}

fn strategy_of_seed() -> impl Strategy<Value = Seed> {
    (
        0usize..4,
        1u32..=6,
        1u32..=6,
        prop::collection::vec(prop::array::uniform4(0usize..5), 36),
        prop::collection::vec(prop::array::uniform4(0usize..5), 6),
        1usize..=6,
        0usize..=6,
    )
        .prop_map(
            |(mode_2, width, height, pixels_2, colors, n, transparent)| Seed {
                mode_2,
                width,
                height,
                pixels_2,
                colors,
                n,
                transparent,
            },
        )
}

fn rgba_of_indices(v: &[usize; 4]) -> Rgba {
    Rgba::new(VALUES[v[0]], VALUES[v[1]], VALUES[v[2]], VALUES[v[3]])
}

fn image_of_seed(s: &Seed) -> Image {
    let (w, h) = (s.width, s.height);
    let total = (w * h) as usize;
    let mode = TEST_MODES[s.mode_2];
    let mut image = match mode {
        Mode::Indexed => {
            let colors: Vec<Rgba> = s.colors.iter().take(s.n).map(rgba_of_indices).collect();
            let mut palette = Palette::from_colors(colors).expect("paleta");
            if s.transparent < s.n {
                palette
                    .set_transparent_index(Some(s.transparent as u8))
                    .expect("índice transparente");
            }
            Image::new_indexed(w, h, palette).expect("imagen indexada")
        }
        other_2 => Image::new(w, h, other_2).expect("imagen"),
    };
    for (i, v) in s.pixels_2.iter().take(total).enumerate() {
        let (x, y) = (i as u32 % w, i as u32 / w);
        let pixel = match mode {
            Mode::Rgba => Pixel::Rgba(rgba_of_indices(v)),
            Mode::Gray => Pixel::Gray(VALUES[v[0]]),
            Mode::GrayAlpha => Pixel::GrayAlpha {
                gray: VALUES[v[0]],
                alpha: VALUES[v[3]],
            },
            Mode::Indexed => Pixel::Indexed((v[0] % s.n) as u8),
        };
        image.set_pixel(x, y, pixel).expect("píxel");
    }
    image
}

const HEXES_OPAQUE: [Rgba; 4] = [
    Rgba::opaque(255, 0, 0),
    Rgba::opaque(0, 0, 0),
    Rgba::opaque(255, 255, 255),
    Rgba::opaque(10, 20, 30),
];

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Para imágenes de hasta 6 × 6 en cualquier modo, cualquier destino distinto del origen y
    /// cualquier `Allowed` (por los permisos): `analyze` coincide con `convert_image(.., NOTHING)`
    /// y `block_reason` es `None` si y solo si `Command::ConvertMode` con ese `Allowed` da `Ok`,
    /// salvo los fondos no opacos, que no producen `Allowed`.
    #[test]
    fn property_1_the_report_and_accept_tell_the_truth(
        seed in strategy_of_seed(),
        target_2 in 0usize..3,
        luma in any::<bool>(),
        alpha_2 in 0usize..5,
        hex in 0usize..4,
        active_2 in 0usize..4,
    ) {
        let image = image_of_seed(&seed);
        let from = image.mode();
        let target = targets_2(from)[target_2];

        // `analyze` es `convert_image(.., NOTHING)` clasificada.
        let analysis = analyze(&image, target);
        let direct = convert_image(&image, target, Allowed::NOTHING);
        match (&analysis, &direct) {
            (Analysis::Exact { palette }, Ok(conversion)) => {
                let expected = if target == Mode::Indexed {
                    let p = conversion.image.palette();
                    prop_assert!(p.is_some(), "una conversión a indexado lleva paleta");
                    p.map(|p| (p.len(), p.transparent_index().is_some()))
                } else {
                    None
                };
                prop_assert_eq!(
                    palette.as_ref().map(|p| (p.colors, p.transparent)),
                    expected
                );
            }
            (Analysis::Lossy(r), Err(ConvertError::LossNotAllowed { report })) => {
                prop_assert_eq!(r, report);
            }
            (Analysis::TooManyColors, Err(ConvertError::TooManyColors { .. })) => {}
            (Analysis::TooLarge, Err(ConvertError::TooLarge { .. })) => {}
            (analysis_2, direct) => {
                return Err(TestCaseError::fail(format!(
                    "analyze {analysis_2:?} no coincide con convert_image {:?}",
                    direct.as_ref().map(|_| "Ok").map_err(|e| e.clone())
                )));
            }
        }

        // `Accept` activo ⇔ `apply` acepta.
        let active_2 = active_2_2()[active_2];
        let perms = permissions(luma, alpha_2, HEXES_OPAQUE[hex]);
        let Some(allowed) = allowed_of(&perms, active_2) else {
            return Ok(());
        };
        let reason_2 = block_reason(Some(&analysis), &perms, active_2);
        let mut document = Document::new(image.clone());
        let id = ImageId::from_raw(0);
        let result = document.apply(Command::ConvertMode { image: id, target, allowed });
        prop_assert_eq!(
            reason_2.is_none(),
            result.is_ok(),
            "{:?} a {:?} con {:?}: motivo {:?}, apply {:?}",
            from, target, allowed, reason_2, result
        );
        if let Some(text_2) = reason_2 {
            // El pie entero mide 44 columnas: 37 tras `error: ` y 44 con `falta: ` en su lugar.
            let limit_2 = if text_2.starts_with("falta: ") { 44 } else { 37 };
            prop_assert!(text_2.chars().count() <= limit_2, "{}", text_2);
            prop_assert!(in_the_set(&text_2), "{}", text_2);
        }
    }
}

// ---------------------------------------------------------------------------------------
// Propiedad 2: el estado del formulario no se corrompe.
// ---------------------------------------------------------------------------------------

const TEXTS_HEX: [&str; 10] = [
    "#ff0000",
    "#ff000080",
    "#fff",
    "#ffff",
    "#fff8",
    "zz",
    "#12",
    "",
    "#80808080",
    "#00ff00",
];

#[derive(Clone, Debug)]
enum TestGesture {
    Target(usize),
    /// 0: la de «Gris con alfa»; 1: la de «Gris».
    TestBox(usize, bool),
    Alpha(usize),
    Hex(usize),
    Confirms,
    Reverts,
}

fn gesture() -> impl Strategy<Value = TestGesture> {
    prop_oneof![
        (0usize..3).prop_map(TestGesture::Target),
        (0usize..2, any::<bool>()).prop_map(|(k, on)| TestGesture::TestBox(k, on)),
        (0usize..5).prop_map(TestGesture::Alpha),
        (0usize..TEXTS_HEX.len()).prop_map(TestGesture::Hex),
        Just(TestGesture::Confirms),
        Just(TestGesture::Reverts),
    ]
}

/// Lo que debería tener el formulario tras los gestos, escrito aparte del motor.
#[derive(Clone, Debug)]
struct Model {
    from: Mode,
    target_2: usize,
    luma: [bool; 2],
    alpha_2: usize,
    confirmed: Rgba,
    pending_2: Option<String>,
}

impl Model {
    fn new_2(from: Mode) -> Model {
        Model {
            from,
            target_2: 0,
            luma: [false; 2],
            alpha_2: 0,
            confirmed: Rgba::new(0x80, 0x80, 0x80, 0xff),
            pending_2: None,
        }
    }

    fn target(&self) -> Mode {
        targets_2(self.from)[self.target_2]
    }

    fn group_visible(&self) -> bool {
        self.target() == Mode::Gray && self.from != Mode::Gray
    }

    fn hex_visible(&self) -> bool {
        self.group_visible() && self.alpha_2 == 4
    }

    /// El color del hexadecimal con su texto pendiente: `Err` si el texto no vale.
    fn effective(&self) -> Result<Rgba, ()> {
        match &self.pending_2 {
            Some(text_2) => parse_hex(text_2, Mode::Rgba, self.confirmed).map_err(|_| ()),
            None => Ok(self.confirmed),
        }
    }

    fn luma_effective(&self) -> bool {
        match (self.from, self.target()) {
            (Mode::Rgba | Mode::Indexed, Mode::GrayAlpha) => self.luma[0],
            (Mode::Rgba | Mode::Indexed, Mode::Gray) => self.luma[1],
            _ => false,
        }
    }

    fn invalid_of_field(&self) -> bool {
        self.hex_visible() && self.effective().is_err()
    }

    fn fails_the_check(&self) -> bool {
        self.hex_visible() && matches!(self.effective(), Ok(c) if c.a < 255)
    }

    fn permissions(&self) -> Permissions {
        let alpha = if !self.group_visible() {
            AlphaChoice::Keep
        } else if self.alpha_2 == 4 {
            AlphaChoice::Hex(self.effective().unwrap_or(self.confirmed))
        } else {
            alpha_of(self.alpha_2, self.confirmed)
        };
        Permissions {
            luma: self.luma_effective(),
            alpha,
        }
    }

    fn plan(&self, active_2: Option<Rgba>) -> Option<Plan> {
        let hex_opaque = self.effective().ok().filter(|c| c.a == 255);
        let allowed = allowed_expected(
            self.from,
            self.target_2,
            self.luma[0],
            self.luma[1],
            self.alpha_2,
            hex_opaque,
            active_2,
        )?;
        Some(Plan {
            target: self.target(),
            allowed,
        })
    }
}

/// Aplica un gesto al diálogo y al modelo, solo sobre campos visibles (como el usuario).
fn applies(d: &mut Dialog, m: &mut Model, g: &TestGesture) {
    match g {
        TestGesture::Target(i) => {
            d.event(Event::Choice {
                field: "destino",
                index: *i,
            });
            m.target_2 = *i;
        }
        TestGesture::TestBox(k, on) => {
            let id = ["luma_gris_alfa", "luma_gris"][*k];
            if d.is_shown(id) {
                d.event(Event::Check { field: id, on: *on });
                m.luma[*k] = *on;
            }
        }
        TestGesture::Alpha(i) => {
            if d.is_shown("alfa_gris") {
                d.event(Event::Choice {
                    field: "alfa_gris",
                    index: *i,
                });
                m.alpha_2 = *i;
            }
        }
        TestGesture::Hex(i) => {
            if d.is_shown("fondo_hex") {
                let text = TEXTS_HEX[*i].to_string();
                d.event(Event::Hex {
                    field: "fondo_hex",
                    text: text.clone(),
                });
                m.pending_2 = Some(text);
            }
        }
        TestGesture::Confirms => {
            if d.is_shown("fondo_hex") {
                d.event(Event::HexConfirm {
                    field: "fondo_hex",
                    blur: false,
                });
                if let Some(text_2) = m.pending_2.clone()
                    && let Ok(color) = parse_hex(&text_2, Mode::Rgba, m.confirmed)
                {
                    m.confirmed = color;
                    m.pending_2 = None;
                }
            }
        }
        TestGesture::Reverts => {
            if d.is_shown("fondo_hex") {
                d.event(Event::HexRevert { field: "fondo_hex" });
                m.pending_2 = None;
            }
        }
    }
}

/// Las invariantes de la propiedad 2 tras un gesto y su `sync_convert`.
fn checks_form(
    d: &Dialog,
    m: &Model,
    sync: &ConvertSync,
    active_2: Option<Rgba>,
    ctx: &str,
) -> Result<(), TestCaseError> {
    // Solo son visibles los campos de D2 para este destino (y la opción del alfa).
    let expected_2_2_2 = ids_visible(m.from, m.target_2, m.alpha_2);
    for id in ALL_THE_FIELDS {
        let must = has_field(d, id) && expected_2_2_2.contains(&id);
        prop_assert_eq!(d.is_shown(id), must, "{}: {}", ctx, id);
    }

    // `values()` es `Some` si y solo si no hay campo inválido, ni `check`, ni motivo externo.
    let valid = !m.invalid_of_field() && !m.fails_the_check() && d.block().is_none();
    let values_2 = d.values();
    prop_assert_eq!(values_2.is_some(), valid, "{}: values", ctx);
    prop_assert_eq!(d.reason().is_none(), valid, "{}: reason", ctx);
    prop_assert_eq!(d.is_valid(), valid, "{}: is_valid", ctx);

    if let Some(values) = &values_2 {
        // Los campos ocultos no salen en `Values`.
        for id in ALL_THE_FIELDS {
            if !expected_2_2_2.contains(&id) {
                prop_assert!(
                    values.get(id).is_none(),
                    "{}: {} sale en los valores",
                    ctx,
                    id
                );
            }
        }
        prop_assert_eq!(values.choice("destino"), Some(m.target_2), "{}", ctx);
        // El `Allowed` no se contamina con una casilla de otro destino.
        let plan = plan_from_values(values, m.from, active_2);
        prop_assert_eq!(plan, m.plan(active_2), "{}: plan", ctx);
        if let Some(plan) = plan {
            prop_assert!(
                plan.target != m.from,
                "{}: el destino es el modo actual",
                ctx
            );
            prop_assert_eq!(plan.target, targets(m.from)[m.target_2], "{}", ctx);
        }
    }

    // El motivo externo es el de `block_reason` con los permisos visibles (salvo el hexadecimal que
    // el propio formulario rechaza).
    if !(m.hex_visible() && (m.invalid_of_field() || m.fails_the_check())) {
        prop_assert_eq!(
            d.block().map(str::to_string),
            block_reason(sync.analysis(), &m.permissions(), active_2),
            "{}: motivo",
            ctx
        );
    }

    // El informe cabe y está en el juego.
    let text_2 = report_of(d);
    let lines = lines_of_zone(&text_2);
    prop_assert!(
        !lines.is_empty() && lines.len() <= 7,
        "{}: {:?}",
        ctx,
        text_2
    );
    for line in &lines {
        prop_assert!(line.chars().count() <= 42, "{}: {:?}", ctx, line);
    }
    prop_assert!(
        in_the_set(&text_2.replace('\n', " ")),
        "{}: {:?}",
        ctx,
        text_2
    );
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Para los cuatro orígenes y secuencias de 0 a 48 eventos (destinos, casillas, grupo del alfa,
    /// hexadecimales válidos e inválidos) con `sync_convert` tras cada una: sin pánico, solo son
    /// visibles los campos de D2, `values()` es `Some` si y solo si no hay campo inválido, ni
    /// `check`, ni motivo externo, los campos ocultos no salen en `Values`, el destino del plan nunca
    /// es el modo actual y el informe cabe.
    #[test]
    fn property_2_the_form_state_is_not_corrupted(
        gestures in prop::collection::vec(gesture(), 0..=48),
        active_2 in 0usize..4,
    ) {
        let active_2 = active_2_2()[active_2];
        for from in TEST_MODES {
            let image = image_of_mode(from);
            let mut modal = Modal::default();
            prop_assert!(open_convert_mode(&mut modal, from, Locks::default()));
            let Modal::Open(d) = &mut modal else {
                return Err(TestCaseError::fail("cerrado"));
            };
            let mut sync = ConvertSync::new(from, 0, active_2);
            let mut analyzer = SystemAnalyzer;
            let mut m = Model::new_2(from);
            let history_2 = History { limit: 64 * MIB, used: 0 };
            sync_convert(d, &mut sync, &image, 0, history_2, &mut analyzer);
            checks_form(d, &m, &sync, active_2, &format!("{from:?}, al abrir"))?;

            for (i, g) in gestures.iter().enumerate() {
                let ctx = format!("{from:?}, paso {i}: {g:?}");
                applies(d, &mut m, g);
                let history_2 = History { limit: 64 * MIB, used: 0 };
                sync_convert(d, &mut sync, &image, 0, history_2, &mut analyzer);
                checks_form(d, &m, &sync, active_2, &ctx)?;
            }
        }
    }
}

// ---------------------------------------------------------------------------------------
// Propiedad 4: calcular solo al cambiar de destino.
// ---------------------------------------------------------------------------------------

/// Una imagen por encima de `SYNC_PIXELS` (2048 × 2049) de cada modo, construida una sola vez.
fn image_large(mode: Mode) -> &'static Image {
    static GRAY: OnceLock<Image> = OnceLock::new();
    static GRAY_ALPHA: OnceLock<Image> = OnceLock::new();
    static RGBA: OnceLock<Image> = OnceLock::new();
    static INDEXED: OnceLock<Image> = OnceLock::new();
    match mode {
        Mode::Gray => GRAY.get_or_init(|| gray_empty(2048, 2049)),
        Mode::GrayAlpha => {
            GRAY_ALPHA.get_or_init(|| Image::new(2048, 2049, Mode::GrayAlpha).expect("imagen"))
        }
        Mode::Rgba => RGBA.get_or_init(|| Image::new(2048, 2049, Mode::Rgba).expect("imagen")),
        Mode::Indexed => INDEXED.get_or_init(|| {
            let palette = Palette::from_colors([Rgba::opaque(1, 2, 3), Rgba::opaque(4, 5, 6)])
                .expect("paleta");
            Image::new_indexed(2048, 2049, palette).expect("imagen")
        }),
    }
}

#[derive(Clone, Debug)]
enum TestStep {
    TestEvent(TestGesture),
    Frame,
}

fn step() -> impl Strategy<Value = TestStep> {
    prop_oneof![
        3 => gesture().prop_map(TestStep::TestEvent),
        2 => Just(TestStep::Frame),
    ]
}

/// Repite una secuencia de eventos y fotogramas sobre una imagen y comprueba la propiedad 4.
fn reproduces(
    from: Mode,
    image: &Image,
    steps_2: &[TestStep],
    active_2: Option<Rgba>,
) -> Result<(), TestCaseError> {
    let large = u64::from(image.width()) * u64::from(image.height()) > SYNC_PIXELS;
    let mut modal = Modal::default();
    prop_assert!(open_convert_mode(&mut modal, from, Locks::default()));
    let Modal::Open(d) = &mut modal else {
        return Err(TestCaseError::fail("cerrado"));
    };
    let mut sync = ConvertSync::new(from, 0, active_2);
    let mut counter = Counter::default();
    let mut m = Model::new_2(from);

    // Estado del modelo de fotogramas: el destino visto en el último `sync_convert`, cuántos
    // cambios se han visto y en qué fotograma fue el último (la apertura cuenta como fotograma 0).
    let mut observed: Option<usize> = None;
    let mut changes_2 = 0usize;
    let mut start_2 = 0usize;
    let mut frame = 0usize;

    let mut synchronizes = |d: &mut Dialog,
                            sync: &mut ConvertSync,
                            counter: &mut Counter,
                            m: &Model,
                            ctx: &str|
     -> Result<(), TestCaseError> {
        if observed != Some(m.target_2) {
            if observed.is_some() {
                changes_2 += 1;
            }
            observed = Some(m.target_2);
            start_2 = frame;
        }
        let history_2 = History {
            limit: 64 * MIB,
            used: 0,
        };
        sync_convert(d, sync, image, 0, history_2, counter);
        let pending_2 = large && frame < start_2 + usize::from(WAIT_FRAMES);
        frame += 1;
        prop_assert!(
            counter.calls.len() <= 1 + changes_2,
            "{}: {} llamadas con {} cambios de destino",
            ctx,
            counter.calls.len(),
            changes_2
        );
        if pending_2 {
            prop_assert_eq!(d.block(), Some(PENDING), "{}: pendiente", ctx);
            prop_assert!(
                !d.is_valid(),
                "{}: Aceptar está bloqueado mientras se calcula",
                ctx
            );
            prop_assert!(
                matches!(d.clone().event(Event::Accept), Reaction::Rejected(_)),
                "{}: Accept con un cálculo pendiente",
                ctx
            );
        } else {
            prop_assert!(
                d.block() != Some(PENDING),
                "{}: no hay nada pendiente y el motivo lo dice",
                ctx
            );
            prop_assert!(sync.analysis().is_some(), "{}: hay informe vigente", ctx);
        }
        Ok(())
    };

    synchronizes(d, &mut sync, &mut counter, &m, "apertura")?;
    for (i, step) in steps_2.iter().enumerate() {
        match step {
            TestStep::TestEvent(g) => applies(d, &mut m, g),
            TestStep::Frame => {
                synchronizes(
                    d,
                    &mut sync,
                    &mut counter,
                    &m,
                    &format!("paso {i}: {step:?}"),
                )?;
            }
        }
    }
    // Tras `WAIT_FRAMES` fotogramas ociosos no queda ningún cálculo pendiente. Si la secuencia acabó
    // en un evento, el primer fotograma ocioso es el que ve el cambio: hacen falta `WAIT_FRAMES + 1`.
    for k in 0..=WAIT_FRAMES {
        synchronizes(d, &mut sync, &mut counter, &m, &format!("ocioso {k}"))?;
    }
    prop_assert!(d.block() != Some(PENDING), "queda un cálculo pendiente");
    prop_assert!(sync.analysis().is_some());
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Para secuencias aleatorias de eventos y fotogramas sobre imágenes de cualquier tamaño (por
    /// debajo y por encima de `SYNC_PIXELS`) y de los cuatro modos: las llamadas a
    /// `Analyzer::analyze` son como mucho las de cambios de destino más una por apertura (nunca
    /// crecen con casillas, fondo, hexadecimal ni fotogramas ociosos); tras `WAIT_FRAMES` fotogramas
    /// ociosos no queda ningún cálculo pendiente; y mientras haya uno pendiente `Accept` está
    /// bloqueado.
    #[test]
    fn property_4_computing_only_when_changing_target(
        steps_2 in prop::collection::vec(step(), 0..=60),
        active_2 in 0usize..4,
    ) {
        let active_2 = active_2_2()[active_2];
        for from in TEST_MODES {
            reproduces(from, &image_of_mode(from), &steps_2, active_2)?;
            reproduces(from, image_large(from), &steps_2, active_2)?;
        }
    }

    /// `PendingConvert` entrega su plan una sola vez, siempre en el `WAIT_FRAMES`-ésimo `tick`, con
    /// cualquier plan y cualquier número de píxeles por encima de `SYNC_PIXELS`; por debajo (o
    /// igual) `start_convert` da `Now` con el mismo plan.
    #[test]
    fn property_4_pending_convert_delivers_its_plan_only_once(
        target_2 in 0usize..4,
        luma in any::<bool>(),
        background_2 in prop::option::of(any::<[u8; 3]>()),
        pixels_2 in prop_oneof![1u64..=SYNC_PIXELS, (SYNC_PIXELS + 1)..=(1u64 << 40)],
        ticks in 0usize..=12,
    ) {
        let plan = Plan {
            target: TEST_MODES[target_2],
            allowed: Allowed { luma, drop_alpha: background_2 },
        };
        match start_convert(plan, pixels_2) {
            Start::Now(delivered) => {
                prop_assert!(pixels_2 <= SYNC_PIXELS, "{} píxeles dan Now", pixels_2);
                prop_assert_eq!(delivered, plan);
            }
            Start::Later(mut pending_2) => {
                prop_assert!(pixels_2 > SYNC_PIXELS, "{} píxeles dan Later", pixels_2);
                prop_assert_eq!(pending_2.plan(), plan);
                let mut deliveries = Vec::new();
                for tick in 1..=ticks {
                    if let Some(p) = pending_2.tick() {
                        deliveries.push((tick, p));
                    }
                }
                if ticks >= usize::from(WAIT_FRAMES) {
                    prop_assert_eq!(deliveries, vec![(usize::from(WAIT_FRAMES), plan)]);
                } else {
                    prop_assert!(deliveries.is_empty());
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------------------
// Fila 12 (medición, ignorada): informe, `apply` y refresco a 4096² y 8192².
// ---------------------------------------------------------------------------------------

/// RGBA de `side_2 × side_2` con 16 grises opacos (exacta a gris, a RGBA y a indexado) y, en una
/// esquina, un píxel de color y otro semitransparente (para la conversión con pérdida).
fn image_of_measurement(side_2: u32) -> Image {
    let mut image = Image::new(side_2, side_2, Mode::Rgba).expect("imagen de medición");
    for y in 0..side_2 {
        for x in 0..side_2 {
            let g = (((x ^ y) & 15) * 17) as u8;
            image
                .set_pixel(x, y, Pixel::Rgba(Rgba::opaque(g, g, g)))
                .expect("píxel");
        }
    }
    image
}

#[test]
#[ignore = "medición: cargo test --release -p px-app -- --ignored --nocapture"]
fn row_12_measurement_of_report_apply_and_refresh_at_4096_and_8192() {
    for side_2 in [4096u32, 8192] {
        let t = Instant::now();
        let mut image = image_of_measurement(side_2);
        println!(
            "{side_2}²: imagen de prueba construida en {:?}",
            t.elapsed()
        );

        // Informes: exacta a indexado (la peor), exacta a gris, y con pérdida.
        let t = Instant::now();
        let a_indexed = analyze(&image, Mode::Indexed);
        let report_indexed = t.elapsed();
        assert_eq!(a_indexed, exact_with_palette(16, false));

        let t = Instant::now();
        let a_gray = analyze(&image, Mode::Gray);
        let report_gray = t.elapsed();
        assert_eq!(a_gray, exact());

        image
            .set_pixel(0, 0, Pixel::Rgba(Rgba::opaque(255, 0, 0)))
            .expect("píxel");
        let t = Instant::now();
        let with_lost = analyze(&image, Mode::Gray);
        let report_with_lost = t.elapsed();
        assert_eq!(with_lost, lost(1, 0));

        println!(
            "{side_2}²: informe a indexado {report_indexed:?}, a gris exacta {report_gray:?}, con pérdida {report_with_lost:?}"
        );
        // Red de seguridad, no un objetivo: el informe de un 4096² no pasa de 2 s.
        if side_2 == 4096 {
            for (name_2, t) in [
                ("a indexado", report_indexed),
                ("a gris", report_gray),
                ("con pérdida", report_with_lost),
            ] {
                assert!(
                    t.as_secs_f64() <= 2.0,
                    "el informe {name_2} de un 4096² tardó {t:?}"
                );
            }
        }

        // `apply` y refresco de la textura.
        let mut document = Document::new(image);
        let id = ImageId::from_raw(0);
        let t = Instant::now();
        let applied = document
            .apply(Command::ConvertMode {
                image: id,
                target: Mode::GrayAlpha,
                allowed: Allowed {
                    luma: true,
                    drop_alpha: None,
                },
            })
            .expect("convertir");
        let apply = t.elapsed();
        assert!(applied);
        let t = Instant::now();
        let rgba8 = px_ui::texture::to_rgba8(document.image(id).expect("imagen"));
        let refresh = t.elapsed();
        assert_eq!(rgba8.len(), 4 * side_2 as usize * side_2 as usize);
        drop(rgba8);
        let t = Instant::now();
        document.undo().expect("deshacer");
        let undo = t.elapsed();
        println!("{side_2}²: apply {apply:?}, refresco (to_rgba8) {refresh:?}, undo {undo:?}");
    }
}

// ---------------------------------------------------------------------------------------
// Unidad 20b, parte 1 (specs/hito-2-motivo-y-marcadores.md): el motivo del pie lista todo lo que
// falta. Filas 1 a 5 y propiedades 1 a 3. En ROJO hasta que existan `Missing`, `missing_text`,
// `pending`, `footer_text`, `Dialog::problems`, `Dialog::set_summary` y `Dialog::summary`.
//
// **Suposiciones y ambigüedades** (la spec no fija un detalle; se elige lo más literal):
// - `Missing` es `Clone + Copy + Debug + PartialEq + Eq + PartialOrd + Ord` (la spec dice «con Ord»).
// - `missing_text` solo recibe listas **ordenadas** (color, fondo, valor) y sin repetidos, como las
//   que da `pending`; no se prueba qué hace con una lista desordenada.
// - `missing_text(&[Value])` (un valor inválido solo) no tiene texto fijado en D2 (en el diálogo el
//   pie pinta el motivo del campo con `error: `), pero la propiedad 1 exige que toda lista no vacía
//   empiece por `falta: ` o `faltan N: `. Solo se exige `falta: ` + el rótulo `background_2 válido`.
// - `faltan 3: color, background_2, value` es la forma con cuenta de la lista: como `falta: `, va sin
//   `error: ` delante (`footer_text` lo deja tal cual: aclaración del visto bueno). La fila 8 no lo
//   pinta (en el diálogo no es alcanzable).
// - `pending` con un análisis imposible (`TooManyColors`, `TooLarge`) da una lista vacía **también**
//   con `value_problem = true`: «imposibles → no entran».
// - En `pending`, `Value` entra si `value_problem` o si el fondo elegido es «color activo» y el
//   activo no existe o tiene alfa (así `block_reason`, que pasa `false`, da `background_2 válido`).
// - Propiedad 3: la frase literal «`summary` es `Some` ⇔ `!is_valid`» contradice D3 (con un solo
//   pendiente el pie sale de `set_block`, sin resumen, y `Accept` está atenuado). Se prueba que
//   hay resumen **exactamente** cuando hay ≥ 2 pendientes y `problems()` no está vacío; que un
//   resumen implica `!is_valid`, `reason()` igual al resumen y `Accept` rechazado; que un diálogo
//   inválido sin resumen tiene su causa en `block()` o en `problems()`; y que `values()` es
//   `Some` ⇔ no hay resumen, ni `block()`, ni `problems()`.
// ---------------------------------------------------------------------------------------

const MISSING_BACKGROUND_AND_VALUE: &str = "falta: fondo del alfa; fondo válido";
const MISSING_THREE: &str = "faltan 3: color, fondo, valor";

/// Las 8 listas ordenadas y sin repetidos de `{Luma, AlphaBackground, Value}`, la vacía primero.
fn lists_of_missing() -> Vec<Vec<Missing>> {
    let all_2 = [Missing::Luma, Missing::AlphaBackground, Missing::Value];
    (0..8u8)
        .map(|mask| {
            all_2
                .iter()
                .enumerate()
                .filter(|(i, _)| mask & (1 << i) != 0)
                .map(|(_, m)| *m)
                .collect()
        })
        .collect()
}

/// El rótulo corto de cada pendiente, tal como sale en la lista de dos.
fn label_short(m: Missing) -> &'static str {
    match m {
        Missing::Luma => "perder el color",
        Missing::AlphaBackground => "fondo del alfa",
        Missing::Value => "fondo válido",
    }
}

/// Cuántos requisitos dice un motivo: 0 sin motivo, N en `faltan N: `, los separados por `; ` tras
/// `falta: ` y 1 en un valor inválido o un imposible.
fn requirements_of_the_text(text_2: Option<&str>) -> usize {
    match text_2 {
        None => 0,
        Some(t) if t.starts_with("falta: ") => t.split(';').count(),
        Some(t) if t.starts_with("faltan ") => t["faltan ".len()..]
            .split(':')
            .next()
            .and_then(|n| n.parse().ok())
            .expect("faltan N: …"),
        Some(_) => 1,
    }
}

fn is_impossible(analysis: &Analysis) -> bool {
    matches!(analysis, Analysis::TooManyColors | Analysis::TooLarge)
}

fn active_bad(active: Option<Rgba>) -> bool {
    active.is_none_or(|c| c.a < 255)
}

// --- Fila 1: un pendiente ---

#[test]
fn row_1_20b_missing_text_of_one_is_the_long_label_with_missing_and_without_error() {
    assert_eq!(
        missing_text(&[Missing::Luma]),
        Some("falta: permitir perder el color".to_string())
    );
    assert_eq!(
        missing_text(&[Missing::AlphaBackground]),
        Some("falta: elegir el fondo del alfa".to_string())
    );
    assert_eq!(MISSING_COLOR, "falta: permitir perder el color");
    assert_eq!(CHOOSES_BACKGROUND, "falta: elegir el fondo del alfa");
    for text_2 in [MISSING_COLOR, CHOOSES_BACKGROUND] {
        assert_eq!(text_2.chars().count(), 31);
        assert!(!text_2.contains("error"), "{text_2:?}");
    }
}

#[test]
fn row_1_20b_block_reason_of_color_only_and_background_only_gives_the_literals() {
    for active_2 in active_2_2() {
        // Solo color: el alfa no se pierde (o se cubre con un fondo opaco).
        assert_eq!(
            block_reason(
                Some(&lost(2, 0)),
                &permissions(false, 1, TEST_RED),
                active_2
            ),
            Some(MISSING_COLOR.to_string())
        );
        assert_eq!(
            block_reason(
                Some(&lost(2, 3)),
                &permissions(false, 2, TEST_RED),
                active_2
            ),
            Some(MISSING_COLOR.to_string())
        );
        // Solo fondo: la casilla está marcada.
        assert_eq!(
            block_reason(
                Some(&lost(0, 3)),
                &permissions(false, 0, TEST_RED),
                active_2
            ),
            Some(CHOOSES_BACKGROUND.to_string())
        );
        assert_eq!(
            block_reason(Some(&lost(2, 3)), &permissions(true, 0, TEST_RED), active_2),
            Some(CHOOSES_BACKGROUND.to_string())
        );
    }
}

#[test]
fn row_1_20b_an_invalid_value_alone_keeps_todays_text_without_missing() {
    let analysis_2 = lost(0, 1);
    assert_eq!(
        block_reason(Some(&analysis_2), &permissions(false, 3, TEST_RED), None),
        Some("no hay color activo".to_string())
    );
    assert_eq!(
        block_reason(
            Some(&analysis_2),
            &permissions(false, 3, TEST_RED),
            Some(ACTIVE_WITH_ALPHA)
        ),
        Some("el color activo #ff000080 tiene alfa".to_string())
    );
    assert_eq!(
        block_reason(
            Some(&analysis_2),
            &permissions(true, 3, TEST_RED),
            Some(ACTIVE_TRANSPARENT)
        ),
        Some("el color activo #00000000 tiene alfa".to_string())
    );
    // Ninguno lleva `falta: ` (el pie les pone `error: `).
    for active_2 in [None, Some(ACTIVE_WITH_ALPHA), Some(ACTIVE_TRANSPARENT)] {
        let reason_2 = block_reason(Some(&exact()), &permissions(true, 3, TEST_RED), active_2)
            .expect("hay motivo");
        assert!(!reason_2.starts_with("falta"), "{reason_2:?}");
        assert!(reason_2.chars().count() <= 37, "{reason_2:?}");
    }
}

#[test]
fn row_1_20b_the_unreadable_hexadecimal_field_with_the_box_checked_is_only_the_value() {
    for from in [Mode::Rgba, Mode::Indexed] {
        let mut m = Setup::new_2(from, image_of_mode(from));
        m.synchronizes();
        m.chooses_target(2);
        m.synchronizes();
        mark(m.dialog_mut(), "luma_gris", true);
        chooses(m.dialog_mut(), "alfa_gris", 4);
        writes_hex(m.dialog_mut(), "zz");
        m.synchronizes();
        let reason_2 = m.dialog().reason().expect("hay motivo");
        assert!(reason_2.starts_with("Color: "), "{from:?}: {reason_2:?}");
        assert_eq!(m.dialog().summary(), None, "{from:?}: un solo pendiente");
        assert_eq!(m.dialog().block(), None, "{from:?}");
        assert_eq!(m.dialog().problems(), vec![reason_2], "{from:?}");
    }
}

// --- Fila 2: dos pendientes ---

#[test]
fn row_2_20b_color_and_background_give_the_list_of_two_and_each_thing_goes_back_to_the_one_of_one()
{
    for (name_2, analysis_2) in [("Lossy 2/3", lost(2, 3)), ("Lossy 1/1", lost(1, 1))] {
        let active_2 = Some(ACTIVE_OPAQUE);
        assert_eq!(
            block_reason(
                Some(&analysis_2),
                &permissions(false, 0, TEST_RED),
                active_2
            ),
            Some(MISSING_BOTH.to_string()),
            "{name_2}"
        );
        // Marcar el permiso quita el color.
        assert_eq!(
            block_reason(Some(&analysis_2), &permissions(true, 0, TEST_RED), active_2),
            Some(CHOOSES_BACKGROUND.to_string()),
            "{name_2}"
        );
        // Elegir un fondo opaco quita el fondo.
        for alpha_2 in 1..5 {
            assert_eq!(
                block_reason(
                    Some(&analysis_2),
                    &permissions(false, alpha_2, TEST_RED),
                    active_2
                ),
                Some(MISSING_COLOR.to_string()),
                "{name_2}, alfa {}",
                NAMES_OF_ALPHA[alpha_2]
            );
        }
        // Las dos cosas: nada.
        assert_eq!(
            block_reason(Some(&analysis_2), &permissions(true, 1, TEST_RED), active_2),
            None,
            "{name_2}"
        );
    }
    assert_eq!(MISSING_BOTH, "falta: perder el color; fondo del alfa");
    assert_eq!(MISSING_BOTH.chars().count(), 38);
}

#[test]
fn row_2_20b_color_and_an_invalid_active_background_give_color_and_valid_background() {
    for analysis_2 in [lost(2, 0), lost(2, 1), lost(1, 3)] {
        for active_2 in [None, Some(ACTIVE_WITH_ALPHA), Some(ACTIVE_TRANSPARENT)] {
            assert_eq!(
                block_reason(
                    Some(&analysis_2),
                    &permissions(false, 3, TEST_RED),
                    active_2
                ),
                Some(MISSING_COLOR_AND_VALUE.to_string()),
                "{analysis_2:?}, activo {active_2:?}"
            );
            // Con el permiso, vuelve al valor solo (el texto de hoy, sin `falta: `).
            let only = block_reason(Some(&analysis_2), &permissions(true, 3, TEST_RED), active_2)
                .expect("sigue el valor inválido");
            assert!(!only.starts_with("falta"), "{only:?}");
        }
        // Con un activo opaco, solo el color.
        assert_eq!(
            block_reason(
                Some(&analysis_2),
                &permissions(false, 3, TEST_RED),
                Some(ACTIVE_OPAQUE)
            ),
            Some(MISSING_COLOR.to_string())
        );
    }
}

#[test]
fn row_2_20b_in_the_rgba_and_indexed_to_gray_dialog_with_nothing_it_says_color_and_background() {
    for from in [Mode::Rgba, Mode::Indexed] {
        let mut m = Setup::new_2(from, image_of_mode(from));
        m.synchronizes();
        m.chooses_target(2);
        m.synchronizes();
        assert_eq!(m.sync.analysis(), Some(&lost(2, 1)), "{from:?}");
        assert_eq!(m.dialog().block(), Some(MISSING_BOTH), "{from:?}");
        assert_eq!(
            m.dialog().reason(),
            Some(MISSING_BOTH.to_string()),
            "{from:?}"
        );
        // Sin campo inválido, no hace falta resumen.
        assert_eq!(m.dialog().summary(), None, "{from:?}");
        assert!(m.accept_rejected());

        // Un fondo elegido vuelve al de uno: el color.
        chooses(m.dialog_mut(), "alfa_gris", 1);
        m.synchronizes();
        assert_eq!(
            m.dialog().reason(),
            Some(MISSING_COLOR.to_string()),
            "{from:?}"
        );
        // La casilla marcada y sin fondo: el fondo.
        chooses(m.dialog_mut(), "alfa_gris", 0);
        mark(m.dialog_mut(), "luma_gris", true);
        m.synchronizes();
        assert_eq!(
            m.dialog().reason(),
            Some(CHOOSES_BACKGROUND.to_string()),
            "{from:?}"
        );
        chooses(m.dialog_mut(), "alfa_gris", 2);
        m.synchronizes();
        assert_eq!(m.dialog().reason(), None, "{from:?}");
        assert!(!m.accept_rejected());
    }
}

#[test]
fn row_2_20b_invalid_hexadecimal_with_the_box_unchecked_is_color_and_valid_background() {
    for from in [Mode::Rgba, Mode::Indexed] {
        for text_2 in ["zz", "#ff000080"] {
            let ctx = format!("{from:?}, {text_2:?}");
            let mut m = Setup::new_2(from, image_of_mode(from));
            m.synchronizes();
            m.chooses_target(2);
            chooses(m.dialog_mut(), "alfa_gris", 4);
            writes_hex(m.dialog_mut(), text_2);
            m.synchronizes();
            // El pendiente de la casilla va en el motivo externo; el resumen suma el valor.
            assert_eq!(m.dialog().block(), Some(MISSING_COLOR), "{ctx}");
            assert_eq!(m.dialog().summary(), Some(MISSING_COLOR_AND_VALUE), "{ctx}");
            assert_eq!(
                m.dialog().reason(),
                Some(MISSING_COLOR_AND_VALUE.to_string()),
                "{ctx}"
            );
            assert!(m.accept_rejected(), "{ctx}");
            // El detalle del valor sigue en el motor.
            let problems = m.dialog().problems();
            assert_eq!(problems.len(), 1, "{ctx}: {problems:?}");
            assert!(problems[0].starts_with("Color: "), "{ctx}: {problems:?}");

            // Marcar el permiso vuelve al valor solo: el motivo del campo, sin resumen.
            mark(m.dialog_mut(), "luma_gris", true);
            m.synchronizes();
            assert_eq!(m.dialog().summary(), None, "{ctx}");
            assert_eq!(m.dialog().block(), None, "{ctx}");
            assert_eq!(m.dialog().reason(), Some(problems[0].clone()), "{ctx}");

            // Desmarcar vuelve al resumen; arreglar el hexadecimal, al motivo de uno.
            mark(m.dialog_mut(), "luma_gris", false);
            m.synchronizes();
            assert_eq!(m.dialog().summary(), Some(MISSING_COLOR_AND_VALUE), "{ctx}");
            writes_hex(m.dialog_mut(), "#ff0000");
            m.synchronizes();
            assert_eq!(m.dialog().summary(), None, "{ctx}");
            assert_eq!(
                m.dialog().reason(),
                Some(MISSING_COLOR.to_string()),
                "{ctx}"
            );
        }
    }
}

#[test]
fn row_2_20b_an_active_color_with_alpha_without_the_box_is_color_and_valid_background_without_a_summary()
 {
    for from in [Mode::Rgba, Mode::Indexed] {
        let mut m = Setup::new_2(from, image_of_mode(from));
        m.sync = ConvertSync::new(from, 0, Some(ACTIVE_WITH_ALPHA));
        m.synchronizes();
        m.chooses_target(2);
        chooses(m.dialog_mut(), "alfa_gris", 3);
        m.synchronizes();
        // Ningún campo es inválido: no hay resumen y el texto sale de `block_reason`.
        assert!(m.dialog().problems().is_empty(), "{from:?}");
        assert_eq!(m.dialog().summary(), None, "{from:?}");
        assert_eq!(
            m.dialog().block(),
            Some(MISSING_COLOR_AND_VALUE),
            "{from:?}"
        );
        assert_eq!(
            m.dialog().reason(),
            Some(MISSING_COLOR_AND_VALUE.to_string()),
            "{from:?}"
        );
        // Con la casilla, vuelve al texto de hoy del valor.
        mark(m.dialog_mut(), "luma_gris", true);
        m.synchronizes();
        assert_eq!(
            m.dialog().reason(),
            Some("el color activo #ff000080 tiene alfa".to_string()),
            "{from:?}"
        );
    }
}

// --- Fila 3: tres y recuento ---

#[test]
fn row_3_20b_the_seven_non_empty_lists_give_the_texts_of_d2_and_the_empty_one_none() {
    let lists = lists_of_missing();
    assert_eq!(lists.len(), 8);
    assert_eq!(missing_text(&lists[0]), None);
    assert_eq!(missing_text(&[]), None);
    let expected_2_2_2: [(&[Missing], &str); 6] = [
        (&[Missing::Luma], MISSING_COLOR),
        (&[Missing::AlphaBackground], CHOOSES_BACKGROUND),
        (&[Missing::Luma, Missing::AlphaBackground], MISSING_BOTH),
        (&[Missing::Luma, Missing::Value], MISSING_COLOR_AND_VALUE),
        (
            &[Missing::AlphaBackground, Missing::Value],
            MISSING_BACKGROUND_AND_VALUE,
        ),
        (
            &[Missing::Luma, Missing::AlphaBackground, Missing::Value],
            MISSING_THREE,
        ),
    ];
    for (list, text_2) in expected_2_2_2 {
        assert_eq!(missing_text(list), Some(text_2.to_string()), "{list:?}");
    }
    // Un valor inválido solo: empieza por `falta: ` y dice el rótulo del valor (ver suposiciones).
    let only = missing_text(&[Missing::Value]).expect("no vacía");
    assert!(only.starts_with("falta: "), "{only:?}");
    assert!(only.contains("fondo válido"), "{only:?}");
}

#[test]
fn row_3_20b_the_texts_of_d2_measure_what_the_table_says() {
    for (text_2, length) in [
        (MISSING_COLOR, 31),
        (CHOOSES_BACKGROUND, 31),
        (MISSING_BOTH, 38),
        (MISSING_COLOR_AND_VALUE, 36),
        (MISSING_BACKGROUND_AND_VALUE, 35),
        (MISSING_THREE, 29),
    ] {
        assert_eq!(text_2.chars().count(), length, "{text_2:?}");
    }
}

#[test]
fn row_3_20b_every_text_of_the_list_fits_in_the_44_column_footer_and_in_the_set() {
    for list in lists_of_missing() {
        let Some(text_2) = missing_text(&list) else {
            assert!(list.is_empty());
            continue;
        };
        assert!(text_2.chars().count() <= 44, "{list:?}: {text_2:?}");
        assert!(in_the_set(&text_2), "{list:?}: {text_2:?}");
        let footer = px_ui::dialog_view::footer_text(&text_2);
        assert!(footer.chars().count() <= 44, "{list:?}: {footer:?}");
        assert!(in_the_set(&footer), "{footer:?}");
        assert_eq!(
            footer, text_2,
            "una lista (`falta: ` o `faltan N: `) va tal cual"
        );
    }
}

#[test]
fn row_3_20b_footer_text_puts_error_only_on_what_does_not_start_with_missing() {
    assert_eq!(px_ui::dialog_view::MISSING_PREFIX, "falta: ");
    assert_eq!(
        px_ui::dialog_view::footer_text("falta: permitir perder el color"),
        "falta: permitir perder el color"
    );
    assert_eq!(
        px_ui::dialog_view::footer_text("no hay color activo"),
        format!("{}no hay color activo", px_ui::dialog_view::ERROR_PREFIX)
    );
    assert_eq!(
        px_ui::dialog_view::footer_text("Color: el fondo no lleva alfa"),
        "error: Color: el fondo no lleva alfa"
    );
    assert_eq!(
        px_ui::dialog_view::footer_text(MISSING_THREE),
        MISSING_THREE,
        "la forma con cuenta también es una lista"
    );
    // El motivo más largo sin `falta: ` (37) con su `error: ` llena las 44 columnas.
    let length = "m".repeat(37);
    assert_eq!(px_ui::dialog_view::footer_text(&length).chars().count(), 44);
}

// --- Fila 4: orden fijo de `pending` ---

/// Lo que debe dar `pending`, escrito aparte: color, fondo y valor, en ese orden.
fn pending_expected(
    analysis: &Analysis,
    luma: bool,
    alpha_2: usize,
    active_2: Option<Rgba>,
    value_problem: bool,
) -> Vec<Missing> {
    if is_impossible(analysis) {
        return Vec::new();
    }
    let (color, background_2) = match analysis {
        Analysis::Lossy(r) => (r.color_lost > 0, r.alpha_lost > 0),
        _ => (false, false),
    };
    let mut v = Vec::new();
    if color && !luma {
        v.push(Missing::Luma);
    }
    if background_2 && alpha_2 == 0 {
        v.push(Missing::AlphaBackground);
    }
    if value_problem || (alpha_2 == 3 && active_bad(active_2)) {
        v.push(Missing::Value);
    }
    v
}

#[test]
fn row_4_20b_pending_in_all_combinations_follows_the_order_and_the_oracle() {
    let mut checked = 0;
    for (name_2, analysis) in analysis_of_row_3() {
        for luma in [false, true] {
            for alpha_2 in 0..5 {
                for active_2 in active_2_2() {
                    for value_problem in [false, true] {
                        let ctx = format!(
                            "{name_2}, luma {luma}, alfa {}, activo {active_2:?}, valor {value_problem}",
                            NAMES_OF_ALPHA[alpha_2]
                        );
                        let real = pending(
                            &analysis,
                            &permissions(luma, alpha_2, TEST_RED),
                            active_2,
                            value_problem,
                        );
                        assert_eq!(
                            real,
                            pending_expected(&analysis, luma, alpha_2, active_2, value_problem),
                            "{ctx}"
                        );
                        // Ordenada y sin repetidos.
                        assert!(real.windows(2).all(|w| w[0] < w[1]), "{ctx}: {real:?}");
                        assert!(real.len() <= 3, "{ctx}");
                        checked += 1;
                    }
                }
            }
        }
    }
    assert_eq!(checked, 7 * 2 * 5 * 4 * 2);
}

#[test]
fn row_4_20b_pending_is_empty_if_and_only_if_the_20_would_say_none_except_the_impossible_ones() {
    for (name_2, analysis) in analysis_of_row_3() {
        for luma in [false, true] {
            for alpha_2 in 0..5 {
                for active_2 in active_2_2() {
                    let ctx = format!(
                        "{name_2}, luma {luma}, alfa {}, activo {active_2:?}",
                        NAMES_OF_ALPHA[alpha_2]
                    );
                    let list = pending(
                        &analysis,
                        &permissions(luma, alpha_2, TEST_RED),
                        active_2,
                        false,
                    );
                    let reason_2 = block_reason(
                        Some(&analysis),
                        &permissions(luma, alpha_2, TEST_RED),
                        active_2,
                    );
                    if is_impossible(&analysis) {
                        assert!(list.is_empty(), "{ctx}");
                        assert!(
                            reason_2.is_some(),
                            "{ctx}: el imposible tiene su motivo solo"
                        );
                    } else {
                        assert_eq!(list.is_empty(), reason_2.is_none(), "{ctx}");
                        assert_eq!(
                            reason_2,
                            reason_expected(Some(&analysis), luma, alpha_2, active_2),
                            "{ctx}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn row_4_20b_value_problem_adds_only_the_value_at_the_end_and_the_impossible_ones_stay_alone() {
    let lossy = lost(2, 0);
    let with = pending(
        &lossy,
        &permissions(false, 1, TEST_RED),
        Some(ACTIVE_OPAQUE),
        true,
    );
    assert_eq!(with, vec![Missing::Luma, Missing::Value]);
    let without = pending(
        &lossy,
        &permissions(false, 1, TEST_RED),
        Some(ACTIVE_OPAQUE),
        false,
    );
    assert_eq!(without, vec![Missing::Luma]);
    let already = pending(
        &lossy,
        &permissions(true, 1, TEST_RED),
        Some(ACTIVE_OPAQUE),
        true,
    );
    assert_eq!(already, vec![Missing::Value]);
    for impossible in [Analysis::TooManyColors, Analysis::TooLarge] {
        for vp in [false, true] {
            assert_eq!(
                pending(&impossible, &permissions(false, 0, TEST_RED), None, vp),
                Vec::<Missing>::new()
            );
        }
    }
}

#[test]
fn row_4_20b_missing_is_ord_in_the_order_color_background_value() {
    assert!(Missing::Luma < Missing::AlphaBackground);
    assert!(Missing::AlphaBackground < Missing::Value);
    let mut v = vec![Missing::Value, Missing::Luma, Missing::AlphaBackground];
    v.sort();
    assert_eq!(
        v,
        vec![Missing::Luma, Missing::AlphaBackground, Missing::Value]
    );
}

// --- Fila 5: oráculo nuevo de `block_reason` ---

/// El motivo de D2 por tabla de (color, fondo, valor), escrito aparte de `pending` y de
/// `missing_text`.
fn reason_per_table(
    analysis: Option<&Analysis>,
    luma: bool,
    alpha_2: usize,
    active_2: Option<Rgba>,
) -> Option<String> {
    let Some(analysis) = analysis else {
        return Some("calculando el informe".to_string());
    };
    let (color, background_2) = match analysis {
        Analysis::TooManyColors => return Some("más de 256 colores: sin cuantizador".to_string()),
        Analysis::TooLarge => return Some("la imagen convertida no cabe".to_string()),
        Analysis::Blocked(block) => return Some(block.reason().to_string()),
        Analysis::Lossy(r) => (r.color_lost > 0 && !luma, r.alpha_lost > 0 && alpha_2 == 0),
        Analysis::Exact { .. } => (false, false),
    };
    let value = alpha_2 == 3 && active_bad(active_2);
    match (color, background_2, value) {
        (false, false, false) => None,
        (true, false, false) => Some("falta: permitir perder el color".to_string()),
        (false, true, false) => Some("falta: elegir el fondo del alfa".to_string()),
        (true, true, false) => Some("falta: perder el color; fondo del alfa".to_string()),
        (true, false, true) => Some("falta: perder el color; fondo válido".to_string()),
        (false, false, true) => Some(match active_2 {
            None => "no hay color activo".to_string(),
            Some(c) => format!(
                "el color activo #{:02x}{:02x}{:02x}{:02x} tiene alfa",
                c.r, c.g, c.b, c.a
            ),
        }),
        // El fondo y el valor no coexisten: un valor inválido solo existe con un fondo elegido.
        (_, true, true) => unreachable!("fondo pendiente y fondo inválido a la vez"),
    }
}

#[test]
fn row_5_20b_block_reason_matches_the_new_oracle_in_the_whole_matrix() {
    let mut checked = 0;
    for (name_2, analysis) in analysis_of_row_3() {
        for luma in [false, true] {
            for alpha_2 in 0..5 {
                for active_2 in active_2_2() {
                    let ctx = format!(
                        "{name_2}, luma {luma}, alfa {}, activo {active_2:?}",
                        NAMES_OF_ALPHA[alpha_2]
                    );
                    let real = block_reason(
                        Some(&analysis),
                        &permissions(luma, alpha_2, TEST_RED),
                        active_2,
                    );
                    let expected_2 = reason_per_table(Some(&analysis), luma, alpha_2, active_2);
                    assert_eq!(real, expected_2, "{ctx}");
                    // `None` si y solo si la 20 lo decía.
                    assert_eq!(
                        real.is_none(),
                        reason_expected(Some(&analysis), luma, alpha_2, active_2).is_none(),
                        "{ctx}"
                    );
                    checked += 1;
                }
            }
        }
    }
    for luma in [false, true] {
        for alpha_2 in 0..5 {
            for active_2 in active_2_2() {
                assert_eq!(
                    block_reason(None, &permissions(luma, alpha_2, TEST_RED), active_2),
                    reason_per_table(None, luma, alpha_2, active_2)
                );
                checked += 1;
            }
        }
    }
    assert_eq!(checked, 7 * 2 * 5 * 4 + 2 * 5 * 4);
}

#[test]
fn row_5_20b_with_one_pending_the_same_as_the_20_and_with_two_the_text_of_two() {
    for (_, analysis) in analysis_of_row_3() {
        for luma in [false, true] {
            for alpha_2 in 0..5 {
                for active_2 in active_2_2() {
                    let perms = permissions(luma, alpha_2, TEST_RED);
                    let list = pending(&analysis, &perms, active_2, false);
                    let reason_2 = block_reason(Some(&analysis), &perms, active_2);
                    match list.len() {
                        0 | 1 if is_impossible(&analysis) => {}
                        0 => assert_eq!(reason_2, None),
                        1 => {
                            // Con uno, el motivo es el rótulo largo o el valor de hoy.
                            let text_2 = reason_2.expect("hay motivo");
                            match list[0] {
                                Missing::Luma => assert_eq!(text_2, MISSING_COLOR),
                                Missing::AlphaBackground => assert_eq!(text_2, CHOOSES_BACKGROUND),
                                Missing::Value => {
                                    assert!(!text_2.starts_with("falta"), "{text_2:?}")
                                }
                            }
                        }
                        _ => assert_eq!(reason_2, missing_text(&list)),
                    }
                }
            }
        }
    }
}

#[test]
fn row_5_20b_there_are_never_two_mixed_reasons() {
    let known = [
        MISSING_COLOR,
        CHOOSES_BACKGROUND,
        MISSING_BOTH,
        MISSING_COLOR_AND_VALUE,
        PENDING,
        WITHOUT_QUANTIZER,
        NOT_FITS,
        "no hay color activo",
    ];
    for (_, analysis) in analysis_of_row_3() {
        for luma in [false, true] {
            for alpha_2 in 0..5 {
                for active_2 in active_2_2() {
                    let Some(text_2) = block_reason(
                        Some(&analysis),
                        &permissions(luma, alpha_2, TEST_RED),
                        active_2,
                    ) else {
                        continue;
                    };
                    let is_value_active =
                        text_2.starts_with("el color activo #") && text_2.ends_with(" tiene alfa");
                    assert!(
                        is_value_active || known.contains(&text_2.as_str()),
                        "texto desconocido: {text_2:?}"
                    );
                    // Un `falta: …` no arrastra un motivo de otra clase ni un `error`.
                    if text_2.starts_with("falta: ") {
                        for foreign in ["error", "no hay color", "tiene alfa", "calculando", "256"]
                        {
                            assert!(!text_2.contains(foreign), "{text_2:?} contiene {foreign:?}");
                        }
                    }
                }
            }
        }
    }
}

// --- Propiedad 1 ---

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Para cualquier lista sin repetidos de `Missing` (0 a 3): `missing_text` es `None` si y solo
    /// si está vacía; si no, empieza por `falta: ` o `faltan N: `, mide como mucho 44 columnas, está
    /// en el juego y contiene el rótulo de **cada** pendiente en el orden fijo, o la forma con
    /// cuenta de N (nunca omite uno sin decirlo).
    #[test]
    fn property_1_the_text_says_everything_and_fits(
        list in prop::sample::subsequence(
            vec![Missing::Luma, Missing::AlphaBackground, Missing::Value],
            0..=3usize,
        ),
    ) {
        let text_2 = missing_text(&list);
        prop_assert_eq!(text_2.is_none(), list.is_empty());
        let Some(text_2) = text_2 else { return Ok(()); };
        prop_assert!(text_2.chars().count() <= 44, "{}", text_2);
        prop_assert!(in_the_set(&text_2), "{}", text_2);
        let footer = px_ui::dialog_view::footer_text(&text_2);
        prop_assert!(footer.chars().count() <= 44, "{}", footer);

        if text_2.starts_with("faltan ") {
            // La forma con cuenta dice N y las palabras de cada pendiente, en orden.
            prop_assert_eq!(
                text_2.starts_with(&format!("faltan {}: ", list.len())),
                true,
                "{}", text_2
            );
            let mut from_2 = 0;
            for word in ["color", "fondo", "valor"].iter().take(list.len()) {
                let found = text_2[from_2..].find(word);
                prop_assert!(found.is_some(), "{} no dice {}", text_2, word);
                from_2 += found.unwrap_or(0) + word.len();
            }
        } else {
            prop_assert!(text_2.starts_with("falta: "), "{}", text_2);
            // El rótulo de cada pendiente, en el orden fijo.
            let mut from_2 = 0;
            for m in &list {
                let label = label_short(*m);
                let found_2 = text_2[from_2..].find(label);
                prop_assert!(found_2.is_some(), "{} no dice {:?}", text_2, label);
                from_2 += found_2.unwrap_or(0) + label.len();
            }
        }
    }
}

// --- Propiedad 2 ---

/// `apply` acepta la conversión con ese `Allowed` en una copia de la imagen.
fn apply_accepts(image: &Image, target: Mode, allowed: Allowed) -> bool {
    let mut document = Document::new(image.clone());
    let id = ImageId::from_raw(0);
    document
        .apply(Command::ConvertMode {
            image: id,
            target,
            allowed,
        })
        .is_ok()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Para imágenes aleatorias de hasta 6 × 6, cualquier destino, permisos y color activo:
    /// `block_reason` es `None` si y solo si `Command::ConvertMode` da `Ok` (lo de la 20), y el
    /// número de requisitos del texto es el de los que **realmente faltan**, medidos con `apply`
    /// (sin `needs`): el color falta si sin la casilla falla y con ella y con fondo pasa; el fondo
    /// falta si sin fondo falla y con él pasa.
    #[test]
    fn property_2_accept_and_the_list_match_apply(
        seed in strategy_of_seed(),
        target_2 in 0usize..3,
        luma in any::<bool>(),
        alpha_2 in 0usize..5,
        hex in 0usize..4,
        active_2 in 0usize..4,
    ) {
        let image = image_of_seed(&seed);
        let target = targets_2(image.mode())[target_2];
        let analysis = analyze(&image, target);
        let active_2 = active_2_2()[active_2];
        let perms = permissions(luma, alpha_2, HEXES_OPAQUE[hex]);
        let reason_2 = block_reason(Some(&analysis), &perms, active_2);

        if let Some(allowed) = allowed_of(&perms, active_2) {
            let accepts = apply_accepts(&image, target, allowed);
            prop_assert_eq!(reason_2.is_none(), accepts, "motivo {:?}, allowed {:?}", reason_2, allowed);
        } else {
            prop_assert!(reason_2.is_some(), "un fondo no opaco no puede dejar Aceptar activo");
        }

        // Los requisitos que realmente faltan, con `apply` como juez.
        let white = Some([255, 255, 255]);
        let color_does_missing = !apply_accepts(&image, target, Allowed { luma: false, drop_alpha: white })
            && apply_accepts(&image, target, Allowed { luma: true, drop_alpha: white });
        let background_does_missing = !apply_accepts(&image, target, Allowed { luma: true, drop_alpha: None })
            && apply_accepts(&image, target, Allowed { luma: true, drop_alpha: white });
        let missing_color = color_does_missing && !perms.luma;
        let missing_background = background_does_missing && perms.alpha == AlphaChoice::Keep;
        let missing_value = !is_impossible(&analysis) && alpha_2 == 3 && active_bad(active_2);
        let real_2 = usize::from(missing_color) + usize::from(missing_background) + usize::from(missing_value);

        if is_impossible(&analysis) {
            prop_assert!(reason_2.is_some());
            prop_assert!(pending(&analysis, &perms, active_2, false).is_empty());
        } else {
            prop_assert_eq!(
                requirements_of_the_text(reason_2.as_deref()),
                real_2,
                "{:?} a {:?}: motivo {:?}", image.mode(), target, reason_2
            );
            prop_assert_eq!(
                pending(&analysis, &perms, active_2, false).len(),
                real_2,
                "pending de {:?} a {:?}", image.mode(), target
            );
        }
        if let Some(text_2) = reason_2 {
            let limit_2 = if text_2.starts_with("falta: ") { 44 } else { 37 };
            prop_assert!(text_2.chars().count() <= limit_2, "{}", text_2);
            prop_assert!(in_the_set(&text_2), "{}", text_2);
        }
    }
}

// --- Propiedad 3 ---

/// Las invariantes del resumen tras un gesto y su `sync_convert`.
fn checks_summary(
    d: &Dialog,
    m: &Model,
    sync: &ConvertSync,
    active_2: Option<Rgba>,
    ctx: &str,
) -> Result<(), TestCaseError> {
    let Some(analysis) = sync.analysis() else {
        return Err(TestCaseError::fail(format!("{ctx}: sin informe")));
    };
    let problems = d.problems();
    let there_is_problem = m.invalid_of_field() || m.fails_the_check();
    prop_assert_eq!(
        !problems.is_empty(),
        there_is_problem,
        "{}: problems {:?}",
        ctx,
        problems
    );

    // El resumen esperado, con el modelo escrito aparte.
    let perms = m.permissions();
    let need = needs(analysis);
    let impossible = is_impossible(analysis);
    let mut list = Vec::new();
    if !impossible && need.luma && !perms.luma {
        list.push(Missing::Luma);
    }
    if !impossible && need.alpha && perms.alpha == AlphaChoice::Keep {
        list.push(Missing::AlphaBackground);
    }
    let active_invalid = perms.alpha == AlphaChoice::Active && active_bad(active_2);
    if !impossible && (there_is_problem || active_invalid) {
        list.push(Missing::Value);
    }
    let expected_2 = if list.len() >= 2 && there_is_problem {
        missing_text(&list)
    } else {
        None
    };
    prop_assert_eq!(
        d.summary().map(str::to_string),
        expected_2,
        "{}: resumen",
        ctx
    );

    match d.summary() {
        Some(summary) => {
            prop_assert!(!d.is_valid(), "{}: resumen con Aceptar activo", ctx);
            prop_assert_eq!(d.reason(), Some(summary.to_string()), "{}", ctx);
            prop_assert!(
                !problems.is_empty(),
                "{}: resumen sin problema de campo",
                ctx
            );
            prop_assert_eq!(d.values(), None, "{}", ctx);
            prop_assert_eq!(
                d.clone().event(Event::Accept),
                Reaction::Rejected(summary.to_string()),
                "{}",
                ctx
            );
        }
        None => {
            // Sin resumen, el diálogo es inválido solo por su motivo externo o por un problema.
            let cause = d.block().is_some() || !problems.is_empty();
            prop_assert_eq!(d.is_valid(), !cause, "{}: validez", ctx);
        }
    }
    // `values()` es `Some` si y solo si no hay resumen ni ninguna otra causa.
    let clean = d.summary().is_none() && d.block().is_none() && problems.is_empty();
    prop_assert_eq!(d.values().is_some(), clean, "{}: values", ctx);
    prop_assert_eq!(d.reason().is_none(), clean, "{}: reason", ctx);
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Para secuencias de 0 a 48 eventos (destino, casilla, alfa, hexadecimal válido e inválido) con
    /// `sync_convert` tras cada una y desde los cuatro orígenes: hay resumen exactamente con ≥ 2
    /// pendientes y un problema de campo; un resumen nunca convive con `Accept` activo; y
    /// `values()` es `Some` si y solo si no hay resumen, motivo externo ni problema.
    #[test]
    fn property_3_the_summary_never_contradicts_the_validity(
        gestures in prop::collection::vec(gesture(), 0..=48),
        active_2 in 0usize..4,
    ) {
        let active_2 = active_2_2()[active_2];
        for from in TEST_MODES {
            let image = image_of_mode(from);
            let mut modal = Modal::default();
            prop_assert!(open_convert_mode(&mut modal, from, Locks::default()));
            let Modal::Open(d) = &mut modal else {
                return Err(TestCaseError::fail("cerrado"));
            };
            let mut sync = ConvertSync::new(from, 0, active_2);
            let mut analyzer = SystemAnalyzer;
            let mut m = Model::new_2(from);
            let history_2 = History { limit: 64 * MIB, used: 0 };
            sync_convert(d, &mut sync, &image, 0, history_2, &mut analyzer);
            checks_summary(d, &m, &sync, active_2, &format!("{from:?}, al abrir"))?;
            for (i, g) in gestures.iter().enumerate() {
                let ctx = format!("{from:?}, paso {i}: {g:?}");
                applies(d, &mut m, g);
                let history_2 = History { limit: 64 * MIB, used: 0 };
                sync_convert(d, &mut sync, &image, 0, history_2, &mut analyzer);
                checks_summary(d, &m, &sync, active_2, &ctx)?;
            }
        }
    }
}
