# Novedades

Lo más importante que trae cada versión de Tlaku, escrito para quien usa la aplicación. El detalle técnico está en el repositorio.

## Máquinas, en la próxima versión

Cuarta gran tanda: Tlaku sale del cable USB y de GRBL, y la ventana de la máquina gana lo que más se echaba de menos. **Nada de esto se ha probado con una máquina real**: se validó con emuladores, máquinas virtuales e imágenes sintéticas. Lo marcado *experimental* puede equivocarse con una máquina de verdad: empieza con el láser deshabilitado o a potencia baja, material de desecho y el paro de emergencia al alcance.

### Conexión por red y automatización

- **GRBL por red**: grblHAL con Ethernet y FluidNC por **TCP / Telnet**, y FluidNC y ESP3D por **WebSocket**. Se elige en el perfil, con un botón **Probar conexión** que no mueve nada y errores de red explicados en español. Mira [Conexión por red y automatización](08b-red-y-automatizacion.md).
- **Máquina virtual por red**: el mismo emulador detrás de un socket TCP local, para ver la conexión por red sin tener la máquina.
- **Automatización por UDP**: otros programas de tu equipo pueden cargar un diseño, preguntar el estado o detener el trabajo con comandos de texto (`PING`, `LOAD`, `START`, `STATUS`, `STOP`). Está **apagada** de fábrica, solo escucha en `127.0.0.1` y `START` solo abre el diálogo de iniciar salvo que lo permitas tú.

### Ruida y K40 (experimental)

- **Ruida (CO2) en la app**: conectar por IP, enviar el trabajo (por defecto **se guarda en la memoria de la controladora y no arranca**; tú lo inicias desde su panel), pausar, reanudar, detener, mover, encuadrar, leer la información de la máquina y la lista de documentos. Los ajustes de la controladora solo se leen. Hay una máquina virtual **Ruida (CO2)**. Mira [Ruida y K40](13b-ruida-y-k40.md).
- **K40 con placa M2 Nano**, que LightBurn no soporta: ahora también **en la app** (tipo de máquina «K40 con M2 Nano»): conexión por USB directo, home propio, aviso de lo que la placa ignora antes de enviar, aviso de paro de emergencia si detener no se confirma y máquina virtual `virtual:k40`. Sigue disponible `tlaku k40` (envío, estado de solo lectura, `.egv`). Es **experimental y solo está probado contra el emulador**. **La potencia no se controla por software: va con la perilla de la máquina.**

### Control de la máquina

- **Macros** (hasta 10 por perfil), con botones y atajos, que pasan por las reglas de seguridad de la consola.
- **Mover con Z y U**, **Enfocar**, **posiciones guardadas** y **disparo de prueba** (hasta 20 % y 2 s, con doble confirmación). Mira [Control avanzado de la máquina](09b-control-avanzado.md).
- **Ajustes `$$`** de la controladora con nombre, descripción y marca de crítico, edición con confirmación y **respaldo** `.tlakuset` con restauración por diferencias.
- **Encuadre a baja potencia** y **en bucle** hasta detener.
- **Láser 2** por capa y **eje U** para mover a mano.
- **Paquetes `.tlakubundle`**: tus máquinas (con macros y posiciones), materiales, preferencias y atajos en un solo archivo, con vista previa antes de importar.
- **Potencia en puentes**: el láser puede marcar el puente sin atravesarlo, para que la pieza se suelte con un golpe. Mira [Capas y ajustes de corte](05-capas-y-ajustes.md).

### Cámara

- **Ojo de pez**: nuevo modelo de lente con elección automática, que se prueba con vistas reservadas. Pensado para las cámaras de la tapa de los láseres de CO2.
- **Capturar la imagen de la cama** como imagen del documento y **vectorizarla**.
- **Cámara en el cabezal**: calibración del desfase y mosaico de la cama.
- **Buscador de centro**: encuentra el centro de un objeto redondo y mueve la cabeza o fija el origen ahí. Mira [Cámara](10-camara.md).

### Pequeñas mejoras

- Las **pulgadas** de las preferencias llegan a la barra de estado, las reglas, los pasos de las flechas y los diálogos de Organizar y de Acomodar piezas, con fracciones (`3/8"`).
- Las piezas del nesting llevan nombres legibles, y las copias del arreglo virtual cuentan en las cajas del diseño.
- La copia de recuperación de una ventana ya no se ofrece en otra ventana abierta (corregido leyendo el código y con pruebas; falta comprobarlo con ventanas reales).

### Lo que sigue sin estar

- Probar todo con hardware real. Ruida no escribe ajustes ni borra o ejecuta documentos; el K40 solo se ha probado contra el emulador, sin placa real.
- Cámaras de red, presets de cámara y alinear la cámara del cabezal con marcas.
- Las pulgadas en las capas, las máquinas y la cámara; marcar un perfil como predeterminado; autoenfoque.
- Galvo, **Print & Cut** y **tiling**.

## Diseño avanzado, en la próxima versión

Tercera gran tanda: las herramientas de diseño que más se echaban de menos. Sigue sin probarse con una máquina real: revisa siempre la vista previa y empieza con material de desecho.

### Cambiar la forma de un dibujo

- **Recortar formas** (herramienta `X`): clic en un tramo entre dos cruces para borrarlo.
- **Separar subtrayectos**, **optimizar formas** (menos nodos con una tolerancia), **redondear esquinas** (redondeada, chaflán o mordisco), **cortar formas con una cuchilla** y **redimensionar ranuras** de encastre al grosor real del material.
- **Deformar** (arco, abombado, pellizco, perspectiva y sesgo, con esquinas y bordes ajustables), **contorno de banda elástica**, **círculo tangente** y el **asistente booleano** (`Ctrl/Cmd+B`), que enseña el resultado de cada operación antes de aplicarla.

### Copias y acomodo

- **Copiar a lo largo de un trazo**, **arreglo virtual** (copias que no son objetos, con fantasmas en el lienzo), **acoplar y mover juntos** y **agrupar automáticamente por contención**.
- **Nesting**: acomoda piezas en la cama o dentro de un material de forma irregular, con rotaciones, espejo, huecos aprovechables y un tope de tiempo, con progreso, aprovechamiento y vista previa; se puede detener.

### Texto variable

- **Números de serie**, **datos de un CSV**, **fecha y hora** dentro de un texto, con avance al terminar cada trabajo y **una copia por fila**. Mira [Texto variable](04b-texto-variable.md).

### Lienzo y proyecto

- **Guías** que se arrastran desde las reglas, **puentes manuales** (`U`), **punto de inicio y sentido** del corte (`Q`) que el planificador respeta, **notas** del proyecto, **plantillas**, **nueva ventana** y pestañas del panel derecho que se reordenan.
- **Biblioteca de arte** con 41 dibujos (flechas, marcos, estrellas, corazones, iconos, bordes y cajas con encastre) y tus propios SVG.

### Imágenes

- **Recortar una imagen con una forma** (máscara), **convertir vectores a mapa de bits**, **guardar la imagen procesada**, **reemplazar la imagen** y **vectorizar varios archivos** de una vez.

### Importar proyectos de LightBurn

- El importador ahora trae el **perforado**, el **modo de puntos**, el **relleno inundado**, el **sobrecorte**, la **rampa**, las **pausas**, el **corte pasante** y las **sub-capas** (como operaciones adicionales). Varias conversiones se infirieron de la documentación pública: revisa cada capa.
- El editor de capas ya no deja escribir valores que el motor rechazaría (ángulos de entrada y salida, corte pasante de 0 ms) y admite el mismo máximo de operaciones adicionales.
- La **potencia mínima** deja de mostrarse en las capas de línea, con una explicación: la baja de potencia en las esquinas la decide la controladora.

### Lo que sigue sin estar

- **Deformar** se hace desde un diálogo: aún no hay manijas en el lienzo ni malla de 16 puntos.
- **Redondear esquinas** no tiene la herramienta de clic por esquina; los **puentes manuales** no se arrastran (la potencia de puente llegó después, ver más arriba); no hay guías temporales de alineación ni paneles flotantes.
- **Nueva ventana** no se ha probado en una ventana real. Al abrirse, el aviso de trabajo sin guardar podía mostrar la copia de la otra ventana; la ola de máquinas lo corrigió (ver más arriba), pero sigue sin comprobarse con ventanas reales: ante la duda elige **Decidir después**.
- **Tiling** y **Print & Cut** siguen sin estar.

## Capas y corte: ajustes avanzados

Segunda gran tanda de funciones, pensada para quien viene de LightBurn y quiere el mismo control sobre cómo se corta. Sigue sin probarse con una máquina real: empieza con poca potencia y material de desecho.

### Ajustes de corte (capas de Línea)

- **Corte avanzado** en el editor de capa: **perforación** (corta un tramo y salta otro, ideal para dobleces en cartón), **entrada y salida del contorno** (recta o en arco, para que el arranque no deje marca en el borde), **sobrecorte**, **pausas** al empezar y al terminar, **corte pasante**, **modo punto** y **rampa de potencia**.
- Las pausas usan `G4`; el **Dialecto de G-code** ahora dice en qué unidad (segundos o milisegundos) las lee tu controladora.
- Si dos ajustes no se combinan (modo punto con perforación, por ejemplo), el editor lo marca y la vista previa avisa cuál se ignoró.

### Relleno e imagen

- Relleno **por regiones** (menos viajes en vacío con muchas piezas sueltas) y **agrupación** del relleno: todo junto, forma por forma o por grupos.
- **Incremento de ángulo** entre pasadas.
- Imágenes: **enviar sin tramar** (pass-through), **realce de bordes** y **quitar motas**.

### Capas

- **Operaciones adicionales (sub-capas)**: rellena y luego contornea la misma forma con una sola capa.
- **Copiar y pegar ajustes** entre capas, **borrar una capa con sus objetos**, **valores por defecto** propios para las capas nuevas y **Modo principiante** para ocultar los ajustes avanzados.
- **Prioridad de corte** y **escala de potencia** por objeto (ventana Propiedades del objeto), con números sobre el lienzo para revisarlas.
- Las formas paramétricas (rectángulo, elipse, polígono y estrella) **guardan sus parámetros en el proyecto**: al reabrirlo se siguen editando.

### Orden del trabajo

- **Ajustes de optimización**: respetar capas, grupos y prioridad, interiores primero, mejor punto de inicio, quitar tramos que se cortarían dos veces y esconder el juego mecánico.
- **Solo la selección**: vista previa, encuadre, trabajo y G-code con únicamente lo seleccionado, con la opción de usar su propio origen.

### En la máquina

- **Encuadrar contorno** (banda elástica) para piezas largas o en diagonal.
- **Posicionar el láser**: un clic en la cama lleva el cabezal allí, con el láser apagado.
- **Ajustes en vivo** de velocidad y potencia durante el trabajo (controladoras GRBL).
- Ventana Mover: **pasos configurables**, **posición final** (quedarse, volver al origen o ir a un punto) e **ir al origen de trabajo**.
- **Lista de comprobación** configurable en el diálogo de iniciar (aire, enfoque, material, extractor y tapa), con la opción de exigirla antes de enviar.

### Lo que sigue sin estar

- La **potencia mínima** de una capa de línea no cambia el G-code (solo en imágenes); desde la tanda siguiente el campo ya no se muestra en esas capas.
- El encuadre no se repite en bucle y la ventana Mover no tiene Z, posiciones guardadas, Fire ni Focus.
- *(Resuelto en la tanda de diseño avanzado: los puentes manuales y el importador de proyectos de LightBurn con estos ajustes.)*

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
- Proyectos de LightBurn (`.lbrn` y `.lbrn2`) con sus capas y ajustes.
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

### Idiomas

- Interfaz en español e inglés, con cambio de idioma en vivo desde Preferencias.

### Ayuda

- Este manual dentro de la app (menú Ayuda, con buscador), atajos de teclado con `F1`, Novedades, Reportar un problema y Acerca de.

### Lo que viene

- Las herramientas de diseño que se anunciaban aquí (recortar, separar, redondear esquinas, deformar, texto variable y números de serie) llegaron en la tanda de diseño avanzado.
- Ruida (CO2) existe solo por línea de comandos y es experimental. Conexión por red, K40 y galvo siguen sin estar.
