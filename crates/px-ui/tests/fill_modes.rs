//! The modes of the fill tool (specs/hito-4-relleno.md, unit 4.5): rows 9 and 10.
//!
//! Red phase: `FillMode`, `ToolState::{fill_mode, set_fill_mode}` and `OptionRow::FillMode` do not
//! exist yet, so this file does not compile until the logic of `px-ui`.

use px_core::{Command, Document, DocumentError, Image, ImageId, Mode, Palette, Pixel, Rgba};
use px_raster::{Brush, BrushShape, PatternTile};
use px_ui::canvas::CanvasEvent;
use px_ui::options::options_row_count;
use px_ui::tool::{FillMode, OptionRow, Tool, ToolAction, ToolState};

const RED: Rgba = Rgba::opaque(255, 0, 0);
const BLUE: Rgba = Rgba::opaque(0, 0, 255);
const GREEN: Rgba = Rgba::opaque(0, 255, 0);

fn red() -> Pixel {
    Pixel::Rgba(RED)
}

fn blue() -> Pixel {
    Pixel::Rgba(BLUE)
}

fn fill_tool() -> ToolState {
    let mut tool = ToolState::new();
    tool.apply_action(ToolAction::Select(Tool::Fill));
    tool
}

/// A click: press and release. Returns the result of the press.
fn click(
    tool: &mut ToolState,
    doc: &mut Document,
    id: ImageId,
    pixel: Pixel,
    at: (i32, i32),
) -> Result<bool, DocumentError> {
    let result = tool.canvas_event_with(doc, id, Some(pixel), CanvasEvent::StrokeBegin(at));
    tool.canvas_event_with(doc, id, Some(pixel), CanvasEvent::StrokeEnd)
        .expect("release");
    assert!(!tool.busy());
    result
}

fn rgba_image(width: u32, height: u32, f: impl Fn(u32, u32) -> Rgba) -> Image {
    let mut image = Image::new(width, height, Mode::Rgba).expect("image");
    for y in 0..height {
        for x in 0..width {
            image.set_pixel(x, y, Pixel::Rgba(f(x, y))).expect("inside");
        }
    }
    image
}

fn pixel_at(doc: &Document, id: ImageId, x: u32, y: u32) -> Pixel {
    doc.image(id)
        .expect("cell")
        .get_pixel(x, y)
        .expect("inside")
}

// Row 9.

#[test]
fn row_09_the_rows_of_the_fill_follow_its_mode() {
    let mut tool = fill_tool();
    assert_eq!(tool.fill_mode(), FillMode::Solid);
    assert_eq!(tool.option_rows(), [OptionRow::FillMode]);
    assert_eq!(options_row_count(Tool::Fill), 1);
    assert_eq!(tool.option_text(OptionRow::FillMode), "Modo: Sólido");

    tool.click_option(OptionRow::FillMode, true, false);
    assert_eq!(tool.fill_mode(), FillMode::Pattern);
    assert_eq!(
        tool.option_rows(),
        [OptionRow::FillMode, OptionRow::Shape, OptionRow::Size]
    );
    assert_eq!(tool.option_text(OptionRow::FillMode), "Modo: Patrón");

    // `Mayús` does not change the step of a cycle row.
    tool.click_option(OptionRow::FillMode, true, true);
    assert_eq!(tool.fill_mode(), FillMode::UntilBackground);
    assert_eq!(tool.option_rows(), [OptionRow::FillMode]);
    assert_eq!(
        tool.option_text(OptionRow::FillMode),
        "Modo: Hasta el fondo"
    );

    tool.click_option(OptionRow::FillMode, true, false);
    assert_eq!(tool.fill_mode(), FillMode::Solid, "it cycles");
    tool.click_option(OptionRow::FillMode, false, false);
    assert_eq!(
        tool.fill_mode(),
        FillMode::UntilBackground,
        "right click: previous"
    );

    // `+` and `-` change the size only with the pattern; the brush rows are the shared brush.
    tool.set_fill_mode(FillMode::Solid);
    assert!(!tool.step_brush_size(1));
    tool.set_fill_mode(FillMode::UntilBackground);
    assert!(!tool.step_brush_size(1));
    tool.set_fill_mode(FillMode::Pattern);
    assert!(tool.step_brush_size(2));
    assert_eq!(tool.brush().size(), 3);
    tool.click_option(OptionRow::Shape, true, false);
    assert_eq!(tool.brush().shape(), BrushShape::Square);
    assert_eq!(tool.option_text(OptionRow::Size), "Tamaño: 3");

    // The mode is kept when the tool changes, and the others still have no rows.
    tool.apply_action(ToolAction::Select(Tool::Pencil));
    tool.apply_action(ToolAction::Select(Tool::Fill));
    assert_eq!(tool.fill_mode(), FillMode::Pattern);
    for other in [Tool::Dropper, Tool::Eraser] {
        tool.apply_action(ToolAction::Select(other));
        assert!(tool.option_rows().is_empty(), "{other:?}");
    }
}

// Row 10.

#[test]
fn row_10_a_click_per_mode_is_one_step_on_rgba() {
    // Pattern: only the mask inside the region.
    let mut tool = fill_tool();
    tool.set_fill_mode(FillMode::Pattern);
    tool.set_brush(Brush::new(BrushShape::Plus, 3));
    let tile = PatternTile::from_brush(&tool.brush());
    let image = rgba_image(12, 9, |x, _| if x < 7 { GREEN } else { BLUE });
    let mut doc = Document::new(image.clone());
    let id = ImageId::from_raw(0);
    assert_eq!(click(&mut tool, &mut doc, id, red(), (2, 2)), Ok(true));
    for y in 0..9 {
        for x in 0..12 {
            let want = if x < 7 && tile.contains(x, y) {
                red()
            } else {
                image.get_pixel(x, y).unwrap()
            };
            assert_eq!(pixel_at(&doc, id, x, y), want, "({x}, {y})");
        }
    }
    doc.undo().expect("one step");
    assert_eq!(doc.image(id).unwrap().bytes(), image.bytes());
    assert!(!doc.can_undo(), "a single step");

    // Until the background: the other slot is the boundary.
    let ring = |x: u32, y: u32| {
        let on_ring =
            (1..=5).contains(&x) && (1..=5).contains(&y) && (x == 1 || x == 5 || y == 1 || y == 5);
        if on_ring {
            BLUE
        } else if (2..=4).contains(&x) && (2..=4).contains(&y) {
            Rgba::opaque(10 * (x as u8 + 3 * y as u8), 99, 1)
        } else {
            GREEN
        }
    };
    let image = rgba_image(7, 7, ring);
    let mut tool = fill_tool();
    tool.set_fill_mode(FillMode::UntilBackground);
    tool.set_other_pixel(Some(blue()));
    let mut doc = Document::new(image.clone());
    assert_eq!(click(&mut tool, &mut doc, id, red(), (3, 3)), Ok(true));
    for y in 0..7 {
        for x in 0..7 {
            let inner = (2..=4).contains(&x) && (2..=4).contains(&y);
            let want = if inner {
                red()
            } else {
                image.get_pixel(x, y).unwrap()
            };
            assert_eq!(pixel_at(&doc, id, x, y), want, "({x}, {y})");
        }
    }
    doc.undo().expect("one step");
    assert!(!doc.can_undo());

    // Without a color in the other slot it behaves like the solid fill.
    tool.set_other_pixel(None);
    let mut doc = Document::new(image.clone());
    assert_eq!(click(&mut tool, &mut doc, id, red(), (3, 3)), Ok(true));
    assert_eq!(pixel_at(&doc, id, 3, 3), red());
    assert_eq!(
        pixel_at(&doc, id, 2, 2),
        image.get_pixel(2, 2).unwrap(),
        "only the seed value"
    );
    // A seed that is the background does nothing.
    tool.set_other_pixel(Some(blue()));
    let mut doc = Document::new(image);
    assert_eq!(click(&mut tool, &mut doc, id, red(), (1, 1)), Ok(false));
    assert!(!doc.can_undo());
}

#[test]
fn row_10_indexed_and_the_empty_cell_do_not_materialize_before_applying() {
    // Indexed pattern.
    let palette = Palette::from_colors([GREEN, BLUE, RED]).expect("palette");
    let image = Image::new_indexed(10, 6, palette).expect("image");
    let mut doc = Document::new(image);
    let id = ImageId::from_raw(0);
    let mut tool = fill_tool();
    tool.set_fill_mode(FillMode::Pattern);
    tool.set_brush(Brush::new(BrushShape::Slash, 2));
    assert_eq!(
        click(&mut tool, &mut doc, id, Pixel::Indexed(2), (0, 0)),
        Ok(true)
    );
    for y in 0..6 {
        for x in 0..10 {
            let want = if (x + y) % 2 == 1 { 2 } else { 0 };
            assert_eq!(pixel_at(&doc, id, x, y), Pixel::Indexed(want), "({x}, {y})");
        }
    }

    // An empty cell: all three modes work on it.
    let new_cell = |doc: &mut Document| {
        let at = doc.layers().len();
        doc.apply(Command::AddLayer {
            name: "B".into(),
            at,
        })
        .expect("layer");
        doc.layers()[at].cel()
    };
    let background = rgba_image(8, 6, |_, _| GREEN);
    let mut doc = Document::new(background);
    let cel = new_cell(&mut doc);
    let before = doc.clone();
    let mut tool = fill_tool();
    tool.set_fill_mode(FillMode::Pattern);
    tool.set_brush(Brush::new(BrushShape::Slash, 2));
    assert_eq!(click(&mut tool, &mut doc, cel, red(), (3, 3)), Ok(true));
    for y in 0..6 {
        for x in 0..8 {
            let want = if (x + y) % 2 == 1 {
                red()
            } else {
                Pixel::Rgba(Rgba::TRANSPARENT)
            };
            assert_eq!(pixel_at(&doc, cel, x, y), want, "({x}, {y})");
        }
    }
    doc.undo().expect("one step");
    assert_eq!(doc, before);
    assert!(doc.image(cel).is_none(), "the cell is empty again");

    // Until the background: the boundary is the transparent value (nothing to do) or a color
    // that is not on the cell (all the canvas).
    tool.set_fill_mode(FillMode::UntilBackground);
    tool.set_other_pixel(Some(Pixel::Rgba(Rgba::TRANSPARENT)));
    assert_eq!(click(&mut tool, &mut doc, cel, red(), (3, 3)), Ok(false));
    assert_eq!(doc, before);
    tool.set_other_pixel(Some(blue()));
    assert_eq!(click(&mut tool, &mut doc, cel, red(), (3, 3)), Ok(true));
    assert!((0..6).all(|y| (0..8).all(|x| pixel_at(&doc, cel, x, y) == red())));
}

#[test]
fn row_10_the_14c_cap_holds_in_the_new_modes() {
    // 16384 x 368 comb (`S = 3 006 465`): row 0 all `A`; elsewhere `A` on even columns.
    let a = [10u8, 20, 30, 255];
    let b = [40u8, 50, 60, 255];
    let mut data = Vec::with_capacity(16384 * 368 * 4);
    for y in 0..368usize {
        for x in 0..16384usize {
            data.extend_from_slice(if y == 0 || x % 2 == 0 { &a } else { &b });
        }
    }
    let image = Image::from_bytes(16384, 368, Mode::Rgba, data).expect("image");
    let id = ImageId::from_raw(0);
    for mode in [FillMode::Pattern, FillMode::UntilBackground] {
        let mut doc = Document::new(image.clone());
        let state = doc.state_id();
        let mut tool = fill_tool();
        tool.set_fill_mode(mode);
        tool.set_other_pixel(Some(Pixel::Rgba(Rgba::new(40, 50, 60, 255))));
        let result = click(&mut tool, &mut doc, id, red(), (0, 0));
        assert!(
            matches!(result, Err(DocumentError::FillTooFragmented { .. })),
            "{mode:?}: {result:?}"
        );
        assert!(tool.take_changes().is_empty(), "{mode:?}");
        assert_eq!(doc.state_id(), state, "{mode:?}");
        assert!(!doc.can_undo(), "{mode:?}");
    }
}
