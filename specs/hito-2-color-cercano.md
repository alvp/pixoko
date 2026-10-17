# Hito 2 · Unidad 8: color más cercano en una paleta

Estado: **aprobada** (2026-10-17; decisiones 1 a 5 tal cual, con los matices del visto bueno incorporados al texto: sección «Para la unidad 9», cuatro filas calculadas a mano y escritas explícitamente, diferencias en `i64` y oráculo en `i128` con la forma expandida, y tests en debug para que un desbordamiento sea un pánico). Las filas de «Casos límite» salen de un modelo en Python de la fórmula del comportamiento 2 (aparte del repo); las cuatro marcadas con «a mano» se calcularon además a mano (derivación bajo la tabla).

## Objetivo

Añadir a `Palette` (`px-core`) la búsqueda de la entrada más parecida a un `Rgba`. La usará la unidad 9 (conversión RGBA → indexado). Es matemática entera pura sobre tipos que ya existen, sin dependencias.

## Entradas y salidas

- `Palette::nearest(&self, color: Rgba) -> Option<u8>`: el índice de la entrada de menor distancia a `color`, o `None` si la paleta está vacía.
- No hay `Result`: toda paleta y todo color tienen respuesta. El índice cabe en `u8` porque hay como mucho 256 entradas.
- `color` se toma tal cual: es el valor que muestra un píxel, no algo que haya que resolver. Las entradas, en cambio, se comparan por su valor **resuelto** (`resolved`, ver comportamiento 3).
- La función de distancia es privada; la API pública no cambia más que con este método.

## Comportamiento

1. **Resultado.** `nearest` devuelve `Some(i)` con la distancia mínima; con varias entradas a la misma distancia mínima, el índice más bajo (comportamiento 4). Con la paleta vacía devuelve `None`. Nunca hay pánico ni modifica la paleta.
2. **Distancia: euclídea al cuadrado sobre el color premultiplicado, solo con enteros.** Para un color resuelto `(r, g, b, a)` se define el vector `v = (r·a, g·a, b·a, 255·a)`, con cada componente en `[0, 65025]`. La distancia entre `p` y `q` es `Σ (v_p − v_q)²` sobre las cuatro componentes; es decir, `Σ_c (c_p·a_p − c_q·a_q)² + 255²·(a_p − a_q)²` con `c` en `r, g, b`. Sin coma flotante, sin raíz (comparar cuadrados da el mismo orden) y sin división: el resultado es idéntico en Linux, Windows y macOS.
   - **Rango.** Cada diferencia está en `[−65025, 65025]` y su cuadrado en `[0, 4 228 250 625]`, que cabe en `u32` por poco; la suma de cuatro llega como máximo a `4·65025² = 16 913 002 500`, que **no** cabe en `u32` (máximo 4 294 967 295) y cabe de sobra en `i64`. Se calcula en `i64`: componentes, diferencias, cuadrados y suma (el máximo, 1,7·10¹⁰, es una fracción mínima de `i64::MAX`, 9,2·10¹⁸): sin desbordamiento posible, ni en debug ni en release.
   - **Por qué premultiplicado.** Un color con poco alfa apenas se ve, así que su RGB debe pesar poco. Con la distancia plana en RGBA, un píxel `(0,0,0,0)` estaría más cerca de un negro opaco `(0,0,0,255)` que de un transparente `(255,255,255,0)`, que es justo lo contrario de lo que pide el alfa. Premultiplicando, cualquier color con `a = 0` es el vector cero sea cual sea su RGB, y no hace falta ningún caso especial: el comportamiento 3 sale de la fórmula.
   - **Por qué el término `255·a`.** Sin él, el transparente y el negro opaco (RGB 0) estarían a distancia cero. Con él, transparente frente a negro opaco vale `(255·255)²`, igual que la diferencia de un canal entero (negro frente a rojo puro).
   - **Opacos.** Si `p` y `q` son opacos (`a = 255`), el término de alfa vale 0 y la distancia es `65025·Σ (Δc)²`: el orden es exactamente el de la distancia euclídea al cuadrado en RGB.
   - **Sin ponderar.** Los tres canales pesan igual. Ponderaciones perceptuales (pesos 2-4-3, «redmean», Lab) son una mejora posterior y no cambian la API.
3. **Alfa e índice transparente.** Las entradas se miden por `resolved(i)`: la entrada del índice transparente cuenta con `a = 0` aunque tenga guardado un alfa mayor.
   - Un color con `a = 0` está a distancia 0 de toda entrada resuelta con `a = 0`, con cualquier RGB: elige la **primera** entrada transparente (por alfa guardado `0` o por ser el índice transparente), si existe.
   - Su RGB no influye: `nearest(Rgba(r, g, b, 0))` da lo mismo para todo `r, g, b`, y el RGB de una entrada resuelta a `a = 0` tampoco influye.
   - Si no hay entrada transparente, un color con `a = 0` es el vector cero y va a la entrada más cercana al vector cero: entre entradas opacas, la más oscura (menor `r² + g² + b²`); si hay semitransparentes, puede ganar una de ellas (la de menor `Σ (c·a)² + (255·a)²`). Es consecuencia del premultiplicado y se documenta, no se corrige aquí; la conversión de imágenes no puede apoyarse en ello (ver «Para la unidad 9»).
   - Un color opaco igual al guardado en el índice transparente no encuentra ese índice, porque la imagen lo muestra transparente.
   - Fijar el índice transparente equivale a sustituir esa entrada por su valor resuelto en una paleta sin índice (propiedad 5).
4. **Empates y duplicados.** Se recorre en orden de índice y solo una distancia estrictamente menor sustituye a la mejor: gana el índice más bajo. Con colores duplicados devuelve el primero. Un color exacto devuelve el primer índice cuyo valor resuelto es equivalente (mismo RGBA, o ambos con `a = 0`): si no hay duplicados ni transparentes, devuelve su propio índice. Se permite cortar el recorrido en la primera distancia 0, porque ninguna posterior puede mejorarla ni ganar el empate.
5. **Coste.** Recorrido lineal sobre como mucho 256 entradas, sin reservar memoria: una llamada son unas pocas miles de operaciones enteras. Una caché de colores ya vistos o un índice espacial son de una unidad posterior si hace falta: la 9 (conversión de imágenes grandes, con pocos colores distintos) decidirá si añade una caché `Rgba → u8` por conversión. Esta unidad no la añade.

## Fuera de alcance

- Caché, índice espacial (k-d, octree) y búsquedas por lotes.
- Distancias perceptuales (Lab, luma de Rec. 709, «redmean») y pesos por canal.
- Devolver la distancia, o excluir la entrada transparente de la búsqueda.
- Tramado (dithering), cuantización y generación de paletas.
- La conversión de imágenes (unidad 9).

## Para la unidad 9

El caso «`a = 0` sin entrada transparente» (comportamiento 3) existe en `nearest` solo para que la función sea total. **La conversión RGBA → indexado no puede resolverlo en silencio mapeando esos píxeles a la entrada más oscura**: un píxel invisible pasaría a ser visible (negro, o lo que haya más cerca del vector cero) sin que nadie lo decida. La unidad 9 debe fijar la política y dejarla escrita en su spec, con una de estas dos vías (o una combinación, con su error):

- **Añadir una entrada transparente** si hay hueco (la paleta tiene menos de 256 entradas): `push` de un color con `a = 0`, o marcar una existente con `set_transparent_index`, y mapear los píxeles `a = 0` a ella.
- **Rechazar y explicar** si no hay hueco (256 entradas sin ninguna transparente) y la imagen tiene algún píxel con `a = 0`: un error con la cantidad de píxeles afectados, sin imagen parcial.

También le corresponde decidir qué pasa con los píxeles semitransparentes (`0 < a < 255`) cuando la paleta no tiene alfa parcial: `nearest` los acerca a transparentes u opacos según el umbral del comportamiento 2, y la conversión debe decir que eso es una pérdida.

## Casos límite

Notación: `P[...]` es una paleta con esas entradas, con colores `Rgba(r, g, b, a)` escritos `(r,g,b,a)`; «índice transp.» es `set_transparent_index`; «sin» significa que no se fija.

| Paleta | Consulta | Resultado |
| --- | --- | --- |
| `Palette::new()` | cualquier color, incluido `(0,0,0,0)` | `None` |
| `P[(0,0,0,0)]` (una entrada) | `(255,255,255,255)` (distancia máxima, 16 913 002 500) | `Some(0)` |
| `R = P[(255,0,0,255), (0,255,0,255), (0,0,255,255)]` | cada una de sus tres entradas; la de en medio, `(0,255,0,255)` (a mano) | `Some(0)`, `Some(1)`, `Some(2)`; distancias `8 456 501 250`, `0`, `8 456 501 250` |
| `R` | `(200,30,30,255)` (opaco intermedio, a mano); `(10,10,240,255)` | `Some(0)` con distancias `313 745 625`, `5 951 413 125`, `5 951 413 125`; `Some(2)` |
| 256 grises `(i,i,i,255)`, `i` en `0..=255` | `(v,v,v,255)` para cada `v`; `(254,255,255,255)` | `Some(v)` (incluido `Some(255)`); `Some(255)` |
| `P[(0,0,0,255), (2,0,0,255)]` y `P[(2,0,0,255), (0,0,0,255)]` | `(1,0,0,255)` (empate exacto) | `Some(0)` en las dos |
| `P[(9,9,9,255), (5,5,5,255), (5,5,5,255)]` | `(5,5,5,255)` | `Some(1)` (el primero de los duplicados) |
| `P[(0,0,0,255), (255,255,255,0)]` (a mano) | `(0,0,0,0)` | `Some(1)`, con distancias `4 228 250 625` (el negro opaco: solo el término de alfa, `65025²`) y `0` (la transparente) |
| `P[(10,20,30,0), (10,20,30,255)]`; `P[(1,2,3,0), (9,9,9,0)]` | `(200,100,50,0)`; `(9,9,9,0)` | `Some(0)`; `Some(0)` (el RGB no cuenta con `a = 0`: el exacto no devuelve su propio índice) |
| `P[(1,1,1,255), (5,5,5,0), (7,7,7,0)]` | `(0,0,0,0)` sin; con índice transp. `0` | `Some(1)`; `Some(0)` (la entrada 0 ya cuenta como transparente y es la primera) |
| `P[(0,0,0,255), (255,255,255,255)]`; `P[(200,0,0,255), (9,8,7,255)]`, ambas sin | `(255,255,255,0)`; `(0,0,0,0)` | `Some(0)`; `Some(1)` (sin transparente, va a la más oscura) |
| `Q = P[(200,0,0,255), (9,8,7,255)]` sin | `(200,0,0,255)`; `(0,0,0,0)` | `Some(0)`; `Some(1)` |
| `Q` con índice transp. `0` | `(0,0,0,0)`; `(200,0,0,255)` | `Some(0)`; `Some(1)` (su valor guardado ya no la encuentra) |
| `Q` con índice transp. `1` | `(9,8,7,255)`; `(0,0,0,0)` | `Some(0)`; `Some(1)` |
| `E = P[(255,255,255,255), (255,255,255,0)]` (a mano) | `(255,255,255,a)` con `a` = 127; `a` = 128 | `Some(1)`; `Some(0)` (umbral entre 127 y 128: ver la derivación) |
| `S = P[(255,0,0,128), (255,0,0,255), (0,0,0,0)]` | `(255,0,0,130)`; `(255,0,0,250)`; `(0,0,255,10)` | `Some(0)`; `Some(1)`; `Some(2)` |

Las cuatro filas «a mano» salen de la fórmula del comportamiento 2, sin el modelo:

1. **Exacto.** La consulta `(0,255,0,255)` tiene `v = (0, 65025, 0, 65025)`. La entrada 1 es idéntica: distancia 0. La entrada 0 tiene `v = (65025, 0, 0, 65025)`: diferencias `(65025, 65025, 0, 0)` y distancia `2·65025² = 8 456 501 250`; la entrada 2 es simétrica. Gana la 1.
2. **Opaco intermedio.** Todo es opaco, así que la distancia es `65025·Σ (Δc)²`. Con `(200,30,30)`: frente a `(255,0,0)`, `(55, 30, 30)` da `3025 + 900 + 900 = 4825` y `65025·4825 = 313 745 625`; frente a `(0,255,0)`, `(200, 225, 30)` da `40000 + 50625 + 900 = 91525`, y frente a `(0,0,255)`, `(200, 30, 225)` da lo mismo; `65025·91525 = 5 951 413 125`. Gana la 0.
3. **Alfa 0 con entrada transparente.** La consulta `(0,0,0,0)` es el vector cero. La entrada 1 (`a = 0`) también: distancia 0. La entrada 0, `(0,0,0,255)`, tiene `v = (0, 0, 0, 65025)`: distancia `65025² = 4 228 250 625`. Gana la 1 aunque el RGB de la consulta coincida con el de la 0.
4. **Umbral 127/128.** Para la consulta `(255,255,255,a)`, `v = (255a, 255a, 255a, 255a)`. El blanco opaco tiene `v = (65025, 65025, 65025, 65025)`: cada componente difiere en `255·(255 − a)` y la distancia es `4·255²·(255 − a)²`. La entrada transparente es el vector cero: cada componente difiere en `255a` y la distancia es `4·255²·a²`. Se igualan cuando `255 − a = a`, es decir `a = 127,5`, que no es un `u8`: con `a = 127` las distancias son `4 261 478 400` (opaco) y `4 195 152 900` (transparente), gana la 1; con `a = 128` son `4 195 152 900` y `4 261 478 400`, gana la 0. No hay empate posible en ningún `a` entero.

## Criterios de aceptación

Desde la raíz, en Linux (el CI cubre los otros dos sistemas):

- `cargo fmt --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `cargo deny check` (no se añade ninguna dependencia: `proptest` ya está como dev-dependency de `px-core`)

Los tests de unidades anteriores no cambian. Si alguno hubiera que tocar, se para y se avisa. Los tests de esta unidad corren en el perfil de test (debug, con `overflow-checks`): el workspace no define ningún `[profile.*]` que lo cambie, así que un desbordamiento aparecería como pánico. No se ejecutan con `--release` para dar la unidad por buena.

## Tests

En `crates/px-core/tests/palette_nearest.rs` (archivo nuevo; integración sobre la API pública, como en `Palette` y `Hsl`):

**Unitarios**: una prueba por fila de la tabla de casos límite (cada fila puede ser varias aserciones). La paleta de 256 grises se construye con `(0..=255).map(...)`.

**Propiedades** (`proptest`). Los colores se generan de dos formas, mezcladas: uniformes en `u8` y de un conjunto pequeño (`0, 1, 127, 128, 255` por canal y alfa) para forzar empates, duplicados y alfa 0. Las paletas pequeñas tienen de 0 a 8 entradas, con índice transparente o sin él; las grandes, de 0 a 256, con menos casos.

1. **Oráculo.** Un oráculo ingenuo escrito en el test, sin compartir código con la implementación (ni la función de distancia, ni la forma de la fórmula), calcula, para cada entrada `i` y su `resolved(i)`, la distancia con la forma expandida `Σ_c (c_p·a_p − c_q·a_q)² + 255²·(a_p − a_q)²` en `i128` (la implementación usa el vector premultiplicado en `i64`), y elige el mínimo por (distancia, índice). `nearest` coincide con él en paletas pequeñas y grandes, con la paleta vacía dando `None`, y el índice devuelto es `< len`. (El oráculo comprueba que la implementación calcula la fórmula sin desbordar; que la fórmula sea buena lo cubren las filas de la tabla.)
2. **Exacto devuelve el primero equivalente.** Para cualquier paleta no vacía y cualquier `i < len`, `nearest(resolved(i))` es `Some(j)` con `j` el menor índice cuyo `resolved` es igual a `resolved(i)` o tiene `a = 0` igual que él. En particular `j <= i`, y `j == i` si no hay duplicados ni transparentes.
3. **Opacos como RGB.** Si la paleta y la consulta son opacas (`a = 255`, sin índice transparente), el resultado coincide con el mínimo de `Σ (Δc)²` en RGB por índice más bajo.
4. **Alfa 0.** Con una consulta de `a = 0` y al menos una entrada resuelta con `a = 0`, el resultado es la primera de ellas, sea cual sea el RGB de la consulta.
5. **Resuelto equivale a sustituir.** Para cualquier paleta, índice transparente `t < len` y consulta, el resultado de la paleta con `set_transparent_index(Some(t))` es igual al de una copia sin índice transparente donde la entrada `t` es `resolved(t)`.
6. **Invariancia del RGB transparente.** Para cualquier paleta, consulta de `a = 0` y cualesquiera `r, g, b`, `nearest(Rgba(r, g, b, 0))` no cambia; y sustituir el RGB de toda entrada con `a = 0` por otro cualquiera no cambia el resultado de ninguna consulta.

No hay dorado: la métrica es una definición propia de la unidad (matemática pública, distancia euclídea sobre color premultiplicado), no el comportamiento de un programa de referencia. No se copia ni se traduce ningún algoritmo de búsqueda de color de otros editores. Si más adelante hubiera que igualar el resultado de la conversión de Grafx2 o LibreSprite, se haría ejecutando el programa y guardando la salida en `golden/`, con una unidad propia.

## Decisiones

Marcadas con «confirmar» las que necesito que apruebes.

1. **Métrica premultiplicada con término de alfa** (confirmar). Es una sola fórmula, sin ramas, y de ella salen la preferencia por transparentes y el RGB irrelevante con `a = 0`. La alternativa es la euclídea plana en RGBA con un caso especial para `a = 0`, más simple de contar pero discontinua: un `a = 1` con RGB cualquiera ya compararía el RGB como si importara.
2. **Canales sin ponderar** (confirmar). Peso 1 en R, G y B, también para la unidad 9. Un ajuste perceptual posterior solo cambia constantes y las filas de la tabla.
3. **Sin entrada transparente, `a = 0` va a la entrada opaca más oscura** (confirmar). Es consecuencia de la decisión 1. Alternativa: que un `a = 0` sin transparente use el RGB de la consulta; pero ese RGB no significa nada en un píxel invisible, y la conversión ya pierde su alfa.
4. **«Color exacto devuelve su índice» con matiz** (confirmar). Devuelve el primer índice equivalente: con duplicados, el primero; con `a = 0`, la primera entrada transparente aunque tenga otro RGB (fila `(9,9,9,0)`).
5. **El índice transparente se compara resuelto** (confirmar). Un píxel opaco igual al valor guardado en esa entrada no la elige. Es lo que muestra la imagen.
6. **`i64` para componentes, diferencias y suma** (aprobado), con corte en la primera distancia 0. Sin `i128` en el código; solo en el oráculo del test, con la forma expandida de la fórmula.
7. **Sin dependencias nuevas, sin dorado y sin caché.** La caché la decide la unidad 9 con la imagen delante.
