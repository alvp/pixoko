//! The keyboard as a data table (specs/interfaz-teclado.md, unit I.1).
//!
//! `Keymap` assigns keys to [`AppAction`]; it is the only source of the translation of keys, of the
//! shortcut the menu shows and of the letter of the buttons. All of it is pure logic, without egui:
//! the normalized input is a [`Press`] (built by `key_events` from the egui events).
//!
//! The **character the key produces** is compared, not the physical key: `<` and `>` are the same
//! thing on a Spanish keyboard (own key) and on an English one (`Shift`+`,`), and `[` or `]` work
//! wherever the label of the key puts them. Letters go in lowercase and `Shift` apart, so that Caps
//! Lock does not break them.

use std::sync::OnceLock;

use crate::actions::{AppAction, Platform};
use crate::tool::{ShapeKind, Tool};

/// A key of the table: a character (letters in lowercase) or a named key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyId {
    Char(char),
    Home,
    PageUp,
    PageDown,
}

/// On which platforms a row works or is shown.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Platforms {
    All,
    Only(Platform),
    Except(Platform),
    /// On none (a row that works but is not shown).
    Never,
}

impl Platforms {
    /// Whether `platform` is included.
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

/// A row of the table.
///
/// `shift` only counts in letters, with `command` and... in no other key: in a symbol or a digit
/// without `command` the character itself already says whether `Shift` was held (`<` versus `,`),
/// so it must be `false` there.
#[derive(Clone, Debug, PartialEq)]
pub struct Binding {
    pub key: KeyId,
    pub command: bool,
    pub shift: bool,
    pub action: AppAction,
    /// Where the row works.
    pub active: Platforms,
    /// Where the row is the one shown in the menu and the labels.
    pub shown: Platforms,
}

/// A shortcut to show it: the key, whether it carries `command` (`Ctrl` or `Cmd`) and whether it carries `Shift`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Chord {
    pub key: KeyId,
    pub command: bool,
    pub shift: bool,
}

impl Chord {
    /// Text of the shortcut: `Ctrl+O`, `Cmd+Shift+S`, `Home`, `d`, `>`. With `command` the letter goes in
    /// uppercase (as the menus have always done); without it, as it is typed.
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
        // `Shift` with `Page Up` or `Page Down` (the background color) is said even without `command`.
        if self.shift && !self.command && matches!(self.key, KeyId::PageUp | KeyId::PageDown) {
            text.push_str("Mayús+");
        }
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

/// What the user has pressed, already normalized.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Press {
    /// The character the key produces (letters in lowercase) or the named key; `None` if it gives none
    /// (a dead key, a composition, a key without character).
    pub key: Option<KeyId>,
    /// `Ctrl` on Linux and Windows, `Cmd` on macOS.
    pub command: bool,
    pub shift: bool,
    pub alt: bool,
    /// A `Ctrl` that is not `command` (the one of macOS).
    pub other_ctrl: bool,
    /// Press repeated because the key is held down.
    pub repeat: bool,
}

/// A problem of a table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Conflict {
    /// Rows `i` and `j` (`i < j`) match the same press on some platform.
    Duplicate(usize, usize),
    /// Row `i` uses a reserved key.
    Reserved(usize),
}

/// The table of keys.
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
    /// The default scheme: 43 rows (34 without `command` and 9 with it).
    pub fn default_scheme() -> Keymap {
        use Platforms::{All, Except, Never, Only};
        let c = KeyId::Char;
        let tool = |tool| AppAction::SelectTool(tool);
        let shape = AppAction::SelectShape;
        Keymap::new(vec![
            row(c('d'), false, false, tool(Tool::Pencil), All, All),
            row(c('v'), false, false, tool(Tool::Line), All, All),
            row(c('f'), false, false, tool(Tool::Fill), All, All),
            row(c(','), false, false, tool(Tool::Dropper), All, All),
            row(c('k'), false, false, tool(Tool::Eraser), All, All),
            row(c('s'), false, false, AppAction::SelectDotted, All, All),
            row(c('q'), false, false, AppAction::SelectCurve, All, All),
            row(c('r'), false, false, shape(ShapeKind::Rectangle), All, All),
            row(c('c'), false, false, shape(ShapeKind::Circle), All, All),
            row(c('e'), false, false, shape(ShapeKind::Ellipse), All, All),
            row(c('w'), false, false, shape(ShapeKind::Polygon), All, All),
            row(c('d'), false, true, shape(ShapeKind::Freeform), All, All),
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
            row(c('+'), false, false, AppAction::BrushBigger, All, All),
            row(c('='), false, false, AppAction::BrushBigger, All, Never),
            row(c('<'), false, false, AppAction::ZoomOut, All, All),
            row(c('-'), false, false, AppAction::BrushSmaller, All, All),
            row(KeyId::Home, false, false, AppAction::ZoomFit, All, All),
            row(c('1'), true, false, AppAction::ZoomActualSize, All, All),
            row(c('n'), false, false, AppAction::CenterOnCursor, All, All),
            row(c('k'), false, true, AppAction::ClearPage, All, All),
            row(c('\''), false, false, AppAction::ToggleLayerPanel, All, All),
            row(c('p'), false, false, AppAction::ToggleColorEditor, All, All),
            row(c(';'), false, false, AppAction::NewLayer, All, All),
            row(c(')'), false, false, AppAction::LayerUp, All, All),
            row(c('('), false, false, AppAction::LayerDown, All, All),
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

    /// A table with exactly these rows (whole replacement, milestone 8).
    pub fn new(rows: Vec<Binding>) -> Keymap {
        Keymap { rows }
    }

    pub fn rows(&self) -> &[Binding] {
        &self.rows
    }

    /// The action of the first row that matches `press` on `platform`.
    pub fn action_for(&self, press: &Press, platform: Platform) -> Option<AppAction> {
        self.rows
            .iter()
            .find(|row| matches(row, press, platform))
            .map(|row| row.action)
    }

    /// The shortcut shown for `action` on `platform`: the first `shown` row on it.
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

    /// Text of the shortcut shown for `action` on `platform`.
    pub fn shown_text(&self, action: AppAction, platform: Platform) -> Option<String> {
        self.shown(action, platform)
            .map(|chord| chord.text(platform))
    }

    /// Duplicates (`Duplicate(i, j)` with `i < j`, sorted by `i` and then `j`) and reserved rows
    /// (`Reserved(i)`, in row order, after the duplicates).
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

/// The default table, built only once, for the functions that do not receive a table.
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

/// Whether `shift` counts to match the row: in letters, with `command` and in `Page Up` and `Page Down`
/// (`Shift` with them is the background color).
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
    // With `command`, `Shift` is exact and `Alt` cancels the row, whatever the key
    // (`Ctrl+Alt+Z` is not `Ctrl+Z`).
    if row.command {
        return press.shift == row.shift && !press.alt;
    }
    match row.key {
        // A letter does not admit `Alt` (`Alt`+`d` is not `d`) nor a `Ctrl` that is not `command`, and
        // `Shift` is exact.
        KeyId::Char(_) if is_letter(row.key) => {
            !press.alt && !press.other_ctrl && press.shift == row.shift
        }
        KeyId::Char(_) if is_digit(row.key) => !press.alt && !press.other_ctrl,
        // Symbols: the character already carries `Shift`, AltGr or `Option`.
        KeyId::Char(_) => true,
        // `Page Up` and `Page Down`: `Shift` is exact; `Alt` and a lone `Ctrl` do not count.
        KeyId::PageUp | KeyId::PageDown => press.shift == row.shift,
        // `Home`: `Shift`, `Alt` and a lone `Ctrl` do not count.
        KeyId::Home => true,
    }
}

fn same_chord(a: &Binding, b: &Binding) -> bool {
    a.key == b.key && a.command == b.command && (!shift_counts(a) || a.shift == b.shift)
}

fn is_reserved(row: &Binding) -> bool {
    match row.key {
        // Space, `Escape` and the other control characters: they belong to the canvas.
        KeyId::Char(c) if !c.is_ascii_graphic() => true,
        // The clipboard of the text fields.
        KeyId::Char('c' | 'x' | 'v' | 'a') if row.command && !row.shift => true,
        // `Ctrl+Q` only on Linux: on Windows `Alt+F4` belongs to the system and so does `Cmd+Q`.
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
