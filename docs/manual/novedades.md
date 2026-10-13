# Novedades

Lo más importante que trae cada versión de Tlaku, escrito para quien usa la aplicación. El detalle técnico está en el repositorio.

## Versión 0.1.0, primera versión pública

Primera versión de la aplicación de escritorio. Es una versión beta: **todavía no se ha probado con una máquina láser real**, solo con el emulador y las máquinas virtuales de la app. Empieza siempre con poca potencia y material de desecho.

### Diseño y edición

- Lienzo con la cama de tu máquina, reglas, zoom, ajuste magnético y barra de valores exactos.
- Herramientas de línea (con curvas Bézier), rectángulo, elipse, polígono, estrella, texto, medir y editor de nodos.
- Voltear, espejo sobre una línea, giros rápidos y por dos puntos, orden de dibujo, bloquear, grupos anidados, igualar tamaños y las selecciones especiales (formas abiertas, círculos por tamaño y más).
- Menú contextual con clic derecho, ajuste magnético a nodos, puntos medios, centros e intersecciones, y copiar y pegar SVG entre programas.
- Unión, diferencia, intersección, soldar, offset, alinear, distribuir y arreglos en rejilla y circulares.
- Unir trazos, cerrar trayectos y quitar duplicados.
- Texto con fuentes del sistema, también sobre un trayecto, **curvado** y con las letras **soldadas**.
- 15 fuentes de una línea (Hershey) incluidas y lector de fuentes SHX.
- Códigos QR, Data Matrix y de barras (Code 128, Code 39, EAN-13, EAN-8, UPC-A, ITF y Codabar).
- Deshacer y rehacer con un historial al que puedes saltar.

### Importar

- SVG (con recortes reales), DXF, **PDF**, **AI**, **HPGL** (`.plt`), **G-code** e imágenes PNG, JPG, BMP, **WebP**, **TIFF**, **GIF** y **TGA**. Cada color del archivo se vuelve una capa.
- **Importar con opciones**: escala, DPI, unidad de un DXF, página de un PDF, agrupar y capa destino.
- Vectorizar una imagen con vista previa.
- Los importadores avisan de todo lo que no pudieron traer, en vez de callar.

### Exportar e imprimir

- Exportar el diseño (todo o solo la selección) a **SVG**, **DXF**, **PNG** y **PDF** vectorial a escala 1:1, y **Imprimir** a tamaño real.

### Preferencias y respaldo

- Ventana de **Preferencias** (`Ctrl/Cmd+,`): unidades, pasos de las flechas, imán, rejilla, tema, autoguardado y archivos recientes.
- **Atajos de teclado configurables**, con aviso de conflictos, y archivos `.tlakuprefs` para llevar tu configuración a otro equipo.
- **Autoguardado** y recuperación del trabajo tras un cierre inesperado.

### Capas y corte

- Modos Línea, Relleno e Imagen, con velocidad, potencia, pasadas, intervalo, ángulo y overscan calculado con la física de la máquina.
- Compensación de kerf, puentes para que la pieza no se suelte, aire asistido y eje Z.
- Imágenes con varios métodos de dithering, semitono, periódico y boceto.
- Biblioteca de materiales y prueba de materiales.

### Vista previa y orden de corte

- Reproducción del recorrido con tiempo estimado y desglose por capa.
- Planificador explicable: cada paso dice qué regla lo decidió.

### Máquina

- Asistente para agregar tu máquina, con detección de solo lectura y un catálogo de modelos.
- Máquinas virtuales con fallas simuladas, para practicar sin riesgo.
- Revisión de seguridad antes de enviar, encuadre, origen de 9 puntos, "Iniciar desde", pausa y paro.
- Consola que explica cada alarma y cada error.
- Dialectos de G-code para GRBL, grblHAL, FluidNC, GRBL-LPC, Smoothie, Marlin y G-code propio.
- Rotativo (rodillo o mandril) y corrección de barrido.
- Cámara con calibración de lente, alineación y marcas grabadas.

### Ayuda

- Este manual dentro de la app (menú Ayuda, con buscador), atajos de teclado con `F1`, Novedades, Reportar un problema y Acerca de.

### Lo que viene

- **Importar proyectos de LightBurn** (`.lbrn` y `.lbrn2`) con sus capas y ajustes.
- **Interfaz en inglés**, con cambio de idioma en vivo desde Preferencias.
- Más herramientas de diseño (recortar, separar, redondear esquinas, deformar), ajustes de corte avanzados (perforación, lead-in, sub-capas), texto variable y números de serie.
- Ruida (CO2) existe solo por línea de comandos y es experimental. Conexión por red, K40 y galvo siguen sin estar.
