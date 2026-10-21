//! Tests de la captura de teclas y de `F1` (specs/interfaz-ayuda.md, unidad I.5, decisión 6).
//! En ROJO hasta la lógica: `Capture`, `captures`, `help_pressed` y `chord_of` no existen todavía.
//!
//! - Fila 3 (parte de eventos): `help_pressed`.
//! - Fila 6: captura con las secuencias de I.1 (español, inglés, alemán, macOS, Dvorak, ruso,
//!   Bloq Mayús, AZERTY), teclas muertas, IME, `Escape`, `F1`, reservadas, repeticiones.
//! - Propiedad 3: la captura es total y lo que da es asignable.

use egui::{Event, ImeEvent, Key, Modifiers};
use proptest::prelude::*;
use px_ui::actions::Platform;
use px_ui::key_events::{Capture, captures, chord_of, help_pressed};
use px_ui::keymap::{Binding, Chord, KeyId, Keymap, Platforms, Press};
use px_ui::actions::AppAction;

fn tecla(key: Key, modifiers: Modifiers) -> Event {
    Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers,
    }
}

fn repetida(key: Key, modifiers: Modifiers) -> Event {
    Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: true,
        modifiers,
    }
}

fn suelta(key: Key, modifiers: Modifiers) -> Event {
    Event::Key {
        key,
        physical_key: Some(key),
        pressed: false,
        repeat: false,
        modifiers,
    }
}

fn texto(t: &str) -> Event {
    Event::Text(t.to_string())
}

const NADA: Modifiers = Modifiers::NONE;

fn mayus() -> Modifiers {
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
fn captura(eventos: &[Event]) -> Result<Chord, String> {
    match captures(eventos).first() {
        Some(Capture::Press(press)) => chord_of(press).map_err(str::to_string),
        Some(Capture::Cancel) => Err("cancel".into()),
        Some(Capture::Reserved(motivo)) => Err((*motivo).to_string()),
        None => Err("nada".into()),
    }
}

fn simbolo(c: char) -> Chord {
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
fn fila_3_solo_f1_sin_modificadores_ni_repeticion_abre_la_ayuda() {
    assert!(help_pressed(&[tecla(Key::F1, NADA)]));
    assert!(help_pressed(&[tecla(Key::A, NADA), tecla(Key::F1, NADA)]));
    assert!(!help_pressed(&[]));
    assert!(!help_pressed(&[tecla(Key::F1, mayus())]));
    assert!(!help_pressed(&[tecla(Key::F1, ctrl())]));
    assert!(!help_pressed(&[tecla(Key::F1, alt())]));
    assert!(!help_pressed(&[suelta(Key::F1, NADA)]));
    assert!(!help_pressed(&[repetida(Key::F1, NADA)]));
    assert!(!help_pressed(&[tecla(Key::F2, NADA), tecla(Key::A, NADA)]));
    assert!(!help_pressed(&[texto("F1")]));
}

// ---------------------------------------------------------------------------------------------
// Fila 6: captura
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_6_las_distribuciones_dan_el_caracter_que_produce_la_tecla() {
    // Español: tecla ISO `<`, `Mayús`+ISO `>`; AltGr+`[` (con alt).
    assert_eq!(
        captura(&[tecla(Key::IntlBackslash, NADA), texto("<")]),
        Ok(simbolo('<'))
    );
    assert_eq!(
        captura(&[tecla(Key::IntlBackslash, mayus()), texto(">")]),
        Ok(simbolo('>')),
        "Mayús no cuenta en un símbolo"
    );
    assert_eq!(
        captura(&[tecla(Key::OpenBracket, alt()), texto("[")]),
        Ok(simbolo('['))
    );
    // Inglés: Mayús+`,` con texto `<`.
    assert_eq!(
        captura(&[tecla(Key::Comma, mayus()), texto("<")]),
        Ok(simbolo('<'))
    );
    // Alemán: AltGr+8 → `[`.
    assert_eq!(
        captura(&[tecla(Key::Num8, alt()), texto("[")]),
        Ok(simbolo('['))
    );
    // macOS español: `Option`+`[` con alt.
    assert_eq!(
        captura(&[tecla(Key::Num5, alt()), texto("[")]),
        Ok(simbolo('['))
    );
    // Dvorak: la `d` en otra posición física; ruso: `Key::D` con `в` (cae a la tecla).
    assert_eq!(captura(&[tecla(Key::E, NADA), texto("d")]), Ok(simbolo('d')));
    assert_eq!(captura(&[tecla(Key::D, NADA), texto("в")]), Ok(simbolo('d')));
    // Bloq Mayús: `Text("D")` sin `shift` es la `d`.
    assert_eq!(captura(&[tecla(Key::D, NADA), texto("D")]), Ok(simbolo('d')));
    // Con Mayús, una letra lleva `shift`.
    assert_eq!(
        captura(&[tecla(Key::K, mayus()), texto("K")]),
        Ok(Chord {
            key: KeyId::Char('k'),
            command: false,
            shift: true
        })
    );
    // Teclas con nombre.
    assert_eq!(
        captura(&[tecla(Key::PageDown, mayus())]),
        Ok(Chord {
            key: KeyId::PageDown,
            command: false,
            shift: true
        })
    );
    assert_eq!(
        captura(&[tecla(Key::Home, mayus())]),
        Ok(Chord {
            key: KeyId::Home,
            command: false,
            shift: false
        }),
        "Mayús no cuenta en Inicio"
    );
    // Una cifra.
    assert_eq!(captura(&[tecla(Key::Num3, NADA), texto("3")]), Ok(simbolo('3')));
}

#[test]
fn fila_6_lo_que_no_vale_da_su_motivo_y_ninguno_pasa_de_37_caracteres() {
    let motivos = [
        // Tecla muerta, vocal compuesta, composición, `Option`+`d` (`∂`), IME, texto suelto.
        captura(&[tecla(Key::OpenBracket, NADA)]),
        captura(&[tecla(Key::U, NADA), texto("ú")]),
        captura(&[tecla(Key::D, NADA), texto("´d")]),
        captura(&[tecla(Key::D, alt()), texto("∂")]),
        // Teclas sin carácter.
        captura(&[tecla(Key::Enter, NADA)]),
        captura(&[tecla(Key::Tab, NADA)]),
        captura(&[tecla(Key::ArrowLeft, NADA)]),
        captura(&[tecla(Key::Backspace, NADA)]),
        captura(&[tecla(Key::F5, NADA)]),
        // `Ctrl`+letra y `Alt`+letra.
        captura(&[tecla(Key::Z, ctrl())]),
        captura(&[tecla(Key::C, ctrl())]),
        captura(&[tecla(Key::D, alt()), texto("d")]),
        // Espacio.
        captura(&[tecla(Key::Space, NADA), texto(" ")]),
    ];
    for motivo in &motivos {
        let m = motivo.as_ref().expect_err("no vale");
        assert!(m.chars().count() <= 37, "{m}");
        assert!(!m.is_empty());
    }
    assert_eq!(motivos[4], Err("esa tecla no vale".to_string()));
    assert!(motivos[9].as_ref().unwrap_err().contains("Ctrl"));
    assert!(motivos[11].as_ref().unwrap_err().contains("Alt"));
    assert!(motivos[12].as_ref().unwrap_err().contains("Espacio"));
    // IME y texto suelto no dan captura.
    assert!(captures(&[Event::Ime(ImeEvent::Commit("d".into()))]).is_empty());
    assert!(captures(&[texto("d")]).is_empty());
    assert!(captures(&[suelta(Key::D, NADA)]).is_empty());
}

#[test]
fn fila_6_escape_f1_repeticiones_y_la_primera_del_fotograma() {
    assert_eq!(captures(&[tecla(Key::Escape, NADA)]), vec![Capture::Cancel]);
    assert_eq!(captures(&[tecla(Key::Escape, mayus())]), vec![Capture::Cancel]);
    assert!(captures(&[repetida(Key::Escape, NADA)]).is_empty());
    assert_eq!(
        captura(&[tecla(Key::F1, NADA)]),
        Err("F1 es de la ayuda".to_string())
    );
    // Las repeticiones se descartan: una tecla mantenida al armar no se captura sola.
    assert!(captures(&[repetida(Key::D, NADA), texto("d")]).is_empty());
    // Vale la primera del fotograma.
    let dos = captures(&[
        tecla(Key::X, NADA),
        texto("x"),
        tecla(Key::Y, NADA),
        texto("y"),
    ]);
    assert_eq!(chord_of(match &dos[0] {
        Capture::Press(p) => p,
        otro => panic!("{otro:?}"),
    }), Ok(simbolo('x')));
}

#[test]
fn fila_6_lo_capturado_funciona_como_fila_de_la_tabla() {
    // Para cada carácter asignable de un teclado cualquiera, una fila con ese `Chord` casa con su
    // propia pulsación.
    for c in (b' '..=b'~').map(char::from) {
        if !c.is_ascii_graphic() {
            continue;
        }
        let mods = if c.is_ascii_uppercase() { mayus() } else { NADA };
        let Ok(chord) = captura(&[tecla(Key::A, mods), texto(&c.to_string())]) else {
            continue;
        };
        let fila = Binding {
            key: chord.key,
            command: false,
            shift: chord.shift,
            action: AppAction::ZoomFit,
            active: Platforms::All,
            shown: Platforms::All,
        };
        let tabla = Keymap::new(vec![fila]);
        let press = match captures(&[tecla(Key::A, mods), texto(&c.to_string())])
            .first()
            .unwrap()
        {
            Capture::Press(p) => *p,
            otro => panic!("{otro:?}"),
        };
        assert_eq!(
            tabla.action_for(&press, Platform::Linux),
            Some(AppAction::ZoomFit),
            "{c:?}"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Propiedad 3
// ---------------------------------------------------------------------------------------------

fn evento() -> impl Strategy<Value = Event> {
    let teclas = prop_oneof![
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
            ..Modifiers::NONE
        },
    );
    prop_oneof![
        (teclas, mods, any::<bool>(), any::<bool>()).prop_map(|(key, modifiers, pressed, repeat)| {
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
    fn propiedad_3_la_captura_es_total_y_lo_que_da_es_asignable(
        eventos in proptest::collection::vec(evento(), 0..8),
    ) {
        for capture in captures(&eventos) {
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
                        let fila = Binding {
                            key: chord.key,
                            command: false,
                            shift: chord.shift,
                            action: AppAction::ZoomFit,
                            active: Platforms::All,
                            shown: Platforms::All,
                        };
                        let tabla = Keymap::new(vec![fila]);
                        prop_assert_eq!(
                            tabla.action_for(&press, Platform::Linux),
                            Some(AppAction::ZoomFit)
                        );
                    }
                }
            }
        }
        // `Escape` nunca da un `Chord` y `F1` tampoco.
        for c in captures(&[tecla(Key::Escape, NADA), tecla(Key::F1, NADA)]) {
            if let Capture::Press(p) = c {
                prop_assert!(chord_of(&p).is_err());
            }
        }
    }
}

#[allow(dead_code)]
fn _usa(_: Press) {}
