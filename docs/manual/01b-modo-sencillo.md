# Modo sencillo

El modo sencillo es una versión guiada de Tlaku para quien nunca ha usado un láser. En lugar de paneles, menús y herramientas, ves **un paso a la vez** con botones grandes: abrir el diseño, elegir el material, colocarlo, revisar e iniciar. Tlaku pone la velocidad, la potencia y las pasadas por ti, a partir de la [biblioteca de materiales](11-materiales.md).

> **Ojo:** los ajustes que el modo sencillo elige son **puntos de partida**, no una garantía. Vienen de tablas publicadas por fabricantes y guías, no de tu máquina. Haz siempre una prueba pequeña en un retazo del mismo material antes de cortar o grabar tu diseño, y no dejes el láser sin vigilancia. Nada de esto se ha probado todavía con una máquina real: empieza con la máquina virtual.

## Cómo entrar y salir

La primera vez que abres Tlaku aparece la pregunta **¿Es tu primera vez con un láser?**

![Pregunta del primer arranque: sí quiero el modo sencillo, no quiero todas las herramientas, saltar](../../apps/desktop/docs/screenshots/58-primer-arranque.png)

- **Sí, quiero el modo sencillo** entra al modo sencillo.
- **No, quiero todas las herramientas** y **Saltar** dejan el modo completo, el de siempre.

La respuesta se guarda y la pregunta no vuelve a salir. Para cambiar de modo cuando quieras, usa el botón de la barra superior: dice **Modo sencillo** cuando estás en el completo y **Modo completo** cuando estás en el sencillo. El cambio **no toca tu diseño**: el documento, las capas y el historial de deshacer son los mismos en los dos modos, solo cambia lo que se muestra. Puedes hacer una parte del trabajo en un modo y seguir en el otro.

En el modo sencillo se esconden los menús, las herramientas del costado, la paleta de colores y el panel acoplado; el lienzo y la barra superior se quedan, y a la derecha aparece el panel de pasos.

## Los cinco pasos

Arriba del panel hay una barra con los cinco pasos: **1. Diseño**, **2. Material**, **3. Colocar**, **4. Revisar** y **5. Iniciar**. Avanzas con **Siguiente** y retrocedes con **Atrás**; también puedes volver a cualquier paso ya recorrido tocándolo en la barra. **Siguiente** se desactiva y dice por qué cuando falta algo (por ejemplo, "Elige un material para seguir").

### Paso 1: tu diseño

Tres tarjetas grandes: **Abrir un archivo** (SVG, DXF, PDF, imágenes y más, como en [Importar y exportar](03b-importar-y-exportar.md)), **Biblioteca de arte** (corazones, marcos, estrellas, cajas) y **Probar con un ejemplo**, un dibujo listo para practicar. Cuando ya hay un diseño, el paso dice cuántos objetos tiene, en cuántos colores y cuánto mide.

### Paso 2: material y qué hacer con cada color

Aquí está el corazón del modo sencillo.

![Paso Material: 20 W elegido, contrachapado de abedul 3 mm y los avisos de valores de partida](../../apps/desktop/docs/screenshots/59-simple-material.png)

1. **Tu máquina**: elige el perfil. Con la máquina conectada no se puede cambiar.
2. **¿De cuántos watts es tu láser?** Tlaku necesita la potencia para elegir los ajustes de la clase correcta (por ejemplo, **Diodo 20 W**). Elige el valor más cercano; suele estar escrito en el láser o en su caja. Se recuerda para esa máquina. Si eliges **No lo sé**, Tlaku usa los ajustes **más prudentes** y lo dice en los avisos. Si no hay ajustes para tu potencia exacta, usa los de la clase más parecida y también te avisa.
3. **Material**: la lista sale de la biblioteca de fábrica y de tus propios materiales (solo los que tienen ajustes para tu máquina). Debajo se explica de dónde salió cada ajuste ("Cortar: Ajuste para tu clase de máquina").
4. **Colores del diseño**: una tarjeta por cada color (cada capa con objetos). Para cada una eliges:
   - **Cortar**: atraviesa el material siguiendo el trazo.
   - **Grabar**: marca la superficie (rellenos e imágenes).
   - **Ignorar**: ese color no se envía a la máquina.

   Las imágenes solo se pueden grabar o ignorar. Si una acción no es posible, el botón se desactiva y debajo dice por qué: "Este material no tiene ajustes de corte", "Este material solo tiene ajustes para otra máquina o para otra potencia", etcétera.

Mientras decides, el panel muestra los **avisos** en recuadros: que son valores de partida (con el botón **Hacer la prueba de materiales**), si la fuente da un rango de valores, si hay textos sin convertir a curvas o trazos abiertos en lo que vas a cortar.

**Acrílico transparente con diodo.** Un láser de diodo no corta el acrílico transparente (la luz lo atraviesa). Si eliges ese material con una máquina de diodo, **Cortar** queda desactivado en todas las tarjetas, con la explicación "Este material no se puede cortar con tu tipo de láser". Usa acrílico opaco o de color, o un láser de CO2.

![Acrílico transparente con diodo: Cortar desactivado con su explicación](../../apps/desktop/docs/screenshots/59c-simple-acrilico.png)

#### Ajustar, Ver números y Guardar como mi material

En cada color que se corta o se graba, debajo de los valores (**Velocidad**, **Potencia**, **Pasadas**) hay un botón **Ajustar** con controles de una sola frase:

- **Cortar**: **No atraviesa: más pasadas** y **Quema mucho: menos potencia y más velocidad**.
- **Grabar**: un control de **Más claro o más oscuro**, de "Mucho más claro" a "Mucho más oscuro", con "Como dice el material" en el centro.

**Ver números** despliega los valores exactos para escribirlos a mano. Lo que cambies se lista como "Lo que cambiaste respecto del material" (por ejemplo, `Velocidad: 150 mm/min → 165 mm/min`). Con **Volver al material** regresas a los valores originales. Y con **Guardar como mi material** lo que te funcionó pasa a tu biblioteca (como "Mi corte" o "Mi grabado") para usarlo después, también en el modo completo.

![Ajustar abierto en Corte: un cambio hecho, Ver números desplegado, Volver al material y Guardar como mi material](../../apps/desktop/docs/screenshots/59b-simple-ajustar.png)

### Paso 3: colócalo en la cama

Dice cuánto mide tu diseño y cuánto la cama. Puedes **arrastrar el diseño en el lienzo** o llevarlo con los nueve botones de posición (esquinas, bordes y **Centrar en la cama**). Si el diseño se sale de la cama, aparece un aviso con dos salidas: **Reducir para que quepa** o **Dividir en tramos** (ver [Tiling y alimentador automático](09d-tiling-y-alimentador.md)). No puedes avanzar mientras se salga.

### Paso 4: revisa

Muestra el **tiempo estimado** y el tiempo de cada color, y dibuja el recorrido del láser sobre el lienzo, con la vista previa de siempre para reproducirlo (ver [Vista previa](07-vista-previa.md)). Con **Encuadrar** el láser recorre el borde del diseño con el haz apagado, para que veas dónde va a trabajar.

![Paso Revisar: tiempo estimado, tiempo por color y la vista previa del recorrido](../../apps/desktop/docs/screenshots/60-simple-revisar.png)

### Paso 5: iniciar

Un resumen de lo que va a pasar (máquina, material y qué se hace con cada color), el puerto de la máquina y los avisos que sigan pendientes. Si no hay máquina conectada, el aviso ofrece **Conecta tu máquina** o **Probar con la máquina virtual**, que no enciende ningún láser.

![Paso Iniciar con la máquina virtual conectada y el botón Iniciar](../../apps/desktop/docs/screenshots/60b-simple-iniciar.png)

**Iniciar** no arranca el láser de inmediato: antes se muestra la **lista de comprobación de seguridad** de siempre, y el trabajo pasa por la misma revisión previa (preflight) que en el modo completo. Mira [Enviar trabajos con seguridad](09-enviar-con-seguridad.md).

## Lo que el modo sencillo no hace

- No sustituye la prueba de materiales: los valores son de partida. Para encontrar los de tu máquina, usa **Hacer la prueba de materiales** ([Materiales](11-materiales.md)).
- No tiene todas las operaciones (offset, nesting, texto variable, Print & Cut, galvo, cámara...). Para eso, cambia al modo completo con el botón de la barra superior; tu diseño te sigue.
- Si no hay materiales con ajustes para tu máquina, la lista sale vacía y el panel te manda al modo completo: agrega uno en la pestaña **Materiales**.

Siguiente: [La interfaz](02-interfaz.md)
