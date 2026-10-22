-- Sonda propia de «Combinar abajo» y «Aplanar» (specs/hito-3-combinar.md, «Lo comprobado» 3).
-- NO es un caso del corpus (no genera ficheros en golden/aseprite/): es la herramienta con la que
-- se observó, como caja negra, qué hace Aseprite, y justifica la regla de combinar abajo de la
-- unidad 3.5. Se ejecuta a mano:
--   mkdir -p ~/.cache/pixoko-sonda
--   aseprite -b --script-param salida="$HOME/.cache/pixoko-sonda" --script tools/corpus-h3/merge-probe.lua
-- (los --script-param van antes de --script; ver specs/corpus-hito-3.md, sección 5). Escribe
-- sonda-combinar.txt con las capas antes y después de cada caso. Borra la carpeta al terminar.
--
-- Observado con Aseprite 1.3.18.6-dev el 2026-10-22 (comandos MergeDownLayer y FlattenLayers):
--   * Combinar abajo deja la capa de abajo (mismo nombre, visibilidad y editable) con opacidad 255
--     y mezcla Normal: su opacidad y su mezcla se hornean en los píxeles. El píxel nuevo es la
--     composición de las dos capas sobre un lienzo vacío, cada una con su opacidad y su modo. La
--     capa activa pasa a ser la de abajo. Fusiona aunque estén ocultas o bloqueadas.
--   * Ejemplo (caso "c1"): abajo opacidad 128 Normal con (200,100,50,255), (200,100,50,128),
--     (0,0,0,0), (10,20,30,255); arriba Multiplicar opacidad 200 con (100,200,255,255) x3 y
--     (100,200,255,100) da (99,113,104,228), (99,148,165,214), (100,200,255,200), (27,55,74,167).
--     Pixoko (composite_pixel de abajo a arriba) da lo mismo salvo ±1 en 5 canales de 3 píxeles,
--     todos dentro del dominio empírico de la unidad 3.2.
--   * FlattenLayers, llamado por script, incluyó una capa oculta (su exportación a PNG, la del
--     corpus, no). Pixoko aplana lo que se ve.

local salida = app.params["salida"]
if not salida then
  error("falta --script-param salida=<carpeta>")
end

local function px(r, g, b, a) return app.pixelColor.rgba(r, g, b, a) end

local function pon_celda(sprite, capa, colores)
  local img = Image(#colores, 1, ColorMode.RGB)
  for x, c in ipairs(colores) do img:putPixel(x - 1, 0, c) end
  sprite:newCel(capa, 1, img, Point(0, 0))
end

local function vuelca(f, sprite, etiqueta)
  f:write("-- " .. etiqueta .. "\n")
  for i, l in ipairs(sprite.layers) do
    f:write(string.format("capa %d nombre=%s visible=%s opacidad=%d mezcla=%d editable=%s\n",
      i, l.name, tostring(l.isVisible), l.opacity, l.blendMode, tostring(l.isEditable)))
    local celda = l:cel(1)
    if celda then
      local fila = {}
      for x = 0, celda.image.width - 1 do
        local v = celda.image:getPixel(x, 0)
        fila[#fila + 1] = string.format("(%d,%d,%d,%d)", app.pixelColor.rgbaR(v),
          app.pixelColor.rgbaG(v), app.pixelColor.rgbaB(v), app.pixelColor.rgbaA(v))
      end
      f:write("  " .. table.concat(fila, " ") .. "\n")
    else
      f:write("  sin celda\n")
    end
  end
  f:write("capa activa=" .. (app.activeLayer and app.activeLayer.name or "ninguna") .. "\n")
end

local f = io.open(salida .. "/sonda-combinar.txt", "w")

-- Pares: abajo "L" y arriba "U"; `ajusta` cambia sus propiedades.
local function combinar_abajo(etiqueta, abajo, arriba, ajusta)
  local s = Sprite(#abajo, 1, ColorMode.RGB)
  app.activeSprite = s
  local l = s.layers[1]
  l.name = "L"
  pon_celda(s, l, abajo)
  local u = s:newLayer()
  u.name = "U"
  pon_celda(s, u, arriba)
  ajusta(l, u)
  vuelca(f, s, etiqueta .. " antes")
  app.activeLayer = u
  app.command.MergeDownLayer()
  vuelca(f, s, etiqueta .. " después")
end

local nada = function() end
local u4 = { px(100, 200, 255, 255), px(100, 200, 255, 255), px(100, 200, 255, 255), px(100, 200, 255, 100) }
local l4 = { px(200, 100, 50, 255), px(200, 100, 50, 128), px(0, 0, 0, 0), px(10, 20, 30, 255) }
combinar_abajo("c1", l4, u4, function(l, u)
  l.opacity = 128
  u.blendMode = BlendMode.MULTIPLY
  u.opacity = 200
end)

local l2 = { px(200, 100, 50, 255), px(200, 100, 50, 100) }
local u2 = { px(100, 200, 255, 255), px(100, 200, 255, 150) }
combinar_abajo("L-multiplicar-255", l2, u2, function(l) l.blendMode = BlendMode.MULTIPLY end)
combinar_abajo("L-screen-128", l2, u2, function(l)
  l.blendMode = BlendMode.SCREEN
  l.opacity = 128
end)
combinar_abajo("L-oculta", l2, u2, function(l) l.isVisible = false end)
combinar_abajo("U-oculta", l2, u2, function(_, u) u.isVisible = false end)
combinar_abajo("L-bloqueada", l2, u2, function(l) l.isEditable = false end)
combinar_abajo("U-bloqueada", l2, u2, function(_, u) u.isEditable = false end)
combinar_abajo("U-opacidad-100-multiplicar", l2, u2, function(_, u)
  u.opacity = 100
  u.blendMode = BlendMode.MULTIPLY
end)

-- Aplanar con una capa oculta, una bloqueada con opacidad y una con mezcla.
do
  local s = Sprite(4, 1, ColorMode.RGB)
  app.activeSprite = s
  local a = s.layers[1]
  a.name = "a"
  pon_celda(s, a, { px(255, 0, 0, 255), px(255, 0, 0, 255), px(255, 0, 0, 255), px(255, 0, 0, 255) })
  local b = s:newLayer()
  b.name = "b-oculta"
  b.isVisible = false
  pon_celda(s, b, { px(0, 255, 0, 255), px(0, 255, 0, 255), px(0, 255, 0, 255), px(0, 255, 0, 255) })
  local c = s:newLayer()
  c.name = "c-bloqueada"
  c.isEditable = false
  c.opacity = 128
  pon_celda(s, c, { px(0, 0, 255, 255), px(0, 0, 255, 255), px(0, 0, 0, 0), px(0, 0, 0, 0) })
  local d = s:newLayer()
  d.name = "d"
  d.blendMode = BlendMode.SCREEN
  pon_celda(s, d, { px(0, 0, 0, 0), px(50, 50, 50, 255), px(50, 50, 50, 255), px(50, 50, 50, 255) })
  vuelca(f, s, "aplanar antes")
  app.command.FlattenLayers()
  vuelca(f, s, "aplanar después")
end

f:close()
