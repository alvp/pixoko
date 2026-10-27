//! Measurement of the quantizer at 8192 x 8192 RGBA (specs/hito-4-cuantizador.md, D10). Not a gate
//! of the CI: `cargo test --release -p px-raster --test quantize_perf -- --ignored --nocapture`.
//!
//! Red phase: `quantize_cels` does not exist yet.

use std::time::Instant;

use px_core::{Image, Mode};
use px_raster::{Dither, PaletteSource, QuantizeOptions, quantize_cels};

/// A smooth picture with some noise, made of bytes (no per-pixel calls): 8192 x 8192 RGBA.
fn big(seed: u32) -> Image {
    let side = 8192u32;
    let mut data = vec![0u8; (side as usize) * (side as usize) * 4];
    let mut state = seed.wrapping_mul(2_654_435_761).wrapping_add(1);
    for (i, px) in data.as_chunks_mut::<4>().0.iter_mut().enumerate() {
        let (x, y) = ((i as u32) % side, (i as u32) / side);
        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let noise = (state >> 28) as u8;
        px[0] = ((x >> 5) as u8).wrapping_add(noise);
        px[1] = ((y >> 5) as u8).wrapping_add(noise);
        px[2] = (((x + y) >> 6) as u8).wrapping_add(noise);
        px[3] = 255;
    }
    Image::from_bytes(side, side, Mode::Rgba, data).expect("image")
}

#[test]
#[ignore = "measurement at 8192 x 8192; run in release with --ignored"]
fn measure_quantize_at_8192_squared_with_one_and_four_layers() {
    for layers in [1usize, 4] {
        let images: Vec<Image> = (0..layers as u32).map(big).collect();
        let cels: Vec<Option<&Image>> = images.iter().map(Some).collect();
        for dither in [Dither::None, Dither::FloydSteinberg, Dither::Ordered] {
            let options = QuantizeOptions {
                colors: 64,
                dither,
                alpha_threshold: 128,
                palette: PaletteSource::Computed,
            };
            let start = Instant::now();
            let q = quantize_cels(&cels, (8192, 8192), Mode::Rgba, &options).expect("quantize");
            let seconds = start.elapsed().as_secs_f64();
            println!(
                "{layers} layer(s), {dither:?}: {seconds:.2} s, {} colors",
                q.palette.len()
            );
            assert!(seconds < 120.0, "safety net, not a goal");
        }
    }
}
