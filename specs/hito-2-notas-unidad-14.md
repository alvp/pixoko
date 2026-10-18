# Hito 2 · Unidad 14: notas pendientes (consolidación)

Solo notas: no es una spec de comportamiento ni tiene tests. Al empezar la unidad 14 se escribirá su spec propia, que enlazará con esta página. Cada punto remite a la spec donde se anotó, que sigue siendo la fuente.

## Rellenos

- **Tope de tramos por relleno, o relleno fuera del hilo de la interfaz.** El relleno corre síncrono; con 16384² serían del orden de 5 s con la ventana bloqueada (estimado, sin medir). [`hito-2-linea-relleno-app.md`](hito-2-linea-relleno-app.md), «Errores» (alcance de «no colgarse») y «Para la unidad 14».
- **Pico de memoria del peine en 16384×16384 RGBA.** Unos 5,3 GiB con `apply` (estimado, sin medir). Con *overcommit* un `try_reserve*` con éxito no impide que el sistema mate el proceso. Valorar un tope configurable que dé error antes de reservar la salida, y medir la memoria residente antes de fijar su valor por defecto. [`hito-2-relleno.md`](hito-2-relleno.md), «Para la unidad 14» y comportamiento 7.

## Lienzo y textura

- **Textura por regiones y superposición del trazo.** Actualizar solo la caja envolvente tras trazo, línea, relleno, `undo` y `redo` (hoy `to_rgba8` es O(ancho·alto) cada vez). La superposición del trazo como textura pequeña, si la medida lo pide. [`hito-2-lapiz-deshacer.md`](hito-2-lapiz-deshacer.md), «Para la unidad 14» y §2 (umbral de fluidez); [`hito-2-linea-relleno-app.md`](hito-2-linea-relleno-app.md), «Textura».
- **Vista previa del editor de color.** [`hito-2-editar-color.md`](hito-2-editar-color.md), decisión 7 y prueba manual 9(c). **Medida del usuario (2026-10-18):** la vista previa regenera la textura entera en cada cambio de valor. Con un PNG indexado de 4096×4096 (16 M de píxeles) va ligeramente lenta; con 6000×6000 (36 M), la lentitud es notable. La actualización por regiones no la arregla: editar una entrada de la paleta cambia píxeles repartidos por toda la imagen.

## API

- **`screen_to_image_i32` conserva un parámetro `Size` sin usar** (`_image`, [`view.rs`](../crates/px-ui/src/view.rs)). Quitarlo toca las 14 llamadas de `crates/px-ui/tests/view_i32.rs` y la de `canvas.rs`. [`hito-2-linea-relleno-app.md`](hito-2-linea-relleno-app.md), decisión 7.
- **Campo propio `tool` en `TitleInfo`**, en vez de meter `status_text` dentro de `color`. Tocaría los 13 literales de los tests de la unidad 6. [`hito-2-linea-relleno-app.md`](hito-2-linea-relleno-app.md), decisión 6.
- **Acceso por filas (y bytes) a `Image`.** Unidad 12 del hito. [`hito-1-png.md`](hito-1-png.md), «Para el hito 2»; reparto en [`hito-2-hsl.md`](hito-2-hsl.md), «Reparto de las notas».

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
