# Troubleshooting

What the alarms and errors Tlaku shows mean, and what to do when there is no port, the laser does not fire, or the size does not come out right.

> **Warning:** these solutions come from Tlaku's code, from the public documentation of the GRBL firmware, and from user reports. Nothing has been tested with a real machine yet. If something does not match what you see, the machine rules: when in doubt, stop the job and turn off the laser.

## Where to look first

- If there is an alarm, the app shows a banner with the code, an explanation, and the **Clear alarm ($X)…** and **Home ($H)…** buttons. Each button asks for confirmation; Tlaku never sends them on its own.
- The **Console** tab keeps everything sent and received. The copy button puts the log on the clipboard, useful for asking for help (**Help > Report a Problem…** menu).
- To see the machine's settings, send `$$` from the console (there is a quick-query button).

## `ALARM:N` alarms

An alarm locks the machine until you clear it or home. These are the codes with their own text in Tlaku.

| Code | Meaning | What to do |
|---|---|---|
| `ALARM:1` | A limit switch was triggered. | Move the head off the limit by hand and clear the alarm. |
| `ALARM:2` | The requested move goes outside the machine's travel. | Same; check that the design fits on the bed. |
| `ALARM:3` | Reset during a move: the position was lost. | Home if there are limit switches; if not, set the origin by hand. See the Falcon2 section. |
| `ALARM:4`, `ALARM:5` | Probe failure. | Clear the alarm. Does not apply to a normal laser. |
| `ALARM:6` to `ALARM:9` | Homing failure (reset, door, limit switch not released or not found). | Check the door, limit switches, and wiring; repeat `$H`. |
| `ALARM:10` | Depends on the firmware: auto-squaring without a second limit switch (GRBL 1.1), emergency stop (grblHAL), or spindle failure (FluidNC). | If the app knows your firmware, it tells you which. With an emergency stop, release it before clearing the alarm. |
| `ALARM:11` to `ALARM:22` | grblHAL only (for example: 11 asks for homing, 12 limits engaged). | Read the banner text. |

When to use each button:

- **With limit switches and homing enabled (`$22=1`):** the normal choice is **Home ($H)**. The head travels to the limit switches, so clear the area first.
- **Without limit switches (`$22=0`):** the normal choice is **Clear alarm ($X)**. Without homing, the machine does not know where it is: set the origin again.
- `$X` checks nothing. Use it only if you know why the alarm triggered.

## `error:N` errors

They appear when the machine rejects a line of G-code or a command.

| Code | Meaning | Common cause |
|---|---|---|
| `error:1` | The command letter is missing. | A badly written line in the console or in the start G-code. |
| `error:2` | Bad number format. | An `S` with no value in the profile's start or end G-code. |
| `error:3` | Invalid `$` command. | A badly written setting command. |
| `error:5` | Setting disabled. | `$H` on a machine without homing enabled. |
| `error:8` | The machine is not idle. | Command sent during a move. |
| `error:9` | G-code locked out by an alarm. | There is an unresolved alarm. |
| `error:20` | Unsupported G-code command. | The firmware does not understand one of the dialect's commands (check the controller type). |
| `error:22` | Feed rate undefined. | `F` is missing on a move. |
| `error:33` | Invalid move target. | Coordinates outside what the firmware accepts. |

The numbers **18, 19, and from 39** mean different things in grblHAL and in FluidNC. If the app does not know your firmware, it only says "extended error"; in FluidNC you can look it up with `$E=N`.

> **Note:** `error:5` (lowercase, from GRBL) is not the same as `ERROR:05` (uppercase, from Creality). Tlaku tells them apart; so should you when reading the console.

## Creality Falcon2

The Falcon2 has its own codes, which arrive as `ERROR:NN` (uppercase and two digits). Tlaku shows them in a separate banner, **Machine fault (Creality)**.

| Code | Meaning |
|---|---|
| `ERROR:01` | Abnormal angle: the tilt or vibration sensor was triggered. |
| `ERROR:02` | There is no G-code file in the root of the TF card. |
| `ERROR:04` | No airflow: check the pump, the flow knob, and the tube. |
| `ERROR:05` | Flame detected: the machine stopped. |
| `ERROR:06` | Contaminated lens: clean it. |
| `ERROR:07` | Laser module too hot. |
| `ERROR:08` to `ERROR:12` | A module sensor failed: restart the machine. |
| `ERROR:13`, `ERROR:14` | High temperature of the module's or the host's CPU. |
| `ERROR:24` | Machine not sealed: close the lid and the drawer. |
| `ERROR:25` | Abnormal air pump. |

To recover, fix the cause and restart with the machine's **Reset** button (and Start on its panel if it asks). Tlaku does not clear it on its own.

Other Falcon2 points, according to public reports (untested here):

- **`ALARM:3` is the most common.** The causes users cite are a weak power supply, the USB cable, a sensor that resets the board, or raising the power too high on the 40 W module. Try another power supply and another cable.
- **USB-C cable:** a USB-C to USB-C cable may not work; use a USB-A to USB-C one. It needs the CH340 driver.
- **Bed and speeds:** different firmwares come with different values. Read your machine's with `$$` and compare them with your profile.
- **G54 to G59 offsets:** they may show `-nan`. Tlaku does not use them.
- If engraving comes out darker at the corners, the profile must use `M4` (**GRBL 1.1** type), not the legacy type with `M3`.

You can see how these faults appear, without risk, in the virtual machine (**Simulate fault**: flame, tilt, alarm 3, cable unplugged). See [Connecting your machine](08-conectar-maquina.md).

## The app does not find the port

> If your machine connects **over the network** (TCP, WebSocket, or Ruida) there is no serial port to look for: see the sections **The network connection fails** and **Ruida does not respond** below.

1. Check that the cable carries **data** (some only charge) and that the machine is on.
2. Press **Scan for ports again**.
3. Close any other program that has the port open (other laser software or a serial monitor). Only one at a time can use it.
4. Install the board's driver if needed (see below) and connect again.

**Windows.** Open Device Manager and look at **Ports (COM & LPT)**. If your machine appears as an "unknown device" or with a warning, the chip's driver is missing, normally **CH340** (sometimes FTDI). Install it, reconnect the cable, and choose the `COMn` port that appears.

**macOS.** Some machines with a CH340 chip need the chip manufacturer's driver. After installing it, the port appears with a name like `/dev/cu.usbserial-...`. If macOS asks you to approve a system extension, do so in System Settings and restart. Verify in your macOS version whether a driver is needed.

**Linux.** Your user must belong to the `dialout` group (on Arch and derivatives, `uucp`):

```sh
sudo usermod -aG dialout "$USER"
```

Log out and back in for it to take effect. If the machine appears and disappears, it is usually `ModemManager` or `brltty` grabbing the port: disable `ModemManager` (`sudo systemctl disable --now ModemManager`) or uninstall `brltty`.

## It connects but does not respond

- **Baud rate:** most GRBL machines use 115200; Marlin usually uses 250000; some Neje models ask for 38400.
- **Wait a few seconds.** Opening the port resets many boards; Marlin takes longer.
- If detection says "Not recognized", check the cable, try other baud rates, or choose **Other / generic** in the wizard.
- If the machine returns no settings, check that it is a GRBL controller.

## The laser does not fire

Check in this order:

1. **Laser mode off.** If the app warns about `$32=0`, enable it with `$32=1` (there is a button in the Machine panel). Without it, `M4` does not follow the speed and engraving comes out wrong.
2. **Power.** The layer has power greater than 0 and the profile's **Max S** matches the machine's `$30` (1000 on current GRBL, 255 on old versions, 1 on GRBL-LPC and Smoothie).
3. **Layer output.** The layer has output enabled (it is not turned off or a guides layer: guides are never sent) and has visible objects of its mode.
4. **Controller type.** A type that does not match your firmware changes the G-code (for example `M4` where only `M3` exists). Check the profile.
5. **Machine safety.** Lid, key, emergency stop, or sensors (on the Falcon2, `ERROR:24` or `ERROR:01`). Look at the console.
6. If you use the virtual machine, remember that **there is no real laser**: nothing is engraved.

## The size does not come out as expected

- **Measure on the canvas first.** Select the object and look at Width and Height in the numeric bar. Images are imported at 96 pixels per inch: a photo with many pixels can come out large or small. Fix it in the numeric bar.
- **Machine that moves too much or too little.** If the square comes out at a different measurement, `$100` and `$101` (steps per mm for X and Y) may be wrong. Measure what the machine did and adjust: new value = current value times (requested measurement / actual measurement). Do it from the console and carefully.
- **Rotary enabled.** With a rotary, the Y scale changes. If you are not using it, turn it off in the profile.
- **Profile bed.** The profile's work area must match `$130` and `$131`.
- **It goes off the bed.** The **Start from** section of the Machine panel warns "It goes off the bed" and tells you on which side.
- **It comes out mirrored or flipped.** The profile's **Machine origin** does not match the real corner of your X0 Y0.
- **It comes out shifted.** Check **Start from** and the job point that is anchored: see [Sending jobs safely](09-enviar-con-seguridad.md).

## The network connection fails

For GRBL, grblHAL, and FluidNC over TCP or WebSocket (see [Network connection and automation](08b-red-y-automatizacion.md)). Always start with **Test connection** in the profile: it moves nothing and the message says what failed. The most common messages:

| Message | What to check |
|---|---|
| "'X' was not found on the network" | The name does not resolve (`fluidnc.local` does not work on all networks). Use the IP address |
| "The machine at X refused the connection" | The port is closed or the board's network service is off. Telnet is usually 23; FluidNC's and ESP3D's WebSocket, 81. Look at the board's network configuration |
| "X did not answer in time" | 5 seconds passed with no answer. Is it on? Are your computer and the machine on the **same network**? Did the IP change (the router reassigned it)? Reserve a fixed IP |
| "X cannot be reached" | There is no route to that device: Wi-Fi or cable disconnected, or a guest network that isolates devices |
| "X closed the connection when it was opened: is another program connected to it?" | Many boards accept only one client at a time. Close the machine's web interface in the browser, another copy of Tlaku, or other laser software |
| "Encrypted connections (wss://) are not supported" | Use a `ws://` address |
| "The machine answered, but the firmware was not recognized" | There is a connection, but the greeting is not GRBL's. Check that the port is the controller's console one and not that of another service |

Other ideas:

- **It connects but drops in the middle of the job.** It is almost always the Wi-Fi: bring the machine closer to the router or use a cable. A connection that drops can leave the machine running what it already had in its buffer; keep the emergency stop within reach.
- **You cannot change the address in the Machine tab.** This is on purpose: the address belongs to the profile. Open **Manage Devices…** and disconnect first.
- **Trying the app without the machine's network:** use the **Virtual machine over network**. If that one connects, the problem is in the machine or in your network, not in Tlaku.
- **UDP automation does not answer** (`PING` with no reply): check that it is turned on in **Preferences > Automation**, that the status says "Listening on 127.0.0.1:…" (if the port is busy it says so), and that you send the datagram to `127.0.0.1`, not to your network IP: it only listens on your own computer.

## Ruida does not respond

The Ruida connection is experimental (see [Ruida and K40](13b-ruida-y-k40.md)). Check in this order:

1. **The IP.** The profile's must be the controller's (on its screen, in the network menu). Your computer must be on the **same /24 network** (for example, both on `192.168.1.x`).
2. **Another program has the port.** If you see "could not open local UDP port 40200 (is another laser program open?)", close LightBurn, another copy of Tlaku, or any other laser software: only one at a time can use that UDP port.
3. **The computer's firewall** may block UDP. Allow Tlaku on the private network.
4. **The magic.** In **Automatic** Tlaku tries `0x88` and then the other known ones; if your controller uses another, set it **Manual** (`0x88` on almost all RDC644x, `0x11` on the 634XG, `0x38` on the RDL9635).
5. **"No response from the machine"** when connecting or reading information: the machine did not answer. Try a direct cable to the PC or to the router, without Wi-Fi repeaters. UDP has no retries and Ruida controllers are not very tolerant of a slow network.
6. **The bed does not match.** If the warning says the profile's bed does not match the machine's, no job will be sent: fix the profile's bed size with what the machine reports (**Read information**).
7. **During a job** some controllers ignore the network; Tlaku does not take that for a lost connection while a job is running.
8. **"The machine rejected packet N"** when uploading: the job did not arrive in full. Tlaku sends *Stop*; check the network and try again.
9. **Ruling out Tlaku:** connect with the **Ruida (CO2)** virtual machine. If that one works, the problem is in the network or in the controller.

## The K40 does not show up over USB

K40 with M2 Nano support is experimental (tested only against the emulator) and is in the app and on the command line (`tlaku k40`); see [Ruida and K40](13b-ruida-y-k40.md). The connection is over direct USB, not a serial port: in the app, the board must appear as `usb:k40:<n>` in the Machine panel's port list. If it does not appear, or `tlaku k40 list` says "No CH341 boards (USB 1A86:5512) connected":

1. **Cable and power.** Use the USB cable that comes with the machine, make sure the machine is on, and try another USB port on the computer (preferably direct, without a hub).
2. **Another program has the board.** K40 Whisperer, MeerK40t, or LaserDRW cannot have it open at the same time. Close them.
3. **Linux: missing permissions.** Your user needs a udev rule. Create `/etc/udev/rules.d/99-k40.rules` with this line:

   ```
   SUBSYSTEM=="usb", ATTR{idVendor}=="1a86", ATTR{idProduct}=="5512", MODE="0666"
   ```

   reload with `sudo udevadm control --reload-rules && sudo udevadm trigger` and unplug and plug the board back in. To check that the system sees it: `lsusb | grep 1a86:5512`. (`MODE="0666"` opens the board to all users; to restrict it use `TAG+="uaccess"` instead.)
4. **Windows:** the board must use the **WinUSB** driver (for example, with Zadig). With the chip manufacturer's (WCH) original driver Tlaku cannot open it.
5. **macOS:** it needs no driver (the app carries the USB entitlement). If you have WCH's installed, uninstall it.
6. **It appears but fails on connect (or `tlaku k40 status` fails) or says Moshiboard.** A Moshiboard shares the same USB, but it is not an M2 Nano and Tlaku does not support it. If it is an M2 Nano and does not respond, power the machine off and on and try again.
7. **It keeps waiting for the job to end.** If your board does not report the "finished" state, try `--no-wait-finish`.

And remember: the M2 Nano **does not control power by software**; if the laser does not mark, turn up the machine's knob, not the layer's power.

Other K40 questions in the app: if the panel says "Unknown position", **home** (stopping a job loses it; sending a job recovers it by homing first). If you changed the profile (board, bed, origin), **reconnect**. If the "The board did not confirm the stop" notice appears, use the machine's **emergency stop**. The console, overrides, `$` settings, Z, U, and air do not exist on the K40.

## Warnings when calculating or starting a job

- **"The serial link cannot keep up".** The job asks for more lines per second than fit on your cable at that baud rate: the machine would run out of lines, pause, and could mark the material. Lower the speed of the layers with the most detail (the warning says to what fraction), simplify the design (**Review design**, **Optimize shapes**), or use a faster link (native USB, network). See [Sending jobs safely](09-enviar-con-seguridad.md).
- **"The layer asks for X mm/min and the machine reaches at most Y".** The layer's speed exceeds the profile's. The machine will run at its maximum and the result will be darker; lower the layer's speed, or fix the profile's max speed if it is set wrong.
- **"Ruida supports 128 layers…".** Your design has too many colors with different settings for the `.rd` file. Unify colors or settings of consecutive layers; see [Ruida and K40](13b-ruida-y-k40.md). A speed above 36,000 mm/min is clamped to that cap.
- **Review design found something serious.** Objects off the bed or a layer with no output. Press **Look**: [Review design](07b-revisar-diseno.md).
- **My board resets (and loses its position) every time I connect.** In the profile, tick **Open the serial port without raising DTR**. On Linux, and with some adapters, it will have no effect. See [Connecting your machine](08-conectar-maquina.md#profile-settings-worth-knowing).
- **The board loses lines or gives stray `error:` messages mid-job.** Lower the profile's **Receive buffer**; if that is not enough, try **Synchronous streaming** (slower).

## Macros, `$$` settings, and test fire

- **"Macro … stopped at …: the machine answered …".** The macro sends lines one by one and stops at the first one the machine rejects. Look at the `error:N` in the **`error:N` errors** section of this page and fix that line in the profile.
- **"Macro … is not valid".** It has an empty name, no lines, more than 50 lines, or a line with `?`, `!`, or `~`. The macro editor marks which one.
- **A macro does nothing during a job.** This is on purpose: macros are not sent while a job is running.
- **I wrote a `$` setting and "on re-reading some values do not match".** The controller said `ok` but stored another value, or only applies it on restart. Restart the controller and press **Reload**. If you made a mistake, **Restore from file…** with the `.tlakuset` backup you made beforehand.
- **The Test fire button is gray.** It needs the machine connected, no job running, and idle (no alarm or hold). Hover over it to see the reason.
- **"It could not be confirmed that the laser turned off".** Restart the machine or cut the power and inspect it before continuing. It may be a connection dropped right during the fire.
- **Frame with the laser on is disabled.** Your machine's dialect does not put power inside each move, so there is no way to turn the laser off during travel: you can only frame with the laser off.

## Problems importing or exporting

- **I imported a `.lbrn` or `.lbrn2` and cut settings are missing.** The import notice lists what it could not bring in. The importer does translate perforation, dot mode, flood fill, overcut, ramp, pauses, cut-through, and sub-layers (several conversions are inferred: check each layer in **Advanced cutting** and in the **Fill** selector). What has no equivalent (PPI, laser frequency, second laser, constant power, blower speed, per-object fill, and lead-in and lead-out) is warned about and has to be set by hand. LightBurn's manual tabs are distributed automatically. See [Coming from LightBurn](14-desde-lightburn.md).
- **A PDF or AI arrives without text or without photos.** Only paths are imported: convert the text to curves in the source program and export again. The import notice says so. If the file has several pages, the first is imported; choose another in **Import with options…**.
- **"It is an old AI (PostScript)".** Open it in Illustrator and save it with **Create PDF Compatible File**, or export it to PDF or SVG.
- **An HPGL or an SVG comes out at a different size.** HPGL does not declare its scale (40 units per mm is assumed) and an SVG without units uses 96 dpi. Use **Scale (%)** and **Resolution (dpi)** in **File > Import with options…**; Illustrator usually needs 72.
- **A DXF comes out about 25 times larger or smaller.** It is a DXF without units in inches or millimeters: choose the **Unit for a DXF without units** in the import options.
- **"The file contains no paths".** Read the rest of the message: it says **what was skipped** (for example, "3 HATCH and 12 TEXT were skipped" in a DXF, or text and shapes without an outline in an SVG) and what to do: convert it to outlines or paths in the source program and export again. See [Import and export](03b-importar-y-exportar.md).
- **I imported an SVG and an image appeared instead of lines.** If the SVG has no paths but does have embedded images, it is imported as an image and says so; the copies are left next to the SVG (`name.tlaku-img1.png`…). To cut outlines you need an SVG with paths.
- **A TIFF photo does not load.** Large TIFFs and YCbCr ones now load. If the message talks about memory or a format it does not understand, save the image as PNG from your editor and import it again.
- **I imported a `.rd` or an `.egv` and it comes out shifted or mirrored.** Those formats do not store the machine's origin: Tlaku assumes home at the top left and says so. Move or mirror it. In an `.egv` the power is not included either: set it on each layer.
- **The G-code imports empty: "The G-code has no cuts with power".** All its feed moves arrive with `M3 S0`. Accept **Import with that power** in the dialog and check the result.
- **I imported G-code and it ended up in a single layer (or in many).** By default one layer per power level is made; choose **A single layer** in the options.
- **"There are no objects to export".** Exporting needs at least one visible object; check that they are not hidden and that their layers are not turned off.
- **The PDF or the printout comes out at another scale.** Print from the viewer at **actual size (100 %)** and without **fit to page**.
- **The QR code does not scan.** Increase its size or the **Error correction**, adjust the layer's engraving so the contrast is clear, and avoid **Invert** if your reader does not support it. Always test with a real reader.
- **Tlaku closed and I lost changes.** When you reopen it, it should offer to recover the copy (see [Preferences, shortcuts, and autosave](02b-preferencias-y-atajos.md)). If it does not appear, check that **Save recovery copies automatically** was on.
- **A shortcut does nothing.** You may have changed it: look at **Edit > Edit keyboard shortcuts…**. Single-letter keys do not work while you are typing in a field.

## If none of this helps

Copy the console log and use **Help > Report a Problem…**. State your firmware (the version `$I` returns), the profile's controller type, and whether you tried it on the virtual machine.

Next: [Performance with large designs](15b-rendimiento.md)
