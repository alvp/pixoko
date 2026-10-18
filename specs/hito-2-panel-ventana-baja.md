# Hito 2 · Unidad 11c (ajuste): panel de paletas con la ventana baja

Estado: **pendiente de visto bueno** (2026-10-18). Sin implementar. Si se aprueba, **sustituye a la decisión 12 de [`hito-2-editar-color.md`](hito-2-editar-color.md)** («con `alto_px < (41 + 14 + H)·n` el editor se oculta») y a su fila 18. No toca `px-core` ni `px-raster`.

## Lo que se ve

Al reducir la altura funciona, pero se siente brusco: salto cuando el editor aparece o desaparece, parpadeo al arrastrar el borde y retraso.

## Lo comprobado

Con la lógica pura y el banco de egui sin ventana (`Context::run_ui`, sin GPU ni pantalla), con un test desechable que **no se guarda**. El escritorio no se ha capturado.

- **(a) Umbral.** `editor_layout` es una función pura de `(modo, alto del panel, n)` sin memoria. Indexada, `n = 1`: con 240 px el panel no da editor y la rejilla tiene **14 filas**; con 241 px da el editor (186 px reservados) y **1 fila**. Con `n = 2` lo mismo en 481 → 482. Es una discontinuidad de 13 filas y de 186 u de editor en 1 px de altura. Fuera del umbral no hay saltos: las filas cambian de una en una cada 14·`n` px y el editor, anclado al fondo, se mueve `n` px por cada `n` px.
  **No hay oscilación provocada por la propia app:** la barra de estado mide siempre `20·n` px (`bar_height_px`, texto recortado en una línea), así que el aviso no cambia la altura del panel. Con una altura que crece o baja de forma monótona no hay parpadeo; solo si la altura que llega **oscila ±1 px** en torno a 241·`n` (un gestor que reasigna el tamaño, algo que aquí no se puede reproducir) el editor aparece y desaparece en cada fotograma. Con A y B el umbral deja de existir, así que **no hace falta histéresis** (decisión 6).
- **(b) Desplazamiento.** `scroll.clamp(total, visible)` se aplica en el mismo fotograma, antes de pintar: al pasar de 1 a 14 filas la primera fila baja de 10 a 2 (el máximo) y nunca se pintan filas inexistentes. Pero **no se recuerda la posición** (al volver el editor la primera fila sigue en 2) y **no hay «mostrar la elegida»**: con la fila 12 elegida, el editor vuelve con 1 fila visible y la elegida fuera de la vista, así que se edita un color que no se ve.
- **(c) Retraso.** El diseño no se retrasa: `editor_hidden` ya es cierto en el primer fotograma tras bajar la ventana. **El aviso de la barra sí llega un fotograma tarde:** en [`main.rs`](../crates/px-app/src/main.rs) `status::show` pinta el texto del fotograma anterior (`self.status`) y `self.editor_hidden` se asigna después; el texto nuevo se calcula al final y pide otro fotograma. Lo mismo vale para un `Notice` creado en ese fotograma. El resto del retraso que se nota al arrastrar el borde (el compositor entrega el nuevo tamaño con uno o más fotogramas de retraso) **no se puede medir sin pantalla**.
- **(d) Selección y campo enfocado.** La elección (`choice`) no cambia nunca al ocultar o mostrar. Un arrastre en curso se cancela (ya cubierto por la fila 18). **Fallo:** con el campo hexadecimal enfocado y texto pendiente (`#12` o `ff0000`), al ocultarse el editor egui le quita el foco (comprobado: ya en ese fotograma), pero `lost_focus()` no se dispara porque el campo no se pinta: no hay `Commit`, no hay aviso y `pending` **queda huérfano**. Al volver el editor el campo muestra ese texto viejo, también con **otra** entrada elegida (comprobado con la 9), y un clic dentro y fuera del campo lo aplica a la entrada original. Es un texto tecleado que se pierde en silencio y reaparece solo. Mismo camino sin destino (la elección pasa a algo sin editor).

## Soluciones

**A. Altura mínima de la ventana.** Una función pura `min_panel_height_px(n) = (GRID_Y_U + PITCH_U + editor_height_u_max)·n` (sin los literales 41 y 14 que hoy lleva `editor_layout`; ahí se llama a ella) y `min_window_height_pt(ppp) = (bar_height_px(n) + min_panel_height_px(n)) / ppp`. En `px-app`: `ViewportBuilder::with_min_inner_size([ancho, alto])` al crear la ventana con el valor para `ppp` 1,0, y `ViewportCommand::MinInnerSize` cuando `apply_theme` ve otro `ppp` (el `ppp` real solo se conoce con la ventana abierta). `editor_height_u_max` es el mayor entre modos (hoy 186), no el del modo actual, para que abrir una imagen Gray no cambie el mínimo ni redimensione la ventana. Valores: 261 pt a `ppp` 1,0, 2,0 y 3,0; 208,8 pt a 1,25; 348 pt a 1,5.

**B. Respaldo sin ocultar el editor.** Si el panel mide menos de `min_panel_height_px` (un compositor que ignora el mínimo), el bloque `[texto · rejilla de 1 fila · editor]`, de esa altura, se desplaza **entero** con un único `offset_px` (0 a `min − alto`; con 0 es el diseño normal, así que **no hay salto** en el umbral). La rueda sobre el panel mueve el `offset_px` y no las filas de la rejilla (que vuelven al agrandar la ventana). El editor no desaparece: sus controles se desplazan con el bloque, y los puntos fuera del rectángulo del panel no golpean ningún control. La barra dice `aviso: ventana baja: panel con desplazamiento` mientras ocurra (sustituye a `EDITOR_HIDDEN_NOTICE`).

**Una sola función para la altura del editor.** `editor_height_u(mode)` es el único sitio: se **deriva** de la maqueta (`notice_y_u + NOTICE_LINES·ROW_H_U + BLOCK_GAP_U`, hoy 186, 112 y 94) en vez de llevar literales, y de ella cuelgan `editor_layout`, `min_panel_height_px`, `min_window_height_pt` y el alto del bloque de B. La unidad 11d solo añade el alto de los botones al cálculo de `field_y_u` o `notice_y_u`; el resto sigue solo. Un test fija 186, 112 y 94 mientras no haya botones.

## Casos (máximo 8; tests de lógica pura y del banco, sin ventana)

| # | Entrada | Esperado |
| --- | --- | --- |
| 1 | `min_panel_height_px(n)` para `n` = 1, 2, 3; `min_window_height_pt` a `ppp` 1,0, 1,25, 1,5, 2,0 y 3,0 | 241·`n`; 261, 208,8, 348, 261 y 261 pt; con `editor_height_u_max` = 186 en todos los modos |
| 2 | `editor_height_u` frente a la maqueta; después, una altura de botones simulada de 20 u | 186, 112 y 94 coinciden con la derivación; con 20 u más cambian `editor_layout`, el mínimo y el bloque de B **sin tocar nada más** |
| 3 | Indexada, alto del panel 240·`n`, 241·`n`, 200·`n` y 100·`n`, a `ppp` 1,0 y 2,0 | el editor se ve siempre; la rejilla tiene 1 fila; con 241·`n` el diseño es **idéntico** al normal y con 240·`n` difiere en menos de `n` px (sin salto) |
| 4 | Alto de 200·`n`: rueda sobre el panel, sobre el editor, hasta los dos extremos; al volver a 241·`n` | `offset_px` queda en 0 a `min − alto`; las filas de la rejilla no se mueven; con alto ≥ 241·`n` el `offset_px` es 0 **en el mismo fotograma** |
| 5 | Alto de 200·`n` y `offset_px` > 0: clic sobre un deslizador, sobre el campo y fuera del rectángulo del panel | el control se halla con `y − offset_px`; fuera del panel no se golpea nada; el arrastre no se cancela por bajar la ventana |
| 6 | Campo enfocado con `#12` o `ff0000` y la ventana baja de 241·`n` a 200·`n` y vuelve | el campo sigue enfocado con su texto; sin destino o con otro destino se resuelve como perder el foco: válido → `Commit` al destino original, inválido → revierte con el aviso de 5 s; `pending` es `None` tras el cambio |
| 7 | Primera fila 2 con 16 filas, elegida la fila 12 y el editor vuelve (de 14 a 1 fila visible) | con la decisión 5 aprobada, la elegida queda visible (`Scroll::reveal`); sin ella, como hoy |
| 8 | El panel baja de 241·`n` a 200·`n` y sube | el texto de la barra con el aviso sale **en el mismo fotograma**, no uno después; vuelve a su texto normal también en el mismo |

La prueba manual (la haces tú, sin capturas): reducir la altura con el ratón hasta el mínimo (el compositor debe parar en él, y la rejilla muestra una fila y el editor entero); en un gestor en mosaico, forzar una altura menor y comprobar que el panel se desplaza con la rueda, que el editor sigue ahí y que la barra avisa; teclear en el campo y reducir la ventana a medias.

## Decisiones que necesito que confirmes

1. **A y B juntas**, con B sustituyendo a «ocultar el editor» (decisión 12 de 11c). Sin B, un gestor en mosaico seguiría con el salto y el fallo (d).
2. **El mínimo usa el modo de mayor editor** (186 u) para todos los modos, no el del modo actual. Cuesta 92 u de altura mínima extra en Gray (186 − 94), a cambio de que abrir un fichero no redimensione la ventana.
3. **En B, la rueda mueve todo el panel y no las filas de la rejilla.** Con la ventana muy baja solo se ve la primera fila hasta que se agranda. La alternativa (dos desplazamientos anidados) es más difícil de usar y de probar.
4. **Texto pendiente del hexadecimal cuando el campo desaparece (sin destino o sin editor): resolverlo como perder el foco** (aplica si es válido, revierte con aviso si no). Es un arreglo del fallo (d) y vale aunque no apruebes A ni B. La alternativa, descartarlo en silencio, pierde lo tecleado.
5. **Mostrar la elegida** (`Scroll::reveal`) al cambiar el número de filas visibles. No lo pediste y cambia un comportamiento: hoy no se hace ni al elegir con las teclas 1 a 9. Mi propuesta: hacerlo solo cuando cambia el número de filas, y dejar el resto para el pulido de 11d.
6. **Sin histéresis.** Con A y B el umbral no existe; con una histéresis habría que guardar un estado que B hace innecesario.
7. **El aviso de la barra sin retraso** (fila 8): el estado «panel bajo» se calcula al principio de `ui()` con la misma función pura y la altura de la ventana, antes de pintar la barra. Cambia el orden en `main.rs`. Sin esto, queda un fotograma de retraso en el aviso.
