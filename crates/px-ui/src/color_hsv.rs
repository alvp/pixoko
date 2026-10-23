//! HSV model of the color editor, kept next to the `Hsl` of 11c (specs/ui-color-block.md,
//! decision 10). The `Rgba` is always the real color; this one is only the auxiliary that keeps
//! the hue from jumping when the square passes through gray or black.

use px_core::Rgba;

/// A color in HSV without alpha: `h` in `[0, 360)` degrees, `s` and `v` in `[0, 1]`, never NaN.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Hsv {
    h: f32,
    s: f32,
    v: f32,
}

impl Hsv {
    /// Normalizes like `Hsl::new`: the hue wraps into `[0, 360)` (not finite becomes 0) and `s`
    /// and `v` are clamped to `[0, 1]` (NaN becomes 0).
    pub fn new(h: f32, s: f32, v: f32) -> Hsv {
        Hsv {
            h: normalize_hue(h),
            s: clamp_unit(s),
            v: clamp_unit(v),
        }
    }

    pub fn h(&self) -> f32 {
        self.h
    }

    pub fn s(&self) -> f32 {
        self.s
    }

    pub fn v(&self) -> f32 {
        self.v
    }

    /// Converts a color to HSV; the alpha is ignored.
    pub fn from_rgba(color: Rgba) -> Hsv {
        let (ri, gi, bi) = (color.r, color.g, color.b);
        let max_i = ri.max(gi).max(bi);
        let min_i = ri.min(gi).min(bi);
        let (r, g, b) = (
            f32::from(ri) / 255.0,
            f32::from(gi) / 255.0,
            f32::from(bi) / 255.0,
        );
        let max = f32::from(max_i) / 255.0;
        let d = max - f32::from(min_i) / 255.0;
        if max_i == min_i {
            return Hsv::new(0.0, 0.0, max);
        }
        // With a tie of maxima r is tried first, then g, then b (like `Hsl::from_rgba`).
        let sector = if ri == max_i {
            ((g - b) / d).rem_euclid(6.0)
        } else if gi == max_i {
            (b - r) / d + 2.0
        } else {
            (r - g) / d + 4.0
        };
        Hsv::new(60.0 * sector, d / max, max)
    }

    /// Converts back to RGB; the alpha is passed apart because `Hsv` does not keep it.
    pub fn to_rgba(self, alpha: u8) -> Rgba {
        let c = self.v * self.s;
        let hp = self.h / 60.0;
        let x = c * (1.0 - (hp % 2.0 - 1.0).abs());
        let m = self.v - c;
        let (r, g, b) = match (hp.floor() as usize).min(5) {
            0 => (c, x, 0.0),
            1 => (x, c, 0.0),
            2 => (0.0, c, x),
            3 => (0.0, x, c),
            4 => (x, 0.0, c),
            _ => (c, 0.0, x),
        };
        Rgba::new(to_u8(r + m), to_u8(g + m), to_u8(b + m), alpha)
    }
}

/// The `Hsv` after editing RGB: the one of `from_rgba`, except gray (keeps the previous hue) and
/// black (keeps the previous hue and saturation).
pub fn hsv_after_rgb(previous: Hsv, new: Rgba) -> Hsv {
    let hsv = Hsv::from_rgba(new);
    if new.r == new.g && new.g == new.b {
        let s = if new.r == 0 { previous.s() } else { 0.0 };
        Hsv::new(previous.h(), s, hsv.v())
    } else {
        hsv
    }
}

fn normalize_hue(h: f32) -> f32 {
    if !h.is_finite() {
        return 0.0;
    }
    let wrapped = h.rem_euclid(360.0);
    // A tiny negative can round to exactly 360.0.
    if wrapped >= 360.0 { 0.0 } else { wrapped }
}

fn clamp_unit(v: f32) -> f32 {
    if v.is_nan() { 0.0 } else { v.clamp(0.0, 1.0) }
}

/// A channel in `[0, 1]` to 8 bits: `channel · 255` rounded half away from zero.
fn to_u8(channel: f32) -> u8 {
    (channel * 255.0).round().clamp(0.0, 255.0) as u8
}
