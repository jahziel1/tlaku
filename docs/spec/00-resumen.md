# 00 - Resumen consolidado y hoja de ruta

Inventario funcional de LightBurn hecho en sala limpia (solo documentación pública, foro y
tutoriales; sin código, textos ni íconos de LightBurn). Detalle por área:

| Archivo | Área | Funciones | P0 | P1 | P2 |
|---|---|---|---|---|---|
| [01-diseno-edicion.md](01-diseno-edicion.md) | Interfaz, dibujo, edición, importación | 100 | 32 | 39 | 29 |
| [02-capas-corte-imagen.md](02-capas-corte-imagen.md) | Capas, ajustes de corte, rellenos, imagen, vista previa | 60 | 22 | 23 | 15 |
| [03-maquinas-avanzadas.md](03-maquinas-avanzadas.md) | Dispositivos, control, cámara, rotary, galvo | 54 | 14 | 24 | 16 |
| **Total** | | **214** | **68** | **86** | **60** |

La fila 21 del inventario 03 mezcla prioridades (P0 para G-code, P1 para `.rd`) y se cuenta como P0;
por eso son 54 funciones y 68 P0. El detalle de avance está en [04-cobertura.md](04-cobertura.md).

Lo marcado "(no verificado)" o "(inferido)" en cada archivo debe confirmarse antes de implementarlo.

## Arquitectura

```
crates/
  tlaku-core     modelo del documento: formas con curvas, capas, ajustes, trabajo
  tlaku-geom     booleanas, offset, kerf, auto-join, cerrar trayectos       (i_overlay / clipper2, cavalier_contours, kurbo)
  tlaku-raster   ajustes de imagen, dithering, umbral, escala de grises     (image, imageproc)
  tlaku-plan     orden de corte, rellenos por barrido, overscan, tiempo estimado
  tlaku-svg      importación SVG (ya existe); luego tlaku-dxf, imágenes, formato nativo
  tlaku-grbl     G-code y envío por serie (ya existe); luego laser-ruida, laser-jcz
  tlaku-cli      herramienta de línea de comandos (ya existe)
apps/desktop     Tauri + TypeScript, lienzo WebGL
```

Regla: el motor no sabe nada de la interfaz. Todo lo que hace la app se puede ejecutar y
probar desde la línea de comandos con archivos dorados.

## Fases

**M1 - Motor completo sin interfaz.** Formas con curvas (no solo polilíneas), importación
SVG/DXF/imágenes, modos Line/Fill/Image, dithering (Floyd-Steinberg, Jarvis, Stucki, umbral),
ajustes de imagen, orden de corte, overscan, estimación de tiempo con modelo de aceleración,
G-code GRBL M3/M4 y envío con conteo de caracteres probado contra el simulador de GRBL.

**M2 - App básica (primer lanzamiento público).** Lienzo con zoom, selección y transformaciones,
paleta de colores/capas, editor de ajustes por capa, vista previa, gestor de dispositivos,
ventana de control (conectar, encuadre, iniciar/pausar/detener, jog, consola, origen de 9
puntos, "iniciar desde"), deshacer/rehacer, formato de proyecto propio.

**M3 - Edición.** Texto con fuentes del sistema, edición de nodos, booleanas, offset, soldar,
alinear, arreglos, snapping, auto-join. Con esto se cubren los 68 P0.

**M4 - Taller.** Ruida (red y USB), biblioteca de materiales, test de materiales, kerf, tabs,
sub-capas, eje Z, rotary, cámara (calibración de lente + homografía), macros.

**M5 - Pro.** Galvo EZCAD2, Print & Cut, texto variable/CSV/números de serie, tiling.

## Dónde podemos superar a LightBurn (quejas del foro)

- **Tiling:** LightBurn 2.1 no lo tiene; trabajos más grandes que la cama se hacen a mano.
- **Planificador de corte explicable:** la queja más repetida es no entender por qué el orden
  de corte sale como sale. Mostrar qué regla decidió cada paso.
- **Offset Fill que no se cuelga** y vista previa de tabs/kerf que coincide con la salida.
- **Importadores fieles:** unidades ausentes, 72 vs 96 DPI, máscaras de recorte.
- **El encuadre considera el overscan** para avisar antes de iniciar, no a mitad del trabajo.

## Dependencias y licencia

El proyecto es MIT (decidido 2026-10-12). Las bibliotecas citadas existen en crates.io
(verificado 2026-10-12). Solo se aceptan dependencias con licencias permisivas (MIT, Apache-2.0,
BSD, Zlib, ISC); una dependencia GPL obligaría a distribuir el binario como GPL (por ejemplo,
usar vtracer y no potrace). Se automatizará con `cargo-deny`.

## Estado (2026-10-13)

Las olas del [plan](../plan-completo.md) 1 a 4 están **terminadas**. Según el recuento de
[04-cobertura.md](04-cobertura.md) (que sale de contar las filas de sus tablas): **173 de 214
funciones hechas (81 %), 26 parciales y 15 faltantes**; en P0, **64 de 68 hechas** (las 4 parciales
son C51, M3, M5 y M6). **Ninguna función de máquina se ha probado con hardware real**: todo se
validó con emuladores (`tlaku-emu`, el de Ruida y el de la placa K40), máquinas virtuales e
imágenes sintéticas.

| Ola | Tema | Estado | Hechas / 214 | P0 hechas / 68 |
|---|---|---|---:|---:|
| (inicio) | M1 a M4, oleadas A a C | Terminada | 88 | 53 |
| 1 | Base para publicar: edición básica, preferencias y atajos, autoguardado, exportar e importar formatos, texto, QR, manual | Terminada (`736d472`) | 119 | 60 |
| 2 | Capas y corte: trazos, orden y optimización, relleno e imagen, trabajo | Terminada (`ec9cdb4`) | 143 | 62 |
| 3 | Diseño avanzado: operaciones de forma, nesting real, texto variable, imagen | Terminada (`b78790e`) | 165 | 62 |
| 4 | Máquinas: Ruida, red, control, cámara y K40 | Terminada (`10c435d`; el K40 en la app, `c6c61a2`) | 173 | 64 |
| 5 | Pro y galvo | Pendiente | | |
| 6 | Calidad y publicación | Pendiente | | |

**K40 (M2 Nano).** No es una fila del inventario (LightBurn no lo soporta) y no cuenta en los
totales. Está en la app como **experimental, probado solo contra el emulador**: USB directo (no
serie), home propio, la potencia va con la perilla, sin Z, aire, consola ni overrides, y aviso de
paro de emergencia si detener no se confirma. También sigue disponible por línea de comandos
(`tlaku k40`).

**Lo que queda, según el plan:**

- **Ola 5, pro y galvo:** Print & Cut (M37), tiling (M39), alimentador automático (M47), ajustes
  dependientes del controlador (C59) y galvo completo con emulador: asistente (M41), encuadre con
  luz roja (M42), calibración de lente (M43), temporización (M44), E/S (M45), marcado dividido y
  repetido (M40), rotativo galvo (M30), corrección de cilindro (M46) y corte 3D por capas (C47).
  De la ola 4 llegan además el ajuste de U en los trabajos y el láser 2 con Ruida (M32), y el
  autoenfoque y optimizar Z (M31).
- **Ola 6, calidad y publicación:** rendimiento del lienzo con diseños grandes, lo que quedó de la
  ola 3 (paneles flotantes, manijas y malla 4×4 de deformar, clic por esquina del redondeo, guías
  temporales, recuperación compartida entre ventanas), accesibilidad, manual completo y sitio web
  con donaciones, correr el flujo de instaladores y activar las actualizaciones automáticas,
  repetir la auditoría de cobertura, lo que quedó de la ola 4 (ajustes de dispositivo M3, M5 y M6;
  cámaras de red y presets, M35; alineación automática de la cámara del cabezal, M36; cámaras en
  los paquetes, M4; pulgadas en las capas, las máquinas y la cámara, D12) y la **lista de
  validación con hardware real** de todo lo experimental: Falcon2 y el resto de GRBL, la conexión
  por red, Ruida (incluido escribir ajustes y borrar o ejecutar documentos), el K40, el ojo de
  pez, la cámara en el cabezal y el buscador de centro. Esa prueba con máquinas reales es el
  principal riesgo para publicar.

### Historia previa a las olas (M1 a M4)

**M1 terminado** (motor + CLI): importación SVG/DXF/imagen, modos Line/Fill/Image, kerf,
planificador explicable, overscan físico, estimación de tiempo, G-code GRBL, envío con conteo
de caracteres, verificación de estado antes de enviar, encuadre y consola segura. 408 pruebas.

Pendiente detectado en M1:
- No probado con hardware real (Falcon2 disponible para pruebas).
- ~~Recorte real de clip-path/mask en SVG~~ (fase 4; mask es aproximada).
- ~~Planificador: figuras que se solapan sin anidarse~~ (fase 4).
- Estimación: se asume parada entre rápidos y cortes; calibrar contra tiempos reales.
- ~~Imagen: ángulo de barrido, scanning offset, Newsprint/Halftone/Sketch. Relleno: Offset~~
  (fase 4). Falta Flood fill.
- Textos de ayuda y errores propios de clap en inglés.

**M2 y M3 terminados** (app Tauri, edición completa) y cámara partes 1-2 (calibración de lente,
fondo en vivo, alineación de 4 puntos).

**M4, oleada A (sin hardware) terminada:** puentes, eje Z por capa, rotativo (rodillos/mandril),
corrección de barrido por velocidad, semitono/periódico/boceto, ángulo de barrido en imágenes,
relleno por contornos, recorte SVG real, planificador con solapes, biblioteca y prueba de
materiales, unir/cerrar/quitar duplicados, instaladores (GitHub Actions) e ícono propio.
807 pruebas en Rust y 899 en la interfaz.

**M4, oleada B (validación sin hardware) terminada:** investigación de hardware con fuentes en
`docs/hardware/`; driver GRBL endurecido (ruido serie, `ERROR:NN` de Creality, parada real ante
`error:N`, cancelación sin ALARM:3, homing sin falso timeout, bloqueo con `$32=0`); emulador de
controladoras (`tlaku-emu`: GRBL, Falcon2 Pro, grblHAL, FluidNC, GRBL-LPC, Marlin y Smoothie
básicos); dialectos de G-code (GRBL-M3, grblHAL, FluidNC, GRBL-LPC, Smoothie, Marlin, personalizado);
Z relativa con regreso, corrección de barrido repartida, rotativo con valores reales y espejo;
núcleo Ruida con exportación `.rd` (`tlaku-ruida`, sin red); cámara parte 3 (marcas grabadas con
validación cruzada y corrección por altura). 1105 pruebas en Rust y 940 en la interfaz.

**M4, oleada C (máquinas) terminada:** máquinas virtuales en la app con fallas simuladas; envío
línea a línea para Marlin y Smoothie; asistente "Agregar máquina" con detección de solo lectura y
catálogo de 21 modelos con fuentes; selector de tipo de controladora y editor de G-code
personalizado; Ruida por red (sondeo, envío guardando o ejecutando, estado, pausa, detención)
desde el CLI, con las dudas del protocolo resueltas leyendo código MIT (ver THIRD_PARTY.md).
1225 pruebas en Rust y 1072 en la interfaz.
