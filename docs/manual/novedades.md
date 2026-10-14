# Novedades

Lo más importante que trae cada versión de Tlaku, escrito para quien usa la aplicación. El detalle técnico está en el repositorio.

## Imágenes, Print & Cut y alimentador, en la próxima versión

- **Presets de imagen** (Foto en madera, Logo de alto contraste y más) que aplican brillo, contraste, gamma, realce y quitar motas de una vez, y los tuyos con **Guardar como…**. Mira [Imágenes](06-imagenes.md).
- **Vista previa de la imagen**: ve el resultado tramado de una imagen antes de calcular el trabajo, entera o a tamaño real con zoom.
- **Las imágenes giran con el registro de Print & Cut.**
- **Repetir un trabajo con el alimentador**: N repeticiones con el avance del alimentador entre una y otra, desde el diálogo de inicio. Mira [Tiling y alimentador](09d-tiling-y-alimentador.md). Solo con máquinas GRBL; el alimentador propio de una controladora DSP sigue sin estar.
- **Límite:** nada de esto se ha probado con una máquina real.

## Modo sencillo, materiales de fábrica y control desde el celular, en la próxima versión

Sexta tanda: Tlaku se vuelve más fácil para quien empieza, trae ajustes de partida para los materiales más comunes y deja vigilar un trabajo desde el celular. **Nada de esto se ha probado con una máquina real**: se validó con la máquina virtual, con pruebas automáticas y con un celular simulado.

### Modo sencillo

- **Una versión guiada en cinco pasos**: abrir el diseño, elegir el material, colocarlo, revisar e iniciar. La primera vez, Tlaku pregunta **¿Es tu primera vez con un láser?**; el botón de la barra superior cambia de modo cuando quieras, **sin tocar tu diseño**.
- **Cortar, Grabar o Ignorar por color**: eliges el material y qué hacer con cada color; Tlaku pone velocidad, potencia y pasadas. Con **Ajustar** (más pasadas, menos quemado, más claro o más oscuro), **Ver números** y **Guardar como mi material**.
- **Te pregunta los watts de tu láser** ("¿De cuántos watts es tu láser?") para usar los ajustes de la clase correcta; sin saberlo, usa los más prudentes. El acrílico transparente con diodo no deja elegir **Cortar**. Mira [Modo sencillo](01b-modo-sencillo.md).

### Materiales de fábrica

- **134 ajustes de partida** de corte y grabado, en la sección **De fábrica** de la pestaña Materiales (la biblioteca ya no empieza vacía). Cada uno dice su **fuente** (con enlace) y su **confianza** (**Oficial**, **Guía** o **Foro**).
- **Clase de máquina** (diodo, CO2 o fibra, por potencia) para ver los ajustes de la tuya, y un aviso fijo: **son puntos de partida, no una garantía**. Se duplican a **Mis materiales** para editarlos. Mira [Materiales](11-materiales.md).

### Control desde el celular

- **Ver el avance y pausar o detener desde el celular**, escaneando un código QR en **Preferencias > Control remoto**; sin instalar nada. **Pausar y detener funcionan siempre**; **iniciar** exige el checklist en el celular y que el trabajo esté **encuadrado en la computadora después del último cambio**. El láser galvo nunca se inicia desde el celular.
- **Apagado de fábrica, solo en tu red local y con una clave que cambia cada vez que abres la app.** **No hay HTTPS**: úsalo solo en una red de confianza. Mira [Control desde el celular](09e-control-desde-el-celular.md).
- **Límite:** Safari en iPhone y iPad no deja que la página haga vibrar el celular al terminar (el aviso y el pitido sí salen).

### Lo que sigue sin estar

- Probar todo con hardware real y con celulares reales.

## Pro y galvo, en la próxima versión

Quinta gran tanda: lo que más se pedía para trabajos de producción, y un primer paso hacia los láseres galvo. **Nada de esto se ha probado con una máquina real**: Print & Cut y el tiling se validaron con la máquina virtual, una hoja impresa simulada y fotos sintéticas, y el galvo con una tarjeta simulada que escribimos nosotros.

### Print & Cut

- **Cortar justo sobre lo impreso.** Un asistente de tres pasos (**Máquina > Print & Cut…**): agrega marcas de registro de 2 a 4 (cruces o círculos) en las esquinas del diseño, imprímelas a tamaño real y mide dónde quedaron, **con el láser** (las flechas y «Guardar posición») **o con la cámara** (un clic en la imagen alineada).
- **Ajuste sin escala, de escala uniforme o afín**, con el error de cada marca a la vista antes de aceptar y avisos si la impresión salió escalada, espejada o estirada distinto en cada eje. La vista previa, el encuadre y el trabajo salen movidos según el registro. Mira [Print & Cut](09c-print-and-cut.md).
- Las imágenes **giran** con el registro, igual que los vectores.

### Tiling y alimentador automático

- **Dividir en tramos** (**Máquina > Dividir en tramos…**): un trabajo más grande que la cama se parte en tramos con solape, por filas o en serpentina, con el plano dibujado en el lienzo. **LightBurn no tiene tiling.**
- **Marcas de alineación** en las zonas de solape, grabadas en los dos tramos, para colocar el material.
- **Envío por tramos**: tú mueves el material entre uno y otro, puedes saltar o repetir un tramo, y si se cierra la app o se corta la conexión, **retomas** donde ibas.
- **Alimentador automático** en el perfil de la máquina: un G-code de avance con `{mm}` que el tiling usa solo entre tramos cuando van en una sola columna. Mira [Tiling y alimentador automático](09d-tiling-y-alimentador.md).

### Galvo (experimental)

- **Láseres de fibra, UV y CO2 galvo** con la tarjeta LMCV4 de EZCAD2 (USB `9588:9899`), **desde la línea de comandos**: `tlaku galvo list`, `status`, `light` (encuadre con el puntero rojo, que nunca enciende el láser) y `mark` (con `--red-only`, `--dry-run` y `--emu` para probar sin máquina). **La interfaz en la app está en camino.**
- **Pensado para no dañar a nadie**: con hardware real exige `--i-understand-laser-safety`, usa una lista blanca de comandos, apaga la fuente ante cualquier error y **no tiene pausa**, solo detener. Un láser de fibra es clase 4 y su haz es invisible: **lee [Seguridad del galvo](13d-galvo-seguridad.md) antes de conectar nada.** Mira también [Galvo (experimental)](13c-galvo.md).
- **En el motor**: grabado 3D por cortes (también sirve en pórtico), corrección de cilindro y de cono, bamboleo, y la secuencia del marcado dividido, repetido y rotativo. Mira [Grabado 3D por cortes, cilindro y cono](12b-grabado-3d-y-cilindro.md).
- **Experimental de verdad**: las unidades de velocidad, frecuencia, potencia y retardos vienen de otros proyectos de código abierto y no están verificadas con una tarjeta real.

### Lo que sigue sin estar

- Probar todo con hardware real, y la prueba mínima del galvo con una tarjeta sin fuente.
- Autoenfoque, y el ajuste de U por capa.

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
- Galvo, **Print & Cut** y **tiling** (llegaron en la tanda siguiente: ver «Pro y galvo»).

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
