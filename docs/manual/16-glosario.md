# Glosario

Términos de láser y de Tlaku, en orden alfabético, explicados en pocas líneas.

## A

- **Acoplar (dock)**: pegar piezas entre sí deslizándolas hasta tocar una forma de referencia. Ver [Copias, arreglo virtual y nesting](03d-copias-y-nesting.md).
- **Aire asistido**: chorro de aire sobre el punto de corte; apaga llamas y limpia humo. En G-code se enciende con `M8` y se apaga con `M9`. Se activa por capa.
- **Ajustes en vivo**: cambiar la velocidad o la potencia (de 10 % a 200 %) mientras el trabajo corre, sin recalcular el G-code. Solo en controladoras de la familia GRBL. Ver [Enviar trabajos con seguridad](09-enviar-con-seguridad.md).
- **Alarma (`ALARM:N`)**: estado en el que la máquina se bloquea por seguridad (límite, reinicio, homing fallido). Se sale con desbloquear o con homing. Ver [Solución de problemas](15-solucion-de-problemas.md).
- **Arreglo virtual**: un objeto que se corta o graba varias veces sin que existan las copias como objetos; el lienzo las muestra como fantasmas. Ver [Copias, arreglo virtual y nesting](03d-copias-y-nesting.md).
- **Asistente booleano**: diálogo que muestra el resultado de cada operación (unión, A menos B, intersección, diferencia simétrica y soldar) antes de aplicarla (`Ctrl/Cmd+B`).
- **Autoguardado**: copia de recuperación que Tlaku guarda cada minuto (si hay cambios) para ofrecértela si la aplicación se cierra de golpe. No reemplaza a guardar. Ver [Preferencias, atajos y autoguardado](02b-preferencias-y-atajos.md).

## B

- **Baudios**: velocidad de la conexión por cable USB serie. 115200 en casi todos los GRBL; 250000 en Marlin.
- **Bidireccional**: relleno que graba tanto de ida como de vuelta. Si las líneas salen desfasadas, hay que corregirlo con la tabla de corrección de barrido del perfil.
- **Bézier**: curva definida por nodos y manijas. La herramienta **Dibujar línea** la crea con clic y arrastre; **Editar nodos** la afina.
- **Banda elástica (contorno de)**: forma cerrada que envuelve la selección como una liga estirada; convexa o cóncava. No es lo mismo que **Encuadrar contorno**, que mueve el cabezal.

## C

- **Cama**: el área de trabajo de la máquina, en mm. Equivale a `$130` y `$131` en GRBL.
- **Capa**: grupo de objetos del mismo color que comparten ajustes de corte (modo, velocidad, potencia, pasadas...). Tlaku ejecuta las capas en el orden de su lista.
- **Consola**: pestaña donde ves lo que se envía a la máquina y lo que contesta, y donde puedes escribir comandos.
- **Contención (agrupar por)**: formar un grupo con cada pieza y los huecos y grabados que quedan dentro de su contorno.
- **Contornos**: forma de rellenar siguiendo la silueta en anillos, en lugar de líneas de barrido.
- **Código QR / Data Matrix / de barras**: código que Tlaku genera como formas vectoriales listas para grabar (**Herramientas > Insertar código…**). Ver [Texto](04-texto.md).
- **Corte pasante**: arranque de cada contorno cerrado con potencia extra durante unos milisegundos, para atravesar material grueso sin dejar un punto quemado. Ver [Capas y ajustes de corte](05-capas-y-ajustes.md).
- **Curvatura**: grados que abarca un texto sobre un arco. Positivo lo arquea hacia arriba, negativo hacia abajo.
- **Cuchilla**: la forma cerrada con que **Cortar formas** parte las demás.

## D

- **Dialecto de G-code**: la forma en que un firmware espera las órdenes (cómo se enciende el láser, el rango de `S`, el aire). Cambia entre GRBL, Marlin y Smoothieware. Ver [Controladoras](13-controladoras.md).
- **Dithering (tramado)**: truco para grabar fotos con un láser que solo enciende o apaga: simula tonos de gris con puntos. Hay varios algoritmos (Umbral, Floyd-Steinberg, Jarvis, Stucki, entre otros).
- **DPI / LPI**: densidad de líneas del grabado. A menor **intervalo** entre líneas, más DPI. Tlaku muestra la equivalencia al editar el intervalo (por ejemplo "0,1 mm = 254 DPI"). LPI son líneas por pulgada.
- **Deformar (warp)**: meter un dibujo en una envolvente de cuatro esquinas y cuatro bordes curvos para arquearlo, abombarlo o darle perspectiva.

## E

- **Encuadrar contorno**: encuadre de "banda elástica": recorre con el láser apagado el contorno convexo del trabajo en vez del rectángulo. Ver [Enviar trabajos con seguridad](09-enviar-con-seguridad.md).
- **Encuadrar**: recorrer con el cabezal el rectángulo del trabajo, con el láser apagado, para comprobar dónde caerá. Es el botón **Encuadrar**.
- **Eje Z**: altura del cabezal o de la cama. Algunas capas pueden moverlo.
- **Entrada y salida (lead-in / lead-out)**: tramo con el que el láser empieza o termina un contorno desde fuera de la pieza, para que la marca del arranque quede en el sobrante y no en el borde.
- **Escala de potencia**: porcentaje de la potencia de la capa que usa un objeto en particular (0 a 100 %). Se pone en **Propiedades del objeto**.
- **Estado (`<Idle|...>`)**: informe que la máquina manda cuando se le pregunta con `?`: reposo, en marcha, pausa, alarma, y su posición.

## F

- **FluidNC**: firmware compatible con GRBL para placas ESP32, que se configura con archivos en lugar de ajustes `$`.
- **Final de carrera**: interruptor que detecta que el cabezal llegó al borde. Hace posible el homing.
- **Fuente de una línea**: fuente de un solo trazo (Hershey o SHX) que dibuja el centro de cada letra en vez de su contorno. Se graba como línea. Ver [Texto](04-texto.md).

## G

- **Guías**: líneas de referencia que se arrastran desde las reglas y atraen a los objetos; no se graban. No son la capa de **Guías** de la paleta.
- **G-code**: el lenguaje de órdenes que entienden las máquinas (`G0` mover rápido, `G1` mover grabando, `M3`/`M4` encender láser). Tlaku lo genera a partir de tu diseño; puedes verlo con **Exportar G-code…**.
- **GRBL**: el firmware más común en láseres de diodo. Tlaku lo habla por USB. Existen variantes: **grblHAL**, **FluidNC** y **GRBL-LPC**.

## H

- **Homing (`$H`)**: la máquina busca sus finales de carrera para saber dónde está. Solo vale en máquinas que los tienen y con `$22=1`. Antes de hacerlo, despeja el área.
- **Hershey**: familia de fuentes de una línea de uso libre, de las que Tlaku incluye 15.
- **HPGL**: lenguaje de plotters y cortadoras de vinilo (`.plt`, `.hpgl`). Tlaku lo importa como trazos, una capa por pluma. Ver [Importar y exportar](03b-importar-y-exportar.md).

## I

- **Incremento de ángulo**: grados que se suman al ángulo de barrido en cada pasada de un relleno o una imagen.
- **Intervalo**: distancia entre dos líneas de un relleno o imagen, en mm. Un intervalo más pequeño da más detalle y más tiempo.
- **Iniciar desde**: de dónde parte el trabajo: **Coordenadas absolutas**, **Posición actual** u **Origen de usuario**.

## J

- **Jog**: mover el cabezal a mano desde la app (botones o `Ctrl/Cmd+Alt+flechas`).

## K

- **Kerf**: ancho del material que el láser consume al cortar. Se compensa para que la pieza final salga a la medida.

## M

- **M3 / M4**: órdenes que encienden el láser. `M3` da **potencia constante**; `M4` da **potencia dinámica**, que baja al frenar en las esquinas y evita quemarlas. M4 necesita GRBL 1.1f o posterior con `$32=1`.
- **Máquina virtual**: emulador dentro de la app para probar sin hardware (puerto `virtual:...`). Puede simular fallas. No graba nada.
- **Máscara (de imagen)**: forma vectorial que recorta una imagen para que solo se grabe lo que queda dentro.
- **Modo láser (`$32=1`)**: ajuste de GRBL que enciende y apaga el rayo según el movimiento. Sin él, `M4` no funciona bien.
- **Modo principiante**: interruptor del editor de capa que oculta los ajustes avanzados sin borrarlos ni desactivarlos.
- **Modo punto**: en lugar de una línea continua, el láser dispara un punto cada cierta distancia.

## N

- **Nesting**: acomodo automático de piezas para desperdiciar lo menos posible de la lámina o de un material irregular. Ver [Copias, arreglo virtual y nesting](03d-copias-y-nesting.md).
- **Notas (del proyecto)**: cuaderno que se guarda con el proyecto y no se envía a la máquina (`Ctrl/Cmd+7`).

## O

- **Offset**: copia de un contorno agrandada o reducida una distancia fija.
- **Operaciones adicionales (sub-capas)**: ajustes extra de una capa que se ejecutan en orden sobre los mismos objetos después de la principal, por ejemplo rellenar y luego cortar el contorno.
- **Optimización (ajustes de)**: opciones que cambian el orden y el recorrido del trabajo: respetar capas, grupos y prioridad, quitar tramos repetidos, esconder el juego mecánico. Ver [Vista previa y orden de corte](07-vista-previa.md).
- **Origen**: punto de partida de las coordenadas. El **origen de la máquina** es la esquina donde está su X0 Y0; el **origen de usuario** es un punto que tú guardas con **Fijar origen aquí**.
- **Overscan**: tramo extra que el cabezal recorre fuera del borde del relleno, con el láser apagado, para que alcance velocidad pareja antes de grabar.

## P

- **Pasada**: cada vez que el láser recorre un mismo trayecto. Varias pasadas cortan más hondo.
- **Pass-through (enviar sin tramar)**: la imagen se envía tal cual, sin tramado ni ajustes de tono; cada punto ya es la potencia.
- **Perforación**: corte intermitente, con tramos cortados y tramos saltados, para líneas de doblez o piezas que deben quedar unidas.
- **Planificador**: la parte del programa que decide en qué orden se ejecuta cada paso. En la vista previa puedes ver qué regla decidió cada uno.
- **Plantilla**: diseño, capas y ajustes guardados como punto de partida para proyectos nuevos (**Archivo > Guardar como plantilla…**).
- **Posición final**: dónde deja el cabezal al terminar un trabajo bien: quedarse, volver al origen o ir a un punto.
- **Posicionar el láser**: modo en el que un clic en la cama del lienzo manda el cabezal a ese punto, con el láser apagado.
- **Potencia mínima / máxima**: en una imagen, la potencia de los puntos más claros y de los más oscuros.
- **Prioridad de corte**: número que se le da a un objeto para cortarlo antes o después que otros de su capa (menor, antes).
- **Puentes**: trozos que se dejan sin cortar para que la pieza no se caiga ni se mueva antes de terminar, repartidos solos por la capa o colocados a mano con la herramienta `U`.
- **Punto de inicio**: el lugar donde empieza el corte de un contorno cerrado; se puede fijar a mano con la herramienta `Q`, junto con el sentido.
- **Preferencias (`.tlakuprefs`)**: tus ajustes y atajos de teclado; se exportan a un archivo `.tlakuprefs` para llevarlos a otro equipo. Ver [Preferencias, atajos y autoguardado](02b-preferencias-y-atajos.md).

## R

- **Recortar (trim)**: borrar el tramo de un trazo que queda entre dos cruces, con la herramienta `X`.
- **Rampa de potencia**: la potencia sube poco a poco al empezar un trazo y baja al terminarlo, en lugar de entrar de golpe.
- **Raster / vector**: raster es un grabado punto a punto (imágenes, rellenos); vector es seguir un trazo (cortes y líneas).
- **Relleno por regiones (flood fill)**: relleno que recorre cada región del dibujo por separado sin cruzar los espacios vacíos.
- **Rotativo**: accesorio que gira una pieza cilíndrica, normalmente en lugar del eje Y. Ver [Rotativo y eje Z](12-rotativo-y-eje-z.md).
- **Ruida**: controladora de muchos láseres de CO2. En Tlaku es experimental y solo por línea de comandos.

## S

- **S máxima (`$30`)**: valor de `S` que equivale al 100 % de potencia. Debe coincidir en tu perfil y en la máquina (1000, 255 o 1 según el firmware).
- **Sobrecorte (overcut)**: tramo que el láser sigue cortando más allá del punto donde se cierra un contorno.
- **Soft limit**: límite virtual que impide moverse fuera de la cama; al excederlo salta `ALARM:2` o `error:10`.
- **SHX**: formato de fuentes de una línea de AutoCAD. Tlaku importa las que ya tengas (**Herramientas > Importar fuente SHX…**).
- **Solo la selección**: opción para que la vista previa, el encuadre, el trabajo y el G-code lleven únicamente lo que está seleccionado.
- **SVG / DXF / PDF**: formatos abiertos de dibujo vectorial. Tlaku los importa y también exporta tu diseño a ellos (más PNG). Ver [Importar y exportar](03b-importar-y-exportar.md).

## T

- **Trayecto**: línea o curva vectorial. Un texto se convierte en trayectos para poder editar sus nodos.
- **Texto soldado**: texto cuyas letras que se tocan o se solapan se funden en un solo contorno (casilla **Soldar letras**).
- **Texto variable**: texto con llaves (`{serie}`, `{csv:Nombre}`, `{fecha}`) que se cambia por valores al enviar el trabajo. Ver [Texto variable](04b-texto-variable.md).

## V

- **Valores por defecto**: ajustes con los que empiezan las capas nuevas; se guardan desde el editor de una capa.
- **Vectorizar (trace)**: convertir una imagen en trazos.
- **Vista previa**: simulación del trabajo con el tiempo estimado, antes de enviarlo.
- **Voltear**: reflejar la selección de izquierda a derecha (`Alt+H`) o de arriba abajo (`Alt+V`). **Espejo sobre una línea** crea una copia reflejada sobre una línea que marcas.

## Símbolos de la consola

- **`$$`**: lista los ajustes de la máquina.
- **`$I`**: versión y opciones del firmware.
- **`$X`**: desbloquea una alarma sin hacer homing.
- **`ok`**: la máquina aceptó la línea. **`error:N`**: la rechazó.

Siguiente: [Índice](00-indice.md)
