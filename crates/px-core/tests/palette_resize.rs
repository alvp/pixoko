//! Tests of `Command::ResizePalette` (specs/ui-palette-handle.md, section "Cambios en px-core"
//! PC-01 to PC-03, cases 1 to 3, property 1 and the "Visto bueno" conditions 1 to 3).
//!
//! The command changes the length of the one palette of the document (the canvas template and
//! every cell with an image) in a single exclusive step. Growing appends opaque black entries;
//! trimming moves the pixels that use a removed entry to `into` and drops the transparent
//! index if it falls outside.
//!
//! One test per row 1 to 3 (`row_NN_...`), the cost-proportionality test of condition 1, the
//! `into`-outside-the-new-length test of condition 3, and property 1 (`property_01_...`) with
//! a model written in this file (a `Vec` of colors and one `Vec` of pixels per cell). The
//! figures of the rows are computed by hand from the spec; the model does not call the code
//! under test. Cost is only measured with `history_bytes()` and with the bounds of the spec.
//!
//! These tests do not compile until `Command::ResizePalette`, `PaletteEditError::BadLength` and
//! `PaletteEditError::NoDestination` exist.

use proptest::prelude::*;
use proptest::test_runner::TestCaseError;
use px_core::{
    BlendMode, Command, Document, DocumentError, Image, ImageId, LayerId, LayerSpec, Mode, Palette,
    PaletteEditError, Pixel, PixelEdit, Rgba,
};

// Colors.

const R: Rgba = Rgba::new(255, 0, 0, 255);
const G: Rgba = Rgba::new(0, 255, 0, 255);
const B: Rgba = Rgba::new(0, 0, 255, 255);
const Y: Rgba = Rgba::new(255, 255, 0, 255);
/// Opaque black: the color of the entries that growing appends.
const K: Rgba = Rgba::new(0, 0, 0, 255);

/// `n` distinct opaque colors, none of them opaque black (n <= 256).
fn colors_different(n: usize) -> Vec<Rgba> {
    (0..n)
        .map(|i| Rgba::new(i as u8, 255 - i as u8, 7, 255))
        .collect()
}

// Building documents.

fn palette_of_colors(colors: &[Rgba], transparent: Option<u8>) -> Palette {
    let mut palette = Palette::from_colors(colors.iter().copied()).unwrap();
    palette.set_transparent_index(transparent).unwrap();
    palette
}

/// Indexed cell image of `w` x `h` with the given palette and pixels (row by row).
fn cell(w: u32, h: u32, colors: &[Rgba], transparent: Option<u8>, pixels: &[u8]) -> Image {
    assert_eq!(pixels.len(), (w * h) as usize);
    let mut img = Image::new_indexed(w, h, palette_of_colors(colors, transparent)).unwrap();
    for (i, &v) in pixels.iter().enumerate() {
        img.set_pixel(i as u32 % w, i as u32 / w, Pixel::Indexed(v))
            .unwrap();
    }
    img
}

fn layer(name: &str, visible: bool, locked: bool, image: Option<Image>) -> LayerSpec {
    LayerSpec {
        name: name.to_string(),
        visible,
        opacity: 255,
        blend: BlendMode::Normal,
        locked,
        image,
    }
}

/// Indexed document with the given layers (bottom to top). `LayerId(i)` and `ImageId(i)` are
/// the position `i`.
fn document(
    w: u32,
    h: u32,
    colors: &[Rgba],
    transparent: Option<u8>,
    layers: Vec<LayerSpec>,
) -> Document {
    Document::from_layers(
        w,
        h,
        Mode::Indexed,
        Some(palette_of_colors(colors, transparent)),
        layers,
    )
    .unwrap()
}

/// Single-layer indexed document.
fn single(w: u32, h: u32, colors: &[Rgba], transparent: Option<u8>, pixels: &[u8]) -> Document {
    document(
        w,
        h,
        colors,
        transparent,
        vec![layer(
            "L0",
            true,
            false,
            Some(cell(w, h, colors, transparent, pixels)),
        )],
    )
}

fn cel_id(i: u32) -> ImageId {
    ImageId::from_raw(i)
}

fn resize(image: ImageId, len: u16, into: Option<u8>) -> Command {
    Command::ResizePalette { image, len, into }
}

fn set_pixel(image: ImageId, x: u32, y: u32, v: u8) -> Command {
    Command::SetPixels {
        image,
        pixels: vec![PixelEdit {
            x,
            y,
            pixel: Pixel::Indexed(v),
        }],
    }
}

// Reading documents.

fn pixels_of_image(img: &Image) -> Vec<u8> {
    let mut v = Vec::new();
    for y in 0..img.height() {
        for x in 0..img.width() {
            match img.get_pixel(x, y) {
                Ok(Pixel::Indexed(i)) => v.push(i),
                other => panic!("an indexed pixel was expected and {other:?} came out"),
            }
        }
    }
    v
}

fn palette_colors(doc: &Document) -> Vec<Rgba> {
    doc.palette()
        .map(|p| p.colors().to_vec())
        .unwrap_or_default()
}

fn transparent_of(doc: &Document) -> Option<u8> {
    doc.palette().and_then(Palette::transparent_index)
}

/// One layer: ids, then (palette, transparent index, pixels) if the cell has an image.
type CellPrint = (LayerId, ImageId, Option<(Vec<Rgba>, Option<u8>, Vec<u8>)>);

/// Explicit comparison of the four data that condition 2 names: `LayerId` and `ImageId`,
/// palette, transparent index, pixels, and which cells are empty. It also includes the
/// palette of the canvas template.
#[derive(Debug, PartialEq, Eq)]
struct Print {
    cells: Vec<CellPrint>,
    canvas_colors: Vec<Rgba>,
    canvas_transparent: Option<u8>,
}

fn print_of(doc: &Document) -> Print {
    let cells = doc
        .layers()
        .iter()
        .map(|l| {
            let detail = doc.image(l.cel()).map(|img| {
                let p = img.palette();
                (
                    p.map(|p| p.colors().to_vec()).unwrap_or_default(),
                    p.and_then(Palette::transparent_index),
                    if p.is_some() {
                        pixels_of_image(img)
                    } else {
                        Vec::new()
                    },
                )
            });
            (l.id(), l.cel(), detail)
        })
        .collect();
    Print {
        cells,
        canvas_colors: palette_colors(doc),
        canvas_transparent: transparent_of(doc),
    }
}

/// Checks the palette, the transparent index and the pixels of every cell (`None` = empty
/// cell), that every palette (canvas template and cells) is identical, that no pixel is
/// outside the palette and that the template is coherent.
fn expect_doc(doc: &Document, colors: &[Rgba], transparent: Option<u8>, cells: &[Option<&[u8]>]) {
    assert_eq!(palette_colors(doc), colors);
    assert_eq!(transparent_of(doc), transparent);
    assert_eq!(doc.layers().len(), cells.len());
    for (layer, expected) in doc.layers().iter().zip(cells) {
        let img = doc.image(layer.cel());
        match (img, expected) {
            (None, None) => {}
            (Some(img), Some(expected)) => {
                assert_eq!(
                    pixels_of_image(img).as_slice(),
                    *expected,
                    "cell {:?}",
                    layer.cel()
                );
                assert_eq!(img.palette(), doc.palette(), "cell {:?}", layer.cel());
                assert!(
                    pixels_of_image(img)
                        .iter()
                        .all(|&v| usize::from(v) < colors.len())
                );
            }
            (img, expected) => panic!(
                "cell {:?}: image {:?}, expected {:?}",
                layer.cel(),
                img.map(|_| "with image"),
                expected
            ),
        }
    }
    let template = doc.canvas_image();
    assert_eq!(template.palette(), doc.palette());
    match template.get_pixel(0, 0) {
        Ok(Pixel::Indexed(v)) => assert!(usize::from(v) < colors.len(), "template pixel {v}"),
        other => panic!("the template must have an indexed pixel and has {other:?}"),
    }
}

// Errors.

fn editing(e: PaletteEditError) -> DocumentError {
    DocumentError::PaletteEdit(e)
}

fn outside(index: u8, len: usize) -> DocumentError {
    editing(PaletteEditError::IndexOutOfRange { index, len })
}

fn bad_length(len: u16) -> DocumentError {
    editing(PaletteEditError::BadLength { len })
}

fn in_batch(index: usize, source: DocumentError) -> DocumentError {
    DocumentError::InBatch {
        index,
        source: Box::new(source),
    }
}

// Observable state of the document.

/// Bound so that a defective `undo`/`redo` that never fails cannot hang the tests.
const CAP_STEPS: usize = 100_000;

fn steps_undoable(doc: &Document) -> usize {
    let mut copy = doc.clone();
    let mut n = 0;
    while n < CAP_STEPS && copy.undo().is_ok() {
        n += 1;
    }
    n
}

/// Everything a failed or no-op command must not touch.
#[derive(Debug, PartialEq, Eq)]
struct State {
    doc: Document,
    print: Print,
    can_undo: bool,
    can_redo: bool,
    bytes: usize,
    state_id: u64,
    undoable: usize,
}

fn state(doc: &Document) -> State {
    State {
        doc: doc.clone(),
        print: print_of(doc),
        can_undo: doc.can_undo(),
        can_redo: doc.can_redo(),
        bytes: doc.history_bytes(),
        state_id: doc.state_id(),
        undoable: steps_undoable(doc),
    }
}

/// `apply(cmd)` gives exactly `expected` and touches nothing observable.
fn rejects(doc: &mut Document, cmd: Command, expected: DocumentError) {
    let before = state(doc);
    assert_eq!(doc.apply(cmd.clone()), Err(expected), "{cmd:?}");
    assert_eq!(state(doc), before, "{cmd:?}");
}

/// `apply(cmd)` gives `Ok(false)` and touches nothing observable (no step, same `state_id`).
fn without_step(doc: &mut Document, cmd: Command) {
    let before = state(doc);
    assert_eq!(doc.apply(cmd.clone()), Ok(false), "{cmd:?}");
    assert_eq!(state(doc), before, "{cmd:?}");
}

/// `apply(cmd)` gives `Ok(true)`: exactly one new step, a new `state_id`, empty redo stack.
fn with_step(doc: &mut Document, cmd: Command) {
    let id_before = doc.state_id();
    let steps_before = steps_undoable(doc);
    assert_eq!(doc.apply(cmd.clone()), Ok(true), "{cmd:?}");
    assert_ne!(doc.state_id(), id_before, "{cmd:?}");
    assert!(doc.can_undo(), "{cmd:?}");
    assert!(!doc.can_redo(), "{cmd:?}");
    assert_eq!(steps_undoable(doc), steps_before + 1, "{cmd:?}");
}

/// Applies `cmd` (which must give `Ok(true)`), undoes it and redoes it; checks that
/// `history_bytes()` after `redo` is the one after `apply` and that `undo` goes back to the
/// previous one. Returns how much `history_bytes()` went up.
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

// Row 1: growing.

/// 3 x 2, palette `[R,G,B,Y]`, transparent `Some(1)`; layers: A (image), B (empty), C (image).
fn row_1_document() -> Document {
    let colors = [R, G, B, Y];
    document(
        3,
        2,
        &colors,
        Some(1),
        vec![
            layer(
                "A",
                true,
                false,
                Some(cell(3, 2, &colors, Some(1), &[0, 1, 2, 3, 0, 3])),
            ),
            layer("B", true, false, None),
            layer(
                "C",
                true,
                false,
                Some(cell(3, 2, &colors, Some(1), &[3, 3, 2, 1, 0, 0])),
            ),
        ],
    )
}

const ROW_1_A: [u8; 6] = [0, 1, 2, 3, 0, 3];
const ROW_1_C: [u8; 6] = [3, 3, 2, 1, 0, 0];

#[test]
fn row_01_growing_adds_black_entries_in_one_step_and_undo_redo_are_exact() {
    let mut doc = row_1_document();
    let id = cel_id(0);
    let original = doc.clone();
    let original_print = print_of(&doc);
    expect_doc(
        &doc,
        &[R, G, B, Y],
        Some(1),
        &[Some(&ROW_1_A), None, Some(&ROW_1_C)],
    );

    // len 8: the 4 new entries are opaque black; `into` is ignored when growing.
    with_step(&mut doc, resize(id, 8, Some(9)));
    expect_doc(
        &doc,
        &[R, G, B, Y, K, K, K, K],
        Some(1),
        &[Some(&ROW_1_A), None, Some(&ROW_1_C)],
    );
    assert_eq!(steps_undoable(&doc), 1, "one single step");
    assert_ne!(print_of(&doc), original_print);
    let grown = doc.clone();
    let grown_print = print_of(&doc);
    let grown_id = doc.state_id();

    // `undo` and `redo` are exact, ids included.
    assert_eq!(doc.undo(), Ok(()));
    assert_eq!(doc, original);
    assert_eq!(print_of(&doc), original_print);
    assert!(!doc.can_undo());
    assert!(doc.can_redo());
    assert_eq!(doc.redo(), Ok(()));
    assert_eq!(doc, grown);
    assert_eq!(print_of(&doc), grown_print);
    assert_eq!(doc.state_id(), grown_id);

    // len 256 is valid and any cell id works (here the empty one): the palette belongs to
    // the document. The cell stays empty.
    let mut doc = row_1_document();
    with_step(&mut doc, resize(cel_id(1), 256, None));
    let mut expected_colors = vec![R, G, B, Y];
    expected_colors.resize(256, K);
    expect_doc(
        &doc,
        &expected_colors,
        Some(1),
        &[Some(&ROW_1_A), None, Some(&ROW_1_C)],
    );
    assert!(doc.image(cel_id(1)).is_none(), "the empty cell stays empty");
    assert_eq!(doc.undo(), Ok(()));
    assert_eq!(doc, original);
    assert_eq!(print_of(&doc), original_print);
}

#[test]
fn row_01_same_length_bad_lengths_batch_unknown_cell_and_rgba() {
    let mut doc = row_1_document();
    let id = cel_id(0);

    // len 4 with 4 entries: Ok(false) without a step, `into` is not looked at.
    without_step(&mut doc, resize(id, 4, None));
    without_step(&mut doc, resize(id, 4, Some(9)));

    // len 0, 257 and the largest u16: BadLength with the given length.
    for len in [0u16, 257, 1000, u16::MAX] {
        rejects(&mut doc, resize(id, len, None), bad_length(len));
        rejects(&mut doc, resize(id, len, Some(9)), bad_length(len));
    }

    // Inside a Batch: NotBatchable, like the other palette commands.
    let batch = Command::Batch(vec![resize(id, 8, None)]);
    rejects(&mut doc, batch, in_batch(0, DocumentError::NotBatchable));
    let batch = Command::Batch(vec![set_pixel(id, 0, 0, 3), resize(id, 8, None)]);
    rejects(&mut doc, batch, in_batch(1, DocumentError::NotBatchable));
    // NotBatchable wins over an unknown cell and over a bad length.
    let batch = Command::Batch(vec![resize(cel_id(99), 0, None)]);
    rejects(&mut doc, batch, in_batch(0, DocumentError::NotBatchable));

    // Unknown cell, even with a bad length.
    let unknown = cel_id(99);
    rejects(
        &mut doc,
        resize(unknown, 8, None),
        DocumentError::UnknownImage(unknown),
    );
    rejects(
        &mut doc,
        resize(unknown, 0, None),
        DocumentError::UnknownImage(unknown),
    );

    // Not indexed documents: NotIndexed, even with a bad length.
    for mode in [Mode::Rgba, Mode::Gray, Mode::GrayAlpha] {
        let mut other = Document::new(Image::new(2, 1, mode).unwrap());
        let bad = editing(PaletteEditError::NotIndexed { mode });
        rejects(&mut other, resize(cel_id(0), 8, None), bad.clone());
        rejects(&mut other, resize(cel_id(0), 0, Some(3)), bad);
    }

    // With 256 entries and len 256: Ok(false).
    let mut full = single(2, 1, &colors_different(256), None, &[0, 255]);
    without_step(&mut full, resize(cel_id(0), 256, None));
    without_step(&mut full, resize(cel_id(0), 256, Some(7)));
}

// Row 2: trimming without affected pixels.

/// 3 x 2, 6 entries, pixels only < 4; two layers: an image and an empty cell.
fn row_2_document(transparent: Option<u8>) -> Document {
    let colors = colors_different(6);
    document(
        3,
        2,
        &colors,
        transparent,
        vec![
            layer(
                "A",
                true,
                false,
                Some(cell(3, 2, &colors, transparent, &ROW_2_A)),
            ),
            layer("B", true, false, None),
        ],
    )
}

const ROW_2_A: [u8; 6] = [0, 1, 2, 3, 3, 0];

#[test]
fn row_02_trimming_without_affected_pixels() {
    let colors = colors_different(6);
    let kept = colors[..4].to_vec();

    // Transparent Some(5) falls outside: it disappears. `into` None is enough.
    let mut doc = row_2_document(Some(5));
    let original = doc.clone();
    let original_print = print_of(&doc);
    with_step(&mut doc, resize(cel_id(0), 4, None));
    expect_doc(&doc, &kept, None, &[Some(&ROW_2_A), None]);
    assert_eq!(steps_undoable(&doc), 1, "one single step");
    let trimmed = doc.clone();
    let trimmed_print = print_of(&doc);
    // `undo` gives back colors, length and transparent index exactly.
    assert_eq!(doc.undo(), Ok(()));
    assert_eq!(doc, original);
    assert_eq!(print_of(&doc), original_print);
    expect_doc(&doc, &colors, Some(5), &[Some(&ROW_2_A), None]);
    assert_eq!(doc.redo(), Ok(()));
    assert_eq!(doc, trimmed);
    assert_eq!(print_of(&doc), trimmed_print);

    // Transparent Some(2) stays; `into` Some(0) is valid and nothing needs it.
    let mut doc = row_2_document(Some(2));
    let original = doc.clone();
    let original_print = print_of(&doc);
    with_step(&mut doc, resize(cel_id(0), 4, Some(0)));
    expect_doc(&doc, &kept, Some(2), &[Some(&ROW_2_A), None]);
    assert_eq!(doc.undo(), Ok(()));
    assert_eq!(doc, original);
    assert_eq!(print_of(&doc), original_print);

    // Transparent Some(5) with a valid `into`: the transparent index is still dropped.
    let mut doc = row_2_document(Some(5));
    with_step(&mut doc, resize(cel_id(1), 4, Some(0)));
    expect_doc(&doc, &kept, None, &[Some(&ROW_2_A), None]);

    // `into` outside the new length (even if no pixel needs it): IndexOutOfRange with the
    // new length, and nothing changes.
    for transparent in [Some(5), Some(2)] {
        let mut doc = row_2_document(transparent);
        rejects(&mut doc, resize(cel_id(0), 4, Some(4)), outside(4, 4));
        rejects(&mut doc, resize(cel_id(0), 4, Some(9)), outside(9, 4));
    }
}

// Row 3: trimming with pixels in several layers.

const ROW_3_VISIBLE: [u8; 8] = [0, 5, 2, 7, 9, 1, 5, 3];
const ROW_3_HIDDEN: [u8; 8] = [7, 7, 0, 9, 4, 5, 1, 3];

/// 4 x 2, 10 entries, transparent `Some(1)`; layers: visible; hidden and locked; empty. The
/// entries 5, 7 and 9 are used in the two cells with an image.
fn row_3_document() -> Document {
    let colors = colors_different(10);
    document(
        4,
        2,
        &colors,
        Some(1),
        vec![
            layer(
                "visible",
                true,
                false,
                Some(cell(4, 2, &colors, Some(1), &ROW_3_VISIBLE)),
            ),
            layer(
                "hidden and locked",
                false,
                true,
                Some(cell(4, 2, &colors, Some(1), &ROW_3_HIDDEN)),
            ),
            layer("empty", true, false, None),
        ],
    )
}

fn clamp_to(pixels: &[u8], len: u8, into: u8) -> Vec<u8> {
    pixels
        .iter()
        .map(|&v| if v >= len { into } else { v })
        .collect()
}

#[test]
fn row_03_trimming_with_pixels_in_several_layers() {
    let colors = colors_different(10);
    let kept = colors[..5].to_vec();
    let mut doc = row_3_document();
    let original = doc.clone();
    let original_print = print_of(&doc);

    // Without destination: NoDestination and everything intact (document, history, state_id).
    rejects(
        &mut doc,
        resize(cel_id(0), 5, None),
        editing(PaletteEditError::NoDestination),
    );
    // Same with a history to look at (a pixel edit that is undone leaves a redo step).
    assert_eq!(doc.apply(set_pixel(cel_id(0), 0, 0, 4)), Ok(true));
    assert_eq!(doc.undo(), Ok(()));
    assert!(doc.can_redo());
    rejects(
        &mut doc,
        resize(cel_id(2), 5, None),
        editing(PaletteEditError::NoDestination),
    );
    assert!(
        doc.can_redo(),
        "a failed command does not clear the redo stack"
    );
    let after_undo = doc.clone();
    assert_eq!(after_undo, original);

    // With destination 2: every pixel >= 5 of the two cells with an image becomes 2 (also in
    // the hidden and locked one); the rest is equal; the palettes are identical.
    let expected_visible = clamp_to(&ROW_3_VISIBLE, 5, 2);
    let expected_hidden = clamp_to(&ROW_3_HIDDEN, 5, 2);
    assert_eq!(expected_visible, vec![0, 2, 2, 2, 2, 1, 2, 3]);
    assert_eq!(expected_hidden, vec![2, 2, 0, 2, 4, 2, 1, 3]);
    let mut doc = original.clone();
    let bytes_before = doc.history_bytes();
    with_step(&mut doc, resize(cel_id(0), 5, Some(2)));
    expect_doc(
        &doc,
        &kept,
        Some(1),
        &[Some(&expected_visible), Some(&expected_hidden), None],
    );
    assert_eq!(steps_undoable(&doc), 1, "one single step");
    let trimmed = doc.clone();
    let trimmed_print = print_of(&doc);

    // The step costs about what PC-05 says: the removed colors (4 B each), one byte per
    // affected pixel (4 + 4 = 8) and the masks; a loose upper bound with a small constant.
    let cost = doc.history_bytes() - bytes_before;
    assert!(cost >= 8, "history_bytes() went up {cost}");
    assert!(
        cost <= 2 * 8 + 4 * 5 + 512,
        "history_bytes() went up {cost}"
    );

    // A single `undo` gives every pixel back to ITS own index (5, 7 or 9) and the colors.
    assert_eq!(doc.undo(), Ok(()));
    assert_eq!(doc, original);
    assert_eq!(print_of(&doc), original_print);
    expect_doc(
        &doc,
        &colors,
        Some(1),
        &[Some(&ROW_3_VISIBLE), Some(&ROW_3_HIDDEN), None],
    );
    assert!(!doc.can_undo());
    // `redo` repeats it.
    assert_eq!(doc.redo(), Ok(()));
    assert_eq!(doc, trimmed);
    assert_eq!(print_of(&doc), trimmed_print);

    // `into` equal to the transparent index that is kept: allowed; the pixels become
    // transparent.
    let mut doc = original.clone();
    with_step(&mut doc, resize(cel_id(1), 5, Some(1)));
    expect_doc(
        &doc,
        &kept,
        Some(1),
        &[
            Some(&clamp_to(&ROW_3_VISIBLE, 5, 1)),
            Some(&clamp_to(&ROW_3_HIDDEN, 5, 1)),
            None,
        ],
    );
    assert_eq!(doc.undo(), Ok(()));
    assert_eq!(print_of(&doc), original_print);

    // Only the hidden and locked layer uses an entry that goes: it is enough for
    // NoDestination, and with a destination only that cell changes.
    let colors_2 = colors_different(10);
    let build_hidden_only = || {
        document(
            2,
            2,
            &colors_2,
            None,
            vec![
                layer(
                    "visible",
                    true,
                    false,
                    Some(cell(2, 2, &colors_2, None, &[0, 1, 2, 3])),
                ),
                layer(
                    "hidden and locked",
                    false,
                    true,
                    Some(cell(2, 2, &colors_2, None, &[0, 7, 2, 3])),
                ),
            ],
        )
    };
    let mut doc = build_hidden_only();
    rejects(
        &mut doc,
        resize(cel_id(0), 5, None),
        editing(PaletteEditError::NoDestination),
    );
    with_step(&mut doc, resize(cel_id(0), 5, Some(4)));
    expect_doc(
        &doc,
        &colors_2[..5],
        None,
        &[Some(&[0, 1, 2, 3]), Some(&[0, 4, 2, 3])],
    );
}

#[test]
fn row_03_into_outside_the_new_length_is_the_existing_index_out_of_range() {
    // Condition 3: no new error; `IndexOutOfRange { index, len }` with the NEW length.
    let mut doc = row_3_document();
    rejects(&mut doc, resize(cel_id(0), 5, Some(5)), outside(5, 5));
    rejects(&mut doc, resize(cel_id(0), 5, Some(9)), outside(9, 5));
    rejects(&mut doc, resize(cel_id(1), 5, Some(200)), outside(200, 5));
    rejects(&mut doc, resize(cel_id(0), 1, Some(1)), outside(1, 1));

    // Without pixels to move it is also checked (and it is the new length, not the old one).
    let mut doc = row_2_document(Some(2));
    rejects(&mut doc, resize(cel_id(0), 4, Some(5)), outside(5, 4));
    // `into` valid for the old length and not for the new one.
    rejects(&mut doc, resize(cel_id(0), 2, Some(3)), outside(3, 2));
}

#[test]
fn row_03_the_order_of_the_checks() {
    // unknown cell -> not indexed -> BadLength -> same length -> `into` -> NoDestination.
    let mut rgba = Document::new(Image::new(2, 1, Mode::Rgba).unwrap());
    rejects(
        &mut rgba,
        resize(cel_id(77), 0, Some(9)),
        DocumentError::UnknownImage(cel_id(77)),
    );
    rejects(
        &mut rgba,
        resize(cel_id(0), 0, Some(9)),
        editing(PaletteEditError::NotIndexed { mode: Mode::Rgba }),
    );

    let mut doc = row_3_document();
    // BadLength before the same-length rule and before `into`.
    rejects(&mut doc, resize(cel_id(0), 0, Some(9)), bad_length(0));
    rejects(&mut doc, resize(cel_id(0), 257, Some(9)), bad_length(257));
    // Same length: Ok(false) without looking at `into`.
    without_step(&mut doc, resize(cel_id(0), 10, Some(200)));
    // `into` before NoDestination: invalid `into` with affected pixels is IndexOutOfRange.
    rejects(&mut doc, resize(cel_id(0), 5, Some(9)), outside(9, 5));
    // Valid `into` missing: NoDestination.
    rejects(
        &mut doc,
        resize(cel_id(0), 5, None),
        editing(PaletteEditError::NoDestination),
    );
}

// Condition 1: the cost of the step is proportional to the affected pixels.

/// 64 x 64, 256 entries, every pixel 0 except the 10 x 10 corner, which uses entry 255.
fn corner_document() -> Document {
    let colors = colors_different(256);
    let mut pixels = vec![0u8; 64 * 64];
    for y in 0..10 {
        for x in 0..10 {
            pixels[y * 64 + x] = 255;
        }
    }
    single(64, 64, &colors, None, &pixels)
}

#[test]
fn row_03_the_cost_is_proportional_to_the_affected_pixels_not_to_the_removed_entries() {
    let affected = 100usize;

    // Same document, same affected pixels (the 100 pixels of entry 255): trimming 1 entry
    // or 200 entries.
    let mut one = corner_document();
    let cost_one = cost_of_the_step(&mut one, resize(cel_id(0), 255, Some(0)));
    let mut many = corner_document();
    let cost_many = cost_of_the_step(&mut many, resize(cel_id(0), 56, Some(0)));
    // 199 more removed entries: at most 4 bytes each plus a small constant. With one mask bit
    // per pixel and per entry, 199 entries of 4096 pixels would add about 100 KB.
    let removed_difference = 255 - 56;
    assert!(
        cost_many.abs_diff(cost_one) <= 4 * removed_difference + 256,
        "trimming 1 entry costs {cost_one} and trimming 200 costs {cost_many}"
    );
    // And it does pay for the pixels it moves: at least one byte per affected pixel.
    assert!(cost_one >= affected, "cost {cost_one}");
    assert!(cost_many >= affected, "cost {cost_many}");
    // Sanity upper bound: one byte per affected pixel, the mask of the cell (one bit per pixel of
    // the 64 x 64 cell: 512 B), 4 B per removed color and a small constant for the step itself.
    let mask = 64 * 64 / 8;
    assert!(
        cost_one <= affected + mask + 4 + 512,
        "history_bytes() went up {cost_one}"
    );
    assert!(
        cost_many <= affected + mask + 4 * 200 + 512,
        "history_bytes() went up {cost_many}"
    );

    // With 0 affected pixels the cost does not depend on the pixels of the cell.
    let colors = colors_different(256);
    let mut small = single(8, 8, &colors, None, &[0u8; 8 * 8]);
    let mut large = single(512, 512, &colors, None, &vec![0u8; 512 * 512]);
    let cost_small = cost_of_the_step(&mut small, resize(cel_id(0), 56, None));
    let cost_large = cost_of_the_step(&mut large, resize(cel_id(0), 56, None));
    assert_eq!(cost_small, cost_large);
    assert!(
        cost_small <= 4 * 200 + 512,
        "history_bytes() went up {cost_small}"
    );

    // Growing stores a number, not colors: it does not depend on the size of the cell or on
    // how many entries are added beyond a small constant.
    let mut grow_small = single(8, 8, &colors_different(8), None, &[0u8; 8 * 8]);
    let mut grow_large = single(512, 512, &colors_different(8), None, &vec![0u8; 512 * 512]);
    let cost_a = cost_of_the_step(&mut grow_small, resize(cel_id(0), 256, None));
    let cost_b = cost_of_the_step(&mut grow_large, resize(cel_id(0), 256, None));
    assert_eq!(cost_a, cost_b);
    assert!(cost_a <= 512, "history_bytes() went up {cost_a}");
}

// Property 1: ResizePalette against a model, with the other palette commands and pixel edits.

/// The model: one `Vec` of colors and, per cell, `None` (empty) or its pixels row by row.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Model {
    width: u32,
    height: u32,
    colors: Vec<Rgba>,
    transparent: Option<u8>,
    cells: Vec<Option<Vec<u8>>>,
}

/// Starting data of a scenario.
#[derive(Clone, Debug)]
struct Setup {
    model: Model,
}

/// Index of a pixel for a palette of `n` entries, biased toward the last one.
fn pixel_index(n: usize) -> impl Strategy<Value = u8> {
    prop_oneof![
        3 => (0..n).prop_map(|v| v as u8),
        1 => Just(u8::try_from(n - 1).unwrap_or(u8::MAX)),
    ]
}

/// Small set of colors to provoke repeated ones and zero alpha.
fn color_p() -> impl Strategy<Value = Rgba> {
    prop::sample::select(vec![
        R,
        G,
        B,
        Y,
        K,
        Rgba::TRANSPARENT,
        Rgba::new(255, 0, 0, 0),
        Rgba::new(0, 255, 0, 128),
    ])
}

/// Palette length: 1 to 16 and, one case in three, 255 or 256.
fn length() -> impl Strategy<Value = usize> {
    prop_oneof![
        2 => 1usize..=16,
        1 => prop::sample::select(vec![255usize, 256]),
    ]
}

/// From 1 to 4 layers (some empty cells, only if there is a transparent index), a 1 x 1 to
/// 3 x 3 canvas, with or without transparent index and 1 to 256 entries.
fn setup() -> impl Strategy<Value = Setup> {
    (1u32..=3, 1u32..=3, length(), 1usize..=4)
        .prop_flat_map(|(w, h, n, layers)| {
            (
                Just((w, h)),
                prop::collection::vec(color_p(), n),
                prop::option::of(0..n),
                prop::collection::vec(
                    prop::option::of(prop::collection::vec(pixel_index(n), (w * h) as usize)),
                    layers,
                ),
            )
        })
        .prop_map(|((width, height), colors, t, cells)| {
            let transparent = t.map(|t| t as u8);
            let cells = cells
                .into_iter()
                .map(|c| match (c, transparent) {
                    // An empty cell needs a transparent index.
                    (None, None) => Some(vec![0u8; (width * height) as usize]),
                    (c, _) => c,
                })
                .collect();
            Setup {
                model: Model {
                    width,
                    height,
                    colors,
                    transparent,
                    cells,
                },
            }
        })
}

/// An order in raw numbers; it is resolved against the current state of the model.
#[derive(Clone, Debug)]
enum Op {
    Resize {
        cell: usize,
        len_pick: u16,
        into_pick: Option<u16>,
    },
    Set {
        cell: usize,
        x: u32,
        y: u32,
        value: u16,
    },
    Add {
        cell: usize,
        color: Rgba,
    },
    Remove {
        cell: usize,
        index_pick: u16,
        into_pick: u16,
    },
    Move {
        cell: usize,
        from_pick: u16,
        to_pick: u16,
    },
}

fn op() -> impl Strategy<Value = Op> {
    let cells_range = 0usize..8;
    prop_oneof![
        5 => (cells_range.clone(), any::<u16>(), prop::option::weighted(0.65, any::<u16>()))
            .prop_map(|(cell, len_pick, into_pick)| Op::Resize { cell, len_pick, into_pick }),
        2 => (cells_range.clone(), 0u32..8, 0u32..8, any::<u16>())
            .prop_map(|(cell, x, y, value)| Op::Set { cell, x, y, value }),
        1 => (cells_range.clone(), color_p()).prop_map(|(cell, color)| Op::Add { cell, color }),
        1 => (cells_range.clone(), any::<u16>(), any::<u16>()).prop_map(
            |(cell, index_pick, into_pick)| Op::Remove { cell, index_pick, into_pick }
        ),
        1 => (cells_range, any::<u16>(), any::<u16>()).prop_map(
            |(cell, from_pick, to_pick)| Op::Move { cell, from_pick, to_pick }
        ),
    ]
}

/// A `u8` index for a palette of `n` entries: almost always valid, sometimes `n` (out of the
/// palette) or 255.
fn index_pick(p: u16, n: usize) -> u8 {
    match p % 8 {
        0 => u8::try_from(n).unwrap_or(u8::MAX),
        1 => u8::MAX,
        _ => u8::try_from(usize::from(p / 8) % n.max(1)).unwrap_or(u8::MAX),
    }
}

/// Turns an `Op` into a `Command` against the current model; `None` skips the order (a pixel
/// edit of an empty cell is not modelled: it would materialize the cell).
fn resolve(op: &Op, m: &Model) -> Option<Command> {
    let n = m.colors.len();
    let layers = m.cells.len();
    let image = |cell: usize| cel_id((cell % layers) as u32);
    match op {
        Op::Resize {
            cell,
            len_pick,
            into_pick,
        } => {
            let len = match len_pick % 10 {
                0 => 0,
                1 => 257,
                2 => n as u16,
                3 => (n + 1) as u16,
                4 => n.saturating_sub(1) as u16,
                5 => 1,
                6 => 256,
                _ => 1 + (len_pick / 10) % 256,
            };
            let target = usize::from(len).clamp(1, 256);
            let into = into_pick.map(|p| {
                if p % 4 == 0 {
                    (p / 4 % 256) as u8
                } else {
                    (usize::from(p / 4) % target) as u8
                }
            });
            Some(resize(image(*cell), len, into))
        }
        Op::Set { cell, x, y, value } => {
            m.cells[cell % layers].as_ref()?;
            Some(set_pixel(
                image(*cell),
                x % m.width,
                y % m.height,
                (usize::from(*value) % n) as u8,
            ))
        }
        Op::Add { cell, color } => Some(Command::AddPaletteEntry {
            image: image(*cell),
            color: *color,
        }),
        Op::Remove {
            cell,
            index_pick: i,
            into_pick: j,
        } => Some(Command::RemovePaletteEntry {
            image: image(*cell),
            index: index_pick(*i, n),
            into: index_pick(*j, n),
        }),
        Op::Move {
            cell,
            from_pick,
            to_pick,
        } => Some(Command::MovePaletteEntry {
            image: image(*cell),
            from: index_pick(*from_pick, n),
            to: index_pick(*to_pick, n),
        }),
    }
}

/// Where the pixel/index `p` ends up when the entry `from` moves to `to`.
fn moved(p: u8, from: u8, to: u8) -> u8 {
    if p == from {
        to
    } else if from < to && p > from && p <= to {
        p - 1
    } else if to < from && p >= to && p < from {
        p + 1
    } else {
        p
    }
}

/// The model of every command of the property. Atomic: nothing changes on an error or
/// `Ok(false)`.
fn model_apply(m: &mut Model, cmd: &Command) -> Result<bool, DocumentError> {
    let n = m.colors.len();
    match cmd {
        Command::ResizePalette { len, into, .. } => {
            if *len == 0 || *len > 256 {
                return Err(bad_length(*len));
            }
            let new_len = usize::from(*len);
            if new_len == n {
                return Ok(false);
            }
            if new_len > n {
                m.colors.resize(new_len, K);
                return Ok(true);
            }
            if let Some(i) = into
                && usize::from(*i) >= new_len
            {
                return Err(outside(*i, new_len));
            }
            let used = m
                .cells
                .iter()
                .flatten()
                .flatten()
                .any(|&p| usize::from(p) >= new_len);
            if used && into.is_none() {
                return Err(editing(PaletteEditError::NoDestination));
            }
            let destination = into.unwrap_or(0);
            for p in m.cells.iter_mut().flatten().flatten() {
                if usize::from(*p) >= new_len {
                    *p = destination;
                }
            }
            m.colors.truncate(new_len);
            if m.transparent.is_some_and(|t| usize::from(t) >= new_len) {
                m.transparent = None;
            }
            Ok(true)
        }
        Command::SetPixels { image, pixels } => {
            let cell = m.cells[image.raw() as usize]
                .as_mut()
                .expect("only cells with an image are edited");
            let mut changed = false;
            for e in pixels {
                let Pixel::Indexed(v) = e.pixel else {
                    unreachable!("the property only writes indexed pixels")
                };
                let slot = &mut cell[(e.y * m.width + e.x) as usize];
                changed |= *slot != v;
                *slot = v;
            }
            Ok(changed)
        }
        Command::AddPaletteEntry { color, .. } => {
            if n >= 256 {
                return Err(editing(PaletteEditError::PaletteFull));
            }
            m.colors.push(*color);
            Ok(true)
        }
        Command::RemovePaletteEntry { index, into, .. } => {
            if usize::from(*index) >= n {
                return Err(outside(*index, n));
            }
            if usize::from(*into) >= n {
                return Err(outside(*into, n));
            }
            if n == 1 {
                return Err(editing(PaletteEditError::LastEntry));
            }
            if index == into {
                return Err(editing(PaletteEditError::RemoveIntoSelf { index: *index }));
            }
            // Indexes before removing: the pixels of `index` go to `into`, then the entries
            // above `index` shift down by one.
            let after = |p: u8| if p > *index { p - 1 } else { p };
            let destination = after(*into);
            for p in m.cells.iter_mut().flatten().flatten() {
                *p = if *p == *index { destination } else { after(*p) };
            }
            m.transparent = match m.transparent {
                Some(t) if t == *index => None,
                Some(t) => Some(after(t)),
                None => None,
            };
            m.colors.remove(usize::from(*index));
            Ok(true)
        }
        Command::MovePaletteEntry { from, to, .. } => {
            if usize::from(*from) >= n {
                return Err(outside(*from, n));
            }
            if usize::from(*to) >= n {
                return Err(outside(*to, n));
            }
            if from == to {
                return Ok(false);
            }
            let color = m.colors.remove(usize::from(*from));
            m.colors.insert(usize::from(*to), color);
            for p in m.cells.iter_mut().flatten().flatten() {
                *p = moved(*p, *from, *to);
            }
            m.transparent = m.transparent.map(|t| moved(t, *from, *to));
            Ok(true)
        }
        other => unreachable!("not a command of the property: {other:?}"),
    }
}

/// The document of a scenario. Odd layers are hidden (visibility does not matter for palette
/// commands, and none is locked so that pixel edits are accepted).
fn build(m: &Model) -> Document {
    let layers = m
        .cells
        .iter()
        .enumerate()
        .map(|(i, c)| {
            layer(
                &format!("L{i}"),
                i % 2 == 0,
                false,
                c.as_ref()
                    .map(|p| cell(m.width, m.height, &m.colors, m.transparent, p)),
            )
        })
        .collect();
    document(m.width, m.height, &m.colors, m.transparent, layers)
}

/// The document matches the model and the invariants hold: all the palettes identical, every
/// pixel inside the palette and a valid or absent transparent index.
fn matches_model(doc: &Document, m: &Model) -> Result<(), TestCaseError> {
    let palette = doc.palette().expect("indexed");
    prop_assert_eq!(palette.colors(), &m.colors[..]);
    prop_assert_eq!(palette.transparent_index(), m.transparent);
    prop_assert!(
        m.transparent
            .is_none_or(|t| usize::from(t) < m.colors.len())
    );
    prop_assert_eq!(doc.layers().len(), m.cells.len());
    for (i, (l, expected)) in doc.layers().iter().zip(&m.cells).enumerate() {
        prop_assert_eq!(l.id(), LayerId::from_raw(i as u32));
        prop_assert_eq!(l.cel(), cel_id(i as u32));
        match (doc.image(l.cel()), expected) {
            (None, None) => {}
            (Some(img), Some(pixels)) => {
                prop_assert_eq!(&pixels_of_image(img), pixels, "cell {}", i);
                prop_assert_eq!(img.palette(), doc.palette(), "palette of cell {}", i);
                prop_assert!(pixels.iter().all(|&v| usize::from(v) < m.colors.len()));
            }
            (img, expected) => prop_assert!(
                false,
                "cell {}: image {:?}, model {:?}",
                i,
                img.map(|_| "with image"),
                expected
            ),
        }
    }
    let template = doc.canvas_image();
    prop_assert_eq!(template.palette(), doc.palette());
    match template.get_pixel(0, 0) {
        Ok(Pixel::Indexed(v)) => prop_assert!(usize::from(v) < m.colors.len()),
        other => prop_assert!(false, "template pixel {:?}", other),
    }
    Ok(())
}

/// Document, explicit print and `state_id` at one point of the history.
type Moment = (Document, Print, u64);

fn moment(doc: &Document) -> Moment {
    (doc.clone(), print_of(doc), doc.state_id())
}

fn is_moment(doc: &Document, m: &Moment) -> Result<(), TestCaseError> {
    prop_assert_eq!(doc, &m.0);
    prop_assert_eq!(&print_of(doc), &m.1);
    prop_assert_eq!(doc.state_id(), m.2);
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    // Property 1: `ResizePalette` (mixed with pixel edits and the other palette commands)
    // against the model; errors and `Ok(false)` leave document, history and `state_id`
    // equal; `undo` to the end gives the initial document IDENTICAL (ids, palette,
    // transparent index, pixels and empty cells) and `redo` to the end the final one.
    #[test]
    fn property_01_resize_against_a_model_and_undo_redo_are_identical(
        scenario in setup(),
        ops in prop::collection::vec(op(), 0..=12),
    ) {
        let mut model = scenario.model.clone();
        let mut doc = build(&model);
        matches_model(&doc, &model)?;
        let mut moments: Vec<Moment> = vec![moment(&doc)];

        for op in &ops {
            let Some(cmd) = resolve(op, &model) else { continue };
            let before = state_light(&doc);
            let expected = model_apply(&mut model, &cmd);
            let got = doc.apply(cmd.clone());
            prop_assert_eq!(&got, &expected, "{:?}", cmd);
            match got {
                Ok(true) => {
                    prop_assert_ne!(doc.state_id(), before.state_id, "{:?}", cmd);
                    prop_assert!(doc.can_undo());
                    prop_assert!(!doc.can_redo());
                    moments.push(moment(&doc));
                }
                _ => {
                    prop_assert_eq!(state_light(&doc), before, "{:?}", cmd);
                }
            }
            matches_model(&doc, &model)?;
        }

        // Undo to the end: every state is identical to the one it had.
        let steps = moments.len() - 1;
        for k in (0..steps).rev() {
            prop_assert_eq!(doc.undo(), Ok(()));
            is_moment(&doc, &moments[k])?;
        }
        prop_assert!(!doc.can_undo());
        prop_assert_eq!(doc.undo(), Err(DocumentError::NothingToUndo));
        // Redo to the end.
        for moment in moments.iter().take(steps + 1).skip(1) {
            prop_assert_eq!(doc.redo(), Ok(()));
            is_moment(&doc, moment)?;
        }
        prop_assert!(!doc.can_redo());
        prop_assert_eq!(doc.redo(), Err(DocumentError::NothingToRedo));
        matches_model(&doc, &model)?;
    }
}

/// A light snapshot for the property (no step count: the property counts the steps itself).
#[derive(Debug, PartialEq, Eq)]
struct Light {
    doc: Document,
    print: Print,
    can_undo: bool,
    can_redo: bool,
    bytes: usize,
    state_id: u64,
}

fn state_light(doc: &Document) -> Light {
    Light {
        doc: doc.clone(),
        print: print_of(doc),
        can_undo: doc.can_undo(),
        can_redo: doc.can_redo(),
        bytes: doc.history_bytes(),
        state_id: doc.state_id(),
    }
}
