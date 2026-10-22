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

pub const MODOS: [Mode; 4] = [Mode::Rgba, Mode::Indexed, Mode::Gray, Mode::GrayAlpha];

/// Los alfas de la spec: 0, 1, 128 y 255.
pub const ALFAS: [u8; 4] = [0, 1, 128, 255];

// ---------------------------------------------------------------------------------------------
// Imágenes de prueba
// ---------------------------------------------------------------------------------------------

/// Un color RGBA distinto según `k`, con los cuatro alfas por turno.
pub fn rgba_de(k: u32) -> Rgba {
    Rgba::new(
        (k.wrapping_mul(37) % 256) as u8,
        (k.wrapping_mul(91) % 256) as u8,
        (k.wrapping_mul(53).wrapping_add(17) % 256) as u8,
        ALFAS[(k % 4) as usize],
    )
}

/// Paleta de ocho entradas con una duplicada (la 4 es la 2), una de alfa 0 (la 5), una de alfa 1
/// (la 3), una de alfa 128 (la 1) y el índice transparente en la 7 (que guarda blanco opaco:
/// resuelto da alfa 0).
pub fn paleta() -> Palette {
    let mut paleta = Palette::from_colors([
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
    paleta
        .set_transparent_index(Some(7))
        .expect("la entrada 7 existe");
    paleta
}

/// Un píxel válido del modo, distinto según `k`.
pub fn pixel_de(modo: Mode, k: u32, entradas: usize) -> Pixel {
    match modo {
        Mode::Rgba => Pixel::Rgba(rgba_de(k)),
        Mode::Indexed => Pixel::Indexed((k as usize % entradas.max(1)) as u8),
        Mode::Gray => Pixel::Gray((k.wrapping_mul(29) % 256) as u8),
        Mode::GrayAlpha => Pixel::GrayAlpha {
            gray: (k.wrapping_mul(29) % 256) as u8,
            alpha: ALFAS[((k / 4) % 4) as usize],
        },
    }
}

/// Imagen de `ancho × alto` en `modo`, hecha de celdas de `celda × celda` píxeles de un mismo
/// valor (para que el relleno tenga regiones de varios píxeles y los alfas 0, 1, 128 y 255
/// aparezcan por todas partes).
pub fn imagen_de(modo: Mode, ancho: u32, alto: u32, celda: u32) -> Image {
    let celda = celda.max(1);
    let entradas = paleta().len();
    let valor = |x: u32, y: u32| pixel_de(modo, x / celda + 3 * (y / celda), entradas);
    let mut imagen = match modo {
        Mode::Indexed => Image::new_indexed(ancho, alto, paleta()).expect("imagen indexada"),
        _ => Image::new(ancho, alto, modo).expect("imagen"),
    };
    for y in 0..alto {
        for x in 0..ancho {
            imagen.set_pixel(x, y, valor(x, y)).expect("dentro");
        }
    }
    imagen
}

/// El valor con el que escribe el borrador, si el modo lo tiene.
pub fn borrado(imagen: &Image) -> Option<Pixel> {
    match imagen.mode() {
        Mode::Rgba => Some(Pixel::Rgba(Rgba::TRANSPARENT)),
        Mode::GrayAlpha => Some(Pixel::GrayAlpha { gray: 0, alpha: 0 }),
        Mode::Indexed => imagen
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
pub fn esperado(imagen: &Image) -> Vec<Color32> {
    to_rgba8(imagen)
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

/// Los píxeles de `r` de una textura de `ancho` de ancho, por filas.
pub fn recorte(pixeles: &[Color32], ancho: usize, r: TileRect) -> Vec<Color32> {
    let mut fuera = Vec::with_capacity(r.width as usize * r.height as usize);
    for y in r.y..r.y + r.height {
        let desde = y as usize * ancho + r.x as usize;
        fuera.extend_from_slice(&pixeles[desde..desde + r.width as usize]);
    }
    fuera
}

/// Modelo de un conjunto de bloques sucios: coordenadas de bloque.
pub type Modelo = BTreeSet<(u32, u32)>;

/// Los bloques de los píxeles dados (los que caen dentro de la imagen).
pub fn modelo_de_pixeles(pixeles: &[(u32, u32)], ancho: u32, alto: u32) -> Modelo {
    pixeles
        .iter()
        .filter(|&&(x, y)| x < ancho && y < alto)
        .map(|&(x, y)| (x / B, y / B))
        .collect()
}

/// Todos los bloques de una imagen.
pub fn modelo_lleno(ancho: u32, alto: u32) -> Modelo {
    let mut m = Modelo::new();
    for by in 0..alto.div_ceil(B) {
        for bx in 0..ancho.div_ceil(B) {
            m.insert((bx, by));
        }
    }
    m
}

/// Los bloques que corta el rectángulo `[x, x+ancho) × [y, y+alto)` (con signo), recortado antes a
/// la imagen de `iw × ih` (lo que cae fuera de la imagen no cuenta).
pub fn modelo_de_rect(x: i64, y: i64, ancho: u64, alto: u64, iw: u32, ih: u32) -> Modelo {
    let mut m = Modelo::new();
    let x0 = i128::from(x).max(0);
    let y0 = i128::from(y).max(0);
    let x1 = (i128::from(x) + i128::from(ancho)).min(i128::from(iw));
    let y1 = (i128::from(y) + i128::from(alto)).min(i128::from(ih));
    if ancho == 0 || alto == 0 || x0 >= x1 || y0 >= y1 {
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
pub fn corta(bx: u32, by: u32, r: TileRect) -> bool {
    let (x0, y0) = (bx * B, by * B);
    x0 < r.x + r.width && r.x < x0 + B && y0 < r.y + r.height && r.y < y0 + B
}

/// Las series de `runs` de un rectángulo: por cada fila de bloques, cada serie máxima de bloques
/// sucios contiguos que cortan `r`, en píxeles y recortada a `r`; por filas y por `x`.
pub fn runs_esperados(modelo: &Modelo, r: TileRect) -> Vec<TileRect> {
    let mut fuera = Vec::new();
    if r.width == 0 || r.height == 0 {
        return fuera;
    }
    let (bx_min, bx_max) = (r.x / B, (r.x + r.width - 1) / B);
    let (by_min, by_max) = (r.y / B, (r.y + r.height - 1) / B);
    for by in by_min..=by_max {
        let mut bx = bx_min;
        while bx <= bx_max {
            if modelo.contains(&(bx, by)) {
                let inicio = bx;
                while bx < bx_max && modelo.contains(&(bx + 1, by)) {
                    bx += 1;
                }
                let x0 = (inicio * B).max(r.x);
                let x1 = ((bx + 1) * B).min(r.x + r.width);
                let y0 = (by * B).max(r.y);
                let y1 = ((by + 1) * B).min(r.y + r.height);
                fuera.push(TileRect {
                    x: x0,
                    y: y0,
                    width: x1 - x0,
                    height: y1 - y0,
                });
            }
            bx += 1;
        }
    }
    fuera
}

// ---------------------------------------------------------------------------------------------
// Comandos a partir de una semilla (Q2 y Q3)
// ---------------------------------------------------------------------------------------------

/// `(tipo, a, b, c, d)`: el tipo elige la operación y el resto la parametriza.
pub type Semilla = (u8, u32, u32, u32, u32);

pub fn semilla() -> impl Strategy<Value = Semilla> {
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
pub enum Paso {
    Aplica(Command),
    Deshacer,
    Rehacer,
    /// La semilla no da una operación válida para esta imagen (p. ej. paleta en un gris).
    Nada,
}

/// Traduce la semilla en una operación sobre el estado actual del documento. Tipos: 0 trazo,
/// 1 línea, 2 relleno, 3 borrador, 4 tramos, 5 lote, 6 color de una entrada, 7 mover entrada,
/// 8 permutar la paleta, 9 añadir entrada, 10 quitar entrada, 11 índice transparente,
/// 12 rotar colores, 13 convertir de modo, 14 deshacer y 15 rehacer.
pub fn paso_de(doc: &Document, semilla: Semilla) -> Paso {
    let (tipo, a, b, c, d) = semilla;
    let id = ImageId::from_raw(0);
    let Some(imagen) = doc.image(id) else {
        return Paso::Nada;
    };
    let (w, h) = (imagen.width(), imagen.height());
    let modo = imagen.mode();
    let entradas = imagen.palette().map_or(0, |p| p.len());
    let indexada = modo == Mode::Indexed;
    let pixel = |k: u32| pixel_de(modo, k, entradas);
    let puntos = |n: u32| -> Vec<PixelEdit> {
        (0..n)
            .map(|i| PixelEdit {
                x: b.wrapping_add(i.wrapping_mul(7919)) % w,
                y: c.wrapping_add(i.wrapping_mul(104_729)) % h,
                pixel: pixel(d.wrapping_add(i)),
            })
            .collect()
    };
    let tramos = || -> Vec<Span> {
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
    let con = |c: Command| Paso::Aplica(c);
    match tipo % 16 {
        0 => con(Command::SetPixels {
            image: id,
            pixels: puntos(1 + a % 6),
        }),
        1 => {
            let a_ = ((b % (w + 4)) as i32 - 2, (c % (h + 4)) as i32 - 2);
            let b_ = ((d % (w + 4)) as i32 - 2, (a % (h + 4)) as i32 - 2);
            match line_command(doc, id, a_, b_, pixel(a)) {
                Ok(Some(cmd)) => con(cmd),
                _ => Paso::Nada,
            }
        }
        2 => match fill_command(doc, id, (b % w) as i32, (c % h) as i32, pixel(d)) {
            Ok(Some(cmd)) => con(cmd),
            _ => Paso::Nada,
        },
        3 => match borrado(imagen) {
            Some(pixel) => con(Command::SetPixels {
                image: id,
                pixels: puntos(1 + a % 6)
                    .into_iter()
                    .map(|e| PixelEdit { pixel, ..e })
                    .collect(),
            }),
            None => Paso::Nada,
        },
        4 => con(Command::FillSpans {
            image: id,
            spans: tramos(),
            pixel: pixel(a),
        }),
        5 => con(Command::Batch(vec![
            Command::SetPixels {
                image: id,
                pixels: puntos(1 + a % 3),
            },
            Command::FillSpans {
                image: id,
                spans: tramos(),
                pixel: pixel(a.wrapping_add(1)),
            },
        ])),
        6 | 7 | 8 | 10 | 11 | 12 if !indexada => Paso::Nada,
        9 if !indexada || entradas >= 256 => Paso::Nada,
        6 => con(Command::SetPaletteColor {
            image: id,
            index: (a as usize % entradas) as u8,
            color: rgba_de(b),
        }),
        7 => con(Command::MovePaletteEntry {
            image: id,
            from: (a as usize % entradas) as u8,
            to: (b as usize % entradas) as u8,
        }),
        8 => {
            let giro = a as usize % entradas;
            con(Command::PermutePalette {
                image: id,
                order: (0..entradas)
                    .map(|i| ((i + giro) % entradas) as u8)
                    .collect(),
            })
        }
        9 => con(Command::AddPaletteEntry {
            image: id,
            color: rgba_de(b),
        }),
        10 if entradas < 2 => Paso::Nada,
        10 => {
            let index = a as usize % entradas;
            let into = (index + 1 + b as usize % (entradas - 1)) % entradas;
            con(Command::RemovePaletteEntry {
                image: id,
                index: index as u8,
                into: into as u8,
            })
        }
        11 => con(Command::SetTransparentIndex {
            image: id,
            index: (a % 5 != 0).then(|| (b as usize % entradas) as u8),
        }),
        12 => {
            let start = a as usize % entradas;
            let len = 1 + b as usize % (entradas - start);
            con(Command::RotateColors {
                image: id,
                start: start as u8,
                len: len as u16,
                steps: (c % 7) as i32 - 3,
            })
        }
        13 => {
            let otros: Vec<Mode> = MODOS.iter().copied().filter(|&m| m != modo).collect();
            con(Command::ConvertMode {
                image: id,
                target: otros[a as usize % otros.len()],
                allowed: Allowed {
                    luma: true,
                    drop_alpha: Some([128, 128, 128]),
                },
            })
        }
        14 => Paso::Deshacer,
        _ => Paso::Rehacer,
    }
}

/// Una imagen aleatoria: modo, tamaño `1..=max` y celda.
pub fn imagen_aleatoria(modo: u8, ancho: u32, alto: u32, celda: u32) -> Image {
    imagen_de(MODOS[usize::from(modo) % 4], ancho, alto, celda)
}

/// El id de la imagen principal de un documento.
pub fn principal(_doc: &Document) -> ImageId {
    ImageId::from_raw(0)
}
