//! Color quantizer (specs/hito-4-cuantizador.md): RGBA or gray-with-alpha cells to indexed cells
//! with one common palette of at most 256 colors. Integer arithmetic only, no hash maps, ties
//! broken by index: the same input always gives the same bytes.
//!
//! The steps, in order: one scan of the cells (alpha threshold, histogram of 6 bits per channel and
//! the exact list of colors while it stays small); if the colors fit the palette is the exact one
//! (the one of `convert_cels`); otherwise a weighted median cut over the histogram refined with
//! three Lloyd iterations; and a last pass that maps every pixel (optionally dithering).

use std::fmt;

use px_core::{BlendMode, Command, Document, Image, Mode, Palette, Rgba};

/// How the quantization error is spread (decision D6).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dither {
    /// Every pixel takes the closest color.
    None,
    /// Floyd–Steinberg, left to right, error in sixteenths.
    FloydSteinberg,
    /// Ordered, 8 × 8 Bayer matrix.
    Ordered,
}

/// Where the palette comes from (decision D3).
#[derive(Clone, Debug, PartialEq)]
pub enum PaletteSource {
    /// Computed from the pixels.
    Computed,
    /// The given palette is the palette of the result, transparent index included.
    Fixed(Palette),
}

#[derive(Clone, Debug, PartialEq)]
pub struct QuantizeOptions {
    /// Number of entries of the palette, transparent one included (1 to 256). Ignored with a fixed
    /// palette.
    pub colors: u16,
    pub dither: Dither,
    /// A pixel with an alpha below this is transparent (1 to 255).
    pub alpha_threshold: u8,
    pub palette: PaletteSource,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QuantizeError {
    /// `colors` is not between 1 and 256.
    InvalidColorCount,
    /// `alpha_threshold` is 0.
    InvalidThreshold,
    /// A transparent entry is needed and `colors` is 1, or a fixed palette has no opaque entry.
    NoColorSlots,
    /// The source is not RGBA or gray with alpha.
    SourceMode(Mode),
    /// A cell does not have the size given.
    SizeMismatch,
    /// A layer with an opacity other than 255.
    LayerOpacity,
    /// A layer with a blend mode other than `Normal`.
    LayerBlend,
    /// A transparent entry is needed and the fixed palette has none.
    FixedNeedsTransparent,
    /// The result would exceed the limit of the cells of the document.
    TooLarge {
        needed: usize,
        limit: usize,
    },
    OutOfMemory,
}

impl fmt::Display for QuantizeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            QuantizeError::InvalidColorCount => write!(f, "el número de colores va de 1 a 256"),
            QuantizeError::InvalidThreshold => write!(f, "el alfa mínimo va de 1 a 255"),
            QuantizeError::NoColorSlots => write!(f, "no queda sitio para ningún color"),
            QuantizeError::SourceMode(mode) => {
                write!(
                    f,
                    "solo se reducen RGBA y gris con alfa, y el modo es {mode:?}"
                )
            }
            QuantizeError::SizeMismatch => write!(f, "una celda no tiene el tamaño del lienzo"),
            QuantizeError::LayerOpacity => {
                write!(f, "en indexado la opacidad de las capas es siempre 255")
            }
            QuantizeError::LayerBlend => {
                write!(f, "en indexado todas las capas se mezclan en modo Normal")
            }
            QuantizeError::FixedNeedsTransparent => {
                write!(
                    f,
                    "la paleta no tiene entrada transparente y la imagen la necesita"
                )
            }
            QuantizeError::TooLarge { needed, limit } => write!(
                f,
                "las celdas necesitarían {needed} bytes y el límite es {limit}"
            ),
            QuantizeError::OutOfMemory => write!(f, "no hay memoria para reducir los colores"),
        }
    }
}

impl std::error::Error for QuantizeError {}

/// What the quantization did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QuantizeStats {
    /// Entries of the palette.
    pub colors: usize,
    /// Every opaque color of the image is an entry (so the error is 0).
    pub exact: bool,
    pub transparent_pixels: u64,
    /// Pixels at or above the alpha threshold.
    pub opaque_pixels: u64,
    /// Sum over the opaque pixels of |dr| + |dg| + |db| against the entry they took.
    pub abs_error: u64,
}

impl QuantizeStats {
    /// Mean absolute error per channel in thousandths of a level; `None` without opaque pixels.
    pub fn mae_milli(&self) -> Option<u64> {
        (self.opaque_pixels > 0).then(|| self.abs_error * 1000 / (3 * self.opaque_pixels))
    }
}

#[derive(Clone, Debug)]
pub struct Quantized {
    /// One per input cell; `None` where the cell was empty.
    pub images: Vec<Option<Image>>,
    pub palette: Palette,
    pub stats: QuantizeStats,
}

/// Error of an indexed image against its original (decision D8).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ErrorStats {
    pub pixels: u64,
    pub sum_abs: u64,
    pub sum_sq: u64,
}

impl ErrorStats {
    /// Mean absolute error per channel in thousandths of a level.
    pub fn mae_milli(&self) -> u64 {
        self.sum_abs * 1000 / (3 * self.pixels.max(1))
    }
}

/// The error of `indexed` against `original` over the pixels whose alpha is at or above
/// `alpha_threshold`. `None` without such pixels or when the sizes differ.
pub fn mean_abs_error(
    original: &Image,
    indexed: &Image,
    alpha_threshold: u8,
) -> Option<ErrorStats> {
    if (original.width(), original.height()) != (indexed.width(), indexed.height()) {
        return None;
    }
    let palette = indexed.palette()?;
    let source = Source::new(original)?;
    let mut stats = ErrorStats {
        pixels: 0,
        sum_abs: 0,
        sum_sq: 0,
    };
    for (i, index) in indexed.bytes().iter().enumerate() {
        let [r, g, b, a] = source.get(i);
        if a < alpha_threshold {
            continue;
        }
        let entry = palette.get(*index).ok()?;
        for (x, y) in [(r, entry.r), (g, entry.g), (b, entry.b)] {
            let d = u64::from(x.abs_diff(y));
            stats.sum_abs += d;
            stats.sum_sq += d * d;
        }
        stats.pixels += 1;
    }
    (stats.pixels > 0).then_some(stats)
}

// ---------------------------------------------------------------------------------------------
// Reading pixels
// ---------------------------------------------------------------------------------------------

/// The bytes of an RGBA or gray-with-alpha image read as `[r, g, b, a]`.
struct Source<'a> {
    bytes: &'a [u8],
    gray_alpha: bool,
}

impl<'a> Source<'a> {
    fn new(image: &'a Image) -> Option<Source<'a>> {
        match image.mode() {
            Mode::Rgba => Some(Source {
                bytes: image.bytes(),
                gray_alpha: false,
            }),
            Mode::GrayAlpha => Some(Source {
                bytes: image.bytes(),
                gray_alpha: true,
            }),
            _ => None,
        }
    }

    #[inline]
    fn get(&self, i: usize) -> [u8; 4] {
        if self.gray_alpha {
            let (gray, alpha) = (self.bytes[i * 2], self.bytes[i * 2 + 1]);
            [gray, gray, gray, alpha]
        } else {
            let at = i * 4;
            [
                self.bytes[at],
                self.bytes[at + 1],
                self.bytes[at + 2],
                self.bytes[at + 3],
            ]
        }
    }
}

#[inline]
fn pack(rgb: [u8; 3]) -> u32 {
    u32::from_be_bytes([0, rgb[0], rgb[1], rgb[2]])
}

#[inline]
fn bin_of(rgb: [u8; 3]) -> usize {
    (usize::from(rgb[0] >> 2) << 12) | (usize::from(rgb[1] >> 2) << 6) | usize::from(rgb[2] >> 2)
}

#[inline]
fn dist2(a: [i32; 3], b: [i32; 3]) -> i64 {
    let d = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    d.iter().map(|x| i64::from(*x) * i64::from(*x)).sum()
}

const BINS: usize = 1 << 18;

fn zeroed<T: Clone>(len: usize, value: T) -> Result<Vec<T>, QuantizeError> {
    let mut v = Vec::new();
    v.try_reserve_exact(len)
        .map_err(|_| QuantizeError::OutOfMemory)?;
    v.resize(len, value);
    Ok(v)
}

// ---------------------------------------------------------------------------------------------
// Scan: transparency, histogram and the exact list of colors
// ---------------------------------------------------------------------------------------------

struct Scan {
    transparent_pixels: u64,
    /// Histogram of 6 bits per channel: pixel count and sum of r, g, b per bin.
    count: Vec<u64>,
    sum: Vec<[u64; 3]>,
    /// Opaque colors in order of first appearance while there are at most 256 of them.
    first_seen: Vec<[u8; 3]>,
    exact_active: bool,
}

fn scan(
    sources: &[Option<Source<'_>>],
    pixels: usize,
    threshold: u8,
    histogram: bool,
) -> Result<Scan, QuantizeError> {
    let (count, sum) = if histogram {
        (zeroed(BINS, 0u64)?, zeroed(BINS, [0u64; 3])?)
    } else {
        (Vec::new(), Vec::new())
    };
    let mut scan = Scan {
        transparent_pixels: 0,
        count,
        sum,
        first_seen: Vec::new(),
        exact_active: histogram,
    };
    let mut sorted: Vec<u32> = Vec::new();
    let mut last = u32::MAX;
    for source in sources.iter().flatten() {
        for i in 0..pixels {
            let [r, g, b, a] = source.get(i);
            if a < threshold {
                scan.transparent_pixels += 1;
                continue;
            }
            if !histogram {
                continue;
            }
            let rgb = [r, g, b];
            let bin = bin_of(rgb);
            scan.count[bin] += 1;
            let s = &mut scan.sum[bin];
            s[0] += u64::from(r);
            s[1] += u64::from(g);
            s[2] += u64::from(b);
            let key = pack(rgb);
            if scan.exact_active && key != last {
                last = key;
                if let Err(at) = sorted.binary_search(&key) {
                    if sorted.len() == Palette::MAX_COLORS {
                        scan.exact_active = false;
                    } else {
                        sorted.insert(at, key);
                        scan.first_seen.push(rgb);
                    }
                }
            }
        }
    }
    Ok(scan)
}

// ---------------------------------------------------------------------------------------------
// Palette: weighted median cut and Lloyd iterations
// ---------------------------------------------------------------------------------------------

/// The mean color of a non-empty bin.
fn bin_mean(scan: &Scan, bin: usize) -> [i32; 3] {
    let n = scan.count[bin];
    let s = scan.sum[bin];
    [
        ((s[0] + n / 2) / n) as i32,
        ((s[1] + n / 2) / n) as i32,
        ((s[2] + n / 2) / n) as i32,
    ]
}

struct CutBox {
    start: usize,
    end: usize,
    /// Squared error of the bins of the box around their weighted mean.
    sse: i128,
}

fn box_sse(scan: &Scan, means: &[[i32; 3]], bins: &[u32], range: (usize, usize)) -> i128 {
    let (mut n, mut s, mut a) = (0i128, [0i128; 3], 0i128);
    for k in range.0..range.1 {
        let c = i128::from(scan.count[bins[k] as usize]);
        let m = means[k];
        n += c;
        for axis in 0..3 {
            s[axis] += c * i128::from(m[axis]);
        }
        a += c * m
            .iter()
            .map(|x| i128::from(*x) * i128::from(*x))
            .sum::<i128>();
    }
    if n == 0 {
        return 0;
    }
    a - s.iter().map(|x| x * x).sum::<i128>() / n
}

/// The palette colors (opaque, `slots` at most) of the occupied bins of `scan`.
fn compute_colors(scan: &Scan, slots: usize) -> Vec<[u8; 3]> {
    // Occupied bins in ascending order, and their mean colors.
    let mut bins: Vec<u32> = (0..BINS as u32)
        .filter(|b| scan.count[*b as usize] > 0)
        .collect();
    let mut means: Vec<[i32; 3]> = bins.iter().map(|b| bin_mean(scan, *b as usize)).collect();
    if bins.is_empty() {
        return Vec::new();
    }
    let mut boxes = vec![CutBox {
        start: 0,
        end: bins.len(),
        sse: box_sse(scan, &means, &bins, (0, bins.len())),
    }];
    while boxes.len() < slots {
        // The splittable box with the largest error (ties: the first).
        let Some(at) = (0..boxes.len())
            .filter(|i| boxes[*i].end - boxes[*i].start > 1 && boxes[*i].sse > 0)
            .max_by(|a, b| boxes[*a].sse.cmp(&boxes[*b].sse).then(b.cmp(a)))
        else {
            break;
        };
        let (start, end) = (boxes[at].start, boxes[at].end);
        // Axis of the largest weighted variance (ties: the lowest axis).
        let mut best_axis = 0;
        let mut best_var = -1i128;
        for axis in 0..3 {
            let (mut n, mut s, mut q) = (0i128, 0i128, 0i128);
            for (bin, m) in bins[start..end].iter().zip(&means[start..end]) {
                let c = i128::from(scan.count[*bin as usize]);
                let v = i128::from(m[axis]);
                n += c;
                s += c * v;
                q += c * v * v;
            }
            let var = q - s * s / n.max(1);
            if var > best_var {
                best_var = var;
                best_axis = axis;
            }
        }
        // Sort the slice of the box along that axis (ties: the bin) and cut at the weighted median.
        let mut order: Vec<usize> = (start..end).collect();
        order.sort_by_key(|k| (means[*k][best_axis], bins[*k]));
        let sorted_bins: Vec<u32> = order.iter().map(|k| bins[*k]).collect();
        let sorted_means: Vec<[i32; 3]> = order.iter().map(|k| means[*k]).collect();
        bins[start..end].copy_from_slice(&sorted_bins);
        means[start..end].copy_from_slice(&sorted_means);
        let total: u64 = (start..end).map(|k| scan.count[bins[k] as usize]).sum();
        let mut running = 0u64;
        let mut cut = start;
        for (k, bin) in bins.iter().enumerate().take(end).skip(start) {
            running += scan.count[*bin as usize];
            cut = k;
            if running * 2 >= total {
                break;
            }
        }
        let cut = cut.min(end - 2) + 1;
        let left = CutBox {
            start,
            end: cut,
            sse: box_sse(scan, &means, &bins, (start, cut)),
        };
        let right = CutBox {
            start: cut,
            end,
            sse: box_sse(scan, &means, &bins, (cut, end)),
        };
        boxes[at] = left;
        boxes.push(right);
    }
    // Stable order: boxes by their smallest bin.
    let mut keyed: Vec<(u32, usize)> = boxes
        .iter()
        .enumerate()
        .map(|(i, b)| {
            let first = bins[b.start..b.end].iter().copied().min().unwrap_or(0);
            (first, i)
        })
        .collect();
    keyed.sort();
    let mut centers: Vec<[i32; 3]> = Vec::with_capacity(boxes.len());
    for (_, i) in &keyed {
        centers.push(box_center(scan, &bins[boxes[*i].start..boxes[*i].end]));
    }
    // Lloyd iterations over the occupied bins.
    for _ in 0..3 {
        let mut acc = vec![([0u64; 3], 0u64); centers.len()];
        for (b, mean) in bins.iter().zip(&means) {
            let nearest = nearest_center(&centers, *mean);
            let bin = *b as usize;
            let slot = &mut acc[nearest];
            for axis in 0..3 {
                slot.0[axis] += scan.sum[bin][axis];
            }
            slot.1 += scan.count[bin];
        }
        for (center, (s, n)) in centers.iter_mut().zip(&acc) {
            if *n > 0 {
                *center = [
                    ((s[0] + n / 2) / n) as i32,
                    ((s[1] + n / 2) / n) as i32,
                    ((s[2] + n / 2) / n) as i32,
                ];
            }
        }
    }
    centers
        .iter()
        .map(|c| [c[0] as u8, c[1] as u8, c[2] as u8])
        .collect()
}

fn box_center(scan: &Scan, bins: &[u32]) -> [i32; 3] {
    let (mut s, mut n) = ([0u64; 3], 0u64);
    for b in bins {
        let bin = *b as usize;
        for (total, v) in s.iter_mut().zip(scan.sum[bin]) {
            *total += v;
        }
        n += scan.count[bin];
    }
    let n = n.max(1);
    [
        ((s[0] + n / 2) / n) as i32,
        ((s[1] + n / 2) / n) as i32,
        ((s[2] + n / 2) / n) as i32,
    ]
}

fn nearest_center(centers: &[[i32; 3]], color: [i32; 3]) -> usize {
    let mut best = (i64::MAX, 0);
    for (i, c) in centers.iter().enumerate() {
        let d = dist2(*c, color);
        if d < best.0 {
            best = (d, i);
        }
    }
    best.1
}

// ---------------------------------------------------------------------------------------------
// Mapping pixels to entries
// ---------------------------------------------------------------------------------------------

struct Mapper {
    /// `(color, index)` of the opaque entries, in index order.
    entries: Vec<([i32; 3], u8)>,
    /// `(packed color, index)`, sorted by color: exact matches (the lowest index wins).
    exact: Vec<(u32, u8)>,
    /// The color of every index of the palette (the transparent one included).
    by_index: Vec<[i32; 3]>,
    /// Lazy table of the closest entry to the center of every 6-bit bin (`u16::MAX`: unknown).
    table: Vec<u16>,
}

impl Mapper {
    fn new(palette: &Palette) -> Result<Mapper, QuantizeError> {
        let skip = palette.transparent_index();
        let entries: Vec<([i32; 3], u8)> = palette
            .colors()
            .iter()
            .enumerate()
            .filter(|(i, _)| Some(*i as u8) != skip)
            .map(|(i, c)| ([i32::from(c.r), i32::from(c.g), i32::from(c.b)], i as u8))
            .collect();
        if entries.is_empty() {
            return Err(QuantizeError::NoColorSlots);
        }
        let mut exact: Vec<(u32, u8)> = entries
            .iter()
            .map(|(c, i)| (pack([c[0] as u8, c[1] as u8, c[2] as u8]), *i))
            .collect();
        exact.sort();
        exact.dedup_by_key(|e| e.0);
        let by_index = palette
            .colors()
            .iter()
            .map(|c| [i32::from(c.r), i32::from(c.g), i32::from(c.b)])
            .collect();
        Ok(Mapper {
            entries,
            exact,
            by_index,
            table: zeroed(BINS, u16::MAX)?,
        })
    }

    /// The index of the entry for `rgb`: an exact match, or the closest to the center of its bin
    /// (at most 2 levels per channel from the exact search, decision D5).
    #[inline]
    fn index(&mut self, rgb: [u8; 3]) -> u8 {
        let key = pack(rgb);
        if let Ok(at) = self.exact.binary_search_by_key(&key, |e| e.0) {
            return self.exact[at].1;
        }
        let bin = bin_of(rgb);
        let cached = self.table[bin];
        if cached != u16::MAX {
            return cached as u8;
        }
        // Doubled coordinates: the bin spans 4 levels, its center is 4k + 1.5.
        let center = [
            (i32::from(rgb[0] >> 2) << 3) + 3,
            (i32::from(rgb[1] >> 2) << 3) + 3,
            (i32::from(rgb[2] >> 2) << 3) + 3,
        ];
        let mut best = (i64::MAX, 0u8);
        for (c, i) in &self.entries {
            let d = dist2([c[0] * 2, c[1] * 2, c[2] * 2], center);
            if d < best.0 {
                best = (d, *i);
            }
        }
        self.table[bin] = u16::from(best.1);
        best.1
    }

    #[inline]
    fn color(&self, index: u8) -> [i32; 3] {
        self.by_index[usize::from(index)]
    }
}

/// The 8 × 8 Bayer matrix, built by the recursion `M(2n) = [[4M, 4M + 2], [4M + 3, 4M + 1]]`:
/// `M8[y][x] = 4 · M4[y % 4][x % 4] + M2[y / 4][x / 4]`, and the same one level down.
fn bayer8() -> [[u8; 8]; 8] {
    const M2: [[u8; 2]; 2] = [[0, 2], [3, 1]];
    let mut m = [[0u8; 8]; 8];
    for (y, row) in m.iter_mut().enumerate() {
        for (x, cell) in row.iter_mut().enumerate() {
            let m4 = 4 * M2[y % 2][x % 2] + M2[(y / 2) % 2][(x / 2) % 2];
            *cell = 4 * m4 + M2[y / 4][x / 4];
        }
    }
    m
}

/// Smallest `c` with `c³ ≥ k` (at least 1).
fn ceil_cbrt(k: usize) -> i32 {
    let mut c = 1;
    while (c * c * c) < k as i32 {
        c += 1;
    }
    c
}

/// Amplitude `s` of the ordered dither: the offsets go from about `-s/2` to `s/2`.
///
/// It is the smaller of the amplitude of a color cube with `k` opaque entries (`256 / ⌈∛k⌉`) and
/// the mean distance from each opaque entry to its nearest neighbour (largest difference of a
/// channel). A palette that is not a cube, such as a ramp of grays, has its entries much closer
/// than the cube would say, and the cube amplitude would drown the image in noise. The result
/// is at least 1.
fn ordered_spread(palette: &Palette, transparent: Option<u8>, opaque: usize) -> i32 {
    let cube = 256 / ceil_cbrt(opaque.max(1));
    let colors: Vec<[i32; 3]> = (0..palette.len())
        .filter(|&i| Some(i as u8) != transparent)
        .filter_map(|i| palette.get(i as u8).ok())
        .map(|c| [i32::from(c.r), i32::from(c.g), i32::from(c.b)])
        .collect();
    if colors.len() < 2 {
        return cube;
    }
    let sum: i64 = colors
        .iter()
        .enumerate()
        .map(|(i, a)| {
            colors
                .iter()
                .enumerate()
                .filter(|&(j, _)| j != i)
                .map(|(_, b)| (0..3).map(|k| (a[k] - b[k]).abs()).max().unwrap_or(0))
                .filter(|&d| d > 0)
                .min()
                .map_or(0, i64::from)
        })
        .sum();
    let nearest = (sum / colors.len() as i64) as i32;
    cube.min(nearest).max(1)
}

// ---------------------------------------------------------------------------------------------
// The quantizer
// ---------------------------------------------------------------------------------------------

/// Quantizes the cells (bottom to top, all of `size`) of `source` mode into indexed cells sharing
/// one palette. The transparent entry (when needed: several cells, an empty one, or a pixel below
/// the threshold) is index 0 of a computed palette.
pub fn quantize_cels(
    cels: &[Option<&Image>],
    size: (u32, u32),
    source: Mode,
    options: &QuantizeOptions,
) -> Result<Quantized, QuantizeError> {
    if options.colors == 0 || options.colors > 256 {
        return Err(QuantizeError::InvalidColorCount);
    }
    if options.alpha_threshold == 0 {
        return Err(QuantizeError::InvalidThreshold);
    }
    if !matches!(source, Mode::Rgba | Mode::GrayAlpha) {
        return Err(QuantizeError::SourceMode(source));
    }
    let (width, height) = size;
    let pixels = (width as usize)
        .checked_mul(height as usize)
        .ok_or(QuantizeError::OutOfMemory)?;
    let mut sources: Vec<Option<Source<'_>>> = Vec::with_capacity(cels.len());
    for cel in cels {
        sources.push(match cel {
            None => None,
            Some(image) => {
                if image.mode() != source {
                    return Err(QuantizeError::SourceMode(image.mode()));
                }
                if (image.width(), image.height()) != size {
                    return Err(QuantizeError::SizeMismatch);
                }
                Source::new(image)
            }
        });
    }
    let threshold = options.alpha_threshold;
    let computed = matches!(options.palette, PaletteSource::Computed);
    let scanned = scan(&sources, pixels, threshold, computed)?;
    let needs_transparent =
        cels.len() > 1 || cels.iter().any(Option::is_none) || scanned.transparent_pixels > 0;

    // The palette, and whether it is the exact one.
    let (palette, exact) = match &options.palette {
        PaletteSource::Fixed(palette) => {
            if needs_transparent && palette.transparent_index().is_none() {
                return Err(QuantizeError::FixedNeedsTransparent);
            }
            (palette.clone(), false)
        }
        PaletteSource::Computed => {
            let slots = usize::from(options.colors) - usize::from(needs_transparent);
            if slots == 0 {
                return Err(QuantizeError::NoColorSlots);
            }
            let (colors, exact) = if scanned.exact_active && scanned.first_seen.len() <= slots {
                (scanned.first_seen.clone(), true)
            } else {
                (compute_colors(&scanned, slots), false)
            };
            let transparent = needs_transparent.then_some(Rgba::TRANSPARENT);
            let mut palette = Palette::from_colors(
                transparent
                    .into_iter()
                    .chain(colors.iter().map(|c| Rgba::new(c[0], c[1], c[2], 255))),
            )
            .map_err(|_| QuantizeError::NoColorSlots)?;
            if needs_transparent {
                palette
                    .set_transparent_index(Some(0))
                    .map_err(|_| QuantizeError::NoColorSlots)?;
            }
            (palette, exact)
        }
    };
    if palette.is_empty() {
        return Err(QuantizeError::NoColorSlots);
    }
    let transparent_index = palette.transparent_index();
    let opaque_entries = palette.len() - usize::from(transparent_index.is_some());
    let total = pixels as u64 * sources.iter().flatten().count() as u64;
    let mut mapper = if total > scanned.transparent_pixels {
        Some(Mapper::new(&palette)?)
    } else {
        None
    };

    let mut stats = QuantizeStats {
        colors: palette.len(),
        exact,
        transparent_pixels: scanned.transparent_pixels,
        opaque_pixels: 0,
        abs_error: 0,
    };
    let dither = if exact { Dither::None } else { options.dither };
    let matrix = bayer8();
    let spread = ordered_spread(&palette, transparent_index, opaque_entries);
    let transparent_byte = transparent_index.unwrap_or(0);
    let mut images = Vec::new();
    images
        .try_reserve_exact(cels.len())
        .map_err(|_| QuantizeError::OutOfMemory)?;
    for source in &sources {
        let Some(source) = source else {
            images.push(None);
            continue;
        };
        let mut data = Vec::new();
        data.try_reserve_exact(pixels)
            .map_err(|_| QuantizeError::OutOfMemory)?;
        data.resize(pixels, 0u8);
        if let Some(mapper) = mapper.as_mut() {
            let mut cursor = Cursor {
                mapper,
                source,
                data: &mut data,
                width: width as usize,
                height: height as usize,
                threshold,
                transparent: transparent_byte,
                stats: &mut stats,
            };
            match dither {
                Dither::None => cursor.run_plain(),
                Dither::Ordered => cursor.run_ordered(&matrix, spread),
                Dither::FloydSteinberg => cursor.run_diffused()?,
            }
        } else {
            data.fill(transparent_byte);
        }
        let image = Image::from_bytes_indexed(width, height, palette.clone(), data)
            .map_err(|_| QuantizeError::OutOfMemory)?;
        images.push(Some(image));
    }
    Ok(Quantized {
        images,
        palette,
        stats,
    })
}

/// Everything the mapping loops share.
struct Cursor<'a, 'b> {
    mapper: &'a mut Mapper,
    source: &'a Source<'b>,
    data: &'a mut Vec<u8>,
    width: usize,
    height: usize,
    threshold: u8,
    transparent: u8,
    stats: &'a mut QuantizeStats,
}

impl Cursor<'_, '_> {
    /// Records the error of mapping `rgb` (the true color) to `index`.
    #[inline]
    fn account(&mut self, rgb: [u8; 3], index: u8) {
        let c = self.mapper.color(index);
        self.stats.opaque_pixels += 1;
        self.stats.abs_error += (0..3)
            .map(|k| u64::from(rgb[k].abs_diff(c[k] as u8)))
            .sum::<u64>();
    }

    fn run_plain(&mut self) {
        let mut last: Option<([u8; 3], u8)> = None;
        for i in 0..self.width * self.height {
            let [r, g, b, a] = self.source.get(i);
            if a < self.threshold {
                self.data[i] = self.transparent;
                continue;
            }
            let rgb = [r, g, b];
            let index = match last {
                Some((c, idx)) if c == rgb => idx,
                _ => {
                    let idx = self.mapper.index(rgb);
                    last = Some((rgb, idx));
                    idx
                }
            };
            self.data[i] = index;
            self.account(rgb, index);
        }
    }

    fn run_ordered(&mut self, matrix: &[[u8; 8]; 8], spread: i32) {
        for y in 0..self.height {
            for x in 0..self.width {
                let i = y * self.width + x;
                let [r, g, b, a] = self.source.get(i);
                if a < self.threshold {
                    self.data[i] = self.transparent;
                    continue;
                }
                let m = i32::from(matrix[y % 8][x % 8]);
                let offset = ((2 * m + 1 - 64) * spread) / 128;
                let shifted = [r, g, b].map(|v| (i32::from(v) + offset).clamp(0, 255) as u8);
                let index = self.mapper.index(shifted);
                self.data[i] = index;
                self.account([r, g, b], index);
            }
        }
    }

    fn run_diffused(&mut self) -> Result<(), QuantizeError> {
        let w = self.width;
        // Error in sixteenths of a level for the current and the next row, with one spare cell at
        // each end so that no index needs a check.
        let mut current = zeroed(w + 2, [0i32; 3])?;
        let mut next = zeroed(w + 2, [0i32; 3])?;
        for y in 0..self.height {
            for x in 0..w {
                let i = y * w + x;
                let [r, g, b, a] = self.source.get(i);
                if a < self.threshold {
                    self.data[i] = self.transparent;
                    continue;
                }
                let carried = current[x + 1];
                let adjusted = [
                    (i32::from(r) + ((carried[0] + 8) >> 4)).clamp(0, 255),
                    (i32::from(g) + ((carried[1] + 8) >> 4)).clamp(0, 255),
                    (i32::from(b) + ((carried[2] + 8) >> 4)).clamp(0, 255),
                ];
                let index = self.mapper.index(adjusted.map(|v| v as u8));
                self.data[i] = index;
                self.account([r, g, b], index);
                let chosen = self.mapper.color(index);
                for k in 0..3 {
                    let error = adjusted[k] - chosen[k];
                    current[x + 2][k] += error * 7;
                    next[x][k] += error * 3;
                    next[x + 1][k] += error * 5;
                    next[x + 2][k] += error;
                }
            }
            std::mem::swap(&mut current, &mut next);
            next.fill([0; 3]);
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------------------------
// On a document
// ---------------------------------------------------------------------------------------------

/// The command that quantizes every cell of `doc` (hidden and locked layers included) in a single
/// step, and what it does. Checks the layers and the limit of the cells **before** reserving the
/// result.
pub fn quantize_command(
    doc: &Document,
    options: &QuantizeOptions,
) -> Result<(Command, QuantizeStats), QuantizeError> {
    let source = doc.mode();
    if !matches!(source, Mode::Rgba | Mode::GrayAlpha) {
        return Err(QuantizeError::SourceMode(source));
    }
    for layer in doc.layers() {
        if layer.opacity() != 255 {
            return Err(QuantizeError::LayerOpacity);
        }
        if layer.blend() != BlendMode::Normal {
            return Err(QuantizeError::LayerBlend);
        }
    }
    let cels: Vec<Option<&Image>> = doc
        .layers()
        .iter()
        .map(|layer| doc.image(layer.cel()))
        .collect();
    let per_cell = (doc.width() as usize).saturating_mul(doc.height() as usize);
    let needed = per_cell.saturating_mul(cels.iter().flatten().count());
    if needed > doc.cells_limit() && needed > doc.materialized_bytes() {
        return Err(QuantizeError::TooLarge {
            needed,
            limit: doc.cells_limit(),
        });
    }
    let quantized = quantize_cels(&cels, (doc.width(), doc.height()), source, options)?;
    Ok((
        Command::QuantizeAll {
            palette: quantized.palette,
            images: quantized.images,
            based_on: doc.state_id(),
        },
        quantized.stats,
    ))
}
