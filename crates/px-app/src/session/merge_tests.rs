//! Combinar, aplanar y exportar en la sesión (specs/hito-3-combinar.md, unidad 3.5: filas 9, 10
//! y 11, decisiones D2 a D4, D9 y condición (c)). Sin ventana: `Session`, `dispatch` y los
//! ayudantes puros de `merge_flow`. En ROJO hasta la lógica.
//!
//! Contrato de API que estas pruebas fijan (la lógica debe ofrecerlo):
//! - `crate::merge_flow`: `MergeKind::{Down, Visible, Flatten}`, `merge_pixels`, `wait_notice`,
//!   `start_merge`, `StartMerge::{Now, Later}`, `PendingMerge::{kind, tick}`, `flatten_gate`,
//!   `MERGE_SYNC_PIXELS`.
//! - `Session`: `merge(kind) -> LayerReport`, `merge_refusal(kind) -> Option<&'static str>`,
//!   `flatten_hidden_with_pixels() -> usize`, `export_flat(&Path) -> Result<Saved, FileError>`,
//!   `describe_exported(&self, &Saved) -> String`.
//! - `Dispatched::Merge(MergeKind)` y `Dispatched::ExportFlat` para las cuatro `AppAction` nuevas.
//! - `Request::FlattenHidden` en `crate::confirm`.

use super::*;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use px_core::{BlendMode, ImageId, Layer, LayerId, PixelEdit, Rgba};
use px_formats::write_png;
use px_ui::actions::{AppAction, Locks, Origin};

use crate::confirm::{Confirm, Request};
use crate::convert_mode::WAIT_FRAMES;
use crate::dispatch::{Dispatched, Targets, dispatch};
use crate::merge_flow::{
    MERGE_SYNC_PIXELS, MergeKind, StartMerge, flatten_gate, merge_pixels, start_merge, wait_notice,
};

// ---------------------------------------------------------------------------------------------
// Ayudantes
// ---------------------------------------------------------------------------------------------

static CONTADOR: AtomicU64 = AtomicU64::new(0);

struct Tmp {
    ruta: PathBuf,
}

impl Tmp {
    fn nuevo() -> Tmp {
        let n = CONTADOR.fetch_add(1, Ordering::SeqCst);
        let ruta =
            std::env::temp_dir().join(format!("pixoko-px-app-combinar-{}-{n}", std::process::id()));
        fs::create_dir_all(&ruta).expect("directorio temporal");
        Tmp { ruta }
    }

    fn ruta(&self) -> &Path {
        &self.ruta
    }
}

impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.ruta);
    }
}

const FONDO: Rgba = Rgba::opaque(10, 20, 30);
const VERDE: Rgba = Rgba::opaque(0, 255, 0);
const AZUL: Rgba = Rgba::opaque(0, 0, 255);
const ROJO: Rgba = Rgba::opaque(255, 0, 0);

fn rgba_lleno(ancho: u32, alto: u32, color: Rgba) -> Image {
    Image::filled(ancho, alto, Pixel::Rgba(color)).expect("imagen")
}

/// Un documento de 4 × 4 con las capas `nombres` de abajo arriba: la de abajo llena de `FONDO` y
/// las demás vacías.
fn sesion_con(nombres: &[&str]) -> Session {
    let mut sesion = Session::sample(Document::new(rgba_lleno(4, 4, FONDO)));
    for (i, nombre) in nombres.iter().enumerate().skip(1) {
        sesion
            .apply_layer(Command::AddLayer {
                name: (*nombre).to_string(),
                at: i,
            })
            .expect("añadir capa");
    }
    sesion
}

fn id(sesion: &Session, posicion: usize) -> LayerId {
    sesion.document.layers()[posicion].id()
}

fn celda(sesion: &Session, posicion: usize) -> ImageId {
    sesion.document.layers()[posicion].cel()
}

fn pinta(sesion: &mut Session, posicion: usize, puntos: &[(u32, u32)], color: Rgba) {
    let image = celda(sesion, posicion);
    sesion
        .document
        .apply(Command::SetPixels {
            image,
            pixels: puntos
                .iter()
                .map(|&(x, y)| PixelEdit {
                    x,
                    y,
                    pixel: Pixel::Rgba(color),
                })
                .collect(),
        })
        .expect("pintar");
}

fn ejecuta(sesion: &mut Session, accion: AppAction) -> Dispatched {
    let mut tools = px_ui::tool::ToolState::new();
    dispatch(
        accion,
        Origin::Menu,
        &mut Targets {
            session: sesion,
            tools: &mut tools,
            locks: Locks::default(),
        },
    )
}

// ---------------------------------------------------------------------------------------------
// Fila 9: combinar abajo
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_9_combinar_abajo_une_la_activa_con_la_de_abajo_y_la_activa_vuelve_al_deshacer() {
    let mut sesion = sesion_con(&["A", "B", "C"]);
    pinta(&mut sesion, 1, &[(0, 0)], VERDE);
    pinta(&mut sesion, 2, &[(1, 0)], AZUL);
    sesion
        .apply_layer(Command::SetLayerOpacity {
            layer: id(&sesion, 1),
            opacity: 128,
        })
        .expect("opacidad");
    let (b, c) = (id(&sesion, 1), id(&sesion, 2));
    sesion.set_active_layer(c).expect("activa");
    let antes = sesion.document.clone();

    let informe = sesion.merge(MergeKind::Down);
    assert!(informe.changed, "{informe:?}");
    assert!(
        informe
            .notice
            .as_deref()
            .is_some_and(|t| t.contains("combinada")),
        "{informe:?}"
    );
    let capas = sesion.document.layers();
    assert_eq!(capas.len(), 2);
    assert_eq!(capas[1].id(), b, "queda la de abajo, con su id");
    assert_eq!(
        (capas[1].opacity(), capas[1].blend(), capas[1].locked()),
        (255, BlendMode::Normal, false)
    );
    assert_eq!(sesion.active_layer(), b, "la activa es la que queda");
    assert!(sesion.modified());

    // Deshacer: vuelve C con sus propiedades y la activa vuelve a C (nota de la combinación).
    sesion.undo().expect("deshacer");
    assert_eq!(sesion.document, antes);
    assert_eq!(sesion.active_layer(), c);
    // Rehacer: la activa es la superviviente.
    sesion.redo().expect("rehacer");
    assert_eq!(sesion.document.layers().len(), 2);
    assert_eq!(sesion.active_layer(), b);
}

#[test]
fn fila_9_combinar_abajo_rechaza_con_su_aviso_y_sin_tocar_nada() {
    // (cómo dejar el documento, qué trozo debe decir el aviso)
    type Preparar = fn(&mut Session);
    let casos: [(&str, Preparar, &str); 4] = [
        (
            "activa oculta",
            |s| {
                let u = id(s, 2);
                s.apply_layer(Command::SetLayerVisible {
                    layer: u,
                    visible: false,
                })
                .expect("ocultar");
            },
            "la capa activa está oculta",
        ),
        (
            "la de abajo oculta",
            |s| {
                let l = id(s, 1);
                s.apply_layer(Command::SetLayerVisible {
                    layer: l,
                    visible: false,
                })
                .expect("ocultar");
            },
            "la capa de abajo está oculta",
        ),
        (
            "la de abajo bloqueada",
            |s| {
                let l = id(s, 1);
                s.apply_layer(Command::SetLayerLocked {
                    layer: l,
                    locked: true,
                })
                .expect("bloquear");
            },
            "la capa de abajo está bloqueada",
        ),
        (
            "la activa es la de abajo",
            |s| {
                let a = id(s, 0);
                s.set_active_layer(a).expect("activa");
            },
            "no hay capa debajo",
        ),
    ];
    for (nombre, preparar, aviso) in casos {
        let mut sesion = sesion_con(&["A", "B", "C"]);
        pinta(&mut sesion, 1, &[(0, 0)], VERDE);
        pinta(&mut sesion, 2, &[(1, 0)], AZUL);
        let c = id(&sesion, 2);
        sesion.set_active_layer(c).expect("activa");
        preparar(&mut sesion);
        let (copia, estado) = (sesion.document.clone(), sesion.document.state_id());
        let motivo = sesion.merge_refusal(MergeKind::Down);
        assert!(
            motivo.is_some_and(|m| m.contains(aviso)),
            "{nombre}: {motivo:?}"
        );
        let informe = sesion.merge(MergeKind::Down);
        assert!(!informe.changed, "{nombre}");
        assert!(
            informe.notice.as_deref().is_some_and(|t| t.contains(aviso)),
            "{nombre}: {informe:?}"
        );
        assert_eq!(sesion.document, copia, "{nombre}");
        assert_eq!(sesion.document.state_id(), estado, "{nombre}");
    }
}

#[test]
fn fila_9_combinar_abajo_con_la_activa_bloqueada_vale() {
    // El candado de la que se consume no protege de quitarla (decisión 9 de 3.4).
    let mut sesion = sesion_con(&["A", "B"]);
    pinta(&mut sesion, 1, &[(2, 2)], VERDE);
    let b = id(&sesion, 1);
    sesion
        .apply_layer(Command::SetLayerLocked {
            layer: b,
            locked: true,
        })
        .expect("bloquear");
    sesion.set_active_layer(b).expect("activa");
    assert_eq!(sesion.merge_refusal(MergeKind::Down), None);
    assert!(sesion.merge(MergeKind::Down).changed);
    assert_eq!(sesion.document.layers().len(), 1);
    assert!(!sesion.document.layers()[0].locked());
}

// ---------------------------------------------------------------------------------------------
// Fila 9: combinar visibles y aplanar
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_9_combinar_visibles_deja_las_ocultas_y_necesita_dos_visibles() {
    let mut sesion = sesion_con(&["A", "B", "C", "D"]);
    pinta(&mut sesion, 1, &[(0, 0)], VERDE);
    pinta(&mut sesion, 2, &[(1, 0)], AZUL);
    pinta(&mut sesion, 3, &[(2, 0)], ROJO);
    let (a, b, d) = (id(&sesion, 0), id(&sesion, 1), id(&sesion, 3));
    sesion
        .apply_layer(Command::SetLayerVisible {
            layer: b,
            visible: false,
        })
        .expect("ocultar");
    sesion
        .apply_layer(Command::SetLayerLocked {
            layer: d,
            locked: true,
        })
        .expect("bloquear");
    sesion.set_active_layer(d).expect("activa");
    let antes = sesion.document.clone();

    assert_eq!(sesion.merge_refusal(MergeKind::Visible), None);
    let informe = sesion.merge(MergeKind::Visible);
    assert!(informe.changed, "{informe:?}");
    let ids: Vec<LayerId> = sesion.document.layers().iter().map(Layer::id).collect();
    assert_eq!(ids, [a, b], "se quedan la visible más baja y la oculta");
    assert!(
        !sesion.document.layers()[0].locked(),
        "el resultado queda desbloqueado"
    );
    assert_eq!(sesion.active_layer(), a);
    sesion.undo().expect("deshacer");
    assert_eq!(sesion.document, antes);
    assert_eq!(sesion.active_layer(), d);

    // Con una sola visible no hay nada que combinar.
    let mut una = sesion_con(&["A", "B"]);
    let b = id(&una, 1);
    una.apply_layer(Command::SetLayerVisible {
        layer: b,
        visible: false,
    })
    .expect("ocultar");
    let (copia, estado) = (una.document.clone(), una.document.state_id());
    assert!(
        una.merge_refusal(MergeKind::Visible)
            .is_some_and(|m| m.contains("hace falta más de una capa visible"))
    );
    assert!(!una.merge(MergeKind::Visible).changed);
    assert_eq!(
        (una.document.clone(), una.document.state_id()),
        (copia, estado)
    );
}

#[test]
fn fila_9_aplanar_quita_las_ocultas_pide_confirmacion_y_se_deshace_entero() {
    let mut sesion = sesion_con(&["A", "B", "C"]);
    pinta(&mut sesion, 1, &[(0, 0)], VERDE);
    pinta(&mut sesion, 2, &[(1, 0)], AZUL);
    let (b, c) = (id(&sesion, 1), id(&sesion, 2));
    sesion
        .apply_layer(Command::SetLayerVisible {
            layer: b,
            visible: false,
        })
        .expect("ocultar");
    sesion.set_active_layer(c).expect("activa");
    let antes = sesion.document.clone();
    assert_eq!(sesion.flatten_hidden_with_pixels(), 1);

    let informe = sesion.merge(MergeKind::Flatten);
    assert!(informe.changed, "{informe:?}");
    assert!(
        informe
            .notice
            .as_deref()
            .is_some_and(|t| t.contains("aplanadas")),
        "{informe:?}"
    );
    assert_eq!(sesion.document.layers().len(), 1);
    assert_eq!(sesion.active_layer(), id(&sesion, 0));
    sesion.undo().expect("deshacer");
    assert_eq!(sesion.document, antes, "vuelven las tres con sus ids");
    assert_eq!(sesion.active_layer(), c);
    sesion.redo().expect("rehacer");
    assert_eq!(sesion.document.layers().len(), 1);

    // Una oculta sin píxeles no pide confirmación.
    let mut vacia = sesion_con(&["A", "B", "C"]);
    let b = id(&vacia, 1);
    vacia
        .apply_layer(Command::SetLayerVisible {
            layer: b,
            visible: false,
        })
        .expect("ocultar");
    assert_eq!(vacia.flatten_hidden_with_pixels(), 0);
}

#[test]
fn fila_9_aplanar_sin_capas_visibles_o_con_una_sola_capa_se_rechaza() {
    let mut sesion = sesion_con(&["A", "B"]);
    for posicion in 0..2 {
        let capa = id(&sesion, posicion);
        sesion
            .apply_layer(Command::SetLayerVisible {
                layer: capa,
                visible: false,
            })
            .expect("ocultar");
    }
    assert!(
        sesion
            .merge_refusal(MergeKind::Flatten)
            .is_some_and(|m| m.contains("no hay capas visibles que aplanar"))
    );
    let (copia, estado) = (sesion.document.clone(), sesion.document.state_id());
    assert!(!sesion.merge(MergeKind::Flatten).changed);
    assert_eq!(
        (sesion.document.clone(), sesion.document.state_id()),
        (copia, estado)
    );

    let mut una = sesion_con(&["A"]);
    assert!(una.merge_refusal(MergeKind::Flatten).is_some());
    assert!(!una.merge(MergeKind::Flatten).changed);
}

#[test]
fn fila_9_el_aviso_de_historia_sale_solo_si_lo_guardado_supera_el_limite() {
    for (limite, esperado) in [(16usize, true), (64 << 20, false)] {
        let mut sesion = sesion_con(&["A", "B"]);
        pinta(&mut sesion, 1, &[(0, 0)], VERDE);
        sesion.document.set_history_limit(limite);
        let b = id(&sesion, 1);
        sesion.set_active_layer(b).expect("activa");
        let informe = sesion.merge(MergeKind::Down);
        assert!(informe.changed);
        let dice = informe
            .notice
            .as_deref()
            .is_some_and(|t| t.contains("siguen en la historia hasta tu próxima edición"));
        assert_eq!(dice, esperado, "límite {limite}: {informe:?}");
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 9: la confirmación de Aplanar y la espera de «Combinando…» y «Aplanando…» (condición (c))
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_9_aplanar_con_ocultas_con_pixeles_se_confirma_repitiendo_en_5_segundos() {
    // Sin ocultas con píxeles se ejecuta a la primera.
    let mut confirm = Confirm::new();
    assert!(flatten_gate(0, &mut confirm, 0.0));
    // Con ellas: la primera arma, a los 4 s ejecuta, a los 6 s vuelve a avisar.
    let mut confirm = Confirm::new();
    assert!(!flatten_gate(2, &mut confirm, 0.0));
    assert!(flatten_gate(2, &mut confirm, 4.0));
    assert!(!flatten_gate(2, &mut confirm, 10.0));
    assert!(!flatten_gate(2, &mut confirm, 16.0), "a los 6 s ya caducó");
    // Tras el aviso queda armada la petición propia de Aplanar.
    assert!(confirm.confirm_armed(&Request::FlattenHidden, 17.0));
}

#[test]
fn fila_9_combinar_y_aplanar_muestran_su_aviso_antes_de_empezar_si_pasan_de_2_a_la_24() {
    assert_eq!(MERGE_SYNC_PIXELS, 1 << 24);
    assert_eq!(wait_notice(MergeKind::Down), "Combinando…");
    assert_eq!(wait_notice(MergeKind::Visible), "Combinando…");
    assert_eq!(wait_notice(MergeKind::Flatten), "Aplanando…");
    for tipo in [MergeKind::Down, MergeKind::Visible, MergeKind::Flatten] {
        // Hasta 2²⁴: en el acto.
        assert_eq!(start_merge(tipo, MERGE_SYNC_PIXELS), StartMerge::Now);
        // Más: espera `WAIT_FRAMES` fotogramas, ejecuta exactamente en el último y nunca después.
        let StartMerge::Later(mut espera) = start_merge(tipo, MERGE_SYNC_PIXELS + 1) else {
            panic!("{tipo:?}: debía esperar");
        };
        assert_eq!(espera.kind(), tipo);
        for fotograma in 1..=WAIT_FRAMES {
            assert_eq!(
                espera.tick(),
                fotograma == WAIT_FRAMES,
                "{tipo:?} {fotograma}"
            );
        }
        assert!(!espera.tick());
        assert!(!espera.tick());
    }
}

#[test]
fn fila_9_los_pixeles_a_componer_son_ancho_por_alto_por_capas_visibles_con_imagen() {
    let mut sesion = sesion_con(&["A", "B", "C", "D"]);
    pinta(&mut sesion, 1, &[(0, 0)], VERDE);
    pinta(&mut sesion, 2, &[(1, 0)], AZUL);
    // D queda vacía; C se oculta: componen A y B.
    let c = id(&sesion, 2);
    sesion
        .apply_layer(Command::SetLayerVisible {
            layer: c,
            visible: false,
        })
        .expect("ocultar");
    let d = id(&sesion, 3);
    sesion.set_active_layer(d).expect("activa");
    assert_eq!(
        merge_pixels(&sesion.document, MergeKind::Visible, d),
        4 * 4 * 2
    );
    assert_eq!(
        merge_pixels(&sesion.document, MergeKind::Flatten, d),
        4 * 4 * 2
    );
    // Combinar abajo: la activa (vacía) y la de abajo (oculta, con imagen): una sola con imagen
    // entra en la cuenta si es visible; aquí C está oculta, así que 0 con imagen visible.
    assert_eq!(merge_pixels(&sesion.document, MergeKind::Down, d), 0);
}

// ---------------------------------------------------------------------------------------------
// Fila 10 (parte de px-app): las cuatro acciones llegan a la ventana
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_10_dispatch_pasa_las_cuatro_acciones_a_la_ventana_y_respeta_los_bloqueos() {
    let mut sesion = sesion_con(&["A", "B"]);
    for (accion, esperado) in [
        (
            AppAction::MergeLayerDown,
            Dispatched::Merge(MergeKind::Down),
        ),
        (
            AppAction::MergeVisibleLayers,
            Dispatched::Merge(MergeKind::Visible),
        ),
        (
            AppAction::FlattenLayers,
            Dispatched::Merge(MergeKind::Flatten),
        ),
        (AppAction::ExportFlat, Dispatched::ExportFlat),
    ] {
        assert_eq!(ejecuta(&mut sesion, accion), esperado, "{accion:?}");
    }
    // No tocan el documento por sí solas: lo hace la ventana tras la espera.
    assert_eq!(sesion.document.layers().len(), 2);
    // Con un trazo en curso, bloqueadas.
    let mut tools = px_ui::tool::ToolState::new();
    let locks = Locks {
        gesture: true,
        ..Locks::default()
    };
    let bloqueada = dispatch(
        AppAction::FlattenLayers,
        Origin::Key,
        &mut Targets {
            session: &mut sesion,
            tools: &mut tools,
            locks,
        },
    );
    assert_eq!(bloqueada, Dispatched::Blocked);
}

// ---------------------------------------------------------------------------------------------
// Fila 11: Exportar PNG aplanado
// ---------------------------------------------------------------------------------------------

fn sesion_con_tres_capas_y_una_oculta() -> Session {
    let mut sesion = sesion_con(&["A", "B", "C"]);
    pinta(&mut sesion, 1, &[(0, 0)], VERDE);
    pinta(&mut sesion, 2, &[(1, 0)], AZUL);
    let b = id(&sesion, 1);
    sesion
        .apply_layer(Command::SetLayerVisible {
            layer: b,
            visible: false,
        })
        .expect("ocultar");
    sesion
}

#[test]
fn fila_11_exportar_escribe_el_aplanado_y_no_toca_el_estado_del_documento() {
    let tmp = Tmp::nuevo();
    let mut sesion = sesion_con_tres_capas_y_una_oculta();
    let original = tmp.ruta().join("dibujo.png");
    fs::write(&original, b"x").expect("fichero");
    sesion.path = Some(original.clone());
    let (ruta, guardado, modificado, convertido) = (
        sesion.path().map(Path::to_path_buf),
        sesion.saved_id,
        sesion.modified(),
        sesion.converted(),
    );
    assert!(modificado, "hay cambios sin guardar");
    let copia = sesion.document.clone();

    let destino = tmp.ruta().join("plano.png");
    let guardado_en = sesion.export_flat(&destino).expect("exportar");
    assert_eq!(guardado_en.path, destino);
    let esperado =
        write_png(&px_ui::source::flatten(&sesion.document).expect("aplanar")).expect("png");
    assert_eq!(fs::read(&destino).expect("leer"), esperado);
    assert_eq!(
        fs::read(&original).expect("leer"),
        b"x",
        "el original no se toca"
    );

    assert_eq!(sesion.path().map(Path::to_path_buf), ruta);
    assert_eq!(sesion.saved_id, guardado);
    assert_eq!(sesion.modified(), modificado);
    assert_eq!(sesion.converted(), convertido);
    assert_eq!(sesion.document, copia);

    // El aviso cuenta capas y ocultas.
    let texto = sesion.describe_exported(&guardado_en);
    assert!(texto.contains("exportado plano.png"), "{texto}");
    assert!(texto.contains("aplanado de 3 capas"), "{texto}");
    assert!(texto.contains("1 oculta"), "{texto}");
}

#[test]
fn fila_11_exportar_con_una_capa_vale_y_sigue_las_reglas_de_ruta_de_guardar_como() {
    let tmp = Tmp::nuevo();
    let mut sesion = sesion_con(&["A"]);
    // Sin extensión se añade `.png`; si ya existe ese fichero no se pisa.
    let elegido = tmp.ruta().join("salida");
    let guardado = sesion.export_flat(&elegido).expect("exportar");
    assert_eq!(guardado.path, tmp.ruta().join("salida.png"));
    assert!(guardado.path.exists());
    let otra_vez = sesion.export_flat(&elegido);
    assert!(
        otra_vez.is_err(),
        "no pisa el fichero al que se añadió .png"
    );

    // El original que se convierte al abrir no se sobrescribe: se escribe la copia.
    let original = tmp.ruta().join("foto.png");
    fs::write(&original, b"original").expect("fichero");
    sesion.path = Some(original.clone());
    sesion.conversion = Some("modo".to_string());
    let guardado = sesion.export_flat(&original).expect("exportar");
    assert_ne!(guardado.path, original);
    assert_eq!(fs::read(&original).expect("leer"), b"original");
    assert!(sesion.converted(), "exportar no limpia la conversión");
}
