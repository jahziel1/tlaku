# La interfaz

Un recorrido por la ventana de Tlaku: qué hay en cada zona y qué atajos de teclado te ahorran tiempo. Los atajos se escriben con `Ctrl/Cmd`: `Cmd` en macOS, `Ctrl` en Windows y Linux.

![Ventana de Tlaku con el panel Capas acoplado a la derecha](../../apps/desktop/docs/screenshots/18-pestanas-ancho.png)

## Barra superior

De izquierda a derecha:

- Los menús **Archivo**, **Editar**, **Herramientas**, **Ver**, **Máquina**, **Organizar** y **Ayuda**. Cada comando muestra su atajo; si no está disponible, explica por qué.
- Botones de archivo: nuevo, abrir, guardar, importar y exportar G-code. A su lado, **Deshacer** y **Rehacer**.
- El nombre del proyecto (**Sin título** hasta que lo guardas) y el aviso **Sin guardar** cuando hay cambios.
- El botón de tema (sol o luna) y, a la derecha, los tres botones del trabajo: **Vista previa**, **Encuadrar** e **Iniciar**. Encuadrar e Iniciar se activan solo con una máquina conectada y algo que enviar.

El menú **Ayuda** abre este **Manual de usuario** (con buscador), la lista de **Atajos de teclado…**, las **Novedades**, **Reportar un problema…**, **Donar al proyecto…** y **Acerca de Tlaku**. Las preferencias están en **Editar > Preferencias…** (`Ctrl/Cmd+,`).

![El manual de usuario abierto dentro de la aplicación, con el índice a la izquierda y el buscador](../../apps/desktop/docs/screenshots/32-manual.png)

## Paleta de herramientas

La columna de la izquierda, de arriba abajo:

| Herramienta | Tecla |
|---|---|
| Seleccionar | `V` (también `Esc`) |
| Editar nodos | `N` |
| Dibujar línea | `L` |
| Rectángulo | `R` |
| Elipse | `E` |
| Polígono | `G` |
| Estrella | `Mayús+G` |
| Texto | `T` |
| Mano (mover la vista) | `H` |
| Medir | `M` |

Las teclas sueltas funcionan en toda la ventana, salvo mientras escribes en un campo. **Medir** muestra distancias, ángulos y datos de una forma. Cómo se usa cada herramienta está en [Dibujar y editar](03-dibujar-y-editar.md).

## Lienzo y reglas

El lienzo muestra la cama de la máquina activa (arriba a la izquierda, su nombre y su tamaño en mm) con reglas en milímetros. El origen está abajo a la izquierda y Y crece hacia arriba.

- **Zoom:** rueda del ratón (centrado en el cursor), pellizco en el trackpad, o los botones de abajo a la derecha (alejar, acercar, ajustar a la cama). `Ctrl/Cmd+=` y `Ctrl/Cmd+-` acercan y alejan; `Ctrl/Cmd+0` ajusta a la cama y `Ctrl/Cmd+Shift+0` al diseño.
- **Mover la vista:** botón central, `Espacio` + arrastrar, dos dedos en el trackpad o la herramienta Mano.
- **Ajuste magnético:** tres interruptores abajo a la izquierda: **Ajuste magnético** (`S`), **Ajustar a la cuadrícula** y **Ajustar a objetos y cama** (nodos, puntos medios, centros e intersecciones). Mantén `Alt` mientras arrastras para suspenderlo solo en ese gesto.
- **Menú contextual:** clic derecho sobre el lienzo abre las acciones más usadas (ver [Dibujar y editar](03-dibujar-y-editar.md)).
- **Solo contornos** (`W`) dibuja los objetos sin relleno.

## Barra numérica

Sobre el lienzo, con objetos seleccionados, escribe valores exactos: **X**, **Y**, **Ancho**, **Alto**, **Rotación** (en grados, antihorario) y el **Paso de giro** (el de `Mayús` al girar). La cuadrícula de nueve puntos de la izquierda elige el **punto de referencia**: el origen de X e Y, el ancla de la escala y el centro del giro. El candado mantiene la proporción. Los campos aceptan expresiones: `100/3`, `+5` (suma), `*2` (multiplica), `50 %`, `5in`. Con **Editar nodos** esta barra pasa a ser la barra de nodos (ver [Dibujar y editar](03-dibujar-y-editar.md)). Las imágenes no se pueden rotar.

## Panel acoplado

A la derecha hay un panel con pestañas. Arrastra su borde izquierdo para cambiar el ancho; si queda estrecho, las pestañas pasan a solo ícono.

| Pestaña | Atajo | Para qué |
|---|---|---|
| Capas | `Ctrl/Cmd+1` | ajustes de corte y orden de ejecución |
| Máquina | `Ctrl/Cmd+2` | conexión, estado, mover el cabezal, trabajo |
| Consola | `Ctrl/Cmd+3` | comandos y respuestas de la máquina |
| Historial | `Ctrl/Cmd+4` | pasos para deshacer y rehacer |
| Cámara | `Ctrl/Cmd+5` | ver [Cámara](10-camara.md) |
| Materiales | `Ctrl/Cmd+6` | ver [Materiales](11-materiales.md) |

El panel **Historial** lista cada acción (hasta 200 pasos). Haz clic en un paso para volver a ese punto; lo deshecho se puede recuperar mientras no hagas otra acción. El botón de la papelera vacía el historial sin cambiar el diseño.

## Paleta de colores y barra de estado

Abajo, la franja de **colores de capa**: 16 colores y las **Guías**. Con objetos seleccionados, elegir un color los mueve a esa capa; sin selección, vuelve activa esa capa para lo que dibujes. `Alt+1` a `Alt+0` eligen los diez primeros y `Alt+G` las guías. Las guías nunca se envían a la máquina.

La barra de estado muestra la posición del cursor en mm, el zoom, la cantidad de objetos seleccionados y su tamaño, el total de objetos, si el proyecto está guardado, la máquina conectada (con la etiqueta **Virtual** si es un emulador) y el tiempo estimado del trabajo.

## Atajos de teclado

Pulsa `?`, `F1` o `Ctrl/Cmd+/` para abrir la lista completa, con buscador (muestra los atajos de fábrica). Todos se pueden cambiar: ver [Preferencias, atajos y autoguardado](02b-preferencias-y-atajos.md). Los más usados:

| Acción | Atajo |
|---|---|
| Nuevo, abrir, guardar | `Ctrl/Cmd+N`, `Ctrl/Cmd+O`, `Ctrl/Cmd+S` |
| Importar diseño | `Ctrl/Cmd+I` |
| Deshacer, rehacer | `Ctrl/Cmd+Z`, `Ctrl/Cmd+Shift+Z` (o `Ctrl/Cmd+Y`) |
| Copiar, cortar, pegar | `Ctrl/Cmd+C`, `Ctrl/Cmd+X`, `Ctrl/Cmd+V` |
| Seleccionar todo, invertir | `Ctrl/Cmd+A`, `Ctrl/Cmd+Shift+I` |
| Duplicar, eliminar | `Ctrl/Cmd+D`, `Supr` o `Retroceso` |
| Agrupar, desagrupar | `Ctrl/Cmd+G`, `Ctrl/Cmd+Shift+G` |
| Bloquear, desbloquear todo | `Ctrl/Cmd+L`, `Ctrl/Cmd+Shift+L` |
| Voltear horizontal, vertical | `Alt+H`, `Alt+V` |
| Girar 90° a la izquierda, a la derecha | `[`, `]` |
| Traer al frente, enviar al fondo | `Ctrl/Cmd+RePág`, `Ctrl/Cmd+AvPág` |
| Preferencias | `Ctrl/Cmd+,` |
| Imprimir (PDF a escala 1:1) | `Ctrl/Cmd+P` |
| Mover la selección | flechas: 1 mm; con `Shift` 10 mm; con `Alt` 0,1 mm |
| Vista previa | `P` |
| Encuadrar, iniciar | `F`, `Ctrl/Cmd+Intro` |

![Diálogo de atajos de teclado con el buscador](../../apps/desktop/docs/screenshots/09-atajos.png)

## Tema claro u oscuro

El botón de sol o luna de la barra superior (o **Ver > Cambiar tema claro u oscuro**) alterna entre los dos temas. Tlaku recuerda tu elección; por defecto abre en oscuro. El mismo ajuste está en **Preferencias > Apariencia**, junto con la rejilla del lienzo.

Siguiente: [Preferencias, atajos y autoguardado](02b-preferencias-y-atajos.md)
