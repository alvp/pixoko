//! Tests de `stroke::Stroke`, la lógica de trazo pura (specs/hito-2-lapiz-deshacer.md,
//! secciones 1 y 2; filas 1 a 10 de «Casos límite» y propiedades 1 a 4).
//!
//! Las filas son listas literales calculadas a mano. Los oráculos de las propiedades se escriben
//! aparte con `px_raster::Line` (la geometría) y un `HashMap` (la regla «la última escritura
//! gana»); no comparten código con `Stroke`.
//!
//! Los lienzos y las imágenes miden como mucho 16 x 16, y las posiciones son `i32` arbitrarios:
//! `Stroke` recorta antes de recorrer, así que ninguna prueba recorre un segmento grande.

use std::collections::{BTreeSet, HashMap, VecDeque};

use proptest::collection::vec;
use proptest::prelude::*;
use proptest::test_runner::TestCaseError;
use px_core::{
    Command, Document, DocumentError, Image, ImageError, Mode, Palette, Pixel, PixelEdit, Rgba,
};
use px_raster::Line;
use px_ui::stroke::Stroke;

const MIN: i32 = i32::MIN;
const MAX: i32 = i32::MAX;

type Punto = (i32, i32);

// Apoyo común.

fn pixel_a() -> Pixel {
    Pixel::Rgba(Rgba::opaque(200, 40, 40))
}

fn pixel_b() -> Pixel {
    Pixel::Rgba(Rgba::opaque(40, 40, 200))
}

/// Un píxel RGBA distinto para cada `k` (hasta 256).
fn pixel_k(k: usize) -> Pixel {
    Pixel::Rgba(Rgba::new(k as u8, (k * 7) as u8, 3, 255))
}

fn edicion(x: i32, y: i32, pixel: Pixel) -> PixelEdit {
    PixelEdit {
        x: u32::try_from(x).expect("dentro del lienzo"),
        y: u32::try_from(y).expect("dentro del lienzo"),
        pixel,
    }
}

/// Las ediciones esperadas de una lista de coordenadas con un mismo píxel.
fn ediciones(coords: &[Punto], pixel: Pixel) -> Vec<PixelEdit> {
    coords.iter().map(|&(x, y)| edicion(x, y, pixel)).collect()
}

/// Crea un trazo en un lienzo de `width` x `height` y le da las posiciones, cada una con su píxel.
fn trazo(width: u32, height: u32, posiciones: &[(Punto, Pixel)]) -> Stroke {
    let mut stroke = Stroke::new(width, height);
    for &(pos, pixel) in posiciones {
        stroke.move_to(pos, pixel);
    }
    stroke
}

/// Como `trazo`, con el mismo píxel en todas las posiciones.
fn trazo_con(width: u32, height: u32, posiciones: &[Punto], pixel: Pixel) -> Stroke {
    let mut stroke = Stroke::new(width, height);
    for &pos in posiciones {
        stroke.move_to(pos, pixel);
    }
    stroke
}

fn coordenadas(stroke: &Stroke) -> Vec<Punto> {
    stroke
        .pixels()
        .iter()
        .map(|e| (e.x as i32, e.y as i32))
        .collect()
}

fn doc_rgba(width: u32, height: u32) -> Document {
    Document::new(Image::new(width, height, Mode::Rgba).unwrap())
}

fn imagen_de(doc: &Document) -> Image {
    doc.image(doc.main_image()).unwrap().clone()
}

/// Todo lo observable de un documento, para comprobar que una operación no tocó nada.
fn instantanea(doc: &Document) -> (Document, bool, bool, usize) {
    (
        doc.clone(),
        doc.can_undo(),
        doc.can_redo(),
        doc.history_bytes(),
    )
}

fn dentro(p: Punto, width: u32, height: u32) -> bool {
    (0..i64::from(width)).contains(&i64::from(p.0))
        && (0..i64::from(height)).contains(&i64::from(p.1))
}

// Filas 1 a 7: la lista de píxeles.

#[test]
fn fila_1_una_posicion_pinta_un_pixel() {
    let mut stroke = Stroke::new(8, 8);
    assert!(stroke.is_empty());
    assert_eq!(stroke.last(), None);
    stroke.move_to((2, 3), pixel_a());
    assert_eq!(stroke.pixels(), [edicion(2, 3, pixel_a())]);
    assert!(!stroke.is_empty());
    assert_eq!(stroke.last(), Some((2, 3)));
}

#[test]
fn fila_2_la_repetida_se_ignora_y_el_salto_no_deja_huecos() {
    let stroke = trazo_con(8, 8, &[(1, 1), (1, 1), (4, 1)], pixel_a());
    assert_eq!(
        stroke.pixels(),
        ediciones(&[(1, 1), (2, 1), (3, 1), (4, 1)], pixel_a())
    );
    assert_eq!(stroke.last(), Some((4, 1)));
}

#[test]
fn fila_3_un_cruce_visita_dos_veces_y_sale_una() {
    let stroke = trazo_con(8, 8, &[(0, 2), (4, 2), (2, 0), (2, 4)], pixel_a());
    let esperado = [
        (0, 2),
        (1, 2),
        (2, 2),
        (3, 2),
        (4, 2),
        (3, 1),
        (2, 0),
        (2, 1),
        (2, 3),
        (2, 4),
    ];
    assert_eq!(stroke.pixels().len(), 10);
    assert_eq!(coordenadas(&stroke), esperado);
}

#[test]
fn fila_4_la_ultima_escritura_gana_y_el_segmento_usa_el_pixel_nuevo() {
    // `(1,1)` con A, `(3,1)` con A y otra vez `(1,1)` con B: el segmento de vuelta pinta los tres
    // con B, extremo ya pintado incluido.
    let vuelta = trazo(
        8,
        8,
        &[
            ((1, 1), pixel_a()),
            ((3, 1), pixel_a()),
            ((1, 1), pixel_b()),
        ],
    );
    assert_eq!(
        vuelta.pixels(),
        ediciones(&[(1, 1), (2, 1), (3, 1)], pixel_b())
    );
    // `(1,1)` con A y `(3,1)` con B: el extremo `(1,1)`, que estaba con A, pasa a B.
    let ida = trazo(8, 8, &[((1, 1), pixel_a()), ((3, 1), pixel_b())]);
    assert_eq!(
        ida.pixels(),
        ediciones(&[(1, 1), (2, 1), (3, 1)], pixel_b())
    );
}

#[test]
fn una_posicion_igual_a_la_anterior_no_hace_nada_aunque_el_pixel_cambie() {
    let mut stroke = Stroke::new(8, 8);
    stroke.move_to((2, 2), pixel_a());
    stroke.move_to((2, 2), pixel_b());
    assert_eq!(stroke.pixels(), [edicion(2, 2, pixel_a())]);
}

#[test]
fn fila_5_todo_fuera_deja_el_trazo_vacio_pero_recuerda_la_ultima_posicion() {
    let stroke = trazo_con(8, 8, &[(-5, 2), (-1, 6), (-3, 0)], pixel_a());
    assert!(stroke.is_empty());
    assert!(stroke.pixels().is_empty());
    assert_eq!(stroke.last(), Some((-3, 0)));

    let mut doc = doc_rgba(8, 8);
    let id = doc.main_image();
    let antes = instantanea(&doc);
    assert_eq!(stroke.commit(&mut doc, id), Ok(false));
    assert_eq!(instantanea(&doc), antes);
}

#[test]
fn fila_6_el_tramo_de_ida_y_vuelta_por_fuera_no_se_cierra_por_dentro() {
    // (6,2)-(20,2) dentro da (6,2),(7,2); (20,2)-(6,4) tiene 15 píxeles (`i` de 0 a 14) y solo
    // los dos últimos caen dentro: (7,4) y (6,4). No hay línea entre (7,2) y (7,4).
    let stroke = trazo_con(8, 8, &[(6, 2), (20, 2), (6, 4)], pixel_a());
    assert_eq!(
        coordenadas(&stroke),
        [(6, 2), (7, 2), (7, 4), (6, 4)],
        "calculado a mano"
    );
    // El mismo resultado con el oráculo de `Line`, que no depende de lo anterior.
    let mut oraculo: Vec<Punto> = vec![(6, 2)];
    for (a, b) in [((6, 2), (20, 2)), ((20, 2), (6, 4))] {
        for p in Line::new(a, b).clip_to(8, 8) {
            if !oraculo.contains(&p) {
                oraculo.push(p);
            }
        }
    }
    assert_eq!(coordenadas(&stroke), oraculo);
    assert_eq!(stroke.last(), Some((6, 4)));
}

#[test]
fn fila_7_los_extremos_de_i32_no_entran_en_panico() {
    let diagonal = trazo_con(8, 8, &[(MIN, MIN), (MAX, MAX)], pixel_a());
    let esperado: Vec<Punto> = (0..8).map(|k| (k, k)).collect();
    assert_eq!(coordenadas(&diagonal), esperado);
    assert_eq!(diagonal.last(), Some((MAX, MAX)));

    let uno = trazo_con(1, 1, &[(0, 0), (5, 5)], pixel_a());
    assert_eq!(uno.pixels(), [edicion(0, 0, pixel_a())]);
    assert_eq!(uno.last(), Some((5, 5)));
}

#[test]
fn todas_las_esquinas_de_i32_entre_si_no_entran_en_panico() {
    let valores = [MIN, MIN + 1, -1, 0, 1, 7, 8, MAX - 1, MAX];
    for ax in valores {
        for ay in valores {
            for bx in valores {
                for by in valores {
                    let stroke = trazo_con(8, 8, &[(ax, ay), (bx, by)], pixel_a());
                    for (x, y) in coordenadas(&stroke) {
                        assert!(dentro((x, y), 8, 8), "({x}, {y}) fuera de 8x8");
                    }
                    assert_eq!(stroke.last(), Some((bx, by)));
                }
            }
        }
    }
}

// Filas 8 a 10: `commit` contra el documento.

#[test]
fn fila_8_un_trazo_que_cambia_es_un_paso_de_historia() {
    let mut doc = doc_rgba(8, 8);
    let id = doc.main_image();
    let original = imagen_de(&doc);
    let mut editada = original.clone();
    for x in 1..=3 {
        editada.set_pixel(x, 1, pixel_a()).unwrap();
    }

    let stroke = trazo_con(8, 8, &[(1, 1), (3, 1)], pixel_a());
    assert_eq!(stroke.pixels().len(), 3);
    assert_eq!(stroke.commit(&mut doc, id), Ok(true));
    assert!(doc.can_undo());
    assert!(!doc.can_redo());
    assert_eq!(imagen_de(&doc), editada);

    assert_eq!(doc.undo(), Ok(()));
    assert_eq!(imagen_de(&doc), original, "un undo vuelve al original");
    assert!(!doc.can_undo());
    assert_eq!(doc.redo(), Ok(()));
    assert_eq!(imagen_de(&doc), editada);
}

#[test]
fn fila_8_con_dos_trazos_un_undo_deshace_solo_el_ultimo() {
    let mut doc = doc_rgba(8, 8);
    let id = doc.main_image();
    let original = imagen_de(&doc);

    let primero = trazo_con(8, 8, &[(1, 1), (3, 1)], pixel_a());
    assert_eq!(primero.commit(&mut doc, id), Ok(true));
    let tras_primero = imagen_de(&doc);
    let segundo = trazo_con(8, 8, &[(1, 3), (3, 3)], pixel_b());
    assert_eq!(segundo.commit(&mut doc, id), Ok(true));
    assert_ne!(imagen_de(&doc), tras_primero);

    assert_eq!(doc.undo(), Ok(()));
    assert_eq!(imagen_de(&doc), tras_primero);
    assert!(doc.can_undo());
    assert_eq!(doc.undo(), Ok(()));
    assert_eq!(imagen_de(&doc), original);
    assert!(!doc.can_undo());
}

#[test]
fn fila_9_los_trazos_que_no_cambian_nada_no_tocan_la_historia() {
    let mut doc = doc_rgba(8, 8);
    let id = doc.main_image();
    let a = trazo_con(8, 8, &[(1, 1), (3, 1)], pixel_a());
    assert_eq!(a.commit(&mut doc, id), Ok(true));
    let b = trazo_con(8, 8, &[(1, 3), (3, 3)], pixel_b());
    assert_eq!(b.commit(&mut doc, id), Ok(true));
    // Un `undo` previo: queda algo que rehacer, y no debe perderse.
    assert_eq!(doc.undo(), Ok(()));
    let antes = instantanea(&doc);
    assert!(antes.1 && antes.2, "hay algo que deshacer y que rehacer");

    // Vacío: no llama a `apply`, que daría `EmptyCommand`.
    assert_eq!(Stroke::new(8, 8).commit(&mut doc, id), Ok(false));
    assert_eq!(instantanea(&doc), antes);
    // Todo fuera del lienzo.
    let todo_fuera = trazo_con(8, 8, &[(-4, 2), (-1, 6)], pixel_a());
    assert!(todo_fuera.is_empty());
    assert_eq!(todo_fuera.commit(&mut doc, id), Ok(false));
    assert_eq!(instantanea(&doc), antes);
    // Con el color que ya hay (el primer trazo dejó A en la fila 1).
    let igual = trazo_con(8, 8, &[(1, 1), (3, 1)], pixel_a());
    assert_eq!(igual.pixels().len(), 3);
    assert_eq!(igual.commit(&mut doc, id), Ok(false));
    assert_eq!(instantanea(&doc), antes);
    assert!(doc.can_redo(), "el redo pendiente sigue ahí");
}

#[test]
fn fila_10_un_pixel_de_otro_modo_da_error_y_no_toca_el_documento() {
    let mut doc = doc_rgba(8, 8);
    let id = doc.main_image();
    let antes = instantanea(&doc);
    let stroke = trazo_con(8, 8, &[(1, 1), (3, 1)], Pixel::Indexed(0));
    assert_eq!(
        stroke.commit(&mut doc, id),
        Err(DocumentError::Pixel {
            pixel: 0,
            source: ImageError::ModeMismatch {
                expected: Mode::Rgba,
                found: Mode::Indexed
            }
        })
    );
    assert_eq!(instantanea(&doc), antes);
}

#[test]
fn commit_aplica_un_solo_set_pixels_con_los_pixeles_del_trazo() {
    // Lo mismo que `Document::apply(SetPixels { .. })` con la lista del trazo.
    let stroke = trazo_con(8, 8, &[(0, 2), (4, 2), (2, 0)], pixel_a());
    let pixels = stroke.pixels().to_vec();
    let mut con_trazo = doc_rgba(8, 8);
    let id = con_trazo.main_image();
    let mut a_mano = doc_rgba(8, 8);
    assert_eq!(
        a_mano.apply(Command::SetPixels { image: id, pixels }),
        Ok(true)
    );
    assert_eq!(stroke.commit(&mut con_trazo, id), Ok(true));
    assert_eq!(instantanea(&con_trazo), instantanea(&a_mano));
}

// Estrategias.

/// Coordenada `i32`: sesgo hacia dentro de un lienzo de hasta 16 y sus bordes, con los extremos.
fn coordenada() -> impl Strategy<Value = i32> {
    prop_oneof![
        4 => -3i32..=18,
        3 => 0i32..=15,
        1 => Just(0i32),
        1 => Just(15i32),
        1 => any::<i32>(),
        1 => Just(MIN),
        1 => Just(MAX),
    ]
}

fn posiciones(max: usize) -> impl Strategy<Value = Vec<Punto>> {
    vec((coordenada(), coordenada()), 1..=max)
}

/// Si `dentro`, lleva todas las posiciones al interior del lienzo; si no, las deja como están.
fn ajustar(pos: &[Punto], width: u32, height: u32, dentro: bool) -> Vec<Punto> {
    pos.iter()
        .map(|&(x, y)| {
            if dentro {
                (x.rem_euclid(width as i32), y.rem_euclid(height as i32))
            } else {
                (x, y)
            }
        })
        .collect()
}

// Oráculos.

/// Conjunto esperado: el primer punto y, para cada par consecutivo distinto, los puntos de
/// `Line` recortados al lienzo.
fn oraculo_conjunto(pos: &[Punto], width: u32, height: u32) -> BTreeSet<Punto> {
    let mut conjunto = BTreeSet::new();
    let mut ultimo: Option<Punto> = None;
    for &p in pos {
        match ultimo {
            None => {
                if dentro(p, width, height) {
                    conjunto.insert(p);
                }
            }
            Some(anterior) if anterior == p => {}
            Some(anterior) => conjunto.extend(Line::new(anterior, p).clip_to(width, height)),
        }
        ultimo = Some(p);
    }
    conjunto
}

/// Lista esperada con la regla «la última escritura gana» y el orden de primera visita.
/// `pixeles[i]` es el píxel de la llamada `i`.
fn oraculo_valores(pos: &[Punto], pixeles: &[Pixel], width: u32, height: u32) -> Vec<PixelEdit> {
    let mut orden: Vec<Punto> = Vec::new();
    let mut valores: HashMap<Punto, Pixel> = HashMap::new();
    let mut ultimo: Option<Punto> = None;
    for (&p, &pixel) in pos.iter().zip(pixeles) {
        if ultimo == Some(p) {
            continue;
        }
        let puntos: Vec<Punto> = match ultimo {
            None if dentro(p, width, height) => vec![p],
            None => vec![],
            Some(anterior) => Line::new(anterior, p).clip_to(width, height).collect(),
        };
        for q in puntos {
            if valores.insert(q, pixel).is_none() {
                orden.push(q);
            }
        }
        ultimo = Some(p);
    }
    orden
        .into_iter()
        .map(|q| edicion(q.0, q.1, valores[&q]))
        .collect()
}

/// ¿Es el conjunto 8-conexo? (vacío cuenta como conexo).
fn es_8_conexo(conjunto: &BTreeSet<Punto>) -> bool {
    let Some(&inicio) = conjunto.iter().next() else {
        return true;
    };
    let mut visto: BTreeSet<Punto> = BTreeSet::from([inicio]);
    let mut cola: VecDeque<Punto> = VecDeque::from([inicio]);
    while let Some((x, y)) = cola.pop_front() {
        for dx in -1..=1 {
            for dy in -1..=1 {
                let vecino = (x + dx, y + dy);
                if conjunto.contains(&vecino) && visto.insert(vecino) {
                    cola.push_back(vecino);
                }
            }
        }
    }
    visto.len() == conjunto.len()
}

// Propiedad 1: oráculo geométrico y conexión.

proptest! {
    #[test]
    fn los_pixeles_son_la_union_de_las_lineas_recortadas(
        width in 1u32..=16,
        height in 1u32..=16,
        pos in posiciones(8),
        todas_dentro in any::<bool>(),
    ) {
        let pos = ajustar(&pos, width, height, todas_dentro);
        let stroke = trazo_con(width, height, &pos, pixel_a());
        let obtenidos = coordenadas(&stroke);

        // Todos dentro y sin repetidos.
        for &p in &obtenidos {
            prop_assert!(dentro(p, width, height), "{:?} fuera de {}x{}", p, width, height);
        }
        let conjunto: BTreeSet<Punto> = obtenidos.iter().copied().collect();
        prop_assert_eq!(conjunto.len(), obtenidos.len(), "coordenadas repetidas");

        // Exactamente la unión de los segmentos recortados, y nada más.
        prop_assert_eq!(&conjunto, &oraculo_conjunto(&pos, width, height));
        prop_assert_eq!(stroke.is_empty(), conjunto.is_empty());
        prop_assert_eq!(stroke.last(), pos.last().copied());

        // Todas las posiciones dentro: sin huecos.
        if todas_dentro {
            prop_assert!(es_8_conexo(&conjunto), "hueco en {:?}", conjunto);
        }
    }
}

// Propiedad 2: valores y repeticiones.

proptest! {
    #[test]
    fn gana_la_ultima_escritura_y_el_orden_es_el_de_primera_visita(
        width in 1u32..=16,
        height in 1u32..=16,
        pos in posiciones(8),
        todas_dentro in any::<bool>(),
    ) {
        let pos = ajustar(&pos, width, height, todas_dentro);
        let pixeles: Vec<Pixel> = (0..pos.len()).map(pixel_k).collect();
        let llamadas: Vec<(Punto, Pixel)> = pos.iter().copied().zip(pixeles.iter().copied()).collect();
        let stroke = trazo(width, height, &llamadas);
        prop_assert_eq!(
            stroke.pixels().to_vec(),
            oraculo_valores(&pos, &pixeles, width, height)
        );
    }

    #[test]
    fn repetir_una_posicion_seguida_no_cambia_el_resultado(
        width in 1u32..=16,
        height in 1u32..=16,
        pos in posiciones(8),
        repeticiones in vec(0usize..=3, 8),
        todas_dentro in any::<bool>(),
    ) {
        let pos = ajustar(&pos, width, height, todas_dentro);
        let base: Vec<(Punto, Pixel)> = pos
            .iter()
            .enumerate()
            .map(|(i, &p)| (p, pixel_k(i)))
            .collect();
        let sin_repetir = trazo(width, height, &base);

        // Cada posición se repite 0 a 3 veces seguidas, con otro píxel cada vez: se ignoran.
        let mut repetida: Vec<(Punto, Pixel)> = Vec::new();
        for (i, &(p, pixel)) in base.iter().enumerate() {
            repetida.push((p, pixel));
            for j in 0..repeticiones[i] {
                repetida.push((p, pixel_k(100 + 3 * i + j)));
            }
        }
        let con_repeticiones = trazo(width, height, &repetida);
        prop_assert_eq!(sin_repetir.pixels(), con_repeticiones.pixels());
        prop_assert_eq!(sin_repetir.last(), con_repeticiones.last());
    }
}

// Imágenes de los cuatro modos para las propiedades 3 y 4.

const MODOS: [Mode; 4] = [Mode::Rgba, Mode::Indexed, Mode::Gray, Mode::GrayAlpha];

/// Hasta cuatro valores distintos por modo (`v` de 0 a 3), que caben en la paleta de 4 colores.
fn valor(modo: Mode, v: u8) -> Pixel {
    match modo {
        Mode::Rgba => Pixel::Rgba(Rgba::new(v * 60, 10, v * 3, 255)),
        Mode::Indexed => Pixel::Indexed(v),
        Mode::Gray => Pixel::Gray(v * 70),
        Mode::GrayAlpha => Pixel::GrayAlpha {
            gray: 5,
            alpha: 255 - v * 60,
        },
    }
}

fn imagen_vacia(modo: Mode, width: u32, height: u32) -> Image {
    match modo {
        Mode::Indexed => {
            let paleta = Palette::from_colors((0..4).map(|i| Rgba::opaque(i * 50, 0, 0))).unwrap();
            Image::new_indexed(width, height, paleta).unwrap()
        }
        otro => Image::new(width, height, otro).unwrap(),
    }
}

/// Imagen con valores 0 a 2 repartidos según `ruido`.
fn imagen_con_ruido(modo: Mode, width: u32, height: u32, ruido: &[u8]) -> Image {
    let mut image = imagen_vacia(modo, width, height);
    for y in 0..height {
        for x in 0..width {
            let v = ruido[(y * width + x) as usize % ruido.len()] % 3;
            image.set_pixel(x, y, valor(modo, v)).unwrap();
        }
    }
    image
}

/// Imagen toda del valor `v`.
fn imagen_uniforme(modo: Mode, width: u32, height: u32, v: u8) -> Image {
    imagen_con_ruido(modo, width, height, &[v])
}

/// `SetPixels` que pone toda la imagen al valor `v`.
fn rellenar(doc: &mut Document, modo: Mode, v: u8) {
    let id = doc.main_image();
    let (width, height) = {
        let image = doc.image(id).unwrap();
        (image.width(), image.height())
    };
    let pixels: Vec<PixelEdit> = (0..height)
        .flat_map(|y| {
            (0..width).map(move |x| PixelEdit {
                x,
                y,
                pixel: valor(modo, v),
            })
        })
        .collect();
    assert_eq!(
        doc.apply(Command::SetPixels { image: id, pixels }),
        Ok(true)
    );
}

/// Un trazo con una lista de posiciones y un valor, aplicado con `commit` y comprobado contra
/// la imagen antes y después. Devuelve el resultado de `commit`.
fn comprobar_un_paso(
    doc: &mut Document,
    modo: Mode,
    pos: &[Punto],
    v: u8,
) -> Result<bool, TestCaseError> {
    let id = doc.main_image();
    let image = imagen_de(doc);
    let pixel = valor(modo, v);
    let stroke = trazo_con(image.width(), image.height(), pos, pixel);
    let cambia = stroke
        .pixels()
        .iter()
        .any(|e| image.get_pixel(e.x, e.y).unwrap() != pixel);
    let mut editada = image.clone();
    for e in stroke.pixels() {
        editada.set_pixel(e.x, e.y, e.pixel).unwrap();
    }

    let antes = instantanea(doc);
    let resultado = stroke.commit(doc, id);
    prop_assert_eq!(resultado, Ok(cambia));
    if cambia {
        prop_assert!(doc.can_undo());
        prop_assert!(!doc.can_redo());
        prop_assert_eq!(imagen_de(doc), editada.clone());
        // Un solo paso: un undo vuelve exactamente a la imagen previa y un redo a la editada.
        prop_assert_eq!(doc.undo(), Ok(()));
        prop_assert_eq!(imagen_de(doc), image);
        prop_assert!(doc.can_redo());
        prop_assert_eq!(doc.redo(), Ok(()));
        prop_assert_eq!(imagen_de(doc), editada);
    } else {
        prop_assert_eq!(instantanea(doc), antes);
    }
    Ok(cambia)
}

// Propiedad 3: un trazo, un paso.

proptest! {
    #[test]
    fn un_trazo_que_cambia_es_exactamente_un_paso(
        modo in 0usize..4,
        width in 1u32..=16,
        height in 1u32..=16,
        ruido in vec(0u8..3, 256),
        pos in posiciones(8),
        v in 0u8..3,
        todas_dentro in any::<bool>(),
    ) {
        let modo = MODOS[modo];
        let mut doc = Document::new(imagen_con_ruido(modo, width, height, &ruido));
        let pos = ajustar(&pos, width, height, todas_dentro);
        comprobar_un_paso(&mut doc, modo, &pos, v)?;
    }

    #[test]
    fn k_trazos_se_deshacen_y_rehacen_de_uno_en_uno(
        modo in 0usize..4,
        width in 1u32..=16,
        height in 1u32..=16,
        ruido in vec(0u8..3, 256),
        trazos in vec((posiciones(6), 0u8..3, any::<bool>()), 1..=4),
    ) {
        let modo = MODOS[modo];
        let mut doc = Document::new(imagen_con_ruido(modo, width, height, &ruido));
        let original = imagen_de(&doc);

        let mut pasos = 0usize;
        for (pos, v, todas_dentro) in &trazos {
            let pos = ajustar(pos, width, height, *todas_dentro);
            if comprobar_un_paso(&mut doc, modo, &pos, *v)? {
                pasos += 1;
            }
        }
        let final_ = imagen_de(&doc);

        for _ in 0..pasos {
            prop_assert_eq!(doc.undo(), Ok(()));
        }
        prop_assert_eq!(imagen_de(&doc), original);
        prop_assert!(!doc.can_undo());
        for _ in 0..pasos {
            prop_assert_eq!(doc.redo(), Ok(()));
        }
        prop_assert_eq!(imagen_de(&doc), final_);
        prop_assert!(!doc.can_redo());

        // Un trazo nuevo tras un undo vacía lo rehacible. El valor 3 no está en la imagen, así
        // que el trazo siempre cambia algo.
        if pasos >= 1 {
            prop_assert_eq!(doc.undo(), Ok(()));
            prop_assert!(doc.can_redo());
            let nuevo = trazo_con(width, height, &[(0, 0)], valor(modo, 3));
            let id = doc.main_image();
            prop_assert_eq!(nuevo.commit(&mut doc, id), Ok(true));
            prop_assert!(!doc.can_redo());
        }
    }
}

// Propiedad 4: no-ops.

proptest! {
    #[test]
    fn los_trazos_que_no_cambian_nada_no_tocan_el_documento(
        modo in 0usize..4,
        width in 1u32..=16,
        height in 1u32..=16,
        previa in 0u8..3,
        tipo in 0u8..3,
        pos in posiciones(8),
        fuera_por_la_derecha in any::<bool>(),
    ) {
        let modo = MODOS[modo];
        let mut doc = Document::new(imagen_uniforme(modo, width, height, 0));
        let id = doc.main_image();
        // Historia previa: ninguna, un paso (la imagen entera pasa a valer 1), o ese paso
        // deshecho (con algo que rehacer).
        let mut actual = 0;
        if previa >= 1 {
            rellenar(&mut doc, modo, 1);
            actual = 1;
        }
        if previa == 2 {
            prop_assert_eq!(doc.undo(), Ok(()));
            actual = 0;
        }
        let antes = instantanea(&doc);

        let stroke = match tipo {
            // Vacío.
            0 => Stroke::new(width, height),
            // Todo fuera: x < 0 o x >= width, con cualquier y.
            1 => {
                let fuera: Vec<Punto> = pos
                    .iter()
                    .map(|&(x, y)| {
                        let x = if x < 0 { x } else { -1 - x.min(MAX - 1) };
                        let x = if fuera_por_la_derecha {
                            (width as i32 - 1).saturating_sub(x)
                        } else {
                            x
                        };
                        (x, y)
                    })
                    .collect();
                trazo_con(width, height, &fuera, valor(modo, 2))
            }
            // Con el valor que ya tiene toda la imagen.
            _ => trazo_con(width, height, &pos, valor(modo, actual)),
        };
        if tipo == 1 {
            prop_assert!(stroke.is_empty());
        }
        prop_assert_eq!(stroke.commit(&mut doc, id), Ok(false));
        prop_assert_eq!(instantanea(&doc), antes);
    }
}
