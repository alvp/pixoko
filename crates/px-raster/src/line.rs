//! Línea determinista entre dos puntos (specs/hito-2-linea.md).

use std::iter::FusedIterator;

/// Segmento entre dos puntos, como iterador perezoso de píxeles `(x, y)`.
///
/// Con `dx = bx - ax`, `dy = by - ay` y `n = max(|dx|, |dy|)`, el segmento tiene `n + 1`
/// píxeles, indexados desde `a` hacia `b`. En el eje mayor la coordenada avanza de uno en uno;
/// en el otro vale `floor((2·d·i + n) / (2·n))` sobre el origen, es decir, el valor real
/// redondeado con la mitad hacia +y (eje mayor x) o hacia +x (eje mayor y). La regla depende
/// solo de la recta, así que `Line::new(a, b)` es `Line::new(b, a)` al revés.
///
/// Solo guarda los extremos y el rango de índices que quedan: ocupa memoria constante aunque
/// el segmento tenga 2³² píxeles. Toda la cuenta es entera.
#[derive(Clone, Debug)]
pub struct Line {
    /// Origen en el eje mayor y en el menor.
    major_start: i64,
    minor_start: i64,
    /// Sentido de avance en el eje mayor (`-1`, `0` o `1`; `0` solo si `n == 0`).
    major_step: i64,
    /// Desplazamiento total del eje menor (`dx` o `dy`, con su signo).
    minor_delta: i64,
    /// Longitud en el eje mayor (`n`).
    n: u64,
    /// El eje mayor es x (si no, es y).
    major_is_x: bool,
    /// Índices que quedan por devolver: `start..end`.
    start: u64,
    end: u64,
}

impl Line {
    /// Segmento de `a` a `b`, ambos incluidos. No falla con ningún `i32`.
    pub fn new(a: (i32, i32), b: (i32, i32)) -> Line {
        let dx = i64::from(b.0) - i64::from(a.0);
        let dy = i64::from(b.1) - i64::from(a.1);
        let major_is_x = dx.unsigned_abs() >= dy.unsigned_abs();
        let (major_start, minor_start, major_delta, minor_delta) = if major_is_x {
            (i64::from(a.0), i64::from(a.1), dx, dy)
        } else {
            (i64::from(a.1), i64::from(a.0), dy, dx)
        };
        let n = major_delta.unsigned_abs();
        Line {
            major_start,
            minor_start,
            major_step: major_delta.signum(),
            minor_delta,
            n,
            major_is_x,
            start: 0,
            end: n + 1,
        }
    }

    /// Píxeles que quedan por devolver. Es `u64` porque de `i32::MIN` a `i32::MAX` hay 2³².
    pub fn len(&self) -> u64 {
        self.end.saturating_sub(self.start)
    }

    /// `true` si no queda ningún píxel (por ejemplo, tras recortar).
    pub fn is_empty(&self) -> bool {
        self.start >= self.end
    }

    /// Deja solo los píxeles con `0 <= x < width` y `0 <= y < height`, en el mismo orden.
    ///
    /// No recorre el segmento: `x(i)` e `y(i)` son monótonas, así que los índices que caen
    /// dentro forman un intervalo, que se halla con una cuenta en el eje mayor y una búsqueda
    /// binaria en el menor. Los píxeles que quedan salen de la misma función que sin recortar.
    pub fn clip_to(mut self, width: u32, height: u32) -> Line {
        let (major_limit, minor_limit) = if self.major_is_x {
            (width, height)
        } else {
            (height, width)
        };
        if major_limit == 0 || minor_limit == 0 || self.is_empty() {
            self.start = self.end;
            return self;
        }

        // Eje mayor: `0 <= major_start + step·i < major_limit`.
        let first = i128::from(self.major_start);
        let last = i128::from(major_limit) - 1;
        let (lo, hi) = match self.major_step {
            1 => (-first, last - first),
            -1 => (first - last, first),
            // `n == 0`: un solo píxel, y su coordenada no cambia.
            _ if (0..=last).contains(&first) => (0, 0),
            _ => {
                self.start = self.end;
                return self;
            }
        };
        let lo = lo.max(i128::from(self.start));
        let hi = hi.min(i128::from(self.end) - 1);
        let (Ok(lo), Ok(hi)) = (u64::try_from(lo), u64::try_from(hi)) else {
            // `hi < 0` (todo el segmento queda antes del lienzo) o intervalo vacío.
            self.start = self.end;
            return self;
        };
        if lo > hi {
            self.start = self.end;
            return self;
        }

        // Eje menor: monótono no decreciente con `minor_delta >= 0`, no creciente si no.
        let below = |i: u64| self.minor(i) < 0;
        let above = |i: u64| self.minor(i) >= i128::from(minor_limit);
        let (new_start, new_end) = if self.minor_delta >= 0 {
            let s = first_true(lo, hi + 1, |i| !below(i));
            (s, first_true(s, hi + 1, above))
        } else {
            let s = first_true(lo, hi + 1, |i| !above(i));
            (s, first_true(s, hi + 1, below))
        };
        self.start = new_start;
        self.end = new_end;
        self
    }

    /// Como [`Line::clip_to`], pero con un margen de `margin` píxeles alrededor del lienzo: deja
    /// los píxeles con `-margin <= x < width + margin` (y lo mismo en `y`), en el mismo orden. Es
    /// para sellar un pincel cuyo centro cae fuera del lienzo pero cuyo cuerpo no.
    pub(crate) fn clip_with_margin(mut self, width: u32, height: u32, margin: u32) -> Line {
        let m = i64::from(margin);
        self.major_start += m;
        self.minor_start += m;
        let extra = margin.saturating_mul(2);
        let mut clipped = self.clip_to(width.saturating_add(extra), height.saturating_add(extra));
        clipped.major_start -= m;
        clipped.minor_start -= m;
        clipped
    }

    /// Coordenada del eje menor del píxel `i` (con `i <= n`).
    fn minor(&self, i: u64) -> i128 {
        let n = i128::from(self.n);
        if n == 0 {
            return i128::from(self.minor_start);
        }
        let numerator = 2 * i128::from(self.minor_delta) * i128::from(i) + n;
        i128::from(self.minor_start) + numerator.div_euclid(2 * n)
    }

    /// Píxel número `i` del segmento (con `i <= n`).
    fn pixel(&self, i: u64) -> (i32, i32) {
        let major = i128::from(self.major_start) + i128::from(self.major_step) * i128::from(i);
        let (major, minor) = (to_i32(major), to_i32(self.minor(i)));
        if self.major_is_x {
            (major, minor)
        } else {
            (minor, major)
        }
    }
}

/// Convierte a `i32`. Los valores que llegan están entre los dos extremos del segmento, así que
/// la conversión no falla; el recorte de la rama de error solo evita un pánico imposible.
fn to_i32(value: i128) -> i32 {
    i32::try_from(value).unwrap_or(if value < 0 { i32::MIN } else { i32::MAX })
}

/// Primer `i` de `lo..hi` donde `pred` es verdadero, suponiendo que pasa de falso a verdadero
/// una sola vez; `hi` si nunca lo es. Como mucho 33 llamadas.
fn first_true(mut lo: u64, mut hi: u64, pred: impl Fn(u64) -> bool) -> u64 {
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        if pred(mid) {
            hi = mid;
        } else {
            lo = mid + 1;
        }
    }
    lo
}

impl Iterator for Line {
    type Item = (i32, i32);

    fn next(&mut self) -> Option<(i32, i32)> {
        if self.is_empty() {
            return None;
        }
        let pixel = self.pixel(self.start);
        self.start += 1;
        Some(pixel)
    }

    fn nth(&mut self, k: usize) -> Option<(i32, i32)> {
        // `usize` cabe en `u64` en todos los destinos admitidos.
        let k = u64::try_from(k).unwrap_or(u64::MAX);
        if k >= self.len() {
            self.start = self.end;
            return None;
        }
        self.start += k;
        self.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        match usize::try_from(self.len()) {
            Ok(len) => (len, Some(len)),
            Err(_) => (usize::MAX, None),
        }
    }
}

impl FusedIterator for Line {}
