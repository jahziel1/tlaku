# Cola e historial de trabajos

Dos herramientas para quien corta o graba varias piezas seguidas: la **cola de trabajos**, que te guía de un documento al siguiente sin dejar nunca la máquina sola, y el **historial de trabajos**, que recuerda cómo salió cada trabajo y con qué ajustes.

## Cola de trabajos

**Máquina > Cola de trabajos…** abre la lista de trabajos pendientes. Cada elemento es un **documento abierto en una pestaña** o un **archivo `.tlaku`** (que se abre en una pestaña cuando le toca), con su **número de copias** y, si quieres, un **aviso para el paso** («cambiar a MDF de 3 mm») que se muestra antes de empezar ese trabajo.

![El paso de confirmación entre dos trabajos de la cola](../../apps/desktop/docs/screenshots/72-cola-paso-confirmacion.png)

### La regla que no cambia: la cola nunca avanza sola

Un láser sin nadie mirando es un riesgo de incendio. Por eso la cola es **guiada**, no un modo desatendido:

1. **Entre un trabajo y el siguiente, la cola siempre se detiene** y abre **Siguiente trabajo de la cola**. Esto vale también para las copias de un mismo documento: cada copia pide su confirmación (sirve para cambiar la pieza).
2. Ahí tienes que marcar que **cambiaste el material o pusiste la pieza**, que **estás junto a la máquina con el área despejada** y las casillas de tu lista de comprobación (la del [diálogo de inicio](09-enviar-con-seguridad.md)). Mientras falte una, **Continuar** sigue deshabilitado.
3. Opcionalmente puedes **Encuadrar** el trabajo para ver dónde saldrá, **saltarlo**, **pausar** o **detener** la cola.
4. **Continuar no envía nada**: lleva el documento al frente y abre el **diálogo de inicio de siempre**, con su revisión previa (preflight) y su lista de comprobación. El trabajo sale cuando tú lo confirmas ahí.
5. Si cierras la ventana del paso con **Ahora no** (o con `Esc`), la cola sigue esperando; la píldora **Cola 2/5 · Esperando tu confirmación** (abajo a la izquierda) la vuelve a abrir.

Si un trabajo **falla o se cancela**, la cola **se detiene** y no ofrece el siguiente; al reintentar vuelve a pedir confirmación. Detener la cola con un trabajo ya en la máquina **no lo cancela**: sigue hasta el final y la cola no continúa después.

### Qué puedes hacer con la lista

- **Agregar** la pestaña actual, todas las pestañas abiertas o archivos `.tlaku`. Hasta 50 trabajos.
- **Ordenar** con las flechas, **cambiar las copias** (1 a 99), **saltar** un trabajo (se puede volver a ponerlo), **quitarlo** o **quitar los terminados**. Lo que está en la máquina no se puede quitar ni saltar.
- **Pausar la cola**: esperando confirmación, deja de pedirla hasta que **Reanudes**; con un trabajo en marcha, se pausa **al terminar** ese trabajo. Pausar la cola no pausa la máquina (para eso está **Pausar** del trabajo).
- Ver el **progreso total** (trabajos terminados, barra y porcentaje) y el **tiempo estimado que falta** (con lo que calculó la vista previa de cada pestaña, o lo que tardó la última vez ese archivo; si faltan datos lo dice).
- Si una pestaña sin archivo se cierra, su trabajo ya no se puede abrir: la cola se detiene al llegar a él y lo avisa.
- Se recuerda **qué archivos, cuántas copias y qué avisos** tiene la cola entre sesiones, pero la cola **siempre arranca detenida y sin copias hechas**: nada se envía hasta que la empieces y confirmes cada paso. Las pestañas sin guardar no se recuerdan.

La **vista en vivo** muestra en una línea en qué paso va la cola («Cola: trabajo 2 de 5 · "tapa.tlaku", copia 1 de 3»).

### Con el celular

El [control desde el celular](09e-control-desde-el-celular.md) **muestra la cola** (estado, avance, tiempo que falta, el trabajo actual y la lista) en una tarjeta de solo lectura. **Confirmar el siguiente trabajo desde el celular** tiene las mismas reglas que iniciar un trabajo desde él, más una: marcar el checklist en el teléfono (con una cuarta casilla, *Ya cambié el material o la pieza*) y que el documento que la cola espera esté **encuadrado en la computadora** y sea el de la pestaña activa. Si no se cumple, el celular dice por qué y se hace desde la computadora. El celular nunca puede avanzar la cola por otro camino.

## Historial de trabajos

**Ayuda > Historial de trabajos…** (antes «Registro de trabajos») guarda, por cada trabajo que enviaste a una máquina desde esta computadora:

- **Fecha y hora**, **máquina**, **documento** (nombre y ruta) y la **pestaña** desde la que salió.
- **Estimado** y **tiempo real** (sin las pausas), y el **resultado**: **Terminado**, **Cancelado**, **Con error**, **Interrumpido** (con el porcentaje y **Continuar** si queda punto de control; también si la app se cerró a mitad de un trabajo) o **En curso**.
- Las **capas** que salieron con su **velocidad, potencia, pasadas, modo e intervalo** (y ángulo, bidireccional, aire y ancho de corte), el **material** de tu biblioteca cuyos ajustes coinciden con los de la capa, el **área** de las formas y la **longitud de corte**.
- Una **miniatura** del recorrido y tus **notas** (se editan en el detalle y se guardan solas).

![El detalle de un trabajo del historial: datos, capas con sus ajustes, notas y acciones](../../apps/desktop/docs/screenshots/73-historial-detalle.png)

### Buscar y filtrar

Arriba de la tabla hay un buscador (documento, ruta, pestaña, máquina, material, capas, mensaje y notas; sin importar mayúsculas ni acentos) y filtros por **máquina**, **material**, **resultado** y **fechas** (desde y hasta). **Quitar filtros** los limpia.

### Repetir con estos ajustes

En el detalle, **Repetir con estos ajustes** abre el documento del trabajo (si ya está abierto, activa su pestaña; si el archivo ya no existe, ofrece usar la pestaña activa) y muestra **qué ajustes de las capas cambiarían** (velocidad, potencia, pasadas...). Al confirmar se aplican con **un paso de Deshacer**. **No envía nada a la máquina**: revisas la vista previa y lo envías con el diálogo de inicio de siempre.

### Exportar a CSV

**Exportar CSV** guarda las filas que estás viendo (con los filtros aplicados) en un `.csv` que abre cualquier hoja de cálculo: fecha, máquina, documento, ruta, pestaña, material, resultado, mensaje, tiempos estimado y real, área, longitud, resumen de capas y notas. Las celdas que parecen fórmulas se protegen.

### Dónde se guarda

Solo en esta computadora, en un archivo (`job-history.jsonl`, un trabajo por línea) dentro de la **carpeta de datos de la aplicación**; no viaja en el proyecto ni en los paquetes, no sale de tu equipo y las ventanas de Tlaku lo comparten. Guarda **hasta 1000 trabajos** por omisión (**Guardar hasta**, abajo en el historial, o **Preferencias > Archivos**: de 20 a 20 000); al pasarse se descartan los más viejos. El registro de la versión anterior (200 trabajos) se **migra solo** la primera vez. **Quitar del historial** borra un trabajo y **Borrar el registro** los borra todos (piden confirmación). Los trabajos del historial siguen alimentando el **estimado calibrado** de [Costos y precio](07c-costos-y-precio.md).

No registra la exportación de G-code ni lo que mandas desde la consola.
