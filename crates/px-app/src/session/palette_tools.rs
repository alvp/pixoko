//! The palette operations of the `Paleta` menu (specs/hito-4-paleta.md, unit 4.6): copy and
//! paste, swap, gradient, delete several, default palette, loading and saving palette files and
//! replacing or swapping a color in the whole image. Each one is a single undo step.

use px_core::{Command, DocumentError, Palette, Rgba};
use px_ui::palette_file::{PaletteFormat, parse_palette, write_palette};
use px_ui::palette_tools::{
    PaletteTool, default_palette, gradient_colors, replace_color_command, selected_entries,
    swap_colors_command, swap_order,
};
use px_ui::palette_tools_es::{
    NOTICE_COPIED, NOTICE_DELETE_ALL, NOTICE_NEED_RANGE, NOTICE_NEED_TWO, NOTICE_NO_PIXELS,
    NOTICE_NOT_INDEXED, NOTICE_NOTHING_COPIED, NOTICE_SAME_COLOR, deleted_notice, file_error_text,
    loaded_notice,
};

use super::{PaletteOutcome, Role, Session};

fn said(text: &str) -> Result<PaletteOutcome, DocumentError> {
    Ok(PaletteOutcome {
        changed: false,
        notice: Some(text.to_string()),
    })
}

impl Session {
    /// The entries marked in the panel: the range with `Shift`, the ones toggled with `Ctrl` and
    /// the active one.
    fn marked_entries(&self) -> Vec<u8> {
        let len = self.document.palette().map_or(0, Palette::len);
        let Some(active) = self.active_entry() else {
            return Vec::new();
        };
        selected_entries(active, self.ops.range_end(), self.ops.extras())
            .into_iter()
            .filter(|&entry| usize::from(entry) < len)
            .collect()
    }

    /// Applies a command of the palette tools as one step and, if it changed something, drops the
    /// range and the pending text of the hexadecimal field.
    fn apply_tool_command(
        &mut self,
        command: Command,
        notice: Option<String>,
    ) -> Result<PaletteOutcome, DocumentError> {
        let changed = self.apply_layer(command)?;
        if changed {
            self.ops.clear_range();
            self.editor.discard_pending();
        }
        Ok(PaletteOutcome {
            changed,
            notice: notice.filter(|_| changed),
        })
    }

    /// One tool of the `Paleta` menu except the file ones (`load_palette_text` and
    /// `palette_file_text` do those).
    pub fn palette_tool(&mut self, tool: PaletteTool) -> Result<PaletteOutcome, DocumentError> {
        let id = self.active_cel();
        let Some(palette) = self.document.palette().cloned() else {
            return if tool.needs_indexed() {
                said(NOTICE_NOT_INDEXED)
            } else {
                self.replace_in_image(tool)
            };
        };
        let marked = self.marked_entries();
        let stored = |entry: u8| palette.get(entry).unwrap_or_default();
        match tool {
            PaletteTool::Copy => {
                let colors: Vec<Rgba> = marked.iter().map(|&entry| stored(entry)).collect();
                self.ops.set_clipboard(colors);
                said(NOTICE_COPIED)
            }
            PaletteTool::Paste => {
                let start = self.active_entry().unwrap_or(0);
                let mut colors = self.ops.clipboard().to_vec();
                if colors.is_empty() {
                    return said(NOTICE_NOTHING_COPIED);
                }
                colors.truncate(Palette::MAX_COLORS - usize::from(start));
                let command = Command::SetPaletteRange {
                    image: id,
                    start,
                    colors,
                };
                self.apply_tool_command(command, None)
            }
            PaletteTool::Swap => {
                let &[a, b] = marked.as_slice() else {
                    return said(NOTICE_NEED_TWO);
                };
                let command = Command::PermutePalette {
                    image: id,
                    order: swap_order(palette.len(), a, b),
                };
                self.apply_tool_command(command, None)
            }
            PaletteTool::Gradient => {
                let (Some(&first), Some(&last)) = (marked.first(), marked.last()) else {
                    return said(NOTICE_NEED_RANGE);
                };
                let steps = usize::from(last - first);
                if steps < 2 {
                    return said(NOTICE_NEED_RANGE);
                }
                let ramp = gradient_colors(stored(first), stored(last), steps);
                let command = Command::SetPaletteRange {
                    image: id,
                    start: first + 1,
                    colors: ramp[1..steps].to_vec(),
                };
                self.apply_tool_command(command, None)
            }
            PaletteTool::Delete => {
                if marked.len() >= palette.len() {
                    return said(NOTICE_DELETE_ALL);
                }
                if let [_] = marked.as_slice() {
                    return self.menu_remove_entry();
                }
                // The destination is the kept entry nearest to the active one (no pending step).
                let kept: Vec<u8> = (0..palette.len() as u8)
                    .filter(|entry| !marked.contains(entry))
                    .collect();
                let active = self.active_entry().unwrap_or(0);
                let reference = palette.resolved(active).unwrap_or_default();
                let nearest = Palette::from_colors(kept.iter().map(|&entry| stored(entry)))
                    .ok()
                    .and_then(|kept_palette| kept_palette.nearest(reference))
                    .map_or(0, usize::from);
                let into = kept.get(nearest).copied().unwrap_or(kept[0]);
                let command = Command::RemovePaletteEntries {
                    image: id,
                    indices: marked.clone(),
                    into,
                };
                let notice = deleted_notice(marked.len(), nearest);
                self.apply_tool_command(command, Some(notice))
            }
            PaletteTool::Default => {
                let mut colors = default_palette();
                colors.truncate(palette.len());
                let command = Command::SetPaletteRange {
                    image: id,
                    start: 0,
                    colors,
                };
                self.apply_tool_command(command, None)
            }
            PaletteTool::ReplaceColor | PaletteTool::SwapColors => self.replace_in_image(tool),
            // The file tools are run by the app with a file dialog.
            PaletteTool::Load | PaletteTool::LoadAt | PaletteTool::Save => Ok(PaletteOutcome {
                changed: false,
                notice: None,
            }),
        }
    }

    /// `Cambiar color` and `Intercambiar` in the image: foreground and background pixels.
    fn replace_in_image(&mut self, tool: PaletteTool) -> Result<PaletteOutcome, DocumentError> {
        let (Some(from), Some(to)) = (
            self.pixel_of(Role::Foreground),
            self.pixel_of(Role::Background),
        ) else {
            return said(NOTICE_SAME_COLOR);
        };
        if from == to {
            return said(NOTICE_SAME_COLOR);
        }
        let command = if tool == PaletteTool::SwapColors {
            swap_colors_command(&self.document, from, to)?
        } else {
            replace_color_command(&self.document, from, to)?
        };
        match command {
            Some(command) => self.apply_tool_command(command, None),
            None => said(NOTICE_NO_PIXELS),
        }
    }

    /// Writes the colors of a palette file from entry 0 (`at_active` false) or from the active entry;
    /// the palette grows if needed and never shrinks. The notice says how many colors went in.
    pub fn load_palette_text(
        &mut self,
        text: &str,
        at_active: bool,
    ) -> Result<PaletteOutcome, DocumentError> {
        if self.document.palette().is_none() {
            return said(NOTICE_NOT_INDEXED);
        }
        let loaded = match parse_palette(text) {
            Ok(loaded) => loaded,
            Err(error) => return said(&file_error_text(error)),
        };
        let start = if at_active {
            self.active_entry().unwrap_or(0)
        } else {
            0
        };
        let room = Palette::MAX_COLORS - usize::from(start);
        let mut colors = loaded.colors;
        let cut = colors.len().saturating_sub(room);
        colors.truncate(room);
        let notice = loaded_notice(colors.len(), loaded.ignored + cut);
        let command = Command::SetPaletteRange {
            image: self.active_cel(),
            start,
            colors,
        };
        let outcome = self.apply_tool_command(command, Some(notice.clone()))?;
        Ok(PaletteOutcome {
            notice: Some(notice),
            ..outcome
        })
    }

    /// The text of the whole palette in `format`, for `Guardar paleta…`.
    pub fn palette_file_text(&self, format: PaletteFormat) -> Option<String> {
        let palette = self.document.palette()?;
        Some(write_palette(palette.colors(), format))
    }
}
