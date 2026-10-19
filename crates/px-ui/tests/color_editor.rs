//! Tests del editor de color dentro de la ventana (specs/hito-2-editar-color.md: filas 2, 9, 11,
//! 16 y 17 de «Casos límite» en su parte de ventana (la 18, ventana baja, la sustituyó
//! specs/hito-2-panel-ventana-baja.md: ver `panel_baja.rs`), y el arrastre de los deslizadores con el
//! ratón). Lo puro (`ColorEditor`, `edit_layout`, `gate_keys` sueltos) se prueba en
//! `color_edit.rs`, `edit_layout.rs` y `shortcuts_gate.rs`; aquí se prueba que `show_editing`
//! lo une bien a `egui`.
//!
//! El banco es el de `tests/palette_panel.rs`, copiado y ampliado (ese fichero no se toca): un
//! `egui::Context` ejecuta fotogramas con `RawInput` sintético, sin GPU ni pantalla. En cada
//! fotograma, como hará la app, se llama a `ColorEditor::sync` con el destino y el color guardado
//! (lo que hará `Session::sync_editor`), se monta `status::show`, luego
//! `palette_panel::show_editing` y por fin el lienzo (`Canvas::interact_ordered` y
//! `Canvas::paint`) en el mismo `Ui` raíz. `enabled` es `!editor.dragging()`, como `busy` en la
//! app.
//!
//! El banco guarda la imagen, la elección y las ranuras, y **nunca aplica** lo que devuelve el
//! panel: no hay `Document` (la confirmación es de `Session`, en `px-app`). Cada test cambia
//! `banco.choice` o `banco.image` cuando quiere simular lo que haría la app. Cada `PanelOutput`
//! se guarda en `banco.historial`.
//!
//! Lo pintado se lee de las formas del fotograma (`Shape::Text` y `Shape::Rect`): así se ve qué
//! textos hay en el editor y dónde caen. No hay capturas de pantalla.
//!
//! Los cambios de fuentes, de `ppp` y de la política de glifo ausente se aplican al empezar el
//! pase siguiente: el banco hace dos pases vacíos de calentamiento antes del primer fotograma.
//!
//! # Lo que depende del comportamiento real de `egui`
//!
//! La spec lo deja «a comprobar en el banco». Los tests que lo necesitan llevan en su nombre o su
//! comentario la marca **[DEPENDE DE EGUI]**: si fallan, el fallo es información sobre `egui`, no
//! necesariamente un error de `show_editing`:
//!
//! - un clic fuera del campo (rejilla o lienzo) le quita el foco (con el `egui` por defecto,
//!   `SurrenderFocusOn::Clicks`: el foco se pierde al **soltar**, no al pulsar);
//! - `Tab` saca el foco del campo (y no se lo da al lienzo, que es `Sense::click_and_drag` y por
//!   tanto enfocable: si `egui` se lo da, `egui_wants_keyboard_input()` seguiría siendo cierto, por
//!   eso la guarda usa `text_edit_focused()`, que solo vale con un campo de texto);
//! - `Intro` con el campo enfocado le quita el foco (y con un valor inválido `show_editing` se lo
//!   devuelve);
//! - la nitidez del texto del `TextEdit` a 1,25× y 1,5×;
//! - que `Ctrl+A` selecciona el texto del campo (el banco lo usa para sustituir lo que haya).

use egui::epaint::{ClippedShape, Shape};
use egui::{
    Color32, Context, Event, Key, MissingGlyphPolicy, Modifiers, PointerButton, Pos2, RawInput,
    Rect, Vec2, pos2, vec2,
};
use px_core::{Image, Mode, Palette, Pixel, Rgba};
use px_ui::canvas::{Canvas, CanvasInfo, CanvasInput};
use px_ui::color::{ColorChoice, SlotColors};
use px_ui::color_edit::{
    ColorEditor, Commit, Control, EditTarget, HexOutcome, control_value, controls, hex_notice,
    hex_text, slider_value_at, slider_x,
};
use px_ui::edit_layout::{
    FIELD_H_U, FIELD_W_U, FIELD_X_U, HEADER_X_U, HEADER_Y_U, HIT_X0_U, HIT_X1_U, ROW_H_U,
    TRACK_W_U, TRACK_X_U, control_row_u, field_y_u, panel_layout, slider_hit,
};
use px_ui::palette_grid::{
    self, CELL_U, COLUMNS, GRID_X_U, GRID_Y_U, PANEL_WIDTH_U, PITCH_U, TEXT_WIDTH_U,
};
use px_ui::palette_panel::{self, PanelOutput};
use px_ui::shortcuts::gate_keys;
use px_ui::status::{bar_height_px, columns};
use px_ui::text::{INTERFACE_CHARSET, apply_scale, install_fonts, text_scale};
use px_ui::theme::SURFACE_0;
use px_ui::view::Zoom;

// ---------------------------------------------------------------------------------------------
// Imágenes de prueba
// ---------------------------------------------------------------------------------------------

const ROJO: Rgba = Rgba::opaque(255, 0, 0);
const VERDE: Rgba = Rgba::opaque(0, 255, 0);
const AZUL: Rgba = Rgba::opaque(0, 0, 255);
const NEGRO: Rgba = Rgba::opaque(0, 0, 0);
const AMARILLO: Rgba = Rgba::opaque(255, 255, 0);

const MODOS: [Mode; 4] = [Mode::Rgba, Mode::Indexed, Mode::Gray, Mode::GrayAlpha];

/// Indexada de 64×48 (zoom de ajuste 8 con el banco a ppp 1) con la paleta `[R, G, B, K]` de `F`.
/// Es `F` de la spec con un lienzo más grande.
fn imagen_f() -> Image {
    let paleta = Palette::from_colors([ROJO, VERDE, AZUL, NEGRO]).expect("paleta");
    Image::new_indexed(64, 48, paleta).expect("imagen indexada")
}

/// Indexada de 64×48 con 256 colores, el último `#ffffff80` y el índice transparente en el 17.
fn imagen_indexada_256() -> Image {
    let colores: Vec<Rgba> = (0..256usize)
        .map(|i| {
            if i == 255 {
                Rgba::new(255, 255, 255, 128)
            } else {
                Rgba::opaque(i as u8, 255 - i as u8, (i * 7) as u8)
            }
        })
        .collect();
    let mut paleta = Palette::from_colors(colores).expect("paleta de 256 colores");
    paleta
        .set_transparent_index(Some(17))
        .expect("el índice existe");
    Image::new_indexed(64, 48, paleta).expect("imagen indexada")
}

/// Imagen de 64×48 de ese modo (la indexada es la de 256 colores).
fn imagen_de(mode: Mode) -> Image {
    match mode {
        Mode::Indexed => imagen_indexada_256(),
        other => Image::new(64, 48, other).expect("imagen válida"),
    }
}

/// Tres elecciones con destino válido para cada modo: la más larga de la cabecera, la
/// transparente (solo indexadas) y una corriente.
fn elecciones_de(mode: Mode) -> Vec<ColorChoice> {
    match mode {
        Mode::Indexed => vec![
            ColorChoice::Entry(255),
            ColorChoice::Entry(17),
            ColorChoice::Entry(0),
        ],
        _ => vec![
            ColorChoice::Slot(9),
            ColorChoice::Slot(4),
            ColorChoice::Slot(1),
        ],
    }
}

// ---------------------------------------------------------------------------------------------
// Eventos sintéticos
// ---------------------------------------------------------------------------------------------

fn tecla_con(key: Key, modifiers: Modifiers) -> Event {
    Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers,
    }
}

fn tecla(key: Key) -> Event {
    tecla_con(key, Modifiers::NONE)
}

/// Ctrl en Linux y Windows (`command` es Ctrl ahí).
fn ctrl() -> Modifiers {
    Modifiers {
        ctrl: true,
        command: true,
        ..Modifiers::NONE
    }
}

fn boton(pos: Pos2, pressed: bool) -> Event {
    Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    }
}

fn texto(s: &str) -> Event {
    Event::Text(s.to_owned())
}

/// Las pulsaciones de teclear `cadena` como las da una ventana: `Key` y `Text` por cada carácter.
fn pulsaciones(cadena: &str) -> Vec<Event> {
    let mut eventos = Vec::new();
    for c in cadena.chars() {
        let key = match c {
            'f' => Key::F,
            'a' => Key::A,
            'b' => Key::B,
            '0' => Key::Num0,
            otra => panic!("sin tecla para {otra:?} en el test"),
        };
        eventos.push(tecla(key));
        eventos.push(texto(&c.to_string()));
    }
    eventos
}

/// Punto en píxeles físicos de la ventana a puntos.
fn en_puntos(ppp: f32, x_px: i64, y_px: i64) -> Pos2 {
    pos2(x_px as f32 / ppp, y_px as f32 / ppp)
}

// ---------------------------------------------------------------------------------------------
// Lo pintado
// ---------------------------------------------------------------------------------------------

/// Un texto pintado: contenido, esquina de su `galley` en puntos y ancho en píxeles físicos.
#[derive(Clone, Debug)]
struct Pintado {
    texto: String,
    pos: Pos2,
    ancho_px: f32,
}

fn recoge(shape: &Shape, ppp: f32, textos: &mut Vec<Pintado>, rects: &mut Vec<(Rect, Color32)>) {
    match shape {
        Shape::Text(t) => textos.push(Pintado {
            texto: t.galley.text().to_owned(),
            pos: t.pos,
            ancho_px: t.galley.size().x * ppp,
        }),
        Shape::Rect(r) => rects.push((r.rect, r.fill)),
        Shape::Vec(formas) => {
            for forma in formas {
                recoge(forma, ppp, textos, rects);
            }
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------------------------
// Banco
// ---------------------------------------------------------------------------------------------

const PANTALLA_ALTA: Vec2 = vec2(800.0, 600.0);
const PPP_SIEMPRE: [f32; 5] = [1.0, 1.25, 1.5, 2.0, 3.0];

struct Banco {
    ctx: Context,
    canvas: Canvas,
    scroll: palette_grid::Scroll,
    pantalla: Vec2,
    image: Image,
    choice: ColorChoice,
    slots: SlotColors,
    editor: ColorEditor,
    /// Lo que devolvió `show_editing` en cada fotograma, en orden.
    historial: Vec<PanelOutput>,
}

struct Fotograma {
    /// Lo que quedaba libre justo antes del lienzo: pantalla menos la barra y el panel.
    rect_lienzo: Rect,
    entradas: Vec<CanvasInput>,
    info: CanvasInfo,
    salida: PanelOutput,
    textos: Vec<Pintado>,
    rects: Vec<(Rect, Color32)>,
    /// `text_edit_focused()` justo antes y justo después del fotograma.
    quiere_antes: bool,
    quiere_despues: bool,
}

impl Fotograma {
    /// Algún texto pintado es exactamente `buscado`.
    fn pinta(&self, buscado: &str) -> bool {
        self.textos.iter().any(|t| t.texto == buscado)
    }

    /// Algún texto pintado contiene `buscado`.
    fn pinta_algo_con(&self, buscado: &str) -> bool {
        self.textos.iter().any(|t| t.texto.contains(buscado))
    }

    /// El texto pintado exactamente igual a `buscado`, si lo hay.
    fn texto_pintado(&self, buscado: &str) -> Option<&Pintado> {
        self.textos.iter().find(|t| t.texto == buscado)
    }

    fn hay_relleno(&self, color: Color32) -> bool {
        self.rects.iter().any(|(_, fill)| *fill == color)
    }
}

/// Medidas del editor a la escala del banco, en píxeles físicos.
#[derive(Clone, Copy, Debug)]
struct Geo {
    ppp: f32,
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

    /// `x` del inicio de la pista y su largo.
    fn x_pista(&self) -> i64 {
        TRACK_X_U * self.n
    }

    fn ancho_pista(&self) -> i64 {
        TRACK_W_U * self.n
    }

    /// `x` (absoluta) de la posición de la pista que da `valor` de un máximo `max`.
    fn x_de_valor(&self, valor: u32, max: u32) -> i64 {
        self.x_pista() + slider_x(valor, self.ancho_pista(), max)
    }

    /// Valor que da un punto a `dx` píxeles del inicio de la pista.
    fn valor_en(&self, dx: i64, control: Control) -> u32 {
        slider_value_at(dx, self.ancho_pista(), control.max())
    }

    /// Centro del campo hexadecimal.
    fn campo(&self) -> (i64, i64) {
        (
            FIELD_X_U * self.n + FIELD_W_U * self.n / 2,
            self.top_px + field_y_u(self.mode) * self.n + FIELD_H_U * self.n / 2,
        )
    }
}

/// Centro de la muestra `indice` en píxeles físicos, con la primera fila visible `primera_fila`.
fn centro_de_muestra(n: i64, indice: usize, primera_fila: usize) -> (i64, i64) {
    let columna = (indice % COLUMNS) as i64;
    let fila = (indice / COLUMNS) as i64 - primera_fila as i64;
    (
        (GRID_X_U + columna * PITCH_U) * n + CELL_U * n / 2,
        (GRID_Y_U + fila * PITCH_U) * n + CELL_U * n / 2,
    )
}

impl Banco {
    /// Contexto de `pantalla` puntos a `ppp`, con la fuente instalada, la política `Panic` para
    /// el glifo ausente, `image` en el lienzo y dos fotogramas de reposo.
    fn nuevo(ppp: f32, pantalla: Vec2, image: Image, choice: ColorChoice) -> Banco {
        let ctx = Context::default();
        ctx.set_pixels_per_point(ppp);
        ctx.set_missing_glyph_policy(MissingGlyphPolicy::Panic);
        install_fonts(&ctx);
        let mut banco = Banco {
            ctx,
            canvas: Canvas::new(),
            scroll: palette_grid::Scroll::default(),
            pantalla,
            slots: SlotColors::defaults(image.mode()),
            image,
            choice,
            editor: ColorEditor::default(),
            historial: Vec::new(),
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

    /// Banco con un panel de exactamente `panel_h_px` píxeles físicos de alto (la ventana menos
    /// la barra de estado).
    fn con_alto_de_panel(ppp: f32, panel_h_px: i64, image: Image, choice: ColorChoice) -> Banco {
        Banco::nuevo(ppp, pantalla_para_panel(ppp, panel_h_px), image, choice)
    }

    fn entrada(&self, events: Vec<Event>) -> RawInput {
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, self.pantalla)),
            events,
            ..RawInput::default()
        }
    }

    fn ppp(&self) -> f32 {
        self.ctx.pixels_per_point()
    }

    fn n(&self) -> i64 {
        i64::from(text_scale(self.ppp()))
    }

    /// Alto del panel en píxeles físicos, redondeado como lo hace el panel.
    fn panel_h_px(&self) -> i64 {
        (self.pantalla.y * self.ppp()).round() as i64 - i64::from(bar_height_px(self.n() as u32))
    }

    /// Medidas del editor con el destino actual; entra en pánico si no cabe.
    fn geo(&self) -> Geo {
        let mode = self.image.mode();
        let n = self.n();
        let layout = panel_layout(mode, self.panel_h_px(), n as u32, 0);
        assert!(!layout.scrolling, "el bloque cabe sin desplazar");
        Geo {
            ppp: self.ppp(),
            n,
            mode,
            top_px: layout.top_px,
        }
    }

    /// El destino y su color guardado, como los pasa la app a `ColorEditor::sync`.
    fn actual(&self) -> Option<(EditTarget, Rgba)> {
        let destino = EditTarget::of(&self.image, self.choice)?;
        let color = destino.stored(&self.image, &self.slots)?;
        Some((destino, color))
    }

    /// Un fotograma con los eventos dados: sincroniza el editor, aplica la escala, monta la
    /// barra, el panel con el editor y después el lienzo, como la app. Limpia los cambios de
    /// textura (egui entra en pánico si se sueltan sin aplicar).
    fn fotograma(&mut self, events: Vec<Event>) -> Fotograma {
        let actual = self.actual();
        self.editor.sync(actual);
        let enabled = !self.editor.dragging();
        let quiere_antes = self.ctx.text_edit_focused();
        let input = self.entrada(events);
        let ppp = self.ppp();
        let mut resultado = None;
        let Banco {
            ctx,
            canvas,
            scroll,
            image,
            choice,
            slots,
            editor,
            ..
        } = self;
        let mut salida = ctx.run_ui(input, |ui| {
            apply_scale(ui.ctx());
            px_ui::status::show(ui, "lápiz · color 1 #000000", false);
            let panel =
                palette_panel::show_editing(ui, image, *choice, scroll, enabled, slots, editor);
            let rect_lienzo = ui.available_rect_before_wrap();
            let entradas = canvas.interact_ordered(ui);
            let info = canvas.paint(ui);
            resultado = Some((rect_lienzo, entradas, info, panel));
        });
        salida.textures_delta.clear();
        let formas: Vec<ClippedShape> = std::mem::take(&mut salida.shapes);
        let (rect_lienzo, entradas, info, panel) = resultado.expect("el lienzo se dibuja");
        let (mut textos, mut rects) = (Vec::new(), Vec::new());
        for clipped in &formas {
            recoge(&clipped.shape, ppp, &mut textos, &mut rects);
        }
        self.historial.push(panel.clone());
        Fotograma {
            rect_lienzo,
            entradas,
            info,
            salida: panel,
            textos,
            rects,
            quiere_antes,
            quiere_despues: self.ctx.text_edit_focused(),
        }
    }

    fn reposo(&mut self) -> Fotograma {
        self.fotograma(vec![])
    }

    /// `text_edit_focused()` ahora mismo (el estado tras el último fotograma).
    fn quiere_teclado(&self) -> bool {
        self.ctx.text_edit_focused()
    }

    // --- Ratón ---

    fn mueve(&mut self, x_px: i64, y_px: i64) -> Fotograma {
        let pos = en_puntos(self.ppp(), x_px, y_px);
        self.fotograma(vec![Event::PointerMoved(pos)])
    }

    fn pulsa(&mut self, x_px: i64, y_px: i64) -> Fotograma {
        let pos = en_puntos(self.ppp(), x_px, y_px);
        self.fotograma(vec![boton(pos, true)])
    }

    fn suelta(&mut self, x_px: i64, y_px: i64) -> Fotograma {
        let pos = en_puntos(self.ppp(), x_px, y_px);
        self.fotograma(vec![boton(pos, false)])
    }

    /// Mueve el puntero, pulsa y suelta (un fotograma cada vez).
    fn clic(&mut self, x_px: i64, y_px: i64) {
        self.mueve(x_px, y_px);
        self.pulsa(x_px, y_px);
        self.suelta(x_px, y_px);
    }

    /// Mueve el puntero y luego pulsa y suelta **en el mismo fotograma**.
    fn clic_en_un_fotograma(&mut self, x_px: i64, y_px: i64) -> Fotograma {
        self.mueve(x_px, y_px);
        let pos = en_puntos(self.ppp(), x_px, y_px);
        self.fotograma(vec![boton(pos, true), boton(pos, false)])
    }

    /// Un clic en el centro del lienzo (fuera del panel y del campo).
    fn clic_en_el_lienzo(&mut self) {
        let centro = self.reposo().rect_lienzo.center();
        let (x, y) = (
            (centro.x * self.ppp()).round() as i64,
            (centro.y * self.ppp()).round() as i64,
        );
        self.clic(x, y);
    }

    // --- Campo hexadecimal ---

    /// Un clic en el campo hexadecimal y un fotograma de reposo; comprueba que tiene el foco.
    fn enfoca_campo(&mut self) {
        let (x, y) = self.geo().campo();
        self.clic(x, y);
        self.reposo();
        assert!(
            self.quiere_teclado(),
            "un clic en el campo le da el foco de texto"
        );
    }

    /// `Ctrl+A` (seleccionar todo) en un fotograma. **[DEPENDE DE EGUI]**: así lo que se teclee
    /// sustituye lo que hubiera en el campo.
    fn selecciona_todo(&mut self) {
        self.fotograma(vec![tecla_con(Key::A, ctrl())]);
    }

    /// Sustituye el contenido del campo (ya enfocado) por `cadena`; devuelve el fotograma de
    /// las pulsaciones.
    fn escribe(&mut self, cadena: &str) -> Fotograma {
        self.selecciona_todo();
        self.fotograma(vec![texto(cadena)])
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
}

/// Pantalla en puntos para que el panel mida `panel_h_px` píxeles físicos de alto.
fn pantalla_para_panel(ppp: f32, panel_h_px: i64) -> Vec2 {
    let n = text_scale(ppp);
    vec2(
        800.0,
        (panel_h_px + i64::from(bar_height_px(n))) as f32 / ppp,
    )
}

/// Banco con `F` grande, elegida la entrada `entrada`, a `ppp`.
fn banco_f(ppp: f32, entrada: u8) -> Banco {
    Banco::nuevo(ppp, PANTALLA_ALTA, imagen_f(), ColorChoice::Entry(entrada))
}

/// Los colores `(r, g, b, a)` que muestra el editor, si hay.
fn mostrado(banco: &Banco) -> Rgba {
    banco.editor.shown().expect("el editor muestra un color").0
}

fn valor_mostrado(banco: &Banco, control: Control) -> u32 {
    let (rgba, hsl) = banco.editor.shown().expect("el editor muestra un color");
    control_value(rgba, hsl, control)
}

// ---------------------------------------------------------------------------------------------
// Fila 2: los textos del editor caben en 32 columnas y se maquetan con `Panic`
// ---------------------------------------------------------------------------------------------

/// Textos tecleados para provocar cada aviso del hexadecimal (algunos son válidos según el modo).
const TEXTOS_INVALIDOS: [&str; 6] = ["", "#", "12345", "zzz", "#ff0000", "#80808080"];

/// Comprueba que todo lo pintado cabe: ≤ 32 columnas, de `INTERFACE_CHARSET` (salvo lo que ha
/// tecleado el usuario, que aquí es ASCII) y no más ancho que el área de texto del panel.
fn comprueba_que_cabe(f: &Fotograma, ppp: f32, contexto: &str) {
    let n = text_scale(ppp);
    let maximo = columns((TEXT_WIDTH_U * i64::from(n)) as f32, ppp);
    assert_eq!(maximo, 32, "{contexto}: columns del área de texto");
    for t in &f.textos {
        assert!(
            t.texto.chars().count() <= maximo,
            "{contexto}: {:?} tiene {} caracteres y caben {maximo}",
            t.texto,
            t.texto.chars().count()
        );
        for c in t.texto.chars() {
            assert!(
                INTERFACE_CHARSET.contains(c) || c.is_ascii(),
                "{contexto}: {:?} lleva {c:?}",
                t.texto
            );
        }
        assert!(
            t.ancho_px <= (TEXT_WIDTH_U * i64::from(n)) as f32 + 0.5,
            "{contexto}: {:?} mide {} px y caben {}",
            t.texto,
            t.ancho_px,
            TEXT_WIDTH_U * i64::from(n)
        );
    }
}

#[test]
fn fila_02_las_cabeceras_son_las_de_la_spec() {
    assert_eq!(
        EditTarget::Entry(12).header(Mode::Indexed),
        "editar índice 12"
    );
    assert_eq!(EditTarget::Slot(3).header(Mode::Rgba), "editar color 3");
    assert_eq!(
        EditTarget::Slot(4).header(Mode::Gray),
        "editar color 4 gris"
    );
}

#[test]
fn fila_02_el_aviso_de_la_transparente_son_dos_lineas_y_la_primera_27_columnas() {
    let editor = ColorEditor::default();
    let [primera, segunda] = editor.notice(true);
    assert_eq!(primera, "aviso: entrada transparente");
    assert_eq!(primera.chars().count(), 27);
    assert_eq!(segunda, "se guarda con alfa 0");
}

#[test]
fn fila_02_los_textos_mas_largos_se_maquetan_con_panic_y_caben_a_todos_los_ppp() {
    for ppp in PPP_SIEMPRE {
        for mode in MODOS {
            let image = imagen_de(mode);
            for choice in elecciones_de(mode) {
                let contexto = format!("ppp {ppp}, {mode:?}, {choice:?}");
                let mut banco = Banco::nuevo(ppp, PANTALLA_ALTA, image.clone(), choice);
                let destino = EditTarget::of(&image, choice).expect("destino válido");
                let base = destino.stored(&image, &banco.slots).expect("guardado");

                // En reposo: la cabecera y el color del campo, todo dentro de las columnas.
                let f = banco.reposo();
                comprueba_que_cabe(&f, ppp, &contexto);
                assert!(
                    f.pinta(&destino.header(mode)),
                    "{contexto}: falta la cabecera {:?}",
                    destino.header(mode)
                );
                assert!(
                    f.pinta(&hex_text(base)),
                    "{contexto}: falta el campo {:?}",
                    hex_text(base)
                );

                // La transparente: el aviso de dos líneas.
                let transparente = image
                    .palette()
                    .is_some_and(|p| p.transparent_index().map(EditTarget::Entry) == Some(destino));
                if transparente {
                    assert!(
                        f.pinta("aviso: entrada transparente"),
                        "{contexto}: primera línea del aviso"
                    );
                    assert!(
                        f.pinta("se guarda con alfa 0"),
                        "{contexto}: segunda línea del aviso"
                    );
                }

                // Cada aviso del hexadecimal inválido (el de una línea, `aviso: ` + motivo).
                for escrito in TEXTOS_INVALIDOS {
                    banco.editor.type_hex(destino, base, escrito.to_owned());
                    let HexOutcome::Invalid(error) = banco.editor.confirm_hex(mode, false) else {
                        banco.editor.escape_hex();
                        continue;
                    };
                    let f = banco.reposo();
                    let contexto = format!("{contexto}, {escrito:?} → {error:?}");
                    comprueba_que_cabe(&f, ppp, &contexto);
                    let linea = format!("aviso: {}", hex_notice(&error));
                    assert!(f.pinta(&linea), "{contexto}: falta la línea {linea:?}");
                    assert!(linea.chars().count() <= 32, "{contexto}: {linea:?} no cabe");
                    banco.editor.escape_hex();
                }
            }
        }
    }
}

#[test]
fn fila_02_la_cabecera_mas_larga_y_el_valor_de_tres_cifras_caben() {
    // `editar índice 255` y los tres valores de 3 columnas, en las dos escalas extremas.
    for ppp in [1.0_f32, 3.0] {
        let mut banco = Banco::nuevo(
            ppp,
            PANTALLA_ALTA,
            imagen_indexada_256(),
            ColorChoice::Entry(255),
        );
        let f = banco.reposo();
        assert!(f.pinta("editar índice 255"), "ppp {ppp}");
        comprueba_que_cabe(&f, ppp, &format!("ppp {ppp}"));
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 13 (parte de ventana): la transparente muestra el color guardado
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_13b_la_transparente_muestra_el_guardado_y_el_aviso_de_dos_lineas() {
    // [R, G, (0,0,255,255)] con transparente Some(2) y la 2 elegida.
    let mut paleta = Palette::from_colors([ROJO, VERDE, AZUL]).expect("paleta");
    paleta.set_transparent_index(Some(2)).expect("existe");
    let image = Image::new_indexed(64, 48, paleta).expect("imagen");
    let mut banco = Banco::nuevo(1.0, PANTALLA_ALTA, image, ColorChoice::Entry(2));
    let f = banco.reposo();
    assert!(f.pinta("#0000ff"), "el campo muestra el color guardado");
    assert!(
        !f.pinta("#0000ff00"),
        "el campo no muestra el resuelto (con alfa 0)"
    );
    assert!(f.pinta("aviso: entrada transparente"));
    assert!(f.pinta("se guarda con alfa 0"));
}

// ---------------------------------------------------------------------------------------------
// Maqueta: dónde se pinta el editor (a los cinco ppp)
// ---------------------------------------------------------------------------------------------

#[test]
fn el_editor_se_ancla_al_fondo_y_no_se_mueve_con_el_numero_de_entradas() {
    for ppp in PPP_SIEMPRE {
        let n = i64::from(text_scale(ppp));
        let mut pocas = banco_f(ppp, 1);
        let mut muchas = Banco::nuevo(
            ppp,
            PANTALLA_ALTA,
            imagen_indexada_256(),
            ColorChoice::Entry(1),
        );
        let geo = pocas.geo();
        let esperado = (
            (HEADER_X_U * n) as f32,
            (geo.top_px + HEADER_Y_U * n) as f32,
        );
        for (nombre, banco) in [("4 entradas", &mut pocas), ("256 entradas", &mut muchas)] {
            let f = banco.reposo();
            let cabecera = f
                .texto_pintado("editar índice 1")
                .unwrap_or_else(|| panic!("ppp {ppp}, {nombre}: sin cabecera"));
            let (x, y) = (cabecera.pos.x * ppp, cabecera.pos.y * ppp);
            assert!(
                (x - esperado.0).abs() <= 0.05 && (y - esperado.1).abs() <= 0.05,
                "ppp {ppp}, {nombre}: cabecera en ({x}, {y}) px y se esperaba {esperado:?}"
            );
        }
    }
}

#[test]
fn el_campo_hexadecimal_se_pinta_donde_dice_edit_layout_con_el_fondo_surface_0() {
    for ppp in PPP_SIEMPRE {
        let mut banco = banco_f(ppp, 2);
        let geo = banco.geo();
        let n = geo.n;
        let f = banco.reposo();
        let esperado = Rect::from_min_size(
            pos2(
                (FIELD_X_U * n) as f32 / ppp,
                (geo.top_px + field_y_u(geo.mode) * n) as f32 / ppp,
            ),
            vec2((FIELD_W_U * n) as f32 / ppp, (FIELD_H_U * n) as f32 / ppp),
        );
        let tolerancia = 0.05 / ppp;
        assert!(
            f.rects.iter().any(|(rect, fill)| {
                *fill == SURFACE_0
                    && (rect.min.x - esperado.min.x).abs() <= tolerancia
                    && (rect.min.y - esperado.min.y).abs() <= tolerancia
                    && (rect.width() - esperado.width()).abs() <= tolerancia
                    && (rect.height() - esperado.height()).abs() <= tolerancia
            }),
            "ppp {ppp}: ningún rectángulo SURFACE_0 en {esperado:?}"
        );
    }
}

/// **[DEPENDE DE EGUI]** Nitidez del `TextEdit` a 1,25× y 1,5× (spec: «no comprobado»): el texto
/// del campo cae en píxeles físicos enteros, también con el campo enfocado.
#[test]
fn nitidez_el_texto_del_campo_cae_en_pixeles_fisicos_enteros() {
    for ppp in PPP_SIEMPRE {
        let mut banco = banco_f(ppp, 2);
        for enfocado in [false, true] {
            if enfocado {
                banco.enfoca_campo();
            }
            let f = banco.reposo();
            let campo = f
                .texto_pintado("#0000ff")
                .unwrap_or_else(|| panic!("ppp {ppp}: el campo no se pinta"));
            for (eje, valor) in [("x", campo.pos.x), ("y", campo.pos.y)] {
                let fisico = f64::from(valor) * f64::from(ppp);
                // egui redondea el diseño a 1/32 de punto (`GUI_ROUNDING`): la forma pintada puede
                // quedar a menos de 0,03 px de un píxel físico; el teselado redondea el texto al
                // píxel (`round_text_to_pixels`), así que se pinta nítido.
                assert!(
                    (fisico - fisico.round()).abs() <= 0.05,
                    "ppp {ppp}, enfocado {enfocado}: el texto del campo cae en {eje} = {fisico} px"
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 9: flujo con ventana (teclear, Intro, Escape, cambio de muestra)
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_09_en_reposo_el_campo_muestra_el_color_guardado() {
    let mut banco = banco_f(1.0, 2);
    let f = banco.reposo();
    assert!(f.pinta(&hex_text(AZUL)), "el campo muestra #0000ff");
    assert_eq!(banco.editor.hex_display(), None);
}

/// **[DEPENDE DE EGUI]** (Intro quita el foco al `TextEdit`).
#[test]
fn fila_09_teclear_e_intro_confirma_un_color_en_el_destino_y_el_campo_pierde_el_foco() {
    let mut banco = banco_f(1.0, 2);
    banco.enfoca_campo();

    let f = banco.escribe("#00ff00");
    assert_eq!(f.salida.commit, None, "teclear no confirma");
    assert_eq!(banco.editor.hex_display(), Some("#00ff00".to_owned()));
    assert!(banco.commits().is_empty());

    let f = banco.fotograma(vec![tecla(Key::Enter)]);
    assert_eq!(
        f.salida.commit,
        Some(Commit {
            target: EditTarget::Entry(2),
            color: VERDE
        }),
        "Intro confirma"
    );
    assert_eq!(f.salida.notice, None);
    assert_eq!(banco.editor.hex_display(), None, "lo pendiente se borra");

    // Un solo commit, y el campo ya no tiene el foco.
    banco.reposo();
    banco.reposo();
    assert_eq!(banco.commits().len(), 1);
    assert!(
        !banco.quiere_teclado(),
        "tras Intro el campo pierde el foco"
    );
}

#[test]
fn fila_09_teclear_el_color_actual_e_intro_no_da_paso() {
    let mut banco = banco_f(1.0, 2);
    banco.enfoca_campo();
    banco.escribe("#0000ff");
    let f = banco.fotograma(vec![tecla(Key::Enter)]);
    assert_eq!(f.salida.commit, None, "es el color actual: Unchanged");
    assert_eq!(f.salida.notice, None);
    banco.reposo();
    assert!(banco.commits().is_empty());
    assert_eq!(banco.editor.hex_display(), None);
}

#[test]
fn fila_09_escape_revierte_el_texto_sin_paso_ni_aviso() {
    let mut banco = banco_f(1.0, 2);
    banco.enfoca_campo();
    banco.escribe("#00ff00");
    assert_eq!(banco.editor.hex_display(), Some("#00ff00".to_owned()));

    let f = banco.fotograma(vec![tecla(Key::Escape)]);
    assert_eq!(f.salida.commit, None);
    assert_eq!(f.salida.notice, None);
    assert_eq!(banco.editor.hex_display(), None);
    let f = banco.reposo();
    assert!(f.pinta("#0000ff"), "el texto vuelve a #0000ff");
    assert!(!f.pinta("#00ff00"));
    assert!(banco.commits().is_empty());
    assert!(banco.avisos().is_empty());
    assert!(
        !banco.quiere_teclado(),
        "Escape saca el foco del campo (egui lo hace él)"
    );
}

/// **[DEPENDE DE EGUI]** Perder el foco por un clic en la rejilla en el **mismo fotograma** que la
/// elección de la muestra: la confirmación va al destino original.
#[test]
fn fila_09_cambio_de_muestra_en_el_mismo_fotograma_aplica_al_destino_original() {
    let mut banco = banco_f(1.0, 2);
    banco.enfoca_campo();
    banco.escribe("#00ff00");
    let n = banco.n();
    let (x, y) = centro_de_muestra(n, 3, 0);

    let f = banco.clic_en_un_fotograma(x, y);
    assert_eq!(
        f.salida.choice,
        Some(ColorChoice::Entry(3)),
        "el clic elige la muestra 3"
    );
    assert_eq!(
        f.salida.commit,
        Some(Commit {
            target: EditTarget::Entry(2),
            color: VERDE
        }),
        "el color se aplica a la entrada 2, no a la 3"
    );

    // La app cambia la elección; no vuelve a haber commits y el editor apunta a la entrada 3.
    banco.choice = ColorChoice::Entry(3);
    for _ in 0..3 {
        let f = banco.reposo();
        assert_eq!(f.salida.commit, None);
    }
    assert_eq!(banco.commits().len(), 1);
    assert_eq!(banco.editor.hex_display(), None);
}

/// **[DEPENDE DE EGUI]** Lo mismo con el foco perdido al **soltar** el botón (el `egui` por
/// defecto pierde el foco al hacer clic, no al pulsar): entre medias la app ya eligió la muestra 3
/// y `sync` ve otro destino. La confirmación sigue yendo al destino original.
#[test]
fn fila_09_cambio_de_muestra_antes_de_perder_el_foco_aplica_al_destino_original() {
    let mut banco = banco_f(1.0, 2);
    banco.enfoca_campo();
    banco.escribe("#00ff00");
    let n = banco.n();
    let (x, y) = centro_de_muestra(n, 3, 0);
    banco.historial.clear();

    banco.mueve(x, y);
    let f = banco.pulsa(x, y);
    assert_eq!(f.salida.choice, Some(ColorChoice::Entry(3)));
    banco.choice = ColorChoice::Entry(3);
    banco.suelta(x, y);
    banco.reposo();
    banco.reposo();

    assert_eq!(
        banco.commits(),
        vec![Commit {
            target: EditTarget::Entry(2),
            color: VERDE
        }],
        "un solo commit, a la entrada 2 con el verde tecleado"
    );
    assert!(banco.avisos().is_empty());
    assert_eq!(banco.editor.hex_display(), None);
    assert_eq!(banco.elecciones(), vec![ColorChoice::Entry(3)]);
}

/// **[DEPENDE DE EGUI]** Un clic fuera del campo le quita el foco y confirma el texto válido.
#[test]
fn fila_09_un_clic_fuera_confirma_el_texto_valido() {
    let mut banco = banco_f(1.0, 2);
    banco.enfoca_campo();
    banco.escribe("#00ff00");
    banco.historial.clear();
    banco.clic_en_el_lienzo();
    banco.reposo();
    assert_eq!(
        banco.commits(),
        vec![Commit {
            target: EditTarget::Entry(2),
            color: VERDE
        }]
    );
    assert!(!banco.quiere_teclado());
}

/// **[DEPENDE DE EGUI]** Enfocar y salir sin teclear nada: `Nothing`, sin paso ni aviso.
#[test]
fn fila_09_enfocar_y_salir_sin_teclear_no_hace_nada() {
    for salida in ["clic fuera", "Intro", "Escape"] {
        let mut banco = banco_f(1.0, 2);
        banco.enfoca_campo();
        banco.historial.clear();
        match salida {
            "clic fuera" => banco.clic_en_el_lienzo(),
            "Intro" => {
                banco.fotograma(vec![tecla(Key::Enter)]);
            }
            _ => {
                banco.fotograma(vec![tecla(Key::Escape)]);
            }
        }
        banco.reposo();
        banco.reposo();
        assert!(banco.commits().is_empty(), "{salida}: sin paso");
        assert!(banco.avisos().is_empty(), "{salida}: sin aviso");
        assert_eq!(banco.editor.hex_display(), None, "{salida}");
        assert!(!banco.quiere_teclado(), "{salida}: ya no hay foco");
    }
}

// ---------------------------------------------------------------------------------------------
// Fila 11: hexadecimal inválido (Intro, perder el foco, Escape)
// ---------------------------------------------------------------------------------------------

const MOTIVO_DIGITO: &str = "solo dígitos 0-9 y a-f";

/// **[DEPENDE DE EGUI]** Intro con un valor inválido: `show_editing` le devuelve el foco al campo.
#[test]
fn fila_11_intro_con_invalido_mantiene_el_foco_el_texto_y_el_aviso_del_editor() {
    let mut banco = banco_f(1.0, 2);
    let antes = banco.image.clone();
    banco.enfoca_campo();
    banco.escribe("zzz");
    assert_eq!(banco.editor.hex_display(), Some("zzz".to_owned()));

    let f = banco.fotograma(vec![tecla(Key::Enter)]);
    assert_eq!(f.salida.commit, None);
    assert_eq!(
        f.salida.notice, None,
        "con Intro el aviso es del editor, no de la barra"
    );
    let f = banco.reposo();
    assert!(
        banco.quiere_teclado(),
        "Intro con un valor inválido mantiene el foco"
    );
    assert_eq!(
        banco.editor.hex_display(),
        Some("zzz".to_owned()),
        "y el texto escrito"
    );
    assert!(
        f.pinta(&format!("aviso: {MOTIVO_DIGITO}")),
        "el editor muestra el aviso"
    );
    assert_eq!(banco.image, antes, "el documento no cambia");
    assert!(banco.commits().is_empty());

    // Al teclear, el aviso desaparece.
    banco.fotograma(vec![texto("q")]);
    let f = banco.reposo();
    assert_eq!(banco.editor.hex_display(), Some("zzzq".to_owned()));
    assert!(
        !f.pinta_algo_con(MOTIVO_DIGITO),
        "al teclear el aviso del editor se va"
    );
    assert!(banco.quiere_teclado());
}

/// **[DEPENDE DE EGUI]** Intro inválida y luego un clic fuera: el texto vuelve al color y la barra
/// avisa.
#[test]
fn fila_11_intro_invalido_y_clic_fuera_revierte_el_texto_y_avisa_en_la_barra() {
    let mut banco = banco_f(1.0, 2);
    banco.enfoca_campo();
    banco.escribe("zzz");
    banco.fotograma(vec![tecla(Key::Enter)]);
    banco.fotograma(vec![texto("q")]);
    banco.fotograma(vec![tecla(Key::Enter)]);
    banco.reposo();
    assert!(
        banco.quiere_teclado(),
        "sigue enfocado tras la segunda Intro"
    );
    banco.historial.clear();

    banco.clic_en_el_lienzo();
    let f = banco.reposo();
    banco.reposo();
    assert_eq!(
        banco.avisos(),
        vec![MOTIVO_DIGITO.to_owned()],
        "un solo aviso para la barra: el motivo, sin el prefijo"
    );
    assert!(banco.commits().is_empty(), "no hay paso");
    assert_eq!(banco.editor.hex_display(), None, "el texto se revierte");
    assert!(f.pinta(&hex_text(AZUL)), "el campo vuelve a #0000ff");
    assert!(!f.pinta("zzzq"));
    assert!(
        !f.pinta_algo_con(MOTIVO_DIGITO),
        "ya no hay aviso en el editor"
    );
    assert!(!banco.quiere_teclado());
}

/// **[DEPENDE DE EGUI]** Perder el foco con `Tab` sin haber pulsado Intro.
#[test]
fn fila_11_perder_el_foco_con_tab_revierte_y_avisa() {
    let mut banco = banco_f(1.0, 2);
    banco.enfoca_campo();
    banco.escribe("zzz");
    banco.historial.clear();

    banco.fotograma(vec![tecla(Key::Tab)]);
    banco.reposo();
    banco.reposo();
    assert_eq!(banco.avisos(), vec![MOTIVO_DIGITO.to_owned()]);
    assert!(banco.commits().is_empty());
    assert_eq!(banco.editor.hex_display(), None);
    let f = banco.reposo();
    assert!(f.pinta(&hex_text(AZUL)));
}

/// **[DEPENDE DE EGUI]** Perder el foco con un clic fuera sin Intro previa.
#[test]
fn fila_11_perder_el_foco_con_un_clic_revierte_y_avisa() {
    let mut banco = banco_f(1.0, 2);
    banco.enfoca_campo();
    banco.escribe("zzz");
    banco.historial.clear();

    banco.clic_en_el_lienzo();
    banco.reposo();
    banco.reposo();
    assert_eq!(banco.avisos(), vec![MOTIVO_DIGITO.to_owned()]);
    assert!(banco.commits().is_empty());
    assert_eq!(banco.editor.hex_display(), None);
}

#[test]
fn fila_11_escape_revierte_el_texto_sin_aviso_ni_paso() {
    let mut banco = banco_f(1.0, 2);
    banco.enfoca_campo();
    banco.escribe("zzz");
    banco.historial.clear();

    let f = banco.fotograma(vec![tecla(Key::Escape)]);
    assert_eq!(f.salida.notice, None);
    assert_eq!(f.salida.commit, None);
    banco.reposo();
    let f = banco.reposo();
    assert!(banco.avisos().is_empty(), "Escape no avisa");
    assert!(banco.commits().is_empty());
    assert_eq!(banco.editor.hex_display(), None);
    assert!(f.pinta(&hex_text(AZUL)), "el texto vuelve al color actual");
    assert!(
        !f.pinta_algo_con("aviso"),
        "ni aviso en el editor: {:?}",
        f.textos.iter().map(|t| &t.texto).collect::<Vec<_>>()
    );
}

#[test]
fn fila_11_escape_tras_una_intro_invalida_quita_el_aviso_del_editor() {
    let mut banco = banco_f(1.0, 2);
    banco.enfoca_campo();
    banco.escribe("zzz");
    banco.fotograma(vec![tecla(Key::Enter)]);
    let f = banco.reposo();
    assert!(f.pinta_algo_con(MOTIVO_DIGITO));
    banco.historial.clear();

    banco.fotograma(vec![tecla(Key::Escape)]);
    let f = banco.reposo();
    assert!(!f.pinta_algo_con(MOTIVO_DIGITO));
    assert!(banco.avisos().is_empty());
    assert!(banco.commits().is_empty());
    assert_eq!(banco.editor.hex_display(), None);
}

// ---------------------------------------------------------------------------------------------
// Fila 16: teclear `ff00ab` con el campo enfocado
// ---------------------------------------------------------------------------------------------

#[test]
fn fila_16_con_el_campo_enfocado_el_teclado_es_del_campo() {
    let mut banco = banco_f(1.0, 1);
    let zoom_ajuste = banco.reposo().info.zoom;
    assert_eq!(Some(zoom_ajuste), Zoom::from_int(8), "la vista parte en ×8");
    banco.enfoca_campo();
    banco.selecciona_todo();
    banco.historial.clear();

    // Teclear ff00ab.
    let tecleo = pulsaciones("ff00ab");
    let f = banco.fotograma(tecleo.clone());
    assert!(banco.quiere_teclado());
    assert!(f.quiere_antes && f.quiere_despues);
    assert!(
        gate_keys(&tecleo, f.quiere_antes).is_empty(),
        "los atajos no ven lo tecleado: ni la f, ni los números"
    );
    assert!(gate_keys(&tecleo, f.quiere_despues).is_empty());
    assert_eq!(f.info.zoom, zoom_ajuste);
    assert_eq!(
        banco.editor.hex_display(),
        Some("ff00ab".to_owned()),
        "el campo tiene lo tecleado (sin # si no se escribió)"
    );
    assert_eq!(f.salida.commit, None, "no se toca el color hasta confirmar");
    assert_eq!(f.salida.choice, None);
    assert!(f.entradas.is_empty());

    // Home, + y -: el lienzo no cambia la vista.
    let vista = f.info.image_rect;
    let teclas = vec![
        tecla(Key::Home),
        tecla(Key::Plus),
        tecla(Key::Equals),
        tecla(Key::Minus),
    ];
    let f = banco.fotograma(teclas.clone());
    assert!(gate_keys(&teclas, f.quiere_antes).is_empty());
    assert_eq!(
        f.info.zoom, zoom_ajuste,
        "ni Home, ni +, ni - cambian el zoom"
    );
    assert_eq!(f.info.image_rect, vista, "ni la posición de la imagen");
    assert_eq!(banco.editor.hex_display(), Some("ff00ab".to_owned()));
    assert!(banco.commits().is_empty());
}

#[test]
fn fila_16_contraste_sin_el_campo_enfocado_el_lienzo_si_atiende_las_teclas() {
    let mut banco = banco_f(1.0, 1);
    let zoom_ajuste = banco.reposo().info.zoom;
    assert!(!banco.quiere_teclado());
    let f = banco.fotograma(vec![tecla(Key::Plus)]);
    assert_eq!(
        f.info.zoom,
        zoom_ajuste.zoom_in(),
        "sin foco de texto, + acerca (el banco recoge las teclas)"
    );
    let f = banco.fotograma(vec![tecla(Key::Home)]);
    assert_eq!(f.info.zoom, zoom_ajuste, "y Home reencuadra");

    // `gate_keys` sin foco de texto lo deja pasar todo, en el mismo orden.
    let tecleo = pulsaciones("ff00ab");
    assert_eq!(gate_keys(&tecleo, false), tecleo);
}

// ---------------------------------------------------------------------------------------------
// Fila 17: al salir del campo los atajos vuelven
// ---------------------------------------------------------------------------------------------

/// Ctrl+Z, Ctrl+Y, Ctrl+S, la tecla 3 y Home.
fn atajos() -> Vec<Event> {
    vec![
        tecla_con(Key::Z, ctrl()),
        tecla_con(Key::Y, ctrl()),
        tecla_con(Key::S, ctrl()),
        tecla(Key::Num3),
        tecla(Key::Home),
    ]
}

/// Enfoca el campo, comprueba que `gate_keys` calla, sale con `salir` y comprueba que los atajos
/// vuelven (todos, en el mismo orden) y que `Home` y `+` actúan en el lienzo.
fn comprueba_salida_del_campo(nombre: &str, salir: impl FnOnce(&mut Banco)) {
    let mut banco = banco_f(1.0, 1);
    let zoom_ajuste = banco.reposo().info.zoom;
    banco.enfoca_campo();
    let eventos = atajos();
    assert!(
        gate_keys(&eventos, banco.quiere_teclado()).is_empty(),
        "{nombre}: con el campo enfocado no hay atajos"
    );

    salir(&mut banco);
    banco.reposo();
    banco.reposo();
    assert!(
        !banco.quiere_teclado(),
        "{nombre}: tras salir del campo no queda foco de texto"
    );
    assert_eq!(
        gate_keys(&eventos, banco.quiere_teclado()),
        eventos,
        "{nombre}: los atajos vuelven, todos y en el mismo orden"
    );

    // Y en el lienzo: `+` acerca y `Home` reencuadra.
    let f = banco.fotograma(vec![tecla(Key::Plus)]);
    assert_eq!(
        f.info.zoom,
        zoom_ajuste.zoom_in(),
        "{nombre}: + vuelve a acercar"
    );
    let f = banco.fotograma(vec![tecla(Key::Home)]);
    assert_eq!(
        f.info.zoom, zoom_ajuste,
        "{nombre}: Home vuelve a reencuadrar"
    );
}

#[test]
fn fila_17_tras_intro_los_atajos_vuelven() {
    comprueba_salida_del_campo("Intro", |b| {
        b.fotograma(vec![tecla(Key::Enter)]);
    });
}

#[test]
fn fila_17_tras_escape_los_atajos_vuelven() {
    comprueba_salida_del_campo("Escape", |b| {
        b.fotograma(vec![tecla(Key::Escape)]);
    });
}

/// **[DEPENDE DE EGUI]**
#[test]
fn fila_17_tras_un_clic_fuera_los_atajos_vuelven() {
    comprueba_salida_del_campo("clic fuera", |b| b.clic_en_el_lienzo());
}

/// **[DEPENDE DE EGUI]** `Tab` mueve el foco al siguiente widget enfocable: el lienzo es
/// `Sense::click_and_drag` y lo recibe, así que `egui_wants_keyboard_input()` seguiría cierto. La
/// guarda de teclado usa `text_edit_focused()` (solo un campo de texto), y entonces los atajos
/// vuelven (comprobado en el banco: decisión 11 de la spec).
#[test]
fn fila_17_tras_tab_los_atajos_vuelven() {
    comprueba_salida_del_campo("Tab", |b| {
        b.fotograma(vec![tecla(Key::Tab)]);
    });
}

// ---------------------------------------------------------------------------------------------
// Ventana baja
// ---------------------------------------------------------------------------------------------
//
// La fila 18 de la spec del editor (ocultar el editor por debajo de `(41 + 14 + H)·n`, con su
// aviso `editor_hidden` y la cancelación del arrastre) la sustituye specs/hito-2-panel-ventana-baja.md:
// el editor no se oculta nunca por la altura. Esos casos con ventana (el diseño con 241·n, 240·n,
// 200·n y 100·n, el desplazamiento, el arrastre que no se cancela y la ausencia de avisos) están
// en `panel_baja.rs`. Aquí queda lo que sigue siendo cierto sin la altura: sin destino no hay
// editor.

#[test]
fn sin_destino_no_hay_editor_ni_commit_aunque_el_panel_sea_bajo() {
    let n = 1;
    let casos = [
        (imagen_f(), ColorChoice::Entry(200)),
        (imagen_de(Mode::Rgba), ColorChoice::Slot(10)),
        (imagen_de(Mode::Rgba), ColorChoice::Slot(0)),
        (imagen_de(Mode::Gray), ColorChoice::Entry(0)),
    ];
    for (image, choice) in casos {
        for alto in [240 * n, 700 * n] {
            let mut banco = Banco::con_alto_de_panel(1.0, alto, image.clone(), choice);
            assert!(banco.actual().is_none(), "{choice:?} no tiene destino");
            let f = banco.reposo();
            assert!(
                !f.pinta_algo_con("editar"),
                "{choice:?}, alto {alto}: sin destino no hay editor"
            );
            assert_eq!(f.salida.commit, None);
            assert_eq!(f.salida.notice, None);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Arrastre de deslizadores con el ratón
// ---------------------------------------------------------------------------------------------

#[test]
fn deslizador_pulsar_en_la_pista_fija_el_valor_mover_lo_cambia_y_soltar_da_un_commit() {
    for ppp in PPP_SIEMPRE {
        // Entrada 1 de F: verde (0, 255, 0).
        let mut banco = banco_f(ppp, 1);
        let antes = banco.image.clone();
        let geo = banco.geo();
        let y = geo.y_fila(Control::Red);
        let dx_a = geo.ancho_pista() / 2;
        let dx_b = geo.ancho_pista() - 1;
        let (x_a, x_b) = (geo.x_pista() + dx_a, geo.x_pista() + dx_b);
        let valor_a = geo.valor_en(dx_a, Control::Red);
        let valor_b = geo.valor_en(dx_b, Control::Red);
        assert_eq!(valor_b, Control::Red.max());
        assert_eq!(valor_mostrado(&banco, Control::Red), 0);

        // Pulsar ya fija el valor bajo el puntero.
        banco.mueve(x_a, y);
        let f = banco.pulsa(x_a, y);
        assert!(
            banco.editor.dragging(),
            "ppp {ppp}: pulsar empieza el arrastre"
        );
        assert_eq!(valor_mostrado(&banco, Control::Red), valor_a, "ppp {ppp}");
        assert_eq!(
            banco.editor.preview(),
            Some((EditTarget::Entry(1), Rgba::opaque(valor_a as u8, 255, 0))),
            "ppp {ppp}: el provisional solo cambia el canal rojo"
        );
        assert_eq!(f.salida.commit, None, "ppp {ppp}: pulsar no confirma");

        // Mover con el botón pulsado lo cambia; el documento no se toca.
        let f = banco.mueve(x_b, y);
        assert_eq!(valor_mostrado(&banco, Control::Red), valor_b, "ppp {ppp}");
        assert_eq!(
            banco.editor.preview(),
            Some((EditTarget::Entry(1), AMARILLO))
        );
        assert_eq!(f.salida.commit, None);
        assert_eq!(banco.image, antes, "ppp {ppp}: el documento no cambia");
        assert!(f.entradas.is_empty(), "ppp {ppp}: el lienzo no dibuja");

        // La muestra elegida del panel se pinta con el color provisional (y no con el guardado).
        let f = banco.reposo();
        assert!(
            f.hay_relleno(Color32::from_rgb(255, 255, 0)),
            "ppp {ppp}: la muestra se pinta en amarillo"
        );
        assert!(
            !f.hay_relleno(Color32::from_rgb(0, 255, 0)),
            "ppp {ppp}: ya no se pinta el verde guardado"
        );

        // Soltar da un commit, una sola vez.
        let f = banco.suelta(x_b, y);
        assert_eq!(
            f.salida.commit,
            Some(Commit {
                target: EditTarget::Entry(1),
                color: AMARILLO
            }),
            "ppp {ppp}"
        );
        assert!(!banco.editor.dragging());
        banco.reposo();
        banco.reposo();
        assert_eq!(banco.commits().len(), 1, "ppp {ppp}: un solo commit");
        assert_eq!(banco.image, antes);
    }
}

#[test]
fn deslizador_la_x_se_limita_a_la_pista_aunque_el_puntero_salga_de_ella_o_de_la_fila() {
    let mut banco = banco_f(1.0, 1);
    let geo = banco.geo();
    let y = geo.y_fila(Control::Red);
    let x_a = geo.x_de_valor(100, Control::Red.max());
    banco.mueve(x_a, y);
    banco.pulsa(x_a, y);
    assert!(banco.editor.dragging());

    // A la izquierda de la pista, bien arriba (fuera de la fila): el valor es 0.
    let f = banco.mueve(2, 2);
    assert_eq!(valor_mostrado(&banco, Control::Red), 0);
    assert!(f.entradas.is_empty());

    // Sobre el lienzo, a la derecha del panel y bien abajo: el valor es el máximo.
    let fuera_x = PANEL_WIDTH_U * geo.n + 100;
    let f = banco.mueve(fuera_x, geo.top_px + 20 * geo.n);
    assert_eq!(valor_mostrado(&banco, Control::Red), Control::Red.max());
    assert!(
        f.entradas.is_empty(),
        "el lienzo no ve el arrastre del deslizador"
    );

    // Soltar fuera de la ventana de la fila: confirma el último valor.
    let f = banco.suelta(fuera_x, geo.top_px + 20 * geo.n);
    assert_eq!(
        f.salida.commit,
        Some(Commit {
            target: EditTarget::Entry(1),
            color: AMARILLO
        })
    );
    assert!(
        f.entradas.is_empty(),
        "soltar sobre el lienzo no es un trazo"
    );
}

#[test]
fn deslizador_soltar_sin_cambio_da_none() {
    // Un clic sin mover sobre el valor actual (R = 0 en el verde).
    let mut banco = banco_f(1.0, 1);
    let geo = banco.geo();
    let y = geo.y_fila(Control::Red);
    let inicio = geo.x_pista();
    banco.mueve(inicio, y);
    banco.pulsa(inicio, y);
    assert!(banco.editor.dragging());
    assert_eq!(valor_mostrado(&banco, Control::Red), 0);
    let f = banco.suelta(inicio, y);
    assert_eq!(f.salida.commit, None, "mismo valor: sin paso");
    assert!(!banco.editor.dragging());

    // Se aleja y vuelve al valor inicial antes de soltar.
    banco.mueve(inicio, y);
    banco.pulsa(inicio, y);
    let lejos = geo.x_de_valor(200, Control::Red.max());
    banco.mueve(lejos, y);
    assert_ne!(valor_mostrado(&banco, Control::Red), 0);
    banco.mueve(inicio, y);
    let f = banco.suelta(inicio, y);
    assert_eq!(f.salida.commit, None, "vuelve al valor inicial: sin paso");
    banco.reposo();
    assert!(banco.commits().is_empty());
    assert_eq!(banco.editor.preview(), None);
}

#[test]
fn deslizador_escape_cancela_restaura_y_ignora_los_movimientos_siguientes() {
    let mut banco = banco_f(1.0, 1);
    let geo = banco.geo();
    let y = geo.y_fila(Control::Red);
    let max = Control::Red.max();
    let (x_a, x_b) = (geo.x_de_valor(100, max), geo.x_de_valor(max, max));
    banco.mueve(x_a, y);
    banco.pulsa(x_a, y);
    banco.mueve(x_b, y);
    assert!(banco.editor.dragging());
    assert_ne!(mostrado(&banco), VERDE);

    let f = banco.fotograma(vec![tecla(Key::Escape)]);
    assert_eq!(f.salida.commit, None);
    assert!(!banco.editor.dragging(), "Escape cancela el arrastre");
    assert_eq!(banco.editor.preview(), None);
    assert_eq!(mostrado(&banco), VERDE, "restaura el color guardado");
    let f = banco.reposo();
    assert!(
        f.hay_relleno(Color32::from_rgb(0, 255, 0)),
        "la muestra vuelve a pintarse con el guardado"
    );

    // Los movimientos siguientes se ignoran hasta soltar el botón (sin reanudar).
    banco.mueve(x_a, y);
    assert!(!banco.editor.dragging());
    assert_eq!(banco.editor.preview(), None);
    assert_eq!(mostrado(&banco), VERDE);
    let f = banco.suelta(x_a, y);
    assert_eq!(f.salida.commit, None, "soltar tras cancelar no da paso");
    banco.reposo();
    assert!(banco.commits().is_empty());
}

#[test]
fn deslizador_perder_el_foco_la_ventana_cancela() {
    let mut banco = banco_f(1.0, 1);
    let geo = banco.geo();
    let y = geo.y_fila(Control::Red);
    let max = Control::Red.max();
    let (x_a, x_b) = (geo.x_de_valor(100, max), geo.x_de_valor(max, max));
    banco.mueve(x_a, y);
    banco.pulsa(x_a, y);
    banco.mueve(x_b, y);
    assert!(banco.editor.dragging());

    banco.fotograma(vec![Event::WindowFocused(false)]);
    assert!(
        !banco.editor.dragging(),
        "perder el foco la ventana cancela"
    );
    assert_eq!(banco.editor.preview(), None);
    assert_eq!(mostrado(&banco), VERDE);
    banco.suelta(x_b, y);
    banco.reposo();
    assert!(banco.commits().is_empty(), "no se aplica un color a medias");
}

#[test]
fn deslizador_con_un_arrastre_en_curso_la_rejilla_no_elige() {
    // `busy`: mientras hay un arrastre, `enabled` es falso y se ignoran los clics en la rejilla.
    let mut banco = banco_f(1.0, 1);
    let geo = banco.geo();
    let y = geo.y_fila(Control::Red);
    let x_a = geo.x_de_valor(100, Control::Red.max());
    banco.mueve(x_a, y);
    banco.pulsa(x_a, y);
    assert!(banco.editor.dragging());

    let (x, y_muestra) = centro_de_muestra(geo.n, 3, 0);
    banco.mueve(x, y_muestra);
    let pos = en_puntos(geo.ppp, x, y_muestra);
    let f = banco.fotograma(vec![boton(pos, true)]);
    assert_eq!(
        f.salida.choice, None,
        "la rejilla no elige durante el arrastre"
    );
    assert!(banco.elecciones().is_empty());
}

#[test]
fn zona_activa_de_la_fila_de_x_13u_incluida_a_201u_excluida_y_sin_los_huecos_de_grupo() {
    // Casos: (nombre, borde en u, desplazamiento en píxeles, valor que debe fijar si arrastra).
    let casos = [
        ("borde izquierdo incluido", HIT_X0_U, 0, Some(0u32)),
        ("antes del borde izquierdo", HIT_X0_U, -1, None),
        (
            "último píxel de la zona",
            HIT_X1_U,
            -1,
            Some(Control::Red.max()),
        ),
        ("borde derecho excluido", HIT_X1_U, 0, None),
    ];
    for ppp in [1.0_f32, 2.0] {
        for (nombre, borde_u, delta, valor_esperado) in casos {
            let mut banco = banco_f(ppp, 1);
            let geo = banco.geo();
            let x = borde_u * geo.n + delta;
            let y = geo.y_fila(Control::Red);
            assert_eq!(
                slider_hit(geo.mode, x, y - geo.top_px, geo.n as u32).is_some(),
                valor_esperado.is_some(),
                "ppp {ppp}, {nombre}: edit_layout::slider_hit"
            );
            banco.mueve(x, y);
            banco.pulsa(x, y);
            assert_eq!(
                banco.editor.dragging(),
                valor_esperado.is_some(),
                "ppp {ppp}, {nombre}: ¿empieza el arrastre?"
            );
            if let Some(valor) = valor_esperado {
                assert_eq!(
                    valor_mostrado(&banco, Control::Red),
                    valor,
                    "ppp {ppp}, {nombre}"
                );
            }
        }
    }

    // El hueco entre el grupo RGB y el HSL, a la altura de su primer píxel, no es de ninguna fila.
    let mut banco = banco_f(1.0, 1);
    let geo = banco.geo();
    let fila_azul = control_row_u(geo.mode, Control::Blue).expect("tiene B");
    let y = geo.top_px + (fila_azul + ROW_H_U) * geo.n;
    let x = geo.x_de_valor(100, Control::Blue.max());
    assert_eq!(slider_hit(geo.mode, x, y - geo.top_px, geo.n as u32), None);
    banco.mueve(x, y);
    banco.pulsa(x, y);
    assert!(
        !banco.editor.dragging(),
        "el hueco entre grupos no arrastra"
    );
}

#[test]
fn cada_fila_arrastra_su_control() {
    // Indexadas: R G B | H S L | A; cada fila empieza el arrastre de su propio control.
    for control in controls(Mode::Indexed).to_vec() {
        let mut banco = banco_f(1.0, 1);
        let geo = banco.geo();
        let y = geo.y_fila(control);
        let x = geo.x_de_valor(control.max() / 2, control.max());
        banco.mueve(x, y);
        banco.pulsa(x, y);
        assert!(banco.editor.dragging(), "{control:?}");
        let esperado = geo.valor_en(x - geo.x_pista(), control);
        assert_eq!(valor_mostrado(&banco, control), esperado, "{control:?}");
    }
}

#[test]
fn deslizador_gris_de_gray_confirma_una_ranura_gris() {
    // Gray: un solo deslizador «G»; la ranura 5 es un gris (128 por defecto).
    let mut banco = Banco::nuevo(
        1.0,
        PANTALLA_ALTA,
        imagen_de(Mode::Gray),
        ColorChoice::Slot(5),
    );
    assert_eq!(controls(Mode::Gray), &[Control::Gray]);
    let geo = banco.geo();
    let y = geo.y_fila(Control::Gray);
    let max = Control::Gray.max();
    let x = geo.x_de_valor(max, max);
    banco.mueve(x, y);
    banco.pulsa(x, y);
    assert!(banco.editor.dragging());
    assert_eq!(mostrado(&banco), Rgba::opaque(255, 255, 255));
    let f = banco.suelta(x, y);
    assert_eq!(
        f.salida.commit,
        Some(Commit {
            target: EditTarget::Slot(5),
            color: Rgba::opaque(255, 255, 255)
        })
    );
}

#[test]
fn deslizador_de_alfa_de_gray_alpha_confirma_alfa_0() {
    // GrayAlpha: «G» y «A»; la ranura 1 es negro opaco por defecto.
    let mut banco = Banco::nuevo(
        1.0,
        PANTALLA_ALTA,
        imagen_de(Mode::GrayAlpha),
        ColorChoice::Slot(1),
    );
    assert_eq!(controls(Mode::GrayAlpha), &[Control::Gray, Control::Alpha]);
    let geo = banco.geo();
    let y = geo.y_fila(Control::Alpha);
    let x = geo.x_pista();
    banco.mueve(x, y);
    banco.pulsa(x, y);
    assert!(banco.editor.dragging());
    assert_eq!(mostrado(&banco), Rgba::new(0, 0, 0, 0));
    let f = banco.suelta(x, y);
    assert_eq!(
        f.salida.commit,
        Some(Commit {
            target: EditTarget::Slot(1),
            color: Rgba::new(0, 0, 0, 0)
        })
    );
}

#[test]
fn la_imagen_que_recibe_el_panel_no_cambia_durante_todo_el_flujo() {
    // Arrastre confirmado, arrastre cancelado y hexadecimal: la imagen es la misma (el panel solo
    // devuelve `commit`; aplicarlo es de la sesión).
    let mut banco = banco_f(1.0, 1);
    let antes = banco.image.clone();
    let geo = banco.geo();
    let y = geo.y_fila(Control::Green);
    let max = Control::Green.max();
    let x = geo.x_de_valor(10, max);
    banco.mueve(x, y);
    banco.pulsa(x, y);
    banco.suelta(x, y);
    banco.mueve(x, y);
    banco.pulsa(x, y);
    banco.fotograma(vec![tecla(Key::Escape)]);
    banco.suelta(x, y);
    banco.enfoca_campo();
    banco.escribe("#123456");
    banco.fotograma(vec![tecla(Key::Enter)]);
    assert_eq!(banco.image, antes);
    // Nada tocó `Pixel` alguno: la imagen es una indexada vacía de índice 0.
    assert_eq!(banco.image.get_pixel(0, 0), Ok(Pixel::Indexed(0)));
}
