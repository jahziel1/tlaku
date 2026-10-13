# 00 - Resumen consolidado y hoja de ruta

Inventario funcional de LightBurn hecho en sala limpia (solo documentación pública, foro y
tutoriales; sin código, textos ni íconos de LightBurn). Detalle por área:

| Archivo | Área | Funciones | P0 | P1 | P2 |
|---|---|---|---|---|---|
| [01-diseno-edicion.md](01-diseno-edicion.md) | Interfaz, dibujo, edición, importación | 100 | 32 | 39 | 29 |
| [02-capas-corte-imagen.md](02-capas-corte-imagen.md) | Capas, ajustes de corte, rellenos, imagen, vista previa | 60 | 22 | 23 | 15 |
| [03-maquinas-avanzadas.md](03-maquinas-avanzadas.md) | Dispositivos, control, cámara, rotary, galvo | 53 | 13 | 24 | 16 |
| **Total** | | **213** | **67** | **86** | **60** |

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
alinear, arreglos, snapping, auto-join. Con esto se cubren los 67 P0.

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

Pendiente de M4: Ruida en la app, sub-capas, macros, modelo ojo de pez, galvo y K40 (M2 Nano), y
prueba con hardware real.
