-- Corpus del hito 3 (specs/corpus-hito-3.md): genera UN caso con la API de Lua de Aseprite.
--
-- Lo lanza generar.sh, una vez por caso:
--   aseprite -b --script-param caso=<nombre> --script-param salida=<carpeta> \
--     --script tools/corpus-h3/generar.lua
-- (los --script-param van antes de --script)
--
-- Todo es propio y se crea desde cero: los colores y las formas se definen aquí (rectángulos,
-- discos y diagonales por aritmética de enteros, sin suavizado), cada sprite lleva una paleta
-- propia y no se usa la paleta por defecto ni ningún recurso del programa. Licencia MIT, como el
-- resto del repositorio.
--
-- Las dudas sobre la API están marcadas con "comprobar": se escribió sin ejecutar nada. Todo el
-- dibujo pasa por `pon`, `imagen`, `celda` y `capa`, así que una duda se arregla en un solo sitio.

local caso = app.params["caso"] -- comprobar: app.params y --script-param
local salida = app.params["salida"]
if not caso or not salida then
  error("faltan --script-param caso=<nombre> y --script-param salida=<carpeta>")
end

---------------------------------------------------------------------------------------------
-- Colores propios. El orden de NOMBRES es el de la paleta de 16 entradas (índice = posición).
---------------------------------------------------------------------------------------------

local NOMBRES = {
  "negro", "blanco", "rojo", "verde", "azul", "amarillo", "cian", "magenta",
  "naranja", "gris-claro", "gris", "gris-oscuro", "marron", "rosa", "marino", "lima",
}
local RGB = {
  ["negro"] = { 0, 0, 0 }, ["blanco"] = { 255, 255, 255 }, ["rojo"] = { 196, 46, 52 },
  ["verde"] = { 58, 168, 76 }, ["azul"] = { 44, 92, 200 }, ["amarillo"] = { 232, 198, 52 },
  ["cian"] = { 66, 196, 206 }, ["magenta"] = { 186, 64, 170 }, ["naranja"] = { 236, 132, 44 },
  ["gris-claro"] = { 190, 190, 190 }, ["gris"] = { 128, 128, 128 },
  ["gris-oscuro"] = { 64, 64, 64 }, ["marron"] = { 120, 76, 40 }, ["rosa"] = { 250, 170, 190 },
  ["marino"] = { 24, 60, 100 }, ["lima"] = { 160, 220, 120 },
}
local PALETA = {}
local IDX = {} -- nombre -> índice de paleta (base 0)
for i, nombre in ipairs(NOMBRES) do
  PALETA[i] = RGB[nombre]
  IDX[nombre] = i - 1
end

-- Ocho colores de fondo (A) y ocho de origen (B) para las matrices de mezcla. Incluyen blanco y
-- negro (identidades) y valores sin patrón (37, 201, 143, 211, 17, 99) para que el redondeo se vea.
local A = {
  { 255, 255, 255 }, { 0, 0, 0 }, { 196, 46, 52 }, { 58, 168, 76 },
  { 44, 92, 200 }, { 232, 198, 52 }, { 128, 128, 128 }, { 37, 201, 143 },
}
local B = {
  { 255, 255, 255 }, { 0, 0, 0 }, { 90, 60, 220 }, { 220, 120, 30 },
  { 20, 200, 200 }, { 180, 180, 60 }, { 100, 100, 100 }, { 211, 17, 99 },
}

-- Los ocho modos de D07, con el nombre del enum de Aseprite.
local MODOS = {
  { "normal", BlendMode.NORMAL }, -- comprobar: nombres del enum BlendMode
  { "multiplicar", BlendMode.MULTIPLY },
  { "trama", BlendMode.SCREEN },
  { "superponer", BlendMode.OVERLAY },
  { "oscurecer", BlendMode.DARKEN },
  { "aclarar", BlendMode.LIGHTEN },
  { "sumar", BlendMode.ADDITION },
  { "diferencia", BlendMode.DIFFERENCE },
}

---------------------------------------------------------------------------------------------
-- Ayudantes. Un caso por ejecución: `primera_usada` sigue a la capa que trae el sprite nuevo.
---------------------------------------------------------------------------------------------

local primera_usada = false

-- Valor de píxel RGBA como entero.
local function rgb(color, alfa)
  return app.pixelColor.rgba(color[1], color[2], color[3], alfa or 255)
end

local function col(nombre, alfa)
  return rgb(RGB[nombre], alfa)
end

-- Sprite nuevo con paleta propia (lista de {r, g, b[, a]}). Nunca se usa la paleta por defecto.
local function nuevo_sprite(ancho, alto, indexado, paleta)
  local s = Sprite(ancho, alto, indexado and ColorMode.INDEXED or ColorMode.RGB)
  -- comprobar: Sprite(ancho, alto, modo); si no, Sprite{ width=, height=, colorMode= }
  local pal = Palette(#paleta) -- comprobar: constructor Palette(n)
  for i, c in ipairs(paleta) do
    pal:setColor(i - 1, Color{ r = c[1], g = c[2], b = c[3], a = c[4] or 255 })
  end
  s:setPalette(pal) -- comprobar: Sprite:setPalette
  return s
end

-- Capa nueva (encima de las anteriores). La primera es la que trae el sprite.
local function capa(s, nombre, props)
  props = props or {}
  local l
  if not primera_usada then
    l = s.layers[1]
    primera_usada = true
  else
    l = s:newLayer()
  end
  l.name = nombre
  l.opacity = props.opacidad or 255 -- comprobar: Layer.opacity (0 a 255)
  l.blendMode = props.mezcla or BlendMode.NORMAL
  l.isVisible = props.visible ~= false
  return l
end

-- Imagen vacía (transparente) o rellena de `relleno` (valor de píxel o índice).
local function imagen(s, ancho, alto, relleno)
  local img = Image(ancho, alto, s.colorMode) -- comprobar: Image(ancho, alto, modo)
  if relleno ~= nil then
    for y = 0, alto - 1 do
      for x = 0, ancho - 1 do
        img:putPixel(x, y, relleno)
      end
    end
  end
  return img
end

-- Único punto donde se escribe un píxel.
local function pon(img, x, y, valor)
  if x >= 0 and y >= 0 and x < img.width and y < img.height then
    img:putPixel(x, y, valor) -- comprobar: putPixel(x, y, entero de píxel o índice)
  end
end

-- Celda de la capa `l` en el único fotograma, con la imagen en (x, y) del lienzo.
local function celda(s, l, img, x, y)
  s:newCel(l, 1, img, Point(x or 0, y or 0)) -- comprobar: reemplaza una celda existente
end

local function quitar_celda(s, l)
  local c = l:cel(1)
  if c then
    s:deleteCel(c) -- comprobar: Sprite:deleteCel(cel)
  end
end

local function rect(img, x0, y0, x1, y1, valor)
  for y = y0, y1 do
    for x = x0, x1 do
      pon(img, x, y, valor)
    end
  end
end

local function disco(img, cx, cy, r, valor)
  for y = cy - r, cy + r do
    for x = cx - r, cx + r do
      if (x - cx) * (x - cx) + (y - cy) * (y - cy) <= r * r then
        pon(img, x, y, valor)
      end
    end
  end
end

-- Triángulo rectángulo con el ángulo recto en (x0, y1): la fila y llena x0..x0 + (y - y0).
local function triangulo(img, x0, y0, y1, valor)
  for y = y0, y1 do
    for x = x0, x0 + (y - y0) do
      pon(img, x, y, valor)
    end
  end
end

local function diagonal(img, x0, y0, largo, valor, grueso)
  for i = 0, largo - 1 do
    for g = 0, (grueso or 1) - 1 do
      pon(img, x0 + i + g, y0 + i, valor)
    end
  end
end

local function guardar(s, nombre)
  s:saveAs(salida .. "/" .. nombre .. ".aseprite") -- comprobar: guarda en formato .aseprite
  print("ok " .. nombre)
end

---------------------------------------------------------------------------------------------
-- Casos
---------------------------------------------------------------------------------------------

local CASOS = {}

CASOS["rgba-una-capa"] = function()
  local s = nuevo_sprite(16, 16, false, PALETA)
  local l = capa(s, "unica")
  local img = imagen(s, 16, 16)
  disco(img, 10, 7, 5, col("rojo"))
  rect(img, 1, 8, 5, 12, col("azul", 128))
  rect(img, 1, 1, 4, 4, col("verde", 64))
  -- RGB distinto de cero bajo alfa 0: lo que haga el programa con él se anota, no se exige.
  pon(img, 0, 0, col("amarillo", 0))
  pon(img, 1, 0, col("amarillo", 0))
  for x = 0, 15 do
    pon(img, x, 15, col("cian", x * 17))
  end
  celda(s, l, img)
  guardar(s, "rgba-una-capa")
end

CASOS["capas-normal-tres"] = function()
  local s = nuevo_sprite(24, 24, false, PALETA)
  local fondo = capa(s, "fondo")
  local medio = capa(s, "medio")
  local arriba = capa(s, "arriba")

  local a = imagen(s, 24, 24, col("marino"))
  rect(a, 2, 15, 8, 21, col("lima"))
  celda(s, fondo, a)

  local b = imagen(s, 24, 24)
  disco(b, 12, 12, 8, col("naranja"))
  disco(b, 12, 12, 3, col("amarillo"))
  celda(s, medio, b)

  local c = imagen(s, 24, 24)
  triangulo(c, 0, 0, 11, col("magenta"))
  diagonal(c, 10, 10, 13, col("rosa"), 2)
  celda(s, arriba, c)
  guardar(s, "capas-normal-tres")
end

CASOS["capas-opacidad"] = function()
  local s = nuevo_sprite(32, 16, false, PALETA)
  local base = capa(s, "fondo")
  local img = imagen(s, 32, 16)
  for y = 0, 15 do
    for x = 0, 31 do
      pon(img, x, y, ((x // 4) + (y // 4)) % 2 == 0 and col("blanco") or col("negro"))
    end
  end
  celda(s, base, img)
  local opacidades = { 0, 64, 128, 200, 255 }
  for k, op in ipairs(opacidades) do
    local l = capa(s, "op-" .. op, { opacidad = op })
    local t = imagen(s, 32, 16)
    local x0 = 1 + 6 * (k - 1)
    rect(t, x0, 2, x0 + 4, 13, col("rojo"))
    celda(s, l, t)
  end
  guardar(s, "capas-opacidad")
end

CASOS["capas-alfa-origen"] = function()
  local s = nuevo_sprite(32, 16, false, PALETA)
  local base = capa(s, "fondo")
  local fondo = imagen(s, 32, 16)
  local bandas = { "blanco", "gris", "azul", "negro" }
  for k, nombre in ipairs(bandas) do
    rect(fondo, 8 * (k - 1), 0, 8 * k - 1, 15, col(nombre))
  end
  celda(s, base, fondo)
  local arriba = capa(s, "origen")
  local img = imagen(s, 32, 16)
  local alfas = { 0, 36, 73, 109, 146, 182, 219, 255 }
  for k, a in ipairs(alfas) do
    local x0 = 4 * (k - 1)
    rect(img, x0, 0, x0 + 3, 7, col("naranja", a))
    rect(img, x0, 8, x0 + 3, 15, col("cian", a))
  end
  celda(s, arriba, img)
  guardar(s, "capas-alfa-origen")
end

CASOS["capas-alfa-fondo"] = function()
  local s = nuevo_sprite(32, 16, false, PALETA)
  local base = capa(s, "base")
  local alfas = { 255, 200, 128, 64 }
  local a = imagen(s, 32, 16)
  for k, al in ipairs(alfas) do
    rect(a, 8 * (k - 1), 0, 8 * k - 1, 15, col("verde", al))
  end
  celda(s, base, a)
  local arriba = capa(s, "origen")
  local b = imagen(s, 32, 16)
  for k, al in ipairs(alfas) do
    rect(b, 0, 4 * (k - 1), 31, 4 * k - 1, col("magenta", al))
  end
  celda(s, arriba, b)
  guardar(s, "capas-alfa-fondo")
end

-- Matriz 8 x 8: columnas de 4 px con los colores A (fondo), filas de 4 px con los B (origen).
local function matriz(nombre_modo, modo)
  CASOS["mezcla-" .. nombre_modo] = function()
    local s = nuevo_sprite(32, 32, false, PALETA)
    local base = capa(s, "base")
    local fuente = capa(s, "fuente", { mezcla = modo })
    local a = imagen(s, 32, 32)
    local b = imagen(s, 32, 32)
    for y = 0, 31 do
      for x = 0, 31 do
        pon(a, x, y, rgb(A[x // 4 + 1]))
        pon(b, x, y, rgb(B[y // 4 + 1]))
      end
    end
    celda(s, base, a)
    celda(s, fuente, b)
    guardar(s, "mezcla-" .. nombre_modo)
  end
end
for _, m in ipairs(MODOS) do
  matriz(m[1], m[2])
end

-- Un sprite de 64 x 16: base con los colores A por columna (x mod 8) y ocho capas, una por modo,
-- cada una en su tira de 8 columnas; el origen cambia de color por fila.
local function ocho_modos(nombre, opacidad, alfa_alto, alfa_bajo, alfa_base)
  CASOS[nombre] = function()
    local s = nuevo_sprite(64, 16, false, PALETA)
    local base = capa(s, "base")
    local a = imagen(s, 64, 16)
    for y = 0, 15 do
      for x = 0, 63 do
        pon(a, x, y, rgb(A[x % 8 + 1], alfa_base))
      end
    end
    celda(s, base, a)
    for k, m in ipairs(MODOS) do
      local l = capa(s, "m-" .. m[1], { mezcla = m[2], opacidad = opacidad })
      local img = imagen(s, 64, 16)
      for y = 0, 15 do
        local color = B[(y + (y >= 8 and 3 or 0)) % 8 + 1]
        local alfa = (y < 8) and alfa_alto or alfa_bajo
        for x = 8 * (k - 1), 8 * k - 1 do
          pon(img, x, y, rgb(color, alfa))
        end
      end
      celda(s, l, img)
    end
    guardar(s, nombre)
  end
end
ocho_modos("mezcla-ocho-opacidad", 128, 255, 255, 255)
ocho_modos("mezcla-ocho-alfa", 255, 200, 77, 255)
ocho_modos("mezcla-ocho-fondo-translucido", 255, 255, 100, 150)

-- Rejilla de alfas (ampliación del corpus, specs/corpus-hito-3.md «Casos añadidos»): 64 x 32, dos
-- rejillas de 8 x 8 celdas de 3 px. Columnas = alfa del fondo, filas = alfa de la fuente, los dos
-- con los mismos valores (el 127 al final, fuera de orden, a propósito). La base es opaca fuera de
-- las rejillas (gris 90) y translúcida dentro; la capa `fuente` solo existe dentro. Dos pares de
-- colores con canales distintos; el segundo cambia fuente y fondo de sitio.
local ALFAS_REJILLA = { 0, 40, 80, 120, 160, 200, 255, 127 }
local PARES_REJILLA = {
  { x0 = 4, fuente = { 200, 60, 20 }, fondo = { 30, 120, 220 } },
  { x0 = 36, fuente = { 30, 120, 220 }, fondo = { 200, 60, 20 } },
}
local function rejilla(nombre, modo, opacidad)
  CASOS[nombre] = function()
    local s = nuevo_sprite(64, 32, false, PALETA)
    local base = capa(s, "base")
    local fuente = capa(s, "fuente", { mezcla = modo, opacidad = opacidad })
    local a = imagen(s, 64, 32, rgb({ 90, 90, 90 }))
    local b = imagen(s, 64, 32)
    for _, p in ipairs(PARES_REJILLA) do
      for i, alfa_fondo in ipairs(ALFAS_REJILLA) do
        for j, alfa_fuente in ipairs(ALFAS_REJILLA) do
          local x0 = p.x0 + 3 * (i - 1)
          local y0 = 4 + 3 * (j - 1)
          rect(a, x0, y0, x0 + 2, y0 + 2, rgb(p.fondo, alfa_fondo))
          rect(b, x0, y0, x0 + 2, y0 + 2, rgb(p.fuente, alfa_fuente))
        end
      end
    end
    celda(s, base, a)
    celda(s, fuente, b)
    guardar(s, nombre)
  end
end
rejilla("mezcla-alfa-rejilla-multiplicar", BlendMode.MULTIPLY, 255)
rejilla("mezcla-alfa-rejilla-superponer", BlendMode.OVERLAY, 128)

-- La capa de abajo con opacidad 128 y modo Multiplicar sobre el lienzo transparente (no hay nada
-- que mezclar debajo) y una capa Normal encima. Alfas de la base 255, 223, 191, 159, 127, 95, 63 y
-- 0 por filas de 4 px (con 128 la parte fraccionaria de alfa x 128 / 255 es mayor que 0,5 en seis
-- de las ocho) y los colores A por columnas de 4 px; la fila de alfa 0 lleva RGB distinto de 0.
CASOS["fondo-con-opacidad"] = function()
  local s = nuevo_sprite(32, 32, false, PALETA)
  local base = capa(s, "base", { mezcla = BlendMode.MULTIPLY, opacidad = 128 })
  local arriba = capa(s, "arriba")
  local alfas = { 255, 223, 191, 159, 127, 95, 63, 0 }
  local a = imagen(s, 32, 32)
  for y = 0, 31 do
    for x = 0, 31 do
      pon(a, x, y, rgb(A[x // 4 + 1], alfas[y // 4 + 1]))
    end
  end
  celda(s, base, a)
  local b = imagen(s, 32, 32)
  disco(b, 16, 16, 9, col("naranja"))
  rect(b, 2, 2, 13, 13, col("cian", 128))
  celda(s, arriba, b)
  guardar(s, "fondo-con-opacidad")
end

CASOS["indexado-capas"] = function()
  local s = nuevo_sprite(16, 16, true, PALETA)
  s.transparentColor = 0 -- comprobar: Sprite.transparentColor
  local fondo = capa(s, "fondo")
  local oculta = capa(s, "oculta", { visible = false })
  local medio = capa(s, "medio")
  local arriba = capa(s, "arriba")

  local a = imagen(s, 16, 16, IDX["gris"])
  rect(a, 3, 3, 12, 12, IDX["gris-oscuro"])
  celda(s, fondo, a)

  celda(s, oculta, imagen(s, 16, 16, IDX["magenta"]))

  local b = imagen(s, 16, 16, 0)
  disco(b, 8, 8, 5, IDX["rojo"])
  disco(b, 8, 8, 2, IDX["amarillo"])
  celda(s, medio, b)

  local c = imagen(s, 16, 16, 0)
  diagonal(c, 0, 0, 16, IDX["cian"], 2)
  celda(s, arriba, c)
  guardar(s, "indexado-capas")
end

CASOS["indexado-transparente-7"] = function()
  local s = nuevo_sprite(16, 16, true, PALETA)
  s.transparentColor = 7
  local fondo = capa(s, "fondo")
  local arriba = capa(s, "arriba")

  local a = imagen(s, 16, 16, 0) -- el índice 0 (negro) es opaco aquí
  rect(a, 4, 4, 11, 11, IDX["azul"])
  celda(s, fondo, a)

  local b = imagen(s, 16, 16, 7) -- el 7 es el transparente
  rect(b, 2, 2, 13, 13, IDX["verde"])
  rect(b, 6, 6, 9, 9, 7)
  rect(b, 0, 14, 15, 15, 0)
  celda(s, arriba, b)
  guardar(s, "indexado-transparente-7")
end

CASOS["indexado-paleta-256"] = function()
  local paleta = {}
  for i = 0, 255 do
    paleta[i + 1] = { (i * 53 + 17) % 256, (i * 97 + 43) % 256, (i * 151 + 89) % 256 }
  end
  local s = nuevo_sprite(64, 64, true, paleta)
  s.transparentColor = 0
  local l = capa(s, "unica")
  local img = imagen(s, 64, 64, 0)
  for by = 0, 15 do
    for bx = 0, 15 do
      rect(img, bx * 4, by * 4, bx * 4 + 3, by * 4 + 3, by * 16 + bx)
    end
  end
  celda(s, l, img)
  guardar(s, "indexado-paleta-256")
end

CASOS["indexado-celdas"] = function()
  local s = nuevo_sprite(32, 32, true, PALETA)
  s.transparentColor = 0
  local la = capa(s, "desplazada")
  local lb = capa(s, "recorte-izq-arriba")
  local lc = capa(s, "recorte-der-abajo")

  local a = imagen(s, 12, 12, 0)
  disco(a, 6, 6, 5, IDX["rojo"])
  rect(a, 0, 0, 2, 2, IDX["amarillo"])
  celda(s, la, a, 5, 7)

  local b = imagen(s, 14, 14, IDX["verde"])
  disco(b, 7, 7, 4, IDX["blanco"])
  celda(s, lb, b, -6, -4)

  local c = imagen(s, 10, 10, IDX["azul"])
  rect(c, 0, 0, 4, 4, IDX["naranja"])
  celda(s, lc, c, 27, 25)
  guardar(s, "indexado-celdas")
end

CASOS["celda-desplazada"] = function()
  local s = nuevo_sprite(32, 32, false, PALETA)
  local la = capa(s, "a")
  local lb = capa(s, "b")
  local lc = capa(s, "c-borde")
  local ld = capa(s, "d-un-pixel")

  local a = imagen(s, 10, 8, col("rojo"))
  rect(a, 1, 1, 3, 3, col("blanco"))
  celda(s, la, a, 3, 5)

  local b = imagen(s, 12, 12)
  disco(b, 6, 6, 5, col("verde"))
  celda(s, lb, b, 16, 4)

  local c = imagen(s, 8, 8, col("azul"))
  diagonal(c, 0, 0, 8, col("amarillo"))
  celda(s, lc, c, 24, 24)

  celda(s, ld, imagen(s, 1, 1, col("naranja")), 31, 31)
  guardar(s, "celda-desplazada")
end

CASOS["celda-recortada"] = function()
  local s = nuevo_sprite(32, 32, false, PALETA)
  local grande = capa(s, "grande")
  local izq = capa(s, "arriba-izq")
  local der = capa(s, "derecha")
  local aba = capa(s, "abajo")
  local fd = capa(s, "fuera-der")
  local fi = capa(s, "fuera-izq")

  local g = imagen(s, 48, 48, col("gris-claro"))
  diagonal(g, 0, 0, 48, col("marron"), 3)
  celda(s, grande, g, -8, -8)

  local a = imagen(s, 16, 16, col("rojo"))
  disco(a, 8, 8, 6, col("blanco"))
  celda(s, izq, a, -8, -8)

  local b = imagen(s, 16, 16, col("verde"))
  rect(b, 0, 0, 3, 15, col("amarillo"))
  celda(s, der, b, 24, 10)

  celda(s, aba, imagen(s, 20, 10, col("azul")), 6, 26)

  -- Dos celdas del todo fuera del lienzo: al leerlas, la celda queda vacía.
  celda(s, fd, imagen(s, 16, 16, col("magenta")), 40, 2)
  celda(s, fi, imagen(s, 16, 16, col("naranja")), -16, -16)
  guardar(s, "celda-recortada")
end

CASOS["celda-vacia"] = function()
  local s = nuevo_sprite(16, 16, false, PALETA)
  local con = capa(s, "con-celda")
  local sin = capa(s, "sin-celda", { mezcla = BlendMode.MULTIPLY, opacidad = 100 })
  local transp = capa(s, "transparente")

  local a = imagen(s, 16, 16)
  rect(a, 2, 2, 13, 13, col("azul"))
  celda(s, con, a)
  quitar_celda(s, sin) -- la capa existe con sus propiedades pero no tiene celda
  celda(s, transp, imagen(s, 16, 16)) -- celda que existe pero con todo el alfa a 0
  guardar(s, "celda-vacia")
end

CASOS["capas-ocultas"] = function()
  local s = nuevo_sprite(24, 24, false, PALETA)
  local fondo = capa(s, "fondo")
  local o1 = capa(s, "oculta-1", { visible = false })
  local v1 = capa(s, "visible-1")
  local o2 = capa(s, "oculta-2", { visible = false, mezcla = BlendMode.MULTIPLY, opacidad = 128 })
  local v2 = capa(s, "visible-2")

  celda(s, fondo, imagen(s, 24, 24, col("gris-claro")))
  celda(s, o1, imagen(s, 24, 24, col("magenta")))
  local a = imagen(s, 24, 24)
  disco(a, 12, 12, 8, col("naranja"))
  celda(s, v1, a)
  celda(s, o2, imagen(s, 24, 24, col("negro")))
  local b = imagen(s, 24, 24)
  triangulo(b, 2, 2, 12, col("cian"))
  celda(s, v2, b)
  guardar(s, "capas-ocultas")
end

CASOS["mixto-rgba"] = function()
  local s = nuevo_sprite(48, 48, false, PALETA)
  local fondo = capa(s, "fondo")
  local sombra = capa(s, "sombra", { mezcla = BlendMode.MULTIPLY, opacidad = 192 })
  local luz = capa(s, "luz", { mezcla = BlendMode.SCREEN, opacidad = 128 })
  local oculta = capa(s, "oculta", { visible = false })
  local borde = capa(s, "borde")
  local dif = capa(s, "diferencia", { mezcla = BlendMode.DIFFERENCE, opacidad = 90 })

  local f = imagen(s, 48, 48)
  local bandas = { "marino", "gris-oscuro", "gris", "gris-claro" }
  for y = 0, 47 do
    rect(f, 0, y, 47, y, col(bandas[y // 12 + 1]))
  end
  celda(s, fondo, f)

  local a = imagen(s, 24, 24)
  disco(a, 12, 12, 11, col("gris", 220))
  disco(a, 12, 12, 5, col("gris", 110))
  celda(s, sombra, a, 10, 12)

  local b = imagen(s, 30, 30, col("naranja"))
  disco(b, 15, 15, 10, col("amarillo"))
  celda(s, luz, b, -8, 20)

  celda(s, oculta, imagen(s, 48, 48, col("magenta")))

  local c = imagen(s, 48, 48)
  diagonal(c, 0, 0, 47, col("blanco", 128), 2)
  celda(s, borde, c)

  local d = imagen(s, 16, 16, col("cian"))
  disco(d, 8, 8, 5, col("rosa"))
  celda(s, dif, d, 30, 30)
  guardar(s, "mixto-rgba")
end

local generar = CASOS[caso]
if not generar then
  error("caso desconocido: " .. caso)
end
generar()
