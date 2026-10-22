//! Los guiones de punta a punta de la puerta del hito 3 (specs/hito-3-puerta.md, filas 5 a 8): órdenes
//! reales de capa, pintura, mezcla, combinar, aplanar y conversión, con los índices calculados
//! sobre el estado del modelo y los vectores a mano de 3.2 (fila 2 de `hito-3-blend_2.md`, valores
//! contrastados con Aseprite) como `Order::Vector`.

use px_core::{BlendMode, Document, Image, Mode, Palette, Pixel, Rgba};

use super::model::{Cel4, Order::*};
use super::world::World3;
use crate::confirm::Confirm;
use crate::merge_flow::{MergeKind, flatten_gate};
use crate::session::Session;

/// Fondo y origen de los vectores a mano de 3.2.
pub const TEST_BACKGROUND: Cel4 = [196, 46, 52, 255];
pub const SOURCE: Cel4 = [90, 60, 220, 255];

/// Los ocho modos con fondo `TEST_BACKGROUND` y origen `SOURCE` opacos y opacidad 255: valores hechos a
/// mano (3.2, fila 2), no salen de ningún código de Pixoko.
pub const LITERALS: [(BlendMode, Cel4); 8] = [
    (BlendMode::Multiply, [69, 11, 45, 255]),
    (BlendMode::Screen, [217, 95, 227, 255]),
    (BlendMode::Add, [255, 106, 255, 255]),
    (BlendMode::Difference, [106, 14, 168, 255]),
    (BlendMode::Overlay, [179, 22, 90, 255]),
    (BlendMode::Darken, [90, 46, 52, 255]),
    (BlendMode::Lighten, [196, 60, 220, 255]),
    (BlendMode::Normal, [90, 60, 220, 255]),
];

pub fn session_rgba() -> Session {
    session_rgba_of(8, 8)
}

pub fn session_rgba_of(width: u32, height: u32) -> Session {
    let image_2 =
        Image::filled(width, height, Pixel::Rgba(Rgba::new(196, 46, 52, 255))).expect("imagen");
    Session::sample(Document::new(image_2))
}

/// Indexada de 8×8: paleta de 16 entradas (la 0 transparente, la 9 con alfa 128) y fondo del 5.
pub fn session_indexed_2() -> Session {
    session_indexed_of(8, 8)
}

pub fn session_indexed_of(width: u32, height: u32) -> Session {
    let colors = (0..16u8).map(|i| {
        let alpha_2 = if i == 9 { 128 } else { 255 };
        Rgba::new(
            i.wrapping_mul(16),
            255 - i.wrapping_mul(13),
            i.wrapping_mul(37),
            alpha_2,
        )
    });
    let mut palette = Palette::from_colors(colors).expect("paleta");
    palette
        .set_transparent_index(Some(0))
        .expect("transparente");
    let image_2 = Image::filled_indexed(width, height, palette, 5).expect("imagen indexada");
    Session::sample(Document::new(image_2))
}

fn i(n: u8) -> Cel4 {
    [n, 0, 0, 0]
}

/// RGBA de punta a punta: ocho modos con sus vectores, opacidad, pintura en tres capas, duplicar,
/// mover, ocultas y bloqueadas (rechazos), combinar abajo, combinar visibles y aplanar.
pub fn script_rgba(w: &mut World3) {
    w.apply(&Vector((0, 0), TEST_BACKGROUND));
    assert!(w.apply(&New));
    assert!(w.apply(&Fill((0, 0), SOURCE)));
    w.apply(&Vector((0, 0), SOURCE));
    for (mode_2, expected_2) in LITERALS {
        let changes = w.m.layers_2[1].blend_2 != mode_2;
        assert_eq!(w.apply(&Blend(1, mode_2)), changes);
        w.apply(&Vector((0, 0), expected_2));
    }
    assert!(!w.apply(&Opacity(1, 255)), "misma opacidad: ningún paso");
    w.apply(&Blend(1, BlendMode::Multiply));
    w.apply(&Opacity(1, 128));
    // Pintura sobre la capa 1 (lápiz translúcido, línea a 45°, borrador y relleno).
    w.apply(&Pencil(vec![(1, 1), (4, 1), (4, 4)], [200, 100, 0, 128]));
    w.apply(&Line((0, 7), (7, 0), [10, 200, 10, 255]));
    w.apply(&Eraser(vec![(0, 0), (3, 0)]));
    w.apply(&Fill((7, 7), [30, 30, 200, 255]));
    // Pintura sobre la capa 0.
    w.apply(&Activate(0));
    w.apply(&Pencil(vec![(2, 2), (2, 5)], [255, 255, 0, 255]));
    w.apply(&Fill((7, 0), [0, 0, 0, 255]));
    // Duplicar, renombrar, mover (en el extremo no hay paso) y propiedades de la copia.
    w.apply(&Activate(1));
    assert!(w.apply(&Duplicate));
    w.apply(&Rename(2, "copiada".to_string()));
    assert!(
        !w.apply(&Rename(2, "copiada".to_string())),
        "mismo nombre: ningún paso"
    );
    assert!(w.apply(&Mover(-1)));
    assert!(w.apply(&Mover(1)));
    assert!(!w.apply(&Mover(1)), "en el extremo no hay paso");
    w.apply(&Blend(2, BlendMode::Screen));
    w.apply(&Opacity(2, 200));
    // Oculta y bloqueada: pintar se rechaza sin paso.
    w.apply(&Visible(2, false));
    assert!(!w.apply(&Pencil(vec![(5, 5)], [1, 2, 3, 255])));
    w.apply(&Visible(2, true));
    w.apply(&Lock(2, true));
    assert!(!w.apply(&Fill((0, 0), [9, 9, 9, 255])));
    assert!(!w.apply(&Eraser(vec![(1, 1)])));
    w.apply(&Lock(2, false));
    // Capas nuevas: una translúcida con Superponer y otra, con píxeles, que se oculta.
    w.apply(&New);
    w.apply(&Fill((3, 3), [255, 255, 255, 100]));
    w.apply(&Blend(3, BlendMode::Overlay));
    w.apply(&New);
    w.apply(&Pencil(vec![(0, 0), (7, 7)], [0, 255, 255, 255]));
    w.apply(&Visible(4, false));
    assert_eq!(w.m.layers_2.len(), 5);
    // Con varias capas la conversión de modo se rechaza sin tocar nada.
    w.rejects_convert(Mode::Indexed);
    // Combinar abajo: con la activa oculta no; con la de abajo bloqueada no; si no, sí.
    assert!(!w.apply(&Merge(MergeKind::Down)));
    w.apply(&Activate(3));
    assert!(w.apply(&Merge(MergeKind::Down)));
    w.apply(&Lock(1, true));
    w.apply(&Activate(2));
    assert!(!w.apply(&Merge(MergeKind::Down)));
    w.apply(&Lock(1, false));
    // Combinar visibles deja la oculta; aplanar la quita tras la confirmación de 5 s.
    assert!(w.apply(&Merge(MergeKind::Visible)));
    assert_eq!(w.m.layers_2.len(), 2);
    assert!(
        !w.apply(&Merge(MergeKind::Down)),
        "la de abajo no tiene nada debajo"
    );
    confirmation_of_flatten(w);
    assert!(w.apply(&Merge(MergeKind::Flatten)));
    assert_eq!(w.m.layers_2.len(), 1);
    assert!(
        !w.apply(&Merge(MergeKind::Flatten)),
        "una capa no se aplana"
    );
    assert!(!w.apply(&Merge(MergeKind::Visible)));
    assert!(!w.apply(&Remove), "la última capa no se quita");
}

/// La regla de `Aplanar` con ocultas con píxeles: la primera petición avisa y la repetida en 5 s lo
/// hace; pasado ese tiempo vuelve a avisar; sin ocultas se hace a la primera.
fn confirmation_of_flatten(w: &World3) {
    let hidden = w.s.flatten_hidden_with_pixels();
    assert_eq!(hidden, 1, "la capa oculta con píxeles");
    let mut c = Confirm::new();
    assert!(!flatten_gate(hidden, &mut c, 0.0));
    assert!(flatten_gate(hidden, &mut c, 1.0));
    let mut c = Confirm::new();
    assert!(!flatten_gate(hidden, &mut c, 0.0));
    assert!(
        !flatten_gate(hidden, &mut c, 6.0),
        "pasados 5 s hay que volver a pedirlo"
    );
    assert!(flatten_gate(0, &mut c, 7.0));
}

/// Indexado de punta a punta: lo que admite (capas, visibilidad, bloqueo, pintura, combinar,
/// aplanar), lo que rechaza (opacidad y mezcla) y las conversiones exactas a RGBA y de vuelta.
pub fn script_indexed(w: &mut World3) {
    assert!(w.apply(&New));
    assert!(w.apply(&Fill((0, 0), i(3))));
    assert!(!w.apply(&Opacity(1, 128)), "el indexado no tiene opacidad");
    assert!(!w.apply(&Blend(1, BlendMode::Multiply)), "ni mezcla");
    w.apply(&Pencil(vec![(1, 1), (5, 1)], i(0)));
    w.apply(&Line((0, 0), (7, 7), i(7)));
    w.apply(&Eraser(vec![(6, 0), (6, 3)]));
    w.apply(&Activate(0));
    w.apply(&Pencil(vec![(2, 2), (2, 6)], i(9)));
    w.apply(&Fill((7, 7), i(12)));
    w.apply(&Activate(1));
    assert!(w.apply(&Duplicate));
    w.apply(&Rename(2, "copia".to_string()));
    assert!(w.apply(&Mover(-1)));
    assert!(w.apply(&Mover(1)));
    w.apply(&Visible(2, false));
    assert!(!w.apply(&Pencil(vec![(5, 5)], i(1))));
    w.apply(&Visible(2, true));
    w.apply(&Lock(2, true));
    assert!(!w.apply(&Fill((0, 0), i(2))));
    w.apply(&Lock(2, false));
    w.apply(&New);
    w.apply(&Pencil(vec![(0, 7), (7, 0)], i(4)));
    w.apply(&New);
    w.apply(&Fill((0, 0), i(2)));
    w.apply(&Visible(4, false));
    w.rejects_convert(Mode::Rgba);
    assert!(!w.apply(&Merge(MergeKind::Down)), "la activa está oculta");
    w.apply(&Activate(3));
    assert!(w.apply(&Merge(MergeKind::Down)));
    assert!(w.apply(&Merge(MergeKind::Visible)));
    // Indexado → RGBA, exacto y en un solo paso; en RGBA ya hay opacidad y mezcla.
    assert!(w.apply(&Convert(Mode::Rgba)));
    w.apply(&New);
    let top = w.m.layers_2.len() - 1;
    w.apply(&Fill((2, 2), [100, 200, 50, 128]));
    w.apply(&Blend(top, BlendMode::Screen));
    w.apply(&Opacity(top, 200));
    // Para volver a indexado todo ha de ser Normal y opaco (el núcleo lo exige).
    w.apply(&Blend(top, BlendMode::Normal));
    w.apply(&Opacity(top, 255));
    w.apply(&Visible(1, true));
    assert!(w.apply(&Convert(Mode::Indexed)), "RGBA → indexado exacto");
    assert!(w.apply(&Merge(MergeKind::Flatten)));
    assert_eq!(w.m.layers_2.len(), 1);
}

/// Un guion corto con varias capas y propiedades variadas, para guardar y reabrir (fila 8): en
/// RGBA, `TEST_BACKGROUND`, `SOURCE` en Multiplicar (el píxel `(0, 0)` compone a `(69, 11, 45, 255)`), una
/// capa translúcida con Trama y otra oculta y bloqueada.
pub fn script_short_rgba(w: &mut World3) {
    w.apply(&New);
    w.apply(&Fill((0, 0), SOURCE));
    w.apply(&Blend(1, BlendMode::Multiply));
    w.apply(&New);
    w.apply(&Pencil(vec![(3, 3), (6, 3), (6, 6)], [200, 100, 0, 128]));
    w.apply(&Blend(2, BlendMode::Screen));
    w.apply(&Opacity(2, 128));
    w.apply(&New);
    w.apply(&Pencil(vec![(0, 7), (7, 0)], [0, 255, 255, 255]));
    w.apply(&Rename(3, "ñandú".to_string()));
    w.apply(&Visible(3, false));
    w.apply(&Lock(3, true));
    w.apply(&Vector((0, 0), [69, 11, 45, 255]));
}

/// Lo mismo en indexado (sin opacidad ni mezcla).
pub fn script_short_indexed(w: &mut World3) {
    w.apply(&New);
    w.apply(&Fill((0, 0), i(3)));
    w.apply(&Pencil(vec![(1, 1), (5, 1)], i(0)));
    w.apply(&New);
    w.apply(&Pencil(vec![(0, 7), (7, 0)], i(7)));
    w.apply(&Rename(2, "ñandú".to_string()));
    w.apply(&Visible(2, false));
    w.apply(&Lock(2, true));
}
