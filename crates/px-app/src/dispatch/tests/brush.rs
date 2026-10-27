//! Row 12 of `specs/hito-4-pincel.md` (unit 4.3), the part that lives in the dispatcher: the size
//! and mode actions reach the tool state, and a stroke with a thick brush is one undo step.
//!
//! Red phase: `AppAction::{BrushBigger, BrushSmaller, SelectDotted, SelectCurve}` and
//! `ToolState::{brush, set_brush, dotted, curve}` do not exist yet.

use px_raster::{Brush, BrushShape};

use super::*;

fn done() -> Dispatched {
    Dispatched::Done {
        refresh: false,
        notice: None,
    }
}

#[test]
fn row_12_the_size_actions_change_the_brush_of_a_stroke_tool_only() {
    let mut world = World::standard();
    assert_eq!(world.tools.brush().size(), 1);
    assert_eq!(
        world.dispatches(AppAction::BrushBigger, Origin::Key),
        done()
    );
    assert_eq!(
        world.dispatches(AppAction::BrushBigger, Origin::Key),
        done()
    );
    assert_eq!(world.tools.brush().size(), 3);
    assert_eq!(
        world.dispatches(AppAction::BrushSmaller, Origin::Key),
        done()
    );
    assert_eq!(world.tools.brush().size(), 2);
    world.dispatches(AppAction::SelectTool(Tool::Fill), Origin::Key);
    world.dispatches(AppAction::BrushBigger, Origin::Key);
    assert_eq!(world.tools.brush().size(), 2, "the fill has no brush");
}

#[test]
fn row_12_the_mode_actions_select_dotted_and_curve() {
    let mut world = World::standard();
    assert_eq!(
        world.dispatches(AppAction::SelectDotted, Origin::Key),
        done()
    );
    assert_eq!(
        (world.tools.tool(), world.tools.dotted()),
        (Tool::Pencil, true)
    );
    assert_eq!(
        world.dispatches(AppAction::SelectCurve, Origin::Key),
        done()
    );
    assert_eq!(
        (world.tools.tool(), world.tools.curve()),
        (Tool::Line, true)
    );
}

#[test]
fn row_12_a_thick_stroke_on_an_indexed_image_is_one_undo_step() {
    let mut world = World::standard();
    let before = world.image_2().clone();
    world.tools.set_brush(Brush::new(BrushShape::Square, 2));
    let id = ImageId::from_raw(0);
    for event in [CanvasEvent::StrokeBegin((3, 3)), CanvasEvent::StrokeEnd] {
        world
            .tools
            .canvas_event_with(
                &mut world.session.document,
                id,
                Some(Pixel::Indexed(2)),
                event,
            )
            .expect("stroke");
    }
    for (x, y) in [(2, 2), (3, 2), (2, 3), (3, 3)] {
        assert_eq!(
            world.image_2().get_pixel(x, y),
            Ok(Pixel::Indexed(2)),
            "({x}, {y})"
        );
    }
    world
        .session
        .undo()
        .expect("one step undoes the whole stroke");
    assert_eq!(world.image_2(), &before);
}
