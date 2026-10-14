# Glossary

Laser and Tlaku terms, in alphabetical order, explained in a few lines.

## A

- **Additional operations (sub-layers)**: extra settings of a layer that run in order on the same objects after the main one, for example filling and then cutting the outline.
- **Air assist**: a jet of air on the cutting point; it puts out flames and clears smoke. In G-code it is turned on with `M8` and off with `M9`. It is enabled per layer.
- **Alarm (`ALARM:N`)**: a state in which the machine locks itself for safety (limit, reset, failed homing). You get out of it by clearing the alarm or homing. See [Troubleshooting](15-solucion-de-problemas.md).
- **Angle increment**: degrees added to the scan angle on each pass of a fill or an image.
- **Automatic feeder**: a belt or rollers that advance the material by themselves. Tlaku uses it between tiling's tiles when they run in a single column. See [Tiling and automatic feeder](09d-tiling-y-alimentador.md).
- **Autosave**: a recovery copy that Tlaku saves every minute (if there are changes) to offer you if the application closes abruptly. It does not replace saving. See [Preferences, shortcuts, and autosave](02b-preferencias-y-atajos.md).

## B

- **Baud rate**: the speed of the USB serial cable connection. 115200 on almost all GRBL machines; 250000 on Marlin.
- **Bed**: the machine's work area, in mm. Equivalent to `$130` and `$131` in GRBL.
- **Beginner mode**: another name for [simple mode](01b-modo-sencillo.md), the guided version that hides menus, tools and advanced settings without deleting or disabling them.
- **Bend**: the degrees a text spans along an arc. Positive arches it upward, negative downward.
- **Bidirectional**: a fill that engraves both going and returning. If the lines come out misaligned, correct it with the profile's scan correction table.
- **Blade**: the closed shape with which **Cut shapes** splits the others.
- **Boolean assistant**: a dialog that shows the result of each operation (union, A minus B, intersection, exclusive OR, and weld) before applying it (`Ctrl/Cmd+B`).
- **Bézier**: a curve defined by nodes and handles. The **Draw Line** tool creates it with click and drag; **Edit Nodes** fine-tunes it.

## C

- **Console**: a tab where you see what is sent to the machine and what it answers, and where you can type commands.
- **Containment (group by)**: form a group out of each piece and the holes and engravings that fall inside its outline.
- **Convex hull**: the minimum convex polygon that wraps a set of shapes, like a rubber band stretched around them. It is the framing of **Frame outline** and one of the galvo's framing shapes. See [Galvo (experimental)](13c-galvo.md).
- **Cut priority**: a number given to an object to cut it before or after others on its layer (lower, earlier).
- **Cut-through**: starting each closed contour with extra power for a few milliseconds, to pierce thick material without leaving a burned spot. See [Layers and cut settings](05-capas-y-ajustes.md).

## D

- **Defaults**: the settings new layers start with; they are saved from a layer's editor.
- **Dithering**: a trick for engraving photos with a laser that can only turn on or off: it simulates gray tones with dots. There are several algorithms (Threshold, Floyd-Steinberg, Jarvis, Stucki, among others).
- **Dock**: stick pieces together by sliding them until they touch a reference shape. See [Copies, virtual array, and nesting](03d-copias-y-nesting.md).
- **Dot mode**: instead of a continuous line, the laser fires a dot every certain distance.
- **DPI / LPI**: the density of the engraving's lines. The smaller the **interval** between lines, the higher the DPI. Tlaku shows the equivalent when you edit the interval (for example "0.1 mm = 254 DPI"). LPI is lines per inch.
- **DTR**: a serial port signal that many Arduino boards use to reset when the port is opened. The profile can open it without raising it (**Open the serial port without raising DTR**). See [Connecting your machine](08-conectar-maquina.md).

## F

- **Finding**: something **Review design** found that is worth a look (an almost-closed shape, duplicate lines, an object off the bed...). It can be fixed, selected, or ignored. See [Review design](07b-revisar-diseno.md).
- **Finish position**: where the head is left when a job finishes successfully: stay, return to origin, or go to a point.
- **Flip**: mirror the selection left to right (`Alt+H`) or top to bottom (`Alt+V`). **Mirror Across a Line** creates a mirrored copy across a line you mark.
- **Flood fill**: a fill that travels each region of the drawing separately without crossing the empty spaces.
- **FluidNC**: GRBL-compatible firmware for ESP32 boards, configured with files instead of `$` settings.
- **Frame**: trace the job's rectangle with the head, with the laser off, to check where it will land. It is the **Frame** button.
- **Frame outline**: "rubber band" framing: with the laser off it traces the job's convex hull instead of the rectangle. See [Sending jobs safely](09-enviar-con-seguridad.md).

## G

- **Galvo**: a laser that steers the beam with two rotating mirrors (fiber, UV, or CO2 galvo) and marks at high speed within a fixed field, with no overscan by default. In Tlaku it is experimental and is driven from the app and from the command line. See [Galvo (experimental)](13c-galvo.md).
- **G-code**: the command language that machines understand (`G0` rapid move, `G1` engraving move, `M3`/`M4` turn the laser on). Tlaku generates it from your design; you can see it with **Export G-code…**.
- **G-code dialect**: the way a firmware expects commands (how the laser is turned on, the range of `S`, the air). It changes between GRBL, Marlin, and Smoothieware. See [Controllers](13-controladoras.md).
- **GRBL**: the most common firmware on diode lasers. Tlaku speaks it over USB. There are variants: **grblHAL**, **FluidNC**, and **GRBL-LPC**.
- **Guides**: reference lines that you drag from the rulers and that attract objects; they are not engraved. They are not the **Guides** layer in the palette.

## H

- **Hershey**: a family of freely usable single-line fonts, 15 of which Tlaku includes.
- **Homing (`$H`)**: the machine looks for its limit switches to know where it is. It only applies to machines that have them and with `$22=1`. Before doing it, clear the area.
- **HPGL**: the language of plotters and vinyl cutters (`.plt`, `.hpgl`). Tlaku imports it as strokes, one layer per pen. See [Import and export](03b-importar-y-exportar.md).

## I

- **Input interlock** (galvo): require that a board input (lid, emergency stop) have a certain state before each job and each frame. See [Galvo (experimental)](13c-galvo.md).
- **Interval**: the distance between two lines of a fill or image, in mm. A smaller interval gives more detail and more time.

## J

- **Jog**: move the head by hand from the app (buttons or `Ctrl/Cmd+Alt+arrows`).

## K

- **K40**: a cheap Chinese CO2 laser, usually with an **M2 Nano** board that LightBurn does not support. In Tlaku it is experimental and the power is set with the machine's knob, not by software. See [Ruida and K40](13b-ruida-y-k40.md).
- **Kerf**: the width of material the laser consumes when cutting. It is compensated so that the final piece comes out to size.

## L

- **Laser mode (`$32=1`)**: a GRBL setting that turns the beam on and off according to the movement. Without it, `M4` does not work well.
- **Layer**: a group of objects of the same color that share cut settings (mode, speed, power, passes...). Tlaku runs the layers in the order of their list.
- **Lead-in / lead-out**: a stretch with which the laser starts or ends a contour from outside the piece, so that the start mark lands in the waste and not on the edge.
- **Limit switch**: a switch that detects that the head reached the edge. It makes homing possible.
- **Live adjustments**: change the speed or power (from 10 % to 200 %) while the job runs, without recalculating the G-code. Only on GRBL-family controllers. See [Sending jobs safely](09-enviar-con-seguridad.md).

## M

- **M3 / M4**: commands that turn the laser on. `M3` gives **constant power**; `M4` gives **dynamic power**, which drops when braking at corners and avoids burning them. M4 needs GRBL 1.1f or later with `$32=1`.
- **Macro**: a button that sends the machine lines of G-code stored in the profile. It goes through the console's safety rules. See [Advanced machine control](09b-control-avanzado.md).
- **Magic (Ruida)**: an obfuscation byte that each Ruida controller model uses in its packets; it is detected automatically or set by hand.
- **Mask (of an image)**: a vector shape that crops an image so that only what falls inside is engraved.
- **Max S (`$30`)**: the value of `S` that equals 100 % power. It must match in your profile and in the machine (1000, 255, or 1 depending on the firmware).
- **Minimum / maximum power**: in an image, the power of the lightest and of the darkest dots.

## N

- **Nesting**: automatic arrangement of pieces to waste as little of the sheet or of an irregular material as possible. See [Copies, virtual array, and nesting](03d-copias-y-nesting.md).
- **Notes (project)**: a notebook saved with the project and not sent to the machine (`Ctrl/Cmd+7`).

## O

- **OD (optical density)**: how much a laser safety glass attenuates at a given wavelength. You need the OD that your laser requires, labeled together with the wavelength on the glasses themselves. See [Galvo safety](13d-galvo-seguridad.md).
- **Offset**: a copy of an outline enlarged or reduced by a fixed distance.
- **Offset fill**: a way of filling by following the silhouette in rings, instead of scan lines.
- **Optimization (settings)**: options that change the job's order and path: respect layers, groups, and priority, remove repeated stretches, hide mechanical backlash. See [Preview and cut order](07-vista-previa.md).
- **Origin**: the starting point of the coordinates. The **machine origin** is the corner where its X0 Y0 is; the **user origin** is a point that you save with **Set origin here**.
- **Overcut**: a stretch that the laser keeps cutting past the point where a contour closes.
- **Overscan**: an extra stretch that the head travels outside the edge of the fill, with the laser off, so it reaches even speed before engraving.

## P

- **Pass**: each time the laser travels the same path. Several passes cut deeper.
- **Pass-through (send untouched)**: the image is sent as is, with no dithering or tone adjustments; each dot already is the power.
- **Path**: a vector line or curve. Text is converted to paths so its nodes can be edited.
- **Perforation**: intermittent cutting, with cut stretches and skipped stretches, for fold lines or pieces that must stay joined.
- **Planner**: the part of the program that decides in what order each step runs. In the preview you can see which rule decided each one.
- **Position the laser**: a mode in which a click on the bed on the canvas sends the head to that point, with the laser off.
- **Power ramp**: the power rises gradually at the start of a stroke and falls at the end, instead of coming in all at once.
- **Power scale**: the percentage of the layer's power that a particular object uses (0 to 100 %). It is set in **Object properties**.
- **Preferences (`.tlakuprefs`)**: your settings and keyboard shortcuts; they are exported to a `.tlakuprefs` file to take them to another computer. See [Preferences, shortcuts, and autosave](02b-preferencias-y-atajos.md).
- **Preview**: a simulation of the job with the estimated time, before sending it.
- **Print & Cut**: print a design with registration marks and have the laser cut right on the print, by measuring where the marks ended up. See [Print & Cut](09c-print-and-cut.md).

## Q

- **QR code / Data Matrix / barcode**: a code that Tlaku generates as vector shapes ready to engrave (**Tools > Insert code…**). See [Text](04-texto.md).

## R

- **Raster / vector**: raster is dot-by-dot engraving (images, fills); vector is following a stroke (cuts and lines).
- **Residual error (Print & Cut)**: what is left over, in mm, after fitting the marks: the distance between where each mark should be with the chosen transformation and where you measured it. It measures the consistency of your measurements, not the cut's accuracy. With two marks and uniform scale it is always almost zero. See [Print & Cut](09c-print-and-cut.md).
- **Rotary**: an accessory that turns a cylindrical piece, normally instead of the Y axis. See [Rotary and Z axis](12-rotativo-y-eje-z.md).
- **Rubber band (outline)**: a closed shape that wraps the selection like a stretched rubber band; convex or concave. It is not the same as **Frame outline**, which moves the head.
- **Ruida**: the controller of many CO2 lasers. In Tlaku it is experimental, over the network (UDP), and by default the job is only saved to its memory. See [Ruida and K40](13b-ruida-y-k40.md).

## S

- **Selection only**: an option so that the preview, framing, the job, and the G-code include only what is selected.
- **Settings bundle (`.tlakubundle`)**: a file with your machines, materials, preferences, and shortcuts, to take your configuration to another computer. See [Advanced machine control](09b-control-avanzado.md).
- **SHX**: AutoCAD's single-line font format. Tlaku imports the ones you already have (**Tools > Import SHX font…**).
- **Single-line font**: a single-stroke font (Hershey or SHX) that draws the center of each letter instead of its outline. It is engraved as a line. See [Text](04-texto.md).
- **Soft limit**: a virtual limit that prevents moving outside the bed; when exceeded, `ALARM:2` or `error:10` is raised.
- **Start from**: where the job starts from: **Absolute coordinates**, **Current position**, or **User origin**.
- **Start point**: the place where the cut of a closed contour begins; it can be fixed by hand with the `Q` tool, together with the direction.
- **State (`<Idle|...>`)**: the report the machine sends when asked with `?`: idle, running, hold, alarm, and its position.
- **SVG / DXF / PDF**: open vector drawing formats. Tlaku imports them and also exports your design to them (plus PNG). See [Import and export](03b-importar-y-exportar.md).

## T

- **Tabs**: pieces left uncut so the piece does not fall or move before it is finished, distributed automatically by the layer or placed by hand with the `U` tool. With **Tab power (%)** the laser can mark them without cutting through.
- **TCP / Telnet and WebSocket**: the two ways of connecting a GRBL machine over the network (port 23 and `ws://`). See [Network connection and automation](08b-red-y-automatizacion.md).
- **Template**: design, layers, and settings saved as a starting point for new projects (**File > Save as template…**).
- **Tile**: each of the parts of a job split by tiling.
- **Tiling (split into tiles)**: split a job larger than the bed into tiles with overlap and send them one at a time. See [Tiling and automatic feeder](09d-tiling-y-alimentador.md).
- **Trace**: convert an image into strokes.
- **Trim**: delete the stretch of a stroke between two crossings, with the `X` tool.

## U

- **UDP automation**: an optional local server (off by default) with which other programs on your computer send Tlaku text commands such as `LOAD` or `STATUS`. See [Network connection and automation](08b-red-y-automatizacion.md).

## V

- **Variable text**: text with braces (`{serie}`, `{csv:Nombre}`, `{fecha}`) that is replaced by values when the job is sent. See [Variable text](04b-texto-variable.md).
- **Virtual array**: an object that is cut or engraved several times without the copies existing as objects; the canvas shows them as ghosts. See [Copies, virtual array, and nesting](03d-copias-y-nesting.md).
- **Virtual machine**: an emulator inside the app for testing without hardware (`virtual:...` port). It can simulate faults. Nothing is engraved.

## W

- **Warp**: fit a drawing into an envelope of four corners and four curved edges to arch it, bulge it, or give it perspective.
- **Welded text**: text whose letters that touch or overlap are fused into a single outline (**Weld letters** checkbox).

## Z

- **Z axis**: the height of the head or of the bed. Some layers can move it.

## Console symbols

- **`$$`**: lists the machine's settings.
- **`$I`**: firmware version and options.
- **`$X`**: clears an alarm without homing.
- **`ok`**: the machine accepted the line. **`error:N`**: it rejected it.

Next: [Index](00-indice.md)
