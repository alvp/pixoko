//! Image operations (specs/hito-4-imagen.md, unit 4.1): flips, turns of 90° and canvas resizing as
//! pure functions over an [`Image`], and [`image_command`], which applies one of them to every
//! cell of a document and returns the `Command::ReplaceImages` for `Document::apply` (same
//! pattern as `merge_down_command`: the core only validates, the pixels are computed here).

use std::error::Error;
use std::fmt;

use px_core::{Command, Document, DocumentError, Image, ImageError, Pixel};

/// Where the old content sits when the canvas changes size.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Anchor {
    TopLeft,
    Top,
    TopRight,
    Left,
    Center,
    Right,
    BottomLeft,
    Bottom,
    BottomRight,
}

/// One of the image operations of the `Imagen` menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageOp {
    FlipH,
    FlipV,
    /// Turn 90° clockwise.
    Cw,
    /// Turn 90° counterclockwise.
    Ccw,
    /// Turn 180°.
    Half,
    Canvas {
        width: u32,
        height: u32,
        anchor: Anchor,
    },
}

/// Why the command could not be built.
#[derive(Debug, PartialEq)]
pub enum ImageOpError {
    Image(ImageError),
    Document(DocumentError),
}

impl fmt::Display for ImageOpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ImageOpError::Image(error) => write!(f, "{error}"),
            ImageOpError::Document(error) => write!(f, "{error}"),
        }
    }
}

impl Error for ImageOpError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            ImageOpError::Image(error) => Some(error),
            ImageOpError::Document(error) => Some(error),
        }
    }
}

impl From<ImageError> for ImageOpError {
    fn from(error: ImageError) -> ImageOpError {
        ImageOpError::Image(error)
    }
}

impl From<DocumentError> for ImageOpError {
    fn from(error: DocumentError) -> ImageOpError {
        ImageOpError::Document(error)
    }
}

/// A zeroed buffer for an image of `width × height`, or the error of an image that cannot exist.
fn buffer(src: &Image, width: u32, height: u32) -> Result<Vec<u8>, ImageError> {
    let len = Image::byte_len(width, height, src.mode())?;
    let mut data = Vec::new();
    data.try_reserve_exact(len)
        .map_err(|_| ImageError::TooLarge { width, height })?;
    data.resize(len, 0);
    Ok(data)
}

fn rebuild(src: &Image, width: u32, height: u32, data: Vec<u8>) -> Result<Image, ImageError> {
    match src.palette() {
        Some(palette) => Image::from_bytes_indexed(width, height, palette.clone(), data),
        None => Image::from_bytes(width, height, src.mode(), data),
    }
}

/// The image `width × height` whose pixel `(x, y)` is the source pixel `from(x, y)`.
fn remap(
    src: &Image,
    width: u32,
    height: u32,
    from: impl Fn(u32, u32) -> (u32, u32),
) -> Result<Image, ImageError> {
    match src.mode().bytes_per_pixel() {
        1 => remap_n::<1>(src, width, height, from),
        2 => remap_n::<2>(src, width, height, from),
        _ => remap_n::<4>(src, width, height, from),
    }
}

/// Side of the tiles of the remap: the reads of a turn jump a whole row between pixels, so the
/// destination is filled by small tiles to keep the source rows in the cache.
const TILE: usize = 32;

/// `remap` for `N` bytes per pixel: with `N` known, each pixel is one load and one store instead
/// of a call to `memcpy` with a variable length.
fn remap_n<const N: usize>(
    src: &Image,
    width: u32,
    height: u32,
    from: impl Fn(u32, u32) -> (u32, u32),
) -> Result<Image, ImageError> {
    let mut data = buffer(src, width, height)?;
    let row = src.row_len();
    let bytes = src.bytes();
    let (w, h) = (width as usize, height as usize);
    for ty in (0..h).step_by(TILE) {
        for tx in (0..w).step_by(TILE) {
            for y in ty..(ty + TILE).min(h) {
                for x in tx..(tx + TILE).min(w) {
                    let (sx, sy) = from(x as u32, y as u32);
                    let at = sy as usize * row + sx as usize * N;
                    let to = (y * w + x) * N;
                    if let (Some(pixel), Some(out)) =
                        (bytes.get(at..at + N), data.get_mut(to..to + N))
                    {
                        out.copy_from_slice(pixel);
                    }
                }
            }
        }
    }
    rebuild(src, width, height, data)
}

/// The pixels of `src` in reverse order, row by row (`per_row`) or as a whole. It copies forward
/// (a plain `memcpy`) and then swaps the pixels of both ends in place: reading backwards pixel by
/// pixel was several times slower at 8192 × 8192.
fn reversed_n<const N: usize>(src: &Image, per_row: bool) -> Result<Image, ImageError> {
    let (width, height) = (src.width(), src.height());
    let mut data = buffer(src, width, height)?;
    data.copy_from_slice(src.bytes());
    let span = if per_row { src.row_len() } else { data.len() };
    for part in data.chunks_exact_mut(span) {
        let half = part.len() / N / 2;
        let (left, right) = part.split_at_mut(half * N);
        // The middle pixel of an odd count is the first of `right` and stays where it is.
        let skip = right.len() - half * N;
        let (left, _) = left.as_chunks_mut::<N>();
        let (right, _) = right[skip..].as_chunks_mut::<N>();
        for (a, b) in left.iter_mut().zip(right.iter_mut().rev()) {
            std::mem::swap(a, b);
        }
    }
    rebuild(src, width, height, data)
}

fn reversed(src: &Image, per_row: bool) -> Result<Image, ImageError> {
    match src.mode().bytes_per_pixel() {
        1 => reversed_n::<1>(src, per_row),
        2 => reversed_n::<2>(src, per_row),
        _ => reversed_n::<4>(src, per_row),
    }
}

pub fn flip_horizontal(image: &Image) -> Result<Image, ImageError> {
    reversed(image, true)
}

pub fn flip_vertical(image: &Image) -> Result<Image, ImageError> {
    let h = image.height();
    remap(image, image.width(), h, |x, y| (x, h - 1 - y))
}

/// 90° clockwise: the source pixel `(x, y)` ends at `(h − 1 − y, x)`.
pub fn rotate_cw(image: &Image) -> Result<Image, ImageError> {
    let h = image.height();
    remap(image, h, image.width(), |x, y| (y, h - 1 - x))
}

/// 90° counterclockwise: the source pixel `(x, y)` ends at `(y, w − 1 − x)`.
pub fn rotate_ccw(image: &Image) -> Result<Image, ImageError> {
    let w = image.width();
    remap(image, image.height(), image.width(), |x, y| (w - 1 - y, x))
}

pub fn rotate_180(image: &Image) -> Result<Image, ImageError> {
    reversed(image, false)
}

/// Offset of the old content along one axis: `0`, `⌊(new − old) / 2⌋` (toward −∞) or `new − old`.
fn offset(old: u32, new: u32, side: i8) -> i64 {
    let diff = i64::from(new) - i64::from(old);
    match side {
        -1 => 0,
        0 => diff.div_euclid(2),
        _ => diff,
    }
}

fn sides(anchor: Anchor) -> (i8, i8) {
    match anchor {
        Anchor::TopLeft => (-1, -1),
        Anchor::Top => (0, -1),
        Anchor::TopRight => (1, -1),
        Anchor::Left => (-1, 0),
        Anchor::Center => (0, 0),
        Anchor::Right => (1, 0),
        Anchor::BottomLeft => (-1, 1),
        Anchor::Bottom => (0, 1),
        Anchor::BottomRight => (1, 1),
    }
}

/// The image resized to `width × height` with the old content placed by `anchor`; what is left
/// uncovered takes `fill`, and what no longer fits is lost. Mode and palette do not change.
pub fn resize_canvas(
    image: &Image,
    width: u32,
    height: u32,
    anchor: Anchor,
    fill: Pixel,
) -> Result<Image, ImageError> {
    let mut data = buffer(image, width, height)?;
    image.check_pixel(fill)?;
    let cell = match (fill, image.palette()) {
        (Pixel::Indexed(index), Some(palette)) => {
            Image::filled_indexed(1, 1, palette.clone(), index)?
        }
        (fill, _) => Image::filled(1, 1, fill)?,
    };
    let bpp = image.mode().bytes_per_pixel();
    for out in data.chunks_exact_mut(bpp) {
        out.copy_from_slice(cell.bytes());
    }
    let (sx, sy) = sides(anchor);
    let dx = offset(image.width(), width, sx);
    let dy = offset(image.height(), height, sy);
    let (old_w, old_h) = (i64::from(image.width()), i64::from(image.height()));
    let x_from = dx.max(0);
    let x_to = (dx + old_w).min(i64::from(width));
    if x_from < x_to {
        let count = (x_to - x_from) as usize * bpp;
        for ny in 0..i64::from(height) {
            let oy = ny - dy;
            if oy < 0 || oy >= old_h {
                continue;
            }
            let src_row = image.row(oy as u32).unwrap_or(&[]);
            let from = (x_from - dx) as usize * bpp;
            let at = (ny as usize * width as usize + x_from as usize) * bpp;
            data[at..at + count].copy_from_slice(&src_row[from..from + count]);
        }
    }
    rebuild(image, width, height, data)
}

fn apply_op(image: &Image, op: ImageOp, fill: Pixel) -> Result<Image, ImageError> {
    match op {
        ImageOp::FlipH => flip_horizontal(image),
        ImageOp::FlipV => flip_vertical(image),
        ImageOp::Cw => rotate_cw(image),
        ImageOp::Ccw => rotate_ccw(image),
        ImageOp::Half => rotate_180(image),
        ImageOp::Canvas {
            width,
            height,
            anchor,
        } => resize_canvas(image, width, height, anchor, fill),
    }
}

/// The command that applies `op` to every cell with an image of `doc` (empty cells stay empty)
/// and, for a turn or a resize, changes the canvas size. `fill` is only used by `Canvas`. The
/// limit of the cells is checked before reserving anything: it fails only if the result is over
/// `cells_limit()` **and** bigger than what the document has now.
pub fn image_command(doc: &Document, op: ImageOp, fill: Pixel) -> Result<Command, ImageOpError> {
    let (width, height) = match op {
        ImageOp::FlipH | ImageOp::FlipV | ImageOp::Half => (doc.width(), doc.height()),
        ImageOp::Cw | ImageOp::Ccw => (doc.height(), doc.width()),
        ImageOp::Canvas { width, height, .. } => (width, height),
    };
    let per_cel = Image::byte_len(width, height, doc.mode())?;
    let cels = doc
        .layers()
        .iter()
        .filter(|layer| doc.image(layer.cel()).is_some())
        .count();
    let needed = per_cel.saturating_mul(cels);
    if needed > doc.cells_limit() && needed > doc.materialized_bytes() {
        return Err(DocumentError::CelsTooLarge {
            needed,
            limit: doc.cells_limit(),
        }
        .into());
    }
    let mut images = Vec::new();
    images
        .try_reserve_exact(doc.layers().len())
        .map_err(|_| DocumentError::OutOfMemory)?;
    for layer in doc.layers() {
        images.push(match doc.image(layer.cel()) {
            Some(image) => Some(apply_op(image, op, fill)?),
            None => None,
        });
    }
    Ok(Command::ReplaceImages {
        width,
        height,
        images,
        based_on: doc.state_id(),
    })
}
