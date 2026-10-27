//! The window of options of the active tool (specs/ui-single-column.md, decisions 8 and 11), without
//! egui: its geometry in `u`, the text of today, the brief help, the scroll by rows and the state
//! (open or closed, position and scroll by tool). The painting is in `options_view`.
//!
//! There is only one window: it follows the active tool and its POSITION is one and shared by all the
//! tools; only the scroll of the body is remembered by tool. Today no tool has options: the window
//! shows the name, the shortcut of the table, `sin opciones` and a brief help. Milestone 4 extends
//! [`options_row_count`] (and the body) without touching the rest.

use crate::actions::{AppAction, Platform};
use crate::float_window;
use crate::help_texts::{help_entry, help_key};
use crate::keymap::Keymap;
use crate::tool::{Tool, default_option_rows};
use crate::view::ScreenRect;

/// Text of the body when the tool has no options.
pub const NO_OPTIONS: &str = "sin opciones";

/// Rows of the body that one notch of the wheel scrolls.
pub const SCROLL_ROWS_PER_NOTCH: i64 = 3;

/// Width of the window and of one row of the body, in `u`.
pub const WINDOW_W_U: i64 = 224;
pub const ROW_H_U: i64 = 18;
/// Rows of the body that are seen at once; with more, the body scrolls by whole rows.
pub const BODY_MAX_ROWS: usize = 8;
/// Title bar (the name and the `x`), the line of the shortcut and the start of the body.
pub const TITLE_BAR_H_U: i64 = 16;
pub const CLOSE_X_U: i64 = 207;
pub const KEY_Y_U: i64 = 21;
pub const BODY_Y_U: i64 = 39;
/// Where the texts start inside the window, in `u`.
pub const TEXT_X_U: i64 = 5;
pub const TITLE_TEXT_Y_U: i64 = 2;
/// Width of the scrollbar of the body (only with more than [`BODY_MAX_ROWS`] rows) and its `x`.
pub const SCROLLBAR_W_U: i64 = 8;
pub const SCROLLBAR_X_U: i64 = 211;
/// The brief help: lines of 14 u, 30 columns wide, at most four.
pub const HELP_LINES: usize = 4;
pub const HELP_COLUMNS: usize = 30;
pub const HELP_LINE_H_U: i64 = 14;
/// Distance of the first position to the right of the canvas and to the layers panel, in `u`.
const DEFAULT_MARGIN_U: i64 = 8;
const DEFAULT_CASCADE_U: i64 = 24;

/// Rows of options of `tool` with the default state (a continuous spacing): 3 for the pencil, 4
/// for the line and 0 for the rest. The line grows to 5 with a spacing that is not continuous
/// (`ToolState::option_rows`).
pub fn options_row_count(tool: Tool) -> usize {
    default_option_rows(tool).len()
}

/// Highest scroll of the body, in rows, with `rows` rows of options.
pub fn options_max_scroll(rows: usize) -> usize {
    rows.saturating_sub(BODY_MAX_ROWS)
}

/// Rows of the body that are seen: from 1 (`sin opciones` takes one) to 8.
pub fn body_rows(rows: usize) -> usize {
    rows.clamp(1, BODY_MAX_ROWS)
}

/// Height of the window with `rows` rows of options, in `u`: `104 + 18·f`.
pub fn window_height_u(rows: usize) -> i64 {
    104 + ROW_H_U * body_rows(rows) as i64
}

/// `y` of the brief help: 4 u under the body.
pub fn help_y_u(rows: usize) -> i64 {
    BODY_Y_U + ROW_H_U * body_rows(rows) as i64 + 4
}

/// Title of the window: the name of the tool, the same label as its button.
pub fn title(tool: Tool) -> &'static str {
    match tool {
        Tool::Pencil => "Lápiz",
        Tool::Line => "Línea",
        Tool::Fill => "Relleno",
        Tool::Dropper => "Cuentagotas",
        Tool::Eraser => "Borrador",
    }
}

/// Line of the shortcut, read from the table: `tecla d`, or `sin tecla` if the table gives none.
pub fn key_line(tool: Tool, keymap: &Keymap, platform: Platform) -> String {
    match keymap.shown_text(AppAction::SelectTool(tool), platform) {
        Some(text) => format!("tecla {text}"),
        None => "sin tecla".to_string(),
    }
}

/// The brief help of the tool: the first sentence of the description of its help.
pub fn brief_help(tool: Tool) -> String {
    let Some(entry) = help_key(AppAction::SelectTool(tool)).and_then(help_entry) else {
        return String::new();
    };
    let text = entry.description;
    let end = text
        .match_indices(". ")
        .next()
        .map_or(text.len(), |(i, _)| i + 1);
    text[..end].to_string()
}

/// The text wrapped at [`HELP_COLUMNS`] columns by words (a word longer than the line is cut). The
/// lines joined with a space give back the text.
pub fn wrap_help(text: &str) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for word in text.split_whitespace() {
        let mut word = word.to_string();
        loop {
            let fits = lines.last().is_some_and(|line| {
                line.chars().count() + 1 + word.chars().count() <= HELP_COLUMNS
            });
            if fits {
                if let Some(line) = lines.last_mut() {
                    line.push(' ');
                    line.push_str(&word);
                }
                break;
            }
            if word.chars().count() <= HELP_COLUMNS {
                lines.push(word);
                break;
            }
            let head: String = word.chars().take(HELP_COLUMNS).collect();
            word = word.chars().skip(HELP_COLUMNS).collect();
            lines.push(head);
        }
    }
    lines
}

fn tool_index(tool: Tool) -> usize {
    match tool {
        Tool::Pencil => 0,
        Tool::Line => 1,
        Tool::Fill => 2,
        Tool::Dropper => 3,
        Tool::Eraser => 4,
    }
}

/// The state of the window: open or closed, where it is (`u` from the corner of the central strip;
/// `None` until the first frame: the first position), the position at which a drag began and the scroll
/// of the body of each tool. Not of the document, and not saved (preferences: milestone 8).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OptionsWindow {
    pub open: bool,
    pub pos_u: Option<(i64, i64)>,
    /// The position when the drag of the title bar began.
    pub drag_from: Option<(i64, i64)>,
    /// The scroll of the body, in rows, of each tool (the order of the five buttons).
    pub scroll: [usize; 5],
}

impl OptionsWindow {
    /// A right click on the button of `clicked` while `active` was the tool: it closes the window if
    /// it was open showing that tool; otherwise it opens it (a closed one, or one that was showing
    /// another tool, which now shows this one). Choosing the tool is the caller's.
    pub fn right_click(&mut self, active: Tool, clicked: Tool) {
        self.open = !(self.open && active == clicked);
    }

    /// Closes the window (its `x`, `Esc`).
    pub fn close(&mut self) {
        self.open = false;
    }

    /// The scroll of the body of `tool`, in rows.
    pub fn scroll_of(&self, tool: Tool) -> usize {
        self.scroll[tool_index(tool)]
    }

    /// Scrolls the body of `tool` `delta` rows (positive: down), clamped to `0..=max` with `rows` rows
    /// of options.
    pub fn scroll_by(&mut self, tool: Tool, delta: i64, rows: usize) {
        let max = options_max_scroll(rows) as i64;
        let slot = &mut self.scroll[tool_index(tool)];
        *slot = (*slot as i64 + delta).clamp(0, max) as usize;
    }

    /// The first position (`u` from the strip): against the right of the canvas with 8 u of margin, 24
    /// u to the left and 24 u lower than where the layers panel starts.
    fn default_pos_u(window_px: (i64, i64), n: u32, wanted_cols: i64) -> (i64, i64) {
        let k = i64::from(n.max(1));
        let canvas = crate::chrome::window_layout_cols(window_px, n, wanted_cols).canvas;
        (
            (canvas.width / k - WINDOW_W_U - DEFAULT_MARGIN_U - DEFAULT_CASCADE_U).max(0),
            DEFAULT_MARGIN_U + DEFAULT_CASCADE_U,
        )
    }

    /// The rectangle of the window in this window of the app (physical pixels), without saving
    /// anything (for `F1` and the clicks).
    pub fn current_rect(
        &self,
        rows: usize,
        window_px: (i64, i64),
        n: u32,
        wanted_cols: i64,
    ) -> ScreenRect {
        let pos = self
            .pos_u
            .unwrap_or_else(|| Self::default_pos_u(window_px, n, wanted_cols));
        float_window::place(pos, (WINDOW_W_U, window_height_u(rows)), window_px, n)
    }

    /// Like [`OptionsWindow::current_rect`], saving the position already clamped, so a change of the
    /// size of the window or of the scale keeps it in `u`.
    pub fn rect(
        &mut self,
        rows: usize,
        window_px: (i64, i64),
        n: u32,
        wanted_cols: i64,
    ) -> ScreenRect {
        let rect = self.current_rect(rows, window_px, n, wanted_cols);
        self.pos_u = Some(float_window::pos_of(rect, window_px, n));
        rect
    }

    /// Takes the window to `from` plus the whole displacement `dx_px`, `dy_px` (without adding
    /// roundings frame after frame).
    pub fn drag_total(&mut self, from: (i64, i64), dx_px: i64, dy_px: i64, n: u32) {
        self.pos_u = Some(float_window::dragged(from, dx_px, dy_px, n));
    }
}
