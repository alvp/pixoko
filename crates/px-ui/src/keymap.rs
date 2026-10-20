//! El teclado como tabla de datos (specs/interfaz-teclado.md, unidad I.1).
//!
//! `Keymap` asigna teclas a [`AppAction`]; es la única fuente de la traducción de teclas, del atajo
//! que enseña el menú y de la letra de los botones. Todo es lógica pura, sin egui: la entrada
//! normalizada es una [`Press`] (la construye `key_events` a partir de los eventos de egui).
//!
//! Se compara el **carácter que produce la tecla**, no la tecla física: `<` y `>` son la misma
//! cosa en un teclado español (tecla propia) y en uno inglés (`Mayús`+`,`), y `[` o `]` valen donde
//! el rótulo de la tecla los ponga. Las letras van en minúscula y `Mayús` aparte, para que el Bloq
//! Mayús no las rompa.

use std::sync::OnceLock;

use crate::actions::{AppAction, Platform};
use crate::tool::Tool;

/// Una tecla de la tabla: un carácter (las letras en minúscula) o una tecla con nombre.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyId {
    Char(char),
    Home,
    PageUp,
    PageDown,
}

/// En qué plataformas vale una fila o se muestra.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Platforms {
    All,
    Only(Platform),
    Except(Platform),
    /// En ninguna (una fila que funciona pero no se muestra).
    Never,
}

impl Platforms {
    /// Si `platform` está incluida.
    pub fn includes(self, platform: Platform) -> bool {
        match self {
            Platforms::All => true,
            Platforms::Only(only) => only == platform,
            Platforms::Except(except) => except != platform,
            Platforms::Never => false,
        }
    }

    pub(crate) fn overlaps(self, other: Platforms) -> bool {
        ALL_PLATFORMS
            .iter()
            .any(|&platform| self.includes(platform) && other.includes(platform))
    }
}

const ALL_PLATFORMS: [Platform; 3] = [Platform::Linux, Platform::Windows, Platform::Mac];

/// Una fila de la tabla.
///
/// `shift` solo cuenta en letras, con `command` y... en ninguna otra tecla: en un símbolo o una
/// cifra sin `command` el propio carácter ya dice si hubo `Mayús` (`<` frente a `,`), así que ahí
/// debe ir en `false`.
#[derive(Clone, Debug, PartialEq)]
pub struct Binding {
    pub key: KeyId,
    pub command: bool,
    pub shift: bool,
    pub action: AppAction,
    /// Dónde funciona la fila.
    pub active: Platforms,
    /// Dónde es la fila que se muestra en el menú y los rótulos.
    pub shown: Platforms,
}

/// Un atajo para mostrarlo: la tecla, si lleva `command` (`Ctrl` o `Cmd`) y si lleva `Mayús`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Chord {
    pub key: KeyId,
    pub command: bool,
    pub shift: bool,
}

impl Chord {
    /// Texto del atajo: `Ctrl+O`, `Cmd+Mayús+S`, `Inicio`, `d`, `>`. Con `command` la letra va en
    /// mayúscula (como los menús de siempre); sin él, como se teclea.
    pub fn text(self, platform: Platform) -> String {
        let key = match self.key {
            KeyId::Home => "Inicio".to_string(),
            KeyId::PageUp => "Re Pág".to_string(),
            KeyId::PageDown => "Av Pág".to_string(),
            KeyId::Char(c) if c.is_ascii_alphabetic() => {
                if self.command || self.shift {
                    c.to_ascii_uppercase().to_string()
                } else {
                    c.to_ascii_lowercase().to_string()
                }
            }
            KeyId::Char(c) => c.to_string(),
        };
        let mut text = String::new();
        if self.command {
            text.push_str(if platform == Platform::Mac {
                "Cmd+"
            } else {
                "Ctrl+"
            });
            if self.shift {
                text.push_str("Mayús+");
            }
        }
        text.push_str(&key);
        text
    }
}

/// Lo que el usuario ha pulsado, ya normalizado.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Press {
    /// El carácter que produce la tecla (letras en minúscula) o la tecla con nombre; `None` si no
    /// da ninguno (una tecla muerta, una composición, una tecla sin carácter).
    pub key: Option<KeyId>,
    /// `Ctrl` en Linux y Windows, `Cmd` en macOS.
    pub command: bool,
    pub shift: bool,
    pub alt: bool,
    /// Un `Ctrl` que no es `command` (el de macOS).
    pub other_ctrl: bool,
    /// Pulsación repetida por mantener la tecla.
    pub repeat: bool,
}

/// Un problema de una tabla.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Conflict {
    /// Las filas `i` y `j` (`i < j`) casan con la misma pulsación en alguna plataforma.
    Duplicate(usize, usize),
    /// La fila `i` usa una tecla reservada.
    Reserved(usize),
}

/// La tabla de teclas.
#[derive(Clone, Debug, PartialEq)]
pub struct Keymap {
    rows: Vec<Binding>,
}

fn row(
    key: KeyId,
    command: bool,
    shift: bool,
    action: AppAction,
    active: Platforms,
    shown: Platforms,
) -> Binding {
    Binding {
        key,
        command,
        shift,
        action,
        active,
        shown,
    }
}

impl Keymap {
    /// El esquema por defecto: 30 filas (21 sin `command` y 9 con él).
    pub fn default_scheme() -> Keymap {
        use Platforms::{All, Except, Never, Only};
        let c = KeyId::Char;
        let tool = |tool| AppAction::SelectTool(tool);
        Keymap::new(vec![
            row(c('d'), false, false, tool(Tool::Pencil), All, All),
            row(c('v'), false, false, tool(Tool::Line), All, All),
            row(c('f'), false, false, tool(Tool::Fill), All, All),
            row(c(','), false, false, tool(Tool::Dropper), All, All),
            row(c('z'), true, false, AppAction::Undo, All, All),
            row(c('u'), false, false, AppAction::Undo, All, Never),
            row(
                c('y'),
                true,
                false,
                AppAction::Redo,
                All,
                Except(Platform::Mac),
            ),
            row(
                c('z'),
                true,
                true,
                AppAction::Redo,
                All,
                Only(Platform::Mac),
            ),
            row(c(']'), false, false, AppAction::ColorNext, All, All),
            row(c('['), false, false, AppAction::ColorPrev, All, All),
            row(
                KeyId::PageDown,
                false,
                false,
                AppAction::ColorNext,
                All,
                Never,
            ),
            row(
                KeyId::PageUp,
                false,
                false,
                AppAction::ColorPrev,
                All,
                Never,
            ),
            row(
                KeyId::PageDown,
                false,
                true,
                AppAction::BackgroundNext,
                All,
                Never,
            ),
            row(
                KeyId::PageUp,
                false,
                true,
                AppAction::BackgroundPrev,
                All,
                Never,
            ),
            row(c('}'), false, false, AppAction::BackgroundNext, All, All),
            row(c('{'), false, false, AppAction::BackgroundPrev, All, All),
            row(c('>'), false, false, AppAction::ZoomIn, All, All),
            row(c('+'), false, false, AppAction::ZoomIn, All, Never),
            row(c('='), false, false, AppAction::ZoomIn, All, Never),
            row(c('<'), false, false, AppAction::ZoomOut, All, All),
            row(c('-'), false, false, AppAction::ZoomOut, All, Never),
            row(KeyId::Home, false, false, AppAction::ZoomFit, All, All),
            row(c('1'), true, false, AppAction::ZoomActual, All, All),
            row(c('n'), false, false, AppAction::CenterOnCursor, All, All),
            row(c('k'), false, true, AppAction::ClearPage, All, All),
            row(c('o'), true, false, AppAction::Open, All, All),
            row(c('s'), true, false, AppAction::Save, All, All),
            row(c('s'), true, true, AppAction::SaveAs, All, All),
            row(c('n'), true, false, AppAction::NewImage, All, All),
            row(
                c('q'),
                true,
                false,
                AppAction::Quit,
                Only(Platform::Linux),
                Only(Platform::Linux),
            ),
        ])
    }

    /// Una tabla con exactamente estas filas (sustitución entera, hito 8).
    pub fn new(rows: Vec<Binding>) -> Keymap {
        Keymap { rows }
    }

    pub fn rows(&self) -> &[Binding] {
        &self.rows
    }

    /// La acción de la primera fila que casa con `press` en `platform`.
    pub fn action_for(&self, press: &Press, platform: Platform) -> Option<AppAction> {
        self.rows
            .iter()
            .find(|row| matches(row, press, platform))
            .map(|row| row.action)
    }

    /// El atajo que se muestra para `action` en `platform`: la primera fila `shown` en ella.
    pub fn shown(&self, action: AppAction, platform: Platform) -> Option<Chord> {
        self.rows
            .iter()
            .find(|row| row.action == action && row.shown.includes(platform))
            .map(|row| Chord {
                key: row.key,
                command: row.command,
                shift: row.shift,
            })
    }

    /// Texto del atajo que se muestra para `action` en `platform`.
    pub fn shown_text(&self, action: AppAction, platform: Platform) -> Option<String> {
        self.shown(action, platform)
            .map(|chord| chord.text(platform))
    }

    /// Duplicados (`Duplicate(i, j)` con `i < j`, ordenados por `i` y luego `j`) y filas
    /// reservadas (`Reserved(i)`, por orden de fila, tras los duplicados).
    pub fn conflicts(&self) -> Vec<Conflict> {
        let mut conflicts = Vec::new();
        for (i, a) in self.rows.iter().enumerate() {
            for (j, b) in self.rows.iter().enumerate().skip(i + 1) {
                if same_chord(a, b) && a.active.overlaps(b.active) {
                    conflicts.push(Conflict::Duplicate(i, j));
                }
            }
        }
        for (i, row) in self.rows.iter().enumerate() {
            if is_reserved(row) {
                conflicts.push(Conflict::Reserved(i));
            }
        }
        conflicts
    }
}

/// La tabla por defecto, construida una sola vez, para las funciones que no reciben una tabla.
pub fn default_keymap() -> &'static Keymap {
    static DEFAULT: OnceLock<Keymap> = OnceLock::new();
    DEFAULT.get_or_init(Keymap::default_scheme)
}

fn is_letter(key: KeyId) -> bool {
    matches!(key, KeyId::Char(c) if c.is_ascii_alphabetic())
}

fn is_digit(key: KeyId) -> bool {
    matches!(key, KeyId::Char(c) if c.is_ascii_digit())
}

/// Si `shift` cuenta para casar la fila: en letras, con `command` y en `Re Pág` y `Av Pág`
/// (`Mayús` con ellas es el color de fondo).
fn shift_counts(row: &Binding) -> bool {
    row.command || is_letter(row.key) || matches!(row.key, KeyId::PageUp | KeyId::PageDown)
}

fn matches(row: &Binding, press: &Press, platform: Platform) -> bool {
    if !row.active.includes(platform) || row.command != press.command {
        return false;
    }
    if press.key != Some(row.key) {
        return false;
    }
    // Con `command`, `Mayús` es exacto y `Alt` anula la fila, sea la tecla que sea
    // (`Ctrl+Alt+Z` no es `Ctrl+Z`).
    if row.command {
        return press.shift == row.shift && !press.alt;
    }
    match row.key {
        // Una letra no admite `Alt` (`Alt`+`d` no es la `d`) ni un `Ctrl` que no es `command`, y
        // `Mayús` es exacto.
        KeyId::Char(_) if is_letter(row.key) => {
            !press.alt && !press.other_ctrl && press.shift == row.shift
        }
        KeyId::Char(_) if is_digit(row.key) => !press.alt && !press.other_ctrl,
        // Símbolos: el carácter ya lleva `Mayús`, AltGr u `Option`.
        KeyId::Char(_) => true,
        // `Re Pág` y `Av Pág`: `Mayús` es exacto; `Alt` y `Ctrl` suelto no cuentan.
        KeyId::PageUp | KeyId::PageDown => press.shift == row.shift,
        // `Inicio`: `Mayús`, `Alt` y `Ctrl` suelto no cuentan.
        KeyId::Home => true,
    }
}

fn same_chord(a: &Binding, b: &Binding) -> bool {
    a.key == b.key && a.command == b.command && (!shift_counts(a) || a.shift == b.shift)
}

fn is_reserved(row: &Binding) -> bool {
    match row.key {
        // Espacio, `Escape` y demás caracteres de control: son del lienzo.
        KeyId::Char(c) if !c.is_ascii_graphic() => true,
        // El portapapeles de los campos de texto.
        KeyId::Char('c' | 'x' | 'v' | 'a') if row.command && !row.shift => true,
        // `Ctrl+Q` solo en Linux: en Windows `Alt+F4` es del sistema y `Cmd+Q` también.
        KeyId::Char('q')
            if row.command
                && !row.shift
                && (row.active.includes(Platform::Windows)
                    || row.active.includes(Platform::Mac)) =>
        {
            true
        }
        _ => false,
    }
}
