# Venir desde LightBurn

Si ya usas LightBurn, aquí está cómo se llaman las cosas en Tlaku, qué atajos coinciden, qué falta todavía y cómo pasar un diseño de un programa al otro.

Tlaku es un programa independiente, hecho desde cero y sin usar el código ni los textos de LightBurn. Se parece en la idea (capas por color, modos Línea, Relleno e Imagen, vista previa, consola), pero no es un clon: algunas cosas se llaman distinto y varias funciones no existen aún. Los nombres de LightBurn de esta página son los que usa su interfaz; si tu versión los llama de otra forma, **verifica en tu versión**.

## Equivalencias de nombres

| En LightBurn | En Tlaku |
|---|---|
| Cuts / Layers (panel) | Pestaña **Capas** del panel derecho |
| Cut Settings Editor | Ajustes de la capa: doble clic o Intro sobre la capa, o `Alt+Enter` |
| Line / Fill / Image | Modos **Línea**, **Relleno**, **Imagen** |
| Offset Fill | Opción **Contornos** de una capa de relleno |
| Laser (panel) y Move | Pestaña **Máquina** (conexión, mover el cabezal, origen, trabajo) |
| Console | Pestaña **Consola** |
| Library | Pestaña **Materiales** (biblioteca de presets) |
| Material Test | **Prueba de materiales…** |
| Trace Image | **Vectorizar imagen…** |
| Camera | Pestaña **Cámara** |
| Frame | **Encuadrar** |
| Start / Pause / Stop | **Iniciar** / **Pausar** / **Detener** |
| Save GCode | **Exportar G-code…** (menú Archivo) |
| Start From: Absolute Coords, Current Position, User Origin | **Iniciar desde**: Coordenadas absolutas, Posición actual, Origen de usuario |
| Job Origin (los 9 puntos) | Punto del trabajo que se ancla (9 posiciones) |
| Set Origin | **Fijar origen aquí** |
| Devices, Find My Laser | **Gestionar dispositivos** y el asistente **Agregar máquina** (paso **Detectar**) |
| Numeric Edits | Barra numérica sobre el lienzo (X, Y, Ancho, Alto, Rotación) |
| Edit Nodes | **Editar nodos** (herramienta de nodos) |
| Boolean Union, Subtract, Intersection | **Unión**, **Diferencia (A menos B)…**, **Intersección** (más **XOR**) |
| Weld, Offset Shapes | **Soldar**, **Offset…** |
| Close Path, Join, Delete Duplicates | **Cerrar trayectos…**, **Unir trazos…**, **Quitar duplicados…** |
| Grid Array, Circular Array | **Arreglo en rejilla…**, **Arreglo circular…** |
| Align y Distribute | Menú **Organizar** y **Alinear y distribuir…** |
| Preview | **Vista previa** |
| Wireframe | **Solo contornos** |
| Undo History | Pestaña **Historial** |
| Convert to Path (texto) | **Convertir a trayectos** en el panel de texto |

Las ventanas de Tlaku no flotan: son pestañas del panel derecho (ver [La interfaz](02-interfaz.md)).

## Atajos

El esquema de Tlaku es **propio**: no reproduce el de LightBurn. En la tabla, la columna LightBurn solo lleva los atajos que son estándar en casi cualquier programa; para el resto escribo "verifica en tu versión".

| Acción | Tlaku | LightBurn |
|---|---|---|
| Nuevo proyecto | `Ctrl/Cmd+N` | `Ctrl+N` |
| Abrir | `Ctrl/Cmd+O` | `Ctrl+O` |
| Guardar | `Ctrl/Cmd+S` | `Ctrl+S` |
| Importar diseño | `Ctrl/Cmd+I` | `Ctrl+I` |
| Deshacer / Rehacer | `Ctrl/Cmd+Z` / `Ctrl/Cmd+Shift+Z` o `Ctrl/Cmd+Y` | `Ctrl+Z` / `Ctrl+Y` |
| Copiar, cortar, pegar | `Ctrl/Cmd+C`, `X`, `V` | iguales |
| Seleccionar todo | `Ctrl/Cmd+A` | `Ctrl+A` |
| Duplicar | `Ctrl/Cmd+D` | verifica en tu versión |
| Agrupar / Desagrupar | `Ctrl/Cmd+G` / `Ctrl/Cmd+Shift+G` | verifica en tu versión |
| Pegar en el mismo lugar | `Ctrl/Cmd+Shift+V` | verifica en tu versión |
| Invertir selección | `Ctrl/Cmd+Shift+I` | verifica en tu versión |
| Herramientas | `V` seleccionar, `N` nodos, `L` línea, `R` rectángulo, `E` elipse, `T` texto, `H` mano | verifica en tu versión |
| Vista previa | `P` | verifica en tu versión |
| Encuadrar | `F` | verifica en tu versión |
| Iniciar | `Ctrl/Cmd+Enter` | verifica en tu versión |
| Pausar / Detener | `Ctrl/Cmd+Shift+P` / `Ctrl/Cmd+.` | verifica en tu versión |
| Elegir capa por color | `Alt+1` a `Alt+0` | verifica en tu versión |
| Exportar G-code | `Ctrl/Cmd+Shift+E` | verifica en tu versión |
| Mostrar capas, máquina, consola, historial | `Ctrl/Cmd+1`, `2`, `3`, `4` | verifica en tu versión |
| Lista de atajos | `F1` o `Ctrl/Cmd+/` | verifica en tu versión |

Dos diferencias que muerden: en Tlaku las teclas de herramienta son **letras sueltas** (sin `Ctrl`) y no funcionan mientras escribes en un campo. Y los atajos **no se pueden reasignar** todavía (ver abajo).

## Lo que falta o es distinto

Hecho hoy en Tlaku: capas por color con modos Línea, Relleno e Imagen, vista previa con orden de corte, conexión GRBL por USB, consola, cámara, materiales, rotativo y eje Z, vectorizar, booleanas, arreglos y texto.

Falta, y no lo prometo si no está en el código:

- **Medir** está en el menú, pero hoy no tiene lógica propia (actúa como Seleccionar).
- No hay menú contextual (clic derecho) en el lienzo; las acciones de capa están en el menú **...** de cada fila.
- No hay ventana de preferencias: el imán (snap), los pasos de flecha y las unidades son fijos (milímetros; puedes escribir `5in` o `50%` en los campos).
- Máquinas de red (Wi-Fi, Ethernet), Ruida en la app, K40 con M2 Nano, galvo y fibra: ver [Controladoras soportadas](13-controladoras.md).
- Solo hay una capa de guías; no hay modo principiante.

Planeado para la primera ola, aún **no** en el programa:

- Voltear, bloquear y orden de dibujo de objetos (próximamente).
- Menú contextual en el lienzo (próximamente).
- Polígono, estrella y curvas Bézier dibujadas (próximamente).
- Preferencias, autoguardado y editor de atajos (próximamente).
- Exportar SVG, DXF e imagen (próximamente).
- Importar PDF, AI, WebP y TIFF (próximamente).
- Importar proyectos `.lbrn` y `.lbrn2` con sus capas y ajustes (próximamente).
- Fuentes de una línea, texto curvado, QR y códigos de barras (próximamente).

## Cómo pasar un diseño

Hoy se importa **SVG**, **DXF** e **imágenes** (PNG, JPG, BMP, GIF, entre otras). Tlaku **no abre** `.lbrn` ni `.lbrn2` todavía: si intentas abrir uno, la app dice que aún no se puede importar.

1. En el programa donde lo dibujaste, guarda o exporta el diseño como **SVG** (lo mejor, conserva las curvas) o **DXF**. Si lo hiciste en LightBurn, comprueba en tu versión qué formatos de exportación ofrece.
2. En Tlaku abre **Archivo > Importar diseño…** (`Ctrl/Cmd+I`).
3. Los trazos llegan **agrupados por color**: cada color pasa a ser una capa de modo Línea. Cambia el modo a Relleno donde quieras grabar.
4. **Vuelve a poner velocidad, potencia y pasadas.** Los ajustes de corte de LightBurn no viajan en SVG ni DXF. Puedes partir de la pestaña **Materiales** (ver [Materiales](11-materiales.md)).
5. Revisa el tamaño en la barra numérica. Si no coincide con el original, mira [Solución de problemas](15-solucion-de-problemas.md).
6. Crea o elige tu máquina ([Conectar tu máquina](08-conectar-maquina.md)) y mira la **Vista previa** antes de enviar.

> **Consejo:** prueba con la **máquina virtual** antes de tu primer trabajo real: no gasta material y te enseña cómo se ven el encuadre, las pausas y las alarmas.

> **Ojo:** nada de la conexión con la máquina se ha probado con hardware real. Mientras LightBurn te siga funcionando con tu láser, mantenlo como plan B.

Siguiente: [Solución de problemas](15-solucion-de-problemas.md)
