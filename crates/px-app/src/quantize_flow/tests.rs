//! Unit 4.2 (specs/hito-4-cuantizador.md), app part: row 10 (the form, the plan, the two-step
//! accept, the checks and the button of `Convertir modo…`) and row 11 (the session and `dispatch`).
//! No window.
//!
//! Red phase: `quantize_flow`, `Session::quantize`, `Session::saved_palette`, `FIELD_QUANTIZE` and
//! `Dispatched::Quantize` do not exist yet, so this file does not compile until the logic.

use super::*;

use px_core::{BlendMode, Document, Image, LayerSpec, Mode, Palette, Pixel, Rgba};
use px_raster::Dither;
use px_ui::actions::{AppAction, Locks, Origin};
use px_ui::dialog::{Dialog, DialogKind, Event, FieldKind, Modal, Reaction};
use px_ui::tool::ToolState;

use crate::convert_mode::{
    FIELD_QUANTIZE, FIELD_TARGET, SYNC_PIXELS, WAIT_FRAMES, convert_form_layers, quantize_offer,
    targets,
};
use crate::dispatch::{Dispatched, Targets, dispatch};
use crate::session::Session;

fn many_colors(seed: u8) -> Image {
    let mut image = Image::new(8, 8, Mode::Rgba).expect("image");
    for y in 0..8u32 {
        for x in 0..8u32 {
            let v = (x * 31 + y * 7) as u8;
            image
                .set_pixel(x, y, Pixel::Rgba(Rgba::new(v, v.wrapping_mul(5).wrapping_add(seed), (y * 30) as u8, 255)))
                .expect("pixel");
        }
    }
    image
}

fn layer(visible: bool, locked: bool, opacity: u8, blend: BlendMode, image: Option<Image>) -> LayerSpec {
    LayerSpec {
        name: "layer".to_string(),
        visible,
        opacity,
        blend,
        locked,
        image,
    }
}

/// Three layers of 8 × 8: plain, empty, and hidden and locked (opacity 255, Normal).
fn three_layers() -> Session {
    let layers = vec![
        layer(true, false, 255, BlendMode::Normal, Some(many_colors(1))),
        layer(true, false, 255, BlendMode::Normal, None),
        layer(false, true, 255, BlendMode::Normal, Some(many_colors(90))),
    ];
    Session::sample(Document::from_layers(8, 8, Mode::Rgba, None, layers).expect("document"))
}

fn one_layer(opacity: u8, blend: BlendMode) -> Session {
    let layers = vec![layer(true, false, opacity, blend, Some(many_colors(3)))];
    Session::sample(Document::from_layers(8, 8, Mode::Rgba, None, layers).expect("document"))
}

fn palette() -> Palette {
    Palette::from_colors((0..8u8).map(|i| Rgba::opaque(i * 30, 255 - i * 30, i))).expect("palette")
}

fn options(colors: u16) -> px_raster::QuantizeOptions {
    px_raster::QuantizeOptions {
        colors,
        dither: Dither::None,
        alpha_threshold: 128,
        palette: px_raster::PaletteSource::Computed,
    }
}

// ---------------------------------------------------------------------------------------
// Row 10: the form and the plan.
// ---------------------------------------------------------------------------------------

#[test]
fn row_10_the_form_has_the_defaults_of_d11_and_validates() {
    for saved in [None, Some(8)] {
        let spec = quantize_form(64, 64, 1, saved);
        spec.validate().expect("valid form");
        let find = |id| spec.fields.iter().find(|f| f.id == id).unwrap_or_else(|| panic!("{id}"));
        match &find(FIELD_COLORS).kind {
            FieldKind::Number { min, max, default } => assert_eq!((*min, *max, *default), (2, 256, 256)),
            other => panic!("{other:?}"),
        }
        match &find(FIELD_THRESHOLD).kind {
            FieldKind::Number { min, max, default } => assert_eq!((*min, *max, *default), (1, 255, 128)),
            other => panic!("{other:?}"),
        }
        match &find(FIELD_DITHER).kind {
            FieldKind::Choice { options, default } => assert_eq!((options.len(), *default), (3, 0)),
            other => panic!("{other:?}"),
        }
        match &find(FIELD_PALETTE).kind {
            FieldKind::Choice { options, default } => {
                assert_eq!((options.len(), *default), (1 + usize::from(saved.is_some()), 0));
            }
            other => panic!("{other:?}"),
        }
        assert!(matches!(find(FIELD_REPORT).kind, FieldKind::ReadOnly { .. }));
    }
}

#[test]
fn row_10_the_defaults_make_the_plan_and_the_bounds_are_rejected() {
    let spec = quantize_form(64, 64, 1, Some(8));
    let mut dialog = Dialog::new(DialogKind::Quantize, spec).expect("dialog");
    assert_eq!(
        dialog.values().and_then(|v| plan_from_values(&v)),
        Some(QuantizePlan {
            colors: 256,
            dither: Dither::None,
            threshold: 128,
            saved_palette: false
        })
    );
    for (field, text, valid) in [
        (FIELD_COLORS, "2", true),
        (FIELD_COLORS, "256", true),
        (FIELD_COLORS, "1", false),
        (FIELD_COLORS, "0", false),
        (FIELD_COLORS, "300", false),
        (FIELD_COLORS, "", false),
        (FIELD_THRESHOLD, "1", true),
        (FIELD_THRESHOLD, "255", true),
        (FIELD_THRESHOLD, "0", false),
        (FIELD_THRESHOLD, "256", false),
    ] {
        dialog.event(Event::Number { field, text: text.to_string() });
        let accepted = matches!(dialog.event(Event::Accept), Reaction::Accepted(_));
        assert_eq!(accepted, valid, "{field} {text:?}");
        if !valid {
            assert!(dialog.reason().is_some(), "{field} {text:?} has a reason");
        }
        // Back to a valid value for the next case.
        dialog.event(Event::Number { field, text: (if field == FIELD_COLORS { "16" } else { "50" }).to_string() });
    }
    dialog.event(Event::Choice { field: FIELD_DITHER, index: 2 });
    dialog.event(Event::Choice { field: FIELD_PALETTE, index: 1 });
    let Reaction::Accepted(values) = dialog.event(Event::Accept) else {
        panic!("accepted");
    };
    assert_eq!(
        plan_from_values(&values),
        Some(QuantizePlan {
            colors: 16,
            dither: Dither::Ordered,
            threshold: 50,
            saved_palette: true
        })
    );
    dialog.event(Event::Choice { field: FIELD_DITHER, index: 1 });
    let Reaction::Accepted(values) = dialog.event(Event::Accept) else {
        panic!("accepted");
    };
    assert_eq!(plan_from_values(&values).map(|p| p.dither), Some(Dither::FloydSteinberg));
}

#[test]
fn row_10_options_use_the_saved_palette_only_when_asked_and_present() {
    let plan = QuantizePlan {
        colors: 32,
        dither: Dither::FloydSteinberg,
        threshold: 100,
        saved_palette: false,
    };
    let computed = options_of(&plan, None).expect("computed");
    assert_eq!((computed.colors, computed.dither, computed.alpha_threshold), (32, Dither::FloydSteinberg, 100));
    assert!(matches!(computed.palette, px_raster::PaletteSource::Computed));
    let saved = palette();
    let asked = QuantizePlan { saved_palette: true, ..plan };
    match options_of(&asked, Some(&saved)).expect("fixed").palette {
        px_raster::PaletteSource::Fixed(p) => assert_eq!(p, saved),
        other => panic!("{other:?}"),
    }
    assert!(options_of(&asked, None).is_none(), "nothing saved");
}

#[test]
fn row_10_the_dialog_opens_only_without_locks() {
    let mut modal = Modal::default();
    assert!(open_quantize(&mut modal, 8, 8, 1, None, Locks::default()));
    assert_eq!(modal.kind(), Some(DialogKind::Quantize));
    modal.abandon();
    let busy = Locks { gesture: true, ..Locks::default() };
    assert!(!open_quantize(&mut modal, 8, 8, 1, None, busy));
}

#[test]
fn row_10_a_big_image_is_quantized_two_frames_after_accepting() {
    let plan = QuantizePlan { colors: 8, dither: Dither::None, threshold: 128, saved_palette: false };
    assert_eq!(start_quantize(plan, SYNC_PIXELS), QuantizeStart::Now(plan));
    let QuantizeStart::Later(mut pending) = start_quantize(plan, SYNC_PIXELS + 1) else {
        panic!("later");
    };
    assert_eq!(pending.plan(), plan);
    for _ in 1..WAIT_FRAMES {
        assert_eq!(pending.tick(), None);
    }
    assert_eq!(pending.tick(), Some(plan));
    assert_eq!(pending.tick(), None, "it does not repeat");
}

#[test]
fn row_10_only_rgba_and_gray_alpha_without_layer_properties_can_be_reduced() {
    assert_eq!(can_quantize(&one_layer(255, BlendMode::Normal).document), Ok(()));
    let gray_alpha = Document::new(Image::filled(2, 2, Pixel::GrayAlpha { gray: 4, alpha: 255 }).unwrap());
    assert_eq!(can_quantize(&gray_alpha), Ok(()));
    let indexed = Document::new(Image::filled_indexed(2, 2, palette(), 0).unwrap());
    assert_eq!(can_quantize(&indexed), Err(px_ui::quantize_es::QUANTIZE_ONLY_COLOR.to_string()));
    let gray = Document::new(Image::filled(2, 2, Pixel::Gray(4)).unwrap());
    assert!(can_quantize(&gray).is_err());
    assert!(can_quantize(&one_layer(128, BlendMode::Normal).document).is_err());
    assert!(can_quantize(&one_layer(255, BlendMode::Multiply).document).is_err());
}

#[test]
fn row_10_the_notice_reports_the_colors_and_the_mean_error() {
    assert_eq!(done_notice(16, Some(3400)), "colores reducidos a 16 (error medio 3,4)");
    assert_eq!(done_notice(2, Some(0)), "colores reducidos a 2 (error medio 0,0)");
    assert_eq!(done_notice(5, None), "colores reducidos a 5");
}

#[test]
fn row_10_convert_mode_offers_the_quantizer_with_an_indexed_target_only_for_color_sources() {
    for from in [Mode::Rgba, Mode::GrayAlpha] {
        let spec = convert_form_layers(from, 1);
        let button = spec.fields.iter().find(|f| f.id == FIELD_QUANTIZE).expect("button");
        match &button.kind {
            FieldKind::Button { text, aside } => {
                assert_eq!(text, "Reducir colores…");
                assert_eq!(*aside, None);
            }
            other => panic!("{other:?}"),
        }
        let indexed = targets(from).iter().position(|m| *m == Mode::Indexed).expect("Indexado");
        let when = button.shown_when.expect("conditioned on the target");
        assert_eq!((when.field, when.choice), (FIELD_TARGET, indexed), "{from:?}");
    }
    for from in [Mode::Indexed, Mode::Gray] {
        let spec = convert_form_layers(from, 1);
        assert!(spec.fields.iter().all(|f| f.id != FIELD_QUANTIZE), "{from:?}");
    }
    assert_eq!(quantize_offer(FIELD_QUANTIZE), Some(AppAction::Quantize));
    assert_eq!(quantize_offer(FIELD_TARGET), None);
}

// ---------------------------------------------------------------------------------------
// Row 11: the session.
// ---------------------------------------------------------------------------------------

#[test]
fn row_11_quantizing_is_one_step_for_every_layer_and_undo_restores_it() {
    let mut session = three_layers();
    let (before, active) = (session.document.clone(), session.active_layer());
    let saved_id = session.document.state_id();
    let report = session.quantize(&options(16));
    assert!(report.changed, "{:?}", report.notice);
    let notice = report.notice.expect("notice");
    assert!(notice.starts_with("colores reducidos a "), "{notice}");
    assert!(notice.contains("error medio"), "{notice}");
    assert_eq!(session.document.mode(), Mode::Indexed);
    assert_ne!(session.document.state_id(), saved_id);
    assert!(session.modified());
    assert_eq!(session.active_layer(), active, "the active layer does not change");
    let layers = session.document.layers();
    assert!(session.document.image(layers[1].cel()).is_none(), "the empty cell stays empty");
    assert!(session.document.image(layers[2].cel()).is_some(), "hidden and locked ones too");
    session.undo().expect("undo");
    assert_eq!(session.document, before);
    assert!(!session.document.can_undo(), "it was one step");
    session.redo().expect("redo");
    assert_eq!(session.document.mode(), Mode::Indexed);
}

#[test]
fn row_11_a_refusal_gives_a_notice_and_leaves_the_session_intact() {
    for mut session in [one_layer(128, BlendMode::Normal), one_layer(255, BlendMode::Multiply)] {
        let (before, id) = (session.document.clone(), session.document.state_id());
        let report = session.quantize(&options(16));
        assert!(!report.changed);
        assert!(report.notice.is_some());
        assert_eq!((&session.document, session.document.state_id()), (&before, id));
    }
    let mut gray = Session::sample(Document::new(Image::filled(2, 2, Pixel::Gray(4)).unwrap()));
    let report = gray.quantize(&options(16));
    assert!(!report.changed);
    assert!(report.notice.is_some());
}

#[test]
fn row_11_leaving_an_indexed_document_keeps_its_palette_for_the_next_reduction() {
    let saved = palette();
    let mut session = Session::sample(Document::new(Image::filled_indexed(3, 2, saved.clone(), 0).unwrap()));
    session.convert_mode(Mode::Rgba, px_core::Allowed::NOTHING).expect("convert");
    assert_eq!(session.saved_palette(), Some(&saved));
    // Reducing with that palette gives exactly that palette.
    let plan = QuantizePlan { colors: 8, dither: Dither::None, threshold: 128, saved_palette: true };
    let report = session.quantize(&options_of(&plan, session.saved_palette()).expect("options"));
    assert!(report.changed, "{:?}", report.notice);
    assert_eq!(session.document.palette(), Some(&saved));
}

#[test]
fn row_11_dispatch_asks_the_app_to_open_the_dialog() {
    let mut session = three_layers();
    let mut tools = ToolState::default();
    let result = dispatch(
        AppAction::Quantize,
        Origin::Menu,
        &mut Targets { session: &mut session, tools: &mut tools, locks: Locks::default() },
    );
    assert!(matches!(result, Dispatched::Quantize), "{result:?}");
    let busy = Locks { dialog: true, ..Locks::default() };
    let blocked = dispatch(
        AppAction::Quantize,
        Origin::Menu,
        &mut Targets { session: &mut session, tools: &mut tools, locks: busy },
    );
    assert!(matches!(blocked, Dispatched::Blocked), "{blocked:?}");
}
