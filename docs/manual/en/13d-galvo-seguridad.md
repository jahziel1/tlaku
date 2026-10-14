# Galvo safety

This is the most important page of everything about the galvo. Read all of it **before** connecting a board, even if you only plan to test with the emulator, and come back to it every time you change laser source or setup.

> **What this is:** an industrial-grade **fiber** (or UV, or CO2 galvo) laser. It is not a 5 W diode behind an acrylic lid. It can **permanently** damage the eyesight of anyone in the room, start fires, and burn through the workpiece, the fixture, and the head itself within milliseconds. Tlaku's galvo support is **experimental and has not been tested with a real board**: software cannot guarantee your safety. The protections on this page are physical, and they are yours.

> **Where it is driven from:** the galvo is driven from the app (see [Galvo (experimental)](13c-galvo.md), "The galvo in the app") and from the command line (`tlaku galvo`). Everything this page says applies equally to both.

## The hazards, one by one

### 1. The beam is invisible and class 4

A typical fiber laser emits at 1064 nm, **near-infrared that cannot be seen**. There is no bright dot to warn you: the beam can be pointing at your face and you would not know. It is a **class 4** laser: even a **diffuse reflection** (the beam bouncing off polished metal, glass, a wall) can damage the eye. UV and CO2 galvo have their own risks and wavelengths; the principle is the same.

- **Never look at the beam**, direct or reflected, not even "for a moment" to check whether it comes out.
- **Never put a hand, a piece of paper, or a tool** in the path to "see" whether it marks.
- The **red pointer** is a separate light: it shows **where** the beam will go, but it **does not mean the laser is off**. Whether the pointer is on or off tells you nothing about the state of the source.

### 2. Glasses with the correct wavelength and optical density

Safety glasses **must match your laser**: they are labeled with the **wavelength** they filter and their **optical density (OD)**.

- Glasses for CO2 (10,600 nm) **do not protect** against a fiber laser (1064 nm), and neither do blue-diode glasses against UV. Check the label against your source's datasheet.
- The **minimum OD** you need depends on your laser's power, the pulse duration, and the distance: it is set by the standards (ANSI Z136.1, IEC 60825-1, EN 207) and by your source's manufacturer or a laser safety officer. **Tlaku cannot calculate it for you and does not make up a number**: ask whoever sold you the source and insist on a label that covers it.
- Glasses **get scratched, crack, and expire**. Damaged glasses are not glasses.
- **Everyone** in the room while the source is connected needs their own, even if they do not touch the machine.

### 3. Closed enclosure, with interlock and shutter

- Work with the head inside a **closed enclosure** that blocks the beam, with an **interlocked lid** (one that turns the laser off when opened).
- Use a **shutter**: the board's own manual recommends it. It is a physical barrier that cuts the beam even if the software or the board fail.
- Keep the **emergency stop** within reach, an arm's length away, and without having to pass in front of the beam.
- Have a suitable **fire extinguisher** nearby and do not leave the machine unattended while it marks.

### 4. Software is not the last safeguard

There are reports from users of other galvo programs in which the mirror stops but the beam **stays on**, or the laser is left on when a job ends. A beam fixed on one spot **burns through** the workpiece, the fixture, or the head, and it is the typical way a galvo starts a fire. If the USB cable drops in the middle of a job, **what the board does without the computer telling it anything cannot be guaranteed by software**: it has to be measured on your machine (see the test plan, below).

### 5. Parameters that damage the source

A fiber source has start-up and pulse rules. With a frequency, a pulse width, or a power outside what your source supports, the **laser module can be damaged** (back-reflection). That is why Tlaku forces you to enter your source's **frequency range** (`--freq-range`), the allowed **pulse widths** (`--pulse-widths`), and a **maximum power** (`--max-power`), and **rejects** everything outside them, before connecting. These values come from the manual of **your** source, not from Tlaku's defaults.

## What Tlaku does to protect you (and its limits)

This is what the code guarantees, **verified against the simulated board** and not a real one:

- **It rejects the unknown.** It only accepts the `9588:9899` board. Clones and boards without firmware are rejected with the reason.
- **It asks you to confirm.** With real hardware, everything except `tlaku galvo list` requires the `--i-understand-laser-safety` flag; without it the USB is not even opened. In addition, `mark` asks whether you have your glasses on, the enclosure closed, and the shutter and emergency stop ready.
- **Command whitelist.** Only the commands Tlaku needs are sent. The EEPROM, firmware, correction table, and board configuration are **never** written, even if something asked for it.
- **It checks every list** before sending it: that it starts and ends properly, that there is no mark before speed, delays, frequency, and power are set, and that power, frequency, and pulse width are within the profile.
- **The laser starts disarmed.** Each marking starts with the source disarmed and arms it just before executing the list. **Framing with the red pointer never enables the laser.**
- **Guaranteed switch-off.** However the job ends (it finishes, fails, or you cancel with **Ctrl+C**), Tlaku runs a **full switch-off sequence and repeats it if it fails**, and repeats it as well if the session closes uncleanly. If it cannot confirm that it switched the source off, **it tells you** ("There was NO confirmation: it may still be enabled"); in that case use the emergency stop and open the enclosure. If what drops is the USB cable, the switch-off command **has no way to arrive** (see hazard 4).
- **Verification without the laser when adding.** In the app, the **Add machine** wizard ends with **Verify without the laser**: it reads the board and checks the profile without moving the mirrors or turning on the pointer or the source (with a real USB board, it asks for the safety confirmation first). See [Galvo (experimental)](13c-galvo.md).
- **Confirmation on every connection.** In the app, connecting with a real USB board opens **Laser safety before connecting** and requires the three checkboxes (glasses, enclosure, emergency stop) **every time**.
- **Software interlock (optional).** In the app, the profile has **Input interlock** (and on the command line, `--require-input MASK=VALUE`) that requires, **before each job and each frame**, that the board's inputs satisfy `(inputs & MASK) == VALUE`. It is useful if your machine's door or emergency stop reaches an input on the board. **But the bits of the emergency stop and of the source's alarms are not documented**: you have to measure them yourself, and until you set them, Tlaku cannot check the interlock (and it warns you).

**What it cannot do:** prevent damage from a disconnected USB, a wrong unit value, a badly configured source, or a person without glasses. Tlaku does not replace physical protection.

## The `--red-only` mode

`tlaku galvo mark --red-only` traces **the whole job**, with the same positions, speeds, and delays the marking would have, but with the **laser disabled** and only the **red pointer** on. It is for seeing the real path over the workpiece before firing:

```
tlaku galvo mark pieza.svg --red-only --frequency 30 --i-understand-laser-safety
```

It is a good test, but **it is not a safety guarantee**:

- It depends on the red pointer and the laser output bit being what Tlaku assumes (bit 8 for the pointer and bit 0 for the laser, from other projects, **not verified on a board**).
- If your head has no red pointer, you will see nothing.
- A `--red-only` does not ask the usual confirmation because the laser is not armed; it **still needs** the safety flag with real hardware.

And the sibling command `tlaku galvo light` draws the rectangle (or the outline with `--contour`) with the red pointer in a loop until Ctrl+C, never enabling the laser.

## Why there is no pause

In other programs' interfaces there is a **Pause** button. In Tlaku, with the galvo, it **does not exist**: you can only **stop**.

The reason is safety: in galvo users' reports, **pausing stops the mirrors but the beam may stay on** (sometimes at 95 % power). A still mirror with the beam on burns through the workpiece at one point and can start a fire. Stopping, by contrast, runs the full switch-off sequence. If you need to halt, stop the job (**Ctrl+C**), or use the emergency stop.

## Test plan with the board and no source

Before connecting the laser source, do this test with **the board and the head powered and the laser source disconnected** (or with the source's DB25 cable disconnected). It is the plan the project uses for the first validation with hardware (`docs/hardware/galvo.md`, section 9). Ideally you have an oscilloscope or a logic analyzer to measure the signals: without one, the test is still useful for the movement, but not for checking that the laser stays off.

1. **Setup.** The LMCV4-FIBER-M board with the galvo head powered. **Turn on the board and the head's supply first, before the laser**: otherwise the beam can deflect uncontrollably (warning from the board's manual). The oscilloscope on the head's signals (clock and sync) and, on the laser connector, on the oscillator, amplifier, frequency, and red pointer signals.
2. **Identify.** `tlaku galvo list` and then `tlaku galvo status --i-understand-laser-safety`: the `9588:9899` board should appear, with its serial number and version, and the status "idle". Check that **nothing moved**.
3. **Move without laser.** `tlaku galvo light design.svg --i-understand-laser-safety --loops 3`: the rectangle of a 20 mm square should be visible (with the red pointer, or by measuring the head's signals) with its **center** at the center of the field and the right size. Compare it with your lens's field.
4. **Check that the laser stays off.** With the oscilloscope, during and after `tlaku galvo mark design.svg --red-only --frequency 30 --i-understand-laser-safety`, the source's signals (oscillator, amplifier, frequency) must **remain at rest**. Also try **cancelling with Ctrl+C halfway** and check that they stay off. And, **most important**, **disconnect the USB cable in the middle of a job** and see whether the signals stay off: no software source can guarantee it, it has to be measured on your board.
5. **Measure the frequency and the delays.** The frequency generated on the frequency signal against the one requested with `--frequency`, and the on and off delays against what was requested (resolution, sign, unit). This is what checks that the **unit conversions** are correct, and it is the biggest unknown in the whole support.

Only if all of the above goes well, and **with the source connected**:

6. **Minimum power, closed enclosure, glasses with the correct OD, and shutter**, on a material that does not matter. Verify a **point**, a **line**, and a **square**. Compare with the same job done in EZCAD2 and measure the field distortion with an engraved grid.
7. **Report it.** A real result, good or bad, is the most valuable thing you can contribute: **Help > Report a Problem…** with the commands' output, the board, the source, and the lens.

Until someone completes these tests with a real board, the galvo will stay marked **experimental**.

## Summary in ten lines

1. Fiber = class 4, invisible: **never look at the beam, ever**.
2. Glasses with your laser's **wavelength** and **OD**; everyone wears them.
3. **Closed enclosure, interlock, shutter, and emergency stop** within reach.
4. No pause: only **Stop** (Ctrl+C) and the emergency stop.
5. First `--emu` and `--dry-run`; then the board **without the source**; then `--red-only`; then the source at minimum power.
6. The frequency, pulse, and power ranges are those of **your source**.
7. `--i-understand-laser-safety` is a confirmation that you read this, not a formality.
8. Do not use `--yes` in your first tests.
9. None of this has been tested with a real board.
10. When in doubt, switch off the source and open the enclosure.

Back to: [Galvo (experimental)](13c-galvo.md). Next: [Coming from LightBurn](14-desde-lightburn.md)
