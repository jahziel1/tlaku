# Lista de validación con hardware real

Ola 6D, 2026-10-14. Para cuando haya máquinas. **Ninguna función de máquina de Tlaku se ha probado con hardware real**: todo se validó con emuladores (`tlaku-emu`, el de Ruida, el de la placa K40 y la tarjeta galvo simulada), máquinas virtuales (`virtual:`, `virtualnet:`), imágenes sintéticas y un servidor de demostración. Esta lista dice qué probar, en qué orden y qué datos mandar si algo falla. La primera máquina es la **Creality Falcon2** del dueño del proyecto; el resto queda listo para quien tenga otra.

Documentos con los que esta lista no debe chocar: [hardware/falcon2.md](hardware/falcon2.md), [hardware/grbl.md](hardware/grbl.md), [hardware/ruida.md](hardware/ruida.md) (10.6 y 14), [hardware/dsp-co2.md](hardware/dsp-co2.md), [hardware/galvo.md](hardware/galvo.md) (sección 9) y [hardware/taller.md](hardware/taller.md). Si un resultado real contradice a uno de ellos, el resultado real gana y hay que corregir el documento.

## 0. Cómo usar esta lista

### Reglas que valen para todo

- **El orden es de lo más seguro a lo más arriesgado.** No pases al nivel siguiente si el anterior falló o te dejó dudas. Cada nivel dice qué resultado esperar; si no coincide, para y manda los datos de la sección 1.
- **Niveles:** N1 conectar y leer (no mueve nada) → N2 mover sin láser → N3 encuadre (con el láser apagado, y después, solo si el equipo lo permite, a baja potencia) → N4 disparo de prueba a baja potencia → N5 trabajo chico sobre material de desecho → N6 trabajo real.
- **Antes de cada sesión:** gafas con la densidad óptica correcta para la longitud de onda de tu láser, recinto o tapa cerrada si existe, paro de emergencia al alcance, extractor o aire encendido, material de desecho que no importe y un extintor a la mano. Nunca dejes la máquina sola.
- **Cada prueba con láser se repite primero en la máquina virtual** (Máquina, Puerto: Máquina virtual) para ver el mismo flujo sin riesgo.
- **Lo que reporta el usuario real vale más que lo del emulador.** Un resultado bueno también se reporta: lo que más ayuda al proyecto es saber qué funcionó.

### Leyenda de la columna «Emulador»

| Marca | Significa |
|---|---|
| **E** | El emulador o las pruebas automáticas ya cubren la lógica; la máquina real solo confirma que el emulador se parece a ella. |
| **P** | El emulador cubre una parte (el flujo, el formato); lo físico (tiempos, unidades, bits, mecánica) solo se ve en la máquina. |
| **R** | Solo la máquina real puede validarlo (no hay forma de simularlo con fidelidad). |

### Cómo anotar cada prueba

Copia la tabla de la sección que vayas a hacer y añade una columna «Resultado» con «OK», «Falló» o «Distinto». Si es «Falló» o «Distinto», adjunta lo de la sección 1. Para los resultados buenos basta con el modelo de máquina, la versión de Tlaku y la fecha.

## 1. Qué datos mandar siempre que algo falle

El paquete mínimo, en este orden de utilidad:

1. **Versión y sistema.** Ayuda > Reportar un problema… > «Copiar la información» (versión de Tlaku, sistema operativo). Si es una compilación de desarrollo, el hash del commit.
2. **La máquina.** Marca, modelo exacto (por ejemplo «Falcon2 22 W» o «Falcon2 Pro 40 W»), controladora y versión del firmware.
3. **`$I` y `$$` (GRBL).** Con la máquina conectada y en reposo: panel Máquina > Consola > «Consultas rápidas» ($I, $$, $#, $G; son de solo lectura), o desde la terminal sin abrir la app: `tlaku info --port <puerto>` (lee `$I` y `$$` sin mover nada; `tlaku ports` lista los puertos). Pega el texto completo, sin recortarlo.
4. **El registro de la consola.** Panel Máquina > Consola > «Copiar el registro» (todo lo enviado y las respuestas). Cópialo **antes** de «Borrar el registro» y antes de cerrar la app.
5. **El perfil de la máquina.** «Gestionar dispositivos» (o el comando «Exportar paquete de configuración…» con solo la sección de máquinas): el `.tlakubundle` o una captura de los grupos del perfil. Quita contraseñas de las URL de las cámaras (el paquete ya las quita).
6. **El diseño y el G-code.** El `.tlaku` que lo provoca y el G-code (Archivo > Exportar G-code…, o `tlaku gcode`). Con Ruida, el `.rd` (`tlaku ruida export-rd`); con K40, el `.egv` (`tlaku k40 export-egv`).
7. **Capturas de pantalla** de la app en el momento del fallo (avisos, diálogos, la vista previa) y **fotos de la máquina y del resultado** con una regla o escuadra a la vista para poder medir.
8. **Registro de trabajos.** Ayuda > Registro de trabajos… (fecha, máquina, tiempo estimado y real, resultado). Sirve sobre todo para la sección 12.
9. **Para Ruida, K40 y galvo:** la salida de la terminal del comando que usaste (`tlaku ruida status <ip>`, `tlaku k40 status`, `tlaku galvo status`), completa.

Si el problema es una alarma o la máquina se detuvo: anota qué decía la pantalla de la máquina, si hubo que reiniciarla o apretar el paro, y si perdió la posición.

---

## 2. Creality Falcon2 (primero)

Máquina de diodo azul, GRBL 1.1f modificado por Creality sobre ESP32-S3, 115 200 baudios, cama 400 × 415 mm, origen abajo a la izquierda, `$30=1000`, modo láser, M4 dinámico, homing con finales de carrera (`$22=1`). Detalle y fuentes en [hardware/falcon2.md](hardware/falcon2.md). Ojo: casi toda la evidencia pública es de la **Falcon2 Pro**; la clásica del dueño **no tiene una salida de consola publicada**, así que la primera sesión también sirve para completar `falcon2.md`.

### 2.1 Antes de empezar

- Cable USB **A a C** (los reportes dicen que C a C no funciona) y el driver CH340/CH341 instalado si tu sistema lo pide. Cierra cualquier otro programa que use el puerto (LightBurn, LaserGRBL).
- Perfil: en el asistente «Agregar máquina» elige el modelo Creality Falcon2 22 W (o 40 W) del catálogo; revisa que el perfil coincida con `falcon2.md` sección 7 (cama 400 × 415, `s_max` 1000, M4, sin Z).
- Elige el puerto a mano si la autodetección falla (el hilo de conexión del foro lo recomienda).
- Anota el **color y la altura** del material de desecho y mide la distancia de enfoque con la regla de la máquina.

### 2.2 Pasos, de lo más seguro a lo más arriesgado

| # | Nivel | Qué hacer | Resultado esperado | Emulador |
|---|---|---|---|---|
| 1 | N1 | Conectar desde Tlaku al puerto de la Falcon2. No mandes nada más. | La consola muestra el saludo de GRBL (`Grbl 1.1f ['$' for help]`, quizá con líneas `[OK] ...` propias de Creality antes y después). El panel pasa a «Reposo/Idle» y no se queda esperando. | P (el banner de la Pro está en las pruebas) |
| 2 | N1 | «Consultas rápidas» > `$I`. | `[VER:1.1f.20220810:]`, `[OPT:VHL,127,65536]` y `[software versions: ...]` con la versión de la placa maestra y del módulo láser; **o solo `ok`** (hay un reporte así). Tlaku no debe fallar con ninguna de las dos. Anota el texto exacto de tu modelo. | P |
| 3 | N1 | Consulta `$$` y compara con `falcon2.md` sección 2/7. | `$30=1000`, `$32=1`, `$22=1`, `$21=1`, `$23=3`, `$100=$101=80`, `$130=400`, `$131=415` (y los `$150` a `$161` propios de Creality). Cualquier diferencia se reporta. Si `$30` no es 1000 o `$32` no es 1, **no sigas**: la potencia saldría mal escalada. | P |
| 4 | N1 | Consulta `?` varias veces y mira el panel. | Informe `<Idle\|MPos:...\|Bf:127,65535\|FS:0,0\|...>` sin `Ln:` ni `Ov:`. El panel muestra la posición y el búfer sin `NaN`. | E |
| 5 | N1 | Abre «Ajustes de la máquina…» (panel Máquina). **Solo mira; no escribas nada.** | La tabla muestra cada `$n` con nombre y unidad; marca como críticos los que lo son. | E |
| 6 | N2 | Con el área despejada, «Homing» (`$H`) desde la confirmación de Tlaku. | El cabezal va a la esquina de los interruptores y el estado pasa por `Home` a `Idle`. Tras el homing `MPos` es ~0 y **crece hacia dentro de la cama**. Anota en qué esquina física termina (se espera abajo a la izquierda). | P |
| 7 | N2 | En «Mover», pasos de 1, 10 y 50 mm en las cuatro direcciones. | Cada flecha mueve en el sentido de la flecha (X a la derecha, Y hacia atrás/arriba) la distancia pedida (mídela con la regla en 50 mm: ±0,5 mm). Si algún eje va al revés, anótalo: no cambies el perfil todavía. **Un jog fuera de la cama debe rechazarse:** tras el homing (paso 6), un paso de 1 mm hacia la izquierda desde el origen (o `jog` con dx = −5) no sale a la placa y Tlaku dice «el movimiento saldría de la cama: X llegaría a −4.97 mm (límite 0 a 400)». Comprueba que el cabezal no se mueve. Sin homing en la sesión, o con la posición sin confirmar, no hay revisión y el registro lo avisa una vez («sin límites conocidos»). | P |
| 8 | N2 | «Ir al origen» y «Posiciones guardadas». | El cabezal vuelve a donde dice el panel. | E |
| 9 | N3 | Un cuadrado de 50 mm. Encuadre con el **láser apagado** (borde del trabajo, también «contorno»). | El cabezal recorre el rectángulo exacto, sin tocar los límites. Mide con la regla: ±1 mm. El overscan queda dentro de la cama (si no, Tlaku avisa antes). | P |
| 10 | N3 | Encuadre en bucle (activar «Repetir el encuadre hasta detener» y detener con el botón). | Repite sin deriva visible y se detiene al pulsar «No repetir más»/Detener. | P |
| 11 | N3 | Encuadre con el láser a **baja potencia** (la casilla «Encuadrar con el láser encendido a baja potencia»; empieza en 1 %, el máximo es 20 %). Solo con gafas y material de desecho. | Se ve un trazo muy débil de la luz del láser. Si ya marca el material, baja el porcentaje. Los primeros reportes de la Falcon2 mencionan el sensor de inclinación al llegar al borde: observa si salta un `ERROR:01`. | R |
| 12 | N4 | «Disparo de prueba» (ventana Mover): 2 %, 200 ms (el valor por omisión), sobre cartón. Pide dos confirmaciones. | Un punto minúsculo y claro. El láser se apaga solo al terminar. Anota el porcentaje mínimo con el que marca y si el aire se enciende. | R |
| 13 | N5 | Trabajo chico: un cuadrado de línea de 20 mm, una pasada, a la potencia/velocidad que dice la biblioteca de materiales para tu material de desecho, un 30 % por debajo. Inicia desde el diálogo (revisa la lista de comprobación). | Se traza el cuadrado con un tamaño de 20 mm ±0,3 mm; empieza y termina con el láser apagado (`M5`) y el aire apagado (`M9`). El panel muestra avance, tiempo restante y vuelve a «Terminado». | P |
| 14 | N5 | Grabado chico: un relleno de 20 × 20 mm a 0,1 mm de intervalo con una imagen o un relleno por barrido. | Sin líneas dobles ni líneas desplazadas entre ida y vuelta. Si las hay, es el scan offset (sección 2.3 y `taller.md` 3): anota la separación medida. | R |
| 15 | N5 | Pausa y reanudar a mitad del trabajo del paso 13/14. | Se detiene (feed hold), el láser deja de emitir al pausar y reanuda en el mismo punto sin marcar. | R |
| 16 | N5 | Detener a mitad de un trabajo. | El cabezal se detiene, el láser se apaga, el estado queda en Reposo o alarma. Verifica que el aire también se apaga. | P |
| 17 | N5 | Si hay sensor: un fallo provocado a propósito (levantar el cabezal/inclinar con cuidado para la alarma de inclinación; tapar el aire). | Aparece `ERROR:NN` o `ALARM:3`. Tlaku debe **detener el envío** y mostrar el consejo de la alarma en vez de quedarse esperando un `ok`. La recuperación es con reinicio físico y el botón de inicio del panel de la máquina. | R |
| 18 | N6 | El trabajo real: corte de 3 mm de madera o el que uses a diario, en dos pasadas si hace falta. | Resultado equivalente al de LightBurn con el mismo material y ajustes. Compara tiempos (sección 12). | R |

**Nota: reinicio de la placa a mitad de sesión.** La Falcon2 puede reiniciarse sola (un final de carrera
al que se llega sin límites por software, la tapa, un corte de energía) y manda de nuevo
`[MSG:Using cpu_map...esp32-s3]` y `Grbl 1.1f ['$' for help]`. Tlaku distingue ese saludo del esperado
(el de la conexión y el que sigue a un `0x18` que mandó él mismo en los últimos ~3 s: cancelar,
parada segura, reset manual). Si es inesperado: **en reposo** avisa «La máquina se reinició (¿un final
de carrera, la tapa o un corte de energía?). Haz homing antes de mover», el panel Máquina muestra
«Posición sin confirmar» con un botón de homing y la revisión de la cama queda apagada hasta el
próximo homing; **con un trabajo en curso** lo detiene con la parada segura, lo marca como error («la
máquina se reinició a mitad del trabajo») y conserva el punto de control para reanudar (sección 2.5).
Para probarlo con cuidado: con el área despejada y sin trabajo, provoca el reinicio (apagar y encender
la máquina con el cable USB conectado, o abrir la tapa si tu modelo reinicia) y confirma el aviso; después
cancela un encuadre desde Tlaku y comprueba que **no** sale el aviso (el saludo lo provocó el reset de
Tlaku y la posición se conserva). Con `$22=1` y `$20=0` Tlaku recomienda además activar los límites por
software (`$20=1`) en «Ajustes de la máquina…»; nunca los escribe solo, y con `$20=1` GRBL exige que
`$130`/`$131` sean correctos.

### 2.3 Los ajustes nuevos de la ola 6C

Se prueban **después** del paso 7 (la máquina ya se conecta y se mueve) y se dejan al final como estaban.

| Ajuste | Dónde está | Qué hacer | Resultado esperado | Emulador |
|---|---|---|---|---|
| **Homing al conectar** (`home_on_connect`) | Perfil > Comunicación: «Ofrecer el homing ($H) al conectar» | Activarlo, desconectar y conectar. | Al conectar se abre la confirmación del homing; **nunca se envía solo**. Cancelarla deja la máquina conectada en reposo. Aceptarla hace `$H` y llega a `Idle`. | E (el diálogo) / P (el movimiento) |
| **Sin DTR** (`no_dtr_reset`) | Perfil > Comunicación: «Abrir el puerto serie sin subir DTR» | Con la máquina en una posición conocida tras `$H`, desconectar y conectar con el ajuste apagado; luego repetir con el ajuste encendido. | Con el ajuste apagado la placa **puede** reiniciarse (se ve de nuevo el saludo y se pierde el estado). Con el ajuste encendido debe conservar la posición y no reiniciarse. Si reinicia igual, anótalo: en Linux el controlador sube DTR siempre y algunos adaptadores (CH340) también. La Falcon2 es un ESP32-S3: confirma si el reinicio ocurre con DTR o con RTS. | R |
| **Envío síncrono** (`sync_streaming`) | Perfil > Comunicación: «Mandar una línea y esperar su «ok»» | Enviar el cuadrado del paso 13 con el ajuste apagado (conteo de caracteres) y luego encendido. | Los dos terminan con el mismo resultado físico. Con síncrono el movimiento puede entrecortarse en trazos finos (más lento): anota la diferencia de tiempo. Si con conteo de caracteres alguna línea se pierde y con síncrono no, es la señal de que el búfer RX real es menor de lo que dice `Bf:127,65535`. | P |
| **Intercambio X/Y** (`swap_xy`) | Perfil > Cama: «Intercambiar X e Y en la máquina» | **Solo con un diseño de 100 × 50 mm, láser apagado** (la cama 400 × 415 no es cuadrada y el eje cruzado podría pasarse del recorrido con diseños grandes). Encuadre con el ajuste apagado y luego encendido. | Apagado: el lado de 100 mm corre a lo largo del eje X de la máquina. Encendido: ese lado corre a lo largo del eje Y de la máquina y el de 50 mm a lo largo de X. Las flechas de «Mover» también cruzan sus ejes. Dejarlo **apagado** al terminar (la Falcon2 no tiene los ejes cruzados). | E (geometría con pruebas doradas) / R (el sentido físico) |
| **Desfase del puntero** (`pointer_offset`) | Perfil > Cama: «Desfase del puntero rojo» | La Falcon2 no trae puntero rojo. Haz la prueba como prueba de **matemática**: pon X=20, Y=0, encuadra el mismo cuadrado de 50 mm con el láser apagado y marca con un lápiz sobre el papel dónde queda el cabezal en una esquina. Repite con X=0. | Con X=20 la cabeza se mueve 20 mm a la izquierda de donde estaba con X=0 (la cabeza va a «el punto menos el desfase»); la forma y el tamaño del encuadre no cambian. Dejarlo en 0 y 0 al terminar. En una máquina con puntero rojo real, el puntero (y no el rayo) debe recorrer el borde del trabajo. | E / R (con puntero real) |
| **Unidad de velocidad** (`speed_unit`) | Perfil > Cama: «Unidad de velocidad» | Cambiar a mm/s y volver. | Los campos de velocidad del perfil y del diálogo de inicio muestran el valor convertido (25 000 mm/min = 416,7 mm/s); el G-code **sigue en mm/min** (`F25000`). | E |

Más sobre la Falcon2 que conviene mirar en la misma sesión (de `falcon2.md`, sección 6 y 8):

- **`ERROR:NN` con mayúsculas.** Hoy el streamer los reconoce. Provoca uno si puedes (por ejemplo `ERROR:24` abriendo la tapa o el cajón, si tu modelo los tiene) y confirma que el trabajo se detiene y que el mensaje es el de Creality, no el de GRBL.
- **G54-G59 con `-nan`.** Consulta `$#`. Si hay `nan`, Tlaku no debe mostrarlo ni romperse (no usa G54-G59).
- **G92.** Prueba si «Origen de usuario» (Iniciar desde: Origen de usuario) conserva el desplazamiento y qué pasa tras reiniciar.
- **Búfer.** Anota si `Bf:` se mantiene cerca de `127,65535` durante un trabajo; el conteo de caracteres de Tlaku usa 127 bytes por defecto.
- **Rotativo.** Si tienes el rotativo de Creality: modo mandril, 40 mm por vuelta y `$101=80`. El perfil debe quedar `kind: chuck`, `mm_per_rotation: 40`. Prueba una vuelta con «Prueba de una vuelta» sin material.
- **Scan offset.** Una tabla de velocidad contra separación (`taller.md` 3) no existe públicamente; la calibración de Tlaku (grabar una rejilla y medir) la crea.

### 2.4 Sonda de contacto y eje U (ola 6E)

Solo con una máquina GRBL que tenga una sonda o placa de contacto cableada a la entrada de la sonda (`Pn:P` en el reporte `?`) y, para el eje U, un eje extra que responda a `G0 U..`. **Láser apagado y sin material para todo lo de esta tabla.** Si no tienes sonda, salta las filas 1 a 4 y haz solo la 5 (modo del puntero).

| # | Qué hacer | Resultado esperado | Emulador |
|---|---|---|---|
| 1 | Sin tocar nada, mira el reporte `?` desde la consola con la sonda libre y luego presionándola a mano. | `Pn:P` aparece solo mientras la sonda está activa. Si aparece siempre o nunca, revisa el cableado y `$6` (invertir el pin de la sonda) antes de seguir. | P |
| 2 | Máquina > Buscador de centro con el cabezal…, modo «Con sonda». Velocidad 100 mm/min, recorrido 30 mm, el cabezal a unos 15 mm de un objeto conductor. «Hacia la derecha» y confirma. | Se muestra el G-code exacto (`M5`, `G91`, `G38.3 X30 Y0 F100`, `G90`). La cabeza avanza despacio, **se detiene al tocar** y retrocede 2 mm. La consola muestra `[PRB:x,y,z:1]`. Anota cuánto se pasa del punto de contacto al frenar (GRBL frena con una rampa) y si el retroceso devuelve el pin a «libre». | P |
| 3 | Repetir el toque con el objeto fuera de alcance (a más de 30 mm). | `[PRB:...:0]`, el aviso «No hubo contacto en 30 mm» y **ninguna alarma**; la máquina sigue en `Idle`. | P |
| 4 | Un objeto redondo de diámetro conocido (50 mm): tres toques desde tres lados y «Mover la cabeza al centro». | El centro y el diámetro coinciden con la medida real (±0,5 mm en el centro; el diámetro depende de la punta de la sonda, anótalo). Con la sonda ya activa al empezar debe salir `ALARM:4` explicado y la máquina bloqueada hasta `$X`. | R |
| 5 | Modo «Con el puntero»: lleva el puntero rojo (o la cabeza) a tres puntos del borde y confirma cada uno. | Mismo centro que el de la fila 4 (±1 mm). Con desfase del puntero en el perfil, el rayo queda en el centro al mover la cabeza. | R |
| 6 | **Eje U por capa.** Perfil con «Eje U»; dos capas, la segunda con «Desplazamiento U» de 5 mm. Ver el G-code en la vista previa y luego ejecutar con el eje U libre (sin pieza). | El G-code trae `G91` / `G0 U5` / `G90` entre las dos capas (con el láser apagado) y al final `G0 U-5`. El eje se mueve 5 mm hacia donde corresponde (anota el sentido) y vuelve a donde estaba. Con 0 en todas las capas el G-code es el de siempre. | E (el G-code) / R (el eje) |

Datos si falla: el registro de la consola (`[PRB:...]`, `Pn:`), la velocidad y el recorrido usados, el cableado de la sonda y `$6`.

### 2.5 Continuar tras un corte de luz (trabajo interrumpido)

Prueba la función «Continuar un trabajo interrumpido» ([manual](manual/09-enviar-con-seguridad.md#continuar-un-trabajo-interrumpido)) con la Falcon2 (`$22=1`, búfer de 127 bloques). **Solo después de los pasos 13 a 16 de la sección 2.2, y siempre con material de desecho**, gafas puestas y la mano cerca del paro de emergencia. Nada de esto se ha probado con una máquina real: el emulador cubre la lógica del punto de control y del preámbulo, pero no los tiempos de la placa ni lo que pasa con los motores al cortar la corriente.

| # | Nivel | Qué hacer | Resultado esperado | Emulador |
|---|---|---|---|---|
| 1 | N5 | Prepara un grabado con relleno (por ejemplo 40 × 40 mm a 0,1 mm de intervalo) o una imagen pequeña sobre material de desecho. Inicia desde el diálogo y espera a que pase del 40 % o 50 %. | El trabajo avanza con normalidad. Anota el porcentaje y la hora. | P |
| 2 | N5 | **Corte de luz:** a mitad del grabado, desenchufa la **fuente de alimentación** de la máquina (no solo el cable USB). Deja pasar unos segundos. No muevas el material ni el cabezal. | La máquina se apaga y el láser deja de emitir. Anota si el cabezal siguió un trecho por inercia. | R |
| 3 | N5 | Vuelve a conectar la fuente y enciende la máquina. Abre Tlaku (o déjala abierta y reconecta). | Aparece el aviso **El último trabajo se interrumpió al N %. ¿Continuar?** con **Continuar**, **Elegir el punto** y **Descartar**. El porcentaje se parece al que anotaste. El Registro de trabajos lo muestra como interrumpido y con botón **Continuar**. | P |
| 4 | N5 | Pulsa **Continuar**. Mira la vista previa del diálogo de reanudación y la barra del punto de reinicio. | Lo hecho se ve con línea continua y lo que falta con línea discontinua. El retroceso por omisión es de 3 líneas o 1,5 mm (lo que quede más atrás); anota el que usaste y si tuviste que moverlo. | E |
| 5 | N2 | Sin el home hecho, intenta reanudar. Luego haz **Hacer home ($H)…** (la Falcon2 tiene `$22=1`). | Sin home, el botón de reanudar no se activa y el diálogo dice por qué. Tras el home, MPos es ~0 y el estado pasa a Reposo. | P |
| 6 | N3 | Encuadra el trabajo completo (láser apagado) y completa la lista de comprobación. Confirma que el material sigue en su lugar. | El cabezal recorre el rectángulo del trabajo y cae sobre lo ya grabado. Si el encuadre no coincide, **para**: algo se movió. | P |
| 7 | N5 | Confirma la reanudación y mira el primer movimiento (mano en el paro). | El cabezal viaja hasta el punto de inicio **con el láser apagado**, y solo entonces empieza a grabar. El preámbulo en la consola es `M5`, `G90`/`G21`, aire, `G0` al punto y después `M4` con `S` y `F`. | P (el preámbulo) / R (el viaje) |
| 8 | N5 | Deja terminar el grabado. Mide con la regla y la lupa el punto de la unión. | El grabado continúa **sin hueco ni desplazamiento**. Mide el **solape o la marca** en la unión (esperado: hasta ~1 s de recorrido repetido más el retroceso, una marca levemente más oscura) y el **desfase** en mm entre lo grabado antes y después del corte (esperado: casi nulo; un desfase claro apunta a pasos perdidos al cortar la luz o a que algo se movió). | R |
| 9 | N5 | **Segunda interrupción:** repite los pasos 1 a 3, pero **cerrando la app** a mitad del grabado (no la luz). Abre Tlaku de nuevo. | Se conserva el punto de control y sale el aviso. El trabajo reanudado del paso 8 también deja punto de control, así que se puede interrumpir y continuar varias veces. | P |
| 10 | N5 | **Conexión perdida:** repite con el trabajo en marcha desconectando el **cable USB** y volviéndolo a conectar. | Al perderse la conexión aparece el aviso (sin cerrar la app). Con el USB reconectado, ofrece continuar. Anota si la placa conservó la posición o se reinició. | R |

Anota en cada intento: el **porcentaje** del aviso, el **retroceso usado** (líneas o mm), el **desfase en mm** medido en la unión, el sentido de la marca (más oscura o clara) y si hizo falta mover el punto con la barra. Esta prueba también sirve para confirmar el búfer real: si la marca de la unión es mucho mayor que ~1 s de recorrido, el búfer de la placa va más lleno de lo que dice `Bf:`.

Datos si falla: el registro de la consola completo (incluido el preámbulo enviado), el porcentaje y la línea del aviso, el retroceso usado, la carpeta `interrupted-job/` de la app y una foto de la unión con una regla a la vista.

### 2.6 Resultados de la primera sesión real (Falcon2 Pro 22 W, 2026-10-14, macOS)

Sin láser (potencia 0 en todo), sin regla, sin material y sin luz para la cámara. Detalle técnico en [`hardware/falcon2.md`](hardware/falcon2.md) §10. Firmware `CV50-Pro-MASTER-Release V1.0.0 20231129-1024`, GRBL `1.1f.20220810`, placa ESP32-S3 por USB (`303A:4001`).

| Paso | Resultado | Notas |
|---|---|---|
| 2.2 n.º 1-4 | Pasó | Saludo, `$I`, `$$` y `?` como se esperaba. `$#` trae `-nan` en la X de G54-G59, G28 y G30: Tlaku los ignora y avisa una vez. Se corrigió que la app eligiera `/dev/cu.debug-console` como puerto por omisión: ahora van primero los USB. |
| 2.2 n.º 6 | Pasó | Homing a la esquina inferior izquierda, MPos (0.025, 0.025). Reconectar no pierde la posición. |
| 2.2 n.º 7 | Pasó sin regla | Un cuadrado de jog de 50 mm vuelve exacto (según MPos). Un jog de −5 mm desde el origen **llegó al final de carrera y la placa se reinició** (no da `ALARM:1`): se añadió el bloqueo de jog fuera de la cama y la detección del reinicio. El bloqueo falta comprobarlo en la máquina con el perfil cargado. |
| 2.2 n.º 9 | Pasó sin regla | Encuadre con el láser apagado (S0) exacto según MPos. |
| 2.2 n.º 15 | Pasó con potencia 0 | Pausa y reanudar (`[MSG:Restoring spindle]`), vuelve a 0,0 al terminar. |
| 2.2 n.º 16 | Pasó con potencia 0 | Detener: el hold llega en ~120 ms, pero **la Falcon necesita ~16 mm para frenar a 3000 mm/min**. Tras el reinicio de Tlaku la placa conserva la posición. |
| Aire | Funciona | `M8`/`M9` funcionan; tras `M9` el aire sigue unos segundos más (el retardo de `$152`, 30 s de fábrica según foros; sin medir). |
| Tapa | **No se detecta** | Con la tapa abierta el estado sigue en `Idle`, sin `Door` ni pin: Tlaku no puede saber que la tapa está abierta. |
| Paro de emergencia | Corta la energía de la placa | Tlaku informa «se perdió la conexión» y guarda el punto de control (enviadas 9, confirmadas 8, MPos 167.725, 100). Al volver a encender: `Idle` en 0,0, sin alarma, sin homing. Con esos datos se corrigió el punto de reinicio (proponía repetir ~66 mm; ahora ~1,5 mm antes). |
| Tiempo estimado | Corto en ~15 % antes del arreglo | Cuadrado de 40 mm a 3000 mm/min: 4,24 s estimado contra ~4,9 s de movimiento. Faltaba el regreso al origen. Además el firmware tarda ~0,9 s en aceptar el `M4 S0` inicial de cada trabajo (un `G0` suelto arranca en ~60 ms). |
| Conexión | Se cayó 3 veces | Dos veces el puerto desapareció solo y una vez todo el árbol USB (cámara incluida): cable o alimentación. |

Falta con la máquina: medir con regla (pasos 7 y 9), calibrar la cámara (lente con tablero y alineación de 4 puntos), los pasos con láser (11 a 14, 17 y 18), el corte de luz real con material (2.5), el control desde el teléfono con la máquina real, el bloqueo de jog fuera de la cama con el perfil y comprobar el estimado corregido.

---

## 3. Otros GRBL (diodos genéricos, grblHAL, FluidNC, Marlin y Smoothie)

Mismo orden que la sección 2 (N1 a N5), con estas diferencias. Marca en el reporte el **dialecto elegido** (GRBL, GRBL-M3, grblHAL, FluidNC, GRBL-LPC, Smoothie, Marlin).

| Máquina | Qué cambia y qué mirar | Emulador |
|---|---|---|
| **GRBL 1.1 genérico (diodo)** | Los pasos 1 a 17 de la sección 2. Mira que `$32=1` (modo láser) esté activo: sin él, los rápidos con `S` encienden el láser. | E |
| **GRBL-M3** | Con M3 el láser queda siempre encendido a la potencia pedida; el grabado puede verse más oscuro en las esquinas (`falcon2.md`). Si tu placa solo admite M3, compáralo con M4. | P |
| **grblHAL** | `?` con campos extra, `$I` distinto, ajustes `$` propios (el panel marca «Ajuste sin descripción» a los desconocidos). Comprueba que `error:N` y `ALARM:N` coincidan con los de grblHAL (`grbl.md` 6.3). | P |
| **FluidNC** | La configuración real es `config.yaml`: «Ajustes de la máquina…» solo muestra `$$` con una nota. Prueba la conexión por WebSocket además de la serie. | P |
| **Marlin y Smoothie** | Envío línea a línea (sin conteo de caracteres), aviso de posición perdida, pausa por M0/M25. Probar el aire (`M106/M107` o `M8/M9`, según el dialecto). | P |
| **Cualquier GRBL con rotativo** | Diámetro real de la pieza, pasos por vuelta (`$101`) y sentido. Una vuelta de prueba **sin** material y midiendo con una marca. | E (matemática) / R (mecánica) |
| **Cualquier GRBL con eje Z** | Z por capa y relativo, «Enfocar», signo de Z. Primero Z sin el láser y con la mano en el paro (`taller.md` 2). | P |
| **Cualquier GRBL con eje U o láser 2** | Salida del eje U (`u_offset` por trabajo) y órdenes de selección del láser 2. Solo con un montaje real; sin él, deja `u_axis` y `laser2` apagados. | P |

Datos que sí o sí hay que mandar de una máquina GRBL nueva: `$I`, `$$`, `$#`, el saludo completo al conectar y qué pasa tras un `ALARM:3` o `ALARM:1`.

---

## 4. Conexión por red

Tlaku conecta GRBL por **TCP** y **WebSocket**, y Ruida por **UDP** (la sección 5). Se prueba la red solo cuando la conexión por USB ya funciona.

| # | Nivel | Qué hacer | Resultado esperado | Emulador |
|---|---|---|---|---|
| 1 | N1 | Perfil con conexión de red (host, puerto, TCP o WebSocket) y el botón «Probar conexión» sin conectar de verdad. | Dice si el host responde y qué saludo manda. Un host equivocado da un mensaje claro, no se cuelga. | E (`virtualnet:`) / R (red real) |
| 2 | N1 | Conectar y consultar `$I` y `$$`. | Mismo resultado que por USB. | P |
| 3 | N2 | «Mover» unos milímetros sin láser. | Movimiento con latencia normal; sin retrasos de segundos. | R |
| 4 | N3 | Encuadre. | Igual que por USB. | P |
| 5 | N5 | Trabajo chico (el del paso 13 de la sección 2). Durante el envío, desconecta el Wi-Fi de la máquina (o apaga el router) 2 segundos y reconéctalo. | Si se corta, Tlaku debe detener el trabajo y avisar (no seguir enviando a ciegas) y el láser debe apagarse. Anota si la placa conserva la posición. | P (corte simulado) / R |
| 6 | N5 | Un trabajo largo (más de 10 minutos) por red. | Sin pérdida de líneas ni pausas inexplicables. Anota el RSSI del Wi-Fi si lo sabes. | R |

Datos si falla: el host y el puerto (no la clave), TCP o WebSocket, el firmware de la placa, el registro de la consola y qué hizo el router (aislamiento de clientes, VLAN). La máquina virtual por red (`virtualnet:`) sirve para separar «mi red» de «Tlaku».

---

## 5. Ruida

**Experimental, sin una sola prueba real.** Probado solo contra el emulador UDP de `tlaku-ruida` (con el formato de documentos de MeerK40t). Plan base: [hardware/ruida.md](hardware/ruida.md) 10.6 y 14. Máquina recomendada: una RDC6442S/G o 6445G (magic `0x88`). **Todo el CO2 con tubo**: el agua circulando antes de cualquier prueba con láser, tapa cerrada, **interruptor del láser apagado (o potencia 0) hasta el paso 8**.

| # | Nivel | Qué hacer | Resultado esperado | Emulador |
|---|---|---|---|---|
| 1 | N1 | `tlaku ruida status <ip>` (solo lectura). Después, conectar desde la app y «Leer información». | Modelo, ID de tarjeta (por ejemplo `0x65106510` en una RDC6442S), cama, estado en reposo. Anota el **magic** detectado y si el ACK llegó como `C6`. La cama del perfil debe coincidir con la que informa la máquina (Tlaku la exige antes de enviar). | P |
| 2 | N1 | Panel «Controladora Ruida»: horas encendida, trabajos, recorrido. | Valores razonables. Las direcciones y unidades de esas lecturas (µm/s, µm/s²) **no están verificadas**: compáralas con las que muestra el panel de la máquina. | P |
| 3 | N1 | «Documentos en la memoria» > «Leer lista». | La cantidad y los nombres coinciden con el panel de la máquina (con la numeración desde 1). | P |
| 4 | N2 | Mover 10 mm con láser apagado y volver. Comprobar el *home*. | Signo de X e Y como esperabas; el origen del perfil (`origin`) coincide con el home real. | R |
| 5 | N3 | «Encuadrar». | Cinco saltos que dibujan el rectángulo, láser apagado. | P |
| 6 | N5 | `tlaku ruida send <ip> cuadrado.svg --dry-run` y luego **sin `--run`** (modo guardar). O desde la app con «Ejecutar al subir» **apagado**. | La máquina lista el documento (`TLAKUnnn`); no arranca. Al iniciarlo **desde el panel de la máquina** con el láser deshabilitado traza el cuadrado sin láser. Cortar la subida a mitad (desenchufar el cable): no debe quedar nada arrancado. | P |
| 7 | N5 | Subir y ejecutar con «Ejecutar al subir» (`--run`) y el láser **deshabilitado**. Probar pausa, reanudar y detener desde la app. | Empieza al llegar el último paquete; la pausa detiene el cabezal y reanuda en el mismo lugar; detener corta de inmediato. El estado vuelve a Reposo. | P |
| 8 | N5 | Con láser a baja potencia sobre material de desecho: cuadrado de 20 mm, comprobar tamaño (±0,1 mm); dos capas con velocidades distintas; aire encendido/apagado. | Tamaño correcto; las dos capas con la velocidad pedida; el aire sigue el estado de la capa. Después un ráster pequeño para ver la escala de la potencia variable (`C7`). | R |
| 9 | N5 | **Tope de velocidad.** Una capa a más de 36 000 mm/min (600 mm/s), por ejemplo 45 000. | Tlaku **avisa al calcular** y limita la velocidad del `.rd` a 36 000. Mide con cronómetro un recorrido de 400 mm: no debe ir más rápido de lo esperado a 36 000 y la máquina no debe perder pasos ni dar alarma. Prueba también con más de 128 capas: debe avisar. | E (aviso y recorte) / R (la máquina real a ese tope) |
| 10 | N5 | **Escribir ajustes (EXPERIMENTAL, ola 6E).** **Primer paso: activar la preferencia «Escrituras experimentales en Ruida»** (panel de Ruida; está apagada de fábrica, nunca se importa de un `.tlakuprefs` y pide escribir «ENTIENDO» o marcar una casilla). Sin ella, todo lo que escribe (ajustes, calibración, rotativo, borrar, ejecutar y reemplazar documentos) está deshabilitado en la interfaz y el motor lo rechaza; leer y exportar un respaldo no la necesitan. En la CLI, `tlaku ruida doc` exige `--experimental-writes`. Después: antes de escribir nada, **anota a mano todos los valores del menú de la máquina** y exporta un respaldo manual (Ajustes de la controladora > «Exportar respaldo…»). Primero **leer**: compara con el menú cada valor que muestra la ventana (velocidades en mm/s, aceleraciones, cama, paso de los ejes). Luego **un solo cambio inocuo**: la velocidad de home (`home_speed`) 1 mm/s menos, o el paso de un eje a su mismo valor ±1. Revisa la lista de diferencias, marca «Entiendo que esto cambia la configuración de mi controladora» y escribe. | El respaldo previo aparece en la lista «Respaldos automáticos» (carpeta `ruida-backups/` de la configuración) con la fecha; el informe dice «coinciden»; el menú de la máquina muestra el valor nuevo. Si el informe dice «NO coincide» o falla: **restaurar** (ver abajo) y mándanos el registro. Qué mirar de más: que la segunda copia del valor en `DA 01 dirección v v` no cambie otra cosa (compara **todo** el menú antes y después). | P (escritura, relectura, respaldo y restauración) / R (todo lo demás) |
| 10b | N5 | **Restaurar.** Cambia otro valor inocuo, escribe, y restaura el respaldo previo («Restaurar…» en la lista). Prueba también «Importar respaldo…» con el `.tlakuruida` exportado. | La ventana muestra las diferencias, pide la casilla, guarda un respaldo del estado actual, escribe y verifica; el menú vuelve a los valores anotados. Un respaldo de otra tarjeta se rechaza. | P / R |
| 10c | N5 | **Calibrar un eje (solo después de 10 y 10b).** Manda mover X 100 mm con el botón Mover, mide con una regla, escribe la medida en «Calibrar un eje» y carga el valor propuesto. | El valor propuesto es el paso actual × medido ÷ pedido. Tras escribirlo, repite la medición: el error debe **bajar**. Si **sube al doble**, el ajuste no es una longitud de paso (la dirección del cálculo estaría invertida): **restaura el respaldo** y avísanos. También anota la **unidad real** del paso (µm o no) y su resolución. | P (cálculo) / R |
| 10d | N5 | **Rotativo (M28).** Con el rotativo apagado, lee `rotary_pulse` y `rotary_diameter` y compáralos con el menú de la máquina («Rotate Pulse», «Rotate D»). Cámbialos a un valor distinto y vuelve a leer. | Cuál es la unidad real de cada uno y si coinciden con los del panel. **Cómo se activa el rotativo** sigue sin estar en ninguna fuente pública: anota el parámetro del menú que lo activa y su efecto en las lecturas. | P / R |
| 11 | N5 | **Documentos: nombre, borrar y ejecutar (EXPERIMENTAL).** Empieza con **un solo documento de prueba** en la memoria (guarda uno con el nombre `PRUEBA1`: campo «Nombre del documento» del diálogo Iniciar). Después guarda otro con el **mismo** nombre sin marcar el reemplazo (debe fallar sin tocar nada) y luego marcándolo. Con la lista a la vista: «Ejecutar» sobre `PRUEBA1` con el **láser deshabilitado** y pasando la lista de comprobación; y «Borrar». | (a) El nombre queda como lo escribiste. (b) Sin permiso, el error dice que ya existe y la lista no cambia; con permiso queda **uno solo**. (c) Ejecutar: la máquina traza el documento correcto (si solo selecciona, el registro avisa). (d) Borrar: la cuenta baja en uno y el panel de la máquina ya no lo lista. **Lo que hay que anotar:** si `E8 00 n(2) 00 00` y `E8 03 n(2)` fueron aceptados o se ignoraron (hay otra forma en EduTech/ruida-pa: `E8 03 n` con un byte y `E8 00` con dos valores de 35 bits); si guardar un nombre repetido duplica o sobrescribe; el nombre máximo que acepta. Con varios documentos, comprueba que el borrado y la ejecución actuaron sobre **el del nombre confirmado** y no sobre otro (por eso la primera prueba es con uno solo). | P (todo el flujo con el formato de MeerK40t) / R |
| 11b | N5 | **Láser 2 (M32).** Solo en una máquina de dos láseres, con ambos deshabilitados primero: una capa marcada «Láser 2» en un perfil con «Láser 2» activo; guardar y ejecutar con la potencia a 0. | La capa usa el segundo láser (`CA 01 11`); comprueba también si hace falta la máscara de tubos (`CA 03`) que Tlaku fija con el bit 1. Una capa de imagen con láser 2 se rechaza al calcular. | P (`.rd`) / R |
| 12 | N5 | **Importar `.rd` reales.** Archivo > Importar: abre al menos 3 `.rd` hechos con LightBurn o RDWorks (uno de corte, uno de relleno, uno con ráster). | Una capa por capa del archivo, con velocidad, potencia, color y aire; el origen se supone arriba a la izquierda y se avisa. La potencia variable `C7` se toma como la máxima. Compara el tamaño del diseño con el del archivo original. Si algo falta o sale volteado, manda el `.rd`. | P (ida y vuelta con archivos propios) / R (archivos reales) |
| 13 | N6 | Trabajo real chico (una pieza de 100 mm) en doble pasada. | Compara con el mismo trabajo hecho con LightBurn. | R |

Datos si falla: `tlaku ruida status <ip>` completo, el modelo de la controladora, el `.rd` que generó Tlaku (`tlaku ruida export-rd`) y, si puedes, **una captura de LightBurn del mismo trabajo** (hace falta una persona con la máquina y LightBurn) para comparar byte a byte, sobre todo el comando `E5 05`. Pasar a «perfil probado» exige pasos 1 a 3 correctos, el 7 sin sorpresas y esa captura comparada (`ruida.md` 14).

**Rotativo DSP.** Se activa en los ajustes de la propia controladora y Tlaku manda la Y en mm de superficie (lo avisa en el registro). Si en la máquina real el trabajo debía ir ya escalado, es un cambio de `export_rd_for_device`. Prueba una vuelta sin material.

**Cómo restaurar tras escribir ajustes.** (1) En la ventana «Ajustes de la controladora», lista «Respaldos automáticos» > «Restaurar…» del más reciente (el de antes del cambio), revisa las diferencias, marca la casilla y restaura; o «Importar respaldo…» con el `.tlakuruida` exportado a mano. (2) Los respaldos automáticos son archivos de texto JSON en la carpeta `ruida-backups/` de la configuración de la app. (3) Si la app no llega a conectar, **escribe a mano en el menú de la máquina** los valores que anotaste antes de empezar: el menú del panel es la restauración de último recurso. (4) Tras restaurar, vuelve a leer y compara con el menú. Un respaldo de otra tarjeta se rechaza a propósito.

**Qué probar primero en una máquina real (orden):** 1 a 3 (solo lectura, incluida la versión de la placa y las funciones que ahora se leen), luego 10 con **un cambio inocuo**, 10b (restaurar), y solo después 10c, 10d y 11. Los pasos 11 y 11b dependen de formatos de orden que las fuentes públicas no confirman. **Todas las escrituras de Ruida requieren antes activar «Escrituras experimentales en Ruida»**; apágala al terminar.

---

## 6. K40 con placa M2 Nano

**Experimental, sin una sola placa real.** USB directo (CH341, `1A86:5512`), con el código de `tlaku-lhymicro` contrastado con 1511 vectores de MeerK40t. La potencia de las capas **no se controla**: va con la perilla de la máquina. Plan base en el manual (`manual/13b-ruida-y-k40.md`) y [hardware/dsp-co2.md](hardware/dsp-co2.md) sección 4.

**Preparación:** agua circulando por el tubo **antes** de cualquier prueba con láser, tapa cerrada, gafas para CO2, **perilla de potencia al mínimo**, paro de emergencia al alcance y, si la máquina tiene un interruptor para deshabilitar el láser, apagado en las primeras pruebas. En Linux hace falta la regla de udev (`manual/13b`, «Permisos del USB»); en macOS el permiso de USB.

| # | Nivel | Qué hacer | Resultado esperado | Emulador |
|---|---|---|---|---|
| 1 | N0 | Sin máquina: `tlaku k40 send diseño.svg --emu --bed 300x200 --yes`. | «La placa confirmó el final del trabajo». | E |
| 2 | N1 | `tlaku k40 list`. | Una placa `1A86:5512`. Si sale una Moshiboard, `tlaku k40 status` lo avisa y **no** se sigue. | P |
| 3 | N1 | `tlaku k40 status`, y con `--repeat` pulsa los botones de la máquina. | «Estado: lista». El código de estado cambia con los botones. Anota los códigos y el modelo del chip (placa A, B, B1, B2, M, M1, M2 o M3). | P |
| 4 | N2 | En la app: conectar al perfil «K40 con M2 Nano» (`usb:k40:<n>`) y «Home». | El cabezal sube a la esquina de home (la placa no informa su posición; Tlaku hace su propio home). Si va a la esquina contraria, corrige `--home`/el perfil. | R |
| 5 | N3 | Enviar un cuadrado de 20 mm con el **láser deshabilitado** (interruptor o perilla al mínimo con el tubo sin alimentación). | Home, sube a la esquina, traza un cuadrado de 20 mm (±0,3 mm) y vuelve. El aviso previo (potencia con la perilla, grises binarizados, sin Z ni aire) aparece. | R |
| 6 | N4 | Detener a mitad de un trabajo, y el aviso del paro. | La máquina se detiene; si la placa no confirma, Tlaku **pide pulsar el paro de emergencia**. | P |
| 7 | N5 | Láser a la **mínima potencia** sobre cartón o papel de desecho; sube la perilla de poco en poco. Cuadrado de 100 mm. | Tamaño correcto (compara con una regla: ±0,3 mm); grabado y corte uniformes. Anota la velocidad máxima sin perder pasos. | R |
| 8 | N5 | Un trabajo con grabado de imagen (puntos). | Binarizado sin franjas ni líneas dobles. Anota la separación entre ida y vuelta si la hay. | R |
| 9 | N5 | **Importar `.egv` reales.** Archivo > Importar: al menos 3 `.egv` hechos con K40 Whisperer o MeerK40t (uno de corte, uno de grabado, uno de combinado). | Una capa por velocidad (sin potencia ni modo trama; supone placa M2); el origen se supone arriba a la izquierda y se avisa. Compara el tamaño. Si alguna variante de placa no decodifica, manda el archivo. | P (ida y vuelta) / R (archivos reales) |
| 10 | N5 | Desconectar el USB a mitad de un trabajo. | Tlaku lo detecta y avisa; anota si la máquina se detiene sola o sigue y **si el láser queda encendido** (por eso la perilla al mínimo y el paro a mano). | R |

Datos si falla: salida de `tlaku k40 list` y `status`, el modelo y año de la placa (foto de la placa si puedes), el `.egv` que generó Tlaku (`tlaku k40 export-egv`) y el sistema operativo (y la regla de udev en Linux).

---

## 7. Galvo (tarjetas JCZ/BJJCZ LMC)

**Experimental, solo emulador.** Un láser de fibra es clase 4 y su haz es invisible; el haz puede reflejarse. Sigue **al pie de la letra** el plan de [hardware/galvo.md](hardware/galvo.md) sección 9. Solo la tarjeta `9588:9899` (LMCV4-FIBER-M); otras se rechazan. EZCAD3/DLC y BSL, fuera de alcance. **No hay pausa a propósito**: solo se puede detener. Y **no se anuncia como soporte** hasta cumplir esto.

### 7.1 Sin fuente láser (la parte segura)

Materiales: una LMCV4-FIBER-M con cabezal galvo alimentado, **sin la fuente láser conectada** (o con el cable DB25 desconectado). Osciloscopio o analizador lógico en CLK/SYNC de CON1 y en MO, AP, PRR, PLATCH y P0-P7 de CON2. Alimenta la tarjeta y el cabezal **antes** de cualquier otra cosa (el manual de JCZ avisa que si no, el haz se desvía sin control).

| # | Nivel | Qué hacer | Resultado esperado | Emulador |
|---|---|---|---|---|
| 1 | N1 | `tlaku galvo list`. | Lista las tarjetas `9588:xxxx` y dice si son compatibles. No abre nada. | E |
| 2 | N1 | `tlaku galvo status` (serie, versión, estado, **entradas**). | Valores razonables; anota los bits de las entradas con la puerta abierta y cerrada, y con el paro de emergencia pulsado. Hay que **medir** cuáles son EMSTOP y las alarmas de la fuente (no están documentados). | P |
| 3 | N1 | En la app: perfil «galvo» (tipo de máquina, fuente, campo, frecuencia, anchos de pulso) y conectar. Aparece el diálogo de seguridad láser (gafas, recinto, paro) que hay que confirmar **en cada conexión**. | Se conecta y muestra el estado; sin la confirmación no deja conectar. | E |
| 4 | N2 | Encuadre con el puntero rojo (`tlaku galvo light` o el panel Galvo: rectángulo, contorno, casco, objetos). | Se ve el puntero recorrer el rectángulo; el centro cae en `0x8000`; el tamaño coincide con el campo en mm (anota el error en una regla a la distancia de trabajo). **El bit del puntero (8) no está verificado en una tarjeta.** | R |
| 5 | N2 | Medir en las señales: que el encuadre **nunca** habilita el láser (MO, AP y PRR en reposo durante todo el encuadre). | MO, AP y PRR en reposo. | R |
| 6 | N3 | `tlaku galvo mark --red-only` (todo el trabajo con el láser deshabilitado y el puntero rojo). | Recorre todo el trabajo sin que MO/AP/PRR se muevan. | R |
| 7 | N3 | **Cancelar a mitad** de un trabajo (Ctrl+C o Detener). | `DisableLaser`/EMSTOP dejan MO, AP y PRR en reposo; la secuencia de apagado se completa. | R |
| 8 | N3 | **Desconectar el USB a mitad de un trabajo.** Lo más importante. | Ninguna fuente de software puede garantizar el apagado: **hay que medirlo**. MO, AP y PRR deben quedar en reposo. Si no, **no se sigue**. | R |
| 9 | N3 | Medir la frecuencia generada en PRR contra la pedida (por ejemplo 30 kHz) y los retardos TC contra los pedidos. | Frecuencia ±2 %; retardos con la resolución, el signo y la unidad esperados (esperas en pasos de 10 µs). Las fórmulas de velocidad, frecuencia, potencia y retardos de `params.rs` **vienen de galvoplotter y MeerK40t y no están verificadas**. Anota las discrepancias. | R |
| 10 | N3 | **Entradas requeridas.** En el perfil galvo, el enclavamiento: una máscara y un valor que deben cumplirse antes de cada trabajo (`--require-input MASK=VALOR` por CLI). Con la entrada en un estado y luego en otro. | Con la condición cumplida el trabajo arranca; **con la condición rota, Tlaku se niega a iniciar** y lo dice. Anota qué bit es la puerta y cuál el paro. | P (la lógica) / R (los bits reales) |
| 11 | N3 | **Overscan.** En rellenos e imágenes el perfil lleva «Overscan de rellenos e imágenes» (0 a 20 mm; 0 = sin overscan). Con la fuente desconectada y el puntero rojo, generar un relleno con overscan 0 y otro con 2 mm. | Con 2 mm el trazo empieza y termina fuera de la línea de relleno (los espejos recorren el margen con el láser apagado, a la velocidad de marcado, sin retardo de salto); el trazo arranca a velocidad constante. | P |

### 7.2 Con la fuente conectada (solo si 7.1 salió bien)

Recinto cerrado con enclavamiento, gafas con la DO correcta para la longitud de onda y la potencia, obturador, extinción de incendio y un material que no importe.

| # | Nivel | Qué hacer | Resultado esperado | Emulador |
|---|---|---|---|---|
| 1 | N4 | Un **punto** con la **potencia mínima** y el período corto. | Una marca puntual. Anota la potencia mínima que marca. El orden de encendido (MO antes que AP) debe respetarse. | R |
| 2 | N5 | Una **línea** y un **cuadrado** a baja potencia. | El tamaño coincide con el diseño (±0,2 mm sobre 50 mm). Compáralo con el mismo trabajo hecho con EZCAD2. | R |
| 3 | N5 | Una **rejilla de calibración** con el asistente de calibración del campo (perfil galvo > «Asistente de calibración del campo…»): marcar a potencia baja, medir con regla, aplicar. | Calcula la escala y el barril con el error de cada punto; tras aplicar, una segunda rejilla debe salir mejor. | P |
| 4 | N5 | Desde la capa: frecuencia, ancho de pulso (si MOPA), retardos y bamboleo. | Los efectos del cambio se ven y se miden (ancho de línea, brillo). | R |
| 5 | N5 | Marcado dividido/repetido y rotativo galvo (solo si tienes la mesa o el rotativo): la app **espera tu confirmación** entre piezas. | Las piezas encajan con el solape pedido; el rotativo gira lo que dice el perfil (el signo del giro se ajusta con el montaje). | R |
| 6 | N5 | Cilindro y cono (si tienes una pieza cilíndrica). | La proyección corrige la deformación a lo largo de la curvatura. **Modelo propio sin verificar con hardware.** | R |
| 7 | N6 | Repetir con una segunda tarjeta o clon si se quiere declarar compatibilidad más amplia que «probado con X». | | R |
| 8 | N5 | **Asistente de nueve puntos** (perfil galvo > «Asistente de nueve puntos…»): marcar la cuadrícula de 3 x 3 cruces a potencia baja, medir cada cruz **respecto de la del centro** (X a la derecha, Y hacia arriba, con el trazo +X a la derecha), aplicar y repetir. Mide también con una regla alineada a los ejes de la máquina y activa «Corregir también el giro» solo en esa vuelta. | El error máximo por cruz baja en cada repetición (el asistente dice cuánto espera que baje); anota el error antes y después. El modelo es propio: si la segunda rejilla no mejora, anota las medidas. | P (el ajuste) / R (la lente) |
| 9 | N3 | **`.cor` de EZCAD aplicado en el equipo.** Con una lente con su `.cor` de CorFile2, poner la ruta en el perfil y hacer primero el **encuadre del puntero rojo** y el **recorrido en rojo** (láser deshabilitado), midiendo dónde cae el puntero con y sin el archivo. Probar las casillas «Intercambiar los ejes de la tabla» e «Invertir el signo de la tabla». | Con la combinación correcta el error del puntero en la rejilla **baja**; con las otras, sube. Anota cuál es: **la lectura de la tabla (qué índice es X, el signo, el rango de los nodos) es una inferencia sin verificar**. Un archivo que desplaza más de 2048 unidades se rechaza. **Confirma con un analizador USB (o el registro de la sesión) que no sale ningún `WriteCorTable`, `WriteCorLine` ni otro comando de configuración**: Tlaku nunca escribe el `.cor` en la tarjeta. | P (la lógica, y que no se escribe nada) / R (la semántica de la tabla) |
| 10 | N5 | **Partir por formas enteras** con un diseño de varias formas más ancho que el campo (marcado dividido > «Partir por formas enteras»). | Cada forma sale completa en una sola pieza, sin costura; entre piezas la app pide mover el material. | R |
| 11 | N5 | **Rotativo del galvo** con su propio editor (diámetro de la pieza, pasos por vuelta, eje, sentido): marcar dos franjas de una pieza cilíndrica. | El giro que pide la app (grados y pasos) lleva la pieza a la franja siguiente con el empalme esperado; si gira al revés, «Invertir el sentido del giro». Con el eje Y, las franjas son verticales. | R |
| 12 | N5 | **Vista previa de la deformación** (cilindro y cono, «Mostrar la deformación en el lienzo») contra una pieza real: mide dónde cae cada punto de una cuadrícula marcada en una taza o un cono. | La rejilla de la vista previa coincide con la marcada. **El modelo es propio y se fijó con casos calculados a mano, no con una pieza**; el recorte en el horizonte del cilindro convexo (`acos(R / (R + D))`) tampoco está medido. | R |

Publicar el soporte con la etiqueta «probado con: tarjeta, fuente, lente» y bandera explícita para el resto (`galvo.md` 9.6). El control desde el celular **nunca** inicia un galvo.

**Regla que no cambia con ninguna función nueva:** diálogo de seguridad en cada conexión USB, sin pausa, encuadre solo con el puntero rojo, nunca se escribe la EEPROM ni la configuración de la tarjeta, y se rechazan los VID:PID desconocidos. Las pruebas `tests/wave6e_safety.rs` (`tlaku-galvo`) y `machine/galvo/tests/wave6e.rs` (app) lo comprueban contra la tarjeta simulada, que cuenta cualquier comando fuera de la lista blanca.

Datos si falla: `tlaku galvo list` y `status`, el perfil exportado, fotos del montaje (tarjeta, fuente, lente y modelo del cabezal), el volcado de la lista (`tlaku galvo mark --dump`), las mediciones del osciloscopio si las hay y qué señal se movió cuando no debía.

---

## 8. Cámara

**Todo se probó con imágenes sintéticas.** Antes de cualquier prueba con láser, la cámara se prueba sin disparar.

### 8.1 Ojo de pez y calibración de lente

| # | Qué hacer | Resultado esperado | Emulador |
|---|---|---|---|
| 1 | Panel Cámara: elegir la cámara USB y ver el video en vivo. | Imagen estable, sin parpadeo; la resolución pedida. | R |
| 2 | Calibración de lente con el tablero (5 capturas: cuatro esquinas y el centro). Para un ojo de pez, deja que Tlaku elija el modelo por validación cruzada. | Un porcentaje de error bajo; después de aplicar, **las rectas salen rectas** en la rejilla. Un número bajo no basta: mira la rejilla. | P (imágenes sintéticas) / R |
| 3 | Alineación con marcas grabadas (recomendada): graba el patrón de marcas en material de desecho y deja que Tlaku las detecte. | Detecta las marcas (al menos 4) y calcula la homografía; la validación de dejar una marca fuera da un error en mm. Compara con la regla: ±1 mm en el centro y hasta 2 a 8 mm en los bordes con ojo de pez es lo que reporta la gente. | P / R |
| 4 | Altura del material: repite con dos grosores distintos. | El error crece con el grosor sin corrección; con la corrección por altura debe bajar. | P |

### 8.2 Cámara de red real

| # | Qué hacer | Resultado esperado | Emulador |
|---|---|---|---|
| 1 | Añade una cámara de red por URL de snapshot HTTP o MJPEG (red local; con usuario y clave en la URL si hace falta). | Imagen que se actualiza. **Solo `http://`**: `https://`, autenticación Digest y RTSP no se admiten. Si tu cámara solo sirve RTSP o Digest, dilo en el reporte. | E (servidor local de prueba) / R |
| 2 | Cortar la red de la cámara a mitad. | La vista avisa «sin imagen» y no se cuelga; vuelve sola al restablecerse. | P |
| 3 | Guardar la configuración con nombre y exportar el paquete de configuración. | El `.tlakubundle` **no** incluye usuario ni clave de las URL. | E |

### 8.3 Cámara en el cabezal

| # | Qué hacer | Resultado esperado | Emulador |
|---|---|---|---|
| 1 | Calibración del desfase cámara-láser con una marca grabada (varias posiciones del cabezal). | Desfase en mm con un error bajo. | P |
| 2 | Alineación automática con marcas (un cuadro de 9 marcas grabado en un lugar conocido; lado mínimo 60 mm). | Detecta las marcas sin clics; el desfase y la escala resultantes concuerdan con la regla. | P |
| 3 | Recorrido en cuadrícula (con confirmación, láser apagado, cancelable) de una zona dibujada en el lienzo. | El cabezal recorre la zona; el mosaico une las fotos con mezcla en los bordes, sin costuras visibles ni desplazamientos. | P |
| 4 | Un trabajo real posicionado sobre el mosaico. | El trabajo cae donde se vio en la imagen (±1 mm). | R |

### 8.4 Buscador de centro

| # | Qué hacer | Resultado esperado | Emulador |
|---|---|---|---|
| 1 | Un objeto redondo conocido (por ejemplo una taza o moneda de 50 mm). 3 clics en el borde o detección automática del círculo. | El centro y el diámetro coinciden con la medida real (±1 mm). | E (imágenes sintéticas) / R |
| 2 | «Ir al centro» con el láser apagado. | El cabezal queda sobre el centro (compara con una cruz). | R |
| 3 | El mismo objeto con el cabezal como sonda o con el puntero (ver 2.4, filas 2 a 5). | El centro coincide con el de la cámara (±1 mm). | E (contra el emulador) / R |

Datos si falla: capturas de la calibración (porcentaje de error, rejilla antes y después), foto del montaje y la cámara (modelo, resolución, lente), el `camera.json` sin la clave y el paquete de usuario con la sección de cámaras.

---

## 9. Print & Cut con imágenes giradas

Máquina > Print & Cut…. Se probó solo con la máquina virtual y fotos sintéticas. **Desde la ola 6C las imágenes giran con el registro** (se vuelven a muestrear sobre la caja girada); esta es la primera vez que se probará con una impresión real.

| # | Nivel | Qué hacer | Resultado esperado | Emulador |
|---|---|---|---|---|
| 1 | N0 | Con la máquina virtual: agregar marcas (2 a 4), «medir» con posiciones simuladas con un giro de 2° y verificar el error. | Error medio por debajo de 0,3 mm; avisos de escala, espejo o estirón cuando corresponda. | E |
| 2 | N2 | Diseña unas pegatinas (un logo vectorial y una imagen) con 3 marcas. Imprime a **tamaño real al 100 %** (no «ajustar a página»). Mide la hoja impresa con una regla. | Las marcas están a la distancia pedida ±0,3 mm en el papel. | R |
| 3 | N2 | Pon la hoja **un poco torcida** (2 a 5°) sobre la cama. Mide cada marca con el láser apagado (flechas y «Guardar posición») o con la cámara. | El resultado muestra el giro (2 a 5°), la escala (~100 %) y el error de cada marca (<0,3 mm). | R |
| 4 | N3 | Encuadre del trabajo transformado, láser apagado. | El rectángulo sigue la hoja torcida. | P |
| 5 | N5 | **Grabar la imagen incluida a baja potencia** (y el vector como línea). | La **imagen sale girada igual que la hoja** (antes salía recta); los bordes de la imagen coinciden con los del impreso (±0,3 mm). Sin escalones visibles por el remuestreo a 2° y 5°. | P (la rotación se prueba con la máquina virtual) / R |
| 6 | N5 | Repetir con ajuste uniforme y afín, con una impresión **estirada** a propósito (imprímela al 98 % en un eje). | El ajuste afín corrige el estirón; el uniforme deja error en marcas. | R |
| 7 | N6 | Cortar la pegatina. | El corte cae sobre el borde impreso (±0,5 mm). | R |

Datos si falla: la foto de la hoja con las marcas medidas, las posiciones guardadas de cada marca, la captura del resultado (giro, escala, error por marca) y el `.tlaku`.

---

## 10. Tiling y alimentador

Máquina > Dividir en tramos…. Probado con la máquina virtual y fotos sintéticas. El alimentador usa el G-code de avance del perfil (de fábrica `G91`, `G0 Y{mm}`, `G90`): **el plan del alimentador es lo más especulativo de todo lo de 5/6**.

| # | Nivel | Qué hacer | Resultado esperado | Emulador |
|---|---|---|---|---|
| 1 | N0 | Con la máquina virtual: un letrero de 600 × 300 mm en una cama de 400 × 415 (dos tramos, solape de 5 mm, marcas de alineación). | Plan con dos tramos, marcas en la zona de solape. | E |
| 2 | N3 | Mismo letrero con la Falcon2 y papel de desecho. Primer tramo; encuadre. | Cabe en la cama menos el margen (5 mm). | P |
| 3 | N5 | Grabar el primer tramo con las marcas de alineación a baja potencia. Mover el material a mano (sin alimentador) y grabar el segundo. | Las marcas del solape coinciden (±0,5 mm) y el diseño continúa sin escalón. | R |
| 4 | N5 | Usar «Saltar», «Repetir» y cerrar la app a mitad y **retomar**. | Se ofrece retomar solo si el diseño, las opciones y la máquina son los mismos y continúa en el tramo correcto. | E (el retomar) |
| 5 | N2 | **Alimentador:** activar «Alimentador automático» en el perfil; dejar el G-code de avance por defecto, avance de 10 mm y probar el avance **sin láser y con el material suelto**. | El material (o la banda) avanza exactamente `{mm}` ±0,5 mm. Sin máquina con alimentador, esta prueba se hace solo mirando el G-code que muestra el perfil. | R |
| 6 | N5 | Tiling en **una columna** con el alimentador: dos tramos y avance entre ellos. | La app espera a que la máquina vuelva a reposo, avanza (alto del tramo menos el solape) y envía el siguiente tramo sin pedir confirmación (el primero sí la pide). Anota si `G91`/`G0 Y{mm}`/`G90` basta o si hay que esperar más tras el avance. | R |
| 7 | N5 | **Repetir con el alimentador** (diálogo de inicio, de 2 a 99 veces) con un trabajo chico, 3 repeticiones. | Cada repetición se graba tras el avance; el panel de avance muestra cuántas van; «Detener» corta la que está en curso y no deja salir más. | R |
| 8 | N5 | Tiling con **varias columnas** (letrero de 600 × 300 mm, solape de 10 mm, marcas activadas). | Al terminar el primer tramo se abre el diálogo «Mueve el material a la columna 2 de 2» con las marcas de registro (las cruces del solape). «Llevar el puntero» (o la cabeza, sin puntero) va al sitio donde la cruz grabada debe quedar; el material se mueve a mano hasta que coincidan (±0,5 mm) y «Ya está colocado» envía el tramo. | P |
| 9 | N5 | Varias columnas con alimentador y orden «Serpentina» (dos columnas, dos filas). | Entre dos filas de la **misma** columna el material avanza solo (alto del tramo menos el solape) y el tramo sale sin confirmar; al cambiar de columna vuelve la pausa guiada. Anota si el avance de una fila deja las marcas del solape alineadas. | R |
| 10 | N5 | Con el material ya movido a mano a la columna 2, comprobar con «Llevar el puntero» que las cruces del tramo 1 coinciden con las que se grabarán. | El desfase entre la cruz grabada y la del tramo nuevo es de ±0,5 mm o menos. Si es mayor, anótalo: puede ser holgura del material o el margen del perfil. | R |

Datos si falla: el G-code de avance del perfil y lo que hizo el material, fotos de las marcas de alineación en el solape y el avance de tramos (registro del tiling).

No hay prueba del alimentador nativo de una controladora DSP (Ruida o Trocen): no existe en Tlaku porque no hay documentación pública del comando de avance; el alimentador de Tlaku es siempre G-code de GRBL.

---

## 11. Control desde el celular (iPhone y Android)

Preferencias > Control remoto. Servidor local de la app, **apagado de fábrica**, solo red local, sin HTTPS, con una clave aleatoria en el QR que cambia cada vez que se abre la app. Probado con la máquina virtual, el servidor de demostración (`cargo test -p tlaku-desktop demo_server -- --ignored --nocapture`) y pruebas automáticas, **nunca con un celular real**. Siempre con la máquina virtual primero, y con una máquina real solo estando junto a ella.

| # | Nivel | Qué hacer | Resultado esperado | Emulador |
|---|---|---|---|---|
| 1 | N1 | Activar el control remoto. Mira «Escuchando en 192.168.x.x:47802» y el QR. | La dirección es una IP privada de tu red. | E |
| 2 | N1 | **iPhone (Safari) y Android (Chrome):** escanear el QR con la cámara y abrir el enlace. Repetir con «Copiar enlace» enviado a mano. | La página carga. Anota el modelo del celular, la versión de iOS/Android y el navegador. Si el celular está en otra red (datos móviles, Wi-Fi de invitados con aislamiento), no conecta: es lo esperado. | R |
| 3 | N1 | Con la máquina virtual y un trabajo corriendo: estado, porcentaje, tiempo restante, capa y recorrido con el punto del cabezal. | Se actualiza en vivo; sin saltos de más de unos segundos. | E / R (latencia) |
| 4 | N1 | «Quién está conectado», «Desconectar» y «Regenerar clave». | El celular desconectado muestra «Te desconectaron» y el QR viejo ya no sirve. | E |
| 5 | N2 | **Pausar, Reanudar y Detener** desde el celular con una máquina real y el láser apagado. | Pausar y Reanudar funcionan; **Detener** pide un segundo toque («Toca otra vez para DETENER») y va directo a la máquina aunque la ventana de la PC esté ocupada. | P / R |
| 6 | N2 | Iniciar un trabajo desde el celular. Condiciones: marcar la lista de comprobación **en el celular**, y que el trabajo ya se haya **encuadrado en la PC**. | Si falta algo, la barra dice por qué. Con todo cumplido, arranca. | E |
| 7 | N1 | Fin del trabajo (y un fallo provocado). | Aviso a pantalla completa («¡Trabajo terminado!» o «El trabajo falló»), pitido, parpadeo del título y pantalla encendida mientras trabaja. **La vibración funciona en Android; en Safari de iPhone no existe** (limitación del navegador). Anota si se apaga la pantalla en iPhone con el trabajo en curso. | R |
| 8 | N1 | Bloquear el celular a mitad del trabajo y desbloquearlo. | Reconecta solo y muestra el estado actual. | R |
| 9 | N1 | Imagen de la cámara de la PC (si está encendida). | Se actualiza cada pocos segundos. | R |
| 10 | N1 | Intentar iniciar un **galvo** desde el celular. | **Nunca** permite iniciar; solo detener. | E |
| 11 | N1 | Cerrar y volver a abrir Tlaku. | El QR anterior ya no sirve. Un archivo de preferencias o un `.tlakubundle` importado **jamás** enciende el control remoto. | E |

Datos si falla: modelo y versión del celular, navegador, tipo de red (Wi-Fi doméstico, guest, VPN), la dirección que muestra la PC, captura de la página del celular y del diálogo de la PC. Para pruebas serias, Wi-Fi de confianza: sin HTTPS, lo que pasa por la red local se puede espiar.

---

## 12. Tiempos estimados contra reales

La prueba de estrés dio una mediana de **+1,2 %** entre el estimado de Tlaku y el simulado por el emulador GRBL (con aceleración y velocidades máximas igualadas al perfil), pero **eso es contra el emulador**; el estimado nunca se calibró con tiempos reales (C58). Hace falta contrastarlo con la máquina.

| # | Qué hacer | Resultado esperado | Emulador |
|---|---|---|---|
| 1 | Lee `$110`, `$111` (velocidades máximas) y `$120`, `$121` (aceleraciones) con `$$` y ponlas en el perfil (la Falcon2 publica 1000 o 5000 mm/s² según el volcado). | El perfil refleja la máquina. | n/a |
| 2 | Cuatro trabajos tipo: (a) un corte de línea largo (≥ 2 min), (b) un relleno grande, (c) un grabado de imagen, (d) muchos trazos cortos (un mapa o texto). Para cada uno anota el **tiempo estimado** que mostró la vista previa. | Los cuatro estimados quedan anotados. | n/a |
| 3 | Ejecuta cada uno y cronometra con un reloj (de «Iniciar» a «Terminado»). El **Registro de trabajos** (Ayuda > Registro de trabajos…) guarda el estimado y el real. | La diferencia estimada contra real: dentro de ±10 % es aceptable; por encima, anótalo. | R |
| 4 | Repite (b) y (d) con el envío síncrono apagado y encendido. | El síncrono es más lento; anota cuánto. | R |

Lo que más suele fallar: el estimado **asume parada entre rápidos y cortes**, mientras la máquina real mezcla con la curvatura (junction deviation); en segmentos cortos el enlace serie puede ser el límite (a 115 200 baudios son unas 600 líneas por segundo). Datos si hay diferencia grande: `$$` completo, el G-code, el estimado, el real y el registro de la consola.

---

## 13. Prueba de humo de Tauri en Windows y Linux

`apps/desktop/scripts/smoke-tauri.sh` (`npm run smoke:tauri` dentro de `apps/desktop`) arranca la aplicación real con una ventana oculta, manda los comandos de solo lectura por `invoke` y revisa las respuestas. Detecta lo que el modo simulado no ve: comandos mal registrados, argumentos con el nombre equivocado (camelCase contra snake_case) y estado sin registrar. Hoy se corre en macOS; **falta correrla en Windows y Linux** y no hay un job de CI que lo haga (el CI compila y prueba en las tres plataformas, pero no corre la prueba de humo).

| # | Plataforma | Qué hacer | Resultado esperado | Emulador |
|---|---|---|---|---|
| 1 | **Linux** | Instala `libwebkit2gtk-4.1-dev`, `libgtk-3-dev` y `xvfb`; corre `apps/desktop/scripts/smoke-tauri.sh` (si no hay `$DISPLAY` se relanza con `xvfb-run`). | Sale con código 0 en menos de 90 s y no toca los datos reales (aísla `HOME` y `XDG_*`). | E (corre en CI de Linux si se agrega el job) |
| 2 | **Windows** | No hay script: corre el equivalente en PowerShell, `cargo run -p tlaku-desktop --example smoke`, con `HOME`/`APPDATA` apuntando a una carpeta temporal. | Termina con código 0. Anota las rutas donde escribe (`%APPDATA%`) y si algún comando falla por rutas con `\` o con espacios. | R |
| 3 | Ambas | Abrir la app instalada (el `.exe`/`.msi` en Windows, el `.AppImage`/`.deb` en Linux), crear un diseño, importar un SVG y guardarlo. | Abre sin errores; guarda y reabre. Anota si falta el driver USB o un permiso. | R |
| 4 | **Linux** | Conectar un puerto serie real (la Falcon2): ¿tu usuario pertenece al grupo `dialout`? | Se abre sin `permission denied`. Para el K40, la regla de udev. | R |
| 5 | **Windows** | Conectar con el driver CH340/CH341. | El puerto aparece como `COMn` y conecta. | R |
| 6 | Ambas | «Nueva ventana» (`Ctrl/Cmd+Mayús+N`): D19 está Hecho desde la ola 6E con la prueba de humo de Tauri (ventanas reales, sin interfaz web); esta fila es la prueba a mano con la interfaz completa. | Abre un documento independiente que comparte máquina y preferencias; la recuperación de una ventana no se ofrece en la otra. | R |
| 7 | Ambas | Tras un cierre forzado (matar el proceso), reabrir. | Ofrece recuperar o descartar la copia. | P |

Datos si falla: la salida completa de la prueba, el sistema y su versión, el tipo de sesión gráfica (X11, Wayland, Windows) y las rutas de configuración.

---

## 14. Resumen: qué cubre el emulador y qué solo la máquina real

| Área | Lo que ya cubre el emulador o las pruebas | Lo que solo la máquina real puede validar |
|---|---|---|
| GRBL / Falcon2 | Protocolo, conteo de caracteres, alarmas y fallas simuladas, preflight, punto de control y preámbulo de reanudación, dialectos, geometría de `swap_xy` y `pointer_offset`, diálogo de homing al conectar, ajustes `$`, la sonda `G38.2` a `G38.5` contra una superficie simulada y el G-code del eje U por capa | Pin de la sonda real (`$6`), cuánto frena tras el contacto, eje U real, DTR y reinicio de la placa, continuar tras un corte de luz (reinicio de la placa, pasos perdidos, marca y desfase en la unión), envío síncrono real, `ERROR:NN` físicos, sensores, scan offset, potencia y tamaño del resultado, búfer real, G92, tiempos |
| Red | `virtualnet:` (TCP local), WebSocket, «Probar conexión» | Latencia y pérdida del Wi-Fi, ESP32 reales, aislamiento del router |
| Ruida | Emulador UDP, `.rd` de ida y vuelta, aviso y recorte de velocidad, lectura y escritura de ajustes con respaldo, confirmación, verificación y restauración, borrar/ejecutar documentos con el formato de MeerK40t, nombre y reemplazo, láser 2 en el `.rd` | Magic y ACK reales, unidades de las lecturas y de los ajustes, **que la controladora acepte `DA 01 dirección v v` y no cambie nada más**, la forma real de `E8 00` y `E8 03`, numeración de documentos, qué hace guardar un nombre repetido, la dirección de la calibración, cómo se activa el rotativo, `CA 01 11` y la máscara `CA 03`, `.rd` reales, el tope de velocidad en la máquina |
| K40 | Placa simulada, codificador contra 1511 vectores, `.egv` de ida y vuelta | USB CH341 real, códigos de estado, home, paro, `.egv` reales, potencia por perilla |
| Galvo | Tarjeta simulada con rastro del haz y fallos inyectables, compilador, apagado garantizado | Todo lo de la sección 7: unidades, bits de estado y de salida, caída del USB, EMSTOP, frecuencia, campo y distorsión |
| Cámara | Calibración, homografía, marcas, mosaico y círculo con imágenes sintéticas | Cámaras reales (USB y de red), ojo de pez, paralaje, cámara en el cabezal real, buscador de centro real (cámara, sonda y puntero) |
| Print & Cut | Ajuste, errores y rotación de imágenes en la máquina virtual | Hoja impresa real torcida, remuestreo visible, corte sobre el borde |
| Tiling y alimentador | Plan, recorte, marcas, retomar, G-code de avance, la guía de columnas y las marcas de registro | Avance real del material, espera tras el avance, alinear el material a mano con las cruces |
| Control remoto | Servidor, autenticación, página con la máquina virtual | iPhone y Android reales, red doméstica, vibración, pantalla encendida |
| Tiempos | Estimado contra emulador (+1,2 % de mediana) | Estimado contra cronómetro |
| Tauri | Smoke en macOS | Windows, Linux, instaladores, permisos de USB |

## 15. Plantilla del reporte

Copia y pega:

```
Máquina:           (marca, modelo, controladora, firmware)
Tlaku:             (versión / commit, sistema operativo)
Sección y paso:    (por ejemplo 2.2 paso 11)
Qué esperaba:
Qué pasó:
$I:
$$:
Registro de la consola:
Adjuntos:          (.tlaku, G-code, .tlakubundle, capturas, fotos con regla)
```
