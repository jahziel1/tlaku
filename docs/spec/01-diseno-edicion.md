# Inventario funcional: interfaz y herramientas de diseño/edición

Área 01 del reemplazo open source de LightBurn en Rust. Documento de especificación de sala limpia: describe COMPORTAMIENTO observable con palabras propias, a partir de documentación pública, foro y tutoriales. No se consultó ni se copió código, texto literal, íconos ni capturas.

Fecha de investigación: 2026-10-12. Versión de referencia: documentación LightBurn 2.1 (con notas de 2.0/1.7 donde se encontraron).

## 0. Convenciones y fuentes

Abreviaturas de URL (para no repetirlas en cada línea):

- `D:` = `https://docs.lightburnsoftware.com/2.1/Reference/` (por ejemplo `D:Text/` = `https://docs.lightburnsoftware.com/2.1/Reference/Text/`)
- `U:` = `https://docs.lightburnsoftware.com/2.1/Reference/UI/`
- `G:` = `https://docs.lightburnsoftware.com/2.1/`
- `H:` = `https://docs.lightburnsoftware.com/Hotkeys` (tabla de atajos)
- `F:NNNN` = hilo del foro `https://forum.lightburnsoftware.com/t/<slug>/NNNN` (se listan completos en el apéndice A).

Marcas:

- "(no verificado)" = no se pudo confirmar en una fuente leída en esta sesión.
- "(inferido)" = deducción razonable a partir de lo verificado, no afirmada por la fuente.
- Los nombres de crates y proyectos de la columna "Ref. OSS" salen de conocimiento previo del autor y NO se verificaron en esta sesión (salvo `lbrn2-to-svg`, que apareció en una búsqueda). Validar versión, licencia y estado antes de depender de ellos.

Prioridades: P0 = imprescindible para un MVP; P1 = importante; P2 = avanzado. Complejidad: baja / media / alta (esfuerzo de implementación en Rust, no de uso).

Alcance: se excluye todo lo propio de control de máquina (Laser window, Move window, cámara, rotary, galvo, print-and-cut, ajustes de capas de corte/relleno/imagen, optimización de trayectorias, Preview). Se mencionan solo cuando tocan el diseño (por ejemplo, el panel de capas como destino de la paleta de colores).

### Inconsistencias detectadas en la propia documentación oficial

Útiles para no copiar errores:

- Convert to Bitmap: la página del menú Edit lista el mismo atajo que Convert to Path; la tabla de atajos lista Ctrl/Cmd+Shift+B (`U:EditMenu/` vs `H:`).
- Cut Shapes: la página del menú Tools dice Alt+Shift+C; la tabla de atajos dice Alt+C (`U:ToolsMenu/` vs `H:`).
- Measure: Alt+M en su página y en la barra Creation; Ctrl/Cmd+M en el menú Tools. Position Laser aparece como Alt+L y como Ctrl/Cmd+L (este último choca con Draw Lines) (`U:CreationToolbar/`, `U:ToolsMenu/`).
- Flip Horizontal/Vertical: el menú Arrange dice que "Flip Horizontal refleja sobre el eje X", mientras que la página de Flip dice que usa el centro horizontal de la selección. La semántica práctica (invertir izquierda/derecha vs arriba/abajo) debe decidirse por prueba, no por lectura. (`U:ArrangeMenu/`, `D:FlipMirror/`).
- Draw Order: la tabla describe "push forward" y "push backward" ambos como "subir una posición"; el texto aclara que es un error de la tabla (`D:DrawOrder/`).
- Flip: atajos "V" y "H" (solo con el lienzo enfocado) frente a Ctrl/Cmd+Shift+H/V (globales). Ambos existen (`D:FlipMirror/`, `H:`).

---

## 1. Tabla resumen

| ID | Función | Qué hace | Prio | Compl. | Referencia OSS |
|---|---|---|---|---|---|
| F01 | Lienzo (Workspace) | Área de trabajo con borde y rejilla del tamaño de la cama; fondo claro u oscuro; objetos se asignan a operaciones por color. | P0 | media | egui/wgpu o vello + kurbo; Inkscape canvas |
| F02 | Paleta de colores / capas | Cada color = una capa (operación). Clic con nada seleccionado fija el color de lo que se dibuje; con selección, reasigna capa. Dos capas "herramienta" (T1, T2) que nunca se envían al láser. | P0 | media | MeerK40t (operaciones por elemento) |
| F03 | Lista de capas (vista de diseño) | Lista las capas en uso, reordenable; Shift+clic selecciona todo lo de la capa; clic derecho "parpadea" los objetos. Ajustes de corte = otra área. | P0 | baja | n/a |
| F04 | Barras y ventanas acoplables | Paneles y barras que se arrastran, flotan, se apilan en pestañas, se restauran con "Reset layout" (o Shift al iniciar). | P1 | alta | egui_dock, Dockview-style; Qt dock (referencia) |
| F05 | Barra de estado | Coordenadas del cursor, estado de la herramienta activa (caja de selección, dx/dy/longitud/ángulo), mensajes, indicador de modos. | P1 | baja | n/a |
| F06 | Barra Numeric Edits | X/Y/ancho/alto/rotación numéricos, rejilla de 9 puntos de anclaje, candado de proporción, % de escala, mm/in, expresiones matemáticas y sufijos de unidad. | P0 | media | evalexpr / fasteval (parser de expresiones) |
| F07 | Zoom y paneo | Rueda (centrada en cursor), trackpad, +/-, Zoom to Page, Frame Selection, paneo con botón central/Espacio/herramienta mano. | P0 | baja | n/a |
| F08 | Estilo de vista | Alambre (solo contornos) o Relleno (capas Fill se ven sólidas); calidad de bordes suave/gruesa. | P1 | baja | n/a |
| F09 | Menú contextual | Clic derecho sobre objetos: editar texto, convertir a path, bloquear, aplicar path a texto, trace, editar código de barras, etc. | P1 | baja | n/a |
| F10 | Atajos por defecto | Conjunto grande de atajos de teclado con variantes Win/Linux y macOS, algunos solo con el lienzo enfocado. | P0 | baja | n/a |
| F11 | Editor de atajos | Buscar comando, reasignar o limpiar atajo, resetear, exportar/importar. | P1 | media | n/a |
| F12 | Preferencias de edición | Distancia de snap a rejilla/objeto (px), incremento de nudge, tolerancia de clic, "volver a Select tras dibujar", unidades de diseño vs control, rejilla visual. | P1 | baja | n/a |
| F13 | Deshacer / rehacer | Pila sin límite fijo, se limpia al cerrar o abrir proyecto nuevo. | P0 | media | undo basado en comandos o snapshots persistentes (im-rs) |
| F14 | Historial de deshacer | Lista de acciones pasadas, previsualiza el proyecto en cada punto y salta a él. Nuevo en 2.1. | P2 | media | n/a |
| F15 | Portapapeles básico | Copiar, cortar, pegar (centrado en cursor), pegar en sitio, duplicar (sobre el original), borrar. | P0 | baja | arboard |
| F16 | Portapapeles entre apps | Pegar vectores/texto/imágenes desde Inkscape/Illustrator/Corel; a veces llegan como imagen. | P1 | alta | arboard + parser SVG/PDF del portapapeles |
| F17 | Interruptores de transformación | Habilitan/deshabilitan globalmente mover, escalar, rotar, sesgar para evitar accidentes. | P2 | baja | n/a |
| F18 | Modo principiante | Oculta/simplifica controles (asistentes de alinear y booleanas, sin tabs, sin set start point). | P2 | baja | n/a |
| F19 | Nueva ventana, notas, plantillas | Segunda instancia, notas de proyecto, plantillas `.lbt` que no se sobrescriben al guardar. | P2 | baja | n/a |
| F20 | Selección | Clic, caja envolvente (derecha, solo contenidos) o cruzada (izquierda, toca), Shift/Ctrl, Tab para ciclar, Esc deselecciona. | P0 | media | rstar/bvh para hit-test |
| F21 | Selecciones especiales | Invertir, formas abiertas, abiertas en capa de relleno, todas en capa, contenidas, menores que la seleccionada, círculos por tamaño. | P1 | media | n/a |
| F22 | Manejadores de transformación | Mover, escalar (esquina proporcional; lado un eje), rotar (esquinas curvas, pasos 15°/5°), sesgar; Esc cancela, Shift/Ctrl/Alt modifican. | P0 | media | kurbo::Affine |
| F23 | Nudge con flechas | Mueve la selección con flechas; incrementos configurables (defecto 5 mm; 1, 20 y 0,1 mm con modificadores). | P0 | baja | n/a |
| F24 | Rotación rápida por teclado | Giros de 90°, 45°, 10° y 5° con teclas; ángulo libre en Numeric Edits. | P0 | baja | kurbo |
| F25 | Rotación/escala por dos puntos | Elige un pivote (con snap) y arrastra para rotar; Shift escala a la vez. | P2 | media | n/a |
| F26 | Voltear H/V | Refleja la selección sobre el centro de su caja envolvente. | P0 | baja | kurbo |
| F27 | Espejo sobre línea | Crea copia reflejada respecto a una recta de dos puntos elegida (última en la selección). | P1 | media | kurbo |
| F28 | Bloquear formas | Impide mover, escalar y borrar objetos individuales; sirve como referencia de alineación. | P1 | baja | n/a |
| F29 | Orden de dibujo | Enviar adelante/atrás un paso o al frente/fondo; solo visible en modo alambre. | P1 | baja | n/a |
| F30 | Agrupar / desagrupar | Une objetos de cualquier tipo/capa en una unidad; grupos anidados; muchas herramientas no operan sobre grupos. | P0 | media | n/a |
| F31 | Auto-group | Agrupa formas contenidas por completo dentro de una forma exterior. | P2 | baja | geo (contains) |
| F32 | Dibujar líneas / Bézier | Polilíneas y curvas por clic o clic+arrastre; S alterna nodo suave/esquina; cierre al clic en el inicio; auto-unión de extremos. | P0 | alta | kurbo (BezPath) |
| F33 | Rectángulo y elipse | Formas primitivas paramétricas: ancho/alto, radio de esquina (también negativo), desde centro con Ctrl, regular con Shift. | P0 | media | kurbo (Rect, Ellipse, RoundedRect) |
| F34 | Polígono, estrella y afines | Triángulo, pentágono, octágono, polígono n-lados, estrella (puntas, bulge, ratio) y estrella de doble radio. | P1 | media | kurbo |
| F35 | Ventana Shape Properties | Edita por tipo: ancho/alto, lados, radio de esquina, texto (ancho máx.), imagen (gamma, contraste…), más prioridad de corte, power scale y bloqueo. | P1 | baja | n/a |
| F36 | Crear/editar texto | Clic y escribir; Enter = nueva línea; Esc termina; doble clic edita; ancla por defecto en el centro. | P0 | alta | cosmic-text, rustybuzz, ttf-parser/skrifa, fontdb |
| F37 | Opciones de texto | Fuente, altura, HSpace, VSpace, alineación X/Y, negrita, cursiva, mayúsculas, RTL. | P0 | media | rustybuzz, swash |
| F38 | Texto soldado (Welded) | Fusiona letras que se solapan (fuentes script); también al reducir el espaciado. | P1 | media | i_overlay / clipper2 |
| F39 | Texto curvado (Bend) | Manejador azul que dobla el texto sobre un círculo; opción Distort. | P1 | media | kurbo |
| F40 | Aplicar path a texto | Texto sigue el contorno de un path arbitrario; el path sigue editable y el texto se actualiza; alineación X/Y. | P1 | alta | kurbo (arclen, ParamCurve) |
| F41 | Gestor de fuentes y SHX | Buscar, favoritas, ocultar, categorías; fuentes SHX de una sola línea. | P2 | media | fontdb; parser SHX propio |
| F42 | Texto variable | Sustitución en salida: fecha/hora, serie, CSV (merge), valores de ajustes de corte; ventana con start/end/avance; "bake". | P1 | media | csv, chrono/time |
| F43 | Códigos de barras y QR | Muchas simbologías (QR, Data Matrix, Code128, PDF417, EAN/UPC, Aztec, AprilTag…); datos variables; corrección de error. | P1 | media | rxing, qrcode, rqrr (verificar) |
| F44 | Medir | Hover: perímetro, ancho/alto, abierta/cerrada, longitud y ángulo de segmento, radios de arco; arrastre entre dos puntos. | P1 | media | kurbo |
| F45 | Tabs / puentes | Pequeños tramos sin cortar para que las piezas no caigan; manuales (arrastrables) o automáticos; potencia de tab. | P1 | media | kurbo (arclen) |
| F46 | Círculo tangente | Genera círculos tangentes a otros tres, eligiendo entre candidatos. | P2 | baja | geometría analítica (Apolonio) |
| F47 | Biblioteca de arte | Guarda objetos reutilizables en archivos `.lbart`; arrastrar al lienzo. | P2 | baja | n/a |
| F48 | Editar nodos | Mover/insertar/borrar nodos, línea↔curva, suave↔esquina, romper, unir arrastrando extremos, alinear, recortar, extender; multiselección. | P0 | alta | kurbo; Inkscape node tool |
| F49 | Trim Shapes | Hover resalta tramo hasta la próxima intersección; clic lo borra; reune el resto en un path. | P1 | alta | kurbo (intersect), geo |
| F50 | Convertir a path | Primitivas, texto y códigos de barras pasan a nodos editables; se pierden propiedades paramétricas. | P0 | baja | kurbo |
| F51 | Auto-Join | Une segmentos sueltos cuyos extremos coinciden (tolerancia 0,05 mm) en paths continuos. | P0 | media | kurbo; Inkscape "combine/join" |
| F52 | Close Path | Cierra un path con extremos a ≤0,5 mm añadiendo una línea. | P1 | baja | n/a |
| F53 | Cerrar con tolerancia | Diálogo con umbral; cierra uniendo con línea o moviendo extremos; informa de cuántas quedaron abiertas. | P1 | baja | n/a |
| F54 | Borrar duplicados | Elimina objetos idénticos en forma, tamaño y posición. | P1 | media | hash geométrico canónico |
| F55 | Break Apart | Divide cada path en sus segmentos individuales. | P1 | baja | kurbo |
| F56 | Optimizar formas | Suaviza, ajusta a líneas y a arcos dentro de una tolerancia; muestra conteo de nodos antes/después. | P1 | alta | kurbo (fit_to_bezpath), Douglas-Peucker/Visvalingam (crate `simplify-polyline`/propio), cavalier_contours (arcos) |
| F57 | Dirección y punto de inicio | Invertir dirección de un path; fijar nodo de inicio y sentido (flecha azul). | P1 | baja | kurbo |
| F58 | Warp / Deform | Deformación no lineal por 4 esquinas o malla 4×4 (16 puntos). | P2 | alta | kurbo; mallas de Bézier/TPS propio |
| F59 | Contorno "banda elástica" | Crea el contorno más ajustado que envuelve la selección. | P2 | baja | geo (convex hull) |
| F60 | Radio / Fillet | Redondea esquinas (radio negativo = mordisco), o revierte a esquina viva; clic por esquina. | P1 | media | kurbo; cavalier_contours |
| F61 | Offset de formas | Contorno paralelo hacia dentro, fuera o ambos; estilos de esquina; solo exteriores; borrar originales; simplificar. | P0 | alta | i_overlay (offset/outline), clipper2, cavalier_contours, kurbo::offset |
| F62 | Booleanas: unión, resta, intersección | Operan sobre exactamente dos formas/grupos cerrados; en resta el 2.º seleccionado se resta del 1.º. | P0 | alta | i_overlay, clipper2, geo |
| F63 | Weld | Como unión pero con N objetos a la vez. | P0 | media | i_overlay, clipper2 |
| F64 | Asistente booleano | Diálogo con vista previa de la operación; confirmar o cancelar. | P2 | baja | n/a |
| F65 | Cut Shapes | Una forma cerrada (la última seleccionada) corta a las demás en dos grupos: dentro y fuera. | P1 | media | i_overlay |
| F66 | Grid Array | Matriz de columnas/filas con espaciado centro-centro o por borde, desfases, espejo alterno, rotación aleatoria con semilla, total en ancho/alto. | P0 | media | n/a |
| F67 | Arreglo virtual | Clones sincronizados con el original; editar/aplanar/salir del arreglo. | P2 | alta | n/a |
| F68 | Circular Array | Copias radiales alrededor de un centro: copias, inicio/fin, paso, rotar copias. | P1 | baja | kurbo |
| F69 | Copy Along Path | Copias a lo largo de un path con espaciado, rotación y escala progresiva. | P2 | media | kurbo (arclen) |
| F70 | Igualar ancho/alto | Redimensiona a la medida del objeto de referencia (con Shift mantiene proporción). | P2 | baja | n/a |
| F71 | Resize Slots | Cambia ranuras/pestañas de ensamblaje al cambiar grosor de material. | P2 | alta | n/a |
| F72 | Alinear | Bordes y centros respecto a un objeto de referencia (el último añadido por defecto); reglas por objetos bloqueados. | P0 | baja | n/a |
| F73 | Distribuir | Espaciado igual entre bordes o centros, H o V. | P1 | baja | n/a |
| F74 | Move Together y Dock | Pegar objetos entre sí o deslizarlos hasta borde/objeto, con padding y "como grupo". | P2 | media | n/a |
| F75 | Mover selección a posición | A esquinas, bordes o centro de la cama. | P1 | baja | n/a |
| F76 | Nesting | Quick Nest (interno, empaqueta por rectángulos mínimos) y vía SVGnest externo. | P2 | alta | jagua-rs / sparrow, SVGnest (JS) como referencia |
| F77 | Snapping | A nodo, punto medio, centro, intersección, línea y rejilla; Alt "pegajoso"; Ctrl invierte. | P0 | media | rstar |
| F78 | Guías | Alineación temporal (Alt al arrastrar) y guías arrastrables desde reglas (en capa T1). | P2 | baja | n/a |
| F79 | Trace Image | Convierte mapa de bits a vectores: umbral/corte, ignorar manchas, optimizar, suavidad, transparencia, modo boceto, vista previa. | P1 | media | vtracer (visioncortex), potrace (bindings), imageproc |
| F80 | Trace multiarchivo | Automatiza el trazado de varios archivos (función experimental "Labs"). | P2 | media | n/a |
| F81 | Máscara / recorte de imagen | Oculta parte de una imagen con una forma cerrada; aplanar o recortar de forma permanente. | P2 | media | image |
| F82 | Imagen a mapa de bits y opciones de imagen | Convertir vectores a bitmap; guardar bitmap procesado; refrescar/reemplazar imagen. | P2 | media | tiny-skia/resvg para rasterizar |
| F83 | Importar SVG | Importa SVG como paths (opción 96 o 72 DPI), grupos, texto con fuentes instaladas. | P0 | alta | usvg/resvg |
| F84 | Importar DXF | Importa DXF con detección de unidades, cierre automático por tolerancia, opción ignorar texto. | P0 | alta | dxf (dxf-rs), Inkscape/LibreCAD |
| F85 | Importar AI | Importa Adobe Illustrator; capas ocultas opcionales; sin soporte de máscaras de recorte. | P1 | alta | (AI moderno = PDF) lopdf/pdf-extract; usvg no |
| F86 | Importar PDF | Importa vectores de PDF; soporta más compresión desde 2.1. | P1 | alta | lopdf, pdfium-render, mupdf |
| F87 | Importar imágenes | BMP, JPG, GIF, TIF/TIFF, TGA, WEBP, PNG como objetos imagen. | P0 | baja | image |
| F88 | Importar PLT/HPGL | Formatos de plotter con precisión configurable. | P2 | media | parser propio (HPGL sencillo) |
| F89 | Importar G-code | Solo geometría (se pierden velocidad y potencia); rellenos entran como líneas. | P2 | media | MeerK40t / gcode crates |
| F90 | Otros formatos de máquina | .rd, .sc, .scpro2, .nc, .mage (listados en el menú Import). | P2 | alta | MeerK40t (RD) (no verificado en detalle) |
| F91 | Opciones de importación | Agrupar importados, seleccionar, centrar, permitir capas T, DPI SVG, unidades DXF, tolerancia de auto-cierre. | P0 | media | n/a |
| F92 | Exportar SVG | Exporta selección o todo; capas ocultas se conservan como ocultas (2.1). | P1 | media | svg crate / usvg writer |
| F93 | Exportar DXF | Exporta selección o todo; versiones AC1009 y AC1021 (2.1); arcos como arcos o segmentos. | P1 | media | dxf (dxf-rs) |
| F94 | Exportar AI | Exportación Illustrator desde el menú Export. | P2 | media | (no verificado) |
| F95 | Exportar imágenes | PNG/JPG/BMP solo con una imagen seleccionada; sin ajustes de capa. | P2 | baja | image |
| F96 | Proyecto nativo | Abrir/guardar proyecto propio (en LightBurn: `.lbrn2`, con `.lbrn` heredado); plantillas. | P0 | media | serde + formato propio |
| F97 | Importar `.lbrn`/`.lbrn2` | Leer proyectos de LightBurn (XML sin documentación oficial) para migración. | P1 | alta | `lbrn2-to-svg` (TypeScript, MIT) como referencia de estructura |
| F98 | Imprimir / PDF | Imprimir solo negro o conservando colores. | P2 | media | printpdf |
| F99 | Preferencias y bundles | Importar/exportar preferencias `.lbprefs` y bundles `.lbzip`. | P2 | baja | n/a |
| F100 | Arrastrar y soltar archivos | Soltar archivos de imagen/vector/proyecto sobre la ventana. | P1 | baja | winit |

---

## 2. Detalle de funciones P0 y P1

Formato por función: **Comportamiento**, **Parámetros visibles**, **Casos borde / quejas**, **Fuentes**.

### 2.1 Interfaz principal

#### F01 Lienzo (Workspace) [P0]
- **Comportamiento:** el lienzo es el "Edit Window". Su borde y su rejilla reflejan el área útil de la máquina (ancho y alto configurados en los ajustes de dispositivo). Fondo gris claro por defecto, gris oscuro opcional. Con cámara, se puede dibujar la captura detrás de la rejilla; sin cámara, se puede dibujar el material como forma en una capa de herramienta y colocar el diseño encima.
- **Parámetros:** tamaño de cama, espaciado visual de rejilla (la rejilla muestra una subdivisión 10×10 al acercar), contraste de rejilla, fondo oscuro, cruz en el centro del área, cursor de línea completa.
- **Casos borde:** el tamaño de cama viene del dispositivo; si no coincide con la máquina real, el origen y el encuadre fallan (ver "Machine size mismatch" en soporte; fuera de esta área).
- **Fuentes:** `U:Workspace/`, `U:` (índice de la ventana), `D:SettingsPreferences/`.

#### F02 Paleta de colores / capas [P0]
- **Comportamiento:** cada color de la paleta es una capa = una operación del trabajo (no el color final del producto). Con nada seleccionado, pulsar un color fija el color/capa de los objetos que se dibujen después. Con objetos seleccionados, pulsar un color los recolorea y los mueve a esa capa. Las dos últimas entradas (T1 y T2) son capas de herramienta: no tienen parámetros de corte y nunca van al láser; sirven para límites de material, guías, trayectorias de texto y máscaras de imagen. Al importar, se intenta emparejar los colores del archivo con los de la paleta (coincidencia exacta asegura la capa correcta).
- **Parámetros:** 30 colores de capa numerados 00 a 29 más T1 y T2 (la página publica la tabla RGB/hex), paletas descargables para Inkscape/GIMP, Corel y Adobe/Affinity.
- **Casos borde:** colores importados que no coinciden exactamente caen en la capa más cercana (comportamiento del emparejamiento no detallado: no verificado); los objetos de una misma capa comparten operación, por eso duplicados o solapes en la misma capa dan resultados raros (ver F54).
- **Fuentes:** `U:ColorPalette/`, `D:CutsLayersWindow/` (nombre de la ventana de capas, contenido de ajustes fuera de alcance).

#### F03 Lista de capas (vista de diseño) [P0]
- **Comportamiento (solo lo relevante a diseño):** lista las capas en uso, permite reordenarlas, Shift+clic en una capa selecciona todos sus objetos, clic derecho hace "parpadear" los objetos de esa capa, doble clic abre los ajustes completos.
- **Fuentes:** `H:` (sección Cuts/Layers), `D:Selection/`, `G:GetStarted/ColorsAndLayers/` (no leída en detalle).

#### F04 Barras de herramientas y ventanas acoplables [P1]
- **Comportamiento:** cualquier ventana se arrastra por su barra de título, flota o se acopla a izquierda/derecha/arriba/abajo, apilada o en pestañas; las barras se arrastran por su asa y se acoplan arriba, abajo o al lado. Se redimensionan las zonas acopladas. Si una barra no cabe, aparece una flecha que despliega el resto. "Reset to Default Layout" restaura todo; mantener Shift al iniciar también. No se documenta guardar layouts personalizados con nombre.
- **Paneles del layout por defecto:** menús, barra principal, barra Arrange, Numeric Edits, Text Options (solo con texto seleccionado), Modes, Docking, Creation (lateral), Modifiers (lateral), Cuts/Layers, Move, Console, Macros, Shape Properties, Laser, Material Library, paleta de color, interruptores de transformación, barra de estado. La lista del menú Window añade Art Library, Camera Control, File List, Variable Text, Tools.
- **Casos borde:** el espacio de acoplamiento depende del tamaño de pantalla y de la ventana (si no deja soltar, maximizar). Shape Properties y Variable Text se acoplan por defecto detrás de Cuts/Layers, lo que los vuelve poco descubribles.
- **Fuentes:** `G:Guides/CustomizingTheLightBurnWindow/`, `U:` (índice), `U:WindowMenu/`, `D:ResetToDefaultLayout/` (listada, no leída).

#### F05 Barra de estado [P1]
- **Comportamiento:** abajo a la izquierda. Cuatro zonas: coordenadas X/Y del cursor (se congelan al salir del lienzo); estado de la herramienta (con Select: esquinas de la caja y número de objetos; con Draw Lines: posición, dx/dy desde el inicio de la línea, longitud, ángulo entre segmentos); notificaciones; indicador de modo especial.
- **Fuentes:** `U:StatusBar/`, `D:DrawLines/`.

#### F06 Barra Numeric Edits [P0]
- **Comportamiento:** edita tamaño, posición y rotación de la selección como unidad. Una rejilla de 9 puntos (esquinas, mitades de lados, centro) define el origen de las coordenadas X/Y, el ancla del escalado y el centro de rotación. Candado de proporción. Casillas de porcentaje junto a ancho/alto. Rotación positiva = sentido horario. Selector mm/in (en 2.1, dos botones de opción). Los campos X, Y, ancho, alto y rotación aceptan expresiones (`(10+2)*4+1`, constantes como pi, funciones como sin/sqrt; Enter aplica). X, Y, ancho y alto aceptan sufijos de unidad (`5in` en documento mm se convierte).
- **Casos borde:** en 2.1 las unidades de diseño y de control son separables pero están enlazadas por defecto, lo que sorprende al cambiar una; el ancla elegida cambia el resultado de un redimensionado. Hilo del foro pide mover un nodo a coordenadas exactas: no soportado en Edit Nodes (`F:138366`).
- **Fuentes:** `D:NumericEditsToolbar/`, `G:NewFeatures/NewIn2.1/`, `F:138366`.

#### F07 Zoom y paneo [P0]
- **Comportamiento:** rueda del ratón = zoom centrado en el cursor (dirección invertible en ajustes); trackpad con dos dedos; teclas `+`/`-` con el lienzo enfocado (con Ctrl/Cmd si no); Zoom to Page (Ctrl/Cmd+0); Frame Selection (Ctrl/Cmd+Shift+A; encuadra todo si no hay selección); paneo con botón central, tecla Espacio + arrastre, o herramienta mano; Esc sale del modo paneo. Atajos de teclado de zoom funcionan solo con el lienzo enfocado.
- **Casos borde:** el "enfoque" del lienzo es fuente de confusión (atajos que "no hacen nada"). Hacer clic en zona vacía lo recupera.
- **Fuentes:** `D:ZoomPan/`, `H:`.

#### F08 Estilo de vista [P1]
- **Comportamiento:** Filled muestra las capas Fill sólidas (oculta lo que esté detrás, puede esconder duplicados, más lento en equipos viejos); Wireframe muestra solo contornos (rápido, ve mejor los problemas). La calidad de borde suave/gruesa cambia fidelidad vs velocidad. Alt+Shift+W alterna. No cambia lo que el láser hace.
- **Fuentes:** `D:ViewStyle/`, `H:`.

#### F09 Menú contextual [P1]
- **Comportamiento:** clic derecho en el lienzo. Entradas confirmadas por distintas páginas: Edit Text Shape, Convert to Path, Lock/Unlock Selected Shapes, Apply Path to Text, Trace Image, Edit Bar Code (solo con un objeto seleccionado), opciones de arreglos virtuales (Edit/Flatten/Exit Array…), cortar/copiar/pegar. Lista completa: no verificado.
- **Fuentes:** `D:Text/`, `D:ConvertToPath/`, `D:LockShapes/`, `D:ApplyPathToText/`, `D:TraceImage/`, `D:BarCode/`, `D:GridArray/`, `D:Clipboard/`.

#### F10 Atajos de teclado por defecto [P0]
Resumen de los principales (Win/Linux; en macOS Ctrl→Cmd y Alt→Option). Fuente única: `H:`.

| Grupo | Atajos |
|---|---|
| Archivo | Ctrl+N nuevo, O abrir, I importar, S guardar, Shift+S guardar como, Alt+X exportar, Ctrl+P imprimir, Ctrl+Q salir |
| Edición | Ctrl+Z / Ctrl+Shift+Z deshacer/rehacer, A todo, Shift+I invertir selección, C/X/V, D duplicar, Alt+V pegar en sitio, Supr/Retroceso borrar, Ctrl+Shift+C a path, Alt+J auto-join, Alt+Shift+O optimizar, Alt+D borrar duplicados |
| Herramientas | Esc seleccionar; Ctrl+L líneas; Ctrl+R rectángulo; Ctrl+E elipse; Ctrl+`` ` `` editar nodos; Ctrl+K trim (solo en la barra Creation y el menú Tools, no en la tabla de atajos); Ctrl+Tab tabs; Ctrl+T texto; Alt+O offset; Ctrl+W weld; Alt+`+`/`-`/`*` unión/resta/intersección; Ctrl+B asistente booleano; Alt+B break apart; Alt+C (tabla) o Alt+Shift+C (menú) cortar formas; Alt+I ajustar imagen; Alt+T trace |
| Vista | Alt+P preview, Ctrl+`=`/`-` zoom, Ctrl+0 página, Alt+Shift+W alambre/relleno, Ctrl+Shift+A encuadrar selección |
| Selección y movimiento | Flechas mueven; Shift = paso grande; Ctrl = paso pequeño; Tab/Shift+Tab ciclan objetos |
| Globales con selección | Ctrl+G agrupar, Ctrl+U desagrupar, Ctrl+Shift+H/V voltear, Ctrl+Shift+M espejo sobre línea, Alt+flechas alinear, Alt+RePág/AvPág centrar, Alt+Shift+H/V move together, RePág/AvPág orden de dibujo (Ctrl = al extremo) |
| Solo con lienzo enfocado | `.` y `,` giro 90°, Ctrl+`.`/`,` 10°, Ctrl+Shift+`.`/`,` 5°, Shift+`>`/`<` 45°, Ctrl+2 rotación por dos puntos, L/R/T/B/C/E alinear, P centrar en página, V/H voltear |
| Edit Nodes (con el cursor encima) | S suave, L línea, C esquina, D borrar, I insertar, M punto medio, B romper, T recortar, E extender, A alinear |

- **Casos borde:** las teclas de una sola letra solo funcionan con el lienzo enfocado, y las letras de Edit Nodes cambian de significado según lo que hay bajo el cursor (D y S se usan dos veces). Conflictos detectados en la propia documentación (ver sección 0). Ctrl/Cmd se sobrecarga: nudge fino, invertir snap, quitar de selección.
- **Fuentes:** `H:`, `D:EditNodes/`, `D:TransformControls/`, `D:Selection/`.

#### F11 Editor de atajos [P1]
- **Comportamiento:** ventana en Archivo > Preferencias > Editar atajos. Categorías a la izquierda y buscador; se selecciona un comando, se escribe la nueva combinación; "Clear" quita el atajo; "Reset All" restaura; exportar/importar para respaldo; los atajos personalizados también viajan en un bundle. Existen PDF descargables con tablas por plataforma.
- **Fuentes:** `D:EditHotkeys/`.

#### F12 Preferencias de edición [P1]
- **Parámetros verificados:** Design Units (enlazables a unidades del dispositivo), espaciado visual de rejilla, contraste de rejilla, distancia de snap a rejilla (independiente del espaciado visual), distancia de snap a objeto (px), activar/desactivar snap a objetos y a rejilla (Ctrl/Cmd temporalmente los invierte), incrementos de nudge (flecha, Ctrl+flecha, Shift+flecha), tolerancia de clic en px, "volver a Select tras dibujar una forma", tamaño de cursores/nodos, calidad de curva, antialiasing, tema, tamaño de fuente e iconos, grupos de importación, estado por defecto de interruptores.
- **Casos borde:** el snap de rejilla puede no funcionar si el de objetos está apagado (bug en macOS M3 reproducido y corregido hacia 1.7.00 según el hilo `F:146107`).
- **Fuentes:** `D:SettingsPreferences/`, `D:Snapping/`, `F:146107`.

#### F13 Deshacer / rehacer [P0]
- **Comportamiento:** sin límite fijo; se vacía solo al cerrar o abrir proyecto nuevo. Atajos estándar, botones, menú. La documentación no dice qué acciones no son deshacibles.
- **Casos borde / quejas:** desde 1.4.00, mientras haya nodos seleccionados en Edit Nodes el deshacer queda bloqueado (cambio deliberado según el soporte); hay que deseleccionar con Esc, Espacio o clic en vacío; usuarios lo describen como impráctico con varios nodos y algunos volvieron a 1.3.01 (`F:99126`, `F:99888`). Otra petición: deshacer cambios de potencia/velocidad sin deshacer dibujo (`F:38177`, no leído). Hilo "Undo feature request" pide lista visible de pasos (`F:102868`), atendido con Undo History en 2.1. Sesiones largas consumen memoria; la pila de undo se limpia al abrir/crear proyecto (`F:185601`).
- **Fuentes:** `D:UndoRedo/`, `F:99126`, `F:99888`, `F:102868`, `F:185601`.

#### F15 Portapapeles básico [P0]
- **Comportamiento:** copiar y cortar guardan en el portapapeles del sistema (por defecto); pegar centra en la posición del cursor; **pegar en sitio** conserva la posición original del proyecto de origen; **duplicar** crea la copia exactamente sobre el original (de ahí los duplicados accidentales, ver F54); borrar elimina. Se accede desde menú Edit, barra principal, atajos y clic derecho.
- **Casos borde:** desactivar "Use System Clipboard" acelera pero impide copiar entre LightBurn y otras apps y entre ventanas de LightBurn.
- **Fuentes:** `D:Clipboard/`, `U:EditMenu/`.

#### F16 Portapapeles entre aplicaciones [P1]
- **Comportamiento:** se puede pegar texto, vectores e imágenes desde otro software o arrastrarlos. Los vectores de Illustrator a veces llegan como imagen (la documentación sugiere activar la opción de copiar como SVG en Illustrator o importar un SVG).
- **Casos borde / quejas:** en macOS, pegar desde Inkscape o Illustrator produce imagen en vez de vector, incluso tras actualizaciones recientes (2025); el portapapeles guarda varios formatos y LightBurn parece tomar el bitmap antes que el vectorial (`F:184747`, `F:177636`, `F:53131`, `F:177506`). Oportunidad: preferir siempre la representación vectorial (SVG/PDF) del portapapeles.
- **Fuentes:** `D:Clipboard/`, foros citados.

### 2.2 Selección y transformación

#### F20 Selección [P0]
- **Comportamiento:** la herramienta Select (Esc) es la activa por defecto. Clic selecciona (la tolerancia en px es configurable). Caja arrastrando a la derecha (rojo) = solo objetos totalmente dentro; hacia la izquierda (verde) = objetos dentro o tocados. Shift añade; Ctrl/Cmd alterna; Ctrl/Cmd+Shift quita; Esc o clic en vacío limpia; Tab y Shift+Tab ciclan objetos o grupos en orden de creación. Un grupo cuenta como un solo objeto. Contorno de selección animado discontinuo para objetos y punto-punto-raya para grupos.
- **Casos borde:** si la selección parpadea suele haber geometría duplicada o solapada (importar dos veces); solución: Delete Duplicates o desagrupar y borrar. Dificultad para seleccionar objetos ocultos bajo rellenos en vista Filled (ver F08).
- **Fuentes:** `D:Selection/`, `H:`.

#### F21 Selecciones especiales [P1]
- **Comportamiento (menú Edit):** Invertir selección (Ctrl+Shift+I); Seleccionar formas abiertas; abiertas en capas Fill u Offset Fill (porque no se pueden rellenar); todas las de la capa actual; formas contenidas dentro de la forma seleccionada (la contenedora sigue seleccionada, ignora agrupadas); formas menores que la seleccionada (saca coincidencias de sus grupos); círculos por tamaño exacto o rango (útil para agujeros de tornillos, nuevo en 2.1).
- **Fuentes:** `D:Selection/`, `U:EditMenu/`, `G:NewFeatures/NewIn2.1/`, `D:Grouping/`.

#### F22 Manejadores de transformación [P0]
- **Comportamiento:** aparecen sobre la caja mínima que contiene la selección. Mover: arrastrar el cuadrado central; Esc antes de soltar cancela; Shift restringe a 90°/45°; Alt muestra guías de alineación temporales; Ctrl/Cmd desactiva snap. Escalar: esquinas proporcional, puntos medios en un eje, el lado opuesto es ancla; Shift en esquina permite no proporcional; Ctrl/Cmd escala desde el centro. Rotar: cuatro manejadores curvos en las esquinas, gira alrededor del centro; Shift = pasos de 15°; Ctrl/Cmd = pasos de 5°; el ángulo se muestra en la barra de estado. Sesgar (shear): manejadores pequeños junto a las esquinas.
- **Casos borde:** arrastrar es menos preciso que escribir valores (la documentación lo recomienda); pivote personalizado solo con Two-Point Rotate/Scale (F25). Atención: Shift invierte su sentido según el gesto (en esquina, desbloquea la proporción en vez de bloquearla).
- **Fuentes:** `D:TransformControls/`, `H:`.

#### F23 Nudge con flechas [P0]
- **Comportamiento:** flechas mueven la selección 5 mm por defecto; Ctrl/Cmd = 1 mm; Shift = 20 mm; Ctrl/Cmd+Shift = 0,1 mm (todo configurable). Funciona solo con el lienzo enfocado. En Edit Nodes mueve el nodo/manejador seleccionado con el mismo incremento.
- **Fuentes:** `D:TransformControls/`, `D:EditNodes/`, `D:SettingsPreferences/`.

#### F24 Rotación rápida por teclado [P0]
- **Comportamiento:** giros de 90° (`.` horario, `,` antihorario), 45°, 10° y 5° con combinaciones; ángulos arbitrarios y negativos en Numeric Edits. Solo con lienzo enfocado.
- **Fuentes:** `H:`, `U:ArrangeMenu/`, `D:TransformControls/`.

#### F26 Voltear H/V [P0]
- **Comportamiento:** refleja la selección sobre el centro de la caja mínima que contiene a todos los objetos. Atajos V y H (con lienzo enfocado) o Ctrl/Cmd+Shift+H/V.
- **Casos borde:** el texto volteado sigue siendo texto; el texto aplicado a path se voltea con el path (ver F40). Ver inconsistencia de nombres en sección 0.
- **Fuentes:** `D:FlipMirror/`, `U:ArrangeMenu/`.

#### F27 Espejo sobre línea [P1]
- **Comportamiento:** crea una copia reflejada respecto a una línea elegida. Se seleccionan los objetos y por último la línea, que debe ser recta y de exactamente dos puntos. Sirve para dibujar la mitad de un diseño simétrico.
- **Fuentes:** `D:FlipMirror/`.

#### F28 Bloquear formas [P1]
- **Comportamiento:** impide mover, escalar, rotar y borrar. Se activa con la casilla Locked de Shape Properties, el menú Arrange o clic derecho. Un objeto bloqueado es la referencia de Align y de Make Same Width/Height. Distinto de los interruptores globales (F17).
- **Fuentes:** `D:LockShapes/`, `D:Align/`, `D:MakeSameWidthHeight/`.

#### F29 Orden de dibujo [P1]
- **Comportamiento:** por defecto lo más reciente queda arriba. Comandos: adelante un paso (AvPág), atrás un paso (RePág según tabla de atajos: la documentación de la página se contradice), al frente (Ctrl+RePág), al fondo (Ctrl+AvPág). Con varios objetos conservan su orden relativo. Útil cuando un vector queda tapado por una imagen. Solo visible en Wireframe; en Filled manda el orden de capas.
- **Fuentes:** `D:DrawOrder/`, `H:`.

#### F30 Agrupar / desagrupar [P0]
- **Comportamiento:** Ctrl/Cmd+G une objetos de cualquier tipo o capa (líneas, formas, texto, imágenes), incluso grupos existentes (anidados; hay que desagrupar nivel por nivel). Un grupo conserva posiciones relativas al mover/escalar y las booleanas lo tratan como una unidad. Los objetos importados se agrupan automáticamente (ajuste "Group imported shapes").
- **Herramientas que NO operan sobre grupos:** Edit Nodes, Trim, Resize Slots, Convert to Path, Auto-Join, Close Path, Close with Tolerance, Break Apart, Optimize, Select Contained. En Cut Shapes el cortador no puede estar agrupado; Apply Path to Text exige desagrupar.
- **Casos borde:** si algún objeto del grupo es abierto, todo el grupo queda fuera de las booleanas; "Select Open Shapes" y "Select Smaller" extraen coincidencias del grupo. Esta restricción masiva genera confusión (botón gris sin explicación). Oportunidad: mostrar tooltip con el motivo de deshabilitado.
- **Fuentes:** `D:Grouping/`, `U:ArrangeMenu/`.

### 2.3 Herramientas de creación

#### F32 Dibujar líneas / Bézier [P0]
- **Comportamiento:** crea paths de nodos unidos por segmentos rectos o curvas Bézier cúbicas con manejadores. Cada clic coloca un nodo esquina y añade un segmento recto. Para terminar: clic en el nodo inicial (cierra la forma), clic derecho o Esc (queda abierta). Curvas por dos métodos: (a) suavizado automático: tecla S cambia a nodos suaves y la curva siguiente toma dirección y fuerza de la posición del siguiente nodo, no queda fijada hasta colocarlo; (b) controlado: clic y arrastre sobre el nodo saca los manejadores, se suelta y se hace clic en el siguiente.
- **Modificadores:** Shift restringe nodos y manejadores a horizontal, vertical o 45°; Ctrl/Cmd evita la unión automática con otros paths abiertos; Alt/Option amplía la distancia de snap; S alterna esquina/suave.
- **Snapping:** a líneas existentes, esquinas, puntos medios, intersecciones y nodos; el cursor cambia para indicar el tipo. Hacer clic en el inicio o fin de un path abierto lo extiende; si una línea nueva conecta los extremos de dos paths abiertos, se fusionan automáticamente.
- **Casos borde:** la unión automática ocurre sin querer al conectar paths abiertos (hay que mantener Ctrl/Cmd para evitarla). La barra de estado muestra dx, dy, longitud y ángulo. Sin panel de opciones dedicado: todo va por teclas y modificadores.
- **Fuentes:** `D:DrawLines/`, `D:Snapping/`, `U:CreationToolbar/`.

#### F33 Rectángulo y elipse [P0]
- **Comportamiento:** forma primitiva paramétrica (conserva ancho, alto, radio). Clic y arrastre desde la esquina; Ctrl/Cmd = crece desde el centro; Shift = cuadrado o círculo; ambos pueden combinarse. La herramienta sigue activa tras dibujar (una preferencia puede volver a Select). Radio de esquina (solo rectángulos): con Ctrl/Cmd aparece un manejador azul; arrastrarlo redondea las cuatro esquinas; arrastrar en vertical da un "mordisco" cóncavo (radio negativo); el radio se mantiene fijo al redimensionar; también por campo numérico. Atajos Ctrl/Cmd+R y Ctrl/Cmd+E.
- **Casos borde:** para editar nodos hay que convertir a path, lo cual elimina para siempre ancho/alto paramétricos y radio. Trim Shapes y Radius también convierten a path.
- **Fuentes:** `D:PrimaryShapes/`, `D:ShapeProperties/`, `D:RadiusFillet/`.

#### F34 Polígono, estrella y afines [P1]
- **Comportamiento:** menú Draw Shapes: triángulo, pentágono, polígono (hexágono por defecto, lados ajustables), octágono, estrella, estrella de doble radio. Triángulo, pentágono y octágono arrancan con su cantidad de lados, pero cualquier polígono puede cambiar de lados después (campo en Shape Properties o Ctrl/Cmd + arrastrar el manejador morado). Estrella: Points (cantidad; en doble radio el total es el doble), Bulge 0 a 1 (curva cada punta de la punta a la base), Ratio 0 a 1 (longitud y ángulo de las puntas); la de doble radio añade Ratio2.
- **Fuentes:** `D:PrimaryShapes/`, `D:ShapeProperties/`, `U:ToolsMenu/`.

#### F35 Ventana Shape Properties [P1]
- **Comportamiento:** oculta por defecto (menú Window). Campos para todos los objetos: Cut Order Priority (menor = antes, solo efectivo si "Order by Priority" está activo en optimización), Power Scale (% de potencia por forma) y Locked. Por tipo: Imagen (gamma, contraste, brillo, enhance con radio/cantidad/denoise), Elipse (ancho/alto), Polígono (ancho/alto/lados), Rectángulo (ancho/alto/radio, negativo = muescas), Texto (Max Width, Squeeze, Ignore Empty Vars). Path: solo los comunes. Con selección de tipos distintos solo aparecen los tres comunes; editar con varias formas aplica a todas.
- **Fuentes:** `D:ShapeProperties/`, `D:VariableText/`.

#### F36 Crear / editar texto [P0]
- **Comportamiento:** herramienta Create Text (Ctrl/Cmd+T): clic en el lienzo y escribir; Enter = nueva línea; Esc termina el objeto. Por defecto el punto de clic es el centro del texto (cambia con Align X/Y). Editar: clic de nuevo con la herramienta activa, doble clic, o menú contextual "Edit Text Shape" (una preferencia hace que el doble clic abra un diálogo). Las fuentes salen del sistema operativo; instalar una fuente nueva exige reiniciar. Al guardar, se incrusta una copia en forma de path del texto.
- **Casos borde / quejas:** abrir un proyecto en otro equipo sin la fuente muestra advertencia; el archivo se corta bien (usa el path incrustado) pero el texto no se puede editar hasta instalar la fuente (`F:15780`, `F:157492`, `F:190416`). Convert to Path hace el texto vector permanente. Los textos importados de SVG/AI necesitan la fuente instalada; no hay fuentes incrustadas (`D:FileManagement/`). Negrita o cursiva solo existen si la familia las tiene como archivos de fuente; en macOS se ha reportado que no aparecen estilos o no funciona la negrita (`F:168320`, `F:104773`).
- **Fuentes:** `D:Text/`, `F:15780`, `F:157492`, `F:190416`, `F:168320`.

#### F37 Opciones de texto [P0]
- **Parámetros (barra Text Options, aparece solo con texto seleccionado):** Font; Height (aproximada, varía por fuente); HSpace (% de espaciado entre caracteres, negativo aprieta); VSpace (% de interlineado sobre la altura); Align X (izquierda, derecha, centro); Align Y (abajo, arriba, medio; arriba y medio son útiles con Apply Path to Text); Bold; Italic; Upper Case; Right-to-Left (solo si se habilita en ajustes; se activa solo si el idioma de entrada es RTL); Welded; Distort; Text Mode y Offset (texto variable).
- **Globales:** fuente y altura por defecto (reinicio para aplicar a nuevos proyectos), RTL, doble clic abre diálogo.
- **Casos borde:** kerning y selección de glifos vienen del motor de fuentes; diferencias con Illustrator en pares de letras en fuentes script (`F:76059`). Oportunidad: shaping correcto con rustybuzz y OpenType features.
- **Fuentes:** `D:Text/`, `F:76059`.

#### F38 Texto soldado (Welded) [P1]
- **Comportamiento:** fusiona los caracteres que se solapan en una sola silueta, importante para fuentes script cuyos trazos se cruzan (si no, el área solapada sale como hueco por la regla par-impar del relleno). También se suelda de forma implícita al reducir HSpace hasta que las letras se solapan.
- **Casos borde:** las letras deben tocarse para soldarse; no todas las fuentes script tienen colas que se solapan, a veces hace falta editar nodos; pares de letras mal soldados por kerning (`F:76059`, `F:166650`, `F:123848`, `F:122938`). Soldar manualmente con Weld Shapes tras Convert to Path es el recurso.
- **Fuentes:** `D:Text/`, `F:76059`, `F:166650`.

#### F39 Texto curvado (Bend) [P1]
- **Comportamiento:** el texto seleccionado muestra un manejador azul; arrastrar hacia arriba/abajo curva el texto sobre un círculo; doble clic en el manejador quita la curva. Distort decide si cada letra se deforma con la curva. Para curvas complejas se usa F40.
- **Fuentes:** `D:Text/`.

#### F40 Aplicar path a texto [P1]
- **Comportamiento:** con exactamente un texto y un path vectorial seleccionados (Tools > Apply Path to Text o clic derecho), el texto se mueve al path y adopta su contorno; no se puede mover de forma independiente, pero sus atributos siguen editables; el path sigue editable y el texto se actualiza; borrar el path devuelve el texto a su orientación original. Align X (posición a lo largo del path: centro, izquierda = inicio del texto en el inicio del path, derecha) y Align Y (centro, abajo = texto sobre el path, arriba = texto colgando bajo el path). El path puede asignarse a una capa de herramienta para que no se corte.
- **Casos borde / quejas:** la dirección del texto la fija la dirección en que se dibujó el path (el primer clic con Draw Lines es el inicio); si queda al revés hay que volver a dibujarlo, rotarlo, o voltear el texto antes de aplicar. No hay herramienta para invertir un path ni se puede espejar texto sobre path sin convertir a path (`F:162591`, `F:185323`, `F:106227`, `F:50256`). Se comporta distinto en paths abiertos y cerrados (círculos) (`F:64859`). Si el comando está gris, el "path" son varios trozos desconectados (unir con Auto-Join); no funciona con grupos. Oportunidad: botón "invertir dirección" y control numérico de desplazamiento a lo largo del path (`F:122249`).
- **Fuentes:** `D:ApplyPathToText/`, foros citados.

#### F42 Texto variable [P1]
- **Comportamiento:** expresiones especiales que se sustituyen al enviar/iniciar, en Preview, al guardar archivos de máquina y al exportar SVG/DXF/AI. Cuatro modos (menú Text Mode): Fecha/Hora, Merge/CSV, Serial Number, Cut Setting. Ventana Variable Text: Start, End, Advance By, Current; Browse/Clear para CSV; Auto-Advance (en cada Start/Send/Save); Previous/Next/Reset; botón Test (mantener); Bake (convierte a texto estático; Shift+clic solo selección). Offset por objeto para producir lotes (4 etiquetas con offsets 0 a 3 y avance 4). Shape Properties: Max Width, Squeeze, Ignore Empty Vars.
- **Sintaxis verificada:** texto entre comillas simples sale literal; Fecha: d dd ddd dddd, M MM MMM MMMM, yy yyyy, h hh H HH, m mm s ss, z zzz, AP/ap, t (zona); Serie: `d` decimal, `h`/`H` hex, `0` inicial rellena con ceros; CSV: `%0`, `%1`… columna, fila 0-indexada; Cut Setting: `C03` capa, `s` velocidad, `p`/`m` potencia, `n` pasadas, etc.
- **Casos borde:** si Start ≠ 0 hay que pulsar Reset antes de la primera corrida; CSV debe ser UTF-8; en arreglos virtuales Test no muestra el resultado (usar Preview); Auto-Increment solo con CSV o serie; Bake no vuelve a incrementar.
- **Fuentes:** `D:VariableText/`, `D:VariableText/VariableTextFormatting/`, `D:GridArray/`, `D:CircularArray/`.

#### F43 Códigos de barras y QR [P1]
- **Comportamiento:** herramienta Create Bar Code; tipo por defecto QR (o el último usado). Datos en Raw Content; QR tiene pestañas WIFI y Contact que rellenan el contenido. Contador de caracteres restantes; Text Evaluation (variable text) con preview de "Evaluated Text" y Variable Offset; Process Control Characters (`<RS>`, `<GS>` para GS1); Error Correction (deslizador); Show Bar Code Text; Extra Data Spacing (EAN/ISBN/UPC con `+`); Force Square (Data Matrix). Simbologías: lineales (2 of 5, CodaBar, Code 128, Code 93, EAN, ISBN, Plessey, UPC A/E, VIN), 2D (Aztec, Chinese Sensible, Data Matrix, Grid Matrix, PDF417, QR, Micro QR…), marcadores AprilTag/ArUco (identificador 0 a 1000).
- **Casos borde:** datos inválidos muestran una X y mensaje con los caracteres ofensores; reabrir con doble clic o "Edit Bar Code"; Cancel al crear borra el código; Convert to Path pierde el contenido editable.
- **Fuentes:** `D:BarCode/`, `D:ConvertToPath/`.

#### F44 Medir [P1]
- **Comportamiento:** hover sobre una forma la resalta en verde y el segmento bajo el cursor en morado; ventana de medición con nodos/líneas/curvas, perímetro, área (cerradas), inicio/fin/diferencia del segmento, ángulo, y para arcos verdaderos radio y centro; mm/in. Arrastrar entre dos puntos crea línea de medida temporal con snap; Shift restringe a 0/45/90. Durante la creación de formas la barra de estado muestra dimensiones. Sirve también para detectar abierto/cerrado.
- **Fuentes:** `D:Measure/`, `G:Explainers/OpenClosedShapes/`.

#### F45 Tabs / puentes [P1]
- **Comportamiento:** secciones sin cortar que mantienen la pieza unida al material. Herramienta Add Tabs (Ctrl/Cmd+Tab): clic en una forma en capa de modo Línea; cada tab es un punto rojo arrastrable (incluso a otra forma), doble clic o Shift+clic lo quita; ancho por defecto 0,5 mm; añadir uno manual activa Tabs en esa capa.
- **Parámetros (en el editor de capa):** Tabs/Bridges on/off, Generación Manual/Automática, Tab Size (longitud total por forma), Even Spacing (en el inicio de cada path y repite por intervalo), Limit Max Tabs, Tabs Per Shape (cantidad fija equiespaciada), Tab Cut Power (pulsa en vez de saltar), Skip Inner Shapes (solo automáticos, solo cerradas), Clear Tabs.
- **Casos borde:** no disponible en modo principiante. La salida real (saltar el tramo) pertenece al pipeline de generación de G-code, pero el modelo de datos (posición paramétrica sobre el path) es parte del diseño.
- **Fuentes:** `D:AddTabs/`, `D:BeginnerMode/`.

### 2.4 Edición de paths

#### F48 Editar nodos [P0]
- **Comportamiento:** edita nodos (puntos ancla), segmentos rectos y curvas con manejadores. Se activa con Ctrl/Cmd+`` ` ``; los objetos seleccionados muestran sus nodos al instante. Una barra propia de Edit Nodes ofrece modos, y además hay atajos de letra que actúan sobre lo que está bajo el cursor.
- **Ratón:** arrastrar nodos, manejadores o curvas; clic en un nodo/manejador lo selecciona y las flechas lo mueven (incremento de nudge); Shift+clic suma a la multiselección y se mueven juntos; arrastrar una línea recta la dobla en curva; arrastrar un extremo abierto sobre otro los une en un solo path.
- **Operaciones con tecla (cursor encima):** D borrar nodo (une los vecinos) o borrar línea (abre o parte el path); I insertar nodo; M insertar en el punto medio; C suave→esquina (manejadores independientes); S esquina→suave (manejadores comparten ángulo) o línea→curva suave; B romper el path en el nodo; L curva→línea; A alinear (gira la selección para que el segmento caiga en el horizontal/vertical/45° más cercano); T recortar hasta la siguiente intersección; E extender un extremo abierto hasta la intersección más cercana.
- **Modificadores:** Shift al mover restringe a 90°/45° con guías temporales; con varios nodos, solo el último seleccionado se restringe y el resto lo sigue. Snapping a puntos de otros objetos.
- **Restricciones:** solo paths; texto, primitivas y códigos de barras deben convertirse; los grupos no se editan.
- **Casos borde / quejas:** deshacer bloqueado con nodos seleccionados (ver F13); hay peticiones de selección con lazo/caja de varios nodos y de mover un nodo a coordenadas X/Y exactas, aún no soportado (`F:11363`, `F:31908`, `F:138366`, `F:168115`); herramienta Radius con errores en esquinas de tres nodos (`F:99888`).
- **Fuentes:** `D:EditNodes/`, `H:`, foros citados.

#### F49 Trim Shapes [P1]
- **Comportamiento:** tijeras (Ctrl/Cmd+K). Al pasar sobre un objeto, el tramo que se eliminaría se resalta en rojo y el cursor cambia; el clic lo borra. Si la forma no cruza ninguna otra línea, se resalta y borra completa. Cuando es posible, las piezas restantes se auto-unen en las intersecciones formando un solo path, que queda seleccionado.
- **Modificadores:** Ctrl/Cmd+clic desactiva la auto-unión; Shift+clic conserva una copia del tramo borrado en la capa actualmente seleccionada (advierte si es la misma capa).
- **Casos borde:** las primitivas se convierten a path; los grupos no se pueden recortar. Es una herramienta que depende de un cálculo robusto de intersecciones (tolerancias).
- **Fuentes:** `D:TrimShapes/`, `D:EditNodes/`.

#### F50 Convertir a path [P0]
- **Comportamiento:** Ctrl/Cmd+Shift+C o menú Edit/clic derecho. Convierte rectángulos, elipses, polígonos, texto (cada letra queda como forma separada y movible) y códigos de barras en paths. No funciona con grupos.
- **Se pierde:** ancho/alto/radio/lados, Max Width del texto, propiedades de la barra de texto, sustitución de texto variable y contenido del código de barras.
- **Fuentes:** `D:ConvertToPath/`.

#### F51 Auto-Join [P0]
- **Comportamiento:** busca segmentos abiertos cuyos extremos coinciden (o casi) pero no están enlazados formalmente, y los fusiona en un path continuo. Tolerancia 0,05 mm; Alt/Option+J sobre la selección. Típico para arreglar importaciones con formas abiertas.
- **Casos borde:** si los extremos están fuera de tolerancia no se une; recomendación del fabricante: Break Apart primero y luego Auto-Join. No se documenta cómo desempata cuando varios extremos están cerca ni si el resultado se marca cerrado (no verificado). No opera sobre grupos.
- **Quejas:** las formas abiertas son la causa nº 1 de que Fill, Offset y booleanas fallen (`F:93714`, `F:143272`).
- **Fuentes:** `D:AutoJoinSelectedShapes/`, `G:Explainers/OpenClosedShapes/`.

#### F52 y F53 Close Path / cerrar con tolerancia [P1]
- **Close Path:** añade una línea entre inicio y fin de un path si distan ≤0,5 mm. Cada path debe estar ya conectado en todo su largo; no une dos paths separados.
- **Cerrar con tolerancia:** diálogo con deslizador de umbral; modos "Move Ends Together" (junta los extremos existentes) o "Join with Line" (añade segmento); estado: cuántas abiertas se encontraron, cerraron y quedan. OK aplica, Cancel descarta.
- **Fuentes:** `D:ClosePath/`, `D:CloseSelectedPathsWithTolerance/`, `G:Explainers/OpenClosedShapes/`.

#### F54 Borrar duplicados [P1]
- **Comportamiento:** elimina objetos idénticos en tamaño, forma y posición, sin necesidad de seleccionarlos; deben estar desagrupados; Alt/Option+D. No documenta tolerancia (parece exigir identidad exacta).
- **Casos borde:** los duplicados hacen que los rellenos parezcan solo contorno en el lienzo, desaparezcan en Preview, que Line haga pasadas extra y que Fill falle.
- **Fuentes:** `D:DeleteDuplicates/`, `G:Troubleshooting/ImportedExtraLines/`, `D:Selection/`.

#### F55 Break Apart [P1]
- **Comportamiento:** divide los paths seleccionados en sus líneas y curvas individuales (un rectángulo pasa a 4 líneas con 8 extremos). Alt/Option+B. No opera sobre grupos.
- **Uso típico:** preparar un Auto-Join que no funcionó.
- **Fuentes:** `D:BreakApart/`, `D:AutoJoinSelectedShapes/`.

#### F56 Optimizar formas [P1]
- **Comportamiento:** ventana con interruptores y deslizadores de tolerancia de error y vista previa en vivo: suavizar las fuentes (alisa trazos ondulados, p. ej. de un trace), ajustar a líneas (reemplaza tramos casi rectos por líneas), ajustar a arcos (convierte segmentos en arcos, recupera círculos exportados como muchos segmentos cortos). Estadísticas de puntos, líneas y curvas antes y después. Alt/Option+Shift+O. Primitivas y texto deben convertirse antes; no funciona con grupos.
- **Casos borde / quejas:** archivos con millones de nodos (mapas, DXF comprados) ralentizan el programa y pueden hacer que la máquina vaya a tirones; Optimize ayuda, pero llevar los deslizadores al extremo distorsiona la forma; un caso reducía 2 MB a 181 KB; otro usuario solo lo resolvió rasterizando y trazando de nuevo (`F:191055`, `F:53497`, `F:84298`). Oportunidad: simplificador con garantía de error máximo (Hausdorff) visible, ajuste a arcos G2/G1 sin deformar.
- **Fuentes:** `D:OptimizeSelectedShapes/`, `F:191055`.

#### F57 Dirección y punto de inicio [P1]
- **Comportamiento:** Set Start Point (barra Modifiers): se activa y se hace clic en un nodo; el inicio definido por el usuario se muestra con una flecha azul (el predeterminado gris); Shift+clic invierte el sentido; Ctrl/Cmd+clic restablece. Edit > Reverse Shape Direction invierte la dirección desde el punto inicial.
- **Casos borde:** si no hay nodo donde se quiere empezar hay que insertarlo con Edit Nodes; las opciones de optimización "elegir mejor punto/dirección" pueden anular la elección; no afecta a Fill ni Imagen (ahí se usa ángulo de escaneo).
- **Fuentes:** `D:SetStartPoint/`, `U:EditMenu/`.

#### F60 Radio / Fillet [P1]
- **Comportamiento:** redondea una esquina viva, hace un mordisco cóncavo con valor negativo, o revierte un redondeo a esquina viva. Se elige la herramienta en la barra Modifiers, se escribe el radio, se selecciona el objeto y se hace clic en cada esquina; Esc sale. El cursor indica si se puede añadir/quitar fillet o si no hay espacio. Rectángulos tienen además campo de radio paramétrico independiente.
- **Casos borde:** exige que las dos líneas adyacentes sean suficientemente largas y estén unidas por un nodo; quitar el redondeo de arcos exportados como muchos segmentos puede fallar (usar Optimize para recuperar el arco); no opera en grupos.
- **Fuentes:** `D:RadiusFillet/`, `F:99888`.

### 2.5 Modificar, combinar y organizar

#### F61 Offset de formas [P0]
- **Comportamiento:** crea un nuevo contorno a distancia fija de las formas seleccionadas (paths, primitivas, texto, grupos; varios objetos a la vez). Vista previa en vivo; OK crea, Cancel descarta; Ctrl/Cmd+clic en el botón reutiliza los últimos ajustes sin abrir el diálogo. Atajo Alt/Option+O.
- **Parámetros:** Offset Distance (campo, flechas, rueda); Direction (Outward, Inward, Both); Corner Style (Round, Bevel, Corner; solo afecta esquinas que apuntan en la dirección del offset); Outer shapes only (ignora formas anidadas, útil para cortar el contorno de un grabado complejo); Select resulting objects; Delete original objects; Optimize/Simplify results (reduce nodos, útil con trazados).
- **Casos borde:** un offset interior solo funciona en formas cerradas; un offset de 0 sirve para reparar vectores raros; duplicados en direcciones opuestas generan doble offset; solapes en esquinas cóncavas se recortan; formas "casi cerradas" con huecos mínimos fallan (`F:93714`, `F:78319`, `F:114525`). Se usa también para compensar el kerf de corte.
- **Fuentes:** `D:OffsetShapes/`, `F:93714`, `F:78319`.

#### F62 Booleanas: unión, resta, intersección [P0]
- **Comportamiento:** operan sobre exactamente dos objetos (dos formas, dos grupos, o una forma y un grupo). Todos los objetos deben ser cerrados y vectoriales (imágenes inhabilitan). **El orden de selección importa en la resta:** el segundo seleccionado se resta del primero y desaparece. Agrupar varias formas permite tratarlas como una (anillo = círculo exterior + interior). Atajos: Alt+`+` unión, Alt+`-` resta, Alt+`*` intersección.
- **Casos borde / quejas:** botones grises sin explicación cuando hay más de dos objetos, formas abiertas o imágenes; formas que "parecen tocarse" pero tienen holguras minúsculas hacen fallar la unión (solución: solapar un poco, o juntar con docking/padding 0 o snap); las tolerancias internas dejan micro-solapes; en AI/SVG con compuestos, el resultado de la unión difiere (`F:143272`, `F:133319`, `F:82266`, `F:44318`, `F:49155`). Oportunidad: aceptar N operandos, mostrar el motivo de deshabilitado y tolerancia configurable.
- **Fuentes:** `D:BooleanTools/`, `F:143272`.

#### F63 Weld [P0]
- **Comportamiento:** equivale a la unión pero acepta cualquier número de objetos a la vez (Ctrl/Cmd+W). Importante para fusionar letras y para eliminar solapes.
- **Casos borde:** si una forma queda totalmente dentro de otra, Weld puede eliminar el contenido interior (ejemplo: texto dentro de un anillo); solución del fabricante: agrupar por separado el borde y el contenido y aplicar Boolean Union a los dos grupos. Foros: soldar texto con una forma hace desaparecer el texto (`F:65917`, `F:49155`). (Una respuesta resumida de búsqueda afirmaba que Weld "conserva la intersección"; contradice la documentación, no se usa aquí: no verificado.)
- **Fuentes:** `D:BooleanTools/`, `U:ToolsMenu/`, foros citados.

#### F65 Cut Shapes [P1]
- **Comportamiento:** la última forma añadida a la selección actúa como cortador; cada otra forma se divide donde la cruza. El cortador se consume. Los resultados quedan en dos grupos (dentro y fuera del cortador). En capas Fill/Offset Fill las piezas se cierran con una línea; en Line/Tool el hueco queda abierto. Con imágenes crea dos copias enmascaradas y duplica el cortador a capas Tool. Útil para dividir diseños grandes.
- **Requisitos:** cortador cerrado y sin agrupar; para conservarlo hay que duplicarlo antes. Atajo Alt+Shift+C o Alt+C (documentación inconsistente).
- **Fuentes:** `D:CutShapes/`.

#### F66 Grid Array [P0]
- **Comportamiento:** copia la selección en filas y columnas.
- **Parámetros:** X Columns / Y Rows; Total Width/Height como alternativa (calcula cantidades sin exceder); X/Y Spacing con modo "centro a centro" o "separación entre bordes", por eje; X Column Shift / Y Row Shift (desplaza alternas); Reverse Direction por eje; Shift by Half (patrón de ladrillo); Mirror Alternate Columns/Rows en X o Y (ahorra material); Random Orientation con semilla reproducible; Auto-Increment Variable Text; Create Virtual Array (F67); Total Size and Count (incluye el original); Group Results; Select Results.
- **Casos borde:** Total Width/Height cambian solos a menos que se editen directamente (revisarlos si se apunta a un tamaño).
- **Fuentes:** `D:GridArray/`.

#### F68 Circular Array [P1]
- **Parámetros:** Copies, Center of Rotation (X/Y de la cama o "usar el último objeto seleccionado"), Start y End (ángulo parcial), Step (relacionado con End y Copies: editar uno cambia los otros), Rotate object copies (gira cada copia hacia el centro como marcas de reloj; apagado mantiene orientación, como números de reloj), Auto-Increment, Group Results, Select Results.
- **Fuentes:** `D:CircularArray/`.

#### F72 Alinear [P0]
- **Comportamiento:** bordes (arriba, abajo, izquierda, derecha) y centros (vertical, horizontal, punto central). Un grupo cuenta como un objeto. **Referencia:** por defecto el último objeto añadido a la selección; Ctrl+clic doble sobre un objeto lo convierte en referencia; si se seleccionan varios a la vez desde cero, el más alto en orden de dibujo; si hay uno bloqueado, es la referencia; si hay varios bloqueados, se quedan quietos.
- **Atajos:** Alt+flechas (globales) y L/R/T/B/C/E (con lienzo enfocado). En modo principiante, un asistente con vista previa.
- **Fuentes:** `D:Align/`, `H:`, `D:BeginnerMode/`.

#### F73 Distribuir [P1]
- **Comportamiento:** iguala los huecos entre bordes (V-/H-Spaced) o las distancias entre centros (V-/H-Centered): promedia el espaciado actual y recoloca.
- **Casos borde:** no garantiza huecos positivos; con objetos de tamaños muy distintos distribuidos por centro pueden solaparse.
- **Fuentes:** `D:Distribute/`.

#### F75 Mover selección a posición [P1]
- **Comportamiento:** lleva la selección como bloque a esquinas, bordes (centrada en el eje perpendicular) o centro del área de trabajo (tecla P con lienzo enfocado); en gantry, a la última posición reportada del cabezal (requiere conexión; Ctrl/Cmd mueve el láser en vez de los objetos).
- **Fuentes:** `D:MoveSelectedObjects/`.

#### F77 Snapping [P0]
- **Comportamiento:** puntos de snap a objetos: nodo (o esquina de imagen), punto medio, centro, intersección y cualquier punto de una línea; más snap a esquinas de la rejilla (distancia de snap propia, no la visual). El cursor cambia según el tipo. Funciona al mover, al dibujar líneas (otro clic coloca el nodo) y al crear primitivas (soltar termina). Para mover un objeto desde un punto de snap, se arrastra desde él (para varios objetos, seleccionarlos o agruparlos antes). Marca de cuatro flechas en el centro de lo seleccionado.
- **Modificadores:** Alt/Option = snap más "pegajoso" y guías de alineación; Ctrl/Cmd mantenido invierte el snap (por defecto lo apaga; si se desactivó en ajustes, lo enciende).
- **Casos borde / quejas:** el snap es de "preferencia fuerte", no un bloqueo; depende del zoom (`F:34681`, `F:120751`, `F:94334`); en macOS el snap a rejilla dependía del snap a objetos (corregido hacia 1.7.00 según desarrolladores, `F:146107`); usuarios no encuentran cómo mover libremente (`F:34681`). Con Wireframe hay que seleccionar antes de poder pasar sobre el centro.
- **Fuentes:** `D:Snapping/`, `D:DrawLines/`, `D:TransformControls/`, `F:146107`.

### 2.6 Imagen y trazado

#### F79 Trace Image [P1]
- **Comportamiento:** convierte una imagen importada en contornos vectoriales; funciona mejor con alto contraste (logotipos, siluetas, dibujos, escritura a mano). Se abre con Alt/Option+T, menú Tools o clic derecho. Vista previa en líneas moradas que se actualiza al cambiar parámetros, con zoom y paneo; doble clic en el botón central resetea la vista. El resultado es un **grupo** de vectores (desagrupar para editar formas sueltas) que puede quedar oculto debajo de la imagen original (mover la imagen o reordenar).
- **Parámetros:** Cutoff y Threshold (rango de brillo, por defecto 0 a 128); Ignore Less Than (descarta regiones de pocos píxeles); Optimize (reduce nodos; 0 sin optimizar, defecto 0,2); Smoothness (0,0 solo líneas, 1,33 solo curvas); Trace Transparency (usa el canal alfa); Sketch Trace (bordes por contraste local, para fotos de hojas con luz desigual); Delete Image After Trace; Fade Image; Boundary / Clear Boundary (limita el trazado a una región); Show Points.
- **Casos borde / quejas:** esquinas vivas y texto pequeño salen mal; fotos varían mucho; resultados ásperos o miles de formas diminutas en imágenes complejas (`F:165946`, `F:186211`, `F:44540`); no hay trazado de línea central (centerline); el motor es Potrace licenciado, sin modo centerline (`F:34828`, `F:185142`); algunas trazas generan cientos o miles de nodos por forma sin forma sencilla de simplificar (`F:13842`, `F:191055`). Alternativa del fabricante: modo Threshold de imagen para grabar la original. Oportunidades: centerline, preprocesado (contraste/gamma) integrado en la propia ventana, simplificación con tolerancia.
- **Fuentes:** `D:TraceImage/`, `F:34828`, `F:185142`, `F:165946`.

### 2.7 Importación y exportación

#### F91 Opciones de importación [P0]
- **Comportamiento / parámetros (pestaña Import/Export de preferencias):** DPI de SVG (96 = defecto de Inkscape; 72 = defecto de Illustrator; si el tamaño sale mal, probar el otro); DXF: detectar unidades si es posible, unidad manual cuando el archivo no la trae, tolerancia de auto-cierre en mm (distancia máxima para unir nodos en un path cerrado), ignorar objetos de texto; HPGL/PLT: precisión; generales: agrupar formas importadas, importar capas ocultas de AI, seleccionar lo importado, centrar en el lienzo, permitir importar a capas de herramienta. En salida: tolerancia de curva (0,05 mm por defecto) y ancho de línea de impresión. Un changelog posterior menciona detección automática del DPI para archivos de Illustrator/Inkscape (puede estar en versiones recientes; no verificado).
- **Casos borde:** ver F83, F84. Importar un `.lbrn/.lbrn2` con Import trae solo gráficos; con Open trae también los ajustes de corte.
- **Fuentes:** `D:SettingsPreferences/`, `D:FileManagement/`, `G:NewFeatures/NewIn2.1/`.

#### F83 Importar SVG [P0]
- **Comportamiento:** importa trazados vectoriales; texto con fuente instalada (sin fuentes incrustadas); agrupa lo importado por defecto; 2.1 amplía el soporte de coordenadas `tspan`. Los colores se mapean a capas.
- **Casos borde / quejas:** tamaño desviado por DPI 72 vs 96 (factor 1,333) (`F:91373`, `F:46787`); diferencias de décimas de mm por el cuadro delimitador visual vs geométrico de Inkscape que incluye el trazo (`F:156583`); objetos con trazo (stroke) sin convertir entran como línea; rellenos compuestos con la regla par-impar (huecos por nodos y anidación, no por dirección) (`F:190892`); clip-paths/máscaras no soportados (`F:126687`); formas apiladas o enmascaradas en el diseño original llegan como líneas extra (`G:Troubleshooting/ImportedExtraLines/`); SVG generados por IA que no cumplen el estándar fallan. Oportunidad: respetar viewBox/unidades físicas y mostrar al usuario un diálogo de verificación de escala; soportar clipPath/mask aplicando booleanas.
- **Fuentes:** `D:FileManagement/`, `G:GetStarted/CreatingAndImportingArtwork/`, `D:SettingsPreferences/`, foros citados.

#### F84 Importar DXF [P0]
- **Comportamiento:** importa entidades geométricas; las unidades pueden no estar en el archivo (si se importa en mm algo hecho en pulgadas queda 25,4 veces menor). Auto-detección de unidades, selección manual, tolerancia de auto-cierre, ignorar texto.
- **Casos borde / quejas:** múltiples hilos de DXF con tamaño erróneo (Fusion 360, AutoCAD, micras como unidad); la auto-detección a veces falla y conviene apagarla; hay que reescalar con una dimensión conocida y proporción bloqueada (`F:112974`, `F:27025`, `F:128070`, `F:138200`, `F:5271`); DXF con nodos de más en rectas tras importar (`F:191055`); no se pudo verificar manejo de splines/elipses/bloques/capas DXF (no verificado). Oportunidad: mostrar bounding box con unidad sugerida y diálogo "¿ese tamaño es razonable?"; mapear capas DXF a capas de color.
- **Fuentes:** `D:FileManagement/`, `D:SettingsPreferences/`, foros citados.

#### F85 / F86 Importar AI y PDF [P1]
- **Comportamiento:** importa vectores de AI y PDF; capas ocultas de AI opcionales; 2.1 maneja más compresión de PDF. Recomendación de la propia documentación: convertir texto a contornos antes de importar.
- **Casos borde / quejas:** máscaras de recorte y rellenos de patrón no soportados, elementos desconocidos se ignoran en silencio; pérdida de líneas; máscara de imagen PDF puede invertir la imagen; solapes con regla par-impar parecen formas faltantes (arreglo: Weld o quitar solapes); alternativa: guardar como PNG si solo se graba (`F:110270`, `F:35545`, `F:97481`, `F:126687`, `F:102515`, `F:130034`, `F:146600`). Oportunidad: avisar qué elementos se ignoraron al importar (hoy es silencioso).
- **Fuentes:** `D:FileManagement/`, `G:NewFeatures/NewIn2.1/`, foros citados.

#### F87 Importar imágenes [P0]
- **Comportamiento:** BMP, JPG, GIF, TIF/TIFF, TGA, WEBP (el menú); la guía de inicio lista PNG, JPG, BMP, GIF, TIFF; el objeto imagen conserva archivo fuente para Refresh/Replace (F82). Abrir (en vez de importar) una imagen reemplaza el proyecto actual.
- **Fuentes:** `U:FileMenu/`, `D:FileManagement/`.

#### F92 / F93 Exportar SVG y DXF [P1]
- **Comportamiento:** Export (Alt/Option+X) exporta la selección o todo el proyecto como AI, SVG o DXF. SVG incluye capas con Output desactivado pero ocultas (2.1). DXF: versiones AC1009 y AC1021 (2.1); opción de exportar arcos como arcos o como segmentos cortos.
- **Fuentes:** `D:FileManagement/`, `G:NewFeatures/NewIn2.1/`, `D:SettingsPreferences/`.
- **Queja:** `F:55589` (tamaño de DXF exportado incorrecto; no leído en detalle).

#### F96 Proyecto nativo y plantillas [P0]
- **Comportamiento (LightBurn):** Save guarda `.lbrn2` por defecto (`.lbrn` para versiones muy antiguas); Save As Template escribe `.lbt` que conserva todos los ajustes de capa, no solo los usados, y al abrirlo se crea un proyecto sin nombre (la plantilla no se sobrescribe). No guardar en carpetas sincronizadas (Dropbox, Drive, OneDrive) por riesgo de corrupción. Si el proyecto guardó otro dispositivo, ofrece cambiar de dispositivo al abrir.
- **Para nuestro proyecto:** definir formato propio versionado y documentado (serde/JSON o ZIP+XML). Guardar en nuestro formato es P0; leer el de LightBurn es F97.
- **Fuentes:** `D:FileManagement/`.

#### F97 Importar `.lbrn` / `.lbrn2` [P1]
- **Hechos verificados:** ambos son XML; `.lbrn2` es compacto y menos legible que `.lbrn`; el formato no está documentado oficialmente, aunque el soporte oficial dice que no es difícil de leer y ofreció ayudar con el elemento de transformación (`xform`); existen datos de path con segmentos línea (L) o Bézier (B) y manejadores c0/c1; existe un conversor comunitario MIT (`lbrn2-to-svg`) que parsea formas, imágenes y contornos de texto.
- **Estado legal:** la estructura se puede deducir observando archivos propios y salidas públicas; no se revisó licencia/estabilidad (no verificado).
- **Fuentes:** `F:56221`, `F:52174`, `F:42317`, https://unpkg.com/lbrn2-to-svg@0.5.3/README.md

#### F100 Arrastrar y soltar [P1]
- **Comportamiento:** arrastrar archivos de imagen/vector/proyecto dentro de la ventana los importa; también arrastrar desde otras apps; un `.lbzip` arrastrado importa un bundle.
- **Fuentes:** `D:FileManagement/`, `D:Clipboard/`, `U:FileMenu/`.

---

## 3. Funciones P2 (resumen de comportamiento)

Para completitud del inventario; sin sección detallada por ser avanzadas. Todas verificadas en las fuentes de la tabla 1.

- F14 Undo History (2.1): `D:UndoRedo/`, `G:NewFeatures/NewIn2.1/`.
- F17 Transform Control Toggles: `D:TransformControls/`. F18 Beginner Mode: `D:BeginnerMode/`.
- F19 New Window / Show Notes / Plantillas: `U:FileMenu/`.
- F25 Two-Point Rotate/Scale: `D:TwoPointRotateScale/`. F31 Auto-Group: `D:Grouping/`.
- F41 Font Manager y SHX: `D:Text/`. F46 Tangent Circle: `G:NewFeatures/NewIn2.1/`. F47 Art Library: `D:ArtLibrary/`.
- F58 Warp (4 puntos) / Deform (16 puntos, Shift/Alt para simetría, doble clic reinicia un manejador): `D:WarpDeform/`.
- F59 Rubber band outline (contorno más ajustado, va a la última capa elegida): `D:CreateRubberBandOutline/`.
- F64 Boolean Assistant (Ctrl/Cmd+B): `D:BooleanTools/`. F67 Virtual Arrays y su menú contextual: `D:GridArray/`.
- F69 Copy Along Path (espaciado por número o por distancia, rotar y escalar progresivo): `D:CopyAlongPath/`.
- F70 Make Same Width/Height: `D:MakeSameWidthHeight/`. F71 Resize Slots: `D:ResizeSlots/` (solo esquinas vivas, sin ranuras internas, sin grupos).
- F74 Move Together y Dock (padding, como grupo, bloquear internos): `D:Dock/`, `D:Distribute/`.
- F76 Nest: Quick Nest (interno, requiere 2.1, empaqueta rectángulos mínimos) y Nest Selected vía SVGnest externo (exporta `LightBurnNest.svg`, manual, no exporta partes dentro de otro gráfico): `D:NestSelected/`, `G:NewFeatures/NewIn2.1/`.
- F78 Guías automáticas y arrastrables: `D:AutomaticGuidelines/`. Quedan en T1; si el origen es posición actual/usuario pueden alterar el cálculo del origen.
- F80 Multi-file Trace (Labs): `U:ToolsMenu/`. F81 Apply Mask / Crop Image: `D:ApplyMaskToImage/`. F82 Convert to Bitmap, Save Processed Bitmap, Refresh/Replace Image: `U:EditMenu/`, `D:ImageOptions/`.
- F88 a F90 PLT/HPGL, G-code, otros: `U:FileMenu/`, `D:FileManagement/` (la lista completa de extensiones aceptadas es: .lbrn2, .lbrn, .mage, .bmp, .jpg, .gif, .tif/.tiff, .tga, .webp, .ai, .pdf, .dxf, .nc, .hpgl, .svg, .gc, .gcode, .plt, .rd, .sc, .scpro2; el menú lista ".pg", probablemente errata de .png: inferido).
- F94 a F95 Export AI e imágenes. F98 Imprimir. F99 Preferencias y bundles `.lbprefs`/`.lbzip`: `U:FileMenu/`.

---

## 4. Quejas frecuentes de usuarios en el foro (oportunidades)

Todas provienen de hilos leídos o de resúmenes de búsqueda indicados; los hilos cerrados automáticamente pueden estar desactualizados. No se contó frecuencia, solo recurrencia en resultados.

1. **Formas abiertas invisibles:** Offset, Fill, booleanas y Cut Shapes fallan porque un hueco mínimo deja el path abierto; el usuario no lo ve. Oportunidad: indicador visual permanente de abierto/cerrado, diagnóstico "cerrar con tolerancia" al fallar la operación (`F:93714`, `F:143272`; `G:Explainers/OpenClosedShapes/`).
2. **Botones deshabilitados sin explicación:** booleanas, Apply Path to Text, Cut Shapes, Optimize, Edit Nodes se vuelven grises por grupos, formas abiertas, selección de más de dos objetos, etc. Oportunidad: tooltip de motivo y atajo "arreglar" (`F:143272`; `D:Grouping/`).
3. **Boolean solo con dos operandos y orden de selección implícito:** la resta depende de cuál se seleccionó segundo. Oportunidad: N operandos, "A menos B" explícito, vista previa siempre (`D:BooleanTools/`).
4. **Weld/Union se llevan el contenido interior** (texto dentro de un anillo) o no unen shapes que solo se tocan (`D:BooleanTools/`, `F:65917`, `F:49155`, `F:143272`).
5. **Importación con escala errónea:** SVG 72 vs 96 DPI, bounding box visual de Inkscape, DXF sin unidades (25,4×), micras. Oportunidad: asistente de verificación de tamaño al importar (`F:91373`, `F:156583`, `F:112974`, `F:27025`).
6. **Importación silenciosa de lo no soportado:** máscaras de recorte de AI/SVG/PDF, rellenos de patrón, máscaras de imagen PDF invertidas; formas extra por enmascarados (`F:110270`, `F:126687`, `F:97481`, `F:102515`; `G:Troubleshooting/ImportedExtraLines/`).
7. **Regla par-impar del relleno:** solapes se ven como huecos; el usuario espera "no-cero" como en Inkscape/Illustrator (`F:190892`).
8. **Copiar/pegar desde Inkscape/Illustrator llega como imagen, sobre todo en macOS** (`F:184747`, `F:177636`, `F:53131`).
9. **Texto:** fuentes faltantes en otro equipo, no hay fuentes incrustadas ni forma de sustituir; estilos negrita/cursiva inconsistentes en macOS; soldado y kerning distintos a Illustrator en fuentes script (`F:157492`, `F:190416`, `F:168320`, `F:76059`).
10. **Texto sobre path:** dirección dictada por el sentido de dibujo del path, no se puede invertir ni espejar sin convertir; comportamiento distinto en cerrado/abierto; falta de control de desplazamiento a lo largo del path (`F:162591`, `F:185323`, `F:106227`, `F:122249`, `F:64859`).
11. **Edit Nodes:** deshacer bloqueado con nodos seleccionados, sin selección por lazo o caja de nodos, sin mover un nodo a coordenadas exactas, herramienta Radius con errores en esquinas de 3 nodos (`F:99126`, `F:99888`, `F:31908`, `F:138366`, `F:168115`). Nota: un resumen de búsqueda indica que la multiselección con Shift sí existe desde 0.9.10.
12. **Trace Image:** resultados ásperos, demasiadas formas pequeñas, sin centerline, sin simplificar nodos; parte del problema es preprocesado de la imagen (`F:165946`, `F:186211`, `F:34828`, `F:13842`; el motor Potrace licenciado se cita en resúmenes de búsqueda, no verificado).
13. **Rendimiento con archivos densos:** decenas de millones de nodos, importaciones de 30 a 45 s, lentitud tras sesiones largas, carga de ~10 min con archivos de 15 a 18 MB (`F:53497`, `F:84298`, `F:52578`, `F:185601`). Oportunidad: estructura espacial para hit-test y render con nivel de detalle.
14. **Snapping poco predecible:** "preferencia fuerte" no bloqueo, dependiente del zoom; la tecla Ctrl/Cmd hace de nudge fino, de quitar selección y de invertir el snap; el snap de rejilla requería snap de objetos activo en macOS (`F:34681`, `F:120751`, `F:146107`).
15. **Atajos que no responden por foco del lienzo y letras de una tecla sensibles al contexto** (D y S con dos significados en Edit Nodes); documentación con atajos contradictorios (`D:ZoomPan/`, `D:EditNodes/`, sección 0).
16. **Exportación DXF/SVG con tamaño erróneo y sin garantías de unidades** (`F:55589`).
17. **Panel de layout:** no se documenta guardar layouts con nombre; Shape Properties y Variable Text escondidos detrás de otra pestaña (`G:Guides/CustomizingTheLightBurnWindow/`).
18. **Pegar a otro lado:** Duplicar crea copias exactamente encimadas y los duplicados degradan relleno y cortes; no hay aviso inmediato (`D:DeleteDuplicates/`).

---

## 5. Notas para la implementación (no son comportamiento de LightBurn)

### 5.1 Las tres funciones más difíciles de replicar

1. **Edit Nodes + Trim + Auto-Join + Close (robustez geométrica de paths):** exigen intersecciones y uniones con tolerancias consistentes (0,05 mm y 0,5 mm), manipulación interactiva de Bézier con nodos suaves/esquina, y comportamiento coherente con deshacer. La experiencia de LightBurn se rompe justo en esos bordes (holguras mínimas, duplicados).
2. **Booleanas + Offset + Weld + texto soldado sobre Bézier:** requieren polígonos con aproximación controlada de curvas (aplanar y volver a ajustar curvas), reglas de relleno par-impar vs no-cero, estabilidad ante geometría degenerada y offset con estilos de esquina. Hay bibliotecas sólidas (i_overlay, clipper2) pero trabajan sobre polígonos; recuperar curvas limpias es el reto.
3. **Importadores fieles (SVG/DXF/AI/PDF) con unidades y capas:** cada formato tiene decenas de rarezas (clip-paths, bloques, splines, unidades ausentes, texto sin fuentes incrustadas). Es la fuente más grande de quejas y de soporte.

Menciones: texto (shaping, fuentes, kerning, texto sobre path con edición posterior), trazado de imágenes de calidad comparable, docking de ventanas.

### 5.2 Referencias OSS por familia (no verificadas en esta sesión)

- Geometría de curvas: `kurbo`; teselado/render: `lyon`, `vello`; booleanas y offset de polígonos: `i_overlay`, `clipper2`, `geo`; offset con arcos: `cavalier_contours`.
- SVG: `usvg`/`resvg`; DXF: `dxf`; PDF: `lopdf`, `pdfium-render`.
- Texto: `rustybuzz`, `ttf-parser`, `fontdb`, `cosmic-text`, `swash`.
- Trazado: `vtracer`/`visioncortex`, bindings de potrace.
- Nesting: `jagua-rs`, SVGnest (JS, referencia).
- Aplicaciones de referencia (comportamiento, no copiar código): Inkscape (nodos, booleanas, trazado), MeerK40t (operaciones láser en Python), Rayforge.

---

## Apéndice A. Hilos del foro citados (`F:NNNN`)

Todos bajo `https://forum.lightburnsoftware.com/t/<slug>/NNNN`. Se leyeron completos los marcados con asterisco (*); el resto solo se vio vía resultados de búsqueda (títulos y fragmentos).

- 143272* Boolean union or weld shapes - neither works as expected: `.../boolean-union-or-weld-shapes-neither-works-as-expected/143272`
- 156583* Inkscape SVG Import scaling issue: `.../inkscape-svg-import-scaling-issue/156583`
- 191055* Too Many Nodes?: `.../too-many-nodes/191055`
- 76059* Font letter joining / welding different between Illustrator and LightBurn: `.../font-letter-joining-welding-different-between-illustrator-and-lightburn/76059`
- 99126* LightBurn 1.4.00 undo of node editing not working correctly: `.../lightburn-1-4-00-undo-of-node-editing-not-working-correctly-after-update/99126`
- 99888* Node editor bug and radius tool issues: `.../node-editor-bug-and-radius-tool-issues/99888`
- 112974* DXF import not sizing correctly: `.../dxf-import-not-sizing-correctly/112974`
- 56221* Is LightBurn File Format (lbrn2) documented or open sourced?: `.../is-lightburn-file-format-lbrn2-documented-or-open-sourced/56221`
- Solo búsqueda: 44318 weld shapes imported; 65917 problem welding letters and shapes; 49155 welding text to an imported shape; 133319 boolean union not working on custom SVG; 82266 using weld or boolean; 102868 undo feature request; 38177 undo and speeds; 185601 lag after long sessions; 11363, 31908 multiple node selection; 138366 move selected node to coords; 168115 request additional edit nodes movements; 91373, 46787 SVG scale; 27025, 128070, 138200, 5271 DXF scale; 55589 DXF export size; 110270, 35545, 97481, 126687, 102515, 130034, 146600 AI/PDF/SVG masks; 190892 SVG compound paths and even-odd; 184747, 177636, 53131, 177506 paste as image; 162591, 185323, 106227, 50256, 64859, 122249 text on path; 165946, 186211, 44540, 34828, 185142, 13842 trace; 53497, 84298, 52578 performance; 93714, 78319, 114525 offset; 146107, 34681, 120751, 94334 snapping; 15780, 157492, 190416, 168320, 104773 fonts; 166650, 123848, 122938 welding letters; 52174, 42317 lbrn2 format; 186538 y 181573 SVG pequeños.

Slugs completos disponibles vía `https://forum.lightburnsoftware.com/t/NNNN` (el foro redirige por número, no verificado).

## Apéndice B. Documentación oficial consultada

Raíz `https://docs.lightburnsoftware.com/2.1/`: Reference/UI/{Workspace, CreationToolbar, MainToolbar, ModifiersToolbar, ModesToolbar, ColorPalette, StatusBar, ArrangeToolbar, ArrangeMenu, EditMenu, ToolsMenu, FileMenu, WindowMenu}; Reference/{DrawLines, PrimaryShapes, Text, VariableText, VariableText/VariableTextFormatting, ShapeProperties, EditNodes, BooleanTools, OffsetShapes, Align, Distribute, GridArray, CircularArray, Grouping, FlipMirror, Snapping, AutomaticGuidelines, Selection, TransformControls, NumericEditsToolbar, TraceImage, UndoRedo, EditHotkeys, ConvertToPath, AutoJoinSelectedShapes, ClosePath, CloseSelectedPathsWithTolerance, DeleteDuplicates, BreakApart, OptimizeSelectedShapes, TrimShapes, ApplyPathToText, RadiusFillet, CopyAlongPath, CutShapes, Dock, TwoPointRotateScale, WarpDeform, NestSelected, FileManagement, ZoomPan, ViewStyle, Clipboard, MoveSelectedObjects, DrawOrder, LockShapes, AddTabs, BarCode, CreateRubberBandOutline, SettingsPreferences, BeginnerMode, MakeSameWidthHeight, Measure, ResizeSlots, ArtLibrary, SetStartPoint, ImageOptions, ApplyMaskToImage}; GetStarted/CreatingAndImportingArtwork; Guides/CustomizingTheLightBurnWindow; Explainers/OpenClosedShapes; Troubleshooting/ImportedExtraLines; NewFeatures/NewIn2.1; tabla de atajos `https://docs.lightburnsoftware.com/Hotkeys`.

No se pudieron cargar: NewFeatures/NewIn2.0 (404) y UI/ArrangeToolbars (la URL correcta es ArrangeToolbar). Las versiones 2.0/1.7 de la documentación solo se vieron a través de resultados de búsqueda (p. ej. `.../2.0/Reference/Text/`, `.../1.7/Reference/ApplyPathToText/`); no se contrastaron diferencias entre versiones salvo las listadas en "What's New 2.1".
