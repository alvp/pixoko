//! Tests de `Document::state_id` (specs/hito-2-abrir-guardar.md, sección «Cambios en otras
//! crates»; filas 25 a 27 y propiedad 3).
//!
//! Ningún test reserva más de unos pocos KiB: las imágenes miden como mucho 4 x 4 y las
//! historias tienen como mucho unas decenas de pasos. El id es opaco: solo se compara por
//! igualdad y con `0` en el estado inicial; nunca se asume un valor concreto.

use std::collections::{HashMap, HashSet};

use proptest::collection::vec;
use proptest::prelude::*;
use proptest::test_runner::TestCaseError;
use px_core::{Command, Document, DocumentError, Image, ImageId, Mode, Pixel, PixelEdit, Rgba};

/// Colores de apoyo; `c0` es el valor inicial (transparente) de una imagen RGBA nueva.
fn color(n: u8) -> Rgba {
    match n {
        0 => Rgba::TRANSPARENT,
        1 => Rgba::new(40, 50, 60, 255),
        2 => Rgba::new(70, 80, 90, 255),
        _ => Rgba::new(100, 110, 120, 255),
    }
}

/// Documento de una imagen RGBA de 4 x 4 recién creado.
fn doc() -> Document {
    Document::new(Image::new(4, 4, Mode::Rgba).unwrap())
}

/// Comando de un solo píxel.
fn pone(image: ImageId, x: u32, y: u32, c: Rgba) -> Command {
    Command::SetPixels {
        image,
        pixels: vec![PixelEdit {
            x,
            y,
            pixel: Pixel::Rgba(c),
        }],
    }
}

/// Un `apply` que cambia seguro: el píxel `(i, 0)` pasa a un color que no tenía (el inicial
/// es transparente y los pasos de las pruebas no repiten píxel).
fn paso(doc: &mut Document, i: u32) {
    let id = doc.main_image();
    assert_eq!(doc.apply(pone(id, i, 0, color(1))), Ok(true), "paso {i}");
}

// ---------------------------------------------------------------------------------------
// Fila 25: documento nuevo, clon, apply con cambio / sin cambio / con error.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_25_documento_nuevo_tiene_id_cero() {
    // Fila 25: nuevo ⇒ 0.
    assert_eq!(doc().state_id(), 0);
}

#[test]
fn fila_25_el_clon_copia_el_id() {
    // Fila 25: clon ⇒ igual al original, tanto en el estado inicial como tras editar.
    let mut d = doc();
    assert_eq!(d.clone().state_id(), d.state_id());
    paso(&mut d, 0);
    paso(&mut d, 1);
    let clon = d.clone();
    assert_eq!(clon.state_id(), d.state_id());
    assert_ne!(clon.state_id(), 0);
    // El clon es independiente: editar uno no mueve el id del otro.
    let mut clon = clon;
    let antes = d.state_id();
    paso(&mut clon, 2);
    assert_eq!(d.state_id(), antes);
    assert_ne!(clon.state_id(), antes);
}

#[test]
fn fila_25_apply_con_cambio_da_ids_nuevos() {
    // Fila 25: tras `apply` con cambio, otro `apply`: ids distintos de 0 y entre sí.
    let mut d = doc();
    paso(&mut d, 0);
    let a = d.state_id();
    assert_ne!(a, 0);
    paso(&mut d, 1);
    let b = d.state_id();
    assert_ne!(b, 0);
    assert_ne!(b, a);
}

#[test]
fn fila_25_apply_sin_cambios_no_mueve_el_id() {
    // Fila 25: `apply` sin cambios (Ok(false)) ⇒ sin cambio de id.
    let mut d = doc();
    let id = d.main_image();
    // Pintar de transparente una imagen transparente no cambia nada.
    assert_eq!(d.apply(pone(id, 0, 0, color(0))), Ok(false));
    assert_eq!(d.state_id(), 0);
    // Igual tras un paso real: repetir el mismo píxel y color no cambia nada.
    paso(&mut d, 0);
    let a = d.state_id();
    assert_eq!(d.apply(pone(id, 0, 0, color(1))), Ok(false));
    assert_eq!(d.state_id(), a);
}

#[test]
fn fila_25_apply_con_error_no_mueve_el_id() {
    // Fila 25: `apply` con error (píxel fuera) ⇒ sin cambio de id.
    let mut d = doc();
    let id = d.main_image();
    assert!(d.apply(pone(id, 10, 10, color(1))).is_err());
    assert_eq!(d.state_id(), 0);
    // También con historia previa, y con un lote que falla a medias (la primera parte se
    // revierte y el id no se mueve).
    paso(&mut d, 0);
    let a = d.state_id();
    assert!(d.apply(pone(id, 4, 0, color(2))).is_err());
    assert_eq!(d.state_id(), a);
    let lote = Command::Batch(vec![pone(id, 1, 1, color(2)), pone(id, 9, 9, color(2))]);
    assert!(d.apply(lote).is_err());
    assert_eq!(d.state_id(), a);
    // Un lote vacío es un error y tampoco lo mueve.
    assert_eq!(
        d.apply(Command::Batch(vec![])),
        Err(DocumentError::EmptyCommand)
    );
    assert_eq!(d.state_id(), a);
}

// ---------------------------------------------------------------------------------------
// Fila 26: undo y redo recorren los ids; un id nunca se reutiliza.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_26_undo_y_redo_recorren_los_ids() {
    let mut d = doc();
    let cero = d.state_id();
    assert_eq!(cero, 0);
    paso(&mut d, 0);
    let a = d.state_id();
    paso(&mut d, 1);
    let b = d.state_id();
    // Fila 26: ids 0 → a → b, todos distintos.
    assert_ne!(a, cero);
    assert_ne!(b, cero);
    assert_ne!(a, b);

    // Deshacer devuelve el id de antes del paso: b → a → 0.
    d.undo().unwrap();
    assert_eq!(d.state_id(), a);
    d.undo().unwrap();
    assert_eq!(d.state_id(), cero);

    // Rehacer devuelve el de después: 0 → a → b.
    d.redo().unwrap();
    assert_eq!(d.state_id(), a);
    d.redo().unwrap();
    assert_eq!(d.state_id(), b);
}

#[test]
fn fila_26_un_apply_tras_deshacer_da_un_id_nunca_usado() {
    let mut d = doc();
    paso(&mut d, 0);
    let a = d.state_id();
    paso(&mut d, 1);
    let b = d.state_id();

    // Fila 26: `undo`, `apply` C ⇒ id nuevo distinto de a, b y 0, aunque la rama de rehacer
    // (el paso B) se descarte.
    d.undo().unwrap();
    assert_eq!(d.state_id(), a);
    let id = d.main_image();
    assert_eq!(d.apply(pone(id, 2, 0, color(2))), Ok(true));
    let c = d.state_id();
    assert_ne!(c, a);
    assert_ne!(c, b);
    assert_ne!(c, 0);

    // `redo` falla (la rama se descartó) y no cambia el id.
    assert_eq!(d.redo(), Err(DocumentError::NothingToRedo));
    assert_eq!(d.state_id(), c);

    // Deshacer C vuelve a `a`, no a `b`; y otro apply tampoco reutiliza ningún id.
    d.undo().unwrap();
    assert_eq!(d.state_id(), a);
    assert_eq!(d.apply(pone(id, 3, 0, color(3))), Ok(true));
    let e = d.state_id();
    for viejo in [0, a, b, c] {
        assert_ne!(e, viejo);
    }
}

#[test]
fn fila_26_deshacer_todo_y_rehacer_todo_da_los_mismos_ids() {
    // Fila 26 con una historia más larga: lo que recorre `undo` lo recorre `redo` al revés.
    let mut d = doc();
    let mut ids = vec![d.state_id()];
    for i in 0..4 {
        paso(&mut d, i);
        ids.push(d.state_id());
    }
    let distintos: HashSet<u64> = ids.iter().copied().collect();
    assert_eq!(distintos.len(), ids.len(), "ids repetidos: {ids:?}");
    for esperado in ids.iter().rev().skip(1) {
        d.undo().unwrap();
        assert_eq!(d.state_id(), *esperado);
    }
    for esperado in ids.iter().skip(1) {
        d.redo().unwrap();
        assert_eq!(d.state_id(), *esperado);
    }
}

// ---------------------------------------------------------------------------------------
// Fila 27: el límite de la historia y los errores de undo/redo no mueven el id.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_27_set_history_limit_no_cambia_ningun_id() {
    let mut d = doc();
    let mut ids = vec![d.state_id()];
    for i in 0..4 {
        paso(&mut d, i);
        ids.push(d.state_id());
    }
    // Cinco pasos (el quinto, en otro píxel) para tener historia que descartar.
    let id = d.main_image();
    assert_eq!(d.apply(pone(id, 0, 1, color(2))), Ok(true));
    ids.push(d.state_id());
    assert_eq!(ids.len(), 6);
    let actual = d.state_id();

    // Fila 27: `set_history_limit(0)` descarta pasos viejos, pero el id actual no cambia.
    d.set_history_limit(0);
    assert_eq!(d.state_id(), actual);

    // Cuántos `undo` quedan: lo da el propio documento (no se asume el tamaño de un paso),
    // pero con límite 0 y cinco pasos se descartó alguno.
    let mut copia = d.clone();
    let mut retenidos = 0;
    while copia.undo().is_ok() {
        retenidos += 1;
        assert!(retenidos <= 5);
    }
    assert!(retenidos >= 1, "queda al menos el paso más reciente");
    assert!(retenidos < 5, "el límite 0 descartó pasos viejos");

    // Los `undo` restantes dan los ids del historial retenido, en orden inverso, sin
    // saltarse ni inventar ninguno...
    for k in 1..=retenidos {
        d.undo().unwrap();
        assert_eq!(d.state_id(), ids[5 - k], "undo número {k}");
    }
    // ...y al llegar al paso más antiguo conservado el id no es 0: el 0 ya no es alcanzable.
    assert_ne!(d.state_id(), 0);
    assert_eq!(d.state_id(), ids[5 - retenidos]);
}

#[test]
fn fila_27_los_errores_de_undo_y_redo_no_cambian_el_id() {
    // Fila 27: `NothingToUndo` / `NothingToRedo` en un documento nuevo.
    let mut d = doc();
    assert_eq!(d.undo(), Err(DocumentError::NothingToUndo));
    assert_eq!(d.state_id(), 0);
    assert_eq!(d.redo(), Err(DocumentError::NothingToRedo));
    assert_eq!(d.state_id(), 0);

    // Y en los dos extremos de una historia: tras deshacerlo todo y tras rehacerlo todo.
    paso(&mut d, 0);
    let a = d.state_id();
    assert_eq!(d.redo(), Err(DocumentError::NothingToRedo));
    assert_eq!(d.state_id(), a);
    d.undo().unwrap();
    assert_eq!(d.undo(), Err(DocumentError::NothingToUndo));
    assert_eq!(d.state_id(), 0);
    d.redo().unwrap();
    assert_eq!(d.state_id(), a);
    assert_eq!(d.redo(), Err(DocumentError::NothingToRedo));
    assert_eq!(d.state_id(), a);
}

#[test]
fn fila_27_el_limite_tampoco_cambia_el_id_tras_deshacer() {
    // Fila 27, variante: con pasos por rehacer, bajar el límite no mueve el id actual ni
    // los ids que luego recorre `redo`.
    let mut d = doc();
    paso(&mut d, 0);
    let a = d.state_id();
    paso(&mut d, 1);
    let b = d.state_id();
    paso(&mut d, 2);
    let c = d.state_id();
    d.undo().unwrap();
    assert_eq!(d.state_id(), b);
    d.set_history_limit(0);
    assert_eq!(d.state_id(), b);
    d.redo().unwrap();
    assert_eq!(d.state_id(), c);
    // `a` sigue siendo distinto de los demás aunque su paso ya no se pueda alcanzar.
    assert_ne!(a, b);
    assert_ne!(a, c);
}

// ---------------------------------------------------------------------------------------
// Propiedad 3: secuencias aleatorias de apply, undo, redo y set_history_limit.
// ---------------------------------------------------------------------------------------

#[derive(Clone, Debug)]
enum Op {
    /// `apply` de un píxel; `x` llega a 4 (fuera de la imagen) para provocar errores, y el
    /// color 0 (transparente) da a menudo `Ok(false)`.
    Apply {
        x: u32,
        y: u32,
        color: u8,
    },
    Undo,
    Redo,
    Limit(usize),
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        4 => (0..5u32, 0..4u32, 0..4u8).prop_map(|(x, y, color)| Op::Apply { x, y, color }),
        2 => Just(Op::Undo),
        2 => Just(Op::Redo),
        1 => prop_oneof![Just(0usize), Just(1), Just(200), Just(64 << 20)].prop_map(Op::Limit),
    ]
}

fn imagen(d: &Document) -> Image {
    d.image(d.main_image()).unwrap().clone()
}

/// Dos instantes con el mismo id tienen la misma imagen.
fn registra(
    vistos: &mut HashMap<u64, Image>,
    d: &Document,
    contexto: &str,
) -> Result<(), TestCaseError> {
    let img = imagen(d);
    match vistos.get(&d.state_id()) {
        Some(previa) => prop_assert_eq!(previa, &img, "{}: mismo id, distinta imagen", contexto),
        None => {
            vistos.insert(d.state_id(), img);
        }
    }
    Ok(())
}

proptest! {
    /// Propiedad 3: ids nuevos solo con `apply` con cambios y nunca repetidos; `undo` y
    /// `redo` se compensan; mismo id ⇒ misma imagen; ni los errores ni `trim` mueven el id.
    #[test]
    fn propiedad_3_state_id_con_secuencias_aleatorias(ops in vec(op(), 0..40)) {
        let mut d = doc();
        // Ids ya dados (el 0 es el del estado inicial) y la imagen de cada uno.
        let mut dados: HashSet<u64> = HashSet::from([0]);
        let mut vistos: HashMap<u64, Image> = HashMap::new();
        registra(&mut vistos, &d, "inicio")?;

        for (n, op) in ops.iter().enumerate() {
            let antes = d.state_id();
            let ctx = format!("paso {n}: {op:?}");
            match op {
                Op::Apply { x, y, color: c } => {
                    let id = d.main_image();
                    match d.apply(pone(id, *x, *y, color(*c))) {
                        Ok(true) => {
                            // Un id nuevo, que nunca se había dado.
                            let nuevo = d.state_id();
                            prop_assert!(dados.insert(nuevo), "{}: id {} reutilizado", ctx, nuevo);
                            // Y rehacer ya no es posible: la rama se descartó.
                            prop_assert!(!d.can_redo(), "{}", ctx);
                        }
                        Ok(false) | Err(_) => {
                            prop_assert_eq!(d.state_id(), antes, "{}: sin cambio, mismo id", ctx);
                        }
                    }
                }
                Op::Undo => {
                    let mut copia = d.clone();
                    match d.undo() {
                        Ok(()) => {
                            // El id al que se llega ya se había dado.
                            prop_assert!(dados.contains(&d.state_id()), "{}", ctx);
                            // `undo` seguido de `redo` devuelve el mismo id.
                            copia.undo().unwrap();
                            copia.redo().unwrap();
                            prop_assert_eq!(copia.state_id(), antes, "{}: undo y redo", ctx);
                        }
                        Err(_) => {
                            prop_assert_eq!(d.state_id(), antes, "{}: error sin cambio", ctx);
                        }
                    }
                }
                Op::Redo => {
                    let mut copia = d.clone();
                    match d.redo() {
                        Ok(()) => {
                            prop_assert!(dados.contains(&d.state_id()), "{}", ctx);
                            // `redo` seguido de `undo` devuelve el mismo id.
                            copia.redo().unwrap();
                            copia.undo().unwrap();
                            prop_assert_eq!(copia.state_id(), antes, "{}: redo y undo", ctx);
                        }
                        Err(_) => {
                            prop_assert_eq!(d.state_id(), antes, "{}: error sin cambio", ctx);
                        }
                    }
                }
                Op::Limit(bytes) => {
                    d.set_history_limit(*bytes);
                    prop_assert_eq!(d.state_id(), antes, "{}: el límite no mueve el id", ctx);
                }
            }
            // Dos instantes con el mismo id tienen la misma imagen.
            registra(&mut vistos, &d, &ctx)?;
        }
    }
}
