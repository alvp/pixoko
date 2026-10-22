//! Cota de lectura de `read_png` frente a otros compresores (specs/hito-2-png-flujo.md, unidad 13):
//! fila 7 y propiedad 3.
//!
//! **Este módulo SÍ usa el crate `png`** (dependencia directa de `px-formats`) como codificador,
//! al contrario que el resto de `tests/png.rs` («sin `png`»): así se prueban los flujos DEFLATE de
//! miniz_oxide (por flate2, niveles 1 a 9), de fdeflate (`FdeflateUltraFast`) y los bloques sin
//! comprimir (`NoCompression`), con los filtros `NoFilter`, `Sub`, `Up` y `Adaptive`, y además
//! `Z(needed)` a mano. Ninguna combinación debe rechazarse.
//!
//! Estado hoy (antes de la cota): todo en VERDE, porque el decodificador actual acepta todos esos
//! ficheros; son la red de seguridad contra una cota demasiado agresiva.

use super::*;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;

use super::max_ratio::{file, plte_black, z};

// ---------------------------------------------------------------------------------------
// Formas de comprimir y filtros.
// ---------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Compressor {
    /// Niveles 1 a 9 de miniz_oxide (por flate2).
    Level(u8),
    /// fdeflate.
    Fdeflate,
    /// Bloques sin comprimir.
    Without,
    /// `Z(needed)` hecho a mano (no pasa por el codificador; el filtro no cuenta).
    Z,
}

const COMPRESSORS_PNG: [Compressor; 11] = [
    Compressor::Level(1),
    Compressor::Level(2),
    Compressor::Level(3),
    Compressor::Level(4),
    Compressor::Level(5),
    Compressor::Level(6),
    Compressor::Level(7),
    Compressor::Level(8),
    Compressor::Level(9),
    Compressor::Fdeflate,
    Compressor::Without,
];

fn filters() -> [png::Filter; 4] {
    [
        png::Filter::NoFilter,
        png::Filter::Sub,
        png::Filter::Up,
        png::Filter::Adaptive,
    ]
}

fn compression(c: Compressor) -> png::DeflateCompression {
    match c {
        Compressor::Level(n) => png::DeflateCompression::Level(n),
        Compressor::Fdeflate => png::DeflateCompression::FdeflateUltraFast,
        Compressor::Without => png::DeflateCompression::NoCompression,
        Compressor::Z => unreachable!("Z no pasa por el codificador"),
    }
}

// ---------------------------------------------------------------------------------------
// Imagen cruda y su codificación con el crate `png`.
// ---------------------------------------------------------------------------------------

/// Imagen ya empaquetada tal como la espera `Encoder::write_image_data`.
struct Raw {
    width: u32,
    height: u32,
    color: png::ColorType,
    prof: png::BitDepth,
    /// Filas empaquetadas, una detrás de otra.
    data: Vec<u8>,
    /// PLTE (RGB por entrada) si es indexada.
    palette: Option<Vec<u8>>,
    /// tRNS si lo lleva.
    trns: Option<Vec<u8>>,
}

fn encodes(img: &Raw, comp: Compressor, filter: png::Filter) -> Vec<u8> {
    let mut output = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut output, img.width, img.height);
        enc.set_color(img.color);
        enc.set_depth(img.prof);
        enc.set_deflate_compression(compression(comp));
        enc.set_filter(filter);
        if let Some(p) = &img.palette {
            enc.set_palette(p.as_slice());
        }
        if let Some(t) = &img.trns {
            enc.set_trns(t.as_slice());
        }
        let mut w = enc.write_header().unwrap();
        w.write_image_data(&img.data).unwrap();
        w.finish().unwrap();
    }
    output
}

// ---------------------------------------------------------------------------------------
// Fila 7: un solo color y patrones repetidos en gris 8, indexado 8 y RGBA 8.
// ---------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Shape {
    Gray,
    TestIndexed,
    Rgba,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Content {
    UnColor,
    Patterns,
}

/// Lado de la imagen: 1024² en RGBA y 2048² con un byte por píxel (4 MiB de datos sin
/// comprimir). Con menos, la razón de los niveles 6 a 9 de miniz_oxide se queda por debajo de 500
/// (medido: 1 MiB da ≈ 350; 4 MiB da 500 a 660).
fn side(shape: Shape) -> u32 {
    match shape {
        Shape::Rgba => 1024,
        Shape::Gray | Shape::TestIndexed => 2048,
    }
}

/// Paleta de 16 entradas, algunas con alfa no trivial, para el indexado.
fn palette_16() -> Vec<Rgba> {
    (0..16u8)
        .map(|i| {
            Rgba::new(
                i * 16,
                255 - i * 9,
                i * 5 + 3,
                if i % 4 == 0 { 100 + i } else { 255 },
            )
        })
        .collect()
}

/// Valor del píxel (x, y) como 4 bytes RGBA con alfa no trivial, o su índice / gris.
fn patron(content: Content, x: usize, y: usize) -> usize {
    match content {
        Content::UnColor => 5,
        // Franjas verticales de 16 y horizontales de 64 combinadas con bloques de 8×8 que se
        // repiten cada 128 píxeles.
        Content::Patterns => {
            (x / 16 + (y / 64) * 3 + usize::from((x % 128 < 8) && (y % 128 < 8)) * 7) % 16
        }
    }
}

/// Imagen cruda y la `Image` que `read_png` debe devolver, para esa forma y ese contenido.
fn builds(shape: Shape, content: Content) -> (Raw, Image) {
    let n = side(shape);
    let (w, h) = (n as usize, n as usize);
    let colors = palette_16();
    match shape {
        Shape::Gray => {
            let data: Vec<u8> = (0..w * h)
                .map(|i| (patron(content, i % w, i / w) * 16 + 9) as u8)
                .collect();
            let expected = Image::from_bytes(n, n, Mode::Gray, data.clone()).unwrap();
            let raw = Raw {
                width: n,
                height: n,
                color: png::ColorType::Grayscale,
                prof: png::BitDepth::Eight,
                data,
                palette: None,
                trns: None,
            };
            (raw, expected)
        }
        Shape::TestIndexed => {
            let data: Vec<u8> = (0..w * h)
                .map(|i| patron(content, i % w, i / w) as u8)
                .collect();
            let palette = Palette::from_colors(colors.iter().copied()).unwrap();
            let expected = Image::from_bytes_indexed(n, n, palette, data.clone()).unwrap();
            let rgb: Vec<u8> = colors.iter().flat_map(|c| [c.r, c.g, c.b]).collect();
            // tRNS: los alfas de las entradas hasta la última no opaca.
            let last = colors.iter().rposition(|c| c.a != 255).unwrap();
            let trns: Vec<u8> = colors[..=last].iter().map(|c| c.a).collect();
            let raw = Raw {
                width: n,
                height: n,
                color: png::ColorType::Indexed,
                prof: png::BitDepth::Eight,
                data,
                palette: Some(rgb),
                trns: Some(trns),
            };
            (raw, expected)
        }
        Shape::Rgba => {
            let mut data = Vec::with_capacity(w * h * 4);
            for i in 0..w * h {
                let c = colors[patron(content, i % w, i / w)];
                // Alfa no trivial: la de la paleta si no es opaca y, si lo es, 200.
                let a = if c.a == 255 { 200 } else { c.a };
                data.extend([c.r, c.g, c.b, a]);
            }
            let expected = Image::from_bytes(n, n, Mode::Rgba, data.clone()).unwrap();
            let raw = Raw {
                width: n,
                height: n,
                color: png::ColorType::Rgba,
                prof: png::BitDepth::Eight,
                data,
                palette: None,
                trns: None,
            };
            (raw, expected)
        }
    }
}

/// Una combinación a probar: (forma, contenido, compresor, filtro).
type Combination = (Shape, Content, Compressor, png::Filter);

/// Ejecuta las combinaciones repartidas en hilos (el perfil de test no optimiza) y hace pánico
/// con la combinación que falle.
fn in_parallel(jobs: &[Combination]) {
    let next = AtomicUsize::new(0);
    let threads = thread::available_parallelism()
        .map_or(4, |n| n.get())
        .min(8);
    thread::scope(|s| {
        let handlers: Vec<_> = (0..threads)
            .map(|_| {
                s.spawn(|| {
                    // Cada hilo conserva la última imagen construida para no regenerarla.
                    let mut cache: Option<((Shape, Content), Raw, Image)> = None;
                    loop {
                        let i = next.fetch_add(1, Ordering::SeqCst);
                        let Some(&(shape, content, comp, filter)) = jobs.get(i) else {
                            break;
                        };
                        if cache.as_ref().map(|c| c.0) != Some((shape, content)) {
                            let (c, e) = builds(shape, content);
                            cache = Some(((shape, content), c, e));
                        }
                        let (_, raw, expected) = cache.as_ref().unwrap();
                        let name_2 = format!("{shape:?} {content:?} {comp:?} {filter:?}");
                        if comp == Compressor::Z {
                            // Z(needed): la imagen es toda cero (con PLTE en indexado).
                            let (color, bpp) = match shape {
                                Shape::Gray => (0u8, 8u64),
                                Shape::TestIndexed => (3, 8),
                                Shape::Rgba => (6, 32),
                            };
                            let n = raw.width;
                            let needed = super::max_ratio::necessary(n, n, bpp, false);
                            let flow = z(needed);
                            let before = if color == 3 {
                                vec![plte_black(256)]
                            } else {
                                vec![]
                            };
                            let bytes = file(n, n, 8, color, false, &before, Some(&flow));
                            let r = read_png(&bytes)
                                .unwrap_or_else(|e| panic!("{name_2}: rechazado: {e:?}"));
                            assert!(
                                r.image.bytes().iter().all(|&b| b == 0),
                                "{name_2}: la imagen no es toda cero"
                            );
                            continue;
                        }
                        let bytes = encodes(raw, comp, filter);
                        if content == Content::UnColor
                            && matches!(comp, Compressor::Level(6 | 9))
                            && filter == png::Filter::Up
                        {
                            // El caso extremo de verdad: razón de expansión por encima de 500.
                            assert!(
                                bytes.len() * 500 < raw.data.len(),
                                "{name_2}: {} bytes para {}",
                                bytes.len(),
                                raw.data.len()
                            );
                        }
                        let r = read_png(&bytes)
                            .unwrap_or_else(|e| panic!("{name_2}: rechazado: {e:?}"));
                        assert!(
                            r.image == *expected,
                            "{name_2}: la imagen leída es distinta"
                        );
                    }
                })
            })
            .collect();
        for m in handlers {
            if let Err(e) = m.join() {
                std::panic::resume_unwind(e);
            }
        }
    });
}

/// Rejilla de la fila 7: `levels` de miniz_oxide, fdeflate y sin comprimir con los cuatro filtros,
/// y `Z(needed)` una vez por forma (no usa filtro ni contenido).
fn grid(levels: &[u8]) -> Vec<Combination> {
    let mut jobs: Vec<Combination> = Vec::new();
    for shape in [Shape::Gray, Shape::TestIndexed, Shape::Rgba] {
        for content in [Content::UnColor, Content::Patterns] {
            let comps = levels
                .iter()
                .map(|&n| Compressor::Level(n))
                .chain([Compressor::Fdeflate, Compressor::Without]);
            for comp in comps {
                for filter in filters() {
                    jobs.push((shape, content, comp, filter));
                }
            }
        }
        jobs.push((
            shape,
            Content::UnColor,
            Compressor::Z,
            png::Filter::NoFilter,
        ));
    }
    jobs
}

/// Suite: niveles 1, 6 y 9 (el 1 es el más flojo, el 6 el de por defecto y el 9 el más fuerte).
#[test]
fn row7_no_compressor_or_filter_is_rejected() {
    in_parallel(&grid(&[1, 6, 9]));
}

/// Rejilla completa de la spec (niveles 1 a 9 × cuatro filtros, tamaños de la spec): se ejecuta al
/// cerrar con `cargo test --release -p px-formats -- --ignored`.
#[test]
#[ignore = "rejilla completa: lenta sin optimizar; se ejecuta al cerrar con --release -- --ignored"]
fn row7_full_grid_levels_1_to_9() {
    in_parallel(&grid(&[1, 2, 3, 4, 5, 6, 7, 8, 9]));
}

// ---------------------------------------------------------------------------------------
// Propiedad 3: ningún compresor es rechazado (cualquier tipo y profundidad).
// ---------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug)]
enum Fill {
    UnColor,
    Bands,
    Noise,
}

/// (tipo de color, profundidad, canales) que admite el codificador.
fn types() -> Vec<(png::ColorType, png::BitDepth, usize)> {
    use png::BitDepth::{Eight, Four, One, Sixteen, Two};
    use png::ColorType::{Grayscale, GrayscaleAlpha, Indexed, Rgb, Rgba};
    let mut v = Vec::new();
    for p in [One, Two, Four, Eight, Sixteen] {
        v.push((Grayscale, p, 1));
    }
    for p in [Eight, Sixteen] {
        v.push((GrayscaleAlpha, p, 2));
        v.push((Rgb, p, 3));
        v.push((Rgba, p, 4));
    }
    for p in [One, Two, Four, Eight] {
        v.push((Indexed, p, 1));
    }
    v
}

/// Muestra (x, y, canal) del relleno, de `bits` bits, derivada de `seed`.
fn swatch(fill: Fill, seed: u64, bits: u32, x: usize, y: usize, c: usize) -> u16 {
    let mask = ((1u32 << bits) - 1) as u16;
    let blend = |a: u64| {
        let mut s = seed ^ a.wrapping_mul(0x9E37_79B9_7F4A_7C15);
        s ^= s >> 30;
        s = s.wrapping_mul(0xBF58_476D_1CE4_E5B9);
        s ^= s >> 27;
        s = s.wrapping_mul(0x94D0_49BB_1331_11EB);
        s ^ (s >> 31)
    };
    let v = match fill {
        Fill::UnColor => blend(c as u64),
        Fill::Bands => blend((x / 5 + (y / 3) * 7 + c * 1000) as u64 % 11),
        Fill::Noise => blend(((y * 64 + x) * 4 + c) as u64 + 77),
    };
    (v as u16) & mask
}

fn raw_random(
    width: u32,
    height: u32,
    kind: (png::ColorType, png::BitDepth, usize),
    fill: Fill,
    seed: u64,
) -> Raw {
    let (color, prof, channels) = kind;
    let bits = prof as u32;
    let n_palette = 1usize << bits;
    let indexed = color == png::ColorType::Indexed;
    let mut data = Vec::new();
    for y in 0..height as usize {
        let row: Vec<u16> = (0..width as usize)
            .flat_map(|x| (0..channels).map(move |c| (x, c)))
            .map(|(x, c)| swatch(fill, seed, bits, x, y, c))
            .collect();
        data.extend(packs(&row, bits as u8));
    }
    let (palette, trns) = if indexed {
        let rgb: Vec<u8> = (0..n_palette)
            .flat_map(|i| [i as u8, (i * 7) as u8, (i * 13) as u8])
            .collect();
        (Some(rgb), Some(vec![0u8, 128]))
    } else {
        (None, None)
    };
    Raw {
        width,
        height,
        color,
        prof,
        data,
        palette,
        trns,
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    // Propiedad 3: `read_png` es `Ok` para cualquier imagen que el codificador admita y cualquier
    // forma de comprimir de la fila 7, con y sin `Z(needed)` a mano.
    #[test]
    fn p3_no_compressor_is_rejected(
        width in 1u32..=64,
        height in 1u32..=64,
        kind in select(types()),
        fill in select(vec![Fill::UnColor, Fill::Bands, Fill::Noise]),
        seed in any::<u64>(),
        compressor in select(COMPRESSORS_PNG.to_vec()),
        filter in select(filters().to_vec()),
    ) {
        let img = raw_random(width, height, kind, fill, seed);
        let bytes = encodes(&img, compressor, filter);
        let r = read_png(&bytes);
        prop_assert!(
            r.is_ok(),
            "{}x{} {:?} {:?} {:?} {:?}: {:?}",
            width, height, kind, compressor, filter, fill, r.as_ref().err()
        );
        let r = r.unwrap();
        prop_assert_eq!((r.image.width(), r.image.height()), (width, height));
    }

    // Propiedad 3 con `Z(needed)` a mano en cualquier tipo y profundidad.
    #[test]
    fn p3_z_by_hand_is_never_rejected(
        width in 1u32..=64,
        height in 1u32..=64,
        kind in select(types()),
    ) {
        let (color, prof, channels) = kind;
        let color_png = match color {
            png::ColorType::Grayscale => 0u8,
            png::ColorType::Rgb => 2,
            png::ColorType::Indexed => 3,
            png::ColorType::GrayscaleAlpha => 4,
            png::ColorType::Rgba => 6,
        };
        let bits = prof as u8;
        let bpp = (channels as u64) * u64::from(bits);
        let needed = super::max_ratio::necessary(width, height, bpp, false);
        let before = if color_png == 3 { vec![plte_black(1 << bits)] } else { vec![] };
        let bytes = file(width, height, bits, color_png, false, &before, Some(&z(needed)));
        let r = read_png(&bytes);
        prop_assert!(r.is_ok(), "{:?}", r.as_ref().err());
    }
}
