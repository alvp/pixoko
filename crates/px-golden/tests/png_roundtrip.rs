//! Corpus de ida y vuelta de PNG en `px-golden` (specs/hito-1-png-corpus.md, puntos 1 a 3).
//!
//! Tres bloques, todos con la API pública de `px-formats` y sin ficheros en disco:
//!
//! 1. Ida y vuelta sintética (filas 1 a 6): `read_png(&write_png(&img)?)` debe dar informe
//!    vacío e imagen igual a `normalized(img)`. Un informe no vacío aquí es un fallo.
//! 2. Conversiones declaradas (filas 7 a 9): ficheros construidos byte a byte con `common`;
//!    cada caso comprueba a la vez la imagen y la lista exacta del informe.
//! 3. Errores (filas 10 a 12): la variante exacta de `PngError`, nunca solo `is_err()`.
//!
//! Los valores esperados salen de la especificación del PNG y de la spec de la unidad 5, no
//! de ningún programa de referencia. Ningún test reserva más de unos pocos KiB: los casos de
//! tamaño se prueban por el lado del rechazo, con cabecera enorme y datos mínimos.

mod common;

use common::{Png, SIGNATURE, adler32, crc32, piece, plte, position_idat, trns, walks};
use proptest::collection::vec;
use proptest::prelude::*;
use proptest::sample::select;
use px_core::{Image, Mode, Palette, Pixel, Rgba};
use px_formats::{Conversion, PngError, PngRead, read_png, write_png};

// ---------------------------------------------------------------------------------------
// Ayudantes de imagen.
// ---------------------------------------------------------------------------------------

/// Todos los píxeles, fila a fila.
fn pixels_2(img: &Image) -> Vec<Pixel> {
    let mut v = Vec::new();
    for y in 0..img.height() {
        for x in 0..img.width() {
            v.push(img.get_pixel(x, y).unwrap());
        }
    }
    v
}

/// Rellena una imagen con los píxeles dados, fila a fila.
fn fills(mut img: Image, px: impl IntoIterator<Item = Pixel>) -> Image {
    let w = img.width();
    for (i, p) in px.into_iter().enumerate() {
        img.set_pixel(i as u32 % w, i as u32 / w, p).unwrap();
    }
    img
}

fn grays(v: &[u8]) -> Vec<Pixel> {
    v.iter().map(|&g| Pixel::Gray(g)).collect()
}

fn grays_alpha(v: &[(u8, u8)]) -> Vec<Pixel> {
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
fn indexed(w: u32, colors: &[Rgba], transparent: Option<u8>, px: &[u8]) -> Image {
    let mut p = Palette::from_colors(colors.iter().copied()).unwrap();
    p.set_transparent_index(transparent).unwrap();
    let h = px.len() as u32 / w;
    fills(
        Image::new_indexed(w, h, p).unwrap(),
        px.iter().map(|&i| Pixel::Indexed(i)),
    )
}

/// `img`, salvo en un indexado con índice transparente, cuya paleta sale ya resuelta
/// (entrada `t` con `a = 0`) y sin índice transparente (spec de PNG, comportamiento 11).
fn normalized(img: &Image) -> Image {
    let Some(p) = img.palette() else {
        return img.clone();
    };
    if p.transparent_index().is_none() {
        return img.clone();
    }
    let colors: Vec<Rgba> = (0..p.len()).map(|i| p.resolved(i as u8).unwrap()).collect();
    let copy = Image::new_indexed(
        img.width(),
        img.height(),
        Palette::from_colors(colors).unwrap(),
    )
    .unwrap();
    fills(copy, pixels_2(img))
}

/// «Exacto»: `write_png` y `read_png` dan `Ok`, informe vacío e imagen igual a
/// `normalized(img)`. Devuelve la imagen leída para aserciones adicionales.
fn checks_exact(img: &Image, label: &str) -> Image {
    let bytes = write_png(img).unwrap_or_else(|e| panic!("{label}: write_png failed: {e}"));
    let r = read_png(&bytes).unwrap_or_else(|e| panic!("{label}: read_png failed: {e}"));
    assert!(r.is_exact(), "{label}: non-empty report: {:?}", r.report);
    assert!(r.report.is_empty(), "{label}");
    assert_eq!(r.image, normalized(img), "{label}");
    r.image
}

fn read_bytes(bytes: &[u8]) -> PngRead {
    read_png(bytes).unwrap()
}

fn error_of(bytes: &[u8]) -> PngError {
    read_png(bytes).unwrap_err()
}

fn es_decode(r: &Result<PngRead, PngError>) -> bool {
    matches!(r, Err(PngError::Decode(_)))
}

// ---------------------------------------------------------------------------------------
// Comprobación del propio ayudante (el corpus depende de él).
// ---------------------------------------------------------------------------------------

#[test]
fn crc32_adler32_helper_and_walker_with_known_values() {
    // CRC del chunk IEND vacío, conocido: 0xAE426082.
    assert_eq!(crc32(b"IEND"), 0xAE42_6082);
    assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    assert_eq!(adler32(b"Wikipedia"), 0x11E6_0398);
    assert_eq!(adler32(b""), 1);
    assert_eq!(
        piece(b"IEND", &[]),
        [0, 0, 0, 0, b'I', b'E', b'N', b'D', 0xAE, 0x42, 0x60, 0x82]
    );

    let good = Png::new(2, 1, 8, 0).swatches(&[1, 2]).bytes();
    assert_eq!(good[..8], SIGNATURE);
    let types: Vec<[u8; 4]> = walks(&good).unwrap().iter().map(|t| t.kind).collect();
    assert_eq!(types, [*b"IHDR", *b"IDAT", *b"IEND"]);
    let mut with_garbage = good.clone();
    with_garbage.push(0);
    assert!(walks(&with_garbage).is_err());
    assert!(walks(&good[..good.len() - 1]).is_err());
}

// ---------------------------------------------------------------------------------------
// Punto 1: ida y vuelta sintética (filas 1 a 6).
// ---------------------------------------------------------------------------------------

/// Píxel determinista y distinto de sus vecinos para los modos no indexados.
fn pixel_of_pattern(mode: Mode, k: usize) -> Pixel {
    let b = |m: usize, c: usize| (k * m + c) as u8;
    match mode {
        Mode::Rgba => rgba(b(7, 1), b(13, 2), b(29, 3), b(31, 5)),
        Mode::Gray => Pixel::Gray(b(37, 11)),
        Mode::GrayAlpha => Pixel::GrayAlpha {
            gray: b(37, 11),
            alpha: b(53, 7),
        },
        Mode::Indexed => unreachable!("the indexed mode needs a palette"),
    }
}

/// Paleta determinista de `n` entradas, con alfas distintos.
fn colors_of_patron(n: usize) -> Vec<Rgba> {
    (0..n)
        .map(|i| {
            let b = |m: usize, c: usize| (i * m + c) as u8;
            Rgba::new(b(7, 1), b(13, 2), b(29, 3), b(31, 5))
        })
        .collect()
}

#[test]
fn row_01_the_four_modes_in_four_shapes() {
    for (w, h) in [(1u32, 1u32), (3, 3), (1, 300), (300, 1)] {
        let n = (w * h) as usize;
        for mode in [Mode::Rgba, Mode::Gray, Mode::GrayAlpha] {
            let img = fills(
                Image::new(w, h, mode).unwrap(),
                (0..n).map(|k| pixel_of_pattern(mode, k)),
            );
            checks_exact(&img, &format!("{mode:?} {w}x{h}"));
        }

        // Indexado con una paleta de hasta 200 entradas (todas distintas si caben) y el
        // píxel k apuntando a la entrada k mod len.
        let len = n.min(200);
        let img = indexed(
            w,
            &colors_of_patron(len),
            None,
            &(0..n).map(|k| (k % len) as u8).collect::<Vec<u8>>(),
        );
        checks_exact(&img, &format!("Indexed {w}x{h}"));

        // Indexado con la paleta mínima: una sola entrada.
        let img = indexed(w, &[Rgba::new(11, 22, 33, 44)], None, &vec![0u8; n]);
        let read = checks_exact(&img, &format!("Indexed de 1 entrada {w}x{h}"));
        assert_eq!(read.palette().unwrap().len(), 1);
    }
}

#[test]
fn row_02_indexed_with_palette_of_256_entries() {
    let colors: Vec<Rgba> = (0..=255u8)
        .map(|i| Rgba::new(i, 255 - i, i ^ 0x5A, i.wrapping_mul(3)))
        .collect();
    let px: Vec<u8> = (0..=255u8).collect();
    let img = indexed(256, &colors, None, &px);
    let read = checks_exact(&img, "paleta de 256");
    assert_eq!(read.palette().unwrap().len(), 256);
    assert_eq!(pixels_2(&read), indices(&px));
}

#[test]
fn row_03_duplicate_colors_are_not_merged() {
    let colors = [
        Rgba::new(9, 9, 9, 255),
        Rgba::new(9, 9, 9, 255),
        Rgba::new(9, 9, 9, 128),
        Rgba::new(9, 9, 9, 255),
    ];
    let img = indexed(4, &colors, None, &[0, 1, 2, 3]);
    let read = checks_exact(&img, "duplicados");
    let palette = read.palette().unwrap();
    assert_eq!(palette.len(), 4);
    assert_eq!(palette.colors(), colors);
    assert_eq!(pixels_2(&read), indices(&[0, 1, 2, 3]));
}

#[test]
fn row_04_alphas_0_1_254_and_255_without_premultiplying() {
    let alphas = [0u8, 1, 254, 255];

    // GrayAlpha: gris distinto de 0 también con alfa 0.
    let ga = fills(
        Image::new(4, 1, Mode::GrayAlpha).unwrap(),
        alphas
            .iter()
            .enumerate()
            .map(|(i, &alpha)| Pixel::GrayAlpha {
                gray: 77 + i as u8,
                alpha,
            }),
    );
    let read = checks_exact(&ga, "GrayAlpha");
    assert_eq!(
        pixels_2(&read),
        grays_alpha(&[(77, 0), (78, 1), (79, 254), (80, 255)])
    );

    // Rgba: RGB distinto de 0 también con alfa 0.
    let c = fills(
        Image::new(4, 1, Mode::Rgba).unwrap(),
        alphas
            .iter()
            .enumerate()
            .map(|(i, &a)| rgba(10 + i as u8, 20 + i as u8, 30 + i as u8, a)),
    );
    let read = checks_exact(&c, "Rgba");
    assert_eq!(
        pixels_2(&read),
        [
            rgba(10, 20, 30, 0),
            rgba(11, 21, 31, 1),
            rgba(12, 22, 32, 254),
            rgba(13, 23, 33, 255)
        ]
    );

    // Paleta con esos alfas (sin índice transparente): la entrada 0 conserva su RGB.
    let colors: Vec<Rgba> = alphas
        .iter()
        .enumerate()
        .map(|(i, &a)| Rgba::new(40 + i as u8, 50 + i as u8, 60 + i as u8, a))
        .collect();
    let img = indexed(4, &colors, None, &[0, 1, 2, 3]);
    let read = checks_exact(&img, "paleta con alfas");
    let palette = read.palette().unwrap();
    assert_eq!(palette.get(0).unwrap(), Rgba::new(40, 50, 60, 0));
    assert_eq!(palette.get(1).unwrap(), Rgba::new(41, 51, 61, 1));
    assert_eq!(palette.get(2).unwrap(), Rgba::new(42, 52, 62, 254));
    assert_eq!(palette.get(3).unwrap(), Rgba::new(43, 53, 63, 255));
    assert_eq!(palette.transparent_index(), None);
}

#[test]
fn row_05_transparent_index_comes_out_resolved_and_without_index() {
    let c = Rgba::new;
    // (colores, índice transparente, píxeles)
    let cases: [(Vec<Rgba>, u8, Vec<u8>); 4] = [
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
    for (colors, t, px) in cases {
        let img = indexed(px.len() as u32, &colors, Some(t), &px);
        assert_eq!(img.palette().unwrap().transparent_index(), Some(t));
        let read = checks_exact(&img, &format!("transparente {t}"));
        let palette = read.palette().unwrap();
        assert_eq!(palette.transparent_index(), None, "transparente {t}");
        assert_eq!(palette.len(), colors.len(), "transparente {t}");
        for (i, original) in colors.iter().enumerate() {
            let expected = if i == usize::from(t) {
                Rgba { a: 0, ..*original }
            } else {
                *original
            };
            assert_eq!(
                palette.colors()[i],
                expected,
                "transparente {t}, entrada {i}"
            );
        }
        assert_eq!(pixels_2(&read), indices(&px), "transparente {t}");
    }
}

// Propiedad de la fila 6: imágenes aleatorias en los cuatro modos, con las formas alargadas
// y las paletas de hasta 256 entradas con duplicados que la propiedad 1 de `px-formats`
// (lados de 1 a 8) no cubre.

/// Alfa: con frecuencia uno de los valores de borde, si no cualquiera.
fn alpha_2() -> impl Strategy<Value = u8> {
    prop_oneof![select(vec![0u8, 1, 254, 255]), any::<u8>()]
}

/// RGB cualquiera (también con alfa 0).
fn color_random() -> impl Strategy<Value = Rgba> {
    (any::<u8>(), any::<u8>(), any::<u8>(), alpha_2())
        .prop_map(|(r, g, b, a)| Rgba::new(r, g, b, a))
}

/// Color de un grupo de 3, para que las paletas tengan entradas duplicadas.
fn color_of_group() -> impl Strategy<Value = Rgba> {
    select(vec![
        Rgba::new(9, 9, 9, 255),
        Rgba::new(9, 9, 9, 128),
        Rgba::new(200, 100, 50, 0),
    ])
}

/// 1 × 1, 1 × N, N × 1 (N de 2 a 300) y A × B (de 1 a 48 de lado).
fn shape() -> impl Strategy<Value = (u32, u32)> {
    prop_oneof![
        Just((1u32, 1u32)),
        (2u32..=300).prop_map(|n| (1, n)),
        (2u32..=300).prop_map(|n| (n, 1)),
        (1u32..=48, 1u32..=48),
    ]
}

/// Imagen de cualquiera de los cuatro modos con una forma de `shape()`.
fn image_arbitrary() -> BoxedStrategy<Image> {
    shape()
        .prop_flat_map(|(w, h)| {
            let n = (w * h) as usize;
            prop_oneof![
                vec(color_random(), n).prop_map(move |px| fills(
                    Image::new(w, h, Mode::Rgba).unwrap(),
                    px.into_iter().map(Pixel::Rgba)
                )),
                vec(any::<u8>(), n).prop_map(move |px| fills(
                    Image::new(w, h, Mode::Gray).unwrap(),
                    px.into_iter().map(Pixel::Gray)
                )),
                vec((any::<u8>(), alpha_2()), n).prop_map(move |px| fills(
                    Image::new(w, h, Mode::GrayAlpha).unwrap(),
                    px.into_iter()
                        .map(|(gray, alpha)| Pixel::GrayAlpha { gray, alpha })
                )),
                (1usize..=256)
                    .prop_flat_map(move |len| {
                        (
                            prop_oneof![vec(color_of_group(), len), vec(color_random(), len)],
                            proptest::option::of(0..len),
                            vec(0..len, n),
                        )
                    })
                    .prop_map(move |(colors, transparent, px)| {
                        let mut p = Palette::from_colors(colors).unwrap();
                        p.set_transparent_index(transparent.map(|t| t as u8))
                            .unwrap();
                        fills(
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
    fn row_06_property_exact_round_trip(img in image_arbitrary()) {
        let bytes = write_png(&img).unwrap();
        let r = read_png(&bytes).unwrap();
        prop_assert!(r.report.is_empty());
        prop_assert!(r.is_exact());
        prop_assert_eq!(r.image, normalized(&img));
    }
}

// ---------------------------------------------------------------------------------------
// Punto 2: conversiones declaradas (filas 7 a 9).
// ---------------------------------------------------------------------------------------

#[test]
fn row_07_gray_of_1_2_and_4_bits_is_scaled() {
    // 1 bit: 0 y 255.
    let r1 = read_bytes(&Png::new(2, 1, 1, 0).swatches(&[0, 1]).bytes());
    assert_eq!(r1.image.mode(), Mode::Gray);
    assert_eq!(pixels_2(&r1.image), grays(&[0, 255]));
    assert_eq!(r1.report, [Conversion::DepthExpanded { from_bits: 1 }]);
    assert!(!r1.is_exact());

    // 2 bits: 0, 85, 170, 255.
    let r2 = read_bytes(&Png::new(4, 1, 2, 0).swatches(&[0, 1, 2, 3]).bytes());
    assert_eq!(r2.image.mode(), Mode::Gray);
    assert_eq!(pixels_2(&r2.image), grays(&[0, 85, 170, 255]));
    assert_eq!(r2.report, [Conversion::DepthExpanded { from_bits: 2 }]);
    assert!(!r2.is_exact());

    // 4 bits, todos los valores: v * 17 (0, 17, 34, ..., 255).
    let all: Vec<u16> = (0..16).collect();
    let r4 = read_bytes(&Png::new(16, 1, 4, 0).swatches(&all).bytes());
    let expected_2: Vec<u8> = (0..16u8).map(|v| v * 17).collect();
    assert_eq!(expected_2[15], 255);
    assert_eq!(r4.image.mode(), Mode::Gray);
    assert_eq!(pixels_2(&r4.image), grays(&expected_2));
    assert_eq!(r4.report, [Conversion::DepthExpanded { from_bits: 4 }]);
    assert!(!r4.is_exact());
}

#[test]
fn row_08_sixteen_bits_reduce_with_rounding() {
    // (muestra de 16 bits, valor de 8 bits esperado). Redondeo, no el byte alto:
    // 0x01FF da 2 y no 1.
    let table: [(u16, u8); 5] = [
        (0x0000, 0),
        (0x0080, 0),
        (0x0081, 1),
        (0x01FF, 2),
        (0xFFFF, 255),
    ];
    let m = |i: usize| table[i % 5].0;
    let v = |i: usize| table[i % 5].1;

    // Gris 16.
    let swatches_g: Vec<u16> = (0..5).map(m).collect();
    let g = read_bytes(&Png::new(5, 1, 16, 0).swatches(&swatches_g).bytes());
    assert_eq!(g.image.mode(), Mode::Gray);
    assert_eq!(pixels_2(&g.image), grays(&[0, 0, 1, 2, 255]));
    assert_eq!(g.report, [Conversion::Depth16Reduced]);
    assert!(!g.is_exact());

    // Gris con alfa 16: gris = tabla[i], alfa = tabla[4 - i].
    let swatches_ga: Vec<u16> = (0..5).flat_map(|i| [m(i), m(4 - i)]).collect();
    let ga = read_bytes(&Png::new(5, 1, 16, 4).swatches(&swatches_ga).bytes());
    assert_eq!(ga.image.mode(), Mode::GrayAlpha);
    let expected_ga: Vec<(u8, u8)> = (0..5).map(|i| (v(i), v(4 - i))).collect();
    assert_eq!(expected_ga, [(0, 255), (0, 2), (1, 1), (2, 0), (255, 0)]);
    assert_eq!(pixels_2(&ga.image), grays_alpha(&expected_ga));
    assert_eq!(ga.report, [Conversion::Depth16Reduced]);
    assert!(!ga.is_exact());

    // RGBA 16: el píxel i usa tabla[i], tabla[i + 1], tabla[i + 2] y tabla[i + 3].
    let swatches_c: Vec<u16> = (0..5)
        .flat_map(|i| [m(i), m(i + 1), m(i + 2), m(i + 3)])
        .collect();
    let c = read_bytes(&Png::new(5, 1, 16, 6).swatches(&swatches_c).bytes());
    assert_eq!(c.image.mode(), Mode::Rgba);
    let expected_c = [
        rgba(0, 0, 1, 2),
        rgba(0, 1, 2, 255),
        rgba(1, 2, 255, 0),
        rgba(2, 255, 0, 0),
        rgba(255, 0, 0, 1),
    ];
    assert_eq!(pixels_2(&c.image), expected_c);
    assert_eq!(c.report, [Conversion::Depth16Reduced]);
    assert!(!c.is_exact());

    // RGB 16 (0x0102, 0x0304, 0xFFFF): Rgba(1, 3, 255, 255).
    let rgb = read_bytes(
        &Png::new(1, 1, 16, 2)
            .swatches(&[0x0102, 0x0304, 0xFFFF])
            .bytes(),
    );
    assert_eq!(rgb.image.mode(), Mode::Rgba);
    assert_eq!(pixels_2(&rgb.image), [rgba(1, 3, 255, 255)]);
    assert_eq!(
        rgb.report,
        [Conversion::Depth16Reduced, Conversion::RgbToRgba]
    );
    assert!(!rgb.is_exact());
}

#[test]
fn row_09_rgb_and_color_keys_trns() {
    // RGB 8 sin tRNS.
    let r = read_bytes(&Png::new(1, 1, 8, 2).swatches(&[1, 2, 3]).bytes());
    assert_eq!(r.image.mode(), Mode::Rgba);
    assert_eq!(pixels_2(&r.image), [rgba(1, 2, 3, 255)]);
    assert_eq!(r.report, [Conversion::RgbToRgba]);
    assert!(!r.is_exact());

    // tRNS de gris 8 con clave 7 (dos bytes, big-endian) y píxeles [7, 8].
    let g8 = read_bytes(
        &Png::new(2, 1, 8, 0)
            .swatches(&[7, 8])
            .before(trns(&[0, 7]))
            .bytes(),
    );
    assert_eq!(g8.image.mode(), Mode::GrayAlpha);
    assert_eq!(pixels_2(&g8.image), grays_alpha(&[(7, 0), (8, 255)]));
    assert_eq!(g8.report, [Conversion::ColorKeyToAlpha]);
    assert!(!g8.is_exact());

    // Gris de 2 bits con clave 1 y píxeles [1, 2]: 1 -> 85 transparente, 2 -> 170 opaco.
    let g2 = read_bytes(
        &Png::new(2, 1, 2, 0)
            .swatches(&[1, 2])
            .before(trns(&[0, 1]))
            .bytes(),
    );
    assert_eq!(g2.image.mode(), Mode::GrayAlpha);
    assert_eq!(pixels_2(&g2.image), grays_alpha(&[(85, 0), (170, 255)]));
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
    let c8 = read_bytes(
        &Png::new(2, 1, 8, 2)
            .swatches(&[1, 2, 3, 1, 2, 4])
            .before(trns(&[0, 1, 0, 2, 0, 3]))
            .bytes(),
    );
    assert_eq!(c8.image.mode(), Mode::Rgba);
    assert_eq!(pixels_2(&c8.image), [rgba(1, 2, 3, 0), rgba(1, 2, 4, 255)]);
    assert_eq!(
        c8.report,
        [Conversion::RgbToRgba, Conversion::ColorKeyToAlpha]
    );
    assert!(!c8.is_exact());

    // RGB 16 con tRNS: la clave se compara con las muestras de 16 bits originales, antes
    // de reducir. 0xFFFF se reduce a 255 y 0xFF00 a 254 (65280 / 257 = 254,01).
    let c16 = read_bytes(
        &Png::new(2, 1, 16, 2)
            .swatches(&[0x0102, 0x0304, 0xFFFF, 0x0102, 0x0304, 0xFF00])
            .before(trns(&[0x01, 0x02, 0x03, 0x04, 0xFF, 0xFF]))
            .bytes(),
    );
    assert_eq!(c16.image.mode(), Mode::Rgba);
    assert_eq!(
        pixels_2(&c16.image),
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
fn row_10_truncated_corrupt_files_or_with_garbage() {
    // Indexado 4 x 4 escrito por `write_png`.
    let colors = [
        Rgba::new(1, 2, 3, 255),
        Rgba::new(4, 5, 6, 128),
        Rgba::new(7, 8, 9, 255),
    ];
    let px: Vec<u8> = (0..16).map(|k| (k % 3) as u8).collect();
    let valid = write_png(&indexed(4, &colors, None, &px)).unwrap();
    assert!(read_png(&valid).is_ok());
    assert!(walks(&valid).is_ok());

    // Cada prefijo estricto, de 0 a len - 1 bytes.
    for n in 0..valid.len() {
        let r = read_png(&valid[..n]);
        assert!(es_decode(&r), "prefijo de {n} bytes: {r:?}");
    }

    // CRC erróneo en IHDR (bytes 29..33: 8 de firma, 4 de longitud, 4 de tipo y 13 de
    // datos), en IDAT y en IEND (los 4 últimos bytes del fichero).
    let (idat, length_idat) = position_idat(&valid);
    let positions = [
        ("IHDR", 32),
        ("IDAT", idat + 8 + length_idat + 3),
        ("IEND", valid.len() - 1),
    ];
    for (name_2, pos) in positions {
        let mut broken = valid.clone();
        broken[pos] ^= 0xFF;
        let r = read_png(&broken);
        assert!(es_decode(&r), "wrong CRC in {name_2}: {r:?}");
    }

    // Un byte de más tras IEND.
    let mut with_x = valid.clone();
    with_x.extend(b"x");
    assert_eq!(error_of(&with_x), PngError::TrailingData);

    // Un chunk tEXt completo tras IEND.
    let mut with_text = valid.clone();
    with_text.extend(piece(b"tEXt", b"k\0v"));
    assert_eq!(error_of(&with_text), PngError::TrailingData);
}

#[test]
fn row_11_invalid_palette_and_trns() {
    // Indexado de 8 bits con PLTE de 2 entradas y un píxel 5.
    let outside = Png::new(2, 1, 8, 3)
        .swatches(&[0, 5])
        .before(plte(&[(1, 1, 1), (2, 2, 2)]))
        .bytes();
    assert_eq!(
        error_of(&outside),
        PngError::PaletteIndexOutOfRange { index: 5, len: 2 }
    );

    // Indexado sin PLTE.
    let without_plte = Png::new(2, 1, 8, 3).swatches(&[0, 1]).bytes();
    assert_eq!(error_of(&without_plte), PngError::InvalidPalette);

    // tRNS de gris de 1 byte (debe ser de 2).
    let gray = Png::new(1, 1, 8, 0)
        .swatches(&[1])
        .before(trns(&[7]))
        .bytes();
    assert_eq!(error_of(&gray), PngError::InvalidTrns);

    // tRNS de RGB de 4 bytes (debe ser de 6).
    let rgb = Png::new(1, 1, 8, 2)
        .swatches(&[1, 2, 3])
        .before(trns(&[0, 1, 0, 2]))
        .bytes();
    assert_eq!(error_of(&rgb), PngError::InvalidTrns);

    // tRNS de 3 entradas con PLTE de 2.
    let length = Png::new(2, 1, 8, 3)
        .swatches(&[0, 1])
        .before(plte(&[(1, 1, 1), (2, 2, 2)]))
        .before(trns(&[0, 1, 2]))
        .bytes();
    assert_eq!(error_of(&length), PngError::InvalidTrns);
}

#[test]
fn row_12_huge_header_with_minimal_data_is_too_large() {
    // Gris 8 de 32769 x 32768: 1 byte por encima de `Image::MAX_BYTES` (2^30). El IDAT es
    // mínimo: no se reserva nada y no hay pánico.
    let almost = Png::new(32769, 32768, 8, 0).bytes();
    assert_eq!(
        error_of(&almost),
        PngError::TooLarge {
            width: 32769,
            height: 32768
        }
    );

    // Gris 8 de 100000 x 100000.
    let huge = Png::new(100_000, 100_000, 8, 0).bytes();
    assert_eq!(
        error_of(&huge),
        PngError::TooLarge {
            width: 100_000,
            height: 100_000
        }
    );
}
