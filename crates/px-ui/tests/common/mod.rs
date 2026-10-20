//! Ayudantes compartidos por los bancos de egui de la unidad 17 (specs/hito-2-menu-herramientas.md):
//! eventos sintéticos, contexto con la fuente instalada y lectura de lo pintado (textos con el
//! color de cada tramo, rectángulos rellenos y su `clip_rect`).
//!
//! Cada fichero de `tests/*.rs` es un binario de test distinto y solo usa una parte de este
//! módulo, así que lo que uno no necesita le saldría como código muerto; de ahí el `allow`
//! (como en `px-golden/tests/common`).
//!
//! Todo se ejecuta con `egui::Context::run_ui` y `RawInput` sintético: sin GPU, sin ventana y sin
//! capturas de pantalla.
#![allow(dead_code)]

pub mod texturas;
pub mod zonas;

use egui::epaint::{ClippedShape, Shape};
use egui::{
    Color32, Context, Event, Key, MissingGlyphPolicy, Modifiers, MouseWheelSource, MouseWheelUnit,
    PointerButton, Pos2, RawInput, Rect, TouchPhase, Ui, Vec2, pos2, vec2,
};
use px_ui::text::{apply_scale, install_fonts, text_scale};

/// Las cinco escalas de pantalla de la spec.
pub const PPPS: [f32; 5] = [1.0, 1.25, 1.5, 2.0, 3.0];

/// Tolerancia del error de `f32` al pasar de puntos a píxeles físicos.
pub const TOLERANCIA: f32 = 0.001;

pub fn cerca(a: f32, b: f32) -> bool {
    (a - b).abs() <= TOLERANCIA
}

/// `n = text_scale(ppp)` como `i64`.
pub fn n_de(ppp: f32) -> i64 {
    i64::from(text_scale(ppp))
}

/// Un valor en puntos es un número entero de píxeles físicos (error de `f32` ≤ 0,001).
pub fn es_entero_fisico(puntos: f32, ppp: f32) -> bool {
    let fisico = puntos * ppp;
    (fisico - fisico.round()).abs() <= TOLERANCIA
}

/// Tolerancia, en píxeles físicos, de lo que egui coloca por su cuenta: `Panel`, `allocate_rect` y
/// `allocate_exact_size` redondean a un múltiplo par de `emath::GUI_ROUNDING` (1/16 pt), así que a
/// `ppp` 1,25 y 1,5 un borde puede caer hasta 1/32 pt (`ppp/32` px) del entero. Lo que se calcula
/// sin egui usa [`TOLERANCIA`].
pub fn tolerancia_de_egui(ppp: f32) -> f32 {
    ppp / 32.0 + TOLERANCIA
}

/// Como [`es_entero_fisico`] con la tolerancia del redondeo de egui.
pub fn es_entero_fisico_en_egui(puntos: f32, ppp: f32) -> bool {
    let fisico = puntos * ppp;
    (fisico - fisico.round()).abs() <= tolerancia_de_egui(ppp)
}

// ---------------------------------------------------------------------------------------------
// Eventos sintéticos
// ---------------------------------------------------------------------------------------------

pub fn mover(pos: Pos2) -> Event {
    Event::PointerMoved(pos)
}

pub fn boton(pos: Pos2, pressed: bool) -> Event {
    boton_de(pos, PointerButton::Primary, pressed)
}

pub fn boton_de(pos: Pos2, button: PointerButton, pressed: bool) -> Event {
    Event::PointerButton {
        pos,
        button,
        pressed,
        modifiers: Modifiers::NONE,
    }
}

pub fn tecla(key: Key, modifiers: Modifiers) -> Event {
    Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers,
    }
}

/// Una muesca de la rueda, en líneas (`arriba`: acerca).
pub fn rueda_linea(arriba: bool) -> Event {
    Event::MouseWheel {
        unit: MouseWheelUnit::Line,
        delta: vec2(0.0, if arriba { 1.0 } else { -1.0 }),
        phase: TouchPhase::Move,
        source: MouseWheelSource::Unknown,
        modifiers: Modifiers::NONE,
    }
}

/// La rueda en puntos: `delta.y = −px / ppp` desplaza `px` píxeles físicos hacia abajo.
pub fn rueda_puntos(px_abajo: i64, ppp: f32) -> Event {
    Event::MouseWheel {
        unit: MouseWheelUnit::Point,
        delta: vec2(0.0, -(px_abajo as f32) / ppp),
        phase: TouchPhase::Move,
        source: MouseWheelSource::Unknown,
        modifiers: Modifiers::NONE,
    }
}

/// Posición en puntos de una coordenada en píxeles físicos.
pub fn en_puntos(ppp: f32, x_px: i64, y_px: i64) -> Pos2 {
    pos2(x_px as f32 / ppp, y_px as f32 / ppp)
}

/// Modificadores de `command` como los entrega egui en cada plataforma: `ctrl` y `command` en
/// Linux y Windows, `mac_cmd` y `command` en macOS.
pub fn modificadores_command(mac: bool) -> Modifiers {
    if mac {
        Modifiers {
            mac_cmd: true,
            command: true,
            ..Modifiers::NONE
        }
    } else {
        Modifiers {
            ctrl: true,
            command: true,
            ..Modifiers::NONE
        }
    }
}

/// Las 32 combinaciones de los cinco modificadores de egui.
pub fn modificadores(bits: u8) -> Modifiers {
    Modifiers {
        alt: bits & 1 != 0,
        ctrl: bits & 2 != 0,
        shift: bits & 4 != 0,
        mac_cmd: bits & 8 != 0,
        command: bits & 16 != 0,
    }
}

// ---------------------------------------------------------------------------------------------
// Contexto y fotogramas
// ---------------------------------------------------------------------------------------------

/// Contexto a `ppp` con la fuente instalada, la escala aplicada y `MissingGlyphPolicy::Panic`
/// (un glifo ausente en texto fijo es un fallo), ya calentado.
pub fn contexto(ppp: f32) -> Context {
    let ctx = Context::default();
    ctx.set_pixels_per_point(ppp);
    ctx.set_missing_glyph_policy(MissingGlyphPolicy::Panic);
    install_fonts(&ctx);
    for _ in 0..2 {
        let entrada = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(100.0, 100.0))),
            ..RawInput::default()
        };
        let salida = ctx.run_ui(entrada, |ui| apply_scale(ui.ctx()));
        salida.drop_without_applying_deltas();
    }
    ctx
}

/// Ejecuta un fotograma de `pantalla` puntos con `events`, aplica `dibuja` y devuelve lo pintado
/// (sin los cambios de textura, que egui exige descartar).
pub fn fotograma(
    ctx: &Context,
    pantalla: Vec2,
    events: Vec<Event>,
    dibuja: impl FnMut(&mut Ui),
) -> Vec<ClippedShape> {
    let entrada = RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, pantalla)),
        events,
        ..RawInput::default()
    };
    let mut salida = ctx.run_ui(entrada, dibuja);
    salida.textures_delta.clear();
    std::mem::take(&mut salida.shapes)
}

// ---------------------------------------------------------------------------------------------
// Lo pintado
// ---------------------------------------------------------------------------------------------

/// Un tramo de texto de un solo color dentro de una forma de texto.
#[derive(Clone, Debug)]
pub struct Tramo {
    pub texto: String,
    pub color: Color32,
}

/// Una forma de texto: su esquina superior izquierda, su tamaño, su recorte y sus tramos.
#[derive(Clone, Debug)]
pub struct Texto {
    pub texto: String,
    pub pos: Pos2,
    pub tamano: Vec2,
    pub clip: Rect,
    pub tramos: Vec<Tramo>,
    /// `opacity_factor` de la forma (egui atenúa lo deshabilitado con él o con el color).
    pub opacidad: f32,
}

impl Texto {
    /// Color del primer tramo y opacidad: lo que cambia al atenuar un texto.
    pub fn aspecto(&self) -> (Color32, u32) {
        (self.color(), self.opacidad.to_bits())
    }

    /// Color del primer tramo (el de todo el texto si solo hay uno).
    pub fn color(&self) -> Color32 {
        self.tramos
            .first()
            .map_or(Color32::PLACEHOLDER, |t| t.color)
    }

    /// Rectángulo que ocupa, en puntos.
    pub fn rect(&self) -> Rect {
        Rect::from_min_size(self.pos, self.tamano)
    }
}

/// Un rectángulo relleno y su recorte.
#[derive(Clone, Copy, Debug)]
pub struct Relleno {
    pub rect: Rect,
    pub fill: Color32,
    pub clip: Rect,
}

#[derive(Clone, Debug, Default)]
pub struct Pintado {
    pub textos: Vec<Texto>,
    pub rellenos: Vec<Relleno>,
}

impl Pintado {
    /// Alguna forma de texto es exactamente `buscado`.
    pub fn pinta(&self, buscado: &str) -> bool {
        self.textos.iter().any(|t| t.texto == buscado)
    }

    /// La primera forma de texto que contiene `buscado`.
    pub fn texto_con(&self, buscado: &str) -> Option<&Texto> {
        self.textos.iter().find(|t| t.texto.contains(buscado))
    }

    /// Un tramo de color de **cualquier** forma de texto cuyo texto recortado es `buscado`
    /// (sirve tanto si la letra va en su propia forma como si va en otro tramo de la misma).
    pub fn tramo(&self, buscado: &str) -> Option<&Tramo> {
        self.textos
            .iter()
            .flat_map(|t| t.tramos.iter())
            .find(|tramo| tramo.texto.trim() == buscado)
    }

    /// Todo el texto pintado, forma a forma.
    pub fn todos_los_textos(&self) -> Vec<&str> {
        self.textos.iter().map(|t| t.texto.as_str()).collect()
    }
}

fn recoge(shape: &Shape, clip: Rect, fuera: &mut Pintado) {
    match shape {
        Shape::Text(t) => {
            let job = &t.galley.job;
            let tramos = job
                .sections
                .iter()
                .map(|s| Tramo {
                    texto: job.text[s.byte_range.start.0..s.byte_range.end.0].to_owned(),
                    color: t.override_text_color.unwrap_or(
                        if s.format.color == Color32::PLACEHOLDER {
                            t.fallback_color
                        } else {
                            s.format.color
                        },
                    ),
                })
                .collect();
            fuera.textos.push(Texto {
                texto: t.galley.text().to_owned(),
                pos: t.pos,
                tamano: t.galley.size(),
                clip,
                tramos,
                opacidad: t.opacity_factor,
            });
        }
        Shape::Rect(r) => fuera.rellenos.push(Relleno {
            rect: r.rect,
            fill: r.fill,
            clip,
        }),
        Shape::Vec(formas) => {
            for forma in formas {
                recoge(forma, clip, fuera);
            }
        }
        _ => {}
    }
}

/// Reúne los textos y los rectángulos rellenos de las formas del fotograma.
pub fn pintado(formas: &[ClippedShape]) -> Pintado {
    let mut fuera = Pintado::default();
    for clipped in formas {
        recoge(&clipped.shape, clipped.clip_rect, &mut fuera);
    }
    fuera
}
