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

pub mod icons;
pub mod textures;
pub mod toolbox;
pub mod zones;

use egui::epaint::{ClippedShape, Shape};
use egui::{
    Color32, Context, Event, Key, MissingGlyphPolicy, Modifiers, MouseWheelSource, MouseWheelUnit,
    PointerButton, Pos2, RawInput, Rect, TextureId, TouchPhase, Ui, Vec2, pos2, vec2,
};
use px_ui::text::{apply_scale, install_fonts, text_scale};

/// Las cinco escalas de pantalla de la spec.
pub const PPPS: [f32; 5] = [1.0, 1.25, 1.5, 2.0, 3.0];

/// Tolerancia del error de `f32` al pasar de puntos a píxeles físicos.
pub const TOLERANCE: f32 = 0.001;

pub fn near(a: f32, b: f32) -> bool {
    (a - b).abs() <= TOLERANCE
}

/// `n = text_scale(ppp)` como `i64`.
pub fn n_of(ppp: f32) -> i64 {
    i64::from(text_scale(ppp))
}

/// Un valor en puntos es un número entero de píxeles físicos (error de `f32` ≤ 0,001).
pub fn is_integer_physical(points: f32, ppp: f32) -> bool {
    let physical = points * ppp;
    (physical - physical.round()).abs() <= TOLERANCE
}

/// Tolerancia, en píxeles físicos, de lo que egui coloca por su cuenta: `Panel`, `allocate_rect` y
/// `allocate_exact_size` redondean a un múltiplo par de `emath::GUI_ROUNDING` (1/16 pt), así que a
/// `ppp` 1,25 y 1,5 un borde puede caer hasta 1/32 pt (`ppp/32` px) del entero. Lo que se calcula
/// sin egui usa [`TOLERANCE`].
pub fn tolerance_of_egui(ppp: f32) -> f32 {
    ppp / 32.0 + TOLERANCE
}

/// Como [`is_integer_physical`] con la tolerancia del redondeo de egui.
pub fn is_physical_integer_in_egui(points: f32, ppp: f32) -> bool {
    let physical = points * ppp;
    (physical - physical.round()).abs() <= tolerance_of_egui(ppp)
}

// ---------------------------------------------------------------------------------------------
// Eventos sintéticos
// ---------------------------------------------------------------------------------------------

pub fn move_pointer(pos: Pos2) -> Event {
    Event::PointerMoved(pos)
}

pub fn button_2(pos: Pos2, pressed: bool) -> Event {
    button_of_2(pos, PointerButton::Primary, pressed)
}

pub fn button_of_2(pos: Pos2, button: PointerButton, pressed: bool) -> Event {
    Event::PointerButton {
        pos,
        button,
        pressed,
        modifiers: Modifiers::NONE,
    }
}

pub fn key_2(key: Key, modifiers: Modifiers) -> Event {
    Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers,
    }
}

/// Una muesca de la rueda, en líneas (`top`: acerca).
pub fn wheel_line(top: bool) -> Event {
    Event::MouseWheel {
        unit: MouseWheelUnit::Line,
        delta: vec2(0.0, if top { 1.0 } else { -1.0 }),
        phase: TouchPhase::Move,
        source: MouseWheelSource::Unknown,
        modifiers: Modifiers::NONE,
    }
}

/// La rueda en puntos: `delta.y = −px / ppp` desplaza `px` píxeles físicos hacia abajo.
pub fn wheel_points(px_below: i64, ppp: f32) -> Event {
    Event::MouseWheel {
        unit: MouseWheelUnit::Point,
        delta: vec2(0.0, -(px_below as f32) / ppp),
        phase: TouchPhase::Move,
        source: MouseWheelSource::Unknown,
        modifiers: Modifiers::NONE,
    }
}

/// Posición en puntos de una coordenada en píxeles físicos.
pub fn in_points(ppp: f32, x_px: i64, y_px: i64) -> Pos2 {
    pos2(x_px as f32 / ppp, y_px as f32 / ppp)
}

/// Modificadores de `command` como los entrega egui en cada plataforma: `ctrl` y `command` en
/// Linux y Windows, `mac_cmd` y `command` en macOS.
pub fn modifiers_command(mac: bool) -> Modifiers {
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
pub fn modifiers_2(bits: u8) -> Modifiers {
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
pub fn context(ppp: f32) -> Context {
    let ctx = Context::default();
    ctx.set_pixels_per_point(ppp);
    ctx.set_missing_glyph_policy(MissingGlyphPolicy::Panic);
    install_fonts(&ctx);
    for _ in 0..2 {
        let input_2 = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(100.0, 100.0))),
            ..RawInput::default()
        };
        let output_2 = ctx.run_ui(input_2, |ui| apply_scale(ui.ctx()));
        output_2.drop_without_applying_deltas();
    }
    ctx
}

/// Ejecuta un fotograma de `screen` puntos con `events`, aplica `draws` y devuelve lo pintado
/// (sin los cambios de textura, que egui exige descartar).
pub fn frame(
    ctx: &Context,
    screen: Vec2,
    events: Vec<Event>,
    draws: impl FnMut(&mut Ui),
) -> Vec<ClippedShape> {
    let input_2 = RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, screen)),
        events,
        ..RawInput::default()
    };
    let mut output_2 = ctx.run_ui(input_2, draws);
    output_2.textures_delta.clear();
    std::mem::take(&mut output_2.shapes)
}

// ---------------------------------------------------------------------------------------------
// Lo pintado
// ---------------------------------------------------------------------------------------------

/// Un tramo de texto de un solo color dentro de una forma de texto.
#[derive(Clone, Debug)]
pub struct TestSpan {
    pub text_2: String,
    pub color: Color32,
}

/// Una forma de texto: su esquina superior izquierda, su tamaño, su recorte y sus tramos.
#[derive(Clone, Debug)]
pub struct Text {
    pub text_2: String,
    pub pos: Pos2,
    pub size_2: Vec2,
    pub clip: Rect,
    pub spans_2: Vec<TestSpan>,
    /// `opacity_factor` de la forma (egui atenúa lo deshabilitado con él o con el color).
    pub opacity_2: f32,
}

impl Text {
    /// Color del primer tramo y opacidad: lo que cambia al atenuar un texto.
    pub fn look(&self) -> (Color32, u32) {
        (self.color(), self.opacity_2.to_bits())
    }

    /// Color del primer tramo (el de todo el texto si solo hay uno).
    pub fn color(&self) -> Color32 {
        self.spans_2
            .first()
            .map_or(Color32::PLACEHOLDER, |t| t.color)
    }

    /// Rectángulo que ocupa, en puntos.
    pub fn rect(&self) -> Rect {
        Rect::from_min_size(self.pos, self.size_2)
    }
}

/// Un rectángulo relleno y su recorte.
#[derive(Clone, Copy, Debug)]
pub struct TestFill {
    pub rect: Rect,
    pub fill: Color32,
    pub clip: Rect,
}

/// Una malla con textura (un icono): la textura, el rectángulo que cubren sus vértices, la tinta
/// (el color de sus vértices) y su recorte.
#[derive(Clone, Copy, Debug)]
pub struct TestMesh {
    pub texture_2: TextureId,
    pub rect: Rect,
    /// Rectángulo que cubren las coordenadas de textura de sus vértices.
    pub uv: Rect,
    pub color: Color32,
    pub clip: Rect,
}

#[derive(Clone, Debug, Default)]
pub struct Painted {
    pub texts: Vec<Text>,
    pub fills: Vec<TestFill>,
    pub meshes: Vec<TestMesh>,
}

impl Painted {
    /// Alguna forma de texto es exactamente `searched`.
    pub fn paints(&self, searched: &str) -> bool {
        self.texts.iter().any(|t| t.text_2 == searched)
    }

    /// La primera forma de texto que contiene `searched`.
    pub fn text_with(&self, searched: &str) -> Option<&Text> {
        self.texts.iter().find(|t| t.text_2.contains(searched))
    }

    /// Un tramo de color de **cualquier** forma de texto cuyo texto recortado es `searched`
    /// (sirve tanto si la letra va en su propia forma como si va en otro tramo de la misma).
    pub fn span(&self, searched: &str) -> Option<&TestSpan> {
        self.texts
            .iter()
            .flat_map(|t| t.spans_2.iter())
            .find(|span| span.text_2.trim() == searched)
    }

    /// La malla con esa textura (el icono), si se pintó.
    pub fn mesh_of(&self, texture_2: TextureId) -> Option<&TestMesh> {
        self.meshes.iter().find(|m| m.texture_2 == texture_2)
    }

    /// Todo el texto pintado, forma a forma.
    pub fn all_the_texts(&self) -> Vec<&str> {
        self.texts.iter().map(|t| t.text_2.as_str()).collect()
    }
}

fn collects(shape: &Shape, clip: Rect, outside: &mut Painted) {
    match shape {
        Shape::Text(t) => {
            let job = &t.galley.job;
            let spans_2 = job
                .sections
                .iter()
                .map(|s| TestSpan {
                    text_2: job.text[s.byte_range.start.0..s.byte_range.end.0].to_owned(),
                    color: t.override_text_color.unwrap_or(
                        if s.format.color == Color32::PLACEHOLDER {
                            t.fallback_color
                        } else {
                            s.format.color
                        },
                    ),
                })
                .collect();
            outside.texts.push(Text {
                text_2: t.galley.text().to_owned(),
                pos: t.pos,
                size_2: t.galley.size(),
                clip,
                spans_2,
                opacity_2: t.opacity_factor,
            });
        }
        Shape::Rect(r) => outside.fills.push(TestFill {
            rect: r.rect,
            fill: r.fill,
            clip,
        }),
        Shape::Mesh(mesh) => {
            let points: Vec<Pos2> = mesh.vertices.iter().map(|v| v.pos).collect();
            let uvs: Vec<Pos2> = mesh.vertices.iter().map(|v| v.uv).collect();
            outside.meshes.push(TestMesh {
                texture_2: mesh.texture_id,
                rect: Rect::from_points(&points),
                uv: Rect::from_points(&uvs),
                color: mesh
                    .vertices
                    .first()
                    .map_or(Color32::PLACEHOLDER, |v| v.color),
                clip,
            });
        }
        Shape::Vec(shapes_2) => {
            for shape_2 in shapes_2 {
                collects(shape_2, clip, outside);
            }
        }
        _ => {}
    }
}

/// Reúne los textos y los rectángulos rellenos de las formas del fotograma.
pub fn painted(shapes_2: &[ClippedShape]) -> Painted {
    let mut outside = Painted::default();
    for clipped in shapes_2 {
        collects(&clipped.shape, clipped.clip_rect, &mut outside);
    }
    outside
}
