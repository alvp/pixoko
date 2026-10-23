# Interfaz · C4: textos del bloque de color (para revisión)

Copia de revisión de los textos de [`ui-color-block.md`](ui-color-block.md), tal como están en el código. Si cambias algo aquí, se pasa al código en el commit siguiente. Límites de siempre: nombre ≤ 20 caracteres, descripción ≤ 8 líneas de 42, todo dentro de `INTERFACE_CHARSET`.

Fuentes: `crates/px-ui/src/color_block_es.rs` (textos cortos) y `crates/px-ui/src/help_texts_color_es.rs`, `help_texts_palette_es.rs` (ayuda `F1`).

## `color_block_es.rs`

| Constante | Texto | Dónde se ve |
| --- | --- | --- |
| `FOREGROUND` | Primer plano | tooltip de la barra de arriba, línea 1 |
| `BACKGROUND` | Fondo | tooltip de la barra de abajo, línea 1 |
| `BAR_TIP` | clic: cuentagotas; clic derecho: editar este color | tooltip de las dos barras, línea 2 |
| `TRANSPARENT` | transparente | dentro de la barra cuando lo pintado es del todo transparente |
| `MODE_RGB`, `MODE_HSL`, `MODE_HSV` | RGB, HSL, HSV | selector de los deslizadores |
| `SWAP_TIP` | Intercambiar primer plano y fondo | tooltip del botón de intercambio |
| `MENU_SWAP` | Intercambiar colores | entrada `Editar → Intercambiar colores` |

Las barras dicen además `Idx-N` (indexadas) o `Col-N` (resto de modos), con N desde 0 (decisión 5 de la spec).

## Ayuda `color.bar` (F1 sobre una barra o el campo hexadecimal)

- Nombre: **Barras de color**
- Descripción: Arriba, el primer plano; debajo, el fondo. Clic: cuentagotas de un solo uso para ese color. Clic derecho: abre el editor con ese color; otro clic derecho en la misma barra lo cierra. El borde grueso marca el color que edita el campo hexadecimal (3, 4, 6 u 8 dígitos; Intro confirma).

## Ayuda `color.editor` (F1 sobre el cuadrado, las barras de tono y alfa, el selector o los deslizadores)

- Nombre: **Editor de color**
- Descripción: Se abre con p o con el clic derecho en una barra de color; se cierra con Esc, con p, con otro clic derecho en esa barra o al cambiar de documento. Arrastra en el cuadrado (saturación a la derecha, brillo arriba) o en las barras de tono y alfa. Con el foco, las flechas mueven 1 y con Mayús 10.

## Ayuda `color.swap` (F1 con el puntero sobre el botón de intercambio)

- Nombre: **Intercambiar colores**
- Descripción: Cambia el primer plano por el fondo y el fondo por el primer plano. No es un paso de deshacer. Está en Editar y en el botón junto a las barras de color. No tiene atajo por defecto: puedes asignarle uno.

## Para decidir

- Si `transparente` debe llevar mayúscula inicial o ir sin ella (hoy, en minúscula, como `Idx-N`).
- Si `color.bar` debe nombrar el botón de intercambio (hoy no; `color.swap` lo explica aparte).
