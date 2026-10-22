//! El panel de capas pintado con egui (specs/hito-3-capas-interfaz.md, unidad 3.4: filas 7 y 12 y
//! propiedad 4, y condición 2 del visto bueno). En ROJO hasta la lógica:
//! `px_ui::layers_panel_view` y `layers_panel::layer_nav` no existen todavía.
//!
//! Banco de egui sin ventana (`Context::run_ui` con `RawInput` sintético) a las cinco escalas: el
//! panel (`show_layer_panel`, un `egui::Area` con su rectángulo exacto) se muestra **antes** del
//! lienzo, como en la app, y el `TexturesDelta` de cada fotograma se cuenta. El clic es mover,
//! pulsar y soltar en fotogramas distintos, después de dejar pasar tres fotogramas con el panel
//! abierto (egui decide la capa bajo el puntero con las áreas del fotograma anterior).
//!
//! - Fila 7: cada control emite su evento con un clic real; los botones atenuados no responden;
//!   con `enabled = false` (un diálogo modal abierto, condición 2) el panel no responde a nada;
//!   la barra arrastra y la rueda desplaza la lista.
//! - Fila 12 (parte de `px-ui`): un clic o la rueda dentro del panel no llegan al lienzo, fuera
//!   sí; abrir, cerrar, arrastrar y desplazar no suben texturas.
//! - Propiedad 4: sin comando, el documento no cambia y no se sube ninguna textura.

use egui::{Context, Event, Pos2, RawInput, Rect, vec2};
use px_core::{Command, Document, Image, LayerId, Mode};
use px_ui::canvas::{Canvas, CanvasEvent, CanvasInput};
use px_ui::icons::IconSet;
use px_ui::layers_panel::{
    PanelButton, PanelState, RectU, button_u, close_u, eye_u, layer_nav, lock_u, name_u, title_u,
};
use px_ui::layers_panel_view::{PanelEvent, PanelView, show_layer_panel};
use px_ui::source::DocSource;

mod common;
use common::{PPPS, button_2, context, in_points, mover, n_de, wheel_line};

const WIDTH_PT: f32 = 900.0;
const HEIGHT_PT: f32 = 600.0;

fn document(layers: usize) -> Document {
    let image_2 = Image::new(64, 48, Mode::Rgba).expect("imagen");
    let mut doc = Document::new(image_2);
    for i in 1..layers {
        doc.apply(Command::AddLayer {
            name: format!("Capa {}", i + 1),
            at: i,
        })
        .expect("añadir");
    }
    doc
}

struct Step {
    events_2: Vec<PanelEvent>,
    canvas_2: Vec<CanvasInput>,
    uploaded: usize,
}

struct Bench {
    ctx: Context,
    ppp: f32,
    n: u32,
    window: (i64, i64),
    canvas: Canvas,
    doc: Document,
    active_2: LayerId,
    state_2: PanelState,
    enabled_2: bool,
    icons_2: IconSet,
}

impl Bench {
    fn new_2(ppp: f32, doc: Document, open_2_2: bool) -> Bench {
        let ctx = context(ppp);
        let mut canvas = Canvas::with_tile_side(64);
        canvas.load(&ctx, &DocSource(&doc)).expect("load");
        let active_2 = doc.layers()[doc.layers().len() / 2].id();
        let state_2 = PanelState {
            open: open_2_2,
            ..PanelState::default()
        };
        let mut bench = Bench {
            ctx,
            ppp,
            n: n_de(ppp) as u32,
            window: (
                (WIDTH_PT * ppp).round() as i64,
                (HEIGHT_PT * ppp).round() as i64,
            ),
            canvas,
            doc,
            active_2,
            state_2,
            enabled_2: true,
            icons_2: IconSet::default(),
        };
        for _ in 0..3 {
            bench.step(vec![]);
        }
        bench
    }

    fn step(&mut self, events_2: Vec<Event>) -> Step {
        let input_2 = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(WIDTH_PT, HEIGHT_PT))),
            events: events_2,
            ..RawInput::default()
        };
        let (window, doc, active_2, enabled_2) =
            (self.window, &self.doc, self.active_2, self.enabled_2);
        let (state_2, canvas, icons_2) = (&mut self.state_2, &mut self.canvas, &self.icons_2);
        let ids: Vec<_> = canvas.texture_ids();
        let mut panel = Vec::new();
        let mut canvas_2 = Vec::new();
        let mut output_2 = self.ctx.run_ui(input_2, |ui| {
            panel = show_layer_panel(
                ui.ctx(),
                &PanelView {
                    doc,
                    active: active_2,
                    enabled: enabled_2,
                    icons: icons_2,
                },
                state_2,
                window,
            );
            canvas_2 = canvas
                .interact_roles(ui)
                .into_iter()
                .map(|(input, _)| input)
                .collect::<Vec<_>>();
            canvas.sync(ui.ctx(), &DocSource(doc)).expect("sync");
            canvas.paint(ui);
        });
        let mut uploaded = 0;
        for (id, deltas) in &output_2.textures_delta.set {
            if !ids.contains(id) {
                continue;
            }
            for delta in deltas {
                let egui::ImageData::Color(image_2) = &delta.image;
                uploaded += image_2.size[0] * image_2.size[1];
            }
        }
        output_2.textures_delta.clear();
        Step {
            events_2: panel,
            canvas_2,
            uploaded,
        }
    }

    /// El centro de `r` (en `u` del panel) en puntos de la pantalla.
    fn center(&mut self, r: RectU) -> Pos2 {
        let panel = self.state_2.rect(self.window, self.n);
        let k = i64::from(self.n);
        in_points(
            self.ppp,
            panel.x + (r.x + r.w / 2) * k,
            panel.y + (r.y + r.h / 2) * k,
        )
    }

    /// Mover, pulsar y soltar; devuelve todo lo que salió de los tres fotogramas.
    fn click(&mut self, pos: Pos2) -> Vec<Step> {
        vec![
            self.step(vec![mover(pos)]),
            self.step(vec![button_2(pos, true)]),
            self.step(vec![button_2(pos, false)]),
        ]
    }

    fn events_of(steps_2: &[Step]) -> Vec<PanelEvent> {
        steps_2.iter().flat_map(|p| p.events_2.clone()).collect()
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 7: cada control con un clic real
// ---------------------------------------------------------------------------------------------

#[test]
fn row_7_each_button_the_eye_the_lock_the_row_and_close_emit_their_event() {
    for ppp in PPPS {
        let mut bench = Bench::new_2(ppp, document(3), true);
        let layers: Vec<LayerId> = bench.doc.layers().iter().map(|l| l.id()).collect();
        // La fila 0 es la de arriba (la tercera capa).
        let cases: Vec<(RectU, PanelEvent)> = vec![
            (
                button_u(PanelButton::New),
                PanelEvent::Button(PanelButton::New),
            ),
            (
                button_u(PanelButton::Duplicate),
                PanelEvent::Button(PanelButton::Duplicate),
            ),
            (
                button_u(PanelButton::Remove),
                PanelEvent::Button(PanelButton::Remove),
            ),
            (
                button_u(PanelButton::Up),
                PanelEvent::Button(PanelButton::Up),
            ),
            (
                button_u(PanelButton::Down),
                PanelEvent::Button(PanelButton::Down),
            ),
            (eye_u(0), PanelEvent::ToggleVisible(layers[2])),
            (lock_u(1), PanelEvent::ToggleLocked(layers[1])),
            (name_u(2), PanelEvent::Select(layers[0])),
            (close_u(), PanelEvent::Close),
        ];
        for (rect, expected_2) in cases {
            let pos = bench.center(rect);
            let steps_2 = bench.click(pos);
            assert_eq!(
                Bench::events_of(&steps_2),
                vec![expected_2.clone()],
                "ppp {ppp}: {expected_2:?}"
            );
            assert!(
                steps_2.iter().all(|p| p.canvas_2.is_empty()),
                "ppp {ppp}: el clic llegó al lienzo"
            );
        }
    }
}

#[test]
fn row_7_with_one_layer_remove_raise_and_lower_are_dimmed_and_do_not_respond() {
    for ppp in PPPS {
        let mut bench = Bench::new_2(ppp, document(1), true);
        for button_2 in [PanelButton::Remove, PanelButton::Up, PanelButton::Down] {
            let pos = bench.center(button_u(button_2));
            let steps_2 = bench.click(pos);
            assert_eq!(
                Bench::events_of(&steps_2),
                vec![],
                "ppp {ppp}: {button_2:?}"
            );
        }
        let pos = bench.center(button_u(PanelButton::New));
        let steps_2 = bench.click(pos);
        assert_eq!(
            Bench::events_of(&steps_2),
            vec![PanelEvent::Button(PanelButton::New)]
        );
        let nav = layer_nav(&bench.doc, bench.active_2);
        assert_eq!((nav.count, nav.active_pos, nav.can_add), (1, 0, true));
    }
}

#[test]
fn row_7_with_a_modal_open_the_panel_does_not_respond_to_anything() {
    // Condición 2: `enabled = false` es lo que la app pasa con un diálogo modal (o una conversión
    // o un guardado en espera) abierto.
    for ppp in PPPS {
        let mut bench = Bench::new_2(ppp, document(8), true);
        bench.enabled_2 = false;
        let antes = bench.state_2.rect(bench.window, bench.n);
        let scroll = bench.state_2.scroll;
        for rect in [
            button_u(PanelButton::New),
            eye_u(0),
            lock_u(1),
            name_u(2),
            close_u(),
            title_u(),
        ] {
            let pos = bench.center(rect);
            let steps_2 = bench.click(pos);
            assert_eq!(Bench::events_of(&steps_2), vec![], "ppp {ppp}: {rect:?}");
        }
        // Ni la rueda sobre la lista ni arrastrar la barra de título hacen nada.
        let over_list = bench.center(name_u(1));
        bench.step(vec![mover(over_list)]);
        bench.step(vec![wheel_line(false)]);
        assert_eq!(bench.state_2.scroll, scroll, "ppp {ppp}");
        let bar = bench.center(title_u());
        bench.step(vec![mover(bar)]);
        bench.step(vec![button_2(bar, true)]);
        bench.step(vec![mover(bar + vec2(60.0, 30.0))]);
        bench.step(vec![button_2(bar + vec2(60.0, 30.0), false)]);
        assert_eq!(
            bench.state_2.rect(bench.window, bench.n),
            antes,
            "ppp {ppp}"
        );
        assert!(bench.state_2.open);
    }
}

#[test]
fn row_7_the_bar_drags_the_panel_and_the_wheel_scrolls_the_list() {
    for ppp in PPPS {
        let mut bench = Bench::new_2(ppp, document(10), true);
        let antes = bench.state_2.rect(bench.window, bench.n);
        let bar = bench.center(title_u());
        bench.step(vec![mover(bar)]);
        bench.step(vec![button_2(bar, true)]);
        let target_2 = bar + vec2(-40.0, 25.0);
        bench.step(vec![mover(target_2)]);
        bench.step(vec![button_2(target_2, false)]);
        let after_2 = bench.state_2.rect(bench.window, bench.n);
        let k = i64::from(bench.n);
        let (dx, dy) = (after_2.x - antes.x, after_2.y - antes.y);
        assert!(
            (dx + (40.0 * ppp) as i64).abs() <= 2 * k,
            "ppp {ppp}: dx {dx}"
        );
        assert!(
            (dy - (25.0 * ppp) as i64).abs() <= 2 * k,
            "ppp {ppp}: dy {dy}"
        );
        assert_eq!((dx % k, dy % k), (0, 0), "a pasos de u");
        // La rueda sobre la lista baja 3 filas por muesca (10 capas: como mucho 4).
        let over_list = bench.center(name_u(1));
        bench.step(vec![mover(over_list)]);
        bench.step(vec![wheel_line(false)]);
        assert_eq!(bench.state_2.scroll, 3, "ppp {ppp}");
        bench.step(vec![wheel_line(false)]);
        assert_eq!(bench.state_2.scroll, 4, "ppp {ppp}");
        bench.step(vec![wheel_line(true)]);
        assert_eq!(bench.state_2.scroll, 1, "ppp {ppp}");
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 12 (parte de `px-ui`): el panel y el lienzo
// ---------------------------------------------------------------------------------------------

#[test]
fn row_12_a_click_inside_the_panel_does_not_reach_the_canvas_and_one_outside_does() {
    for ppp in PPPS {
        let mut bench = Bench::new_2(ppp, document(3), true);
        // Un hueco del panel (entre la barra y la lista): dentro del rectángulo, sin control.
        let gap = bench.center(RectU {
            x: 90,
            y: 18,
            w: 2,
            h: 2,
        });
        let steps_2 = bench.click(gap);
        assert!(
            steps_2.iter().all(|p| p.canvas_2.is_empty()),
            "ppp {ppp}: dentro: {:?}",
            steps_2.iter().map(|p| &p.canvas_2).collect::<Vec<_>>()
        );
        assert_eq!(Bench::events_of(&steps_2), vec![]);
        // La rueda dentro del panel tampoco hace zoom en el lienzo.
        let zoom = bench.canvas.zoom();
        bench.step(vec![mover(gap)]);
        bench.step(vec![wheel_line(true)]);
        assert_eq!(bench.canvas.zoom(), zoom, "ppp {ppp}");
        // Fuera del panel (arriba a la izquierda, sobre la imagen) el clic empieza un trazo.
        let outside = Pos2::new(20.0, 20.0);
        let steps_2 = bench.click(outside);
        assert!(
            steps_2.iter().any(|p| p
                .canvas_2
                .iter()
                .any(|e| matches!(e, CanvasInput::Stroke(CanvasEvent::StrokeBegin(_))))),
            "ppp {ppp}: fuera del panel el clic debe llegar al lienzo"
        );
    }
}

#[test]
fn row_12_with_the_panel_closed_it_neither_covers_nor_responds() {
    let mut bench = Bench::new_2(1.0, document(3), false);
    let step = bench.step(vec![]);
    assert!(step.events_2.is_empty());
    // Con el panel cerrado, un clic donde estaría el panel es del lienzo.
    let location = Pos2::new(600.0, 100.0);
    let steps_2 = bench.click(location);
    assert!(steps_2.iter().any(|p| !p.canvas_2.is_empty()));
}

// ---------------------------------------------------------------------------------------------
// Propiedad 4: el panel no toca el lienzo
// ---------------------------------------------------------------------------------------------

#[test]
fn property_4_without_a_command_the_document_does_not_change_or_any_texture_is_uploaded() {
    for ppp in PPPS {
        let mut bench = Bench::new_2(ppp, document(9), true);
        let copy = bench.doc.clone();
        let id = bench.doc.state_id();
        let mut uploaded = 0;
        let bar = bench.center(title_u());
        let list = bench.center(name_u(2));
        let sequence: Vec<Vec<Event>> = vec![
            vec![mover(list)],
            vec![wheel_line(false)],
            vec![wheel_line(false)],
            vec![wheel_line(true)],
            vec![mover(bar)],
            vec![button_2(bar, true)],
            vec![mover(bar + vec2(30.0, 30.0))],
            vec![button_2(bar + vec2(30.0, 30.0), false)],
            vec![mover(Pos2::new(5.0, 5.0))],
            vec![],
        ];
        for events_2 in sequence {
            let step = bench.step(events_2);
            uploaded += step.uploaded;
            assert!(
                step.events_2
                    .iter()
                    .all(|e| matches!(e, PanelEvent::Select(_))),
                "ppp {ppp}: {:?}",
                step.events_2
            );
        }
        // Cerrar y abrir tampoco.
        bench.state_2.open = false;
        uploaded += bench.step(vec![]).uploaded;
        bench.state_2.open = true;
        uploaded += bench.step(vec![]).uploaded;
        assert_eq!(
            uploaded, 0,
            "ppp {ppp}: se subieron {uploaded} píxeles de textura"
        );
        assert_eq!(bench.doc, copy);
        assert_eq!(bench.doc.state_id(), id);
        assert!(!bench.doc.can_redo());
    }
}
