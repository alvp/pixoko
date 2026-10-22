//! Tests de `SlotColors`, `SlotBank`, las variantes `_with` de `color` y de `ToolState`
//! (specs/hito-2-editar-color.md, unidad 11c, decisión 2 y «Piezas»).
//!
//! - `SlotColors::defaults(modo)` da lo mismo que `pixel_for_slot` en cada modo y ranura 1 a 9:
//!   con esos valores, las funciones `_with` coinciden con las de siempre (`pixel_for_choice`,
//!   `cell_color`, `is_transparent_cell`, `cell_label` y `choice_label`).
//! - `get` y `set` (las ranuras 0 y 10 no existen), `SlotBank` (una tabla por modo,
//!   independientes; indexadas comparte la de RGBA).
//! - Una ranura editada en RGBA, Gray y GrayAlpha llega a `pixel_for_choice_with`, a las
//!   etiquetas y a las muestras.
//! - `ToolState::canvas_event_with` y `canvas_input_with` dibujan el `Pixel` dado (lápiz, línea
//!   y relleno) y, con `None`, no empiezan gesto.

use px_core::{Document, Image, ImageId, Mode, Palette, Pixel, Rgba};
use px_ui::canvas::{CanvasEvent, CanvasInput};
use px_ui::color::{
    ColorChoice, SlotBank, SlotColors, cell_color, cell_color_with, cell_count, cell_label,
    cell_label_with, choice_label, choice_label_with, is_transparent_cell,
    is_transparent_cell_with, pixel_for_choice, pixel_for_choice_with, pixel_for_slot,
};
use px_ui::tool::{Tool, ToolAction, ToolState};

const MODOS: [Mode; 4] = [Mode::Rgba, Mode::Indexed, Mode::Gray, Mode::GrayAlpha];

fn rgba(r: u8, g: u8, b: u8, a: u8) -> Rgba {
    Rgba::new(r, g, b, a)
}

/// Color que no es ninguno de los de las ranuras por defecto.
fn propio() -> Rgba {
    Rgba::opaque(12, 34, 56)
}

/// Indexada de 9 entradas opacas y distintas (todas las ranuras valen).
fn indexada_9() -> Image {
    let colores: Vec<Rgba> = (0..9u8)
        .map(|i| Rgba::opaque(i * 20, 255 - i * 20, 7))
        .collect();
    let paleta = Palette::from_colors(colores).expect("paleta válida");
    Image::new_indexed(8, 8, paleta).expect("imagen válida")
}

/// Indexada de 3 entradas, con la 1 transparente.
fn indexada_3_con_transparente() -> Image {
    let mut paleta = Palette::from_colors([
        Rgba::opaque(10, 10, 10),
        Rgba::opaque(20, 20, 20),
        Rgba::opaque(30, 30, 30),
    ])
    .expect("paleta válida");
    paleta
        .set_transparent_index(Some(1))
        .expect("índice válido");
    Image::new_indexed(8, 8, paleta).expect("imagen válida")
}

fn imagen(modo: Mode) -> Image {
    Image::new(8, 8, modo).expect("imagen válida")
}

/// Todas las imágenes de prueba, una por modo (y dos indexadas).
fn imagenes() -> Vec<Image> {
    vec![
        imagen(Mode::Rgba),
        imagen(Mode::Gray),
        imagen(Mode::GrayAlpha),
        indexada_9(),
        indexada_3_con_transparente(),
    ]
}

fn opciones() -> Vec<ColorChoice> {
    let mut opciones: Vec<ColorChoice> = (0..=10).map(ColorChoice::Slot).collect();
    opciones.extend((0..=10).map(ColorChoice::Entry));
    opciones.push(ColorChoice::Entry(255));
    opciones
}

// Valores por defecto.

#[test]
fn los_valores_por_defecto_de_rgba() {
    let tabla = SlotColors::defaults(Mode::Rgba);
    let esperado = [
        Rgba::opaque(0, 0, 0),
        Rgba::opaque(255, 255, 255),
        Rgba::opaque(255, 0, 0),
        Rgba::opaque(0, 255, 0),
        Rgba::opaque(0, 0, 255),
        Rgba::opaque(255, 255, 0),
        Rgba::opaque(255, 0, 255),
        Rgba::opaque(0, 255, 255),
        Rgba::TRANSPARENT,
    ];
    for (i, color) in esperado.into_iter().enumerate() {
        let ranura = i as u8 + 1;
        assert_eq!(tabla.get(ranura), Some(color), "ranura {ranura}");
    }
}

#[test]
fn los_valores_por_defecto_de_gray_y_gray_alpha() {
    let gris = SlotColors::defaults(Mode::Gray);
    let con_alfa = SlotColors::defaults(Mode::GrayAlpha);
    for ranura in 1..=9u8 {
        let valor = if ranura == 9 { 255 } else { (ranura - 1) * 32 };
        assert_eq!(
            gris.get(ranura),
            Some(Rgba::opaque(valor, valor, valor)),
            "Gray, ranura {ranura}"
        );
        let esperado = if ranura == 9 {
            Rgba::TRANSPARENT
        } else {
            Rgba::opaque(valor, valor, valor)
        };
        assert_eq!(
            con_alfa.get(ranura),
            Some(esperado),
            "GrayAlpha, ranura {ranura}"
        );
    }
}

#[test]
fn con_los_valores_por_defecto_pixel_for_choice_with_es_pixel_for_choice() {
    for image in imagenes() {
        let tabla = SlotColors::defaults(image.mode());
        for choice in opciones() {
            assert_eq!(
                pixel_for_choice_with(&image, choice, &tabla),
                pixel_for_choice(&image, choice),
                "{:?} {choice:?}",
                image.mode()
            );
        }
    }
}

#[test]
fn con_los_valores_por_defecto_cada_ranura_da_lo_que_pixel_for_slot() {
    for image in imagenes() {
        let tabla = SlotColors::defaults(image.mode());
        for ranura in 1..=9u8 {
            assert_eq!(
                pixel_for_choice_with(&image, ColorChoice::Slot(ranura), &tabla),
                pixel_for_slot(&image, ranura),
                "{:?} ranura {ranura}",
                image.mode()
            );
        }
    }
}

#[test]
fn con_los_valores_por_defecto_las_muestras_y_etiquetas_no_cambian() {
    for image in imagenes() {
        let tabla = SlotColors::defaults(image.mode());
        // Hasta dos más allá de la última muestra: las inexistentes dan lo mismo (None, false).
        for celda in 0..cell_count(&image) + 2 {
            assert_eq!(
                cell_color_with(&image, celda, &tabla),
                cell_color(&image, celda),
                "{:?} celda {celda}",
                image.mode()
            );
            assert_eq!(
                cell_label_with(&image, celda, &tabla),
                cell_label(&image, celda),
                "{:?} celda {celda}",
                image.mode()
            );
            assert_eq!(
                is_transparent_cell_with(&image, celda, &tabla),
                is_transparent_cell(&image, celda),
                "{:?} celda {celda}",
                image.mode()
            );
        }
        for choice in opciones() {
            assert_eq!(
                choice_label_with(&image, choice, &tabla),
                choice_label(&image, choice),
                "{:?} {choice:?}",
                image.mode()
            );
        }
    }
}

// `get` y `set`.

#[test]
fn get_fuera_de_1_a_9_da_none() {
    for modo in MODOS {
        let tabla = SlotColors::defaults(modo);
        assert_eq!(tabla.get(0), None);
        assert_eq!(tabla.get(10), None);
        assert_eq!(tabla.get(255), None);
        for ranura in 1..=9 {
            assert!(tabla.get(ranura).is_some());
        }
    }
}

#[test]
fn set_cambia_solo_esa_ranura() {
    let mut tabla = SlotColors::defaults(Mode::Rgba);
    let original = tabla.clone();
    assert!(tabla.set(3, rgba(1, 2, 3, 4)));
    assert_eq!(tabla.get(3), Some(rgba(1, 2, 3, 4)));
    for ranura in (1..=9).filter(|r| *r != 3) {
        assert_eq!(tabla.get(ranura), original.get(ranura), "ranura {ranura}");
    }
    assert_ne!(tabla, original);
    // Volver al valor por defecto da la tabla original.
    assert!(tabla.set(3, original.get(3).expect("existe")));
    assert_eq!(tabla, original);
}

#[test]
fn set_en_todas_las_ranuras_de_1_a_9() {
    let mut tabla = SlotColors::defaults(Mode::GrayAlpha);
    for ranura in 1..=9u8 {
        let color = rgba(ranura, ranura, ranura, 100 + ranura);
        assert!(tabla.set(ranura, color), "ranura {ranura}");
        assert_eq!(tabla.get(ranura), Some(color));
    }
}

#[test]
fn set_fuera_de_1_a_9_da_false_y_no_cambia_nada() {
    let mut tabla = SlotColors::defaults(Mode::Rgba);
    let original = tabla.clone();
    for ranura in [0u8, 10, 11, 255] {
        assert!(!tabla.set(ranura, propio()), "ranura {ranura}");
        assert_eq!(tabla, original, "ranura {ranura}");
    }
}

// `SlotBank`.

#[test]
fn un_banco_nuevo_tiene_todas_las_tablas_por_defecto() {
    let banco = SlotBank::new();
    for modo in [Mode::Rgba, Mode::Gray, Mode::GrayAlpha] {
        assert_eq!(banco.get(modo), &SlotColors::defaults(modo), "{modo:?}");
    }
    assert_eq!(SlotBank::default(), banco);
}

#[test]
fn las_tablas_del_banco_son_independientes_por_modo() {
    let mut banco = SlotBank::new();
    let (rgba_antes, gris_antes, alfa_antes) = (
        banco.get(Mode::Rgba).clone(),
        banco.get(Mode::Gray).clone(),
        banco.get(Mode::GrayAlpha).clone(),
    );
    assert!(banco.get_mut(Mode::Rgba).set(3, propio()));
    assert_eq!(banco.get(Mode::Rgba).get(3), Some(propio()));
    assert_eq!(banco.get(Mode::Gray), &gris_antes);
    assert_eq!(banco.get(Mode::GrayAlpha), &alfa_antes);

    assert!(
        banco
            .get_mut(Mode::Gray)
            .set(5, Rgba::opaque(200, 200, 200))
    );
    assert_eq!(
        banco.get(Mode::Gray).get(5),
        Some(Rgba::opaque(200, 200, 200))
    );
    assert_eq!(banco.get(Mode::GrayAlpha), &alfa_antes);
    assert_ne!(banco.get(Mode::Rgba), &rgba_antes);

    assert!(banco.get_mut(Mode::GrayAlpha).set(9, rgba(9, 9, 9, 255)));
    assert_eq!(banco.get(Mode::GrayAlpha).get(9), Some(rgba(9, 9, 9, 255)));
    // Lo anterior sigue como estaba.
    assert_eq!(banco.get(Mode::Rgba).get(3), Some(propio()));
    assert_eq!(
        banco.get(Mode::Gray).get(5),
        Some(Rgba::opaque(200, 200, 200))
    );
}

#[test]
fn indexadas_comparte_la_tabla_de_rgba() {
    let mut banco = SlotBank::new();
    assert_eq!(banco.get(Mode::Indexed), banco.get(Mode::Rgba));
    assert!(banco.get_mut(Mode::Indexed).set(4, propio()));
    assert_eq!(banco.get(Mode::Rgba).get(4), Some(propio()));
    assert!(banco.get_mut(Mode::Rgba).set(6, rgba(1, 1, 1, 1)));
    assert_eq!(banco.get(Mode::Indexed).get(6), Some(rgba(1, 1, 1, 1)));
    // Y no toca las de gris.
    assert_eq!(banco.get(Mode::Gray), &SlotColors::defaults(Mode::Gray));
}

#[test]
fn el_banco_se_puede_clonar_y_comparar() {
    let mut banco = SlotBank::new();
    let copia = banco.clone();
    assert_eq!(banco, copia);
    banco.get_mut(Mode::Rgba).set(1, propio());
    assert_ne!(banco, copia);
}

// Una ranura editada.

#[test]
fn rgba_una_ranura_editada_se_dibuja_con_su_color() {
    let image = imagen(Mode::Rgba);
    let mut tabla = SlotColors::defaults(Mode::Rgba);
    assert!(tabla.set(3, AMARILLO));
    assert_eq!(
        pixel_for_choice_with(&image, ColorChoice::Slot(3), &tabla),
        Some(Pixel::Rgba(AMARILLO))
    );
    // Las demás no cambian.
    assert_eq!(
        pixel_for_choice_with(&image, ColorChoice::Slot(2), &tabla),
        pixel_for_slot(&image, 2)
    );
    // Y las funciones sin `_with` siguen con los valores por defecto.
    assert_eq!(
        pixel_for_choice(&image, ColorChoice::Slot(3)),
        pixel_for_slot(&image, 3)
    );
}

const AMARILLO: Rgba = Rgba::opaque(255, 255, 0);

#[test]
fn rgba_la_muestra_y_las_etiquetas_leen_la_ranura_editada() {
    // Fila 15: `cell_label(2)` da `color 3 #ffff00`.
    let image = imagen(Mode::Rgba);
    let mut tabla = SlotColors::defaults(Mode::Rgba);
    tabla.set(3, AMARILLO);
    assert_eq!(cell_color_with(&image, 2, &tabla), Some(AMARILLO));
    assert_eq!(
        cell_label_with(&image, 2, &tabla).as_deref(),
        Some("color 3 #ffff00")
    );
    assert_eq!(
        choice_label_with(&image, ColorChoice::Slot(3), &tabla).as_deref(),
        Some("color 3 #ffff00")
    );
    assert!(!is_transparent_cell_with(&image, 2, &tabla));
}

#[test]
fn rgba_la_ranura_9_pasa_de_transparente_a_opaca() {
    let image = imagen(Mode::Rgba);
    let mut tabla = SlotColors::defaults(Mode::Rgba);
    assert!(is_transparent_cell_with(&image, 8, &tabla));
    assert_eq!(
        pixel_for_choice_with(&image, ColorChoice::Slot(9), &tabla),
        Some(Pixel::Rgba(Rgba::TRANSPARENT))
    );
    // Fila 15: la 9 (por defecto `(0,0,0,0)`) con A a 255.
    assert!(tabla.set(9, Rgba::opaque(0, 0, 0)));
    assert_eq!(
        pixel_for_choice_with(&image, ColorChoice::Slot(9), &tabla),
        Some(Pixel::Rgba(Rgba::opaque(0, 0, 0)))
    );
    assert!(!is_transparent_cell_with(&image, 8, &tabla));
    assert_eq!(
        cell_color_with(&image, 8, &tabla),
        Some(Rgba::opaque(0, 0, 0))
    );
}

#[test]
fn rgba_una_ranura_con_alfa_cero_es_transparente() {
    let image = imagen(Mode::Rgba);
    let mut tabla = SlotColors::defaults(Mode::Rgba);
    tabla.set(2, rgba(255, 255, 255, 0));
    assert!(is_transparent_cell_with(&image, 1, &tabla));
}

#[test]
fn gray_una_ranura_editada_da_pixel_gray_con_el_rojo() {
    let image = imagen(Mode::Gray);
    let mut tabla = SlotColors::defaults(Mode::Gray);
    // Fila 15: Gray, `Slot(5)` (gris 128) con G a 200.
    assert_eq!(
        pixel_for_choice_with(&image, ColorChoice::Slot(5), &tabla),
        Some(Pixel::Gray(128))
    );
    assert!(tabla.set(5, Rgba::opaque(200, 200, 200)));
    assert_eq!(
        pixel_for_choice_with(&image, ColorChoice::Slot(5), &tabla),
        Some(Pixel::Gray(200))
    );
    assert_eq!(
        cell_label_with(&image, 4, &tabla).as_deref(),
        Some("color 5 gris 200")
    );
    assert_eq!(
        cell_color_with(&image, 4, &tabla),
        Some(Rgba::opaque(200, 200, 200))
    );
}

#[test]
fn gray_alpha_una_ranura_editada_lleva_gris_y_alfa() {
    let image = imagen(Mode::GrayAlpha);
    let mut tabla = SlotColors::defaults(Mode::GrayAlpha);
    assert!(tabla.set(9, rgba(40, 40, 40, 128)));
    assert_eq!(
        pixel_for_choice_with(&image, ColorChoice::Slot(9), &tabla),
        Some(Pixel::GrayAlpha {
            gray: 40,
            alpha: 128
        })
    );
    assert_eq!(
        cell_label_with(&image, 8, &tabla).as_deref(),
        Some("color 9 gris 40 alfa 128")
    );
    assert!(!is_transparent_cell_with(&image, 8, &tabla));

    // A 255 se rotula sin alfa, y con alfa 0 es transparente.
    assert!(tabla.set(2, rgba(77, 77, 77, 255)));
    assert_eq!(
        pixel_for_choice_with(&image, ColorChoice::Slot(2), &tabla),
        Some(Pixel::GrayAlpha {
            gray: 77,
            alpha: 255
        })
    );
    assert_eq!(
        cell_label_with(&image, 1, &tabla).as_deref(),
        Some("color 2 gris 77")
    );
    assert!(tabla.set(3, rgba(10, 10, 10, 0)));
    assert!(is_transparent_cell_with(&image, 2, &tabla));
}

#[test]
fn en_indexadas_las_ranuras_editadas_no_cuentan() {
    for image in [indexada_9(), indexada_3_con_transparente()] {
        let mut tabla = SlotColors::defaults(Mode::Indexed);
        for ranura in 1..=9u8 {
            tabla.set(ranura, propio());
        }
        for choice in opciones() {
            assert_eq!(
                pixel_for_choice_with(&image, choice, &tabla),
                pixel_for_choice(&image, choice),
                "{choice:?}"
            );
        }
        for celda in 0..cell_count(&image) + 2 {
            assert_eq!(
                cell_color_with(&image, celda, &tabla),
                cell_color(&image, celda)
            );
            assert_eq!(
                cell_label_with(&image, celda, &tabla),
                cell_label(&image, celda)
            );
        }
    }
}

#[test]
fn las_elecciones_que_no_valen_siguen_sin_valer_con_una_tabla_editada() {
    let mut tabla = SlotColors::defaults(Mode::Rgba);
    tabla.set(1, propio());
    let image = imagen(Mode::Rgba);
    assert_eq!(
        pixel_for_choice_with(&image, ColorChoice::Slot(0), &tabla),
        None
    );
    assert_eq!(
        pixel_for_choice_with(&image, ColorChoice::Slot(10), &tabla),
        None
    );
    assert_eq!(
        pixel_for_choice_with(&image, ColorChoice::Entry(0), &tabla),
        None
    );
    assert_eq!(cell_color_with(&image, 9, &tabla), None);
    assert_eq!(cell_label_with(&image, 9, &tabla), None);
    assert_eq!(
        choice_label_with(&image, ColorChoice::Slot(10), &tabla),
        None
    );
    assert!(!is_transparent_cell_with(&image, 9, &tabla));
}

// `ToolState::canvas_event_with` y `canvas_input_with`.

fn begin(x: i32, y: i32) -> CanvasEvent {
    CanvasEvent::StrokeBegin((x, y))
}

fn mov(x: i32, y: i32) -> CanvasEvent {
    CanvasEvent::StrokeMove((x, y))
}

fn fin() -> CanvasEvent {
    CanvasEvent::StrokeEnd
}

struct Escena {
    doc: Document,
    id: ImageId,
    tool: ToolState,
}

impl Escena {
    fn nueva(image: Image, herramienta: Tool) -> Escena {
        let doc = Document::new(image);
        let id = ImageId::from_raw(0);
        let mut tool = ToolState::new();
        tool.apply_action(ToolAction::Select(herramienta));
        Escena { doc, id, tool }
    }

    fn rgba(herramienta: Tool) -> Escena {
        Escena::nueva(imagen(Mode::Rgba), herramienta)
    }

    fn imagen(&self) -> &Image {
        self.doc.image(self.id).expect("hay imagen")
    }

    fn pixel(&self, x: u32, y: u32) -> Pixel {
        self.imagen().get_pixel(x, y).expect("píxel dentro")
    }

    /// Un evento con `canvas_event_with`; un `Err` es un fallo del test.
    fn ev(&mut self, pixel: Option<Pixel>, evento: CanvasEvent) -> bool {
        self.tool
            .canvas_event_with(&mut self.doc, self.id, pixel, evento)
            .unwrap_or_else(|e| panic!("{evento:?} dio el error {e:?}"))
    }

    fn entrada(&mut self, pixel: Option<Pixel>, entrada: CanvasInput) -> bool {
        self.tool
            .canvas_input_with(&mut self.doc, self.id, pixel, entrada)
            .unwrap_or_else(|e| panic!("{entrada:?} dio el error {e:?}"))
    }
}

fn pixel_propio() -> Pixel {
    Pixel::Rgba(propio())
}

#[test]
fn el_lapiz_dibuja_el_pixel_dado() {
    let mut escena = Escena::rgba(Tool::Pencil);
    let vacio = escena.pixel(0, 0);
    assert!(!escena.ev(Some(pixel_propio()), begin(2, 2)));
    assert!(!escena.ev(Some(pixel_propio()), mov(5, 2)));
    assert!(
        escena.ev(Some(pixel_propio()), fin()),
        "el trazo cambia el documento"
    );
    assert_eq!(escena.pixel(2, 2), pixel_propio());
    assert_eq!(escena.pixel(5, 2), pixel_propio());
    assert_eq!(escena.pixel(0, 0), vacio);
    assert!(escena.doc.can_undo());
    assert!(!escena.tool.busy());
}

#[test]
fn un_clic_sin_mover_con_el_lapiz_pinta_un_pixel() {
    let mut escena = Escena::rgba(Tool::Pencil);
    escena.ev(Some(pixel_propio()), begin(3, 4));
    assert!(escena.ev(Some(pixel_propio()), fin()));
    assert_eq!(escena.pixel(3, 4), pixel_propio());
    assert_eq!(
        escena.pixel(4, 4),
        imagen(Mode::Rgba).get_pixel(4, 4).unwrap()
    );
}

#[test]
fn la_linea_dibuja_el_pixel_dado() {
    let mut escena = Escena::rgba(Tool::Line);
    escena.ev(Some(pixel_propio()), begin(0, 0));
    escena.ev(Some(pixel_propio()), mov(3, 0));
    assert_eq!(escena.tool.preview().count(), 4, "la línea pendiente se ve");
    assert!(escena.ev(Some(pixel_propio()), fin()));
    for x in 0..=3 {
        assert_eq!(escena.pixel(x, 0), pixel_propio(), "x = {x}");
    }
    assert_ne!(escena.pixel(4, 0), pixel_propio());
}

#[test]
fn el_relleno_pinta_con_el_pixel_dado() {
    let mut escena = Escena::rgba(Tool::Fill);
    assert!(escena.ev(Some(pixel_propio()), begin(1, 1)));
    for y in 0..8 {
        for x in 0..8 {
            assert_eq!(escena.pixel(x, y), pixel_propio(), "({x}, {y})");
        }
    }
    assert!(!escena.ev(Some(pixel_propio()), fin()));
    assert!(escena.doc.can_undo());
}

#[test]
fn el_pixel_dado_vale_en_gray_gray_alpha_e_indexadas() {
    let casos = [
        (imagen(Mode::Gray), Pixel::Gray(200)),
        (
            imagen(Mode::GrayAlpha),
            Pixel::GrayAlpha {
                gray: 40,
                alpha: 128,
            },
        ),
        (indexada_9(), Pixel::Indexed(1)),
    ];
    for (image, pixel) in casos {
        let modo = image.mode();
        let mut escena = Escena::nueva(image, Tool::Fill);
        assert!(escena.ev(Some(pixel), begin(0, 0)), "{modo:?}");
        assert_eq!(escena.pixel(7, 7), pixel, "{modo:?}");
    }
    let mut escena = Escena::nueva(imagen(Mode::Gray), Tool::Pencil);
    escena.ev(Some(Pixel::Gray(200)), begin(1, 1));
    assert!(escena.ev(Some(Pixel::Gray(200)), fin()));
    assert_eq!(escena.pixel(1, 1), Pixel::Gray(200));
}

#[test]
fn sin_pixel_ninguna_herramienta_empieza_gesto_ni_cambia_el_documento() {
    for herramienta in [Tool::Pencil, Tool::Line, Tool::Fill] {
        let mut escena = Escena::rgba(herramienta);
        let original = escena.imagen().clone();
        let estado = escena.doc.state_id();
        assert!(!escena.ev(None, begin(2, 2)), "{herramienta:?}");
        assert!(!escena.ev(None, mov(5, 2)), "{herramienta:?}");
        assert_eq!(escena.tool.preview().count(), 0, "{herramienta:?}");
        assert!(!escena.ev(None, fin()), "{herramienta:?}");
        assert_eq!(escena.imagen(), &original, "{herramienta:?}");
        assert_eq!(escena.doc.state_id(), estado, "{herramienta:?}");
        assert!(!escena.doc.can_undo(), "{herramienta:?}");
        assert!(!escena.tool.busy(), "{herramienta:?}");
    }
}

#[test]
fn con_un_pixel_dado_y_luego_sin_el_se_vuelve_a_poder_dibujar() {
    let mut escena = Escena::rgba(Tool::Pencil);
    escena.ev(None, begin(2, 2));
    escena.ev(None, fin());
    escena.ev(Some(pixel_propio()), begin(2, 2));
    assert!(escena.ev(Some(pixel_propio()), fin()));
    assert_eq!(escena.pixel(2, 2), pixel_propio());
}

#[test]
fn canvas_input_with_con_eventos_de_trazo_equivale_a_canvas_event_with() {
    for herramienta in [Tool::Pencil, Tool::Line, Tool::Fill] {
        let mut directa = Escena::rgba(herramienta);
        let mut ordenada = Escena::rgba(herramienta);
        for evento in [begin(1, 1), mov(4, 1), mov(4, 4), fin()] {
            let a = directa.ev(Some(pixel_propio()), evento);
            let b = ordenada.entrada(Some(pixel_propio()), CanvasInput::Stroke(evento));
            assert_eq!(a, b, "{herramienta:?} {evento:?}");
        }
        assert_eq!(directa.imagen(), ordenada.imagen(), "{herramienta:?}");
    }
}

#[test]
fn canvas_input_with_escape_cancela_la_linea_pendiente() {
    let mut escena = Escena::rgba(Tool::Line);
    let original = escena.imagen().clone();
    escena.entrada(Some(pixel_propio()), CanvasInput::Stroke(begin(0, 0)));
    escena.entrada(Some(pixel_propio()), CanvasInput::Stroke(mov(3, 0)));
    assert!(!escena.entrada(Some(pixel_propio()), CanvasInput::Escape));
    assert_eq!(escena.tool.preview().count(), 0);
    assert!(!escena.entrada(Some(pixel_propio()), CanvasInput::Stroke(fin())));
    assert_eq!(escena.imagen(), &original);
    assert!(!escena.tool.busy());
}

#[test]
fn con_el_pixel_de_pixel_for_choice_las_variantes_with_dan_lo_mismo_que_las_de_siempre() {
    for herramienta in [Tool::Pencil, Tool::Line, Tool::Fill] {
        let mut con = Escena::rgba(herramienta);
        let mut sin = Escena::rgba(herramienta);
        let pixel = pixel_for_choice(&imagen(Mode::Rgba), ColorChoice::Slot(3));
        assert_eq!(pixel, Some(Pixel::Rgba(Rgba::opaque(255, 0, 0))));
        for evento in [begin(1, 1), mov(5, 5), fin()] {
            let a = con.ev(pixel, evento);
            let b = sin
                .tool
                .canvas_event(&mut sin.doc, sin.id, 3, evento)
                .expect("sin error");
            assert_eq!(a, b, "{herramienta:?} {evento:?}");
        }
        assert_eq!(con.imagen(), sin.imagen(), "{herramienta:?}");
    }
}

#[test]
fn choice_label_with_rotula_la_ranura_editada() {
    // Como `slot_label`: `color 3 #ffff00`, `color 5 gris 200`, `color 9 transparente`.
    let rgba_img = imagen(Mode::Rgba);
    let mut tabla = SlotColors::defaults(Mode::Rgba);
    tabla.set(3, AMARILLO);
    assert_eq!(
        choice_label_with(&rgba_img, ColorChoice::Slot(3), &tabla).as_deref(),
        Some("color 3 #ffff00")
    );
    assert_eq!(
        choice_label_with(&rgba_img, ColorChoice::Slot(9), &tabla).as_deref(),
        Some("color 9 transparente")
    );
    tabla.set(9, Rgba::opaque(0, 0, 0));
    assert_eq!(
        choice_label_with(&rgba_img, ColorChoice::Slot(9), &tabla).as_deref(),
        Some("color 9 #000000")
    );

    let gris = imagen(Mode::Gray);
    let mut tabla = SlotColors::defaults(Mode::Gray);
    tabla.set(5, Rgba::opaque(200, 200, 200));
    assert_eq!(
        choice_label_with(&gris, ColorChoice::Slot(5), &tabla).as_deref(),
        Some("color 5 gris 200")
    );
}
