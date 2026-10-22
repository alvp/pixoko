//! Tests del borrador como lógica de herramienta, sin ventana (specs/hito-2-cuentagotas-
//! borrador.md, «Casos límite» filas 3, 4 y 15, y propiedad 1 y su ampliación).
//!
//! Los eventos se dan a mano a `ToolState::canvas_event_with`. El borrador ignora el `Pixel` que
//! recibe: aquí se le pasa uno que no es ni del modo de la imagen, para comprobarlo. El oráculo
//! de la propiedad 1 es un lápiz (`Tool::Pencil`) con `eraser_pixel(image)` como color y, para
//! los píxeles que cambian, un recorrido con `Image::get_pixel` y `Image::set_pixel` que no usa
//! nada de `pick`.

use std::collections::BTreeSet;

use proptest::prelude::*;
use proptest::test_runner::TestCaseError;
use px_core::{Command, Document, Image, ImageId, Mode, Palette, Pixel, PixelEdit, Rgba};
use px_ui::canvas::CanvasEvent;
use px_ui::pick::{EraseError, eraser_pixel};
use px_ui::texture::pixel_rgba;
use px_ui::tool::{Tool, ToolAction, ToolEvent, ToolState};

const MIN: i32 = i32::MIN;
const MAX: i32 = i32::MAX;

// Apoyo común.

fn rgba(r: u8, g: u8, b: u8, a: u8) -> Rgba {
    Rgba::new(r, g, b, a)
}

fn begin(x: i32, y: i32) -> CanvasEvent {
    CanvasEvent::StrokeBegin((x, y))
}

fn mov(x: i32, y: i32) -> CanvasEvent {
    CanvasEvent::StrokeMove((x, y))
}

fn fin() -> CanvasEvent {
    CanvasEvent::StrokeEnd
}

/// Imagen RGBA de 8×8 azul opaca `(0, 0, 255, 255)`.
fn rgba_azul() -> Image {
    let mut image = Image::new(8, 8, Mode::Rgba).expect("imagen válida");
    rellena(&mut image, Pixel::Rgba(Rgba::opaque(0, 0, 255)));
    image
}

/// Imagen RGBA de 8×8 vacía: todo `(0, 0, 0, 0)`.
fn rgba_transparente() -> Image {
    Image::new(8, 8, Mode::Rgba).expect("imagen válida")
}

/// Imagen de gris con alfa de 8×8, gris 100 opaco.
fn gris_alfa() -> Image {
    let mut image = Image::new(8, 8, Mode::GrayAlpha).expect("imagen válida");
    rellena(
        &mut image,
        Pixel::GrayAlpha {
            gray: 100,
            alpha: 255,
        },
    );
    image
}

/// Indexada de 8×8: índice 0 azul, índice 1 transparente; todo el lienzo vale `Indexed(0)`.
fn indexada_con_transparente() -> Image {
    let mut paleta = Palette::from_colors([Rgba::opaque(0, 0, 255), Rgba::opaque(255, 0, 255)])
        .expect("paleta válida");
    paleta.set_transparent_index(Some(1)).expect("existe");
    Image::new_indexed(8, 8, paleta).expect("imagen válida")
}

fn indexada_sin_transparente() -> Image {
    let paleta = Palette::from_colors([Rgba::opaque(0, 0, 255), Rgba::opaque(255, 0, 255)])
        .expect("paleta válida");
    Image::new_indexed(8, 8, paleta).expect("imagen válida")
}

fn gris() -> Image {
    let mut image = Image::new(8, 8, Mode::Gray).expect("imagen válida");
    rellena(&mut image, Pixel::Gray(100));
    image
}

fn rellena(image: &mut Image, pixel: Pixel) {
    for y in 0..image.height() {
        for x in 0..image.width() {
            image.set_pixel(x, y, pixel).expect("dentro");
        }
    }
}

/// Un color cualquiera, de otro modo que casi todas las imágenes: el borrador lo ignora.
fn color_de_otro_modo() -> Option<Pixel> {
    Some(Pixel::Rgba(Rgba::opaque(255, 0, 0)))
}

/// Documento y herramienta juntos, con el borrador elegido.
struct Escena {
    doc: Document,
    id: ImageId,
    tool: ToolState,
    /// El `Pixel` que recibe `canvas_event_with` (el borrador no lo usa).
    pixel: Option<Pixel>,
}

impl Escena {
    fn nueva(image: Image) -> Escena {
        Escena::con(image, Tool::Eraser)
    }

    fn con(image: Image, herramienta: Tool) -> Escena {
        let doc = Document::new(image);
        let id = ImageId::from_raw(0);
        let mut tool = ToolState::new();
        tool.apply_action(ToolAction::Select(herramienta));
        assert_eq!(tool.tool(), herramienta);
        Escena {
            doc,
            id,
            tool,
            pixel: color_de_otro_modo(),
        }
    }

    fn imagen(&self) -> &Image {
        self.doc.image(self.id).expect("hay imagen")
    }

    /// Da el evento a la herramienta; un `Err` aquí es un fallo del test.
    fn ev(&mut self, evento: CanvasEvent) -> bool {
        self.tool
            .canvas_event_with(&mut self.doc, self.id, self.pixel, evento)
            .unwrap_or_else(|e| panic!("{evento:?} dio el error {e:?}"))
    }

    fn preview(&self) -> Vec<PixelEdit> {
        self.tool.preview().collect()
    }

    fn historia(&self) -> (bool, bool, usize) {
        historia(&self.doc)
    }
}

/// `can_undo`, `can_redo` e `history_bytes`.
fn historia(doc: &Document) -> (bool, bool, usize) {
    (doc.can_undo(), doc.can_redo(), doc.history_bytes())
}

fn conjunto(puntos: impl IntoIterator<Item = (u32, u32)>) -> BTreeSet<(u32, u32)> {
    puntos.into_iter().collect()
}

/// Coordenadas de los píxeles que difieren entre dos imágenes del mismo tamaño.
fn cambiados(a: &Image, b: &Image) -> BTreeSet<(u32, u32)> {
    assert_eq!((a.width(), a.height()), (b.width(), b.height()));
    (0..a.height())
        .flat_map(|y| (0..a.width()).map(move |x| (x, y)))
        .filter(|&(x, y)| a.get_pixel(x, y).unwrap() != b.get_pixel(x, y).unwrap())
        .collect()
}

/// Los siete píxeles de la fila 3: `(1,1)…(5,1)`, `(5,2)` y `(5,3)`, en ese orden, con `pixel`.
fn los_siete(pixel: Pixel) -> Vec<PixelEdit> {
    [(1, 1), (2, 1), (3, 1), (4, 1), (5, 1), (5, 2), (5, 3)]
        .into_iter()
        .map(|(x, y)| PixelEdit { x, y, pixel })
        .collect()
}

/// El gesto de la fila 3 sin el `End`: `Begin(1,1)`, `Move(5,1)`, `Move(5,3)`.
fn gesto_de_la_fila_3(es: &mut Escena) {
    assert!(!es.ev(begin(1, 1)), "Begin no cambia el documento");
    assert!(!es.ev(mov(5, 1)), "Move no cambia el documento");
    assert!(!es.ev(mov(5, 3)), "Move no cambia el documento");
}

// Fila 3.

/// La fila 3 completa para una imagen y su valor de borrado.
fn fila_3(image: Image, borrado: Pixel) {
    let copia = image.clone();
    let mut es = Escena::nueva(image);
    let state_id = es.doc.state_id();

    // Antes del `End`: el documento no cambia y la vista previa es el trazo con el valor de borrado.
    gesto_de_la_fila_3(&mut es);
    assert_eq!(es.imagen(), &copia);
    assert!(!es.doc.can_undo());
    assert_eq!(es.doc.state_id(), state_id);
    assert!(es.tool.busy());
    assert_eq!(es.preview(), los_siete(borrado));
    for edicion in es.preview() {
        let color = pixel_rgba(&copia, edicion.pixel).expect("el valor de borrado se ve");
        assert_eq!(color.a, 0, "({}, {}) con alfa 0", edicion.x, edicion.y);
    }
    assert_eq!(
        es.tool.take_events(),
        vec![],
        "sin eventos: no hay nada que decir"
    );

    // `End`: un solo paso con esos siete píxeles.
    assert!(es.ev(fin()), "End da Ok(true)");
    assert!(!es.tool.busy());
    assert_eq!(es.preview(), vec![], "nada pendiente tras el End");
    assert_eq!(es.tool.take_events(), vec![]);
    let siete = conjunto(los_siete(borrado).iter().map(|e| (e.x, e.y)));
    assert_eq!(cambiados(es.imagen(), &copia), siete);
    for edicion in los_siete(borrado) {
        assert_eq!(es.imagen().get_pixel(edicion.x, edicion.y), Ok(borrado));
    }
    let editada = es.imagen().clone();
    assert_eq!(es.doc.undo(), Ok(()));
    assert_eq!(es.imagen(), &copia, "un solo paso");
    assert!(!es.doc.can_undo());
    assert_eq!(es.doc.redo(), Ok(()));
    assert_eq!(es.imagen(), &editada);
}

#[test]
fn fila_3_borrador_aplicado_en_rgba() {
    fila_3(rgba_azul(), Pixel::Rgba(Rgba::TRANSPARENT));
}

#[test]
fn fila_3_borrador_aplicado_en_gris_con_alfa() {
    fila_3(gris_alfa(), Pixel::GrayAlpha { gray: 0, alpha: 0 });
}

#[test]
fn fila_3_borrador_aplicado_en_indexada_con_transparente() {
    fila_3(indexada_con_transparente(), Pixel::Indexed(1));
}

#[test]
fn el_borrador_ignora_el_pixel_que_recibe() {
    // Con `None`, con un color del modo equivocado y con un índice que no existe: el mismo trazo.
    let imagenes = [
        (rgba_azul(), Pixel::Rgba(Rgba::TRANSPARENT)),
        (gris_alfa(), Pixel::GrayAlpha { gray: 0, alpha: 0 }),
        (indexada_con_transparente(), Pixel::Indexed(1)),
    ];
    let recibidos = [
        None,
        Some(Pixel::Gray(3)),
        Some(Pixel::Indexed(200)),
        Some(Pixel::Rgba(Rgba::opaque(255, 0, 0))),
        Some(Pixel::GrayAlpha {
            gray: 9,
            alpha: 255,
        }),
    ];
    for (image, borrado) in imagenes {
        let copia = image.clone();
        for recibido in recibidos {
            let mut es = Escena::nueva(image.clone());
            es.pixel = recibido;
            gesto_de_la_fila_3(&mut es);
            assert_eq!(
                es.preview(),
                los_siete(borrado),
                "{:?} con {recibido:?}",
                copia.mode()
            );
            assert!(es.ev(fin()), "{:?} con {recibido:?}", copia.mode());
            assert_eq!(
                cambiados(es.imagen(), &copia).len(),
                7,
                "{:?} con {recibido:?}",
                copia.mode()
            );
            for edicion in los_siete(borrado) {
                assert_eq!(es.imagen().get_pixel(edicion.x, edicion.y), Ok(borrado));
            }
        }
    }
}

#[test]
fn el_borrador_puede_variar_el_pixel_entre_eventos_sin_efecto() {
    // `Begin` fija el valor de borrado; lo que llegue después en `pixel` no cuenta.
    let copia = rgba_azul();
    let mut es = Escena::nueva(copia.clone());
    es.pixel = Some(Pixel::Gray(1));
    assert!(!es.ev(begin(1, 1)));
    es.pixel = None;
    assert!(!es.ev(mov(5, 1)));
    es.pixel = Some(Pixel::Indexed(7));
    assert!(!es.ev(mov(5, 3)));
    assert_eq!(es.preview(), los_siete(Pixel::Rgba(Rgba::TRANSPARENT)));
    es.pixel = color_de_otro_modo();
    assert!(es.ev(fin()));
    assert_eq!(cambiados(es.imagen(), &copia).len(), 7);
}

#[test]
fn un_clic_sin_movimiento_borra_un_pixel() {
    let copia = rgba_azul();
    let mut es = Escena::nueva(copia.clone());
    assert!(!es.ev(begin(2, 3)));
    assert_eq!(es.preview().len(), 1);
    assert!(es.ev(fin()));
    assert_eq!(cambiados(es.imagen(), &copia), conjunto([(2, 3)]));
    assert_eq!(
        es.imagen().get_pixel(2, 3),
        Ok(Pixel::Rgba(Rgba::TRANSPARENT))
    );
    assert_eq!(es.doc.undo(), Ok(()));
    assert_eq!(es.imagen(), &copia);
}

#[test]
fn el_trazo_del_borrador_se_recorta_al_lienzo() {
    let copia = rgba_azul();
    let mut es = Escena::nueva(copia.clone());
    assert!(!es.ev(begin(-3, 2)));
    assert!(!es.ev(mov(3, 2)));
    assert!(es.ev(fin()));
    assert_eq!(
        cambiados(es.imagen(), &copia),
        conjunto((0..=3).map(|x| (x, 2)))
    );
}

#[test]
fn un_trazo_que_sale_y_vuelve_une_los_extremos_como_el_lapiz() {
    // Mismas posiciones que el lápiz: la parte de fuera se recorta y no se «cierra» por dentro.
    let copia = rgba_azul();
    let mut borrador = Escena::nueva(copia.clone());
    let mut lapiz = Escena::con(copia.clone(), Tool::Pencil);
    lapiz.pixel = Some(Pixel::Rgba(Rgba::TRANSPARENT));
    for evento in [begin(6, 2), mov(20, 2), mov(6, 4), fin()] {
        borrador.ev(evento);
        lapiz.ev(evento);
        assert_eq!(borrador.preview(), lapiz.preview(), "{evento:?}");
    }
    assert_eq!(borrador.imagen(), lapiz.imagen());
    assert_eq!(
        cambiados(borrador.imagen(), &copia),
        conjunto([(6, 2), (7, 2), (7, 4), (6, 4)])
    );
}

#[test]
fn dos_trazos_son_dos_pasos_y_un_undo_deshace_solo_el_ultimo() {
    let copia = rgba_azul();
    let mut es = Escena::nueva(copia.clone());
    es.ev(begin(0, 0));
    es.ev(mov(2, 0));
    assert!(es.ev(fin()));
    let primero = es.imagen().clone();
    es.ev(begin(0, 5));
    es.ev(mov(2, 5));
    assert!(es.ev(fin()));
    assert_eq!(es.doc.undo(), Ok(()));
    assert_eq!(es.imagen(), &primero);
    assert_eq!(es.doc.undo(), Ok(()));
    assert_eq!(es.imagen(), &copia);
}

// Fila 4.

#[test]
fn fila_4a_una_zona_ya_transparente_no_cambia_nada_ni_crea_paso() {
    // `(255, 0, 0, 0)` es lo habitual en PNG reales y no se limpia.
    let mut image = rgba_transparente();
    image
        .set_pixel(3, 2, Pixel::Rgba(rgba(255, 0, 0, 0)))
        .expect("dentro");
    let copia = image.clone();
    let mut es = Escena::nueva(image);
    let antes = es.historia();
    let state_id = es.doc.state_id();

    assert!(!es.ev(begin(2, 2)));
    assert!(!es.ev(mov(4, 2)));
    assert_eq!(
        es.preview().len(),
        3,
        "la vista previa muestra todo el trazo"
    );
    assert!(!es.ev(fin()), "Ok(false)");
    assert_eq!(es.imagen(), &copia);
    assert_eq!(
        es.imagen().get_pixel(3, 2),
        Ok(Pixel::Rgba(rgba(255, 0, 0, 0))),
        "el RGB oculto sigue siendo 255, 0, 0"
    );
    assert_eq!(es.historia(), antes);
    assert_eq!(es.doc.state_id(), state_id);
    assert!(!es.doc.can_undo());
}

#[test]
fn fila_4a_gris_con_alfa_ya_transparente_no_cambia_nada() {
    let mut image = Image::new(8, 8, Mode::GrayAlpha).expect("imagen válida");
    image
        .set_pixel(3, 2, Pixel::GrayAlpha { gray: 7, alpha: 0 })
        .expect("dentro");
    let copia = image.clone();
    let mut es = Escena::nueva(image);
    es.ev(begin(2, 2));
    es.ev(mov(4, 2));
    assert!(!es.ev(fin()));
    assert_eq!(es.imagen(), &copia);
    assert!(!es.doc.can_undo());
}

#[test]
fn fila_4a_indexada_con_los_pixeles_ya_transparentes_no_cambia_nada() {
    let mut image = indexada_con_transparente();
    for x in 2..=4 {
        image.set_pixel(x, 2, Pixel::Indexed(1)).expect("dentro");
    }
    let copia = image.clone();
    let mut es = Escena::nueva(image);
    es.ev(begin(2, 2));
    es.ev(mov(4, 2));
    assert!(!es.ev(fin()));
    assert_eq!(es.imagen(), &copia);
    assert!(!es.doc.can_undo());
}

#[test]
fn en_un_trazo_mixto_solo_cambian_los_pixeles_que_no_eran_transparentes() {
    // (2,2) opaco, (3,2) `(255, 0, 0, 0)`, (4,2) `(0, 0, 0, 0)`, (5,2) con alfa 1.
    let mut image = rgba_transparente();
    image
        .set_pixel(2, 2, Pixel::Rgba(Rgba::opaque(0, 0, 255)))
        .expect("dentro");
    image
        .set_pixel(3, 2, Pixel::Rgba(rgba(255, 0, 0, 0)))
        .expect("dentro");
    image
        .set_pixel(5, 2, Pixel::Rgba(rgba(9, 9, 9, 1)))
        .expect("dentro");
    let copia = image.clone();
    let mut es = Escena::nueva(image);
    es.ev(begin(2, 2));
    es.ev(mov(5, 2));
    assert!(es.ev(fin()), "cambian dos píxeles");
    assert_eq!(cambiados(es.imagen(), &copia), conjunto([(2, 2), (5, 2)]));
    assert_eq!(
        es.imagen().get_pixel(3, 2),
        Ok(Pixel::Rgba(rgba(255, 0, 0, 0))),
        "el transparente con RGB oculto no se toca"
    );
    assert_eq!(es.doc.undo(), Ok(()));
    assert_eq!(es.imagen(), &copia);
    assert!(!es.doc.can_undo());
}

#[test]
fn fila_4b_tras_un_undo_el_borrador_sin_efecto_no_borra_el_rehacer() {
    let mut es = Escena::nueva(rgba_transparente());
    let copia = es.imagen().clone();
    // Un paso previo, deshecho: hay rehacer.
    let paso = Command::SetPixels {
        image: es.id,
        pixels: vec![PixelEdit {
            x: 0,
            y: 0,
            pixel: Pixel::Rgba(Rgba::opaque(1, 2, 3)),
        }],
    };
    assert_eq!(es.doc.apply(paso), Ok(true));
    assert_eq!(es.doc.undo(), Ok(()));
    assert!(es.doc.can_redo());
    assert_eq!(es.imagen(), &copia);
    let antes = es.historia();

    es.ev(begin(2, 2));
    es.ev(mov(4, 2));
    assert!(!es.ev(fin()));
    assert_eq!(es.imagen(), &copia);
    assert_eq!(es.historia(), antes);
    assert!(es.doc.can_redo(), "el rehacer sigue disponible");
}

/// La fila 4c: el `Begin` no abre gesto, avisa una sola vez y el gesto queda en espera.
fn fila_4c(image: Image, error: EraseError) {
    let copia = image.clone();
    let mut es = Escena::nueva(image);
    let antes = es.historia();
    let state_id = es.doc.state_id();
    assert_eq!(eraser_pixel(&copia), Err(error));

    assert!(!es.ev(begin(2, 2)), "Begin: Ok(false)");
    assert_eq!(
        es.tool.take_events(),
        vec![ToolEvent::Refused(error)],
        "un solo Refused, en el Begin"
    );
    assert!(es.tool.busy(), "busy desde el Begin");
    assert_eq!(es.preview(), vec![], "sin vista previa");

    assert!(!es.ev(mov(4, 2)));
    assert!(es.tool.busy(), "busy hasta el End");
    assert_eq!(es.preview(), vec![]);
    assert_eq!(es.tool.take_events(), vec![], "no se vuelve a avisar");

    assert!(!es.ev(fin()), "Ok(false)");
    assert!(!es.tool.busy());
    assert_eq!(es.tool.take_events(), vec![], "ni en el End");
    assert_eq!(es.imagen(), &copia);
    assert_eq!(es.historia(), antes);
    assert_eq!(es.doc.state_id(), state_id);
}

#[test]
fn fila_4c_indexada_sin_transparente_avisa_y_no_cambia_nada() {
    fila_4c(indexada_sin_transparente(), EraseError::NoTransparentIndex);
}

#[test]
fn fila_4c_gris_avisa_y_no_cambia_nada() {
    fila_4c(gris(), EraseError::NoTransparency);
}

#[test]
fn fila_4c_cada_pulsacion_avisa_una_vez() {
    let mut es = Escena::nueva(gris());
    for _ in 0..3 {
        assert!(!es.ev(begin(2, 2)));
        assert!(!es.ev(mov(3, 2)));
        assert!(!es.ev(fin()));
        assert_eq!(
            es.tool.take_events(),
            vec![ToolEvent::Refused(EraseError::NoTransparency)]
        );
    }
}

#[test]
fn fila_4c_un_begin_con_el_gesto_en_espera_no_avisa_otra_vez() {
    let mut es = Escena::nueva(gris());
    assert!(!es.ev(begin(2, 2)));
    assert_eq!(es.tool.take_events().len(), 1);
    // `Canvas` no emite dos `Begin` seguidos, pero si llegara, el gesto sigue siendo el mismo.
    assert!(!es.ev(begin(3, 3)));
    assert_eq!(es.tool.take_events(), vec![]);
    assert!(es.tool.busy());
    assert!(!es.ev(fin()));
    assert!(!es.tool.busy());
}

#[test]
fn con_el_borrador_sin_poder_se_puede_cambiar_de_herramienta_tras_el_end() {
    let mut es = Escena::nueva(gris());
    es.ev(begin(2, 2));
    es.tool.apply_action(ToolAction::Select(Tool::Pencil));
    assert_eq!(es.tool.tool(), Tool::Eraser, "busy hasta el End");
    es.ev(fin());
    es.tool.apply_action(ToolAction::Select(Tool::Pencil));
    assert_eq!(es.tool.tool(), Tool::Pencil);
}

#[test]
fn el_borrador_no_deja_eventos_si_puede_borrar() {
    for image in [rgba_azul(), gris_alfa(), indexada_con_transparente()] {
        let mut es = Escena::nueva(image);
        es.ev(begin(1, 1));
        es.ev(mov(5, 1));
        es.ev(fin());
        assert_eq!(es.tool.take_events(), vec![]);
    }
}

// Estrategias de la propiedad 1.

/// Valores de un fondo con transparentes de todas clases: alfa 0 con RGB oculto, alfa 1, opaco.
fn pool(mode: Mode) -> Vec<Pixel> {
    match mode {
        Mode::Rgba => vec![
            Pixel::Rgba(rgba(10, 20, 30, 255)),
            Pixel::Rgba(rgba(255, 0, 0, 0)),
            Pixel::Rgba(rgba(0, 0, 0, 0)),
            Pixel::Rgba(rgba(9, 9, 9, 1)),
            Pixel::Rgba(rgba(40, 50, 60, 128)),
            Pixel::Rgba(rgba(0, 0, 0, 255)),
        ],
        Mode::GrayAlpha => vec![
            Pixel::GrayAlpha {
                gray: 5,
                alpha: 255,
            },
            Pixel::GrayAlpha { gray: 5, alpha: 0 },
            Pixel::GrayAlpha { gray: 0, alpha: 0 },
            Pixel::GrayAlpha { gray: 9, alpha: 1 },
            Pixel::GrayAlpha {
                gray: 7,
                alpha: 128,
            },
        ],
        Mode::Indexed => (0..6).map(Pixel::Indexed).collect(),
        Mode::Gray => vec![Pixel::Gray(1), Pixel::Gray(2)],
    }
}

/// Cualquier `i32`, con el interior de un lienzo de hasta 16 de lado y los extremos siempre
/// presentes.
fn coordenada() -> impl Strategy<Value = i32> {
    prop_oneof![
        6 => -2i32..=17,
        2 => any::<i32>(),
        1 => Just(MIN),
        1 => Just(MAX),
        1 => Just(MIN + 1),
        1 => Just(MAX - 1),
    ]
}

fn posicion() -> impl Strategy<Value = (i32, i32)> {
    (coordenada(), coordenada())
}

/// Un paso previo en la historia: `0` nada, `1` un paso, `2` un paso deshecho (hay rehacer).
fn paso_previo(es: &mut Escena, variante: u8) {
    if variante == 0 {
        return;
    }
    let actual = es.imagen().get_pixel(0, 0).unwrap();
    let otro = pool(es.imagen().mode())
        .into_iter()
        .find(|&p| p != actual)
        .expect("el fondo tiene al menos dos valores");
    let comando = Command::SetPixels {
        image: es.id,
        pixels: vec![PixelEdit {
            x: 0,
            y: 0,
            pixel: otro,
        }],
    };
    assert_eq!(es.doc.apply(comando), Ok(true));
    if variante == 2 {
        assert_eq!(es.doc.undo(), Ok(()));
    }
}

// Fila 15: borrar y omitir con la regla de la entrada de alfa 0.
//
// Paleta de 5: `[azul, rojo, (0,255,0,α0), azul claro, (9,9,9,α0)]` (la de la fila 14). Fila de 6
// píxeles `(0,0)…(5,0)` con `Indexed(0)`, `(2)`, `(4)`, `(1)`, `(3)`, `(0)`; el resto, `Indexed(0)`.

fn paleta_fila_15(transparente: Option<u8>) -> Palette {
    let mut paleta = Palette::from_colors([
        Rgba::opaque(0, 0, 255),
        Rgba::opaque(255, 0, 0),
        rgba(0, 255, 0, 0),
        Rgba::opaque(100, 150, 255),
        rgba(9, 9, 9, 0),
    ])
    .expect("paleta válida");
    paleta
        .set_transparent_index(transparente)
        .expect("el índice existe");
    paleta
}

fn imagen_fila_15(transparente: Option<u8>) -> Image {
    let mut image = Image::new_indexed(8, 8, paleta_fila_15(transparente)).expect("imagen válida");
    for (x, indice) in [0u8, 2, 4, 1, 3, 0].into_iter().enumerate() {
        image
            .set_pixel(u32::try_from(x).unwrap(), 0, Pixel::Indexed(indice))
            .expect("dentro");
    }
    image
}

/// Las seis posiciones del trazo `(0,0)…(5,0)`, con `pixel`.
fn la_fila(pixel: Pixel) -> Vec<PixelEdit> {
    (0..6).map(|x| PixelEdit { x, y: 0, pixel }).collect()
}

/// Un paso previo, deshecho: hay rehacer y la imagen es la de antes.
fn con_un_undo_previo(es: &mut Escena) {
    let copia = es.imagen().clone();
    let paso = Command::SetPixels {
        image: es.id,
        pixels: vec![PixelEdit {
            x: 7,
            y: 7,
            pixel: Pixel::Indexed(1),
        }],
    };
    assert_eq!(es.doc.apply(paso), Ok(true));
    assert_eq!(es.doc.undo(), Ok(()));
    assert!(es.doc.can_redo());
    assert_eq!(es.imagen(), &copia);
}

#[test]
fn fila_15a_sin_transparent_index_borra_a_la_entrada_2_y_omite_las_de_alfa_0() {
    let copia = imagen_fila_15(None);
    let mut es = Escena::nueva(copia.clone());
    let state_id = es.doc.state_id();

    // Antes del `End`: el documento no cambia y la vista previa es el trazo entero.
    assert!(!es.ev(begin(0, 0)));
    assert!(!es.ev(mov(5, 0)));
    assert_eq!(es.preview(), la_fila(Pixel::Indexed(2)), "trazo entero");
    assert_eq!(es.imagen(), &copia);
    assert_eq!(es.doc.state_id(), state_id);
    assert!(!es.doc.can_undo());
    assert!(es.tool.take_events().is_empty(), "no hay Refused");

    assert!(es.ev(fin()), "Ok(true)");
    assert_eq!(
        cambiados(es.imagen(), &copia),
        conjunto([(0, 0), (3, 0), (4, 0), (5, 0)])
    );
    for x in [0, 3, 4, 5] {
        assert_eq!(
            es.imagen().get_pixel(x, 0),
            Ok(Pixel::Indexed(2)),
            "({x}, 0)"
        );
    }
    // Las entradas 2 y 4 (alfa 0) no se tocan: el `Indexed(4)` no pasa a 2.
    assert_eq!(es.imagen().get_pixel(1, 0), Ok(Pixel::Indexed(2)));
    assert_eq!(es.imagen().get_pixel(2, 0), Ok(Pixel::Indexed(4)));
    assert!(es.tool.take_events().is_empty());

    // Un solo paso.
    let editada = es.imagen().clone();
    assert_eq!(es.doc.undo(), Ok(()));
    assert_eq!(es.imagen(), &copia);
    assert!(!es.doc.can_undo(), "un solo paso");
    assert_eq!(es.doc.redo(), Ok(()));
    assert_eq!(es.imagen(), &editada);
}

#[test]
fn fila_15b_con_transparent_index_3_borra_a_3_y_omite_alfa_0_y_el_propio_3() {
    let copia = imagen_fila_15(Some(3));
    let mut es = Escena::nueva(copia.clone());
    let state_id = es.doc.state_id();

    assert!(!es.ev(begin(0, 0)));
    assert!(!es.ev(mov(5, 0)));
    assert_eq!(es.preview(), la_fila(Pixel::Indexed(3)), "trazo entero");
    assert_eq!(es.imagen(), &copia);
    assert_eq!(es.doc.state_id(), state_id);
    assert!(!es.doc.can_undo());

    assert!(es.ev(fin()), "Ok(true)");
    // (1,0) es la entrada 2 y (2,0) la 4 (alfa 0 guardado); (4,0) ya es `Indexed(3)`.
    assert_eq!(
        cambiados(es.imagen(), &copia),
        conjunto([(0, 0), (3, 0), (5, 0)])
    );
    for x in [0, 3, 5] {
        assert_eq!(
            es.imagen().get_pixel(x, 0),
            Ok(Pixel::Indexed(3)),
            "({x}, 0)"
        );
    }
    assert_eq!(es.imagen().get_pixel(1, 0), Ok(Pixel::Indexed(2)));
    assert_eq!(es.imagen().get_pixel(2, 0), Ok(Pixel::Indexed(4)));
    assert_eq!(es.imagen().get_pixel(4, 0), Ok(Pixel::Indexed(3)));

    let editada = es.imagen().clone();
    assert_eq!(es.doc.undo(), Ok(()));
    assert_eq!(es.imagen(), &copia);
    assert!(!es.doc.can_undo(), "un solo paso");
    assert_eq!(es.doc.redo(), Ok(()));
    assert_eq!(es.imagen(), &editada);
}

#[test]
fn fila_15c_un_trazo_solo_sobre_alfa_0_no_cambia_nada_no_crea_paso_y_no_avisa() {
    let copia = imagen_fila_15(None);
    let mut es = Escena::nueva(copia.clone());
    let antes = es.historia();
    let state_id = es.doc.state_id();

    assert!(!es.ev(begin(1, 0)));
    assert!(!es.ev(mov(2, 0)));
    assert_eq!(
        es.preview(),
        vec![
            PixelEdit {
                x: 1,
                y: 0,
                pixel: Pixel::Indexed(2)
            },
            PixelEdit {
                x: 2,
                y: 0,
                pixel: Pixel::Indexed(2)
            },
        ],
        "la vista previa muestra todo el trazo"
    );
    assert!(!es.ev(fin()), "Ok(false)");
    assert!(!es.tool.busy());
    assert_eq!(es.imagen(), &copia);
    assert_eq!(es.historia(), antes);
    assert_eq!(es.doc.state_id(), state_id);
    assert!(!es.doc.can_undo());
    assert_eq!(es.tool.take_events(), vec![], "sin Refused");
}

#[test]
fn fila_15c_con_transparent_index_tambien_se_omiten_las_de_alfa_0_sin_avisar() {
    let copia = imagen_fila_15(Some(3));
    let mut es = Escena::nueva(copia.clone());
    let antes = es.historia();
    // (1,0) entrada 2, (2,0) entrada 4 y (4,0) `Indexed(3)`: los tres ya son transparentes.
    for (a, b) in [((1, 0), (2, 0)), ((4, 0), (4, 0))] {
        es.ev(begin(a.0, a.1));
        es.ev(mov(b.0, b.1));
        assert!(!es.ev(fin()), "{a:?}-{b:?}");
        assert_eq!(es.imagen(), &copia);
        assert_eq!(es.historia(), antes);
        assert_eq!(es.tool.take_events(), vec![]);
    }
}

#[test]
fn fila_15d_con_un_undo_previo_el_trazo_sin_efecto_no_borra_el_rehacer() {
    let mut es = Escena::nueva(imagen_fila_15(None));
    con_un_undo_previo(&mut es);
    let copia = es.imagen().clone();
    let antes = es.historia();

    es.ev(begin(1, 0));
    es.ev(mov(2, 0));
    assert!(!es.ev(fin()), "Ok(false)");
    assert_eq!(es.imagen(), &copia);
    assert_eq!(es.historia(), antes);
    assert!(es.doc.can_redo(), "el rehacer sigue disponible");
    assert_eq!(es.tool.take_events(), vec![], "sin Refused");
}

#[test]
fn fila_15_el_valor_se_recalcula_en_cada_pulsacion() {
    // Tras editar la paleta (la entrada 2 pasa a opaca) el siguiente trazo escribe la entrada 4.
    let mut es = Escena::nueva(imagen_fila_15(None));
    let paso = Command::SetPaletteColor {
        image: es.id,
        index: 2,
        color: Rgba::opaque(0, 255, 0),
    };
    assert_eq!(es.doc.apply(paso), Ok(true));

    es.ev(begin(0, 0));
    assert_eq!(
        es.preview(),
        vec![PixelEdit {
            x: 0,
            y: 0,
            pixel: Pixel::Indexed(4)
        }]
    );
    es.ev(fin());
    assert_eq!(es.imagen().get_pixel(0, 0), Ok(Pixel::Indexed(4)));
    // La entrada 2 ya es opaca: el `Indexed(2)` de (1,0) se borra; el `Indexed(4)` de (2,0), no.
    es.ev(begin(1, 0));
    es.ev(mov(2, 0));
    assert!(es.ev(fin()));
    assert_eq!(es.imagen().get_pixel(1, 0), Ok(Pixel::Indexed(4)));
    assert_eq!(es.imagen().get_pixel(2, 0), Ok(Pixel::Indexed(4)));
}

// Propiedad 1: el borrador es un lápiz transparente, en RGBA, GrayAlpha e indexada con o sin
// `transparent_index` y con entradas de alfa 0 (o sin ellas). El oráculo de «ya transparente» es el
// valor resuelto con alfa 0 (`pixel_rgba`), no el índice; el valor de borrado se calcula aparte de
// `eraser_pixel`, con las entradas guardadas.

/// Paleta de seis entradas: las de `mascara` (bit `i` para la entrada `i`) con alfa guardado 0 y
/// el resto opacas; `transparente` fijado o no. Sin `transparente`, como mínimo la entrada `k`
/// lleva alfa 0 (se fuerza).
fn paleta_variada(transparente: Option<u8>, mascara: u8, k: u8) -> Palette {
    let mascara = if transparente.is_none() {
        mascara | (1 << k)
    } else {
        mascara
    };
    let mut paleta = Palette::from_colors((0..6u8).map(|i| {
        let alfa = if mascara & (1 << i) != 0 { 0 } else { 255 };
        rgba(10 * i, 20, 30, alfa)
    }))
    .expect("paleta válida");
    paleta.set_transparent_index(transparente).expect("existe");
    paleta
}

/// Parámetros de una paleta indexada: `transparent_index` (o no), máscara de alfa 0 y `k`.
fn paleta_indexada() -> impl Strategy<Value = Palette> {
    (
        prop_oneof![
            1 => Just(None),
            2 => (0u8..6).prop_map(Some),
        ],
        prop_oneof![
            // Mitad de los casos: ninguna entrada extra de alfa 0, la propiedad de antes.
            1 => Just(0u8),
            3 => 0u8..64,
        ],
        0u8..6,
    )
        .prop_map(|(transparente, mascara, k)| paleta_variada(transparente, mascara, k))
}

/// Imagen de 1 a 16 de lado en uno de los tres modos con borrado, con fondo variado; en indexada,
/// con la paleta de `paleta_indexada` (los seis índices aparecen en el fondo).
fn imagenes() -> impl Strategy<Value = Image> {
    (
        prop_oneof![
            1 => Just(Mode::Rgba),
            1 => Just(Mode::GrayAlpha),
            4 => Just(Mode::Indexed),
        ],
        1u32..=16,
        1u32..=16,
        paleta_indexada(),
    )
        .prop_flat_map(|(mode, w, h, paleta)| {
            let celdas = usize::try_from(w * h).unwrap();
            (
                Just((mode, w, h, paleta)),
                prop::collection::vec(any::<u8>(), celdas),
            )
        })
        .prop_map(|((mode, w, h, paleta), celdas)| {
            let pool = pool(mode);
            let mut image = match mode {
                Mode::Indexed => Image::new_indexed(w, h, paleta).unwrap(),
                _ => Image::new(w, h, mode).unwrap(),
            };
            for y in 0..h {
                for x in 0..w {
                    let byte = celdas[usize::try_from(y * w + x).unwrap()];
                    // Una parte de las casillas valen el primer valor del fondo.
                    let k = if byte < 64 {
                        0
                    } else {
                        usize::from(byte) % pool.len()
                    };
                    image.set_pixel(x, y, pool[k]).unwrap();
                }
            }
            image
        })
}

/// Lo que debe escribir el borrador, escrito aparte de `eraser_pixel` y sin `Palette::get`: el
/// índice transparente si lo hay; si no, la primera entrada de `colors()` con alfa 0. `None` si el
/// borrador no puede con la imagen (indexada sin ninguna de las dos cosas).
fn borrado_esperado(image: &Image) -> Option<Pixel> {
    match image.mode() {
        Mode::Rgba => Some(Pixel::Rgba(Rgba::TRANSPARENT)),
        Mode::GrayAlpha => Some(Pixel::GrayAlpha { gray: 0, alpha: 0 }),
        Mode::Indexed => {
            let paleta = image.palette().expect("indexada con paleta");
            paleta
                .transparent_index()
                .or_else(|| {
                    paleta
                        .colors()
                        .iter()
                        .position(|color| color.a == 0)
                        .map(|i| u8::try_from(i).unwrap())
                })
                .map(Pixel::Indexed)
        }
        Mode::Gray => unreachable!("la propiedad no usa Gray"),
    }
}

/// El píxel ya es transparente: su valor resuelto tiene alfa 0, sea cual sea su índice.
fn ya_es_transparente(image: &Image, pixel: Pixel) -> bool {
    pixel_rgba(image, pixel).expect("el píxel se ve").a == 0
}

fn comprueba_propiedad_1(
    image: Image,
    previo: u8,
    a: (i32, i32),
    movimientos: &[(i32, i32)],
) -> Result<(), TestCaseError> {
    let borrado = borrado_esperado(&image);
    // Las imágenes de esta propiedad siempre admiten el borrado: la paleta lleva índice
    // transparente o una entrada de alfa 0.
    let borrado = borrado.expect("la imagen admite el borrado");
    prop_assert_eq!(eraser_pixel(&image), Ok(borrado));

    let mut borrador = Escena::nueva(image.clone());
    let mut lapiz = Escena::con(image, Tool::Pencil);
    lapiz.pixel = Some(borrado);
    paso_previo(&mut borrador, previo);
    paso_previo(&mut lapiz, previo);
    let original = borrador.imagen().clone();
    let historia0 = borrador.historia();
    let state_id0 = borrador.doc.state_id();

    let mut eventos = vec![begin(a.0, a.1)];
    eventos.extend(movimientos.iter().map(|&(x, y)| mov(x, y)));
    for evento in eventos {
        prop_assert!(!borrador.ev(evento), "{:?} no cambia el documento", evento);
        prop_assert!(!lapiz.ev(evento));
        // Mismas posiciones que el lápiz con `eraser_pixel`, todas con el valor de borrado.
        let vista = borrador.preview();
        prop_assert_eq!(&vista, &lapiz.preview(), "{:?}", evento);
        for edicion in &vista {
            prop_assert_eq!(edicion.pixel, borrado);
        }
        // El documento no cambia antes del `End`.
        prop_assert_eq!(borrador.imagen(), &original);
        prop_assert_eq!(borrador.historia(), historia0);
        prop_assert_eq!(borrador.doc.state_id(), state_id0);
        prop_assert!(borrador.tool.busy());
        prop_assert!(borrador.tool.take_events().is_empty());
    }

    let visitados: Vec<(u32, u32)> = borrador.preview().iter().map(|e| (e.x, e.y)).collect();
    let mut esperada = original.clone();
    for &(x, y) in &visitados {
        if !ya_es_transparente(&original, original.get_pixel(x, y).unwrap()) {
            esperada.set_pixel(x, y, borrado).unwrap();
        }
    }

    let cambio = borrador.ev(fin());
    lapiz.ev(fin());
    prop_assert!(!borrador.tool.busy());
    prop_assert!(borrador.preview().is_empty());
    prop_assert!(borrador.tool.take_events().is_empty(), "sin Refused");
    prop_assert_eq!(
        borrador.imagen(),
        &esperada,
        "los que no eran transparentes, borrados"
    );
    prop_assert_eq!(
        cambio,
        esperada != original,
        "Ok(true) si y solo si alguno cambió"
    );
    if cambio {
        // Es un paso y `undo` deja la imagen exacta.
        prop_assert!(borrador.doc.can_undo());
        prop_assert_eq!(borrador.doc.undo(), Ok(()));
        prop_assert_eq!(borrador.imagen(), &original);
        prop_assert_eq!(borrador.doc.can_undo(), historia0.0, "un solo paso");
        prop_assert_eq!(borrador.doc.redo(), Ok(()));
        prop_assert_eq!(borrador.imagen(), &esperada);
    } else {
        prop_assert_eq!(
            borrador.historia(),
            historia0,
            "sin paso y sin tocar el rehacer"
        );
        prop_assert_eq!(borrador.doc.state_id(), state_id0);
    }
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn propiedad_1_el_borrador_es_un_lapiz_transparente(
        image in imagenes(),
        previo in 0u8..=2,
        a in posicion(),
        movimientos in prop::collection::vec(posicion(), 0..=4),
    ) {
        comprueba_propiedad_1(image, previo, a, &movimientos)?;
    }
}

/// Un fondo de las paletas de la propiedad 1: cubre a mano lo que el azar puede tardar en dar.
#[test]
fn propiedad_1_ejemplo_sin_transparent_index_con_la_entrada_1_de_alfa_0() {
    let paleta = paleta_variada(None, 0b000010, 1);
    let mut image = Image::new_indexed(4, 1, paleta).expect("imagen válida");
    for (x, i) in [0u8, 1, 2, 5].into_iter().enumerate() {
        image
            .set_pixel(u32::try_from(x).unwrap(), 0, Pixel::Indexed(i))
            .unwrap();
    }
    // El valor de borrado es `Indexed(1)`; el píxel `Indexed(1)` se omite y los demás pasan a 1.
    let mut esperada = image.clone();
    for x in [0, 2, 3] {
        esperada.set_pixel(x, 0, Pixel::Indexed(1)).unwrap();
    }
    let mut es = Escena::nueva(image);
    es.ev(begin(0, 0));
    es.ev(mov(3, 0));
    assert!(es.ev(fin()));
    assert_eq!(es.imagen(), &esperada);
}
