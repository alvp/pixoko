//! Tests de la captura de teclas y de `F1` (specs/interfaz-ayuda.md, unidad I.5, decisión 6).
//! En ROJO hasta la lógica: `Capture`, `captures`, `help_pressed` y `chord_of` no existen todavía.
//!
//! - Fila 3 (parte de eventos): `help_pressed`.
//! - Fila 6: captura con las secuencias de I.1 (español, inglés, alemán, macOS, Dvorak, ruso,
//!   Bloq Mayús, AZERTY), teclas muertas, IME, `Escape`, `F1`, reservadas, repeticiones.
//! - Propiedad 3: la captura es total y lo que da es asignable.

use egui::{Event, ImeEvent, Key, Modifiers};
use proptest::prelude::*;
use px_ui::actions::AppAction;
use px_ui::actions::Platform;
use px_ui::key_events::{Capture, captures, chord_of, help_pressed};
use px_ui::keymap::{Binding, Chord, KeyId, Keymap, Platforms, Press};

fn key_2(key: Key, modifiers: Modifiers) -> Event {
    Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers,
    }
}

fn repeated(key: Key, modifiers: Modifiers) -> Event {
    Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: true,
        modifiers,
    }
}

fn releases(key: Key, modifiers: Modifiers) -> Event {
    Event::Key {
        key,
        physical_key: Some(key),
        pressed: false,
        repeat: false,
        modifiers,
    }
}

fn text_2(t: &str) -> Event {
    Event::Text(t.to_string())
}

const NOTHING: Modifiers = Modifiers::NONE;

fn shift_2() -> Modifiers {
    Modifiers {
        shift: true,
        ..Modifiers::NONE
    }
}

fn alt() -> Modifiers {
    Modifiers {
        alt: true,
        ..Modifiers::NONE
    }
}

fn ctrl() -> Modifiers {
    Modifiers {
        ctrl: true,
        command: true,
        ..Modifiers::NONE
    }
}

/// El resultado de capturar una secuencia: `Ok(chord)` o el motivo del rechazo.
fn capture(events_2: &[Event]) -> Result<Chord, String> {
    match captures(events_2).first() {
        Some(Capture::Press(press)) => chord_of(press).map_err(str::to_string),
        Some(Capture::Cancel) => Err("cancel".into()),
        Some(Capture::Reserved(reason_2)) => Err((*reason_2).to_string()),
        None => Err("nada".into()),
    }
}

fn symbol(c: char) -> Chord {
    Chord {
        key: KeyId::Char(c),
        command: false,
        shift: false,
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 3: F1
// ---------------------------------------------------------------------------------------------

#[test]
fn row_3_only_f1_without_modifiers_or_repeat_opens_the_help() {
    assert!(help_pressed(&[key_2(Key::F1, NOTHING)]));
    assert!(help_pressed(&[
        key_2(Key::A, NOTHING),
        key_2(Key::F1, NOTHING)
    ]));
    assert!(!help_pressed(&[]));
    assert!(!help_pressed(&[key_2(Key::F1, shift_2())]));
    assert!(!help_pressed(&[key_2(Key::F1, ctrl())]));
    assert!(!help_pressed(&[key_2(Key::F1, alt())]));
    assert!(!help_pressed(&[releases(Key::F1, NOTHING)]));
    assert!(!help_pressed(&[repeated(Key::F1, NOTHING)]));
    assert!(!help_pressed(&[
        key_2(Key::F2, NOTHING),
        key_2(Key::A, NOTHING)
    ]));
    assert!(!help_pressed(&[text_2("F1")]));
}

// ---------------------------------------------------------------------------------------------
// Fila 6: captura
// ---------------------------------------------------------------------------------------------

#[test]
fn row_6_the_layouts_give_the_character_the_key_produces() {
    // Español: tecla ISO `<`, `Mayús`+ISO `>`; AltGr+`[` (con alt).
    assert_eq!(
        capture(&[key_2(Key::IntlBackslash, NOTHING), text_2("<")]),
        Ok(symbol('<'))
    );
    assert_eq!(
        capture(&[key_2(Key::IntlBackslash, shift_2()), text_2(">")]),
        Ok(symbol('>')),
        "Mayús no cuenta en un símbolo"
    );
    assert_eq!(
        capture(&[key_2(Key::OpenBracket, alt()), text_2("[")]),
        Ok(symbol('['))
    );
    // Inglés: Mayús+`,` con texto `<`.
    assert_eq!(
        capture(&[key_2(Key::Comma, shift_2()), text_2("<")]),
        Ok(symbol('<'))
    );
    // Alemán: AltGr+8 → `[`.
    assert_eq!(
        capture(&[key_2(Key::Num8, alt()), text_2("[")]),
        Ok(symbol('['))
    );
    // macOS español: `Option`+`[` con alt.
    assert_eq!(
        capture(&[key_2(Key::Num5, alt()), text_2("[")]),
        Ok(symbol('['))
    );
    // Dvorak: la `d` en otra posición física; ruso: `Key::D` con `в` (cae a la tecla).
    assert_eq!(
        capture(&[key_2(Key::E, NOTHING), text_2("d")]),
        Ok(symbol('d'))
    );
    assert_eq!(
        capture(&[key_2(Key::D, NOTHING), text_2("в")]),
        Ok(symbol('d'))
    );
    // Bloq Mayús: `Text("D")` sin `shift` es la `d`.
    assert_eq!(
        capture(&[key_2(Key::D, NOTHING), text_2("D")]),
        Ok(symbol('d'))
    );
    // Con Mayús, una letra lleva `shift`.
    assert_eq!(
        capture(&[key_2(Key::K, shift_2()), text_2("K")]),
        Ok(Chord {
            key: KeyId::Char('k'),
            command: false,
            shift: true
        })
    );
    // Teclas con nombre.
    assert_eq!(
        capture(&[key_2(Key::PageDown, shift_2())]),
        Ok(Chord {
            key: KeyId::PageDown,
            command: false,
            shift: true
        })
    );
    assert_eq!(
        capture(&[key_2(Key::Home, shift_2())]),
        Ok(Chord {
            key: KeyId::Home,
            command: false,
            shift: false
        }),
        "Mayús no cuenta en Inicio"
    );
    // Una cifra.
    assert_eq!(
        capture(&[key_2(Key::Num3, NOTHING), text_2("3")]),
        Ok(symbol('3'))
    );
}

#[test]
fn row_6_what_is_not_valid_gives_its_reason_and_none_passes_37_characters() {
    let reasons = [
        // Tecla muerta, vocal compuesta, composición, `Option`+`d` (`∂`), IME, texto suelto.
        capture(&[key_2(Key::OpenBracket, NOTHING)]),
        capture(&[key_2(Key::U, NOTHING), text_2("ú")]),
        capture(&[key_2(Key::D, NOTHING), text_2("´d")]),
        capture(&[key_2(Key::D, alt()), text_2("∂")]),
        // Teclas sin carácter.
        capture(&[key_2(Key::Enter, NOTHING)]),
        capture(&[key_2(Key::Tab, NOTHING)]),
        capture(&[key_2(Key::ArrowLeft, NOTHING)]),
        capture(&[key_2(Key::Backspace, NOTHING)]),
        capture(&[key_2(Key::F5, NOTHING)]),
        // `Ctrl`+letra y `Alt`+letra.
        capture(&[key_2(Key::Z, ctrl())]),
        capture(&[key_2(Key::C, ctrl())]),
        capture(&[key_2(Key::D, alt()), text_2("d")]),
        // Espacio.
        capture(&[key_2(Key::Space, NOTHING), text_2(" ")]),
    ];
    for reason_2 in &reasons {
        let m = reason_2.as_ref().expect_err("no vale");
        assert!(m.chars().count() <= 37, "{m}");
        assert!(!m.is_empty());
    }
    assert_eq!(reasons[4], Err("esa tecla no vale".to_string()));
    assert!(reasons[9].as_ref().unwrap_err().contains("Ctrl"));
    assert!(reasons[11].as_ref().unwrap_err().contains("Alt"));
    assert!(reasons[12].as_ref().unwrap_err().contains("Espacio"));
    // IME y texto suelto no dan captura.
    assert!(captures(&[Event::Ime(ImeEvent::Commit("d".into()))]).is_empty());
    assert!(captures(&[text_2("d")]).is_empty());
    assert!(captures(&[releases(Key::D, NOTHING)]).is_empty());
}

#[test]
fn row_6_escape_f1_repeats_and_the_first_one_of_the_frame() {
    assert_eq!(
        captures(&[key_2(Key::Escape, NOTHING)]),
        vec![Capture::Cancel]
    );
    assert_eq!(
        captures(&[key_2(Key::Escape, shift_2())]),
        vec![Capture::Cancel]
    );
    assert!(captures(&[repeated(Key::Escape, NOTHING)]).is_empty());
    assert_eq!(
        capture(&[key_2(Key::F1, NOTHING)]),
        Err("F1 es de la ayuda".to_string())
    );
    // Las repeticiones se descartan: una tecla mantenida al armar no se captura sola.
    assert!(captures(&[repeated(Key::D, NOTHING), text_2("d")]).is_empty());
    // Vale la primera del fotograma.
    let two = captures(&[
        key_2(Key::X, NOTHING),
        text_2("x"),
        key_2(Key::Y, NOTHING),
        text_2("y"),
    ]);
    assert_eq!(
        chord_of(match &two[0] {
            Capture::Press(p) => p,
            other_2 => panic!("{other_2:?}"),
        }),
        Ok(symbol('x'))
    );
}

#[test]
fn row_6_what_is_captured_works_as_a_row_of_the_table() {
    // Para cada carácter asignable de un teclado cualquiera, una fila con ese `Chord` casa con su
    // propia pulsación.
    for c in (b' '..=b'~').map(char::from) {
        if !c.is_ascii_graphic() {
            continue;
        }
        let mods = if c.is_ascii_uppercase() {
            shift_2()
        } else {
            NOTHING
        };
        let Ok(chord) = capture(&[key_2(Key::A, mods), text_2(&c.to_string())]) else {
            continue;
        };
        let row_2 = Binding {
            key: chord.key,
            command: false,
            shift: chord.shift,
            action: AppAction::ZoomFit,
            active: Platforms::All,
            shown: Platforms::All,
        };
        let table = Keymap::new(vec![row_2]);
        let press = match captures(&[key_2(Key::A, mods), text_2(&c.to_string())])
            .first()
            .unwrap()
        {
            Capture::Press(p) => *p,
            other_2 => panic!("{other_2:?}"),
        };
        assert_eq!(
            table.action_for(&press, Platform::Linux),
            Some(AppAction::ZoomFit),
            "{c:?}"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Propiedad 3
// ---------------------------------------------------------------------------------------------

fn event() -> impl Strategy<Value = Event> {
    let keys = prop_oneof![
        Just(Key::A),
        Just(Key::D),
        Just(Key::K),
        Just(Key::Escape),
        Just(Key::F1),
        Just(Key::F5),
        Just(Key::Space),
        Just(Key::Enter),
        Just(Key::Tab),
        Just(Key::Home),
        Just(Key::PageUp),
        Just(Key::PageDown),
        Just(Key::Num1),
        Just(Key::Comma),
        Just(Key::OpenBracket),
        Just(Key::IntlBackslash),
    ];
    let mods = (any::<bool>(), any::<bool>(), any::<bool>(), any::<bool>()).prop_map(
        |(shift, alt, ctrl, mac)| Modifiers {
            shift,
            alt,
            ctrl,
            command: ctrl,
            mac_cmd: mac,
        },
    );
    prop_oneof![
        (keys, mods, any::<bool>(), any::<bool>()).prop_map(|(key, modifiers, pressed, repeat)| {
            Event::Key {
                key,
                physical_key: Some(key),
                pressed,
                repeat,
                modifiers,
            }
        }),
        "\\PC{0,3}".prop_map(Event::Text),
        "\\PC{0,3}".prop_map(|t| Event::Ime(ImeEvent::Commit(t))),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Propiedad 3: `captures` y `chord_of` no entran en pánico; todo `Chord` que sale no lleva
    /// `command`, no es `Alt`+letra, tiene carácter, no es `Espacio` y una fila con él casa con su
    /// propia `Press`; `Escape` nunca da un `Chord`; las repeticiones y `F1` nunca dan nada.
    #[test]
    fn property_3_the_capture_is_total_and_what_it_gives_is_assignable(
        events_2 in proptest::collection::vec(event(), 0..8),
    ) {
        for capture in captures(&events_2) {
            match capture {
                Capture::Cancel | Capture::Reserved(_) => {}
                Capture::Press(press) => {
                    prop_assert!(!press.repeat);
                    if let Ok(chord) = chord_of(&press) {
                        prop_assert!(!chord.command && !press.command);
                        let KeyId::Char(c) = chord.key else {
                            prop_assert!(!chord.shift || chord.key != KeyId::Home);
                            continue;
                        };
                        prop_assert!(c.is_ascii_graphic() && !c.is_ascii_uppercase(), "{c:?}");
                        if c.is_ascii_alphanumeric() {
                            prop_assert!(!press.alt && !press.other_ctrl, "{press:?}");
                        }
                        prop_assert!(!chord.shift || c.is_ascii_lowercase());
                        let row_2 = Binding {
                            key: chord.key,
                            command: false,
                            shift: chord.shift,
                            action: AppAction::ZoomFit,
                            active: Platforms::All,
                            shown: Platforms::All,
                        };
                        let table = Keymap::new(vec![row_2]);
                        prop_assert_eq!(
                            table.action_for(&press, Platform::Linux),
                            Some(AppAction::ZoomFit)
                        );
                    }
                }
            }
        }
        // `Escape` nunca da un `Chord` y `F1` tampoco.
        for c in captures(&[key_2(Key::Escape, NOTHING), key_2(Key::F1, NOTHING)]) {
            if let Capture::Press(p) = c {
                prop_assert!(chord_of(&p).is_err());
            }
        }
    }
}

#[allow(dead_code)]
fn _uses(_: Press) {}
