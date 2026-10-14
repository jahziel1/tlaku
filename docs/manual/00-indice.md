# Manual de Tlaku

Tlaku es un programa libre (licencia MIT) para preparar y enviar trabajos a cortadoras y grabadoras láser. Este manual explica lo que la aplicación hace hoy. Lo que todavía no está en la aplicación se dice con claridad en cada página (por ejemplo, en la sección "Lo que falta o es distinto" de [Venir desde LightBurn](14-desde-lightburn.md)); no se promete nada que no esté en el código.

> **Ojo:** nada se ha probado todavía con una máquina real. Todo el envío, la conexión, las controladoras, Print & Cut, el tiling y el soporte galvo se validaron con el emulador (`tlaku-emu`), con la tarjeta galvo simulada y con las máquinas virtuales de la propia app. Empieza siempre con la máquina virtual y, cuando uses un láser de verdad, hazlo con supervisión. **Si vas a usar un láser galvo (fibra), lee antes [Seguridad del galvo](13d-galvo-seguridad.md)**: su haz es invisible.

## Para empezar

- [Primeros pasos](01-primeros-pasos.md): instalar, importar un diseño, revisar las capas, ver la vista previa, probar con la máquina virtual y guardar.
- [La interfaz](02-interfaz.md): barra superior, herramientas, lienzo y guías, paneles reordenables, paleta de colores, barra de estado y atajos de teclado.
- [Preferencias, atajos y autoguardado](02b-preferencias-y-atajos.md): la ventana de preferencias (con las pulgadas y la automatización por UDP), cambiar atajos, llevar tu configuración a otro equipo y recuperar el trabajo tras un cierre inesperado.

## Diseñar

- [Dibujar y editar](03-dibujar-y-editar.md): herramientas de dibujo (curvas, polígono y estrella), formas paramétricas, prioridad de corte y escala de potencia por objeto, medir, selección, voltear y girar, orden y bloqueo, nodos, agrupar, menú contextual, menú Organizar y deshacer; en las páginas siguientes, las operaciones avanzadas.
- [Importar y exportar](03b-importar-y-exportar.md): los formatos que se importan (SVG, DXF, PDF, AI, HPGL, G-code e imágenes), sus opciones, y exportar a SVG, DXF, PNG y PDF o imprimir a escala 1:1.
- [Formas avanzadas y deformar](03c-formas-y-deformar.md): recortar formas, separar subtrayectos, optimizar nodos, redondear esquinas, cortar con una cuchilla, redimensionar ranuras de encastre, contorno de banda elástica, círculo tangente, deformar y el asistente booleano.
- [Copias, arreglo virtual y nesting](03d-copias-y-nesting.md): copiar a lo largo de un trazo, arreglo virtual, acoplar y mover juntos, agrupar automáticamente y acomodar piezas en la lámina (nesting).
- [Guías, puentes, punto de inicio y proyecto](03e-guias-puentes-y-proyecto.md): guías desde las reglas, puentes manuales, punto de inicio y sentido del corte, notas, plantillas, nueva ventana, pestañas a tu gusto y la biblioteca de arte.
- [Texto](04-texto.md): crear texto, elegir fuente (también de una línea), curvarlo, soldarlo, escribirlo sobre un trayecto, convertirlo a trayectos e insertar códigos QR y de barras.
- [Texto variable](04b-texto-variable.md): números de serie, datos de un CSV, fecha y hora dentro de un texto, cómo avanzan después de cada trabajo y una copia por fila del CSV.
- [Capas y ajustes de corte](05-capas-y-ajustes.md): modos Línea, Relleno e Imagen, velocidad, potencia, pasadas, puentes, corte avanzado (perforación, entrada y salida, sobrecorte, pausas, modo punto, rampa), operaciones adicionales, valores por defecto, modo principiante y orden de ejecución.
- [Imágenes](06-imagenes.md): importar fotos, elegir el tramado, ajustar tonos, enviar sin tramar, realzar bordes, quitar motas, recortar con una forma, convertir vectores a mapa de bits, guardar la imagen procesada y vectorizar una o varias imágenes.

## Preparar el trabajo

- [Vista previa y orden de corte](07-vista-previa.md): reproducir el recorrido, leer los resultados, los ajustes de optimización y entender por qué se cortó en ese orden.

## La máquina

- [Conectar tu máquina](08-conectar-maquina.md): el asistente Agregar máquina, el puerto, el perfil y la máquina virtual.
- [Conexión por red y automatización](08b-red-y-automatizacion.md): GRBL por TCP y WebSocket, probar la conexión, la máquina virtual por red y la automatización por UDP (protocolo y seguridad).
- [Enviar trabajos con seguridad](09-enviar-con-seguridad.md): enviar solo la selección, origen del trabajo, mover el cabezal y posicionar el láser, encuadrar (también por contorno), el diálogo de iniciar con su lista de comprobación, ajustes en vivo, pausa, detener y exportar G-code.
- [Control avanzado de la máquina](09b-control-avanzado.md): macros, mover con Z y U, enfocar, posiciones guardadas, disparo de prueba, ajustes `$$` con respaldo, encuadre a baja potencia y en bucle, láser 2 y paquetes `.tlakubundle`.
- [Print & Cut](09c-print-and-cut.md): imprimir con marcas de registro y cortar justo sobre lo impreso, paso a paso con el láser o con la cámara, y qué significa el error residual.
- [Tiling y alimentador automático](09d-tiling-y-alimentador.md): dividir un trabajo más grande que la cama en tramos con solape y marcas de alineación, enviarlos de uno en uno, retomar y usar un alimentador automático (algo que LightBurn no tiene).
- [Cámara](10-camara.md): calibrar la lente (también ojo de pez), alinear el diseño con el material, capturar y vectorizar la cama, cámara en el cabezal y buscador de centro.
- [Materiales](11-materiales.md): biblioteca de ajustes, prueba de materiales e intercambio de archivos `.tlakumat`.
- [Rotativo y eje Z](12-rotativo-y-eje-z.md): grabar sobre cilindros y trabajar con una mesa motorizada.
- [Grabado 3D por cortes, cilindro y cono](12b-grabado-3d-y-cilindro.md): el relieve a partir de un mapa de grises en varias pasadas y la corrección para marcar con galvo sobre tazas y vasos cónicos (hoy en el motor).
- [Controladoras soportadas](13-controladoras.md): qué controladoras funcionan, qué se probó en emulador y qué queda por validar con hardware, y los dialectos de G-code.
- [Ruida y K40](13b-ruida-y-k40.md): las dos familias experimentales: qué hacen, qué no, por qué Ruida solo guarda por defecto, el plan de prueba del K40 y por qué su potencia va con la perilla.
- [Galvo (experimental)](13c-galvo.md): láseres de fibra, UV y CO2 galvo con tarjetas LMCV4 (EZCAD2): qué hace `tlaku galvo`, cómo probarlo sin máquina con la tarjeta simulada y qué no hace. Se maneja hoy desde la línea de comandos; la interfaz en la app está en camino.
- [Seguridad del galvo](13d-galvo-seguridad.md): **léela antes de conectar una tarjeta**: haz invisible de clase 4, gafas con la densidad óptica correcta, recinto, el modo `--red-only`, el plan de prueba con la tarjeta sin fuente y por qué no hay pausa.

## Ayuda

- [Venir desde LightBurn](14-desde-lightburn.md): equivalencias de nombres y de atajos (también las herramientas de diseño avanzado, el nesting y el texto variable, Print & Cut y el tiling), qué hay, qué falta, cómo traer un diseño y qué conserva y qué avisa el importador de proyectos de LightBurn.
- [Solución de problemas](15-solucion-de-problemas.md): alarmas y errores de GRBL, puertos serie, drivers, errores de red, Ruida que no responde, K40 que no aparece por USB y qué hacer cuando el láser no dispara.
- [Glosario](16-glosario.md): los términos de láser y de Tlaku, de kerf a overscan.
- [Novedades](novedades.md): lo que trae cada versión.

---

Siguiente: [Primeros pasos](01-primeros-pasos.md)
