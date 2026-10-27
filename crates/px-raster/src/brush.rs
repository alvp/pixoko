//! The brush and the geometry of strokes (specs/hito-4-pincel.md, unit 4.3): ten fixed shapes of
//! 1 to 100 px, stamping clipped to the canvas, spacing of the stamps along a path and the
//! quadratic curve. Pure integer functions: no `Document` here, the caller builds the command.

use std::collections::HashSet;

use crate::Line;

/// Smallest and largest size of a brush, in pixels.
pub const MIN_SIZE: u32 = 1;
pub const MAX_SIZE: u32 = 100;

/// The ten shapes of the brush.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BrushShape {
    Round,
    Square,
    Diamond,
    /// A round brush with a hole: a ring of 1 px.
    Ring,
    Horizontal,
    Vertical,
    /// The diagonal that goes up to the right (`/`).
    Slash,
    /// The diagonal that goes down to the right (`\`).
    Backslash,
    /// A straight cross (`+`).
    Plus,
    /// A diagonal cross (`X`).
    Cross,
}

impl BrushShape {
    /// All the shapes, in the order in which the options window cycles them.
    pub const ALL: [BrushShape; 10] = [
        BrushShape::Round,
        BrushShape::Square,
        BrushShape::Diamond,
        BrushShape::Ring,
        BrushShape::Horizontal,
        BrushShape::Vertical,
        BrushShape::Slash,
        BrushShape::Backslash,
        BrushShape::Plus,
        BrushShape::Cross,
    ];
}

/// A shape with a size.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Brush {
    shape: BrushShape,
    size: u32,
}

impl Brush {
    /// The size is clamped to `1..=100`.
    pub fn new(shape: BrushShape, size: u32) -> Brush {
        Brush {
            shape,
            size: size.clamp(MIN_SIZE, MAX_SIZE),
        }
    }

    pub fn shape(&self) -> BrushShape {
        self.shape
    }

    pub fn size(&self) -> u32 {
        self.size
    }

    /// The pixels of the mask relative to the center of the stamp, by rows (`y`, then `x`). The
    /// box is `size × size` (the bars, `size × 1` and `1 × size`) and the center is at
    /// `(⌊size/2⌋, ⌊size/2⌋)`.
    pub fn offsets(&self) -> Vec<(i32, i32)> {
        let n = i64::from(self.size);
        let f = n / 2;
        let mut out = Vec::new();
        for j in 0..n {
            for i in 0..n {
                if self.covers(i, j) {
                    out.push(((i - f) as i32, (j - f) as i32));
                }
            }
        }
        out
    }

    /// Whether the pixel `(i, j)` of the `n × n` box belongs to the mask.
    fn covers(&self, i: i64, j: i64) -> bool {
        let n = i64::from(self.size);
        let (dx, dy) = (2 * i + 1 - n, 2 * j + 1 - n);
        let round = |m: i64| dx * dx + dy * dy <= m * m - m;
        match self.shape {
            BrushShape::Round => round(n),
            BrushShape::Ring => round(n) && (n <= 2 || !round(n - 2)),
            BrushShape::Square => true,
            BrushShape::Diamond => dx.abs() + dy.abs() <= n,
            BrushShape::Horizontal => j == n / 2,
            BrushShape::Vertical => i == n / 2,
            BrushShape::Slash => i + j == n - 1,
            BrushShape::Backslash => i == j,
            BrushShape::Plus => dx.abs() <= 1 || dy.abs() <= 1,
            BrushShape::Cross => i == j || i + j == n - 1,
        }
    }
}

/// How the stamps are spread along a path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Spacing {
    /// One stamp per point of the path.
    Continuous,
    /// One stamp every `n` points, the first one included.
    EveryN(u32),
    /// `n` stamps in all, from the start to the end of the path.
    TotalN(u32),
}

/// The centers of the stamps along `path` (see [`Spacing`]). A count of 0 is taken as 1; with
/// more stamps than points, every point.
pub fn spaced_centers(path: &[(i32, i32)], spacing: Spacing) -> Vec<(i32, i32)> {
    match spacing {
        Spacing::Continuous => path.to_vec(),
        Spacing::EveryN(n) => path.iter().copied().step_by(n.max(1) as usize).collect(),
        Spacing::TotalN(k) => {
            let len = path.len();
            let k = (k.max(1) as usize).min(len);
            if k <= 1 {
                return path.iter().copied().take(k).collect();
            }
            (0..k).map(|i| path[i * (len - 1) / (k - 1)]).collect()
        }
    }
}

/// The pixels covered by stamping `brush` at every center, clipped to the `width × height`
/// canvas, without repeats and in the order of their first visit.
pub fn stamp_pixels(
    brush: Brush,
    centers: impl IntoIterator<Item = (i32, i32)>,
    width: u32,
    height: u32,
) -> Vec<(u32, u32)> {
    let offsets = brush.offsets();
    let (w, h) = (i64::from(width), i64::from(height));
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for (cx, cy) in centers {
        for &(dx, dy) in &offsets {
            let (x, y) = (i64::from(cx) + i64::from(dx), i64::from(cy) + i64::from(dy));
            if (0..w).contains(&x) && (0..h).contains(&y) {
                let pixel = (x as u32, y as u32);
                if seen.insert(pixel) {
                    out.push(pixel);
                }
            }
        }
    }
    out
}

/// The points of the segment from `a` to `b` (the same as [`Line`]), not clipped.
pub fn line_path(a: (i32, i32), b: (i32, i32)) -> Vec<(i32, i32)> {
    Line::new(a, b).collect()
}

/// The part of the segment from `a` to `b` that is within `margin` pixels of the `width × height`
/// canvas (a point farther away cannot be touched by a brush of `margin` pixels). It never
/// builds more points than that, however far the ends are.
pub fn line_path_near(
    a: (i32, i32),
    b: (i32, i32),
    width: u32,
    height: u32,
    margin: u32,
) -> Vec<(i32, i32)> {
    Line::new(a, b)
        .clip_with_margin(width, height, margin)
        .collect()
}

/// Most steps that [`curve_path`] takes; a longer control polygon degrades to its two ends.
pub const MAX_CURVE_STEPS: i64 = 1 << 22;

/// The quadratic curve from `a` to `b` with `control` as its control point, as consecutive
/// points (8-connected, no consecutive repeats).
///
/// `N` is the length of the control polygon in the Chebyshev distance (at least 1); for
/// `i = 0..=N` the point is `((N−i)²·a + 2i(N−i)·control + i²·b) / N²` per coordinate, rounded to
/// the nearest integer with halves up. Two consecutive points that are not neighbors are joined
/// with a [`Line`]. The curve of `a, b, c` and that of `b, a, c` are the same set of points.
pub fn curve_path(a: (i32, i32), b: (i32, i32), control: (i32, i32)) -> Vec<(i32, i32)> {
    let cheb = |p: (i32, i32), q: (i32, i32)| {
        i64::from(p.0.abs_diff(q.0)).max(i64::from(p.1.abs_diff(q.1)))
    };
    let n = (cheb(a, b) + cheb(a, control) + cheb(control, b)).max(1);
    if n > MAX_CURVE_STEPS {
        return vec![a, b];
    }
    let point = |i: i64| {
        let at = |a: i32, c: i32, b: i32| {
            let num = i128::from((n - i) * (n - i)) * i128::from(a)
                + i128::from(2 * i * (n - i)) * i128::from(c)
                + i128::from(i * i) * i128::from(b);
            let den = i128::from(n) * i128::from(n);
            (2 * num + den).div_euclid(2 * den) as i32
        };
        (at(a.0, control.0, b.0), at(a.1, control.1, b.1))
    };
    let mut out = vec![point(0)];
    for i in 1..=n {
        let next = point(i);
        let last = out[out.len() - 1];
        if next == last {
            continue;
        }
        if cheb(last, next) > 1 {
            out.extend(Line::new(last, next).skip(1));
        } else {
            out.push(next);
        }
    }
    out
}
