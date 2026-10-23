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
    pub fn new(_h: f32, _s: f32, _v: f32) -> Hsv {
        todo!("C4: Hsv::new")
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
    pub fn from_rgba(_color: Rgba) -> Hsv {
        todo!("C4: Hsv::from_rgba")
    }

    /// Converts back to RGB; the alpha is passed apart because `Hsv` does not keep it.
    pub fn to_rgba(self, _alpha: u8) -> Rgba {
        todo!("C4: Hsv::to_rgba")
    }
}

/// The `Hsv` after editing RGB: the one of `from_rgba`, except gray (keeps the previous hue) and
/// black (keeps the previous hue and saturation).
pub fn hsv_after_rgb(_previous: Hsv, _new: Rgba) -> Hsv {
    todo!("C4: hsv_after_rgb")
}
