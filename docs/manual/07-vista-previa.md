# Vista previa y orden de corte

La vista previa te muestra el recorrido exacto que hará el láser antes de enviar nada: en qué orden corta, cuánto se mueve en vacío y cuánto tardará. Además te explica por qué cada figura va donde va.

![Vista previa abierta: reproducción, leyenda, resultados y lista por capa](../../apps/desktop/docs/screenshots/04-vista-previa.png)

## Abrirla

- Pulsa el botón **Vista previa** de la barra superior, usa el menú **Ver** o la tecla `P`. La misma tecla la cierra.
- Aparece un panel bajo el lienzo, **Vista previa del trabajo**, y el recorrido se dibuja encima del diseño.
- Se recalcula sola cuando cambias objetos, capas, la máquina activa o las opciones del trabajo (con una pequeña espera; mientras tanto ves **Actualizando…**).
- Si no hay nada que mostrar verás "Importa un diseño para ver el recorrido".

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

Una capa con varias pasadas y bajada de Z aparece como una fila por pasada. Si la capa tiene puentes o pasadas, el resumen de la fila lo indica.

## El planificador explicable

Tlaku decide el orden de las figuras de cada capa de **Línea** con reglas fijas, de la más fuerte a la más débil:

1. **Niveles**: las capas se cortan en su orden; una capa termina antes de empezar la siguiente.
2. **Interiores antes que contenedor**: una figura dentro de otra se corta primero (el agujero antes que el contorno), para que la pieza no se suelte antes de tiempo. Esta regla no se rompe ni para ahorrar recorrido. También se aplica a figuras que se solapan en su mayor parte.
3. **Recorrido corto**: entre las figuras que ya pueden cortarse, va la más cercana, y después una mejora (2-opt y Or-opt) acorta el total.
4. **Punto de inicio y sentido**: cada figura cerrada empieza donde conviene y cada abierta se recorre en el sentido que conviene. Esto nunca cambia el orden entre figuras.

Al expandir una capa, cada figura trae su razón con palabras claras. Por ejemplo:

```
figura #3 (cerrada): va antes que la figura #1, que la contiene: interiores primero
figura #5 (abierta): la más cercana (12.40 mm) entre las que podían cortarse ahora
```

Así sabes si una figura salió en ese puesto por estar dentro de otra, por cercanía o por la mejora del recorrido. Si una regla no pudo cumplirse, aparece en **Advertencias**.

> **Ojo:** las explicaciones por figura existen solo para capas de **Línea**. En **Relleno** e **Imagen** el detalle muestra distancias, sin razones. Hoy no hay ajustes del planificador en la interfaz: usa los valores por defecto.

## Orden de las capas y advertencias

- **Orden de las capas** explica por qué se ejecutan así (por ejemplo, "grabados primero, cortes al final"). Cámbialo en la pestaña **Capas**; ver [Capas y ajustes de corte](05-capas-y-ajustes.md).
- **Advertencias** reúne avisos del cálculo: puentes frente al kerf, límites por software con el rotativo, ajustes que la máquina activa ignora (como Z sin eje Z), entre otros.

Antes de enviar, usa esta vista para revisar que lo que va primero es lo que quieres que vaya primero.

Siguiente: [Conectar tu máquina](08-conectar-maquina.md)
