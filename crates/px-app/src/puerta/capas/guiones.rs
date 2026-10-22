//! Los guiones de punta a punta de la puerta del hito 3 (specs/hito-3-puerta.md, filas 5 a 8): órdenes
//! reales de capa, pintura, mezcla, combinar, aplanar y conversión, con los índices calculados
//! sobre el estado del modelo y los vectores a mano de 3.2 (fila 2 de `hito-3-mezcla.md`, valores
//! contrastados con Aseprite) como `Orden::Vector`.

use px_core::{BlendMode, Document, Image, Mode, Palette, Pixel, Rgba};

use super::modelo::{Celda4, Orden::*};
use super::mundo::Mundo3;
use crate::confirm::Confirm;
use crate::merge_flow::{MergeKind, flatten_gate};
use crate::session::Session;

/// Fondo y origen de los vectores a mano de 3.2.
pub const FONDO: Celda4 = [196, 46, 52, 255];
pub const ORIGEN: Celda4 = [90, 60, 220, 255];

/// Los ocho modos con fondo `FONDO` y origen `ORIGEN` opacos y opacidad 255: valores hechos a
/// mano (3.2, fila 2), no salen de ningún código de Pixoko.
pub const LITERALES: [(BlendMode, Celda4); 8] = [
    (BlendMode::Multiply, [69, 11, 45, 255]),
    (BlendMode::Screen, [217, 95, 227, 255]),
    (BlendMode::Add, [255, 106, 255, 255]),
    (BlendMode::Difference, [106, 14, 168, 255]),
    (BlendMode::Overlay, [179, 22, 90, 255]),
    (BlendMode::Darken, [90, 46, 52, 255]),
    (BlendMode::Lighten, [196, 60, 220, 255]),
    (BlendMode::Normal, [90, 60, 220, 255]),
];

pub fn sesion_rgba() -> Session {
    sesion_rgba_de(8, 8)
}

pub fn sesion_rgba_de(ancho: u32, alto: u32) -> Session {
    let imagen =
        Image::filled(ancho, alto, Pixel::Rgba(Rgba::new(196, 46, 52, 255))).expect("imagen");
    Session::sample(Document::new(imagen))
}

/// Indexada de 8×8: paleta de 16 entradas (la 0 transparente, la 9 con alfa 128) y fondo del 5.
pub fn sesion_indexada() -> Session {
    sesion_indexada_de(8, 8)
}

pub fn sesion_indexada_de(ancho: u32, alto: u32) -> Session {
    let colores = (0..16u8).map(|i| {
        let alfa = if i == 9 { 128 } else { 255 };
        Rgba::new(
            i.wrapping_mul(16),
            255 - i.wrapping_mul(13),
            i.wrapping_mul(37),
            alfa,
        )
    });
    let mut paleta = Palette::from_colors(colores).expect("paleta");
    paleta.set_transparent_index(Some(0)).expect("transparente");
    let imagen = Image::filled_indexed(ancho, alto, paleta, 5).expect("imagen indexada");
    Session::sample(Document::new(imagen))
}

fn i(n: u8) -> Celda4 {
    [n, 0, 0, 0]
}

/// RGBA de punta a punta: ocho modos con sus vectores, opacidad, pintura en tres capas, duplicar,
/// mover, ocultas y bloqueadas (rechazos), combinar abajo, combinar visibles y aplanar.
pub fn guion_rgba(w: &mut Mundo3) {
    w.aplicar(&Vector((0, 0), FONDO));
    assert!(w.aplicar(&Nueva));
    assert!(w.aplicar(&Relleno((0, 0), ORIGEN)));
    w.aplicar(&Vector((0, 0), ORIGEN));
    for (modo, esperado) in LITERALES {
        let cambia = w.m.capas[1].mezcla != modo;
        assert_eq!(w.aplicar(&Mezcla(1, modo)), cambia);
        w.aplicar(&Vector((0, 0), esperado));
    }
    assert!(!w.aplicar(&Opacidad(1, 255)), "misma opacidad: ningún paso");
    w.aplicar(&Mezcla(1, BlendMode::Multiply));
    w.aplicar(&Opacidad(1, 128));
    // Pintura sobre la capa 1 (lápiz translúcido, línea a 45°, borrador y relleno).
    w.aplicar(&Lapiz(vec![(1, 1), (4, 1), (4, 4)], [200, 100, 0, 128]));
    w.aplicar(&Linea((0, 7), (7, 0), [10, 200, 10, 255]));
    w.aplicar(&Borrador(vec![(0, 0), (3, 0)]));
    w.aplicar(&Relleno((7, 7), [30, 30, 200, 255]));
    // Pintura sobre la capa 0.
    w.aplicar(&Activar(0));
    w.aplicar(&Lapiz(vec![(2, 2), (2, 5)], [255, 255, 0, 255]));
    w.aplicar(&Relleno((7, 0), [0, 0, 0, 255]));
    // Duplicar, renombrar, mover (en el extremo no hay paso) y propiedades de la copia.
    w.aplicar(&Activar(1));
    assert!(w.aplicar(&Duplicar));
    w.aplicar(&Renombrar(2, "copiada".to_string()));
    assert!(
        !w.aplicar(&Renombrar(2, "copiada".to_string())),
        "mismo nombre: ningún paso"
    );
    assert!(w.aplicar(&Mover(-1)));
    assert!(w.aplicar(&Mover(1)));
    assert!(!w.aplicar(&Mover(1)), "en el extremo no hay paso");
    w.aplicar(&Mezcla(2, BlendMode::Screen));
    w.aplicar(&Opacidad(2, 200));
    // Oculta y bloqueada: pintar se rechaza sin paso.
    w.aplicar(&Visible(2, false));
    assert!(!w.aplicar(&Lapiz(vec![(5, 5)], [1, 2, 3, 255])));
    w.aplicar(&Visible(2, true));
    w.aplicar(&Bloqueo(2, true));
    assert!(!w.aplicar(&Relleno((0, 0), [9, 9, 9, 255])));
    assert!(!w.aplicar(&Borrador(vec![(1, 1)])));
    w.aplicar(&Bloqueo(2, false));
    // Capas nuevas: una translúcida con Superponer y otra, con píxeles, que se oculta.
    w.aplicar(&Nueva);
    w.aplicar(&Relleno((3, 3), [255, 255, 255, 100]));
    w.aplicar(&Mezcla(3, BlendMode::Overlay));
    w.aplicar(&Nueva);
    w.aplicar(&Lapiz(vec![(0, 0), (7, 7)], [0, 255, 255, 255]));
    w.aplicar(&Visible(4, false));
    assert_eq!(w.m.capas.len(), 5);
    // Con varias capas la conversión de modo se rechaza sin tocar nada.
    w.rechaza_convertir(Mode::Indexed);
    // Combinar abajo: con la activa oculta no; con la de abajo bloqueada no; si no, sí.
    assert!(!w.aplicar(&Combinar(MergeKind::Down)));
    w.aplicar(&Activar(3));
    assert!(w.aplicar(&Combinar(MergeKind::Down)));
    w.aplicar(&Bloqueo(1, true));
    w.aplicar(&Activar(2));
    assert!(!w.aplicar(&Combinar(MergeKind::Down)));
    w.aplicar(&Bloqueo(1, false));
    // Combinar visibles deja la oculta; aplanar la quita tras la confirmación de 5 s.
    assert!(w.aplicar(&Combinar(MergeKind::Visible)));
    assert_eq!(w.m.capas.len(), 2);
    assert!(
        !w.aplicar(&Combinar(MergeKind::Down)),
        "la de abajo no tiene nada debajo"
    );
    confirmacion_de_aplanar(w);
    assert!(w.aplicar(&Combinar(MergeKind::Flatten)));
    assert_eq!(w.m.capas.len(), 1);
    assert!(
        !w.aplicar(&Combinar(MergeKind::Flatten)),
        "una capa no se aplana"
    );
    assert!(!w.aplicar(&Combinar(MergeKind::Visible)));
    assert!(!w.aplicar(&Quitar), "la última capa no se quita");
}

/// La regla de `Aplanar` con ocultas con píxeles: la primera petición avisa y la repetida en 5 s lo
/// hace; pasado ese tiempo vuelve a avisar; sin ocultas se hace a la primera.
fn confirmacion_de_aplanar(w: &Mundo3) {
    let ocultas = w.s.flatten_hidden_with_pixels();
    assert_eq!(ocultas, 1, "la capa oculta con píxeles");
    let mut c = Confirm::new();
    assert!(!flatten_gate(ocultas, &mut c, 0.0));
    assert!(flatten_gate(ocultas, &mut c, 1.0));
    let mut c = Confirm::new();
    assert!(!flatten_gate(ocultas, &mut c, 0.0));
    assert!(
        !flatten_gate(ocultas, &mut c, 6.0),
        "pasados 5 s hay que volver a pedirlo"
    );
    assert!(flatten_gate(0, &mut c, 7.0));
}

/// Indexado de punta a punta: lo que admite (capas, visibilidad, bloqueo, pintura, combinar,
/// aplanar), lo que rechaza (opacidad y mezcla) y las conversiones exactas a RGBA y de vuelta.
pub fn guion_indexado(w: &mut Mundo3) {
    assert!(w.aplicar(&Nueva));
    assert!(w.aplicar(&Relleno((0, 0), i(3))));
    assert!(
        !w.aplicar(&Opacidad(1, 128)),
        "el indexado no tiene opacidad"
    );
    assert!(!w.aplicar(&Mezcla(1, BlendMode::Multiply)), "ni mezcla");
    w.aplicar(&Lapiz(vec![(1, 1), (5, 1)], i(0)));
    w.aplicar(&Linea((0, 0), (7, 7), i(7)));
    w.aplicar(&Borrador(vec![(6, 0), (6, 3)]));
    w.aplicar(&Activar(0));
    w.aplicar(&Lapiz(vec![(2, 2), (2, 6)], i(9)));
    w.aplicar(&Relleno((7, 7), i(12)));
    w.aplicar(&Activar(1));
    assert!(w.aplicar(&Duplicar));
    w.aplicar(&Renombrar(2, "copia".to_string()));
    assert!(w.aplicar(&Mover(-1)));
    assert!(w.aplicar(&Mover(1)));
    w.aplicar(&Visible(2, false));
    assert!(!w.aplicar(&Lapiz(vec![(5, 5)], i(1))));
    w.aplicar(&Visible(2, true));
    w.aplicar(&Bloqueo(2, true));
    assert!(!w.aplicar(&Relleno((0, 0), i(2))));
    w.aplicar(&Bloqueo(2, false));
    w.aplicar(&Nueva);
    w.aplicar(&Lapiz(vec![(0, 7), (7, 0)], i(4)));
    w.aplicar(&Nueva);
    w.aplicar(&Relleno((0, 0), i(2)));
    w.aplicar(&Visible(4, false));
    w.rechaza_convertir(Mode::Rgba);
    assert!(
        !w.aplicar(&Combinar(MergeKind::Down)),
        "la activa está oculta"
    );
    w.aplicar(&Activar(3));
    assert!(w.aplicar(&Combinar(MergeKind::Down)));
    assert!(w.aplicar(&Combinar(MergeKind::Visible)));
    // Indexado → RGBA, exacto y en un solo paso; en RGBA ya hay opacidad y mezcla.
    assert!(w.aplicar(&Convertir(Mode::Rgba)));
    w.aplicar(&Nueva);
    let arriba = w.m.capas.len() - 1;
    w.aplicar(&Relleno((2, 2), [100, 200, 50, 128]));
    w.aplicar(&Mezcla(arriba, BlendMode::Screen));
    w.aplicar(&Opacidad(arriba, 200));
    // Para volver a indexado todo ha de ser Normal y opaco (el núcleo lo exige).
    w.aplicar(&Mezcla(arriba, BlendMode::Normal));
    w.aplicar(&Opacidad(arriba, 255));
    w.aplicar(&Visible(1, true));
    assert!(
        w.aplicar(&Convertir(Mode::Indexed)),
        "RGBA → indexado exacto"
    );
    assert!(w.aplicar(&Combinar(MergeKind::Flatten)));
    assert_eq!(w.m.capas.len(), 1);
}

/// Un guion corto con varias capas y propiedades variadas, para guardar y reabrir (fila 8): en
/// RGBA, `FONDO`, `ORIGEN` en Multiplicar (el píxel `(0, 0)` compone a `(69, 11, 45, 255)`), una
/// capa translúcida con Trama y otra oculta y bloqueada.
pub fn guion_corto_rgba(w: &mut Mundo3) {
    w.aplicar(&Nueva);
    w.aplicar(&Relleno((0, 0), ORIGEN));
    w.aplicar(&Mezcla(1, BlendMode::Multiply));
    w.aplicar(&Nueva);
    w.aplicar(&Lapiz(vec![(3, 3), (6, 3), (6, 6)], [200, 100, 0, 128]));
    w.aplicar(&Mezcla(2, BlendMode::Screen));
    w.aplicar(&Opacidad(2, 128));
    w.aplicar(&Nueva);
    w.aplicar(&Lapiz(vec![(0, 7), (7, 0)], [0, 255, 255, 255]));
    w.aplicar(&Renombrar(3, "ñandú".to_string()));
    w.aplicar(&Visible(3, false));
    w.aplicar(&Bloqueo(3, true));
    w.aplicar(&Vector((0, 0), [69, 11, 45, 255]));
}

/// Lo mismo en indexado (sin opacidad ni mezcla).
pub fn guion_corto_indexado(w: &mut Mundo3) {
    w.aplicar(&Nueva);
    w.aplicar(&Relleno((0, 0), i(3)));
    w.aplicar(&Lapiz(vec![(1, 1), (5, 1)], i(0)));
    w.aplicar(&Nueva);
    w.aplicar(&Lapiz(vec![(0, 7), (7, 0)], i(7)));
    w.aplicar(&Renombrar(2, "ñandú".to_string()));
    w.aplicar(&Visible(2, false));
    w.aplicar(&Bloqueo(2, true));
}
