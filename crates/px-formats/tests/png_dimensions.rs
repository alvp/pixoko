//! Tests de `png_dimensions` (specs/hito-2-abrir-guardar.md, sección «Cambios en otras
//! crates»; filas 23 y 24 y la primera parte de la propiedad 6).
//!
//! Los bytes de firma + `IHDR` se construyen aquí a mano, con o sin CRC válido según el caso
//! (la función no verifica el CRC ni nada posterior al `IHDR`). Ningún test reserva más de
//! unos pocos KiB: las dimensiones enormes se prueban con cabeceras de 33 bytes y sin datos.

use std::path::Path;

use proptest::collection::vec;
use proptest::prelude::*;
use px_core::{Image, Mode, Palette, Rgba};
use px_formats::{PngError, png_dimensions, read_png, write_png};

// ---------------------------------------------------------------------------------------
// Ayudantes: firma, CRC-32 y chunks.
// ---------------------------------------------------------------------------------------

const TEST_SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

/// Máximo de una dimensión en el formato PNG: 2^31 - 1.
const MAXIMUM: u32 = (1 << 31) - 1;

/// CRC-32 de la especificación PNG (polinomio 0xEDB88320, bit a bit).
fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in data {
        crc ^= u32::from(b);
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

/// Los 13 bytes de datos de un `IHDR`: 8 bits, RGBA, sin entrelazado.
fn ihdr_data(width: u32, height: u32) -> Vec<u8> {
    let mut v = Vec::with_capacity(13);
    v.extend(width.to_be_bytes());
    v.extend(height.to_be_bytes());
    v.extend([8, 6, 0, 0, 0]);
    v
}

/// Chunk con la longitud que se quiera (puede no coincidir con los datos) y el CRC dado.
fn chunk_raw(length: u32, kind: &[u8; 4], data: &[u8], crc: u32) -> Vec<u8> {
    let mut v = Vec::new();
    v.extend(length.to_be_bytes());
    v.extend(kind);
    v.extend(data);
    v.extend(crc.to_be_bytes());
    v
}

/// Firma + `IHDR` completo con CRC válido.
fn header_valid(width: u32, height: u32) -> Vec<u8> {
    let data = ihdr_data(width, height);
    let mut body = b"IHDR".to_vec();
    body.extend(&data);
    let mut v = TEST_SIGNATURE.to_vec();
    v.extend(chunk_raw(13, b"IHDR", &data, crc32(&body)));
    v
}

/// Firma + `IHDR` con el CRC a cero (no válido): lo único que hay en el fichero.
fn header_without_valid_crc(width: u32, height: u32) -> Vec<u8> {
    let mut v = TEST_SIGNATURE.to_vec();
    v.extend(chunk_raw(13, b"IHDR", &ihdr_data(width, height), 0));
    v
}

fn es_decode<T: std::fmt::Debug>(r: Result<T, PngError>) -> bool {
    matches!(r, Err(PngError::Decode(_)))
}

/// Imagen de cada modo; el indexado usa una paleta de `n` colores (de 1 a 256).
fn image(mode: Mode, width: u32, height: u32, n: usize) -> Image {
    match mode {
        Mode::Indexed => {
            let palette =
                Palette::from_colors((0..n).map(|i| Rgba::new(i as u8, 255 - i as u8, 7, 255)))
                    .unwrap();
            Image::new_indexed(width, height, palette).unwrap()
        }
        _ => Image::new(width, height, mode).unwrap(),
    }
}

const ALL: [Mode; 4] = [Mode::Rgba, Mode::Indexed, Mode::Gray, Mode::GrayAlpha];

// ---------------------------------------------------------------------------------------
// Fila 23: dimensiones correctas.
// ---------------------------------------------------------------------------------------

#[test]
fn row_23_write_png_in_the_four_modes() {
    // Fila 23: las dimensiones de `write_png` en los cuatro modos, con lados distintos.
    for mode in ALL {
        for (width, height) in [(1, 1), (3, 2), (2, 5), (16, 1)] {
            let bytes = write_png(&image(mode, width, height, 3)).unwrap();
            assert_eq!(
                png_dimensions(&bytes),
                Ok((width, height)),
                "{mode:?} {width}x{height}"
            );
        }
    }
}

#[test]
fn row_23_golden_corpus() {
    // Fila 23: cada PNG del corpus da las dimensiones que luego decodifica `read_png`.
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../golden/png");
    let mut seen = 0;
    for input in std::fs::read_dir(&dir).unwrap() {
        let path = input.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("png") {
            continue;
        }
        let bytes = std::fs::read(&path).unwrap();
        let read = read_png(&bytes).unwrap().image;
        assert_eq!(
            png_dimensions(&bytes),
            Ok((read.width(), read.height())),
            "{}",
            path.display()
        );
        // El corpus mide 16 x 16 (spec, «Casos límite»).
        assert_eq!(png_dimensions(&bytes), Ok((16, 16)), "{}", path.display());
        seen += 1;
    }
    assert_eq!(seen, 4, "el corpus tiene cuatro PNG");
}

#[test]
fn row_23_only_signature_and_ihdr_without_valid_crc() {
    // Fila 23: 20000 x 30000 con solo firma + IHDR (33 bytes, CRC a cero): no se decodifica
    // ni se reserva nada proporcional a las dimensiones.
    let bytes = header_without_valid_crc(20000, 30000);
    assert_eq!(bytes.len(), 8 + 4 + 4 + 13 + 4);
    assert_eq!(png_dimensions(&bytes), Ok((20000, 30000)));
    // Con CRC válido da lo mismo.
    assert_eq!(
        png_dimensions(&header_valid(20000, 30000)),
        Ok((20000, 30000))
    );
}

#[test]
fn row_23_the_format_maximum_is_valid() {
    // Fila 23: 2^31 - 1 por 1 es válido (y también al revés y por sí mismo).
    assert_eq!(
        png_dimensions(&header_without_valid_crc(MAXIMUM, 1)),
        Ok((2147483647, 1))
    );
    assert_eq!(
        png_dimensions(&header_without_valid_crc(1, MAXIMUM)),
        Ok((1, 2147483647))
    );
    assert_eq!(
        png_dimensions(&header_without_valid_crc(MAXIMUM, MAXIMUM)),
        Ok((MAXIMUM, MAXIMUM))
    );
}

#[test]
fn row_23_does_not_look_at_the_crc_or_anything_after_the_ihdr() {
    // «No verifica el CRC ni nada posterior al IHDR»: da igual lo que venga después.
    let mut with_garbage = header_without_valid_crc(7, 9);
    with_garbage.extend([0xFF; 5]);
    assert_eq!(png_dimensions(&with_garbage), Ok((7, 9)));

    // Un CRC distinto de cero y también equivocado.
    let mut v = TEST_SIGNATURE.to_vec();
    v.extend(chunk_raw(13, b"IHDR", &ihdr_data(5, 6), 0xDEAD_BEEF));
    assert_eq!(png_dimensions(&v), Ok((5, 6)));

    // Sin IDAT ni IEND, o con un chunk posterior de longitud absurda.
    let mut v = header_valid(5, 6);
    v.extend(0xFFFF_FFFFu32.to_be_bytes());
    v.extend(b"IDAT");
    assert_eq!(png_dimensions(&v), Ok((5, 6)));
}

// ---------------------------------------------------------------------------------------
// Fila 24: diez entradas que dan Err(Decode).
// ---------------------------------------------------------------------------------------

#[test]
fn row_24_empty() {
    assert!(es_decode(png_dimensions(&[])));
}

#[test]
fn row_24_seven_bytes() {
    // Menos que la firma.
    assert!(es_decode(png_dimensions(&TEST_SIGNATURE[..7])));
}

#[test]
fn row_24_wrong_signature() {
    // Una cabecera válida con un byte de la firma cambiado, en cada posición.
    for (i, _) in TEST_SIGNATURE.iter().enumerate() {
        let mut v = header_valid(4, 4);
        v[i] ^= 0x01;
        assert!(es_decode(png_dimensions(&v)), "byte {i} de la firma");
    }
    // Y un JPEG o texto cualquiera.
    assert!(es_decode(png_dimensions(b"\xFF\xD8\xFF\xE0 no es un PNG")));
}

#[test]
fn row_24_signature_alone() {
    assert!(es_decode(png_dimensions(&TEST_SIGNATURE)));
}

#[test]
fn row_24_ihdr_truncated_in_the_middle() {
    // Fila 24: la firma y el IHDR cortado en cada posición antes de que estén completos los
    // 8 bytes de anchura y altura (8 + 8 + 8 = 24 bytes). Cortar entre ese punto y el final de
    // los 13 bytes de datos queda sin fijar por la spec y no se prueba.
    let complete = header_valid(4, 4);
    for cut in 8..(8 + 8 + 8) {
        assert!(
            es_decode(png_dimensions(&complete[..cut])),
            "cortado en {cut}"
        );
    }
    // A mitad de los datos, expresamente.
    assert!(es_decode(png_dimensions(&complete[..8 + 8 + 6])));
}

#[test]
fn row_24_first_chunk_idat() {
    // Fila 24: el primer chunk no es IHDR (aunque lleve 13 bytes y buenas dimensiones).
    let mut v = TEST_SIGNATURE.to_vec();
    v.extend(chunk_raw(13, b"IDAT", &ihdr_data(4, 4), 0));
    assert!(es_decode(png_dimensions(&v)));
    // Tampoco con un tipo parecido en minúsculas.
    let mut v = TEST_SIGNATURE.to_vec();
    v.extend(chunk_raw(13, b"ihdr", &ihdr_data(4, 4), 0));
    assert!(es_decode(png_dimensions(&v)));
}

#[test]
fn row_24_ihdr_of_length_12() {
    // Fila 24: IHDR con longitud declarada 12 (y 12 bytes de datos).
    let data = &ihdr_data(4, 4)[..12];
    let mut v = TEST_SIGNATURE.to_vec();
    v.extend(chunk_raw(12, b"IHDR", data, 0));
    assert!(es_decode(png_dimensions(&v)));
    // Y la longitud 14 tampoco vale.
    let mut data = ihdr_data(4, 4);
    data.push(0);
    let mut v = TEST_SIGNATURE.to_vec();
    v.extend(chunk_raw(14, b"IHDR", &data, 0));
    assert!(es_decode(png_dimensions(&v)));
}

#[test]
fn row_24_width_zero() {
    assert!(es_decode(png_dimensions(&header_without_valid_crc(0, 5))));
    assert!(es_decode(png_dimensions(&header_valid(0, 5))));
}

#[test]
fn row_24_height_zero() {
    assert!(es_decode(png_dimensions(&header_without_valid_crc(5, 0))));
    assert!(es_decode(png_dimensions(&header_valid(5, 0))));
}

#[test]
fn row_24_width_2_to_the_31() {
    // Fila 24: 2^31 es una más que el máximo; también el alto, y el máximo de u32.
    assert!(es_decode(png_dimensions(&header_without_valid_crc(
        1 << 31,
        1
    ))));
    assert!(es_decode(png_dimensions(&header_without_valid_crc(
        1,
        1 << 31
    ))));
    assert!(es_decode(png_dimensions(&header_without_valid_crc(
        u32::MAX,
        u32::MAX
    ))));
}

// ---------------------------------------------------------------------------------------
// Propiedad 6, primera parte.
// ---------------------------------------------------------------------------------------

proptest! {
    /// Propiedad 6: con bytes arbitrarios nunca entra en pánico (da `Ok` o `Err`).
    #[test]
    fn property_6_arbitrary_bytes_without_panic(bytes in vec(any::<u8>(), 0..64)) {
        let _ = png_dimensions(&bytes);
    }

    /// Propiedad 6: con la firma delante, y así llegando más hondo, tampoco hay pánico, y
    /// un `Err` es siempre `Decode`.
    #[test]
    fn property_6_signature_and_arbitrary_bytes_without_panic(rest_2 in vec(any::<u8>(), 0..48)) {
        let mut bytes = TEST_SIGNATURE.to_vec();
        bytes.extend(rest_2);
        if let Err(e) = png_dimensions(&bytes) {
            prop_assert!(matches!(e, PngError::Decode(_)), "{e:?}");
        }
    }

    /// Propiedad 6: con un `IHDR` bien formado, `Ok` si y solo si ambas dimensiones están
    /// entre 1 y 2^31 - 1, para cualquier `(u32, u32)`.
    #[test]
    fn property_6_any_ihdr_dimension(width in any::<u32>(), height in any::<u32>()) {
        let r = png_dimensions(&header_without_valid_crc(width, height));
        if (1..=MAXIMUM).contains(&width) && (1..=MAXIMUM).contains(&height) {
            prop_assert_eq!(r, Ok((width, height)));
        } else {
            prop_assert!(matches!(r, Err(PngError::Decode(_))), "{r:?}");
        }
    }

    /// Propiedad 6: para cualquier `write_png(image)` (1 a 16 de lado, los cuatro modos,
    /// paletas de 1 a 256 colores) da las dimensiones de la imagen.
    #[test]
    fn property_6_write_png_gives_the_dimensions_of_the_image(
        mode in 0..4usize,
        width in 1..=16u32,
        height in 1..=16u32,
        n in 1..=256usize,
    ) {
        let img = image(ALL[mode], width, height, n);
        let bytes = write_png(&img).unwrap();
        prop_assert_eq!(png_dimensions(&bytes), Ok((img.width(), img.height())));
    }
}
