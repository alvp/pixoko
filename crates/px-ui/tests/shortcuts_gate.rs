//! Tests de `shortcuts::gate_keys` (specs/hito-2-editar-color.md, unidad 11c, decisión 11; filas
//! 16 y 17 en su parte pura).
//!
//! Con un campo de texto enfocado (`wants_keyboard`), `gate_keys` no deja pasar ningún evento a
//! los atajos: teclear `ff00ab` no cambia de herramienta (`f`), de color (`0` a `9`) ni de vista
//! (`Home`, `+`, `-`), y `Ctrl+Z`, `Ctrl+Y` y `Ctrl+S` no tocan el documento ni el fichero. Sin
//! foco de texto devuelve todos los eventos, en el mismo orden, y `action_for_key` y
//! `tool_action_for_key` siguen viéndolos.

use egui::{Event, Key, Modifiers, PointerButton, pos2};
use px_ui::shortcuts::{Action, FileAction, action_for_key, file_action_for_key, gate_keys};
use px_ui::tool::{Tool, ToolAction, tool_action_for_key};

fn key_2(key: Key, modifiers: Modifiers) -> Event {
    Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers,
    }
}

fn text_2(text_2: &str) -> Event {
    Event::Text(text_2.to_string())
}

/// Las teclas de la fila 16: `f`, `f`, `0`, `0`, `a`, `b`, y `Home`, `+`, `-`.
fn keys_of_row_16() -> Vec<Event> {
    [
        Key::F,
        Key::F,
        Key::Num0,
        Key::Num0,
        Key::A,
        Key::B,
        Key::Home,
        Key::Plus,
        Key::Minus,
    ]
    .into_iter()
    .map(|key| key_2(key, Modifiers::NONE))
    .collect()
}

/// Los textos que llegan con esas teclas.
fn texts_of_row_16() -> Vec<Event> {
    ["f", "f", "0", "0", "a", "b"]
        .into_iter()
        .map(text_2)
        .collect()
}

/// Teclas con modificador de la fila 17.
fn shortcuts_of_row_17() -> Vec<Event> {
    vec![
        key_2(Key::Z, Modifiers::COMMAND),
        key_2(Key::Y, Modifiers::COMMAND),
        key_2(Key::S, Modifiers::COMMAND),
    ]
}

/// Todo lo de las filas 16 y 17, mezclado como lo entregaría egui: texto y tecla por cada letra.
fn frame_complete() -> Vec<Event> {
    let mut events_2 = Vec::new();
    for (key, letter) in [
        (Key::F, "f"),
        (Key::F, "f"),
        (Key::Num0, "0"),
        (Key::Num0, "0"),
        (Key::A, "a"),
        (Key::B, "b"),
    ] {
        events_2.push(key_2(key, Modifiers::NONE));
        events_2.push(text_2(letter));
    }
    events_2.extend([
        key_2(Key::Home, Modifiers::NONE),
        key_2(Key::Plus, Modifiers::NONE),
        key_2(Key::Minus, Modifiers::NONE),
    ]);
    events_2.extend(shortcuts_of_row_17());
    events_2
}

// Fila 16: con el campo enfocado.

#[test]
fn row_16_with_text_focus_no_key_passes() {
    assert!(gate_keys(&keys_of_row_16(), true).is_empty());
}

#[test]
fn row_16_with_text_focus_the_text_events_do_not_pass() {
    assert!(gate_keys(&texts_of_row_16(), true).is_empty());
}

#[test]
fn row_16_each_event_separately_stays_outside() {
    for event in frame_complete() {
        assert!(
            gate_keys(std::slice::from_ref(&event), true).is_empty(),
            "{event:?}"
        );
    }
}

#[test]
fn row_16_the_whole_frame_with_focus_gives_empty() {
    assert!(gate_keys(&frame_complete(), true).is_empty());
}

#[test]
fn row_16_neither_tool_action_for_key_nor_action_for_key_see_anything_with_focus() {
    let visible_2 = gate_keys(&frame_complete(), true);
    let mut actions_of_tool = 0;
    let mut actions = 0;
    for event in &visible_2 {
        if let Event::Key { key, modifiers, .. } = event {
            actions_of_tool += usize::from(tool_action_for_key(*key, *modifiers).is_some());
            actions += usize::from(action_for_key(*key, *modifiers).is_some());
        }
    }
    assert_eq!((actions_of_tool, actions), (0, 0));
}

#[test]
fn row_16_without_the_filter_the_f_would_choose_fill() {
    // Lo que evita el filtro: la `f` sola es una acción de herramienta.
    assert_eq!(
        tool_action_for_key(Key::F, Modifiers::NONE),
        Some(ToolAction::Select(Tool::Fill))
    );
}

// Fila 17: Ctrl+Z, Ctrl+Y, Ctrl+S.

#[test]
fn row_17_with_text_focus_the_shortcuts_with_control_stay_outside() {
    assert!(gate_keys(&shortcuts_of_row_17(), true).is_empty());
    for event in shortcuts_of_row_17() {
        let visible_2 = gate_keys(std::slice::from_ref(&event), true);
        assert!(visible_2.is_empty(), "{event:?}");
    }
}

#[test]
fn row_17_with_text_focus_the_save_shortcut_does_not_arrive() {
    let visible_2 = gate_keys(&shortcuts_of_row_17(), true);
    let save = visible_2.iter().any(|event| match event {
        Event::Key { key, modifiers, .. } => file_action_for_key(*key, *modifiers).is_some(),
        _ => false,
    });
    assert!(!save);
}

#[test]
fn row_17_without_focus_it_returns_all_in_the_same_order() {
    let events_2 = frame_complete();
    assert_eq!(gate_keys(&events_2, false), events_2);
    let keys = keys_of_row_16();
    assert_eq!(gate_keys(&keys, false), keys);
    let texts = texts_of_row_16();
    assert_eq!(gate_keys(&texts, false), texts);
    let shortcuts = shortcuts_of_row_17();
    assert_eq!(gate_keys(&shortcuts, false), shortcuts);
}

#[test]
fn row_17_without_focus_the_shortcuts_are_seen_again() {
    let events_2 = vec![
        key_2(Key::Z, Modifiers::COMMAND),
        key_2(Key::CloseBracket, Modifiers::NONE),
        key_2(Key::Home, Modifiers::NONE),
    ];
    let visible_2 = gate_keys(&events_2, false);
    assert_eq!(visible_2, events_2);
    let actions: Vec<Option<Action>> = visible_2
        .iter()
        .map(|event| match event {
            Event::Key { key, modifiers, .. } => action_for_key(*key, *modifiers),
            _ => None,
        })
        .collect();
    assert_eq!(actions, [Some(Action::Undo), Some(Action::ColorNext), None]);
}

#[test]
fn row_17_without_focus_the_file_and_tool_shortcuts_are_still_seen() {
    let visible_2 = gate_keys(&shortcuts_of_row_17(), false);
    let save: Vec<Option<FileAction>> = visible_2
        .iter()
        .map(|event| match event {
            Event::Key { key, modifiers, .. } => file_action_for_key(*key, *modifiers),
            _ => None,
        })
        .collect();
    assert_eq!(save, [None, None, Some(FileAction::Save)]);

    let visible_2 = gate_keys(&keys_of_row_16(), false);
    let tool_2: Vec<Option<ToolAction>> = visible_2
        .iter()
        .map(|event| match event {
            Event::Key { key, modifiers, .. } => tool_action_for_key(*key, *modifiers),
            _ => None,
        })
        .collect();
    assert_eq!(tool_2[0], Some(ToolAction::Select(Tool::Fill)));
    assert_eq!(tool_2[1], Some(ToolAction::Select(Tool::Fill)));
    assert!(tool_2[2..].iter().all(Option::is_none));
}

// Casos generales.

#[test]
fn without_events_it_gives_empty_with_and_without_focus() {
    assert!(gate_keys(&[], true).is_empty());
    assert!(gate_keys(&[], false).is_empty());
}

#[test]
fn without_focus_the_non_keyboard_events_also_pass() {
    let events_2 = vec![
        Event::PointerMoved(pos2(1.0, 2.0)),
        Event::PointerButton {
            pos: pos2(1.0, 2.0),
            button: PointerButton::Primary,
            pressed: true,
            modifiers: Modifiers::NONE,
        },
        text_2("x"),
        key_2(Key::Escape, Modifiers::NONE),
    ];
    assert_eq!(gate_keys(&events_2, false), events_2);
}

#[test]
fn with_focus_neither_released_nor_repeated_keys_pass() {
    let released = Event::Key {
        key: Key::Z,
        physical_key: Some(Key::Z),
        pressed: false,
        repeat: false,
        modifiers: Modifiers::COMMAND,
    };
    let repeated = Event::Key {
        key: Key::Num3,
        physical_key: Some(Key::Num3),
        pressed: true,
        repeat: true,
        modifiers: Modifiers::NONE,
    };
    assert!(gate_keys(&[released, repeated], true).is_empty());
}

#[test]
fn the_filter_does_not_modify_the_input() {
    let events_2 = frame_complete();
    let copy = events_2.clone();
    let _ = gate_keys(&events_2, true);
    let _ = gate_keys(&events_2, false);
    assert_eq!(events_2, copy);
}
