# Hito 2 · Unidad 19c: contraste de las entradas de los menús

Estado: **aprobada** (2026-10-20; D1, D2, D4 y D5 como se proponían, con los matices del visto bueno, que **prevalecen sobre cualquier otra frase de la página**: (D4) la fila 4 comprueba además que todo texto desactivado tiene **al menos 2,0:1** (comparación `>=`) sobre su relleno real y que se distingue del habilitado, y la lista cerrada de pares bajo 4,5 se mantiene; (pulido) mejora posterior anotada en «Fuera de alcance»; el mensaje del commit de pruebas incluye el número de pares recorridos y las cinco escalas). Cambia solo `px-ui` (`menu.rs` y una prueba). **No se toca `px-core` ni `px-raster`.** Sin dependencias nuevas ni `unsafe`. Regla de contraste y `contrast_ratio`: [`hito-2-dialogos-modales.md`](hito-2-dialogos-modales.md) (la 18) y `px-ui/src/theme.rs`. Menú y submenú con marca: [`hito-2-menu-herramientas.md`](hito-2-menu-herramientas.md) y [`hito-2-fondo-color.md`](hito-2-fondo-color.md) (D3).

## Objetivo

Que **todo** texto de las entradas de los menús tenga contraste ≥ 4,5:1 sobre el relleno que lleva, en reposo, con el ratón encima y marcado. Lo desactivado queda exento (excepción documentada en D4).

## Lo comprobado (y lo que no)

- **Causa.** `visuals()` fija `override_text_color = Some(TEXT)`. En egui 0.37 un botón `selected` rellena con `selection.bg_fill` (`ACCENT`) y quiere pintar con `selection.stroke` (`SURFACE_0`), pero el color de texto forzado gana: el rótulo sale en `TEXT` sobre `ACCENT`, **1,47:1**. Un color puesto a mano en el `RichText` sí gana al forzado (lo que ya hace el valor de `Color…`). Solo el menú usa `Button::selected` en todo el código (`grep` en `px-ui` y `px-app`): el defecto no sale de ahí.
- **Medido** en una copia desechable (`~/.cache`, fuera de `/tmp` y del repositorio, ya borrada) con el banco de egui de `background_menu.rs` (clics reales, sin ventana ni capturas): las 444 parejas texto-fondo distintas de la barra, de `Archivo`, `Editar`, `Imagen` y `Ver`, del submenú, en reposo, con el ratón sobre cada fila y con tres bloqueos (libre, diálogo, gesto), color real = el del glifo mezclado con el fondo. Con todo habilitado fallan **dos** cosas, no una:
  1. el **rótulo de la entrada marcada**: `TEXT` sobre `ACCENT` = **1,47** (también con el ratón encima: la fila marcada sigue en `ACCENT`);
  2. el **atajo** (`Ctrl+S`, `+`, `Inicio`…) **con el ratón encima**: egui lo pinta «débil» (`TEXT` al 60 % de alfa) y sobre `SURFACE_3` da **3,84**. En reposo da más de 4,5 (pasa). Es un defecto que no estaba descrito.
- **Pasan sin tocar:** rótulos de entradas no marcadas (`TEXT` sobre `SURFACE_2` 9,9 y sobre `SURFACE_3` 7,6), títulos de la barra, la fila del submenú, y el valor `#rrggbb` (`SURFACE_0` sobre `ACCENT`, 8,98).
- **Con el arreglo candidato** (D1 y D2) aplicado en la copia: los pares habilitados dan **todos ≥ 4,5**; y **todo** `cargo test --workspace --locked --no-fail-fast` da 94 ejecutables, 3255 pruebas, **0 fallos**. Es decir: **ninguna prueba anterior cambia, ni una cifra** (ninguna fija el color del rótulo marcado ni del atajo). Por eso esta unidad **no lleva el commit de adaptación de pruebas anteriores**.
- **No comprobado:** el aspecto real (prueba manual). Solo `ppp` 1,0 en la sonda; la prueba nueva recorre las cinco escalas.

## Decisiones (⚠ = necesita tu confirmación)

**D1. Rótulo de la entrada marcada en `SURFACE_0`** (8,98:1 sobre `ACCENT`), igual que el valor de `Color…` y la letra de la herramienta activa de la columna (la 17). Se aplica **por entrada** en `marked_entry`, con el color en el `RichText`. Es lo único que se pinta sobre `ACCENT`, y la fila marcada **no cambia de relleno con el ratón** (egui da prioridad a la marca), así que el mismo color vale en reposo y con el ratón. ⚠ **Alternativa descartada:** quitar `override_text_color` de `visuals()` para que mande el trazo de selección: afecta a todo el texto de la aplicación (diálogos, campos, avisos) y a las pruebas que leen ese campo; la causa es local y el arreglo también.

**D2. ⚠ Atajos del menú en `TEXT`, sin atenuar** (`weak_text_alpha = 1.0` solo en el estilo del menú): 9,9 en reposo y 7,6 con el ratón. Es una decisión que **no pedías**: la jerarquía rótulo/atajo pasa a depender solo de la columna (el atajo va alineado a la derecha). **Alternativa:** una constante nueva de gris claro (≈ `#b4b4b4`, 5,1 sobre `SURFACE_3`) que conserva la jerarquía, a costa de un color más en el tema. `TEXT_DIM` **no sirve**: da 3,98 sobre `SURFACE_3` (cifra ya fijada en la 18). Recomiendo `TEXT`: una regla de dos colores en el menú (`TEXT` y `SURFACE_0` sobre `ACCENT`), fácil de recorrer en la prueba.

**D3. Entradas con el ratón encima** (no marcadas): sin cambios en el rótulo (`TEXT` sobre `SURFACE_3`, 7,6). Se mide y se fija, no se cambia. Nota de lo que **no** se arregla: con el ratón sobre una fila marcada no hay realce (el relleno ya es `ACCENT`); es el comportamiento actual de egui y no es un problema de contraste.

**D4. ⚠ Excepción de lo desactivado.** egui lo pinta al 50 % de opacidad (texto **y** relleno), lo que baja el contraste por diseño. Medido: rótulos 3,72; atajos 2,26 → **3,72** con D2; el rótulo marcado de `Color…` desactivado, 1,98 → **2,00** con D1 (corregido al escribir las pruebas: la primera cifra, 1,06, salía de una sonda que no mezclaba el alfa del glifo). La prueba **no exige 4,5** a lo desactivado (WCAG 1.4.3 lo exime), pero sí que cada par desactivado sea **estrictamente menor** que el mismo par habilitado (se nota que está atenuado) y que **los únicos pares bajo 4,5 sean desactivados** (lista cerrada). **Suelo de 2,0:1** (visto bueno): todo texto desactivado debe dar `>= 2,0` sobre su relleno real; el valor más bajo medido (`Color…` marcada) es 2,00, con muy poco margen. Subirlo más exigiría pintar lo desactivado a mano en lugar de la opacidad de egui, y queda fuera de esta unidad.

**D5. Dónde va la prueba.** Se **añade a `px-ui/tests/background_menu.rs`** (que ya tiene el banco con clics reales y el cálculo del fondo bajo cada texto), como módulo de ese ejecutable: ni un enlazado nuevo ni una copia del banco. El nombre del fichero queda corto para lo que hace; moverlo a `common` tocaría decenas de pruebas, y no lo propongo.

## Piezas

Ninguna firma pública cambia.

- `px-ui/src/menu.rs`: en `marked_entry`, el rótulo de la entrada marcada lleva `.color(SURFACE_0)`; en la función `style` de `show_menu_with_background`, `style.visuals.weak_text_alpha = 1.0`. (Unas 4 líneas.)
- `px-ui/tests/background_menu.rs`: la prueba de recorrido y las dos propiedades.

## Casos (5 filas; banco de egui con clics reales, sin ventana)

Cada fila recorre las cinco escalas (`PPPS`). Color medido = el del primer glifo mezclado sobre el relleno real que hay debajo (la función `fondo_bajo` del banco), contraste con `contrast_ratio` de la 18.

| # | Entrada | Esperado |
| --- | --- | --- |
| 1 | **Recorrido en reposo y con el ratón.** Abrir `Archivo`, `Editar`, `Imagen` y `Ver` y el submenú; sin bloqueos; con el activo en cada uno de los cinco fondos (`Custom(#336699)` y `Custom(#000000)`); en reposo y con el ratón sobre **cada** fila, uno a uno | **Todo** texto pintado (rótulos, atajos, títulos, valor) tiene ≥ 4,5. Ningún texto queda sin relleno debajo |
| 2 | **Marcada.** Con cada uno de los cinco fondos activos: la fila marcada en reposo y con el ratón; las otras cuatro; el valor de `Color…` | El rótulo de la marcada es `SURFACE_0` sobre `ACCENT` (8,98, ≥ 4,5); el de las demás, `TEXT`; el valor, `SURFACE_0`. Con cada fondo hay **una sola** fila sobre `ACCENT` |
| 3 | **Atajos.** Todos los atajos de la tabla de menús (`Archivo`, `Editar`, `Ver`), en reposo y con el ratón sobre su fila, en las tres plataformas | Se pintan en `TEXT` opaco (alfa 255), ≥ 4,5 en ambos estados; el texto es el de `shortcut_text` |
| 4 | **Desactivado.** Con `dialog`, `gesture` (que atenúa `Color…`) y `text_focus`, por separado, cada fila atenuada, marcada y no marcada | Cada par es estrictamente menor que el mismo par habilitado; **no** se exige 4,5, pero **sí `>= 2,0`** (suelo de D4); el conjunto de pares bajo 4,5 en todo el recorrido está **solo** entre los desactivados. Las cifras medidas (3,72; 3,72; 2,00) se anotan en el comentario de la prueba, sin fijarlas con un `==` |
| 5 | **Nada más cambia.** Posiciones y tamaños de cada texto del menú, `submenu_width_u` (131 u) y `visuals(ppp).override_text_color` | Idénticos a los de antes (comparados con la caja de cada texto, a la tolerancia de egui de `common`); `override_text_color` sigue siendo `Some(TEXT)`; el resto de las pruebas de `px-ui` y `px-app` pasan sin tocarlas |

## Propiedades (proptest; 32 casos, cada uno abre dos menús con clics)

1. **Contraste con cualquier fondo.** Para un `CanvasBackground` cualquiera (`Custom` con RGB opaco aleatorio incluido), una escala de `PPPS` y una fila bajo el ratón: todo par texto-fondo habilitado ≥ 4,5. (El color elegido no entra en el contraste: la marca va sobre `ACCENT`, no sobre el color del fondo.)
2. **El color del texto depende solo del estado.** Con los mismos datos: el color pintado de un rótulo es `SURFACE_0` si está marcado y `TEXT` si no, y el del atajo es `TEXT`; no cambia con el fondo activo, la escala ni qué fila tiene el ratón.

## Orden de trabajo y commits

Sin pruebas anteriores que adaptar (medido: ninguna cambia), así que los commits son: **spec; pruebas (las filas 1, 2 y 3 y las dos propiedades en rojo; la 4 y la 5 ya pasan); lógica** (`menu.rs`). Sin `px-app`: `main.rs` no cambia. Si al escribir las pruebas alguna anterior necesitara algo más que una cifra, me paro. Los commits de pruebas en rojo no tienen por qué compilar el espacio de trabajo entero (aquí sí compilan: son pruebas, no firmas).

## Prueba manual (la haces tú, sin capturas)

1. `Ver → Fondo del lienzo`: el rótulo de la entrada marcada (`· Damero`) se lee oscuro sobre el verde; elige `Color…`, acepta `#336699` y reabre: `· Color…` y el valor se leen claramente.
2. Abre `Archivo` y pasa el ratón por `Guardar`, `Abrir…`…: el atajo (`Ctrl+S`) se lee igual de claro que el rótulo, también con la fila resaltada.
3. Con una quita de color pendiente (o con un trazo a medias), `Color…` sale atenuada: debe notarse que está desactivada aunque se lea poco (D4).

## Fuera de alcance

**Mejora posterior (anotada):** la fila marcada no tiene realce con el ratón encima (su relleno ya es `ACCENT`); valorar un borde o un tono más claro. Tampoco: el contraste de la columna de herramientas, los diálogos y el panel (ya cubiertos por la 17 y la 18); pintar lo desactivado a mano para subir su contraste.

## Aclaraciones al escribir las pruebas

- **Cifra corregida.** `Color…` marcada y desactivada da **1,98** hoy y **2,0045** con la lógica (no 1,06 → 2,00): el suelo de 2,0 de D4 se cumple con un margen de 0,0045. Hoy la fila 4 no pasa entera: el marcado desactivado (1,98) no es menor que el habilitado (1,47) y la lista cerrada falla porque los habilitados siguen bajo 4,5; pasan los rótulos y atajos desactivados no marcados. Las filas 4 y 5 no son «ya verdes» salvo en esa parte.
- **Fila 5.** No hay captura previa que comparar; se fijan los invariantes de la geometría de la 17 (7 u por carácter, 14 u de alto de texto, fila de 16 u, margen de 6 u, `popup_width_u` y `submenu_width_u`), que es lo que un cambio de color no debe mover. A `ppp` 1,25 egui coloca algunas filas fuera de la rejilla: se tolera 1 píxel físico.
- **Fila 1** recorre además el menú `Ayuda` (Licencias) y los cuatro fondos fijos con los dos `Custom`: 6 fondos × 5 escalas × 6 escenas, reposo y cada fila (10 390 medidas, 72 pares distintos). En depuración a `ppp` 1,25 egui pinta un texto sin relleno («Unaligned»); `medir` lo salta.
- **Fila 3.** El banco ahora pinta las tres plataformas de verdad (11 atajos en Linux, 10 en Windows y macOS).
