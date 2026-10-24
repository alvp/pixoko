//! Time and memory of the image operations (specs/hito-4-imagen.md, row 8): **budget figures, not
//! a CI gate** (`#[ignore]`). Run by hand, in release and with one test thread:
//!
//! `cargo test --release -p px-raster --test image_ops_perf -- --ignored --nocapture --test-threads=1`
//!
//! Documents of 8192 × 8192 RGBA with 1 and 4 layers of non-uniform content. For each operation it
//! prints the time to build the command (the pixels), the time of `apply` and of `undo`, and the
//! growth of the resident memory (the peak, reset with `clear_refs`, minus what the document
//! already used). Linux only: it reads `/proc/self/status`.

use std::time::Instant;

use px_core::{BlendMode, Document, Image, LayerSpec, Mode, Pixel, Rgba};
use px_raster::{Anchor, ImageOp, image_command};

const SIDE: u32 = 8192;

fn kib(field: &str) -> usize {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|text| {
            text.lines()
                .find(|line| line.starts_with(field))
                .and_then(|line| line.split_whitespace().nth(1)?.parse().ok())
        })
        .unwrap_or(0)
}

fn reset_peak() {
    let _ = std::fs::write("/proc/self/clear_refs", "5");
}

fn cel(seed: u32) -> Image {
    let bytes: Vec<u8> = (0..(SIDE as usize) * (SIDE as usize) * 4)
        .map(|i| (i as u32 ^ (i >> 11) as u32).wrapping_mul(seed) as u8)
        .collect();
    Image::from_bytes(SIDE, SIDE, Mode::Rgba, bytes).expect("cel")
}

fn document(layers: usize) -> Document {
    let specs = (0..layers)
        .map(|i| LayerSpec {
            name: format!("c{i}"),
            visible: true,
            opacity: 255,
            blend: BlendMode::Normal,
            locked: false,
            image: Some(cel(2 * i as u32 + 3)),
        })
        .collect();
    let mut doc = Document::from_layers(SIDE, SIDE, Mode::Rgba, None, specs).expect("document");
    doc.set_cells_limit(usize::MAX);
    doc
}

#[test]
#[ignore]
fn image_ops_time_and_memory_at_8192() {
    let fill = Pixel::Rgba(Rgba::TRANSPARENT);
    let ops = [
        (
            "flip horizontal (cold: first touch of the memory)",
            ImageOp::FlipH,
        ),
        ("flip vertical", ImageOp::FlipV),
        ("turn right", ImageOp::Cw),
        (
            "canvas +1 (anchor center)",
            ImageOp::Canvas {
                width: SIDE + 1,
                height: SIDE + 1,
                anchor: Anchor::Center,
            },
        ),
        (
            "canvas half (anchor center)",
            ImageOp::Canvas {
                width: SIDE / 2,
                height: SIDE / 2,
                anchor: Anchor::Center,
            },
        ),
        ("flip horizontal (warm)", ImageOp::FlipH),
        ("turn half", ImageOp::Half),
    ];
    for layers in [1usize, 4] {
        for (name, op) in ops {
            let mut doc = document(layers);
            let base = kib("VmRSS:");
            reset_peak();
            let t = Instant::now();
            let command = image_command(&doc, op, fill).expect("command");
            let build = t.elapsed();
            let t = Instant::now();
            doc.apply(command).expect("apply");
            let apply = t.elapsed();
            let peak = kib("VmHWM:").saturating_sub(base);
            let t = Instant::now();
            doc.undo().expect("undo");
            let undo = t.elapsed();
            println!(
                "{layers} layer(s) {name}: build {build:.2?}, apply {apply:.2?}, undo {undo:.2?}, peak growth {} MiB",
                peak / 1024
            );
        }
    }
}
