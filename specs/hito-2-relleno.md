# Hito 2 · Unidad 5: relleno por inundación

Estado: **aprobada** (2026-10-17; decisiones 1 a 7 con cuatro matices incorporados al texto: (a) la pasada final solo recorre las filas tocadas y el coste es O(R + N/8); (b) `Image::check_pixel` pasa a ser pública en `px-core`, con sus tests, y es la única comprobación de `Document` y de `px-raster`; (c) la pila se libera antes de reservar la salida, con el pico estimado en 16384² RGBA; (d) pruebas grandes de 1024² en la ejecución normal y las de 2048² en `#[ignore]` si pasan de ~10 s en debug, con los tiempos medidos tras implementar).

## Objetivo

Calcular en `px-raster` la región de píxeles contiguos que comparte el valor de una semilla y rellenarla con un `Pixel`, como un único `Command::FillSpans` de `Document` (unidad 4). Coste en historia: 20 B por tramo, no por píxel. Es la base de la herramienta de relleno (unidad 8).

## Entradas y salidas

- `fill_command(doc: &Document, image: ImageId, x: i32, y: i32, pixel: Pixel) -> Result<Option<Command>, DocumentError>`: solo lectura. `Some(FillSpans { image, spans, pixel })` o `None` si no hay nada que cambiar. `x` e `y` son `i32`, como en `Line`, para que un clic fuera del lienzo se pueda expresar.
- `flood_fill(doc: &mut Document, image, x, y, pixel) -> Result<bool, DocumentError>`: `fill_command` y, solo con `Some`, `doc.apply`. Con `None`, `Ok(false)` sin llamar a `apply` (no toca `can_undo`, `can_redo` ni `history_bytes`). Si no, devuelve lo que devuelva `apply`.
- Errores: `UnknownImage`, `Pixel { pixel: 0, source }` (modo o índice) y `OutOfMemory`. Sin tipo de error nuevo. Sin dependencias nuevas (`proptest` 1.12.0 ya es dev-dependency de `px-raster`).
- **Único cambio en `px-core`:** `Image::check_pixel(&self, pixel: Pixel) -> Result<(), ImageError>` pasa de `pub(crate)` a `pub`, con su doc y sus tests en `crates/px-core/tests/image.rs`. No cambia de firma ni de comportamiento: `ModeMismatch { expected, found }` si el modo no coincide e `IndexNotInPalette { index, len }` si el índice no cabe en la paleta. `Document` ya la usa y `px-raster` la usa en vez de repetir la comprobación. Si algún test del hito 1 hubiera que tocarlo, se para y se avisa.

## Comportamiento

1. **Región.** Los píxeles que se alcanzan desde la semilla moviéndose por **4 vecinos** (arriba, abajo, izquierda, derecha) y que valen **exactamente lo mismo que la semilla**. La de 8 vecinos y la tolerancia de color son del hito 4.
2. **Comparación por valor almacenado** (`Pixel == Pixel`, leído con `Image::get_pixel`):
   - `Indexed`: el índice. Dos índices con el mismo color en la paleta, o con distinto alfa de paleta, son valores distintos.
   - `Rgba`: `r`, `g`, `b` y `a` exactos. Consecuencia: `(0,0,0,0)` y `(9,9,9,0)` son valores distintos aunque ambos sean invisibles.
   - `Gray` y `GrayAlpha`: el valor exacto (en `GrayAlpha`, `gray` y `alpha`).
3. **Orden de comprobaciones, el primero que falle gana:** (1) `UnknownImage`; (2) `Image::check_pixel(pixel)` falla (otro modo o índice fuera de la paleta): `Pixel { pixel: 0, source }`, el mismo error que daría `apply`, porque es la misma función (se comprueba **antes** de inundar para no hacer el trabajo en balde); (3) semilla fuera del lienzo (`x < 0`, `x >= width`, `y < 0`, `y >= height`): `Ok(None)`; (4) `pixel` igual al valor de la semilla: `Ok(None)`. Los pasos 3 y 4 no reservan memoria.
4. **Algoritmo: relleno por tramos con pila explícita, sin recursión.**
   - Un *tramo* es una serie **máxima** de píxeles de la región en una fila. La pila guarda tramos ya completos (`y`, `x_start`, `x_end`: 12 B, `size_of::<Span>()`).
   - Se extiende la semilla a izquierda y derecha hasta el primer píxel distinto o el borde, se marca el tramo en el mapa de visitados y se apila.
   - Al sacar un tramo, se recorre su intervalo `[x_start, x_end)` en las filas `y − 1` e `y + 1`. Cada píxel de esas filas que vale lo de la semilla y no está visitado inicia un tramo nuevo: se extiende a ambos lados **más allá del intervalo si hace falta**, se marca y se apila; y el recorrido sigue tras él.
   - **Invariante:** un píxel visitado tiene visitado todo su tramo máximo. Por eso la extensión solo compara valores, no mira el mapa, y **cada tramo se apila exactamente una vez**.
5. **Mapa de visitados:** un bit por píxel, contiguo, en `Vec<u64>` de `⌈W·H/64⌉` palabras (`W·H ≤ 2³⁰` porque `Image::MAX_BYTES` es 1 GiB, así que el índice cabe en `usize` hasta en 32 bits). Se reserva con `try_reserve_exact` y se pone a cero antes de empezar.
6. **Orden de salida y fusión.** La pila descubre los tramos en un orden que no es el de `FillSpans`. Tras la inundación (con la pila ya liberada) una **pasada final** recorre el mapa de visitados de la fila `y_min` a la `y_max` (se anotan al apilar) y emite cada racha de bits a 1, **partida en los límites de fila**. Eso da los tramos ordenados por `(y, x_start)`, sin solapes, sin vacíos y fusionados (dos tramos máximos nunca están pegados). La pila se **libera (se suelta el `Vec`) antes** de reservar el `Vec<Span>` de salida, que se reserva **exacto** con `try_reserve_exact`: el número de tramos se cuenta al apilarlos. El mapa se libera antes de devolver el comando.
7. **Memoria.** Sea `N = W·H`, `R` los píxeles de la región y `S` el número de tramos. Cada fila de ancho `W` admite como mucho `⌈W/2⌉` tramos (hay un píxel distinto entre dos), así que `S ≤ H·⌈W/2⌉` (≈ `N/2` en lienzos anchos; `N` solo con `W = 1`) y `S ≤ R`.
   - Mapa: `N/8` B (512 KiB en 2048², 32 MiB en 16384², 128 MiB como máximo).
   - Pila: cada tramo se apila una vez, luego **pico ≤ S** entradas = `12·S` B, y la capacidad llega a 2× por el crecimiento amortizado. Antes de cada `push` se llama a `try_reserve(1)`: nunca aborta. La cota es la demostrada; en la práctica el orden LIFO la deja muy por debajo.
   - Tramos de salida: `12·S` B, exactos. **Pico de `fill_command` = imagen + mapa + máximo(pila, salida)**, no la suma, porque la pila se libera antes de reservar la salida. `apply` añade 20 B por tramo en historia (todos los trozos del relleno valen lo de la semilla y son distintos de `pixel`, luego **un trozo por tramo**).
   - Peor caso: el **peine** (una fila llena arriba y dientes de 1 píxel en las columnas pares): `S ≈ N/2`. En 2048²: `S = 2 096 129`, 24 MiB de tramos y 40 MiB de historia. En 16384² con RGBA (el máximo) el pico estimado de `fill_command` en el peine es **imagen 1 GiB + mapa 32 MiB + tramos 1,6 GiB ≈ 2,65 GiB** (la pila del peine es de unos `W/2` = 8192 tramos, despreciable, aunque la cota demostrada es mayor), y con `apply` sobre él se añaden 20 B por tramo de historia (≈ 2,7 GiB) mientras el comando sigue vivo: **≈ 5,3 GiB**. `OutOfMemory` es posible y el documento queda intacto. En Linux, con *overcommit*, `try_reserve` puede tener éxito aunque luego falte memoria real (misma nota que en la línea).
8. **Tiempo: O(R + N/8).** La inundación y la salida cuestan **O(R)**: la extensión lee cada píxel de la región una vez, los barridos de fila leen cada píxel de la región como mucho dos veces y la frontera (como mucho dos píxeles distintos por píxel de región más dos por tramo) otra vez; en total ~5·R lecturas. Más un **término de área**: reservar y poner a cero el mapa (`N/8` B) y la pasada final, que solo recorre las filas entre la mínima y la máxima tocadas por la región (`(y_max − y_min + 1)·W/64` palabras, más `S` rachas). **No es lineal en la región**: con una región de 1 píxel en 16384² cuesta un `memset` de 32 MiB (unos ms). Quitarlo exigiría un mapa disperso, que no compensa.
9. **Sin pánico ni `unsafe`:** ningún `i32` de semilla, ningún lienzo de 1 de ancho o de alto, ningún `Pixel`. La aritmética de índices usa `checked_*` o `try_from`.
10. **El comando vale para el estado con el que se calculó.** `flood_fill` lo aplica de inmediato. Quien llame a `fill_command` y aplique después otros comandos obtiene un relleno válido (los tramos siguen dentro del lienzo), pero no la región del estado nuevo.

## Fuera de alcance

- Conectividad de 8 vecinos y tolerancia de color (hito 4): con 8 vecinos, el barrido de filas vecinas pasa a ser `[x_start − 1, x_end + 1)`; el resto no cambia.
- Relleno global (todos los píxeles de ese valor, aunque no sean contiguos), con patrón o degradado, o limitado a una selección.
- La herramienta y su clic en la ventana (unidad 8). Antialiasing o mezcla de alfa: el relleno escribe el `Pixel`, no lo mezcla.
- Comparar con Grafx2 o LibreSprite: el comportamiento «contiguos y del mismo valor» no tiene casos dudosos, y con tolerancia sí los habrá (hito 4, con dorados).

## Casos límite

Imágenes RGBA salvo que se diga otra cosa. `A` y `B` son dos colores distintos, `C` el color de relleno (distinto de ambos). `sp(y, a, b)` es `Span { y, x_start: a, x_end: b }`. Las matrices se leen por filas, `y` de arriba abajo. «Fuera» es un `Ok(None)` / `Ok(false)`. Las filas 1 a 9 salen de una traza a mano.

| # | Entrada | Resultado |
| --- | --- | --- |
| 1 | 3×2 todo `A`, semilla `(1,1)` | `[sp(0,0,3), sp(1,0,3)]` |
| 2 | 1×1 `A`, semilla `(0,0)` | `[sp(0,0,1)]` |
| 3 | 5×1 todo `A`, semilla `(2,0)` | `[sp(0,0,5)]`: un solo tramo, no partido en la semilla |
| 4 | 2×3 todo `A`, semilla `(1,0)` | `[sp(0,0,2), sp(1,0,2), sp(2,0,2)]`: ninguno cruza de fila |
| 5 | 3×3 `A B A / B A B / A B A`, semilla `(1,1)` | `[sp(1,1,2)]`: las diagonales no cuentan (4 vecinos); las esquinas no cambian |
| 6 | 4×4 tablero (`A` si `x+y` es par, si no `B`), semilla `(1,1)` | `[sp(1,1,2)]`: la región es un píxel |
| 7 | 1×4 columna `A B A A`, semilla `(0,3)` | `[sp(2,0,1), sp(3,0,1)]`: el `A` de arriba queda aislado |
| 8 | 3×3 `A A A / A B A / A B A`, semilla `(2,2)` (se descubre en otro orden) | `[sp(0,0,3), sp(1,0,1), sp(1,2,3), sp(2,0,1), sp(2,2,3)]`: salida ordenada por `(y, x_start)` |
| 9 | 5×3 peine: `A A A A A / A B A B A / A B A B A`, semilla `(4,2)`; y la misma desde `(0,0)` | `[sp(0,0,5), sp(1,0,1), sp(1,2,3), sp(1,4,5), sp(2,0,1), sp(2,2,3), sp(2,4,5)]` las dos veces |
| 10 | Semilla `(-1,0)`, `(W,0)`, `(0,H)`, `(i32::MIN, i32::MAX)` | fuera; el documento y su historia no cambian |
| 11 | `ImageId` desconocido, con semilla fuera del lienzo y con `C` igual al valor de la semilla | `Err(UnknownImage(id))`: gana a `None` |
| 12 | `Pixel::Indexed(0)` sobre una imagen RGBA, con semilla fuera; y `Indexed(5)` con paleta de 3 | `Err(Pixel { pixel: 0, source: ModeMismatch { expected: Rgba, found: Indexed } })`; `Err(Pixel { pixel: 0, source: IndexNotInPalette { index: 5, len: 3 } })`. Gana a «fuera» |
| 13 | `C` igual al valor de la semilla | fuera; `flood_fill` da `Ok(false)` y `can_undo`, `can_redo` e `history_bytes` no cambian |
| 14 | Indexed 3×1 con índices `0 1 0`, paleta `[rojo, rojo, azul]`, semilla `(0,0)`, relleno `Indexed(2)`; y relleno `Indexed(1)` | `[sp(0,0,1)]` (el índice 1 tiene el mismo color, pero es otro valor); y con `Indexed(1)` es un cambio real: `Some`, `Ok(true)` |
| 15 | Rgba `(10,20,30,255)` y `(10,20,30,254)` seguidos, semilla en el primero; y `GrayAlpha{5,0}` junto a `GrayAlpha{6,0}` | solo el píxel de la semilla en cada caso; rellenar el primero con `(10,20,30,254)` es un cambio real |
| 16 | `L`×`L` RGBA, con `L = 1024` en la ejecución normal y `L = 2048` en pruebas `#[ignore]`: (a) uniforme, semilla en el centro; (b) tablero, semilla en el centro; (c) peine (fila 0 llena y columnas pares `A` en el resto), semilla `(0,1)` | (a) `L` tramos `sp(y,0,L)`; (b) 1 tramo; (c) `S = 1 + (L−1)·L/2` tramos (523 777 con 1024; 2 096 129 con 2048). En los tres, `apply` devuelve `true`, `undo` restaura la imagen y `redo` la rellena |

**Tiempo esperado** (estimación sin medir; se sustituye por los tiempos medidos tras implementar, en debug y en `--release`): en `--release`, (a) y (b) por debajo de ~0,2 s y (c) por debajo de ~0,5 s con 2048². En el perfil de test del workspace (sin optimizar), unos pocos segundos con 1024² y varias veces más con 2048². Las pruebas imprimen los tiempos y **solo fallan por encima de 120 s**, que ya delata un coste cuadrático; no se comprueban cifras finas para no crear un test inestable. Las de `L = 1024` se ejecutan siempre. Las de `L = 2048` van en `#[ignore]` (si en debug pasan de ~10 s) y se lanzan con `cargo test --release -- --ignored`; el CI no cambia. (b) es barata en tiempo, pero el mapa se reserva igual (128 KiB con 1024²).

## Propiedades (proptest, imágenes de 1 a 12 de lado)

El generador usa un **fondo de 2 a 4 valores** que incluye casi duplicados (mismo RGB con distinto alfa, índices con el mismo color de paleta, grises iguales con distinto alfa), para que ganen las regiones grandes y la comparación por valor se ejerza. Semilla siempre dentro, salvo en la 5.

1. **Oráculo.** En los cuatro modos, el conjunto de píxeles de los tramos es **idéntico** al de un recorrido ingenuo pixel a pixel con cola (`VecDeque`, 4 vecinos, comparando `get_pixel`); tras `apply`, la imagen es la original con esa región en `pixel` y nada más; `flood_fill` da `Ok(true)` si y solo si `pixel` es distinto del valor de la semilla.
2. **Forma de los tramos.** No vacíos, dentro del lienzo, estrictamente ordenados por `(y, x_start)`, sin solapes, y entre dos de la misma fila `x_end < x_start` del siguiente (fusionados). Cada uno es máximo: el píxel de su izquierda y el de su derecha, si existen, no valen lo de la semilla. Cuenta `≤ H·⌈W/2⌉`.
3. **Historia.** Un solo paso: `undo` deja la imagen igual que la copia previa, `redo` igual que la rellena, y `fill_command` no cambia el documento (copia antes y después).
4. **Repetición y no-ops.** Rellenar dos veces con el mismo `pixel` desde la misma semilla da `Ok(true)` y `Ok(false)`; la segunda no cambia `can_undo`, `can_redo` ni `history_bytes`. Tras un `undo`, un relleno que da `Ok(false)` no borra la pila de rehacer (`can_redo` sigue a `true`).
5. **Errores y semillas arbitrarias.** Con cualquier `i32` de semilla (dentro, fuera y extremos), cualquier `ImageId` y píxeles de cualquier modo: sin pánico; `UnknownImage` gana a `Pixel`, `Pixel` gana a «fuera» y «fuera» gana a «igual»; con cualquier `Err` o `Ok(false)` el documento y su historia no cambian. El error de `Pixel` coincide con el de `apply(SetPixels)` para el mismo píxel sobre una imagen del mismo modo (las dos usan `Image::check_pixel`; la propiedad protege que `fill_command` no cambie el orden ni el contenido del error).
6. **Independencia de la semilla.** Para dos píxeles cualesquiera de la misma región, `fill_command` da exactamente los mismos tramos.

## Decisiones

Todas aprobadas (2026-10-17) con los matices de la cabecera.

1. **Se apilan tramos completos y marcados al descubrirlos (12 B), no semillas por píxel.** Cada tramo entra una vez, así que el pico de la pila está acotado por `S` y los tramos de salida salen máximos sin fusionar. Alternativa: apilar semillas `(x, y)` (8 B) y marcar al sacarlas: más pequeña por entrada, pero puede apilar el mismo tramo varias veces (cota `R + S`, no `S`) y exige fusionar después.
2. **Mapa de visitados de 1 bit por píxel y pasada final por las filas tocadas** (lo que proponías), frente a ordenar los tramos con `sort_unstable` (`O(S log S)`, sin mapa, pero más lento en el peine y sin garantizar la fusión). Coste: un término de área `N/8` B de memoria y de `memset`, así que el tiempo es **O(R + N/8)**, no lineal en la región (comportamiento 8).
3. **Semilla fuera del lienzo es `Ok(None)`, no un error; `x` e `y` son `i32`.** Igual que una línea fuera del lienzo (`Ok(None)`); la herramienta no tiene que filtrar los clics fuera. Orden: `UnknownImage` > `Pixel` > fuera > igual.
4. **`Image::check_pixel` pública en `px-core`; no se duplica la validación** (matiz de la aprobación). `Document` y `px-raster` llaman a la misma función, antes de inundar en el segundo caso. Es el único cambio en `px-core`, con sus tests; la propiedad 5 sigue comparando el error con el de `apply`.
5. **El tablero de ajedrez no es el peor caso de tramos con 4 vecinos.** Allí cada casilla del mismo color es una región de 1 píxel (solo se tocan en diagonal), así que la inundación termina enseguida con 1 tramo. El peor caso de tramos (hasta ~`N/2`, un tramo por diente) es el **peine**. Pruebo los dos: el tablero comprueba que la región se queda en 1 píxel y no se arrastra la diagonal; el peine ejerce la memoria y el tiempo máximos. Con 8 vecinos (hito 4) el tablero sí sería el peor caso.
6. **Lectura con `Image::get_pixel`**, sin API nueva en `px-core`. Es lo más lento del algoritmo (comprueba límites y construye un `Pixel` en cada lectura). Si al medir domina, un acceso por filas en `px-core` sería una unidad aparte, no esta.
7. **Pruebas grandes** (fila 16): 1024² siempre, 2048² en `#[ignore]` si pasan de ~10 s en debug; imprimen tiempos, techo de 120 s; tras implementar se miden y se sustituyen las estimaciones.

## Para la unidad 8 y el hito 4

- La herramienta debe llamar a `flood_fill` con la semilla en coordenadas de lienzo (con signo) y no filtrar «fuera» ni «igual»: ya dan `Ok(false)` sin tocar la historia.
- La tolerancia de color cambia el predicado de «mismo valor» y deja de ser cierto que cada trozo de historia sea un tramo (los valores anteriores pueden diferir dentro de un tramo); el coste por tramo de 20 B pasa a ser por trozo.
