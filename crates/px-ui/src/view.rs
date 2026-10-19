//! Modelo de vista del lienzo: zoom, desplazamiento y conversión entre pantalla e imagen
//! (specs/hito-2-ventana.md, «Modelo de vista»).
//!
//! No usa ningún tipo de una crate gráfica: solo enteros. Las coordenadas de pantalla son
//! píxeles físicos en `i64`, de modo que cada píxel de imagen ocupa exactamente `zoom × zoom`
//! píxeles de pantalla (para zooms enteros) y no hay desbordamiento con imágenes grandes.

use std::cmp::Ordering;
use std::fmt;

/// Mínimo de píxeles de la imagen que `View::clamp` deja visibles por eje.
pub const MIN_VISIBLE: u32 = 16;

/// Nivel de zoom.
///
/// La representación es privada (una fracción `num / den`). La escalera es 1/64, 1/32, …, 1/2,
/// ×1, ×2, …, ×64 (specs/hito-2-zoom-baldosas.md, decisión 2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Zoom {
    num: u32,
    den: u32,
}

impl Zoom {
    pub const ONE: Zoom = Zoom { num: 1, den: 1 };
    /// Zoom mínimo: 1/64 (el mayor factor de alejamiento).
    pub const MIN: Zoom = Zoom { num: 1, den: 64 };
    /// Zoom máximo: 64.
    pub const MAX: Zoom = Zoom { num: 64, den: 1 };

    /// `Some` solo con `1 <= n <= 64`.
    pub fn from_int(n: u32) -> Option<Zoom> {
        (1..=Zoom::MAX.num)
            .contains(&n)
            .then_some(Zoom { num: n, den: 1 })
    }

    /// `Some(1/k)` solo con `k` potencia de dos de 2 a 64.
    pub fn shrink(k: u32) -> Option<Zoom> {
        (k.is_power_of_two() && (2..=Zoom::MIN.den).contains(&k)).then_some(Zoom { num: 1, den: k })
    }

    /// Los 70 niveles, de menor a mayor.
    pub fn levels() -> impl DoubleEndedIterator<Item = Zoom> + Clone {
        let shrinks = (1..=6u32).rev().map(|e| Zoom {
            num: 1,
            den: 1 << e,
        });
        let integers = (1..=Zoom::MAX.num).map(|n| Zoom { num: n, den: 1 });
        shrinks.chain(integers)
    }

    /// El factor `K` de un nivel `1/K` (de alejamiento), o `None` con zoom ≥ ×1: a ese nivel el
    /// píxel de pantalla `s` muestra el píxel de imagen `s·K`.
    pub fn shrink_factor(self) -> Option<u32> {
        (self.den > 1).then_some(self.den)
    }

    /// Nivel siguiente (+1 en la escalera); satura en [`Zoom::MAX`].
    pub fn zoom_in(self) -> Zoom {
        if self >= Zoom::MAX {
            Zoom::MAX
        } else if self.den > 2 {
            Zoom {
                num: 1,
                den: self.den / 2,
            }
        } else if self.den == 2 {
            Zoom::ONE
        } else {
            Zoom {
                num: self.num + 1,
                den: 1,
            }
        }
    }

    /// Nivel anterior (-1 en la escalera); satura en [`Zoom::MIN`].
    pub fn zoom_out(self) -> Zoom {
        if self <= Zoom::MIN {
            Zoom::MIN
        } else if self.den > 1 {
            Zoom {
                num: 1,
                den: self.den * 2,
            }
        } else if self.num == 1 {
            Zoom { num: 1, den: 2 }
        } else {
            Zoom {
                num: self.num - 1,
                den: 1,
            }
        }
    }

    /// Longitud en pantalla de `n` píxeles de imagen: `ceil(n · zoom)`. Con zoom ≥ ×1 es
    /// [`Zoom::scale`]; a 1/K el último píxel de la rejilla también se ve.
    pub fn extent(self, n: i64) -> i64 {
        ceil_ratio(i128::from(n) * i128::from(self.num), i128::from(self.den))
    }

    /// Imagen → pantalla: `floor(n · zoom)`.
    pub fn scale(self, n: i64) -> i64 {
        floor_ratio(i128::from(n) * i128::from(self.num), i128::from(self.den))
    }

    /// Pantalla → imagen: `floor(n / zoom)`.
    pub fn unscale(self, n: i64) -> i64 {
        floor_ratio(i128::from(n) * i128::from(self.den), i128::from(self.num))
    }

    /// `floor(n · to / self)`: lleva una longitud de pantalla de este zoom al zoom `to`.
    pub fn rescale(self, n: i64, to: Zoom) -> i64 {
        floor_ratio(
            i128::from(n) * i128::from(to.num) * i128::from(self.den),
            i128::from(to.den) * i128::from(self.num),
        )
    }
}

/// `ceil(num / den)` con `den > 0`, saturado a `i64`.
fn ceil_ratio(num: i128, den: i128) -> i64 {
    let q = -((-num).div_euclid(den));
    i64::try_from(q).unwrap_or(if q < 0 { i64::MIN } else { i64::MAX })
}

/// `floor(num / den)` con `den > 0`, saturado a `i64`.
fn floor_ratio(num: i128, den: i128) -> i64 {
    let q = num.div_euclid(den);
    i64::try_from(q).unwrap_or(if q < 0 { i64::MIN } else { i64::MAX })
}

impl Default for Zoom {
    fn default() -> Zoom {
        Zoom::ONE
    }
}

impl PartialOrd for Zoom {
    fn partial_cmp(&self, other: &Zoom) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Zoom {
    fn cmp(&self, other: &Zoom) -> Ordering {
        (u64::from(self.num) * u64::from(other.den))
            .cmp(&(u64::from(other.num) * u64::from(self.den)))
    }
}

impl fmt::Display for Zoom {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.den == 1 {
            write!(f, "×{}", self.num)
        } else {
            write!(f, "×{}/{}", self.num, self.den)
        }
    }
}

/// Tamaño en píxeles (de una imagen o del lienzo).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Size {
    pub width: u32,
    pub height: u32,
}

impl Size {
    pub fn new(width: u32, height: u32) -> Size {
        Size { width, height }
    }
}

/// Operación de zoom pedida desde el menú, la columna o el teclado
/// (specs/hito-2-menu-herramientas.md).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewOp {
    /// Un nivel más, anclado en el centro del lienzo.
    ZoomIn,
    /// Un nivel menos, anclado en el centro del lienzo.
    ZoomOut,
    /// Mayor zoom entero con la imagen entera visible, centrada.
    Fit,
    /// Zoom ×1 anclado en el centro del lienzo.
    Actual,
}

/// Rectángulo en coordenadas de pantalla.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScreenRect {
    pub x: i64,
    pub y: i64,
    pub width: i64,
    pub height: i64,
}

/// Zoom y posición de la imagen en el lienzo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct View {
    zoom: Zoom,
    /// Esquina superior izquierda de la imagen respecto a la del lienzo.
    origin: (i64, i64),
}

impl View {
    /// Mayor nivel de la escalera con la imagen entera visible, centrada. Si ni ×1 cabe, el mayor
    /// nivel `1/K` que quepa y, si ni 1/64 cabe, el mínimo.
    pub fn fit(viewport: Size, image: Size) -> View {
        let fits = |dim: u32, view: u32| view.checked_div(dim).unwrap_or(u32::MAX);
        let best = fits(image.width, viewport.width).min(fits(image.height, viewport.height));
        let zoom = if best >= 1 {
            Zoom::from_int(best.min(Zoom::MAX.num)).unwrap_or(Zoom::ONE)
        } else {
            Zoom::levels()
                .rev()
                .find(|zoom| {
                    zoom.extent(i64::from(image.width)) <= i64::from(viewport.width)
                        && zoom.extent(i64::from(image.height)) <= i64::from(viewport.height)
                })
                .unwrap_or(Zoom::MIN)
        };
        let center =
            |view: u32, dim: u32| (i64::from(view) - zoom.extent(i64::from(dim))).div_euclid(2);
        View {
            zoom,
            origin: (
                center(viewport.width, image.width),
                center(viewport.height, image.height),
            ),
        }
    }

    pub fn zoom(&self) -> Zoom {
        self.zoom
    }

    pub fn origin(&self) -> (i64, i64) {
        self.origin
    }

    /// Cambia el zoom dejando fijo el píxel que hay bajo `anchor`. No hace `clamp`.
    pub fn set_zoom(&mut self, zoom: Zoom, anchor: (i64, i64)) {
        let axis = |anchor: i64, origin: i64| {
            let offset = anchor.saturating_sub(origin);
            anchor.saturating_sub(self.zoom.rescale(offset, zoom))
        };
        self.origin = (axis(anchor.0, self.origin.0), axis(anchor.1, self.origin.1));
        self.zoom = zoom;
    }

    pub fn zoom_in(&mut self, anchor: (i64, i64)) {
        self.set_zoom(self.zoom.zoom_in(), anchor);
    }

    pub fn zoom_out(&mut self, anchor: (i64, i64)) {
        self.set_zoom(self.zoom.zoom_out(), anchor);
    }

    /// Aplica una [`ViewOp`] sobre un lienzo de `viewport` píxeles y una imagen de `image`, y deja
    /// la vista con `clamp`. `ZoomIn`, `ZoomOut` y `Actual` anclan en el centro del lienzo.
    pub fn apply_op(&mut self, op: ViewOp, viewport: Size, image: Size) {
        let center = (
            i64::from(viewport.width) / 2,
            i64::from(viewport.height) / 2,
        );
        match op {
            ViewOp::ZoomIn => self.zoom_in(center),
            ViewOp::ZoomOut => self.zoom_out(center),
            ViewOp::Actual => self.set_zoom(Zoom::ONE, center),
            ViewOp::Fit => *self = View::fit(viewport, image),
        }
        self.clamp(viewport, image);
    }

    /// Desplaza el origen y hace `clamp`.
    pub fn pan(&mut self, dx: i64, dy: i64, viewport: Size, image: Size) {
        self.origin = (
            self.origin.0.saturating_add(dx),
            self.origin.1.saturating_add(dy),
        );
        self.clamp(viewport, image);
    }

    /// Deja visibles al menos `min(MIN_VISIBLE, extensión · zoom, lienzo)` píxeles de la imagen
    /// en cada eje; si ese mínimo es 0 (lienzo o imagen vacíos) no mueve la vista.
    pub fn clamp(&mut self, viewport: Size, image: Size) {
        let axis = |origin: i64, view: u32, dim: u32| {
            let extent = self.zoom.extent(i64::from(dim));
            let view = i64::from(view);
            let keep = i64::from(MIN_VISIBLE).min(extent).min(view);
            if keep == 0 {
                // Lienzo o imagen sin píxeles: no hay nada que mantener visible.
                return origin;
            }
            origin.clamp(keep - extent, view - keep)
        };
        self.origin = (
            axis(self.origin.0, viewport.width, image.width),
            axis(self.origin.1, viewport.height, image.height),
        );
    }

    /// Rectángulo que ocupa la imagen entera en pantalla.
    pub fn image_rect(&self, image: Size) -> ScreenRect {
        ScreenRect {
            x: self.origin.0,
            y: self.origin.1,
            width: self.zoom.extent(i64::from(image.width)),
            height: self.zoom.extent(i64::from(image.height)),
        }
    }

    /// Píxel de la imagen bajo el punto de pantalla `p`, o `None` si cae fuera.
    pub fn screen_to_image(&self, p: (i64, i64), image: Size) -> Option<(u32, u32)> {
        let axis = |p: i64, origin: i64, dim: u32| {
            let delta = p.saturating_sub(origin);
            if delta < 0 {
                return None;
            }
            let index = self.zoom.unscale(delta);
            if index >= i64::from(dim) {
                return None;
            }
            u32::try_from(index).ok()
        };
        Some((
            axis(p.0, self.origin.0, image.width)?,
            axis(p.1, self.origin.1, image.height)?,
        ))
    }

    /// Como [`View::screen_to_image`], pero sin `None` fuera de la imagen: coordenadas con
    /// signo (`floor` de la división), saturadas a `i32`. Sirve para los trazos, que pueden
    /// salir del lienzo y volver. `image` no cambia el resultado: se acepta por simetría.
    pub fn screen_to_image_i32(&self, p: (i64, i64), _image: Size) -> (i32, i32) {
        let axis = |p: i64, origin: i64| {
            let index = self.zoom.unscale(p.saturating_sub(origin));
            i32::try_from(index).unwrap_or(if index < 0 { i32::MIN } else { i32::MAX })
        };
        (axis(p.0, self.origin.0), axis(p.1, self.origin.1))
    }

    /// Rectángulo `(mínimo, máximo)` en pantalla del bloque del píxel `(x, y)`: `zoom × zoom`
    /// píxeles con zoom ≥ ×1; a 1/K, 1×1 píxel físico si `x` e `y` son múltiplos de `K` (los
    /// únicos píxeles que se ven) y `None` si no lo son.
    pub fn pixel_block(&self, x: u32, y: u32) -> Option<((i64, i64), (i64, i64))> {
        match self.zoom.shrink_factor() {
            None => Some((
                self.image_to_screen(x, y),
                self.image_to_screen(x.saturating_add(1), y.saturating_add(1)),
            )),
            Some(k) if x.is_multiple_of(k) && y.is_multiple_of(k) => {
                let min = self.image_to_screen(x, y);
                Some((min, (min.0.saturating_add(1), min.1.saturating_add(1))))
            }
            Some(_) => None,
        }
    }

    /// Esquina superior izquierda, en pantalla, del bloque del píxel `(x, y)`.
    pub fn image_to_screen(&self, x: u32, y: u32) -> (i64, i64) {
        (
            self.origin.0.saturating_add(self.zoom.scale(i64::from(x))),
            self.origin.1.saturating_add(self.zoom.scale(i64::from(y))),
        )
    }
}
