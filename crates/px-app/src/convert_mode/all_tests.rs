//! `Imagen → Convertir modo…` con varias capas (specs/hito-3-combinar.md, unidad 3.5: fila 12,
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

const GRIS: Rgba = Rgba::opaque(40, 40, 40);
const NINGUNO: Option<Rgba> = Some(Rgba::opaque(1, 2, 3));
const MIB: usize = 1 << 20;

fn historial() -> History {
    History {
        limit: 64 * MIB,
        used: 0,
    }
}

/// Un documento RGBA de 4 × 4 con `capas` capas; la de abajo, llena de gris, y las demás vacías.
fn documento(capas: usize) -> Document {
    let mut doc = Document::new(Image::filled(4, 4, Pixel::Rgba(GRIS)).expect("imagen"));
    for i in 1..capas {
        doc.apply(Command::AddLayer {
            name: format!("Capa {}", i + 1),
            at: i,
        })
        .expect("añadir capa");
    }
    doc
}

fn pinta(doc: &mut Document, posicion: usize, puntos: &[(u32, u32)], color: Rgba) {
    let image = doc.layers()[posicion].cel();
    doc.apply(Command::SetPixels {
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

fn dialogo(modal: &Modal) -> &Dialog {
    match modal {
        Modal::Open(d) => d,
        Modal::Closed => panic!("el diálogo está cerrado"),
    }
}

fn campo<'a>(spec: &'a px_ui::dialog::FormSpec, id: &str) -> Option<&'a px_ui::dialog::FieldSpec> {
    spec.fields.iter().find(|f| f.id == id)
}

fn informe_de(d: &Dialog) -> String {
    match &campo(d.spec(), "informe").expect("informe").kind {
        FieldKind::ReadOnly { text, .. } => text.clone(),
        otro => panic!("{otro:?}"),
    }
}

// ---------------------------------------------------------------------------------------------
// El formulario ofrece Aplanar (condición (a))
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_12_con_varias_capas_el_formulario_gana_un_boton_aplanar_solo_con_destino_gris() {
    for from in [Mode::Rgba, Mode::Indexed, Mode::GrayAlpha] {
        // Con una capa es el formulario de siempre.
        let una = convert_form_layers(from, 1);
        assert_eq!(una.fields, convert_form(from).fields, "{from:?}");
        assert!(campo(&una, FIELD_FLATTEN).is_none(), "{from:?}");

        let varias = convert_form_layers(from, 3);
        let boton = campo(&varias, FIELD_FLATTEN).unwrap_or_else(|| panic!("{from:?}"));
        match &boton.kind {
            FieldKind::Button { text, aside } => {
                assert_eq!(text, "Aplanar");
                assert_eq!(*aside, None);
            }
            otro => panic!("{from:?}: {otro:?}"),
        }
        // Visible solo con el destino Gris (el último de los tres).
        let gris = targets(from)
            .iter()
            .position(|m| *m == Mode::Gray)
            .expect("Gris");
        let cuando = boton.shown_when.expect("condicionado al destino");
        assert_eq!(
            (cuando.field, cuando.choice),
            (FIELD_TARGET, gris),
            "{from:?}"
        );
        // El resto de campos, intactos.
        let resto: Vec<_> = varias
            .fields
            .iter()
            .filter(|f| f.id != FIELD_FLATTEN)
            .cloned()
            .collect();
        assert_eq!(resto, convert_form(from).fields, "{from:?}");
    }
}

#[test]
fn fila_12_el_boton_aplanar_lanza_la_accion_aplanar() {
    assert_eq!(flatten_offer(FIELD_FLATTEN), Some(AppAction::FlattenLayers));
    for otro in [FIELD_TARGET, FIELD_REPORT, FIELD_LUMA_GRAY, FIELD_ALPHA] {
        assert_eq!(flatten_offer(otro), None, "{otro}");
    }
}

// ---------------------------------------------------------------------------------------------
// El análisis del documento y sus motivos
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_12_analiza_todas_las_capas_y_suma_las_perdidas() {
    let mut doc = documento(3);
    pinta(
        &mut doc,
        1,
        &[(0, 0), (1, 0), (2, 0)],
        Rgba::opaque(255, 0, 0),
    );
    pinta(
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
fn fila_12_indexado_con_opacidad_o_mezcla_o_con_demasiados_colores_no_vale() {
    let mut doc = documento(2);
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
    let mut grande = Document::new(Image::new(16, 16, Mode::Rgba).expect("imagen"));
    grande
        .apply(Command::AddLayer {
            name: "B".to_string(),
            at: 1,
        })
        .expect("añadir");
    for (posicion, desde) in [(0usize, 0u32), (1, 100)] {
        let image = grande.layers()[posicion].cel();
        let pixels: Vec<PixelEdit> = (0..200u32)
            .map(|k| PixelEdit {
                x: k % 16,
                y: k / 16,
                pixel: Pixel::Rgba(Rgba::opaque((desde + k) as u8 | 1, k as u8, 7)),
            })
            .collect();
        grande
            .apply(Command::SetPixels { image, pixels })
            .expect("pintar");
    }
    assert_eq!(
        analyze_document(&grande, Mode::Indexed),
        Analysis::TooManyColors
    );
}

#[test]
fn fila_12_los_motivos_son_cortos_y_el_de_gris_ofrece_aplanar() {
    let gris = LayerBlock::MultiLayerGray.reason();
    assert_eq!(gris, "con varias capas: aplana antes");
    let capas = LayerBlock::LayerProps.reason();
    assert_eq!(capas, "hay capas con opacidad o mezcla");
    for motivo in [gris, capas] {
        assert!(motivo.chars().count() <= 37, "{motivo}");
    }
}

// ---------------------------------------------------------------------------------------------
// El diálogo entero: informe sumado, motivo, botón y espera
// ---------------------------------------------------------------------------------------------

/// Cuenta las llamadas y da el análisis real del documento.
#[derive(Default)]
struct Cuenta {
    llamadas: Vec<Mode>,
}

impl DocAnalyzer for Cuenta {
    fn analyze(&mut self, doc: &Document, target: Mode) -> Analysis {
        self.llamadas.push(target);
        analyze_document(doc, target)
    }
}

struct Montaje {
    modal: Modal,
    sync: ConvertSync,
    cuenta: Cuenta,
    doc: Document,
}

impl Montaje {
    fn nuevo(doc: Document) -> Montaje {
        let from = doc.mode();
        let mut modal = Modal::default();
        assert!(open_convert_mode_layers(
            &mut modal,
            from,
            doc.layers().len(),
            None,
            Locks::default()
        ));
        Montaje {
            modal,
            sync: ConvertSync::new(from, doc.state_id(), NINGUNO),
            cuenta: Cuenta::default(),
            doc,
        }
    }

    fn sincroniza(&mut self) -> bool {
        let Montaje {
            modal,
            sync,
            cuenta,
            doc,
        } = self;
        let Modal::Open(dialogo) = modal else {
            panic!("el diálogo está cerrado");
        };
        sync_convert_all(dialogo, sync, doc, historial(), cuenta)
    }

    fn dialogo_mut(&mut self) -> &mut Dialog {
        match &mut self.modal {
            Modal::Open(d) => d,
            Modal::Closed => panic!("el diálogo está cerrado"),
        }
    }

    fn elige_destino(&mut self, destino: Mode) {
        let index = targets(self.doc.mode())
            .iter()
            .position(|m| *m == destino)
            .expect("destino");
        assert_eq!(
            self.dialogo_mut().event(Event::Choice {
                field: FIELD_TARGET,
                index
            }),
            Reaction::None
        );
    }
}

#[test]
fn fila_12_el_informe_lleva_las_capas_y_las_perdidas_sumadas() {
    let mut doc = documento(3);
    pinta(
        &mut doc,
        1,
        &[(0, 0), (1, 0), (2, 0)],
        Rgba::opaque(255, 0, 0),
    );
    pinta(
        &mut doc,
        2,
        &[(0, 1), (1, 1), (2, 1), (3, 1), (0, 2)],
        Rgba::opaque(0, 0, 255),
    );
    let mut m = Montaje::nuevo(doc);
    m.sincroniza();
    // El primer destino de RGBA es Indexado: exacto, con la paleta común.
    let texto = informe_de(dialogo(&m.modal));
    assert!(texto.contains("Capas: 3 (se convierten todas)"), "{texto}");
    assert!(
        texto.contains("Paleta nueva de 4 colores, el 0 transparente"),
        "{texto}"
    );
    m.elige_destino(Mode::GrayAlpha);
    m.sincroniza();
    let texto = informe_de(dialogo(&m.modal));
    assert!(texto.contains("Capas: 3 (se convierten todas)"), "{texto}");
    assert!(texto.contains("Pierde el color en 8 píxeles"), "{texto}");
    assert_eq!(
        dialogo(&m.modal).block(),
        Some("falta: permitir perder el color")
    );
    // Se calculó una vez por destino y no al repetir el fotograma.
    m.sincroniza();
    assert_eq!(m.cuenta.llamadas, [Mode::Indexed, Mode::GrayAlpha]);
}

#[test]
fn fila_12_gris_con_varias_capas_bloquea_aceptar_y_el_boton_aplanar_se_ve() {
    let mut m = Montaje::nuevo(documento(3));
    m.sincroniza();
    assert!(
        !dialogo(&m.modal).is_shown(FIELD_FLATTEN),
        "con Indexado no"
    );
    m.elige_destino(Mode::Gray);
    m.sincroniza();
    let d = dialogo(&m.modal);
    assert_eq!(d.block(), Some("con varias capas: aplana antes"));
    assert!(d.is_shown(FIELD_FLATTEN), "el mensaje ofrece Aplanar");
    // `Aceptar` rechazado mientras haya motivo.
    assert!(matches!(
        d.clone().event(Event::Accept),
        Reaction::Rejected(_)
    ));
    // Pulsar el botón no cierra el diálogo: la ventana lo cierra y lanza la acción.
    let reaccion = m.dialogo_mut().event(Event::Press {
        field: FIELD_FLATTEN,
        aside: false,
    });
    assert_eq!(
        reaccion,
        Reaction::Pressed {
            field: FIELD_FLATTEN,
            aside: false
        }
    );
    // Volver a otro destino lo esconde.
    m.elige_destino(Mode::GrayAlpha);
    m.sincroniza();
    assert!(!dialogo(&m.modal).is_shown(FIELD_FLATTEN));
}

#[test]
fn fila_12_indexado_con_capas_con_opacidad_dice_su_motivo() {
    let mut doc = documento(2);
    let b = doc.layers()[1].id();
    doc.apply(Command::SetLayerOpacity {
        layer: b,
        opacity: 128,
    })
    .expect("opacidad");
    let mut m = Montaje::nuevo(doc);
    m.sincroniza();
    assert_eq!(
        dialogo(&m.modal).block(),
        Some("hay capas con opacidad o mezcla")
    );
}

#[test]
fn fila_12_el_calculo_cuenta_los_pixeles_de_todas_las_celdas_y_va_en_dos_tiempos() {
    // Una capa sola de 1500 × 1500 (2,25 Mpx) se calcula en el acto; con dos capas con imagen
    // son 4,5 Mpx (> 2²²) y espera.
    let mut grande = Document::new(Image::new(1500, 1500, Mode::Rgba).expect("imagen"));
    grande
        .apply(Command::AddLayer {
            name: "B".to_string(),
            at: 1,
        })
        .expect("añadir");
    assert_eq!(
        convert_pixels(&grande),
        1500 * 1500,
        "la celda vacía no cuenta"
    );
    let image = grande.layers()[1].cel();
    grande
        .apply(Command::SetPixels {
            image,
            pixels: vec![PixelEdit {
                x: 0,
                y: 0,
                pixel: Pixel::Rgba(Rgba::opaque(1, 2, 3)),
            }],
        })
        .expect("pintar");
    assert_eq!(convert_pixels(&grande), 2 * 1500 * 1500);

    let mut m = Montaje::nuevo(grande);
    let mut primera = None;
    for fotograma in 1..=6 {
        m.sincroniza();
        if primera.is_none() && !m.cuenta.llamadas.is_empty() {
            primera = Some(fotograma);
        }
    }
    let primera = primera.expect("acaba calculando");
    assert!(
        primera > 1,
        "el primer fotograma solo avisa (llegó en el {primera})"
    );
    assert_eq!(primera, usize::from(WAIT_FRAMES) + 1);
    assert_eq!(m.cuenta.llamadas.len(), 1, "se calcula una sola vez");

    // Documento pequeño: en el primer fotograma.
    let mut pequeno = Montaje::nuevo(documento(3));
    pequeno.sincroniza();
    assert_eq!(pequeno.cuenta.llamadas.len(), 1);
}

// ---------------------------------------------------------------------------------------------
// Aplicar: un paso, deshacer, activa y paleta común
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_12_aceptar_convierte_todas_las_capas_en_un_paso_y_se_deshace() {
    let mut doc = documento(3);
    pinta(&mut doc, 1, &[(0, 0)], Rgba::opaque(255, 0, 0));
    pinta(&mut doc, 2, &[(1, 0)], Rgba::opaque(0, 0, 255));
    let mut sesion = Session::sample(doc);
    let activa = sesion.document.layers()[1].id();
    sesion.set_active_layer(activa).expect("activa");
    let antes = sesion.document.clone();
    let permitido = Allowed {
        luma: false,
        drop_alpha: None,
    };
    assert_eq!(sesion.convert_all(Mode::Indexed, permitido), Ok(true));
    assert_eq!(sesion.document.mode(), Mode::Indexed);
    let paleta = sesion.document.palette().expect("paleta").clone();
    for capa in sesion.document.layers() {
        if let Some(imagen) = sesion.document.image(capa.cel()) {
            assert_eq!(imagen.palette(), Some(&paleta), "paleta común");
        }
    }
    assert_eq!(sesion.active_layer(), activa);
    assert!(sesion.modified());
    sesion.undo().expect("deshacer");
    assert_eq!(sesion.document, antes, "un solo paso");
    assert_eq!(sesion.document.mode(), Mode::Rgba);
    let _: &[Layer] = sesion.document.layers();
}
