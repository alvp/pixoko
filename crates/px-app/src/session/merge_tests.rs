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

static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

struct Tmp {
    path_2: PathBuf,
}

impl Tmp {
    fn new_2() -> Tmp {
        let n = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let path_2 =
            std::env::temp_dir().join(format!("pixoko-px-app-combinar-{}-{n}", std::process::id()));
        fs::create_dir_all(&path_2).expect("directorio temporal");
        Tmp { path_2 }
    }

    fn path_2(&self) -> &Path {
        &self.path_2
    }
}

impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path_2);
    }
}

const TEST_BACKGROUND: Rgba = Rgba::opaque(10, 20, 30);
const TEST_GREEN: Rgba = Rgba::opaque(0, 255, 0);
const TEST_BLUE: Rgba = Rgba::opaque(0, 0, 255);
const TEST_RED: Rgba = Rgba::opaque(255, 0, 0);

fn rgba_full(width: u32, height: u32, color: Rgba) -> Image {
    Image::filled(width, height, Pixel::Rgba(color)).expect("imagen")
}

/// Un documento de 4 × 4 con las capas `names` de abajo arriba: la de abajo llena de `TEST_BACKGROUND` y
/// las demás vacías.
fn session_with(names: &[&str]) -> Session {
    let mut session_2 = Session::sample(Document::new(rgba_full(4, 4, TEST_BACKGROUND)));
    for (i, name_2) in names.iter().enumerate().skip(1) {
        session_2
            .apply_layer(Command::AddLayer {
                name: (*name_2).to_string(),
                at: i,
            })
            .expect("añadir capa");
    }
    session_2
}

fn id(session_2: &Session, position: usize) -> LayerId {
    session_2.document.layers()[position].id()
}

fn cel(session_2: &Session, position: usize) -> ImageId {
    session_2.document.layers()[position].cel()
}

fn paints(session_2: &mut Session, position: usize, points: &[(u32, u32)], color: Rgba) {
    let image = cel(session_2, position);
    session_2
        .document
        .apply(Command::SetPixels {
            image,
            pixels: points
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

fn runs(session_2: &mut Session, action: AppAction) -> Dispatched {
    let mut tools = px_ui::tool::ToolState::new();
    dispatch(
        action,
        Origin::Menu,
        &mut Targets {
            session: session_2,
            tools: &mut tools,
            locks: Locks::default(),
        },
    )
}

// ---------------------------------------------------------------------------------------------
// Fila 9: combinar abajo
// ---------------------------------------------------------------------------------------------

#[test]
fn row_9_merge_down_joins_the_active_with_the_one_below_and_the_active_one_returns_on_undo() {
    let mut session_2 = session_with(&["A", "B", "C"]);
    paints(&mut session_2, 1, &[(0, 0)], TEST_GREEN);
    paints(&mut session_2, 2, &[(1, 0)], TEST_BLUE);
    session_2
        .apply_layer(Command::SetLayerOpacity {
            layer: id(&session_2, 1),
            opacity: 128,
        })
        .expect("opacidad");
    let (b, c) = (id(&session_2, 1), id(&session_2, 2));
    session_2.set_active_layer(c).expect("activa");
    let before = session_2.document.clone();

    let report = session_2.merge(MergeKind::Down);
    assert!(report.changed, "{report:?}");
    assert!(
        report
            .notice
            .as_deref()
            .is_some_and(|t| t.contains("combinada")),
        "{report:?}"
    );
    let layers_2 = session_2.document.layers();
    assert_eq!(layers_2.len(), 2);
    assert_eq!(layers_2[1].id(), b, "queda la de abajo, con su id");
    assert_eq!(
        (
            layers_2[1].opacity(),
            layers_2[1].blend(),
            layers_2[1].locked()
        ),
        (255, BlendMode::Normal, false)
    );
    assert_eq!(session_2.active_layer(), b, "la activa es la que queda");
    assert!(session_2.modified());

    // Deshacer: vuelve C con sus propiedades y la activa vuelve a C (nota de la combinación).
    session_2.undo().expect("deshacer");
    assert_eq!(session_2.document, before);
    assert_eq!(session_2.active_layer(), c);
    // Rehacer: la activa es la superviviente.
    session_2.redo().expect("rehacer");
    assert_eq!(session_2.document.layers().len(), 2);
    assert_eq!(session_2.active_layer(), b);
}

#[test]
fn row_9_merge_down_rejects_with_its_notice_and_without_touching_anything() {
    // (cómo dejar el documento, qué trozo debe decir el aviso)
    type Prepare = fn(&mut Session);
    let cases: [(&str, Prepare, &str); 4] = [
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
    for (name_2, prepare, notice) in cases {
        let mut session_2 = session_with(&["A", "B", "C"]);
        paints(&mut session_2, 1, &[(0, 0)], TEST_GREEN);
        paints(&mut session_2, 2, &[(1, 0)], TEST_BLUE);
        let c = id(&session_2, 2);
        session_2.set_active_layer(c).expect("activa");
        prepare(&mut session_2);
        let (copy, state_2) = (session_2.document.clone(), session_2.document.state_id());
        let reason_2 = session_2.merge_refusal(MergeKind::Down);
        assert!(
            reason_2.is_some_and(|m| m.contains(notice)),
            "{name_2}: {reason_2:?}"
        );
        let report = session_2.merge(MergeKind::Down);
        assert!(!report.changed, "{name_2}");
        assert!(
            report.notice.as_deref().is_some_and(|t| t.contains(notice)),
            "{name_2}: {report:?}"
        );
        assert_eq!(session_2.document, copy, "{name_2}");
        assert_eq!(session_2.document.state_id(), state_2, "{name_2}");
    }
}

#[test]
fn row_9_merge_down_with_the_active_one_locked_works() {
    // El candado de la que se consume no protege de quitarla (decisión 9 de 3.4).
    let mut session_2 = session_with(&["A", "B"]);
    paints(&mut session_2, 1, &[(2, 2)], TEST_GREEN);
    let b = id(&session_2, 1);
    session_2
        .apply_layer(Command::SetLayerLocked {
            layer: b,
            locked: true,
        })
        .expect("bloquear");
    session_2.set_active_layer(b).expect("activa");
    assert_eq!(session_2.merge_refusal(MergeKind::Down), None);
    assert!(session_2.merge(MergeKind::Down).changed);
    assert_eq!(session_2.document.layers().len(), 1);
    assert!(!session_2.document.layers()[0].locked());
}

// ---------------------------------------------------------------------------------------------
// Fila 9: combinar visibles y aplanar
// ---------------------------------------------------------------------------------------------

#[test]
fn row_9_merge_visible_leaves_the_hidden_ones_and_needs_two_visible() {
    let mut session_2 = session_with(&["A", "B", "C", "D"]);
    paints(&mut session_2, 1, &[(0, 0)], TEST_GREEN);
    paints(&mut session_2, 2, &[(1, 0)], TEST_BLUE);
    paints(&mut session_2, 3, &[(2, 0)], TEST_RED);
    let (a, b, d) = (id(&session_2, 0), id(&session_2, 1), id(&session_2, 3));
    session_2
        .apply_layer(Command::SetLayerVisible {
            layer: b,
            visible: false,
        })
        .expect("ocultar");
    session_2
        .apply_layer(Command::SetLayerLocked {
            layer: d,
            locked: true,
        })
        .expect("bloquear");
    session_2.set_active_layer(d).expect("activa");
    let before = session_2.document.clone();

    assert_eq!(session_2.merge_refusal(MergeKind::Visible), None);
    let report = session_2.merge(MergeKind::Visible);
    assert!(report.changed, "{report:?}");
    let ids: Vec<LayerId> = session_2.document.layers().iter().map(Layer::id).collect();
    assert_eq!(ids, [a, b], "se quedan la visible más baja y la oculta");
    assert!(
        !session_2.document.layers()[0].locked(),
        "el resultado queda desbloqueado"
    );
    assert_eq!(session_2.active_layer(), a);
    session_2.undo().expect("deshacer");
    assert_eq!(session_2.document, before);
    assert_eq!(session_2.active_layer(), d);

    // Con una sola visible no hay nada que combinar.
    let mut a_2 = session_with(&["A", "B"]);
    let b = id(&a_2, 1);
    a_2.apply_layer(Command::SetLayerVisible {
        layer: b,
        visible: false,
    })
    .expect("ocultar");
    let (copy, state_2) = (a_2.document.clone(), a_2.document.state_id());
    assert!(
        a_2.merge_refusal(MergeKind::Visible)
            .is_some_and(|m| m.contains("hace falta más de una capa visible"))
    );
    assert!(!a_2.merge(MergeKind::Visible).changed);
    assert_eq!(
        (a_2.document.clone(), a_2.document.state_id()),
        (copy, state_2)
    );
}

#[test]
fn row_9_flatten_removes_the_hidden_ones_asks_for_confirmation_and_is_undone_whole() {
    let mut session_2 = session_with(&["A", "B", "C"]);
    paints(&mut session_2, 1, &[(0, 0)], TEST_GREEN);
    paints(&mut session_2, 2, &[(1, 0)], TEST_BLUE);
    let (b, c) = (id(&session_2, 1), id(&session_2, 2));
    session_2
        .apply_layer(Command::SetLayerVisible {
            layer: b,
            visible: false,
        })
        .expect("ocultar");
    session_2.set_active_layer(c).expect("activa");
    let before = session_2.document.clone();
    assert_eq!(session_2.flatten_hidden_with_pixels(), 1);

    let report = session_2.merge(MergeKind::Flatten);
    assert!(report.changed, "{report:?}");
    assert!(
        report
            .notice
            .as_deref()
            .is_some_and(|t| t.contains("aplanadas")),
        "{report:?}"
    );
    assert_eq!(session_2.document.layers().len(), 1);
    assert_eq!(session_2.active_layer(), id(&session_2, 0));
    session_2.undo().expect("deshacer");
    assert_eq!(session_2.document, before, "vuelven las tres con sus ids");
    assert_eq!(session_2.active_layer(), c);
    session_2.redo().expect("rehacer");
    assert_eq!(session_2.document.layers().len(), 1);

    // Una oculta sin píxeles no pide confirmación.
    let mut empty = session_with(&["A", "B", "C"]);
    let b = id(&empty, 1);
    empty
        .apply_layer(Command::SetLayerVisible {
            layer: b,
            visible: false,
        })
        .expect("ocultar");
    assert_eq!(empty.flatten_hidden_with_pixels(), 0);
}

#[test]
fn row_9_flatten_without_visible_layers_or_with_a_single_layer_is_rejected() {
    let mut session_2 = session_with(&["A", "B"]);
    for position in 0..2 {
        let layer_2 = id(&session_2, position);
        session_2
            .apply_layer(Command::SetLayerVisible {
                layer: layer_2,
                visible: false,
            })
            .expect("ocultar");
    }
    assert!(
        session_2
            .merge_refusal(MergeKind::Flatten)
            .is_some_and(|m| m.contains("no hay capas visibles que aplanar"))
    );
    let (copy, state_2) = (session_2.document.clone(), session_2.document.state_id());
    assert!(!session_2.merge(MergeKind::Flatten).changed);
    assert_eq!(
        (session_2.document.clone(), session_2.document.state_id()),
        (copy, state_2)
    );

    let mut a_2 = session_with(&["A"]);
    assert!(a_2.merge_refusal(MergeKind::Flatten).is_some());
    assert!(!a_2.merge(MergeKind::Flatten).changed);
}

#[test]
fn row_9_the_history_notice_comes_out_only_if_what_is_saved_exceeds_the_limit() {
    for (limit_2, expected_2) in [(16usize, true), (64 << 20, false)] {
        let mut session_2 = session_with(&["A", "B"]);
        paints(&mut session_2, 1, &[(0, 0)], TEST_GREEN);
        session_2.document.set_history_limit(limit_2);
        let b = id(&session_2, 1);
        session_2.set_active_layer(b).expect("activa");
        let report = session_2.merge(MergeKind::Down);
        assert!(report.changed);
        let says = report
            .notice
            .as_deref()
            .is_some_and(|t| t.contains("siguen en la historia hasta tu próxima edición"));
        assert_eq!(says, expected_2, "límite {limit_2}: {report:?}");
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 9: la confirmación de Aplanar y la espera de «Combinando…» y «Aplanando…» (condición (c))
// ---------------------------------------------------------------------------------------------

#[test]
fn row_9_flatten_with_hidden_ones_with_pixels_is_confirmed_by_repeating_within_5_seconds() {
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
fn row_9_merge_and_flatten_show_their_notice_before_starting_if_they_exceed_2_to_the_24() {
    assert_eq!(MERGE_SYNC_PIXELS, 1 << 24);
    assert_eq!(wait_notice(MergeKind::Down), "Combinando…");
    assert_eq!(wait_notice(MergeKind::Visible), "Combinando…");
    assert_eq!(wait_notice(MergeKind::Flatten), "Aplanando…");
    for kind in [MergeKind::Down, MergeKind::Visible, MergeKind::Flatten] {
        // Hasta 2²⁴: en el acto.
        assert_eq!(start_merge(kind, MERGE_SYNC_PIXELS), StartMerge::Now);
        // Más: espera `WAIT_FRAMES` fotogramas, ejecuta exactamente en el último y nunca después.
        let StartMerge::Later(mut waits) = start_merge(kind, MERGE_SYNC_PIXELS + 1) else {
            panic!("{kind:?}: debía esperar");
        };
        assert_eq!(waits.kind(), kind);
        for frame in 1..=WAIT_FRAMES {
            assert_eq!(waits.tick(), frame == WAIT_FRAMES, "{kind:?} {frame}");
        }
        assert!(!waits.tick());
        assert!(!waits.tick());
    }
}

#[test]
fn row_9_the_pixels_to_compose_are_width_times_height_times_visible_layers_with_an_image() {
    let mut session_2 = session_with(&["A", "B", "C", "D"]);
    paints(&mut session_2, 1, &[(0, 0)], TEST_GREEN);
    paints(&mut session_2, 2, &[(1, 0)], TEST_BLUE);
    // D queda vacía; C se oculta: componen A y B.
    let c = id(&session_2, 2);
    session_2
        .apply_layer(Command::SetLayerVisible {
            layer: c,
            visible: false,
        })
        .expect("ocultar");
    let d = id(&session_2, 3);
    session_2.set_active_layer(d).expect("activa");
    assert_eq!(
        merge_pixels(&session_2.document, MergeKind::Visible, d),
        4 * 4 * 2
    );
    assert_eq!(
        merge_pixels(&session_2.document, MergeKind::Flatten, d),
        4 * 4 * 2
    );
    // Combinar abajo: la activa (vacía) y la de abajo (oculta, con imagen): una sola con imagen
    // entra en la cuenta si es visible; aquí C está oculta, así que 0 con imagen visible.
    assert_eq!(merge_pixels(&session_2.document, MergeKind::Down, d), 0);
}

// ---------------------------------------------------------------------------------------------
// Fila 10 (parte de px-app): las cuatro acciones llegan a la ventana
// ---------------------------------------------------------------------------------------------

#[test]
fn row_10_dispatch_passes_the_four_actions_to_the_window_and_respects_the_locks() {
    let mut session_2 = session_with(&["A", "B"]);
    for (action, expected_2) in [
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
        assert_eq!(runs(&mut session_2, action), expected_2, "{action:?}");
    }
    // No tocan el documento por sí solas: lo hace la ventana tras la espera.
    assert_eq!(session_2.document.layers().len(), 2);
    // Con un trazo en curso, bloqueadas.
    let mut tools = px_ui::tool::ToolState::new();
    let locks = Locks {
        gesture: true,
        ..Locks::default()
    };
    let locked_2 = dispatch(
        AppAction::FlattenLayers,
        Origin::Key,
        &mut Targets {
            session: &mut session_2,
            tools: &mut tools,
            locks,
        },
    );
    assert_eq!(locked_2, Dispatched::Blocked);
}

// ---------------------------------------------------------------------------------------------
// Fila 11: Exportar PNG aplanado
// ---------------------------------------------------------------------------------------------

fn session_with_three_layers_and_one_hidden() -> Session {
    let mut session_2 = session_with(&["A", "B", "C"]);
    paints(&mut session_2, 1, &[(0, 0)], TEST_GREEN);
    paints(&mut session_2, 2, &[(1, 0)], TEST_BLUE);
    let b = id(&session_2, 1);
    session_2
        .apply_layer(Command::SetLayerVisible {
            layer: b,
            visible: false,
        })
        .expect("ocultar");
    session_2
}

#[test]
fn row_11_export_writes_the_flattened_and_does_not_touch_the_document_state() {
    let tmp = Tmp::new_2();
    let mut session_2 = session_with_three_layers_and_one_hidden();
    let original = tmp.path_2().join("dibujo.png");
    fs::write(&original, b"x").expect("fichero");
    session_2.path = Some(original.clone());
    let (path_2, saved, modified_2, converted) = (
        session_2.path().map(Path::to_path_buf),
        session_2.saved_id,
        session_2.modified(),
        session_2.converted(),
    );
    assert!(modified_2, "hay cambios sin guardar");
    let copy = session_2.document.clone();

    let target_2 = tmp.path_2().join("plano.png");
    let saved_in = session_2.export_flat(&target_2).expect("exportar");
    assert_eq!(saved_in.path, target_2);
    let expected_2 =
        write_png(&px_ui::source::flatten(&session_2.document).expect("aplanar")).expect("png");
    assert_eq!(fs::read(&target_2).expect("leer"), expected_2);
    assert_eq!(
        fs::read(&original).expect("leer"),
        b"x",
        "el original no se toca"
    );

    assert_eq!(session_2.path().map(Path::to_path_buf), path_2);
    assert_eq!(session_2.saved_id, saved);
    assert_eq!(session_2.modified(), modified_2);
    assert_eq!(session_2.converted(), converted);
    assert_eq!(session_2.document, copy);

    // El aviso cuenta capas y ocultas.
    let text_2 = session_2.describe_exported(&saved_in);
    assert!(text_2.contains("exportado plano.png"), "{text_2}");
    assert!(text_2.contains("aplanado de 3 capas"), "{text_2}");
    assert!(text_2.contains("1 oculta"), "{text_2}");
}

#[test]
fn row_11_export_with_one_layer_works_and_follows_the_path_rules_of_save_as() {
    let tmp = Tmp::new_2();
    let mut session_2 = session_with(&["A"]);
    // Sin extensión se añade `.png`; si ya existe ese fichero no se pisa.
    let chosen = tmp.path_2().join("salida");
    let saved = session_2.export_flat(&chosen).expect("exportar");
    assert_eq!(saved.path, tmp.path_2().join("salida.png"));
    assert!(saved.path.exists());
    let other_time = session_2.export_flat(&chosen);
    assert!(
        other_time.is_err(),
        "no pisa el fichero al que se añadió .png"
    );

    // El original que se convierte al abrir no se sobrescribe: se escribe la copia.
    let original = tmp.path_2().join("foto.png");
    fs::write(&original, b"original").expect("fichero");
    session_2.path = Some(original.clone());
    session_2.conversion = Some("modo".to_string());
    let saved = session_2.export_flat(&original).expect("exportar");
    assert_ne!(saved.path, original);
    assert_eq!(fs::read(&original).expect("leer"), b"original");
    assert!(session_2.converted(), "exportar no limpia la conversión");
}
