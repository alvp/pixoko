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
    fn bytes_per_pixel(self) -> usize {
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

    /// Los bytes de los píxeles, fila a fila, tal como los describe el tipo.
    pub(crate) fn raw(&self) -> &[u8] {
        &self.data
    }

    /// Bytes de píxeles más 4 por entrada de paleta: lo que ocupa la imagen en la historia.
    pub(crate) fn content_bytes(&self) -> usize {
        let palette = self.palette.as_ref().map_or(0, Palette::len);
        self.data
            .len()
            .saturating_add(palette.saturating_mul(size_of::<Rgba>()))
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
            ImageError::IndexNotInPalette { index, len } => {
                write!(f, "índice {index} fuera de una paleta de {len} colores")
            }
        }
    }
}

impl Error for ImageError {}
