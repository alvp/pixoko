//! Objetivo de fuzzing de `read_aseprite` (specs/hito-3-aseprite.md).
//!
//! Un pánico, un desbordamiento de enteros o un agotamiento de memoria o de tiempo lo detectan
//! `libfuzzer-sys` y libFuzzer. Aquí se comprueba además que todo documento que `read_aseprite`
//! acepta sobrevive a `write_aseprite` y a una nueva lectura con el mismo contenido.

#![no_main]
#![forbid(unsafe_code)]

use libfuzzer_sys::fuzz_target;
use px_core::{Document, Image, Mode};
use px_formats::{AsepriteError, AsepriteLimits, read_aseprite, write_aseprite};

/// Límites pequeños: el fuzzer no debe pasarse el rato reservando lienzos enormes.
const LIMITS: AsepriteLimits = AsepriteLimits {
    max_pixels: 1 << 24,
    max_cells_bytes: 256 << 20,
};

/// Alfa 0 en RGBA, índice transparente en indexado.
fn transparent(image: &Image, pixel: &[u8]) -> bool {
    match image.mode() {
        Mode::Indexed => {
            image
                .palette()
                .and_then(|palette| palette.transparent_index())
                == Some(pixel[0])
        }
        _ => pixel[3] == 0,
    }
}

/// Mismo contenido de píxeles: una celda vacía y una toda transparente son iguales, y dos píxeles
/// transparentes son iguales aunque su RGB difiera.
fn same_cell(a: Option<&Image>, b: Option<&Image>, bpp: usize) -> bool {
    let any_visible = |image: &Image| {
        image
            .bytes()
            .chunks_exact(bpp)
            .any(|pixel| !transparent(image, pixel))
    };
    match (a, b) {
        (None, None) => true,
        (Some(image), None) | (None, Some(image)) => !any_visible(image),
        (Some(a), Some(b)) => a
            .bytes()
            .chunks_exact(bpp)
            .zip(b.bytes().chunks_exact(bpp))
            .all(|(p, q)| p == q || (transparent(a, p) && transparent(b, q))),
    }
}

fn same_content(a: &Document, b: &Document) -> bool {
    a.width() == b.width()
        && a.height() == b.height()
        && a.mode() == b.mode()
        && a.palette() == b.palette()
        && a.layers().len() == b.layers().len()
        && a.layers().iter().zip(b.layers()).all(|(x, y)| {
            (x.name(), x.visible(), x.opacity(), x.blend(), x.locked())
                == (y.name(), y.visible(), y.opacity(), y.blend(), y.locked())
                && same_cell(
                    a.image(x.cel()),
                    b.image(y.cel()),
                    a.mode().bytes_per_pixel(),
                )
        })
}

fuzz_target!(|data: &[u8]| {
    let Ok(read) = read_aseprite(data, &LIMITS) else {
        return;
    };
    let bytes = match write_aseprite(&read.document) {
        Ok(bytes) => bytes,
        // Un indexado sin índice transparente se lee, pero no se puede escribir.
        Err(AsepriteError::NoTransparentIndex) => return,
        Err(error) => {
            panic!("write_aseprite rechaza un documento que read_aseprite aceptó: {error}")
        }
    };
    let again = read_aseprite(&bytes, &LIMITS)
        .expect("read_aseprite rechaza lo que write_aseprite acaba de escribir");
    assert!(
        same_content(&read.document, &again.document),
        "la relectura cambia el contenido"
    );
});
