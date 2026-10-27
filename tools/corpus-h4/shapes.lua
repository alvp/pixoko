-- Corpus de las formas (specs/hito-4-formas.md, «Referencia»): dibuja con Aseprite los mismos casos
-- que las filas 2 a 4 de la spec en un lienzo RGBA de 64 × 64 transparente, en rojo y con un pincel
-- de 1 píxel, y escribe un PNG por caso en <salida>:
--   rect-outline.png, rect-filled.png
--   circle-r<R>-outline.png, circle-r<R>-filled.png          R = 1, 2, 3, 5, 10 (centro 32,32)
--   ellipse-<A>x<B>-outline.png, ellipse-<A>x<B>-filled.png  (A,B) = (3,2), (5,1), (2,4), (12,7), (20,9)
--   poly-<nombre>-outline.png, poly-<nombre>-filled.png      triangle, notch, bowtie, star
--
-- Lo lanza generate-shapes.sh (no a mano):
--   aseprite -b --script-param salida=<carpeta> --script tools/corpus-h4/shapes.lua
--
-- Todo es propio y se crea desde cero. Licencia MIT, como el resto del repositorio. SIN EJECUTAR:
-- las dudas sobre la API están marcadas con "comprobar" y se arreglan en un solo sitio, la función
-- `dibuja`. La elipse de Aseprite se define por su caja: el centro (32,32) y los radios (A,B) son la
-- caja de (32-A, 32-B) a (32+A, 32+B).

local salida = app.params["salida"] -- comprobar: app.params y --script-param
if not salida then
  error("falta --script-param salida=<carpeta>")
end

local ROJO = Color { r = 255, g = 0, b = 0, a = 255 } -- comprobar: Color{...}

-- Dibuja una forma en un sprite nuevo y lo guarda. `herramienta` es el nombre de la herramienta de
-- Aseprite y `puntos` la lista de {x, y}.
local function dibuja(nombre, herramienta, puntos)
  local sprite = Sprite(64, 64, ColorMode.RGB) -- comprobar: la capa inicial es transparente
  local lista = {}
  for _, p in ipairs(puntos) do
    lista[#lista + 1] = Point(p[1], p[2])
  end
  app.useTool { -- comprobar: app.useTool y sus campos
    tool = herramienta,
    color = ROJO,
    brush = Brush(1),
    points = lista,
  }
  sprite:saveCopyAs(salida .. "/" .. nombre .. ".png") -- comprobar: saveCopyAs
  sprite:close()
end

-- Rectángulo (5,5)-(20,12).
dibuja("rect-outline", "rectangle", { { 5, 5 }, { 20, 12 } })
dibuja("rect-filled", "filled_rectangle", { { 5, 5 }, { 20, 12 } })

-- Círculos y elipses: la caja del centro (32,32) y los radios.
local function elipse(nombre, a, b)
  local caja = { { 32 - a, 32 - b }, { 32 + a, 32 + b } }
  dibuja(nombre .. "-outline", "ellipse", caja)
  dibuja(nombre .. "-filled", "filled_ellipse", caja)
end
for _, r in ipairs({ 1, 2, 3, 5, 10 }) do
  elipse("circle-r" .. r, r, r)
end
for _, ab in ipairs({ { 3, 2 }, { 5, 1 }, { 2, 4 }, { 12, 7 }, { 20, 9 } }) do
  elipse("ellipse-" .. ab[1] .. "x" .. ab[2], ab[1], ab[2])
end

-- Polígonos. Aseprite no tiene opción `fill` en la herramienta "polygon" ni una "filled_polygon"
-- (las dos se probaron y las rechazó): el contorno es "polygon" y el relleno, "contour".
local POLIGONOS = {
  { "triangle", { { 5, 5 }, { 40, 5 }, { 5, 40 } } },
  { "notch", { { 5, 5 }, { 20, 5 }, { 20, 30 }, { 35, 30 }, { 35, 5 }, { 50, 5 }, { 50, 50 }, { 5, 50 } } },
  { "bowtie", { { 5, 5 }, { 50, 5 }, { 5, 50 }, { 50, 50 } } },
  { "star", { { 32, 3 }, { 50, 58 }, { 3, 22 }, { 61, 22 }, { 14, 58 } } },
}
for _, poligono in ipairs(POLIGONOS) do
  dibuja("poly-" .. poligono[1] .. "-outline", "polygon", poligono[2])
  -- No hay "filled_polygon" (comprobado con Aseprite): el relleno sale de la herramienta "contour"
  -- (a mano alzada, rellena siempre) con los vértices y el punto de cierre como trazo.
  local trazo = {}
  for _, p in ipairs(poligono[2]) do
    trazo[#trazo + 1] = p
  end
  trazo[#trazo + 1] = poligono[2][1]
  dibuja("poly-" .. poligono[1] .. "-filled", "contour", trazo)
end
end
