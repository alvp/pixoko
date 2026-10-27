//! The drawing of a shape (specs/hito-4-formas.md, unit 4.4): what a rectangle, circle, ellipse,
//! polygon or freeform becomes as pixels and as a command. Pure: `ToolState` holds the gestures and
//! asks this module for the pixels (preview) and the command (release).

use px_core::{Command, ImageId, Pixel, PixelEdit, Span};
use px_raster::{
    Brush, Spacing, circle_radius, ellipse_ring, ellipse_spans, polygon_ring, polygon_spans,
    rect_ring, rect_spans, spaced_centers_closed, stamp_pixels,
};

use crate::tool::{ShapeFill, ShapeKind};

/// What a shape is made of, frozen when the gesture begins.
#[derive(Clone, Debug)]
pub(crate) struct ShapeStyle {
    /// The color of the outline (or of the fill, when there is no outline).
    pub pixel: Pixel,
    /// The color of the fill when there is an outline too.
    pub other: Pixel,
    pub fill: ShapeFill,
    pub brush: Brush,
    pub spacing: Spacing,
    pub width: u32,
    pub height: u32,
}

/// The figure in canvas coordinates.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Geom {
    Rect((i32, i32), (i32, i32)),
    Ellipse((i32, i32), u32, u32),
    Poly(Vec<(i32, i32)>),
}

/// The figure that a drag from `from` to `to` draws for `kind` (`Polygon` and `Freeform` are made of
/// their vertices, not of a drag).
pub(crate) fn drag_geom(kind: ShapeKind, from: (i32, i32), to: (i32, i32), shift: bool) -> Geom {
    let (dx, dy) = (
        i64::from(to.0) - i64::from(from.0),
        i64::from(to.1) - i64::from(from.1),
    );
    match kind {
        ShapeKind::Rectangle if shift => {
            let side = dx.abs().max(dy.abs());
            let corner = |origin: i32, delta: i64| {
                (i64::from(origin) + if delta < 0 { -side } else { side })
                    .clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
            };
            Geom::Rect(from, (corner(from.0, dx), corner(from.1, dy)))
        }
        ShapeKind::Rectangle => Geom::Rect(from, to),
        ShapeKind::Circle => {
            let r = circle_radius(dx, dy);
            Geom::Ellipse(from, r, r)
        }
        ShapeKind::Ellipse if shift => {
            let r = dx
                .unsigned_abs()
                .max(dy.unsigned_abs())
                .min(u64::from(u32::MAX)) as u32;
            Geom::Ellipse(from, r, r)
        }
        ShapeKind::Ellipse => Geom::Ellipse(
            from,
            dx.unsigned_abs().min(u64::from(u32::MAX)) as u32,
            dy.unsigned_abs().min(u64::from(u32::MAX)) as u32,
        ),
        ShapeKind::Polygon | ShapeKind::Freeform => Geom::Poly(vec![from, to]),
    }
}

impl ShapeStyle {
    fn spans(&self, geom: &Geom) -> Vec<Span> {
        let (w, h) = (self.width, self.height);
        match geom {
            Geom::Rect(a, b) => rect_spans(*a, *b, w, h),
            Geom::Ellipse(c, a, b) => ellipse_spans(*c, *a, *b, w, h),
            Geom::Poly(v) => polygon_spans(v, w, h),
        }
    }

    fn outline(&self, geom: &Geom) -> Vec<(u32, u32)> {
        let (w, h) = (self.width, self.height);
        let margin = self.brush.size();
        let ring = match geom {
            Geom::Rect(a, b) => rect_ring(*a, *b, w, h, margin),
            Geom::Ellipse(c, a, b) => ellipse_ring(*c, *a, *b, w, h, margin),
            Geom::Poly(v) => polygon_ring(v, w, h, margin),
        };
        let centers = spaced_centers_closed(&ring, self.spacing);
        stamp_pixels(self.brush, centers, w, h)
    }

    fn fill_pixel(&self) -> Pixel {
        match self.fill {
            ShapeFill::Both => self.other,
            _ => self.pixel,
        }
    }

    /// The fill (spans) and the outline (pixels) of `geom`, empty where the fill mode has none.
    fn parts(&self, geom: &Geom) -> (Vec<Span>, Vec<(u32, u32)>) {
        let spans = if self.fill == ShapeFill::Outline {
            Vec::new()
        } else {
            self.spans(geom)
        };
        let outline = if self.fill == ShapeFill::Filled {
            Vec::new()
        } else {
            self.outline(geom)
        };
        (spans, outline)
    }

    /// The pixels as the overlay shows them: the fill first and the outline over it, no pixel twice.
    pub fn preview(&self, geom: &Geom) -> Vec<PixelEdit> {
        let (spans, outline) = self.parts(geom);
        let on_top: std::collections::HashSet<(u32, u32)> = outline.iter().copied().collect();
        let fill = self.fill_pixel();
        let mut out: Vec<PixelEdit> = spans
            .iter()
            .flat_map(|s| (s.x_start..s.x_end).map(move |x| (x, s.y)))
            .filter(|p| !on_top.contains(p))
            .map(|(x, y)| PixelEdit { x, y, pixel: fill })
            .collect();
        out.extend(outline.into_iter().map(|(x, y)| PixelEdit {
            x,
            y,
            pixel: self.pixel,
        }));
        out
    }

    /// The command that applies `geom` to `image`: a `FillSpans` for the fill, a `SetPixels` for
    /// the outline, both in a `Batch` (the fill first); `None` if nothing is left on the canvas.
    pub fn command(&self, image: ImageId, geom: &Geom) -> Option<Command> {
        let (spans, outline) = self.parts(geom);
        let mut commands = Vec::new();
        if !spans.is_empty() {
            commands.push(Command::FillSpans {
                image,
                spans,
                pixel: self.fill_pixel(),
            });
        }
        if !outline.is_empty() {
            let pixel = self.pixel;
            commands.push(Command::SetPixels {
                image,
                pixels: outline
                    .into_iter()
                    .map(|(x, y)| PixelEdit { x, y, pixel })
                    .collect(),
            });
        }
        match commands.len() {
            0 => None,
            1 => commands.pop(),
            _ => Some(Command::Batch(commands)),
        }
    }
}
