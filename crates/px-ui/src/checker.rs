//! Damero de transparencia del lienzo, sin tipos de egui (specs/hito-2-damero-lienzo.md).
//!
//! La casilla mide `8·n` píxeles físicos (`n = text_scale(ppp)`) con cualquier zoom, y el damero
//! está anclado a la esquina de la imagen: la casilla `(0, 0)` es A. El lienzo lo pinta como un
//! solo cuadrilátero con una textura de 2×2 repetida ([`quad`]); [`cells`] enumera las casillas
//! una a una y es el respaldo por malla (`PIXOKO_DAMERO_MALLA=1`) y el oráculo de las pruebas.

use px_core::Rgba;

use crate::palette_grid::{CHECK_A, CHECK_B};
use crate::view::{ScreenRect, Size};

/// Lado de una casilla, en píxeles de interfaz `u`.
pub const CELL_U: i64 = 8;

/// Variable de entorno de pruebas: `PIXOKO_DAMERO_MALLA=1` pinta el damero con una malla de
/// casillas en vez de la textura repetida, para comparar los dos caminos sin recompilar.
pub const ENV_MESH: &str = "PIXOKO_DAMERO_MALLA";

/// El respaldo por malla está activo solo con el valor `1` de [`ENV_MESH`].
pub fn mesh_fallback_enabled(value: Option<&str>) -> bool {
    value == Some("1")
}

/// Lado de la casilla en píxeles físicos para el factor de texto `n` (con `n = 0` se toma 1).
pub fn cell_px(n: u32) -> i64 {
    CELL_U * i64::from(n.max(1))
}

/// Uno de los dos tonos del damero: `A` es [`CHECK_A`] y `B` es [`CHECK_B`], las mismas
/// constantes que las muestras transparentes del panel de paletas.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shade {
    A,
    B,
}

impl Shade {
    pub fn color(self) -> Rgba {
        match self {
            Shade::A => CHECK_A,
            Shade::B => CHECK_B,
        }
    }
}

/// Tono del píxel físico `(x, y)` con la esquina de la imagen en `origin` y casillas de `cell`
/// píxeles: A si `floor((x−ox)/cell) + floor((y−oy)/cell)` es par, B si es impar. Con `cell < 1`
/// se toma 1.
pub fn shade_at(x: i64, y: i64, origin: (i64, i64), cell: i64) -> Shade {
    let cell = i128::from(cell.max(1));
    let col = (i128::from(x) - i128::from(origin.0)).div_euclid(cell);
    let row = (i128::from(y) - i128::from(origin.1)).div_euclid(cell);
    if (col + row).rem_euclid(2) == 0 {
        Shade::A
    } else {
        Shade::B
    }
}

/// Los píxeles de la textura de 2×2 del damero, por filas: `[A, B, B, A]`. Con `NEAREST` y
/// envoltura `Repeat`, un texel cubre una casilla.
pub fn texels() -> [Rgba; 4] {
    [
        Shade::A.color(),
        Shade::B.color(),
        Shade::B.color(),
        Shade::A.color(),
    ]
}

/// `image ∩ (0, 0, viewport.width, viewport.height)`, o `None` si queda vacío: la parte de la
/// imagen que cae en el lienzo.
pub fn clip(image: ScreenRect, viewport: Size) -> Option<ScreenRect> {
    let x0 = image.x.max(0);
    let y0 = image.y.max(0);
    let x1 = image
        .x
        .saturating_add(image.width)
        .min(i64::from(viewport.width));
    let y1 = image
        .y
        .saturating_add(image.height)
        .min(i64::from(viewport.height));
    (x1 > x0 && y1 > y0).then_some(ScreenRect {
        x: x0,
        y: y0,
        width: x1 - x0,
        height: y1 - y0,
    })
}

/// Un cuadrilátero con la textura de 2×2 repetida: el rectángulo y sus coordenadas de textura
/// `[u0, v0, u1, v1]`, medidas en **períodos** (dos casillas por período).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CheckerQuad {
    pub rect: ScreenRect,
    pub uv: [f32; 4],
}

/// Cuadrilátero que cubre `area` con el damero anclado en `origin`. La fase se calcula con
/// enteros (`rem_euclid` del desplazamiento respecto a `origin` en un período de `2·cell`), así
/// que `u0` y `v0` caen en `[0, 1)` y no pierden precisión con áreas ni orígenes lejanos. `None`
/// si `area` está vacía o `cell < 1`.
pub fn quad(area: ScreenRect, origin: (i64, i64), cell: i64) -> Option<CheckerQuad> {
    if area.width <= 0 || area.height <= 0 || cell < 1 {
        return None;
    }
    let period = 2 * i128::from(cell);
    let phase = |pos: i64, origin: i64| (i128::from(pos) - i128::from(origin)).rem_euclid(period);
    let (px, py) = (phase(area.x, origin.0), phase(area.y, origin.1));
    let p = period as f64;
    let (u0, v0) = (px as f64 / p, py as f64 / p);
    let (u1, v1) = (u0 + area.width as f64 / p, v0 + area.height as f64 / p);
    Some(CheckerQuad {
        rect: area,
        uv: [u0 as f32, v0 as f32, u1 as f32, v1 as f32],
    })
}

/// Una casilla del damero recortada a un área.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    pub rect: ScreenRect,
    pub shade: Shade,
}

/// Las casillas que cortan `area`, por filas, recortadas a `area` (las de los bordes salen
/// parciales). Vacío si `area` está vacía o `cell < 1`.
pub fn cells(area: ScreenRect, origin: (i64, i64), cell: i64) -> impl Iterator<Item = Cell> {
    let valid = area.width > 0 && area.height > 0 && cell >= 1;
    let step = i128::from(cell.max(1));
    let index = |pos: i64, origin: i64| (i128::from(pos) - i128::from(origin)).div_euclid(step);
    // Rangos inclusivos de columnas y filas; vacíos con `area` inválida.
    let (c0, c1, r0, r1) = if valid {
        (
            index(area.x, origin.0),
            index(area.x.saturating_add(area.width) - 1, origin.0),
            index(area.y, origin.1),
            index(area.y.saturating_add(area.height) - 1, origin.1),
        )
    } else {
        (0, -1, 0, -1)
    };
    let span = move |start: i64, len: i64, origin: i64, i: i128| {
        let lo = (i128::from(origin) + i * step).max(i128::from(start));
        let hi = (i128::from(origin) + (i + 1) * step).min(i128::from(start) + i128::from(len));
        (lo as i64, (hi - lo) as i64)
    };
    (r0..=r1).flat_map(move |row| {
        (c0..=c1).map(move |col| {
            let (x, width) = span(area.x, area.width, origin.0, col);
            let (y, height) = span(area.y, area.height, origin.1, row);
            Cell {
                rect: ScreenRect {
                    x,
                    y,
                    width,
                    height,
                },
                shade: if (col + row).rem_euclid(2) == 0 {
                    Shade::A
                } else {
                    Shade::B
                },
            }
        })
    })
}
