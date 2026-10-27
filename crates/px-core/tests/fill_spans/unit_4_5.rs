//! Unit 4.5 (specs/hito-4-relleno.md), row 8 and property 3: `FillSpans` applied, undone and
//! redone by rows must do exactly what the per-pixel version did.
//!
//! The model writes pixel by pixel with `Image::set_pixel` and splits the runs by hand.

use proptest::prelude::*;
use px_core::{
    Command, Document, DocumentError, Image, ImageError, ImageId, Mode, Palette, Pixel, Rgba, Span,
};

const MODES: [Mode; 4] = [Mode::Rgba, Mode::Indexed, Mode::Gray, Mode::GrayAlpha];

fn palette() -> Palette {
    Palette::from_colors((0..8u8).map(|n| Rgba::opaque(n * 30, 255 - n * 30, n * 7))).unwrap()
}

fn value(mode: Mode, n: u8) -> Pixel {
    match mode {
        Mode::Rgba => Pixel::Rgba(Rgba::new(n * 30, 255 - n * 30, n * 7, 255)),
        Mode::Indexed => Pixel::Indexed(n),
        Mode::Gray => Pixel::Gray(n * 30),
        Mode::GrayAlpha => Pixel::GrayAlpha {
            gray: n * 30,
            alpha: 255 - n,
        },
    }
}

fn image_with(mode: Mode, width: u32, height: u32, mut f: impl FnMut(u32, u32) -> u8) -> Image {
    let mut image = match mode {
        Mode::Indexed => Image::new_indexed(width, height, palette()).unwrap(),
        other => Image::new(width, height, other).unwrap(),
    };
    for y in 0..height {
        for x in 0..width {
            image.set_pixel(x, y, value(mode, f(x, y))).unwrap();
        }
    }
    image
}

fn sp(y: u32, x_start: u32, x_end: u32) -> Span {
    Span { y, x_start, x_end }
}

/// The model: the image after the fill and the number of runs (maximal series of equal previous
/// values, different from `pixel`, inside each span).
fn model(image: &Image, spans: &[Span], pixel: Pixel) -> (Image, usize) {
    let mut out = image.clone();
    let mut runs = 0;
    for span in spans {
        let mut previous: Option<Pixel> = None;
        for x in span.x_start..span.x_end {
            let old = image.get_pixel(x, span.y).unwrap();
            if old != pixel {
                if previous != Some(old) {
                    runs += 1;
                }
                out.set_pixel(x, span.y, pixel).unwrap();
                previous = Some(old);
            } else {
                previous = None;
            }
        }
    }
    (out, runs)
}

/// Applies, undoes and redoes `spans`, comparing with the model. Returns the bytes of history
/// that the step added and the number of runs of the model.
fn check(image: &Image, spans: Vec<Span>, pixel: Pixel) -> (usize, usize) {
    let (expected, runs) = model(image, &spans, pixel);
    let mut doc = Document::new(image.clone());
    let id = ImageId::from_raw(0);
    let base = doc.history_bytes();
    let applied = doc
        .apply(Command::FillSpans {
            image: id,
            spans,
            pixel,
        })
        .unwrap();
    assert_eq!(applied, runs > 0);
    assert_eq!(doc.image(id).unwrap().bytes(), expected.bytes());
    if applied {
        let added = doc.history_bytes() - base;
        doc.undo().unwrap();
        assert_eq!(doc.image(id).unwrap().bytes(), image.bytes());
        doc.redo().unwrap();
        assert_eq!(doc.image(id).unwrap().bytes(), expected.bytes());
        (added, runs)
    } else {
        (0, 0)
    }
}

#[test]
fn row_08_apply_undo_and_redo_by_rows_match_the_per_pixel_model() {
    let spans = vec![
        sp(0, 0, 9),
        sp(1, 0, 1),
        sp(1, 8, 9),
        sp(2, 3, 4),
        sp(3, 2, 7),
    ];
    for mode in MODES {
        let image = image_with(mode, 9, 4, |x, y| ((x + y) % 3) as u8);
        let (added, runs) = check(&image, spans.clone(), value(mode, 7));
        assert_eq!(
            added, 412,
            "{mode:?}: history bytes of the step (pinned before the rewrite)"
        );
        assert_eq!(
            runs,
            9 + 1 + 1 + 1 + 5,
            "{mode:?}: pieces of one value each"
        );

        // Runs of one value: whole-row and left/right-edge spans over a two-valued image.
        let image = image_with(mode, 9, 4, |x, _| u8::from(x >= 4));
        let (_, runs) = check(&image, spans.clone(), value(mode, 7));
        assert_eq!(runs, 2 + 1 + 1 + 1 + 2, "{mode:?}");

        // Pixels that already have the value are not pieces.
        let image = image_with(mode, 9, 4, |x, _| if x % 2 == 0 { 7 } else { 1 });
        let (_, runs) = check(&image, vec![sp(0, 0, 9)], value(mode, 7));
        assert_eq!(runs, 4, "{mode:?}");
        let (added, runs) = check(&image, vec![sp(0, 0, 1)], value(mode, 7));
        assert_eq!((added, runs), (0, 0), "{mode:?}: nothing changes");
    }

    // The errors do not change: out of the canvas and out of order.
    let image = image_with(Mode::Rgba, 9, 4, |_, _| 0);
    let id = ImageId::from_raw(0);
    let mut doc = Document::new(image);
    let pixel = value(Mode::Rgba, 7);
    let run = |doc: &mut Document, spans: Vec<Span>| {
        doc.apply(Command::FillSpans {
            image: id,
            spans,
            pixel,
        })
    };
    assert!(matches!(
        run(&mut doc, vec![sp(0, 0, 10)]),
        Err(DocumentError::Span {
            span: 0,
            source: ImageError::OutOfBounds { x: 9, .. }
        })
    ));
    assert_eq!(
        run(&mut doc, vec![sp(1, 0, 2), sp(0, 0, 2)]),
        Err(DocumentError::BadSpans { span: 1 })
    );
    assert_eq!(
        run(&mut doc, vec![sp(0, 3, 3)]),
        Err(DocumentError::BadSpans { span: 0 })
    );
    assert!(!doc.can_undo());
}

fn random_case() -> impl Strategy<Value = (Image, Vec<Span>)> {
    (
        0usize..4,
        1u32..=40,
        1u32..=6,
        2u8..=4,
        any::<u64>(),
        any::<u64>(),
    )
        .prop_map(|(mode, width, height, kinds, seed, mask_seed)| {
            let mut state = seed | 1;
            let mut next = move || {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                state
            };
            let image = image_with(MODES[mode], width, height, |_, _| {
                (next() % u64::from(kinds)) as u8
            });
            let mut state = mask_seed | 1;
            let mut spans = Vec::new();
            for y in 0..height {
                let mut x = 0;
                while x < width {
                    state ^= state << 13;
                    state ^= state >> 7;
                    state ^= state << 17;
                    let start = x + (state % 4) as u32;
                    let len = 1 + ((state >> 8) % 12) as u32;
                    if start >= width {
                        break;
                    }
                    let end = (start + len).min(width);
                    spans.push(sp(y, start, end));
                    x = end;
                }
            }
            (image, spans)
        })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Property 3.
    #[test]
    fn property_3_apply_undo_redo_equal_the_model_and_cost_by_pieces((image, spans) in random_case()) {
        prop_assume!(!spans.is_empty());
        let mode = image.mode();
        let (added, runs) = check(&image, spans.clone(), value(mode, 7));
        // The cost of a step is a function of its pieces only: the same spans on an image with
        // the same pieces cost the same (row 8 pins the number).
        if runs > 0 {
            prop_assert!(added > 0);
        }
    }
}
