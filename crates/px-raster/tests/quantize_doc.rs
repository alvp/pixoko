//! Tests of `quantize_command` over documents (specs/hito-4-cuantizador.md, unit 4.2): the layer
//! rules of row 7 and property 4 of «Propiedades».
//!
//! Red phase: `quantize_command` and `Command::QuantizeAll` do not exist yet, so this file does not
//! compile until the logic of `px-core` and `px-raster`.

use proptest::prelude::*;
use px_core::{BlendMode, Document, DocumentError, Image, LayerSpec, Mode, Pixel, Rgba};
use px_raster::{Dither, PaletteSource, QuantizeError, QuantizeOptions, quantize_command};

fn options(colors: u16, dither: Dither) -> QuantizeOptions {
    QuantizeOptions {
        colors,
        dither,
        alpha_threshold: 128,
        palette: PaletteSource::Computed,
    }
}

fn spec(image: Option<Image>, opacity: u8, blend: BlendMode, visible: bool, locked: bool) -> LayerSpec {
    LayerSpec {
        name: "layer".to_string(),
        visible,
        opacity,
        blend,
        locked,
        image,
    }
}

fn picture(w: u32, h: u32, seed: u8) -> Image {
    let mut image = Image::new(w, h, Mode::Rgba).expect("image");
    for y in 0..h {
        for x in 0..w {
            let v = (x * 37 + y * 11) as u8;
            let a = if (x + y) % 5 == 0 { 0 } else { 255 };
            image
                .set_pixel(x, y, Pixel::Rgba(Rgba::new(v, v.wrapping_mul(3).wrapping_add(seed), seed, a)))
                .expect("pixel");
        }
    }
    image
}

#[test]
fn row_07_layers_with_opacity_or_blend_cannot_become_indexed() {
    let translucent = Document::from_layers(
        4,
        4,
        Mode::Rgba,
        None,
        vec![spec(Some(picture(4, 4, 1)), 128, BlendMode::Normal, true, false)],
    )
    .expect("document");
    assert_eq!(
        quantize_command(&translucent, &options(8, Dither::None)).unwrap_err(),
        QuantizeError::LayerOpacity
    );
    let blended = Document::from_layers(
        4,
        4,
        Mode::Rgba,
        None,
        vec![spec(Some(picture(4, 4, 1)), 255, BlendMode::Multiply, true, false)],
    )
    .expect("document");
    assert_eq!(
        quantize_command(&blended, &options(8, Dither::None)).unwrap_err(),
        QuantizeError::LayerBlend
    );
    let indexed = Document::new(
        Image::filled_indexed(
            2,
            2,
            px_core::Palette::from_colors([Rgba::new(1, 2, 3, 255)]).unwrap(),
            0,
        )
        .unwrap(),
    );
    assert_eq!(
        quantize_command(&indexed, &options(8, Dither::None)).unwrap_err(),
        QuantizeError::SourceMode(Mode::Indexed)
    );
}

#[test]
fn row_07_the_command_quantizes_every_cell_including_hidden_and_locked_ones() {
    let mut doc = Document::from_layers(
        6,
        5,
        Mode::Rgba,
        None,
        vec![
            spec(Some(picture(6, 5, 1)), 255, BlendMode::Normal, true, false),
            spec(None, 255, BlendMode::Normal, true, false),
            spec(Some(picture(6, 5, 90)), 255, BlendMode::Normal, false, true),
        ],
    )
    .expect("document");
    let (command, stats) = quantize_command(&doc, &options(8, Dither::FloydSteinberg)).expect("command");
    assert!(stats.colors <= 8);
    assert_eq!(doc.apply(command), Ok(true));
    assert_eq!(doc.mode(), Mode::Indexed);
    let layers = doc.layers();
    assert!(doc.image(layers[0].cel()).is_some());
    assert!(doc.image(layers[1].cel()).is_none(), "an empty cell stays empty");
    assert!(doc.image(layers[2].cel()).is_some());
    assert!(!layers[2].visible() && layers[2].locked());
    assert!(doc.palette().expect("palette").transparent_index().is_some());
}

// ---------------------------------------------------------------------------------------------
// Property 4
// ---------------------------------------------------------------------------------------------

fn cell_strategy() -> impl Strategy<Value = Option<Image>> {
    (any::<bool>(), 0u8..=255, any::<bool>()).prop_map(|(present, seed, few)| {
        present.then(|| {
            let mut image = Image::new(5, 4, Mode::Rgba).expect("image");
            for y in 0..4u32 {
                for x in 0..5u32 {
                    let k = ((x + y * 5) as u8).wrapping_add(seed);
                    let (r, g, b) = if few {
                        (k % 3 * 100, 20, 40)
                    } else {
                        (k, k.wrapping_mul(5), k.wrapping_mul(11))
                    };
                    let a = if k % 7 == 0 { 0 } else { 255 };
                    image.set_pixel(x, y, Pixel::Rgba(Rgba::new(r, g, b, a))).unwrap();
                }
            }
            image
        })
    })
}

fn document_strategy() -> impl Strategy<Value = Document> {
    proptest::collection::vec((cell_strategy(), any::<bool>(), any::<bool>()), 1..=3).prop_map(|cells| {
        let layers = cells
            .into_iter()
            .map(|(image, visible, locked)| spec(image, 255, BlendMode::Normal, visible, locked))
            .collect();
        Document::from_layers(5, 4, Mode::Rgba, None, layers).expect("document")
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    // Property 4: a quantization is one step, reversible, and a failure changes nothing.
    #[test]
    fn property_4_quantizing_a_document_is_one_exact_reversible_step(
        mut doc in document_strategy(),
        n in 2u16..=64,
        floyd in any::<bool>(),
    ) {
        let initial = doc.clone();
        let dither = if floyd { Dither::FloydSteinberg } else { Dither::None };
        let (command, _) = quantize_command(&doc, &options(n, dither)).unwrap();
        let again = command.clone();
        let applied = doc.apply(command);
        prop_assert_eq!(applied, Ok(true));
        prop_assert_ne!(doc.state_id(), initial.state_id());
        prop_assert_eq!(doc.mode(), Mode::Indexed);
        let after = doc.clone();
        // A second application is stale: it fails and leaves everything identical.
        let before_failure = (doc.clone(), doc.state_id(), doc.history_bytes());
        let error = doc.apply(again).unwrap_err();
        prop_assert!(matches!(error, DocumentError::StaleImage { .. }), "{:?}", error);
        prop_assert_eq!((doc.clone(), doc.state_id(), doc.history_bytes()), before_failure);
        // One step back to the initial document, including its id.
        prop_assert_eq!(doc.undo(), Ok(()));
        prop_assert_eq!(&doc, &initial);
        prop_assert_eq!(doc.state_id(), initial.state_id());
        prop_assert!(!doc.can_undo());
        prop_assert_eq!(doc.redo(), Ok(()));
        prop_assert_eq!(&doc, &after);
    }
}
