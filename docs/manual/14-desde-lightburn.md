# Venir desde LightBurn

Si ya usas LightBurn, aquí está cómo se llaman las cosas en Tlaku, qué atajos coinciden, qué falta todavía y cómo pasar un diseño de un programa al otro.

Tlaku es un programa independiente, hecho desde cero y sin usar el código ni los textos de LightBurn. Se parece en la idea (capas por color, modos Línea, Relleno e Imagen, vista previa, consola), pero no es un clon: algunas cosas se llaman distinto y varias funciones no existen aún. Los nombres de LightBurn de esta página son los que usa su interfaz según su documentación pública; si tu versión los llama de otra forma, **verifícalo en tu versión**.

## Equivalencias de nombres

| En LightBurn | En Tlaku |
|---|---|
| Cuts / Layers (panel) | Pestaña **Capas** del panel derecho |
| Cut Settings Editor | Ajustes de la capa: doble clic o Intro sobre la capa, o `Alt+Enter` |
| Line / Fill / Image | Modos **Línea**, **Relleno**, **Imagen** |
| Offset Fill | Opción **Contornos** de una capa de relleno |
| Flood Fill | Opción **Por regiones** del selector **Relleno** (sección **Relleno** del editor de una capa de relleno) |
| Fill Grouping | Selector **Agrupación**: **Todo junto**, **Forma por forma**, **Por grupos** |
| Angle Increment | **Incremento de ángulo** (grupo **Barrido**, con 2 o más pasadas) |
| Perforation Mode | **Perforación (corta y salta)** en **Corte avanzado**: **Tramo cortado** y **Tramo saltado** |
| Lead In / Lead Out | **Entrada al contorno** y **Salida del contorno** (**Recta** o **Arco**) |
| Overcut | **Sobrecorte** |
| Start/End Pause, Cut Through | **Pausa al empezar**, **Pausa al terminar** y **Corte pasante** |
| Dot Mode | **Modo punto** |
| Ramp Length | **Rampa de potencia** |
| Sub-Layers | **Operaciones adicionales** (hasta 10 más la principal; no tienen pestañas ni nombre propio) |
| Pass-Through, Enhance Edges, Denoise (Adjust Image) | **Enviar sin tramar (pass-through)**, **Realce de bordes** y **Quitar motas** en **Imagen avanzada** |
| Copiar y pegar ajustes de capa, borrar capa | Menú de la fila de la capa: **Copiar ajustes**, **Pegar ajustes**, **Borrar capa y sus objetos…** |
| Default Layer Settings | **Valores por defecto** al final del editor de capa (un solo juego para todas las capas nuevas) |
| Beginner Mode | **Modo principiante** del editor de capa (oculta los ajustes avanzados de la capa; no cambia menús ni herramientas) |
| Cut Order Priority, Power Scale (por objeto) | **Prioridad de corte** y **Escala de potencia** en **Propiedades del objeto**; **Cortar antes / Cortar después** en el menú Organizar |
| Optimization Settings | **Ajustes de optimización…** (vista previa, panel Máquina y menú Máquina) |
| Laser (panel) y Move | Pestaña **Máquina** (conexión, mover el cabezal, origen, trabajo) |
| Console | Pestaña **Consola** |
| Library | Pestaña **Materiales** (biblioteca de presets) |
| Material Test | **Prueba de materiales…** |
| Trace Image | **Vectorizar imagen…** |
| Camera | Pestaña **Cámara** |
| Frame | **Encuadrar** |
| Rubber Band Frame | **Encuadrar contorno** |
| Cut Selected Graphics, Use Selection Origin | **Solo la selección** y **Usar el origen de la selección** |
| Position Laser | **Posicionar el láser** |
| Adjust Speed / Adjust Power (durante el trabajo) | **Ajustes en vivo**: **Velocidad** y **Potencia** |
| Set Finish Position | **Posición final** (en **Mover cabezal**) |
| Job checklist | Lista **Antes de enviar** del diálogo de iniciar, configurable |
| Start / Pause / Stop | **Iniciar** / **Pausar** / **Detener** |
| Save GCode | **Exportar G-code…** (menú Archivo) |
| Export SVG, DXF, imágenes | **Exportar SVG…**, **Exportar DXF…**, **Exportar imagen PNG…** (menú Archivo); además **Exportar PDF…** e **Imprimir…** |
| Print | **Imprimir…**: genera un PDF a escala 1:1 y lo abre en el visor del sistema |
| Start From: Absolute Coords, Current Position, User Origin | **Iniciar desde**: Coordenadas absolutas, Posición actual, Origen de usuario |
| Job Origin (los 9 puntos) | Punto del trabajo que se ancla (9 posiciones) |
| Set Origin | **Fijar origen aquí** |
| Devices, Find My Laser | **Gestionar dispositivos** y el asistente **Agregar máquina** (paso **Detectar**) |
| Numeric Edits | Barra numérica sobre el lienzo (X, Y, Ancho, Alto, Rotación) |
| Create Lines (con curvas) | Herramienta **Dibujar línea** (`L`): clic y arrastre dibuja curvas Bézier |
| Draw Shapes: polígono, estrella | **Polígono** (`G`) y **Estrella** (`Mayús+G`), con la ventana **Propiedades de forma** |
| Shape Properties | **Ver > Propiedades de forma** (rectángulo, elipse, polígono y estrella) |
| Measure | **Medir** (`M`) |
| Create Bar Code (QR y otros) | **Herramientas > Insertar código…** (QR, Data Matrix y siete códigos de barras) |
| Create Text, Font Manager, SHX | Herramienta **Texto** (`T`) y su panel; **Herramientas > Importar fuente SHX…**; 15 fuentes de una línea incluidas |
| Welded text, Bend (texto curvado) | Casilla **Soldar letras** y campo **Curvatura** del panel Texto |
| Edit Nodes | **Editar nodos** (herramienta de nodos) |
| Flip Horizontal / Vertical | **Voltear horizontal** / **Voltear vertical** (menú Organizar) |
| Mirror Across Line | **Espejo sobre una línea** |
| Rotación por dos puntos | **Girar y escalar por dos puntos** |
| Draw Order (al frente, al fondo, subir, bajar) | **Traer al frente**, **Enviar al fondo**, **Subir un nivel**, **Bajar un nivel** |
| Lock (bloquear formas) | **Bloquear** y **Desbloquear todo** |
| Transform toggles | **Permitir mover**, **Permitir escalar**, **Permitir girar** (menú Organizar) |
| Make Same Width / Height | **Igualar ancho**, **Igualar alto** (y sus variantes con proporción) |
| Select open shapes, contained shapes, circles by size... | Menú **Editar**: **Seleccionar formas abiertas**, **...dentro de la seleccionada**, **...más chicas**, **Seleccionar círculos por tamaño…** |
| Right-click menu | **Menú contextual** del lienzo (clic derecho) |
| Archivo > Preferencias y Editar atajos | **Editar > Preferencias…** (`Ctrl/Cmd+,`) y **Editar > Editar atajos de teclado…** |
| Boolean Union, Subtract, Intersection | **Unión**, **Diferencia (A menos B)…**, **Intersección** (más **XOR**) |
| Weld, Offset Shapes | **Soldar**, **Offset…** |
| Close Path, Join, Delete Duplicates | **Cerrar trayectos…**, **Unir trazos…**, **Quitar duplicados…** |
| Grid Array, Circular Array | **Arreglo en rejilla…**, **Arreglo circular…** |
| Align y Distribute | Menú **Organizar** y **Alinear y distribuir…** |
| Preview | **Vista previa** |
| Wireframe | **Solo contornos** |
| Undo History | Pestaña **Historial** |
| Convert to Path (texto) | **Convertir a trayectos** en el panel de texto y en el menú contextual |

Las ventanas de Tlaku no flotan: son pestañas del panel derecho (ver [La interfaz](02-interfaz.md)).

## Atajos

El esquema de Tlaku es **propio**: no reproduce el de LightBurn, y tú puedes cambiar cualquier atajo (ver [Preferencias, atajos y autoguardado](02b-preferencias-y-atajos.md)); no hay todavía un perfil "tipo LightBurn". La columna LightBurn sale de su tabla de atajos oficial (documentación de LightBurn 2.1); esa tabla tiene contradicciones, y donde **no encontré fuente** escribo **(verifícalo en tu versión)**.

| Acción | Tlaku | LightBurn |
|---|---|---|
| Nuevo, abrir, guardar | `Ctrl/Cmd+N`, `O`, `S` | `Ctrl+N`, `O`, `S` |
| Guardar como | `Ctrl/Cmd+Shift+S` | `Ctrl+Shift+S` |
| Importar diseño | `Ctrl/Cmd+I` | `Ctrl+I` |
| Exportar | G-code: `Ctrl/Cmd+Shift+E`; SVG, DXF, PNG y PDF sin atajo | `Alt+X` (exportar) |
| Imprimir | `Ctrl/Cmd+P` | `Ctrl+P` |
| Deshacer / Rehacer | `Ctrl/Cmd+Z` / `Ctrl/Cmd+Shift+Z` o `Ctrl/Cmd+Y` | `Ctrl+Z` / `Ctrl+Shift+Z` |
| Copiar, cortar, pegar | `Ctrl/Cmd+C`, `X`, `V` | iguales |
| Seleccionar todo, invertir selección | `Ctrl/Cmd+A`, `Ctrl/Cmd+Shift+I` | `Ctrl+A`, `Ctrl+Shift+I` |
| Duplicar | `Ctrl/Cmd+D` | `Ctrl+D` |
| Pegar en el mismo lugar | `Ctrl/Cmd+Shift+V` | `Alt+V` |
| Agrupar / Desagrupar | `Ctrl/Cmd+G` / `Ctrl/Cmd+Shift+G` | `Ctrl+G` / `Ctrl+U` |
| Voltear horizontal / vertical | `Alt+H` o `Ctrl/Cmd+Shift+H` / `Alt+V` | `Ctrl+Shift+H` / `Ctrl+Shift+V` (y `H` / `V` con el lienzo enfocado) |
| Espejo sobre una línea | `Alt+Shift+M` | `Ctrl+Shift+M` |
| Girar 90° | `[` / `]` | `.` y `,` (con el lienzo enfocado) |
| Girar por dos puntos | `Alt+T` | `Ctrl+2` (con el lienzo enfocado) |
| Orden de dibujo | `RePág` / `AvPág`; con `Ctrl`, al extremo | `RePág` / `AvPág`; con `Ctrl`, al extremo (la propia tabla se contradice sobre cuál sube y cuál baja) |
| Seleccionar | `V` (también `Esc`) | `Esc` |
| Línea, rectángulo, elipse, texto | `L`, `R`, `E`, `T` (letras sueltas) | `Ctrl+L`, `Ctrl+R`, `Ctrl+E`, `Ctrl+T` |
| Editar nodos | `N` | `Ctrl` + la tecla del acento grave |
| Mano | `H` | (verifícalo en tu versión) |
| Polígono, estrella | `G`, `Mayús+G` | (verifícalo en tu versión) |
| Medir | `M` | `Alt+M` o `Ctrl+M` (la documentación dice las dos) |
| Offset | `Alt+O` | `Alt+O` |
| Soldar | `Alt+W` | `Ctrl+W` |
| Unión / Diferencia / Intersección | `Alt+U` / `Alt+D` / sin atajo | `Alt` con los signos `+`, `-` y `*` del teclado |
| Unir trazos (Auto-Join) | sin atajo | `Alt+J` |
| Quitar duplicados | sin atajo | `Alt+D` |
| Vista previa | `P` | `Alt+P` |
| Zoom, ajustar a la cama | `Ctrl/Cmd+=` y `-`, `Ctrl/Cmd+0` | `Ctrl+=` y `-`, `Ctrl+0` |
| Solo contornos | `W` | `Alt+Shift+W` |
| Mover con flechas | 1 mm; `Mayús` 10 mm; `Alt` 0,1 mm (configurable) | 5 mm; `Mayús` paso grande; `Ctrl` paso pequeño (configurable) |
| Alinear con el teclado | sin atajo | `Alt`+flechas |
| Encuadrar | `F` | (verifícalo en tu versión) |
| Iniciar | `Ctrl/Cmd+Enter` | (verifícalo en tu versión) |
| Pausar / Detener | `Ctrl/Cmd+Shift+P` / `Ctrl/Cmd+.` | (verifícalo en tu versión) |
| Elegir capa por color | `Alt+1` a `Alt+0` | (verifícalo en tu versión) |
| Mostrar capas, máquina, consola, historial | `Ctrl/Cmd+1`, `2`, `3`, `4` | (verifícalo en tu versión) |
| Lista de atajos | `F1` o `Ctrl/Cmd+/` | (verifícalo en tu versión) |

Diferencias que muerden:

- En Tlaku las teclas de herramienta son **letras sueltas** (sin `Ctrl`) y no funcionan mientras escribes en un campo. Lo que en LightBurn es `Ctrl+L` (línea), en Tlaku es `L`; y `Ctrl/Cmd+L` es **Bloquear**.
- **`Alt+V`** pega en el mismo lugar en LightBurn, pero **voltea en vertical** en Tlaku (ahí, pegar en el mismo lugar es `Ctrl/Cmd+Shift+V`).
- **`Alt+D`** quita duplicados en LightBurn; en Tlaku es **Diferencia**.
- **`Alt`+flechas** alinea en LightBurn; en Tlaku mueve la selección 0,1 mm.
- **`Ctrl/Cmd+W`** suelda en LightBurn; en Tlaku `Alt+W`.

## Lo que falta o es distinto

Hecho hoy en Tlaku: capas por color con modos Línea, Relleno e Imagen, vista previa con orden de corte y ajustes de optimización, conexión GRBL por USB, consola, cámara, materiales, rotativo y eje Z, vectorizar, booleanas, arreglos, texto (con fuentes de una línea, curvado y soldado), QR y códigos de barras, polígono y estrella, curvas Bézier, medir, voltear, orden de dibujo y bloqueo, menú contextual, preferencias, atajos configurables, autoguardado, los ajustes de corte avanzados (perforación, entrada y salida, sobrecorte, pausas, corte pasante, modo punto, rampa, operaciones adicionales, prioridad y escala de potencia por objeto, relleno por regiones, incremento de ángulo, imagen sin tramar), cortar solo la selección, encuadre de contorno, posicionar el láser, ajustes de velocidad y potencia en vivo, y exportar e importar los formatos de [Importar y exportar](03b-importar-y-exportar.md).

Falta, y no lo prometo si no está en el código:

- **Todo lo de la lista siguiente:** Trim, Break Apart, Fillet, Cut Shapes, Optimizar formas, deformar, nesting, Print & Cut, texto variable y números de serie, macros. Están planificados, pero **no** están en el programa.
- **Del corte:** puentes colocados a mano (los de Tlaku se reparten solos), valores por defecto por color y por máquina (hay un solo juego), presets de Adjust Image y velocidad en mm/s (todo está en mm/min). El encuadre no se repite en bucle ni va con el láser encendido a baja potencia, y la ventana Mover no tiene Z, posiciones guardadas, Fire ni Focus.
- **Del modo principiante:** en Tlaku solo oculta ajustes avanzados del editor de capa; no oculta herramientas ni menús como el de LightBurn.
- No se pueden ocultar objetos sueltos (sí capas), no hay selección con `Tab` ni sesgar.
- Las **pulgadas** de las preferencias solo se aplican hoy en Medir y en la ventana de Preferencias; la barra numérica sigue en mm y acepta `5in` o `50%`.
- Máquinas de red (Wi-Fi, Ethernet), Ruida en la app, K40 con M2 Nano, galvo y fibra: ver [Controladoras soportadas](13-controladoras.md).
- Solo hay una capa de guías.

## Cómo pasar un diseño

Hay dos caminos: **exportar desde LightBurn a un formato abierto** (lo que funciona hoy en la app) o **traer el proyecto `.lbrn` o `.lbrn2` completo**, con sus capas y ajustes (más abajo).

### Con SVG, DXF o PDF (funciona hoy)

1. En el programa donde lo dibujaste, guarda o exporta el diseño como **SVG** (lo mejor, conserva las curvas), **DXF** o **PDF**. Si lo hiciste en LightBurn, comprueba en tu versión qué formatos de exportación ofrece.
2. En Tlaku abre **Archivo > Importar diseño…** (`Ctrl/Cmd+I`), o **Importar con opciones…** si necesitas elegir la escala o el DPI.
3. Los trazos llegan **agrupados por color**: cada color pasa a ser una capa de modo Línea. Cambia el modo a Relleno donde quieras grabar.
4. **Vuelve a poner velocidad, potencia y pasadas.** Los ajustes de corte no viajan en SVG, DXF ni PDF. Puedes partir de la pestaña **Materiales** (ver [Materiales](11-materiales.md)).
5. Revisa el tamaño en la barra numérica. Si no coincide con el original, prueba otro DPI en las opciones de importación (Illustrator usa 72) o mira [Solución de problemas](15-solucion-de-problemas.md).
6. Crea o elige tu máquina ([Conectar tu máquina](08-conectar-maquina.md)) y mira la **Vista previa** antes de enviar.

### Con un proyecto de LightBurn (`.lbrn` y `.lbrn2`)

Tlaku abre proyectos de LightBurn con sus **capas y ajustes**: usa **Archivo > Importar diseño…** o arrastra el archivo a la ventana.

**Qué se conserva**

- **Formas:** rectángulos (con su radio de esquina), elipses, trayectos con curvas, grupos, **textos** y **imágenes** incrustadas. Cada forma queda en la misma posición de la cama. Los grupos anidados se aplanan a un solo grupo; los objetos bloqueados siguen bloqueados.
- **Imágenes:** la imagen incrustada se extrae a un archivo en una carpeta `NOMBRE_imagenes` junto al proyecto (o, si no se puede escribir ahí, en la carpeta temporal, y lo avisa). Si una capa lleva imágenes y vectores a la vez, las imágenes van a una capa propia.
- **Capas** (`C00` a `C29`): el nombre, el **modo** (línea, relleno, relleno por contornos, imagen), la **velocidad** (de mm/s a mm/min), la **potencia máxima y mínima**, las **pasadas**, el **intervalo** (o el DPI), el **ángulo**, el relleno cruzado, **bidireccional**, el **overscan**, el **kerf**, el **aire**, el **desplazamiento Z y Z por pasada**, si la capa sale a la máquina y si está oculta, los **puentes** y, en imágenes, el **método de tramado** y el semitono.
- **Orden de ejecución:** las capas se ordenan por la **prioridad** de LightBurn.
- **Guías:** las capas de herramienta `T1` y `T2` pasan a ser guías, que nunca salen a la máquina.
- **Colores:** LightBurn no publica el RGB exacto de su paleta de 30 colores; Tlaku usa colores **aproximados**, distintos por capa. Cada capa de LightBurn recibe un color distinto y conserva su nombre y su orden, pero no esperes el mismo tono.

**Qué se avisa**

Nada se pierde en silencio. Al importar, un aviso lista lo que no pudo traer:

- **Formas de un tipo no soportado:** se omiten y el aviso dice **cuántas y de qué tipo**.
- **Texto:** si el archivo no trae el contorno de las letras, Tlaku lo genera con tus fuentes (y avisa si la fuente no está instalada y usó otra); el **espaciado de letras y líneas** de LightBurn no se importa.
- **Ajustes de capa que el importador no traduce**, solo si estaban activos: perforado, modo de puntos, relleno inundado (flood fill), sobrecorte (overcut), rampa de potencia, pulsos por pulgada (PPI), frecuencia del láser, retardos de encendido y apagado, potencia de atravesado (cut-through), segundo láser, potencia constante, velocidad del soplador y relleno por objeto. Para seis de ellos (perforado, modo de puntos, relleno inundado, sobrecorte, rampa y potencia de atravesado) Tlaku ya tiene función propia (ver la tabla de equivalencias), pero el importador todavía **no los trae**: tendrás que volver a encenderlos a mano en el editor de la capa.
- **Relleno + línea** en una sola capa: se importa solo el relleno (crea otra capa de línea si quieres el contorno).
- **Puentes manuales:** en LightBurn se colocan a mano; aquí se reparten solos, así que revisa su posición.
- **Capas citadas pero no definidas** en el archivo, un modo de capa o de imagen desconocido, o un archivo guardado con una versión de LightBurn más nueva de las que Tlaku conoce (se intenta leer igual).

> **Ojo:** aunque los ajustes lleguen, **compruébalos antes de cortar**. La conversión de unidades de overscan se infirió de la documentación pública, y nada de esto se ha probado todavía con una máquina real. Revisa cada capa y mira la **Vista previa**.

> **Consejo:** prueba con la **máquina virtual** antes de tu primer trabajo real: no gasta material y te enseña cómo se ven el encuadre, las pausas y las alarmas.

> **Ojo:** nada de la conexión con la máquina se ha probado con hardware real. Mientras LightBurn te siga funcionando con tu láser, mantenlo como plan B.

Siguiente: [Solución de problemas](15-solucion-de-problemas.md)
