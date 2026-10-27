//! Tests of the shape geometry (specs/hito-4-formas.md, unit 4.4): rows 1 to 6 of «Casos» and
//! properties 1 to 4.
//!
//! Red phase: `circle_radius`, `rect_spans`, `ellipse_spans`, `polygon_spans`, `rect_ring`,
//! `ellipse_ring`, `polygon_ring` and `spaced_centers_closed` do not exist yet, so this file does
//! not compile until the logic of `px-raster`.

use std::collections::BTreeSet;
use std::time::{Duration, Instant};

use proptest::prelude::*;
use px_core::Span;
use px_raster::{
    Line, Spacing, circle_radius, ellipse_ring, ellipse_spans, polygon_ring, polygon_spans,
    rect_ring, rect_spans, spaced_centers_closed,
};

// ---------------------------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------------------------

fn pixels(spans: &[Span]) -> BTreeSet<(i32, i32)> {
    spans
        .iter()
        .flat_map(|s| (s.x_start..s.x_end).map(move |x| (x as i32, s.y as i32)))
        .collect()
}

/// The rows `y0..=y1` and columns `x0..=x1` of `set` as `#` and `.`, joined by `/`.
fn ascii(set: &BTreeSet<(i32, i32)>, x0: i32, y0: i32, x1: i32, y1: i32) -> String {
    (y0..=y1)
        .map(|y| {
            (x0..=x1)
                .map(|x| if set.contains(&(x, y)) { '#' } else { '.' })
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("/")
}

/// The spans are sorted by row and `x`, without empty or touching/overlapping spans, and inside the
/// canvas.
fn assert_well_formed(spans: &[Span], width: u32, height: u32) {
    for s in spans {
        assert!(s.x_start < s.x_end, "tramo vacío: {s:?}");
        assert!(s.x_end <= width && s.y < height, "fuera del lienzo: {s:?}");
    }
    for pair in spans.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        assert!(
            a.y < b.y || (a.y == b.y && a.x_end < b.x_start),
            "desordenados o juntos: {a:?} {b:?}"
        );
    }
}

fn naive_inside(a: i64, b: i64, dx: i64, dy: i64) -> bool {
    if dx.abs() > a || dy.abs() > b {
        return false;
    }
    match (a, b) {
        (0, 0) => dx == 0 && dy == 0,
        (0, _) => dx == 0 && dy.abs() <= b,
        (_, 0) => dy == 0 && dx.abs() <= a,
        _ => 2 * (b * b * dx * dx + a * a * dy * dy) < a * b * (2 * a * b + a + b),
    }
}

/// The ellipse of the naive model, centered at `(cx, cy)`.
fn naive_ellipse(cx: i32, cy: i32, a: u32, b: u32) -> BTreeSet<(i32, i32)> {
    let (a, b) = (i64::from(a), i64::from(b));
    let mut set = BTreeSet::new();
    for dy in -b..=b {
        for dx in -a..=a {
            if naive_inside(a, b, dx, dy) {
                set.insert((cx + dx as i32, cy + dy as i32));
            }
        }
    }
    set
}

/// The pixels of `set` with a 4-neighbour outside `set`.
fn border_of(set: &BTreeSet<(i32, i32)>) -> BTreeSet<(i32, i32)> {
    set.iter()
        .copied()
        .filter(|&(x, y)| {
            [(1, 0), (-1, 0), (0, 1), (0, -1)]
                .iter()
                .any(|&(dx, dy)| !set.contains(&(x + dx, y + dy)))
        })
        .collect()
}

fn ring_set(ring: &[(i32, i32)]) -> BTreeSet<(i32, i32)> {
    ring.iter().copied().collect()
}

// ---------------------------------------------------------------------------------------------
// Row 1: radius of the circle
// ---------------------------------------------------------------------------------------------

#[test]
fn row_01_circle_radius_is_the_rounded_distance() {
    for (dx, dy, radius) in [
        (0, 0, 0),
        (1, 0, 1),
        (1, 1, 1),
        (2, 1, 2),
        (3, 4, 5),
        (5, 12, 13),
        (-3, 4, 5),
        (0, -7, 7),
    ] {
        assert_eq!(circle_radius(dx, dy), radius, "({dx}, {dy})");
    }
}

#[test]
fn row_01_circle_radius_satisfies_the_inequality_without_overflow() {
    let extreme = i64::from(i32::MAX);
    for (dx, dy) in [(extreme, extreme), (-extreme - 1, extreme), (extreme, 0)] {
        let r = i128::from(circle_radius(dx, dy));
        let d2 = i128::from(dx) * i128::from(dx) + i128::from(dy) * i128::from(dy);
        assert!(r * r - r < d2 && d2 <= r * r + r, "({dx}, {dy}) -> {r}");
    }
    for d2 in 0..2000_i64 {
        // (d, 0) has the distance d: the sweep covers many non-square squared distances through
        // the pairs (x, y) with x² + y² = d2.
        for x in 0..=45_i64 {
            let y2 = d2 - x * x;
            if y2 < 0 {
                break;
            }
            let y = (y2 as f64).sqrt() as i64;
            if y * y == y2 {
                let r = i64::from(circle_radius(x, y));
                if d2 == 0 {
                    assert_eq!(r, 0);
                    continue;
                }
                assert!(r * r - r < d2 && d2 <= r * r + r, "({x}, {y}) -> {r}");
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Row 2: circle and ellipse (ASCII)
// ---------------------------------------------------------------------------------------------

/// `(a, b, filled, outline)`: the box of `2a+1` by `2b+1` pixels around the center.
const ELLIPSES: [(u32, u32, &str, &str); 10] = [
    (0, 0, "#", "#"),
    (1, 1, ".#./###/.#.", ".#./#.#/.#."),
    (
        2,
        2,
        ".###./#####/#####/#####/.###.",
        ".###./#...#/#...#/#...#/.###.",
    ),
    (
        3,
        3,
        "..###../.#####./#######/#######/#######/.#####./..###..",
        "..###../.#...#./#.....#/#.....#/#.....#/.#...#./..###..",
    ),
    (
        4,
        4,
        "...###.../.#######./.#######./#########/#########/#########/.#######./.#######./...###...",
        "...###.../.##...##./.#.....#./#.......#/#.......#/#.......#/.#.....#./.##...##./...###...",
    ),
    (
        3,
        2,
        "..###../#######/#######/#######/..###..",
        "..###../##...##/#.....#/##...##/..###..",
    ),
    (
        5,
        1,
        "..#######../###########/..#######..",
        "..#######../##.......##/..#######..",
    ),
    (
        2,
        4,
        ".###./.###./#####/#####/#####/#####/#####/.###./.###.",
        ".###./.#.#./#...#/#...#/#...#/#...#/#...#/.#.#./.###.",
    ),
    (0, 3, "#/#/#/#/#/#/#", "#/#/#/#/#/#/#"),
    (3, 0, "#######", "#######"),
];

#[test]
fn row_02_filled_ellipses_and_circles_match_the_hand_written_masks() {
    for (a, b, filled, _) in ELLIPSES {
        let (cx, cy) = (a as i32 + 2, b as i32 + 2);
        let (w, h) = (2 * a + 5, 2 * b + 5);
        let spans = ellipse_spans((cx, cy), a, b, w, h);
        assert_well_formed(&spans, w, h);
        let got = ascii(
            &pixels(&spans),
            cx - a as i32,
            cy - b as i32,
            cx + a as i32,
            cy + b as i32,
        );
        assert_eq!(got, filled, "relleno ({a}, {b})");
        assert_eq!(pixels(&spans).len(), filled.matches('#').count());
    }
}

#[test]
fn row_02_outlines_are_the_four_neighbour_border_of_the_fill() {
    for (a, b, _, outline) in ELLIPSES {
        let (cx, cy) = (a as i32 + 2, b as i32 + 2);
        let (w, h) = (2 * a + 5, 2 * b + 5);
        let ring = ellipse_ring((cx, cy), a, b, w, h, 0);
        let got = ascii(
            &ring_set(&ring),
            cx - a as i32,
            cy - b as i32,
            cx + a as i32,
            cy + b as i32,
        );
        assert_eq!(got, outline, "contorno ({a}, {b})");
        assert_eq!(
            ring.len(),
            ring_set(&ring).len(),
            "sin repetidos ({a}, {b})"
        );
    }
}

#[test]
fn row_02_the_ring_goes_clockwise_from_the_east() {
    let ring = ellipse_ring((5, 5), 1, 1, 11, 11, 0);
    assert_eq!(ring, vec![(6, 5), (5, 6), (4, 5), (5, 4)]);
    for (a, b, _, _) in ELLIPSES.iter().filter(|(a, b, ..)| *a > 0 && *b > 0) {
        let (cx, cy) = (*a as i32 + 2, *b as i32 + 2);
        let ring = ellipse_ring((cx, cy), *a, *b, 2 * a + 5, 2 * b + 5, 0);
        assert!(
            ring[0].1 == cy && ring[0].0 > cx,
            "empieza en el este ({a}, {b}): {:?}",
            ring[0]
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Row 3: rectangle
// ---------------------------------------------------------------------------------------------

#[test]
fn row_03_rectangle_ring_is_clockwise_from_the_top_left_in_any_corner_order() {
    let expected = vec![
        (1, 1),
        (2, 1),
        (3, 1),
        (4, 1),
        (4, 2),
        (4, 3),
        (3, 3),
        (2, 3),
        (1, 3),
        (1, 2),
    ];
    for (a, b) in [
        ((1, 1), (4, 3)),
        ((4, 3), (1, 1)),
        ((4, 1), (1, 3)),
        ((1, 3), (4, 1)),
    ] {
        assert_eq!(rect_ring(a, b, 10, 10, 0), expected, "{a:?} {b:?}");
    }
}

#[test]
fn row_03_degenerate_rectangles_are_a_point_or_a_line() {
    assert_eq!(rect_ring((2, 2), (2, 2), 6, 6, 0), vec![(2, 2)]);
    assert_eq!(
        rect_ring((2, 0), (2, 4), 6, 6, 0),
        vec![(2, 0), (2, 1), (2, 2), (2, 3), (2, 4)]
    );
    assert_eq!(
        rect_ring((0, 3), (4, 3), 6, 6, 0),
        vec![(0, 3), (1, 3), (2, 3), (3, 3), (4, 3)]
    );
}

#[test]
fn row_03_rectangle_fill_is_the_box_one_span_per_row() {
    let spans = rect_spans((4, 3), (1, 1), 10, 10);
    assert_eq!(
        spans,
        (1..=3)
            .map(|y| Span {
                y,
                x_start: 1,
                x_end: 5
            })
            .collect::<Vec<_>>()
    );
    assert_eq!(
        rect_spans((2, 2), (2, 2), 6, 6),
        vec![Span {
            y: 2,
            x_start: 2,
            x_end: 3
        }]
    );
}

// ---------------------------------------------------------------------------------------------
// Row 4: polygon, even-odd
// ---------------------------------------------------------------------------------------------

#[test]
fn row_04_triangle_fill_includes_its_border() {
    let spans = polygon_spans(&[(0, 0), (6, 0), (0, 6)], 10, 10);
    assert_well_formed(&spans, 10, 10);
    assert_eq!(
        ascii(&pixels(&spans), 0, 0, 6, 6),
        "#######/######./#####../####.../###..../##...../#......"
    );
}

#[test]
fn row_04_concave_notch_leaves_the_hollow_empty() {
    let spans = polygon_spans(
        &[
            (0, 0),
            (2, 0),
            (2, 4),
            (4, 4),
            (4, 0),
            (6, 0),
            (6, 6),
            (0, 6),
        ],
        10,
        10,
    );
    assert_eq!(
        ascii(&pixels(&spans), 0, 0, 6, 6),
        "###.###/###.###/###.###/###.###/#######/#######/#######"
    );
}

#[test]
fn row_04_self_intersecting_bowtie_fills_two_triangles() {
    let spans = polygon_spans(&[(0, 0), (6, 0), (0, 6), (6, 6)], 10, 10);
    assert_eq!(
        ascii(&pixels(&spans), 0, 0, 6, 6),
        "#######/.#####./..###../...#.../..###../.#####./#######"
    );
}

#[test]
fn row_04_pentagram_has_an_empty_center_and_filled_tips() {
    // Winding number 2 in the central pentagon: even-odd leaves it empty.
    let star = [(12, 2), (18, 20), (2, 9), (22, 9), (6, 20)];
    let spans = polygon_spans(&star, 25, 25);
    assert_well_formed(&spans, 25, 25);
    let set = pixels(&spans);
    assert!(!set.contains(&(12, 12)), "el centro es un agujero");
    for tip in [(12, 5), (4, 9), (20, 9), (7, 18), (17, 18)] {
        assert!(set.contains(&tip), "{tip:?}");
    }
    for vertex in star {
        assert!(set.contains(&vertex), "{vertex:?}");
    }
}

#[test]
fn row_04_fewer_than_three_vertices_are_a_line_or_a_point() {
    assert!(polygon_spans(&[], 6, 6).is_empty());
    assert_eq!(
        pixels(&polygon_spans(&[(2, 3)], 6, 6)),
        BTreeSet::from([(2, 3)])
    );
    let line: BTreeSet<(i32, i32)> = Line::new((0, 0), (4, 2)).collect();
    assert_eq!(pixels(&polygon_spans(&[(0, 0), (4, 2)], 6, 6)), line);
}

// ---------------------------------------------------------------------------------------------
// Row 5: spacing on a closed ring
// ---------------------------------------------------------------------------------------------

fn twelve() -> Vec<(i32, i32)> {
    (0..12).map(|i| (i, 0)).collect()
}

fn indices(centers: &[(i32, i32)]) -> Vec<i32> {
    centers.iter().map(|c| c.0).collect()
}

#[test]
fn row_05_every_n_takes_the_indices_0_n_2n() {
    let ring = twelve();
    assert_eq!(
        indices(&spaced_centers_closed(&ring, Spacing::EveryN(5))),
        vec![0, 5, 10]
    );
    assert_eq!(
        spaced_centers_closed(&ring, Spacing::EveryN(1)),
        ring,
        "EveryN(1) es continuo"
    );
    assert_eq!(spaced_centers_closed(&ring, Spacing::Continuous), ring);
    assert_eq!(
        spaced_centers_closed(&ring, Spacing::EveryN(0)),
        ring,
        "0 se toma como 1"
    );
}

#[test]
fn row_05_total_n_spreads_k_centers_around_the_closed_ring() {
    let ring = twelve();
    assert_eq!(
        indices(&spaced_centers_closed(&ring, Spacing::TotalN(4))),
        vec![0, 3, 6, 9]
    );
    assert_eq!(
        indices(&spaced_centers_closed(&ring, Spacing::TotalN(1))),
        vec![0]
    );
    assert_eq!(
        indices(&spaced_centers_closed(&ring, Spacing::TotalN(0))),
        vec![0],
        "0 se toma como 1"
    );
    assert_eq!(
        spaced_centers_closed(&ring, Spacing::TotalN(30)),
        ring,
        "k > L = todos"
    );
    assert!(spaced_centers_closed(&[], Spacing::TotalN(3)).is_empty());
}

// ---------------------------------------------------------------------------------------------
// Row 6: clipping and extremes
// ---------------------------------------------------------------------------------------------

#[test]
fn row_06_shapes_partly_and_wholly_outside_a_6_by_6_canvas() {
    // Rectangle on the corner.
    let spans = rect_spans((-3, -3), (2, 2), 6, 6);
    assert_well_formed(&spans, 6, 6);
    assert_eq!(pixels(&spans).len(), 9);
    // Wholly outside.
    assert!(rect_spans((10, 10), (20, 20), 6, 6).is_empty());
    assert!(ellipse_spans((40, 40), 3, 3, 6, 6).is_empty());
    assert!(polygon_spans(&[(10, 10), (20, 10), (15, 20)], 6, 6).is_empty());
    assert!(rect_ring((10, 10), (20, 20), 6, 6, 0).is_empty());
    assert!(ellipse_ring((40, 40), 3, 3, 6, 6, 0).is_empty());
    assert!(polygon_ring(&[(10, 10), (20, 10), (15, 20)], 6, 6, 0).is_empty());
    // A circle partly outside is the model clipped.
    let spans = ellipse_spans((0, 0), 3, 3, 6, 6);
    let model: BTreeSet<(i32, i32)> = naive_ellipse(0, 0, 3, 3)
        .into_iter()
        .filter(|&(x, y)| (0..6).contains(&x) && (0..6).contains(&y))
        .collect();
    assert_eq!(pixels(&spans), model);
}

#[test]
fn row_06_extreme_coordinates_and_radii_finish_at_once() {
    let start = Instant::now();
    let full = rect_spans((i32::MIN, i32::MIN), (i32::MAX, i32::MAX), 6, 6);
    assert_eq!(pixels(&full).len(), 36);
    let huge = ellipse_spans((0, 0), 1 << 30, 1 << 30, 6, 6);
    assert_eq!(pixels(&huge).len(), 36);
    let wild = ellipse_spans((i32::MIN, i32::MAX), u32::MAX, u32::MAX, 6, 6);
    assert_well_formed(&wild, 6, 6);
    let tri = [(i32::MIN, i32::MIN), (i32::MAX, i32::MIN), (0, i32::MAX)];
    let spans = polygon_spans(&tri, 64, 64);
    assert_well_formed(&spans, 64, 64);
    for ring in [
        rect_ring((i32::MIN, i32::MIN), (i32::MAX, i32::MAX), 6, 6, 2),
        ellipse_ring((3, 3), 1 << 30, 1 << 30, 6, 6, 2),
        ellipse_ring((0, 0), u32::MAX, 5, 6, 6, 2),
        polygon_ring(&tri, 6, 6, 2),
    ] {
        assert!(ring.len() < 1000, "{} puntos", ring.len());
        for (x, y) in ring {
            assert!((-2..8).contains(&x) && (-2..8).contains(&y), "({x}, {y})");
        }
    }
    assert!(
        start.elapsed() < Duration::from_secs(5),
        "tardó {:?}",
        start.elapsed()
    );
}

// ---------------------------------------------------------------------------------------------
// Properties
// ---------------------------------------------------------------------------------------------

/// The naive even-odd model with the outline of the sides (`Line`), clipped to the canvas.
fn naive_polygon(vertices: &[(i32, i32)], width: i32, height: i32) -> BTreeSet<(i32, i32)> {
    let n = vertices.len();
    let mut set = BTreeSet::new();
    for y in 0..height {
        for x in 0..width {
            let mut crossings = 0;
            for i in 0..n {
                let (x0, y0) = vertices[i];
                let (x1, y1) = vertices[(i + 1) % n];
                if (y0 <= y && y < y1) || (y1 <= y && y < y0) {
                    // x_c > x  <=>  x0 + (y - y0)(x1 - x0)/(y1 - y0) > x, exact with a positive
                    // denominator.
                    let (num, den) = (i64::from(y - y0) * i64::from(x1 - x0), i64::from(y1 - y0));
                    let (num, den) = if den < 0 { (-num, -den) } else { (num, den) };
                    if i64::from(x0 - x) * den + num > 0 {
                        crossings += 1;
                    }
                }
            }
            if crossings % 2 == 1 {
                set.insert((x, y));
            }
        }
    }
    for i in 0..n {
        let from = vertices[i];
        let to = vertices[(i + 1) % n];
        set.extend(
            Line::new(from, to)
                .filter(|&(x, y)| (0..width).contains(&x) && (0..height).contains(&y)),
        );
    }
    set
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Property 1: the ellipse is the model, symmetric, one span per row, nested, and its ring is
    /// exactly the four-neighbour border.
    #[test]
    fn prop_01_ellipse(a in 0u32..=60, b in 0u32..=60) {
        let (cx, cy) = (a as i32 + 3, b as i32 + 3);
        let (w, h) = (2 * a + 7, 2 * b + 7);
        let spans = ellipse_spans((cx, cy), a, b, w, h);
        assert_well_formed(&spans, w, h);
        let set = pixels(&spans);
        prop_assert_eq!(&set, &naive_ellipse(cx, cy, a, b));
        let mut rows = BTreeSet::new();
        for s in &spans {
            prop_assert!(rows.insert(s.y), "dos tramos en la fila {}", s.y);
        }
        for &(x, y) in &set {
            prop_assert!(set.contains(&(2 * cx - x, y)), "espejo en x");
            prop_assert!(set.contains(&(x, 2 * cy - y)), "espejo en y");
        }
        let bigger_a = pixels(&ellipse_spans((cx, cy), a + 1, b, w, h));
        let bigger_b = pixels(&ellipse_spans((cx, cy), a, b + 1, w, h));
        prop_assert!(set.is_subset(&bigger_a));
        prop_assert!(set.is_subset(&bigger_b));
        let ring = ellipse_ring((cx, cy), a, b, w, h, 0);
        prop_assert_eq!(ring.len(), ring_set(&ring).len());
        prop_assert_eq!(ring_set(&ring), border_of(&set));
    }

    /// Property 2: the polygon fill is the even-odd model plus the outline, and the ring is in it.
    #[test]
    fn prop_02_polygon(
        vertices in proptest::collection::vec((-5i32..25, -5i32..25), 3..=8),
    ) {
        let spans = polygon_spans(&vertices, 20, 20);
        assert_well_formed(&spans, 20, 20);
        let set = pixels(&spans);
        prop_assert_eq!(&set, &naive_polygon(&vertices, 20, 20));
        let ring = polygon_ring(&vertices, 20, 20, 0);
        for p in &ring {
            prop_assert!(set.contains(p), "{:?} del anillo fuera del relleno", p);
        }
    }

    /// Property 3: the rectangle fill is the clipped box and the rings stay in the window.
    #[test]
    fn prop_03_clip_and_rectangle(
        a in prop_oneof![-10i32..16, Just(i32::MIN), Just(i32::MAX)],
        b in prop_oneof![-10i32..16, Just(i32::MIN), Just(i32::MAX)],
        c in prop_oneof![-10i32..16, Just(i32::MIN), Just(i32::MAX)],
        d in prop_oneof![-10i32..16, Just(i32::MIN), Just(i32::MAX)],
        radius in prop_oneof![0u32..12, Just(u32::MAX)],
        margin in 0u32..5,
    ) {
        let (w, h) = (6u32, 6u32);
        let spans = rect_spans((a, b), (c, d), w, h);
        assert_well_formed(&spans, w, h);
        let (x0, x1) = (a.min(c), a.max(c));
        let (y0, y1) = (b.min(d), b.max(d));
        let model: BTreeSet<(i32, i32)> = (0..h as i32)
            .flat_map(|y| (0..w as i32).map(move |x| (x, y)))
            .filter(|&(x, y)| x0 <= x && x <= x1 && y0 <= y && y <= y1)
            .collect();
        prop_assert_eq!(pixels(&spans), model);
        let m = margin as i32;
        let rings = [
            rect_ring((a, b), (c, d), w, h, margin),
            ellipse_ring((a, b), radius, radius.min(7), w, h, margin),
            polygon_ring(&[(a, b), (c, d), (a, d)], w, h, margin),
        ];
        for ring in rings {
            for pair in ring.windows(2) {
                prop_assert_ne!(pair[0], pair[1], "repetidos consecutivos");
            }
            for &(x, y) in &ring {
                prop_assert!(
                    (-m..w as i32 + m).contains(&x) && (-m..h as i32 + m).contains(&y),
                    "({}, {}) fuera de la ventana", x, y
                );
            }
        }
    }

    /// Property 4: spacing on a closed ring.
    #[test]
    fn prop_04_closed_spacing(len in 1usize..=200, n in 1u32..=300, k in 1u32..=300) {
        let ring: Vec<(i32, i32)> = (0..len as i32).map(|i| (i, 0)).collect();
        let every = spaced_centers_closed(&ring, Spacing::EveryN(n));
        let expected: Vec<(i32, i32)> = (0..len).step_by(n as usize).map(|i| ring[i]).collect();
        prop_assert_eq!(every, expected);
        let total = spaced_centers_closed(&ring, Spacing::TotalN(k));
        prop_assert_eq!(total.len(), (k as usize).min(len));
        prop_assert_eq!(total[0], ring[0]);
        for pair in total.windows(2) {
            prop_assert!(pair[0].0 < pair[1].0, "no crecientes");
        }
    }
}
