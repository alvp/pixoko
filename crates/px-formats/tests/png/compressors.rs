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

use super::max_ratio::{fichero, plte_negra, z};

// ---------------------------------------------------------------------------------------
// Formas de comprimir y filtros.
// ---------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Compresor {
    /// Niveles 1 a 9 de miniz_oxide (por flate2).
    Nivel(u8),
    /// fdeflate.
    Fdeflate,
    /// Bloques sin comprimir.
    Sin,
    /// `Z(needed)` hecho a mano (no pasa por el codificador; el filtro no cuenta).
    Z,
}

const COMPRESORES_PNG: [Compresor; 11] = [
    Compresor::Nivel(1),
    Compresor::Nivel(2),
    Compresor::Nivel(3),
    Compresor::Nivel(4),
    Compresor::Nivel(5),
    Compresor::Nivel(6),
    Compresor::Nivel(7),
    Compresor::Nivel(8),
    Compresor::Nivel(9),
    Compresor::Fdeflate,
    Compresor::Sin,
];

fn filtros() -> [png::Filter; 4] {
    [
        png::Filter::NoFilter,
        png::Filter::Sub,
        png::Filter::Up,
        png::Filter::Adaptive,
    ]
}

fn compresion(c: Compresor) -> png::DeflateCompression {
    match c {
        Compresor::Nivel(n) => png::DeflateCompression::Level(n),
        Compresor::Fdeflate => png::DeflateCompression::FdeflateUltraFast,
        Compresor::Sin => png::DeflateCompression::NoCompression,
        Compresor::Z => unreachable!("Z no pasa por el codificador"),
    }
}

// ---------------------------------------------------------------------------------------
// Imagen cruda y su codificación con el crate `png`.
// ---------------------------------------------------------------------------------------

/// Imagen ya empaquetada tal como la espera `Encoder::write_image_data`.
struct Cruda {
    ancho: u32,
    alto: u32,
    color: png::ColorType,
    prof: png::BitDepth,
    /// Filas empaquetadas, una detrás de otra.
    datos: Vec<u8>,
    /// PLTE (RGB por entrada) si es indexada.
    paleta: Option<Vec<u8>>,
    /// tRNS si lo lleva.
    trns: Option<Vec<u8>>,
}

fn codifica(img: &Cruda, comp: Compresor, filtro: png::Filter) -> Vec<u8> {
    let mut salida = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut salida, img.ancho, img.alto);
        enc.set_color(img.color);
        enc.set_depth(img.prof);
        enc.set_deflate_compression(compresion(comp));
        enc.set_filter(filtro);
        if let Some(p) = &img.paleta {
            enc.set_palette(p.as_slice());
        }
        if let Some(t) = &img.trns {
            enc.set_trns(t.as_slice());
        }
        let mut w = enc.write_header().unwrap();
        w.write_image_data(&img.datos).unwrap();
        w.finish().unwrap();
    }
    salida
}

// ---------------------------------------------------------------------------------------
// Fila 7: un solo color y patrones repetidos en gris 8, indexado 8 y RGBA 8.
// ---------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Forma {
    Gris,
    Indexado,
    Rgba,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Contenido {
    UnColor,
    Patrones,
}

/// Lado de la imagen: 1024² en RGBA y 2048² con un byte por píxel (4 MiB de datos sin
/// comprimir). Con menos, la razón de los niveles 6 a 9 de miniz_oxide se queda por debajo de 500
/// (medido: 1 MiB da ≈ 350; 4 MiB da 500 a 660).
fn lado(forma: Forma) -> u32 {
    match forma {
        Forma::Rgba => 1024,
        Forma::Gris | Forma::Indexado => 2048,
    }
}

/// Paleta de 16 entradas, algunas con alfa no trivial, para el indexado.
fn paleta16() -> Vec<Rgba> {
    (0..16u8)
        .map(|i| Rgba::new(i * 16, 255 - i * 9, i * 5 + 3, if i % 4 == 0 { 100 + i } else { 255 }))
        .collect()
}

/// Valor del píxel (x, y) como 4 bytes RGBA con alfa no trivial, o su índice / gris.
fn patron(contenido: Contenido, x: usize, y: usize) -> usize {
    match contenido {
        Contenido::UnColor => 5,
        // Franjas verticales de 16 y horizontales de 64 combinadas con bloques de 8×8 que se
        // repiten cada 128 píxeles.
        Contenido::Patrones => (x / 16 + (y / 64) * 3 + usize::from((x % 128 < 8) && (y % 128 < 8)) * 7) % 16,
    }
}

/// Imagen cruda y la `Image` que `read_png` debe devolver, para esa forma y ese contenido.
fn construye(forma: Forma, contenido: Contenido) -> (Cruda, Image) {
    let n = lado(forma);
    let (w, h) = (n as usize, n as usize);
    let colores = paleta16();
    match forma {
        Forma::Gris => {
            let datos: Vec<u8> = (0..w * h)
                .map(|i| (patron(contenido, i % w, i / w) * 16 + 9) as u8)
                .collect();
            let esperada = Image::from_bytes(n, n, Mode::Gray, datos.clone()).unwrap();
            let cruda = Cruda {
                ancho: n,
                alto: n,
                color: png::ColorType::Grayscale,
                prof: png::BitDepth::Eight,
                datos,
                paleta: None,
                trns: None,
            };
            (cruda, esperada)
        }
        Forma::Indexado => {
            let datos: Vec<u8> = (0..w * h)
                .map(|i| patron(contenido, i % w, i / w) as u8)
                .collect();
            let paleta = Palette::from_colors(colores.iter().copied()).unwrap();
            let esperada = Image::from_bytes_indexed(n, n, paleta, datos.clone()).unwrap();
            let rgb: Vec<u8> = colores.iter().flat_map(|c| [c.r, c.g, c.b]).collect();
            // tRNS: los alfas de las entradas hasta la última no opaca.
            let ultimo = colores.iter().rposition(|c| c.a != 255).unwrap();
            let trns: Vec<u8> = colores[..=ultimo].iter().map(|c| c.a).collect();
            let cruda = Cruda {
                ancho: n,
                alto: n,
                color: png::ColorType::Indexed,
                prof: png::BitDepth::Eight,
                datos,
                paleta: Some(rgb),
                trns: Some(trns),
            };
            (cruda, esperada)
        }
        Forma::Rgba => {
            let mut datos = Vec::with_capacity(w * h * 4);
            for i in 0..w * h {
                let c = colores[patron(contenido, i % w, i / w)];
                // Alfa no trivial: la de la paleta si no es opaca y, si lo es, 200.
                let a = if c.a == 255 { 200 } else { c.a };
                datos.extend([c.r, c.g, c.b, a]);
            }
            let esperada = Image::from_bytes(n, n, Mode::Rgba, datos.clone()).unwrap();
            let cruda = Cruda {
                ancho: n,
                alto: n,
                color: png::ColorType::Rgba,
                prof: png::BitDepth::Eight,
                datos,
                paleta: None,
                trns: None,
            };
            (cruda, esperada)
        }
    }
}

/// Una combinación a probar: (forma, contenido, compresor, filtro).
type Combinacion = (Forma, Contenido, Compresor, png::Filter);

/// Ejecuta las combinaciones repartidas en hilos (el perfil de test no optimiza) y hace pánico
/// con la combinación que falle.
fn en_paralelo(trabajos: &[Combinacion]) {
    let siguiente = AtomicUsize::new(0);
    let hilos = thread::available_parallelism().map_or(4, |n| n.get()).min(8);
    thread::scope(|s| {
        let manejadores: Vec<_> = (0..hilos)
            .map(|_| {
                s.spawn(|| {
                    // Cada hilo conserva la última imagen construida para no regenerarla.
                    let mut cache: Option<((Forma, Contenido), Cruda, Image)> = None;
                    loop {
                        let i = siguiente.fetch_add(1, Ordering::SeqCst);
                        let Some(&(forma, contenido, comp, filtro)) = trabajos.get(i) else {
                            break;
                        };
                        if cache.as_ref().map(|c| c.0) != Some((forma, contenido)) {
                            let (c, e) = construye(forma, contenido);
                            cache = Some(((forma, contenido), c, e));
                        }
                        let (_, cruda, esperada) = cache.as_ref().unwrap();
                        let nombre = format!("{forma:?} {contenido:?} {comp:?} {filtro:?}");
                        if comp == Compresor::Z {
                            // Z(needed): la imagen es toda cero (con PLTE en indexado).
                            let (color, bpp) = match forma {
                                Forma::Gris => (0u8, 8u64),
                                Forma::Indexado => (3, 8),
                                Forma::Rgba => (6, 32),
                            };
                            let n = cruda.ancho;
                            let needed = super::max_ratio::necesarios(n, n, bpp, false);
                            let flujo = z(needed);
                            let antes = if color == 3 {
                                vec![plte_negra(256)]
                            } else {
                                vec![]
                            };
                            let bytes = fichero(n, n, 8, color, false, &antes, Some(&flujo));
                            let r = read_png(&bytes)
                                .unwrap_or_else(|e| panic!("{nombre}: rechazado: {e:?}"));
                            assert!(
                                r.image.bytes().iter().all(|&b| b == 0),
                                "{nombre}: la imagen no es toda cero"
                            );
                            continue;
                        }
                        let bytes = codifica(cruda, comp, filtro);
                        if contenido == Contenido::UnColor
                            && matches!(comp, Compresor::Nivel(6 | 9))
                            && filtro == png::Filter::Up
                        {
                            // El caso extremo de verdad: razón de expansión por encima de 500.
                            assert!(
                                bytes.len() * 500 < cruda.datos.len(),
                                "{nombre}: {} bytes para {}",
                                bytes.len(),
                                cruda.datos.len()
                            );
                        }
                        let r = read_png(&bytes)
                            .unwrap_or_else(|e| panic!("{nombre}: rechazado: {e:?}"));
                        assert!(r.image == *esperada, "{nombre}: la imagen leída es distinta");
                    }
                })
            })
            .collect();
        for m in manejadores {
            if let Err(e) = m.join() {
                std::panic::resume_unwind(e);
            }
        }
    });
}

/// Rejilla de la fila 7: `niveles` de miniz_oxide, fdeflate y sin comprimir con los cuatro filtros,
/// y `Z(needed)` una vez por forma (no usa filtro ni contenido).
fn rejilla(niveles: &[u8]) -> Vec<Combinacion> {
    let mut trabajos: Vec<Combinacion> = Vec::new();
    for forma in [Forma::Gris, Forma::Indexado, Forma::Rgba] {
        for contenido in [Contenido::UnColor, Contenido::Patrones] {
            let comps = niveles
                .iter()
                .map(|&n| Compresor::Nivel(n))
                .chain([Compresor::Fdeflate, Compresor::Sin]);
            for comp in comps {
                for filtro in filtros() {
                    trabajos.push((forma, contenido, comp, filtro));
                }
            }
        }
        trabajos.push((forma, Contenido::UnColor, Compresor::Z, png::Filter::NoFilter));
    }
    trabajos
}

/// Suite: niveles 1, 6 y 9 (el 1 es el más flojo, el 6 el de por defecto y el 9 el más fuerte).
#[test]
fn fila7_ningun_compresor_ni_filtro_es_rechazado() {
    en_paralelo(&rejilla(&[1, 6, 9]));
}

/// Rejilla completa de la spec (niveles 1 a 9 × cuatro filtros, tamaños de la spec): se ejecuta al
/// cerrar con `cargo test --release -p px-formats -- --ignored`.
#[test]
#[ignore = "rejilla completa: lenta sin optimizar; se ejecuta al cerrar con --release -- --ignored"]
fn fila7_rejilla_completa_niveles_1_a_9() {
    en_paralelo(&rejilla(&[1, 2, 3, 4, 5, 6, 7, 8, 9]));
}

// ---------------------------------------------------------------------------------------
// Propiedad 3: ningún compresor es rechazado (cualquier tipo y profundidad).
// ---------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug)]
enum Relleno {
    UnColor,
    Franjas,
    Ruido,
}

/// (tipo de color, profundidad, canales) que admite el codificador.
fn tipos() -> Vec<(png::ColorType, png::BitDepth, usize)> {
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

/// Muestra (x, y, canal) del relleno, de `bits` bits, derivada de `semilla`.
fn muestra(relleno: Relleno, semilla: u64, bits: u32, x: usize, y: usize, c: usize) -> u16 {
    let mascara = ((1u32 << bits) - 1) as u16;
    let mezcla = |a: u64| {
        let mut s = semilla ^ a.wrapping_mul(0x9E37_79B9_7F4A_7C15);
        s ^= s >> 30;
        s = s.wrapping_mul(0xBF58_476D_1CE4_E5B9);
        s ^= s >> 27;
        s = s.wrapping_mul(0x94D0_49BB_1331_11EB);
        s ^ (s >> 31)
    };
    let v = match relleno {
        Relleno::UnColor => mezcla(c as u64),
        Relleno::Franjas => mezcla((x / 5 + (y / 3) * 7 + c * 1000) as u64 % 11),
        Relleno::Ruido => mezcla(((y * 64 + x) * 4 + c) as u64 + 77),
    };
    (v as u16) & mascara
}

fn cruda_aleatoria(
    ancho: u32,
    alto: u32,
    tipo: (png::ColorType, png::BitDepth, usize),
    relleno: Relleno,
    semilla: u64,
) -> Cruda {
    let (color, prof, canales) = tipo;
    let bits = prof as u32;
    let n_paleta = 1usize << bits;
    let indexada = color == png::ColorType::Indexed;
    let mut datos = Vec::new();
    for y in 0..alto as usize {
        let fila: Vec<u16> = (0..ancho as usize)
            .flat_map(|x| (0..canales).map(move |c| (x, c)))
            .map(|(x, c)| muestra(relleno, semilla, bits, x, y, c))
            .collect();
        datos.extend(empaqueta(&fila, bits as u8));
    }
    let (paleta, trns) = if indexada {
        let rgb: Vec<u8> = (0..n_paleta)
            .flat_map(|i| [i as u8, (i * 7) as u8, (i * 13) as u8])
            .collect();
        (Some(rgb), Some(vec![0u8, 128]))
    } else {
        (None, None)
    };
    Cruda {
        ancho,
        alto,
        color,
        prof,
        datos,
        paleta,
        trns,
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    // Propiedad 3: `read_png` es `Ok` para cualquier imagen que el codificador admita y cualquier
    // forma de comprimir de la fila 7, con y sin `Z(needed)` a mano.
    #[test]
    fn p3_ningun_compresor_es_rechazado(
        ancho in 1u32..=64,
        alto in 1u32..=64,
        tipo in select(tipos()),
        relleno in select(vec![Relleno::UnColor, Relleno::Franjas, Relleno::Ruido]),
        semilla in any::<u64>(),
        compresor in select(COMPRESORES_PNG.to_vec()),
        filtro in select(filtros().to_vec()),
    ) {
        let img = cruda_aleatoria(ancho, alto, tipo, relleno, semilla);
        let bytes = codifica(&img, compresor, filtro);
        let r = read_png(&bytes);
        prop_assert!(
            r.is_ok(),
            "{}x{} {:?} {:?} {:?} {:?}: {:?}",
            ancho, alto, tipo, compresor, filtro, relleno, r.as_ref().err()
        );
        let r = r.unwrap();
        prop_assert_eq!((r.image.width(), r.image.height()), (ancho, alto));
    }

    // Propiedad 3 con `Z(needed)` a mano en cualquier tipo y profundidad.
    #[test]
    fn p3_z_a_mano_nunca_es_rechazado(
        ancho in 1u32..=64,
        alto in 1u32..=64,
        tipo in select(tipos()),
    ) {
        let (color, prof, canales) = tipo;
        let color_png = match color {
            png::ColorType::Grayscale => 0u8,
            png::ColorType::Rgb => 2,
            png::ColorType::Indexed => 3,
            png::ColorType::GrayscaleAlpha => 4,
            png::ColorType::Rgba => 6,
        };
        let bits = prof as u8;
        let bpp = (canales as u64) * u64::from(bits);
        let needed = super::max_ratio::necesarios(ancho, alto, bpp, false);
        let antes = if color_png == 3 { vec![plte_negra(1 << bits)] } else { vec![] };
        let bytes = fichero(ancho, alto, bits, color_png, false, &antes, Some(&z(needed)));
        let r = read_png(&bytes);
        prop_assert!(r.is_ok(), "{:?}", r.as_ref().err());
    }
}
