# Controladoras soportadas

Qué firmware entiende Tlaku, en qué estado está cada uno y qué dialecto de G-code usa. Léela antes de conectar una máquina real.

> **Ojo, lo más importante:** Tlaku **nunca se ha probado con una máquina real**. Todo se validó con el emulador (`tlaku-emu`), con las máquinas virtuales de la app y con pruebas automáticas. Cuando esta página dice "estable en emulador" significa exactamente eso: el flujo funciona contra un láser simulado, no contra el tuyo. Haz tu primera prueba con el láser a potencia baja, material de desecho, sin dejar la máquina sola y con el paro de emergencia a mano.

## Estados que usa esta página

| Estado | Qué significa |
|---|---|
| **Estable en emulador** | Conecta, mueve, encuadra, envía, pausa, detiene y maneja alarmas contra un emulador. Falta probarlo con hardware. |
| **Por validar con máquina real** | Existe en la app o en el motor, pero con menos cobertura o sin máquina virtual propia. |
| **Experimental** | Existe solo en el motor o en la línea de comandos y está marcado así. No está en la app. |
| **No soportada** | No hay código. |

## Tabla de controladoras

| Familia | Tipo en el perfil | Estado | Notas |
|---|---|---|---|
| GRBL 1.1f o posterior (la mayoría de los diodos) | GRBL 1.1 | Estable en emulador | Usa `M4` (potencia dinámica) con `$32=1`, S de 0 a 1000. |
| grblHAL | grblHAL | Estable en emulador | Hay máquina virtual **grblHAL**. Ej.: Ortur Laser Master 3. |
| FluidNC | FluidNC | Estable en emulador | Hay máquina virtual **FluidNC**. Solo por USB; el Wi-Fi no existe en la app. |
| Marlin 2.x con láser | Marlin | Estable en emulador | Hay máquina virtual **Marlin**. Envío línea a línea, arranque lento al abrir el puerto. |
| Smoothieware | Smoothieware | Estable en emulador | Hay máquina virtual **Smoothieware**. S de 0 a 1 por defecto. |
| GRBL antiguo (1.1e o anterior, solo `M3`) | GRBL antiguo (sin M4) | Por validar con máquina real | Sin máquina virtual propia. Potencia constante; `G0` no apaga el rayo, así que Tlaku escribe `S0` en los desplazamientos. |
| GRBL-LPC (Cohesion3D) | GRBL-LPC / Cohesion3D | Por validar con máquina real | Sin máquina virtual propia en la lista de la app. S de 0 a 1. |
| G-code propio | G-code personalizado | Por validar con máquina real | Tú escribes el dialecto; parte del de GRBL 1.1. |
| Ruida (CO2 de caja con pantalla) | no es un tipo de perfil | **Experimental, solo CLI** | Ver más abajo. |
| K40 con placa M2 Nano | — | **No soportada** | Con la placa cambiada a GRBL-LPC o Smoothie sí funciona como esas familias. |
| Galvo (fibra, UV, CO2 galvo; EZCAD) | — | **No soportada** | |
| Trocen, TopWisdom | — | **No soportada** | |
| Máquinas de red (Wi-Fi, Ethernet) | — | **No soportada** en la app | La app solo conecta por puerto serie (USB) o máquina virtual. xTool por red tampoco. |

Las máquinas virtuales que ofrece la app son seis: **Falcon2 Pro**, **GRBL 1.1**, **grblHAL**, **FluidNC**, **Marlin** y **Smoothieware**. Ver [Conectar tu máquina](08-conectar-maquina.md).

## Ruida: experimental y solo por línea de comandos

Tlaku puede escribir archivos `.rd` y hablar con una Ruida por red desde el programa `tlaku`, pero:

- Está marcado **EXPERIMENTAL** en el propio programa: no se probó con ninguna máquina real.
- No aparece en el asistente **Agregar máquina** ni en el panel de la máquina.
- Los subcomandos son `tlaku ruida export-rd` (guardar el archivo), `tlaku ruida send` (enviar por UDP) y `tlaku ruida status` (leer el estado, solo lectura).

Si tienes una Ruida, lo más seguro hoy es seguir usando el software que ya te funciona para ella. Si pruebas `export-rd`, hazlo con el láser deshabilitado o a potencia 0.

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

Siguiente: [Venir desde LightBurn](14-desde-lightburn.md)
