# Glosario

Términos de láser y de Tlaku, en orden alfabético, explicados en pocas líneas.

## A

- **Aire asistido**: chorro de aire sobre el punto de corte; apaga llamas y limpia humo. En G-code se enciende con `M8` y se apaga con `M9`. Se activa por capa.
- **Alarma (`ALARM:N`)**: estado en el que la máquina se bloquea por seguridad (límite, reinicio, homing fallido). Se sale con desbloquear o con homing. Ver [Solución de problemas](15-solucion-de-problemas.md).

## B

- **Baudios**: velocidad de la conexión por cable USB serie. 115200 en casi todos los GRBL; 250000 en Marlin.
- **Bidireccional**: relleno que graba tanto de ida como de vuelta. Si las líneas salen desfasadas, hay que corregirlo con la tabla de corrección de barrido del perfil.

## C

- **Cama**: el área de trabajo de la máquina, en mm. Equivale a `$130` y `$131` en GRBL.
- **Capa**: grupo de objetos del mismo color que comparten ajustes de corte (modo, velocidad, potencia, pasadas...). Tlaku ejecuta las capas en el orden de su lista.
- **Consola**: pestaña donde ves lo que se envía a la máquina y lo que contesta, y donde puedes escribir comandos.
- **Contornos**: forma de rellenar siguiendo la silueta en anillos, en lugar de líneas de barrido.

## D

- **Dialecto de G-code**: la forma en que un firmware espera las órdenes (cómo se enciende el láser, el rango de `S`, el aire). Cambia entre GRBL, Marlin y Smoothieware. Ver [Controladoras](13-controladoras.md).
- **Dithering (tramado)**: truco para grabar fotos con un láser que solo enciende o apaga: simula tonos de gris con puntos. Hay varios algoritmos (Umbral, Floyd-Steinberg, Jarvis, Stucki, entre otros).
- **DPI / LPI**: densidad de líneas del grabado. A menor **intervalo** entre líneas, más DPI. Tlaku muestra la equivalencia al editar el intervalo (por ejemplo "0,1 mm = 254 DPI"). LPI son líneas por pulgada.

## E

- **Encuadrar**: recorrer con el cabezal el rectángulo del trabajo, con el láser apagado, para comprobar dónde caerá. Es el botón **Encuadrar**.
- **Eje Z**: altura del cabezal o de la cama. Algunas capas pueden moverlo.
- **Estado (`<Idle|...>`)**: informe que la máquina manda cuando se le pregunta con `?`: reposo, en marcha, pausa, alarma, y su posición.

## F

- **FluidNC**: firmware compatible con GRBL para placas ESP32, que se configura con archivos en lugar de ajustes `$`.
- **Final de carrera**: interruptor que detecta que el cabezal llegó al borde. Hace posible el homing.

## G

- **G-code**: el lenguaje de órdenes que entienden las máquinas (`G0` mover rápido, `G1` mover grabando, `M3`/`M4` encender láser). Tlaku lo genera a partir de tu diseño; puedes verlo con **Exportar G-code…**.
- **GRBL**: el firmware más común en láseres de diodo. Tlaku lo habla por USB. Existen variantes: **grblHAL**, **FluidNC** y **GRBL-LPC**.

## H

- **Homing (`$H`)**: la máquina busca sus finales de carrera para saber dónde está. Solo vale en máquinas que los tienen y con `$22=1`. Antes de hacerlo, despeja el área.

## I

- **Intervalo**: distancia entre dos líneas de un relleno o imagen, en mm. Un intervalo más pequeño da más detalle y más tiempo.
- **Iniciar desde**: de dónde parte el trabajo: **Coordenadas absolutas**, **Posición actual** u **Origen de usuario**.

## J

- **Jog**: mover el cabezal a mano desde la app (botones o `Ctrl/Cmd+Alt+flechas`).

## K

- **Kerf**: ancho del material que el láser consume al cortar. Se compensa para que la pieza final salga a la medida.

## M

- **M3 / M4**: órdenes que encienden el láser. `M3` da **potencia constante**; `M4` da **potencia dinámica**, que baja al frenar en las esquinas y evita quemarlas. M4 necesita GRBL 1.1f o posterior con `$32=1`.
- **Máquina virtual**: emulador dentro de la app para probar sin hardware (puerto `virtual:...`). Puede simular fallas. No graba nada.
- **Modo láser (`$32=1`)**: ajuste de GRBL que enciende y apaga el rayo según el movimiento. Sin él, `M4` no funciona bien.

## O

- **Offset**: copia de un contorno agrandada o reducida una distancia fija.
- **Origen**: punto de partida de las coordenadas. El **origen de la máquina** es la esquina donde está su X0 Y0; el **origen de usuario** es un punto que tú guardas con **Fijar origen aquí**.
- **Overscan**: tramo extra que el cabezal recorre fuera del borde del relleno, con el láser apagado, para que alcance velocidad pareja antes de grabar.

## P

- **Pasada**: cada vez que el láser recorre un mismo trayecto. Varias pasadas cortan más hondo.
- **Planificador**: la parte del programa que decide en qué orden se ejecuta cada paso. En la vista previa puedes ver qué regla decidió cada uno.
- **Potencia mínima / máxima**: en una imagen, la potencia de los puntos más claros y de los más oscuros.
- **Puentes**: trozos que se dejan sin cortar para que la pieza no se caiga ni se mueva antes de terminar.

## R

- **Raster / vector**: raster es un grabado punto a punto (imágenes, rellenos); vector es seguir un trazo (cortes y líneas).
- **Rotativo**: accesorio que gira una pieza cilíndrica, normalmente en lugar del eje Y. Ver [Rotativo y eje Z](12-rotativo-y-eje-z.md).
- **Ruida**: controladora de muchos láseres de CO2. En Tlaku es experimental y solo por línea de comandos.

## S

- **S máxima (`$30`)**: valor de `S` que equivale al 100 % de potencia. Debe coincidir en tu perfil y en la máquina (1000, 255 o 1 según el firmware).
- **Soft limit**: límite virtual que impide moverse fuera de la cama; al excederlo salta `ALARM:2` o `error:10`.

## T

- **Trayecto**: línea o curva vectorial. Un texto se convierte en trayectos para poder editar sus nodos.

## V

- **Vectorizar (trace)**: convertir una imagen en trazos.
- **Vista previa**: simulación del trabajo con el tiempo estimado, antes de enviarlo.

## Símbolos de la consola

- **`$$`**: lista los ajustes de la máquina.
- **`$I`**: versión y opciones del firmware.
- **`$X`**: desbloquea una alarma sin hacer homing.
- **`ok`**: la máquina aceptó la línea. **`error:N`**: la rechazó.

Siguiente: [Índice](00-indice.md)
