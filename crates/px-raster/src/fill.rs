//! Relleno por inundación como comando de `Document` (specs/hito-2-relleno.md).
//!
//! Relleno por tramos con pila explícita: cada tramo (serie máxima de píxeles de la región en
//! una fila) se marca en un mapa de visitados al descubrirlo y se apila una sola vez. Una pasada
//! final sobre el mapa devuelve los tramos ordenados por `(y, x_start)`, sin solapes y
//! fusionados, como exige `Command::FillSpans`.

use px_core::{Command, Document, DocumentError, Image, ImageId, Pixel, Span};

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
    let spans = region_spans(target, x, y, seed, max_spans)?;
    Ok(Some(Command::FillSpans {
        image,
        spans,
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

    fn contains(&self, x: u32, y: u32) -> bool {
        let bit = self.at(x, y);
        (self.word(bit / 64) >> (bit % 64)) & 1 == 1
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

fn matches(image: &Image, x: u32, y: u32, seed: Pixel) -> bool {
    matches!(image.get_pixel(x, y), Ok(found) if found == seed)
}

/// Tramo máximo de píxeles iguales a `seed` en la fila `y` que contiene a `x`.
fn extend(image: &Image, x: u32, y: u32, seed: Pixel) -> Span {
    let mut x_start = x;
    while x_start > 0 && matches(image, x_start - 1, y, seed) {
        x_start -= 1;
    }
    let mut x_end = x + 1;
    while x_end < image.width() && matches(image, x_end, y, seed) {
        x_end += 1;
    }
    Span { y, x_start, x_end }
}

/// Los tramos de la región de `(x, y)` (que vale `seed`), ordenados por `(y, x_start)`.
fn region_spans(
    image: &Image,
    x: u32,
    y: u32,
    seed: Pixel,
    max_spans: usize,
) -> Result<Vec<Span>, DocumentError> {
    let (width, height) = (image.width(), image.height());
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
        let span = extend(image, x, y, seed);
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
            let mut cx = span.x_start;
            while cx < span.x_end {
                if !visited.contains(cx, row) && matches(image, cx, row, seed) {
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
