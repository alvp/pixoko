//! El fichero de atajos (specs/interfaz-ayuda.md, decisiones 9 y 10 y visto bueno 1): texto propio,
//! versionado y editable a mano, sin `serde`, que guarda solo lo que difiere del esquema por
//! defecto. La lectura es total (nunca hace pánico) y tolerante. Sin E/S ni egui: la carpeta y la
//! escritura son de `px-app`.
//!
//! ```text
//! pixoko-atajos 1
//! # solo lo que difiere del esquema por defecto
//! tool.pencil 1 x
//! tool.pencil 2 none
//! color.next 1 shift+pagedown
//! ```
//!
//! Cada línea es `<clave> <casilla> <shortcut>` (la clave de la tabla de ayuda; la casilla, de 1 a
//! 8; el atajo, `none` o `[shift+]<tecla>` con una tecla ASCII imprimible que no sea mayúscula ni
//! espacio, o `home`, `pageup`, `pagedown`).

use crate::actions::AppAction;
use crate::help_texts::{action_of_key, help_key, help_table, is_editable};
use crate::keymap::{Binding, Chord, Conflict, KeyId, Keymap};
use crate::shortcut_editor::{slots_of, with_slots, without_chord};

/// Tamaño máximo del fichero: más se ignora sin recorrerlo.
pub const MAX_BYTES: usize = 64 * 1024;
/// Líneas con contenido (sin contar la cabecera, los comentarios ni las vacías) como mucho.
pub const MAX_LINES: usize = 1024;
/// Casillas por acción que admite el fichero.
pub const MAX_SLOT: usize = 8;
/// Primera palabra de la cabecera.
pub const HEADER: &str = "pixoko-atajos";
/// Versión que se lee y se escribe.
pub const VERSION: u32 = 1;

/// Por qué se ignora el fichero entero.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fatal {
    /// No es texto UTF-8 o no tiene la cabecera.
    Unreadable,
    /// Pasa de [`MAX_BYTES`] o de [`MAX_LINES`].
    TooBig,
    /// La versión de la cabecera no es la 1.
    Version,
}

/// Lo que dio leer el fichero.
#[derive(Clone, Debug, PartialEq)]
pub struct Loaded {
    /// La tabla resultante (sin conflictos; la de fábrica si hay un `fatal`).
    pub keymap: Keymap,
    /// Líneas ignoradas por estar mal.
    pub ignored_lines: usize,
    /// Atajos quitados por repetidos (una casilla repetida o una tecla de dos acciones).
    pub dropped_rows: usize,
    /// Las acciones que perdieron un atajo por un choque (la marca `conflicto` de la ventana).
    pub conflicted: Vec<AppAction>,
    pub fatal: Option<Fatal>,
}

fn fatal(default: &Keymap, why: Fatal) -> Loaded {
    Loaded {
        keymap: default.clone(),
        ignored_lines: 0,
        dropped_rows: 0,
        conflicted: Vec::new(),
        fatal: Some(why),
    }
}

/// El atajo de un `<shortcut>`: `Some(None)` es `none`; `None`, un atajo que no vale.
fn parse_chord(token: &str) -> Option<Option<Chord>> {
    if token == "none" {
        return Some(None);
    }
    let (shift, rest) = match token.strip_prefix("shift+") {
        Some(rest) => (true, rest),
        None => (false, token),
    };
    let key = match rest {
        "" => return None,
        "home" => KeyId::Home,
        "pageup" => KeyId::PageUp,
        "pagedown" => KeyId::PageDown,
        _ => {
            let mut chars = rest.chars();
            let (Some(c), None) = (chars.next(), chars.next()) else {
                return None;
            };
            if !c.is_ascii_graphic() || c.is_ascii_uppercase() {
                return None;
            }
            KeyId::Char(c)
        }
    };
    let counts = match key {
        KeyId::Char(c) => c.is_ascii_lowercase(),
        KeyId::PageUp | KeyId::PageDown => true,
        KeyId::Home => false,
    };
    if shift && !counts {
        return None;
    }
    Some(Some(Chord {
        key,
        command: false,
        shift,
    }))
}

fn chord_text(chord: Chord) -> String {
    let key = match chord.key {
        KeyId::Char(c) => c.to_string(),
        KeyId::Home => "home".to_string(),
        KeyId::PageUp => "pageup".to_string(),
        KeyId::PageDown => "pagedown".to_string(),
    };
    if chord.shift {
        format!("shift+{key}")
    } else {
        key
    }
}

fn row_chord(row: &Binding) -> Chord {
    Chord {
        key: row.key,
        command: row.command,
        shift: row.shift,
    }
}

/// Lee el fichero. Total: con cualquier `bytes` da una tabla sin conflictos y con las filas con
/// `command` intactas. Las líneas se aplican sobre las casillas de `default`; si dos filas chocan
/// gana la del usuario frente a una por defecto (que se quita y se marca como `conflicted`) y, entre
/// dos del usuario, la primera de la tabla.
pub fn parse(bytes: &[u8], default: &Keymap) -> Loaded {
    if bytes.len() > MAX_BYTES {
        return fatal(default, Fatal::TooBig);
    }
    let Ok(text) = std::str::from_utf8(bytes) else {
        return fatal(default, Fatal::Unreadable);
    };
    let mut content = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'));
    let Some(header) = content.next() else {
        return fatal(default, Fatal::Unreadable);
    };
    let mut words = header.split_whitespace();
    if words.next() != Some(HEADER) {
        return fatal(default, Fatal::Unreadable);
    }
    if words.next() != Some("1") || words.next().is_some() {
        return fatal(default, Fatal::Version);
    }

    let mut ignored = 0usize;
    let mut dropped = 0usize;
    let mut entries: Vec<(AppAction, usize, Option<Chord>)> = Vec::new();
    for (count, line) in content.enumerate() {
        if count >= MAX_LINES {
            return fatal(default, Fatal::TooBig);
        }
        let tokens: Vec<&str> = line.split_whitespace().collect();
        let parsed = match tokens.as_slice() {
            [key, slot, chord] => action_of_key(key)
                .filter(|action| is_editable(*action))
                .zip(
                    slot.parse::<usize>()
                        .ok()
                        .filter(|n| (1..=MAX_SLOT).contains(n)),
                )
                .zip(parse_chord(chord)),
            _ => None,
        };
        match parsed {
            Some(((action, slot), chord)) => {
                if entries.iter().any(|(a, s, _)| *a == action && *s == slot) {
                    ignored += 1;
                } else {
                    entries.push((action, slot, chord));
                }
            }
            None => ignored += 1,
        }
    }

    let mut keymap = default.clone();
    let mut user: Vec<(AppAction, Chord)> = Vec::new();
    for entry in help_table() {
        let Some(action) = action_of_key(entry.key) else {
            continue;
        };
        if !entries.iter().any(|(a, _, _)| *a == action) {
            continue;
        }
        let mut slots: Vec<Option<Chord>> = slots_of(default, action)
            .slots
            .into_iter()
            .map(Some)
            .collect();
        for (_, slot, chord) in entries.iter().filter(|(a, _, _)| *a == action) {
            if slots.len() < *slot {
                slots.resize(*slot, None);
            }
            slots[slot - 1] = *chord;
        }
        let mut unique: Vec<Chord> = Vec::new();
        for chord in slots.into_iter().flatten() {
            if unique.contains(&chord) {
                dropped += 1;
            } else {
                unique.push(chord);
            }
        }
        for (_, _, chord) in entries.iter().filter(|(a, _, _)| *a == action) {
            if let Some(chord) = chord
                && unique.contains(chord)
            {
                user.push((action, *chord));
            }
        }
        keymap = with_slots(&keymap, default, action, unique);
    }

    let mut conflicted: Vec<AppAction> = Vec::new();
    for _ in 0..keymap.rows().len() + MAX_SLOT {
        let Some((i, j)) = keymap.conflicts().into_iter().find_map(|c| match c {
            Conflict::Duplicate(i, j) => Some((i, j)),
            Conflict::Reserved(_) => None,
        }) else {
            break;
        };
        let (a, b) = (keymap.rows()[i].clone(), keymap.rows()[j].clone());
        let is_user = |row: &Binding| user.contains(&(row.action, row_chord(row)));
        // Gana la fila del usuario frente a una por defecto; si son las dos del mismo tipo, la
        // primera de la tabla.
        let loser = match (is_user(&a), is_user(&b)) {
            (false, true) => a,
            _ => b,
        };
        keymap = without_chord(&keymap, default, loser.action, row_chord(&loser));
        dropped += 1;
        if !conflicted.contains(&loser.action) {
            conflicted.push(loser.action);
        }
    }
    Loaded {
        keymap,
        ignored_lines: ignored,
        dropped_rows: dropped,
        conflicted,
        fatal: None,
    }
}

/// El fichero de `keymap` respecto a `default`: la cabecera y, en el orden de la tabla de ayuda,
/// una línea por cada casilla que difiere (ya compactadas). Una tabla por defecto da solo la cabecera.
pub fn to_text(keymap: &Keymap, default: &Keymap) -> String {
    let mut out = format!("{HEADER} {VERSION}\n# solo lo que difiere del esquema por defecto\n");
    for entry in help_table() {
        let Some(action) = action_of_key(entry.key) else {
            continue;
        };
        if !is_editable(action) {
            continue;
        }
        let now = slots_of(keymap, action).slots;
        let before = slots_of(default, action).slots;
        if now == before {
            continue;
        }
        for i in 0..now.len().max(before.len()).min(MAX_SLOT) {
            let (new, old) = (now.get(i), before.get(i));
            if new != old {
                let shortcut = new.map_or("none".to_string(), |chord| chord_text(*chord));
                out.push_str(&format!(
                    "{} {} {shortcut}\n",
                    help_key(action).unwrap_or(""),
                    i + 1
                ));
            }
        }
    }
    out
}

/// El aviso de 5 s al arrancar, de 60 caracteres como mucho; `None` si la lectura no ignoró nada.
pub fn load_notice(loaded: &Loaded) -> Option<String> {
    match loaded.fatal {
        Some(Fatal::Unreadable) => {
            return Some("atajos: fichero ilegible, se usan los de fábrica".to_string());
        }
        Some(Fatal::TooBig) => {
            return Some("atajos: fichero demasiado grande, se usan los de fábrica".to_string());
        }
        Some(Fatal::Version) => {
            return Some("atajos: versión desconocida, se usan los de fábrica".to_string());
        }
        None => {}
    }
    let lines = match loaded.ignored_lines {
        0 => None,
        1 => Some("1 línea ignorada".to_string()),
        n => Some(format!("{n} líneas ignoradas")),
    };
    let repeated = (loaded.dropped_rows > 0).then(|| format!("{} repetidos", loaded.dropped_rows));
    match (lines, repeated) {
        (None, None) => None,
        (Some(a), None) => Some(format!("atajos: {a}")),
        (None, Some(b)) => Some(format!("atajos: {b}")),
        (Some(a), Some(b)) => Some(format!("atajos: {a} y {b}")),
    }
}
