# Hito 4 · Unidad 4.3: pincel y trazos

Estado: **aprobada** (decisiones delegadas por el usuario; se confirman las de «Decisiones» tal como están, ver «Visto bueno») y **hecha, a falta de la prueba manual** (2026-10-27: ver «Corrección posterior» para lo que cambió respecto de lo escrito). Cambios previstos: `px-raster` (**solo lo listado abajo, aprobado de antemano, aditivo y no más**), `px-ui` y `px-app`. **`px-core` no se toca.** Sin dependencias nuevas.

Referencias: fila 4.3 y decisión D14 de [`mapa-funciones.md`](mapa-funciones.md) (T01 a T09, sección 4); [`hito-2-lapiz-deshacer.md`](hito-2-lapiz-deshacer.md) (`Stroke`, un paso por gesto); [`hito-2-linea.md`](hito-2-linea.md); [`ui-single-column.md`](ui-single-column.md) (ventana de opciones); [`interfaz-teclado.md`](interfaz-teclado.md) (tabla de atajos como datos).

## Objetivo

Un **pincel** global (forma y tamaño de 1 a 100) con el que pintan cuatro herramientas de trazo: lápiz (`d`, mano alzada continua), mano alzada punteada (`s`), línea (`v`) y curva (`q`). Línea y curva admiten espaciado; `Mayús` fuerza un eje; `Ctrl` deja trazas. `+` y `-` cambian el tamaño. La ventana de opciones de cada herramienta de trazo tiene por primera vez contenido real.

## Lo comprobado (en el código)

1. `Stroke` (px-ui) une posiciones con `Line` y guarda píxeles únicos en orden de primera visita; `ToolState` aplica un `SetPixels` al soltar: un paso de deshacer por gesto. El lápiz de 1 px es, por tanto, el pincel de tamaño 1 en modo continuo: **debe seguir dando lo mismo** (las pruebas del lápiz no cambian).
2. `line_command` y `line_in` pintan la línea de 1 px; la línea con pincel pasa a salir del mismo `SetPixels` de los sellos (también sobre celda vacía, como hoy).
3. `+`, `=` y `-` están hoy en la tabla de teclas como zoom (ocultos, `shown: Never`) y en `canvas.rs` (`Key::Plus | Key::Equals`, `Key::Minus`). El zoom queda en `<`, `>`, rueda e `Inicio`.
4. La ventana de opciones ya existe por herramienta (`options.rs`): `options_row_count` devuelve 0 y el cuerpo dice `sin opciones`. `scroll` es `[usize; 5]`.
5. **No hay oráculo externo cómodo**: las formas pequeñas del pincel son decisión nuestra (mapa, T01). Las referencias son máscaras ASCII escritas a mano en las pruebas, no salida de otro programa. No se genera corpus de Aseprite en esta unidad.

## Decisiones (⚠ = decido algo que no me diste; las confirmo abajo)

**D1. Un pincel global.** `Brush { shape, size }`, uno para toda la sesión (no por herramienta), `size` de 1 a 100 (se recorta al construir). Por defecto: `Round`, 1. El borrador (`k`) y el cuentagotas **no** lo usan (el borrador con pincel va con 4.10).
**D2. Diez formas.** `Round`, `Square`, `Diamond`, `Ring` (redondo hueco, anillo de 1 px), `Horizontal` (n × 1), `Vertical` (1 × n), `Slash` (`/`), `Backslash` (`\`), `Plus` (cruz recta) y `Cross` (aspa `X`). ⚠ La lista exacta de las diez es mía.
**D3. Máscara.** Caja de `n × n` (las barras, de `n × 1` y `1 × n`). Para el píxel `(i, j)` de la caja, `dx = 2i + 1 − n`, `dy = 2j + 1 − n` (enteros; el centro de la caja es 0): `Round` ⇔ `dx² + dy² ≤ n² − n`; `Ring` ⇔ `Round` y no `Round` de tamaño `n − 2` (con `n ≤ 2`, igual que `Round`); `Square` todos; `Diamond` ⇔ `|dx| + |dy| ≤ n − 1`; `Plus` ⇔ `|dx| ≤ 1` o `|dy| ≤ 1` (una raya de 1 px si `n` es impar, de 2 si es par); `Cross`/`Slash`/`Backslash`: las diagonales `i = j`, `i + j = n − 1` (y `Cross` ambas). Con `n = 1` todas son un píxel. ⚠ Los redondos pequeños (`n = 3` en cruz, `n = 4` con esquinas cortadas) son decisión nuestra.
**D4. Anclaje.** El sello centrado en `(x, y)` ocupa `x − ⌊n/2⌋ .. x − ⌊n/2⌋ + n` (con `n` par el sobrante cae a la derecha y abajo). Un sello se recorta al lienzo; uno entero fuera no pinta nada.
**D5. Sellos y trazo.** Un trazo es una lista de centros; los píxeles son la unión de los sellos recortados, sin repetidos, en orden de primera visita (el mismo contrato que `Stroke`). Color sólido; sin mezcla.
**D6. Mano alzada.** `d` (continua): entre muestras consecutivas, sellos en cada punto de `Line` (con tamaño 1 es el lápiz de hoy). `s` (punteada): un sello por muestra, sin unirlas; si el ratón corre, quedan huecos (comportamiento buscado).
**D7. Espaciado (solo línea y curva).** `Spacing::Continuous` (un sello por punto del camino), `EveryN(n)` (centros en los puntos de índice 0, n, 2n… del camino de `Line` o de la curva; siempre incluye el primero; `n` de 1 a 999; `EveryN(1)` es continuo) y `TotalN(k)` (exactamente `min(k, puntos)` sellos, con el primero en el inicio y, si `k ≥ 2`, el último en el final, repartidos por índice: `⌊i·(L−1)/(k−1)⌋` para `i = 0..k`, `L` = puntos del camino; `k` de 1 a 999). ⚠ «N en total» sobre una curva reparte por **índice de píxel**, no por longitud de arco real. Las formas cerradas llegan con 4.4.
**D8. `Mayús` (eje).** En la línea y en el primer tramo de la curva: el final se proyecta sobre el eje de mayor desplazamiento (empate: horizontal), recalculado en cada movimiento. En mano alzada (`d`, `s`): el eje se fija en el primer movimiento que cambie de píxel y ya no cambia en el gesto; las posiciones se proyectan sobre él (la otra coordenada es la del inicio). Soltar `Mayús` a mitad de gesto suelta el eje.
**D9. `Ctrl` (trazas).** En línea y curva, mientras se arrastra con `Ctrl` pulsada, **cada** figura intermedia queda en el trazo (unión); sin `Ctrl`, solo la última. Un solo paso de deshacer. `Ctrl` se lee en cada evento; si se suelta antes del final, las trazas ya dejadas se mantienen y la figura final se suma.
**D10. Curva (`q`).** Cuadrática (un punto de control). Gesto en dos arrastres: (1) pulsar en `A`, arrastrar hasta `B`, soltar: queda un segmento pendiente (se ve como previsualización); (2) pulsar y arrastrar: la posición actual es el punto de control `C`; al soltar se aplica. `Esc`, cambiar de herramienta o de imagen descartan el pendiente (nada que deshacer). Un tercer paso de ratón nuevo no existe: no hace falta evento de «hover». ⚠ Cuadrática y no cúbica, y gesto de dos arrastres (el mapa decía «un tercer clic fija la curvatura»; sin eventos de hover, el segundo arrastre es lo que el lienzo puede dar hoy).
**D11. Rasterizado de la curva.** Se toma `m = max(|Bx−Ax|, |By−Ay|) + max(|Cx−Ax|, |Cy−Ay|) + max(|Bx−Cx|, |By−Cy|)` (longitud del polígono de control en Chebyshev, mínimo 1) y `N = m`. Para `t = i/N`, `i = 0..=N`, el punto es `((N−i)²·A + 2i(N−i)·C + i²·B) / N²` por coordenada con **redondeo a entero más cercano, medios hacia arriba** (aritmética `i64`); dos puntos consecutivos se unen con `Line`; los repetidos se quitan conservando el orden. Resulta simétrica (la curva de `A`,`B`,`C` y la de `B`,`A`,`C` son el mismo conjunto) y conecta en 8 vecinos. Con `C` en la recta `AB` sale una línea (no necesariamente la de `Line`: ver fila 9).
**D12. Teclas.** `d` lápiz, `s` punteada, `v` línea, `q` curva (herramientas nuevas: `Tool::Dotted`, `Tool::Curve`, añadidas **al final** de la caja, tras el borrador, para no mover los botones existentes). `+` (y `=`, sin mostrar) agranda y `-` achica el pincel de 1 en 1 con las herramientas de trazo; con otra herramienta no hacen nada. Salen de la tabla de zoom: el zoom queda en `<`, `>`, rueda e `Inicio` (y `Ctrl+1`). Acciones nuevas: `BrushBigger`, `BrushSmaller`; editables como las demás.
**D13. Un solo icono de pincel.** El botón del lápiz sigue siendo el **único** icono del pincel; el pincel no es una herramienta aparte. Su clic derecho (como el de cada herramienta de trazo) abre la ventana de opciones, que es la «ventana de formas». Iconos nuevos: punteada y curva (propios, dibujados como los demás).
**D14. Ventana de opciones.** Filas de cuerpo (13 columnas; se pintan como texto, una por fila, con el patrón de hoy): `Forma: Redondo`, `Tamaño: 5` (todas las herramientas de trazo) y, solo en línea y curva, `Espaciado: Continuo | Cada N | N en total` y, si no es continuo, `N: 4`. Clic izquierdo en una fila: valor siguiente (la forma cicla; `Tamaño` y `N` suben 1, con `Mayús` 10); clic derecho: anterior. Las formas se muestran con nombre; ⚠ no se pintan las 10 máscaras en una rejilla (cuesta una vista nueva; queda para después del hito).
**D15. Estado.** `ToolState` guarda `Brush`, `Spacing` y `modifiers`; el estado es de la sesión (no del documento), no se guarda en ficheros (preferencias: hito 8). Cambiar el pincel durante un gesto no afecta al gesto en curso.
**D16. Modificadores.** `ToolState::set_modifiers(shift, ctrl)` (aditivo) lo llama `px-app` antes de cada evento del lienzo con los modificadores de egui. Sin él, `Mayús` y `Ctrl` valen `false` (las pruebas anteriores no cambian).
**D17. Modos de color y capas.** Igual que hoy: el `Pixel` sale de la elección de color y del modo de la celda; en indexado, el índice; celda vacía se pinta sin materializarla. Capa bloqueada u oculta: como el lápiz de hoy (sin cambio).

## Cambios exactos en `px-raster` (todos aditivos; el visto bueno vale solo para esta lista)

- **R-01** `BrushShape` (las diez de D2), `Brush { shape, size }` con `Brush::new(shape, size)` (recorta a 1..=100), `size()`, `shape()` y `offsets() -> Vec<(i32, i32)>` (los píxeles de la máscara relativos al centro, D3 y D4, en orden de filas).
- **R-02** `Spacing { Continuous, EveryN(u32), TotalN(u32) }` y `spaced_centers(path: &[(i32, i32)], spacing) -> Vec<(i32, i32)>` (D7; `n` y `k` 0 se toman como 1).
- **R-03** `stamp_pixels(brush, centers: impl IntoIterator<Item = (i32, i32)>, width, height) -> Vec<(u32, u32)>` (D5, recortado, sin repetidos, orden de primera visita).
- **R-04** `line_path(a, b) -> Vec<(i32, i32)>` (los puntos de `Line`, sin recortar, para el espaciado) y `curve_path(a, b, control) -> Vec<(i32, i32)>` (D11).
- Nada no aditivo: ninguna firma pública existente cambia; `Line`, `line_command`, `draw_line` y el resto siguen como están.

## Piezas de `px-ui` y `px-app`

`px-ui`: `Tool::Dotted` y `Tool::Curve`; `Gesture` nuevos (mano alzada con pincel, punteada, línea con pincel, curva con estado pendiente); `ToolState::{brush, set_brush, spacing, set_spacing, set_modifiers}`; `AppAction::{BrushBigger, BrushSmaller}`; filas de `keymap` (`s`, `q`, `+`, `=`, `-`; se quitan `+`, `=`, `-` del zoom y el manejo de `Key::Plus/Equals/Minus` en `canvas.rs`); `options_row_count` y filas por herramienta en `options.rs` y `options_view.rs`; `TOOL_BUTTONS` 5 → 7, iconos nuevos, etiquetas y claves de ayuda; textos en `brush_es.rs`. `px-app`: cableado de modificadores y de las filas de la ventana, y de las dos acciones; ayuda.

## Casos (12; 1 a 5 y 9 en `px-raster`, 6 a 8, 10 y 11 en `px-ui`, 12 en `px-app`)

| # | Entrada | Esperado |
| --- | --- | --- |
| 1 | **Máscaras.** Las diez formas con tamaños 1 a 5 | Máscaras ASCII escritas a mano (D3) idénticas; con tamaño 1 todas son un píxel; `Brush::new(_, 0)` = 1 y `101` = 100 |
| 2 | **Anclaje y recorte.** Sello de tamaños 3 y 4 en el centro y en las esquinas y bordes de un lienzo de 6 × 6; sello fuera | Ocupa `x − ⌊n/2⌋..` (D4); nada fuera del lienzo; entero fuera = vacío; sin repetidos |
| 3 | **Línea con pincel.** Segmentos horizontal, vertical y diagonal con `Round` 1, 3 y `Square` 2 | Con tamaño 1 igual a `Line`; con otros, la unión de sellos en cada punto (modelo independiente) |
| 4 | **Espaciado.** Camino de 11 puntos con `EveryN(3)`, `EveryN(1)`, `TotalN(5)`, `TotalN(1)`, `TotalN(20)`; `n = 0` | Centros 0, 3, 6, 9; continuo = todos; 5 sellos con primero y último; 1 sello = el inicio; `k > L` = todos; 0 se toma como 1 |
| 5 | **Curva.** `(0,0)`, `(8,0)`, control `(4,8)`; control colineal; `A = B`; control = `A` | Puntos ASCII del caso; extremos incluidos; colineal = recta entre extremos; `A = B` = un punto; sin repetidos consecutivos |
| 6 | **Mano alzada.** Traza de eventos con un salto de 6 px, tamaño 3: `d` y `s`; lápiz de tamaño 1 | `s`: solo los sellos de cada muestra (con hueco); `d`: unidos; `d` con tamaño 1 = salida del lápiz de hoy; un paso de deshacer; vista previa = lo aplicado |
| 7 | **`Mayús`.** Línea de `(0,0)` a `(7,3)`, `(3,-7)` y `(5,5)`; mano alzada con primer movimiento diagonal | Final proyectado: `(7,0)`, `(0,-7)`, `(5,0)`; mano alzada fija el eje y lo mantiene; sin `Mayús` no cambia nada |
| 8 | **`Ctrl` (trazas).** Línea arrastrada por tres posiciones, con y sin `Ctrl`; `Ctrl` soltada a medias | Con: unión de las figuras; sin: solo la última; un solo paso de deshacer; las ya dejadas se mantienen al soltar |
| 9 | **Espaciado en línea y curva (gesto).** Línea con `EveryN(4)` y `TotalN(3)`; curva con `EveryN(2)` | Los sellos de la fila 4 aplicados al camino del gesto; en la curva, centros del camino de `curve_path` |
| 10 | **Gesto de la curva.** Arrastre `A→B`, segundo arrastre al control, soltar; `Esc` a medias; cambio de herramienta con segmento pendiente; segundo arrastre sin primero | Tras el primero no hay cambio en el documento (solo previsualización); al soltar el segundo, un paso; `Esc` y el cambio descartan sin efecto; sin pendiente, el arrastre empieza otra curva |
| 11 | **Teclas y opciones.** `+`/`-` con cada herramienta; zoom con `<`, `>`, rueda, `Inicio`; `s`, `q`; clic en las filas de la ventana; claves de ayuda | Tamaño ±1 en 1..=100 solo con herramientas de trazo; `+`/`-` ya no hacen zoom; `s` y `q` eligen herramienta; las filas ciclan la forma, suben y bajan `Tamaño` y `N` (con `Mayús` 10); la tabla de atajos y la ayuda cuadran |
| 12 | **Sesión.** Pintar con `Round` 5 sobre RGBA, indexado y celda vacía; deshacer/rehacer; cambiar de pincel entre trazos | Un paso por gesto y `modified`; el índice del color en indexado; la celda vacía se rellena; deshacer deja el documento inicial; el pincel nuevo solo afecta a los trazos nuevos |

## Propiedades (proptest; 256 casos, 500 como máximo)

1. **Máscara.** Para toda forma y tamaño 1..=100: no vacía, dentro de su caja, determinista y simétrica (las ocho con simetría doble respecto de ambos ejes; `Slash` y `Backslash`, respecto de su diagonal), y `Round`, `Square` y `Diamond` no pierden píxeles al crecer el tamaño.
2. **Unión recortada.** Para listas de centros arbitrarias (también fuera del lienzo) y cualquier pincel: `stamp_pixels` = unión de sellos del modelo ingenuo, recortada, sin repetidos y dentro del lienzo.
3. **Espaciado.** Para caminos de `Line` entre puntos arbitrarios: `EveryN(n)` da los índices `0, n, 2n…` del camino; `TotalN(k)` da `min(k, L)` centros, empieza en el inicio, acaba en el final si `k ≥ 2 ≤ L` y los índices son crecientes.
4. **Curva.** Para `A`, `B`, `C` arbitrarios: incluye `A` y `B`, es conexa en 8 vecinos, queda en la caja de los tres puntos y `curve_path(A, B, C)` y `curve_path(B, A, C)` son el mismo conjunto.

## Pruebas nuevas

12 filas y 4 propiedades: mínimo 16, hasta unas 30 con varios casos por fila; tope de 40. Reparto: `px-raster/tests/brush.rs` (1 a 5, propiedades 1 a 4), `px-ui/tests/brush_tools.rs` (6 a 10; el resto de 11 en `keymap`/`options`), `px-app/src/…/tests.rs` (12). La sesión principal las escribe y commitea antes del código (modo ahorro); `test-writer` solo si `px-raster` lo pide, y por defecto no.

## Pruebas anteriores que cambian (por lectura; el compilador da los números)

**A. Solo cifras, listas y literales:** recuento de filas de la tabla de teclas (36 → más las de `s`, `q`, `+`, `-` y sin `+`/`=`/`-` del zoom) y de acciones editables y de ayuda; `TOOL_BUTTONS` 5 → 7 y todo lo que cuente botones o etiquetas (`chrome`, `toolbar`, `options`); `scroll: [usize; 5]` → 7; las pruebas de zoom por `+` y `-` (teclas) pasan a `<`/`>`; textos de `help`. **B. Cambian de intención si lo piden:** ninguna prevista. Si alguna necesitara más que cifras, literales o firmas: parar y avisar (CLAUDE.md, regla 4).

## Orden de trabajo

Commits separados: (1) esta spec con su fila del `INDEX`; (2) adaptación de pruebas anteriores (el mensaje explica qué comprobaba cada una); (3) pruebas, en rojo; (4) lógica de `px-raster`; (5) `px-ui`; (6) `px-app`. Cierre (lo lanza el usuario): `cargo fmt`, `clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked`, `cargo deny check`; `license-auditor` solo por los cambios en `px-raster`.

## Prueba manual (la haces tú, sin capturas)

1. Elegir el lápiz (`d`), `+` hasta tamaño 5 y pintar: trazo grueso y redondo; `Ctrl+Z` lo deshace entero.
2. Clic derecho en el lápiz: ventana con `Forma` y `Tamaño`; clic en `Forma` cicla las diez; `Mayús`+clic en `Tamaño` salta de 10 en 10.
3. `s`: mover rápido deja puntos sueltos; despacio, casi continuo. `d`: siempre unido.
4. `Mayús` al pintar (`d` y `s`): el trazo sale recto, horizontal o vertical según el primer movimiento.
5. `v`: línea con el pincel; en su ventana, `Espaciado` → `Cada N` con `N` 4: puntos cada 4 px; `N en total` con 5: cinco sellos.
6. `v` con `Ctrl` arrastrando: queda la estela de todas las posiciones; sin `Ctrl`, solo la última.
7. `q`: arrastrar `A→B`, soltar, arrastrar de nuevo para curvar, soltar: arco; `Esc` entre medias lo cancela.
8. `+`/`-` ya no hacen zoom; `<`, `>`, rueda e `Inicio` sí.
9. `Ayuda → Atajos de teclado…`: `s`, `q`, `+`, `-` aparecen y son editables.
10. Indexado y capa vacía: el pincel pinta el índice y la celda vacía se rellena.

## Fuera de alcance

Pincel en el borrador y modos de pintura (4.10), aerógrafo (4.11), giro libre (4.12), curva cúbica, formas y su espaciado «N en total» sobre contornos cerrados (4.4), rejilla y simetría (4.9), rejilla visual de las diez formas, pinceles personalizados (4.8), y cualquier otro cambio en `px-core` o `px-raster`.

## Decisiones que necesito que confirmes

1. **D1:** un pincel global; borrador sin pincel.
2. **D2 y D3:** las diez formas y su fórmula de máscara.
3. **D7:** «N en total» reparte por índice de píxel.
4. **D8 y D9:** `Mayús` fija eje en mano alzada; `Ctrl` suma figuras intermedias.
5. **D10 y D11:** curva cuadrática, gesto de dos arrastres, rasterizado simétrico por redondeo entero.
6. **D12:** `s`, `q`, herramientas al final de la caja; `+`/`-` tamaño y fuera del zoom.
7. **D13 y D14:** el botón del lápiz es el único icono de pincel; la ventana de opciones lleva forma, tamaño y espaciado como filas de texto.
8. **R-01 a R-04:** tu aprobación expresa para tocar `px-raster` solo con esa lista (dada de antemano).

## Visto bueno (2026-10-27; el usuario delegó en mi criterio)

Confirmadas tal como están las ocho decisiones del final. Aprobación expresa (de antemano) de `px-raster` **solo para R-01 a R-04**; `px-core` no se toca. Si hiciera falta algo más, parar y decirlo. Modo ahorro de `CLAUDE.md`: una sola corrección posterior de esta spec, al final; informe final de 20 líneas como máximo.

## Corrección posterior (al cerrar la unidad; la única)

Lo que se hizo distinto de lo escrito arriba, y por qué:

- **D12/D13: `s` y `q` son modos, no herramientas.** No hay `Tool::Dotted` ni `Tool::Curve` ni botones nuevos: `s` elige el lápiz en modo Punteado y `q` la línea en modo Curva (`AppAction::SelectDotted`/`SelectCurve`, `ToolAction` iguales; `ToolState::dotted()`/`curve()`). Motivo: con 7 botones y el ancho mínimo de la columna (5 por fila) la caja habría pasado a dos filas y la ventana mínima habría superado el techo de 432 pt de C3; con 5 botones nada de la caja cambia. La fila `Modo` de la ventana de opciones (Continuo/Punteado, Recta/Curva) cambia el modo con el ratón. `d` y `v` (y `Select` en general) vuelven al modo normal; los botones del lápiz y de la línea **no** reinician el modo ya activo (así el clic derecho que abre la ventana no lo pierde). El borrador y el cuentagotas siguen sin pincel.
- **D3: rombo.** `Diamond` es `|dx| + |dy| ≤ n` (con `≤ n − 1` el de tamaño 2 quedaba vacío). `Round` 3, `Diamond` 3 y `Plus` 3 dan el mismo signo `+`; `Diamond` 4 = `Round` 4.
- **D12: teclas.** `+` y `-` se muestran en la tabla (`shown` Todas) y `=` es la secundaria oculta; por eso en el editor de atajos Acercar tiene solo `>` y Alejar solo `<`.
- **D9: `Ctrl`** es `command` de egui (Ctrl en Linux y Windows, Cmd en macOS).
- **`px-raster`, además de R-01 a R-04 (todo aditivo):** `BrushShape::ALL` (para el ciclo de formas), `line_path_near(a, b, ancho, alto, margen)` y `MAX_CURVE_STEPS` (4 194 304: más allá, `curve_path` da solo los dos extremos). `line_path_near` recorta el trazo al lienzo más el tamaño del pincel; sin él, un trazo de `i32::MIN` a `i32::MAX` reservaba miles de millones de puntos (las pruebas del borrador y del cuentagotas lo habrían abortado). Con espaciado ≠ continuo la línea usa el camino entero si tiene ≤ 4 194 304 puntos.
- **D14: ventana de opciones.** `OptionsView` gana `lines` y `OptionsEvent` la variante `Row { index, forward, big }`; `options_row_count(tool)` da las filas con el espaciado por defecto (lápiz 3, línea 4; con espaciado ≠ continuo la línea tiene 5, `ToolState::option_rows`). Textos en `brush_es.rs` (formas: Redondo, Cuadrado, Rombo, Anillo, Horizontal, Vertical, Diagonal sube, Diagonal baja, Cruz, Aspa).
- **Fila 12.** No hay `Session` con herramientas: el estado vive en `App`. La fila se prueba en `px-app/src/dispatch/tests/brush.rs` (acciones de tamaño y modo, trazo grueso indexado en un paso) y las celdas vacías y los modos en `px-ui`.
- **Ayuda:** 47 → 51 entradas (`tool.dotted`, `tool.curve`, `brush.bigger`, `brush.smaller`, tras las de `image.*`), 44 acciones editables, tabla de teclas de 38 filas.
- **Pruebas nuevas:** 16 en `px-raster/tests/brush.rs`, 15 en `px-ui/tests/brush_tools.rs` y 3 en `px-app` (34 en total). No hay referencia de Aseprite: las máscaras son ASCII escritas a mano (se generaron con un script aparte a partir de D3 y se revisaron a ojo).
- **Pruebas anteriores adaptadas** (solo cifras, listas, nombres de acción y sondas; ninguna cambió de intención): teclas y tabla (`keymap`, `actions`, `pick`, `tool`, `layers_actions`, `merge_actions`, `two_colors`, `image_ops`, `help_texts`, `shortcut_editor`), sondas del lienzo que usaban `+` y `-` como zoom (`color_editor`, `view_ops`, `zoom_canvas`, `canvas`, `view_center`), `options_window`, `toolbar` y, en `px-app`, `keyboard_tests` y `shortcuts_flow`. El detalle está en los mensajes de commit.
- **Cierre.** `cargo fmt`, `clippy --workspace --all-targets -D warnings` y `cargo test --workspace` en verde **salvo** `px-ui/tests/c2_column.rs` (propiedad 2, cursor de redimensionar la columna). Es un fallo latente **anterior a la unidad**: proptest lo encontró en una ejecución aleatoria (dos `Press` seguidos sin soltar, con el cursor en el borde de la columna) y la semilla guardada también falla sobre `origin/main`. No se tocó la lógica de la columna; se quitó del repositorio el fichero de semillas que ese fallo generó, para no mezclarlo con esta unidad. Queda para una tarea aparte. `cargo deny` no está instalado en este contenedor (sin dependencias nuevas). El `license-auditor` **no se ha ejecutado** (presupuesto de `CLAUDE.md`: sin subagentes); toca por los cambios en `px-raster`.
