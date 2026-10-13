# Enviar trabajos con seguridad

El orden recomendado para mandar un diseño a la máquina: revisar la vista previa, colocar el trabajo, encuadrar, confirmar y vigilar. Tlaku frena todo lo que puede mover el láser sin que tú lo pidas.

> **Ojo:** nada de esto se ha probado con una máquina real. Todo el flujo (encuadrar, iniciar, pausar, detener, alarmas) se validó contra el emulador y las máquinas virtuales de la app. Practica primero con una **máquina virtual** ([Conectar tu máquina](08-conectar-maquina.md)) y haz tu primer trabajo real con material de desecho. Mantén siempre a mano el paro de emergencia físico de tu máquina.

## Qué comprueba Tlaku antes de transmitir

Al iniciar o encuadrar, la app consulta el estado de la máquina (`?`) y **no transmite nada** si no está en reposo (**Lista**). En alarma, pausa, puerta abierta, en marcha o en cualquier otro estado, te explica por qué y qué hacer (aparece **La máquina no aceptó el trabajo** en la sección **Trabajo**, y el mensaje queda también en la consola). También verifica el G-code contra el búfer de la placa y te avisa si:

- el modo láser está apagado (`$32=0`): bloquea el envío de un trabajo, porque el láser quedaría encendido en los desplazamientos;
- la S máxima del perfil no coincide con `$30` de la máquina: aviso, la potencia saldría escalada;
- el firmware reporta valores `nan` en los desplazamientos de trabajo: aviso (Tlaku no usa G54 a G59).

La app **nunca** envía `$X`, `$H` ni `~` por su cuenta: cada uno se pide con un botón que explica el riesgo y pide confirmación.

## 1. Colocar el trabajo: Iniciar desde

En la pestaña **Máquina**, sección **Trabajo**, **Iniciar desde** decide dónde cae el diseño sobre la cama.

| Modo | Qué hace |
|---|---|
| **Coordenadas absolutas** | El trabajo sale en el lugar de la cama donde lo ves en el lienzo |
| **Posición actual** | Se ancla donde esté el cabezal al iniciar (necesita la máquina conectada) |
| **Origen de usuario** | Se ancla en el punto que guardaste con **Fijar origen aquí** |

Con los dos últimos eliges además el **punto del trabajo que se ancla**, entre nueve (cuadrícula de 3 por 3): las cuatro esquinas, el centro de cada lado y el centro. Por ejemplo, con "centro" el centro del diseño queda sobre el cabezal.

Debajo ves una ilustración de la cama con el trabajo, el cabezal y los orígenes, y una frase con coordenadas: "El trabajo ocupará X de 60,0 a 280,0 mm e Y de 70,0 a 300,0 mm de la cama". Si algo se sale, dice por qué lado y cuántos milímetros ("Se sale de la cama: por la izquierda ...") y el diálogo de inicio no te dejará confirmar.

### Mover el cabezal y fijar el origen

1. En **Mover cabezal** usa el pad de flechas. Un clic mueve un paso (`0,1`, `1`, `10` o `50 mm`); mantener pulsado mueve sin parar y se detiene al soltar. Con el foco en el pad, las flechas dan un paso, `Mayús` lo multiplica por 10 y `Alt` lo divide entre 10. Desde cualquier panel: `Ctrl/Cmd+Alt+flechas`.
2. Ajusta la **Velocidad** (en mm/min; no puede pasar de la velocidad máxima del perfil).
3. Cuando el cabezal esté donde quieres que empiece el trabajo, pulsa **Fijar origen aquí**. Solo lo guarda la app; no se envía nada a la máquina. **Borrar el origen de usuario** lo quita.
4. El botón central del pad lleva el cabezal al cero de trabajo (X0 Y0) o, si hay origen de usuario, a ese origen.

**Hacer home ($H)…** es para máquinas con finales de carrera; antes de moverse te recuerda despejar el área.

## 2. Encuadrar

**Encuadrar** (`F`, o el botón de la barra superior) recorre con el láser **apagado** el rectángulo que ocupa todo el trabajo, overscan incluido. Sirve para ver si el diseño cae donde quieres y dentro del material. Va a la velocidad que tengas en **Mover cabezal**, con `M5` y `S0`.

> **Consejo:** el rectángulo es el del conjunto, no el de cada pieza. Y aunque el láser vaya apagado, mantén las gafas puestas y las manos fuera.

## 3. Iniciar

1. Pulsa **Iniciar** (`Ctrl/Cmd+Intro`). Nunca sale directo: se abre el diálogo **¿Láser listo y área despejada?**
2. Léelo con calma. La lista recuerda: el material está colocado, plano y enfocado; nada estorba el recorrido; la extracción y ventilación están encendidas y llevas gafas; te quedarás junto a la máquina con un extintor cerca. Es una lista para leer, no para marcar.
3. El diálogo resume el **Tiempo estimado**, las **Capas** (potencia, velocidad y pasadas de cada una), la **Potencia máxima** y dónde quedará el trabajo, además de las advertencias del cálculo.
4. **Cancelar** es el botón con el foco. **Iniciar trabajo** se activa solo cuando el resumen ya está calculado y el trabajo cabe en la cama; si no, dice "Espera al resumen del trabajo y corrige lo marcado en rojo".

![Diálogo de confirmación con el resumen del trabajo](../../apps/desktop/docs/screenshots/05-maquina-iniciar.png)

Durante el trabajo ves el progreso (porcentaje, **Líneas**, **Transcurrido** y **Restante**) en la pestaña **Máquina**.

## Pausar y detener

| Acción | Qué hace |
|---|---|
| **Pausar** (`Ctrl/Cmd+Shift+P`) | Feed hold: detiene el movimiento. El mismo botón pasa a **Reanudar** |
| **Detener** (`Ctrl/Cmd+.`) | Feed hold, espera a que frene y reinicio suave de la controladora; el trabajo termina y el láser queda apagado |

La app indica que en pausa el láser está apagado, pero eso aún no está comprobado con una máquina real: en una pausa larga no dejes un diodo enfocado sobre el material.

Si **Detener** ocurre con la máquina en movimiento, la posición puede haberse perdido. Aparece **La posición puede haberse perdido**: con finales de carrera haz **Hacer home ($H)…**; sin ellos, vuelve a fijar el origen a mano. Con Marlin y Smoothieware, cancelar siempre marca la posición como posiblemente perdida.

Si cierras la app con un trabajo en curso, se cancela de la misma forma.

## Alarmas

Si la máquina entra en alarma, aparece un banner rojo en la pestaña **Máquina**: **La máquina está en alarma**, con el código (`ALARM:N`), su explicación en español y **Qué hacer**, según si tu máquina tiene homing activado (`$22`). Los botones son **Desbloquear ($X)…** y **Hacer home ($H)…**: cada uno abre un diálogo con su riesgo y una lista de comprobación, y solo envía al confirmar. `$X` desbloquea sin comprobar la posición; `$H` mueve el cabezal hasta los finales de carrera, así que úsalo solo si tu máquina los tiene. Las causas de cada código están en [Solución de problemas](15-solucion-de-problemas.md).

## Consola segura

La pestaña **Consola** (`Ctrl/Cmd+3`) es el registro de lo enviado y lo recibido, con una línea para escribir comandos.

- Los comandos de solo lectura (`$I`, `$$`, `$#`, `$G`, `$N`, `M5` y `status`) salen sin preguntar. Hay atajos a un clic: `$$`, `$I`, `$G`, `$#` y `status`.
- Todo lo demás (mover, encender el láser, cambiar un ajuste, `$X`, `$H`) abre **Confirmar comando**, que muestra "Se enviará exactamente:" la línea ya normalizada y el riesgo. Solo **Enviar a la máquina** la manda. `$RST` y `$SLP` llevan un aviso extra porque no se pueden deshacer.
- `?`, `!` y `~` se rechazan escritos a mano: usa los nombres `status`, `hold`, `resume` y `reset`.
- Durante un trabajo solo se aceptan `status`, `hold`, `resume` y `reset`.
- Con las flechas arriba y abajo recorres el historial; `Tab` completa el comando.
- Las respuestas `error:N` y `ALARM:N` llegan con su explicación. **Ocultar reportes de estado** limpia el ruido de los `?` periódicos, y puedes copiar o borrar el registro.

## Exportar G-code

Si prefieres enviar con otro programa, **Archivo > Exportar G-code…** (`Ctrl/Cmd+Shift+E`) guarda un archivo `.gcode` con el mismo cálculo y las mismas opciones del trabajo. Al terminar muestra las líneas y el tiempo estimado. Está desactivado si ninguna capa con objetos tiene la salida activada. Si usas **Posición actual**, necesitas la máquina conectada para exportar; sin ella usa **Coordenadas absolutas**.

Por línea de comandos, el programa `tlaku` hace lo mismo y más: `tlaku send` (con las mismas comprobaciones), `tlaku frame`, `tlaku cmd` y `tlaku plan`. Ver el README del proyecto.

Siguiente: [Cámara](10-camara.md)
