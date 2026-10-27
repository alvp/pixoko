//! Row 11 of `specs/hito-4-relleno.md` (unit 4.5), the part that lives in the dispatcher: the fill
//! modes work on an indexed image through the session and each click is one undo step.
//!
//! Red phase: `FillMode` and `ToolState::{fill_mode, set_fill_mode}` do not exist yet.

use px_raster::{Brush, BrushShape};
use px_ui::tool::FillMode;

use super::*;

/// A click with the fill tool: press and release, with `other` as the color of the other slot.
fn click(world: &mut World, pixel: Pixel, other: Option<Pixel>, at: (i32, i32)) -> bool {
    world.tools.set_other_pixel(other);
    let id = ImageId::from_raw(0);
    let mut changed = false;
    for event in [CanvasEvent::StrokeBegin(at), CanvasEvent::StrokeEnd] {
        changed |= world
            .tools
            .canvas_event_with(&mut world.session.document, id, Some(pixel), event)
            .expect("fill");
    }
    changed
}

#[test]
fn row_11_the_pattern_fill_on_an_indexed_image_is_one_undo_step() {
    let mut world = World::new_2(indexed(4, None), 0, 0);
    let before = world.image_2().clone();
    world.dispatches(AppAction::SelectTool(Tool::Fill), Origin::Button);
    world.tools.set_fill_mode(FillMode::Pattern);
    world.tools.set_brush(Brush::new(BrushShape::Slash, 2));
    assert!(click(
        &mut world,
        Pixel::Indexed(2),
        Some(Pixel::Indexed(3)),
        (0, 0)
    ));
    for y in 0..4 {
        for x in 0..4 {
            let want = if (x + y) % 2 == 1 { 2 } else { 0 };
            assert_eq!(world.image_2().get_pixel(x, y), Ok(Pixel::Indexed(want)));
        }
    }
    world
        .session
        .undo()
        .expect("one step undoes the whole fill");
    assert_eq!(world.image_2(), &before);
}

#[test]
fn row_11_until_the_background_uses_the_other_slot() {
    let mut world = World::new_2(indexed(4, None), 0, 0);
    let before = world.image_2().clone();
    world.dispatches(AppAction::SelectTool(Tool::Fill), Origin::Button);
    world.tools.set_fill_mode(FillMode::UntilBackground);
    // The seed is the background (index 0): nothing happens and nothing is recorded.
    assert!(!click(
        &mut world,
        Pixel::Indexed(2),
        Some(Pixel::Indexed(0)),
        (1, 1)
    ));
    assert_eq!(world.image_2(), &before);
    // A background that is not in the image: the whole canvas, in one step.
    assert!(click(
        &mut world,
        Pixel::Indexed(2),
        Some(Pixel::Indexed(3)),
        (1, 1)
    ));
    assert!(
        (0..4).all(|y| (0..4).all(|x| world.image_2().get_pixel(x, y) == Ok(Pixel::Indexed(2))))
    );
    world.session.undo().expect("one step");
    assert_eq!(world.image_2(), &before);
}
