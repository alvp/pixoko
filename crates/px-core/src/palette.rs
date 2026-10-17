//! Paleta del modo indexado: hasta 256 colores `Rgba` y un índice transparente opcional.

use std::error::Error;
use std::fmt;

use crate::Rgba;

/// Lista ordenada de hasta 256 colores, cada uno con su propio alfa.
///
/// Invariante: `transparent_index` es `None` o apunta a una entrada existente.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Palette {
    colors: Vec<Rgba>,
    transparent_index: Option<u8>,
}

impl Palette {
    /// Número máximo de entradas.
    pub const MAX_COLORS: usize = 256;

    /// Paleta vacía, sin índice transparente.
    pub fn new() -> Palette {
        Palette::default()
    }

    /// Crea una paleta con los colores dados, en orden. Lee como mucho 257 elementos.
    pub fn from_colors(colors: impl IntoIterator<Item = Rgba>) -> Result<Palette, PaletteError> {
        let colors: Vec<Rgba> = colors.into_iter().take(Self::MAX_COLORS + 1).collect();
        if colors.len() > Self::MAX_COLORS {
            return Err(PaletteError::TooManyColors);
        }
        Ok(Palette {
            colors,
            transparent_index: None,
        })
    }

    pub fn len(&self) -> usize {
        self.colors.len()
    }

    pub fn is_empty(&self) -> bool {
        self.colors.is_empty()
    }

    pub fn colors(&self) -> &[Rgba] {
        &self.colors
    }

    /// Color tal como está guardado.
    pub fn get(&self, index: u8) -> Result<Rgba, PaletteError> {
        self.colors
            .get(usize::from(index))
            .copied()
            .ok_or_else(|| self.out_of_range(index))
    }

    /// Color efectivo: el guardado, salvo en el índice transparente, donde `a = 0`.
    pub fn resolved(&self, index: u8) -> Result<Rgba, PaletteError> {
        let color = self.get(index)?;
        if self.transparent_index == Some(index) {
            Ok(Rgba { a: 0, ..color })
        } else {
            Ok(color)
        }
    }

    /// Añade un color al final y devuelve el índice de la entrada nueva.
    pub fn push(&mut self, color: Rgba) -> Result<u8, PaletteError> {
        let index = u8::try_from(self.colors.len()).map_err(|_| PaletteError::TooManyColors)?;
        self.colors.push(color);
        Ok(index)
    }

    /// Sustituye el color de una entrada.
    pub fn set(&mut self, index: u8, color: Rgba) -> Result<(), PaletteError> {
        let len = self.colors.len();
        let slot = self
            .colors
            .get_mut(usize::from(index))
            .ok_or(PaletteError::IndexOutOfRange { index, len })?;
        *slot = color;
        Ok(())
    }

    pub fn transparent_index(&self) -> Option<u8> {
        self.transparent_index
    }

    /// Fija o quita el índice transparente; si es `Some`, debe existir.
    pub fn set_transparent_index(&mut self, index: Option<u8>) -> Result<(), PaletteError> {
        if let Some(i) = index
            && usize::from(i) >= self.colors.len()
        {
            return Err(self.out_of_range(i));
        }
        self.transparent_index = index;
        Ok(())
    }

    /// Índice de la entrada más parecida a `color`, o `None` si la paleta está vacía.
    ///
    /// Compara con el valor resuelto de cada entrada. Con empate gana el índice más
    /// bajo. Distancia y motivos: `specs/hito-2-color-cercano.md`.
    pub fn nearest(&self, color: Rgba) -> Option<u8> {
        let target = premultiplied(color);
        let mut best: Option<(i64, u8)> = None;
        for (i, &stored) in self.colors.iter().enumerate() {
            // Como mucho 256 entradas: el índice siempre cabe en `u8`.
            let index = u8::try_from(i).ok()?;
            let entry = if self.transparent_index == Some(index) {
                Rgba { a: 0, ..stored }
            } else {
                stored
            };
            let distance = squared_distance(premultiplied(entry), target);
            // Solo una distancia estrictamente menor desplaza a la mejor.
            if best.is_none_or(|(d, _)| distance < d) {
                best = Some((distance, index));
                if distance == 0 {
                    break;
                }
            }
        }
        best.map(|(_, index)| index)
    }

    fn out_of_range(&self, index: u8) -> PaletteError {
        PaletteError::IndexOutOfRange {
            index,
            len: self.colors.len(),
        }
    }
}

/// Color premultiplicado por el alfa: `(r·a, g·a, b·a, 255·a)`, cada componente en `[0, 65025]`.
fn premultiplied(color: Rgba) -> [i64; 4] {
    let a = i64::from(color.a);
    [
        i64::from(color.r) * a,
        i64::from(color.g) * a,
        i64::from(color.b) * a,
        255 * a,
    ]
}

/// Distancia euclídea al cuadrado; como máximo `4 · 65025²`, sin riesgo en `i64`.
fn squared_distance(p: [i64; 4], q: [i64; 4]) -> i64 {
    p.iter()
        .zip(q)
        .map(|(&x, y)| {
            let d = x - y;
            d * d
        })
        .sum()
}

/// Error al operar con una paleta.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PaletteError {
    /// Se intentó superar [`Palette::MAX_COLORS`] entradas.
    TooManyColors,
    /// El índice es mayor o igual que la longitud de la paleta.
    IndexOutOfRange { index: u8, len: usize },
}

impl fmt::Display for PaletteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PaletteError::TooManyColors => {
                write!(
                    f,
                    "una paleta admite como máximo {} colores",
                    Palette::MAX_COLORS
                )
            }
            PaletteError::IndexOutOfRange { index, len } => {
                write!(
                    f,
                    "índice {index} fuera de rango en una paleta de {len} colores"
                )
            }
        }
    }
}

impl Error for PaletteError {}
