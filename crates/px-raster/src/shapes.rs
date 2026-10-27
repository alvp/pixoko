//! The geometry of the shapes (specs/hito-4-formas.md, unit 4.4): rectangle, circle, ellipse and
//! polygon as clipped fills (`Vec<Span>`) and as ordered closed rings for the brush. Pure integer
//! functions; nothing here grows with the size of the shape, only with the part of the canvas it
//! can touch.

use std::cmp::Ordering;
use std::collections::BTreeMap;

use px_core::Span;

use crate::brush::{Spacing, line_path_near};
use crate::line::Line;

/// Radii are clamped to this value so that the integer products fit in `i128`.
pub const MAX_RADIUS: u32 = 1 << 30;

/// The distance `√(dx² + dy²)` rounded to the nearest integer: the `r` with `r² − r < d² ≤ r² + r`
/// (`0` for `d = 0`). There are no ties: the square root of an integer is never a half.
pub fn circle_radius(dx: i64, dy: i64) -> u32 {
    let d2 = i128::from(dx) * i128::from(dx) + i128::from(dy) * i128::from(dy);
    let mut s = (d2 as f64).sqrt() as i128;
    while s * s > d2 {
        s -= 1;
    }
    while (s + 1) * (s + 1) <= d2 {
        s += 1;
    }
    let r = if d2 > s * s + s { s + 1 } else { s };
    u32::try_from(r).unwrap_or(u32::MAX)
}

fn isqrt(n: i128) -> i128 {
    let mut s = (n as f64).sqrt() as i128;
    while s * s > n {
        s -= 1;
    }
    while (s + 1) * (s + 1) <= n {
        s += 1;
    }
    s
}

/// Rows `y0..=y1` of a box, clipped to `height`, as spans of the columns `x0..=x1` clipped to
/// `width`.
fn box_spans(x0: i64, x1: i64, y0: i64, y1: i64, width: u32, height: u32) -> Vec<Span> {
    let xs = x0.max(0);
    let xe = x1.min(i64::from(width) - 1);
    let ys = y0.max(0);
    let ye = y1.min(i64::from(height) - 1);
    if xs > xe || ys > ye {
        return Vec::new();
    }
    (ys..=ye)
        .map(|y| Span {
            y: y as u32,
            x_start: xs as u32,
            x_end: (xe + 1) as u32,
        })
        .collect()
}

/// The filled box with corners `a` and `b` (in any order), clipped to the canvas.
pub fn rect_spans(a: (i32, i32), b: (i32, i32), width: u32, height: u32) -> Vec<Span> {
    box_spans(
        i64::from(a.0.min(b.0)),
        i64::from(a.0.max(b.0)),
        i64::from(a.1.min(b.1)),
        i64::from(a.1.max(b.1)),
        width,
        height,
    )
}

/// The largest `|dx|` inside the ellipse of radii `a` and `b` at the row `dy`; `None` if the row
/// has no pixel of it.
fn extent(a: i64, b: i64, dy: i64) -> Option<i64> {
    if dy.abs() > b {
        return None;
    }
    match (a, b) {
        (0, _) => Some(0),
        (_, 0) => Some(a),
        _ => {
            let (a, b, dy) = (i128::from(a), i128::from(b), i128::from(dy));
            let num = a * b * (2 * a * b + a + b) - 2 * a * a * dy * dy;
            if num <= 0 {
                return None;
            }
            // Strict: `2b²dx² < num`, that is `dx² ≤ ⌊(num − 1) / 2b²⌋`.
            let reach = isqrt((num - 1) / (2 * b * b));
            Some(reach.min(a) as i64)
        }
    }
}

fn clamp_radius(radius: u32) -> i64 {
    i64::from(radius.min(MAX_RADIUS))
}

/// The filled ellipse (or circle) of radii `a` and `b` around `center`, clipped to the canvas: a
/// pixel at `(dx, dy)` is inside when `|dx| ≤ a`, `|dy| ≤ b` and
/// `2(b²dx² + a²dy²) < ab(2ab + a + b)`.
pub fn ellipse_spans(center: (i32, i32), a: u32, b: u32, width: u32, height: u32) -> Vec<Span> {
    let (a, b) = (clamp_radius(a), clamp_radius(b));
    let (cx, cy) = (i64::from(center.0), i64::from(center.1));
    let y0 = (cy - b).max(0);
    let y1 = (cy + b).min(i64::from(height) - 1);
    let mut spans = Vec::new();
    for y in y0..=y1 {
        let Some(e) = extent(a, b, y - cy) else {
            continue;
        };
        let xs = (cx - e).max(0);
        let xe = (cx + e).min(i64::from(width) - 1);
        if xs <= xe {
            spans.push(Span {
                y: y as u32,
                x_start: xs as u32,
                x_end: (xe + 1) as u32,
            });
        }
    }
    spans
}

/// The vertices as a closed path: every side, the last one back to the first. With two vertices it
/// is the one side; with one, a point.
fn sides(vertices: &[(i32, i32)]) -> Vec<((i32, i32), (i32, i32))> {
    let n = vertices.len();
    match n {
        0 => Vec::new(),
        1 => vec![(vertices[0], vertices[0])],
        2 => vec![(vertices[0], vertices[1])],
        _ => (0..n)
            .map(|i| (vertices[i], vertices[(i + 1) % n]))
            .collect(),
    }
}

/// `⌈num / den⌉` for `den > 0`.
fn ceil_div(num: i128, den: i128) -> i128 {
    num.div_euclid(den) + i128::from(num.rem_euclid(den) != 0)
}

/// The filled polygon (even-odd rule) plus the outline of its sides, clipped to the canvas.
///
/// A pixel `(x, y)` is inside if an odd number of sides cross the row `y` (sides with
/// `min(y0, y1) ≤ y < max(y0, y1)`) at an `x` greater than its own; the sides drawn with [`Line`]
/// are added, so the fill always contains its border. The closing side goes from the last vertex
/// to the first. With fewer than three vertices it is the line or the point.
pub fn polygon_spans(vertices: &[(i32, i32)], width: u32, height: u32) -> Vec<Span> {
    let sides = sides(vertices);
    let mut rows: BTreeMap<u32, Vec<(u32, u32)>> = BTreeMap::new();
    if let (Some(&min_y), Some(&max_y)) = (
        vertices.iter().map(|v| v.1).min().as_ref(),
        vertices.iter().map(|v| v.1).max().as_ref(),
    ) {
        let y0 = i64::from(min_y).max(0);
        let y1 = i64::from(max_y).min(i64::from(height) - 1);
        let mut crossings: Vec<i128> = Vec::new();
        for y in y0..=y1 {
            crossings.clear();
            for &(p, q) in &sides {
                let (py, qy) = (i64::from(p.1), i64::from(q.1));
                if (py <= y && y < qy) || (qy <= y && y < py) {
                    let num = i128::from(y - py) * i128::from(i64::from(q.0) - i64::from(p.0));
                    let den = i128::from(qy - py);
                    let (num, den) = if den < 0 { (-num, -den) } else { (num, den) };
                    crossings.push(i128::from(p.0) + ceil_div(num, den));
                }
            }
            crossings.sort_unstable();
            for pair in crossings.chunks(2) {
                // The count of crossings of a closed path is even; `[a, b]` is a full pair.
                let &[first, second] = pair else {
                    continue;
                };
                let start = first.max(0);
                let end = second.min(i128::from(width));
                if start < end {
                    rows.entry(y as u32)
                        .or_default()
                        .push((start as u32, end as u32));
                }
            }
        }
    }
    for &(p, q) in &sides {
        for (x, y) in Line::new(p, q).clip_to(width, height) {
            if let (Ok(x), Ok(y)) = (u32::try_from(x), u32::try_from(y)) {
                rows.entry(y).or_default().push((x, x + 1));
            }
        }
    }
    let mut spans = Vec::new();
    for (y, mut ranges) in rows {
        ranges.sort_unstable();
        let mut current = ranges[0];
        for &(s, e) in &ranges[1..] {
            if s <= current.1 {
                current.1 = current.1.max(e);
            } else {
                spans.push(Span {
                    y,
                    x_start: current.0,
                    x_end: current.1,
                });
                current = (s, e);
            }
        }
        spans.push(Span {
            y,
            x_start: current.0,
            x_end: current.1,
        });
    }
    spans
}

/// The window around the canvas that rings are clipped to: `[-margin, size + margin)`.
fn window(width: u32, height: u32, margin: u32) -> (i64, i64, i64, i64) {
    let m = i64::from(margin);
    (-m, i64::from(width) + m - 1, -m, i64::from(height) + m - 1)
}

/// The ring of the perimeter of the box with corners `a` and `b`: from the top left corner,
/// clockwise, no repeats (a point or a line if a side has one pixel), keeping only the points
/// within `margin` pixels of the canvas.
pub fn rect_ring(
    a: (i32, i32),
    b: (i32, i32),
    width: u32,
    height: u32,
    margin: u32,
) -> Vec<(i32, i32)> {
    let (x0, x1) = (i64::from(a.0.min(b.0)), i64::from(a.0.max(b.0)));
    let (y0, y1) = (i64::from(a.1.min(b.1)), i64::from(a.1.max(b.1)));
    let (wx0, wx1, wy0, wy1) = window(width, height, margin);
    let mut ring = Vec::new();
    let mut push = |x: i64, y: i64| ring.push((x as i32, y as i32));
    // Top side, left to right.
    if (wy0..=wy1).contains(&y0) {
        for x in x0.max(wx0)..=x1.min(wx1) {
            push(x, y0);
        }
    }
    // Right side, downwards.
    if y1 > y0 && (wx0..=wx1).contains(&x1) {
        for y in (y0 + 1).max(wy0)..=y1.min(wy1) {
            push(x1, y);
        }
    }
    // Bottom side, right to left.
    if y1 > y0 && x1 > x0 && (wy0..=wy1).contains(&y1) {
        for x in (x0.max(wx0)..=(x1 - 1).min(wx1)).rev() {
            push(x, y1);
        }
    }
    // Left side, upwards.
    if x1 > x0 && y1 - 1 > y0 && (wx0..=wx1).contains(&x0) {
        for y in ((y0 + 1).max(wy0)..=(y1 - 1).min(wy1)).rev() {
            push(x0, y);
        }
    }
    ring
}

/// Orders `(dx, dy)` by the angle from the east, clockwise on the screen (`y` grows downwards),
/// exactly; the nearer first at the same angle.
fn by_angle(p: (i64, i64), q: (i64, i64)) -> Ordering {
    let half = |v: (i64, i64)| u8::from(!(v.1 > 0 || (v.1 == 0 && v.0 > 0)));
    half(p).cmp(&half(q)).then_with(|| {
        let cross = i128::from(p.0) * i128::from(q.1) - i128::from(p.1) * i128::from(q.0);
        match cross.cmp(&0) {
            Ordering::Greater => Ordering::Less,
            Ordering::Less => Ordering::Greater,
            Ordering::Equal => {
                let dist = |v: (i64, i64)| {
                    i128::from(v.0) * i128::from(v.0) + i128::from(v.1) * i128::from(v.1)
                };
                dist(p).cmp(&dist(q))
            }
        }
    })
}

/// The outline of the ellipse (the pixels of [`ellipse_spans`] with a 4-neighbour outside it) as a
/// ring ordered by the exact angle from the east, clockwise, keeping only the points within
/// `margin` pixels of the canvas.
pub fn ellipse_ring(
    center: (i32, i32),
    a: u32,
    b: u32,
    width: u32,
    height: u32,
    margin: u32,
) -> Vec<(i32, i32)> {
    let (a, b) = (clamp_radius(a), clamp_radius(b));
    let (cx, cy) = (i64::from(center.0), i64::from(center.1));
    let (wx0, wx1, wy0, wy1) = window(width, height, margin);
    let mut points: Vec<(i64, i64)> = Vec::new();
    for y in (cy - b).max(wy0)..=(cy + b).min(wy1) {
        let dy = y - cy;
        let Some(e) = extent(a, b, dy) else {
            continue;
        };
        // The interior of the row: the columns whose four neighbours are inside.
        let inner = (e - 1)
            .min(extent(a, b, dy - 1).unwrap_or(-1))
            .min(extent(a, b, dy + 1).unwrap_or(-1));
        let mut take = |from: i64, to: i64| {
            for dx in from.max(wx0 - cx)..=to.min(wx1 - cx) {
                points.push((dx, dy));
            }
        };
        if inner < 0 {
            take(-e, e);
        } else {
            take(-e, -inner - 1);
            take(inner + 1, e);
        }
    }
    points.sort_unstable_by(|&p, &q| by_angle(p, q));
    points
        .into_iter()
        .map(|(dx, dy)| ((cx + dx) as i32, (cy + dy) as i32))
        .collect()
}

/// The ring of the polygon: its sides one after another (the closing one included, without
/// repeating the first point), keeping only the points within `margin` pixels of the canvas. With
/// two vertices it is the one side.
pub fn polygon_ring(
    vertices: &[(i32, i32)],
    width: u32,
    height: u32,
    margin: u32,
) -> Vec<(i32, i32)> {
    let mut ring: Vec<(i32, i32)> = Vec::new();
    for (p, q) in sides(vertices) {
        for point in line_path_near(p, q, width, height, margin) {
            if ring.last() != Some(&point) {
                ring.push(point);
            }
        }
    }
    if ring.len() > 1 && ring.first() == ring.last() {
        ring.pop();
    }
    ring
}

/// The centers of the stamps along a closed ring: all of them (continuous), the indices
/// `0, n, 2n…` (`EveryN`) or `k` centers at `⌊i·L/k⌋` (`TotalN`, `k` at most the length, so the
/// first is not repeated at the end). `n` and `k` of 0 are taken as 1.
pub fn spaced_centers_closed(ring: &[(i32, i32)], spacing: Spacing) -> Vec<(i32, i32)> {
    let len = ring.len();
    match spacing {
        _ if len == 0 => Vec::new(),
        Spacing::Continuous => ring.to_vec(),
        Spacing::EveryN(n) => ring.iter().copied().step_by(n.max(1) as usize).collect(),
        Spacing::TotalN(k) => {
            let k = (k.max(1) as usize).min(len);
            (0..k).map(|i| ring[i * len / k]).collect()
        }
    }
}
