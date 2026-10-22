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
use common::{PPPS, boton, contexto, en_puntos, mover, n_de, rueda_linea};

const ANCHO_PT: f32 = 900.0;
const ALTO_PT: f32 = 600.0;

fn documento(capas: usize) -> Document {
    let imagen = Image::new(64, 48, Mode::Rgba).expect("imagen");
    let mut doc = Document::new(imagen);
    for i in 1..capas {
        doc.apply(Command::AddLayer {
            name: format!("Capa {}", i + 1),
            at: i,
        })
        .expect("añadir");
    }
    doc
}

struct Paso {
    eventos: Vec<PanelEvent>,
    lienzo: Vec<CanvasInput>,
    subidos: usize,
}

struct Banco {
    ctx: Context,
    ppp: f32,
    n: u32,
    ventana: (i64, i64),
    canvas: Canvas,
    doc: Document,
    activa: LayerId,
    estado: PanelState,
    habilitado: bool,
    iconos: IconSet,
}

impl Banco {
    fn nuevo(ppp: f32, doc: Document, abierto: bool) -> Banco {
        let ctx = contexto(ppp);
        let mut canvas = Canvas::with_tile_side(64);
        canvas.load(&ctx, &DocSource(&doc)).expect("load");
        let activa = doc.layers()[doc.layers().len() / 2].id();
        let estado = PanelState {
            open: abierto,
            ..PanelState::default()
        };
        let mut banco = Banco {
            ctx,
            ppp,
            n: n_de(ppp) as u32,
            ventana: (
                (ANCHO_PT * ppp).round() as i64,
                (ALTO_PT * ppp).round() as i64,
            ),
            canvas,
            doc,
            activa,
            estado,
            habilitado: true,
            iconos: IconSet::default(),
        };
        for _ in 0..3 {
            banco.paso(vec![]);
        }
        banco
    }

    fn paso(&mut self, eventos: Vec<Event>) -> Paso {
        let entrada = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(ANCHO_PT, ALTO_PT))),
            events: eventos,
            ..RawInput::default()
        };
        let (ventana, doc, activa, habilitado) =
            (self.ventana, &self.doc, self.activa, self.habilitado);
        let (estado, canvas, iconos) = (&mut self.estado, &mut self.canvas, &self.iconos);
        let ids: Vec<_> = canvas.texture_ids();
        let mut panel = Vec::new();
        let mut lienzo = Vec::new();
        let mut salida = self.ctx.run_ui(entrada, |ui| {
            panel = show_layer_panel(
                ui.ctx(),
                &PanelView {
                    doc,
                    active: activa,
                    enabled: habilitado,
                    icons: iconos,
                },
                estado,
                ventana,
            );
            lienzo = canvas
                .interact_roles(ui)
                .into_iter()
                .map(|(input, _)| input)
                .collect::<Vec<_>>();
            canvas.sync(ui.ctx(), &DocSource(doc)).expect("sync");
            canvas.paint(ui);
        });
        let mut subidos = 0;
        for (id, deltas) in &salida.textures_delta.set {
            if !ids.contains(id) {
                continue;
            }
            for delta in deltas {
                let egui::ImageData::Color(imagen) = &delta.image;
                subidos += imagen.size[0] * imagen.size[1];
            }
        }
        salida.textures_delta.clear();
        Paso {
            eventos: panel,
            lienzo,
            subidos,
        }
    }

    /// El centro de `r` (en `u` del panel) en puntos de la pantalla.
    fn centro(&mut self, r: RectU) -> Pos2 {
        let panel = self.estado.rect(self.ventana, self.n);
        let k = i64::from(self.n);
        en_puntos(
            self.ppp,
            panel.x + (r.x + r.w / 2) * k,
            panel.y + (r.y + r.h / 2) * k,
        )
    }

    /// Mover, pulsar y soltar; devuelve todo lo que salió de los tres fotogramas.
    fn clic(&mut self, pos: Pos2) -> Vec<Paso> {
        vec![
            self.paso(vec![mover(pos)]),
            self.paso(vec![boton(pos, true)]),
            self.paso(vec![boton(pos, false)]),
        ]
    }

    fn eventos_de(pasos: &[Paso]) -> Vec<PanelEvent> {
        pasos.iter().flat_map(|p| p.eventos.clone()).collect()
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 7: cada control con un clic real
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_7_cada_boton_el_ojo_el_candado_la_fila_y_cerrar_emiten_su_evento() {
    for ppp in PPPS {
        let mut banco = Banco::nuevo(ppp, documento(3), true);
        let capas: Vec<LayerId> = banco.doc.layers().iter().map(|l| l.id()).collect();
        // La fila 0 es la de arriba (la tercera capa).
        let casos: Vec<(RectU, PanelEvent)> = vec![
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
            (eye_u(0), PanelEvent::ToggleVisible(capas[2])),
            (lock_u(1), PanelEvent::ToggleLocked(capas[1])),
            (name_u(2), PanelEvent::Select(capas[0])),
            (close_u(), PanelEvent::Close),
        ];
        for (rect, esperado) in casos {
            let pos = banco.centro(rect);
            let pasos = banco.clic(pos);
            assert_eq!(
                Banco::eventos_de(&pasos),
                vec![esperado.clone()],
                "ppp {ppp}: {esperado:?}"
            );
            assert!(
                pasos.iter().all(|p| p.lienzo.is_empty()),
                "ppp {ppp}: el clic llegó al lienzo"
            );
        }
    }
}

#[test]
fn fila_7_con_una_capa_quitar_subir_y_bajar_estan_atenuados_y_no_responden() {
    for ppp in PPPS {
        let mut banco = Banco::nuevo(ppp, documento(1), true);
        for boton in [PanelButton::Remove, PanelButton::Up, PanelButton::Down] {
            let pos = banco.centro(button_u(boton));
            let pasos = banco.clic(pos);
            assert_eq!(Banco::eventos_de(&pasos), vec![], "ppp {ppp}: {boton:?}");
        }
        let pos = banco.centro(button_u(PanelButton::New));
        let pasos = banco.clic(pos);
        assert_eq!(
            Banco::eventos_de(&pasos),
            vec![PanelEvent::Button(PanelButton::New)]
        );
        let nav = layer_nav(&banco.doc, banco.activa);
        assert_eq!((nav.count, nav.active_pos, nav.can_add), (1, 0, true));
    }
}

#[test]
fn fila_7_con_un_modal_abierto_el_panel_no_responde_a_nada() {
    // Condición 2: `enabled = false` es lo que la app pasa con un diálogo modal (o una conversión
    // o un guardado en espera) abierto.
    for ppp in PPPS {
        let mut banco = Banco::nuevo(ppp, documento(8), true);
        banco.habilitado = false;
        let antes = banco.estado.rect(banco.ventana, banco.n);
        let scroll = banco.estado.scroll;
        for rect in [
            button_u(PanelButton::New),
            eye_u(0),
            lock_u(1),
            name_u(2),
            close_u(),
            title_u(),
        ] {
            let pos = banco.centro(rect);
            let pasos = banco.clic(pos);
            assert_eq!(Banco::eventos_de(&pasos), vec![], "ppp {ppp}: {rect:?}");
        }
        // Ni la rueda sobre la lista ni arrastrar la barra de título hacen nada.
        let sobre_lista = banco.centro(name_u(1));
        banco.paso(vec![mover(sobre_lista)]);
        banco.paso(vec![rueda_linea(false)]);
        assert_eq!(banco.estado.scroll, scroll, "ppp {ppp}");
        let barra = banco.centro(title_u());
        banco.paso(vec![mover(barra)]);
        banco.paso(vec![boton(barra, true)]);
        banco.paso(vec![mover(barra + vec2(60.0, 30.0))]);
        banco.paso(vec![boton(barra + vec2(60.0, 30.0), false)]);
        assert_eq!(
            banco.estado.rect(banco.ventana, banco.n),
            antes,
            "ppp {ppp}"
        );
        assert!(banco.estado.open);
    }
}

#[test]
fn fila_7_la_barra_arrastra_el_panel_y_la_rueda_desplaza_la_lista() {
    for ppp in PPPS {
        let mut banco = Banco::nuevo(ppp, documento(10), true);
        let antes = banco.estado.rect(banco.ventana, banco.n);
        let barra = banco.centro(title_u());
        banco.paso(vec![mover(barra)]);
        banco.paso(vec![boton(barra, true)]);
        let destino = barra + vec2(-40.0, 25.0);
        banco.paso(vec![mover(destino)]);
        banco.paso(vec![boton(destino, false)]);
        let despues = banco.estado.rect(banco.ventana, banco.n);
        let k = i64::from(banco.n);
        let (dx, dy) = (despues.x - antes.x, despues.y - antes.y);
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
        let sobre_lista = banco.centro(name_u(1));
        banco.paso(vec![mover(sobre_lista)]);
        banco.paso(vec![rueda_linea(false)]);
        assert_eq!(banco.estado.scroll, 3, "ppp {ppp}");
        banco.paso(vec![rueda_linea(false)]);
        assert_eq!(banco.estado.scroll, 4, "ppp {ppp}");
        banco.paso(vec![rueda_linea(true)]);
        assert_eq!(banco.estado.scroll, 1, "ppp {ppp}");
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 12 (parte de `px-ui`): el panel y el lienzo
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_12_un_clic_dentro_del_panel_no_llega_al_lienzo_y_uno_fuera_si() {
    for ppp in PPPS {
        let mut banco = Banco::nuevo(ppp, documento(3), true);
        // Un hueco del panel (entre la barra y la lista): dentro del rectángulo, sin control.
        let hueco = banco.centro(RectU {
            x: 90,
            y: 18,
            w: 2,
            h: 2,
        });
        let pasos = banco.clic(hueco);
        assert!(
            pasos.iter().all(|p| p.lienzo.is_empty()),
            "ppp {ppp}: dentro: {:?}",
            pasos.iter().map(|p| &p.lienzo).collect::<Vec<_>>()
        );
        assert_eq!(Banco::eventos_de(&pasos), vec![]);
        // La rueda dentro del panel tampoco hace zoom en el lienzo.
        let zoom = banco.canvas.zoom();
        banco.paso(vec![mover(hueco)]);
        banco.paso(vec![rueda_linea(true)]);
        assert_eq!(banco.canvas.zoom(), zoom, "ppp {ppp}");
        // Fuera del panel (arriba a la izquierda, sobre la imagen) el clic empieza un trazo.
        let fuera = Pos2::new(20.0, 20.0);
        let pasos = banco.clic(fuera);
        assert!(
            pasos.iter().any(|p| p
                .lienzo
                .iter()
                .any(|e| matches!(e, CanvasInput::Stroke(CanvasEvent::StrokeBegin(_))))),
            "ppp {ppp}: fuera del panel el clic debe llegar al lienzo"
        );
    }
}

#[test]
fn fila_12_con_el_panel_cerrado_no_tapa_ni_responde() {
    let mut banco = Banco::nuevo(1.0, documento(3), false);
    let paso = banco.paso(vec![]);
    assert!(paso.eventos.is_empty());
    // Con el panel cerrado, un clic donde estaría el panel es del lienzo.
    let donde = Pos2::new(600.0, 100.0);
    let pasos = banco.clic(donde);
    assert!(pasos.iter().any(|p| !p.lienzo.is_empty()));
}

// ---------------------------------------------------------------------------------------------
// Propiedad 4: el panel no toca el lienzo
// ---------------------------------------------------------------------------------------------

#[test]
fn propiedad_4_sin_comando_no_cambia_el_documento_ni_se_sube_ninguna_textura() {
    for ppp in PPPS {
        let mut banco = Banco::nuevo(ppp, documento(9), true);
        let copia = banco.doc.clone();
        let id = banco.doc.state_id();
        let mut subidos = 0;
        let barra = banco.centro(title_u());
        let lista = banco.centro(name_u(2));
        let secuencia: Vec<Vec<Event>> = vec![
            vec![mover(lista)],
            vec![rueda_linea(false)],
            vec![rueda_linea(false)],
            vec![rueda_linea(true)],
            vec![mover(barra)],
            vec![boton(barra, true)],
            vec![mover(barra + vec2(30.0, 30.0))],
            vec![boton(barra + vec2(30.0, 30.0), false)],
            vec![mover(Pos2::new(5.0, 5.0))],
            vec![],
        ];
        for eventos in secuencia {
            let paso = banco.paso(eventos);
            subidos += paso.subidos;
            assert!(
                paso.eventos
                    .iter()
                    .all(|e| matches!(e, PanelEvent::Select(_))),
                "ppp {ppp}: {:?}",
                paso.eventos
            );
        }
        // Cerrar y abrir tampoco.
        banco.estado.open = false;
        subidos += banco.paso(vec![]).subidos;
        banco.estado.open = true;
        subidos += banco.paso(vec![]).subidos;
        assert_eq!(
            subidos, 0,
            "ppp {ppp}: se subieron {subidos} píxeles de textura"
        );
        assert_eq!(banco.doc, copia);
        assert_eq!(banco.doc.state_id(), id);
        assert!(!banco.doc.can_redo());
    }
}
