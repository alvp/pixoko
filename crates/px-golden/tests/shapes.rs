//! Reference of specs/hito-4-formas.md (unit 4.4): the shapes against the ones that Aseprite draws.
//!
//! The reference PNGs live in `golden/aseprite-shapes/` and are made by
//! `tools/corpus-h4/generate-shapes.sh` (the user runs it; the CI does not). Until they exist the
//! test is `#[ignore]`d; run it with `cargo test -p px-golden --test shapes -- --include-ignored`.
//! Once they are committed, remove the `ignore`.
//!
//! The criterion (section «Referencia»): rectangles identical; circles, ellipses and polygons with
//! a symmetric difference of at most 10 % of the pixels of the reference plus 2.

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use px_core::{Pixel, Span};
use px_formats::read_png;
use px_raster::{ellipse_ring, ellipse_spans, polygon_ring, polygon_spans, rect_ring, rect_spans};

const SIZE: u32 = 64;

type Points = BTreeSet<(i32, i32)>;

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../golden/aseprite-shapes")
}

/// The non-transparent pixels of the reference.
fn reference(name: &str) -> Points {
    let path = dir().join(format!("{name}.png"));
    let bytes = fs::read(&path).unwrap_or_else(|error| {
        panic!(
            "falta {}: {error}. Genera el corpus con tools/corpus-h4/generate-shapes.sh \
             (specs/hito-4-formas.md)",
            path.display()
        )
    });
    let image = read_png(&bytes).expect("png").image;
    assert_eq!((image.width(), image.height()), (SIZE, SIZE), "{name}");
    let mut out = Points::new();
    for y in 0..SIZE {
        for x in 0..SIZE {
            if let Ok(Pixel::Rgba(c)) = image.get_pixel(x, y)
                && c.a > 0
            {
                out.insert((x as i32, y as i32));
            }
        }
    }
    out
}

fn of_spans(spans: &[Span]) -> Points {
    spans
        .iter()
        .flat_map(|s| (s.x_start..s.x_end).map(move |x| (x as i32, s.y as i32)))
        .collect()
}

fn of_ring(ring: &[(i32, i32)]) -> Points {
    ring.iter().copied().collect()
}

/// `(name, ours, exact)`.
fn cases() -> Vec<(String, Points, bool)> {
    let mut out = Vec::new();
    let corners = ((5, 5), (20, 12));
    out.push((
        "rect-outline".to_string(),
        of_ring(&rect_ring(corners.0, corners.1, SIZE, SIZE, 0)),
        true,
    ));
    out.push((
        "rect-filled".to_string(),
        of_spans(&rect_spans(corners.0, corners.1, SIZE, SIZE)),
        true,
    ));
    let mut ellipse = |name: String, a: u32, b: u32| {
        out.push((
            format!("{name}-outline"),
            of_ring(&ellipse_ring((32, 32), a, b, SIZE, SIZE, 0)),
            false,
        ));
        out.push((
            format!("{name}-filled"),
            of_spans(&ellipse_spans((32, 32), a, b, SIZE, SIZE)),
            false,
        ));
    };
    for r in [1u32, 2, 3, 5, 10] {
        ellipse(format!("circle-r{r}"), r, r);
    }
    for (a, b) in [(3u32, 2u32), (5, 1), (2, 4), (12, 7), (20, 9)] {
        ellipse(format!("ellipse-{a}x{b}"), a, b);
    }
    let polygons: [(&str, &[(i32, i32)]); 4] = [
        ("triangle", &[(5, 5), (40, 5), (5, 40)]),
        (
            "notch",
            &[
                (5, 5),
                (20, 5),
                (20, 30),
                (35, 30),
                (35, 5),
                (50, 5),
                (50, 50),
                (5, 50),
            ],
        ),
        ("bowtie", &[(5, 5), (50, 5), (5, 50), (50, 50)]),
        ("star", &[(32, 3), (50, 58), (3, 22), (61, 22), (14, 58)]),
    ];
    for (name, vertices) in polygons {
        out.push((
            format!("poly-{name}-outline"),
            of_ring(&polygon_ring(vertices, SIZE, SIZE, 0)),
            false,
        ));
        out.push((
            format!("poly-{name}-filled"),
            of_spans(&polygon_spans(vertices, SIZE, SIZE)),
            false,
        ));
    }
    out
}

#[test]
#[ignore = "needs golden/aseprite-shapes/ (tools/corpus-h4/generate-shapes.sh)"]
fn reference_the_shapes_match_aseprite_within_the_margin() {
    let all = cases();
    assert_eq!(all.len(), 30, "lista exacta de casos");
    let mut report = Vec::new();
    let mut failures = Vec::new();
    for (name, ours, exact) in all {
        let theirs = reference(&name);
        let diff = ours.symmetric_difference(&theirs).count();
        report.push(format!(
            "{name}: ours {} reference {} difference {diff}",
            ours.len(),
            theirs.len()
        ));
        let allowed = if exact { 0 } else { theirs.len() / 10 + 2 };
        if diff > allowed {
            failures.push(format!("{name}: difference {diff} > {allowed}"));
        }
    }
    println!("{}", report.join("\n"));
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn the_list_of_cases_is_the_one_of_the_script() {
    let names: Vec<String> = cases().into_iter().map(|c| c.0).collect();
    assert_eq!(names.len(), 30);
    let unique: BTreeSet<&String> = names.iter().collect();
    assert_eq!(unique.len(), 30, "sin repetidos");
    for expected in [
        "rect-outline",
        "circle-r10-filled",
        "ellipse-20x9-outline",
        "poly-star-filled",
    ] {
        assert!(names.iter().any(|n| n == expected), "{expected}");
    }
}
