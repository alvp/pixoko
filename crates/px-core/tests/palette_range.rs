//! Tests of `Command::SetPaletteRange` and `Command::RemovePaletteEntries`
//! (specs/hito-4-paleta.md, rows PC-01 to PC-06 and properties A and B).
//!
//! The figures are computed by hand from the spec; the property models are written here and do
//! not call the code under test. These tests do not compile until both commands and
//! `PaletteEditError::BadSelection` exist.

use proptest::prelude::*;
use px_core::{
    Command, Document, DocumentError, Image, ImageId, Palette, PaletteEditError, Pixel, Rgba,
};

fn colors(n: usize) -> Vec<Rgba> {
    (0..n)
        .map(|i| Rgba::new(i as u8, 255 - i as u8, 7, 255))
        .collect()
}

fn doc(n: usize, transparent: Option<u8>, pixels: &[u8]) -> Document {
    let mut palette = Palette::from_colors(colors(n)).unwrap();
    palette.set_transparent_index(transparent).unwrap();
    let w = pixels.len() as u32;
    let mut img = Image::new_indexed(w, 1, palette).unwrap();
    for (x, &v) in pixels.iter().enumerate() {
        img.set_pixel(x as u32, 0, Pixel::Indexed(v)).unwrap();
    }
    Document::new(img)
}

fn id() -> ImageId {
    ImageId::from_raw(0)
}

fn px(d: &Document) -> Vec<u8> {
    let img = d.image(id()).unwrap();
    (0..img.width())
        .map(|x| match img.get_pixel(x, 0) {
            Ok(Pixel::Indexed(v)) => v,
            other => panic!("indexed pixel expected, got {other:?}"),
        })
        .collect()
}

fn pal(d: &Document) -> Vec<Rgba> {
    d.palette().unwrap().colors().to_vec()
}

fn set_range(start: u8, colors: Vec<Rgba>) -> Command {
    Command::SetPaletteRange {
        image: id(),
        start,
        colors,
    }
}

fn remove(indices: Vec<u8>, into: u8) -> Command {
    Command::RemovePaletteEntries {
        image: id(),
        indices,
        into,
    }
}

fn edit_err(r: Result<bool, DocumentError>) -> PaletteEditError {
    match r {
        Err(DocumentError::PaletteEdit(e)) => e,
        other => panic!("a palette error was expected, got {other:?}"),
    }
}

const W: Rgba = Rgba::new(250, 250, 250, 255);
const K: Rgba = Rgba::new(1, 2, 3, 255);

#[test]
fn row_pc01_set_range_inside_changes_colors_not_pixels() {
    let mut d = doc(4, None, &[0, 1, 2, 3]);
    let before = pal(&d);
    assert_eq!(d.apply(set_range(1, vec![W, K])), Ok(true));
    assert_eq!(pal(&d), vec![before[0], W, K, before[3]]);
    assert_eq!(px(&d), vec![0, 1, 2, 3]);
    d.undo().unwrap();
    assert_eq!(pal(&d), before);
    d.redo().unwrap();
    assert_eq!(pal(&d), vec![before[0], W, K, before[3]]);
}

#[test]
fn row_pc02_set_range_past_the_end_grows_and_undo_shrinks() {
    let mut d = doc(3, Some(1), &[0, 1, 2]);
    let before = pal(&d);
    assert_eq!(d.apply(set_range(2, vec![W, K, W])), Ok(true));
    assert_eq!(pal(&d), vec![before[0], before[1], W, K, W]);
    assert_eq!(d.palette().unwrap().transparent_index(), Some(1));
    assert_eq!(px(&d), vec![0, 1, 2]);
    d.undo().unwrap();
    assert_eq!(pal(&d), before);
    d.redo().unwrap();
    assert_eq!(d.palette().unwrap().len(), 5);
}

#[test]
fn row_pc03_set_range_errors_and_no_ops() {
    let mut d = doc(3, None, &[0, 1, 2]);
    let state = d.history_bytes();
    assert!(matches!(
        edit_err(d.apply(set_range(4, vec![W]))),
        PaletteEditError::IndexOutOfRange { .. }
    ));
    assert!(matches!(
        edit_err(d.apply(set_range(2, vec![W; 255]))),
        PaletteEditError::RangeOutOfPalette { .. }
    ));
    assert_eq!(d.apply(set_range(1, vec![])), Ok(false));
    let same = pal(&d)[1..3].to_vec();
    assert_eq!(d.apply(set_range(1, same)), Ok(false));
    assert_eq!(d.history_bytes(), state);
}

#[test]
fn row_pc04_remove_entries_moves_pixels_to_into_and_undo_restores() {
    let mut d = doc(5, None, &[0, 1, 2, 3, 4]);
    let before = pal(&d);
    assert_eq!(d.apply(remove(vec![1, 3], 0)), Ok(true));
    assert_eq!(px(&d), vec![0, 0, 1, 0, 2]);
    assert_eq!(pal(&d), vec![before[0], before[2], before[4]]);
    d.undo().unwrap();
    assert_eq!(px(&d), vec![0, 1, 2, 3, 4]);
    assert_eq!(pal(&d), before);
    d.redo().unwrap();
    assert_eq!(px(&d), vec![0, 0, 1, 0, 2]);
}

#[test]
fn row_pc05_remove_entries_transparent_inside_dropped_outside_shifts() {
    let mut d = doc(5, Some(3), &[0, 1, 2, 3, 4]);
    d.apply(remove(vec![3], 0)).unwrap();
    assert_eq!(d.palette().unwrap().transparent_index(), None);
    d.undo().unwrap();
    assert_eq!(d.palette().unwrap().transparent_index(), Some(3));
    d.apply(remove(vec![0, 1], 2)).unwrap();
    assert_eq!(d.palette().unwrap().transparent_index(), Some(1));
    assert_eq!(px(&d), vec![0, 0, 0, 1, 2]);
    d.undo().unwrap();
    assert_eq!(d.palette().unwrap().transparent_index(), Some(3));
}

#[test]
fn row_pc06_remove_entries_errors() {
    let mut d = doc(4, None, &[0, 1, 2, 3]);
    assert_eq!(
        edit_err(d.apply(remove(vec![], 0))),
        PaletteEditError::BadSelection
    );
    assert_eq!(
        edit_err(d.apply(remove(vec![2, 1], 0))),
        PaletteEditError::BadSelection
    );
    assert_eq!(
        edit_err(d.apply(remove(vec![1, 1], 0))),
        PaletteEditError::BadSelection
    );
    assert!(matches!(
        edit_err(d.apply(remove(vec![1, 9], 0))),
        PaletteEditError::IndexOutOfRange { .. }
    ));
    assert_eq!(
        edit_err(d.apply(remove(vec![1, 2], 2))),
        PaletteEditError::RemoveIntoSelf { index: 2 }
    );
    assert_eq!(
        edit_err(d.apply(remove(vec![0, 1, 2, 3], 0))),
        PaletteEditError::LastEntry
    );
    assert_eq!(d.history_bytes(), 0);
}

fn rgba_strategy() -> impl Strategy<Value = Rgba> {
    (any::<u8>(), any::<u8>(), any::<u8>(), any::<u8>())
        .prop_map(|(r, g, b, a)| Rgba::new(r, g, b, a))
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    /// Property A: applying `SetPaletteRange` and undoing it is the identity.
    #[test]
    fn property_a_set_range_undo_is_identity(
        n in 1usize..40,
        start in 0usize..40,
        new in proptest::collection::vec(rgba_strategy(), 1..20),
    ) {
        let start = start.min(n);
        let pixels: Vec<u8> = (0..n as u8).collect();
        let mut d = doc(n, None, &pixels);
        let (p0, c0, h0) = (px(&d), pal(&d), d.history_bytes());
        let applied = d.apply(set_range(start as u8, new.clone())).unwrap();
        let mut expected = c0.clone();
        for (i, c) in new.iter().enumerate() {
            if start + i < expected.len() { expected[start + i] = *c } else { expected.push(*c) }
        }
        prop_assert_eq!(pal(&d), expected.clone());
        prop_assert_eq!(px(&d), p0.clone());
        if applied {
            d.undo().unwrap();
            prop_assert_eq!((px(&d), pal(&d), d.history_bytes()), (p0, c0, h0));
            d.redo().unwrap();
            prop_assert_eq!(pal(&d), expected);
        }
    }

    /// Property B: removing a set keeps every pixel in range and undoing is exact.
    #[test]
    fn property_b_remove_entries_invariants_and_exact_undo(
        n in 2usize..30,
        mask in proptest::collection::vec(any::<bool>(), 30),
        pixels in proptest::collection::vec(0u8..30, 1..40),
        transparent in proptest::option::of(0u8..30),
        into_pick in 0usize..30,
    ) {
        let pixels: Vec<u8> = pixels.iter().map(|p| p % n as u8).collect();
        let transparent = transparent.map(|t| t % n as u8);
        let removed: Vec<u8> = (0..n as u8).filter(|&i| mask[i as usize]).collect();
        let kept: Vec<u8> = (0..n as u8).filter(|&i| !mask[i as usize]).collect();
        prop_assume!(!removed.is_empty() && !kept.is_empty());
        let into = kept[into_pick % kept.len()];
        let mut d = doc(n, transparent, &pixels);
        let (p0, c0) = (px(&d), pal(&d));
        d.apply(remove(removed.clone(), into)).unwrap();
        let new_of = |old: u8| kept.iter().position(|&k| k == old).unwrap() as u8;
        let model: Vec<u8> = p0.iter().map(|&v| if mask[v as usize] { new_of(into) } else { new_of(v) }).collect();
        prop_assert_eq!(px(&d), model);
        prop_assert!(px(&d).iter().all(|&v| (v as usize) < kept.len()));
        prop_assert_eq!(pal(&d), kept.iter().map(|&k| c0[k as usize]).collect::<Vec<_>>());
        let t = d.palette().unwrap().transparent_index();
        prop_assert_eq!(t, transparent.and_then(|t| kept.iter().position(|&k| k == t).map(|p| p as u8)));
        d.undo().unwrap();
        prop_assert_eq!((px(&d), pal(&d)), (p0, c0));
        prop_assert_eq!(d.palette().unwrap().transparent_index(), transparent);
    }
}
