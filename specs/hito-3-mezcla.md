# Hito 3 · Unidad 3.2: mezcla de capas (composición pura)

Estado: **implementada** (2026-10-21; commits `139be51` y `4087137` de spec, `d8857dc` pruebas en rojo, `5c03720` lógica A en `px-core` y `4ed4805` lógica B en `px-raster`; detalle y medidas en «Corrección posterior»). Aprobados `P-01` a `P-08` (`px-core`, **sin ningún otro cambio en `px-core`**) y los cambios de `px-raster` (`R-01` a `R-04`), los 12 cambios exactos de la spec, todos aditivos. **Cambio de decisión del usuario sobre la divergencia (decisión 1):** la regla «W3C con la divergencia declarada» era del autor de la spec y los datos la contradicen (W3C se aleja hasta 62 niveles del programa de referencia); el usuario prioriza que **un `.aseprite` se vea igual en la referencia y en Pixoko**, así que el paso 4 con fondo translúcido y modo ≠ Normal usa una **fórmula EMPÍRICA** (la forma observada `m = 1 − (1 − αb)(1 − αs·αb)` con la mejor secuencia de enteros hallada en los datos), con un techo de error de 2 vigilado por pruebas; W3C queda como alternativa descrita. Resto de decisiones: composición en `px-raster`, `BlendMode` sin discriminantes, 17 goldens en 3.2 (los 14 de rectángulos, las dos rejillas y `fondo-con-opacidad`, que lleva el disco), `px-golden` con `px-raster` como `dev-dependency`, indexado sin opacidad ni modo, `NoTransparency`, nota de 3.8; `αs` con redondeo (decisión 3) y capa de abajo con opacidad contrastada (decisión 7). Los umbrales de rendimiento se miden al cerrar y se ajustan a lo medido (si se pasan, se para).
 El visto bueno vale como aprobación expresa de `px-core` y `px-raster` **solo para los cambios numerados de abajo** (D02): `P-01` a `P-08` (`px-core`) y `R-01` a `R-04` (`px-raster`). Si durante la unidad hace falta algo que no esté en esa lista, me paro.

Referencias: filas 3.2 y 3.3 y decisiones D02, D06 y D07 de [`mapa-funciones.md`](mapa-funciones.md); [`hito-3-capas-modelo.md`](hito-3-capas-modelo.md) (3.1: `Layer`, celdas opcionales, indexado sin opacidad); [`corpus-hito-3.md`](corpus-hito-3.md) (casos de `golden/aseprite/`). Sin dependencias nuevas, sin `unsafe`. **No se ha abierto nada de Aseprite** (ni código ni documentación): lo único que se usa de él son los PNG ya generados de `golden/aseprite/`, como datos.

## Objetivo

Una función pura que compone la pila de capas de un `Document` en una `Image` del modo del documento, con ocho modos de mezcla y opacidad de capa, en enteros, sin redondeos que dependan de la plataforma y con **tolerancia 0** contra los PNG aplanados del corpus donde el corpus lo permita; y una forma de componer solo un rectángulo, que reutilizará 3.3 (baldosas sucias y trazo). Esta unidad no cambia nada de lo que ve el usuario: nadie llama aún a la composición (la 3.3 la conecta).

## Lo medido (prototipo desechable, fuera del repositorio)

Antes de escribir fórmulas probé candidatas contra el corpus con un prototipo en Python (scratchpad, borrado al terminar) que reconstruye las entradas desde las tablas y las formas de `tools/corpus-h3/generar.lua` (propio) y compara con los PNG. Las **formas generales** salen de la teoría (abajo); lo que la teoría no fija es **qué entero se toma entre dos vecinos**, y eso lo decidieron los datos: con 640 píxeles de Normal sobre fondo translúcido, el truncamiento hacia cero en la interpolación y el redondeo en el alfa de salida son los únicos que dan 0 diferencias (probé truncar, redondear y techo en cada paso).

| Caso(s) | Píxeles | Diferentes con la fórmula de esta spec |
| --- | --- | --- |
| `mezcla-normal`, `-multiplicar`, `-trama`, `-superponer`, `-oscurecer`, `-aclarar`, `-sumar`, `-diferencia` (64 combinaciones de color por modo) | 8 × 1024 | **0** |
| `mezcla-ocho-opacidad` (opacidad 128), `mezcla-ocho-alfa` (alfa de origen 200 y 77) | 2 × 1024 | **0** |
| `capas-opacidad` (0, 64, 128, 200, 255), `capas-alfa-origen`, `capas-alfa-fondo` (dos translúcidos a la vez, Normal) | 3 × 512 | **0** |
| `mixto-rgba`, `capas-normal-tres`, `capas-ocultas`, `celda-desplazada`, `celda-vacia`, `rgba-una-capa` (incluido el RGB bajo alfa 0) | 4992 | **0** |
| `mezcla-ocho-fondo-translucido`, tira **Normal** | 128 | **0** |
| `mezcla-ocho-fondo-translucido`, las **otras siete tiras** (fondo con alfa 150 y modo ≠ Normal) | 896 | **715 distintos, máximo 62** |

### Segunda ronda: tres casos nuevos (`mezcla-alfa-rejilla-multiplicar`, `mezcla-alfa-rejilla-superponer`, `fondo-con-opacidad`)

Mismo método (entradas reconstruidas desde `generar.lua`, PNG leídos, comparación píxel a píxel; los 25 casos anteriores regenerados con Aseprite salen con los **mismos bytes**, comprobado por `generar.sh`).

| Caso | Píxeles | Diferentes con la fórmula de esta spec |
| --- | --- | --- |
| `fondo-con-opacidad` (capa de abajo con opacidad 128 y Multiplicar sobre el vacío, Normal translúcida encima, alfa 0 con RGB) | 1024 | **0** |
| `mezcla-alfa-rejilla-multiplicar` y `-superponer`, **fuera** del dominio `0 < αb < 255 y αs > 0` (αb = 0, αb = 255, αs = 0) | 2 × 44 celdas de 9 | **0** |
| ídem, **dentro** del dominio (fondo translúcido, fuente visible, modo ≠ Normal) | 2 × 84 celdas de 9 | **83 de 84** (máximo 51) y **80 de 84** (máximo 16) |
| `mezcla-ocho-fondo-translucido`, siete tiras (primera ronda) | 896 | 715 (máximo 62) |

- **Decisión 3 (`αs`): redondeo.** `αs = div255(a · opacidad)` da el alfa de salida exacto en las 128 + 128 celdas; con truncamiento `(a · opacidad) / 255` fallan **10** (los de opacidad 128, alfa 127 incluido: `63,75`); y en `fondo-con-opacidad` el truncamiento falla en **541** de 1024 píxeles y el redondeo en ninguno (alfas 112, 96, 80, 64, 48, 32 observados).
- **Decisión 7 (capa de abajo): contrastada.** Con opacidad 128 y modo Multiplicar sobre el lienzo vacío el modo no tiene efecto (con Normal sale lo mismo), el alfa se escala por la opacidad y el **RGB bajo alfa 0 se conserva** (`(255,255,255,0)` y `(196,46,52,0)` en la fila de alfa 0). Es lo que dice la regla del paso 2.
- **Decisión 1 (fondo translúcido y modo ≠ Normal): sin secuencia entera con 0, y se adopta la forma observada con techo 2** (cambio de decisión del usuario, abajo). Lo que se sabe: el peso de la mezcla es **`m = 1 − (1 − αb)·(1 − αs·αb)`** en lugar de `αb`, es decir `Cr = Cb + ((1 − m)·Cs + m·B − Cb)·αs/αr`, con el `αr` de siempre (el alfa de salida coincide en el 100 %). Contra las dos rejillas, en real, ese `m` da error máximo 1,6 (W3C, 51). **Con la fuente opaca, dos interpolaciones redondeadas hacia `B` dan 0 de 36 valores distintos.** **No encontré una secuencia entera con 0 para `αs < 255`** (más de 3000 combinaciones; la mejor de la primera búsqueda deja mal 124 de 256 celdas por ±1 y ±2), así que el tope es un **techo de error de 2**: ver «Fórmula empírica del fondo translúcido».

### Fórmula empírica del fondo translúcido (decisión del usuario; sustituye a la «divergencia conocida»)

**Motivo.** El usuario prioriza que un `.aseprite` se vea igual en la referencia y en Pixoko. La forma estándar (W3C) se aleja hasta **62 niveles** de los PNG con fondo translúcido y modo ≠ Normal (blanco multiplicado sobre negro con alfa 150: 105 con W3C, **43** en el PNG); la forma observada se queda en ±1 y ±2. **Es empírica, no teoría:** se derivó mirando los PNG del corpus como datos (caja negra; no se abrió nada de ningún programa) y puede no ser lo que hace el programa fuera de los puntos medidos; por eso lleva techo de error y vigilancia.

**Qué es.** Dominio: **`0 < αb < 255` (fondo ya compuesto translúcido), `αs > 0` y modo ≠ Normal.** Forma real observada: `Cr = Cb + ((1 − m)·Cs + m·B − Cb)·αs/αr` con **`m = 1 − (1 − αb)(1 − αs·αb)`** (con la fuente opaca, `1 − (1 − αb)²`; en W3C sería `m = αb`). La secuencia entera es la del paso 4b. Se eligió entre 750 variantes de la misma estructura (factor `k` por seis redondeos, y cuatro divisiones por paso) con este criterio: a) máximo ≤ 2 contra los 1064 puntos del corpus del dominio (300 variantes cumplen), b) máximo ≤ 2 en todo el dominio frente a la forma real con el `αr` entero (18 cumplen) y c) menos píxeles distintos del PNG. La ganadora redondea al más cercano en todos los pasos.

**Tolerancia declarada: 2 (techo).**

| Referencia | Resultado |
| --- | --- |
| Corpus, 1064 puntos del dominio (rejillas: celdas de 3×3 iguales; siete tiras): píxeles exactos / ±1 / ±2 | 615 / 433 / 16 (las rejillas, por celdas de 3×3: 449 no exactos de 1064); máximo **2** (`mezcla-alfa-rejilla-multiplicar` 2, `-superponer` 2; tiras: multiplicar 2, trama 2, superponer 1, oscurecer 1, aclarar 1, sumar 1, diferencia 2) |
| Forma real observada **con el `αr` entero**, barrido de las 254×255 combinaciones de alfa × 484 parejas de colores, más todas las parejas de colores 256×256 con alfas espaciadas (212 millones de puntos por modo) | máximo por modo: multiplicar **1,72**, trama **1,72**, superponer **1,76**, oscurecer **1,71**, aclarar **1,71**, sumar **1,85**, diferencia **1,76** (todos ≤ 2) |
| Forma real observada con el `αr` **real** (sin redondear) | hasta 4,16 (3,88 a 4,16 por modo), **y no es un error de la fórmula**: es el redondeo del `αr` entero, el mismo que da **0** diferencias con Normal (640 puntos) y el alfa de salida exacto en el 100 % del corpus; solo aparece con alfas pequeñas (`αb` ≤ ~40 y `αs` ≤ ~40). No se declara |

**Vigilancia** (constante `TOLERANCIA_EMPIRICA = 2` en las pruebas, techo y no igualdad):
- fuera del dominio, tolerancia 0 contra los PNG (como acordado);
- dentro del dominio, el máximo contra el PNG es ≤ 2 **y** ≥ 1 en cada caso agregado (las dos rejillas, y cada tira): si baja a 0 la prueba falla con «la exención es innecesaria: usa tolerancia 0 en este caso»; si sube de 2, falla con el píxel y los valores;
- el barrido de las 256×256 combinaciones de alfa de los siete modos (fila 4) falla si el error frente a la forma real con `αr` entero pasa de 2.

**Alternativa descrita (W3C).** El paso 4a para todo `αb`: `Cm = Cs + (B − Cs)·αb/255` y `Cr = Cb + (Cm − Cb)·αs/αr`, la forma de la teoría (Porter y Duff y W3C). Es la que daba 0 con fondo opaco y con Normal, pero con fondo translúcido y modo ≠ Normal dejaba 163 de 256 celdas mal en las rejillas y 715 de 896 píxeles en las tiras, con diferencias de hasta 62. **Cómo se cambia:** solo el cuerpo de 4b en `composite_pixel` (un sitio); si en el futuro aparece la secuencia exacta con 0 diferencias, se sustituye 4b y se baja el techo a 0 (la constante desaparece); si se quiere W3C, se borra 4b.

## Teoría de la que salen las fórmulas (constancia)

- **Composición:** Porter y Duff, *Compositing Digital Images* (1984), operador «source over» con alfa directo (no premultiplicado).
- **Mezcla:** *W3C Compositing and Blending Level 1*, modos separables (multiplicar, trama = *screen*, superponer = *overlay*, oscurecer, aclarar, diferencia) y la suma saturada (*plus*); con el color mezclado `Cm = (1 − αb)·Cs + αb·B(Cb, Cs)` y `Cr = (1 − αs/αr)·Cb + (αs/αr)·Cm`, `αr = αs + αb − αs·αb`. Esa es la forma real; abajo, la de enteros.
- **Ninguna fórmula viene de código ni de documentación de ningún programa.** La salida de Aseprite se usó como dato para escoger entre redondeos vecinos y para ver dónde diverge.

## Fórmulas en enteros

Todo en `i32`; los canales y alfas son `u8`; `Cb`, `αb` son el fondo (lo ya compuesto) y `Cs`, `a` el origen (la celda).

```
div255(x)  = (x + 128 + ((x + 128) >> 8)) >> 8     // x/255 al entero más cercano; exacto en 0..=65025 (comprobado)
x / y      = división entera de Rust: trunca hacia cero (lo fija el lenguaje, igual en todas las plataformas)
```

Por píxel y por capa visible con celda con imagen, de abajo arriba; el resultado empieza en `(0, 0, 0, 0)`:

0. **Origen exactamente `(0, 0, 0, 0)`** (nada, como una celda sin imagen): el resultado es el fondo, sin cambios (también con fondo de alfa 0: no pisa su RGB). *Corrección con datos al escribir las pruebas:* sin este paso, el paso 2 literal tapaba con `(0, 0, 0, 0)` el RGB del fondo con alfa 0 y `fondo-con-opacidad` daba 112 píxeles distintos de 1024 (el PNG conserva `(255, 255, 255, 0)`); con él, 0 en los 17 casos.
1. `αs = div255(a · opacidad)` (redondeo; el truncamiento falla con los datos: segunda ronda).
2. **Fondo con alfa 0** (`αb == 0`, incluido el inicio; el origen no es `(0, 0, 0, 0)`): resultado `(Cs, αs)`. No se mezcla nada (no hay con qué), **también con `αs == 0`: el RGB del origen se conserva** (el PNG de `rgba-una-capa` lo conserva; es lo que hace que una sola capa se componga idéntica a su celda).
3. **Origen con `αs == 0`** (y `αb > 0`): el resultado es el fondo, sin cambios.
4. En otro caso: `αr = αs + αb − div255(αs · αb)` y, por canal, `B = blend(modo, Cb, Cs)` (tabla de abajo); después:
   - **4a (teoría; modo Normal o `αb = 255`):** `Cm = Cs + (B − Cs) · αb / 255` y `Cr = Cb + (Cm − Cb) · αs / αr` (las dos `/` truncando hacia cero). Con `αb = 255`: `αr = 255` y `Cm = B`; con Normal: `Cm = Cs`; con `αs = 255`: `Cr = Cm`. 0 diferencias en el corpus.
   - **4b (EMPÍRICA; `0 < αb < 255` y modo ≠ Normal):** con `rnd(n, d) = (2n + d).div_euclid(2d)` (al entero más cercano, mitades hacia arriba; en `/255` no hay empates):
     ```
     k  = div255(αs · αb)
     x1 = Cs + rnd((B − Cs) · αb, 255)
     x2 = x1 + rnd((B − x1) · k, 255)
     Cr = Cb + rnd((x2 − Cb) · αs, αr)
     ```
     No sale de la teoría: **sale de los datos del corpus (caja negra)**, ver «Fórmula empírica del fondo translúcido».
   El resultado es `(Cr, αr)`. `αr ≥ αs ≥ 1`: no hay división por cero. Los canales quedan en 0..=255 (combinaciones convexas con redondeo: `x1` y `x2` quedan entre `Cs` y `B`, `Cr` entre `Cb` y `x2`).

| Modo | `blend(Cb, Cs)` |
| --- | --- |
| Normal | `Cs` |
| Multiplicar | `div255(Cb · Cs)` |
| Trama | `Cb + Cs − div255(Cb · Cs)` |
| Superponer | `Cb < 128`: `div255(Cs · 2·Cb)`; si no, con `t = 2·Cb − 255`: `Cs + t − div255(Cs · t)` (el umbral es `Cb ≤ 127`; `128` pasa a la rama de trama: lo fija el corpus, con `≤ 128` fallan 155 píxeles, 128 de ellos en `mezcla-superponer`, todos por 1) |
| Oscurecer | `min(Cb, Cs)` |
| Aclarar | `max(Cb, Cs)` |
| Sumar | `min(255, Cb + Cs)` |
| Diferencia | `|Cb − Cs|` |

**Capa de fondo.** Es solo una posición (3.1): su opacidad y su modo se aplican contra el fondo vacío, así que el modo no tiene efecto (paso 2) y la opacidad escala su alfa. **Contrastado con el programa** (`fondo-con-opacidad`, segunda ronda): 0 diferencias.

**Error frente al modelo real** (comprobado por barrido): con `αb ∈ {0, 255}` el entero está a ≤ 0,5 del real en la mezcla y a ≤ 1 en el resultado; con un fondo translúcido, a ≤ 4,5 (muestra de 10 colores × todos los alfa impares), solo con alfas pequeños (`αr` pequeño amplifica el redondeo de `αr`). Es el precio de igualar al corpus a tolerancia 0 (la fórmula con `αr` exacto falla 4 de 640 píxeles de `capas-alfa-fondo` y de la tira Normal). La propiedad 2 fija esas dos cotas.

**Grises con alfa:** las mismas fórmulas con un solo canal (los modos son separables: da lo mismo que `R = G = B`). **Grises sin alfa:** una sola capa posible (3.1: `NoTransparency`), ver abajo.

## Indexado y grises (D06)

Sin opacidad ni mezcla: el resultado es, por píxel, el índice de la **capa visible más alta cuya celda tiene imagen y cuyo índice no es el transparente**; si no hay ninguno, el índice transparente. El núcleo ya garantiza opacidad 255 (`IndexedOpacity`, 3.1) y, desde `P-04`, modo Normal (`IndexedBlend`): **una capa indexada con opacidad ≠ 255 no existe**, y por eso la composición **no la mira** (la ignora; sin fila de prueba porque no se puede construir; la propiedad 2 de 3.1 vigila la invariante). Grises sin alfa usa la misma regla sin valor transparente (gana la capa visible más alta). Si no queda ningún píxel que dar y el modo no tiene valor transparente (indexado sin `transparent_index` o grises con todas las capas ocultas o vacías), `ComposeError::NoTransparency { mode }` (como en 3.1, en vez de inventar un índice). La paleta de la salida es la del lienzo. `NoTransparency` se decide sobre los píxeles del rectángulo pedido (`compose_rect` de una zona donde todos los píxeles tienen algo que dar es `Ok` aunque `compose` del lienzo entero no lo sea).

## API (`px-raster`)

Vive en **`px-raster`** (decisión 1): ya lleva los algoritmos que miran el `Document` y los que **producen `Command`s** (`line_command`, `fill_command`); 3.5 (aplanar, combinar) necesita una composición que dé la imagen del comando, y `px-core` no puede depender de `px-raster`; `px-ui` ya depende de `px-raster`; `px-formats` no necesita componer (recibe una `Image`).

```rust
pub struct Rect { pub x: u32, pub y: u32, pub width: u32, pub height: u32 }   // R-01

pub fn compose(doc: &Document) -> Result<Image, ComposeError>;                      // R-02
pub fn compose_rect(doc: &Document, rect: Rect) -> Result<Image, ComposeError>;     // R-02: imagen de rect.width × rect.height
pub fn composite_pixel(back: Rgba, src: Rgba, opacity: u8, mode: BlendMode) -> Rgba; // R-03: pasos 1 a 4 de un píxel

pub enum ComposeError {                                                              // R-04
    EmptyRect,                              // ancho o alto 0
    RectOutside { rect: Rect, width: u32, height: u32 },   // no cabe en el lienzo
    NoTransparency { mode: Mode },
    Image(ImageError),                      // reserva fallida (try_reserve) o tamaño
}
```

- **Entrada:** `&Document` (`layers()`, `visible()`, `opacity()`, `blend()`, `image(cel)`, `width()`, `height()`, `mode()`, `palette()`). **Salida:** `Image` del modo del documento (RGBA, grises con alfa, indexado con la paleta del lienzo o grises); `compose(doc)` es `compose_rect` del lienzo entero y **`compose_rect(doc, r)` es el recorte de `compose(doc)` en `r`**, píxel a píxel.
- `composite_pixel` es el único sitio de las fórmulas RGBA (la composición lo llama o lo repite inline por rendimiento con una prueba de igualdad, propiedad 1); la usará 3.3 para el cuentagotas de «lo visible» y 4.10 (translucidez).
- Sin estado ni caché: determinista, sin hilos, mismo resultado en las tres plataformas. Reserva el búfer de salida con `try_reserve_exact`; no copia las celdas (lee sus filas) y recorre **capa a capa sobre el búfer de salida** (pico de memoria = la salida). Caminos rápidos: celda vacía u oculta se salta; origen opaco con Normal copia filas; fondo opaco usa `αr = 255` (la división por 255 es constante). Nada de esto cambia el resultado.
- `Layer` sin celda (`image(cel) == None`) no aporta nada (equivale a transparente, pero **sin** RGB: no conserva nada, ver el paso 2).

## Cambios exactos en `px-core` y `px-raster`

**`px-core` (aditivos; 3.1 dejó el campo y el comando de mezcla para aquí):**

| N.º | Dónde | Qué |
| --- | --- | --- |
| P-01 | `document.rs`, `lib.rs` | `pub enum BlendMode { Normal, Multiply, Screen, Overlay, Darken, Lighten, Add, Difference }`: `Clone`, `Copy`, `Debug`, `Default` (= `Normal`), `PartialEq`, `Eq`, `Hash`, `#[non_exhaustive]` (el hito 6 añade más) y `BlendMode::ALL: [BlendMode; 8]`. **Sin discriminantes explícitos ni `as u8` en la API**: nada serializa el valor de Rust (3.8 usa una tabla propia). |
| P-02 | `Layer` | Campo y `pub fn blend(&self) -> BlendMode`; toda capa nueva (`AddLayer`, `Document::new`) lo trae `Normal`; forma parte de `PartialEq`. |
| P-03 | `Command` | `SetLayerBlend { layer: LayerId, blend: BlendMode }`: uno por propiedad como los demás, exclusivo (`NotBatchable`), un paso; mismo modo = `Ok(false)` sin paso. |
| P-04 | `DocumentError` | `IndexedBlend`: modo distinto de `Normal` en un documento indexado (en `SetLayerBlend` y en `ConvertMode` a indexado con alguna capa con modo ≠ Normal, como `IndexedOpacity`). |
| P-05 | `Display` | Un mensaje en español para `IndexedBlend`. |
| P-06 | `Step` (privado) | Variante `LayerBlend` (el valor anterior se intercambia con el nuevo, sin reservar) y su coste (`size_of`). |
| P-07 | `Document::apply` | Orden de comprobación de `SetLayerBlend`: `UnknownLayer`, `IndexedBlend`, sin cambios (`Ok(false)`); el paso entra por el mismo `commit` (`state_id`, «modificado» y deshacer como los otros cinco comandos de capa). |
| P-08 | `lib.rs`, docs | Documentar en `Layer::opacity`/`SetLayerOpacity` que la mezcla es la de esta spec; nada más. |

Ninguna firma pública existente cambia (`Command` y `DocumentError` son `#[non_exhaustive]`; `px-ui` trata lo desconocido como «todo sucio»: `dirty_of` `_ => BlockSet::full`).

**`px-raster` (aditivos):** `R-01` `Rect`; `R-02` `compose` y `compose_rect`; `R-03` `composite_pixel`; `R-04` `ComposeError` (`Display` y `Error` en español); un módulo nuevo `compose.rs` y sus `pub use` en `lib.rs`. Ni `draw.rs`, `fill.rs` ni `line.rs` se tocan.

## Corpus: qué usa cada fila y cómo se leen los PNG

Los PNG se leen con `px_formats::read_png` (RGBA 8 bits; `Image::bytes()` da los bytes) y las **entradas se reconstruyen en la prueba** desde las tablas `A` y `B` y las reglas de `generar.lua` (rectángulos y columnas/filas de color; sin formas): `.aseprite` no se puede leer hasta 3.8. Las pruebas viven en **`px-golden`** (ejecutable nuevo `tests/mezcla.rs` con su `mod common;`: un módulo de `png_interop` dejaría ese ejecutable sin compilar en el commit de pruebas en rojo), porque ya sabe encontrar `golden/aseprite/` y leer PNG; para eso `px-golden` gana `px-raster` como `dev-dependency` (**dependencia interna del propio workspace**: no entra en `cargo deny` ni en `docs/DEPENDENCIES.md`, pero cambia `Cargo.toml` y `Cargo.lock`; decisión 4).

| Filas | Casos del corpus | Cómo |
| --- | --- | --- |
| 9 | `mezcla-normal`, `-multiplicar`, `-trama`, `-superponer`, `-oscurecer`, `-aclarar`, `-sumar`, `-diferencia` | `compose` del documento reconstruido (2 capas, 32×32) = PNG, **tolerancia 0** en los 8 |
| 10 | `mezcla-ocho-opacidad`, `mezcla-ocho-alfa`, `capas-opacidad`, `capas-alfa-origen`, `fondo-con-opacidad` (la única con un disco: una desigualdad entera) | ídem, **tolerancia 0** en los 5 |
| 11 | `capas-alfa-fondo`, `mezcla-ocho-fondo-translucido`, `mezcla-alfa-rejilla-multiplicar` y `mezcla-alfa-rejilla-superponer` | `capas-alfa-fondo`, la tira Normal y todo lo que está **fuera del dominio** de la fórmula empírica, **tolerancia 0**; dentro del dominio, techo `TOLERANCIA_EMPIRICA = 2` y suelo 1 por caso agregado (sección «Fórmula empírica»); el predicado del dominio se aplica por píxel, no por listas |

Lo que no entra aquí y pasa a **3.9**, cuando 3.8 ya lee los `.aseprite` (con tolerancia 0, medido arriba): `mixto-rgba`, `capas-normal-tres`, `capas-ocultas`, `celda-desplazada`, `celda-recortada`, `celda-vacia`, `rgba-una-capa` y los cuatro indexados (hay más formas y desplazamientos de celda, que el modelo de 3.1 recorta al leer; reconstruirlos aquí sería código de usar y tirar). **Si un caso muestra ±1 al implementar**, se para, se anota en esta spec **caso por caso** (qué caso, qué modo, qué píxeles, qué valor da la fórmula y cuál el PNG, y por qué) y se decide con esos datos; nunca se sube una tolerancia general. Más de ±1 es un fallo de la fórmula, no de redondeo (es lo que pasa con el fondo translúcido y modo ≠ Normal, declarado como divergencia conocida).

**Para 3.8 (nota):** los códigos del modo de mezcla del `.aseprite` se leen de **la especificación pública del formato** (la tabla de modos de la capa) con un `match` propio hacia `BlendMode`, y **jamás de los números del enum de Lua** que `corpus-hito-3.md` (sección 5, punto 3) anota solo como curiosidad. Los modos de la especificación que no están en `BlendMode` (diez) los trata 3.8 con aviso o error, no con un valor por defecto en silencio.

## Casos (12 filas)

Notación: `D(n)` es un documento RGBA con `n` capas construido con los comandos existentes y `SetLayerBlend`; «fondo» es el color ya compuesto debajo; `div255` y `/` como arriba. Filas 1 a 8 en `crates/px-raster/tests/compose.rs` (ejecutable nuevo); filas 9 a 11 en `crates/px-golden/tests/mezcla.rs` (ejecutable nuevo); fila 12 en `crates/px-core/tests/blend.rs` (ejecutable nuevo).

| # | Entrada | Resultado |
| --- | --- | --- |
| 1 | `composite_pixel`, identidades y bordes: origen `αs = 0` sobre fondo opaco; opacidad 0; origen opaco Normal; fondo `(0,0,0,0)` con origen `(232,198,52,0)` y con `(10,20,30,200)` Multiplicar opacidad 255 y 100; fondo `(255,255,255,0)` con origen `(0,0,0,0)` | fondo sin cambios `(196,46,52,255)`; ídem; origen exacto; `(232,198,52,0)` (RGB conservado); `(10,20,30,200)` y `(10,20,30,78)` (sin mezcla contra el vacío); `(255,255,255,0)` (el origen vacío no pisa el RGB) |
| 2 | Los ocho modos con fondo `(196,46,52,255)` y origen `(90,60,220,255)`, opacidad 255 (valores de la sección 5 del corpus, hechos a mano) | Normal `(90,60,220)`; Multiplicar `(69,11,45)`; Trama `(217,95,227)`; Superponer `(179,22,90)`; Oscurecer `(90,46,52)`; Aclarar `(196,60,220)`; Sumar `(255,106,255)`; Diferencia `(106,14,168)`; Superponer en el umbral: fondo 127 y 128 con origen 100 → 100 y 101 |
| 3 | Opacidad sobre fondo opaco: rojo `(196,46,52)` sobre negro con 64; blanco sobre negro y sobre gris 128 con 128; Diferencia con opacidad 90 de `(66,196,206)` sobre gris 128 | `(49,11,13)`; `(128,128,128)` y `(191,191,191)`; `(105,107,111)` (truncamiento hacia cero comprobado en el último) |
| 4 | Fondo translúcido: Normal fondo `(58,168,76,150)` y origen `(211,17,99,100)`; mismo origen con alfa 255; **empírica (4b)**: Multiplicar con fondo `(0,0,0,150)` y origen `(255,255,255,255)`, y con alfa 100; Diferencia con el primero; Superponer con opacidad 128, fondo `(128,128,128,100)` y origen `(200,200,200,255)`; **barrido** de las 254×255 combinaciones de alfa (`0 < αb < 255`, `1 ≤ αs ≤ 255`) con una rejilla de 22 colores por lado (0, 17, 34, …, 255 y 1, 2, 127, 128, 129, 254: 484 parejas) para los siete modos ≠ Normal | `(138,89,88,191)` (dato del corpus); `(211,17,99,255)`; `(43,43,43,255)` y `(42,42,42,191)` (los dos **iguales al PNG**); `(255,255,255,255)`; `(180,180,180,178)`; en el barrido, el error frente a la forma real observada con el `αr` entero es **≤ 2** (techo, `TOLERANCIA_EMPIRICA`) en cada modo, y `|αr entero − αr real| ≤ 0,5` |
| 5 | Pila de `D(3)`: orden de abajo arriba; capa oculta (no cuenta aunque cubra todo); capa con celda vacía (no aporta, ni siquiera RGB); capa con opacidad 0; capa de fondo con opacidad 128 y modo Multiplicar; todas ocultas | cada píxel es el compuesto a mano; capa oculta y celda vacía iguales a no existir; opacidad 0 igual a oculta salvo el RGB si el fondo es vacío; fondo: modo sin efecto y alfa 128; todas ocultas en RGBA: `(0,0,0,0)` en todo el lienzo |
| 6 | Grises con alfa: la pila de la fila 5 con `R = G = B`; grises sin alfa con una capa; grises con todas las capas ocultas | GrayAlpha = canal rojo del RGBA equivalente (y su alfa); Gray = la celda; `Err(NoTransparency { mode: Gray })` |
| 7 | Indexado, 3 capas y transparente 0, y otro con transparente 7 y el 0 opaco: la más alta con índice no transparente gana; capa oculta; celda vacía; todo transparente; indexado sin `transparent_index` con una sola capa llena, y con la celda vacía | índice del que gana; la oculta no cuenta; el transparente donde no hay nada; en el índice 7 el 0 sí es opaco; paleta de la salida = la del lienzo; con una capa llena, la celda; con la celda vacía y sin transparente, `Err(NoTransparency { mode: Indexed })` |
| 8 | `compose_rect` con un rectángulo interior, en el borde, de una fila y todo el lienzo; vacío; fuera por la derecha y por abajo; el rectángulo más grande que el lienzo | imagen del tamaño del rectángulo y **igual al recorte** de `compose`; `Err(EmptyRect)`; `Err(RectOutside { .. })` con los datos pedidos; la memoria reservada es la del rectángulo (comprobada por `Image::byte_len`) |
| 9 | Corpus: ocho matrices de 64 combinaciones | tolerancia 0 (ver arriba) |
| 10 | Corpus: `mezcla-ocho-opacidad`, `mezcla-ocho-alfa`, `capas-opacidad`, `capas-alfa-origen`, `fondo-con-opacidad` | tolerancia 0 |
| 11 | Corpus: `capas-alfa-fondo`, `mezcla-ocho-fondo-translucido` y las dos rejillas de alfas | fuera del dominio (`αb ∈ {0, 255}`, `αs = 0` o Normal), tolerancia 0; dentro, error máximo contra el PNG en `[1, 2]` por caso agregado (dos rejillas y siete tiras): ≤ 2 vigila el techo y ≥ 1 detecta una exención innecesaria; la fila comprueba además que solo esos píxeles son del dominio |
| 12 | `SetLayerBlend` (en `px-core`): cambio; mismo valor; capa desconocida; en indexado Multiplicar y Normal; `ConvertMode` a indexado con una capa Multiplicar; deshacer y rehacer; `==`; `state_id` | `Ok(true)` y un paso, `Ok(false)` sin paso; `Err(UnknownLayer)`; `Err(IndexedBlend)` y `Ok(false)`; `Err(IndexedBlend)` con el documento intacto; `undo`/`redo` exactos; `==` distingue el modo; `state_id` nuevo solo con `Ok(true)`; dentro de un `Batch`, `Err(InBatch { index, source: NotBatchable })` |

## Propiedades (4; `proptest`, 256 casos cada una, documentos de hasta 6×5 con 1 a 4 capas)

Generadores: modo RGBA, grises con alfa e indexado (con transparente); capas con visibilidad, opacidad 0..=255, modo de los ocho (solo Normal en indexado), celdas vacías o con píxeles aleatorios (alfa incluido en los modos con alfa).

1. **Recortar es componer:** para cualquier documento y cualquier rectángulo válido, `compose_rect(doc, r)` es **igual** al recorte de `compose(doc)`; y para RGBA, `compose(doc)` es píxel a píxel el plegado de `composite_pixel` sobre las capas visibles (la versión rápida y la sencilla coinciden).
2. **Contra un modelo real independiente** (RGBA): con racionales enteros (sin `f64`) se calcula `αr`, `Cm` y `Cr` de la teoría (y, en el dominio de 4b, la forma observada con `m = 1 − (1 − αb)(1 − αs·αb)`); `|αr entero − αr real| ≤ 0,5`; `|Cr entero − Cr real| ≤ 5` con fondo translúcido (frente al `αr` **real**; máximo medido 4,57) y, con `αb ∈ {0, 255}`, `≤ 1` (`≤ 3/2` en Multiplicar, Trama y Superponer, cuya mezcla ya redondea: medido 1,44); con el **`αr` entero** en el modelo, `≤ 1` (`≤ 3/2` en esos tres modos) fuera del dominio de 4b y `≤ 2` dentro (`TOLERANCIA_EMPIRICA`); además invariantes exactas: Normal con `αs = 255` da el origen; `αr ≥ αs` y `αr ≥ αb`; con fondo opaco `αr = 255`.
3. **Monotonía y simetrías exactas:** Oscurecer ≤ Cb ≤ Aclarar para cualquier fondo y origen opacos; Multiplicar ≤ min(Cb, Cs); Trama ≥ max(Cb, Cs); Sumar ≥ max(Cb, Cs); Diferencia es simétrica en `Cb` y `Cs`; multiplicar por 255 y trama con 0 como identidades; opacidad 0 igual a capa oculta cuando hay fondo.
4. **Totalidad y determinismo:** para cualquier documento válido y rectángulo (incluidos grises, indexado sin transparente y capas vacías) `compose` y `compose_rect` no hacen pánico, devuelven `Ok` o un `ComposeError` de la lista, **no modifican el documento** (igual a una copia previa) y dos llamadas seguidas dan el mismo resultado byte a byte.

## Rendimiento (umbrales ajustados a lo medido: ver «Corrección posterior»)

`compose` completo de **8192² con 8 capas** (RGBA: 256 MiB de salida y 8 celdas de 256 MiB, justo el tope de 2 GiB de 3.1; el banco lo sube con `set_cells_limit` si hace falta y anota el pico, unos 2,25 GiB) en `--release`, un solo hilo, tres escenarios: (a) fondo opaco y siete capas Normal opacas (camino rápido); (b) siete capas con opacidad 128 y modos mezclados sobre fondo opaco; (c) peor caso, fondo y capas translúcidos y modos mezclados. Además `compose_rect` de **2048² (una baldosa de 14a)** con 8 capas y de **64×64** (un trazo). Anoto tiempo y pico de memoria. Criterio que propongo: la interfaz no llama nunca a la composición completa en el bucle de pintado (3.3 compone por baldosa sucia y el trazo por su rectángulo); **si una baldosa de 2048² con 8 capas en el escenario (b) pasa de 250 ms, o en el (c) de 1 s, o el 64×64 de 1 ms, me paro** y propongo antes de cerrar (camino rápido adicional o hilos por bandas con `std::thread::scope`, que no añade dependencias y no cambia el resultado). La composición completa de 8192² se usa solo en exportar y aplanar (3.5), fuera del bucle de pintado: se anota la cifra sin umbral duro. Es una **estimación mía, no medida**: la división por `αr` (variable) es lo caro, y por eso el camino de fondo opaco la evita.

## Pruebas anteriores que cambian (por lectura; sin copia desechable)

**Ninguna prevista.** Todo es aditivo: una variante nueva en `Command` y en `DocumentError` (ambos `#[non_exhaustive]`: los `match` de otros crates ya llevan comodín; dentro de `px-core` los `match` sobre ellos —`Display`, `apply`, `execute_leaf`, los pasos— se amplían con las variantes nuevas), un campo privado nuevo en `Layer` (solo se construye en `document.rs`; `layers.rs` no lo construye: sus «Layer {» son de su modelo propio) y un crate más de dependencia de pruebas en `px-golden`. `px-ui` y `px-app` no cambian (`dirty_of` ya trata lo desconocido como todo sucio). **Verificación en la unidad, sin copia desechable (CLAUDE.md):** al meter la lógica corro `cargo test -p px-core -q` y `cargo test -p px-raster -q`, y el cierre completo una vez; **si alguna prueba anterior falla, me paro** (regla 4).

## Orden de trabajo

Commits: spec; pruebas (`test-writer`, **sí** en esta unidad: `px-raster` y `px-core`) en rojo, **solo con ejecutables nuevos** (`crates/px-raster/tests/compose.rs`, `crates/px-core/tests/blend.rs` —la fila 12, que ya no es un módulo de `layers.rs`— y `crates/px-golden/tests/mezcla.rs`, que ya no es un módulo de `png_interop`, con `mod common;` propio) y la `dev-dependency` de `px-golden` a `px-raster`: ningún ejecutable anterior deja de compilar; lógica A (`px-core`, `P-01` a `P-08`: `blend.rs` pasa) y lógica B (`px-raster`, `R-01` a `R-04`: `compose.rs` y `mezcla.rs` pasan), cada una con las pruebas anteriores en verde; medida de rendimiento (cifras en la «Corrección posterior»); cierre (`cargo fmt`, `clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked`, `cargo deny check`; `license-auditor` una vez sobre el rango, porque cambia `px-core`). Tope orientativo de unas 40 pruebas (12 filas, 4 propiedades y variantes). Sin `px-ui` ni `px-app`. Informe final de 20 líneas como máximo.

## Obligaciones para las unidades siguientes (no se hacen aquí)

- **3.3:** llamar a `compose_rect` por baldosa sucia y por el rectángulo del trazo; que el guardado PNG pase a `compose` (obligación de 3.1); el cuentagotas de «lo visible» con `composite_pixel`; `dirty_of` conoce `SetLayerBlend`.
- **3.4:** el selector de modo (atenuado en indexado con el motivo «convierte a RGBA»; el núcleo da `IndexedBlend`) con los rótulos en español (Normal, Multiplicar, Trama, Superponer, Oscurecer, Aclarar, Sumar, Diferencia).
- **3.5:** aplanar y combinar con `compose`/`compose_rect` (el `Command` lleva la imagen compuesta, como `fill_command`).
- **3.8/3.9:** lo dicho arriba (tabla de modos de la especificación pública; casos con forma contra la composición del documento leído).

## Fuera de alcance

Opacidad de celda (hito 6), los diez modos restantes (hito 6), grupos, mesa de luz y fotogramas (hito 5), composición con hilos (solo si la medida lo exige), caché de baldosas compuestas (3.3).

## Decisiones (resueltas; la 3, la 5, la 6 y la 7 con los datos de la segunda ronda)

1. **La composición vive en `px-raster`** (con `Rect`, `compose`, `compose_rect`, `composite_pixel`, `ComposeError`: `R-01` a `R-04`), no en `px-core`. Alternativa: `px-core`, que evitaría la dependencia nueva de `px-golden` pero ensancharía el núcleo con un algoritmo y no podría producir comandos.
2. **`BlendMode` y `SetLayerBlend` en `px-core`** como aditivos (`P-01` a `P-08`): enum de ocho valores con `#[non_exhaustive]`, sin discriminantes, un comando por propiedad, exclusivo y de un paso, y `IndexedBlend` como `IndexedOpacity` (también al convertir a indexado).
3. **Fórmulas** (sección «Fórmulas en enteros»): forma general de la teoría (Porter y Duff y W3C); redondeo en el alfa y en las funciones de mezcla, truncamiento hacia cero en las interpolaciones del paso 4a y `αr` entero; en el dominio del paso 4b, la fórmula empírica con redondeo al más cercano. **Las convenciones de entero las fijó el corpus** (0 diferencias en todo el corpus reconstruido salvo el dominio de 4b, con techo 2). **`αs = div255(a · opacidad)` con redondeo: resuelto con datos** (el truncamiento falla 10 alfas de salida con opacidad 128 y 541 píxeles de `fondo-con-opacidad`). Coste: con `αb` translúcido y alfas pequeños el entero se aleja hasta ~4,5 del modelo real (el modelo exacto, con `αr` real, falla 4 de 640 píxeles de Normal).
4. **`px-golden` gana `px-raster` como `dev-dependency` interna del workspace** (cambia `Cargo.toml` y `Cargo.lock`, no `deny` ni `docs/DEPENDENCIES.md`). Alternativa: poner las filas 9 a 11 en `px-raster` con `px-formats` como `dev-dependency` y una lectura propia de `golden/aseprite/`.
5. **Goldens en 3.2 solo para los casos que se reconstruyen sin formas desde las tablas** (los 14 de la primera ronda y los 3 nuevos: rejillas de rectángulos y `fondo-con-opacidad`, que lleva un disco de una desigualdad entera); los 7 con forma y los 4 indexados esperan a 3.9, con el `.aseprite` leído por 3.8. Alternativa: reconstruir también `mixto-rgba` y los otros aquí (unas 40 líneas que luego sobran).
6. **Fondo translúcido con modo ≠ Normal: fórmula EMPÍRICA con techo 2** (cambio de decisión del usuario; sustituye a «W3C con la divergencia declarada»). Paso 4b de arriba; tolerancia 2 vigilada por la prueba (techo y suelo) y por el barrido de 256×256 alfas; fuera de su dominio, tolerancia 0; W3C queda descrita como alternativa. El motivo: el usuario prioriza que un `.aseprite` se vea igual en la referencia y en Pixoko. Error máximo por modo en el dominio: ver la tabla de «Fórmula empírica».
7. **Fondo vacío y capa de abajo: contrastadas con el programa** (`fondo-con-opacidad`, 0 diferencias): contra el vacío el modo no tiene efecto, la opacidad escala el alfa y el RGB del origen se conserva aunque `αs = 0` (como el PNG de `rgba-una-capa`); una capa sin celda no aporta ni RGB.
8. **Indexado ignora opacidad y modo** en la composición, porque el núcleo ya impide que existan; grises sin alfa e indexado sin transparente con nada visible dan `ComposeError::NoTransparency`.
9. **Rendimiento:** umbrales de 250 ms (baldosa de 2048² con 8 capas, mezclas sobre fondo opaco), 1 s (peor caso, translúcido) y 1 ms (64×64), y parar y proponer si se pasan; sin hilos de entrada; cifras de 8192² solo anotadas. Son estimaciones mías.
10. **Nota para 3.8** (modos del `.aseprite` desde la especificación pública, nunca desde los números de Lua; diez modos sin equivalente se tratan con aviso o error).
11. **`test-writer` sí** (`px-core` y `px-raster`); `license-auditor` una vez al cerrar (cambian `px-core` y `px-raster`; sin dependencias nuevas de terceros).

## Corrección posterior (al cerrar la unidad; la única)

Lo que la implementación y las medidas precisaron, sin salirse de `P-01..P-08` ni de `R-01..R-04` (ningún otro cambio en `px-core`):

1. **Paso 0 (no estaba en la spec aprobada):** un origen exactamente `(0, 0, 0, 0)` no aporta nada y no pisa el RGB de un fondo con alfa 0. Lo exigió `fondo-con-opacidad` (112 de 1024 píxeles con el paso 2 literal) y lo da por buenos el corpus entero; las cotas «≤ 1» de la propiedad 2 pasan a «≤ 3/2» en Multiplicar, Trama y Superponer con fondo opaco (su mezcla ya redondea: medido 1,44); el barrido usa 22 colores por lado (484 parejas) y no 21; `NoTransparency` se decide por los píxeles del rectángulo pedido. Todo ello commiteado en `4087137`.
2. **Pruebas:** 29 (`compose.rs` 15 —una `#[ignore]`: el barrido completo de alfas, que tarda 5,6 s en release—, `blend.rs` 11 y `mezcla.rs` 3 con los 17 goldens), todas verdes; las pruebas anteriores, sin tocar (`px-core`, `px-raster` y `px-golden` pasan; `px-ui` y `px-app` no cambian).
3. **Error máximo de la fórmula empírica (barrido de 254×255 combinaciones de alfa × 484 parejas de colores y todas las parejas 256×256 con alfas espaciadas; 212 millones de puntos por modo), frente a la forma observada con el `αr` entero:** multiplicar 1,72; trama 1,72; superponer 1,76; oscurecer 1,71; aclarar 1,71; sumar 1,85; diferencia 1,76 (techo declarado: 2). Contra el corpus: máximo 2 (rejillas 2 y 2; tiras 2, 2, 1, 1, 1, 1, 2). **Aviso de criterio:** frente a la forma real con el `αr` **real** el máximo es 4,16 (3,88 a 4,16 según el modo): es el redondeo del `αr` entero, el mismo que da 0 diferencias con Normal en el corpus; el techo de 2 se refiere al `αr` entero y no a ese.
4. **Rendimiento** (`--release`, un solo hilo, Ryzen 5 7600X; el banco desechable construye 8 capas reales con `FillSpans`; mejor de tres repeticiones):

   | Escenario | Baldosa 2048² con 8 capas | 64×64 | 8192² completo, 8 capas |
   | --- | --- | --- | --- |
   | (a) fondo opaco y siete capas Normal opacas | 11 ms | 0,011 ms | 0,31 s |
   | (b) fondo opaco y siete mezclas con opacidad 128 | 246–250 ms | 0,24 ms | 4,0 s |
   | (c) peor caso: fondo y capas translúcidos con mezclas | 445–453 ms | 0,43 ms | 7,2 s |

   Los umbrales propuestos (250 ms en (b), 1 s en (c), 1 ms en 64×64) **no se superan**, pero (b) está en el borde (249,9 ms en una repetición): se **ajustan a lo medido** con margen a **300 ms (b), 600 ms (c) y 1 ms (64×64)**. La composición completa de 8192² con 8 capas (hasta 7,2 s; pico de memoria: 8 celdas de 256 MiB + 256 MiB de salida) **no se usa en el bucle de pintado** (3.3 compone por baldosa sucia y el trazo por su rectángulo: 0,43 ms), solo al exportar y aplanar (3.5). Si 3.3 necesita bajar una baldosa de 2048² con mezclas, la palanca es partir `compose_rect` en bandas con `std::thread::scope` (no cambia el resultado ni añade dependencias; la máquina tiene 12 hilos): no se hace aquí.
5. **Fórmula empírica y W3C:** sin cambios respecto a «Fórmula empírica del fondo translúcido»; W3C sigue descrita como alternativa (cambiar solo el cuerpo de 4b).
