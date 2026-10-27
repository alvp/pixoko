-- Corpus del cuantizador (specs/corpus-hito-4-cuantizador.md): crea 6 imágenes RGBA propias y, de cada
-- una, la versión indexada que produce el cuantizador de Aseprite con 16 y 64 colores y con tramado
-- ninguno, de difusión y ordenado. Escribe en <salida>:
--   <caso>.src.png                      la imagen RGBA original
--   <caso>-n<N>-<tramado>.png           el resultado indexado (tramado: none, floyd, ordered)
--
-- Lo lanza generate.sh (no a mano):
--   aseprite -b --script-param salida=<carpeta> --script tools/corpus-h4/quantize.lua
--
-- Todo es propio y se crea desde cero, con aritmética de enteros. Licencia MIT, como el resto del
-- repositorio. SIN EJECUTAR: las dudas sobre la API están marcadas con "comprobar" y se arreglan en
-- un solo sitio, la función `cuantiza`.

local salida = app.params["salida"] -- comprobar: app.params y --script-param
if not salida then
  error("falta --script-param salida=<carpeta>")
end

local function rgba(r, g, b, a)
  return app.pixelColor.rgba(r, g, b, a) -- comprobar: app.pixelColor.rgba
end

---------------------------------------------------------------------------------------------
-- Los 6 casos: nombre, ancho, alto y función (x, y) -> r, g, b, a
---------------------------------------------------------------------------------------------

local CASOS = {
  { "degradado", 64, 64, function(x, y)
      return x * 4, y * 4, (x + y) * 2, 255
  end },
  { "anillos", 64, 64, function(x, y)
      local d = (x - 32) * (x - 32) + (y - 32) * (y - 32)
      return (d * 3) % 256, (d * 5) % 256, (255 - d // 8) % 256, 255
  end },
  { "manchas", 60, 20, function(x, y)
      local base = { { 200, 30, 30 }, { 30, 200, 30 }, { 30, 30, 200 } }
      local b = base[x // 20 + 1]
      local j = (x * 7 + y * 13) % 5
      return b[1] + j, b[2] + j, b[3] + j, 255
  end },
  { "alfa", 64, 16, function(x, y)
      return (x * 4) % 256, 255 - (y * 16) % 256, 128, x * 4
  end },
  { "grises", 64, 16, function(x, y)
      local v = x * 4
      return v, v, v, 255
  end },
  { "mixto", 64, 64, function(x, y)
      local d = (x - 20) * (x - 20) + (y - 40) * (y - 40)
      if d < 144 then return 250, 220 - d, 30, 255 end
      if (x + y) % 16 < 3 then return 20, 20, 20 + x * 3, 255 end
      return (y * 4) % 256, (x * 3) % 256, 140 + (x * y) % 100, 255
  end },
}

local TRAMADOS = {
  { "none", "none" },
  { "floyd", "floyd-steinberg" }, -- comprobar el nombre en ChangePixelFormat
  { "ordered", "ordered" },
}

local COLORES = { 16, 64 }

---------------------------------------------------------------------------------------------
-- Dibujo y exportación
---------------------------------------------------------------------------------------------

local function crea(ancho, alto, f)
  local spr = Sprite(ancho, alto, ColorMode.RGB) -- comprobar: Sprite(w, h, colorMode)
  local img = Image(ancho, alto, ColorMode.RGB)
  for y = 0, alto - 1 do
    for x = 0, ancho - 1 do
      local r, g, b, a = f(x, y)
      img:putPixel(x, y, rgba(r, g, b, a))
    end
  end
  spr.cels[1].image = img -- comprobar: asignar la imagen de la celda
  return spr
end

-- El único sitio que habla con el cuantizador: crea la paleta de `n` colores a partir de la imagen
-- y pasa a indexado con el tramado pedido.
local function cuantiza(spr, n, tramado)
  app.command.ColorQuantization { -- comprobar: nombre y parámetros
    ui = false,
    withAlpha = true,
    maxColors = n,
    useRange = false,
  }
  app.command.ChangePixelFormat { -- comprobar: nombre y parámetros
    format = "indexed",
    dithering = tramado,
    rgbmap = "octree",
    toGray = "default",
  }
end

for _, caso in ipairs(CASOS) do
  local nombre, ancho, alto, f = caso[1], caso[2], caso[3], caso[4]
  local base = crea(ancho, alto, f)
  base:saveCopyAs(salida .. "/" .. nombre .. ".src.png")
  base:close()
  for _, n in ipairs(COLORES) do
    for _, t in ipairs(TRAMADOS) do
      local spr = crea(ancho, alto, f)
      app.activeSprite = spr
      cuantiza(spr, n, t[2])
      spr:saveCopyAs(salida .. "/" .. nombre .. "-n" .. n .. "-" .. t[1] .. ".png")
      spr:close()
    end
  end
end
