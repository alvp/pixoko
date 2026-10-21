//! Tests de `dispatch` con las acciones de ayuda (specs/interfaz-ayuda.md, unidad I.5, filas 3 y 4,
//! la parte de `dispatch`): `ShowHelp(destino)` y `ShowShortcuts` dan `Dispatched::Help` y
//! `Dispatched::Shortcuts`, con los bloqueos de una acción que abre un diálogo, sin tocar el
//! documento. En ROJO hasta la lógica: las acciones y las variantes no existen todavía.

use px_core::{Document, Image, Mode};
use px_ui::actions::{AppAction, Locks, Origin};
use px_ui::help::HelpTarget;
use px_ui::tool::{Tool, ToolState};

use super::{Dispatched, Targets, dispatch};
use crate::session::Session;

const ORIGENES: [Origin; 3] = [Origin::Menu, Origin::Button, Origin::Key];

fn bloqueos(bits: u8) -> Locks {
    Locks {
        gesture: bits & 1 != 0,
        stroke: bits & 2 != 0,
        dialog: bits & 4 != 0,
        text_focus: bits & 8 != 0,
        removal: bits & 16 != 0,
        modal: bits & 32 != 0,
    }
}

fn destinos() -> Vec<HelpTarget> {
    vec![
        HelpTarget::Active,
        HelpTarget::Tool(Tool::Fill),
        HelpTarget::ZoomIn,
        HelpTarget::ZoomActual,
    ]
}

#[test]
fn fila_4_show_help_y_show_shortcuts_se_despachan_o_se_bloquean_sin_tocar_el_documento() {
    for bits in 0..64u8 {
        let locks = bloqueos(bits);
        for origen in ORIGENES {
            let mut session = Session::sample(Document::new(Image::new(4, 4, Mode::Rgba).unwrap()));
            let mut tools = ToolState::new();
            let estado = session.document.state_id();
            let bloqueada = locks.modal || locks.dialog || locks.gesture || locks.text_focus || locks.removal;
            let mut todas: Vec<(AppAction, Dispatched)> = destinos()
                .into_iter()
                .map(|d| (AppAction::ShowHelp(d), Dispatched::Help(d)))
                .collect();
            todas.push((AppAction::ShowShortcuts, Dispatched::Shortcuts));
            for (action, esperado) in todas {
                let resultado = dispatch(
                    action,
                    origen,
                    &mut Targets {
                        session: &mut session,
                        tools: &mut tools,
                        locks,
                    },
                );
                if bloqueada {
                    assert_eq!(resultado, Dispatched::Blocked, "{action:?} {locks:?} {origen:?}");
                } else {
                    assert_eq!(resultado, esperado, "{action:?} {locks:?} {origen:?}");
                }
                assert_eq!(session.document.state_id(), estado);
                assert_eq!(tools.tool(), Tool::Pencil);
            }
        }
    }
}
