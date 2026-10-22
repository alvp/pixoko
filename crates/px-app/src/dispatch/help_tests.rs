//! Tests de `dispatch` con las acciones de ayuda (specs/interfaz-ayuda.md, unidad I.5, filas 3 y 4,
//! la parte de `dispatch`): `ShowHelp(target_2)` y `ShowShortcuts` dan `Dispatched::Help` y
//! `Dispatched::Shortcuts`, con los bloqueos de una acción que abre un diálogo, sin tocar el
//! documento. En ROJO hasta la lógica: las acciones y las variantes no existen todavía.

use px_core::{Document, Image, Mode};
use px_ui::actions::{AppAction, Locks, Origin};
use px_ui::help::HelpTarget;
use px_ui::tool::{Tool, ToolState};

use super::{Dispatched, Targets, dispatch};
use crate::session::Session;

const SOURCES: [Origin; 3] = [Origin::Menu, Origin::Button, Origin::Key];

fn locks_2(bits: u8) -> Locks {
    Locks {
        gesture: bits & 1 != 0,
        stroke: bits & 2 != 0,
        dialog: bits & 4 != 0,
        text_focus: bits & 8 != 0,
        removal: bits & 16 != 0,
        modal: bits & 32 != 0,
    }
}

fn targets_2() -> Vec<HelpTarget> {
    vec![
        HelpTarget::Active,
        HelpTarget::Tool(Tool::Fill),
        HelpTarget::ZoomIn,
        HelpTarget::ZoomActualSize,
    ]
}

#[test]
fn row_4_show_help_and_show_shortcuts_dispatch_or_are_blocked_without_touching_the_document() {
    for bits in 0..64u8 {
        let locks = locks_2(bits);
        for source_2 in SOURCES {
            let mut session = Session::sample(Document::new(Image::new(4, 4, Mode::Rgba).unwrap()));
            let mut tools = ToolState::new();
            let state_2 = session.document.state_id();
            let locked_2 =
                locks.modal || locks.dialog || locks.gesture || locks.text_focus || locks.removal;
            let mut all: Vec<(AppAction, Dispatched)> = targets_2()
                .into_iter()
                .map(|d| (AppAction::ShowHelp(d), Dispatched::Help(d)))
                .collect();
            all.push((AppAction::ShowShortcuts, Dispatched::Shortcuts));
            for (action, expected_2) in all {
                let result = dispatch(
                    action,
                    source_2,
                    &mut Targets {
                        session: &mut session,
                        tools: &mut tools,
                        locks,
                    },
                );
                if locked_2 {
                    assert_eq!(
                        result,
                        Dispatched::Blocked,
                        "{action:?} {locks:?} {source_2:?}"
                    );
                } else {
                    assert_eq!(result, expected_2, "{action:?} {locks:?} {source_2:?}");
                }
                assert_eq!(session.document.state_id(), state_2);
                assert_eq!(tools.tool(), Tool::Pencil);
            }
        }
    }
}
