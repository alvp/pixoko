//! Tests de `View::screen_to_image_i32` (specs/hito-2-lapiz-deshacer.md, fila 13).
//!
//! Es `screen_to_image` sin el `None` de fuera de la imagen: el índice de bloque
//! `floor((p - origen) / zoom)` en cada eje, saturado a `i32`. `View` solo se construye con
//! `View::fit`; la vista de la fila 13 (zoom ×4, origen (10, 10)) sale de `fit`, `set_zoom` y
//! `pan`, y se comprueba con `zoom()` y `origin()` antes de usarla.

use proptest::prelude::*;
use px_ui::view::{Size, View, Zoom};

fn zoom(n: u32) -> Zoom {
    Zoom::from_int(n).unwrap_or_else(|| panic!("el zoom {n} debería existir"))
}

/// Zoom ×4 con origen (10, 10): se parte de un viewport del tamaño de la imagen (zoom 1 y
/// origen (0, 0)), se sube a ×4 con el ancla en el origen y se desplaza 10 en cada eje (a 90
/// píxeles visibles, más que `MIN_VISIBLE`, así que `pan` no acota nada).
fn vista_x4_en_10_10() -> (View, Size) {
    let tam = Size::new(100, 100);
    let mut v = View::fit(tam, tam);
    v.set_zoom(zoom(4), (0, 0));
    v.pan(10, 10, tam, tam);
    assert_eq!(v.zoom(), zoom(4), "preparación de la vista");
    assert_eq!(v.origin(), (10, 10), "preparación de la vista");
    (v, tam)
}

#[test]
fn fila_13_zoom_x4_con_origen_10_10() {
    let (v, _) = vista_x4_en_10_10();
    assert_eq!(v.screen_to_image_i32((10, 10)), (0, 0));
    assert_eq!(v.screen_to_image_i32((9, 9)), (-1, -1));
    assert_eq!(
        v.screen_to_image_i32((i64::MIN, i64::MAX)),
        (i32::MIN, i32::MAX)
    );
}

#[test]
fn los_bloques_de_cuatro_pixeles_y_los_signos_de_cada_eje_son_independientes() {
    let (v, _) = vista_x4_en_10_10();
    // Dentro del primer bloque y en el siguiente.
    assert_eq!(v.screen_to_image_i32((13, 13)), (0, 0));
    assert_eq!(v.screen_to_image_i32((14, 14)), (1, 1));
    // Antes del origen: floor hacia -infinito, no hacia cero.
    assert_eq!(v.screen_to_image_i32((6, 6)), (-1, -1));
    assert_eq!(v.screen_to_image_i32((5, 5)), (-2, -2));
    // Cada eje por separado.
    assert_eq!(v.screen_to_image_i32((10, 9)), (0, -1));
    assert_eq!(v.screen_to_image_i32((9, 14)), (-1, 1));
    // Fuera de la imagen por la derecha: sigue dando el índice, no `None`.
    assert_eq!(v.screen_to_image_i32((10 + 4 * 150, 10)), (150, 0));
    // Los dos extremos de `i64` en el eje contrario.
    assert_eq!(
        v.screen_to_image_i32((i64::MAX, i64::MIN)),
        (i32::MAX, i32::MIN)
    );
}

#[test]
fn dentro_de_la_imagen_coincide_con_screen_to_image() {
    let (v, img) = vista_x4_en_10_10();
    for p in [(10, 10), (13, 25), (50, 60), (409, 409)] {
        // El bloque (99, 99) llega hasta 10 + 400 - 1 = 409.
        let (x, y) = v.screen_to_image(p, img).expect("dentro de la imagen");
        assert_eq!(v.screen_to_image_i32(p), (x as i32, y as i32), "{p:?}");
    }
}

/// Modelo: `floor((p - origen) / z)` en `i128`, saturado a `i32`.
fn modelo(p: i64, origen: i64, z: u32) -> i32 {
    let bloque = (i128::from(p) - i128::from(origen)).div_euclid(i128::from(z));
    bloque.clamp(i128::from(i32::MIN), i128::from(i32::MAX)) as i32
}

fn coordenada() -> impl Strategy<Value = i64> {
    prop_oneof![
        4 => -(1i64 << 40)..=(1i64 << 40),
        3 => -200i64..=200,
        1 => Just(i64::MIN),
        1 => Just(i64::MAX),
        1 => Just(i64::from(i32::MIN) * 64),
        1 => Just(i64::from(i32::MAX) * 64),
    ]
}

proptest! {
    #[test]
    fn coincide_con_el_modelo_y_con_screen_to_image(
        vpw in 0u32..=500,
        vph in 0u32..=500,
        iw in 1u32..=100,
        ih in 1u32..=100,
        n in 1u32..=64,
        ancla in (-(1i64 << 20)..=(1i64 << 20), -(1i64 << 20)..=(1i64 << 20)),
        desplazamiento in (-1000i64..=1000, -1000i64..=1000),
        p in (coordenada(), coordenada()),
    ) {
        let (vp, img) = (Size::new(vpw, vph), Size::new(iw, ih));
        let mut v = View::fit(vp, img);
        v.set_zoom(zoom(n), ancla);
        v.pan(desplazamiento.0, desplazamiento.1, vp, img);
        let (ox, oy) = v.origin();
        let z = v.zoom().scale(1) as u32;

        let obtenido = v.screen_to_image_i32(p);
        prop_assert_eq!(obtenido, (modelo(p.0, ox, z), modelo(p.1, oy, z)));
        // Cuando el punto cae sobre la imagen, es el mismo píxel.
        if let Some((x, y)) = v.screen_to_image(p, img) {
            prop_assert_eq!(obtenido, (x as i32, y as i32));
        }
        // Ida y vuelta: la esquina de cada bloque da su propio índice.
        let (cx, cy) = v.image_to_screen(iw - 1, ih - 1);
        prop_assert_eq!(v.screen_to_image_i32((cx, cy)), (iw as i32 - 1, ih as i32 - 1));
    }
}
