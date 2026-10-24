//! Unit 4.1 (specs/hito-4-imagen.md), app part: row 10 (the form of `Tamaño del lienzo…`), row 11
//! (the session: one step per operation, the fill, the unchanged notice, the refusal by memory and
//! `dispatch`) and row 12 (saving and reopening after a turn). No window.

use super::*;

use px_core::{BlendMode, LayerSpec, Palette, Pixel, Rgba};
use px_formats::read_png;
use px_raster::{Anchor, ImageOp};
use px_ui::actions::{AppAction, Locks, Origin};
use px_ui::dialog::{Dialog, DialogKind, Event, Modal, Reaction};
use px_ui::tool::ToolState;

use crate::canvas_size::{
    FIELD_ANCHOR, FIELD_HEIGHT, FIELD_WIDTH, canvas_size_form, op_from_values, open_canvas_size,
};
use crate::dispatch::{Dispatched, Targets, dispatch};

fn rgba_cel(seed: u8) -> Image {
    let bytes: Vec<u8> = (0..3 * 2 * 4)
        .map(|i| seed.wrapping_add(i as u8 * 9))
        .collect();
    Image::from_bytes(3, 2, Mode::Rgba, bytes).expect("cel")
}

/// Three layers of 3 × 2: plain, empty, and hidden, locked and translucent.
fn three_layers() -> Session {
    let layer = |name: &str, visible, locked, opacity, blend, image| LayerSpec {
        name: name.to_string(),
        visible,
        opacity,
        blend,
        locked,
        image,
    };
    let layers = vec![
        layer("a", true, false, 255, BlendMode::Normal, Some(rgba_cel(1))),
        layer("b", true, false, 255, BlendMode::Normal, None),
        layer(
            "c",
            false,
            true,
            128,
            BlendMode::Multiply,
            Some(rgba_cel(50)),
        ),
    ];
    Session::sample(Document::from_layers(3, 2, Mode::Rgba, None, layers).expect("document"))
}

fn single(mode: Mode, palette: Option<Palette>) -> Session {
    let image = match &palette {
        Some(palette) => Image::filled_indexed(3, 2, palette.clone(), 0).expect("indexed"),
        None => Image::filled(3, 2, Pixel::Gray(40)).expect("gray"),
    };
    assert_eq!(image.mode(), mode);
    Session::sample(Document::new(image))
}

fn palette(transparent: Option<u8>) -> Palette {
    let mut palette = Palette::from_colors((0..8u8).map(|i| Rgba::opaque(i * 30, 255 - i * 30, i)))
        .expect("palette");
    palette
        .set_transparent_index(transparent)
        .expect("transparent");
    palette
}

fn size_of(session: &Session) -> (u32, u32) {
    (session.document.width(), session.document.height())
}

// ---------------------------------------------------------------------------------------
// Row 10: the form.
// ---------------------------------------------------------------------------------------

#[test]
fn row_10_the_form_starts_at_the_current_size_with_the_center_anchor_and_rejects_bad_sizes() {
    let spec = canvas_size_form(3, 2);
    spec.validate().expect("valid form");
    let mut dialog = Dialog::new(DialogKind::CanvasSize, spec).expect("dialog");
    let center = ImageOp::Canvas {
        width: 3,
        height: 2,
        anchor: Anchor::Center,
    };
    assert_eq!(
        dialog.values().and_then(|v| op_from_values(&v)),
        Some(center)
    );
    for (text, valid) in [
        ("1", true),
        ("16384", true),
        ("0", false),
        ("16385", false),
        ("", false),
        ("7x", false),
    ] {
        dialog.event(Event::Number {
            field: FIELD_WIDTH,
            text: text.to_string(),
        });
        assert_eq!(dialog.is_valid(), valid, "width {text:?}");
        let reaction = dialog.event(Event::Accept);
        assert_eq!(matches!(reaction, Reaction::Accepted(_)), valid, "{text:?}");
        if !valid {
            assert!(dialog.reason().is_some(), "{text:?} has a reason");
        }
    }
    dialog.event(Event::Number {
        field: FIELD_WIDTH,
        text: "5".to_string(),
    });
    dialog.event(Event::Number {
        field: FIELD_HEIGHT,
        text: "9".to_string(),
    });
    dialog.event(Event::Choice {
        field: FIELD_ANCHOR,
        index: 0,
    });
    let Reaction::Accepted(values) = dialog.event(Event::Accept) else {
        panic!("accepted");
    };
    assert_eq!(
        op_from_values(&values),
        Some(ImageOp::Canvas {
            width: 5,
            height: 9,
            anchor: Anchor::TopLeft
        })
    );
    // The dialog opens only without locks.
    let mut modal = Modal::default();
    assert!(open_canvas_size(&mut modal, 3, 2, Locks::default()));
    assert_eq!(modal.kind(), Some(DialogKind::CanvasSize));
    modal.abandon();
    let busy = Locks {
        gesture: true,
        ..Locks::default()
    };
    assert!(!open_canvas_size(&mut modal, 3, 2, busy));
}

// ---------------------------------------------------------------------------------------
// Row 11: the session.
// ---------------------------------------------------------------------------------------

#[test]
fn row_11_each_operation_is_one_step_keeps_the_active_layer_and_undo_restores_it() {
    let ops = [
        (ImageOp::FlipH, (3, 2)),
        (ImageOp::FlipV, (3, 2)),
        (ImageOp::Cw, (2, 3)),
        (ImageOp::Ccw, (2, 3)),
        (ImageOp::Half, (3, 2)),
        (
            ImageOp::Canvas {
                width: 5,
                height: 4,
                anchor: Anchor::BottomRight,
            },
            (5, 4),
        ),
    ];
    for (op, size) in ops {
        let mut session = three_layers();
        let (before, active, layers) = (
            session.document.clone(),
            session.active_layer(),
            session.document.layers().len(),
        );
        let id = session.document.state_id();
        let report = session.image_op(op);
        assert!(report.changed, "{op:?}");
        assert_eq!(size_of(&session), size, "{op:?}");
        assert_ne!(session.document.state_id(), id);
        assert!(session.modified());
        assert_eq!(
            session.active_layer(),
            active,
            "the active layer does not change"
        );
        assert_eq!(session.document.layers().len(), layers);
        let after = session.document.clone();
        session.undo().expect("undo");
        assert_eq!(session.document, before, "{op:?}");
        assert_eq!(size_of(&session), (3, 2));
        assert!(!session.document.can_undo(), "{op:?} was one step");
        session.redo().expect("redo");
        assert_eq!(session.document, after, "{op:?}");
    }
}

#[test]
fn row_11_the_new_area_is_transparent_where_the_mode_allows_it_and_the_background_where_not() {
    let canvas = ImageOp::Canvas {
        width: 4,
        height: 2,
        anchor: Anchor::Left,
    };
    let corner = |session: &Session| {
        let cel = session.active_cel();
        session
            .document
            .image(cel)
            .expect("cel")
            .get_pixel(3, 0)
            .expect("pixel")
    };
    // RGBA: transparent.
    let mut rgba = three_layers();
    rgba.image_op(canvas);
    assert_eq!(corner(&rgba), Pixel::Rgba(Rgba::TRANSPARENT).clone());
    // Indexed with a transparent entry: that entry.
    let mut with = single(Mode::Indexed, Some(palette(Some(3))));
    with.background = ColorChoice::Entry(7);
    with.image_op(canvas);
    assert_eq!(corner(&with), Pixel::Indexed(3));
    // Indexed without one, and gray: the background of the session.
    let mut without = single(Mode::Indexed, Some(palette(None)));
    without.background = ColorChoice::Entry(7);
    let wanted = without.pixel_of(Role::Background).expect("background");
    without.image_op(canvas);
    assert_eq!(corner(&without), wanted);
    assert_eq!(wanted, Pixel::Indexed(7));
    let mut gray = single(Mode::Gray, None);
    let wanted = gray.pixel_of(Role::Background).expect("background");
    gray.image_op(canvas);
    assert_eq!(corner(&gray), wanted);
}

#[test]
fn row_11_an_operation_that_changes_nothing_says_so_and_a_refusal_changes_nothing() {
    // A uniform image does not change with a flip: no step.
    let mut session = single(Mode::Indexed, Some(palette(None)));
    let id = session.document.state_id();
    let report = session.image_op(ImageOp::FlipH);
    assert!(!report.changed);
    assert_eq!(
        report.notice.as_deref(),
        Some(px_ui::image_ops_es::IMAGE_OP_NOTHING)
    );
    assert_eq!(session.document.state_id(), id);
    // Over the limit of the cells: notice and the document intact.
    let mut tight = three_layers();
    let limit = tight.document.materialized_bytes();
    tight.document.set_cells_limit(limit);
    let (before, id) = (tight.document.clone(), tight.document.state_id());
    let report = tight.image_op(ImageOp::Canvas {
        width: 30,
        height: 20,
        anchor: Anchor::Center,
    });
    assert!(!report.changed);
    assert!(report.notice.is_some());
    assert_eq!((tight.document.state_id(), &tight.document), (id, &before));
}

#[test]
fn row_11_dispatch_runs_the_five_and_opens_the_dialog_for_the_size() {
    let mut session = three_layers();
    let mut tools = ToolState::default();
    let mut run = |action: AppAction, session: &mut Session| {
        dispatch(
            action,
            Origin::Menu,
            &mut Targets {
                session,
                tools: &mut tools,
                locks: Locks::default(),
            },
        )
    };
    for action in [
        AppAction::FlipHorizontal,
        AppAction::FlipVertical,
        AppAction::TurnRight,
        AppAction::TurnLeft,
    ] {
        match run(action, &mut session) {
            Dispatched::Done { refresh: true, .. } => {}
            other => panic!("{action:?}: {other:?}"),
        }
    }
    assert!(matches!(
        run(AppAction::CanvasSize, &mut session),
        Dispatched::CanvasSize
    ));
}

// ---------------------------------------------------------------------------------------
// Row 12: save and reopen.
// ---------------------------------------------------------------------------------------

#[test]
fn row_12_a_turned_image_is_saved_with_the_new_size() {
    let dir = std::env::temp_dir().join(format!("pixoko-px-app-imageops-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("directory");
    let mut indexed = single(Mode::Indexed, Some(palette(None)));
    indexed.image_op(ImageOp::Cw);
    let path = dir.join("turned.png");
    indexed.save_to(&path).expect("save");
    let read = read_png(&std::fs::read(&path).expect("file"))
        .expect("png")
        .image;
    assert_eq!((read.width(), read.height()), (2, 3));
    // Two layers: the flattened PNG has the new size too.
    let mut layers = three_layers();
    layers.image_op(ImageOp::Ccw);
    let path = dir.join("layers.png");
    layers.save_to(&path).expect("save");
    let read = read_png(&std::fs::read(&path).expect("file"))
        .expect("png")
        .image;
    assert_eq!((read.width(), read.height()), (2, 3));
    let _ = std::fs::remove_dir_all(&dir);
}
