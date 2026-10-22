//! Ayudantes de `tiles.rs` y `tiles_canvas.rs` (unidad 14a-2, specs/hito-2-zoom-baldosas.md):
//! imágenes de prueba en los cuatro modos, comandos de documento a partir de una semilla y los
//! oráculos de la textura, de los bloques sucios y de las series (`runs`).
//!
//! Los oráculos están escritos **aquí**, con enteros y conjuntos sencillos, y no llaman a
//! `BlockSet`, `Tiling` ni `dirty_of`: la implementación se compara con ellos, no consigo misma.
//!
//! Va en un directorio (`tests/tiles_support/mod.rs`) y no como `tests/tiles_support.rs` para que
//! cargo no lo trate como un ejecutable de pruebas más; los dos ficheros lo incluyen con
//! `mod tiles_support;`, como `common`. Cada uno usa solo una parte, de ahí el `allow`.
#![allow(dead_code)]

use std::collections::BTreeSet;

use egui::Color32;
use proptest::prelude::*;
use px_core::{
    Allowed, Command, Document, Image, ImageId, Mode, Palette, Pixel, PixelEdit, Rgba, Span,
};
use px_raster::{fill_command, line_command};
use px_ui::texture::to_rgba8;
use px_ui::tiles::TileRect;

/// Lado de los bloques de trabajo y de la región sucia (`px_ui::tiles::BLOCK`), repetido aquí para
/// que los oráculos no dependan de él.
pub const B: u32 = 64;

pub const TEST_MODES: [Mode; 4] = [Mode::Rgba, Mode::Indexed, Mode::Gray, Mode::GrayAlpha];

/// Los alfas de la spec: 0, 1, 128 y 255.
pub const ALPHAS: [u8; 4] = [0, 1, 128, 255];

// ---------------------------------------------------------------------------------------------
// Imágenes de prueba
// ---------------------------------------------------------------------------------------------

/// Un color RGBA distinto según `k`, con los cuatro alfas por turno.
pub fn rgba_of(k: u32) -> Rgba {
    Rgba::new(
        (k.wrapping_mul(37) % 256) as u8,
        (k.wrapping_mul(91) % 256) as u8,
        (k.wrapping_mul(53).wrapping_add(17) % 256) as u8,
        ALPHAS[(k % 4) as usize],
    )
}

/// Paleta de ocho entradas con una duplicada (la 4 es la 2), una de alfa 0 (la 5), una de alfa 1
/// (la 3), una de alfa 128 (la 1) y el índice transparente en la 7 (que guarda blanco opaco:
/// resuelto da alfa 0).
pub fn palette() -> Palette {
    let mut palette = Palette::from_colors([
        Rgba::opaque(200, 20, 20),
        Rgba::new(20, 20, 200, 128),
        Rgba::opaque(20, 200, 20),
        Rgba::new(90, 90, 90, 1),
        Rgba::opaque(20, 200, 20),
        Rgba::new(10, 220, 230, 0),
        Rgba::opaque(0, 0, 255),
        Rgba::opaque(255, 255, 255),
    ])
    .expect("paleta de ocho entradas");
    palette
        .set_transparent_index(Some(7))
        .expect("la entrada 7 existe");
    palette
}

/// Un píxel válido del modo, distinto según `k`.
pub fn pixel_of(mode_2: Mode, k: u32, entries: usize) -> Pixel {
    match mode_2 {
        Mode::Rgba => Pixel::Rgba(rgba_of(k)),
        Mode::Indexed => Pixel::Indexed((k as usize % entries.max(1)) as u8),
        Mode::Gray => Pixel::Gray((k.wrapping_mul(29) % 256) as u8),
        Mode::GrayAlpha => Pixel::GrayAlpha {
            gray: (k.wrapping_mul(29) % 256) as u8,
            alpha: ALPHAS[((k / 4) % 4) as usize],
        },
    }
}

/// Imagen de `width_2 × height_2` en `mode_2`, hecha de celdas de `cel × cel` píxeles de un mismo
/// valor (para que el relleno tenga regiones de varios píxeles y los alfas 0, 1, 128 y 255
/// aparezcan por todas partes).
pub fn image_of(mode_2: Mode, width_2: u32, height_2: u32, cel: u32) -> Image {
    let cel = cel.max(1);
    let entries = palette().len();
    let value = |x: u32, y: u32| pixel_of(mode_2, x / cel + 3 * (y / cel), entries);
    let mut image_2 = match mode_2 {
        Mode::Indexed => Image::new_indexed(width_2, height_2, palette()).expect("imagen indexada"),
        _ => Image::new(width_2, height_2, mode_2).expect("imagen"),
    };
    for y in 0..height_2 {
        for x in 0..width_2 {
            image_2.set_pixel(x, y, value(x, y)).expect("dentro");
        }
    }
    image_2
}

/// El valor con el que escribe el borrador, si el modo lo tiene.
pub fn deleted(image_2: &Image) -> Option<Pixel> {
    match image_2.mode() {
        Mode::Rgba => Some(Pixel::Rgba(Rgba::TRANSPARENT)),
        Mode::GrayAlpha => Some(Pixel::GrayAlpha { gray: 0, alpha: 0 }),
        Mode::Indexed => image_2
            .palette()
            .and_then(|p| p.transparent_index())
            .map(Pixel::Indexed),
        Mode::Gray => None,
    }
}

// ---------------------------------------------------------------------------------------------
// Oráculos
// ---------------------------------------------------------------------------------------------

/// La textura esperada de la imagen entera: `Color32::from_rgba_unmultiplied` de cada píxel de
/// `to_rgba8` (el «oráculo de la textura» de la spec), por filas.
pub fn expected_2(image_2: &Image) -> Vec<Color32> {
    to_rgba8(image_2)
        .as_chunks::<4>()
        .0
        .iter()
        .map(|c| Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3]))
        .collect()
}

/// El texel de un color con alfa directo, como lo guarda egui.
pub fn texel(color: Rgba) -> Color32 {
    Color32::from_rgba_unmultiplied(color.r, color.g, color.b, color.a)
}

/// Los píxeles de `r` de una textura de `width_2` de ancho, por filas.
pub fn clip_2(pixels_2: &[Color32], width_2: usize, r: TileRect) -> Vec<Color32> {
    let mut outside = Vec::with_capacity(r.width as usize * r.height as usize);
    for y in r.y..r.y + r.height {
        let from_2 = y as usize * width_2 + r.x as usize;
        outside.extend_from_slice(&pixels_2[from_2..from_2 + r.width as usize]);
    }
    outside
}

/// Modelo de un conjunto de bloques sucios: coordenadas de bloque.
pub type Model = BTreeSet<(u32, u32)>;

/// Los bloques de los píxeles dados (los que caen dentro de la imagen).
pub fn model_of_pixels(pixels_2: &[(u32, u32)], width_2: u32, height_2: u32) -> Model {
    pixels_2
        .iter()
        .filter(|&&(x, y)| x < width_2 && y < height_2)
        .map(|&(x, y)| (x / B, y / B))
        .collect()
}

/// Todos los bloques de una imagen.
pub fn model_full(width_2: u32, height_2: u32) -> Model {
    let mut m = Model::new();
    for by in 0..height_2.div_ceil(B) {
        for bx in 0..width_2.div_ceil(B) {
            m.insert((bx, by));
        }
    }
    m
}

/// Los bloques que corta el rectángulo `[x, x+width_2) × [y, y+height_2)` (con signo), recortado antes a
/// la imagen de `iw × ih` (lo que cae fuera de la imagen no cuenta).
pub fn model_of_rect(x: i64, y: i64, width_2: u64, height_2: u64, iw: u32, ih: u32) -> Model {
    let mut m = Model::new();
    let x0 = i128::from(x).max(0);
    let y0 = i128::from(y).max(0);
    let x1 = (i128::from(x) + i128::from(width_2)).min(i128::from(iw));
    let y1 = (i128::from(y) + i128::from(height_2)).min(i128::from(ih));
    if width_2 == 0 || height_2 == 0 || x0 >= x1 || y0 >= y1 {
        return m;
    }
    for by in 0..ih.div_ceil(B) {
        for bx in 0..iw.div_ceil(B) {
            let (bx0, by0) = (i128::from(bx * B), i128::from(by * B));
            let (bx1, by1) = (bx0 + i128::from(B), by0 + i128::from(B));
            if bx0 < x1 && x0 < bx1 && by0 < y1 && y0 < by1 {
                m.insert((bx, by));
            }
        }
    }
    m
}

/// Cuántos bloques del modelo cortan el rectángulo.
pub fn short(bx: u32, by: u32, r: TileRect) -> bool {
    let (x0, y0) = (bx * B, by * B);
    x0 < r.x + r.width && r.x < x0 + B && y0 < r.y + r.height && r.y < y0 + B
}

/// Las series de `runs` de un rectángulo: por cada fila de bloques, cada serie máxima de bloques
/// sucios contiguos que cortan `r`, en píxeles y recortada a `r`; por filas y por `x`.
pub fn runs_expected(model: &Model, r: TileRect) -> Vec<TileRect> {
    let mut outside = Vec::new();
    if r.width == 0 || r.height == 0 {
        return outside;
    }
    let (bx_min, bx_max) = (r.x / B, (r.x + r.width - 1) / B);
    let (by_min, by_max) = (r.y / B, (r.y + r.height - 1) / B);
    for by in by_min..=by_max {
        let mut bx = bx_min;
        while bx <= bx_max {
            if model.contains(&(bx, by)) {
                let start_2 = bx;
                while bx < bx_max && model.contains(&(bx + 1, by)) {
                    bx += 1;
                }
                let x0 = (start_2 * B).max(r.x);
                let x1 = ((bx + 1) * B).min(r.x + r.width);
                let y0 = (by * B).max(r.y);
                let y1 = ((by + 1) * B).min(r.y + r.height);
                outside.push(TileRect {
                    x: x0,
                    y: y0,
                    width: x1 - x0,
                    height: y1 - y0,
                });
            }
            bx += 1;
        }
    }
    outside
}

// ---------------------------------------------------------------------------------------------
// Comandos a partir de una semilla (Q2 y Q3)
// ---------------------------------------------------------------------------------------------

/// `(kind, a, b, c, d)`: el tipo elige la operación y el resto la parametriza.
pub type Seed = (u8, u32, u32, u32, u32);

pub fn seed() -> impl Strategy<Value = Seed> {
    (
        0u8..16,
        any::<u32>(),
        any::<u32>(),
        any::<u32>(),
        any::<u32>(),
    )
}

/// Lo que una semilla pide.
#[derive(Debug)]
pub enum Step {
    Applies(Command),
    Undo,
    Redo,
    /// La semilla no da una operación válida para esta imagen (p. ej. paleta en un gris).
    Nothing,
}

/// Traduce la semilla en una operación sobre el estado actual del documento. Tipos: 0 trazo,
/// 1 línea, 2 relleno, 3 borrador, 4 tramos, 5 lote, 6 color de una entrada, 7 mover entrada,
/// 8 permutar la paleta, 9 añadir entrada, 10 quitar entrada, 11 índice transparente,
/// 12 rotar colores, 13 convertir de modo, 14 deshacer y 15 rehacer.
pub fn step_of(doc: &Document, seed: Seed) -> Step {
    let (kind, a, b, c, d) = seed;
    let id = ImageId::from_raw(0);
    let Some(image_2) = doc.image(id) else {
        return Step::Nothing;
    };
    let (w, h) = (image_2.width(), image_2.height());
    let mode_2 = image_2.mode();
    let entries = image_2.palette().map_or(0, |p| p.len());
    let indexed = mode_2 == Mode::Indexed;
    let pixel = |k: u32| pixel_of(mode_2, k, entries);
    let points = |n: u32| -> Vec<PixelEdit> {
        (0..n)
            .map(|i| PixelEdit {
                x: b.wrapping_add(i.wrapping_mul(7919)) % w,
                y: c.wrapping_add(i.wrapping_mul(104_729)) % h,
                pixel: pixel(d.wrapping_add(i)),
            })
            .collect()
    };
    let spans_2 = || -> Vec<Span> {
        let y0 = b % h;
        let x0 = c % w;
        let x1 = x0 + 1 + d % (w - x0);
        let mut v = vec![Span {
            y: y0,
            x_start: x0,
            x_end: x1,
        }];
        if y0 + 1 < h {
            v.push(Span {
                y: y0 + 1,
                x_start: 0,
                x_end: 1 + (d / 7) % w,
            });
        }
        v
    };
    let with = |c: Command| Step::Applies(c);
    match kind % 16 {
        0 => with(Command::SetPixels {
            image: id,
            pixels: points(1 + a % 6),
        }),
        1 => {
            let a_ = ((b % (w + 4)) as i32 - 2, (c % (h + 4)) as i32 - 2);
            let b_ = ((d % (w + 4)) as i32 - 2, (a % (h + 4)) as i32 - 2);
            match line_command(doc, id, a_, b_, pixel(a)) {
                Ok(Some(cmd)) => with(cmd),
                _ => Step::Nothing,
            }
        }
        2 => match fill_command(doc, id, (b % w) as i32, (c % h) as i32, pixel(d)) {
            Ok(Some(cmd)) => with(cmd),
            _ => Step::Nothing,
        },
        3 => match deleted(image_2) {
            Some(pixel) => with(Command::SetPixels {
                image: id,
                pixels: points(1 + a % 6)
                    .into_iter()
                    .map(|e| PixelEdit { pixel, ..e })
                    .collect(),
            }),
            None => Step::Nothing,
        },
        4 => with(Command::FillSpans {
            image: id,
            spans: spans_2(),
            pixel: pixel(a),
        }),
        5 => with(Command::Batch(vec![
            Command::SetPixels {
                image: id,
                pixels: points(1 + a % 3),
            },
            Command::FillSpans {
                image: id,
                spans: spans_2(),
                pixel: pixel(a.wrapping_add(1)),
            },
        ])),
        6 | 7 | 8 | 10 | 11 | 12 if !indexed => Step::Nothing,
        9 if !indexed || entries >= 256 => Step::Nothing,
        6 => with(Command::SetPaletteColor {
            image: id,
            index: (a as usize % entries) as u8,
            color: rgba_of(b),
        }),
        7 => with(Command::MovePaletteEntry {
            image: id,
            from: (a as usize % entries) as u8,
            to: (b as usize % entries) as u8,
        }),
        8 => {
            let turn = a as usize % entries;
            with(Command::PermutePalette {
                image: id,
                order: (0..entries).map(|i| ((i + turn) % entries) as u8).collect(),
            })
        }
        9 => with(Command::AddPaletteEntry {
            image: id,
            color: rgba_of(b),
        }),
        10 if entries < 2 => Step::Nothing,
        10 => {
            let index = a as usize % entries;
            let into = (index + 1 + b as usize % (entries - 1)) % entries;
            with(Command::RemovePaletteEntry {
                image: id,
                index: index as u8,
                into: into as u8,
            })
        }
        11 => with(Command::SetTransparentIndex {
            image: id,
            index: (a % 5 != 0).then(|| (b as usize % entries) as u8),
        }),
        12 => {
            let start = a as usize % entries;
            let len = 1 + b as usize % (entries - start);
            with(Command::RotateColors {
                image: id,
                start: start as u8,
                len: len as u16,
                steps: (c % 7) as i32 - 3,
            })
        }
        13 => {
            let other: Vec<Mode> = TEST_MODES
                .iter()
                .copied()
                .filter(|&m| m != mode_2)
                .collect();
            with(Command::ConvertMode {
                image: id,
                target: other[a as usize % other.len()],
                allowed: Allowed {
                    luma: true,
                    drop_alpha: Some([128, 128, 128]),
                },
            })
        }
        14 => Step::Undo,
        _ => Step::Redo,
    }
}

/// Una imagen aleatoria: modo, tamaño `1..=max` y celda.
pub fn image_random(mode_2: u8, width_2: u32, height_2: u32, cel: u32) -> Image {
    image_of(TEST_MODES[usize::from(mode_2) % 4], width_2, height_2, cel)
}

/// El id de la imagen principal de un documento.
pub fn main(_doc: &Document) -> ImageId {
    ImageId::from_raw(0)
}
