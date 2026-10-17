# Advanced machine control

What the **Machine** tab offers beyond moving, framing, and starting: **macros**, moving the **Z** and **U** axes, **saved positions**, **focusing**, the **test fire**, the controller's **`$$` settings** with backup, **low-power and looping framing**, **laser 2**, and **`.tlakubundle` bundles** for taking your configuration to another computer.

> **Warning:** of all this, only jogging the head, homing, the test fire at low power, and assist air were tested on a real machine (a Creality Falcon2 Pro). The rest (macros, Z and U axes, the probe, the second laser, and so on) was validated with the emulator and the virtual machines ([Connecting your machine](08-conectar-maquina.md)). Everything that turns the laser on or writes to the controller asks for confirmation, and even so you should try it first on a virtual machine. Almost all of this is for the **GRBL family** (GRBL, grblHAL, FluidNC, GRBL-LPC); with Marlin and Smoothieware several functions change or do not exist, and with Ruida they do not apply (see [Ruida and K40](13b-ruida-y-k40.md)).

## Macros

A **macro** is a button that sends the machine one or more lines of G-code that you saved in the profile. It is for things you always do: turn on the air, take the head to the corner where you remove the piece, ask for the status.

**Creating a macro.** **Machine** menu, **Manage Devices…**, choose the profile, and scroll down to the **Macros** group:

1. Press **Add macro** (up to 10 per profile).
2. Give it a **name** (it appears on the button).
3. In **G-code (one line per command)** type the lines, up to 50.
4. Tick **Ask for confirmation before sending it** if you want to see it before it goes out.
5. **Save changes**.

The buttons appear in the **Machine** tab, **Macros** section, and each macro is also a command (**Macro 1** to **Macro 10**) to which you can assign a shortcut in **Edit > Edit keyboard shortcuts…** (see [Preferences, shortcuts, and autosave](02b-preferencias-y-atajos.md)).

**Examples.**

| Macro | G-code | What happens when you press it |
|---|---|---|
| Air on | `M8` | Asks for confirmation (it is not read-only) and sends the line |
| Air off | `M9` | Same |
| Unload corner | `$J=G90 G21 X0 Y380 F3000` | Asks for confirmation, because it moves the machine; takes the head to X0 Y380 at 3000 mm/min |
| Information | `$I` and on another line `$G` | They are read-only: they go out directly, without asking |

**Safety rules.** Macros go through the same rules as the [safe console](09-enviar-con-seguridad.md):

- Only read-only lines (`$I`, `$$`, `$#`, `$G`, `$N`, and `M5`) go out without asking. If any line moves the machine, turns the laser on, or changes a setting, **the exact lines are always shown** and you must confirm, even if you did not tick the checkbox.
- A macro **cannot** contain `?`, `!`, or `~` (real-time commands): to pause or reset use the panel buttons. The editor tells you which line is not valid.
- **They are never sent during a job.**
- Lines go out one by one and the macro **stops at the first one the machine rejects**, telling you which one it was and what the machine answered.
- Macros do not exist with Ruida.

## Moving with Z and U

If the profile declares an axis, the **Move the head** section adds the **Extra axes** block with the **Up** and **Down** buttons.

- **Z axis:** turned on in the profile with **Motorized Z axis** (**Axes and accessories** group, see [Rotary and Z axis](12-rotativo-y-eje-z.md)). **Up** moves the head away from the material and **Down** brings it closer; with **Invert Z** on, Tlaku already compensates for the direction.
- **U axis:** turned on with the **The machine has a U axis (rotary or another extra axis)** checkbox. Since firmwares name it differently, choose the **U axis letter in the G-code** (`U`, `A`, `B`, or `C`). This checkbox enables manual movement. Jobs only move U if you set a **Job U offset** other than 0 in the same group: it is how far the axis moves **at the start of each job** (and it is undone at the end, with relative moves and the laser off). With 0 (the default), **the jobs' G-code does not touch U**.
- Each click moves the same **step** as the pad (from `0.01` to `500 mm`) at the **Speed** of **Move the head**. With a job running, no connection, or in alarm, the buttons are disabled and say why.

> **Example:** your motorized table goes down with +Z. Tick **Motorized Z axis**, set the step to `1`, press **Down** three times to bring the head closer to the material, and confirm the distance to the focus by eye.

## Focus

With the Z axis active, **Focus** takes Z to a focus height that you save once:

1. Focus your material by hand (with **Up** and **Down** or with the machine's panel) and note the Z position your controller reports. Tlaku's status bar shows only X and Y; you can see Z in the status reports in the **Console**, if your firmware reports it.
2. Type that height in **Focus height (Z)** (in mm, in the controller's work coordinates).
3. When you change material, adjust the value; after that **Focus** takes Z up to there.

The command is also in the command menu as **Focus (move Z to the focus height)**. Autofocus and Z optimization are missing.

## Saved positions

To get quickly back to places you use: the loading corner, the table center, the rotary position.

1. Move the head to the place.
2. In **Saved positions** type a **Position name** and press **Save here** (needs the machine connected). Up to 20 are stored per profile, and a repeated name **replaces** the previous one.
3. To go back, press **Go** next to the name: the head moves with the **laser off** at the **Move the head** speed. **Delete position** removes it.

They are stored in the profile (and travel in bundles) in **machine coordinates**, so they remain valid even if you change the job origin.

## Test fire

A short laser pulse at low power to see **where the spot lands** without launching a job (for example, to check focus or the origin on a piece of cardboard).

> **Warning, safety.** The test fire **really turns the laser on**. Put on the right safety glasses, keep your hands clear, keep the emergency stop within reach, and make sure there is nothing under the spot that you do not want marked. Do not use it with a machine that is not in your sight.

- In **Test fire** adjust the **Power** (from 0.1 to **20 %**, 2 % by default) and the **Duration** (from 10 to **2000 ms**, 200 ms by default). The limits cannot be exceeded.
- **Test fire…** opens a **double-confirmation** dialog: it says "The laser will turn on at 2 % for 200 ms and switch itself off", repeats the warning, and **does not let you fire until you tick "I am wearing protective glasses and the area is clear"**. Then press **Fire**.
- If you need to cut it short, **Turn off now** sends an immediate reset and an `M5`.
- Internally, Tlaku sends `M3` with a low S, a pause, and **always `M5` at the end**, no matter what fails. If it cannot even confirm that the laser turned off, it tells you clearly and asks you to reset the machine or cut the power.
- It is disabled without a connection, during a job, or if the machine is not idle (alarm, hold, running).

## The controller's `$$` settings, with backup

**Machine Settings…** (button in the **Machine settings** section of the panel, or the `machine.settings` command) reads the GRBL controller's `$n` values and shows them with **name, description, unit**, and a **Critical** mark on those that, with a wrong value, can make the machine move wrongly, remove its protections, or turn laser mode off. It covers GRBL 1.1 and the known grblHAL settings; a setting that Tlaku does not know appears as "Setting without a description".

- **Reload** reads the machine again. The search box filters by number or name.
- **Edit** a value validates it (number, range, 0 or 1) and moves on to **Review the change**, which shows the **current value** and the **exact line** that will be sent, with the warning that it is stored in permanent memory (EEPROM). Only **Write `$n=value`** sends it.
- When writing, it **waits for the `ok` of each line**, stops at the first error, and at the end **rereads `$$`** to verify; if a value does not match, it tells you (sometimes the controller needs a reset).
- **FluidNC** stores its configuration in `config.yaml`: here you only see a view, with a note. With Ruida the button does not appear.

**Backup.** Before touching anything, press **Export backup**: it saves a `.tlakuset` file with all the settings. To go back, **Restore from file…**:

1. Choose the `.tlakuset`.
2. You see a table of **differences** ("Now → backup"), with a checkbox per setting. Untick what you do not want to change. Settings that the machine does not have, or invalid values, are skipped with a warning; if the backup is from another firmware family, it warns you.
3. **Write N setting(s)** sends them one at a time, with the same checks.

> **Tip:** export a backup the first time you connect a new machine and after you have it well calibrated. If a setting spoils the behavior, restoring takes a minute.

There is no LightBurn `.lbset` format and no automatic backup.

## Low-power and looping framing

Below the framing buttons (**Job** section) there are two options (see also [Sending jobs safely](09-enviar-con-seguridad.md)):

- **Frame with the laser on at low power:** framing traces the edge with the laser on so you can **see it on the material**. You choose the **Framing power** (from 0.1 to **20 %**, 1 % by default). The laser turns on just before the first stretch of the edge, so the move to the first corner is still made with the laser off; the final `M5` turns it off. It is available only if your machine's dialect puts **power inside each move** (GRBL 1.1 and similar); otherwise the checkbox is disabled and says why. With Ruida, framing is always with the laser off. It warns that the laser can mark or burn: **safety glasses are still mandatory**. The switch is **not remembered** when you restart the application (the power is).
- **Repeat the frame until stopped:** when one pass ends, another begins, so you can position the material at your leisure. A notice ("Looping frame…") and the **Stop repeating** button show it; it also stops with **Stop**, on disconnection, or when a job starts.

> **Example:** you put in a piece of cardboard, turn on the loop with the power at `1 %`, and move the material until the visible edge lands where you want. Then **Stop repeating** and **Start**.

## Laser 2

For machines with two lasers that are selected with a firmware command:

1. In the profile, **Axes and accessories** group, tick **The machine has two lasers**.
2. In **G-code dialect**, **Second laser** group, type the **Select laser 2** line and the **Back to laser 1** line that **your** firmware uses. These fields are edited with the **Custom G-code** controller type; if you leave the select-laser-2 line empty, nothing is emitted and a warning is shown.
3. In each layer's editor the **Laser 2** option appears: layers that tick it run with the second laser.

If a layer asks for laser 2 and the active machine does not have it, the layer **runs with laser 1** and a warning is shown. Tlaku sends the select command when switching lasers between layers; there is no head 2 offset, and no laser 2 with Ruida.

## Configuration bundles (`.tlakubundle`)

A **bundle** is a single file for taking your configuration to another computer or sharing it: your **machines** (with their macros and saved positions, and which one is active), your **materials library**, your **cameras** (calibration, alignment, and presets), your **own art library**, and your **preferences and shortcuts**.

**Exporting.** **File > Export settings bundle…**: tick the sections you want (you can take only the machines, for example) and save.

**Importing.** **File > Import settings bundle…**: choose the file. Before touching anything you see a **preview** with a checkbox per section and a summary of each:

- **Machines:** how many are new, how many would replace one with the same identifier (it tells you which), and how many do not change. Invalid profiles are discarded with the reason.
- **Materials:** they are **merged**, never replacing yours: the ones you already have are kept and a different material with the same identifier comes in as a copy.
- **Cameras:** the lens calibration, the bed alignment, and the head-mounted camera calibration **replace** the ones you have (there is only one of each; the preview tells you which). If a different lens comes in and the bundle does not include its alignment, yours is removed because it would no longer fit. Named **camera presets** are merged and never overwrite yours. **Network camera URLs travel without a username or password**: those credentials stay only on your computer; if a file from someone else contains them, they are also removed on import.
- **Own art library:** merged like materials; a drawing with the same name and different content comes in under a new name. The library included with Tlaku does not travel (every installation already has it).
- **Preferences and shortcuts:** they are applied; preferences or commands that this version does not know are ignored or kept, and if the bundle includes shortcuts, they replace the current ones.

Nothing is written until you press **Apply**. You cannot import while a job is running. Materials can still be exchanged separately as `.tlakumat` ([Materials](11-materiales.md)), and preferences and shortcuts as `.tlakuprefs`.

Next: [Print & Cut](09c-print-and-cut.md)
