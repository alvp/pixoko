//! Reference of specs/hito-4-relleno.md (unit 4.5): the solid fill against the paint bucket of
//! Aseprite (contiguous, tolerance 0, red from the seed of each case).
//!
//! The cases are in `golden/aseprite-fill/cases.txt` (shared with the script) and the reference
//! PNGs are made by `tools/corpus-h4/generate-fill.sh` (the user runs it; the CI does not). Until
//! they exist `matches_aseprite` is `#[ignore]`d; run it with
//! `cargo test -p px-golden --test fill_reference -- --include-ignored`. Once they are
//! committed, remove the `ignore`. The criterion: identical, pixel by pixel. The pattern and the
//! "until the background" modes have no counterpart in Aseprite: they are our own decision (D3).

use std::fs;
use std::path::PathBuf;

use px_core::{Document, Image, ImageId, Mode, Pixel, Rgba};
use px_formats::read_png;
use px_raster::flood_fill;

const SIZE: u32 = 16;
const NAMES: [&str; 6] = ["uniform", "ring", "diagonal", "stripes", "gap", "islands"];

struct Case {
    name: String,
    seed: (i32, i32),
    image: Image,
}

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../golden/aseprite-fill")
}

fn cases() -> Vec<Case> {
    let text = fs::read_to_string(dir().join("cases.txt")).expect("cases.txt");
    let mut out = Vec::new();
    let mut lines = text.lines().peekable();
    while let Some(line) = lines.next() {
        let Some(rest) = line.strip_prefix("# case ") else {
            continue;
        };
        let mut parts = rest.split(' ');
        let name = parts.next().expect("name").to_string();
        let x: i32 = parts.next().expect("x").parse().expect("number");
        let y: i32 = parts.next().expect("y").parse().expect("number");
        let mut image = Image::new(SIZE, SIZE, Mode::Rgba).expect("image");
        for fy in 0..SIZE {
            let row = lines.next().expect("16 rows");
            assert_eq!(row.len(), SIZE as usize, "{name}: row {fy}");
            for (fx, ch) in row.chars().enumerate() {
                let color = match ch {
                    '.' => Rgba::TRANSPARENT,
                    'B' => Rgba::opaque(0, 0, 255),
                    'G' => Rgba::opaque(0, 255, 0),
                    other => panic!("{name}: unknown cell {other:?}"),
                };
                image
                    .set_pixel(fx as u32, fy, Pixel::Rgba(color))
                    .expect("inside");
            }
        }
        out.push(Case {
            name,
            seed: (x, y),
            image,
        });
    }
    out
}

#[test]
fn the_cases_file_is_well_formed() {
    let cases = cases();
    let names: Vec<&str> = cases.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, NAMES);
    for case in &cases {
        let (x, y) = case.seed;
        assert!(
            (0..SIZE as i32).contains(&x) && (0..SIZE as i32).contains(&y),
            "{}",
            case.name
        );
    }
}

#[test]
#[ignore = "needs golden/aseprite-fill/*.png (tools/corpus-h4/generate-fill.sh)"]
fn matches_aseprite() {
    for case in cases() {
        let mut doc = Document::new(case.image.clone());
        let red = Pixel::Rgba(Rgba::opaque(255, 0, 0));
        flood_fill(
            &mut doc,
            ImageId::from_raw(0),
            case.seed.0,
            case.seed.1,
            red,
        )
        .expect("fill");
        let ours = doc.image(ImageId::from_raw(0)).expect("image");
        let path = dir().join(format!("{}.png", case.name));
        let bytes = fs::read(&path).unwrap_or_else(|error| {
            panic!(
                "falta {}: {error}. Genera el corpus con tools/corpus-h4/generate-fill.sh",
                path.display()
            )
        });
        let reference = read_png(&bytes).expect("png").image;
        for y in 0..SIZE {
            for x in 0..SIZE {
                // Transparent pixels may carry any RGB in the PNG: compare them by alpha only.
                let (a, b) = (
                    ours.get_pixel(x, y).unwrap(),
                    reference.get_pixel(x, y).unwrap(),
                );
                let same = match (a, b) {
                    (Pixel::Rgba(a), Pixel::Rgba(b)) => a == b || (a.a == 0 && b.a == 0),
                    _ => false,
                };
                assert!(
                    same,
                    "{}: ({x}, {y}) is {a:?}, Aseprite gives {b:?}",
                    case.name
                );
            }
        }
    }
}
