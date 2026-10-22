//! Filas 1 a 10 de la puerta del hito 2 (specs/hito-2-puerta.md). La fila 11 está en
//! `crates/px-golden/tests/png_interop/gate_oraculo.rs` y la 12 en `grande.rs`.
//!
//! Cada test crea su directorio de trabajo (`Trabajo`), copia allí los reales y abre la copia:
//! nada se escribe en `golden/png/` y el guardián de `Trabajo` lo comprueba al terminar.
//!
//! **Suposiciones y ambigüedades:**
//! - Fila 1, «`converted()` coincide con que el informe o `aux` no esté vacío»: se compara con
//!   «informe no vacío o `aux.appearance` presente» (los auxiliares inocuos no fuerzan copia).
//! - Fila 6, «`-copia-2` al repetir»: tras un `save_as` la ruta del documento es la copia, así que
//!   repetir sobre ella daría `x-copia-copia.png`; el `-copia-2` sale al repetir desde una sesión
//!   nueva que abre el mismo original.
//! - Fila 9, «al terminar toda la puerta»: las pruebas corren en paralelo y no hay un «final»; la
//!   instantánea inicial se toma una sola vez (`OnceLock`) antes de que nada escriba, cada
//!   `Trabajo` comprueba al soltarse que `golden/png/` sigue igual, y un test dedicado lo comprueba
//!   también, además del SHA-256 de cada uno de los 4 PNG.
//! - Fila 2, el borrador de una indexada sin índice transparente: si la paleta tiene una entrada de
//!   alfa guardado 0 el borrador la usa (spec del cuentagotas y el borrador), así que solo se
//!   rechaza cuando no hay ninguna; el test de rechazos usa una paleta sin alfa 0.

use px_core::ImageId;
use std::fs;

use px_core::{Allowed, ConvertError, Document, DocumentError, Mode};
use px_formats::{AuxReport, read_png};
use px_ui::canvas::Canvas;

use crate::confirm::Confirm;
use crate::dialogs::{DialogKind, Finished, Outcome, open_gate};
use crate::files::FileError;
use crate::session::Session;

use super::comun::sha256::{sha256_hex, vectores};
use super::comun::*;

/// Texto del modo que lleva el aviso de apertura.
fn texto_del_modo(modo: Mode) -> &'static str {
    match modo {
        Mode::Indexed => "indexado",
        Mode::Gray => "gris",
        Mode::GrayAlpha => "gris con alfa",
        Mode::Rgba => "RGBA",
    }
}

/// Pixoko convierte el fichero al abrir: el informe de `read_png` no está vacío o hay un auxiliar
/// que cambia la apariencia.
fn convierte(fuente: &Fuente) -> bool {
    let leido = read_png(&fuente.bytes).expect("read_png de la fuente");
    !leido.report.is_empty() || leido.aux.appearance.is_some()
}

// ---------------------------------------------------------------------------------------
// Fila 1: abrir.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_1_open_de_los_reales_y_los_generados() {
    let t = Trabajo::nuevo();
    for fuente in todas() {
        let ctx = fuente.nombre.clone();
        let ruta = t.poner(&fuente);
        let mut s = sesion_vacia();
        let aviso = s
            .open(&ruta, Canvas::MAX_SIDE, false)
            .unwrap_or_else(|e| panic!("{ctx}: open dio error: {e}"));
        let imagen = s
            .document
            .image(ImageId::from_raw(0))
            .expect("el documento tiene imagen");

        // Imagen = lectura independiente del fichero (modelo, o el .sha256 de los reales).
        comprobar_apertura(&fuente, imagen);
        // Modo y tamaño, del IHDR.
        let (ancho, alto, _, _) = ihdr(&fuente.bytes);
        assert_eq!(
            (imagen.width(), imagen.height()),
            (ancho, alto),
            "{ctx}: tamaño"
        );
        assert_eq!(
            imagen.mode(),
            modo_del_fichero(&fuente.bytes),
            "{ctx}: modo"
        );

        // Estado de la sesión recién abierta.
        assert_eq!(s.document.state_id(), 0, "{ctx}: state_id");
        assert!(!s.modified(), "{ctx}: modified");
        assert!(!s.document.can_undo(), "{ctx}: hay historia que deshacer");
        assert!(!s.document.can_redo(), "{ctx}: hay historia que rehacer");
        assert_eq!(s.path(), Some(ruta.as_path()), "{ctx}: ruta");

        // El aviso lleva nombre, tamaño y modo.
        assert!(aviso.contains(&fuente.nombre), "{ctx}: sin nombre: {aviso}");
        assert!(
            aviso.contains(&format!("{ancho}×{alto}")),
            "{ctx}: sin tamaño: {aviso}"
        );
        let modo = texto_del_modo(imagen.mode());
        assert!(
            aviso.contains(&format!(" · {modo}")),
            "{ctx}: sin modo {modo}: {aviso}"
        );
        if imagen.mode() == Mode::Gray {
            assert!(
                !aviso.contains("gris con alfa"),
                "{ctx}: modo equivocado: {aviso}"
            );
        }

        // `converted()` coincide con el informe y los auxiliares de `read_png`.
        assert_eq!(s.converted(), convierte(&fuente), "{ctx}: converted()");
        if let Some(esperada) = fuente.convertida {
            assert_eq!(
                s.converted(),
                esperada,
                "{ctx}: converted() por construcción"
            );
        }
    }
}

// ---------------------------------------------------------------------------------------
// Fila 2: herramientas.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_2_lapiz_linea_relleno_y_borrador_sobre_cada_fuente() {
    for fuente in todas() {
        let t = Trabajo::nuevo();
        let mut w = abrir(&t, &fuente);
        let guion = guion_herramientas(&w.m);
        let resultados: Vec<bool> = guion.iter().map(|paso| w.aplicar(paso)).collect();
        let con_cambio = resultados.iter().filter(|&&c| c).count();
        let ctx = &fuente.nombre;

        assert!(
            con_cambio >= 3,
            "{ctx}: el guion casi no hizo nada ({resultados:?})"
        );
        // Un paso de historia por gesto con cambio.
        assert_eq!(w.pasos(), con_cambio, "{ctx}: pasos de historia");
        // El mismo trazo con el mismo color y el relleno de un mismo color no dan paso.
        assert!(
            !resultados[1],
            "{ctx}: repetir el trazo con el mismo color dio paso"
        );
        assert!(
            !resultados[6],
            "{ctx}: rellenar con el mismo color dio paso"
        );
        if w.m.modo == Mode::Gray {
            // El borrador se rechaza en gris, sin paso.
            assert_eq!(w.rechazos, 2, "{ctx}: borrador en gris");
            assert!(
                !resultados[7] && !resultados[8],
                "{ctx}: el borrador cambió un gris"
            );
        }
        // `undo` deshace exactamente esos pasos.
        assert_eq!(
            w.deshacer_todo(),
            con_cambio,
            "{ctx}: undo hasta el principio"
        );
        assert!(!w.s.modified(), "{ctx}: modified al volver al principio");
    }
}

#[test]
fn fila_2_borrador_rechazado_en_gris_y_en_indexado_sin_transparente() {
    // Gris: sin transparencia.
    let gris = modelo_de(Mode::Gray, 4, 4, vec![[100, 0, 0, 0]; 16]);
    let mut w = Mundo::de_sesion(Session::sample(Document::new(gris.a_imagen())), "gris");
    assert!(!w.aplicar(&Paso::Borrador(vec![(1, 1), (2, 1)])));
    assert_eq!(w.rechazos, 1, "gris: el borrador debía rechazarse");
    assert_eq!(w.s.document.state_id(), 0, "gris: el rechazo dio paso");
    assert!(!w.s.document.can_undo());

    // Indexada sin índice transparente ni entradas de alfa 0: rechazado hasta fijarlo.
    let mut indexada = modelo_de(Mode::Indexed, 4, 4, vec![[0, 0, 0, 0]; 16]);
    indexada.paleta = vec![[10, 10, 10, 255], [200, 0, 0, 255], [0, 200, 0, 255]];
    let mut w = Mundo::de_sesion(
        Session::sample(Document::new(indexada.a_imagen())),
        "indexada",
    );
    assert!(!w.aplicar(&Paso::Borrador(vec![(1, 1), (2, 1)])));
    assert_eq!(w.rechazos, 1, "indexada: el borrador debía rechazarse");
    assert_eq!(w.pasos(), 0, "indexada: el rechazo dio paso");

    // Al fijar el índice transparente, el borrador ya escribe en él.
    assert!(w.aplicar(&Paso::Transparente(Some(1))));
    assert!(w.aplicar(&Paso::Borrador(vec![(1, 1), (2, 1)])));
    assert_eq!(w.rechazos, 1, "indexada: ya no debía rechazarse");
    assert_eq!(w.pasos(), 2);
    assert_eq!(
        w.m.celdas[4 + 1],
        [1, 0, 0, 0],
        "el borrador no escribió el índice transparente"
    );
}

#[test]
fn fila_2_el_comparador_de_imagenes_sabe_fallar() {
    // Control: un modelo con un píxel cambiado no es la imagen.
    let modelo = generadas()[0].esperado.clone().expect("modelo");
    let imagen = modelo.a_imagen();
    assert_igual(&imagen, &modelo, "control: igual");
    let mut otro = modelo.clone();
    otro.celdas[5][0] ^= 1;
    let resultado = std::panic::catch_unwind(|| assert_igual(&imagen, &otro, "control"));
    assert!(resultado.is_err(), "assert_igual no vio un píxel distinto");
}

// ---------------------------------------------------------------------------------------
// Fila 3: paleta.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_3_paleta_de_las_indexadas() {
    let mut vistas = 0;
    for fuente in todas()
        .into_iter()
        .filter(|f| modo_del_fichero(&f.bytes) == Mode::Indexed)
    {
        vistas += 1;
        let t = Trabajo::nuevo();
        let mut w = abrir(&t, &fuente);
        let ctx = fuente.nombre.clone();
        let mut cambios = 0;
        for paso in guion_paleta(&w.m) {
            let antes_bytes = w.imagen().bytes().to_vec();
            let antes_colores = w.imagen().palette().expect("paleta").colors().to_vec();
            if w.aplicar(&paso) {
                cambios += 1;
            }
            if let Paso::Transparente(_) = paso {
                // `SetTransparent` solo mueve la marca: los índices y los colores guardados no cambian.
                assert!(
                    w.imagen().bytes() == &antes_bytes[..],
                    "{ctx}: SetTransparent cambió los índices"
                );
                assert_eq!(
                    w.imagen().palette().expect("paleta").colors(),
                    &antes_colores[..],
                    "{ctx}: SetTransparent cambió los colores guardados"
                );
            }
        }
        assert!(
            cambios >= 6,
            "{ctx}: el guion casi no hizo nada ({cambios} pasos)"
        );
        // Un paso por operación.
        assert_eq!(w.pasos(), cambios, "{ctx}: pasos de historia");
        assert_eq!(w.deshacer_todo(), cambios, "{ctx}: undo hasta el principio");
    }
    assert!(
        vistas >= 4,
        "se esperaban al menos 4 indexadas y hubo {vistas}"
    );
}

// ---------------------------------------------------------------------------------------
// Fila 4: conversión de modo.
// ---------------------------------------------------------------------------------------

/// Convertir a gris sin permiso perdería algo: algún píxel con alfa < 255 (se compondría) o a > 0
/// con r, g y b distintos (se llevaría a gris).
fn pierde_al_pasar_a_gris(m: &Modelo) -> bool {
    m.rgba()
        .as_chunks::<4>()
        .0
        .iter()
        .any(|p| p[3] < 255 || (p[3] > 0 && !(p[0] == p[1] && p[1] == p[2])))
}

#[test]
fn fila_4_conversion_exacta_y_con_perdida() {
    let (mut exactas, mut con_perdida) = (0, 0);
    for fuente in todas() {
        let t = Trabajo::nuevo();
        let mut w = abrir(&t, &fuente);
        let ctx = fuente.nombre.clone();
        let original = w.imagen().try_clone().expect("copia");

        // La que pierde datos sin permiso da `Err` y no toca ni el documento ni la historia.
        if w.m.modo != Mode::Gray && pierde_al_pasar_a_gris(&w.m) {
            let resultado = w.s.convert_mode(Mode::Gray, Allowed::NOTHING);
            assert!(
                matches!(
                    resultado,
                    Err(DocumentError::Convert(ConvertError::LossNotAllowed { .. }))
                ),
                "{ctx}: a gris sin permiso dio {resultado:?}"
            );
            assert_eq!(
                w.s.document.state_id(),
                0,
                "{ctx}: el error cambió el state_id"
            );
            assert!(!w.s.document.can_undo(), "{ctx}: el error dejó un paso");
            assert!(w.imagen() == &original, "{ctx}: el error cambió la imagen");
            assert!(!w.s.modified(), "{ctx}: el error marcó cambios");
            w.comprobar();
            con_perdida += 1;
        }

        // El camino exacto de su modo da el modelo en un paso.
        let cambio = w.aplicar(&Paso::Convertir);
        if cambio {
            exactas += 1;
            assert_eq!(w.pasos(), 1, "{ctx}: la conversión no fue un solo paso");
            w.deshacer();
            assert!(
                w.imagen() == &original,
                "{ctx}: undo no devolvió la original"
            );
            w.rehacer();
        }
    }
    assert!(
        con_perdida >= 3,
        "pocas conversiones con pérdida: {con_perdida}"
    );
    assert!(exactas >= 7, "pocas conversiones exactas: {exactas}");
}

#[test]
fn fila_4_demasiados_colores_da_error_sin_cambiar_nada() {
    let muchos = rgba_de_muchos_colores();
    let mut w = Mundo::de_sesion(Session::sample(Document::new(muchos.a_imagen())), "muchos");
    let original = w.imagen().try_clone().expect("copia");
    let resultado = w.s.convert_mode(Mode::Indexed, Allowed::NOTHING);
    assert!(
        matches!(
            resultado,
            Err(DocumentError::Convert(ConvertError::TooManyColors { .. }))
        ),
        "RGBA con más de 256 colores a indexado dio {resultado:?}"
    );
    assert_eq!(w.s.document.state_id(), 0);
    assert!(!w.s.document.can_undo());
    assert!(w.imagen() == &original);
    // El mismo caso por el camino del modelo.
    assert!(!w.aplicar(&Paso::Convertir));
    assert_eq!(w.pasos(), 0);
}

// ---------------------------------------------------------------------------------------
// Fila 5: deshacer y rehacer.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_5_deshacer_y_rehacer_el_guion_completo() {
    for fuente in todas() {
        let t = Trabajo::nuevo();
        let mut w = abrir(&t, &fuente);
        let ctx = fuente.nombre.clone();
        let original = w.imagen().try_clone().expect("copia");
        for paso in guion_completo(&w.m) {
            w.aplicar(&paso);
        }
        let n = w.pasos();
        assert!(n >= 4, "{ctx}: el guion casi no hizo nada ({n} pasos)");
        let final_ = w.imagen().try_clone().expect("copia");

        // `undo` hasta el principio: cada paso da su instantánea (lo comprueba el mundo).
        assert_eq!(w.deshacer_todo(), n, "{ctx}: pasos deshechos");
        assert!(
            w.imagen() == &original,
            "{ctx}: no volvió la imagen original exacta"
        );
        assert_eq!(w.s.document.state_id(), 0, "{ctx}: state_id al principio");
        assert!(!w.s.modified(), "{ctx}: modified al principio");

        // `redo` hasta el final.
        assert_eq!(w.rehacer_todo(), n, "{ctx}: pasos rehechos");
        assert!(w.imagen() == &final_, "{ctx}: no volvió la imagen final");
        assert!(w.s.modified(), "{ctx}: modified al final");

        // Se guarda el estado final; después, `undo` ×2 + edición nueva + `redo`.
        guardar(&mut w.s, t.ruta()).unwrap_or_else(|e| panic!("{ctx}: save dio {e}"));
        w.marcar_guardado();
        w.deshacer();
        w.deshacer();
        assert!(w.s.modified(), "{ctx}: modified tras deshacer del guardado");
        let nueva = edicion_que_cambia(&w.m);
        assert!(w.aplicar(&nueva), "{ctx}: la edición nueva no cambió nada");
        // `redo` falla (lo comprueba el mundo) y el estado guardado ya no se alcanza.
        w.rehacer();
        assert!(w.s.modified(), "{ctx}: modified tras la edición nueva");
        while w.pos > 0 {
            w.deshacer();
            assert!(
                w.s.modified(),
                "{ctx}: el estado guardado volvió a ser alcanzable (undo)"
            );
        }
        while w.pos + 1 < w.snaps.len() {
            w.rehacer();
            assert!(
                w.s.modified(),
                "{ctx}: el estado guardado volvió a ser alcanzable (redo)"
            );
        }
    }
}

// ---------------------------------------------------------------------------------------
// Fila 6: save y save_as.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_6_save_y_save_as_tras_el_guion() {
    for fuente in todas() {
        let t = Trabajo::nuevo();
        let mut w = abrir(&t, &fuente);
        let ctx = fuente.nombre.clone();
        for paso in guion_completo(&w.m) {
            w.aplicar(&paso);
        }
        let abierta = w.s.path().expect("ruta").to_path_buf();
        let convertida = w.s.converted();
        let bytes_abierta = fs::read(&abierta).expect("leer la copia");

        // `save`: sobrescribe la copia (o escribe `-copia` si el fichero se convierte).
        let destino = if convertida {
            libre_copia(&abierta)
        } else {
            abierta.clone()
        };
        let guardado = guardar(&mut w.s, t.ruta()).unwrap_or_else(|e| panic!("{ctx}: save: {e}"));
        assert_eq!(guardado.path, destino, "{ctx}: destino de save");
        assert_eq!(
            guardado.copy_reason.is_some(),
            convertida,
            "{ctx}: copy_reason"
        );
        w.marcar_guardado();
        assert_eq!(w.s.path(), Some(destino.as_path()), "{ctx}: ruta tras save");
        assert_igual(
            &leer(&destino).image,
            &w.m.normalizada(),
            &format!("{ctx}: relectura tras save"),
        );
        sin_temporales(&t.nombres());
        if convertida {
            assert_eq!(
                fs::read(&abierta).expect("leer el original"),
                bytes_abierta,
                "{ctx}: save tocó el original que se convierte"
            );
        }

        // `save_as`: `x-copia.png`, el fichero anterior intacto y la ruta nueva.
        w.aplicar(&edicion_que_cambia(&w.m));
        let anterior = fs::read(&destino).expect("leer lo guardado");
        let esperada = libre_copia(&destino);
        let copia =
            guardar_como(&mut w.s, t.ruta()).unwrap_or_else(|e| panic!("{ctx}: save_as: {e}"));
        assert_eq!(copia.path, esperada, "{ctx}: destino de save_as");
        assert!(
            copia.copy_reason.is_none(),
            "{ctx}: save_as dio copy_reason"
        );
        w.marcar_guardado();
        assert_eq!(
            w.s.path(),
            Some(copia.path.as_path()),
            "{ctx}: ruta tras save_as"
        );
        assert_eq!(
            fs::read(&destino).expect("leer lo anterior"),
            anterior,
            "{ctx}: save_as tocó el fichero anterior"
        );
        assert_igual(
            &leer(&copia.path).image,
            &w.m.normalizada(),
            &format!("{ctx}: relectura tras save_as"),
        );

        // Repetir desde el mismo original: `-copia-2`, sin tocar la primera copia.
        let primera_copia = fs::read(&copia.path).expect("leer la copia");
        let mut s2 = sesion_vacia();
        s2.open(&abierta, Canvas::MAX_SIDE, false)
            .unwrap_or_else(|e| panic!("{ctx}: reabrir el original: {e}"));
        let mut w2 = Mundo::de_sesion(s2, &ctx);
        w2.aplicar(&edicion_que_cambia(&w2.m));
        let esperada_2 = libre_copia(&abierta);
        assert!(
            esperada_2.to_string_lossy().ends_with("-copia-2.png"),
            "{ctx}: se esperaba -copia-2 y es {}",
            esperada_2.display()
        );
        let copia_2 =
            guardar_como(&mut w2.s, t.ruta()).unwrap_or_else(|e| panic!("{ctx}: save_as 2: {e}"));
        assert_eq!(
            copia_2.path, esperada_2,
            "{ctx}: destino del segundo save_as"
        );
        assert_eq!(
            fs::read(&copia.path).expect("leer la primera copia"),
            primera_copia,
            "{ctx}: el segundo save_as tocó la primera copia"
        );
        assert_igual(
            &leer(&copia_2.path).image,
            &w2.m.normalizada(),
            &format!("{ctx}: relectura tras el segundo save_as"),
        );
        sin_temporales(&t.nombres());
    }
}

#[test]
fn fila_6_un_real_abierto_directamente_se_guarda_en_el_trabajo() {
    for fuente in reales() {
        let t = Trabajo::nuevo();
        let ctx = fuente.nombre.clone();
        // La apertura directa del real es solo lectura.
        let real = golden_png().join(&fuente.nombre);
        let mut s = sesion_vacia();
        s.open(&real, Canvas::MAX_SIDE, false)
            .unwrap_or_else(|e| panic!("{ctx}: open del real: {e}"));
        comprobar_apertura(
            &fuente,
            s.document.image(ImageId::from_raw(0)).expect("imagen"),
        );
        let mut w = Mundo::de_sesion(s, &ctx);
        w.aplicar(&edicion_que_cambia(&w.m));

        // Su primer guardado es `save_to(<trabajo>/…)`.
        let elegido = t.ruta().join("guardado.png");
        let guardado =
            guardar_en(&mut w.s, &elegido).unwrap_or_else(|e| panic!("{ctx}: save_to: {e}"));
        assert_eq!(guardado.path, elegido, "{ctx}: destino");
        assert!(guardado.copy_reason.is_none(), "{ctx}: copy_reason");
        w.marcar_guardado();
        assert_eq!(w.s.path(), Some(elegido.as_path()), "{ctx}: ruta");
        assert_igual(&leer(&elegido).image, &w.m.normalizada(), &ctx);
        assert_eq!(
            t.nombres(),
            vec!["guardado.png".to_string()],
            "{ctx}: ficheros del trabajo"
        );
    }
}

// ---------------------------------------------------------------------------------------
// Fila 7: diálogos con selector falso.
// ---------------------------------------------------------------------------------------

/// Resultado de repetir el pegamento de `main.rs` (`finish_dialog`) sin egui.
enum Pegado {
    Cancelado,
    Guardado(Result<crate::session::Saved, FileError>),
    Abierto(Result<String, FileError>),
}

/// Lo que hacen `finish_dialog` y `open_file` de `main.rs` con un diálogo terminado: `Chosen` con
/// `Save` o `SaveAs` es `save_to`, y con `Open`, abrir con el descarte que dijo el diálogo.
fn pegamento(s: &mut Session, terminado: Finished) -> Pegado {
    match terminado.outcome {
        Outcome::Cancelled => Pegado::Cancelado,
        Outcome::Unavailable => panic!("el selector falso nunca falla"),
        Outcome::Chosen(ruta) => match terminado.kind {
            DialogKind::Open => Pegado::Abierto(s.open(&ruta, Canvas::MAX_SIDE, terminado.discard)),
            DialogKind::Save | DialogKind::SaveAs => Pegado::Guardado(guardar_en(s, &ruta)),
        },
    }
}

#[test]
fn fila_7_dialogos_con_selector_falso() {
    let t = Trabajo::nuevo();
    let generada = generadas().remove(1);
    let imagen = generada.esperado.clone().expect("modelo").a_imagen();
    let mut w = Mundo::de_sesion(Session::sample(Document::new(imagen)), "dialogos");
    let (mut dialogos, mando) = dialogos(t.ruta());
    w.aplicar(&edicion_que_cambia(&w.m));
    assert!(w.s.modified() && w.s.path().is_none());

    // `Ctrl+S` sin ruta: propone `sin-titulo.png`; el resultado `T/mi-dibujo` escribe
    // `mi-dibujo.png` y fija la ruta.
    assert!(dialogos.begin(
        DialogKind::Save,
        false,
        w.s.path(),
        w.s.converted(),
        0.0,
        None
    ));
    let peticion = mando.ultima();
    assert_eq!(peticion.kind, DialogKind::Save);
    assert_eq!(peticion.file_name.as_deref(), Some("sin-titulo.png"));
    assert!(dialogos.pending());
    assert!(
        dialogos.poll(0.1).is_none(),
        "el diálogo debía seguir abierto"
    );
    mando.elegir(&t.ruta().join("mi-dibujo"));
    let terminado = dialogos.poll(1.0).expect("el diálogo terminó");
    assert!(!dialogos.pending());
    assert_eq!(terminado.kind, DialogKind::Save);
    let Pegado::Guardado(resultado) = pegamento(&mut w.s, terminado) else {
        panic!("Save no guardó");
    };
    let guardado = resultado.expect("Save guardó");
    let primero = t.ruta().join("mi-dibujo.png");
    assert_eq!(guardado.path, primero);
    assert!(guardado.copy_reason.is_none());
    w.marcar_guardado();
    assert_eq!(w.s.path(), Some(primero.as_path()), "Save no fijó la ruta");
    assert_eq!(t.nombres(), vec!["mi-dibujo.png".to_string()]);
    assert_igual(&leer(&primero).image, &w.m.normalizada(), "mi-dibujo.png");
    let bytes_primero = fs::read(&primero).expect("leer mi-dibujo.png");

    // `SaveAs`: propone `mi-dibujo.png`, en el directorio de la ruta, y escribe lo elegido sin
    // tocar el primero.
    w.aplicar(&edicion_que_cambia(&w.m));
    assert!(dialogos.begin(
        DialogKind::SaveAs,
        false,
        w.s.path(),
        w.s.converted(),
        10.0,
        None
    ));
    let peticion = mando.ultima();
    assert_eq!(peticion.file_name.as_deref(), Some("mi-dibujo.png"));
    assert_eq!(peticion.dir.as_deref(), Some(t.ruta()));
    let otra = t.ruta().join("otra.png");
    mando.elegir(&otra);
    let terminado = dialogos.poll(11.0).expect("el diálogo terminó");
    let Pegado::Guardado(resultado) = pegamento(&mut w.s, terminado) else {
        panic!("SaveAs no guardó");
    };
    assert_eq!(resultado.expect("SaveAs guardó").path, otra);
    w.marcar_guardado();
    assert_eq!(w.s.path(), Some(otra.as_path()));
    assert_eq!(
        fs::read(&primero).expect("leer"),
        bytes_primero,
        "SaveAs tocó el primero"
    );
    assert_igual(&leer(&otra).image, &w.m.normalizada(), "otra.png");
    assert_eq!(
        t.nombres(),
        vec!["mi-dibujo.png".to_string(), "otra.png".to_string()]
    );

    // `Cancelled`: no escribe nada y deja `modified`.
    w.aplicar(&edicion_que_cambia(&w.m));
    let nombres = t.nombres();
    let arranques = mando.arranques();
    assert!(dialogos.begin(
        DialogKind::SaveAs,
        false,
        w.s.path(),
        w.s.converted(),
        20.0,
        None
    ));
    assert_eq!(mando.arranques(), arranques + 1);
    mando.cerrar();
    let terminado = dialogos.poll(30.0).expect("el diálogo terminó");
    assert_eq!(terminado.outcome, Outcome::Cancelled);
    assert!(matches!(pegamento(&mut w.s, terminado), Pegado::Cancelado));
    assert_eq!(t.nombres(), nombres, "Cancelled escribió algo");
    assert!(w.s.modified(), "Cancelled quitó modified");
    assert_eq!(w.s.path(), Some(otra.as_path()));
    w.comprobar();

    // `Exists`: elegir `otro` con `otro.png` ya existente no escribe nada.
    let previo = t.escribir("otro.png", b"no soy un png");
    let nombres = t.nombres();
    assert!(dialogos.begin(
        DialogKind::SaveAs,
        false,
        w.s.path(),
        w.s.converted(),
        40.0,
        None
    ));
    mando.elegir(&t.ruta().join("otro"));
    let terminado = dialogos.poll(41.0).expect("el diálogo terminó");
    let Pegado::Guardado(resultado) = pegamento(&mut w.s, terminado) else {
        panic!("el diálogo no llegó a guardar");
    };
    assert_eq!(resultado, Err(FileError::Exists(previo.clone())));
    assert_eq!(
        fs::read(&previo).expect("leer"),
        b"no soy un png",
        "Exists pisó otro.png"
    );
    assert_eq!(t.nombres(), nombres, "Exists escribió algo");
    assert!(w.s.modified(), "Exists quitó modified");
    assert_eq!(w.s.path(), Some(otra.as_path()), "Exists cambió la ruta");
    sin_temporales(&t.nombres());

    // `Open` de lo guardado, con la confirmación por repetición de `Ctrl+O` con cambios.
    let sin_descartar = w.s.open(&primero, Canvas::MAX_SIDE, false);
    assert_eq!(
        sin_descartar,
        Err(FileError::Unsaved),
        "open con cambios sin descartar"
    );
    let mut confirmacion = Confirm::new();
    assert_eq!(open_gate(w.s.modified(), &mut confirmacion, 100.0), None);
    let descartar = open_gate(w.s.modified(), &mut confirmacion, 101.0).expect("segunda pulsación");
    assert!(descartar);
    assert!(dialogos.begin(
        DialogKind::Open,
        descartar,
        w.s.path(),
        w.s.converted(),
        102.0,
        None
    ));
    assert_eq!(mando.ultima().file_name, None, "abrir no propone nombre");
    mando.elegir(&primero);
    let terminado = dialogos.poll(103.0).expect("el diálogo terminó");
    assert!(terminado.discard);
    let Pegado::Abierto(resultado) = pegamento(&mut w.s, terminado) else {
        panic!("Open no abrió");
    };
    resultado.expect("Open abrió");
    let abierta = w.s.document.image(ImageId::from_raw(0)).expect("imagen");
    // Lo guardado en `mi-dibujo.png` era el modelo de entonces (`snaps[1]`), normalizado.
    assert_igual(
        abierta,
        &w.snaps[1].1.normalizada(),
        "Open de mi-dibujo.png",
    );
    assert_eq!(w.s.path(), Some(primero.as_path()));
    assert!(!w.s.modified());
    assert_eq!(w.s.document.state_id(), 0);
}

// ---------------------------------------------------------------------------------------
// Fila 8: relectura.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_8_relectura_y_segundo_guardado_con_los_mismos_bytes() {
    for fuente in todas() {
        let t = Trabajo::nuevo();
        let mut w = abrir(&t, &fuente);
        let ctx = fuente.nombre.clone();
        for paso in guion_completo(&w.m) {
            w.aplicar(&paso);
        }
        let guardado = guardar(&mut w.s, t.ruta()).unwrap_or_else(|e| panic!("{ctx}: save: {e}"));
        let bytes_1 = fs::read(&guardado.path).expect("leer lo guardado");

        // Sin conversiones ni auxiliares: `write_png` no escribe nada que `read_png` anote.
        let leido = read_png(&bytes_1).unwrap_or_else(|e| panic!("{ctx}: read_png: {e}"));
        assert!(
            leido.report.is_empty(),
            "{ctx}: conversiones: {:?}",
            leido.report
        );
        assert_eq!(leido.aux, AuxReport::default(), "{ctx}: auxiliares");

        // Otra sesión abre lo guardado.
        let mut s2 = sesion_vacia();
        s2.open(&guardado.path, Canvas::MAX_SIDE, false)
            .unwrap_or_else(|e| panic!("{ctx}: reabrir: {e}"));
        let imagen = s2.document.image(ImageId::from_raw(0)).expect("imagen");
        assert_igual(imagen, &w.m.normalizada(), &format!("{ctx}: relectura"));
        assert!(!s2.converted(), "{ctx}: lo guardado se convierte al abrir");
        assert_eq!(s2.document.state_id(), 0, "{ctx}: state_id");
        assert!(!s2.modified(), "{ctx}: modified");

        // Y vuelve a guardar: `write_png` es determinista.
        let segundo = guardar(&mut s2, t.ruta()).unwrap_or_else(|e| panic!("{ctx}: save 2: {e}"));
        assert_eq!(
            segundo.path, guardado.path,
            "{ctx}: ruta del segundo guardado"
        );
        assert!(
            segundo.copy_reason.is_none(),
            "{ctx}: copy_reason del segundo guardado"
        );
        assert!(
            fs::read(&segundo.path).expect("leer el segundo") == bytes_1,
            "{ctx}: los bytes del segundo guardado no son los del primero"
        );
        sin_temporales(&t.nombres());
    }
}

// ---------------------------------------------------------------------------------------
// Fila 9: golden/png intacto y controles negativos.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_9_golden_png_intacto_al_terminar_la_puerta() {
    let antes = inicial();
    let mut nombres_esperados: Vec<String> = REALES.iter().map(|n| (*n).to_string()).collect();
    nombres_esperados.extend(REALES.iter().map(|n| format!("{n}.sha256")));
    nombres_esperados.push("README.md".to_string());
    nombres_esperados.sort();
    assert_eq!(
        antes.keys().cloned().collect::<Vec<_>>(),
        nombres_esperados,
        "golden/png/ no tiene los 9 ficheros del corpus"
    );

    // Los 9 ficheros con los mismos bytes y ninguno más.
    let ahora = instantanea_de(&golden_png());
    if let Err(problemas) = comparar_instantaneas(antes, &ahora) {
        panic!("golden/png/ cambió: {problemas}");
    }
    // El SHA-256 de cada PNG es el de la instantánea inicial.
    for nombre in REALES {
        assert_eq!(
            sha256_hex(&ahora[nombre]),
            sha256_hex(&antes[nombre]),
            "{nombre}: el SHA-256 cambió"
        );
    }
    golden_intacto();
}

#[test]
fn fila_9_sha256_propio_cumple_sus_vectores() {
    for (nombre, mensaje, esperado) in vectores() {
        assert_eq!(sha256_hex(&mensaje), esperado, "vector {nombre}");
    }
}

#[test]
#[should_panic(expected = "escritura prohibida en golden/png")]
fn fila_9_control_fuera_de_golden_hace_panico_con_un_fichero_de_golden() {
    fuera_de_golden(&golden_png().join("grafx2-golden1.png"));
}

#[test]
#[should_panic(expected = "escritura prohibida en golden/png")]
fn fila_9_control_fuera_de_golden_hace_panico_con_un_fichero_nuevo_de_golden() {
    fuera_de_golden(&golden_png().join("nuevo.png"));
}

#[test]
#[should_panic(expected = "escritura prohibida en golden/png")]
fn fila_9_control_fuera_de_golden_hace_panico_con_la_carpeta_misma() {
    fuera_de_golden(&golden_png());
}

#[test]
#[should_panic(expected = "escritura prohibida en golden/png")]
fn fila_9_control_fuera_de_golden_hace_panico_con_una_ruta_con_puntos_puntos() {
    let rodeo = raiz_repo()
        .join("crates")
        .join("..")
        .join("golden")
        .join("png")
        .join("..")
        .join("png")
        .join("x.png");
    fuera_de_golden(&rodeo);
}

#[test]
fn fila_9_fuera_de_golden_deja_pasar_el_trabajo_y_el_resto_del_repo() {
    let t = Trabajo::nuevo();
    fuera_de_golden(&t.ruta().join("x.png"));
    fuera_de_golden(&golden_png().parent().expect("golden").join("otra-cosa.png"));
    fuera_de_golden(&raiz_repo().join("target").join("x.png"));
}

#[test]
fn fila_9_el_comparador_de_instantaneas_sabe_fallar() {
    let base = inicial().clone();
    assert_eq!(comparar_instantaneas(&base, &base), Ok(()));
    let nombre = REALES[0];

    // Cambia un byte de un PNG.
    let mut cambiado = base.clone();
    cambiado.get_mut(nombre).expect("png")[10] ^= 1;
    let error = comparar_instantaneas(&base, &cambiado).expect_err("un byte distinto");
    assert!(error.contains(nombre), "{error}");

    // Cambia un byte del README.
    let mut cambiado = base.clone();
    cambiado.get_mut("README.md").expect("readme")[0] ^= 1;
    assert!(comparar_instantaneas(&base, &cambiado).is_err());

    // Aparece un fichero.
    let mut con_uno_mas = base.clone();
    con_uno_mas.insert("intruso.png".to_string(), vec![1, 2, 3]);
    let error = comparar_instantaneas(&base, &con_uno_mas).expect_err("fichero nuevo");
    assert!(error.contains("intruso.png"), "{error}");

    // Un PNG más corto, y un fichero que falta.
    let mut recortado = base.clone();
    recortado.get_mut(nombre).expect("png").pop();
    assert!(comparar_instantaneas(&base, &recortado).is_err());
    let mut sin_uno = base.clone();
    sin_uno.remove(nombre);
    let error = comparar_instantaneas(&base, &sin_uno).expect_err("fichero que falta");
    assert!(error.contains(nombre), "{error}");
}

// ---------------------------------------------------------------------------------------
// Fila 10: PNG que Pixoko convierte.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_10_los_png_que_pixoko_convierte_guardan_primero_una_copia() {
    let mut fuentes = convertidas();
    // Con los mismos criterios, los reales que se conviertan.
    fuentes.extend(reales().into_iter().filter(convierte));
    assert!(fuentes.len() >= 4);
    for fuente in fuentes {
        let t = Trabajo::nuevo();
        let ctx = fuente.nombre.clone();
        let mut w = abrir(&t, &fuente);
        assert!(w.s.converted(), "{ctx}: debía convertirse al abrir");
        let origen = w.s.path().expect("ruta").to_path_buf();
        let bytes_origen = fs::read(&origen).expect("leer la copia de origen");
        let copia = libre_copia(&origen);
        let nombres = {
            let mut v = vec![
                origen
                    .file_name()
                    .expect("nombre")
                    .to_string_lossy()
                    .into_owned(),
                copia
                    .file_name()
                    .expect("nombre")
                    .to_string_lossy()
                    .into_owned(),
            ];
            v.sort();
            v
        };

        // El primer `save` escribe `-copia` y deja intacta la copia de origen.
        let modelo_0 = w.m.clone();
        let primero = guardar(&mut w.s, t.ruta()).unwrap_or_else(|e| panic!("{ctx}: save 1: {e}"));
        assert_eq!(primero.path, copia, "{ctx}: destino del primer save");
        assert!(
            primero
                .copy_reason
                .as_deref()
                .is_some_and(|razon| !razon.is_empty()),
            "{ctx}: copy_reason debía ser Some"
        );
        assert_eq!(
            fs::read(&origen).expect("leer"),
            bytes_origen,
            "{ctx}: tocó el origen"
        );
        assert_eq!(t.nombres(), nombres, "{ctx}: ficheros tras el primer save");
        assert_igual(
            &leer(&copia).image,
            &modelo_0.normalizada(),
            &format!("{ctx}: relectura de la copia"),
        );
        w.marcar_guardado();
        assert!(!w.s.converted(), "{ctx}: sigue convertida tras guardar");
        assert_eq!(w.s.path(), Some(copia.as_path()));

        // El segundo `save` sobrescribe la copia.
        w.aplicar(&edicion_que_cambia(&w.m));
        let segundo = guardar(&mut w.s, t.ruta()).unwrap_or_else(|e| panic!("{ctx}: save 2: {e}"));
        assert_eq!(segundo.path, copia, "{ctx}: destino del segundo save");
        assert!(
            segundo.copy_reason.is_none(),
            "{ctx}: el segundo save dio copy_reason"
        );
        assert_eq!(t.nombres(), nombres, "{ctx}: ficheros tras el segundo save");
        assert_eq!(
            fs::read(&origen).expect("leer"),
            bytes_origen,
            "{ctx}: tocó el origen (2)"
        );
        // La relectura da la imagen del modelo.
        assert_igual(
            &leer(&copia).image,
            &w.m.normalizada(),
            &format!("{ctx}: relectura tras el segundo save"),
        );
        sin_temporales(&t.nombres());
    }
}

#[test]
fn fila_10_un_real_sin_conversion_se_sobrescribe_en_el_trabajo() {
    let mut vistos = 0;
    for fuente in reales().into_iter().filter(|f| !convierte(f)) {
        vistos += 1;
        let t = Trabajo::nuevo();
        let ctx = fuente.nombre.clone();
        let mut w = abrir(&t, &fuente);
        assert!(!w.s.converted(), "{ctx}: no debía convertirse");
        let origen = w.s.path().expect("ruta").to_path_buf();
        w.aplicar(&edicion_que_cambia(&w.m));
        let guardado = guardar(&mut w.s, t.ruta()).unwrap_or_else(|e| panic!("{ctx}: save: {e}"));
        assert_eq!(guardado.path, origen, "{ctx}: se sobrescribe la copia");
        assert!(guardado.copy_reason.is_none(), "{ctx}: copy_reason");
        assert_eq!(
            t.nombres(),
            vec![fuente.nombre.clone()],
            "{ctx}: ficheros del trabajo"
        );
        assert_igual(
            &leer(&origen).image,
            &w.m.normalizada(),
            &format!("{ctx}: relectura"),
        );
        // Y nunca en golden/png: lo comprueba la instantánea al soltar el trabajo.
        assert_ne!(
            origen.parent(),
            Some(golden_png().as_path()),
            "{ctx}: la copia está en golden/png"
        );
    }
    assert!(vistos >= 1, "ningún real sin conversión");
}
