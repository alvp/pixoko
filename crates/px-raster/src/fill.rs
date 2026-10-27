//! Flood fill as a `Document` command (specs/hito-2-relleno.md), by rows since unit 4.5
//! (specs/hito-4-relleno.md): the bytes of the pixels are compared over the rows of the image
//! instead of building a `Pixel` per cell. Besides the solid fill, the "until the background
//! color" region and the pattern aligned to the canvas.
//!
//! Fill by spans with an explicit stack: each span (maximal series of pixels of the region in a
//! row) is marked in a visited map when it is discovered and pushed once. A final pass over the
//! map returns the spans ordered by `(y, x_start)`, without overlaps and merged, as
//! `Command::FillSpans` requires.

use px_core::{Command, Document, DocumentError, Image, ImageId, Pixel, Span};

use crate::Brush;

/// `FillSpans` with the region of contiguous pixels (4 neighbours) that hold the same value as the
/// seed `(x, y)`, compared by stored value; `None` if there is nothing to change.
///
/// Order of checks: unknown image, `pixel` that is not of the mode of the image or has an index
/// outside the palette (`Image::check_pixel`), seed outside the canvas (`None`) and `pixel` equal
/// to the value of the seed (`None`). It only reads `doc`; it reserves with `try_reserve*`, so
/// running out of memory is `Err(OutOfMemory)` and never aborts. A region with more than
/// `Document::MAX_FILL_SPANS` spans is `Err(FillTooFragmented)` (specs/hito-2-limites-relleno.md).
pub fn fill_command(
    doc: &Document,
    image: ImageId,
    x: i32,
    y: i32,
    pixel: Pixel,
) -> Result<Option<Command>, DocumentError> {
    fill_command_with_limit(doc, image, x, y, pixel, Document::MAX_FILL_SPANS)
}

/// Like [`fill_command`], with the cap of spans as a parameter: on discovering span number
/// `max_spans + 1` it returns `Err(FillTooFragmented { limit: max_spans })` before reserving the
/// output; with exactly `max_spans` spans, `Ok`. The cap is the last check.
pub fn fill_command_with_limit(
    doc: &Document,
    image: ImageId,
    x: i32,
    y: i32,
    pixel: Pixel,
    max_spans: usize,
) -> Result<Option<Command>, DocumentError> {
    let target = doc.image(image).ok_or(DocumentError::UnknownImage(image))?;
    target
        .check_pixel(pixel)
        .map_err(|source| DocumentError::Pixel { pixel: 0, source })?;
    let (Ok(x), Ok(y)) = (u32::try_from(x), u32::try_from(y)) else {
        return Ok(None);
    };
    // Inside the canvas `get_pixel` does not fail; outside, there is nothing to fill.
    let Ok(seed) = target.get_pixel(x, y) else {
        return Ok(None);
    };
    if seed == pixel {
        return Ok(None);
    }
    let spans = region_spans(target, x, y, &Rule::same(seed), max_spans)?;
    Ok(Some(Command::FillSpans {
        image,
        spans,
        pixel,
    }))
}

/// Like [`fill_command`], but the region is that of the pixels connected (4 neighbours) to the seed
/// that do **not** hold `boundary` (the background color): "fill until the background color". If
/// the seed holds `boundary` there is nothing to fill (`None`); `pixel` may equal `boundary`. The
/// 14c caps apply the same.
pub fn fill_until_command(
    doc: &Document,
    image: ImageId,
    x: i32,
    y: i32,
    pixel: Pixel,
    boundary: Pixel,
) -> Result<Option<Command>, DocumentError> {
    fill_until_command_with_limit(doc, image, x, y, pixel, boundary, Document::MAX_FILL_SPANS)
}

/// Like [`fill_until_command`], with the cap of spans as a parameter (see
/// [`fill_command_with_limit`]). Order of checks: unknown image, `pixel` and `boundary` that are
/// not of the mode of the image (`Pixel { pixel: 0 | 1, .. }`), seed outside (`None`), seed equal
/// to the background (`None`) and, last, the cap.
pub fn fill_until_command_with_limit(
    doc: &Document,
    image: ImageId,
    x: i32,
    y: i32,
    pixel: Pixel,
    boundary: Pixel,
    max_spans: usize,
) -> Result<Option<Command>, DocumentError> {
    let target = doc.image(image).ok_or(DocumentError::UnknownImage(image))?;
    for (index, value) in [pixel, boundary].into_iter().enumerate() {
        target
            .check_pixel(value)
            .map_err(|source| DocumentError::Pixel {
                pixel: index,
                source,
            })?;
    }
    let (Ok(x), Ok(y)) = (u32::try_from(x), u32::try_from(y)) else {
        return Ok(None);
    };
    let Ok(seed) = target.get_pixel(x, y) else {
        return Ok(None);
    };
    if seed == boundary {
        return Ok(None);
    }
    let spans = region_spans(target, x, y, &Rule::not(boundary), max_spans)?;
    Ok(Some(Command::FillSpans {
        image,
        spans,
        pixel,
    }))
}

/// The tile of a pattern: the mask of the brush in its `n × n` box, repeated over the whole canvas
/// and **aligned to its origin** (the pixel `(x, y)` is painted if the cell `(x mod n, y mod n)`
/// is), not to the seed or to the region.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PatternTile {
    size: u32,
    /// `size × size` cells by rows.
    cells: Vec<bool>,
}

impl PatternTile {
    /// The mask of `brush.offsets()` with the center at `(⌊n/2⌋, ⌊n/2⌋)` of the box.
    pub fn from_brush(brush: &Brush) -> PatternTile {
        let size = brush.size();
        let n = size as usize;
        let center = i64::from(size / 2);
        let mut cells = vec![false; n * n];
        for (dx, dy) in brush.offsets() {
            let (tx, ty) = (i64::from(dx) + center, i64::from(dy) + center);
            if let (Ok(tx), Ok(ty)) = (usize::try_from(tx), usize::try_from(ty))
                && tx < n
                && ty < n
                && let Some(cell) = cells.get_mut(ty * n + tx)
            {
                *cell = true;
            }
        }
        PatternTile { size, cells }
    }

    /// Whether the pixel `(x, y)` of the canvas is painted by the pattern.
    pub fn contains(&self, x: u32, y: u32) -> bool {
        let n = self.size as usize;
        let at = (y % self.size) as usize * n + (x % self.size) as usize;
        self.cells.get(at).copied().unwrap_or(false)
    }

    /// Row `y mod n` of the tile.
    fn row(&self, y: u32) -> &[bool] {
        let n = self.size as usize;
        let at = (y % self.size) as usize * n;
        self.cells.get(at..at + n).unwrap_or(&[])
    }
}

/// The pieces of `region` (ordered spans without overlaps) that the pattern paints: ordered,
/// without overlaps, without empty spans and inside the region. If there are more than
/// `max_spans`, `Err(FillTooFragmented { limit: max_spans })` without having reserved anything
/// extra (the output never exceeds `max_spans` spans).
pub fn pattern_spans(
    region: &[Span],
    tile: &PatternTile,
    max_spans: usize,
) -> Result<Vec<Span>, DocumentError> {
    let n = tile.size;
    let mut out: Vec<Span> = Vec::new();
    for span in region {
        let row = tile.row(span.y);
        if row.iter().all(|&on| !on) {
            continue;
        }
        let full = row.iter().all(|&on| on);
        let mut open: Option<u32> = None;
        let mut push = |x_start: u32, x_end: u32| {
            if out.len() >= max_spans {
                return Err(DocumentError::FillTooFragmented { limit: max_spans });
            }
            out.try_reserve(1).map_err(|_| DocumentError::OutOfMemory)?;
            out.push(Span {
                y: span.y,
                x_start,
                x_end,
            });
            Ok(())
        };
        if full {
            push(span.x_start, span.x_end)?;
            continue;
        }
        for x in span.x_start..span.x_end {
            let on = row.get((x % n) as usize).copied().unwrap_or(false);
            match (on, open) {
                (true, None) => open = Some(x),
                (false, Some(start)) => {
                    push(start, x)?;
                    open = None;
                }
                _ => {}
            }
        }
        if let Some(start) = open {
            push(start, span.x_end)?;
        }
    }
    Ok(out)
}

/// Like [`fill_command`], but it only paints with `pixel` the pixels of the region that `tile`
/// covers. `None` if the region does not change (as in `fill_command`) or the pattern touches no
/// pixel of it. More than `Document::MAX_FILL_SPANS` spans, in the region or in the pattern, is
/// `Err(FillTooFragmented)`.
pub fn fill_pattern_command(
    doc: &Document,
    image: ImageId,
    x: i32,
    y: i32,
    pixel: Pixel,
    tile: &PatternTile,
) -> Result<Option<Command>, DocumentError> {
    let Some(Command::FillSpans { spans, .. }) = fill_command(doc, image, x, y, pixel)? else {
        return Ok(None);
    };
    let pieces = pattern_spans(&spans, tile, Document::MAX_FILL_SPANS)?;
    // The region is no longer needed: the peak is the maximum of the two lists, not the sum.
    drop(spans);
    if pieces.is_empty() {
        return Ok(None);
    }
    Ok(Some(Command::FillSpans {
        image,
        spans: pieces,
        pixel,
    }))
}

/// Fills the region of the seed `(x, y)` with `pixel` as a single undo step. Returns what
/// `Document::apply` returns; if there is nothing to change, `Ok(false)` without calling `apply`
/// (an empty `FillSpans` would be `EmptyCommand`).
pub fn flood_fill(
    doc: &mut Document,
    image: ImageId,
    x: i32,
    y: i32,
    pixel: Pixel,
) -> Result<bool, DocumentError> {
    match fill_command(doc, image, x, y, pixel)? {
        Some(command) => doc.apply(command),
        None => Ok(false),
    }
}

/// Map of one bit per pixel, contiguous (`y * width + x`), with no padding between rows.
struct Visited {
    bits: Vec<u64>,
    width: usize,
}

impl Visited {
    fn new(width: u32, height: u32) -> Result<Visited, DocumentError> {
        // `width * height` fits in `usize`: `Image::MAX_BYTES` limits the image to 2^30 pixels.
        let width = width as usize;
        let words = width
            .checked_mul(height as usize)
            .ok_or(DocumentError::OutOfMemory)?
            .div_ceil(64);
        let mut bits = Vec::new();
        bits.try_reserve_exact(words)
            .map_err(|_| DocumentError::OutOfMemory)?;
        bits.resize(words, 0);
        Ok(Visited { bits, width })
    }

    fn word(&self, index: usize) -> u64 {
        self.bits.get(index).copied().unwrap_or(0)
    }

    fn at(&self, x: u32, y: u32) -> usize {
        y as usize * self.width + x as usize
    }

    /// Marks the bits `from..to`.
    fn set_range(&mut self, from: usize, to: usize) {
        let mut i = from;
        while i < to {
            let (word, low) = (i / 64, i % 64);
            let high = (to - word * 64).min(64);
            let mask = if high - low == 64 {
                u64::MAX
            } else {
                ((1u64 << (high - low)) - 1) << low
            };
            if let Some(slot) = self.bits.get_mut(word) {
                *slot |= mask;
            }
            i = word * 64 + high;
        }
    }

    /// First bit set in `from..end`.
    fn next_one(&self, from: usize, end: usize) -> Option<usize> {
        let mut i = from;
        while i < end {
            let shifted = self.word(i / 64) >> (i % 64);
            if shifted != 0 {
                let found = i + shifted.trailing_zeros() as usize;
                return (found < end).then_some(found);
            }
            i = (i / 64 + 1) * 64;
        }
        None
    }

    /// First bit clear in `from..end`, or `end` if there is none.
    fn next_zero(&self, from: usize, end: usize) -> usize {
        let mut i = from;
        while i < end {
            let shifted = !(self.word(i / 64) >> (i % 64));
            let offset = shifted.trailing_zeros() as usize;
            // The zeros shifted in do not count: only a 0 inside the word is valid.
            if i % 64 + offset < 64 {
                return (i + offset).min(end);
            }
            i = (i / 64 + 1) * 64;
        }
        end
    }
}

/// Which pixels belong to the region, by their stored bytes: those that hold the key (`equal`) or
/// all those that do not.
struct Rule {
    key: [u8; 4],
    len: usize,
    equal: bool,
}

impl Rule {
    fn new(pixel: Pixel, equal: bool) -> Rule {
        let (key, len) = match pixel {
            Pixel::Rgba(c) => ([c.r, c.g, c.b, c.a], 4),
            Pixel::Indexed(i) => ([i, 0, 0, 0], 1),
            Pixel::Gray(g) => ([g, 0, 0, 0], 1),
            Pixel::GrayAlpha { gray, alpha } => ([gray, alpha, 0, 0], 2),
        };
        Rule { key, len, equal }
    }

    /// The pixels that hold `seed`.
    fn same(seed: Pixel) -> Rule {
        Rule::new(seed, true)
    }

    /// The pixels that do not hold `boundary`.
    fn not(boundary: Pixel) -> Rule {
        Rule::new(boundary, false)
    }

    /// Whether the pixel with bytes `chunk` belongs to the region.
    fn holds(&self, chunk: &[u8]) -> bool {
        let same = match chunk {
            // One byte per pixel (indexed, gray): no slice comparison.
            [byte] => self.len == 1 && *byte == self.key[0],
            _ => chunk == self.key.get(..self.len).unwrap_or(&[]),
        };
        same == self.equal
    }
}

/// Maximal span of pixels of the region in the row `row` (of line `y`) that contains `x` (which
/// belongs to it).
fn extend(row: &[u8], bpp: usize, x: u32, y: u32, rule: &Rule) -> Span {
    let at = x as usize * bpp;
    let left = row.get(..at).map_or(0, |head| {
        head.rchunks_exact(bpp)
            .take_while(|c| rule.holds(c))
            .count()
    });
    let right = row.get(at..).map_or(0, |tail| {
        tail.chunks_exact(bpp).take_while(|c| rule.holds(c)).count()
    });
    // `left <= x` and `x + right <= width`: they come from counting cells of the row itself.
    Span {
        y,
        x_start: x - left as u32,
        x_end: x + right as u32,
    }
}

/// The spans of the region of `(x, y)` (which belongs to it), ordered by `(y, x_start)`.
fn region_spans(
    image: &Image,
    x: u32,
    y: u32,
    rule: &Rule,
    max_spans: usize,
) -> Result<Vec<Span>, DocumentError> {
    let (width, height) = (image.width(), image.height());
    let bpp = image.mode().bytes_per_pixel();
    let mut visited = Visited::new(width, height)?;
    let mut stack: Vec<Span> = Vec::new();
    let mut count = 0usize;
    let (mut y_min, mut y_max) = (y, y);

    // Discovers a maximal span, marks it and pushes it. A visited pixel has its whole maximal
    // span visited, so each span enters once.
    let mut discover = |visited: &mut Visited, stack: &mut Vec<Span>, x: u32, y: u32| {
        // The cap goes before marking and pushing: the stack and the output never pass `max_spans`.
        if count >= max_spans {
            return Err(DocumentError::FillTooFragmented { limit: max_spans });
        }
        let span = match image.row(y) {
            Some(row) => extend(row, bpp, x, y, rule),
            None => Span {
                y,
                x_start: x,
                x_end: x + 1,
            },
        };
        let row = visited.at(0, y);
        visited.set_range(row + span.x_start as usize, row + span.x_end as usize);
        stack
            .try_reserve(1)
            .map_err(|_| DocumentError::OutOfMemory)?;
        stack.push(span);
        count += 1;
        y_min = y_min.min(y);
        y_max = y_max.max(y);
        Ok::<u32, DocumentError>(span.x_end)
    };

    discover(&mut visited, &mut stack, x, y)?;
    while let Some(span) = stack.pop() {
        let above = span.y.checked_sub(1);
        let below = span.y.checked_add(1).filter(|&next| next < height);
        for row in [above, below].into_iter().flatten() {
            let Some(bytes) = image.row(row) else {
                continue;
            };
            let (base, end) = (visited.at(0, row), span.x_end as usize);
            let mut cx = span.x_start;
            while cx < span.x_end {
                // Jump over the pixels that are already in a span: the common case in big regions.
                let free = visited.next_zero(base + cx as usize, base + end) - base;
                cx = free as u32;
                if cx >= span.x_end {
                    break;
                }
                let at = cx as usize * bpp;
                if bytes.get(at..at + bpp).is_some_and(|c| rule.holds(c)) {
                    cx = discover(&mut visited, &mut stack, cx, row)?;
                } else {
                    cx += 1;
                }
            }
        }
    }
    // The stack is dropped before reserving the output: the peak is the maximum of the two, not the sum.
    drop(stack);

    let mut spans = Vec::new();
    spans
        .try_reserve_exact(count)
        .map_err(|_| DocumentError::OutOfMemory)?;
    let w = visited.width;
    let (start, end) = (visited.at(0, y_min), visited.at(0, y_max) + w);
    let mut pos = start;
    while let Some(first) = visited.next_one(pos, end) {
        let last = visited.next_zero(first, end);
        // A run may cross the end of a row: it is split at the row boundaries.
        let mut piece = first;
        while piece < last {
            let row = piece / w;
            let piece_end = last.min((row + 1) * w);
            if let Ok(y) = u32::try_from(row)
                && let (Ok(x_start), Ok(x_end)) = (
                    u32::try_from(piece - row * w),
                    u32::try_from(piece_end - row * w),
                )
            {
                spans.push(Span { y, x_start, x_end });
            }
            piece = piece_end;
        }
        pos = last;
    }
    Ok(spans)
}
