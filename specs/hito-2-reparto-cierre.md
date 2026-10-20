# Hito 2 · Reparto del cierre

Estado: **aprobado** (2026-10-18; orden y decisiones D1 a D10 como las confirmó el usuario). Solo reparto: no es una spec de comportamiento ni tiene tests. **Sustituye** a la lista de unidades de [`hito-2-hsl.md`](hito-2-hsl.md) («Unidades del hito 2») en lo que queda por hacer: las unidades 1 a 11 y la 15 mínima están hechas, y de la 15 queda el diálogo de archivo (unidad 16) y la puerta (unidad 21). Cada unidad tendrá su spec y su visto bueno antes de implementarse.

## Orden

**Secuencial y no en paralelo**: cada unidad pasa por la revisión del usuario y las unidades de interfaz y las de rendimiento tocan `main.rs`. Primero la interfaz, después el rendimiento.

| Orden | Unidad | Contenido | Crates | Dependencias nuevas |
| --- | --- | --- | --- | --- |
| 1 | **16** | Diálogos de archivo con `rfd` (abrir, guardar y guardar como) | `px-app` | `rfd` |
| 2 | **17a** | Cuentagotas y borrador | `px-ui`, `px-app` | ninguna |
| 3 | **17** | Menú, barra de herramientas y botones de zoom | `px-ui`, `px-app` | ninguna |
| 4 | **17b** | Damero de transparencia **de fondo del lienzo** (unidad corta; solo pintado, no depende de la textura) | `px-ui` | ninguna |
| 5 | **18** | Diálogos modales y campos numéricos | `px-ui` | ninguna |
| 6 | **19** | Nueva imagen (la spec, aprobada el 2026-10-19, no toca `px-core`: [`hito-2-nueva-imagen.md`](hito-2-nueva-imagen.md), D10) | `px-app`, `px-ui` | ninguna |
| 6b | **19b** | «Color…» del fondo del lienzo (unidad corta tras la 19: un diálogo con un campo hexadecimal, reutilizando el de la 11c y la infraestructura de la 18; nota de [`hito-2-damero-lienzo.md`](hito-2-damero-lienzo.md)) | `px-ui`, `px-app` | ninguna |
| 7 | **20** | Convertir de modo (spec: [`hito-2-convertir-modo.md`](hito-2-convertir-modo.md); menú `Imagen` nuevo, sin tocar `px-core`) | `px-app`, `px-ui` | ninguna |
| 8 | **13b** | Chunks auxiliares del PNG en el informe de lectura | `px-formats`, `px-app` | ninguna |
| 9 | **12** | Acceso por filas y bytes a `Image`, con `uses_index`, `filled`, `try_clone` y `from_bytes` (spec: [`hito-2-image-filas-bytes.md`](hito-2-image-filas-bytes.md); solo aditivo, sin las limpiezas de API de [`hito-2-notas-unidad-14.md`](hito-2-notas-unidad-14.md), que cambian pruebas) | `px-core`, `px-formats`, `px-ui`, `px-app` | ninguna |
| 10 | **13** | Escritor de PNG por flujo y lectura sin reserva anticipada | `px-formats` | ninguna |
| 11 | **14a** | Zoom 1/2, 1/4 y 1/8, textura por regiones, superposición del trazo como textura o sombreador (con el damero de la superposición del borrador) y medición del editor de color | `px-ui` | ninguna |
| 11b | **14-limpieza** | Unidad corta junto a la 14a: dos limpiezas de API que cambian pruebas anteriores (D13), con spec propia y visto bueno antes de tocar nada | `px-ui`, `px-app` | ninguna |
| 12 | ~~**14b**~~ | **DESCARTADA para el hito 2** (prueba manual del usuario; ver D2). Era: índices en la GPU con la paleta en un sombreador, solo si la medición de 14a lo justificaba | ~~`px-render`~~ | ~~`glow` y `egui_glow` como directas~~ (no se añaden) |
| 13 | **14c** | Tope de tramos del relleno y tope de memoria del peine | `px-core`, `px-raster` | ninguna |
| 14 | **21** | Puerta del hito: abrir, editar y guardar un PNG real | `px-app`, `px-golden` | ninguna |

## Decisiones

- **D1. Herramientas.** Pincel, rectángulo y elipse pasan del hito 2 al hito 4 (ya corregido en `docs/PLAN.md`). Se quedan cuentagotas y borrador (17a), que no tocan `px-core`:
  - **Borrador** = lápiz con el valor transparente. Indexado: el índice transparente; si la imagen no tiene, mensaje claro. RGBA y GrayAlpha: el transparente canónico. Gray: desactivado con aviso.
  - **Cuentagotas.** Indexado: elige la entrada. Resto de modos: cambia el color de la ranura activa.
  - Cada uno es un `Command` de `Document` o estado de sesión, según corresponda.
- **D2. Medir antes del sombreador.** 14a incluye medir el editor de color a 4096², 6000² y 16384²; con los números se decide 14b.
  - **Resultado:** la unidad 14b queda **DESCARTADA para el hito 2**. En la prueba manual del usuario, tras la 14a, el editor de color es fluido con un PNG indexado de 6000×6000 y de 8192×8192, tanto a zoom de trabajo como con la imagen entera visible. Se retoma solo si el ciclo de color de los hitos 5 o 7 lo exige (idea de diseño conservada en [`hito-2-notas-unidad-14.md`](hito-2-notas-unidad-14.md), «Idea de diseño: paleta en el sombreador»). Sin `px-render` ni `glow`/`egui_glow` como dependencias directas en este hito.
- **D3. Tamaño de la puerta.** 8192×8192 en RGBA (2²⁶ píxeles, el tope de apertura de hoy, `MAX_OPEN_PIXELS`) y un PNG indexado de 6000×6000. Por encima, un mensaje claro hasta que existan texturas por regiones.
- **D4. Menú** dentro de la ventana en los tres sistemas (sin barra de menú nativa de macOS).
- **D5. Barra de herramientas:** columna estrecha a la izquierda del panel de paletas, con botones de texto. La geometría y el mínimo de ventana (ancho y alto) se fijan **una sola vez**, en la spec de la unidad 17.
- **`rfd`** (unidad 16):
  - Feature `wayland` en Linux.
  - Diálogos no bloqueantes, consultados en cada fotograma y detrás de un trait, para probarlos sin ventana.
  - `zenity` como respaldo aceptado.
  - Sin portal ni `zenity`: aviso claro en la barra de estado, y arrastrar y soltar y `Ctrl`+`S` siguen funcionando.
  - El `unsafe` de `rfd` se anota en `docs/DEPENDENCIES.md`.
- **D7. Paleta inicial de «Nueva imagen»** (19): generada por una regla documentada, de **32 colores**: entrada 0 transparente (índice transparente 0), ocho matices en tres luminosidades y siete grises. Con pruebas (32 entradas, distintas entre sí, determinista). Fondo transparente por defecto o un color elegido. Lado máximo 8192 y `MAX_BYTES`.
- **D8. Convertir** (20): el cuantizador es una unidad aparte, **fuera del hito 2 y anotada para el hito 4**; la interfaz solo explica `TooManyColors`. Para `drop_alpha` no hay fondo por defecto: se elige blanco, negro, el color activo o un hexadecimal.
- **D9. Puerta** (21): la prueba va en `px-app` y `px-golden`, sin xvfb ni cambios en el CI (no `px-cli`, que no tiene código hasta el hito 8). **Unidad 13b:** la lectura de PNG lista en su informe los chunks auxiliares ignorados (`iCCP`, `gAMA`, `sRGB`, `pHYs`, texto…); con `iCCP`, `Ctrl`+`S` guarda una copia, como con las conversiones, y el título lo explica. El usuario probó a mano la puerta actual en Linux, macOS y Windows (todo bien) y la repetirá al final del hito.
- **D10. Rellenos y memoria** (14c): tope de tramos del relleno con error claro y constante con nombre (sin hilo aparte), y tope de memoria del peine.
- **D11. Damero de transparencia** (nota de [`hito-2-notas-unidad-14.md`](hito-2-notas-unidad-14.md), aprobada 2026-10-18). El damero **de fondo del lienzo** es la unidad **17b**, corta y tras la 17: solo cambia `Canvas::paint` (el fondo liso `BACKGROUND` pasa a damero) y la fila 10 de [`hito-2-cuentagotas-borrador.md`](hito-2-cuentagotas-borrador.md) para el fondo. El damero **de la superposición del borrador** va **dentro de la 14a**, junto a la superposición del trazo como textura o sombreador (medida del zigzag a 4096² en las notas de la 14): su forma depende de cómo la textura resuelva «borrar», y con formas por píxel empeoraría el retraso medido. Cada una tendrá su spec y su visto bueno.
- **D12. «Color…» del fondo del lienzo** (aprobada 2026-10-19, en la spec de la unidad 18): queda fuera de la 18 y es la unidad corta **19b**, tras la 19, con un diálogo de un campo hexadecimal ([`hito-2-fondo-color.md`](hito-2-fondo-color.md)). **Pendiente para la unidad de ajustes del hito 8:** guardar en disco la preferencia del fondo del lienzo, incluido el color elegido, y leerla al arrancar (hoy es solo de la sesión). Cada una tendrá su spec y su visto bueno.
- **D13. Unidad corta de limpieza junto a la 14a** (asignada el 2026-10-19 al aprobar la unidad 12, que las dejó fuera por cambiar pruebas anteriores). Cada una necesita su visto bueno y el aviso de la lista de pruebas que cambian; esta es la lista de partida, a confirmar con la spec de la unidad:
  - **`View::screen_to_image_i32`: quitar el parámetro `Size` sin usar (`_image`).** Cambia la definición ([`view.rs`](../crates/px-ui/src/view.rs)), su única llamada de producción (`canvas.rs`) y las pruebas que lo llaman, **medidas** en [`hito-2-limpieza-api.md`](hito-2-limpieza-api.md): `view_i32.rs` (4 pruebas, 14 llamadas), `view_ops.rs` (1 prueba, 2 llamadas) y **`zoom_ladder.rs`** (5 pruebas, 12 llamadas; de la 14a-1, no estaba en la lista de partida). Cambia solo la firma: ningún valor esperado.
  - **`TitleInfo`: campo propio `tool: Option<Tool>` en vez de meter `status_text` dentro de `color`.** Cambia `crates/px-app/src/title.rs` (`title_text` escribe la herramienta; `window_title` y `status_line` pierden el argumento `tool`) y los literales de sus **tres** módulos de pruebas con `TitleInfo` (`tests`, `window_title_tests` y `status_line_tests`; `file_title_tests` no usa `TitleInfo`): 27 literales, de los que **7** enumeran todos los campos y necesitan `tool: ..` (los otros 20 usan `..base()`/`..completo()`), y **15 llamadas** a `window_title`/`status_line` pierden el argumento; en total 14 pruebas y 4 ayudas, más el literal de `main.rs`, que pasa a `app_title_info`. Los textos esperados no cambian. Si algún texto esperado tuviera que cambiar, me paro.
  - Total: **24 pruebas anteriores** adaptadas (10 en `px-ui`, 14 en `title.rs`). Spec aprobada el 2026-10-20.

## Notas para el hito 4

- Pincel, rectángulo y elipse.
- **Cuantizador** de color (RGBA o gris con alfa a indexado con más de 256 colores): hoy `convert_image` da `TooManyColors` ([`hito-2-conversion-modos.md`](hito-2-conversion-modos.md)).

## Comprobado para `rfd` (antes de este reparto)

Hecho en una copia del repositorio, fuera de él:

- `rfd` 0.17.2 (2026-01-12), licencia MIT. Sus dependencias directas son `libc`, `log`, `percent-encoding`, `pollster` y `raw-window-handle` (más `windows-sys` en Windows y la familia `objc2`, ya presente por `eframe`, en macOS).
- `cargo deny --locked check` da `advisories ok, bans ok, licenses ok, sources ok` sin tocar `deny.toml`, con `default-features = false` y, solo en Linux, `xdg-portal` y `wayland`. `Cargo.lock` gana 2 entradas (`rfd`, `pollster`).
- `cargo check -p px-app --locked` compila en Linux con esa configuración. **No** se ha compilado en Windows ni macOS.
- En Linux carga `libdbus-1.so` con `dlopen` en ejecución (sin paquetes de desarrollo ni paso `apt` en el CI) y cae a `zenity` si el portal falla. Necesita `xdg-desktop-portal` con un backend de GTK, GNOME o KDE (el de wlroots solo no sirve).
