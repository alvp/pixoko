//! Tests del panel de paletas con los botones y las operaciones de la unidad 11d, dentro de la
//! ventana (specs/hito-2-paleta-operaciones.md, tabla «Casos», filas 2, 3 a 6, 7, 9, 10, 13 y 14
//! en lo que toca al panel y al banco). Lo puro (`palette_ops`, `Grid::drop_index`, `MoveDrag`,
//! `button_hit`, `edges_u`) se prueba en `palette_ops.rs`, `palette_grid.rs` y `edit_layout.rs`;
//! `Session` y `apply_palette_op`, en `px-app`. Aquí se prueba que `show_editing_ops` lo une bien
//! a `egui`: qué se pinta, dónde se pulsa y qué `PanelOutput` sale.
//!
//! El banco es el de `tests/panel_baja.rs`, copiado (ese fichero no se toca) y ampliado: un
//! `egui::Context` ejecuta fotogramas con `RawInput` sintético, sin GPU ni pantalla. En cada
//! fotograma, como hace `main`, se llama a `PaletteOps::sync(doc_state, len)` con un `doc_state`
//! que controla el test, a `ColorEditor::sync`, se monta `status::show`, luego
//! `palette_panel::show_editing_ops` con un `PanelView` y un `PaletteOps` que **persisten entre
//! fotogramas** y por fin el lienzo. `enabled` es `!busy`, con `busy` = un gesto de dibujo
//! (`gesto_dibujo`, lo pone el test) o un deslizador o un arrastre de muestra en curso. A
//! diferencia de `panel_baja.rs`, el banco **sí aplica la elección** que devuelve el panel (como
//! `main`), para poder encadenar gestos; **nunca aplica** una `op` ni un `commit` (eso es de
//! `Session`): cada salida del panel se guarda en `banco.historial`.
//!
//! Se mide a los cinco `ppp` 1,0, 1,25, 1,5, 2,0 y 3,0 (`n` = 1, 1, 2, 2 y 3). Los clics se dan en
//! píxeles físicos enteros. Lo pintado se lee de las formas del fotograma. No hay capturas de
//! pantalla.

use egui::epaint::{ClippedShape, Shape};
use egui::{
    Color32, Context, Event, Key, MissingGlyphPolicy, Modifiers, MouseWheelSource, MouseWheelUnit,
    PointerButton, Pos2, RawInput, Rect, TextureId, TouchPhase, Vec2, pos2, vec2,
};
use px_core::{Command, Document, Image, Mode, Palette, Pixel, Rgba};
use px_ui::canvas::Canvas;
use px_ui::color::{ColorChoice, SlotColors, cell_count, initial_choice};
use px_ui::color_edit::{ColorEditor, Commit, Control, EditTarget, slider_x};
use px_ui::edit_layout::{
    BUTTON_TEXT_X_U, BUTTON_TEXT_Y_U, FIELD_H_U, FIELD_W_U, FIELD_X_U, PaletteButton, PanelLayout,
    ROW_H_U, TRACK_W_U, TRACK_X_U, button_rect_u, control_row_u, field_y_u, panel_layout,
};
use px_ui::icons::{Icon, IconSet};
use px_ui::palette_grid::{
    self, CELL_U, COLUMNS, GRID_X_U, GRID_Y_U, Grid, PANEL_WIDTH_U, PITCH_U, PanelView,
    drop_mark_rect, ring_rects,
};
use px_ui::palette_ops::{PaletteOp, PaletteOps, Removal};
use px_ui::palette_panel::{self, PanelOutput};
use px_ui::status::bar_height_px;
use px_ui::text::{apply_scale, install_fonts, text_scale};
use px_ui::theme::{
    ACCENT, DROP_MARK, RING_DEST, RING_RANGE, SURFACE_0, SURFACE_2, SURFACE_3, TEXT, TEXT_DIM,
    TEXT_DISABLED,
};
use px_ui::view::ScreenRect;

#[path = "common/iconos.rs"]
mod iconos;
#[path = "common/zonas.rs"]
mod zonas;
use iconos::IconosDePrueba;
use zonas::panel_a_la_izquierda;

// ---------------------------------------------------------------------------------------------
// Imágenes de prueba
// ---------------------------------------------------------------------------------------------

const ROJO: Rgba = Rgba::opaque(255, 0, 0);
const VERDE: Rgba = Rgba::opaque(0, 255, 0);
const AZUL: Rgba = Rgba::opaque(0, 0, 255);
const NEGRO: Rgba = Rgba::opaque(0, 0, 0);
/// `X` y `Y` de la paleta `P5` (fila «colores intermedios» de `hito-2-color-cercano.md`).
const X: Rgba = Rgba::opaque(200, 30, 30);
const Y: Rgba = Rgba::opaque(10, 10, 240);

const PPPS: [f32; 5] = [1.0, 1.25, 1.5, 2.0, 3.0];

/// Indexada con los píxeles `pixeles` (una fila) y la paleta `colores`, con el transparente dado.
fn imagen_con(colores: &[Rgba], transparente: Option<u8>, pixeles: &[u8]) -> Image {
    let mut paleta = Palette::from_colors(colores.iter().copied()).expect("paleta");
    paleta
        .set_transparent_index(transparente)
        .expect("transparente");
    let mut imagen = Image::new_indexed(pixeles.len() as u32, 1, paleta).expect("imagen indexada");
    for (x, indice) in pixeles.iter().enumerate() {
        imagen
            .set_pixel(x as u32, 0, Pixel::Indexed(*indice))
            .expect("píxel");
    }
    imagen
}

/// `F`: `[R, G, B, K]` 4×1 con los píxeles `[0, 1, 2, 3]` (todas las entradas se usan).
fn imagen_f() -> Image {
    imagen_con(&[ROJO, VERDE, AZUL, NEGRO], None, &[0, 1, 2, 3])
}

/// `P5`: `[R, G, B, X, Y]` 5×1 con los píxeles `[0, 1, 2, 3, 3]` (la 4 no la usa nadie).
fn imagen_p5() -> Image {
    imagen_con(&[ROJO, VERDE, AZUL, X, Y], None, &[0, 1, 2, 3, 3])
}

/// `P5` con la entrada `transparente` como transparente.
fn imagen_p5_transparente(transparente: u8) -> Image {
    imagen_con(
        &[ROJO, VERDE, AZUL, X, Y],
        Some(transparente),
        &[0, 1, 2, 3, 3],
    )
}

/// Paleta de una sola entrada.
fn imagen_una_entrada() -> Image {
    imagen_con(&[ROJO], None, &[0, 0])
}

/// Indexada de 64×48 con 256 colores (16 filas de la rejilla); todos los píxeles en la entrada 0.
fn imagen_indexada_256() -> Image {
    let colores: Vec<Rgba> = (0..256usize)
        .map(|i| Rgba::opaque(i as u8, 255 - i as u8, (i * 7) as u8))
        .collect();
    let paleta = Palette::from_colors(colores).expect("paleta de 256 colores");
    Image::new_indexed(64, 48, paleta).expect("imagen indexada")
}

// ---------------------------------------------------------------------------------------------
// Eventos sintéticos
// ---------------------------------------------------------------------------------------------

const MAYUS: Modifiers = Modifiers {
    shift: true,
    ..Modifiers::NONE
};

fn boton(pos: Pos2, pressed: bool, modifiers: Modifiers) -> Event {
    Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers,
    }
}

fn texto(s: &str) -> Event {
    Event::Text(s.to_owned())
}

fn tecla(key: Key) -> Event {
    Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    }
}

fn tecla_ctrl_a() -> Event {
    Event::Key {
        key: Key::A,
        physical_key: Some(Key::A),
        pressed: true,
        repeat: false,
        modifiers: Modifiers {
            ctrl: true,
            command: true,
            ..Modifiers::NONE
        },
    }
}

fn en_puntos(ppp: f32, x_px: i64, y_px: i64) -> Pos2 {
    pos2(x_px as f32 / ppp, y_px as f32 / ppp)
}

// ---------------------------------------------------------------------------------------------
// Lo pintado
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Debug)]
struct Pintado {
    texto: String,
    pos: Pos2,
    color: Color32,
}

/// Una malla con textura (un icono): la textura, el rectángulo que cubre y el color de sus vértices
/// (la tinta).
#[derive(Clone, Copy, Debug)]
struct Malla {
    textura: TextureId,
    rect: Rect,
    /// Rectángulo que cubren las coordenadas de textura de sus vértices.
    uv: Rect,
    color: Color32,
}

fn recoge(
    shape: &Shape,
    textos: &mut Vec<Pintado>,
    rects: &mut Vec<(Rect, Color32)>,
    mallas: &mut Vec<Malla>,
) {
    match shape {
        Shape::Mesh(malla) => {
            let puntos: Vec<Pos2> = malla.vertices.iter().map(|v| v.pos).collect();
            let uvs: Vec<Pos2> = malla.vertices.iter().map(|v| v.uv).collect();
            mallas.push(Malla {
                textura: malla.texture_id,
                rect: Rect::from_points(&puntos),
                uv: Rect::from_points(&uvs),
                color: malla.vertices.first().map_or(Color32::PLACEHOLDER, |v| v.color),
            });
        }
        Shape::Text(t) => textos.push(Pintado {
            texto: t.galley.text().to_owned(),
            pos: t.pos,
            color: t.fallback_color,
        }),
        Shape::Rect(r) => rects.push((r.rect, r.fill)),
        Shape::Vec(formas) => {
            for forma in formas {
                recoge(forma, textos, rects, mallas);
            }
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------------------------
// Banco
// ---------------------------------------------------------------------------------------------

const PANTALLA_ALTA: Vec2 = vec2(800.0, 600.0);

struct Banco {
    ctx: Context,
    canvas: Canvas,
    scroll: palette_grid::Scroll,
    view: PanelView,
    pantalla: Vec2,
    documento: Document,
    image: Image,
    choice: ColorChoice,
    slots: SlotColors,
    editor: ColorEditor,
    /// Estado de las operaciones de paleta, entre fotogramas, como el de `Session`.
    ops: PaletteOps,
    /// Los iconos de prueba (una textura blanca por icono).
    iconos: IconosDePrueba,
    /// «Estado del documento» que se pasa a `ops.sync`; lo cambia el test (y `aplica`, `deshace`
    /// y `rehace`).
    doc_state: u64,
    /// Veces que `ops.sync` canceló una quita pendiente.
    cancelaciones: usize,
    /// Hay un gesto de dibujo en curso (`enabled` falso).
    gesto_dibujo: bool,
    /// Usa `show_editing_view` (sin estado de operaciones) en lugar de `show_editing_ops`.
    sin_ops: bool,
    /// Lo que devolvió el panel en cada fotograma, en orden.
    historial: Vec<PanelOutput>,
    /// Tiempo del fotograma para los tooltips: sin él (`None`) `RawInput::time` queda vacío, como
    /// en todas las pruebas anteriores; `con_tiempo` lo enciende y avanza 0,1 s por fotograma.
    tiempo: std::cell::Cell<Option<f64>>,
}

struct Fotograma {
    salida: PanelOutput,
    textos: Vec<Pintado>,
    rects: Vec<(Rect, Color32)>,
    mallas: Vec<Malla>,
    formas: Vec<ClippedShape>,
}

impl Fotograma {
    fn pinta(&self, buscado: &str) -> bool {
        self.textos.iter().any(|t| t.texto == buscado)
    }

    fn pinta_algo_con(&self, buscado: &str) -> bool {
        self.textos.iter().any(|t| t.texto.contains(buscado))
    }

    fn texto_pintado(&self, buscado: &str) -> Option<&Pintado> {
        self.textos.iter().find(|t| t.texto == buscado)
    }

    /// La malla del icono, si se pintó.
    fn malla_de(&self, iconos: &IconSet, icono: Icon) -> Option<&Malla> {
        let id = iconos.get(icono)?;
        self.mallas.iter().find(|m| m.textura == id)
    }

    fn pinta_icono(&self, iconos: &IconSet, icono: Icon) -> bool {
        self.malla_de(iconos, icono).is_some()
    }

    /// La tinta con que se pintó el icono.
    fn tinta_de(&self, iconos: &IconSet, icono: Icon) -> Color32 {
        self.malla_de(iconos, icono)
            .unwrap_or_else(|| panic!("no se pinta el icono {icono:?}"))
            .color
    }

    fn color_de(&self, buscado: &str) -> Color32 {
        self.texto_pintado(buscado)
            .unwrap_or_else(|| panic!("no se pinta {buscado:?}"))
            .color
    }
}

/// Medidas del editor a la escala del banco, en píxeles físicos desde el borde superior del panel.
#[derive(Clone, Copy, Debug)]
struct Geo {
    n: i64,
    mode: Mode,
    top_px: i64,
}

impl Geo {
    /// `y` del centro de la fila del deslizador `control`.
    fn y_fila(&self, control: Control) -> i64 {
        let fila = control_row_u(self.mode, control).expect("el modo tiene ese deslizador");
        self.top_px + fila * self.n + ROW_H_U * self.n / 2
    }

    fn x_de_valor(&self, valor: u32, max: u32) -> i64 {
        TRACK_X_U * self.n + slider_x(valor, TRACK_W_U * self.n, max)
    }

    /// Centro del campo hexadecimal.
    fn campo(&self) -> (i64, i64) {
        (
            FIELD_X_U * self.n + FIELD_W_U * self.n / 2,
            self.top_px + field_y_u(self.mode) * self.n + FIELD_H_U * self.n / 2,
        )
    }
}

/// Centro de la muestra `indice` en píxeles físicos con el bloque sin desplazar.
fn centro_de_muestra(n: i64, indice: usize, primera_fila: usize) -> (i64, i64) {
    let columna = (indice % COLUMNS) as i64;
    let fila = (indice / COLUMNS) as i64 - primera_fila as i64;
    (
        (GRID_X_U + columna * PITCH_U) * n + CELL_U * n / 2,
        (GRID_Y_U + fila * PITCH_U) * n + CELL_U * n / 2,
    )
}

fn pantalla_para_panel(ppp: f32, panel_h_px: i64) -> Vec2 {
    let n = text_scale(ppp);
    vec2(
        800.0,
        (panel_h_px + i64::from(bar_height_px(n))) as f32 / ppp,
    )
}

/// Rectángulo en `u` `(x, y, ancho, alto)` de un botón de indexadas.
fn rect_boton_u(button: PaletteButton) -> (i64, i64, i64, i64) {
    button_rect_u(Mode::Indexed, button).expect("las indexadas tienen botones")
}

impl Banco {
    fn nuevo(ppp: f32, pantalla: Vec2, image: Image, choice: ColorChoice) -> Banco {
        let ctx = Context::default();
        ctx.set_pixels_per_point(ppp);
        ctx.set_missing_glyph_policy(MissingGlyphPolicy::Panic);
        install_fonts(&ctx);
        let iconos = IconosDePrueba::nuevos(&ctx);
        let mut banco = Banco {
            ctx,
            canvas: Canvas::new(),
            scroll: palette_grid::Scroll::default(),
            view: PanelView {
                icons: iconos.set,
                ..PanelView::default()
            },
            pantalla,
            documento: Document::new(image.clone()),
            slots: SlotColors::defaults(image.mode()),
            image,
            choice,
            editor: ColorEditor::default(),
            ops: PaletteOps::default(),
            iconos,
            doc_state: 0,
            cancelaciones: 0,
            gesto_dibujo: false,
            sin_ops: false,
            historial: Vec::new(),
            tiempo: std::cell::Cell::new(None),
        };
        for _ in 0..2 {
            let salida = banco.ctx.run_ui(banco.entrada(vec![]), |ui| {
                apply_scale(ui.ctx());
            });
            salida.drop_without_applying_deltas();
        }
        banco.canvas.set_image(&banco.image);
        banco.reposo();
        banco.reposo();
        banco.historial.clear();
        banco
    }

    fn con_alto_de_panel(ppp: f32, panel_h_px: i64, image: Image, choice: ColorChoice) -> Banco {
        Banco::nuevo(ppp, pantalla_para_panel(ppp, panel_h_px), image, choice)
    }

    fn entrada(&self, events: Vec<Event>) -> RawInput {
        let time = self.tiempo.get().map(|t| {
            self.tiempo.set(Some(t + 0.1));
            t + 0.1
        });
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, self.pantalla)),
            events,
            time,
            ..RawInput::default()
        }
    }

    fn ppp(&self) -> f32 {
        self.ctx.pixels_per_point()
    }

    fn n(&self) -> i64 {
        i64::from(text_scale(self.ppp()))
    }

    fn panel_h_px(&self) -> i64 {
        (self.pantalla.y * self.ppp()).round() as i64 - i64::from(bar_height_px(self.n() as u32))
    }

    /// El panel, en puntos.
    fn rect_panel(&self) -> Rect {
        let u = self.n() as f32 / self.ppp();
        Rect::from_min_size(
            Pos2::ZERO,
            vec2(
                PANEL_WIDTH_U as f32 * u,
                self.panel_h_px() as f32 / self.ppp(),
            ),
        )
    }

    /// El diseño del panel con el desplazamiento que tiene la vista ahora.
    fn disposicion(&self) -> PanelLayout {
        panel_layout(
            self.image.mode(),
            self.panel_h_px(),
            self.n() as u32,
            self.view.offset_px,
        )
    }

    /// El máximo desplazamiento posible con el alto actual.
    fn max_offset(&self) -> i64 {
        panel_layout(self.image.mode(), self.panel_h_px(), self.n() as u32, 0).max_offset_px
    }

    fn geo(&self) -> Geo {
        Geo {
            n: self.n(),
            mode: self.image.mode(),
            top_px: self.disposicion().top_px,
        }
    }

    fn actual(&self) -> Option<(EditTarget, Rgba)> {
        let destino = EditTarget::of(&self.image, self.choice)?;
        let color = destino.stored(&self.image, &self.slots)?;
        Some((destino, color))
    }

    fn fotograma(&mut self, events: Vec<Event>) -> Fotograma {
        let len = self.image.palette().map_or(0, Palette::len);
        if self.ops.sync(self.doc_state, len) {
            self.cancelaciones += 1;
        }
        let actual = self.actual();
        self.editor.sync(actual);
        let enabled = !(self.gesto_dibujo || self.editor.dragging() || self.ops.busy());
        let sin_ops = self.sin_ops;
        let input = self.entrada(events);
        let mut resultado = None;
        let Banco {
            ctx,
            canvas,
            scroll,
            view,
            image,
            choice,
            slots,
            editor,
            ops,
            ..
        } = self;
        let mut salida = ctx.run_ui(input, |ui| {
            apply_scale(ui.ctx());
            px_ui::status::show(ui, "lápiz · color 1 #000000", false);
            let (panel, ()) = panel_a_la_izquierda(
                ui,
                |ui| {
                    if sin_ops {
                        palette_panel::show_editing_view(
                            ui, image, *choice, scroll, view, enabled, slots, editor,
                        )
                    } else {
                        palette_panel::show_editing_ops(
                            ui, image, *choice, scroll, view, enabled, slots, editor, ops,
                        )
                    }
                },
                |ui| {
                    let _ = canvas.interact_ordered(ui);
                    let _ = canvas.paint(ui);
                },
            );
            resultado = Some(panel);
        });
        salida.textures_delta.clear();
        let formas: Vec<ClippedShape> = std::mem::take(&mut salida.shapes);
        let panel = resultado.expect("el panel se dibuja");
        let (mut textos, mut rects, mut mallas) = (Vec::new(), Vec::new(), Vec::new());
        for clipped in &formas {
            recoge(&clipped.shape, &mut textos, &mut rects, &mut mallas);
        }
        if let Some(elegida) = panel.choice {
            self.choice = elegida;
        }
        self.historial.push(panel.clone());
        Fotograma {
            salida: panel,
            textos,
            rects,
            mallas,
            formas,
        }
    }

    /// Un fotograma en el que egui recibe los eventos pero el panel no se monta (para perder un
    /// soltar sin que el panel lo vea).
    fn fotograma_sin_panel(&mut self, events: Vec<Event>) {
        let input = self.entrada(events);
        let salida = self.ctx.run_ui(input, |ui| {
            apply_scale(ui.ctx());
        });
        salida.drop_without_applying_deltas();
    }

    fn reposo(&mut self) -> Fotograma {
        self.fotograma(vec![])
    }

    fn quiere_teclado(&self) -> bool {
        self.ctx.text_edit_focused()
    }

    // --- Ratón ---

    fn mueve(&mut self, x_px: i64, y_px: i64) -> Fotograma {
        let pos = en_puntos(self.ppp(), x_px, y_px);
        self.fotograma(vec![Event::PointerMoved(pos)])
    }

    fn pulsa_con(&mut self, x_px: i64, y_px: i64, modifiers: Modifiers) -> Fotograma {
        let pos = en_puntos(self.ppp(), x_px, y_px);
        self.fotograma(vec![boton(pos, true, modifiers)])
    }

    fn pulsa(&mut self, x_px: i64, y_px: i64) -> Fotograma {
        self.pulsa_con(x_px, y_px, Modifiers::NONE)
    }

    fn suelta_con(&mut self, x_px: i64, y_px: i64, modifiers: Modifiers) -> Fotograma {
        let pos = en_puntos(self.ppp(), x_px, y_px);
        self.fotograma(vec![boton(pos, false, modifiers)])
    }

    fn suelta(&mut self, x_px: i64, y_px: i64) -> Fotograma {
        self.suelta_con(x_px, y_px, Modifiers::NONE)
    }

    /// Mover, pulsar y soltar. Devuelve el fotograma de pulsar (donde actúan botones y muestras).
    fn clic_con(&mut self, x_px: i64, y_px: i64, modifiers: Modifiers) -> Fotograma {
        self.mueve(x_px, y_px);
        let pulsado = self.pulsa_con(x_px, y_px, modifiers);
        self.suelta_con(x_px, y_px, modifiers);
        pulsado
    }

    fn clic(&mut self, x_px: i64, y_px: i64) -> Fotograma {
        self.clic_con(x_px, y_px, Modifiers::NONE)
    }

    /// La rueda, `px_abajo` píxeles físicos hacia abajo, en el punto donde esté el puntero.
    fn rueda(&mut self, px_abajo: i64) -> Fotograma {
        let delta = vec2(0.0, -(px_abajo as f32) / self.ppp());
        self.fotograma(vec![Event::MouseWheel {
            unit: MouseWheelUnit::Point,
            delta,
            phase: TouchPhase::Move,
            source: MouseWheelSource::Unknown,
            modifiers: Modifiers::NONE,
        }])
    }

    fn rueda_en(&mut self, x_px: i64, y_px: i64, px_abajo: i64) -> Fotograma {
        self.mueve(x_px, y_px);
        self.rueda(px_abajo)
    }

    /// Un punto del panel sobre el texto de arriba.
    fn sobre_el_texto(&self) -> (i64, i64) {
        (10 * self.n(), 10 * self.n())
    }

    // --- Botones y muestras ---

    /// Centro del botón en píxeles físicos, con el desplazamiento del panel de ahora.
    fn centro_de_boton(&self, button: PaletteButton) -> (i64, i64) {
        let (x, y, w, h) = rect_boton_u(button);
        let n = self.n();
        ((x + w / 2) * n, self.geo().top_px + (y + h / 2) * n)
    }

    /// Rectángulo del botón, en puntos.
    fn rect_de_boton(&self, button: PaletteButton) -> Rect {
        let (x, y, w, h) = rect_boton_u(button);
        let (n, ppp) = (self.n(), self.ppp());
        let top = self.geo().top_px;
        Rect::from_min_size(
            pos2((x * n) as f32 / ppp, (top + y * n) as f32 / ppp),
            vec2((w * n) as f32 / ppp, (h * n) as f32 / ppp),
        )
    }

    /// La tinta del icono en lo pintado en `f`.
    fn tinta(&self, f: &Fotograma, icono: Icon) -> Color32 {
        f.tinta_de(&self.iconos.set, icono)
    }

    /// Clic completo en el botón. Devuelve el fotograma de pulsar.
    fn clic_boton(&mut self, button: PaletteButton) -> Fotograma {
        let (x, y) = self.centro_de_boton(button);
        self.clic(x, y)
    }

    fn centro_de_la_muestra(&self, indice: usize) -> (i64, i64) {
        centro_de_muestra(self.n(), indice, self.scroll.first_row)
    }

    /// Clic en la muestra. Devuelve el fotograma de pulsar.
    fn clic_muestra(&mut self, indice: usize) -> Fotograma {
        let (x, y) = self.centro_de_la_muestra(indice);
        self.clic(x, y)
    }

    fn clic_muestra_con(&mut self, indice: usize, modifiers: Modifiers) -> Fotograma {
        let (x, y) = self.centro_de_la_muestra(indice);
        self.clic_con(x, y, modifiers)
    }

    /// Pulsa en el centro de `desde` y mueve el puntero al centro de `hasta`, sin soltar.
    fn arrastra(&mut self, desde: usize, hasta: usize) -> Fotograma {
        let (x, y) = self.centro_de_la_muestra(desde);
        self.mueve(x, y);
        self.pulsa(x, y);
        let (x, y) = self.centro_de_la_muestra(hasta);
        self.mueve(x, y)
    }

    /// Rectángulo en píxeles físicos desde el origen de la rejilla, en puntos.
    fn rect_de_rejilla(&self, r: ScreenRect) -> Rect {
        let (n, ppp) = (self.n(), self.ppp());
        let (x, y) = (GRID_X_U * n + r.x, GRID_Y_U * n - self.view.offset_px + r.y);
        Rect::from_min_size(
            pos2(x as f32 / ppp, y as f32 / ppp),
            vec2(r.width as f32 / ppp, r.height as f32 / ppp),
        )
    }

    fn rejilla(&self) -> Grid {
        Grid::new(
            self.n() as u32,
            cell_count(&self.image),
            self.disposicion().visible_rows,
            self.scroll.first_row,
        )
    }

    fn celda(&self, indice: usize) -> ScreenRect {
        self.rejilla()
            .cell_rect(indice)
            .unwrap_or_else(|| panic!("la muestra {indice} se ve"))
    }

    // --- Campo hexadecimal ---

    fn enfoca_campo(&mut self) {
        let (x, y) = self.geo().campo();
        self.clic(x, y);
        self.reposo();
        assert!(
            self.quiere_teclado(),
            "un clic en el campo le da el foco de texto"
        );
    }

    fn escribe(&mut self, cadena: &str) -> Fotograma {
        self.fotograma(vec![tecla_ctrl_a()]);
        self.fotograma(vec![texto(cadena)])
    }

    // --- Documento ---

    fn refresca(&mut self) {
        let id = self.documento.main_image();
        self.image = self.documento.image(id).expect("la imagen").clone();
        self.doc_state += 1;
    }

    fn aplica(&mut self, command: Command) {
        self.documento.apply(command).expect("comando válido");
        self.refresca();
    }

    fn deshace(&mut self) {
        self.documento.undo().expect("hay algo que deshacer");
        self.refresca();
    }

    fn pone_color(&mut self, index: u8, color: Rgba) {
        let image = self.documento.main_image();
        self.aplica(Command::SetPaletteColor {
            image,
            index,
            color,
        });
    }

    // --- Lo recogido ---

    fn commits(&self) -> Vec<Commit> {
        self.historial.iter().filter_map(|o| o.commit).collect()
    }

    fn avisos(&self) -> Vec<String> {
        self.historial
            .iter()
            .filter_map(|o| o.notice.clone())
            .collect()
    }

    fn elecciones(&self) -> Vec<ColorChoice> {
        self.historial.iter().filter_map(|o| o.choice).collect()
    }

    fn operaciones(&self) -> Vec<PaletteOp> {
        self.historial.iter().filter_map(|o| o.op).collect()
    }

    /// Todo el historial sin una sola petición: ni elección, ni `commit`, ni aviso, ni `op`.
    fn historial_vacio(&self) -> bool {
        self.historial.iter().all(|o| *o == PanelOutput::default())
    }
}

fn banco_f(ppp: f32, entrada: u8) -> Banco {
    Banco::nuevo(ppp, PANTALLA_ALTA, imagen_f(), ColorChoice::Entry(entrada))
}

fn banco_p5(ppp: f32, entrada: u8) -> Banco {
    Banco::nuevo(ppp, PANTALLA_ALTA, imagen_p5(), ColorChoice::Entry(entrada))
}

/// Banco a 1,0 con el campo hexadecimal enfocado y `escrito` pendiente en la entrada `choice`.
fn banco_con_pendiente(image: Image, choice: ColorChoice, escrito: &str) -> Banco {
    let mut banco = Banco::nuevo(1.0, PANTALLA_ALTA, image, choice);
    banco.enfoca_campo();
    banco.escribe(escrito);
    banco.reposo();
    assert!(banco.editor.has_pending(), "se tecleó {escrito:?}");
    banco.historial.clear();
    banco
}

// ---------------------------------------------------------------------------------------------
// Comprobaciones de lo pintado
// ---------------------------------------------------------------------------------------------

const TOLERANCIA: f32 = 0.05;

fn cerca(a: f32, b: f32) -> bool {
    (a - b).abs() <= TOLERANCIA
}

fn cerca_rect(a: Rect, b: Rect) -> bool {
    cerca(a.min.x, b.min.x)
        && cerca(a.min.y, b.min.y)
        && cerca(a.max.x, b.max.x)
        && cerca(a.max.y, b.max.y)
}

/// Todo lo pintado que cae sobre el panel se recorta al panel: el `clip_rect` de cada forma que
/// asoma en él está dentro del panel. Lo de la barra de estado y el lienzo no se solapa con él.
fn comprueba_dentro_del_panel(f: &Fotograma, panel: Rect, contexto: &str) {
    let panel = panel.expand(TOLERANCIA);
    for forma in &f.formas {
        let visible = forma
            .shape
            .visual_bounding_rect()
            .intersect(forma.clip_rect);
        let solape = visible.intersect(panel);
        if !(visible.is_positive()
            && solape.width() > 2.0 * TOLERANCIA
            && solape.height() > 2.0 * TOLERANCIA)
        {
            continue;
        }
        assert!(
            panel.contains_rect(forma.clip_rect),
            "{contexto}: una forma que cae en el panel se recorta a {:?}, fuera del panel {panel:?}",
            forma.clip_rect
        );
        assert!(
            panel.contains_rect(visible),
            "{contexto}: se ve {visible:?} y el panel es {panel:?}"
        );
    }
}

/// Los cuatro rectángulos del anillo de `grosor_u` alrededor de la muestra `indice`, con `color`.
fn hay_anillo(f: &Fotograma, banco: &Banco, indice: usize, grosor_u: i64, color: Color32) -> bool {
    let anillo = ring_rects(banco.celda(indice), grosor_u * banco.n());
    anillo.into_iter().all(|parte| {
        let esperado = banco.rect_de_rejilla(parte);
        f.rects
            .iter()
            .any(|(r, relleno)| *relleno == color && cerca_rect(*r, esperado))
    })
}

/// Hay una forma rellena de `color` en el rectángulo `esperado`.
fn hay_rect(f: &Fotograma, esperado: Rect, color: Color32) -> bool {
    f.rects
        .iter()
        .any(|(r, relleno)| *relleno == color && cerca_rect(*r, esperado))
}

/// Los botones de texto: solo `Confirmar` y `Cancelar` (specs/interfaz-iconos.md, decisión 8).
const ROTULOS: [(PaletteButton, &str); 2] = [
    (PaletteButton::Confirm, "Confirmar"),
    (PaletteButton::Cancel, "Cancelar"),
];

/// Los botones de icono de la fila de la paleta.
const ICONOS_DE_BOTON: [(PaletteButton, Icon); 5] = [
    (PaletteButton::Add, Icon::Add),
    (PaletteButton::Remove, Icon::Remove),
    (PaletteButton::RotateLeft, Icon::RotateLeft),
    (PaletteButton::RotateRight, Icon::RotateRight),
    (PaletteButton::Transparent, Icon::Transparent),
];

/// Los botones de `visibles` se pintan en su sitio: los de icono, con la malla de 16 u a 4 u del
/// borde del botón (o 1 u más abajo y a la derecha si está hundido) sobre una cara `SURFACE_2`
/// (`SURFACE_3` con el cursor encima, `SURFACE_0` si está hundido); `Confirmar` y `Cancelar`, con
/// su rótulo en `(6, 5)` dentro del botón y la misma cara. Los demás, no.
fn comprueba_botones(f: &Fotograma, banco: &Banco, visibles: &[PaletteButton], contexto: &str) {
    let (ppp, n) = (banco.ppp(), banco.n());
    let top = banco.geo().top_px;
    let iconos = banco.iconos.set;
    for (button, icono) in ICONOS_DE_BOTON {
        // `Transparent` lo comprueba `comprueba_boton` (su estado depende de la entrada).
        if button == PaletteButton::Transparent {
            continue;
        }
        if !visibles.contains(&button) {
            assert!(
                !f.pinta_icono(&iconos, icono),
                "{contexto}: no debe pintarse el icono {icono:?}"
            );
            continue;
        }
        let rect = banco.rect_de_boton(button);
        assert!(
            hay_rect(f, rect, SURFACE_2) || hay_rect(f, rect, SURFACE_3),
            "{contexto}: no hay cara del botón {button:?} en {rect:?}"
        );
        let malla = f
            .malla_de(&iconos, icono)
            .unwrap_or_else(|| panic!("{contexto}: no se pinta el icono {icono:?}"));
        let (x, y, _, _) = rect_boton_u(button);
        let esperado = Rect::from_min_size(
            pos2(((x + 4) * n) as f32 / ppp, (top + (y + 4) * n) as f32 / ppp),
            vec2((16 * n) as f32 / ppp, (16 * n) as f32 / ppp),
        );
        assert!(
            cerca_rect(malla.rect, esperado),
            "{contexto}: el icono {icono:?} está en {:?} y la maqueta dice {esperado:?}",
            malla.rect
        );
    }
    for (button, nombre) in ROTULOS {
        if !visibles.contains(&button) {
            assert!(!f.pinta(nombre), "{contexto}: no debe pintarse {nombre:?}");
            continue;
        }
        let (x, y, _, _) = rect_boton_u(button);
        let pintado = f
            .texto_pintado(nombre)
            .unwrap_or_else(|| panic!("{contexto}: no se pinta {nombre:?}"));
        let esperado = (
            ((x + BUTTON_TEXT_X_U) * n) as f32,
            (top + (y + BUTTON_TEXT_Y_U) * n) as f32,
        );
        assert!(
            cerca(pintado.pos.x * ppp, esperado.0) && cerca(pintado.pos.y * ppp, esperado.1),
            "{contexto}: {nombre:?} en ({}, {}) px y la maqueta dice {esperado:?}",
            pintado.pos.x * ppp,
            pintado.pos.y * ppp
        );
        let rect = banco.rect_de_boton(button);
        assert!(
            hay_rect(f, rect, SURFACE_2) || hay_rect(f, rect, SURFACE_3),
            "{contexto}: no hay cara del botón {nombre:?} en {rect:?}"
        );
    }
}

const NORMALES: [PaletteButton; 4] = [
    PaletteButton::Add,
    PaletteButton::Remove,
    PaletteButton::RotateLeft,
    PaletteButton::RotateRight,
];

/// Con una quita pendiente la fila de iconos desaparece: solo `Confirmar` y `Cancelar`.
const CON_QUITA: [PaletteButton; 2] = [PaletteButton::Confirm, PaletteButton::Cancel];

// ---------------------------------------------------------------------------------------------
// Fila 2: los botones se pintan en su sitio y se pulsan donde la maqueta dice
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_2_los_botones_se_pintan_con_sus_iconos_en_su_sitio_a_los_cinco_ppp() {
    for ppp in PPPS {
        let mut banco = banco_f(ppp, 1);
        let f = banco.reposo();
        let contexto = format!("ppp {ppp}");
        comprueba_botones(&f, &banco, &NORMALES, &contexto);
        // Dentro del panel, y con los cuatro botones cabiendo (sin desplazamiento).
        assert!(!banco.disposicion().scrolling, "{contexto}");
        let panel = banco.rect_panel();
        for button in NORMALES {
            assert!(
                panel
                    .expand(TOLERANCIA)
                    .contains_rect(banco.rect_de_boton(button)),
                "{contexto}: {button:?} dentro del panel"
            );
        }
        comprueba_dentro_del_panel(&f, panel, &contexto);
    }
}

#[test]
fn fila_2_con_quita_pendiente_se_pintan_confirmar_y_cancelar_en_lugar_de_los_iconos() {
    for ppp in PPPS {
        let mut banco = banco_p5(ppp, 3);
        let contexto = format!("ppp {ppp}");
        comprueba_botones(&banco.reposo(), &banco, &NORMALES, &contexto);
        banco.clic_boton(PaletteButton::Remove);
        assert!(banco.ops.removal().is_some(), "{contexto}: quita pendiente");
        let f = banco.reposo();
        comprueba_botones(&f, &banco, &CON_QUITA, &contexto);
        comprueba_dentro_del_panel(&f, banco.rect_panel(), &contexto);
        // Al cancelar vuelven los de siempre.
        banco.clic_boton(PaletteButton::Cancel);
        comprueba_botones(&banco.reposo(), &banco, &NORMALES, &contexto);
    }
}

/// Modo B (spec de la ventana baja): alto 200·n, el panel entero se desplaza con la rueda y los
/// botones van con él (`y − offset`).
#[test]
fn fila_2_con_el_panel_desplazado_los_botones_se_pintan_y_se_pulsan_con_y_menos_offset() {
    for ppp in PPPS {
        let n = (ppp + 0.5).floor().max(1.0) as i64;
        let mut banco = Banco::con_alto_de_panel(ppp, 200 * n, imagen_f(), ColorChoice::Entry(1));
        assert!(
            banco.disposicion().scrolling,
            "ppp {ppp}: el panel se desplaza"
        );
        let (x, y) = banco.sobre_el_texto();
        let max = banco.max_offset();
        assert!(max >= 105 * n, "ppp {ppp}: hay recorrido ({max})");
        for offset in [98 * n, max] {
            banco.rueda_en(x, y, -100_000 * n);
            banco.rueda(offset);
            assert_eq!(banco.view.offset_px, offset, "ppp {ppp}");
            let contexto = format!("ppp {ppp}, offset {offset}");
            let f = banco.reposo();
            assert_eq!(banco.geo().top_px, 91 * n - offset, "{contexto}");
            // Con 98·n el botón de abajo asoma 3·n por debajo del panel: se pinta (egui no
            // descarta formas) y se recorta al panel, lo que comprueba la línea siguiente.
            comprueba_botones(&f, &banco, &NORMALES, &contexto);
            comprueba_dentro_del_panel(&f, banco.rect_panel(), &contexto);

            // Pulsar donde está ahora cada botón.
            let f = banco.clic_boton(PaletteButton::Add);
            assert_eq!(f.salida.op, Some(PaletteOp::Add), "{contexto}");
            let f = banco.clic_boton(PaletteButton::RotateRight);
            assert_eq!(
                f.salida.op,
                Some(PaletteOp::Rotate {
                    start: 0,
                    len: 4,
                    steps: 1
                }),
                "{contexto}"
            );
            let f = banco.clic_boton(PaletteButton::RotateLeft);
            assert_eq!(
                f.salida.op,
                Some(PaletteOp::Rotate {
                    start: 0,
                    len: 4,
                    steps: -1
                }),
                "{contexto}"
            );
            // Donde estaría `Añadir` sin desplazar no hay nada.
            let (x_a, _) = banco.centro_de_boton(PaletteButton::Add);
            let sin_desplazar = (55 + 186 + 8) * n;
            if sin_desplazar < banco.panel_h_px() {
                let f = banco.clic(x_a, sin_desplazar);
                assert_eq!(f.salida.op, None, "{contexto}");
            }
        }
        assert_eq!(banco.view.offset_px, max);
        assert!(banco.avisos().is_empty(), "ppp {ppp}");
    }
}

#[test]
fn fila_2_lo_que_cae_fuera_del_panel_no_se_golpea_con_el_panel_desplazado() {
    for ppp in PPPS {
        let n = (ppp + 0.5).floor().max(1.0) as i64;
        let mut banco = Banco::con_alto_de_panel(ppp, 200 * n, imagen_f(), ColorChoice::Entry(1));
        // Con 84·n de desplazamiento, `Añadir` va de 193·n a 209·n y el panel acaba en 200·n.
        let (x, y) = banco.sobre_el_texto();
        banco.rueda_en(x, y, 84 * n);
        assert_eq!(banco.view.offset_px, 84 * n, "ppp {ppp}");
        let (x, _) = banco.centro_de_boton(PaletteButton::Add);
        let alto = banco.panel_h_px();
        assert_eq!(banco.geo().top_px, 7 * n);

        // Por debajo del panel (sobre la barra de estado), aunque dentro del rectángulo del botón.
        let f = banco.clic(x, alto + n);
        assert_eq!(f.salida.op, None, "ppp {ppp}: fuera del panel");
        assert!(banco.avisos().is_empty());
        // La parte de dentro sí.
        let f = banco.clic(x, alto - n);
        assert_eq!(
            f.salida.op,
            Some(PaletteOp::Add),
            "ppp {ppp}: dentro del panel"
        );
    }
}

#[test]
fn fila_2_en_rgba_gray_y_grayalpha_no_hay_botones_ni_iconos_ni_texto_de_botones() {
    for ppp in PPPS {
        for modo in [Mode::Rgba, Mode::Gray, Mode::GrayAlpha] {
            let image = Image::new(64, 48, modo).expect("imagen");
            let choice = initial_choice(&image);
            let mut banco = Banco::nuevo(ppp, PANTALLA_ALTA, image, choice);
            let contexto = format!("ppp {ppp}, {modo:?}");
            let f = banco.reposo();
            for (_, nombre) in ROTULOS {
                assert!(!f.pinta_algo_con(nombre), "{contexto}: pinta {nombre:?}");
            }
            for (_, icono) in ICONOS_DE_BOTON {
                assert!(
                    !f.pinta_icono(&banco.iconos.set, icono),
                    "{contexto}: pinta el icono {icono:?}"
                );
            }
            assert!(
                !f.pinta_algo_con("Rotar") && !f.pinta_algo_con("quitar"),
                "{contexto}"
            );
            comprueba_dentro_del_panel(&f, banco.rect_panel(), &contexto);

            // Barrido de clics por el editor, donde estarían los botones y por debajo: nada pide
            // una operación ni avisa ni deja una quita pendiente.
            let n = banco.n();
            let top = banco.geo().top_px;
            let mut y = top + 4 * n;
            while y < banco.panel_h_px() {
                for x_u in [10, 60, 120, 200] {
                    let f = banco.clic(x_u * n, y);
                    assert_eq!(f.salida.op, None, "{contexto}: clic en ({x_u}·n, {y})");
                    assert_eq!(f.salida.notice, None, "{contexto}");
                    assert!(!f.pinta_algo_con("Confirmar"), "{contexto}");
                }
                y += 8 * n;
            }
            for button in NORMALES {
                let (x, y) = banco.centro_de_boton(button);
                banco.clic(x, y);
            }
            assert!(banco.operaciones().is_empty(), "{contexto}");
            assert!(banco.ops.removal().is_none(), "{contexto}");
            assert_eq!(banco.ops.range_end(), None, "{contexto}");
            assert!(banco.avisos().is_empty(), "{contexto}");
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Efecto {
    Anadir,
    Quitar,
    Izquierda,
    Derecha,
    Nada,
}

fn efecto(banco: &Banco, f: &Fotograma) -> Efecto {
    match f.salida.op {
        Some(PaletteOp::Add) => Efecto::Anadir,
        Some(PaletteOp::Rotate { steps, .. }) if steps < 0 => Efecto::Izquierda,
        Some(PaletteOp::Rotate { .. }) => Efecto::Derecha,
        Some(otro) => panic!("operación inesperada {otro:?}"),
        None if banco.ops.removal().is_some() => Efecto::Quitar,
        None => Efecto::Nada,
    }
}

/// Los puntos de la fila 2 de la spec, en píxeles desde la esquina del editor (`n` = escala).
fn puntos_de_la_maqueta(n: i64) -> [(i64, i64, Efecto); 11] {
    [
        (4 * n, 186 * n, Efecto::Anadir),
        (114 * n - 1, 186 * n, Efecto::Anadir),
        (114 * n, 186 * n, Efecto::Nada),
        (119 * n, 186 * n, Efecto::Quitar),
        (229 * n - 1, 202 * n - 1, Efecto::Quitar),
        (10 * n, 202 * n, Efecto::Nada),
        (10 * n, 204 * n, Efecto::Izquierda),
        (10 * n, 222 * n, Efecto::Derecha),
        (10 * n, 238 * n, Efecto::Nada),
        (3 * n, 204 * n, Efecto::Nada),
        (229 * n, 204 * n, Efecto::Nada),
    ]
}

#[test]
fn fila_2_pulsar_donde_la_maqueta_dice_a_los_cinco_ppp() {
    for ppp in PPPS {
        let mut banco = banco_f(ppp, 1);
        let n = banco.n();
        let top = banco.geo().top_px;
        for (x, y, esperado) in puntos_de_la_maqueta(n) {
            let f = banco.clic(x, top + y);
            assert_eq!(
                efecto(&banco, &f),
                esperado,
                "ppp {ppp}: pulsar ({x}, {y}) desde el editor"
            );
            banco.ops.cancel_removal();
            banco.reposo();
        }
        assert!(banco.avisos().is_empty(), "ppp {ppp}");
    }
}

#[test]
fn fila_2_con_quita_pendiente_la_fila_1_es_confirmar_y_cancelar_en_sus_puntos() {
    for ppp in PPPS {
        let mut banco = banco_p5(ppp, 3);
        let n = banco.n();
        let top = banco.geo().top_px;
        // El hueco de 5 u no pulsa nada.
        banco.clic_boton(PaletteButton::Remove);
        let f = banco.clic(114 * n, top + 186 * n);
        assert_eq!(f.salida.op, None, "ppp {ppp}");
        assert_eq!(
            banco.ops.removal(),
            Some(Removal {
                index: 3,
                into: Some(0)
            })
        );
        // (119n, 186n) es Cancelar.
        banco.clic(119 * n, top + 186 * n);
        assert_eq!(banco.ops.removal(), None, "ppp {ppp}: Cancelar");
        // (4n, 186n) es Confirmar.
        banco.clic_boton(PaletteButton::Remove);
        let f = banco.clic(4 * n, top + 186 * n);
        assert_eq!(
            f.salida.op,
            Some(PaletteOp::Remove { index: 3, into: 0 }),
            "ppp {ppp}: Confirmar"
        );
        // Las filas de rotar siguen siendo rotar con la quita pendiente.
        banco.clic_boton(PaletteButton::Remove);
        let f = banco.clic(10 * n, top + 222 * n);
        assert!(
            matches!(f.salida.op, Some(PaletteOp::Rotate { steps: 1, .. })),
            "ppp {ppp}: {:?}",
            f.salida.op
        );
    }
}

#[test]
fn fila_2_todo_lo_pintado_con_botones_y_estados_queda_dentro_del_panel() {
    for ppp in PPPS {
        let n = (ppp + 0.5).floor().max(1.0) as i64;
        // Normal, con quita pendiente (con y sin destino), con rango y arrastrando.
        let mut banco = banco_p5(ppp, 3);
        let panel = banco.rect_panel();
        comprueba_dentro_del_panel(&banco.reposo(), panel, &format!("ppp {ppp}: reposo"));
        banco.clic_boton(PaletteButton::Remove);
        comprueba_dentro_del_panel(&banco.reposo(), panel, &format!("ppp {ppp}: quita"));
        banco.clic_muestra(2);
        comprueba_dentro_del_panel(&banco.reposo(), panel, &format!("ppp {ppp}: destino"));
        banco.ops.cancel_removal();
        banco.clic_muestra_con(1, MAYUS);
        comprueba_dentro_del_panel(&banco.reposo(), panel, &format!("ppp {ppp}: rango"));
        banco.ops.clear_range();
        // Hacia atrás y hacia delante, con la barra a izquierda y derecha de la muestra.
        for (desde, hasta) in [(3, 0), (0, 4)] {
            let f = banco.arrastra(desde, hasta);
            assert!(banco.ops.busy());
            comprueba_dentro_del_panel(&f, panel, &format!("ppp {ppp}: {desde} a {hasta}"));
            banco.fotograma(vec![tecla(Key::Escape)]);
            let (x, y) = banco.centro_de_la_muestra(hasta);
            banco.suelta(x, y);
        }
        // Con el panel desplazado y una quita pendiente puesta a mano.
        let mut banco = Banco::con_alto_de_panel(ppp, 200 * n, imagen_p5(), ColorChoice::Entry(3));
        let panel = banco.rect_panel();
        banco.ops.begin_removal(3, Some(0));
        let (x, y) = banco.sobre_el_texto();
        banco.mueve(x, y);
        let max = banco.max_offset();
        for objetivo in [0, n, 84 * n, max] {
            banco.rueda(-100_000 * n);
            banco.rueda(objetivo);
            let contexto = format!("ppp {ppp}, offset {}", banco.view.offset_px);
            comprueba_dentro_del_panel(&banco.reposo(), panel, &contexto);
        }
    }
}

#[test]
fn fila_2_show_editing_view_sin_ops_pinta_los_botones_pero_sin_estado() {
    for ppp in PPPS {
        let mut banco = banco_p5(ppp, 3);
        banco.sin_ops = true;
        let f = banco.reposo();
        comprueba_botones(&f, &banco, &NORMALES, &format!("ppp {ppp}"));
        // Quitar una entrada usada no deja pendiente nada: el estado se pierde cada fotograma.
        let f = banco.clic_boton(PaletteButton::Remove);
        assert_eq!(f.salida.op, None, "ppp {ppp}");
        let f = banco.reposo();
        assert!(
            f.pinta_icono(&banco.iconos.set, Icon::Remove) && !f.pinta("Confirmar"),
            "ppp {ppp}"
        );
        assert!(banco.ops.removal().is_none());
        // `Mayús` + clic no deja rango.
        banco.clic_muestra_con(1, MAYUS);
        let f = banco.reposo();
        assert_eq!(banco.ops.range_end(), None, "ppp {ppp}");
        assert!(!hay_anillo(&f, &banco, 2, 1, RING_RANGE), "ppp {ppp}");
        // Un arrastre no se arma.
        banco.arrastra(0, 2);
        assert!(!banco.ops.busy(), "ppp {ppp}");
        // Los botones que no necesitan estado siguen dando su operación.
        banco.sin_ops = true;
        let f = banco.clic_boton(PaletteButton::Add);
        assert_eq!(f.salida.op, Some(PaletteOp::Add), "ppp {ppp}");
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 3: Añadir
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_3_pulsar_anadir_da_op_add_y_nada_mas() {
    for ppp in PPPS {
        let mut banco = banco_f(ppp, 1);
        let f = banco.clic_boton(PaletteButton::Add);
        assert_eq!(
            f.salida,
            PanelOutput {
                op: Some(PaletteOp::Add),
                ..PanelOutput::default()
            },
            "ppp {ppp}"
        );
        banco.reposo();
        banco.reposo();
        assert_eq!(
            banco.operaciones(),
            vec![PaletteOp::Add],
            "ppp {ppp}: una sola vez"
        );
        assert!(banco.ops.removal().is_none());
        assert!(banco.avisos().is_empty());
        assert!(banco.elecciones().is_empty());
    }
}

#[test]
fn fila_3_con_256_entradas_anadir_da_el_aviso_y_ningun_op() {
    for ppp in PPPS {
        let mut banco = Banco::nuevo(
            ppp,
            PANTALLA_ALTA,
            imagen_indexada_256(),
            ColorChoice::Entry(5),
        );
        let f = banco.reposo();
        assert_eq!(banco.tinta(&f, Icon::Add), TEXT_DISABLED, "ppp {ppp}: atenuado");
        assert_eq!(banco.tinta(&f, Icon::Remove), TEXT, "ppp {ppp}");
        assert_eq!(banco.tinta(&f, Icon::RotateLeft), TEXT);
        let f = banco.clic_boton(PaletteButton::Add);
        assert_eq!(f.salida.op, None, "ppp {ppp}");
        assert_eq!(
            f.salida.notice.as_deref(),
            Some("paleta llena: 256 entradas"),
            "ppp {ppp}"
        );
        assert!(banco.operaciones().is_empty());
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 4: Quitar
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_4_quitar_una_entrada_sin_pixeles_da_remove_con_el_destino_por_defecto() {
    for ppp in PPPS {
        let mut banco = banco_p5(ppp, 4);
        let f = banco.clic_boton(PaletteButton::Remove);
        assert_eq!(
            f.salida.op,
            Some(PaletteOp::Remove { index: 4, into: 2 }),
            "ppp {ppp}: el más cercano a Y es B"
        );
        assert!(f.salida.op_unused, "ppp {ppp}");
        assert_eq!(f.salida.notice, None);
        assert!(banco.ops.removal().is_none(), "ppp {ppp}: nada pendiente");
        banco.reposo();
        assert_eq!(banco.operaciones().len(), 1);
    }
}

#[test]
fn fila_4_quitar_una_usada_deja_la_quita_pendiente_con_las_lineas_del_aviso() {
    for ppp in PPPS {
        let mut banco = banco_p5(ppp, 3);
        let f = banco.clic_boton(PaletteButton::Remove);
        assert_eq!(f.salida.op, None, "ppp {ppp}");
        assert!(!f.salida.op_unused);
        assert_eq!(
            banco.ops.removal(),
            Some(Removal {
                index: 3,
                into: Some(0)
            }),
            "ppp {ppp}"
        );
        let f = banco.reposo();
        assert!(f.pinta("quitar 3; destino 0"), "ppp {ppp}");
        assert!(f.pinta("elige otra muestra o confirma"), "ppp {ppp}");
        // La quitada lleva el anillo activo (2 u) y el destino el anillo de aviso (2 u).
        assert!(hay_anillo(&f, &banco, 3, 2, ACCENT), "ppp {ppp}: activo");
        assert!(
            hay_anillo(&f, &banco, 0, 2, RING_DEST),
            "ppp {ppp}: destino"
        );
        assert!(!hay_anillo(&f, &banco, 2, 2, RING_DEST), "ppp {ppp}");
        comprueba_dentro_del_panel(&f, banco.rect_panel(), &format!("ppp {ppp}"));
    }
}

#[test]
fn fila_4_pulsar_otra_muestra_cambia_el_destino_y_no_la_seleccion() {
    for ppp in PPPS {
        let mut banco = banco_p5(ppp, 3);
        banco.clic_boton(PaletteButton::Remove);
        banco.historial.clear();

        let f = banco.clic_muestra(2);
        assert_eq!(f.salida.choice, None, "ppp {ppp}: no elige");
        assert_eq!(f.salida.op, None);
        assert_eq!(
            banco.ops.removal(),
            Some(Removal {
                index: 3,
                into: Some(2)
            }),
            "ppp {ppp}"
        );
        assert!(f.pinta("quitar 3; destino 2"), "ppp {ppp}");
        assert!(hay_anillo(&f, &banco, 2, 2, RING_DEST), "ppp {ppp}");
        assert!(!hay_anillo(&f, &banco, 0, 2, RING_DEST), "ppp {ppp}");
        assert_eq!(banco.choice, ColorChoice::Entry(3));

        // La propia entrada se ignora.
        banco.clic_muestra(3);
        assert_eq!(
            banco.ops.removal(),
            Some(Removal {
                index: 3,
                into: Some(2)
            }),
            "ppp {ppp}: la propia se ignora"
        );
        // Y no se arma ningún arrastre mientras hay quita pendiente.
        assert!(banco.ops.drag_state().is_none(), "ppp {ppp}");
        assert!(
            banco.elecciones().is_empty(),
            "ppp {ppp}: la selección no cambia"
        );
        assert_eq!(banco.choice, ColorChoice::Entry(3));
        assert!(banco.operaciones().is_empty());
    }
}

#[test]
fn fila_4_confirmar_da_el_remove_y_cierra_la_quita() {
    for ppp in PPPS {
        let mut banco = banco_p5(ppp, 3);
        banco.clic_boton(PaletteButton::Remove);
        banco.clic_muestra(2);
        let f = banco.clic_boton(PaletteButton::Confirm);
        assert_eq!(
            f.salida.op,
            Some(PaletteOp::Remove { index: 3, into: 2 }),
            "ppp {ppp}"
        );
        assert!(!f.salida.op_unused, "ppp {ppp}: con píxeles");
        assert_eq!(f.salida.notice, None);
        assert!(banco.ops.removal().is_none(), "ppp {ppp}");
        // Un solo `Remove`, y los botones vuelven a ser Añadir y Quitar.
        let f = banco.reposo();
        assert_eq!(banco.operaciones().len(), 1, "ppp {ppp}");
        comprueba_botones(&f, &banco, &NORMALES, &format!("ppp {ppp}"));
    }
}

#[test]
fn fila_4_confirmar_con_el_destino_por_defecto() {
    let mut banco = banco_p5(1.0, 3);
    banco.clic_boton(PaletteButton::Remove);
    let f = banco.clic_boton(PaletteButton::Confirm);
    assert_eq!(f.salida.op, Some(PaletteOp::Remove { index: 3, into: 0 }));
}

#[test]
fn fila_4_cancelar_y_escape_cancelan_sin_op_ni_aviso() {
    for ppp in PPPS {
        for como in ["Cancelar", "Escape"] {
            let mut banco = banco_p5(ppp, 3);
            banco.clic_boton(PaletteButton::Remove);
            assert!(banco.ops.removal().is_some());
            banco.historial.clear();
            if como == "Cancelar" {
                banco.clic_boton(PaletteButton::Cancel);
            } else {
                banco.fotograma(vec![tecla(Key::Escape)]);
            }
            let f = banco.reposo();
            let contexto = format!("ppp {ppp}, {como}");
            assert!(banco.ops.removal().is_none(), "{contexto}");
            assert!(banco.historial_vacio(), "{contexto}: ni op ni aviso");
            comprueba_botones(&f, &banco, &NORMALES, &contexto);
            assert_eq!(banco.choice, ColorChoice::Entry(3), "{contexto}");
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 5: Quitar con transparente, con una entrada, destino transparente
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_5_la_transparente_no_tiene_destino_y_confirmar_avisa_hasta_elegir_uno() {
    for ppp in PPPS {
        let mut banco = Banco::nuevo(
            ppp,
            PANTALLA_ALTA,
            imagen_p5_transparente(3),
            ColorChoice::Entry(3),
        );
        let f = banco.clic_boton(PaletteButton::Remove);
        assert_eq!(f.salida.op, None, "ppp {ppp}");
        assert_eq!(
            banco.ops.removal(),
            Some(Removal {
                index: 3,
                into: None
            }),
            "ppp {ppp}"
        );
        let f = banco.reposo();
        assert!(f.pinta("quitar 3: pulsa un destino"), "ppp {ppp}");
        assert!(f.pinta("aviso: sus píxeles quedan opacos"), "ppp {ppp}");
        assert_eq!(
            f.color_de("Confirmar"),
            TEXT_DISABLED,
            "ppp {ppp}: desactivado"
        );
        assert_eq!(f.color_de("Cancelar"), TEXT, "ppp {ppp}");
        assert!(!hay_anillo(&f, &banco, 4, 2, RING_DEST), "ppp {ppp}");

        // Confirmar sin destino: aviso, ningún op y la quita sigue.
        let f = banco.clic_boton(PaletteButton::Confirm);
        assert_eq!(f.salida.op, None, "ppp {ppp}");
        assert_eq!(
            f.salida.notice.as_deref(),
            Some("elige un destino pulsando una muestra"),
            "ppp {ppp}"
        );
        assert_eq!(banco.ops.removal().map(|r| r.index), Some(3), "ppp {ppp}");

        // Elegir el 4 activa Confirmar.
        let f = banco.clic_muestra(4);
        assert!(f.pinta("quitar 3; destino 4"), "ppp {ppp}");
        assert!(f.pinta("aviso: sus píxeles quedan opacos"), "ppp {ppp}");
        assert_eq!(f.color_de("Confirmar"), TEXT, "ppp {ppp}: activo");
        assert!(hay_anillo(&f, &banco, 4, 2, RING_DEST), "ppp {ppp}");
        let f = banco.clic_boton(PaletteButton::Confirm);
        assert_eq!(
            f.salida.op,
            Some(PaletteOp::Remove { index: 3, into: 4 }),
            "ppp {ppp}"
        );
        assert!(banco.ops.removal().is_none());
        assert_eq!(banco.avisos().len(), 1, "ppp {ppp}: solo el de antes");
    }
}

#[test]
fn fila_5_con_destino_la_transparente_se_pinta_el_aviso_de_destino_transparente() {
    for ppp in PPPS {
        let mut banco = Banco::nuevo(
            ppp,
            PANTALLA_ALTA,
            imagen_p5_transparente(1),
            ColorChoice::Entry(3),
        );
        banco.clic_boton(PaletteButton::Remove);
        let f = banco.clic_muestra(1);
        assert!(f.pinta("quitar 3; destino 1"), "ppp {ppp}");
        assert!(f.pinta("aviso: destino transparente"), "ppp {ppp}");
        assert!(!f.pinta("elige otra muestra o confirma"), "ppp {ppp}");
        // Con otro destino vuelve la ayuda.
        let f = banco.clic_muestra(2);
        assert!(f.pinta("elige otra muestra o confirma"), "ppp {ppp}");
        assert!(!f.pinta_algo_con("aviso:"), "ppp {ppp}");
    }
}

#[test]
fn fila_5_con_una_entrada_quitar_avisa_y_no_deja_nada_pendiente() {
    for ppp in PPPS {
        let mut banco = Banco::nuevo(
            ppp,
            PANTALLA_ALTA,
            imagen_una_entrada(),
            ColorChoice::Entry(0),
        );
        let f = banco.reposo();
        assert_eq!(
            banco.tinta(&f, Icon::Remove),
            TEXT_DISABLED,
            "ppp {ppp}: atenuado"
        );
        assert_eq!(banco.tinta(&f, Icon::Add), TEXT, "ppp {ppp}");
        assert_eq!(
            banco.tinta(&f, Icon::RotateRight),
            TEXT,
            "ppp {ppp}: rotar nunca se atenúa"
        );
        let f = banco.clic_boton(PaletteButton::Remove);
        assert_eq!(f.salida.op, None, "ppp {ppp}");
        assert_eq!(
            f.salida.notice.as_deref(),
            Some("no se puede quitar la única entrada"),
            "ppp {ppp}"
        );
        assert!(banco.ops.removal().is_none(), "ppp {ppp}");
        // Añadir sí se puede.
        let f = banco.clic_boton(PaletteButton::Add);
        assert_eq!(f.salida.op, Some(PaletteOp::Add), "ppp {ppp}");
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 6: Rotar y rango
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_6_rotar_sin_rango_gira_toda_la_paleta_y_no_toca_la_seleccion() {
    for ppp in PPPS {
        let mut banco = banco_f(ppp, 2);
        let f = banco.clic_boton(PaletteButton::RotateRight);
        assert_eq!(
            f.salida.op,
            Some(PaletteOp::Rotate {
                start: 0,
                len: 4,
                steps: 1
            }),
            "ppp {ppp}: derecha = +1"
        );
        let f = banco.clic_boton(PaletteButton::RotateLeft);
        assert_eq!(
            f.salida.op,
            Some(PaletteOp::Rotate {
                start: 0,
                len: 4,
                steps: -1
            }),
            "ppp {ppp}: izquierda = −1"
        );
        assert_eq!(banco.choice, ColorChoice::Entry(2), "ppp {ppp}");
        assert!(banco.avisos().is_empty(), "ppp {ppp}");
        assert!(banco.elecciones().is_empty());
    }
}

#[test]
fn fila_6_con_256_entradas_rotar_gira_las_256() {
    let mut banco = Banco::nuevo(
        1.0,
        PANTALLA_ALTA,
        imagen_indexada_256(),
        ColorChoice::Entry(5),
    );
    let f = banco.clic_boton(PaletteButton::RotateRight);
    assert_eq!(
        f.salida.op,
        Some(PaletteOp::Rotate {
            start: 0,
            len: 256,
            steps: 1
        })
    );
}

#[test]
fn fila_6_mayus_clic_fija_el_rango_y_rotar_lo_usa_y_lo_conserva() {
    for ppp in PPPS {
        // Desde Entry(1) con el otro extremo en la 3, y al revés desde Entry(3) con la 1.
        for (activa, otro) in [(1u8, 3usize), (3, 1)] {
            let mut banco = banco_f(ppp, activa);
            let f = banco.clic_muestra_con(otro, MAYUS);
            assert_eq!(f.salida.choice, None, "ppp {ppp}: no cambia la selección");
            assert_eq!(banco.ops.range_end(), Some(otro as u8), "ppp {ppp}");
            assert_eq!(banco.choice, ColorChoice::Entry(activa));
            assert!(banco.elecciones().is_empty());

            let f = banco.clic_boton(PaletteButton::RotateRight);
            assert_eq!(
                f.salida.op,
                Some(PaletteOp::Rotate {
                    start: 1,
                    len: 3,
                    steps: 1
                }),
                "ppp {ppp}, activa {activa}"
            );
            let f = banco.clic_boton(PaletteButton::RotateLeft);
            assert_eq!(
                f.salida.op,
                Some(PaletteOp::Rotate {
                    start: 1,
                    len: 3,
                    steps: -1
                }),
                "ppp {ppp}, activa {activa}"
            );
            // El panel no borra el rango al rotar (se puede repetir) ni cambia la selección.
            assert_eq!(banco.ops.range_end(), Some(otro as u8), "ppp {ppp}");
            assert_eq!(banco.choice, ColorChoice::Entry(activa));
        }
    }
}

#[test]
fn fila_6_se_ven_los_anillos_del_rango_y_la_bandera_rango() {
    for ppp in PPPS {
        let mut banco = banco_f(ppp, 1);
        let antes = banco.reposo();
        assert!(!hay_anillo(&antes, &banco, 2, 1, RING_RANGE), "ppp {ppp}");
        let f = banco.clic_muestra_con(3, MAYUS);
        // Anillos finos (1 u) en las muestras del rango que no son la activa; la activa lleva el
        // suyo de 2 u y la 0 (fuera del rango) ninguno.
        assert!(hay_anillo(&f, &banco, 2, 1, RING_RANGE), "ppp {ppp}: la 2");
        assert!(hay_anillo(&f, &banco, 3, 1, RING_RANGE), "ppp {ppp}: la 3");
        assert!(hay_anillo(&f, &banco, 1, 2, ACCENT), "ppp {ppp}: la activa");
        assert!(!hay_anillo(&f, &banco, 0, 1, RING_RANGE), "ppp {ppp}: la 0");
        // El cursor está sobre la 3, que está en el rango y no es la activa.
        assert!(f.pinta("cursor · rango"), "ppp {ppp}");
        comprueba_dentro_del_panel(&f, banco.rect_panel(), &format!("ppp {ppp}"));
        // Con el cursor en la 2 y luego en la activa.
        let (x, y) = banco.centro_de_la_muestra(2);
        assert!(banco.mueve(x, y).pinta("cursor · rango"), "ppp {ppp}");
        let (x, y) = banco.centro_de_la_muestra(1);
        let f = banco.mueve(x, y);
        assert!(
            f.pinta("cursor · activa"),
            "ppp {ppp}: la activa no lleva `rango`"
        );
        assert!(!f.pinta_algo_con("rango"), "ppp {ppp}");
        // Sin el cursor en la rejilla no sale la bandera en la información de la elegida.
        let f = banco.mueve(300 * banco.n(), 100 * banco.n());
        assert!(f.pinta("activa"), "ppp {ppp}");
    }
}

#[test]
fn fila_6_mayus_clic_no_arma_arrastre_aunque_el_puntero_se_mueva_20_px() {
    for ppp in PPPS {
        let mut banco = banco_f(ppp, 1);
        let (x, y) = banco.centro_de_la_muestra(3);
        banco.mueve(x, y);
        banco.pulsa_con(x, y, MAYUS);
        assert!(banco.ops.drag_state().is_none(), "ppp {ppp}: no se arma");
        assert_eq!(banco.ops.range_end(), Some(3));
        // 20 px a la izquierda (a la muestra 2 o su separación) y luego a la muestra 0.
        let f = banco.mueve(x - 20, y);
        assert!(!banco.ops.busy(), "ppp {ppp}");
        assert!(!f.pinta_algo_con("mover"), "ppp {ppp}");
        let (x0, y0) = banco.centro_de_la_muestra(0);
        banco.mueve(x0, y0);
        let f = banco.suelta_con(x0, y0, MAYUS);
        assert_eq!(f.salida.op, None, "ppp {ppp}: ningún Move");
        assert!(banco.operaciones().is_empty());
        assert_eq!(banco.ops.range_end(), Some(3), "ppp {ppp}");
        assert_eq!(banco.choice, ColorChoice::Entry(1));
        assert!(banco.elecciones().is_empty());
    }
}

#[test]
fn fila_6_un_clic_normal_borra_el_rango_y_elige() {
    for ppp in PPPS {
        let mut banco = banco_f(ppp, 1);
        banco.clic_muestra_con(3, MAYUS);
        assert_eq!(banco.ops.range_end(), Some(3));
        let f = banco.clic_muestra(0);
        assert_eq!(f.salida.choice, Some(ColorChoice::Entry(0)), "ppp {ppp}");
        assert_eq!(banco.ops.range_end(), None, "ppp {ppp}");
        let f = banco.clic_boton(PaletteButton::RotateRight);
        assert_eq!(
            f.salida.op,
            Some(PaletteOp::Rotate {
                start: 0,
                len: 4,
                steps: 1
            }),
            "ppp {ppp}: sin rango otra vez"
        );
        let f = banco.reposo();
        assert!(!hay_anillo(&f, &banco, 2, 1, RING_RANGE), "ppp {ppp}");
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 7: arrastrar muestras
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_7_pulsar_y_mover_menos_de_4n_y_soltar_solo_selecciona() {
    for ppp in PPPS {
        let mut banco = banco_f(ppp, 3);
        let n = banco.n();
        let (x, y) = banco.centro_de_la_muestra(0);
        banco.mueve(x, y);
        let f = banco.pulsa(x, y);
        assert_eq!(
            f.salida.choice,
            Some(ColorChoice::Entry(0)),
            "ppp {ppp}: se elige al pulsar"
        );
        assert!(banco.ops.drag_state().is_some(), "ppp {ppp}: armado");
        assert!(!banco.ops.busy(), "ppp {ppp}: armado no es ocupado");
        // 4n − 1 px: aún un clic.
        let f = banco.mueve(x + 4 * n - 1, y);
        assert!(!banco.ops.busy(), "ppp {ppp}");
        assert!(!f.pinta_algo_con("mover"), "ppp {ppp}");
        let f = banco.suelta(x + 4 * n - 1, y);
        assert_eq!(f.salida.op, None, "ppp {ppp}");
        assert!(banco.ops.drag_state().is_none(), "ppp {ppp}");
        assert_eq!(banco.choice, ColorChoice::Entry(0));
        assert!(banco.operaciones().is_empty());
        assert!(banco.avisos().is_empty());
    }
}

#[test]
fn fila_7_a_partir_de_4n_empieza_el_arrastre_con_la_barra_y_las_lineas() {
    for ppp in PPPS {
        let mut banco = banco_f(ppp, 3);
        let n = banco.n();
        let (x, y) = banco.centro_de_la_muestra(0);
        banco.mueve(x, y);
        banco.pulsa(x, y);
        // Justo 4n px (distancia euclídea 4n): empieza.
        banco.mueve(x + 4 * n, y);
        assert!(banco.ops.busy(), "ppp {ppp}: con 4n empieza");

        // Hacia la muestra 2: barra a la derecha de la muestra destino y las dos líneas.
        let (x2, y2) = banco.centro_de_la_muestra(2);
        let f = banco.mueve(x2, y2);
        assert!(banco.ops.busy(), "ppp {ppp}");
        assert!(f.pinta("mover 0 a 2"), "ppp {ppp}");
        assert!(f.pinta("Escape cancela"), "ppp {ppp}");
        let barra = drop_mark_rect(banco.celda(2), true, n as u32);
        assert!(
            hay_rect(&f, banco.rect_de_rejilla(barra), DROP_MARK),
            "ppp {ppp}: barra a la derecha de la 2"
        );
        comprueba_dentro_del_panel(&f, banco.rect_panel(), &format!("ppp {ppp}"));
        // El documento no cambia ni hay ninguna petición mientras dura.
        assert!(banco.operaciones().is_empty(), "ppp {ppp}");
        assert_eq!(banco.doc_state, 0);

        // Hacia la 0 (la misma): sin barra y con `mover 0 a 0`... la línea de destino no es un
        // movimiento: no se pinta barra.
        let (x0, y0) = banco.centro_de_la_muestra(0);
        let f = banco.mueve(x0, y0);
        let barra_0 = drop_mark_rect(banco.celda(0), false, n as u32);
        assert!(
            !hay_rect(&f, banco.rect_de_rejilla(barra_0), DROP_MARK),
            "ppp {ppp}"
        );
        banco.suelta(x0, y0);
    }
}

#[test]
fn fila_7_hacia_atras_la_barra_va_a_la_izquierda_de_la_muestra_destino() {
    for ppp in PPPS {
        let mut banco = banco_f(ppp, 0);
        let n = banco.n();
        let f = banco.arrastra(3, 1);
        assert!(banco.ops.busy(), "ppp {ppp}");
        assert!(f.pinta("mover 3 a 1"), "ppp {ppp}");
        let izquierda = drop_mark_rect(banco.celda(1), false, n as u32);
        let derecha = drop_mark_rect(banco.celda(1), true, n as u32);
        assert!(
            hay_rect(&f, banco.rect_de_rejilla(izquierda), DROP_MARK),
            "ppp {ppp}"
        );
        assert!(
            !hay_rect(&f, banco.rect_de_rejilla(derecha), DROP_MARK),
            "ppp {ppp}"
        );
        let (x, y) = banco.centro_de_la_muestra(1);
        banco.suelta(x, y);
    }
}

#[test]
fn fila_7_soltar_en_otra_muestra_da_un_solo_move() {
    for ppp in PPPS {
        let mut banco = banco_f(ppp, 3);
        banco.arrastra(0, 2);
        let (x, y) = banco.centro_de_la_muestra(2);
        let f = banco.suelta(x, y);
        assert_eq!(
            f.salida.op,
            Some(PaletteOp::Move { from: 0, to: 2 }),
            "ppp {ppp}"
        );
        assert!(!banco.ops.busy(), "ppp {ppp}: el arrastre termina");
        assert!(banco.ops.drag_state().is_none());
        // La elección del pulsar (la 0) es lo único que pidió el panel además del `Move`; la
        // selección final (`to`) la fija `Session` al aplicar.
        assert_eq!(banco.elecciones(), vec![ColorChoice::Entry(0)], "ppp {ppp}");
        for _ in 0..3 {
            banco.reposo();
        }
        let (x1, y1) = banco.centro_de_la_muestra(1);
        banco.mueve(x1, y1);
        banco.reposo();
        assert_eq!(
            banco.operaciones(),
            vec![PaletteOp::Move { from: 0, to: 2 }],
            "ppp {ppp}: un solo op"
        );
        assert!(banco.avisos().is_empty());
    }
}

#[test]
fn fila_7_con_16_o_mas_entradas_el_destino_es_el_indice_final() {
    // 256 entradas, 0 a 17 (segunda fila, segunda columna) y de 17 a 255 (la última).
    let mut banco = Banco::nuevo(
        1.0,
        PANTALLA_ALTA,
        imagen_indexada_256(),
        ColorChoice::Entry(9),
    );
    banco.arrastra(0, 17);
    let (x, y) = banco.centro_de_la_muestra(17);
    let f = banco.suelta(x, y);
    assert_eq!(f.salida.op, Some(PaletteOp::Move { from: 0, to: 17 }));
    banco.arrastra(17, 255);
    let (x, y) = banco.centro_de_la_muestra(255);
    let f = banco.suelta(x, y);
    assert_eq!(f.salida.op, Some(PaletteOp::Move { from: 17, to: 255 }));
}

#[test]
fn fila_7_soltar_en_la_misma_fuera_de_la_rejilla_o_fuera_del_panel_no_hace_nada() {
    for ppp in PPPS {
        let n_ppp = (ppp + 0.5).floor().max(1.0) as i64;
        // Puntos de suelta, en píxeles: (descripción, x, y) con el bloque sin desplazar.
        let derecha_de_la_rejilla = (228 * n_ppp, (GRID_Y_U + 5) * n_ppp);
        let debajo_de_la_ultima_fila = (10 * n_ppp, (GRID_Y_U + 30) * n_ppp);
        let sobre_el_texto = (10 * n_ppp, 10 * n_ppp);
        let sobre_el_lienzo = (400 * n_ppp, (GRID_Y_U + 5) * n_ppp);
        let x_negativa = (-5, (GRID_Y_U + 5) * n_ppp);
        let casos = [
            ("la misma", None),
            ("a la derecha de la rejilla", Some(derecha_de_la_rejilla)),
            ("debajo de la última fila", Some(debajo_de_la_ultima_fila)),
            ("sobre el texto", Some(sobre_el_texto)),
            ("sobre el lienzo", Some(sobre_el_lienzo)),
            ("fuera de la ventana por la izquierda", Some(x_negativa)),
        ];
        for (nombre, punto) in casos {
            let mut banco = banco_f(ppp, 3);
            let (x0, y0) = banco.centro_de_la_muestra(0);
            banco.mueve(x0, y0);
            banco.pulsa(x0, y0);
            // Pasa el umbral con un destino válido (la muestra 2) y luego va al punto de suelta.
            let (x2, y2) = banco.centro_de_la_muestra(2);
            banco.mueve(x2, y2);
            assert!(banco.ops.busy(), "ppp {ppp}, {nombre}");
            let (xs, ys) = punto.unwrap_or((x0, y0));
            let f = banco.mueve(xs, ys);
            if punto.is_some() {
                assert!(
                    f.pinta_algo_con("suelta en la rejilla"),
                    "ppp {ppp}, {nombre}"
                );
            }
            let f = banco.suelta(xs, ys);
            assert_eq!(f.salida.op, None, "ppp {ppp}, {nombre}: no mueve");
            assert!(banco.operaciones().is_empty(), "ppp {ppp}, {nombre}");
            assert!(!banco.ops.busy(), "ppp {ppp}, {nombre}");
            assert!(banco.avisos().is_empty(), "ppp {ppp}, {nombre}");
            banco.reposo();
            assert!(banco.operaciones().is_empty(), "ppp {ppp}, {nombre}");
        }
    }
}

#[test]
fn fila_7_escape_cancela_el_arrastre_y_la_pulsada_sigue_elegida() {
    for ppp in PPPS {
        let mut banco = banco_f(ppp, 1);
        banco.arrastra(3, 0);
        assert!(banco.ops.busy(), "ppp {ppp}");
        let f = banco.fotograma(vec![tecla(Key::Escape)]);
        assert!(!banco.ops.busy(), "ppp {ppp}");
        assert!(banco.ops.drag_state().is_none(), "ppp {ppp}");
        assert!(
            !f.pinta_algo_con("mover"),
            "ppp {ppp}: ya no se pintan las líneas"
        );
        let (x, y) = banco.centro_de_la_muestra(0);
        let f = banco.suelta(x, y);
        assert_eq!(
            f.salida.op, None,
            "ppp {ppp}: soltar tras cancelar no mueve"
        );
        assert!(banco.operaciones().is_empty(), "ppp {ppp}");
        assert_eq!(
            banco.choice,
            ColorChoice::Entry(3),
            "ppp {ppp}: la pulsada sigue elegida"
        );
        assert!(banco.avisos().is_empty(), "ppp {ppp}: cancelar no avisa");
    }
}

#[test]
fn fila_7_perder_el_foco_la_ventana_cancela_el_arrastre() {
    for ppp in PPPS {
        let mut banco = banco_f(ppp, 1);
        banco.arrastra(3, 0);
        assert!(banco.ops.busy(), "ppp {ppp}");
        banco.fotograma(vec![Event::WindowFocused(false)]);
        assert!(!banco.ops.busy(), "ppp {ppp}");
        assert!(banco.ops.drag_state().is_none(), "ppp {ppp}");
        let (x, y) = banco.centro_de_la_muestra(0);
        let f = banco.suelta(x, y);
        assert_eq!(f.salida.op, None, "ppp {ppp}");
        assert!(banco.operaciones().is_empty(), "ppp {ppp}");
        assert_eq!(banco.choice, ColorChoice::Entry(3), "ppp {ppp}");
        assert!(banco.avisos().is_empty(), "ppp {ppp}");
    }
}

/// Con 256 entradas y la rejilla con filas ocultas, la rueda durante el arrastre desplaza las
/// filas y el destino sigue a la muestra que queda bajo el puntero.
#[test]
fn fila_7_la_rueda_durante_el_arrastre_desplaza_las_filas() {
    for ppp in PPPS {
        let n = (ppp + 0.5).floor().max(1.0) as i64;
        let mut banco =
            Banco::con_alto_de_panel(ppp, 400 * n, imagen_indexada_256(), ColorChoice::Entry(0));
        assert!(!banco.disposicion().scrolling, "ppp {ppp}");
        let visibles = banco.disposicion().visible_rows;
        assert!(
            visibles < 16,
            "ppp {ppp}: faltan filas por ver ({visibles})"
        );
        assert_eq!(banco.scroll.first_row, 0);

        banco.arrastra(0, 17);
        assert!(banco.ops.busy(), "ppp {ppp}");
        let f = banco.rueda(2 * PITCH_U * n);
        assert_eq!(
            banco.scroll.first_row, 2,
            "ppp {ppp}: la rueda desplaza las filas"
        );
        assert_eq!(banco.view.offset_px, 0, "ppp {ppp}");
        assert!(banco.ops.busy(), "ppp {ppp}: el arrastre sigue");
        // El puntero sigue en la fila 1 y columna 1 de lo que se ve: la muestra (2 + 1)·16 + 1.
        assert!(f.pinta("mover 0 a 49"), "ppp {ppp}");
        let (x, y) = centro_de_muestra(n, 17, 0);
        let f = banco.suelta(x, y);
        assert_eq!(
            f.salida.op,
            Some(PaletteOp::Move { from: 0, to: 49 }),
            "ppp {ppp}"
        );
    }
}

/// **[DEPENDE DE EGUI]** Si egui ya no ve el botón abajo (se perdió el soltar), es un soltar.
#[test]
fn fila_7_si_egui_ya_no_ve_el_boton_abajo_es_un_soltar() {
    for ppp in PPPS {
        let mut banco = banco_f(ppp, 3);
        banco.arrastra(0, 2);
        assert!(banco.ops.busy(), "ppp {ppp}");
        // El soltar llega a egui pero el panel no lo ve.
        let (x, y) = banco.centro_de_la_muestra(2);
        let pos = en_puntos(ppp, x, y);
        banco.fotograma_sin_panel(vec![boton(pos, false, Modifiers::NONE)]);
        assert!(banco.ops.busy(), "ppp {ppp}: el panel aún no lo sabe");
        let f = banco.reposo();
        assert_eq!(
            f.salida.op,
            Some(PaletteOp::Move { from: 0, to: 2 }),
            "ppp {ppp}: el fotograma siguiente lo trata como un soltar"
        );
        assert!(!banco.ops.busy(), "ppp {ppp}");
        banco.reposo();
        assert_eq!(banco.operaciones().len(), 1, "ppp {ppp}: un solo op");
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 9: gestos en curso, `enabled` falso y teclado
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_9_con_enabled_falso_ni_botones_ni_muestras_ni_mayus_clic_hacen_nada() {
    for ppp in PPPS {
        let imagenes: [(&str, Image, u8, PaletteButton); 3] = [
            ("F", imagen_f(), 1, PaletteButton::Remove),
            // Con 256 entradas, `Añadir` avisaría si estuviera activo.
            ("256", imagen_indexada_256(), 5, PaletteButton::Add),
            // Con una sola entrada, `Quitar` avisaría.
            ("1", imagen_una_entrada(), 0, PaletteButton::Remove),
        ];
        for (nombre, image, entrada, aviso) in imagenes {
            let mut banco = Banco::nuevo(ppp, PANTALLA_ALTA, image, ColorChoice::Entry(entrada));
            banco.gesto_dibujo = true;
            let contexto = format!("ppp {ppp}, {nombre}");
            for button in NORMALES {
                banco.clic_boton(button);
            }
            banco.clic_boton(aviso);
            banco.clic_muestra(0);
            banco.clic_muestra_con(0, MAYUS);
            banco.arrastra(0, 0);
            assert!(!banco.ops.busy(), "{contexto}");
            let (x, y) = banco.centro_de_la_muestra(0);
            banco.suelta(x, y);
            let f = banco.reposo();
            assert!(banco.historial_vacio(), "{contexto}: {:?}", banco.historial);
            assert!(banco.ops.removal().is_none(), "{contexto}");
            assert_eq!(banco.ops.range_end(), None, "{contexto}");
            assert!(banco.ops.drag_state().is_none(), "{contexto}");
            assert_eq!(banco.choice, ColorChoice::Entry(entrada), "{contexto}");
            // Se ven igual.
            comprueba_botones(&f, &banco, &NORMALES, &contexto);
        }
    }
}

#[test]
fn fila_9_con_enabled_falso_y_una_quita_pendiente_nada_la_cambia() {
    for ppp in PPPS {
        let mut banco = banco_p5(ppp, 3);
        banco.clic_boton(PaletteButton::Remove);
        let pendiente = banco.ops.removal();
        assert!(pendiente.is_some());
        banco.historial.clear();
        banco.gesto_dibujo = true;
        for button in [PaletteButton::Confirm, PaletteButton::Cancel] {
            banco.clic_boton(button);
        }
        banco.clic_muestra(2);
        banco.clic_muestra_con(1, MAYUS);
        assert_eq!(banco.ops.removal(), pendiente, "ppp {ppp}");
        assert!(banco.historial_vacio(), "ppp {ppp}");
        let f = banco.reposo();
        comprueba_botones(&f, &banco, &CON_QUITA, &format!("ppp {ppp}"));
    }
}

#[test]
fn fila_9_con_un_deslizador_en_curso_los_botones_y_las_muestras_no_hacen_nada() {
    for ppp in PPPS {
        let mut banco = banco_p5(ppp, 3);
        let geo = banco.geo();
        let (x, y) = (
            geo.x_de_valor(128, Control::Red.max()),
            geo.y_fila(Control::Red),
        );
        banco.mueve(x, y);
        banco.pulsa(x, y);
        assert!(
            banco.editor.dragging(),
            "ppp {ppp}: el deslizador está en curso"
        );
        banco.historial.clear();
        // Nuevas pulsaciones (sin soltar el deslizador) sobre los controles nuevos.
        for button in NORMALES {
            let (bx, by) = banco.centro_de_boton(button);
            banco.mueve(bx, by);
            banco.pulsa(bx, by);
        }
        for (indice, mods) in [(1, Modifiers::NONE), (2, MAYUS)] {
            let (cx, cy) = banco.centro_de_la_muestra(indice);
            banco.mueve(cx, cy);
            banco.pulsa_con(cx, cy, mods);
        }
        assert!(banco.editor.dragging(), "ppp {ppp}");
        assert!(banco.operaciones().is_empty(), "ppp {ppp}");
        assert!(banco.avisos().is_empty(), "ppp {ppp}");
        assert!(banco.elecciones().is_empty(), "ppp {ppp}");
        assert!(banco.ops.removal().is_none(), "ppp {ppp}");
        assert_eq!(banco.ops.range_end(), None, "ppp {ppp}");
        assert!(banco.ops.drag_state().is_none(), "ppp {ppp}");
    }
}

#[test]
fn fila_9_con_un_arrastre_de_muestra_en_curso_los_botones_no_hacen_nada() {
    for ppp in PPPS {
        let mut banco = banco_f(ppp, 1);
        banco.arrastra(0, 2);
        assert!(banco.ops.busy(), "ppp {ppp}");
        banco.historial.clear();
        // Una pulsación más (sin soltar) sobre los botones y sobre una muestra con `Mayús`.
        for button in NORMALES {
            let (x, y) = banco.centro_de_boton(button);
            banco.mueve(x, y);
            let f = banco.pulsa(x, y);
            assert_eq!(f.salida.op, None, "ppp {ppp}: {button:?}");
            assert!(banco.ops.busy(), "ppp {ppp}: {button:?}");
        }
        let (x, y) = banco.centro_de_la_muestra(3);
        banco.mueve(x, y);
        banco.pulsa_con(x, y, MAYUS);
        assert_eq!(banco.ops.range_end(), None, "ppp {ppp}");
        assert!(banco.ops.removal().is_none(), "ppp {ppp}");
        assert!(banco.avisos().is_empty(), "ppp {ppp}");
        assert!(banco.operaciones().is_empty(), "ppp {ppp}");
    }
}

#[test]
fn fila_9_con_un_campo_enfocado_escape_no_cancela_la_quita_pendiente() {
    for escrito in [None, Some("#00ff00")] {
        let mut banco = banco_p5(1.0, 3);
        banco.clic_boton(PaletteButton::Remove);
        let pendiente = banco.ops.removal();
        assert!(pendiente.is_some());
        banco.enfoca_campo();
        if let Some(texto) = escrito {
            banco.escribe(texto);
        }
        banco.historial.clear();
        banco.fotograma(vec![tecla(Key::Escape)]);
        assert_eq!(
            banco.ops.removal(),
            pendiente,
            "{escrito:?}: Escape es del campo"
        );
        assert!(
            !banco.quiere_teclado(),
            "{escrito:?}: egui saca el foco del campo"
        );
        assert!(banco.avisos().is_empty(), "{escrito:?}");
        assert!(
            banco.commits().is_empty(),
            "{escrito:?}: Escape no confirma"
        );
        assert_eq!(banco.editor.hex_display(), None, "{escrito:?}");
        let f = banco.reposo();
        assert!(
            f.pinta("Confirmar"),
            "{escrito:?}: la quita sigue a la vista"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 10: el texto pendiente se resuelve antes de operar
// ---------------------------------------------------------------------------------------------

/// **[DEPENDE DE EGUI]** Con texto válido en el campo y el clic entero (pulsar y soltar) en un
/// solo fotograma, el panel devuelve el `Commit` y la operación juntos: `main` aplica primero el
/// commit y luego la operación.
#[test]
fn fila_10_con_el_clic_en_un_fotograma_el_commit_y_la_operacion_salen_juntos() {
    let mut banco = banco_con_pendiente(imagen_f(), ColorChoice::Entry(1), "#ff00ff");
    let (x, y) = banco.centro_de_boton(PaletteButton::Add);
    banco.mueve(x, y);
    let pos = en_puntos(banco.ppp(), x, y);
    let f = banco.fotograma(vec![
        boton(pos, true, Modifiers::NONE),
        boton(pos, false, Modifiers::NONE),
    ]);
    assert_eq!(
        f.salida.commit,
        Some(Commit {
            target: EditTarget::Entry(1),
            color: Rgba::opaque(255, 0, 255)
        })
    );
    assert_eq!(f.salida.op, Some(PaletteOp::Add));
    assert!(!banco.editor.has_pending(), "no queda texto");
    assert!(banco.avisos().is_empty());
}

/// **[DEPENDE DE EGUI]** Con texto válido en el campo, el clic en un botón (pulsar y soltar en
/// fotogramas distintos, como en una ventana real) da el `Commit` antes de la operación (spec,
/// fila 10: «el `Commit` llega antes de la operación»). egui pierde el foco al **soltar**, y el
/// botón actúa al **pulsar**, así que la operación sale un fotograma antes que el commit.
#[test]
fn fila_10_el_commit_del_campo_llega_antes_que_la_operacion_del_boton() {
    let mut banco = banco_con_pendiente(imagen_f(), ColorChoice::Entry(1), "#ff00ff");
    banco.clic_boton(PaletteButton::Add);
    banco.reposo();
    banco.reposo();
    assert_eq!(
        banco.commits(),
        vec![Commit {
            target: EditTarget::Entry(1),
            color: Rgba::opaque(255, 0, 255)
        }]
    );
    assert_eq!(banco.operaciones(), vec![PaletteOp::Add]);
    let del_commit = banco.historial.iter().position(|o| o.commit.is_some());
    let de_la_op = banco.historial.iter().position(|o| o.op.is_some());
    assert!(
        del_commit <= de_la_op,
        "el commit ({del_commit:?}) llega antes que la op ({de_la_op:?})"
    );
    assert!(!banco.editor.has_pending(), "no queda texto");
    assert!(banco.avisos().is_empty());
}

// ---------------------------------------------------------------------------------------------
// Fila 13: rotar sin nada que rotar
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_13_con_una_entrada_el_boton_de_rotar_emite_el_rotate_y_el_panel_no_avisa() {
    for ppp in PPPS {
        let mut banco = Banco::nuevo(
            ppp,
            PANTALLA_ALTA,
            imagen_una_entrada(),
            ColorChoice::Entry(0),
        );
        for steps in [1, -1] {
            let boton = if steps > 0 {
                PaletteButton::RotateRight
            } else {
                PaletteButton::RotateLeft
            };
            let f = banco.clic_boton(boton);
            assert_eq!(
                f.salida,
                PanelOutput {
                    op: Some(PaletteOp::Rotate {
                        start: 0,
                        len: 1,
                        steps
                    }),
                    ..PanelOutput::default()
                },
                "ppp {ppp}: el aviso lo da la sesión"
            );
        }
        assert!(banco.avisos().is_empty(), "ppp {ppp}");
        assert_eq!(banco.choice, ColorChoice::Entry(0));
    }
}

#[test]
fn fila_13_un_rango_de_una_entrada_emite_el_rotate_de_len_1() {
    for ppp in PPPS {
        let mut banco = banco_f(ppp, 2);
        // `Mayús` + clic en la propia activa: rango de una sola entrada.
        let f = banco.clic_muestra_con(2, MAYUS);
        assert_eq!(f.salida.choice, None, "ppp {ppp}");
        assert_eq!(banco.ops.range_end(), Some(2), "ppp {ppp}");
        for steps in [1, -1] {
            let boton = if steps > 0 {
                PaletteButton::RotateRight
            } else {
                PaletteButton::RotateLeft
            };
            let f = banco.clic_boton(boton);
            assert_eq!(
                f.salida.op,
                Some(PaletteOp::Rotate {
                    start: 2,
                    len: 1,
                    steps
                }),
                "ppp {ppp}"
            );
            assert_eq!(f.salida.notice, None, "ppp {ppp}");
        }
        assert_eq!(
            banco.ops.range_end(),
            Some(2),
            "ppp {ppp}: el rango se queda"
        );
        assert_eq!(banco.choice, ColorChoice::Entry(2));
        // El botón no está atenuado.
        let f = banco.reposo();
        assert_eq!(banco.tinta(&f, Icon::RotateRight), TEXT, "ppp {ppp}");
        assert_eq!(banco.tinta(&f, Icon::RotateLeft), TEXT, "ppp {ppp}");
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 14: prioridad de Escape
// ---------------------------------------------------------------------------------------------

/// (a) Quita pendiente con el campo hexadecimal enfocado y texto escrito: el primer `Escape` solo
/// revierte el texto y la quita sigue; el segundo la cancela.
#[test]
fn fila_14_a_escape_con_campo_enfocado_y_quita_pendiente_consume_un_nivel_cada_vez() {
    for ppp in [1.0, 2.0] {
        let mut banco = Banco::nuevo(ppp, PANTALLA_ALTA, imagen_p5(), ColorChoice::Entry(3));
        banco.clic_boton(PaletteButton::Remove);
        let pendiente = banco.ops.removal();
        assert!(pendiente.is_some());
        banco.enfoca_campo();
        banco.escribe("#00ff00");
        assert!(banco.editor.has_pending(), "ppp {ppp}");
        banco.historial.clear();

        banco.fotograma(vec![tecla(Key::Escape)]);
        assert_eq!(
            banco.editor.hex_display(),
            None,
            "ppp {ppp}: el texto se revierte"
        );
        assert!(!banco.editor.has_pending(), "ppp {ppp}");
        assert_eq!(banco.ops.removal(), pendiente, "ppp {ppp}: la quita sigue");
        assert!(!banco.quiere_teclado(), "ppp {ppp}");

        banco.fotograma(vec![tecla(Key::Escape)]);
        assert_eq!(
            banco.ops.removal(),
            None,
            "ppp {ppp}: el segundo la cancela"
        );
        assert!(
            banco.historial_vacio(),
            "ppp {ppp}: sin commit, op ni aviso"
        );
        let f = banco.reposo();
        comprueba_botones(&f, &banco, &NORMALES, &format!("ppp {ppp}"));
    }
}

/// (b) Quita pendiente con un gesto de dibujo en curso (`enabled` falso): `Escape` no cancela la
/// quita (el gesto lo cancela quien lo lleva); sin gesto, sí.
#[test]
fn fila_14_b_con_un_gesto_de_dibujo_en_curso_escape_no_cancela_la_quita() {
    for ppp in [1.0, 2.0] {
        let mut banco = Banco::nuevo(ppp, PANTALLA_ALTA, imagen_p5(), ColorChoice::Entry(3));
        banco.clic_boton(PaletteButton::Remove);
        let pendiente = banco.ops.removal();
        assert!(pendiente.is_some());
        banco.historial.clear();
        banco.gesto_dibujo = true;
        banco.fotograma(vec![tecla(Key::Escape)]);
        assert_eq!(banco.ops.removal(), pendiente, "ppp {ppp}: la quita sigue");
        // El gesto se cancela (lo hace el lienzo) y entonces el siguiente `Escape` la cancela.
        banco.gesto_dibujo = false;
        banco.fotograma(vec![tecla(Key::Escape)]);
        assert_eq!(banco.ops.removal(), None, "ppp {ppp}");
        assert!(banco.historial_vacio(), "ppp {ppp}");
    }
}

/// (c) Un arrastre de muestra en curso: `Escape` cancela solo el arrastre; la muestra pulsada
/// sigue elegida y no hay paso.
#[test]
fn fila_14_c_escape_con_un_arrastre_de_muestra_cancela_solo_el_arrastre() {
    for ppp in [1.0, 2.0] {
        let mut banco = banco_f(ppp, 1);
        banco.arrastra(3, 0);
        assert!(banco.ops.busy(), "ppp {ppp}");
        banco.historial.clear();
        banco.fotograma(vec![tecla(Key::Escape)]);
        assert!(banco.ops.drag_state().is_none(), "ppp {ppp}");
        assert_eq!(
            banco.choice,
            ColorChoice::Entry(3),
            "ppp {ppp}: la pulsada sigue elegida"
        );
        let (x, y) = banco.centro_de_la_muestra(0);
        banco.suelta(x, y);
        assert!(banco.historial_vacio(), "ppp {ppp}: ni paso ni aviso");

        // También con el botón solo armado (antes del umbral).
        let (x, y) = banco.centro_de_la_muestra(2);
        banco.mueve(x, y);
        banco.pulsa(x, y);
        assert!(
            banco.ops.drag_state().is_some() && !banco.ops.busy(),
            "ppp {ppp}: armado"
        );
        banco.fotograma(vec![tecla(Key::Escape)]);
        assert!(
            banco.ops.drag_state().is_none(),
            "ppp {ppp}: armado también se cancela"
        );
        banco.suelta(x, y);
        assert!(banco.operaciones().is_empty(), "ppp {ppp}");
    }
}

/// (d) Un deslizador en curso con una quita pendiente: el primer `Escape` cancela el deslizador
/// y restaura el color; la quita sigue; el segundo la cancela.
#[test]
fn fila_14_d_escape_con_un_deslizador_en_curso_cancela_solo_el_deslizador() {
    for ppp in [1.0, 2.0] {
        let mut banco = banco_p5(ppp, 3);
        banco.clic_boton(PaletteButton::Remove);
        let pendiente = banco.ops.removal();
        let geo = banco.geo();
        let (x, y) = (
            geo.x_de_valor(10, Control::Red.max()),
            geo.y_fila(Control::Red),
        );
        banco.mueve(x, y);
        banco.pulsa(x, y);
        assert!(banco.editor.dragging(), "ppp {ppp}");
        assert!(
            banco.editor.preview().is_some(),
            "ppp {ppp}: hay color provisional"
        );
        banco.historial.clear();

        banco.fotograma(vec![tecla(Key::Escape)]);
        assert!(
            !banco.editor.dragging(),
            "ppp {ppp}: se cancela el deslizador"
        );
        assert_eq!(
            banco.editor.preview(),
            None,
            "ppp {ppp}: el color se restaura"
        );
        assert_eq!(banco.ops.removal(), pendiente, "ppp {ppp}: la quita sigue");
        banco.suelta(x, y);
        assert!(banco.commits().is_empty(), "ppp {ppp}: no hay paso");

        banco.fotograma(vec![tecla(Key::Escape)]);
        assert_eq!(
            banco.ops.removal(),
            None,
            "ppp {ppp}: el segundo cancela la quita"
        );
        assert!(banco.historial_vacio(), "ppp {ppp}");
    }
}

/// (e) Nada en curso: `Escape` no hace nada y no avisa.
#[test]
fn fila_14_e_escape_sin_nada_en_curso_no_hace_nada() {
    for ppp in PPPS {
        let mut banco = banco_p5(ppp, 3);
        banco.clic_muestra_con(1, MAYUS);
        let rango = banco.ops.range_end();
        banco.historial.clear();
        let f = banco.fotograma(vec![tecla(Key::Escape)]);
        assert_eq!(f.salida, PanelOutput::default(), "ppp {ppp}");
        assert!(banco.historial_vacio(), "ppp {ppp}");
        assert_eq!(banco.choice, ColorChoice::Entry(3), "ppp {ppp}");
        // `Escape` no consume el rango: queda como estaba.
        assert_eq!(banco.ops.range_end(), rango, "ppp {ppp}");
        assert!(banco.ops.removal().is_none());
        assert!(!banco.ops.busy());
    }
}

// ---------------------------------------------------------------------------------------------
// El estado y el documento
// ---------------------------------------------------------------------------------------------

#[test]
fn un_cambio_del_documento_cancela_la_quita_pendiente_y_los_botones_vuelven() {
    for ppp in PPPS {
        let mut banco = banco_p5(ppp, 3);
        banco.clic_boton(PaletteButton::Remove);
        let f = banco.reposo();
        comprueba_botones(&f, &banco, &CON_QUITA, &format!("ppp {ppp}"));
        assert_eq!(banco.cancelaciones, 0);
        // Editar un color de otra entrada cambia el estado del documento.
        banco.pone_color(0, Rgba::opaque(1, 2, 3));
        let f = banco.reposo();
        assert_eq!(banco.cancelaciones, 1, "ppp {ppp}: `sync` avisa una vez");
        assert!(banco.ops.removal().is_none(), "ppp {ppp}");
        comprueba_botones(&f, &banco, &NORMALES, &format!("ppp {ppp}"));
        assert!(!f.pinta_algo_con("quitar 3"), "ppp {ppp}");
        banco.reposo();
        assert_eq!(banco.cancelaciones, 1, "ppp {ppp}: no se repite");

        // Y deshacer también cancela una quita nueva.
        banco.clic_boton(PaletteButton::Remove);
        assert!(banco.ops.removal().is_some());
        banco.deshace();
        banco.reposo();
        assert_eq!(banco.cancelaciones, 2, "ppp {ppp}");
        assert!(banco.ops.removal().is_none(), "ppp {ppp}");
    }
}

// ---------------------------------------------------------------------------------------------
// Unidad 21: botón «Fijar/Quitar transparente» (specs/hito-2-fijar-transparente.md, filas 2, 3, 5 y
// 9 en su parte de ventana). Lo puro (rótulo, `transparent_toggle`, resúmenes) está en
// `palette_ops.rs`; `button_hit` y `edges_u`, en `edit_layout.rs`; `info_flags_alpha0` e
// `is_alpha0_cell_with`, en `color.rs`; `Session`, en `px-app`. El banco no aplica las `op`: las
// pruebas que necesitan el documento nuevo lo aplican con `Banco::aplica`.
// ---------------------------------------------------------------------------------------------

mod fijar_transparente {
    use super::*;
    use px_ui::edit_layout::min_panel_height_px;
    use px_ui::text::{INTERFACE_CHARSET, fit_text};

    const FIJAR: &str = "Fijar transparente";
    const QUITAR: &str = "Quitar transparente";

    const ALFA_0: Rgba = Rgba::new(0, 255, 0, 0);

    /// `[R, G con alfa guardado 0, B, K]`, píxeles `[0, 1, 2, 3]`, con `marca` como transparente.
    fn imagen_alfa_0(marca: Option<u8>) -> Image {
        imagen_con(&[ROJO, ALFA_0, AZUL, NEGRO], marca, &[0, 1, 2, 3])
    }

    fn op_de(index: Option<u8>) -> Option<PaletteOp> {
        Some(PaletteOp::SetTransparent { index })
    }

    /// El botón `Transparent` se pinta en su sitio como icono y con el estado del rótulo: hundido
    /// (cara `SURFACE_0`, icono `ACCENT`, 1 u abajo y a la derecha) si el rótulo es `QUITAR`; si no,
    /// normal (cara `SURFACE_2` o `SURFACE_3`, icono `TEXT`). El rótulo mismo es del tooltip.
    fn comprueba_boton(f: &Fotograma, banco: &Banco, rotulo: &str, contexto: &str) {
        let (ppp, n) = (banco.ppp(), banco.n());
        let top = banco.geo().top_px;
        let (x, y, _, _) = rect_boton_u(PaletteButton::Transparent);
        let hundido = rotulo == QUITAR;
        let malla = f
            .malla_de(&banco.iconos.set, Icon::Transparent)
            .unwrap_or_else(|| panic!("{contexto}: no se pinta el icono de {rotulo:?}"));
        let desplazamiento = if hundido { 1 } else { 0 };
        let esperado = Rect::from_min_size(
            pos2(
                ((x + 4 + desplazamiento) * n) as f32 / ppp,
                (top + (y + 4 + desplazamiento) * n) as f32 / ppp,
            ),
            vec2((16 * n) as f32 / ppp, (16 * n) as f32 / ppp),
        );
        assert!(
            cerca_rect(malla.rect, esperado),
            "{contexto}: {rotulo:?}: el icono está en {:?} y la maqueta dice {esperado:?}",
            malla.rect
        );
        assert_eq!(
            malla.color,
            if hundido { ACCENT } else { TEXT },
            "{contexto}: {rotulo:?}"
        );
        let rect = banco.rect_de_boton(PaletteButton::Transparent);
        if hundido {
            assert!(hay_rect(f, rect, SURFACE_0), "{contexto}: cara hundida");
        } else {
            assert!(
                hay_rect(f, rect, SURFACE_2) || hay_rect(f, rect, SURFACE_3),
                "{contexto}: no hay cara del botón en {rect:?}"
            );
        }
    }

    /// Algún texto de la línea 2 (en `TEXT_DIM`) dice `transparente` y `alfa 0` a la vez.
    fn linea_2_mezcla_las_dos(f: &Fotograma) -> bool {
        f.textos.iter().any(|t| {
            t.color == TEXT_DIM && t.texto.contains("transparente") && t.texto.contains("alfa 0")
        })
    }

    // Fila 2: se pinta y se pulsa donde la maqueta dice.

    #[test]
    fn fila_2_el_icono_se_pinta_con_el_estado_del_rotulo_a_los_cinco_ppp() {
        for ppp in PPPS {
            let contexto = format!("ppp {ppp}");
            // Sin marca: `Fijar`; con la activa marcada: `Quitar`; con otra marcada: `Fijar`.
            for (marca, activa, esperado, otro) in [
                (None, 1u8, FIJAR, QUITAR),
                (Some(1u8), 1, QUITAR, FIJAR),
                (Some(2), 1, FIJAR, QUITAR),
            ] {
                let mut banco = Banco::nuevo(
                    ppp,
                    PANTALLA_ALTA,
                    imagen_alfa_0(marca),
                    ColorChoice::Entry(activa),
                );
                let f = banco.reposo();
                let contexto = format!("{contexto}, marca {marca:?}");
                comprueba_boton(&f, &banco, esperado, &contexto);
                // El estado contrario se ve distinto (hundido o no): `otro` no se cumple.
                assert_eq!(
                    banco.tinta(&f, Icon::Transparent) == ACCENT,
                    otro == FIJAR,
                    "{contexto}: solo un estado"
                );
                // Las tres primeras filas siguen donde estaban.
                comprueba_botones(&f, &banco, &NORMALES, &contexto);
                comprueba_dentro_del_panel(&f, banco.rect_panel(), &contexto);
            }
        }
    }

    #[test]
    fn fila_2_con_el_panel_en_su_alto_minimo_el_boton_cabe_sin_desplazar() {
        for ppp in PPPS {
            let n = i64::from(text_scale(ppp));
            let mut banco = Banco::con_alto_de_panel(
                ppp,
                min_panel_height_px(n as u32),
                imagen_f(),
                ColorChoice::Entry(1),
            );
            let contexto = format!("ppp {ppp}");
            assert!(!banco.disposicion().scrolling, "{contexto}");
            assert_eq!(banco.panel_h_px(), 305 * n, "{contexto}");
            let f = banco.reposo();
            comprueba_boton(&f, &banco, FIJAR, &contexto);
            comprueba_botones(&f, &banco, &NORMALES, &contexto);
            comprueba_dentro_del_panel(&f, banco.rect_panel(), &contexto);
            // El botón acaba 4 u antes del borde inferior del panel.
            let rect = banco.rect_de_boton(PaletteButton::Transparent);
            let alto_panel = banco.rect_panel().max.y;
            assert!(
                cerca(rect.max.y + 4.0 * n as f32 / ppp, alto_panel),
                "{contexto}"
            );
            let f = banco.clic_boton(PaletteButton::Transparent);
            assert_eq!(f.salida.op, op_de(Some(1)), "{contexto}");
        }
    }

    #[test]
    fn fila_2_pulsar_en_los_puntos_de_la_maqueta_del_boton_nuevo() {
        for ppp in PPPS {
            let mut banco = banco_f(ppp, 2);
            let n = banco.n();
            let top = banco.geo().top_px;
            // Dentro: la esquina y el último píxel físico; fuera: izquierda, derecha, 1 u sobre
            // el botón y bajo él.
            for (x, y, dentro) in [
                (118 * n, 186 * n, true),
                (142 * n - 1, 210 * n - 1, true),
                (130 * n, 198 * n, true),
                (117 * n, 198 * n, false),
                (142 * n, 198 * n, false),
                (130 * n, 185 * n, false),
                (130 * n, 210 * n, false),
            ] {
                banco.historial.clear();
                let f = banco.clic(x, top + y);
                let contexto = format!("ppp {ppp}, ({x}, {y})");
                if dentro {
                    assert_eq!(f.salida.op, op_de(Some(2)), "{contexto}");
                } else {
                    assert_eq!(f.salida.op, None, "{contexto}");
                }
                assert_eq!(f.salida.notice, None, "{contexto}");
                assert_eq!(banco.elecciones(), vec![], "{contexto}");
            }
            // Y las filas 1 a 3 siguen en sus puntos.
            let f = banco.clic(4 * n, top + 186 * n);
            assert_eq!(f.salida.op, Some(PaletteOp::Add));
        }
    }

    #[test]
    fn fila_2_con_el_panel_desplazado_el_boton_se_pulsa_con_y_menos_offset() {
        for ppp in PPPS {
            let n = i64::from(text_scale(ppp));
            let mut banco =
                Banco::con_alto_de_panel(ppp, 200 * n, imagen_f(), ColorChoice::Entry(1));
            assert!(banco.disposicion().scrolling, "ppp {ppp}");
            // Sin desplazar el botón queda por debajo del panel: no se golpea.
            let (x, _) = banco.centro_de_boton(PaletteButton::Transparent);
            let f = banco.clic(x, (55 + 248) * n);
            assert_eq!(f.salida.op, None, "ppp {ppp}: fuera del panel");
            // Con el máximo desplazamiento entero cae dentro.
            let (tx, ty) = banco.sobre_el_texto();
            banco.rueda_en(tx, ty, -100_000 * n);
            banco.rueda(banco.max_offset());
            assert_eq!(banco.view.offset_px, banco.max_offset(), "ppp {ppp}");
            let f = banco.reposo();
            comprueba_boton(&f, &banco, FIJAR, &format!("ppp {ppp}"));
            comprueba_dentro_del_panel(&f, banco.rect_panel(), &format!("ppp {ppp}"));
            let f = banco.clic_boton(PaletteButton::Transparent);
            assert_eq!(f.salida.op, op_de(Some(1)), "ppp {ppp}");
        }
    }

    // Fila 3: pulsar emite la op y el rótulo sale del estado.

    #[test]
    fn fila_3_pulsar_fijar_emite_set_transparent_y_nada_mas() {
        for ppp in PPPS {
            let mut banco = banco_f(ppp, 2);
            let f = banco.clic_boton(PaletteButton::Transparent);
            assert_eq!(
                f.salida,
                PanelOutput {
                    op: op_de(Some(2)),
                    ..PanelOutput::default()
                },
                "ppp {ppp}"
            );
            banco.reposo();
            banco.reposo();
            assert_eq!(
                banco.operaciones(),
                vec![PaletteOp::SetTransparent { index: Some(2) }]
            );
            assert!(banco.ops.removal().is_none());
            assert!(banco.avisos().is_empty());
            assert!(banco.elecciones().is_empty(), "no cambia la selección");
            assert_eq!(banco.choice, ColorChoice::Entry(2));
        }
    }

    #[test]
    fn fila_3_aplicada_la_op_el_rotulo_y_la_linea_2_cambian_y_pulsar_otra_vez_quita() {
        for ppp in PPPS {
            let mut banco = banco_f(ppp, 2);
            let id = banco.documento.main_image();
            banco.clic_boton(PaletteButton::Transparent);
            banco.aplica(Command::SetTransparentIndex {
                image: id,
                index: Some(2),
            });
            let f = banco.reposo();
            let contexto = format!("ppp {ppp}");
            comprueba_boton(&f, &banco, QUITAR, &contexto);
            assert!(f.pinta("activa · transparente"), "{contexto}");

            banco.historial.clear();
            let f = banco.clic_boton(PaletteButton::Transparent);
            assert_eq!(
                f.salida.op,
                op_de(None),
                "{contexto}: la activa es la marcada"
            );
            banco.aplica(Command::SetTransparentIndex {
                image: id,
                index: None,
            });
            let f = banco.reposo();
            comprueba_boton(&f, &banco, FIJAR, &contexto);
            assert!(f.pinta("activa"), "{contexto}");

            // Deshacer: vuelve a la marca y el rótulo cambia solo.
            banco.deshace();
            let f = banco.reposo();
            comprueba_boton(&f, &banco, QUITAR, &contexto);
            banco.deshace();
            let f = banco.reposo();
            comprueba_boton(&f, &banco, FIJAR, &contexto);
        }
    }

    #[test]
    fn fila_3_con_otra_entrada_marcada_el_boton_fija_la_activa() {
        for ppp in PPPS {
            let mut banco = Banco::nuevo(
                ppp,
                PANTALLA_ALTA,
                imagen_con(&[ROJO, VERDE, AZUL, NEGRO], Some(1), &[0, 1, 2, 3]),
                ColorChoice::Entry(3),
            );
            let f = banco.clic_boton(PaletteButton::Transparent);
            assert_eq!(f.salida.op, op_de(Some(3)), "ppp {ppp}");
        }
    }

    // Fila 5: la línea 2 de la muestra.

    #[test]
    fn fila_5_la_linea_2_de_una_marcada_de_alfa_0_sin_marcar_de_una_opaca_y_vacia() {
        for ppp in PPPS {
            // Marcada la 2; la 1 tiene alfa guardado 0; la 0 y la 3 son opacas.
            for (activa, esperado) in [
                (2u8, "activa · transparente"),
                (1, "activa · alfa 0"),
                (3, "activa"),
                (0, "activa"),
            ] {
                let mut banco = Banco::nuevo(
                    ppp,
                    PANTALLA_ALTA,
                    imagen_alfa_0(Some(2)),
                    ColorChoice::Entry(activa),
                );
                let f = banco.reposo();
                let contexto = format!("ppp {ppp}, activa {activa}");
                let linea = f
                    .texto_pintado(esperado)
                    .unwrap_or_else(|| panic!("{contexto}: no se pinta {esperado:?}"));
                assert_eq!(linea.color, TEXT_DIM, "{contexto}");
                // `transparente` y `alfa 0` nunca juntas.
                assert!(!linea_2_mezcla_las_dos(&f), "{contexto}");
            }
            // Sin cursor ni elegida en la rejilla (elegida fuera de rango no hay): vacía no se
            // puede probar aquí; una opaca solo dice `activa`.
            let mut banco = Banco::nuevo(
                ppp,
                PANTALLA_ALTA,
                imagen_alfa_0(Some(2)),
                ColorChoice::Entry(0),
            );
            let f = banco.reposo();
            assert!(!f.pinta("activa · alfa 0"), "ppp {ppp}");
            assert!(!f.pinta("activa · transparente"), "ppp {ppp}");
        }
    }

    #[test]
    fn fila_5_con_el_cursor_encima_la_linea_mas_larga_y_las_demas() {
        for ppp in PPPS {
            let mut banco = Banco::nuevo(
                ppp,
                PANTALLA_ALTA,
                imagen_alfa_0(Some(2)),
                ColorChoice::Entry(2),
            );
            let contexto = format!("ppp {ppp}");
            // Cursor sobre la activa marcada: la línea más larga, 30 columnas, sin recortar.
            let (x, y) = banco.centro_de_la_muestra(2);
            let f = banco.mueve(x, y);
            let linea = "cursor · activa · transparente";
            assert_eq!(linea.chars().count(), 30);
            assert!(f.pinta(linea), "{contexto}");
            // Cursor sobre la de alfa 0 (no es la activa).
            let (x, y) = banco.centro_de_la_muestra(1);
            assert!(banco.mueve(x, y).pinta("cursor · alfa 0"), "{contexto}");
            // Cursor sobre una opaca.
            let (x, y) = banco.centro_de_la_muestra(3);
            assert!(banco.mueve(x, y).pinta("cursor"), "{contexto}");
            // Con la activa de alfa 0 y el cursor encima.
            banco.choice = ColorChoice::Entry(1);
            let (x, y) = banco.centro_de_la_muestra(1);
            let f = banco.mueve(x, y);
            assert!(f.pinta("cursor · activa · alfa 0"), "{contexto}");
            assert!(!linea_2_mezcla_las_dos(&f), "{contexto}");
            // Y sobre la marcada, con la de alfa 0 activa.
            let (x, y) = banco.centro_de_la_muestra(2);
            assert!(
                banco.mueve(x, y).pinta("cursor · transparente"),
                "{contexto}"
            );
        }
    }

    #[test]
    fn fila_5_la_marca_de_una_entrada_con_alfa_guardado_0_dice_transparente_no_alfa_0() {
        for ppp in PPPS {
            // La 1 tiene alfa guardado 0 y está marcada: ya resuelve alfa 0 y dice `transparente`.
            let mut banco = Banco::nuevo(
                ppp,
                PANTALLA_ALTA,
                imagen_alfa_0(Some(1)),
                ColorChoice::Entry(1),
            );
            let f = banco.reposo();
            assert!(f.pinta("activa · transparente"), "ppp {ppp}");
            assert!(!f.pinta("activa · alfa 0"), "ppp {ppp}");
            // Tras `Quitar` (aplicado en el documento), pasa a decir `alfa 0`.
            let id = banco.documento.main_image();
            banco.aplica(Command::SetTransparentIndex {
                image: id,
                index: None,
            });
            let f = banco.reposo();
            assert!(f.pinta("activa · alfa 0"), "ppp {ppp}");
            assert!(!f.pinta("activa · transparente"), "ppp {ppp}");
            assert!(!linea_2_mezcla_las_dos(&f), "ppp {ppp}");
        }
    }

    #[test]
    fn fila_5_con_tres_cifras_los_rotulos_y_las_lineas_se_pintan_sin_recortar() {
        for ppp in PPPS {
            let colores: Vec<Rgba> = (0..256usize)
                .map(|i| Rgba::opaque(i as u8, 255 - i as u8, 7))
                .collect();
            let imagen = imagen_con(&colores, Some(254), &[0, 255]);
            let mut banco = Banco::nuevo(ppp, PANTALLA_ALTA, imagen, ColorChoice::Entry(255));
            let contexto = format!("ppp {ppp}");
            let f = banco.reposo();
            comprueba_boton(&f, &banco, FIJAR, &contexto);
            assert!(f.pinta("activa"), "{contexto}");
            let f = banco.clic_boton(PaletteButton::Transparent);
            assert_eq!(f.salida.op, op_de(Some(255)), "{contexto}");
            banco.choice = ColorChoice::Entry(254);
            let f = banco.reposo();
            comprueba_boton(&f, &banco, QUITAR, &contexto);
            assert!(f.pinta("activa · transparente"), "{contexto}");
            let f = banco.clic_boton(PaletteButton::Transparent);
            assert_eq!(f.salida.op, op_de(None), "{contexto}");
        }
    }

    #[test]
    fn fila_5_los_rotulos_salen_del_juego_de_caracteres_y_caben_en_225_u() {
        assert_eq!(6 + 7 * QUITAR.chars().count(), 139);
        for rotulo in [FIJAR, QUITAR, "alfa 0", "transparente"] {
            assert!(
                rotulo.chars().all(|c| INTERFACE_CHARSET.contains(c)),
                "{rotulo}"
            );
            let usado = 6 + 7 * rotulo.chars().count() as i64;
            assert!(usado <= 225, "{rotulo}: {usado} u");
            assert_eq!(
                fit_text(rotulo, 32, |c| INTERFACE_CHARSET.contains(c)),
                rotulo
            );
        }
    }

    #[test]
    fn fila_5_el_boton_nunca_esta_atenuado() {
        for ppp in PPPS {
            let casos: [(&str, Image, u8, &str); 4] = [
                ("F sin marca", imagen_f(), 1, FIJAR),
                ("una entrada", imagen_una_entrada(), 0, FIJAR),
                ("256", imagen_indexada_256(), 5, FIJAR),
                (
                    "marcada",
                    imagen_con(&[ROJO, VERDE], Some(0), &[0, 1]),
                    0,
                    QUITAR,
                ),
            ];
            for (nombre, imagen, activa, rotulo) in casos {
                let mut banco =
                    Banco::nuevo(ppp, PANTALLA_ALTA, imagen, ColorChoice::Entry(activa));
                let f = banco.reposo();
                // Nunca atenuado: normal con `TEXT`, o hundido con `ACCENT` si es la marcada.
                let esperado = if rotulo == QUITAR { ACCENT } else { TEXT };
                assert_eq!(
                    banco.tinta(&f, Icon::Transparent),
                    esperado,
                    "ppp {ppp}, {nombre}"
                );
            }
        }
    }

    #[test]
    fn fila_5_con_una_entrada_y_con_256_pulsar_emite_la_op_sin_aviso() {
        for ppp in PPPS {
            let mut banco = Banco::nuevo(
                ppp,
                PANTALLA_ALTA,
                imagen_una_entrada(),
                ColorChoice::Entry(0),
            );
            let f = banco.clic_boton(PaletteButton::Transparent);
            assert_eq!(f.salida.op, op_de(Some(0)), "ppp {ppp}");
            assert_eq!(f.salida.notice, None, "ppp {ppp}");

            let mut banco = Banco::nuevo(
                ppp,
                PANTALLA_ALTA,
                imagen_indexada_256(),
                ColorChoice::Entry(255),
            );
            let f = banco.clic_boton(PaletteButton::Transparent);
            assert_eq!(f.salida.op, op_de(Some(255)), "ppp {ppp}");
            assert_eq!(f.salida.notice, None, "ppp {ppp}");
        }
    }

    // Fila 9: bloqueos.

    /// `enabled` falso (un gesto de dibujo en curso; un diálogo abierto da el mismo `enabled`).
    #[test]
    fn fila_9_con_un_gesto_o_un_dialogo_abierto_el_boton_se_ve_y_no_hace_nada() {
        for ppp in PPPS {
            for (marca, rotulo) in [(None, FIJAR), (Some(1u8), QUITAR)] {
                let mut banco = Banco::nuevo(
                    ppp,
                    PANTALLA_ALTA,
                    imagen_alfa_0(marca),
                    ColorChoice::Entry(1),
                );
                banco.gesto_dibujo = true;
                let contexto = format!("ppp {ppp}, marca {marca:?}");
                banco.clic_boton(PaletteButton::Transparent);
                let (x, y) = banco.centro_de_boton(PaletteButton::Transparent);
                banco.pulsa(x, y);
                banco.reposo();
                banco.suelta(x, y);
                let f = banco.reposo();
                assert!(banco.historial_vacio(), "{contexto}: {:?}", banco.historial);
                assert_eq!(banco.doc_state, 0, "{contexto}");
                assert_eq!(banco.choice, ColorChoice::Entry(1), "{contexto}");
                assert!(banco.ops.removal().is_none(), "{contexto}");
                // Se ve igual.
                comprueba_boton(&f, &banco, rotulo, &contexto);
            }
        }
    }

    #[test]
    fn fila_9_con_un_deslizador_o_un_arrastre_de_muestra_en_curso_no_hace_nada() {
        for ppp in PPPS {
            // Deslizador en curso.
            let mut banco = banco_p5(ppp, 3);
            let geo = banco.geo();
            let (x, y) = (
                geo.x_de_valor(128, Control::Red.max()),
                geo.y_fila(Control::Red),
            );
            banco.mueve(x, y);
            banco.pulsa(x, y);
            assert!(banco.editor.dragging(), "ppp {ppp}");
            banco.historial.clear();
            let (bx, by) = banco.centro_de_boton(PaletteButton::Transparent);
            banco.mueve(bx, by);
            let f = banco.pulsa(bx, by);
            assert_eq!(f.salida.op, None, "ppp {ppp}: deslizador");
            assert!(banco.operaciones().is_empty(), "ppp {ppp}");
            assert!(banco.avisos().is_empty(), "ppp {ppp}");
            assert!(banco.editor.dragging(), "ppp {ppp}");

            // Arrastre de muestra en curso.
            let mut banco = banco_f(ppp, 1);
            banco.arrastra(0, 2);
            assert!(banco.ops.busy(), "ppp {ppp}");
            banco.historial.clear();
            let (bx, by) = banco.centro_de_boton(PaletteButton::Transparent);
            banco.mueve(bx, by);
            let f = banco.pulsa(bx, by);
            assert_eq!(f.salida.op, None, "ppp {ppp}: arrastre");
            assert!(banco.ops.busy(), "ppp {ppp}");
            assert!(banco.operaciones().is_empty(), "ppp {ppp}");
            assert!(banco.avisos().is_empty(), "ppp {ppp}");
        }
    }

    #[test]
    fn fila_9_con_texto_valido_en_el_campo_el_commit_llega_antes_que_la_op() {
        let mut banco = banco_con_pendiente(imagen_f(), ColorChoice::Entry(1), "#ff00ff");
        banco.clic_boton(PaletteButton::Transparent);
        banco.reposo();
        banco.reposo();
        assert_eq!(
            banco.commits(),
            vec![Commit {
                target: EditTarget::Entry(1),
                color: Rgba::opaque(255, 0, 255)
            }]
        );
        assert_eq!(
            banco.operaciones(),
            vec![PaletteOp::SetTransparent { index: Some(1) }]
        );
        let del_commit = banco.historial.iter().position(|o| o.commit.is_some());
        let de_la_op = banco.historial.iter().position(|o| o.op.is_some());
        assert!(
            del_commit.is_some() && del_commit <= de_la_op,
            "el commit ({del_commit:?}) llega antes que la op ({de_la_op:?})"
        );
        assert!(!banco.editor.has_pending(), "no queda texto");
        assert!(banco.avisos().is_empty());
    }

    #[test]
    fn fila_9_con_el_clic_en_un_solo_fotograma_el_commit_y_la_op_salen_juntos() {
        let mut banco = banco_con_pendiente(imagen_f(), ColorChoice::Entry(1), "#ff00ff");
        let (x, y) = banco.centro_de_boton(PaletteButton::Transparent);
        banco.mueve(x, y);
        let pos = en_puntos(banco.ppp(), x, y);
        let f = banco.fotograma(vec![
            boton(pos, true, Modifiers::NONE),
            boton(pos, false, Modifiers::NONE),
        ]);
        assert_eq!(
            f.salida.commit,
            Some(Commit {
                target: EditTarget::Entry(1),
                color: Rgba::opaque(255, 0, 255)
            })
        );
        assert_eq!(f.salida.op, op_de(Some(1)));
        assert!(!banco.editor.has_pending(), "no queda texto");
    }

    /// Con una quita pendiente el icono no se ve ni golpea (specs/interfaz-iconos.md, decisión 8):
    /// donde estaba cae `Cancelar`, y la quita se cancela además al cambiar el documento.
    #[test]
    fn fila_9_con_una_quita_pendiente_el_icono_no_se_ve_ni_golpea_y_la_quita_se_cancela_al_cambiar_el_documento()
     {
        for ppp in PPPS {
            let mut banco = banco_p5(ppp, 3);
            let id = banco.documento.main_image();
            // Rango de rotación 3..=4 con Mayús, y la quita pendiente de la 3.
            banco.clic_muestra_con(4, MAYUS);
            assert_eq!(banco.ops.range_end(), Some(4), "ppp {ppp}");
            banco.clic_boton(PaletteButton::Remove);
            assert!(banco.ops.removal().is_some(), "ppp {ppp}");
            let f = banco.reposo();
            comprueba_botones(&f, &banco, &CON_QUITA, &format!("ppp {ppp}"));
            assert!(
                !f.pinta_icono(&banco.iconos.set, Icon::Transparent),
                "ppp {ppp}: el icono no se ve con una quita pendiente"
            );
            banco.historial.clear();

            // Donde estaba el icono ahora está `Cancelar`: cancela la quita y no pide ninguna op.
            let f = banco.clic_boton(PaletteButton::Transparent);
            assert_eq!(f.salida.op, None, "ppp {ppp}: no golpea");
            assert_eq!(f.salida.notice, None, "ppp {ppp}");
            assert!(banco.ops.removal().is_none(), "ppp {ppp}: `Cancelar` cancela");
            assert_eq!(banco.choice, ColorChoice::Entry(3), "ppp {ppp}");
            assert_eq!(banco.ops.range_end(), Some(4), "ppp {ppp}: rango intacto");

            // De nuevo con la quita pendiente: si la sesión cambia el documento (aquí con la op
            // que antes pedía el icono), en el siguiente fotograma la quita se cancela con aviso.
            banco.clic_boton(PaletteButton::Remove);
            assert!(banco.ops.removal().is_some(), "ppp {ppp}");
            banco.aplica(Command::SetTransparentIndex {
                image: id,
                index: Some(3),
            });
            let f = banco.reposo();
            assert_eq!(banco.cancelaciones, 1, "ppp {ppp}");
            assert!(banco.ops.removal().is_none(), "ppp {ppp}");
            assert_eq!(banco.ops.range_end(), Some(4), "ppp {ppp}");
            assert_eq!(banco.choice, ColorChoice::Entry(3), "ppp {ppp}");
            comprueba_botones(&f, &banco, &NORMALES, &format!("ppp {ppp}"));
            comprueba_boton(&f, &banco, QUITAR, &format!("ppp {ppp}"));
        }
    }

    #[test]
    fn fila_9_en_rgba_gray_y_grayalpha_no_hay_boton_ni_op() {
        for ppp in PPPS {
            for modo in [Mode::Rgba, Mode::Gray, Mode::GrayAlpha] {
                let image = Image::new(64, 48, modo).expect("imagen");
                let choice = initial_choice(&image);
                let mut banco = Banco::nuevo(ppp, PANTALLA_ALTA, image, choice);
                let contexto = format!("ppp {ppp}, {modo:?}");
                let f = banco.reposo();
                assert!(!f.pinta_algo_con(FIJAR), "{contexto}");
                assert!(!f.pinta_algo_con(QUITAR), "{contexto}");
                let n = banco.n();
                let top = banco.geo().top_px;
                let mut y = top + 4 * n;
                while y < banco.panel_h_px() {
                    for x_u in [10, 60, 120, 200] {
                        let f = banco.clic(x_u * n, y);
                        assert_eq!(f.salida.op, None, "{contexto}: ({x_u}·n, {y})");
                        assert_eq!(f.salida.notice, None, "{contexto}");
                    }
                    y += 8 * n;
                }
                // Donde estaría el botón de indexadas, medido desde el borde superior del editor.
                let (x, y) = (10 * n, top + 248 * n);
                if y < banco.panel_h_px() {
                    let f = banco.clic(x, y);
                    assert_eq!(f.salida.op, None, "{contexto}");
                }
                assert!(banco.operaciones().is_empty(), "{contexto}");
                assert!(banco.avisos().is_empty(), "{contexto}");
            }
        }
    }

    #[test]
    fn fila_2_show_editing_view_sin_ops_tambien_pinta_el_boton() {
        // La spec no dice si el botón emite `op` sin estado de operaciones (como `Añadir`, no lo
        // necesita); aquí solo se fija que se pinta.
        for ppp in PPPS {
            let mut banco = banco_f(ppp, 2);
            banco.sin_ops = true;
            let f = banco.reposo();
            comprueba_boton(&f, &banco, FIJAR, &format!("ppp {ppp}"));
        }
    }
}

// Unidad 21, ampliación: clic real con el rótulo del estado y avisos en la barra de estado.

mod fijar_transparente_clic_y_barra {
    use super::*;
    use px_ui::palette_ops::{summary, transparent_summary, transparent_toggle};
    use px_ui::status::{bar_text, columns};
    use px_ui::text::fit_text;
    use px_ui::theme::NOTICE_PREFIX;

    /// Con un clic real (mover, pulsar y soltar en la ventana del banco), la op que sale es
    /// `SetTransparent { index: transparent_toggle(palette, activa) }`, en los dos estados.
    #[test]
    fn el_clic_real_emite_la_op_del_conmutador_con_la_activa_sin_marca_y_con_la_activa_marcada() {
        for ppp in PPPS {
            for marca in [None, Some(0u8), Some(1), Some(3)] {
                for activa in 0..4u8 {
                    let imagen = imagen_con(&[ROJO, VERDE, AZUL, NEGRO], marca, &[0, 1, 2, 3]);
                    let esperado = transparent_toggle(imagen.palette().unwrap(), activa);
                    assert_eq!(
                        esperado,
                        if marca == Some(activa) {
                            None
                        } else {
                            Some(activa)
                        }
                    );
                    let mut banco =
                        Banco::nuevo(ppp, PANTALLA_ALTA, imagen, ColorChoice::Entry(activa));
                    let contexto = format!("ppp {ppp}, marca {marca:?}, activa {activa}");
                    let f = banco.clic_boton(PaletteButton::Transparent);
                    assert_eq!(
                        f.salida.op,
                        Some(PaletteOp::SetTransparent { index: esperado }),
                        "{contexto}"
                    );
                    // Tras pulsar y soltar, la op salió una sola vez y no hay nada más.
                    banco.reposo();
                    assert_eq!(banco.operaciones().len(), 1, "{contexto}");
                    assert!(banco.avisos().is_empty(), "{contexto}");
                    assert!(banco.elecciones().is_empty(), "{contexto}");
                }
            }
        }
    }

    /// Los avisos caben en la barra de estado a las cinco escalas: `bar_text` con las columnas
    /// de 361 u − 6 u, `MissingGlyphPolicy::Panic` y sin recortar.
    #[test]
    fn los_avisos_de_transparente_caben_en_la_barra_a_los_cinco_ppp_sin_recorte() {
        for ppp in PPPS {
            let banco = banco_f(ppp, 1);
            let n = text_scale(ppp);
            let columnas = columns((361 - 6) as f32 * n as f32, ppp);
            assert_eq!(columnas, 50, "ppp {ppp}");
            let mut avisos: Vec<String> = vec![
                transparent_summary(Some(255), 255, Some(254)),
                transparent_summary(Some(255), 255, None),
                transparent_summary(None, 255, None),
                transparent_summary(Some(3), 3, Some(1)),
                transparent_summary(Some(0), 0, Some(255)),
            ];
            for (index, activa) in [(Some(255u8), 255u8), (None, 255), (Some(2), 2), (None, 0)] {
                avisos.push(summary(
                    &PaletteOp::SetTransparent { index },
                    activa,
                    256,
                    false,
                    false,
                ));
            }
            for i in [0u8, 9, 10, 99, 100, 255] {
                for p in [0u8, 9, 10, 99, 100, 254] {
                    if p != i {
                        avisos.push(transparent_summary(Some(i), i, Some(p)));
                    }
                }
            }
            for texto in &avisos {
                assert!(texto.chars().count() <= 43, "{texto:?}");
                let esperado = format!("{NOTICE_PREFIX}{texto}");
                assert_eq!(
                    bar_text(&banco.ctx, texto, true, columnas),
                    esperado,
                    "ppp {ppp}: se recortó o se sustituyó {texto:?}"
                );
                assert_eq!(
                    fit_text(&esperado, columnas, |_| true),
                    esperado,
                    "ppp {ppp}: fit_text recorta {texto:?}"
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Unidad I.4 (specs/interfaz-iconos.md): los iconos del editor, sus estados, tooltips y el icono
// de fijar o quitar la transparente con dos estados
// ---------------------------------------------------------------------------------------------

mod iconos_editor {
    use super::*;
    use px_ui::icon_button::{Look, look};
    use px_ui::palette_ops::palette_button_tooltip;
    use px_ui::text::INTERFACE_CHARSET;

    const PPPS_FINOS: [f32; 9] = [1.0, 1.25, 1.5, 1.75, 2.0, 2.25, 2.5, 2.75, 3.0];

    fn icono_de(button: PaletteButton) -> Icon {
        ICONOS_DE_BOTON
            .iter()
            .find(|(b, _)| *b == button)
            .map(|(_, i)| *i)
            .expect("botón de icono")
    }

    /// El botón se pinta con la cara, el relieve y el icono del aspecto `l` (spec, decisión 3): la
    /// cara llena el rectángulo, las franjas de 1 u del relieve van arriba y a la izquierda
    /// (`top_left`) y abajo y a la derecha (`bottom_right`), y el icono de `16n` píxeles va a 4 u
    /// del borde más el desplazamiento, con las esquinas en píxeles enteros y el UV completo.
    fn comprueba(f: &Fotograma, banco: &Banco, button: PaletteButton, l: &Look, contexto: &str) {
        let (ppp, n) = (banco.ppp(), banco.n());
        let u = n as f32 / ppp;
        let rect = banco.rect_de_boton(button);
        assert!(hay_rect(f, rect, l.face), "{contexto}: falta la cara {:?}", l.face);
        let franjas = [
            (Rect::from_min_size(rect.min, vec2(rect.width(), u)), l.top_left),
            (Rect::from_min_size(rect.min, vec2(u, rect.height())), l.top_left),
            (
                Rect::from_min_size(pos2(rect.min.x, rect.max.y - u), vec2(rect.width(), u)),
                l.bottom_right,
            ),
            (
                Rect::from_min_size(pos2(rect.max.x - u, rect.min.y), vec2(u, rect.height())),
                l.bottom_right,
            ),
        ];
        for (franja, color) in franjas {
            assert!(
                hay_rect(f, franja, color),
                "{contexto}: falta la franja del relieve {franja:?} de {color:?}"
            );
        }
        let malla = f
            .malla_de(&banco.iconos.set, icono_de(button))
            .unwrap_or_else(|| panic!("{contexto}: no se pinta el icono de {button:?}"));
        let desplazamiento = (4 + l.offset_u) as f32 * u;
        let esperado = Rect::from_min_size(
            rect.min + vec2(desplazamiento, desplazamiento),
            vec2(16.0 * u, 16.0 * u),
        );
        assert!(
            cerca_rect(malla.rect, esperado),
            "{contexto}: el icono está en {:?} y debería estar en {esperado:?}",
            malla.rect
        );
        for esquina in [malla.rect.min, malla.rect.max] {
            for v in [esquina.x * ppp, esquina.y * ppp] {
                assert!((v - v.round()).abs() <= 0.05, "{contexto}: esquina fuera de píxel: {v}");
            }
        }
        assert!(
            ((malla.rect.width() * ppp) - (16 * n) as f32).abs() <= 0.05,
            "{contexto}: el icono mide {} px",
            malla.rect.width() * ppp
        );
        assert_eq!(malla.color, l.tint, "{contexto}: tinta");
        assert!(
            malla.uv.min.distance(pos2(0.0, 0.0)) < 1e-4
                && malla.uv.max.distance(pos2(1.0, 1.0)) < 1e-4,
            "{contexto}: UV incompleto: {:?}",
            malla.uv
        );
    }

    #[test]
    fn fila_6_los_iconos_se_pintan_con_cara_relieve_e_icono_de_cada_estado_a_ppp_de_1_a_3() {
        for ppp in PPPS_FINOS {
            let contexto = format!("ppp {ppp}");
            // Normales: los cuatro de la fila con una paleta de 4 entradas (todos habilitados).
            let mut banco = banco_f(ppp, 1);
            let f = banco.reposo();
            for button in [
                PaletteButton::Add,
                PaletteButton::Remove,
                PaletteButton::RotateLeft,
                PaletteButton::RotateRight,
                PaletteButton::Transparent,
            ] {
                comprueba(&f, &banco, button, &look(false, true, false), &format!("{contexto} {button:?}"));
            }
            // Con 256 entradas, `Añadir` está desactivado; con una sola, `Quitar`.
            let mut banco = Banco::nuevo(ppp, PANTALLA_ALTA, imagen_indexada_256(), ColorChoice::Entry(5));
            let f = banco.reposo();
            comprueba(&f, &banco, PaletteButton::Add, &look(false, false, false), &contexto);
            comprueba(&f, &banco, PaletteButton::Remove, &look(false, true, false), &contexto);
            let mut banco = Banco::nuevo(ppp, PANTALLA_ALTA, imagen_una_entrada(), ColorChoice::Entry(0));
            let f = banco.reposo();
            comprueba(&f, &banco, PaletteButton::Remove, &look(false, false, false), &contexto);
            comprueba(&f, &banco, PaletteButton::RotateRight, &look(false, true, false), &contexto);
            // Hundido: la entrada elegida es la transparente.
            let mut banco = Banco::nuevo(
                ppp,
                PANTALLA_ALTA,
                imagen_con(&[ROJO, VERDE, AZUL, NEGRO], Some(2), &[0, 1, 2, 3]),
                ColorChoice::Entry(2),
            );
            let f = banco.reposo();
            comprueba(&f, &banco, PaletteButton::Transparent, &look(true, true, false), &contexto);
        }
    }

    #[test]
    fn fila_6_con_el_raton_encima_la_cara_se_aclara_y_deshabilitado_no_responde_al_raton() {
        for ppp in PPPS {
            let mut banco = banco_f(ppp, 1);
            let (x, y) = banco.centro_de_boton(PaletteButton::RotateLeft);
            banco.mueve(x, y);
            let f = banco.reposo();
            comprueba(&f, &banco, PaletteButton::RotateLeft, &look(false, true, true), &format!("ppp {ppp}"));
            comprueba(&f, &banco, PaletteButton::Add, &look(false, true, false), &format!("ppp {ppp}"));
            // Un botón desactivado con el ratón encima sigue con la cara normal.
            let mut banco = Banco::nuevo(ppp, PANTALLA_ALTA, imagen_indexada_256(), ColorChoice::Entry(5));
            let (x, y) = banco.centro_de_boton(PaletteButton::Add);
            banco.mueve(x, y);
            let f = banco.reposo();
            comprueba(&f, &banco, PaletteButton::Add, &look(false, false, true), &format!("ppp {ppp}"));
            // Con un gesto en curso todo se ve normal (los controles no responden).
            assert!(f.pinta_icono(&banco.iconos.set, Icon::Add));
        }
    }

    #[test]
    fn fila_8_los_tooltips_del_editor_son_los_de_la_spec_y_caben_en_el_juego_de_caracteres() {
        for (boton, activo, texto) in [
            (PaletteButton::Add, false, "Añadir entrada"),
            (PaletteButton::Remove, false, "Quitar entrada"),
            (PaletteButton::RotateLeft, false, "Rotar a la izquierda"),
            (PaletteButton::RotateRight, false, "Rotar a la derecha"),
            (PaletteButton::Transparent, false, "Fijar transparente"),
            (PaletteButton::Transparent, true, "Quitar transparente"),
            (PaletteButton::Confirm, false, "Confirmar"),
            (PaletteButton::Cancel, false, "Cancelar"),
        ] {
            let real = palette_button_tooltip(boton, activo);
            assert_eq!(real, texto, "{boton:?} {activo}");
            assert!(real.chars().all(|c| INTERFACE_CHARSET.contains(c)), "{real}");
            assert!(!real.contains('\n'), "sin segunda línea ni atajo");
        }
    }

    #[test]
    fn fila_8_el_hover_enseña_el_tooltip_de_cada_icono_con_la_fuente_de_pixeles_y_sin_hover_no() {
        for ppp in [1.0, 1.5, 2.0] {
            for (button, esperado) in [
                (PaletteButton::Add, "Añadir entrada"),
                (PaletteButton::Remove, "Quitar entrada"),
                (PaletteButton::RotateLeft, "Rotar a la izquierda"),
                (PaletteButton::RotateRight, "Rotar a la derecha"),
                (PaletteButton::Transparent, "Fijar transparente"),
            ] {
                let mut banco = banco_f(ppp, 1);
                banco.tiempo.set(Some(1.0));
                let sin = banco.reposo();
                assert!(!sin.pinta(esperado), "ppp {ppp}: {esperado} sin hover");
                let (x, y) = banco.centro_de_boton(button);
                banco.mueve(x, y);
                let mut f = banco.reposo();
                for _ in 0..8 {
                    f = banco.reposo();
                }
                let texto = f
                    .texto_pintado(esperado)
                    .unwrap_or_else(|| panic!("ppp {ppp}: falta el tooltip {esperado:?}"));
                assert!(texto.color == TEXT, "ppp {ppp}: {esperado} en TEXT");
                banco.mueve(3000, 3000);
                let lejos = banco.reposo();
                assert!(!lejos.pinta(esperado), "ppp {ppp}: {esperado} sin el ratón");
            }
        }
    }

    #[test]
    fn fila_8_pulsar_un_icono_habilitado_emite_widget_info_de_tipo_boton_con_su_rotulo() {
        use egui::output::OutputEvent;
        use egui::WidgetType;
        for (button, esperado) in [
            (PaletteButton::Add, "Añadir entrada"),
            (PaletteButton::RotateRight, "Rotar a la derecha"),
            (PaletteButton::Transparent, "Fijar transparente"),
        ] {
            let mut banco = banco_f(1.0, 1);
            let (x, y) = banco.centro_de_boton(button);
            let pos = en_puntos(1.0, x, y);
            banco.mueve(x, y);
            let actual = banco.actual();
            banco.editor.sync(actual);
            let mut eventos = Vec::new();
            for events in [vec![boton(pos, true, Modifiers::NONE)], vec![boton(pos, false, Modifiers::NONE)]] {
                let input = banco.entrada(events);
                let Banco { ctx, canvas, scroll, view, image, choice, slots, editor, ops, .. } = &mut banco;
                let salida = ctx.run_ui(input, |ui| {
                    apply_scale(ui.ctx());
                    palette_panel::show_editing_ops(
                        ui, image, *choice, scroll, view, true, slots, editor, ops,
                    );
                    let _ = &canvas;
                });
                eventos.extend(salida.platform_output.events.clone());
                salida.drop_without_applying_deltas();
            }
            assert!(
                eventos.iter().any(|e| matches!(e, OutputEvent::Clicked(info)
                    if info.typ == WidgetType::Button
                        && info.label.as_deref() == Some(esperado)
                        && info.enabled)),
                "{button:?}: no hay WidgetInfo de botón con el rótulo {esperado:?}; hay {eventos:?}"
            );
        }
    }

    #[test]
    fn fila_9_los_cinco_iconos_dan_lo_mismo_que_antes_y_los_desactivados_avisan() {
        use PaletteOp::*;
        for ppp in PPPS {
            let mut banco = banco_f(ppp, 1);
            let f = banco.clic_boton(PaletteButton::Add);
            assert_eq!(f.salida.op, Some(Add), "ppp {ppp}");
            let f = banco.clic_boton(PaletteButton::RotateLeft);
            assert_eq!(f.salida.op, Some(Rotate { start: 0, len: 4, steps: -1 }), "ppp {ppp}");
            let f = banco.clic_boton(PaletteButton::RotateRight);
            assert_eq!(f.salida.op, Some(Rotate { start: 0, len: 4, steps: 1 }), "ppp {ppp}");
            let f = banco.clic_boton(PaletteButton::Transparent);
            assert_eq!(f.salida.op, Some(SetTransparent { index: Some(1) }), "ppp {ppp}");
            // `Quitar` una entrada usada deja la quita pendiente: aparecen `Confirmar` y `Cancelar`.
            banco.clic_boton(PaletteButton::Remove);
            assert!(banco.ops.removal().is_some(), "ppp {ppp}");
            let f = banco.reposo();
            comprueba_botones(&f, &banco, &CON_QUITA, &format!("ppp {ppp}"));
            // Desactivados: 256 entradas y una sola entrada avisan, sin op.
            let mut banco = Banco::nuevo(ppp, PANTALLA_ALTA, imagen_indexada_256(), ColorChoice::Entry(5));
            let f = banco.clic_boton(PaletteButton::Add);
            assert_eq!((f.salida.op, f.salida.notice.as_deref()), (None, Some("paleta llena: 256 entradas")));
            let mut banco = Banco::nuevo(ppp, PANTALLA_ALTA, imagen_una_entrada(), ColorChoice::Entry(0));
            let f = banco.clic_boton(PaletteButton::Remove);
            assert_eq!(f.salida.op, None);
            assert_eq!(f.salida.notice.as_deref(), Some("no se puede quitar la única entrada"));
            // Con un gesto en curso no responde ninguno y nada cambia.
            let mut banco = banco_f(ppp, 1);
            banco.gesto_dibujo = true;
            for (button, _) in ICONOS_DE_BOTON {
                banco.clic_boton(button);
            }
            assert!(banco.historial_vacio(), "ppp {ppp}");
        }
    }

    #[test]
    fn fila_10_el_icono_de_transparente_tiene_dos_estados_en_el_mismo_rectangulo_y_tooltip_alternante() {
        for ppp in PPPS {
            let imagen = |marca| imagen_con(&[ROJO, VERDE, AZUL, NEGRO], marca, &[0, 1, 2, 3]);
            let mut rectangulos = Vec::new();
            for (marca, activa, hundido) in [
                (Some(3u8), 3u8, true),
                (Some(3), 1, false),
                (None, 1, false),
                (Some(1), 1, true),
            ] {
                let mut banco = Banco::nuevo(ppp, PANTALLA_ALTA, imagen(marca), ColorChoice::Entry(activa));
                let f = banco.reposo();
                let contexto = format!("ppp {ppp}, marca {marca:?}, activa {activa}");
                comprueba(&f, &banco, PaletteButton::Transparent, &look(hundido, true, false), &contexto);
                let palette = banco.image.palette().expect("indexada");
                let texto = px_ui::palette_ops::transparent_button_label(palette, activa);
                assert_eq!(
                    palette_button_tooltip(PaletteButton::Transparent, hundido),
                    texto,
                    "{contexto}: el tooltip sale del estado"
                );
                assert_eq!(texto == "Quitar transparente", hundido, "{contexto}");
                rectangulos.push(banco.rect_de_boton(PaletteButton::Transparent));
            }
            assert!(rectangulos.windows(2).all(|p| p[0] == p[1]), "ppp {ppp}: mismo rectángulo");
            // Pulsar aplica `SetTransparentIndex` (un paso) y el icono cambia en el siguiente
            // fotograma, con la op aplicada.
            let mut banco = banco_f(ppp, 2);
            let id = banco.documento.main_image();
            let f = banco.clic_boton(PaletteButton::Transparent);
            assert_eq!(f.salida.op, Some(PaletteOp::SetTransparent { index: Some(2) }), "ppp {ppp}");
            banco.aplica(Command::SetTransparentIndex { image: id, index: Some(2) });
            let f = banco.reposo();
            comprueba(&f, &banco, PaletteButton::Transparent, &look(true, true, false), &format!("ppp {ppp}: tras fijar"));
        }
    }
}
