//! Tests de `new_gate` (specs/hito-2-nueva-imagen.md, unidad 19, fila 10 y D6): la confirmación por
//! repetición al **abrir** el diálogo de «Nueva imagen…», como `open_gate` con `Ctrl+O`.
//!
//! - Sin cambios sin guardar: `true` a la primera y sin armar nada.
//! - Con cambios: la primera petición da `false` (queda armada y `App` avisa con `new_warning()`);
//!   la segunda, dentro de `CONFIRM_SECS`, da `true`; pasado el plazo vuelve a `false`.
//! - Una `Open`, una `Close` o una `OpenDialog` entre medias sustituyen a la armada.
//! - `Quit` con el diálogo abierto lo abandona sin crear nada y aplica la regla de cierre de siempre.
//! - Tras confirmar, aceptar reemplaza sin volver a preguntar (`Session::new_image(.., true)`).
//!
//! Suposiciones donde la spec no fija un detalle:
//! - `new_gate(modified, &mut Confirm, now) -> bool` usa `Request::NewImage` y, con
//!   `modified == false`, no toca el `Confirm` (como `close_allowed` y `open_gate`).
//! - La aceptación se prueba con `Session::new_image(imagen, true)`: es lo que hace `App` con el
//!   descarte que recordó `new_gate`; el diálogo (`Modal`) se abre con `open_new_image`.

use super::*;

use px_core::{Command, Document, Image, Mode, Pixel, PixelEdit, Rgba};
use px_ui::actions::Locks;
use px_ui::dialog::{DialogKind as ModalKind, Modal};

use crate::confirm::close_allowed;
use crate::files::FileError;
use crate::new_image::open_new_image;
use crate::session::Session;

fn sesion(modificada: bool) -> Session {
    let image = Image::new(4, 4, Mode::Rgba).expect("imagen");
    let mut session = Session::sample(Document::new(image));
    if modificada {
        let id = session.document.main_image();
        session
            .document
            .apply(Command::SetPixels {
                image: id,
                pixels: vec![PixelEdit {
                    x: 0,
                    y: 0,
                    pixel: Pixel::Rgba(Rgba::opaque(1, 2, 3)),
                }],
            })
            .expect("editar");
        assert!(session.modified());
    }
    session
}

fn imagen_nueva() -> Image {
    Image::new(8, 8, Mode::Rgba).expect("imagen")
}

// ---------------------------------------------------------------------------------------
// Fila 10: sin cambios.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_10_sin_cambios_new_gate_da_true_a_la_primera_y_no_arma_nada() {
    let mut confirm = Confirm::new();
    for now in [0.0, 0.1, 100.0] {
        assert!(new_gate(false, &mut confirm, now), "now {now}");
    }
    // No quedó nada armado: la primera petición con cambios arma en vez de confirmar.
    assert_eq!(
        confirm.request(Request::NewImage, 100.5),
        Confirmation::Armed
    );
}

// ---------------------------------------------------------------------------------------
// Fila 10: con cambios.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_10_con_cambios_la_primera_peticion_arma_y_la_segunda_dentro_del_plazo_confirma() {
    let mut confirm = Confirm::new();
    assert!(!new_gate(true, &mut confirm, 0.0), "la primera arma");
    assert!(new_gate(true, &mut confirm, 4.9), "la segunda confirma");
    // Al confirmar se vació: la tercera vuelve a armar.
    assert!(!new_gate(true, &mut confirm, 5.0));
}

#[test]
fn fila_10_pasado_el_plazo_vuelve_a_armar_y_la_siguiente_dentro_de_plazo_confirma() {
    let mut confirm = Confirm::new();
    assert!(!new_gate(true, &mut confirm, 0.0));
    assert!(
        !new_gate(true, &mut confirm, CONFIRM_SECS),
        "a los CONFIRM_SECS justos la primera ya caducó"
    );
    assert!(new_gate(true, &mut confirm, CONFIRM_SECS + 1.0));
}

#[test]
fn fila_10_la_peticion_armada_es_request_new_image() {
    let mut confirm = Confirm::new();
    assert!(!new_gate(true, &mut confirm, 0.0));
    assert!(confirm.confirm_armed(&Request::NewImage, 1.0));
}

#[test]
fn fila_10_una_open_close_u_open_dialog_entre_medias_sustituye_a_la_armada() {
    // Cada «otra petición» se arma entre las dos de «Nueva imagen»: la segunda no confirma.
    type Arma = fn(&mut Confirm, f64);
    let armas: [(&str, Arma); 3] = [
        ("Open", |c, now| {
            c.request(Request::Open("a.png".into()), now);
        }),
        ("Close", |c, now| {
            assert!(!close_allowed(true, c, now));
        }),
        ("OpenDialog", |c, now| {
            assert_eq!(open_gate(true, c, now), None);
        }),
    ];
    for (nombre, arma) in armas {
        let mut confirm = Confirm::new();
        assert!(!new_gate(true, &mut confirm, 0.0), "{nombre}");
        arma(&mut confirm, 1.0);
        assert!(
            !new_gate(true, &mut confirm, 2.0),
            "{nombre}: la otra petición sustituyó a la armada"
        );
        // Y ahora sí: la armada es otra vez la de «Nueva imagen».
        assert!(new_gate(true, &mut confirm, 3.0), "{nombre}");
    }
}

#[test]
fn fila_10_new_gate_sustituye_a_una_close_armada_y_close_allowed_sigue_su_regla() {
    let mut confirm = Confirm::new();
    assert!(!close_allowed(true, &mut confirm, 0.0), "Close armada");
    assert!(
        !new_gate(true, &mut confirm, 1.0),
        "NewImage no la confirma"
    );
    // La armada ya es `NewImage`: cerrar vuelve a armar `Close`.
    assert!(!close_allowed(true, &mut confirm, 2.0));
    assert!(close_allowed(true, &mut confirm, 3.0));
}

// ---------------------------------------------------------------------------------------
// Fila 10: `Quit` con el diálogo abierto.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_10_quit_con_el_dialogo_abierto_lo_abandona_sin_crear_nada_y_sigue_la_regla_de_cierre() {
    let session = sesion(true);
    let imagen_antes = session
        .document
        .image(session.document.main_image())
        .expect("imagen")
        .clone();
    let mut confirm = Confirm::new();
    assert!(!new_gate(true, &mut confirm, 0.0));
    assert!(new_gate(true, &mut confirm, 1.0));
    let mut modal = Modal::default();
    assert!(open_new_image(&mut modal, 16384, Locks::default()));
    assert_eq!(modal.kind(), Some(ModalKind::NewImage));

    // El cierre de la ventana: con cambios, la primera petición no cierra (arma `Close`) y la
    // segunda sí; en las dos el diálogo se abandona y no se crea nada.
    assert!(modal.abandon());
    assert!(!modal.is_open());
    assert!(!close_allowed(session.modified(), &mut confirm, 2.0));
    assert!(close_allowed(session.modified(), &mut confirm, 3.0));

    assert!(session.modified(), "nada se creó: sigue con sus cambios");
    assert_eq!(
        session
            .document
            .image(session.document.main_image())
            .expect("imagen"),
        &imagen_antes
    );
}

#[test]
fn fila_10_sin_cambios_quit_con_el_dialogo_abierto_cierra_a_la_primera() {
    let mut confirm = Confirm::new();
    let mut modal = Modal::default();
    assert!(open_new_image(&mut modal, 16384, Locks::default()));
    assert!(modal.abandon());
    assert!(close_allowed(false, &mut confirm, 0.0));
}

// ---------------------------------------------------------------------------------------
// Fila 10: aceptar tras confirmar.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_10_tras_confirmar_aceptar_reemplaza_sin_volver_a_preguntar() {
    let mut session = sesion(true);
    let mut confirm = Confirm::new();
    assert!(!new_gate(session.modified(), &mut confirm, 0.0));
    let discard = new_gate(session.modified(), &mut confirm, 1.0);
    assert!(
        discard,
        "la segunda petición confirma y recuerda el descarte"
    );

    // Aceptar: el descarte recordado basta; no se consulta el `Confirm`.
    assert_eq!(session.new_image(imagen_nueva(), discard), Ok(()));
    assert!(!session.modified());
    assert!(!session.document.can_undo());
    assert_eq!(
        confirm.request(Request::NewImage, 1.5),
        Confirmation::Armed,
        "aceptar no dejó nada armado ni consumió nada más"
    );
}

#[test]
fn fila_10_sin_el_descarte_recordado_aceptar_con_cambios_no_reemplaza() {
    let mut session = sesion(true);
    let before = session.document.state_id();
    assert_eq!(
        session.new_image(imagen_nueva(), false),
        Err(FileError::Unsaved)
    );
    assert_eq!(session.document.state_id(), before);
    assert!(session.modified());
}

#[test]
fn fila_10_tras_crear_la_imagen_nueva_no_tiene_cambios_y_el_siguiente_new_gate_abre_a_la_primera() {
    let mut session = sesion(true);
    let mut confirm = Confirm::new();
    assert!(!new_gate(session.modified(), &mut confirm, 0.0));
    let discard = new_gate(session.modified(), &mut confirm, 1.0);
    session.new_image(imagen_nueva(), discard).expect("crear");
    assert!(new_gate(session.modified(), &mut confirm, 2.0));
}
