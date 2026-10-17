//! Conversión entre modos de imagen sin pérdidas en silencio (specs/hito-2-conversion-modos.md).
//!
//! La única cosa que se descarta sin avisar es el RGB de un píxel con `a = 0`: en toda conversión
//! ese píxel pasa a ser el transparente canónico del modo destino.

use std::error::Error;
use std::fmt;

use crate::{Image, Mode, Palette, Rgba};

/// Pesos de `r`, `g` y `b` de [`luma`], en 256avos (Rec. 709: 0,2126 / 0,7152 / 0,0722). Suman 256.
pub const LUMA_WEIGHTS: [u32; 3] = [54, 183, 19];

/// Lo que se permite perder al convertir. No hay `Default`: hay que escribirlo siempre.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Allowed {
    /// Permite pasar de color a gris con [`luma`].
    pub luma: bool,
    /// Permite descartar el alfa componiendo sobre este fondo opaco.
    pub drop_alpha: Option<[u8; 3]>,
}

impl Allowed {
    /// No se permite perder nada.
    pub const NOTHING: Allowed = Allowed {
        luma: false,
        drop_alpha: None,
    };
}

/// Píxeles que perdieron color o alfa en una conversión.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ConversionReport {
    /// Píxeles con `a > 0` cuyos `r, g, b` no eran iguales y pasaron a gris.
    pub color_lost: u64,
    /// Píxeles con `a < 255` compuestos sobre el fondo al pasar a `Gray`.
    pub alpha_lost: u64,
}

/// Resultado de una conversión: la imagen nueva y lo que se perdió.
#[derive(Clone, Debug)]
pub struct Conversion {
    pub image: Image,
    pub report: ConversionReport,
}

/// Error al convertir una imagen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConvertError {
    /// La conversión perdería algo que `allowed` no cubre; el informe lleva los contadores
    /// completos.
    LossNotAllowed { report: ConversionReport },
    /// El destino es indexado y la imagen tiene más de 256 colores. `at_least` es el número visto
    /// al cortar el recuento: siempre 257.
    TooManyColors { at_least: usize },
    /// La imagen de destino supera [`Image::MAX_BYTES`] o no se pudo reservar.
    TooLarge { width: u32, height: u32 },
}

impl fmt::Display for ConvertError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConvertError::LossNotAllowed { report } => write!(
                f,
                "la conversión perdería color en {} píxeles y alfa en {} que no se han permitido",
                report.color_lost, report.alpha_lost
            ),
            ConvertError::TooManyColors { .. } => write!(
                f,
                "la imagen tiene más de {} colores y no cabe en una paleta",
                Palette::MAX_COLORS
            ),
            ConvertError::TooLarge { width, height } => write!(
                f,
                "una imagen de {width} × {height} en el modo de destino supera el máximo de {} bytes",
                Image::MAX_BYTES
            ),
        }
    }
}

impl Error for ConvertError {}

/// Luminancia entera de un color sRGB codificado (no lineal), redondeada al nivel más cercano.
///
/// `luma(v, v, v) = v`. Error: como mucho 0,5 niveles sobre su propia fracción y menos de 1,02
/// sobre la Rec. 709 con pesos reales.
pub fn luma(r: u8, g: u8, b: u8) -> u8 {
    let [wr, wg, wb] = LUMA_WEIGHTS;
    let sum = wr * u32::from(r) + wg * u32::from(g) + wb * u32::from(b) + 128;
    // La suma de pesos es 256: el máximo es (255·256 + 128) >> 8 = 255.
    (sum >> 8).min(255) as u8
}

/// Composición al nivel más cercano de `c` sobre `k` con alfa `a`.
fn compose(c: u8, k: u8, a: u8) -> u8 {
    let (c, k, a) = (u32::from(c), u32::from(k), u32::from(a));
    // El máximo es 255·255 + 127, y el cociente cabe en `u8`.
    ((c * a + k * (255 - a) + 127) / 255).min(255) as u8
}

/// Tamaño en bytes de los píxeles de una imagen de ese tamaño en `target`, sin reservar memoria.
///
/// `TooLarge` si desborda o supera [`Image::MAX_BYTES`].
pub fn target_byte_len(width: u32, height: u32, target: Mode) -> Result<usize, ConvertError> {
    Image::byte_len(width, height, target).map_err(|_| ConvertError::TooLarge { width, height })
}

/// Convierte `image` a `target`; no la modifica.
///
/// Si `target` es su propio modo, devuelve una copia idéntica. Orden: tamaño del destino,
/// pasada de solo lectura (informe y paleta), permisos, reserva y pasada que escribe.
pub fn convert_image(
    image: &Image,
    target: Mode,
    allowed: Allowed,
) -> Result<Conversion, ConvertError> {
    let (width, height) = (image.width(), image.height());
    let too_large = || ConvertError::TooLarge { width, height };
    let len = target_byte_len(width, height, target)?;

    if image.mode() == target {
        let mut data = Vec::new();
        data.try_reserve_exact(len).map_err(|_| too_large())?;
        data.extend_from_slice(image.raw());
        let palette = image.palette().cloned();
        return Ok(Conversion {
            image: Image::from_raw(width, height, target, data, palette),
            report: ConversionReport::default(),
        });
    }

    // Pasada 1, de solo lectura.
    let mut report = ConversionReport::default();
    let mut builder = (target == Mode::Indexed)
        .then(|| PaletteBuilder::new().ok_or_else(too_large))
        .transpose()?;
    for pixel in Pixels::new(image) {
        if matches!(target, Mode::Gray | Mode::GrayAlpha)
            && pixel.a > 0
            && !(pixel.r == pixel.g && pixel.g == pixel.b)
        {
            report.color_lost += 1;
        }
        if target == Mode::Gray && pixel.a < 255 {
            report.alpha_lost += 1;
        }
        if let Some(builder) = builder.as_mut() {
            builder.add(pixel)?;
        }
    }
    if (report.color_lost > 0 && !allowed.luma)
        || (report.alpha_lost > 0 && allowed.drop_alpha.is_none())
    {
        return Err(ConvertError::LossNotAllowed { report });
    }

    // Reserva de lo que se va a escribir: ya no se puede fallar después de este punto, salvo
    // por la propia reserva.
    let palette = builder.as_ref().map(PaletteBuilder::palette).transpose()?;
    let mut data = Vec::new();
    data.try_reserve_exact(len).map_err(|_| too_large())?;

    // Pasada 2, que escribe dentro de la capacidad reservada.
    let background = allowed.drop_alpha.map_or(0, |[r, g, b]| luma(r, g, b));
    for pixel in Pixels::new(image) {
        let Rgba { r, g, b, a } = pixel;
        match target {
            Mode::Rgba => data.extend_from_slice(&[r, g, b, a]),
            Mode::GrayAlpha => data.extend_from_slice(&[luma(r, g, b), a]),
            Mode::Gray => data.push(compose(luma(r, g, b), background, a)),
            Mode::Indexed => {
                if let Some(builder) = &builder {
                    data.push(builder.index_of(pixel));
                }
            }
        }
    }
    Ok(Conversion {
        image: Image::from_raw(width, height, target, data, palette),
        report,
    })
}

/// Recorre los píxeles de una imagen resueltos a `Rgba`, con `a = 0` canonizado a
/// `(0,0,0,0)`.
struct Pixels<'a> {
    mode: Mode,
    bytes: &'a [u8],
    at: usize,
    /// Valor resuelto de cada índice; solo se usa en modo indexado.
    table: [Rgba; 256],
}

impl<'a> Pixels<'a> {
    fn new(image: &'a Image) -> Pixels<'a> {
        let mut table = [Rgba::TRANSPARENT; 256];
        if let Some(palette) = image.palette() {
            for (slot, index) in table.iter_mut().zip(0..=u8::MAX) {
                if let Ok(color) = palette.resolved(index) {
                    *slot = color;
                }
            }
        }
        Pixels {
            mode: image.mode(),
            bytes: image.raw(),
            at: 0,
            table,
        }
    }
}

impl Iterator for Pixels<'_> {
    type Item = Rgba;

    fn next(&mut self) -> Option<Rgba> {
        let d = self.bytes.get(self.at..)?;
        let (pixel, size) = match self.mode {
            Mode::Rgba => (
                Rgba::new(*d.first()?, *d.get(1)?, *d.get(2)?, *d.get(3)?),
                4,
            ),
            Mode::GrayAlpha => {
                let g = *d.first()?;
                (Rgba::new(g, g, g, *d.get(1)?), 2)
            }
            Mode::Gray => {
                let g = *d.first()?;
                (Rgba::opaque(g, g, g), 1)
            }
            Mode::Indexed => (self.table[usize::from(*d.first()?)], 1),
        };
        self.at += size;
        Some(if pixel.a == 0 {
            Rgba::TRANSPARENT
        } else {
            pixel
        })
    }
}

/// Construye la paleta de un destino indexado: el transparente en el índice 0 si lo hay y el
/// resto por primera aparición. Sin `HashMap`: la búsqueda es binaria sobre una tabla ordenada.
struct PaletteBuilder {
    /// Colores con `a > 0`, en orden de primera aparición.
    colors: Vec<Rgba>,
    /// `(color empaquetado, posición en `colors`)`, ordenada por el color.
    lookup: Vec<(u32, u8)>,
    has_transparent: bool,
}

impl PaletteBuilder {
    fn new() -> Option<PaletteBuilder> {
        let mut colors = Vec::new();
        let mut lookup = Vec::new();
        colors.try_reserve_exact(Palette::MAX_COLORS).ok()?;
        lookup.try_reserve_exact(Palette::MAX_COLORS).ok()?;
        Some(PaletteBuilder {
            colors,
            lookup,
            has_transparent: false,
        })
    }

    fn key(color: Rgba) -> u32 {
        u32::from_be_bytes([color.r, color.g, color.b, color.a])
    }

    /// Registra un color. Corta con `TooManyColors` al llegar al 257.
    fn add(&mut self, color: Rgba) -> Result<(), ConvertError> {
        if color.a == 0 {
            self.has_transparent = true;
        } else if let Err(at) = self.lookup.binary_search_by_key(&Self::key(color), |e| e.0) {
            // Siempre cabe: se comprueba el límite justo debajo, con la capacidad reservada.
            let position = u8::try_from(self.colors.len()).unwrap_or(u8::MAX);
            if self.colors.len() >= Palette::MAX_COLORS {
                return Err(too_many_colors());
            }
            self.colors.push(color);
            self.lookup.insert(at, (Self::key(color), position));
        }
        if self.colors.len() + usize::from(self.has_transparent) > Palette::MAX_COLORS {
            return Err(too_many_colors());
        }
        Ok(())
    }

    fn palette(&self) -> Result<Palette, ConvertError> {
        let transparent = self.has_transparent.then_some(Rgba::TRANSPARENT);
        let mut palette =
            Palette::from_colors(transparent.into_iter().chain(self.colors.iter().copied()))
                .map_err(|_| too_many_colors())?;
        if self.has_transparent {
            palette
                .set_transparent_index(Some(0))
                .map_err(|_| too_many_colors())?;
        }
        Ok(palette)
    }

    /// Índice final de un color ya registrado.
    fn index_of(&self, color: Rgba) -> u8 {
        let offset = u8::from(self.has_transparent);
        if color.a == 0 {
            return 0;
        }
        self.lookup
            .binary_search_by_key(&Self::key(color), |e| e.0)
            .ok()
            .and_then(|at| self.lookup.get(at))
            .map_or(0, |entry| entry.1.saturating_add(offset))
    }
}

fn too_many_colors() -> ConvertError {
    ConvertError::TooManyColors {
        at_least: Palette::MAX_COLORS + 1,
    }
}
