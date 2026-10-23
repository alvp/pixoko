//! Unit C4 (specs/ui-color-block.md), session part: row 4 (the right click on a bar), row 5 (the
//! one-use dropper carries the role of its bar), row 6 (the hexadecimal field writes to the role the
//! editor edits), the action `Editar → Intercambiar colores`, the model of the sliders that lasts
//! the session and property 4 (a gesture is one step and the document stays intact while it lasts).
//! No window: a `Session`, a `DropperOnce` and `dispatch`, as `lib.rs` does.

use super::*;

use proptest::prelude::*;
use px_core::{Image, Palette, Rgba};
use px_ui::actions::{AppAction, Locks, Origin};
use px_ui::color::{ColorChoice, Role};
use px_ui::color_edit::{Commit, Control, EditTarget, SliderMode};
use px_ui::pick::Picked;
use px_ui::tool::{Tool, ToolState};

use crate::dispatch::{Dispatched, Targets, dispatch};
use crate::dropper_once::DropperOnce;

fn indexed_image() -> Image {
    let colors: Vec<Rgba> = (0..6)
        .map(|i| Rgba::opaque((i * 30) as u8, 255 - (i * 9) as u8, (i * 7) as u8))
        .collect();
    Image::new_indexed(8, 4, Palette::from_colors(colors).expect("palette")).expect("image")
}

fn session() -> Session {
    let mut session = Session::sample(Document::new(indexed_image()));
    session.choice = ColorChoice::Entry(2);
    session.background = ColorChoice::Entry(4);
    session
}

fn rgba_session() -> Session {
    let image = Image::new(8, 4, Mode::Rgba).expect("image");
    let mut session = Session::sample(Document::new(image));
    session.choice = ColorChoice::Slot(3);
    session.background = ColorChoice::Slot(9);
    session
}

fn dispatches(session: &mut Session, action: AppAction, locks: Locks) -> Dispatched {
    dispatch(
        action,
        Origin::Menu,
        &mut Targets {
            session,
            tools: &mut ToolState::new(),
            locks,
        },
    )
}

// ---------------------------------------------------------------------------------------
// Row 4: the right click on a bar
// ---------------------------------------------------------------------------------------

#[test]
fn row_4_a_right_click_on_a_bar_opens_switches_or_closes_the_editor() {
    let mut session = session();
    session.bar_right_click(Role::Background);
    assert!(session.editor_open);
    assert_eq!(session.editing, Role::Background);
    // On the other bar the role changes without closing.
    session.bar_right_click(Role::Foreground);
    assert!(session.editor_open);
    assert_eq!(session.editing, Role::Foreground);
    // A second one on the edited bar closes the editor, and the role goes back to the foreground.
    session.bar_right_click(Role::Background);
    session.bar_right_click(Role::Background);
    assert!(!session.editor_open);
    assert_eq!(session.editing, Role::Foreground);
}

// ---------------------------------------------------------------------------------------
// Row 5: the one-use dropper carries the role of its bar
// ---------------------------------------------------------------------------------------

/// What the app does with a picked entry: `role_for` says which role gets it.
fn pick(session: &mut Session, once: &DropperOnce, button: Role, index: u8) {
    let role = once.role_for(button);
    session.apply_pick_for(Picked::Entry(index), role);
}

#[test]
fn row_5_the_color_goes_to_the_role_of_the_bar_with_either_button_and_nothing_else_changes() {
    for (bar, other) in [
        (Role::Background, Role::Foreground),
        (Role::Foreground, Role::Background),
    ] {
        for button in [Role::Foreground, Role::Background] {
            let mut session = session();
            let (state, undo) = (session.document.state_id(), session.document.can_undo());
            let kept = session.choice_of(other);
            let mut once = DropperOnce::new();
            once.arm(Tool::Pencil, bar);
            assert_eq!(once.role(), Some(bar));
            pick(&mut session, &once, button, 5);
            assert_eq!(
                session.choice_of(bar),
                ColorChoice::Entry(5),
                "bar {bar:?}, button {button:?}"
            );
            assert_eq!(
                session.choice_of(other),
                kept,
                "the other role does not change"
            );
            assert_eq!(session.document.state_id(), state);
            assert_eq!(session.document.can_undo(), undo);
            // At the release of its first gesture the tool goes back and the button decides again.
            assert_eq!(once.update(Tool::Dropper, true), None);
            assert_eq!(once.update(Tool::Dropper, false), Some(Tool::Pencil));
            assert_eq!(once.role(), None);
            assert_eq!(once.role_for(button), button);
        }
    }
    // Gray and RGBA: the slots of the role of the bar receive the color.
    let mut session = rgba_session();
    let mut once = DropperOnce::new();
    once.arm(Tool::Line, Role::Background);
    let role = once.role_for(Role::Foreground);
    session.apply_pick_for(Picked::Color(Rgba::opaque(1, 2, 3)), role);
    let mode = session.document.canvas_image().mode();
    assert_eq!(session.slots.get(mode).get(9), Some(Rgba::opaque(1, 2, 3)));
    assert_ne!(session.slots.get(mode).get(3), Some(Rgba::opaque(1, 2, 3)));
}

#[test]
fn row_5_choosing_another_tool_cancels_it_and_with_i_the_button_decides() {
    let mut once = DropperOnce::new();
    once.arm(Tool::Pencil, Role::Background);
    assert_eq!(
        once.update(Tool::Fill, false),
        None,
        "another tool cancels it"
    );
    assert_eq!(once.role(), None);
    for button in [Role::Foreground, Role::Background] {
        assert_eq!(once.role_for(button), button);
    }
    // The dropper chosen with `I` (or with the button of the toolbox) never armed anything.
    let mut session = session();
    pick(&mut session, &once, Role::Background, 1);
    assert_eq!(session.background, ColorChoice::Entry(1));
    assert_eq!(session.choice, ColorChoice::Entry(2));
    // Arming again from the dropper itself keeps the role of the new bar.
    once.arm(Tool::Dropper, Role::Foreground);
    assert_eq!(once.role_for(Role::Background), Role::Foreground);
}

// ---------------------------------------------------------------------------------------
// Row 6: the hexadecimal field writes to the role the editor edits
// ---------------------------------------------------------------------------------------

#[test]
fn row_6_a_commit_to_the_edited_role_is_one_step_in_indexed_and_none_in_slots() {
    let mut session = session();
    session.bar_right_click(Role::Background);
    session.sync_editor();
    assert_eq!(session.editor.target(), Some(EditTarget::Entry(4)));
    let state = session.document.state_id();
    let commit = Commit {
        target: EditTarget::Entry(4),
        color: Rgba::opaque(255, 136, 0),
    };
    assert!(session.commit_edit(commit).expect("applies"));
    assert_ne!(session.document.state_id(), state);
    session.undo().expect("one step undoes it");
    assert_eq!(session.document.state_id(), state);
    // The slots of RGBA are session state: no step and no change in the document.
    let mut session = rgba_session();
    session.bar_right_click(Role::Background);
    session.sync_editor();
    assert_eq!(session.editor.target(), Some(EditTarget::Slot(9)));
    let state = session.document.state_id();
    let commit = Commit {
        target: EditTarget::Slot(9),
        color: Rgba::new(255, 136, 0, 0),
    };
    assert!(!session.commit_edit(commit).expect("applies"));
    assert_eq!(session.document.state_id(), state);
    assert!(!session.document.can_undo());
    session.close_editor();
    session.sync_editor();
    assert_eq!(
        session.editor.target(),
        Some(EditTarget::Slot(3)),
        "back to the foreground"
    );
}

// ---------------------------------------------------------------------------------------
// The action Editar → Intercambiar colores
// ---------------------------------------------------------------------------------------

#[test]
fn the_swap_action_swaps_the_two_colors_without_history_and_the_editor_follows() {
    let mut session = session();
    session.bar_right_click(Role::Background);
    session.sync_editor();
    let (state, undo, modified) = (
        session.document.state_id(),
        session.document.can_undo(),
        session.modified(),
    );
    assert_eq!(session.editor.target(), Some(EditTarget::Entry(4)));
    let result = dispatches(&mut session, AppAction::SwapColors, Locks::default());
    assert!(matches!(result, Dispatched::Done { refresh: false, .. }));
    assert_eq!(
        (session.choice, session.background),
        (ColorChoice::Entry(4), ColorChoice::Entry(2))
    );
    // `editing` and the editor stay open; the editor now shows the other stored color.
    assert_eq!(
        (session.editing, session.editor_open),
        (Role::Background, true)
    );
    session.sync_editor();
    assert_eq!(session.editor.target(), Some(EditTarget::Entry(2)));
    assert_eq!(
        (
            session.document.state_id(),
            session.document.can_undo(),
            session.modified()
        ),
        (state, undo, modified),
        "session state: no step, no state_id"
    );
    // Swapping twice gives the original back.
    dispatches(&mut session, AppAction::SwapColors, Locks::default());
    assert_eq!(
        (session.choice, session.background),
        (ColorChoice::Entry(2), ColorChoice::Entry(4))
    );
    // Like the rest of the session actions it does nothing with a slider drag or a pending removal,
    // and in a modal or a dialog.
    for locks in [
        Locks {
            gesture: true,
            ..Locks::default()
        },
        Locks {
            removal: true,
            ..Locks::default()
        },
        Locks {
            modal: true,
            ..Locks::default()
        },
        Locks {
            dialog: true,
            ..Locks::default()
        },
    ] {
        let result = dispatches(&mut session, AppAction::SwapColors, locks);
        assert!(matches!(result, Dispatched::Blocked), "{locks:?}");
        assert_eq!(session.choice, ColorChoice::Entry(2));
    }
}

#[test]
fn the_model_of_the_sliders_lasts_the_session() {
    let mut session = session();
    assert_eq!(session.slider_mode, SliderMode::Rgb, "RGB by default");
    session.set_slider_mode(SliderMode::Hsv);
    assert_eq!(
        (session.slider_mode, session.view.slider_mode),
        (SliderMode::Hsv, SliderMode::Hsv)
    );
    // Closing and opening the editor, opening another document and converting the mode keep it.
    session.open_editor(Role::Foreground);
    session.close_editor();
    assert_eq!(session.slider_mode, SliderMode::Hsv);
    session
        .new_image(Image::new(4, 4, Mode::Rgba).expect("image"), true)
        .expect("a new document");
    assert_eq!(session.slider_mode, SliderMode::Hsv);
    assert_eq!(
        session.view.slider_mode,
        SliderMode::Hsv,
        "the view is not reset"
    );
    session
        .convert_mode(
            Mode::Gray,
            Allowed {
                luma: true,
                drop_alpha: Some([0, 0, 0]),
            },
        )
        .ok();
    assert_eq!(session.slider_mode, SliderMode::Hsv);
}

// ---------------------------------------------------------------------------------------
// Property 4: a gesture is one step and the document stays intact while it lasts
// ---------------------------------------------------------------------------------------

#[derive(Clone, Debug)]
enum Gesture {
    Square(f32, f32, f32, f32),
    Bar(Control, u32, u32),
    Slider(Control, u32, u32),
    Cancelled(f32, f32),
    Role(Role),
    Mode(SliderMode),
}

fn gesture() -> impl Strategy<Value = Gesture> {
    let control = prop::sample::select(vec![
        Control::Red,
        Control::Green,
        Control::Blue,
        Control::Hue,
        Control::Saturation,
        Control::Lightness,
        Control::Alpha,
    ]);
    prop_oneof![
        (0.0f32..=1.0, 0.0f32..=1.0, 0.0f32..=1.0, 0.0f32..=1.0)
            .prop_map(|(a, b, c, d)| Gesture::Square(a, b, c, d)),
        (
            prop::sample::select(vec![Control::HueV, Control::Alpha]),
            0u32..=400,
            0u32..=400
        )
            .prop_map(|(c, a, b)| Gesture::Bar(c, a, b)),
        (control, 0u32..=400, 0u32..=400).prop_map(|(c, a, b)| Gesture::Slider(c, a, b)),
        (0.0f32..=1.0, 0.0f32..=1.0).prop_map(|(s, v)| Gesture::Cancelled(s, v)),
        prop::sample::select(vec![Role::Foreground, Role::Background]).prop_map(Gesture::Role),
        prop::sample::select(vec![SliderMode::Rgb, SliderMode::Hsl, SliderMode::Hsv])
            .prop_map(Gesture::Mode),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn property_4_a_gesture_is_one_step_and_the_document_is_intact_while_it_lasts(
        gestures in prop::collection::vec(gesture(), 1..16),
    ) {
        let mut session = session();
        session.open_editor(Role::Foreground);
        let initial: Vec<Rgba> = (0..6u8)
            .map(|i| session.document.canvas_image().palette().unwrap().get(i).unwrap())
            .collect();
        let mut steps = 0usize;
        for gesture in gestures {
            session.sync_editor();
            let before = session.document.state_id();
            let began = match gesture {
                Gesture::Square(s, v, s2, v2) => {
                    let began = session.editor.begin_square(s, v);
                    session.editor.drag_square(s2, v2);
                    began
                }
                Gesture::Bar(c, a, b) | Gesture::Slider(c, a, b) => {
                    let began = session.editor.begin(c, a);
                    session.editor.drag(b);
                    began
                }
                Gesture::Cancelled(s, v) => {
                    let began = session.editor.begin_square(s, v);
                    prop_assert!(session.busy() == began);
                    session.editor.cancel();
                    prop_assert!(!session.busy());
                    false
                }
                Gesture::Role(role) => {
                    session.bar_right_click(role);
                    false
                }
                Gesture::Mode(mode) => {
                    session.set_slider_mode(mode);
                    false
                }
            };
            prop_assert_eq!(session.busy(), began, "busy is exactly a live drag");
            prop_assert_eq!(session.document.state_id(), before, "the document does not change during a gesture");
            if began && let Some(commit) = session.editor.release() {
                let changed = session.commit_edit(commit).expect("a valid commit");
                prop_assert!(changed, "a commit is only asked for a different color");
                steps += 1;
                prop_assert_eq!(session.document.state_id() != before, true);
            }
            prop_assert!(!session.busy());
        }
        // Undoing every step gives the initial palette back, exactly.
        for _ in 0..steps {
            session.undo().expect("a step to undo");
        }
        let after: Vec<Rgba> = (0..6u8)
            .map(|i| session.document.canvas_image().palette().unwrap().get(i).unwrap())
            .collect();
        prop_assert_eq!(after, initial);
    }
}
