# Galvo (experimental)

Un láser **galvo** mueve el haz con dos espejos giratorios en vez de mover un cabezal: es el que usan los marcadores de **fibra**, **UV** y **CO2 galvo** para grabar metales, plásticos y vidrio a gran velocidad. Lo controla una tarjeta (la más común es la **LMCV4-FIBER-M**, que se maneja con el programa EZCAD2) conectada por USB.

> **Lee primero [Seguridad del galvo](13d-galvo-seguridad.md).** Un láser de fibra es **clase 4** y su haz es **invisible**: puede dañar la vista de forma permanente, incluso por una reflexión difusa, y quemar lo que haya detrás. Esta página explica qué hace Tlaku; esa explica cómo no hacerte daño.

> **Estado: experimental y sin probar con una tarjeta real.** Todo el soporte galvo se validó contra una **tarjeta simulada** que escribimos nosotros: eso prueba que nuestro código es coherente con lo que *creemos* que dice el protocolo, no que una tarjeta de verdad lo interprete igual. Las unidades de la velocidad, la frecuencia, la potencia y los retardos vienen del código de otros proyectos de código abierto y **no están verificadas con hardware**.

> **Dónde se maneja:** el galvo se maneja hoy desde la **línea de comandos** (`tlaku galvo`); la interfaz en la app está en camino.

## Qué puede hacer hoy

- **Listar** las tarjetas conectadas y decir si son compatibles, sin abrir nada (`tlaku galvo list`).
- **Leer el estado** de la tarjeta, sin mover ni encender nada (`tlaku galvo status`).
- **Encuadrar con el puntero rojo**: dibuja el rectángulo del diseño (o su contorno) en bucle, **sin habilitar nunca el láser** (`tlaku galvo light`).
- **Marcar** el diseño, o recorrerlo entero con el láser deshabilitado y el puntero rojo (`tlaku galvo mark`, con `--red-only`).
- **Compilar y revisar** el trabajo sin conectar con nada (`--dry-run`), volcar la lista de comandos a un archivo (`--dump`) y probar todo contra la tarjeta simulada (`--emu`).

En el motor también hay: parámetros por capa (frecuencia, ancho de pulso, retardos, saltos, repeticiones), bamboleo, [grabado 3D por cortes y corrección de cilindro y cono](12b-grabado-3d-y-cilindro.md), y la secuencia del marcado dividido, repetido y rotativo.

## Qué tarjetas se aceptan

Solo la **`9588:9899`** ("BJJCZ USBLMCV4", la LMCV4-FIBER-M de EZCAD2). Cualquier otro identificador USB se **rechaza**, con el motivo: los clones `9588:9980`, `9588:9990` y `9588:9999` (no se sabe si usan el mismo protocolo) y la tarjeta sin firmware `9588:9880` (necesita cargar el firmware del FPGA, y Tlaku no escribe en la tarjeta). Las tarjetas **EZCAD3 / DLC** y las **BSL** quedan fuera de alcance: no hay documentación pública de su protocolo.

Ver cuáles hay y si valen:

```
tlaku galvo list
```

Sin tarjetas conectadas dice «No hay dispositivos BJJCZ (USB 9588:xxxx) conectados».

## Probar sin máquina: la tarjeta simulada

Todo lo siguiente se puede hacer sin comprar nada. Con un diseño de ejemplo (`cuadro.svg`, un cuadrado de 30 mm):

```
tlaku galvo mark cuadro.svg --emu --frequency 30 --power 5
```

Tlaku compila el trabajo, lo manda a la tarjeta simulada y resume lo que ocurrió: cuántas tandas y comandos, que «la fuente estuvo armada durante el trabajo» y que «la fuente se apagó y la lista se vació», y lo que vio el emulador (tramos con el haz saliendo, que la fuente terminó apagada y 0 violaciones del protocolo).

Para ver el encuadre con el puntero rojo (dos vueltas y termina):

```
tlaku galvo light cuadro.svg --emu --loops 2
```

Y para solo compilar y revisar, sin conectar con nada:

```
tlaku galvo mark cuadro.svg --dry-run --frequency 30
```

que imprime algo como «Lista de la tarjeta: 18 comandos en 1 tandas de 256; 4 marcas (120.0 mm), 1 saltos (70.7 mm) …» y termina con «Simulacro (--dry-run): no se conectó con la tarjeta».

## El perfil de la máquina: banderas

Desde la línea de comandos, el perfil de tu máquina se escribe en cada comando:

| Bandera | Qué es | De fábrica |
|---|---|---|
| `--field` | Lado del campo de la lente F-theta, en mm (110, 150, 200, 300…) | 110 |
| `--source` | `fiber-q` (fibra Q-switched), `fiber-mopa`, `co2` o `uv` | `fiber-q` |
| `--freq-range MIN-MAX` | Rango de frecuencia de **tu fuente**, en kHz. Fuera de él no se envía nada | `20-80` |
| `--pulse-widths` | Anchos de pulso permitidos en ns, separados por comas (obligatorio con `fiber-mopa`) | — |
| `--max-power` | Potencia máxima del perfil, en %. Una capa que la supere es un error | 100 |
| `--light-speed` | Velocidad del puntero rojo, en mm/s | 2000 |
| `--jump-speed` | Velocidad de los saltos, en mm/s | 4000 |
| `--invert-x`, `--invert-y`, `--swap-xy`, `--rotate` | Cómo está montado el cabezal: negar un eje, intercambiarlos o girar el campo (grados, antihorario) | — |
| `--require-input MASK=VALOR` | Enclavamiento por software (ver [Seguridad del galvo](13d-galvo-seguridad.md)) | — |

Y los parámetros del marcado (`mark`):

| Bandera | Qué es | De fábrica |
|---|---|---|
| `--frequency` | Frecuencia en kHz. **Obligatoria** y dentro del rango del perfil | — |
| `--pulse-width` | Ancho de pulso en ns (fuentes MOPA y UV; tiene que estar en `--pulse-widths`) | — |
| `--mark-speed` | Velocidad de marcado, en **mm/s** | 1000 |
| `--power` | Potencia en % | **10** (baja a propósito) |
| `--laser-on`, `--laser-off`, `--polygon`, `--end-delay` | Retardos del láser en microsegundos (pueden ser negativos los de encendido y apagado) | los del perfil |
| `--repeat` | Veces que la tarjeta repite el trabajo | 1 |

Los demás argumentos del diseño (`--layer`, `--mode`, `--interval`, `--image-size`…) son los mismos que `tlaku gcode`; `tlaku galvo mark --help` los lista.

Un ejemplo con una fibra MOPA de campo 150 mm:

```
tlaku galvo mark logo.svg --dry-run --source fiber-mopa --pulse-widths 4,50,200 \
  --pulse-width 50 --frequency 40 --power 15 --field 150
```

Los anchos de pulso del ejemplo son inventados: escribe los que admite **tu** fuente según su manual. Tlaku rechaza un ancho que no esté en la lista y una frecuencia fuera del rango, con un mensaje claro («frecuencia 5 kHz fuera del rango del perfil (20 a 80 kHz)»).

## Cómo se coloca el diseño

El campo de la lente es un **cuadrado centrado**. En la línea de comandos el diseño se coloca con el origen (abajo a la izquierda) en la esquina inferior izquierda del campo y la Y hacia arriba. Un diseño que se sale del campo **no se marca** (`mark` falla); el encuadre te dice el rectángulo exacto («Encuadre: X 5.00 a 35.00 mm, Y 5.00 a 35.00 mm (campo de 110 mm)»).

Un galvo **no usa overscan**: los espejos tienen poca inercia, así que los rellenos y las imágenes salen sin el margen que sí necesita un láser de pórtico.

## Un flujo seguro: del emulador a la fuente

1. **Sin máquina.** `--emu` y `--dry-run` hasta que entiendas la salida.
2. **Tarjeta conectada, sin fuente láser** (el cable de la fuente desconectado). Es la prueba de [Seguridad del galvo](13d-galvo-seguridad.md): identificar la tarjeta y mover los espejos sin que nada pueda emitir.
3. **Con la fuente, `--red-only`**: recorre todo el trabajo con el **láser deshabilitado** y el puntero rojo, para ver el recorrido real.
4. **Marcar de verdad**, con la potencia mínima, sobre un material de desecho, con gafas, recinto y paro de emergencia.

Con hardware real, **todo menos `list`** exige la bandera `--i-understand-laser-safety`; sin ella ni siquiera se abre el USB. `mark` además pregunta «El láser DISPARARÁ. ¿Gafas puestas, recinto cerrado, obturador y paro de emergencia listos, y el material correcto bajo el haz? [s/N]»; `--yes` omite la pregunta, y **no deberías usarlo** en tus primeras pruebas. **Ctrl+C** detiene el trabajo y apaga la fuente.

## Lo que Tlaku no hace (a propósito)

- **No hay pausa**: solo se puede detener. Ver [por qué](13d-galvo-seguridad.md).
- No escribe la **EEPROM**, el firmware, la **tabla de corrección** ni la configuración de la tarjeta. Si tu tarjeta necesita inicializarse, hazlo antes con EZCAD2.
- No aplica los archivos de corrección **`.cor`** de EZCAD (se lee su estructura, pero no se sabe qué significan los números). La corrección del campo (escala, giro, barril, trapecio, sesgo) la calcula Tlaku en el equipo con un modelo propio, y supone que la tabla de la tarjeta es la identidad.
- No marca **al vuelo**, no maneja ejes Z ni mesas, y no escribe salidas arbitrarias.
- No tiene un asistente de calibración de la lente ni importa archivos de configuración de EZCAD.

Siguiente: [Seguridad del galvo](13d-galvo-seguridad.md)
