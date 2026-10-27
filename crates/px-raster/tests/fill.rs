//! Tests del relleno por inundación (specs/hito-2-relleno.md).
//!
//! Una prueba unitaria por cada fila de «Casos límite» (`fila_NN_...`) y una propiedad por
//! cada una de las seis de «Propiedades». Las filas 1 a 15 y las propiedades usan imágenes de
//! 12 x 12 como mucho. La fila 16 es la única grande: 1024 x 1024 y 2048 x 2048, las dos en
//! la ejecución normal (de 0,6 a 1,8 s cada una en debug); imprimen sus tiempos y solo fallan por
//! encima de 120 s.
//!
//! El oráculo (`region_naive`) es un recorrido pixel a pixel con `VecDeque`, 4 vecinos y
//! `Image::get_pixel`; no usa nada de `px-raster`. Las filas «a mano» son listas literales.

use std::collections::{BTreeSet, VecDeque};
use std::time::{Duration, Instant};

use proptest::prelude::*;
use proptest::test_runner::TestCaseError;
use px_core::{
    Command, Document, DocumentError, Image, ImageError, ImageId, Mode, Palette, Pixel, PixelEdit,
    Rgba, Span,
};
use px_raster::{fill_command, flood_fill};

// Apoyo común.

const MIN: i32 = i32::MIN;
const MAX: i32 = i32::MAX;
const ALL: [Mode; 4] = [Mode::Rgba, Mode::Indexed, Mode::Gray, Mode::GrayAlpha];

fn rgba(r: u8, g: u8, b: u8, a: u8) -> Rgba {
    Rgba::new(r, g, b, a)
}

/// `A`, `B` y `C` de la tabla: tres colores distintos (`C` es el de relleno).
fn pixel_a() -> Pixel {
    Pixel::Rgba(rgba(10, 20, 30, 255))
}

fn pixel_b() -> Pixel {
    Pixel::Rgba(rgba(40, 50, 60, 255))
}

fn pixel_c() -> Pixel {
    Pixel::Rgba(rgba(200, 100, 50, 255))
}

/// `sp(y, a, b)` de la tabla.
fn sp(y: u32, x_start: u32, x_end: u32) -> Span {
    Span { y, x_start, x_end }
}

fn i(v: u32) -> i32 {
    i32::try_from(v).expect("cabe en i32")
}

fn width_usize(v: u32) -> usize {
    usize::try_from(v).expect("cabe en usize")
}

fn palette(colors: &[Rgba]) -> Palette {
    Palette::from_colors(colors.iter().copied()).unwrap()
}

/// Imagen RGBA con el píxel que da `f` en cada casilla.
fn image_of(width: u32, height: u32, f: impl Fn(u32, u32) -> Pixel) -> Image {
    let mut img = Image::new(width, height, Mode::Rgba).unwrap();
    for y in 0..height {
        for x in 0..width {
            img.set_pixel(x, y, f(x, y)).unwrap();
        }
    }
    img
}

/// Imagen RGBA a partir de filas de texto con `A` y `B` (de arriba abajo).
fn image_matrix(rows: &[&str]) -> Image {
    let height = u32::try_from(rows.len()).unwrap();
    let width = u32::try_from(rows[0].len()).unwrap();
    image_of(width, height, |x, y| {
        match rows[width_usize(y)].as_bytes()[width_usize(x)] {
            b'A' => pixel_a(),
            b'B' => pixel_b(),
            other => panic!("invalid character {other} in the matrix"),
        }
    })
}

fn doc_matrix(rows: &[&str]) -> Document {
    Document::new(image_matrix(rows))
}

/// Todo lo observable de un documento, para comprobar que una operación no tocó nada.
fn snapshot(doc: &Document) -> (Document, bool, bool, usize) {
    (
        doc.clone(),
        doc.can_undo(),
        doc.can_redo(),
        doc.history_bytes(),
    )
}

fn editing(x: u32, y: u32, pixel: Pixel) -> PixelEdit {
    PixelEdit { x, y, pixel }
}

/// Deja en `doc` un paso deshecho (`can_undo` a `false`, `can_redo` a `true`): así se ve si
/// una operación borra la pila de rehacer. El píxel `(0, 0)` debe poder cambiar a `other`.
fn with_redo(doc: &mut Document, other: Pixel) {
    let id = ImageId::from_raw(0);
    let step = Command::SetPixels {
        image: id,
        pixels: vec![editing(0, 0, other)],
    };
    assert_eq!(doc.apply(step), Ok(true));
    assert_eq!(doc.undo(), Ok(()));
    assert!(doc.can_redo());
}

/// `Ok(Some(FillSpans { .. }))` con los datos dados.
fn command(image: ImageId, spans: &[Span], pixel: Pixel) -> Result<Option<Command>, DocumentError> {
    Ok(Some(Command::FillSpans {
        image,
        spans: spans.to_vec(),
        pixel,
    }))
}

/// Comprueba una fila «a mano»: `fill_command` da exactamente `spans` con `pixel_c()`, no toca el
/// documento; `flood_fill` lo aplica (`Ok(true)`, la imagen es la original con esos tramos
/// pintados y nada más) y `undo` la restaura.
fn checks_region(doc: &Document, seed: (i32, i32), spans: &[Span]) {
    let id = ImageId::from_raw(0);
    let before = snapshot(doc);
    assert_eq!(
        fill_command(doc, id, seed.0, seed.1, pixel_c()),
        command(id, spans, pixel_c()),
        "semilla {seed:?}"
    );
    assert_eq!(
        snapshot(doc),
        before,
        "fill_command does not touch the document"
    );

    let mut expected = doc.image(id).unwrap().clone();
    for s in spans {
        for x in s.x_start..s.x_end {
            expected.set_pixel(x, s.y, pixel_c()).unwrap();
        }
    }
    let mut copy = doc.clone();
    assert_eq!(
        flood_fill(&mut copy, id, seed.0, seed.1, pixel_c()),
        Ok(true)
    );
    assert_eq!(copy.image(id), Some(&expected), "semilla {seed:?}");
    assert_eq!(copy.undo(), Ok(()));
    assert_eq!(copy.image(id), doc.image(id), "undo restores the image");
}

/// Comprueba que `fill_command` y `flood_fill` dan `Err(error)` y que el documento no cambia.
fn checks_error(
    doc: &mut Document,
    id: ImageId,
    seed: (i32, i32),
    pixel: Pixel,
    error: &DocumentError,
) {
    let before = snapshot(doc);
    assert_eq!(
        fill_command(doc, id, seed.0, seed.1, pixel),
        Err(error.clone()),
        "fill_command, semilla {seed:?}"
    );
    assert_eq!(
        flood_fill(doc, id, seed.0, seed.1, pixel),
        Err(error.clone()),
        "flood_fill, semilla {seed:?}"
    );
    assert_eq!(snapshot(doc), before, "semilla {seed:?}");
}

/// Comprueba que no hay nada que cambiar y que la historia no se toca.
fn checks_outside(doc: &mut Document, id: ImageId, seed: (i32, i32), pixel: Pixel) {
    let before = snapshot(doc);
    assert_eq!(
        fill_command(doc, id, seed.0, seed.1, pixel),
        Ok(None),
        "fill_command, semilla {seed:?}"
    );
    assert_eq!(
        flood_fill(doc, id, seed.0, seed.1, pixel),
        Ok(false),
        "flood_fill, semilla {seed:?}"
    );
    assert_eq!(snapshot(doc), before, "semilla {seed:?}");
}

// Una prueba por fila de la tabla de casos límite.

#[test]
fn row_01_3x2_uniform() {
    let doc = doc_matrix(&["AAA", "AAA"]);
    checks_region(&doc, (1, 1), &[sp(0, 0, 3), sp(1, 0, 3)]);
}

#[test]
fn row_02_one_pixel() {
    let doc = doc_matrix(&["A"]);
    checks_region(&doc, (0, 0), &[sp(0, 0, 1)]);
}

#[test]
fn row_03_a_single_row_is_not_split_at_the_seed() {
    let doc = doc_matrix(&["AAAAA"]);
    checks_region(&doc, (2, 0), &[sp(0, 0, 5)]);
}

#[test]
fn row_04_no_span_crosses_a_row() {
    let doc = doc_matrix(&["AA", "AA", "AA"]);
    checks_region(&doc, (1, 0), &[sp(0, 0, 2), sp(1, 0, 2), sp(2, 0, 2)]);
}

#[test]
fn row_05_diagonals_do_not_count() {
    let doc = doc_matrix(&["ABA", "BAB", "ABA"]);
    checks_region(&doc, (1, 1), &[sp(1, 1, 2)]);
    // Las esquinas (`A`) quedan como estaban.
    let mut copy = doc.clone();
    let id = ImageId::from_raw(0);
    assert_eq!(flood_fill(&mut copy, id, 1, 1, pixel_c()), Ok(true));
    for (x, y) in [(0, 0), (2, 0), (0, 2), (2, 2)] {
        assert_eq!(copy.image(id).unwrap().get_pixel(x, y), Ok(pixel_a()));
    }
}

#[test]
fn row_06_board_the_region_is_one_pixel() {
    let rows = ["ABAB", "BABA", "ABAB", "BABA"];
    let doc = doc_matrix(&rows);
    // (1, 1) es `A` (x + y par).
    assert_eq!(
        doc.image(ImageId::from_raw(0)).unwrap().get_pixel(1, 1),
        Ok(pixel_a())
    );
    checks_region(&doc, (1, 1), &[sp(1, 1, 2)]);
}

#[test]
fn row_07_column_with_an_isolated_a() {
    let doc = doc_matrix(&["A", "B", "A", "A"]);
    checks_region(&doc, (0, 3), &[sp(2, 0, 1), sp(3, 0, 1)]);
}

#[test]
fn row_08_discovered_in_another_order_and_comes_out_sorted() {
    let doc = doc_matrix(&["AAA", "ABA", "ABA"]);
    checks_region(
        &doc,
        (2, 2),
        &[
            sp(0, 0, 3),
            sp(1, 0, 1),
            sp(1, 2, 3),
            sp(2, 0, 1),
            sp(2, 2, 3),
        ],
    );
}

#[test]
fn row_09_comb_from_two_seeds() {
    let doc = doc_matrix(&["AAAAA", "ABABA", "ABABA"]);
    let expected_2_2 = [
        sp(0, 0, 5),
        sp(1, 0, 1),
        sp(1, 2, 3),
        sp(1, 4, 5),
        sp(2, 0, 1),
        sp(2, 2, 3),
        sp(2, 4, 5),
    ];
    checks_region(&doc, (4, 2), &expected_2_2);
    checks_region(&doc, (0, 0), &expected_2_2);
}

#[test]
fn row_10_seed_outside_the_canvas() {
    // 3 x 2, con historia y con algo que rehacer: nada se toca.
    let mut doc = doc_matrix(&["AAA", "AAA"]);
    with_redo(&mut doc, pixel_b());
    let id = ImageId::from_raw(0);
    for seed in [(-1, 0), (3, 0), (0, 2), (MIN, MAX)] {
        checks_outside(&mut doc, id, seed, pixel_c());
    }
    // Otros extremos y esquinas contiguas al lienzo.
    for seed in [
        (0, -1),
        (MAX, 0),
        (0, MAX),
        (MIN, 0),
        (0, MIN),
        (3, 2),
        (-1, -1),
    ] {
        checks_outside(&mut doc, id, seed, pixel_c());
    }
    // Y la historia de un documento sin pasos tampoco cambia.
    let mut new_2 = doc_matrix(&["AAA", "AAA"]);
    let id = ImageId::from_raw(0);
    checks_outside(&mut new_2, id, (-1, 0), pixel_c());
    assert!(!new_2.can_undo());
}

#[test]
fn row_11_unknown_image_beats_none() {
    let mut doc = doc_matrix(&["AAA", "AAA"]);
    let unknown = ImageId::from_raw(99);
    let error = DocumentError::UnknownImage(unknown);
    // Semilla fuera y `C` distinto.
    checks_error(&mut doc, unknown, (-1, 0), pixel_c(), &error);
    // Semilla dentro y `C` igual al valor de la semilla.
    checks_error(&mut doc, unknown, (0, 0), pixel_a(), &error);
    // Las dos cosas a la vez: semilla fuera y píxel igual al de la imagen.
    checks_error(&mut doc, unknown, (MIN, MAX), pixel_a(), &error);
    // También gana a un píxel de otro modo.
    checks_error(&mut doc, unknown, (0, 0), Pixel::Gray(1), &error);
    checks_error(&mut doc, unknown, (0, 0), Pixel::Indexed(200), &error);
}

#[test]
fn row_12_pixel_of_another_mode_or_index_outside_the_palette_beats_outside() {
    // Indexed(0) sobre una imagen RGBA, con semilla fuera.
    let mut doc = doc_matrix(&["AAA", "AAA"]);
    let id = ImageId::from_raw(0);
    let error = DocumentError::Pixel {
        pixel: 0,
        source: ImageError::ModeMismatch {
            expected: Mode::Rgba,
            found: Mode::Indexed,
        },
    };
    checks_error(&mut doc, id, (-1, 0), Pixel::Indexed(0), &error);
    checks_error(&mut doc, id, (MIN, MAX), Pixel::Indexed(0), &error);
    // Con la semilla dentro es el mismo error.
    checks_error(&mut doc, id, (0, 0), Pixel::Indexed(0), &error);

    // Indexed(5) con una paleta de 3, con semilla fuera.
    let colors = [
        rgba(255, 0, 0, 255),
        rgba(0, 255, 0, 255),
        rgba(0, 0, 255, 255),
    ];
    let mut indexed = Document::new(Image::new_indexed(3, 2, palette(&colors)).unwrap());
    let id = ImageId::from_raw(0);
    let error = DocumentError::Pixel {
        pixel: 0,
        source: ImageError::IndexNotInPalette { index: 5, len: 3 },
    };
    checks_error(&mut indexed, id, (-1, 0), Pixel::Indexed(5), &error);
    checks_error(&mut indexed, id, (0, 2), Pixel::Indexed(5), &error);
    checks_error(&mut indexed, id, (0, 0), Pixel::Indexed(5), &error);
    // El último índice válido y el primero inválido.
    assert_eq!(
        flood_fill(&mut indexed, id, 0, 0, Pixel::Indexed(2)),
        Ok(true)
    );
    let error = DocumentError::Pixel {
        pixel: 0,
        source: ImageError::IndexNotInPalette { index: 3, len: 3 },
    };
    checks_error(&mut indexed, id, (0, 0), Pixel::Indexed(3), &error);
}

#[test]
fn row_13_pixel_equal_to_the_seeds_is_outside() {
    let mut doc = doc_matrix(&["AAB", "ABB"]);
    with_redo(&mut doc, pixel_c());
    let id = ImageId::from_raw(0);
    let before = snapshot(&doc);
    assert!(!before.1 && before.2);
    // `C` igual al valor de la semilla: `A` en (0, 0) y `B` en (2, 0).
    checks_outside(&mut doc, id, (0, 0), pixel_a());
    checks_outside(&mut doc, id, (2, 0), pixel_b());
    assert_eq!(snapshot(&doc), before);

    // Con una pila de deshacer no vacía.
    let mut with_steps = doc_matrix(&["AAB", "ABB"]);
    let id = ImageId::from_raw(0);
    assert_eq!(flood_fill(&mut with_steps, id, 0, 0, pixel_c()), Ok(true));
    let before = snapshot(&with_steps);
    assert!(before.1 && !before.2 && before.3 > 0);
    checks_outside(&mut with_steps, id, (0, 0), pixel_c());
    checks_outside(&mut with_steps, id, (2, 1), pixel_b());
    assert_eq!(snapshot(&with_steps), before);

    // Imagen recién creada: transparente con transparente.
    let mut new = Document::new(Image::new(2, 2, Mode::Rgba).unwrap());
    let id = ImageId::from_raw(0);
    checks_outside(&mut new, id, (1, 1), Pixel::Rgba(Rgba::TRANSPARENT));
}

fn indexed_doc_0_1_0(colors: &[Rgba]) -> Document {
    let mut img = Image::new_indexed(3, 1, palette(colors)).unwrap();
    img.set_pixel(1, 0, Pixel::Indexed(1)).unwrap();
    Document::new(img)
}

#[test]
fn row_14_indices_with_the_same_color_are_different_values() {
    let red = rgba(255, 0, 0, 255);
    let blue = rgba(0, 0, 255, 255);
    let doc = indexed_doc_0_1_0(&[red, red, blue]);
    let id = ImageId::from_raw(0);
    for fill in [Pixel::Indexed(2), Pixel::Indexed(1)] {
        // El índice 1 tiene el mismo color que el 0, pero es otro valor: la región es (0, 0).
        let before = snapshot(&doc);
        assert_eq!(
            fill_command(&doc, id, 0, 0, fill),
            command(id, &[sp(0, 0, 1)], fill),
            "relleno {fill:?}"
        );
        assert_eq!(snapshot(&doc), before);
        // Y es un cambio real: `Ok(true)`, también con `Indexed(1)`.
        let mut copy = doc.clone();
        assert_eq!(flood_fill(&mut copy, id, 0, 0, fill), Ok(true));
        let img = copy.image(id).unwrap();
        assert_eq!(img.get_pixel(0, 0), Ok(fill));
        assert_eq!(img.get_pixel(1, 0), Ok(Pixel::Indexed(1)));
        assert_eq!(img.get_pixel(2, 0), Ok(Pixel::Indexed(0)));
    }
    // Desde el índice 1, en medio, la región es solo él.
    assert_eq!(
        fill_command(&doc, id, 1, 0, Pixel::Indexed(2)),
        command(id, &[sp(0, 1, 2)], Pixel::Indexed(2))
    );
}

#[test]
fn row_14_indices_with_different_palette_alpha_are_different_values() {
    let red = rgba(255, 0, 0, 255);
    let red_translucent = rgba(255, 0, 0, 128);
    let doc = indexed_doc_0_1_0(&[red, red_translucent, rgba(0, 0, 255, 255)]);
    let id = ImageId::from_raw(0);
    assert_eq!(
        fill_command(&doc, id, 0, 0, Pixel::Indexed(2)),
        command(id, &[sp(0, 0, 1)], Pixel::Indexed(2))
    );
    let mut copy = doc.clone();
    assert_eq!(flood_fill(&mut copy, id, 2, 0, Pixel::Indexed(1)), Ok(true));
    let img = copy.image(id).unwrap();
    assert_eq!(img.get_pixel(0, 0), Ok(Pixel::Indexed(0)));
    assert_eq!(img.get_pixel(2, 0), Ok(Pixel::Indexed(1)));
}

#[test]
fn row_15_equal_rgb_with_different_alpha_are_different_values() {
    let mut img = Image::new(2, 1, Mode::Rgba).unwrap();
    img.set_pixel(0, 0, Pixel::Rgba(rgba(10, 20, 30, 255)))
        .unwrap();
    img.set_pixel(1, 0, Pixel::Rgba(rgba(10, 20, 30, 254)))
        .unwrap();
    let doc = Document::new(img);
    let id = ImageId::from_raw(0);
    assert_eq!(
        fill_command(&doc, id, 0, 0, pixel_c()),
        command(id, &[sp(0, 0, 1)], pixel_c())
    );
    assert_eq!(
        fill_command(&doc, id, 1, 0, pixel_c()),
        command(id, &[sp(0, 1, 2)], pixel_c())
    );
    // Rellenar el primero con el valor del segundo es un cambio real.
    let same_as_the_second = Pixel::Rgba(rgba(10, 20, 30, 254));
    assert_eq!(
        fill_command(&doc, id, 0, 0, same_as_the_second),
        command(id, &[sp(0, 0, 1)], same_as_the_second)
    );
    let mut copy = doc.clone();
    assert_eq!(
        flood_fill(&mut copy, id, 0, 0, same_as_the_second),
        Ok(true)
    );
    assert_eq!(
        copy.image(id).unwrap().get_pixel(0, 0),
        Ok(same_as_the_second)
    );
    assert!(copy.can_undo());
}

#[test]
fn row_15_invisible_with_different_rgb_are_different_values() {
    // Comportamiento 2: (0,0,0,0) y (9,9,9,0) son distintos aunque ambos sean invisibles.
    let mut img = Image::new(2, 1, Mode::Rgba).unwrap();
    img.set_pixel(1, 0, Pixel::Rgba(rgba(9, 9, 9, 0))).unwrap();
    assert_eq!(img.get_pixel(0, 0), Ok(Pixel::Rgba(Rgba::TRANSPARENT)));
    let doc = Document::new(img);
    let id = ImageId::from_raw(0);
    assert_eq!(
        fill_command(&doc, id, 0, 0, pixel_c()),
        command(id, &[sp(0, 0, 1)], pixel_c())
    );
    assert_eq!(
        fill_command(&doc, id, 1, 0, pixel_c()),
        command(id, &[sp(0, 1, 2)], pixel_c())
    );
}

#[test]
fn row_15_gray_alpha_with_equal_gray_or_equal_alpha_are_different_values() {
    let mut img = Image::new(2, 1, Mode::GrayAlpha).unwrap();
    img.set_pixel(0, 0, Pixel::GrayAlpha { gray: 5, alpha: 0 })
        .unwrap();
    img.set_pixel(1, 0, Pixel::GrayAlpha { gray: 6, alpha: 0 })
        .unwrap();
    let doc = Document::new(img);
    let id = ImageId::from_raw(0);
    let fill = Pixel::GrayAlpha { gray: 9, alpha: 9 };
    assert_eq!(
        fill_command(&doc, id, 0, 0, fill),
        command(id, &[sp(0, 0, 1)], fill)
    );
    assert_eq!(
        fill_command(&doc, id, 1, 0, fill),
        command(id, &[sp(0, 1, 2)], fill)
    );
    let mut copy = doc.clone();
    assert_eq!(flood_fill(&mut copy, id, 0, 0, fill), Ok(true));
    let img = copy.image(id).unwrap();
    assert_eq!(img.get_pixel(0, 0), Ok(fill));
    assert_eq!(
        img.get_pixel(1, 0),
        Ok(Pixel::GrayAlpha { gray: 6, alpha: 0 })
    );

    // Mismo gris con distinto alfa.
    let mut img = Image::new(2, 1, Mode::GrayAlpha).unwrap();
    img.set_pixel(
        0,
        0,
        Pixel::GrayAlpha {
            gray: 5,
            alpha: 255,
        },
    )
    .unwrap();
    img.set_pixel(
        1,
        0,
        Pixel::GrayAlpha {
            gray: 5,
            alpha: 254,
        },
    )
    .unwrap();
    let doc = Document::new(img);
    let id = ImageId::from_raw(0);
    assert_eq!(
        fill_command(&doc, id, 0, 0, fill),
        command(id, &[sp(0, 0, 1)], fill)
    );
}

// Fila 16: lienzos grandes.

/// Por encima de este tiempo hay un coste cuadrático: es lo único que hace fallar.
const LIMIT: Duration = Duration::from_secs(120);

#[derive(Clone, Copy, Debug)]
enum Large {
    /// (a) Todo `A`, semilla en el centro.
    Uniform,
    /// (b) Tablero (`A` si `x + y` es par), semilla en el centro.
    Board,
    /// (c) Peine: fila 0 llena y columnas pares `A` en el resto; semilla `(0, 1)`.
    Comb,
}

impl Large {
    fn es_a(self, x: u32, y: u32) -> bool {
        match self {
            Large::Uniform => true,
            Large::Board => (x + y).is_multiple_of(2),
            Large::Comb => y == 0 || x.is_multiple_of(2),
        }
    }

    fn seed(self, l: u32) -> (u32, u32) {
        match self {
            Large::Uniform | Large::Board => (l / 2, l / 2),
            Large::Comb => (0, 1),
        }
    }

    /// Casilla que cambia con el relleno.
    fn in_region(self, l: u32, x: u32, y: u32) -> bool {
        match self {
            Large::Uniform | Large::Comb => self.es_a(x, y),
            Large::Board => (x, y) == self.seed(l),
        }
    }

    /// Comprueba los tramos de la tabla.
    fn checks_spans(self, l: u32, spans: &[Span]) {
        match self {
            Large::Uniform => {
                assert_eq!(spans.len(), width_usize(l), "(a): L tramos");
                assert!(
                    spans.iter().copied().eq((0..l).map(|y| sp(y, 0, l))),
                    "(a): one span sp(y, 0, L) per row"
                );
            }
            Large::Board => {
                assert_eq!(spans, [sp(l / 2, l / 2, l / 2 + 1)], "(b): un solo tramo");
            }
            Large::Comb => {
                let l64 = u64::from(l);
                let expected_2 = 1 + (l64 - 1) * l64 / 2;
                assert_eq!(u64::try_from(spans.len()).unwrap(), expected_2, "(c): S");
                // Unos cuantos tramos concretos.
                assert_eq!(spans[0], sp(0, 0, l));
                assert_eq!(spans[1], sp(1, 0, 1));
                assert_eq!(spans[2], sp(1, 2, 3));
                assert_eq!(spans[width_usize(l / 2)], sp(1, l - 2, l - 1));
                assert_eq!(spans[width_usize(l / 2) + 1], sp(2, 0, 1));
                assert_eq!(spans[spans.len() - 1], sp(l - 1, l - 2, l - 1));
                // Y toda la secuencia, en el orden de la salida.
                let rest = (1..l).flat_map(|y| (0..l / 2).map(move |k| sp(y, 2 * k, 2 * k + 1)));
                assert!(
                    spans
                        .iter()
                        .copied()
                        .eq(std::iter::once(sp(0, 0, l)).chain(rest)),
                    "(c): the whole sequence"
                );
            }
        }
    }
}

/// Ejecuta una de las pruebas grandes y escribe sus tiempos.
fn test_large(case: Large, l: u32) {
    let start = Instant::now();
    let original = image_of(l, l, |x, y| {
        if case.es_a(x, y) {
            pixel_a()
        } else {
            pixel_b()
        }
    });
    let fills = image_of(l, l, |x, y| {
        if case.in_region(l, x, y) {
            pixel_c()
        } else if case.es_a(x, y) {
            pixel_a()
        } else {
            pixel_b()
        }
    });
    let t_images = start.elapsed();

    let mut doc = Document::new(original.clone());
    let id = ImageId::from_raw(0);
    let (sx, sy) = case.seed(l);

    let t0 = Instant::now();
    let result = fill_command(&doc, id, i(sx), i(sy), pixel_c());
    let t_fill = t0.elapsed();
    let command = match result {
        Ok(Some(c)) => c,
        other => panic!("{case:?} L={l}: Some(..) was expected, got {other:?}"),
    };
    let number_of_spans = match &command {
        Command::FillSpans {
            image,
            spans,
            pixel,
        } => {
            assert_eq!(*image, id);
            assert_eq!(*pixel, pixel_c());
            case.checks_spans(l, spans);
            spans.len()
        }
        other => panic!("{case:?} L={l}: FillSpans was expected, got {other:?}"),
    };
    assert!(
        doc.image(id) == Some(&original),
        "fill_command must not touch the image"
    );

    let t1 = Instant::now();
    let applied = doc.apply(command);
    let t_apply = t1.elapsed();
    assert_eq!(applied, Ok(true), "{case:?} L={l}: apply");
    assert!(
        doc.image(id) == Some(&fills),
        "{case:?} L={l}: the image is filled"
    );

    let t2 = Instant::now();
    assert_eq!(doc.undo(), Ok(()));
    let t_undo = t2.elapsed();
    assert!(
        doc.image(id) == Some(&original),
        "{case:?} L={l}: undo restores the image"
    );

    let t3 = Instant::now();
    assert_eq!(doc.redo(), Ok(()));
    let t_redo = t3.elapsed();
    assert!(
        doc.image(id) == Some(&fills),
        "{case:?} L={l}: redo fills it again"
    );

    let total = start.elapsed();
    println!(
        "fila 16 {case:?} L={l}: {number_of_spans} tramos; imágenes {t_images:?}, \
         fill_command {t_fill:?}, apply {t_apply:?}, undo {t_undo:?}, redo {t_redo:?}, \
         total {total:?}, history_bytes {}",
        doc.history_bytes()
    );
    assert!(
        t_fill < LIMIT && t_apply < LIMIT && total < LIMIT,
        "{case:?} L={l}: exceeds {LIMIT:?} (quadratic cost)"
    );
}

#[test]
fn row_16a_uniform_1024() {
    test_large(Large::Uniform, 1024);
}

#[test]
fn row_16b_board_1024() {
    test_large(Large::Board, 1024);
}

#[test]
fn row_16c_comb_1024() {
    test_large(Large::Comb, 1024);
}

#[test]
fn row_16a_uniform_2048() {
    test_large(Large::Uniform, 2048);
}

#[test]
fn row_16b_board_2048() {
    test_large(Large::Board, 2048);
}

#[test]
fn row_16c_comb_2048() {
    test_large(Large::Comb, 2048);
}

// Oráculo.

/// Recorrido ingenuo pixel a pixel con cola: 4 vecinos y valor exacto (`get_pixel`).
fn region_naive(img: &Image, sx: u32, sy: u32) -> BTreeSet<(u32, u32)> {
    let value = img.get_pixel(sx, sy).unwrap();
    let mut region = BTreeSet::new();
    let mut queue = VecDeque::new();
    region.insert((sx, sy));
    queue.push_back((sx, sy));
    while let Some((x, y)) = queue.pop_front() {
        let (xi, yi) = (i64::from(x), i64::from(y));
        for (nx, ny) in [(xi - 1, yi), (xi + 1, yi), (xi, yi - 1), (xi, yi + 1)] {
            let (Ok(nx), Ok(ny)) = (u32::try_from(nx), u32::try_from(ny)) else {
                continue;
            };
            if !region.contains(&(nx, ny)) && img.get_pixel(nx, ny) == Ok(value) {
                region.insert((nx, ny));
                queue.push_back((nx, ny));
            }
        }
    }
    region
}

/// Los píxeles de los tramos, uno por uno y con repeticiones si las hubiera.
fn pixels_of(spans: &[Span]) -> Vec<(u32, u32)> {
    spans
        .iter()
        .flat_map(|s| (s.x_start..s.x_end).map(move |x| (x, s.y)))
        .collect()
}

// Estrategias.

fn mode_2() -> impl Strategy<Value = Mode> {
    prop::sample::select(ALL.to_vec())
}

/// Paleta de seis entradas con casi duplicados: 0 y 1 igual color, 2 mismo RGB con otro alfa.
fn palette_six() -> Palette {
    palette(&[
        rgba(200, 0, 0, 255),
        rgba(200, 0, 0, 255),
        rgba(200, 0, 0, 128),
        rgba(0, 0, 200, 255),
        rgba(0, 200, 0, 255),
        rgba(5, 5, 5, 0),
    ])
}

fn image_empty(mode: Mode, width: u32, height: u32) -> Image {
    match mode {
        Mode::Indexed => Image::new_indexed(width, height, palette_six()).unwrap(),
        _ => Image::new(width, height, mode).unwrap(),
    }
}

/// Valores posibles del fondo, con casi duplicados (hay al menos cuatro por modo).
fn pool_background(mode: Mode) -> Vec<Pixel> {
    match mode {
        Mode::Rgba => vec![
            Pixel::Rgba(rgba(10, 20, 30, 255)),
            Pixel::Rgba(rgba(10, 20, 30, 254)),
            Pixel::Rgba(rgba(10, 20, 30, 0)),
            Pixel::Rgba(rgba(9, 9, 9, 0)),
            Pixel::Rgba(rgba(40, 50, 60, 255)),
        ],
        Mode::Indexed => (0..6).map(Pixel::Indexed).collect(),
        Mode::Gray => vec![
            Pixel::Gray(5),
            Pixel::Gray(6),
            Pixel::Gray(7),
            Pixel::Gray(8),
        ],
        Mode::GrayAlpha => vec![
            Pixel::GrayAlpha {
                gray: 5,
                alpha: 255,
            },
            Pixel::GrayAlpha {
                gray: 5,
                alpha: 254,
            },
            Pixel::GrayAlpha { gray: 5, alpha: 0 },
            Pixel::GrayAlpha { gray: 6, alpha: 0 },
            Pixel::GrayAlpha {
                gray: 6,
                alpha: 255,
            },
        ],
    }
}

/// Valores de relleno válidos: los del fondo (hay no-ops y cambios a un casi duplicado) y
/// alguno que no está en el fondo.
fn pool_fill(mode: Mode) -> Vec<Pixel> {
    let mut v = pool_background(mode);
    match mode {
        Mode::Rgba => {
            v.push(Pixel::Rgba(rgba(1, 2, 3, 4)));
            v.push(Pixel::Rgba(Rgba::TRANSPARENT));
        }
        Mode::Indexed => {}
        Mode::Gray => v.push(Pixel::Gray(99)),
        Mode::GrayAlpha => v.push(Pixel::GrayAlpha { gray: 99, alpha: 1 }),
    }
    v
}

/// Imagen pequeña con un fondo de 2 a 4 valores, una semilla dentro y un relleno válido.
#[derive(Clone, Debug)]
struct Case {
    image: Image,
    seed: (u32, u32),
    pixel: Pixel,
}

impl Case {
    fn document(&self) -> (Document, ImageId) {
        let doc = Document::new(self.image.clone());
        let id = ImageId::from_raw(0);
        (doc, id)
    }

    fn seed_i_32(&self) -> (i32, i32) {
        (i(self.seed.0), i(self.seed.1))
    }

    fn value(&self) -> Pixel {
        self.image.get_pixel(self.seed.0, self.seed.1).unwrap()
    }
}

fn cases() -> impl Strategy<Value = Case> {
    (mode_2(), 1u32..=12, 1u32..=12)
        .prop_flat_map(|(mode, w, h)| {
            let cels = width_usize(w * h);
            (
                Just((mode, w, h)),
                prop::sample::subsequence(pool_background(mode), 2..=4),
                prop::collection::vec(any::<u8>(), cels),
                0..w,
                0..h,
                prop::sample::select(pool_fill(mode)),
            )
        })
        .prop_map(|((mode, w, h), background, cels, sx, sy, pixel)| {
            let mut image = image_empty(mode, w, h);
            for y in 0..h {
                for x in 0..w {
                    let byte = cels[width_usize(y * w + x)];
                    // La mitad de las casillas valen el primer valor: ganan las regiones grandes.
                    let k = if byte < 128 {
                        0
                    } else {
                        usize::from(byte) % background.len()
                    };
                    image.set_pixel(x, y, background[k]).unwrap();
                }
            }
            Case {
                image,
                seed: (sx, sy),
                pixel,
            }
        })
}

/// Píxel de cualquier modo y con cualquier índice (a menudo fuera de la paleta de seis).
fn pixel_any() -> impl Strategy<Value = Pixel> {
    prop_oneof![
        (any::<u8>(), any::<u8>(), any::<u8>(), any::<u8>())
            .prop_map(|(r, g, b, a)| Pixel::Rgba(rgba(r, g, b, a))),
        prop_oneof![3 => 0u8..8, 1 => any::<u8>()].prop_map(Pixel::Indexed),
        any::<u8>().prop_map(Pixel::Gray),
        (any::<u8>(), any::<u8>()).prop_map(|(gray, alpha)| Pixel::GrayAlpha { gray, alpha }),
    ]
}

/// Cualquier `i32`, con los extremos y las cercanías de un lienzo pequeño siempre presentes.
fn coordinate() -> impl Strategy<Value = i32> {
    prop_oneof![
        4 => -2i32..=13,
        2 => any::<i32>(),
        1 => Just(MIN),
        1 => Just(MAX),
        1 => Just(MIN + 1),
        1 => Just(MAX - 1),
    ]
}

// Apoyo de las propiedades.

/// Los tramos de un `Ok(Some(FillSpans { .. }))` sobre `id` con `pixel`; `None` si no hay
/// comando. Cualquier otra cosa es un fallo.
fn spans_of(
    result: Result<Option<Command>, DocumentError>,
    id: ImageId,
    pixel: Pixel,
) -> Result<Option<Vec<Span>>, TestCaseError> {
    match result {
        Ok(Some(Command::FillSpans {
            image,
            spans,
            pixel: written,
        })) => {
            prop_assert_eq!(image, id);
            prop_assert_eq!(written, pixel);
            Ok(Some(spans))
        }
        Ok(None) => Ok(None),
        other => Err(TestCaseError::fail(format!(
            "se esperaba Ok(None) u Ok(Some(FillSpans)), salió {other:?}"
        ))),
    }
}

/// Imagen original con `region` escrita con `pixel`, solo con `Image::set_pixel`.
fn with_region_painted(original: &Image, region: &BTreeSet<(u32, u32)>, pixel: Pixel) -> Image {
    let mut img = original.clone();
    for &(x, y) in region {
        img.set_pixel(x, y, pixel).unwrap();
    }
    img
}

/// Error que daría `Document::apply(SetPixels)` con `pixel` sobre un documento del mismo modo
/// y con la misma paleta que `img` (`None` si el píxel es válido).
fn error_of_apply(img: &Image, pixel: Pixel) -> Option<DocumentError> {
    let reference = match img.palette() {
        Some(p) => Image::new_indexed(1, 1, p.clone()).unwrap(),
        None => Image::new(1, 1, img.mode()).unwrap(),
    };
    let mut doc = Document::new(reference);
    let id = ImageId::from_raw(0);
    let step = Command::SetPixels {
        image: id,
        pixels: vec![editing(0, 0, pixel)],
    };
    doc.apply(step).err()
}

/// Un paso previo que cambia `(0, 0)` por otro valor (si lo hay).
fn step_previous(doc: &mut Document) {
    let id = ImageId::from_raw(0);
    let image_2 = doc.image(id).unwrap();
    let actual = image_2.get_pixel(0, 0).unwrap();
    let other = pool_fill(image_2.mode()).into_iter().find(|&p| p != actual);
    if let Some(other) = other {
        let step = Command::SetPixels {
            image: id,
            pixels: vec![editing(0, 0, other)],
        };
        assert_eq!(doc.apply(step), Ok(true));
    }
}

/// Forma de los tramos (propiedad 2).
fn checks_shape(img: &Image, value: Pixel, spans: &[Span]) -> Result<(), TestCaseError> {
    let (w, h) = (img.width(), img.height());
    let cap = width_usize(h) * width_usize(w.div_ceil(2));
    prop_assert!(spans.len() <= cap, "{} tramos > {}", spans.len(), cap);
    for s in spans {
        prop_assert!(s.x_start < s.x_end, "empty span {:?}", s);
        prop_assert!(s.y < h && s.x_end <= w, "{:?} fuera de {}x{}", s, w, h);
        // Máximo: el píxel de la izquierda y el de la derecha, si existen, no valen lo de la semilla.
        if s.x_start > 0 {
            prop_assert_ne!(img.get_pixel(s.x_start - 1, s.y).unwrap(), value, "{:?}", s);
        }
        if s.x_end < w {
            prop_assert_ne!(img.get_pixel(s.x_end, s.y).unwrap(), value, "{:?}", s);
        }
        for x in s.x_start..s.x_end {
            prop_assert_eq!(img.get_pixel(x, s.y).unwrap(), value, "({}, {})", x, s.y);
        }
    }
    for par in spans.windows(2) {
        let (a, b) = (par[0], par[1]);
        prop_assert!(
            (a.y, a.x_start) < (b.y, b.x_start),
            "not strictly ordered: {:?} and {:?}",
            a,
            b
        );
        if a.y == b.y {
            // Sin solapes y fusionados: queda al menos un píxel distinto entre los dos.
            prop_assert!(a.x_end < b.x_start, "{:?} and {:?} touch or overlap", a, b);
        }
    }
    Ok(())
}

// Propiedades.

proptest! {
    // Propiedad 1: oráculo.
    #[test]
    fn region_equal_to_the_oracle(case in cases()) {
        let (mut doc, id) = case.document();
        let (sx, sy) = case.seed;
        let (x, y) = case.seed_i_32();
        let value = case.value();
        let region = region_naive(&case.image, sx, sy);

        let result = fill_command(&doc, id, x, y, case.pixel);
        let changes = case.pixel != value;
        match spans_of(result, id, case.pixel)? {
            None => prop_assert!(!changes, "Ok(None) with a pixel different from the seed one"),
            Some(spans) => {
                prop_assert!(changes, "Some(..) with a pixel equal to the seed one");
                let list = pixels_of(&spans);
                let set: BTreeSet<(u32, u32)> = list.iter().copied().collect();
                prop_assert_eq!(list.len(), set.len(), "repeated pixels in the spans");
                prop_assert_eq!(&set, &region);
            }
        }

        // Tras `flood_fill`: la original con la región en `pixel` y nada más.
        prop_assert_eq!(flood_fill(&mut doc, id, x, y, case.pixel), Ok(changes));
        let expected = if changes {
            with_region_painted(&case.image, &region, case.pixel)
        } else {
            case.image.clone()
        };
        prop_assert_eq!(doc.image(id), Some(&expected));
    }

    // Propiedad 2: forma de los tramos.
    #[test]
    fn shape_of_the_spans(case in cases()) {
        let (doc, id) = case.document();
        let (x, y) = case.seed_i_32();
        let value = case.value();
        match spans_of(fill_command(&doc, id, x, y, case.pixel), id, case.pixel)? {
            None => prop_assert_eq!(case.pixel, value),
            Some(spans) => {
                prop_assert!(!spans.is_empty());
                checks_shape(&case.image, value, &spans)?;
            }
        }
    }

    // Propiedad 3: historia de un solo paso.
    #[test]
    fn a_single_history_step(case in cases(), preview in any::<bool>()) {
        let (mut doc, id) = case.document();
        if preview {
            step_previous(&mut doc);
        }
        let (x, y) = case.seed_i_32();
        let before = snapshot(&doc);
        let image_before = doc.image(id).unwrap().clone();
        // El paso previo puede haber cambiado la semilla (solo si es (0, 0)).
        let value_now = image_before.get_pixel(case.seed.0, case.seed.1).unwrap();

        // `fill_command` no cambia el documento.
        let c = fill_command(&doc, id, x, y, case.pixel);
        prop_assert!(c.is_ok(), "fill_command dio {:?}", c);
        prop_assert_eq!(snapshot(&doc), before.clone());

        let changes = case.pixel != value_now;
        prop_assert_eq!(flood_fill(&mut doc, id, x, y, case.pixel), Ok(changes));
        if !changes {
            prop_assert_eq!(snapshot(&doc), before);
            return Ok(());
        }
        let fills = doc.image(id).unwrap().clone();
        prop_assert!(doc.can_undo());
        prop_assert!(!doc.can_redo());
        prop_assert!(doc.history_bytes() > before.3);

        // Un solo paso: un `undo` deja la imagen y la historia como estaban.
        prop_assert_eq!(doc.undo(), Ok(()));
        prop_assert_eq!(doc.image(id), Some(&image_before));
        prop_assert_eq!(doc.can_undo(), before.1);
        prop_assert!(doc.can_redo());
        prop_assert_eq!(doc.history_bytes(), before.3);

        prop_assert_eq!(doc.redo(), Ok(()));
        prop_assert_eq!(doc.image(id), Some(&fills));
        prop_assert!(doc.can_undo());
        prop_assert!(!doc.can_redo());
    }

    // Propiedad 4: repetición y no-ops.
    #[test]
    fn repeat_and_no_ops(case in cases()) {
        let (mut doc, id) = case.document();
        let (x, y) = case.seed_i_32();
        let value = case.value();
        let changes = case.pixel != value;

        prop_assert_eq!(flood_fill(&mut doc, id, x, y, case.pixel), Ok(changes));
        // La segunda vez la semilla ya vale `pixel`: no hay nada que cambiar.
        let after = snapshot(&doc);
        prop_assert_eq!(flood_fill(&mut doc, id, x, y, case.pixel), Ok(false));
        prop_assert_eq!(fill_command(&doc, id, x, y, case.pixel), Ok(None));
        prop_assert_eq!(snapshot(&doc), after);

        if changes {
            // Tras un `undo`, un relleno que da `Ok(false)` no borra la pila de rehacer.
            prop_assert_eq!(doc.undo(), Ok(()));
            prop_assert!(doc.can_redo());
            let after_undo = snapshot(&doc);
            // Mismo valor que la semilla (el original): no cambia nada.
            prop_assert_eq!(flood_fill(&mut doc, id, x, y, value), Ok(false));
            prop_assert!(doc.can_redo());
            prop_assert_eq!(snapshot(&doc), after_undo.clone());
            // Semilla fuera del lienzo.
            prop_assert_eq!(flood_fill(&mut doc, id, -1, y, case.pixel), Ok(false));
            prop_assert_eq!(flood_fill(&mut doc, id, x, MAX, case.pixel), Ok(false));
            prop_assert!(doc.can_redo());
            prop_assert_eq!(snapshot(&doc), after_undo);
        }
    }

    // Propiedad 5: errores y semillas arbitrarias.
    #[test]
    fn errors_and_arbitrary_seeds(
        case in cases(),
        seed_of_the_case in any::<bool>(),
        (xr, yr) in (coordinate(), coordinate()),
        other_pixel in pixel_any(),
        uses_other_pixel in any::<bool>(),
        foreign in any::<bool>(),
        raw in any::<u32>(),
    ) {
        let (mut doc, main) = case.document();
        let (x, y) = if seed_of_the_case { case.seed_i_32() } else { (xr, yr) };
        let pixel = if uses_other_pixel { other_pixel } else { case.pixel };
        let id = if foreign { ImageId::from_raw(raw) } else { main };
        let (w, h) = (i64::from(case.image.width()), i64::from(case.image.height()));
        let outside = x < 0 || y < 0 || i64::from(x) >= w || i64::from(y) >= h;

        let before = snapshot(&doc);
        // La imagen tal como estaba, antes de rellenar.
        let image_preview = doc.image(id).cloned();
        let c = fill_command(&doc, id, x, y, pixel);
        prop_assert_eq!(snapshot(&doc), before.clone());
        let f = flood_fill(&mut doc, id, x, y, pixel);

        // `UnknownImage` > `Pixel` > fuera > igual.
        let Some(img) = image_preview else {
            let e = DocumentError::UnknownImage(id);
            prop_assert_eq!(c, Err(e.clone()));
            prop_assert_eq!(f, Err(e));
            prop_assert_eq!(snapshot(&doc), before);
            return Ok(());
        };
        if let Some(e) = error_of_apply(&img, pixel) {
            prop_assert!(
                matches!(&e, DocumentError::Pixel { pixel: 0, .. }),
                "apply should give Pixel {{ pixel: 0, .. }}, gave {:?}", e
            );
            prop_assert_eq!(c, Err(e.clone()));
            prop_assert_eq!(f, Err(e));
            prop_assert_eq!(snapshot(&doc), before);
            return Ok(());
        }
        if outside {
            prop_assert_eq!(c, Ok(None));
            prop_assert_eq!(f, Ok(false));
            prop_assert_eq!(snapshot(&doc), before);
            return Ok(());
        }
        let (ux, uy) = (u32::try_from(x).unwrap(), u32::try_from(y).unwrap());
        if img.get_pixel(ux, uy).unwrap() == pixel {
            prop_assert_eq!(c, Ok(None));
            prop_assert_eq!(f, Ok(false));
            prop_assert_eq!(snapshot(&doc), before);
            return Ok(());
        }
        prop_assert!(
            matches!(c, Ok(Some(Command::FillSpans { .. }))),
            "Some(FillSpans) was expected, got {:?}", c
        );
        prop_assert_eq!(f, Ok(true));
        prop_assert_eq!(doc.image(id).unwrap().get_pixel(ux, uy), Ok(pixel));
    }

    // Propiedad 6: independencia de la semilla.
    #[test]
    fn independent_of_the_seed(case in cases()) {
        let (doc, id) = case.document();
        let (x, y) = case.seed_i_32();
        let base = fill_command(&doc, id, x, y, case.pixel);
        let region = region_naive(&case.image, case.seed.0, case.seed.1);
        for &(px, py) in &region {
            let other = fill_command(&doc, id, i(px), i(py), case.pixel);
            prop_assert_eq!(&other, &base, "semilla ({}, {})", px, py);
        }
    }
}

// Unidad 14c: límites del relleno (specs/hito-2-limites-relleno.md).
//
// Filas 1 a 6 y 15 y propiedades 1 a 3 de la spec. Las filas 7 a 10 están en `px-core`, la 11 en
// `px-ui`, la 13 en `px-ui` (`tests/status.rs`) y la 12 y la 14 en `tests/fill_memory.rs` (que
// son `#[ignore]`). Las filas 4, 5 y 15 usan peines de 16384 x 368 (24 MiB en RGBA) y cuestan
// unos segundos en debug; el resto son imágenes de 12 de lado o menos.

mod limits {
    use super::*;
    use px_raster::fill_command_with_limit;

    /// Valor de la spec (D1). La única fuente es `Document::MAX_FILL_SPANS`; aquí se repite el
    /// número a propósito para que cambiarlo haga fallar la prueba.
    const CAP: usize = 3_000_000;

    fn too_much(limit: usize) -> DocumentError {
        DocumentError::FillTooFragmented { limit }
    }

    /// Peine de `w` x `h` RGBA: fila 0 toda `A` y, en el resto, `A` en las columnas pares y `B`
    /// en las impares. Con `teeth = Some(k)`, la última fila solo tiene `A` en las columnas
    /// pares `0, 2, .., 2(k - 1)`, y el resto `B`. Se construye con `Image::from_bytes`.
    fn comb(w: u32, h: u32, teeth: Option<u32>) -> Image {
        let (wu, hu) = (width_usize(w), width_usize(h));
        let a = [10u8, 20, 30, 255];
        let b = [40u8, 50, 60, 255];
        let mut data = Vec::with_capacity(wu * hu * 4);
        for y in 0..hu {
            for x in 0..wu {
                let es_a = match teeth {
                    _ if y == 0 => true,
                    Some(k) if y == hu - 1 => x % 2 == 0 && x / 2 < width_usize(k),
                    _ => x % 2 == 0,
                };
                data.extend_from_slice(if es_a { &a } else { &b });
            }
        }
        let img = Image::from_bytes(w, h, Mode::Rgba, data).unwrap();
        assert_eq!(img.get_pixel(0, 0), Ok(pixel_a()), "the byte order is RGBA");
        assert_eq!(img.get_pixel(1, 0), Ok(pixel_a()));
        img
    }

    fn doc_comb(w: u32, h: u32, teeth: Option<u32>) -> (Document, ImageId) {
        let doc = Document::new(comb(w, h, teeth));
        let id = ImageId::from_raw(0);
        (doc, id)
    }

    fn spans_of_the(result: Result<Option<Command>, DocumentError>) -> Vec<Span> {
        match result {
            Ok(Some(Command::FillSpans { spans, .. })) => spans,
            other => panic!("Ok(Some(FillSpans)) was expected, got {other:?}"),
        }
    }

    /// `apply` del comando da `Ok(true)` y el paso cabe en el límite por defecto de la historia.
    /// Si `cycle`, `undo` restaura la imagen original y `redo` la rellena otra vez.
    fn applies_and_checks(doc: &mut Document, id: ImageId, command: Command, cycle: bool) {
        let original = doc.image(id).unwrap().clone();
        assert_eq!(doc.apply(command), Ok(true));
        assert!(
            doc.history_bytes() <= Document::DEFAULT_HISTORY_LIMIT,
            "the step takes {} bytes and the default limit is {}",
            doc.history_bytes(),
            Document::DEFAULT_HISTORY_LIMIT
        );
        if cycle {
            let fills = doc.image(id).unwrap().clone();
            assert!(fills != original, "the fill changes the image");
            assert_eq!(fills.get_pixel(0, 0), Ok(pixel_c()));
            assert_eq!(fills.get_pixel(1, 0), Ok(pixel_c()));
            assert_eq!(
                fills.get_pixel(1, 1),
                Ok(pixel_b()),
                "the wall does not change"
            );
            assert_eq!(doc.undo(), Ok(()));
            assert!(doc.image(id) == Some(&original), "undo restores the image");
            assert_eq!(doc.redo(), Ok(()));
            assert!(doc.image(id) == Some(&fills), "redo fills it again");
        }
    }

    // Fila 1.

    #[test]
    fn row_01_comb_5x3_exactly_at_the_cap_and_one_below() {
        let doc = doc_matrix(&["AAAAA", "ABABA", "ABABA"]);
        let id = ImageId::from_raw(0);
        let expected_2_2 = [
            sp(0, 0, 5),
            sp(1, 0, 1),
            sp(1, 2, 3),
            sp(1, 4, 5),
            sp(2, 0, 1),
            sp(2, 2, 3),
            sp(2, 4, 5),
        ];
        assert_eq!(
            fill_command_with_limit(&doc, id, 0, 0, pixel_c(), 7),
            command(id, &expected_2_2, pixel_c()),
            "7 spans with cap 7"
        );
        assert_eq!(
            fill_command_with_limit(&doc, id, 4, 2, pixel_c(), 7),
            command(id, &expected_2_2, pixel_c()),
            "otra semilla, mismo resultado"
        );
        assert_eq!(
            fill_command_with_limit(&doc, id, 0, 0, pixel_c(), 6),
            Err(too_much(6))
        );
        assert_eq!(
            fill_command_with_limit(&doc, id, 4, 2, pixel_c(), 6),
            Err(too_much(6))
        );
    }

    // Fila 2.

    #[test]
    fn row_02_cap_zero_gives_error_and_the_board_fits_in_one() {
        let doc = doc_matrix(&["AAA"]);
        let id = ImageId::from_raw(0);
        assert_eq!(
            fill_command_with_limit(&doc, id, 1, 0, pixel_c(), 0),
            Err(too_much(0))
        );
        // Con tope 1 el mismo relleno (1 tramo) sí cabe.
        assert_eq!(
            fill_command_with_limit(&doc, id, 1, 0, pixel_c(), 1),
            command(id, &[sp(0, 0, 3)], pixel_c())
        );

        // Tablero de 64 x 64 (`A` si x + y es par), semilla en el centro: la región es un píxel.
        let board = Document::new(image_of(64, 64, |x, y| {
            if (x + y) % 2 == 0 {
                pixel_a()
            } else {
                pixel_b()
            }
        }));
        let id = ImageId::from_raw(0);
        assert_eq!(
            fill_command_with_limit(&board, id, 32, 32, pixel_c(), 1),
            command(id, &[sp(32, 32, 33)], pixel_c())
        );
    }

    // Fila 3.

    #[test]
    fn row_03_with_cap_zero_the_cap_is_checked_last() {
        let doc = doc_matrix(&["AAA", "AAA"]);
        let id = ImageId::from_raw(0);
        let before = snapshot(&doc);

        let unknown = ImageId::from_raw(99);
        assert_eq!(
            fill_command_with_limit(&doc, unknown, 0, 0, pixel_c(), 0),
            Err(DocumentError::UnknownImage(unknown))
        );
        let result = fill_command_with_limit(&doc, id, 0, 0, Pixel::Indexed(0), 0);
        assert!(
            matches!(result, Err(DocumentError::Pixel { pixel: 0, .. })),
            "got {result:?}"
        );
        assert_eq!(
            fill_command_with_limit(&doc, id, -1, 0, pixel_c(), 0),
            Ok(None)
        );
        assert_eq!(
            fill_command_with_limit(&doc, id, MIN, MAX, pixel_c(), 0),
            Ok(None)
        );
        assert_eq!(
            fill_command_with_limit(&doc, id, 0, 0, pixel_a(), 0),
            Ok(None)
        );
        // Y el tope, si hay algo que rellenar.
        assert_eq!(
            fill_command_with_limit(&doc, id, 0, 0, pixel_c(), 0),
            Err(too_much(0))
        );
        assert_eq!(snapshot(&doc), before, "no call touches the document");
    }

    // Fila 4.

    #[test]
    fn row_04_flood_fill_with_the_real_cap_touches_neither_document_nor_history() {
        let (mut doc, id) = doc_comb(16384, 368, None);
        // Un paso de deshacer y uno de rehacer, sobre paredes (columnas impares): no cambian
        // los tramos del peine.
        for x in [1, 3] {
            let step = Command::SetPixels {
                image: id,
                pixels: vec![editing(x, 5, pixel_c())],
            };
            assert_eq!(doc.apply(step), Ok(true));
        }
        assert_eq!(doc.undo(), Ok(()));
        assert!(doc.can_undo() && doc.can_redo());

        let image_2 = doc.image(id).unwrap().clone();
        let (undo, redo, bytes, state) = (
            doc.can_undo(),
            doc.can_redo(),
            doc.history_bytes(),
            doc.state_id(),
        );

        let result = flood_fill(&mut doc, id, 0, 0, pixel_c());
        assert!(
            matches!(
                result,
                Err(DocumentError::FillTooFragmented {
                    limit: Document::MAX_FILL_SPANS
                })
            ),
            "got {result:?}"
        );
        assert!(doc.image(id) == Some(&image_2), "the image does not change");
        assert_eq!(doc.can_undo(), undo);
        assert_eq!(doc.can_redo(), redo, "the redo stack is still alive");
        assert_eq!(doc.history_bytes(), bytes);
        assert_eq!(doc.state_id(), state, "apply was not called");
    }

    // Fila 5.

    #[test]
    fn row_05_comb_of_368_rows_exceeds_the_real_cap() {
        // S = 1 + 367 * 8192 = 3 006 465.
        let (doc, id) = doc_comb(16384, 368, None);
        assert_eq!(
            fill_command(&doc, id, 0, 0, pixel_c()),
            Err(too_much(CAP)),
            "the error carries the real cap"
        );
    }

    #[test]
    fn row_05_comb_of_367_rows_fits_and_its_step_fits_in_the_history() {
        // S = 1 + 366 * 8192 = 2 998 273.
        let (mut doc, id) = doc_comb(16384, 367, None);
        let command = fill_command(&doc, id, 0, 0, pixel_c());
        let n = spans_of_the(command.clone()).len();
        assert_eq!(n, 2_998_273);
        let command = command.unwrap().unwrap();
        applies_and_checks(&mut doc, id, command, true);
    }

    // Fila 6 (la mitad de px-raster: la constante; el coste real del paso máximo va en px-core).

    #[test]
    fn row_06_the_cap_constant_is_three_million() {
        assert_eq!(Document::MAX_FILL_SPANS, 3_000_000);
        assert_eq!(Document::MAX_FILL_SPANS, CAP);
    }

    // Fila 15: justo por debajo, en y por encima del tope real. Peine de 16384 x 368 cuya última
    // fila solo tiene `k` dientes: S = 2 998 273 + k.

    #[test]
    fn row_15_k_1726_one_span_below_the_cap() {
        let (mut doc, id) = doc_comb(16384, 368, Some(1726));
        let command = fill_command(&doc, id, 0, 0, pixel_c());
        assert_eq!(spans_of_the(command.clone()).len(), 2_999_999);
        applies_and_checks(&mut doc, id, command.unwrap().unwrap(), false);
    }

    #[test]
    fn row_15_k_1727_exactly_at_the_cap() {
        let (mut doc, id) = doc_comb(16384, 368, Some(1727));
        let command = fill_command(&doc, id, 0, 0, pixel_c());
        assert_eq!(spans_of_the(command.clone()).len(), 3_000_000);
        applies_and_checks(&mut doc, id, command.unwrap().unwrap(), false);
    }

    #[test]
    fn row_15_k_1728_one_span_above_the_cap() {
        let (doc, id) = doc_comb(16384, 368, Some(1728));
        assert_eq!(
            fill_command(&doc, id, 0, 0, pixel_c()),
            Err(too_much(CAP)),
            "S = 3 000 001"
        );
    }

    // Apoyo de las propiedades.

    /// Número de tramos máximos (por fila) de una región.
    fn spans_maximums(region: &BTreeSet<(u32, u32)>) -> usize {
        region
            .iter()
            .filter(|&&(x, y)| x == 0 || !region.contains(&(x - 1, y)))
            .count()
    }

    /// Topes pequeños con los extremos siempre presentes.
    fn cap_any() -> impl Strategy<Value = usize> {
        prop_oneof![
            6 => 0usize..=40,
            1 => Just(0usize),
            1 => Just(usize::MAX),
            1 => any::<usize>(),
        ]
    }

    proptest! {
        // Propiedad 1: oráculo del tope.
        #[test]
        fn cap_equal_to_the_oracle(case in cases(), l in 0usize..=40) {
            let (doc, id) = case.document();
            let (x, y) = case.seed_i_32();
            let value = case.value();
            let region = region_naive(&case.image, case.seed.0, case.seed.1);
            let s = spans_maximums(&region);

            let with_cap = fill_command_with_limit(&doc, id, x, y, case.pixel, l);
            let without_cap = fill_command(&doc, id, x, y, case.pixel);
            if case.pixel == value {
                prop_assert_eq!(with_cap, Ok(None));
            } else if s > l {
                prop_assert_eq!(with_cap, Err(DocumentError::FillTooFragmented { limit: l }));
            } else {
                // Los mismos tramos que sin tope, y exactamente `s`.
                prop_assert_eq!(&with_cap, &without_cap);
                let Ok(Some(Command::FillSpans { spans, .. })) = with_cap else {
                    return Err(TestCaseError::fail(format!("salió {with_cap:?}")));
                };
                prop_assert_eq!(spans.len(), s);
                prop_assert!(spans.len() <= l);
                prop_assert_eq!(spans.capacity(), spans.len());
            }
        }

        // Propiedad 2: atomicidad y orden de las comprobaciones.
        #[test]
        fn cap_atomicity_and_order(
            case in cases(),
            l in cap_any(),
            seed_of_the_case in any::<bool>(),
            (xr, yr) in (coordinate(), coordinate()),
            other_pixel in pixel_any(),
            uses_other_pixel in any::<bool>(),
            foreign in any::<bool>(),
            raw in any::<u32>(),
        ) {
            let (mut doc, main) = case.document();
            step_previous(&mut doc);
            let (x, y) = if seed_of_the_case { case.seed_i_32() } else { (xr, yr) };
            let pixel = if uses_other_pixel { other_pixel } else { case.pixel };
            let id = if foreign { ImageId::from_raw(raw) } else { main };
            let (w, h) = (i64::from(case.image.width()), i64::from(case.image.height()));
            let outside = x < 0 || y < 0 || i64::from(x) >= w || i64::from(y) >= h;

            let before = snapshot(&doc);
            let image_preview = doc.image(id).cloned();
            let c = fill_command_with_limit(&doc, id, x, y, pixel, l);
            // Con cualquier resultado, `fill_command_with_limit` solo lee.
            prop_assert_eq!(snapshot(&doc), before.clone());

            // UnknownImage > Pixel > fuera > igual > tope.
            let Some(img) = image_preview else {
                prop_assert_eq!(c, Err(DocumentError::UnknownImage(id)));
                return Ok(());
            };
            if let Some(e) = error_of_apply(&img, pixel) {
                prop_assert!(
                    matches!(&e, DocumentError::Pixel { pixel: 0, .. }),
                    "apply should give Pixel {{ pixel: 0, .. }}, gave {:?}", e
                );
                prop_assert_eq!(c, Err(e));
                return Ok(());
            }
            if outside {
                prop_assert_eq!(c, Ok(None));
                return Ok(());
            }
            let (ux, uy) = (u32::try_from(x).unwrap(), u32::try_from(y).unwrap());
            if img.get_pixel(ux, uy).unwrap() == pixel {
                prop_assert_eq!(c, Ok(None));
                return Ok(());
            }
            let s = spans_maximums(&region_naive(&img, ux, uy));
            if s > l {
                prop_assert_eq!(c, Err(DocumentError::FillTooFragmented { limit: l }));
            } else {
                prop_assert!(
                    matches!(c, Ok(Some(Command::FillSpans { .. }))),
                    "Some(FillSpans) was expected, got {:?}", c
                );
            }
        }

        // Propiedad 3: monotonía del tope.
        #[test]
        fn cap_monotonic(case in cases(), a in cap_any(), b in cap_any()) {
            let (doc, id) = case.document();
            let (x, y) = case.seed_i_32();
            let (smaller, greater) = (a.min(b), a.max(b));
            let with_smaller = fill_command_with_limit(&doc, id, x, y, case.pixel, smaller);
            let with_greater = fill_command_with_limit(&doc, id, x, y, case.pixel, greater);
            if with_smaller.is_ok() {
                // Ok(x) con L implica Ok(x) con cualquier L' >= L.
                prop_assert_eq!(&with_greater, &with_smaller);
            }
            if with_greater.is_err() {
                // Err con L implica Err con cualquier L' < L, y cada uno con su tope.
                prop_assert_eq!(
                    with_smaller,
                    Err(DocumentError::FillTooFragmented { limit: smaller })
                );
                prop_assert_eq!(
                    with_greater,
                    Err(DocumentError::FillTooFragmented { limit: greater })
                );
            }
        }
    }
}

#[path = "fill/unit_4_5.rs"]
mod unit_4_5;
