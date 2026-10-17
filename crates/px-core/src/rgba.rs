//! Color RGBA de 8 bits por canal, con alfa directo (no premultiplicado).

use std::error::Error;
use std::fmt;
use std::str::FromStr;

/// Color con alfa directo: `r`, `g` y `b` no dependen de `a`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Rgba {
    /// Totalmente transparente: `(0, 0, 0, 0)`.
    pub const TRANSPARENT: Rgba = Rgba::new(0, 0, 0, 0);

    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Rgba {
        Rgba { r, g, b, a }
    }

    /// Color opaco (`a = 255`).
    pub const fn opaque(r: u8, g: u8, b: u8) -> Rgba {
        Rgba::new(r, g, b, 255)
    }

    /// Lee `rgb`, `rgba`, `rrggbb` o `rrggbbaa`, con un `#` inicial opcional.
    pub fn from_hex(s: &str) -> Result<Rgba, ParseHexError> {
        let digits = s.strip_prefix('#').unwrap_or(s);
        let len = digits.chars().count();
        if !matches!(len, 3 | 4 | 6 | 8) {
            return Err(ParseHexError::Length(len));
        }
        let mut values = [0u8; 8];
        for (value, ch) in values.iter_mut().zip(digits.chars()) {
            *value = ch.to_digit(16).ok_or(ParseHexError::InvalidDigit(ch))? as u8;
        }
        let values = &values[..len];
        let (r, g, b, a) = if len <= 4 {
            let channel = |i: usize| values[i] * 17;
            (
                channel(0),
                channel(1),
                channel(2),
                if len == 4 { channel(3) } else { 255 },
            )
        } else {
            let channel = |i: usize| values[2 * i] * 16 + values[2 * i + 1];
            (
                channel(0),
                channel(1),
                channel(2),
                if len == 8 { channel(3) } else { 255 },
            )
        };
        Ok(Rgba::new(r, g, b, a))
    }
}

impl fmt::Display for Rgba {
    /// Forma canónica: `#rrggbb` si es opaco y `#rrggbbaa` en otro caso.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{:02x}{:02x}{:02x}", self.r, self.g, self.b)?;
        if self.a != 255 {
            write!(f, "{:02x}", self.a)?;
        }
        Ok(())
    }
}

impl FromStr for Rgba {
    type Err = ParseHexError;

    fn from_str(s: &str) -> Result<Rgba, ParseHexError> {
        Rgba::from_hex(s)
    }
}

/// Error al leer un color hexadecimal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParseHexError {
    /// Número de caracteres, sin el `#` inicial, distinto de 3, 4, 6 y 8.
    Length(usize),
    /// Primer carácter que no es un dígito hexadecimal ASCII.
    InvalidDigit(char),
}

impl fmt::Display for ParseHexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseHexError::Length(n) => {
                write!(
                    f,
                    "se esperaban 3, 4, 6 u 8 dígitos hexadecimales y hay {n}"
                )
            }
            ParseHexError::InvalidDigit(c) => write!(f, "{c:?} no es un dígito hexadecimal"),
        }
    }
}

impl Error for ParseHexError {}
