//! Tests of `Command::ReplaceImages` (specs/hito-4-imagen.md, unit 4.1, changes C-01 to C-05):
//! rows 5, 6 and 8 of «Casos». Rows 1 to 4 and 7 and the four properties live in
//! `crates/px-raster/tests/image_ops.rs` (they need the pure functions of `px-raster`).
//!
//! Red phase: `Command::ReplaceImages` and the three new `DocumentError` variants do not exist
//! yet, so this file does not compile until the logic of `px-core`.

use px_core::{
    BlendMode, Command, Document, DocumentError, Image, LayerSpec, Mode, Palette, Pixel, Rgba,
};

fn layer(
    name: &str,
    visible: bool,
    locked: bool,
    opacity: u8,
    blend: BlendMode,
    image: Option<Image>,
) -> LayerSpec {
    LayerSpec {
        name: name.to_string(),
        visible,
        opacity,
        blend,
        locked,
        image,
    }
}

fn filled(w: u32, h: u32, value: u8) -> Image {
    Image::filled(w, h, Pixel::Rgba(Rgba::new(value, value, value, 255))).expect("image")
}

/// Three layers of 3 × 2: a plain one, an empty one and a hidden, locked, translucent one.
fn document() -> Document {
    let layers = vec![
        layer(
            "base",
            true,
            false,
            255,
            BlendMode::Normal,
            Some(filled(3, 2, 10)),
        ),
        layer("empty", true, false, 255, BlendMode::Normal, None),
        layer(
            "top",
            false,
            true,
            128,
            BlendMode::Multiply,
            Some(filled(3, 2, 20)),
        ),
    ];
    Document::from_layers(3, 2, Mode::Rgba, None, layers).expect("document")
}

fn replace(doc: &Document, w: u32, h: u32, images: Vec<Option<Image>>) -> Command {
    Command::ReplaceImages {
        width: w,
        height: h,
        images,
        based_on: doc.state_id(),
    }
}

/// Everything an observer can see of the document, history included.
fn view(doc: &Document) -> (Document, u64, usize, (u32, u32)) {
    (
        doc.clone(),
        doc.state_id(),
        doc.history_bytes(),
        (doc.width(), doc.height()),
    )
}

#[test]
fn row_5_replace_images_is_one_step_and_undo_and_redo_are_exact() {
    let mut doc = document();
    let before = view(&doc);
    let images = vec![Some(filled(2, 4, 1)), None, Some(filled(2, 4, 2))];
    let command = replace(&doc, 2, 4, images);
    assert_eq!(doc.apply(command), Ok(true));
    assert_eq!((doc.width(), doc.height()), (2, 4));
    assert_ne!(doc.state_id(), before.1);
    let layers = doc.layers();
    assert_eq!(layers.len(), 3);
    assert_eq!(doc.image(layers[0].cel()), Some(&filled(2, 4, 1)));
    assert_eq!(
        doc.image(layers[1].cel()),
        None,
        "an empty cell stays empty"
    );
    assert_eq!(doc.image(layers[2].cel()), Some(&filled(2, 4, 2)));
    for (new, old) in layers.iter().zip(before.0.layers()) {
        assert_eq!(new.id(), old.id());
        assert_eq!(new.name(), old.name());
        assert_eq!(new.visible(), old.visible());
        assert_eq!(new.locked(), old.locked());
        assert_eq!(new.opacity(), old.opacity());
        assert_eq!(new.blend(), old.blend());
    }
    let after = view(&doc);
    // One step: a single undo brings everything back, including the canvas size and the id.
    assert_eq!(doc.undo(), Ok(()));
    assert_eq!(view(&doc), before);
    assert!(!doc.can_undo(), "it was one step");
    assert_eq!(doc.redo(), Ok(()));
    assert_eq!(view(&doc), after);
    // An operation that changes nothing (same size, same images) is not a step.
    let same = vec![Some(filled(2, 4, 1)), None, Some(filled(2, 4, 2))];
    let command = replace(&doc, 2, 4, same);
    assert_eq!(doc.apply(command), Ok(false));
    assert_eq!(view(&doc), after);
}

#[test]
fn row_6_replace_images_rejects_and_leaves_the_document_untouched() {
    let mut doc = document();
    let good = || vec![Some(filled(3, 2, 1)), None, Some(filled(3, 2, 2))];
    let mut indexed_palette =
        Palette::from_colors([Rgba::new(0, 0, 0, 255), Rgba::new(9, 9, 9, 255)]).expect("palette");
    indexed_palette
        .set_transparent_index(Some(0))
        .expect("transparent");
    let other_palette =
        Palette::from_colors([Rgba::new(1, 1, 1, 255), Rgba::new(9, 9, 9, 255)]).expect("palette");
    type Case = (&'static str, Command, fn(&DocumentError) -> bool);
    let cases: Vec<Case> = vec![
        (
            "stale",
            Command::ReplaceImages {
                width: 3,
                height: 2,
                images: good(),
                based_on: doc.state_id() + 7,
            },
            |e| matches!(e, DocumentError::StaleImage { .. }),
        ),
        (
            "too few",
            replace(&doc, 3, 2, vec![Some(filled(3, 2, 1)), None]),
            |e| {
                matches!(
                    e,
                    DocumentError::ReplaceCountMismatch {
                        expected: 3,
                        found: 2
                    }
                )
            },
        ),
        (
            "size",
            replace(
                &doc,
                3,
                2,
                vec![Some(filled(2, 2, 1)), None, Some(filled(3, 2, 2))],
            ),
            |e| matches!(e, DocumentError::ReplaceImageMismatch),
        ),
        (
            "mode",
            replace(
                &doc,
                3,
                2,
                vec![
                    Some(Image::filled(3, 2, Pixel::Gray(1)).expect("gray")),
                    None,
                    Some(filled(3, 2, 2)),
                ],
            ),
            |e| matches!(e, DocumentError::ReplaceImageMismatch),
        ),
        (
            "palette",
            replace(
                &doc,
                3,
                2,
                vec![
                    Some(Image::filled_indexed(3, 2, indexed_palette.clone(), 1).expect("indexed")),
                    None,
                    Some(filled(3, 2, 2)),
                ],
            ),
            |e| matches!(e, DocumentError::ReplaceImageMismatch),
        ),
        (
            "an empty cell gets an image",
            replace(
                &doc,
                3,
                2,
                vec![
                    Some(filled(3, 2, 1)),
                    Some(filled(3, 2, 5)),
                    Some(filled(3, 2, 2)),
                ],
            ),
            |e| matches!(e, DocumentError::ReplaceEmptiness),
        ),
        (
            "a cell with image is emptied",
            replace(&doc, 3, 2, vec![None, None, Some(filled(3, 2, 2))]),
            |e| matches!(e, DocumentError::ReplaceEmptiness),
        ),
        ("zero width", replace(&doc, 0, 2, good()), |e| {
            matches!(e, DocumentError::ReplaceImageMismatch)
        }),
        (
            "inside a batch",
            Command::Batch(vec![replace(&doc, 3, 2, good())]),
            |e| format!("{e:?}").contains("NotBatchable"),
        ),
    ];
    for (name, command, expected) in cases {
        let before = view(&doc);
        let error = doc.apply(command).expect_err(name);
        assert!(expected(&error), "{name}: {error:?}");
        assert_eq!(view(&doc), before, "{name} changed the document");
    }
    // The palette of an indexed canvas must be the same one.
    let image = Image::filled_indexed(2, 2, indexed_palette, 1).expect("indexed");
    let mut indexed = Document::new(image);
    let before = view(&indexed);
    let wrong = Image::filled_indexed(2, 2, other_palette, 1).expect("indexed");
    let command = replace(&indexed, 2, 2, vec![Some(wrong)]);
    assert_eq!(
        indexed.apply(command),
        Err(DocumentError::ReplaceImageMismatch)
    );
    assert_eq!(view(&indexed), before);
}

#[test]
fn row_8_the_cells_limit_applies_to_the_result_and_only_if_it_grows() {
    // At the limit: replacing with images of the same size is allowed.
    let mut doc = document();
    doc.set_cells_limit(doc.materialized_bytes());
    let command = replace(
        &doc,
        3,
        2,
        vec![Some(filled(3, 2, 1)), None, Some(filled(3, 2, 2))],
    );
    assert_eq!(doc.apply(command), Ok(true));
    // A turn keeps the number of bytes: allowed at the limit too.
    let command = replace(
        &doc,
        2,
        3,
        vec![Some(filled(2, 3, 1)), None, Some(filled(2, 3, 2))],
    );
    assert_eq!(doc.apply(command), Ok(true));
    // Growing above the limit is `CelsTooLarge`, before touching anything.
    let before = view(&doc);
    let command = replace(
        &doc,
        4,
        3,
        vec![Some(filled(4, 3, 1)), None, Some(filled(4, 3, 2))],
    );
    assert!(matches!(
        doc.apply(command),
        Err(DocumentError::CelsTooLarge { .. })
    ));
    assert_eq!(view(&doc), before);
    // Shrinking is always fine, and the bytes follow the cells.
    let command = replace(
        &doc,
        1,
        1,
        vec![Some(filled(1, 1, 1)), None, Some(filled(1, 1, 2))],
    );
    assert_eq!(doc.apply(command), Ok(true));
    assert_eq!(doc.materialized_bytes(), 2 * 4);
    // A step bigger than the history limit is kept (the last step is never dropped) and undone.
    let mut big = document();
    big.set_history_limit(1);
    let command = replace(
        &big,
        6,
        4,
        vec![Some(filled(6, 4, 1)), None, Some(filled(6, 4, 2))],
    );
    assert_eq!(big.apply(command), Ok(true));
    assert!(big.can_undo());
    assert!(
        big.history_bytes() >= 2 * 3 * 2 * 4,
        "the old cells are counted"
    );
    assert_eq!(big.undo(), Ok(()));
    assert_eq!((big.width(), big.height()), (3, 2));
    assert_eq!(big.image(big.layers()[0].cel()), Some(&filled(3, 2, 10)));
}
