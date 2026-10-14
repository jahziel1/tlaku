# Cámara

Pon una cámara sobre la cama y Tlaku muestra su imagen como fondo del lienzo, para colocar el diseño justo sobre el material. Para que coincida hace falta corregir la lente, alinear la imagen con la cama y, si quieres precisión, indicar el grosor del material.

> **Ojo:** la cámara se probó con cámaras y fotos simuladas, no con la cámara de una máquina real. Además de servir de fondo de referencia, la imagen puede guardarse en el documento, vectorizarse, armarse como mosaico con una cámara montada en el cabezal y usarse para encontrar el centro de un objeto redondo. También se admiten **cámaras de red** (una URL de snapshot o MJPEG de tu red local), **configuraciones de cámara** con nombre y un **ajuste manual** del fondo. Lo marcado *por validar con hardware real* se probó solo con datos sintéticos.

## El panel Cámara

Abre la pestaña **Cámara** (`Ctrl/Cmd+5`). De arriba abajo trae: fuente de video, **Configuraciones de cámara**, vista en vivo, **Fondo en el lienzo**, **Ajuste manual del fondo**, **Imagen de la cama**, **Lente**, **Alinear con marcas grabadas**, **Altura del material**, **Alineación con la cama**, **Cámara en el cabezal** y **Buscador de centro**. Los mismos comandos aparecen en el menú **Máquina**: **Encender cámara**, **Mostrar fondo de cámara en el lienzo**, **Congelar el cuadro de la cámara**, **Calibrar lente de la cámara…**, **Alinear cámara con marcas grabadas…**, **Capturar imagen de la cama**, **Vectorizar desde la cámara…**, **Actualizar la superposición de la cámara** y **Buscador de centro (cámara)…**.

![Panel Cámara con la vista en vivo, el fondo, la lente y las marcas grabadas](../../apps/desktop/docs/screenshots/17-panel-camara.png)

1. Elige la **Cámara** (por defecto, la predeterminada del sistema) y la **Resolución**. Pulsa **Encender cámara**; verás "Entrega 640 × 480 px" o la que corresponda.
2. Si falla, el panel dice por qué. En macOS, si no hay permiso, abre Ajustes del Sistema, Privacidad y seguridad, Cámara, activa Tlaku y vuelve a encender. Si dice que está en uso, cierra la otra aplicación que la tenga.

> **Consejo:** usa siempre la misma resolución. La calibración y la alineación se guardan para esa resolución; si cambia, la app avisa y hay que repetirlas.

La configuración (calibración y alineación) se guarda y se conserva al reiniciar.

### Cámara de red

En **Tipo de cámara** elige **Cámara de red (URL)** y pega la dirección de un *snapshot* HTTP (JPEG) o de un flujo MJPEG, por ejemplo `http://192.168.1.20/snapshot.jpg`. Tlaku pide los cuadros a la cámara (unos cuantos por segundo), así que el fondo no es tan fluido como el de una webcam USB.

- **Debe ser de tu red local** (`192.168.x.x`, `10.x.x.x`...) y empezar con `http://`. Tlaku no se conecta a direcciones de internet ni a `https://`.
- Si la cámara pide usuario y clave, ponlos en la URL (`http://usuario:clave@192.168.1.20/...`; autenticación Basic, no Digest). **Se guardan solo en este equipo**, en el archivo de configuración de la cámara (`camera.json`, en texto plano: protege esa carpeta como cualquier archivo con contraseñas). En el panel se muestran con la clave oculta y **nunca** se incluyen en un paquete `.tlakubundle`.
- Si no hay imagen, el panel muestra el motivo que dio la cámara (clave rechazada, dirección fuera de la red local, no responde...).

### Configuraciones de cámara

Guarda con un nombre la fuente (cámara local o URL de red), la resolución y el ajuste manual del fondo, y vuelve a ellos con un clic; si la cámara estaba encendida se reabre con la configuración elegida. Hasta 32; guardar con un nombre que ya existe lo actualiza.

## Paso 1: calibrar la lente

Casi todas las cámaras curvan la imagen, sobre todo en los bordes. Calibrar mide esa distorsión y la corrige. Pulsa **Calibrar lente…** (o **Recalibrar…**).

![Asistente de calibración con el tablero detectado y los resultados](../../apps/desktop/docs/screenshots/16-camara-calibracion.png)

1. Imprime el **Tablero para imprimir** (9 × 6 esquinas internas) con **Imprimir…**, **Guardar PDF…** o **Guardar SVG…**. Elige el tamaño del cuadro y la hoja (**A4 horizontal** o **Carta horizontal**). Imprímelo al 100 % y comprueba con una regla que un cuadro mide lo que dice. Pégalo sobre una superficie plana y rígida.
2. Enciende la cámara y muestra el tablero inclinado hacia distintos lados y en las esquinas del cuadro. Cuando aparezca **Tablero detectado**, pulsa **Capturar vista**.
3. Reúne al menos 6 vistas variadas (el mínimo absoluto es 3). El asistente muestra la **Cobertura del campo** y te dice qué zonas faltan ("Sin cubrir: arriba a la izquierda...").
4. Pulsa **Calcular**. El **Error de reproyección** se califica así: menos de 0,5 px **excelente**, de 0,5 a 1 **buena**, de 1 a 2 **aceptable** y 2 o más **mala, conviene repetir** (repite con el tablero plano, sin reflejos, quieto y con más ángulos).
5. Si el resultado te sirve, **Usar y guardar**. **Descartar resultado** lo tira.

Calibra a la misma resolución con la que vas a trabajar. **Quitar calibración** la borra.

## Paso 2: alinear con la cama

Alinear le dice a Tlaku qué parte de la imagen es qué parte de la cama. Hay dos métodos; sin alineación el fondo no se muestra.

### Con marcas grabadas (recomendado)

Graba cruces en posiciones conocidas y la cámara se alinea sola, midiendo su precisión en mm.

1. En **Alinear con marcas grabadas** elige el **Número de marcas**: **9 marcas (mide la precisión)** o **4 marcas (rápido)**.
2. Pulsa **Abrir documento de marcas**. Se abre un documento nuevo, con una capa de baja potencia y las cruces en el 85 % de la cama; la esquina inferior izquierda lleva un círculo para fijar la orientación.
3. Graba ese documento sobre un cartón fijo en la cama y **no lo muevas después**.
4. Sin mover el cartón ni la cámara, enciende la cámara hasta ver la cama y pulsa **Capturar y detectar…**. En el asistente, deja la **Altura de las marcas** en 0 si están sobre la cama (o el grosor del material) y pulsa **Capturar y detectar**.
5. Revisa la foto: verde es una marca usada, ámbar tiene error alto, rojo está descartada, rojo punteado no se encontró. El **Error del ajuste** se califica de excelente (menos de 0,3 mm) a mala (1 mm o más). Con 9 marcas también ves la **Precisión real, dejando una marca fuera**; con menos de 5 no se puede medir.
6. Si es buena, **Aceptar alineación** (hacen falta al menos 4 marcas usadas). Si no, **Descartar y repetir**.

### A mano, con 4 puntos

En **Alineación con la cama**, marca en la imagen las 4 esquinas de una hoja de tamaño conocido (o de la cama), en este orden, vistas desde el frente de la máquina: inferior izquierda, inferior derecha, superior derecha, superior izquierda. Arrastra los puntos para ajustarlos y escribe sus coordenadas en mm, o pulsa **Cama completa**, **Hoja A4** u **Hoja carta** para rellenarlas. Luego **Aplicar alineación**. Te dice el error de los puntos en mm. **Quitar puntos** y **Borrar alineación** deshacen.

## Paso 3: usar el fondo

Con la alineación hecha, marca **Mostrar la cámara bajo el diseño**. Ajusta la **Opacidad** y, si quieres, **Congelar cuadro**: el cuadro se queda como fondo aunque apagues la cámara, útil para dejar el material en su sitio y diseñar sin la cámara encendida. Si la alineación guardada es de una imagen de otro tamaño, la app te pide volver a alinear.

## Preferencias de la cámara

Las preferencias de **cada cámara** (dispositivo, resolución, opacidad, escala de grises, actualizar al terminar un trabajo y el ajuste manual del fondo) están en este panel y se guardan en el perfil local de la cámara. Las **generales** están en **Edición > Preferencias… > General > Cámara**: **Reabrir la cámara al iniciar** (si el fondo estaba visible al cerrar, vuelve a encenderse solo; encendida de fábrica) y **Ancho máximo de la imagen** (de 320 a 1920 px, 1280 de fábrica: el ancho al que se reduce el cuadro para pintarlo y para detectar marcas y círculos; más ancho da más detalle y pide más al equipo).

## Altura del material

La cámara ve la superficie del material, que está más cerca que la cama, así que el fondo se corre según el grosor. En **Altura del material** escribe el **Grosor del material** (de 0 a 300 mm) y el fondo se corrige. La app dice qué tan fiable es la corrección:

- **Estimada con la lente** (aproximada), si solo hay una alineación.
- **Medida con marcas** a las alturas que hayas capturado. Para medirla, repite las marcas sobre una tabla de otra altura (al menos 2 mm de diferencia con las anteriores) y escribe esa altura en el asistente. Si tu grosor queda fuera del rango medido, te avisa que extrapolar es menos fiable.

## Lentes de ojo de pez

Las cámaras de 5 MP que vienen en la tapa de muchas láser de CO2 son de ojo de pez: la distorsión es tan fuerte que el modelo normal (Brown-Conrady) acaba con errores de varios milímetros en las esquinas aunque en el centro parezca bien. En el asistente de calibración, **Modelo de lente** ofrece:

- **Automático (recomendado)**: ajusta los dos modelos y elige el que mejor predice las vistas que reservó, no solo el de menor error de reproyección. Una lente normal se queda en Brown-Conrady.
- **Brown-Conrady** y **Ojo de pez (Kannala-Brandt)**, para fijar uno.

El resultado muestra el modelo elegido y una tabla con el ajuste y el error en vistas reservadas de cada modelo. Con ojo de pez toma vistas del tablero también en las esquinas de la imagen. La imagen corregida de un ojo de pez muestra un campo más amplio, con menos píxeles por milímetro en el centro. *Por validar con hardware real: se probó con cámaras sintéticas, no con una lente real.*

> **Ejemplo:** tu cámara de la tapa deja las esquinas de la cama curvas y con errores de varios milímetros aunque en el centro se vea bien. Calibra con el modelo **Automático** y toma vistas del tablero también en las esquinas. En la prueba sintética del proyecto (un ojo de pez de 140°), el modelo normal parecía bueno con 0,56 px de error de reproyección pero se equivocaba hasta 108 mm en el plano; el de ojo de pez, 3,2 mm. Por eso el asistente compara con vistas reservadas y no solo con el error de reproyección.

## Ajuste manual del fondo

Si la alineación quedó casi bien pero no del todo, **Ajuste manual del fondo** (con la cámara fija y una alineación guardada) deja afinar a mano: marca **Fijar la resolución a mano** y cambia los **puntos por mm (DPM)** (debajo ves los que da la alineación), y mueve el fondo con el **Desplazamiento X** y **Y** en mm. El ajuste se aplica encima de la alineación, sin cambiarla, y también a la imagen de la cama que captures. **Quitar el ajuste** vuelve a la alineación. No se usa con la cámara en el cabezal.

## Capturar la imagen de la cama

Con la cámara alineada, **Imagen de la cama** guarda lo que ve la cámara dentro del documento:

- **Capturar imagen de la cama** guarda la foto, ya corregida y alineada, como una imagen del documento en el lugar exacto donde está sobre la cama (con la corrección por grosor del material). Si el proyecto no está guardado, el PNG queda en la carpeta temporal y la app te avisa.
- **Vectorizar desde la cámara** hace lo mismo y abre enseguida el diálogo de vectorizar sobre esa imagen (ver [Imágenes](06-imagenes.md)).

> **Ejemplo:** pones un dibujo hecho a mano en tinta negra sobre papel en la cama, capturas la imagen y la vectorizas: queda un trayecto que puedes cortar o grabar encima de la pieza real, en su lugar exacto.

En **Fondo en el lienzo** (junto a la opacidad) hay dos ajustes más: **Fondo en escala de grises** (para ver mejor el diseño encima; la imagen que captures también sale en gris) y **Actualizar el fondo al terminar un trabajo** (renueva el cuadro congelado, o repite el recorrido de la cámara del cabezal).

## Cámara en el cabezal

Si tu cámara va montada en el cabezal de la máquina, marca **La cámara está montada en el cabezal**: la imagen se mueve con la posición de la máquina. Hace falta calibrar el desfase entre la cámara y el láser:

1. **Generar marca** abre una cruz en el centro de la cama (puedes cambiar su posición). Grábala sobre un cartón sin moverlo, en coordenadas absolutas.
2. Enciende la cámara, conecta la máquina, mueve el cabezal hasta ver la cruz y pulsa **Medir con la cámara**. Haz clic en el centro de la cruz en la imagen.
3. Pulsa **Siguiente posición**: el cabezal se mueve unos milímetros y vuelves a hacer clic. Con 3 puntos o más se calculan el desfase, la escala y el error; **Guardar calibración** los conserva.

**Actualizar la superposición** recorre la cama en cuadrícula con el láser apagado, toma una foto en cada punto y las une con mezcla en los bordes; el mosaico queda de fondo fijo. Antes de mover la máquina la app pide confirmación y muestra cuántas posiciones son; el recorrido se puede cancelar. En **Zona del recorrido** eliges recorrer **toda la cama**, **la selección** o **una zona dibujada en el lienzo**: pulsa **Dibujar zona en el lienzo** y arrastra un rectángulo (Esc cancela); queda marcado con líneas punteadas y su tamaño, y el recorrido, la captura y «actualizar al terminar un trabajo» cubren solo esa zona. El mismo recorrido se usa en **Capturar imagen de la cama**. *Por validar con hardware real: probado con la máquina virtual y fotos sintéticas.*

**Alineación automática con marcas.** En lugar de hacer clic en la cruz, **Alineación automática con marcas** calcula el desfase y la escala de una sola foto: **Generar las marcas como documento** abre un cuadro de 9 cruces (de 60 mm de lado como mínimo; puedes cambiar el lado y la esquina) y la primera lleva un círculo. Grábalas sobre un cartón sin moverlo, mueve el cabezal hasta ver las marcas con el círculo a la vista, déjalo quieto y pulsa **Detectar marcas**. Con 4 marcas o más sale la calibración (puedes repetir desde otra posición para afinarla); **Guardar calibración** la conserva. *Por validar con hardware real.*

## Buscador de centro

**Buscador de centro** encuentra el centro de un objeto redondo con la cámara: marca **3 puntos del borde** en la imagen, o elige **Detectar el círculo** y haz clic dentro del objeto. Te dice el centro y el radio en mm y ofrece **Mover la cabeza al centro** (con el láser apagado) y **Origen del trabajo aquí** (fija el origen de usuario; después elige «Origen de usuario» en «Iniciar desde»). *Por validar con hardware real.*

> **Ejemplo:** quieres grabar un logotipo en el centro de un posavasos redondo. Enciende la cámara con la alineación hecha, abre **Buscador de centro**, elige **Detectar el círculo**, haz clic dentro del posavasos y mira el centro y el radio. Pulsa **Origen del trabajo aquí**, y en **Iniciar desde** elige **Origen de usuario** con el punto del trabajo en el **centro**: el logotipo queda centrado. Si la detección falla (necesita ver al menos el 45 % del contorno), usa los **3 puntos del borde**. Este buscador no toca el cabezal: se apoya en la alineación de la cámara, así que es tan exacto como ella. Si no tienes cámara, o quieres medir con la propia máquina, usa el siguiente.

## Buscador de centro con el cabezal

**Máquina > Buscador de centro con el cabezal…** halla el centro de un objeto redondo con tres puntos de su borde medidos por la **propia máquina**, sin cámara. Hay dos modos:

- **Con sonda o placa de contacto (G38).** Solo con controladoras **GRBL** que tengan una sonda cableada a la entrada de la sonda. Pon el cabezal **fuera** del objeto, a menos del recorrido de su borde, y elige hacia dónde tocar (arriba, abajo, izquierda o derecha). Antes de mover, Tlaku te muestra el recorrido, la velocidad y el G-code exacto (`M5`, `G91`, `G38.3 X30 Y0 F100`, `G90`) y pide que confirmes. Entonces el cabezal avanza **despacio y con el láser apagado** hasta que la sonda hace contacto, se detiene y retrocede unos milímetros. Repite desde tres lados distintos: al tercer punto ves el centro y el radio.
- **Con el puntero.** Sin sonda: lleva el puntero rojo (o la cabeza) a un punto del borde con **Mover** y pulsa **Guardar punto**; repite en tres puntos bien repartidos. Cada punto lo confirmas tú. Si el perfil tiene un desfase del puntero, se tiene en cuenta.

Después, igual que con la cámara: **Mover la cabeza al centro** (láser apagado) y **Origen del trabajo aquí**. **Quitar el último** y **Empezar de nuevo** corrigen un punto mal puesto.

**Seguridad.** Los valores de fábrica son prudentes: 100 mm/min de velocidad (tope 500), 30 mm de recorrido (tope 100) y 2 mm de retroceso. El láser queda apagado todo el tiempo, solo se mueve con la máquina en reposo y el motor rechaza el toque si no hubo confirmación. Si no hay contacto en todo el recorrido, Tlaku lo avisa («No hubo contacto en 30 mm») y **no deja la máquina en alarma** (usa `G38.3`, que no la provoca); si la sonda ya estaba activa al empezar, la controladora da `ALARM:4` y hay que alejar el cabezal y desbloquear. El contacto se toma donde está la punta de la sonda: mide primero un objeto de diámetro conocido para saber cuánto se desvía. *Por validar con hardware real: se probó con el emulador de GRBL, que simula la sonda (`G38.2` a `G38.5`).* Las máquinas virtuales GRBL de la app traen un disco de 30 mm de diámetro centrado en X100 Y100 para probarlo sin una máquina.

> **Ejemplo:** quieres centrar un grabado en una moneda de 25 mm pegada en la cama. Coloca el cabezal a 10 mm a la izquierda del borde, elige **Hacia la derecha**, confirma, y repite desde abajo y desde arriba. Si el diámetro que muestra no es 25 mm (±0,5 mm), tu sonda tiene un desvío: anótalo.

Siguiente: [Materiales](11-materiales.md)
