//! Fuzzing target for `read_aseprite` (specs/hito-3-aseprite.md).
//!
//! A panic, an integer overflow or running out of memory or time is caught by
//! `libfuzzer-sys` and libFuzzer. Here we also check that every document that `read_aseprite`
//! accepts survives `write_aseprite` and a new read with the same content.

#![no_main]
#![forbid(unsafe_code)]

use libfuzzer_sys::fuzz_target;
use px_core::{Document, Image, Mode};
use px_formats::{AsepriteError, AsepriteLimits, read_aseprite, write_aseprite};

/// Small limits: the fuzzer must not spend its time allocating huge canvases.
const LIMITS: AsepriteLimits = AsepriteLimits {
    max_pixels: 1 << 24,
    max_cells_bytes: 256 << 20,
};

/// Alpha 0 in RGBA, transparent index in indexed.
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

/// Same pixel content: an empty cel and a fully transparent one are equal, and two transparent
/// pixels are equal even if their RGB differs.
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
        // An indexed document without a transparent index is read, but cannot be written.
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
