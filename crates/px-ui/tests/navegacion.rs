//! Tests de la navegación del lienzo con la tabla de gestos (specs/interfaz-raton.md, casos 5, 6,
//! 8 (en el lienzo), 10, 11 y 12 y propiedad 3): un `egui::Context` con `RawInput` sintético a 60
//! fotogramas por segundo, sin GPU ni pantalla. El lienzo es toda la pantalla de 800×600 con una
//! imagen de 64×48 (ajuste = ×12), `ppp` 1,0 salvo donde se dice.

use egui::{
    Context, Event, Modifiers, MouseWheelSource, MouseWheelUnit, PointerButton, Pos2, RawInput,
    Rect, TouchPhase, Vec2, pos2, vec2,
};
use proptest::prelude::*;
use px_core::{Image, Mode};
use px_ui::canvas::{Canvas, CanvasInfo, CanvasInput};
use px_ui::color::Role;
use px_ui::gestures::{Button, GestureAction, GestureMap, GestureRow, Input, Mods, Scope};
use px_ui::keymap::Platforms;
use px_ui::view::Zoom;

const SCREEN: Vec2 = vec2(800.0, 600.0);
const CURSOR: Pos2 = pos2(300.0, 200.0);

struct Banco {
    ctx: Context,
    canvas: Canvas,
    t: f64,
    info: CanvasInfo,
    entradas: Vec<(CanvasInput, Role)>,
}

impl Banco {
    fn nuevo() -> Banco {
        Banco::con_ppp(1.0)
    }

    fn con_ppp(ppp: f32) -> Banco {
        let ctx = Context::default();
        ctx.set_pixels_per_point(ppp);
        let mut canvas = Canvas::new();
        canvas.set_image(&Image::new(64, 48, Mode::Rgba).expect("imagen"));
        let mut banco = Banco {
            ctx,
            canvas,
            t: 0.0,
            info: CanvasInfo {
                zoom: Zoom::default(),
                hover: None,
                image_rect: None,
                error: None,
            },
            entradas: Vec::new(),
        };
        banco.paso(vec![]);
        banco
    }

    /// Un banco con el cursor ya sobre el lienzo.
    fn con_cursor() -> Banco {
        let mut banco = Banco::nuevo();
        banco.paso(vec![Event::PointerMoved(CURSOR)]);
        banco
    }

    fn paso(&mut self, events: Vec<Event>) -> Vec<(CanvasInput, Role)> {
        self.t += 1.0 / 60.0;
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SCREEN)),
            time: Some(self.t),
            predicted_dt: 1.0 / 60.0,
            events,
            ..RawInput::default()
        };
        let canvas = &mut self.canvas;
        let mut resultado = None;
        let mut salida = self.ctx.run_ui(input, |ui| {
            let entradas = canvas.interact_roles(ui);
            resultado = Some((entradas, canvas.paint(ui)));
        });
        salida.textures_delta.clear();
        let (entradas, info) = resultado.expect("el lienzo se dibuja");
        self.info = info;
        self.entradas.extend(entradas.iter().copied());
        entradas
    }

    fn zoom(&self) -> Zoom {
        self.info.zoom
    }

    fn origen(&self) -> Pos2 {
        self.info.image_rect.expect("hay imagen").min
    }
}

fn evento_rueda(
    unit: MouseWheelUnit,
    delta: Vec2,
    modifiers: Modifiers,
    source: MouseWheelSource,
) -> Event {
    Event::MouseWheel {
        unit,
        delta,
        phase: TouchPhase::Move,
        source,
        modifiers,
    }
}

fn linea(dy: f32) -> Event {
    evento_rueda(
        MouseWheelUnit::Line,
        vec2(0.0, dy),
        Modifiers::NONE,
        MouseWheelSource::Unknown,
    )
}

fn dedos(dx: f32, dy: f32) -> Event {
    evento_rueda(
        MouseWheelUnit::Point,
        vec2(dx, dy),
        Modifiers::NONE,
        MouseWheelSource::Trackpad,
    )
}

fn boton(pos: Pos2, button: PointerButton, pressed: bool, modifiers: Modifiers) -> Event {
    Event::PointerButton {
        pos,
        button,
        pressed,
        modifiers,
    }
}

fn espacio(pressed: bool) -> Event {
    Event::Key {
        key: egui::Key::Space,
        physical_key: Some(egui::Key::Space),
        pressed,
        repeat: false,
        modifiers: Modifiers::NONE,
    }
}

fn pasos_de_zoom(desde: Zoom, hasta: Zoom) -> i32 {
    let (mut z, mut n) = (desde, 0);
    while z != hasta && n < 200 {
        z = if hasta > z { z.zoom_in() } else { z.zoom_out() };
        n += if hasta > desde { 1 } else { -1 };
    }
    n
}

// --- Fila 5 ---

#[test]
fn fila_5_una_muesca_es_un_paso_anclado_y_tres_muescas_de_golpe_son_tres() {
    let mut b = Banco::con_cursor();
    let ajuste = b.zoom();
    let pixel = b.info.hover.expect("el cursor está sobre la imagen");
    b.paso(vec![linea(1.0)]);
    assert_eq!(b.zoom(), ajuste.zoom_in());
    assert_eq!(
        b.info.hover,
        Some(pixel),
        "el píxel bajo el cursor se conserva"
    );
    let antes = b.zoom();
    b.paso(vec![linea(3.0)]);
    assert_eq!(pasos_de_zoom(antes, b.zoom()), 3, "antes de I.2b eran 1");
    assert_eq!(b.info.hover, Some(pixel));
    let antes = b.zoom();
    b.paso(vec![linea(-1.0), linea(-1.0)]);
    assert_eq!(pasos_de_zoom(antes, b.zoom()), -2, "un paso por muesca");
}

#[test]
fn fila_5_los_dedos_en_puntos_desplazan_y_no_hacen_zoom() {
    let mut b = Banco::con_cursor();
    let (zoom, origen) = (b.zoom(), b.origen());
    for _ in 0..30 {
        b.paso(vec![dedos(0.0, 6.0)]);
    }
    assert_eq!(b.zoom(), zoom, "antes de I.2b eran 30 pasos de zoom");
    assert_ne!(b.origen(), origen, "la imagen se desplaza");
    assert!(b.entradas.is_empty(), "sin eventos de trazo");
}

#[test]
fn fila_5_mayus_mas_rueda_desplaza_en_horizontal_sin_zoom() {
    let mut b = Banco::con_cursor();
    let (zoom, origen) = (b.zoom(), b.origen());
    b.paso(vec![evento_rueda(
        MouseWheelUnit::Line,
        vec2(0.0, 1.0),
        Modifiers::SHIFT,
        MouseWheelSource::Unknown,
    )]);
    assert_eq!(b.zoom(), zoom);
    assert_eq!(
        b.origen(),
        origen + vec2(40.0, 0.0),
        "40 puntos por muesca, a la derecha"
    );
}

#[test]
fn fila_5_ctrl_mas_una_muesca_da_un_solo_paso() {
    let mut b = Banco::con_cursor();
    let ajuste = b.zoom();
    b.paso(vec![evento_rueda(
        MouseWheelUnit::Line,
        vec2(0.0, 1.0),
        Modifiers::COMMAND,
        MouseWheelSource::Unknown,
    )]);
    for _ in 0..40 {
        b.paso(vec![]);
    }
    assert_eq!(pasos_de_zoom(ajuste, b.zoom()), 1, "antes de I.2b eran 11");
}

#[test]
fn fila_5_un_pellizco_lento_da_tres_pasos() {
    let mut b = Banco::con_cursor();
    let ajuste = b.zoom();
    for _ in 0..30 {
        b.paso(vec![Event::Zoom(1.02)]);
    }
    for _ in 0..5 {
        b.paso(vec![]);
    }
    assert_eq!(pasos_de_zoom(ajuste, b.zoom()), 3, "antes de I.2b eran 30");
}

#[test]
fn fila_5_sin_cursor_sobre_el_lienzo_la_rueda_no_hace_nada() {
    let mut b = Banco::nuevo();
    let (zoom, origen) = (b.zoom(), b.origen());
    b.paso(vec![linea(1.0), dedos(0.0, 20.0), Event::Zoom(1.5)]);
    assert_eq!(b.zoom(), zoom);
    assert_eq!(b.origen(), origen);
}

// --- Fila 6 ---

#[test]
fn fila_6_central_y_espacio_mas_izquierdo_desplazan_sin_trazo() {
    for (boton_pan, con_espacio) in [
        (PointerButton::Middle, false),
        (PointerButton::Primary, true),
    ] {
        let mut b = Banco::con_cursor();
        let origen = b.origen();
        if con_espacio {
            b.paso(vec![espacio(true)]);
        }
        b.paso(vec![boton(CURSOR, boton_pan, true, Modifiers::NONE)]);
        b.paso(vec![Event::PointerMoved(CURSOR + vec2(10.0, 5.0))]);
        b.paso(vec![Event::PointerMoved(CURSOR + vec2(20.0, 10.0))]);
        b.paso(vec![boton(
            CURSOR + vec2(20.0, 10.0),
            boton_pan,
            false,
            Modifiers::NONE,
        )]);
        assert!(b.entradas.is_empty(), "{boton_pan:?}: sin eventos de trazo");
        assert!(
            b.origen().x > origen.x && b.origen().y > origen.y,
            "{boton_pan:?}"
        );
    }
}

#[test]
fn fila_6_los_dedos_desplazan_con_un_trazo_en_curso_y_el_trazo_sigue() {
    let mut b = Banco::con_cursor();
    let origen = b.origen();
    b.paso(vec![boton(
        CURSOR,
        PointerButton::Primary,
        true,
        Modifiers::NONE,
    )]);
    b.paso(vec![dedos(0.0, 8.0)]);
    assert_eq!(b.origen(), origen + vec2(0.0, 8.0), "la vista se mueve");
    b.paso(vec![Event::PointerMoved(CURSOR + vec2(12.0, 0.0))]);
    b.paso(vec![boton(
        CURSOR + vec2(12.0, 0.0),
        PointerButton::Primary,
        false,
        Modifiers::NONE,
    )]);
    let tiene = |f: fn(&CanvasInput) -> bool| b.entradas.iter().any(|(e, _)| f(e));
    assert!(tiene(|e| matches!(
        e,
        CanvasInput::Stroke(px_ui::canvas::CanvasEvent::StrokeBegin(_))
    )));
    assert!(tiene(|e| matches!(
        e,
        CanvasInput::Stroke(px_ui::canvas::CanvasEvent::StrokeEnd)
    )));
    assert!(!tiene(|e| matches!(e, CanvasInput::Escape)));
}

#[test]
fn fila_6_el_desplazamiento_es_entero_en_pixeles_fisicos_y_clamp_manda() {
    for ppp in [1.0f32, 1.25, 1.5, 2.0] {
        let mut b = Banco::con_ppp(ppp);
        b.paso(vec![Event::PointerMoved(CURSOR)]);
        let a = b.origen();
        b.paso(vec![dedos(0.0, 7.3)]);
        let d = (b.origen().y - a.y) * ppp;
        assert!((d - d.round()).abs() < 1e-3, "ppp {ppp}: {d} px físicos");
        // Un desplazamiento enorme no saca la imagen del lienzo.
        b.paso(vec![dedos(5000.0, 5000.0), dedos(-90000.0, 0.0)]);
        let rect = b.info.image_rect.expect("hay imagen");
        assert!(
            rect.intersects(Rect::from_min_size(Pos2::ZERO, SCREEN)),
            "ppp {ppp}"
        );
    }
}

// --- Fila 8 (en el lienzo): tabla sustituida ---

fn fila_de(input: Input, action: GestureAction) -> GestureRow {
    GestureRow {
        input,
        mods: Mods {
            command: None,
            shift: None,
            space: None,
        },
        action,
        scope: Scope::Canvas,
        active: Platforms::All,
    }
}

#[test]
fn fila_8_con_una_tabla_vacia_el_lienzo_no_responde_a_nada() {
    let mut b = Banco::con_cursor();
    b.canvas.set_gestures(GestureMap::new(vec![]));
    let (zoom, origen) = (b.zoom(), b.origen());
    b.paso(vec![linea(1.0), dedos(0.0, 9.0), Event::Zoom(1.5)]);
    b.paso(vec![boton(
        CURSOR,
        PointerButton::Primary,
        true,
        Modifiers::NONE,
    )]);
    b.paso(vec![boton(
        CURSOR,
        PointerButton::Primary,
        false,
        Modifiers::NONE,
    )]);
    b.paso(vec![boton(
        CURSOR,
        PointerButton::Middle,
        true,
        Modifiers::NONE,
    )]);
    b.paso(vec![Event::PointerMoved(CURSOR + vec2(30.0, 30.0))]);
    assert_eq!(b.zoom(), zoom);
    assert_eq!(b.origen(), origen);
    assert!(b.entradas.is_empty());
}

#[test]
fn fila_8_rueda_asignada_a_desplazar_y_dedos_a_zoom() {
    let mut b = Banco::con_cursor();
    b.canvas.set_gestures(GestureMap::new(vec![
        fila_de(Input::Wheel, GestureAction::Pan),
        fila_de(Input::Scroll, GestureAction::ZoomStep),
    ]));
    let (zoom, origen) = (b.zoom(), b.origen());
    b.paso(vec![linea(1.0)]);
    assert_eq!(b.zoom(), zoom, "la rueda ya no hace zoom");
    assert_eq!(
        b.origen(),
        origen + vec2(0.0, 40.0),
        "desplaza 40 puntos por muesca"
    );
    let antes = b.zoom();
    b.paso(vec![dedos(0.0, 20.0)]);
    assert_eq!(
        b.zoom(),
        antes,
        "media muesca (20 de 40 puntos) no cruza el umbral"
    );
    b.paso(vec![dedos(0.0, 20.0)]);
    assert_eq!(
        b.zoom(),
        antes.zoom_in(),
        "40 puntos de dedos = una muesca = un paso"
    );
}

// --- Fila 10 ---

#[test]
fn fila_10_el_derecho_pinta_con_el_fondo_con_y_sin_espacio_y_alt_no_cuenta() {
    for (espacio_abajo, alt) in [(false, false), (true, false), (false, true), (true, true)] {
        let mut b = Banco::con_cursor();
        if espacio_abajo {
            b.paso(vec![espacio(true)]);
        }
        let m = if alt { Modifiers::ALT } else { Modifiers::NONE };
        let e = b.paso(vec![boton(CURSOR, PointerButton::Secondary, true, m)]);
        assert!(
            matches!(
                e.as_slice(),
                [(
                    CanvasInput::Stroke(px_ui::canvas::CanvasEvent::StrokeBegin(_)),
                    Role::Background
                )]
            ),
            "espacio {espacio_abajo}, alt {alt}: {e:?}"
        );
    }
}

#[test]
fn fila_10_alt_no_cambia_la_rueda_ni_el_izquierdo_y_mayus_izquierdo_dibuja() {
    let mut a = Banco::con_cursor();
    let mut b = Banco::con_cursor();
    a.paso(vec![linea(1.0)]);
    b.paso(vec![evento_rueda(
        MouseWheelUnit::Line,
        vec2(0.0, 1.0),
        Modifiers::ALT,
        MouseWheelSource::Unknown,
    )]);
    assert_eq!(a.zoom(), b.zoom(), "Alt no cuenta en la rueda");
    for m in [Modifiers::NONE, Modifiers::ALT, Modifiers::SHIFT] {
        let mut b = Banco::con_cursor();
        let e = b.paso(vec![boton(CURSOR, PointerButton::Primary, true, m)]);
        assert!(
            matches!(
                e.as_slice(),
                [(
                    CanvasInput::Stroke(px_ui::canvas::CanvasEvent::StrokeBegin(_)),
                    Role::Foreground
                )]
            ),
            "{m:?}: {e:?}"
        );
    }
}

// --- Fila 11 ---

#[test]
fn fila_11_el_signo_lo_da_el_sistema_y_la_app_no_lo_invierte() {
    // (dx, dy) del delta → cuánto se mueve la imagen: el mismo signo.
    for (dx, dy) in [(0.0, 6.0), (0.0, -6.0), (6.0, 0.0), (-6.0, 0.0)] {
        let mut b = Banco::con_cursor();
        let origen = b.origen();
        b.paso(vec![dedos(dx, dy)]);
        assert_eq!(b.origen() - origen, vec2(dx, dy), "delta ({dx}, {dy})");
    }
    // Con el desplazamiento natural de macOS el sistema ya entrega el signo contrario para el mismo
    // movimiento de dedos: movimientos opuestos, sin ramas por `source`.
    for source in [
        MouseWheelSource::Unknown,
        MouseWheelSource::Wheel,
        MouseWheelSource::Trackpad,
        MouseWheelSource::Momentum,
    ] {
        let mover = |dy: f32| {
            let mut b = Banco::con_cursor();
            let origen = b.origen();
            b.paso(vec![evento_rueda(
                MouseWheelUnit::Point,
                vec2(0.0, dy),
                Modifiers::NONE,
                source,
            )]);
            b.origen().y - origen.y
        };
        assert_eq!(mover(9.0), 9.0, "{source:?}");
        assert_eq!(mover(-9.0), -9.0, "{source:?}");
    }
}

#[test]
fn fila_11_mayus_mas_linea_suma_los_dos_ejes() {
    for (dx, dy, esperado) in [
        (0.0, 1.0, 40.0),
        (1.0, 0.0, 40.0),
        (0.0, -1.0, -40.0),
        (-1.0, 0.0, -40.0),
    ] {
        let mut b = Banco::con_cursor();
        let origen = b.origen();
        b.paso(vec![evento_rueda(
            MouseWheelUnit::Line,
            vec2(dx, dy),
            Modifiers::SHIFT,
            MouseWheelSource::Unknown,
        )]);
        assert_eq!(
            b.origen() - origen,
            vec2(esperado, 0.0),
            "delta ({dx}, {dy})"
        );
    }
}

// --- Fila 12 ---

#[test]
fn fila_12_los_restos_de_medio_pixel_se_arrastran() {
    // 7 eventos de 0,4 pt: a ppp 1,0 son 2,8 px (se mueven 2); a 1,5, 4,2 (4); a 2,0, 5,6 (5).
    for (ppp, fisicos) in [(1.0f32, 2.0f32), (1.5, 4.0), (2.0, 5.0)] {
        for signo in [1.0f32, -1.0] {
            let mut b = Banco::con_ppp(ppp);
            b.paso(vec![Event::PointerMoved(CURSOR)]);
            let origen = b.origen();
            for _ in 0..7 {
                b.paso(vec![dedos(0.0, 0.4 * signo)]);
            }
            let movido = (b.origen().y - origen.y) * ppp;
            assert!(
                (movido - fisicos * signo).abs() < 1e-3,
                "ppp {ppp}, signo {signo}: {movido} px físicos"
            );
            assert_eq!(b.origen().x, origen.x);
        }
    }
}

#[test]
fn fila_12_el_resto_se_arrastra_por_eje() {
    let mut b = Banco::con_cursor();
    let origen = b.origen();
    for _ in 0..3 {
        b.paso(vec![dedos(0.4, 0.0)]);
    }
    for _ in 0..3 {
        b.paso(vec![dedos(0.0, 0.4)]);
    }
    assert_eq!(
        b.origen() - origen,
        vec2(1.0, 1.0),
        "1,2 px en cada eje: uno entero"
    );
}

// --- Propiedad 3 ---

fn evento_de_navegacion() -> impl Strategy<Value = Event> {
    let modificadores = (any::<bool>(), any::<bool>()).prop_map(|(c, s)| Modifiers {
        command: c,
        ctrl: c,
        shift: s,
        ..Modifiers::NONE
    });
    let unidad = prop::sample::select(vec![
        MouseWheelUnit::Line,
        MouseWheelUnit::Point,
        MouseWheelUnit::Page,
    ]);
    prop_oneof![
        (unidad, -6.0f32..6.0, -6.0f32..6.0, modificadores)
            .prop_map(|(u, x, y, m)| { evento_rueda(u, vec2(x, y), m, MouseWheelSource::Unknown) }),
        (0.5f32..2.0).prop_map(Event::Zoom),
        any::<bool>().prop_map(espacio),
        any::<bool>().prop_map(|p| boton(CURSOR, PointerButton::Middle, p, Modifiers::NONE)),
        (any::<bool>(), any::<bool>()).prop_map(|(a, p)| {
            let b = if a {
                PointerButton::Extra1
            } else {
                PointerButton::Extra2
            };
            boton(CURSOR, b, p, Modifiers::NONE)
        }),
        (0.0f32..800.0, 0.0f32..600.0).prop_map(|(x, y)| Event::PointerMoved(pos2(x, y))),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Propiedad 3: navegar no dibuja y la vista siempre vale.
    #[test]
    fn propiedad_3_navegar_no_dibuja_y_la_vista_siempre_vale(
        eventos in prop::collection::vec(evento_de_navegacion(), 0..14),
    ) {
        let mut b = Banco::con_cursor();
        for e in eventos {
            b.paso(vec![e]);
            prop_assert!(b.zoom() >= Zoom::MIN && b.zoom() <= Zoom::MAX);
            let rect = b.info.image_rect.expect("hay imagen");
            prop_assert!(rect.intersects(Rect::from_min_size(Pos2::ZERO, SCREEN)), "{rect:?}");
        }
        b.paso(vec![]);
        prop_assert!(b.entradas.is_empty(), "eventos de trazo: {:?}", b.entradas);
    }
}
