//! Filas 5 a 9 y 11 de la puerta del hito 3 (specs/hito-3-puerta.md, «Casos»). La fila 10 está en
//! `grande.rs` (`#[ignore]`) y las propiedades 1 a 3 en `propiedades.rs`.

use std::fs;
use std::path::{Path, PathBuf};

use px_formats::{read_png, write_png};
use px_raster::compose;
use px_ui::canvas::Canvas;

use super::guiones::{
    guion_corto_indexado, guion_corto_rgba, guion_indexado, guion_rgba, sesion_indexada,
    sesion_rgba,
};
use super::modelo::Pila;
use super::mundo::Mundo3;
use super::{
    Trabajo3, comprobar_golden_aseprite, fuera_de_golden_aseprite, golden_aseprite,
    igual_salvo_rgb_con_alfa_0, inicial,
};
use crate::convert_mode::WAIT_FRAMES;
use crate::dialogs::{DialogKind, Outcome, SaveRoute, proposed_save_name};
use crate::puerta::comun::sha256::sha256_hex;
use crate::puerta::comun::{
    comparar_instantaneas, dialogos, nombres_en, sesion_vacia, sin_temporales,
};
use crate::save_wait::{StartSave, aseprite_pixels, save_pixels, start_save};
use crate::session::with_flatten_notice;

// ---------------------------------------------------------------------------------------
// Filas 5 y 6: la sesión de punta a punta.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_05_rgba_de_punta_a_punta() {
    let mut w = Mundo3::de_sesion(sesion_rgba(), "rgba");
    guion_rgba(&mut w);
    assert!(w.pasos() >= 35, "el guion hace {} pasos", w.pasos());
    assert!(w.s.modified());
}

#[test]
fn fila_06_indexado_de_punta_a_punta() {
    let mut w = Mundo3::de_sesion(sesion_indexada(), "indexado");
    guion_indexado(&mut w);
    assert!(w.pasos() >= 20, "el guion hace {} pasos", w.pasos());
    // La paleta es la misma en todas las celdas tras cada orden (la comprueba `igual` del modelo,
    // que compara la paleta del documento) y, además, la de cada celda.
    let paleta = w.s.document.palette().expect("paleta").clone();
    for capa in w.s.document.layers() {
        if let Some(imagen) = w.s.document.image(capa.cel()) {
            assert_eq!(imagen.palette(), Some(&paleta), "paleta de la celda");
        }
    }
}

// ---------------------------------------------------------------------------------------
// Fila 7: deshacer y rehacer todo.
// ---------------------------------------------------------------------------------------

fn historia(mut w: Mundo3) {
    let pasos = w.pasos();
    let ultimo = w.leer();
    assert_eq!(w.deshacer_todo(), pasos, "deshacer hasta el principio");
    assert_eq!(w.s.document.state_id(), 0, "al principio, state_id 0");
    assert!(!w.s.modified(), "al principio no hay cambios");
    assert_eq!(w.m.capas.len(), 1, "al principio, el documento original");
    assert_eq!(w.rehacer_todo(), pasos, "rehacer hasta el final");
    ultimo
        .igual_contenido(&w.leer())
        .expect("el final es el final");
    assert!(w.s.modified());

    // Guardado en medio, una orden nueva tras deshacer y el estado guardado deja de ser alcanzable.
    for _ in 0..5 {
        w.deshacer();
    }
    w.s.mark_unmodified();
    w.marcar_guardado();
    assert!(!w.s.modified());
    w.deshacer();
    assert!(w.s.modified(), "un paso antes de lo guardado");
    assert!(w.aplicar(&super::modelo::Orden::Nueva));
    assert!(w.s.modified());
    w.rehacer(); // al final de la rama: `NothingToRedo`, comprobado dentro.
    w.deshacer();
    assert!(
        w.s.modified(),
        "el estado guardado ya no es alcanzable: sigue modificado"
    );
    w.comprobar();
}

#[test]
fn fila_07_deshacer_y_rehacer_todo_en_rgba_y_en_indexado() {
    let mut rgba = Mundo3::de_sesion(sesion_rgba(), "historia rgba");
    guion_rgba(&mut rgba);
    historia(rgba);
    let mut indexado = Mundo3::de_sesion(sesion_indexada(), "historia indexado");
    guion_indexado(&mut indexado);
    historia(indexado);
}

// ---------------------------------------------------------------------------------------
// Fila 8: guardar y reabrir.
// ---------------------------------------------------------------------------------------

fn guardar_y_reabrir(indexado: bool) {
    let t = Trabajo3::nuevo();
    let etiqueta = if indexado {
        "guardar indexado"
    } else {
        "guardar rgba"
    };
    let mut w = Mundo3::de_sesion(
        if indexado {
            sesion_indexada()
        } else {
            sesion_rgba()
        },
        etiqueta,
    );
    if indexado {
        guion_corto_indexado(&mut w);
    } else {
        guion_corto_rgba(&mut w);
    }
    let capas = w.m.capas.len();
    assert!(capas >= 3);

    // `Ctrl+S` sin ruta con 3 o más capas: diálogo que propone `sin-titulo.aseprite`.
    assert_eq!(w.s.save_route(), SaveRoute::Dialog);
    let (mut dialogos, mando) = dialogos(t.ruta());
    let propuesto = PathBuf::from(proposed_save_name(w.s.path(), capas));
    assert!(dialogos.begin(
        DialogKind::Save,
        false,
        Some(&propuesto),
        w.s.converted(),
        0.0,
        None
    ));
    assert_eq!(
        mando.ultima().file_name.as_deref(),
        Some("sin-titulo.aseprite")
    );
    let destino = t.ruta().join("prueba.aseprite");
    mando.elegir(&destino);
    let terminado = dialogos.poll(1.0).expect("el diálogo terminó");
    let Outcome::Chosen(elegida) = terminado.outcome else {
        panic!("el diálogo no eligió ninguna ruta");
    };
    fuera_de_golden_aseprite(&elegida);
    let guardado = w.s.save_to(&elegida).expect("guardar el .aseprite");
    assert_eq!(guardado.path, destino);
    assert!(guardado.copy_reason.is_none());
    w.marcar_guardado();
    assert_eq!(
        w.s.path(),
        Some(destino.as_path()),
        "la sesión pasa a ese fichero"
    );
    assert_eq!(w.s.save_route(), SaveRoute::Direct);
    assert_eq!(t.nombres(), vec!["prueba.aseprite".to_string()]);

    // Reabrir: la capa activa es la de arriba y el documento es el de antes, sin ids.
    let mut otra = sesion_vacia();
    let aviso = otra
        .open(&destino, Canvas::MAX_SIDE, true)
        .expect("reabrir el .aseprite");
    assert!(aviso.contains("prueba.aseprite"), "{aviso}");
    assert!(!otra.converted() && !otra.modified());
    assert_eq!(otra.document.state_id(), 0);
    let ultima = otra.document.layers().last().expect("capas").id();
    assert_eq!(
        otra.active_layer(),
        ultima,
        "la capa activa es la de arriba"
    );
    let leida = Pila::de_documento(&otra.document, capas - 1);
    w.m.canonica()
        .igual_contenido(&leida.canonica())
        .expect("el reabierto es el documento guardado");
    let antes = compose(&w.s.document).expect("compose");
    let despues = compose(&otra.document).expect("compose");
    igual_salvo_rgb_con_alfa_0(&antes, &despues).expect("la composición sobrevive");
    if !indexado {
        // Vector a mano de 3.2: Multiplicar de `ORIGEN` sobre `FONDO`.
        assert_eq!(
            super::rgba_de(&despues, 0, 0),
            px_core::Rgba::new(69, 11, 45, 255)
        );
    }

    // Exportar el PNG aplanado: ni ruta, ni historia ni «modificado» cambian.
    let (id, ruta_doc) = (w.s.document.state_id(), w.s.path().map(Path::to_path_buf));
    let plano = t.ruta().join("plano");
    fuera_de_golden_aseprite(&plano);
    let exportado = w.s.export_flat(&plano).expect("exportar");
    assert_eq!(exportado.path, t.ruta().join("plano.png"));
    assert_eq!(w.s.document.state_id(), id);
    assert_eq!(w.s.path(), ruta_doc.as_deref());
    assert!(!w.s.modified());
    let png = read_png(&fs::read(&exportado.path).expect("leer"))
        .expect("PNG")
        .image;
    igual_salvo_rgb_con_alfa_0(&png, &antes).expect("el PNG exportado es la composición");
    if !indexado {
        assert_eq!(
            super::rgba_de(&png, 0, 0),
            px_core::Rgba::new(69, 11, 45, 255)
        );
    }

    // Guardar como PNG: aplanado, con su aviso, y la sesión pasa a ese PNG.
    let aviso = with_flatten_notice("guardado".to_string(), &w.s.document);
    assert!(aviso.contains("se guarda aplanado"), "{aviso}");
    let otro = t.ruta().join("aplanado.png");
    fuera_de_golden_aseprite(&otro);
    let guardado = w.s.save_to(&otro).expect("guardar el PNG aplanado");
    assert_eq!(guardado.path, otro);
    w.marcar_guardado();
    let png = read_png(&fs::read(&otro).expect("leer"))
        .expect("PNG")
        .image;
    igual_salvo_rgb_con_alfa_0(&png, &antes).expect("el PNG guardado es la composición");
    // Con 3 o más capas y ruta PNG, `Ctrl+S` pide el diálogo (y propone `.aseprite`).
    assert_eq!(w.s.save_route(), SaveRoute::Dialog);
    assert_eq!(proposed_save_name(w.s.path(), capas), "aplanado.aseprite");

    let nombres = t.nombres();
    sin_temporales(&nombres);
    assert_eq!(
        nombres,
        vec![
            "aplanado.png".to_string(),
            "plano.png".to_string(),
            "prueba.aseprite".to_string()
        ]
    );
}

#[test]
fn fila_08_guardar_y_reabrir_en_rgba() {
    guardar_y_reabrir(false);
}

#[test]
fn fila_08_guardar_y_reabrir_en_indexado() {
    guardar_y_reabrir(true);
}

#[test]
fn fila_08_un_png_con_dos_capas_pide_el_dialogo_y_propone_aseprite() {
    let t = Trabajo3::nuevo();
    let base = t.ruta().join("base.png");
    let imagen =
        px_core::Image::filled(4, 4, px_core::Pixel::Rgba(px_core::Rgba::new(1, 2, 3, 255)))
            .expect("imagen");
    fs::write(&base, write_png(&imagen).expect("PNG")).expect("escribir base.png");
    let mut sesion = sesion_vacia();
    sesion.open(&base, Canvas::MAX_SIDE, true).expect("abrir");
    assert_eq!(
        sesion.save_route(),
        SaveRoute::Direct,
        "una capa: guarda sin diálogo"
    );
    let mut w = Mundo3::de_sesion(sesion, "png con capas");
    assert!(w.aplicar(&super::modelo::Orden::Nueva));
    assert_eq!(w.s.save_route(), SaveRoute::Dialog);
    assert_eq!(proposed_save_name(w.s.path(), 2), "base.aseprite");
    // Elegir `.png` en ese diálogo guarda aplanado, como siempre.
    let (mut dialogos, mando) = dialogos(t.ruta());
    let propuesto = PathBuf::from("base.aseprite");
    assert!(dialogos.begin(DialogKind::Save, false, Some(&propuesto), false, 0.0, None));
    mando.elegir(&t.ruta().join("elegido.png"));
    let Some(Outcome::Chosen(ruta)) = dialogos.poll(1.0).map(|f| f.outcome) else {
        panic!("el diálogo no eligió");
    };
    let guardado = w.s.save_to(&ruta).expect("guardar");
    assert_eq!(guardado.path, t.ruta().join("elegido.png"));
}

// ---------------------------------------------------------------------------------------
// Fila 9: los 29 por la sesión.
// ---------------------------------------------------------------------------------------

fn casos_del_corpus() -> Vec<String> {
    let mut casos: Vec<String> = nombres_en(&golden_aseprite())
        .into_iter()
        .filter_map(|n| n.strip_suffix(".aseprite").map(str::to_string))
        .collect();
    casos.sort();
    casos
}

#[test]
fn fila_09_los_29_por_la_sesion_se_abren_se_guardan_y_se_reabren() {
    let t = Trabajo3::nuevo();
    let casos = casos_del_corpus();
    assert_eq!(casos.len(), 29, "golden/aseprite tiene los 29 casos");
    for caso in &casos {
        let nombre = format!("{caso}.aseprite");
        let copia = t.copia_del_corpus(caso, &nombre);
        let mut s = sesion_vacia();
        let aviso = s
            .open(&copia, Canvas::MAX_SIDE, true)
            .unwrap_or_else(|e| panic!("{caso}: abrir dio {e}"));
        assert!(aviso.contains(&nombre), "{caso}: {aviso}");
        assert!(
            aviso.contains(&format!("{}×{}", s.document.width(), s.document.height())),
            "{caso}: {aviso}"
        );
        assert!(!s.converted(), "{caso}: sin avisos de lo ignorado");
        assert_eq!(s.document.state_id(), 0, "{caso}");
        assert!(!s.modified(), "{caso}");
        let capas = s.document.layers().len();
        let ultima = s.document.layers().last().expect("capas").id();
        assert_eq!(
            s.active_layer(),
            ultima,
            "{caso}: la activa es la de arriba"
        );
        let original = Pila::de_documento(&s.document, capas - 1);

        fuera_de_golden_aseprite(&copia);
        let guardado = s
            .save(t.ruta())
            .unwrap_or_else(|e| panic!("{caso}: guardar dio {e}"));
        assert_eq!(guardado.path, copia, "{caso}: sobrescribe la copia");
        assert!(guardado.copy_reason.is_none(), "{caso}");

        let mut otra = sesion_vacia();
        otra.open(&copia, Canvas::MAX_SIDE, true)
            .unwrap_or_else(|e| panic!("{caso}: reabrir dio {e}"));
        let leida = Pila::de_documento(&otra.document, capas - 1);
        original
            .canonica()
            .igual_contenido(&leida.canonica())
            .unwrap_or_else(|e| panic!("{caso}: {e}"));
    }
    let nombres = t.nombres();
    sin_temporales(&nombres);
    assert_eq!(nombres.len(), 29, "una copia por caso y nada más");
}

// ---------------------------------------------------------------------------------------
// Fila 10 (condición 6): «Guardando…» al guardar un `.aseprite` grande.
// ---------------------------------------------------------------------------------------

/// Un documento de `lado` × `lado` con una capa por elemento de `visibles` (`Some` sin imagen no
/// existe: las vacías son `None`).
fn documento_de_celdas(lado: u32, capas: &[(bool, bool)]) -> px_core::Document {
    use px_core::{BlendMode, Image, LayerSpec, Mode, Pixel, Rgba};
    let specs = capas
        .iter()
        .enumerate()
        .map(|(i, &(visible, con_imagen))| LayerSpec {
            name: format!("capa {i}"),
            visible,
            opacity: 255,
            blend: BlendMode::Normal,
            locked: false,
            image: con_imagen.then(|| {
                Image::filled(lado, lado, Pixel::Rgba(Rgba::new(i as u8, 2, 3, 255)))
                    .expect("imagen")
            }),
        })
        .collect();
    px_core::Document::from_layers(lado, lado, Mode::Rgba, None, specs).expect("documento")
}

#[test]
fn fila_10_un_aseprite_grande_pide_guardando_y_espera_los_fotogramas_de_siempre() {
    // 4 capas con imagen de 2048² son exactamente 2²⁴ píxeles de celda: se guarda ya; con 5, no.
    let cuatro = documento_de_celdas(2048, &[(true, true); 4]);
    assert_eq!(aseprite_pixels(&cuatro), 1 << 24);
    assert_eq!(start_save(save_pixels(&cuatro, true)), StartSave::Now);
    let cinco = documento_de_celdas(2048, &[(true, true); 5]);
    assert_eq!(aseprite_pixels(&cinco), 5 * 2048 * 2048);
    let StartSave::Later(mut espera) = start_save(save_pixels(&cinco, true)) else {
        panic!("5 capas de 2048² pasan de 2²⁴ píxeles: debía esperar");
    };
    // La misma secuencia que «Guardando…» de un PNG: ni un fotograma antes ni después.
    for fotograma in 1..WAIT_FRAMES {
        assert!(!espera.tick(), "el fotograma {fotograma} aún no guarda");
    }
    assert!(
        espera.tick(),
        "guarda exactamente en el fotograma {WAIT_FRAMES}"
    );
    assert!(!espera.tick(), "y no vuelve a guardar");
    // Las celdas vacías no se escriben y no cuentan; las ocultas sí (el `.aseprite` las guarda),
    // y un PNG solo compone las visibles.
    let mezcla = documento_de_celdas(
        2048,
        &[
            (true, true),
            (false, true),
            (false, true),
            (true, false),
            (true, true),
        ],
    );
    assert_eq!(aseprite_pixels(&mezcla), 4 * 2048 * 2048);
    assert_eq!(save_pixels(&mezcla, true), 4 * 2048 * 2048);
    assert_eq!(save_pixels(&mezcla, false), 2 * 2048 * 2048);
}

// ---------------------------------------------------------------------------------------
// Fila 11: `golden/aseprite/` intacto y los controles negativos.
// ---------------------------------------------------------------------------------------

#[test]
fn fila_11_golden_aseprite_intacto_y_el_control_sabe_fallar() {
    let _t = Trabajo3::nuevo();
    let (instantanea, huellas) = inicial();
    assert_eq!(
        instantanea.len(),
        88,
        "29 casos de tres ficheros y el README"
    );
    assert_eq!(huellas.len(), 88);
    comprobar_golden_aseprite().expect("golden/aseprite es el de antes");
    // Control: un byte cambiado, un fichero más y uno menos dan `Err`, y el SHA-256 lo ve.
    let mut roto = instantanea.clone();
    roto.get_mut("mixto-rgba.png").expect("fichero")[40] ^= 1;
    let error = comparar_instantaneas(instantanea, &roto).expect_err("un byte cambiado");
    assert!(error.contains("mixto-rgba.png"), "{error}");
    assert_ne!(
        sha256_hex(&roto["mixto-rgba.png"]),
        huellas["mixto-rgba.png"]
    );
    let mut con = instantanea.clone();
    con.insert("intruso.aseprite".to_string(), vec![1, 2, 3]);
    let error = comparar_instantaneas(instantanea, &con).expect_err("un fichero más");
    assert!(error.contains("aparece intruso.aseprite"), "{error}");
    let mut sin = instantanea.clone();
    sin.remove("capa-bloqueada.aseprite");
    let error = comparar_instantaneas(instantanea, &sin).expect_err("un fichero menos");
    assert!(error.contains("falta capa-bloqueada.aseprite"), "{error}");
}

#[test]
#[should_panic(expected = "escritura prohibida en golden/aseprite")]
fn fila_11_una_ruta_de_golden_aseprite_hace_panico() {
    fuera_de_golden_aseprite(&golden_aseprite().join("nuevo.aseprite"));
}

#[test]
fn fila_11_el_trabajo_cuelga_de_target_y_se_borra_aunque_la_prueba_falle() {
    use std::sync::Mutex;
    let ruta = Mutex::new(None::<PathBuf>);
    let t = Trabajo3::nuevo();
    let propia = t.ruta().to_path_buf();
    assert!(
        propia.components().any(|c| c.as_os_str() == "pixoko-gate"),
        "{}",
        propia.display()
    );
    assert!(propia.starts_with(
        fs::canonicalize(crate::puerta::comun::raiz_repo().join("target")).expect("target")
    ));
    drop(t);
    assert!(!propia.exists(), "el trabajo se borra al soltarse");
    let fallo = std::panic::catch_unwind(|| {
        let t = Trabajo3::nuevo();
        *ruta.lock().unwrap_or_else(|e| e.into_inner()) = Some(t.ruta().to_path_buf());
        fs::write(t.ruta().join("x"), b"x").expect("escribir");
        panic!("la prueba falla a propósito");
    });
    assert!(fallo.is_err());
    let propia = ruta
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
        .expect("ruta");
    assert!(!propia.exists(), "también se borra si la prueba falla");
}
