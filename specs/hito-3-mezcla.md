# Hito 3 · Unidad 3.2: mezcla de capas (composición pura)

Estado: **aprobada con pausa** (2026-10-21). Aprobados `P-01` a `P-08` y los cambios de `px-raster` tal como están; resto de decisiones aprobadas (composición en `px-raster`, `BlendMode` sin discriminantes, goldens de 14 rectángulos en 3.2 y el resto en 3.9, `px-golden` con `px-raster` como `dev-dependency`, indexado sin opacidad ni modo, `NoTransparency`, nota de 3.8). **Pausa antes de implementar** para resolver con datos tres incertidumbres: el fondo translúcido con modo ≠ Normal (decisión 6), el redondeo de `αs = a·opacidad` (decisión 3) y la capa de abajo con opacidad (decisión 7). Los datos son tres casos nuevos del corpus (`corpus-hito-3.md`, «Casos añadidos»: `mezcla-alfa-rejilla-multiplicar`, `mezcla-alfa-rejilla-superponer` y `fondo-con-opacidad`), pendientes de generar; después se actualiza esta spec (si hay fórmula cerrada con 0 diferencias se adopta; si no, W3C con la divergencia como constante y motivo). Los umbrales de rendimiento se miden al cerrar y se ajustan a lo medido (si se pasan, se para). Lo que sigue es el texto escrito antes de la pausa.
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

Con la fórmula estándar (W3C) un fondo con alfa 150, origen opaco blanco y Multiplicar sobre negro da `(105, 105, 105)`; el PNG tiene `(43, 43, 43)`. No es redondeo: el peso del color de origen en la referencia es `(1 − αb)·(1 − αs·αb)` en vez de `(1 − αb)` (ajuste a los dos pares `(αs, αb)` que hay, `(255, 150)` y `(100, 150)`; error real máximo 1,5, pero ninguna de las 243 combinaciones de redondeo que probé da 0, así que ni siquiera el ajuste cierra). Con solo dos pares no hay datos para fijar una fórmula en todo el dominio. **Decisión D-A** abajo.

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

1. `αs = div255(a · opacidad)`.
2. **Fondo con alfa 0** (`αb == 0`, incluido el inicio): resultado `(Cs, αs)`. No se mezcla nada (no hay con qué), **también con `αs == 0`: el RGB del origen se conserva** (el PNG de `rgba-una-capa` lo conserva; es lo que hace que una sola capa se componga idéntica a su celda).
3. **Origen con `αs == 0`** (y `αb > 0`): el resultado es el fondo, sin cambios.
4. En otro caso: `αr = αs + αb − div255(αs · αb)` y, por canal,
   - `B  = blend(modo, Cb, Cs)` (tabla);
   - `Cm = Cs + (B − Cs) · αb / 255`;
   - `Cr = Cb + (Cm − Cb) · αs / αr`.
   El resultado es `(Cr, αr)`. Con `αb = 255`: `αr = 255` y `Cm = B`; con Normal: `Cm = Cs`; con `αs = 255`: `Cr = Cm`. `αr ≥ αs ≥ 1`: no hay división por cero. Los canales quedan en 0..=255 (combinación convexa).

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

**Capa de fondo.** Es solo una posición (3.1): su opacidad y su modo se aplican contra el fondo vacío, así que el modo no tiene efecto (paso 2) y la opacidad escala su alfa. Ningún caso del corpus tiene una capa de abajo con opacidad < 255 o modo ≠ Normal: **no está contrastado con el programa**, solo con vectores.

**Error frente al modelo real** (comprobado por barrido): con `αb ∈ {0, 255}` el entero está a ≤ 0,5 del real en la mezcla y a ≤ 1 en el resultado; con un fondo translúcido, a ≤ 4,5 (muestra de 10 colores × todos los alfa impares), solo con alfas pequeños (`αr` pequeño amplifica el redondeo de `αr`). Es el precio de igualar al corpus a tolerancia 0 (la fórmula con `αr` exacto falla 4 de 640 píxeles de `capas-alfa-fondo` y de la tira Normal). La propiedad 2 fija esas dos cotas.

**Grises con alfa:** las mismas fórmulas con un solo canal (los modos son separables: da lo mismo que `R = G = B`). **Grises sin alfa:** una sola capa posible (3.1: `NoTransparency`), ver abajo.

## Indexado y grises (D06)

Sin opacidad ni mezcla: el resultado es, por píxel, el índice de la **capa visible más alta cuya celda tiene imagen y cuyo índice no es el transparente**; si no hay ninguno, el índice transparente. El núcleo ya garantiza opacidad 255 (`IndexedOpacity`, 3.1) y, desde `P-04`, modo Normal (`IndexedBlend`): **una capa indexada con opacidad ≠ 255 no existe**, y por eso la composición **no la mira** (la ignora; sin fila de prueba porque no se puede construir; la propiedad 2 de 3.1 vigila la invariante). Grises sin alfa usa la misma regla sin valor transparente (gana la capa visible más alta). Si no queda ningún píxel que dar y el modo no tiene valor transparente (indexado sin `transparent_index` o grises con todas las capas ocultas o vacías), `ComposeError::NoTransparency { mode }` (como en 3.1, en vez de inventar un índice). La paleta de la salida es la del lienzo.

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

Los PNG se leen con `px_formats::read_png` (RGBA 8 bits; `Image::bytes()` da los bytes) y las **entradas se reconstruyen en la prueba** desde las tablas `A` y `B` y las reglas de `generar.lua` (rectángulos y columnas/filas de color; sin formas): `.aseprite` no se puede leer hasta 3.8. Las pruebas viven en **`px-golden`** (módulo nuevo `png_interop/mezcla.rs` del ejecutable `png_interop` existente, junto a `aseprite_corpus`), porque ya sabe encontrar `golden/aseprite/` y leer PNG; para eso `px-golden` gana `px-raster` como `dev-dependency` (**dependencia interna del propio workspace**: no entra en `cargo deny` ni en `docs/DEPENDENCIES.md`, pero cambia `Cargo.toml` y `Cargo.lock`; decisión 4).

| Filas | Casos del corpus | Cómo |
| --- | --- | --- |
| 9 | `mezcla-normal`, `-multiplicar`, `-trama`, `-superponer`, `-oscurecer`, `-aclarar`, `-sumar`, `-diferencia` | `compose` del documento reconstruido (2 capas, 32×32) = PNG, **tolerancia 0** en los 8 |
| 10 | `mezcla-ocho-opacidad`, `mezcla-ocho-alfa`, `capas-opacidad`, `capas-alfa-origen` | ídem, **tolerancia 0** en los 4 |
| 11 | `capas-alfa-fondo` y `mezcla-ocho-fondo-translucido` | `capas-alfa-fondo` y la tira Normal (columnas 0 a 7) exactas; las **siete tiras restantes de `mezcla-ocho-fondo-translucido` no se comparan con el PNG** (divergencia conocida, D-A): una constante `DIVERGENCIA_CONOCIDA` con ese caso y esas columnas, con el motivo, y la fila comprueba que **solo** esas columnas están exentas |

Lo que no entra aquí y pasa a **3.9**, cuando 3.8 ya lee los `.aseprite` (con tolerancia 0, medido arriba): `mixto-rgba`, `capas-normal-tres`, `capas-ocultas`, `celda-desplazada`, `celda-recortada`, `celda-vacia`, `rgba-una-capa` y los cuatro indexados (hay más formas y desplazamientos de celda, que el modelo de 3.1 recorta al leer; reconstruirlos aquí sería código de usar y tirar). **Si un caso muestra ±1 al implementar**, se para, se anota en esta spec **caso por caso** (qué caso, qué modo, qué píxeles, qué valor da la fórmula y cuál el PNG, y por qué) y se decide con esos datos; nunca se sube una tolerancia general. Más de ±1 es un fallo de la fórmula, no de redondeo (es lo que pasa con la tira de D-A, y por eso se plantea aparte).

**Para 3.8 (nota):** los códigos del modo de mezcla del `.aseprite` se leen de **la especificación pública del formato** (la tabla de modos de la capa) con un `match` propio hacia `BlendMode`, y **jamás de los números del enum de Lua** que `corpus-hito-3.md` (sección 5, punto 3) anota solo como curiosidad. Los modos de la especificación que no están en `BlendMode` (diez) los trata 3.8 con aviso o error, no con un valor por defecto en silencio.

## Casos (12 filas)

Notación: `D(n)` es un documento RGBA con `n` capas construido con los comandos existentes y `SetLayerBlend`; «fondo» es el color ya compuesto debajo; `div255` y `/` como arriba. Filas 1 a 8 en `crates/px-raster/tests/compose.rs` (ejecutable nuevo); filas 9 a 11 en `px-golden`; fila 12 en `crates/px-core/tests/layers.rs` (el ejecutable de 3.1, módulo nuevo).

| # | Entrada | Resultado |
| --- | --- | --- |
| 1 | `composite_pixel`, identidades y bordes: origen `αs = 0` sobre fondo opaco; opacidad 0; origen opaco Normal; fondo `(0,0,0,0)` con origen `(232,198,52,0)` y con `(10,20,30,200)` Multiplicar opacidad 255 y 100 | fondo sin cambios `(196,46,52,255)`; ídem; origen exacto; `(232,198,52,0)` (RGB conservado); `(10,20,30,200)` y `(10,20,30,78)` (sin mezcla contra el vacío) |
| 2 | Los ocho modos con fondo `(196,46,52,255)` y origen `(90,60,220,255)`, opacidad 255 (valores de la sección 5 del corpus, hechos a mano) | Normal `(90,60,220)`; Multiplicar `(69,11,45)`; Trama `(217,95,227)`; Superponer `(179,22,90)`; Oscurecer `(90,46,52)`; Aclarar `(196,60,220)`; Sumar `(255,106,255)`; Diferencia `(106,14,168)`; Superponer en el umbral: fondo 127 y 128 con origen 100 → 100 y 101 |
| 3 | Opacidad sobre fondo opaco: rojo `(196,46,52)` sobre negro con 64; blanco sobre negro y sobre gris 128 con 128; Diferencia con opacidad 90 de `(66,196,206)` sobre gris 128 | `(49,11,13)`; `(128,128,128)` y `(191,191,191)`; `(105,107,111)` (truncamiento hacia cero comprobado en el último) |
| 4 | Fondo translúcido: Normal fondo `(58,168,76,150)`, origen `(211,17,99,100)`; mismo origen con alfa 255; Multiplicar con fondo `(0,0,0,150)` y origen `(255,255,255,255)` y con alfa 100 | `(138,89,88,191)` (dato del corpus); `(211,17,99,255)`; `(105,105,105,255)` y `(54,54,54,191)` (W3C, **no** el PNG: ver D-A) |
| 5 | Pila de `D(3)`: orden de abajo arriba; capa oculta (no cuenta aunque cubra todo); capa con celda vacía (no aporta, ni siquiera RGB); capa con opacidad 0; capa de fondo con opacidad 128 y modo Multiplicar; todas ocultas | cada píxel es el compuesto a mano; capa oculta y celda vacía iguales a no existir; opacidad 0 igual a oculta salvo el RGB si el fondo es vacío; fondo: modo sin efecto y alfa 128; todas ocultas en RGBA: `(0,0,0,0)` en todo el lienzo |
| 6 | Grises con alfa: la pila de la fila 5 con `R = G = B`; grises sin alfa con una capa; grises con todas las capas ocultas | GrayAlpha = canal rojo del RGBA equivalente (y su alfa); Gray = la celda; `Err(NoTransparency { mode: Gray })` |
| 7 | Indexado, 3 capas y transparente 0, y otro con transparente 7 y el 0 opaco: la más alta con índice no transparente gana; capa oculta; celda vacía; todo transparente; indexado sin `transparent_index` con una sola capa llena, y con la celda vacía | índice del que gana; la oculta no cuenta; el transparente donde no hay nada; en el índice 7 el 0 sí es opaco; paleta de la salida = la del lienzo; con una capa llena, la celda; con la celda vacía y sin transparente, `Err(NoTransparency { mode: Indexed })` |
| 8 | `compose_rect` con un rectángulo interior, en el borde, de una fila y todo el lienzo; vacío; fuera por la derecha y por abajo; el rectángulo más grande que el lienzo | imagen del tamaño del rectángulo y **igual al recorte** de `compose`; `Err(EmptyRect)`; `Err(RectOutside { .. })` con los datos pedidos; la memoria reservada es la del rectángulo (comprobada por `Image::byte_len`) |
| 9 | Corpus: ocho matrices de 64 combinaciones | tolerancia 0 (ver arriba) |
| 10 | Corpus: `mezcla-ocho-opacidad`, `mezcla-ocho-alfa`, `capas-opacidad`, `capas-alfa-origen` | tolerancia 0 |
| 11 | Corpus: `capas-alfa-fondo` y `mezcla-ocho-fondo-translucido` | `capas-alfa-fondo` y la tira Normal, tolerancia 0; siete tiras exentas **solo** esas, con constante y motivo |
| 12 | `SetLayerBlend` (en `px-core`): cambio; mismo valor; capa desconocida; en indexado Multiplicar y Normal; `ConvertMode` a indexado con una capa Multiplicar; deshacer y rehacer; `==`; `state_id` | `Ok(true)` y un paso, `Ok(false)` sin paso; `Err(UnknownLayer)`; `Err(IndexedBlend)` y `Ok(false)`; `Err(IndexedBlend)` con el documento intacto; `undo`/`redo` exactos; `==` distingue el modo; `state_id` nuevo solo con `Ok(true)`; dentro de un `Batch`, `Err(InBatch { index, source: NotBatchable })` |

## Propiedades (4; `proptest`, 256 casos cada una, documentos de hasta 6×5 con 1 a 4 capas)

Generadores: modo RGBA, grises con alfa e indexado (con transparente); capas con visibilidad, opacidad 0..=255, modo de los ocho (solo Normal en indexado), celdas vacías o con píxeles aleatorios (alfa incluido en los modos con alfa).

1. **Recortar es componer:** para cualquier documento y cualquier rectángulo válido, `compose_rect(doc, r)` es **igual** al recorte de `compose(doc)`; y para RGBA, `compose(doc)` es píxel a píxel el plegado de `composite_pixel` sobre las capas visibles (la versión rápida y la sencilla coinciden).
2. **Contra un modelo real independiente** (RGBA): con racionales enteros (sin `f64`) se calcula `αr`, `Cm` y `Cr` de la teoría; `|αr entero − αr real| ≤ 0,5`; `|Cr entero − Cr real| ≤ 1` si `αb ∈ {0, 255}` y `≤ 5` si no; además invariantes exactas: Normal con `αs = 255` da el origen; `αr ≥ αs` y `αr ≥ αb`; con fondo opaco `αr = 255`.
3. **Monotonía y simetrías exactas:** Oscurecer ≤ Cb ≤ Aclarar para cualquier fondo y origen opacos; Multiplicar ≤ min(Cb, Cs); Trama ≥ max(Cb, Cs); Sumar ≥ max(Cb, Cs); Diferencia es simétrica en `Cb` y `Cs`; multiplicar por 255 y trama con 0 como identidades; opacidad 0 igual a capa oculta cuando hay fondo.
4. **Totalidad y determinismo:** para cualquier documento válido y rectángulo (incluidos grises, indexado sin transparente y capas vacías) `compose` y `compose_rect` no hacen pánico, devuelven `Ok` o un `ComposeError` de la lista, **no modifican el documento** (igual a una copia previa) y dos llamadas seguidas dan el mismo resultado byte a byte.

## Rendimiento (medido en la implementación; umbrales que propongo)

`compose` completo de **8192² con 8 capas** (RGBA: 256 MiB de salida y 8 celdas de 256 MiB, justo el tope de 2 GiB de 3.1; el banco lo sube con `set_cells_limit` si hace falta y anota el pico, unos 2,25 GiB) en `--release`, un solo hilo, tres escenarios: (a) fondo opaco y siete capas Normal opacas (camino rápido); (b) siete capas con opacidad 128 y modos mezclados sobre fondo opaco; (c) peor caso, fondo y capas translúcidos y modos mezclados. Además `compose_rect` de **2048² (una baldosa de 14a)** con 8 capas y de **64×64** (un trazo). Anoto tiempo y pico de memoria. Criterio que propongo: la interfaz no llama nunca a la composición completa en el bucle de pintado (3.3 compone por baldosa sucia y el trazo por su rectángulo); **si una baldosa de 2048² con 8 capas en el escenario (b) pasa de 250 ms, o en el (c) de 1 s, o el 64×64 de 1 ms, me paro** y propongo antes de cerrar (camino rápido adicional o hilos por bandas con `std::thread::scope`, que no añade dependencias y no cambia el resultado). La composición completa de 8192² se usa solo en exportar y aplanar (3.5), fuera del bucle de pintado: se anota la cifra sin umbral duro. Es una **estimación mía, no medida**: la división por `αr` (variable) es lo caro, y por eso el camino de fondo opaco la evita.

## Pruebas anteriores que cambian (por lectura; sin copia desechable)

**Ninguna prevista.** Todo es aditivo: una variante nueva en `Command` y en `DocumentError` (ambos `#[non_exhaustive]`: los `match` de otros crates ya llevan comodín; dentro de `px-core` los `match` sobre ellos —`Display`, `apply`, `execute_leaf`, los pasos— se amplían con las variantes nuevas), un campo privado nuevo en `Layer` (solo se construye en `document.rs`; `layers.rs` no lo construye: sus «Layer {» son de su modelo propio) y un crate más de dependencia de pruebas en `px-golden`. `px-ui` y `px-app` no cambian (`dirty_of` ya trata lo desconocido como todo sucio). **Verificación en la unidad, sin copia desechable (CLAUDE.md):** al meter la lógica corro `cargo test -p px-core -q` y `cargo test -p px-raster -q`, y el cierre completo una vez; **si alguna prueba anterior falla, me paro** (regla 4).

## Orden de trabajo

Commits: spec; pruebas (`test-writer`, **sí** en esta unidad: `px-raster` y `px-core`) en rojo, solo `compose.rs`, el módulo de `layers.rs` y `mezcla.rs` dejan de compilar; lógica A (`P-01` a `P-08`, workspace verde); lógica B (`px-raster`, `R-01` a `R-04`); medida de rendimiento (cifras en la «Corrección posterior»); cierre (`cargo fmt`, `clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked`, `cargo deny check`; `license-auditor` una vez sobre el rango, porque cambia `px-core` y `px-raster`). Tope orientativo de unas 40 pruebas (12 filas, 4 propiedades y variantes). Sin `px-ui` ni `px-app`. Informe final de 20 líneas como máximo.

## Obligaciones para las unidades siguientes (no se hacen aquí)

- **3.3:** llamar a `compose_rect` por baldosa sucia y por el rectángulo del trazo; que el guardado PNG pase a `compose` (obligación de 3.1); el cuentagotas de «lo visible» con `composite_pixel`; `dirty_of` conoce `SetLayerBlend`.
- **3.4:** el selector de modo (atenuado en indexado con el motivo «convierte a RGBA»; el núcleo da `IndexedBlend`) con los rótulos en español (Normal, Multiplicar, Trama, Superponer, Oscurecer, Aclarar, Sumar, Diferencia).
- **3.5:** aplanar y combinar con `compose`/`compose_rect` (el `Command` lleva la imagen compuesta, como `fill_command`).
- **3.8/3.9:** lo dicho arriba (tabla de modos de la especificación pública; casos con forma contra la composición del documento leído).

## Fuera de alcance

Opacidad de celda (hito 6), los diez modos restantes (hito 6), grupos, mesa de luz y fotogramas (hito 5), composición con hilos (solo si la medida lo exige), caché de baldosas compuestas (3.3).

## Decisiones que necesito que confirmes

1. **La composición vive en `px-raster`** (con `Rect`, `compose`, `compose_rect`, `composite_pixel`, `ComposeError`: `R-01` a `R-04`), no en `px-core`. Alternativa: `px-core`, que evitaría la dependencia nueva de `px-golden` pero ensancharía el núcleo con un algoritmo y no podría producir comandos.
2. **`BlendMode` y `SetLayerBlend` en `px-core`** como aditivos (`P-01` a `P-08`): enum de ocho valores con `#[non_exhaustive]`, sin discriminantes, un comando por propiedad, exclusivo y de un paso, y `IndexedBlend` como `IndexedOpacity` (también al convertir a indexado).
3. **Fórmulas** (sección «Fórmulas en enteros»): forma general de la teoría (Porter y Duff y W3C); redondeo en el alfa y en las funciones de mezcla, truncamiento hacia cero en las interpolaciones y `αr` entero. **Las convenciones de entero las fijó el corpus** (0 diferencias en 19 de los 20 casos reconstruidos; en el vigésimo, la tira Normal también); la de `αs = div255(a · opacidad)` (redondeo) **no la distingue el corpus del truncamiento** (ambos dan 0): la dejo en redondeo por simetría. Coste: con `αb` translúcido y alfas pequeños el entero se aleja hasta ~4,5 del modelo real (el modelo exacto, con `αr` real, falla 4 de 640 píxeles del corpus). Si prefieres el modelo exacto y aceptar esos 4, dilo.
4. **`px-golden` gana `px-raster` como `dev-dependency` interna del workspace** (cambia `Cargo.toml` y `Cargo.lock`, no `deny` ni `docs/DEPENDENCIES.md`). Alternativa: poner las filas 9 a 11 en `px-raster` con `px-formats` como `dev-dependency` y una lectura propia de `golden/aseprite/`.
5. **Goldens en 3.2 solo para los casos de rectángulos que se reconstruyen desde las tablas (14 casos)**; los 7 con forma y los 4 indexados esperan a 3.9, con el `.aseprite` leído por 3.8. Alternativa: reconstruir también `mixto-rgba` y los otros aquí (unas 40 líneas de `disco`, `triángulo` y `diagonal` que luego sobran).
6. **D-A, fondo translúcido con modo ≠ Normal.** Recomiendo **dejar la forma W3C de arriba y declarar la divergencia como caso conocido** (la tira Normal exacta, las otras siete exentas de la comparación con el PNG por constante con su motivo; 715 de 896 píxeles distintos, máximo 62). Alternativas: (B) **ajustar la fórmula a lo observado** (peso `(1 − αb)(1 − αs·αb)`, error real ≤ 1,5, pero sale de ver la salida de un programa y no de la teoría, tiene solo dos pares `(αs, αb)` de datos y aún no cierra a tolerancia 0 con ningún redondeo que he probado); haría falta un caso de corpus nuevo con una rejilla de alfas (el corpus está en el tope de 25: habría que fusionar dos) y se podría hacer más adelante cambiando solo ese camino. (C) Pararme aquí hasta tener ese caso. El efecto práctico es raro (un fondo translúcido **debajo** de una capa con modo): con fondo opaco, que es lo habitual en pixel art, hay tolerancia 0 en todos los modos.
7. **Fondo vacío:** el modo no tiene efecto y el RGB del origen se conserva aunque `αs = 0` (como el PNG de `rgba-una-capa`); una capa sin celda no aporta ni RGB. La capa de abajo con opacidad < 255 o modo ≠ Normal **no está contrastada con el programa** (no hay caso en el corpus); si quieres contrastarla, es un caso nuevo (mismo tope de 25).
8. **Indexado ignora opacidad y modo** en la composición, porque el núcleo ya impide que existan; grises sin alfa e indexado sin transparente con nada visible dan `ComposeError::NoTransparency`.
9. **Rendimiento:** umbrales de 250 ms (baldosa de 2048² con 8 capas, mezclas sobre fondo opaco), 1 s (peor caso, translúcido) y 1 ms (64×64), y parar y proponer si se pasan; sin hilos de entrada; cifras de 8192² solo anotadas. Son estimaciones mías.
10. **Nota para 3.8** (modos del `.aseprite` desde la especificación pública, nunca desde los números de Lua; diez modos sin equivalente se tratan con aviso o error).
11. **`test-writer` sí** (`px-core` y `px-raster`); `license-auditor` una vez al cerrar (cambian `px-core` y `px-raster`; sin dependencias nuevas de terceros).
