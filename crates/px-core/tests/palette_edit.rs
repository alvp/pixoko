//! Tests de los siete comandos de edición de la paleta de una imagen indexada
//! (specs/hito-2-paleta-editar.md): `SetPaletteColor`, `MovePaletteEntry`, `PermutePalette`,
//! `AddPaletteEntry`, `RemovePaletteEntry`, `SetTransparentIndex` y `RotateColors`, con
//! `DocumentError::PaletteEdit(PaletteEditError)`.
//!
//! Una prueba por cada fila 1 a 19 de la tabla de casos límite (`fila_NN_...`; las filas 14,
//! 15, 17 y 18 se reparten en varias) y una propiedad por cada una de las ocho de «Tests»
//! (`propiedad_N_...`). Las cifras de las filas son las de la spec, calculadas a mano; no se
//! recalculan con el código. Los oráculos de las propiedades 1, 3 y 8 están escritos aquí
//! (`visible`, `resolved`, `rotates_by_hand`...): no llaman a `Palette::resolved` ni a
//! `Image::set_pixel` para calcular lo esperado, solo leen `palette().colors()`,
//! `transparent_index()` y `get_pixel`. El coste de la historia solo se mide con
//! `history_bytes()` y con las cotas de la spec: nunca se asume el valor de `size_of`.

use std::error::Error;
use std::fmt::{Debug, Display};

use proptest::prelude::*;
use proptest::test_runner::TestCaseError;
use px_core::{
    Allowed, Command, Document, DocumentError, Image, ImageId, Mode, Palette, PaletteEditError,
    Pixel, PixelEdit, Rgba, Span,
};

// Colores de la spec.

const R: Rgba = Rgba::new(255, 0, 0, 255);
const G: Rgba = Rgba::new(0, 255, 0, 255);
const B: Rgba = Rgba::new(0, 0, 255, 255);
const K: Rgba = Rgba::new(0, 0, 0, 255);
const Y: Rgba = Rgba::new(255, 255, 0, 255);
const W: Rgba = Rgba::new(255, 255, 255, 255);

// Construcción de imágenes, documentos y comandos.

/// Imagen indexada con la paleta, el transparente y los píxeles dados (fila a fila).
fn indexed(width: u32, height: u32, colors_2: &[Rgba], t: Option<u8>, pixels: &[u8]) -> Image {
    assert_eq!(pixels.len(), (width * height) as usize);
    let mut palette_2 = Palette::from_colors(colors_2.iter().copied()).unwrap();
    palette_2.set_transparent_index(t).unwrap();
    let mut img = Image::new_indexed(width, height, palette_2).unwrap();
    for (i, &v) in pixels.iter().enumerate() {
        img.set_pixel(i as u32 % width, i as u32 / width, Pixel::Indexed(v))
            .unwrap();
    }
    img
}

/// `F` de la spec: Indexed 4 x 1, paleta `[R,G,B,K]`, píxeles `[0,1,2,3]`.
fn img_f(t: Option<u8>) -> Image {
    indexed(4, 1, &[R, G, B, K], t, &[0, 1, 2, 3])
}

fn doc_f(t: Option<u8>) -> Document {
    Document::new(img_f(t))
}

/// `n` colores opacos distintos (n <= 256).
fn colors_different(n: usize) -> Vec<Rgba> {
    (0..n)
        .map(|i| Rgba::new(i as u8, 255 - i as u8, 7, 255))
        .collect()
}

/// Documento con una imagen indexada de `w` x `h` (todos los píxeles en el índice 0) y una
/// paleta de `n` colores distintos.
fn doc_with_palette(w: u32, h: u32, n: usize, t: Option<u8>) -> Document {
    let pixels = vec![0u8; (w * h) as usize];
    Document::new(indexed(w, h, &colors_different(n), t, &pixels))
}

fn image_of(doc: &Document) -> &Image {
    doc.image(ImageId::from_raw(0)).unwrap()
}

fn palette_of(img: &Image) -> Vec<Rgba> {
    img.palette().unwrap().colors().to_vec()
}

fn transparent_of(img: &Image) -> Option<u8> {
    img.palette().and_then(Palette::transparent_index)
}

fn length_of_palette(img: &Image) -> usize {
    img.palette().map_or(0, Palette::len)
}

/// Los índices de los píxeles, fila a fila.
fn pixels_of(img: &Image) -> Vec<u8> {
    let mut v = Vec::new();
    for y in 0..img.height() {
        for x in 0..img.width() {
            match img.get_pixel(x, y) {
                Ok(Pixel::Indexed(i)) => v.push(i),
                other => panic!("se esperaba un píxel indexado y salió {other:?}"),
            }
        }
    }
    v
}

/// Color visible de una entrada: el guardado, con alfa 0 si es la transparente.
fn visible(color: Rgba, is_transparent: bool) -> Rgba {
    if is_transparent {
        Rgba::new(color.r, color.g, color.b, 0)
    } else {
        color
    }
}

/// Color resuelto de cada píxel, calculado aquí a partir de la paleta guardada.
fn resolved(img: &Image) -> Vec<Rgba> {
    let colors_2 = palette_of(img);
    let t = transparent_of(img);
    pixels_of(img)
        .into_iter()
        .map(|v| visible(colors_2[usize::from(v)], t == Some(v)))
        .collect()
}

fn set(image: ImageId, x: u32, y: u32, pixel: Pixel) -> Command {
    Command::SetPixels {
        image,
        pixels: vec![PixelEdit { x, y, pixel }],
    }
}

fn put_color(image: ImageId, index: u8, color: Rgba) -> Command {
    Command::SetPaletteColor {
        image,
        index,
        color,
    }
}

fn moves(image: ImageId, from: u8, to: u8) -> Command {
    Command::MovePaletteEntry { image, from, to }
}

fn permutes(image: ImageId, order: &[u8]) -> Command {
    Command::PermutePalette {
        image,
        order: order.to_vec(),
    }
}

fn adds(image: ImageId, color: Rgba) -> Command {
    Command::AddPaletteEntry { image, color }
}

fn removes(image: ImageId, index: u8, into: u8) -> Command {
    Command::RemovePaletteEntry { image, index, into }
}

fn transparent(image: ImageId, index: Option<u8>) -> Command {
    Command::SetTransparentIndex { image, index }
}

fn rotates(image: ImageId, start: u8, len: u16, steps: i32) -> Command {
    Command::RotateColors {
        image,
        start,
        len,
        steps,
    }
}

/// Los siete comandos, con argumentos válidos para una imagen indexada de 2 o más entradas.
fn seven(image: ImageId) -> Vec<Command> {
    vec![
        put_color(image, 0, R),
        moves(image, 0, 1),
        permutes(image, &[1, 0]),
        adds(image, W),
        removes(image, 0, 1),
        transparent(image, Some(0)),
        rotates(image, 0, 2, 1),
    ]
}

// Errores.

fn editing(e: PaletteEditError) -> DocumentError {
    DocumentError::PaletteEdit(e)
}

fn outside(index: u8, len: usize) -> DocumentError {
    editing(PaletteEditError::IndexOutOfRange { index, len })
}

fn range_outside(start: u8, len: u16, palette_len: usize) -> DocumentError {
    editing(PaletteEditError::RangeOutOfPalette {
        start,
        len,
        palette_len,
    })
}

fn in_batch(index: usize, source: DocumentError) -> DocumentError {
    DocumentError::InBatch {
        index,
        source: Box::new(source),
    }
}

// Estado observable del documento.

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
    state_id: u64,
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
        state_id: doc.state_id(),
        limit: doc.history_limit(),
        undoable: steps_undoable(doc),
        redoable: steps_redoable(doc),
    }
}

/// `apply(cmd)` da exactamente `expected_2` y no toca nada observable.
fn rejects(doc: &mut Document, cmd: Command, expected_2: DocumentError) {
    let before = state(doc);
    assert_eq!(doc.apply(cmd.clone()), Err(expected_2), "{cmd:?}");
    assert_eq!(state(doc), before, "{cmd:?}");
}

/// `apply(cmd)` da `Ok(false)` y no toca nada observable (ni historia ni `state_id`).
fn without_step(doc: &mut Document, cmd: Command) {
    let before = state(doc);
    assert_eq!(doc.apply(cmd.clone()), Ok(false), "{cmd:?}");
    assert_eq!(state(doc), before, "{cmd:?}");
}

/// `apply(cmd)` da `Ok(true)`: un paso nuevo, con `state_id` nuevo y la pila de rehacer vacía.
fn with_step(doc: &mut Document, cmd: Command) {
    let id_before = doc.state_id();
    let steps_2 = steps_undoable(doc);
    assert_eq!(doc.apply(cmd.clone()), Ok(true), "{cmd:?}");
    assert_ne!(doc.state_id(), id_before, "{cmd:?}");
    assert!(doc.can_undo(), "{cmd:?}");
    assert!(!doc.can_redo(), "{cmd:?}");
    assert_eq!(steps_undoable(doc), steps_2 + 1, "{cmd:?}");
}

/// Aplica `cmd` (que debe dar `Ok(true)`), lo deshace y lo rehace; comprueba que
/// `history_bytes()` tras `redo` es el de tras `apply` y que `undo` baja al de antes.
/// Devuelve cuánto subió `history_bytes()`.
fn cost_of_the_step(doc: &mut Document, cmd: Command) -> usize {
    let before = doc.history_bytes();
    assert_eq!(doc.apply(cmd.clone()), Ok(true), "{cmd:?}");
    let after_apply = doc.history_bytes();
    assert!(after_apply >= before, "{cmd:?}");
    assert_eq!(doc.undo(), Ok(()), "{cmd:?}");
    assert_eq!(doc.history_bytes(), before, "{cmd:?}");
    assert_eq!(doc.redo(), Ok(()), "{cmd:?}");
    assert_eq!(doc.history_bytes(), after_apply, "{cmd:?}");
    after_apply - before
}

// Tipos nuevos de la API.

fn assert_error_traits<T: Debug + Clone + PartialEq + Eq + Display + Error>() {}

#[test]
fn new_api_types() {
    assert_error_traits::<PaletteEditError>();
    let errors = [
        PaletteEditError::NotIndexed { mode: Mode::Rgba },
        PaletteEditError::IndexOutOfRange { index: 4, len: 4 },
        PaletteEditError::BadPermutation,
        PaletteEditError::PaletteFull,
        PaletteEditError::RemoveIntoSelf { index: 2 },
        PaletteEditError::LastEntry,
        PaletteEditError::RangeOutOfPalette {
            start: 2,
            len: 3,
            palette_len: 4,
        },
    ];
    for (i, e) in errors.iter().enumerate() {
        assert!(!e.to_string().is_empty(), "{e:?}");
        assert_eq!(e, &e.clone());
        for (j, other) in errors.iter().enumerate() {
            assert_eq!(i == j, e == other, "{e:?} frente a {other:?}");
        }
        let doc = editing(e.clone());
        assert!(!doc.to_string().is_empty());
        assert!(doc.source().is_some(), "PaletteEdit lleva su causa");
    }
    assert_ne!(outside(4, 4), outside(5, 4));
    assert_ne!(outside(4, 4), outside(4, 5));

    let id = ImageId::from_raw(0);
    for cmd in seven(id) {
        assert_eq!(cmd, cmd.clone());
        assert!(!format!("{cmd:?}").is_empty());
    }
    assert_ne!(put_color(id, 1, R), put_color(id, 2, R));
    assert_ne!(put_color(id, 1, R), put_color(id, 1, G));
    assert_ne!(moves(id, 0, 1), moves(id, 1, 0));
    assert_ne!(permutes(id, &[0, 1]), permutes(id, &[1, 0]));
    assert_ne!(removes(id, 0, 1), removes(id, 1, 0));
    assert_ne!(transparent(id, None), transparent(id, Some(0)));
    assert_ne!(rotates(id, 0, 4, 1), rotates(id, 0, 4, -1));
    assert_ne!(rotates(id, 0, 4, 1), rotates(id, 0, 3, 1));
    assert_ne!(rotates(id, 0, 4, 1), rotates(id, 1, 4, 1));
}

// Una prueba por fila de la tabla de casos límite.

#[test]
fn row_01_set_palette_color_and_undo() {
    let mut doc = doc_f(None);
    let id = ImageId::from_raw(0);
    let original = image_of(&doc).clone();

    with_step(&mut doc, put_color(id, 1, Y));
    assert_eq!(palette_of(image_of(&doc)), vec![R, Y, B, K]);
    assert_eq!(pixels_of(image_of(&doc)), vec![0, 1, 2, 3]);
    assert_eq!(resolved(image_of(&doc)), vec![R, Y, B, K]);
    assert_eq!(transparent_of(image_of(&doc)), None);
    let edited = doc.clone();

    // `undo` deja una imagen igual a la original; `redo`, otra vez `[R,Y,B,K]`.
    assert_eq!(doc.undo(), Ok(()));
    assert_eq!(image_of(&doc), &original);
    assert!(!doc.can_undo());
    assert!(doc.can_redo());
    assert_eq!(doc.redo(), Ok(()));
    assert_eq!(palette_of(image_of(&doc)), vec![R, Y, B, K]);
    assert_eq!(pixels_of(image_of(&doc)), vec![0, 1, 2, 3]);
    assert_eq!(doc, edited);
    assert!(doc.can_undo());
    assert!(!doc.can_redo());
}

#[test]
fn row_02_same_color_out_of_range_and_transparent_entry() {
    let mut doc = doc_f(None);
    let id = ImageId::from_raw(0);

    // Mismo color: `Ok(false)` sin paso.
    without_step(&mut doc, put_color(id, 1, G));
    // Índice fuera de la paleta.
    rejects(&mut doc, put_color(id, 4, R), outside(4, 4));

    // Sobre la entrada transparente: `Ok(true)`, el RGB guardado cambia aunque no se vea.
    let mut doc = doc_f(Some(2));
    let id = ImageId::from_raw(0);
    with_step(&mut doc, put_color(id, 2, Rgba::new(9, 9, 9, 255)));
    let img = image_of(&doc);
    assert_eq!(palette_of(img)[2], Rgba::new(9, 9, 9, 255));
    assert_eq!(resolved(img)[2], Rgba::new(9, 9, 9, 0));
    assert_eq!(transparent_of(img), Some(2));
    assert_eq!(pixels_of(img), vec![0, 1, 2, 3]);
}

#[test]
fn row_03_permute_with_transparent() {
    let mut doc = doc_f(Some(0));
    let id = ImageId::from_raw(0);
    let before = resolved(image_of(&doc));
    let expected_2_2 = vec![
        Rgba::new(255, 0, 0, 0),
        Rgba::new(0, 255, 0, 255),
        Rgba::new(0, 0, 255, 255),
        Rgba::new(0, 0, 0, 255),
    ];
    assert_eq!(before, expected_2_2);

    with_step(&mut doc, permutes(id, &[3, 2, 1, 0]));
    let img = image_of(&doc);
    assert_eq!(palette_of(img), vec![K, B, G, R]);
    assert_eq!(pixels_of(img), vec![3, 2, 1, 0]);
    assert_eq!(transparent_of(img), Some(3));
    assert_eq!(resolved(img), expected_2_2);
}

#[test]
fn row_04_permute_with_unused_entries() {
    let mut doc = Document::new(indexed(4, 1, &[R, G, B, K], None, &[0, 0, 2, 2]));
    let id = ImageId::from_raw(0);
    assert_eq!(resolved(image_of(&doc)), vec![R, R, B, B]);

    with_step(&mut doc, permutes(id, &[2, 0, 3, 1]));
    let img = image_of(&doc);
    // `adelante = {0->1, 1->3, 2->0, 3->2}`; G y K (sin usar) viajan a 3 y 2.
    assert_eq!(palette_of(img), vec![B, R, K, G]);
    assert_eq!(pixels_of(img), vec![1, 1, 0, 0]);
    assert_eq!(resolved(img), vec![R, R, B, B]);
}

#[test]
fn row_05_identity_permutation_and_malformed_ones() {
    let mut doc = doc_f(None);
    let id = ImageId::from_raw(0);
    let original = image_of(&doc).clone();

    without_step(&mut doc, permutes(id, &[0, 1, 2, 3]));
    let bad = editing(PaletteEditError::BadPermutation);
    rejects(&mut doc, permutes(id, &[0, 1, 2]), bad.clone());
    rejects(&mut doc, permutes(id, &[0, 1, 1, 3]), bad.clone());
    rejects(&mut doc, permutes(id, &[0, 1, 2, 4]), bad);
    assert_eq!(image_of(&doc), &original);
}

#[test]
fn row_06_move_with_transparent_is_equivalent_to_permute() {
    let mut doc = doc_f(Some(1));
    let id = ImageId::from_raw(0);
    let mut with_permute = doc.clone();

    with_step(&mut doc, moves(id, 0, 2));
    let img = image_of(&doc);
    assert_eq!(palette_of(img), vec![G, B, R, K]);
    assert_eq!(pixels_of(img), vec![2, 0, 1, 3]);
    assert_eq!(transparent_of(img), Some(0));

    // Mismo documento final que `PermutePalette([1,2,0,3])`.
    with_step(&mut with_permute, permutes(id, &[1, 2, 0, 3]));
    assert_eq!(image_of(&with_permute), image_of(&doc));
}

#[test]
fn row_07_move_without_change_and_out_of_range() {
    let mut doc = doc_f(None);
    let id = ImageId::from_raw(0);

    with_step(&mut doc, moves(id, 3, 1));
    let img = image_of(&doc);
    assert_eq!(palette_of(img), vec![R, K, G, B]);
    assert_eq!(pixels_of(img), vec![0, 2, 3, 1]);

    without_step(&mut doc, moves(id, 2, 2));
    rejects(&mut doc, moves(id, 0, 4), outside(4, 4));
    // Por orden de campo: `from` antes que `to`.
    rejects(&mut doc, moves(id, 5, 9), outside(5, 4));
}

#[test]
fn row_08_add_input() {
    let mut doc = doc_f(None);
    let id = ImageId::from_raw(0);
    let original = image_of(&doc).clone();

    with_step(&mut doc, adds(id, W));
    let img = image_of(&doc);
    assert_eq!(img.palette().unwrap().len(), 5);
    assert_eq!(palette_of(img), vec![R, G, B, K, W]);
    assert_eq!(pixels_of(img), vec![0, 1, 2, 3]);
    assert_eq!(transparent_of(img), None);
    let added = doc.clone();

    assert_eq!(doc.undo(), Ok(()));
    assert_eq!(image_of(&doc).palette().unwrap().len(), 4);
    assert_eq!(image_of(&doc), &original);
    assert_eq!(doc.redo(), Ok(()));
    assert_eq!(image_of(&doc).palette().unwrap().len(), 5);
    assert_eq!(doc, added);

    // Con 256 entradas: `PaletteFull`, todo intacto.
    let mut full = Document::new(indexed(2, 1, &colors_different(256), None, &[0, 255]));
    let id = ImageId::from_raw(0);
    rejects(
        &mut full,
        adds(id, W),
        editing(PaletteEditError::PaletteFull),
    );
    assert_eq!(image_of(&full).palette().unwrap().len(), 256);
}

#[test]
fn row_08_adding_a_transparent_does_not_make_it_the_transparent_index() {
    for t in [None, Some(2)] {
        let mut doc = doc_f(t);
        let id = ImageId::from_raw(0);
        with_step(&mut doc, adds(id, Rgba::TRANSPARENT));
        let img = image_of(&doc);
        assert_eq!(palette_of(img), vec![R, G, B, K, Rgba::TRANSPARENT]);
        assert_eq!(transparent_of(img), t);
        assert_eq!(pixels_of(img), vec![0, 1, 2, 3]);
    }
}

#[test]
fn row_09_remove_and_undo_with_the_mask() {
    let mut doc = doc_f(None);
    let id = ImageId::from_raw(0);
    let original = image_of(&doc).clone();
    assert_eq!(resolved(&original), vec![R, G, B, K]);

    with_step(&mut doc, removes(id, 1, 0));
    let img = image_of(&doc);
    assert_eq!(palette_of(img), vec![R, B, K]);
    assert_eq!(pixels_of(img), vec![0, 0, 1, 2]);
    assert_eq!(resolved(img), vec![R, R, B, K]);
    let removed = doc.clone();

    // `undo` restaura exactamente: el píxel 0 sigue en 0 y el 1 vuelve a 1.
    assert_eq!(doc.undo(), Ok(()));
    assert_eq!(image_of(&doc), &original);
    assert_eq!(pixels_of(image_of(&doc)), vec![0, 1, 2, 3]);
    assert_eq!(resolved(image_of(&doc)), vec![R, G, B, K]);
    assert_eq!(doc.redo(), Ok(()));
    assert_eq!(doc, removed);
    assert_eq!(pixels_of(image_of(&doc)), vec![0, 0, 1, 2]);
}

#[test]
fn row_10_remove_with_transparent_greater_than_the_removed() {
    let mut doc = doc_f(Some(2));
    let id = ImageId::from_raw(0);
    let original = image_of(&doc).clone();

    // `into' = 3 - 1 = 2`.
    with_step(&mut doc, removes(id, 0, 3));
    let img = image_of(&doc);
    assert_eq!(palette_of(img), vec![G, B, K]);
    assert_eq!(pixels_of(img), vec![2, 0, 1, 2]);
    assert_eq!(transparent_of(img), Some(1));
    assert_eq!(resolved(img), vec![K, G, Rgba::new(0, 0, 255, 0), K]);

    assert_eq!(doc.undo(), Ok(()));
    assert_eq!(image_of(&doc), &original);
}

#[test]
fn row_11_remove_the_transparent_and_remove_toward_the_transparent() {
    // Se quita la transparente: deja de haber transparente y sus píxeles toman el color de
    // `into`, ahora opaco.
    let mut doc = doc_f(Some(2));
    let id = ImageId::from_raw(0);
    with_step(&mut doc, removes(id, 2, 0));
    let img = image_of(&doc);
    assert_eq!(palette_of(img), vec![R, G, K]);
    assert_eq!(pixels_of(img), vec![0, 1, 0, 2]);
    assert_eq!(transparent_of(img), None);
    assert_eq!(resolved(img)[2], R);

    // `into` es la transparente: los píxeles quedan transparentes.
    let mut doc = doc_f(Some(2));
    let id = ImageId::from_raw(0);
    with_step(&mut doc, removes(id, 0, 2));
    let img = image_of(&doc);
    // `into' = 2 - 1 = 1`.
    assert_eq!(palette_of(img), vec![G, B, K]);
    assert_eq!(pixels_of(img), vec![1, 0, 1, 2]);
    assert_eq!(transparent_of(img), Some(1));
    let transparent_b = Rgba::new(0, 0, 255, 0);
    assert_eq!(resolved(img)[0], transparent_b);
    assert_eq!(resolved(img)[2], transparent_b);
}

#[test]
fn row_12_remove_errors_and_their_order() {
    let mut doc = doc_f(None);
    let id = ImageId::from_raw(0);
    rejects(&mut doc, removes(id, 4, 0), outside(4, 4));
    rejects(&mut doc, removes(id, 0, 4), outside(4, 4));
    // `index` antes que `into`.
    rejects(&mut doc, removes(id, 5, 7), outside(5, 4));
    rejects(
        &mut doc,
        removes(id, 2, 2),
        editing(PaletteEditError::RemoveIntoSelf { index: 2 }),
    );

    // Imagen 2 x 1 de una sola entrada.
    let mut a = Document::new(indexed(2, 1, &[R], None, &[0, 0]));
    let id = ImageId::from_raw(0);
    // `LastEntry` gana a `RemoveIntoSelf`.
    rejects(
        &mut a,
        removes(id, 0, 0),
        editing(PaletteEditError::LastEntry),
    );
    // `IndexOutOfRange` de `into` gana a `LastEntry`.
    rejects(&mut a, removes(id, 0, 1), outside(1, 1));
}

#[test]
fn row_13_transparent_set_and_remove() {
    let mut doc = doc_f(None);
    let id = ImageId::from_raw(0);

    with_step(&mut doc, transparent(id, Some(2)));
    let img = image_of(&doc);
    assert_eq!(pixels_of(img), vec![0, 1, 2, 3]);
    assert_eq!(transparent_of(img), Some(2));
    assert_eq!(resolved(img)[2], Rgba::new(0, 0, 255, 0));

    without_step(&mut doc, transparent(id, Some(2)));
    with_step(&mut doc, transparent(id, Some(1)));
    assert_eq!(transparent_of(image_of(&doc)), Some(1));
    with_step(&mut doc, transparent(id, None));
    assert_eq!(transparent_of(image_of(&doc)), None);
    assert_eq!(resolved(image_of(&doc)), vec![R, G, B, K]);
    without_step(&mut doc, transparent(id, None));
    rejects(&mut doc, transparent(id, Some(4)), outside(4, 4));
    assert_eq!(pixels_of(image_of(&doc)), vec![0, 1, 2, 3]);
}

#[test]
fn row_14_non_indexed_image() {
    for mode in [Mode::Rgba, Mode::Gray] {
        let mut doc = Document::new(Image::new(2, 1, mode).unwrap());
        let id = ImageId::from_raw(0);
        let bad = editing(PaletteEditError::NotIndexed { mode });
        // Los siete comandos, con argumentos válidos y con argumentos inválidos: el modo
        // se comprueba antes que los argumentos.
        let mut commands = seven(id);
        commands.extend([
            put_color(id, 200, R),
            moves(id, 9, 9),
            permutes(id, &[0, 0, 0]),
            removes(id, 0, 0),
            transparent(id, Some(200)),
            rotates(id, 200, 200, 1),
        ]);
        for cmd in commands {
            rejects(&mut doc, cmd, bad.clone());
        }
    }
}

#[test]
fn row_14_image_unknown() {
    let mut doc = doc_f(None);
    let unknown = ImageId::from_raw(99);
    let bad = DocumentError::UnknownImage(unknown);
    let mut commands = seven(unknown);
    commands.extend([
        put_color(unknown, 200, R),
        permutes(unknown, &[0, 0, 0]),
        removes(unknown, 2, 2),
        rotates(unknown, 200, 200, 1),
    ]);
    for cmd in commands {
        rejects(&mut doc, cmd, bad.clone());
    }
}

#[test]
fn row_14_inside_a_batch_they_are_not_admitted() {
    let mut doc = doc_f(None);
    let id = ImageId::from_raw(0);
    let unknown = ImageId::from_raw(99);
    let original = image_of(&doc).clone();

    // El `SetPixels` anterior se revierte.
    let batch = Command::Batch(vec![set(id, 0, 0, Pixel::Indexed(3)), put_color(id, 1, Y)]);
    rejects(&mut doc, batch, in_batch(1, DocumentError::NotBatchable));
    assert_eq!(image_of(&doc), &original);
    assert!(!doc.can_undo());
    assert_eq!(doc.history_bytes(), 0);

    // `NotBatchable` gana a `UnknownImage`.
    let batch = Command::Batch(vec![permutes(unknown, &[0, 1, 2, 3])]);
    rejects(&mut doc, batch, in_batch(0, DocumentError::NotBatchable));
    // ... y a `RemoveIntoSelf`.
    let batch = Command::Batch(vec![removes(id, 2, 2)]);
    rejects(&mut doc, batch, in_batch(0, DocumentError::NotBatchable));
    // Un lote con un `RotateColors` válido también se rechaza.
    let batch = Command::Batch(vec![rotates(id, 0, 4, 1)]);
    rejects(&mut doc, batch, in_batch(0, DocumentError::NotBatchable));

    // Los siete, solos y detrás de un `SetPixels` válido.
    for cmd in seven(id) {
        let batch = Command::Batch(vec![cmd.clone()]);
        rejects(&mut doc, batch, in_batch(0, DocumentError::NotBatchable));
        let batch = Command::Batch(vec![set(id, 0, 0, Pixel::Indexed(3)), cmd]);
        rejects(&mut doc, batch, in_batch(1, DocumentError::NotBatchable));
    }

    // Con historia previa y pila de rehacer, tampoco cambia nada.
    assert_eq!(doc.apply(set(id, 1, 0, Pixel::Indexed(0))), Ok(true));
    assert_eq!(doc.undo(), Ok(()));
    assert!(doc.can_redo());
    let batch = Command::Batch(vec![moves(id, 0, 1)]);
    rejects(&mut doc, batch, in_batch(0, DocumentError::NotBatchable));
    assert!(doc.can_redo());
}

#[test]
fn row_15_permute_costs_the_same_at_any_size() {
    let order: Vec<u8> = (1..16).chain(std::iter::once(0)).collect();
    let mut small_2 = doc_with_palette(8, 8, 16, None);
    let mut large = doc_with_palette(512, 512, 16, None);
    let id = ImageId::from_raw(0);

    let cost_small = cost_of_the_step(&mut small_2, permutes(id, &order));
    let cost_large = cost_of_the_step(&mut large, permutes(id, &order));
    assert_eq!(cost_small, cost_large);
    assert!(
        (16..=512).contains(&cost_small),
        "history_bytes() subió {cost_small}"
    );
}

#[test]
fn row_15_remove_unused_and_remove_used() {
    // Entrada sin usar en 512 x 512: no hay máscara.
    let mut doc = doc_with_palette(512, 512, 16, None);
    let id = ImageId::from_raw(0);
    let cost = cost_of_the_step(&mut doc, removes(id, 5, 0));
    assert!(cost <= 512, "history_bytes() subió {cost}");

    // Entrada usada en 64 x 64: máscara de `ceil(4096/64) * 8 = 512` bytes.
    let mut img = image_of(&doc_with_palette(64, 64, 16, None)).clone();
    img.set_pixel(0, 0, Pixel::Indexed(1)).unwrap();
    let mut doc = Document::new(img);
    let id = ImageId::from_raw(0);
    let cost = cost_of_the_step(&mut doc, removes(id, 1, 0));
    assert!((512..=1024).contains(&cost), "history_bytes() subió {cost}");
}

#[test]
fn row_15_rotate_colors_costs_the_same_at_any_size() {
    let mut small_2 = doc_with_palette(8, 8, 16, None);
    let mut large = doc_with_palette(512, 512, 16, None);
    let id = ImageId::from_raw(0);

    let cost_small = cost_of_the_step(&mut small_2, rotates(id, 0, 16, 1));
    let cost_large = cost_of_the_step(&mut large, rotates(id, 0, 16, 1));
    assert_eq!(cost_small, cost_large);
    assert!(cost_small <= 512, "history_bytes() subió {cost_small}");
}

#[test]
fn row_15_the_mask_counts_toward_the_history_limit() {
    let mut doc = doc_with_palette(64, 64, 16, None);
    let id = ImageId::from_raw(0);
    doc.set_history_limit(300);

    // El `Permute` cuesta menos de 300 y cabe. Con `order[new_2] = old_2`, el índice
    // antiguo 0 (el de todos los píxeles) pasa a ser el 15.
    let order: Vec<u8> = (1..16).chain(std::iter::once(0)).collect();
    assert_eq!(doc.apply(permutes(id, &order)), Ok(true));
    let after_permute_bytes = doc.history_bytes();
    assert!(
        (16..300).contains(&after_permute_bytes),
        "history_bytes() = {after_permute_bytes}"
    );
    assert_eq!(steps_undoable(&doc), 1);
    assert!(pixels_of(image_of(&doc)).iter().all(|&v| v == 15));
    let after_permute = image_of(&doc).clone();

    // El `Remove` de una entrada usada guarda una máscara de 512 bytes: supera el límite,
    // se descarta el `Permute` y queda solo el `Remove`.
    assert_eq!(doc.apply(removes(id, 15, 0)), Ok(true));
    assert!(
        (512..=1024).contains(&doc.history_bytes()),
        "history_bytes() = {}",
        doc.history_bytes()
    );
    assert_eq!(steps_undoable(&doc), 1, "queda solo el Remove");
    assert_eq!(doc.undo(), Ok(()));
    assert_eq!(image_of(&doc), &after_permute);
    assert_eq!(doc.undo(), Err(DocumentError::NothingToUndo));
    assert_eq!(doc.redo(), Ok(()));
    assert_eq!(image_of(&doc).palette().unwrap().len(), 15);
}

#[test]
fn row_16_rotate_colors() {
    // (comando, paleta esperada)
    let cases: [((u8, u16, i32), [Rgba; 4]); 5] = [
        ((0, 4, 1), [K, R, G, B]),
        ((1, 3, 1), [R, K, G, B]),
        ((0, 4, -1), [G, B, K, R]),
        ((1, 3, -1), [R, B, K, G]),
        ((0, 4, 5), [K, R, G, B]),
    ];
    for ((start, len, steps), expected) in cases {
        let mut doc = doc_f(None);
        let id = ImageId::from_raw(0);
        let original = image_of(&doc).clone();

        with_step(&mut doc, rotates(id, start, len, steps));
        let img = image_of(&doc);
        assert_eq!(palette_of(img), expected.to_vec(), "{start} {len} {steps}");
        // Los píxeles conservan su índice y resuelven a la paleta nueva en ese orden.
        assert_eq!(pixels_of(img), vec![0, 1, 2, 3], "{start} {len} {steps}");
        assert_eq!(resolved(img), expected.to_vec(), "{start} {len} {steps}");
        let rotated = doc.clone();

        assert_eq!(doc.undo(), Ok(()));
        assert_eq!(image_of(&doc), &original, "{start} {len} {steps}");
        assert_eq!(palette_of(image_of(&doc)), vec![R, G, B, K]);
        assert_eq!(doc.redo(), Ok(()));
        assert_eq!(doc, rotated, "{start} {len} {steps}");
        assert_eq!(palette_of(image_of(&doc)), expected.to_vec());
    }
}

#[test]
fn row_17_rotations_that_do_nothing() {
    let mut doc = doc_f(None);
    let id = ImageId::from_raw(0);
    // Múltiplos de `len` (también negativos), `len < 2` y rangos vacíos válidos (`start = 4`
    // es la longitud de la paleta).
    without_step(&mut doc, rotates(id, 0, 4, 4));
    without_step(&mut doc, rotates(id, 0, 4, -8));
    without_step(&mut doc, rotates(id, 0, 4, 0));
    without_step(&mut doc, rotates(id, 1, 1, 3));
    without_step(&mut doc, rotates(id, 2, 0, 1));
    without_step(&mut doc, rotates(id, 4, 0, 1));
    assert_eq!(image_of(&doc), &img_f(None));
}

#[test]
fn row_17_steps_i32_min_without_panic() {
    let mut doc = doc_f(None);
    let id = ImageId::from_raw(0);
    // `i32::MIN = -2^31`, `2^31 mod 3 = 2`, luego `k = 1` y `[R,G,B]` pasa a `[B,R,G]`.
    with_step(&mut doc, rotates(id, 0, 3, i32::MIN));
    assert_eq!(palette_of(image_of(&doc)), vec![B, R, G, K]);
    assert_eq!(pixels_of(image_of(&doc)), vec![0, 1, 2, 3]);
}

#[test]
fn row_17_palette_with_equal_colors_and_palette_of_256() {
    // La rotación deja la paleta igual: `Ok(false)`.
    let mut doc = Document::new(indexed(4, 1, &[R, R, R, R], None, &[0, 1, 2, 3]));
    let id = ImageId::from_raw(0);
    without_step(&mut doc, rotates(id, 0, 4, 1));

    // 256 entradas distintas: `len = 256` cabe en `u16`.
    let colors_2 = colors_different(256);
    let mut doc = Document::new(indexed(2, 1, &colors_2, None, &[0, 255]));
    let id = ImageId::from_raw(0);
    // `-2^31` es múltiplo de 256.
    without_step(&mut doc, rotates(id, 0, 256, i32::MIN));
    with_step(&mut doc, rotates(id, 0, 256, 1));
    let new = palette_of(image_of(&doc));
    assert_eq!(new[0], colors_2[255]);
    for i in 1..256 {
        assert_eq!(new[i], colors_2[i - 1], "entrada {i}");
    }
    assert_eq!(pixels_of(image_of(&doc)), vec![0, 255]);
}

#[test]
fn row_18_range_outside_the_palette() {
    let mut doc = doc_f(None);
    let id = ImageId::from_raw(0);
    // 2 + 3 = 5 > 4.
    rejects(&mut doc, rotates(id, 2, 3, 1), range_outside(2, 3, 4));
    rejects(&mut doc, rotates(id, 5, 1, 1), range_outside(5, 1, 4));
    rejects(&mut doc, rotates(id, 0, 5, 1), range_outside(0, 5, 4));
    // 4 + 1 > 4.
    rejects(&mut doc, rotates(id, 4, 1, 1), range_outside(4, 1, 4));
}

#[test]
fn row_18_the_range_error_beats_ok_false() {
    // Un rango fuera de la paleta sigue siendo error aunque la rotación no hiciera nada.
    let mut doc = doc_f(None);
    let id = ImageId::from_raw(0);
    rejects(&mut doc, rotates(id, 0, 5, 0), range_outside(0, 5, 4));
    rejects(&mut doc, rotates(id, 5, 0, 1), range_outside(5, 0, 4));
    rejects(&mut doc, rotates(id, 3, 2, 2), range_outside(3, 2, 4));
}

#[test]
fn row_19_the_transparent_stays_in_its_slot() {
    let mut doc = doc_f(Some(3));
    let id = ImageId::from_raw(0);
    let original = image_of(&doc).clone();
    assert_eq!(resolved(&original), vec![R, G, B, Rgba::new(0, 0, 0, 0)]);

    with_step(&mut doc, rotates(id, 0, 4, 1));
    let img = image_of(&doc);
    assert_eq!(palette_of(img), vec![K, R, G, B]);
    assert_eq!(transparent_of(img), Some(3));
    assert_eq!(pixels_of(img), vec![0, 1, 2, 3]);
    assert_eq!(
        resolved(img),
        vec![
            Rgba::new(0, 0, 0, 255),
            Rgba::new(255, 0, 0, 255),
            Rgba::new(0, 255, 0, 255),
            Rgba::new(0, 0, 255, 0),
        ]
    );

    assert_eq!(doc.undo(), Ok(()));
    assert_eq!(image_of(&doc), &original);
}

// Estrategias y datos aleatorios.

/// Conjunto pequeño de colores para provocar duplicados y alfa 0.
const SET: [Rgba; 8] = [
    R,
    G,
    B,
    K,
    W,
    Rgba::TRANSPARENT,
    Rgba::new(255, 0, 0, 0),
    Rgba::new(0, 255, 0, 128),
];

fn color_p() -> impl Strategy<Value = Rgba> {
    prop::sample::select(SET.to_vec())
}

/// Longitud de paleta: de 1 a 16 y, en uno de cada tres casos, 255 o 256.
fn length() -> impl Strategy<Value = usize> {
    prop_oneof![
        2 => 1usize..=16,
        1 => prop::sample::select(vec![255usize, 256]),
    ]
}

/// El último índice de una paleta de `n` entradas.
fn last(n: usize) -> u8 {
    u8::try_from(n.saturating_sub(1)).unwrap_or(u8::MAX)
}

/// Índice de píxel válido para una paleta de `n` entradas, con sesgo hacia el último.
fn index_of_pixel(n: usize) -> impl Strategy<Value = u8> {
    prop_oneof![
        3 => (0..n).prop_map(|v| v as u8),
        1 => Just(last(n)),
    ]
}

/// Imagen indexada de hasta `max` x `max`, con paleta de colores del conjunto pequeño y,
/// a veces, índice transparente.
fn image_idx(max: u32) -> impl Strategy<Value = Image> {
    (1..=max, 1..=max, length())
        .prop_flat_map(|(w, h, n)| {
            (
                Just((w, h)),
                prop::collection::vec(color_p(), n),
                prop::option::of(0..n),
                prop::collection::vec(index_of_pixel(n), (w * h) as usize),
            )
        })
        .prop_map(|((w, h), colors_2, t, pixels)| {
            indexed(w, h, &colors_2, t.map(|t| t as u8), &pixels)
        })
}

/// Una imagen de 6 x 6 como mucho: casi siempre indexada y, a veces, de otro modo.
fn image_mixed() -> impl Strategy<Value = Image> {
    prop_oneof![
        9 => image_idx(6),
        1 => (
            prop::sample::select(vec![Mode::Rgba, Mode::Gray, Mode::GrayAlpha]),
            1u32..=6,
            1u32..=6,
        )
            .prop_map(|(mode, w, h)| Image::new(w, h, mode).unwrap()),
    ]
}

/// Índice `< n` con sesgo hacia el último.
fn adjusts(v: u16, n: usize) -> usize {
    if v.is_multiple_of(4) {
        n.saturating_sub(1)
    } else {
        usize::from(v / 4) % n.max(1)
    }
}

/// `u8` para un índice de una paleta de `n` entradas: casi siempre válido, y a veces el
/// límite (`n`, que está fuera) o 255.
fn index_u_8(v: u16, n: usize) -> u8 {
    match v % 8 {
        0 => last(n),
        1 => u8::try_from(n).unwrap_or(u8::MAX),
        2 => u8::MAX,
        _ => u8::try_from(usize::from(v / 8) % n.max(1)).unwrap_or(u8::MAX),
    }
}

/// Permutación de `0..n` a partir de una semilla (Fisher-Yates con un generador lineal).
fn permutation_of(n: usize, seed: u64) -> Vec<u8> {
    let mut v: Vec<u8> = (0..n).map(|i| i as u8).collect();
    let mut s = seed;
    for i in (1..n).rev() {
        s = s
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let j = ((s >> 33) as usize) % (i + 1);
        v.swap(i, j);
    }
    v
}

/// `order` de `PermutePalette`: según `kind`, una permutación, la identidad o una lista
/// mal formada (corta, larga, con un repetido o con un valor fuera de la paleta).
fn order_of(n: usize, seed: u64, kind: u8) -> Vec<u8> {
    let mut o = permutation_of(n, seed);
    match kind % 8 {
        1 => {
            o.pop();
        }
        2 => o.push(0),
        3 => {
            if let Some(&u) = o.last() {
                o[0] = u;
            }
        }
        4 => {
            if let Some(first) = o.first_mut() {
                *first = u8::try_from(n).unwrap_or(u8::MAX);
            }
        }
        5 => o = (0..n).map(|i| i as u8).collect(),
        _ => {}
    }
    o
}

/// `order` de un `Move`: `[0..n)` con `Vec::remove` e `insert`.
fn order_of_move(n: usize, from: usize, to: usize) -> Vec<u8> {
    let mut v: Vec<u8> = (0..n).map(|i| i as u8).collect();
    let x = v.remove(from);
    v.insert(to, x);
    v
}

fn steps_2() -> impl Strategy<Value = i32> {
    prop_oneof![
        3 => any::<i32>(),
        3 => -20i32..=20,
        1 => Just(i32::MIN),
        1 => Just(i32::MAX),
    ]
}

/// Números en bruto para fabricar un comando de paleta contra una imagen concreta.
#[derive(Clone, Debug)]
struct Raw {
    a: u16,
    b: u16,
    seed: u64,
    steps: i32,
    color: Rgba,
    kind: u8,
}

fn raw() -> impl Strategy<Value = Raw> {
    (
        any::<u16>(),
        any::<u16>(),
        any::<u64>(),
        steps_2(),
        color_p(),
        0u8..8,
    )
        .prop_map(|(a, b, seed, steps, color, kind)| Raw {
            a,
            b,
            seed,
            steps,
            color,
            kind,
        })
}

/// Un comando de paleta (el `kind` elige uno de los siete) para una paleta de `n` entradas;
/// valido casi siempre y, a veces, con un índice, una permutación o un rango inválidos.
fn command_palette(kind: u8, c: &Raw, id: ImageId, n: usize) -> Command {
    let n = n.max(1);
    match kind % 7 {
        0 => put_color(id, index_u_8(c.a, n), c.color),
        1 => moves(id, index_u_8(c.a, n), index_u_8(c.b, n)),
        2 => permutes(id, &order_of(n, c.seed, c.kind)),
        3 => adds(id, c.color),
        4 => removes(id, index_u_8(c.a, n), index_u_8(c.b, n)),
        5 => transparent(
            id,
            if c.kind.is_multiple_of(4) {
                None
            } else {
                Some(index_u_8(c.a, n))
            },
        ),
        _ => {
            let start = index_u_8(c.a, n);
            let rest = n.saturating_sub(usize::from(start));
            let len = match c.b % 6 {
                0 => rest,
                1 => rest + 1,
                2 => 0,
                3 => 1,
                _ => usize::from(c.b / 6) % (rest + 1),
            };
            rotates(id, start, u16::try_from(len).unwrap_or(u16::MAX), c.steps)
        }
    }
}

/// Rota a mano el trozo `start..start + len` de la paleta `k` posiciones hacia los índices
/// mayores: `new[start + i] = old[start + (i + len - k) mod len]`.
fn rotates_by_hand(old: &[Rgba], start: usize, len: usize, k: usize) -> Vec<Rgba> {
    let mut new = old.to_vec();
    for i in 0..len {
        new[start + i] = old[start + (i + len - k) % len];
    }
    new
}

/// `steps` módulo `len` (siempre en `0..len`; 0 si `len` es 0).
fn modulo(steps: i32, len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    let k = i64::from(steps).rem_euclid(len as i64);
    usize::try_from(k).unwrap_or(0)
}

/// Un píxel de la imagen actual, válido para su modo o (si `bad_2`) de otro modo.
fn pixel_for(img: &Image, v: u8, bad_2: bool) -> Pixel {
    let n = length_of_palette(img).max(1);
    let alpha_2 = if v.is_multiple_of(2) { 0 } else { 255 };
    if bad_2 {
        return match img.mode() {
            Mode::Gray => Pixel::Rgba(Rgba::TRANSPARENT),
            _ => Pixel::Gray(v),
        };
    }
    match img.mode() {
        Mode::Rgba => Pixel::Rgba(Rgba::new(v, v ^ 0x55, v.wrapping_mul(3), alpha_2)),
        Mode::GrayAlpha => Pixel::GrayAlpha {
            gray: v,
            alpha: alpha_2,
        },
        Mode::Gray => Pixel::Gray(v),
        Mode::Indexed => Pixel::Indexed((usize::from(v) % n) as u8),
    }
}

/// Documento con una imagen y algo de historia: un `SetPixels` válido y, según `undo`,
/// un `undo` que deja un paso por rehacer.
fn doc_with_history(img: Image, v: u8, undo: bool) -> Document {
    let mut doc = Document::new(img);
    let id = ImageId::from_raw(0);
    let pixel = pixel_for(image_of(&doc), v, false);
    let _ = doc.apply(set(id, 0, 0, pixel));
    if undo {
        let _ = doc.undo();
    }
    doc
}

/// La invariante central: ningún píxel `>= len` de la paleta y el transparente es `None` o
/// `< len`, mirado solo por la API pública (en cualquier imagen indexada).
fn checks_invariant(doc: &Document, id: ImageId) -> Result<(), TestCaseError> {
    let Some(img) = doc.image(id) else {
        return Ok(());
    };
    let Some(palette_2) = img.palette() else {
        return Ok(());
    };
    let n = palette_2.len();
    prop_assert!(
        palette_2
            .transparent_index()
            .is_none_or(|t| usize::from(t) < n),
        "transparente {:?} con {} entradas",
        palette_2.transparent_index(),
        n
    );
    for y in 0..img.height() {
        for x in 0..img.width() {
            match img.get_pixel(x, y) {
                Ok(Pixel::Indexed(v)) => {
                    prop_assert!(usize::from(v) < n, "píxel {} con {} entradas", v, n)
                }
                other => prop_assert!(false, "píxel inesperado {:?}", other),
            }
        }
    }
    Ok(())
}

// Predicado independiente de la validación (propiedad 5).

enum Expected {
    Err(DocumentError),
    WithoutStep,
    Step2,
}

/// Lo que debe pasar al aplicar `cmd` (un comando de paleta suelto) según los puntos 4 y 5
/// de «Comportamiento». `img` es `None` si el identificador no existe.
fn predicts(img: Option<&Image>, id: ImageId, cmd: &Command) -> Expected {
    let Some(img) = img else {
        return Expected::Err(DocumentError::UnknownImage(id));
    };
    if img.mode() != Mode::Indexed {
        return Expected::Err(editing(PaletteEditError::NotIndexed { mode: img.mode() }));
    }
    let colors_2 = palette_of(img);
    let n = colors_2.len();
    let t = transparent_of(img);
    let step = |changes: bool| {
        if changes {
            Expected::Step2
        } else {
            Expected::WithoutStep
        }
    };
    match cmd {
        Command::SetPaletteColor { index, color, .. } => {
            if usize::from(*index) >= n {
                return Expected::Err(outside(*index, n));
            }
            step(colors_2[usize::from(*index)] != *color)
        }
        Command::MovePaletteEntry { from, to, .. } => {
            if usize::from(*from) >= n {
                return Expected::Err(outside(*from, n));
            }
            if usize::from(*to) >= n {
                return Expected::Err(outside(*to, n));
            }
            step(from != to)
        }
        Command::PermutePalette { order, .. } => {
            let mut seen = vec![false; n];
            let mut valid = order.len() == n;
            for &o in order {
                match seen.get_mut(usize::from(o)) {
                    Some(v) if !*v => *v = true,
                    _ => valid = false,
                }
            }
            if !valid {
                return Expected::Err(editing(PaletteEditError::BadPermutation));
            }
            let identity = order.iter().enumerate().all(|(i, &o)| usize::from(o) == i);
            step(!identity)
        }
        Command::AddPaletteEntry { .. } => {
            if n >= 256 {
                return Expected::Err(editing(PaletteEditError::PaletteFull));
            }
            Expected::Step2
        }
        Command::RemovePaletteEntry { index, into, .. } => {
            if usize::from(*index) >= n {
                return Expected::Err(outside(*index, n));
            }
            if usize::from(*into) >= n {
                return Expected::Err(outside(*into, n));
            }
            if n == 1 {
                return Expected::Err(editing(PaletteEditError::LastEntry));
            }
            if index == into {
                return Expected::Err(editing(PaletteEditError::RemoveIntoSelf { index: *index }));
            }
            Expected::Step2
        }
        Command::SetTransparentIndex { index, .. } => {
            if let Some(i) = index
                && usize::from(*i) >= n
            {
                return Expected::Err(outside(*i, n));
            }
            step(*index != t)
        }
        Command::RotateColors {
            start, len, steps, ..
        } => {
            let (s, l) = (usize::from(*start), usize::from(*len));
            if s + l > n {
                return Expected::Err(range_outside(*start, *len, n));
            }
            if l < 2 {
                return Expected::WithoutStep;
            }
            let k = modulo(*steps, l);
            if k == 0 {
                return Expected::WithoutStep;
            }
            step(rotates_by_hand(&colors_2, s, l, k) != colors_2)
        }
        other => panic!("comando que no es de paleta: {other:?}"),
    }
}

// Propiedad 1: oráculo independiente de reordenar.

/// La imagen que debe salir de reordenar con `order` (`order[new_2] = old_2`): paleta con
/// `order.map`, y píxeles y transparente con una búsqueda lineal del nuevo índice.
fn oracle_reorder(img: &Image, order: &[u8]) -> Image {
    let old = palette_of(img);
    let new: Vec<Rgba> = order.iter().map(|&o| old[usize::from(o)]).collect();
    let new_index = |v: u8| -> u8 {
        let pos = order.iter().position(|&o| o == v).unwrap();
        u8::try_from(pos).unwrap()
    };
    let pixels: Vec<u8> = pixels_of(img).into_iter().map(new_index).collect();
    let t = transparent_of(img).map(new_index);
    indexed(img.width(), img.height(), &new, t, &pixels)
}

/// Aplica `cmd` (equivalente a `order`) y `round_trip` (que lo deshace) y compara con el
/// oráculo.
fn checks_reorder(
    img: &Image,
    cmd: Command,
    order: &[u8],
    round_trip: Command,
) -> Result<(), TestCaseError> {
    let identity = order.iter().enumerate().all(|(i, &o)| usize::from(o) == i);
    let expected = oracle_reorder(img, order);
    let before = resolved(img);

    let mut doc = Document::new(img.clone());
    let real = doc.apply(cmd);
    prop_assert_eq!(real, Ok(!identity));
    // (a) La imagen coincide exactamente: píxeles, paleta y transparente.
    prop_assert_eq!(image_of(&doc), &expected);
    prop_assert_eq!(pixels_of(image_of(&doc)), pixels_of(&expected));
    prop_assert_eq!(palette_of(image_of(&doc)), palette_of(&expected));
    prop_assert_eq!(transparent_of(image_of(&doc)), transparent_of(&expected));
    // (b) Ningún color visible cambia.
    prop_assert_eq!(resolved(image_of(&doc)), before);
    checks_invariant(&doc, ImageId::from_raw(0))?;

    // Ida y vuelta.
    prop_assert_eq!(doc.apply(round_trip), Ok(!identity));
    prop_assert_eq!(image_of(&doc), img);
    Ok(())
}

// Propiedad 2: `Move` equivale a `Permute`.

fn prop_move_is_permute(img: Image, a: u16, b: u16) -> Result<(), TestCaseError> {
    let n = length_of_palette(&img);
    let (from, to) = (adjusts(a, n), adjusts(b, n));
    let order = order_of_move(n, from, to);
    let id = ImageId::from_raw(0);

    let mut with_move = Document::new(img.clone());
    let mut with_permute = Document::new(img.clone());
    let r_move = with_move.apply(moves(id, from as u8, to as u8));
    let r_permute = with_permute.apply(permutes(id, &order));
    prop_assert_eq!(&r_move, &Ok(from != to));
    prop_assert_eq!(&r_move, &r_permute);
    prop_assert_eq!(&with_move, &with_permute);
    prop_assert_eq!(with_move.history_bytes(), with_permute.history_bytes());
    prop_assert_eq!(with_move.can_undo(), with_permute.can_undo());

    if from != to {
        prop_assert_eq!(with_move.undo(), Ok(()));
        prop_assert_eq!(with_permute.undo(), Ok(()));
    }
    prop_assert_eq!(image_of(&with_move), &img);
    prop_assert_eq!(image_of(&with_permute), &img);
    Ok(())
}

// Propiedad 3: oráculo de quitar.

fn prop_remove(img: Image, a: u16, b: u16) -> Result<(), TestCaseError> {
    let old = palette_of(&img);
    let n = old.len();
    if n < 2 {
        return Ok(());
    }
    let index = adjusts(a, n);
    let mut into = adjusts(b, n);
    if into == index {
        into = (index + 1) % n;
    }
    let t = transparent_of(&img);
    let pix_before = pixels_of(&img);
    let res_before = resolved(&img);
    let res_into = visible(old[into], t == Some(into as u8));
    let into_new = if into < index { into } else { into - 1 };

    let mut doc = Document::new(img.clone());
    let id = ImageId::from_raw(0);
    prop_assert_eq!(doc.apply(removes(id, index as u8, into as u8)), Ok(true));
    let after = image_of(&doc);
    let pix_after = pixels_of(after);
    let res_after = resolved(after);

    // La paleta es la antigua sin `index`.
    let mut palette_expected = old.clone();
    palette_expected.remove(index);
    prop_assert_eq!(palette_of(after), palette_expected);

    // El transparente: `None`, `< index`, `== index` y `> index`.
    let t_expected = match t {
        None => None,
        Some(x) if usize::from(x) == index => None,
        Some(x) if usize::from(x) > index => Some(x - 1),
        Some(x) => Some(x),
    };
    prop_assert_eq!(transparent_of(after), t_expected);

    // Los píxeles que no valían `index` resuelven igual; los que sí, al resuelto de `into`.
    for (p, &v) in pix_before.iter().enumerate() {
        if usize::from(v) == index {
            prop_assert_eq!(res_after[p], res_into, "píxel {}", p);
        } else {
            prop_assert_eq!(res_after[p], res_before[p], "píxel {}", p);
        }
    }

    // Ningún píxel `>= len` y el recuento de `into'`.
    prop_assert!(pix_after.iter().all(|&v| usize::from(v) < n - 1));
    let were_valid = |v: usize| pix_before.iter().filter(|&&p| usize::from(p) == v).count();
    let now = pix_after
        .iter()
        .filter(|&&p| usize::from(p) == into_new)
        .count();
    prop_assert_eq!(now, were_valid(into) + were_valid(index));
    checks_invariant(&doc, id)?;

    // `undo` restaura la imagen `==` y `redo` la quitada.
    let removed = after.clone();
    prop_assert_eq!(doc.undo(), Ok(()));
    prop_assert_eq!(image_of(&doc), &img);
    prop_assert_eq!(doc.redo(), Ok(()));
    prop_assert_eq!(image_of(&doc), &removed);
    Ok(())
}

// Propiedad 4: secuencias mezcladas.

#[derive(Clone, Debug)]
enum Op {
    /// Un comando de paleta suelto, a veces sobre un identificador desconocido.
    Palette2 {
        kind: u8,
        c: Raw,
        unknown: bool,
    },
    /// Un `Batch` con un comando de paleta, a veces tras un `SetPixels` válido.
    Batch {
        kind: u8,
        c: Raw,
        with_set: bool,
    },
    Set {
        edits: Vec<(u32, u32, u8)>,
        bad_2: bool,
    },
    Fill {
        y: u32,
        x0: u32,
        x1: u32,
        v: u8,
        bad_2: bool,
    },
    Convert {
        target: Mode,
        allowed: Allowed,
    },
    Undo,
    Redo,
}

fn op() -> impl Strategy<Value = Op> {
    let bad_2 = || prop_oneof![5 => Just(false), 1 => Just(true)];
    prop_oneof![
        12 => (any::<u8>(), raw(), prop_oneof![9 => Just(false), 1 => Just(true)])
            .prop_map(|(kind, c, unknown)| Op::Palette2 { kind, c, unknown }),
        2 => (any::<u8>(), raw(), any::<bool>())
            .prop_map(|(kind, c, with_set)| Op::Batch { kind, c, with_set }),
        2 => (prop::collection::vec((0u32..8, 0u32..8, any::<u8>()), 1..=3), bad_2())
            .prop_map(|(edits, bad_2)| Op::Set { edits, bad_2 }),
        1 => (0u32..8, 0u32..8, 0u32..8, any::<u8>(), bad_2())
            .prop_map(|(y, x0, x1, v, bad_2)| Op::Fill { y, x0, x1, v, bad_2 }),
        2 => (
            prop::sample::select(vec![Mode::Rgba, Mode::GrayAlpha, Mode::Gray, Mode::Indexed]),
            any::<bool>(),
            prop::option::of(any::<[u8; 3]>()),
        )
            .prop_map(|(target, luma, drop_alpha)| Op::Convert {
                target,
                allowed: Allowed { luma, drop_alpha },
            }),
        3 => Just(Op::Undo),
        3 => Just(Op::Redo),
    ]
}

/// Ejecuta `ops` sobre un documento y sobre un modelo de instantáneas con un cursor.
fn runs_sequence(initial: Image, ops: &[Op]) -> Result<(), TestCaseError> {
    let mut doc = Document::new(initial.clone());
    // Un límite de historia tan grande que nunca se descarta un paso.
    doc.set_history_limit(usize::MAX);
    let id = ImageId::from_raw(0);
    let mut instants = vec![initial];
    let mut cursor = 0usize;

    // Registra un paso nuevo: la pila de rehacer se vacía y la imagen actual es la nueva.
    fn new_step(doc: &Document, id: ImageId, instants: &mut Vec<Image>, cursor: &mut usize) {
        instants.truncate(*cursor + 1);
        instants.push(doc.image(id).unwrap().clone());
        *cursor += 1;
    }

    for op in ops {
        let actual = instants[cursor].clone();
        let before = state(&doc);
        match op {
            Op::Palette2 { kind, c, unknown } => {
                let goal = if *unknown {
                    ImageId::from_raw(1 + u32::from(c.a % 3))
                } else {
                    id
                };
                let cmd = command_palette(*kind, c, goal, length_of_palette(&actual));
                let expected_2 = predicts((!unknown).then_some(&actual), goal, &cmd);
                let real = doc.apply(cmd.clone());
                match expected_2 {
                    Expected::Err(e) => {
                        prop_assert_eq!(real, Err(e), "{:?}", cmd);
                        prop_assert_eq!(state(&doc), before, "{:?}", cmd);
                    }
                    Expected::WithoutStep => {
                        prop_assert_eq!(real, Ok(false), "{:?}", cmd);
                        prop_assert_eq!(state(&doc), before, "{:?}", cmd);
                    }
                    Expected::Step2 => {
                        prop_assert_eq!(real, Ok(true), "{:?}", cmd);
                        prop_assert_ne!(doc.state_id(), before.state_id);
                        new_step(&doc, id, &mut instants, &mut cursor);
                    }
                }
            }
            Op::Batch { kind, c, with_set } => {
                let cmd = command_palette(*kind, c, id, length_of_palette(&actual));
                let mut commands = Vec::new();
                if *with_set {
                    commands.push(set(id, 0, 0, pixel_for(&actual, c.kind, false)));
                }
                commands.push(cmd);
                let real = doc.apply(Command::Batch(commands));
                prop_assert_eq!(
                    real,
                    Err(in_batch(
                        usize::from(*with_set),
                        DocumentError::NotBatchable
                    ))
                );
                prop_assert_eq!(state(&doc), before);
            }
            Op::Set { edits, bad_2 } => {
                let pixels: Vec<PixelEdit> = edits
                    .iter()
                    .map(|&(x, y, v)| PixelEdit {
                        x,
                        y,
                        pixel: pixel_for(&actual, v, *bad_2),
                    })
                    .collect();
                match doc.apply(Command::SetPixels { image: id, pixels }) {
                    Ok(true) => new_step(&doc, id, &mut instants, &mut cursor),
                    Ok(false) | Err(_) => prop_assert_eq!(state(&doc), before),
                }
            }
            Op::Fill {
                y,
                x0,
                x1,
                v,
                bad_2,
            } => {
                let span = Span {
                    y: *y,
                    x_start: *x0,
                    x_end: *x1,
                };
                let pixel = pixel_for(&actual, *v, *bad_2);
                match doc.apply(Command::FillSpans {
                    image: id,
                    spans: vec![span],
                    pixel,
                }) {
                    Ok(true) => new_step(&doc, id, &mut instants, &mut cursor),
                    Ok(false) | Err(_) => prop_assert_eq!(state(&doc), before),
                }
            }
            Op::Convert { target, allowed } => {
                match doc.apply(Command::ConvertMode {
                    image: id,
                    target: *target,
                    allowed: *allowed,
                }) {
                    Ok(true) => new_step(&doc, id, &mut instants, &mut cursor),
                    Ok(false) | Err(_) => prop_assert_eq!(state(&doc), before),
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
        // La invariante central y el documento es la instantánea del cursor.
        checks_invariant(&doc, id)?;
        prop_assert_eq!(doc.image(id), Some(&instants[cursor]));
        prop_assert_eq!(doc.can_undo(), cursor > 0);
        prop_assert_eq!(doc.can_redo(), cursor + 1 < instants.len());
    }

    // Deshacer todo pasa por cada instantánea anterior y llega a la inicial exacta.
    let last_instant = instants.len() - 1;
    while cursor > 0 {
        prop_assert_eq!(doc.undo(), Ok(()));
        cursor -= 1;
        prop_assert_eq!(doc.image(id), Some(&instants[cursor]));
        checks_invariant(&doc, id)?;
    }
    prop_assert_eq!(doc.undo(), Err(DocumentError::NothingToUndo));
    prop_assert_eq!(doc.image(id), Some(&instants[0]));
    // Rehacer todo pasa por cada una y llega a la final exacta.
    while cursor < last_instant {
        prop_assert_eq!(doc.redo(), Ok(()));
        cursor += 1;
        prop_assert_eq!(doc.image(id), Some(&instants[cursor]));
        checks_invariant(&doc, id)?;
    }
    prop_assert_eq!(doc.redo(), Err(DocumentError::NothingToRedo));
    prop_assert_eq!(doc.image(id), instants.last());
    Ok(())
}

// Propiedad 5: validación y atomicidad.

fn prop_validation(
    img: Image,
    v: u8,
    undo: bool,
    kind: u8,
    c: Raw,
    unknown: bool,
    shape: u8,
) -> Result<(), TestCaseError> {
    let mut doc = doc_with_history(img, v, undo);
    let main = ImageId::from_raw(0);
    let id = if unknown {
        ImageId::from_raw(1 + u32::from(c.a % 4))
    } else {
        main
    };
    let actual = image_of(&doc).clone();
    let cmd = command_palette(kind, &c, id, length_of_palette(&actual));
    let before = state(&doc);

    match shape % 4 {
        // Dentro de un `Batch`: `NotBatchable` gana a cualquier otro error del comando.
        2 => {
            let batch = Command::Batch(vec![cmd]);
            prop_assert_eq!(
                doc.apply(batch),
                Err(in_batch(0, DocumentError::NotBatchable))
            );
            prop_assert_eq!(state(&doc), before);
        }
        3 => {
            let batch =
                Command::Batch(vec![set(main, 0, 0, pixel_for(&actual, v ^ 1, false)), cmd]);
            prop_assert_eq!(
                doc.apply(batch),
                Err(in_batch(1, DocumentError::NotBatchable))
            );
            prop_assert_eq!(state(&doc), before);
        }
        _ => {
            let expected_2 = predicts((!unknown).then_some(&actual), id, &cmd);
            let real = doc.apply(cmd.clone());
            match expected_2 {
                Expected::Err(e) => {
                    prop_assert_eq!(real, Err(e), "{:?}", cmd);
                    prop_assert_eq!(state(&doc), before, "{:?}", cmd);
                }
                Expected::WithoutStep => {
                    prop_assert_eq!(real, Ok(false), "{:?}", cmd);
                    prop_assert_eq!(state(&doc), before, "{:?}", cmd);
                }
                Expected::Step2 => {
                    prop_assert_eq!(real, Ok(true), "{:?}", cmd);
                    prop_assert_ne!(doc.state_id(), before.state_id);
                    prop_assert!(doc.can_undo());
                    prop_assert!(!doc.can_redo());
                    prop_assert_eq!(steps_undoable(&doc), before.undoable + 1);
                    checks_invariant(&doc, main)?;
                }
            }
        }
    }
    Ok(())
}

// Propiedad 6: coste y límite.

/// Aplica `cmd` al documento; si da `Ok(true)` comprueba que `history_bytes()` tras `redo`
/// es el de tras `apply` y devuelve cuánto subió.
fn measures(
    doc: &mut Document,
    cmd: &Command,
) -> Result<(Result<bool, DocumentError>, usize), TestCaseError> {
    let before = doc.history_bytes();
    let real = doc.apply(cmd.clone());
    if real != Ok(true) {
        prop_assert_eq!(doc.history_bytes(), before);
        return Ok((real, 0));
    }
    let after_apply = doc.history_bytes();
    prop_assert_eq!(doc.undo(), Ok(()));
    prop_assert_eq!(doc.history_bytes(), before);
    prop_assert_eq!(doc.redo(), Ok(()));
    prop_assert_eq!(doc.history_bytes(), after_apply);
    Ok((real, after_apply - before))
}

fn prop_cost(
    small: Image,
    w2: u32,
    h2: u32,
    seed: u64,
    kind: u8,
    c: Raw,
) -> Result<(), TestCaseError> {
    // La misma paleta en dos tamaños de imagen.
    let colors_2 = palette_of(&small);
    let t = transparent_of(&small);
    let n = colors_2.len();
    let pixels: Vec<u8> = (0..w2 * h2)
        .map(|i| {
            let h = seed
                .wrapping_add(u64::from(i))
                .wrapping_mul(0x9E37_79B9_7F4A_7C15);
            u8::try_from(((h >> 40) as usize) % n).unwrap()
        })
        .collect();
    let large = indexed(w2, h2, &colors_2, t, &pixels);
    let id = ImageId::from_raw(0);
    let cmd = command_palette(kind, &c, id, n);

    let images = [small, large];
    let mut results = Vec::new();
    for img in &images {
        let mut doc = Document::new(img.clone());
        let (real, cost) = measures(&mut doc, &cmd)?;
        if let Command::RemovePaletteEntry { index, .. } = &cmd {
            if real == Ok(true) {
                let used = pixels_of(img).contains(index);
                let cels = (img.width() as usize * img.height() as usize).div_ceil(64);
                let mask = if used { cels * 8 } else { 0 };
                prop_assert!(
                    (mask..=mask + 512).contains(&cost),
                    "Remove subió {} con máscara {}",
                    cost,
                    mask
                );
            }
        } else if real == Ok(true) {
            prop_assert!(cost <= 512, "{:?} subió {}", cmd, cost);
        }
        results.push((real, cost));
    }
    // El mismo resultado y, salvo en `Remove`, el mismo coste en los dos tamaños.
    prop_assert_eq!(&results[0].0, &results[1].0);
    if !matches!(cmd, Command::RemovePaletteEntry { .. }) {
        prop_assert_eq!(results[0].1, results[1].1, "{:?}", cmd);
    }
    Ok(())
}

/// Imagen de hasta 128 x 128 con todos los píxeles en el índice 0 (para que un `Remove` pueda
/// tener una máscara mayor que el límite).
fn image_large() -> impl Strategy<Value = Image> {
    (
        1u32..=128,
        1u32..=128,
        prop::collection::vec(color_p(), 2..=16),
    )
        .prop_map(|(w, h, colors_2)| indexed(w, h, &colors_2, None, &vec![0; (w * h) as usize]))
}

fn prop_limit(img: Image, limit: usize, ops: &[(u8, Raw)]) -> Result<(), TestCaseError> {
    let mut doc = Document::new(img);
    doc.set_history_limit(limit);
    let id = ImageId::from_raw(0);
    let mut was_step = false;

    for (kind, c) in ops {
        let actual = image_of(&doc).clone();
        let before = state(&doc);
        let cmd = command_palette(*kind, c, id, length_of_palette(&actual));
        match doc.apply(cmd.clone()) {
            Ok(true) => {
                was_step = true;
                // El último paso nunca se descarta: se puede deshacer y deja la anterior.
                let mut copy = doc.clone();
                prop_assert_eq!(copy.undo(), Ok(()), "{:?}", cmd);
                prop_assert_eq!(copy.image(id), Some(&actual), "{:?}", cmd);
            }
            Ok(false) | Err(_) => prop_assert_eq!(state(&doc), before, "{:?}", cmd),
        }
        let kept = steps_undoable(&doc);
        prop_assert!(
            doc.history_bytes() <= limit || kept <= 1,
            "history_bytes = {} > límite {} con {} pasos",
            doc.history_bytes(),
            limit,
            kept
        );
        prop_assert_eq!(doc.history_limit(), limit);
        if was_step {
            prop_assert!(kept >= 1, "el último paso siempre se conserva");
            prop_assert!(doc.can_undo());
        }
    }
    Ok(())
}

// Propiedad 7: cada edición toca solo lo suyo.

fn prop_touches_only_its_own(img: Image, a: u16, b: u16, color: Rgba) -> Result<(), TestCaseError> {
    let old = palette_of(&img);
    let n = old.len();
    let t = transparent_of(&img);
    let pix = pixels_of(&img);
    let res = resolved(&img);
    let i = adjusts(a, n);

    // `SetPaletteColor(i, c)`: los mismos bytes y el resuelto cambia solo en los píxeles de
    // valor `i`.
    let mut doc = Document::new(img.clone());
    let id = ImageId::from_raw(0);
    let real = doc.apply(put_color(id, i as u8, color));
    prop_assert_eq!(real, Ok(old[i] != color));
    let after = image_of(&doc);
    prop_assert_eq!(pixels_of(after), pix.clone());
    prop_assert_eq!(transparent_of(after), t);
    let res_after = resolved(after);
    for (p, &v) in pix.iter().enumerate() {
        if usize::from(v) == i {
            prop_assert_eq!(res_after[p], visible(color, t == Some(v)), "píxel {}", p);
        } else {
            prop_assert_eq!(res_after[p], res[p], "píxel {}", p);
        }
    }
    let mut palette_expected = old.clone();
    palette_expected[i] = color;
    prop_assert_eq!(palette_of(after), palette_expected);

    // `SetTransparentIndex`: los mismos bytes; solo cambia el alfa de los píxeles que usan
    // la entrada afectada (la antigua y la nueva).
    let new_t = if b.is_multiple_of(5) {
        None
    } else {
        Some(adjusts(b, n) as u8)
    };
    let mut doc = Document::new(img.clone());
    let real = doc.apply(transparent(id, new_t));
    prop_assert_eq!(real, Ok(new_t != t));
    let after = image_of(&doc);
    prop_assert_eq!(pixels_of(after), pix.clone());
    prop_assert_eq!(palette_of(after), old.clone());
    prop_assert_eq!(transparent_of(after), new_t);
    let res_after = resolved(after);
    for (p, &v) in pix.iter().enumerate() {
        let affected = t == Some(v) || new_t == Some(v);
        if affected {
            prop_assert_eq!(
                res_after[p],
                visible(old[usize::from(v)], new_t == Some(v)),
                "píxel {}",
                p
            );
            // El RGB nunca cambia: solo el alfa.
            prop_assert_eq!(
                (res_after[p].r, res_after[p].g, res_after[p].b),
                (res[p].r, res[p].g, res[p].b)
            );
        } else {
            prop_assert_eq!(res_after[p], res[p], "píxel {}", p);
        }
    }

    // `Add` no cambia ni un píxel ni el transparente.
    if n < 256 {
        let mut doc = Document::new(img.clone());
        prop_assert_eq!(doc.apply(adds(id, color)), Ok(true));
        let after = image_of(&doc);
        prop_assert_eq!(pixels_of(after), pix.clone());
        prop_assert_eq!(transparent_of(after), t);
        let mut expected = old.clone();
        expected.push(color);
        prop_assert_eq!(palette_of(after), expected);
        prop_assert_eq!(resolved(after), res.clone());
    }
    Ok(())
}

// Propiedad 8: `RotateColors`.

fn prop_rotate(
    img: Image,
    a: u16,
    b: u16,
    steps_in_raw: i32,
    sel: u8,
    m: i16,
) -> Result<(), TestCaseError> {
    let old = palette_of(&img);
    let n = old.len();
    let t = transparent_of(&img);
    let pix = pixels_of(&img);
    let res_before = resolved(&img);

    // Rango válido (el inválido se prueba en la propiedad 5) y `steps` de varias clases.
    let start = adjusts(a, n);
    let rest = n - start;
    let len = match b % 5 {
        0 => rest,
        1 => 0,
        2 => 1,
        _ => usize::from(b / 5) % (rest + 1),
    };
    let len_i32 = i32::try_from(len).unwrap();
    let steps = match sel % 5 {
        0 => steps_in_raw,
        1 => len_i32 * i32::from(m),
        2 => i32::MIN,
        3 => i32::MAX,
        _ => i32::from(m),
    };
    let (start_u8, len_u16) = (start as u8, len as u16);

    // El oráculo: rotación a mano y color resuelto de cada píxel (el transparente se queda
    // en su ranura).
    let k = modulo(steps, len);
    let new = rotates_by_hand(&old, start, len, k);
    let changes = len >= 2 && k != 0 && new != old;

    let mut doc = Document::new(img.clone());
    let id = ImageId::from_raw(0);
    let before = state(&doc);
    let real = doc.apply(rotates(id, start_u8, len_u16, steps));
    prop_assert_eq!(
        real,
        Ok(changes),
        "start {} len {} steps {}",
        start,
        len,
        steps
    );
    if !changes {
        prop_assert_eq!(state(&doc), before);
    }
    let after = image_of(&doc);
    prop_assert_eq!(palette_of(after), new.clone());
    // Los índices de los píxeles no cambian.
    prop_assert_eq!(pixels_of(after), pix.clone());
    prop_assert_eq!(transparent_of(after), t);
    let res_after = resolved(after);
    for (p, &v) in pix.iter().enumerate() {
        let expected_2 = visible(new[usize::from(v)], t == Some(v));
        prop_assert_eq!(res_after[p], expected_2, "píxel {}", p);
        // Los píxeles fuera del rango resuelven igual que antes.
        let usv = usize::from(v);
        if usv < start || usv >= start + len {
            prop_assert_eq!(res_after[p], res_before[p], "píxel {} fuera del rango", p);
        }
    }
    // Se conserva el multiconjunto de colores de la paleta.
    let mut ordered_old = old.clone();
    let mut ordered_new = palette_of(after);
    ordered_old.sort_by_key(|c| (c.r, c.g, c.b, c.a));
    ordered_new.sort_by_key(|c| (c.r, c.g, c.b, c.a));
    prop_assert_eq!(ordered_new, ordered_old);
    checks_invariant(&doc, id)?;

    if changes {
        // Rotar `-k` devuelve la paleta exacta.
        let rotated = after.clone();
        let k_i32 = i32::try_from(k).unwrap();
        prop_assert_eq!(doc.apply(rotates(id, start_u8, len_u16, -k_i32)), Ok(true));
        prop_assert_eq!(image_of(&doc), &img);
        prop_assert_eq!(doc.undo(), Ok(()));
        prop_assert_eq!(image_of(&doc), &rotated);
        // `undo` y `redo` son exactos.
        prop_assert_eq!(doc.undo(), Ok(()));
        prop_assert_eq!(image_of(&doc), &img);
        prop_assert_eq!(doc.redo(), Ok(()));
        prop_assert_eq!(image_of(&doc), &rotated);
    }

    // Rotar `len` (y cualquier múltiplo, también negativo) da `Ok(false)` sin paso.
    let mut doc = Document::new(img.clone());
    without_step(
        &mut doc,
        rotates(id, start_u8, len_u16, len_i32 * i32::from(m)),
    );
    without_step(&mut doc, rotates(id, start_u8, len_u16, len_i32));
    without_step(&mut doc, rotates(id, start_u8, len_u16, -len_i32));
    Ok(())
}

proptest! {
    // Propiedad 1: `PermutePalette` contra el oráculo, y ida y vuelta.
    #[test]
    fn property_1_permute_oracle(
        img in image_idx(8),
        seed in any::<u64>(),
        identity in prop_oneof![1 => Just(true), 7 => Just(false)],
    ) {
        let n = length_of_palette(&img);
        let order = if identity {
            (0..n).map(|i| i as u8).collect()
        } else {
            permutation_of(n, seed)
        };
        // La inversa: `inverse[order[i]] = i`.
        let mut inverse = vec![0u8; n];
        for (new_2, &old_2) in order.iter().enumerate() {
            inverse[usize::from(old_2)] = new_2 as u8;
        }
        let id = ImageId::from_raw(0);
        checks_reorder(&img, permutes(id, &order), &order, permutes(id, &inverse))?;
    }

    // Propiedad 1: `MovePaletteEntry` contra el oráculo, y ida y vuelta con `Move(to, from)`.
    #[test]
    fn property_1_move_oracle(img in image_idx(8), a in any::<u16>(), b in any::<u16>()) {
        let n = length_of_palette(&img);
        let (from, to) = (adjusts(a, n), adjusts(b, n));
        let order = order_of_move(n, from, to);
        let id = ImageId::from_raw(0);
        checks_reorder(
            &img,
            moves(id, from as u8, to as u8),
            &order,
            moves(id, to as u8, from as u8),
        )?;
    }

    // Propiedad 2: `Move` equivale a `Permute`.
    #[test]
    fn property_2_move_is_equivalent_to_permute(
        img in image_idx(8),
        a in any::<u16>(),
        b in any::<u16>(),
    ) {
        prop_move_is_permute(img, a, b)?;
    }

    // Propiedad 3: oráculo de `RemovePaletteEntry`.
    #[test]
    fn property_3_remove_oracle(img in image_idx(8), a in any::<u16>(), b in any::<u16>()) {
        prop_remove(img, a, b)?;
    }

    // Propiedad 4: secuencias mezcladas de los siete comandos, `SetPixels`, `FillSpans`,
    // `ConvertMode`, `undo` y `redo`.
    #[test]
    fn property_4_sequences_mixed(
        img in image_mixed(),
        ops in prop::collection::vec(op(), 0..=24),
    ) {
        runs_sequence(img, &ops)?;
    }

    // Propiedad 5: validación (contra un predicado independiente) y atomicidad.
    #[test]
    fn property_5_validation_and_atomicity(
        img in image_mixed(),
        v in any::<u8>(),
        undo in any::<bool>(),
        kind in any::<u8>(),
        c in raw(),
        unknown in prop_oneof![9 => Just(false), 1 => Just(true)],
        shape in any::<u8>(),
    ) {
        prop_validation(img, v, undo, kind, c, unknown, shape)?;
    }

    // Propiedad 6, primera parte: coste independiente del tamaño de la imagen.
    #[test]
    fn property_6_step_cost(
        small in image_idx(8),
        w2 in 1u32..=64,
        h2 in 1u32..=64,
        seed in any::<u64>(),
        kind in any::<u8>(),
        c in raw(),
    ) {
        prop_cost(small, w2, h2, seed, kind, c)?;
    }

    // Propiedad 6, segunda parte: con un límite de 0 a 2000 bytes, `history_bytes() <= límite`
    // o solo queda un paso, y el último paso nunca se descarta.
    #[test]
    fn property_6_history_limit(
        img in prop_oneof![1 => image_idx(8), 1 => image_large()],
        limit in 0usize..=2000,
        ops in prop::collection::vec((any::<u8>(), raw()), 1..=8),
    ) {
        prop_limit(img, limit, &ops)?;
    }

    // Propiedad 7: cada edición toca solo lo suyo.
    #[test]
    fn property_7_each_edit_touches_only_its_own(
        img in image_idx(8),
        a in any::<u16>(),
        b in any::<u16>(),
        color in color_p(),
    ) {
        prop_touches_only_its_own(img, a, b, color)?;
    }

    // Propiedad 8: `RotateColors` contra un oráculo que rota a mano.
    #[test]
    fn property_8_rotate_colors(
        img in image_idx(8),
        a in any::<u16>(),
        b in any::<u16>(),
        steps in steps_2(),
        sel in any::<u8>(),
        m in any::<i16>(),
    ) {
        prop_rotate(img, a, b, steps, sel, m)?;
    }
}
