# Rendimiento con diseños grandes

Qué esperar de Tlaku cuando el diseño no es una placa de ejemplo sino un mapa, un SVG exportado de un programa de mapas, una foto de 80 megapíxeles o un DXF con cien mil líneas. Las cifras salen de las pruebas de estrés del proyecto (178 archivos reales y sintéticos, más pruebas del lienzo con 1 000 a 100 000 objetos). Son **medidas con la aplicación en modo simulado, con el emulador y con Chrome pintando por software**: los tiempos de pintura son pesimistas frente a una tarjeta gráfica real, y el cálculo del motor es representativo. Nada de esto se ha medido con una máquina real, y tu equipo puede ir más lento o más rápido.

## En una frase

Hasta **10 000 objetos** el lienzo no tiene costo que se note; con **50 000** sigue fluido para mirar y mover un objeto; con **100 000** funciona, pero mover **todo** a la vez tarda unos 0,4 segundos y la aplicación usa unos 650 MB de memoria. El cálculo de un mapa con decenas de miles de trayectos tarda **un segundo o poco más**. Una imagen de **60 a 100 megapíxeles** se importa sin agotar la memoria. Lo que se vuelve el límite antes que Tlaku es el **cable USB**: un trabajo muy detallado puede pedir más líneas por segundo de las que caben a 115 200 baudios.

## El lienzo

| Diseño | Qué se siente |
|---|---|
| Hasta 10 000 objetos | Todo al instante: cargar, mover, hacer zoom, seleccionar. |
| 50 000 objetos | Carga en unos 0,3 s; el paneo y el zoom siguen fluidos. Mover la selección completa se ve a unos 7 cuadros por segundo y confirmar el movimiento tarda unos 0,3 s. Aceptable. |
| 100 000 objetos | Carga en unos 0,6 s; un clic tarda unos 0,2 s. **Mover todo a la vez tarda unos 0,4 s** (antes de las mejoras de rendimiento, 0,57 s) y la aplicación ocupa unos 650 MB. Es el punto en el que se nota. |
| Un solo trayecto de 100 000 nodos | Sin costo medible. |
| Un solo trayecto de un millón de nodos | Pintar es lo más caro (unos 0,4 s al cargar). El clic tardaba 0,3 s y tras las mejoras unos 50 ms; arrastrar el trayecto entero era lo más lento (unos 3 cuadros por segundo antes de las mejoras). |

Para que el lienzo no se arrastre con diseños así, Tlaku pinta con un **mapa de bits reutilizable** cuando hay más de unos 12 000 segmentos visibles, usa un **índice espacial** para el clic, el pasar del puntero y la selección con rectángulo (no recorre todos los objetos), y calcula solo los **cambios** al editar. Consejos prácticos:

- **Oculta o bloquea** lo que no vas a tocar, y trabaja sobre una selección pequeña: mover 10 objetos de 100 000 cuesta decenas de milisegundos.
- Un archivo con cientos de miles de objetos casi siempre es un **dibujo exportado de un mapa o de un programa de CAD** con una pieza por trazo. Si puedes, une trazos (**Organizar > Unir**) y simplifica (**Optimizar formas**) antes de seguir.
- **Revisar diseño** ([Revisar diseño](07b-revisar-diseno.md)) encuentra líneas duplicadas y nodos de sobra, que son lo que más infla estos archivos; **Arreglar todo lo seguro** los limpia en un paso.

## Calcular el trabajo (vista previa, iniciar, exportar)

Es lo que tarda cuando pulsas **Vista previa** o **Iniciar**. El tiempo depende del número de trayectos y de nodos, no del tamaño en milímetros.

- **Mapas e ilustraciones con decenas de miles de trayectos.** El orden de corte con **Interior primero** (cortar los huecos antes que el contorno) era el cuello de botella: un mapa topográfico con 35 413 trayectos y casi 400 000 nodos tardaba unos 30 s. Ahora tarda **1 a 1,5 s**, con el mismo orden, el mismo tiempo estimado y el mismo recorrido. Un PDF con pocos trayectos muy largos (unos 300 000 nodos) pasó de 14 s a menos de 1 s.
- **Millones de líneas sueltas.** **Reducir recorridos en vacío** con un millón de líneas separadas tardaba unos 40 s; ahora, unos 5 s con el mismo resultado. Con 100 000 líneas, menos de un segundo.
- **Imágenes grandes.** Tlaku lee la imagen **por filas o por franjas** y la reduce a la rejilla de grabado antes de procesarla, así que importar una foto de 60 a 108 megapíxeles ocupa **menos de 75 MB** (antes, de 1,3 a 2,3 GB) y calcular el trabajo tarda de 2,5 a 6 s (antes, de 17 a 28 s). Los TIFF grandes y los YCbCr cargan. Más de 100 megapíxeles no se probó: un mapa de bits de ese tamaño a 0,1 mm es un trabajo de millones de líneas.
- **Tiempo estimado.** Contra el emulador, el estimado quedó a unos pocos por ciento del tiempo simulado en los archivos reales. Con una máquina real puede variar más (la aceleración real, el firmware); trátalo como una guía.

## El G-code y el cable

Un mapa de cuatro millones de nodos produce **millones de líneas y decenas de megabytes de G-code** (el mapa de Alemania de las pruebas: unas 4,3 millones de líneas sin optimizar, 3,0 millones con la fusión de cortes casi alineados, que Tlaku aplica sola sin cambiar el recorrido).

A **115 200 baudios** el cable deja pasar unos 11 500 bytes por segundo, unas 600 líneas por segundo de 19 bytes. Un mapa a 1 000 mm/min puede pedir 70 % de eso, y **por encima de unos 1 400 mm/min la máquina se quedaría sin líneas y se detendría esperando**, dejando marcas en el material. Por eso el diálogo de **Iniciar** avisa cuando el enlace no da abasto (ver [Enviar trabajos con seguridad](09-enviar-con-seguridad.md)), con la velocidad a la que bajar esa capa. Con USB nativo o por red el límite es mucho mayor. Enviar 250 000 líneas por USB serie ya son minutos.

## Qué hacer si tu diseño es pesado

1. **Simplifica antes de enviar:** **Revisar diseño**, **Optimizar formas** y **Quitar duplicados** reducen nodos y líneas sin cambiar cómo se ve.
2. **Baja la velocidad de las capas más detalladas** o usa **Solo la selección** para probar por partes.
3. **Usa un enlace rápido** si tu controladora lo permite (USB nativo, red): el cable serie es lo primero que se acaba.
4. **Prueba con la vista previa** antes de iniciar y mira el tiempo estimado: si tarda decenas de segundos en calcular, la máquina también tendrá mucho que leer.
5. **Imágenes:** no hace falta más resolución que la del intervalo de la capa. **Revisar diseño** avisa de las imágenes con **más resolución de la necesaria**; reducirlas ahorra tiempo de proceso sin cambiar el grabado.

## Cómo se midió

Las pruebas viven en el repositorio: el programa `tlaku-bench` para el motor (`scripts/run-stress.sh`) y `apps/desktop/scripts/canvas-stress.mjs` para el lienzo. Los resultados completos, archivo por archivo, están en `docs/pruebas-estres.md` del repositorio. Si tienes un diseño que se siente lento en tu equipo, **Ayuda > Reportar un problema…** con su tamaño y qué tardó ayuda a mejorarlo.

Siguiente: [Glosario](16-glosario.md)
