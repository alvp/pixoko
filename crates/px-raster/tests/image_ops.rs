//! Tests of the image operations of `px-raster` (specs/hito-4-imagen.md, unit 4.1, changes R-01
//! and R-02): `flip_horizontal`, `flip_vertical`, `rotate_cw`, `rotate_ccw`, `rotate_180`,
//! `resize_canvas`, `Anchor`, `ImageOp` and `image_command`.
//!
//! One test per row 1 to 4 and 7 of «Casos» (`row_N_...`) and one per property (`property_N_...`,
//! 256 cases each). The rows 5, 6 and 8 live in `crates/px-core/tests/replace_images.rs`.
//!
//! The oracle is an independent model written here: grids of pixels indexed `[y][x]` and the
//! formulas of the spec (D3), not the code under test.
//!
//! Red phase: none of those items exists yet, so this file does not compile until the logic.

use proptest::prelude::*;
use px_core::{
    BlendMode, Command, Document, DocumentError, Image, LayerSpec, Mode, Palette, Pixel, Rgba,
};
use px_raster::{
    Anchor, ImageOp, compose, flip_horizontal, flip_vertical, image_command, resize_canvas,
    rotate_180, rotate_ccw, rotate_cw,
};

// ---------------------------------------------------------------------------------------
// Samples and the independent model.
// ---------------------------------------------------------------------------------------

const ANCHORS: [Anchor; 9] = [
    Anchor::TopLeft,
    Anchor::Top,
    Anchor::TopRight,
    Anchor::Left,
    Anchor::Center,
    Anchor::Right,
    Anchor::BottomLeft,
    Anchor::Bottom,
    Anchor::BottomRight,
];

const MODES: [Mode; 4] = [Mode::Rgba, Mode::Gray, Mode::GrayAlpha, Mode::Indexed];

type Grid = Vec<Vec<Pixel>>;

fn palette(transparent: bool) -> Palette {
    let mut palette =
        Palette::from_colors((0..5u8).map(|i| Rgba::new(i * 40, 255 - i * 40, i, 255)))
            .expect("palette");
    if transparent {
        palette.set_transparent_index(Some(0)).expect("transparent");
    }
    palette
}

/// An image whose `i`-th byte is `seed + i` (indexed: modulo 5), so every pixel differs.
fn from_seed(mode: Mode, w: u32, h: u32, transparent: bool, seed: u8) -> Image {
    let bpp = mode.bytes_per_pixel();
    let bytes: Vec<u8> = (0..(w * h) as usize * bpp)
        .map(|i| seed.wrapping_add(i as u8).wrapping_mul(7))
        .collect();
    build(mode, w, h, transparent, bytes)
}

fn build(mode: Mode, w: u32, h: u32, transparent: bool, bytes: Vec<u8>) -> Image {
    match mode {
        Mode::Indexed => {
            let bytes = bytes.into_iter().map(|b| b % 5).collect();
            Image::from_bytes_indexed(w, h, palette(transparent), bytes).expect("indexed")
        }
        mode => Image::from_bytes(w, h, mode, bytes).expect("image"),
    }
}

fn grid(image: &Image) -> Grid {
    (0..image.height())
        .map(|y| {
            (0..image.width())
                .map(|x| image.get_pixel(x, y).expect("pixel"))
                .collect()
        })
        .collect()
}

fn flip_h_model(g: &Grid) -> Grid {
    g.iter()
        .map(|row| row.iter().rev().copied().collect())
        .collect()
}

fn flip_v_model(g: &Grid) -> Grid {
    g.iter().rev().cloned().collect()
}

/// `(x, y)` goes to `(h − 1 − y, x)`.
fn cw_model(g: &Grid) -> Grid {
    let (h, w) = (g.len(), g[0].len());
    let mut out = vec![vec![g[0][0]; h]; w];
    for (y, row) in g.iter().enumerate() {
        for (x, &pixel) in row.iter().enumerate() {
            out[x][h - 1 - y] = pixel;
        }
    }
    out
}

/// `(x, y)` goes to `(y, w − 1 − x)`.
fn ccw_model(g: &Grid) -> Grid {
    let (h, w) = (g.len(), g[0].len());
    let mut out = vec![vec![g[0][0]; h]; w];
    for (y, row) in g.iter().enumerate() {
        for (x, &pixel) in row.iter().enumerate() {
            out[w - 1 - x][y] = pixel;
        }
    }
    out
}

/// Offset of the source along one axis (decision D3: `⌊·⌋` toward −∞).
fn offset(old: u32, new: u32, side: i8) -> i64 {
    let diff = i64::from(new) - i64::from(old);
    match side {
        -1 => 0,
        0 => diff.div_euclid(2),
        _ => diff,
    }
}

fn sides(anchor: Anchor) -> (i8, i8) {
    match anchor {
        Anchor::TopLeft => (-1, -1),
        Anchor::Top => (0, -1),
        Anchor::TopRight => (1, -1),
        Anchor::Left => (-1, 0),
        Anchor::Center => (0, 0),
        Anchor::Right => (1, 0),
        Anchor::BottomLeft => (-1, 1),
        Anchor::Bottom => (0, 1),
        Anchor::BottomRight => (1, 1),
    }
}

fn resize_model(g: &Grid, new_w: u32, new_h: u32, anchor: Anchor, fill: Pixel) -> Grid {
    let (old_h, old_w) = (g.len() as u32, g[0].len() as u32);
    let (sx, sy) = sides(anchor);
    let (dx, dy) = (offset(old_w, new_w, sx), offset(old_h, new_h, sy));
    (0..i64::from(new_h))
        .map(|ny| {
            (0..i64::from(new_w))
                .map(|nx| {
                    let (ox, oy) = (nx - dx, ny - dy);
                    if ox >= 0 && oy >= 0 && ox < i64::from(old_w) && oy < i64::from(old_h) {
                        g[oy as usize][ox as usize]
                    } else {
                        fill
                    }
                })
                .collect()
        })
        .collect()
}

/// The fill the session would pass: transparent where the mode has it.
fn fill_for(image: &Image) -> Pixel {
    match image.mode() {
        Mode::Rgba => Pixel::Rgba(Rgba::TRANSPARENT),
        Mode::Gray => Pixel::Gray(77),
        Mode::GrayAlpha => Pixel::GrayAlpha { gray: 9, alpha: 0 },
        Mode::Indexed => Pixel::Indexed(
            image
                .palette()
                .and_then(Palette::transparent_index)
                .unwrap_or(2),
        ),
    }
}

fn same_meta(a: &Image, b: &Image) {
    assert_eq!(a.mode(), b.mode());
    assert_eq!(a.palette(), b.palette());
}

// ---------------------------------------------------------------------------------------
// Rows 1 to 4.
// ---------------------------------------------------------------------------------------

#[test]
fn row_1_flips_match_the_model_in_the_four_modes_and_twice_is_the_identity() {
    for mode in MODES {
        for transparent in [false, true] {
            let image = from_seed(mode, 3, 2, transparent, 5);
            let g = grid(&image);
            let h = flip_horizontal(&image).expect("flip h");
            let v = flip_vertical(&image).expect("flip v");
            assert_eq!(grid(&h), flip_h_model(&g), "{mode:?} horizontal");
            assert_eq!(grid(&v), flip_v_model(&g), "{mode:?} vertical");
            assert_ne!(h, image, "the sample is not symmetric");
            same_meta(&h, &image);
            same_meta(&v, &image);
            assert_eq!(flip_horizontal(&h).expect("again"), image, "{mode:?}");
            assert_eq!(flip_vertical(&v).expect("again"), image, "{mode:?}");
        }
    }
}

#[test]
fn row_2_turns_match_the_model_and_cancel_each_other() {
    for mode in MODES {
        for (w, h) in [(3, 2), (1, 4)] {
            let image = from_seed(mode, w, h, true, 11);
            let g = grid(&image);
            let cw = rotate_cw(&image).expect("cw");
            let ccw = rotate_ccw(&image).expect("ccw");
            let half = rotate_180(&image).expect("180");
            assert_eq!((cw.width(), cw.height()), (h, w), "{mode:?} swaps the size");
            assert_eq!((half.width(), half.height()), (w, h));
            assert_eq!(grid(&cw), cw_model(&g), "{mode:?} {w}x{h} cw");
            assert_eq!(grid(&ccw), ccw_model(&g), "{mode:?} {w}x{h} ccw");
            assert_eq!(grid(&half), cw_model(&cw_model(&g)), "{mode:?} {w}x{h} 180");
            same_meta(&cw, &image);
            let mut back = image.clone();
            for _ in 0..4 {
                back = rotate_cw(&back).expect("cw");
            }
            assert_eq!(back, image, "four turns");
            assert_eq!(rotate_ccw(&cw).expect("ccw"), image, "cw then ccw");
            assert_eq!(rotate_180(&half).expect("180"), image, "180 twice");
        }
    }
}

#[test]
fn row_3_canvas_anchors_follow_the_offsets_of_the_spec() {
    // (new width, new height): enlarge, shrink, and mixed, with even and odd differences.
    let sizes = [(6, 5), (7, 4), (2, 1), (3, 1), (5, 1), (1, 6), (4, 2)];
    let image = from_seed(Mode::Rgba, 3, 2, false, 3);
    let g = grid(&image);
    let fill = Pixel::Rgba(Rgba::new(9, 8, 7, 6));
    for &(w, h) in &sizes {
        for anchor in ANCHORS {
            let out = resize_canvas(&image, w, h, anchor, fill).expect("resize");
            assert_eq!((out.width(), out.height()), (w, h));
            assert_eq!(
                grid(&out),
                resize_model(&g, w, h, anchor, fill),
                "{w}x{h} {anchor:?}"
            );
        }
    }
    // Center with an odd leftover goes half a pixel up and to the left: 3 → 6 is `⌊3 / 2⌋ = 1`.
    let out = resize_canvas(&image, 6, 2, Anchor::Center, fill).expect("resize");
    assert_eq!(out.get_pixel(1, 0), image.get_pixel(0, 0));
    assert_eq!(out.get_pixel(0, 0), Ok(fill));
    // Same size: the identity.
    assert_eq!(
        resize_canvas(&image, 3, 2, Anchor::Center, fill).expect("same"),
        image
    );
}

#[test]
fn row_4_the_new_area_takes_the_fill_and_the_mode_and_palette_stay() {
    for (mode, transparent) in [
        (Mode::Rgba, false),
        (Mode::GrayAlpha, false),
        (Mode::Indexed, true),
        (Mode::Indexed, false),
        (Mode::Gray, false),
    ] {
        let image = from_seed(mode, 2, 2, transparent, 1);
        let fill = fill_for(&image);
        let out = resize_canvas(&image, 4, 3, Anchor::TopLeft, fill).expect("resize");
        same_meta(&out, &image);
        for (x, y) in [(2, 0), (3, 2), (0, 2)] {
            assert_eq!(out.get_pixel(x, y), Ok(fill), "{mode:?} ({x}, {y})");
        }
        assert_eq!(out.get_pixel(1, 1), image.get_pixel(1, 1));
    }
    // A fill of another mode, an index outside the palette and a zero size are errors.
    let image = from_seed(Mode::Rgba, 2, 2, false, 1);
    assert!(resize_canvas(&image, 3, 3, Anchor::Center, Pixel::Gray(1)).is_err());
    assert!(resize_canvas(&image, 0, 3, Anchor::Center, Pixel::Rgba(Rgba::TRANSPARENT)).is_err());
    let indexed = from_seed(Mode::Indexed, 2, 2, false, 1);
    assert!(resize_canvas(&indexed, 3, 3, Anchor::Center, Pixel::Indexed(200)).is_err());
}

// ---------------------------------------------------------------------------------------
// Row 7 and the documents.
// ---------------------------------------------------------------------------------------

fn spec(
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

fn three_layers() -> Document {
    let layers = vec![
        spec(
            "base",
            true,
            false,
            255,
            BlendMode::Normal,
            Some(from_seed(Mode::Rgba, 3, 2, false, 1)),
        ),
        spec("empty", true, false, 255, BlendMode::Normal, None),
        spec(
            "top",
            false,
            true,
            128,
            BlendMode::Multiply,
            Some(from_seed(Mode::Rgba, 3, 2, false, 90)),
        ),
    ];
    Document::from_layers(3, 2, Mode::Rgba, None, layers).expect("document")
}

fn transform(image: &Image, op: ImageOp, fill: Pixel) -> Image {
    match op {
        ImageOp::FlipH => flip_horizontal(image),
        ImageOp::FlipV => flip_vertical(image),
        ImageOp::Cw => rotate_cw(image),
        ImageOp::Ccw => rotate_ccw(image),
        ImageOp::Half => rotate_180(image),
        ImageOp::Canvas {
            width,
            height,
            anchor,
        } => resize_canvas(image, width, height, anchor, fill),
    }
    .expect("transform")
}

#[test]
fn row_7_the_command_transforms_every_cel_and_compose_commutes() {
    let fill = Pixel::Rgba(Rgba::TRANSPARENT);
    let ops = [
        ImageOp::FlipH,
        ImageOp::FlipV,
        ImageOp::Cw,
        ImageOp::Ccw,
        ImageOp::Half,
        ImageOp::Canvas {
            width: 5,
            height: 4,
            anchor: Anchor::BottomRight,
        },
        ImageOp::Canvas {
            width: 2,
            height: 1,
            anchor: Anchor::Center,
        },
    ];
    for op in ops {
        let mut doc = three_layers();
        let before = doc.clone();
        let composed = compose(&doc).expect("compose");
        let command = image_command(&doc, op, fill).expect("command");
        assert!(matches!(command, Command::ReplaceImages { .. }));
        assert_eq!(doc.apply(command), Ok(true), "{op:?}");
        assert_eq!(doc.layers().len(), 3);
        for (new, old) in doc.layers().iter().zip(before.layers()) {
            assert_eq!(new.id(), old.id());
            assert_eq!(new.name(), old.name());
            assert_eq!(new.visible(), old.visible());
            assert_eq!(new.locked(), old.locked());
            assert_eq!(new.opacity(), old.opacity());
            assert_eq!(new.blend(), old.blend());
            match (doc.image(new.cel()), before.image(old.cel())) {
                (Some(after), Some(prior)) => assert_eq!(*after, transform(prior, op, fill)),
                (None, None) => {}
                other => panic!("emptiness changed: {other:?}"),
            }
        }
        // Compose commutes with the geometric operations and, with a transparent fill, with the
        // canvas resize too (the new area is transparent in every layer).
        assert_eq!(
            compose(&doc).expect("compose"),
            transform(&composed, op, fill),
            "{op:?}"
        );
        doc.undo().expect("undo");
        assert_eq!(doc, before);
        assert_eq!((doc.width(), doc.height()), (3, 2));
    }
}

// ---------------------------------------------------------------------------------------
// Properties.
// ---------------------------------------------------------------------------------------

fn arb_image() -> impl Strategy<Value = Image> {
    (0usize..4, 1u32..=9, 1u32..=7, any::<bool>()).prop_flat_map(|(m, w, h, transparent)| {
        let mode = MODES[m];
        let len = (w * h) as usize * mode.bytes_per_pixel();
        prop::collection::vec(any::<u8>(), len)
            .prop_map(move |bytes| build(mode, w, h, transparent, bytes))
    })
}

fn arb_anchor() -> impl Strategy<Value = Anchor> {
    (0usize..9).prop_map(|i| ANCHORS[i])
}

fn arb_op() -> impl Strategy<Value = ImageOp> {
    prop_oneof![
        Just(ImageOp::FlipH),
        Just(ImageOp::FlipV),
        Just(ImageOp::Cw),
        Just(ImageOp::Ccw),
        Just(ImageOp::Half),
        (1u32..=8, 1u32..=8, arb_anchor()).prop_map(|(width, height, anchor)| ImageOp::Canvas {
            width,
            height,
            anchor
        }),
    ]
}

fn is_geometric(op: ImageOp) -> bool {
    !matches!(op, ImageOp::Canvas { .. })
}

/// A document of 1 to 4 layers (a single one in gray) with random cels, emptiness, visibility,
/// lock, opacity and blend.
fn arb_doc() -> impl Strategy<Value = Document> {
    (0usize..4, 1u32..=6, 1u32..=5, any::<bool>(), 1usize..=4).prop_flat_map(
        |(m, w, h, transparent, n)| {
            let mode = MODES[m];
            let n = if mode == Mode::Gray { 1 } else { n };
            let len = (w * h) as usize * mode.bytes_per_pixel();
            let layer = (
                any::<bool>(),
                any::<bool>(),
                any::<u8>(),
                any::<bool>(),
                any::<bool>(),
                prop::collection::vec(any::<u8>(), len),
            );
            prop::collection::vec(layer, n).prop_map(move |layers| {
                let has_transparency = match mode {
                    Mode::Rgba | Mode::GrayAlpha => true,
                    Mode::Indexed => transparent,
                    Mode::Gray => false,
                };
                let specs = layers
                    .into_iter()
                    .enumerate()
                    .map(|(i, (visible, locked, opacity, multiply, empty, bytes))| {
                        let indexed = mode == Mode::Indexed;
                        spec(
                            &format!("L{i}"),
                            visible,
                            locked,
                            if indexed { 255 } else { opacity },
                            if multiply && !indexed {
                                BlendMode::Multiply
                            } else {
                                BlendMode::Normal
                            },
                            (!(empty && has_transparency))
                                .then(|| build(mode, w, h, transparent, bytes)),
                        )
                    })
                    .collect();
                let palette = (mode == Mode::Indexed).then(|| palette(transparent));
                Document::from_layers(w, h, mode, palette, specs).expect("document")
            })
        },
    )
}

fn doc_fill(doc: &Document) -> Pixel {
    fill_for(doc.canvas_image())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Property 1: the inverses.
    #[test]
    fn property_1_flips_and_turns_have_their_inverses(image in arb_image()) {
        let h = flip_horizontal(&image).unwrap();
        let v = flip_vertical(&image).unwrap();
        prop_assert_eq!(&flip_horizontal(&h).unwrap(), &image);
        prop_assert_eq!(&flip_vertical(&v).unwrap(), &image);
        prop_assert_eq!(
            rotate_180(&image).unwrap(),
            flip_horizontal(&flip_vertical(&image).unwrap()).unwrap()
        );
        let mut back = image.clone();
        for _ in 0..4 {
            back = rotate_cw(&back).unwrap();
        }
        prop_assert_eq!(&back, &image);
        prop_assert_eq!(&rotate_ccw(&rotate_cw(&image).unwrap()).unwrap(), &image);
        prop_assert_eq!(&rotate_cw(&rotate_ccw(&image).unwrap()).unwrap(), &image);
    }

    /// Property 2: the window of the canvas resize and the way back.
    #[test]
    fn property_2_resize_keeps_the_window_and_comes_back(
        image in arb_image(),
        w in 1u32..=12,
        h in 1u32..=12,
        anchor in arb_anchor(),
    ) {
        let fill = fill_for(&image);
        let out = resize_canvas(&image, w, h, anchor, fill).unwrap();
        let g = grid(&image);
        prop_assert_eq!(grid(&out), resize_model(&g, w, h, anchor, fill));
        // Enlarge and cut back with the same anchor: only possible when nothing was cut, and a
        // centered axis needs an even difference (`⌊d / 2⌋` and `⌊−d / 2⌋` differ when `d` is odd).
        let (ow, oh) = (image.width(), image.height());
        let (sx, sy) = sides(anchor);
        let odd_center = (sx == 0 && w.abs_diff(ow) % 2 == 1) || (sy == 0 && h.abs_diff(oh) % 2 == 1);
        if w >= ow && h >= oh && !odd_center {
            let back = resize_canvas(&out, ow, oh, anchor, fill).unwrap();
            prop_assert_eq!(&back, &image);
        }
    }

    /// Property 3: one step per operation, undo and redo exact, compose commutes.
    #[test]
    fn property_3_documents_undo_redo_and_compose(
        doc in arb_doc(),
        ops in prop::collection::vec(arb_op(), 1..=4),
    ) {
        let initial = doc.clone();
        let mut doc = doc;
        let mut snapshots = vec![doc.clone()];
        for op in ops {
            let before = doc.clone();
            let id = doc.state_id();
            let fill = doc_fill(&doc);
            let command = image_command(&doc, op, fill).unwrap();
            if doc.apply(command).unwrap() {
                prop_assert_ne!(doc.state_id(), id);
                prop_assert_eq!(doc.history_bytes() > before.history_bytes(), true);
                if is_geometric(op) {
                    // A gray document with nothing visible has no composition (before or after).
                    match (compose(&before), compose(&doc)) {
                        (Ok(prior), Ok(after)) => {
                            prop_assert_eq!(after, transform(&prior, op, fill));
                        }
                        (prior, after) => prop_assert_eq!(prior.is_err(), after.is_err()),
                    }
                }
                snapshots.push(doc.clone());
            } else {
                prop_assert_eq!(doc.state_id(), id);
            }
        }
        let steps = snapshots.len() - 1;
        for k in (0..steps).rev() {
            doc.undo().unwrap();
            prop_assert_eq!(&doc, &snapshots[k]);
            prop_assert_eq!((doc.width(), doc.height()), (snapshots[k].width(), snapshots[k].height()));
        }
        prop_assert_eq!(&doc, &initial);
        prop_assert_eq!(doc.state_id(), initial.state_id());
        for snapshot in snapshots.iter().skip(1) {
            doc.redo().unwrap();
            prop_assert_eq!(&doc, snapshot);
            prop_assert_eq!((doc.width(), doc.height()), (snapshot.width(), snapshot.height()));
        }
    }

    /// Property 4: memory and failures.
    #[test]
    fn property_4_memory_is_accounted_and_a_failure_changes_nothing(
        doc in arb_doc(),
        op in arb_op(),
        limit_factor in 0usize..=3,
    ) {
        let mut doc = doc;
        let fill = doc_fill(&doc);
        let before = doc.clone();
        let (id, history, bytes) = (doc.state_id(), doc.history_bytes(), doc.materialized_bytes());
        let limit = bytes.saturating_mul(limit_factor) / 2;
        doc.set_cells_limit(limit);
        let applied = image_command(&doc, op, fill).map(|command| doc.apply(command));
        let after = doc.materialized_bytes();
        match applied {
            Ok(Ok(true)) => {
                prop_assert!(after <= limit || after <= bytes, "{after} > {limit} and grew");
                let sum: usize = doc
                    .layers()
                    .iter()
                    .filter_map(|layer| doc.image(layer.cel()))
                    .map(|image| image.bytes().len())
                    .sum();
                prop_assert_eq!(after, sum);
            }
            Ok(Ok(false)) | Ok(Err(_)) | Err(_) => {
                prop_assert_eq!(&doc, &before);
                prop_assert_eq!(doc.state_id(), id);
                prop_assert_eq!(doc.history_bytes(), history);
                prop_assert_eq!((doc.width(), doc.height()), (before.width(), before.height()));
            }
        }
        // The error of the limit is the document's, not a panic.
        if let Ok(Err(error)) = applied {
            let too_large = matches!(error, DocumentError::CelsTooLarge { .. });
            prop_assert!(too_large);
        }
    }
}
