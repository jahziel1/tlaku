# Tlaku Manual

> Screenshots show the Spanish interface; menus are in the same place.

Tlaku is free software (MIT license) for preparing and sending jobs to laser cutters and engravers. This manual explains what the application does today. Anything that is not in the application yet is stated clearly on each page (for example, in the "What is missing or different" section of [Coming from LightBurn](14-desde-lightburn.md)); nothing is promised that is not in the code.

> **Heads up:** Tlaku is a beta. It has been tested on a real Creality Falcon2 Pro 22 W: line engraving, fills and images, pause, stop, resuming after a power cut, live view, and control from your phone. **Cutting at high power has not been tested yet**, and other machines and controllers (Ruida, K40, galvo, network, rotary, a calibrated camera…) have only been tested with emulators and with the app's own virtual machines. The details are in [Supported controllers](13-controladoras.md). Start with low power, scrap material, and supervision. **If you are going to use a galvo laser (fiber), read [Galvo safety](13d-galvo-seguridad.md) first**: its beam is invisible.

## Getting started

- [Getting started](01-primeros-pasos.md): install, import a design, check the layers, see the preview, test with the virtual machine and save.
- [Simple mode](01b-modo-sencillo.md): the guided five-step version for people who have never used a laser: choose the material, cut, engrave or ignore each color, adjust with plain words and review before starting.
- [The interface](02-interfaz.md): top bar, tools, canvas and guides, rearrangeable panels, color palette, status bar and keyboard shortcuts.
- [Preferences, shortcuts and autosave](02b-preferencias-y-atajos.md): the Preferences window (including inches, grid contrast, returning to Select, import options, UDP automation and remote control), changing shortcuts, moving your settings to another computer, checking for updates and recovering your work after an unexpected shutdown.

## Designing

- [Drawing and editing](03-dibujar-y-editar.md): drawing tools (curves, polygon and star), parametric shapes, cut priority and power scale per object, measuring, selection, flip and rotate, order and locking, nodes, grouping, the context menu, the Arrange menu and undo; the following pages cover the advanced operations.
- [Importing and exporting](03b-importar-y-exportar.md): the formats you can import (SVG, DXF, PDF, AI, HPGL, G-code, Ruida `.rd`, K40 `.egv` and images), their options and messages, and exporting to SVG, DXF, PNG, PDF and AI or printing at 1:1 scale.
- [Advanced shapes and warping](03c-formas-y-deformar.md): trim shapes, split subpaths, optimize nodes, round corners (also with one click), cut with a knife, resize slots for joints, rubber-band outline, tangent circle, warp (also with handles on the canvas) and the Boolean assistant.
- [Copies, virtual arrays and nesting](03d-copias-y-nesting.md): copy along a path, virtual array, dock and move together, automatic grouping and arranging parts on the sheet (nesting, with several sheets, offcuts and grain).
- [Guides, tabs, start point and project](03e-guias-puentes-y-proyecto.md): guides from the rulers, manual tabs, start point and cut direction, notes, templates, new window, tabs the way you like them and floating panels, and the art library.
- [Document tabs](03f-pestanas-de-documentos.md): several projects in the same window, each with its own history, selection, zoom and preview; opening, closing with changes, moving to another window, per-tab recovery and session, and how they coexist with the shared machine and the job in progress.
- [Version history](03h-historial-de-versiones.md): go back to how the project looked even after saving: versions on save, automatic and named; preview, comparison and restore (as a new tab or by replacing); where they are stored, how much is kept and what happens if the disk fails.
- [Text](04-texto.md): create text, choose a font (including single-line fonts), curve it, weld it, put it on a path, convert it to paths and insert QR and barcodes.
- [Variable text](04b-texto-variable.md): serial numbers, data from a CSV, date and time inside a text, how they advance after each job and one copy per CSV row.
- [Layers and cut settings](05-capas-y-ajustes.md): Line, Fill and Image modes, speed, power, passes, tabs, advanced cutting (perforation, lead-in and lead-out, overcut, pauses, dot mode, ramp), additional operations, defaults (general, per color and per machine), advanced settings and execution order.
- [Images](06-imagenes.md): import photos, choose the dithering, adjust tones, send without dithering, sharpen edges, remove specks, crop with a shape, image-adjustment presets and their preview, convert vectors to bitmap, save the processed image and trace one or several images.
- [Part generators](03g-generadores-de-piezas.md): finger-joint boxes (closed, open, sliding lid), living hinges (straight, wave, zigzag) and the fit test piece to calibrate kerf and clearance; outside or inside dimensions, engraved labels and "Regenerate" with the parameters saved in the project.

## Preparing the job

- [Preview and cut order](07-vista-previa.md): play back the path, read the results, the optimization settings and understand why things were cut in that order.
- [Review design](07b-revisar-diseno.md): the twelve types of finding (almost-closed paths, duplicate lines, objects outside the bed...), fix, select or ignore, "Fix everything safe" and the warning when starting.
- [Cost and price per piece](07c-costos-y-precio.md): for people who sell: what a piece costs to make (machine, tube or diode wear, electricity, material, labor, packaging) and what to sell it for while covering the platform fee and taxes; machine and material profiles, currency, and copying the breakdown or saving it as CSV.

## The machine

- [Connecting your machine](08-conectar-maquina.md): the Add machine wizard, the port, the profile (with default profile, icon, pointer offset, X/Y swap, mm/s, homing on connect, DTR and synchronous sending) and the virtual machine.
- [Network connection and automation](08b-red-y-automatizacion.md): GRBL over TCP and WebSocket, testing the connection, the virtual machine over the network and UDP automation (protocol and safety).
- [Sending jobs safely](09-enviar-con-seguridad.md): send only the selection, job origin, move the head and position the laser, framing (also by outline), the Start dialog with its checklist and warnings (slow link, layer faster than the machine), live adjustments, pause, stop, continuing an interrupted job (power cut, app closed, connection lost), the job history and exporting G-code.
- [Advanced machine control](09b-control-avanzado.md): macros, move with Z and U, focus, saved positions, test fire, `$$` settings with backup, low-power and looping framing, laser 2 and `.tlakubundle` bundles.
- [Print & Cut](09c-print-and-cut.md): print with registration marks and cut right on top of what was printed, step by step with the laser or with the camera, images also rotate with the registration, and what the residual error means.
- [Tiling and auto feeder](09d-tiling-y-alimentador.md): split a job larger than the bed into sections with overlap and alignment marks, send them one at a time, resume, use an automatic feeder (something LightBurn does not have) and repeat a job N times with an advance between repetitions.
- [Job queue and job history](09f-cola-e-historial-de-trabajos.md): the guided queue of several documents with copies (it always stops between jobs and asks you to confirm; it never moves on by itself), the history with each job's layers and settings, notes, search and filters, repeating with the same settings and exporting to CSV.
- [Control from your phone](09e-control-desde-el-celular.md): watch progress and pause or stop a job from your phone's browser with a QR code; what you can and cannot do, why there is no HTTPS and why the key changes on restart.
- [Camera](10-camara.md): calibrate the lens (including fisheye), align the design with the material, network cameras and setups, manual background adjustment, capture and trace the bed, head camera (with automatic alignment and a drawn zone) and center finder.
- [Materials](11-materiales.md): the built-in library (with its source, its confidence level and machine classes), your own library, the material test, the community settings browser and exchanging `.tlakumat` files.
- [Rotary and Z axis](12-rotativo-y-eje-z.md): engrave on cylinders and work with a motorized table.
- [3D slice engraving, cylinder and cone](12b-grabado-3d-y-cilindro.md): relief from a grayscale map over several passes, and the correction for galvo marking on tapered mugs and tumblers (currently in the engine).
- [Supported controllers](13-controladoras.md): which controllers work, what was tested in the emulator and what remains to be validated with hardware, and the G-code dialects.
- [Ruida and K40](13b-ruida-y-k40.md): the two experimental families: what they do, what they do not, the limits of 128 layers and 36,000 mm/min, why Ruida only saves by default, the K40 test plan and why its power is set with the knob.
- [Galvo (experimental)](13c-galvo.md): fiber, UV and CO2 galvo lasers with LMCV4 cards (EZCAD2): what `tlaku galvo` does, how to test it without a machine using the simulated card and what it does not do. It is operated from the app (setup with a no-laser check, framing with convex hull, overscan and required inputs) and from the command line.
- [Galvo safety](13d-galvo-seguridad.md): **read it before connecting a card**: invisible class 4 beam, goggles with the right optical density, enclosure, the `--red-only` mode, the test plan with the card without a source and why there is no pause.

## Help

- [Coming from LightBurn](14-desde-lightburn.md): name and shortcut equivalents (including the advanced design tools, nesting and variable text, Print & Cut and tiling), what exists, what is missing, how to bring in a design and what the LightBurn project importer keeps and what it warns about.
- [Troubleshooting](15-solucion-de-problemas.md): GRBL alarms and errors, serial ports, drivers, network errors, a Ruida that does not respond, a K40 that does not show up over USB, warnings when calculating a job and what to do when the laser does not fire.
- [Performance with large designs](15b-rendimiento.md): what to expect with 100,000 objects, heavy bitmaps, photos of tens of megapixels and jobs the USB cable cannot keep up with.
- [Glossary](16-glosario.md): laser and Tlaku terms, from kerf to overscan.
- [What's New](novedades.md): what each version brings.

---

Next: [Getting started](01-primeros-pasos.md)
