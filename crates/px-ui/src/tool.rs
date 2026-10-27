//! Herramientas (lápiz, línea, relleno, cuentagotas y borrador) como lógica pura
//! (specs/hito-2-linea-relleno-app.md y specs/hito-2-cuentagotas-borrador.md).
//!
//! Sin tipos de egui en la API de [`ToolState`]: quien dibuja traduce los eventos de egui a
//! [`CanvasEvent`] / [`CanvasInput`] y las teclas a [`ToolAction`].

use egui::{Key, Modifiers};
use px_core::{
    Command, Document, DocumentError, Image, ImageId, Mode, Pixel, PixelEdit, Rgba, Span,
};
use px_raster::{
    Brush, BrushShape, Line, PatternTile, Spacing, curve_path, fill_command, fill_pattern_command,
    fill_until_command, line_path, line_path_near, pattern_spans, spaced_centers, stamp_pixels,
};

use crate::actions::{AppAction, Platform};
use crate::brush_es;
use crate::canvas::{CanvasEvent, CanvasInput};
use crate::color::{ColorChoice, SlotColors, choice_label_with, pixel_for_choice};
use crate::dirty::{Change, dirty_of};
use crate::key_events::press_from_key;
use crate::keymap::default_keymap;
use crate::pick::{
    Dropper, EraseError, Picked, eraser_label, eraser_pixel, is_erasable, picked_at,
};
use crate::shape_tool::{Geom, ShapeStyle, drag_geom};
use crate::source::visible_picked;
use crate::stroke::Stroke;
use crate::view::Size;
use crate::{fill_es, shapes_es};

/// `image` es la celda de alguna capa (vacía o no).
fn is_cell(doc: &Document, image: ImageId) -> bool {
    doc.layers().iter().any(|layer| layer.cel() == image)
}

/// La imagen de la que salen el modo y la paleta de la celda `image`: la propia o, si está vacía,
/// la del lienzo; `None` si `image` no es de ninguna celda.
fn reference(doc: &Document, image: ImageId) -> Option<&Image> {
    doc.image(image)
        .or_else(|| is_cell(doc, image).then(|| doc.canvas_image()))
}

/// El valor de una celda vacía en el modo del lienzo: el transparente (`None` en los modos que no
/// lo tienen).
fn empty_value(canvas: &Image) -> Option<Pixel> {
    match canvas.mode() {
        Mode::Rgba => Some(Pixel::Rgba(Rgba::TRANSPARENT)),
        Mode::GrayAlpha => Some(Pixel::GrayAlpha { gray: 0, alpha: 0 }),
        Mode::Indexed => canvas
            .palette()
            .and_then(|palette| palette.transparent_index())
            .map(Pixel::Indexed),
        Mode::Gray => None,
    }
}

/// The checks that every fill of an empty cell shares: `pixel` belongs to the mode and `(x, y)` is
/// inside the canvas. `Ok(None)` outside; inside, the value of an empty cell (the transparent).
fn empty_seed(
    doc: &Document,
    x: i32,
    y: i32,
    pixel: Pixel,
) -> Result<Option<Pixel>, DocumentError> {
    let canvas = doc.canvas_image();
    canvas
        .check_pixel(pixel)
        .map_err(|source| DocumentError::Pixel { pixel: 0, source })?;
    let (Ok(x), Ok(y)) = (u32::try_from(x), u32::try_from(y)) else {
        return Ok(None);
    };
    if x >= doc.width() || y >= doc.height() {
        return Ok(None);
    }
    let seed = empty_value(canvas).ok_or(DocumentError::NoTransparency { mode: doc.mode() })?;
    Ok(Some(seed))
}

/// One span per row over the whole canvas: the region of an empty cell.
fn canvas_spans(doc: &Document) -> Result<Vec<Span>, DocumentError> {
    let height = doc.height();
    let mut spans = Vec::new();
    spans
        .try_reserve_exact(height as usize)
        .map_err(|_| DocumentError::OutOfMemory)?;
    spans.extend((0..height).map(|y| Span {
        y,
        x_start: 0,
        x_end: doc.width(),
    }));
    Ok(spans)
}

/// Whether the target of the fill is an empty cell of the active layer (not materialized yet).
fn is_empty_cell(doc: &Document, image: ImageId) -> bool {
    doc.image(image).is_none() && is_cell(doc, image)
}

/// `fill_command` también sobre una celda vacía: todo el lienzo es una región del valor
/// transparente, así que el relleno es un `FillSpans` de un tramo por fila (sin reservar la celda
/// antes de aplicarlo). El color igual al transparente no cambia nada.
fn fill_in(
    doc: &Document,
    image: ImageId,
    x: i32,
    y: i32,
    pixel: Pixel,
) -> Result<Option<Command>, DocumentError> {
    if !is_empty_cell(doc, image) {
        return fill_command(doc, image, x, y, pixel);
    }
    let Some(seed) = empty_seed(doc, x, y, pixel)? else {
        return Ok(None);
    };
    if seed == pixel {
        return Ok(None);
    }
    Ok(Some(Command::FillSpans {
        image,
        spans: canvas_spans(doc)?,
        pixel,
    }))
}

/// The fill of the active mode (specs/hito-4-relleno.md). `Hasta el fondo` without a color in the
/// other slot is the solid fill. All of them also work on an empty cell without materializing it.
fn fill_by_mode(
    doc: &Document,
    image: ImageId,
    pos: (i32, i32),
    pixel: Pixel,
    mode: FillMode,
    other: Option<Pixel>,
    brush: Brush,
) -> Result<Option<Command>, DocumentError> {
    let (x, y) = pos;
    match (mode, other) {
        (FillMode::UntilBackground, Some(boundary)) => {
            if !is_empty_cell(doc, image) {
                return fill_until_command(doc, image, x, y, pixel, boundary);
            }
            let canvas = doc.canvas_image();
            for (index, value) in [pixel, boundary].into_iter().enumerate() {
                canvas
                    .check_pixel(value)
                    .map_err(|source| DocumentError::Pixel {
                        pixel: index,
                        source,
                    })?;
            }
            let Some(seed) = empty_seed(doc, x, y, pixel)? else {
                return Ok(None);
            };
            if seed == boundary {
                return Ok(None);
            }
            Ok(Some(Command::FillSpans {
                image,
                spans: canvas_spans(doc)?,
                pixel,
            }))
        }
        (FillMode::Pattern, _) => {
            let tile = PatternTile::from_brush(&brush);
            if !is_empty_cell(doc, image) {
                return fill_pattern_command(doc, image, x, y, pixel, &tile);
            }
            let Some(seed) = empty_seed(doc, x, y, pixel)? else {
                return Ok(None);
            };
            if seed == pixel {
                return Ok(None);
            }
            let pieces = pattern_spans(&canvas_spans(doc)?, &tile, Document::MAX_FILL_SPANS)?;
            Ok((!pieces.is_empty()).then_some(Command::FillSpans {
                image,
                spans: pieces,
                pixel,
            }))
        }
        _ => fill_in(doc, image, x, y, pixel),
    }
}

/// Qué lee el cuentagotas.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DropperSource {
    /// Lo que se ve: la composición de las capas visibles.
    #[default]
    Visible,
    /// La celda de la capa activa (vacía: el valor transparente).
    Layer,
}

/// Lo que elige el cuentagotas en `pos` según `source`; `None` fuera del lienzo.
fn pick_at(
    doc: &Document,
    image: ImageId,
    source: DropperSource,
    x: u32,
    y: u32,
) -> Option<Picked> {
    match source {
        DropperSource::Visible => visible_picked(doc, x, y),
        DropperSource::Layer => match doc.image(image) {
            Some(cell) => picked_at(cell, x, y),
            None => {
                let value = empty_value(doc.canvas_image())?;
                (x < doc.width() && y < doc.height()).then_some(())?;
                Some(match value {
                    Pixel::Indexed(index) => Picked::Entry(index),
                    Pixel::Rgba(color) => Picked::Color(color),
                    Pixel::GrayAlpha { gray, alpha } => {
                        Picked::Color(Rgba::new(gray, gray, gray, alpha))
                    }
                    Pixel::Gray(gray) => Picked::Color(Rgba::opaque(gray, gray, gray)),
                })
            }
        },
    }
}

/// Herramienta activa.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tool {
    #[default]
    Pencil,
    Line,
    Fill,
    Dropper,
    Eraser,
    /// Rectangle, circle, ellipse, polygon or freeform (the kind is in `ToolState`).
    Shape,
}

/// The kind of figure that the shapes tool draws (specs/hito-4-formas.md, D1).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ShapeKind {
    #[default]
    Rectangle,
    Circle,
    Ellipse,
    Polygon,
    Freeform,
}

impl ShapeKind {
    /// All the kinds, in the order in which the options window cycles them.
    pub const ALL: [ShapeKind; 5] = [
        ShapeKind::Rectangle,
        ShapeKind::Circle,
        ShapeKind::Ellipse,
        ShapeKind::Polygon,
        ShapeKind::Freeform,
    ];
}

/// What a shape paints: only the outline (with the brush), only the fill or both (the outline
/// with the color of the button, the fill with the other slot's).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ShapeFill {
    #[default]
    Outline,
    Filled,
    Both,
}

impl ShapeFill {
    pub const ALL: [ShapeFill; 3] = [ShapeFill::Outline, ShapeFill::Filled, ShapeFill::Both];
}

/// How the fill tool fills (specs/hito-4-relleno.md, unit 4.5): the solid fill of always, the
/// pattern (the mask of the brush, aligned to the canvas) or the fill that goes on until it
/// touches the background color (the other slot).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum FillMode {
    #[default]
    Solid,
    Pattern,
    UntilBackground,
}

impl FillMode {
    pub const ALL: [FillMode; 3] = [
        FillMode::Solid,
        FillMode::Pattern,
        FillMode::UntilBackground,
    ];
}

impl Tool {
    /// Nombre para el título: `lápiz`, `línea`, `fill`, `dropper` o `eraser`.
    pub fn label(self) -> &'static str {
        match self {
            Tool::Pencil => "lápiz",
            Tool::Line => "línea",
            Tool::Fill => "relleno",
            Tool::Dropper => "cuentagotas",
            Tool::Eraser => "borrador",
            Tool::Shape => "formas",
        }
    }
}

/// Lo que pide una tecla de herramienta.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolAction {
    Select(Tool),
    /// The pencil in dotted mode (`s`).
    SelectDotted,
    /// The line in curve mode (`q`).
    SelectCurve,
    /// The shapes tool with this kind (`r`, `c`, `e`, `w`, `D`); the fill is kept.
    SelectShape(ShapeKind),
    Cancel,
}

/// Vista de la tabla de teclas por defecto: `d`, `v`, `f`, `,` y `k` eligen herramienta y `Escape`
/// cancela, solo sin ningún modificador (así `Ctrl`+`d` y las demás no hacen nada; `K` con `Mayús`
/// es Borrar página, no el Borrador). Es aparte de [`crate::shortcuts::action_for_key`], que da `None` para estas teclas.
pub fn tool_action_for_key(key: Key, modifiers: Modifiers) -> Option<ToolAction> {
    if !modifiers.is_none() {
        return None;
    }
    if key == Key::Escape {
        return Some(ToolAction::Cancel);
    }
    match default_keymap().action_for(&press_from_key(key, modifiers), Platform::Linux)? {
        AppAction::SelectTool(tool) => Some(ToolAction::Select(tool)),
        AppAction::SelectDotted => Some(ToolAction::SelectDotted),
        AppAction::SelectCurve => Some(ToolAction::SelectCurve),
        AppAction::SelectShape(kind) => Some(ToolAction::SelectShape(kind)),
        _ => None,
    }
}

/// Texto del título para herramienta y color: `línea · color 3 #ff0000`; sin color, solo la
/// herramienta.
pub fn status_text(tool: Tool, color: Option<&str>) -> String {
    match color {
        Some(color) => format!("{} · {color}", tool.label()),
        None => tool.label().to_string(),
    }
}

/// Texto que acompaña a la herramienta en el título y la barra de estado: el color activo (con el
/// cuentagotas, el valor que acaba de elegir) o, con el borrador, lo que escribe
/// (`transparent`) o por qué no puede.
pub fn tool_color_label(
    tool: Tool,
    image: &Image,
    choice: ColorChoice,
    slots: &SlotColors,
) -> Option<String> {
    match tool {
        Tool::Eraser => Some(eraser_label(image)),
        Tool::Pencil | Tool::Line | Tool::Fill | Tool::Dropper | Tool::Shape => {
            choice_label_with(image, choice, slots)
        }
    }
}

/// Lo que un gesto pide a `px-app` además de cambiar el documento.
#[derive(Clone, Debug, PartialEq)]
pub enum ToolEvent {
    /// El cuentagotas eligió un color.
    Picked(Picked),
    /// El borrador no puede con la imagen (aviso al pulsar).
    Refused(EraseError),
}

/// Gesto en curso, desde `Begin` hasta `End`.
#[derive(Debug, Default)]
enum Gesture {
    #[default]
    Idle,
    /// Pulsado sin nada pendiente: ranura sin color, relleno ya hecho o línea cancelada. Solo
    /// espera el `End`.
    Waiting,
    /// Freehand with the brush (continuous or dotted).
    Brush {
        stroke: Stroke,
        pixel: Pixel,
        offsets: Vec<(i32, i32)>,
        dotted: bool,
        /// Where the gesture began and the last position stamped (after the `Mayús` projection).
        start: (i32, i32),
        last: (i32, i32),
        axis: Option<Axis>,
    },
    /// Como el lápiz, con el valor de borrado de la imagen.
    Erase {
        stroke: Stroke,
        pixel: Pixel,
    },
    Dropper(Dropper),
    /// A segment or a curve being dragged.
    Figure(Box<Figure>),
    /// A rectangle, circle, ellipse or freeform being dragged.
    Shape(Box<ShapeDrag>),
    /// A corner of the polygon being placed (the candidate is in `PendingPolygon`).
    Corner,
}

/// The axis that `Mayús` locks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Axis {
    Horizontal,
    Vertical,
}

/// `pos` projected on the axis of its larger displacement from `start` (a tie: horizontal).
fn dominant_axis(start: (i32, i32), pos: (i32, i32)) -> Axis {
    let (dx, dy) = (
        i64::from(pos.0) - i64::from(start.0),
        i64::from(pos.1) - i64::from(start.1),
    );
    if dx.abs() >= dy.abs() {
        Axis::Horizontal
    } else {
        Axis::Vertical
    }
}

fn project(start: (i32, i32), pos: (i32, i32), axis: Axis) -> (i32, i32) {
    match axis {
        Axis::Horizontal => (pos.0, start.1),
        Axis::Vertical => (start.0, pos.1),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FigureKind {
    Segment,
    /// First drag of the curve: only a segment to preview.
    CurveFirst,
    /// Second drag: `to` is the control point.
    CurveSecond {
        a: (i32, i32),
        b: (i32, i32),
    },
}

/// The segment or curve being dragged, with the traces `Ctrl` left.
#[derive(Debug)]
struct Figure {
    kind: FigureKind,
    from: (i32, i32),
    to: (i32, i32),
    pixel: Pixel,
    brush: Brush,
    spacing: Spacing,
    traces: Stroke,
    width: u32,
    height: u32,
}

impl Figure {
    /// The path of the segment: whole when the spacing needs its indices, if it is not too long.
    fn segment(&self) -> Vec<(i32, i32)> {
        let whole = Line::new(self.from, self.to).len() <= MAX_WHOLE_SEGMENT;
        if self.spacing == Spacing::Continuous || !whole {
            self.segment_near()
        } else {
            line_path(self.from, self.to)
        }
    }

    /// Only the points that a brush can reach on the canvas.
    fn segment_near(&self) -> Vec<(i32, i32)> {
        line_path_near(
            self.from,
            self.to,
            self.width,
            self.height,
            self.brush.size(),
        )
    }

    /// The pixels of the figure as it is now.
    fn pixels(&self) -> Vec<(u32, u32)> {
        let (path, spacing) = match self.kind {
            FigureKind::Segment => (self.segment(), self.spacing),
            FigureKind::CurveFirst => (self.segment_near(), Spacing::Continuous),
            FigureKind::CurveSecond { a, b } => (curve_path(a, b, self.to), self.spacing),
        };
        let centers = spaced_centers(&path, spacing);
        stamp_pixels(self.brush, centers, self.width, self.height)
    }

    /// The traces plus the figure, as the edits to apply.
    fn into_pixels(mut self) -> Vec<PixelEdit> {
        let pixels = self.pixels();
        let pixel = self.pixel;
        self.traces.add(pixels, pixel);
        self.traces.into_pixels()
    }
}

/// Longest segment, in points, whose spacing is measured from its real start.
const MAX_WHOLE_SEGMENT: u64 = 1 << 22;

/// The shape being dragged.
#[derive(Debug)]
struct ShapeDrag {
    kind: ShapeKind,
    style: ShapeStyle,
    from: (i32, i32),
    to: (i32, i32),
    shift: bool,
    /// The positions of a freeform, without repeats in a row.
    path: Vec<(i32, i32)>,
}

impl ShapeDrag {
    fn geom(&self) -> Geom {
        match self.kind {
            ShapeKind::Freeform => Geom::Poly(self.path.clone()),
            kind => drag_geom(kind, self.from, self.to, self.shift),
        }
    }
}

/// The polygon whose corners are being placed, one click each.
#[derive(Debug)]
struct PendingPolygon {
    style: ShapeStyle,
    vertices: Vec<(i32, i32)>,
    /// The corner being placed (pressed and not yet released).
    candidate: Option<(i32, i32)>,
}

impl PendingPolygon {
    /// The corners as they are now, with the candidate if there is one.
    fn geom(&self) -> Geom {
        let mut vertices = self.vertices.clone();
        if let Some(c) = self.candidate
            && vertices.last() != Some(&c)
        {
            vertices.push(c);
        }
        Geom::Poly(vertices)
    }

    /// `pos` as a corner: with `Mayús`, on the axis of its larger displacement from the last one.
    fn corner(&self, pos: (i32, i32), shift: bool) -> (i32, i32) {
        match self.vertices.last() {
            Some(&last) if shift => project(last, pos, dominant_axis(last, pos)),
            _ => pos,
        }
    }
}

/// The first drag of a curve, waiting for the second.
#[derive(Debug)]
struct PendingCurve {
    a: (i32, i32),
    b: (i32, i32),
    pixel: Pixel,
    brush: Brush,
    width: u32,
    height: u32,
}

/// Herramienta activa y gesto en curso.
#[derive(Debug)]
pub struct ToolState {
    tool: Tool,
    dropper_source: DropperSource,
    gesture: Gesture,
    events: Vec<ToolEvent>,
    changes: Vec<Change>,
    brush: Brush,
    spacing: Spacing,
    /// The `N` of the spacing while it is `Continuous`.
    spacing_n: u32,
    dotted: bool,
    curve: bool,
    shift: bool,
    ctrl: bool,
    pending: Option<PendingCurve>,
    shape_kind: ShapeKind,
    shape_fill: ShapeFill,
    fill_mode: FillMode,
    /// The pixel of the other slot: the fill of a shape with outline and the background color of
    /// the fill "until the background".
    other: Option<Pixel>,
    polygon: Option<PendingPolygon>,
}

impl Default for ToolState {
    fn default() -> ToolState {
        ToolState {
            tool: Tool::default(),
            dropper_source: DropperSource::default(),
            gesture: Gesture::default(),
            events: Vec::new(),
            changes: Vec::new(),
            brush: Brush::new(BrushShape::Round, 1),
            spacing: Spacing::Continuous,
            spacing_n: DEFAULT_SPACING_N,
            dotted: false,
            curve: false,
            shift: false,
            ctrl: false,
            pending: None,
            shape_kind: ShapeKind::default(),
            shape_fill: ShapeFill::default(),
            fill_mode: FillMode::default(),
            other: None,
            polygon: None,
        }
    }
}

/// The element after (or before) `current` in `all`, wrapping around.
fn cycle<T: Copy + PartialEq>(all: &[T], current: T, forward: bool) -> T {
    let at = all.iter().position(|&item| item == current).unwrap_or(0);
    let next = if forward {
        (at + 1) % all.len()
    } else {
        (at + all.len() - 1) % all.len()
    };
    all[next]
}

/// The `N` that the spacing starts with.
const DEFAULT_SPACING_N: u32 = 4;
/// Largest `N` of the spacing.
const MAX_SPACING_N: u32 = 999;

/// A row of the options window of a stroke tool.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OptionRow {
    /// Continuous or dotted (pencil), straight or curve (line).
    Mode,
    /// The kind of shape.
    Kind,
    /// Outline, fill or both.
    Fill,
    /// Solid, pattern or until the background (the fill tool).
    FillMode,
    Shape,
    Size,
    Spacing,
    /// The `N` of the spacing; only when it is not continuous.
    SpacingN,
}

/// The rows of `tool` with the default spacing (continuous): the same that
/// [`ToolState::option_rows`] gives before the spacing changes.
pub fn default_option_rows(tool: Tool) -> &'static [OptionRow] {
    match tool {
        Tool::Pencil => &[OptionRow::Mode, OptionRow::Shape, OptionRow::Size],
        Tool::Line => &[
            OptionRow::Mode,
            OptionRow::Shape,
            OptionRow::Size,
            OptionRow::Spacing,
        ],
        Tool::Shape => &[
            OptionRow::Kind,
            OptionRow::Fill,
            OptionRow::Shape,
            OptionRow::Size,
            OptionRow::Spacing,
        ],
        Tool::Fill => &[OptionRow::FillMode],
        Tool::Dropper | Tool::Eraser => &[],
    }
}

impl ToolState {
    /// Lápiz y sin gesto.
    pub fn new() -> ToolState {
        ToolState::default()
    }

    pub fn tool(&self) -> Tool {
        self.tool
    }

    /// The brush shared by the stroke tools (pencil and line).
    pub fn brush(&self) -> Brush {
        self.brush
    }

    pub fn set_brush(&mut self, brush: Brush) {
        self.brush = brush;
    }

    /// The spacing of the line and the curve.
    pub fn spacing(&self) -> Spacing {
        self.spacing
    }

    pub fn set_spacing(&mut self, spacing: Spacing) {
        self.spacing = spacing;
    }

    /// The pencil is in dotted mode.
    pub fn dotted(&self) -> bool {
        self.dotted
    }

    /// The line is in curve mode.
    pub fn curve(&self) -> bool {
        self.curve
    }

    /// `Mayús` and `Ctrl`, as read before each canvas event. Without calling it they are off.
    pub fn set_modifiers(&mut self, shift: bool, ctrl: bool) {
        self.shift = shift;
        self.ctrl = ctrl;
    }

    /// The kind of figure of the shapes tool.
    pub fn shape_kind(&self) -> ShapeKind {
        self.shape_kind
    }

    /// Outline, fill or both, for the shapes tool.
    pub fn shape_fill(&self) -> ShapeFill {
        self.shape_fill
    }

    pub fn set_shape_fill(&mut self, fill: ShapeFill) {
        self.shape_fill = fill;
    }

    /// The mode of the fill tool.
    pub fn fill_mode(&self) -> FillMode {
        self.fill_mode
    }

    pub fn set_fill_mode(&mut self, mode: FillMode) {
        self.fill_mode = mode;
    }

    /// The pixel of the other slot (the fill of a shape that has an outline, or the background
    /// of the fill "until the background"); `px-app` sets it
    /// before each canvas event. Without it the fill takes the color of the outline.
    pub fn set_other_pixel(&mut self, pixel: Option<Pixel>) {
        self.other = pixel;
    }

    /// Whether the active tool paints with the brush.
    fn is_stroke_tool(&self) -> bool {
        matches!(self.tool, Tool::Pencil | Tool::Line | Tool::Shape)
    }

    /// Changes the size of the brush by `delta`, inside 1 to 100, only with a stroke tool. `true`
    /// if the size changed.
    pub fn step_brush_size(&mut self, delta: i32) -> bool {
        if !self.is_stroke_tool()
            && !(self.tool == Tool::Fill && self.fill_mode == FillMode::Pattern)
        {
            return false;
        }
        let size = (i64::from(self.brush.size()) + i64::from(delta)).clamp(1, 100) as u32;
        let changed = size != self.brush.size();
        self.brush = Brush::new(self.brush.shape(), size);
        changed
    }

    /// The rows of the options window of the active tool.
    pub fn option_rows(&self) -> Vec<OptionRow> {
        let mut rows = default_option_rows(self.tool).to_vec();
        if self.tool == Tool::Shape && self.shape_fill == ShapeFill::Filled {
            // Only the fill: no brush, no spacing.
            rows.truncate(2);
        }
        if self.tool == Tool::Fill && self.fill_mode == FillMode::Pattern {
            // The pattern is the mask of the brush: its shape and size.
            rows.extend([OptionRow::Shape, OptionRow::Size]);
        }
        if matches!(self.tool, Tool::Line | Tool::Shape)
            && rows.contains(&OptionRow::Spacing)
            && self.spacing != Spacing::Continuous
        {
            rows.push(OptionRow::SpacingN);
        }
        rows
    }

    /// The `N` of the spacing: the current one or, if continuous, the last used.
    fn spacing_count(&self) -> u32 {
        match self.spacing {
            Spacing::EveryN(n) | Spacing::TotalN(n) => n,
            Spacing::Continuous => self.spacing_n,
        }
    }

    /// The text of a row.
    pub fn option_text(&self, row: OptionRow) -> String {
        match row {
            OptionRow::Mode => {
                let on = if self.tool == Tool::Line {
                    self.curve
                } else {
                    self.dotted
                };
                brush_es::mode_text(self.tool == Tool::Line, on).to_string()
            }
            OptionRow::Kind => shapes_es::kind_text(self.shape_kind).to_string(),
            OptionRow::Fill => shapes_es::fill_text(self.shape_fill).to_string(),
            OptionRow::FillMode => fill_es::mode_text(self.fill_mode).to_string(),
            OptionRow::Shape => brush_es::shape_text(self.brush.shape()),
            OptionRow::Size => brush_es::size_text(self.brush.size()),
            OptionRow::Spacing => brush_es::spacing_text(self.spacing).to_string(),
            OptionRow::SpacingN => brush_es::spacing_n_text(self.spacing_count()),
        }
    }

    /// A click on a row: `forward` is the left button and `big` is `Mayús` (steps of 10).
    pub fn click_option(&mut self, row: OptionRow, forward: bool, big: bool) {
        let step = |value: u32, max: u32| {
            let delta: i64 = if big { 10 } else { 1 };
            let delta = if forward { delta } else { -delta };
            (i64::from(value) + delta).clamp(1, i64::from(max)) as u32
        };
        match row {
            OptionRow::Mode if self.tool == Tool::Line => {
                self.curve = !self.curve;
                self.pending = None;
            }
            OptionRow::Mode => self.dotted = !self.dotted,
            OptionRow::Kind => {
                self.shape_kind = cycle(&ShapeKind::ALL, self.shape_kind, forward);
                self.polygon = None;
            }
            OptionRow::Fill => self.shape_fill = cycle(&ShapeFill::ALL, self.shape_fill, forward),
            OptionRow::FillMode => self.fill_mode = cycle(&FillMode::ALL, self.fill_mode, forward),
            OptionRow::Shape => {
                let all = BrushShape::ALL;
                let at = all
                    .iter()
                    .position(|&s| s == self.brush.shape())
                    .unwrap_or(0);
                let next = if forward {
                    (at + 1) % all.len()
                } else {
                    (at + all.len() - 1) % all.len()
                };
                self.brush = Brush::new(all[next], self.brush.size());
            }
            OptionRow::Size => {
                self.brush = Brush::new(self.brush.shape(), step(self.brush.size(), 100))
            }
            OptionRow::Spacing => {
                let n = self.spacing_count();
                self.spacing_n = n;
                self.spacing = match (self.spacing, forward) {
                    (Spacing::Continuous, true) | (Spacing::TotalN(_), false) => Spacing::EveryN(n),
                    (Spacing::EveryN(_), true) | (Spacing::Continuous, false) => Spacing::TotalN(n),
                    (Spacing::TotalN(_), true) | (Spacing::EveryN(_), false) => Spacing::Continuous,
                };
            }
            OptionRow::SpacingN => {
                let n = step(self.spacing_count(), MAX_SPACING_N);
                self.spacing_n = n;
                self.spacing = match self.spacing {
                    Spacing::Continuous => Spacing::Continuous,
                    Spacing::EveryN(_) => Spacing::EveryN(n),
                    Spacing::TotalN(_) => Spacing::TotalN(n),
                };
            }
        }
    }

    /// Qué lee el cuentagotas (por defecto, lo visible). Sin gesto ni opción hasta el panel de
    /// capas (3.4).
    pub fn dropper_source(&self) -> DropperSource {
        self.dropper_source
    }

    pub fn set_dropper_source(&mut self, source: DropperSource) {
        self.dropper_source = source;
    }

    /// Cierto entre `Begin` y `End`, ambos incluidos, haya o no gesto real.
    pub fn busy(&self) -> bool {
        !matches!(self.gesture, Gesture::Idle)
    }

    /// Vacía y devuelve lo que los gestos piden a `px-app` (elecciones del cuentagotas y avisos del
    /// borrador), en orden.
    pub fn take_events(&mut self) -> Vec<ToolEvent> {
        std::mem::take(&mut self.events)
    }

    /// Vacía y devuelve los cambios aplicados al documento desde la última vez, en orden: el
    /// `state_id` que dejó cada `apply` y los bloques que tocó (para el registro de la sesión y la
    /// región sucia del lienzo).
    pub fn take_changes(&mut self) -> Vec<Change> {
        std::mem::take(&mut self.changes)
    }

    /// Aplica `command` y, si cambió algo, anota su región.
    fn apply(&mut self, doc: &mut Document, command: Command) -> Result<bool, DocumentError> {
        let image = match &command {
            Command::SetPixels { image, .. } | Command::FillSpans { image, .. } => Some(*image),
            // A shape with outline and fill: both parts are on the same cell.
            Command::Batch(commands) => commands.iter().find_map(|c| match c {
                Command::SetPixels { image, .. } | Command::FillSpans { image, .. } => Some(*image),
                _ => None,
            }),
            _ => None,
        };
        let size = image
            .filter(|&id| is_cell(doc, id))
            .map(|_| Size::new(doc.width(), doc.height()));
        let blocks = size.map(|size| dirty_of(&command, size));
        let changed = doc.apply(command)?;
        if changed && let Some(blocks) = blocks {
            self.changes.push(Change {
                state: doc.state_id(),
                blocks,
            });
        }
        Ok(changed)
    }

    /// `Select` (y sus dos modos) cambia de herramienta solo en reposo. `Cancel` descarta la
    /// figura que se arrastra (el gesto sigue `busy` hasta su `End`) y el segmento pendiente de
    /// la curva; con lápiz, relleno o en reposo no hace nada más.
    pub fn apply_action(&mut self, action: ToolAction) {
        match action {
            ToolAction::Select(tool) if !self.busy() => {
                self.tool = tool;
                self.dotted = false;
                self.curve = false;
                self.pending = None;
                self.polygon = None;
            }
            ToolAction::SelectShape(kind) if !self.busy() => {
                self.tool = Tool::Shape;
                self.shape_kind = kind;
                self.dotted = false;
                self.curve = false;
                self.pending = None;
                self.polygon = None;
            }
            ToolAction::SelectDotted if !self.busy() => {
                self.tool = Tool::Pencil;
                self.dotted = true;
                self.curve = false;
                self.pending = None;
            }
            ToolAction::SelectCurve if !self.busy() => {
                self.tool = Tool::Line;
                self.dotted = false;
                self.curve = true;
                self.pending = None;
            }
            ToolAction::Cancel => {
                self.pending = None;
                self.polygon = None;
                if matches!(
                    self.gesture,
                    Gesture::Figure(_) | Gesture::Shape(_) | Gesture::Corner
                ) {
                    self.gesture = Gesture::Waiting;
                }
            }
            _ => {}
        }
    }

    /// `Ok(true)`: el documento cambió (hay que regenerar la textura). `Ok(false)`: nada que
    /// hacer. Un `Err` deja el documento intacto y el gesto terminado o en espera de su `End`.
    pub fn canvas_event(
        &mut self,
        doc: &mut Document,
        image: ImageId,
        choice: impl Into<ColorChoice>,
        event: CanvasEvent,
    ) -> Result<bool, DocumentError> {
        let pixel =
            reference(doc, image).and_then(|target| pixel_for_choice(target, choice.into()));
        self.canvas_event_with(doc, image, pixel, event)
    }

    /// Como [`ToolState::canvas_event`], con el `Pixel` ya calculado (`None`: no hay color
    /// para esa elección, y un trazo que empieza no hace nada).
    pub fn canvas_event_with(
        &mut self,
        doc: &mut Document,
        image: ImageId,
        pixel: Option<Pixel>,
        event: CanvasEvent,
    ) -> Result<bool, DocumentError> {
        match event {
            CanvasEvent::StrokeBegin(pos) if !self.busy() => self.begin(doc, image, pixel, pos),
            CanvasEvent::StrokeBegin(_) => Ok(false),
            CanvasEvent::StrokeMove(pos) => {
                let (shift, ctrl) = (self.shift, self.ctrl);
                let brush_size = self.brush.size();
                match &mut self.gesture {
                    Gesture::Erase { stroke, pixel } => {
                        stroke.move_to(pos, *pixel);
                    }
                    Gesture::Brush {
                        stroke,
                        pixel,
                        offsets,
                        dotted,
                        start,
                        last,
                        axis,
                    } => {
                        let pos = if shift {
                            let locked = *axis.get_or_insert_with(|| dominant_axis(*start, pos));
                            if pos == *start && axis.is_some() {
                                pos
                            } else {
                                project(*start, pos, locked)
                            }
                        } else {
                            *axis = None;
                            pos
                        };
                        if pos != *last {
                            if *dotted {
                                stroke.stamp(offsets, pos, *pixel);
                            } else {
                                let reach = brush_size;
                                let (w, h) = stroke.size();
                                for center in line_path_near(*last, pos, w, h, reach) {
                                    stroke.stamp(offsets, center, *pixel);
                                }
                            }
                            *last = pos;
                        }
                    }
                    Gesture::Figure(figure) => {
                        figure.to = match figure.kind {
                            FigureKind::CurveSecond { .. } => pos,
                            _ if shift => {
                                project(figure.from, pos, dominant_axis(figure.from, pos))
                            }
                            _ => pos,
                        };
                        if ctrl && figure.kind != FigureKind::CurveFirst {
                            let pixels = figure.pixels();
                            let pixel = figure.pixel;
                            figure.traces.add(pixels, pixel);
                        }
                    }
                    Gesture::Shape(drag) => {
                        drag.shift = shift;
                        drag.to = pos;
                        if drag.path.last() != Some(&pos) {
                            drag.path.push(pos);
                        }
                    }
                    Gesture::Corner => {
                        if let Some(polygon) = &mut self.polygon {
                            polygon.candidate = Some(polygon.corner(pos, shift));
                        }
                    }
                    Gesture::Dropper(dropper) => {
                        let source = self.dropper_source;
                        let picked =
                            dropper.visit_with((doc.width(), doc.height()), pos, |x, y| {
                                pick_at(doc, image, source, x, y)
                            });
                        self.events.extend(picked.map(ToolEvent::Picked));
                    }
                    _ => {}
                }
                Ok(false)
            }
            CanvasEvent::StrokeEnd => match std::mem::take(&mut self.gesture) {
                Gesture::Brush { stroke, .. } => {
                    if stroke.is_empty() {
                        return Ok(false);
                    }
                    let pixels = stroke.into_pixels();
                    self.apply(doc, Command::SetPixels { image, pixels })
                }
                Gesture::Erase { stroke, .. } => {
                    let Some(target) = doc.image(image) else {
                        return Ok(false);
                    };
                    let pixels: Vec<PixelEdit> = stroke
                        .pixels()
                        .iter()
                        .copied()
                        .filter(|edit| is_erasable(target, edit.x, edit.y))
                        .collect();
                    if pixels.is_empty() {
                        return Ok(false);
                    }
                    self.apply(doc, Command::SetPixels { image, pixels })
                }
                Gesture::Figure(figure) => match figure.kind {
                    FigureKind::CurveFirst => {
                        if figure.from != figure.to {
                            self.pending = Some(PendingCurve {
                                a: figure.from,
                                b: figure.to,
                                pixel: figure.pixel,
                                brush: figure.brush,
                                width: figure.width,
                                height: figure.height,
                            });
                        }
                        Ok(false)
                    }
                    FigureKind::Segment | FigureKind::CurveSecond { .. } => {
                        let pixels = figure.into_pixels();
                        if pixels.is_empty() {
                            return Ok(false);
                        }
                        self.apply(doc, Command::SetPixels { image, pixels })
                    }
                },
                Gesture::Shape(drag) => {
                    let command = drag.style.command(image, &drag.geom());
                    match command {
                        Some(command) => self.apply(doc, command),
                        None => Ok(false),
                    }
                }
                Gesture::Corner => self.place_corner(doc, image),
                Gesture::Idle | Gesture::Waiting | Gesture::Dropper(_) => Ok(false),
            },
        }
    }

    /// Como [`ToolState::canvas_event`], para la entrada ordenada de
    /// `Canvas::interact_roles`: `Escape` equivale a `Cancel` (y da `Ok(false)`).
    pub fn canvas_input(
        &mut self,
        doc: &mut Document,
        image: ImageId,
        choice: impl Into<ColorChoice>,
        input: CanvasInput,
    ) -> Result<bool, DocumentError> {
        match input {
            CanvasInput::Stroke(event) => self.canvas_event(doc, image, choice, event),
            CanvasInput::Escape => {
                self.apply_action(ToolAction::Cancel);
                Ok(false)
            }
            CanvasInput::Confirm => self.close_polygon(doc, image),
        }
    }

    /// Como [`ToolState::canvas_input`], con el `Pixel` ya calculado.
    pub fn canvas_input_with(
        &mut self,
        doc: &mut Document,
        image: ImageId,
        pixel: Option<Pixel>,
        input: CanvasInput,
    ) -> Result<bool, DocumentError> {
        match input {
            CanvasInput::Stroke(event) => self.canvas_event_with(doc, image, pixel, event),
            CanvasInput::Escape => {
                self.apply_action(ToolAction::Cancel);
                Ok(false)
            }
            CanvasInput::Confirm => self.close_polygon(doc, image),
        }
    }

    fn begin(
        &mut self,
        doc: &mut Document,
        image: ImageId,
        pixel: Option<Pixel>,
        pos: (i32, i32),
    ) -> Result<bool, DocumentError> {
        self.gesture = Gesture::Waiting;
        let Some(target) = reference(doc, image) else {
            return Ok(false);
        };
        let (width, height) = (doc.width(), doc.height());
        match self.tool {
            Tool::Dropper => {
                let mut dropper = Dropper::new();
                let source = self.dropper_source;
                let picked = dropper.visit_with((width, height), pos, |x, y| {
                    pick_at(doc, image, source, x, y)
                });
                self.events.extend(picked.map(ToolEvent::Picked));
                self.gesture = Gesture::Dropper(dropper);
                return Ok(false);
            }
            Tool::Eraser => {
                match eraser_pixel(target) {
                    Ok(pixel) => {
                        let mut stroke = Stroke::new(width, height);
                        stroke.move_to(pos, pixel);
                        self.gesture = Gesture::Erase { stroke, pixel };
                    }
                    Err(error) => self.events.push(ToolEvent::Refused(error)),
                }
                return Ok(false);
            }
            Tool::Pencil | Tool::Line | Tool::Fill | Tool::Shape => {}
        }
        let Some(pixel) = pixel else {
            return Ok(false);
        };
        match self.tool {
            Tool::Pencil => {
                let offsets = self.brush.offsets();
                let mut stroke = Stroke::new(width, height);
                stroke.stamp(&offsets, pos, pixel);
                self.gesture = Gesture::Brush {
                    stroke,
                    pixel,
                    offsets,
                    dotted: self.dotted,
                    start: pos,
                    last: pos,
                    axis: None,
                };
                Ok(false)
            }
            Tool::Line => {
                let kind = match self.pending.take() {
                    Some(PendingCurve { a, b, .. }) if self.curve => {
                        FigureKind::CurveSecond { a, b }
                    }
                    _ if self.curve => FigureKind::CurveFirst,
                    _ => FigureKind::Segment,
                };
                self.gesture = Gesture::Figure(Box::new(Figure {
                    kind,
                    from: pos,
                    to: pos,
                    pixel,
                    brush: self.brush,
                    spacing: self.spacing,
                    traces: Stroke::new(width, height),
                    width,
                    height,
                }));
                Ok(false)
            }
            Tool::Fill => match fill_by_mode(
                doc,
                image,
                pos,
                pixel,
                self.fill_mode,
                self.other,
                self.brush,
            )? {
                Some(command) => self.apply(doc, command),
                None => Ok(false),
            },
            Tool::Shape => {
                let style = ShapeStyle {
                    pixel,
                    other: self.other.unwrap_or(pixel),
                    fill: self.shape_fill,
                    brush: self.brush,
                    spacing: self.spacing,
                    width,
                    height,
                };
                if self.shape_kind == ShapeKind::Polygon {
                    // Pending corners of another image do not apply to this one.
                    if self
                        .polygon
                        .as_ref()
                        .is_some_and(|p| (p.style.width, p.style.height) != (width, height))
                    {
                        self.polygon = None;
                    }
                    let polygon = self.polygon.get_or_insert(PendingPolygon {
                        style,
                        vertices: Vec::new(),
                        candidate: None,
                    });
                    polygon.candidate = Some(polygon.corner(pos, self.shift));
                    self.gesture = Gesture::Corner;
                } else {
                    self.gesture = Gesture::Shape(Box::new(ShapeDrag {
                        kind: self.shape_kind,
                        style,
                        from: pos,
                        to: pos,
                        shift: self.shift,
                        path: vec![pos],
                    }));
                }
                Ok(false)
            }
            Tool::Dropper | Tool::Eraser => Ok(false),
        }
    }

    /// Release of a polygon corner: a press on the first corner (with three or more) closes the
    /// polygon; otherwise the corner is added.
    fn place_corner(&mut self, doc: &mut Document, image: ImageId) -> Result<bool, DocumentError> {
        let Some(polygon) = &mut self.polygon else {
            return Ok(false);
        };
        let Some(corner) = polygon.candidate.take() else {
            return Ok(false);
        };
        if polygon.vertices.len() >= 3 && polygon.vertices.first() == Some(&corner) {
            return self.close_polygon(doc, image);
        }
        if polygon.vertices.last() != Some(&corner) {
            polygon.vertices.push(corner);
        }
        Ok(false)
    }

    /// Applies the pending polygon (`Intro`, or a press on its first corner): it needs two corners
    /// at least (two are a line). Without a pending polygon, or while a corner is being placed, it
    /// does nothing.
    fn close_polygon(&mut self, doc: &mut Document, image: ImageId) -> Result<bool, DocumentError> {
        if self.busy() {
            return Ok(false);
        }
        let Some(polygon) = &self.polygon else {
            return Ok(false);
        };
        if polygon.vertices.len() < 2 {
            return Ok(false);
        }
        let command = polygon.style.command(image, &polygon.geom());
        self.polygon = None;
        match command {
            Some(command) => self.apply(doc, command),
            None => Ok(false),
        }
    }

    /// Píxeles pendientes para la superposición: el trazo del pincel o del borrador, la figura
    /// que se arrastra (con sus trazas) o el segmento pendiente de la curva. Vacío en reposo y con
    /// el relleno.
    pub fn preview(&self) -> impl Iterator<Item = PixelEdit> + '_ {
        let pixels: Box<dyn Iterator<Item = PixelEdit> + '_> = match &self.gesture {
            Gesture::Brush { stroke, .. } | Gesture::Erase { stroke, .. } => {
                Box::new(stroke.pixels().iter().copied())
            }
            Gesture::Shape(drag) => Box::new(drag.style.preview(&drag.geom()).into_iter()),
            Gesture::Figure(figure) => {
                let pixel = figure.pixel;
                let figure_pixels = figure.pixels();
                Box::new(
                    figure.traces.pixels().iter().copied().chain(
                        figure_pixels
                            .into_iter()
                            .map(move |(x, y)| PixelEdit { x, y, pixel }),
                    ),
                )
            }
            _ if self.polygon.is_some() => {
                let polygon = self.polygon.iter().next().into_iter();
                Box::new(polygon.flat_map(|p| p.style.preview(&p.geom()).into_iter()))
            }
            _ => match &self.pending {
                Some(p) => {
                    let pixel = p.pixel;
                    let centers = line_path_near(p.a, p.b, p.width, p.height, p.brush.size());
                    let pixels = stamp_pixels(p.brush, centers, p.width, p.height);
                    Box::new(
                        pixels
                            .into_iter()
                            .map(move |(x, y)| PixelEdit { x, y, pixel }),
                    )
                }
                None => Box::new(std::iter::empty()),
            },
        };
        pixels
    }
}
