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

const FIRMA: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

/// Máximo de una dimensión en el formato PNG: 2^31 - 1.
const MAXIMO: u32 = (1 << 31) - 1;

/// CRC-32 de la especificación PNG (polinomio 0xEDB88320, bit a bit).
fn crc32(datos: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in datos {
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
fn ihdr_datos(ancho: u32, alto: u32) -> Vec<u8> {
    let mut v = Vec::with_capacity(13);
    v.extend(ancho.to_be_bytes());
    v.extend(alto.to_be_bytes());
    v.extend([8, 6, 0, 0, 0]);
    v
}

/// Chunk con la longitud que se quiera (puede no coincidir con los datos) y el CRC dado.
fn chunk_crudo(longitud: u32, tipo: &[u8; 4], datos: &[u8], crc: u32) -> Vec<u8> {
    let mut v = Vec::new();
    v.extend(longitud.to_be_bytes());
    v.extend(tipo);
    v.extend(datos);
    v.extend(crc.to_be_bytes());
    v
}

/// Firma + `IHDR` completo con CRC válido.
fn cabecera_valida(ancho: u32, alto: u32) -> Vec<u8> {
    let datos = ihdr_datos(ancho, alto);
    let mut cuerpo = b"IHDR".to_vec();
    cuerpo.extend(&datos);
    let mut v = FIRMA.to_vec();
    v.extend(chunk_crudo(13, b"IHDR", &datos, crc32(&cuerpo)));
    v
}

/// Firma + `IHDR` con el CRC a cero (no válido): lo único que hay en el fichero.
fn cabecera_sin_crc_valido(ancho: u32, alto: u32) -> Vec<u8> {
    let mut v = FIRMA.to_vec();
    v.extend(chunk_crudo(13, b"IHDR", &ihdr_datos(ancho, alto), 0));
    v
}

fn es_decode<T: std::fmt::Debug>(r: Result<T, PngError>) -> bool {
    matches!(r, Err(PngError::Decode(_)))
}

/// Imagen de cada modo; el indexado usa una paleta de `n` colores (de 1 a 256).
fn imagen(modo: Mode, ancho: u32, alto: u32, n: usize) -> Image {
    match modo {
        Mode::Indexed => {
            let paleta =
                Palette::from_colors((0..n).map(|i| Rgba::new(i as u8, 255 - i as u8, 7, 255)))
                    .unwrap();
            Image::new_indexed(ancho, alto, paleta).unwrap()
        }
        _ => Image::new(ancho, alto, modo).unwrap(),
    }
}

const TODOS: [Mode; 4] = [Mode::Rgba, Mode::Indexed, Mode::Gray, Mode::GrayAlpha];

// ---------------------------------------------------------------------------------------
// Fila 23: dimensiones correctas.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_23_write_png_en_los_cuatro_modos() {
    // Fila 23: las dimensiones de `write_png` en los cuatro modos, con lados distintos.
    for modo in TODOS {
        for (ancho, alto) in [(1, 1), (3, 2), (2, 5), (16, 1)] {
            let bytes = write_png(&imagen(modo, ancho, alto, 3)).unwrap();
            assert_eq!(
                png_dimensions(&bytes),
                Ok((ancho, alto)),
                "{modo:?} {ancho}x{alto}"
            );
        }
    }
}

#[test]
fn fila_23_corpus_de_golden() {
    // Fila 23: cada PNG del corpus da las dimensiones que luego decodifica `read_png`.
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../golden/png");
    let mut vistos = 0;
    for entrada in std::fs::read_dir(&dir).unwrap() {
        let ruta = entrada.unwrap().path();
        if ruta.extension().and_then(|e| e.to_str()) != Some("png") {
            continue;
        }
        let bytes = std::fs::read(&ruta).unwrap();
        let leida = read_png(&bytes).unwrap().image;
        assert_eq!(
            png_dimensions(&bytes),
            Ok((leida.width(), leida.height())),
            "{}",
            ruta.display()
        );
        // El corpus mide 16 x 16 (spec, «Casos límite»).
        assert_eq!(png_dimensions(&bytes), Ok((16, 16)), "{}", ruta.display());
        vistos += 1;
    }
    assert_eq!(vistos, 4, "el corpus tiene cuatro PNG");
}

#[test]
fn fila_23_solo_firma_e_ihdr_sin_crc_valido() {
    // Fila 23: 20000 x 30000 con solo firma + IHDR (33 bytes, CRC a cero): no se decodifica
    // ni se reserva nada proporcional a las dimensiones.
    let bytes = cabecera_sin_crc_valido(20000, 30000);
    assert_eq!(bytes.len(), 8 + 4 + 4 + 13 + 4);
    assert_eq!(png_dimensions(&bytes), Ok((20000, 30000)));
    // Con CRC válido da lo mismo.
    assert_eq!(
        png_dimensions(&cabecera_valida(20000, 30000)),
        Ok((20000, 30000))
    );
}

#[test]
fn fila_23_el_maximo_del_formato_es_valido() {
    // Fila 23: 2^31 - 1 por 1 es válido (y también al revés y por sí mismo).
    assert_eq!(
        png_dimensions(&cabecera_sin_crc_valido(MAXIMO, 1)),
        Ok((2147483647, 1))
    );
    assert_eq!(
        png_dimensions(&cabecera_sin_crc_valido(1, MAXIMO)),
        Ok((1, 2147483647))
    );
    assert_eq!(
        png_dimensions(&cabecera_sin_crc_valido(MAXIMO, MAXIMO)),
        Ok((MAXIMO, MAXIMO))
    );
}

#[test]
fn fila_23_no_mira_el_crc_ni_lo_posterior_al_ihdr() {
    // «No verifica el CRC ni nada posterior al IHDR»: da igual lo que venga después.
    let mut con_basura = cabecera_sin_crc_valido(7, 9);
    con_basura.extend([0xFF; 5]);
    assert_eq!(png_dimensions(&con_basura), Ok((7, 9)));

    // Un CRC distinto de cero y también equivocado.
    let mut v = FIRMA.to_vec();
    v.extend(chunk_crudo(13, b"IHDR", &ihdr_datos(5, 6), 0xDEAD_BEEF));
    assert_eq!(png_dimensions(&v), Ok((5, 6)));

    // Sin IDAT ni IEND, o con un chunk posterior de longitud absurda.
    let mut v = cabecera_valida(5, 6);
    v.extend(0xFFFF_FFFFu32.to_be_bytes());
    v.extend(b"IDAT");
    assert_eq!(png_dimensions(&v), Ok((5, 6)));
}

// ---------------------------------------------------------------------------------------
// Fila 24: diez entradas que dan Err(Decode).
// ---------------------------------------------------------------------------------------

#[test]
fn fila_24_vacio() {
    assert!(es_decode(png_dimensions(&[])));
}

#[test]
fn fila_24_siete_bytes() {
    // Menos que la firma.
    assert!(es_decode(png_dimensions(&FIRMA[..7])));
}

#[test]
fn fila_24_firma_incorrecta() {
    // Una cabecera válida con un byte de la firma cambiado, en cada posición.
    for (i, _) in FIRMA.iter().enumerate() {
        let mut v = cabecera_valida(4, 4);
        v[i] ^= 0x01;
        assert!(es_decode(png_dimensions(&v)), "byte {i} de la firma");
    }
    // Y un JPEG o texto cualquiera.
    assert!(es_decode(png_dimensions(b"\xFF\xD8\xFF\xE0 no es un PNG")));
}

#[test]
fn fila_24_firma_sola() {
    assert!(es_decode(png_dimensions(&FIRMA)));
}

#[test]
fn fila_24_ihdr_truncado_a_mitad() {
    // Fila 24: la firma y el IHDR cortado en cada posición antes de que estén completos los
    // 8 bytes de anchura y altura (8 + 8 + 8 = 24 bytes). Cortar entre ese punto y el final de
    // los 13 bytes de datos queda sin fijar por la spec y no se prueba.
    let completo = cabecera_valida(4, 4);
    for corte in 8..(8 + 8 + 8) {
        assert!(
            es_decode(png_dimensions(&completo[..corte])),
            "cortado en {corte}"
        );
    }
    // A mitad de los datos, expresamente.
    assert!(es_decode(png_dimensions(&completo[..8 + 8 + 6])));
}

#[test]
fn fila_24_primer_chunk_idat() {
    // Fila 24: el primer chunk no es IHDR (aunque lleve 13 bytes y buenas dimensiones).
    let mut v = FIRMA.to_vec();
    v.extend(chunk_crudo(13, b"IDAT", &ihdr_datos(4, 4), 0));
    assert!(es_decode(png_dimensions(&v)));
    // Tampoco con un tipo parecido en minúsculas.
    let mut v = FIRMA.to_vec();
    v.extend(chunk_crudo(13, b"ihdr", &ihdr_datos(4, 4), 0));
    assert!(es_decode(png_dimensions(&v)));
}

#[test]
fn fila_24_ihdr_de_longitud_12() {
    // Fila 24: IHDR con longitud declarada 12 (y 12 bytes de datos).
    let datos = &ihdr_datos(4, 4)[..12];
    let mut v = FIRMA.to_vec();
    v.extend(chunk_crudo(12, b"IHDR", datos, 0));
    assert!(es_decode(png_dimensions(&v)));
    // Y la longitud 14 tampoco vale.
    let mut datos = ihdr_datos(4, 4);
    datos.push(0);
    let mut v = FIRMA.to_vec();
    v.extend(chunk_crudo(14, b"IHDR", &datos, 0));
    assert!(es_decode(png_dimensions(&v)));
}

#[test]
fn fila_24_ancho_cero() {
    assert!(es_decode(png_dimensions(&cabecera_sin_crc_valido(0, 5))));
    assert!(es_decode(png_dimensions(&cabecera_valida(0, 5))));
}

#[test]
fn fila_24_alto_cero() {
    assert!(es_decode(png_dimensions(&cabecera_sin_crc_valido(5, 0))));
    assert!(es_decode(png_dimensions(&cabecera_valida(5, 0))));
}

#[test]
fn fila_24_ancho_2_a_la_31() {
    // Fila 24: 2^31 es una más que el máximo; también el alto, y el máximo de u32.
    assert!(es_decode(png_dimensions(&cabecera_sin_crc_valido(
        1 << 31,
        1
    ))));
    assert!(es_decode(png_dimensions(&cabecera_sin_crc_valido(
        1,
        1 << 31
    ))));
    assert!(es_decode(png_dimensions(&cabecera_sin_crc_valido(
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
    fn propiedad_6_bytes_arbitrarios_sin_panico(bytes in vec(any::<u8>(), 0..64)) {
        let _ = png_dimensions(&bytes);
    }

    /// Propiedad 6: con la firma delante, y así llegando más hondo, tampoco hay pánico, y
    /// un `Err` es siempre `Decode`.
    #[test]
    fn propiedad_6_firma_y_bytes_arbitrarios_sin_panico(resto in vec(any::<u8>(), 0..48)) {
        let mut bytes = FIRMA.to_vec();
        bytes.extend(resto);
        if let Err(e) = png_dimensions(&bytes) {
            prop_assert!(matches!(e, PngError::Decode(_)), "{e:?}");
        }
    }

    /// Propiedad 6: con un `IHDR` bien formado, `Ok` si y solo si ambas dimensiones están
    /// entre 1 y 2^31 - 1, para cualquier `(u32, u32)`.
    #[test]
    fn propiedad_6_cualquier_dimension_del_ihdr(ancho in any::<u32>(), alto in any::<u32>()) {
        let r = png_dimensions(&cabecera_sin_crc_valido(ancho, alto));
        if (1..=MAXIMO).contains(&ancho) && (1..=MAXIMO).contains(&alto) {
            prop_assert_eq!(r, Ok((ancho, alto)));
        } else {
            prop_assert!(matches!(r, Err(PngError::Decode(_))), "{r:?}");
        }
    }

    /// Propiedad 6: para cualquier `write_png(imagen)` (1 a 16 de lado, los cuatro modos,
    /// paletas de 1 a 256 colores) da las dimensiones de la imagen.
    #[test]
    fn propiedad_6_write_png_da_las_dimensiones_de_la_imagen(
        modo in 0..4usize,
        ancho in 1..=16u32,
        alto in 1..=16u32,
        n in 1..=256usize,
    ) {
        let img = imagen(TODOS[modo], ancho, alto, n);
        let bytes = write_png(&img).unwrap();
        prop_assert_eq!(png_dimensions(&bytes), Ok((img.width(), img.height())));
    }
}
