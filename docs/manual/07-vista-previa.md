# Vista previa y orden de corte

La vista previa te muestra el recorrido exacto que hará el láser antes de enviar nada: en qué orden corta, cuánto se mueve en vacío y cuánto tardará. Además te explica por qué cada figura va donde va.

![Vista previa abierta: reproducción, leyenda, resultados y lista por capa](../../apps/desktop/docs/screenshots/04-vista-previa.png)

## Abrirla

- Pulsa el botón **Vista previa** de la barra superior, usa el menú **Ver** o la tecla `P`. La misma tecla la cierra.
- Aparece un panel bajo el lienzo, **Vista previa del trabajo**, y el recorrido se dibuja encima del diseño.
- Se recalcula sola cuando cambias objetos, capas, la máquina activa o las opciones del trabajo (con una pequeña espera; mientras tanto ves **Actualizando…**).
- Si no hay nada que mostrar verás "Importa un diseño para ver el recorrido".
- En la barra del panel están el botón **Ajustes de optimización…** (ver más abajo) y las casillas **Solo la selección** y **Usar el origen de la selección** (ver [Enviar trabajos con seguridad](09-enviar-con-seguridad.md)). Con **Solo la selección** encendida, la vista previa muestra únicamente lo que tengas seleccionado y se recalcula al cambiar la selección.

> **Consejo:** si el cálculo falla, el panel dice **No se pudo calcular la vista previa** y el motivo. Dos casos que la app reconoce y para los que da una pista: el diseño se sale de la cama (muévelo o reduce el overscan de las capas de relleno e imagen) y ninguna capa con objetos tiene la salida activada. Con **Reintentar** vuelves a calcular.

## Reproducir el recorrido

| Control | Qué hace |
|---|---|
| Volver al inicio / Ir al final | Saltan al principio o al final del trabajo |
| Reproducir / Pausar (`K`) | Anima el cabezal siguiendo el tiempo real del trabajo |
| `1x` `2x` `5x` `10x` `50x` | Velocidad de la animación (`,` la baja y `.` la sube) |
| Barra **Posición en el recorrido** | Arrástrala para ir a cualquier instante; las marcas indican dónde empieza cada capa |

A la derecha de la barra ves "tiempo actual de total", por ejemplo `10:02 de 10:02`. Los atajos `K`, `,` y `.` solo funcionan con la vista previa abierta y ya calculada.

## Leyenda y colores

Cada tipo de movimiento tiene su color:

- **Rápidos**: desplazamientos con el láser apagado (trazo discontinuo). Puedes ocultarlos.
- **Corte** y **Grabado**: movimientos con el láser encendido.
- **Overscan**: tramos extra fuera de la pieza para que el cabezal llegue a velocidad. Puedes ocultarlos.

Cuanto más transparente sea un trazo, menos potencia lleva.

## Resultados

- **Tiempo total**, con la etiqueta **Estimado**. Se calcula con la aceleración del perfil de la máquina. El tiempo real puede cambiar, sobre todo con imágenes.
- Si ves **aprox.** o `≈`, el motor no entregó el tiempo de cada movimiento y se estimó por distancia y velocidad. En ese caso no se pueden resaltar capas.
- Distancia y tiempo por tipo: corte, grabado, rápidos y overscan.

## Por capa

La sección **Por capa** lista cada capa en el orden en que se ejecutará, con su modo (**Línea**, **Relleno** o **Imagen**), su tiempo y una barra con su parte del total.

- Si el planificador acortó los desplazamientos en vacío, verás "Ahorra N % de recorrido en vacío".
- El icono de mira (**Resaltar en el lienzo**) atenúa el resto y marca solo esa capa.
- Con la flecha de la fila (**Ver cómo se planificó**) se abre el detalle: distancias de esa capa y la explicación de cada figura.

Una capa con varias pasadas y bajada de Z aparece como una fila por pasada. Si la capa tiene puentes o pasadas, el resumen de la fila lo indica. Una capa con [operaciones adicionales](05-capas-y-ajustes.md) aparece como un bloque por operación, con la posición entre las de su capa (por ejemplo "2/3" en el nombre).

## Ajustes de optimización

Con **Ajustes de optimización…** (en la barra de la vista previa, en el panel **Máquina** y en el menú **Máquina**) decides cómo se ordena y se recorre el trabajo. Los cambios se aplican al instante (la vista previa se recalcula sola) y valen para la vista previa, el encuadre, el envío y la exportación de G-code, para que lo que ves sea lo que se corta. Se recuerdan entre sesiones, pero son una preferencia de la aplicación: **no se guardan en el proyecto**. **Valores de fábrica** los restablece. Cuando algo no está en su valor de fábrica, el apartado **Orden de las capas** de los resultados lo resume en una frase que empieza por "optimización:".

![Ventana Ajustes de optimización con las opciones de orden de corte, recorrido y resultado](../../apps/desktop/docs/screenshots/35-optimizacion.png)

| Opción | Qué hace y cuándo cambiarla |
|---|---|
| **Respetar el orden de las capas** (encendida) | Termina una capa antes de empezar la siguiente. Apagada, el motor puede **mezclar capas de Línea consecutivas que tengan exactamente los mismos ajustes** (y ninguna con posición fija) para ahorrar recorrido; las capas con ajustes distintos nunca se mezclan, y el resumen dice cuáles se juntaron. Útil si tienes, por ejemplo, un corte rojo y otro azul con la misma velocidad y potencia y piezas repartidas por toda la cama. |
| **Terminar cada grupo antes del siguiente** (apagada) | Corta todo lo de un grupo (los que haces con **Agrupar**) antes de pasar a otro. Útil para piezas armadas con varios objetos que deben salir completas, una por una. |
| **Respetar la prioridad de los objetos** (encendida) | Los objetos con prioridad menor se cortan primero, antes de optimizar el orden. Ver [Dibujar y editar](03-dibujar-y-editar.md). Apágala si importaste un DXF con prioridades que no te interesan. |
| **Cortar primero las formas interiores** (encendida) | Los agujeros salen antes que el contorno que los rodea, para que la pieza no se mueva al soltarse. Desactívala solo si sabes por qué. |
| **Elegir el mejor punto de inicio** (encendida) | En las formas cerradas empieza por el vértice más cercano al punto anterior, en lugar del primero que trae el dibujo. Apagada, cada forma empieza donde fue dibujada. |
| **Permitir invertir el sentido** (encendida) | Puede recorrer una forma abierta al revés si así acorta el desplazamiento. Apágala si el sentido importa. |
| **Reducir los recorridos en vacío** (encendida) | Ordena las figuras por cercanía y aplica mejoras locales para que el cabezal viaje menos sin cortar. Apagada, se conserva el orden original. |
| **Quitar tramos que se cortarían dos veces** (apagada), con su **Tolerancia de solapes** (0,001 a 1 mm; 0,01 de fábrica) | Detecta segmentos duplicados entre formas distintas y los corta una sola vez. Ejemplo: dos piezas pegadas con un borde en común, que sin esto se quema dos veces; la segunda pasada oscurece y ensancha el corte. Solo en capas de Línea y **no con kerf** (la compensación ya cambió las formas). La vista previa avisa cuántos milímetros quitó. |
| **Esconder el juego mecánico** (apagada) | Recorre las formas cerradas siempre en sentido antihorario y graba los rellenos y las imágenes en un solo sentido, para que el juego de correas y engranes no se note en las uniones y las líneas no se desalineen. Es **más lento**, porque hay más viajes en vacío; la vista previa lo recuerda. Úsalo con máquinas con holgura notable, donde las piezas que "cierran" salen con un escalón. |

> **Consejo:** si dudas, deja todo en su valor de fábrica con **Valores de fábrica**: el orden de capas, los interiores primero y el recorrido corto ya vienen encendidos.

## El planificador explicable

Tlaku decide el orden de las figuras de cada capa de **Línea** con reglas fijas, de la más fuerte a la más débil:

1. **Niveles**: las capas se cortan en su orden; una capa termina antes de empezar la siguiente. Si lo pides en los ajustes de optimización, dentro de cada capa se termina un **grupo** antes del siguiente y, dentro de cada grupo, se respeta la **prioridad** de cada objeto (menor número, antes). La precedencia es siempre capa, grupo, prioridad.
2. **Interiores antes que contenedor**: una figura dentro de otra se corta primero (el agujero antes que el contorno), para que la pieza no se suelte antes de tiempo. Esta regla no se rompe ni para ahorrar recorrido. También se aplica a figuras que se solapan en su mayor parte.
3. **Recorrido corto**: entre las figuras que ya pueden cortarse, va la más cercana, y después una mejora (2-opt y Or-opt) acorta el total.
4. **Punto de inicio y sentido**: cada figura cerrada empieza donde conviene y cada abierta se recorre en el sentido que conviene. Esto nunca cambia el orden entre figuras.

Al expandir una capa, cada figura trae su razón con palabras claras. Por ejemplo:

```
figura #3 (cerrada): va antes que la figura #1, que la contiene: interiores primero
figura #5 (abierta): la más cercana (12.40 mm) entre las que podían cortarse ahora
```

Así sabes si una figura salió en ese puesto por estar dentro de otra, por cercanía o por la mejora del recorrido. Si una regla no pudo cumplirse, aparece en **Advertencias**.

> **Ojo:** las explicaciones por figura existen solo para capas de **Línea**. En **Relleno** e **Imagen** el detalle muestra distancias, sin razones. La prioridad por objeto y la opción de quitar tramos repetidos también valen solo en capas de Línea (la escala de potencia por objeto vale en Línea y en Relleno).

## Orden de las capas y advertencias

- **Orden de las capas** explica por qué se ejecutan así (por ejemplo, "grabados primero, cortes al final"). Cámbialo en la pestaña **Capas**; ver [Capas y ajustes de corte](05-capas-y-ajustes.md).
- **Advertencias** reúne avisos del cálculo: puentes frente al kerf, límites por software con el rotativo, ajustes que la máquina activa ignora (como Z sin eje Z), entre otros.

Antes de enviar, usa esta vista para revisar que lo que va primero es lo que quieres que vaya primero.

Siguiente: [Conectar tu máquina](08-conectar-maquina.md)
