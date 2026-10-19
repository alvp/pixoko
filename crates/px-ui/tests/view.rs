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
fn assert_plano_traits<T: Clone + Copy + Debug + PartialEq + Eq>() {}

#[test]
fn traits_derivados() {
    assert_zoom_traits::<Zoom>();
    assert_plano_traits::<View>();
    assert_plano_traits::<Size>();
    assert_plano_traits::<ScreenRect>();
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
fn fraccion(z: Zoom) -> (i128, i128) {
    if z >= Zoom::ONE {
        (i128::from(z.scale(1)), 1)
    } else {
        (1, i128::from(z.unscale(1)))
    }
}

/// Longitud en pantalla de `n` píxeles de imagen según el modelo: `ceil(n · num / den)`.
fn extension(z: Zoom, n: i128) -> i128 {
    let (num, den) = fraccion(z);
    (n * num + den - 1).div_euclid(den)
}

fn tam(width: u32, height: u32) -> Size {
    Size::new(width, height)
}

/// Índice de píxel de imagen en un eje según el modelo: floor((p - origen) · den / num).
fn indice_modelo(p: i64, origen: i64, z: Zoom) -> i128 {
    let (num, den) = fraccion(z);
    ((p as i128 - origen as i128) * den).div_euclid(num)
}

/// Modelo de `screen_to_image`: `None` si el bloque cae fuera de la imagen.
fn pixel_modelo(v: &View, p: (i64, i64), img: Size) -> Option<(u32, u32)> {
    let (ox, oy) = v.origin();
    let ix = indice_modelo(p.0, ox, v.zoom());
    let iy = indice_modelo(p.1, oy, v.zoom());
    if (0..img.width as i128).contains(&ix) && (0..img.height as i128).contains(&iy) {
        Some((ix as u32, iy as u32))
    } else {
        None
    }
}

/// Píxeles de la imagen visibles en un eje: solape de [origen, origen+ext) con [0, viewport).
fn visibles(origen: i64, ext: i128, viewport: u32) -> i128 {
    let ini = (origen as i128).max(0);
    let fin = (origen as i128 + ext).min(viewport as i128);
    (fin - ini).max(0)
}

/// Mínimo que debe quedar visible en un eje: min(MIN_VISIBLE, ext, viewport).
fn minimo_visible(ext: i128, viewport: u32) -> i128 {
    (MIN_VISIBLE as i128).min(ext).min(viewport as i128)
}

fn eje_cumple(origen: i64, ext: i128, viewport: u32) -> bool {
    visibles(origen, ext, viewport) >= minimo_visible(ext, viewport)
}

fn cumple_visibilidad(v: &View, vp: Size, img: Size) -> bool {
    let (ox, oy) = v.origin();
    eje_cumple(ox, extension(v.zoom(), img.width as i128), vp.width)
        && eje_cumple(oy, extension(v.zoom(), img.height as i128), vp.height)
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

fn aplicar(v: &mut View, op: &Op, vp: Size, img: Size) {
    match op {
        Op::SetZoom(n, a) => v.set_zoom(zoom(*n), *a),
        Op::ZoomIn(a) => v.zoom_in(*a),
        Op::ZoomOut(a) => v.zoom_out(*a),
        Op::Pan(dx, dy) => v.pan(*dx, *dy, vp, img),
        Op::Clamp => v.clamp(vp, img),
    }
}

fn vista(vp: Size, img: Size, ops: &[Op]) -> View {
    let mut v = View::fit(vp, img);
    for op in ops {
        aplicar(&mut v, op, vp, img);
    }
    v
}

// Estrategias.

fn op(rango: i64) -> impl Strategy<Value = Op> {
    let ancla = (-rango..=rango, -rango..=rango);
    prop_oneof![
        (1u32..=64, ancla.clone()).prop_map(|(n, a)| Op::SetZoom(n, a)),
        ancla.clone().prop_map(Op::ZoomIn),
        ancla.prop_map(Op::ZoomOut),
        (-rango..=rango, -rango..=rango).prop_map(|(dx, dy)| Op::Pan(dx, dy)),
        Just(Op::Clamp),
    ]
}

/// Extensión de imagen (al menos 1): pequeña, media o grande.
fn dim_imagen() -> impl Strategy<Value = u32> {
    prop_oneof![1u32..=40, 1u32..=4000, 1u32..=100_000]
}

/// Extensión de viewport (puede ser 0 o 1).
fn dim_viewport() -> impl Strategy<Value = u32> {
    prop_oneof![Just(0u32), Just(1u32), 0u32..=60, 0u32..=5000]
}

fn dim_imagen_extrema() -> impl Strategy<Value = u32> {
    prop_oneof![Just(1u32), 1u32..=(1 << 30), Just(1u32 << 30)]
}

fn dim_viewport_extrema() -> impl Strategy<Value = u32> {
    prop_oneof![Just(0u32), Just(1u32), 0u32..=(1 << 30), Just(1u32 << 30)]
}

// Comprobaciones compartidas (devuelven `Result` para usarlas dentro de `proptest!`).

/// Propiedades 3 y 4 sobre una vista: ida y vuelta, esquinas, vecinos, puntos dentro y fuera.
/// `desv` elige puntos dentro de la imagen y desplazamientos fuera; `lejos` es un punto
/// arbitrario que se contrasta con el modelo.
fn comprobar_geometria(
    v: &View,
    img: Size,
    pix: (u32, u32),
    desv: (u64, u64),
    lejos: (i64, i64),
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
    let pixeles = [
        (pix.0 % w, pix.1 % h),
        (0, 0),
        (w - 1, h - 1),
        (w - 1, 0),
        (0, h - 1),
    ];
    for (x, y) in pixeles {
        let c = v.image_to_screen(x, y);
        prop_assert_eq!(c, (ox + x as i64 * z, oy + y as i64 * z));
        prop_assert_eq!(v.screen_to_image(c, img), Some((x, y)));
        for (ex, ey) in [(z - 1, 0), (0, z - 1), (z - 1, z - 1)] {
            prop_assert_eq!(v.screen_to_image((c.0 + ex, c.1 + ey), img), Some((x, y)));
        }
        // Un píxel de pantalla más allá del bloque pertenece al vecino, o a nadie en el borde.
        let derecha = if x + 1 < w { Some((x + 1, y)) } else { None };
        let izquierda = if x > 0 { Some((x - 1, y)) } else { None };
        let abajo = if y + 1 < h { Some((x, y + 1)) } else { None };
        let arriba = if y > 0 { Some((x, y - 1)) } else { None };
        prop_assert_eq!(v.screen_to_image((c.0 + z, c.1), img), derecha);
        prop_assert_eq!(v.screen_to_image((c.0 - 1, c.1), img), izquierda);
        prop_assert_eq!(v.screen_to_image((c.0, c.1 + z), img), abajo);
        prop_assert_eq!(v.screen_to_image((c.0, c.1 - 1), img), arriba);

        // Propiedad 4: teselado sin huecos ni solapes.
        prop_assert_eq!(v.image_to_screen(x + 1, y), (c.0 + z, c.1));
        prop_assert_eq!(v.image_to_screen(x, y + 1), (c.0, c.1 + z));
    }

    // Propiedad 3, segunda viñeta: un punto dentro de image_rect.
    let p = (
        rect.x + (desv.0 % rect.width as u64) as i64,
        rect.y + (desv.1 % rect.height as u64) as i64,
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
    prop_assert_eq!(Some((ix, iy)), pixel_modelo(v, p, img));

    // Propiedad 3, tercera viñeta: fuera de image_rect siempre None.
    let a = (desv.0 % 1000) as i64;
    let bx = (desv.0 % rect.width as u64) as i64;
    let by = (desv.1 % rect.height as u64) as i64;
    let derecha = rect.x + rect.width;
    let debajo = rect.y + rect.height;
    let fuera = [
        (rect.x - 1, rect.y + by),
        (rect.x - 1 - a, rect.y + by),
        (derecha, rect.y + by),
        (derecha + a, rect.y + by),
        (rect.x + bx, rect.y - 1),
        (rect.x + bx, rect.y - 1 - a),
        (rect.x + bx, debajo),
        (rect.x + bx, debajo + a),
        (rect.x - 1 - a, rect.y - 1 - a),
        (derecha + a, debajo + a),
        (rect.x - 1, debajo),
        (derecha, rect.y - 1),
    ];
    for q in fuera {
        prop_assert_eq!(
            v.screen_to_image(q, img),
            None,
            "punto {:?} en {:?}",
            q,
            rect
        );
    }

    // Un punto arbitrario (posiblemente muy lejano) coincide con el modelo.
    prop_assert_eq!(v.screen_to_image(lejos, img), pixel_modelo(v, lejos, img));
    Ok(())
}

/// Propiedad 5: el píxel bajo el ancla se conserva (con el modelo en `i128`). Con zoom ≥ ×1 es
/// exacto, como siempre; si el nivel nuevo es `1/K`, el píxel sube a la rejilla del nivel
/// (`floor(i / K) · K`; specs/hito-2-zoom-baldosas.md, decisión 4). Con `K = 1` es la misma
/// igualdad de antes.
fn comprobar_ancla(
    antes: &View,
    despues: &View,
    ancla: (i64, i64),
    img: Size,
) -> Result<(), TestCaseError> {
    let rejilla = if despues.zoom() < Zoom::ONE {
        fraccion(despues.zoom()).1
    } else {
        1
    };
    let esperado = |i: i128| i.div_euclid(rejilla) * rejilla;
    let ix = esperado(indice_modelo(ancla.0, antes.origin().0, antes.zoom()));
    let iy = esperado(indice_modelo(ancla.1, antes.origin().1, antes.zoom()));
    prop_assert_eq!(
        ix,
        indice_modelo(ancla.0, despues.origin().0, despues.zoom()),
        "eje x: ancla {:?}, antes {:?}, después {:?}",
        ancla,
        antes,
        despues
    );
    prop_assert_eq!(
        iy,
        indice_modelo(ancla.1, despues.origin().1, despues.zoom()),
        "eje y: ancla {:?}, antes {:?}, después {:?}",
        ancla,
        antes,
        despues
    );
    // Mismo índice implica mismo estado dentro/fuera y mismo píxel (el índice esperado, si cae
    // dentro de la imagen).
    let dentro = (0..img.width as i128).contains(&ix) && (0..img.height as i128).contains(&iy);
    prop_assert_eq!(
        despues.screen_to_image(ancla, img),
        dentro.then_some((ix as u32, iy as u32))
    );
    if rejilla == 1 {
        prop_assert_eq!(
            antes.screen_to_image(ancla, img),
            despues.screen_to_image(ancla, img)
        );
    }
    Ok(())
}

/// Propiedad 5 sobre todos los cambios de zoom posibles desde `antes`.
fn comprobar_ancla_en_todos_los_zooms(
    antes: &View,
    ancla: (i64, i64),
    img: Size,
) -> Result<(), TestCaseError> {
    for n in 1..=64 {
        let mut v = *antes;
        v.set_zoom(zoom(n), ancla);
        prop_assert_eq!(v.zoom(), zoom(n));
        comprobar_ancla(antes, &v, ancla, img)?;
    }
    let mut dentro = *antes;
    dentro.zoom_in(ancla);
    prop_assert_eq!(dentro.zoom(), antes.zoom().zoom_in());
    comprobar_ancla(antes, &dentro, ancla, img)?;
    let mut fuera = *antes;
    fuera.zoom_out(ancla);
    prop_assert_eq!(fuera.zoom(), antes.zoom().zoom_out());
    comprobar_ancla(antes, &fuera, ancla, img)?;
    Ok(())
}

/// Propiedad 6 para un viewport y una imagen.
fn comprobar_fit(vp: Size, img: Size) -> Result<(), TestCaseError> {
    let v = View::fit(vp, img);
    prop_assert_eq!(v, View::fit(vp, img), "fit debe ser determinista");

    // Modelo: el mayor nivel de la escalera con ceil(ancho·z) <= viewport.ancho y
    // ceil(alto·z) <= viewport.alto; si ninguno cabe, el mínimo.
    let cabe = |z: Zoom| {
        extension(z, img.width as i128) <= vp.width as i128
            && extension(z, img.height as i128) <= vp.height as i128
    };
    let esperado = Zoom::levels().rev().find(|&z| cabe(z));
    let z = v.zoom();
    prop_assert_eq!(
        z,
        esperado.unwrap_or(Zoom::MIN),
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
        let izquierda = o as i128;
        let derecha = viewport as i128 - o as i128 - ext;
        prop_assert!(
            (izquierda - derecha).abs() <= 1,
            "márgenes {izquierda} y {derecha} en {v:?}"
        );
        // Si la imagen cabe entera, queda dentro del viewport.
        if esperado.is_some() {
            prop_assert!(izquierda >= 0 && derecha >= 0, "imagen fuera en {v:?}");
        }
    }
    Ok(())
}

// Propiedad 1: niveles.

#[test]
fn from_int_solo_de_1_a_64() {
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
fn constantes_de_zoom() {
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
fn zoom_in_sube_de_uno_en_uno_y_satura() {
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
fn zoom_out_baja_de_uno_en_uno_y_satura() {
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
fn zoom_se_ordena_como_su_entero() {
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
fn zoom_hash_es_coherente_con_eq() {
    let todos: HashSet<Zoom> = (1..=64).map(zoom).collect();
    assert_eq!(todos.len(), 64);
    let hash = |z: Zoom| {
        let mut h = DefaultHasher::new();
        z.hash(&mut h);
        h.finish()
    };
    assert_eq!(hash(zoom(5)), hash(zoom(5)));
    assert_eq!(hash(Zoom::ONE), hash(Zoom::default()));
}

#[test]
fn display_de_zoom_usa_el_signo_por() {
    assert_eq!(zoom(4).to_string(), "×4");
    assert_eq!(zoom(4).to_string(), "\u{00D7}4");
    assert_eq!(format!("{}", Zoom::ONE), "×1");
    assert_eq!(format!("{}", Zoom::MAX), "×64");
    assert_eq!(format!("{}", zoom(12)), "×12");
}

// Propiedad 2: scale, unscale y rescale.

#[test]
fn scale_unscale_rescale_ejemplos() {
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
fn scale_unscale_rescale_exhaustivo_en_rango_pequeno() {
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
                let esperado = (n as i128 * ti as i128).div_euclid(zz);
                assert_eq!(
                    z.rescale(n, zoom(ti)) as i128,
                    esperado,
                    "rescale({n}) de ×{zi} a ×{ti}"
                );
            }
        }
    }
}

proptest! {
    #[test]
    fn scale_y_unscale_con_cualquier_n(
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
        let resto = n as i128 - z.unscale(n) as i128 * zi as i128;
        prop_assert!((0..zi as i128).contains(&resto));
    }

    #[test]
    fn rescale_con_cualquier_n(
        zi in 1u32..=64,
        ti in 1u32..=64,
        n in -(1i64 << 50)..=(1i64 << 50),
    ) {
        let (z, to) = (zoom(zi), zoom(ti));
        let esperado = (n as i128 * ti as i128).div_euclid(zi as i128);
        prop_assert_eq!(z.rescale(n, to) as i128, esperado);
        // Reescalar al mismo zoom no cambia nada.
        prop_assert_eq!(z.rescale(n, z), n);
    }
}

// Propiedad 3: ida y vuelta pantalla <-> imagen.

#[test]
fn pantalla_imagen_con_zoom_1() {
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
fn pantalla_imagen_con_zoom_64() {
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
fn pantalla_imagen_con_origen_negativo() {
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
fn ida_y_vuelta_de_todos_los_pixeles_de_una_imagen_pequena() {
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
    fn ida_y_vuelta_exacta_para_todos_los_zooms(
        vpw in dim_viewport(),
        vph in dim_viewport(),
        iw in dim_imagen(),
        ih in dim_imagen(),
        ops in vec(op(1 << 30), 0..4),
        ancla_final in (-(1i64 << 30)..=(1i64 << 30), -(1i64 << 30)..=(1i64 << 30)),
        pix in (any::<u32>(), any::<u32>()),
        desv in (any::<u64>(), any::<u64>()),
        lejos in (-(1i64 << 40)..=(1i64 << 40), -(1i64 << 40)..=(1i64 << 40)),
    ) {
        let (vp, img) = (tam(vpw, vph), tam(iw, ih));
        for n in 1..=64u32 {
            let mut v = vista(vp, img, &ops);
            v.set_zoom(zoom(n), ancla_final);
            prop_assert_eq!(v.zoom(), zoom(n));
            comprobar_geometria(&v, img, pix, desv, lejos)?;
        }
    }

    /// Lo mismo con orígenes pequeños, que es donde los puntos caen más a menudo dentro de la
    /// imagen y en los bordes de los bloques.
    #[test]
    fn ida_y_vuelta_exacta_con_origen_cercano_a_cero(
        iw in 1u32..=20,
        ih in 1u32..=20,
        n in 1u32..=64,
        ox in -40i64..=40,
        oy in -40i64..=40,
        pix in (any::<u32>(), any::<u32>()),
        desv in (any::<u64>(), any::<u64>()),
    ) {
        let img = tam(iw, ih);
        // Con un viewport del tamaño de la imagen a zoom 1, el origen es (0, 0); un set_zoom con
        // ancla (a, b) a zoom n deja el origen en (a - a·n, b - b·n).
        let mut v = View::fit(img, img);
        prop_assert_eq!(v.origin(), (0, 0));
        v.set_zoom(zoom(n), (ox, oy));
        comprobar_geometria(&v, img, pix, desv, (ox, oy))?;
    }
}

// Propiedad 4: teselado e image_rect.

#[test]
fn image_rect_y_teselado_ejemplo() {
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
    fn teselado_sin_huecos_ni_solapes(
        vpw in dim_viewport(),
        vph in dim_viewport(),
        iw in dim_imagen(),
        ih in dim_imagen(),
        ops in vec(op(1 << 30), 0..4),
        ancla_final in (-(1i64 << 30)..=(1i64 << 30), -(1i64 << 30)..=(1i64 << 30)),
        x in any::<u32>(),
        y in any::<u32>(),
    ) {
        let (vp, img) = (tam(vpw, vph), tam(iw, ih));
        for n in 1..=64u32 {
            let mut v = vista(vp, img, &ops);
            v.set_zoom(zoom(n), ancla_final);
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
            let ultimo = v.image_to_screen(iw - 1, ih - 1);
            prop_assert_eq!(ultimo.0 + z, rect.x + rect.width);
            prop_assert_eq!(ultimo.1 + z, rect.y + rect.height);
        }
    }
}

// Propiedad 5: ancla.

#[test]
fn ancla_dentro_de_la_imagen_ejemplo() {
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
fn ancla_fuera_de_la_imagen_conserva_el_indice_de_bloque() {
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
fn ancla_con_imagen_de_un_pixel() {
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
fn zoom_in_y_zoom_out_ejemplos() {
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
    let antes = max;
    max.zoom_in((17, -4));
    assert_eq!(max, antes);
    let mut min = View::fit(tam(8, 8), img);
    min.set_zoom(Zoom::MIN, (0, 0));
    assert_eq!(min.zoom(), Zoom::MIN);
    let antes = min;
    min.zoom_out((17, -4));
    assert_eq!(min, antes);
}

#[test]
fn set_zoom_al_mismo_zoom_no_mueve_la_vista() {
    let img = tam(30, 20);
    let mut v = View::fit(tam(200, 100), img);
    v.set_zoom(zoom(7), (33, 12));
    let antes = v;
    v.set_zoom(zoom(7), (-500, 9999));
    assert_eq!(v, antes);
}

proptest! {
    #[test]
    fn el_bloque_bajo_el_ancla_se_conserva(
        vpw in dim_viewport(),
        vph in dim_viewport(),
        iw in dim_imagen(),
        ih in dim_imagen(),
        ops in vec(op(1 << 30), 0..4),
        dentro in any::<bool>(),
        fuera in (-(1i64 << 31)..=(1i64 << 31), -(1i64 << 31)..=(1i64 << 31)),
        desv in (any::<u64>(), any::<u64>()),
    ) {
        let (vp, img) = (tam(vpw, vph), tam(iw, ih));
        let antes = vista(vp, img, &ops);
        let rect = antes.image_rect(img);
        let ancla = if dentro {
            (
                rect.x + (desv.0 % rect.width as u64) as i64,
                rect.y + (desv.1 % rect.height as u64) as i64,
            )
        } else {
            fuera
        };
        if dentro {
            prop_assert!(antes.screen_to_image(ancla, img).is_some());
        }
        comprobar_ancla_en_todos_los_zooms(&antes, ancla, img)?;
    }

    /// Cambios de zoom encadenados con anclas distintas siguen conservando el bloque en cada paso.
    #[test]
    fn el_bloque_se_conserva_en_cada_paso_de_una_cadena(
        iw in 1u32..=100,
        ih in 1u32..=100,
        pasos in vec((1u32..=64, (-500i64..=500, -500i64..=500)), 1..8),
    ) {
        let img = tam(iw, ih);
        let mut v = View::fit(tam(300, 200), img);
        for (n, ancla) in pasos {
            let antes = v;
            v.set_zoom(zoom(n), ancla);
            comprobar_ancla(&antes, &v, ancla, img)?;
        }
    }

    /// zoom_in y zoom_out equivalen a set_zoom con el nivel siguiente o anterior.
    #[test]
    fn zoom_in_y_zoom_out_equivalen_a_set_zoom(
        iw in 1u32..=100,
        ih in 1u32..=100,
        ops in vec(op(1 << 20), 0..4),
        ancla in (-(1i64 << 20)..=(1i64 << 20), -(1i64 << 20)..=(1i64 << 20)),
    ) {
        let (vp, img) = (tam(300, 200), tam(iw, ih));
        let base = vista(vp, img, &ops);
        let mut mas = base;
        mas.zoom_in(ancla);
        let mut esperado = base;
        esperado.set_zoom(base.zoom().zoom_in(), ancla);
        prop_assert_eq!(mas, esperado);
        let mut menos = base;
        menos.zoom_out(ancla);
        let mut esperado = base;
        esperado.set_zoom(base.zoom().zoom_out(), ancla);
        prop_assert_eq!(menos, esperado);
    }
}

// Propiedad 6: fit.

#[test]
fn fit_ejemplos() {
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
fn fit_cada_zoom_con_la_imagen_justa() {
    // Una imagen de 3x2 en un viewport de 3z x 2z da exactamente z, y en uno de una unidad menos
    // en un eje da el nivel anterior de la escalera (z - 1, o 1/2 desde ×1).
    let img = tam(3, 2);
    let anterior = |n: u32| {
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
        assert_eq!(v.zoom(), anterior(n), "falta 1 px de ancho para ×{n}");
        let v = View::fit(tam(3 * n, 2 * n - 1), img);
        assert_eq!(v.zoom(), anterior(n), "falta 1 px de alto para ×{n}");
    }
}

proptest! {
    #[test]
    fn fit_es_el_mayor_zoom_que_cabe_y_centra(
        vpw in dim_viewport(),
        vph in dim_viewport(),
        iw in dim_imagen(),
        ih in dim_imagen(),
    ) {
        comprobar_fit(tam(vpw, vph), tam(iw, ih))?;
    }

    /// Con imágenes pequeñas y viewports pequeños se cubren todos los zooms y los bordes.
    #[test]
    fn fit_con_tamanos_pequenos(
        vpw in 0u32..=300,
        vph in 0u32..=300,
        iw in 1u32..=30,
        ih in 1u32..=30,
    ) {
        comprobar_fit(tam(vpw, vph), tam(iw, ih))?;
    }
}

// Propiedad 7: clamp y pan.

#[test]
fn pan_dentro_del_rango_desplaza_sin_clamp() {
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
fn pan_lejos_deja_visibles_al_menos_min_visible() {
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
        let inicio = View::fit(vp, img);
        let mut v = inicio;
        v.pan(dx, dy, vp, img);
        assert!(cumple_visibilidad(&v, vp, img), "pan({dx}, {dy}) dio {v:?}");
        assert_eq!(v.zoom(), inicio.zoom());
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
fn clamp_con_imagen_pequena_usa_su_extension() {
    // Imagen de 4x4 a zoom 2: extensión 8 < MIN_VISIBLE, así que debe verse entera (8 px).
    let img = tam(4, 4);
    let vp = tam(100, 100);
    let mut v = View::fit(tam(8, 8), img);
    assert_eq!(v.zoom(), zoom(2));
    for (dx, dy) in [(-500, -500), (500, 500), (-500, 500), (500, -500)] {
        v.pan(dx, dy, vp, img);
        assert!(cumple_visibilidad(&v, vp, img), "pan({dx}, {dy}) dio {v:?}");
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
fn clamp_con_viewport_de_0_y_de_1_pixel() {
    let img = tam(10, 10);
    // Viewport de 0 px: no se exige visibilidad alguna (el mínimo es 0) y no hay pánico.
    let vp0 = tam(0, 0);
    let mut v = View::fit(vp0, img);
    v.pan(1_000_000, -1_000_000, vp0, img);
    v.clamp(vp0, img);
    assert!(cumple_visibilidad(&v, vp0, img));
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
        assert!(
            cumple_visibilidad(&v, vp1, img),
            "pan({dx}, {dy}) dio {v:?}"
        );
        let (ox, oy) = v.origin();
        assert!(ox <= 0 && ox + 10 >= 1, "x: {v:?}");
        assert!(oy <= 0 && oy + 10 >= 1, "y: {v:?}");
    }
}

#[test]
fn clamp_con_imagen_de_un_pixel() {
    // 1x1 a zoom 1: la extensión es 1, así que ese píxel debe seguir visible.
    let img = tam(1, 1);
    let vp = tam(50, 50);
    let mut v = View::fit(tam(1, 1), img);
    assert_eq!(v.zoom(), Zoom::ONE);
    v.pan(10_000, 10_000, vp, img);
    assert!(cumple_visibilidad(&v, vp, img), "{v:?}");
    let (ox, oy) = v.origin();
    assert!((0..50).contains(&ox) && (0..50).contains(&oy), "{v:?}");
    v.pan(-10_000, -10_000, vp, img);
    assert!(cumple_visibilidad(&v, vp, img), "{v:?}");
    let (ox, oy) = v.origin();
    assert!((0..50).contains(&ox) && (0..50).contains(&oy), "{v:?}");
}

#[test]
fn clamp_recupera_una_vista_que_set_zoom_dejo_fuera() {
    // set_zoom no hace clamp; clamp sí devuelve la imagen al viewport.
    let img = tam(4, 4);
    let vp = tam(40, 40);
    let mut v = View::fit(vp, img);
    v.set_zoom(zoom(3), (-7, 100));
    assert_eq!(v.origin(), (-4, 70));
    assert!(!cumple_visibilidad(&v, vp, img));
    v.clamp(vp, img);
    assert!(cumple_visibilidad(&v, vp, img), "{v:?}");
    assert_eq!(v.zoom(), zoom(3));
    let despues = v;
    v.clamp(vp, img);
    assert_eq!(v, despues);
}

#[test]
fn fit_ya_cumple_la_visibilidad_minima_y_clamp_no_la_mueve() {
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
            cumple_visibilidad(&v, vp, img),
            "fit({vp:?}, {img:?}) dio {v:?}"
        );
        let mut c = v;
        c.clamp(vp, img);
        assert_eq!(c, v, "clamp movió fit({vp:?}, {img:?})");
    }
}

proptest! {
    #[test]
    fn clamp_deja_visible_el_minimo_y_es_idempotente(
        vpw in dim_viewport(),
        vph in dim_viewport(),
        iw in dim_imagen(),
        ih in dim_imagen(),
        ops in vec(op(1 << 30), 0..4),
    ) {
        let (vp, img) = (tam(vpw, vph), tam(iw, ih));
        let sin_clamp = vista(vp, img, &ops);
        let mut v = sin_clamp;
        v.clamp(vp, img);
        prop_assert_eq!(v.zoom(), sin_clamp.zoom(), "clamp no cambia el zoom");
        prop_assert!(cumple_visibilidad(&v, vp, img), "clamp de {:?} dio {:?}", sin_clamp, v);
        let mut otra = v;
        otra.clamp(vp, img);
        prop_assert_eq!(otra, v, "clamp debe ser idempotente");
    }

    /// Si la vista ya cumple la visibilidad mínima, clamp no la mueve (se prueba con todas las
    /// vistas generadas que la cumplen, que son muchas porque el viewport suele ser grande).
    #[test]
    fn clamp_no_mueve_una_vista_que_ya_cumple(
        vpw in dim_viewport(),
        vph in dim_viewport(),
        iw in dim_imagen(),
        ih in dim_imagen(),
        ops in vec(op(1 << 12), 0..4),
    ) {
        let (vp, img) = (tam(vpw, vph), tam(iw, ih));
        let v = vista(vp, img, &ops);
        if cumple_visibilidad(&v, vp, img) {
            let mut c = v;
            c.clamp(vp, img);
            prop_assert_eq!(c, v);
        }
    }

    /// `pan` equivale a desplazar el origen y hacer clamp. Como no hay constructor con origen
    /// libre, se comprueba con consecuencias observables, partiendo de una vista ya acotada.
    #[test]
    fn pan_equivale_a_desplazar_y_acotar(
        vpw in dim_viewport(),
        vph in dim_viewport(),
        iw in dim_imagen(),
        ih in dim_imagen(),
        ops in vec(op(1 << 20), 0..4),
        d in (-(1i64 << 22)..=(1i64 << 22), -(1i64 << 22)..=(1i64 << 22)),
    ) {
        let (vp, img) = (tam(vpw, vph), tam(iw, ih));
        let mut inicio = vista(vp, img, &ops);
        inicio.clamp(vp, img);
        let (ox, oy) = inicio.origin();
        let (ext_x, ext_y) = (
            extension(inicio.zoom(), iw as i128),
            extension(inicio.zoom(), ih as i128),
        );
        let (dx, dy) = d;

        let mut v = inicio;
        v.pan(dx, dy, vp, img);
        prop_assert_eq!(v.zoom(), inicio.zoom(), "pan no cambia el zoom");
        prop_assert!(cumple_visibilidad(&v, vp, img), "pan({dx}, {dy}) de {inicio:?} dio {v:?}");

        // Cada eje es independiente del otro.
        let mut solo_x = inicio;
        solo_x.pan(dx, 0, vp, img);
        let mut solo_y = inicio;
        solo_y.pan(0, dy, vp, img);
        prop_assert_eq!(v.origin().0, solo_x.origin().0);
        prop_assert_eq!(v.origin().1, solo_y.origin().1);
        // Un eje sin desplazar no se mueve: la vista de partida ya cumple.
        prop_assert_eq!(solo_x.origin().1, oy);
        prop_assert_eq!(solo_y.origin().0, ox);

        // Si el origen desplazado ya cumple, el resultado es exactamente ese origen.
        for (nuevo, o, delta, ext, viewport, resultado) in [
            (ox as i128 + dx as i128, ox, dx, ext_x, vpw, v.origin().0),
            (oy as i128 + dy as i128, oy, dy, ext_y, vph, v.origin().1),
        ] {
            let nuevo64 = nuevo as i64;
            if eje_cumple(nuevo64, ext, viewport) {
                prop_assert_eq!(resultado, nuevo64, "el origen desplazado cumple y debe respetarse");
            } else {
                // Si no, el acotado deja el origen entre el de partida y el pedido.
                let (lo, hi) = (o.min(nuevo64), o.max(nuevo64));
                prop_assert!(
                    (lo..=hi).contains(&resultado),
                    "resultado {resultado} fuera de [{lo}, {hi}] (delta {delta})"
                );
            }
        }

        // Saturación: pedir el doble de lejos, cuando ya se acotó, da el mismo origen.
        if dx != 0 && v.origin().0 != ox + dx {
            let mut doble = inicio;
            doble.pan(2 * dx, 0, vp, img);
            prop_assert_eq!(doble.origin().0, v.origin().0);
        }
        if dy != 0 && v.origin().1 != oy + dy {
            let mut doble = inicio;
            doble.pan(0, 2 * dy, vp, img);
            prop_assert_eq!(doble.origin().1, v.origin().1);
        }
    }

    /// Desplazamientos que se anulan mientras no se acota nada vuelven al origen de partida.
    #[test]
    fn pan_pequeno_y_su_inverso_vuelven_al_origen(
        iw in 100u32..=300,
        ih in 100u32..=300,
        dx in -20i64..=20,
        dy in -20i64..=20,
    ) {
        // Viewport mucho menor que la imagen a zoom 1: el origen centrado deja de sobra.
        let (vp, img) = (tam(64, 64), tam(iw, ih));
        let inicio = View::fit(vp, img);
        let mut v = inicio;
        v.pan(dx, dy, vp, img);
        prop_assert_eq!(v.origin(), (inicio.origin().0 + dx, inicio.origin().1 + dy));
        v.pan(-dx, -dy, vp, img);
        prop_assert_eq!(v, inicio);
    }
}

// Propiedad 8: sin pánico ni desbordamiento.

#[test]
fn extremos_deterministas_sin_panico() {
    let grandes = (1u32 << 30, 1u32 << 30);
    let tamanos_imagen = [
        tam(1, 1),
        tam(grandes.0, 1),
        tam(1, grandes.1),
        tam(grandes.0, grandes.1),
    ];
    let tamanos_viewport = [
        tam(0, 0),
        tam(1, 1),
        tam(0, grandes.1),
        tam(grandes.0, grandes.1),
    ];
    let lim = 1i64 << 40;
    let anclas = [(0, 0), (lim, lim), (-lim, -lim), (lim, -lim), (-lim, lim)];
    for img in tamanos_imagen {
        for vp in tamanos_viewport {
            comprobar_fit(vp, img).unwrap();
            for ancla in anclas {
                for n in [1u32, 2, 7, 63, 64] {
                    let mut v = View::fit(vp, img);
                    v.set_zoom(zoom(n), ancla);
                    comprobar_ancla_en_todos_los_zooms(&v, ancla, img).unwrap();
                    comprobar_geometria(&v, img, (u32::MAX, u32::MAX), (u64::MAX, u64::MAX), ancla)
                        .unwrap();
                    v.pan(ancla.0, ancla.1, vp, img);
                    assert!(cumple_visibilidad(&v, vp, img), "{v:?}");
                    v.zoom_in(ancla);
                    v.zoom_out(ancla);
                    v.clamp(vp, img);
                    assert!(cumple_visibilidad(&v, vp, img), "{v:?}");
                }
            }
        }
    }
}

proptest! {
    #[test]
    fn sin_panico_con_tamanos_y_coordenadas_extremos(
        vpw in dim_viewport_extrema(),
        vph in dim_viewport_extrema(),
        iw in dim_imagen_extrema(),
        ih in dim_imagen_extrema(),
        ops in vec(op(1 << 40), 0..4),
        ancla in (-(1i64 << 40)..=(1i64 << 40), -(1i64 << 40)..=(1i64 << 40)),
        n in 1u32..=64,
        pan in (-(1i64 << 40)..=(1i64 << 40), -(1i64 << 40)..=(1i64 << 40)),
        pix in (any::<u32>(), any::<u32>()),
        desv in (any::<u64>(), any::<u64>()),
        lejos in (-(1i64 << 40)..=(1i64 << 40), -(1i64 << 40)..=(1i64 << 40)),
    ) {
        let (vp, img) = (tam(vpw, vph), tam(iw, ih));

        // fit: nivel de la escalera, centrada y determinista.
        comprobar_fit(vp, img)?;

        // Una vista con origen y zoom arbitrarios (set_zoom no acota, así que el origen puede
        // quedar muy lejos del viewport).
        let v = vista(vp, img, &ops);
        let z = v.zoom();
        prop_assert!(Zoom::MIN <= z && z <= Zoom::MAX);
        // La geometría por bloques de `comprobar_geometria` es la de los zooms enteros; los
        // fraccionarios (por rejilla) los cubre `zoom_ladder.rs`.
        if z >= Zoom::ONE {
            comprobar_geometria(&v, img, pix, desv, lejos)?;
        }

        // Ancla en cualquier punto, dentro o fuera, con todos los zooms.
        comprobar_ancla_en_todos_los_zooms(&v, ancla, img)?;
        let mut cambio = v;
        cambio.set_zoom(zoom(n), ancla);
        comprobar_geometria(&cambio, img, pix, desv, lejos)?;

        // Clamp y pan: invariantes de visibilidad e idempotencia.
        let mut c = v;
        c.clamp(vp, img);
        prop_assert!(cumple_visibilidad(&c, vp, img), "clamp de {:?} dio {:?}", v, c);
        let mut otra = c;
        otra.clamp(vp, img);
        prop_assert_eq!(otra, c);

        let mut p = cambio;
        p.pan(pan.0, pan.1, vp, img);
        prop_assert!(cumple_visibilidad(&p, vp, img), "pan de {:?} dio {:?}", cambio, p);
        comprobar_geometria(&p, img, pix, desv, lejos)?;
        let mut otra = p;
        otra.clamp(vp, img);
        prop_assert_eq!(otra, p, "tras pan, clamp no debe mover nada");
    }
}
