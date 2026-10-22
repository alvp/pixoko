//! Propiedades 1 a 3 de la puerta del hito 3 (specs/hito-3-puerta.md, `proptest`): 64 casos las dos
//! primeras y 32 la tercera (toca el disco); se baja con `PROPTEST_CASES` solo al desarrollar. La
//! propiedad 4 está en `px-golden`.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use proptest::collection::vec;
use proptest::prelude::*;
use px_core::{BlendMode, Mode};
use px_formats::read_png;
use px_raster::compose;
use px_ui::canvas::Canvas;

use super::guiones::{sesion_indexada_de, sesion_rgba_de};
use super::modelo::{Celda4, Orden, Pila};
use super::mundo::Mundo3;
use super::{
    Trabajo3, comprobar_golden_aseprite, fuera_de_golden_aseprite, igual_salvo_rgb_con_alfa_0,
};
use crate::merge_flow::MergeKind;
use crate::puerta::comun::{golden_intacto, sesion_vacia, sin_temporales};

/// Una orden abstracta: se concreta contra el estado del modelo (los índices, las coordenadas y los
/// colores dependen de él).
#[derive(Clone, Debug)]
enum A {
    Nueva,
    Duplicar,
    Quitar,
    Subir,
    Bajar,
    Activar(u8),
    Visible(u8, bool),
    Bloqueo(u8, bool),
    Opacidad(u8, u8),
    Mezcla(u8, u8),
    Lapiz(u8, u8, u8, u8, u8),
    Linea(u8, u8, u8, u8, u8),
    Relleno(u8, u8, u8),
    Borrador(u8, u8, u8, u8),
    Combinar(u8),
    ConvertirIndexado,
    ConvertirRgba,
    Guardar,
}

fn orden_a() -> impl Strategy<Value = A> {
    let b = any::<u8>;
    prop_oneof![
        3 => Just(A::Nueva),
        1 => Just(A::Duplicar),
        1 => Just(A::Quitar),
        1 => Just(A::Subir),
        1 => Just(A::Bajar),
        3 => b().prop_map(A::Activar),
        2 => (b(), any::<bool>()).prop_map(|(i, v)| A::Visible(i, v)),
        1 => (b(), any::<bool>()).prop_map(|(i, v)| A::Bloqueo(i, v)),
        2 => (b(), b()).prop_map(|(i, o)| A::Opacidad(i, o)),
        2 => (b(), b()).prop_map(|(i, m)| A::Mezcla(i, m)),
        4 => (b(), b(), b(), b(), b()).prop_map(|(x, y, d, l, c)| A::Lapiz(x, y, d, l, c)),
        3 => (b(), b(), b(), b(), b()).prop_map(|(x, y, d, l, c)| A::Linea(x, y, d, l, c)),
        3 => (b(), b(), b()).prop_map(|(x, y, c)| A::Relleno(x, y, c)),
        2 => (b(), b(), b(), b()).prop_map(|(x, y, d, l)| A::Borrador(x, y, d, l)),
        2 => b().prop_map(A::Combinar),
        1 => Just(A::ConvertirIndexado),
        1 => Just(A::ConvertirRgba),
        1 => Just(A::Guardar),
    ]
}

/// Los seis colores RGBA de las propiedades (con translúcidos, nunca alfa 0) o un índice de 1 a 15.
fn color(m: &Pila, n: u8) -> Celda4 {
    const RGBA: [Celda4; 6] = [
        [255, 0, 0, 255],
        [0, 255, 0, 255],
        [0, 0, 255, 255],
        [255, 255, 0, 128],
        [10, 20, 30, 255],
        [200, 100, 0, 64],
    ];
    match m.modo {
        Mode::Indexed => {
            // Una entrada de la paleta que no sea la transparente.
            let paleta = m.paleta.as_ref().expect("paleta");
            let t = paleta.transparent_index();
            let validas: Vec<u8> = (0..paleta.len() as u8).filter(|&j| Some(j) != t).collect();
            [
                validas
                    .get(usize::from(n) % validas.len().max(1))
                    .copied()
                    .unwrap_or(0),
                0,
                0,
                0,
            ]
        }
        _ => RGBA[usize::from(n) % RGBA.len()],
    }
}

/// Un segmento horizontal, vertical o a 45° que cabe en el lienzo desde `(x, y)`.
fn segmento_dentro(m: &Pila, x: u8, y: u8, dir: u8, largo: u8) -> ((i32, i32), (i32, i32)) {
    let (ancho, alto) = (m.ancho as i32, m.alto as i32);
    let inicio = (i32::from(x) % ancho, i32::from(y) % alto);
    const DIRS: [(i32, i32); 8] = [
        (1, 0),
        (-1, 0),
        (0, 1),
        (0, -1),
        (1, 1),
        (1, -1),
        (-1, 1),
        (-1, -1),
    ];
    let (dx, dy) = DIRS[usize::from(dir) % 8];
    let hasta = |pos: i32, d: i32, tope: i32| match d {
        1 => tope - 1 - pos,
        -1 => pos,
        _ => i32::MAX,
    };
    let maximo = hasta(inicio.0, dx, ancho).min(hasta(inicio.1, dy, alto));
    let pasos = i32::from(largo) % (maximo + 1);
    (inicio, (inicio.0 + dx * pasos, inicio.1 + dy * pasos))
}

fn concreta(a: &A, m: &Pila) -> Orden {
    let capa = |i: u8| usize::from(i) % m.capas.len();
    match a {
        A::Nueva => Orden::Nueva,
        A::Duplicar => Orden::Duplicar,
        A::Quitar => Orden::Quitar,
        A::Subir => Orden::Mover(1),
        A::Bajar => Orden::Mover(-1),
        A::Activar(i) => Orden::Activar(capa(*i)),
        A::Visible(i, v) => Orden::Visible(capa(*i), *v),
        A::Bloqueo(i, v) => Orden::Bloqueo(capa(*i), *v),
        A::Opacidad(i, o) => Orden::Opacidad(capa(*i), *o),
        A::Mezcla(i, k) => Orden::Mezcla(capa(*i), BlendMode::ALL[usize::from(*k) % 8]),
        A::Lapiz(x, y, d, l, c) => {
            let (a, b) = segmento_dentro(m, *x, *y, *d, *l);
            Orden::Lapiz(vec![a, b], color(m, *c))
        }
        A::Linea(x, y, d, l, c) => {
            let (a, b) = segmento_dentro(m, *x, *y, *d, *l);
            Orden::Linea(a, b, color(m, *c))
        }
        A::Relleno(x, y, c) => Orden::Relleno(
            (
                i32::from(*x) % m.ancho as i32,
                i32::from(*y) % m.alto as i32,
            ),
            color(m, *c),
        ),
        A::Borrador(x, y, d, l) => {
            let (a, b) = segmento_dentro(m, *x, *y, *d, *l);
            Orden::Borrador(vec![a, b])
        }
        A::Combinar(k) => Orden::Combinar(match k % 3 {
            0 => MergeKind::Down,
            1 => MergeKind::Visible,
            _ => MergeKind::Flatten,
        }),
        // Solo tienen sentido en el modo de origen; en el otro son una elección de capa sin paso.
        A::ConvertirIndexado if m.modo == Mode::Rgba => Orden::Convertir(Mode::Indexed),
        A::ConvertirRgba if m.modo == Mode::Indexed => Orden::Convertir(Mode::Rgba),
        A::ConvertirIndexado | A::ConvertirRgba | A::Guardar => Orden::Activar(m.activa),
    }
}

/// Un indexado sin índice transparente (el de un RGBA de una capa sin alfa 0 convertido).
fn sin_transparente(w: &Mundo3) -> bool {
    w.m.modo == Mode::Indexed
        && w.m
            .paleta
            .as_ref()
            .is_none_or(|p| p.transparent_index().is_none())
}

fn mundo(indexado: bool, ancho: u32, alto: u32) -> Mundo3 {
    if indexado {
        Mundo3::de_sesion(sesion_indexada_de(ancho, alto), "propiedad")
    } else {
        Mundo3::de_sesion(sesion_rgba_de(ancho, alto), "propiedad")
    }
}

fn ejecutar(w: &mut Mundo3, ordenes: &[A]) {
    for a in ordenes {
        let orden = concreta(a, &w.m);
        w.aplicar(&orden);
        if matches!(a, A::Guardar) {
            w.s.mark_unmodified();
            w.marcar_guardado();
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// P1. Guion aleatorio contra el modelo: tras cada paso la pila de la sesión es la del modelo
    /// y la composición la de `from_layers`; al final, guardar en `.aseprite` y reabrir da el
    /// mismo contenido (con la exención exacta del RGB bajo alfa 0).
    #[test]
    fn propiedad_1_guion_aleatorio_contra_el_modelo(
        indexado in any::<bool>(),
        ancho in 1u32..=10,
        alto in 1u32..=10,
        ordenes in vec(orden_a(), 1..=40),
    ) {
        let mut w = mundo(indexado, ancho, alto);
        ejecutar(&mut w, &ordenes);
        let t = Trabajo3::nuevo();
        let ruta = t.ruta().join("p1.aseprite");
        fuera_de_golden_aseprite(&ruta);
        if sin_transparente(&w) {
            // Indexado sin índice transparente: el escritor lo rechaza (3.8, decisión 2) y no
            // deja ningún fichero.
            let error = w.s.save_to(&ruta).expect_err("indexado sin índice transparente");
            prop_assert!(error.to_string().contains("no tiene entrada transparente"), "{error}");
            prop_assert!(t.nombres().is_empty());
            return Ok(());
        }
        w.s.save_to(&ruta).expect("guardar el .aseprite");
        let mut otra = sesion_vacia();
        otra.open(&ruta, Canvas::MAX_SIDE, true).expect("reabrir");
        let capas = otra.document.layers().len();
        let leida = Pila::de_documento(&otra.document, capas - 1);
        prop_assert!(
            w.m.canonica().igual_contenido(&leida.canonica()).is_ok(),
            "{:?}",
            w.m.canonica().igual_contenido(&leida.canonica())
        );
        let antes = compose(&w.s.document).expect("compose");
        let despues = compose(&otra.document).expect("compose");
        prop_assert_eq!(igual_salvo_rgb_con_alfa_0(&antes, &despues), Ok(()));
    }

    /// P2. Historia con capas: deshacer k veces da la instantánea anotada tras el paso `n − k`
    /// para todo k (con una capa activa que existe siempre) y rehacer las vuelve a dar; «modificado»
    /// es falso si y solo si el `state_id` es el guardado, y nunca con otro contenido.
    #[test]
    fn propiedad_2_la_historia_con_capas_es_exacta(
        indexado in any::<bool>(),
        ancho in 1u32..=10,
        alto in 1u32..=10,
        ordenes in vec(orden_a(), 1..=40),
    ) {
        let mut w = mundo(indexado, ancho, alto);
        ejecutar(&mut w, &ordenes);
        let (pasos, final_) = (w.pasos(), w.leer());
        prop_assert_eq!(w.deshacer_todo(), pasos);
        prop_assert_eq!(w.s.document.state_id(), 0);
        prop_assert_eq!(w.rehacer_todo(), pasos);
        prop_assert_eq!(final_.igual_contenido(&w.leer()), Ok(()));
        // Un paso a paso desde el final, cada vez comprobado dentro de `deshacer` y `rehacer`.
        for _ in 0..pasos {
            w.deshacer();
        }
        for _ in 0..pasos {
            w.rehacer();
        }
        prop_assert_eq!(final_.igual_contenido(&w.leer()), Ok(()));
    }
}

/// Lo que debe haber en un fichero escrito.
enum Esperado {
    Aseprite(Pila),
    Png(px_core::Image),
}

#[derive(Clone, Debug)]
enum B {
    Editar(u8),
    GuardarComo(u8, bool),
    Exportar(u8),
    Directo,
    Copia,
}

fn orden_b() -> impl Strategy<Value = B> {
    prop_oneof![
        3 => any::<u8>().prop_map(B::Editar),
        3 => (0u8..3, any::<bool>()).prop_map(|(n, a)| B::GuardarComo(n, a)),
        2 => (0u8..3).prop_map(B::Exportar),
        1 => Just(B::Directo),
        1 => Just(B::Copia),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    /// P3. Solo se escribe lo que se pide: con secuencias aleatorias de guardar, guardar como,
    /// exportar y guardar una copia, el directorio de trabajo contiene exactamente los ficheros de
    /// los `Saved` devueltos (sin `.pixoko-tmp`), cada uno con el contenido esperado en su último
    /// guardado, y `golden/aseprite/` y `golden/png/` quedan iguales.
    #[test]
    fn propiedad_3_solo_se_escribe_lo_que_se_pide(
        indexado in any::<bool>(),
        ordenes in vec(orden_b(), 1..=14),
    ) {
        let t = Trabajo3::nuevo();
        let mut w = mundo(indexado, 6, 5);
        let mut esperado: BTreeMap<PathBuf, Esperado> = BTreeMap::new();
        let guardar = |w: &mut Mundo3, esperado: &mut BTreeMap<PathBuf, Esperado>, ruta: PathBuf| {
            let aseprite = ruta.extension().is_some_and(|e| e == "aseprite");
            let contenido = if aseprite {
                Esperado::Aseprite(w.m.canonica())
            } else {
                Esperado::Png(compose(&w.s.document).expect("compose"))
            };
            esperado.insert(ruta, contenido);
        };
        for orden in &ordenes {
            match orden {
                B::Editar(n) => {
                    let o = if n % 3 == 0 {
                        Orden::Nueva
                    } else {
                        let c = color(&w.m, *n);
                        Orden::Relleno((i32::from(*n) % 6, i32::from(*n) % 5), c)
                    };
                    w.aplicar(&o);
                }
                B::GuardarComo(n, aseprite) => {
                    let ext = if *aseprite { "aseprite" } else { "png" };
                    let elegida = t.ruta().join(format!("f{n}.{ext}"));
                    fuera_de_golden_aseprite(&elegida);
                    if *aseprite && sin_transparente(&w) {
                        let error = w.s.save_to(&elegida).expect_err("sin índice transparente");
                        prop_assert!(error.to_string().contains("no tiene entrada transparente"), "{error}");
                        continue;
                    }
                    let saved = w.s.save_to(&elegida).expect("guardar");
                    prop_assert_eq!(&saved.path, &elegida);
                    w.marcar_guardado();
                    guardar(&mut w, &mut esperado, saved.path);
                }
                B::Exportar(n) => {
                    let elegida = t.ruta().join(format!("e{n}.png"));
                    fuera_de_golden_aseprite(&elegida);
                    let saved = w.s.export_flat(&elegida).expect("exportar");
                    guardar(&mut w, &mut esperado, saved.path);
                }
                B::Directo => {
                    if w.s.path().is_some() && w.s.save_route() == crate::dialogs::SaveRoute::Direct {
                        let saved = w.s.save(t.ruta()).expect("guardar");
                        w.marcar_guardado();
                        guardar(&mut w, &mut esperado, saved.path);
                    }
                }
                B::Copia => {
                    if w.s.path().is_some() {
                        let saved = w.s.save_as(t.ruta()).expect("guardar copia");
                        w.marcar_guardado();
                        guardar(&mut w, &mut esperado, saved.path);
                    }
                }
            }
        }
        let nombres = t.nombres();
        sin_temporales(&nombres);
        let mut pedidos: Vec<String> = esperado
            .keys()
            .map(|r| r.file_name().expect("nombre").to_string_lossy().into_owned())
            .collect();
        pedidos.sort();
        prop_assert_eq!(&nombres, &pedidos, "el directorio tiene lo que se pidió y nada más");
        for (ruta, contenido) in &esperado {
            match contenido {
                Esperado::Aseprite(pila) => {
                    let mut otra = sesion_vacia();
                    otra.open(ruta, Canvas::MAX_SIDE, true).expect("reabrir");
                    let capas = otra.document.layers().len();
                    let leida = Pila::de_documento(&otra.document, capas - 1);
                    let r = pila.igual_contenido(&leida.canonica());
                    prop_assert!(r.is_ok(), "{}: {:?}", ruta.display(), r);
                }
                Esperado::Png(imagen) => {
                    let leido = read_png(&fs::read(ruta).expect("leer")).expect("PNG").image;
                    let r = igual_salvo_rgb_con_alfa_0(&leido, imagen);
                    prop_assert!(r.is_ok(), "{}: {:?}", ruta.display(), r);
                }
            }
        }
        prop_assert_eq!(comprobar_golden_aseprite(), Ok(()));
        golden_intacto();
    }
}
