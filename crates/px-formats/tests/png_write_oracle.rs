//! Oráculo de `write_png` (specs/hito-2-image-filas-bytes.md, migración M1).
//!
//! `write_png_antiguo` es una copia, dentro de la prueba, de la implementación de `write_png`
//! anterior a la migración (recorre la imagen con `get_pixel` y copia los píxeles a un `Vec`).
//! La salida de `write_png` debe ser idéntica byte a byte, con el corpus de `golden/png` y con
//! imágenes arbitrarias de los cuatro modos. Estas pruebas pasan con la implementación antigua
//! y han de seguir pasando con la que escribe `image.bytes()` directamente.

use std::path::Path;

use png::{BitDepth, ColorType, Encoder};
use proptest::collection::vec;
use proptest::prelude::*;
use px_core::{Image, Mode, Palette, Pixel, Rgba};
use px_formats::{PngError, read_png, write_png};

// ---------------------------------------------------------------------------------------
// Oráculo: copia de la implementación antigua.
// ---------------------------------------------------------------------------------------

fn encode_error(e: png::EncodingError) -> PngError {
    PngError::Encode(e.to_string())
}

fn write_png_antiguo(image: &Image) -> Result<Vec<u8>, PngError> {
    let (width, height) = (image.width(), image.height());
    let mut out = Vec::new();
    let mut encoder = Encoder::new(&mut out, width, height);
    encoder.set_depth(BitDepth::Eight);
    encoder.set_color(match image.mode() {
        Mode::Indexed => ColorType::Indexed,
        Mode::Gray => ColorType::Grayscale,
        Mode::GrayAlpha => ColorType::GrayscaleAlpha,
        Mode::Rgba => ColorType::Rgba,
    });
    if let Some(palette) = image.palette() {
        if palette.is_empty() {
            return Err(PngError::EmptyPalette);
        }
        let mut plte = Vec::with_capacity(palette.len() * 3);
        let mut alphas = Vec::with_capacity(palette.len());
        for (i, color) in palette.colors().iter().enumerate() {
            plte.extend_from_slice(&[color.r, color.g, color.b]);
            let index = u8::try_from(i).map_err(|_| PngError::Encode("paleta larga".into()))?;
            let resolved = palette
                .resolved(index)
                .map_err(|e| PngError::Encode(e.to_string()))?;
            alphas.push(resolved.a);
        }
        encoder.set_palette(plte);
        // tRNS llega hasta la última entrada con alfa distinto de 255; si no hay, se omite.
        if let Some(last) = alphas.iter().rposition(|&a| a != 255) {
            alphas.truncate(last + 1);
            encoder.set_trns(alphas);
        }
    }

    let len = Image::byte_len(width, height, image.mode())
        .map_err(|e| PngError::Encode(e.to_string()))?;
    let mut data = Vec::new();
    data.try_reserve_exact(len)
        .map_err(|_| PngError::Encode("sin memoria para los píxeles".to_string()))?;
    for y in 0..height {
        for x in 0..width {
            let pixel = image
                .get_pixel(x, y)
                .map_err(|e| PngError::Encode(e.to_string()))?;
            match pixel {
                Pixel::Rgba(c) => data.extend_from_slice(&[c.r, c.g, c.b, c.a]),
                Pixel::Indexed(i) | Pixel::Gray(i) => data.push(i),
                Pixel::GrayAlpha { gray, alpha } => data.extend_from_slice(&[gray, alpha]),
            }
        }
    }

    let mut writer = encoder.write_header().map_err(encode_error)?;
    writer.write_image_data(&data).map_err(encode_error)?;
    writer.finish().map_err(encode_error)?;
    Ok(out)
}

// ---------------------------------------------------------------------------------------
// Ayudantes.
// ---------------------------------------------------------------------------------------

/// Rellena `img` con `pixeles` en orden de lectura.
fn rellena(mut img: Image, pixeles: impl IntoIterator<Item = Pixel>) -> Image {
    let w = img.width();
    for (i, p) in pixeles.into_iter().enumerate() {
        img.set_pixel(i as u32 % w, i as u32 / w, p).unwrap();
    }
    img
}

/// Todos los píxeles de `img` en orden de lectura.
fn pixeles(img: &Image) -> Vec<Pixel> {
    (0..img.height())
        .flat_map(|y| (0..img.width()).map(move |x| (x, y)))
        .map(|(x, y)| img.get_pixel(x, y).unwrap())
        .collect()
}

/// `img`, salvo en un indexado con índice transparente, cuya paleta sale ya resuelta (es lo
/// que devuelve `read_png`: el alfa 0 va en el tRNS y no queda índice transparente).
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

/// Comprueba que `write_png` y el oráculo dan lo mismo (bytes o error con el mismo texto).
fn assert_igual_al_oraculo(img: &Image, contexto: &str) -> Vec<u8> {
    let nuevo = write_png(img).map_err(|e| e.to_string());
    let antiguo = write_png_antiguo(img).map_err(|e| e.to_string());
    assert_eq!(nuevo, antiguo, "{contexto}");
    nuevo.unwrap_or_else(|e| panic!("{contexto}: no se pudo escribir: {e}"))
}

fn rgba_arbitrario() -> impl Strategy<Value = Rgba> {
    (
        any::<u8>(),
        any::<u8>(),
        any::<u8>(),
        // Con frecuencia a = 0, con RGB casi siempre distinto de 0.
        prop_oneof![Just(0u8), Just(255u8), any::<u8>()],
    )
        .prop_map(|(r, g, b, a)| Rgba::new(r, g, b, a))
}

/// Imagen de 1 a 16 de lado en cualquiera de los cuatro modos; el indexado lleva una paleta
/// de 1 a 256 entradas y a veces un índice transparente fijado.
fn imagen_arbitraria() -> BoxedStrategy<Image> {
    (1u32..=16, 1u32..=16)
        .prop_flat_map(|(w, h)| {
            let n = (w * h) as usize;
            prop_oneof![
                vec(rgba_arbitrario(), n).prop_map(move |px| rellena(
                    Image::new(w, h, Mode::Rgba).unwrap(),
                    px.into_iter().map(Pixel::Rgba)
                )),
                vec(any::<u8>(), n).prop_map(move |px| rellena(
                    Image::new(w, h, Mode::Gray).unwrap(),
                    px.into_iter().map(Pixel::Gray)
                )),
                vec(any::<(u8, u8)>(), n).prop_map(move |px| rellena(
                    Image::new(w, h, Mode::GrayAlpha).unwrap(),
                    px.into_iter()
                        .map(|(gray, alpha)| Pixel::GrayAlpha { gray, alpha })
                )),
                (1usize..=256)
                    .prop_flat_map(move |len| {
                        (
                            vec(rgba_arbitrario(), len),
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

// ---------------------------------------------------------------------------------------
// Corpus.
// ---------------------------------------------------------------------------------------

#[test]
fn corpus_de_golden_escribe_lo_mismo_que_el_oraculo() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../golden/png");
    let mut vistos = 0;
    for entrada in std::fs::read_dir(&dir).unwrap() {
        let ruta = entrada.unwrap().path();
        if ruta.extension().and_then(|e| e.to_str()) != Some("png") {
            continue;
        }
        let bytes = std::fs::read(&ruta).unwrap();
        let Ok(leida) = read_png(&bytes) else {
            continue;
        };
        assert_igual_al_oraculo(&leida.image, &ruta.display().to_string());
        vistos += 1;
    }
    assert!(vistos >= 4, "el corpus tiene al menos cuatro PNG legibles");
}

// ---------------------------------------------------------------------------------------
// Casos.
// ---------------------------------------------------------------------------------------

#[test]
fn una_imagen_de_1x1_en_cada_modo_escribe_lo_mismo_que_el_oraculo() {
    let casos = [
        (Mode::Rgba, Pixel::Rgba(Rgba::new(1, 2, 3, 4))),
        (Mode::Gray, Pixel::Gray(77)),
        (
            Mode::GrayAlpha,
            Pixel::GrayAlpha {
                gray: 9,
                alpha: 200,
            },
        ),
    ];
    for (modo, pixel) in casos {
        let img = rellena(Image::new(1, 1, modo).unwrap(), [pixel]);
        assert_igual_al_oraculo(&img, &format!("{modo:?} 1x1"));
    }
    let paleta = Palette::from_colors([Rgba::new(10, 20, 30, 128)]).unwrap();
    let img = rellena(
        Image::new_indexed(1, 1, paleta).unwrap(),
        [Pixel::Indexed(0)],
    );
    assert_igual_al_oraculo(&img, "Indexed 1x1");
}

#[test]
fn indexado_con_paleta_de_256_escribe_lo_mismo_que_el_oraculo() {
    let colores = (0..=255u8).map(|i| Rgba::new(i, 255 - i, i ^ 0x5a, i.wrapping_mul(7)));
    let mut paleta = Palette::from_colors(colores).unwrap();
    paleta.set_transparent_index(Some(255)).unwrap();
    // 16 x 16: cada índice aparece una vez.
    let img = rellena(
        Image::new_indexed(16, 16, paleta).unwrap(),
        (0..=255u8).map(Pixel::Indexed),
    );
    let bytes = assert_igual_al_oraculo(&img, "Indexed 256 colores");
    assert_eq!(read_png(&bytes).unwrap().image, normalizada(&img));
}

// ---------------------------------------------------------------------------------------
// Propiedades.
// ---------------------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    // Salida idéntica byte a byte a la del oráculo, y ida y vuelta por `read_png`.
    #[test]
    fn p_write_png_igual_al_oraculo_e_ida_y_vuelta(img in imagen_arbitraria()) {
        let nuevo = write_png(&img).map_err(|e| e.to_string());
        let antiguo = write_png_antiguo(&img).map_err(|e| e.to_string());
        prop_assert_eq!(&nuevo, &antiguo);
        let bytes = nuevo.unwrap();
        let leida = read_png(&bytes).unwrap();
        prop_assert_eq!(leida.image, normalizada(&img));
    }
}
