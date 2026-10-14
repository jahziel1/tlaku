# Ruida and K40 (experimental)

Two families of machines that LightBurn treats differently and that Tlaku supports **experimentally**: the **Ruida** controllers of CO2 lasers with a screen, and the **K40 with an M2 Nano board**. This page says what each one does, what it does not do, and how to test them carefully.

> **Warning, in all honesty:** neither has been tested with a real machine. Ruida was validated against an emulator of our own over UDP, and the K40 against a simulated board and test vectors from another open-source project. When this page says "works" it means exactly that: it works against the emulator. **A wrong command to a CO2 controller can damage a laser tube or hurt whoever is nearby.** Do the first test with the laser disabled, without valuable material, with the emergency stop within reach, and without leaving the machine unattended.

## Ruida

### What it is and what it does

Ruida is the controller of most metal-cased CO2 lasers with a screen. Tlaku talks to it **over the network (UDP)** from the app and from the `tlaku` program. In the app you can:

- **Connect** with the machine's IP. Connecting performs a read-only query (model, board, *magic*, bed).
- **Send a job.** Tlaku calculates the job, converts it to Ruida's `.rd` format, and uploads it with a progress bar.
- **Pause, resume, and stop.**
- **Move the head** with the laser off and **frame** (always with the laser off).
- **Read information** from the controller (speeds, acceleration, power-on time, jobs done, distance traveled) and the **list of documents** saved in its memory.
- Rehearse everything with the **Ruida (CO2) virtual machine**, without risk.

### What it does not do

- **It does not write controller settings.** The information and the document list are read-only. Writing the controller's memory without a test machine can leave it unusable, so it was deliberately left pending.
- **It does not delete or run documents** from its memory (do it from the machine's panel). With a wrong index it would delete or run a different document.
- **It does not use G-code:** the console, live speed and power adjustments, `$` settings, macros, and test fire do not apply (they are hidden, with the reason). Speed and power go inside the job file.
- **It does not frame with the laser on.**
- **It does not configure the rotary.** On DSP controllers the rotary is turned on in the machine's own settings; Tlaku sends Y in surface millimeters, without scaling or mirroring it, and says so in the log.
- **It does not support laser 2 with Ruida** and it does not connect to Trocen or TopWisdom either. `.rd` files **do open** as a design (see [Import and export](03b-importar-y-exportar.md)), but only to recover the geometry and the settings of each layer.

### Preparing the profile

1. **Machine > Add machine…**, **Model** step, choose **Other / generic** and the type **Ruida (CO2, network; experimental)** (or change the type of a profile in **Manage Devices…**). Type the IP if you have it; you can leave it empty and fill it in later (without an IP only the virtual machine works). Tlaku **does not search** for Ruida on the network.
2. In the profile, **Ruida controller (network)** group:
   - **Controller IP**, for example `192.168.1.100`. Your computer must be on the same /24 network and **no other laser program** (LightBurn, another Tlaku) can have UDP port 40200 open.
   - **Magic (obfuscation key):** **Automatic** tries `0x88` and, if there is no answer, the other known ones with a read-only query; or set it **Manual** if you know it (`0x88` on almost all RDC644x, `0x11` on the 634XG, `0x38` on the RDL9635).
   - **Run after upload:** leave it **off** (see below).
3. In **Origin** choose the corner where the machine homes (most commonly, top left). It is an assumption until tested with hardware: first make a small move with the laser off.
4. Bed: the wizard's starting values (600 × 400 mm) are just that; confirm them with **Read information** when you connect. **If the profile's bed does not match the one the machine reports, Tlaku sends no job** until you fix it, because the file's origin depends on it.

### More than 128 layers and high speeds

The `.rd` format supports at most **128 layers**, and the protocol's speed limit is **36,000 mm/min**. Tlaku warns you **when it calculates**, before you try to send:

- **More than 128 layers.** A design with many colors (for example, an SVG with 129 colors) generates one cut layer per color block. If there are more than 128, the warning says: *Ruida supports 128 layers and the job has N: on export, M neighboring layers with identical settings (speed, power, air) will be merged, leaving K; the cut order does not change*. Only **consecutive** layers with **exactly the same settings** are merged (color is just a label); the cut order does not change. If after merging there are still more than 128, the warning says that it **cannot be exported or sent** and what to do: unify colors or settings (speed, power, air) of consecutive layers.
- **36,000 mm/min cap.** If a layer asks for more (for example, 48,000), the `.rd` file uses **36,000 mm/min** and the warning says so (*layer …: asks for 48000 mm/min and Ruida supports at most 36000 mm/min; the .rd file will use 36000 mm/min, so the result will be darker or more burned than expected*). To avoid depending on the clamp, set a speed within the cap on the layer.
- **Layer faster than the machine.** If a layer's speed exceeds the profile's **Max speed**, a separate warning appears. With Ruida that profile maximum is an **assumption** (the controller has its own): if your machine supports more, raise the value in the profile.

These warnings do not change what is sent, except for the clamp to 36,000 mm/min.

### Why it only saves by default

With **Run after upload** off (the default and the recommended setting), the job is **saved as a document** in the controller's memory (with a name like `TLAKU123`) and **does not start**: you start it yourself, from the machine's panel, with your hand on the stop button. The **Start** dialog says "The job will be saved as a document in the controller's memory and will NOT start".

It is like this because a job that starts as soon as it arrives can fire the laser before you are looking, with a bed or an origin wrongly assumed, and nobody has yet tested this route on a real machine. With **Run after upload** on, the job starts running when it arrives and the dialog warns "the laser may fire"; if the upload fails halfway, Tlaku sends *Stop* and tells you.

### First test, step by step

1. **Virtual machine.** Create the Ruida profile and connect to **Ruida (CO2)** (at the end of the **Port** list). Send a job, watch the progress bar, the **Ruida controller** panel, and the document list. It is the same workflow as with the machine.
2. **Real connection, sending nothing.** With the laser disabled (or the tube without power, depending on your machine), close other programs, enter the IP, connect, and press **Read information**. Check that the model and the bed match your machine.
3. **Movement without laser.** Move the head with the pad a few millimeters and check the direction and the *home*. Try **Frame**.
4. **Save a small job** (a 20 mm square) with **Run after upload** off. In **Documents in memory**, **Read list**: it should appear.
5. **Run it from the machine's panel** with the laser disabled or at power 0, and check that the path is the expected one.
6. Only then, low power on scrap material.

From the command line, `tlaku ruida export-rd` (to a `.rd` file, offline), `tlaku ruida send` (over UDP, saves by default; `--run` runs), and `tlaku ruida status` (read-only) still exist.

## K40 with an M2 Nano board

### What it is

The **K40** is the cheap Chinese 40 W CO2 laser with an **M2 Nano** (Lihuiyu) board. **LightBurn does not support it**; Tlaku does, experimentally, by speaking the board's protocol (LHYMICRO-GL) over USB. If your K40 has its board swapped for GRBL-LPC (Cohesion3D, Mini Gerbil) or Smoothie, you do **not** need this: it works as those families.

**Status: experimental, tested only against the emulator (no real board has been tested).** It is available **in the app** (machine type "K40 with M2 Nano (experimental)") and also from the **command line** (`tlaku k40`).

### K40 in the app

- **Direct USB connection** (CH341 chip, `1A86:5512`), **not** through a serial port. First close K40 Whisperer, MeerK40t, and LaserDRW: only one program at a time can have the board. On Linux a udev rule is needed and on Windows the WinUSB driver (see "USB permissions"). On macOS the app carries the USB entitlement, so nothing needs to be installed.
- **Adding the machine:** the "Add machine" wizard looks for boards over USB without sending them anything and offers "K40 with M2 Nano". The board model is the M2 unless yours has another letter printed on it. If afterward you **change the profile** (board, bed, origin, or max speed), you **must reconnect** for it to apply.
- **Ports:** each USB board appears as `usb:k40:<n>` (the list shows "K40 · USB board n", starting from 1) and the **virtual machine** as `virtual:k40`: the simulated board, to practice without hardware.
- **Own home:** the K40 does not report its position. Tlaku counts it from *home* (the `IPP` command, the limit switches). Use "Home the K40…" in the Machine panel. If you send a job **with no known position, the app homes first**.
- **The position is lost on stop:** if you stop or a job fails, the count is lost and you have to home again.
- **If stop is not confirmed:** the app highlights an **emergency stop** notice ("The board did not confirm the stop"): the head may still be moving or the laser on. Use the machine's emergency stop or open the lid.
- **Power:** it is set with the machine's **knob**. The start dialog warns about this **before sending**, along with the encoder's other warnings (air, Z, skipped pauses). **Grays are binarized**.
- **What does not exist on the K40:** Z axis, U axis, air control, console, live adjustments (overrides), `$` settings, and test fire. Framing is always done with the laser off.
- **Moshiboard:** that board shares the M2 Nano's USB but is **not compatible**; if the app detects it on connect, it will say so.

### Most important: power is set with the knob

> **Warning:** the M2 Nano **does not control power by software**. The layers' power **is ignored** (Tlaku warns you for each layer: "the 20 % power is ignored; the M2 Nano does not control it by software"). The real power is set by **the knob (potentiometer) or the panel of your machine**. Before any test, **turn the knob down to the minimum** and raise it little by little.

Other things the board does not do and Tlaku warns about: it does not control **air assist**, it has no **Z axis**, it does no **pauses or in-place pulses** (they are skipped), and **grays** are engraved in black and white (any power greater than 0 burns): to simulate grays use dithering. And the board **does not report its position**: Tlaku counts it from *home*, which is why it starts by taking the head to *home* (the limit switches).

### Commands

With the board connected over USB (nothing to install on macOS; see below for Linux and Windows):

| Command | What it does |
|---|---|
| `tlaku k40 list` | Lists the connected CH341 boards (USB `1A86:5512`) |
| `tlaku k40 status` | **Read-only**: chip model and status code. It moves nothing and does not turn the laser on. It warns if the board is a Moshiboard (another board that shares USB and that Tlaku does not support). `--repeat 20 --interval-ms 500` repeats the query to see how the status changes when you use the machine's buttons |
| `tlaku k40 send design.svg` | Shows what it is going to do, **asks for confirmation**, takes the head to *home*, and engraves. `Ctrl+C` stops the machine and turns the laser off |
| `tlaku k40 send design.svg --dry-run` | Prepares and reviews the job **without connecting** to the machine |
| `tlaku k40 send design.svg --emu` | Everything against a **simulated board**, without USB |
| `tlaku k40 export-egv design.svg output.egv` | Writes the job as an `.egv` file without connecting |

Useful options: `--bed 300x200` (the bed; 300 × 200 mm by default), `--home top-left` (the corner of the limit switches: `top-left`, `top-right`, `bottom-left`, `bottom-right`), `--board M2` (the board: A, B, B1, B2, M, M1, M2 —the standard one— or M3), `--max-speed 140` (mm/s; a faster layer is an error), `--no-home` (if the head is already at *home*), `--device N` (if there are several boards), and `--yes` (do not ask for confirmation; do not use it in your first tests). Layer speeds are written in mm/min as always: `--layer "#ff0000:speed=1000"`.

### Test plan

1. **Without hardware:** `tlaku k40 send design.svg --emu --bed 300x200 --yes`. Read the summary, the board's warnings, and the end: "The board confirmed the end of the job".
2. **Machine preparation:** water circulating through the tube **before** any test with the laser on, lid closed, safety glasses suitable for CO2, **power knob at minimum**, and the emergency stop within reach. If your machine has a switch to disable the laser, leave it off in the first tests.
3. **Detect:** connect the USB and run `tlaku k40 list`. A `1A86:5512` board should appear. If it does not, see [Troubleshooting](15-solucion-de-problemas.md).
4. **Read the status:** `tlaku k40 status`. It should say "Status: ready". With `--repeat`, press the machine's buttons and watch the code change.
5. **Movement with the laser disabled:** send a 20 mm square (`tlaku k40 send square.svg`). Confirm the *home*, that the head goes up to the right corner (if it goes the other way, fix `--home`), and that the square is the right size.
6. **Laser at minimum power** on scrap cardboard or paper, raising the knob little by little. Measure a 100 mm square and compare it with the design.
7. **Report what happened:** copy the command's output and use **Help > Report a Problem…**. A real result, good or bad, is the most valuable thing you can contribute.

With the app, the plan is the same; steps 3 to 6 are done from the "K40 with M2 Nano" profile: choose the `usb:k40:<n>` board in the Machine panel, connect, home, and read the start dialog's warning before sending. For step 1 use the `virtual:k40` virtual machine.

### USB permissions

- **Linux:** a udev rule is needed for your user to open the board. Create `/etc/udev/rules.d/99-k40.rules` with one line:

  ```
  SUBSYSTEM=="usb", ATTR{idVendor}=="1a86", ATTR{idProduct}=="5512", MODE="0666"
  ```

  and run `sudo udevadm control --reload-rules && sudo udevadm trigger`; unplug and plug the board back in. (`MODE="0666"` leaves the board open to all users on the computer; if you prefer to restrict it, replace `MODE="0666"` with `TAG+="uaccess"`.) Tlaku detaches the kernel driver by itself if needed.
- **macOS:** it needs no driver. If you have the chip manufacturer's (WCH) driver installed, remove it.
- **Windows:** the board must be associated with the **WinUSB** driver (for example with the Zadig tool). With WCH's original driver it cannot be opened.

First close any other program that uses the board (K40 Whisperer, MeerK40t, LaserDRW): only one at a time can have it open.

Next: [Galvo (experimental)](13c-galvo.md)
