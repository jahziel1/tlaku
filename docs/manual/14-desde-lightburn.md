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
| Sub-Layers | **Operaciones adicionales** (hasta 11 más la principal; no tienen pestañas ni nombre propio) |
| Pass-Through, Enhance Edges, Denoise (Adjust Image) | **Enviar sin tramar (pass-through)**, **Realce de bordes** y **Quitar motas** en **Imagen avanzada** |
| Adjust Image: presets y vista previa | **Preset de imagen** (7 de fábrica y los tuyos con **Guardar como…**) y **Vista previa de la imagen…** por imagen, sin calcular el trabajo; ver [Imágenes](06-imagenes.md) |
| Copiar y pegar ajustes de capa, borrar capa | Menú de la fila de la capa: **Copiar ajustes**, **Pegar ajustes**, **Borrar capa y sus objetos…** |
| Default Layer Settings | **Valores por defecto** al final del editor de capa (un solo juego para todas las capas nuevas) |
| Beginner Mode | **[Modo sencillo](01b-modo-sencillo.md)** (esconde menús, herramientas y ajustes avanzados, y guía el trabajo paso a paso; es el modo para principiantes de Tlaku) |
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
| Macros | **Macros** del perfil (hasta 10, con botones y atajos); ver [Control avanzado de la máquina](09b-control-avanzado.md) |
| Move (Z y U), Fire, Focus | Bloque **Ejes extra** (Subir/Bajar), **Disparo de prueba** (doble confirmación, hasta 20 % y 2 s) y **Enfocar** en **Mover cabezal** |
| Saved Positions | **Posiciones guardadas** (en el perfil, hasta 20) |
| Frame con el láser encendido, repetir el frame | **Encuadrar con el láser encendido a baja potencia** y **Repetir el encuadre hasta detener** |
| Machine Settings (GRBL) | **Ajustes de la máquina…**: los `$n` con respaldo `.tlakuset`; no lee ni escribe los de Ruida |
| Dual laser (Laser 2) | Casilla **La máquina tiene dos láseres** y opción **Láser 2** por capa |
| User bundle `.lbzip` | **Archivo > Exportar / Importar paquete de configuración…** (`.tlakubundle`; no abre los `.lbzip`) |
| Device: tipo de conexión TCP/IP (red) | **Tipo de conexión** del perfil: **Red · TCP / Telnet** o **Red · WebSocket**, con **Probar conexión**; ver [Conexión por red y automatización](08b-red-y-automatizacion.md) |
| Ruida (DSP) | Perfil **Ruida (CO2, por red; experimental)**; ver [Ruida y K40](13b-ruida-y-k40.md). LightBurn lo soporta; en Tlaku es experimental |
| K40 con M2 Nano | LightBurn no lo soporta. Tlaku: experimental (solo probado contra el emulador), en la app por USB directo y con `tlaku k40`; la potencia va con la perilla; ver [Ruida y K40](13b-ruida-y-k40.md) |
| Automatización por UDP | **Preferencias > Automatización** (protocolo propio, no el de LightBurn); ver [Conexión por red y automatización](08b-red-y-automatizacion.md) |
| Camera: Update Overlay, Trace, Capture | **Actualizar la superposición**, **Vectorizar desde la cámara** y **Capturar imagen de la cama** (ver [Cámara](10-camara.md)) |
| Camera: lente de ojo de pez | **Modelo de lente**: Automático, Brown-Conrady u **Ojo de pez** |
| Center Finder | **Buscador de centro** de la cámara (no toca el cabezal como sonda) |
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
| Virtual Array | **Arreglo virtual…** (copias que no son objetos; ver [Copias, arreglo virtual y nesting](03d-copias-y-nesting.md)) |
| Copy Along Path | **Copiar a lo largo de un trazo…** (sin escala progresiva) |
| Move Together, Dock | **Acoplar y mover juntos…** |
| Nesting | **Acomodar piezas (nesting)…**, en el menú Organizar: cama o material irregular, rotaciones, huecos y tope de tiempo |
| Auto-Group | **Editar > Agrupar automáticamente por contención**, y la opción del mismo nombre al importar con opciones |
| Trim Shapes | Herramienta **Recortar formas** (`X`) y **Recortar en el último clic** (`Ctrl/Cmd+K`); ver [Formas avanzadas y deformar](03c-formas-y-deformar.md) |
| Break Apart | **Separar subtrayectos…** (`Alt+B`): separa por contorno, no por segmento |
| Optimize Shapes | **Optimizar formas…** (`Alt+Mayús+O`): quita y simplifica nodos con una tolerancia, con conteo antes y después |
| Fillet, Radius | **Redondear esquinas…**: redondeada, chaflán o mordisco; y la herramienta **Redondear esquinas con clic** (`Mayús+F`) para una sola esquina, que se revierte con `Alt`+clic (el chaflán no) |
| Cut Shapes | **Cortar formas con una cuchilla…** (`Alt+Mayús+C`) |
| Resize Slots | **Redimensionar ranuras…** |
| Warp, Deform | **Deformar…** (diálogo con presets, cuatro esquinas y cuatro bordes curvos) y **Deformar en el lienzo** (`Mayús+W`) con manijas: **Bilineal**, **Perspectiva** o **Malla 4×4** (16 puntos) |
| Rubber Band Outline (contorno envolvente) | **Contorno de banda elástica…**: convexo o cóncavo |
| Tangent Circle | **Círculo tangente…** |
| Boolean Assistant | **Asistente booleano…** (`Ctrl/Cmd+B`) |
| Guides | **Guías**: arrastra desde las reglas; **Ver > Guías…** para la posición exacta (sin alineación temporal con `Alt`) |
| Tabs / Bridges (manuales) | Herramienta **Puentes manuales** (`U`): clic para poner o quitar; **Potencia en puentes** en el editor de capa |
| Start Point, Reverse Direction | Herramienta **Punto de inicio** (`Q`), **Invertir sentido** y **Quitar inicio y sentido fijados** |
| Variable Text (serie, CSV, fecha y hora) | **Herramientas > Texto variable…** y el menú **Insertar variable** del panel de texto; ver [Texto variable](04b-texto-variable.md) |
| Art Library | **Herramientas > Biblioteca de arte**: 41 dibujos propios y tus SVG |
| (guardar una selección para reutilizarla) | **Guardar como componente…** (clic derecho) y la categoría **Mis componentes** de la biblioteca: guarda objetos con sus capas y ajustes; ver [Guías, puentes y proyecto](03e-guias-puentes-y-proyecto.md#mis-componentes-guardar-una-seleccion-con-sus-capas-y-ajustes) |
| Notes, Templates, New Window | Pestaña **Notas** (`Ctrl/Cmd+7`), **Guardar como plantilla…** y **Nuevo desde plantilla…** (menú Archivo), **Nueva ventana** (`Ctrl/Cmd+Mayús+N`) |
| Dockable windows | Las pestañas del panel se reordenan, se ocultan, **se sacan a ventanas flotantes** (doble clic en la pestaña) y el panel y las herramientas cambian de lado; **Restablecer distribución** lo deja como de fábrica |
| Set Image Mask, Convert to Bitmap, Save Processed Bitmap, Replace Image | **Recortar imagen con forma**, **Convertir a mapa de bits…**, **Guardar imagen procesada…** y **Reemplazar imagen…** (menú Herramientas); ver [Imágenes](06-imagenes.md) |
| Trace Image (varias imágenes) | **Vectorizar varios archivos…** |
| Align y Distribute | Menú **Organizar** y **Alinear y distribuir…** |
| Print & Cut | **Máquina > Print & Cut…**: asistente de 3 pasos con 2 a 4 marcas, medidas con el láser o con la cámara, ajuste sin escala, de escala uniforme o afín y el error de cada marca; ver [Print & Cut](09c-print-and-cut.md) (las imágenes también giran con el registro) |
| (no existe en LightBurn) | **Máquina > Dividir en tramos…** (*tiling*): trabajos más grandes que la cama, con solape, marcas de alineación, envío por tramos y retomar; ver [Tiling y alimentador automático](09d-tiling-y-alimentador.md) |
| Feeder (auto-feeder) | Grupo **Alimentador automático** del perfil: lo usa el tiling entre tramos (una columna) y el diálogo de inicio **repite el trabajo N veces** con avance entre repeticiones (solo GRBL); ver [Tiling y alimentador automático](09d-tiling-y-alimentador.md) |
| Machine profiles: default, order, icon | **Perfil predeterminado**, **Subir/Bajar en la lista** e **Ícono** en el grupo General del perfil; ver [Conectar tu máquina](08-conectar-maquina.md#ajustes-del-perfil-que-conviene-conocer) |
| Red pointer offset, Swap X/Y, Auto-home, DTR, Synchronous mode | **Desfase del puntero rojo** (y **Llevar el puntero rojo al punto**), **Intercambiar X e Y en la máquina**, **Ofrecer el homing al conectar**, **Abrir el puerto serie sin subir DTR** y **Envío síncrono** en el perfil |
| Units: mm/s | **Unidad de velocidad** del perfil (mm/min o mm/s): solo cambia cómo se muestran las velocidades del perfil y del diálogo de inicio |
| Camera presets, network cameras, head alignment | **Configuraciones de cámara**, **Cámara de red (URL)** y **Alineación automática con marcas**; ver [Cámara](10-camara.md) |
| (no existe en LightBurn) | **Máquina > Revisar diseño…** (`Ctrl/Cmd+Alt+R`): doce tipos de hallazgo con arreglos de un clic; ver [Revisar diseño](07b-revisar-diseno.md) |
| Job history / log | **Ayuda > Historial de trabajos…**: fecha, máquina, documento, pestaña, tiempos estimado y real, resultado, capas con sus ajustes, material, miniatura y notas de cada trabajo enviado; búsqueda y filtros, **Repetir con estos ajustes** y CSV ([Cola e historial de trabajos](09f-cola-e-historial-de-trabajos.md)) |
| Job queue | **Máquina > Cola de trabajos…**: varios documentos con copias; siempre se detiene entre trabajos y pide confirmar (no es un modo desatendido) |
| Galvo: Frame con luz roja, Start | Panel **Máquina** con una tarjeta galvo (encuadre con rectángulo, contorno, casco convexo u objetos sueltos, con ajuste en vivo) y `tlaku galvo`; experimental, ver [Galvo](13c-galvo.md) y [Seguridad del galvo](13d-galvo-seguridad.md) |
| 3D Slice, Cylinder Correction, Taper Warp | Con una máquina galvo: corrección de cilindro y de cono en el perfil y grabado 3D por cortes en el editor de capa; ver [Grabado 3D por cortes, cilindro y cono](12b-grabado-3d-y-cilindro.md) |
| Preview | **Vista previa** |
| Wireframe | **Solo contornos** |
| Undo History | Pestaña **Historial** |
| Convert to Path (texto) | **Convertir a trayectos** en el panel de texto y en el menú contextual |

Las ventanas de Tlaku son pestañas del panel, que se pueden reordenar y **sacar a una ventana flotante** (ver [Guías, puentes, punto de inicio y proyecto](03e-guias-puentes-y-proyecto.md)). Las barras de herramientas de arriba no se pueden mover.

## Atajos

El esquema de Tlaku es **propio**: no reproduce el de LightBurn, y tú puedes cambiar cualquier atajo (ver [Preferencias, atajos y autoguardado](02b-preferencias-y-atajos.md)); no hay todavía un perfil "tipo LightBurn". La columna LightBurn sale de su tabla de atajos oficial (documentación de LightBurn 2.1); esa tabla tiene contradicciones, y donde **no encontré fuente** escribo **(verifícalo en tu versión)**.

| Acción | Tlaku | LightBurn |
|---|---|---|
| Nuevo, abrir, guardar | `Ctrl/Cmd+N`, `O`, `S` | `Ctrl+N`, `O`, `S` |
| Guardar como | `Ctrl/Cmd+Shift+S` | `Ctrl+Shift+S` |
| Importar diseño | `Ctrl/Cmd+I` | `Ctrl+I` |
| Exportar | G-code: `Ctrl/Cmd+Shift+E`; SVG, DXF, PNG, PDF y AI sin atajo | `Alt+X` (exportar) |
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
| Puentes manuales / punto de inicio / recortar formas | `U` / `Q` / `X` | (verifícalo en tu versión) |
| Asistente booleano / recortar en el último clic | `Ctrl/Cmd+B` / `Ctrl/Cmd+K` | (verifícalo en tu versión) |
| Deformar en el lienzo / redondear esquinas con clic | `Mayús+W` / `Mayús+F` | (verifícalo en tu versión) |
| Revisar diseño | `Ctrl/Cmd+Alt+R` | (no existe) |
| Nueva ventana | `Ctrl/Cmd+Shift+N` | (verifícalo en tu versión) |
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

Hecho hoy en Tlaku: capas por color con modos Línea, Relleno e Imagen, vista previa con orden de corte y ajustes de optimización, conexión GRBL por USB y por red (TCP y WebSocket), macros, ejes Z y U, posiciones guardadas, ajustes `$$` con respaldo, consola, cámara, materiales, rotativo y eje Z, vectorizar, booleanas y su asistente, arreglos (también virtuales y a lo largo de un trazo), acoplar, nesting, recortar, separar, optimizar, redondear esquinas, cortar con una cuchilla, redimensionar ranuras, deformar, guías, notas y plantillas, biblioteca de arte, máscara de imagen, convertir a mapa de bits, vectorizar varias imágenes, texto (con fuentes de una línea, curvado y soldado), [texto variable](04b-texto-variable.md) (serie, CSV, fecha y hora), QR y códigos de barras, polígono y estrella, curvas Bézier, medir, voltear, orden de dibujo y bloqueo, puentes manuales, punto de inicio y sentido, menú contextual, preferencias, atajos configurables, autoguardado, los ajustes de corte avanzados (perforación, entrada y salida, sobrecorte, pausas, corte pasante, modo punto, rampa, operaciones adicionales, prioridad y escala de potencia por objeto, relleno por regiones, incremento de ángulo, imagen sin tramar), cortar solo la selección, encuadre de contorno, posicionar el láser, encuadre a baja potencia y en bucle, ajustes de velocidad y potencia en vivo, potencia en puentes, cámara con ojo de pez, cámara en el cabezal, buscador de centro, paquetes de configuración, Ruida (experimental), Print & Cut, tiling con alimentador automático, repetir trabajos con el alimentador, deformar con manijas, redondear con un clic, paneles flotantes, presets y vista previa de imagen, cámaras de red, [Revisar diseño](07b-revisar-diseno.md), el historial de trabajos y la cola guiada, el galvo en la app (experimental), y exportar e importar los formatos de [Importar y exportar](03b-importar-y-exportar.md) (también `.rd`, `.egv` y la exportación a `.ai`).

Falta, y no lo prometo si no está en el código:

- **Print & Cut y tiling ya están** (ver [Print & Cut](09c-print-and-cut.md) y [Tiling y alimentador automático](09d-tiling-y-alimentador.md)), pero se probaron solo con la máquina virtual. **El tiling es algo que LightBurn no tiene**: allí un trabajo más grande que la cama se parte a mano. Print & Cut en Tlaku admite hasta 4 marcas, un ajuste afín, medir con la cámara y las imágenes **giran con el registro**.
- **Del alimentador:** el diálogo de inicio repite un trabajo N veces con avance, **solo con máquinas GRBL**; el alimentador propio de una controladora DSP (Ruida, Trocen) no se maneja.
- **Del diseño:** escala progresiva al copiar a lo largo de un trazo, que **Cortar formas** agrupe las piezas en dentro y fuera, guías temporales de alineación y texto variable con valores de ajuste de capa o «hornear» (se logra convirtiendo el texto a trayectos).
- **Del corte:** arrastrar puentes ya puestos y valores por defecto por color y por máquina (hay un solo juego). Las velocidades de las **capas** siguen en mm/min (solo el perfil y el diálogo de inicio pueden mostrarlas en mm/s).
- **De la máquina:** las «unidades de dispositivo y de control» como en LightBurn (solo hay la unidad de velocidad), el ajuste de U por capa (hay un desplazamiento U por trabajo en el perfil) y el offset del segundo láser; el autoenfoque. Con Ruida no se escriben los ajustes de la controladora ni se borran o ejecutan sus documentos (los `.rd` sí se abren como diseño).
- **Del modo principiante:** en Tlaku solo oculta ajustes avanzados del editor de capa; no oculta herramientas ni menús como el de LightBurn.
- No se pueden ocultar objetos sueltos (sí capas), no hay selección con `Tab` ni sesgar.
- Las **pulgadas** de las preferencias se aplican en la barra numérica, la barra de estado, las reglas (décimas de pulgada), Medir y los campos de longitud de los diálogos de Organizar y de Acomodar piezas; los campos aceptan fracciones (`3/8"`, `1 1/2 in`) y sufijos (`5mm`, `50%`). Las velocidades siguen en mm/min y los campos de las capas, de las áreas de máquina y de cámara, en mm.
- **Galvo (fibra, UV):** soporte **experimental** para la tarjeta LMCV4 de EZCAD2, desde la app (alta con verificación sin láser, perfil, encuadre, overscan, entradas requeridas) y desde la línea de comandos (`tlaku galvo`), solo probado contra una tarjeta simulada. Lee primero [Seguridad del galvo](13d-galvo-seguridad.md). No hay pausa (solo detener) ni marcado al vuelo, el archivo `.cor` de EZCAD se aplica en el equipo (nunca se escribe en la tarjeta) y hay un asistente de nueve puntos, y EZCAD3/DLC y BSL no se soportan. Trocen y TopWisdom no están soportados. GRBL por red (TCP y WebSocket) está; Ruida y el K40 con M2 Nano son experimentales y ninguno se probó con una máquina real: ver [Controladoras soportadas](13-controladoras.md).
- Solo hay una capa de guías (la de la paleta de colores); las guías de las reglas son aparte.
- **Nueva ventana** no se ha probado en una ventana real (ver [Guías, puentes, punto de inicio y proyecto](03e-guias-puentes-y-proyecto.md)).

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
- **Ajustes de capa sin equivalente en Tlaku**, solo si estaban activos: pulsos por pulgada (PPI), frecuencia del láser, segundo láser, potencia constante, velocidad del soplador y relleno por objeto. No se traducen; hay que decidir a mano qué hacer.
- **Ajustes avanzados de corte que sí se traducen** (desde la ola 3): perforado, modo de puntos, relleno inundado, sobrecorte, rampa de potencia, pausas de inicio y fin, corte pasante y **sub-capas** (varias definiciones con el mismo índice pasan a **Operaciones adicionales**). Varias conversiones se infirieron de la documentación pública (por ejemplo, el tiempo del corte pasante sale de la pausa de inicio o de fin, la rampa arranca en 0 % y el overscan se interpreta como un porcentaje de la velocidad), así que **verifica cada capa** con la vista previa.
- **Entradas y salidas del contorno** (lead-in y lead-out): LightBurn no las guarda en la capa de un modo que se conozca, así que quedan apagadas.
- **Relleno + línea** en una sola capa: se importa solo el relleno (crea otra capa de línea si quieres el contorno).
- **Puentes manuales:** la posición de los puentes que pusiste a mano en LightBurn no se importa; aquí se reparten solos según los ajustes de la capa. Si te importa dónde quedan, pon los tuyos con la herramienta **Puentes manuales** (`U`).
- **Capas citadas pero no definidas** en el archivo, un modo de capa o de imagen desconocido, o un archivo guardado con una versión de LightBurn más nueva de las que Tlaku conoce (se intenta leer igual).

> **Ojo:** aunque los ajustes lleguen, **compruébalos antes de cortar**. La conversión de unidades de overscan se infirió de la documentación pública, y los ajustes importados de un archivo de LightBurn todavía no se han probado cortando o grabando en una máquina real. Revisa cada capa y mira la **Vista previa**.

> **Consejo:** prueba con la **máquina virtual** antes de tu primer trabajo real: no gasta material y te enseña cómo se ven el encuadre, las pausas y las alarmas.

> **Ojo:** con una máquina real, una Creality Falcon2 Pro con GRBL 1.1, Tlaku ya conectó, hizo homing, encuadró, grabó líneas, rellenos e imágenes, pausó, reanudó, detuvo y continuó tras un corte de energía. Con otras marcas, firmwares y controladoras (Ruida, K40, galvo), con la conexión por red, con cortes de potencia alta y con trabajos largos todavía no hay pruebas reales. Mientras Tlaku siga en beta, y mientras LightBurn te siga funcionando con tu láser, mantenlo como plan B.

Siguiente: [Solución de problemas](15-solucion-de-problemas.md)
