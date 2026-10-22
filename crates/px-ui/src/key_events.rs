//! De los eventos de egui a [`Press`] (specs/interfaz-teclado.md, decisión 1). Usa `egui::Event`
//! como dato, sin `Context`: se prueba sin ventana.
//!
//! `egui-winit` manda un `Event::Key` por cada pulsación y, justo después, un `Event::Text` con el
//! carácter que produce (no lo manda con `Ctrl` ni `Cmd`). Se empareja cada tecla con el texto que
//! le sigue y se compara el **carácter**, no la tecla física.

use egui::{Event, Key, Modifiers};

use crate::help_texts::ui;
use crate::keymap::{Chord, KeyId, Press};

/// La `Press` de cada `Event::Key` pulsado de `events`, en orden. Un `Event::Text` suelto (que no
/// sigue a una tecla pulsada) y los `Event::Ime` no dan ninguna: un IME o un teclado en pantalla
/// no disparan atajos.
///
/// Carácter de la `Press`: con `command`, el de la tecla de egui (con `Ctrl` o `Cmd` no hay texto);
/// sin `command`, solo el texto, si es ASCII imprimible (las letras en minúscula, con `Mayús`
/// aparte). Si el texto es una letra de otra escritura (ruso, griego…) vale el de la tecla de
/// egui, como hace `egui-winit` con el portapapeles. No dan carácter una tecla muerta (sin
/// texto), una vocal con acento, un texto de varios caracteres ni un símbolo que no es ASCII
/// (`∂` de `Option`+`d`). Las teclas con nombre (`Start`, `Re Pág`, `Av Pág`) valen siempre.
pub fn presses(events: &[Event]) -> Vec<Press> {
    let mut out = Vec::new();
    for (i, event) in events.iter().enumerate() {
        let Event::Key {
            key,
            pressed: true,
            repeat,
            modifiers,
            ..
        } = event
        else {
            continue;
        };
        let text = match events.get(i + 1) {
            Some(Event::Text(text)) => Some(text.as_str()),
            _ => None,
        };
        out.push(Press {
            key: key_of(*key, *modifiers, text),
            command: modifiers.command,
            shift: modifiers.shift,
            alt: modifiers.alt,
            other_ctrl: other_ctrl(*modifiers),
            repeat: *repeat,
        });
    }
    out
}

/// Lo que dio un fotograma a la captura de un atajo (specs/interfaz-ayuda.md, decisión 6).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Capture {
    /// `Escape`: desarma la captura.
    Cancel,
    /// Una tecla que no se puede asignar y por qué (`F1`, de la ayuda).
    Reserved(&'static str),
    /// Una pulsación, para `chord_of`.
    Press(Press),
}

/// Las capturas de `events`, en orden: `Escape` (con cualquier modificador) es `Cancel`, `F1` es
/// `Reserved` y el resto, la `Press` con el mismo emparejado `Key`+`Text` que [`presses`]. Se
/// descartan las repeticiones (una tecla mantenida al armar la captura no se captura sola); un
/// `Text` suelto y los `Event::Ime` no dan nada.
pub fn captures(events: &[Event]) -> Vec<Capture> {
    let mut out = Vec::new();
    for (i, event) in events.iter().enumerate() {
        let Event::Key {
            key,
            pressed: true,
            repeat: false,
            modifiers,
            ..
        } = event
        else {
            continue;
        };
        match key {
            Key::Escape => out.push(Capture::Cancel),
            Key::F1 => out.push(Capture::Reserved(ui().f1_reserved)),
            _ => {
                let text = match events.get(i + 1) {
                    Some(Event::Text(text)) => Some(text.as_str()),
                    _ => None,
                };
                out.push(Capture::Press(Press {
                    key: key_of(*key, *modifiers, text),
                    command: modifiers.command,
                    shift: modifiers.shift,
                    alt: modifiers.alt,
                    other_ctrl: other_ctrl(*modifiers),
                    repeat: false,
                }));
            }
        }
    }
    out
}

/// `F1` pulsada, sin modificadores y sin repetición: pide la ayuda.
pub fn help_pressed(events: &[Event]) -> bool {
    events.iter().any(|event| {
        matches!(
            event,
            Event::Key {
                key: Key::F1,
                pressed: true,
                repeat: false,
                modifiers,
                ..
            } if modifiers.is_none()
        )
    })
}

/// El atajo que se puede asignar con una pulsación, o por qué no (≤ 37 caracteres, para la línea
/// de la casilla). Sin carácter (tecla muerta, composición, `Intro`, flechas, teclas de función):
/// no vale; con `Ctrl` o `Cmd`: no se asigna (son las combinaciones modernas); `Alt` o el `Ctrl`
/// de macOS con letra o cifra: no es esa letra; `Espacio` es del lienzo. `Mayús` solo cuenta en
/// letras y en `Re Pág`/`Av Pág`.
pub fn chord_of(press: &Press) -> Result<Chord, &'static str> {
    let strings = ui();
    let Some(key) = press.key else {
        return Err(strings.not_a_key);
    };
    if press.command {
        return Err(strings.no_command);
    }
    let chord = |key, shift| Chord {
        key,
        command: false,
        shift,
    };
    match key {
        KeyId::Char(' ') => Err(strings.no_space),
        KeyId::Char(c) if !c.is_ascii_graphic() => Err(strings.not_a_key),
        KeyId::Char(c) => {
            if c.is_ascii_alphanumeric() && (press.alt || press.other_ctrl) {
                return Err(strings.no_alt_letter);
            }
            Ok(chord(
                KeyId::Char(c.to_ascii_lowercase()),
                c.is_ascii_alphabetic() && press.shift,
            ))
        }
        KeyId::Home => Ok(chord(KeyId::Home, false)),
        KeyId::PageUp | KeyId::PageDown => Ok(chord(key, press.shift)),
    }
}

/// La `Press` de una tecla y unos modificadores sin texto (pruebas y funciones antiguas): la
/// tecla se toma como tecleada. `Mayús`, `Alt` o un `Ctrl` que no es `command` con una cifra o un
/// símbolo dejan la tecla sin carácter (cuál sale depende de la distribución), salvo `+`, `=` y
/// `-`, que valen con cualquier modificador sin `command`, como hasta ahora; las letras llevan
/// `Mayús` y `Alt` aparte y las teclas con nombre valen con cualquier modificador.
pub fn press_from_key(key: Key, modifiers: Modifiers) -> Press {
    let id = key_id(key);
    let free = matches!(
        id,
        Some(KeyId::Home | KeyId::PageUp | KeyId::PageDown) | Some(KeyId::Char('+' | '=' | '-'))
    );
    let letter = matches!(id, Some(KeyId::Char(c)) if c.is_ascii_alphabetic());
    let plain = !(modifiers.shift || modifiers.alt || other_ctrl(modifiers));
    let key = if free || letter || modifiers.command || plain {
        id
    } else {
        None
    };
    Press {
        key,
        command: modifiers.command,
        shift: modifiers.shift,
        alt: modifiers.alt,
        other_ctrl: other_ctrl(modifiers),
        repeat: false,
    }
}

/// Un `Ctrl` (o `Cmd`) que no es `command`: el `Ctrl` de macOS.
fn other_ctrl(modifiers: Modifiers) -> bool {
    (modifiers.ctrl || modifiers.mac_cmd) && !modifiers.command
}

/// La tecla de una pulsación con el texto que le siguió.
fn key_of(key: Key, modifiers: Modifiers, text: Option<&str>) -> Option<KeyId> {
    // Las teclas con nombre no producen texto: valen por la tecla.
    if let Some(named @ (KeyId::Home | KeyId::PageUp | KeyId::PageDown)) = key_id(key) {
        return Some(named);
    }
    if modifiers.command {
        return key_id(key);
    }
    let text = text?;
    let mut chars = text.chars();
    let (Some(c), None) = (chars.next(), chars.next()) else {
        return None;
    };
    if (' '..='~').contains(&c) {
        return Some(KeyId::Char(c.to_ascii_lowercase()));
    }
    if c.is_alphabetic() && !is_latin(c) {
        return key_id(key);
    }
    None
}

/// ASCII, Latin-1, Latín extendido A y B y Latín extendido adicional.
fn is_latin(c: char) -> bool {
    c.is_ascii() || ('\u{a0}'..='\u{24f}').contains(&c) || ('\u{1e00}'..='\u{1eff}').contains(&c)
}

/// El carácter o la tecla con nombre de una tecla de egui, como se teclearía sin `Mayús`.
fn key_id(key: Key) -> Option<KeyId> {
    let c = match key {
        Key::Home => return Some(KeyId::Home),
        Key::PageUp => return Some(KeyId::PageUp),
        Key::PageDown => return Some(KeyId::PageDown),
        Key::Num0 => '0',
        Key::Num1 => '1',
        Key::Num2 => '2',
        Key::Num3 => '3',
        Key::Num4 => '4',
        Key::Num5 => '5',
        Key::Num6 => '6',
        Key::Num7 => '7',
        Key::Num8 => '8',
        Key::Num9 => '9',
        Key::Comma => ',',
        Key::Period => '.',
        Key::Minus => '-',
        Key::Plus => '+',
        Key::Equals => '=',
        Key::Semicolon => ';',
        Key::Colon => ':',
        Key::Slash => '/',
        Key::Backslash => '\\',
        Key::Pipe => '|',
        Key::Questionmark => '?',
        Key::Exclamationmark => '!',
        Key::OpenBracket => '[',
        Key::CloseBracket => ']',
        Key::OpenCurlyBracket => '{',
        Key::CloseCurlyBracket => '}',
        Key::Backtick => '`',
        Key::Quote => '\'',
        other => {
            let name = other.name();
            let mut chars = name.chars();
            match (chars.next(), chars.next()) {
                (Some(letter), None) if letter.is_ascii_uppercase() => letter.to_ascii_lowercase(),
                _ => return None,
            }
        }
    };
    Some(KeyId::Char(c))
}
