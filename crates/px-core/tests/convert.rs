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
fn assert_dato_copiable<T: Clone + Copy + Debug + PartialEq + Eq>() {}
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

const TODOS: [Mode; 4] = [Mode::Rgba, Mode::GrayAlpha, Mode::Gray, Mode::Indexed];

// Construcción y lectura de imágenes.

fn rgba(r: u8, g: u8, b: u8, a: u8) -> Rgba {
    Rgba::new(r, g, b, a)
}

fn opaco(r: u8, g: u8, b: u8) -> Rgba {
    Rgba::opaque(r, g, b)
}

fn transparente() -> Rgba {
    Rgba::new(0, 0, 0, 0)
}

fn informe(color_lost: u64, alpha_lost: u64) -> ConversionReport {
    ConversionReport {
        color_lost,
        alpha_lost,
    }
}

/// Imagen de un modo sin paleta con los píxeles dados, fila a fila.
fn imagen(width: u32, height: u32, mode: Mode, pixels: &[Pixel]) -> Image {
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
    imagen(width, height, Mode::Rgba, &v)
}

fn gris_img(width: u32, height: u32, pixels: &[u8]) -> Image {
    let v: Vec<Pixel> = pixels.iter().map(|&g| Pixel::Gray(g)).collect();
    imagen(width, height, Mode::Gray, &v)
}

fn ga_img(width: u32, height: u32, pixels: &[(u8, u8)]) -> Image {
    let v: Vec<Pixel> = pixels
        .iter()
        .map(|&(gray, alpha)| Pixel::GrayAlpha { gray, alpha })
        .collect();
    imagen(width, height, Mode::GrayAlpha, &v)
}

/// Imagen indexada con la paleta, el índice transparente y los índices dados.
fn indexed_img(
    width: u32,
    height: u32,
    colores: &[Rgba],
    transparente: Option<u8>,
    indices: &[u8],
) -> Image {
    assert_eq!(indices.len(), (width * height) as usize);
    let mut paleta = Palette::from_colors(colores.iter().copied()).unwrap();
    paleta.set_transparent_index(transparente).unwrap();
    let mut img = Image::new_indexed(width, height, paleta).unwrap();
    for (i, &p) in indices.iter().enumerate() {
        img.set_pixel(i as u32 % width, i as u32 / width, Pixel::Indexed(p))
            .unwrap();
    }
    img
}

/// Todos los píxeles de la imagen, fila a fila.
fn pixeles(img: &Image) -> Vec<Pixel> {
    (0..img.height())
        .flat_map(|y| (0..img.width()).map(move |x| (x, y)))
        .map(|(x, y)| img.get_pixel(x, y).unwrap())
        .collect()
}

fn rgbas(img: &Image) -> Vec<Rgba> {
    assert_eq!(img.mode(), Mode::Rgba);
    pixeles(img)
        .into_iter()
        .map(|p| match p {
            Pixel::Rgba(c) => c,
            otro => panic!("píxel {otro:?} en una imagen Rgba"),
        })
        .collect()
}

fn grises(img: &Image) -> Vec<u8> {
    assert_eq!(img.mode(), Mode::Gray);
    pixeles(img)
        .into_iter()
        .map(|p| match p {
            Pixel::Gray(g) => g,
            otro => panic!("píxel {otro:?} en una imagen Gray"),
        })
        .collect()
}

fn pares(img: &Image) -> Vec<(u8, u8)> {
    assert_eq!(img.mode(), Mode::GrayAlpha);
    pixeles(img)
        .into_iter()
        .map(|p| match p {
            Pixel::GrayAlpha { gray, alpha } => (gray, alpha),
            otro => panic!("píxel {otro:?} en una imagen GrayAlpha"),
        })
        .collect()
}

fn indices(img: &Image) -> Vec<u8> {
    assert_eq!(img.mode(), Mode::Indexed);
    pixeles(img)
        .into_iter()
        .map(|p| match p {
            Pixel::Indexed(i) => i,
            otro => panic!("píxel {otro:?} en una imagen Indexed"),
        })
        .collect()
}

/// Entradas de la paleta tal como están guardadas.
fn paleta_de(img: &Image) -> Vec<Rgba> {
    img.palette().expect("imagen indexada").colors().to_vec()
}

fn indice_transparente(img: &Image) -> Option<u8> {
    img.palette().expect("imagen indexada").transparent_index()
}

/// Color resuelto de cada píxel de cualquier imagen (los grises con alfa 255, el alfa
/// directo, y la paleta resuelta), sin canonizar.
fn resueltos(img: &Image) -> Vec<Rgba> {
    pixeles(img)
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

fn convierte(img: &Image, target: Mode, allowed: Allowed) -> Result<Conversion, ConvertError> {
    let antes = img.clone();
    let resultado = convert_image(img, target, allowed);
    assert_eq!(img, &antes, "convert_image tocó la imagen de origen");
    resultado
}

/// Convierte y espera `Ok` con el informe `color/alfa`; devuelve la imagen convertida.
fn ok(img: &Image, target: Mode, allowed: Allowed, color: u64, alfa: u64) -> Image {
    let conversion = convierte(img, target, allowed)
        .unwrap_or_else(|e| panic!("{target:?} con {allowed:?} falló: {e:?}"));
    assert_eq!(conversion.report, informe(color, alfa), "informe");
    assert_eq!(conversion.image.mode(), target, "modo del resultado");
    assert_eq!(conversion.image.width(), img.width());
    assert_eq!(conversion.image.height(), img.height());
    conversion.image
}

/// Convierte y espera `Err(LossNotAllowed { color/alfa })`.
fn rechaza(img: &Image, target: Mode, allowed: Allowed, color: u64, alfa: u64) {
    let error = convierte(img, target, allowed).err();
    assert_eq!(
        error,
        Some(ConvertError::LossNotAllowed {
            report: informe(color, alfa)
        }),
        "{target:?} con {allowed:?}"
    );
}

/// `c(i) = (i mod 256, i / 256, 0, 255)` de la fila 9.
fn c(i: usize) -> Rgba {
    opaco((i % 256) as u8, (i / 256) as u8, 0)
}

fn contiene_mas_de_256(e: &ConvertError) -> bool {
    e.to_string().contains("más de 256 colores")
}

// Tipos nuevos de la API.

#[test]
fn traits_de_los_tipos_nuevos() {
    assert_dato_copiable::<Allowed>();
    assert_dato_copiable::<ConversionReport>();
    assert_default::<ConversionReport>();
    assert_error_traits::<ConvertError>();

    // `ConversionReport::default()` es todo a cero.
    assert_eq!(ConversionReport::default(), informe(0, 0));

    // `Allowed::NOTHING` es una constante, y no pide nada.
    const NADA: Allowed = Allowed::NOTHING;
    assert_eq!(
        NADA,
        Allowed {
            luma: false,
            drop_alpha: None
        }
    );
    assert_ne!(NADA, L);
    assert_ne!(NADA, A);
    assert_ne!(A, A0);
    let copia = A;
    assert_eq!(copia, A);
}

#[test]
fn los_errores_tienen_mensaje() {
    let errores = [
        ConvertError::LossNotAllowed {
            report: informe(2, 1),
        },
        ConvertError::TooManyColors { at_least: 257 },
        ConvertError::TooLarge {
            width: 16385,
            height: 16384,
        },
    ];
    for e in &errores {
        assert!(!e.to_string().is_empty(), "{e:?} sin mensaje");
    }
    assert!(contiene_mas_de_256(&errores[1]));
    assert_ne!(errores[0], errores[1]);
    assert_ne!(
        ConvertError::TooManyColors { at_least: 257 },
        ConvertError::TooManyColors { at_least: 300 }
    );
    assert_ne!(
        ConvertError::LossNotAllowed {
            report: informe(1, 0)
        },
        ConvertError::LossNotAllowed {
            report: informe(0, 1)
        }
    );
}

// Una prueba por fila de la tabla de casos límite.

#[test]
fn fila_01_gray_a_los_otros_tres_modos() {
    let origen = gris_img(3, 1, &[0, 127, 255]);

    let rgba_ = ok(&origen, Mode::Rgba, N, 0, 0);
    assert_eq!(
        rgbas(&rgba_),
        [opaco(0, 0, 0), opaco(127, 127, 127), opaco(255, 255, 255)]
    );

    let ga = ok(&origen, Mode::GrayAlpha, N, 0, 0);
    assert_eq!(pares(&ga), [(0, 255), (127, 255), (255, 255)]);

    let indexada = ok(&origen, Mode::Indexed, N, 0, 0);
    assert_eq!(
        paleta_de(&indexada),
        [opaco(0, 0, 0), opaco(127, 127, 127), opaco(255, 255, 255)]
    );
    assert_eq!(indice_transparente(&indexada), None);
    assert_eq!(indices(&indexada), [0, 1, 2]);
}

#[test]
fn fila_02_grayalpha_a_rgba_canoniza_el_transparente() {
    let origen = ga_img(3, 1, &[(10, 0), (10, 255), (200, 128)]);
    let img = ok(&origen, Mode::Rgba, N, 0, 0);
    assert_eq!(
        rgbas(&img),
        [
            rgba(0, 0, 0, 0),
            opaco(10, 10, 10),
            rgba(200, 200, 200, 128)
        ]
    );
}

#[test]
fn fila_03_grayalpha_a_gray() {
    let origen = ga_img(3, 1, &[(10, 0), (10, 255), (200, 128)]);

    rechaza(&origen, Mode::Gray, N, 0, 2);
    let con_fondo = ok(&origen, Mode::Gray, A, 0, 2);
    assert_eq!(grises(&con_fondo), [255, 10, 227]);
    // Falta el alfa: con `luma` solo no basta.
    rechaza(&origen, Mode::Gray, L, 0, 2);
}

#[test]
fn fila_04_rgba_primarios_a_gray() {
    let origen = rgba_img(
        3,
        1,
        &[(255, 0, 0, 255), (0, 255, 0, 255), (0, 0, 255, 255)],
    );

    rechaza(&origen, Mode::Gray, N, 3, 0);
    let img = ok(&origen, Mode::Gray, L, 3, 0);
    assert_eq!(grises(&img), [54, 182, 19]);
}

#[test]
fn fila_05_rojo_semitransparente() {
    let origen = rgba_img(1, 1, &[(255, 0, 0, 128)]);

    let ga = ok(&origen, Mode::GrayAlpha, L, 1, 0);
    assert_eq!(pares(&ga), [(54, 128)]);

    rechaza(&origen, Mode::Gray, L, 1, 1);

    let gris = ok(&origen, Mode::Gray, L_Y_A, 1, 1);
    assert_eq!(grises(&gris), [154]);
}

#[test]
fn fila_06_el_rgb_de_un_pixel_transparente_no_es_contenido() {
    let origen = rgba_img(1, 1, &[(10, 20, 30, 0)]);

    // Transparente canónico, no (19, 0).
    let ga = ok(&origen, Mode::GrayAlpha, N, 0, 0);
    assert_eq!(pares(&ga), [(0, 0)]);

    // Sin `luma`: el color de un `a = 0` no cuenta en ningún destino.
    let gris = ok(&origen, Mode::Gray, A, 0, 1);
    assert_eq!(grises(&gris), [255]);

    let indexada = ok(&origen, Mode::Indexed, N, 0, 0);
    assert_eq!(paleta_de(&indexada), [transparente()]);
    assert_eq!(indice_transparente(&indexada), Some(0));
    assert_eq!(indices(&indexada), [0]);
}

#[test]
fn fila_07_el_transparente_va_al_indice_0_aunque_aparezca_despues() {
    let origen = rgba_img(
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

    let indexada = ok(&origen, Mode::Indexed, N, 0, 0);
    assert_eq!(
        paleta_de(&indexada),
        [transparente(), opaco(5, 5, 5), opaco(7, 7, 7)]
    );
    assert_eq!(indice_transparente(&indexada), Some(0));
    assert_eq!(indices(&indexada), [1, 0, 1, 0, 2]);

    let vuelta = ok(&indexada, Mode::Rgba, N, 0, 0);
    assert_eq!(
        rgbas(&vuelta),
        [
            opaco(5, 5, 5),
            transparente(),
            opaco(5, 5, 5),
            transparente(),
            opaco(7, 7, 7)
        ]
    );
}

#[test]
fn fila_08_el_semitransparente_es_un_color_mas_y_la_vuelta_es_exacta() {
    let origen = rgba_img(
        3,
        1,
        &[(255, 0, 0, 128), (255, 0, 0, 255), (255, 0, 0, 128)],
    );

    let indexada = ok(&origen, Mode::Indexed, N, 0, 0);
    assert_eq!(
        paleta_de(&indexada),
        [rgba(255, 0, 0, 128), opaco(255, 0, 0)]
    );
    assert_eq!(indices(&indexada), [0, 1, 0]);
    assert_eq!(indice_transparente(&indexada), None);

    let vuelta = ok(&indexada, Mode::Rgba, N, 0, 0);
    assert_eq!(vuelta, origen);
}

#[test]
fn fila_09_el_limite_de_256_colores() {
    // (a) c(0)..c(255): 256 colores, en orden de columna.
    let a: Vec<Rgba> = (0..256).map(c).collect();
    let origen = imagen_de_colores(256, 1, &a);
    let img = ok(&origen, Mode::Indexed, N, 0, 0);
    assert_eq!(paleta_de(&img), a);
    assert_eq!(indice_transparente(&img), None);
    assert_eq!(indices(&img), (0..=255u8).collect::<Vec<u8>>());

    // (b) 255 opacos y un transparente al final: el transparente va al índice 0.
    let mut b: Vec<Rgba> = (0..255).map(c).collect();
    b.push(transparente());
    let origen = imagen_de_colores(256, 1, &b);
    let img = ok(&origen, Mode::Indexed, N, 0, 0);
    assert_eq!(img.palette().unwrap().len(), 256);
    let mut esperada = vec![transparente()];
    esperada.extend((0..255).map(c));
    assert_eq!(paleta_de(&img), esperada);
    assert_eq!(indice_transparente(&img), Some(0));
    let mut esperados: Vec<u8> = (1..=255u8).collect();
    esperados.push(0);
    assert_eq!(indices(&img), esperados);

    // (c) 256 opacos más un transparente: 257.
    let mut cc: Vec<Rgba> = (0..256).map(c).collect();
    cc.push(transparente());
    let origen = imagen_de_colores(257, 1, &cc);
    let error = convierte(&origen, Mode::Indexed, N).err();
    assert_eq!(error, Some(ConvertError::TooManyColors { at_least: 257 }));
    assert!(contiene_mas_de_256(&error.unwrap()));

    // (d) c(0)..c(299): más de 256, no es el número exacto.
    let d: Vec<Rgba> = (0..300).map(c).collect();
    let origen = imagen_de_colores(300, 1, &d);
    let error = convierte(&origen, Mode::Indexed, N).err();
    assert_eq!(error, Some(ConvertError::TooManyColors { at_least: 257 }));
    assert!(contiene_mas_de_256(&error.unwrap()));

    // (e) 300 píxeles transparentes con RGB distinto: una sola entrada.
    let e: Vec<Rgba> = (0..300)
        .map(|i| rgba((i % 256) as u8, (i / 256) as u8, 0, 0))
        .collect();
    let origen = imagen_de_colores(300, 1, &e);
    let img = ok(&origen, Mode::Indexed, N, 0, 0);
    assert_eq!(paleta_de(&img), [transparente()]);
    assert_eq!(indice_transparente(&img), Some(0));
    assert_eq!(indices(&img), vec![0u8; 300]);
}

/// Imagen Rgba de `width x height` con esos colores.
fn imagen_de_colores(width: u32, height: u32, colores: &[Rgba]) -> Image {
    let v: Vec<Pixel> = colores.iter().map(|&c| Pixel::Rgba(c)).collect();
    imagen(width, height, Mode::Rgba, &v)
}

#[test]
fn fila_10_indexed_a_rgba_resuelve_la_paleta() {
    let origen = fila_10_origen(&[0, 1, 2]);
    let img = ok(&origen, Mode::Rgba, N, 0, 0);
    assert_eq!(
        rgbas(&img),
        [opaco(10, 10, 10), opaco(200, 0, 0), transparente()]
    );
}

/// Imagen indexada de las filas 10 y 11, con los píxeles dados.
fn fila_10_origen(pixels: &[u8]) -> Image {
    indexed_img(
        3,
        1,
        &[opaco(10, 10, 10), opaco(200, 0, 0), opaco(5, 5, 5)],
        Some(2),
        pixels,
    )
}

#[test]
fn fila_11_indexed_a_gray_mide_los_pixeles_usados() {
    // Las entradas sin usar no cuentan.
    let img = ok(&fila_10_origen(&[0, 0, 0]), Mode::Gray, N, 0, 0);
    assert_eq!(grises(&img), [10, 10, 10]);

    // El píxel transparente es una pérdida de alfa.
    rechaza(&fila_10_origen(&[0, 0, 2]), Mode::Gray, N, 0, 1);

    // Se cuentan píxeles, no entradas.
    rechaza(&fila_10_origen(&[0, 1, 1]), Mode::Gray, N, 2, 0);
}

#[test]
fn fila_12_el_mismo_modo_es_una_copia_identica() {
    let todos_los_permisos = [
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
    let indexada = indexed_img(1, 1, &[opaco(1, 2, 3), rgba(9, 9, 9, 0)], None, &[1]);
    let ga = ga_img(2, 1, &[(9, 0), (4, 77)]);
    let gris = gris_img(2, 1, &[9, 4]);

    for origen in [&rgba_1, &indexada, &ga, &gris] {
        for allowed in todos_los_permisos {
            let img = ok(origen, origen.mode(), allowed, 0, 0);
            assert_eq!(&img, origen, "{:?} con {allowed:?}", origen.mode());
        }
    }
    // Sin canonizar: el (9, 9, 9, 0) sigue ahí.
    assert_eq!(rgbas(&rgba_1), [rgba(9, 9, 9, 0)]);
    assert_eq!(paleta_de(&indexada)[1], rgba(9, 9, 9, 0));
}

#[test]
fn fila_14_target_byte_len() {
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
fn fila_16_gray_y_grayalpha_a_indexed_por_primera_aparicion() {
    let gris = gris_img(4, 1, &[200, 3, 200, 3]);
    let img = ok(&gris, Mode::Indexed, N, 0, 0);
    assert_eq!(paleta_de(&img), [opaco(200, 200, 200), opaco(3, 3, 3)]);
    assert_eq!(indice_transparente(&img), None);
    assert_eq!(indices(&img), [0, 1, 0, 1]);

    let ga = ga_img(4, 1, &[(10, 0), (10, 255), (10, 0), (7, 255)]);
    let img = ok(&ga, Mode::Indexed, N, 0, 0);
    assert_eq!(
        paleta_de(&img),
        [transparente(), opaco(10, 10, 10), opaco(7, 7, 7)]
    );
    assert_eq!(indice_transparente(&img), Some(0));
    assert_eq!(indices(&img), [0, 1, 0, 2]);
}

#[test]
fn fila_17_composicion_con_redondeo_al_mas_cercano() {
    let origen = ga_img(3, 1, &[(200, 0), (200, 128), (200, 255)]);

    let blanco = ok(&origen, Mode::Gray, A, 0, 2);
    assert_eq!(grises(&blanco), [255, 227, 200]);

    let negro = ok(&origen, Mode::Gray, A0, 0, 2);
    assert_eq!(grises(&negro), [0, 100, 200]);
}

#[test]
fn fila_18_luma_a_mano_y_pesos() {
    assert_eq!(luma(255, 0, 0), 54);
    assert_eq!(luma(0, 255, 0), 182);
    assert_eq!(luma(0, 0, 255), 19);

    assert_eq!(LUMA_WEIGHTS, [54, 183, 19]);
    assert_eq!(LUMA_WEIGHTS.iter().sum::<u32>(), 256);
}

/// Prueba grande de extremo a extremo (256 MiB): `TooLarge` sin llegar a leer un píxel.
#[test]
#[ignore = "reserva 256 MiB; se corre a mano con `cargo test -- --ignored`"]
fn gray_de_16385_por_16384_a_rgba_es_demasiado_grande() {
    let origen = Image::new(16385, 16384, Mode::Gray).unwrap();
    let error = convert_image(&origen, Mode::Rgba, Allowed::NOTHING).err();
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
fn luma_de_los_extremos_del_cubo() {
    // Calculadas a mano con `(54 r + 183 g + 19 b + 128) >> 8`.
    let esquinas = [
        ((0, 0, 0), 0),
        ((255, 0, 0), 54),
        ((0, 255, 0), 182),
        ((0, 0, 255), 19),
        ((255, 255, 0), 236),
        ((255, 0, 255), 73),
        ((0, 255, 255), 201),
        ((255, 255, 255), 255),
    ];
    for ((r, g, b), esperado) in esquinas {
        assert_eq!(luma(r, g, b), esperado, "luma({r}, {g}, {b})");
    }
}

#[test]
fn luma_de_un_gris_es_el_gris_para_los_256_valores() {
    for v in 0..=255u8 {
        assert_eq!(luma(v, v, v), v);
    }
}

// Estrategias.

/// Valores por canal que fuerzan grises, alfa 0 y repeticiones.
const CANALES: [u8; 5] = [0, 1, 127, 128, 255];

fn canal() -> impl Strategy<Value = u8> {
    prop_oneof![any::<u8>(), prop::sample::select(CANALES.to_vec())]
}

fn pequeno() -> impl Strategy<Value = u8> {
    prop::sample::select(CANALES.to_vec())
}

fn fondo() -> impl Strategy<Value = [u8; 3]> {
    (canal(), canal(), canal()).prop_map(|(r, g, b)| [r, g, b])
}

/// Colores uniformes, del conjunto pequeño y grises con alfa de un conjunto pequeño.
fn color() -> impl Strategy<Value = Rgba> {
    prop_oneof![
        (any::<u8>(), any::<u8>(), any::<u8>(), any::<u8>())
            .prop_map(|(r, g, b, a)| rgba(r, g, b, a)),
        (pequeno(), pequeno(), pequeno(), pequeno()).prop_map(|(r, g, b, a)| rgba(r, g, b, a)),
        (canal(), pequeno()).prop_map(|(v, a)| rgba(v, v, v, a)),
    ]
}

fn dims() -> impl Strategy<Value = (u32, u32)> {
    (1u32..=6, 1u32..=6)
}

fn modo() -> impl Strategy<Value = Mode> {
    prop::sample::select(TODOS.to_vec())
}

fn permisos() -> impl Strategy<Value = Allowed> {
    (any::<bool>(), prop::option::of(fondo()))
        .prop_map(|(luma, drop_alpha)| Allowed { luma, drop_alpha })
}

/// Imagen Rgba de `w x h` con los primeros `w * h` colores de `v`.
fn rgba_de(w: u32, h: u32, v: &[Rgba]) -> Image {
    imagen_de_colores(w, h, &v[..(w * h) as usize])
}

/// Imagen indexada de `w x h` con esa paleta; el índice transparente y los píxeles se
/// reducen módulo la longitud de la paleta.
fn indexada_de(
    w: u32,
    h: u32,
    colores: &[Rgba],
    transparente: Option<u8>,
    indices: &[u8],
) -> Image {
    let len = colores.len();
    let t = transparente.map(|t| (t as usize % len) as u8);
    let idx: Vec<u8> = indices[..(w * h) as usize]
        .iter()
        .map(|&i| (i as usize % len) as u8)
        .collect();
    indexed_img(w, h, colores, t, &idx)
}

/// Imagen arbitraria de un modo.
fn imagen_de_modo(mode: Mode) -> BoxedStrategy<Image> {
    let colores = || prop::collection::vec(color(), 36);
    match mode {
        Mode::Rgba => (dims(), colores())
            .prop_map(|((w, h), v)| rgba_de(w, h, &v))
            .boxed(),
        Mode::GrayAlpha => (dims(), colores())
            .prop_map(|((w, h), v)| {
                let p: Vec<(u8, u8)> = v[..(w * h) as usize].iter().map(|c| (c.r, c.a)).collect();
                ga_img(w, h, &p)
            })
            .boxed(),
        Mode::Gray => (dims(), colores())
            .prop_map(|((w, h), v)| {
                let p: Vec<u8> = v[..(w * h) as usize].iter().map(|c| c.g).collect();
                gris_img(w, h, &p)
            })
            .boxed(),
        Mode::Indexed => (
            dims(),
            prop::collection::vec(color(), 1..=8),
            prop::option::of(any::<u8>()),
            prop::collection::vec(any::<u8>(), 36),
        )
            .prop_map(|((w, h), paleta, t, idx)| indexada_de(w, h, &paleta, t, &idx))
            .boxed(),
    }
}

fn imagen_cualquiera() -> impl Strategy<Value = Image> {
    modo().prop_flat_map(imagen_de_modo)
}

// Oráculos ingenuos.

/// Luminancia como redondeo (mitad hacia arriba) de la fracción exacta `suma / 256`, con
/// `suma = 54 r + 183 g + 19 b`: `floor(suma / 256 + 1/2) = floor((2 suma + 256) / 512)`.
fn luma_ingenua(r: u8, g: u8, b: u8) -> i128 {
    let suma = 54 * i128::from(r) + 183 * i128::from(g) + 19 * i128::from(b);
    (2 * suma + 256).div_euclid(512)
}

/// El entero `g` en `0..=255` más cercano a la fracción exacta `(c a + k (255 - a)) / 255`,
/// buscado probando los 256 candidatos (sin la forma `(... + 127) / 255`).
fn compuesta_ingenua(c: i128, k: i128, a: i128) -> i128 {
    let numerador = c * a + k * (255 - a);
    (0..=255i128)
        .min_by_key(|&g| (255 * g - numerador).abs())
        .unwrap()
}

/// Lo que el oráculo espera de una conversión.
struct Esperado {
    /// Un valor por píxel: `Pixel::Rgba` con el color resuelto y canónico si el destino es
    /// `Rgba` o `Indexed`; el `Pixel` del modo en los demás casos.
    valores: Vec<Pixel>,
    color_lost: u64,
    alpha_lost: u64,
}

/// Oráculo píxel a píxel de `img` -> `target` (distinto del modo de `img`), con el fondo de
/// `drop_alpha` si lo hay (sin fondo, los valores de gris con `a < 255` no se especifican).
fn oraculo(img: &Image, target: Mode, fondo: Option<[u8; 3]>) -> Esperado {
    assert_ne!(img.mode(), target);
    let mut esperado = Esperado {
        valores: Vec::new(),
        color_lost: 0,
        alpha_lost: 0,
    };
    for c in resueltos(img) {
        let es_gris = c.r == c.g && c.g == c.b;
        let valor = match target {
            Mode::Rgba | Mode::Indexed => Pixel::Rgba(if c.a == 0 { transparente() } else { c }),
            Mode::GrayAlpha => {
                if c.a == 0 {
                    Pixel::GrayAlpha { gray: 0, alpha: 0 }
                } else {
                    if !es_gris {
                        esperado.color_lost += 1;
                    }
                    Pixel::GrayAlpha {
                        gray: luma_ingenua(c.r, c.g, c.b) as u8,
                        alpha: c.a,
                    }
                }
            }
            Mode::Gray => {
                if c.a > 0 && !es_gris {
                    esperado.color_lost += 1;
                }
                if c.a < 255 {
                    esperado.alpha_lost += 1;
                }
                let gris = if c.a == 0 {
                    0
                } else {
                    luma_ingenua(c.r, c.g, c.b)
                };
                let valor = if c.a == 255 {
                    gris
                } else {
                    match fondo {
                        Some([r, g, b]) => {
                            compuesta_ingenua(gris, luma_ingenua(r, g, b), i128::from(c.a))
                        }
                        None => 0,
                    }
                };
                Pixel::Gray(valor as u8)
            }
        };
        esperado.valores.push(valor);
    }
    esperado
}

/// Valor de cada píxel de una imagen como lo compara el oráculo: el color resuelto para
/// `Indexed` y el propio píxel para los demás modos.
fn valores(img: &Image) -> Vec<Pixel> {
    match img.mode() {
        Mode::Indexed => resueltos(img).into_iter().map(Pixel::Rgba).collect(),
        _ => pixeles(img),
    }
}

/// Contenido visible de una imagen, con `a = 0` canonizado y la paleta resuelta.
fn canonicos(img: &Image) -> Vec<Pixel> {
    let canon = |c: Rgba| Pixel::Rgba(if c.a == 0 { transparente() } else { c });
    match img.mode() {
        Mode::Indexed => resueltos(img).into_iter().map(canon).collect(),
        Mode::Rgba => pixeles(img)
            .into_iter()
            .map(|p| match p {
                Pixel::Rgba(c) => canon(c),
                otro => otro,
            })
            .collect(),
        Mode::GrayAlpha => pixeles(img)
            .into_iter()
            .map(|p| match p {
                Pixel::GrayAlpha { alpha: 0, .. } => Pixel::GrayAlpha { gray: 0, alpha: 0 },
                otro => otro,
            })
            .collect(),
        Mode::Gray => pixeles(img),
    }
}

/// Color visible de cada píxel de cualquier imagen: el resuelto, con `a = 0` canonizado.
/// Es comparable entre imágenes de modos distintos.
fn visibles(img: &Image) -> Vec<Rgba> {
    resueltos(img)
        .into_iter()
        .map(|c| if c.a == 0 { transparente() } else { c })
        .collect()
}

// Propiedad 1: ida y vuelta de lo exacto.

/// `a -> b -> a` con `Allowed::NOTHING`, con las dos conversiones exactas.
fn ida_y_vuelta(a: &Image, b: Mode) -> Result<(), TestCaseError> {
    let ida = convert_image(a, b, Allowed::NOTHING).map_err(|e| {
        TestCaseError::fail(format!("ida {:?} -> {b:?} falló: {e:?} ({a:?})", a.mode()))
    })?;
    prop_assert_eq!(ida.report, ConversionReport::default());
    prop_assert_eq!(ida.image.mode(), b);
    prop_assert_eq!(
        (ida.image.width(), ida.image.height()),
        (a.width(), a.height())
    );

    let vuelta = convert_image(&ida.image, a.mode(), Allowed::NOTHING).map_err(|e| {
        TestCaseError::fail(format!(
            "vuelta {b:?} -> {:?} falló: {e:?} ({a:?})",
            a.mode()
        ))
    })?;
    prop_assert_eq!(vuelta.report, ConversionReport::default());
    prop_assert_eq!(vuelta.image.mode(), a.mode());
    prop_assert_eq!(
        (vuelta.image.width(), vuelta.image.height()),
        (a.width(), a.height())
    );
    prop_assert_eq!(canonicos(&vuelta.image), canonicos(a));
    // El contenido visible tampoco cambia en el destino.
    prop_assert_eq!(visibles(&ida.image), visibles(a));
    if a.mode() == Mode::Gray {
        // Gray no tiene nada que canonizar: la imagen vuelve idéntica, bytes incluidos.
        prop_assert_eq!(&vuelta.image, a);
    }
    Ok(())
}

/// Color de una imagen indexada «gris»: `(v, v, v, a)` si `a > 0`; si no, el RGB se deja.
fn gris_o_transparente(c: Rgba) -> Rgba {
    if c.a > 0 { rgba(c.r, c.r, c.r, c.a) } else { c }
}

/// Imagen indexada cuyos píxeles usados son grises (con alfa cualquiera, o todos opacos si
/// `opaco`), con entradas sin usar arbitrarias detrás.
fn indexada_gris(opaco: bool) -> BoxedStrategy<Image> {
    (
        dims(),
        prop::collection::vec(color(), 1..=4),
        prop::collection::vec(color(), 0..=3),
        prop::option::of(any::<u8>()),
        prop::collection::vec(any::<u8>(), 36),
    )
        .prop_map(move |((w, h), usadas, basura, t, idx)| {
            let n_usadas = usadas.len();
            let mut paleta: Vec<Rgba> = usadas
                .iter()
                .map(|&c| {
                    if opaco {
                        rgba(c.r, c.r, c.r, 255)
                    } else {
                        gris_o_transparente(c)
                    }
                })
                .collect();
            paleta.extend(basura.iter().copied());
            // Con todo opaco, el índice transparente solo puede caer en una entrada sin usar.
            let t = match (opaco, t) {
                (true, Some(t)) if !basura.is_empty() => {
                    Some((n_usadas + t as usize % basura.len()) as u8)
                }
                (true, _) => None,
                (false, t) => t.map(|t| (t as usize % paleta.len()) as u8),
            };
            let pixeles: Vec<u8> = idx[..(w * h) as usize]
                .iter()
                .map(|&i| (i as usize % n_usadas) as u8)
                .collect();
            indexed_img(w, h, &paleta, t, &pixeles)
        })
        .boxed()
}

/// Casos exactos de la matriz: una imagen y los destinos a los que pasa sin pérdida.
fn caso_exacto() -> impl Strategy<Value = (Image, Vec<Mode>)> {
    let colores = || (dims(), prop::collection::vec(color(), 36));
    let rgba_gris = colores()
        .prop_map(|((w, h), v)| {
            let v: Vec<Rgba> = v.into_iter().map(gris_o_transparente).collect();
            (rgba_de(w, h, &v), vec![Mode::GrayAlpha, Mode::Indexed])
        })
        .boxed();
    let rgba_gris_opaco = colores()
        .prop_map(|((w, h), v)| {
            let v: Vec<Rgba> = v.into_iter().map(|c| rgba(c.r, c.r, c.r, 255)).collect();
            (
                rgba_de(w, h, &v),
                vec![Mode::Gray, Mode::GrayAlpha, Mode::Indexed],
            )
        })
        .boxed();
    let rgba_cualquiera = imagen_de_modo(Mode::Rgba)
        .prop_map(|i| (i, vec![Mode::Indexed]))
        .boxed();
    let ga_cualquiera = imagen_de_modo(Mode::GrayAlpha)
        .prop_map(|i| (i, vec![Mode::Rgba, Mode::Indexed]))
        .boxed();
    let ga_opaco = colores()
        .prop_map(|((w, h), v)| {
            let p: Vec<(u8, u8)> = v[..(w * h) as usize].iter().map(|c| (c.g, 255)).collect();
            (ga_img(w, h, &p), vec![Mode::Gray])
        })
        .boxed();
    let gris = imagen_de_modo(Mode::Gray)
        .prop_map(|i| (i, vec![Mode::Rgba, Mode::GrayAlpha, Mode::Indexed]))
        .boxed();
    let indexada_cualquiera = imagen_de_modo(Mode::Indexed)
        .prop_map(|i| (i, vec![Mode::Rgba]))
        .boxed();
    let indexada_a_ga = indexada_gris(false)
        .prop_map(|i| (i, vec![Mode::GrayAlpha, Mode::Rgba]))
        .boxed();
    let indexada_a_gray = indexada_gris(true)
        .prop_map(|i| (i, vec![Mode::Gray, Mode::GrayAlpha, Mode::Rgba]))
        .boxed();
    prop_oneof![
        rgba_gris,
        rgba_gris_opaco,
        rgba_cualquiera,
        ga_cualquiera,
        ga_opaco,
        gris,
        indexada_cualquiera,
        indexada_a_ga,
        indexada_a_gray,
    ]
}

// Propiedad 2: el informe y los valores contra el oráculo.

fn prop_oraculo(
    img: &Image,
    target: Mode,
    allowed: Allowed,
    otro_fondo: [u8; 3],
) -> Result<(), TestCaseError> {
    let antes = img.clone();
    let resultado = convert_image(img, target, allowed);
    prop_assert_eq!(img, &antes, "la imagen de origen cambió");

    if img.mode() == target {
        // No es una conversión: copia idéntica, sin informe, con cualquier permiso.
        let conversion = resultado.map_err(|e| TestCaseError::fail(format!("{e:?}")))?;
        prop_assert_eq!(&conversion.image, img);
        prop_assert_eq!(conversion.report, ConversionReport::default());
        return Ok(());
    }

    let esperado = oraculo(img, target, allowed.drop_alpha);
    let report = informe(esperado.color_lost, esperado.alpha_lost);
    let falta_luma = esperado.color_lost > 0 && !allowed.luma;
    let falta_alfa = esperado.alpha_lost > 0 && allowed.drop_alpha.is_none();

    if falta_luma || falta_alfa {
        prop_assert_eq!(
            resultado.err(),
            Some(ConvertError::LossNotAllowed { report }),
            "{:?} -> {:?} con {:?}",
            img.mode(),
            target,
            allowed
        );
        return Ok(());
    }

    let conversion = resultado.map_err(|e| {
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
    prop_assert_eq!(valores(&conversion.image), esperado.valores.clone());

    // Un permiso sobrante no cambia el resultado: `luma` con `color_lost = 0` ya es el
    // caso del propio `allowed` si tenía `luma`, y un fondo distinto no importa si no se
    // pierde alfa.
    let mut variantes = vec![Allowed {
        luma: true,
        drop_alpha: allowed.drop_alpha,
    }];
    if esperado.alpha_lost == 0 {
        variantes.push(Allowed {
            luma: allowed.luma,
            drop_alpha: Some(otro_fondo),
        });
        variantes.push(Allowed {
            luma: true,
            drop_alpha: Some(otro_fondo),
        });
    }
    for variante in variantes {
        let otra = convert_image(img, target, variante)
            .map_err(|e| TestCaseError::fail(format!("con {variante:?} falló: {e:?}")))?;
        prop_assert_eq!(&otra.image, &conversion.image, "con {:?}", variante);
        prop_assert_eq!(otra.report, conversion.report);
    }
    Ok(())
}

// Propiedad 3: composición a través de `Gray`.

fn prop_composicion(r: u8, g: u8, b: u8, a: u8, bg: [u8; 3]) -> Result<(), TestCaseError> {
    let origen = rgba_img(1, 1, &[(r, g, b, a)]);
    let permiso = Allowed {
        luma: true,
        drop_alpha: Some(bg),
    };
    let conversion = convert_image(&origen, Mode::Gray, permiso)
        .map_err(|e| TestCaseError::fail(format!("{e:?}")))?;
    let gris = grises(&conversion.image)[0];

    let c = luma_ingenua(r, g, b);
    let k = luma_ingenua(bg[0], bg[1], bg[2]);
    let esperado = compuesta_ingenua(c, k, i128::from(a));
    prop_assert_eq!(i128::from(gris), esperado);
    if a == 255 {
        prop_assert_eq!(i128::from(gris), c);
    }
    if a == 0 {
        prop_assert_eq!(i128::from(gris), k);
    }

    if a == 255 {
        prop_assert_eq!(gris, luma(r, g, b));
    }
    if a == 0 {
        prop_assert_eq!(gris, luma(bg[0], bg[1], bg[2]));
    }
    // El informe mide el color de origen solo si el píxel se ve, y el alfa si no es opaco.
    let color_lost = u64::from(a > 0 && !(r == g && g == b));
    let alpha_lost = u64::from(a < 255);
    prop_assert_eq!(conversion.report, informe(color_lost, alpha_lost));
    Ok(())
}

#[test]
fn composicion_exhaustiva_en_alfa() {
    for a in 0..=255u8 {
        for c in CANALES {
            for k in CANALES {
                let origen = ga_img(1, 1, &[(c, a)]);
                let img = ok(
                    &origen,
                    Mode::Gray,
                    Allowed {
                        luma: false,
                        drop_alpha: Some([k, k, k]),
                    },
                    0,
                    u64::from(a < 255),
                );
                let esperado = if a == 0 {
                    i128::from(k)
                } else {
                    compuesta_ingenua(i128::from(c), i128::from(k), i128::from(a))
                };
                assert_eq!(
                    i128::from(grises(&img)[0]),
                    esperado,
                    "c = {c}, k = {k}, a = {a}"
                );
            }
        }
    }
}

// Propiedad 4: paleta determinista y límite de 256.

/// Imagen Rgba, GrayAlpha o Gray de hasta 24 x 24 sacada de un conjunto de colores de hasta
/// 600 (para que haya imágenes con más y con menos de 256 colores distintos).
fn imagen_grande() -> impl Strategy<Value = Image> {
    let uniformes = || {
        prop::collection::vec(
            (any::<u8>(), any::<u8>(), any::<u8>(), any::<u8>())
                .prop_map(|(r, g, b, a)| rgba(r, g, b, a)),
            1..=600,
        )
    };
    let conjunto = prop_oneof![
        uniformes(),
        prop::collection::vec(color(), 1..=600),
        prop::collection::vec(color(), 1..=8),
    ];
    (
        1u32..=24,
        1u32..=24,
        conjunto,
        prop::collection::vec(any::<usize>(), 576),
        prop::sample::select(vec![Mode::Rgba, Mode::GrayAlpha, Mode::Gray]),
    )
        .prop_map(|(w, h, conjunto, elige, mode)| {
            let colores: Vec<Rgba> = elige[..(w * h) as usize]
                .iter()
                .map(|&e| conjunto[e % conjunto.len()])
                .collect();
            match mode {
                Mode::Rgba => imagen_de_colores(w, h, &colores),
                Mode::GrayAlpha => {
                    let p: Vec<(u8, u8)> = colores.iter().map(|c| (c.r, c.a)).collect();
                    ga_img(w, h, &p)
                }
                _ => {
                    let p: Vec<u8> = colores.iter().map(|c| c.g).collect();
                    gris_img(w, h, &p)
                }
            }
        })
}

fn prop_paleta(img: &Image) -> Result<(), TestCaseError> {
    // Oráculo: búsqueda lineal sobre los colores resueltos y canónicos.
    let canon: Vec<Rgba> = resueltos(img)
        .into_iter()
        .map(|c| if c.a == 0 { transparente() } else { c })
        .collect();
    let hay_transparente = canon.iter().any(|c| c.a == 0);
    let mut paleta: Vec<Rgba> = Vec::new();
    if hay_transparente {
        paleta.push(transparente());
    }
    for &c in &canon {
        if c.a > 0 && !paleta.contains(&c) {
            paleta.push(c);
        }
    }

    let antes = img.clone();
    let resultado = convert_image(img, Mode::Indexed, Allowed::NOTHING);
    prop_assert_eq!(img, &antes, "la imagen de origen cambió");

    if paleta.len() > 256 {
        let error = resultado.err();
        prop_assert_eq!(error, Some(ConvertError::TooManyColors { at_least: 257 }));
        return Ok(());
    }

    let conversion = resultado
        .map_err(|e| TestCaseError::fail(format!("{} colores y falló: {e:?}", paleta.len())))?;
    prop_assert_eq!(conversion.report, ConversionReport::default());
    let destino = &conversion.image;
    prop_assert_eq!(destino.mode(), Mode::Indexed);
    prop_assert_eq!(paleta_de(destino), paleta.clone());
    prop_assert_eq!(
        indice_transparente(destino),
        if hay_transparente { Some(0) } else { None }
    );
    // Cada píxel apunta a la posición de su color en la paleta del oráculo.
    let esperados: Vec<u8> = canon
        .iter()
        .map(|c| paleta.iter().position(|p| p == c).unwrap() as u8)
        .collect();
    prop_assert_eq!(indices(destino), esperados);
    // Y todo píxel resuelve al color de origen (canónico).
    prop_assert_eq!(resueltos(destino), canon);

    // Convertir dos veces la misma imagen da paletas (e imágenes) idénticas.
    let otra = convert_image(img, Mode::Indexed, Allowed::NOTHING)
        .map_err(|e| TestCaseError::fail(format!("{e:?}")))?;
    prop_assert_eq!(&otra.image, destino);
    Ok(())
}

proptest! {
    // Propiedad 1: ida y vuelta de lo exacto, en los cuatro modos.
    #[test]
    fn ida_y_vuelta_de_lo_exacto((origen, destinos) in caso_exacto()) {
        for destino in destinos {
            ida_y_vuelta(&origen, destino)?;
        }
    }

    // Propiedad 1, explícita: Gray vuelve exactamente igual por los otros tres modos.
    #[test]
    fn gray_vuelve_identico_por_rgba_grayalpha_e_indexed(origen in imagen_de_modo(Mode::Gray)) {
        for intermedio in [Mode::Rgba, Mode::GrayAlpha, Mode::Indexed] {
            let ida = convert_image(&origen, intermedio, Allowed::NOTHING)
                .map_err(|e| TestCaseError::fail(format!("{e:?}")))?;
            prop_assert_eq!(ida.report, ConversionReport::default());
            let vuelta = convert_image(&ida.image, Mode::Gray, Allowed::NOTHING)
                .map_err(|e| TestCaseError::fail(format!("{e:?}")))?;
            prop_assert_eq!(vuelta.report, ConversionReport::default());
            prop_assert_eq!(&vuelta.image, &origen, "por {:?}", intermedio);
        }
    }

    // Propiedad 2: el informe lista exactamente lo perdido (con permisos de más, de menos
    // y a medias).
    #[test]
    fn el_informe_coincide_con_el_oraculo(
        img in imagen_cualquiera(),
        target in modo(),
        allowed in permisos(),
        otro_fondo in fondo(),
    ) {
        prop_oraculo(&img, target, allowed, otro_fondo)?;
    }

    // Propiedad 2, con todos los permisos: siempre `Ok` y con los valores del oráculo.
    #[test]
    fn con_todos_los_permisos_siempre_coincide(
        img in imagen_cualquiera(),
        target in modo(),
        bg in fondo(),
        otro_fondo in fondo(),
    ) {
        let todos = Allowed { luma: true, drop_alpha: Some(bg) };
        prop_oraculo(&img, target, todos, otro_fondo)?;
        if img.mode() != target {
            let conversion = convert_image(&img, target, todos)
                .map_err(|e| TestCaseError::fail(format!("{e:?}")))?;
            let esperado = oraculo(&img, target, Some(bg));
            prop_assert_eq!(valores(&conversion.image), esperado.valores);
            prop_assert_eq!(
                conversion.report,
                informe(esperado.color_lost, esperado.alpha_lost)
            );
        }
    }

    // Propiedad 3: luminancia.
    #[test]
    fn luma_de_un_gris_es_el_gris(v in any::<u8>()) {
        prop_assert_eq!(luma(v, v, v), v);
    }

    #[test]
    fn luma_esta_cerca_de_su_fraccion_y_de_rec_709(r in canal(), g in canal(), b in canal()) {
        let l = i64::from(luma(r, g, b));
        let (r, g, b) = (i64::from(r), i64::from(g), i64::from(b));
        prop_assert!((256 * l - (54 * r + 183 * g + 19 * b)).abs() <= 128);
        prop_assert!((10000 * l - (2126 * r + 7152 * g + 722 * b)).abs() <= 10200);
        prop_assert_eq!(i128::from(l), luma_ingenua(r as u8, g as u8, b as u8));
    }

    #[test]
    fn luma_es_monotona_en_cada_canal(
        r in canal(), g in canal(), b in canal(),
        r2 in canal(), g2 in canal(), b2 in canal(),
    ) {
        let (r_alto, r_bajo) = (r.max(r2), r.min(r2));
        let (g_alto, g_bajo) = (g.max(g2), g.min(g2));
        let (b_alto, b_bajo) = (b.max(b2), b.min(b2));
        prop_assert!(luma(r_bajo, g, b) <= luma(r_alto, g, b));
        prop_assert!(luma(r, g_bajo, b) <= luma(r, g_alto, b));
        prop_assert!(luma(r, g, b_bajo) <= luma(r, g, b_alto));
    }

    // Propiedad 3: composición, a través de `Gray` con un fondo cualquiera.
    #[test]
    fn composicion_con_redondeo_al_mas_cercano(
        r in canal(), g in canal(), b in canal(),
        a in prop_oneof![any::<u8>(), Just(0u8), Just(255u8), Just(128u8)],
        bg in fondo(),
    ) {
        prop_composicion(r, g, b, a, bg)?;
    }

    // Propiedad 4: paleta determinista y límite de 256.
    #[test]
    fn paleta_determinista_y_limite_de_256(img in imagen_grande()) {
        prop_paleta(&img)?;
    }
}

// Las propiedades 4 con imágenes pequeñas de cualquier modo origen (sin `Indexed`) y pocos
// colores, para que el caso `<= 256` sea el habitual.
proptest! {
    #[test]
    fn paleta_determinista_con_imagenes_pequenas(img in imagen_cualquiera()) {
        if img.mode() != Mode::Indexed {
            prop_paleta(&img)?;
        }
    }
}

// Fila 21: el RGB de un píxel con `a = 0` no cuenta como color perdido en ningún destino.

#[test]
fn fila_21_los_transparentes_con_rgb_distinto_no_cuentan_como_color() {
    // Posiciones de los píxeles que no son transparentes; el resto, 300 transparentes con RGB
    // distinto, en orden de `i`.
    #[derive(Clone, Copy)]
    enum Tipo {
        Gris(u8),
        Semi,
    }
    let especiales = [
        (3, Tipo::Gris(10)),
        (50, Tipo::Gris(90)),
        (120, Tipo::Semi),
        (121, Tipo::Gris(170)),
        (200, Tipo::Semi),
        (305, Tipo::Gris(250)),
    ];
    let mut colores = Vec::new();
    let mut tipos = Vec::new();
    let mut i = 0usize;
    for posicion in 0..306 {
        match especiales.iter().find(|(p, _)| *p == posicion) {
            Some(&(_, Tipo::Gris(v))) => {
                colores.push(opaco(v, v, v));
                tipos.push(Some(Tipo::Gris(v)));
            }
            Some(&(_, Tipo::Semi)) => {
                colores.push(rgba(50, 50, 50, 128));
                tipos.push(Some(Tipo::Semi));
            }
            None => {
                colores.push(rgba(
                    (i % 256) as u8,
                    (i / 256) as u8,
                    ((7 * i) % 256) as u8,
                    0,
                ));
                tipos.push(None);
                i += 1;
            }
        }
    }
    assert_eq!(i, 300);
    let origen = imagen_de_colores(306, 1, &colores);

    // GrayAlpha sin ningún permiso: nada se pierde, y los transparentes salen canónicos.
    let img = ok(&origen, Mode::GrayAlpha, N, 0, 0);
    let esperados: Vec<(u8, u8)> = tipos
        .iter()
        .map(|t| match t {
            None => (0, 0),
            Some(Tipo::Gris(v)) => (*v, 255),
            Some(Tipo::Semi) => (50, 128),
        })
        .collect();
    assert_eq!(pares(&img), esperados);

    // Gray con el fondo blanco y con el negro: 300 + 2 píxeles pierden alfa, ninguno color.
    // Los transparentes dan el fondo; 152 y 25 son `comp(50, fondo, 128)` hechos a mano.
    for (permiso, fondo, semi) in [(A, 255u8, 152u8), (A0, 0u8, 25u8)] {
        let img = ok(&origen, Mode::Gray, permiso, 0, 302);
        let esperados: Vec<u8> = tipos
            .iter()
            .map(|t| match t {
                None => fondo,
                Some(Tipo::Gris(v)) => *v,
                Some(Tipo::Semi) => semi,
            })
            .collect();
        assert_eq!(grises(&img), esperados, "fondo {fondo}");
    }
    // Sin `drop_alpha` hace falta el permiso de alfa, pero nunca el de `luma`.
    rechaza(&origen, Mode::Gray, N, 0, 302);
    rechaza(&origen, Mode::Gray, L, 0, 302);

    // Control: un único píxel visible con color sí cuenta, y solo él (1, nunca 301).
    let mut con_rojo = colores.clone();
    con_rojo.push(opaco(200, 0, 0));
    let origen = imagen_de_colores(307, 1, &con_rojo);
    rechaza(&origen, Mode::GrayAlpha, N, 1, 0);
    rechaza(&origen, Mode::Gray, A, 1, 302);
    let img = ok(&origen, Mode::GrayAlpha, L, 1, 0);
    assert_eq!(pares(&img).last(), Some(&(luma(200, 0, 0), 255)));
}

// Propiedad 7: umbral de 256 colores.

proptest! {
    #[test]
    fn umbral_de_256_colores(
        n in prop::sample::select(vec![255usize, 256, 257]),
        base in 0u32..(1 << 24),
        paso in any::<u32>(),
        transparentes in prop_oneof![
            Just(None),
            (1usize..=40, any::<bool>()).prop_map(Some),
        ],
        orden in prop::collection::vec(any::<u32>(), 300),
    ) {
        // `n` opacos distintos: un paso impar recorre los 2^24 colores sin repetir ninguno.
        let paso = u64::from((paso | 1) & 0x00ff_ffff);
        let mut colores: Vec<Rgba> = (0..n as u64)
            .map(|i| {
                let v = (u64::from(base) + i * paso) % (1 << 24);
                opaco((v >> 16) as u8, (v >> 8) as u8, v as u8)
            })
            .collect();
        // Transparentes con el mismo RGB o con RGB distinto.
        if let Some((k, distintos)) = transparentes {
            colores.extend((0..k).map(|i| {
                if distintos {
                    rgba((i * 37) as u8, (i * 11 + 1) as u8, (255 - i) as u8, 0)
                } else {
                    transparente()
                }
            }));
        }
        // Orden aleatorio, estable: el transparente puede ir en cualquier sitio.
        let mut posiciones: Vec<usize> = (0..colores.len()).collect();
        posiciones.sort_by_key(|&p| orden[p]);
        let colores: Vec<Rgba> = posiciones.iter().map(|&p| colores[p]).collect();
        let origen = imagen_de_colores(colores.len() as u32, 1, &colores);

        let hay_transparente = transparentes.is_some();
        let entradas = n + usize::from(hay_transparente);
        let resultado = convert_image(&origen, Mode::Indexed, Allowed::NOTHING);
        if entradas <= 256 {
            let conversion = resultado
                .map_err(|e| TestCaseError::fail(format!("{entradas} entradas y falló: {e:?}")))?;
            prop_assert_eq!(conversion.report, ConversionReport::default());
            prop_assert_eq!(conversion.image.palette().unwrap().len(), entradas);
            prop_assert_eq!(
                indice_transparente(&conversion.image),
                hay_transparente.then_some(0)
            );
            let canonicos: Vec<Rgba> = colores
                .iter()
                .map(|c| if c.a == 0 { transparente() } else { *c })
                .collect();
            prop_assert_eq!(resueltos(&conversion.image), canonicos);
        } else {
            prop_assert_eq!(
                resultado.err(),
                Some(ConvertError::TooManyColors { at_least: 257 }),
                "{} entradas", entradas
            );
        }
    }
}
