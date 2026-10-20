//! De los eventos de egui a [`Press`] (specs/interfaz-teclado.md, decisión 1). Usa `egui::Event`
//! como dato, sin `Context`: se prueba sin ventana.
//!
//! `egui-winit` manda un `Event::Key` por cada pulsación y, justo después, un `Event::Text` con el
//! carácter que produce (no lo manda con `Ctrl` ni `Cmd`). Se empareja cada tecla con el texto que
//! le sigue y se compara el **carácter**, no la tecla física.

use egui::{Event, Key, Modifiers};

use crate::keymap::{KeyId, Press};

/// La `Press` de cada `Event::Key` pulsado de `events`, en orden. Un `Event::Text` suelto (que no
/// sigue a una tecla pulsada) y los `Event::Ime` no dan ninguna: un IME o un teclado en pantalla
/// no disparan atajos.
///
/// Carácter de la `Press`: con `command`, el de la tecla de egui (con `Ctrl` o `Cmd` no hay texto);
/// sin `command`, solo el texto, si es ASCII imprimible (las letras en minúscula, con `Mayús`
/// aparte). Si el texto es una letra de otra escritura (ruso, griego…) vale el de la tecla de
/// egui, como hace `egui-winit` con el portapapeles. No dan carácter una tecla muerta (sin
/// texto), una vocal con acento, un texto de varios caracteres ni un símbolo que no es ASCII
/// (`∂` de `Option`+`d`). Las teclas con nombre (`Inicio`, `Re Pág`, `Av Pág`) valen siempre.
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
