# Connecting your machine

How to give Tlaku your laser's profile, open the USB port (or the network connection), and try everything first with a virtual machine. This page covers the connection; for networking and automation go to [Network connection and automation](08b-red-y-automatizacion.md), and for sending jobs go to [Sending jobs safely](09-enviar-con-seguridad.md).

> **Be aware, in all honesty:** Tlaku has been tested with **a single real machine**: a Creality Falcon2 Pro (22 W, GRBL 1.1) from macOS. On it, the **Add machine** wizard, port selection, homing, jogging, framing, sending jobs, and pausing all worked. It has **not** been tested with other brands or firmwares, over the network, or with Ruida, K40, or galvo; those connections were only validated with the `tlaku-emu` emulator and the app's virtual machines. Do your first test with scrap material, low power, and without leaving the machine unattended. Which controllers work and in what state: [Supported controllers](13-controladoras.md).

## Two concepts: profile and port

- The **profile** (the **Machine** list) describes your laser: bed size, origin, max S, speeds, G-code dialect. The engine uses it to calculate and generate the G-code.
- The **port** (the **Port** list) is how the app talks to the board over USB. With a machine that connects **over the network** (TCP, WebSocket, or Ruida over UDP) you do not pick a port: the address is part of the profile, and the **Connection** section shows it in place of the port list and the baud rate ([Network connection and automation](08b-red-y-automatizacion.md)). There are also virtual machines.

All of this lives in the **Machine** tab (`Ctrl/Cmd+2`), **Connection** section.

## Option 1: the Add machine wizard

The easiest way to create a profile. Open it from the **Machine** menu, **Add machine…** (it is not available while a job is running). It has four steps: **Detect**, **Model**, **Review**, and **Save**.

![First step of the wizard: port, baud rate, and the Detect button](../../../apps/desktop/docs/screenshots/26-agregar-maquina.png)

1. **Detect.** Choose the **Port** and the **Baud rate** (115200 for almost all GRBL machines; Marlin often uses 250000) and press **Detect**. Detection only listens for the board's greeting and asks for its version and settings (`$I`, `$$`, `M115`). It never moves the machine or changes its configuration. Opening the port may reset the board and takes a few seconds. If you do not have the machine at hand, choose **Skip detection and pick from the catalog**.
2. **Model.** Search the catalog for your brand or model (for example `falcon`, `S9`, `xTool`). If detection recognized the family, only models from that family are listed; tick **Also show models from other firmware families** to see them all. If your machine is not listed, choose **Other / generic** and the firmware type.
3. **Review.** Tlaku compares what the catalog says with what the machine answered to `$$` (max S, bed, max speed, and acceleration). Where they differ, you choose with **Use the machine's value**; by default the machine's value is suggested. With the Falcon2 Pro, for example, the wizard recognized the model and suggested the values the machine reports where they differ from the catalog (such as the Y-axis speed, 8000). This step also warns you if laser mode is off (`$32=0`) or if GRBL is older than 1.1f.
4. **Save.** Give it a **Machine name** and press **Save and activate**. The profile becomes the active machine.

The catalog has 21 entries: 18 models (Creality Falcon2, Sculpfun, Atomstack, Ortur, xTool, Longer, Neje, Two Trees, K40 with Cohesion3D) and 3 generic families (GRBL, Marlin, Smoothieware). Each value says where it comes from (**Official**, **Store or review**, or **Forum**), and values that no source provides are left blank so you can read them from your machine. The advertised figures can differ on your unit: trust what the machine reports.

> **Tip:** network machines (Ruida and similar) are not detected: Tlaku does not scan the network for machines. For a Ruida (experimental), choose the **Ruida** type under "Other / generic" and type its IP ([Ruida and K40](13b-ruida-y-k40.md)). For GRBL over the network, create the profile and choose the connection type in **Manage Devices…**. The K40 with an M2 Nano board is experimental (tested only against the emulator) and goes over direct USB, not a serial port: choose **K40 with M2 Nano** (in the wizard, which also looks for the board over USB without sending it anything) and first close K40 Whisperer, MeerK40t, and LaserDRW; see [Ruida and K40](13b-ruida-y-k40.md). Galvo and fiber are not supported.

## Option 2: manual profile

To create or adjust a profile by hand: **Machine** menu, **Manage Devices…** (or the settings icon next to the **Machine** list). **Devices** opens.

- Your profiles are on the left. **New**, **Duplicate**, and **Delete** manage them; the active one says "in use". You cannot delete the connected machine.
- On the right you edit the profile in groups. Nothing is saved until you press **Save changes** (**Revert changes** discards).
- **Use this machine** makes it the active one.

| Setting | What it is for |
|---|---|
| **Controller type** | Defines how G-code is written and the starting values (max S, power mode, baud rate). When you change it, Tlaku shows what would be overwritten and you can choose **Apply the type's values** or **Change the type only** |
| **Connection type** (**Communication** group) | **Serial port (USB)** by default, **Network · TCP / Telnet**, or **Network · WebSocket**, with the **Test connection** button. See [Network connection and automation](08b-red-y-automatizacion.md) |
| **Machine origin** | The corner where X0 Y0 is. If it is wrong, the job comes out mirrored or flipped |
| **Work area** | Bed width and height in mm ($130 and $131). The engine rejects anything that does not fit |
| **Max S** and **Power mode** | Max S must match `$30`. **Dynamic (M4, recommended)** requires GRBL 1.1f or later with `$32=1`; **Constant (M3)** is for old firmware |
| **Max speed**, **Acceleration** | Per axis. Used to estimate times and calculate automatic overscan |
| **Baud rate**, **Receive buffer** | Must match the board (127 bytes on GRBL 1.1). If the machine stalls, lower the buffer |
| **Start G-code** and **End G-code** | Lines emitted at the start and end of every job. It is best to leave `M5` at the end |
| **Z axis**, **Rotary** | See [Rotary and Z axis](12-rotativo-y-eje-z.md) |
| **U axis**, **Second laser** | Enable moving the U axis and choosing laser 2 per layer. See [Advanced machine control](09b-control-avanzado.md) |
| **Macros** | Buttons with G-code stored in the profile. See [Advanced machine control](09b-control-avanzado.md) |

The controller types offered in the list are: GRBL 1.1, grblHAL, FluidNC, GRBL-LPC / Cohesion3D, Legacy GRBL (no M4), Smoothieware, Marlin, Custom G-code, and Ruida (CO2, network; experimental).

![A machine profile with its G-code dialect and its preview](../../../apps/desktop/docs/screenshots/27-dialecto.png)

The **G-code dialect** shows a summary (for example "S 0-1000, dynamic M4, S on every G1, air M8/M9") and a preview of the G-code. It can only be edited with the **Custom G-code** type; with the others it is view-only. Among its settings is the **Dwell time (G4)**: the unit in which the pauses from the cut settings are written (**Pause at start**, **Pause at end**, **Cut-through**, and **Dot mode**, see [Layers and cut settings](05-capas-y-ajustes.md)). GRBL reads `G4 P` in seconds; Marlin and Smoothieware (in RepRap mode) read it in milliseconds, and Tlaku already picks the right unit for each controller type. A wrong unit makes the pause a thousand times longer or shorter, so change it only if your firmware is unusual.

**Read from machine** (with the machine connected) sends `$$`, which only reads, and suggests values for the profile. You tick which ones to apply; they are not saved either until **Save changes**.

### Importing a LightBurn profile (`.lbdev`)

If your machine is already set up in LightBurn, or the manufacturer publishes a `.lbdev` profile (for example, on their forum), you do not have to type everything again: in **Devices**, below the list, press **Import a LightBurn profile (.lbdev)…** and choose the file.

- A summary opens with the profiles that will be created and a list of **notes**. Nothing is saved until you press **Create N profile(s)**. Profiles are always **new** (with their own id; if the name already exists, “(2)” is added): your current profiles are not touched. Importing never sends anything to the machine.
- A file can hold several machines; one profile is created for each.
- What is read: the name, the work area, the origin corner (inferred from the file's X and Y mirror flags: confirm it with a low-power frame), the controller type (GRBL, GRBL-M3, GRBL-LPC, Smoothieware, Marlin, G-code and Ruida), the baud rate, the max S, the Z axis, air through `M7`, homing on connect, synchronous streaming, and the start and end G-code (review them: they are sent on every job).
- **Speeds and accelerations:** the file stores the ones from LightBurn's *time simulator* (in mm/s; Tlaku converts them to mm/min), not the firmware's. They are a starting point: with the machine connected press **Read from machine**, which reads `$$` and proposes the real values (`$110/$111`, `$120/$121`, `$130/$131`, `$30`) so you decide which to apply. Tlaku does not do this by itself on connect.
- Anything without an equivalent (for example laser 2 or process offsets, finish position, pen, simulator settings) shows up in the notes as “not imported”. Local user data (port, last folders, camera) is ignored. The direction of the Z axis is not imported: test it with a short move.
- Controllers Tlaku does not support (for example Trocen or TopWisdom) are **not imported**: you get a note and that machine is skipped, instead of inventing a G-code profile that does not belong to it. For a Ruida, the file does not carry (as far as we know) the IP address: enter it in the profile before connecting.
- Only `.lbdev` files are read. LightBurn's `.lbvendor` and `.lbzip` bundles are not imported yet (a `.lbzip` is a zip that contains `.lbdev` files: you can extract it and use that file).

## Profile settings worth knowing

Besides the values in the table, the profile stores settings that almost nobody touches at first but that solve very specific problems. They are all in **Devices**, and they all wait for **Save changes**.

### In the General group: default profile, order, and icon

- **Default profile.** Tick **Activate this profile when Tlaku opens** on the one you use almost always: when the app starts it becomes the active machine, and the list shows the **Default** badge. **Only one can be the default**: ticking another clears the previous one. A project saved with another machine still opens with its own.
- **Order.** **Move up in the list** and **Move down in the list** (next to the profile list) sort your profiles: put the one you use most at the top. With a single profile they do nothing.
- **Icon.** Pick one of ten (**Controller**, **Bolt**, **Flame (CO2)**, **Sun (fiber or galvo)**, **Scissors (cutting)**, **Printer**, **Wrench**, **Box**, **Layers**, **Gauge**) to recognize the profile at a glance in the list.

### In the Bed group

- **Crossed axes: Swap X and Y on the machine.** For machines whose X axis runs along the design's **height** and whose Y axis runs along its **width**. The **Work area** above stays in the document's orientation; the G-code, the framing, and manual movement (the arrows in **Move the head**) swap the axes for you. It cannot be combined with the rotary.
- **Red pointer offset** (**Pointer X** and **Pointer Y**, in mm; X to the right, Y up). Where the red pointer lands relative to the beam. With the offset set, **framing with the laser off** and **Position the laser** with the **Bring the red pointer to the point** checkbox make the **pointer**, not the beam, trace the job or reach the point. Measure it on test material: put the pointer on a point, mark there at low power, and measure the distance between the mark and the point. 0 and 0 means the pointer lands on the beam. See [Sending jobs safely](09-enviar-con-seguridad.md).
- **Speed unit: mm/min or mm/s.** This only changes **how speeds are shown and typed** in the profile (each axis's max speed and the rapid-move speed) and in the **Start** dialog. Internally, and in the G-code, everything stays in mm/min; layer speeds are still typed in mm/min. Useful if your laser or manufacturer gives speeds in mm/s (60 mm/s is 3600 mm/min).

### In the Communication group (GRBL-family controllers only)

- **Homing on connect: Offer homing ($H) when connecting.** With limit switches, the machine usually loses its position when powered off. When ticked, connecting opens the homing confirmation (clear the area, home now?). **It is never sent on its own.**
- **Do not reset the board on open: Open the serial port without raising DTR** (USB serial only). Many Arduino boards reset when a program opens the port, and lose their position and state as a result: if yours does this every time you connect, tick this so it keeps its position. It depends on the system: on Linux the driver always raises DTR, and some USB adapters do too, so sometimes it will have no effect.
- **Synchronous streaming: Send one line and wait for its "ok".** Tlaku sends G-code by filling the board's buffer (character counting), which is the fastest way. Some boards **lose data with a full buffer**: lines get dropped, stray `error:` messages appear, or moves do not match. Synchronous streaming sends **one line and waits for its reply before sending the next**: it is slower and motion may **stutter on fine strokes**. Use it only if your board loses lines even after lowering the **Receive buffer**; otherwise leave it off.

### In the Axes and accessories group

- **Job U offset** (with **The machine has a U axis** ticked). How far the U axis moves **at the start of each job**; at the end it is undone (with relative moves and the laser off). It is useful, for example, to advance a table or a belt between jobs without writing a macro. 0 leaves U untouched.

## Connect

1. In the **Machine** tab, choose your profile in **Machine**.
2. Choose the **Port** (if the profile is a network one, you see the **Address** instead and this step is not needed). If the list is empty, plug in the USB cable, install the board's driver (CH340 or FTDI), and press the scan-for-ports button again.
3. Choose the **Baud rate** (the profile already has its own; there is none over the network).
4. Press **Connect**. Over USB it takes about 2 seconds because the board resets; over the network it connects faster.

If all goes well you see "Connected at ..." and the firmware version (for example `Grbl 1.1h`). While you are connected you cannot change the profile, port, or baud rate: press **Disconnect** first.

Warnings the section can give you:

- Firmware older than GRBL 1.1f: it does not support laser mode (M4) well; use constant power (M3) in the profile.
- Laser mode disabled (`$32=0`): engraving would come out wrong. The **Enable laser mode ($32=1)…** button asks for confirmation before sending it.
- If the connection is lost (cable unplugged, port gone, machine switched off), the app tells you.
- If **another program** (for example LightBurn) already has the port open, Tlaku cannot connect and tells you so: a USB port can only be used by one program at a time. Close it and connect again.
- **The lid is not reported.** The Falcon2 Pro board does not report whether the lid is open (with the lid open it still says "Ready"), so Tlaku cannot know and cannot warn you. Check yourself that it is closed before starting.
- If the board **stops answering** for a few seconds (some USB boards do this, such as the Falcon2 behind a hub), Tlaku **does not disconnect**: it waits, just like LightBurn. After 2 seconds a discreet notice appears, "The machine has not answered for N s…", with **Stop** and **Disconnect**; it goes away by itself when the board answers again and the job carries on without you doing anything. If more than a minute passes in the middle of a job, the notice becomes more serious ("It has been silent for over a minute"): check the machine and, if it keeps moving and does not respond, use the emergency stop.

If the port is not found or the connection fails, see [Troubleshooting](15-solucion-de-problemas.md).

Once connected, the **Machine** tab also offers **Job** (framing, preview, start, pause, stop, **Live view**, and progress), **Jog head** (with **Position the laser** by a click on the canvas), **Saved positions**, the **Macros**, the **Live adjustments** for speed and power during a job, **Finish position**, the Z and U axes, **Test fire**, and **Movement settings** (the steps). All of that is explained in [Sending jobs safely](09-enviar-con-seguridad.md) and in [Advanced machine control](09b-control-avanzado.md). Live adjustments exist only with GRBL-family controllers (not with Marlin or Smoothieware).

### The Machine panel: sections that fold

So that the panel does not get very long, every block of the **Machine** tab is a **collapsible section**: the title is a button with an arrow (`›` closed, `⌄` open) that opens and closes with the mouse or with `Enter` / `Space` when focused. Each section remembers how you left it, also after restarting the app.

- **Open by default:** **Job** (at the top, the most used while engraving) and **Jog head**. The order is: Connection, Status, Job, Jog head and then **Saved positions**, **Macros**, **Live adjustments**, **Extra axes**, **Finish position**, **Test fire**, **Movement settings**, **Machine settings**, and **Virtual machine**, all closed. With a Ruida, K40, or galvo profile, its own panel appears open right below **Jog head**.
- **Inside Job** there are three closed sections: **Start from**, **What gets sent**, and **Frame options**. When closed they show one line with what is chosen (for example "Absolute coordinates" or "The whole design"); if the job does not fit the bed or the origin is missing, that line warns you in red. Safety warnings (alarm, laser mode off, old firmware) are always visible, even when the section is closed.
- **Connection.** With no machine connected it is shown in full, as always: it is the first thing you do. With the machine connected it shrinks to one line ("Falcon2 Pro · Connected on /dev/…" with the **Disconnect** button); press the **Connection** title to see the detail (machine, port, and baud rate).
- **Status** is a strip: the state (for example **Running**), the work position X/Y and, if the machine reports them, the feed **F** and power **S**. The **Machine position** is in a foldable detail.
- When something asks you to go to a section (for example the **Go to Live view** command in the command search), the section opens, is brought into view inside the panel, and focus moves to its title.
- **Reset the panel** puts every section back to its default state. It is in **Preferences › Appearance › Window layout**, in the panel's **Customize tabs** button, and in the command search.

The **Camera** tab uses the same system: **Camera**, **Canvas backdrop**, **Bed image**, **Bed alignment**, and **Head-mounted camera** stay open; the rest (lens, marks, material height, adjustment, etc.) is closed. **Simple mode** does not change.

## Virtual machine

At the end of the **Port** list there are six virtual machines, which are an emulator inside the app: **Falcon2 Pro**, **GRBL 1.1**, **grblHAL**, **FluidNC**, **Marlin**, and **Smoothieware**. They let you learn the whole workflow without risk: there is no laser and no hardware, and nothing is engraved. Each one also exists as a **Virtual machine over the network**, which listens on a local TCP port and to which the app really connects through a socket, so you can see the network connection without having the machine; and with a Ruida profile there is a **Ruida (CO2)** ([Network connection and automation](08b-red-y-automatizacion.md), [Ruida and K40](13b-ruida-y-k40.md)).

![Connected to the Falcon2 Pro virtual machine, with the section for simulating faults](../../../apps/desktop/docs/screenshots/25-maquina-virtual.png)

- They start homed and with no alarm, ready to work.
- **Time speed** (`×1`, `×5`, `×20`) speeds up the simulated job; at `×1` it lasts the same as a real one.
- The **VIRTUAL** badge appears in the **Machine** panel and in the status bar.
- The **Virtual machine** section has **Simulate fault**: **Flame detected (ERROR:05)**, **Abnormal tilt (ERROR:01)**, **Alarm 3 (reset while moving)**, and **Cable unplugged**. Choose one and press **Trigger** to see how the alarm banner, the console, and the job react. That section does not exist with a real machine and starts closed: press its title to open it.

Next: [Network connection and automation](08b-red-y-automatizacion.md)
