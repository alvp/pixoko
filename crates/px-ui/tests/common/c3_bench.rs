//! Window-less egui bench of the palette panel for unit C3 (specs/ui-palette-handle.md): the handle,
//! the pending resize, the color bar and the hexadecimal field under the grid.
//!
//! One `egui::Context` runs frames with synthetic `RawInput`, without GPU or screen. Each frame calls
//! `PaletteOps::sync`, `ColorEditor::sync`, `status::show`, then `palette_panel::show_roles` and
//! last the canvas, as the app does. The bench never applies what the panel asks for (`op`,
//! `commit`): the tests do it with `apply_*` when they need the new image. Nothing is captured from
//! the desktop: the painted shapes are read from the frame.

#![allow(dead_code)]

use egui::epaint::{ClippedShape, Shape};
use egui::{
    Color32, Context, CursorIcon, Event, Key, MissingGlyphPolicy, Modifiers, MouseWheelSource,
    MouseWheelUnit, PointerButton, Pos2, RawInput, Rect, TouchPhase, Vec2, pos2, vec2,
};
use px_core::{Command, Document, Image, ImageId, Palette, Pixel, Rgba};
use px_ui::canvas::Canvas;
use px_ui::color::{ColorChoice, Role, SlotColors};
use px_ui::color_edit::{ColorEditor, EditTarget};
use px_ui::edit_layout::{PanelLayout, panel_layout, panel_layout_closed};
use px_ui::icons::IconSet;
use px_ui::palette_grid::{
    self, CELL_U, GRID_X_U, GRID_Y_U, PANEL_WIDTH_U, PITCH_U, PanelView, ring_rects, total_rows,
};
use px_ui::palette_ops::PaletteOps;
use px_ui::palette_panel::{PanelInfo, PanelOutput, Roles, panel_info, show_roles};
use px_ui::status::bar_height_px;
use px_ui::text::{apply_scale, install_fonts, text_scale};
use px_ui::view::ScreenRect;

use super::icons::IconsOfTest;
use super::zones::panel_on_the_left;

pub const PPPS: [f32; 5] = [1.0, 1.25, 1.5, 2.0, 3.0];
pub const TOLERANCE: f32 = 0.05;
pub const SCREEN_HIGH: Vec2 = vec2(800.0, 600.0);

/// Indexed `count`-entry image whose colors are all different and none is opaque black.
pub fn indexed(count: usize) -> Image {
    let colors: Vec<Rgba> = (0..count)
        .map(|i| {
            Rgba::opaque(
                20 + (i * 7 % 200) as u8,
                255 - (i * 5 % 200) as u8,
                40 + i as u8 % 150,
            )
        })
        .collect();
    let palette = Palette::from_colors(colors).expect("palette");
    Image::new_indexed(8, 4, palette).expect("image")
}

pub fn near(a: f32, b: f32) -> bool {
    (a - b).abs() <= TOLERANCE
}

pub fn near_rect(a: Rect, b: Rect) -> bool {
    near(a.min.x, b.min.x)
        && near(a.min.y, b.min.y)
        && near(a.max.x, b.max.x)
        && near(a.max.y, b.max.y)
}

/// A painted text: content, corner of the galley in points and color.
#[derive(Clone, Debug)]
pub struct Painted {
    pub text: String,
    pub pos: Pos2,
    pub color: Color32,
}

fn collects(
    shape: &Shape,
    texts: &mut Vec<Painted>,
    fills: &mut Vec<(Rect, Color32)>,
    strokes: &mut Vec<(Rect, Color32, f32)>,
) {
    match shape {
        Shape::Text(t) => texts.push(Painted {
            text: t.galley.text().to_owned(),
            pos: t.pos,
            color: t.fallback_color,
        }),
        Shape::Rect(r) => {
            fills.push((r.rect, r.fill));
            if r.stroke.width > 0.0 {
                strokes.push((r.rect, r.stroke.color, r.stroke.width));
            }
        }
        Shape::Vec(shapes) => {
            for shape in shapes {
                collects(shape, texts, fills, strokes);
            }
        }
        _ => {}
    }
}

/// What a frame painted and returned.
pub struct Frame {
    pub output: PanelOutput,
    pub info: PanelInfo,
    pub texts: Vec<Painted>,
    pub fills: Vec<(Rect, Color32)>,
    pub strokes: Vec<(Rect, Color32, f32)>,
    pub cursor: CursorIcon,
    pub shapes: Vec<ClippedShape>,
}

impl Frame {
    pub fn paints(&self, searched: &str) -> bool {
        self.texts.iter().any(|t| t.text == searched)
    }

    pub fn paints_something_with(&self, searched: &str) -> bool {
        self.texts.iter().any(|t| t.text.contains(searched))
    }

    pub fn text(&self, searched: &str) -> Option<&Painted> {
        self.texts.iter().find(|t| t.text == searched)
    }

    pub fn hint(&self) -> Option<&str> {
        self.info.status_hint.as_deref()
    }

    pub fn there_is_fill(&self, rect: Rect, color: Color32) -> bool {
        self.fills
            .iter()
            .any(|(r, fill)| *fill == color && near_rect(*r, rect))
    }

    pub fn there_is_stroke(&self, color: Color32) -> bool {
        self.strokes.iter().any(|(_, c, _)| *c == color)
    }
}

pub struct Bench {
    pub ctx: Context,
    canvas: Canvas,
    pub scroll: palette_grid::Scroll,
    pub view: PanelView,
    pub screen: Vec2,
    pub document: Document,
    pub image: Image,
    pub foreground: ColorChoice,
    pub background: ColorChoice,
    pub editing: Role,
    pub slots: SlotColors,
    pub editor: ColorEditor,
    pub ops: PaletteOps,
    pub icons: IconsOfTest,
    pub doc_state: u64,
    /// A drawing gesture is in progress (`enabled` false).
    pub gesture_drawing: bool,
    pub history: Vec<PanelOutput>,
}

impl Bench {
    pub fn new(ppp: f32, screen: Vec2, image: Image, foreground: ColorChoice) -> Bench {
        let ctx = Context::default();
        ctx.set_pixels_per_point(ppp);
        ctx.set_missing_glyph_policy(MissingGlyphPolicy::Panic);
        install_fonts(&ctx);
        let icons = IconsOfTest::new_2_2(&ctx);
        let mut bench = Bench {
            ctx,
            canvas: Canvas::new(),
            scroll: palette_grid::Scroll::default(),
            view: PanelView {
                icons: icons.set,
                ..PanelView::default()
            },
            screen,
            document: Document::new(image.clone()),
            slots: SlotColors::defaults(image.mode()),
            background: px_ui::color::default_background(&image),
            image,
            foreground,
            editing: Role::Foreground,
            editor: ColorEditor::default(),
            ops: PaletteOps::default(),
            icons,
            doc_state: 0,
            gesture_drawing: false,
            history: Vec::new(),
        };
        for _ in 0..2 {
            let out = bench.ctx.run_ui(bench.input(vec![]), |ui| {
                apply_scale(ui.ctx());
            });
            out.drop_without_applying_deltas();
        }
        bench.canvas.set_image(&bench.image);
        bench.rest();
        bench.rest();
        bench.history.clear();
        bench
    }

    pub fn with_panel_height(
        ppp: f32,
        panel_h_px: i64,
        image: Image,
        foreground: ColorChoice,
    ) -> Bench {
        let n = text_scale(ppp);
        let screen = vec2(
            800.0,
            (panel_h_px + i64::from(bar_height_px(n))) as f32 / ppp,
        );
        Bench::new(ppp, screen, image, foreground)
    }

    fn input(&self, events: Vec<Event>) -> RawInput {
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, self.screen)),
            events,
            ..RawInput::default()
        }
    }

    pub fn ppp(&self) -> f32 {
        self.ctx.pixels_per_point()
    }

    pub fn n(&self) -> i64 {
        i64::from(text_scale(self.ppp()))
    }

    pub fn panel_h_px(&self) -> i64 {
        (self.screen.y * self.ppp()).round() as i64 - i64::from(bar_height_px(self.n() as u32))
    }

    pub fn layout(&self) -> PanelLayout {
        let (mode, h, n) = (self.image.mode(), self.panel_h_px(), self.n() as u32);
        if self.view.editor_open {
            panel_layout(mode, h, n, self.view.offset_px)
        } else {
            panel_layout_closed(mode, h, n, self.view.offset_px)
        }
    }

    /// Top edge of the tail (bar, field, icon row) from the top of the panel, in physical pixels.
    pub fn tail_top(&self) -> i64 {
        let cells = self.image.palette().map_or(0, |p| p.len() + 1);
        self.layout()
            .tail_top_for(total_rows(cells), self.n() as u32)
    }

    pub fn actual(&self) -> Option<(EditTarget, Rgba)> {
        let choice = match self.editing {
            Role::Foreground => self.foreground,
            Role::Background => self.background,
        };
        let target = EditTarget::of(&self.image, choice)?;
        let color = target.stored(&self.image, &self.slots)?;
        Some((target, color))
    }

    pub fn frame(&mut self, events: Vec<Event>) -> Frame {
        let len = self.image.palette().map_or(0, Palette::len);
        self.ops.sync(self.doc_state, len);
        let actual = self.actual();
        self.editor.sync(actual);
        let enabled = !(self.gesture_drawing || self.editor.dragging() || self.ops.busy());
        let input = self.input(events);
        let roles = Roles {
            foreground: self.foreground,
            background: self.background,
            editing: self.editing,
        };
        let mut result = None;
        let Bench {
            ctx,
            canvas,
            scroll,
            view,
            image,
            slots,
            editor,
            ops,
            ..
        } = self;
        let mut output = ctx.run_ui(input, |ui| {
            apply_scale(ui.ctx());
            px_ui::status::show(ui, "lápiz · color 1 #000000", false);
            let (panel, ()) = panel_on_the_left(
                ui,
                |ui| show_roles(ui, image, roles, scroll, view, enabled, slots, editor, ops),
                |ui| {
                    let _ = canvas.interact_roles(ui);
                    let _ = canvas.paint(ui);
                },
            );
            result = Some(panel);
        });
        output.textures_delta.clear();
        let cursor = output.platform_output.cursor_icon;
        let shapes: Vec<ClippedShape> = std::mem::take(&mut output.shapes);
        let panel = result.expect("the panel is painted");
        let (mut texts, mut fills, mut strokes) = (Vec::new(), Vec::new(), Vec::new());
        for clipped in &shapes {
            collects(&clipped.shape, &mut texts, &mut fills, &mut strokes);
        }
        if let Some(choice) = panel.choice {
            self.foreground = choice;
        }
        self.history.push(panel.clone());
        Frame {
            output: panel,
            info: panel_info(&self.ctx),
            texts,
            fills,
            strokes,
            cursor,
            shapes,
        }
    }

    pub fn rest(&mut self) -> Frame {
        self.frame(vec![])
    }

    // --- Pointer ---

    fn pos(&self, (x, y): (i64, i64)) -> Pos2 {
        pos2(x as f32 / self.ppp(), y as f32 / self.ppp())
    }

    pub fn moves(&mut self, at: (i64, i64)) -> Frame {
        let pos = self.pos(at);
        self.frame(vec![Event::PointerMoved(pos)])
    }

    fn button(&self, at: (i64, i64), pressed: bool) -> Event {
        Event::PointerButton {
            pos: self.pos(at),
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        }
    }

    pub fn presses(&mut self, at: (i64, i64)) -> Frame {
        let event = self.button(at, true);
        self.frame(vec![event])
    }

    pub fn releases(&mut self, at: (i64, i64)) -> Frame {
        let event = self.button(at, false);
        self.frame(vec![event])
    }

    /// Move, press and release; returns the frame of the press.
    pub fn click(&mut self, at: (i64, i64)) -> Frame {
        self.moves(at);
        let pressed = self.presses(at);
        self.releases(at);
        pressed
    }

    pub fn right_click(&mut self, at: (i64, i64)) -> Frame {
        self.moves(at);
        let pos = self.pos(at);
        let event = |pressed| Event::PointerButton {
            pos,
            button: PointerButton::Secondary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        let pressed = self.frame(vec![event(true)]);
        self.frame(vec![event(false)]);
        pressed
    }

    pub fn key(&mut self, key: Key) -> Frame {
        self.frame(vec![Event::Key {
            key,
            physical_key: Some(key),
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }])
    }

    pub fn escape(&mut self) -> Frame {
        self.key(Key::Escape)
    }

    pub fn wheel(&mut self, px_below: i64) -> Frame {
        self.frame(vec![Event::MouseWheel {
            unit: MouseWheelUnit::Point,
            delta: vec2(0.0, -(px_below as f32) / self.ppp()),
            phase: TouchPhase::Move,
            source: MouseWheelSource::Unknown,
            modifiers: Modifiers::NONE,
        }])
    }

    pub fn wants_keyboard(&self) -> bool {
        self.ctx.text_edit_focused()
    }

    // --- Geometry ---

    /// Center of grid cell `index` in physical pixels with the grid's first visible row.
    pub fn cell_center(&self, index: usize) -> (i64, i64) {
        self.cell_center_at(index, self.scroll.first_row)
    }

    pub fn cell_center_at(&self, index: usize, first_row: usize) -> (i64, i64) {
        let n = self.n();
        let columns = 16usize;
        let (col, row) = (
            (index % columns) as i64,
            (index / columns) as i64 - first_row as i64,
        );
        (
            (GRID_X_U + col * PITCH_U) * n + CELL_U * n / 2,
            (GRID_Y_U + row * PITCH_U) * n - self.view.offset_px + CELL_U * n / 2,
        )
    }

    /// The handle's cell is the one after the last entry.
    pub fn handle_center(&self) -> (i64, i64) {
        self.cell_center(self.image.palette().map_or(0, Palette::len))
    }

    /// Rectangle of a `ScreenRect` given from the origin of the grid, in points.
    pub fn rect_of_grid(&self, r: ScreenRect) -> Rect {
        let (n, ppp) = (self.n(), self.ppp());
        let (x, y) = (GRID_X_U * n + r.x, GRID_Y_U * n - self.view.offset_px + r.y);
        Rect::from_min_size(
            pos2(x as f32 / ppp, y as f32 / ppp),
            vec2(r.width as f32 / ppp, r.height as f32 / ppp),
        )
    }

    /// Rectangle in points of grid cell `index` (16 columns).
    pub fn cell_rect(&self, index: usize) -> Rect {
        let n = self.n();
        let (col, row) = (
            (index % 16) as i64,
            (index / 16) as i64 - self.scroll.first_row as i64,
        );
        self.rect_of_grid(ScreenRect {
            x: col * PITCH_U * n,
            y: row * PITCH_U * n,
            width: CELL_U * n,
            height: CELL_U * n,
        })
    }

    /// The ring of `thickness_u` around cell `index` is painted with `color`.
    pub fn there_is_ring(&self, f: &Frame, index: usize, thickness_u: i64, color: Color32) -> bool {
        let n = self.n();
        let cell = ScreenRect {
            x: ((index % 16) as i64 * PITCH_U) * n,
            y: ((index / 16) as i64 - self.scroll.first_row as i64) * PITCH_U * n,
            width: CELL_U * n,
            height: CELL_U * n,
        };
        ring_rects(cell, thickness_u * n)
            .into_iter()
            .all(|part| f.there_is_fill(self.rect_of_grid(part), color))
    }

    /// Cell `index` is painted with the solid `color` (the swatch part, from the corner).
    pub fn cell_is(&self, f: &Frame, index: usize, color: Color32) -> bool {
        f.there_is_fill(self.cell_rect(index), color)
    }

    // --- Document ---

    pub fn applies(&mut self, command: Command) {
        self.document.apply(command).expect("valid command");
        self.image = self
            .document
            .image(ImageId::from_raw(0))
            .expect("the image")
            .clone();
        self.doc_state += 1;
    }

    pub fn ops_asked(&self) -> Vec<px_ui::palette_ops::PaletteOp> {
        self.history.iter().filter_map(|o| o.op).collect()
    }

    pub fn notices(&self) -> Vec<String> {
        self.history
            .iter()
            .filter_map(|o| o.notice.clone())
            .collect()
    }

    pub fn choices(&self) -> Vec<ColorChoice> {
        self.history.iter().filter_map(|o| o.choice).collect()
    }

    pub fn commits(&self) -> Vec<px_ui::color_edit::Commit> {
        self.history.iter().filter_map(|o| o.commit).collect()
    }

    /// Nothing asked: no choice, commit, notice or op in any frame since the last `history.clear()`.
    pub fn history_is_empty(&self) -> bool {
        self.history.iter().all(|o| *o == PanelOutput::default())
    }
}

/// The `Pixel::Indexed` of a cell, for the tests that write pixels in the bench's document.
pub fn pixel(index: u8) -> Pixel {
    Pixel::Indexed(index)
}

/// `IconSet` of a bench (the panel needs one to paint the icon row).
pub fn icons_of(bench: &Bench) -> IconSet {
    bench.icons.set
}
