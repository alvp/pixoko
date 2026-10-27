//! Tests of the brush and stroke geometry (specs/hito-4-pincel.md, unit 4.3): rows 1 to 5 of
//! «Casos» and properties 1 to 4.
//!
//! Red phase: `Brush`, `BrushShape`, `Spacing`, `spaced_centers`, `stamp_pixels`, `line_path` and
//! `curve_path` do not exist yet, so this file does not compile until the logic of `px-raster`.

use std::collections::BTreeSet;

use proptest::prelude::*;
use px_raster::{
    Brush, BrushShape, Line, Spacing, curve_path, line_path, spaced_centers, stamp_pixels,
};

// ---------------------------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------------------------

/// The mask of `brush` as rows of `#` and `.` on the `n × n` grid whose origin (the center of the
/// stamp) is at `(⌊n/2⌋, ⌊n/2⌋)`.
fn ascii(brush: Brush) -> String {
    let n = brush.size() as i32;
    let f = n / 2;
    let offsets: BTreeSet<(i32, i32)> = brush.offsets().into_iter().collect();
    (0..n)
        .map(|j| {
            (0..n)
                .map(|i| {
                    if offsets.contains(&(i - f, j - f)) {
                        '#'
                    } else {
                        '.'
                    }
                })
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn set(points: &[(u32, u32)]) -> BTreeSet<(u32, u32)> {
    points.iter().copied().collect()
}

fn horizontal(len: i32) -> Vec<(i32, i32)> {
    (0..len).map(|x| (x, 0)).collect()
}

// ---------------------------------------------------------------------------------------------
// Row 1: masks
// ---------------------------------------------------------------------------------------------

const MASKS: [(BrushShape, [&str; 5]); 10] = [
    (
        BrushShape::Round,
        [
            "#",
            "##/##",
            ".#./###/.#.",
            ".##./####/####/.##.",
            ".###./#####/#####/#####/.###.",
        ],
    ),
    (
        BrushShape::Square,
        [
            "#",
            "##/##",
            "###/###/###",
            "####/####/####/####",
            "#####/#####/#####/#####/#####",
        ],
    ),
    (
        BrushShape::Diamond,
        [
            "#",
            "##/##",
            ".#./###/.#.",
            ".##./####/####/.##.",
            "..#../.###./#####/.###./..#..",
        ],
    ),
    (
        BrushShape::Ring,
        [
            "#",
            "##/##",
            ".#./#.#/.#.",
            ".##./#..#/#..#/.##.",
            ".###./##.##/#...#/##.##/.###.",
        ],
    ),
    (
        BrushShape::Horizontal,
        [
            "#",
            "../##",
            ".../###/...",
            "..../..../####/....",
            "...../...../#####/...../.....",
        ],
    ),
    (
        BrushShape::Vertical,
        [
            "#",
            ".#/.#",
            ".#./.#./.#.",
            "..#./..#./..#./..#.",
            "..#../..#../..#../..#../..#..",
        ],
    ),
    (
        BrushShape::Slash,
        [
            "#",
            ".#/#.",
            "..#/.#./#..",
            "...#/..#./.#../#...",
            "....#/...#./..#../.#.../#....",
        ],
    ),
    (
        BrushShape::Backslash,
        [
            "#",
            "#./.#",
            "#../.#./..#",
            "#.../.#../..#./...#",
            "#..../.#.../..#../...#./....#",
        ],
    ),
    (
        BrushShape::Plus,
        [
            "#",
            "##/##",
            ".#./###/.#.",
            ".##./####/####/.##.",
            "..#../..#../#####/..#../..#..",
        ],
    ),
    (
        BrushShape::Cross,
        [
            "#",
            "##/##",
            "#.#/.#./#.#",
            "#..#/.##./.##./#..#",
            "#...#/.#.#./..#../.#.#./#...#",
        ],
    ),
];

#[test]
fn row_01_the_ten_shapes_sizes_1_to_5_match_the_hand_written_masks() {
    for (shape, masks) in MASKS {
        for (k, expected) in masks.iter().enumerate() {
            let brush = Brush::new(shape, k as u32 + 1);
            assert_eq!(ascii(brush), *expected, "{shape:?} size {}", k + 1);
        }
    }
}

#[test]
fn row_01_size_1_is_one_pixel_and_the_size_is_clamped_to_1_through_100() {
    for (shape, _) in MASKS {
        assert_eq!(Brush::new(shape, 1).offsets(), vec![(0, 0)], "{shape:?}");
        assert_eq!(Brush::new(shape, 0).size(), 1, "{shape:?}");
        assert_eq!(Brush::new(shape, 101).size(), 100, "{shape:?}");
        assert_eq!(Brush::new(shape, 100).shape(), shape);
    }
}

// ---------------------------------------------------------------------------------------------
// Row 2: anchor and clipping
// ---------------------------------------------------------------------------------------------

#[test]
fn row_02_the_stamp_is_anchored_at_floor_half_the_size_and_clipped() {
    let square = |n| Brush::new(BrushShape::Square, n);
    // Size 3 in the corner: only the quarter that falls inside.
    assert_eq!(
        set(&stamp_pixels(square(3), [(0, 0)], 6, 6)),
        set(&[(0, 0), (1, 0), (0, 1), (1, 1)])
    );
    // Size 4 covers x - 2 ..= x + 1.
    let four = stamp_pixels(square(4), [(3, 3)], 6, 6);
    assert_eq!(four.len(), 16);
    assert_eq!(four.iter().map(|p| p.0).min(), Some(1));
    assert_eq!(four.iter().map(|p| p.0).max(), Some(4));
    // On the edge: x from 3 to 6 is cut at 5.
    assert_eq!(stamp_pixels(square(4), [(5, 5)], 6, 6).len(), 9);
    // Entirely outside: nothing.
    assert!(stamp_pixels(square(4), [(-10, -10), (50, 2)], 6, 6).is_empty());
}

#[test]
fn row_02_pixels_are_unique_and_in_the_order_of_their_first_visit() {
    let brush = Brush::new(BrushShape::Square, 2);
    assert_eq!(
        stamp_pixels(brush, [(1, 1), (2, 1), (1, 1)], 6, 6),
        vec![(0, 0), (1, 0), (0, 1), (1, 1), (2, 0), (2, 1)]
    );
}

// ---------------------------------------------------------------------------------------------
// Row 3: line with a brush
// ---------------------------------------------------------------------------------------------

#[test]
fn row_03_a_size_1_line_is_the_plain_line() {
    let (a, b) = ((0, 0), (9, 4));
    let plain: Vec<(i32, i32)> = Line::new(a, b).collect();
    assert_eq!(line_path(a, b), plain);
    let drawn = stamp_pixels(Brush::new(BrushShape::Round, 1), line_path(a, b), 20, 20);
    let expected: Vec<(u32, u32)> = plain.iter().map(|&(x, y)| (x as u32, y as u32)).collect();
    assert_eq!(drawn, expected);
}

#[test]
fn row_03_a_thick_line_is_the_union_of_the_stamps_along_the_path() {
    // Round 3 (a plus of 5 pixels) along a horizontal segment from (2, 2) to (5, 2).
    let drawn = stamp_pixels(
        Brush::new(BrushShape::Round, 3),
        line_path((2, 2), (5, 2)),
        8,
        5,
    );
    let mut expected = BTreeSet::new();
    for x in 2..=5 {
        expected.extend([(x, 1), (x, 2), (x, 3)]);
    }
    expected.extend([(1, 2), (6, 2)]);
    assert_eq!(set(&drawn), expected);
    assert_eq!(drawn.len(), expected.len(), "no repeated pixels");
    // Square 2 along a diagonal: each stamp covers (x - 1 ..= x, y - 1 ..= y).
    let diagonal = stamp_pixels(
        Brush::new(BrushShape::Square, 2),
        line_path((1, 1), (3, 3)),
        8,
        8,
    );
    let expected = set(&[
        (0, 0),
        (1, 0),
        (0, 1),
        (1, 1),
        (2, 1),
        (1, 2),
        (2, 2),
        (3, 2),
        (2, 3),
        (3, 3),
    ]);
    assert_eq!(set(&diagonal), expected);
}

// ---------------------------------------------------------------------------------------------
// Row 4: spacing
// ---------------------------------------------------------------------------------------------

#[test]
fn row_04_spacing_picks_centers_from_the_path() {
    let path = horizontal(11);
    let xs = |spacing| -> Vec<i32> {
        spaced_centers(&path, spacing)
            .into_iter()
            .map(|p| p.0)
            .collect()
    };
    assert_eq!(xs(Spacing::EveryN(3)), vec![0, 3, 6, 9]);
    assert_eq!(xs(Spacing::EveryN(1)), (0..11).collect::<Vec<_>>());
    assert_eq!(xs(Spacing::Continuous), (0..11).collect::<Vec<_>>());
    assert_eq!(xs(Spacing::TotalN(5)), vec![0, 2, 5, 7, 10]);
    assert_eq!(xs(Spacing::TotalN(1)), vec![0]);
    assert_eq!(xs(Spacing::TotalN(20)), (0..11).collect::<Vec<_>>());
}

#[test]
fn row_04_zero_counts_are_taken_as_one_and_an_empty_path_gives_nothing() {
    let path = horizontal(11);
    assert_eq!(spaced_centers(&path, Spacing::EveryN(0)).len(), 11);
    assert_eq!(spaced_centers(&path, Spacing::TotalN(0)), vec![(0, 0)]);
    assert!(spaced_centers(&[], Spacing::EveryN(3)).is_empty());
    assert!(spaced_centers(&[], Spacing::TotalN(3)).is_empty());
}

// ---------------------------------------------------------------------------------------------
// Row 5: the curve
// ---------------------------------------------------------------------------------------------

#[test]
fn row_05_the_quadratic_curve_matches_the_hand_computed_points() {
    assert_eq!(
        curve_path((0, 0), (8, 0), (4, 8)),
        vec![
            (0, 0),
            (0, 1),
            (1, 1),
            (1, 2),
            (2, 3),
            (3, 4),
            (4, 4),
            (5, 4),
            (6, 3),
            (7, 2),
            (7, 1),
            (8, 1),
            (8, 0)
        ]
    );
}

#[test]
fn row_05_a_collinear_control_gives_the_straight_segment() {
    assert_eq!(
        curve_path((0, 0), (8, 0), (4, 0)),
        horizontal(9)[..].to_vec()
    );
}

#[test]
fn row_05_degenerate_curves() {
    assert_eq!(curve_path((3, 3), (3, 3), (3, 3)), vec![(3, 3)]);
    // Same ends, control to the side: goes out and comes back.
    assert_eq!(
        curve_path((3, 3), (3, 3), (5, 3)),
        vec![(3, 3), (4, 3), (3, 3)]
    );
}

// ---------------------------------------------------------------------------------------------
// Properties
// ---------------------------------------------------------------------------------------------

fn any_shape() -> impl Strategy<Value = BrushShape> {
    prop::sample::select(MASKS.iter().map(|m| m.0).collect::<Vec<_>>())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Property 1: non-empty, inside its box, symmetric, and the filled shapes only grow.
    #[test]
    fn prop_1_masks_fit_their_box_and_are_symmetric(shape in any_shape(), n in 1u32..=100) {
        let brush = Brush::new(shape, n);
        let n = n as i32;
        let f = n / 2;
        let offsets = brush.offsets();
        prop_assert!(!offsets.is_empty());
        prop_assert_eq!(&offsets, &Brush::new(shape, n as u32).offsets());
        let cells: BTreeSet<(i32, i32)> = offsets.iter().copied().collect();
        prop_assert_eq!(cells.len(), offsets.len(), "no repeated offsets");
        let inside = |v: i32| (-f..n - f).contains(&v);
        for &(x, y) in &cells {
            match shape {
                BrushShape::Horizontal => prop_assert!(inside(x) && y == 0),
                BrushShape::Vertical => prop_assert!(x == 0 && inside(y)),
                _ => prop_assert!(inside(x) && inside(y)),
            }
        }
        // Mirror of a coordinate about the center of the box.
        let m = |v: i32| n - 1 - 2 * f - v;
        for &(x, y) in &cells {
            match shape {
                BrushShape::Slash => prop_assert!(cells.contains(&(m(y), m(x)))),
                BrushShape::Backslash => prop_assert!(cells.contains(&(y, x))),
                BrushShape::Horizontal => prop_assert!(cells.contains(&(m(x), y))),
                BrushShape::Vertical => prop_assert!(cells.contains(&(x, m(y)))),
                _ => {
                    prop_assert!(cells.contains(&(m(x), y)));
                    prop_assert!(cells.contains(&(x, m(y))));
                }
            }
        }
    }

    #[test]
    fn prop_1_filled_shapes_do_not_lose_pixels_when_they_grow(n in 1u32..100) {
        for shape in [BrushShape::Round, BrushShape::Square, BrushShape::Diamond] {
            prop_assert!(Brush::new(shape, n).offsets().len() <= Brush::new(shape, n + 1).offsets().len());
        }
    }

    /// Property 2: `stamp_pixels` is the clipped union of the stamps of the model.
    #[test]
    fn prop_2_stamping_is_the_clipped_union(
        shape in any_shape(),
        n in 1u32..=9,
        width in 1u32..=20,
        height in 1u32..=20,
        centers in prop::collection::vec((-15i32..35, -15i32..35), 0..12),
    ) {
        let brush = Brush::new(shape, n);
        let got = stamp_pixels(brush, centers.iter().copied(), width, height);
        let mut model = BTreeSet::new();
        for &(cx, cy) in &centers {
            for (dx, dy) in brush.offsets() {
                let (x, y) = (cx + dx, cy + dy);
                if (0..width as i32).contains(&x) && (0..height as i32).contains(&y) {
                    model.insert((x as u32, y as u32));
                }
            }
        }
        prop_assert_eq!(got.len(), model.len(), "no repeated pixels");
        prop_assert_eq!(set(&got), model);
    }

    /// Property 3: spacing.
    #[test]
    fn prop_3_spacing_picks_the_documented_indices(
        a in (-20i32..20, -20i32..20),
        b in (-20i32..20, -20i32..20),
        k in 1u32..=40,
    ) {
        let path = line_path(a, b);
        let len = path.len();
        let every = spaced_centers(&path, Spacing::EveryN(k));
        let want: Vec<(i32, i32)> = path.iter().copied().step_by(k as usize).collect();
        prop_assert_eq!(every, want);
        let total = spaced_centers(&path, Spacing::TotalN(k));
        prop_assert_eq!(total.len(), (k as usize).min(len));
        prop_assert_eq!(total.first(), path.first());
        if k >= 2 && (k as usize) <= len {
            prop_assert_eq!(total.last(), path.last());
        }
        let indices: Vec<usize> = total
            .iter()
            .map(|p| path.iter().position(|q| q == p).unwrap_or(usize::MAX))
            .collect();
        prop_assert!(indices.windows(2).all(|w| w[0] < w[1]), "{indices:?}");
    }

    /// Property 4: the curve.
    #[test]
    fn prop_4_curve_is_connected_bounded_and_symmetric(
        a in (-30i32..30, -30i32..30),
        b in (-30i32..30, -30i32..30),
        c in (-30i32..30, -30i32..30),
    ) {
        let path = curve_path(a, b, c);
        prop_assert_eq!(path.first(), Some(&a));
        prop_assert_eq!(path.last(), Some(&b));
        for w in path.windows(2) {
            prop_assert!((w[0].0 - w[1].0).abs() <= 1 && (w[0].1 - w[1].1).abs() <= 1, "{:?}", w);
        }
        let (x0, x1) = (a.0.min(b.0).min(c.0), a.0.max(b.0).max(c.0));
        let (y0, y1) = (a.1.min(b.1).min(c.1), a.1.max(b.1).max(c.1));
        for p in &path {
            prop_assert!((x0..=x1).contains(&p.0) && (y0..=y1).contains(&p.1));
        }
        let forward: BTreeSet<(i32, i32)> = path.iter().copied().collect();
        let backward: BTreeSet<(i32, i32)> = curve_path(b, a, c).into_iter().collect();
        prop_assert_eq!(forward, backward);
    }
}
