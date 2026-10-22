//! Color HSL (matiz, saturación, luminosidad) y su conversión de ida y vuelta con `Rgba`.

use crate::Rgba;

/// Color en HSL sin alfa. `h` en `[0, 360)` grados; `s` y `l` en `[0, 1]`; nunca NaN.
///
/// Los campos son privados para sostener el invariante: `new` normaliza la entrada.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Hsl {
    h: f32,
    s: f32,
    l: f32,
}

impl Hsl {
    /// Normaliza la entrada: el matiz se envuelve a `[0, 360)` (no finito pasa a 0) y `s` y `l`
    /// se recortan a `[0, 1]` (NaN pasa a 0).
    pub fn new(h: f32, s: f32, l: f32) -> Hsl {
        Hsl {
            h: normalize_hue(h),
            s: clamp_unit(s),
            l: clamp_unit(l),
        }
    }

    pub fn h(&self) -> f32 {
        self.h
    }

    pub fn s(&self) -> f32 {
        self.s
    }

    pub fn l(&self) -> f32 {
        self.l
    }

    /// Convierte un color a HSL; el alfa se ignora.
    pub fn from_rgba(color: Rgba) -> Hsl {
        let (ri, gi, bi) = (color.r, color.g, color.b);
        let max_i = ri.max(gi).max(bi);
        let min_i = ri.min(gi).min(bi);
        let r = f32::from(ri) / 255.0;
        let g = f32::from(gi) / 255.0;
        let b = f32::from(bi) / 255.0;
        let max = f32::from(max_i) / 255.0;
        let min = f32::from(min_i) / 255.0;
        let d = max - min;
        let l = (max + min) / 2.0;
        if max_i == min_i {
            return Hsl::new(0.0, 0.0, l);
        }
        let s = d / (1.0 - (2.0 * l - 1.0).abs());
        // Con empate de máximos se prueba r, luego g, luego b.
        let sector = if ri == max_i {
            ((g - b) / d).rem_euclid(6.0)
        } else if gi == max_i {
            (b - r) / d + 2.0
        } else {
            (r - g) / d + 4.0
        };
        Hsl::new(60.0 * sector, s, l)
    }

    /// Convierte a RGB; el alfa se pasa aparte porque `Hsl` no lo guarda.
    pub fn to_rgba(self, alpha: u8) -> Rgba {
        let c = (1.0 - (2.0 * self.l - 1.0).abs()) * self.s;
        let hp = self.h / 60.0;
        let x = c * (1.0 - (hp % 2.0 - 1.0).abs());
        let m = self.l - c / 2.0;
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

fn normalize_hue(h: f32) -> f32 {
    if !h.is_finite() {
        return 0.0;
    }
    let wrapped = h.rem_euclid(360.0);
    // Un negativo diminuto puede redondear a 360.0 exacto.
    if wrapped >= 360.0 { 0.0 } else { wrapped }
}

fn clamp_unit(v: f32) -> f32 {
    if v.is_nan() { 0.0 } else { v.clamp(0.0, 1.0) }
}

/// Canal en `[0, 1]` a 8 bits: `channel · 255` redondeado con la mitad alejándose de cero.
fn to_u8(channel: f32) -> u8 {
    (channel * 255.0).round().clamp(0.0, 255.0) as u8
}
