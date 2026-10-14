# Plan hasta el software completo

Meta: las 214 funciones del inventario (`docs/spec/01` a `03`) más lo que necesita un usuario real
(`docs/spec/04-cobertura.md`, "Necesidades fuera del inventario"), **con o sin máquinas para
probar**.

Reglas:

- Todo lo que dependa de hardware se construye completo y se valida contra emuladores propios
  (`tlaku-emu`, el emulador de Ruida, y los que se agreguen para K40 y galvo). Queda marcado como
  "por validar con máquina real" en la interfaz y en `docs/spec/04-cobertura.md`, sin bloquear el
  avance.
- Cada ola termina con todas las pruebas en verde, revisión visual con capturas y un commit.
- Al final de cada ola se actualiza `04-cobertura.md`.

Estado de partida (2026-10-13): 88 hechas, 31 parciales, 6 solo en motor o CLI, 89 faltantes.

Estado tras la ola 1 (2026-10-13, `04-cobertura.md` actualizado): **119 hechas, 25 parciales, 7 solo en motor o CLI, 63 faltantes**. P0: 60 de 68 hechas (parciales C32, C51, M3, M5, M6, M11, M16; solo motor M15). Con el cierre de la ola 1 (`.lbrn` habilitado en la app) y contando las filas de la tabla, el punto de partida de la ola 2 fue 120 hechas, 27 parciales, 6 solo en motor o CLI y 61 faltantes.

Estado tras la ola 2 (2026-10-13, `04-cobertura.md` actualizado): **143 hechas (67 %), 22 parciales, 3 solo en motor o CLI, 46 faltantes** de 214. P0: 62 de 68 hechas (parciales C51, M3, M5, M6, M11, M16; ninguna P0 solo en el motor ni faltante). Por área: diseño y edición 69 hechas, capas, corte e imagen 54 y máquinas y avanzadas 20.

Estado tras la ola 3 (2026-10-13, `04-cobertura.md` actualizado): **165 hechas (77 %), 23 parciales, 3 solo en motor o CLI, 23 faltantes** de 214. P0 sin cambios: 62 de 68 hechas (parciales C51, M3, M5, M6, M11, M16). Por área: diseño y edición 89 hechas, capas, corte e imagen 55 y máquinas y avanzadas 21.

Estado tras la ola 4 (2026-10-13, `04-cobertura.md` actualizado y recontado con un script): **173 hechas (81 %), 26 parciales, 0 solo en motor o CLI, 15 faltantes** de 214. P0: 64 de 68 hechas (parciales C51, M3, M5, M6). Por área: diseño y edición 90 hechas, capas, corte e imagen 56 y máquinas y avanzadas 27.

## Ola 1: base para publicar

P0 pendientes y lo que más frena a quien viene de LightBurn.

**Terminada el 2026-10-13** (commit `736d472`, "Ola 1: base para publicar"). Entró la edición básica (voltear, espejo, giros, orden, bloqueo, grupos anidados, selecciones especiales, igualar, medir, menú contextual, polígono y estrella, Bézier, snapping completo, interruptores y portapapeles SVG), preferencias, atajos configurables y autoguardado, exportar SVG, DXF, PNG y PDF e imprimir, importar PDF, AI, HPGL, G-code y más imágenes con opciones, fuentes de una línea, texto curvado y soldado, QR y códigos de barras, y el manual de 17 páginas. Se subieron 31 funciones a "hecho" respecto al inicio. Cobertura: 119 de 214 hechas (56 %), 60 de 68 P0.

**Quedó pendiente de la ola** (verificado al actualizar la cobertura): el importador de `.lbrn` y `.lbrn2` (D97) existe en el motor pero la app no acepta las extensiones (`IMPORT_EXTENSIONS` en `apps/desktop/src/features/history/paths.ts`), así que figura como "Solo motor/CLI"; las unidades en pulgadas solo se aplican en Medir y en Preferencias (D12 parcial); las formas paramétricas no conservan sus parámetros al guardar; los enlaces del repositorio y de donaciones del menú Ayuda siguen sin publicar. *Resuelto después:* los proyectos de LightBurn ya se abren desde la app (cierre de la ola 1, `79eb1c8`) y las formas paramétricas ya guardan sus parámetros (ola 2).

| Bloque | Funciones |
|---|---|
| Edición básica | Voltear H/V (D26), espejo sobre línea (D27), rotación rápida (D24), rotación y escala por dos puntos (D25), orden de dibujo (D29), bloquear (D28), agrupar completo (D30), selecciones especiales (D21), igualar ancho y alto (D70), medir (D44), menú contextual (D09), polígono y estrella (D34, D35), portapapeles entre apps (D16), dibujo de Bézier completo (D32), snapping completo (D77) |
| Aplicación | Autoguardado y recuperación, preferencias (D12, M51, D99), editor de atajos (D11), archivos recientes, interruptores de transformación (D17) |
| Formatos | Exportar SVG, DXF e imágenes (D92, D93, D95); importar PDF y AI (D85, D86), HPGL (D88), G-code (D89, M22), WebP, TIFF, GIF y TGA (D87); opciones de importación (D91); imprimir o PDF (D98) |
| Migración | Importar `.lbrn` y `.lbrn2` con capas y ajustes (D97); motor y app hechos |
| Texto y códigos | Fuentes de una línea (Hershey y SHX, D41), texto curvado (D39), texto soldado (D38), QR y códigos de barras (D43) |
| Ayuda | Menú Ayuda, manual de usuario, README actualizado |

Después de la ola: **idioma inglés** (M52), con todas las claves nuevas ya existentes.

## Ola 2: capas y corte

**Terminada el 2026-10-13** (commit `ec9cdb4`, "Ola 2: capas y corte"). Se subieron 23 funciones a "hecho" (de 120 a 143 de 214; P0 de 60 a 62 de 68) y D18 pasó de falta a parcial.

Qué entró, verificado en el código:

- **Trazos de las capas de línea** (`crates/tlaku-pipeline/src/stroke.rs`): perforación, entrada y salida recta o en arco, sobrecorte, pausas al empezar y al terminar, corte pasante, modo punto, rampa de potencia y escala de potencia por objeto; pausas con `Move::Dwell` (`G4`, con cambio temporal a M3 en potencia dinámica). Las combinaciones incompatibles se resuelven con un aviso.
- **Orden**: operaciones adicionales (sub-capas), prioridad y grupo por objeto en el planificador, ajustes de optimización (respetar capas, grupos y prioridad, interiores primero, mejor punto de inicio, sentido, reducir viajes, quitar tramos repetidos, esconder el juego mecánico; también como banderas del CLI), incremento de ángulo por pasada y cortar solo la selección con su origen.
- **Relleno e imagen**: relleno por regiones (`tlaku-fill/src/flood.rs`), agrupación por forma y por grupo, imagen sin tramar, realce de bordes y quitar motas.
- **Interfaz de capas**: secciones avanzadas, operaciones adicionales, copiar y pegar ajustes, borrar capa con sus objetos, valores por defecto propios y modo principiante.
- **Trabajo y control**: diálogo de optimización, encuadre de contorno, posicionar el láser, ajustes de velocidad y potencia en vivo, pasos configurables, posición final y checklist de inicio configurable.
- **Documento**: las formas paramétricas guardan sus parámetros en el proyecto y cada objeto puede llevar prioridad y escala de potencia.

**Quedó pendiente de la ola** (verificado al actualizar la cobertura): los puentes manuales (C14, D45) y la dirección y el punto de inicio manuales (D57) no entraron; la potencia mínima de una capa de línea (C08) tiene campo en el editor pero el motor solo la usa en imágenes; los valores por defecto son un solo juego para todas las capas (C35, parcial); Adjust Image sin presets (C51, parcial); el encuadre no se repite ni va con láser encendido (M11) y la ventana Mover sigue sin Z, posiciones guardadas, Fire ni Focus (M16); el importador de LightBurn no traduce los ajustes nuevos. Detalle y discrepancias entre el mensaje del commit y el código en [04-cobertura.md](spec/04-cobertura.md). *Resuelto después, en la ola 3:* los puentes manuales (queda la potencia de puente), la dirección y el punto de inicio manuales, la decisión sobre la potencia mínima en líneas (el campo se oculta y se explica) y el importador de LightBurn con los ajustes avanzados.

Plan original de la ola:

| Bloque | Funciones |
|---|---|
| Capas | Sub-capas (C30), copiar y pegar ajustes y seleccionar objetos de una capa (C06), ajustes de capa por defecto (C35), potencia mínima (C08), modo principiante (C60, D18) |
| Corte | Perforación (C13), lead-in y lead-out (C15), overcut (C16), pausa al inicio o fin y corte pasante (C17), modo punto (C18), rampa (C20), puentes manuales (C14, D45) |
| Relleno e imagen | Flood fill (C28), agrupación del relleno (C27), pass-through (C48), incremento de ángulo (C50), Adjust Image completo con enhance, denoise y presets (C51) |
| Orden | Prioridad por objeto y escala de potencia (C31), ajustes de optimización (C32, M49), quitar líneas solapadas en el envío (C33), ocultar backlash (C34), dirección y punto de inicio (D57) |
| Trabajo | Cortar solo la selección y origen de la selección (M15), encuadre de banda elástica (M12), posicionar el láser con clic (M17), ajuste de velocidad y potencia en vivo (M18), checklist del trabajo (M53) |

## Ola 3: diseño avanzado

**Terminada el 2026-10-13** (commit `b78790e`, "Ola 3: diseño avanzado"). Se subieron 22 funciones a "hecho" (de 143 a 165 de 214) y 3 pasaron de falta a parcial (D19, D58, D60); los P0 no cambiaron (62 de 68). Los conteos salen de contar las filas de `04-cobertura.md`.

Qué entró, verificado en el código:

- **Operaciones de forma** (`crates/tlaku-geom`, `edit/shape_ops.rs`): recortar con herramienta del lienzo (`X`), separar subtrayectos, optimizar nodos, redondear esquinas (redondeada, chaflán y mordisco), cortar formas con una cuchilla y redimensionar ranuras de encastre.
- **Diseño avanzado** (`edit/design.rs`, `features/arrange/`): deformar por envolvente de cuatro esquinas y cuatro bordes, contorno de banda elástica, círculo tangente, asistente booleano, copiar a lo largo de un trazo, arreglo virtual (expandido en `place.rs` y con fantasmas en el lienzo), acoplar y agrupar automáticamente por contención.
- **Nesting real** (`crates/tlaku-nest`): polígonos de no-ajuste con suma de Minkowski, colocación por gravedad, recocido simulado con tope de tiempo, contenedor irregular y piezas en huecos; comando con progreso y cancelación, optimizado también en el perfil dev.
- **Texto variable**: serie, CSV, fecha y hora, con avance tras el trabajo y una copia por fila.
- **Lienzo y proyecto**: guías, puentes manuales y punto de inicio y sentido (respetados por el planificador), notas, plantillas, nueva ventana, pestañas reordenables y la biblioteca de arte de 41 dibujos.
- **Imagen**: máscara con forma, convertir a mapa de bits, guardar la imagen procesada, reemplazar la imagen y vectorizar en lote.
- **Pendientes de la ola 2 que se cerraron**: la potencia mínima se oculta en las capas de línea con una explicación (decisión, no código nuevo en el motor), el editor valida igual que el motor y el importador de LightBurn trae los ajustes avanzados y las sub-capas.

Cobertura nueva: D31, D42, D46, D47, D49, D55, D56, D57, D59, D64, D65, D67, D69, D71, D74, D76, D78, D80, D81, D82, M38 y C08 pasaron a Hecho; D19, D58 y D60 a Parcial (y D04 sigue Parcial con las pestañas reordenables).

**Pendientes de la ola que pasan a otras olas** (verificados al actualizar la cobertura):

- *Ola 4*: la **potencia de puente** (C14 y D45, parciales), junto con los ajustes de potencia de la salida de máquina. *Resuelto en la ola 4:* la potencia de puente entró y C14 y D45 pasaron a Hecho.
- *Ola 6*: los **paneles flotantes** (D04), las **manijas de deformar en el lienzo** y la **malla 4×4** (D58), la herramienta de clic por esquina del redondeo y revertirlo (D60), las **guías temporales** de alineación (D78) y probar **Nueva ventana** en una ventana real: por lectura del código, la copia de recuperación de una ventana se ofrecería en la otra (D19).
- El **tiling** (M39) sigue en la ola 5.

Plan original de la ola:

Recortar formas (D49), separar (D55), optimizar formas (D56), deformar (D58), contorno de banda
elástica (D59), redondear esquinas (D60), asistente booleano (D64), cortar formas (D65), arreglo
virtual (D67), copiar a lo largo de un trazo (D69), redimensionar ranuras (D71), mover juntos y
acoplar (D74), nesting real (D76), guías (D78), vectorizar varios archivos (D80), máscara de imagen
(D81), convertir a mapa de bits (D82), círculo tangente (D46), biblioteca de arte (D47),
agrupación automática (D31), nueva ventana, notas y plantillas (D19), barras acoplables (D04),
texto variable con números de serie y CSV (D42, M38).

## Ola 4: máquinas

**Terminada el 2026-10-13** (commit `10c435d`, "Ola 4: máquinas"). Se subieron 8 funciones a "hecho" (de 165 a 173 de 214; P0 de 62 a 64 de 68), 6 pasaron de falta a parcial y 2 de solo motor/CLI a parcial. Los conteos salen de contar las filas de `04-cobertura.md`. Todo lo de esta ola se validó con emuladores, máquinas virtuales e imágenes sintéticas: **nada se probó con una máquina real**, y Ruida, el K40, el ojo de pez, la cámara en el cabezal y el buscador de centro están marcados experimentales o por validar con hardware.

Qué entró, verificado en el código:

- **Ruida en la app (experimental)**: conexión activa `ActiveMachine`/`Machine` (GRBL o Ruida), conexión UDP con sondeo de solo lectura y magic automático, máquina virtual `virtual:ruida`, subir el trabajo guardándolo en la memoria (por defecto) o ejecutándolo, estado, pausa, reanudar, detener, saltos, encuadre, información de la controladora y lista de documentos en solo lectura; `export_rd` pasó al pipeline.
- **K40 / M2 Nano (experimental)**: crate `tlaku-lhymicro` con codificador LHYMICRO-GL, CRC, velocidades por placa contrastadas con 1511 vectores de MeerK40t, USB con `nusb`, placa simulada y sesión (pausa, reanudar, detener, home), y `tlaku k40 list/status/send/export-egv`. **Todavía no está en la app**; la integración está en curso. <!-- K40-APP -->
- **Red y conexión**: GRBL por TCP y WebSocket (`tlaku_grbl::net`), tipo de conexión en el perfil con "Probar conexión", máquina virtual por red (`virtualnet:`) y automatización por UDP local (apagada de fábrica, `START` pide confirmación).
- **Control**: macros (hasta 10 por perfil), Mover con Z y U, posiciones guardadas, Enfocar, disparo de prueba con doble confirmación, ajustes `$$` con respaldo y restauración, encuadre a baja potencia y en bucle, láser 2, eje U, paquetes `.tlakubundle` y potencia en puentes.
- **Cámara**: modelo de ojo de pez con elección automática por validación cruzada, captura y vectorizado de la cama, cámara en el cabezal con calibración de desfase y mosaico, y buscador de centro.
- **Correcciones**: recuperación entre ventanas, copias del arreglo virtual en las cajas, nombres legibles en el nesting y pulgadas en la interfaz (barra de estado, reglas, pasos y diálogos de longitud).

Cobertura nueva: M11, M16, M20, M31, M50, M54, D45 y C14 pasaron a Hecho. M4, M24, M28, M32, M36 y M48 pasaron de Falta a Parcial, y M23 y M26 de Solo motor/CLI a Parcial. M3, M5 y M6 siguen Parciales (el perfil ganó macros, posiciones, eje U, láser 2 y red, pero faltan los ajustes que cada fila enumera). El K40 no tiene fila en el inventario: figura en la sección 2.4 de la cobertura como Solo motor/CLI hasta que esté en la app. <!-- K40-APP -->

**Pendientes de la ola que pasan a otras olas** (verificados al actualizar la cobertura):

- *Ola 5*: el ajuste de U en los trabajos y el láser 2 con Ruida (M32); el autoenfoque y optimizar Z (M31), junto con los ajustes de controlador (C59).
- *Ola 6*: lo que falta de los ajustes de dispositivo (M3, M5, M6: marcar por defecto, ordenar, ícono, offset del puntero rojo, unidades de dispositivo y de control, intercambio X/Y, auto-home, DTR y modo síncrono); las cámaras de red y los presets (M35); la alineación automática de la cámara del cabezal (M36); las cámaras en los paquetes de usuario (M4, D99); las pulgadas en las capas, las máquinas y la cámara (D12); y la lista de validación con hardware real de todo lo experimental, que incluye escribir los ajustes de Ruida (M25), borrar o ejecutar sus documentos (M24) y probar el K40.
- **Sin fecha, pendiente de esta ola:** la integración del K40 en la app (tipo de máquina, conexión USB, panel y diálogo de inicio con el aviso de que la potencia va con la perilla). <!-- K40-APP -->

Plan original de la ola:

| Bloque | Funciones |
|---|---|
| Ruida en la app | Perfil con IP, envío a la memoria (M23), lista de archivos del controlador (M24), ajustes de máquina (M25), información del controlador (M26), rotativo DSP (M28) |
| K40 | Driver M2 Nano (LHYMICRO-GL) con emulador, experimental |
| Red y conexión | GRBL por TCP y WebSocket (M50), automatización por UDP (M54) |
| Control | Macros (M20), ventana Mover completa (M16), Z y enfoque (M31), eje U y láser 2 (M32), ajustes de dispositivo completos (M3, M5, M6), encuadre completo (M11), paquetes de usuario `.lbzip` (M4), potencia de puente (C14, D45; viene de la ola 3) |
| Cámara | Overlay, vectorizar desde la cámara, captura (M35), cámara en el cabezal (M36), modelo ojo de pez, buscador de centro (M48) |

## Ola 5: pro y galvo

Print & Cut (M37), tiling (M39), alimentador automático (M47), ajustes dependientes del
controlador (C59), y galvo completo con emulador: asistente (M41), encuadre con luz roja (M42),
calibración de lente (M43), temporización (M44), E/S (M45), marcado dividido y repetido (M40),
rotativo galvo (M30), corrección de cilindro (M46) y corte 3D por capas (C47). De la ola 4 llegan además el ajuste de U en los trabajos y el láser 2 con Ruida (M32), y el autoenfoque y optimizar Z (M31).

## Ola 6: calidad y publicación

- Rendimiento del lienzo con diseños grandes (índice espacial, y WebGL si hace falta).
- Lo que quedó de la ola 3: paneles flotantes, manijas y malla 4×4 de deformar, clic por esquina del redondeo, guías temporales y la recuperación compartida entre ventanas.
- Accesibilidad.
- Manual completo y sitio web con donaciones.
- Correr el flujo de instaladores en GitHub y activar las actualizaciones automáticas.
- Repetir la auditoría de cobertura.
- Lo que quedó de la ola 4: los ajustes de dispositivo que faltan (M3, M5, M6), cámaras de red y presets (M35), la alineación automática de la cámara del cabezal (M36), las cámaras en los paquetes de usuario (M4) y las pulgadas en las capas, las máquinas y la cámara (D12).
- Lista de validación con hardware real, para cuando haya máquinas: Falcon2 y el resto de GRBL, la conexión por red, Ruida (incluido escribir ajustes y borrar o ejecutar documentos), el K40, el ojo de pez, la cámara en el cabezal y el buscador de centro.
