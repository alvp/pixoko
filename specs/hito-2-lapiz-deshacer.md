# Hito 2 · Unidad 6: lápiz y deshacer/rehacer en la app

Estado: **aprobada** (2026-10-17; decisiones 1 a 8 con los matices 1, 3, 5, 6 y 9 incorporados al texto: (1) el segmento entero usa el píxel de la llamada nueva, extremo ya pintado incluido; (3) si hubiera que tocar un test de la unidad 2, se para y se avisa; (5) tabla de colores propia, ranura 9 con alfa 0 y gris propio en Gray, y las ranuras sobrantes del indexado se ignoran sin error; (6) `Canvas::show` y sus pruebas no cambian, `interact` y `paint` son aditivos, y se anota el umbral de la superposición; (9) se usan los eventos de puntero intermedios, que egui 0.37 sí expone). **No toca `px-core`**: `Image::check_pixel` ya es pública (unidad 5) y `Document` trae todo lo demás.

## Objetivo

Dibujar con el botón izquierdo en la ventana y poder deshacer y rehacer. Cada trazo es **un solo paso de historia** y la lógica de trazo, el color activo y los atajos son funciones puras que se prueban sin ventana.

## Piezas

Cambios de infraestructura (sin dependencias externas nuevas): `px-ui` pasa a depender de `px-raster` (crate del propio workspace, por ruta; `px-raster` solo depende de `px-core`, no hay ciclo). `docs/DEPENDENCIES.md` no cambia.

En `px-ui`, módulos nuevos sin tipos de egui salvo el último:

```rust
// stroke.rs
pub struct Stroke { /* ancho, alto, último punto, píxeles, índice por coordenada */ }
impl Stroke {
    pub fn new(width: u32, height: u32) -> Stroke;
    /// Primera llamada: pinta `pos`. Después: la línea de la posición anterior a `pos`.
    pub fn move_to(&mut self, pos: (i32, i32), pixel: Pixel);
    pub fn pixels(&self) -> &[PixelEdit];       // en el orden de primera visita
    pub fn is_empty(&self) -> bool;
    pub fn last(&self) -> Option<(i32, i32)>;   // sin recortar
    pub fn commit(self, doc: &mut Document, image: ImageId) -> Result<bool, DocumentError>;
}
// color.rs
pub const SLOTS: u8 = 9;
pub fn pixel_for_slot(image: &Image, slot: u8) -> Option<Pixel>;
pub fn slot_label(image: &Image, slot: u8) -> Option<String>;
// shortcuts.rs  (usa egui::Key y egui::Modifiers como datos; no necesita Context)
pub enum Action { Undo, Redo, SelectColor(u8) }
pub fn action_for_key(key: egui::Key, modifiers: egui::Modifiers) -> Option<Action>;
```

Además: `View::screen_to_image_i32(p, size) -> (i32, i32)` (como `screen_to_image` pero sin devolver `None` fuera: floor, saturado a `i32`), `texture::pixel_rgba(image, pixel) -> Option<Rgba>` (la conversión que ya usa `to_rgba8`, ahora reutilizable), y en `Canvas` (todo aditivo):

```rust
pub enum CanvasEvent { StrokeBegin((i32, i32)), StrokeMove((i32, i32)), StrokeEnd }
impl Canvas {
    /// Sube la textura si falta, asigna el rectángulo, procesa las entradas (zoom, desplazamiento,
    /// gesto) y devuelve los eventos de trazo del fotograma, en orden.
    pub fn interact(&mut self, ui: &mut Ui) -> Vec<CanvasEvent>;
    /// Pinta fondo, imagen y la superposición `(x, y, color)` de los píxeles pendientes.
    pub fn paint(&mut self, ui: &mut Ui, overlay: impl IntoIterator<Item = (u32, u32, Rgba)>) -> CanvasInfo;
    /// Igual que `interact` y `paint` seguidos, con los eventos descartados y sin superposición.
    pub fn show(&mut self, ui: &mut Ui) -> CanvasInfo;
    /// Regenera la textura con `image` conservando la vista (ver 6).
    pub fn refresh(&mut self, ctx: &Context, image: &Image);
}
```

En `px-app`, `title.rs`: `pub struct TitleInfo { size: Option<(u32, u32)>, zoom: Zoom, hover: Option<(u32, u32)>, color: Option<String>, can_undo: bool, can_redo: bool, error: Option<String> }` y `pub fn title_text(&TitleInfo) -> String`.

Ver 2, 3 y 6.

## Comportamiento

### 1. `Stroke`: lógica de trazo pura

- Trabaja en coordenadas de imagen `i32` (pueden caer fuera del lienzo). Conoce el tamaño del lienzo y **solo guarda píxeles dentro**; `last()` guarda la posición **sin recortar**, para que un trazo que sale y vuelve se una con lo que tiene al otro lado.
- `move_to(pos, pixel)`: si `pos == last()`, **no hace nada** (aunque `pixel` sea otro). Si es la primera, escribe `pos`. Si no, recorre `Line::new(last, pos).clip_to(width, height)` y escribe cada punto. La línea incluye el punto anterior, que ya estaba: no se duplica.
- **El segmento entero usa el píxel de la llamada nueva**, incluido el extremo que ya estaba pintado: unir `(1,1)` con `A` y `(3,1)` con `B` deja `(1,1)`, `(2,1)` y `(3,1)` en `B`.
- **Sin duplicados, la última escritura gana:** una coordenada aparece una sola vez, en el lugar de su primera visita, y con el `Pixel` de su última escritura (coincide con la regla de `SetPixels`). La app siempre pasa el mismo `Pixel` en un trazo; el parámetro existe para que la regla se pueda probar y para tramas con color variable más adelante.
- **Sin huecos:** dos puntos consecutivos de una `Line` difieren como mucho 1 en cada eje, y los segmentos comparten extremos, así que con todas las posiciones dentro el conjunto es 8-conexo. Si una posición cae fuera, la parte exterior de la línea se recorta y puede haber hueco **dentro** del lienzo (caso 6): la línea pasó por fuera.
- Ningún `i32` hace entrar en pánico; el coste por llamada es O(lado mayor del lienzo) porque se recorta antes de recorrer.
- `commit` consume el trazo: vacío ⇒ `Ok(false)` **sin llamar a `apply`** (un `SetPixels` vacío sería `EmptyCommand`); si no, `doc.apply(SetPixels { image, pixels })` y devuelve su resultado.

### 2. Un trazo, un paso, sin tocar `px-core`

- Mientras el botón está abajo **el documento no se modifica**. El trazo pendiente se pinta como **superposición** sobre la textura: por cada píxel pendiente, un rectángulo del tamaño del zoom (y, debajo, uno del color de fondo si el alfa es menor que 255, para que se vea lo que quedará: el resultado **reemplaza** el píxel, no se mezcla con el anterior). No se copia ni se sube la imagen en los fotogramas del trazo.
- Al soltar: `Stroke::commit` ⇒ **un `SetPixels`** (no `Batch`: es una sola imagen y un solo comando). `Ok(true)` ⇒ un paso nuevo; `Ok(false)` (trazo vacío, todo fuera del lienzo, o que no cambia ningún píxel) ⇒ **no entra en la historia** y `can_undo`, `can_redo` e `history_bytes` quedan como estaban (un `redo` pendiente no se pierde).
- Un `Err` (hoy solo `OutOfMemory`) descarta el trazo, deja el documento intacto y se muestra en el título hasta la siguiente acción correcta.
- Coste del overlay: **O(P) por fotograma**, con `P` los píxeles del trazo. **Umbral de fluidez: por medir al implementar** (se anotará aquí: los píxeles por trazo a partir de los cuales el fotograma deja de caber en ~16 ms, en `--release`). Mientras no se mida, la cifra es desconocida. **La superposición como textura es de la unidad 14.**

### 3. Interacción

- **El gesto se decide al pulsar y no cambia hasta soltar.** Botón central ⇒ desplazar. Izquierdo con espacio abajo en ese instante ⇒ desplazar. Izquierdo sin espacio ⇒ dibujar. Otros botones: nada. Pulsar espacio a mitad de un trazo no desplaza (el trazo sigue); soltar espacio a mitad de un desplazamiento no empieza a dibujar; un segundo botón durante un gesto se ignora. Esto **cambia un detalle de la unidad 2** (hoy, espacio a mitad de un arrastre izquierdo ya desplaza); si algún test de esa unidad hubiera que tocarlo, se para y se avisa.
- Solo se empieza un trazo si la pulsación cae dentro del rectángulo del lienzo. Puede caer sobre el fondo gris fuera de la imagen: el trazo empieza allí y no pinta nada hasta entrar (con recorte, como en 1). Durante el trazo la posición se toma de los eventos de puntero aunque salgan del rectángulo o de la ventana, con la vista actual (la rueda puede cambiar el zoom a mitad de trazo; cada posición se convierte con la vista de ese momento).
- `Canvas` emite eventos en orden, en coordenadas de imagen `i32`: `CanvasEvent::StrokeBegin(pos)`, `StrokeMove(pos)` y `StrokeEnd`. **Posiciones:** egui 0.37 entrega en `RawInput::events` **todos** los `PointerMoved` de un fotograma (`egui-winit` emite uno por cada `CursorMoved`), así que se emite un `StrokeMove` por cada uno y no una posición por fotograma; los cambios de dirección rápidos no se pierden y el hueco que quede entre dos eventos lo cierra `Line`. Cada evento se convierte con la vista que hay en ese punto del orden de eventos del fotograma.
- **Un clic sin movimiento** emite `Begin(p)` y `End`: pinta 1 píxel (un paso, salvo que ya fuera de ese color).
- **Cuándo termina un trazo (y aplica lo dibujado, nunca lo descarta en silencio):**
  1. Soltar el botón izquierdo en cualquier sitio, dentro o fuera del lienzo o de la ventana: se añade la posición de soltado (`Move`) y `End`.
  2. `WindowFocused(false)`: `End` con lo que hay. Comprobado en el código de egui 0.37: al perder el foco limpia teclas y modificadores pero **no** suelta los botones del puntero, así que lo hacemos nosotros.
  3. Red de seguridad: en cada fotograma, si hay trazo y egui dice que el botón izquierdo **no** está abajo (un soltar perdido, por ejemplo fuera de la ventana en algún sistema), `End`.
  - `PointerGone` **no** termina el trazo: egui lo trata como «el ratón salió, el botón puede seguir abajo»; si vuelve con el botón abajo el trazo continúa, y si no, lo cierra la regla 3.
- Mientras hay un trazo en curso, los atajos de la sección 5 se **ignoran** (deshacer a mitad de trazo dejaría un trazo pendiente sobre una historia que acaba de cambiar).

### 4. Color activo

- Hay 9 **ranuras** (`1` a `9`). La app guarda la ranura (`u8`, inicial 1), **no un `Pixel`**: el `Pixel` se calcula al empezar cada trazo con `pixel_for_slot(image, slot)`, que mira el modo y la paleta **de esa imagen**. Así un `Pixel` de otro modo o con índice fuera de la paleta no se puede construir desde la app (propiedad 5). Un trazo cuya ranura da `None` no empieza.

| Modo | Ranura `n` (1 a 9) |
| --- | --- |
| RGBA | `#000000`, `#ffffff`, `#ff0000`, `#00ff00`, `#0000ff`, `#ffff00`, `#ff00ff`, `#00ffff`, y la 9 transparente `(0,0,0,0)` (alfa 0) |
| Gray (sin alfa) | `min(255, (n−1)·32)`: 0, 32, …, 224 en las ranuras 1 a 8, y **255 en la 9**, un gris distinto de las demás (no hay transparente) |
| GrayAlpha | como Gray con alfa 255 en las ranuras 1 a 8, y la 9 transparente: `{gray: 0, alpha: 0}` |
| Indexed | `Indexed(n−1)` si `n ≤ paleta.len()`; si no, `None`: las ranuras sobrantes se ignoran sin error (solo se alcanzan los 9 primeros índices hasta el panel de la unidad 11) |

- La tabla de colores es propia y original (no sale de ningún otro programa). Elegir una ranura que da `None` con la imagen actual **se ignora** (sin error, y la ranura activa no cambia): no se puede elegir una ranura inexistente. Cualquier otro valor (0, 10…) da `None`.
- El título muestra la ranura y su color, con `slot_label(image, slot) -> Option<String>`: `color 3 #ff0000`, `color 2 índice 1`, `color 4 gris 96`, `color 9 transparente` (alfa 0 en RGBA y GrayAlpha; `color 9 gris 255` en Gray).

### 5. Deshacer y rehacer

- `action_for_key(key, modifiers)` es total y pura. `command` es Ctrl en Linux y Windows y Cmd en macOS (egui lo resuelve):
  - `command` + `Z`, sin `shift` ni `alt` ⇒ `Undo`.
  - `command` + `Y`, o `command` + `shift` + `Z`, sin `alt` ⇒ `Redo`.
  - `Num1`…`Num9` **sin ningún modificador** ⇒ `SelectColor(n)`.
  - Todo lo demás (incluidos `Num0`, `Shift`+`Num1`, `Ctrl`+`Alt`+`Z`, `command`+`shift`+`Y`, `Z` sola) ⇒ `None`.
- Las repeticiones de tecla cuentan: mantener `Ctrl+Z` deshace varios pasos.
- Si la acción es `Undo` y `can_undo()` (o `Redo` y `can_redo()`), se llama a `doc.undo()` / `doc.redo()`; si no, no hace nada. Un `Err` va al título como en la sección 2.
- El título lleva `deshacer` y/o `rehacer` solo cuando se puede: `Pixoko: 64×48 · ×8 · (3, 4) · color 3 #ff0000 · deshacer · rehacer`. Se construye con una función pura de `px-app` que recibe los datos ya calculados (sin `egui`), para probarla sin ventana.

### 6. Textura

- Tras un `apply` que devuelve `true`, o un `undo`/`redo` correcto, se **regenera la textura completa**: `Canvas::refresh(&mut self, ctx, image)` convierte con `to_rgba8` y reemplaza el contenido de la textura existente, **conservando la vista** (a diferencia de `set_image`, que la reinicia). Si el tamaño de la imagen cambiara, se comporta como `set_image`. Después de cambiar algo se pide `ctx.request_repaint()`.
- Para que al soltar no haya un fotograma sin trazo (overlay ya quitado y textura aún vieja) ni uno con el overlay atrasado, `Canvas::show` se parte en `interact(ui) -> Vec<CanvasEvent>` (vista y eventos) y `paint(ui, overlay)` (dibujo); la app aplica los eventos entre las dos. `show` y sus pruebas **no cambian** (partirlo es aditivo): sigue existiendo, con la misma firma, como las dos seguidas y sin overlay. Si algún test de la unidad 2 hubiera que tocarlo, se para y se avisa.
- Coste: `to_rgba8` es O(ancho·alto) por trazo, por `undo` y por `redo` (no por fotograma). **La actualización por regiones y la superposición como textura son de la unidad 14.**

## Fuera de alcance

Grosor de pincel (unidad 7 o posterior), línea con Mayús, botón derecho (borrador o cuentagotas), cancelar un trazo con `Esc`, paleta editable y selector de color (unidad 11), abrir y guardar (15), actualización de textura por regiones (14), y que `Canvas` sea genérico en el número de imágenes.

## Casos límite

`P` es un píxel cualquiera del modo de la imagen. Trazos sobre 8×8 salvo que se diga otra cosa. Las coordenadas son `(x, y)`. La fila 6 está calculada a mano con la fórmula de `Line` (`i` de 0 a 14: el tramo exterior da `(7,4)` y `(6,4)`).

| # | Entrada | Resultado |
| --- | --- | --- |
| 1 | `move_to((2,3))` | `[(2,3)]` |
| 2 | `(1,1)`, `(1,1)`, `(4,1)` | `[(1,1),(2,1),(3,1),(4,1)]`: la repetida se ignora y el salto de 3 no deja huecos |
| 3 | `(0,2)`, `(4,2)`, `(2,0)`, `(2,4)` | 10 píxeles: `(0,2),(1,2),(2,2),(3,2),(4,2),(3,1),(2,0),(2,1),(2,3),(2,4)`; `(2,2)` se visita dos veces y sale una |
| 4 | `(1,1)` con `A`, `(3,1)` con `A`, `(1,1)` con `B`; y `(1,1)` con `A`, `(3,1)` con `B` | `(1,1),(2,1),(3,1)`, **los tres con `B`** en las dos (última escritura gana, el segmento entero usa el píxel nuevo incluido el extremo ya pintado; orden de primera visita) |
| 5 | Todas con `x < 0`: `(-5,2)`, `(-1,6)`, `(-3,0)` | vacío; `last() == (-3,0)`; `commit` ⇒ `Ok(false)` |
| 6 | `(6,2)`, `(20,2)`, `(6,4)` | `(6,2),(7,2),(7,4),(6,4)`: el tramo de ida y vuelta por fuera no se dibuja y no se «cierra» por dentro |
| 7 | `(i32::MIN,i32::MIN)` → `(i32::MAX,i32::MAX)` en 8×8; y en 1×1 `(0,0)` → `(5,5)` | sin pánico; la diagonal `(0,0)…(7,7)`; y `[(0,0)]` |
| 8 | `commit` de un trazo de 3 píxeles que cambian, en RGBA | `Ok(true)`, `can_undo`, no `can_redo`; `undo` ⇒ imagen igual a la copia previa; `redo` ⇒ igual a la editada. Dos trazos: un `undo` deshace solo el último |
| 9 | `commit` de un trazo vacío, de uno todo fuera, y de uno con el color que ya hay; tras un `undo` previo | `Ok(false)`; imagen, `can_undo`, `can_redo` (sigue `true`) e `history_bytes` iguales; los dos primeros no llaman a `apply` (no hay `EmptyCommand`) |
| 10 | `commit` con `Indexed(0)` sobre una imagen RGBA (el modo de la app no lo permite, el documento lo sigue vigilando) | `Err(Pixel { pixel: 0, source: ModeMismatch { expected: Rgba, found: Indexed } })`; documento intacto |
| 11 | `pixel_for_slot`: RGBA 1 y 9; Gray 8 y 9; GrayAlpha 9; Indexed con paleta de 3 en las ranuras 3 y 4; ranuras 0 y 10 | negro y `(0,0,0,0)`; `Gray(224)` y `Gray(255)`; `GrayAlpha{0,0}`; `Indexed(2)` y `None` (sobrante, sin error); `None` y `None` |
| 12 | `action_for_key`: `COMMAND`+`Z`, +`Y`, `COMMAND\|SHIFT`+`Z` / `Z` sola, `COMMAND\|SHIFT`+`Y`, `COMMAND\|ALT`+`Z` / `Num1`…`Num9` / `Num0`, `SHIFT`+`Num1`, `COMMAND`+`Num1` | `Undo`, `Redo`, `Redo` / `None` ×3 / `SelectColor(1..9)` / `None` ×3 |
| 13 | `View::screen_to_image_i32` con zoom ×4 y origen `(10,10)`: `(10,10)`, `(9,9)`, `(i64::MIN,i64::MAX)` | `(0,0)`, `(-1,-1)`, `(i32::MIN, i32::MAX)` |
| 14 | Harness de egui (`Context` con `RawInput` sintético, como en `tests/canvas.rs`): clic izquierdo sin mover, dentro de la imagen | `Begin(p)`, `End`; con `commit`, 1 píxel y un paso |
| 15 | Mismo harness: botón central arrastrado; izquierdo con espacio abajo al pulsar; espacio pulsado a mitad de un trazo; espacio soltado a mitad de un desplazamiento | sin eventos de trazo y la vista se mueve; igual; el trazo sigue y la vista no se mueve; no se emite `Begin` |
| 16 | Mismo harness: trazo que sale (`PointerGone`) y se suelta fuera con posición; otro con `WindowFocused(false)`; otro con soltar perdido (egui con el botón arriba, sin evento de soltar) | `End` con lo dibujado en los tres, y el último punto del primero incluido; sin trazos que queden abiertos |
| 17 | Título con `can_undo`/`can_redo` en las cuatro combinaciones y un error | `deshacer` y `rehacer` presentes solo si se puede; el error sustituye al estado |

## Propiedades (proptest)

Imágenes y lienzos de 1 a 16 de lado, en los cuatro modos cuando hay imagen; posiciones `i32` arbitrarias (con sesgo hacia dentro y los bordes) salvo donde se diga.

1. **Oráculo geométrico y conexión.** El conjunto de píxeles de un `Stroke` es exactamente la unión de los puntos de `Line::new(a, b)` recortados al lienzo, para cada par consecutivo (y el primer punto), más nada. Todos caen dentro y no hay coordenadas repetidas. Si **todas** las posiciones están dentro, el conjunto es 8-conexo.
2. **Valores y repeticiones.** Con un `Pixel` distinto por llamada, el valor de cada coordenada es el de su última escritura (oráculo con un `HashMap`), el orden es el de primera visita, e insertar repeticiones consecutivas de una posición no cambia el resultado.
3. **Un trazo, un paso.** Sobre cualquier imagen, `commit` da `Ok(true)` si y solo si algún píxel cambia de valor; entonces hay exactamente un paso: `undo` restaura la imagen **exactamente** (`==` con la copia previa) y `redo` la editada. En una secuencia de `k` trazos que dieron `Ok(true)`, `k` `undo` vuelven al original, `k` `redo` al final, y un trazo nuevo tras un `undo` vacía lo rehacible.
4. **No-ops.** Un trazo vacío, todo fuera o que no cambia nada da `Ok(false)` y no altera imagen, `can_undo`, `can_redo` ni `history_bytes`.
5. **El color nunca es del modo equivocado.** Para cualquier imagen y cualquier ranura de 0 a 255: si `pixel_for_slot` da `Some(p)`, `image.check_pixel(p)` es `Ok`; y un trazo hecho con ese `p` nunca devuelve `Err(Pixel { .. })`. `None` solo ocurre con ranura fuera de 1 a 9 o con indexado y `n > paleta.len()`.
6. **Atajos.** Con todas las combinaciones de los cinco modificadores y las teclas `Z`, `Y`, `Num0`…`Num9` y unas cuantas más: `action_for_key` no entra en pánico; da `Undo` o `Redo` solo con `command` y sin `alt`; da `SelectColor` solo sin ningún modificador; y `SelectColor(n)` cumple `1 ≤ n ≤ 9`.

## Decisiones que necesito que confirmes

1. **`Stroke::move_to(pos, pixel)` recibe el píxel en cada llamada** (para que «la última escritura gana» sea observable y probable, aunque la app pase siempre el mismo). Una posición idéntica a la anterior se ignora **aunque el píxel cambie**. Alternativa: un `Pixel` fijo en `Stroke::new`, más simple, con esa regla inobservable.
2. **El trazo guarda solo píxeles dentro y recuerda la última posición sin recortar** (lo que hace que fuera del lienzo no haya líneas «por dentro», fila 6). Alternativa: guardar todo y recortar al aplicar, que gasta memoria sin necesidad.
3. **Reglas de fin de trazo** (sección 3): soltar en cualquier sitio, `WindowFocused(false)` y botón arriba según egui terminan y **aplican**; `PointerGone` solo no termina. Y que **el gesto se decide al pulsar** (espacio a mitad de arrastre ya no desplaza: cambia un detalle de la unidad 2). Alternativa a aplicar al perder el foco: descartar el trazo; me parece peor, porque pierde trabajo sin avisar y con `Ctrl+Z` se corrige lo aplicado.
4. **Atajos ignorados durante un trazo.** Alternativa: aplicarlos y dejar el trazo pendiente sobre el documento nuevo.
5. **Color: ranuras 1 a 9 con la tabla de la sección 4** (incluida la transparente en RGBA y GrayAlpha, para ejercitar el alfa), guardando la **ranura** y no el `Pixel`. Y el formato del título.
6. **Overlay con un rectángulo por píxel del trazo** (O(P) por fotograma, medido al implementar) y **partir `Canvas::show` en `interact` + `paint`**, conservando `show`. Alternativa al desdoble: dejar `show` como está y aceptar un fotograma de desfase al soltar (parpadeo del trazo en cada soltar).
7. **Repetir `Ctrl+Z` al mantenerlo cuenta** (cada repetición deshace un paso).
8. **Sin cambios en `px-core`**, y `px-ui` depende de `px-raster` por ruta. Si al implementar algo exigiera tocar `px-core`, se para y se avisa.

## Para la unidad 14

- Actualización por regiones de la textura (en vez de regenerarla entera tras cada trazo, `undo` y `redo`): el trazo ya conoce su caja envolvente y `Step` los píxeles cambiados.
- Superposición del trazo como textura pequeña (en vez de un rectángulo por píxel), si la medida de la sección 2 lo pide.
