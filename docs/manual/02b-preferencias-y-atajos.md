# Preferencias, atajos y autoguardado

Cómo ajustar Tlaku a tu manera: la ventana de **Preferencias**, el editor de **atajos de teclado**, los archivos `.tlakuprefs` para llevar tu configuración a otro equipo y las **copias de recuperación** que protegen tu trabajo si la aplicación se cierra de golpe. Los atajos se escriben con `Ctrl/Cmd`: `Cmd` en macOS, `Ctrl` en Windows y Linux.

## Abrir las preferencias

Elige **Editar > Preferencias…** o pulsa `Ctrl/Cmd+,`. La ventana tiene siete secciones a la izquierda y **los cambios se guardan solos**: no hay botón de aceptar, solo **Cerrar**.

![Ventana de Preferencias con las secciones General, Edición, Apariencia, Archivos, Automatización y Atajos (la captura es anterior a la sección Control remoto)](../../apps/desktop/docs/screenshots/29-preferencias.png)

Abajo hay tres botones: **Importar…** y **Exportar…** (ver más abajo) y **Restablecer**, que devuelve todas las preferencias a su valor de fábrica tras pedir confirmación. Restablecer **no toca tus atajos de teclado**.

## Qué se puede ajustar

| Sección | Ajuste | De fábrica | Rango |
|---|---|---|---|
| General | **Idioma** | El del sistema (español o inglés) | Español o English; cambia al momento, sin perder el documento. Los avisos y errores del motor salen en el idioma elegido |
| General | **Modo de la interfaz** | Preguntar al abrir | Sencillo (para principiantes), Completo o Preguntar al abrir; elegir uno hace lo mismo que el botón de la barra superior. Ver [Modo sencillo](01b-modo-sencillo.md) |
| General | **Reabrir la cámara al iniciar** | Sí | Sí o no; si el fondo de la cámara estaba visible al cerrar, la cámara vuelve a encenderse al abrir |
| General | **Ancho máximo de la imagen** de la cámara | 1280 px | 320 a 1920 px; el ancho al que se reduce el cuadro para pintarlo y detectar marcas y círculos |
| Edición | **Unidades de longitud** | Milímetros (mm) | mm o pulgadas (in) |
| Edición | **Paso normal** de las flechas | 1 mm | 0,001 a 100 mm |
| Edición | **Paso con Mayús** | 10 mm | 0,001 a 500 mm |
| Edición | **Paso con Alt** | 0,1 mm | 0,001 a 10 mm |
| Edición | **Paso de rotación con Mayús** | 15° | 1° a 180° |
| Edición | **Distancia del ajuste magnético** | 8 px | 1 a 40 px |
| Edición | **Desplazamiento al duplicar y pegar** | 5 mm | 0 a 200 mm |
| Edición | **Volver a Seleccionar tras dibujar una forma** | Sí | Sí o no. Con sí, al terminar un rectángulo, una elipse, un polígono, una estrella o una línea la herramienta pasa a **Seleccionar** y deja la forma elegida; sin él, la herramienta sigue activa para dibujar otra |
| Apariencia | **Tema** | Oscuro | Oscuro o Claro |
| Apariencia | **Rejilla del lienzo** | Automática (según el zoom) | Automática o una medida fija de 1 a 100 mm |
| Apariencia | **Contraste de la rejilla** | Normal | Suave, Normal o Alto |
| Apariencia | **Distribución de la ventana** | Panel a la derecha, herramientas a la izquierda | Izquierda o derecha para cada uno, y **Restablecer distribución**. Ver [Guías, puentes, punto de inicio y proyecto](03e-guias-puentes-y-proyecto.md) |
| Archivos | **Guardar copias de recuperación automáticamente** | Sí | Sí o no |
| General | **Reabrir las pestañas al iniciar** | Sí | Sí o no. Con sí, al abrir Tlaku se vuelven a abrir los proyectos (con archivo) que tenían las pestañas al cerrar. Ver [Pestañas de documentos](03f-pestanas-de-documentos.md#recuperacion-y-sesion) |
| General | **Buscar actualizaciones al iniciar** | No | Sí o no. Unos segundos después de abrir, Tlaku consulta si hay una versión nueva y te avisa. Ver [Actualizaciones](#actualizaciones) |
| Archivos | **Intervalo entre copias** | 60 s | 10 a 3600 s |
| Archivos | **Archivos recientes que se recuerdan** | 10 | 0 a 30 |
| Archivos | **Resolución por omisión** (Importar archivos) | 96 ppp | 10 a 2400 ppp |
| Archivos | **Recordar las opciones del diálogo de importar** | Sí | Sí o no |
| Automatización | **Activar la automatización por UDP** | No | Sí o no. Ver [Conexión por red y automatización](08b-red-y-automatizacion.md) |
| Automatización | **Puerto UDP** | 47801 | 1024 a 65535 (aparece al activarla) |
| Automatización | **Permitir iniciar desde automatización** | No | Sí o no. **Encendida, cualquier programa de tu equipo puede arrancar el láser sin que lo confirmes** |
| Control remoto | **Activar el control remoto desde el celular** | No | Sí o no. Muestra un código QR para abrir una página en el celular, por la red local. Ver [Control desde el celular](09e-control-desde-el-celular.md) |
| Control remoto | **Puerto** y **Dirección de red** | 47802, automática | Puerto 1024 a 65535; la dirección se elige solo si la computadora está en varias redes |

![La interfaz en inglés: paneles y menús traducidos al cambiar el idioma en Preferencias; en esta captura, anterior a la traducción de los mensajes del motor, el resumen del orden de capas aparece en español](../../apps/desktop/docs/screenshots/38-english.png)

Detalles que conviene saber:

- **Unidades.** El diseño se guarda siempre en milímetros; la unidad que eliges cambia cómo se muestran y se escriben las longitudes. Se aplica en la barra numérica (X, Y, ancho y alto), la barra de estado, las reglas y la cuadrícula (en pulgadas, de décima en décima), la herramienta **Medir** (ver [Dibujar y editar](03-dibujar-y-editar.md)), los campos de longitud de los diálogos de Organizar (arreglos, offset, alinear y acomodar piezas) y los de esta ventana. Los campos aceptan la unidad al escribir («5 mm», «0,25 in», «1/8"», «1 1/2 in», `50%`). Las velocidades siguen en mm/min, y los campos de las capas (intervalo, kerf, desplazamiento Z...), de las áreas de máquina y de la cámara, en mm.
- **Pasos de las flechas.** Cambian los tres pasos de las flechas del teclado (ver [Dibujar y editar](03-dibujar-y-editar.md)).
- **Paso de rotación.** Es el que usan el giro con `Mayús` al arrastrar la perilla, los comandos **Girar un paso a la izquierda/derecha** y el campo **Paso de giro** de la barra numérica.
- **Rejilla.** **Automática** ajusta el paso al zoom; con una **medida fija** (de 1 a 100 mm), la rejilla menor mide eso y la mayor diez veces más; con el zoom muy alejado se oculta para no saturar la vista. El **Contraste de la rejilla** (**Suave**, **Normal**, **Alto**) decide cuánto se marcan sus líneas sobre la cama: súbelo a **Alto** si no la ves bien en tu pantalla o con el tema claro, y bájalo a **Suave** si te distrae al dibujar.
- **Volver a Seleccionar.** Es lo que hace que, tras dibujar una figura, puedas moverla o escalarla enseguida sin cambiar de herramienta. Apágalo si dibujas muchas figuras seguidas del mismo tipo; `Esc` también vuelve a **Seleccionar** en cualquier momento.
- **Importar archivos.** La **Resolución por omisión** es con la que se importan los SVG sin unidades y las imágenes cuando no eliges otra en **Importar con opciones…** (96 es lo habitual en los programas de dibujo; Illustrator usa 72). Con **Recordar las opciones del diálogo de importar**, ese diálogo arranca con lo que elegiste la vez anterior (resolución, unidad del DXF, agrupar y capa destino); la **escala** y la **página del PDF** siempre empiezan de cero. **Olvidar las opciones recordadas** las borra. Ver [Importar y exportar](03b-importar-y-exportar.md).
- **Tema.** Es el mismo ajuste que el botón de sol o luna de la barra superior.
- **Archivos recientes.** El botón **Vaciar la lista de recientes** borra la lista de **Archivo > Abrir reciente**. Con 0 no se recuerda ninguno.

### Trabajar en pulgadas

Si tus medidas vienen en pulgadas (planos de EE. UU., madera de 3/4"), cambia **Unidades de longitud** a **Pulgadas** y el programa te habla en esa unidad sin que el diseño deje de guardarse en milímetros.

1. **Editar > Preferencias… > Edición > Unidades de longitud**: pulgadas (`in`).
2. Selecciona un objeto: en la barra numérica, **X**, **Y**, **Ancho** y **Alto** se ven en pulgadas, igual que las reglas (con marcas de décima en décima) y la barra de estado.
3. Escribe como te sea cómodo. Con pulgadas activas, un número sin unidad son pulgadas, y puedes poner fracciones y mezclar unidades: `3/8"`, `1 1/2 in`, `2.25`, `5mm` (se convierte), `50%` (la mitad del valor actual) o una cuenta como `(10+2)*4`.

> **Ejemplo:** una pieza de 3 1/2 × 2 pulgadas. Selecciona el rectángulo, escribe `3 1/2` en **Ancho** y `2` en **Alto**. Por dentro quedan 88,9 × 50,8 mm. Para dejar 1/8" de separación al acomodar piezas, escribe `1/8"` en **Separación entre piezas** de **Acomodar piezas**.

Qué **sigue en milímetros** aunque elijas pulgadas: las velocidades (siempre mm/min), los campos de las capas (intervalo, kerf, desplazamiento Z...), las áreas de las máquinas y de la cámara y el grosor del material. Si ves una cifra en milímetros en un campo de esas, no es un error: ese campo aún no cambia de unidad.

Si tienes dos ventanas de Tlaku abiertas, un cambio de preferencias en una **se refleja en la otra** al momento. Esto se comprueba con la prueba de humo de la aplicación real (`npm run smoke:tauri`), que abre una segunda ventana y verifica que lo que una guarda la otra lo lee y que el aviso de cambio del almacenamiento llega de una ventana a la otra.

Las preferencias viven en un archivo `preferences.json` dentro de la carpeta de configuración de la aplicación, no en la ventana web, así que sobreviven a limpiar los datos del navegador integrado.

## Atajos de teclado

Los atajos de fábrica están en [La interfaz](02-interfaz.md) y en el diálogo que se abre con `F1`, `?` o `Ctrl/Cmd+/`. Para cambiarlos abre **Editar > Editar atajos de teclado…** o la sección **Atajos** de las Preferencias.

![Diálogo de atajos de teclado con el buscador](../../apps/desktop/docs/screenshots/09-atajos.png)

La lista agrupa los comandos por categoría y tiene un buscador (**Buscar un comando o un atajo**). Sobre cada comando:

1. Pulsa **Cambiar el atajo** (el lápiz) y, cuando diga **Pulsa la combinación…**, escribe la nueva. `Esc` cancela.
2. **Añadir otro atajo** (el signo más) deja a un comando varias combinaciones.
3. **Quitar el atajo** lo deja **Sin atajo**.
4. **Restaurar el de fábrica** vuelve a la combinación original de ese comando.

Un comando con atajo cambiado se marca como **Personalizado**, y arriba se cuenta cuántos tienes.

Si la combinación ya la usa otro comando, Tlaku no la pisa en silencio: dice «X» ya es el atajo de «Y» y te ofrece **Quitárselo y asignarlo aquí** o **Cancelar**.

Además:

- **Restaurar los de fábrica** (abajo del panel) quita todos tus cambios, tras confirmar.
- **Exportar atajos…** e **Importar atajos…** guardan y cargan solo los atajos, útiles como respaldo.
- Las teclas de herramienta son letras sueltas y no funcionan mientras escribes en un campo, igual que con los atajos de fábrica.
- El diálogo `F1` y las tablas de este manual muestran **siempre los atajos de fábrica**, no los que hayas cambiado.

## Llevar tu configuración a otro equipo

En la ventana de Preferencias, **Exportar…** guarda un archivo `.tlakuprefs` con todas tus preferencias **y** tus atajos. **Importar…** lo carga en otro equipo (o después de reinstalar). El archivo lleva también las preferencias que se cambian fuera de esta ventana: los **valores por defecto de las capas nuevas**, también los de cada color y máquina (ver [Capas y ajustes de corte](05-capas-y-ajustes.md)), y el **modo de la interfaz** (ver [Modo sencillo](01b-modo-sencillo.md)); un paquete de una versión anterior con el Modo principiante encendido pasa al modo sencillo si no traía un modo elegido y la configuración de la lista **Antes de enviar** del diálogo de iniciar (ver [Enviar trabajos con seguridad](09-enviar-con-seguridad.md)), los **materiales favoritos** (ver [Materiales](11-materiales.md#favoritos)) y si la barra rápida está visible. El **Modo producción** se exporta pero **nunca se importa**: un archivo ajeno no puede quitarte la lista de comprobación; se enciende a mano en cada equipo. Los **Ajustes de optimización** y la **Posición final** se recuerdan entre sesiones, pero no viajan en este archivo. Para llevar también tus **máquinas** (con macros y posiciones), tu **biblioteca de materiales**, tus **cámaras**, tu **arte propio**, tus **perfiles de costos**, tus **retazos** y tus **plantillas** usa un paquete `.tlakubundle` (**Archivo > Exportar paquete de configuración…**; ver [Control avanzado de la máquina](09b-control-avanzado.md)), o déjalo todo respaldado solo en una carpeta ([abajo](#respaldo-de-ajustes-en-una-carpeta)).

Al importar, Tlaku dice cuántas preferencias y de cuántos comandos son los atajos que entraron. Si el archivo viene de otra versión:

- Las preferencias que esta versión no entiende se **ignoran** y se avisa cuántas fueron.
- Los atajos de comandos que no existen en esta versión **se conservan**, por si llegan después, y se avisa cuántos son.

Un archivo que no es un paquete de preferencias de Tlaku se rechaza con un mensaje claro. Los materiales tienen su propio archivo (`.tlakumat`, ver [Materiales](11-materiales.md)); los dispositivos, las cámaras y el arte propio viajan en el paquete `.tlakubundle`.

## Respaldo de ajustes en una carpeta

Para no perder tu configuración (o para tenerla igual en dos computadoras) Tlaku puede guardar **copias de tus ajustes en una carpeta que tú elijas**: la de iCloud Drive, Dropbox, OneDrive, una carpeta que sincroniza Syncthing o una memoria USB. Está en **Archivo > Respaldo de ajustes…**.

**Qué se respalda.** Tus **máquinas** (con sus macros y posiciones guardadas), la **biblioteca de materiales**, los **perfiles de costos**, los **retazos**, las **plantillas de proyecto**, la **calibración de las cámaras**, tu **arte propio**, las **preferencias** y los **atajos**. El **historial de trabajos** solo entra si marcas su casilla (puede pesar bastante). **Nunca** se guardan contraseñas, llaves ni el acceso del control remoto, y las URL de cámara van sin usuario ni clave.

**Cómo se configura.**

1. Pulsa **Elegir carpeta…** y escoge dónde guardar. Los archivos se llaman `tlaku-respaldo-AAAA-MM-DD_HH-MM-SS.tlakubundle` (hora UTC, así ordenan igual en cualquier computadora).
2. Marca cuándo respaldar: **al cerrar Tlaku** y/o **al abrir si pasaron N días** desde el último. También puedes pulsar **Respaldar ahora** (o **Archivo > Respaldar ajustes ahora**) cuando quieras.
3. Elige **cuántos conservar**: Tlaku borra los más viejos al pasarse, y solo toca archivos con su propio nombre; el resto de la carpeta queda como está. Cada archivo se escribe completo antes de aparecer (no deja archivos a medias si algo se interrumpe).

Si la carpeta **no está disponible** (la unidad USB está desconectada o la nube no ha terminado de sincronizar), Tlaku **avisa una vez** y sigue trabajando; los respaldos se reanudan solos cuando vuelve.

**Restaurar.** En el mismo diálogo, la lista **Respaldos en la carpeta** tiene un botón **Restaurar…** por cada uno. Se abre la **vista previa** de siempre (la misma de **Importar paquete de configuración…**): por cada categoría (máquinas, materiales, costos, retazos, plantillas, cámaras, arte, preferencias e historial) ves cuántos elementos son **nuevos**, cuántos **distintos** de los que ya tienes y cuántos **iguales**, y marcas qué categorías importar. Donde hay elementos que chocan por id o nombre eliges qué hacer: **Reemplazar el local**, **Conservar ambos** (el que entra se renombra, por ejemplo «Mesa (importado)») u **Omitir el que entra**. Nada se escribe hasta que pulsas **Aplicar**, y **antes de aplicar Tlaku guarda un respaldo del estado actual** (en su carpeta de configuración, los últimos 5) para que puedas volver atrás; si ese respaldo previo falla, no se aplica nada.

**Llevar tus ajustes a otra computadora.** Elige la **misma carpeta** de respaldo allá. Al abrir Tlaku, si en la carpeta hay un respaldo **más nuevo que lo de esa computadora y hecho en otra**, aparece una pregunta: **Revisar…** abre la vista previa, **Ahora no** lo deja para después y **Omitir este respaldo** lo da por visto. **Nunca se aplica solo.** Mientras haya un respaldo ajeno sin revisar, los respaldos automáticos se pausan para no taparlo.

**Lo que esto no es.** No es sincronización en vivo: no fusiona cambios de dos computadoras a la vez ni resuelve relojes desalineados; tú decides qué importar. Si trabajas en ambas, restaura en la que vas a usar antes de empezar. En Flatpak o con macOS en sandbox, Tlaku solo ve la carpeta que eliges con el diálogo.

## Actualizaciones

Tlaku puede avisarte cuando hay una versión nueva y, si quieres, instalarla desde la misma aplicación.

**Buscar a mano.** Elige **Ayuda > Buscar actualizaciones…**. Se abre la ventana **Actualizaciones** y dice una de tres cosas: que **Tlaku está al día**, que **hay una versión nueva** (con las novedades de esa versión) o por qué no pudo comprobarlo.

**Instalar.** Si hay una versión nueva, pulsa **Descargar e instalar**. La descarga se verifica con la firma del proyecto antes de instalarse. Al terminar, la ventana dice que la actualización quedó instalada y te pide **reiniciar Tlaku** para usarla (**Reiniciar ahora** o **Más tarde**). Guarda tu trabajo antes de reiniciar; en Windows el instalador puede cerrar Tlaku por su cuenta. Si algo falla, el botón **Reintentar** vuelve a intentarlo.

**Con un trabajo en curso no se instala nada.** Mientras la máquina ejecuta un trabajo, la ventana avisa que lo termines antes de instalar la actualización.

**Buscar al iniciar.** En **Preferencias > General** está la casilla **Buscar actualizaciones al iniciar**, **apagada de fábrica**. Si la enciendes, unos segundos después de abrir Tlaku consulta si hay una versión nueva y te avisa. Solo se conecta a la dirección de actualizaciones del proyecto y no envía tus diseños. Esta casilla no viaja en los archivos `.tlakuprefs`: en un equipo nuevo se enciende a mano.

**Flatpak.** Si instalaste Tlaku con Flatpak, las actualizaciones las da Flathub: usa la tienda de software de tu sistema o ejecuta `flatpak update io.github.jahziel1.tlaku`.

**Compilaciones sin actualizaciones.** Si tu copia no trae las actualizaciones automáticas configuradas, la ventana lo dice y la casilla no hace nada: descarga la versión nueva de la página del proyecto e instálala encima de la anterior.

## Autoguardado y recuperación

Tlaku guarda, mientras trabajas, una **copia de recuperación** de tu documento. No sustituye a tu archivo `.tlaku`: **Guardar** sigue siendo lo que conserva tu trabajo, y la copia nunca sobrescribe tu proyecto.

- Cada 60 segundos por omisión (ajustable en **Preferencias > Archivos**, de 10 a 3600 s), y **solo si hay cambios sin guardar** desde la última copia.
- La copia está en la carpeta de datos de la aplicación, en una subcarpeta `recuperacion`. Hay **una copia por pestaña** con cambios (ver [Pestañas de documentos](03f-pestanas-de-documentos.md#recuperacion-y-sesion)); se conservan como máximo 40 copias entre todas las ventanas.
- Se borra sola cuando guardas el proyecto, abres otro o cierras la aplicación con normalidad.
- Se puede apagar con **Guardar copias de recuperación automáticamente**. Si no se puede escribir la copia, Tlaku lo avisa una vez.

Si Tlaku se cierra de golpe (un corte de luz, una falla, un cierre forzado), la próxima vez que lo abras aparece **Se encontró trabajo sin guardar**, con una fila por documento: su nombre, **Copia del** día y hora, y el proyecto de origen (o **Documento que nunca se guardó**). Para cada una:

- **Recuperar** abre la copia en una **pestaña nueva**, marcada como **Sin guardar** (tu documento actual no se toca). **Recuperar todas** abre todas de una vez, cada una en su pestaña. Después de recuperar, **guárdalo** para conservarlo.
- **Descartar** la borra.
- **Descartar todo** borra todas (pide confirmación; no se puede deshacer).
- **Decidir después** cierra la ventana sin tocar nada.

> **Consejo:** el autoguardado te salva de un cierre inesperado, no de un error tuyo. Para volver atrás un cambio, usa **Deshacer** y el panel **Historial** (ver [La interfaz](02-interfaz.md)).

Siguiente: [Dibujar y editar](03-dibujar-y-editar.md)
