//! Pruebas de memoria y de tiempo del relleno en lienzos de 8192 x 8192
//! (specs/hito-2-limites-relleno.md, filas 12 y 14).
//!
//! Todas son `#[ignore]` y solo existen en Linux: miden `VmHWM` (el RSS máximo del **proceso**,
//! de `/proc/self/status`), así que cada una debe ejecutarse sola, en release, en su propio
//! proceso. `--exact` y el nombre completo dejan fuera a las demás:
//!
//! ```text
//! cargo test -p px-raster --test fill_memory --release -- --ignored --exact --nocapture fila_12_peine_indexado_8192
//! cargo test -p px-raster --test fill_memory --release -- --ignored --exact --nocapture fila_14_uniforme_rgba_8192
//! cargo test -p px-raster --test fill_memory --release -- --ignored --exact --nocapture fila_14_uniforme_indexado_8192
//! ```
//!
//! Las tolerancias son generosas (detectan una regresión de cientos de MiB, no de unos pocos):
//! la fila 12 exige que el relleno rechazado crezca menos de 256 MiB (sin tope serían más de
//! 1 GiB; con él, unos 8 MiB) y la fila 14, menos de 128 MiB en indexado. Los tiempos se
//! imprimen y solo hacen fallar por encima de 30 s (fila 12) y de 60 s (fila 14).

#![cfg(target_os = "linux")]

use px_core::ImageId;
use std::time::{Duration, Instant};

use px_core::{Command, Document, DocumentError, Image, Mode, Palette, Pixel, Rgba};
use px_raster::fill_command;

const SIDE: u32 = 8192;
const MIB: u64 = 1024 * 1024;

/// `VmHWM` en bytes: el RSS máximo del proceso hasta ahora.
fn vm_hwm() -> u64 {
    let state = std::fs::read_to_string("/proc/self/status").expect("hay /proc/self/status");
    let line = state
        .lines()
        .find(|l| l.starts_with("VmHWM:"))
        .expect("hay una línea VmHWM");
    let kib: u64 = line
        .split_whitespace()
        .nth(1)
        .expect("VmHWM tiene un valor")
        .parse()
        .expect("VmHWM es un número");
    kib * 1024
}

fn palette() -> Palette {
    Palette::from_colors([Rgba::opaque(10, 20, 30), Rgba::opaque(200, 100, 50)]).unwrap()
}

fn a_rgba() -> Pixel {
    Pixel::Rgba(Rgba::new(10, 20, 30, 255))
}

fn c_rgba() -> Pixel {
    Pixel::Rgba(Rgba::new(200, 100, 50, 255))
}

/// Peine de 8192 x 8192 indexado (índice 0 = `A`, 1 = `B`): fila 0 toda `A`; en el resto, `A`
/// en las columnas pares y `B` en las impares. `S = 1 + 8191 * 4096 = 33 550 337` tramos.
fn comb_indexed() -> Image {
    let side = SIDE as usize;
    let mut data = Vec::with_capacity(side * side);
    data.resize(side, 0u8);
    for _ in 1..side {
        data.extend((0..side).map(|x| u8::from(x % 2 == 1)));
    }
    Image::from_bytes_indexed(SIDE, SIDE, palette(), data).unwrap()
}

fn spans_of(command: &Command) -> usize {
    match command {
        Command::FillSpans { spans, .. } => spans.len(),
        other => panic!("FillSpans was expected, got {other:?}"),
    }
}

/// Fila 12: el peine indexado se rechaza con `FillTooFragmented` sin disparar la memoria.
#[test]
#[ignore = "mide VmHWM: ejecutar sola y en release (ver la cabecera del fichero)"]
fn row_12_indexed_comb_8192() {
    let doc = Document::new(comb_indexed());
    let id = ImageId::from_raw(0);

    let base = vm_hwm();
    let start = Instant::now();
    let result = fill_command(&doc, id, 0, 0, Pixel::Indexed(1));
    let time = start.elapsed();
    let growth = vm_hwm().saturating_sub(base);

    println!(
        "fila 12: {result:?} en {time:?}; VmHWM base {} MiB, crece {} MiB",
        base / MIB,
        growth / MIB
    );
    assert!(
        matches!(
            result,
            Err(DocumentError::FillTooFragmented {
                limit: Document::MAX_FILL_SPANS
            })
        ),
        "FillTooFragmented with the real cap was expected, got {result:?}"
    );
    assert!(
        growth < 256 * MIB,
        "VmHWM grew {} MiB (the limit is 256)",
        growth / MIB
    );
    assert!(time < Duration::from_secs(30), "took {time:?}");
}

/// Fila 14, comprobaciones comunes: relleno de toda la imagen con 8192 tramos, `apply`, `undo` y
/// `redo` restauran los bytes. Devuelve el crecimiento de `VmHWM` sobre `base`.
fn fills_the_whole_image(
    mut doc: Document,
    original: &Image,
    fills: &Image,
    pixel: Pixel,
    label: &str,
) -> u64 {
    let id = ImageId::from_raw(0);
    let base = vm_hwm();
    let start = Instant::now();

    let t0 = Instant::now();
    let command = fill_command(&doc, id, 4096, 4096, pixel);
    let t_fill = t0.elapsed();
    let command = match command {
        Ok(Some(c)) => c,
        other => panic!("{label}: Ok(Some(..)) was expected, got {other:?}"),
    };
    assert_eq!(spans_of(&command), 8192, "{label}: one span per row");

    let t1 = Instant::now();
    assert_eq!(doc.apply(command), Ok(true), "{label}: apply");
    let t_apply = t1.elapsed();
    assert_eq!(
        doc.image(id).unwrap().bytes(),
        fills.bytes(),
        "{label}: the image is filled"
    );

    let t2 = Instant::now();
    assert_eq!(doc.undo(), Ok(()));
    let t_undo = t2.elapsed();
    assert_eq!(
        doc.image(id).unwrap().bytes(),
        original.bytes(),
        "{label}: undo restores the bytes"
    );

    let t3 = Instant::now();
    assert_eq!(doc.redo(), Ok(()));
    let t_redo = t3.elapsed();
    assert_eq!(
        doc.image(id).unwrap().bytes(),
        fills.bytes(),
        "{label}: redo vuelve a rellenar"
    );

    let total = start.elapsed();
    let growth = vm_hwm().saturating_sub(base);
    println!(
        "fila 14 {label}: fill_command {t_fill:?}, apply {t_apply:?}, undo {t_undo:?}, \
         redo {t_redo:?}, total {total:?}; VmHWM base {} MiB, crece {} MiB",
        base / MIB,
        growth / MIB
    );
    assert!(total < Duration::from_secs(60), "{label}: took {total:?}");
    growth
}

/// Fila 14: toda la imagen RGBA de un color sigue rellenándose (la memoria solo se imprime).
#[test]
#[ignore = "mide VmHWM: ejecutar sola y en release (ver la cabecera del fichero)"]
fn row_14_uniform_rgba_8192() {
    let original = Image::filled(SIDE, SIDE, a_rgba()).unwrap();
    let fills = Image::filled(SIDE, SIDE, c_rgba()).unwrap();
    let doc = Document::new(original.clone());
    fills_the_whole_image(doc, &original, &fills, c_rgba(), "RGBA");
}

/// Fila 14: lo mismo en indexado, con `VmHWM` creciendo menos de 128 MiB.
#[test]
#[ignore = "mide VmHWM: ejecutar sola y en release (ver la cabecera del fichero)"]
fn row_14_uniform_indexed_8192() {
    let original = Image::filled_indexed(SIDE, SIDE, palette(), 0).unwrap();
    let fills = Image::filled_indexed(SIDE, SIDE, palette(), 1).unwrap();
    assert_eq!(original.mode(), Mode::Indexed);
    let doc = Document::new(original.clone());
    let growth = fills_the_whole_image(doc, &original, &fills, Pixel::Indexed(1), "indexado");
    assert!(
        growth < 128 * MIB,
        "VmHWM grew {} MiB (the limit is 128)",
        growth / MIB
    );
}

/// Unit 4.5, timing: a scattered region (one `B` pixel every 5 in x and y of an RGBA 8192²: about
/// 2.7 M spans, the worst case that the cap lets through) must still fill, apply, undo and redo
/// exactly. The times are printed; it only fails above 60 s. Before and after the rewrite by rows
/// (specs/hito-4-relleno.md).
#[test]
#[ignore = "mide tiempos: ejecutar sola y en release (ver la cabecera del fichero)"]
fn unit_4_5_scattered_rgba_8192() {
    let (a, b) = (a_rgba(), c_rgba());
    let mut original = Image::filled(SIDE, SIDE, a).unwrap();
    for y in (0..SIDE).step_by(5) {
        for x in (0..SIDE).step_by(5) {
            original.set_pixel(x, y, b).unwrap();
        }
    }
    let new = Pixel::Rgba(Rgba::new(1, 2, 3, 255));
    let mut doc = Document::new(original.clone());
    let id = ImageId::from_raw(0);
    let start = Instant::now();
    let t0 = Instant::now();
    let command = fill_command(&doc, id, 1, 1, new).unwrap().unwrap();
    let t_fill = t0.elapsed();
    let spans = spans_of(&command);
    let t1 = Instant::now();
    assert_eq!(doc.apply(command), Ok(true));
    let t_apply = t1.elapsed();
    let filled = doc.image(id).unwrap().clone();
    let t2 = Instant::now();
    assert_eq!(doc.undo(), Ok(()));
    let t_undo = t2.elapsed();
    assert_eq!(doc.image(id).unwrap().bytes(), original.bytes());
    let t3 = Instant::now();
    assert_eq!(doc.redo(), Ok(()));
    let t_redo = t3.elapsed();
    assert_eq!(doc.image(id).unwrap().bytes(), filled.bytes());
    let total = start.elapsed();
    println!(
        "unit 4.5 scattered RGBA ({spans} spans): fill_command {t_fill:?}, apply {t_apply:?}, \
         undo {t_undo:?}, redo {t_redo:?}, total {total:?}"
    );
    assert!(total < Duration::from_secs(60), "took {total:?}");
}
