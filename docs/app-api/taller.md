# Contrato app-motor: Taller: capas, Z, rotativo y dialectos

Parte de [el contrato app-motor](README.md). Campos nuevos de capas y perfiles (puentes, Z, rotativo, dialectos G-code) de las fases de taller.

## Fase 4: taller (sin hardware)

Campos nuevos del contrato (todos con valor por defecto al leer documentos y perfiles viejos):

- `LayerSettings.fill_style`: `scan` (barrido) u `offset` (contornos concéntricos).
- `LayerSettings.tabs`: puentes de las capas line (`enabled`, `width`, `count`, `spacing`,
  `skip_inner`). Los huecos aparecen en la vista previa y en el G-code.
- `LayerSettings.z_offset` / `z_step`: profundidad del foco al empezar la capa y bajada por
  pasada, **relativas a la altura del cabezal al empezar el trabajo**. Positivo = foco más hacia
  dentro del material (el cabezal baja o la cama sube). Solo se emiten si `DeviceProfile.z_axis`
  es verdadero; si no, el trabajo avisa y los ignora.
- `LayerSettings.angle` ahora también es el ángulo de barrido de las capas image.
- `ImageMode`: `halftone`, `newsprint` y `sketch`; `ImageSettings.halftone_cell` (mm) y
  `halftone_angle` (grados).
- `DeviceProfile.rotary`: rotativo en lugar del eje Y (`roller` o `chuck`). El documento sigue en
  mm de superficie de la pieza; la conversión a mm de Y de la controladora se hace al generar el
  G-code y el encuadre.
- `DeviceProfile.scan_offsets`: corrección de ida y vuelta por velocidad (interpolada).
- `ToolpathBlock.z_mm` (motor): profundidad del foco respecto a la altura inicial al empezar el
  bloque (0 = no mover), `None` = no mover Z.

Campos nuevos de la revisión del taller (`docs/hardware/taller.md`, `dialectos-gcode.md`); un
perfil guardado sin ellos se lee con los valores indicados y produce el mismo G-code que antes
(salvo Z, que ahora es relativa, y el barrido, que ahora se reparte):

- `DeviceKind` pasa de `grbl` a `grbl | grbl_m3 | grbl_hal | fluid_nc | grbl_lpc | smoothie |
  marlin | custom_gcode`.
- `DeviceProfile.dialect: GcodeDialect`: cómo escribe el G-code el firmware (escala y decimales
  de S, línea de encendido `constant_on`/`dynamic_on` (`dynamic_on` nulo = no hay M4),
  `laser_off`, `s_inline` o línea de potencia aparte `power_cmd` con `{s}`, `g0_laser_off`,
  `travel_s0`, `feed_on_g0`, `air_on`/`air_off` (nulos = sin aire), `end_sync` (`M400`),
  `comment` (`semicolon` o `parentheses`), `max_line_len`). Si falta al leer, se toma el del
  `kind` (`GcodeDialect::for_kind`). La escala de S sigue siendo `DeviceProfile.s_max`. La interfaz
  debe aplicar `DeviceProfile::for_kind(kind)` (dialecto, `s_max`, `power_mode`, `baud`) al
  cambiar el tipo de un perfil nuevo y dejar editar los campos en `custom_gcode`.
- `DeviceProfile.z_invert` (por defecto `false`): solo afecta al G-code. Sin él, profundidad
  positiva sale como Z negativa (en GRBL +Z aleja el cabezal); con él, al revés. Prueba: con
  `z_offset = 1` el foco debe entrar 1 mm en el material.
- `DeviceProfile.scan_initial_offset` (mm, por defecto 0): corre todas las líneas de barrido
  (también las unidireccionales) en el sentido de las líneas de ida.
- `ScanOffset.offset` es la separación **medida** entre los extremos de ida y de vuelta; se reparte
  `-offset/2` (ida) y `+offset/2` (vuelta). Avisa si el overscan es menor que |offset|.
- `RotarySettings.mirror` (por defecto `false`): refleja la Y del trabajo dentro de su caja en el
  G-code. Los valores por defecto del rotativo pasan a 32 mm/vuelta y rodillo de 17 mm
  (`RotarySettings::preset(kind)` da 40 mm/vuelta para `chuck`); con `roller` no se valida
  `object_diameter`.
- Avisos nuevos en `JobPreview.warnings`: puentes frente al kerf, soft limits del rotativo
  (`ALARM:2`), overscan menor que el offset, y limitaciones del dialecto (sin aire, sin potencia
  dinámica, imágenes con S en línea aparte).
- Motor: `tlaku_pipeline::rotary_test(device, speed, power)` devuelve un `Document` con una línea
  de exactamente una circunferencia de la pieza (prueba de una vuelta); pasa por `compute` y
  `gcode` como cualquier diseño. Comando de Tauri: `rotary_test`.

`PreviewLayer.summary` puede incluir ", N puentes", ", pasada k/N" y ", Z x mm". Con `z_step != 0`
y varias pasadas, una capa produce una fila por pasada.
