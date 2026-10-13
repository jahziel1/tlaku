# Manual de Tlaku

Tlaku es un programa libre (licencia MIT) para preparar y enviar trabajos a cortadoras y grabadoras láser. Este manual explica lo que la aplicación hace hoy. Lo que todavía no está en la aplicación se dice con claridad en cada página (por ejemplo, en la sección "Lo que falta o es distinto" de [Venir desde LightBurn](14-desde-lightburn.md)); no se promete nada que no esté en el código.

> **Ojo:** nada se ha probado todavía con una máquina real. Todo el envío, la conexión y las controladoras se validaron con el emulador (`tlaku-emu`) y con las máquinas virtuales de la propia app. Empieza siempre con la máquina virtual y, cuando uses un láser de verdad, hazlo con supervisión.

## Para empezar

- [Primeros pasos](01-primeros-pasos.md): instalar, importar un diseño, revisar las capas, ver la vista previa, probar con la máquina virtual y guardar.
- [La interfaz](02-interfaz.md): barra superior, herramientas, lienzo y guías, paneles reordenables, paleta de colores, barra de estado y atajos de teclado.
- [Preferencias, atajos y autoguardado](02b-preferencias-y-atajos.md): la ventana de preferencias, cambiar atajos, llevar tu configuración a otro equipo y recuperar el trabajo tras un cierre inesperado.

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
- [Enviar trabajos con seguridad](09-enviar-con-seguridad.md): enviar solo la selección, origen del trabajo, mover el cabezal y posicionar el láser, encuadrar (también por contorno), el diálogo de iniciar con su lista de comprobación, ajustes en vivo, pausa, detener y exportar G-code.
- [Cámara](10-camara.md): calibrar la lente, alinear el diseño con el material y usar marcas grabadas.
- [Materiales](11-materiales.md): biblioteca de ajustes, prueba de materiales e intercambio de archivos `.tlakumat`.
- [Rotativo y eje Z](12-rotativo-y-eje-z.md): grabar sobre cilindros y trabajar con una mesa motorizada.
- [Controladoras soportadas](13-controladoras.md): qué controladoras funcionan, con qué nivel de validación, y los dialectos de G-code.

## Ayuda

- [Venir desde LightBurn](14-desde-lightburn.md): equivalencias de nombres y de atajos (también las herramientas de diseño avanzado, el nesting y el texto variable), qué hay, qué falta, cómo traer un diseño y qué conserva y qué avisa el importador de proyectos de LightBurn.
- [Solución de problemas](15-solucion-de-problemas.md): alarmas y errores de GRBL, puertos serie, drivers y qué hacer cuando el láser no dispara.
- [Glosario](16-glosario.md): los términos de láser y de Tlaku, de kerf a overscan.
- [Novedades](novedades.md): lo que trae cada versión.

---

Siguiente: [Primeros pasos](01-primeros-pasos.md)
