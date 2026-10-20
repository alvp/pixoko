# Interfaz · Unidad I.2b: navegación y botones del ratón como tabla de datos

Estado: **aprobada** (2026-10-21; decisiones 1 a 8 como están, con los matices del visto bueno del final de la página, que **prevalecen sobre cualquier otra frase de ella**). Cambios previstos: `px-ui` y `px-app`. **No se toca `px-core` ni `px-raster`.** Sin dependencias nuevas ni `unsafe`; `docs/DEPENDENCIES.md` no cambia. Sin pantalla de remapeo ni guardado en disco (hito 8).

Referencias: «Rueda, botón central, botones extra y gestos» y «Soporte mínimo» en `docs/DISENO-INTERFAZ.md`. Tabla de teclas, `Press`, `Platforms`, `Origin` y `is_blocked`: [`interfaz-teclado.md`](interfaz-teclado.md). `Role`, `interact_roles` y el botón derecho: [`interfaz-dos-colores.md`](interfaz-dos-colores.md). Escalera de zoom (1/64 a ×64, `zoom_in`/`zoom_out` de un paso) y rueda anclada: [`hito-2-zoom-baldosas.md`](hito-2-zoom-baldosas.md).

## Objetivo

Una **tabla de datos** (`GestureMap`, sin egui, probada sin ventana) que asigna entradas del ratón y del trackpad a acciones, con el esquema por defecto del diseño; el `Canvas` y la ventana la leen en vez de tener los botones escritos en el código. Como `Keymap`, queda lista para que el hito 8 la sustituya entera.

## Lo comprobado (y lo que no)

- **Botones.** `egui::PointerButton` tiene cinco valores: `Primary`, `Secondary`, `Middle`, `Extra1`, `Extra2`. `egui-winit` 0.37 (`translate_mouse_button`) traduce `Back`→`Extra1`, `Forward`→`Extra2` y **`Other(_)`→nada: el evento muere antes de llegar a egui**; `eframe` 0.37 solo da `raw_input_hook` sobre el `RawInput` ya traducido. `winit` 0.30.13 llama `Back`/`Forward` a: X11, botones 8 y 9; Wayland, `BTN_BACK`/`BTN_SIDE` y `BTN_FORWARD`/`BTN_EXTRA`; Windows, `XBUTTON1`/`2`; macOS, 3 y 4. Todo lo demás es `Other`. **Los botones 6 en adelante no son alcanzables sin salir de `eframe`** (decisión 1).
- **Rueda frente a trackpad.** `Event::MouseWheel { unit: Line | Point | Page, delta, phase, source, modifiers }`. Según el código de `winit`: macOS, rueda → `Line` y trackpad → `Point`; Wayland, rueda → `Line` y dos dedos → `Point`; **X11, siempre `Line`** (los dos dedos llegan como líneas fraccionarias); Windows, `Line` (`WM_MOUSEWHEEL`/120; el touchpad de precisión también, por lo que sé: no verificado). `MouseWheelSource` solo dice algo en macOS (`Wheel`, `Trackpad`, `Momentum`); en el resto es `Unknown`. **«Líneas hacen zoom y píxeles desplazan» (diseño) solo se cumple en macOS y Wayland** (decisión 2).
- **Pellizco.** `Event::Zoom` solo sale de `WindowEvent::PinchGesture`, que `winit` 0.30.13 solo emite en **macOS e iOS** (igual `PanGesture`, que `egui-winit` ya convierte en `MouseWheel` de `Point`). **En Linux el pellizco no llega.** `InputState::zoom_delta()` es el producto de `Event::Zoom` y de **Ctrl/Cmd + rueda**, que egui sintetiza como `exp(Δ/200)` sobre el delta que suaviza durante ~8 fotogramas (`wheel_state.rs`).
- **Medido hoy (2026-10-21, banco de egui sin ventana, `Context::run_ui` a 60 fps, test desechable ya borrado; imagen 64×48, ajuste = ×12):**

  | Entrada | Pasos de zoom hoy |
  | --- | --- |
  | 1 muesca (`Line` 1,0) | 1 |
  | `Line` 3,0 de golpe | 1 |
  | **Ctrl + 1 muesca** | **11** (×12 → ×23 en 10 fotogramas) |
  | **`Zoom(1.02)` en 30 fotogramas seguidos** (0,5 s, un pellizco lento) | **30** (×12 → ×42) |
  | **30 fotogramas de `Point(0, 6)`** (trackpad, píxeles) | **30**: hoy el trackpad hace zoom por el signo |
  | 30 fotogramas de `Line(0, 0,1)` (touchpad en X11) | 30 |
  | `Mayús` + muesca | 1 (zoom; `Mayús` no se mira) |

  No medido: ningún ratón con botones laterales, ningún trackpad, ningún pellizco real; lo hará la prueba manual donde se pueda.

## Qué hace hoy el código (`canvas.rs`, `interact_inner`) y qué cambia

| Gesto | Hoy | Cambia |
| --- | --- | --- |
| Rueda | Cada `MouseWheel` con `delta.y ≠ 0` y sin `command` da **un** paso por el signo, sea `Line`, `Point` o `Page`, anclado al cursor | `Line`/`Page` → `Wheel` (pasos por acumulador); `Point` → `Scroll` (desplaza). `Mayús` + `Wheel` desplaza en horizontal |
| Pellizco y Ctrl + rueda | `zoom_delta` ≠ 1 → un paso **por fotograma** | Acumulador en logaritmo con umbral (decisión 3) |
| Botón central | `Gesture::Pan(Middle)` | Igual, ahora por la tabla |
| Espacio + izquierdo | `Gesture::Pan(Primary)` si `Espacio` está abajo **al pulsar** | Igual por la tabla; **Espacio pulsado con un trazo en curso lo cancela** (hoy no hace nada) |
| Izquierdo / derecho | `Draw(Foreground)` / `Draw(Background)` | Igual por la tabla |
| `Extra1`, `Extra2` | `_ => Gesture::None` (nada) | **Deshacer** y **rehacer**, en toda la ventana (la ventana, no el lienzo) |

## Decisiones (⚠ = cambia o añade algo a lo que pediste)

1. ⚠ **Forma de la tabla** (módulo `gestures`, **sin egui**, como `keymap`): `GestureMap` es un `Vec<GestureRow>` de datos puros.
   - `Button { Left, Middle, Right, Back, Forward }` (los cinco que llegan; ⚠ **«todos los botones extra» se queda en los dos primeros**: del 6 en adelante no existen para la app y la salida es reasignarlos en el sistema a `Back`/`Forward` o a teclas; no se promete más).
   - `Input { Press(Button), Wheel, Scroll, Pinch }`: `Wheel` = muescas (`Line`, `Page`); `Scroll` = píxeles (`Point`); `Pinch` = `zoom_delta` (pellizco y Ctrl/Cmd + rueda).
   - `Mods { command: Option<bool>, shift: Option<bool>, space: Option<bool> }`: `None` = no cuenta, `Some(v)` = debe ser exactamente `v`. **`Alt` no existe en la tabla** (nunca cuenta; decisión 5). `command` es `Ctrl` (Linux, Windows) o `Cmd` (macOS), como en `Press`.
   - `Scope { Canvas, Window }`: `Canvas` exige el cursor sobre el lienzo y ninguna otra capa encima (la misma regla `on_canvas` de hoy); `Window` vale en cualquier sitio.
   - `GestureAction { Draw(Role), Pan, PanX, ZoomStep, App(AppAction) }`. `App` admite **cualquier** `AppAction` de la tabla de teclado (deshacer, zoom, herramienta…), que es lo que pide el soporte mínimo. Combinaciones **inválidas** (`conflicts()` las marca `Invalid`): `Press` con `ZoomStep` o `PanX`; `Wheel`, `Scroll` y `Pinch` con `Draw` o `App`; `Pinch` con `Pan` o `PanX`.
   - `Platforms` (el de `keymap`) por fila. Hoy todas `All`: se deja para el hito 8.
   - **Condiciones: solo `scope` y los tres modificadores van en la tabla.** Un gesto de herramienta en curso, un diálogo, el panel modal y el campo de texto enfocado los sigue resolviendo `is_blocked` por acción, como en I.1: ponerlos en cada fila haría que el menú, la tecla y el botón lateral se bloquearan distinto. Las acciones de vista (zoom y desplazar) **no** son `AppAction` (salvo `ZoomIn`/`ZoomOut` si se asignan a un botón) y siguen permitidas con el botón pulsado, como hoy.
   - **Resolución:** `GestureMap::resolve(&Pointing, over_canvas: bool, Platform) -> Option<GestureAction>` con `Pointing { input, command, shift, space }`. Dos filas chocan si tienen la misma entrada, ámbitos que se solapan, plataformas que se solapan y los tres modificadores **compatibles** (`None` casa con todo); `conflicts() -> Vec<Conflict { Duplicate(usize, usize), Invalid(usize) }` es el validador del hito 8, y la tabla por defecto no tiene ninguno. Sin choques, el resultado no depende del orden de las filas.
   - **Entrada (módulo `pointer_events`, `egui::Event` como dato, sin `Context`):** `button_of(PointerButton) -> Button`, `wheel_of(unit, delta, modifiers) -> (Input, f32, f32)` (`Line`/`Page` → `Wheel`; `Point` → `Scroll`; con `command`, ni `Wheel` ni `Scroll`: ese gesto es de `Pinch`, que egui ya sintetiza y no se cuenta dos veces) y `pointer_presses(&[Event]) -> Vec<(Button, command, shift)>` para la ventana.
2. ⚠ **Rueda y trackpad: se separa por la unidad y se dice dónde no basta.** Sin heurísticas de magnitud (un ratón de rueda libre y un touchpad en X11 se parecen demasiado). En macOS y Wayland el trackpad desplaza; **en X11 y Windows los dos dedos llegan como `Line` fraccionarios y harían zoom lento** en vez de desplazar (por el acumulador: sin carrera, pero zoom). Es el límite real de `winit` 0.30.13, no de la tabla: quien lo sufra reasigna `Wheel` a `Pan` y `Ctrl`+`Wheel` a zoom en el hito 8. La prueba manual dice qué manda **tu** dispositivo.
3. ⚠ **Zoom por pasos con acumulador** (función pura `StepAccumulator { acc, threshold }`; `push(amount) -> i32` con signo, `reset()`):
   - **Rueda (`Wheel`)**: `amount = delta.y` en muescas, umbral **1,0**, **por evento** (no por fotograma: una muesca = un paso como hoy; 200 eventos en un fotograma = 200 pasos, que es lo que ya prueba `el_zoom_se_detiene…`). `Line` 3,0 da **3** pasos (hoy 1: es la cantidad real de muescas); el resto fraccionario se arrastra (`0,4+0,4+0,4` → un paso en el tercero) y **se descarta al cambiar de signo** y en el primer fotograma sin eventos de rueda (la inercia de una rueda libre sigue dando pasos mientras llegan eventos; no los inventa después).
   - **Pellizco (`Pinch`)**: `amount = ln(zoom_delta)` una vez por fotograma, umbral **0,15** (×1,16: una muesca con Ctrl, `ln` total 0,2, da **un** paso; hoy 11). Los 30 fotogramas de `Zoom(1.02)` de arriba dan **3** pasos (hoy 30), `Zoom(1.3)` uno, `Zoom(1.2)` uno, `Zoom(0.7)` dos hacia fuera. El resto se arrastra dentro del gesto y se descarta en el primer fotograma con `zoom_delta` = 1 (±1e-3). Cada paso sigue siendo `zoom_in`/`zoom_out` anclado al cursor. **El umbral 0,15 es una cifra que no puedo medir con un pellizco real**: constante `PINCH_STEP_LN` en un solo sitio, la ajusta la prueba manual.
   - **Desplazamiento (`Scroll`, `PanX`)**: los puntos se pasan a píxeles físicos con `ppp` y los **restos de medio píxel se arrastran** (con `Point(0, 0,4)` a `ppp` 1 el redondeo a ojo se comería el gesto entero); sentido: **el contenido sigue a los dedos** (delta positivo en `y` = la imagen baja), el mismo que `egui` documenta. `Wheel` con `PanX` (`Mayús` + rueda): **`dx = delta.x + delta.y`** (macOS ya intercambia los ejes con `Mayús`), 40 puntos por muesca (`line_scroll_speed` de `egui` nativo), una página = el ancho del lienzo; muesca hacia arriba = el contenido va a la derecha. Siempre `view.pan` y `clamp`, también con un trazo en curso.
4. **Espacio** (⚠ **solo cancela trazos, no diálogos**): con un gesto `Draw` en curso, la pulsación de `Espacio` (**no** su repetición, y con el campo de texto sin foco) emite lo mismo que `Escape` (`CanvasInput::Escape`, mismo orden y mismo efecto: el trazo no deja paso de historia). Sin gesto, `Espacio` no hace nada y es la tecla de la mano (`Left` con `space = Some(true)` → `Pan`, como hoy: se mira al pulsar). Un `Pan` en curso **no** se cancela con `Espacio` (no hay nada que deshacer). **Diálogos:** no. En un diálogo `Espacio` activa el botón enfocado o marca la casilla (hay pruebas de eso, `new_image` y `convert_mode`), y un diálogo no puede coexistir con un gesto (`can_open_dialog` lo impide); cancelar el diálogo con `Espacio` rompería esas pruebas y el uso normal. `Escape` ya lo cierra. **`Espacio` sigue sin ser asignable** (en la lista reservada de `Keymap`); no es una fila de `GestureMap` sino el modificador `space`.
5. **Conflictos.** *Botón derecho:* la fila `Right → Draw(Background)` es la del esquema de dos colores; no abre menú contextual en el lienzo ni desplaza, y `interact_roles` no cambia su comportamiento (las dos pruebas del secundario de `pencil.rs` siguen valiendo). Con `Right` los tres modificadores valen `None`: **`Espacio` + derecho sigue pintando con el fondo**, como hoy. *KDE:* la tabla **nunca mira `Alt`** (ni lo exige ni lo excluye), por la misma razón que el teclado (decisión 1 de I.1): KWin se queda `Alt` + arrastrar (mover y redimensionar ventana) antes de que llegue nada a la app; por defecto es `Alt`, y en versiones recientes puede ser `Meta` (**no comprobado en tu equipo**). No uso `Meta`/`Super` en ninguna fila.
6. ⚠ **Botones laterales: toda la ventana, con `Origin::Button`.** Se resuelven en `px-app` junto a `key_actions` (nueva `pointer_actions`), no en el lienzo, para que valgan sobre la paleta, el menú o la barra. Origen `Button` (sin variante nueva en `Origin`): es la regla de «clic en un botón de la columna»: bloqueados con **un gesto en curso** (no se deshace a mitad de trazo), diálogo, panel modal o campo de texto enfocado (no se deshace el documento mientras se escribe un hexadecimal), y sin historia no hacen nada. `Deshacer` por `Back` y por `Ctrl+Z` dan el mismo `Dispatched` y el mismo estado. No repiten (solo `pressed: true`).
7. ⚠ **Cambios de comportamiento ya medidos** (arriba): el trackpad en píxeles **deja de hacer zoom** y desplaza; `Mayús` + rueda desplaza en vez de acercar; Ctrl + muesca pasa de 11 pasos a 1; el pellizco deja de ser un paso por fotograma.
8. **Fuera de esta unidad (aviso):** clic derecho en una herramienta y en los botones de zoom, que el `INDEX` y `PLAN-RESUMEN` metían en I.2b: no estaban en tu encargo, el primero necesita el panel de opciones (I.3) y el segundo toca la columna; los dejo para I.3 y **corrijo esas dos líneas** al aprobar. `interact` e `interact_ordered` **se quedan** (siguen ignorando el derecho; su `TODO` pasa de `I.2b` a `hito 8`).

## Esquema por defecto (10 filas)

`–` = no cuenta. Todas con `Platforms::All`.

| # | Entrada | `command` | `shift` | `space` | Acción | Ámbito |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | `Press(Left)` | – | – | no | `Draw(Foreground)` | Canvas |
| 2 | `Press(Left)` | – | – | sí | `Pan` | Canvas |
| 3 | `Press(Middle)` | – | – | – | `Pan` | Canvas |
| 4 | `Press(Right)` | – | – | – | `Draw(Background)` | Canvas |
| 5 | `Wheel` | no | no | – | `ZoomStep` | Canvas |
| 6 | `Wheel` | no | sí | – | `PanX` | Canvas |
| 7 | `Scroll` | no | – | – | `Pan` | Canvas |
| 8 | `Pinch` | – | – | – | `ZoomStep` | Canvas |
| 9 | `Press(Back)` | – | – | – | `App(Undo)` | Window |
| 10 | `Press(Forward)` | – | – | – | `App(Redo)` | Window |

Sin fila: `Wheel` con `command` (es de `Pinch`), `Scroll` con `command`, y todo `Press` de `Back`/`Forward` sobre un diálogo (lo bloquea `is_blocked`, no la tabla).

## Piezas (firmas)

```rust
// px-ui/src/gestures.rs (nuevo, sin egui; Binding-like: Clone + PartialEq + Debug, sin serde todavía)
pub enum Button { Left, Middle, Right, Back, Forward }
pub enum Input { Press(Button), Wheel, Scroll, Pinch }
pub struct Mods { pub command: Option<bool>, pub shift: Option<bool>, pub space: Option<bool> }
pub enum Scope { Canvas, Window }
pub enum GestureAction { Draw(Role), Pan, PanX, ZoomStep, App(AppAction) }
pub struct GestureRow { pub input: Input, pub mods: Mods, pub action: GestureAction,
                        pub scope: Scope, pub active: Platforms }
pub struct Pointing { pub input: Input, pub command: bool, pub shift: bool, pub space: bool }
pub enum Conflict { Duplicate(usize, usize), Invalid(usize) }
pub struct GestureMap { /* Vec<GestureRow> */ }
impl GestureMap {
    pub fn default_scheme() -> GestureMap;                    // las 10 filas
    pub fn new(rows: Vec<GestureRow>) -> GestureMap;          // sustitución entera (hito 8)
    pub fn rows(&self) -> &[GestureRow];
    pub fn resolve(&self, p: &Pointing, over_canvas: bool, platform: Platform) -> Option<GestureAction>;
    pub fn conflicts(&self) -> Vec<Conflict>;
}
// Las tres constantes viven JUNTAS en `gestures`, con un comentario que dice que se ajustan tras la prueba manual:
pub const PINCH_STEP_LN: f32 = 0.15;  pub const WHEEL_STEP: f32 = 1.0;  pub const PAN_LINE_POINTS: f32 = 40.0;
pub struct StepAccumulator { /* acc, threshold */ }          // push(amount) -> i32, reset(); descarta al cambiar de signo
pub struct PanCarry { /* resto en px (x, y) */ }             // add(dx_pt, dy_pt, ppp) -> (i64, i64) con arrastre
// px-ui/src/pointer_events.rs (nuevo; egui::Event como dato)
pub fn button_of(PointerButton) -> Button;
pub fn wheel_of(MouseWheelUnit, Vec2, Modifiers) -> Option<(Input, f32, f32)>;   // None con command
pub fn pointer_presses(&[Event]) -> Vec<(Button, bool /*command*/, bool /*shift*/)>;
// px-ui/src/canvas.rs: Canvas { gestures: GestureMap, wheel: StepAccumulator, pinch: StepAccumulator, carry: PanCarry };
//   Canvas::set_gestures(GestureMap); interact_inner lee la tabla (Draw, Pan, ZoomStep, PanX) y emite Escape con Espacio.
// px-app: App { gestures: GestureMap }; pointer_actions(events, &GestureMap, Platform, &mut Vec<(AppAction, Origin)>)
//   junto a key_actions; Origin::Button.
```

## Casos (sin ventana; 5 a 7 y 10 a 12 usan el banco de egui de `tests/canvas.rs` y `pencil.rs`; el 8, `Session` + `dispatch`)

`Pointing` = `P(input, command, shift, space)`. «Las tres plataformas» = `Linux`, `Windows`, `Mac`.

| # | Entrada | Esperado |
| --- | --- | --- |
| 1 | `GestureMap::default_scheme()` por plataforma | Las 10 filas de arriba en ese orden; 10 activas en las tres; `conflicts()` vacío; ninguna fila lleva `Alt` (el tipo no lo tiene) |
| 2 | **Resolver.** `P(Press(b), …)` con los 5 botones × `space` × `shift` × `command`, sobre y fuera del lienzo; `Wheel`, `Scroll` y `Pinch` con las ocho combinaciones | Izquierdo sin `space` → `Draw(Foreground)`, con `space` → `Pan`; central → `Pan`; derecho, con y sin `space` → `Draw(Background)`; `Back` → `App(Undo)` y `Forward` → `App(Redo)` **también fuera del lienzo**; las filas `Canvas` fuera del lienzo → `None`; `Wheel`: sin `shift` `ZoomStep`, con `shift` `PanX`, con `command` `None`; `Scroll` con `command` `None`; `Pinch` siempre `ZoomStep` |
| 3 | **Eventos de egui a entrada** (`pointer_events`, sin `Context`): los 5 `PointerButton`; `MouseWheel` con `Line`, `Page` y `Point`, con y sin `command` y `shift`; `Zoom` | `Primary/Secondary/Middle/Extra1/Extra2` → `Left/Right/Middle/Back/Forward`; `Line` y `Page` → `Wheel`, `Point` → `Scroll`; con `command` → `None`; `pointer_presses` da una por `PointerButton` pulsado (nunca por soltado) y conserva `command` y `shift`; `Other(_)` **no existe** en egui y queda documentado, sin prueba |
| 4 | **Acumulador.** `StepAccumulator` de rueda (1,0) con `1,0`; `3,0`; `0,4+0,4+0,4`; `+0,6` y `−0,6`; `reset` entre dos; de pellizco (0,15) con `ln(1,02)×30`, `ln(1,3)`, `ln(1,2)`, `ln(0,7)`, el total de una muesca con Ctrl (0,2 repartido en 8) | Rueda: 1, 3, 1 (en el tercero), 0 y 0 (cambio de signo descarta), el resto no cruza un `reset`; pellizco: 3, 1, 1, −2, 1; nunca más de lo que dice `⌊Σ/umbral⌋`; sin pánico con `NaN` ni infinitos (se ignoran) |
| 5 | **Rueda en el lienzo.** `Line` ±1 y `Line` 3,0 con el cursor sobre la imagen; `Point(0, 6)` ×30; `Mayús`+`Line`; Ctrl+`Line`; `Event::Zoom(1.02)` ×30; cursor fuera del lienzo y bajo una capa (el área de `canvas_layers`); 200 `Line` en un fotograma | Un paso anclado (el píxel bajo el cursor se conserva, como hoy), 3 pasos, **ningún paso de zoom** y la imagen se desplaza (`Point`), desplazamiento horizontal sin zoom (`Mayús`), **1 paso** (Ctrl), **3 pasos** (pellizco), nada fuera o bajo una capa, y la escalera llega a ×64 y a 1/64 como hoy |
| 6 | **Desplazar.** Botón central; `Espacio`+izquierdo (se mira al pulsar); `Scroll` de dedos con y sin trazo en curso; `clamp` en el borde | Se mueve la vista y no hay evento de trazo; con un trazo a medias la vista se mueve y el trazo sigue; el desplazamiento es siempre un entero de píxeles físicos y `clamp` manda |
| 7 | **Espacio.** `Espacio` con un trazo, una línea o un relleno en curso; sin gesto; repetición (`repeat: true`); con un `Pan` en curso; con campo enfocado; con un diálogo abierto (banco de `new_image`/`convert_mode`) | Con trazo: la misma `CanvasInput::Escape` que `Escape` (el trazo no deja paso y `undo` no cambia); sin gesto, repetición, `Pan` en curso y campo enfocado: nada; el diálogo: `Espacio` sigue activando el botón enfocado y no cancela nada |
| 8 | **Botones laterales** (`Session` + `dispatch`, eventos sintéticos `PointerButton { Extra1/Extra2 }` al nivel al que llegarían desde el hardware): `Back` y `Forward` con historia, sin historia, con un trazo en curso, con diálogo, con modal, con campo enfocado, sobre la paleta; `Back` frente a `Ctrl+Z`; un `Extra1` soltado | `Back` → `Undo` y `Forward` → `Redo` con `Origin::Button`; `Blocked` con gesto, diálogo, modal y campo; sin historia, nada; mismo `Dispatched` y mismo estado que `Ctrl+Z`; soltar no hace nada; **tabla sustituida** (`Back` → `App(ZoomIn)`, `Wheel` ↔ `Pan`, tabla vacía) cambia lo que pasa y con la vacía no responde ningún botón ni rueda |
| 9 | **Reasignar y validar.** `GestureMap::new` con `Back` → `App(ColorNext)`, con filas repetidas con modificadores compatibles (`None` y `Some(false)`), y con combinaciones inválidas | `Back` → `ColorNext`; `Duplicate` para las filas que chocan y **no** para las que se excluyen (`space: Some(true)` y `Some(false)`); `Invalid` para `Wheel`→`Draw`, `Press`→`ZoomStep`, `Pinch`→`PanX` y `Scroll`→`App`; `default_scheme()` ninguno |
| 10 | **Botón derecho y `Alt`.** `Right` con y sin `Espacio`, con `Alt` en cada gesto (el modificador viaja en el evento de egui), `interact`/`interact_ordered` con el derecho; `Mayús`+izquierdo | Derecho → `Draw(Background)` con y sin `Espacio` (como hoy); `Alt` no cambia ningún resultado; `Mayús`+izquierdo sigue dibujando; `interact` e `interact_ordered` siguen sin emitir nada con el derecho |
| 11 | **Sentido del desplazamiento.** `Scroll` con `delta.y` > 0 y < 0, y con `delta.x`; `Mayús` + `Line` con `delta.y` > 0 y con `delta.x` (macOS ya intercambia los ejes); el mismo gesto con el desplazamiento natural de macOS (el sistema ya invierte el signo del delta que entrega: dos eventos con signos opuestos para el mismo movimiento de dedos) | El signo del delta **lo da el sistema y la app no lo invierte**: delta positivo en `y` = la imagen baja, en `x` = va a la derecha; `Mayús` + `Line`: `dx = delta.x + delta.y`; con signos opuestos, movimientos opuestos, sin ninguna rama por plataforma ni por `source` |
| 12 | **Restos de medio píxel.** `Scroll` de `0,4` pt repetido a `ppp` 1,0, 1,5 y 2,0, y de `−0,4`; un `Scroll` con resto que cambia de eje | Tras 7 eventos de 0,4 pt a `ppp` 1 la vista se ha movido 2 px (2,8 px: no 0 ni 3; a 1,5 y 2,0, 4 y 5 px); a otros `ppp`, la suma de lo movido es el entero más cercano por debajo del total (sin perder ni inventar píxeles); el resto se arrastra por eje y con signo; siempre enteros físicos |

## Propiedades (proptest; 256 casos, 500 como máximo)

1. **Sin choques, determinista y alcanzable.** Para cada plataforma la tabla por defecto no tiene `conflicts()`; para cualquier `Pointing` y `over_canvas` el resultado de `resolve` es el mismo con las filas en orden inverso y coincide con un oráculo escrito a mano (la fila cuya entrada, ámbito y modificadores casan); y para cada fila hay un `Pointing` construido desde ella que da **su** acción.
2. **Acumulador.** Para cualquier secuencia de cantidades finitas con un solo signo y sin `reset`: la suma de pasos por el umbral no supera la suma de cantidades y el resto `|acc|` es menor que el umbral; con cambio de signo o `reset` el resto es 0; nunca hay un paso sin haber cruzado el umbral; sin pánico con cualquier `f32` (los no finitos se ignoran).
3. **Navegar no dibuja y la vista siempre vale.** Para cualquier secuencia de eventos de rueda (`Line`, `Point`, `Page`; con `command`, `shift`), `Zoom`, `Espacio`, botón central y `Extra1`/`Extra2`, sin izquierdo ni derecho: no se emite ningún evento de trazo, el documento no cambia, el zoom está en la escalera y el rectángulo de la imagen sigue solapando el lienzo.

## Pruebas anteriores que cambian (por lectura y por la medida de arriba, **no medidas en copia desechable**; modo ahorro)

- **Cambia la intención (más que literales: me paro aquí y espero tu respuesta a la decisión 4 antes de adaptarla):** `px-ui/tests/pencil.rs::fila_15_pulsar_espacio_a_mitad_de_un_trazo_no_desplaza_y_el_trazo_sigue`. Hoy afirma que `Espacio` a mitad de trazo **no** lo cancela (el trazo sigue y la vista no se mueve); con la decisión 4 **lo cancela**. Propuesta: pasa a `…_cancela_el_trazo` con las mismas posiciones (el trazo no deja píxeles ni paso; la vista no se mueve), y la mitad «la vista no se mueve» se conserva.
- **No cambian, y por qué (leído):** `canvas.rs::la_rueda_cambia_el_zoom_…` y `el_zoom_se_detiene_…` (eventos `Line` ±1, el acumulador va por evento: 200 en un fotograma = 200 pasos); `zoom_canvas.rs::fila_a6_*`; `canvas_layers.rs` (filas 5b y 9b: `Zoom(1.3)` da un paso, `Zoom(1.3)`/`0.7` bajo el área siguen bloqueados por la capa); `view_ops.rs:496` con `Zoom(1.2)` (ln 0,182 ≥ 0,15: un paso; margen estrecho, lo fija el caso 4); `pencil.rs` filas 15 de botón central y `Espacio`+izquierdo; `canvas_roles.rs`; las dos pruebas del secundario; `tiles_canvas.rs` (botón central); los diálogos (`Espacio`). No encontré (con `grep` de `Modifiers::SHIFT`, `MouseWheelUnit::Point` y `Extra`) ninguna prueba del lienzo que use `Mayús`+rueda, rueda en píxeles o los botones laterales; `rueda_puntos` de `tests/common` la usan `palette_panel.rs` y `window_stack.rs` (fila 10: con el puntero sobre el panel, no sobre el lienzo, que no debe cambiar). Es lectura, no medida: si al compilar alguna falla, la lista se corrige aquí.
- **Si al escribir las pruebas nuevas alguna otra necesita más que firmas, literales o cifras, paro y aviso.**

## Dónde van las pruebas y orden de trabajo

`px-ui/tests/gestures.rs` (filas 1 a 4 y 9; propiedades 1 y 2), `px-ui/tests/navegacion.rs` (filas 5, 6, 10 a 12; propiedad 3; banco copiado de `canvas.rs`) y `pencil.rs` ampliado con el módulo `espacio` (fila 7), `px-app/src/dispatch/tests.rs` ampliado (fila 8). Sin `test-writer` (unidad de interfaz): una prueba por fila y por propiedad (tope orientativo de 40). **Un commit por paso:** (1) esta spec con su fila del INDEX; (2) adaptación de la prueba anterior (solo tras tu respuesta a la decisión 4); (3) pruebas nuevas, en rojo, escritas por la sesión principal; (4) lógica de `px-ui` (`gestures`, `pointer_events`, acumuladores, `Canvas` leyendo la tabla, `Espacio`); (5) `px-app` (`pointer_actions`, `App.gestures`). Cierre: `cargo fmt`, `clippy -D warnings`, `cargo test --workspace --locked`, `cargo deny check`; sin `license-auditor` (sin dependencias nuevas ni cambios en `px-core`, `px-formats` ni `px-raster`). Al terminar se actualizan la columna «Hoy» de `docs/DISENO-INTERFAZ.md` y `PLAN-RESUMEN.md`.

## Qué se puede probar sin ventana y qué no

- **Sin ventana (todo lo de arriba):** la tabla, la traducción de eventos, los acumuladores, el lienzo con `RawInput` sintético a 60 fps (la rueda con Ctrl y el pellizco ya se miden así) y los botones laterales **como `Event::PointerButton { Extra1/Extra2 }`**, que es exactamente lo que egui entrega.
- **Sin verificar con hardware (y no lo voy a afirmar):** que tu ratón mande `Back`/`Forward` (X11: 8 y 9; Wayland: `BTN_SIDE`/`BTN_EXTRA`); que tu trackpad llegue como `Point` o como `Line`; el pellizco real (en Linux no llega); el umbral 0,15 con un pellizco real; Windows y macOS.
- **Cómo simular un botón lateral sin tenerlo (solo X11 o XWayland):** `xdotool click 8` (atrás) y `xdotool click 9` (adelante) sobre la ventana; no es una captura, es entrada sintética. En Wayland nativo no conozco una forma fiable: se queda sin verificar.

## Prueba manual (la haces tú, sin capturas)

1. **Rueda** (ratón): un paso por muesca, anclado al cursor; con Ctrl, también **un** paso por muesca (antes saltaba ~11).
2. **`Mayús` + rueda** desplaza en horizontal (muesca arriba = la imagen va a la derecha); sin zoom.
3. **Botón central** y **`Espacio` + izquierdo** desplazan; `Espacio` sin gesto no hace nada; **`Espacio` a mitad de un trazo lo cancela** (la imagen queda como estaba) y un diálogo abierto sigue usando `Espacio` como siempre.
4. **Trackpad, dos dedos:** anota si **desplaza** (Wayland/macOS) o **hace zoom lento** (X11/Windows, decisión 2).
5. **Pellizco** (macOS, o Ctrl + rueda en Linux/Windows): pasos sueltos, no una cascada; anota si el umbral 0,15 se siente corto o largo.
6. **Botones laterales** (si el ratón los tiene, o `xdotool click 8` / `9`): atrás deshace y adelante rehace, también con el cursor sobre la paleta; con un trazo a medias o un campo hexadecimal enfocado no hacen nada.
7. **Botón derecho** pinta con el fondo (con y sin `Espacio`); ningún menú contextual en el lienzo.
8. **`Alt`**: `Alt` + arrastrar mueve la ventana en KDE (no llega a la app); anota si en tu versión es `Alt` o `Meta`.
9. Con el cursor sobre la paleta, la rueda sigue desplazando la rejilla y no hace zoom.
10. Con la ventana mínima y `ppp` 1,0, 1,5 y 2,0: el desplazamiento por dedos es suave (sin saltos de un píxel) y la imagen no se sale del lienzo.

## Fuera de alcance

Pantalla de remapeo y guardado en disco (hito 8); botones del 6 en adelante (inalcanzables sin salir de `eframe`); clic derecho en herramientas y en los botones de zoom (I.3); presión de tableta; gestos de pantalla táctil (`Event::Touch`); inercia propia (la de macOS ya llega como `Momentum` en `Point`, sin tocarla); animación entre pasos de zoom; ocultar paneles.

## Decisiones propuestas (aprobadas, ver el visto bueno)

1. **Solo dos botones extra** (`Back`/`Forward`); del 6 en adelante no se promete (egui-winit los descarta). Lo que pediste («todos los botones extra») no se puede sin dejar `eframe`.
2. **Rueda frente a trackpad por la unidad, sin heurísticas**, con la advertencia de que en X11 y Windows los dos dedos llegan como líneas (decisión 2): ¿te vale, con la reasignación en el hito 8 como salida?
3. **Acumuladores:** rueda 1,0 por evento (`Line` 3,0 = 3 pasos, antes 1); pellizco y Ctrl+rueda en logaritmo con umbral **0,15** (ajustable tras la prueba manual); y el trackpad en píxeles y `Mayús` + rueda **dejan de hacer zoom** (decisión 7).
4. **`Espacio` cancela solo trazos, no diálogos**, y la prueba `fila_15_pulsar_espacio_a_mitad_de_un_trazo…` **invierte su intención** (el trazo ahora se cancela). Si prefieres que `Espacio` no cancele nada a mitad de trazo (el trazo sigue, como hoy), dímelo y se queda como está.
5. **Desplazamiento:** el contenido sigue a los dedos; `Mayús` + muesca hacia arriba lleva la imagen a la derecha; 40 puntos por muesca; restos de medio píxel arrastrados.
6. **Botones laterales en toda la ventana con `Origin::Button`** (bloqueados con gesto, diálogo, modal y campo de texto); sin variante nueva en `Origin`.
7. **`Alt` no existe en la tabla** y `Espacio` + derecho sigue pintando con el fondo (los modificadores de `Right` y `Middle` no cuentan).
8. **Clic derecho en herramientas y en los botones de zoom** se van a I.3 (y se corrigen el INDEX y PLAN-RESUMEN al aprobar).

## Corrección posterior (al cerrar la unidad; la única)

Lo que la implementación fijó y la spec no decía o decía distinto: (a) `Scroll` asignado a `ZoomStep` da una muesca por cada `PAN_LINE_POINTS` (40) puntos de dedos, con su propio acumulador; (b) una **página** de rueda (`Page`) vale una muesca (`wheel_of` no distingue `Line` de `Page`), no el ancho del lienzo; (c) la traducción de los botones laterales vive en `px-ui::pointer_events::window_actions` (pura, probada sin ventana) y `App::pointer_actions` solo la llama con `Origin::Button`; (d) `Gesture::Draw` guarda el botón físico que empezó el trazo (la tabla puede asignar `Draw` a cualquiera); (e) el caso 11 usa `Point` y `Line`, y las repeticiones de `Espacio` se prueban tras la pulsación (egui solo marca `repeat` si la tecla ya estaba abajo); (f) el caso 12 usa 7 eventos de 0,4 pt (2,8 px a `ppp` 1), no 5 (2,0 px es el borde del redondeo).

## Contingencia (NO se implementa ahora)

Si la prueba manual en **Windows o X11** muestra que los dos dedos del trackpad hacen zoom lento (llegan como `Line` fraccionarios, decisión 2), se añade **en un commit aparte** la regla «un delta de líneas no entero es un trackpad y desplaza»: en `pointer_events::wheel_of`, un `Line` cuyo `delta.y` (o `delta.x`) no es entero da `Scroll` y no `Wheel`. Limitación conocida de esa regla: una rueda de alta resolución (fracciones por diseño) también desplazaría; por eso no entra sin la prueba. **Windows entrega el pellizco como Ctrl + rueda** (por lo que sé, no verificado) y el acumulador de `Pinch` ya lo cubre: no necesita nada más.

## Visto bueno (2026-10-21)

Las decisiones 1 a 8 como están, con:

1. **Botones:** solo los extra 4 y 5 (`Back`, `Forward`) llegan a la app; `egui-winit` descarta del 6 en adelante. Se anota en `docs/DISENO-INTERFAZ.md` («Soporte mínimo»: izquierdo, central, derecho y dos extra, todos reasignables), en el INDEX y en PLAN-RESUMEN.
2. **Rueda frente a trackpad por la unidad, sin heurísticas**, con la **contingencia** de arriba (no se implementa). Las constantes de los acumuladores (umbral 0,15, `WHEEL_STEP`, `PAN_LINE_POINTS`) viven en un solo sitio (`gestures`), con un comentario para ajustarlas tras la prueba manual.
3. **`Espacio` cancela el trazo en curso y no actúa sobre diálogos** (en un diálogo activa el botón enfocado o marca la casilla, con sus pruebas).
4. **`pencil.rs::fila_15_pulsar_espacio_a_mitad_de_un_trazo…`** se adapta con el nombre explícito `fila_15_espacio_a_mitad_de_un_trazo_lo_cancela`; el mensaje del commit explica qué afirmaba antes y qué afirma ahora. Si otra prueba anterior necesitara más que firmas, literales o cifras, se para.
5. **Dos filas más:** la 11 (el sentido del desplazamiento lo da el sistema, también con el desplazamiento natural de macOS; la app no lo invierte) y la 12 (los restos de medio píxel se arrastran). 12 filas y 3 propiedades.
6. **Clic derecho en herramientas y en los botones de zoom: I.3.** Corregidas las dos líneas del INDEX y de PLAN-RESUMEN.
