//! Relleno por inundación como comando de `Document` (specs/hito-2-relleno.md), por filas desde
//! la unidad 4.5 (specs/hito-4-relleno.md): se comparan los bytes de los píxeles sobre las filas
//! de la imagen en vez de construir un `Pixel` por casilla. Además del relleno sólido, la región
//! «hasta el color de fondo» y el patrón alineado al lienzo.
//!
//! Relleno por tramos con pila explícita: cada tramo (serie máxima de píxeles de la región en
//! una fila) se marca en un mapa de visitados al descubrirlo y se apila una sola vez. Una pasada
//! final sobre el mapa devuelve los tramos ordenados por `(y, x_start)`, sin solapes y
//! fusionados, como exige `Command::FillSpans`.

use px_core::{Command, Document, DocumentError, Image, ImageId, Pixel, Span};

use crate::Brush;

/// `FillSpans` con la región de píxeles contiguos (4 vecinos) que valen lo mismo que la semilla
/// `(x, y)`, comparados por valor almacenado; `None` si no hay nada que cambiar.
///
/// Orden de comprobaciones: imagen desconocida, `pixel` que no es del modo de la imagen o con
/// índice fuera de la paleta (`Image::check_pixel`), semilla fuera del lienzo (`None`) y `pixel`
/// igual al valor de la semilla (`None`). Solo lee `doc`; reserva con `try_reserve*`, así que
/// la falta de memoria es `Err(OutOfMemory)` y nunca aborta. Una región con más de
/// `Document::MAX_FILL_SPANS` tramos es `Err(FillTooFragmented)` (specs/hito-2-limites-relleno.md).
pub fn fill_command(
    doc: &Document,
    image: ImageId,
    x: i32,
    y: i32,
    pixel: Pixel,
) -> Result<Option<Command>, DocumentError> {
    fill_command_with_limit(doc, image, x, y, pixel, Document::MAX_FILL_SPANS)
}

/// Como [`fill_command`], con el tope de tramos como parámetro: al descubrir el tramo número
/// `max_spans + 1` devuelve `Err(FillTooFragmented { limit: max_spans })` sin haber reservado la
/// salida; con exactamente `max_spans` tramos, `Ok`. El tope es la última comprobación.
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
    // Dentro del lienzo `get_pixel` no falla; fuera, no hay nada que rellenar.
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

/// Como [`fill_command`], pero la región es la de los píxeles conectados (4 vecinos) a la semilla
/// que **no valen** `boundary` (el color de fondo): «relleno hasta el color de fondo». Si la
/// semilla vale `boundary` no hay nada que rellenar (`None`); `pixel` puede ser igual a
/// `boundary`. Los topes de 14c valen igual.
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

/// Como [`fill_until_command`], con el tope de tramos como parámetro (ver
/// [`fill_command_with_limit`]). Orden de comprobaciones: imagen desconocida, `pixel` y
/// `boundary` que no son del modo de la imagen (`Pixel { pixel: 0 | 1, .. }`), semilla fuera
/// (`None`), semilla igual al fondo (`None`) y, el último, el tope.
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

/// La baldosa de un patrón: la máscara del pincel en su caja de `n × n`, repetida por todo el
/// lienzo y **alineada a su origen** (el píxel `(x, y)` está pintado si lo está la casilla
/// `(x mod n, y mod n)`), no a la semilla ni a la región.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PatternTile {
    size: u32,
    /// `size × size` casillas por filas.
    cells: Vec<bool>,
}

impl PatternTile {
    /// La máscara de `brush.offsets()` con el centro en `(⌊n/2⌋, ⌊n/2⌋)` de la caja.
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

    /// Si el píxel `(x, y)` del lienzo está pintado por el patrón.
    pub fn contains(&self, x: u32, y: u32) -> bool {
        let n = self.size as usize;
        let at = (y % self.size) as usize * n + (x % self.size) as usize;
        self.cells.get(at).copied().unwrap_or(false)
    }

    /// La fila `y mod n` de la baldosa.
    fn row(&self, y: u32) -> &[bool] {
        let n = self.size as usize;
        let at = (y % self.size) as usize * n;
        self.cells.get(at..at + n).unwrap_or(&[])
    }
}

/// Los trozos de `region` (tramos ordenados y sin solapes) que pinta el patrón: ordenados, sin
/// solapes, sin tramos vacíos y contenidos en la región. Si salen más de `max_spans`,
/// `Err(FillTooFragmented { limit: max_spans })` sin haber reservado nada de más (la salida nunca
/// pasa de `max_spans` tramos).
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

/// Como [`fill_command`], pero solo pinta con `pixel` los píxeles de la región que cubre `tile`.
/// `None` si la región no cambia (como `fill_command`) o si el patrón no toca ningún píxel de
/// ella. Más de `Document::MAX_FILL_SPANS` tramos, en la región o en el patrón, es
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
    // La región ya no hace falta: el pico es el máximo de las dos listas, no la suma.
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

/// Rellena con `pixel` la región de la semilla `(x, y)` como un solo paso de deshacer.
/// Devuelve lo que devuelve `Document::apply`; si no hay nada que cambiar, `Ok(false)` sin
/// llamar a `apply` (un `FillSpans` vacío sería `EmptyCommand`).
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

/// Mapa de un bit por píxel, contiguo (`y * width + x`), sin relleno entre filas.
struct Visited {
    bits: Vec<u64>,
    width: usize,
}

impl Visited {
    fn new(width: u32, height: u32) -> Result<Visited, DocumentError> {
        // `width * height` cabe en `usize`: `Image::MAX_BYTES` limita la imagen a 2^30 píxeles.
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

    /// Marca los bits `from..to`.
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

    /// Primer bit a 1 en `from..end`.
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

    /// Primer bit a 0 en `from..end`, o `end` si no hay.
    fn next_zero(&self, from: usize, end: usize) -> usize {
        let mut i = from;
        while i < end {
            let shifted = !(self.word(i / 64) >> (i % 64));
            let offset = shifted.trailing_zeros() as usize;
            // Los ceros desplazados hacia dentro no cuentan: solo vale un 0 dentro de la palabra.
            if i % 64 + offset < 64 {
                return (i + offset).min(end);
            }
            i = (i / 64 + 1) * 64;
        }
        end
    }
}

/// Qué píxeles pertenecen a la región, por sus bytes almacenados: los que valen la clave
/// (`equal`) o todos los que no la valen.
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

    /// Los píxeles que valen `seed`.
    fn same(seed: Pixel) -> Rule {
        Rule::new(seed, true)
    }

    /// Los píxeles que no valen `boundary`.
    fn not(boundary: Pixel) -> Rule {
        Rule::new(boundary, false)
    }

    /// Si el píxel de bytes `chunk` pertenece a la región.
    fn holds(&self, chunk: &[u8]) -> bool {
        let same = match chunk {
            // One byte per pixel (indexed, gray): no slice comparison.
            [byte] => self.len == 1 && *byte == self.key[0],
            _ => chunk == self.key.get(..self.len).unwrap_or(&[]),
        };
        same == self.equal
    }
}

/// Tramo máximo de píxeles de la región en la fila `row` (de la fila `y`) que contiene a `x`
/// (que pertenece a ella).
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
    // `left <= x` y `x + right <= width`: salen de contar casillas de la propia fila.
    Span {
        y,
        x_start: x - left as u32,
        x_end: x + right as u32,
    }
}

/// Los tramos de la región de `(x, y)` (que pertenece a ella), ordenados por `(y, x_start)`.
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

    // Descubre un tramo máximo, lo marca y lo apila. Un píxel visitado tiene visitado todo su
    // tramo máximo, así que cada tramo entra una sola vez.
    let mut discover = |visited: &mut Visited, stack: &mut Vec<Span>, x: u32, y: u32| {
        // El tope va antes de marcar y de apilar: la pila y la salida nunca pasan de `max_spans`.
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
    // La pila se suelta antes de reservar la salida: el pico es el máximo de las dos, no la suma.
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
        // Una racha puede cruzar el final de una fila: se parte en los límites de fila.
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
