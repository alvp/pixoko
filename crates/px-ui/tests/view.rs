//! Tests del modelo de vista de `px-ui::view` (specs/hito-2-ventana.md, «Modelo de vista»).
//!
//! Se prueban las 8 propiedades de la spec:
//!
//! 1. Niveles: `from_int`, `zoom_in`/`zoom_out` con saturación, orden y `Display`.
//! 2. `scale`, `unscale` y `rescale` frente a un modelo con `i128` y `div_euclid`.
//! 3. Ida y vuelta exacta pantalla <-> imagen para TODOS los zooms enteros de 1 a 64 (los
//!    fraccionarios 1/2 a 1/64, por rejilla, son de `zoom_ladder.rs`).
//! 4. Teselado sin huecos ni solapes y `image_rect`.
//! 5. Ancla: el bloque bajo el ancla se conserva al cambiar de zoom (también fuera de la imagen).
//! 6. `fit`: mayor nivel de la escalera que cabe (entero y, si no cabe a ×1, fraccionario),
//!    imagen centrada, determinista.
//! 7. Clamp: visibilidad mínima, idempotencia, no mueve lo que ya cumple, `pan`.
//! 8. Sin pánico ni desbordamiento con tamaños hasta 2^30 por eje y coordenadas hasta ±2^40.
//!
//! `View` solo se construye con `View::fit`; para orígenes arbitrarios se parte de `fit` y se
//! aplican operaciones (`set_zoom` no hace clamp, `pan` sí). Las imágenes tienen al menos 1x1: la
//! spec no define el comportamiento con una imagen vacía. El viewport sí puede ser de 0 o 1.
//!
//! Los modelos de referencia usan `i128` y `div_euclid`, sin las fórmulas de la implementación.

use std::collections::HashSet;
use std::collections::hash_map::DefaultHasher;
use std::fmt::{Debug, Display};
use std::hash::{Hash, Hasher};

use proptest::collection::vec;
use proptest::prelude::*;
use proptest::test_runner::TestCaseError;
use px_ui::view::{MIN_VISIBLE, ScreenRect, Size, View, Zoom};

// Comprobación en tiempo de compilación de los traits exigidos.
fn assert_zoom_traits<
    T: Clone + Copy + Debug + Display + PartialEq + Eq + PartialOrd + Ord + Hash + Default,
>() {
}
fn assert_plane_traits<T: Clone + Copy + Debug + PartialEq + Eq>() {}

#[test]
fn traits_derived() {
    assert_zoom_traits::<Zoom>();
    assert_plane_traits::<View>();
    assert_plane_traits::<Size>();
    assert_plane_traits::<ScreenRect>();
}

// Utilidades de apoyo.

fn zoom(n: u32) -> Zoom {
    Zoom::from_int(n).unwrap_or_else(|| panic!("el zoom {n} debería existir"))
}

/// Factor entero de un zoom entero (solo para las pruebas que fijan zooms de ×1 a ×64).
fn factor(z: Zoom) -> i64 {
    z.scale(1)
}

/// Fracción `(num, den)` de un nivel de la escalera: `(n, 1)` desde ×1 y `(1, K)` a 1/K.
fn fraction(z: Zoom) -> (i128, i128) {
    if z >= Zoom::ONE {
        (i128::from(z.scale(1)), 1)
    } else {
        (1, i128::from(z.unscale(1)))
    }
}

/// Longitud en pantalla de `n` píxeles de imagen según el modelo: `ceil(n · num / den)`.
fn extension(z: Zoom, n: i128) -> i128 {
    let (num, den) = fraction(z);
    (n * num + den - 1).div_euclid(den)
}

fn tam(width: u32, height: u32) -> Size {
    Size::new(width, height)
}

/// Índice de píxel de imagen en un eje según el modelo: floor((p - origen) · den / num).
fn index_model(p: i64, source_2: i64, z: Zoom) -> i128 {
    let (num, den) = fraction(z);
    ((p as i128 - source_2 as i128) * den).div_euclid(num)
}

/// Modelo de `screen_to_image`: `None` si el bloque cae fuera de la imagen.
fn pixel_model(v: &View, p: (i64, i64), img: Size) -> Option<(u32, u32)> {
    let (ox, oy) = v.origin();
    let ix = index_model(p.0, ox, v.zoom());
    let iy = index_model(p.1, oy, v.zoom());
    if (0..img.width as i128).contains(&ix) && (0..img.height as i128).contains(&iy) {
        Some((ix as u32, iy as u32))
    } else {
        None
    }
}

/// Píxeles de la imagen visibles en un eje: solape de [origen, origen+ext) con [0, viewport).
fn visible_2(source_2: i64, ext: i128, viewport: u32) -> i128 {
    let ini = (source_2 as i128).max(0);
    let fin = (source_2 as i128 + ext).min(viewport as i128);
    (fin - ini).max(0)
}

/// Mínimo que debe quedar visible en un eje: min(MIN_VISIBLE, ext, viewport).
fn minimum_visible(ext: i128, viewport: u32) -> i128 {
    (MIN_VISIBLE as i128).min(ext).min(viewport as i128)
}

fn axis_meets(source_2: i64, ext: i128, viewport: u32) -> bool {
    visible_2(source_2, ext, viewport) >= minimum_visible(ext, viewport)
}

fn meets_visibility(v: &View, vp: Size, img: Size) -> bool {
    let (ox, oy) = v.origin();
    axis_meets(ox, extension(v.zoom(), img.width as i128), vp.width)
        && axis_meets(oy, extension(v.zoom(), img.height as i128), vp.height)
}

/// Operaciones que se encadenan a partir de `View::fit` para obtener vistas con origen y zoom
/// arbitrarios.
#[derive(Clone, Debug)]
enum Op {
    SetZoom(u32, (i64, i64)),
    ZoomIn((i64, i64)),
    ZoomOut((i64, i64)),
    Pan(i64, i64),
    Clamp,
}

fn apply(v: &mut View, op: &Op, vp: Size, img: Size) {
    match op {
        Op::SetZoom(n, a) => v.set_zoom(zoom(*n), *a),
        Op::ZoomIn(a) => v.zoom_in(*a),
        Op::ZoomOut(a) => v.zoom_out(*a),
        Op::Pan(dx, dy) => v.pan(*dx, *dy, vp, img),
        Op::Clamp => v.clamp(vp, img),
    }
}

fn view(vp: Size, img: Size, ops: &[Op]) -> View {
    let mut v = View::fit(vp, img);
    for op in ops {
        apply(&mut v, op, vp, img);
    }
    v
}

// Estrategias.

fn op(range: i64) -> impl Strategy<Value = Op> {
    let anchor = (-range..=range, -range..=range);
    prop_oneof![
        (1u32..=64, anchor.clone()).prop_map(|(n, a)| Op::SetZoom(n, a)),
        anchor.clone().prop_map(Op::ZoomIn),
        anchor.prop_map(Op::ZoomOut),
        (-range..=range, -range..=range).prop_map(|(dx, dy)| Op::Pan(dx, dy)),
        Just(Op::Clamp),
    ]
}

/// Extensión de imagen (al menos 1): pequeña, media o grande.
fn dim_image() -> impl Strategy<Value = u32> {
    prop_oneof![1u32..=40, 1u32..=4000, 1u32..=100_000]
}

/// Extensión de viewport (puede ser 0 o 1).
fn dim_viewport() -> impl Strategy<Value = u32> {
    prop_oneof![Just(0u32), Just(1u32), 0u32..=60, 0u32..=5000]
}

fn dim_image_extreme() -> impl Strategy<Value = u32> {
    prop_oneof![Just(1u32), 1u32..=(1 << 30), Just(1u32 << 30)]
}

fn dim_viewport_extreme() -> impl Strategy<Value = u32> {
    prop_oneof![Just(0u32), Just(1u32), 0u32..=(1 << 30), Just(1u32 << 30)]
}

// Comprobaciones compartidas (devuelven `Result` para usarlas dentro de `proptest!`).

/// Propiedades 3 y 4 sobre una vista: ida y vuelta, esquinas, vecinos, puntos dentro y fuera.
/// `dev` elige puntos dentro de la imagen y desplazamientos fuera; `far` es un punto
/// arbitrario que se contrasta con el modelo.
fn check_geometry(
    v: &View,
    img: Size,
    pix: (u32, u32),
    dev: (u64, u64),
    far: (i64, i64),
) -> Result<(), TestCaseError> {
    let z = factor(v.zoom());
    let (ox, oy) = v.origin();
    let rect = v.image_rect(img);
    let (w, h) = (img.width, img.height);

    // Propiedad 4: image_rect es (origen, scale(ancho), scale(alto)).
    prop_assert_eq!(
        rect,
        ScreenRect {
            x: ox,
            y: oy,
            width: w as i64 * z,
            height: h as i64 * z
        }
    );

    // Propiedad 3, primera viñeta: ida y vuelta de píxeles (incluidos los de los bordes).
    let pixels_2 = [
        (pix.0 % w, pix.1 % h),
        (0, 0),
        (w - 1, h - 1),
        (w - 1, 0),
        (0, h - 1),
    ];
    for (x, y) in pixels_2 {
        let c = v.image_to_screen(x, y);
        prop_assert_eq!(c, (ox + x as i64 * z, oy + y as i64 * z));
        prop_assert_eq!(v.screen_to_image(c, img), Some((x, y)));
        for (ex, ey) in [(z - 1, 0), (0, z - 1), (z - 1, z - 1)] {
            prop_assert_eq!(v.screen_to_image((c.0 + ex, c.1 + ey), img), Some((x, y)));
        }
        // Un píxel de pantalla más allá del bloque pertenece al vecino, o a nadie en el borde.
        let right = if x + 1 < w { Some((x + 1, y)) } else { None };
        let left = if x > 0 { Some((x - 1, y)) } else { None };
        let below = if y + 1 < h { Some((x, y + 1)) } else { None };
        let top = if y > 0 { Some((x, y - 1)) } else { None };
        prop_assert_eq!(v.screen_to_image((c.0 + z, c.1), img), right);
        prop_assert_eq!(v.screen_to_image((c.0 - 1, c.1), img), left);
        prop_assert_eq!(v.screen_to_image((c.0, c.1 + z), img), below);
        prop_assert_eq!(v.screen_to_image((c.0, c.1 - 1), img), top);

        // Propiedad 4: teselado sin huecos ni solapes.
        prop_assert_eq!(v.image_to_screen(x + 1, y), (c.0 + z, c.1));
        prop_assert_eq!(v.image_to_screen(x, y + 1), (c.0, c.1 + z));
    }

    // Propiedad 3, segunda viñeta: un punto dentro de image_rect.
    let p = (
        rect.x + (dev.0 % rect.width as u64) as i64,
        rect.y + (dev.1 % rect.height as u64) as i64,
    );
    let (ix, iy) = match v.screen_to_image(p, img) {
        Some(px) => px,
        None => {
            return Err(TestCaseError::fail(format!(
                "el punto {p:?} está dentro de {rect:?} y dio None"
            )));
        }
    };
    let c = v.image_to_screen(ix, iy);
    prop_assert!(c.0 <= p.0 && p.0 < c.0 + z, "x: {c:?} <= {p:?} < c + {z}");
    prop_assert!(c.1 <= p.1 && p.1 < c.1 + z, "y: {c:?} <= {p:?} < c + {z}");
    prop_assert_eq!(v.screen_to_image(c, img), Some((ix, iy)));
    prop_assert_eq!(Some((ix, iy)), pixel_model(v, p, img));

    // Propiedad 3, tercera viñeta: fuera de image_rect siempre None.
    let a = (dev.0 % 1000) as i64;
    let bx = (dev.0 % rect.width as u64) as i64;
    let by = (dev.1 % rect.height as u64) as i64;
    let right = rect.x + rect.width;
    let below_2 = rect.y + rect.height;
    let outside = [
        (rect.x - 1, rect.y + by),
        (rect.x - 1 - a, rect.y + by),
        (right, rect.y + by),
        (right + a, rect.y + by),
        (rect.x + bx, rect.y - 1),
        (rect.x + bx, rect.y - 1 - a),
        (rect.x + bx, below_2),
        (rect.x + bx, below_2 + a),
        (rect.x - 1 - a, rect.y - 1 - a),
        (right + a, below_2 + a),
        (rect.x - 1, below_2),
        (right, rect.y - 1),
    ];
    for q in outside {
        prop_assert_eq!(
            v.screen_to_image(q, img),
            None,
            "punto {:?} en {:?}",
            q,
            rect
        );
    }

    // Un punto arbitrario (posiblemente muy lejano) coincide con el modelo.
    prop_assert_eq!(v.screen_to_image(far, img), pixel_model(v, far, img));
    Ok(())
}

/// Propiedad 5: el píxel bajo el ancla se conserva (con el modelo en `i128`). Con zoom ≥ ×1 es
/// exacto, como siempre; si el nivel nuevo es `1/K`, el píxel sube a la rejilla del nivel
/// (`floor(i / K) · K`; specs/hito-2-zoom-baldosas.md, decisión 4). Con `K = 1` es la misma
/// igualdad de antes.
fn check_anchor(
    before: &View,
    after_2: &View,
    anchor: (i64, i64),
    img: Size,
) -> Result<(), TestCaseError> {
    let grid = if after_2.zoom() < Zoom::ONE {
        fraction(after_2.zoom()).1
    } else {
        1
    };
    let expected_2 = |i: i128| i.div_euclid(grid) * grid;
    let ix = expected_2(index_model(anchor.0, before.origin().0, before.zoom()));
    let iy = expected_2(index_model(anchor.1, before.origin().1, before.zoom()));
    prop_assert_eq!(
        ix,
        index_model(anchor.0, after_2.origin().0, after_2.zoom()),
        "eje x: ancla {:?}, antes {:?}, después {:?}",
        anchor,
        before,
        after_2
    );
    prop_assert_eq!(
        iy,
        index_model(anchor.1, after_2.origin().1, after_2.zoom()),
        "eje y: ancla {:?}, antes {:?}, después {:?}",
        anchor,
        before,
        after_2
    );
    // Mismo índice implica mismo estado dentro/fuera y mismo píxel (el índice esperado, si cae
    // dentro de la imagen).
    let inside = (0..img.width as i128).contains(&ix) && (0..img.height as i128).contains(&iy);
    prop_assert_eq!(
        after_2.screen_to_image(anchor, img),
        inside.then_some((ix as u32, iy as u32))
    );
    if grid == 1 {
        prop_assert_eq!(
            before.screen_to_image(anchor, img),
            after_2.screen_to_image(anchor, img)
        );
    }
    Ok(())
}

/// Propiedad 5 sobre todos los cambios de zoom posibles desde `before`.
fn check_anchor_at_all_zooms(
    before: &View,
    anchor: (i64, i64),
    img: Size,
) -> Result<(), TestCaseError> {
    for n in 1..=64 {
        let mut v = *before;
        v.set_zoom(zoom(n), anchor);
        prop_assert_eq!(v.zoom(), zoom(n));
        check_anchor(before, &v, anchor, img)?;
    }
    let mut inside = *before;
    inside.zoom_in(anchor);
    prop_assert_eq!(inside.zoom(), before.zoom().zoom_in());
    check_anchor(before, &inside, anchor, img)?;
    let mut outside = *before;
    outside.zoom_out(anchor);
    prop_assert_eq!(outside.zoom(), before.zoom().zoom_out());
    check_anchor(before, &outside, anchor, img)?;
    Ok(())
}

/// Propiedad 6 para un viewport y una imagen.
fn check_fit(vp: Size, img: Size) -> Result<(), TestCaseError> {
    let v = View::fit(vp, img);
    prop_assert_eq!(v, View::fit(vp, img), "fit debe ser determinista");

    // Modelo: el mayor nivel de la escalera con ceil(ancho·z) <= viewport.ancho y
    // ceil(alto·z) <= viewport.alto; si ninguno cabe, el mínimo.
    let fits = |z: Zoom| {
        extension(z, img.width as i128) <= vp.width as i128
            && extension(z, img.height as i128) <= vp.height as i128
    };
    let expected_2 = Zoom::levels().rev().find(|&z| fits(z));
    let z = v.zoom();
    prop_assert_eq!(
        z,
        expected_2.unwrap_or(Zoom::MIN),
        "zoom de fit para {:?} en {:?}",
        img,
        vp
    );

    // Centrada: los márgenes a ambos lados difieren como mucho en 1 px por eje.
    let (ox, oy) = v.origin();
    for (o, ext, viewport) in [
        (ox, extension(z, img.width as i128), vp.width),
        (oy, extension(z, img.height as i128), vp.height),
    ] {
        let left = o as i128;
        let right = viewport as i128 - o as i128 - ext;
        prop_assert!(
            (left - right).abs() <= 1,
            "márgenes {left} y {right} en {v:?}"
        );
        // Si la imagen cabe entera, queda dentro del viewport.
        if expected_2.is_some() {
            prop_assert!(left >= 0 && right >= 0, "imagen fuera en {v:?}");
        }
    }
    Ok(())
}

// Propiedad 1: niveles.

#[test]
fn from_int_only_from_1_to_64() {
    assert!(Zoom::from_int(0).is_none());
    assert!(Zoom::from_int(65).is_none());
    assert!(Zoom::from_int(100).is_none());
    assert!(Zoom::from_int(u32::MAX).is_none());
    for n in 1..=64u32 {
        let z = Zoom::from_int(n).unwrap_or_else(|| panic!("from_int({n}) debería ser Some"));
        assert_eq!(z.scale(1), n as i64, "scale(1) de ×{n}");
    }
}

#[test]
fn constants_of_zoom() {
    // El mínimo es 1/64 (antes ×1): `scale(1)` baja a 0 y `unscale(1)` da 64.
    assert_eq!(Zoom::MIN, Zoom::shrink(64).expect("1/64"));
    assert_eq!(Zoom::MIN.unscale(1), 64);
    assert_eq!(Zoom::ONE.scale(1), 1);
    assert_eq!(Zoom::MIN.scale(1), 0);
    assert_eq!(Zoom::MAX.scale(1), 64);
    assert_eq!(Zoom::from_int(1), Some(Zoom::ONE));
    assert_eq!(Zoom::from_int(64), Some(Zoom::MAX));
    assert_eq!(Zoom::default(), Zoom::ONE);
}

#[test]
fn zoom_in_goes_up_one_by_one_and_saturates() {
    // Los niveles enteros, desde ×1 (el mínimo de antes).
    let mut z = Zoom::ONE;
    for n in 1..64u32 {
        assert_eq!(z, zoom(n));
        z = z.zoom_in();
        assert_eq!(z, zoom(n + 1), "zoom_in de ×{n}");
    }
    assert_eq!(z, Zoom::MAX);
    assert_eq!(Zoom::MAX.zoom_in(), Zoom::MAX);
    assert_eq!(Zoom::MAX.zoom_in().zoom_in(), Zoom::MAX);
}

#[test]
fn zoom_out_goes_down_one_by_one_and_saturates() {
    let mut z = Zoom::MAX;
    for n in (2..=64u32).rev() {
        assert_eq!(z, zoom(n));
        z = z.zoom_out();
        assert_eq!(z, zoom(n - 1), "zoom_out de ×{n}");
    }
    // Los niveles enteros acaban en ×1 (el mínimo de antes); el mínimo de ahora sigue saturando.
    assert_eq!(z, Zoom::ONE);
    assert_eq!(Zoom::MIN.zoom_out(), Zoom::MIN);
    assert_eq!(Zoom::MIN.zoom_out().zoom_out(), Zoom::MIN);
}

#[test]
fn zoom_is_ordered_like_its_integer() {
    for a in 1..=64u32 {
        for b in 1..=64u32 {
            let (za, zb) = (zoom(a), zoom(b));
            assert_eq!(za.cmp(&zb), a.cmp(&b), "cmp de ×{a} y ×{b}");
            assert_eq!(za.partial_cmp(&zb), Some(a.cmp(&b)));
            assert_eq!(za == zb, a == b);
        }
    }
    assert!(Zoom::MIN < Zoom::MAX);
    assert_eq!(Zoom::MIN.max(Zoom::MAX), Zoom::MAX);
}

#[test]
fn zoom_hash_is_consistent_with_eq() {
    let all_2: HashSet<Zoom> = (1..=64).map(zoom).collect();
    assert_eq!(all_2.len(), 64);
    let hash = |z: Zoom| {
        let mut h = DefaultHasher::new();
        z.hash(&mut h);
        h.finish()
    };
    assert_eq!(hash(zoom(5)), hash(zoom(5)));
    assert_eq!(hash(Zoom::ONE), hash(Zoom::default()));
}

#[test]
fn zoom_display_uses_the_times_sign() {
    assert_eq!(zoom(4).to_string(), "×4");
    assert_eq!(zoom(4).to_string(), "\u{00D7}4");
    assert_eq!(format!("{}", Zoom::ONE), "×1");
    assert_eq!(format!("{}", Zoom::MAX), "×64");
    assert_eq!(format!("{}", zoom(12)), "×12");
}

// Propiedad 2: scale, unscale y rescale.

#[test]
fn scale_unscale_rescale_examples() {
    // scale es n·z, también con n negativo.
    assert_eq!(zoom(3).scale(5), 15);
    assert_eq!(zoom(3).scale(0), 0);
    assert_eq!(zoom(3).scale(-5), -15);
    assert_eq!(zoom(64).scale(2), 128);
    assert_eq!(Zoom::ONE.scale(-7), -7);
    // unscale redondea hacia abajo, también con n negativo.
    assert_eq!(zoom(4).unscale(0), 0);
    assert_eq!(zoom(4).unscale(3), 0);
    assert_eq!(zoom(4).unscale(4), 1);
    assert_eq!(zoom(4).unscale(7), 1);
    assert_eq!(zoom(4).unscale(-1), -1);
    assert_eq!(zoom(4).unscale(-4), -1);
    assert_eq!(zoom(4).unscale(-5), -2);
    assert_eq!(Zoom::ONE.unscale(-3), -3);
    assert_eq!(Zoom::MAX.unscale(63), 0);
    assert_eq!(Zoom::MAX.unscale(64), 1);
    assert_eq!(Zoom::MAX.unscale(-1), -1);
    assert_eq!(Zoom::MAX.unscale(-64), -1);
    assert_eq!(Zoom::MAX.unscale(-65), -2);
    // rescale es floor(n·to/z).
    assert_eq!(zoom(2).rescale(5, zoom(4)), 10);
    assert_eq!(zoom(2).rescale(7, zoom(3)), 10); // floor(21/2)
    assert_eq!(zoom(3).rescale(-1, zoom(2)), -1); // floor(-2/3)
    assert_eq!(zoom(3).rescale(-3, zoom(2)), -2);
    assert_eq!(Zoom::MAX.rescale(10, Zoom::ONE), 0); // floor(10/64)
    assert_eq!(Zoom::MAX.rescale(-10, Zoom::ONE), -1); // floor(-10/64)
    assert_eq!(Zoom::ONE.rescale(10, Zoom::MAX), 640);
    assert_eq!(zoom(7).rescale(13, zoom(7)), 13);
}

#[test]
fn scale_unscale_rescale_exhaustive_in_a_small_range() {
    for zi in 1..=64u32 {
        let z = zoom(zi);
        let zz = zi as i128;
        for n in -130..=130i64 {
            assert_eq!(z.scale(n), n * zi as i64, "scale({n}) de ×{zi}");
            assert_eq!(
                z.unscale(n) as i128,
                (n as i128).div_euclid(zz),
                "unscale({n}) de ×{zi}"
            );
            for k in 0..zi as i64 {
                assert_eq!(
                    z.unscale(z.scale(n) + k),
                    n,
                    "unscale(scale({n}) + {k}) de ×{zi}"
                );
            }
            for ti in 1..=64u32 {
                let expected_2 = (n as i128 * ti as i128).div_euclid(zz);
                assert_eq!(
                    z.rescale(n, zoom(ti)) as i128,
                    expected_2,
                    "rescale({n}) de ×{zi} a ×{ti}"
                );
            }
        }
    }
}

proptest! {
    #[test]
    fn scale_and_unscale_with_any_n(
        zi in 1u32..=64,
        n in -(1i64 << 50)..=(1i64 << 50),
        k in 0i64..64,
    ) {
        let z = zoom(zi);
        let k = k % zi as i64;
        prop_assert_eq!(z.scale(n), n * zi as i64);
        prop_assert_eq!(z.unscale(n) as i128, (n as i128).div_euclid(zi as i128));
        prop_assert_eq!(z.unscale(z.scale(n) + k), n);
        // El resto dentro del bloque nunca sale de [0, z).
        let rest_2 = n as i128 - z.unscale(n) as i128 * zi as i128;
        prop_assert!((0..zi as i128).contains(&rest_2));
    }

    #[test]
    fn rescale_with_any_n(
        zi in 1u32..=64,
        ti in 1u32..=64,
        n in -(1i64 << 50)..=(1i64 << 50),
    ) {
        let (z, to) = (zoom(zi), zoom(ti));
        let expected_2 = (n as i128 * ti as i128).div_euclid(zi as i128);
        prop_assert_eq!(z.rescale(n, to) as i128, expected_2);
        // Reescalar al mismo zoom no cambia nada.
        prop_assert_eq!(z.rescale(n, z), n);
    }
}

// Propiedad 3: ida y vuelta pantalla <-> imagen.

#[test]
fn screen_image_with_zoom_1() {
    // Imagen de 64x64 en un viewport de 64x64: zoom 1 y origen (0, 0).
    let img = tam(64, 64);
    let v = View::fit(tam(64, 64), img);
    assert_eq!(v.zoom(), Zoom::ONE);
    assert_eq!(v.origin(), (0, 0));
    assert_eq!(v.screen_to_image((0, 0), img), Some((0, 0)));
    assert_eq!(v.screen_to_image((63, 63), img), Some((63, 63)));
    assert_eq!(v.screen_to_image((10, 20), img), Some((10, 20)));
    assert_eq!(v.screen_to_image((64, 63), img), None);
    assert_eq!(v.screen_to_image((63, 64), img), None);
    assert_eq!(v.screen_to_image((-1, 0), img), None);
    assert_eq!(v.screen_to_image((0, -1), img), None);
    assert_eq!(v.image_to_screen(10, 20), (10, 20));
}

#[test]
fn screen_image_with_zoom_64() {
    // Una imagen de 1x1 en un viewport de 64x64: zoom 64 y origen (0, 0).
    let img = tam(1, 1);
    let v = View::fit(tam(64, 64), img);
    assert_eq!(v.zoom(), Zoom::MAX);
    assert_eq!(v.origin(), (0, 0));
    assert_eq!(
        v.image_rect(img),
        ScreenRect {
            x: 0,
            y: 0,
            width: 64,
            height: 64
        }
    );
    assert_eq!(v.image_to_screen(0, 0), (0, 0));
    for p in [(0, 0), (63, 0), (0, 63), (63, 63), (31, 40)] {
        assert_eq!(v.screen_to_image(p, img), Some((0, 0)), "punto {p:?}");
    }
    for p in [(64, 0), (0, 64), (64, 64), (-1, 0), (0, -1), (-1, -1)] {
        assert_eq!(v.screen_to_image(p, img), None, "punto {p:?}");
    }
}

#[test]
fn screen_image_with_negative_origin() {
    // Imagen de 8x8 a zoom 1; un zoom_in con ancla (5, 5) deja el zoom en 2 y el origen en
    // (-5, -5): el píxel (5, 5), que estaba bajo el ancla, sigue bajo el ancla.
    let img = tam(8, 8);
    let mut v = View::fit(tam(8, 8), img);
    assert_eq!(v.origin(), (0, 0));
    v.zoom_in((5, 5));
    assert_eq!(v.zoom(), zoom(2));
    assert_eq!(v.origin(), (-5, -5));
    assert_eq!(
        v.image_rect(img),
        ScreenRect {
            x: -5,
            y: -5,
            width: 16,
            height: 16
        }
    );
    assert_eq!(v.image_to_screen(0, 0), (-5, -5));
    assert_eq!(v.image_to_screen(7, 7), (9, 9));
    assert_eq!(v.screen_to_image((-5, -5), img), Some((0, 0)));
    assert_eq!(v.screen_to_image((-4, -4), img), Some((0, 0)));
    assert_eq!(v.screen_to_image((-3, -3), img), Some((1, 1)));
    assert_eq!(v.screen_to_image((0, 0), img), Some((2, 2)));
    assert_eq!(v.screen_to_image((5, 5), img), Some((5, 5)));
    assert_eq!(v.screen_to_image((9, 9), img), Some((7, 7)));
    assert_eq!(v.screen_to_image((10, 10), img), Some((7, 7)));
    assert_eq!(v.screen_to_image((-6, -5), img), None);
    assert_eq!(v.screen_to_image((-5, -6), img), None);
    assert_eq!(v.screen_to_image((11, 10), img), None);
    assert_eq!(v.screen_to_image((10, 11), img), None);
}

#[test]
fn round_trip_of_all_the_pixels_of_a_small_image() {
    // Recorre todos los píxeles de una imagen de 5x3 con cada uno de los 64 zooms, partiendo de
    // un viewport pequeño (la imagen se sale) y con el origen movido por el ancla.
    let img = tam(5, 3);
    let vp = tam(7, 4);
    for n in 1..=64u32 {
        let mut v = View::fit(vp, img);
        v.set_zoom(zoom(n), (-13, 29));
        let z = n as i64;
        let (ox, oy) = v.origin();
        for y in 0..3u32 {
            for x in 0..5u32 {
                let c = v.image_to_screen(x, y);
                assert_eq!(c, (ox + x as i64 * z, oy + y as i64 * z), "x{n} ({x}, {y})");
                for (ex, ey) in [(0, 0), (z - 1, 0), (0, z - 1), (z - 1, z - 1)] {
                    assert_eq!(
                        v.screen_to_image((c.0 + ex, c.1 + ey), img),
                        Some((x, y)),
                        "x{n} ({x}, {y}) + ({ex}, {ey})"
                    );
                }
            }
        }
    }
}

proptest! {
    /// Propiedades 3 y 4 para los 64 zooms: para cada zoom se construye una vista con origen
    /// arbitrario (operaciones aleatorias desde `fit` y un `set_zoom` final al zoom probado).
    #[test]
    fn exact_round_trip_for_all_the_zooms(
        vpw in dim_viewport(),
        vph in dim_viewport(),
        iw in dim_image(),
        ih in dim_image(),
        ops in vec(op(1 << 30), 0..4),
        anchor_final in (-(1i64 << 30)..=(1i64 << 30), -(1i64 << 30)..=(1i64 << 30)),
        pix in (any::<u32>(), any::<u32>()),
        dev in (any::<u64>(), any::<u64>()),
        far in (-(1i64 << 40)..=(1i64 << 40), -(1i64 << 40)..=(1i64 << 40)),
    ) {
        let (vp, img) = (tam(vpw, vph), tam(iw, ih));
        for n in 1..=64u32 {
            let mut v = view(vp, img, &ops);
            v.set_zoom(zoom(n), anchor_final);
            prop_assert_eq!(v.zoom(), zoom(n));
            check_geometry(&v, img, pix, dev, far)?;
        }
    }

    /// Lo mismo con orígenes pequeños, que es donde los puntos caen más a menudo dentro de la
    /// imagen y en los bordes de los bloques.
    #[test]
    fn exact_round_trip_with_origin_near_zero(
        iw in 1u32..=20,
        ih in 1u32..=20,
        n in 1u32..=64,
        ox in -40i64..=40,
        oy in -40i64..=40,
        pix in (any::<u32>(), any::<u32>()),
        dev in (any::<u64>(), any::<u64>()),
    ) {
        let img = tam(iw, ih);
        // Con un viewport del tamaño de la imagen a zoom 1, el origen es (0, 0); un set_zoom con
        // ancla (a, b) a zoom n deja el origen en (a - a·n, b - b·n).
        let mut v = View::fit(img, img);
        prop_assert_eq!(v.origin(), (0, 0));
        v.set_zoom(zoom(n), (ox, oy));
        check_geometry(&v, img, pix, dev, (ox, oy))?;
    }
}

// Propiedad 4: teselado e image_rect.

#[test]
fn image_rect_and_tessellation_example() {
    // 64x48 en 640x480: zoom 10, origen (0, 0).
    let img = tam(64, 48);
    let v = View::fit(tam(640, 480), img);
    assert_eq!(v.zoom(), zoom(10));
    assert_eq!(v.origin(), (0, 0));
    assert_eq!(
        v.image_rect(img),
        ScreenRect {
            x: 0,
            y: 0,
            width: 640,
            height: 480
        }
    );
    assert_eq!(v.image_to_screen(0, 0), (0, 0));
    assert_eq!(v.image_to_screen(1, 0), (10, 0));
    assert_eq!(v.image_to_screen(0, 1), (0, 10));
    assert_eq!(v.image_to_screen(63, 47), (630, 470));
    assert_eq!(v.screen_to_image((629, 469), img), Some((62, 46)));
    assert_eq!(v.screen_to_image((630, 470), img), Some((63, 47)));
    assert_eq!(v.screen_to_image((640, 470), img), None);
}

proptest! {
    /// Propiedad 4 con todos los zooms y orígenes arbitrarios: cada bloque mide exactamente z.
    #[test]
    fn tessellation_without_gaps_or_overlaps(
        vpw in dim_viewport(),
        vph in dim_viewport(),
        iw in dim_image(),
        ih in dim_image(),
        ops in vec(op(1 << 30), 0..4),
        anchor_final in (-(1i64 << 30)..=(1i64 << 30), -(1i64 << 30)..=(1i64 << 30)),
        x in any::<u32>(),
        y in any::<u32>(),
    ) {
        let (vp, img) = (tam(vpw, vph), tam(iw, ih));
        for n in 1..=64u32 {
            let mut v = view(vp, img, &ops);
            v.set_zoom(zoom(n), anchor_final);
            let z = n as i64;
            let (x, y) = (x % iw, y % ih);
            let (ox, oy) = v.origin();
            prop_assert_eq!(v.image_to_screen(0, 0), (ox, oy));
            let (cx, cy) = v.image_to_screen(x, y);
            prop_assert_eq!(v.image_to_screen(x + 1, y).0, cx + z);
            prop_assert_eq!(v.image_to_screen(x, y + 1).1, cy + z);
            // El otro eje no se mueve.
            prop_assert_eq!(v.image_to_screen(x + 1, y).1, cy);
            prop_assert_eq!(v.image_to_screen(x, y + 1).0, cx);
            let rect = v.image_rect(img);
            prop_assert_eq!(
                rect,
                ScreenRect { x: ox, y: oy, width: iw as i64 * z, height: ih as i64 * z }
            );
            // El último bloque termina exactamente en el borde del rectángulo.
            let last_2 = v.image_to_screen(iw - 1, ih - 1);
            prop_assert_eq!(last_2.0 + z, rect.x + rect.width);
            prop_assert_eq!(last_2.1 + z, rect.y + rect.height);
        }
    }
}

// Propiedad 5: ancla.

#[test]
fn anchor_inside_the_example_image() {
    // Imagen de 4x4 a zoom 10 (origen 0). El píxel bajo (25, 35) es el (2, 3).
    let img = tam(4, 4);
    let mut v = View::fit(tam(40, 40), img);
    assert_eq!(v.zoom(), zoom(10));
    assert_eq!(v.screen_to_image((25, 35), img), Some((2, 3)));
    v.set_zoom(zoom(20), (25, 35));
    assert_eq!(v.zoom(), zoom(20));
    // origen = 25 - 25·20/10 = -25; 35 - 35·20/10 = -35.
    assert_eq!(v.origin(), (-25, -35));
    assert_eq!(v.screen_to_image((25, 35), img), Some((2, 3)));
    // Y no se hace clamp: la imagen de 80x80 sigue colocada alrededor del ancla.
    v.set_zoom(zoom(1), (25, 35));
    assert_eq!(v.screen_to_image((25, 35), img), Some((2, 3)));
}

#[test]
fn anchor_outside_the_image_keeps_the_block_index() {
    // Imagen de 4x4 a zoom 10 (origen 0). El ancla (-7, 100) está fuera: bloque (-1, 10).
    let img = tam(4, 4);
    let mut v = View::fit(tam(40, 40), img);
    assert_eq!(v.screen_to_image((-7, 100), img), None);
    v.set_zoom(zoom(3), (-7, 100));
    // origen x = -7 - floor(-7·3/10) = -7 + 3 = -4; y = 100 - floor(100·3/10) = 70.
    assert_eq!(v.origin(), (-4, 70));
    assert_eq!(v.zoom().unscale(-7 - v.origin().0), -1);
    assert_eq!(v.zoom().unscale(100 - v.origin().1), 10);
    assert_eq!(v.screen_to_image((-7, 100), img), None);
    // set_zoom no hace clamp: la imagen queda fuera del viewport de 40x40 en y.
    assert_eq!(v.image_rect(img).y, 70);
}

#[test]
fn anchor_with_a_one_pixel_image() {
    // 1x1 a zoom 64; al bajar a zoom 1 con el ancla en el borde del bloque.
    let img = tam(1, 1);
    let mut v = View::fit(tam(64, 64), img);
    assert_eq!(v.zoom(), Zoom::MAX);
    v.set_zoom(Zoom::ONE, (64, 0)); // fuera de la imagen: bloque (1, 0)
    // origen x = 64 - floor(64/64) = 63; y = 0 - 0 = 0.
    assert_eq!(v.origin(), (63, 0));
    assert_eq!(v.screen_to_image((64, 0), img), None);
    assert_eq!(v.screen_to_image((63, 0), img), Some((0, 0)));

    let mut w = View::fit(tam(64, 64), img);
    w.set_zoom(Zoom::ONE, (10, 10)); // dentro: píxel (0, 0)
    assert_eq!(w.origin(), (10, 10));
    assert_eq!(w.screen_to_image((10, 10), img), Some((0, 0)));
}

#[test]
fn zoom_in_and_zoom_out_examples() {
    let img = tam(8, 8);
    let mut v = View::fit(tam(8, 8), img);
    v.zoom_in((5, 5));
    assert_eq!((v.zoom(), v.origin()), (zoom(2), (-5, -5)));
    // origen = 3 - floor((3 + 5)·3/2) = -9; 7 - floor((7 + 5)·3/2) = -11.
    v.zoom_in((3, 7));
    assert_eq!((v.zoom(), v.origin()), (zoom(3), (-9, -11)));
    // origen = 3 - floor((3 + 9)·2/3) = -5; 7 - floor((7 + 11)·2/3) = -5.
    v.zoom_out((3, 7));
    assert_eq!((v.zoom(), v.origin()), (zoom(2), (-5, -5)));
    // Saturación: en el máximo, zoom_in no cambia nada; en el mínimo, zoom_out tampoco.
    let mut max = View::fit(tam(64, 64), tam(1, 1));
    assert_eq!(max.zoom(), Zoom::MAX);
    let before = max;
    max.zoom_in((17, -4));
    assert_eq!(max, before);
    let mut min = View::fit(tam(8, 8), img);
    min.set_zoom(Zoom::MIN, (0, 0));
    assert_eq!(min.zoom(), Zoom::MIN);
    let before = min;
    min.zoom_out((17, -4));
    assert_eq!(min, before);
}

#[test]
fn set_zoom_to_the_same_zoom_does_not_move_the_view() {
    let img = tam(30, 20);
    let mut v = View::fit(tam(200, 100), img);
    v.set_zoom(zoom(7), (33, 12));
    let before = v;
    v.set_zoom(zoom(7), (-500, 9999));
    assert_eq!(v, before);
}

proptest! {
    #[test]
    fn the_block_under_the_anchor_is_kept(
        vpw in dim_viewport(),
        vph in dim_viewport(),
        iw in dim_image(),
        ih in dim_image(),
        ops in vec(op(1 << 30), 0..4),
        inside in any::<bool>(),
        outside in (-(1i64 << 31)..=(1i64 << 31), -(1i64 << 31)..=(1i64 << 31)),
        dev in (any::<u64>(), any::<u64>()),
    ) {
        let (vp, img) = (tam(vpw, vph), tam(iw, ih));
        let before = view(vp, img, &ops);
        let rect = before.image_rect(img);
        let anchor = if inside {
            (
                rect.x + (dev.0 % rect.width as u64) as i64,
                rect.y + (dev.1 % rect.height as u64) as i64,
            )
        } else {
            outside
        };
        if inside {
            prop_assert!(before.screen_to_image(anchor, img).is_some());
        }
        check_anchor_at_all_zooms(&before, anchor, img)?;
    }

    /// Cambios de zoom encadenados con anclas distintas siguen conservando el bloque en cada paso.
    #[test]
    fn the_block_is_kept_at_each_step_of_a_chain(
        iw in 1u32..=100,
        ih in 1u32..=100,
        steps_2 in vec((1u32..=64, (-500i64..=500, -500i64..=500)), 1..8),
    ) {
        let img = tam(iw, ih);
        let mut v = View::fit(tam(300, 200), img);
        for (n, anchor) in steps_2 {
            let before = v;
            v.set_zoom(zoom(n), anchor);
            check_anchor(&before, &v, anchor, img)?;
        }
    }

    /// zoom_in y zoom_out equivalen a set_zoom con el nivel siguiente o anterior.
    #[test]
    fn zoom_in_and_zoom_out_are_equivalent_to_set_zoom(
        iw in 1u32..=100,
        ih in 1u32..=100,
        ops in vec(op(1 << 20), 0..4),
        anchor in (-(1i64 << 20)..=(1i64 << 20), -(1i64 << 20)..=(1i64 << 20)),
    ) {
        let (vp, img) = (tam(300, 200), tam(iw, ih));
        let base = view(vp, img, &ops);
        let mut more = base;
        more.zoom_in(anchor);
        let mut expected_2 = base;
        expected_2.set_zoom(base.zoom().zoom_in(), anchor);
        prop_assert_eq!(more, expected_2);
        let mut less = base;
        less.zoom_out(anchor);
        let mut expected_2 = base;
        expected_2.set_zoom(base.zoom().zoom_out(), anchor);
        prop_assert_eq!(less, expected_2);
    }
}

// Propiedad 6: fit.

#[test]
fn fit_examples() {
    // La imagen cabe 10 veces: zoom 10, origen (0, 0).
    let v = View::fit(tam(640, 480), tam(64, 48));
    assert_eq!((v.zoom(), v.origin()), (zoom(10), (0, 0)));

    // El eje que limita es el alto: 100/50 = 2 frente a 100/30 = 3. Sobran 40 px en x.
    let v = View::fit(tam(100, 100), tam(30, 50));
    assert_eq!((v.zoom(), v.origin()), (zoom(2), (20, 0)));

    // No cabe ni a zoom 1: el mayor nivel 1/K que quepa (1/8: 8×6 píxeles) y centrada.
    let v = View::fit(tam(10, 10), tam(64, 48));
    assert_eq!((Some(v.zoom()), v.origin()), (Zoom::shrink(8), (1, 2)));

    // Tope en 64.
    let v = View::fit(tam(1000, 1000), tam(1, 1));
    assert_eq!(v.zoom(), Zoom::MAX);
    let (ox, oy) = v.origin();
    assert!(
        (ox - 468).abs() <= 1 && (oy - 468).abs() <= 1,
        "origen {:?}",
        v.origin()
    );

    // Imagen de 1x1 en un viewport de 1x1.
    let v = View::fit(tam(1, 1), tam(1, 1));
    assert_eq!((v.zoom(), v.origin()), (Zoom::ONE, (0, 0)));

    // Viewport de 1 px y una imagen de 3x3: 1/4 (ocupa 1 px) y origen 0.
    let v = View::fit(tam(1, 1), tam(3, 3));
    assert_eq!((Some(v.zoom()), v.origin()), (Zoom::shrink(4), (0, 0)));

    // Viewport de 0 px: no cabe nada, así que el mínimo (1/64, 1 px), centrado con diferencia
    // de como mucho 1.
    let v = View::fit(tam(0, 0), tam(5, 5));
    assert_eq!(v.zoom(), Zoom::MIN);
    assert!([-1, 0].contains(&v.origin().0) && [-1, 0].contains(&v.origin().1));

    // Ajuste exacto en un eje y holgado en el otro.
    let v = View::fit(tam(64, 640), tam(64, 48));
    assert_eq!(v.zoom(), Zoom::ONE);
    assert_eq!(v.origin().0, 0);
}

#[test]
fn fit_each_zoom_with_the_exact_image() {
    // Una imagen de 3x2 en un viewport de 3z x 2z da exactamente z, y en uno de una unidad menos
    // en un eje da el nivel anterior de la escalera (z - 1, o 1/2 desde ×1).
    let img = tam(3, 2);
    let previous = |n: u32| {
        if n == 1 {
            Zoom::shrink(2).expect("1/2")
        } else {
            zoom(n - 1)
        }
    };
    for n in 1..=64u32 {
        let v = View::fit(tam(3 * n, 2 * n), img);
        assert_eq!(v.zoom(), zoom(n), "viewport justo para ×{n}");
        assert_eq!(v.origin(), (0, 0));
        let v = View::fit(tam(3 * n - 1, 2 * n), img);
        assert_eq!(v.zoom(), previous(n), "falta 1 px de ancho para ×{n}");
        let v = View::fit(tam(3 * n, 2 * n - 1), img);
        assert_eq!(v.zoom(), previous(n), "falta 1 px de alto para ×{n}");
    }
}

proptest! {
    #[test]
    fn fit_is_the_largest_zoom_that_fits_and_centers(
        vpw in dim_viewport(),
        vph in dim_viewport(),
        iw in dim_image(),
        ih in dim_image(),
    ) {
        check_fit(tam(vpw, vph), tam(iw, ih))?;
    }

    /// Con imágenes pequeñas y viewports pequeños se cubren todos los zooms y los bordes.
    #[test]
    fn fit_with_small_sizes(
        vpw in 0u32..=300,
        vph in 0u32..=300,
        iw in 1u32..=30,
        ih in 1u32..=30,
    ) {
        check_fit(tam(vpw, vph), tam(iw, ih))?;
    }
}

// Propiedad 7: clamp y pan.

#[test]
fn pan_inside_the_range_scrolls_without_clamp() {
    let img = tam(100, 100);
    let vp = tam(100, 100);
    let mut v = View::fit(vp, img);
    assert_eq!(v.origin(), (0, 0));
    // Quedan 90 y 80 píxeles visibles, más que MIN_VISIBLE.
    v.pan(-10, 20, vp, img);
    assert_eq!(v.origin(), (-10, 20));
    v.pan(5, -5, vp, img);
    assert_eq!(v.origin(), (-5, 15));
    v.pan(0, 0, vp, img);
    assert_eq!(v.origin(), (-5, 15));
}

#[test]
fn pan_far_leaves_at_least_min_visible_visible() {
    let img = tam(100, 100);
    let vp = tam(100, 100);
    for (dx, dy) in [
        (-1000, 0),
        (1000, 0),
        (0, -1000),
        (0, 1000),
        (-100_000, 100_000),
        (i64::from(u32::MAX), -i64::from(u32::MAX)),
    ] {
        let start_2 = View::fit(vp, img);
        let mut v = start_2;
        v.pan(dx, dy, vp, img);
        assert!(meets_visibility(&v, vp, img), "pan({dx}, {dy}) dio {v:?}");
        assert_eq!(v.zoom(), start_2.zoom());
        // El clamp no se pasa del destino: no sale más lejos de lo pedido.
        assert!(
            v.origin().0 * dx.signum() <= dx.abs(),
            "x de pan({dx}, {dy}): {v:?}"
        );
        assert!(
            v.origin().1 * dy.signum() <= dy.abs(),
            "y de pan({dx}, {dy}): {v:?}"
        );
        // Y se queda en el lado hacia el que se movió.
        assert!(
            (dx == 0 || v.origin().0 * dx.signum() >= 0)
                && (dy == 0 || v.origin().1 * dy.signum() >= 0),
            "pan({dx}, {dy}) dio {v:?}"
        );
    }
}

#[test]
fn clamp_with_a_small_image_uses_its_extent() {
    // Imagen de 4x4 a zoom 2: extensión 8 < MIN_VISIBLE, así que debe verse entera (8 px).
    let img = tam(4, 4);
    let vp = tam(100, 100);
    let mut v = View::fit(tam(8, 8), img);
    assert_eq!(v.zoom(), zoom(2));
    for (dx, dy) in [(-500, -500), (500, 500), (-500, 500), (500, -500)] {
        v.pan(dx, dy, vp, img);
        assert!(meets_visibility(&v, vp, img), "pan({dx}, {dy}) dio {v:?}");
        let (ox, oy) = v.origin();
        assert!(
            ox >= 0 && ox + 8 <= 100,
            "imagen entera visible en x: {v:?}"
        );
        assert!(
            oy >= 0 && oy + 8 <= 100,
            "imagen entera visible en y: {v:?}"
        );
    }
}

#[test]
fn clamp_with_viewport_of_0_and_of_1_pixel() {
    let img = tam(10, 10);
    // Viewport de 0 px: no se exige visibilidad alguna (el mínimo es 0) y no hay pánico.
    let vp0 = tam(0, 0);
    let mut v = View::fit(vp0, img);
    v.pan(1_000_000, -1_000_000, vp0, img);
    v.clamp(vp0, img);
    assert!(meets_visibility(&v, vp0, img));
    // Viewport de 1 px: debe quedar visible al menos 1 px por eje.
    let vp1 = tam(1, 1);
    for (dx, dy) in [
        (1_000_000, 0),
        (-1_000_000, 0),
        (0, 1_000_000),
        (0, -1_000_000),
    ] {
        let mut v = View::fit(vp1, img);
        v.pan(dx, dy, vp1, img);
        assert!(meets_visibility(&v, vp1, img), "pan({dx}, {dy}) dio {v:?}");
        let (ox, oy) = v.origin();
        assert!(ox <= 0 && ox + 10 >= 1, "x: {v:?}");
        assert!(oy <= 0 && oy + 10 >= 1, "y: {v:?}");
    }
}

#[test]
fn clamp_with_a_one_pixel_image() {
    // 1x1 a zoom 1: la extensión es 1, así que ese píxel debe seguir visible.
    let img = tam(1, 1);
    let vp = tam(50, 50);
    let mut v = View::fit(tam(1, 1), img);
    assert_eq!(v.zoom(), Zoom::ONE);
    v.pan(10_000, 10_000, vp, img);
    assert!(meets_visibility(&v, vp, img), "{v:?}");
    let (ox, oy) = v.origin();
    assert!((0..50).contains(&ox) && (0..50).contains(&oy), "{v:?}");
    v.pan(-10_000, -10_000, vp, img);
    assert!(meets_visibility(&v, vp, img), "{v:?}");
    let (ox, oy) = v.origin();
    assert!((0..50).contains(&ox) && (0..50).contains(&oy), "{v:?}");
}

#[test]
fn clamp_recovers_a_view_that_set_zoom_left_outside() {
    // set_zoom no hace clamp; clamp sí devuelve la imagen al viewport.
    let img = tam(4, 4);
    let vp = tam(40, 40);
    let mut v = View::fit(vp, img);
    v.set_zoom(zoom(3), (-7, 100));
    assert_eq!(v.origin(), (-4, 70));
    assert!(!meets_visibility(&v, vp, img));
    v.clamp(vp, img);
    assert!(meets_visibility(&v, vp, img), "{v:?}");
    assert_eq!(v.zoom(), zoom(3));
    let after_2 = v;
    v.clamp(vp, img);
    assert_eq!(v, after_2);
}

#[test]
fn fit_already_meets_the_minimum_visibility_and_clamp_does_not_move_it() {
    for (vpw, vph, iw, ih) in [
        (640, 480, 64, 48),
        (10, 10, 64, 48),
        (1, 1, 1, 1),
        (1, 1, 3, 3),
        (0, 0, 5, 5),
        (100, 1, 1000, 1),
        (20, 20, 1, 1),
        (1000, 1000, 1, 1),
    ] {
        let (vp, img) = (tam(vpw, vph), tam(iw, ih));
        let v = View::fit(vp, img);
        assert!(
            meets_visibility(&v, vp, img),
            "fit({vp:?}, {img:?}) dio {v:?}"
        );
        let mut c = v;
        c.clamp(vp, img);
        assert_eq!(c, v, "clamp movió fit({vp:?}, {img:?})");
    }
}

proptest! {
    #[test]
    fn clamp_leaves_the_minimum_visible_and_is_idempotent(
        vpw in dim_viewport(),
        vph in dim_viewport(),
        iw in dim_image(),
        ih in dim_image(),
        ops in vec(op(1 << 30), 0..4),
    ) {
        let (vp, img) = (tam(vpw, vph), tam(iw, ih));
        let without_clamp = view(vp, img, &ops);
        let mut v = without_clamp;
        v.clamp(vp, img);
        prop_assert_eq!(v.zoom(), without_clamp.zoom(), "clamp no cambia el zoom");
        prop_assert!(meets_visibility(&v, vp, img), "clamp de {:?} dio {:?}", without_clamp, v);
        let mut other = v;
        other.clamp(vp, img);
        prop_assert_eq!(other, v, "clamp debe ser idempotente");
    }

    /// Si la vista ya cumple la visibilidad mínima, clamp no la mueve (se prueba con todas las
    /// vistas generadas que la cumplen, que son muchas porque el viewport suele ser grande).
    #[test]
    fn clamp_does_not_move_a_view_that_already_complies(
        vpw in dim_viewport(),
        vph in dim_viewport(),
        iw in dim_image(),
        ih in dim_image(),
        ops in vec(op(1 << 12), 0..4),
    ) {
        let (vp, img) = (tam(vpw, vph), tam(iw, ih));
        let v = view(vp, img, &ops);
        if meets_visibility(&v, vp, img) {
            let mut c = v;
            c.clamp(vp, img);
            prop_assert_eq!(c, v);
        }
    }

    /// `pan` equivale a desplazar el origen y hacer clamp. Como no hay constructor con origen
    /// libre, se comprueba con consecuencias observables, partiendo de una vista ya acotada.
    #[test]
    fn pan_is_equivalent_to_scrolling_and_bounding(
        vpw in dim_viewport(),
        vph in dim_viewport(),
        iw in dim_image(),
        ih in dim_image(),
        ops in vec(op(1 << 20), 0..4),
        d in (-(1i64 << 22)..=(1i64 << 22), -(1i64 << 22)..=(1i64 << 22)),
    ) {
        let (vp, img) = (tam(vpw, vph), tam(iw, ih));
        let mut start_2 = view(vp, img, &ops);
        start_2.clamp(vp, img);
        let (ox, oy) = start_2.origin();
        let (ext_x, ext_y) = (
            extension(start_2.zoom(), iw as i128),
            extension(start_2.zoom(), ih as i128),
        );
        let (dx, dy) = d;

        let mut v = start_2;
        v.pan(dx, dy, vp, img);
        prop_assert_eq!(v.zoom(), start_2.zoom(), "pan no cambia el zoom");
        prop_assert!(meets_visibility(&v, vp, img), "pan({dx}, {dy}) de {start_2:?} dio {v:?}");

        // Cada eje es independiente del otro.
        let mut only_x = start_2;
        only_x.pan(dx, 0, vp, img);
        let mut only_y = start_2;
        only_y.pan(0, dy, vp, img);
        prop_assert_eq!(v.origin().0, only_x.origin().0);
        prop_assert_eq!(v.origin().1, only_y.origin().1);
        // Un eje sin desplazar no se mueve: la vista de partida ya cumple.
        prop_assert_eq!(only_x.origin().1, oy);
        prop_assert_eq!(only_y.origin().0, ox);

        // Si el origen desplazado ya cumple, el resultado es exactamente ese origen.
        for (new_2, o, delta, ext, viewport, result) in [
            (ox as i128 + dx as i128, ox, dx, ext_x, vpw, v.origin().0),
            (oy as i128 + dy as i128, oy, dy, ext_y, vph, v.origin().1),
        ] {
            let new_64 = new_2 as i64;
            if axis_meets(new_64, ext, viewport) {
                prop_assert_eq!(result, new_64, "el origen desplazado cumple y debe respetarse");
            } else {
                // Si no, el acotado deja el origen entre el de partida y el pedido.
                let (lo, hi) = (o.min(new_64), o.max(new_64));
                prop_assert!(
                    (lo..=hi).contains(&result),
                    "resultado {result} fuera de [{lo}, {hi}] (delta {delta})"
                );
            }
        }

        // Saturación: pedir el doble de lejos, cuando ya se acotó, da el mismo origen.
        if dx != 0 && v.origin().0 != ox + dx {
            let mut double = start_2;
            double.pan(2 * dx, 0, vp, img);
            prop_assert_eq!(double.origin().0, v.origin().0);
        }
        if dy != 0 && v.origin().1 != oy + dy {
            let mut double = start_2;
            double.pan(0, 2 * dy, vp, img);
            prop_assert_eq!(double.origin().1, v.origin().1);
        }
    }

    /// Desplazamientos que se anulan mientras no se acota nada vuelven al origen de partida.
    #[test]
    fn small_pan_and_its_inverse_return_to_the_origin(
        iw in 100u32..=300,
        ih in 100u32..=300,
        dx in -20i64..=20,
        dy in -20i64..=20,
    ) {
        // Viewport mucho menor que la imagen a zoom 1: el origen centrado deja de sobra.
        let (vp, img) = (tam(64, 64), tam(iw, ih));
        let start_2 = View::fit(vp, img);
        let mut v = start_2;
        v.pan(dx, dy, vp, img);
        prop_assert_eq!(v.origin(), (start_2.origin().0 + dx, start_2.origin().1 + dy));
        v.pan(-dx, -dy, vp, img);
        prop_assert_eq!(v, start_2);
    }
}

// Propiedad 8: sin pánico ni desbordamiento.

#[test]
fn deterministic_extremes_without_panic() {
    let large = (1u32 << 30, 1u32 << 30);
    let sizes_image = [
        tam(1, 1),
        tam(large.0, 1),
        tam(1, large.1),
        tam(large.0, large.1),
    ];
    let sizes_viewport = [tam(0, 0), tam(1, 1), tam(0, large.1), tam(large.0, large.1)];
    let lim = 1i64 << 40;
    let anchors = [(0, 0), (lim, lim), (-lim, -lim), (lim, -lim), (-lim, lim)];
    for img in sizes_image {
        for vp in sizes_viewport {
            check_fit(vp, img).unwrap();
            for anchor in anchors {
                for n in [1u32, 2, 7, 63, 64] {
                    let mut v = View::fit(vp, img);
                    v.set_zoom(zoom(n), anchor);
                    check_anchor_at_all_zooms(&v, anchor, img).unwrap();
                    check_geometry(&v, img, (u32::MAX, u32::MAX), (u64::MAX, u64::MAX), anchor)
                        .unwrap();
                    v.pan(anchor.0, anchor.1, vp, img);
                    assert!(meets_visibility(&v, vp, img), "{v:?}");
                    v.zoom_in(anchor);
                    v.zoom_out(anchor);
                    v.clamp(vp, img);
                    assert!(meets_visibility(&v, vp, img), "{v:?}");
                }
            }
        }
    }
}

proptest! {
    #[test]
    fn without_panic_with_extreme_sizes_and_coordinates(
        vpw in dim_viewport_extreme(),
        vph in dim_viewport_extreme(),
        iw in dim_image_extreme(),
        ih in dim_image_extreme(),
        ops in vec(op(1 << 40), 0..4),
        anchor in (-(1i64 << 40)..=(1i64 << 40), -(1i64 << 40)..=(1i64 << 40)),
        n in 1u32..=64,
        pan in (-(1i64 << 40)..=(1i64 << 40), -(1i64 << 40)..=(1i64 << 40)),
        pix in (any::<u32>(), any::<u32>()),
        dev in (any::<u64>(), any::<u64>()),
        far in (-(1i64 << 40)..=(1i64 << 40), -(1i64 << 40)..=(1i64 << 40)),
    ) {
        let (vp, img) = (tam(vpw, vph), tam(iw, ih));

        // fit: nivel de la escalera, centrada y determinista.
        check_fit(vp, img)?;

        // Una vista con origen y zoom arbitrarios (set_zoom no acota, así que el origen puede
        // quedar muy lejos del viewport).
        let v = view(vp, img, &ops);
        let z = v.zoom();
        prop_assert!(Zoom::MIN <= z && z <= Zoom::MAX);
        // La geometría por bloques de `check_geometry` es la de los zooms enteros; los
        // fraccionarios (por rejilla) los cubre `zoom_ladder.rs`.
        if z >= Zoom::ONE {
            check_geometry(&v, img, pix, dev, far)?;
        }

        // Ancla en cualquier punto, dentro o fuera, con todos los zooms.
        check_anchor_at_all_zooms(&v, anchor, img)?;
        let mut change = v;
        change.set_zoom(zoom(n), anchor);
        check_geometry(&change, img, pix, dev, far)?;

        // Clamp y pan: invariantes de visibilidad e idempotencia.
        let mut c = v;
        c.clamp(vp, img);
        prop_assert!(meets_visibility(&c, vp, img), "clamp de {:?} dio {:?}", v, c);
        let mut other = c;
        other.clamp(vp, img);
        prop_assert_eq!(other, c);

        let mut p = change;
        p.pan(pan.0, pan.1, vp, img);
        prop_assert!(meets_visibility(&p, vp, img), "pan de {:?} dio {:?}", change, p);
        check_geometry(&p, img, pix, dev, far)?;
        let mut other = p;
        other.clamp(vp, img);
        prop_assert_eq!(other, p, "tras pan, clamp no debe mover nada");
    }
}
