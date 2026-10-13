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

## Ola 1: base para publicar

P0 pendientes y lo que más frena a quien viene de LightBurn.

| Bloque | Funciones |
|---|---|
| Edición básica | Voltear H/V (D26), espejo sobre línea (D27), rotación rápida (D24), rotación y escala por dos puntos (D25), orden de dibujo (D29), bloquear (D28), agrupar completo (D30), selecciones especiales (D21), igualar ancho y alto (D70), medir (D44), menú contextual (D09), polígono y estrella (D34, D35), portapapeles entre apps (D16), dibujo de Bézier completo (D32), snapping completo (D77) |
| Aplicación | Autoguardado y recuperación, preferencias (D12, M51, D99), editor de atajos (D11), archivos recientes, interruptores de transformación (D17) |
| Formatos | Exportar SVG, DXF e imágenes (D92, D93, D95); importar PDF y AI (D85, D86), HPGL (D88), G-code (D89, M22), WebP, TIFF, GIF y TGA (D87); opciones de importación (D91); imprimir o PDF (D98) |
| Migración | Importar `.lbrn` y `.lbrn2` con capas y ajustes (D97) |
| Texto y códigos | Fuentes de una línea (Hershey y SHX, D41), texto curvado (D39), texto soldado (D38), QR y códigos de barras (D43) |
| Ayuda | Menú Ayuda, manual de usuario, README actualizado |

Después de la ola: **idioma inglés** (M52), con todas las claves nuevas ya existentes.

## Ola 2: capas y corte

| Bloque | Funciones |
|---|---|
| Capas | Sub-capas (C30), copiar y pegar ajustes y seleccionar objetos de una capa (C06), ajustes de capa por defecto (C35), potencia mínima (C08), modo principiante (C60, D18) |
| Corte | Perforación (C13), lead-in y lead-out (C15), overcut (C16), pausa al inicio o fin y corte pasante (C17), modo punto (C18), rampa (C20), puentes manuales (C14, D45) |
| Relleno e imagen | Flood fill (C28), agrupación del relleno (C27), pass-through (C48), incremento de ángulo (C50), Adjust Image completo con enhance, denoise y presets (C51) |
| Orden | Prioridad por objeto y escala de potencia (C31), ajustes de optimización (C32, M49), quitar líneas solapadas en el envío (C33), ocultar backlash (C34), dirección y punto de inicio (D57) |
| Trabajo | Cortar solo la selección y origen de la selección (M15), encuadre de banda elástica (M12), posicionar el láser con clic (M17), ajuste de velocidad y potencia en vivo (M18), checklist del trabajo (M53) |

## Ola 3: diseño avanzado

Recortar formas (D49), separar (D55), optimizar formas (D56), deformar (D58), contorno de banda
elástica (D59), redondear esquinas (D60), asistente booleano (D64), cortar formas (D65), arreglo
virtual (D67), copiar a lo largo de un trazo (D69), redimensionar ranuras (D71), mover juntos y
acoplar (D74), nesting real (D76), guías (D78), vectorizar varios archivos (D80), máscara de imagen
(D81), convertir a mapa de bits (D82), círculo tangente (D46), biblioteca de arte (D47),
agrupación automática (D31), nueva ventana, notas y plantillas (D19), barras acoplables (D04),
texto variable con números de serie y CSV (D42, M38).

## Ola 4: máquinas

| Bloque | Funciones |
|---|---|
| Ruida en la app | Perfil con IP, envío a la memoria (M23), lista de archivos del controlador (M24), ajustes de máquina (M25), información del controlador (M26), rotativo DSP (M28) |
| K40 | Driver M2 Nano (LHYMICRO-GL) con emulador, experimental |
| Red y conexión | GRBL por TCP y WebSocket (M50), automatización por UDP (M54) |
| Control | Macros (M20), ventana Mover completa (M16), Z y enfoque (M31), eje U y láser 2 (M32), ajustes de dispositivo completos (M3, M5, M6), encuadre completo (M11), paquetes de usuario `.lbzip` (M4) |
| Cámara | Overlay, vectorizar desde la cámara, captura (M35), cámara en el cabezal (M36), modelo ojo de pez, buscador de centro (M48) |

## Ola 5: pro y galvo

Print & Cut (M37), tiling (M39), alimentador automático (M47), ajustes dependientes del
controlador (C59), y galvo completo con emulador: asistente (M41), encuadre con luz roja (M42),
calibración de lente (M43), temporización (M44), E/S (M45), marcado dividido y repetido (M40),
rotativo galvo (M30), corrección de cilindro (M46) y corte 3D por capas (C47).

## Ola 6: calidad y publicación

- Rendimiento del lienzo con diseños grandes (índice espacial, y WebGL si hace falta).
- Accesibilidad.
- Manual completo y sitio web con donaciones.
- Correr el flujo de instaladores en GitHub y activar las actualizaciones automáticas.
- Repetir la auditoría de cobertura.
- Lista de validación con hardware real, para cuando haya máquinas.
