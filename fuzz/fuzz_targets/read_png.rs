//! Fuzzing target for `read_png` (specs/hito-1-fuzz.md).
//!
//! A panic, an integer overflow or running out of memory or time is caught by
//! `libfuzzer-sys` and libFuzzer. Here we also check that every image that
//! `read_png` accepts survives `write_png` and a new read unchanged.

#![no_main]
#![forbid(unsafe_code)]

use libfuzzer_sys::fuzz_target;
use px_core::Image;
use px_formats::{read_png, write_png};

/// Above this size the round trip is not checked: `write_png` copies the image and
/// with a canvas at the limit the peak would exceed 2 GiB.
const ROUNDTRIP_MAX_BYTES: usize = 16 << 20;

fuzz_target!(|data: &[u8]| {
    let Ok(read) = read_png(data) else {
        return;
    };
    let image = &read.image;
    match Image::byte_len(image.width(), image.height(), image.mode()) {
        Ok(len) if len <= ROUNDTRIP_MAX_BYTES => {}
        _ => return,
    }

    let bytes = write_png(image).expect("write_png rechaza una imagen que read_png aceptó");
    let again = read_png(&bytes).expect("read_png rechaza lo que write_png acaba de escribir");
    assert!(again.is_exact(), "la relectura declara conversiones");
    // No `assert_eq!`: with a large image it would dump tens of MiB of `Debug`.
    assert!(again.image == *image, "la ida y vuelta cambia la imagen");
});
