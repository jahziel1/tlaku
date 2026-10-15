# Historial de versiones

Guardar sobrescribe el archivo, y **Deshacer** se pierde al cerrar. El **historial de versiones** conserva cómo estaba el proyecto en el pasado para que puedas **volver a un estado anterior aunque ya lo hayas guardado**. Todo es **local**: las versiones no salen de tu computadora.

Se abre con **Archivo > Historial de versiones…**. **Archivo > Guardar versión…** guarda una versión con nombre.

## Qué se conserva y cuándo

| Tipo | Cuándo se crea | Qué guarda |
|---|---|---|
| **Guardado** | Cada vez que guardas un proyecto que ya tenía archivo | Lo que tenía el archivo **justo antes** de sobrescribirlo (con la fecha del archivo). El primer guardado no crea ninguna: todavía no había nada que conservar |
| **Automática** | Cada pocos minutos (5 de fábrica) mientras editas, solo si hay cambios sin guardar | El documento tal como está en pantalla, aunque no lo hayas guardado |
| **Con nombre** | **Archivo > Guardar versión…** | El documento tal como está ahora (con los cambios sin guardar) y el nombre que escribas. **Nunca se borran solas** |
| **Antes de restaurar** | Al restaurar reemplazando el documento (ver abajo) | El estado que tenías, para poder volver |

- **No se repite lo que no cambió**: si el contenido es igual al de la versión más reciente, no se guarda otra. Si guardas una versión con nombre de un documento que no cambió desde la última, esa versión pasa a llevar tu nombre.
- Los proyectos **sin archivo** («Sin título») no tienen historial: los protege la copia de recuperación (ver [Pestañas de documentos](03f-pestanas-de-documentos.md#recuperacion-y-sesion)). En cuanto los guardas, empiezan a tener historial.
- Cada pestaña tiene su propio historial (el del archivo que muestra). Guardar una pestaña de fondo conserva la versión de **su** archivo.
- Las versiones se comprimen (gzip): un proyecto típico ocupa unos pocos KB por versión.

> **Las imágenes no viajan dentro del proyecto**: el `.tlaku` guarda su **ruta**. Una versión antigua recupera el diseño tal como estaba, pero si moviste, borraste o cambiaste el archivo de imagen, la versión mostrará la imagen nueva o no la encontrará. Lo mismo vale para los CSV de los datos variables.

## El diálogo

A la izquierda, la lista de versiones, **de la más nueva a la más vieja**: miniatura, nombre o fecha, tipo (guardado, automática, con nombre, antes de restaurar), cantidad de objetos y capas, y tamaño. Al elegir una versión, a la derecha ves:

- una **vista previa** grande (no toca tu documento);
- **qué cambiaría al restaurarla**, comparada con el documento abierto: cuántos objetos que agregaste después se perderían, cuántos que quitaste volverían, cuántos volverían a su estado anterior, y las capas, notas, guías o dispositivo que cambiarían. Si el archivo no está abierto en ninguna pestaña, no hay con qué comparar;
- los botones de abajo.

### Restaurar

- **Restaurar como pestaña nueva**: abre la versión en una pestaña **sin archivo** y con cambios sin guardar. Tu documento no se toca. Guárdala con otro nombre para conservarla.
- **Reemplazar el documento abierto…**: pide confirmación y reemplaza el documento de la pestaña que tiene ese archivo. Antes guarda lo que tenías como versión **«Antes de restaurar»**, y el reemplazo es **un solo paso de Deshacer**: con **Deshacer** vuelves a como estabas, y si ya cerraste o guardaste, esa versión sigue en la lista. Si no se puede guardar ese respaldo (por ejemplo, el disco está lleno), no se reemplaza nada. Solo está disponible si el archivo está abierto en una pestaña.
- El documento queda **sin guardar** tras reemplazar: el archivo no cambia hasta que guardes. Al guardar, lo que tenía el archivo pasa a ser una versión «Guardado».

### Otras acciones

- **Resaltar en el lienzo lo que cambia**: selecciona en el documento abierto los objetos que agregaste o modificaste desde esa versión (los que se perderían o cambiarían al restaurar).
- **Borrar esta versión**: pide confirmación y la quita para siempre.
- **Historial de otro archivo…**: elige un `.tlaku` y abre su historial **aunque no esté abierto** (solo puedes restaurarlo como pestaña nueva). Si guardaste las versiones en la carpeta de datos de la app, la lista **Otros proyectos con historial** muestra todos los proyectos que tienen versiones, también los que **moviste o renombraste** (un archivo se identifica por su ruta: al moverlo empieza un historial nuevo y el viejo sigue en esa lista).

## Dónde se guardan

Por omisión, en la **carpeta de datos de la aplicación**, en una subcarpeta `versiones` con una carpeta por proyecto:

| Sistema | Carpeta |
|---|---|
| macOS | `~/Library/Application Support/org.tlaku.app/versiones/` |
| Windows | `%APPDATA%\org.tlaku.app\versiones\` |
| Linux | `~/.local/share/org.tlaku.app/versiones/` |

Esto no ensucia tus carpetas ni viaja por error con el proyecto cuando lo mandas por correo o lo sincronizas con una nube (las versiones contienen el diseño completo, por ejemplo el de un cliente). Si prefieres que **viajen con el archivo**, en **Ajustes del historial** elige **Junto al archivo**: se guardan en una carpeta oculta `.tlaku-versions` al lado del proyecto. Al cambiar de lugar, el historial anterior sigue visible; solo lo nuevo va al lugar elegido. Para **borrar todo el historial de un proyecto** usa **Borrar todo el historial de este proyecto…** (pide confirmación).

## Ajustes del historial

Se cambian en el propio diálogo (**Ajustes del historial**) y se aplican con **Aplicar**:

| Ajuste | De fábrica | Qué hace |
|---|---|---|
| Conservar la versión anterior en cada guardado y hacer instantáneas | Sí | Apagado, no se guardan versiones solas (las con nombre siguen funcionando) |
| Dónde guardar | Carpeta de datos de la app | O junto al archivo, en `.tlaku-versions` |
| Instantánea automática cada | 5 min | `0` = nunca |
| Conservar todas las de las últimas | 24 h | |
| Después, una por día hasta hace | 30 días | De cada día queda la más nueva |
| Después, una por semana hasta hace | 26 semanas | Más viejo que eso, lo automático se borra |
| Tope de tamaño por proyecto | 200 MB | Lo que ocupan en disco |
| Tope de versiones por proyecto | 500 | |

Después de cada versión nueva se hace la **limpieza**: se aplican los escalones por antigüedad y luego los topes (borrando primero lo más viejo). La limpieza **nunca borra la versión más reciente ni las versiones con nombre**, así que si solo quedan esas, un tope puede quedar superado. Los días y las semanas se cuentan en UTC.

## Si algo falla

- **El guardado del proyecto no depende del historial.** Si no se puede escribir la versión (disco lleno, sin permisos), Tlaku lo avisa y **guarda el proyecto de todos modos**.
- Las escrituras son atómicas: un corte de luz a media escritura no deja una versión a medias; los restos se limpian solos.
- Si el historial de una versión se ve vacío en el diálogo pero hay archivos en la carpeta, revisa el lugar elegido: el diálogo mira los dos.
- Un proyecto con **más de 128 MB** no se guarda en el historial.

## Lo que no hace

- No es control de versiones con ramas ni mezcla: es una línea de tiempo por archivo.
- No protege los archivos de imagen ni los CSV externos (ver arriba).
- No sincroniza entre computadoras: todo es local. Con **Junto al archivo**, lo que sincronice tu nube es cosa de tu nube.
- No guarda cambios de **ajustes de máquina** ni de preferencias; solo el proyecto.
