# Supported controllers

Which firmware Tlaku understands, what state each one is in, and which G-code dialect it uses. Read it before connecting a real machine.

> **Warning, the most important thing:** Tlaku **has never been tested with a real machine**. Everything was validated with the emulator (`tlaku-emu`), with the app's virtual machines, and with automated tests. When this page says "stable in emulator" it means exactly that: the workflow works against a simulated laser, not against yours. Do your first test with the laser at low power, scrap material, without leaving the machine unattended and with the emergency stop within reach.

## States used on this page

| State | What it means |
|---|---|
| **Stable in emulator** | Connects, moves, frames, sends, pauses, stops, and handles alarms against an emulator. Still to be tested with hardware. |
| **To be validated with a real machine** | It exists in the app or in the engine, but with less coverage or without its own virtual machine. |
| **Experimental** | It is marked that way in the app or on the command line. It was tested only against an emulator or a simulated board and can be wrong with a real machine. |
| **Not supported** | There is no code. |

## Controller table

Two columns say honestly what was done: what was **tested in emulator** and what remains **to be validated with hardware**. In every row, the second is "everything": none has been tested with a real machine yet.

| Family | Type in the profile | Connection | State | Tested in emulator | To be validated with hardware |
|---|---|---|---|---|---|
| GRBL 1.1f or later (most diodes) | GRBL 1.1 | USB serial | Stable in emulator | Connect, move, frame, send, pause, stop, alarms. **GRBL 1.1** and **Falcon2 Pro** virtual machines | Everything. The first real test will be a Creality Falcon2 |
| grblHAL (for example, Ortur Laser Master 3) | grblHAL | USB serial; TCP network | Stable in emulator | Same; **grblHAL** virtual machine | Everything, including the network |
| FluidNC | FluidNC | USB serial; TCP or WebSocket network | Stable in emulator | Same; **FluidNC** virtual machine | Everything, including the network |
| Marlin 2.x with laser | Marlin | USB serial | Stable in emulator | Line-by-line sending and slow start; **Marlin** virtual machine | Everything |
| Smoothieware | Smoothieware | USB serial | Stable in emulator | S from 0 to 1 by default; **Smoothieware** virtual machine | Everything |
| Legacy GRBL (1.1e or earlier, `M3` only) | Legacy GRBL (no M4) | USB serial | To be validated with a real machine | Only the G-code (engine tests); no virtual machine of its own. Constant power; Tlaku writes `S0` on travel moves | Everything |
| GRBL-LPC (Cohesion3D) | GRBL-LPC / Cohesion3D | USB serial | To be validated with a real machine | Only the G-code; no virtual machine of its own in the list. S from 0 to 1 | Everything |
| Own G-code | Custom G-code | USB serial | To be validated with a real machine | You write the dialect; it starts from GRBL 1.1's | Everything |
| Ruida (boxed CO2 with a screen) | Ruida (CO2, network; experimental) | Network (UDP) | **Experimental** | Own UDP emulator: connect, probe, upload saving or running, pause, resume, stop, jumps, framing, information, and document list. **Ruida (CO2)** virtual machine | Everything. Addresses and units of the information, document numbering, home, pause states. See [Ruida and K40](13b-ruida-y-k40.md) |
| K40 with M2 Nano board | K40 with M2 Nano (experimental) | Direct USB | **Experimental** (against the emulator only): in the app and on the command line (`tlaku k40`) | Simulated board: CRC, packets, states, execution on a virtual clock; each board's speeds against 1511 vectors from another open-source project | Everything: real USB, the board, home, timings. See [Ruida and K40](13b-ruida-y-k40.md) |
| Galvo (fiber, UV, CO2 galvo; EZCAD2 JCZ/BJJCZ LMCV4 board) | — (driven from the Machine panel or with `tlaku galvo`) | Direct USB | **Experimental**, from the app and from the command line | Simulated board: compile, send, frame with the red pointer, mark, switch off on errors, stop, and injected USB failures | Everything: real USB, speed, frequency, power, and delay units, status bits, switching off on a dropped USB. Only the `9588:9899` board. See [Galvo](13c-galvo.md) and, **before connecting anything**, [Galvo safety](13d-galvo-seguridad.md) |
| Galvo EZCAD3 / DLC, BSL | — | — | **Not supported** | — | — |
| Trocen, TopWisdom | — | — | **Not supported** | — | — |
| xTool over proprietary Wi-Fi, Glowforge | — | — | **Not supported** | — | — |

A K40 whose board has been swapped for GRBL-LPC or Smoothie works as those families. xTool over USB is treated as GRBL (see below).

### Network connection

For GRBL, grblHAL, and FluidNC the app connects over **TCP / Telnet** or **WebSocket** in addition to the serial port, with "Test connection" and a virtual machine over the network. They are chosen in the profile; see [Network connection and automation](08b-red-y-automatizacion.md). Ruida uses UDP with the profile's IP. There is no encrypted connection (`wss://`) or machine discovery on the network.

### Machine functions: what was tested

| Function | Tested in emulator or with test data | To be validated with hardware |
|---|---|---|
| Macros, saved positions, moving Z and U, focus | GRBL virtual machines and automated tests | Everything |
| Test fire and framing with the laser on | Virtual machines (the G-code is checked, and that `M5` is always sent) | **Everything, and it is the most delicate: it turns the laser on** |
| `$$` settings with backup | Virtual machines; writing one at a time with a re-read | Everything; a critical setting set wrong makes the machine move wrongly |
| Laser 2 | Generated G-code, with the dialect's commands | Everything, with the firmware you use |
| Fisheye | Synthetic cameras | A real lens |
| Head-mounted camera and mosaic | Virtual machine and synthetic photos | A camera mounted on a head |
| Center finder (camera) | Synthetic images | A real camera |
| Center finder with the head (`G38` probe) | GRBL emulator with a simulated contact surface (`G38.2` to `G38.5`, `[PRB:...]`, `Pn:P`, `ALARM:4` and `ALARM:5`) and a virtual machine with a test disc | A real probe or touch plate: the pin, how far it coasts after contact, and the tip offset |
| Per-layer U axis | Generated G-code and engine tests | A real U axis: direction and distance |
| UDP automation | Protocol tests and `START` safety tests | — (it is local; it does not depend on the machine) |
| Print & Cut | Virtual machine and a simulated printed sheet (shifted and rotated by a few millimeters); the fit is tested with generated marks | A real laser and a real camera; the scale of a real printer |
| Tiling | Virtual machine and tests of the trimming (strokes, fills, and images), of the session, and of resuming | A real machine: the alignment of the joints depends on the material |
| Automatic feeder | Advance G-code sent to the virtual machine; with several columns, the guided pause and the registration marks | A real feeder: the direction and the distance of the advance. A DSP's built-in feeder (Ruida, Trocen) does not exist in Tlaku: there is no public documentation of the command |
| Galvo (`tlaku galvo`) | Own simulated board; it does not prove that a real board interprets the parameters the same way | **Everything, and it is the most delicate: it is a class 4 laser with an invisible beam.** See [Galvo safety](13d-galvo-seguridad.md) |

The app offers six virtual machines (**Falcon2 Pro**, **GRBL 1.1**, **grblHAL**, **FluidNC**, **Marlin**, and **Smoothieware**, each also **over the network**) plus **Ruida (CO2)** with a Ruida profile. See [Connecting your machine](08-conectar-maquina.md).

## Ruida and K40

Both are marked **experimental** and have their own page: what they do, what they do not do, why Ruida only saves the job by default, how to test them carefully, and why on the K40 **the power is set with the machine's knob**. See [Ruida and K40](13b-ruida-y-k40.md).

## Galvo

JCZ/BJJCZ LMCV4 galvo boards (fiber, UV, and CO2 galvo lasers with EZCAD2) have **experimental** support from the app (adding with verification without the laser, profile, framing with the red pointer, and marking) and from the command line (`tlaku galvo`). It was tested only against a simulated board and, since fiber is a class 4 laser with an invisible beam, it has two pages: [Galvo (experimental)](13c-galvo.md) and [Galvo safety](13d-galvo-seguridad.md), which you should read **before** connecting anything.

## Catalog brands

The **Add machine** wizard has a catalog of 21 entries: 18 specific models and 3 generic families (GRBL, Marlin, and Smoothie).

| Brand | Catalog models |
|---|---|
| Creality | Falcon2 22 W, Falcon2 40 W, Falcon2 Pro (22 / 40 W) |
| Sculpfun | S9, S30 / S30 Pro / S30 Pro Max |
| Atomstack | A5 Pro, A10 Pro, X20 Pro |
| Ortur | Laser Master 2 Pro, Laser Master 3 |
| xTool | D1, D1 Pro, S1 |
| Longer | Ray5 (5 / 10 / 20 W) |
| Neje | Master 2S / 2S Plus |
| Two Trees | TTS-10 Pro, TTS-20 Pro |
| K40 | K40 with Cohesion3D (GRBL-LPC) |

How to read the catalog honestly:

- A model being on the list does **not** mean it has been tested. It only means Tlaku knows its firmware and some data.
- Each value (bed, speed, max S, baud rate...) cites its source: **Official**, **Store or review**, or **Forum**. Store and forum values may differ from your unit.
- Whatever has no source is left blank and the wizard asks you to read it from the machine (`$$`).
- The Falcon2 is the only machine with in-depth research (its own `ERROR:NN` alarms are in [troubleshooting](15-solucion-de-problemas.md)). Even so, most of it comes from public reports on the Pro; nobody has connected one yet.
- xTool uses a GRBL with its own extensions: if the G-code fails, try the generic profile first. USB only.

## G-code dialects

The profile's **controller type** decides the starting dialect: how the laser is turned on, what range `S` has, whether there is `M4`, how it is turned off, and how air is sent. It is changed in **Manage devices**, "Controller type" field; when you change it, the app offers to apply the type's starting values or to change only the type.

| Type | Starting max S | Dynamic power |
|---|---|---|
| GRBL 1.1, grblHAL, FluidNC | 1000 | `M4` |
| GRBL-LPC / Cohesion3D | 1 | `M4` (same dialect as GRBL 1.1) |
| Legacy GRBL (no M4) | 255 | No, `M3` only |
| Smoothieware | 1 | No, constant on |
| Marlin | 255 | No, constant on with power on every `G1` (`M3 S0 I`) |

To see or edit the commands, open the profile's **G-code dialect** group. Only the **Custom G-code** type lets you edit them; with the others the dialect is view-only. There is a preview of the G-code that will be generated.

![A machine profile with the controller type, the bed, the max S, and the G-code dialect](../../../apps/desktop/docs/screenshots/27-dialecto.png)

Dialect fields worth understanding:

- **Write S on every G1**: the power goes inside the move (the usual way). Otherwise it goes on a separate line and the machine slows down at each change.
- **The firmware has dynamic power (M4)**: without it, dynamic layers use constant on.
- **G0 turns the laser off by itself**: tick it only if your firmware does this. Otherwise, Tlaku turns the beam off before every travel move.

> **Tip:** if your machine is not in any row, choose **Other / generic** in the wizard, start from the type that most resembles it, and adjust `$30` (max S) with what your machine reads.

Next: [Ruida and K40](13b-ruida-y-k40.md)
