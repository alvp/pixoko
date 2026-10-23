# Diseño de interfaz y comandos de Pixoko

Oct 20, 2026 · @Álvaro Pérez Pozo

## Principios

Pixoko toma del programa de referencia la organización y el modo de trabajar, y nada de sus gráficos.

- **Se mantiene:** la disposición de la pantalla, el esquema de dos colores, las convenciones del ratón, las letras de los atajos y la estructura de menús.
- **Se hace propio:** los iconos (dibujados nuevos, con el propio Pixoko), la tipografía (Departure Mono), los colores de la interfaz y todos los textos.
- **Se deja fuera** lo que solo tenía sentido en el Amiga de 1994 (sección 7).
- **Estilo:** moderno, el de la aplicación actual: fondos oscuros y acento verde.

Regla de uso del manual: es una referencia privada, guardada fuera del repositorio. Las funciones y los comportamientos no están protegidos, pero el texto y las imágenes sí. Por eso las specs lo describen con palabras propias, sin copiar frases ni gráficos.

## Disposición de la pantalla

La página ocupa la izquierda y todos los controles van a la derecha, en una sola columna de ancho ajustable (C2), como en el programa de referencia.

&#91;embedded content: disposición de la pantalla · lienzo, paleta y caja de herramientas\]

El indicador de color y el panel de opciones son las dos piezas nuevas; el resto ya existe y solo cambia de lado.

| Zona | Contenido | Estado hoy |
| --- | --- | --- |
| Arriba | Barra de menús, siempre visible (el original la muestra con el botón derecho sobre la barra de título) | Existe, con 5 menús |
| Izquierda | La página o lienzo, con zoom y desplazamiento | Existe |
| Derecha, junto al lienzo | Panel de la paleta, bajo la caja: el indicador de color grande arriba, la rejilla compacta, la fila de iconos de la paleta y, a demanda, el editor de color | Hecho (I.2a, I.3, C1, C2): parte de la columna única; el editor sale con `p` o con el clic derecho en el indicador |
| Derecha, en el borde | Caja de herramientas, arriba de la columna: herramientas y botones de zoom; las opciones de la herramienta activa van en una ventana flotante | Hecho (I.3, I.4, C2): una rejilla de iconos de 24 u en filas de `c(k)` botones; el clic derecho en una herramienta la elige y abre o cierra su ventana de opciones (224 × (104 + 18·f) u: nombre, atajo, `sin opciones` y ayuda breve) |
| Abajo | Barra de estado con herramienta, coordenadas, color y avisos | Existe |

**Hecho en C2 (23 de octubre de 2026):** una sola columna en el borde derecho (caja · indicador · rejilla · iconos · editor) de ancho ajustable arrastrando su borde izquierdo (zona de 4 u, pasos de una columna de muestras de 14 u: `W(k) = 14k + 9`, de 10 columnas (149 u, la menor que contiene entera la fila de iconos de la paleta) a lo que deje 128 u al lienzo; 16 = 233 u por defecto; la columna cede antes que el lienzo y vuelve al ancho elegido al ensanchar la ventana). Los botones de la caja se reparten en filas de `c(k) = ⌊(14k+2)/25⌋`. Con el editor abierto o una quita pendiente la columna es de al menos 16 columnas (el editor mide 233 u). El recuadro de opciones desaparece: el clic derecho en una herramienta la elige y abre o cierra una **ventana flotante** no modal, una sola, con posición única y desplazamiento por herramienta; `Esc` la cierra antes que el editor. Con la ventana baja el bloque entero (caja incluida) se desplaza con la rueda; al abrir el editor el desplazamiento va al máximo y al cerrarlo vuelve al anterior. Menú y barra de estado dejan de tener cursor de redimensionar. Las ventanas mínimas no cambian (313 u de alto, 468 u de ancho: el ancho lo fijan ahora las 66 columnas de la barra de estado, no la columna). Spec: `specs/ui-single-column.md`. **Mejora futura anotada:** el editor de color como ventana flotante.

**Hecho en C1 (22 de octubre de 2026):** el editor de color deja de estar siempre abierto (`p`, o clic derecho en el indicador; se cierra con `Esc`, con `p`, con un segundo clic derecho en el rectángulo que edita o al cambiar de documento) y la rejilla gana su espacio; el indicador son dos rectángulos de 40 × 24 y 64 × 32 u en una banda de 44 u (el borde del rol que edita el editor mide 2 u `ACCENT`); la rejilla es compacta (muestras de 12 u, huecos de 2 u, paso de 14 u); la información de la muestra sale en la barra de estado (la rejilla no tiene tooltip; el indicador sí); las cinco operaciones de paleta viven en una fila de iconos justo bajo la rejilla (anclada al fondo si la rejilla es más alta que el espacio; 28 u, con el editor abierto y cerrado). El bloque mide 277 u y la ventana mínima baja de 341 a **313 u** (417,33 pt a `ppp` 1,5), el suelo que pone el panel de capas flotante; el criterio del alto pasa del 65 % al **60 %** de 720 pt (432). El ancho mínimo (468 u) no cambia. Spec: `specs/ui-composition.md`.

**Hecho en I.3 (21 de octubre de 2026):** el orden de izquierda a derecha es lienzo, paleta y caja de herramientas, y el ancho mínimo de ventana (468 u) no cambió: 128 u de lienzo, 233 u de paleta y 107 u de caja. El lienzo empieza en `(0, 16·n)` y `chrome::window_layout` es la única función que conoce el orden. Spec: `specs/interfaz-caja-derecha.md`.

Tres puntos que condicionan la geometría:

- **Iconos.** Hecho en I.4 (21 de octubre de 2026): las herramientas, el zoom y el editor de la paleta usan 14 iconos propios de 16 × 16 (`assets/icons/`) sobre botones de 24 u con relieve; una rejilla de cuatro columnas dejaba sitio a 16 herramientas en el alto mínimo (con C1 y C2, la ventana mínima aloja hasta **20** herramientas a 10 columnas (el mínimo), cuatro filas, sin recuadro de opciones: la ventana de opciones flota). Spec: `specs/interfaz-iconos.md`.
- **Panel de opciones.** Su altura se reserva siempre, también cuando la herramienta no tiene opciones, para que la caja no salte al cambiar de herramienta.
- **Ocultar paneles.** F10 oculta la caja y la barra, y F9 solo la barra, como en el original. Es una mejora posterior; la barra de menús y la de estado se quedan siempre.

## Dos colores y convenciones del ratón

Pixoko pasa de un color activo a dos, primer plano y fondo, y el botón derecho pinta con el fondo.

| Gesto | Efecto |
| --- | --- |
| Botón izquierdo sobre el lienzo | Pinta con el color de primer plano |
| Botón derecho sobre el lienzo | Pinta con el color de fondo («borrar al fondo») |
| Clic en una muestra de la paleta | Elige el primer plano |
| Clic derecho en una muestra | Elige el fondo |
| Clic en el indicador de color, o la tecla coma | Cuentagotas: clic elige el primer plano y clic derecho el fondo |
| Clic derecho en el indicador de color | Abre el editor de color con el color de ese rectángulo (con el editor abierto: cambia de rectángulo; en el que se edita: lo cierra) |
| Clic derecho en una herramienta | Lleva el foco al panel de opciones de esa herramienta (sección 6) |
| Clic derecho en el selector de pincel | Restaura el último pincel propio |
| Clic en el botón de zoom y clic derecho | Acerca y aleja |

El indicador de color son dos rectángulos superpuestos: el pequeño es el primer plano y el grande, el fondo.

Propuesta de fondo por defecto, según el modo de la imagen:

- **RGBA y gris con alfa:** transparente. Pintar con el botón derecho borra, y la herramienta Borrador se elige con `k` (desde el 21 de octubre).
- **Indexado:** la entrada transparente si existe y, si no, la 0.
- **Gris:** blanco, porque no tiene transparencia.

Borrar la página (tecla K) la rellena con el color de fondo.

Qué cambia en el código: hoy hay un solo color activo, que pasa a ser un par. Lo usan el relleno, el borrador, el cuentagotas, la barra de estado y el título de la ventana, y los botones del ratón llegan a las herramientas con su botón de origen.

### Rueda, botón central, botones extra y gestos

Navegar por la imagen se hace con la rueda, el botón central y los gestos del trackpad, sin teclas ni barras, y cualquier botón extra del ratón tiene una acción por defecto.

| Gesto | Efecto | Estado |
| --- | --- | --- |
| Rueda sobre el lienzo | Zoom anclado al cursor, un paso por muesca | Hecho (I.2b) |
| Botón central y arrastrar | Desplaza la vista | Hecho (I.2b: por la tabla) |
| Espacio y arrastrar con el botón izquierdo | Desplaza la vista, la «sobre» habitual de los editores | Hecho (I.2b); `Espacio` a mitad de un trazo lo cancela |
| Mayús y rueda | Desplaza en horizontal | Hecho (I.2b) |
| Rueda sobre la paleta | Desplaza la rejilla de la paleta | Existe |
| Dos dedos en el trackpad | Desplaza la vista | Hecho (I.2b) para `Point` (macOS y Wayland); en X11 y Windows llegan como líneas: pendiente de la prueba manual |
| Pellizco en el trackpad | Zoom | Hecho (I.2b, acumulador); en Linux el pellizco no llega (`winit` 0.30.13) |
| Botón lateral «atrás» (el 4) | Deshacer | Hecho (I.2b), reasignable en la tabla |
| Botón lateral «adelante» (el 5) | Rehacer | Hecho (I.2b), reasignable en la tabla |
| Botones 6 y siguientes | No llegan a la app (`egui-winit` los descarta); se reasignan en el sistema | Imposible hoy |

Reglas:

- **Rueda frente a trackpad:** el sistema entrega las ruedas como líneas y los trackpads como píxeles. Las líneas hacen zoom y los píxeles desplazan, así que el mismo gesto de dos dedos no hace zoom por error. Ya había un aviso de que el desplazamiento del panel con un trackpad podía salir rápido o lento.
- **Espacio:** en el original cancela la operación en curso. Aquí cancela solo mientras haya un gesto o un diálogo en curso; si no, es la tecla de la mano para desplazar.
- **Zoom por pasos:** los niveles son potencias de 2 para que el píxel de la imagen caiga siempre en píxeles enteros de pantalla. El pellizco acumula el gesto y da un paso al cruzar un umbral; una animación entre pasos sería solo estética.
- **Botón derecho:** pinta con el fondo y no abre menús contextuales sobre el lienzo, para no chocar con el esquema de dos colores. Tampoco desplaza.
- **Con el botón pulsado:** el zoom y el desplazamiento siguen permitidos mientras se dibuja, como hoy.
- **Configurable:** la tabla de gestos son datos, como la de atajos, y el hito 8 la hará editable, incluidos los botones extra.

**Soporte mínimo (decidido el 20 de octubre de 2026; limitación anotada el 21):** cinco botones, todos reasignables: izquierdo, central y derecho con asignación propia, más **dos extra** (4 y 5, `Back` y `Forward`), que se pueden reasignar a cualquier acción de la tabla de atajos. **Del botón 6 en adelante no llega nada a la app:** `egui-winit` descarta `MouseButton::Other` antes de que egui lo vea y `eframe` no deja engancharse antes; la salida es reasignarlos en el sistema a los extra 4 y 5 o a teclas. Spec: `specs/interfaz-raton.md`. Quien no tenga tres botones o un trackpad llega a lo mismo con gestos y teclas: Espacio y arrastrar para desplazar, y rueda y pellizco para el zoom. 

## Atajos de teclado

Las herramientas usan las letras del programa de referencia, y los atajos modernos (`Ctrl+Z`, `Ctrl+S`…) se quedan además de los clásicos.

| Función | Tecla por defecto | Hoy | Hito |
| --- | --- | --- | --- |
| Mano alzada continua (el lápiz actual) | `d` (`D`: forma libre rellena) | `P` | 2 |
| Mano alzada punteada | `s` | no existe | 4 |
| Línea | `v` | `L` | 2 |
| Curva | `q` | no existe | 4 |
| Relleno | `f` | `F` | 2 |
| Cuentagotas | `,` | `I` | 2 |
| Borrador | `k` (`K` con `Mayús` es Borrar página; el botón derecho con fondo transparente también borra) | `E` | 2 |
| Rectángulo, círculo, elipse, polígono | `r`, `c`, `e`, `w`; la mayúscula, con relleno | no existen | 4 |
| Selector de pincel y restaurar pincel | `b`, `B` | no existen | 4 |
| Pincel de un píxel | `.` | no existe | 4 |
| Texto, rejilla, simetría, lupa | `t`, `g`, `/`, `m` | no existen | por decidir |
| Deshacer y borrar la página | `u` y `K` con `Mayús` (más `Ctrl+Z`) | `Ctrl+Z` | 2 |
| Editor de color (abrir y cerrar) | `p` | no existe | C1 |
| Color de primer plano, siguiente y anterior | `]` y `[` | cifras 1 a 9 | 2 |
| Color de fondo, siguiente y anterior | `}` y `{` | no existe | 2 |
| Zoom | `<` y `>`, rueda y `Home` | `+`, `-`, rueda y `Home` | 2 |
| Centrar bajo el cursor | `n` | no existe | 2 |
| Cancelar y detener una operación | `Espacio` y `Esc` | `Esc` | 2 |
| Ciclo de color | `Tab` | no existe | 5 |
| Repetir la última orden | `a` | no existe | 8 |
| Modos de pincel | `Mayús`+`F1` a `Mayús`+`F8` (desde el 21 de octubre; `F1` es la ayuda) | no existen | 4 |
| Restringir el cursor y dejar trazas | `Mayus` y `Ctrl` mientras se dibuja | no existen | 4 |

**Hecho en la unidad I.1 (2026-10-20, `specs/interfaz-teclado.md`):** `d v f ,` (herramientas), `u` (más `Ctrl+Z`), `]` y `[` (también `Av Pág` y `Re Pág`), `<` y `>` (además de `+`, `=` y `-`), `n`, `Inicio`; el Borrador quedó sin letra (desde el 21 de octubre tiene `k`) y las cifras 1 a 9 sin asignar. Lo demás de la tabla sigue pendiente en su hito.

**Hecho en la unidad C1 (2026-10-22, `specs/ui-composition.md`):** `p` alterna el editor de color (`Ver → Editor de color`, editable en `Ayuda → Atajos de teclado…`); `p` ya no está reservada sin asignar. Siguen sin asignar `l i e` y `P L I E`.

**Hecho en la unidad I.2a (2026-10-20, `specs/interfaz-dos-colores.md`):** `}` y `{` (también `Mayús`+`Av Pág` y `Mayús`+`Re Pág`) cambian el fondo, `K` borra la página con él (también `Editar → Borrar página`), el botón derecho pinta con el fondo, el indicador de color y los dos anillos del panel. Con un trazo en curso el color ya se puede cambiar (afecta al trazo siguiente).

**Hecho en la unidad I.5 (2026-10-21, `specs/interfaz-ayuda.md`):** `F1` abre la ayuda de la herramienta activa o del botón de la caja bajo el puntero (y `Ayuda → Ayuda de la herramienta`); `Ayuda → Atajos de teclado…` lista los atajos con `Restablecer todo`; cada atajo se edita (principal y secundario, captura del carácter que produce la tecla, conflictos con reasignar o dejarlo) y se guarda en `atajos.txt` de la carpeta de configuración. `F1` queda **reservada** (no es reasignable: la tabla no tiene teclas de función) y los modos de pincel de la fila de arriba pasan de `F1`–`F8` a `Mayús`+`F1`–`F8` (hará falta `KeyId::Function(u8)` en el hito 4). Las acciones de archivo conservan sus combinaciones modernas; ninguna combinación con `Ctrl`/`Cmd` se puede asignar. Ante un choque con un atajo por defecto nuevo gana siempre el del usuario.

Reglas del esquema:

- **Cifras 1 a 9:** dejan de elegir color y se reservan para los controles de animación (hito 5).
- **`+` y `-`:** siguen siendo el zoom hasta que lleguen los pinceles; entonces pasarán a cambiar su tamaño, como en el original, y el zoom queda en `<`, `>`, la rueda y `Home`.
- **Texto activo:** con la herramienta Texto las letras escriben y no cambian de herramienta, igual que con un campo de texto enfocado.
- **Con el botón pulsado:** casi todos los atajos del original funcionan mientras se dibuja. Pixoko lo permite solo para acciones que no modifican el documento: zoom, desplazamiento, cambio de color y cancelar.
- **Sin `Alt`:** en KDE `Alt` con arrastre mueve la ventana, así que ningún atajo de ratón depende de esa tecla. Los rangos de color usan `Mayus` y `Ctrl`.
- **Datos, no código:** el esquema es una tabla de datos, que el hito 8 hará configurable.

## Menús

La barra de Pixoko sigue los nueve menús del original con nombres en español, y cada uno se añade cuando existe su función.

| Original | Pixoko | Qué contiene | Hito |
| --- | --- | --- | --- |
| Picture | Archivo e Imagen | Nueva, abrir, guardar; voltear, tamaño del lienzo, página de reserva, convertir modo | 2 (Archivo, Convertir modo); resto, 4 |
| Brush | Pincel | Copiar, pegar, restaurar, tamaño, voltear, borde, rotar, punto de agarre; deformar y cizalla, más tarde | 4 |
| Mode | Modo | Matte, Color y Reemplazar; después Difuminar, Mezclar, Sombrear, Suavizar y Mezcla | 4; resto, 7 |
| Anim | Animación | Fotogramas, control, ritmo, paleta por fotograma; después mover, cámara y pincel animado | 5; resto, 7 |
| (no existe) | Capa | Nueva capa, duplicar, quitar, subir, bajar, combinar abajo, combinar visibles, aplanar, renombrar, propiedades y mostrar el panel de capas (hecho en 3.4 y 3.5; atajos `;` `)` `(` y `'`; combinar y aplanar sin tecla por defecto). `Archivo → Exportar PNG aplanado…` (3.5) | 3 |
| Colour | Color | Rangos, ciclo, editor de color, copiar, intercambiar, degradado de paleta, borrar, cargar y guardar paleta, Remap | 4; rangos y ciclo, 5 |
| Effect | Efecto | Plantilla, mesa de luz, fijar fondo, antialias, proceso, translucidez, perspectiva, textura | 7 (opcional); mesa de luz, 5 |
| Macro y User | Macro | Grabar, ejecutar, cargar, guardar y diez ranuras | 8 |
| Prefs | Preferencias | Coordenadas, ciclo múltiple, guardar y restaurar preferencias | 8 |
| (no existe) | Ver y Ayuda | Zoom, fondo del lienzo; licencias | 2 |

El orden de la barra queda así: Archivo, Editar, Imagen, Pincel, Modo, Animación, Color, Efecto, Macro, Preferencias, Ver y Ayuda.

**Problema de ancho:** los cinco títulos de hoy suman 249 u y la ventana mínima mide 468 u. Con doce títulos, a unas 45 u cada uno, la barra pasa de 540 u y no cabe. Hay que decidir entre agrupar menús, un menú de desbordamiento al final de la barra o un ancho mínimo mayor (sección 8).

Otras cosas del original que se adoptan:

- **Marca de verificación** junto a las opciones activadas, como en las preferencias.
- **Atajo mostrado** en cada entrada, con `Cmd` en macOS.
- **Repetir la última orden** (`a`), que encaja con la tabla de acciones que ya comparten menús, botones y teclas.
- **Opciones de carga y guardado** de los diálogos de archivo: tamaño, número de colores y nombre del autor.

## Opciones de herramienta

Las opciones de la herramienta activa viven en un panel fijo bajo la caja de herramientas, no en ventanas que tapan la paleta como en el original; el clic derecho sobre una herramienta lleva el foco a ese panel.

Ventajas del panel fijo: la paleta no se oculta nunca, las opciones se ven mientras se dibuja y se pueden cambiar con el ratón sin abrir ni cerrar nada. Su altura se reserva siempre y, si una herramienta tiene más opciones de las que caben, el panel se desplaza por dentro.

| Herramienta | Opciones que trae el original | Hito propuesto |
| --- | --- | --- |
| Pinceles integrados | Tamaño, arrastrando en diagonal | 4 |
| Aerógrafo | Tipo (antiguo o estándar), radio de 1 a 100, flujo de 1 a 100, tamaño de la boquilla | por decidir |
| Línea, curva y formas sin relleno | Espaciado: continuo, número total de puntos o cada n puntos; aerógrafo | 4 |
| Relleno y formas rellenas | Tipo de relleno: sólido, pincel, envolver, perspectiva, patrón, desde pincel, rango, línea, forma, círculo, contorno, tramado | 4 (sólido, patrón, rango); resto, 7 |
| Selector de pincel | Restaurar el último pincel propio | 4 |
| Texto | Fuente, tamaño y estilo | por decidir |
| Rejilla | Espaciado en X y en Y; ajuste visual con una plantilla | 4 |
| Simetría | Tipo (punto, mosaico, cíclica, espejo), orden y colocación del centro | 4 |
| Lupa y zoom | Nivel de zoom | 2 (ya hecho con los botones de zoom) |

Hoy el lápiz, la línea, el relleno, el cuentagotas y el borrador no tienen opciones: la ventana de opciones (C2) enseña el nombre, el atajo de la tabla, `sin opciones` y una ayuda breve, y se llena con cada herramienta nueva (su cuerpo, de hasta 8 filas de 18 u, se desplaza por filas y se recuerda por herramienta). Cada fila de la tabla es una spec con su propio visto bueno. Las opciones del hito 4 caben en esa ventana de 224 u de ancho; el menú del pincel es una ventana aparte.

## Qué se descarta

Quedan fuera las funciones que dependen del Amiga, de sus modos de pantalla o de la memoria de 1994.

| Descartado | Motivo |
| --- | --- |
| Formato de pantalla y modos Amiga (HAM, Halfbrite, resoluciones, overscan) | Pixoko trabaja con imágenes de cualquier tamaño y modos indexado, gris y RGBA |
| Respaldo de 24 bits | Todo el modo RGBA ya es de 24 bits con alfa |
| Recompute y Fast Adjust | Solo tenían sentido con HAM |
| Workbench, Mouse Track, Icons, Font Dir | Son del sistema operativo del Amiga; Pixoko usa las fuentes y los iconos del sistema |
| Be Square | Los píxeles de hoy son cuadrados |
| Imprimir y Borrar archivo | Los hace el sistema operativo |
| Método de memoria de la animación (comprimido o expandido) | Pixoko elige su propia representación |
| Gestión de memoria, utilidad Player, ARexx | Los sustituyen los límites de memoria de la app, un reproductor propio (hito 5) y el scripting del hito 8 |
| Presión de tableta | Aplazado hasta que haya una tableta con la que probarlo |

Lo que se descarta se puede recuperar más tarde: esta lista es una decisión de orden de trabajo, no una prohibición.

## Decisiones abiertas y orden de trabajo

Antes de implementar hay cinco decisiones por tomar, y el trabajo se reparte en cinco unidades después de cerrar el hito 2.

| Nº | Decisión | Propuesta |
| --- | --- | --- |
| 1 | Doce menús no caben en la ventana mínima de 468 u | Menú de desbordamiento al final de la barra; los menús se añaden cuando existe su función |
| 2 | Fondo por defecto según el modo | Transparente en RGBA y gris con alfa; entrada transparente (o la 0) en indexado; blanco en gris |
| 3 | Selección de rangos de color | `Mayus` para un rango contiguo y `Ctrl` para añadir o quitar (el original usa `Alt`, que en KDE mueve la ventana) |
| 4 | Acciones permitidas con el botón pulsado | Zoom, desplazamiento, cambio de color y cancelar |
| 5 | Cuándo hacer los iconos propios | Antes de añadir las herramientas de formas del hito 4 |

**Decidido el 20 de octubre de 2026:** las cinco, tal como se proponen. Ese mismo día se añadió el apartado de la rueda, el botón central, los botones extra y los gestos del trackpad, dentro de la sección 3.

Orden de trabajo después del hito 2:

1. **Teclado:** tabla de atajos como datos y el esquema de letras de la sección 4.
2. **Dos colores y ratón:** el indicador de color, el fondo y el botón derecho.
3. **Caja a la derecha y panel de opciones:** la geometría se fija una sola vez, con el panel vacío al principio.
4. **Iconos propios:** dibujados con Pixoko y exportados como recursos de la aplicación.
5. **Menús del original:** se añaden a medida que llegan las funciones de cada hito.

**Excepción transitoria (I.2a, 2026-10-20; retirada en I.4, 2026-10-21: el criterio del alto es otra vez el 65 % de 720 pt, 468,0; ventana mínima de 341 u, 454,67 pt a `ppp` 1,5):** el indicador de color añade una banda de 36 u al panel de la paleta y el criterio del alto mínimo de ventana sube del 65 % al 72 % de 720 pt (518,4 pt; a `ppp` 1,5 la ventana mínima pasa de 468,0 a 516,0 pt). Con los iconos propios (I.4) las filas de botones del editor se redujeron a una y el criterio **volvió al 65 %**. Spec: `specs/interfaz-dos-colores.md`. **C1 (22 de octubre de 2026): el criterio del alto pasa del 65 % al 60 % de 720 pt (432 pt); ventana mínima de 313 u, 417,33 pt a `ppp` 1,5 (58,0 %).**

Este documento es la entrada de la planificación de los hitos siguientes. La planificación lee además las secciones del manual que cada hito necesite, de una en una, y produce el mapa de funciones con su hito, sus dependencias y su riesgo.

**Nota de I.3 sobre la excepción transitoria:** el recuadro fijo de opciones (138 u) sube el contenido de la caja de 193 a 335 u de los 351 u del panel mínimo: los mínimos de ventana no cambian (387 u de alto y 468 u de ancho). I.4 reevaluó el recuadro fijo: se queda en 138 u (el contenido de la caja pasa a 250 u de 305 u; ver `specs/interfaz-iconos.md`, decisión 6).

## Decisiones del 21 de octubre de 2026

Dos decisiones de estilo y de herramientas, solo anotadas: no se implementan en I.3.

- **(A) Estilo de los iconos de I.4.** Iconos de 1 bit, con un trazo oscuro y grueso, sobre un botón cuadrado con relieve que se ve hundido cuando la herramienta está activa. Van dentro de la interfaz oscura actual y se dibujan nuevos, con el propio Pixoko.
- **(B) Pincel en el hito 4.** Un único icono de pincel que muestra el pincel activo. El clic derecho sobre él abre una ventana con la rejilla de formas y un botón de preajustes; no es este panel de opciones.

## Para más adelante

- **Soltar ficheros no funciona en Linux con Wayland** (el cursor muestra «prohibido»): en winit 0.30.13 solo el backend X11 (`platform_impl/linux/x11/event_processor.rs`) maneja `DroppedFile` y `HoveredFile`; el de Wayland no lo implementa. winit llega como dependencia transitiva de eframe y no se sustituye. Caminos que funcionan: `Ctrl`+`O`, pasar el fichero como argumento (o «Abrir con» del explorador) y lanzar con X11: `env -u WAYLAND_DISPLAY px-app`. No se promete que una versión futura lo resuelva.

### Límite conocido del `.aseprite` (22 de octubre de 2026)

**El RGB bajo alfa 0 fuera del rectángulo mínimo no se guarda; no afecta a nada visible.** Pixoko escribe de cada celda solo el rectángulo mínimo que contiene los píxeles no transparentes (`specs/hito-3-aseprite.md`, decisión 9); un píxel con alfa 0 fuera de él vuelve de la lectura como `(0, 0, 0, 0)`. Medido con los 29 casos del corpus: 4 casos y 133 píxeles, todos con alfa 0 en la composición (`specs/hito-3-puerta.md`). Los píxeles con alfa mayor que 0 se conservan siempre.
