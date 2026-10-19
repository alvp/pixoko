# Hito 2 · Unidad 14: notas pendientes (consolidación)

Solo notas: no es una spec de comportamiento ni tiene tests. Al empezar la unidad 14 se escribirá su spec propia, que enlazará con esta página. Cada punto remite a la spec donde se anotó, que sigue siendo la fuente.

## Rellenos

- **Tope de tramos por relleno, o relleno fuera del hilo de la interfaz.** El relleno corre síncrono; con 16384² serían del orden de 5 s con la ventana bloqueada (estimado, sin medir). [`hito-2-linea-relleno-app.md`](hito-2-linea-relleno-app.md), «Errores» (alcance de «no colgarse») y «Para la unidad 14».
- **Pico de memoria del peine en 16384×16384 RGBA.** Unos 5,3 GiB con `apply` (estimado, sin medir). Con *overcommit* un `try_reserve*` con éxito no impide que el sistema mate el proceso. Valorar un tope configurable que dé error antes de reservar la salida, y medir la memoria residente antes de fijar su valor por defecto. [`hito-2-relleno.md`](hito-2-relleno.md), «Para la unidad 14» y comportamiento 7.

## Lienzo y textura

- **Textura por regiones y superposición del trazo.** Actualizar solo la caja envolvente tras trazo, línea, relleno, `undo` y `redo` (hoy `to_rgba8` es O(ancho·alto) cada vez). La superposición del trazo como textura pequeña, si la medida lo pide. [`hito-2-lapiz-deshacer.md`](hito-2-lapiz-deshacer.md), «Para la unidad 14» y §2 (umbral de fluidez); [`hito-2-linea-relleno-app.md`](hito-2-linea-relleno-app.md), «Textura».
- **Medida del usuario (2026-10-18): la superposición del trazo ya no basta.** Con un PNG RGBA de 4096×4096 en `--release` a escala 1,0, un zigzag largo con el borrador va con **retraso apreciable respecto al ratón**. Coincide con la predicción de la spec ([`hito-2-cuentagotas-borrador.md`](hito-2-cuentagotas-borrador.md), «Lo comprobado»): el coste de la superposición es lineal en los píxeles del trazo (~0,13 µs por píxel solo en CPU) y un zigzag largo en esa imagen pasa de los **50 000 a 100 000 píxeles por trazo**, que es donde se acaba el fotograma de 16 ms. **Consecuencia para la 14a:** la superposición del trazo (lápiz, línea y borrador) deja de ser una forma por píxel y pasa a ser **una textura pequeña o un sombreador**. Dibujar con el lápiz y la línea comparte el mismo cuello de botella aunque la medida se hiciera con el borrador.
- **Damero de transparencia.** Hoy el lienzo muestra la transparencia con un fondo liso (`BACKGROUND`, gris 40): un píxel borrado y uno transparente se ven igual, y un editor de pixel art necesita un damero detrás de la imagen **y en la superposición del borrador** (los píxeles con alfa 0 deben pintar el damero, no el fondo). Cambia `Canvas::paint` y la fila 10 de [`hito-2-cuentagotas-borrador.md`](hito-2-cuentagotas-borrador.md) (hoy compara con `BACKGROUND`; ver su sección «Para el damero»). **Propuesta (una línea):** el damero de fondo, en una unidad corta propia tras la 17 (solo pintado; no depende de la textura), y el de la superposición del borrador dentro de la 14a, porque su forma depende de cómo la textura o el sombreador resuelvan «borrar» (con una superposición de formas por píxel el damero empeoraría el retraso medido arriba).
- **Damero y fondo elegible en la 14a (unidad 17b, [`hito-2-damero-lienzo.md`](hito-2-damero-lienzo.md)).** La 17b pinta el damero de fondo y el de la superposición con formas por píxel (cuadriláteros de la textura 2×2, en dos pasadas con alfa parcial; con un fondo sólido, una pasada de rectángulos). Al pasar la superposición del trazo a textura o sombreador, la **14a debe incluir el damero y la opción de fondo** (`CanvasBackground`: Damero, Negro, Gris, Blanco): «borrar» y el alfa parcial deben resolverse sobre el fondo elegido, con el mismo oráculo (`checker::shade_at` y `background_fill`) y pasando las pruebas de la 17b (filas 7 y 14, propiedad 3), también con `PIXOKO_DAMERO_MALLA=1`.
- **Vista previa del editor de color.** [`hito-2-editar-color.md`](hito-2-editar-color.md), decisión 7 y prueba manual 9(c). **Medida del usuario (2026-10-18):** la vista previa regenera la textura entera en cada cambio de valor. Con un PNG indexado de 4096×4096 (16 M de píxeles) va ligeramente lenta; con 6000×6000 (36 M), la lentitud es notable. La actualización por regiones no la arregla: editar una entrada de la paleta cambia píxeles repartidos por toda la imagen.

## API

- **`Image::uses_index` (o un histograma de índices), aditivo en `px-core`**, para que quitar una entrada de la paleta no recorra los píxeles con `get_pixel` (≈ 1,3 s con 16384² y la entrada sin usar, estimado). [`hito-2-paleta-operaciones.md`](hito-2-paleta-operaciones.md), «Para la unidad 14».
- **`screen_to_image_i32` conserva un parámetro `Size` sin usar** (`_image`, [`view.rs`](../crates/px-ui/src/view.rs)). Quitarlo toca las 14 llamadas de `crates/px-ui/tests/view_i32.rs` y la de `canvas.rs`. [`hito-2-linea-relleno-app.md`](hito-2-linea-relleno-app.md), decisión 7.
- **Campo propio `tool` en `TitleInfo`**, en vez de meter `status_text` dentro de `color`. Tocaría los 13 literales de los tests de la unidad 6. [`hito-2-linea-relleno-app.md`](hito-2-linea-relleno-app.md), decisión 6.
- **Acceso por filas (y bytes) a `Image`.** Unidad 12 del hito, ahora con spec propia (pendiente de visto bueno): [`hito-2-image-filas-bytes.md`](hito-2-image-filas-bytes.md), que también recoge `uses_index`, `Image::filled` e `Image::try_clone` de los puntos vecinos; `convert_report` **no** entra en ella y sigue aquí; las dos limpiezas de API (`screen_to_image_i32`, `TitleInfo.tool`) tampoco, y están asignadas a una unidad corta junto a la 14a ([`hito-2-reparto-cierre.md`](hito-2-reparto-cierre.md), D13). [`hito-1-png.md`](hito-1-png.md), «Para el hito 2»; reparto en [`hito-2-hsl.md`](hito-2-hsl.md), «Reparto de las notas».
- **`Image::filled(width, height, pixel)` e `Image::try_clone()`, aditivos en `px-core`**, candidatos de la unidad 12 para abaratar la creación de lienzos grandes: la unidad 19 (nueva imagen) rellena con `set_pixel` y construye dos imágenes (≈ 0,9 s a 8192² RGBA con fondo blanco, medido en release; ≈ 0,15 s con estas dos). `px-core` no se tocó en la 19 por decisión del 2026-10-19. [`hito-2-nueva-imagen.md`](hito-2-nueva-imagen.md), D10.
- **`convert_report(image, target) -> Result<ConversionReport, ConvertError>`, aditivo en `px-core`**, candidato de la unidad 12: la pasada 1 de `convert_image` sin construir la imagen. La unidad 20 (convertir de modo) calcula el informe con `convert_image(.., Allowed::NOTHING)`, que cuando la conversión es exacta construye y descarta la imagen convertida; a 8192² el informe cuesta hasta 0,86 s (a indexado) y esta función lo dejaría en la mitad. `px-core` no se tocó en la 20. [`hito-2-convertir-modo.md`](hito-2-convertir-modo.md), D3.

## PNG

- **Escritor por flujo.** `write_png` sobre un `impl Write`, fila a fila: el pico de memoria (hoy ~2 GiB con un lienzo en el límite) baja a la `Image` más una fila. Necesita el acceso por filas. [`hito-1-png.md`](hito-1-png.md), «Para el hito 2».
- **Lectura sin reserva anticipada.** `read_png` reserva hasta 1 GiB antes de saber si el IDAT trae datos (un fichero de ~70 bytes truncado cuesta 1 GiB de RSS). Comprobar el tamaño real antes de reservar, o leer por flujo. [`hito-1-png.md`](hito-1-png.md), «Para el hito 2»; medida en [`hito-1-fuzz.md`](hito-1-fuzz.md).

## Idea de diseño: paleta en el sombreador

**Propuesta.** Para imágenes indexadas, subir a la GPU la imagen de **índices** (una textura de un canal de 8 bits) y la paleta (256 entradas RGBA, otra textura o un arreglo uniforme), y resolver el color en un programa de sombreado de fragmentos. Con `eframe` en `glow`, se hace con `egui_glow` y un `PaintCallback` en el lienzo.

**Qué da.**

- Editar un color de la paleta cuesta **O(256)** (subir la paleta), no O(píxeles). La vista previa del editor de color y el arrastre dejan de depender del tamaño de la imagen: se arregla la medida de arriba de raíz.
- Un trazo o relleno sobre una imagen indexada sube solo los índices de la región (junto con la textura por regiones de arriba).
- Sirve para el **ciclo de color** de los hitos 5 y 7: la fase es un uniforme y el sombreador desplaza los índices del rango, sin tocar el documento ni subir nada por fotograma. [`hito-2-paleta-editar.md`](hito-2-paleta-editar.md), «Para el hito 5 o 7: ciclo de color como animación».

**Riesgos.**

- **macOS:** OpenGL está obsoleto y se queda en 4.1 (sin 4.3, sin cómputo); el sombreador tendría que ser GLSL 3.30. Hoy el proyecto elige `glow` sabiendo esto ([`hito-2-ventana.md`](hito-2-ventana.md), «glow frente a wgpu»); esta idea aumenta lo que depende de OpenGL, y volver a `wgpu` costaría reescribir el sombreador en WGSL.
- **Nitidez:** hay que garantizar muestreo `NEAREST` en la textura de índices y **sin filtrado de la paleta**; un filtrado lineal sobre índices mezclaría colores inexistentes. Los bordes con zoom no entero (1/2, 1/4, 1/8) y la escala de pantalla fraccionaria necesitan comprobación en pantalla.
- **La prueba sin ventana:** el CI no abre un contexto OpenGL, así que el sombreador no se puede probar tal cual. Mitigación: mantener `to_rgba8` como **referencia en CPU** y comprobar con tests que la resolución de índices con paleta da los mismos bytes (la lógica pura), dejando para la prueba manual solo que la GPU pinta igual.
- **`unsafe`:** las llamadas de `glow` son `unsafe`; solo se permitirían en `px-render`, con su comentario, como pide `CLAUDE.md`. Hoy `px-ui` no lo necesita.
- **Alcance:** solo cubre imágenes indexadas. RGBA, Gray y GrayAlpha siguen con la textura de bytes y la subida por regiones.
- **Dos caminos de pintado** (textura RGBA e índices con paleta) que mantener iguales; la capacidad de la GPU (`max_texture_side`, [`hito-2-ventana.md`](hito-2-ventana.md)) vale para los dos.
