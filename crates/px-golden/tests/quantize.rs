//! Row 12 of specs/hito-4-cuantizador.md (unit 4.2): the quantizer against the reference, the
//! quantizer of Aseprite, over the six images of `specs/corpus-hito-4-cuantizador.md`.
//!
//! The reference PNGs live in `golden/aseprite-quant/` and are made by
//! `tools/corpus-h4/generate.sh` (the user runs it; the CI does not). Until they exist the test is
//! `#[ignore]`d; run it with `cargo test -p px-golden --test quantize -- --include-ignored`. Once
//! they are committed, remove the `ignore`.
//!
//! The criterion (decision D8): for each case, number of colors (16 and 64) and dither (none,
//! Floyd–Steinberg, ordered), the mean absolute error per channel of our result against the source
//! is at most that of the reference times 1.10 plus half a level.

use std::fs;
use std::path::PathBuf;

use px_core::Mode;
use px_formats::read_png;
use px_raster::{Dither, PaletteSource, QuantizeOptions, mean_abs_error, quantize_cels};

const CASES: [&str; 6] = ["degradado", "anillos", "manchas", "alfa", "grises", "mixto"];
const DITHERS: [(&str, Dither); 3] = [
    ("none", Dither::None),
    ("floyd", Dither::FloydSteinberg),
    ("ordered", Dither::Ordered),
];

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../golden/aseprite-quant")
}

fn read(name: &str) -> px_core::Image {
    let path = dir().join(name);
    let bytes = fs::read(&path).unwrap_or_else(|error| {
        panic!(
            "falta {}: {error}. Genera el corpus con tools/corpus-h4/generate.sh \
             (specs/corpus-hito-4-cuantizador.md)",
            path.display()
        )
    });
    read_png(&bytes).expect("png").image
}

#[test]
fn row_12_the_error_is_not_worse_than_the_reference_plus_the_margin() {
    let mut report = Vec::new();
    let mut failures = Vec::new();
    for case in CASES {
        let source = read(&format!("{case}.src.png"));
        assert_eq!(source.mode(), Mode::Rgba, "{case}");
        for colors in [16u16, 64] {
            for (label, dither) in DITHERS {
                let reference = read(&format!("{case}-n{colors}-{label}.png"));
                assert_eq!(reference.mode(), Mode::Indexed, "{case} {colors} {label}");
                let options = QuantizeOptions {
                    colors,
                    dither,
                    alpha_threshold: 128,
                    palette: PaletteSource::Computed,
                };
                let ours = quantize_cels(
                    &[Some(&source)],
                    (source.width(), source.height()),
                    Mode::Rgba,
                    &options,
                )
                .expect("quantize");
                let ours_milli = mean_abs_error(&source, ours.images[0].as_ref().unwrap(), 128)
                    .map_or(0, |e| e.mae_milli());
                let reference_milli =
                    mean_abs_error(&source, &reference, 128).map_or(0, |e| e.mae_milli());
                // Reference times 1.10 plus half a level (500 thousandths).
                let limit = reference_milli * 110 / 100 + 500;
                report.push(format!(
                    "{case} n{colors} {label}: ours {ours_milli} reference {reference_milli} limit {limit}"
                ));
                if ours_milli > limit {
                    failures.push(format!(
                        "{case} n{colors} {label}: ours {ours_milli} > limit {limit} (reference {reference_milli})"
                    ));
                }
            }
        }
    }
    // The whole report first, then the verdict: one run shows every case that is over the limit.
    println!("{}", report.join("\n"));
    assert!(
        failures.is_empty(),
        "{} cases over the limit:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
