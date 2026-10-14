# Controladoras soportadas

Qué firmware entiende Tlaku, en qué estado está cada uno y qué dialecto de G-code usa. Léela antes de conectar una máquina real.

> **Ojo, lo más importante:** Tlaku **nunca se ha probado con una máquina real**. Todo se validó con el emulador (`tlaku-emu`), con las máquinas virtuales de la app y con pruebas automáticas. Cuando esta página dice "estable en emulador" significa exactamente eso: el flujo funciona contra un láser simulado, no contra el tuyo. Haz tu primera prueba con el láser a potencia baja, material de desecho, sin dejar la máquina sola y con el paro de emergencia a mano.

## Estados que usa esta página

| Estado | Qué significa |
|---|---|
| **Estable en emulador** | Conecta, mueve, encuadra, envía, pausa, detiene y maneja alarmas contra un emulador. Falta probarlo con hardware. |
| **Por validar con máquina real** | Existe en la app o en el motor, pero con menos cobertura o sin máquina virtual propia. |
| **Experimental** | Está marcado así en la app o en la línea de comandos. Se probó solo contra un emulador o una placa simulada y puede equivocarse con una máquina de verdad. |
| **No soportada** | No hay código. |

## Tabla de controladoras

Dos columnas dicen con honestidad lo que se hizo: qué se **probó en emulador** y qué queda **por validar con hardware**. En todas, lo segundo es «todo»: ninguna se ha probado todavía con una máquina real.

| Familia | Tipo en el perfil | Conexión | Estado | Probado en emulador | Por validar con hardware |
|---|---|---|---|---|---|
| GRBL 1.1f o posterior (la mayoría de los diodos) | GRBL 1.1 | USB serie | Estable en emulador | Conectar, mover, encuadrar, enviar, pausar, detener, alarmas. Máquina virtual **GRBL 1.1** y **Falcon2 Pro** | Todo. La primera prueba real será una Creality Falcon2 |
| grblHAL (por ejemplo, Ortur Laser Master 3) | grblHAL | USB serie; red TCP | Estable en emulador | Igual; máquina virtual **grblHAL** | Todo, incluida la red |
| FluidNC | FluidNC | USB serie; red TCP o WebSocket | Estable en emulador | Igual; máquina virtual **FluidNC** | Todo, incluida la red |
| Marlin 2.x con láser | Marlin | USB serie | Estable en emulador | Envío línea a línea y arranque lento; máquina virtual **Marlin** | Todo |
| Smoothieware | Smoothieware | USB serie | Estable en emulador | S de 0 a 1 por defecto; máquina virtual **Smoothieware** | Todo |
| GRBL antiguo (1.1e o anterior, solo `M3`) | GRBL antiguo (sin M4) | USB serie | Por validar con máquina real | Solo el G-code (pruebas del motor); sin máquina virtual propia. Potencia constante; Tlaku escribe `S0` en los desplazamientos | Todo |
| GRBL-LPC (Cohesion3D) | GRBL-LPC / Cohesion3D | USB serie | Por validar con máquina real | Solo el G-code; sin máquina virtual propia en la lista. S de 0 a 1 | Todo |
| G-code propio | G-code personalizado | USB serie | Por validar con máquina real | Tú escribes el dialecto; parte del de GRBL 1.1 | Todo |
| Ruida (CO2 de caja con pantalla) | Ruida (CO2, por red; experimental) | Red (UDP) | **Experimental** | Emulador UDP propio: conectar, sondear, subir guardando o ejecutando, pausa, reanudar, detener, saltos, encuadre, información y lista de documentos. Máquina virtual **Ruida (CO2)** | Todo. Direcciones y unidades de la información, numeración de documentos, home, estados de pausa. Ver [Ruida y K40](13b-ruida-y-k40.md) |
| K40 con placa M2 Nano | K40 con M2 Nano (experimental) | USB directo | **Experimental**: solo desde la línea de comandos (`tlaku k40`) <!-- K40-APP --> | Placa simulada: CRC, paquetes, estados, ejecución en reloj virtual; las velocidades de cada placa contra 1511 vectores de otro proyecto de código abierto | Todo: el USB real, la placa, el home, los tiempos. Ver [Ruida y K40](13b-ruida-y-k40.md) |
| Galvo (fibra, UV, CO2 galvo; EZCAD) | — | — | **No soportada** | — | — |
| Trocen, TopWisdom | — | — | **No soportada** | — | — |
| xTool por Wi-Fi propietario, Glowforge | — | — | **No soportada** | — | — |

Un K40 con la placa cambiada a GRBL-LPC o Smoothie funciona como esas familias. xTool por USB se trata como GRBL (ver más abajo).

### Conexión por red

Para GRBL, grblHAL y FluidNC la app conecta por **TCP / Telnet** o **WebSocket** además del puerto serie, con «Probar conexión» y una máquina virtual por red. Se eligen en el perfil; ver [Conexión por red y automatización](08b-red-y-automatizacion.md). Ruida usa UDP con la IP del perfil. No hay conexión cifrada (`wss://`) ni búsqueda de máquinas en la red.

### Funciones de máquina: qué se probó

| Función | Probado en emulador o con datos de prueba | Por validar con hardware |
|---|---|---|
| Macros, posiciones guardadas, mover Z y U, enfocar | Máquinas virtuales GRBL y pruebas automáticas | Todo |
| Disparo de prueba y encuadre con el láser encendido | Máquinas virtuales (se comprueba el G-code y que `M5` salga siempre) | **Todo, y es lo más delicado: enciende el láser** |
| Ajustes `$$` con respaldo | Máquinas virtuales; escritura de uno en uno con relectura | Todo; un ajuste crítico mal puesto mueve mal la máquina |
| Láser 2 | G-code generado, con las órdenes del dialecto | Todo, con el firmware que uses |
| Ojo de pez | Cámaras sintéticas | Una lente real |
| Cámara en el cabezal y mosaico | Máquina virtual y fotos sintéticas | Una cámara montada en un cabezal |
| Buscador de centro | Imágenes sintéticas | Una cámara real |
| Automatización por UDP | Pruebas del protocolo y de la seguridad de `START` | — (es local; no depende de la máquina) |

Las máquinas virtuales que ofrece la app son seis (**Falcon2 Pro**, **GRBL 1.1**, **grblHAL**, **FluidNC**, **Marlin** y **Smoothieware**, cada una también **por red**) más **Ruida (CO2)** con un perfil Ruida. Ver [Conectar tu máquina](08-conectar-maquina.md).

## Ruida y K40

Las dos están marcadas **experimentales** y tienen su propia página: qué hacen, qué no hacen, por qué Ruida solo guarda el trabajo por defecto, cómo probarlas con cuidado y por qué en el K40 **la potencia va con la perilla de la máquina**. Ver [Ruida y K40](13b-ruida-y-k40.md).

## Marcas del catálogo

El asistente **Agregar máquina** trae un catálogo de 21 entradas: 18 modelos concretos y 3 familias genéricas (GRBL, Marlin y Smoothie).

| Marca | Modelos del catálogo |
|---|---|
| Creality | Falcon2 22 W, Falcon2 40 W, Falcon2 Pro (22 / 40 W) |
| Sculpfun | S9, S30 / S30 Pro / S30 Pro Max |
| Atomstack | A5 Pro, A10 Pro, X20 Pro |
| Ortur | Laser Master 2 Pro, Laser Master 3 |
| xTool | D1, D1 Pro, S1 |
| Longer | Ray5 (5 / 10 / 20 W) |
| Neje | Master 2S / 2S Plus |
| Two Trees | TTS-10 Pro, TTS-20 Pro |
| K40 | K40 con Cohesion3D (GRBL-LPC) |

Cómo leer el catálogo con honestidad:

- Que un modelo esté en la lista **no** significa que esté probado. Solo significa que Tlaku conoce su firmware y algunos datos.
- Cada dato (cama, velocidad, S máxima, baudios...) cita su fuente: **Oficial**, **Tienda o reseña** o **Foro**. Los de tienda y foro pueden diferir de tu unidad.
- Lo que no tiene fuente queda en blanco y el asistente te pide leerlo de la máquina (`$$`).
- La Falcon2 es la única máquina con una investigación a fondo (sus alarmas propias `ERROR:NN` están en la [solución de problemas](15-solucion-de-problemas.md)). Aun así, su mayor parte sale de reportes públicos de la Pro; nadie la ha conectado todavía.
- xTool usa un GRBL con extensiones propias: si el G-code falla, prueba primero con el perfil genérico. Solo por USB.

## Dialectos de G-code

El **tipo de controladora** del perfil decide el dialecto de partida: cómo se enciende el láser, qué rango tiene `S`, si hay `M4`, cómo se apaga y cómo se manda el aire. Se cambia en **Gestionar dispositivos**, campo "Tipo de controladora"; al cambiarlo, la app te ofrece aplicar los valores de partida del tipo o solo cambiar el tipo.

| Tipo | S máxima de partida | Potencia dinámica |
|---|---|---|
| GRBL 1.1, grblHAL, FluidNC | 1000 | `M4` |
| GRBL-LPC / Cohesion3D | 1 | `M4` (mismo dialecto que GRBL 1.1) |
| GRBL antiguo (sin M4) | 255 | No, solo `M3` |
| Smoothieware | 1 | No, encendido constante |
| Marlin | 255 | No, encendido constante con potencia en cada `G1` (`M3 S0 I`) |

Para ver o editar las órdenes, abre el grupo **Dialecto de G-code** del perfil. Solo el tipo **G-code personalizado** deja editarlas; en los demás el dialecto es de consulta. Allí hay una vista previa del G-code que se generará.

![El perfil de una máquina con el tipo de controladora, la cama, la S máxima y el dialecto de G-code](../../apps/desktop/docs/screenshots/27-dialecto.png)

Campos del dialecto que conviene entender:

- **Escribir S en cada G1**: la potencia va dentro del movimiento (lo habitual). Si no, va en una línea aparte y la máquina frena en cada cambio.
- **El firmware tiene potencia dinámica (M4)**: sin ella, las capas dinámicas usan encendido constante.
- **G0 apaga el láser por sí solo**: márcalo solo si tu firmware lo hace. Si no, Tlaku apaga el rayo antes de cada desplazamiento.

> **Consejo:** si tu máquina no está en ninguna fila, elige **Otro / genérico** en el asistente, parte del tipo que más se parezca y ajusta `$30` (S máxima) con lo que lea tu máquina.

Siguiente: [Ruida y K40](13b-ruida-y-k40.md)
