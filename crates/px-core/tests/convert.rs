//! Tests de la conversión entre modos, parte de la función pura
//! (specs/hito-2-conversion-modos.md): `convert_image`, `target_byte_len`, `luma`,
//! `LUMA_WEIGHTS`, `Allowed`, `Conversion`, `ConversionReport` y `ConvertError`.
//!
//! Una prueba por cada fila 1-12, 14 (solo `target_byte_len`), 16, 17 y 18 de la tabla de
//! casos límite (`fila_NN_...`), una prueba grande con `#[ignore]` y las propiedades 1 a 4.
//! Las imágenes son minúsculas (hasta 6 x 6, y hasta 24 x 24 en la propiedad 4).
//!
//! Los oráculos de las propiedades 2, 3 y 4 son ingenuos y no comparten código ni forma de
//! cálculo con la spec de la implementación: píxel a píxel, con búsqueda lineal y con
//! aritmética en `i128` (la luminancia como redondeo de una fracción exacta; la composición
//! como el entero más cercano a la fracción, buscado entre los 256 candidatos).
//!
//! Corren en el perfil de test (debug, con `overflow-checks`): un desbordamiento de la
//! implementación aparecería como pánico.

use std::error::Error;
use std::fmt::{Debug, Display};

use proptest::prelude::*;
use proptest::test_runner::TestCaseError;
use px_core::{
    Allowed, Conversion, ConversionReport, ConvertError, Image, LUMA_WEIGHTS, Mode, Palette, Pixel,
    Rgba, convert_image, luma, target_byte_len,
};

// Comprobaciones en tiempo de compilación de los traits exigidos.
fn assert_datum_copyable<T: Clone + Copy + Debug + PartialEq + Eq>() {}
fn assert_error_traits<T: Debug + Clone + PartialEq + Eq + Display + Error>() {}
fn assert_default<T: Default>() {}

// Constantes de la notación de la tabla: `N`, `L`, `A` y `A0`.

const N: Allowed = Allowed::NOTHING;
const L: Allowed = Allowed {
    luma: true,
    drop_alpha: None,
};
const A: Allowed = Allowed {
    luma: false,
    drop_alpha: Some([255, 255, 255]),
};
const A0: Allowed = Allowed {
    luma: false,
    drop_alpha: Some([0, 0, 0]),
};
const L_Y_A: Allowed = Allowed {
    luma: true,
    drop_alpha: Some([255, 255, 255]),
};

const ALL_2: [Mode; 4] = [Mode::Rgba, Mode::GrayAlpha, Mode::Gray, Mode::Indexed];

// Construcción y lectura de imágenes.

fn rgba(r: u8, g: u8, b: u8, a: u8) -> Rgba {
    Rgba::new(r, g, b, a)
}

fn opaque(r: u8, g: u8, b: u8) -> Rgba {
    Rgba::opaque(r, g, b)
}

fn transparent() -> Rgba {
    Rgba::new(0, 0, 0, 0)
}

fn report_2(color_lost: u64, alpha_lost: u64) -> ConversionReport {
    ConversionReport {
        color_lost,
        alpha_lost,
    }
}

/// Imagen de un modo sin paleta con los píxeles dados, fila a fila.
fn image_2(width: u32, height: u32, mode: Mode, pixels: &[Pixel]) -> Image {
    assert_eq!(pixels.len(), (width * height) as usize);
    let mut img = Image::new(width, height, mode).unwrap();
    for (i, &p) in pixels.iter().enumerate() {
        img.set_pixel(i as u32 % width, i as u32 / width, p)
            .unwrap();
    }
    img
}

fn rgba_img(width: u32, height: u32, pixels: &[(u8, u8, u8, u8)]) -> Image {
    let v: Vec<Pixel> = pixels
        .iter()
        .map(|&(r, g, b, a)| Pixel::Rgba(rgba(r, g, b, a)))
        .collect();
    image_2(width, height, Mode::Rgba, &v)
}

fn gray_img(width: u32, height: u32, pixels: &[u8]) -> Image {
    let v: Vec<Pixel> = pixels.iter().map(|&g| Pixel::Gray(g)).collect();
    image_2(width, height, Mode::Gray, &v)
}

fn ga_img(width: u32, height: u32, pixels: &[(u8, u8)]) -> Image {
    let v: Vec<Pixel> = pixels
        .iter()
        .map(|&(gray, alpha)| Pixel::GrayAlpha { gray, alpha })
        .collect();
    image_2(width, height, Mode::GrayAlpha, &v)
}

/// Imagen indexada con la paleta, el índice transparente y los índices dados.
fn indexed_img(
    width: u32,
    height: u32,
    colors_2: &[Rgba],
    transparent: Option<u8>,
    indices: &[u8],
) -> Image {
    assert_eq!(indices.len(), (width * height) as usize);
    let mut palette_2 = Palette::from_colors(colors_2.iter().copied()).unwrap();
    palette_2.set_transparent_index(transparent).unwrap();
    let mut img = Image::new_indexed(width, height, palette_2).unwrap();
    for (i, &p) in indices.iter().enumerate() {
        img.set_pixel(i as u32 % width, i as u32 / width, Pixel::Indexed(p))
            .unwrap();
    }
    img
}

/// Todos los píxeles de la imagen, fila a fila.
fn pixels(img: &Image) -> Vec<Pixel> {
    (0..img.height())
        .flat_map(|y| (0..img.width()).map(move |x| (x, y)))
        .map(|(x, y)| img.get_pixel(x, y).unwrap())
        .collect()
}

fn rgbas(img: &Image) -> Vec<Rgba> {
    assert_eq!(img.mode(), Mode::Rgba);
    pixels(img)
        .into_iter()
        .map(|p| match p {
            Pixel::Rgba(c) => c,
            other_2 => panic!("píxel {other_2:?} en una imagen Rgba"),
        })
        .collect()
}

fn grays(img: &Image) -> Vec<u8> {
    assert_eq!(img.mode(), Mode::Gray);
    pixels(img)
        .into_iter()
        .map(|p| match p {
            Pixel::Gray(g) => g,
            other_2 => panic!("píxel {other_2:?} en una imagen Gray"),
        })
        .collect()
}

fn pares(img: &Image) -> Vec<(u8, u8)> {
    assert_eq!(img.mode(), Mode::GrayAlpha);
    pixels(img)
        .into_iter()
        .map(|p| match p {
            Pixel::GrayAlpha { gray, alpha } => (gray, alpha),
            other_2 => panic!("píxel {other_2:?} en una imagen GrayAlpha"),
        })
        .collect()
}

fn indices(img: &Image) -> Vec<u8> {
    assert_eq!(img.mode(), Mode::Indexed);
    pixels(img)
        .into_iter()
        .map(|p| match p {
            Pixel::Indexed(i) => i,
            other_2 => panic!("píxel {other_2:?} en una imagen Indexed"),
        })
        .collect()
}

/// Entradas de la paleta tal como están guardadas.
fn palette_of(img: &Image) -> Vec<Rgba> {
    img.palette().expect("imagen indexada").colors().to_vec()
}

fn index_transparent(img: &Image) -> Option<u8> {
    img.palette().expect("imagen indexada").transparent_index()
}

/// Color resuelto de cada píxel de cualquier imagen (los grises con alfa 255, el alfa
/// directo, y la paleta resuelta), sin canonizar.
fn resolved(img: &Image) -> Vec<Rgba> {
    pixels(img)
        .into_iter()
        .map(|p| match p {
            Pixel::Rgba(c) => c,
            Pixel::Gray(g) => rgba(g, g, g, 255),
            Pixel::GrayAlpha { gray, alpha } => rgba(gray, gray, gray, alpha),
            Pixel::Indexed(i) => img.palette().unwrap().resolved(i).unwrap(),
        })
        .collect()
}

// Llamadas a `convert_image` que comprueban que la imagen de origen no cambia.

fn converts(img: &Image, target: Mode, allowed: Allowed) -> Result<Conversion, ConvertError> {
    let antes = img.clone();
    let result = convert_image(img, target, allowed);
    assert_eq!(img, &antes, "convert_image tocó la imagen de origen");
    result
}

/// Convierte y espera `Ok` con el informe `color/alpha`; devuelve la imagen convertida.
fn ok(img: &Image, target: Mode, allowed: Allowed, color: u64, alpha: u64) -> Image {
    let conversion = converts(img, target, allowed)
        .unwrap_or_else(|e| panic!("{target:?} con {allowed:?} falló: {e:?}"));
    assert_eq!(conversion.report, report_2(color, alpha), "informe");
    assert_eq!(conversion.image.mode(), target, "modo del resultado");
    assert_eq!(conversion.image.width(), img.width());
    assert_eq!(conversion.image.height(), img.height());
    conversion.image
}

/// Convierte y espera `Err(LossNotAllowed { color/alpha })`.
fn rejects(img: &Image, target: Mode, allowed: Allowed, color: u64, alpha: u64) {
    let error = converts(img, target, allowed).err();
    assert_eq!(
        error,
        Some(ConvertError::LossNotAllowed {
            report: report_2(color, alpha)
        }),
        "{target:?} con {allowed:?}"
    );
}

/// `c(i) = (i mod 256, i / 256, 0, 255)` de la fila 9.
fn c(i: usize) -> Rgba {
    opaque((i % 256) as u8, (i / 256) as u8, 0)
}

fn contains_more_of_256(e: &ConvertError) -> bool {
    e.to_string().contains("más de 256 colores")
}

// Tipos nuevos de la API.

#[test]
fn traits_of_the_new_types() {
    assert_datum_copyable::<Allowed>();
    assert_datum_copyable::<ConversionReport>();
    assert_default::<ConversionReport>();
    assert_error_traits::<ConvertError>();

    // `ConversionReport::default()` es todo a cero.
    assert_eq!(ConversionReport::default(), report_2(0, 0));

    // `Allowed::NOTHING` es una constante, y no pide nada.
    const NOTHING: Allowed = Allowed::NOTHING;
    assert_eq!(
        NOTHING,
        Allowed {
            luma: false,
            drop_alpha: None
        }
    );
    assert_ne!(NOTHING, L);
    assert_ne!(NOTHING, A);
    assert_ne!(A, A0);
    let copy = A;
    assert_eq!(copy, A);
}

#[test]
fn the_errors_have_message() {
    let errors = [
        ConvertError::LossNotAllowed {
            report: report_2(2, 1),
        },
        ConvertError::TooManyColors { at_least: 257 },
        ConvertError::TooLarge {
            width: 16385,
            height: 16384,
        },
    ];
    for e in &errors {
        assert!(!e.to_string().is_empty(), "{e:?} sin mensaje");
    }
    assert!(contains_more_of_256(&errors[1]));
    assert_ne!(errors[0], errors[1]);
    assert_ne!(
        ConvertError::TooManyColors { at_least: 257 },
        ConvertError::TooManyColors { at_least: 300 }
    );
    assert_ne!(
        ConvertError::LossNotAllowed {
            report: report_2(1, 0)
        },
        ConvertError::LossNotAllowed {
            report: report_2(0, 1)
        }
    );
}

// Una prueba por fila de la tabla de casos límite.

#[test]
fn row_01_gray_to_the_other_three_modes() {
    let source = gray_img(3, 1, &[0, 127, 255]);

    let rgba_ = ok(&source, Mode::Rgba, N, 0, 0);
    assert_eq!(
        rgbas(&rgba_),
        [
            opaque(0, 0, 0),
            opaque(127, 127, 127),
            opaque(255, 255, 255)
        ]
    );

    let ga = ok(&source, Mode::GrayAlpha, N, 0, 0);
    assert_eq!(pares(&ga), [(0, 255), (127, 255), (255, 255)]);

    let indexed = ok(&source, Mode::Indexed, N, 0, 0);
    assert_eq!(
        palette_of(&indexed),
        [
            opaque(0, 0, 0),
            opaque(127, 127, 127),
            opaque(255, 255, 255)
        ]
    );
    assert_eq!(index_transparent(&indexed), None);
    assert_eq!(indices(&indexed), [0, 1, 2]);
}

#[test]
fn row_02_grayalpha_to_rgba_canonicalizes_the_transparent() {
    let source = ga_img(3, 1, &[(10, 0), (10, 255), (200, 128)]);
    let img = ok(&source, Mode::Rgba, N, 0, 0);
    assert_eq!(
        rgbas(&img),
        [
            rgba(0, 0, 0, 0),
            opaque(10, 10, 10),
            rgba(200, 200, 200, 128)
        ]
    );
}

#[test]
fn row_03_grayalpha_to_gray() {
    let source = ga_img(3, 1, &[(10, 0), (10, 255), (200, 128)]);

    rejects(&source, Mode::Gray, N, 0, 2);
    let with_background = ok(&source, Mode::Gray, A, 0, 2);
    assert_eq!(grays(&with_background), [255, 10, 227]);
    // Falta el alfa: con `luma` solo no basta.
    rejects(&source, Mode::Gray, L, 0, 2);
}

#[test]
fn row_04_rgba_primaries_to_gray() {
    let source = rgba_img(
        3,
        1,
        &[(255, 0, 0, 255), (0, 255, 0, 255), (0, 0, 255, 255)],
    );

    rejects(&source, Mode::Gray, N, 3, 0);
    let img = ok(&source, Mode::Gray, L, 3, 0);
    assert_eq!(grays(&img), [54, 182, 19]);
}

#[test]
fn row_05_red_semitransparent() {
    let source = rgba_img(1, 1, &[(255, 0, 0, 128)]);

    let ga = ok(&source, Mode::GrayAlpha, L, 1, 0);
    assert_eq!(pares(&ga), [(54, 128)]);

    rejects(&source, Mode::Gray, L, 1, 1);

    let gray_2 = ok(&source, Mode::Gray, L_Y_A, 1, 1);
    assert_eq!(grays(&gray_2), [154]);
}

#[test]
fn row_06_the_rgb_of_a_transparent_pixel_is_not_content() {
    let source = rgba_img(1, 1, &[(10, 20, 30, 0)]);

    // Transparente canónico, no (19, 0).
    let ga = ok(&source, Mode::GrayAlpha, N, 0, 0);
    assert_eq!(pares(&ga), [(0, 0)]);

    // Sin `luma`: el color de un `a = 0` no cuenta en ningún destino.
    let gray_2 = ok(&source, Mode::Gray, A, 0, 1);
    assert_eq!(grays(&gray_2), [255]);

    let indexed = ok(&source, Mode::Indexed, N, 0, 0);
    assert_eq!(palette_of(&indexed), [transparent()]);
    assert_eq!(index_transparent(&indexed), Some(0));
    assert_eq!(indices(&indexed), [0]);
}

#[test]
fn row_07_the_transparent_goes_to_index_0_even_if_it_appears_later() {
    let source = rgba_img(
        5,
        1,
        &[
            (5, 5, 5, 255),
            (9, 9, 9, 0),
            (5, 5, 5, 255),
            (1, 2, 3, 0),
            (7, 7, 7, 255),
        ],
    );

    let indexed = ok(&source, Mode::Indexed, N, 0, 0);
    assert_eq!(
        palette_of(&indexed),
        [transparent(), opaque(5, 5, 5), opaque(7, 7, 7)]
    );
    assert_eq!(index_transparent(&indexed), Some(0));
    assert_eq!(indices(&indexed), [1, 0, 1, 0, 2]);

    let ret = ok(&indexed, Mode::Rgba, N, 0, 0);
    assert_eq!(
        rgbas(&ret),
        [
            opaque(5, 5, 5),
            transparent(),
            opaque(5, 5, 5),
            transparent(),
            opaque(7, 7, 7)
        ]
    );
}

#[test]
fn row_08_the_semitransparent_is_one_more_color_and_the_round_trip_is_exact() {
    let source = rgba_img(
        3,
        1,
        &[(255, 0, 0, 128), (255, 0, 0, 255), (255, 0, 0, 128)],
    );

    let indexed = ok(&source, Mode::Indexed, N, 0, 0);
    assert_eq!(
        palette_of(&indexed),
        [rgba(255, 0, 0, 128), opaque(255, 0, 0)]
    );
    assert_eq!(indices(&indexed), [0, 1, 0]);
    assert_eq!(index_transparent(&indexed), None);

    let ret = ok(&indexed, Mode::Rgba, N, 0, 0);
    assert_eq!(ret, source);
}

#[test]
fn row_09_the_256_color_limit() {
    // (a) c(0)..c(255): 256 colores, en orden de columna.
    let a: Vec<Rgba> = (0..256).map(c).collect();
    let source = image_of_colors(256, 1, &a);
    let img = ok(&source, Mode::Indexed, N, 0, 0);
    assert_eq!(palette_of(&img), a);
    assert_eq!(index_transparent(&img), None);
    assert_eq!(indices(&img), (0..=255u8).collect::<Vec<u8>>());

    // (b) 255 opacos y un transparente al final: el transparente va al índice 0.
    let mut b: Vec<Rgba> = (0..255).map(c).collect();
    b.push(transparent());
    let source = image_of_colors(256, 1, &b);
    let img = ok(&source, Mode::Indexed, N, 0, 0);
    assert_eq!(img.palette().unwrap().len(), 256);
    let mut expected = vec![transparent()];
    expected.extend((0..255).map(c));
    assert_eq!(palette_of(&img), expected);
    assert_eq!(index_transparent(&img), Some(0));
    let mut expected_2_2: Vec<u8> = (1..=255u8).collect();
    expected_2_2.push(0);
    assert_eq!(indices(&img), expected_2_2);

    // (c) 256 opacos más un transparente: 257.
    let mut cc: Vec<Rgba> = (0..256).map(c).collect();
    cc.push(transparent());
    let source = image_of_colors(257, 1, &cc);
    let error = converts(&source, Mode::Indexed, N).err();
    assert_eq!(error, Some(ConvertError::TooManyColors { at_least: 257 }));
    assert!(contains_more_of_256(&error.unwrap()));

    // (d) c(0)..c(299): más de 256, no es el número exacto.
    let d: Vec<Rgba> = (0..300).map(c).collect();
    let source = image_of_colors(300, 1, &d);
    let error = converts(&source, Mode::Indexed, N).err();
    assert_eq!(error, Some(ConvertError::TooManyColors { at_least: 257 }));
    assert!(contains_more_of_256(&error.unwrap()));

    // (e) 300 píxeles transparentes con RGB distinto: una sola entrada.
    let e: Vec<Rgba> = (0..300)
        .map(|i| rgba((i % 256) as u8, (i / 256) as u8, 0, 0))
        .collect();
    let source = image_of_colors(300, 1, &e);
    let img = ok(&source, Mode::Indexed, N, 0, 0);
    assert_eq!(palette_of(&img), [transparent()]);
    assert_eq!(index_transparent(&img), Some(0));
    assert_eq!(indices(&img), vec![0u8; 300]);
}

/// Imagen Rgba de `width x height` con esos colores.
fn image_of_colors(width: u32, height: u32, colors_2: &[Rgba]) -> Image {
    let v: Vec<Pixel> = colors_2.iter().map(|&c| Pixel::Rgba(c)).collect();
    image_2(width, height, Mode::Rgba, &v)
}

#[test]
fn row_10_indexed_to_rgba_resolves_the_palette() {
    let source = row_10_source(&[0, 1, 2]);
    let img = ok(&source, Mode::Rgba, N, 0, 0);
    assert_eq!(
        rgbas(&img),
        [opaque(10, 10, 10), opaque(200, 0, 0), transparent()]
    );
}

/// Imagen indexada de las filas 10 y 11, con los píxeles dados.
fn row_10_source(pixels: &[u8]) -> Image {
    indexed_img(
        3,
        1,
        &[opaque(10, 10, 10), opaque(200, 0, 0), opaque(5, 5, 5)],
        Some(2),
        pixels,
    )
}

#[test]
fn row_11_indexed_to_gray_measures_the_used_pixels() {
    // Las entradas sin usar no cuentan.
    let img = ok(&row_10_source(&[0, 0, 0]), Mode::Gray, N, 0, 0);
    assert_eq!(grays(&img), [10, 10, 10]);

    // El píxel transparente es una pérdida de alfa.
    rejects(&row_10_source(&[0, 0, 2]), Mode::Gray, N, 0, 1);

    // Se cuentan píxeles, no entradas.
    rejects(&row_10_source(&[0, 1, 1]), Mode::Gray, N, 2, 0);
}

#[test]
fn row_12_the_same_mode_is_an_identical_copy() {
    let all_the_permissions = [
        N,
        L,
        A,
        A0,
        L_Y_A,
        Allowed {
            luma: true,
            drop_alpha: Some([7, 8, 9]),
        },
    ];

    let rgba_1 = rgba_img(1, 1, &[(9, 9, 9, 0)]);
    let indexed = indexed_img(1, 1, &[opaque(1, 2, 3), rgba(9, 9, 9, 0)], None, &[1]);
    let ga = ga_img(2, 1, &[(9, 0), (4, 77)]);
    let gray_2 = gray_img(2, 1, &[9, 4]);

    for source in [&rgba_1, &indexed, &ga, &gray_2] {
        for allowed in all_the_permissions {
            let img = ok(source, source.mode(), allowed, 0, 0);
            assert_eq!(&img, source, "{:?} con {allowed:?}", source.mode());
        }
    }
    // Sin canonizar: el (9, 9, 9, 0) sigue ahí.
    assert_eq!(rgbas(&rgba_1), [rgba(9, 9, 9, 0)]);
    assert_eq!(palette_of(&indexed)[1], rgba(9, 9, 9, 0));
}

#[test]
fn row_14_target_byte_len() {
    assert_eq!(
        target_byte_len(16385, 16384, Mode::Rgba),
        Err(ConvertError::TooLarge {
            width: 16385,
            height: 16384
        })
    );
    assert_eq!(target_byte_len(16384, 16384, Mode::Rgba), Ok(1 << 30));
    assert_eq!(target_byte_len(16385, 16384, Mode::Gray), Ok(268_451_840));

    // Los bytes por píxel de cada modo, en tamaños minúsculos.
    assert_eq!(target_byte_len(3, 2, Mode::Rgba), Ok(24));
    assert_eq!(target_byte_len(3, 2, Mode::GrayAlpha), Ok(12));
    assert_eq!(target_byte_len(3, 2, Mode::Gray), Ok(6));
    assert_eq!(target_byte_len(3, 2, Mode::Indexed), Ok(6));

    // Dimensiones enormes: error, no pánico ni desbordamiento.
    assert_eq!(
        target_byte_len(u32::MAX, u32::MAX, Mode::Rgba),
        Err(ConvertError::TooLarge {
            width: u32::MAX,
            height: u32::MAX
        })
    );
}

#[test]
fn row_16_gray_and_grayalpha_to_indexed_by_first_appearance() {
    let gray_2 = gray_img(4, 1, &[200, 3, 200, 3]);
    let img = ok(&gray_2, Mode::Indexed, N, 0, 0);
    assert_eq!(palette_of(&img), [opaque(200, 200, 200), opaque(3, 3, 3)]);
    assert_eq!(index_transparent(&img), None);
    assert_eq!(indices(&img), [0, 1, 0, 1]);

    let ga = ga_img(4, 1, &[(10, 0), (10, 255), (10, 0), (7, 255)]);
    let img = ok(&ga, Mode::Indexed, N, 0, 0);
    assert_eq!(
        palette_of(&img),
        [transparent(), opaque(10, 10, 10), opaque(7, 7, 7)]
    );
    assert_eq!(index_transparent(&img), Some(0));
    assert_eq!(indices(&img), [0, 1, 0, 2]);
}

#[test]
fn row_17_composition_with_rounding_to_nearest() {
    let source = ga_img(3, 1, &[(200, 0), (200, 128), (200, 255)]);

    let white = ok(&source, Mode::Gray, A, 0, 2);
    assert_eq!(grays(&white), [255, 227, 200]);

    let negro = ok(&source, Mode::Gray, A0, 0, 2);
    assert_eq!(grays(&negro), [0, 100, 200]);
}

#[test]
fn row_18_luma_by_hand_and_weights() {
    assert_eq!(luma(255, 0, 0), 54);
    assert_eq!(luma(0, 255, 0), 182);
    assert_eq!(luma(0, 0, 255), 19);

    assert_eq!(LUMA_WEIGHTS, [54, 183, 19]);
    assert_eq!(LUMA_WEIGHTS.iter().sum::<u32>(), 256);
}

/// Prueba grande de extremo a extremo (256 MiB): `TooLarge` sin llegar a leer un píxel.
#[test]
#[ignore = "reserva 256 MiB; se corre a mano con `cargo test -- --ignored`"]
fn gray_of_16385_by_16384_to_rgba_is_too_large() {
    let source = Image::new(16385, 16384, Mode::Gray).unwrap();
    let error = convert_image(&source, Mode::Rgba, Allowed::NOTHING).err();
    assert_eq!(
        error,
        Some(ConvertError::TooLarge {
            width: 16385,
            height: 16384
        })
    );
}

// Otras pruebas unitarias (no son filas de la tabla).

#[test]
fn luma_of_the_cube_extremes() {
    // Calculadas a mano con `(54 r + 183 g + 19 b + 128) >> 8`.
    let corners = [
        ((0, 0, 0), 0),
        ((255, 0, 0), 54),
        ((0, 255, 0), 182),
        ((0, 0, 255), 19),
        ((255, 255, 0), 236),
        ((255, 0, 255), 73),
        ((0, 255, 255), 201),
        ((255, 255, 255), 255),
    ];
    for ((r, g, b), expected_2) in corners {
        assert_eq!(luma(r, g, b), expected_2, "luma({r}, {g}, {b})");
    }
}

#[test]
fn luma_of_a_gray_is_the_gray_for_all_256_values() {
    for v in 0..=255u8 {
        assert_eq!(luma(v, v, v), v);
    }
}

// Estrategias.

/// Valores por canal que fuerzan grises, alfa 0 y repeticiones.
const CHANNELS: [u8; 5] = [0, 1, 127, 128, 255];

fn canal() -> impl Strategy<Value = u8> {
    prop_oneof![any::<u8>(), prop::sample::select(CHANNELS.to_vec())]
}

fn small() -> impl Strategy<Value = u8> {
    prop::sample::select(CHANNELS.to_vec())
}

fn background() -> impl Strategy<Value = [u8; 3]> {
    (canal(), canal(), canal()).prop_map(|(r, g, b)| [r, g, b])
}

/// Colores uniformes, del conjunto pequeño y grises con alfa de un conjunto pequeño.
fn color() -> impl Strategy<Value = Rgba> {
    prop_oneof![
        (any::<u8>(), any::<u8>(), any::<u8>(), any::<u8>())
            .prop_map(|(r, g, b, a)| rgba(r, g, b, a)),
        (small(), small(), small(), small()).prop_map(|(r, g, b, a)| rgba(r, g, b, a)),
        (canal(), small()).prop_map(|(v, a)| rgba(v, v, v, a)),
    ]
}

fn dims() -> impl Strategy<Value = (u32, u32)> {
    (1u32..=6, 1u32..=6)
}

fn mode_2() -> impl Strategy<Value = Mode> {
    prop::sample::select(ALL_2.to_vec())
}

fn permissions() -> impl Strategy<Value = Allowed> {
    (any::<bool>(), prop::option::of(background()))
        .prop_map(|(luma, drop_alpha)| Allowed { luma, drop_alpha })
}

/// Imagen Rgba de `w x h` con los primeros `w * h` colores de `v`.
fn rgba_de(w: u32, h: u32, v: &[Rgba]) -> Image {
    image_of_colors(w, h, &v[..(w * h) as usize])
}

/// Imagen indexada de `w x h` con esa paleta; el índice transparente y los píxeles se
/// reducen módulo la longitud de la paleta.
fn indexed_of(w: u32, h: u32, colors_2: &[Rgba], transparent: Option<u8>, indices: &[u8]) -> Image {
    let len = colors_2.len();
    let t = transparent.map(|t| (t as usize % len) as u8);
    let idx: Vec<u8> = indices[..(w * h) as usize]
        .iter()
        .map(|&i| (i as usize % len) as u8)
        .collect();
    indexed_img(w, h, colors_2, t, &idx)
}

/// Imagen arbitraria de un modo.
fn image_of_mode(mode: Mode) -> BoxedStrategy<Image> {
    let colors_2 = || prop::collection::vec(color(), 36);
    match mode {
        Mode::Rgba => (dims(), colors_2())
            .prop_map(|((w, h), v)| rgba_de(w, h, &v))
            .boxed(),
        Mode::GrayAlpha => (dims(), colors_2())
            .prop_map(|((w, h), v)| {
                let p: Vec<(u8, u8)> = v[..(w * h) as usize].iter().map(|c| (c.r, c.a)).collect();
                ga_img(w, h, &p)
            })
            .boxed(),
        Mode::Gray => (dims(), colors_2())
            .prop_map(|((w, h), v)| {
                let p: Vec<u8> = v[..(w * h) as usize].iter().map(|c| c.g).collect();
                gray_img(w, h, &p)
            })
            .boxed(),
        Mode::Indexed => (
            dims(),
            prop::collection::vec(color(), 1..=8),
            prop::option::of(any::<u8>()),
            prop::collection::vec(any::<u8>(), 36),
        )
            .prop_map(|((w, h), palette_2, t, idx)| indexed_of(w, h, &palette_2, t, &idx))
            .boxed(),
    }
}

fn image_any() -> impl Strategy<Value = Image> {
    mode_2().prop_flat_map(image_of_mode)
}

// Oráculos ingenuos.

/// Luminancia como redondeo (mitad hacia arriba) de la fracción exacta `sum / 256`, con
/// `sum = 54 r + 183 g + 19 b`: `floor(sum / 256 + 1/2) = floor((2 sum + 256) / 512)`.
fn luma_naive(r: u8, g: u8, b: u8) -> i128 {
    let sum = 54 * i128::from(r) + 183 * i128::from(g) + 19 * i128::from(b);
    (2 * sum + 256).div_euclid(512)
}

/// El entero `g` en `0..=255` más cercano a la fracción exacta `(c a + k (255 - a)) / 255`,
/// buscado probando los 256 candidatos (sin la forma `(... + 127) / 255`).
fn composite_naive(c: i128, k: i128, a: i128) -> i128 {
    let numerator = c * a + k * (255 - a);
    (0..=255i128)
        .min_by_key(|&g| (255 * g - numerator).abs())
        .unwrap()
}

/// Lo que el oráculo espera de una conversión.
struct Expected {
    /// Un valor por píxel: `Pixel::Rgba` con el color resuelto y canónico si el destino es
    /// `Rgba` o `Indexed`; el `Pixel` del modo en los demás casos.
    values: Vec<Pixel>,
    color_lost: u64,
    alpha_lost: u64,
}

/// Oráculo píxel a píxel de `img` -> `target` (distinto del modo de `img`), con el fondo de
/// `drop_alpha` si lo hay (sin fondo, los valores de gris con `a < 255` no se especifican).
fn oracle(img: &Image, target: Mode, background: Option<[u8; 3]>) -> Expected {
    assert_ne!(img.mode(), target);
    let mut expected_2 = Expected {
        values: Vec::new(),
        color_lost: 0,
        alpha_lost: 0,
    };
    for c in resolved(img) {
        let is_gray = c.r == c.g && c.g == c.b;
        let valor = match target {
            Mode::Rgba | Mode::Indexed => Pixel::Rgba(if c.a == 0 { transparent() } else { c }),
            Mode::GrayAlpha => {
                if c.a == 0 {
                    Pixel::GrayAlpha { gray: 0, alpha: 0 }
                } else {
                    if !is_gray {
                        expected_2.color_lost += 1;
                    }
                    Pixel::GrayAlpha {
                        gray: luma_naive(c.r, c.g, c.b) as u8,
                        alpha: c.a,
                    }
                }
            }
            Mode::Gray => {
                if c.a > 0 && !is_gray {
                    expected_2.color_lost += 1;
                }
                if c.a < 255 {
                    expected_2.alpha_lost += 1;
                }
                let gray_2 = if c.a == 0 {
                    0
                } else {
                    luma_naive(c.r, c.g, c.b)
                };
                let valor = if c.a == 255 {
                    gray_2
                } else {
                    match background {
                        Some([r, g, b]) => {
                            composite_naive(gray_2, luma_naive(r, g, b), i128::from(c.a))
                        }
                        None => 0,
                    }
                };
                Pixel::Gray(valor as u8)
            }
        };
        expected_2.values.push(valor);
    }
    expected_2
}

/// Valor de cada píxel de una imagen como lo compara el oráculo: el color resuelto para
/// `Indexed` y el propio píxel para los demás modos.
fn values(img: &Image) -> Vec<Pixel> {
    match img.mode() {
        Mode::Indexed => resolved(img).into_iter().map(Pixel::Rgba).collect(),
        _ => pixels(img),
    }
}

/// Contenido visible de una imagen, con `a = 0` canonizado y la paleta resuelta.
fn canonical(img: &Image) -> Vec<Pixel> {
    let canon = |c: Rgba| Pixel::Rgba(if c.a == 0 { transparent() } else { c });
    match img.mode() {
        Mode::Indexed => resolved(img).into_iter().map(canon).collect(),
        Mode::Rgba => pixels(img)
            .into_iter()
            .map(|p| match p {
                Pixel::Rgba(c) => canon(c),
                other_2 => other_2,
            })
            .collect(),
        Mode::GrayAlpha => pixels(img)
            .into_iter()
            .map(|p| match p {
                Pixel::GrayAlpha { alpha: 0, .. } => Pixel::GrayAlpha { gray: 0, alpha: 0 },
                other_2 => other_2,
            })
            .collect(),
        Mode::Gray => pixels(img),
    }
}

/// Color visible de cada píxel de cualquier imagen: el resuelto, con `a = 0` canonizado.
/// Es comparable entre imágenes de modos distintos.
fn visible(img: &Image) -> Vec<Rgba> {
    resolved(img)
        .into_iter()
        .map(|c| if c.a == 0 { transparent() } else { c })
        .collect()
}

// Propiedad 1: ida y vuelta de lo exacto.

/// `a -> b -> a` con `Allowed::NOTHING`, con las dos conversiones exactas.
fn round_trip(a: &Image, b: Mode) -> Result<(), TestCaseError> {
    let outward = convert_image(a, b, Allowed::NOTHING).map_err(|e| {
        TestCaseError::fail(format!("ida {:?} -> {b:?} falló: {e:?} ({a:?})", a.mode()))
    })?;
    prop_assert_eq!(outward.report, ConversionReport::default());
    prop_assert_eq!(outward.image.mode(), b);
    prop_assert_eq!(
        (outward.image.width(), outward.image.height()),
        (a.width(), a.height())
    );

    let ret = convert_image(&outward.image, a.mode(), Allowed::NOTHING).map_err(|e| {
        TestCaseError::fail(format!(
            "vuelta {b:?} -> {:?} falló: {e:?} ({a:?})",
            a.mode()
        ))
    })?;
    prop_assert_eq!(ret.report, ConversionReport::default());
    prop_assert_eq!(ret.image.mode(), a.mode());
    prop_assert_eq!(
        (ret.image.width(), ret.image.height()),
        (a.width(), a.height())
    );
    prop_assert_eq!(canonical(&ret.image), canonical(a));
    // El contenido visible tampoco cambia en el destino.
    prop_assert_eq!(visible(&outward.image), visible(a));
    if a.mode() == Mode::Gray {
        // Gray no tiene nada que canonizar: la imagen vuelve idéntica, bytes incluidos.
        prop_assert_eq!(&ret.image, a);
    }
    Ok(())
}

/// Color de una imagen indexada «gris»: `(v, v, v, a)` si `a > 0`; si no, el RGB se deja.
fn gray_or_transparent(c: Rgba) -> Rgba {
    if c.a > 0 { rgba(c.r, c.r, c.r, c.a) } else { c }
}

/// Imagen indexada cuyos píxeles usados son grises (con alfa cualquiera, o todos opacos si
/// `opaque`), con entradas sin usar arbitrarias detrás.
fn indexed_gray(opaque: bool) -> BoxedStrategy<Image> {
    (
        dims(),
        prop::collection::vec(color(), 1..=4),
        prop::collection::vec(color(), 0..=3),
        prop::option::of(any::<u8>()),
        prop::collection::vec(any::<u8>(), 36),
    )
        .prop_map(move |((w, h), used, garbage, t, idx)| {
            let n_used = used.len();
            let mut palette_2: Vec<Rgba> = used
                .iter()
                .map(|&c| {
                    if opaque {
                        rgba(c.r, c.r, c.r, 255)
                    } else {
                        gray_or_transparent(c)
                    }
                })
                .collect();
            palette_2.extend(garbage.iter().copied());
            // Con todo opaco, el índice transparente solo puede caer en una entrada sin usar.
            let t = match (opaque, t) {
                (true, Some(t)) if !garbage.is_empty() => {
                    Some((n_used + t as usize % garbage.len()) as u8)
                }
                (true, _) => None,
                (false, t) => t.map(|t| (t as usize % palette_2.len()) as u8),
            };
            let pixels: Vec<u8> = idx[..(w * h) as usize]
                .iter()
                .map(|&i| (i as usize % n_used) as u8)
                .collect();
            indexed_img(w, h, &palette_2, t, &pixels)
        })
        .boxed()
}

/// Casos exactos de la matriz: una imagen y los destinos a los que pasa sin pérdida.
fn case_exact() -> impl Strategy<Value = (Image, Vec<Mode>)> {
    let colors_2 = || (dims(), prop::collection::vec(color(), 36));
    let rgba_gray = colors_2()
        .prop_map(|((w, h), v)| {
            let v: Vec<Rgba> = v.into_iter().map(gray_or_transparent).collect();
            (rgba_de(w, h, &v), vec![Mode::GrayAlpha, Mode::Indexed])
        })
        .boxed();
    let rgba_gray_opaque = colors_2()
        .prop_map(|((w, h), v)| {
            let v: Vec<Rgba> = v.into_iter().map(|c| rgba(c.r, c.r, c.r, 255)).collect();
            (
                rgba_de(w, h, &v),
                vec![Mode::Gray, Mode::GrayAlpha, Mode::Indexed],
            )
        })
        .boxed();
    let rgba_any = image_of_mode(Mode::Rgba)
        .prop_map(|i| (i, vec![Mode::Indexed]))
        .boxed();
    let ga_any = image_of_mode(Mode::GrayAlpha)
        .prop_map(|i| (i, vec![Mode::Rgba, Mode::Indexed]))
        .boxed();
    let ga_opaque = colors_2()
        .prop_map(|((w, h), v)| {
            let p: Vec<(u8, u8)> = v[..(w * h) as usize].iter().map(|c| (c.g, 255)).collect();
            (ga_img(w, h, &p), vec![Mode::Gray])
        })
        .boxed();
    let gray_2 = image_of_mode(Mode::Gray)
        .prop_map(|i| (i, vec![Mode::Rgba, Mode::GrayAlpha, Mode::Indexed]))
        .boxed();
    let indexed_any = image_of_mode(Mode::Indexed)
        .prop_map(|i| (i, vec![Mode::Rgba]))
        .boxed();
    let indexed_to_ga = indexed_gray(false)
        .prop_map(|i| (i, vec![Mode::GrayAlpha, Mode::Rgba]))
        .boxed();
    let indexed_to_gray = indexed_gray(true)
        .prop_map(|i| (i, vec![Mode::Gray, Mode::GrayAlpha, Mode::Rgba]))
        .boxed();
    prop_oneof![
        rgba_gray,
        rgba_gray_opaque,
        rgba_any,
        ga_any,
        ga_opaque,
        gray_2,
        indexed_any,
        indexed_to_ga,
        indexed_to_gray,
    ]
}

// Propiedad 2: el informe y los valores contra el oráculo.

fn prop_oracle(
    img: &Image,
    target: Mode,
    allowed: Allowed,
    other_background: [u8; 3],
) -> Result<(), TestCaseError> {
    let antes = img.clone();
    let result = convert_image(img, target, allowed);
    prop_assert_eq!(img, &antes, "la imagen de origen cambió");

    if img.mode() == target {
        // No es una conversión: copia idéntica, sin informe, con cualquier permiso.
        let conversion = result.map_err(|e| TestCaseError::fail(format!("{e:?}")))?;
        prop_assert_eq!(&conversion.image, img);
        prop_assert_eq!(conversion.report, ConversionReport::default());
        return Ok(());
    }

    let expected_2 = oracle(img, target, allowed.drop_alpha);
    let report = report_2(expected_2.color_lost, expected_2.alpha_lost);
    let missing_luma = expected_2.color_lost > 0 && !allowed.luma;
    let missing_alpha = expected_2.alpha_lost > 0 && allowed.drop_alpha.is_none();

    if missing_luma || missing_alpha {
        prop_assert_eq!(
            result.err(),
            Some(ConvertError::LossNotAllowed { report }),
            "{:?} -> {:?} con {:?}",
            img.mode(),
            target,
            allowed
        );
        return Ok(());
    }

    let conversion = result.map_err(|e| {
        TestCaseError::fail(format!(
            "{:?} -> {target:?} con {allowed:?} falló: {e:?}",
            img.mode()
        ))
    })?;
    prop_assert_eq!(conversion.report, report);
    prop_assert_eq!(conversion.image.mode(), target);
    prop_assert_eq!(
        (conversion.image.width(), conversion.image.height()),
        (img.width(), img.height())
    );
    prop_assert_eq!(values(&conversion.image), expected_2.values.clone());

    // Un permiso sobrante no cambia el resultado: `luma` con `color_lost = 0` ya es el
    // caso del propio `allowed` si tenía `luma`, y un fondo distinto no importa si no se
    // pierde alfa.
    let mut variants = vec![Allowed {
        luma: true,
        drop_alpha: allowed.drop_alpha,
    }];
    if expected_2.alpha_lost == 0 {
        variants.push(Allowed {
            luma: allowed.luma,
            drop_alpha: Some(other_background),
        });
        variants.push(Allowed {
            luma: true,
            drop_alpha: Some(other_background),
        });
    }
    for variant in variants {
        let other = convert_image(img, target, variant)
            .map_err(|e| TestCaseError::fail(format!("con {variant:?} falló: {e:?}")))?;
        prop_assert_eq!(&other.image, &conversion.image, "con {:?}", variant);
        prop_assert_eq!(other.report, conversion.report);
    }
    Ok(())
}

// Propiedad 3: composición a través de `Gray`.

fn prop_composition(r: u8, g: u8, b: u8, a: u8, bg: [u8; 3]) -> Result<(), TestCaseError> {
    let source = rgba_img(1, 1, &[(r, g, b, a)]);
    let permission = Allowed {
        luma: true,
        drop_alpha: Some(bg),
    };
    let conversion = convert_image(&source, Mode::Gray, permission)
        .map_err(|e| TestCaseError::fail(format!("{e:?}")))?;
    let gray_2 = grays(&conversion.image)[0];

    let c = luma_naive(r, g, b);
    let k = luma_naive(bg[0], bg[1], bg[2]);
    let expected_2 = composite_naive(c, k, i128::from(a));
    prop_assert_eq!(i128::from(gray_2), expected_2);
    if a == 255 {
        prop_assert_eq!(i128::from(gray_2), c);
    }
    if a == 0 {
        prop_assert_eq!(i128::from(gray_2), k);
    }

    if a == 255 {
        prop_assert_eq!(gray_2, luma(r, g, b));
    }
    if a == 0 {
        prop_assert_eq!(gray_2, luma(bg[0], bg[1], bg[2]));
    }
    // El informe mide el color de origen solo si el píxel se ve, y el alfa si no es opaco.
    let color_lost = u64::from(a > 0 && !(r == g && g == b));
    let alpha_lost = u64::from(a < 255);
    prop_assert_eq!(conversion.report, report_2(color_lost, alpha_lost));
    Ok(())
}

#[test]
fn composition_exhaustive_in_alpha() {
    for a in 0..=255u8 {
        for c in CHANNELS {
            for k in CHANNELS {
                let source = ga_img(1, 1, &[(c, a)]);
                let img = ok(
                    &source,
                    Mode::Gray,
                    Allowed {
                        luma: false,
                        drop_alpha: Some([k, k, k]),
                    },
                    0,
                    u64::from(a < 255),
                );
                let expected_2 = if a == 0 {
                    i128::from(k)
                } else {
                    composite_naive(i128::from(c), i128::from(k), i128::from(a))
                };
                assert_eq!(
                    i128::from(grays(&img)[0]),
                    expected_2,
                    "c = {c}, k = {k}, a = {a}"
                );
            }
        }
    }
}

// Propiedad 4: paleta determinista y límite de 256.

/// Imagen Rgba, GrayAlpha o Gray de hasta 24 x 24 sacada de un conjunto de colores de hasta
/// 600 (para que haya imágenes con más y con menos de 256 colores distintos).
fn image_large() -> impl Strategy<Value = Image> {
    let uniform = || {
        prop::collection::vec(
            (any::<u8>(), any::<u8>(), any::<u8>(), any::<u8>())
                .prop_map(|(r, g, b, a)| rgba(r, g, b, a)),
            1..=600,
        )
    };
    let set = prop_oneof![
        uniform(),
        prop::collection::vec(color(), 1..=600),
        prop::collection::vec(color(), 1..=8),
    ];
    (
        1u32..=24,
        1u32..=24,
        set,
        prop::collection::vec(any::<usize>(), 576),
        prop::sample::select(vec![Mode::Rgba, Mode::GrayAlpha, Mode::Gray]),
    )
        .prop_map(|(w, h, set, chooses, mode)| {
            let colors_2: Vec<Rgba> = chooses[..(w * h) as usize]
                .iter()
                .map(|&e| set[e % set.len()])
                .collect();
            match mode {
                Mode::Rgba => image_of_colors(w, h, &colors_2),
                Mode::GrayAlpha => {
                    let p: Vec<(u8, u8)> = colors_2.iter().map(|c| (c.r, c.a)).collect();
                    ga_img(w, h, &p)
                }
                _ => {
                    let p: Vec<u8> = colors_2.iter().map(|c| c.g).collect();
                    gray_img(w, h, &p)
                }
            }
        })
}

fn prop_palette(img: &Image) -> Result<(), TestCaseError> {
    // Oráculo: búsqueda lineal sobre los colores resueltos y canónicos.
    let canon: Vec<Rgba> = resolved(img)
        .into_iter()
        .map(|c| if c.a == 0 { transparent() } else { c })
        .collect();
    let there_is_transparent = canon.iter().any(|c| c.a == 0);
    let mut palette_2: Vec<Rgba> = Vec::new();
    if there_is_transparent {
        palette_2.push(transparent());
    }
    for &c in &canon {
        if c.a > 0 && !palette_2.contains(&c) {
            palette_2.push(c);
        }
    }

    let antes = img.clone();
    let result = convert_image(img, Mode::Indexed, Allowed::NOTHING);
    prop_assert_eq!(img, &antes, "la imagen de origen cambió");

    if palette_2.len() > 256 {
        let error = result.err();
        prop_assert_eq!(error, Some(ConvertError::TooManyColors { at_least: 257 }));
        return Ok(());
    }

    let conversion = result
        .map_err(|e| TestCaseError::fail(format!("{} colores y falló: {e:?}", palette_2.len())))?;
    prop_assert_eq!(conversion.report, ConversionReport::default());
    let target = &conversion.image;
    prop_assert_eq!(target.mode(), Mode::Indexed);
    prop_assert_eq!(palette_of(target), palette_2.clone());
    prop_assert_eq!(
        index_transparent(target),
        if there_is_transparent { Some(0) } else { None }
    );
    // Cada píxel apunta a la posición de su color en la paleta del oráculo.
    let expected_2_2: Vec<u8> = canon
        .iter()
        .map(|c| palette_2.iter().position(|p| p == c).unwrap() as u8)
        .collect();
    prop_assert_eq!(indices(target), expected_2_2);
    // Y todo píxel resuelve al color de origen (canónico).
    prop_assert_eq!(resolved(target), canon);

    // Convertir dos veces la misma imagen da paletas (e imágenes) idénticas.
    let other = convert_image(img, Mode::Indexed, Allowed::NOTHING)
        .map_err(|e| TestCaseError::fail(format!("{e:?}")))?;
    prop_assert_eq!(&other.image, target);
    Ok(())
}

proptest! {
    // Propiedad 1: ida y vuelta de lo exacto, en los cuatro modos.
    #[test]
    fn round_trip_of_the_exact((source, targets) in case_exact()) {
        for target in targets {
            round_trip(&source, target)?;
        }
    }

    // Propiedad 1, explícita: Gray vuelve exactamente igual por los otros tres modos.
    #[test]
    fn gray_comes_back_identical_through_rgba_grayalpha_and_indexed(source in image_of_mode(Mode::Gray)) {
        for intermediate in [Mode::Rgba, Mode::GrayAlpha, Mode::Indexed] {
            let outward = convert_image(&source, intermediate, Allowed::NOTHING)
                .map_err(|e| TestCaseError::fail(format!("{e:?}")))?;
            prop_assert_eq!(outward.report, ConversionReport::default());
            let ret = convert_image(&outward.image, Mode::Gray, Allowed::NOTHING)
                .map_err(|e| TestCaseError::fail(format!("{e:?}")))?;
            prop_assert_eq!(ret.report, ConversionReport::default());
            prop_assert_eq!(&ret.image, &source, "por {:?}", intermediate);
        }
    }

    // Propiedad 2: el informe lista exactamente lo perdido (con permisos de más, de menos
    // y a medias).
    #[test]
    fn the_report_matches_the_oracle(
        img in image_any(),
        target in mode_2(),
        allowed in permissions(),
        other_background in background(),
    ) {
        prop_oracle(&img, target, allowed, other_background)?;
    }

    // Propiedad 2, con todos los permisos: siempre `Ok` y con los valores del oráculo.
    #[test]
    fn with_all_permissions_it_always_matches(
        img in image_any(),
        target in mode_2(),
        bg in background(),
        other_background in background(),
    ) {
        let all = Allowed { luma: true, drop_alpha: Some(bg) };
        prop_oracle(&img, target, all, other_background)?;
        if img.mode() != target {
            let conversion = convert_image(&img, target, all)
                .map_err(|e| TestCaseError::fail(format!("{e:?}")))?;
            let expected_2 = oracle(&img, target, Some(bg));
            prop_assert_eq!(values(&conversion.image), expected_2.values);
            prop_assert_eq!(
                conversion.report,
                report_2(expected_2.color_lost, expected_2.alpha_lost)
            );
        }
    }

    // Propiedad 3: luminancia.
    #[test]
    fn luma_of_a_gray_is_the_gray(v in any::<u8>()) {
        prop_assert_eq!(luma(v, v, v), v);
    }

    #[test]
    fn luma_is_close_to_its_fraction_and_to_rec_709(r in canal(), g in canal(), b in canal()) {
        let l = i64::from(luma(r, g, b));
        let (r, g, b) = (i64::from(r), i64::from(g), i64::from(b));
        prop_assert!((256 * l - (54 * r + 183 * g + 19 * b)).abs() <= 128);
        prop_assert!((10000 * l - (2126 * r + 7152 * g + 722 * b)).abs() <= 10200);
        prop_assert_eq!(i128::from(l), luma_naive(r as u8, g as u8, b as u8));
    }

    #[test]
    fn luma_is_monotonic_in_each_channel(
        r in canal(), g in canal(), b in canal(),
        r2 in canal(), g2 in canal(), b2 in canal(),
    ) {
        let (r_height, r_under) = (r.max(r2), r.min(r2));
        let (g_height, g_under) = (g.max(g2), g.min(g2));
        let (b_height, b_under) = (b.max(b2), b.min(b2));
        prop_assert!(luma(r_under, g, b) <= luma(r_height, g, b));
        prop_assert!(luma(r, g_under, b) <= luma(r, g_height, b));
        prop_assert!(luma(r, g, b_under) <= luma(r, g, b_height));
    }

    // Propiedad 3: composición, a través de `Gray` con un fondo cualquiera.
    #[test]
    fn composition_with_rounding_to_nearest(
        r in canal(), g in canal(), b in canal(),
        a in prop_oneof![any::<u8>(), Just(0u8), Just(255u8), Just(128u8)],
        bg in background(),
    ) {
        prop_composition(r, g, b, a, bg)?;
    }

    // Propiedad 4: paleta determinista y límite de 256.
    #[test]
    fn deterministic_palette_and_limit_of_256(img in image_large()) {
        prop_palette(&img)?;
    }
}

// Las propiedades 4 con imágenes pequeñas de cualquier modo origen (sin `Indexed`) y pocos
// colores, para que el caso `<= 256` sea el habitual.
proptest! {
    #[test]
    fn deterministic_palette_with_small_images(img in image_any()) {
        if img.mode() != Mode::Indexed {
            prop_palette(&img)?;
        }
    }
}

// Fila 21: el RGB de un píxel con `a = 0` no cuenta como color perdido en ningún destino.

#[test]
fn row_21_transparents_with_different_rgb_do_not_count_as_a_color() {
    // Posiciones de los píxeles que no son transparentes; el resto, 300 transparentes con RGB
    // distinto, en orden de `i`.
    #[derive(Clone, Copy)]
    enum Type {
        Gray(u8),
        Semi,
    }
    let special = [
        (3, Type::Gray(10)),
        (50, Type::Gray(90)),
        (120, Type::Semi),
        (121, Type::Gray(170)),
        (200, Type::Semi),
        (305, Type::Gray(250)),
    ];
    let mut colors_2 = Vec::new();
    let mut types = Vec::new();
    let mut i = 0usize;
    for position in 0..306 {
        match special.iter().find(|(p, _)| *p == position) {
            Some(&(_, Type::Gray(v))) => {
                colors_2.push(opaque(v, v, v));
                types.push(Some(Type::Gray(v)));
            }
            Some(&(_, Type::Semi)) => {
                colors_2.push(rgba(50, 50, 50, 128));
                types.push(Some(Type::Semi));
            }
            None => {
                colors_2.push(rgba(
                    (i % 256) as u8,
                    (i / 256) as u8,
                    ((7 * i) % 256) as u8,
                    0,
                ));
                types.push(None);
                i += 1;
            }
        }
    }
    assert_eq!(i, 300);
    let source = image_of_colors(306, 1, &colors_2);

    // GrayAlpha sin ningún permiso: nada se pierde, y los transparentes salen canónicos.
    let img = ok(&source, Mode::GrayAlpha, N, 0, 0);
    let expected_2_2: Vec<(u8, u8)> = types
        .iter()
        .map(|t| match t {
            None => (0, 0),
            Some(Type::Gray(v)) => (*v, 255),
            Some(Type::Semi) => (50, 128),
        })
        .collect();
    assert_eq!(pares(&img), expected_2_2);

    // Gray con el fondo blanco y con el negro: 300 + 2 píxeles pierden alfa, ninguno color.
    // Los transparentes dan el fondo; 152 y 25 son `comp(50, background, 128)` hechos a mano.
    for (permission, background, semi) in [(A, 255u8, 152u8), (A0, 0u8, 25u8)] {
        let img = ok(&source, Mode::Gray, permission, 0, 302);
        let expected_2_2: Vec<u8> = types
            .iter()
            .map(|t| match t {
                None => background,
                Some(Type::Gray(v)) => *v,
                Some(Type::Semi) => semi,
            })
            .collect();
        assert_eq!(grays(&img), expected_2_2, "fondo {background}");
    }
    // Sin `drop_alpha` hace falta el permiso de alfa, pero nunca el de `luma`.
    rejects(&source, Mode::Gray, N, 0, 302);
    rejects(&source, Mode::Gray, L, 0, 302);

    // Control: un único píxel visible con color sí cuenta, y solo él (1, nunca 301).
    let mut with_red = colors_2.clone();
    with_red.push(opaque(200, 0, 0));
    let source = image_of_colors(307, 1, &with_red);
    rejects(&source, Mode::GrayAlpha, N, 1, 0);
    rejects(&source, Mode::Gray, A, 1, 302);
    let img = ok(&source, Mode::GrayAlpha, L, 1, 0);
    assert_eq!(pares(&img).last(), Some(&(luma(200, 0, 0), 255)));
}

// Propiedad 7: umbral de 256 colores.

proptest! {
    #[test]
    fn threshold_of_256_colors(
        n in prop::sample::select(vec![255usize, 256, 257]),
        base in 0u32..(1 << 24),
        step in any::<u32>(),
        transparent_2 in prop_oneof![
            Just(None),
            (1usize..=40, any::<bool>()).prop_map(Some),
        ],
        order in prop::collection::vec(any::<u32>(), 300),
    ) {
        // `n` opacos distintos: un paso impar recorre los 2^24 colores sin repetir ninguno.
        let step = u64::from((step | 1) & 0x00ff_ffff);
        let mut colors_2: Vec<Rgba> = (0..n as u64)
            .map(|i| {
                let v = (u64::from(base) + i * step) % (1 << 24);
                opaque((v >> 16) as u8, (v >> 8) as u8, v as u8)
            })
            .collect();
        // Transparentes con el mismo RGB o con RGB distinto.
        if let Some((k, different)) = transparent_2 {
            colors_2.extend((0..k).map(|i| {
                if different {
                    rgba((i * 37) as u8, (i * 11 + 1) as u8, (255 - i) as u8, 0)
                } else {
                    transparent()
                }
            }));
        }
        // Orden aleatorio, estable: el transparente puede ir en cualquier sitio.
        let mut positions: Vec<usize> = (0..colors_2.len()).collect();
        positions.sort_by_key(|&p| order[p]);
        let colors_2: Vec<Rgba> = positions.iter().map(|&p| colors_2[p]).collect();
        let source = image_of_colors(colors_2.len() as u32, 1, &colors_2);

        let there_is_transparent = transparent_2.is_some();
        let entries = n + usize::from(there_is_transparent);
        let result = convert_image(&source, Mode::Indexed, Allowed::NOTHING);
        if entries <= 256 {
            let conversion = result
                .map_err(|e| TestCaseError::fail(format!("{entries} entradas y falló: {e:?}")))?;
            prop_assert_eq!(conversion.report, ConversionReport::default());
            prop_assert_eq!(conversion.image.palette().unwrap().len(), entries);
            prop_assert_eq!(
                index_transparent(&conversion.image),
                there_is_transparent.then_some(0)
            );
            let canonical: Vec<Rgba> = colors_2
                .iter()
                .map(|c| if c.a == 0 { transparent() } else { *c })
                .collect();
            prop_assert_eq!(resolved(&conversion.image), canonical);
        } else {
            prop_assert_eq!(
                result.err(),
                Some(ConvertError::TooManyColors { at_least: 257 }),
                "{} entradas", entries
            );
        }
    }
}
