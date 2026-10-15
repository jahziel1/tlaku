# What's New

The most important things each version of Tlaku brings, written for people who use the application. The technical detail is in the repository.

## Guided queue and job history, in the next version

- **Job queue** (**Machine > Job Queue…**): several documents (tabs or files) with copies, run one at a time. **Between jobs it always stops** and asks you to confirm the material or workpiece change and the checklist; it never moves on by itself, and every job goes through the regular start dialog. If one fails or is cancelled, the queue stops. Reorder, skip, remove and pause, with overall progress and estimated time. The phone **shows** the queue and only confirms the next job under the rules for starting (checklist on the phone and framing on the PC). See [Job queue and job history](09f-cola-e-historial-de-trabajos.md).
- A fuller **job history**: it keeps the **layers with their settings**, the material, the area, a thumbnail, the origin tab and **notes**; **search and filter** by date, machine, material and result; **Repeat with these settings** (with confirmation, sending nothing) and **export to CSV**. It lives in a file in the data folder (1000 jobs by default, configurable) and migrates the previous log by itself.

## Document tabs, in the next version

- **Several projects in the same window**, each in its own tab, with its own Undo history, selection, zoom, preview and saved state (something LightBurn does not have). A tab bar above the canvas with an unsaved-changes dot, close, **+**, drag-to-reorder and a context menu (close, close others, duplicate, move, **move to a new window**). `Ctrl/Cmd+T`, `Ctrl/Cmd+W` and `Ctrl+Tab`.
- **Opening** a project opens it in a new tab (or replaces the empty one; if it was already open, its tab is activated). The clipboard is shared between tabs.
- **Closing with changes** asks about the tab; closing the window with several unsaved shows **the list** with **Save all** / **Discard all**.
- **One recovery copy per tab**, and **Recover all** after an unexpected shutdown; at startup the tabs with a file are **reopened** (it can be turned off in Preferences).
- There is **only one machine**: the job in progress uses its own copy, the tab of origin carries a printer and the live view keeps showing it even if you switch tabs. See [Document tabs](03f-pestanas-de-documentos.md).

## Wave 6: Review design, canvas editing, importing, and machines, in the next version

Seventh batch, of polish and of gaps that remained compared with LightBurn. **None of this has been tested with a real machine**: it was validated with the virtual machine, the emulator, the simulated galvo board, and automated tests.

### Live view of the job

- **Live view** (checkbox in the **Machine** tab, off by default): a **floating window** that does not cover the design shows the whole bed and what the machine is doing, without a camera, following the real position it reports: what is already burned in dark, what is pending faint, and the head, with percentage, state, elapsed and time left. You can move it, resize it, enlarge it, close it, and **open it in a separate window** (for example, on another monitor). Turn it on and off whenever you like, also in the middle of a job. It is also on the **phone** page (with its own switch). View only. See [Sending safely](09-enviar-con-seguridad.md#live-view-of-the-job) and [Control from the phone](09e-control-desde-el-celular.md#live-view-on-the-phone).

### Part generators

- **Tools > Part generators…** draws parts you assemble: a **finger-joint box** (closed, open or with a sliding lid; outside or inside dimensions; finger width that adjusts to an odd number of cells; fit clearance separate from the kerf; name engraved on every panel), a **living hinge** (staggered straight cuts, wave or zigzag) and a **fit test piece** (thirteen slots of different widths and a tab) to calibrate kerf and clearance with your material. The box adds **dividers** with compartments and a **plinth or legs**, and there is a **tray and drawer** generator (handles, oversized front and outer case) and an involute **gears** generator (single, pair, pinion with ring gear or rack; bore, keyway and lightening). The hinge adds **brick** and **diamond** patterns; there are also loose **joints** (flat dovetail and tab and slot with clearance) and a **rounded box** with a wall that bends at the corners thanks to the hinge. Live preview, **Insert** as grouped vector objects inside the bed and without covering what is there, and **Regenerate** replaces the parts in place with the parameters saved in the project. Geometry comes out nominal: **turn on kerf in the cut layer** so the joints fit. See [Part generators](03g-generadores-de-piezas.md).

### Cost and price per piece

- **Machine > Cost and price…** (also in the **Machine** tab and in the preview bar) works out what a piece costs to make and what to sell it for if you sell on Etsy, Mercado Libre or to order: machine time (the estimate, calibrated with your jobs, or the real time of the last one), **tube or diode wear** only for the time the laser is on, electricity, **material** by area, by real area or by sheet, labor, packaging, quantity and a **price that covers the platform fee and taxes** (they are charged on the final price, not on the cost). It keeps values **per machine** and **material profiles**, uses your region's currency and format (MXN with `es-MX`), shows the formula with your numbers and lets you **copy the breakdown** as text or CSV. Nothing leaves your computer. See [Cost and price per piece](07c-costos-y-precio.md).

### Version history

- **File > Version history…** lets you **go back to how the project looked even after saving**. Every time you save, what the file held is kept first; while you edit, **automatic snapshots** are taken every few minutes (if there are changes), and **File > Save version…** stores a **named** version that is never deleted on its own. They are compressed, not repeated when nothing changed, and stored in the app data folder (or, if you prefer, next to the file in a hidden folder). Cleanup is configurable: everything from the last 24 hours, one per day for 30 days, one per week after that, with a size cap per project. The list shows thumbnail, date, kind and size; picking a version shows a **preview**, **what restoring it would change**, and you can **restore as a new tab** or **replace the document** (it stores what you had first and is a single Undo step), highlight what changes, delete the version or open the history of a file that is not open. If the disk fails, it tells you and saves the project anyway. Everything is local. See [Version history](03h-historial-de-versiones.md).

### Review design

- **Machine > Review design…** (`Ctrl/Cmd+Alt+R`) reads your design and lists **twelve kinds of findings** (almost-closed strokes, duplicate lines, pending joins, tiny fragments, extra nodes, objects outside or past the bed, unconverted text, layers with no output or hidden with objects, and images with too much or too little resolution). Each one comes with **Fix** (if there is one), **Select**, and **Ignore**; **Fix everything safe** closes, joins, and removes duplicates in a single **Undo** step.
- A **notice in the Start dialog** (with a **Look** link) when there is something serious, and the same findings in plain words in **simple mode**. See [Review design](07b-revisar-diseno.md).

### Advanced text

- **OpenType features per text**: ligatures (standard and discretionary), stylistic alternates and sets, swash, small caps, numerals (old-style, lining, tabular, proportional), fractions and kerning on or off. The panel reads the font and **offers only what it has**, with an off/on preview of each feature.
- **Variable fonts**: one slider per axis (weight, width, slant, optical size and custom axes) and the named styles; the outline is computed at the chosen position.
- **Forced uppercase**, **word gap** and **vertical alignment** (also on a path: the text can hang from the line, centre on it or sit on top). See [Text](04-texto.md#advanced-text-font-features-and-variable-fonts).

### Variable data from CSV

- **Several CSV files per project**, each with its own name: `{csv.products:Name}` picks the source and the column. The first one is the main source and rules over the rows; the others repeat from the start (or block sending) if they have fewer rows. They are saved with a path relative to the project, and a project with a single CSV opens as before.
- **Variable images and logos**: a column with the file path (absolute or relative to the CSV) replaces the marked image, fitted into its box (whole or filling and cropping) and engraved with its layer's image mode. If a file is missing, sending is blocked and says which one.
- **Variable QR and barcodes**: the content can contain `{csv:Id}` and is generated again with each row's data.
- **Preview by row**, **One copy per row** with all of the above, and **Check data**, which lists the rows with empty cells, missing images or codes without content. See [Variable text](04b-texto-variable.md).

### Canvas and editing

- **Warp on canvas** (`Shift+W`): drag the handles of a **bilinear**, **perspective**, or **4×4-point** mesh and see the result live.
- **Fillet corner by click** (`Shift+F`), with `Alt`+click to **revert** a rounded or bitten corner (the chamfer too, because the object stores the original point). See [Advanced shapes and warp](03c-formas-y-deformar.md).
- **Floating panels**: pull any tab out into a window over the canvas (double-click the tab), switch the panel's and the tools' side, and **Reset Layout**. See [Guides, tabs, start point, and project](03e-guias-puentes-y-proyecto.md).
- New preferences: **Return to Select after drawing a shape**, **Grid contrast**, default import resolution, and remembered import options. Preference changes are reflected in the other open windows.

### Import and export

- **Messages with the cause**: when a file brings nothing importable, the error says what was skipped and what to do.
- An **SVG with only images** is imported as images (extracted next to the SVG); **large TIFFs** and **YCbCr TIFFs** load.
- Ruida **`.rd`** files and K40 **`.egv`** files are imported to recover the design. It **exports to `.ai`** (really a layered PDF that Illustrator opens; it is not a native document of theirs).
- A **G-code with power 0** no longer imports empty: a dialog lets you choose the power with which to take the cuts.
- See [Import and export](03b-importar-y-exportar.md).

### Images

- **Image adjustment presets** (7 built-in and your own) and **per-image preview** without calculating the job. See [Images](06-imagenes.md).

### Machines

- Profile: **default**, **order**, and **icon**; **red pointer offset** (and **Bring the red pointer to the point**); **swap X and Y**; speeds in **mm/s**; **homing on connect**; **do not raise DTR** and **synchronous streaming** for difficult boards; **U offset** of jobs. See [Connecting your machine](08-conectar-maquina.md).
- **Warnings when calculating**: **saturated serial link** (the job asks for more lines per second than fit at that baud rate), **layer faster than the machine**, and with Ruida **more than 128 layers** (identical neighbors are merged) and the **36,000 mm/min cap**. See [Sending jobs safely](09-enviar-con-seguridad.md) and [Ruida and K40](13b-ruida-y-k40.md).
- **Feeder**: repeat a job N times with an advance between repeats (GRBL only). **Print & Cut**: images rotate with the registration.
- **Job log** (Help; now **Job history**): date, machine, document, estimated and actual time, and result, stored only on your computer.

### Camera

- **Network cameras** (`http://` URL on your local network; credentials stay on your computer and do not travel in bundles), **camera presets**, **manual backdrop adjustment**, **automatic alignment** of the head-mounted camera with marks, and a **drawn area** for the sweep. **Cameras and your art library** travel in `.tlakubundle` bundles. See [Camera](10-camara.md).

### Galvo (experimental)

- The galvo is now **driven from the app**: adding with **verification without the laser**, profile, **framing** with rectangle, outline, **convex hull**, or **loose objects**, loop counter and **live adjustment**, optional **overscan**, and **required inputs** (interlock). See [Galvo (experimental)](13c-galvo.md) and [Galvo safety](13d-galvo-seguridad.md).

### Performance

- Maps with tens of thousands of paths calculate in **1 to 1.5 s** (before, up to 30 s); photos of 60 to 100 megapixels import with less than 75 MB; the canvas handles 100,000 objects. See [Performance with large designs](15b-rendimiento.md).

### What is still missing

- Testing everything with real hardware.
- Dragging tabs already placed, per-color and per-machine defaults, and the per-layer U setting.

## Images, Print & Cut, and feeder, in the next version

- **Image presets** (Photo on wood, High-contrast logo, and more) that apply brightness, contrast, gamma, enhancement, and speck removal at once, and your own with **Save as…**. See [Images](06-imagenes.md).
- **Image preview**: see an image's dithered result before calculating the job, whole or at actual size with zoom.
- **Images rotate with the Print & Cut registration.**
- **Repeat a job with the feeder**: N repeats with the feeder's advance between them, from the Start dialog. See [Tiling and feeder](09d-tiling-y-alimentador.md). Only with GRBL machines; the built-in feeder of a DSP controller is still missing.
- **Limit:** none of this has been tested with a real machine.

## Simple mode, built-in materials, and remote control from your phone, in the next version

Sixth batch: Tlaku becomes easier for beginners, comes with starting settings for the most common materials, and lets you watch a job from your phone. **None of this has been tested with a real machine**: it was validated with the virtual machine, automated tests, and a simulated phone.

### Simple mode

- **A guided version in five steps**: open the design, choose the material, place it, review, and start. The first time, Tlaku asks **Is this your first time with a laser?**; the top bar's button switches mode whenever you like, **without touching your design**.
- **Cut, Engrave, or Ignore per color**: you choose the material and what to do with each color; Tlaku sets speed, power, and passes. With **Adjust** (more passes, less burn, lighter or darker), **Show numbers**, and **Save as my material**.
- **It asks the watts of your laser** ("How many watts is your laser?") to use the settings of the right class; if you do not know, it uses the most careful ones. Clear acrylic with a diode does not let you choose **Cut**. See [Simple mode](01b-modo-sencillo.md).

### Built-in materials

- **134 starting settings** for cutting and engraving, in the **Built-in** section of the Materials tab (the library no longer starts empty). Each one states its **source** (with a link) and its **confidence** (**Official**, **Guide**, or **Forum**).
- **Machine class** (diode, CO2, or fiber, by power) to see the settings for yours, and a fixed notice: **they are starting points, not a guarantee**. They are duplicated into **My materials** to edit them. See [Materials](11-materiales.md).

### Remote control from your phone

- **See progress and pause or stop from your phone**, by scanning a QR code in **Preferences > Remote control**; nothing to install. **Pause and stop always work**; **starting** requires the checklist on the phone and that the job be **framed on the computer after the last change**. A galvo laser is never started from the phone.
- **Off by default, only on your local network, and with a key that changes every time you open the app.** **There is no HTTPS**: use it only on a trusted network. See [Remote control from your phone](09e-control-desde-el-celular.md).
- **Limit:** Safari on iPhone and iPad does not let the page vibrate the phone when it finishes (the notice and the beep do come out).

### What is still missing

- Testing everything with real hardware and with real phones.

## Pro and galvo, in the next version

Fifth big batch: what was most requested for production work, and a first step toward galvo lasers. **None of this has been tested with a real machine**: Print & Cut and tiling were validated with the virtual machine, a simulated printed sheet, and synthetic photos, and the galvo with a simulated board that we wrote ourselves.

### Print & Cut

- **Cut right on the print.** A three-step wizard (**Machine > Print & Cut…**): add 2 to 4 registration marks (crosses or circles) at the corners of the design, print them at actual size, and measure where they ended up, **with the laser** (the arrows and "Save position") **or with the camera** (a click on the aligned image).
- **No-scaling, uniform-scale, or affine fit**, with each mark's error in view before accepting and warnings if the print came out scaled, mirrored, or stretched differently on each axis. The preview, the framing, and the job come out moved according to the registration. See [Print & Cut](09c-print-and-cut.md).
- Images **rotate** with the registration, just like vectors.

### Tiling and automatic feeder

- **Split into tiles** (**Machine > Split into tiles…**): a job larger than the bed is split into tiles with overlap, by rows or serpentine, with the plan drawn on the canvas. **LightBurn has no tiling.**
- **Alignment marks** in the overlap zones, engraved in both tiles, to position the material.
- **Sending in tiles**: you move the material between one and the next, can skip or repeat a tile, and if the app closes or the connection is cut, you **resume** where you were.
- **Automatic feeder** in the machine profile: an advance G-code with `{mm}` that tiling uses by itself between tiles when they run in a single column. See [Tiling and automatic feeder](09d-tiling-y-alimentador.md).

### Galvo (experimental)

- **Fiber, UV, and CO2 galvo lasers** with EZCAD2's LMCV4 board (USB `9588:9899`), **from the command line**: `tlaku galvo list`, `status`, `light` (framing with the red pointer, which never turns the laser on), and `mark` (with `--red-only`, `--dry-run`, and `--emu` to test without a machine). **The interface in the app arrived later, in "Wave 6".**
- **Designed not to hurt anyone**: with real hardware it requires `--i-understand-laser-safety`, uses a command whitelist, switches the source off on any error, and **has no pause**, only stop. A fiber laser is class 4 and its beam is invisible: **read [Galvo safety](13d-galvo-seguridad.md) before connecting anything.** See also [Galvo (experimental)](13c-galvo.md).
- **In the engine**: 3D slice engraving (it also works on gantry machines), cylinder and cone correction, wobble, and the sequence for split, repeated, and rotary marking. See [3D slice engraving, cylinder, and cone](12b-grabado-3d-y-cilindro.md).
- **Truly experimental**: the units for speed, frequency, power, and delays come from other open-source projects and are not verified with a real board.

### What is still missing

- Testing everything with real hardware, and the galvo's minimum test with a board without a source.
- Autofocus, and the per-layer U setting.

## Machines, in the next version

Fourth big batch: Tlaku leaves the USB cable and GRBL behind, and the machine window gains what was missed most. **None of this has been tested with a real machine**: it was validated with emulators, virtual machines, and synthetic images. Anything marked *experimental* may be wrong with a real machine: start with the laser disabled or at low power, scrap material, and the emergency stop within reach.

### Network connection and automation

- **GRBL over the network**: grblHAL with Ethernet and FluidNC over **TCP / Telnet**, and FluidNC and ESP3D over **WebSocket**. It is chosen in the profile, with a **Test connection** button that moves nothing and network errors explained in plain language. See [Network connection and automation](08b-red-y-automatizacion.md).
- **Virtual machine over the network**: the same emulator behind a local TCP socket, to see the network connection without having the machine.
- **UDP automation**: other programs on your computer can load a design, ask the status, or stop the job with text commands (`PING`, `LOAD`, `START`, `STATUS`, `STOP`). It is **off** by default, only listens on `127.0.0.1`, and `START` only opens the Start dialog unless you allow it.

### Ruida and K40 (experimental)

- **Ruida (CO2) in the app**: connect by IP, send the job (by default **it is saved in the controller's memory and does not start**; you start it from its panel), pause, resume, stop, move, frame, read the machine's information and the document list. The controller's settings are read-only. There is a **Ruida (CO2)** virtual machine. See [Ruida and K40](13b-ruida-y-k40.md).
- **K40 with an M2 Nano board**, which LightBurn does not support: now also **in the app** (machine type "K40 with M2 Nano"): direct USB connection, own home, a warning about what the board ignores before sending, an emergency-stop notice if stop is not confirmed, and the `virtual:k40` virtual machine. `tlaku k40` is still available (send, read-only status, `.egv`). It is **experimental and tested only against the emulator**. **Power is not controlled by software: it is set with the machine's knob.**

### Machine control

- **Macros** (up to 10 per profile), with buttons and shortcuts, which go through the console's safety rules.
- **Moving with Z and U**, **Focus**, **saved positions**, and **test fire** (up to 20 % and 2 s, with double confirmation). See [Advanced machine control](09b-control-avanzado.md).
- The controller's **`$$` settings** with name, description, and critical mark, editing with confirmation, and a `.tlakuset` **backup** with restore by differences.
- **Low-power framing** and **looping** until stopped.
- **Laser 2** per layer and **U axis** for moving by hand.
- **`.tlakubundle` bundles**: your machines (with macros and positions), materials, preferences, and shortcuts in a single file, with a preview before importing.
- **Tab power**: the laser can mark the tab without cutting through it, so the piece comes loose with a tap. See [Layers and cut settings](05-capas-y-ajustes.md).

### Camera

- **Fisheye**: a new lens model with automatic selection, tested with held-out views. Designed for the cameras on the lid of CO2 lasers.
- **Capture the bed image** as a document image and **trace it**.
- **Head-mounted camera**: offset calibration and bed mosaic.
- **Center finder**: finds the center of a round object and moves the head or sets the origin there. See [Camera](10-camara.md).

### Small improvements

- The **inches** from the preferences reach the status bar, the rulers, the arrow-key steps, and the Arrange and Nest parts dialogs, with fractions (`3/8"`).
- Nesting pieces have readable names, and the copies of the virtual array count in the design's boxes.
- A window's recovery copy is no longer offered in another open window (fixed by reading the code and with tests; still to be checked with real windows).

### What is still missing

- Testing everything with real hardware. Ruida does not write settings or delete or run documents; the K40 has only been tested against the emulator, with no real board.
- Inches in layers, machines, and the camera; autofocus.
- (They have already arrived, in "Wave 6": network cameras, camera presets, aligning the head-mounted camera with marks, and marking a profile as default.)
- Galvo, **Print & Cut**, and **tiling** (they arrived in the following batch: see "Pro and galvo").

## Advanced design, in the next version

Third big batch: the design tools that were missed most. It is still untested with a real machine: always check the preview and start with scrap material.

### Changing the shape of a drawing

- **Trim shapes** (`X` tool): click a stretch between two crossings to delete it.
- **Break apart subpaths**, **optimize shapes** (fewer nodes with a tolerance), **round corners** (rounded, chamfer, or bite), **cut shapes with a blade**, and **resize slots** for interlocking joints to the material's real thickness.
- **Warp** (arch, bulge, pinch, perspective, and skew, with adjustable corners and edges), **rubber band outline**, **tangent circle**, and the **boolean assistant** (`Ctrl/Cmd+B`), which shows the result of each operation before applying it.

### Copies and arrangement

- **Copy along a path**, **virtual array** (copies that are not objects, with ghosts on the canvas), **dock and move together**, and **auto-group by containment**.
- **Nesting**: arranges pieces on the bed or inside an irregularly shaped material, with rotations, mirroring, usable gaps, and a time limit, with progress, utilization, and preview; it can be stopped.

### Variable text

- **Serial numbers**, **CSV data**, **date and time** inside a text, with advance when each job finishes and **one copy per row**. See [Variable text](04b-texto-variable.md).

### Canvas and project

- **Guides** dragged from the rulers, **manual tabs** (`U`), the cut's **start point and direction** (`Q`) that the planner respects, project **notes**, **templates**, **new window**, and right-panel tabs that can be reordered.
- **Art library** with 41 drawings (arrows, frames, stars, hearts, icons, borders, and boxes with joints) and your own SVGs.

### Images

- **Crop an image with a shape** (mask), **convert vectors to bitmap**, **save the processed image**, **replace the image**, and **trace several files** at once.

### Importing LightBurn projects

- The importer now brings **perforation**, **dot mode**, **flood fill**, **overcut**, **ramp**, **pauses**, **cut-through**, and **sub-layers** (as additional operations). Several conversions were inferred from public documentation: check each layer.
- The layer editor no longer lets you type values that the engine would reject (lead-in and lead-out angles, cut-through of 0 ms) and supports the same maximum of additional operations.
- **Minimum power** is no longer shown on line layers, with an explanation: the power drop at corners is decided by the controller.

### What is still missing

- **Warp** is done from a dialog: there are no handles on the canvas or 16-point mesh yet. *(Resolved in "Wave 6".)*
- **Round corners** has no click-per-corner tool; **manual tabs** cannot be dragged (tab power arrived later, see above); there are no temporary alignment guides or floating panels. *(The click-per-corner tool and floating panels arrived in "Wave 6".)*
- **New window** has not been tested in a real window. When opened, the unsaved-work notice could show the other window's copy; the machines wave fixed it (see above), but it is still unchecked with real windows: when in doubt choose **Decide later**.
- **Tiling** and **Print & Cut** are still missing. *(They arrived in "Pro and galvo".)*

## Layers and cutting: advanced settings

Second big batch of features, designed for people coming from LightBurn who want the same control over how things are cut. It is still untested with a real machine: start with low power and scrap material.

### Cut settings (Line layers)

- **Advanced cutting** in the layer editor: **perforation** (cuts one stretch and skips another, ideal for folds in cardboard), **lead-in and lead-out** (straight or arc, so the start leaves no mark on the edge), **overcut**, **pauses** at start and end, **cut-through**, **dot mode**, and **power ramp**.
- Pauses use `G4`; the **G-code dialect** now says in which unit (seconds or milliseconds) your controller reads them.
- If two settings cannot be combined (dot mode with perforation, for example), the editor marks it and the preview warns which one was ignored.

### Fill and image

- **Flood** fill (less empty travel with many loose pieces) and fill **grouping**: all together, shape by shape, or by group.
- **Angle increment** between passes.
- Images: **send untouched** (pass-through), **edge enhance**, and **remove specks**.

### Layers

- **Additional operations (sub-layers)**: fill and then outline the same shape with a single layer.
- **Copy and paste settings** between layers, **delete a layer with its objects**, your own **defaults** for new layers, and **Beginner mode** to hide the advanced settings.
- **Cut priority** and **power scale** per object (Object properties window), with numbers on the canvas to review them.
- Parametric shapes (rectangle, ellipse, polygon, and star) **save their parameters in the project**: when you reopen it they can still be edited.

### Job order

- **Optimization settings**: respect layers, groups, and priority, inner shapes first, better start point, remove stretches that would be cut twice, and hide mechanical backlash.
- **Selection only**: preview, framing, job, and G-code with only what is selected, with the option to use its own origin.

### On the machine

- **Frame outline** (rubber band) for long or diagonal pieces.
- **Position the laser**: a click on the bed takes the head there, with the laser off.
- **Live adjustments** of speed and power during the job (GRBL controllers).
- Move window: **configurable steps**, **finish position** (stay, return to origin, or go to a point), and **go to work origin**.
- Configurable **checklist** in the Start dialog (air, focus, material, extractor, and lid), with the option to require it before sending.

### What is still missing

- A line layer's **minimum power** does not change the G-code (only on images); since the following batch the field is no longer shown on those layers.
- Framing is not repeated in a loop and the Move window has no Z, saved positions, Fire, or Focus. *(Resolved in the "Machines" batch.)*
- *(Resolved in the advanced design batch: manual tabs and the LightBurn project importer with these settings.)*

## Version 0.1.0, first public release

First version of the desktop application. It is a beta version: **it has not yet been tested with a real laser machine**, only with the emulator and the app's virtual machines. Always start with low power and scrap material.

### Design and editing

- Canvas with your machine's bed, rulers, zoom, snapping, and an exact-values bar.
- Line tools (with Bézier curves), rectangle, ellipse, polygon, star, text, measure, and node editor.
- Flip, mirror across a line, quick rotations and rotation by two points, draw order, lock, nested groups, match sizes, and the special selections (open shapes, circles by size, and more).
- Context menu with right-click, snapping to nodes, midpoints, centers, and intersections, and copy and paste of SVG between programs.
- Union, subtract, intersection, weld, offset, align, distribute, and grid and circular arrays.
- Join paths, close paths, and remove duplicates.
- Text with system fonts, also along a path, **bent**, and with **welded** letters.
- 15 single-line fonts (Hershey) included and an SHX font reader.
- QR codes, Data Matrix, and barcodes (Code 128, Code 39, EAN-13, EAN-8, UPC-A, ITF, and Codabar).
- Undo and redo with a history you can jump to.

### Import

- SVG (with real clipping), DXF, **PDF**, **AI**, **HPGL** (`.plt`), **G-code**, and PNG, JPG, BMP, **WebP**, **TIFF**, **GIF**, and **TGA** images. Each color in the file becomes a layer.
- **Import with options**: scale, DPI, unit of a DXF, page of a PDF, grouping, and target layer.
- Trace an image with a preview.
- LightBurn projects (`.lbrn` and `.lbrn2`) with their layers and settings.
- The importers warn about everything they could not bring in, instead of staying silent.

### Export and print

- Export the design (all or only the selection) to **SVG**, **DXF**, **PNG**, and vector **PDF** at 1:1 scale, and **Print** at actual size.

### Preferences and backup

- **Preferences** window (`Ctrl/Cmd+,`): units, arrow-key steps, snapping, grid, theme, autosave, and recent files.
- **Configurable keyboard shortcuts**, with conflict warnings, and `.tlakuprefs` files to take your configuration to another computer.
- **Autosave** and recovery of work after an unexpected close.

### Layers and cutting

- Line, Fill, and Image modes, with speed, power, passes, interval, angle, and overscan calculated with the machine's physics.
- Kerf compensation, tabs so the piece does not come loose, air assist, and Z axis.
- Images with several dithering methods, halftone, periodic, and sketch.
- Materials library and material test.

### Preview and cut order

- Playback of the path with estimated time and a per-layer breakdown.
- Explainable planner: each step says which rule decided it.

### Machine

- Wizard for adding your machine, with read-only detection and a catalog of models.
- Virtual machines with simulated faults, to practice without risk.
- Safety check before sending, framing, 9-point origin, "Start from", pause, and stop.
- Console that explains every alarm and every error.
- G-code dialects for GRBL, grblHAL, FluidNC, GRBL-LPC, Smoothie, Marlin, and custom G-code.
- Rotary (roller or chuck) and scan correction.
- Camera with lens calibration, alignment, and engraved marks.

### Languages

- Interface in Spanish and English, with live language switching from Preferences.

### Help

- This manual inside the app (Help menu, with search), keyboard shortcuts with `F1`, What's New, Report a Problem, and About.

### What is coming

- The design tools announced here (trim, break apart, round corners, warp, variable text, and serial numbers) arrived in the advanced design batch.
- Ruida (CO2) exists only on the command line and is experimental. Network connection, K40, and galvo are still missing. (All of this arrived in later batches: see "Machines", "Pro and galvo", and "Wave 6".)
