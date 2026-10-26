//! Tests of the color quantizer (specs/hito-4-cuantizador.md, unit 4.2): rows 1 to 7 of «Casos» and
//! properties 1 to 3. Row 8 and 9 live in `px-core/tests/quantize_all.rs`; the document-level
//! property 4 in `quantize_doc.rs`; row 12 (the reference) in `px-golden`.
//!
//! Red phase: `quantize_cels`, `mean_abs_error` and their types do not exist yet, so this file does
//! not compile until the logic of `px-raster`.

use proptest::prelude::*;
use px_core::{Allowed, Image, Mode, Palette, Pixel, Rgba, convert_cels};
use px_raster::{
    Dither, PaletteSource, QuantizeError, QuantizeOptions, Quantized, mean_abs_error, quantize_cels,
};

// ---------------------------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------------------------

fn opts(colors: u16) -> QuantizeOptions {
    QuantizeOptions {
        colors,
        dither: Dither::None,
        alpha_threshold: 128,
        palette: PaletteSource::Computed,
    }
}

fn fixed(palette: Palette, dither: Dither) -> QuantizeOptions {
    QuantizeOptions {
        colors: 256,
        dither,
        alpha_threshold: 128,
        palette: PaletteSource::Fixed(palette),
    }
}

fn rgba_image(w: u32, h: u32, f: impl Fn(u32, u32) -> [u8; 4]) -> Image {
    let mut image = Image::new(w, h, Mode::Rgba).expect("image");
    for y in 0..h {
        for x in 0..w {
            let [r, g, b, a] = f(x, y);
            image
                .set_pixel(x, y, Pixel::Rgba(Rgba::new(r, g, b, a)))
                .expect("pixel");
        }
    }
    image
}

fn quant(image: &Image, options: &QuantizeOptions) -> Result<Quantized, QuantizeError> {
    quantize_cels(
        &[Some(image)],
        (image.width(), image.height()),
        image.mode(),
        options,
    )
}

fn index_at(q: &Quantized, x: u32, y: u32) -> u8 {
    match q.images[0].as_ref().expect("cell").get_pixel(x, y) {
        Ok(Pixel::Indexed(i)) => i,
        other => panic!("not an indexed pixel: {other:?}"),
    }
}

fn color_at(q: &Quantized, x: u32, y: u32) -> Rgba {
    q.palette.get(index_at(q, x, y)).expect("entry")
}

fn mae_milli(original: &Image, q: &Quantized, threshold: u8) -> u64 {
    mean_abs_error(original, q.images[0].as_ref().expect("cell"), threshold)
        .expect("opaque pixels")
        .mae_milli()
}

fn rgb_of(pixel: Pixel) -> ([u8; 3], u8) {
    match pixel {
        Pixel::Rgba(c) => ([c.r, c.g, c.b], c.a),
        Pixel::GrayAlpha { gray, alpha } => ([gray, gray, gray], alpha),
        other => panic!("unexpected pixel {other:?}"),
    }
}

fn sq_dist(a: [u8; 3], b: [u8; 3]) -> u32 {
    a.iter()
        .zip(b)
        .map(|(x, y)| {
            let d = i32::from(*x) - i32::from(y);
            (d * d) as u32
        })
        .sum()
}

/// Squared distance from `c` to the closest opaque entry of `palette` (the transparent one skipped).
fn best_dist(palette: &Palette, c: [u8; 3]) -> u32 {
    let skip = palette.transparent_index();
    palette
        .colors()
        .iter()
        .enumerate()
        .filter(|(i, _)| Some(*i as u8) != skip)
        .map(|(_, e)| sq_dist([e.r, e.g, e.b], c))
        .min()
        .expect("an opaque entry")
}

/// The chosen entry is at most 6 levels (Euclidean) farther than the best one: the mapping looks
/// colors up in a table of 6 bits per channel (decision D5).
fn assert_near_best(q: &Quantized, original: &Image) {
    let cell = q.images[0].as_ref().expect("cell");
    for y in 0..original.height() {
        for x in 0..original.width() {
            let (rgb, alpha) = rgb_of(original.get_pixel(x, y).expect("pixel"));
            if alpha < 128 {
                continue;
            }
            let Ok(Pixel::Indexed(i)) = cell.get_pixel(x, y) else {
                panic!("indexed")
            };
            let e = q.palette.get(i).expect("entry");
            let chosen = f64::from(sq_dist([e.r, e.g, e.b], rgb)).sqrt();
            let best = f64::from(best_dist(&q.palette, rgb)).sqrt();
            assert!(chosen <= best + 6.0, "({x},{y}): {chosen} vs best {best}");
        }
    }
}

/// The oracle: the uniform grid of at most `n` cells (levels per channel grown round-robin).
fn grid_palette(n: usize) -> Vec<[u8; 3]> {
    let mut l = [1usize; 3];
    loop {
        let mut grew = false;
        for c in 0..3 {
            let mut t = l;
            t[c] += 1;
            if t[0] * t[1] * t[2] <= n {
                l = t;
                grew = true;
            }
        }
        if !grew {
            break;
        }
    }
    let level = |count: usize, i: usize| -> u8 {
        if count == 1 {
            128
        } else {
            (i * 255 / (count - 1)) as u8
        }
    };
    let mut colors = Vec::new();
    for r in 0..l[0] {
        for g in 0..l[1] {
            for b in 0..l[2] {
                colors.push([level(l[0], r), level(l[1], g), level(l[2], b)]);
            }
        }
    }
    colors
}

/// Mean absolute error (per channel, in thousandths of a level) of the best the grid can do.
fn grid_mae_milli(original: &Image, n: usize, threshold: u8) -> u64 {
    let grid = grid_palette(n);
    let (mut sum, mut pixels) = (0u64, 0u64);
    for y in 0..original.height() {
        for x in 0..original.width() {
            let (rgb, alpha) = rgb_of(original.get_pixel(x, y).expect("pixel"));
            if alpha < threshold {
                continue;
            }
            let best = grid
                .iter()
                .min_by_key(|g| sq_dist(**g, rgb))
                .expect("grid");
            sum += rgb
                .iter()
                .zip(best)
                .map(|(a, b)| u64::from(a.abs_diff(*b)))
                .sum::<u64>();
            pixels += 1;
        }
    }
    sum * 1000 / (3 * pixels)
}

fn palette_of(colors: &[Rgba], transparent: Option<u8>) -> Palette {
    let mut palette = Palette::from_colors(colors.iter().copied()).expect("palette");
    palette.set_transparent_index(transparent).expect("transparent");
    palette
}

// ---------------------------------------------------------------------------------------------
// Row 1: exact when the colors fit
// ---------------------------------------------------------------------------------------------

const SIX: [[u8; 4]; 6] = [
    [255, 0, 0, 255],
    [0, 255, 0, 255],
    [0, 0, 255, 255],
    [255, 255, 0, 255],
    [10, 20, 30, 255],
    [200, 100, 50, 255],
];

fn six_colors(transparent_at_origin: bool) -> Image {
    rgba_image(5, 4, |x, y| {
        if transparent_at_origin && (x, y) == (0, 0) {
            [9, 9, 9, 0]
        } else {
            SIX[((x + y * 5) % 6) as usize]
        }
    })
}

fn convert_all_of(image: &Image) -> px_core::CelsConversion {
    convert_cels(
        &[Some(image)],
        (image.width(), image.height()),
        image.mode(),
        Mode::Indexed,
        Allowed::NOTHING,
    )
    .expect("conversion")
}

#[test]
fn row_01_when_the_colors_fit_the_result_is_the_one_of_convert_all() {
    for transparent in [false, true] {
        let image = six_colors(transparent);
        let needed = 6 + usize::from(transparent);
        let exact = convert_all_of(&image);
        for n in [256u16, 16, needed as u16] {
            let q = quant(&image, &opts(n)).expect("quantize");
            assert!(q.stats.exact, "transparent {transparent}, n {n}");
            assert_eq!(q.images, exact.images, "transparent {transparent}, n {n}");
            assert_eq!(Some(&q.palette), exact.palette.as_ref());
            assert_eq!(q.stats.colors, q.palette.len());
            assert_eq!(mae_milli(&image, &q, 128), 0);
        }
        // One color short: no longer exact, never more than n entries, and an error appears.
        let short = quant(&image, &opts(needed as u16 - 1)).expect("quantize");
        assert!(!short.stats.exact);
        assert!(short.palette.len() < needed);
        assert!(mae_milli(&image, &short, 128) > 0);
    }
}

#[test]
fn row_01_dithering_does_nothing_when_the_colors_fit() {
    let image = six_colors(false);
    let plain = quant(&image, &opts(16)).expect("plain");
    for dither in [Dither::FloydSteinberg, Dither::Ordered] {
        let mut options = opts(16);
        options.dither = dither;
        let q = quant(&image, &options).expect("dithered");
        assert_eq!(q.images, plain.images, "{dither:?}");
    }
}

#[test]
fn row_01_gray_alpha_is_converted_like_rgb_gray() {
    let mut image = Image::new(4, 2, Mode::GrayAlpha).expect("image");
    for y in 0..2 {
        for x in 0..4 {
            let pixel = match (x + y) % 3 {
                0 => Pixel::GrayAlpha { gray: 10, alpha: 255 },
                1 => Pixel::GrayAlpha { gray: 200, alpha: 255 },
                _ => Pixel::GrayAlpha { gray: 77, alpha: 0 },
            };
            image.set_pixel(x, y, pixel).expect("pixel");
        }
    }
    let exact = convert_all_of(&image);
    let q = quant(&image, &opts(8)).expect("quantize");
    assert!(q.stats.exact);
    assert_eq!(q.images, exact.images);
    assert_eq!(Some(&q.palette), exact.palette.as_ref());
}

// ---------------------------------------------------------------------------------------------
// Row 2: reducing
// ---------------------------------------------------------------------------------------------

fn gradient() -> Image {
    rgba_image(64, 64, |x, y| [(x * 4) as u8, (y * 4) as u8, ((x + y) * 2) as u8, 255])
}

#[test]
fn row_02_a_gradient_of_4096_colors_is_reduced_to_at_most_n_and_no_worse_than_the_grid() {
    let image = gradient();
    for n in [2u16, 4, 16, 256] {
        let q = quant(&image, &opts(n)).expect("quantize");
        assert!(!q.stats.exact, "n {n}");
        assert!(q.palette.len() <= usize::from(n), "n {n}: {}", q.palette.len());
        assert!(q.palette.len() >= 1);
        for y in 0..64 {
            for x in 0..64 {
                assert!(usize::from(index_at(&q, x, y)) < q.palette.len());
            }
        }
        let ours = mae_milli(&image, &q, 128);
        let grid = grid_mae_milli(&image, usize::from(n), 128);
        assert!(ours <= grid, "n {n}: ours {ours} vs grid {grid}");
        assert_near_best(&q, &image);
        // Repeatable bit by bit.
        assert_eq!(quant(&image, &opts(n)).expect("again").images, q.images);
    }
}

#[test]
fn row_02_three_flat_patches_keep_their_mean_color() {
    let bases = [[200u8, 30, 30], [30, 200, 30], [30, 30, 200]];
    let image = rgba_image(30, 10, |x, y| {
        let base = bases[(x / 10) as usize];
        let jitter = ((x + y) % 3) as u8;
        [base[0] + jitter, base[1] + jitter, base[2] + jitter, 255]
    });
    let q = quant(&image, &opts(3)).expect("quantize");
    assert!(!q.stats.exact, "9 colors do not fit 3");
    assert_eq!(q.palette.len(), 3);
    for patch in 0..3u32 {
        let c = color_at(&q, patch * 10 + 5, 5);
        let base = bases[patch as usize];
        for (got, want) in [c.r, c.g, c.b].into_iter().zip(base) {
            assert!(got.abs_diff(want) <= 8, "patch {patch}: {c:?} vs {base:?}");
        }
    }
}

#[test]
fn row_02_the_error_in_the_stats_matches_the_independent_measure() {
    let image = gradient();
    for n in [4u16, 64] {
        let q = quant(&image, &opts(n)).expect("quantize");
        let measured = mae_milli(&image, &q, 128);
        let reported = q.stats.mae_milli().expect("opaque pixels");
        assert!(measured.abs_diff(reported) <= 1, "n {n}: {measured} vs {reported}");
    }
    let exact = quant(&six_colors(false), &opts(16)).expect("exact");
    assert_eq!(exact.stats.mae_milli(), Some(0));
    let transparent_only = rgba_image(2, 2, |_, _| [1, 2, 3, 0]);
    assert_eq!(quant(&transparent_only, &opts(4)).expect("all transparent").stats.mae_milli(), None);
}

// ---------------------------------------------------------------------------------------------
// Row 3: alpha threshold
// ---------------------------------------------------------------------------------------------

#[test]
fn row_03_a_pixel_is_transparent_exactly_when_its_alpha_is_below_the_threshold() {
    let alphas = [0u8, 1, 127, 128, 255];
    let image = rgba_image(5, 1, |x, _| {
        [(x * 40 + 20) as u8, (x * 10) as u8, 200 - (x * 30) as u8, alphas[x as usize]]
    });
    for threshold in [1u8, 128, 255] {
        for n in [256u16, 3] {
            let mut options = opts(n);
            options.alpha_threshold = threshold;
            let q = quant(&image, &options).expect("quantize");
            let transparent = q.palette.transparent_index();
            let any_below = alphas.iter().any(|a| *a < threshold);
            assert_eq!(transparent.is_some(), any_below, "t {threshold}, n {n}");
            for (x, alpha) in alphas.iter().enumerate() {
                let i = index_at(&q, x as u32, 0);
                assert_eq!(
                    Some(i) == transparent,
                    *alpha < threshold,
                    "t {threshold}, n {n}, x {x}"
                );
            }
            if n == 256 {
                // Exact: the opaque ones keep their RGB, and the partial alpha does not survive.
                for (x, alpha) in alphas.iter().enumerate() {
                    if *alpha >= threshold {
                        let (rgb, _) = rgb_of(image.get_pixel(x as u32, 0).unwrap());
                        let c = color_at(&q, x as u32, 0);
                        assert_eq!([c.r, c.g, c.b, c.a], [rgb[0], rgb[1], rgb[2], 255]);
                    }
                }
            }
        }
    }
}

#[test]
fn row_03_gray_alpha_follows_the_same_threshold() {
    let mut image = Image::new(3, 1, Mode::GrayAlpha).expect("image");
    for (x, alpha) in [10u8, 127, 128].into_iter().enumerate() {
        image
            .set_pixel(x as u32, 0, Pixel::GrayAlpha { gray: 50 + alpha / 2, alpha })
            .unwrap();
    }
    let q = quant(&image, &opts(256)).expect("quantize");
    let transparent = q.palette.transparent_index().expect("two are below 128");
    assert_eq!(index_at(&q, 0, 0), transparent);
    assert_eq!(index_at(&q, 1, 0), transparent);
    assert_ne!(index_at(&q, 2, 0), transparent);
}

// ---------------------------------------------------------------------------------------------
// Row 4: dithering
// ---------------------------------------------------------------------------------------------

/// The 8 × 8 Bayer matrix (recursive construction M(2n) = [[4M, 4M+2], [4M+3, 4M+1]]).
const BAYER: [[u8; 8]; 8] = [
    [0, 32, 8, 40, 2, 34, 10, 42],
    [48, 16, 56, 24, 50, 18, 58, 26],
    [12, 44, 4, 36, 14, 46, 6, 38],
    [60, 28, 52, 20, 62, 30, 54, 22],
    [3, 35, 11, 43, 1, 33, 9, 41],
    [51, 19, 59, 27, 49, 17, 57, 25],
    [15, 47, 7, 39, 13, 45, 5, 37],
    [63, 31, 55, 23, 61, 29, 53, 21],
];

fn black_and_white() -> Palette {
    palette_of(&[Rgba::new(0, 0, 0, 255), Rgba::new(255, 255, 255, 255)], None)
}

#[test]
fn row_04_ordered_dithering_of_a_flat_gray_draws_the_bayer_pattern() {
    let image = rgba_image(16, 16, |_, _| [128, 128, 128, 255]);
    let q = quant(&image, &fixed(black_and_white(), Dither::Ordered)).expect("quantize");
    let mut white = 0;
    for y in 0..16u32 {
        for x in 0..16u32 {
            let expected = u8::from(BAYER[(y % 8) as usize][(x % 8) as usize] >= 32);
            assert_eq!(index_at(&q, x, y), expected, "({x},{y})");
            white += u32::from(expected);
        }
    }
    assert_eq!(white, 4 * 32, "half of every 8 x 8 tile is white");
    assert_eq!(
        quant(&image, &fixed(black_and_white(), Dither::Ordered)).unwrap().images,
        q.images,
        "deterministic"
    );
}

fn four_grays() -> Palette {
    palette_of(
        &[
            Rgba::new(0, 0, 0, 255),
            Rgba::new(85, 85, 85, 255),
            Rgba::new(170, 170, 170, 255),
            Rgba::new(255, 255, 255, 255),
        ],
        None,
    )
}

/// Sum over the 4 x 4 blocks of |sum of original - sum of quantized| (gray ramp images).
fn block_error(original: &Image, q: &Quantized) -> u64 {
    let mut total = 0;
    for by in 0..original.height() / 4 {
        for bx in 0..original.width() / 4 {
            let (mut a, mut b) = (0i64, 0i64);
            for y in by * 4..by * 4 + 4 {
                for x in bx * 4..bx * 4 + 4 {
                    a += i64::from(rgb_of(original.get_pixel(x, y).unwrap()).0[0]);
                    b += i64::from(color_at(q, x, y).r);
                }
            }
            total += a.abs_diff(b);
        }
    }
    total
}

#[test]
fn row_04_error_diffusion_lowers_the_error_of_a_gray_ramp_in_blocks() {
    let image = rgba_image(64, 16, |x, _| {
        let v = (x * 4) as u8;
        [v, v, v, 255]
    });
    let none = quant(&image, &fixed(four_grays(), Dither::None)).expect("none");
    let diffused = quant(&image, &fixed(four_grays(), Dither::FloydSteinberg)).expect("fs");
    let ordered = quant(&image, &fixed(four_grays(), Dither::Ordered)).expect("ordered");
    let (e_none, e_fs, e_ord) = (
        block_error(&image, &none),
        block_error(&image, &diffused),
        block_error(&image, &ordered),
    );
    assert!(e_fs * 2 < e_none, "diffusion {e_fs} vs none {e_none}");
    assert!(e_ord < e_none, "ordered {e_ord} vs none {e_none}");
    assert_eq!(
        quant(&image, &fixed(four_grays(), Dither::FloydSteinberg)).unwrap().images,
        diffused.images,
        "deterministic"
    );
}

#[test]
fn row_04_diffused_error_does_not_cross_a_transparent_pixel() {
    let palette = palette_of(
        &[
            Rgba::new(0, 0, 0, 0),
            Rgba::new(0, 0, 0, 255),
            Rgba::new(255, 255, 255, 255),
        ],
        Some(0),
    );
    let image = rgba_image(3, 1, |x, _| if x == 1 { [0, 0, 0, 0] } else { [100, 100, 100, 255] });
    let q = quant(&image, &fixed(palette, Dither::FloydSteinberg)).expect("quantize");
    assert_eq!(index_at(&q, 0, 0), 1, "100 is closer to black");
    assert_eq!(index_at(&q, 1, 0), 0, "the transparent entry");
    assert_eq!(index_at(&q, 2, 0), 1, "no error arrives through the transparent pixel");
}

// ---------------------------------------------------------------------------------------------
// Row 5: foreign palette
// ---------------------------------------------------------------------------------------------

#[test]
fn row_05_a_fixed_palette_is_the_palette_of_the_result_and_pixels_take_the_closest_entry() {
    let given = palette_of(
        &[
            Rgba::new(0, 0, 0, 255),
            Rgba::new(255, 255, 255, 255),
            Rgba::new(255, 0, 0, 255),
        ],
        None,
    );
    let image = rgba_image(2, 2, |x, y| match (x, y) {
        (0, 0) => [255, 0, 0, 255],
        (1, 0) => [10, 10, 10, 255],
        (0, 1) => [250, 250, 250, 255],
        _ => [240, 20, 20, 255],
    });
    let mut options = fixed(given.clone(), Dither::None);
    options.colors = 1; // ignored with a fixed palette
    let q = quant(&image, &options).expect("quantize");
    assert_eq!(q.palette, given);
    assert_eq!(
        [index_at(&q, 0, 0), index_at(&q, 1, 0), index_at(&q, 0, 1), index_at(&q, 1, 1)],
        [2, 0, 1, 2]
    );
}

#[test]
fn row_05_a_fixed_palette_with_a_transparent_entry_maps_transparent_pixels_to_it() {
    let given = palette_of(
        &[
            Rgba::new(0, 0, 0, 255),
            Rgba::new(255, 255, 255, 255),
            Rgba::new(255, 0, 0, 255),
            Rgba::new(0, 0, 0, 0),
        ],
        Some(3),
    );
    let image = rgba_image(2, 1, |x, _| if x == 0 { [5, 5, 5, 0] } else { [5, 5, 5, 255] });
    let q = quant(&image, &fixed(given.clone(), Dither::None)).expect("quantize");
    assert_eq!(q.palette, given);
    assert_eq!(index_at(&q, 0, 0), 3);
    assert_eq!(index_at(&q, 1, 0), 0);
}

#[test]
fn row_05_a_fixed_palette_without_a_transparent_entry_fails_when_one_is_needed() {
    let image = rgba_image(2, 1, |x, _| if x == 0 { [5, 5, 5, 0] } else { [5, 5, 5, 255] });
    assert_eq!(
        quant(&image, &fixed(black_and_white(), Dither::None)).unwrap_err(),
        QuantizeError::FixedNeedsTransparent
    );
    // Several cells need one even without transparent pixels.
    let opaque = rgba_image(2, 1, |_, _| [5, 5, 5, 255]);
    assert_eq!(
        quantize_cels(
            &[Some(&opaque), None],
            (2, 1),
            Mode::Rgba,
            &fixed(black_and_white(), Dither::None)
        )
        .unwrap_err(),
        QuantizeError::FixedNeedsTransparent
    );
}

// ---------------------------------------------------------------------------------------------
// Row 6: several layers
// ---------------------------------------------------------------------------------------------

#[test]
fn row_06_several_cells_share_one_palette_with_a_transparent_entry() {
    let a = rgba_image(8, 8, |x, y| [(x * 30) as u8, (y * 30) as u8, 40, 255]);
    let b = rgba_image(8, 8, |x, y| [200, (x * 25) as u8, (y * 25) as u8, 255]);
    let q = quantize_cels(&[Some(&a), None, Some(&b)], (8, 8), Mode::Rgba, &opts(16))
        .expect("quantize");
    assert_eq!(q.images.len(), 3);
    assert!(q.images[1].is_none(), "an empty cell stays empty");
    assert!(q.palette.len() <= 16);
    let transparent = q.palette.transparent_index().expect("several cells need one");
    for (cell, original) in [(0usize, &a), (2, &b)] {
        let image = q.images[cell].as_ref().expect("cell");
        assert_eq!(image.palette(), Some(&q.palette));
        for y in 0..8 {
            for x in 0..8 {
                let Ok(Pixel::Indexed(i)) = image.get_pixel(x, y) else {
                    panic!("indexed")
                };
                assert_ne!(i, transparent, "opaque pixels never use the transparent entry");
                let (rgb, _) = rgb_of(original.get_pixel(x, y).unwrap());
                let e = q.palette.get(i).unwrap();
                let chosen = f64::from(sq_dist([e.r, e.g, e.b], rgb)).sqrt();
                let best = f64::from(best_dist(&q.palette, rgb)).sqrt();
                assert!(chosen <= best + 6.0, "cell {cell} ({x},{y})");
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Row 7: errors
// ---------------------------------------------------------------------------------------------

#[test]
fn row_07_invalid_options_and_sources_are_errors() {
    let image = six_colors(false);
    assert_eq!(quant(&image, &opts(0)).unwrap_err(), QuantizeError::InvalidColorCount);
    assert_eq!(quant(&image, &opts(257)).unwrap_err(), QuantizeError::InvalidColorCount);
    let mut zero = opts(8);
    zero.alpha_threshold = 0;
    assert_eq!(quant(&image, &zero).unwrap_err(), QuantizeError::InvalidThreshold);
    // One color and a transparent entry needed: no slot for any color.
    let with_hole = six_colors(true);
    assert_eq!(quant(&with_hole, &opts(1)).unwrap_err(), QuantizeError::NoColorSlots);
    // Sources other than RGBA and gray with alpha.
    let gray = Image::filled(2, 2, Pixel::Gray(3)).unwrap();
    assert_eq!(
        quant(&gray, &opts(8)).unwrap_err(),
        QuantizeError::SourceMode(Mode::Gray)
    );
    let palette = black_and_white();
    let indexed = Image::filled_indexed(2, 2, palette, 0).unwrap();
    assert_eq!(
        quant(&indexed, &opts(8)).unwrap_err(),
        QuantizeError::SourceMode(Mode::Indexed)
    );
}

// ---------------------------------------------------------------------------------------------
// Properties
// ---------------------------------------------------------------------------------------------

fn build(w: u32, h: u32, gray_alpha: bool, few: bool, bytes: &[u8]) -> Image {
    let mode = if gray_alpha { Mode::GrayAlpha } else { Mode::Rgba };
    let mut image = Image::new(w, h, mode).expect("image");
    for y in 0..h {
        for x in 0..w {
            let at = ((y * w + x) * 4) as usize;
            let b = &bytes[at..at + 4];
            let alpha = match b[3] % 4 {
                0 => 0,
                1 => 127,
                2 => 128,
                _ => 255,
            };
            let pixel = if gray_alpha {
                let gray = if few { (b[0] % 4) * 60 } else { b[0] };
                Pixel::GrayAlpha { gray, alpha }
            } else if few {
                let k = b[0] % 4;
                Pixel::Rgba(Rgba::new(k * 60, 255 - k * 60, k * 10, alpha))
            } else {
                Pixel::Rgba(Rgba::new(b[0], b[1], b[2], alpha))
            };
            image.set_pixel(x, y, pixel).unwrap();
        }
    }
    image
}

fn image_strategy() -> impl Strategy<Value = Image> {
    (1u32..=12, 1u32..=9, any::<bool>(), any::<bool>()).prop_flat_map(|(w, h, ga, few)| {
        proptest::collection::vec(any::<u8>(), (w * h * 4) as usize)
            .prop_map(move |bytes| build(w, h, ga, few, &bytes))
    })
}

fn dither_strategy() -> impl Strategy<Value = Dither> {
    prop_oneof![
        Just(Dither::None),
        Just(Dither::FloydSteinberg),
        Just(Dither::Ordered)
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    // Property 1: valid, deterministic, exact when the colors fit.
    #[test]
    fn property_1_the_result_is_valid_deterministic_and_exact_when_it_fits(
        image in image_strategy(),
        n in 1u16..=256,
        dither in dither_strategy(),
        threshold in 1u8..=255,
    ) {
        let options = QuantizeOptions { colors: n, dither, alpha_threshold: threshold, palette: PaletteSource::Computed };
        let mut opaque = std::collections::BTreeSet::new();
        let mut transparent = false;
        for y in 0..image.height() {
            for x in 0..image.width() {
                let (rgb, alpha) = rgb_of(image.get_pixel(x, y).unwrap());
                if alpha < threshold { transparent = true } else { opaque.insert(rgb); }
            }
        }
        let total = opaque.len() + usize::from(transparent);
        let result = quant(&image, &options);
        if n == 1 && transparent {
            prop_assert_eq!(result.unwrap_err(), QuantizeError::NoColorSlots);
            return Ok(());
        }
        let q = result.unwrap();
        prop_assert!(q.palette.len() <= usize::from(n));
        for y in 0..image.height() {
            for x in 0..image.width() {
                prop_assert!(usize::from(index_at(&q, x, y)) < q.palette.len());
            }
        }
        prop_assert_eq!(&quant(&image, &options).unwrap().images, &q.images);
        if total <= usize::from(n) {
            prop_assert!(q.stats.exact);
            if !opaque.is_empty() {
                prop_assert_eq!(mae_milli(&image, &q, threshold), 0);
            }
        }
    }

    // Property 2: transparent exactly below the threshold.
    #[test]
    fn property_2_a_pixel_is_transparent_exactly_below_the_threshold(
        image in image_strategy(),
        n in 2u16..=256,
        dither in dither_strategy(),
        threshold in 1u8..=255,
    ) {
        let options = QuantizeOptions { colors: n, dither, alpha_threshold: threshold, palette: PaletteSource::Computed };
        let q = quant(&image, &options).unwrap();
        let transparent = q.palette.transparent_index();
        let mut any_below = false;
        for y in 0..image.height() {
            for x in 0..image.width() {
                let (_, alpha) = rgb_of(image.get_pixel(x, y).unwrap());
                any_below |= alpha < threshold;
                prop_assert_eq!(Some(index_at(&q, x, y)) == transparent, alpha < threshold);
            }
        }
        prop_assert_eq!(transparent.is_some(), any_below);
    }

    // Property 3: without dithering, never worse than the uniform grid (plus one level).
    #[test]
    fn property_3_the_error_is_not_worse_than_the_uniform_grid(
        w in 4u32..=16,
        h in 4u32..=16,
        n in 2u16..=256,
        seed in proptest::collection::vec(any::<u8>(), 16 * 16 * 3),
    ) {
        let image = rgba_image(w, h, |x, y| {
            let at = ((y * w + x) * 3) as usize;
            [seed[at], seed[at + 1], seed[at + 2], 255]
        });
        let q = quant(&image, &opts(n)).unwrap();
        let ours = mae_milli(&image, &q, 128);
        let grid = grid_mae_milli(&image, usize::from(n), 128);
        prop_assert!(ours <= grid + 1000, "ours {} vs grid {}", ours, grid);
    }
}
