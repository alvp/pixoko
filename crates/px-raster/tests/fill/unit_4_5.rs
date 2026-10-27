//! Unit 4.5 (specs/hito-4-relleno.md): the fill rewritten by rows must give the same spans as the
//! naive per-pixel walk, plus the two new modes ("until the background" and the pattern).
//!
//! The oracle (`region_set`) is a plain breadth-first walk with 4 neighbours and
//! `Image::get_pixel`; it uses nothing of `px-raster`.

use std::collections::{BTreeSet, VecDeque};

use proptest::prelude::*;
use px_core::{Command, Document, DocumentError, Image, ImageId, Mode, Palette, Pixel, Rgba, Span};
use px_raster::{
    Brush, BrushShape, PatternTile, fill_command, fill_command_with_limit, fill_pattern_command,
    fill_until_command, fill_until_command_with_limit, pattern_spans,
};

const MODES: [Mode; 4] = [Mode::Rgba, Mode::Indexed, Mode::Gray, Mode::GrayAlpha];

fn palette() -> Palette {
    let colors = (0..8u8).map(|n| Rgba::opaque(n * 30, 255 - n * 30, n * 7));
    Palette::from_colors(colors).unwrap()
}

/// The `n`-th value (0 to 7) of a mode: all different from each other.
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

/// An image whose cell `(x, y)` holds `value(mode, f(x, y))`.
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

/// Image from rows of digits.
fn image_rows(mode: Mode, rows: &[&str]) -> Image {
    let cells: Vec<Vec<u8>> = rows
        .iter()
        .map(|row| row.bytes().map(|b| b - b'0').collect())
        .collect();
    image_with(mode, cells[0].len() as u32, cells.len() as u32, |x, y| {
        cells[y as usize][x as usize]
    })
}

fn doc_of(image: Image) -> (Document, ImageId) {
    (Document::new(image), ImageId::from_raw(0))
}

fn spans_of(command: Result<Option<Command>, DocumentError>) -> Option<Vec<Span>> {
    match command.unwrap() {
        Some(Command::FillSpans { spans, .. }) => Some(spans),
        Some(other) => panic!("FillSpans was expected, got {other:?}"),
        None => None,
    }
}

/// The pixels reached from `(x, y)` with 4 neighbours while `inside` holds.
fn region_set(
    image: &Image,
    x: u32,
    y: u32,
    inside: impl Fn(Pixel) -> bool,
) -> BTreeSet<(u32, u32)> {
    let mut seen = BTreeSet::new();
    let mut queue = VecDeque::new();
    queue.push_back((x, y));
    seen.insert((x, y));
    while let Some((cx, cy)) = queue.pop_front() {
        let around = [
            (cx.wrapping_sub(1), cy),
            (cx + 1, cy),
            (cx, cy.wrapping_sub(1)),
            (cx, cy + 1),
        ];
        for (nx, ny) in around {
            if let Ok(found) = image.get_pixel(nx, ny)
                && inside(found)
                && seen.insert((nx, ny))
            {
                queue.push_back((nx, ny));
            }
        }
    }
    seen
}

/// Maximal spans of a set of pixels, ordered by `(y, x)`.
fn spans_from_set(set: &BTreeSet<(u32, u32)>) -> Vec<Span> {
    let mut by_row: BTreeSet<(u32, u32)> = BTreeSet::new();
    for &(x, y) in set {
        by_row.insert((y, x));
    }
    let mut spans: Vec<Span> = Vec::new();
    for (y, x) in by_row {
        match spans.last_mut() {
            Some(last) if last.y == y && last.x_end == x => last.x_end += 1,
            _ => spans.push(Span {
                y,
                x_start: x,
                x_end: x + 1,
            }),
        }
    }
    spans
}

fn set_of_spans(spans: &[Span]) -> BTreeSet<(u32, u32)> {
    spans
        .iter()
        .flat_map(|s| (s.x_start..s.x_end).map(move |x| (x, s.y)))
        .collect()
}

fn sp(y: u32, x_start: u32, x_end: u32) -> Span {
    Span { y, x_start, x_end }
}

// Row 1.

#[test]
fn row_01_same_spans_as_the_naive_walk_on_fixed_images() {
    let images: [&[&str]; 8] = [
        &["0000", "0000", "0000"],
        &["00000", "01010", "01010"],
        &["0101", "1010", "0101", "1010"],
        &["00000", "01110", "01010", "01110", "00000"],
        &[
            "1111111", "0000001", "1111101", "1000101", "1011101", "1000001", "1111111",
        ],
        &["0101101"],
        &["0"],
        &["0001", "0001", "1000"],
    ];
    for mode in MODES {
        for rows in images {
            let image = image_rows(mode, rows);
            let (w, h) = (image.width(), image.height());
            let (doc, id) = doc_of(image.clone());
            let fill = value(mode, 7);
            for y in 0..h {
                for x in 0..w {
                    let seed = image.get_pixel(x, y).unwrap();
                    let expected = spans_from_set(&region_set(&image, x, y, |p| p == seed));
                    let got = spans_of(fill_command(&doc, id, x as i32, y as i32, fill));
                    assert_eq!(got, Some(expected), "{mode:?} {rows:?} at ({x}, {y})");
                    let same = fill_command(&doc, id, x as i32, y as i32, seed);
                    assert_eq!(same, Ok(None), "the same value is nothing to do");
                }
            }
            assert_eq!(fill_command(&doc, id, -1, 0, fill), Ok(None));
            assert_eq!(fill_command(&doc, id, w as i32, 0, fill), Ok(None));
            assert_eq!(fill_command(&doc, id, 0, h as i32, fill), Ok(None));
        }
    }
}

// Rows 2 and 3.

#[test]
fn row_02_until_the_background_on_one_row() {
    // `1 2 3 B 5 6` with B = 4.
    let image = image_with(Mode::Rgba, 6, 1, |x, _| (x + 1) as u8);
    let (doc, id) = doc_of(image);
    let (fill, background) = (value(Mode::Rgba, 7), value(Mode::Rgba, 4));
    let run = |x: i32, fill: Pixel| fill_until_command(&doc, id, x, 0, fill, background);
    assert_eq!(spans_of(run(1, fill)), Some(vec![sp(0, 0, 3)]));
    assert_eq!(run(3, fill), Ok(None), "the seed is the background");
    assert_eq!(spans_of(run(4, fill)), Some(vec![sp(0, 4, 6)]));
    assert_eq!(
        spans_of(run(0, background)),
        Some(vec![sp(0, 0, 3)]),
        "the fill may be the background color"
    );
    assert_eq!(run(-1, fill), Ok(None));
    assert_eq!(run(6, fill), Ok(None));
}

/// A ring of `4` (the background) in the box `(1, 1)..=(5, 5)` of a 7×7 image; inside, a gradient
/// of the values 0 to 3 and 5 to 7; outside, the value 0. `gap` removes some cells of the ring.
fn ring(mode: Mode, gap: &[(u32, u32)]) -> Image {
    image_with(mode, 7, 7, |x, y| {
        let on_ring = (1..=5).contains(&x)
            && (1..=5).contains(&y)
            && (x == 1 || x == 5 || y == 1 || y == 5)
            && !gap.contains(&(x, y));
        let inside = (2..=4).contains(&x) && (2..=4).contains(&y);
        match (on_ring, inside) {
            (true, _) => 4,
            (false, true) => [5, 6, 7, 1, 2, 3, 5, 6, 7][((y - 2) * 3 + x - 2) as usize],
            (false, false) => 0,
        }
    })
}

#[test]
fn row_03_until_the_background_in_two_dimensions() {
    for mode in MODES {
        let background = value(mode, 4);
        let fill = value(mode, 0);
        let run = |image: &Image, x: i32, y: i32| {
            let (doc, id) = doc_of(image.clone());
            spans_of(fill_until_command(&doc, id, x, y, fill, background)).unwrap()
        };
        let inner: BTreeSet<(u32, u32)> =
            (2..=4).flat_map(|y| (2..=4).map(move |x| (x, y))).collect();

        // Closed ring, and one that is open only on a diagonal: nothing leaks (4 neighbours).
        for gap in [vec![], vec![(1, 1)], vec![(1, 1), (5, 5)]] {
            let image = ring(mode, &gap);
            let got = run(&image, 3, 3);
            assert_eq!(set_of_spans(&got), inner, "{mode:?} gap {gap:?}");
            assert_eq!(got, vec![sp(2, 2, 5), sp(3, 2, 5), sp(4, 2, 5)]);
        }

        // A straight gap lets the fill out: everything that is not the background.
        let open = ring(mode, &[(3, 1)]);
        let got = set_of_spans(&run(&open, 3, 3));
        let expected = region_set(&open, 3, 3, |p| p != background);
        assert_eq!(got, expected, "{mode:?}");
        assert!(got.contains(&(0, 0)) && got.contains(&(3, 1)) && !got.contains(&(1, 2)));
        assert_eq!(got.len(), 49 - 15, "7×7 minus the 16 − 1 cells of the ring");
    }
}

// Row 4.

#[test]
fn row_04_the_14c_caps_hold_for_both_regions() {
    let image = image_rows(Mode::Rgba, &["00000", "01010", "01010"]);
    let (doc, id) = doc_of(image);
    let (fill, one) = (value(Mode::Rgba, 7), value(Mode::Rgba, 1));
    type Run<'a> = Box<dyn Fn(usize) -> Result<Option<Command>, DocumentError> + 'a>;
    let runs: [Run; 2] = [
        Box::new(|max| fill_command_with_limit(&doc, id, 0, 0, fill, max)),
        Box::new(|max| fill_until_command_with_limit(&doc, id, 0, 0, fill, one, max)),
    ];
    for run in &runs {
        assert_eq!(spans_of(run(7)).map(|s| s.len()), Some(7));
        assert_eq!(run(6), Err(DocumentError::FillTooFragmented { limit: 6 }));
        assert_eq!(run(0), Err(DocumentError::FillTooFragmented { limit: 0 }));
    }
    // The cap is the last check: the seed that is the background is `None` even with 0.
    assert_eq!(
        fill_until_command_with_limit(&doc, id, 1, 1, fill, one, 0),
        Ok(None)
    );
    let missing = ImageId::from_raw(99);
    assert_eq!(
        fill_until_command_with_limit(&doc, missing, 0, 0, fill, one, 0),
        Err(DocumentError::UnknownImage(missing))
    );
}

// Rows 5 to 7.

fn tile_rows(brush: &Brush) -> Vec<String> {
    let tile = PatternTile::from_brush(brush);
    let n = brush.size();
    (0..n)
        .map(|y| {
            (0..n)
                .map(|x| if tile.contains(x, y) { '#' } else { '.' })
                .collect()
        })
        .collect()
}

#[test]
fn row_05_the_tile_is_the_mask_of_the_brush() {
    let tile = |shape, size| tile_rows(&Brush::new(shape, size));
    assert_eq!(tile(BrushShape::Square, 2), ["##", "##"]);
    assert_eq!(tile(BrushShape::Plus, 3), [".#.", "###", ".#."]);
    assert_eq!(tile(BrushShape::Round, 3), [".#.", "###", ".#."]);
    assert_eq!(tile(BrushShape::Horizontal, 3), ["...", "###", "..."]);
    assert_eq!(tile(BrushShape::Cross, 3), ["#.#", ".#.", "#.#"]);
    assert_eq!(tile(BrushShape::Slash, 3), ["..#", ".#.", "#.."]);
    assert_eq!(tile(BrushShape::Backslash, 3), ["#..", ".#.", "..#"]);
    assert_eq!(tile(BrushShape::Round, 1), ["#"]);
    // Period n on both axes, also far from the origin.
    let brush = Brush::new(BrushShape::Cross, 3);
    let t = PatternTile::from_brush(&brush);
    for (x, y) in [(0, 0), (1, 1), (2, 0), (1, 0), (0, 1)] {
        for k in [1u32, 7, 1000, 1_000_000] {
            assert_eq!(t.contains(x + 3 * k, y), t.contains(x, y));
            assert_eq!(t.contains(x, y + 3 * k), t.contains(x, y));
        }
    }
    // Size 1 paints everything; the biggest brush is accepted.
    let one = PatternTile::from_brush(&Brush::new(BrushShape::Round, 1));
    assert!(one.contains(0, 0) && one.contains(12345, 678));
    let big = PatternTile::from_brush(&Brush::new(BrushShape::Square, 100));
    assert!(big.contains(99, 99) && big.contains(100, 100));
}

#[test]
fn row_06_the_pattern_is_aligned_to_the_page() {
    let region: Vec<Span> = (2..7).map(|y| sp(y, 3, 10)).collect();
    let slash2 = PatternTile::from_brush(&Brush::new(BrushShape::Slash, 2));
    let got = set_of_spans(&pattern_spans(&region, &slash2, usize::MAX).unwrap());
    let expected: BTreeSet<(u32, u32)> = set_of_spans(&region)
        .into_iter()
        .filter(|&(x, y)| (x + y) % 2 == 1)
        .collect();
    assert_eq!(got, expected, "slash 2 is a checkerboard of the odd cells");

    let plus3 = PatternTile::from_brush(&Brush::new(BrushShape::Plus, 3));
    let got = pattern_spans(&region, &plus3, usize::MAX).unwrap();
    let expected: BTreeSet<(u32, u32)> = set_of_spans(&region)
        .into_iter()
        .filter(|&(x, y)| plus3.contains(x, y))
        .collect();
    assert_eq!(set_of_spans(&got), expected);
    // Ordered by (y, x), no overlap, no empty span, inside the region.
    assert!(got.iter().all(|s| s.x_start < s.x_end));
    assert!(
        got.windows(2)
            .all(|w| (w[0].y, w[0].x_end) <= (w[1].y, w[1].x_start))
    );

    // Two contiguous regions give the same as the union: the pattern continues across them.
    let left: Vec<Span> = (0..4).map(|y| sp(y, 0, 5)).collect();
    let right: Vec<Span> = (0..4).map(|y| sp(y, 5, 9)).collect();
    let both: Vec<Span> = (0..4).map(|y| sp(y, 0, 9)).collect();
    let a = pattern_spans(&left, &plus3, usize::MAX).unwrap();
    let b = pattern_spans(&right, &plus3, usize::MAX).unwrap();
    let union = set_of_spans(&a).union(&set_of_spans(&b)).copied().collect();
    assert_eq!(
        set_of_spans(&pattern_spans(&both, &plus3, usize::MAX).unwrap()),
        union
    );
}

#[test]
fn row_07_the_pattern_command_paints_only_the_mask_inside_the_region() {
    let tile = PatternTile::from_brush(&Brush::new(BrushShape::Round, 3));
    for mode in MODES {
        let image = image_with(mode, 12, 12, |x, _| u8::from(x >= 6));
        let (mut doc, id) = doc_of(image.clone());
        let fill = value(mode, 7);
        let command = fill_pattern_command(&doc, id, 1, 1, fill, &tile)
            .unwrap()
            .expect("a pattern command");
        assert!(doc.apply(command).unwrap());
        let after = doc.image(id).unwrap();
        for y in 0..12 {
            for x in 0..12 {
                let painted = x < 6 && tile.contains(x, y);
                let want = if painted {
                    fill
                } else {
                    image.get_pixel(x, y).unwrap()
                };
                assert_eq!(after.get_pixel(x, y).unwrap(), want, "{mode:?} ({x}, {y})");
            }
        }
        // One step to undo.
        doc.undo().unwrap();
        assert_eq!(doc.image(id).unwrap().bytes(), image.bytes());
    }

    // Nothing to do: the seed already has the color, or the region has no pixel of the mask.
    let mut image = image_with(Mode::Rgba, 12, 12, |_, _| 0);
    image.set_pixel(0, 0, value(Mode::Rgba, 1)).unwrap();
    let (doc, id) = doc_of(image);
    let fill = value(Mode::Rgba, 7);
    assert_eq!(
        fill_pattern_command(&doc, id, 5, 5, value(Mode::Rgba, 0), &tile),
        Ok(None)
    );
    assert_eq!(
        fill_pattern_command(&doc, id, 0, 0, fill, &tile),
        Ok(None),
        "the corner (0, 0) is not in the plus"
    );
    assert_eq!(fill_pattern_command(&doc, id, -1, 0, fill, &tile), Ok(None));

    // The cap counts the pieces of the pattern.
    let slash2 = PatternTile::from_brush(&Brush::new(BrushShape::Slash, 2));
    let row = [sp(0, 0, 10)];
    assert_eq!(pattern_spans(&row, &slash2, 5).map(|s| s.len()), Ok(5));
    assert_eq!(
        pattern_spans(&row, &slash2, 4),
        Err(DocumentError::FillTooFragmented { limit: 4 })
    );
}

// Properties.

fn arbitrary_image() -> impl Strategy<Value = Image> {
    (0usize..4, 1u32..=12, 1u32..=12, 2u8..=3, any::<u64>()).prop_map(
        |(mode, width, height, kinds, seed)| {
            let mut state = seed | 1;
            image_with(MODES[mode], width, height, move |_, _| {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                (state % u64::from(kinds)) as u8
            })
        },
    )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Property 1: the solid fill is the naive walk.
    #[test]
    fn property_1_solid_fill_equals_the_naive_walk(
        image in arbitrary_image(), x in -2i32..14, y in -2i32..14, with_seed_value in any::<bool>(),
    ) {
        let mode = image.mode();
        let (doc, id) = doc_of(image.clone());
        let seed = u32::try_from(x).ok().zip(u32::try_from(y).ok())
            .and_then(|(x, y)| image.get_pixel(x, y).ok());
        let fill = if with_seed_value { seed.unwrap_or(value(mode, 6)) } else { value(mode, 6) };
        let got = fill_command(&doc, id, x, y, fill).unwrap();
        match seed {
            Some(seed) if seed != fill => {
                let (ux, uy) = (x as u32, y as u32);
                let expected = spans_from_set(&region_set(&image, ux, uy, |p| p == seed));
                prop_assert_eq!(got, Some(Command::FillSpans { image: id, spans: expected, pixel: fill }));
            }
            _ => prop_assert_eq!(got, None),
        }
    }

    /// Property 2: "until the background" is the naive walk with "different from".
    #[test]
    fn property_2_until_the_background_equals_the_naive_walk(
        image in arbitrary_image(), x in -2i32..14, y in -2i32..14, limit in 0usize..40,
    ) {
        let mode = image.mode();
        let (doc, id) = doc_of(image.clone());
        let background = value(mode, 1);
        let fill = value(mode, 6);
        let seed = u32::try_from(x).ok().zip(u32::try_from(y).ok())
            .and_then(|(x, y)| image.get_pixel(x, y).ok().map(|p| (x, y, p)));
        let got = fill_until_command_with_limit(&doc, id, x, y, fill, background, limit);
        match seed {
            Some((sx, sy, seed)) if seed != background => {
                let set = region_set(&image, sx, sy, |p| p != background);
                prop_assert!(set.iter().all(|&(px, py)| image.get_pixel(px, py).unwrap() != background));
                let expected = spans_from_set(&set);
                if expected.len() > limit {
                    prop_assert_eq!(got, Err(DocumentError::FillTooFragmented { limit }));
                } else {
                    prop_assert_eq!(got, Ok(Some(Command::FillSpans { image: id, spans: expected, pixel: fill })));
                }
            }
            _ => prop_assert_eq!(got, Ok(None)),
        }
    }

    /// Property 4: the pattern is exactly the mask inside the region.
    #[test]
    fn property_4_pattern_spans_equal_the_mask_inside_the_region(
        rows in prop::collection::vec(prop::collection::vec(any::<bool>(), 12), 1..12),
        shape in 0usize..10, size in 1u32..=12, limit in 0usize..60,
    ) {
        let region_set_: BTreeSet<(u32, u32)> = rows.iter().enumerate()
            .flat_map(|(y, row)| row.iter().enumerate()
                .filter(|(_, &on)| on).map(move |(x, _)| (x as u32, y as u32)))
            .collect();
        let region = spans_from_set(&region_set_);
        let tile = PatternTile::from_brush(&Brush::new(BrushShape::ALL[shape], size));
        let expected: BTreeSet<(u32, u32)> =
            region_set_.iter().copied().filter(|&(x, y)| tile.contains(x, y)).collect();
        let expected_spans = spans_from_set(&expected);
        let got = pattern_spans(&region, &tile, limit);
        if expected_spans.len() > limit {
            prop_assert_eq!(got, Err(DocumentError::FillTooFragmented { limit }));
        } else {
            prop_assert_eq!(got, Ok(expected_spans));
        }
    }
}
