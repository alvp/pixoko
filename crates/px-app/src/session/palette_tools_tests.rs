//! Unit 4.6 (specs/hito-4-paleta.md), app part: row PC-12 (swapping entries keeps the resolved image,
//! every operation is one undo step) and the tools around it. No window.

use super::*;

use px_core::{Palette, Pixel, Rgba};
use px_ui::color::ColorChoice;
use px_ui::palette_file::PaletteFormat;
use px_ui::palette_tools::PaletteTool;

fn five() -> Session {
    let palette =
        Palette::from_colors((0..5u8).map(|i| Rgba::opaque(i * i * 12, 255 - i * 40, 9))).unwrap();
    let mut image = Image::filled_indexed(5, 1, palette, 0).expect("indexed");
    for x in 0..5 {
        image.set_pixel(x, 0, Pixel::Indexed(x as u8)).unwrap();
    }
    Session::sample(Document::new(image))
}

fn cel(s: &Session) -> &Image {
    s.document.image(s.active_cel()).expect("cel")
}

fn resolved(s: &Session) -> Vec<Rgba> {
    let image = cel(&s);
    let palette = image.palette().unwrap();
    (0..5)
        .map(|x| match image.get_pixel(x, 0).unwrap() {
            Pixel::Indexed(i) => palette.resolved(i).unwrap(),
            other => panic!("{other:?}"),
        })
        .collect()
}

fn mark(s: &mut Session, active: u8, range_end: Option<u8>, extras: &[u8]) {
    s.choice = ColorChoice::Entry(active);
    s.ops.clear_range();
    if let Some(end) = range_end {
        s.ops.set_range_end(end);
    }
    for &extra in extras {
        s.ops.toggle_extra(extra);
    }
}

#[test]
fn row_pc12_swap_keeps_the_image_and_is_one_undo_step() {
    let mut s = five();
    let before = resolved(&s);
    let palette_before = s.document.palette().unwrap().colors().to_vec();
    mark(&mut s, 1, None, &[3]);
    let outcome = s.palette_tool(PaletteTool::Swap).unwrap();
    assert!(outcome.changed);
    assert_eq!(resolved(&s), before);
    let swapped = s.document.palette().unwrap().colors().to_vec();
    assert_eq!(swapped[1], palette_before[3]);
    assert_eq!(swapped[3], palette_before[1]);
    s.document.undo().unwrap();
    assert_eq!(s.document.palette().unwrap().colors(), &palette_before[..]);
    assert!(!s.document.can_undo());
}

#[test]
fn swap_needs_exactly_two_marked() {
    let mut s = five();
    mark(&mut s, 1, Some(3), &[]);
    let outcome = s.palette_tool(PaletteTool::Swap).unwrap();
    assert!(!outcome.changed && outcome.notice.is_some());
    assert!(!s.document.can_undo());
}

#[test]
fn gradient_delete_default_and_copy_paste_are_one_step_each() {
    let mut s = five();
    mark(&mut s, 0, Some(4), &[]);
    assert!(s.palette_tool(PaletteTool::Gradient).unwrap().changed);
    let colors = s.document.palette().unwrap().colors().to_vec();
    assert_eq!(colors[2].r, 96, "midpoint of 0 and 192");
    s.document.undo().unwrap();

    mark(&mut s, 1, None, &[3]);
    assert!(s.palette_tool(PaletteTool::Delete).unwrap().changed);
    assert_eq!(s.document.palette().unwrap().len(), 3);
    s.document.undo().unwrap();
    assert_eq!(s.document.palette().unwrap().len(), 5);

    assert!(s.palette_tool(PaletteTool::Default).unwrap().changed);
    assert_eq!(s.document.palette().unwrap().len(), 5);
    s.document.undo().unwrap();

    mark(&mut s, 0, Some(1), &[]);
    s.palette_tool(PaletteTool::Copy).unwrap();
    mark(&mut s, 4, None, &[]);
    assert!(s.palette_tool(PaletteTool::Paste).unwrap().changed);
    assert_eq!(
        s.document.palette().unwrap().len(),
        6,
        "pasting past the end grows"
    );
    s.document.undo().unwrap();
    assert_eq!(s.document.palette().unwrap().len(), 5);
}

#[test]
fn load_and_save_palette_text_round_trip() {
    let mut s = five();
    let text = s.palette_file_text(PaletteFormat::Hex).unwrap();
    let before = s.document.palette().unwrap().colors().to_vec();
    assert!(s.palette_tool(PaletteTool::Default).unwrap().changed);
    let outcome = s.load_palette_text(&text, false).unwrap();
    assert!(outcome.changed);
    assert_eq!(s.document.palette().unwrap().colors(), &before[..]);
    let bad = s.load_palette_text("JASC-PAL\n0200\n1\n", false).unwrap();
    assert!(!bad.changed && bad.notice.is_some());
}

#[test]
fn replace_and_swap_color_in_image_use_foreground_and_background() {
    let mut s = five();
    s.choice = ColorChoice::Entry(1);
    s.background = ColorChoice::Entry(3);
    assert!(s.palette_tool(PaletteTool::ReplaceColor).unwrap().changed);
    let image = cel(&s);
    assert_eq!(image.get_pixel(1, 0).unwrap(), Pixel::Indexed(3));
    s.document.undo().unwrap();
    assert!(s.palette_tool(PaletteTool::SwapColors).unwrap().changed);
    let image = cel(&s);
    assert_eq!(image.get_pixel(1, 0).unwrap(), Pixel::Indexed(3));
    assert_eq!(image.get_pixel(3, 0).unwrap(), Pixel::Indexed(1));
    s.document.undo().unwrap();
    s.background = ColorChoice::Entry(1);
    assert!(!s.palette_tool(PaletteTool::ReplaceColor).unwrap().changed);
}
