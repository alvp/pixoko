//! Row 12 of `specs/hito-4-formas.md` (unit 4.4), the part that lives in the dispatcher: the
//! shape actions reach the tool state, the button keeps the kind and a rectangle with both
//! colors on an indexed image is one undo step.
//!
//! Red phase: `AppAction::SelectShape`, `ShapeKind`, `ShapeFill`, `Tool::Shape` and
//! `ToolState::{shape_kind, shape_fill, set_shape_fill, set_other_pixel}` do not exist yet.

use px_ui::tool::{ShapeFill, ShapeKind};

use super::*;

fn done() -> Dispatched {
    Dispatched::Done {
        refresh: false,
        notice: None,
    }
}

#[test]
fn row_12_the_kind_actions_select_the_shapes_button_with_that_kind() {
    let mut world = World::standard();
    world.tools.set_shape_fill(ShapeFill::Both);
    for kind in [
        ShapeKind::Rectangle,
        ShapeKind::Circle,
        ShapeKind::Ellipse,
        ShapeKind::Polygon,
        ShapeKind::Freeform,
    ] {
        assert_eq!(
            world.dispatches(AppAction::SelectShape(kind), Origin::Key),
            done()
        );
        assert_eq!(world.tools.tool(), Tool::Shape);
        assert_eq!(world.tools.shape_kind(), kind);
        assert_eq!(world.tools.shape_fill(), ShapeFill::Both, "the fill is kept");
    }
}

#[test]
fn row_12_the_button_keeps_the_kind_and_does_not_reset_it_when_already_active() {
    let mut world = World::standard();
    world.dispatches(AppAction::SelectShape(ShapeKind::Circle), Origin::Key);
    world.dispatches(AppAction::SelectTool(Tool::Pencil), Origin::Key);
    world.dispatches(AppAction::SelectTool(Tool::Shape), Origin::Button);
    assert_eq!(world.tools.tool(), Tool::Shape);
    assert_eq!(world.tools.shape_kind(), ShapeKind::Circle);
    world.dispatches(AppAction::SelectTool(Tool::Shape), Origin::Button);
    assert_eq!(world.tools.shape_kind(), ShapeKind::Circle);
}

#[test]
fn row_12_a_rectangle_with_both_colors_on_an_indexed_image_is_one_undo_step() {
    let mut world = World::standard();
    let before = world.image_2().clone();
    world.dispatches(AppAction::SelectShape(ShapeKind::Rectangle), Origin::Key);
    world.tools.set_shape_fill(ShapeFill::Both);
    world.tools.set_other_pixel(Some(Pixel::Indexed(3)));
    let id = ImageId::from_raw(0);
    for event in [
        CanvasEvent::StrokeBegin((2, 2)),
        CanvasEvent::StrokeMove((6, 5)),
        CanvasEvent::StrokeEnd,
    ] {
        world
            .tools
            .canvas_event_with(
                &mut world.session.document,
                id,
                Some(Pixel::Indexed(2)),
                event,
            )
            .expect("shape");
    }
    assert_eq!(world.image_2().get_pixel(2, 2), Ok(Pixel::Indexed(2)));
    assert_eq!(world.image_2().get_pixel(4, 3), Ok(Pixel::Indexed(3)));
    world
        .session
        .undo()
        .expect("one step undoes the whole shape");
    assert_eq!(world.image_2(), &before);
}
