//! Tests de `FillSpans` y de la historia por regiones (specs/hito-2-historia-regiones.md).
//!
//! Una prueba unitaria por cada fila de «Casos límite» (`fila_NN_...`) y una propiedad por
//! cada una de las seis de «Tests». Las imágenes miden como mucho 6 x 6 (salvo las dos de
//! la fila 14, de 4000 x 3 como mucho). El coste de la historia solo se mide con
//! `history_bytes()` y con comparaciones relativas: nunca se asume el valor de `size_of`.

use std::error::Error;
use std::fmt::{Debug, Display};

use proptest::prelude::*;
use proptest::test_runner::TestCaseError;
use px_core::{
    Command, Document, DocumentError, Image, ImageError, ImageId, Mode, Palette, Pixel, PixelEdit,
    Rgba, Span,
};

// Comprobaciones en tiempo de compilación de los traits exigidos.
fn assert_span_traits<T: Clone + Copy + Debug + PartialEq + Eq>() {}
fn assert_datum_traits<T: Clone + Debug + PartialEq + Eq>() {}
fn assert_error_traits<T: Debug + Clone + PartialEq + Eq + Display + Error>() {}

// Colores, imágenes, documentos y comandos de apoyo.

const ALL_2: [Mode; 4] = [Mode::Rgba, Mode::Indexed, Mode::Gray, Mode::GrayAlpha];

/// Valor inicial (transparente) de la imagen de `doc_d`.
fn c0() -> Rgba {
    Rgba::TRANSPARENT
}

fn c1() -> Rgba {
    Rgba::new(40, 50, 60, 255)
}

fn c2() -> Rgba {
    Rgba::new(70, 80, 90, 255)
}

fn c3() -> Rgba {
    Rgba::new(100, 110, 120, 255)
}

fn palette_of(n: usize) -> Palette {
    Palette::from_colors((0..n).map(|i| Rgba::new(i as u8, 255 - i as u8, 7, 255))).unwrap()
}

/// Crea una imagen de cualquier modo; el indexado usa una paleta de `n` entradas.
fn creates(mode: Mode, width: u32, height: u32, n: usize) -> Image {
    match mode {
        Mode::Indexed => Image::new_indexed(width, height, palette_of(n)).unwrap(),
        _ => Image::new(width, height, mode).unwrap(),
    }
}

/// `D` de la spec: documento de una imagen RGBA 4 x 3 recién creado.
fn doc_d() -> Document {
    Document::new(Image::new(4, 3, Mode::Rgba).unwrap())
}

/// Documento de una imagen indexada de 4 x 3 con paleta de 2 entradas.
fn doc_indexed() -> Document {
    Document::new(Image::new_indexed(4, 3, palette_of(2)).unwrap())
}

fn px(x: u32, y: u32, c: Rgba) -> PixelEdit {
    PixelEdit {
        x,
        y,
        pixel: Pixel::Rgba(c),
    }
}

/// `S(...)` de la spec.
fn set(image: ImageId, pixels: &[PixelEdit]) -> Command {
    Command::SetPixels {
        image,
        pixels: pixels.to_vec(),
    }
}

/// `sp(y, a, b)` de la spec.
fn sp(y: u32, a: u32, b: u32) -> Span {
    Span {
        y,
        x_start: a,
        x_end: b,
    }
}

/// `FillSpans` con cualquier `Pixel`.
fn fill(image: ImageId, spans: &[Span], pixel: Pixel) -> Command {
    Command::FillSpans {
        image,
        spans: spans.to_vec(),
        pixel,
    }
}

/// `F(spans, c)` de la spec con un color RGBA.
fn fr(image: ImageId, spans: &[Span], c: Rgba) -> Command {
    fill(image, spans, Pixel::Rgba(c))
}

fn batch(commands: Vec<Command>) -> Command {
    Command::Batch(commands)
}

fn pixel_en(doc: &Document, x: u32, y: u32) -> Pixel {
    image_of(doc).get_pixel(x, y).unwrap()
}

fn image_of(doc: &Document) -> &Image {
    doc.image(ImageId::from_raw(0)).unwrap()
}

fn outside(x: u32, y: u32, width: u32, height: u32) -> ImageError {
    ImageError::OutOfBounds {
        x,
        y,
        width,
        height,
    }
}

fn failure_pixel(pixel: usize, source: ImageError) -> DocumentError {
    DocumentError::Pixel { pixel, source }
}

fn failure_span(span: usize, source: ImageError) -> DocumentError {
    DocumentError::Span { span, source }
}

fn in_batch(index: usize, source: DocumentError) -> DocumentError {
    DocumentError::InBatch {
        index,
        source: Box::new(source),
    }
}

/// Comprueba cada píxel de la imagen RGBA principal: los de `listed` (si un píxel
/// aparece varias veces, vale el último) y todos los demás valen `rest`.
fn checks_image(doc: &Document, listed: &[(u32, u32, Rgba)], rest: Rgba) {
    let img = image_of(doc);
    for y in 0..img.height() {
        for x in 0..img.width() {
            let expected = listed
                .iter()
                .rev()
                .find(|&&(lx, ly, _)| lx == x && ly == y)
                .map_or(rest, |&(_, _, c)| c);
            assert_eq!(img.get_pixel(x, y), Ok(Pixel::Rgba(expected)), "({x}, {y})");
        }
    }
}

/// Cota para que un `undo`/`redo` defectuoso que nunca falla no cuelgue los tests.
const CAP_STEPS: usize = 100_000;

/// Número de `undo()` que admite el documento (se cuenta en una copia).
fn steps_undoable(doc: &Document) -> usize {
    let mut copy = doc.clone();
    let mut n = 0;
    while n < CAP_STEPS && copy.undo().is_ok() {
        n += 1;
    }
    n
}

/// Número de `redo()` que admite el documento (se cuenta en una copia).
fn steps_redoable(doc: &Document) -> usize {
    let mut copy = doc.clone();
    let mut n = 0;
    while n < CAP_STEPS && copy.redo().is_ok() {
        n += 1;
    }
    n
}

/// Todo lo observable de un documento, para comprobar que una operación no tocó nada.
#[derive(Debug, PartialEq, Eq)]
struct State {
    doc: Document,
    can_undo: bool,
    can_redo: bool,
    bytes: usize,
    limit: usize,
    undoable: usize,
    redoable: usize,
}

fn state(doc: &Document) -> State {
    State {
        doc: doc.clone(),
        can_undo: doc.can_undo(),
        can_redo: doc.can_redo(),
        bytes: doc.history_bytes(),
        limit: doc.history_limit(),
        undoable: steps_undoable(doc),
        redoable: steps_redoable(doc),
    }
}

/// `apply(cmd)` da exactamente `expected` y no toca nada observable.
fn rejects(doc: &mut Document, cmd: Command, expected: DocumentError) {
    let antes = state(doc);
    assert_eq!(doc.apply(cmd.clone()), Err(expected), "{cmd:?}");
    assert_eq!(state(doc), antes, "{cmd:?}");
}

/// `apply(cmd)` da `Ok(false)` y no toca nada observable.
fn unchanged(doc: &mut Document, cmd: Command) {
    let antes = state(doc);
    assert_eq!(doc.apply(cmd.clone()), Ok(false), "{cmd:?}");
    assert_eq!(state(doc), antes, "{cmd:?}");
}

/// Coste de la historia tras un relleno de una fila entera de `D` (un solo trozo).
fn cost_of_a_fill(y: u32) -> usize {
    let mut doc = doc_d();
    let id = ImageId::from_raw(0);
    assert_eq!(doc.apply(fr(id, &[sp(y, 0, 4)], c1())), Ok(true));
    doc.history_bytes()
}

// Tipos nuevos de la API.

#[test]
fn derived_traits_of_span_and_commands() {
    assert_span_traits::<Span>();
    assert_datum_traits::<Command>();
    assert_datum_traits::<Document>();
    assert_error_traits::<DocumentError>();

    // Campos públicos y comparación.
    let t = Span {
        y: 1,
        x_start: 2,
        x_end: 3,
    };
    assert_eq!(t, sp(1, 2, 3));
    assert_ne!(t, sp(1, 2, 4));
    assert_ne!(t, sp(0, 2, 3));
    assert_ne!(t, sp(1, 1, 3));
    let copy = t;
    assert_eq!(copy, t);

    let id = ImageId::from_raw(0);
    let a = fr(id, &[sp(0, 0, 4)], c1());
    assert_eq!(a, a.clone());
    assert_ne!(a, fr(id, &[sp(0, 0, 3)], c1()));
    assert_ne!(a, fr(id, &[sp(0, 0, 4)], c2()));
    assert_ne!(a, fr(ImageId::from_raw(1), &[sp(0, 0, 4)], c1()));
    assert_ne!(a, set(id, &[px(0, 0, c1())]));
}

#[test]
fn errors_new_have_message() {
    let new_2_2 = [
        failure_span(0, outside(9, 9, 4, 3)),
        DocumentError::BadSpans { span: 3 },
        in_batch(1, DocumentError::BadSpans { span: 0 }),
        in_batch(1, failure_span(2, outside(9, 9, 4, 3))),
    ];
    for e in &new_2_2 {
        assert!(!e.to_string().is_empty(), "{e:?} sin mensaje");
    }
    assert_ne!(
        DocumentError::BadSpans { span: 0 },
        DocumentError::BadSpans { span: 1 }
    );
    assert_ne!(
        failure_span(0, outside(9, 9, 4, 3)),
        failure_span(1, outside(9, 9, 4, 3))
    );
}

// Una prueba por fila de la tabla de casos límite.

#[test]
fn row_01_a_whole_row_with_undo_and_redo() {
    let mut doc = doc_d();
    let initial = doc.clone();
    let id = ImageId::from_raw(0);

    assert_eq!(doc.apply(fr(id, &[sp(0, 0, 4)], c1())), Ok(true));
    checks_image(
        &doc,
        &[(0, 0, c1()), (1, 0, c1()), (2, 0, c1()), (3, 0, c1())],
        c0(),
    );
    assert!(doc.can_undo());
    assert!(!doc.can_redo());
    assert_eq!(steps_undoable(&doc), 1);
    let after_apply = doc.clone();

    assert_eq!(doc.undo(), Ok(()));
    assert_eq!(doc, initial);
    checks_image(&doc, &[], c0());
    assert!(doc.can_redo());
    assert!(!doc.can_undo());

    assert_eq!(doc.redo(), Ok(()));
    assert_eq!(doc, after_apply);
    assert!(doc.can_undo());
    assert!(!doc.can_redo());
}

#[test]
fn row_02_spans_in_several_rows_and_adjacent() {
    let mut doc = doc_d();
    let initial = doc.clone();
    let id = ImageId::from_raw(0);
    let spans = [sp(0, 1, 3), sp(1, 0, 2), sp(1, 2, 4), sp(2, 3, 4)];

    assert_eq!(doc.apply(fr(id, &spans, c1())), Ok(true));
    checks_image(
        &doc,
        &[
            (1, 0, c1()),
            (2, 0, c1()),
            (0, 1, c1()),
            (1, 1, c1()),
            (2, 1, c1()),
            (3, 1, c1()),
            (3, 2, c1()),
        ],
        c0(),
    );
    assert_eq!(steps_undoable(&doc), 1);
    let after_apply = doc.clone();

    assert_eq!(doc.undo(), Ok(()));
    assert_eq!(doc, initial);
    assert!(!doc.can_undo());

    assert_eq!(doc.redo(), Ok(()));
    assert_eq!(doc, after_apply);
}

#[test]
fn row_03_the_different_previous_value_is_restored() {
    let mut doc = doc_d();
    let initial = doc.clone();
    let id = ImageId::from_raw(0);

    assert_eq!(doc.apply(set(id, &[px(1, 0, c2())])), Ok(true));
    let with_a = doc.clone();
    assert_eq!(doc.apply(fr(id, &[sp(0, 0, 4)], c1())), Ok(true));
    checks_image(
        &doc,
        &[(0, 0, c1()), (1, 0, c1()), (2, 0, c1()), (3, 0, c1())],
        c0(),
    );
    assert_eq!(steps_undoable(&doc), 2);
    let after_fill = doc.clone();

    assert_eq!(doc.undo(), Ok(()));
    assert_eq!(pixel_en(&doc, 1, 0), Pixel::Rgba(c2()));
    checks_image(&doc, &[(1, 0, c2())], c0());
    assert_eq!(doc, with_a);

    assert_eq!(doc.redo(), Ok(()));
    assert_eq!(doc, after_fill);

    assert_eq!(doc.undo(), Ok(()));
    assert_eq!(doc.undo(), Ok(()));
    assert_eq!(doc, initial);
}

#[test]
fn row_04_pixels_that_already_had_the_color_are_not_touched() {
    let mut doc = doc_d();
    let initial = doc.clone();
    let id = ImageId::from_raw(0);

    assert_eq!(doc.apply(set(id, &[px(1, 0, c1())])), Ok(true));
    let with_a = doc.clone();
    assert_eq!(doc.apply(fr(id, &[sp(0, 0, 4)], c1())), Ok(true));
    checks_image(
        &doc,
        &[(0, 0, c1()), (1, 0, c1()), (2, 0, c1()), (3, 0, c1())],
        c0(),
    );
    let after_fill = doc.clone();

    assert_eq!(doc.undo(), Ok(()));
    checks_image(&doc, &[(1, 0, c1())], c0());
    assert_eq!(doc, with_a);

    assert_eq!(doc.redo(), Ok(()));
    assert_eq!(doc, after_fill);

    assert_eq!(doc.undo(), Ok(()));
    assert_eq!(doc.undo(), Ok(()));
    assert_eq!(doc, initial);
}

#[test]
fn row_05_no_changes_is_ok_false() {
    // En D: rellenar con el valor que ya hay.
    let mut doc = doc_d();
    let id = ImageId::from_raw(0);
    unchanged(&mut doc, fr(id, &[sp(0, 0, 4)], c0()));
    assert!(!doc.can_undo());
    assert!(!doc.can_redo());
    assert_eq!(doc.history_bytes(), 0);

    // Varios tramos, de varias filas, todos ya a c0.
    unchanged(&mut doc, fr(id, &[sp(0, 1, 3), sp(2, 0, 4)], c0()));
    assert_eq!(doc.history_bytes(), 0);

    // Tras un `undo`, la pila de rehacer se conserva.
    let mut doc = doc_d();
    assert_eq!(doc.apply(set(id, &[px(0, 0, c1())])), Ok(true));
    assert_eq!(doc.undo(), Ok(()));
    assert!(doc.can_redo());
    let antes = state(&doc);
    assert_eq!(
        doc.apply(fr(id, &[sp(0, 0, 4)], c0())),
        Ok(false),
        "relleno sin cambios"
    );
    assert!(doc.can_redo());
    assert_eq!(doc.can_undo(), antes.can_undo);
    assert_eq!(doc.history_bytes(), antes.bytes);
    assert_eq!(state(&doc), antes);
    assert_eq!(doc.redo(), Ok(()));
    assert_eq!(pixel_en(&doc, 0, 0), Pixel::Rgba(c1()));
}

#[test]
fn row_06_empty_list_unknown_image_and_invalid_pixel() {
    let mut doc = doc_d();
    let id = ImageId::from_raw(0);

    rejects(&mut doc, fr(id, &[], c1()), DocumentError::EmptyCommand);

    let unknown = ImageId::from_raw(99);
    rejects(
        &mut doc,
        Command::FillSpans {
            image: unknown,
            spans: vec![sp(0, 0, 4)],
            pixel: Pixel::Rgba(c1()),
        },
        DocumentError::UnknownImage(unknown),
    );

    rejects(
        &mut doc,
        fill(id, &[sp(0, 0, 4)], Pixel::Gray(1)),
        failure_pixel(
            0,
            ImageError::ModeMismatch {
                expected: Mode::Rgba,
                found: Mode::Gray,
            },
        ),
    );

    // Indexed(9) en una imagen indexada con paleta de 2.
    let mut doc = doc_indexed();
    let id = ImageId::from_raw(0);
    rejects(
        &mut doc,
        fill(id, &[sp(0, 0, 4)], Pixel::Indexed(9)),
        failure_pixel(0, ImageError::IndexNotInPalette { index: 9, len: 2 }),
    );
}

#[test]
fn check_order_of_fill_spans() {
    let mut doc = doc_d();
    let id = ImageId::from_raw(0);
    let unknown = ImageId::from_raw(99);
    let bad = [sp(9, 5, 2), sp(0, 0, 1)];

    // EmptyCommand gana a UnknownImage (y al píxel).
    rejects(
        &mut doc,
        fill(unknown, &[], Pixel::Gray(1)),
        DocumentError::EmptyCommand,
    );
    // UnknownImage gana al píxel y a los tramos.
    rejects(
        &mut doc,
        fill(unknown, &bad, Pixel::Gray(1)),
        DocumentError::UnknownImage(unknown),
    );
    // El píxel gana a los tramos (vacío, desordenado y fuera del lienzo).
    rejects(
        &mut doc,
        fill(id, &bad, Pixel::Gray(1)),
        failure_pixel(
            0,
            ImageError::ModeMismatch {
                expected: Mode::Rgba,
                found: Mode::Gray,
            },
        ),
    );
    // Con el píxel bien, el tramo 0 vacío (que además se sale) es `BadSpans`.
    rejects(
        &mut doc,
        fr(id, &bad, c1()),
        DocumentError::BadSpans { span: 0 },
    );

    // En un documento indexado, un índice fuera de la paleta gana a los tramos malos.
    let mut doc = doc_indexed();
    let id = ImageId::from_raw(0);
    rejects(
        &mut doc,
        fill(id, &bad, Pixel::Indexed(2)),
        failure_pixel(0, ImageError::IndexNotInPalette { index: 2, len: 2 }),
    );
}

#[test]
fn row_07_span_outside_the_canvas() {
    let cases = [
        // y >= height: primer píxel fuera (x_start, y).
        (vec![sp(3, 0, 1)], 0usize, outside(0, 3, 4, 3)),
        // x_end > width: (max(x_start, width), y).
        (vec![sp(0, 2, 5)], 0, outside(4, 0, 4, 3)),
        // x_start y x_end fuera.
        (vec![sp(1, 5, 7)], 0, outside(5, 1, 4, 3)),
        // El fallo está en el segundo tramo.
        (vec![sp(0, 0, 2), sp(1, 3, 9)], 1, outside(4, 1, 4, 3)),
        // y fuera con x_start grande: sigue siendo (x_start, y), no el máximo con width.
        (vec![sp(5, 9, 10)], 0, outside(9, 5, 4, 3)),
        // Valores extremos, sin desbordar ni hacer pánico.
        (
            vec![sp(u32::MAX, u32::MAX - 1, u32::MAX)],
            0,
            outside(u32::MAX - 1, u32::MAX, 4, 3),
        ),
        (vec![sp(0, 0, u32::MAX)], 0, outside(4, 0, 4, 3)),
        (vec![sp(2, 3, u32::MAX)], 0, outside(4, 2, 4, 3)),
    ];
    for (spans, n, failure) in cases {
        let mut doc = doc_d();
        let id = ImageId::from_raw(0);
        rejects(&mut doc, fr(id, &spans, c1()), failure_span(n, failure));
        // Atomicidad: los tramos anteriores al defectuoso no se escribieron.
        checks_image(&doc, &[], c0());
        assert!(!doc.can_undo(), "{spans:?}");
        assert_eq!(doc.history_bytes(), 0, "{spans:?}");
    }
}

#[test]
fn row_07_outside_span_leaves_existing_history_intact() {
    let mut doc = doc_d();
    let id = ImageId::from_raw(0);
    assert_eq!(doc.apply(set(id, &[px(3, 2, c2())])), Ok(true));
    assert_eq!(doc.apply(set(id, &[px(2, 2, c3())])), Ok(true));
    assert_eq!(doc.undo(), Ok(()));
    assert!(doc.can_undo() && doc.can_redo());

    rejects(
        &mut doc,
        fr(id, &[sp(0, 0, 2), sp(1, 3, 9)], c1()),
        failure_span(1, outside(4, 1, 4, 3)),
    );
    assert!(doc.can_redo());
}

#[test]
fn row_08_empty_or_inverted_spans() {
    for spans in [vec![sp(0, 2, 2)], vec![sp(0, 3, 1)]] {
        let mut doc = doc_d();
        let id = ImageId::from_raw(0);
        rejects(
            &mut doc,
            fr(id, &spans, c1()),
            DocumentError::BadSpans { span: 0 },
        );
        checks_image(&doc, &[], c0());
        assert!(!doc.can_undo());
    }
    // Un tramo vacío tampoco se tolera en medio de una lista válida.
    let mut doc = doc_d();
    let id = ImageId::from_raw(0);
    rejects(
        &mut doc,
        fr(id, &[sp(0, 0, 2), sp(1, 1, 1), sp(2, 0, 1)], c1()),
        DocumentError::BadSpans { span: 1 },
    );
    checks_image(&doc, &[], c0());
}

#[test]
fn row_09_unordered_overlapping_or_repeated_spans() {
    let cases = [
        vec![sp(1, 0, 2), sp(0, 0, 2)],
        vec![sp(0, 0, 3), sp(0, 2, 4)],
        vec![sp(0, 0, 2), sp(0, 0, 2)],
    ];
    for spans in cases {
        let mut doc = doc_d();
        let id = ImageId::from_raw(0);
        rejects(
            &mut doc,
            fr(id, &spans, c1()),
            DocumentError::BadSpans { span: 1 },
        );
        checks_image(&doc, &[], c0());
        assert!(!doc.can_undo(), "{spans:?}");
    }
    // Desorden dentro de la misma fila: el segundo empieza antes del fin del primero.
    let mut doc = doc_d();
    let id = ImageId::from_raw(0);
    rejects(
        &mut doc,
        fr(id, &[sp(0, 2, 4), sp(0, 0, 1)], c1()),
        DocumentError::BadSpans { span: 1 },
    );
    // El desorden se detecta también en el tercer tramo.
    rejects(
        &mut doc,
        fr(id, &[sp(0, 0, 1), sp(1, 0, 1), sp(0, 2, 3)], c1()),
        DocumentError::BadSpans { span: 2 },
    );
}

#[test]
fn row_10_valid_pastes_and_error_order() {
    // Dos tramos pegados son válidos.
    let mut doc = doc_d();
    let initial = doc.clone();
    let id = ImageId::from_raw(0);
    assert_eq!(
        doc.apply(fr(id, &[sp(0, 0, 2), sp(0, 2, 4)], c1())),
        Ok(true)
    );
    checks_image(
        &doc,
        &[(0, 0, c1()), (1, 0, c1()), (2, 0, c1()), (3, 0, c1())],
        c0(),
    );
    assert_eq!(doc.undo(), Ok(()));
    assert_eq!(doc, initial);

    // Gana el primer tramo defectuoso, aunque el siguiente también lo sea.
    let mut doc = doc_d();
    let id = ImageId::from_raw(0);
    rejects(
        &mut doc,
        fr(id, &[sp(1, 0, 9), sp(0, 0, 1)], c1()),
        failure_span(0, outside(4, 1, 4, 3)),
    );
    // El tramo 1 solapa y además se sale: `BadSpans` antes que `Span`.
    rejects(
        &mut doc,
        fr(id, &[sp(0, 0, 2), sp(0, 1, 9)], c1()),
        DocumentError::BadSpans { span: 1 },
    );
    checks_image(&doc, &[], c0());
}

#[test]
fn row_11_mixed_batch_is_one_step() {
    // (x, color) del píxel suelto `a` (antes del relleno) y del `b` (después).
    let variants = [
        ((1, c2()), (2, c3())),
        ((1, c2()), (1, c3())),
        ((0, c2()), (3, c2())),
    ];
    for ((ax, ac), (bx, bc)) in variants {
        let mut doc = doc_d();
        let initial = doc.clone();
        let id = ImageId::from_raw(0);
        let command = batch(vec![
            set(id, &[px(ax, 0, ac)]),
            fr(id, &[sp(0, 0, 4)], c1()),
            set(id, &[px(bx, 0, bc)]),
        ]);

        assert_eq!(doc.apply(command), Ok(true));
        checks_image(
            &doc,
            &[
                (0, 0, c1()),
                (1, 0, c1()),
                (2, 0, c1()),
                (3, 0, c1()),
                (bx, 0, bc),
            ],
            c0(),
        );
        assert_eq!(steps_undoable(&doc), 1);
        let after_apply = doc.clone();

        assert_eq!(doc.undo(), Ok(()));
        assert_eq!(doc, initial, "a = ({ax}, 0), b = ({bx}, 0)");
        assert!(!doc.can_undo());
        assert!(doc.can_redo());

        assert_eq!(doc.redo(), Ok(()));
        assert_eq!(doc, after_apply, "a = ({ax}, 0), b = ({bx}, 0)");
        assert!(doc.can_undo());
        assert!(!doc.can_redo());
    }
}

#[test]
fn row_12_batch_with_two_fills() {
    let row_0 = [sp(0, 0, 4)];

    // Dos rellenos iguales: solo cambia el primero, y es un único paso.
    let mut doc = doc_d();
    let initial = doc.clone();
    let id = ImageId::from_raw(0);
    assert_eq!(
        doc.apply(batch(vec![fr(id, &row_0, c1()), fr(id, &row_0, c1())])),
        Ok(true)
    );
    let mut only_the_first = doc_d();
    assert_eq!(only_the_first.apply(fr(id, &row_0, c1())), Ok(true));
    assert_eq!(doc, only_the_first);
    assert_eq!(steps_undoable(&doc), 1);
    assert_eq!(doc.undo(), Ok(()));
    assert_eq!(doc, initial);
    assert!(!doc.can_undo());

    // Cambia y restaura: cuenta como cambio y el contenido es igual a D.
    let mut doc = doc_d();
    assert_eq!(
        doc.apply(batch(vec![fr(id, &row_0, c1()), fr(id, &row_0, c0())])),
        Ok(true)
    );
    assert_eq!(doc, initial);
    assert!(doc.can_undo());
    assert!(doc.history_bytes() > 0);
    assert_eq!(steps_undoable(&doc), 1);
    assert_eq!(doc.undo(), Ok(()));
    assert_eq!(doc, initial);
    assert!(!doc.can_undo());
    assert!(doc.can_redo());
    assert_eq!(doc.redo(), Ok(()));
    assert_eq!(doc, initial);
    assert!(doc.can_undo());
}

#[test]
fn batch_only_with_unchanged_fills_creates_no_step() {
    let mut doc = doc_d();
    let id = ImageId::from_raw(0);
    let row_0 = [sp(0, 0, 4)];
    unchanged(
        &mut doc,
        batch(vec![fr(id, &row_0, c0()), fr(id, &[sp(2, 1, 3)], c0())]),
    );
    assert_eq!(doc.history_bytes(), 0);

    // Un relleno sin cambios más un `SetPixels` sin cambios tampoco.
    unchanged(
        &mut doc,
        batch(vec![fr(id, &row_0, c0()), set(id, &[px(3, 2, c0())])]),
    );

    // Con un cambio en cualquiera de los dos, sí.
    assert_eq!(
        doc.apply(batch(vec![
            fr(id, &row_0, c0()),
            set(id, &[px(3, 2, c1())])
        ])),
        Ok(true)
    );
    assert_eq!(steps_undoable(&doc), 1);
    let mut other = doc_d();
    assert_eq!(
        other.apply(batch(vec![
            set(id, &[px(3, 2, c1())]),
            fr(id, &row_0, c0())
        ])),
        Ok(true)
    );
    assert_eq!(steps_undoable(&other), 1);
    assert_eq!(doc, other);
}

/// Documentos de partida para las pruebas de lotes erróneos: uno nuevo y otro con
/// historia y pila de rehacer.
fn documents_with_and_without_history() -> Vec<Document> {
    let new_2 = doc_d();
    let mut with_history = doc_d();
    let id = ImageId::from_raw(0);
    assert_eq!(with_history.apply(set(id, &[px(3, 2, c2())])), Ok(true));
    assert_eq!(with_history.apply(set(id, &[px(2, 2, c3())])), Ok(true));
    assert_eq!(with_history.undo(), Ok(()));
    assert!(with_history.can_undo() && with_history.can_redo());
    vec![new_2, with_history]
}

#[test]
fn row_13_batch_with_an_inner_failure_changes_nothing() {
    for mut doc in documents_with_and_without_history() {
        let id = ImageId::from_raw(0);
        let row_0 = [sp(0, 0, 4)];

        // `Span` en el comando 1; el relleno anterior se revierte.
        rejects(
            &mut doc,
            batch(vec![fr(id, &row_0, c1()), fr(id, &[sp(3, 0, 1)], c1())]),
            in_batch(1, failure_span(0, outside(0, 3, 4, 3))),
        );

        // `BadSpans` en el comando 1 tras un `SetPixels`.
        rejects(
            &mut doc,
            batch(vec![
                set(id, &[px(0, 0, c1())]),
                fr(id, &[sp(1, 0, 2), sp(0, 0, 1)], c1()),
            ]),
            in_batch(1, DocumentError::BadSpans { span: 1 }),
        );

        // Un `Batch` dentro de otro.
        rejects(
            &mut doc,
            batch(vec![
                fr(id, &row_0, c1()),
                batch(vec![set(id, &[px(1, 0, c1())])]),
            ]),
            in_batch(1, DocumentError::NestedBatch),
        );
    }
}

#[test]
fn batch_with_empty_unknown_fill_or_invalid_pixel() {
    let unknown = ImageId::from_raw(99);
    for mut doc in documents_with_and_without_history() {
        let id = ImageId::from_raw(0);
        let good = set(id, &[px(0, 0, c1())]);

        rejects(
            &mut doc,
            batch(vec![good.clone(), fr(id, &[], c1())]),
            in_batch(1, DocumentError::EmptyCommand),
        );
        rejects(
            &mut doc,
            batch(vec![good.clone(), fr(unknown, &[sp(0, 0, 1)], c1())]),
            in_batch(1, DocumentError::UnknownImage(unknown)),
        );
        rejects(
            &mut doc,
            batch(vec![good.clone(), fill(id, &[sp(0, 0, 1)], Pixel::Gray(1))]),
            in_batch(
                1,
                failure_pixel(
                    0,
                    ImageError::ModeMismatch {
                        expected: Mode::Rgba,
                        found: Mode::Gray,
                    },
                ),
            ),
        );
        // El fallo en el primer comando también sale como `InBatch` con índice 0.
        rejects(
            &mut doc,
            batch(vec![fr(id, &[sp(0, 2, 2)], c1()), good]),
            in_batch(0, DocumentError::BadSpans { span: 0 }),
        );
    }
}

#[test]
fn row_14_the_cost_does_not_depend_on_the_width() {
    fn bytes_of_fill_all(width: u32) -> usize {
        let mut doc = Document::new(Image::new(width, 3, Mode::Rgba).unwrap());
        let id = ImageId::from_raw(0);
        let spans = [sp(0, 0, width), sp(1, 0, width), sp(2, 0, width)];
        assert_eq!(doc.apply(fr(id, &spans, c1())), Ok(true));
        assert_eq!(pixel_en(&doc, 0, 0), Pixel::Rgba(c1()));
        assert_eq!(pixel_en(&doc, width - 1, 2), Pixel::Rgba(c1()));
        assert_eq!(steps_undoable(&doc), 1);
        doc.history_bytes()
    }

    let small = bytes_of_fill_all(4);
    assert!(small > 0);
    assert_eq!(bytes_of_fill_all(4000), small);
    assert_eq!(bytes_of_fill_all(64), small);
    assert_eq!(bytes_of_fill_all(1), small);

    // Con `SetPixels`, en cambio, el coste crece con los píxeles: el relleno es mucho
    // más barato en el lienzo ancho.
    let mut doc = Document::new(Image::new(4000, 3, Mode::Rgba).unwrap());
    let id = ImageId::from_raw(0);
    let pixels_2: Vec<PixelEdit> = (0..3)
        .flat_map(|y| (0..4000).map(move |x| px(x, y, c1())))
        .collect();
    assert_eq!(doc.apply(set(id, &pixels_2)), Ok(true));
    assert!(
        small < doc.history_bytes(),
        "{} no es menor que {}",
        small,
        doc.history_bytes()
    );
}

#[test]
fn row_15_limit_zero_keeps_only_the_last_fill() {
    let mut doc = doc_d();
    let id = ImageId::from_raw(0);
    doc.set_history_limit(0);
    assert_eq!(doc.apply(fr(id, &[sp(0, 0, 4)], c1())), Ok(true));
    assert_eq!(doc.apply(fr(id, &[sp(1, 0, 4)], c1())), Ok(true));

    assert_eq!(steps_undoable(&doc), 1);
    assert_eq!(doc.history_bytes(), cost_of_a_fill(1));

    assert_eq!(doc.undo(), Ok(()));
    assert_eq!(doc.undo(), Err(DocumentError::NothingToUndo));
    checks_image(
        &doc,
        &[(0, 0, c1()), (1, 0, c1()), (2, 0, c1()), (3, 0, c1())],
        c0(),
    );
}

#[test]
fn row_15_a_fill_bigger_than_the_limit_is_kept() {
    let mut doc = doc_d();
    let initial = doc.clone();
    let id = ImageId::from_raw(0);
    doc.set_history_limit(1);

    let all = [sp(0, 0, 4), sp(1, 0, 4), sp(2, 0, 4)];
    assert_eq!(doc.apply(fr(id, &all, c1())), Ok(true));
    assert!(doc.history_bytes() > doc.history_limit());
    assert_eq!(steps_undoable(&doc), 1);
    assert_eq!(doc.undo(), Ok(()));
    assert_eq!(doc, initial);
    assert_eq!(doc.undo(), Err(DocumentError::NothingToUndo));

    // Con otro paso encima, el grande (el más antiguo) se descarta y el nuevo se conserva.
    assert_eq!(doc.redo(), Ok(()));
    assert_eq!(doc.apply(fr(id, &[sp(0, 0, 1)], c2())), Ok(true));
    assert_eq!(steps_undoable(&doc), 1);
    assert_eq!(doc.undo(), Ok(()));
    assert_eq!(pixel_en(&doc, 0, 0), Pixel::Rgba(c1()));
    assert_eq!(pixel_en(&doc, 3, 2), Pixel::Rgba(c1()));
}

#[test]
fn row_15_exact_limit_for_two_fills() {
    let mut doc = doc_d();
    let id = ImageId::from_raw(0);
    assert_eq!(doc.apply(fr(id, &[sp(0, 0, 4)], c1())), Ok(true));
    assert_eq!(doc.apply(fr(id, &[sp(1, 0, 4)], c1())), Ok(true));
    let two = doc.history_bytes();
    assert!(two > 0);

    // Un límite igual a la suma de dos pasos no descarta nada.
    doc.set_history_limit(two);
    assert_eq!(steps_undoable(&doc), 2);
    assert_eq!(doc.history_bytes(), two);

    // El tercero hace que se descarte el más antiguo.
    assert_eq!(doc.apply(fr(id, &[sp(2, 0, 4)], c1())), Ok(true));
    assert_eq!(steps_undoable(&doc), 2);
    assert!(doc.history_bytes() <= two);
    let mut copy = doc.clone();
    assert_eq!(copy.undo(), Ok(()));
    assert_eq!(copy.undo(), Ok(()));
    assert_eq!(copy.undo(), Err(DocumentError::NothingToUndo));
    let mut with_the_first = doc_d();
    assert_eq!(with_the_first.apply(fr(id, &[sp(0, 0, 4)], c1())), Ok(true));
    assert_eq!(copy, with_the_first);
}

#[test]
fn row_16_index_outside_the_palette() {
    // Literales en `IndexNotInPalette`, para no fijar el tipo entero de sus campos.
    let cases = [
        (
            2u8,
            failure_pixel(0, ImageError::IndexNotInPalette { index: 2, len: 2 }),
        ),
        (
            9,
            failure_pixel(0, ImageError::IndexNotInPalette { index: 9, len: 2 }),
        ),
        (
            255,
            failure_pixel(0, ImageError::IndexNotInPalette { index: 255, len: 2 }),
        ),
    ];
    for (index_2, expected) in cases {
        let mut doc = doc_indexed();
        let id = ImageId::from_raw(0);

        // Suelto.
        rejects(
            &mut doc,
            fill(id, &[sp(0, 0, 2)], Pixel::Indexed(index_2)),
            expected.clone(),
        );

        // Segundo comando de un `Batch` tras un `SetPixels` válido, que se revierte.
        let ok = set(
            id,
            &[PixelEdit {
                x: 3,
                y: 2,
                pixel: Pixel::Indexed(1),
            }],
        );
        rejects(
            &mut doc,
            batch(vec![ok, fill(id, &[sp(0, 0, 2)], Pixel::Indexed(index_2))]),
            in_batch(1, expected),
        );
        assert_eq!(pixel_en(&doc, 3, 2), Pixel::Indexed(0));
        assert!(!doc.can_undo());
    }
}

#[test]
fn row_16_indexed_1_is_valid_in_a_palette_of_2() {
    let mut doc = doc_indexed();
    let initial = doc.clone();
    let id = ImageId::from_raw(0);

    assert_eq!(
        doc.apply(fill(id, &[sp(0, 0, 2)], Pixel::Indexed(1))),
        Ok(true)
    );
    assert_eq!(pixel_en(&doc, 0, 0), Pixel::Indexed(1));
    assert_eq!(pixel_en(&doc, 1, 0), Pixel::Indexed(1));
    assert_eq!(pixel_en(&doc, 2, 0), Pixel::Indexed(0));
    assert_eq!(pixel_en(&doc, 0, 1), Pixel::Indexed(0));
    assert_eq!(doc.undo(), Ok(()));
    assert_eq!(doc, initial);

    // Y dentro de un `Batch`.
    let ok = set(
        id,
        &[PixelEdit {
            x: 3,
            y: 2,
            pixel: Pixel::Indexed(1),
        }],
    );
    assert_eq!(
        doc.apply(batch(vec![ok, fill(id, &[sp(0, 0, 2)], Pixel::Indexed(1))])),
        Ok(true)
    );
    assert_eq!(pixel_en(&doc, 3, 2), Pixel::Indexed(1));
    assert_eq!(pixel_en(&doc, 1, 0), Pixel::Indexed(1));
    assert_eq!(steps_undoable(&doc), 1);
    assert_eq!(doc.undo(), Ok(()));
    assert_eq!(doc, initial);

    // Rellenar con el índice que ya hay (0) no cambia nada.
    unchanged(&mut doc, fill(id, &[sp(0, 0, 4)], Pixel::Indexed(0)));
}

/// `D'` de la spec: `D` tras `S(px(1,0,c1))` y `S(px(2,1,c2))`.
fn d_prime() -> Document {
    let mut doc = doc_d();
    let id = ImageId::from_raw(0);
    assert_eq!(doc.apply(set(id, &[px(1, 0, c1())])), Ok(true));
    assert_eq!(doc.apply(set(id, &[px(2, 1, c2())])), Ok(true));
    doc
}

/// Aplica `command` sobre `D'` y comprueba la fila 17: un solo paso más, el estado tras
/// `apply` es `expected` (los píxeles que no valen `c0`; en `D'` el `(2,1)` ya es `c2`),
/// `undo()` deja una imagen idéntica a `D'` y `redo()` deja exactamente la de tras `apply`.
fn checks_row_17(command: Command, expected: &[(u32, u32, Rgba)]) {
    let mut doc = d_prime();
    let original = doc.clone();

    assert_eq!(doc.apply(command.clone()), Ok(true), "{command:?}");
    assert_eq!(steps_undoable(&doc), 3, "{command:?}");
    checks_image(&doc, expected, c0());
    let after_apply = doc.clone();

    for _ in 0..2 {
        assert_eq!(doc.undo(), Ok(()));
        assert_eq!(doc, original, "{command:?}");
        assert_eq!(pixel_en(&doc, 1, 0), Pixel::Rgba(c1()));
        assert_eq!(steps_undoable(&doc), 2);
        assert!(doc.can_redo());

        assert_eq!(doc.redo(), Ok(()));
        assert_eq!(doc, after_apply, "{command:?}");
        checks_image(&doc, expected, c0());
        assert_eq!(steps_undoable(&doc), 3);
        assert!(!doc.can_redo());
    }
}

#[test]
fn row_17a_the_fill_overwrites_the_loose_pixel() {
    let id = ImageId::from_raw(0);
    checks_row_17(
        batch(vec![
            set(id, &[px(1, 0, c2())]),
            fr(id, &[sp(0, 0, 4)], c1()),
        ]),
        &[
            (0, 0, c1()),
            (1, 0, c1()),
            (2, 0, c1()),
            (3, 0, c1()),
            (2, 1, c2()),
        ],
    );
}

#[test]
fn row_17b_the_loose_pixel_overwrites_the_fill() {
    let id = ImageId::from_raw(0);
    checks_row_17(
        batch(vec![
            fr(id, &[sp(0, 0, 4)], c1()),
            set(id, &[px(1, 0, c2())]),
        ]),
        &[
            (0, 0, c1()),
            (1, 0, c2()),
            (2, 0, c1()),
            (3, 0, c1()),
            (2, 1, c2()),
        ],
    );
}

#[test]
fn row_17c_pixel_fill_pixel() {
    let id = ImageId::from_raw(0);
    // `p` en (1,0) y `p'` en otra casilla de la fila.
    checks_row_17(
        batch(vec![
            set(id, &[px(1, 0, c2())]),
            fr(id, &[sp(0, 0, 4)], c3()),
            set(id, &[px(2, 0, c1())]),
        ]),
        &[
            (0, 0, c3()),
            (1, 0, c3()),
            (2, 0, c1()),
            (3, 0, c3()),
            (2, 1, c2()),
        ],
    );
    // `p'` en la misma casilla que `p`, y con el valor que `D'` ya tenía ahí (c1).
    checks_row_17(
        batch(vec![
            set(id, &[px(1, 0, c2())]),
            fr(id, &[sp(0, 0, 4)], c3()),
            set(id, &[px(1, 0, c1())]),
        ]),
        &[
            (0, 0, c3()),
            (1, 0, c1()),
            (2, 0, c3()),
            (3, 0, c3()),
            (2, 1, c2()),
        ],
    );
    // Un `SetPixels` con el mismo píxel repetido antes y después del relleno.
    checks_row_17(
        batch(vec![
            set(id, &[px(1, 0, c2()), px(1, 0, c3())]),
            fr(id, &[sp(0, 1, 3)], c2()),
            set(id, &[px(2, 0, c3()), px(1, 0, c3()), px(2, 0, c1())]),
        ]),
        &[
            (0, 0, c0()),
            (1, 0, c3()),
            (2, 0, c1()),
            (3, 0, c0()),
            (2, 1, c2()),
        ],
    );
}

#[test]
fn row_17d_two_consecutive_fills_that_overlap() {
    let id = ImageId::from_raw(0);
    checks_row_17(
        batch(vec![
            fr(id, &[sp(0, 0, 3)], c3()),
            fr(id, &[sp(0, 1, 4)], c2()),
        ]),
        &[
            (0, 0, c3()),
            (1, 0, c2()),
            (2, 0, c2()),
            (3, 0, c2()),
            (2, 1, c2()),
        ],
    );
    checks_row_17(
        batch(vec![
            fr(id, &[sp(0, 1, 4)], c2()),
            fr(id, &[sp(0, 0, 3)], c3()),
        ]),
        &[
            (0, 0, c3()),
            (1, 0, c3()),
            (2, 0, c3()),
            (3, 0, c2()),
            (2, 1, c2()),
        ],
    );
    // Relleno, píxel suelto en medio y otro relleno que lo pisa.
    checks_row_17(
        batch(vec![
            fr(id, &[sp(0, 0, 4)], c3()),
            set(id, &[px(1, 0, c2())]),
            fr(id, &[sp(0, 1, 3)], c1()),
        ]),
        &[
            (0, 0, c3()),
            (1, 0, c1()),
            (2, 0, c1()),
            (3, 0, c3()),
            (2, 1, c2()),
        ],
    );
}

// Estrategias.

fn mode_2() -> impl Strategy<Value = Mode> {
    prop::sample::select(ALL_2.to_vec())
}

/// Imagen de hasta 6 x 6 en cualquier modo; el indexado lleva una paleta de 1 a 8 entradas.
fn image_6() -> impl Strategy<Value = Image> {
    (mode_2(), 1u32..=6, 1u32..=6, 1usize..=8).prop_map(|(m, w, h, n)| creates(m, w, h, n))
}

/// Lo que hace falta saber de una imagen para construir comandos válidos o con fallos.
#[derive(Clone, Copy, Debug)]
struct Shape {
    width: u32,
    height: u32,
    mode: Mode,
    colors_2: usize,
}

fn shape(img: &Image) -> Shape {
    Shape {
        width: img.width(),
        height: img.height(),
        mode: img.mode(),
        colors_2: img.palette().map_or(0, |p| p.len()),
    }
}

/// Píxel válido de un modo a partir de un valor pequeño. El 0 coincide con el valor
/// inicial de la imagen, y los demás se repiten mucho: así abundan los no-ops.
fn pixel_de(mode: Mode, colors_2: usize, valor: u8) -> Pixel {
    match mode {
        Mode::Rgba if valor == 0 => Pixel::Rgba(Rgba::TRANSPARENT),
        Mode::Rgba => Pixel::Rgba(Rgba::new(valor, 255 - valor, valor ^ 5, 255)),
        Mode::Indexed => Pixel::Indexed((valor as usize % colors_2.max(1)) as u8),
        Mode::Gray => Pixel::Gray(valor),
        Mode::GrayAlpha if valor == 0 => Pixel::GrayAlpha { gray: 0, alpha: 0 },
        Mode::GrayAlpha => Pixel::GrayAlpha {
            gray: valor,
            alpha: 255,
        },
    }
}

fn other_mode(mode: Mode, k: u8) -> Mode {
    let i = ALL_2.iter().position(|&m| m == mode).unwrap();
    ALL_2[(i + 1 + k as usize % 3) % ALL_2.len()]
}

/// Tramos válidos (ordenados por `(y, x_start)`, sin solapes ni vacíos y siempre al menos
/// uno) dentro de una imagen de forma `f`. Cada fila `y` toma los bits de `masks[y]`: cada
/// serie de bits seguidos es un tramo, y con `cut` las series de 2 o más píxeles se
/// parten en dos tramos pegados.
fn spans_valid(f: Shape, masks: &[u8], cut: bool) -> Vec<Span> {
    let mut spans = Vec::new();
    for (y, &m) in masks.iter().enumerate().take(f.height as usize) {
        let y = y as u32;
        let mut x = 0u32;
        while x < f.width {
            if (m >> x) & 1 == 0 {
                x += 1;
                continue;
            }
            let ini = x;
            while x < f.width && (m >> x) & 1 == 1 {
                x += 1;
            }
            if cut && x - ini >= 2 {
                let middle = ini + (x - ini) / 2;
                spans.push(sp(y, ini, middle));
                spans.push(sp(y, middle, x));
            } else {
                spans.push(sp(y, ini, x));
            }
        }
    }
    if spans.is_empty() {
        spans.push(sp(0, 0, 1));
    }
    spans
}

fn masks() -> impl Strategy<Value = Vec<u8>> {
    prop::collection::vec(0u8..64, 6)
}

/// Escrituras en bruto de un `SetPixels`: coordenadas sin acotar y un valor pequeño.
fn edits() -> impl Strategy<Value = Vec<(u32, u32, u8)>> {
    prop::collection::vec((any::<u32>(), any::<u32>(), 0u8..4), 1..=4)
}

/// Una hoja (un `SetPixels` o un `FillSpans`) en bruto. Las variantes con referencia
/// (`FillCovers`, `FillCopy`, `SetInside`) apuntan, módulo el número de candidatas, a otra
/// hoja **del mismo lote**, para que los píxeles sueltos y los tramos se solapen a propósito:
///
/// - `FillCovers`: tramos de un píxel sobre los píxeles de un `Set` libre del lote.
/// - `FillCopy`: los mismos tramos (o su primera mitad) que un `Fill` propio del lote, con
///   otro valor: dos rellenos que se pisan.
/// - `SetInside`: píxeles sueltos elegidos dentro de los tramos de un `Fill` propio.
///
/// Si el lote no tiene a quién apuntar, la hoja se degrada a una libre.
#[derive(Clone, Debug)]
enum SheetRaw {
    Fill {
        masks: Vec<u8>,
        cut: bool,
        valor: u8,
    },
    FillCovers {
        set: usize,
        valor: u8,
    },
    FillCopy {
        fill: usize,
        half: bool,
        valor: u8,
    },
    Set {
        edits: Vec<(u32, u32, u8)>,
    },
    SetInside {
        fill: usize,
        edits: Vec<(u32, u32, u8)>,
    },
}

/// Fallos que se inyectan en un `FillSpans` ya construido. No garantizan el error
/// concreto (lo decide el modelo), solo lo provocan a menudo.
#[derive(Clone, Copy, Debug)]
enum FailSpan {
    ListEmpty,
    IdUnknown,
    OtherMode,
    IndexOutside,
    Empty,
    Unordered,
    Overlapping,
    OutsideX,
    OutsideY,
}

const FAILS: [FailSpan; 9] = [
    FailSpan::ListEmpty,
    FailSpan::IdUnknown,
    FailSpan::OtherMode,
    FailSpan::IndexOutside,
    FailSpan::Empty,
    FailSpan::Unordered,
    FailSpan::Overlapping,
    FailSpan::OutsideX,
    FailSpan::OutsideY,
];

#[derive(Clone, Debug)]
struct Sheet {
    raw: SheetRaw,
    /// Solo se aplica si la hoja resulta ser un `FillSpans`.
    fail: Option<(FailSpan, u8)>,
}

#[derive(Clone, Debug)]
enum Cmd {
    Sheet(Sheet),
    Batch(Vec<Sheet>),
}

fn sheet_raw() -> impl Strategy<Value = SheetRaw> {
    prop_oneof![
        3 => (masks(), any::<bool>(), 0u8..4)
            .prop_map(|(masks, cut, valor)| SheetRaw::Fill { masks, cut, valor }),
        2 => (any::<usize>(), 0u8..4)
            .prop_map(|(set, valor)| SheetRaw::FillCovers { set, valor }),
        2 => (any::<usize>(), any::<bool>(), 0u8..4)
            .prop_map(|(fill, half, valor)| SheetRaw::FillCopy { fill, half, valor }),
        3 => edits().prop_map(|edits| SheetRaw::Set { edits }),
        3 => (any::<usize>(), edits())
            .prop_map(|(fill, edits)| SheetRaw::SetInside { fill, edits }),
    ]
}

fn fail_span(with_failures: bool) -> BoxedStrategy<Option<(FailSpan, u8)>> {
    if with_failures {
        prop_oneof![
            5 => Just(None::<(FailSpan, u8)>),
            2 => (prop::sample::select(FAILS.to_vec()), any::<u8>()).prop_map(Some),
        ]
        .boxed()
    } else {
        Just(None).boxed()
    }
}

fn sheet(with_failures: bool) -> impl Strategy<Value = Sheet> {
    (sheet_raw(), fail_span(with_failures)).prop_map(|(raw, fail)| Sheet { raw, fail })
}

/// Una hoja suelta o un `Batch` de 1 a 4 hojas (de 0 a 4 con fallos), con fallos o sin ellos.
fn cmd(with_failures: bool) -> impl Strategy<Value = Cmd> {
    let largo = if with_failures { 0..=4 } else { 1..=4 };
    prop_oneof![
        2 => sheet(with_failures).prop_map(Cmd::Sheet),
        2 => prop::collection::vec(sheet(with_failures), largo).prop_map(Cmd::Batch),
    ]
}

fn set_free(f: Shape, id: ImageId, edits: &[(u32, u32, u8)]) -> Command {
    Command::SetPixels {
        image: id,
        pixels: edits
            .iter()
            .map(|&(xb, yb, v)| PixelEdit {
                x: xb % f.width,
                y: yb % f.height,
                pixel: pixel_de(f.mode, f.colors_2, v),
            })
            .collect(),
    }
}

fn spans_in(cmds: &[Option<Command>], i: usize) -> Vec<Span> {
    match cmds.get(i) {
        Some(Some(Command::FillSpans { spans, .. })) => spans.clone(),
        _ => Vec::new(),
    }
}

fn pixels_in(cmds: &[Option<Command>], i: usize) -> Vec<PixelEdit> {
    match cmds.get(i) {
        Some(Some(Command::SetPixels { pixels, .. })) => pixels.clone(),
        _ => Vec::new(),
    }
}

/// Aplica un fallo a un `FillSpans`; cualquier otro comando queda igual.
fn injects(f: Shape, cmd: Command, fail: FailSpan, k: u8) -> Command {
    match cmd {
        Command::FillSpans {
            mut image,
            mut spans,
            mut pixel,
        } => {
            match fail {
                FailSpan::ListEmpty => spans.clear(),
                // La imagen principal es la 0.
                FailSpan::IdUnknown => image = ImageId::from_raw(1 + k as u32),
                FailSpan::OtherMode => pixel = pixel_de(other_mode(f.mode, k), 8, k),
                // En una imagen no indexada esto es un `ModeMismatch`.
                FailSpan::IndexOutside => {
                    pixel = Pixel::Indexed((f.colors_2 + (k as usize % 4)) as u8)
                }
                FailSpan::Empty => {
                    let n = spans.len().max(1);
                    if let Some(t) = spans.get_mut(k as usize % n) {
                        t.x_end = t.x_start;
                    }
                }
                FailSpan::Unordered => {
                    if spans.len() >= 2 {
                        spans.swap(0, 1);
                    } else {
                        spans.insert(0, sp(f.height, 0, 1));
                    }
                }
                FailSpan::Overlapping => {
                    if let Some(&t) = spans.first() {
                        spans.insert(1, t);
                    }
                }
                FailSpan::OutsideX => {
                    if let Some(t) = spans.last_mut() {
                        t.x_end = f.width + 1 + (k as u32 % 5);
                    }
                }
                FailSpan::OutsideY => {
                    if let Some(t) = spans.last_mut() {
                        t.y = f.height + (k as u32 % 5);
                    }
                }
            }
            Command::FillSpans {
                image,
                spans,
                pixel,
            }
        }
        other => other,
    }
}

/// Convierte las hojas de un lote en comandos. Las referencias se resuelven en una
/// segunda pasada contra las hojas independientes (sin fallos aún: los fallos se inyectan
/// al final), así que los píxeles sueltos caen dentro de los tramos y al revés.
fn resolves(f: Shape, id: ImageId, sheets: &[Sheet]) -> Vec<Command> {
    let pixel = |v: u8| pixel_de(f.mode, f.colors_2, v);
    let fill_2 = |spans: Vec<Span>, v: u8| Command::FillSpans {
        image: id,
        spans,
        pixel: pixel(v),
    };

    // Primera pasada: hojas independientes.
    let mut cmds: Vec<Option<Command>> = sheets
        .iter()
        .map(|h| match &h.raw {
            SheetRaw::Fill { masks, cut, valor } => {
                Some(fill_2(spans_valid(f, masks, *cut), *valor))
            }
            SheetRaw::Set { edits } => Some(set_free(f, id, edits)),
            _ => None,
        })
        .collect();
    let fills_2: Vec<usize> = sheets
        .iter()
        .enumerate()
        .filter(|(_, h)| matches!(h.raw, SheetRaw::Fill { .. }))
        .map(|(i, _)| i)
        .collect();
    let sets: Vec<usize> = sheets
        .iter()
        .enumerate()
        .filter(|(_, h)| matches!(h.raw, SheetRaw::Set { .. }))
        .map(|(i, _)| i)
        .collect();

    // Segunda pasada: hojas que apuntan a otras del mismo lote.
    for (i, h) in sheets.iter().enumerate() {
        let new_2 = match &h.raw {
            SheetRaw::FillCovers { set, valor } => {
                let mut coords: Vec<(u32, u32)> = if sets.is_empty() {
                    vec![(0, 0)]
                } else {
                    pixels_in(&cmds, sets[*set % sets.len()])
                        .iter()
                        .map(|p| (p.y, p.x))
                        .collect()
                };
                coords.sort_unstable();
                coords.dedup();
                let spans = coords.iter().map(|&(y, x)| sp(y, x, x + 1)).collect();
                fill_2(spans, *valor)
            }
            SheetRaw::FillCopy { fill, half, valor } => {
                let mut spans = if fills_2.is_empty() {
                    vec![sp(0, 0, 1)]
                } else {
                    spans_in(&cmds, fills_2[*fill % fills_2.len()])
                };
                if *half {
                    spans.truncate(spans.len().div_ceil(2));
                }
                fill_2(spans, *valor)
            }
            SheetRaw::SetInside { fill, edits } => {
                let spans_2 = if fills_2.is_empty() {
                    Vec::new()
                } else {
                    spans_in(&cmds, fills_2[*fill % fills_2.len()])
                };
                if spans_2.is_empty() {
                    set_free(f, id, edits)
                } else {
                    Command::SetPixels {
                        image: id,
                        pixels: edits
                            .iter()
                            .map(|&(xb, yb, v)| {
                                let t = spans_2[yb as usize % spans_2.len()];
                                PixelEdit {
                                    x: t.x_start + xb % (t.x_end - t.x_start),
                                    y: t.y,
                                    pixel: pixel(v),
                                }
                            })
                            .collect(),
                    }
                }
            }
            _ => continue,
        };
        cmds[i] = Some(new_2);
    }

    cmds.into_iter()
        .zip(sheets)
        .map(|(c, h)| {
            let c = c.expect("todas las hojas quedan resueltas");
            match h.fail {
                Some((fail, k)) => injects(f, c, fail, k),
                None => c,
            }
        })
        .collect()
}

fn command(f: Shape, id: ImageId, raw: &Cmd) -> Command {
    match raw {
        Cmd::Sheet(h) => resolves(f, id, std::slice::from_ref(h))
            .pop()
            .expect("una hoja da un comando"),
        Cmd::Batch(sheets) => Command::Batch(resolves(f, id, sheets)),
    }
}

/// Documento con una imagen (con contenido previo, sacado de aplicar `seed`) pero sin
/// historia.
fn document_initial(img: Image, seed: &[Cmd]) -> Document {
    let f = shape(&img);
    let mut doc = Document::new(img);
    let id = ImageId::from_raw(0);
    for c in seed {
        let _ = doc.apply(command(f, id, c));
    }
    Document::new(image_of(&doc).clone())
}

// Comandos totalmente arbitrarios (propiedad 3).

#[derive(Clone, Debug)]
enum ArbRaw {
    Fill {
        id: Option<u32>,
        spans: Vec<(u32, u32, u32)>,
        pixel: Pixel,
        /// Si hay valor, sustituye al píxel por uno válido del modo de la imagen.
        own: Option<u8>,
    },
    Set {
        id: Option<u32>,
        edits: Vec<(u32, u32, u8)>,
    },
    Batch(Vec<ArbRaw>),
}

fn pixel_arbitrary() -> impl Strategy<Value = Pixel> {
    prop_oneof![
        (any::<u8>(), any::<u8>(), any::<u8>(), any::<u8>())
            .prop_map(|(r, g, b, a)| Pixel::Rgba(Rgba::new(r, g, b, a))),
        any::<u8>().prop_map(Pixel::Indexed),
        any::<u8>().prop_map(Pixel::Gray),
        (any::<u8>(), any::<u8>()).prop_map(|(gray, alpha)| Pixel::GrayAlpha { gray, alpha }),
    ]
}

fn coordinate_arbitrary() -> impl Strategy<Value = u32> {
    prop_oneof![
        6 => 0u32..7,
        1 => any::<u32>(),
        1 => Just(u32::MAX),
    ]
}

fn id_arbitrary() -> impl Strategy<Value = Option<u32>> {
    prop_oneof![4 => Just(None), 1 => any::<u32>().prop_map(Some)]
}

fn arb_raw() -> impl Strategy<Value = ArbRaw> {
    let fill_2 = (
        id_arbitrary(),
        prop::collection::vec(
            (
                coordinate_arbitrary(),
                coordinate_arbitrary(),
                coordinate_arbitrary(),
            ),
            0..=5,
        ),
        pixel_arbitrary(),
        prop_oneof![2 => Just(None), 3 => (0u8..4).prop_map(Some)],
    )
        .prop_map(|(id, spans, pixel, own)| ArbRaw::Fill {
            id,
            spans,
            pixel,
            own,
        });
    let loose = (id_arbitrary(), edits()).prop_map(|(id, edits)| ArbRaw::Set { id, edits });
    let sheet = prop_oneof![4 => fill_2, 1 => loose];
    sheet.prop_recursive(2, 16, 3, |internal| {
        prop::collection::vec(internal, 0..=3).prop_map(ArbRaw::Batch)
    })
}

fn arb_command(f: Shape, id: ImageId, raw: &ArbRaw) -> Command {
    match raw {
        ArbRaw::Fill {
            id: other,
            spans,
            pixel,
            own,
        } => Command::FillSpans {
            image: other.map_or(id, ImageId::from_raw),
            spans: spans
                .iter()
                .map(|&(y, a, b)| Span {
                    y,
                    x_start: a,
                    x_end: b,
                })
                .collect(),
            pixel: own.map_or(*pixel, |v| pixel_de(f.mode, f.colors_2, v)),
        },
        ArbRaw::Set { id: other, edits } => Command::SetPixels {
            image: other.map_or(id, ImageId::from_raw),
            pixels: edits
                .iter()
                .map(|&(xb, yb, v)| PixelEdit {
                    x: xb,
                    y: yb,
                    pixel: pixel_de(f.mode, f.colors_2, v),
                })
                .collect(),
        },
        ArbRaw::Batch(commands) => {
            Command::Batch(commands.iter().map(|c| arb_command(f, id, c)).collect())
        }
    }
}

// Modelo de referencia: escribe píxel a píxel sobre instantáneas de `Image` (propiedades 2 y 5).

/// Aplica un comando hoja sobre `img` y dice si cambió algo. Para `FillSpans` sigue el
/// orden de la spec: `EmptyCommand`, `UnknownImage`, el píxel, y después cada tramo en
/// orden (`BadSpans` antes que `Span`), todo en una pasada de solo lectura antes de escribir.
fn model_sheet(img: &mut Image, id: ImageId, cmd: &Command) -> Result<bool, DocumentError> {
    match cmd {
        Command::SetPixels { image, pixels } => {
            if pixels.is_empty() {
                return Err(DocumentError::EmptyCommand);
            }
            if *image != id {
                return Err(DocumentError::UnknownImage(*image));
            }
            let mut change = false;
            for (i, e) in pixels.iter().enumerate() {
                let antes = img.get_pixel(e.x, e.y);
                img.set_pixel(e.x, e.y, e.pixel)
                    .map_err(|source| failure_pixel(i, source))?;
                if antes != Ok(e.pixel) {
                    change = true;
                }
            }
            Ok(change)
        }
        Command::FillSpans {
            image,
            spans,
            pixel,
        } => {
            if spans.is_empty() {
                return Err(DocumentError::EmptyCommand);
            }
            if *image != id {
                return Err(DocumentError::UnknownImage(*image));
            }
            // El error de píxel lo da la propia imagen (en una copia, en la esquina 0, 0).
            img.clone()
                .set_pixel(0, 0, *pixel)
                .map_err(|source| failure_pixel(0, source))?;
            let (w, h) = (img.width(), img.height());
            for (i, t) in spans.iter().enumerate() {
                let unordered = i > 0 && {
                    let previous = &spans[i - 1];
                    t.y < previous.y || (t.y == previous.y && t.x_start < previous.x_end)
                };
                if t.x_start >= t.x_end || unordered {
                    return Err(DocumentError::BadSpans { span: i });
                }
                if t.y >= h {
                    return Err(failure_span(i, outside(t.x_start, t.y, w, h)));
                }
                if t.x_end > w {
                    return Err(failure_span(i, outside(t.x_start.max(w), t.y, w, h)));
                }
            }
            let mut change = false;
            for t in spans {
                for x in t.x_start..t.x_end {
                    if img.get_pixel(x, t.y) != Ok(*pixel) {
                        img.set_pixel(x, t.y, *pixel)
                            .expect("el tramo y el píxel ya están validados");
                        change = true;
                    }
                }
            }
            Ok(change)
        }
        other => unreachable!("comando sin modelo: {other:?}"),
    }
}

/// Resultado esperado de `apply` sobre `actual`: la imagen resultante y si cambió, o el
/// error. Los comandos de un `Batch` se ejecutan en orden y el primer fallo gana.
fn model_apply(
    actual: &Image,
    id: ImageId,
    command: &Command,
) -> Result<(Image, bool), DocumentError> {
    let mut work = actual.clone();
    match command {
        Command::Batch(commands) => {
            if commands.is_empty() {
                return Err(DocumentError::EmptyCommand);
            }
            let mut change = false;
            for (index, c) in commands.iter().enumerate() {
                match c {
                    Command::Batch(_) => return Err(in_batch(index, DocumentError::NestedBatch)),
                    sheet => {
                        change |=
                            model_sheet(&mut work, id, sheet).map_err(|e| in_batch(index, e))?;
                    }
                }
            }
            Ok((work, change))
        }
        sheet => {
            let change = model_sheet(&mut work, id, sheet)?;
            Ok((work, change))
        }
    }
}

/// El documento tiene exactamente `expected` como contenido.
fn checks_content(doc: &Document, expected: &Image) -> Result<(), TestCaseError> {
    prop_assert_eq!(doc.image(ImageId::from_raw(0)), Some(expected));
    prop_assert_eq!(doc, &Document::new(expected.clone()));
    Ok(())
}

#[derive(Clone, Debug)]
enum Op {
    Apply(Cmd),
    Undo,
    Redo,
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        3 => cmd(false).prop_map(Op::Apply),
        3 => cmd(true).prop_map(Op::Apply),
        2 => Just(Op::Undo),
        2 => Just(Op::Redo),
    ]
}

/// Ejecuta `ops` sobre un documento y sobre el modelo (instantáneas de la imagen y un
/// cursor) y los compara tras cada operación.
fn runs_model(initial: Document, ops: &[Op]) -> Result<(), TestCaseError> {
    let img = image_of(&initial).clone();
    let f = shape(&img);
    let mut doc = initial;
    let id = ImageId::from_raw(0);
    let mut instants = vec![img];
    let mut cursor = 0usize;

    for op in ops {
        let antes = state(&doc);
        match op {
            Op::Apply(raw) => {
                let cmd = command(f, id, raw);
                let expected_2 = model_apply(&instants[cursor], id, &cmd);
                let real = doc.apply(cmd.clone());
                match expected_2 {
                    Ok((new, true)) => {
                        prop_assert_eq!(real, Ok(true), "{:?}", cmd);
                        instants.truncate(cursor + 1);
                        instants.push(new);
                        cursor += 1;
                    }
                    Ok((_, false)) => {
                        prop_assert_eq!(real, Ok(false), "{:?}", cmd);
                        prop_assert_eq!(state(&doc), antes);
                    }
                    Err(e) => {
                        prop_assert_eq!(real, Err(e), "{:?}", cmd);
                        prop_assert_eq!(state(&doc), antes);
                    }
                }
            }
            Op::Undo => {
                if cursor > 0 {
                    prop_assert_eq!(doc.undo(), Ok(()));
                    cursor -= 1;
                } else {
                    prop_assert_eq!(doc.undo(), Err(DocumentError::NothingToUndo));
                    prop_assert_eq!(state(&doc), antes);
                }
            }
            Op::Redo => {
                if cursor + 1 < instants.len() {
                    prop_assert_eq!(doc.redo(), Ok(()));
                    cursor += 1;
                } else {
                    prop_assert_eq!(doc.redo(), Err(DocumentError::NothingToRedo));
                    prop_assert_eq!(state(&doc), antes);
                }
            }
        }
        checks_content(&doc, &instants[cursor])?;
        prop_assert_eq!(doc.can_undo(), cursor > 0);
        prop_assert_eq!(doc.can_redo(), cursor + 1 < instants.len());
        prop_assert_eq!(steps_undoable(&doc), cursor);
        prop_assert_eq!(steps_redoable(&doc), instants.len() - 1 - cursor);
    }
    Ok(())
}

#[derive(Clone, Debug)]
enum OpArb {
    Cmd(Cmd),
    Arb(ArbRaw),
    Undo,
    Redo,
}

fn op_arb() -> impl Strategy<Value = OpArb> {
    prop_oneof![
        4 => cmd(true).prop_map(OpArb::Cmd),
        4 => arb_raw().prop_map(OpArb::Arb),
        1 => Just(OpArb::Undo),
        1 => Just(OpArb::Redo),
    ]
}

/// `apply(cmd)`: si no da `Ok(true)` no se toca nada observable; si lo da, hay algo que
/// deshacer y nada que rehacer.
fn applies_and_checks(
    doc: &mut Document,
    cmd: Command,
    antes: &State,
) -> Result<(), TestCaseError> {
    match doc.apply(cmd.clone()) {
        Ok(true) => {
            prop_assert!(doc.can_undo());
            prop_assert!(!doc.can_redo());
        }
        Ok(false) | Err(_) => prop_assert_eq!(&state(doc), antes, "{:?}", cmd),
    }
    Ok(())
}

/// Cuenta `undo()` hasta el primer `Err`, sin pasarse de `cap + 1` por si nunca falla.
fn undoes_all(doc: &mut Document, cap: usize) -> usize {
    let mut n = 0;
    while n <= cap && doc.undo().is_ok() {
        n += 1;
    }
    n
}

fn redoes_all(doc: &mut Document, cap: usize) -> usize {
    let mut n = 0;
    while n <= cap && doc.redo().is_ok() {
        n += 1;
    }
    n
}

/// Comprobaciones de la propiedad 5 sobre un documento que solo ha recibido `apply`:
/// `instants[0]` es el inicial y cada `Ok(true)` añadió uno.
fn checks_limit(doc: &Document, instants: &[Image], limit: usize) -> Result<(), TestCaseError> {
    let created = instants.len() - 1;
    let kept = steps_undoable(doc);

    prop_assert_eq!(doc.history_limit(), limit);
    prop_assert!(kept <= created, "{} > {}", kept, created);
    prop_assert!(
        created == 0 || kept >= 1,
        "el último paso siempre se conserva"
    );
    prop_assert!(
        doc.history_bytes() <= limit || kept == 1,
        "history_bytes = {} > límite {} con {} pasos",
        doc.history_bytes(),
        limit,
        kept
    );
    prop_assert_eq!(doc.can_undo(), kept > 0);
    prop_assert!(!doc.can_redo());
    checks_content(doc, &instants[created])?;

    // Deshacer todo llega al instante en que empezó el primer paso conservado.
    let bytes_after_apply = doc.history_bytes();
    let mut copy = doc.clone();
    prop_assert_eq!(undoes_all(&mut copy, kept), kept);
    prop_assert_eq!(copy.undo(), Err(DocumentError::NothingToUndo));
    checks_content(&copy, &instants[created - kept])?;

    // Rehacer todo devuelve el final, con el mismo coste que tras el `apply`: `undo` y
    // `redo` solo mueven los pasos entre pilas.
    prop_assert_eq!(redoes_all(&mut copy, kept), kept);
    prop_assert_eq!(copy.redo(), Err(DocumentError::NothingToRedo));
    checks_content(&copy, &instants[created])?;
    prop_assert_eq!(copy.history_bytes(), bytes_after_apply);
    Ok(())
}

/// Aplica `cmds` (válidos) a `doc`, comprueba cada resultado contra el modelo, añade a
/// `instants` una imagen por cada `Ok(true)` y tras cada `apply` comprueba la propiedad 5.
fn applies_valid(
    doc: &mut Document,
    instants: &mut Vec<Image>,
    f: Shape,
    cmds: &[Cmd],
    limit: usize,
) -> Result<(), TestCaseError> {
    let id = ImageId::from_raw(0);
    for raw in cmds {
        let cmd = command(f, id, raw);
        let actual = instants.last().unwrap().clone();
        let (new, change) = match model_apply(&actual, id, &cmd) {
            Ok(r) => r,
            Err(e) => {
                return Err(TestCaseError::fail(format!(
                    "el generador sin fallos produjo un comando inválido: {e:?} ({cmd:?})"
                )));
            }
        };
        prop_assert_eq!(doc.apply(cmd), Ok(change));
        if change {
            instants.push(new);
        }
        checks_limit(doc, instants, limit)?;
    }
    Ok(())
}

// `history_bytes()` es el coste de la pila de deshacer (fila 18 y propiedad 7).

/// Aplica `cmds` en orden, todos con `Ok(true)`, y comprueba que `history_bytes()` sube con
/// cada `apply`, baja con cada `undo` hasta 0 y sube con cada `redo` hasta el valor final,
/// pasando por los mismos valores. Devuelve esos valores (el primero es 0).
fn checks_stack_bytes(mut doc: Document, cmds: Vec<Command>) -> Vec<usize> {
    let mut bytes = vec![doc.history_bytes()];
    assert_eq!(bytes[0], 0);
    for cmd in cmds {
        assert_eq!(doc.apply(cmd), Ok(true));
        let now = doc.history_bytes();
        assert!(
            now > *bytes.last().unwrap(),
            "apply debe sumar el coste del paso"
        );
        bytes.push(now);
    }
    for (j, expected) in bytes.iter().rev().skip(1).enumerate() {
        assert_eq!(doc.undo(), Ok(()));
        assert_eq!(doc.history_bytes(), *expected, "tras {} undo", j + 1);
    }
    assert_eq!(doc.history_bytes(), 0);
    for (j, expected) in bytes.iter().enumerate().skip(1) {
        assert_eq!(doc.redo(), Ok(()));
        assert_eq!(doc.history_bytes(), *expected, "tras {j} redo");
    }
    bytes
}

#[test]
fn row_18_history_bytes_follows_the_undo_stack() {
    let id = ImageId::from_raw(0);
    // Un `SetPixels`, un `FillSpans` y un `Batch` que mezcla ambos, uno tras otro.
    let bytes = checks_stack_bytes(
        doc_d(),
        vec![
            set(id, &[px(0, 0, c1())]),
            fr(id, &[sp(1, 0, 4), sp(2, 1, 3)], c2()),
            batch(vec![
                set(id, &[px(3, 0, c1())]),
                fr(id, &[sp(0, 0, 4)], c3()),
                set(id, &[px(2, 2, c1())]),
            ]),
        ],
    );
    assert_eq!(bytes.len(), 4);
}

#[test]
fn row_18_a_single_step_of_each_kind() {
    let id = ImageId::from_raw(0);
    let cases = [
        set(id, &[px(0, 0, c1())]),
        fr(id, &[sp(0, 0, 4), sp(1, 0, 4), sp(2, 0, 4)], c1()),
        batch(vec![
            set(id, &[px(1, 1, c1())]),
            fr(id, &[sp(1, 0, 4)], c2()),
        ]),
    ];
    for cmd in cases {
        let mut doc = doc_d();
        assert_eq!(doc.apply(cmd), Ok(true));
        let cost = doc.history_bytes();
        assert!(cost > 0);
        assert_eq!(doc.undo(), Ok(()));
        assert_eq!(
            doc.history_bytes(),
            0,
            "tras undo la pila de deshacer está vacía"
        );
        assert!(doc.can_redo());
        assert_eq!(doc.redo(), Ok(()));
        assert_eq!(
            doc.history_bytes(),
            cost,
            "tras redo vuelve al coste del paso"
        );
    }
}

#[test]
fn row_18_an_apply_after_undo_counts_only_its_step() {
    let mut doc = doc_d();
    let id = ImageId::from_raw(0);
    assert_eq!(doc.apply(fr(id, &[sp(0, 0, 4)], c1())), Ok(true));
    assert_eq!(doc.apply(fr(id, &[sp(1, 0, 4)], c2())), Ok(true));
    let two = doc.history_bytes();
    assert_eq!(doc.undo(), Ok(()));
    let one = doc.history_bytes();
    assert!(one < two);
    // Un paso nuevo borra rehacer: la pila de deshacer pasa a tener el primero y el nuevo.
    assert_eq!(doc.apply(fr(id, &[sp(1, 0, 4)], c3())), Ok(true));
    assert!(!doc.can_redo());
    assert_eq!(doc.history_bytes(), two);
    // Un `apply` sin cambios y un `undo` fallido no tocan `history_bytes()`.
    assert_eq!(doc.apply(fr(id, &[sp(1, 0, 4)], c3())), Ok(false));
    assert_eq!(doc.history_bytes(), two);
    let mut empty = doc_d();
    assert_eq!(empty.undo(), Err(DocumentError::NothingToUndo));
    assert_eq!(empty.history_bytes(), 0);
}

// Propiedades.

proptest! {
    // Propiedad 1: central ampliada, con lotes mezclados y coordenadas solapadas.
    #[test]
    fn undo_all_and_redo_all_with_fills(
        img in image_6(),
        seed in prop::collection::vec(cmd(false), 0..=3),
        cmds in prop::collection::vec(cmd(true), 0..=30),
    ) {
        let f = shape(&img);
        let initial = document_initial(img, &seed);
        let id = ImageId::from_raw(0);
        let mut doc = initial.clone();

        let mut with_change = 0usize;
        for raw in &cmds {
            // Los que fallan se ignoran.
            if let Ok(true) = doc.apply(command(f, id, raw)) {
                with_change += 1;
            }
        }
        let final_ = doc.clone();

        prop_assert_eq!(undoes_all(&mut doc, cmds.len()), with_change);
        prop_assert_eq!(doc.undo(), Err(DocumentError::NothingToUndo));
        prop_assert_eq!(&doc, &initial);
        prop_assert!(!doc.can_undo());

        prop_assert_eq!(redoes_all(&mut doc, cmds.len()), with_change);
        prop_assert_eq!(doc.redo(), Err(DocumentError::NothingToRedo));
        prop_assert_eq!(&doc, &final_);
        prop_assert!(!doc.can_redo());
    }

    // Propiedad 2: la historia contra un modelo.
    #[test]
    fn history_against_a_model_with_fills(
        img in image_6(),
        seed in prop::collection::vec(cmd(false), 0..=3),
        ops in prop::collection::vec(op(), 0..=40),
    ) {
        runs_model(document_initial(img, &seed), &ops)?;
    }

    // Propiedad 3: atomicidad y ausencia de pánicos con `FillSpans` arbitrarios.
    #[test]
    fn atomicity_and_no_panics_with_fills(
        img in image_6(),
        seed in prop::collection::vec(cmd(false), 0..=4),
        ops in prop::collection::vec(op_arb(), 0..=20),
    ) {
        let f = shape(&img);
        let mut doc = Document::new(img);
        let id = ImageId::from_raw(0);
        for raw in &seed {
            let _ = doc.apply(command(f, id, raw));
        }

        for op in &ops {
            let antes = state(&doc);
            match op {
                OpArb::Cmd(raw) => applies_and_checks(&mut doc, command(f, id, raw), &antes)?,
                OpArb::Arb(raw) => applies_and_checks(&mut doc, arb_command(f, id, raw), &antes)?,
                OpArb::Undo => match doc.undo() {
                    Ok(()) => prop_assert!(doc.can_redo()),
                    Err(e) => {
                        prop_assert_eq!(e, DocumentError::NothingToUndo);
                        prop_assert_eq!(state(&doc), antes);
                    }
                },
                OpArb::Redo => match doc.redo() {
                    Ok(()) => prop_assert!(doc.can_undo()),
                    Err(e) => {
                        prop_assert_eq!(e, DocumentError::NothingToRedo);
                        prop_assert_eq!(state(&doc), antes);
                    }
                },
            }
        }
    }

    // Propiedad 4, primera parte: un relleno nunca cuesta más que el `SetPixels`
    // equivalente (más un margen fijo), sobre cualquier contenido.
    #[test]
    fn fill_costs_no_more_than_set_pixels(
        img in image_6(),
        seed in prop::collection::vec(cmd(false), 0..=4),
        masks in masks(),
        cut in any::<bool>(),
        valor in 0u8..4,
    ) {
        // Imagen con contenido previo (a menudo ruidoso), y una copia sin historia por rama.
        let previous = document_initial(img, &seed);
        let image_2 = image_of(&previous).clone();
        let f = shape(&image_2);
        let pixel = pixel_de(f.mode, f.colors_2, valor);
        let spans = spans_valid(f, &masks, cut);
        let pixels_2: Vec<PixelEdit> = spans
            .iter()
            .flat_map(|t| (t.x_start..t.x_end).map(move |x| PixelEdit { x, y: t.y, pixel }))
            .collect();

        let mut with_fill = Document::new(image_2.clone());
        let mut with_set = Document::new(image_2);
        let id = ImageId::from_raw(0);
        let r_fill = with_fill.apply(Command::FillSpans { image: id, spans, pixel });
        let r_set = with_set.apply(Command::SetPixels { image: id, pixels: pixels_2 });

        prop_assert_eq!(&r_fill, &r_set);
        prop_assert_eq!(&with_fill, &with_set);
        prop_assert!(
            with_fill.history_bytes() <= with_set.history_bytes() + 128,
            "relleno {} B, SetPixels {} B",
            with_fill.history_bytes(),
            with_set.history_bytes()
        );
    }

    // Propiedad 4, segunda parte: sobre fondo uniforme el coste depende de los trozos y
    // no de los píxeles (mismos tramos en las mismas filas con la mitad de ancho).
    #[test]
    fn fill_cost_does_not_depend_on_span_width(
        img in image_6(),
        masks in masks(),
        cut in any::<bool>(),
        valor in 0u8..4,
    ) {
        let f = shape(&img);
        let pixel = pixel_de(f.mode, f.colors_2, valor);
        let spans = spans_valid(f, &masks, cut);
        let half: Vec<Span> = spans
            .iter()
            .map(|t| sp(t.y, t.x_start, t.x_start + ((t.x_end - t.x_start) / 2).max(1)))
            .collect();

        let mut complete = Document::new(img.clone());
        let mut reduced = Document::new(img);
        let id = ImageId::from_raw(0);
        let r_complete = complete.apply(Command::FillSpans { image: id, spans, pixel });
        let r_reduced = reduced.apply(Command::FillSpans { image: id, spans: half, pixel });

        prop_assert_eq!(&r_complete, &r_reduced);
        prop_assert_eq!(complete.history_bytes(), reduced.history_bytes());
    }

    // Propiedad 5: el límite con el coste real, con el límite fijado antes de editar.
    #[test]
    fn history_limit_with_fills(
        img in image_6(),
        limit in 0usize..=2000,
        cmds in prop::collection::vec(cmd(false), 0..=30),
    ) {
        let f = shape(&img);
        let mut doc = Document::new(img.clone());
        doc.set_history_limit(limit);
        let mut instants = vec![img];
        applies_valid(&mut doc, &mut instants, f, &cmds, limit)?;
        checks_limit(&doc, &instants, limit)?;
    }

    // Propiedad 7: `history_bytes()` es el coste de la pila de deshacer: baja con cada `undo`,
    // sube con cada `redo` y pasa por los mismos valores.
    #[test]
    fn history_bytes_follows_the_undo_stack(
        img in image_6(),
        cmds in prop::collection::vec(cmd(false), 0..=12),
    ) {
        let f = shape(&img);
        let mut doc = Document::new(img);
        let id = ImageId::from_raw(0);
        let mut bytes = vec![0usize];
        for raw in &cmds {
            if doc.apply(command(f, id, raw)) == Ok(true) {
                bytes.push(doc.history_bytes());
            }
        }
        prop_assert!(bytes.windows(2).all(|w| w[0] < w[1]), "no crece: {bytes:?}");
        for expected in bytes.iter().rev().skip(1) {
            prop_assert_eq!(doc.undo(), Ok(()));
            prop_assert_eq!(doc.history_bytes(), *expected);
        }
        prop_assert_eq!(doc.history_bytes(), 0);
        for expected in bytes.iter().skip(1) {
            prop_assert_eq!(doc.redo(), Ok(()));
            prop_assert_eq!(doc.history_bytes(), *expected);
        }
    }

    // Propiedad 6: un `Batch` mixto es un paso.
    #[test]
    fn mixed_batch_is_one_step(
        img in image_6(),
        seed in prop::collection::vec(cmd(false), 0..=3),
        sheets in prop::collection::vec(sheet(false), 1..=6),
    ) {
        let f = shape(&img);
        let initial = document_initial(img, &seed);
        let id = ImageId::from_raw(0);
        let cmds = resolves(f, id, &sheets);

        // Uno a uno.
        let mut one_by_one = initial.clone();
        let mut some = false;
        for c in &cmds {
            match one_by_one.apply(c.clone()) {
                Ok(change) => some |= change,
                Err(e) => {
                    return Err(TestCaseError::fail(format!(
                        "un comando válido falló: {e:?} ({c:?})"
                    )))
                }
            }
        }

        // Como un Batch.
        let mut with_batch = initial.clone();
        prop_assert_eq!(with_batch.apply(Command::Batch(cmds)), Ok(some));
        prop_assert_eq!(&with_batch, &one_by_one);

        if some {
            prop_assert!(with_batch.can_undo());
            prop_assert!(!with_batch.can_redo());
            prop_assert_eq!(steps_undoable(&with_batch), 1);

            prop_assert_eq!(with_batch.undo(), Ok(()));
            prop_assert_eq!(&with_batch, &initial);
            prop_assert!(!with_batch.can_undo());
            prop_assert!(with_batch.can_redo());

            prop_assert_eq!(with_batch.redo(), Ok(()));
            prop_assert_eq!(&with_batch, &one_by_one);
            prop_assert!(with_batch.can_undo());
            prop_assert!(!with_batch.can_redo());
        } else {
            prop_assert_eq!(state(&with_batch), state(&initial));
        }
    }
}

// Unidad 14c: límite de trozos por paso de relleno (specs/hito-2-limites-relleno.md, filas 6 a
// 10). Las imágenes miden 16384 x 368 o 16384 x 184 (RGBA, 24 y 12 MiB) y los pasos tienen
// hasta 3 millones de trozos, así que cada prueba cuesta unos segundos en debug.

mod limits {
    use super::*;

    /// Valor de la spec (D1). La única fuente es `Document::MAX_FILL_SPANS`; aquí se repite el
    /// número a propósito para que cambiarlo haga fallar la prueba.
    const CAP: usize = 3_000_000;

    const WIDTH: u32 = 16384;

    fn too_much() -> DocumentError {
        DocumentError::FillTooFragmented { limit: CAP }
    }

    /// Imagen RGBA de 16384 x `height` con un solo valor (`c0`, transparente).
    fn uniform(height: u32) -> Document {
        Document::new(Image::filled(WIDTH, height, Pixel::Rgba(c0())).unwrap())
    }

    /// Los `n` primeros tramos de un píxel en las columnas pares, en orden de lectura
    /// (8192 por fila): sobre una imagen de un solo valor, un trozo cada uno.
    fn spans_of_a_pixel(n: usize) -> Vec<Span> {
        (0..)
            .flat_map(|y| (0..WIDTH / 2).map(move |k| sp(y, 2 * k, 2 * k + 1)))
            .take(n)
            .collect()
    }

    /// Imagen RGBA de 16384 x `height` con `c2` en las columnas pares y `c3` en las impares.
    fn toggles(height: u32) -> Document {
        let (a, b) = (c2(), c3());
        let mut data = Vec::with_capacity(WIDTH as usize * height as usize * 4);
        for _ in 0..height {
            for x in 0..WIDTH {
                let c = if x % 2 == 0 { a } else { b };
                data.extend_from_slice(&[c.r, c.g, c.b, c.a]);
            }
        }
        let image_2 = Image::from_bytes(WIDTH, height, Mode::Rgba, data).unwrap();
        assert_eq!(image_2.get_pixel(0, 0), Ok(Pixel::Rgba(a)), "orden RGBA");
        assert_eq!(image_2.get_pixel(1, 0), Ok(Pixel::Rgba(b)));
        Document::new(image_2)
    }

    /// `n` tramos de fila completa (filas `0..n`).
    fn rows_complete(n: u32) -> Vec<Span> {
        (0..n).map(|y| sp(y, 0, WIDTH)).collect()
    }

    /// Lo observable y barato de comparar de un documento (no clona la historia entera).
    fn snapshot(doc: &Document) -> (Image, bool, bool, usize, u64) {
        (
            image_of(doc).clone(),
            doc.can_undo(),
            doc.can_redo(),
            doc.history_bytes(),
            doc.state_id(),
        )
    }

    /// Deja `doc` con un paso de deshacer y otro de rehacer (sobre columnas impares y sobre
    /// una fila a la que no llegan los tramos de las pruebas).
    fn with_undo_and_redo(doc: &mut Document) {
        let id = ImageId::from_raw(0);
        assert_eq!(doc.apply(set(id, &[px(1, 1, c2())])), Ok(true));
        assert_eq!(doc.apply(set(id, &[px(3, 1, c3())])), Ok(true));
        assert_eq!(doc.undo(), Ok(()));
        assert!(doc.can_undo() && doc.can_redo());
    }

    // Filas 6 y 7.

    #[test]
    fn row_06_and_07_exactly_max_pieces_fit_in_the_default_limit() {
        // Fila 6: la constante es única y el paso máximo que admite cabe en la historia.
        assert_eq!(Document::MAX_FILL_SPANS, 3_000_000);
        assert_eq!(Document::MAX_FILL_SPANS, CAP);

        let mut doc = uniform(368);
        let id = ImageId::from_raw(0);
        let original = image_of(&doc).clone();
        let spans = spans_of_a_pixel(Document::MAX_FILL_SPANS);
        assert_eq!(spans.len(), CAP);
        assert_eq!(doc.apply(fr(id, &spans, c1())), Ok(true));
        let bytes = doc.history_bytes();
        assert!(
            bytes <= Document::DEFAULT_HISTORY_LIMIT,
            "el paso máximo ocupa {bytes} bytes y el límite por defecto es {}",
            Document::DEFAULT_HISTORY_LIMIT
        );
        // Un trozo por tramo: no puede ser menos de 3 millones de trozos.
        assert!(bytes >= CAP, "{bytes} bytes para {CAP} trozos");
        let fills = image_of(&doc).clone();
        assert_eq!(fills.get_pixel(0, 0), Ok(Pixel::Rgba(c1())));
        assert_eq!(fills.get_pixel(1, 0), Ok(Pixel::Rgba(c0())));

        assert_eq!(doc.undo(), Ok(()));
        assert!(image_of(&doc) == &original, "undo restaura la imagen");
        assert_eq!(doc.redo(), Ok(()));
        assert!(image_of(&doc) == &fills);
    }

    #[test]
    fn row_07_one_piece_over_the_cap_is_rejected_without_touching_anything() {
        let mut doc = uniform(368);
        with_undo_and_redo(&mut doc);
        let id = ImageId::from_raw(0);
        let antes = snapshot(&doc);

        let spans = spans_of_a_pixel(CAP + 1);
        assert_eq!(spans.len(), CAP + 1);
        assert_eq!(doc.apply(fr(id, &spans, c1())), Err(too_much()));
        let after = snapshot(&doc);
        assert!(
            after == antes,
            "imagen, can_undo, can_redo, history_bytes y state_id iguales"
        );
        assert!(doc.can_redo(), "la pila de rehacer sigue viva");

        // `undo` y `redo` posteriores siguen sin error.
        assert_eq!(doc.redo(), Ok(()));
        assert_eq!(doc.undo(), Ok(()));
        assert_eq!(doc.undo(), Ok(()));
        assert_eq!(doc.redo(), Ok(()));
        assert_eq!(doc.redo(), Ok(()));
    }

    // Fila 8: pocos tramos, muchos trozos.

    #[test]
    fn row_08_counts_pieces_not_spans_184_rows_are_rejected() {
        // 184 tramos de 16384 píxeles, cada píxel distinto de `c1` y de su vecino: 3 014 656
        // trozos.
        let mut doc = toggles(184);
        let id = ImageId::from_raw(0);
        let antes = snapshot(&doc);
        assert_eq!(
            doc.apply(fr(id, &rows_complete(184), c1())),
            Err(too_much())
        );
        assert!(snapshot(&doc) == antes, "el documento no cambia");
    }

    #[test]
    fn row_08_183_rows_fit_and_undo_restores() {
        // 183 tramos: 2 998 272 trozos.
        let mut doc = toggles(184);
        let id = ImageId::from_raw(0);
        let original = image_of(&doc).clone();
        assert_eq!(doc.apply(fr(id, &rows_complete(183), c1())), Ok(true));
        assert!(doc.history_bytes() <= Document::DEFAULT_HISTORY_LIMIT);
        let img = image_of(&doc);
        assert_eq!(img.get_pixel(0, 0), Ok(Pixel::Rgba(c1())));
        assert_eq!(img.get_pixel(WIDTH - 1, 182), Ok(Pixel::Rgba(c1())));
        assert_eq!(
            img.get_pixel(0, 183),
            Ok(Pixel::Rgba(c2())),
            "la fila 183 no se toca"
        );
        assert_eq!(doc.undo(), Ok(()));
        assert!(image_of(&doc) == &original, "undo restaura la imagen");
    }

    // Fila 9.

    #[test]
    fn row_09_batch_with_a_too_fragmented_fill_is_reverted() {
        let mut doc = uniform(368);
        with_undo_and_redo(&mut doc);
        let id = ImageId::from_raw(0);
        let antes = snapshot(&doc);

        // `S` pinta una columna impar de la fila 0 (los tramos solo tocan columnas pares).
        let step = batch(vec![
            set(id, &[px(1, 0, c2())]),
            fr(id, &spans_of_a_pixel(CAP + 1), c1()),
        ]);
        assert_eq!(
            doc.apply(step),
            Err(in_batch(1, too_much())),
            "InBatch con index 1 y el error del relleno"
        );
        assert_eq!(
            pixel_en(&doc, 1, 0),
            Pixel::Rgba(c0()),
            "el píxel de S se revierte"
        );
        assert!(snapshot(&doc) == antes, "historia y estado intactos");
    }

    // Fila 10.

    #[test]
    fn row_10_display_source_eq_and_clone_of_the_error() {
        let e = DocumentError::FillTooFragmented { limit: CAP };
        let text = e.to_string();
        assert_eq!(text, "relleno cancelado: región demasiado fragmentada");
        assert_eq!(text.chars().count(), 47);
        for number in [
            "3000000",
            "3 000 000",
            "3_000_000",
            "3.000.000",
            "3,000,000",
        ] {
            assert!(!text.contains(number), "el texto no lleva el número");
        }
        assert!(e.source().is_none());
        assert_eq!(e.clone(), e);
        assert_ne!(e, DocumentError::FillTooFragmented { limit: 0 });
        assert_ne!(e, DocumentError::OutOfMemory);
        assert_error_traits::<DocumentError>();
        // Dentro de un Batch el texto del lote lo contiene y `source` es el error de dentro.
        let inside = in_batch(1, e.clone());
        assert!(inside.to_string().contains(&text));
        assert_eq!(inside.source().map(ToString::to_string), Some(text));
    }
}
