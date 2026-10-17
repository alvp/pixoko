# Hito 2 · Unidad 7: herramientas línea y relleno en la app

Estado: **aprobada** (2026-10-17; decisiones 1 a 9 con los matices del visto bueno incorporados al texto: filas de `Ctrl`+`P`/`L`/`F`, un relleno por pulsación, orden de `Escape` y soltado según los eventos del fotograma, caducidad del error como función pura con `request_repaint_after`, estimación del relleno de 16384² y prueba manual en release, título completo desde una sola función, `busy()` que se limpia al perder el foco y al soltar fuera, y `ToolState` sin tipos de egui). **No toca `px-core` ni `px-raster`**: `Line`, `line_command`, `draw_line`, `fill_command` y `flood_fill` ya son públicos y bastan. Sin dependencias nuevas. No se escribe código ni tests hasta la aprobación.

## Objetivo

Elegir entre lápiz, línea y relleno con el teclado y usar las dos nuevas con el ratón. Cada gesto es **un solo paso de historia**. La lógica de herramientas es pura y se prueba sin ventana.

## Piezas

La lógica de trazo que hoy está en `main.rs` pasa a `px-ui` para poder probarla. `Stroke`, `color` y `shortcuts` **no cambian**. `Canvas` solo gana, de forma aditiva, el evento `CanvasEvent::Escape` (ver sección 2). La API de `ToolState` **no usa tipos de egui**: `main.rs` solo traduce eventos de egui a `CanvasEvent` y a `ToolAction`.

```rust
// px-ui/src/tool.rs  (solo usa egui::Key y egui::Modifiers como datos)
pub enum Tool { Pencil, Line, Fill }
impl Tool { pub fn label(self) -> &'static str }   // "lápiz", "línea", "relleno"
pub enum ToolAction { Select(Tool), Cancel }
pub fn tool_action_for_key(key: Key, modifiers: Modifiers) -> Option<ToolAction>;
/// Texto del título para herramienta y color: "línea · color 3 #ff0000"; sin color, solo la herramienta.
pub fn status_text(tool: Tool, color: Option<&str>) -> String;

// px-ui/src/canvas.rs: variante nueva, aditiva
// CanvasEvent::Escape   // `Escape` sin modificadores, en el mismo orden que los eventos del puntero

pub struct ToolState { /* herramienta activa y gesto en curso */ }
impl ToolState {
    pub fn new() -> ToolState;                      // lápiz
    pub fn tool(&self) -> Tool;
    pub fn busy(&self) -> bool;                     // entre Begin y End, ambos incluidos
    pub fn apply_action(&mut self, action: ToolAction);
    /// Ok(true): el documento cambió (hay que hacer `refresh`). Ok(false): nada que hacer.
    /// `CanvasEvent::Escape` equivale a `apply_action(Cancel)`.
    pub fn canvas_event(&mut self, doc: &mut Document, image: ImageId, slot: u8, event: CanvasEvent)
        -> Result<bool, DocumentError>;
    /// Píxeles pendientes para la superposición (lápiz: el trazo; línea: la línea recortada).
    pub fn preview(&self) -> impl Iterator<Item = PixelEdit> + '_;
}

// px-ui/src/notice.rs  (sin egui; el tiempo es un f64 en segundos que da quien llama)
pub const NOTICE_SECS: f64 = 5.0;
pub struct Notice { /* texto y instante de caducidad */ }
impl Notice {
    pub fn new(text: String, now: f64) -> Notice;
    pub fn visible(&self, now: f64) -> Option<&str>;   // Some si now < instante + NOTICE_SECS
}
```

En `px-app/src/title.rs`, una **sola función** da el título completo: `pub fn window_title(tool: Tool, info: &TitleInfo) -> String`, que construye `status_text(tool, info.color)` dentro del campo `color` y llama a `title_text`. `title_text` y `TitleInfo` no cambian (sus tests tampoco). El apaño de meter la herramienta en `color` queda dentro de esa función y anotado para la unidad 14.

`px-app` queda como pegamento: pasa los eventos de `Canvas::interact` a `ToolState` (incluido `Escape`), las teclas **de selección** a `tool_action_for_key` (el `Cancel` ya llega por `CanvasEvent::Escape`, en orden) y `action_for_key`, y llama a `refresh` cuando algo cambia (como en la unidad 6).

## Comportamiento

### 1. Selección de herramienta y título

- `tool_action_for_key` es total y pura: **`P` ⇒ `Select(Pencil)`, `L` ⇒ `Select(Line)`, `F` ⇒ `Select(Fill)`, `Escape` ⇒ `Cancel`, solo sin ningún modificador**. Con cualquier modificador, o con otra tecla, `None`. Las teclas son provisionales hasta la barra de herramientas (unidad 11).
- **Función aparte de `action_for_key`**: la propiedad 6 de la unidad 6 recorre `Key::ALL` y exige `None` para todo lo que no sea `Z`, `Y` o `1` a `9`. Ampliarla obligaría a tocar esa prueba. `action_for_key` sigue dando `None` para `P`, `L`, `F` y `Escape`.
- **Teclados no QWERTY.** egui-winit 0.37 usa la tecla lógica (la letra que produce la distribución) y, si no es una letra latina, cae a la tecla física (`logical_key.or(physical_key)`, `egui-winit-0.37.0/src/lib.rs:1128`). Así `P`, `L` y `F` funcionan en español, y en distribuciones no latinas por posición. Esas tres letras no cambian de sitio entre QWERTY, QWERTZ y AZERTY. **No está probado en un teclado físico no QWERTY**: la prueba cubre la función pura. Las teclas de color `1` a `9` son de la unidad 6 y no cambian (en AZERTY exigen Mayús; queda para la unidad 11).
- Sin choques con lo que ya existe: `Canvas` usa `Space`, `Home`, `+`, `-`, `=` y rueda; la app usa `command`+`Z`/`Y` y `1` a `9`.
- **`Ctrl`+`P`, `Ctrl`+`L` y `Ctrl`+`F` no cambian de herramienta** (hay que cubrir `ctrl`, `command` y `mac_cmd` por separado y juntos: en Linux egui da `ctrl` y `command` a la vez).
- **Título:** `Pixoko: 64×48 · ×8 · (3, 4) · línea · color 3 #ff0000 · deshacer`, de `window_title` (ver Piezas), sin cambiar `TitleInfo` ni `title_text` (añadir un campo obligaría a tocar los 13 literales de los tests de la unidad 6).
- **Cambiar de herramienta durante un gesto se ignora**: `Select` no hace nada si `busy()`. Lo mismo ocurre con deshacer, rehacer y las teclas de color (regla de la unidad 6). `Cancel` sí actúa durante un gesto.
- `busy()` es cierto desde `Begin` hasta `End` **aunque no haya gesto real** (por ejemplo, una ranura sin color en indexado). Cambia un detalle de la unidad 6: allí, en ese caso, los atajos valían a mitad de pulsación. **Nunca queda bloqueado:** `Canvas` emite siempre un `End` (soltar dentro o fuera de la ventana, perder el foco o el botón que egui ya no ve), y `busy()` vuelve a falso con él; una ranura sin color no deja los atajos bloqueados después del `End`.
- Desplazamiento, zoom, `Home`, `+`, `-`, atajos de deshacer y teclas de color valen con cualquier herramienta: `Canvas` no conoce la herramienta.

### 2. Línea

- `Begin(p)`: guarda el origen `p`, el destino `p` y el `Pixel` de la ranura activa en ese momento (`pixel_for_slot`; `None` ⇒ no hay gesto, solo `busy`). `Move(q)`: el destino pasa a ser `q`. **No se acumula nada**: la línea es siempre `origen → último destino`.
- **Vista previa:** `Line::new(origen, destino).clip_to(ancho, alto)` con el `Pixel` del gesto, calculada de forma perezosa en cada fotograma (el coste es como mucho el lado mayor del lienzo). Se pinta como superposición igual que el lápiz. **El documento no cambia hasta `End`.**
- `End`: `draw_line(doc, image, origen, destino, pixel)`. `Ok(true)` ⇒ **un solo `SetPixels`**, un paso. `Ok(false)` (vacía, fuera del lienzo o sin cambios) ⇒ sin paso, y `can_undo`, `can_redo` e `history_bytes` quedan como estaban. Como la vista previa y `draw_line` usan `Line` y `clip_to`, **coinciden píxel a píxel**.
- **Un clic sin movimiento** (`Begin(p)`, `End`): `Line::new(p, p)` tiene un píxel, así que pinta 1 píxel.
- **Extremos fuera del lienzo:** se recortan con `clip_to`, así que se dibuja la parte visible. Una línea entera fuera es `Ok(false)`.
- **Fin del gesto:** igual que el lápiz, porque lo emite `Canvas`: soltar en cualquier sitio (con su posición como último `Move`), perder el foco y el botón que egui ya no ve pulsado **aplican** la línea hasta el último destino. `PointerGone` no termina.
- **`Escape` cancela:** descarta la línea pendiente, sin paso ni cambios en la historia. El gesto sigue `busy` hasta soltar el botón, y los `Move` y el `End` que lleguen no aplican nada. `Escape` sin una línea en curso (en reposo, con lápiz o con relleno) no hace nada: el lápiz no se cancela, la unidad 6 lo dejó fuera de alcance.
- **Orden dentro de un fotograma:** egui entrega teclas y botones en una sola lista ordenada. `Canvas::interact` ya recorre esa lista y emite `CanvasEvent::Escape` en su sitio, entre los `Move` y el `End`. Así **manda el orden de los eventos**: si `Escape` va **antes** del soltado, cancela; si va **después**, el soltado ya aplicó la línea y `Escape` no hace nada (`Ctrl+Z` la deshace). Si los eventos no llegaran ordenados, gana el soltado (se aplica; la otra opción pierde una línea sin remedio). `px-app` no procesa `Cancel` por la vía de las teclas, para no tratarlo dos veces ni fuera de orden.

### 3. Relleno

- `Begin(p)` ejecuta **`flood_fill(doc, image, p.0, p.1, pixel)` al pulsar**, con el color de la ranura. `Move` y `End` no hacen nada más (el gesto sigue `busy` hasta `End`). Un arrastre con relleno hace un solo relleno, en el punto de pulsación, y **mantener el botón pulsado no lo repite** en cada fotograma: un relleno por pulsación.
- Fuera del lienzo ⇒ `Ok(false)`. Semilla que ya vale el color activo ⇒ `Ok(false)`, **sin paso**. `Ok(true)` ⇒ un solo `FillSpans`, un paso. No se filtra «fuera» ni «igual» aquí: ya lo hace `flood_fill` (aviso de la spec del relleno).
- **Errores.** `flood_fill` no hace pánico y no aborta por falta de memoria: reserva con `try_reserve*` y devuelve `Err(OutOfMemory)`, con el documento intacto. Cualquier `Err` de una herramienta, o de `undo`/`redo`, crea un `Notice` y el título muestra el error **5 s**. Un `Notice` nuevo sustituye al anterior y un éxito no lo borra antes. La caducidad es la función pura `Notice::visible(now)`, que recibe el tiempo como parámetro (se prueba sin ventana). `px-app` toma `now` de `ctx.input(|i| i.time)` y, al crear el `Notice`, llama a `ctx.request_repaint_after(5 s)` para que el título se actualice al caducar. **Cambia la unidad 6**, donde el error duraba hasta la siguiente acción correcta.
- **Alcance real de «no colgarse».** El relleno corre **síncrono en el hilo de la interfaz**. Con regiones enormes puede tardar y la ventana no responde mientras tanto: la spec del relleno mide órdenes de 0,1 s en 2048², y en 16384² (el máximo) serían segundos (extrapolación, no medida).
  **Estimación del relleno uniforme de 16384×16384 (sin medir), extrapolando ×64 los números de la spec del relleno (2048² uniforme en `--release`):** `fill_command` unos **0,9 s** (14 ms × 64) y `apply` unos **4,5 s** (70 ms × 64), es decir, **del orden de 5 s con la ventana bloqueada**, más la regeneración de la textura (`to_rgba8`, 1 GiB). Mucho más en debug (el perfil de test de 2048² ya tarda 0,16 s y 0,49 s). Esa imagen además no se podría mostrar si supera el lado máximo de textura de la GPU (`CanvasError::TooLarge`). **La prueba manual de rellenos grandes se hace en `--release`.** **Un tope de tramos por relleno, o llevarlo a otro hilo, es de la unidad 14.** En Linux, con *overcommit*, `try_reserve` puede tener éxito y luego faltar memoria real (misma nota que en la spec del relleno).
- El `OutOfMemory` real **no se puede provocar en tests** (exige imágenes de GiB): se prueba el camino con la parte pura (`Notice`) y la ausencia de pánico con semillas extremas.

### 4. Textura

- Tras `canvas_event` con `Ok(true)`, o `undo`/`redo` correcto, la app llama a `Canvas::refresh` y `request_repaint`, como en el lápiz. La actualización por regiones es de la unidad 14.

### 5. `screen_to_image_i32`

- **No se retira el parámetro `Size`.** Quitarlo obliga a cambiar las 14 llamadas de `crates/px-ui/tests/view_i32.rs` (unidad 6) y la de `canvas.rs`. Según el encargo, queda anotado para la unidad 14.

## Fuera de alcance

Grosor, línea con Mayús, botón derecho, cancelar el lápiz con `Esc`, barra de herramientas y fuente (unidad 11), relleno con tolerancia y de 8 vecinos (hito 4), actualización por regiones, tope de tramos y relleno en otro hilo (unidad 14).

## Casos límite

Imagen RGBA de 8×8 vacía (todo `A`) salvo que se diga otra cosa, ranura 3 (rojo) salvo que se diga otra cosa. `(x, y)`. Los eventos se dan a mano a `ToolState::canvas_event`, sin ventana. Las líneas están calculadas a mano con `Line` (horizontal, vertical y diagonal exacta no tienen empates).

| # | Entrada | Resultado |
| --- | --- | --- |
| 1 | `tool_action_for_key`: `P`, `L`, `F`, `Escape` sin modificadores | `Select(Pencil)`, `Select(Line)`, `Select(Fill)`, `Cancel` |
| 2 | Las mismas con `SHIFT`, `COMMAND` o `ALT`; y `Z`, `Num1`, `Space`, `Home` sin modificadores; y `action_for_key` con `P`, `L`, `F`, `Escape` | `None` en todas |
| 3 | `status_text`: `(Line, Some("color 3 #ff0000"))`, `(Pencil, None)`, `(Fill, Some("color 9 transparente"))` | `"línea · color 3 #ff0000"`, `"lápiz"`, `"relleno · color 9 transparente"` |
| 4 | Línea: `Begin(1,1)`, `Move(5,1)`, sin `End` | imagen igual a la copia, `can_undo` falso, `preview` = `(1,1)…(5,1)` en rojo (5 píxeles) |
| 5 | Lo anterior más `End` | `Ok(true)`; esos 5 píxeles en rojo; un paso; `undo` ⇒ igual a la copia, `redo` ⇒ igual a la editada |
| 6 | `Begin(0,0)`, `Move(7,0)`, `Move(3,3)`, `End` | solo la diagonal `(0,0)…(3,3)` (4 píxeles): los destinos intermedios no se acumulan |
| 7 | `Begin(2,3)`, `End` | `Ok(true)`, 1 píxel `(2,3)`, un paso |
| 8 | `Begin(-3,2)`, `Move(3,2)`, `End`; y `Begin(-5,-5)`, `Move(-1,-1)`, `End` tras un `undo`; y la 5 repetida sobre píxeles ya rojos | `(0,2)…(3,2)`, un paso; `Ok(false)` sin paso y `can_redo` sigue a `true`; `Ok(false)` sin paso |
| 9 | `Begin(1,1)`, `Move(5,1)`, `apply_action(Cancel)`, `Move(6,1)`, `End` | `Ok(false)` en todo; imagen e historia intactas; `busy` hasta el `End`. `Cancel` en reposo, con lápiz o con relleno: nada (el trazo del lápiz se aplica) |
| 10 | Línea en curso y `Select(Fill)`; y, tras `End`, `Select(Fill)`. Indexado con paleta de 3, ranura 5: `Begin`, `End` | la herramienta sigue siendo `Line`; luego `Fill`. Con la ranura 5: sin cambios, y `busy` es cierto entre `Begin` y `End` |
| 11 | Relleno: `Begin(3,3)`, `Move(0,0)`, `Move(7,7)`, `End` sobre 8×8 uniforme | `Ok(true)` solo en `Begin`; todo el lienzo en rojo; un paso (`FillSpans`); `undo` ⇒ igual a la copia |
| 12 | Relleno: semilla `(-1,0)` y `(8,0)`; semilla ya roja (segundo clic igual, y tras un `undo` previo) | `Ok(false)`; sin paso; `can_redo` sigue a `true` |
| 13 | Imagen de muestra (64×48, `demo_image`): clic en `(0,0)` con ranura 1 (negro); en `(30,22)` con ranura 3; en `(10,10)` con ranura 3 | 220 píxeles (el marco blanco, 94 tramos), 64 (el agujero, 8 tramos) y 1 (cada píxel interior vale distinto); en los tres, igual al oráculo ingenuo del hito 2 (cola, 4 vecinos), un paso y `undo` exacto |
| 14 | Muestra: clic en `(30,22)` con ranura 9 (transparente); clic en `(0,0)` con ranura 2 (blanco) | `Ok(false)` los dos: el valor ya es ese; sin paso |
| 15 | `Notice::new("x".into(), 10.0)`: `visible` en 10,0, 14,9 y 15,0; y un segundo `Notice` | `Some("x")`, `Some("x")`, `None`; el segundo sustituye al primero |
| 16 | `tool_action_for_key` con `P`, `L`, `F` y `Modifiers::CTRL`, `COMMAND`, `MAC_CMD`, y `ctrl` + `command` a la vez (lo que da egui en Linux) | `None` en las doce: `Ctrl`+`P`, `Ctrl`+`L` y `Ctrl`+`F` no cambian de herramienta |
| 17 | Relleno mantenido: `Begin(3,3)` y después 100 `Move` a la misma posición (y, con el banco de egui, el botón abajo durante 30 fotogramas sin otro evento) | un solo `Ok(true)`, un paso; `Canvas` solo emitió `Begin`; ningún relleno más |
| 18 | `Escape` **antes** del soltado: `Begin(1,1)`, `Move(5,1)`, `Escape`, `Move(5,1)`, `End` | `Ok(false)` en `Escape`, `Move` y `End`; imagen e historia intactas; `busy` hasta el `End` |
| 19 | `Escape` **después** del soltado: `Begin(1,1)`, `Move(5,1)`, `Move(5,1)`, `End`, `Escape` | `End` da `Ok(true)` con los 5 píxeles rojos y un paso; `Escape` da `Ok(false)` y no lo deshace (`can_undo` sigue a `true`) |
| 20 | Banco de egui (como `tests/pencil.rs`): trazo abierto y un fotograma con [`Escape`, soltar]; otro con [soltar, `Escape`]; `Shift`+`Escape`; `Escape` sin trazo | `[Escape, Move, End]`; `[Move, End, Escape]`; sin evento de `Escape`; `[Escape]` (y `ToolState` no hace nada con él) |
| 21 | `Canvas` + `ToolState` con el banco de egui: trazo de línea y `WindowFocused(false)`; otro con `PointerGone` y soltar fuera de la ventana; otro con el soltar perdido; y en indexado con paleta de 3, ranura 5: pulsar y soltar | `busy()` es falso después de cada uno, sin gesto colgado y sin cambios en el último; tras el último, `Select(Fill)` cambia de herramienta |
| 22 | `window_title(Line, info)` con `info` completo; con `color: None`; con `can_undo` y `can_redo`; con `error` | `Pixoko: 64×48 · ×8 · (3, 4) · línea · color 3 #ff0000 · deshacer · rehacer`; `… · ×8 · lápiz` (solo la herramienta, para `Pencil`); `deshacer` y `rehacer` solo si se puede; con error, `Pixoko: {error}` y nada más |

## Propiedades (proptest)

Imágenes de 1 a 16 de lado, en los cuatro modos; posiciones `i32` arbitrarias con sesgo hacia dentro y los bordes.

1. **Previsualización = aplicado = `draw_line`.** Para cualquier `a`, `b`: el conjunto de `preview` tras `Begin(a)`, `Move(b)` es igual al de `line_command(a, b)` y, tras `End`, la imagen es la original con esos píxeles en `pixel` y nada más. El documento no cambia en ningún evento antes del `End`.
2. **Un gesto, un paso.** Con secuencias de gestos de cualquier herramienta: cada gesto añade como mucho un paso, y `Ok(true)` si y solo si añadió uno. Tras `k` gestos con `Ok(true)`, `k` `undo` dejan la imagen **exactamente** igual que al principio y `k` `redo` igual que al final.
3. **Cancelar.** Con `Cancel` en cualquier punto de una línea, antes del `End`: imagen, `can_undo`, `can_redo` e `history_bytes` iguales; los eventos posteriores no aplican nada; `busy` hasta el `End`. Con `Cancel` después del `End`, nada de lo aplicado cambia.
4. **Relleno contra el oráculo.** Sobre imágenes con fondo de 2 a 4 valores (incluidos casi duplicados), un clic da el mismo resultado que el recorrido ingenuo con cola: `Ok(true)` si y solo si la semilla está dentro y su valor es distinto del activo. Con cualquier semilla `i32`, sin pánico y con la historia intacta si no hay cambio.
5. **Teclas.** `tool_action_for_key` sobre `Key::ALL` por las 32 combinaciones de modificadores: sin pánico, y `Select` o `Cancel` solo sin modificadores y solo con `P`, `L`, `F` y `Escape`; `action_for_key` da `None` para esas cuatro.
6. **Herramienta estable.** Con cualquier secuencia de eventos y acciones: `Select` no cambia `tool()` si `busy()`, y la cambia si no; `busy()` es cierto exactamente entre un `Begin` y su `End`, y `preview` está vacío en reposo.

## Dónde van las pruebas

`px-ui/tests/tool.rs` (filas 1 a 12, 15, 16, 18 y 19 y la primera parte de la 17, propiedades 1 a 6), `px-ui/tests/tool_canvas.rs` (filas 20 y 21 y la segunda parte de la 17, con el banco de egui de `tests/pencil.rs`, copiado y no modificado) y un módulo nuevo de tests en `px-app/src/title.rs` (fila 22; no se tocan los tests que ya tiene). Las filas 13 y 14 van en el módulo de tests de `px-app/src/demo.rs`, porque la imagen de muestra vive en el binario, con el oráculo escrito en el propio test. No se tocan los tests de las unidades anteriores.

## Decisiones que necesito que confirmes

1. **Teclas `P`, `L`, `F` y `Escape` sin modificadores**, y la función `tool_action_for_key` aparte de `action_for_key` (para no tocar la propiedad 6 de la unidad 6). Sin probar en teclado físico no QWERTY.
2. **El relleno se ejecuta al pulsar**, no al soltar; un arrastre no hace nada más. Alternativa: al soltar en el punto de pulsación.
3. **`Escape` solo cancela la línea** (el lápiz no cambia), y si coincide en el mismo fotograma con el soltado, **manda el orden de los eventos** (antes cancela, después gana el soltado). Para eso `Canvas` emite `CanvasEvent::Escape`, un cambio aditivo.
4. **Error en el título 5 s** y un éxito no lo borra antes (cambia la regla de la unidad 6: «hasta la siguiente acción correcta»).
5. **El relleno es síncrono:** puede bloquear la ventana con regiones enormes; el tope de tramos o el hilo aparte son de la unidad 14. No puedo prometer que «no se cuelgue», solo que no hace pánico ni aborta por memoria.
6. **El título completo sale de `window_title`**, que mete la herramienta dentro del campo `color` (`status_text`) en vez de añadir un campo, para no tocar 13 literales de la unidad 6. Un campo propio, para la unidad 14.
7. **`screen_to_image_i32` conserva `Size`:** quitarlo toca 14 llamadas de `view_i32.rs` (unidad 6), así que queda para la unidad 14.
8. **`busy()` va de `Begin` a `End` aunque no haya gesto real** (cambio mínimo respecto a la unidad 6 en el caso de ranura sin color).
9. **La lógica de herramientas se mueve de `main.rs` a `px-ui`** (`ToolState`, sin tipos de egui en su API), sin cambiar `Stroke`; `Canvas` solo gana `CanvasEvent::Escape`.

## Para la unidad 14

- Tope de tramos por relleno, o relleno fuera del hilo de la interfaz.
- Quitar `Size` de `View::screen_to_image_i32` (tocar `tests/view_i32.rs` y `canvas.rs`).
- Un campo `tool` en `TitleInfo` en vez de `status_text` dentro de `color`.
- Actualización de la textura por regiones tras línea y relleno.
