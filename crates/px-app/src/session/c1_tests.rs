//! Unit C1 (specs/ui-composition.md), session part: rows 5 to 10 (opening and the four ways of
//! closing the color editor, and what does not close it), row 14 (the status line and the text of a
//! pending removal) and property 2 (opening and closing never touch the document or the choice).
//! No window: a `Session` and `dispatch`, as `main.rs` does.

use super::*;

use proptest::prelude::*;
use px_core::{Image, Palette, Rgba};
use px_ui::actions::{AppAction, Locks, Origin};
use px_ui::color::{ColorChoice, Role};
use px_ui::edit_layout::min_window_width_pt;
use px_ui::palette_ops::{RemovalWarning, removal_prompt};
use px_ui::status::bar_columns;
use px_ui::theme::NOTICE_PREFIX;
use px_ui::tool::{Tool, ToolState};

use crate::dispatch::{Dispatched, Targets, dispatch};
use crate::title::status_for;

fn indexed_image() -> Image {
    let colors: Vec<Rgba> = (0..6)
        .map(|i| Rgba::opaque((i * 30) as u8, 255 - (i * 9) as u8, (i * 7) as u8))
        .collect();
    Image::new_indexed(8, 4, Palette::from_colors(colors).expect("palette")).expect("image")
}

struct World {
    session: Session,
    tools: ToolState,
}

impl World {
    fn new() -> World {
        let mut session = Session::sample(Document::new(indexed_image()));
        session.choice = ColorChoice::Entry(2);
        session.background = ColorChoice::Entry(4);
        World {
            session,
            tools: ToolState::new(),
        }
    }

    fn dispatches(&mut self, action: AppAction, origin: Origin, locks: Locks) -> Dispatched {
        dispatch(
            action,
            origin,
            &mut Targets {
                session: &mut self.session,
                tools: &mut self.tools,
                locks,
            },
        )
    }

    fn press_p(&mut self) -> Dispatched {
        self.dispatches(AppAction::ToggleColorEditor, Origin::Key, Locks::default())
    }
}

// ---------------------------------------------------------------------------------------
// Row 5: opening
// ---------------------------------------------------------------------------------------

#[test]
fn row_5_the_editor_starts_closed_and_p_toggles_it_keeping_the_role() {
    let mut world = World::new();
    assert!(!world.session.editor_open, "starts closed");
    assert_eq!(world.session.editing, Role::Foreground);
    assert!(matches!(world.press_p(), Dispatched::Done { .. }));
    assert!(world.session.editor_open);
    assert_eq!(world.session.editing, Role::Foreground);
    world.press_p();
    assert!(!world.session.editor_open);
    assert_eq!(world.session.editing, Role::Foreground);
}

#[test]
fn row_5_a_right_click_on_a_rectangle_opens_the_editor_on_that_role() {
    for role in [Role::Foreground, Role::Background] {
        let mut world = World::new();
        world.session.bar_right_click(role);
        assert!(world.session.editor_open, "{role:?}");
        assert_eq!(world.session.editing, role);
    }
}

#[test]
fn row_5_p_is_blocked_by_a_modal_a_dialog_and_a_focused_text_field_and_not_by_a_gesture_or_a_removal()
 {
    for (locks, blocked) in [
        (
            Locks {
                modal: true,
                ..Locks::default()
            },
            true,
        ),
        (
            Locks {
                dialog: true,
                ..Locks::default()
            },
            true,
        ),
        (
            Locks {
                text_focus: true,
                ..Locks::default()
            },
            true,
        ),
        (
            Locks {
                gesture: true,
                ..Locks::default()
            },
            false,
        ),
        (
            Locks {
                removal: true,
                ..Locks::default()
            },
            false,
        ),
    ] {
        let mut world = World::new();
        let result = world.dispatches(AppAction::ToggleColorEditor, Origin::Key, locks);
        assert_eq!(matches!(result, Dispatched::Blocked), blocked, "{locks:?}");
        assert_eq!(world.session.editor_open, !blocked, "{locks:?}");
    }
    // The menu is not stopped by a focused text field (only the key is).
    let mut world = World::new();
    let locks = Locks {
        text_focus: true,
        ..Locks::default()
    };
    world.dispatches(AppAction::ToggleColorEditor, Origin::Menu, locks);
    assert!(world.session.editor_open);
}

// ---------------------------------------------------------------------------------------
// Rows 6 to 9: the four ways of closing
// ---------------------------------------------------------------------------------------

#[test]
fn row_6_escape_closes_the_editor_and_the_role_goes_back_to_the_foreground() {
    let mut world = World::new();
    world.session.bar_right_click(Role::Background);
    // The panel asks to close (`PanelOutput::close_editor`) and the app calls this.
    world.session.close_editor();
    assert!(!world.session.editor_open);
    assert_eq!(world.session.editing, Role::Foreground);
}

#[test]
fn row_7_p_closes_an_open_editor_and_the_role_goes_back_to_the_foreground() {
    let mut world = World::new();
    world.session.bar_right_click(Role::Background);
    world.press_p();
    assert!(!world.session.editor_open);
    assert_eq!(world.session.editing, Role::Foreground);
}

#[test]
fn row_8_a_second_right_click_on_the_edited_rectangle_closes_and_on_the_other_switches() {
    let mut world = World::new();
    world.session.bar_right_click(Role::Background);
    // The other rectangle: the editor stays open and edits that role.
    world.session.bar_right_click(Role::Foreground);
    assert!(world.session.editor_open);
    assert_eq!(world.session.editing, Role::Foreground);
    world.session.bar_right_click(Role::Background);
    assert!(world.session.editor_open);
    assert_eq!(world.session.editing, Role::Background);
    // The same one: closes.
    world.session.bar_right_click(Role::Background);
    assert!(!world.session.editor_open);
    assert_eq!(world.session.editing, Role::Foreground);
}

#[test]
fn row_9_changing_document_closes_the_editor_and_converting_only_when_nothing_is_left_to_edit() {
    // A new document (what `open` and `Nueva imagen…` end up doing).
    let mut world = World::new();
    world.session.bar_right_click(Role::Background);
    world.session.replace_document(
        Document::new(indexed_image()),
        ColorChoice::Entry(0),
        None,
        None,
    );
    assert!(!world.session.editor_open);
    assert_eq!(world.session.editing, Role::Foreground);

    // Converting keeps the editor open when there is a target to edit...
    let mut world = World::new();
    world.session.open_editor(Role::Foreground);
    world.session.sync_editor();
    assert!(world.session.editor_open, "there is something to edit");
    // ...and closes it when there is not (the chosen entry no longer exists).
    world.session.choice = ColorChoice::Entry(200);
    world.session.sync_editor();
    assert!(!world.session.editor_open);
    assert_eq!(world.session.editing, Role::Foreground);

    // Saving does not close it.
    let mut world = World::new();
    world.session.open_editor(Role::Background);
    world.session.sync_editor();
    assert!(world.session.editor_open);
    assert_eq!(world.session.editing, Role::Background);
}

#[test]
fn row_10_choosing_a_tool_a_swatch_or_a_color_key_does_not_close_the_editor() {
    let mut world = World::new();
    world.session.bar_right_click(Role::Background);
    for action in [
        AppAction::SelectTool(Tool::Line),
        AppAction::SelectTool(Tool::Dropper),
        AppAction::SelectTool(Tool::Pencil),
        AppAction::ColorNext,
        AppAction::ColorPrev,
        AppAction::BackgroundNext,
        AppAction::BackgroundPrev,
        AppAction::SelectColor(3),
    ] {
        for origin in [Origin::Key, Origin::Button] {
            world.dispatches(action, origin, Locks::default());
            assert!(world.session.editor_open, "{action:?} {origin:?}");
            assert_eq!(world.session.editing, Role::Background, "{action:?}");
        }
    }
    // Choosing a swatch (what a click or right click in the grid does) and undo neither.
    world.session.choice = ColorChoice::Entry(1);
    world.session.background = ColorChoice::Entry(3);
    world.dispatches(AppAction::Undo, Origin::Key, Locks::default());
    assert!(world.session.editor_open);
}

// ---------------------------------------------------------------------------------------
// Row 14: the status line
// ---------------------------------------------------------------------------------------

#[test]
fn row_14_the_status_line_priority_is_notice_then_panel_hint_then_the_usual_line() {
    let usual = "lápiz · color 3".to_string();
    // Nothing from the panel: the usual line.
    assert_eq!(
        status_for(usual.clone(), false, None),
        (usual.clone(), false)
    );
    // A hint from the panel replaces the usual line (not a notice).
    assert_eq!(
        status_for(usual.clone(), false, Some("índice 3 #ff0000")),
        ("índice 3 #ff0000".to_string(), false)
    );
    // A notice or an error always wins.
    assert_eq!(
        status_for("no hay memoria".to_string(), true, Some("índice 3 #ff0000")),
        ("no hay memoria".to_string(), true)
    );
}

#[test]
fn row_14_the_text_of_a_pending_removal_fits_the_status_bar_at_the_minimum_window() {
    // The longest texts: three-digit indices and the two warnings, joined as the status line does.
    let joined = |lines: [String; 2]| format!("{} · {}", lines[0], lines[1]);
    let mut texts = Vec::new();
    for index in [0u8, 9, 10, 99, 100, 255] {
        for into in [None, Some(0u8), Some(99), Some(255)] {
            for warning in [
                None,
                Some(RemovalWarning::PixelsOpaque),
                Some(RemovalWarning::DestinationTransparent),
            ] {
                texts.push(joined(removal_prompt(index, into, warning)));
            }
        }
    }
    for ppp in [1.0f32, 1.25, 1.5, 2.0, 3.0] {
        let columns = bar_columns(min_window_width_pt(ppp), ppp);
        for text in &texts {
            assert!(
                text.chars().count() <= columns,
                "ppp {ppp}: {text:?} has {} characters and the bar {columns}",
                text.chars().count()
            );
        }
    }
    // The warning prefix is part of the text.
    assert!(
        joined(removal_prompt(5, None, Some(RemovalWarning::PixelsOpaque))).contains(NOTICE_PREFIX)
    );
}

// ---------------------------------------------------------------------------------------
// Property 2: opening and closing do not touch the document or the choice
// ---------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug)]
enum Step {
    P,
    RightClick(Role),
    Close,
    Tool(Tool),
    ColorNext,
    ColorPrev,
    BackgroundNext,
    BackgroundPrev,
    NewDocument,
}

fn step_strategy() -> impl Strategy<Value = Step> {
    prop_oneof![
        Just(Step::P),
        Just(Step::RightClick(Role::Foreground)),
        Just(Step::RightClick(Role::Background)),
        Just(Step::Close),
        Just(Step::Tool(Tool::Pencil)),
        Just(Step::Tool(Tool::Line)),
        Just(Step::Tool(Tool::Fill)),
        Just(Step::Tool(Tool::Dropper)),
        Just(Step::Tool(Tool::Eraser)),
        Just(Step::ColorNext),
        Just(Step::ColorPrev),
        Just(Step::BackgroundNext),
        Just(Step::BackgroundPrev),
        Just(Step::NewDocument),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn property_2_the_editor_follows_the_four_ways_of_closing_and_nothing_else_changes(
        steps in proptest::collection::vec(step_strategy(), 0..60),
    ) {
        let mut world = World::new();
        // Model: `(open, role)`; no editor at all for everything that is not one of the four.
        let (mut open, mut role) = (false, Role::Foreground);
        for step in steps {
            let state_before = world.session.document.state_id();
            let (undo_before, redo_before) = (
                world.session.document.can_undo(),
                world.session.document.can_redo(),
            );
            let replaced = matches!(step, Step::NewDocument);
            match step {
                Step::P => {
                    world.press_p();
                    if open {
                        (open, role) = (false, Role::Foreground);
                    } else {
                        open = true;
                    }
                }
                Step::RightClick(clicked) => {
                    world.session.bar_right_click(clicked);
                    if open && role == clicked {
                        (open, role) = (false, Role::Foreground);
                    } else {
                        (open, role) = (true, clicked);
                    }
                }
                Step::Close => {
                    world.session.close_editor();
                    (open, role) = (false, Role::Foreground);
                }
                Step::Tool(tool) => {
                    world.dispatches(AppAction::SelectTool(tool), Origin::Button, Locks::default());
                }
                Step::ColorNext => {
                    world.dispatches(AppAction::ColorNext, Origin::Key, Locks::default());
                }
                Step::ColorPrev => {
                    world.dispatches(AppAction::ColorPrev, Origin::Key, Locks::default());
                }
                Step::BackgroundNext => {
                    world.dispatches(AppAction::BackgroundNext, Origin::Key, Locks::default());
                }
                Step::BackgroundPrev => {
                    world.dispatches(AppAction::BackgroundPrev, Origin::Key, Locks::default());
                }
                Step::NewDocument => {
                    world.session.replace_document(
                        Document::new(indexed_image()),
                        ColorChoice::Entry(0),
                        None,
                        None,
                    );
                    (open, role) = (false, Role::Foreground);
                }
            }
            world.session.sync_editor();
            prop_assert_eq!(world.session.editor_open, open, "{:?}", step);
            prop_assert_eq!(world.session.editing, role, "{:?}", step);
            if !open {
                prop_assert_eq!(world.session.editing, Role::Foreground);
            }
            // Both colors are always valid.
            prop_assert!(world.session.pixel_of(Role::Foreground).is_some());
            prop_assert!(world.session.pixel_of(Role::Background).is_some());
            // None of these changes the document.
            if !replaced {
                prop_assert_eq!(world.session.document.state_id(), state_before);
                prop_assert_eq!(world.session.document.can_undo(), undo_before);
                prop_assert_eq!(world.session.document.can_redo(), redo_before);
            }
        }
    }
}
