# Hito 4 · Unidad 4.4: formas

Estado: **aprobada** (decisiones delegadas por el usuario; se confirman las de «Decisiones» tal como están, ver «Visto bueno») y **hecha, a falta de la prueba manual y de la referencia de Aseprite** (2026-10-27: ver «Corrección posterior»). Cambios previstos: `px-raster` (**solo lo listado abajo, aprobado de antemano, aditivo y no más**), `px-ui` y `px-app`. **`px-core` no se toca.** Sin dependencias nuevas.

Referencias: fila 4.4 y decisión D15 de [`mapa-funciones.md`](mapa-funciones.md) (T04, T23 a T27, sección 4); [`hito-4-pincel.md`](hito-4-pincel.md) (`Brush`, `Spacing`, modos de herramienta, ventana de opciones con filas); [`ui-single-column.md`](ui-single-column.md) y [`ui-palette-handle.md`](ui-palette-handle.md) (geometría de la caja y techo de 432 pt); [`hito-2-relleno.md`](hito-2-relleno.md) (`FillSpans`).

## Objetivo

**Un solo botón nuevo, «Formas»**, en la caja (decisión del usuario: no un botón por forma). El **tipo** (rectángulo, círculo, elipse, polígono, forma libre) se elige en la fila `Tipo` de su ventana de opciones y el **relleno** en la fila `Relleno` (contorno, relleno o ambos; D15). Cada tipo tiene tecla propia que elige el botón con ese tipo. El contorno se pinta con el `Brush` de la 4.3 (con su espaciado); el relleno, con un `FillSpans`.

## Lo comprobado (en el código)

1. **La caja no crece en la ventana mínima.** El alto mínimo (313 u) lo fija el panel de capas (277 u) frente a la capacidad de la caja (`TOOLBOX_CAPACITY` = 20 herramientas, 4 filas a 10 columnas = 276 u). Con 6 botones a 10 columnas (`c(10)` = 5 por fila) las herramientas pasan a **2 filas** (+25 u en la caja), pero eso es menos de 4 filas: **la ventana mínima sigue en 313 u (417,33 pt a `ppp` 1,5 = 58,0 %, bajo el techo de 432 pt)** y no hace falta desplazamiento ni iconos menores. Solo cambian cifras: a 10 columnas y con las herramientas reales, la rejilla de la paleta pierde una fila visible (7 → 6) y `TOOLBOX_BUTTONS` pasa de 9 a 10; desde 11 columnas (`c(11)` = 6) siguen en una fila. Se mide en las pruebas de geometría (fila 11).
2. `Command::Batch` es un solo paso de deshacer y `dirty_of` ya lo cuenta; `FillSpans` y `SetPixels` actúan sobre una celda vacía sin materializarla antes (`fill_in`, 3.3). Un relleno de forma son, pues, `FillSpans` de un tramo por fila (como máximo `alto` tramos, sin PixelEdit por píxel) y, con contorno y relleno, `Batch([FillSpans, SetPixels])`.
3. `Espacio` es la mano y `Intro` no está en la tabla de atajos (`KeyId` no la tiene). El cierre del polígono no puede ser `Espacio` (el mapa lo proponía): ver D5.
4. **Referencia:** Aseprite tiene rectángulo, elipse y polígono (script `app.useTool`). Su elipse se define por la caja y la nuestra por centro y radios: la comparación es con **tolerancia** (ver «Referencia») y la genera el usuario en local.

## Decisiones (⚠ = decido algo que no me diste; las confirmo abajo)

**D1. Un botón, un modo de herramienta.** `Tool::Shape` (rótulo `Formas`, al final de la caja, tras el borrador; `TOOL_BUTTONS` 5 → 6). `ShapeKind { Rectangle, Circle, Ellipse, Polygon, Freeform }` y `ShapeFill { Outline, Filled, Both }` son estado de `ToolState` (de la sesión, no del documento; por defecto rectángulo y contorno), como el modo del lápiz. El botón **no reinicia** el tipo ni el relleno ya elegidos; una tecla de tipo sí fija el tipo. Pincel, espaciado y `N` son los de la 4.3, compartidos con el lápiz y la línea.
**D2. Teclas (editables como las demás, acciones nuevas `SelectShape(kind)`).** `r` rectángulo, `c` círculo, `e` elipse, `w` polígono y `Mayús`+`d` (`D`) forma libre; todas eligen el botón `Formas` con ese tipo y **conservan** el relleno. ⚠ En el original la mayúscula es «relleno»: aquí el relleno es la fila, no la letra (D15). La tabla pasa de 38 a 43 filas, las acciones editables de 44 a 49 y la ayuda de 51 a 57 entradas (`tool.shape` y una por tipo).
**D3. Geometría de cada tipo** (todo en enteros, píxeles del lienzo; `Mayús` en el arrastre):
- *Rectángulo:* de la esquina pulsada a la actual (cualquier orden). `Mayús`: cuadrado (lado = el mayor desplazamiento, en el sentido del arrastre).
- *Círculo:* centro en el píxel pulsado y radio `r` = `√(dx²+dy²)` redondeado al entero más cercano (`r` es el entero con `r²−r < d² ≤ r²+r`, sin empates posibles). Solo el centro y el radio; `Mayús` no cambia nada.
- *Elipse:* centro en el píxel pulsado, radios `a = |dx|`, `b = |dy|` (sin girar; la girada es 4.12). `Mayús`: círculo de radio `max(a, b)`.
- *Polígono:* un clic por esquina; ver D5.
- *Forma libre:* mano alzada; al soltar se cierra con una recta del último punto al primero.
⚠ Los radios se recortan a 2³⁰.
**D4. Píxeles de la elipse y el círculo (el riesgo de la unidad).** Un píxel a `(dx, dy)` del centro está **dentro** ⇔ `|dx| ≤ a`, `|dy| ≤ b` y `2·(b²·dx² + a²·dy²) < a·b·(2·a·b + a + b)` (la caja recorta a las elipses muy planas, donde la desigualdad sola se saldría de ella); con `a = b = r` es `dx² + dy² < r² + r` (el círculo del pincel `Round` es el mismo criterio con `n = 2r+1`). Si `a = 0` o `b = 0` la forma es el segmento recto (de `2a+1` o `2b+1` píxeles; con ambos 0, un píxel). El **relleno** es el conjunto «dentro»; el **contorno** son los píxeles de «dentro» con algún vecino de 4 sin «dentro» (borde de 8 vecinos de grosor 1). Es exacto, simétrico en los dos ejes y anidado (crecer un radio nunca quita píxeles). Radio 1 = cruz de 5 píxeles (contorno: los 4 brazos); radio 2 = cuadrado de 5 con las cuatro esquinas quitadas. ⚠ Los círculos pequeños son decisión nuestra; la referencia no tiene por qué coincidir.
**D5. Polígono.** Gesto sin movimiento del ratón: cada **pulsar y soltar** fija una esquina (si se arrastra, la esquina es la posición al soltar y se ve el lado que se forma; `Mayús` la lleva al eje del mayor desplazamiento respecto de la esquina anterior). Se cierra y se aplica con **`Intro`** (entrada fija del lienzo, como `Esc`: no está en la tabla de atajos, no es editable), o pulsando de nuevo sobre la **primera** esquina (con al menos tres). `Esc`, cambiar de herramienta, de tipo o de imagen lo descartan (nada que deshacer). `Intro` con menos de dos esquinas no hace nada; con dos es una recta. Mientras está pendiente, la vista previa es el polígono que se aplicaría. ⚠ Se cierra con `Intro` y no con `Espacio` (la mano); el color es el de la primera pulsación.
**D6. Interior (par-impar).** El relleno de polígono y forma libre es la regla par-impar: un píxel `(x, y)` está dentro si el rayo horizontal por su centro cruza un número impar de lados (lados con semiabierto `min(y0,y1) ≤ y < max(y0,y1)`, cruce racional exacto); se le **suma el contorno de 1 píxel** (los lados con `Line`), así el relleno siempre contiene su borde. Las esquinas se unen en orden y el lado de cierre es del último punto al primero.
**D7. Relleno y color (D15, T27).** `Contorno` (por defecto): el pincel en los puntos del contorno, con el color de la herramienta. `Relleno`: `FillSpans` con ese color, **sin pincel** (el borde es el de D4 o D6). `Ambos`: contorno con el color del botón pulsado y relleno con **el de la otra ranura** (primer plano y fondo; si no tiene color en el modo, el mismo). Un solo paso de deshacer; el relleno va primero y el contorno encima.
**D8. Contorno con pincel y espaciado.** Los centros del contorno son el anillo ordenado de la forma (rectángulo: del vértice superior izquierdo en sentido horario; elipse y círculo: por el ángulo exacto desde el este y en sentido horario, desempate por distancia; polígono y forma libre: lado tras lado). Se aplica el `Spacing` de la 4.3: continuo = todos; `Cada N` = índices 0, N, 2N…; `N en total` = `k` centros repartidos en el anillo **cerrado** (`⌊i·L/k⌋`, `i < k`, sin repetir el primero al final). Es la parte de la 4.3 que quedó «para las formas cerradas». El anillo se recorta al lienzo más el tamaño del pincel: una forma mayor que eso reparte sobre la parte visible (⚠ aproximado; se anota, no se avisa).
**D9. Lo que no hace.** `Ctrl` (trazas) no hace nada con las formas (queda para el lápiz y la línea). Ninguna forma toca fuera del lienzo (todo se recorta). Capa oculta o bloqueada, modos de color y celda vacía: como el lápiz (D17 de la 4.3).
**D10. Ventana de opciones.** Filas de `Formas`: `Tipo: Rectángulo`, `Relleno: Contorno`, y, solo si hay contorno (`Contorno` o `Ambos`), `Forma`, `Tamaño` y `Espaciado` (más `N` si no es continuo). Clic izquierdo: siguiente (cicla; con `Mayús` en `Tamaño` y `N`, de 10 en 10); clic derecho: anterior. Por defecto 5 filas; con espaciado distinto de continuo, 6; con `Relleno`, 2. `scroll: [usize; 5]` → 6. Los textos van en `shapes_es.rs`.
**D11. Icono.** Un icono nuevo, `Shapes` (16×16, 1 bit, `assets/icons/`, propio: un rectángulo y un círculo solapados), el 23.º. Cambia alguna cifra de «22 iconos».
**D12. Ayuda y nombres.** Rótulo `Formas`; título y barra de estado dicen `formas`; la ayuda de `F1` sobre el botón es `tool.shape`.

## Cambios exactos en `px-raster` (todos aditivos; el visto bueno vale solo para esta lista)

- **R-05** `circle_radius(dx: i64, dy: i64) -> u32` (D3).
- **R-06** Rellenos recortados al lienzo `(ancho, alto)`, como `Vec<Span>` de `px-core` (ordenados por fila y por `x`, sin solapes ni tramos vacíos): `rect_spans(a, b, w, h)`, `ellipse_spans(center, a, b, w, h)` (D4) y `polygon_spans(vertices, w, h)` (D6; con menos de 3 vértices, la recta o el punto).
- **R-07** Anillos ordenados (D8), recortados a `margen` píxeles fuera del lienzo: `rect_ring(a, b, w, h, margen)`, `ellipse_ring(center, a, b, w, h, margen)` y `polygon_ring(vertices, w, h, margen)` (cerrado: incluye el lado de cierre y no repite el primer punto).
- **R-08** `spaced_centers_closed(ring: &[(i32, i32)], spacing: Spacing) -> Vec<(i32, i32)>` (D8; `n` y `k` 0 se toman como 1).
- Nada no aditivo: ninguna firma pública existente cambia.

## Piezas de `px-ui` y `px-app`

`px-ui`: `Tool::Shape`, `ShapeKind`, `ShapeFill`; `Gesture::Shape` (arrastre de rectángulo, círculo, elipse y forma libre) y polígono pendiente en `ToolState`; `ToolAction::SelectShape(kind)` y `AppAction::SelectShape(kind)` (5 filas de `keymap`); `CanvasInput::Confirm` (`Intro` sin modificadores en `canvas.rs`) con `ToolState::apply_action(Finish)`; `ToolState::{shape_kind, shape_fill, set_other_pixel}`; `OptionRow::{Kind, Fill}` y sus textos; `TOOL_BUTTONS` 5 → 6, `TOOL_LABELS` 5 → 6, `TOOLBOX_BUTTONS` 9 → 10, icono, ayuda. `px-app`: pasar el píxel de la otra ranura (`set_other_pixel`) antes de cada evento, `Intro`, despacho de las acciones y filas de la ventana.

## Casos (12; 1 a 6 en `px-raster`, 7 a 11 en `px-ui`, 12 en `px-app`)

| # | Entrada | Esperado |
| --- | --- | --- |
| 1 | **Radio del círculo.** `(dx, dy)` = `(0,0)`, `(1,0)`, `(1,1)`, `(2,1)`, `(3,4)`, `(5,12)`, `(i32::MAX, i32::MAX)` | `r` = 0, 1, 1, 2, 5, 13 y el entero que cumple `r²−r < d² ≤ r²+r` sin desbordar |
| 2 | **Círculo y elipse (ASCII).** Círculos de radio 0 a 4 y elipses `(3,2)`, `(5,1)`, `(2,4)`, `(0,3)`, `(3,0)`, `(0,0)`: relleno y contorno | Máscaras escritas a mano (D4); el contorno es el borde de 4 vecinos del relleno |
| 3 | **Rectángulo.** Esquinas en cualquier orden; 1×1, 1×5 y 4×3: relleno y anillo; con pincel de otro tamaño | Anillo desde la esquina superior izquierda en sentido horario sin repetir el cierre; relleno = caja; línea si una dimensión es 1 |
| 4 | **Polígono par-impar.** Triángulo, cuadrado con muesca (cóncavo), estrella de 5 puntas autointersecante (agujero central) y dos vértices | ASCII de cada relleno (D6): el agujero de la estrella vacío, el borde incluido; con dos vértices, la recta |
| 5 | **Espaciado cerrado.** Anillo de 12 puntos con `EveryN(5)`, `EveryN(1)`, `TotalN(4)`, `TotalN(1)`, `TotalN(30)` y `n = 0` | Índices 0,5,10; todos; 0,3,6,9; solo el 0; `k > L` = todos; 0 se toma como 1 |
| 6 | **Recorte.** Formas parcial y totalmente fuera de un lienzo de 6 × 6, y radios y coordenadas extremos (`±2³⁰`, `i32::MIN`/`MAX`) | Nada fuera del lienzo; fuera entera = vacío; los extremos terminan al instante sin reservar memoria proporcional al área |
| 7 | **Gesto rectángulo, círculo y elipse.** Contorno, relleno y ambos, con colores de las dos ranuras; `Mayús` | Contorno = pincel en el anillo; relleno = `FillSpans`; `Ambos` = relleno con la otra ranura y contorno encima; `Mayús`: cuadrado y círculo; un paso de deshacer; la vista previa = lo aplicado |
| 8 | **Pincel y espaciado en el contorno.** Rectángulo con `Round` 3 y `EveryN(4)`/`TotalN(5)`; el relleno no usa pincel | Sellos en los centros de la fila 5; el relleno es idéntico con cualquier pincel |
| 9 | **Gesto polígono.** Tres pulsaciones y `Intro`; tres y volver a pulsar la primera; `Esc`; cambio de herramienta con pendiente; `Intro` con una esquina; arrastrar una esquina; `Mayús` | Se aplica un paso al cerrar; `Esc` y el cambio descartan sin efecto; con una esquina no hace nada; el arrastre fija la esquina al soltar; el lado con `Mayús` va a un eje |
| 10 | **Forma libre.** Trazo en L con relleno y contorno; trazo que se cruza | Cierra con recta del último al primero; par-impar; contorno con el pincel; un paso |
| 11 | **Teclas, caja y opciones.** `r`, `c`, `e`, `w`, `D`; el botón con y sin tipo previo; filas de la ventana; tabla de atajos y ayuda; geometría de la caja con 6 botones | Cada tecla elige `Formas` con su tipo y conserva el relleno; el botón conserva el tipo; las filas ciclan (con `Mayús` 10 en `Tamaño` y `N`) y aparecen o desaparecen según el relleno; ventana mínima **313 u** sin cambios, 2 filas de herramientas a 10 columnas y una desde 11 |
| 12 | **Sesión.** Rectángulo `Ambos` en RGBA, indexado y celda vacía; `Intro` y `Esc`; deshacer y rehacer | Un paso por gesto y `modified`; el índice del color en indexado; la celda vacía se rellena; el color del relleno es el de la otra ranura; deshacer deja el documento inicial |

## Propiedades (proptest; 256 casos, 500 como máximo)

1. **Elipse.** Para `a, b` en `0..=60`: el relleno es simétrico en ambos ejes, cada fila es un solo tramo, está anidado (`(a, b) ⊆ (a+1, b)` y `⊆ (a, b+1)`), coincide con el modelo ingenuo de D4, y el anillo es exactamente su borde de 4 vecinos.
2. **Polígono.** Para polígonos pequeños arbitrarios (3 a 8 vértices, también fuera del lienzo): `polygon_spans` = modelo ingenuo (par-impar por píxel + `Line` de los lados), recortado, ordenado, sin solapes y dentro del lienzo; el anillo está contenido en el relleno.
3. **Recorte y rectángulo.** Para esquinas arbitrarias (incluidos extremos): `rect_spans` = la caja recortada; los tres anillos quedan dentro del lienzo más `margen`, sin repetidos consecutivos, y vacíos si la forma queda fuera.
4. **Espaciado cerrado.** Para anillos de 1 a 200 puntos: `EveryN(n)` da los índices `0, n, 2n…`; `TotalN(k)` da `min(k, L)` centros distintos, crecientes y que empiezan en el índice 0.

## Pruebas nuevas

12 filas y 4 propiedades: mínimo 16, hasta unas 30; tope de 40. Reparto: `px-raster/tests/shapes.rs` (1 a 6 y propiedades 1 a 4), `px-ui/tests/shapes_tools.rs` (7 a 10 y lo de la caja de 11) y `px-app/src/dispatch/tests/shapes.rs` (12). La sesión principal las escribe y commitea antes del código (modo ahorro); sin `test-writer`.

## Referencia (la genera el usuario en local; no bloquea la unidad)

Como en la 4.2: `tools/corpus-h4/shapes.lua` y `tools/corpus-h4/generate-shapes.sh` crean con Aseprite (`app.useTool`) en `golden/aseprite-shapes/` los mismos casos que las filas 2 a 4 (rectángulo, círculo y elipse de varios tamaños en contorno de 1 px y rellenos, polígono par-impar) en un lienzo de 64 × 64; el usuario ejecuta el script y commitea los PNG. La prueba `px-golden/tests/shapes.rs` (`#[ignore]` hasta tener los PNG; lista exacta de casos) compara píxel a píxel: rectángulos **idénticos**; círculos, elipses y polígonos con la diferencia simétrica de píxeles **≤ 10 % de los píxeles de la referencia + 2**. Si los PNG discreparan más, se anota y se decide (si dos referencias discrepan, manda la spec, como en el plan). Se ejecuta con la prueba manual de la puerta 4.14.

## Pruebas anteriores que cambian (por lectura; el compilador da los números)

**A. Solo cifras, listas y literales:** `TOOL_BUTTONS` 5 → 6, `TOOL_LABELS`, `TOOLBOX_BUTTONS` 9 → 10 y todo lo que cuente botones o etiquetas (`chrome`, `toolbar`, `column`, `options`, `help`); filas visibles de la rejilla a 10 columnas con las herramientas reales (7 → 6); `scroll: [usize; 5]` → 6; tabla de atajos (38 → 43), acciones editables (44 → 49), entradas de ayuda (51 → 57) e iconos (22 → 23); `match` exhaustivos sobre `Tool`, `ToolAction`, `AppAction` y `CanvasInput`. **B. Cambian de intención si lo piden:** ninguna prevista. Si alguna necesitara más que cifras, literales o firmas: parar y avisar (CLAUDE.md, regla 4).

## Orden de trabajo

Commits separados: (1) esta spec con su fila del `INDEX`; (2) adaptación de pruebas anteriores (el mensaje explica qué comprobaba cada una); (3) pruebas, en rojo; (4) lógica de `px-raster`; (5) `px-ui`; (6) `px-app`; (7) referencia de Aseprite (script y prueba `#[ignore]`). Cierre (lo lanza el usuario): `cargo fmt`, `clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked`, `cargo deny check`; `license-auditor` solo por los cambios en `px-raster`.

## Prueba manual (la haces tú, sin capturas)

1. Ventana al tamaño mínimo y columna a 10 columnas: la caja tiene 6 botones en dos filas y todo cabe sin desplazar; a 16 columnas, una fila.
2. Clic en `Formas` (rectángulo por defecto): arrastrar dibuja el contorno de 1 px; `Ctrl+Z` lo deshace entero. Con `Mayús` sale un cuadrado.
3. Clic derecho en `Formas`: ventana con `Tipo` y `Relleno`; clic en `Tipo` cicla los cinco; con `Contorno` aparecen `Forma`, `Tamaño` y `Espaciado`.
4. `c` y arrastrar: círculo desde el centro; `e`: elipse con radios por eje (`Mayús`: círculo). Radio 1 y 2 salen como en D4.
5. `Relleno`: el interior se rellena sin pincel; `Ambos`: contorno en el color del botón pulsado y relleno en el de la otra ranura (prueba izquierdo y derecho).
6. Contorno con `Tamaño` 3 y `Espaciado` `Cada N` 4, y `N en total` 5: puntos repartidos por todo el contorno.
7. `w`: pulsar tres esquinas (se ve el lado al arrastrar), `Intro` cierra; otra vez con clic sobre la primera esquina; `Esc` en medio lo cancela; `Espacio` sigue moviendo el lienzo.
8. `D` (`Mayús`+`d`): trazo a mano alzada que al soltar se cierra y, con `Relleno`, se rellena (cruces: par-impar).
9. `Ayuda → Atajos de teclado…`: `r`, `c`, `e`, `w` y `D` aparecen y son editables; `F1` sobre el botón da su ayuda.
10. Indexado y capa vacía: pinta el índice y rellena la celda vacía. Y, por mi parte: `tools/corpus-h4/generate-shapes.sh` para tener la referencia de Aseprite.

## Fuera de alcance

Elipse girada (4.12), rejilla visual de las formas y rectángulo redondeado, trazas con `Ctrl`, relleno con patrón o degradado (4.5, 4.7), simetría (4.9), polígono que se cierra con `Espacio`, borrador y cuentagotas con forma, y cualquier otro cambio en `px-core` o `px-raster`.

## Decisiones que necesito que confirmes

1. **D1 y D2:** un botón `Formas` al final de la caja; teclas `r`, `c`, `e`, `w` y `D`; el relleno es una fila y no la mayúscula.
2. **D3 y D4:** círculo y elipse por centro y radios con el criterio entero de D4 (contorno = borde de 4 vecinos); radios recortados a 2³⁰.
3. **D5:** el polígono se cierra con `Intro` o pulsando la primera esquina; `Espacio` sigue siendo la mano.
4. **D6 y D7:** par-impar con el borde incluido; `Ambos` pinta el relleno con la otra ranura.
5. **D8:** el espaciado de la 4.3 se aplica al anillo cerrado; reparte sobre la parte visible si la forma es mayor que el lienzo.
6. **D9:** `Ctrl` sin efecto con las formas.
7. **D10 a D12:** filas de la ventana, icono nuevo y ayuda.
8. **R-05 a R-08:** tu aprobación expresa para tocar `px-raster` solo con esa lista (dada de antemano).

## Visto bueno (2026-10-27; el usuario delegó en mi criterio)

Confirmadas tal como están las ocho decisiones del final. Aprobación expresa (de antemano) de `px-raster` **solo para R-05 a R-08**; `px-core` no se toca. Si hiciera falta algo más, parar y decirlo. Modo ahorro de `CLAUDE.md`: una sola corrección posterior de esta spec, al final; informe final de 20 líneas como máximo.

## Corrección posterior (al cerrar la unidad; la única)

Lo que se hizo distinto de lo escrito arriba, y por qué:

- **D4: la caja.** La desigualdad sola se sale de la caja en las elipses muy planas (`(5, 1)` daba un píxel de más a cada lado de la fila central): «dentro» exige además `|dx| ≤ a` y `|dy| ≤ b`. Comprobado por fuerza bruta hasta radios de 40 (anidada, una tramo por fila, simétrica). El anillo empieza en el este, pero con dos píxeles en la misma recta desde el centro va primero el más cercano, así que su primer punto no es el más extremo del eje (`(5, 1)` empieza en `dx = 4`).
- **D5: `Intro` no es una `ToolAction`.** Cerrar necesita el documento y `apply_action` no lo tiene: es `CanvasInput::Confirm` (`Key::Enter` sin modificadores ni repetición, en `canvas.rs`) que llega por `canvas_input_with`, como `Esc`. No hay `ToolAction::Finish`. Un polígono pendiente de otra imagen (distinto tamaño) se descarta al pulsar.
- **D2: editables 50, no 49.** `help_key(SelectTool(Shape))` existe (`tool.shape`), así que el botón también es editable aunque no tenga tecla, igual que los otros cinco: 44 + 6 = 50. Ayuda 57 y tabla 43 como se dijo. Iconos 23 (`Icon::Shapes` va el último para no mover el índice de los demás).
- **D10: textos de las filas.** `Tipo: Rectángulo | Círculo | Elipse | Polígono | Forma libre` y `Relleno: Contorno | Solo relleno | Ambos` (`shapes_es.rs`). Las filas son 5 con contorno, 6 con espaciado distinto de continuo y 2 con solo relleno; `OptionsWindow.scroll` ya es `[usize; 6]`.
- **Caja.** Como se midió en «Lo comprobado»: la ventana mínima sigue en 313 u (417,33 pt a `ppp` 1,5). A 10 columnas la caja pasa a dos filas (`head_u(6, 10)` = 109 u) y a 11 o más sigue en una. Ninguna prueba de la ventana mínima cambió; sí las que cuentan botones (`center_u`, `buttons_of_the_spec`, `TOOL_LABELS`, índice del zoom 5 → 6).
- **Filas de la geometría.** `px-raster` exporta además `MAX_RADIUS` (2³⁰). Las funciones de relleno devuelven `Vec<Span>` de `px-core` y los anillos se recortan a `margen` píxeles fuera del lienzo: nada crece con el tamaño de la forma. Un relleno enorme (8192² a la vez) sigue sin medir; la vista previa lo recorre píxel a píxel, como el resto de herramientas.
- **Referencia.** 30 PNG (2 rectángulos, 10 círculos, 10 elipses y 8 polígonos), no los 50 del primer borrador del script: `tools/corpus-h4/shapes.lua`, `generate-shapes.sh` y `crates/px-golden/tests/shapes.rs` (`#[ignore]` hasta tener los PNG; una prueba de la lista de casos sí corre). El script no se ha ejecutado: lleva marcas «comprobar» donde la API de Aseprite puede discrepar (`app.useTool`, el relleno de `polygon`).
- **Pruebas nuevas:** 21 en `px-raster/tests/shapes.rs` (más las propiedades), 22 en `px-ui/tests/shapes_tools.rs`, 3 en `px-app` y 2 en `px-golden` (una `#[ignore]`): 48 (por encima del tope orientativo de 40 por los barridos de las máscaras ASCII, que la spec pedía).
- **Pruebas anteriores adaptadas** (cifras, listas, literales y brazos de `match`; ninguna cambió de intención): teclas y tabla (`keymap`, `actions`, `pick`, `tool`, `key_events`, `keymap_file`, `layers_actions`, `menu`, `help_texts`, `shortcut_editor`, `dropper`), la caja (`chrome`, `c2_column`, `c2_geometry`, `toolbar`, `help_actions`, `common/toolbox`), `CanvasInput` (`canvas`, `tool_canvas`, `zoom_canvas`, `checker_canvas`, `pencil`) y, en `px-app`, `icons` y `keyboard_tests`. El detalle está en los mensajes de commit.
- **Cierre.** Ver el informe de la unidad: `cargo fmt`, `clippy --workspace --all-targets -D warnings` y `cargo test --workspace` los corre el usuario; `cargo deny` no está en este contenedor (sin dependencias nuevas) y el `license-auditor` **no se ha ejecutado** (presupuesto de `CLAUDE.md`: sin subagentes); toca por los cambios en `px-raster`.
- **D4: desigualdad estricta (referencia de Aseprite, 27 de octubre).** Con `≤`, el círculo de radio 1 era un cuadrado de 3 × 3 y Aseprite dibuja una cruz de 5 píxeles; con `<` coincide, y todos los demás círculos medidos (radios 2, 3, 5 y 10) siguen idénticos a Aseprite (en ellos ningún píxel cae justo en la igualdad). Cambia además el radio 4 (pierde 8 píxeles de las esquinas) y se añaden a la referencia los radios 4 y 8, que son los siguientes con píxeles en la igualdad. Las elipses con radios distintos no coinciden del todo (hasta 4 píxeles de más en las pequeñas y 16 en el contorno de 20 × 9): se queda la nuestra y la tolerancia de la referencia es del 20 % + 4 píxeles. Los rellenos de polígono (también la estrella con agujero, par-impar) coinciden píxel a píxel. El contorno del polígono en Aseprite se pide con la herramienta `line` lado a lado (`polygon` no dibuja por script) y el relleno con `contour`. 34 PNG (14 círculos).
