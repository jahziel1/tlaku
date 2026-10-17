# Rotativo y eje Z

Dos accesorios que cambian cómo se genera el G-code: el eje Z motorizado (para enfocar por capa y bajar en cada pasada) y el rotativo (para grabar piezas cilíndricas en lugar de planas). Los dos se activan en el perfil de la máquina.

> **Ojo:** ni el rotativo ni el eje Z se han probado con una máquina real (Tlaku sí se probó con una Falcon2 Pro, pero sin rotativo ni eje Z); esto se validó con el emulador y con pruebas automáticas. Los modelos de rotativo precargados usan cifras de fabricante o de reseñas: calibra siempre con la **Prueba de una vuelta** y empieza con poca potencia y una pieza de desecho.

## Eje Z

### Activarlo en el perfil

1. Abre el menú **Máquina**, **Gestionar dispositivos…**, y elige tu máquina.
2. En el grupo **Ejes y accesorios**, marca **Eje Z motorizado**. Hazlo solo si tu máquina mueve el enfoque con un motor.
3. Aparece **Invertir Z**. Déjalo apagado, a menos que en tu máquina +Z acerque el cabezal o suba la cama.
4. **Guardar cambios**.

Sin esta casilla, los ajustes de Z de las capas se ignoran al enviar, y la capa muestra un aviso.

### Usarlo en una capa

Con el eje Z activo, el editor de la capa tiene el grupo **Eje Z**:

| Ajuste | Qué hace |
|---|---|
| **Desplazamiento Z** | Mueve el foco al empezar la capa. Positivo = el foco entra más adentro del material (el cabezal baja o la cama sube). Negativo, hacia fuera |
| **Bajada por pasada** | Milímetros extra que baja el foco en cada pasada después de la primera, para cortes profundos. `0` = todas las pasadas a la misma altura |

Si la capa tiene ajustes de Z y la máquina no tiene eje Z, ves un aviso y el botón **Poner Z en 0**.

### Cómo se comporta

- El cero es **donde enfocaste tú**: Tlaku no conoce el cero de Z de la máquina, así que antes de iniciar enfoca a mano sobre la superficie del material. El movimiento de Z se envía **relativo** a esa altura, y cada capa vuelve a ella al terminar.
- La altura de cada pasada es `Desplazamiento Z + Bajada por pasada × (pasada − 1)`. Con bajada distinta de cero y varias pasadas, la capa se divide en una fila por pasada en la [vista previa](07-vista-previa.md) ("pasada 2/3").
- **Prueba de sentido:** con un desplazamiento Z de 1 mm, el foco debe entrar 1 mm en el material. Si sale al revés, activa **Invertir Z**. Haz la prueba sin material y con el láser apagado o a potencia mínima.
- Hoy el tiempo estimado no suma lo que tarda Z en moverse. Con el eje Z activo, **Mover cabezal** tiene los botones **Subir** y **Bajar** y **Enfocar** (lleva Z a una altura de enfoque guardada), explicados en [Control avanzado de la máquina](09b-control-avanzado.md). No hay autoenfoque ni optimizar Z.

## Rotativo

El rotativo **sustituye al eje Y**: el diseño se dibuja en milímetros sobre la superficie de la pieza, con X a lo largo de la pieza e Y alrededor de ella, y Tlaku convierte esa Y a los pasos que la controladora debe dar al generar el G-code. No usa un eje A o U aparte. La velocidad de cada capa se interpreta sobre la superficie de la pieza.

### Configurarlo

En **Dispositivos**, grupo **Ejes y accesorios**:

1. Marca **Usar un rotativo en lugar del eje Y**.
2. Si tu accesorio está en la lista, elige el **Modelo de rotativo**. Rellena tipo y medidas con las cifras del fabricante.

| Modelo | Tipo | mm por vuelta | Rodillo |
|---|---|---|---|
| xTool RA2 Pro, rodillos | Rodillos | 32 | 17 mm |
| xTool RA2 Pro, mandril | Mandril | 128 | no aplica |
| Creality, mandril | Mandril | 40 | no aplica |

3. Si no está, elige a mano el **Tipo de rotativo**: **Rodillos** (la pieza descansa sobre dos rodillos y gira con ellos) o **Mandril** (la pieza se sujeta por un extremo). Los valores de partida son 32 mm por vuelta para rodillos y 40 para mandril.
4. **mm por vuelta**: cuántos mm de Y debe recorrer la controladora para que el rotativo dé una vuelta completa.
5. **Diámetro del rodillo** (solo rodillos) y **Diámetro de la pieza** en mm. Con rodillos, lo que mueve la pieza es el diámetro del rodillo; el de la pieza solo sirve para la circunferencia y la prueba. El perfil te dice "Una vuelta de la pieza mide X mm sobre su superficie".
6. **Espejo** (**Invertir el sentido de giro**): actívalo si el diseño sale al revés alrededor de la pieza, algo que depende de cómo montaste el motor. Solo afecta al G-code, no al área del trabajo.
7. **Guardar cambios**.

![Perfil con el rotativo activado: modelo, tipo, mm por vuelta, diámetro y espejo](../../apps/desktop/docs/screenshots/22-maquina-rotativo.png)

### Calibrar con la prueba de una vuelta

Es el paso que no debes saltarte. Al final del grupo del rotativo está **Prueba de una vuelta**.

1. Guarda antes los cambios del perfil y asegúrate de que esa máquina sea la activa (la prueba se calcula con el perfil guardado).
2. Ajusta **Velocidad de la prueba** (1500 mm/min por defecto) y **Potencia de la prueba** (10 % por defecto): solo debe verse el trazo, sin quemar la pieza.
3. Pulsa **Probar una vuelta**. Se abre un documento nuevo con una sola línea del largo exacto de una vuelta (π por el diámetro de la pieza).
4. Marca la pieza donde empieza el trazo, envíalo y mira dónde termina. El final de la línea debe caer sobre su principio.
5. Si el trazo se queda corto, sube **mm por vuelta**; si se pasa, bájalo. Es proporcional: si la pieza solo giró el 90 % de la vuelta, divide el valor actual entre 0,9. Repite hasta que cierre.

Con rodillos debe dar una vuelta completa el rodillo motriz; con mandril, la pieza.

### Límites por software

El rotativo no tiene final de carrera, y su Y de controladora puede pasar de la medida de la cama. Si el trabajo la rebasa, la vista previa avisa en **Advertencias** que, con los límites por software activos (`$20=1`), la máquina daría `ALARM:2`. La solución que propone la app es desactivar los límites (`$20=0`) mientras uses el rotativo. Puedes enviar ese ajuste con la consola ([Enviar trabajos con seguridad](09-enviar-con-seguridad.md)), que te pide confirmación.

### Qué cambia en la ventana Máquina

Con el rotativo activo, **Posicionar el láser** (un clic en la cama) se deshabilita y dice por qué: la Y es el giro de la pieza, no un punto de la cama. Los encuadres (el rectángulo y **Encuadrar contorno**) sí funcionan y usan la escala del rotativo, igual que el trabajo.

### Lo que no existe

El rotativo para galvo (girar la pieza entre franjas) existe solo como cálculo en el motor; la corrección de cilindro y de cono es una función del galvo y se maneja desde el perfil de una máquina galvo: ver [Grabado 3D por cortes, cilindro y cono](12b-grabado-3d-y-cilindro.md) y [Galvo (experimental)](13c-galvo.md). Un eje A o U aparte se mueve a mano (casilla **Eje U** del perfil y botones en **Mover cabezal**) y, si quieres, también dentro del trabajo (ver [El eje U en el trabajo y por capa](#el-eje-u-en-el-trabajo-y-por-capa)). Con una controladora Ruida el rotativo se activa en los ajustes de la propia máquina: Tlaku manda la Y en mm de superficie, sin escalarla, y no escribe esos ajustes.

## El eje U en el trabajo y por capa

Si tu máquina tiene un eje extra (U) que responde a `G0 U..`, marca **Eje U** en el perfil. Hay dos desplazamientos, los dos relativos y con el láser apagado (Tlaku no conoce el cero de U):

- **Desplazamiento U de los trabajos** (en el perfil): el eje se mueve esa distancia al empezar el trabajo y vuelve al terminar.
- **Desplazamiento U** de cada **capa** (en el editor de capa, grupo principal): la posición de U con la que se ejecuta esa capa, **sumada** a la del trabajo y relativa a donde estaba U al empezar. Entre dos capas Tlaku manda solo la diferencia, y al final deshace el total. Sirve, por ejemplo, para que cada capa grabe a una altura o en una posición distinta del eje. El campo solo aparece con una máquina con eje U (o si la capa ya trae un valor); si la máquina no tiene eje U, el valor se ignora y la vista previa lo avisa.

Con **0** en todas las capas el G-code es el de siempre. El movimiento sale como `G91`, `G0 U..`, `G90` al empezar la capa, antes de grabar nada. *Por validar con una máquina real con eje U: solo se comprobó el G-code y el emulador.* Esto no es el rotativo: el rotativo sustituye al eje Y.

Siguiente: [Grabado 3D por cortes, cilindro y cono](12b-grabado-3d-y-cilindro.md)
