//! Corpus de ida y vuelta de PNG en `px-golden` (specs/hito-1-png-corpus.md, puntos 1 a 3).
//!
//! Tres bloques, todos con la API pública de `px-formats` y sin ficheros en disco:
//!
//! 1. Ida y vuelta sintética (filas 1 a 6): `read_png(&write_png(&img)?)` debe dar informe
//!    vacío e imagen igual a `normalizada(img)`. Un informe no vacío aquí es un fallo.
//! 2. Conversiones declaradas (filas 7 a 9): ficheros construidos byte a byte con `common`;
//!    cada caso comprueba a la vez la imagen y la lista exacta del informe.
//! 3. Errores (filas 10 a 12): la variante exacta de `PngError`, nunca solo `is_err()`.
//!
//! Los valores esperados salen de la especificación del PNG y de la spec de la unidad 5, no
//! de ningún programa de referencia. Ningún test reserva más de unos pocos KiB: los casos de
//! tamaño se prueban por el lado del rechazo, con cabecera enorme y datos mínimos.

mod common;

use common::{FIRMA, Png, adler32, crc32, plte, posicion_idat, recorre, trns, trozo};
use proptest::collection::vec;
use proptest::prelude::*;
use proptest::sample::select;
use px_core::{Image, Mode, Palette, Pixel, Rgba};
use px_formats::{Conversion, PngError, PngRead, read_png, write_png};

// ---------------------------------------------------------------------------------------
// Ayudantes de imagen.
// ---------------------------------------------------------------------------------------

/// Todos los píxeles, fila a fila.
fn pixeles(img: &Image) -> Vec<Pixel> {
    let mut v = Vec::new();
    for y in 0..img.height() {
        for x in 0..img.width() {
            v.push(img.get_pixel(x, y).unwrap());
        }
    }
    v
}

/// Rellena una imagen con los píxeles dados, fila a fila.
fn rellena(mut img: Image, px: impl IntoIterator<Item = Pixel>) -> Image {
    let w = img.width();
    for (i, p) in px.into_iter().enumerate() {
        img.set_pixel(i as u32 % w, i as u32 / w, p).unwrap();
    }
    img
}

fn grises(v: &[u8]) -> Vec<Pixel> {
    v.iter().map(|&g| Pixel::Gray(g)).collect()
}

fn grises_alfa(v: &[(u8, u8)]) -> Vec<Pixel> {
    v.iter()
        .map(|&(gray, alpha)| Pixel::GrayAlpha { gray, alpha })
        .collect()
}

fn indices(v: &[u8]) -> Vec<Pixel> {
    v.iter().map(|&i| Pixel::Indexed(i)).collect()
}

fn rgba(r: u8, g: u8, b: u8, a: u8) -> Pixel {
    Pixel::Rgba(Rgba::new(r, g, b, a))
}

/// Imagen indexada de ancho `w` con esa paleta, índice transparente y píxeles (por filas).
fn indexada(w: u32, colores: &[Rgba], transparente: Option<u8>, px: &[u8]) -> Image {
    let mut p = Palette::from_colors(colores.iter().copied()).unwrap();
    p.set_transparent_index(transparente).unwrap();
    let h = px.len() as u32 / w;
    rellena(
        Image::new_indexed(w, h, p).unwrap(),
        px.iter().map(|&i| Pixel::Indexed(i)),
    )
}

/// `img`, salvo en un indexado con índice transparente, cuya paleta sale ya resuelta
/// (entrada `t` con `a = 0`) y sin índice transparente (spec de PNG, comportamiento 11).
fn normalizada(img: &Image) -> Image {
    let Some(p) = img.palette() else {
        return img.clone();
    };
    if p.transparent_index().is_none() {
        return img.clone();
    }
    let colores: Vec<Rgba> = (0..p.len()).map(|i| p.resolved(i as u8).unwrap()).collect();
    let copia = Image::new_indexed(
        img.width(),
        img.height(),
        Palette::from_colors(colores).unwrap(),
    )
    .unwrap();
    rellena(copia, pixeles(img))
}

/// «Exacto»: `write_png` y `read_png` dan `Ok`, informe vacío e imagen igual a
/// `normalizada(img)`. Devuelve la imagen leída para aserciones adicionales.
fn comprueba_exacto(img: &Image, etiqueta: &str) -> Image {
    let bytes = write_png(img).unwrap_or_else(|e| panic!("{etiqueta}: write_png falló: {e}"));
    let r = read_png(&bytes).unwrap_or_else(|e| panic!("{etiqueta}: read_png falló: {e}"));
    assert!(r.is_exact(), "{etiqueta}: informe no vacío: {:?}", r.report);
    assert!(r.report.is_empty(), "{etiqueta}");
    assert_eq!(r.image, normalizada(img), "{etiqueta}");
    r.image
}

fn lee(bytes: &[u8]) -> PngRead {
    read_png(bytes).unwrap()
}

fn error_de(bytes: &[u8]) -> PngError {
    read_png(bytes).unwrap_err()
}

fn es_decode(r: &Result<PngRead, PngError>) -> bool {
    matches!(r, Err(PngError::Decode(_)))
}

// ---------------------------------------------------------------------------------------
// Comprobación del propio ayudante (el corpus depende de él).
// ---------------------------------------------------------------------------------------

#[test]
fn ayudante_crc32_adler32_y_recorredor_con_valores_conocidos() {
    // CRC del chunk IEND vacío, conocido: 0xAE426082.
    assert_eq!(crc32(b"IEND"), 0xAE42_6082);
    assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    assert_eq!(adler32(b"Wikipedia"), 0x11E6_0398);
    assert_eq!(adler32(b""), 1);
    assert_eq!(
        trozo(b"IEND", &[]),
        [0, 0, 0, 0, b'I', b'E', b'N', b'D', 0xAE, 0x42, 0x60, 0x82]
    );

    let bueno = Png::new(2, 1, 8, 0).muestras(&[1, 2]).bytes();
    assert_eq!(bueno[..8], FIRMA);
    let tipos: Vec<[u8; 4]> = recorre(&bueno).unwrap().iter().map(|t| t.tipo).collect();
    assert_eq!(tipos, [*b"IHDR", *b"IDAT", *b"IEND"]);
    let mut con_basura = bueno.clone();
    con_basura.push(0);
    assert!(recorre(&con_basura).is_err());
    assert!(recorre(&bueno[..bueno.len() - 1]).is_err());
}

// ---------------------------------------------------------------------------------------
// Punto 1: ida y vuelta sintética (filas 1 a 6).
// ---------------------------------------------------------------------------------------

/// Píxel determinista y distinto de sus vecinos para los modos no indexados.
fn pixel_de_patron(modo: Mode, k: usize) -> Pixel {
    let b = |m: usize, c: usize| (k * m + c) as u8;
    match modo {
        Mode::Rgba => rgba(b(7, 1), b(13, 2), b(29, 3), b(31, 5)),
        Mode::Gray => Pixel::Gray(b(37, 11)),
        Mode::GrayAlpha => Pixel::GrayAlpha {
            gray: b(37, 11),
            alpha: b(53, 7),
        },
        Mode::Indexed => unreachable!("el indexado necesita paleta"),
    }
}

/// Paleta determinista de `n` entradas, con alfas distintos.
fn colores_de_patron(n: usize) -> Vec<Rgba> {
    (0..n)
        .map(|i| {
            let b = |m: usize, c: usize| (i * m + c) as u8;
            Rgba::new(b(7, 1), b(13, 2), b(29, 3), b(31, 5))
        })
        .collect()
}

#[test]
fn fila_01_los_cuatro_modos_en_cuatro_formas() {
    for (w, h) in [(1u32, 1u32), (3, 3), (1, 300), (300, 1)] {
        let n = (w * h) as usize;
        for modo in [Mode::Rgba, Mode::Gray, Mode::GrayAlpha] {
            let img = rellena(
                Image::new(w, h, modo).unwrap(),
                (0..n).map(|k| pixel_de_patron(modo, k)),
            );
            comprueba_exacto(&img, &format!("{modo:?} {w}x{h}"));
        }

        // Indexado con una paleta de hasta 200 entradas (todas distintas si caben) y el
        // píxel k apuntando a la entrada k mod len.
        let len = n.min(200);
        let img = indexada(
            w,
            &colores_de_patron(len),
            None,
            &(0..n).map(|k| (k % len) as u8).collect::<Vec<u8>>(),
        );
        comprueba_exacto(&img, &format!("Indexed {w}x{h}"));

        // Indexado con la paleta mínima: una sola entrada.
        let img = indexada(w, &[Rgba::new(11, 22, 33, 44)], None, &vec![0u8; n]);
        let leida = comprueba_exacto(&img, &format!("Indexed de 1 entrada {w}x{h}"));
        assert_eq!(leida.palette().unwrap().len(), 1);
    }
}

#[test]
fn fila_02_indexado_con_paleta_de_256_entradas() {
    let colores: Vec<Rgba> = (0..=255u8)
        .map(|i| Rgba::new(i, 255 - i, i ^ 0x5A, i.wrapping_mul(3)))
        .collect();
    let px: Vec<u8> = (0..=255u8).collect();
    let img = indexada(256, &colores, None, &px);
    let leida = comprueba_exacto(&img, "paleta de 256");
    assert_eq!(leida.palette().unwrap().len(), 256);
    assert_eq!(pixeles(&leida), indices(&px));
}

#[test]
fn fila_03_colores_duplicados_no_se_fusionan() {
    let colores = [
        Rgba::new(9, 9, 9, 255),
        Rgba::new(9, 9, 9, 255),
        Rgba::new(9, 9, 9, 128),
        Rgba::new(9, 9, 9, 255),
    ];
    let img = indexada(4, &colores, None, &[0, 1, 2, 3]);
    let leida = comprueba_exacto(&img, "duplicados");
    let paleta = leida.palette().unwrap();
    assert_eq!(paleta.len(), 4);
    assert_eq!(paleta.colors(), colores);
    assert_eq!(pixeles(&leida), indices(&[0, 1, 2, 3]));
}

#[test]
fn fila_04_alfas_0_1_254_y_255_sin_premultiplicar() {
    let alfas = [0u8, 1, 254, 255];

    // GrayAlpha: gris distinto de 0 también con alfa 0.
    let ga = rellena(
        Image::new(4, 1, Mode::GrayAlpha).unwrap(),
        alfas
            .iter()
            .enumerate()
            .map(|(i, &alpha)| Pixel::GrayAlpha {
                gray: 77 + i as u8,
                alpha,
            }),
    );
    let leida = comprueba_exacto(&ga, "GrayAlpha");
    assert_eq!(
        pixeles(&leida),
        grises_alfa(&[(77, 0), (78, 1), (79, 254), (80, 255)])
    );

    // Rgba: RGB distinto de 0 también con alfa 0.
    let c = rellena(
        Image::new(4, 1, Mode::Rgba).unwrap(),
        alfas
            .iter()
            .enumerate()
            .map(|(i, &a)| rgba(10 + i as u8, 20 + i as u8, 30 + i as u8, a)),
    );
    let leida = comprueba_exacto(&c, "Rgba");
    assert_eq!(
        pixeles(&leida),
        [
            rgba(10, 20, 30, 0),
            rgba(11, 21, 31, 1),
            rgba(12, 22, 32, 254),
            rgba(13, 23, 33, 255)
        ]
    );

    // Paleta con esos alfas (sin índice transparente): la entrada 0 conserva su RGB.
    let colores: Vec<Rgba> = alfas
        .iter()
        .enumerate()
        .map(|(i, &a)| Rgba::new(40 + i as u8, 50 + i as u8, 60 + i as u8, a))
        .collect();
    let img = indexada(4, &colores, None, &[0, 1, 2, 3]);
    let leida = comprueba_exacto(&img, "paleta con alfas");
    let paleta = leida.palette().unwrap();
    assert_eq!(paleta.get(0).unwrap(), Rgba::new(40, 50, 60, 0));
    assert_eq!(paleta.get(1).unwrap(), Rgba::new(41, 51, 61, 1));
    assert_eq!(paleta.get(2).unwrap(), Rgba::new(42, 52, 62, 254));
    assert_eq!(paleta.get(3).unwrap(), Rgba::new(43, 53, 63, 255));
    assert_eq!(paleta.transparent_index(), None);
}

#[test]
fn fila_05_indice_transparente_sale_resuelto_y_sin_indice() {
    let c = Rgba::new;
    // (colores, índice transparente, píxeles)
    let casos: [(Vec<Rgba>, u8, Vec<u8>); 4] = [
        (
            vec![c(10, 20, 30, 255), c(40, 50, 60, 128), c(70, 80, 90, 255)],
            0,
            vec![0, 1, 2],
        ),
        // Última de 3.
        (
            vec![c(10, 20, 30, 255), c(40, 50, 60, 128), c(70, 80, 90, 255)],
            2,
            vec![2, 1, 0],
        ),
        // Paleta de 1 entrada.
        (vec![c(5, 6, 7, 255)], 0, vec![0, 0]),
        // Una entrada con alfa 200 marcada transparente: el alfa pasa a 0.
        (
            vec![c(1, 2, 3, 255), c(4, 5, 6, 200), c(7, 8, 9, 255)],
            1,
            vec![0, 1, 2],
        ),
    ];
    for (colores, t, px) in casos {
        let img = indexada(px.len() as u32, &colores, Some(t), &px);
        assert_eq!(img.palette().unwrap().transparent_index(), Some(t));
        let leida = comprueba_exacto(&img, &format!("transparente {t}"));
        let paleta = leida.palette().unwrap();
        assert_eq!(paleta.transparent_index(), None, "transparente {t}");
        assert_eq!(paleta.len(), colores.len(), "transparente {t}");
        for (i, original) in colores.iter().enumerate() {
            let esperado = if i == usize::from(t) {
                Rgba { a: 0, ..*original }
            } else {
                *original
            };
            assert_eq!(
                paleta.colors()[i],
                esperado,
                "transparente {t}, entrada {i}"
            );
        }
        assert_eq!(pixeles(&leida), indices(&px), "transparente {t}");
    }
}

// Propiedad de la fila 6: imágenes aleatorias en los cuatro modos, con las formas alargadas
// y las paletas de hasta 256 entradas con duplicados que la propiedad 1 de `px-formats`
// (lados de 1 a 8) no cubre.

/// Alfa: con frecuencia uno de los valores de borde, si no cualquiera.
fn alfa() -> impl Strategy<Value = u8> {
    prop_oneof![select(vec![0u8, 1, 254, 255]), any::<u8>()]
}

/// RGB cualquiera (también con alfa 0).
fn color_aleatorio() -> impl Strategy<Value = Rgba> {
    (any::<u8>(), any::<u8>(), any::<u8>(), alfa()).prop_map(|(r, g, b, a)| Rgba::new(r, g, b, a))
}

/// Color de un grupo de 3, para que las paletas tengan entradas duplicadas.
fn color_de_grupo() -> impl Strategy<Value = Rgba> {
    select(vec![
        Rgba::new(9, 9, 9, 255),
        Rgba::new(9, 9, 9, 128),
        Rgba::new(200, 100, 50, 0),
    ])
}

/// 1 × 1, 1 × N, N × 1 (N de 2 a 300) y A × B (de 1 a 48 de lado).
fn forma() -> impl Strategy<Value = (u32, u32)> {
    prop_oneof![
        Just((1u32, 1u32)),
        (2u32..=300).prop_map(|n| (1, n)),
        (2u32..=300).prop_map(|n| (n, 1)),
        (1u32..=48, 1u32..=48),
    ]
}

/// Imagen de cualquiera de los cuatro modos con una forma de `forma()`.
fn imagen_arbitraria() -> BoxedStrategy<Image> {
    forma()
        .prop_flat_map(|(w, h)| {
            let n = (w * h) as usize;
            prop_oneof![
                vec(color_aleatorio(), n).prop_map(move |px| rellena(
                    Image::new(w, h, Mode::Rgba).unwrap(),
                    px.into_iter().map(Pixel::Rgba)
                )),
                vec(any::<u8>(), n).prop_map(move |px| rellena(
                    Image::new(w, h, Mode::Gray).unwrap(),
                    px.into_iter().map(Pixel::Gray)
                )),
                vec((any::<u8>(), alfa()), n).prop_map(move |px| rellena(
                    Image::new(w, h, Mode::GrayAlpha).unwrap(),
                    px.into_iter()
                        .map(|(gray, alpha)| Pixel::GrayAlpha { gray, alpha })
                )),
                (1usize..=256)
                    .prop_flat_map(move |len| {
                        (
                            prop_oneof![vec(color_de_grupo(), len), vec(color_aleatorio(), len)],
                            proptest::option::of(0..len),
                            vec(0..len, n),
                        )
                    })
                    .prop_map(move |(colores, transparente, px)| {
                        let mut p = Palette::from_colors(colores).unwrap();
                        p.set_transparent_index(transparente.map(|t| t as u8))
                            .unwrap();
                        rellena(
                            Image::new_indexed(w, h, p).unwrap(),
                            px.into_iter().map(|i| Pixel::Indexed(i as u8)),
                        )
                    }),
            ]
        })
        .boxed()
}

proptest! {
    // Fila 6: la misma aserción que la fila 1 (exacto), 256 casos por defecto.
    #[test]
    fn fila_06_propiedad_ida_y_vuelta_exacta(img in imagen_arbitraria()) {
        let bytes = write_png(&img).unwrap();
        let r = read_png(&bytes).unwrap();
        prop_assert!(r.report.is_empty());
        prop_assert!(r.is_exact());
        prop_assert_eq!(r.image, normalizada(&img));
    }
}

// ---------------------------------------------------------------------------------------
// Punto 2: conversiones declaradas (filas 7 a 9).
// ---------------------------------------------------------------------------------------

#[test]
fn fila_07_gris_de_1_2_y_4_bits_se_escala() {
    // 1 bit: 0 y 255.
    let r1 = lee(&Png::new(2, 1, 1, 0).muestras(&[0, 1]).bytes());
    assert_eq!(r1.image.mode(), Mode::Gray);
    assert_eq!(pixeles(&r1.image), grises(&[0, 255]));
    assert_eq!(r1.report, [Conversion::DepthExpanded { from_bits: 1 }]);
    assert!(!r1.is_exact());

    // 2 bits: 0, 85, 170, 255.
    let r2 = lee(&Png::new(4, 1, 2, 0).muestras(&[0, 1, 2, 3]).bytes());
    assert_eq!(r2.image.mode(), Mode::Gray);
    assert_eq!(pixeles(&r2.image), grises(&[0, 85, 170, 255]));
    assert_eq!(r2.report, [Conversion::DepthExpanded { from_bits: 2 }]);
    assert!(!r2.is_exact());

    // 4 bits, todos los valores: v * 17 (0, 17, 34, ..., 255).
    let todos: Vec<u16> = (0..16).collect();
    let r4 = lee(&Png::new(16, 1, 4, 0).muestras(&todos).bytes());
    let esperados: Vec<u8> = (0..16u8).map(|v| v * 17).collect();
    assert_eq!(esperados[15], 255);
    assert_eq!(r4.image.mode(), Mode::Gray);
    assert_eq!(pixeles(&r4.image), grises(&esperados));
    assert_eq!(r4.report, [Conversion::DepthExpanded { from_bits: 4 }]);
    assert!(!r4.is_exact());
}

#[test]
fn fila_08_dieciseis_bits_se_reduce_con_redondeo() {
    // (muestra de 16 bits, valor de 8 bits esperado). Redondeo, no el byte alto:
    // 0x01FF da 2 y no 1.
    let tabla: [(u16, u8); 5] = [
        (0x0000, 0),
        (0x0080, 0),
        (0x0081, 1),
        (0x01FF, 2),
        (0xFFFF, 255),
    ];
    let m = |i: usize| tabla[i % 5].0;
    let v = |i: usize| tabla[i % 5].1;

    // Gris 16.
    let muestras_g: Vec<u16> = (0..5).map(m).collect();
    let g = lee(&Png::new(5, 1, 16, 0).muestras(&muestras_g).bytes());
    assert_eq!(g.image.mode(), Mode::Gray);
    assert_eq!(pixeles(&g.image), grises(&[0, 0, 1, 2, 255]));
    assert_eq!(g.report, [Conversion::Depth16Reduced]);
    assert!(!g.is_exact());

    // Gris con alfa 16: gris = tabla[i], alfa = tabla[4 - i].
    let muestras_ga: Vec<u16> = (0..5).flat_map(|i| [m(i), m(4 - i)]).collect();
    let ga = lee(&Png::new(5, 1, 16, 4).muestras(&muestras_ga).bytes());
    assert_eq!(ga.image.mode(), Mode::GrayAlpha);
    let esperados_ga: Vec<(u8, u8)> = (0..5).map(|i| (v(i), v(4 - i))).collect();
    assert_eq!(esperados_ga, [(0, 255), (0, 2), (1, 1), (2, 0), (255, 0)]);
    assert_eq!(pixeles(&ga.image), grises_alfa(&esperados_ga));
    assert_eq!(ga.report, [Conversion::Depth16Reduced]);
    assert!(!ga.is_exact());

    // RGBA 16: el píxel i usa tabla[i], tabla[i + 1], tabla[i + 2] y tabla[i + 3].
    let muestras_c: Vec<u16> = (0..5)
        .flat_map(|i| [m(i), m(i + 1), m(i + 2), m(i + 3)])
        .collect();
    let c = lee(&Png::new(5, 1, 16, 6).muestras(&muestras_c).bytes());
    assert_eq!(c.image.mode(), Mode::Rgba);
    let esperados_c = [
        rgba(0, 0, 1, 2),
        rgba(0, 1, 2, 255),
        rgba(1, 2, 255, 0),
        rgba(2, 255, 0, 0),
        rgba(255, 0, 0, 1),
    ];
    assert_eq!(pixeles(&c.image), esperados_c);
    assert_eq!(c.report, [Conversion::Depth16Reduced]);
    assert!(!c.is_exact());

    // RGB 16 (0x0102, 0x0304, 0xFFFF): Rgba(1, 3, 255, 255).
    let rgb = lee(&Png::new(1, 1, 16, 2)
        .muestras(&[0x0102, 0x0304, 0xFFFF])
        .bytes());
    assert_eq!(rgb.image.mode(), Mode::Rgba);
    assert_eq!(pixeles(&rgb.image), [rgba(1, 3, 255, 255)]);
    assert_eq!(
        rgb.report,
        [Conversion::Depth16Reduced, Conversion::RgbToRgba]
    );
    assert!(!rgb.is_exact());
}

#[test]
fn fila_09_rgb_y_colores_clave_trns() {
    // RGB 8 sin tRNS.
    let r = lee(&Png::new(1, 1, 8, 2).muestras(&[1, 2, 3]).bytes());
    assert_eq!(r.image.mode(), Mode::Rgba);
    assert_eq!(pixeles(&r.image), [rgba(1, 2, 3, 255)]);
    assert_eq!(r.report, [Conversion::RgbToRgba]);
    assert!(!r.is_exact());

    // tRNS de gris 8 con clave 7 (dos bytes, big-endian) y píxeles [7, 8].
    let g8 = lee(&Png::new(2, 1, 8, 0)
        .muestras(&[7, 8])
        .antes(trns(&[0, 7]))
        .bytes());
    assert_eq!(g8.image.mode(), Mode::GrayAlpha);
    assert_eq!(pixeles(&g8.image), grises_alfa(&[(7, 0), (8, 255)]));
    assert_eq!(g8.report, [Conversion::ColorKeyToAlpha]);
    assert!(!g8.is_exact());

    // Gris de 2 bits con clave 1 y píxeles [1, 2]: 1 -> 85 transparente, 2 -> 170 opaco.
    let g2 = lee(&Png::new(2, 1, 2, 0)
        .muestras(&[1, 2])
        .antes(trns(&[0, 1]))
        .bytes());
    assert_eq!(g2.image.mode(), Mode::GrayAlpha);
    assert_eq!(pixeles(&g2.image), grises_alfa(&[(85, 0), (170, 255)]));
    assert_eq!(
        g2.report,
        [
            Conversion::DepthExpanded { from_bits: 2 },
            Conversion::ColorKeyToAlpha
        ]
    );
    assert!(!g2.is_exact());

    // RGB 8 con clave (1, 2, 3) y píxeles (1, 2, 3) y (1, 2, 4): el RGB de la clave se
    // conserva con alfa 0.
    let c8 = lee(&Png::new(2, 1, 8, 2)
        .muestras(&[1, 2, 3, 1, 2, 4])
        .antes(trns(&[0, 1, 0, 2, 0, 3]))
        .bytes());
    assert_eq!(c8.image.mode(), Mode::Rgba);
    assert_eq!(pixeles(&c8.image), [rgba(1, 2, 3, 0), rgba(1, 2, 4, 255)]);
    assert_eq!(
        c8.report,
        [Conversion::RgbToRgba, Conversion::ColorKeyToAlpha]
    );
    assert!(!c8.is_exact());

    // RGB 16 con tRNS: la clave se compara con las muestras de 16 bits originales, antes
    // de reducir. 0xFFFF se reduce a 255 y 0xFF00 a 254 (65280 / 257 = 254,01).
    let c16 = lee(&Png::new(2, 1, 16, 2)
        .muestras(&[0x0102, 0x0304, 0xFFFF, 0x0102, 0x0304, 0xFF00])
        .antes(trns(&[0x01, 0x02, 0x03, 0x04, 0xFF, 0xFF]))
        .bytes());
    assert_eq!(c16.image.mode(), Mode::Rgba);
    assert_eq!(
        pixeles(&c16.image),
        [rgba(1, 3, 255, 0), rgba(1, 3, 254, 255)]
    );
    assert_eq!(
        c16.report,
        [
            Conversion::Depth16Reduced,
            Conversion::RgbToRgba,
            Conversion::ColorKeyToAlpha
        ]
    );
    assert!(!c16.is_exact());
}

// ---------------------------------------------------------------------------------------
// Punto 3: errores (filas 10 a 12). Ningún caso hace pánico: un pánico falla el test.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_10_ficheros_truncados_corruptos_o_con_basura() {
    // Indexado 4 x 4 escrito por `write_png`.
    let colores = [
        Rgba::new(1, 2, 3, 255),
        Rgba::new(4, 5, 6, 128),
        Rgba::new(7, 8, 9, 255),
    ];
    let px: Vec<u8> = (0..16).map(|k| (k % 3) as u8).collect();
    let valido = write_png(&indexada(4, &colores, None, &px)).unwrap();
    assert!(read_png(&valido).is_ok());
    assert!(recorre(&valido).is_ok());

    // Cada prefijo estricto, de 0 a len - 1 bytes.
    for n in 0..valido.len() {
        let r = read_png(&valido[..n]);
        assert!(es_decode(&r), "prefijo de {n} bytes: {r:?}");
    }

    // CRC erróneo en IHDR (bytes 29..33: 8 de firma, 4 de longitud, 4 de tipo y 13 de
    // datos), en IDAT y en IEND (los 4 últimos bytes del fichero).
    let (idat, largo_idat) = posicion_idat(&valido);
    let posiciones = [
        ("IHDR", 32),
        ("IDAT", idat + 8 + largo_idat + 3),
        ("IEND", valido.len() - 1),
    ];
    for (nombre, pos) in posiciones {
        let mut roto = valido.clone();
        roto[pos] ^= 0xFF;
        let r = read_png(&roto);
        assert!(es_decode(&r), "CRC erróneo en {nombre}: {r:?}");
    }

    // Un byte de más tras IEND.
    let mut con_x = valido.clone();
    con_x.extend(b"x");
    assert_eq!(error_de(&con_x), PngError::TrailingData);

    // Un chunk tEXt completo tras IEND.
    let mut con_texto = valido.clone();
    con_texto.extend(trozo(b"tEXt", b"k\0v"));
    assert_eq!(error_de(&con_texto), PngError::TrailingData);
}

#[test]
fn fila_11_paleta_y_trns_invalidos() {
    // Indexado de 8 bits con PLTE de 2 entradas y un píxel 5.
    let fuera = Png::new(2, 1, 8, 3)
        .muestras(&[0, 5])
        .antes(plte(&[(1, 1, 1), (2, 2, 2)]))
        .bytes();
    assert_eq!(
        error_de(&fuera),
        PngError::PaletteIndexOutOfRange { index: 5, len: 2 }
    );

    // Indexado sin PLTE.
    let sin_plte = Png::new(2, 1, 8, 3).muestras(&[0, 1]).bytes();
    assert_eq!(error_de(&sin_plte), PngError::InvalidPalette);

    // tRNS de gris de 1 byte (debe ser de 2).
    let gris = Png::new(1, 1, 8, 0)
        .muestras(&[1])
        .antes(trns(&[7]))
        .bytes();
    assert_eq!(error_de(&gris), PngError::InvalidTrns);

    // tRNS de RGB de 4 bytes (debe ser de 6).
    let rgb = Png::new(1, 1, 8, 2)
        .muestras(&[1, 2, 3])
        .antes(trns(&[0, 1, 0, 2]))
        .bytes();
    assert_eq!(error_de(&rgb), PngError::InvalidTrns);

    // tRNS de 3 entradas con PLTE de 2.
    let largo = Png::new(2, 1, 8, 3)
        .muestras(&[0, 1])
        .antes(plte(&[(1, 1, 1), (2, 2, 2)]))
        .antes(trns(&[0, 1, 2]))
        .bytes();
    assert_eq!(error_de(&largo), PngError::InvalidTrns);
}

#[test]
fn fila_12_cabecera_enorme_con_datos_minimos_es_too_large() {
    // Gris 8 de 32769 x 32768: 1 byte por encima de `Image::MAX_BYTES` (2^30). El IDAT es
    // mínimo: no se reserva nada y no hay pánico.
    let casi = Png::new(32769, 32768, 8, 0).bytes();
    assert_eq!(
        error_de(&casi),
        PngError::TooLarge {
            width: 32769,
            height: 32768
        }
    );

    // Gris 8 de 100000 x 100000.
    let enorme = Png::new(100_000, 100_000, 8, 0).bytes();
    assert_eq!(
        error_de(&enorme),
        PngError::TooLarge {
            width: 100_000,
            height: 100_000
        }
    );
}
