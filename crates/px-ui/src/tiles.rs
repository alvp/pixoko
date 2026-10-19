//! Baldosas: la imagen se reparte en texturas de a lo sumo `TILE_SIDE`² (specs/hito-2-zoom-baldosas.md,
//! decisión 5). Sin tipos de egui salvo el `Color32` del asignador: la geometría es entera.

use egui::Color32;

use crate::view::{ScreenRect, Size, View};

/// Lado de baldosa por defecto.
pub const TILE_SIDE: usize = 2048;

/// Lado de los bloques de la región sucia y de la superposición.
pub const BLOCK: u32 = 64;

/// Lado de baldosa para un contexto: la mayor potencia de dos ≤ `min(TILE_SIDE, max)`, como
/// mínimo 64. Siempre múltiplo de 64 (el mayor factor de alejamiento) y ≤ lo que dé el contexto.
pub fn tile_side(max_texture_side: usize) -> usize {
    let cap = max_texture_side.clamp(BLOCK as usize, TILE_SIDE);
    1 << (usize::BITS - 1 - cap.leading_zeros())
}

/// Rectángulo en píxeles del bloque `(bx, by)` de una imagen de `size`, recortado a ella.
pub fn block_rect(bx: u32, by: u32, size: Size) -> TileRect {
    let x = bx * BLOCK;
    let y = by * BLOCK;
    TileRect {
        x,
        y,
        width: BLOCK.min(size.width.saturating_sub(x)),
        height: BLOCK.min(size.height.saturating_sub(y)),
    }
}

/// Rectángulo en coordenadas de píxel de imagen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TileRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl TileRect {
    /// Primera columna fuera del rectángulo.
    pub fn right(&self) -> u64 {
        u64::from(self.x) + u64::from(self.width)
    }

    /// Primera fila fuera del rectángulo.
    pub fn bottom(&self) -> u64 {
        u64::from(self.y) + u64::from(self.height)
    }

    /// Intersección con otro rectángulo, si tiene área.
    pub fn intersect(&self, other: &TileRect) -> Option<TileRect> {
        let x0 = self.x.max(other.x);
        let y0 = self.y.max(other.y);
        let x1 = self.right().min(other.right());
        let y1 = self.bottom().min(other.bottom());
        (u64::from(x0) < x1 && u64::from(y0) < y1).then(|| TileRect {
            x: x0,
            y: y0,
            width: (x1 - u64::from(x0)) as u32,
            height: (y1 - u64::from(y0)) as u32,
        })
    }
}

/// Reparto de una imagen en baldosas de `side`, por filas (`índice = fila · cols + col`). Las
/// del borde son menores.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tiling {
    size: Size,
    side: usize,
    cols: usize,
    rows: usize,
}

impl Tiling {
    /// Cualquier `side ≥ 1` (con 0 se toma 1).
    pub fn new(size: Size, side: usize) -> Tiling {
        let side = side.max(1);
        Tiling {
            size,
            side,
            cols: (size.width as usize).div_ceil(side),
            rows: (size.height as usize).div_ceil(side),
        }
    }

    pub fn size(&self) -> Size {
        self.size
    }

    pub fn side(&self) -> usize {
        self.side
    }

    pub fn cols(&self) -> usize {
        self.cols
    }

    pub fn rows(&self) -> usize {
        self.rows
    }

    /// Número de baldosas.
    pub fn len(&self) -> usize {
        self.cols * self.rows
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Rectángulo de la baldosa `i` en la imagen, recortado en el borde.
    pub fn rect(&self, i: usize) -> TileRect {
        let (col, row) = (i % self.cols.max(1), i / self.cols.max(1));
        let axis = |index: usize, dim: u32| {
            let start = (index * self.side).min(dim as usize);
            let end = ((index + 1) * self.side).min(dim as usize);
            (start as u32, (end - start) as u32)
        };
        let (x, width) = axis(col, self.size.width);
        let (y, height) = axis(row, self.size.height);
        TileRect {
            x,
            y,
            width,
            height,
        }
    }

    /// Cuadrilátero de la baldosa en pantalla: `[o + extent(a), o + extent(b))` por eje. Las
    /// baldosas contiguas no se solapan ni dejan huecos y cubren exactamente `View::image_rect`.
    pub fn screen_rect(&self, i: usize, view: &View) -> ScreenRect {
        let rect = self.rect(i);
        let zoom = view.zoom();
        let (ox, oy) = view.origin();
        let axis = |origin: i64, start: u32, len: u32| {
            let a = zoom.extent(i64::from(start));
            let b = zoom.extent(i64::from(start) + i64::from(len));
            (origin.saturating_add(a), b - a)
        };
        let (x, width) = axis(ox, rect.x, rect.width);
        let (y, height) = axis(oy, rect.y, rect.height);
        ScreenRect {
            x,
            y,
            width,
            height,
        }
    }

    /// `[u0, v0, u1, v1]` del cuadrilátero. Con zoom ≥ ×1, de 0 a 1. A 1/K el píxel de pantalla
    /// `s` (desde el origen de la imagen) muestra el texel local `s·K − a`: el uv va desplazado
    /// `0,5 − K/2` texels en el borde izquierdo del píxel, de modo que el centro de cada píxel de
    /// pantalla cae en el centro de un texel y no en un borde (decisión 3).
    pub fn uv(&self, i: usize, view: &View) -> [f32; 4] {
        let Some(k) = view.zoom().shrink_factor() else {
            return [0.0, 0.0, 1.0, 1.0];
        };
        let rect = self.rect(i);
        let quad = self.screen_rect(i, view);
        let zoom = view.zoom();
        let (ox, oy) = view.origin();
        let k = i64::from(k);
        let axis = |origin: i64, quad_start: i64, quad_len: i64, start: u32, texels: u32| {
            // Píxel de pantalla del borde izquierdo, contado desde el origen de la imagen.
            let s0 = quad_start - origin;
            debug_assert_eq!(s0, zoom.extent(i64::from(start)));
            let off = 0.5 - k as f64 / 2.0;
            let local = |s: i64| ((s * k - i64::from(start)) as f64 + off) / f64::from(texels);
            (local(s0) as f32, local(s0 + quad_len) as f32)
        };
        let (u0, u1) = axis(ox, quad.x, quad.width, rect.x, rect.width);
        let (v0, v1) = axis(oy, quad.y, quad.height, rect.y, rect.height);
        [u0, v0, u1, v1]
    }

    /// El cuadrilátero corta el lienzo de `viewport` con área positiva.
    pub fn visible(&self, i: usize, view: &View, viewport: Size) -> bool {
        let quad = self.screen_rect(i, view);
        let x1 = quad
            .x
            .saturating_add(quad.width)
            .min(i64::from(viewport.width));
        let y1 = quad
            .y
            .saturating_add(quad.height)
            .min(i64::from(viewport.height));
        quad.width > 0 && quad.height > 0 && x1 > quad.x.max(0) && y1 > quad.y.max(0)
    }
}

/// Reserva de la memoria de un búfer de baldosa, tras un trait para probar el fallo sin agotarla.
pub trait TileAlloc {
    /// Un `Vec` vacío con capacidad `len`, o `None` si no hay memoria.
    fn pixels(&mut self, len: usize) -> Option<Vec<Color32>>;
}

/// La reserva real: `try_reserve_exact`.
pub struct SystemTileAlloc;

impl TileAlloc for SystemTileAlloc {
    fn pixels(&mut self, len: usize) -> Option<Vec<Color32>> {
        let mut pixels = Vec::new();
        pixels.try_reserve_exact(len).ok()?;
        Some(pixels)
    }
}
