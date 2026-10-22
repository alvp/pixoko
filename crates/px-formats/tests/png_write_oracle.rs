//! Oráculo de `write_png` (specs/hito-2-image-filas-bytes.md, migración M1).
//!
//! `write_png_old` es una copia, dentro de la prueba, de la implementación de `write_png`
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

fn write_png_old(image: &Image) -> Result<Vec<u8>, PngError> {
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

/// Rellena `img` con `pixels` en orden de lectura.
fn fills(mut img: Image, pixels: impl IntoIterator<Item = Pixel>) -> Image {
    let w = img.width();
    for (i, p) in pixels.into_iter().enumerate() {
        img.set_pixel(i as u32 % w, i as u32 / w, p).unwrap();
    }
    img
}

/// Todos los píxeles de `img` en orden de lectura.
fn pixels(img: &Image) -> Vec<Pixel> {
    (0..img.height())
        .flat_map(|y| (0..img.width()).map(move |x| (x, y)))
        .map(|(x, y)| img.get_pixel(x, y).unwrap())
        .collect()
}

/// `img`, salvo en un indexado con índice transparente, cuya paleta sale ya resuelta (es lo
/// que devuelve `read_png`: el alfa 0 va en el tRNS y no queda índice transparente).
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
    fills(copy, pixels(img))
}

/// Comprueba que `write_png` y el oráculo dan lo mismo (bytes o error con el mismo texto).
fn assert_equal_to_the_oracle(img: &Image, context: &str) -> Vec<u8> {
    let new_2 = write_png(img).map_err(|e| e.to_string());
    let old = write_png_old(img).map_err(|e| e.to_string());
    assert_eq!(new_2, old, "{context}");
    new_2.unwrap_or_else(|e| panic!("{context}: no se pudo escribir: {e}"))
}

fn rgba_arbitrary() -> impl Strategy<Value = Rgba> {
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
fn image_arbitrary() -> BoxedStrategy<Image> {
    (1u32..=16, 1u32..=16)
        .prop_flat_map(|(w, h)| {
            let n = (w * h) as usize;
            prop_oneof![
                vec(rgba_arbitrary(), n).prop_map(move |px| fills(
                    Image::new(w, h, Mode::Rgba).unwrap(),
                    px.into_iter().map(Pixel::Rgba)
                )),
                vec(any::<u8>(), n).prop_map(move |px| fills(
                    Image::new(w, h, Mode::Gray).unwrap(),
                    px.into_iter().map(Pixel::Gray)
                )),
                vec(any::<(u8, u8)>(), n).prop_map(move |px| fills(
                    Image::new(w, h, Mode::GrayAlpha).unwrap(),
                    px.into_iter()
                        .map(|(gray, alpha)| Pixel::GrayAlpha { gray, alpha })
                )),
                (1usize..=256)
                    .prop_flat_map(move |len| {
                        (
                            vec(rgba_arbitrary(), len),
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

// ---------------------------------------------------------------------------------------
// Corpus.
// ---------------------------------------------------------------------------------------

#[test]
fn golden_corpus_writes_the_same_as_the_oracle() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../golden/png");
    let mut seen = 0;
    for input in std::fs::read_dir(&dir).unwrap() {
        let path = input.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("png") {
            continue;
        }
        let bytes = std::fs::read(&path).unwrap();
        let Ok(read) = read_png(&bytes) else {
            continue;
        };
        assert_equal_to_the_oracle(&read.image, &path.display().to_string());
        seen += 1;
    }
    assert!(seen >= 4, "el corpus tiene al menos cuatro PNG legibles");
}

// ---------------------------------------------------------------------------------------
// Casos.
// ---------------------------------------------------------------------------------------

#[test]
fn a_1x1_image_in_each_mode_writes_the_same_as_the_oracle() {
    let cases = [
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
    for (mode, pixel) in cases {
        let img = fills(Image::new(1, 1, mode).unwrap(), [pixel]);
        assert_equal_to_the_oracle(&img, &format!("{mode:?} 1x1"));
    }
    let palette = Palette::from_colors([Rgba::new(10, 20, 30, 128)]).unwrap();
    let img = fills(
        Image::new_indexed(1, 1, palette).unwrap(),
        [Pixel::Indexed(0)],
    );
    assert_equal_to_the_oracle(&img, "Indexed 1x1");
}

#[test]
fn indexed_with_palette_of_256_writes_the_same_as_the_oracle() {
    let colors = (0..=255u8).map(|i| Rgba::new(i, 255 - i, i ^ 0x5a, i.wrapping_mul(7)));
    let mut palette = Palette::from_colors(colors).unwrap();
    palette.set_transparent_index(Some(255)).unwrap();
    // 16 x 16: cada índice aparece una vez.
    let img = fills(
        Image::new_indexed(16, 16, palette).unwrap(),
        (0..=255u8).map(Pixel::Indexed),
    );
    let bytes = assert_equal_to_the_oracle(&img, "Indexed 256 colores");
    assert_eq!(read_png(&bytes).unwrap().image, normalized(&img));
}

// ---------------------------------------------------------------------------------------
// Propiedades.
// ---------------------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    // Salida idéntica byte a byte a la del oráculo, y ida y vuelta por `read_png`.
    #[test]
    fn p_write_png_equals_the_oracle_and_round_trip(img in image_arbitrary()) {
        let new_2 = write_png(&img).map_err(|e| e.to_string());
        let old = write_png_old(&img).map_err(|e| e.to_string());
        prop_assert_eq!(&new_2, &old);
        let bytes = new_2.unwrap();
        let read = read_png(&bytes).unwrap();
        prop_assert_eq!(read.image, normalized(&img));
    }
}
