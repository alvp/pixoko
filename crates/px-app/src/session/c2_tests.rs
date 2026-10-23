//! Unit C2 (specs/ui-single-column.md), `px-app` part: row 7 (the right click on a tool opens and
//! closes the window of options: the glue between the toolbox and `OptionsWindow`), row 10 (the
//! chain of `Esc`: one link per press, the window of options before the editor) and property 4
//! (opening, closing and resizing do not touch the document). No window: a `Session`, `dispatch`
//! and the pure functions that `main` uses, as `c1_tests.rs` does.

use super::*;

use proptest::prelude::*;
use px_core::{Image, Palette, Rgba};
use px_ui::actions::{AppAction, Locks, Origin};
use px_ui::color::ColorChoice;
use px_ui::options::OptionsWindow;
use px_ui::tool::{Tool, ToolState};
use px_ui::toolbar::ToolboxOutput;

use crate::dispatch::{Dispatched, Targets, dispatch};
use crate::{EscapeStep, apply_toolbox_toggle, escape_step};

fn indexed_image() -> Image {
    let colors: Vec<Rgba> = (0..6)
        .map(|i| Rgba::opaque((i * 30) as u8, 255 - (i * 9) as u8, (i * 7) as u8))
        .collect();
    Image::new_indexed(8, 4, Palette::from_colors(colors).expect("palette")).expect("image")
}

const TOOLS: [Tool; 5] = [
    Tool::Pencil,
    Tool::Line,
    Tool::Fill,
    Tool::Dropper,
    Tool::Eraser,
];

struct World {
    session: Session,
    tools: ToolState,
    window: OptionsWindow,
}

impl World {
    fn new() -> World {
        let mut session = Session::sample(Document::new(indexed_image()));
        session.choice = ColorChoice::Entry(2);
        session.background = ColorChoice::Entry(4);
        World {
            session,
            tools: ToolState::new(),
            window: OptionsWindow::default(),
        }
    }

    fn dispatches(&mut self, action: AppAction, origin: Origin) -> Dispatched {
        dispatch(
            action,
            origin,
            &mut Targets {
                session: &mut self.session,
                tools: &mut self.tools,
                locks: Locks::default(),
            },
        )
    }

    /// A right click on `tool`: what `main` does with the output of the toolbox.
    fn right_click(&mut self, tool: Tool) {
        let active_before = self.tools.tool();
        self.dispatches(AppAction::SelectTool(tool), Origin::Button);
        let output = ToolboxOutput {
            action: Some(AppAction::SelectTool(tool)),
            toggle_options: true,
        };
        apply_toolbox_toggle(&mut self.window, &output, active_before);
    }

    /// A press of `Esc` with `free` (the palette leaves it free): the one link it closes.
    fn escape(&mut self, free: bool) -> EscapeStep {
        let step = escape_step(free, self.window.open, self.session.editor_open);
        match step {
            EscapeStep::CloseOptions => self.window.close(),
            EscapeStep::CloseEditor => self.session.close_editor(),
            EscapeStep::Nothing => {}
        }
        step
    }
}

// ---------------------------------------------------------------------------------------
// Row 7: the right click
// ---------------------------------------------------------------------------------------

#[test]
fn row_7_the_right_click_chooses_the_tool_and_opens_then_closes_the_window_on_the_same_tool() {
    let mut world = World::new();
    for tool in TOOLS {
        world.right_click(tool);
        assert_eq!(world.tools.tool(), tool);
        assert!(world.window.open, "abre con {tool:?}");
        world.right_click(tool);
        assert_eq!(world.tools.tool(), tool);
        assert!(
            !world.window.open,
            "el segundo clic derecho en {tool:?} cierra"
        );
    }
    // Open on a tool, another tool by button, by key or by right click: it stays open and follows.
    world.right_click(Tool::Line);
    assert!(world.window.open);
    world.dispatches(AppAction::SelectTool(Tool::Fill), Origin::Button);
    assert!(world.window.open);
    world.dispatches(AppAction::SelectTool(Tool::Dropper), Origin::Key);
    assert!(world.window.open);
    world.right_click(Tool::Pencil);
    assert!(
        world.window.open,
        "otra herramienta con la ventana abierta no la cierra"
    );
    // The position is shared: choosing tools never moves it.
    let before = world.window.pos_u;
    for tool in TOOLS {
        world.right_click(tool);
    }
    assert_eq!(world.window.pos_u, before);
}

#[test]
fn row_7_what_is_not_an_allowed_right_click_on_a_tool_does_not_touch_the_window() {
    let mut window = OptionsWindow::default();
    // A zoom button, a left click and a blocked tool give no toggle.
    for output in [
        ToolboxOutput {
            action: Some(AppAction::ZoomOut),
            toggle_options: false,
        },
        ToolboxOutput {
            action: Some(AppAction::SelectTool(Tool::Line)),
            toggle_options: false,
        },
        ToolboxOutput {
            action: None,
            toggle_options: false,
        },
    ] {
        apply_toolbox_toggle(&mut window, &output, Tool::Pencil);
        assert!(!window.open, "{output:?}");
    }
}

// ---------------------------------------------------------------------------------------
// Row 10: the chain of Esc
// ---------------------------------------------------------------------------------------

#[test]
fn row_10_each_escape_closes_one_link_the_window_of_options_before_the_editor() {
    let mut world = World::new();
    world.right_click(Tool::Line);
    world.dispatches(AppAction::ToggleColorEditor, Origin::Key);
    assert!(world.window.open && world.session.editor_open);
    // Not free (a text field, a stroke, a slider, a swatch drag or a pending removal claim it):
    // nothing closes.
    assert_eq!(world.escape(false), EscapeStep::Nothing);
    assert!(world.window.open && world.session.editor_open);
    // Free: the window first, then the editor, then nothing is left for `Esc` (the canvas's).
    assert_eq!(world.escape(true), EscapeStep::CloseOptions);
    assert!(!world.window.open && world.session.editor_open);
    assert_eq!(world.escape(true), EscapeStep::CloseEditor);
    assert!(!world.window.open && !world.session.editor_open);
    assert_eq!(world.escape(true), EscapeStep::Nothing);
    // Only the window open: it closes with the first `Esc`.
    world.right_click(Tool::Fill);
    assert_eq!(world.escape(true), EscapeStep::CloseOptions);
    // The layers panel is not part of this chain: `Esc` leaves it alone (the state is another).
    assert_eq!(world.escape(true), EscapeStep::Nothing);
}

// ---------------------------------------------------------------------------------------
// Property 4: opening, closing and resizing do not touch the document
// ---------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug)]
enum Step {
    RightClick(usize),
    KeyTool(usize),
    CloseWindow,
    Escape(bool),
    P,
    Undo,
    Redo,
    ClearPage,
    NewDocument,
}

fn step_strategy() -> impl Strategy<Value = Step> {
    prop_oneof![
        3 => (0usize..5).prop_map(Step::RightClick),
        2 => (0usize..5).prop_map(Step::KeyTool),
        1 => Just(Step::CloseWindow),
        2 => any::<bool>().prop_map(Step::Escape),
        2 => Just(Step::P),
        2 => Just(Step::Undo),
        1 => Just(Step::Redo),
        2 => Just(Step::ClearPage),
        1 => Just(Step::NewDocument),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn property_4_the_window_and_the_column_do_not_change_what_the_document_does(
        steps in proptest::collection::vec(step_strategy(), 0..60),
    ) {
        // The world with the window and a model without it that gets only the real commands.
        let mut world = World::new();
        let mut model = World::new();
        for step in steps {
            match step {
                Step::RightClick(t) => {
                    world.right_click(TOOLS[t]);
                    model.dispatches(AppAction::SelectTool(TOOLS[t]), Origin::Button);
                }
                Step::KeyTool(t) => {
                    world.dispatches(AppAction::SelectTool(TOOLS[t]), Origin::Key);
                    model.dispatches(AppAction::SelectTool(TOOLS[t]), Origin::Key);
                }
                Step::CloseWindow => world.window.close(),
                Step::Escape(free) => {
                    let was_open = world.window.open;
                    let editor_before = world.session.editor_open;
                    let step = world.escape(free);
                    // One link per press, in order, and only when the palette leaves it free.
                    match (free, was_open, editor_before) {
                        (false, ..) | (true, false, false) => prop_assert_eq!(step, EscapeStep::Nothing),
                        (true, true, _) => prop_assert_eq!(step, EscapeStep::CloseOptions),
                        (true, false, true) => {
                            prop_assert_eq!(step, EscapeStep::CloseEditor);
                            model.session.close_editor();
                        }
                    }
                }
                Step::P => {
                    world.dispatches(AppAction::ToggleColorEditor, Origin::Key);
                    model.dispatches(AppAction::ToggleColorEditor, Origin::Key);
                }
                Step::Undo => {
                    world.dispatches(AppAction::Undo, Origin::Key);
                    model.dispatches(AppAction::Undo, Origin::Key);
                }
                Step::Redo => {
                    world.dispatches(AppAction::Redo, Origin::Key);
                    model.dispatches(AppAction::Redo, Origin::Key);
                }
                Step::ClearPage => {
                    world.dispatches(AppAction::ClearPage, Origin::Key);
                    model.dispatches(AppAction::ClearPage, Origin::Key);
                }
                Step::NewDocument => {
                    for w in [&mut world, &mut model] {
                        w.session.replace_document(
                            Document::new(indexed_image()),
                            ColorChoice::Entry(0),
                            None,
                            None,
                        );
                    }
                }
            }
            world.session.sync_editor();
            model.session.sync_editor();
            prop_assert_eq!(world.session.document.state_id(), model.session.document.state_id(), "{:?}", step);
            prop_assert_eq!(world.session.document.can_undo(), model.session.document.can_undo());
            prop_assert_eq!(world.session.document.can_redo(), model.session.document.can_redo());
            prop_assert_eq!(world.session.editor_open, model.session.editor_open);
            // Both colors are valid and the same as in the model.
            prop_assert!(world.session.pixel_of(px_ui::color::Role::Foreground).is_some());
            prop_assert!(world.session.pixel_of(px_ui::color::Role::Background).is_some());
            prop_assert_eq!(world.session.choice, model.session.choice);
            prop_assert_eq!(world.session.background, model.session.background);
            prop_assert_eq!(world.tools.tool(), model.tools.tool());
        }
    }
}
