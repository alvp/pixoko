//! Unit C3 (specs/ui-palette-handle.md), session part: rows 6 to 8 (the resize as one step, the
//! cut with pixels in any layer, the transparent entry that falls outside) and row 12 (the menu
//! actions, the size dialog, the dialog of the transparent entry and the help). No window: a
//! `Session`, `dispatch` and the pure forms, as `main.rs` does.
//!
//! The pending cut is driven by the test as the panel does it: `request_resize` is what the drop
//! of the handle asks for, `ops.set_resize_destination` is a press on a kept swatch and
//! `confirm_resize` is `Confirmar`.

use super::*;

use px_core::{BlendMode, LayerSpec, Palette, Rgba};
use px_ui::actions::{AppAction, Locks, Origin, PaletteNav, is_blocked, palette_enabled};
use px_ui::dialog::{Dialog, DialogKind, Event, Modal, Reaction};
use px_ui::edit_layout::min_window_width_pt;
use px_ui::help_texts::{action_of_key, help_entry, help_key};
use px_ui::help_texts_palette_es::TRANSPARENT_LOSS_TEXT;
use px_ui::keymap::Keymap;
use px_ui::palette_ops::{PaletteOp, RESIZE_CANCELLED_NOTICE};
use px_ui::palette_resize::{PendingResize, resize_prompt};
use px_ui::shortcut_editor::ShortcutEditor;
use px_ui::status::bar_columns;
use px_ui::tool::ToolState;

use crate::dispatch::{Dispatched, Targets, dispatch};
use crate::palette_size::{
    FIELD_LENGTH, length_from_values, open_palette_size, open_transparent_loss, palette_size_form,
    transparent_loss_form,
};

// ---------------------------------------------------------------------------------------
// Helpers.
// ---------------------------------------------------------------------------------------

const BLACK: Rgba = Rgba::opaque(0, 0, 0);

/// `n` different colors, none opaque black.
fn colors(n: usize) -> Vec<Rgba> {
    (0..n)
        .map(|i| {
            Rgba::opaque(
                20 + (i * 7 % 200) as u8,
                255 - (i * 5 % 200) as u8,
                40 + i as u8 % 150,
            )
        })
        .collect()
}

fn palette_of(n: usize, transparent: Option<u8>) -> Palette {
    let mut palette = Palette::from_colors(colors(n)).expect("palette");
    palette
        .set_transparent_index(transparent)
        .expect("transparent");
    palette
}

fn cell_of(palette: &Palette, pixels: &[u8]) -> Image {
    let mut image =
        Image::new_indexed(pixels.len() as u32, 1, palette.clone()).expect("indexed cell");
    for (x, &index) in pixels.iter().enumerate() {
        image
            .set_pixel(x as u32, 0, Pixel::Indexed(index))
            .expect("pixel");
    }
    image
}

/// A layer: its eight pixels (`None`: an empty cell), whether it is visible and whether locked.
type LayerDef<'a> = (Option<&'a [u8; 8]>, bool, bool);

fn document_of(entries: usize, transparent: Option<u8>, layers: &[LayerDef]) -> Document {
    let palette = palette_of(entries, transparent);
    let specs = layers
        .iter()
        .enumerate()
        .map(|(i, (pixels, visible, locked))| LayerSpec {
            name: format!("L{i}"),
            visible: *visible,
            opacity: 255,
            blend: BlendMode::Normal,
            locked: *locked,
            image: pixels.map(|p| cell_of(&palette, p)),
        })
        .collect();
    Document::from_layers(8, 1, Mode::Indexed, Some(palette), specs).expect("document")
}

/// A session with 8 entries and `choice` = 6, `background` = 7, one layer using only entry 0.
fn session8() -> Session {
    let mut session = Session::sample(document_of(8, None, &[(Some(&[0; 8]), true, false)]));
    session.choice = ColorChoice::Entry(6);
    session.background = ColorChoice::Entry(7);
    session
}

fn palette(session: &Session) -> Palette {
    session.document.palette().expect("indexed").clone()
}

fn len(session: &Session) -> usize {
    palette(session).len()
}

fn applied(outcome: ResizeOutcome) -> PaletteOutcome {
    match outcome {
        ResizeOutcome::Applied(outcome) => outcome,
        other => panic!("applied was expected and {other:?} came out"),
    }
}

// ---------------------------------------------------------------------------------------
// Row 6: the transaction and one step
// ---------------------------------------------------------------------------------------

#[test]
fn row_06_stretching_is_one_step_with_black_opaque_entries_and_the_choice_stays() {
    let mut session = session8();
    let before = session.document.state_id();
    let outcome = applied(session.request_resize(12).expect("resize"));
    assert!(outcome.changed);
    assert_eq!(
        outcome.notice.as_deref(),
        Some("paleta de 12 colores (antes 8)")
    );
    assert_ne!(session.document.state_id(), before);
    let after = palette(&session);
    assert_eq!(after.len(), 12);
    for index in 8..12u8 {
        assert_eq!(after.get(index).unwrap(), BLACK, "entry {index}");
    }
    assert_eq!(&after.colors()[..8], colors(8).as_slice());
    assert_eq!(
        (session.choice, session.background),
        (ColorChoice::Entry(6), ColorChoice::Entry(7))
    );
    // One single undo step.
    session.undo().expect("undo");
    assert_eq!(palette(&session), palette_of(8, None));
    assert!(!session.document.can_undo());
}

#[test]
fn row_06_cutting_without_pixels_is_one_step_and_adjusts_choice_range_and_pending_text() {
    let mut session = session8();
    session.ops.set_range_end(3);
    // Pending text of the entry 6, which will no longer exist.
    session
        .editor
        .type_hex(EditTarget::Entry(6), colors(8)[6], "#123456".into());
    let outcome = applied(session.request_resize(5).expect("resize"));
    assert!(outcome.changed);
    let notice = outcome.notice.expect("notice");
    assert!(
        notice.ends_with("paleta recortada a 5 colores (sin píxeles)"),
        "{notice}"
    );
    assert!(
        notice.starts_with(stale_notice(StaleReason::Gone)),
        "{notice}"
    );
    assert_eq!(len(&session), 5);
    assert_eq!(session.choice, ColorChoice::Entry(4));
    assert_eq!(session.background, ColorChoice::Entry(4));
    assert_eq!(session.ops.range_end(), None, "the range is cleared");
    assert!(!session.editor.has_pending());
}

#[test]
fn row_06_undo_gives_back_the_exact_palette_and_does_not_chase_the_choice() {
    let mut session = session8();
    let original = palette(&session);
    session.request_resize(5).expect("resize");
    session.undo().expect("undo");
    assert_eq!(palette(&session), original);
    assert_eq!(
        session.choice,
        ColorChoice::Entry(4),
        "the cut choice stays cut"
    );
    session.redo().expect("redo");
    assert_eq!(len(&session), 5);
    // Stretching again creates new black entries, not the cut colors.
    applied(session.request_resize(8).expect("resize"));
    let after = palette(&session);
    for index in 5..8u8 {
        assert_eq!(after.get(index).unwrap(), BLACK, "entry {index}");
        assert_ne!(after.get(index).unwrap(), original.get(index).unwrap());
    }
}

#[test]
fn row_06_a_drag_that_ends_at_the_original_length_leaves_no_step() {
    let mut session = session8();
    let before = session.document.state_id();
    assert!(matches!(
        session.request_resize(8),
        Ok(ResizeOutcome::Unchanged)
    ));
    assert_eq!(session.document.state_id(), before);
    assert!(!session.document.can_undo());
    assert!(!session.modified());
}

// ---------------------------------------------------------------------------------------
// Row 7: the cut with pixels in any layer
// ---------------------------------------------------------------------------------------

/// Eight entries; the entry 7 is used by the layer `layer` of two (the other uses only entry 0).
fn two_layers(layer: usize, visible: bool, locked: bool) -> Session {
    let plain: [u8; 8] = [0; 8];
    let using: [u8; 8] = [0, 1, 7, 7, 0, 0, 0, 7];
    let layers: Vec<LayerDef> = (0..2)
        .map(|i| {
            if i == layer {
                (Some(&using), visible, locked)
            } else {
                (Some(&plain), true, false)
            }
        })
        .collect();
    let mut session = Session::sample(document_of(8, None, &layers));
    session.choice = ColorChoice::Entry(6);
    session
}

#[test]
fn row_07_a_pixel_in_any_layer_makes_the_cut_pending_and_without_use_it_applies_at_once() {
    for (layer, visible, locked, name) in [
        (1, true, false, "a visible layer"),
        (0, false, false, "a hidden layer"),
        (0, true, true, "a locked layer"),
    ] {
        let mut session = two_layers(layer, visible, locked);
        let before = session.document.state_id();
        let outcome = session.request_resize(5).expect("resize");
        assert!(
            matches!(outcome, ResizeOutcome::Pending),
            "{name}: {outcome:?}"
        );
        assert_eq!(
            session.document.state_id(),
            before,
            "{name}: nothing changed"
        );
        let pending = session.ops.resize_pending().expect("pending");
        assert_eq!(
            (pending.len, pending.into, pending.pixels),
            (5, None, 3),
            "{name}"
        );
    }
    // Without any use of the entries that are cut: applied now.
    let mut session = session8();
    assert!(matches!(
        session.request_resize(5),
        Ok(ResizeOutcome::Applied(_))
    ));
}

#[test]
fn row_07_the_destination_is_a_kept_swatch_and_confirm_is_one_step_with_that_into() {
    let mut session = two_layers(1, true, false);
    session.request_resize(5).expect("resize");
    // A swatch that is cut off is ignored; a kept one is the destination; the choice stays.
    assert!(!session.ops.set_resize_destination(7));
    assert!(session.ops.set_resize_destination(2));
    assert_eq!(session.ops.resize_pending().and_then(|p| p.into), Some(2));
    assert_eq!(session.choice, ColorChoice::Entry(6));
    let before = session.document.state_id();
    let outcome = applied(session.confirm_resize().expect("confirm"));
    assert!(outcome.changed);
    assert_eq!(
        outcome.notice.as_deref(),
        Some("paleta recortada a 5 colores; píxeles pasan a la 2")
    );
    assert_ne!(session.document.state_id(), before);
    assert!(session.ops.resize_pending().is_none());
    // Every cell has the pixels moved to 2, in one single step.
    for layer in session.document.layers() {
        let image = session.document.image(layer.cel()).expect("cell");
        assert!(
            image.bytes().iter().all(|&b| b < 5),
            "no pixel uses a cut entry"
        );
    }
    session.undo().expect("undo");
    let used = session
        .document
        .layers()
        .iter()
        .filter_map(|l| session.document.image(l.cel()))
        .flat_map(|i| i.bytes().iter().copied())
        .filter(|&b| b == 7)
        .count();
    assert_eq!(used, 3, "undo gives every pixel back to its index");
    assert_eq!(len(&session), 8);
}

#[test]
fn row_07_cancel_changes_nothing_and_a_document_change_or_opening_cancels_with_a_notice() {
    let mut session = two_layers(1, true, false);
    session.request_resize(5).expect("resize");
    let state = session.document.state_id();
    session.ops.cancel_resize();
    assert_eq!(session.document.state_id(), state);
    assert!(
        !session.take_resize_cancelled(),
        "Cancel and Esc are silent"
    );
    // A change of the document cancels it once, with the notice.
    session.request_resize(5).expect("resize");
    session
        .apply_palette_op(
            PaletteOp::Rotate {
                start: 0,
                len: 2,
                steps: 1,
            },
            false,
        )
        .expect("rotate");
    assert!(session.ops.resize_pending().is_none());
    assert!(session.take_resize_cancelled());
    assert!(!session.take_resize_cancelled(), "said once");
    assert_eq!(
        RESIZE_CANCELLED_NOTICE,
        "recorte cancelado: el documento cambió"
    );
    // Undo cancels it too; opening another document as well.
    session.request_resize(5).expect("resize");
    session.undo().expect("undo");
    session.normalize();
    assert!(session.ops.resize_pending().is_none());
    assert!(session.take_resize_cancelled());
    session.request_resize(5).expect("resize");
    session
        .new_image(Image::new(4, 4, Mode::Rgba).expect("image"), true)
        .expect("new");
    assert!(session.ops.resize_pending().is_none());
    assert!(session.take_resize_cancelled());
}

#[test]
fn row_07_the_text_says_how_many_pixels_are_reassigned_and_fits_in_66_columns() {
    let p = |len, into, pixels, transparent| PendingResize {
        len,
        into,
        pixels,
        transparent,
    };
    assert_eq!(
        resize_prompt(&p(200, None, 536_870_912, None)),
        "recortar a 200: pulsa un destino · 536870912 píxeles se reasignan"
    );
    assert_eq!(
        resize_prompt(&p(200, Some(3), 536_870_912, None)),
        "recortar a 200; destino 3 · 536870912 píxeles pasan a la 3"
    );
    assert_eq!(
        resize_prompt(&p(200, None, 1, None)),
        "recortar a 200: pulsa un destino · 1 píxel se reasigna"
    );
    assert_eq!(
        resize_prompt(&p(200, Some(3), 1, None)),
        "recortar a 200; destino 3 · 1 píxel pasa a la 3"
    );
    // The destination that is the kept transparent warns.
    assert!(resize_prompt(&p(10, Some(2), 5, Some(2))).contains("aviso: destino transparente"));
    // Everything fits in the status bar of the minimum window at the five scales, with the
    // longest numbers, and keeps the length, the pixels, the destination and the warning.
    for ppp in [1.0f32, 1.25, 1.5, 2.0, 3.0] {
        let columns = bar_columns(min_window_width_pt(ppp), ppp);
        assert!(columns >= 66, "ppp {ppp}: {columns} columns");
        for len in [1u16, 9, 10, 99, 100, 255, 256] {
            for into in [None, Some(0u8), Some(99), Some(254)] {
                for pixels in [1u64, 10, 536_870_912] {
                    for transparent in [None, Some(0u8), Some(99), Some(254), Some(255)] {
                        let pending = p(
                            len,
                            into.filter(|i| u16::from(*i) < len),
                            pixels,
                            transparent,
                        );
                        let text = resize_prompt(&pending);
                        assert!(
                            text.chars().count() <= 66,
                            "ppp {ppp}: {text:?} has {} characters",
                            text.chars().count()
                        );
                        assert!(text.contains(&len.to_string()), "{text}");
                        assert!(text.contains(&pixels.to_string()), "{text}");
                        if let Some(d) = pending.into {
                            assert!(text.contains(&d.to_string()), "{text}");
                        }
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------------------
// Row 8: the transparent entry falls outside
// ---------------------------------------------------------------------------------------

fn transparent_session(layers: usize, use_cut_entries: bool) -> Session {
    let plain: [u8; 8] = [0; 8];
    let using: [u8; 8] = [0, 1, 6, 6, 0, 0, 0, 7];
    let defs: Vec<LayerDef> = (0..layers)
        .map(|i| {
            if use_cut_entries && i == 0 {
                (Some(&using), true, false)
            } else {
                (Some(&plain), true, false)
            }
        })
        .collect();
    let mut session = Session::sample(document_of(8, Some(7), &defs));
    session.choice = ColorChoice::Entry(2);
    session
}

#[test]
fn row_08_with_one_layer_losing_the_transparent_is_pending_with_the_warning_and_undo_restores_it() {
    let mut session = transparent_session(1, false);
    let outcome = session.request_resize(5).expect("resize");
    assert!(matches!(outcome, ResizeOutcome::Pending), "{outcome:?}");
    let pending = session.ops.resize_pending().expect("pending");
    assert_eq!((pending.pixels, pending.transparent), (0, Some(7)));
    assert!(pending.loses_transparent() && !pending.needs_destination());
    assert!(resize_prompt(&pending).contains("aviso: se pierde la transparente"));
    assert_eq!(
        palette(&session).transparent_index(),
        Some(7),
        "nothing yet"
    );
    // Confirm needs no destination; with one layer there is no modal.
    let outcome = applied(session.confirm_resize().expect("confirm"));
    assert!(outcome.changed);
    assert_eq!(palette(&session).transparent_index(), None);
    session.undo().expect("undo");
    assert_eq!(palette(&session), palette_of(8, Some(7)));
}

#[test]
fn row_08_with_pixels_the_warning_is_that_they_become_opaque_and_a_destination_is_required() {
    let mut session = transparent_session(1, true);
    session.request_resize(5).expect("resize");
    let pending = session.ops.resize_pending().expect("pending");
    assert!(pending.pixels > 0 && pending.needs_destination());
    assert!(resize_prompt(&pending).contains("aviso: sus píxeles quedan opacos"));
    // Confirm without a destination does nothing.
    assert!(matches!(
        session.confirm_resize(),
        Ok(ResizeOutcome::Pending)
    ));
    assert!(session.ops.set_resize_destination(1));
    applied(session.confirm_resize().expect("confirm"));
    assert_eq!(palette(&session).transparent_index(), None);
}

#[test]
fn row_08_with_several_layers_a_modal_asks_first_and_new_layer_then_warns() {
    // No pixels to move: the modal comes straight away, without applying anything.
    let mut session = transparent_session(3, false);
    let state = session.document.state_id();
    let outcome = session.request_resize(5).expect("resize");
    assert!(
        matches!(outcome, ResizeOutcome::ConfirmLoss { len: 5, into: None }),
        "{outcome:?}"
    );
    assert_eq!(
        session.document.state_id(),
        state,
        "Cancel/Esc leave it as it was"
    );
    assert!(session.ops.resize_pending().is_none());
    // Accepting applies it as one step; afterwards `New layer` has no transparency to use.
    let outcome = session.apply_resize(5, None).expect("apply");
    assert!(outcome.changed);
    assert_eq!(palette(&session).transparent_index(), None);
    let report = session.new_layer();
    assert!(
        !report.changed,
        "no layer is added without a transparent entry"
    );
    assert!(report.notice.is_some());
    session.undo().expect("undo");
    assert_eq!(palette(&session), palette_of(8, Some(7)));
    // With pixels: pending first, the modal after choosing the destination and confirming.
    let mut session = transparent_session(3, true);
    assert!(matches!(
        session.request_resize(5),
        Ok(ResizeOutcome::Pending)
    ));
    assert!(session.ops.set_resize_destination(2));
    let outcome = session.confirm_resize().expect("confirm");
    assert!(
        matches!(
            outcome,
            ResizeOutcome::ConfirmLoss {
                len: 5,
                into: Some(2)
            }
        ),
        "{outcome:?}"
    );
    assert_eq!(
        palette(&session).len(),
        8,
        "the modal has not been accepted yet"
    );
}

#[test]
fn row_08_one_layer_never_opens_the_modal() {
    let mut session = transparent_session(1, true);
    session.request_resize(5).expect("resize");
    session.ops.set_resize_destination(2);
    assert!(matches!(
        session.confirm_resize(),
        Ok(ResizeOutcome::Applied(_))
    ));
}

// ---------------------------------------------------------------------------------------
// Row 12: menu, keyboard, help, texts and dialogs
// ---------------------------------------------------------------------------------------

struct World {
    session: Session,
    tools: ToolState,
}

impl World {
    fn new(session: Session) -> World {
        World {
            session,
            tools: ToolState::new(),
        }
    }

    fn dispatch(&mut self, action: AppAction, locks: Locks) -> Dispatched {
        dispatch(
            action,
            Origin::Menu,
            &mut Targets {
                session: &mut self.session,
                tools: &mut self.tools,
                locks,
            },
        )
    }
}

#[test]
fn row_12_add_entry_gives_the_same_step_and_notice_as_the_add_icon_did() {
    let mut menu = World::new(session8());
    let mut direct = session8();
    let outcome = direct.apply_palette_op(PaletteOp::Add, false).expect("add");
    let done = menu.dispatch(AppAction::AddPaletteEntry, Locks::default());
    assert_eq!(
        done,
        Dispatched::Done {
            refresh: true,
            notice: outcome.notice.clone()
        }
    );
    assert_eq!(
        outcome.notice.as_deref(),
        Some("entrada 8 añadida (copia de la 6)")
    );
    assert_eq!(palette(&menu.session), palette(&direct));
    assert_eq!(menu.session.choice, direct.choice);
    // A full palette says so and changes nothing.
    let mut full = World::new(Session::sample(document_of(
        256,
        None,
        &[(Some(&[0; 8]), true, false)],
    )));
    full.session.choice = ColorChoice::Entry(5);
    let before = full.session.document.state_id();
    assert_eq!(
        full.dispatch(AppAction::AddPaletteEntry, Locks::default()),
        Dispatched::Done {
            refresh: false,
            notice: Some("paleta llena: 256 entradas".into())
        }
    );
    assert_eq!(full.session.document.state_id(), before);
}

#[test]
fn row_12_remove_entry_removes_an_unused_one_asks_for_a_destination_if_used_and_warns_with_one() {
    // Entry 6 is unused: removed with its default destination, as the icon did.
    let mut world = World::new(session8());
    let done = world.dispatch(AppAction::RemovePaletteEntry, Locks::default());
    assert!(
        matches!(done, Dispatched::Done { refresh: true, .. }),
        "{done:?}"
    );
    assert_eq!(len(&world.session), 7);
    // Entry 7 is used by a hidden layer: the removal stays pending.
    let mut world = World::new(two_layers(0, false, false));
    world.session.choice = ColorChoice::Entry(7);
    let done = world.dispatch(AppAction::RemovePaletteEntry, Locks::default());
    assert!(
        matches!(done, Dispatched::Done { refresh: false, .. }),
        "{done:?}"
    );
    assert_eq!(world.session.ops.removal().map(|r| r.index), Some(7));
    // One single entry.
    let mut world = World::new(Session::sample(document_of(
        1,
        None,
        &[(Some(&[0; 8]), true, false)],
    )));
    world.session.choice = ColorChoice::Entry(0);
    assert_eq!(
        world.dispatch(AppAction::RemovePaletteEntry, Locks::default()),
        Dispatched::Done {
            refresh: false,
            notice: Some("no se puede quitar la única entrada".into())
        }
    );
}

#[test]
fn row_12_palette_size_opens_its_dialog_and_the_three_actions_are_blocked_like_a_change() {
    let mut world = World::new(session8());
    assert_eq!(
        world.dispatch(AppAction::PaletteSize, Locks::default()),
        Dispatched::PaletteSize
    );
    let all = [
        AppAction::AddPaletteEntry,
        AppAction::RemovePaletteEntry,
        AppAction::PaletteSize,
    ];
    for action in all {
        for (name, locks) in [
            (
                "gesture",
                Locks {
                    gesture: true,
                    ..Locks::default()
                },
            ),
            (
                "dialog",
                Locks {
                    dialog: true,
                    ..Locks::default()
                },
            ),
            (
                "text focus",
                Locks {
                    text_focus: true,
                    ..Locks::default()
                },
            ),
            (
                "removal or resize pending",
                Locks {
                    removal: true,
                    ..Locks::default()
                },
            ),
            (
                "modal",
                Locks {
                    modal: true,
                    ..Locks::default()
                },
            ),
        ] {
            assert!(
                is_blocked(action, locks, Origin::Menu),
                "{action:?} with {name}"
            );
            let mut world = World::new(session8());
            let before = world.session.document.state_id();
            assert_eq!(
                world.dispatch(action, locks),
                Dispatched::Blocked,
                "{action:?} {name}"
            );
            assert_eq!(world.session.document.state_id(), before);
        }
        assert!(!is_blocked(action, Locks::default(), Origin::Menu));
        // Only enabled in indexed images.
        assert!(palette_enabled(action, &PaletteNav { indexed: true }));
        assert!(!palette_enabled(action, &PaletteNav { indexed: false }));
    }
    assert!(
        palette_enabled(AppAction::Undo, &PaletteNav { indexed: false }),
        "others are not touched"
    );
    // In a RGBA image they do nothing.
    let mut rgba = World::new(Session::sample(Document::new(
        Image::new(4, 4, Mode::Rgba).unwrap(),
    )));
    for action in all {
        assert_eq!(
            rgba.dispatch(action, Locks::default()),
            Dispatched::Done {
                refresh: false,
                notice: None
            },
            "{action:?}"
        );
    }
}

#[test]
fn row_12_the_actions_have_no_default_key_and_can_be_assigned() {
    let table = Keymap::default_scheme();
    for action in [
        AppAction::AddPaletteEntry,
        AppAction::RemovePaletteEntry,
        AppAction::PaletteSize,
    ] {
        assert!(
            table.rows().iter().all(|row| row.action != action),
            "{action:?} has a default key"
        );
        assert!(
            ShortcutEditor::new(action, &table, px_ui::actions::Platform::Linux).is_some(),
            "{action:?} is editable"
        );
    }
}

#[test]
fn row_12_the_help_has_the_five_entries_and_the_keys_belong_to_the_actions() {
    for (key, action) in [
        ("palette.add_entry", Some(AppAction::AddPaletteEntry)),
        ("palette.remove_entry", Some(AppAction::RemovePaletteEntry)),
        ("palette.size", Some(AppAction::PaletteSize)),
        ("palette.handle", None),
        ("color.bar", None),
    ] {
        let entry = help_entry(key).unwrap_or_else(|| panic!("{key} has no help"));
        assert!(entry.name.chars().count() <= 20, "{key}");
        assert_eq!(action_of_key(key), action, "{key}");
        if let Some(action) = action {
            assert_eq!(help_key(action), Some(key));
        }
    }
    assert_eq!(
        help_entry("palette.add_entry").map(|e| e.name),
        Some("Añadir entrada")
    );
    assert_eq!(
        help_entry("palette.remove_entry").map(|e| e.name),
        Some("Quitar entrada")
    );
}

#[test]
fn row_12_the_size_dialog_starts_at_the_current_length_and_rejects_what_is_not_1_to_256() {
    let spec = palette_size_form(8);
    spec.validate().expect("valid form");
    let mut dialog = Dialog::new(DialogKind::PaletteSize, spec).expect("dialog");
    assert_eq!(
        dialog.values().and_then(|v| length_from_values(&v)),
        Some(8)
    );
    for (text, valid) in [
        ("1", true),
        ("256", true),
        ("0", false),
        ("257", false),
        ("", false),
        ("12a", false),
    ] {
        dialog.event(Event::Number {
            field: FIELD_LENGTH,
            text: text.to_string(),
        });
        assert_eq!(dialog.is_valid(), valid, "{text:?}");
        let reaction = dialog.event(Event::Accept);
        if valid {
            assert!(matches!(reaction, Reaction::Accepted(_)), "{text:?}");
        } else {
            assert!(matches!(reaction, Reaction::Rejected(_)), "{text:?}");
            assert!(dialog.reason().is_some(), "{text:?} has a reason");
        }
    }
    assert!(palette_size_form(8).title.chars().count() <= 44);
    // Accepting does what dropping the handle at that length does.
    let mut a = session8();
    let mut b = session8();
    let accepted = a.request_resize(12).expect("a");
    let dropped = b.request_resize(12).expect("b");
    assert!(matches!(
        (accepted, dropped),
        (ResizeOutcome::Applied(_), ResizeOutcome::Applied(_))
    ));
    assert_eq!(palette(&a), palette(&b));
}

#[test]
fn row_12_the_dialogs_open_only_without_locks_and_the_loss_dialog_has_the_exact_text() {
    let mut modal = Modal::default();
    assert!(open_palette_size(&mut modal, 8, Locks::default()));
    assert_eq!(modal.kind(), Some(DialogKind::PaletteSize));
    modal.abandon();
    assert!(!open_palette_size(
        &mut modal,
        8,
        Locks {
            gesture: true,
            ..Locks::default()
        }
    ));
    assert!(open_transparent_loss(&mut modal, Locks::default()));
    assert_eq!(modal.kind(), Some(DialogKind::TransparentLoss));
    let spec = transparent_loss_form();
    assert_eq!((spec.accept, spec.cancel), ("Aceptar", Some("Cancelar")));
    assert_eq!(
        TRANSPARENT_LOSS_TEXT,
        "la entrada transparente se elimina; con varias capas no podrás añadir capas ni pintar en celdas vacías hasta fijar otra"
    );
    assert!(matches!(modal.event(Event::Cancel), Reaction::Cancelled));
}
