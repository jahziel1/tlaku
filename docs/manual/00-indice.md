# Manual de Tlaku

Tlaku es un programa libre (licencia MIT) para preparar y enviar trabajos a cortadoras y grabadoras láser. Este manual explica lo que la aplicación hace hoy; lo que todavía no está en la aplicación se marca como "(próximamente)".

> **Ojo:** nada se ha probado todavía con una máquina real. Todo el envío, la conexión y las controladoras se validaron con el emulador (`tlaku-emu`) y con las máquinas virtuales de la propia app. Empieza siempre con la máquina virtual y, cuando uses un láser de verdad, hazlo con supervisión.

## Para empezar

- [Primeros pasos](01-primeros-pasos.md): instalar, importar un diseño, revisar las capas, ver la vista previa, probar con la máquina virtual y guardar.
- [La interfaz](02-interfaz.md): barra superior, herramientas, lienzo, paneles, paleta de colores, barra de estado y atajos de teclado.
- [Preferencias, atajos y autoguardado](02b-preferencias-y-atajos.md): la ventana de preferencias, cambiar atajos, llevar tu configuración a otro equipo y recuperar el trabajo tras un cierre inesperado.

## Diseñar

- [Dibujar y editar](03-dibujar-y-editar.md): herramientas de dibujo (curvas, polígono y estrella), medir, selección, voltear y girar, orden y bloqueo, nodos, agrupar, menú contextual, menú Organizar y deshacer.
- [Importar y exportar](03b-importar-y-exportar.md): los formatos que se importan (SVG, DXF, PDF, AI, HPGL, G-code e imágenes), sus opciones, y exportar a SVG, DXF, PNG y PDF o imprimir a escala 1:1.
- [Texto](04-texto.md): crear texto, elegir fuente (también de una línea), curvarlo, soldarlo, escribirlo sobre un trayecto, convertirlo a trayectos e insertar códigos QR y de barras.
- [Capas y ajustes de corte](05-capas-y-ajustes.md): modos Línea, Relleno e Imagen, velocidad, potencia, pasadas, puentes y orden de ejecución.
- [Imágenes](06-imagenes.md): importar fotos, elegir el tramado, ajustar tonos y vectorizar una imagen.

## Preparar el trabajo

- [Vista previa y orden de corte](07-vista-previa.md): reproducir el recorrido, leer los resultados y entender por qué se cortó en ese orden.

## La máquina

- [Conectar tu máquina](08-conectar-maquina.md): el asistente Agregar máquina, el puerto, el perfil y la máquina virtual.
- [Enviar trabajos con seguridad](09-enviar-con-seguridad.md): encuadrar, origen del trabajo, el diálogo de iniciar, pausa, detener y exportar G-code.
- [Cámara](10-camara.md): calibrar la lente, alinear el diseño con el material y usar marcas grabadas.
- [Materiales](11-materiales.md): biblioteca de ajustes, prueba de materiales e intercambio de archivos `.tlakumat`.
- [Rotativo y eje Z](12-rotativo-y-eje-z.md): grabar sobre cilindros y trabajar con una mesa motorizada.
- [Controladoras soportadas](13-controladoras.md): qué controladoras funcionan, con qué nivel de validación, y los dialectos de G-code.

## Ayuda

- [Venir desde LightBurn](14-desde-lightburn.md): equivalencias de nombres y de atajos, qué hay, qué falta, cómo traer un diseño y qué conserva y qué avisa el importador de proyectos de LightBurn.
- [Solución de problemas](15-solucion-de-problemas.md): alarmas y errores de GRBL, puertos serie, drivers y qué hacer cuando el láser no dispara.
- [Glosario](16-glosario.md): los términos de láser y de Tlaku, de kerf a overscan.
- [Novedades](novedades.md): lo que trae cada versión.

---

Siguiente: [Primeros pasos](01-primeros-pasos.md)
