//! Imagen de un solo plano en modo RGBA, indexado, grises o grises con alfa.

use std::error::Error;
use std::fmt;

use crate::{Palette, Rgba};

/// Modo de color de una imagen.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Mode {
    Rgba,
    Indexed,
    Gray,
    GrayAlpha,
}

impl Mode {
    /// Bytes que ocupa un píxel en el almacenamiento de [`Image`]: RGBA 4, grises con alfa 2,
    /// indexado y grises 1. Ver la tabla de [`Image::bytes`].
    pub fn bytes_per_pixel(self) -> usize {
        match self {
            Mode::Rgba => 4,
            Mode::GrayAlpha => 2,
            Mode::Indexed | Mode::Gray => 1,
        }
    }
}

/// Un píxel, con el tipo de su modo.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Pixel {
    /// Color con alfa directo.
    Rgba(Rgba),
    /// Índice en la paleta de la imagen.
    Indexed(u8),
    /// Gris opaco.
    Gray(u8),
    /// Gris con alfa directo.
    GrayAlpha { gray: u8, alpha: u8 },
}

impl Pixel {
    fn mode(self) -> Mode {
        match self {
            Pixel::Rgba(_) => Mode::Rgba,
            Pixel::Indexed(_) => Mode::Indexed,
            Pixel::Gray(_) => Mode::Gray,
            Pixel::GrayAlpha { .. } => Mode::GrayAlpha,
        }
    }
}

/// Rectángulo de píxeles de un modo.
///
/// Invariantes: `width >= 1` y `height >= 1`; `data.len()` es
/// `width * height * bytes_por_píxel` y no supera [`Image::MAX_BYTES`]; `palette` es `Some`
/// si y solo si el modo es `Indexed`, y entonces no está vacía y todo píxel es menor que su
/// longitud.
///
/// Los píxeles se guardan en un solo `Vec<u8>`, fila a fila (la de arriba primero), sin relleno
/// entre filas. Bytes por píxel: RGBA `r, g, b, a`; grises con alfa `gray, alpha`; indexado y
/// grises, uno.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Image {
    width: u32,
    height: u32,
    mode: Mode,
    data: Vec<u8>,
    palette: Option<Palette>,
}

impl Image {
    /// Tamaño máximo de los píxeles, en bytes (1 GiB).
    pub const MAX_BYTES: usize = 1 << 30;

    /// Tamaño en bytes de los píxeles de una imagen de ese tamaño y modo, sin reservar memoria.
    ///
    /// Error si alguna dimensión es cero o si el tamaño desborda o supera [`Image::MAX_BYTES`].
    pub fn byte_len(width: u32, height: u32, mode: Mode) -> Result<usize, ImageError> {
        if width == 0 || height == 0 {
            return Err(ImageError::ZeroSize { width, height });
        }
        let too_large = || ImageError::TooLarge { width, height };
        usize::try_from(width)
            .ok()
            .and_then(|w| w.checked_mul(usize::try_from(height).ok()?))
            .and_then(|pixels| pixels.checked_mul(mode.bytes_per_pixel()))
            .filter(|&bytes| bytes <= Image::MAX_BYTES)
            .ok_or_else(too_large)
    }

    /// Imagen a ceros en un modo que no necesita paleta; con `Mode::Indexed` usa [`Image::new_indexed`].
    pub fn new(width: u32, height: u32, mode: Mode) -> Result<Image, ImageError> {
        let len = Image::byte_len(width, height, mode)?;
        if mode == Mode::Indexed {
            return Err(ImageError::PaletteRequired);
        }
        Image::with_data(width, height, mode, len, None)
    }

    /// Imagen indexada con todos los píxeles en el índice 0; la paleta no puede estar vacía.
    pub fn new_indexed(width: u32, height: u32, palette: Palette) -> Result<Image, ImageError> {
        let len = Image::byte_len(width, height, Mode::Indexed)?;
        if palette.is_empty() {
            return Err(ImageError::EmptyPalette);
        }
        Image::with_data(width, height, Mode::Indexed, len, Some(palette))
    }

    fn with_data(
        width: u32,
        height: u32,
        mode: Mode,
        len: usize,
        palette: Option<Palette>,
    ) -> Result<Image, ImageError> {
        let mut data = Vec::new();
        data.try_reserve_exact(len)
            .map_err(|_| ImageError::TooLarge { width, height })?;
        data.resize(len, 0);
        Ok(Image {
            width,
            height,
            mode,
            data,
            palette,
        })
    }

    /// Imagen a partir de sus bytes ya construidos. Quien llama garantiza las invariantes:
    /// `data.len()` es el de `Image::byte_len` y `palette` es `Some` solo en modo indexado, con
    /// todo píxel menor que su longitud (solo lo usa la conversión de modos).
    pub(crate) fn from_raw(
        width: u32,
        height: u32,
        mode: Mode,
        data: Vec<u8>,
        palette: Option<Palette>,
    ) -> Image {
        Image {
            width,
            height,
            mode,
            data,
            palette,
        }
    }

    /// Bytes de píxeles más 4 por entrada de paleta: lo que ocupa la imagen en la historia.
    pub(crate) fn content_bytes(&self) -> usize {
        let palette = self.palette.as_ref().map_or(0, Palette::len);
        self.data
            .len()
            .saturating_add(palette.saturating_mul(size_of::<Rgba>()))
    }

    // Edición de la paleta (specs/hito-2-paleta-editar.md, punto 11). Son la única vía para
    // cambiar la paleta de una imagen sin romper su invariante: cada método deja todo píxel
    // menor que la longitud de la paleta y el índice transparente existente. Con argumentos
    // inválidos devuelven `false` sin tocar nada; ninguno entra en pánico.

    /// Cambia el color guardado de una entrada.
    pub(crate) fn set_palette_entry(&mut self, index: u8, color: Rgba) -> bool {
        self.palette
            .as_mut()
            .is_some_and(|palette| palette.set(index, color).is_ok())
    }

    /// Fija o quita el índice transparente.
    pub(crate) fn set_transparent(&mut self, index: Option<u8>) -> bool {
        self.palette
            .as_mut()
            .is_some_and(|palette| palette.set_transparent_index(index).is_ok())
    }

    /// Añade una entrada al final. Los píxeles no cambian.
    pub(crate) fn push_palette_entry(&mut self, color: Rgba) -> bool {
        self.palette
            .as_mut()
            .is_some_and(|palette| palette.push(color).is_ok())
    }

    /// Quita la última entrada, si ningún píxel la usa y no es la única.
    pub(crate) fn pop_palette_entry(&mut self) -> bool {
        let Some(palette) = self.palette.as_mut() else {
            return false;
        };
        let Some(last) = palette.len().checked_sub(1) else {
            return false;
        };
        let used = u8::try_from(last)
            .map(|last| self.data.contains(&last))
            .unwrap_or(true);
        if used || last == 0 {
            return false;
        }
        palette.pop().is_some()
    }

    /// Makes room for `count` more palette entries without allocating later.
    pub(crate) fn reserve_palette_slots(&mut self, count: usize) -> bool {
        self.palette
            .as_mut()
            .is_some_and(|palette| palette.reserve(count))
    }

    /// Appends `count` opaque black entries. The pixels and the transparent index do not
    /// change.
    pub(crate) fn extend_palette(&mut self, count: usize) -> bool {
        self.palette
            .as_mut()
            .is_some_and(|palette| palette.extend(count, Rgba::opaque(0, 0, 0)))
    }

    /// Cuts the palette to `len` entries (from 1 to the current length): every pixel `>= len` moves to
    /// `into` (which must be `< len`) and the transparent index, if it falls outside, is dropped. With
    /// invalid arguments it returns `false` and touches nothing.
    pub(crate) fn truncate_palette(&mut self, len: usize, into: u8) -> bool {
        let Some(palette) = self.palette.as_mut() else {
            return false;
        };
        if len == 0 || len > palette.len() || usize::from(into) >= len {
            return false;
        }
        for byte in &mut self.data {
            if usize::from(*byte) >= len {
                *byte = into;
            }
        }
        palette.truncate(len);
        true
    }

    /// Undoes `truncate_palette`: gives back the colors `removed`, sets the previous transparent
    /// index and returns the pixels marked in `mask` (one bit per pixel) to the values of
    /// `previous`, in order. Empty `mask` and `previous`: no pixel changed.
    pub(crate) fn restore_truncated_palette(
        &mut self,
        removed: &[Rgba],
        transparent: Option<u8>,
        mask: &[u64],
        previous: &[u8],
    ) -> bool {
        let marked: usize = mask.iter().map(|word| word.count_ones() as usize).sum();
        let room = mask.len().saturating_mul(64) >= self.data.len();
        let Some(palette) = self.palette.as_mut() else {
            return false;
        };
        if marked != previous.len() || (marked > 0 && !room) {
            return false;
        }
        if !palette.restore(removed, transparent) {
            return false;
        }
        let mut values = previous.iter();
        for (position, byte) in self.data.iter_mut().enumerate() {
            let is_marked = mask
                .get(position / 64)
                .is_some_and(|word| word >> (position % 64) & 1 == 1);
            if is_marked && let Some(value) = values.next() {
                *byte = *value;
            }
        }
        true
    }

    /// Number of pixels with a value `>= len`.
    pub(crate) fn count_pixels_from(&self, len: usize) -> usize {
        self.data
            .iter()
            .filter(|&&byte| usize::from(byte) >= len)
            .count()
    }

    /// Marks in `mask` (one bit per pixel, bit 0 of word 0 is the first) the pixels with
    /// a value `>= len` and copies their values, in order, into `previous`. `false` if there is no room in
    /// `mask`; `previous` must have capacity for all of them (it does not reserve).
    pub(crate) fn fill_mask_from(
        &self,
        len: usize,
        mask: &mut [u64],
        previous: &mut Vec<u8>,
    ) -> bool {
        if mask.len().saturating_mul(64) < self.data.len() {
            return false;
        }
        for (position, &byte) in self.data.iter().enumerate() {
            if usize::from(byte) >= len
                && let Some(word) = mask.get_mut(position / 64)
            {
                *word |= 1 << (position % 64);
                previous.push(byte);
            }
        }
        true
    }

    /// Reordena la paleta con la tabla `adelante[old] = new` y remapea los píxeles y el
    /// índice transparente a la vez: ningún color resuelto cambia.
    pub(crate) fn reorder_palette(&mut self, forward: &[u8]) -> bool {
        let Some(palette) = self.palette.as_mut() else {
            return false;
        };
        if !palette.permute(forward) {
            return false;
        }
        for byte in &mut self.data {
            *byte = forward.get(usize::from(*byte)).copied().unwrap_or(*byte);
        }
        true
    }

    /// Quita la entrada `index`: los píxeles que la usan pasan a `into` (índices antes de
    /// quitar) y los de detrás bajan un lugar.
    pub(crate) fn remove_palette_entry(&mut self, index: u8, into: u8) -> bool {
        let Some(palette) = self.palette.as_mut() else {
            return false;
        };
        let len = palette.len();
        if len < 2 || usize::from(index) >= len || usize::from(into) >= len || index == into {
            return false;
        }
        let into_after = if into < index { into } else { into - 1 };
        for byte in &mut self.data {
            *byte = match (*byte).cmp(&index) {
                std::cmp::Ordering::Less => *byte,
                std::cmp::Ordering::Equal => into_after,
                std::cmp::Ordering::Greater => *byte - 1,
            };
        }
        palette.remove_entry(index).is_some()
    }

    /// Deshace `remove_palette_entry`: reinserta el color en `index`, fija el índice
    /// transparente anterior y devuelve a `index` los píxeles marcados en `mask` (un bit por
    /// píxel, vacía si ninguno la usaba); los demás vuelven a su valor anterior.
    pub(crate) fn restore_palette_entry(
        &mut self,
        index: u8,
        color: Rgba,
        transparent: Option<u8>,
        mask: &[u64],
    ) -> bool {
        let Some(palette) = self.palette.as_mut() else {
            return false;
        };
        if !mask.is_empty() && mask.len().saturating_mul(64) < self.data.len() {
            return false;
        }
        if !palette.insert_entry(index, color, transparent) {
            return false;
        }
        for (position, byte) in self.data.iter_mut().enumerate() {
            let marked = mask
                .get(position / 64)
                .is_some_and(|word| word >> (position % 64) & 1 == 1);
            *byte = if marked {
                index
            } else if *byte < index {
                *byte
            } else {
                byte.saturating_add(1)
            };
        }
        true
    }

    /// `table[v]` is the index of the pixel valued `v` after removing the entries `indices` (strictly
    /// increasing) and sending their pixels to `into`; `None` if the arguments are invalid.
    fn removal_table(&self, indices: &[u8], into: u8) -> Option<[u8; Palette::MAX_COLORS]> {
        let len = self.palette.as_ref()?.len();
        let removed = |v: u8| indices.binary_search(&v).is_ok();
        if indices.is_empty()
            || indices.len() >= len
            || indices.windows(2).any(|pair| pair[0] >= pair[1])
            || indices.last().is_some_and(|&last| usize::from(last) >= len)
            || usize::from(into) >= len
            || removed(into)
        {
            return None;
        }
        let below = |v: u8| indices.iter().filter(|&&i| i < v).count() as u8;
        let mut table = [0u8; Palette::MAX_COLORS];
        for v in 0..=u8::MAX {
            let target = if removed(v) { into } else { v };
            if let Some(slot) = table.get_mut(usize::from(v)) {
                *slot = target - below(target);
            }
        }
        Some(table)
    }

    /// Removes the entries `indices`: their pixels move to `into` (index before removing) and the
    /// ones after go down. `false`, touching nothing, with invalid arguments.
    pub(crate) fn remove_palette_entries(&mut self, indices: &[u8], into: u8) -> bool {
        let Some(table) = self.removal_table(indices, into) else {
            return false;
        };
        let Some(palette) = self.palette.as_mut() else {
            return false;
        };
        if !palette.remove_entries(indices) {
            return false;
        }
        for byte in &mut self.data {
            *byte = table.get(usize::from(*byte)).copied().unwrap_or(*byte);
        }
        true
    }

    /// Number of pixels whose value is one of `indices`.
    pub(crate) fn count_pixels_in(&self, indices: &[u8]) -> usize {
        self.data
            .iter()
            .filter(|&&byte| indices.binary_search(&byte).is_ok())
            .count()
    }

    /// Marks in `mask` the pixels whose value is one of `indices` and copies their values, in order,
    /// into `previous` (which must have room: it does not reserve). `false` if `mask` is too small.
    pub(crate) fn fill_mask_in(
        &self,
        indices: &[u8],
        mask: &mut [u64],
        previous: &mut Vec<u8>,
    ) -> bool {
        if mask.len().saturating_mul(64) < self.data.len() {
            return false;
        }
        for (position, &byte) in self.data.iter().enumerate() {
            if indices.binary_search(&byte).is_ok()
                && let Some(word) = mask.get_mut(position / 64)
            {
                *word |= 1 << (position % 64);
                previous.push(byte);
            }
        }
        true
    }

    /// Undoes `remove_palette_entries`: puts the colors back at `indices`, sets the previous
    /// transparent index, gives the marked pixels their values of `previous` (in order) and
    /// shifts the rest back up.
    pub(crate) fn restore_palette_entries(
        &mut self,
        indices: &[u8],
        colors: &[Rgba],
        transparent: Option<u8>,
        mask: &[u64],
        previous: &[u8],
    ) -> bool {
        let marked: usize = mask.iter().map(|word| word.count_ones() as usize).sum();
        let room = mask.len().saturating_mul(64) >= self.data.len();
        let Some(palette) = self.palette.as_mut() else {
            return false;
        };
        if marked != previous.len() || (marked > 0 && !room) {
            return false;
        }
        let old_len = palette.len() + indices.len();
        if !palette.insert_entries(indices, colors, transparent) {
            return false;
        }
        // `kept[new]` is the index that the surviving entry `new` had before removing.
        let mut kept = [0u8; Palette::MAX_COLORS];
        let mut slot = 0usize;
        for old in 0..old_len {
            if indices.binary_search(&(old as u8)).is_err() {
                if let Some(entry) = kept.get_mut(slot) {
                    *entry = old as u8;
                }
                slot += 1;
            }
        }
        let mut values = previous.iter();
        for (position, byte) in self.data.iter_mut().enumerate() {
            let is_marked = mask
                .get(position / 64)
                .is_some_and(|word| word >> (position % 64) & 1 == 1);
            if is_marked {
                if let Some(value) = values.next() {
                    *byte = *value;
                }
            } else {
                *byte = kept.get(usize::from(*byte)).copied().unwrap_or(*byte);
            }
        }
        true
    }

    /// Writes `colors` from entry `start` on (growing the palette with opaque black first if they do
    /// not fit). The pixels and the transparent index do not change.
    pub(crate) fn write_palette_range(&mut self, start: u8, colors: &[Rgba]) -> bool {
        let Some(palette) = self.palette.as_mut() else {
            return false;
        };
        let end = usize::from(start) + colors.len();
        if end > Palette::MAX_COLORS || usize::from(start) > palette.len() {
            return false;
        }
        if end > palette.len() && !palette.extend(end - palette.len(), Rgba::opaque(0, 0, 0)) {
            return false;
        }
        colors
            .iter()
            .zip(start..=u8::MAX)
            .all(|(&color, index)| palette.set(index, color).is_ok())
    }

    /// Rota las entradas `start..start + len` de la paleta (ver `Palette::rotate_range`). Los
    /// píxeles no cambian de índice.
    pub(crate) fn rotate_palette_range(
        &mut self,
        start: u8,
        len: u16,
        k: u16,
        right: bool,
    ) -> bool {
        self.palette
            .as_mut()
            .is_some_and(|palette| palette.rotate_range(start, len, k, right))
    }

    /// Número de píxeles con el valor `index`.
    pub(crate) fn count_pixels_using(&self, index: u8) -> usize {
        self.data.iter().filter(|&&byte| byte == index).count()
    }

    /// Marca en `mask` (un bit por píxel, el bit 0 de la palabra 0 es el primero) los píxeles
    /// con el valor `index`. `false` si `mask` no tiene sitio para todos.
    pub(crate) fn fill_mask(&self, index: u8, mask: &mut [u64]) -> bool {
        if mask.len().saturating_mul(64) < self.data.len() {
            return false;
        }
        for (position, &byte) in self.data.iter().enumerate() {
            if byte == index
                && let Some(word) = mask.get_mut(position / 64)
            {
                *word |= 1 << (position % 64);
            }
        }
        true
    }

    /// Bytes que ocupa una fila: `width × bytes_per_pixel`. Como no hay relleno entre filas, es
    /// también el paso entre una fila y la siguiente en [`Image::bytes`].
    pub fn row_len(&self) -> usize {
        // El producto cabe: `byte_len` lo comprobó al crear la imagen.
        self.width as usize * self.mode.bytes_per_pixel()
    }

    /// Los bytes de los píxeles, de solo lectura y sin copiar. Su longitud es exactamente
    /// `Image::byte_len(width, height, mode)`.
    ///
    /// Formato: filas de arriba abajo y, dentro de una fila, de izquierda a derecha, **sin relleno
    /// entre filas**. El píxel `(x, y)` empieza en `(y × width + x) × bytes_per_pixel`. Cada canal
    /// es de 8 bits, así que no hay orden de bytes que elegir.
    ///
    /// | Modo | Bytes por píxel | Orden |
    /// | --- | --- | --- |
    /// | `Rgba` | 4 | `r, g, b, a`; alfa directo, no premultiplicado; con `a = 0` el RGB se conserva tal cual |
    /// | `Indexed` | 1 | el índice; siempre menor que `palette().len()`; el índice transparente no se resuelve |
    /// | `Gray` | 1 | `gray` (opaco) |
    /// | `GrayAlpha` | 2 | `gray, alpha` |
    pub fn bytes(&self) -> &[u8] {
        &self.data
    }

    /// La fila `y` (`row_len()` bytes, formato de [`Image::bytes`]), o `None` si `y >= height`.
    /// No hace pánico, tampoco con `u32::MAX`.
    pub fn row(&self, y: u32) -> Option<&[u8]> {
        if y >= self.height {
            return None;
        }
        let len = self.row_len();
        let at = y as usize * len;
        self.data.get(at..at + len)
    }

    /// Las `height` filas, de arriba abajo, cada una como en [`Image::row`].
    pub fn rows(&self) -> impl ExactSizeIterator<Item = &[u8]> + DoubleEndedIterator + Clone {
        // `row_len() >= 1` porque `width >= 1`: `chunks_exact` no entra en pánico.
        self.data.chunks_exact(self.row_len())
    }

    /// Imagen de un modo que no necesita paleta con todos los píxeles iguales a `pixel` (guardado
    /// tal cual, sin normalizar). Con `Pixel::Indexed(_)` usa [`Image::filled_indexed`].
    ///
    /// Errores, en este orden: `ZeroSize` y `TooLarge` (también si no se pudo reservar) y
    /// `PaletteRequired`. Equivale a [`Image::new`] más `set_pixel` en cada píxel.
    pub fn filled(width: u32, height: u32, pixel: Pixel) -> Result<Image, ImageError> {
        let len = Image::byte_len(width, height, pixel.mode())?;
        if pixel.mode() == Mode::Indexed {
            return Err(ImageError::PaletteRequired);
        }
        Image::with_pixel(width, height, len, pixel, None)
    }

    /// Imagen indexada con todos los píxeles en `index`.
    ///
    /// Errores, en este orden: `ZeroSize` y `TooLarge`, `EmptyPalette` e
    /// `IndexNotInPalette { index, len }` si `index >= palette.len()`.
    pub fn filled_indexed(
        width: u32,
        height: u32,
        palette: Palette,
        index: u8,
    ) -> Result<Image, ImageError> {
        let len = Image::byte_len(width, height, Mode::Indexed)?;
        if palette.is_empty() {
            return Err(ImageError::EmptyPalette);
        }
        if usize::from(index) >= palette.len() {
            return Err(ImageError::IndexNotInPalette {
                index,
                len: palette.len(),
            });
        }
        Image::with_pixel(width, height, len, Pixel::Indexed(index), Some(palette))
    }

    /// Reserva `len` bytes y los llena repitiendo el píxel: un `memset` con un byte y, con más,
    /// copias que doblan lo ya escrito. Quien llama ya comprobó `len` y, en indexado, el índice.
    fn with_pixel(
        width: u32,
        height: u32,
        len: usize,
        pixel: Pixel,
        palette: Option<Palette>,
    ) -> Result<Image, ImageError> {
        let (pattern, size) = match pixel {
            Pixel::Rgba(c) => ([c.r, c.g, c.b, c.a], 4),
            Pixel::GrayAlpha { gray, alpha } => ([gray, alpha, 0, 0], 2),
            Pixel::Indexed(v) | Pixel::Gray(v) => ([v, 0, 0, 0], 1),
        };
        let mut data = Vec::new();
        data.try_reserve_exact(len)
            .map_err(|_| ImageError::TooLarge { width, height })?;
        if size == 1 {
            data.resize(len, pattern[0]);
        } else {
            data.extend_from_slice(&pattern[..size]);
            while data.len() < len {
                let more = data.len().min(len - data.len());
                data.extend_from_within(..more);
            }
        }
        Ok(Image {
            width,
            height,
            mode: pixel.mode(),
            data,
            palette,
        })
    }

    /// Copia de la imagen que no aborta el proceso si falta memoria: `TooLarge` si la reserva
    /// falla. Es igual a `clone()` y no comparte memoria con el original.
    pub fn try_clone(&self) -> Result<Image, ImageError> {
        let mut data = Vec::new();
        data.try_reserve_exact(self.data.len())
            .map_err(|_| ImageError::TooLarge {
                width: self.width,
                height: self.height,
            })?;
        data.extend_from_slice(&self.data);
        Ok(Image {
            width: self.width,
            height: self.height,
            mode: self.mode,
            data,
            palette: self.palette.clone(),
        })
    }

    /// Imagen a partir de sus bytes (formato de [`Image::bytes`]) en un modo que no necesita
    /// paleta; con `Mode::Indexed` usa [`Image::from_bytes_indexed`]. Toma el `Vec` sin copiarlo.
    ///
    /// **Quien llama debe haber reservado `data` con `try_reserve` (o `try_reserve_exact`)**, no con
    /// `vec![..]` ni con un `resize` a ciegas: una reserva que aborta el proceso no se puede
    /// convertir en `Err`.
    ///
    /// Errores, en este orden: `ZeroSize` y `TooLarge` (antes de mirar `data`), `PaletteRequired`
    /// (modo indexado) y `WrongLength { expected, found }` si `data.len()` no es
    /// `Image::byte_len`. Con un error el `Vec` se descarta.
    pub fn from_bytes(
        width: u32,
        height: u32,
        mode: Mode,
        data: Vec<u8>,
    ) -> Result<Image, ImageError> {
        let len = Image::byte_len(width, height, mode)?;
        if mode == Mode::Indexed {
            return Err(ImageError::PaletteRequired);
        }
        if data.len() != len {
            return Err(ImageError::WrongLength {
                expected: len,
                found: data.len(),
            });
        }
        Ok(Image {
            width,
            height,
            mode,
            data,
            palette: None,
        })
    }

    /// Como [`Image::from_bytes`] para una imagen indexada: cada byte es un índice y debe ser
    /// menor que `palette.len()`. Toma el `Vec` sin copiarlo; **quien llama debe haberlo reservado
    /// con `try_reserve` (o `try_reserve_exact`)**, como en `from_bytes`.
    ///
    /// Errores, en este orden: `ZeroSize` y `TooLarge` (antes de mirar `data`),
    /// `WrongLength { expected, found }`, `EmptyPalette` e `IndexNotInPalette { index, len }`, con
    /// el **mayor** índice encontrado. Con un error el `Vec` se descarta.
    pub fn from_bytes_indexed(
        width: u32,
        height: u32,
        palette: Palette,
        data: Vec<u8>,
    ) -> Result<Image, ImageError> {
        let len = Image::byte_len(width, height, Mode::Indexed)?;
        if data.len() != len {
            return Err(ImageError::WrongLength {
                expected: len,
                found: data.len(),
            });
        }
        if palette.is_empty() {
            return Err(ImageError::EmptyPalette);
        }
        if let Some(&index) = data.iter().max()
            && usize::from(index) >= palette.len()
        {
            return Err(ImageError::IndexNotInPalette {
                index,
                len: palette.len(),
            });
        }
        Ok(Image {
            width,
            height,
            mode: Mode::Indexed,
            data,
            palette: Some(palette),
        })
    }

    /// Algún píxel vale `index`. Siempre `false` si la imagen no es indexada.
    pub fn uses_index(&self, index: u8) -> bool {
        self.mode == Mode::Indexed && self.data.contains(&index)
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// La paleta, solo en modo indexado.
    pub fn palette(&self) -> Option<&Palette> {
        self.palette.as_ref()
    }

    pub fn get_pixel(&self, x: u32, y: u32) -> Result<Pixel, ImageError> {
        let at = self.offset(x, y)?;
        let d = &self.data[at..at + self.mode.bytes_per_pixel()];
        Ok(match self.mode {
            Mode::Rgba => Pixel::Rgba(Rgba::new(d[0], d[1], d[2], d[3])),
            Mode::Indexed => Pixel::Indexed(d[0]),
            Mode::Gray => Pixel::Gray(d[0]),
            Mode::GrayAlpha => Pixel::GrayAlpha {
                gray: d[0],
                alpha: d[1],
            },
        })
    }

    /// Escribe un píxel del modo de la imagen; si hay error, la imagen no cambia.
    pub fn set_pixel(&mut self, x: u32, y: u32, pixel: Pixel) -> Result<(), ImageError> {
        let at = self.offset(x, y)?;
        self.check_pixel(pixel)?;
        let d = &mut self.data[at..at + self.mode.bytes_per_pixel()];
        match pixel {
            Pixel::Rgba(c) => d.copy_from_slice(&[c.r, c.g, c.b, c.a]),
            Pixel::Indexed(i) => d[0] = i,
            Pixel::Gray(g) => d[0] = g,
            Pixel::GrayAlpha { gray, alpha } => d.copy_from_slice(&[gray, alpha]),
        }
        Ok(())
    }

    /// Los bytes con que se guarda un píxel y cuántos son (los de [`Image::bytes`]).
    pub(crate) fn encode(pixel: Pixel) -> ([u8; 4], usize) {
        match pixel {
            Pixel::Rgba(c) => ([c.r, c.g, c.b, c.a], 4),
            Pixel::Indexed(i) => ([i, 0, 0, 0], 1),
            Pixel::Gray(g) => ([g, 0, 0, 0], 1),
            Pixel::GrayAlpha { gray, alpha } => ([gray, alpha, 0, 0], 2),
        }
    }

    /// Escribe `pixel` en los `len` píxeles de la fila `y` que empiezan en `x`, por filas. Si el
    /// píxel no es del modo de la imagen (o su índice no cabe en la paleta) o la racha se sale de
    /// la fila, no escribe nada y devuelve `false`.
    pub(crate) fn fill_run(&mut self, x: u32, y: u32, len: u32, pixel: Pixel) -> bool {
        if self.check_pixel(pixel).is_err() || y >= self.height {
            return false;
        }
        let Some(end) = x.checked_add(len).filter(|&end| end <= self.width) else {
            return false;
        };
        let bpp = self.mode.bytes_per_pixel();
        let (key, _) = Image::encode(pixel);
        let Some(key) = key.get(..bpp) else {
            return false;
        };
        let row_len = self.row_len();
        let at = y as usize * row_len;
        let Some(part) = self
            .data
            .get_mut(at + x as usize * bpp..at + end as usize * bpp)
        else {
            return false;
        };
        if bpp == 1 {
            part.fill(key[0]);
        } else {
            for chunk in part.chunks_exact_mut(bpp) {
                chunk.copy_from_slice(key);
            }
        }
        true
    }

    /// Comprueba, sin escribir, que el píxel es del modo de la imagen y que su índice cabe
    /// en la paleta.
    ///
    /// Errores: `ModeMismatch { expected, found }` si el modo no coincide (también con un índice
    /// fuera de la paleta: el modo se comprueba primero) e `IndexNotInPalette { index, len }`
    /// si el índice es `>= len`. Es la comprobación de `set_pixel` y de `Document::apply`.
    pub fn check_pixel(&self, pixel: Pixel) -> Result<(), ImageError> {
        if pixel.mode() != self.mode {
            return Err(ImageError::ModeMismatch {
                expected: self.mode,
                found: pixel.mode(),
            });
        }
        if let (Pixel::Indexed(index), Some(palette)) = (pixel, &self.palette)
            && usize::from(index) >= palette.len()
        {
            return Err(ImageError::IndexNotInPalette {
                index,
                len: palette.len(),
            });
        }
        Ok(())
    }

    /// Posición del primer byte del píxel `(x, y)`.
    fn offset(&self, x: u32, y: u32) -> Result<usize, ImageError> {
        if x >= self.width || y >= self.height {
            return Err(ImageError::OutOfBounds {
                x,
                y,
                width: self.width,
                height: self.height,
            });
        }
        // El producto cabe: `byte_len` lo comprobó al crear la imagen.
        Ok((y as usize * self.width as usize + x as usize) * self.mode.bytes_per_pixel())
    }
}

/// Error al crear o usar una imagen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImageError {
    /// Ancho o alto cero.
    ZeroSize { width: u32, height: u32 },
    /// El tamaño en bytes desborda, supera [`Image::MAX_BYTES`] o no se pudo reservar.
    TooLarge { width: u32, height: u32 },
    /// `Image::new` con `Mode::Indexed`.
    PaletteRequired,
    /// `Image::new_indexed` con una paleta sin entradas.
    EmptyPalette,
    /// La coordenada cae fuera de la imagen.
    OutOfBounds {
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    },
    /// El píxel no es del modo de la imagen.
    ModeMismatch { expected: Mode, found: Mode },
    /// `Pixel::Indexed(index)` con `index` mayor o igual que la longitud de la paleta.
    IndexNotInPalette { index: u8, len: usize },
    /// `Image::from_bytes` con una cantidad de bytes distinta de la que pide el tamaño.
    WrongLength { expected: usize, found: usize },
}

impl fmt::Display for ImageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ImageError::ZeroSize { width, height } => {
                write!(f, "una imagen de {width} × {height} no tiene píxeles")
            }
            ImageError::TooLarge { width, height } => write!(
                f,
                "una imagen de {width} × {height} supera el máximo de {} bytes",
                Image::MAX_BYTES
            ),
            ImageError::PaletteRequired => {
                write!(f, "el modo indexado necesita una paleta")
            }
            ImageError::EmptyPalette => {
                write!(
                    f,
                    "una imagen indexada necesita al menos un color en la paleta"
                )
            }
            ImageError::OutOfBounds {
                x,
                y,
                width,
                height,
            } => write!(
                f,
                "el píxel ({x}, {y}) cae fuera de una imagen de {width} × {height}"
            ),
            ImageError::ModeMismatch { expected, found } => write!(
                f,
                "píxel de modo {found:?} en una imagen de modo {expected:?}"
            ),
            ImageError::WrongLength { expected, found } => {
                write!(f, "{found} bytes de píxeles en vez de {expected}")
            }
            ImageError::IndexNotInPalette { index, len } => {
                write!(f, "índice {index} fuera de una paleta de {len} colores")
            }
        }
    }
}

impl Error for ImageError {}
