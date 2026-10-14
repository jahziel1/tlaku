# Print & Cut

Imprimes un diseño en papel, vinilo o madera con la impresora y quieres que el láser corte **exactamente** por el contorno, aunque hayas puesto la hoja un poco torcida o corrida sobre la cama. Eso es **Print & Cut**: se imprimen unas **marcas de registro** junto con el diseño, le dices a Tlaku dónde quedaron de verdad en la máquina, y Tlaku mueve, gira (y si hace falta estira) todo el trabajo para que caiga sobre lo impreso.

> **Ojo:** Print & Cut se probó con la máquina virtual y con una hoja impresa simulada (unos milímetros corrida y un poco girada). **No se ha probado con un láser ni con una cámara reales.** Haz las primeras pruebas con papel de desecho, la potencia al mínimo o el láser deshabilitado, y la mano cerca del paro de emergencia.

## Cuándo sirve

- Pegatinas, etiquetas y marcos de fotos: la impresora pone el color y el láser corta el contorno.
- Piezas que ya vienen impresas o grabadas y que hay que cortar o grabar de nuevo en su sitio.
- Cualquier trabajo en dos máquinas distintas (la impresora y el láser) que tiene que coincidir.

No sirve para un material que se mueve entre una marca y otra: **no muevas la hoja mientras mides**.

## Antes de empezar

1. **Conecta la máquina y haz home.** Print & Cut trabaja con las coordenadas absolutas de la cama, así que Tlaku necesita saber dónde está el cabezal. Al aceptar el registro, el trabajo pasa solo a **Iniciar desde: coordenadas absolutas** y la app te avisa.
2. **No muevas el origen** del trabajo después de medir.
3. **Con un rotativo activo no se puede medir con el láser** (la posición del cabezal ya no es un punto plano de la cama).
4. Si vas a medir con la cámara, ten la cámara **calibrada y alineada** con la cama y con su imagen visible en el lienzo ([Cámara](10-camara.md)).

## Paso a paso: unas pegatinas

El ejemplo: una hoja de pegatinas con un contorno de corte rojo, para cortar con el láser.

### 1. Diseña y agrega las marcas

1. Dibuja o importa el arte y el contorno de corte (el contorno va en una capa de línea).
2. **Máquina > Print & Cut…** abre el asistente, que flota sobre el lienzo y no lo bloquea: puedes seguir seleccionando objetos mientras lo usas. Tiene tres pasos: **Marcas**, **Medir** y **Resultado**.
3. En **Agregar marcas de registro** elige la **forma** (**Cruz** o **Círculo**) y la **cantidad** (2, 3 o 4 marcas) y pulsa **Agregar marcas de registro**. Tlaku pone las marcas en las esquinas del diseño, a 8 mm de su borde: con 2 van en diagonal, con 3 en L y con 4 en las cuatro esquinas.
4. Las marcas van en la **capa de guías**: **no se cortan** ni salen a la máquina, pero sí se imprimen con el diseño. Cuanto más separadas estén, más preciso es el giro que se mide.

Si ya tienes tus propias marcas (por ejemplo, las de un archivo que te mandó otra persona), en lugar de agregar marcas selecciona en el lienzo **un objeto o un grupo por marca** y pulsa **Usar la selección como marcas**. Hacen falta de 2 a 4, y a más de 10 mm una de otra: si están más cerca, el asistente no te deja avanzar porque así no se puede medir el giro.

### 2. Imprime a tamaño real

**Archivo > Imprimir…** (`Ctrl/Cmd+P`) genera un PDF a escala 1:1 y lo abre en el visor del sistema; si solo quieres imprimir el arte y las marcas, elige **solo la selección** en el diálogo. Imprímelo con **tamaño real (100 %)** y **sin ajustar a la página** (ver [Importar y exportar](03b-importar-y-exportar.md)). Si la impresora escala la hoja, Tlaku lo notará más adelante como una escala distinta de 100 %.

### 3. Coloca la hoja y mide cada marca

Pon la hoja en la cama, **fíjala** (cinta o imanes) y pasa al paso **Medir**. Elige cómo medir:

**Con el láser.** Para cada marca:

1. **Ir a la marca (aproximado)** lleva el cabezal a donde estaría la marca si la hoja estuviera exactamente en su sitio.
2. Afina con las **flechas** (con los mismos pasos que la pestaña Máquina) hasta que el punto del láser quede en el **centro de la cruz**. Para ver dónde apunta el cabezal usa el **puntero rojo** si tu máquina lo tiene o el **disparo de prueba a la potencia mínima** (pestaña Máquina). Con el láser encendido: protección ocular y nada inflamable ni reflejante bajo el haz.
3. Pulsa **Guardar posición**. El asistente pasa a la marca siguiente.

**Con la cámara** (si hay una alineada y visible): pulsa **Elegir en la imagen** y haz clic en el centro de cada marca sobre la imagen de la cámara. La posición sale de la alineación de la cámara, así que la medida es tan buena como esa alineación.

### 4. Revisa el resultado

El paso **Resultado** muestra una tabla con **Diseño (mm)**, **Medida (mm)** y **Error (mm)** de cada marca, y el resumen: **Error medio**, **Error máximo**, **Giro**, **Escala** y **Desplazamiento**.

Tlaku da el registro por **bueno** si el error máximo es de **0,3 mm o menos** y no hay avisos. Si el error es bajo pero hay avisos, dice que los revises antes de aceptar; si es mayor, que revises las posiciones guardadas o **Volver a medir**.

Si todo está bien, **Aceptar**. Desde ese momento la vista previa, el encuadre y el trabajo salen movidos según el registro, y el asistente muestra «Registro activo» con el error medio, el giro y la escala. **Restablecer** (o **Máquina > Quitar el registro de Print & Cut**) lo quita y el trabajo vuelve a salir donde dice el diseño.

### 5. Corta

Revisa la **vista previa**, que ya muestra el trabajo movido según el registro, **encuadra** con el láser apagado para comprobar que el rectángulo cae sobre la hoja y envía el trabajo como siempre ([Enviar trabajos con seguridad](09-enviar-con-seguridad.md)).

## Qué significa el error residual

Después de medir, Tlaku busca la transformación (un desplazamiento, un giro y, según el tipo de ajuste, una escala) que lleve **cada marca del diseño lo más cerca posible de donde la mediste**. El **error residual** es lo que sobra: la distancia, en milímetros, entre dónde **debería** estar cada marca con la transformación elegida y dónde **dijiste** que estaba.

- Un error **pequeño** (por debajo de 0,3 mm) significa que tus medidas y la hoja impresa son coherentes entre sí: se puede describir con un solo movimiento.
- Un error **grande** significa que **algo no cuadra**: mediste mal una marca, moviste la hoja entre una medida y otra, la impresora estiró el papel o una marca no es la que creías. No lo aceptes: **vuelve a medir**. Mira primero la marca con el error más grande.
- **No es la precisión del corte.** Es una medida de coherencia. El corte puede fallar por otras razones (backlash, el centro de la cruz que elegiste a ojo), y un error bajo no las detecta.

### Cuántas marcas y qué tipo de ajuste

| Tipo de ajuste | Qué corrige | Marcas |
|---|---|---|
| **Sin escala** | Solo desplazamiento y giro (supone que la impresión salió a tamaño real) | 2 o más |
| **Escala uniforme** | Desplazamiento, giro y una escala igual en X e Y | 2 o más |
| **Afín** | Todo lo anterior, y además estirones distintos en cada eje y sesgo | 3 o más, que no estén en línea recta |
| **Automático** | Con 2 marcas, escala uniforme; con 3 o más, afín | — |

**Un detalle importante con 2 marcas.** Con dos marcas y escala uniforme, siempre existe un desplazamiento, un giro y una escala que las unen **exactamente**: el error sale casi cero **aunque hayas medido mal**. Con dos marcas el error ya no te protege; mira entonces el **aviso de escala** y, sobre todo, usa **3 o 4 marcas** para tener un control real. Con **Sin escala** y 2 marcas, la diferencia entre la distancia impresa y la medida se reparte como error, lo que sí delata una escala rara.

Los avisos que da Tlaku:

- **Escala fuera de 100 % por más de 0,5 %:** la impresión puede estar deformada, la impresora escaló la hoja o una de las posiciones es mala.
- **Impresión espejada:** las marcas físicas están en otro orden que las del diseño. Revisa que mediste la marca 1 en la marca 1, y así.
- **Estirada distinto en cada eje** (solo con el ajuste afín): el papel se deformó, o la impresora imprime más largo en un sentido.

## Límites

- **Las imágenes giran con el registro.** Si la hoja está torcida, la imagen para grabar se gira con ella (se vuelve a muestrear, así que con un giro grande pierde un poco de nitidez). Su máscara se mueve con ella. Con un giro mínimo (menos de 0,006 °) se graba alineada a los ejes y no cambia nada.
- No se puede medir con el láser con el rotativo activo.
- El registro es parte de las **opciones del trabajo** de la sesión, no del archivo del proyecto: no se guarda con él, y si cierras la app o cambias la hoja de sitio, hay que medir otra vez.
- Se probó con la máquina virtual; con un láser real, empieza con la potencia al mínimo.

Siguiente: [Tiling y alimentador automático](09d-tiling-y-alimentador.md)
