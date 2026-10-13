# Cámara

Pon una cámara sobre la cama y Tlaku muestra su imagen como fondo del lienzo, para colocar el diseño justo sobre el material. Para que coincida hace falta corregir la lente, alinear la imagen con la cama y, si quieres precisión, indicar el grosor del material.

> **Ojo:** la cámara se probó con una cámara simulada, no con la cámara de una máquina real. Hoy sirve solo como fondo de referencia: no hay cámara montada en el cabezal, cámaras de red ni vectorizar a partir de la imagen. Usa las cámaras locales que el sistema ofrezca (por ejemplo, una webcam USB).

## El panel Cámara

Abre la pestaña **Cámara** (`Ctrl/Cmd+5`). De arriba abajo trae: fuente de video, vista en vivo, **Fondo en el lienzo**, **Lente**, **Alinear con marcas grabadas**, **Altura del material** y **Alineación con la cama**. Los mismos comandos aparecen en el menú **Máquina**: **Encender cámara**, **Mostrar fondo de cámara en el lienzo**, **Congelar el cuadro de la cámara**, **Calibrar lente de la cámara…** y **Alinear cámara con marcas grabadas…**.

![Panel Cámara con la vista en vivo, el fondo, la lente y las marcas grabadas](../../apps/desktop/docs/screenshots/17-panel-camara.png)

1. Elige la **Cámara** (por defecto, la predeterminada del sistema) y la **Resolución**. Pulsa **Encender cámara**; verás "Entrega 640 × 480 px" o la que corresponda.
2. Si falla, el panel dice por qué. En macOS, si no hay permiso, abre Ajustes del Sistema, Privacidad y seguridad, Cámara, activa Tlaku y vuelve a encender. Si dice que está en uso, cierra la otra aplicación que la tenga.

> **Consejo:** usa siempre la misma resolución. La calibración y la alineación se guardan para esa resolución; si cambia, la app avisa y hay que repetirlas.

La configuración (calibración y alineación) se guarda y se conserva al reiniciar.

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

## Altura del material

La cámara ve la superficie del material, que está más cerca que la cama, así que el fondo se corre según el grosor. En **Altura del material** escribe el **Grosor del material** (de 0 a 300 mm) y el fondo se corrige. La app dice qué tan fiable es la corrección:

- **Estimada con la lente** (aproximada), si solo hay una alineación.
- **Medida con marcas** a las alturas que hayas capturado. Para medirla, repite las marcas sobre una tabla de otra altura (al menos 2 mm de diferencia con las anteriores) y escribe esa altura en el asistente. Si tu grosor queda fuera del rango medido, te avisa que extrapolar es menos fiable.

## Lentes de ojo de pez

Las cámaras de 5 MP que vienen en la tapa de muchas láser de CO2 son de ojo de pez: la distorsión es tan fuerte que el modelo normal (Brown-Conrady) acaba con errores de varios milímetros en las esquinas aunque en el centro parezca bien. En el asistente de calibración, **Modelo de lente** ofrece:

- **Automático (recomendado)**: ajusta los dos modelos y elige el que mejor predice las vistas que reservó, no solo el de menor error de reproyección. Una lente normal se queda en Brown-Conrady.
- **Brown-Conrady** y **Ojo de pez (Kannala-Brandt)**, para fijar uno.

El resultado muestra el modelo elegido y una tabla con el ajuste y el error en vistas reservadas de cada modelo. Con ojo de pez toma vistas del tablero también en las esquinas de la imagen. La imagen corregida de un ojo de pez muestra un campo más amplio, con menos píxeles por milímetro en el centro. *Por validar con hardware real.*

## Capturar la imagen de la cama

Con la cámara alineada, **Imagen de la cama** guarda lo que ve la cámara dentro del documento:

- **Capturar imagen de la cama** guarda la foto, ya corregida y alineada, como una imagen del documento en el lugar exacto donde está sobre la cama (con la corrección por grosor del material). Si el proyecto no está guardado, el PNG queda en la carpeta temporal y la app te avisa.
- **Vectorizar desde la cámara** hace lo mismo y abre enseguida el diálogo de vectorizar sobre esa imagen.

En **Fondo en el lienzo** (junto a la opacidad) hay dos ajustes más: **Fondo en escala de grises** (para ver mejor el diseño encima; la imagen que captures también sale en gris) y **Actualizar el fondo al terminar un trabajo** (renueva el cuadro congelado, o repite el recorrido de la cámara del cabezal).

## Cámara en el cabezal

Si tu cámara va montada en el cabezal de la máquina, marca **La cámara está montada en el cabezal**: la imagen se mueve con la posición de la máquina. Hace falta calibrar el desfase entre la cámara y el láser:

1. **Generar marca** abre una cruz en el centro de la cama (puedes cambiar su posición). Grábala sobre un cartón sin moverlo, en coordenadas absolutas.
2. Enciende la cámara, conecta la máquina, mueve el cabezal hasta ver la cruz y pulsa **Medir con la cámara**. Haz clic en el centro de la cruz en la imagen.
3. Pulsa **Siguiente posición**: el cabezal se mueve unos milímetros y vuelves a hacer clic. Con 3 puntos o más se calculan el desfase, la escala y el error; **Guardar calibración** los conserva.

**Actualizar la superposición** recorre la cama en cuadrícula con el láser apagado, toma una foto en cada punto y las une con mezcla en los bordes; el mosaico queda de fondo fijo. Antes de mover la máquina la app pide confirmación y muestra cuántas posiciones son; el recorrido se puede cancelar. Con una selección puedes limitarlo a esa zona. El mismo recorrido se usa en **Capturar imagen de la cama**. *Por validar con hardware real.*

## Buscador de centro

**Buscador de centro** encuentra el centro de un objeto redondo con la cámara: marca **3 puntos del borde** en la imagen, o elige **Detectar el círculo** y haz clic dentro del objeto. Te dice el centro y el radio en mm y ofrece **Mover la cabeza al centro** (con el láser apagado) y **Origen del trabajo aquí** (fija el origen de usuario; después elige «Origen de usuario» en «Iniciar desde»). *Por validar con hardware real.*

Siguiente: [Materiales](11-materiales.md)
