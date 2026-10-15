# Pestañas de documentos

Abre **varios proyectos en la misma ventana**, cada uno en su pestaña, como en un navegador. Es algo que LightBurn no tiene (abre un proyecto por ventana, y sus *Pages* son otra cosa: hojas dentro de un mismo proyecto). Una pestaña es un documento completo: su diseño, sus capas y ajustes de corte, su historial de **Deshacer**, su archivo y su estado de guardado. Los atajos se escriben con `Ctrl/Cmd`: `Cmd` en macOS, `Ctrl` en Windows y Linux.

## La barra de pestañas

Está encima del lienzo. Cada pestaña muestra:

- el **nombre del archivo**, o **Sin título 1**, **Sin título 2**... si todavía no se guardó;
- un **punto naranja** cuando tiene cambios sin guardar (un lector de pantalla lo anuncia como «con cambios sin guardar»);
- una **impresora** si el trabajo que corre en la máquina se lanzó desde esa pestaña (ver [La máquina es una sola](#la-maquina-es-una-sola));
- una **X** para cerrarla. También cierra el clic con la rueda del ratón, `Supr` con la pestaña enfocada y `Ctrl/Cmd+W`.

El botón **+** (o `Ctrl/Cmd+T`) abre una pestaña vacía. Arrastra una pestaña para **reordenarla**. El clic derecho (o la tecla de menú, o `Mayús+F10`) abre su menú:

| Opción | Qué hace |
|---|---|
| Cerrar pestaña | Como la X; si tiene cambios, pregunta antes |
| Cerrar las demás pestañas | Deja solo esta. Si varias tienen cambios, una sola pregunta con la lista |
| Duplicar pestaña | Copia el documento a una pestaña nueva, a su derecha. La copia **no tiene archivo** y cuenta como con cambios sin guardar: guárdala para conservarla |
| Mover a la izquierda / a la derecha | Reordena sin arrastrar (sirve con teclado) |
| Mover a una ventana nueva | Pasa la pestaña a otra ventana de Tlaku (ver abajo). No está disponible si es la única pestaña |

> **Todavía no hay** «Mostrar en la carpeta»: la app no tiene permiso para abrir el explorador de archivos del sistema.

Con muchas pestañas, la barra se desplaza y la activa siempre queda a la vista. En el [modo sencillo](01b-modo-sencillo.md) la barra solo aparece si ya hay más de una pestaña.

## Qué es de cada pestaña y qué es de toda la ventana

| De cada pestaña | De toda la ventana |
|---|---|
| El diseño, las capas y los ajustes de corte | El **portapapeles**: copia en una pestaña y pega en otra (con los ajustes de las capas que usan) |
| El historial de **Deshacer** y **Rehacer** | La **máquina** (conexión, estado, trabajo en curso) y el perfil de máquina activo |
| El archivo y si hay cambios sin guardar | Las **preferencias** y los atajos |
| La selección, la capa activa y los nodos elegidos | La herramienta activa, el tema y la distribución de los paneles |
| El zoom y el paneo | La vista en vivo |
| La vista previa (visible, avance, resultado) y las **opciones del trabajo** (origen, orden de capas, optimización) | |

Una pestaña nueva hereda las opciones del trabajo de la que tenías (así no repites el origen del trabajo en cada documento), sin la selección ni el tiling.

## Abrir archivos

- **Abrir proyecto** (`Ctrl/Cmd+O`) y los **recientes** abren el proyecto en una **pestaña nueva**. Si la pestaña activa está **vacía y sin cambios**, se reemplaza en vez de dejar una pestaña vacía. Si el proyecto **ya está abierto**, Tlaku activa su pestaña y avisa.
- **Nuevo proyecto** (`Ctrl/Cmd+N`) abre una pestaña vacía; no hace falta guardar nada antes, porque no se pierde ningún documento. Lo mismo hacen **Nuevo desde plantilla**, las marcas de la cámara, la prueba de materiales y la prueba del rotativo: cada uno en su pestaña.
- **Arrastrar archivos** de diseño sobre el lienzo o **Importar** (`Ctrl/Cmd+I`) los agrega a la pestaña **activa**, como siempre. Si cambias de pestaña mientras se importa, el resultado entra en la pestaña donde lo pediste, no en la que miras ahora.
- Lo mismo vale para guardar, exportar, vectorizar, acomodar (nesting), insertar arte y las demás operaciones que tardan: terminan en su pestaña de origen. Si cerraste esa pestaña mientras tanto, Tlaku avisa y no escribe en otra.

## Atajos

| Acción | Atajo |
|---|---|
| Nueva pestaña | `Ctrl/Cmd+T` |
| Cerrar pestaña | `Ctrl/Cmd+W` |
| Pestaña siguiente / anterior | `Ctrl+Tab` / `Ctrl+Mayús+Tab` (en macOS también es `Ctrl`, porque `Cmd+Tab` lo usa el sistema) |
| Moverse por la barra (con el foco en ella) | `←` `→` (activan la pestaña), `Inicio`, `Fin` |
| Cerrar la pestaña enfocada | `Supr` |

No existen `Cmd+1..9` ni `Cmd+Opción+←/→`: ya tienen otro dueño (`Ctrl/Cmd+1..7` abren los paneles laterales y `Ctrl/Cmd+Alt+flechas` mueven el cabezal de la máquina), y Tlaku no reasigna atajos que ya existen. Puedes asignarle los que quieras a los comandos de pestañas en **Preferencias > Atajos**.

> **macOS:** el menú por defecto del sistema trae **Cerrar ventana** con `Cmd+W`. Tlaku lo quita para que `Cmd+W` cierre la pestaña; la ventana se cierra con su botón rojo.

## Cerrar con cambios

- **Una pestaña con cambios:** al cerrarla aparece **Cambios sin guardar** con **Guardar**, **No guardar** y **Cancelar**. Si era una pestaña de fondo, se activa para que veas cuál es. **Guardar** escribe el documento de esa pestaña; si el guardado falla, la pestaña no se cierra.
- **Cerrar la última pestaña** deja una pestaña vacía nueva (la ventana no se cierra).
- **Cerrar la ventana o salir** con una sola pestaña sucia: la pregunta de siempre. Con **varias**: un diálogo **Cambios sin guardar en N pestañas** con la **lista** de nombres y los botones **Guardar todas** (se guardan una por una; si una falla o cancelas el selector de archivo, se detiene y nada se cierra), **Descartar todas** y **Cancelar**.
- **Cerrar las demás** hace lo mismo con las otras pestañas.
- **Archivo > Guardar todas las pestañas** guarda todas las que tienen cambios.

## Recuperación y sesión

- **Una copia de recuperación por pestaña** con cambios (cada una con su propio nombre, por ejemplo «Sin título 2»). Se borra cuando guardas esa pestaña, vuelves con **Deshacer** al estado guardado o la cierras. Se conservan hasta 40 copias entre todas las ventanas.
- La copia de recuperación es para **no perder trabajo sin guardar**. Para **volver a un estado anterior ya guardado** está el [historial de versiones](03h-historial-de-versiones.md) (cada pestaña con archivo tiene el suyo).
- Tras un cierre inesperado, **Se encontró trabajo sin guardar** lista cada copia; cada **Recuperar** la abre en **su propia pestaña** (marcada como sin guardar), y **Recuperar todas** abre todas de una vez. Si el proyecto de origen ya está abierto y sin cambios, la copia ocupa esa pestaña en vez de duplicarla. Con varias ventanas, cada una solo ofrece copias huérfanas: las de otras ventanas abiertas no se mezclan.
- **Reabrir las pestañas al iniciar** (**Preferencias > General**, encendido de fábrica): al abrir Tlaku se vuelven a abrir los proyectos que tenían las pestañas al cerrar y queda activa la que lo estaba. Las pestañas **Sin título** no se reabren (si tenían trabajo sin guardar, lo ofrece la recuperación). Si un archivo ya no existe, se avisa y se sigue con los demás. Solo la ventana principal guarda y restaura la sesión.

## La máquina es una sola

La máquina, su conexión y el trabajo en curso son de toda la ventana; las pestañas no los duplican.

- Un trabajo **usa su propia copia**: cambiar de pestaña, o editar otra, no cambia lo que se está cortando. Si cierras la pestaña de origen, el trabajo sigue.
- La pestaña desde la que lanzaste el trabajo lleva una **impresora** mientras corre. La **vista en vivo** (y la del celular) sigue mostrando ese trabajo aunque cambies de pestaña, y el avance, el tiempo estimado y la capa en curso son los de ese trabajo.
- **Vista previa**, **Encuadrar** e **Iniciar** actúan sobre la pestaña **activa**, y lo dicen: la barra de esos botones se llama «Trabajo de *nombre*» cuando hay varias pestañas, y el diálogo de inicio dice **Se enviará «nombre» a *máquina***.
- Al terminar bien un trabajo, el avance automático del texto variable (serie y fila del CSV) se aplica en la pestaña de origen, no en la que estés viendo.

## Mover una pestaña a una ventana nueva

**Mover a una ventana nueva** abre otra ventana de Tlaku (ver [Nueva ventana](03e-guias-puentes-y-proyecto.md#nueva-ventana)) con ese documento, sus cambios sin guardar y su archivo, y cierra la pestaña aquí. El documento viaja como una copia de recuperación con id propio: si la ventana nueva no llegara a recogerlo, la copia queda disponible para recuperar al volver a abrir Tlaku. Si la ventana no se puede abrir, la pestaña se queda donde estaba.

## Memoria

Cada paso de **Deshacer** guarda la geometría que la acción reemplazó, así que varias pestañas con documentos grandes multiplican la memoria. La pestaña **activa** conserva su historial completo (hasta 200 pasos); una pestaña **en segundo plano** conserva los **30 pasos más recientes** y un presupuesto de unos 200 mil puntos de geometría (la décima parte del de la activa, unos 10 MB). Lo que se recorta es solo el historial antiguo: el documento y su estado de guardado no cambian. Medido con un documento de 6 000 objetos de 40 puntos y 150 ediciones por pestaña, el historial de una pestaña en segundo plano pasa de unos 243 mil puntos a 96 mil. Cinco de esas pestañas ocupan en total unos 350 MB en vez de unos 410 MB.

## Accesibilidad

La barra es un `tablist` con una pestaña (`tab`) por documento, y la zona del diseño es su `tabpanel`. Solo una pestaña es parada de `Tab`; con `←`/`→` se recorren y se activan. El nombre de cada pestaña incluye si tiene cambios sin guardar o un trabajo en curso. Los textos están en español e inglés.

Siguiente: [Texto](04-texto.md)
