//! `TestImage → Convert mode_2…` con varias capas (specs/hito-3-combinar.md, unidad 3.5: fila 12,
//! decisión D8 y condición (a)). Sin ventana: el formulario, el análisis del documento, el informe
//! sumado, el motivo que ofrece Aplanar y la espera en dos tiempos. En ROJO hasta la lógica.
//!
//! Contrato de API que estas pruebas fijan (la lógica debe ofrecerlo; las firmas de lo que ya
//! existía no cambian):
//! - `FIELD_FLATTEN`, `convert_form_layers(from, layers)`, `open_convert_mode_layers(modal, from,
//!   layers, target, locks)`, `flatten_offer(field) -> Option<AppAction>`.
//! - `LayerBlock::{MultiLayerGray, LayerProps}` con `reason()`, `layer_block(&Document, Mode)` y
//!   `analyze_document(&Document, Mode) -> Analysis` (solo si no hay bloqueo de capas).
//! - `report_text_all(analysis, layers, size, perms, active, pixel_bytes, history)`.
//! - `DocAnalyzer` (`analyze(&mut self, &Document, Mode) -> Analysis`) y `sync_convert_all(dialog,
//!   sync, doc, history, analyzer) -> bool`.
//! - `convert_pixels(&Document) -> u64`: píxeles de las celdas con imagen (para la espera).

use super::*;

use px_core::{
    Allowed, BlendMode, Command, ConversionReport, Document, Image, Layer, Mode, Pixel, PixelEdit,
    Rgba,
};
use px_ui::actions::{AppAction, Locks};
use px_ui::dialog::{Dialog, Event, FieldKind, Modal, Reaction};

use crate::session::Session;

const GRAY: Rgba = Rgba::opaque(40, 40, 40);
const NONE: Option<Rgba> = Some(Rgba::opaque(1, 2, 3));
const MIB: usize = 1 << 20;

fn history_2() -> History {
    History {
        limit: 64 * MIB,
        used: 0,
    }
}

/// Un documento RGBA de 4 × 4 con `layers_2` capas; la de abajo, llena de gris, y las demás vacías.
fn document(layers_2: usize) -> Document {
    let mut doc = Document::new(Image::filled(4, 4, Pixel::Rgba(GRAY)).expect("imagen"));
    for i in 1..layers_2 {
        doc.apply(Command::AddLayer {
            name: format!("Capa {}", i + 1),
            at: i,
        })
        .expect("añadir capa");
    }
    doc
}

fn paints(doc: &mut Document, position: usize, points: &[(u32, u32)], color: Rgba) {
    let image = doc.layers()[position].cel();
    doc.apply(Command::SetPixels {
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

fn dialog(modal: &Modal) -> &Dialog {
    match modal {
        Modal::Open(d) => d,
        Modal::Closed => panic!("el diálogo está cerrado"),
    }
}

fn field_2<'a>(
    spec: &'a px_ui::dialog::FormSpec,
    id: &str,
) -> Option<&'a px_ui::dialog::FieldSpec> {
    spec.fields.iter().find(|f| f.id == id)
}

fn report_of(d: &Dialog) -> String {
    match &field_2(d.spec(), "informe").expect("informe").kind {
        FieldKind::ReadOnly { text, .. } => text.clone(),
        other_2 => panic!("{other_2:?}"),
    }
}

// ---------------------------------------------------------------------------------------------
// El formulario ofrece Aplanar (condición (a))
// ---------------------------------------------------------------------------------------------

#[test]
fn row_12_with_several_layers_the_form_gains_a_flatten_button_only_with_a_gray_target() {
    for from in [Mode::Rgba, Mode::Indexed, Mode::GrayAlpha] {
        // Con una capa es el formulario de siempre (más el botón de reducir colores, 4.2, que
        // solo tienen RGBA y gris con alfa).
        let a_2 = convert_form_layers(from, 1);
        let without_quantize: Vec<_> = a_2
            .fields
            .iter()
            .filter(|f| f.id != FIELD_QUANTIZE)
            .cloned()
            .collect();
        assert_eq!(without_quantize, convert_form(from).fields, "{from:?}");
        assert!(field_2(&a_2, FIELD_FLATTEN).is_none(), "{from:?}");

        let several = convert_form_layers(from, 3);
        let button_2 = field_2(&several, FIELD_FLATTEN).unwrap_or_else(|| panic!("{from:?}"));
        match &button_2.kind {
            FieldKind::Button { text, aside } => {
                assert_eq!(text, "Aplanar");
                assert_eq!(*aside, None);
            }
            other_2 => panic!("{from:?}: {other_2:?}"),
        }
        // Visible solo con el destino Gris (el último de los tres).
        let gray = targets(from)
            .iter()
            .position(|m| *m == Mode::Gray)
            .expect("Gris");
        let when = button_2.shown_when.expect("condicionado al destino");
        assert_eq!((when.field, when.choice), (FIELD_TARGET, gray), "{from:?}");
        // El resto de campos, intactos.
        let rest: Vec<_> = several
            .fields
            .iter()
            .filter(|f| f.id != FIELD_FLATTEN && f.id != FIELD_QUANTIZE)
            .cloned()
            .collect();
        assert_eq!(rest, convert_form(from).fields, "{from:?}");
    }
}

#[test]
fn row_12_the_flatten_button_launches_the_flatten_action() {
    assert_eq!(flatten_offer(FIELD_FLATTEN), Some(AppAction::FlattenLayers));
    for other_2 in [FIELD_TARGET, FIELD_REPORT, FIELD_LUMA_GRAY, FIELD_ALPHA] {
        assert_eq!(flatten_offer(other_2), None, "{other_2}");
    }
}

// ---------------------------------------------------------------------------------------------
// El análisis del documento y sus motivos
// ---------------------------------------------------------------------------------------------

#[test]
fn row_12_analyzes_all_the_layers_and_sums_the_losses() {
    let mut doc = document(3);
    paints(
        &mut doc,
        1,
        &[(0, 0), (1, 0), (2, 0)],
        Rgba::opaque(255, 0, 0),
    );
    paints(
        &mut doc,
        2,
        &[(0, 1), (1, 1), (2, 1), (3, 1), (0, 2)],
        Rgba::opaque(0, 0, 255),
    );
    // Gris con alfa: 3 + 5 píxeles de color se pierden (la capa de abajo es gris).
    assert_eq!(
        analyze_document(&doc, Mode::GrayAlpha),
        Analysis::Lossy(ConversionReport {
            color_lost: 8,
            alpha_lost: 0
        })
    );
    // A RGBA es exacto; a Gris, con varias capas, no hay modo (condición (a)).
    assert_eq!(
        analyze_document(&doc, Mode::Rgba),
        Analysis::Exact { palette: None }
    );
    assert_eq!(
        layer_block(&doc, Mode::Gray),
        Some(LayerBlock::MultiLayerGray)
    );
    assert_eq!(layer_block(&doc, Mode::Rgba), None);
    // A indexado: una paleta común con el transparente en el 0 (siempre con varias capas) más el
    // gris, el rojo y el azul.
    assert_eq!(
        analyze_document(&doc, Mode::Indexed),
        Analysis::Exact {
            palette: Some(PaletteInfo {
                colors: 4,
                transparent: true
            })
        }
    );
}

#[test]
fn row_12_indexed_with_opacity_or_blend_or_with_too_many_colors_is_not_valid() {
    let mut doc = document(2);
    let b = doc.layers()[1].id();
    doc.apply(Command::SetLayerOpacity {
        layer: b,
        opacity: 128,
    })
    .expect("opacidad");
    assert_eq!(
        layer_block(&doc, Mode::Indexed),
        Some(LayerBlock::LayerProps)
    );
    doc.apply(Command::SetLayerOpacity {
        layer: b,
        opacity: 255,
    })
    .expect("opacidad");
    doc.apply(Command::SetLayerBlend {
        layer: b,
        blend: BlendMode::Multiply,
    })
    .expect("mezcla");
    assert_eq!(
        layer_block(&doc, Mode::Indexed),
        Some(LayerBlock::LayerProps)
    );
    doc.apply(Command::SetLayerBlend {
        layer: b,
        blend: BlendMode::Normal,
    })
    .expect("mezcla");

    // Dos capas de 200 colores distintos cada una: la unión pasa de 256.
    let mut large = Document::new(Image::new(16, 16, Mode::Rgba).expect("imagen"));
    large
        .apply(Command::AddLayer {
            name: "B".to_string(),
            at: 1,
        })
        .expect("añadir");
    for (position, from_2) in [(0usize, 0u32), (1, 100)] {
        let image = large.layers()[position].cel();
        let pixels: Vec<PixelEdit> = (0..200u32)
            .map(|k| PixelEdit {
                x: k % 16,
                y: k / 16,
                pixel: Pixel::Rgba(Rgba::opaque((from_2 + k) as u8 | 1, k as u8, 7)),
            })
            .collect();
        large
            .apply(Command::SetPixels { image, pixels })
            .expect("pintar");
    }
    assert_eq!(
        analyze_document(&large, Mode::Indexed),
        Analysis::TooManyColors
    );
}

#[test]
fn row_12_the_reasons_are_short_and_the_gray_one_offers_flatten() {
    let gray = LayerBlock::MultiLayerGray.reason();
    assert_eq!(gray, "con varias capas: aplana antes");
    let layers_2 = LayerBlock::LayerProps.reason();
    assert_eq!(layers_2, "hay capas con opacidad o mezcla");
    for reason_2 in [gray, layers_2] {
        assert!(reason_2.chars().count() <= 37, "{reason_2}");
    }
}

// ---------------------------------------------------------------------------------------------
// El diálogo entero: informe sumado, motivo, botón y espera
// ---------------------------------------------------------------------------------------------

/// Cuenta las llamadas y da el análisis real del documento.
#[derive(Default)]
struct Count {
    calls: Vec<Mode>,
}

impl DocAnalyzer for Count {
    fn analyze(&mut self, doc: &Document, target: Mode) -> Analysis {
        self.calls.push(target);
        analyze_document(doc, target)
    }
}

struct Setup {
    modal: Modal,
    sync: ConvertSync,
    count: Count,
    doc: Document,
}

impl Setup {
    fn new_2(doc: Document) -> Setup {
        let from = doc.mode();
        let mut modal = Modal::default();
        assert!(open_convert_mode_layers(
            &mut modal,
            from,
            doc.layers().len(),
            None,
            Locks::default()
        ));
        Setup {
            modal,
            sync: ConvertSync::new(from, doc.state_id(), NONE),
            count: Count::default(),
            doc,
        }
    }

    fn synchronizes(&mut self) -> bool {
        let Setup {
            modal,
            sync,
            count,
            doc,
        } = self;
        let Modal::Open(dialog) = modal else {
            panic!("el diálogo está cerrado");
        };
        sync_convert_all(dialog, sync, doc, history_2(), count)
    }

    fn dialog_mut(&mut self) -> &mut Dialog {
        match &mut self.modal {
            Modal::Open(d) => d,
            Modal::Closed => panic!("el diálogo está cerrado"),
        }
    }

    fn chooses_target(&mut self, target_2: Mode) {
        let index = targets(self.doc.mode())
            .iter()
            .position(|m| *m == target_2)
            .expect("destino");
        assert_eq!(
            self.dialog_mut().event(Event::Choice {
                field: FIELD_TARGET,
                index
            }),
            Reaction::None
        );
    }
}

#[test]
fn row_12_the_report_carries_the_layers_and_the_summed_losses() {
    let mut doc = document(3);
    paints(
        &mut doc,
        1,
        &[(0, 0), (1, 0), (2, 0)],
        Rgba::opaque(255, 0, 0),
    );
    paints(
        &mut doc,
        2,
        &[(0, 1), (1, 1), (2, 1), (3, 1), (0, 2)],
        Rgba::opaque(0, 0, 255),
    );
    let mut m = Setup::new_2(doc);
    m.synchronizes();
    // El primer destino de RGBA es Indexado: exacto, con la paleta común.
    let text_2 = report_of(dialog(&m.modal));
    assert!(
        text_2.contains("Capas: 3 (se convierten todas)"),
        "{text_2}"
    );
    assert!(
        text_2.contains("Paleta nueva de 4 colores, el 0 transparente"),
        "{text_2}"
    );
    m.chooses_target(Mode::GrayAlpha);
    m.synchronizes();
    let text_2 = report_of(dialog(&m.modal));
    assert!(
        text_2.contains("Capas: 3 (se convierten todas)"),
        "{text_2}"
    );
    assert!(text_2.contains("Pierde el color en 8 píxeles"), "{text_2}");
    assert_eq!(
        dialog(&m.modal).block(),
        Some("falta: permitir perder el color")
    );
    // Se calculó una vez por destino y no al repetir el fotograma.
    m.synchronizes();
    assert_eq!(m.count.calls, [Mode::Indexed, Mode::GrayAlpha]);
}

#[test]
fn row_12_gray_with_several_layers_blocks_accept_and_the_flatten_button_shows() {
    let mut m = Setup::new_2(document(3));
    m.synchronizes();
    assert!(!dialog(&m.modal).is_shown(FIELD_FLATTEN), "con Indexado no");
    m.chooses_target(Mode::Gray);
    m.synchronizes();
    let d = dialog(&m.modal);
    assert_eq!(d.block(), Some("con varias capas: aplana antes"));
    assert!(d.is_shown(FIELD_FLATTEN), "el mensaje ofrece Aplanar");
    // `Accept` rechazado mientras haya motivo.
    assert!(matches!(
        d.clone().event(Event::Accept),
        Reaction::Rejected(_)
    ));
    // Pulsar el botón no cierra el diálogo: la ventana lo cierra y lanza la acción.
    let reaction = m.dialog_mut().event(Event::Press {
        field: FIELD_FLATTEN,
        aside: false,
    });
    assert_eq!(
        reaction,
        Reaction::Pressed {
            field: FIELD_FLATTEN,
            aside: false
        }
    );
    // Volver a otro destino lo esconde.
    m.chooses_target(Mode::GrayAlpha);
    m.synchronizes();
    assert!(!dialog(&m.modal).is_shown(FIELD_FLATTEN));
}

#[test]
fn row_12_indexed_with_layers_with_opacity_says_its_reason() {
    let mut doc = document(2);
    let b = doc.layers()[1].id();
    doc.apply(Command::SetLayerOpacity {
        layer: b,
        opacity: 128,
    })
    .expect("opacidad");
    let mut m = Setup::new_2(doc);
    m.synchronizes();
    assert_eq!(
        dialog(&m.modal).block(),
        Some("hay capas con opacidad o mezcla")
    );
}

#[test]
fn row_12_the_computation_counts_the_pixels_of_all_cels_and_takes_two_steps() {
    // Una capa sola de 1500 × 1500 (2,25 Mpx) se calcula en el acto; con dos capas con imagen
    // son 4,5 Mpx (> 2²²) y espera.
    let mut large = Document::new(Image::new(1500, 1500, Mode::Rgba).expect("imagen"));
    large
        .apply(Command::AddLayer {
            name: "B".to_string(),
            at: 1,
        })
        .expect("añadir");
    assert_eq!(
        convert_pixels(&large),
        1500 * 1500,
        "la celda vacía no cuenta"
    );
    let image = large.layers()[1].cel();
    large
        .apply(Command::SetPixels {
            image,
            pixels: vec![PixelEdit {
                x: 0,
                y: 0,
                pixel: Pixel::Rgba(Rgba::opaque(1, 2, 3)),
            }],
        })
        .expect("pintar");
    assert_eq!(convert_pixels(&large), 2 * 1500 * 1500);

    let mut m = Setup::new_2(large);
    let mut first = None;
    for frame in 1..=6 {
        m.synchronizes();
        if first.is_none() && !m.count.calls.is_empty() {
            first = Some(frame);
        }
    }
    let first = first.expect("acaba calculando");
    assert!(
        first > 1,
        "el primer fotograma solo avisa (llegó en el {first})"
    );
    assert_eq!(first, usize::from(WAIT_FRAMES) + 1);
    assert_eq!(m.count.calls.len(), 1, "se calcula una sola vez");

    // Documento pequeño: en el primer fotograma.
    let mut small = Setup::new_2(document(3));
    small.synchronizes();
    assert_eq!(small.count.calls.len(), 1);
}

// ---------------------------------------------------------------------------------------------
// Aplicar: un paso, deshacer, activa y paleta común
// ---------------------------------------------------------------------------------------------

#[test]
fn row_12_accept_converts_all_the_layers_in_one_step_and_it_can_be_undone() {
    let mut doc = document(3);
    paints(&mut doc, 1, &[(0, 0)], Rgba::opaque(255, 0, 0));
    paints(&mut doc, 2, &[(1, 0)], Rgba::opaque(0, 0, 255));
    let mut session_2 = Session::sample(doc);
    let active_2 = session_2.document.layers()[1].id();
    session_2.set_active_layer(active_2).expect("activa");
    let before = session_2.document.clone();
    let allowed = Allowed {
        luma: false,
        drop_alpha: None,
    };
    assert_eq!(session_2.convert_all(Mode::Indexed, allowed), Ok(true));
    assert_eq!(session_2.document.mode(), Mode::Indexed);
    let palette = session_2.document.palette().expect("paleta").clone();
    for layer_2 in session_2.document.layers() {
        if let Some(image_2) = session_2.document.image(layer_2.cel()) {
            assert_eq!(image_2.palette(), Some(&palette), "paleta común");
        }
    }
    assert_eq!(session_2.active_layer(), active_2);
    assert!(session_2.modified());
    session_2.undo().expect("deshacer");
    assert_eq!(session_2.document, before, "un solo paso");
    assert_eq!(session_2.document.mode(), Mode::Rgba);
    let _: &[Layer] = session_2.document.layers();
}

// ---------------------------------------------------------------------------------------------
// Unidad L1, fila 4 (specs/limpieza-1.md): `Analysis::Blocked(LayerBlock)`. En ROJO hasta la lógica.
// ---------------------------------------------------------------------------------------------

/// Con bloqueo de capas, `analyze_document` da `Analysis::Blocked(..)` (sin mirar los píxeles); sin
/// bloqueo, lo de siempre.
#[test]
fn row_04_analyze_document_with_layer_lock_gives_blocked() {
    let doc = document(2);
    assert_eq!(
        analyze_document(&doc, Mode::Gray),
        Analysis::Blocked(LayerBlock::MultiLayerGray)
    );
    assert_eq!(
        analyze_document(&doc, Mode::Rgba),
        Analysis::Exact { palette: None }
    );

    let mut doc = document(2);
    let b = doc.layers()[1].id();
    doc.apply(Command::SetLayerOpacity {
        layer: b,
        opacity: 128,
    })
    .expect("opacidad");
    assert_eq!(
        analyze_document(&doc, Mode::Indexed),
        Analysis::Blocked(LayerBlock::LayerProps)
    );
}

/// El motivo (≤ 37 caracteres) y la explicación de `Blocked` son los de `LayerBlock`; no hay nada
/// que permitir ni que pedir.
#[test]
fn row_04_the_reason_and_report_of_blocked_are_the_locks() {
    let perms = Permissions {
        luma: false,
        alpha: AlphaChoice::Keep,
    };
    for block in [LayerBlock::MultiLayerGray, LayerBlock::LayerProps] {
        let analysis = Analysis::Blocked(block);
        assert_eq!(
            block_reason(Some(&analysis), &perms, None),
            Some(block.reason().to_string())
        );
        assert!(pending(&analysis, &perms, None, true).is_empty());
        assert_eq!(
            needs(&analysis),
            Needs {
                luma: false,
                alpha: false
            }
        );
        let report = report_text_all(Some(&analysis), 2, (4, 4), &perms, None, 32, history_2());
        assert!(report.contains(block.explanation()), "{report}");
    }
}
