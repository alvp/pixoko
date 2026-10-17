//! Objetivo de fuzzing de `read_png` (specs/hito-1-fuzz.md).
//!
//! Un pánico, un desbordamiento de enteros o un agotamiento de memoria o de tiempo lo
//! detectan `libfuzzer-sys` y libFuzzer. Aquí se comprueba además que toda imagen que
//! `read_png` acepta sobrevive a `write_png` y a una nueva lectura sin cambiar.

#![no_main]
#![forbid(unsafe_code)]

use libfuzzer_sys::fuzz_target;
use px_core::Image;
use px_formats::{read_png, write_png};

/// Por encima de este tamaño no se comprueba la ida y vuelta: `write_png` copia la imagen y
/// con un lienzo en el límite el pico pasaría de 2 GiB.
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
    // Sin `assert_eq!`: con una imagen grande volcaría decenas de MiB de `Debug`.
    assert!(again.image == *image, "la ida y vuelta cambia la imagen");
});
