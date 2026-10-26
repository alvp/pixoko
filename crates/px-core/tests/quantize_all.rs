//! Tests of `Command::QuantizeAll` (specs/hito-4-cuantizador.md, unit 4.2, changes Q-01 to Q-04):
//! rows 8 and 9 of «Casos». The quantizer itself lives in `px-raster`.
//!
//! Red phase: `Command::QuantizeAll` and `DocumentError::QuantizeSource` do not exist yet, so this
//! file does not compile until the logic of `px-core`.

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

fn filled(value: u8) -> Image {
    Image::filled(3, 2, Pixel::Rgba(Rgba::new(value, value, 0, 255))).expect("image")
}

/// Three layers of 3 × 2 RGBA: a plain one, an empty one and a hidden, locked one.
fn document_with(opacity: u8, blend: BlendMode) -> Document {
    let layers = vec![
        layer("base", true, false, 255, BlendMode::Normal, Some(filled(10))),
        layer("empty", true, false, 255, BlendMode::Normal, None),
        layer("top", false, true, opacity, blend, Some(filled(20))),
    ];
    Document::from_layers(3, 2, Mode::Rgba, None, layers).expect("document")
}

fn document() -> Document {
    document_with(255, BlendMode::Normal)
}

/// A palette with a transparent entry at 0 and two colors.
fn palette() -> Palette {
    let mut palette = Palette::from_colors([
        Rgba::new(0, 0, 0, 0),
        Rgba::new(10, 10, 0, 255),
        Rgba::new(20, 20, 0, 255),
    ])
    .expect("palette");
    palette.set_transparent_index(Some(0)).expect("transparent");
    palette
}

fn indexed(palette: &Palette, w: u32, h: u32, index: u8) -> Image {
    Image::filled_indexed(w, h, palette.clone(), index).expect("indexed image")
}

fn quantize(doc: &Document, images: Vec<Option<Image>>, palette: Palette) -> Command {
    Command::QuantizeAll {
        palette,
        images,
        based_on: doc.state_id(),
    }
}

fn good(palette: &Palette) -> Vec<Option<Image>> {
    vec![
        Some(indexed(palette, 3, 2, 1)),
        None,
        Some(indexed(palette, 3, 2, (palette.len() - 1).min(2) as u8)),
    ]
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
fn row_8_quantize_all_is_one_step_and_undo_and_redo_are_exact() {
    let mut doc = document();
    let before = view(&doc);
    let palette = palette();
    let command = quantize(&doc, good(&palette), palette.clone());
    assert_eq!(doc.apply(command), Ok(true));
    assert_eq!(doc.mode(), Mode::Indexed);
    assert_eq!(doc.palette(), Some(&palette));
    assert_ne!(doc.state_id(), before.1);
    let layers = doc.layers();
    assert_eq!(layers.len(), 3);
    assert_eq!(doc.image(layers[0].cel()), Some(&indexed(&palette, 3, 2, 1)));
    assert_eq!(doc.image(layers[1].cel()), None, "an empty cell stays empty");
    assert_eq!(doc.image(layers[2].cel()), Some(&indexed(&palette, 3, 2, 2)));
    for (new, old) in layers.iter().zip(before.0.layers()) {
        assert_eq!(new.id(), old.id());
        assert_eq!(new.name(), old.name());
        assert_eq!(new.visible(), old.visible());
        assert_eq!(new.locked(), old.locked());
        assert_eq!(new.opacity(), old.opacity());
        assert_eq!(new.blend(), old.blend());
    }
    let after = view(&doc);
    // One step: a single undo brings back the mode, the palette, the cells and the id.
    assert_eq!(doc.undo(), Ok(()));
    assert_eq!(view(&doc), before);
    assert_eq!(doc.mode(), Mode::Rgba);
    assert!(!doc.can_undo(), "it was one step");
    assert_eq!(doc.redo(), Ok(()));
    assert_eq!(view(&doc), after);
    assert_eq!(doc.mode(), Mode::Indexed);
}

#[test]
fn row_8_a_gray_alpha_document_can_be_quantized_too() {
    let image = Image::filled(2, 2, Pixel::GrayAlpha { gray: 9, alpha: 255 }).expect("image");
    let mut doc = Document::new(image);
    let mut palette = Palette::from_colors([Rgba::new(9, 9, 9, 255)]).expect("palette");
    palette.set_transparent_index(None).expect("none");
    let command = quantize(&doc, vec![Some(indexed(&palette, 2, 2, 0))], palette.clone());
    assert_eq!(doc.apply(command), Ok(true));
    assert_eq!(doc.mode(), Mode::Indexed);
    assert_eq!(doc.undo(), Ok(()));
    assert_eq!(doc.mode(), Mode::GrayAlpha);
}

#[test]
fn row_9_quantize_all_rejects_what_does_not_fit_and_leaves_the_document_identical() {
    let palette = palette();
    let doc = document();
    let other_palette = {
        let mut p = palette.clone();
        p.set(1, Rgba::new(99, 99, 99, 255)).expect("set");
        p
    };
    let no_transparent = Palette::from_colors([
        Rgba::new(10, 10, 0, 255),
        Rgba::new(20, 20, 0, 255),
    ])
    .expect("palette");
    let stale = Command::QuantizeAll {
        palette: palette.clone(),
        images: good(&palette),
        based_on: doc.state_id().wrapping_sub(1),
    };
    let cases: Vec<(&str, Document, Command, &str)> = vec![
        ("stale", doc.clone(), stale, "StaleImage"),
        (
            "count",
            doc.clone(),
            quantize(&doc, vec![Some(indexed(&palette, 3, 2, 1))], palette.clone()),
            "ReplaceCountMismatch",
        ),
        (
            "not indexed",
            doc.clone(),
            quantize(
                &doc,
                vec![Some(filled(1)), None, Some(indexed(&palette, 3, 2, 2))],
                palette.clone(),
            ),
            "ReplaceImageMismatch",
        ),
        (
            "other size",
            doc.clone(),
            quantize(
                &doc,
                vec![
                    Some(indexed(&palette, 4, 2, 1)),
                    None,
                    Some(indexed(&palette, 3, 2, 2)),
                ],
                palette.clone(),
            ),
            "ReplaceImageMismatch",
        ),
        (
            "other palette",
            doc.clone(),
            quantize(&doc, good(&other_palette), palette.clone()),
            "ReplaceImageMismatch",
        ),
        (
            "empty cell gets an image",
            doc.clone(),
            quantize(
                &doc,
                vec![
                    Some(indexed(&palette, 3, 2, 1)),
                    Some(indexed(&palette, 3, 2, 1)),
                    Some(indexed(&palette, 3, 2, 2)),
                ],
                palette.clone(),
            ),
            "ReplaceEmptiness",
        ),
        (
            "cell with image becomes empty",
            doc.clone(),
            quantize(
                &doc,
                vec![None, None, Some(indexed(&palette, 3, 2, 2))],
                palette.clone(),
            ),
            "ReplaceEmptiness",
        ),
        (
            "translucent layer",
            document_with(128, BlendMode::Normal),
            quantize(&doc, good(&palette), palette.clone()),
            "IndexedOpacity",
        ),
        (
            "blend mode",
            document_with(255, BlendMode::Multiply),
            quantize(&doc, good(&palette), palette.clone()),
            "IndexedBlend",
        ),
        (
            "several layers without a transparent entry",
            doc.clone(),
            quantize(&doc, good(&no_transparent), no_transparent.clone()),
            "NoTransparency",
        ),
        (
            "inside a batch",
            doc.clone(),
            Command::Batch(vec![quantize(&doc, good(&palette), palette.clone())]),
            "NotBatchable",
        ),
    ];
    for (name, mut target, command, expected) in cases {
        // Some cases use another document than the one the command was built on: rebase it, so
        // that only the property under test is wrong (the stale case keeps its old id).
        let command = match command {
            Command::QuantizeAll {
                palette, images, ..
            } if name != "stale" => Command::QuantizeAll {
                palette,
                images,
                based_on: target.state_id(),
            },
            other => other,
        };
        let before = view(&target);
        let error = target.apply(command).expect_err(name);
        assert!(
            format!("{error:?}").contains(expected),
            "{name}: got {error:?}, wanted {expected}"
        );
        assert_eq!(view(&target), before, "{name}: the document must not change");
    }
}

#[test]
fn row_9_an_indexed_or_gray_document_cannot_be_quantized() {
    let palette = palette();
    let image = indexed(&palette, 3, 2, 1);
    let mut doc = Document::new(image.clone());
    let before = view(&doc);
    let error = doc
        .apply(quantize(&doc, vec![Some(image)], palette.clone()))
        .expect_err("indexed source");
    assert_eq!(error, DocumentError::QuantizeSource { mode: Mode::Indexed });
    assert_eq!(view(&doc), before);

    let gray = Image::filled(3, 2, Pixel::Gray(5)).expect("gray");
    let mut doc = Document::new(gray);
    let before = view(&doc);
    let error = doc
        .apply(quantize(&doc, vec![Some(indexed(&palette, 3, 2, 1))], palette.clone()))
        .expect_err("gray source");
    assert_eq!(error, DocumentError::QuantizeSource { mode: Mode::Gray });
    assert_eq!(view(&doc), before);
}

#[test]
fn row_9_the_cells_limit_applies_only_when_the_result_grows() {
    // An indexed result takes a quarter of the RGBA bytes, so even with the limit at the current
    // size the command passes (it does not grow); with a limit of zero it still passes for the
    // same reason, and the step stays undoable.
    let palette = palette();
    let mut doc = document();
    doc.set_cells_limit(doc.materialized_bytes());
    let command = quantize(&doc, good(&palette), palette.clone());
    assert_eq!(doc.apply(command), Ok(true));
    assert_eq!(doc.undo(), Ok(()));
    assert_eq!(doc.mode(), Mode::Rgba);
}
