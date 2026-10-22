//! Composición de la pila de capas de un `Document` en una `Image` (unidad 3.2).
//!
//! Función pura y determinista: enteros, sin estado, sin hilos y sin redondeos que dependan de
//! la plataforma. Las fórmulas y su motivo están en `specs/hito-3-mezcla.md`; resumen:
//!
//! - `αs = div255(a · opacity)`; un origen exactamente `(0, 0, 0, 0)` es «nada»; contra un
//!   fondo de alfa 0 el resultado es el origen (con su RGB, también si `αs = 0`).
//! - Teoría (Porter y Duff, W3C): `αr = αs + αb − αs·αb` y `Cr = Cb + (Cm − Cb)·αs/αr` con
//!   `Cm = Cs + (B − Cs)·αb/255`; con fondo opaco `Cm = B`, con Normal `Cm = Cs`.
//! - **Empírica** (fondo translúcido, `0 < αb < 255`, y modo ≠ Normal): sale de los PNG del
//!   corpus y no de la teoría; peso de la mezcla `m = 1 − (1 − αb)(1 − αs·αb)` con la secuencia
//!   entera de [`composite_channels`]. Tolerancia declarada: 2.

use std::error::Error;
use std::fmt;

use px_core::{BlendMode, Document, Image, ImageError, Layer, LayerId, Mode, Rgba};

/// Rectángulo de píxeles del lienzo: esquina superior izquierda y tamaño.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// Por qué no se pudo componer.
#[derive(Debug, PartialEq)]
pub enum ComposeError {
    /// El rectángulo tiene ancho o alto 0.
    EmptyRect,
    /// El rectángulo no cabe en el lienzo de `width × height`.
    RectOutside { rect: Rect, width: u32, height: u32 },
    /// Hay píxeles sin nada que dar y el modo no tiene valor transparente (grises sin alfa, o
    /// indexado sin índice transparente).
    NoTransparency { mode: Mode },
    /// No se pudo reservar la imagen de salida (o su tamaño no vale).
    Image(ImageError),
}

impl fmt::Display for ComposeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ComposeError::EmptyRect => write!(f, "el rectángulo no tiene píxeles"),
            ComposeError::RectOutside {
                rect,
                width,
                height,
            } => write!(
                f,
                "el rectángulo de {} × {} en ({}, {}) no cabe en el lienzo de {width} × {height}",
                rect.width, rect.height, rect.x, rect.y
            ),
            ComposeError::NoTransparency { mode } => write!(
                f,
                "el modo {mode:?} no tiene valor transparente para los píxeles sin capa visible"
            ),
            ComposeError::Image(source) => {
                write!(f, "no se pudo crear la imagen compuesta: {source}")
            }
        }
    }
}

impl Error for ComposeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            ComposeError::Image(source) => Some(source),
            _ => None,
        }
    }
}

impl From<ImageError> for ComposeError {
    fn from(source: ImageError) -> ComposeError {
        ComposeError::Image(source)
    }
}

/// `x / 255` al entero más cercano; exacto en `0..=65025`.
#[inline]
fn div255(x: i32) -> i32 {
    let x = x + 128;
    (x + (x >> 8)) >> 8
}

/// `n / d` al entero más cercano, mitades hacia arriba (`d > 0`). Con `d = 255` no hay empates.
#[inline]
fn rnd(n: i32, d: i32) -> i32 {
    (2 * n + d).div_euclid(2 * d)
}

#[inline]
fn mul(b: i32, s: i32) -> i32 {
    div255(b * s)
}

#[inline]
fn screen(b: i32, s: i32) -> i32 {
    b + s - mul(b, s)
}

/// Función de mezcla de un canal: `b` es el fondo y `s` el origen. Un modo que esta versión no
/// conoce (`BlendMode` es `#[non_exhaustive]`) se trata como Normal.
#[inline]
fn blend(mode: BlendMode, b: i32, s: i32) -> i32 {
    match mode {
        BlendMode::Multiply => mul(b, s),
        BlendMode::Screen => screen(b, s),
        BlendMode::Overlay => {
            if b < 128 {
                mul(s, 2 * b)
            } else {
                screen(s, 2 * b - 255)
            }
        }
        BlendMode::Darken => b.min(s),
        BlendMode::Lighten => b.max(s),
        BlendMode::Add => (b + s).min(255),
        BlendMode::Difference => (b - s).abs(),
        _ => s,
    }
}

/// Un píxel de `N` canales con alfa: el resultado de poner `src` (con alfa `a` y la `opacity` de
/// su capa) sobre `back` (con alfa `ab`). Es la única copia de las fórmulas.
#[inline]
fn composite_channels<const N: usize>(
    back: [u8; N],
    ab: u8,
    src: [u8; N],
    a: u8,
    opacity: u8,
    mode: BlendMode,
) -> ([u8; N], u8) {
    // Paso 0: un origen exactamente transparente es «nada».
    if a == 0 && src.iter().all(|&c| c == 0) {
        return (back, ab);
    }
    // Paso 1.
    let a_s = div255(i32::from(a) * i32::from(opacity));
    // Paso 2: contra un fondo vacío no hay con qué mezclar; el RGB del origen se conserva.
    if ab == 0 {
        return (src, a_s.clamp(0, 255) as u8);
    }
    // Paso 3.
    if a_s == 0 {
        return (back, ab);
    }
    // Paso 4.
    let ab_i = i32::from(ab);
    let ar = a_s + ab_i - div255(a_s * ab_i);
    let empirical = mode != BlendMode::Normal && ab < 255;
    let k = if empirical { div255(a_s * ab_i) } else { 0 };
    let mut out = [0u8; N];
    for (slot, (&cb, &cs)) in out.iter_mut().zip(back.iter().zip(src.iter())) {
        let (cb, cs) = (i32::from(cb), i32::from(cs));
        let b = blend(mode, cb, cs);
        let cr = if ab == 255 {
            // 4a con fondo opaco: `Cm = B` y `αr = 255` (la división es por una constante).
            cb + (b - cb) * a_s / 255
        } else if !empirical {
            // 4a (teoría), Normal con fondo translúcido: `Cm = Cs`.
            let cm = cs + (b - cs) * ab_i / 255;
            cb + (cm - cb) * a_s / ar
        } else {
            // 4b (empírica).
            let x1 = cs + rnd((b - cs) * ab_i, 255);
            let x2 = x1 + rnd((b - x1) * k, 255);
            cb + rnd((x2 - cb) * a_s, ar)
        };
        *slot = cr.clamp(0, 255) as u8;
    }
    (out, ar.clamp(0, 255) as u8)
}

/// Pone `src` con la `opacity` y el `mode` de su capa sobre `back` (pasos 0 a 4 de la spec): lo
/// que hace `compose` con cada píxel y cada capa. Con `back` de alfa 0 el resultado es el origen
/// con su RGB (`αs = div255(a · opacity)`), también si ese alfa es 0; un origen
/// `(0, 0, 0, 0)` no cambia nada.
pub fn composite_pixel(back: Rgba, src: Rgba, opacity: u8, mode: BlendMode) -> Rgba {
    let (rgb, a) = composite_channels(
        [back.r, back.g, back.b],
        back.a,
        [src.r, src.g, src.b],
        src.a,
        opacity,
        mode,
    );
    Rgba::new(rgb[0], rgb[1], rgb[2], a)
}

/// Compone todo el lienzo: es `compose_rect` del rectángulo entero.
pub fn compose(doc: &Document) -> Result<Image, ComposeError> {
    compose_rect(
        doc,
        Rect {
            x: 0,
            y: 0,
            width: doc.width(),
            height: doc.height(),
        },
    )
}

/// Compone el rectángulo `rect` del lienzo y devuelve una imagen de `rect.width × rect.height`
/// del modo del documento: **es el recorte de `compose(doc)`**, píxel a píxel.
///
/// Recorre las capas visibles con imagen de abajo arriba sobre un solo búfer de salida (el pico
/// de memoria es la salida). Las celdas vacías y las capas ocultas no cuestan nada.
pub fn compose_rect(doc: &Document, rect: Rect) -> Result<Image, ComposeError> {
    compose_rect_of(doc, rect, Layer::visible)
}

/// La composición de **solo** las capas `layers`, en el orden del documento y **sin mirar su
/// visibilidad**, sobre vacío (unidad 3.5: es lo que se combina). Con exactamente las capas
/// visibles es `compose` byte a byte; mismos errores que [`compose_rect`].
pub fn compose_layers(doc: &Document, layers: &[LayerId]) -> Result<Image, ComposeError> {
    let rect = Rect {
        x: 0,
        y: 0,
        width: doc.width(),
        height: doc.height(),
    };
    compose_rect_of(doc, rect, |layer| layers.contains(&layer.id()))
}

/// El bucle de [`compose_rect`] y [`compose_layers`]: compone las capas que cumplen `include`.
fn compose_rect_of(
    doc: &Document,
    rect: Rect,
    include: impl Fn(&Layer) -> bool,
) -> Result<Image, ComposeError> {
    if rect.width == 0 || rect.height == 0 {
        return Err(ComposeError::EmptyRect);
    }
    let (width, height) = (doc.width(), doc.height());
    let fits = u64::from(rect.x) + u64::from(rect.width) <= u64::from(width)
        && u64::from(rect.y) + u64::from(rect.height) <= u64::from(height);
    if !fits {
        return Err(ComposeError::RectOutside {
            rect,
            width,
            height,
        });
    }
    let mode = doc.mode();
    let len = Image::byte_len(rect.width, rect.height, mode)?;
    let transparent = match mode {
        Mode::Indexed => doc
            .palette()
            .and_then(|palette| palette.transparent_index()),
        _ => None,
    };
    let any_visible = doc
        .layers()
        .iter()
        .any(|layer| include(layer) && doc.image(layer.cel()).is_some());
    if matches!(mode, Mode::Indexed | Mode::Gray) && transparent.is_none() && !any_visible {
        return Err(ComposeError::NoTransparency { mode });
    }

    let mut data = Vec::new();
    data.try_reserve_exact(len).map_err(|_| {
        ComposeError::Image(ImageError::TooLarge {
            width: rect.width,
            height: rect.height,
        })
    })?;
    data.resize(len, transparent.unwrap_or(0));

    let bpp = mode.bytes_per_pixel();
    let (x0, w) = (rect.x as usize * bpp, rect.width as usize * bpp);
    for layer in doc.layers() {
        if !include(layer) {
            continue;
        }
        let Some(cel) = doc.image(layer.cel()) else {
            continue;
        };
        if cel.mode() != mode || cel.width() != width || cel.height() != height {
            continue;
        }
        let (opacity, blend_mode) = (layer.opacity(), layer.blend());
        for (out_row, y) in data.chunks_exact_mut(w).zip(rect.y..) {
            let Some(src_row) = cel.row(y).and_then(|row| row.get(x0..x0 + w)) else {
                continue;
            };
            match mode {
                Mode::Rgba => compose_row_rgba(out_row, src_row, opacity, blend_mode),
                Mode::GrayAlpha => compose_row_gray_alpha(out_row, src_row, opacity, blend_mode),
                Mode::Indexed => compose_row_indexed(out_row, src_row, transparent),
                Mode::Gray => out_row.copy_from_slice(src_row),
            }
        }
    }

    match mode {
        Mode::Indexed => {
            let palette = doc
                .palette()
                .ok_or(ComposeError::NoTransparency { mode })?
                .clone();
            Ok(Image::from_bytes_indexed(
                rect.width,
                rect.height,
                palette,
                data,
            )?)
        }
        _ => Ok(Image::from_bytes(rect.width, rect.height, mode, data)?),
    }
}

fn compose_row_rgba(out: &mut [u8], src: &[u8], opacity: u8, mode: BlendMode) {
    let opaque_copy = opacity == 255 && mode == BlendMode::Normal;
    let (out, _) = out.as_chunks_mut::<4>();
    let (src, _) = src.as_chunks::<4>();
    for (d, s) in out.iter_mut().zip(src) {
        // Origen opaco con Normal y opacidad 255: `αs = 255`, `αr = 255` y `Cr = Cs`.
        if opaque_copy && s[3] == 255 {
            *d = *s;
            continue;
        }
        let (rgb, a) = composite_channels(
            [d[0], d[1], d[2]],
            d[3],
            [s[0], s[1], s[2]],
            s[3],
            opacity,
            mode,
        );
        *d = [rgb[0], rgb[1], rgb[2], a];
    }
}

fn compose_row_gray_alpha(out: &mut [u8], src: &[u8], opacity: u8, mode: BlendMode) {
    let (out, _) = out.as_chunks_mut::<2>();
    let (src, _) = src.as_chunks::<2>();
    for (d, s) in out.iter_mut().zip(src) {
        let (gray, a) = composite_channels([d[0]], d[1], [s[0]], s[1], opacity, mode);
        *d = [gray[0], a];
    }
}

/// Indexado (D06): sin opacidad ni mezcla; gana el índice de la capa más alta que no es el
/// transparente.
fn compose_row_indexed(out: &mut [u8], src: &[u8], transparent: Option<u8>) {
    for (d, &s) in out.iter_mut().zip(src) {
        if Some(s) != transparent {
            *d = s;
        }
    }
}
