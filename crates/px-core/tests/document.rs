//! Tests de `Document` y los comandos con deshacer (specs/hito-1-document.md).
//!
//! Ningún test reserva más de unos pocos KiB: las imágenes miden como mucho 4 x 4 y las
//! historias tienen como mucho unas decenas de pasos. Para medir pasos en las pruebas
//! de límite solo se usan `history_bytes`, `history_limit`, `set_history_limit` y el
//! número de `undo()` que admite una copia: nunca se asume el tamaño exacto de un paso.

use std::error::Error;
use std::fmt::{Debug, Display};
use std::hash::Hash;

use proptest::prelude::*;
use proptest::test_runner::TestCaseError;
use px_core::{
    Command, Document, DocumentError, Image, ImageError, ImageId, Mode, Palette, Pixel, PixelEdit,
    Rgba,
};

// Comprobaciones en tiempo de compilación de los traits exigidos.
fn assert_id_traits<T: Clone + Copy + Debug + PartialEq + Eq + Hash + PartialOrd + Ord>() {}
fn assert_edit_traits<T: Clone + Copy + Debug + PartialEq + Eq>() {}
fn assert_datum_traits<T: Clone + Debug + PartialEq + Eq>() {}
fn assert_error_traits<T: Debug + Clone + PartialEq + Eq + Display + Error>() {}

#[test]
fn traits_derived() {
    assert_id_traits::<ImageId>();
    assert_edit_traits::<PixelEdit>();
    assert_datum_traits::<Command>();
    assert_datum_traits::<Document>();
    assert_error_traits::<DocumentError>();
}

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

fn px(x: u32, y: u32, c: Rgba) -> PixelEdit {
    PixelEdit {
        x,
        y,
        pixel: Pixel::Rgba(c),
    }
}

/// `S(...)` de la spec.
fn s(image: ImageId, pixels: &[PixelEdit]) -> Command {
    Command::SetPixels {
        image,
        pixels: pixels.to_vec(),
    }
}

fn batch(commands: Vec<Command>) -> Command {
    Command::Batch(commands)
}

fn pixel_en(doc: &Document, x: u32, y: u32) -> Pixel {
    doc.image(ImageId::from_raw(0))
        .unwrap()
        .get_pixel(x, y)
        .unwrap()
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

fn in_batch(index: usize, source: DocumentError) -> DocumentError {
    DocumentError::InBatch {
        index,
        source: Box::new(source),
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

/// Coste en bytes de la historia tras un `apply` de un solo píxel que cambia, medido en
/// un documento nuevo con el límite por defecto.
fn cost_of_a_step() -> usize {
    let mut doc = doc_d();
    let id = ImageId::from_raw(0);
    assert_eq!(doc.apply(s(id, &[px(0, 0, c1())])), Ok(true));
    doc.history_bytes()
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

// Identificadores, constantes y errores.

#[test]
fn image_id_round_trip_of_the_raw_value() {
    for raw in [0u32, 1, 99, u32::MAX] {
        assert_eq!(ImageId::from_raw(raw).raw(), raw);
    }
    assert_eq!(ImageId::from_raw(7), ImageId::from_raw(7));
    assert_ne!(ImageId::from_raw(7), ImageId::from_raw(8));
    assert!(ImageId::from_raw(1) < ImageId::from_raw(2));
}

#[test]
fn default_limit_is_64_mib() {
    assert_eq!(Document::DEFAULT_HISTORY_LIMIT, 64 << 20);
}

#[test]
fn commands_are_data_comparable() {
    let id = ImageId::from_raw(0);
    let a = s(id, &[px(0, 0, c1()), px(1, 0, c2())]);
    assert_eq!(a, a.clone());
    assert_ne!(a, s(id, &[px(0, 0, c1())]));
    assert_ne!(
        a,
        s(ImageId::from_raw(1), &[px(0, 0, c1()), px(1, 0, c2())])
    );
    assert_eq!(batch(vec![a.clone()]), batch(vec![a.clone()]));
    assert_ne!(batch(vec![a.clone()]), a);
    assert_eq!(px(1, 2, c1()), px(1, 2, c1()));
    assert_ne!(px(1, 2, c1()), px(2, 1, c1()));
}

#[test]
fn errors_with_display_and_source() {
    let internal = outside(9, 9, 4, 3);
    let with_source = [
        failure_pixel(0, internal.clone()),
        in_batch(0, DocumentError::NestedBatch),
    ];
    for e in &with_source {
        assert!(e.source().is_some(), "{e:?} debería exponer su source");
    }
    assert_eq!(
        with_source[0].source().map(|e| e.to_string()),
        Some(internal.to_string())
    );
    assert_eq!(
        with_source[1].source().map(|e| e.to_string()),
        Some(DocumentError::NestedBatch.to_string())
    );

    let all = [
        DocumentError::UnknownImage(ImageId::from_raw(99)),
        failure_pixel(1, internal),
        DocumentError::EmptyCommand,
        DocumentError::NestedBatch,
        in_batch(2, DocumentError::EmptyCommand),
        DocumentError::NothingToUndo,
        DocumentError::NothingToRedo,
        DocumentError::OutOfMemory,
    ];
    for e in &all {
        assert!(!e.to_string().is_empty(), "{e:?} sin mensaje");
    }
}

#[test]
fn document_equality_compares_content_and_not_history() {
    // Mismo contenido por caminos distintos.
    let mut a = doc_d();
    let mut b = doc_d();
    let id = ImageId::from_raw(0);
    assert_eq!(a.apply(s(id, &[px(0, 0, c1()), px(0, 0, c2())])), Ok(true));
    assert_eq!(b.apply(s(id, &[px(0, 0, c2())])), Ok(true));
    assert_eq!(a, b);
    assert_ne!(a.history_bytes(), 0);

    // Un documento con historia es igual a uno sin historia si el contenido coincide.
    let mut c = doc_d();
    assert_eq!(c.apply(s(id, &[px(0, 0, c2())])), Ok(true));
    assert_eq!(c.apply(s(id, &[px(0, 0, c0())])), Ok(true));
    assert_eq!(c, doc_d());

    // Contenido distinto: distintos.
    assert_ne!(a, doc_d());
    let other_image = Document::new(Image::new(4, 3, Mode::Gray).unwrap());
    assert_ne!(other_image, doc_d());
}

#[test]
fn history_limit_is_read_and_written() {
    let mut doc = doc_d();
    for limit in [123usize, 0, usize::MAX, 1] {
        doc.set_history_limit(limit);
        assert_eq!(doc.history_limit(), limit);
    }
}

// Una prueba por fila de la tabla de casos límite.

#[test]
fn row_new_leaves_the_document_without_history() {
    let img = Image::new(4, 3, Mode::Rgba).unwrap();
    let doc = Document::new(img.clone());
    assert_eq!(doc.image(ImageId::from_raw(0)), Some(&img));
    assert!(!doc.can_undo());
    assert!(!doc.can_redo());
    assert_eq!(doc.history_bytes(), 0);
    assert_eq!(doc.history_limit(), Document::DEFAULT_HISTORY_LIMIT);
}

#[test]
fn row_new_with_an_indexed_image() {
    let img = Image::new_indexed(2, 2, palette_of(3)).unwrap();
    let doc = Document::new(img.clone());
    assert_eq!(doc.image(ImageId::from_raw(0)), Some(&img));
    assert!(!doc.can_undo());
    assert!(!doc.can_redo());
    assert_eq!(doc.history_bytes(), 0);
    assert_eq!(doc.history_limit(), Document::DEFAULT_HISTORY_LIMIT);
}

#[test]
fn row_apply_undo_redo() {
    let mut doc = doc_d();
    let initial = doc.clone();
    let id = ImageId::from_raw(0);

    assert_eq!(doc.apply(s(id, &[px(0, 0, c1())])), Ok(true));
    assert_eq!(pixel_en(&doc, 0, 0), Pixel::Rgba(c1()));
    assert!(doc.can_undo());
    assert!(!doc.can_redo());
    let after_apply = doc.clone();

    assert_eq!(doc.undo(), Ok(()));
    assert_eq!(doc, initial);
    assert!(doc.can_redo());
    assert!(!doc.can_undo());

    assert_eq!(doc.redo(), Ok(()));
    assert_eq!(doc, after_apply);
    assert!(doc.can_undo());
    assert!(!doc.can_redo());
}

#[test]
fn row_apply_new_clears_the_redo_stack() {
    let mut doc = doc_d();
    let id = ImageId::from_raw(0);
    let a = px(0, 0, c1());
    let b = px(1, 0, c2());

    assert_eq!(doc.apply(s(id, &[a])), Ok(true));
    assert_eq!(doc.undo(), Ok(()));
    assert!(doc.can_redo());

    assert_eq!(doc.apply(s(id, &[b])), Ok(true));
    assert!(!doc.can_redo());
    assert_eq!(doc.redo(), Err(DocumentError::NothingToRedo));

    assert_eq!(pixel_en(&doc, 1, 0), Pixel::Rgba(c2()));
    assert_eq!(pixel_en(&doc, 0, 0), Pixel::Rgba(c0()));
    let mut expected = doc_d();
    assert_eq!(expected.apply(s(id, &[b])), Ok(true));
    assert_eq!(doc, expected);
}

#[test]
fn row_undo_and_redo_on_a_new_document() {
    let mut doc = doc_d();
    let before = state(&doc);
    assert_eq!(doc.undo(), Err(DocumentError::NothingToUndo));
    assert_eq!(doc.redo(), Err(DocumentError::NothingToRedo));
    assert_eq!(state(&doc), before);
}

#[test]
fn row_repeated_coordinates_in_the_same_set_pixels() {
    // Gana la última escritura; deshacer restaura el valor anterior al comando.
    let mut doc = doc_d();
    let initial = doc.clone();
    let id = ImageId::from_raw(0);
    assert_eq!(
        doc.apply(s(id, &[px(0, 0, c1()), px(0, 0, c2())])),
        Ok(true)
    );
    assert_eq!(pixel_en(&doc, 0, 0), Pixel::Rgba(c2()));
    assert_eq!(doc.undo(), Ok(()));
    assert_eq!(pixel_en(&doc, 0, 0), Pixel::Rgba(c0()));
    assert_eq!(doc, initial);
    assert!(!doc.can_undo());
}

#[test]
fn row_a_write_that_changes_and_restores_counts_as_a_change() {
    let mut doc = doc_d();
    let initial = doc.clone();
    let id = ImageId::from_raw(0);
    assert_eq!(
        doc.apply(s(id, &[px(0, 0, c1()), px(0, 0, c0())])),
        Ok(true)
    );
    assert_eq!(pixel_en(&doc, 0, 0), Pixel::Rgba(c0()));
    // El contenido es el mismo, pero hay un paso (de dos cambios).
    assert_eq!(doc, initial);
    assert!(doc.can_undo());
    assert!(doc.history_bytes() > 0);
    assert_eq!(steps_undoable(&doc), 1);

    assert_eq!(doc.undo(), Ok(()));
    assert_eq!(doc, initial);
    assert!(!doc.can_undo());
    assert!(doc.can_redo());
}

#[test]
fn row_empty_commands_are_empty_command() {
    // En un documento nuevo.
    let mut doc = doc_d();
    let id = ImageId::from_raw(0);
    let before = state(&doc);
    assert_eq!(doc.apply(s(id, &[])), Err(DocumentError::EmptyCommand));
    assert_eq!(doc.apply(batch(vec![])), Err(DocumentError::EmptyCommand));
    assert_eq!(state(&doc), before);

    // Con historia y con algo que rehacer: nada se toca.
    assert_eq!(doc.apply(s(id, &[px(0, 0, c1())])), Ok(true));
    assert_eq!(doc.apply(s(id, &[px(1, 0, c1())])), Ok(true));
    assert_eq!(doc.undo(), Ok(()));
    let before = state(&doc);
    assert!(before.can_undo && before.can_redo);
    assert_eq!(doc.apply(s(id, &[])), Err(DocumentError::EmptyCommand));
    assert_eq!(doc.apply(batch(vec![])), Err(DocumentError::EmptyCommand));
    assert_eq!(state(&doc), before);
}

#[test]
fn row_out_of_range() {
    let mut doc = doc_d();
    let id = ImageId::from_raw(0);
    let before = state(&doc);

    assert_eq!(
        doc.apply(s(id, &[px(4, 0, c1())])),
        Err(failure_pixel(0, outside(4, 0, 4, 3)))
    );
    assert_eq!(state(&doc), before);

    assert_eq!(
        doc.apply(s(id, &[px(0, 0, c1()), px(9, 9, c1())])),
        Err(failure_pixel(1, outside(9, 9, 4, 3)))
    );
    assert_eq!(pixel_en(&doc, 0, 0), Pixel::Rgba(c0()));
    assert!(!doc.can_undo());
    assert_eq!(state(&doc), before);
}

#[test]
fn row_failure_after_repeated_writes_restores_the_original_value() {
    let mut doc = doc_d();
    let id = ImageId::from_raw(0);
    assert_eq!(doc.apply(s(id, &[px(0, 0, c3())])), Ok(true));
    let before = state(&doc);

    let r = doc.apply(s(
        id,
        &[
            px(0, 0, c1()),
            px(0, 0, c2()),
            px(1, 1, c1()),
            px(4, 3, c1()),
        ],
    ));
    assert_eq!(r, Err(failure_pixel(3, outside(4, 3, 4, 3))));
    assert_eq!(pixel_en(&doc, 0, 0), Pixel::Rgba(c3()));
    assert_eq!(state(&doc), before);
}

#[test]
fn row_other_mode_and_index_outside_the_palette() {
    // Gray(1) en una imagen RGBA.
    let mut doc = doc_d();
    let id = ImageId::from_raw(0);
    let before = state(&doc);
    let gray = PixelEdit {
        x: 0,
        y: 0,
        pixel: Pixel::Gray(1),
    };
    assert_eq!(
        doc.apply(s(id, &[gray])),
        Err(failure_pixel(
            0,
            ImageError::ModeMismatch {
                expected: Mode::Rgba,
                found: Mode::Gray
            }
        ))
    );
    assert_eq!(state(&doc), before);

    // Indexed(9) en una imagen indexada con paleta de 2.
    let mut doc = Document::new(Image::new_indexed(4, 3, palette_of(2)).unwrap());
    let id = ImageId::from_raw(0);
    let before = state(&doc);
    let index_2 = PixelEdit {
        x: 0,
        y: 0,
        pixel: Pixel::Indexed(9),
    };
    assert_eq!(
        doc.apply(s(id, &[index_2])),
        Err(failure_pixel(
            0,
            ImageError::IndexNotInPalette { index: 9, len: 2 }
        ))
    );
    assert_eq!(state(&doc), before);
}

#[test]
fn row_image_unknown() {
    let mut doc = doc_d();
    let unknown = ImageId::from_raw(99);
    let before = state(&doc);
    assert_eq!(doc.image(unknown), None);
    assert_eq!(
        doc.apply(s(unknown, &[px(0, 0, c1())])),
        Err(DocumentError::UnknownImage(unknown))
    );
    assert_eq!(doc.image(unknown), None);
    assert_eq!(state(&doc), before);
}

#[test]
fn check_order_of_set_pixels() {
    let mut doc = doc_d();
    let unknown = ImageId::from_raw(99);
    let before = state(&doc);

    // EmptyCommand gana a UnknownImage.
    assert_eq!(doc.apply(s(unknown, &[])), Err(DocumentError::EmptyCommand));
    // UnknownImage gana a los fallos de píxel.
    assert_eq!(
        doc.apply(s(unknown, &[px(9, 9, c1())])),
        Err(DocumentError::UnknownImage(unknown))
    );
    assert_eq!(state(&doc), before);
}

#[test]
fn row_invalid_command_after_undo_keeps_redo() {
    let id = ImageId::from_raw(0);
    let invalid_2 = [
        s(id, &[]),
        s(id, &[px(4, 0, c1())]),
        s(ImageId::from_raw(99), &[px(0, 0, c1())]),
        batch(vec![]),
        batch(vec![s(id, &[px(0, 0, c1())]), batch(vec![])]),
    ];
    for invalid in invalid_2 {
        let mut doc = doc_d();
        assert_eq!(doc.apply(s(id, &[px(0, 0, c2())])), Ok(true));
        let with_a = doc.clone();
        assert_eq!(doc.undo(), Ok(()));
        let before = state(&doc);

        assert!(doc.apply(invalid.clone()).is_err(), "{invalid:?}");
        assert!(doc.can_redo(), "{invalid:?}");
        assert_eq!(state(&doc), before, "{invalid:?}");
        assert_eq!(doc.redo(), Ok(()));
        assert_eq!(doc, with_a);
    }
}

#[test]
fn row_writing_the_value_already_there_changes_nothing() {
    let mut doc = doc_d();
    let id = ImageId::from_raw(0);
    let before = state(&doc);
    assert_eq!(doc.apply(s(id, &[px(0, 0, c0())])), Ok(false));
    assert!(!doc.can_undo());
    assert_eq!(doc.history_bytes(), 0);
    assert_eq!(state(&doc), before);
}

#[test]
fn row_no_op_after_undo_keeps_redo() {
    let mut doc = doc_d();
    let id = ImageId::from_raw(0);
    assert_eq!(doc.apply(s(id, &[px(0, 0, c1())])), Ok(true));
    let with_a = doc.clone();
    assert_eq!(doc.undo(), Ok(()));
    let before = state(&doc);
    assert!(before.can_redo);

    assert_eq!(doc.apply(s(id, &[px(0, 0, c0())])), Ok(false));
    assert!(doc.can_redo());
    assert_eq!(doc.can_undo(), before.can_undo);
    assert_eq!(doc.history_bytes(), before.bytes);
    assert_eq!(state(&doc), before);

    assert_eq!(doc.redo(), Ok(()));
    assert_eq!(doc, with_a);
}

#[test]
fn row_batch_is_a_single_step() {
    let mut doc = doc_d();
    let initial = doc.clone();
    let id = ImageId::from_raw(0);
    let command = batch(vec![
        s(id, &[px(0, 0, c1())]),
        s(id, &[px(1, 0, c2())]),
        s(id, &[px(2, 0, c3())]),
    ]);

    assert_eq!(doc.apply(command), Ok(true));
    assert_eq!(pixel_en(&doc, 0, 0), Pixel::Rgba(c1()));
    assert_eq!(pixel_en(&doc, 1, 0), Pixel::Rgba(c2()));
    assert_eq!(pixel_en(&doc, 2, 0), Pixel::Rgba(c3()));
    assert_eq!(steps_undoable(&doc), 1);
    let after_apply = doc.clone();

    assert_eq!(doc.undo(), Ok(()));
    assert_eq!(doc, initial);
    assert!(!doc.can_undo());

    assert_eq!(doc.redo(), Ok(()));
    assert_eq!(doc, after_apply);
    assert_eq!(pixel_en(&doc, 0, 0), Pixel::Rgba(c1()));
    assert_eq!(pixel_en(&doc, 1, 0), Pixel::Rgba(c2()));
    assert_eq!(pixel_en(&doc, 2, 0), Pixel::Rgba(c3()));
}

#[test]
fn row_batch_with_no_ops() {
    let mut doc = doc_d();
    let id = ImageId::from_raw(0);
    let no_op = s(id, &[px(3, 2, c0())]);
    let a = s(id, &[px(0, 0, c1())]);

    // Un no-op y un cambio: un paso que guarda solo el cambio de `a`.
    assert_eq!(doc.apply(batch(vec![no_op.clone(), a.clone()])), Ok(true));
    assert_eq!(steps_undoable(&doc), 1);
    let mut only_a = doc_d();
    assert_eq!(only_a.apply(a), Ok(true));
    assert_eq!(doc, only_a);
    assert_eq!(doc.history_bytes(), only_a.history_bytes());

    // Solo no-ops: sin paso y sin tocar la historia.
    let mut other = doc_d();
    let before = state(&other);
    assert_eq!(other.apply(batch(vec![no_op.clone(), no_op])), Ok(false));
    assert!(!other.can_undo());
    assert_eq!(other.history_bytes(), 0);
    assert_eq!(state(&other), before);
}

#[test]
fn row_batch_with_an_inner_failure_changes_nothing() {
    let mut doc = doc_d();
    let id = ImageId::from_raw(0);
    assert_eq!(doc.apply(s(id, &[px(3, 2, c2())])), Ok(true));
    let before = state(&doc);

    // Fuera de rango en el comando 1 (la escritura (1, 0) anterior al fallo se revierte).
    let out_of_range = batch(vec![
        s(id, &[px(0, 0, c1())]),
        s(id, &[px(1, 0, c1()), px(9, 9, c1())]),
        s(id, &[px(2, 0, c1())]),
    ]);
    assert_eq!(
        doc.apply(out_of_range),
        Err(in_batch(1, failure_pixel(1, outside(9, 9, 4, 3))))
    );
    assert_eq!(pixel_en(&doc, 0, 0), Pixel::Rgba(c0()));
    assert_eq!(pixel_en(&doc, 1, 0), Pixel::Rgba(c0()));
    assert_eq!(pixel_en(&doc, 2, 0), Pixel::Rgba(c0()));
    assert_eq!(state(&doc), before);

    // Batch anidado en el comando 1.
    let nested = batch(vec![
        s(id, &[px(0, 0, c1())]),
        batch(vec![s(id, &[px(1, 0, c1())])]),
    ]);
    assert_eq!(
        doc.apply(nested),
        Err(in_batch(1, DocumentError::NestedBatch))
    );
    assert_eq!(pixel_en(&doc, 0, 0), Pixel::Rgba(c0()));
    assert_eq!(state(&doc), before);
}

#[test]
fn batch_with_empty_command_or_unknown_image_inside() {
    let mut doc = doc_d();
    let id = ImageId::from_raw(0);
    let before = state(&doc);

    assert_eq!(
        doc.apply(batch(vec![s(id, &[px(0, 0, c1())]), s(id, &[])])),
        Err(in_batch(1, DocumentError::EmptyCommand))
    );
    assert_eq!(state(&doc), before);

    let unknown = ImageId::from_raw(99);
    assert_eq!(
        doc.apply(batch(vec![
            s(id, &[px(0, 0, c1())]),
            s(unknown, &[px(0, 0, c2())])
        ])),
        Err(in_batch(1, DocumentError::UnknownImage(unknown)))
    );
    assert_eq!(state(&doc), before);

    // Un lote vacío dentro de un lote es EmptyCommand en el comando interno, y un
    // lote dentro de otro, aunque esté vacío, es NestedBatch o EmptyCommand según
    // el orden de comprobación de la spec (no está fijado): solo se comprueba el índice
    // y que no se toca nada.
    let r = doc.apply(batch(vec![s(id, &[px(0, 0, c1())]), batch(vec![])]));
    assert!(
        matches!(r, Err(DocumentError::InBatch { index: 1, .. })),
        "salió {r:?}"
    );
    assert_eq!(state(&doc), before);
}

#[test]
fn row_limit_zero_keeps_only_the_last_step() {
    let mut doc = doc_d();
    let id = ImageId::from_raw(0);
    doc.set_history_limit(0);
    for x in 0..3 {
        assert_eq!(doc.apply(s(id, &[px(x, 0, c1())])), Ok(true));
    }
    assert_eq!(steps_undoable(&doc), 1);
    assert_eq!(doc.history_bytes(), cost_of_a_step());

    assert_eq!(doc.undo(), Ok(()));
    assert_eq!(doc.undo(), Err(DocumentError::NothingToUndo));
    assert_eq!(pixel_en(&doc, 0, 0), Pixel::Rgba(c1()));
    assert_eq!(pixel_en(&doc, 1, 0), Pixel::Rgba(c1()));
    assert_eq!(pixel_en(&doc, 2, 0), Pixel::Rgba(c0()));
}

#[test]
fn row_exact_limit_for_two_steps() {
    let mut doc = doc_d();
    let id = ImageId::from_raw(0);
    assert_eq!(doc.apply(s(id, &[px(0, 0, c1())])), Ok(true));
    assert_eq!(doc.apply(s(id, &[px(1, 0, c1())])), Ok(true));
    let two = doc.history_bytes();
    assert!(two > 0);

    // Un límite igual al tamaño de la pila no descarta nada (solo si es mayor).
    doc.set_history_limit(two);
    assert_eq!(doc.history_limit(), two);
    assert_eq!(steps_undoable(&doc), 2);
    assert_eq!(doc.history_bytes(), two);

    // El tercero hace que se descarte el más antiguo.
    assert_eq!(doc.apply(s(id, &[px(2, 0, c1())])), Ok(true));
    assert_eq!(steps_undoable(&doc), 2);
    assert!(doc.history_bytes() <= two);
    let mut copy = doc.clone();
    assert_eq!(copy.undo(), Ok(()));
    assert_eq!(copy.undo(), Ok(()));
    assert_eq!(copy.undo(), Err(DocumentError::NothingToUndo));
    let mut with_the_first = doc_d();
    assert_eq!(with_the_first.apply(s(id, &[px(0, 0, c1())])), Ok(true));
    assert_eq!(copy, with_the_first);

    // Con límite 0 queda solo el último.
    doc.set_history_limit(0);
    assert_eq!(steps_undoable(&doc), 1);
    assert_eq!(doc.history_bytes(), cost_of_a_step());
    assert_eq!(doc.undo(), Ok(()));
    assert_eq!(doc.undo(), Err(DocumentError::NothingToUndo));
    assert_eq!(pixel_en(&doc, 1, 0), Pixel::Rgba(c1()));
    assert_eq!(pixel_en(&doc, 2, 0), Pixel::Rgba(c0()));
}

#[test]
fn row_a_step_bigger_than_the_limit_is_kept() {
    let mut doc = doc_d();
    let initial = doc.clone();
    let id = ImageId::from_raw(0);
    doc.set_history_limit(1);
    let large: Vec<PixelEdit> = (0..4).map(|x| px(x, 0, c1())).collect();

    assert_eq!(doc.apply(s(id, &large)), Ok(true));
    assert!(doc.history_bytes() > doc.history_limit());
    assert_eq!(steps_undoable(&doc), 1);
    assert_eq!(doc.undo(), Ok(()));
    assert_eq!(doc, initial);
    assert_eq!(doc.undo(), Err(DocumentError::NothingToUndo));

    // Con otro paso encima, el grande (el más antiguo) se descarta y el nuevo se conserva.
    assert_eq!(doc.redo(), Ok(()));
    assert_eq!(doc.apply(s(id, &[px(0, 1, c2())])), Ok(true));
    assert_eq!(steps_undoable(&doc), 1);
    assert_eq!(doc.undo(), Ok(()));
    assert_eq!(pixel_en(&doc, 0, 1), Pixel::Rgba(c0()));
    assert_eq!(pixel_en(&doc, 3, 0), Pixel::Rgba(c1()));
}

#[test]
fn lowering_the_limit_does_not_clear_the_redo_stack() {
    let mut doc = doc_d();
    let id = ImageId::from_raw(0);
    for x in 0..3 {
        assert_eq!(doc.apply(s(id, &[px(x, 0, c1())])), Ok(true));
    }
    assert_eq!(doc.undo(), Ok(()));
    assert_eq!(doc.undo(), Ok(()));
    assert_eq!(steps_redoable(&doc), 2);

    doc.set_history_limit(0);
    assert_eq!(steps_redoable(&doc), 2);
    assert!(doc.can_redo());
    assert_eq!(steps_undoable(&doc), 1);
}

// Estrategias.

fn mode_2() -> impl Strategy<Value = Mode> {
    prop::sample::select(ALL_2.to_vec())
}

/// Imagen de hasta 4 x 4 en cualquier modo; el indexado lleva una paleta de 1 a 8 entradas.
fn image_small() -> impl Strategy<Value = Image> {
    (mode_2(), 1u32..=4, 1u32..=4, 1usize..=8).prop_map(|(m, w, h, n)| creates(m, w, h, n))
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
fn pixel_de(mode: Mode, colors_2: usize, value: u8) -> Pixel {
    match mode {
        Mode::Rgba if value == 0 => Pixel::Rgba(Rgba::TRANSPARENT),
        Mode::Rgba => Pixel::Rgba(Rgba::new(value, 255 - value, value ^ 5, 255)),
        Mode::Indexed => Pixel::Indexed((value as usize % colors_2.max(1)) as u8),
        Mode::Gray => Pixel::Gray(value),
        Mode::GrayAlpha if value == 0 => Pixel::GrayAlpha { gray: 0, alpha: 0 },
        Mode::GrayAlpha => Pixel::GrayAlpha {
            gray: value,
            alpha: 255,
        },
    }
}

fn other_mode(mode: Mode, k: u8) -> Mode {
    let i = ALL_2.iter().position(|&m| m == mode).unwrap();
    ALL_2[(i + 1 + k as usize % 3) % ALL_2.len()]
}

/// Fallos inyectables en un píxel.
#[derive(Clone, Copy, Debug)]
enum Fail {
    OutsideX,
    OutsideY,
    OtherMode,
    IndexOutside,
}

/// Una escritura en bruto: coordenadas sin acotar, un valor pequeño y un posible fallo.
#[derive(Clone, Copy, Debug)]
struct EditingRaw {
    xb: u32,
    yb: u32,
    value: u8,
    fail: Option<Fail>,
}

/// `SetPixels` en bruto: puede apuntar a un id desconocido y tener la lista vacía.
#[derive(Clone, Debug)]
struct SetRaw {
    unknown: Option<u32>,
    edits: Vec<EditingRaw>,
}

#[derive(Clone, Debug)]
enum SubRaw {
    Set(SetRaw),
    Nested(Vec<SetRaw>),
}

#[derive(Clone, Debug)]
enum CmdRaw {
    Set(SetRaw),
    Batch(Vec<SubRaw>),
}

fn fail(with_failures: bool) -> impl Strategy<Value = Option<Fail>> {
    let mut options: Vec<Option<Fail>> = vec![None];
    if with_failures {
        options.extend([None; 29]);
        options.extend([
            Some(Fail::OutsideX),
            Some(Fail::OutsideY),
            Some(Fail::OtherMode),
            Some(Fail::IndexOutside),
        ]);
    }
    prop::sample::select(options)
}

fn editing_raw(with_failures: bool) -> impl Strategy<Value = EditingRaw> {
    (any::<u32>(), any::<u32>(), 0u8..4, fail(with_failures)).prop_map(|(xb, yb, value, fail)| {
        EditingRaw {
            xb,
            yb,
            value,
            fail,
        }
    })
}

/// `SetPixels` en bruto. Sin fallos: lista de 1 a 5 escrituras válidas (con coordenadas
/// repetidas y no-ops). Con fallos: además listas vacías, ids desconocidos y escrituras
/// fuera de la imagen, de otro modo o con índice fuera de la paleta.
fn set_raw(with_failures: bool) -> impl Strategy<Value = SetRaw> {
    let unknown = if with_failures {
        prop_oneof![
            12 => Just(None),
            1 => (1u32..=u32::MAX).prop_map(Some),
        ]
        .boxed()
    } else {
        Just(None).boxed()
    };
    let edits = if with_failures {
        prop_oneof![
            1 => Just(Vec::<EditingRaw>::new()),
            10 => prop::collection::vec(editing_raw(true), 1..=5),
        ]
        .boxed()
    } else {
        prop::collection::vec(editing_raw(false), 1..=5).boxed()
    };
    (unknown, edits).prop_map(|(unknown, edits)| SetRaw { unknown, edits })
}

fn sub_raw(with_failures: bool) -> impl Strategy<Value = SubRaw> {
    if with_failures {
        prop_oneof![
            12 => set_raw(true).prop_map(SubRaw::Set),
            1 => prop::collection::vec(set_raw(true), 1..=2).prop_map(SubRaw::Nested),
        ]
        .boxed()
    } else {
        set_raw(false).prop_map(SubRaw::Set).boxed()
    }
}

/// `SetPixels` (tres de cada cuatro) o `Batch` de 0 a 4 comandos, con fallos o sin ellos.
fn cmd_raw(with_failures: bool) -> impl Strategy<Value = CmdRaw> {
    let length = if with_failures { 0..=4 } else { 1..=4 };
    prop_oneof![
        3 => set_raw(with_failures).prop_map(CmdRaw::Set),
        1 => prop::collection::vec(sub_raw(with_failures), length).prop_map(CmdRaw::Batch),
    ]
}

fn editing(f: Shape, e: &EditingRaw) -> PixelEdit {
    let mut x = e.xb % f.width;
    let mut y = e.yb % f.height;
    let mut pixel = pixel_de(f.mode, f.colors_2, e.value);
    match e.fail {
        None => {}
        Some(Fail::OutsideX) => x = f.width + e.xb % 1000,
        Some(Fail::OutsideY) => y = f.height + e.yb % 1000,
        Some(Fail::OtherMode) => pixel = pixel_de(other_mode(f.mode, e.value), 8, e.value),
        // En una imagen no indexada esto es un `ModeMismatch`.
        Some(Fail::IndexOutside) => pixel = Pixel::Indexed((f.colors_2 + e.value as usize) as u8),
    }
    PixelEdit { x, y, pixel }
}

fn set_cmd(f: Shape, id: ImageId, raw: &SetRaw) -> Command {
    Command::SetPixels {
        image: raw.unknown.map_or(id, ImageId::from_raw),
        pixels: raw.edits.iter().map(|e| editing(f, e)).collect(),
    }
}

fn command(f: Shape, id: ImageId, raw: &CmdRaw) -> Command {
    match raw {
        CmdRaw::Set(s) => set_cmd(f, id, s),
        CmdRaw::Batch(subs) => Command::Batch(
            subs.iter()
                .map(|sub| match sub {
                    SubRaw::Set(s) => set_cmd(f, id, s),
                    SubRaw::Nested(v) => {
                        Command::Batch(v.iter().map(|s| set_cmd(f, id, s)).collect())
                    }
                })
                .collect(),
        ),
    }
}

// Comandos totalmente arbitrarios (propiedad 3).

/// Escritura con coordenadas y píxel arbitrarios; `own` la sustituye por un píxel
/// del modo de la imagen para que los comandos también fallen a mitad de camino.
#[derive(Clone, Copy, Debug)]
struct ArbEdit {
    x: u32,
    y: u32,
    pixel: Pixel,
    own: Option<u8>,
}

#[derive(Clone, Debug)]
enum ArbRaw {
    Set(Option<u32>, Vec<ArbEdit>),
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
        6 => 0u32..5,
        1 => any::<u32>(),
        1 => Just(u32::MAX),
    ]
}

fn arb_edit() -> impl Strategy<Value = ArbEdit> {
    (
        coordinate_arbitrary(),
        coordinate_arbitrary(),
        pixel_arbitrary(),
        prop_oneof![2 => Just(None), 3 => (0u8..4).prop_map(Some)],
    )
        .prop_map(|(x, y, pixel, own)| ArbEdit { x, y, pixel, own })
}

fn arb_cmd() -> impl Strategy<Value = ArbRaw> {
    let sheet = (
        prop_oneof![4 => Just(None), 1 => any::<u32>().prop_map(Some)],
        prop::collection::vec(arb_edit(), 0..=5),
    )
        .prop_map(|(id, edits)| ArbRaw::Set(id, edits));
    sheet.prop_recursive(3, 24, 4, |internal| {
        prop::collection::vec(internal, 0..=4).prop_map(ArbRaw::Batch)
    })
}

fn arb_command(f: Shape, id: ImageId, raw: &ArbRaw) -> Command {
    match raw {
        ArbRaw::Set(unknown, edits) => Command::SetPixels {
            image: unknown.map_or(id, ImageId::from_raw),
            pixels: edits
                .iter()
                .map(|e| PixelEdit {
                    x: e.x,
                    y: e.y,
                    pixel: e.own.map_or(e.pixel, |v| pixel_de(f.mode, f.colors_2, v)),
                })
                .collect(),
        },
        ArbRaw::Batch(commands) => {
            Command::Batch(commands.iter().map(|c| arb_command(f, id, c)).collect())
        }
    }
}

// Modelo de referencia (propiedades 2 y 5).

/// Aplica un `SetPixels` sobre una copia con `Image::set_pixel` y dice si cambió algo.
/// "Cambió" es que alguna escritura modificó el valor que había en ese momento.
fn model_set(
    img: &mut Image,
    id: ImageId,
    image: ImageId,
    pixels: &[PixelEdit],
) -> Result<bool, DocumentError> {
    if pixels.is_empty() {
        return Err(DocumentError::EmptyCommand);
    }
    if image != id {
        return Err(DocumentError::UnknownImage(image));
    }
    let mut change = false;
    for (i, e) in pixels.iter().enumerate() {
        let before = img.get_pixel(e.x, e.y);
        img.set_pixel(e.x, e.y, e.pixel)
            .map_err(|source| DocumentError::Pixel { pixel: i, source })?;
        if before != Ok(e.pixel) {
            change = true;
        }
    }
    Ok(change)
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
        Command::SetPixels { image, pixels } => {
            let change = model_set(&mut work, id, *image, pixels)?;
            Ok((work, change))
        }
        Command::Batch(commands) => {
            if commands.is_empty() {
                return Err(DocumentError::EmptyCommand);
            }
            let mut change = false;
            for (index, c) in commands.iter().enumerate() {
                match c {
                    Command::SetPixels { image, pixels } => {
                        change |= model_set(&mut work, id, *image, pixels)
                            .map_err(|e| in_batch(index, e))?;
                    }
                    Command::Batch(_) => return Err(in_batch(index, DocumentError::NestedBatch)),
                    other => unreachable!("comando sin modelo: {other:?}"),
                }
            }
            Ok((work, change))
        }
        other => unreachable!("comando sin modelo: {other:?}"),
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
    Apply(CmdRaw),
    Undo,
    Redo,
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        3 => cmd_raw(false).prop_map(Op::Apply),
        2 => cmd_raw(true).prop_map(Op::Apply),
        2 => Just(Op::Undo),
        2 => Just(Op::Redo),
    ]
}

/// Ejecuta `ops` sobre un documento y sobre el modelo (instantáneas de la imagen y un
/// cursor) y los compara tras cada operación.
fn runs_model(img: Image, ops: &[Op]) -> Result<(), TestCaseError> {
    let f = shape(&img);
    let mut doc = Document::new(img.clone());
    let id = ImageId::from_raw(0);
    let mut instants = vec![img];
    let mut cursor = 0usize;

    for op in ops {
        let before = state(&doc);
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
                        prop_assert_eq!(state(&doc), before);
                    }
                    Err(e) => {
                        prop_assert_eq!(real, Err(e), "{:?}", cmd);
                        prop_assert_eq!(state(&doc), before);
                    }
                }
            }
            Op::Undo => {
                if cursor > 0 {
                    prop_assert_eq!(doc.undo(), Ok(()));
                    cursor -= 1;
                } else {
                    prop_assert_eq!(doc.undo(), Err(DocumentError::NothingToUndo));
                    prop_assert_eq!(state(&doc), before);
                }
            }
            Op::Redo => {
                if cursor + 1 < instants.len() {
                    prop_assert_eq!(doc.redo(), Ok(()));
                    cursor += 1;
                } else {
                    prop_assert_eq!(doc.redo(), Err(DocumentError::NothingToRedo));
                    prop_assert_eq!(state(&doc), before);
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
    Apply(ArbRaw),
    Undo,
    Redo,
    Limit(usize),
}

fn op_arbitrary() -> impl Strategy<Value = OpArb> {
    prop_oneof![
        5 => arb_cmd().prop_map(OpArb::Apply),
        1 => Just(OpArb::Undo),
        1 => Just(OpArb::Redo),
        1 => prop_oneof![
            any::<usize>(),
            0usize..4096,
            Just(0usize),
            Just(usize::MAX),
        ]
        .prop_map(OpArb::Limit),
    ]
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

/// Comprobaciones de la propiedad 5 sobre un documento que solo ha recibido `apply`
/// (y quizá cambios de límite): `instants[0]` es el inicial y cada `Ok(true)` añadió uno.
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
    let mut copy = doc.clone();
    prop_assert_eq!(undoes_all(&mut copy, kept), kept);
    prop_assert_eq!(copy.undo(), Err(DocumentError::NothingToUndo));
    checks_content(&copy, &instants[created - kept])?;

    // Rehacer todo devuelve el final.
    prop_assert_eq!(redoes_all(&mut copy, kept), kept);
    prop_assert_eq!(copy.redo(), Err(DocumentError::NothingToRedo));
    checks_content(&copy, &instants[created])?;
    Ok(())
}

/// Aplica `cmds` (válidos) a `doc`, comprueba cada resultado contra el modelo y añade a
/// `instants` una imagen por cada `Ok(true)`. Si hay `limit`, tras cada `apply` comprueba
/// la propiedad 5.
fn applies_valid(
    doc: &mut Document,
    instants: &mut Vec<Image>,
    f: Shape,
    cmds: &[CmdRaw],
    limit: Option<usize>,
) -> Result<(), TestCaseError> {
    let id = ImageId::from_raw(0);
    for raw in cmds {
        let cmd = command(f, id, raw);
        let actual = instants.last().unwrap().clone();
        let (new, change) = match model_apply(&actual, id, &cmd) {
            Ok(r) => r,
            Err(e) => {
                return Err(TestCaseError::fail(format!(
                    "el generador sin fallos produjo un comando inválido: {e:?}"
                )));
            }
        };
        prop_assert_eq!(doc.apply(cmd), Ok(change));
        if change {
            instants.push(new);
        }
        if let Some(limit) = limit {
            checks_limit(doc, instants, limit)?;
        }
    }
    Ok(())
}

// Propiedades.

proptest! {
    // Propiedad 1: deshacer todo y rehacer todo.
    #[test]
    fn undo_all_and_redo_all(
        img in image_small(),
        cmds in prop::collection::vec(cmd_raw(true), 0..=30),
    ) {
        let f = shape(&img);
        let initial = Document::new(img);
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
    fn history_against_a_model(
        img in image_small(),
        ops in prop::collection::vec(op(), 0..=40),
    ) {
        runs_model(img, &ops)?;
    }

    // Propiedad 3: atomicidad y ausencia de pánicos con comandos arbitrarios.
    #[test]
    fn atomicity_and_no_panics(
        img in image_small(),
        seed in prop::collection::vec(cmd_raw(false), 0..=4),
        ops in prop::collection::vec(op_arbitrary(), 0..=20),
    ) {
        let f = shape(&img);
        let mut doc = Document::new(img);
        let id = ImageId::from_raw(0);
        for raw in &seed {
            let _ = doc.apply(command(f, id, raw));
        }

        for op in &ops {
            let before = state(&doc);
            match op {
                OpArb::Apply(raw) => {
                    let cmd = arb_command(f, id, raw);
                    match doc.apply(cmd.clone()) {
                        Ok(true) => {
                            prop_assert!(doc.can_undo());
                            prop_assert!(!doc.can_redo());
                        }
                        Ok(false) | Err(_) => prop_assert_eq!(state(&doc), before, "{:?}", cmd),
                    }
                }
                OpArb::Undo => match doc.undo() {
                    Ok(()) => prop_assert!(doc.can_redo()),
                    Err(e) => {
                        prop_assert_eq!(e, DocumentError::NothingToUndo);
                        prop_assert_eq!(state(&doc), before);
                    }
                },
                OpArb::Redo => match doc.redo() {
                    Ok(()) => prop_assert!(doc.can_undo()),
                    Err(e) => {
                        prop_assert_eq!(e, DocumentError::NothingToRedo);
                        prop_assert_eq!(state(&doc), before);
                    }
                },
                OpArb::Limit(limit) => {
                    doc.set_history_limit(*limit);
                    prop_assert_eq!(doc.history_limit(), *limit);
                    // Cambiar el límite no toca el contenido ni la pila de rehacer.
                    prop_assert_eq!(&doc, &before.doc);
                    prop_assert_eq!(doc.can_redo(), before.can_redo);
                    prop_assert_eq!(steps_redoable(&doc), before.redoable);
                }
            }
        }
    }

    // Propiedad 4: un Batch es un paso.
    #[test]
    fn batch_is_a_step(
        img in image_small(),
        list in prop::collection::vec(set_raw(false), 1..=6),
    ) {
        let f = shape(&img);
        let initial = Document::new(img);
        let id = ImageId::from_raw(0);
        let cmds: Vec<Command> = list.iter().map(|s| set_cmd(f, id, s)).collect();

        // Uno a uno.
        let mut one_by_one = initial.clone();
        let mut some = false;
        for c in &cmds {
            match one_by_one.apply(c.clone()) {
                Ok(change) => some |= change,
                Err(e) => {
                    return Err(TestCaseError::fail(format!(
                        "un SetPixels válido falló: {e:?} ({c:?})"
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

    // Propiedad 5: el límite de la historia, con el límite fijado antes de editar.
    #[test]
    fn limit_of_the_history(
        img in image_small(),
        limit in 0usize..=2000,
        cmds in prop::collection::vec(cmd_raw(false), 0..=30),
    ) {
        let f = shape(&img);
        let mut doc = Document::new(img.clone());
        doc.set_history_limit(limit);
        let mut instants = vec![img];
        applies_valid(&mut doc, &mut instants, f, &cmds, Some(limit))?;
        checks_limit(&doc, &instants, limit)?;
    }

    // Propiedad 5, segunda parte: bajar el límite con `set_history_limit` cumple lo mismo.
    #[test]
    fn lowering_the_history_limit(
        img in image_small(),
        cmds in prop::collection::vec(cmd_raw(false), 0..=30),
        limits in prop::collection::vec(0usize..=2000, 1..=3),
    ) {
        let f = shape(&img);
        let mut doc = Document::new(img.clone());
        let mut instants = vec![img];
        applies_valid(&mut doc, &mut instants, f, &cmds, None)?;

        // Con el límite por defecto no se descarta nada.
        checks_limit(&doc, &instants, Document::DEFAULT_HISTORY_LIMIT)?;
        prop_assert_eq!(steps_undoable(&doc), instants.len() - 1);

        for limit in limits {
            doc.set_history_limit(limit);
            checks_limit(&doc, &instants, limit)?;
        }
    }
}
