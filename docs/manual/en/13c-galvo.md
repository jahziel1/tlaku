# Galvo (experimental)

A **galvo** laser steers the beam with two rotating mirrors instead of moving a head: it is what **fiber**, **UV**, and **CO2 galvo** markers use to engrave metals, plastics, and glass at high speed. It is controlled by a board (the most common is the **LMCV4-FIBER-M**, which is driven by the EZCAD2 program) connected over USB.

> **Read [Galvo safety](13d-galvo-seguridad.md) first.** A fiber laser is **class 4** and its beam is **invisible**: it can permanently damage eyesight, even from a diffuse reflection, and burn whatever is behind. This page explains what Tlaku does; that one explains how not to get hurt.

> **Status: experimental and untested with a real board.** All galvo support was validated against a **simulated board** that we wrote ourselves: that proves our code is consistent with what we *believe* the protocol says, not that a real board interprets it the same way. The units for speed, frequency, power, and delays come from the code of other open-source projects and are **not verified with hardware**.

> **Where it is driven from:** the galvo is driven **from the app** (this page, in "The galvo in the app") and **from the command line** (`tlaku galvo`, below). Both use the same engine and the same safety rules. (The command line uses the language set by `TLAKU_LANG` or `LANG`, Spanish if neither is set; the messages quoted here are translated into English.)

## What it can do today

- **List** the connected boards and say whether they are compatible, without opening anything (`tlaku galvo list`).
- **Read the board's status**, without moving or turning anything on (`tlaku galvo status`).
- **Frame with the red pointer**: draws the design's rectangle (or its outline) in a loop, **without ever enabling the laser** (`tlaku galvo light`).
- **Mark** the design, or trace all of it with the laser disabled and the red pointer (`tlaku galvo mark`, with `--red-only`).
- **Compile and review** the job without connecting to anything (`--dry-run`), dump the command list to a file (`--dump`), and test everything against the simulated board (`--emu`).

There are also: per-layer parameters (frequency, pulse width, delays, jumps, repeats), wobble, [3D slice engraving and cylinder and cone correction](12b-grabado-3d-y-cilindro.md), and the sequence for split, repeated, and rotary marking.

## Which boards are accepted

Only **`9588:9899`** ("BJJCZ USBLMCV4", EZCAD2's LMCV4-FIBER-M). Any other USB identifier is **rejected**, with the reason: the clones `9588:9980`, `9588:9990`, and `9588:9999` (it is not known whether they use the same protocol) and the board without firmware `9588:9880` (it needs the FPGA firmware loaded, and Tlaku does not write to the board). **EZCAD3 / DLC** and **BSL** boards are out of scope: there is no public documentation of their protocol.

To see which ones there are and whether they qualify:

```
tlaku galvo list
```

With no boards connected it says "No BJJCZ devices (USB 9588:xxxx) connected".

## The galvo in the app

![The Machine panel with a simulated galvo board: the convex hull frame, the loop counter, and the live adjustment](../../../apps/desktop/docs/screenshots/70-galvo-encuadre.png)

### Adding the machine, with verification without the laser

**Machine > Add machine…** offers **Galvo (fiber or UV, EZCAD2) over USB**, which looks for BJJCZ boards over USB (it does not use a serial port). If there are none, you can add the machine anyway and test with the **simulated board**. Adding is a four-step wizard:

1. **Lens and source.** Choose the source (**Q-switched fiber**, **MOPA fiber**, **UV**, or **CO2 galvo**). An F-theta lens is made for one wavelength: a lens for another wavelength focuses badly and can be damaged by the source's power.
2. **Field.** The side of your lens's field (110, 150, 200, 300 mm, or **Other size**). It is used as the machine's bed.
3. **Board.** The board to verify, or **Simulated board (no hardware)**. Only the LMCV4-FIBER-M (USB `9588:9899`) is accepted.
4. **Verification without the laser.** **Verify without the laser** reads the board (serial number, version, status, and inputs) and checks four things: **Board recognised**, **Board idle**, **Field frame** (that the profile compiles), and **Input interlock**. **It does not move the mirrors, and does not turn on the pointer or the source.** With a real USB board it first asks for the safety confirmation (glasses, enclosure, emergency stop); if you do not give it, the board is not opened. If something fails, it says so before you connect.

The frequencies, pulse widths, delays, and correction that the profile comes with are **starting values**: complete them from your source's manual. A **MOPA** source needs the list of pulse widths in the profile before connecting.

### The galvo profile

In **Manage Devices…**, a galvo profile has its own **Galvo (JCZ/BJJCZ, EZCAD2)** group and hides what does not apply (G-code, max S, baud rate, macros, acceleration, Z and U axes). The "bed" is the lens's field, with the origin at the lower left (it is the only valid origin). In it are:

- **Lens and source**: field, source, source power (informational only, shown in the safety notice), **maximum allowed power** (a layer that exceeds it is rejected), **frequency range**, and **pulse widths**.
- Default **delays and jumps** (each layer can use its own with **Override delays**) and the **red pointer** speed.
- **Red pointer offset** (**X** and **Y**, from −50 to 50 mm): where the pointer lands relative to the beam. Framing and the red run **compensate** for it, so that the pointer passes where the laser will mark. **A real mark never uses it.** To measure it, mark a test point at low power and measure how far away the pointer lands.
- **Fill and image overscan** (0 to 20 mm; 0 by default). See below.
- **Field correction**: scale, offset, and rotation, head mounting (invert axes or swap X and Y), and barrel, keystone, and skew, plus the note of the `.cor` file (informational only). A **field calibration wizard** (in three steps: mark a grid of two squares, measure it with a ruler, and compute the scale and barrel) fine-tunes those values; what it measures is the error that **remains** with the current correction, so repeat it until the error is small. It is Tlaku's own model, not verified with a real lens.
- **Cylinder and cone**, **split and repeat marking**, and **Required inputs** (below).

### Connecting and marking

With a real USB board, each connection opens **Laser safety before connecting** and asks you to tick **all three** confirmations (glasses with the right optical density for the wavelength and power, laser inside a closed enclosure, and emergency stop within reach). **It is asked on every connection; Tlaku does not remember it.** With the simulated board it is not needed.

When you **Start**, the dialog says **Galvo: the laser will fire**, repeats the warning about the invisible beam and that **there is no pause**, and summarizes the field, the estimated time, and the maximum power. With split or repeated marking it says in how many steps it marks and how many times you will have to **move the material** and press **I moved the material: continue** (the board does not move the table: Tlaku stops and waits). **Stop** turns the source off and empties the board's list; if the board does not confirm turning the source off, a red notice (**The source may have stayed ON**) tells you to cut the power **now**.

### Framing with the red pointer

In the **Machine** panel, **Framing with the red pointer** draws the job with the pointer **without ever enabling the laser**:

- **What to draw**: **Rectangle** (the one the job occupies), **Outline** (the job's strokes), **Convex hull** (the minimum convex polygon that wraps what is marked: tighter than the rectangle on a round or diagonal design), or **Loose objects** (one rectangle per separate object, to see where each piece will land).
- **Loops**: how many loops it draws before switching off by itself. Empty (**∞**) repeats until you press **Stop and switch off**. While it runs, a **counter** says **Loop 3 of 5** (or **Loop 3** if you did not enter a number).
- **Move and scale live**: while the pointer draws, the arrows and the **Larger** and **Smaller** buttons (with the **Step** you choose) shift or resize the frame to align it with the piece, and below you see the offset and the scale (`Offset X 1.00 · Y 0.00 mm · scale ×1.10`). **Reset** leaves it as it was. **It is only the frame: the design does not change.** If the result would go outside the field, the adjustment is discarded (**That adjustment put the frame outside the field**). The buttons are only enabled while a frame is running.
- **Red run** traces **all** of the job with the laser disabled and the red pointer; **Turn pointer on** and **Turn pointer off** drive it by hand.

### Overscan

A galvo has little inertia, so **by default it uses no overscan** (0 mm). If you need it, in the profile you can set **Fill and image overscan**: extra millimeters at the start and end of each line, which the mirrors travel **with the laser off**, at marking speed and with no jump delay, so the stroke starts at constant speed. A layer can set its own. **The design plus the overscan must fit in the field.** It has not been verified with hardware.

### Required inputs (input interlock)

If your machine's lid or emergency stop reaches an input on the board, in the profile, **Input interlock**, you choose for each of the 16 inputs whether **Do not care**, **Must be high**, or **Must be low**. **Before each job and each frame** Tlaku reads the inputs and requires that state; if it is not met, it **does nothing**, and the panel says **OPEN: nothing is marked or framed**. When it is met, it says **Satisfied**.

> **Warning:** the bits of your source's emergency stop and alarms **are not documented**. In the panel, **Inputs (read-only)** shows the 16 inputs live: trigger the emergency stop or open the lid, see which input changes, and put it in the profile. **Until you measure it and set it, Tlaku cannot check the interlock for you.** Only inputs are read: Tlaku does not write outputs.

## Testing without a machine: the simulated board

All of the following can be done without buying anything. With a sample design (`cuadro.svg`, a 30 mm square):

```
tlaku galvo mark cuadro.svg --emu --frequency 30 --power 5
```

Tlaku compiles the job, sends it to the simulated board, and summarizes what happened: how many batches and commands, that "the source was armed during the job" and that "the source was switched off and the list was emptied", and what the emulator saw (stretches with the beam out, that the source ended up off, and 0 protocol violations).

To see the framing with the red pointer (two loops and it ends):

```
tlaku galvo light cuadro.svg --emu --loops 2
```

And to only compile and review, without connecting to anything:

```
tlaku galvo mark cuadro.svg --dry-run --frequency 30
```

which prints something like "Board list: 18 commands in 1 batches of 256; 4 marks (120.0 mm), 1 jumps (70.7 mm) …" and ends with "Dry run (--dry-run): the board was not connected".

## The machine profile: flags

From the command line, your machine's profile is written in each command:

| Flag | What it is | Default |
|---|---|---|
| `--field` | Side of the F-theta lens's field, in mm (110, 150, 200, 300…) | 110 |
| `--source` | `fiber-q` (Q-switched fiber), `fiber-mopa`, `co2`, or `uv` | `fiber-q` |
| `--freq-range MIN-MAX` | Frequency range of **your source**, in kHz. Nothing outside it is sent | `20-80` |
| `--pulse-widths` | Allowed pulse widths in ns, comma-separated (required with `fiber-mopa`) | — |
| `--max-power` | The profile's maximum power, in %. A layer that exceeds it is an error | 100 |
| `--light-speed` | Red pointer speed, in mm/s | 2000 |
| `--jump-speed` | Jump speed, in mm/s | 4000 |
| `--invert-x`, `--invert-y`, `--swap-xy`, `--rotate` | How the head is mounted: negate an axis, swap them, or rotate the field (degrees, counterclockwise) | — |
| `--require-input MASK=VALUE` | Software interlock (see [Galvo safety](13d-galvo-seguridad.md)) | — |

And the marking parameters (`mark`):

| Flag | What it is | Default |
|---|---|---|
| `--frequency` | Frequency in kHz. **Required** and within the profile's range | — |
| `--pulse-width` | Pulse width in ns (MOPA and UV sources; it must be in `--pulse-widths`) | — |
| `--mark-speed` | Marking speed, in **mm/s** | 1000 |
| `--power` | Power in % | **10** (low on purpose) |
| `--laser-on`, `--laser-off`, `--polygon`, `--end-delay` | Laser delays in microseconds (the on and off ones can be negative) | the profile's |
| `--repeat` | Times the board repeats the job | 1 |

The other design arguments (`--layer`, `--mode`, `--interval`, `--image-size`…) are the same as `tlaku gcode`; `tlaku galvo mark --help` lists them.

An example with a MOPA fiber with a 150 mm field:

```
tlaku galvo mark logo.svg --dry-run --source fiber-mopa --pulse-widths 4,50,200 \
  --pulse-width 50 --frequency 40 --power 15 --field 150
```

The example's pulse widths are made up: enter the ones **your** source supports according to its manual. Tlaku rejects a width that is not on the list and a frequency outside the range, with a clear message ("frequency 5 kHz outside the profile's range (20 to 80 kHz)").

## How the design is placed

The lens's field is a **centered square**. On the command line the design is placed with the origin (lower left) at the field's lower left corner and Y pointing up. A design that goes outside the field **is not marked** (`mark` fails); the frame tells you the exact rectangle ("Frame: X 5.00 to 35.00 mm, Y 5.00 to 35.00 mm (110 mm field)").

By default a galvo **uses no overscan**: the mirrors have little inertia, so fills and images come out without the margin that a gantry laser does need. If you need it, the profile has a **Fill and image overscan** (see "The galvo in the app").

## A safe workflow: from the emulator to the source

1. **No machine.** `--emu` and `--dry-run` until you understand the output.
2. **Board connected, no laser source** (the source's cable disconnected). This is the test in [Galvo safety](13d-galvo-seguridad.md): identify the board and move the mirrors with nothing able to emit.
3. **With the source, `--red-only`**: traces all of the job with the **laser disabled** and the red pointer, to see the real path.
4. **Actually mark**, with minimum power, on scrap material, with glasses, an enclosure, and the emergency stop.

With real hardware, **everything except `list`** requires the `--i-understand-laser-safety` flag; without it, the USB is not even opened. `mark` also asks "The laser WILL FIRE. Glasses on, enclosure closed, shutter and emergency stop ready, and the right material under the beam? [y/N]"; `--yes` skips the question, and **you should not use it** in your first tests. **Ctrl+C** stops the job and turns the source off.

## What Tlaku does not do (on purpose)

- **There is no pause**: it can only be stopped. See [why](13d-galvo-seguridad.md).
- It does not write the board's **EEPROM**, firmware, **correction table**, or configuration. If your board needs initializing, do it beforehand with EZCAD2.
- It does not apply EZCAD's **`.cor`** correction files (their structure is read, but it is not known what the numbers mean). Field correction (scale, rotation, barrel, keystone, skew) is calculated by Tlaku on the computer with its own model, and assumes the board's table is the identity.
- It does not mark **on the fly**, does not drive Z axes or tables, and does not write arbitrary outputs.
- It does not calibrate the lens with the **9 points** or EZCAD's `.cor` files (the app's wizard measures two squares and computes scale and barrel with its own model), and it does not import EZCAD configuration files.

Next: [Galvo safety](13d-galvo-seguridad.md)
