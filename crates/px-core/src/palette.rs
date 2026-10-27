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

    /// Makes room for `count` more entries without allocating later; `false` if there is no memory.
    pub(crate) fn reserve(&mut self, count: usize) -> bool {
        self.colors.try_reserve(count).is_ok()
    }

    /// Appends `count` entries of color `color`. `false`, touching nothing, if they do not fit.
    pub(crate) fn extend(&mut self, count: usize, color: Rgba) -> bool {
        if self.colors.len().saturating_add(count) > Palette::MAX_COLORS {
            return false;
        }
        self.colors.resize(self.colors.len() + count, color);
        true
    }

    /// Cuts the palette to `len` entries (at most its length). If the transparent index was
    /// `len` or more, there is none any more.
    pub(crate) fn truncate(&mut self, len: usize) {
        self.colors.truncate(len);
        if self
            .transparent_index
            .is_some_and(|t| usize::from(t) >= self.colors.len())
        {
            self.transparent_index = None;
        }
    }

    /// Undoes `truncate`: gives back at the end the colors `removed` and sets the transparent index
    /// as given. `false`, touching nothing, if they do not fit or the index would not exist after.
    pub(crate) fn restore(&mut self, removed: &[Rgba], transparent: Option<u8>) -> bool {
        let len = self.colors.len() + removed.len();
        if len > Palette::MAX_COLORS || transparent.is_some_and(|t| usize::from(t) >= len) {
            return false;
        }
        self.colors.extend_from_slice(removed);
        self.transparent_index = transparent;
        true
    }

    /// Quita la última entrada. Si era la transparente, el índice transparente se quita.
    pub(crate) fn pop(&mut self) -> Option<Rgba> {
        let color = self.colors.pop()?;
        if self
            .transparent_index
            .is_some_and(|t| usize::from(t) >= self.colors.len())
        {
            self.transparent_index = None;
        }
        Some(color)
    }

    /// Reordena las entradas: la entrada `i` pasa a la posición `forward[i]`, y el índice
    /// transparente la sigue. Sin tocar nada y `false` si `forward` no es una permutación de
    /// `0..len`.
    pub(crate) fn permute(&mut self, forward: &[u8]) -> bool {
        if !is_permutation(forward, self.colors.len()) {
            return false;
        }
        let mut old = [Rgba::default(); Palette::MAX_COLORS];
        for (slot, color) in old.iter_mut().zip(&self.colors) {
            *slot = *color;
        }
        for (color, &to) in old.iter().zip(forward) {
            if let Some(slot) = self.colors.get_mut(usize::from(to)) {
                *slot = *color;
            }
        }
        self.transparent_index = self
            .transparent_index
            .and_then(|t| forward.get(usize::from(t)).copied());
        true
    }

    /// Quita una entrada y ajusta el índice transparente: desaparece si era la quitada y baja
    /// un lugar si estaba detrás.
    pub(crate) fn remove_entry(&mut self, index: u8) -> Option<Rgba> {
        if usize::from(index) >= self.colors.len() {
            return None;
        }
        let color = self.colors.remove(usize::from(index));
        self.transparent_index = match self.transparent_index {
            Some(t) if t == index => None,
            Some(t) if t > index => Some(t - 1),
            other => other,
        };
        Some(color)
    }

    /// Inserta una entrada en `index` (como mucho `len`) y fija el índice transparente tal
    /// cual se da. `false`, sin tocar nada, si no cabe o algún índice no existe después.
    pub(crate) fn insert_entry(&mut self, index: u8, color: Rgba, transparent: Option<u8>) -> bool {
        let len = self.colors.len();
        if len >= Palette::MAX_COLORS
            || usize::from(index) > len
            || transparent.is_some_and(|t| usize::from(t) > len)
        {
            return false;
        }
        self.colors.insert(usize::from(index), color);
        self.transparent_index = transparent;
        true
    }

    /// Removes the entries `indices` (strictly increasing, all existing, not every entry). The
    /// transparent index is dropped if it was removed and lowered by the removed entries before it
    /// otherwise. `false`, touching nothing, with invalid arguments.
    pub(crate) fn remove_entries(&mut self, indices: &[u8]) -> bool {
        let len = self.colors.len();
        if indices.is_empty()
            || indices.len() >= len
            || indices.windows(2).any(|pair| pair[0] >= pair[1])
            || indices.last().is_some_and(|&last| usize::from(last) >= len)
        {
            return false;
        }
        let mut position = 0usize;
        self.colors.retain(|_| {
            let drop = indices.binary_search(&(position as u8)).is_ok();
            position += 1;
            !drop
        });
        self.transparent_index = match self.transparent_index {
            Some(t) if indices.binary_search(&t).is_ok() => None,
            Some(t) => {
                let below = indices.iter().filter(|&&i| i < t).count();
                Some(t - below as u8)
            }
            None => None,
        };
        true
    }

    /// Undoes `remove_entries`: puts `colors[k]` back at the final position `indices[k]` and sets
    /// the transparent index as given. `false`, touching nothing, if it does not fit.
    pub(crate) fn insert_entries(
        &mut self,
        indices: &[u8],
        colors: &[Rgba],
        transparent: Option<u8>,
    ) -> bool {
        let len = self.colors.len() + indices.len();
        if indices.len() != colors.len()
            || len > Palette::MAX_COLORS
            || indices.windows(2).any(|pair| pair[0] >= pair[1])
            || indices.last().is_some_and(|&last| usize::from(last) >= len)
            || transparent.is_some_and(|t| usize::from(t) >= len)
        {
            return false;
        }
        for (&index, &color) in indices.iter().zip(colors) {
            self.colors.insert(usize::from(index), color);
        }
        self.transparent_index = transparent;
        true
    }

    /// Rota hacia índices mayores (`right`) o menores `k` posiciones las entradas
    /// `start..start + len`. El índice transparente no se mueve. `false`, sin tocar nada, si el
    /// rango se sale de la paleta.
    pub(crate) fn rotate_range(&mut self, start: u8, len: u16, k: u16, right: bool) -> bool {
        let start = usize::from(start);
        let Some(end) = start.checked_add(usize::from(len)) else {
            return false;
        };
        let Some(range) = self.colors.get_mut(start..end) else {
            return false;
        };
        if range.is_empty() {
            return true;
        }
        let k = usize::from(k) % range.len();
        if right {
            range.rotate_right(k);
        } else {
            range.rotate_left(k);
        }
        true
    }

    fn out_of_range(&self, index: u8) -> PaletteError {
        PaletteError::IndexOutOfRange {
            index,
            len: self.colors.len(),
        }
    }
}

/// `true` si `table` tiene `len` elementos y contiene cada valor de `0..len` exactamente una vez.
pub(crate) fn is_permutation(table: &[u8], len: usize) -> bool {
    if table.len() != len || len > Palette::MAX_COLORS {
        return false;
    }
    let mut seen = [false; Palette::MAX_COLORS];
    table.iter().all(|&value| {
        seen.get_mut(usize::from(value))
            .is_some_and(|seen| !std::mem::replace(seen, true))
            && usize::from(value) < len
    })
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
