# Solución de problemas

Qué significan las alarmas y errores que muestra Tlaku y qué hacer cuando no hay puerto, el láser no dispara o el tamaño no sale bien.

> **Ojo:** estas soluciones salen del código de Tlaku, de la documentación pública del firmware GRBL y de reportes de usuarios. Nada se ha probado con una máquina real todavía. Si algo no coincide con lo que ves, la máquina manda: ante la duda, detén el trabajo y apaga el láser.

## Dónde mirar primero

- Si hay una alarma, la app muestra un banner con el código, una explicación y botones **Desbloquear ($X)…** y **Hacer home ($H)…**. Cada botón pide confirmación; Tlaku nunca los envía por su cuenta.
- La pestaña **Consola** guarda todo lo enviado y lo recibido. El botón de copiar deja el registro en el portapapeles, útil para pedir ayuda (menú **Ayuda > Reportar un problema…**).
- Para ver los ajustes de la máquina, envía `$$` desde la consola (hay un botón de consulta rápida).

## Alarmas `ALARM:N`

Una alarma bloquea la máquina hasta que la desbloqueas o haces homing. Estos son los códigos con texto propio en Tlaku.

| Código | Significado | Qué hacer |
|---|---|---|
| `ALARM:1` | Se activó un final de carrera. | Aparta el cabezal del límite a mano y desbloquea. |
| `ALARM:2` | El movimiento pedido sale del recorrido de la máquina. | Igual; revisa que el diseño quepa en la cama. |
| `ALARM:3` | Reinicio durante un movimiento: se perdió la posición. | Haz homing si hay finales de carrera; si no, fija el origen a mano. Ver la sección de Falcon2. |
| `ALARM:4`, `ALARM:5` | Fallo de sonda. | Desbloquea. No aplica a un láser normal. |
| `ALARM:6` a `ALARM:9` | Fallo de homing (reinicio, puerta, final de carrera no liberado o no encontrado). | Revisa puerta, finales de carrera y cableado; repite `$H`. |
| `ALARM:10` | Depende del firmware: autoescuadre sin segundo final de carrera (GRBL 1.1), paro de emergencia (grblHAL) o fallo del husillo (FluidNC). | Si la app conoce tu firmware, te dice cuál. Con paro de emergencia, suéltalo antes de desbloquear. |
| `ALARM:11` a `ALARM:22` | Solo grblHAL (por ejemplo: 11 pide homing, 12 límites activados). | Lee el texto del banner. |

Cuándo usar cada botón:

- **Con finales de carrera y homing activado (`$22=1`):** lo normal es **Hacer home ($H)**. El cabezal viaja hasta los finales de carrera, así que despeja el área antes.
- **Sin finales de carrera (`$22=0`):** lo normal es **Desbloquear ($X)**. Sin homing, la máquina no sabe dónde está: vuelve a fijar el origen.
- `$X` no comprueba nada. Úsalo solo si sabes por qué saltó la alarma.

## Errores `error:N`

Salen cuando la máquina rechaza una línea de G-code o un comando.

| Código | Significado | Causa frecuente |
|---|---|---|
| `error:1` | Falta la letra del comando. | Línea mal escrita en la consola o en el G-code de inicio. |
| `error:2` | Formato de número incorrecto. | Un `S` sin valor en el G-code de inicio o de fin del perfil. |
| `error:3` | Comando `$` no válido. | Comando de ajuste mal escrito. |
| `error:5` | Ajuste desactivado. | `$H` en una máquina sin homing activado. |
| `error:8` | La máquina no está en reposo. | Comando enviado durante un movimiento. |
| `error:9` | G-code bloqueado por alarma. | Hay una alarma sin resolver. |
| `error:20` | Comando G-code no soportado. | El firmware no entiende alguna orden del dialecto (revisa el tipo de controladora). |
| `error:22` | Velocidad de avance sin definir. | Falta `F` en un movimiento. |
| `error:33` | Destino del movimiento no válido. | Coordenadas fuera de lo que acepta el firmware. |

Los números **18, 19 y desde el 39** significan cosas distintas en grblHAL y en FluidNC. Si la app no conoce tu firmware, solo dice "error extendido"; en FluidNC puedes consultarlo con `$E=N`.

> **Ojo:** `error:5` (minúsculas, de GRBL) no es lo mismo que `ERROR:05` (mayúsculas, de Creality). Tlaku los distingue; tú también al leer la consola.

## Creality Falcon2

La Falcon2 tiene códigos propios, que llegan como `ERROR:NN` (mayúsculas y dos dígitos). Tlaku los muestra en un banner aparte, **Falla de la máquina (Creality)**.

| Código | Significado |
|---|---|
| `ERROR:01` | Ángulo anormal: se activó el sensor de inclinación o vibración. |
| `ERROR:02` | No hay archivo G-code en la raíz de la tarjeta TF. |
| `ERROR:04` | Sin flujo de aire: revisa bomba, perilla de caudal y tubo. |
| `ERROR:05` | Llama detectada: la máquina se detuvo. |
| `ERROR:06` | Lente contaminada: límpiala. |
| `ERROR:07` | Módulo láser demasiado caliente. |
| `ERROR:08` a `ERROR:12` | Fallo de un sensor del módulo: reinicia la máquina. |
| `ERROR:13`, `ERROR:14` | Temperatura alta de la CPU del módulo o del anfitrión. |
| `ERROR:24` | Máquina no sellada: cierra la tapa y el cajón. |
| `ERROR:25` | Bomba de aire anormal. |

Para recuperarte, corrige la causa y reinicia con el botón **Reset** de la máquina (y Start en su panel si lo pide). Tlaku no la desbloquea por su cuenta.

Otros puntos de la Falcon2, según reportes públicos (sin probar aquí):

- **`ALARM:3` es lo más común.** Las causas que citan los usuarios son fuente de alimentación débil, cable USB, un sensor que reinicia la placa o subir demasiado la potencia en el módulo de 40 W. Prueba otra fuente y otro cable.
- **Cable USB-C:** un cable de USB-C a USB-C puede no funcionar; usa uno de USB-A a USB-C. Necesita el driver CH340.
- **Cama y velocidades:** distintos firmwares traen valores distintos. Lee los de tu máquina con `$$` y compáralos con tu perfil.
- **Desplazamientos G54 a G59:** pueden mostrar `-nan`. Tlaku no los usa.
- Si el grabado sale más oscuro en las esquinas, el perfil debe usar `M4` (tipo **GRBL 1.1**), no el tipo antiguo con `M3`.

Puedes ver cómo se muestran estas fallas sin riesgo en la máquina virtual (**Simular falla**: llama, inclinación, alarma 3, cable desconectado). Ver [Conectar tu máquina](08-conectar-maquina.md).

## La app no encuentra el puerto

1. Revisa que el cable transmita **datos** (algunos solo cargan) y que la máquina esté encendida.
2. Pulsa **Buscar puertos de nuevo**.
3. Cierra cualquier otro programa que tenga el puerto abierto (otro software de láser o un monitor serie). Solo uno a la vez puede usarlo.
4. Instala el driver de la placa si hace falta (ver abajo) y vuelve a conectar.

**Windows.** Abre el Administrador de dispositivos y mira **Puertos (COM y LPT)**. Si tu máquina sale como "dispositivo desconocido" o con una advertencia, falta el driver del chip, normalmente **CH340** (a veces FTDI). Instálalo, reconecta el cable y elige el puerto `COMn` que aparezca.

**macOS.** Algunas máquinas con chip CH340 necesitan el driver del fabricante del chip. Tras instalarlo, el puerto aparece con un nombre como `/dev/cu.usbserial-...`. Si macOS pide aprobar una extensión del sistema, hazlo en Ajustes del Sistema y reinicia. Verifica en tu versión de macOS si hace falta driver.

**Linux.** Tu usuario debe pertenecer al grupo `dialout` (en Arch y derivadas, `uucp`):

```sh
sudo usermod -aG dialout "$USER"
```

Cierra la sesión y vuelve a entrar para que surta efecto. Si la máquina aparece y desaparece, suele ser `ModemManager` o `brltty` quedándose con el puerto: desactiva `ModemManager` (`sudo systemctl disable --now ModemManager`) o desinstala `brltty`.

## Conecta pero no responde

- **Baudios:** la mayoría de los GRBL usan 115200; Marlin suele usar 250000; algunos modelos Neje piden 38400.
- **Espera unos segundos.** Abrir el puerto reinicia muchas placas; Marlin tarda más.
- Si la detección dice "No se reconoció", revisa el cable, prueba otros baudios o elige **Otro / genérico** en el asistente.
- Si la máquina no devuelve ajustes, comprueba que sea una controladora GRBL.

## El láser no dispara

Revisa en este orden:

1. **Modo láser apagado.** Si la app avisa de `$32=0`, actívalo con `$32=1` (hay un botón en el panel Máquina). Sin eso, `M4` no sigue la velocidad y el grabado sale mal.
2. **Potencia.** La capa tiene potencia mayor que 0 y la **S máxima** del perfil coincide con `$30` de la máquina (1000 en GRBL actual, 255 en versiones antiguas, 1 en GRBL-LPC y Smoothie).
3. **Salida de la capa.** La capa tiene la salida activa (no está apagada ni es de guías: las guías nunca se envían) y tiene objetos visibles de su modo.
4. **Tipo de controladora.** Un tipo que no corresponde a tu firmware cambia el G-code (por ejemplo `M4` donde solo existe `M3`). Revisa el perfil.
5. **Seguridad de la máquina.** Tapa, llave, paro de emergencia o sensores (en la Falcon2, `ERROR:24` o `ERROR:01`). Mira la consola.
6. Si usas la máquina virtual, recuerda que **no hay láser real**: no se graba nada.

## El tamaño no sale como esperabas

- **Mide en el lienzo primero.** Selecciona el objeto y mira Ancho y Alto en la barra numérica. Las imágenes se importan a 96 píxeles por pulgada: una foto de muchos píxeles puede salir grande o pequeña. Corrige en la barra numérica.
- **Máquina que se mueve de más o de menos.** Si el cuadro sale a otra medida, `$100` y `$101` (pasos por mm de X e Y) pueden estar mal. Mide lo que hizo la máquina y ajusta: nuevo valor = valor actual por (medida pedida / medida real). Hazlo desde la consola y con cuidado.
- **Rotativo activado.** Con un rotativo, la escala de Y cambia. Si no lo usas, desactívalo en el perfil.
- **Cama del perfil.** El área de trabajo del perfil debe coincidir con `$130` y `$131`.
- **Se sale de la cama.** La sección **Iniciar desde** del panel Máquina avisa "Se sale de la cama" e indica por qué lado.
- **Sale espejado o invertido.** El **Origen de la máquina** del perfil no coincide con la esquina real de tu X0 Y0.
- **Sale desplazado.** Revisa **Iniciar desde** y el punto del trabajo que se ancla: ver [Enviar trabajos con seguridad](09-enviar-con-seguridad.md).

## Si nada de esto sirve

Copia el registro de la consola y usa **Ayuda > Reportar un problema…**. Indica tu firmware (versión que devuelve `$I`), el tipo de controladora del perfil y si lo probaste en la máquina virtual.

Siguiente: [Glosario](16-glosario.md)
