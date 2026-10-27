//! Rows PC-07 to PC-11 and property D of specs/hito-4-paleta.md (unit 4.6): selection with
//! `Shift` and `Ctrl`, gradient, default palette, palette files and replacing or swapping a
//! color in the whole image. Figures computed by hand from the spec.
//!
//! These tests do not compile until `px_ui::palette_tools` and `px_ui::palette_file` exist.

use proptest::prelude::*;
use px_core::{
    BlendMode, Command, Document, Image, LayerSpec, Mode, Palette, Pixel, Rgba,
};
use px_ui::palette_file::{PaletteFormat, parse_palette, write_palette};
use px_ui::palette_tools::{
    default_palette, gradient_colors, replace_color_command, selected_entries, swap_colors_command,
    swap_order,
};

#[test]
fn row_pc07_selection_range_extras_and_active() {
    // Active 4, range to 2: 2, 3, 4.
    assert_eq!(selected_entries(4, Some(2), &[]), vec![2, 3, 4]);
    // Ctrl adds an entry outside the range and the active one is always in.
    assert_eq!(selected_entries(4, Some(2), &[7]), vec![2, 3, 4, 7]);
    // Ctrl on an entry inside the range removes it (XOR) but never the active one.
    assert_eq!(selected_entries(4, Some(2), &[3]), vec![2, 4]);
    assert_eq!(selected_entries(4, Some(2), &[4]), vec![2, 3, 4]);
    // Without range: the active one and the extras, in order and without repeating.
    assert_eq!(selected_entries(5, None, &[1, 9]), vec![1, 5, 9]);
    assert_eq!(selected_entries(5, None, &[]), vec![5]);
}

#[test]
fn row_pc08_gradient_between_two_entries() {
    let black = Rgba::new(0, 0, 0, 255);
    let white = Rgba::new(255, 255, 255, 255);
    let ramp: Vec<u8> = gradient_colors(black, white, 4).iter().map(|c| c.r).collect();
    assert_eq!(ramp, vec![0, 64, 128, 191, 255]);
    let faded = gradient_colors(Rgba::new(10, 20, 30, 0), Rgba::new(10, 20, 30, 255), 3);
    let alphas: Vec<u8> = faded.iter().map(|c| c.a).collect();
    assert_eq!(alphas, vec![0, 85, 170, 255]);
    assert!(faded.iter().all(|c| (c.r, c.g, c.b) == (10, 20, 30)));
}

#[test]
fn row_pc09_default_palette_has_256_known_entries() {
    let p = default_palette();
    assert_eq!(p.len(), 256);
    assert_eq!(p[16], Rgba::new(0, 0, 0, 255));
    assert_eq!(p[21], Rgba::new(0, 0, 255, 255));
    assert_eq!(p[196], Rgba::new(255, 0, 0, 255));
    assert_eq!(p[231], Rgba::new(255, 255, 255, 255));
    assert_eq!(p[232], Rgba::new(8, 8, 8, 255));
    assert_eq!(p[255], Rgba::new(238, 238, 238, 255));
}

#[test]
fn row_pc10_palette_files_read_write_and_errors() {
    let jasc = "JASC-PAL\r\n0100\r\n2\r\n255 0 0\r\n0 0 255\r\n";
    let gpl = "GIMP Palette\nName: x\nColumns: 4\n# c\n255   0   0 Red\n  0   0 255 Blue\n";
    let hex = "# comment\nFF0000\n#0000ff\n\n";
    let red_blue = vec![Rgba::new(255, 0, 0, 255), Rgba::new(0, 0, 255, 255)];
    for text in [jasc, gpl, hex] {
        let loaded = parse_palette(text).expect(text);
        assert_eq!(loaded.colors, red_blue);
        assert_eq!(loaded.ignored, 0);
    }
    for format in [PaletteFormat::Pal, PaletteFormat::Gpl, PaletteFormat::Hex] {
        assert_eq!(parse_palette(&write_palette(&red_blue, format)).unwrap().colors, red_blue);
    }
    assert!(parse_palette("").is_err());
    assert!(parse_palette("JASC-PAL\n0100\n2\n1 2 3\n").is_err());
    assert!(parse_palette("JASC-PAL\n0100\n1\n1 2 300\n").is_err());
    assert!(parse_palette("GIMP Palette\n").is_err());
    assert!(parse_palette("zzzzzz\n").is_err());
    let many: String = (0..300).map(|i| format!("{:06X}\n", i)).collect();
    let loaded = parse_palette(&many).unwrap();
    assert_eq!((loaded.colors.len(), loaded.ignored), (256, 44));
    assert_eq!(PaletteFormat::from_extension("GPL"), Some(PaletteFormat::Gpl));
    assert_eq!(PaletteFormat::from_extension("png"), None);
}

fn indexed_doc(layers: &[Vec<u8>]) -> Document {
    let palette = Palette::from_colors((0..4).map(|i| Rgba::new(i * 60, 0, 0, 255))).unwrap();
    let specs = layers
        .iter()
        .map(|pixels| {
            let mut img = Image::new_indexed(2, 2, palette.clone()).unwrap();
            for (i, &v) in pixels.iter().enumerate() {
                img.set_pixel(i as u32 % 2, i as u32 / 2, Pixel::Indexed(v)).unwrap();
            }
            LayerSpec {
                name: "L".into(),
                visible: true,
                opacity: 255,
                blend: BlendMode::Normal,
                locked: false,
                image: Some(img),
            }
        })
        .collect();
    Document::from_layers(2, 2, Mode::Indexed, Some(palette), specs).unwrap()
}

fn cels(doc: &Document) -> Vec<Vec<u8>> {
    doc.layers()
        .iter()
        .map(|l| {
            let img = doc.image(l.cel()).unwrap();
            (0..4)
                .map(|i| match img.get_pixel(i % 2, i / 2).unwrap() {
                    Pixel::Indexed(v) => v,
                    other => panic!("{other:?}"),
                })
                .collect()
        })
        .collect()
}

#[test]
fn row_pc11_replace_and_swap_color_in_every_cell() {
    let mut d = indexed_doc(&[vec![0, 1, 2, 1], vec![1, 1, 3, 0]]);
    let command = replace_color_command(&d, Pixel::Indexed(1), Pixel::Indexed(3)).unwrap().unwrap();
    assert!(matches!(command, Command::ReplaceImages { .. }));
    assert_eq!(d.apply(command), Ok(true));
    assert_eq!(cels(&d), vec![vec![0, 3, 2, 3], vec![3, 3, 3, 0]]);
    d.undo().unwrap();
    assert_eq!(cels(&d), vec![vec![0, 1, 2, 1], vec![1, 1, 3, 0]]);
    let swap = swap_colors_command(&d, Pixel::Indexed(1), Pixel::Indexed(3)).unwrap().unwrap();
    assert_eq!(d.apply(swap), Ok(true));
    assert_eq!(cels(&d), vec![vec![0, 3, 2, 3], vec![3, 3, 1, 0]]);
    // Same color on both sides: nothing to do.
    assert!(replace_color_command(&d, Pixel::Indexed(2), Pixel::Indexed(2)).unwrap().is_none());
    // RGBA: exact match of the whole pixel.
    let mut img = Image::new(2, 1, Mode::Rgba).unwrap();
    let (a, b) = (Rgba::new(9, 9, 9, 255), Rgba::new(9, 9, 9, 128));
    img.set_pixel(0, 0, Pixel::Rgba(a)).unwrap();
    img.set_pixel(1, 0, Pixel::Rgba(b)).unwrap();
    let mut rgba = Document::new(img);
    let c = replace_color_command(&rgba, Pixel::Rgba(a), Pixel::Rgba(Rgba::new(1, 2, 3, 255))).unwrap().unwrap();
    rgba.apply(c).unwrap();
    let canvas = rgba.image(rgba.layers()[0].cel()).unwrap();
    assert_eq!(canvas.get_pixel(0, 0).unwrap(), Pixel::Rgba(Rgba::new(1, 2, 3, 255)));
    assert_eq!(canvas.get_pixel(1, 0).unwrap(), Pixel::Rgba(b));
}

#[test]
fn swap_order_is_a_transposition() {
    assert_eq!(swap_order(5, 1, 3), vec![0, 3, 2, 1, 4]);
    assert_eq!(swap_order(5, 3, 1), vec![0, 3, 2, 1, 4]);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    /// Property D: writing and reading any palette in every format gives it back (alpha 255).
    #[test]
    fn property_d_file_round_trip(raw in proptest::collection::vec((any::<u8>(), any::<u8>(), any::<u8>()), 1..257)) {
        let colors: Vec<Rgba> = raw.iter().map(|&(r, g, b)| Rgba::new(r, g, b, 255)).collect();
        for format in [PaletteFormat::Pal, PaletteFormat::Gpl, PaletteFormat::Hex] {
            let back = parse_palette(&write_palette(&colors, format)).unwrap();
            prop_assert_eq!(&back.colors, &colors);
            prop_assert_eq!(back.ignored, 0);
        }
    }
}
