# Coming from LightBurn

If you already use LightBurn, here is what things are called in Tlaku, which shortcuts match, what is still missing, and how to move a design from one program to the other.

Tlaku is an independent program, built from scratch without using LightBurn's code or text. It resembles it in idea (layers by color, Line, Fill, and Image modes, preview, console), but it is not a clone: some things are named differently and several features do not exist yet. The LightBurn names on this page are the ones its interface uses according to its public documentation; if your version calls them something else, **verify it in your version**.

## Name equivalents

| In LightBurn | In Tlaku |
|---|---|
| Cuts / Layers (panel) | **Layers** tab of the right-hand panel |
| Cut Settings Editor | Layer settings: double-click or Enter on the layer, or `Alt+Enter` |
| Line / Fill / Image | **Line**, **Fill**, **Image** modes |
| Offset Fill | **Offset fill** option of a fill layer |
| Flood Fill | **Flood fill** option of the **Fill** selector (**Fill** section of a fill layer's editor) |
| Fill Grouping | **Grouping** selector: **All together**, **Shape by shape**, **By group** |
| Angle Increment | **Angle increment** (**Scan** group, with 2 or more passes) |
| Perforation Mode | **Perforation (cut and skip)** in **Advanced cutting**: **Cut length** and **Skip length** |
| Lead In / Lead Out | **Lead-in to the contour** and **Lead-out from the contour** (**Straight** or **Arc**) |
| Overcut | **Overcut** |
| Start/End Pause, Cut Through | **Pause at start**, **Pause at end**, and **Cut-through** |
| Dot Mode | **Dot mode** |
| Ramp Length | **Power ramp** |
| Sub-Layers | **Additional operations** (up to 11 more besides the main one; they have no tabs or names of their own) |
| Pass-Through, Enhance Edges, Denoise (Adjust Image) | **Send untouched (pass-through)**, **Edge enhance**, and **Remove specks** in **Advanced image** |
| Adjust Image: presets and preview | **Image preset** (7 built-in and your own with **Save as…**) and **Image preview…** per image, without calculating the job; see [Images](06-imagenes.md) |
| Copy and paste layer settings, delete layer | Layer row menu: **Copy settings**, **Paste settings**, **Delete layer and its objects…** |
| Default Layer Settings | **Defaults** at the end of the layer editor (a single set for all new layers) |
| Beginner Mode | **Beginner mode** of the layer editor (hides the layer's advanced settings; does not change menus or tools) |
| Cut Order Priority, Power Scale (per object) | **Cut priority** and **Power scale** in **Object properties**; **Cut earlier / Cut later** in the Arrange menu |
| Optimization Settings | **Optimization settings…** (preview, Machine panel, and Machine menu) |
| Laser (panel) and Move | **Machine** tab (connection, moving the head, origin, job) |
| Console | **Console** tab |
| Library | **Materials** tab (preset library) |
| Material Test | **Material test…** |
| Trace Image | **Trace image…** |
| Camera | **Camera** tab |
| Frame | **Frame** |
| Rubber Band Frame | **Frame outline** |
| Cut Selected Graphics, Use Selection Origin | **Selection only** and **Use the selection's origin** |
| Position Laser | **Position the laser** |
| Adjust Speed / Adjust Power (during the job) | **Live adjustments**: **Speed** and **Power** |
| Set Finish Position | **Finish position** (in **Move the head**) |
| Job checklist | **Before sending** list in the Start dialog, configurable |
| Start / Pause / Stop | **Start** / **Pause** / **Stop** |
| Save GCode | **Export G-code…** (File menu) |
| Export SVG, DXF, images | **Export SVG…**, **Export DXF…**, **Export PNG image…** (File menu); also **Export PDF…** and **Print…** |
| Print | **Print…**: generates a 1:1 scale PDF and opens it in the system viewer |
| Start From: Absolute Coords, Current Position, User Origin | **Start from**: Absolute coordinates, Current position, User origin |
| Job Origin (the 9 points) | Job point that is anchored (9 positions) |
| Set Origin | **Set origin here** |
| Macros | Profile **Macros** (up to 10, with buttons and shortcuts); see [Advanced machine control](09b-control-avanzado.md) |
| Move (Z and U), Fire, Focus | **Extra axes** block (Up/Down), **Test fire** (double confirmation, up to 20 % and 2 s), and **Focus** in **Move the head** |
| Saved Positions | **Saved positions** (in the profile, up to 20) |
| Frame with the laser on, repeat the frame | **Frame with the laser on at low power** and **Repeat the frame until stopped** |
| Machine Settings (GRBL) | **Machine Settings…**: the `$n` values with a `.tlakuset` backup; it neither reads nor writes Ruida's |
| Dual laser (Laser 2) | **The machine has two lasers** checkbox and a **Laser 2** option per layer |
| User bundle `.lbzip` | **File > Export / Import settings bundle…** (`.tlakubundle`; it does not open `.lbzip` files) |
| Device: TCP/IP (network) connection type | Profile **Connection type**: **Network · TCP / Telnet** or **Network · WebSocket**, with **Test connection**; see [Network connection and automation](08b-red-y-automatizacion.md) |
| Ruida (DSP) | **Ruida (CO2, network; experimental)** profile; see [Ruida and K40](13b-ruida-y-k40.md). LightBurn supports it; in Tlaku it is experimental |
| K40 with M2 Nano | LightBurn does not support it. Tlaku: experimental (tested only against the emulator), in the app over direct USB and with `tlaku k40`; power is set with the knob; see [Ruida and K40](13b-ruida-y-k40.md) |
| UDP automation | **Preferences > Automation** (own protocol, not LightBurn's); see [Network connection and automation](08b-red-y-automatizacion.md) |
| Camera: Update Overlay, Trace, Capture | **Update the overlay**, **Vectorize from camera**, and **Capture bed image** (see [Camera](10-camara.md)) |
| Camera: fisheye lens | **Lens model**: Automatic, Brown-Conrady, or **Fisheye** |
| Center Finder | Camera **Center finder** (does not use the head as a probe) |
| Devices, Find My Laser | **Manage Devices** and the **Add machine** wizard (**Detect** step) |
| Numeric Edits | Numeric bar above the canvas (X, Y, Width, Height, Rotation) |
| Create Lines (with curves) | **Draw Line** tool (`L`): click and drag draws Bézier curves |
| Draw Shapes: polygon, star | **Polygon** (`G`) and **Star** (`Shift+G`), with the **Shape properties** window |
| Shape Properties | **View > Shape Properties** (rectangle, ellipse, polygon, and star) |
| Measure | **Measure** (`M`) |
| Create Bar Code (QR and others) | **Tools > Insert code…** (QR, Data Matrix, and seven barcodes) |
| Create Text, Font Manager, SHX | **Text** tool (`T`) and its panel; **Tools > Import SHX font…**; 15 single-line fonts included |
| Welded text, Bend (curved text) | **Weld letters** checkbox and **Bend** field of the Text panel |
| Edit Nodes | **Edit Nodes** (node tool) |
| Flip Horizontal / Vertical | **Flip Horizontal** / **Flip Vertical** (Arrange menu) |
| Mirror Across Line | **Mirror Across a Line** |
| Rotation by two points | **Rotate and Scale by Two Points** |
| Draw Order (front, back, forward, backward) | **Bring to Front**, **Send to Back**, **Bring Forward**, **Send Backward** |
| Lock (lock shapes) | **Lock** and **Unlock All** |
| Transform toggles | **Allow Move**, **Allow Scale**, **Allow Rotate** (Arrange menu) |
| Make Same Width / Height | **Match Width**, **Match Height** (and their keep-proportions variants) |
| Select open shapes, contained shapes, circles by size... | **Edit** menu: **Select Open Shapes**, **Select Shapes Inside the Selection**, **Select Smaller Shapes**, **Select Circles by Size…** |
| Right-click menu | Canvas **context menu** (right-click) |
| File > Preferences and Edit Hotkeys | **Edit > Preferences…** (`Ctrl/Cmd+,`) and **Edit > Edit keyboard shortcuts…** |
| Boolean Union, Subtract, Intersection | **Union**, **Subtract (A minus B)…**, **Intersection** (plus **XOR**) |
| Weld, Offset Shapes | **Weld**, **Offset…** |
| Close Path, Join, Delete Duplicates | **Close paths…**, **Join paths…**, **Remove duplicates…** |
| Grid Array, Circular Array | **Grid array…**, **Circular array…** |
| Virtual Array | **Virtual array…** (copies that are not objects; see [Copies, virtual array, and nesting](03d-copias-y-nesting.md)) |
| Copy Along Path | **Copy along a path…** (without progressive scaling) |
| Move Together, Dock | **Dock and move together…** |
| Nesting | **Nest parts…**, in the Arrange menu: bed or irregular material, rotations, gaps, and a time limit |
| Auto-Group | **Edit > Auto-group by containment**, and the option of the same name when importing with options |
| Trim Shapes | **Trim shapes** tool (`X`) and **Trim at last click** (`Ctrl/Cmd+K`); see [Advanced shapes and warp](03c-formas-y-deformar.md) |
| Break Apart | **Break apart subpaths…** (`Alt+B`): separates by contour, not by segment |
| Optimize Shapes | **Optimize shapes…** (`Alt+Shift+O`): removes and simplifies nodes with a tolerance, with counts before and after |
| Fillet, Radius | **Round corners…**: rounded, chamfer, or bite; and the **Fillet corner by click** tool (`Shift+F`) for a single corner, which is reverted with `Alt`+click (the chamfer is not) |
| Cut Shapes | **Cut shapes with a blade…** (`Alt+Shift+C`) |
| Resize Slots | **Resize slots…** |
| Warp, Deform | **Warp…** (dialog with presets, four corners, and four curved edges) and **Warp on canvas** (`Shift+W`) with handles: **Bilinear**, **Perspective**, or **4×4 mesh** (16 points) |
| Rubber Band Outline | **Rubber band outline…**: convex or concave |
| Tangent Circle | **Tangent circle…** |
| Boolean Assistant | **Boolean assistant…** (`Ctrl/Cmd+B`) |
| Guides | **Guides**: drag from the rulers; **View > Guides…** for the exact position (no temporary alignment with `Alt`) |
| Tabs / Bridges (manual) | **Manual tabs** tool (`U`): click to add or remove; **Tab power (%)** in the layer editor |
| Start Point, Reverse Direction | **Start point** tool (`Q`), **Reverse direction**, and **Clear fixed start and direction** |
| Variable Text (serial, CSV, date and time) | **Tools > Variable text…** and the **Insert variable** menu of the text panel; see [Variable text](04b-texto-variable.md) |
| Art Library | **Tools > Art library**: 41 drawings of our own and your SVGs |
| Notes, Templates, New Window | **Notes** tab (`Ctrl/Cmd+7`), **Save as template…** and **New from template…** (File menu), **New window** (`Ctrl/Cmd+Shift+N`) |
| Dockable windows | Panel tabs can be reordered, hidden, **pulled out into floating windows** (double-click the tab), and the panel and tools can switch sides; **Reset Layout** puts it back to the default |
| Set Image Mask, Convert to Bitmap, Save Processed Bitmap, Replace Image | **Crop image to shape**, **Convert to bitmap…**, **Save processed image…**, and **Replace image…** (Tools menu); see [Images](06-imagenes.md) |
| Trace Image (several images) | **Trace multiple files…** |
| Align and Distribute | **Arrange** menu and **Align and distribute…** |
| Print & Cut | **Machine > Print & Cut…**: 3-step wizard with 2 to 4 marks, measured with the laser or the camera, no-scaling, uniform-scale, or affine fit, and each mark's error; see [Print & Cut](09c-print-and-cut.md) (images also rotate with the registration) |
| (does not exist in LightBurn) | **Machine > Split into tiles…** (*tiling*): jobs larger than the bed, with overlap, alignment marks, sending in tiles, and resuming; see [Tiling and automatic feeder](09d-tiling-y-alimentador.md) |
| Feeder (auto-feeder) | **Automatic feeder** group of the profile: tiling uses it between tiles (one column) and the Start dialog **repeats the job N times** with an advance between repeats (GRBL only); see [Tiling and automatic feeder](09d-tiling-y-alimentador.md) |
| Machine profiles: default, order, icon | **Default profile**, **Move up/down in the list**, and **Icon** in the profile's General group; see [Connecting your machine](08-conectar-maquina.md#profile-settings-worth-knowing) |
| Red pointer offset, Swap X/Y, Auto-home, DTR, Synchronous mode | **Red pointer offset** (and **Bring the red pointer to the point**), **Swap X and Y on the machine**, **Offer homing on connect**, **Open the serial port without raising DTR**, and **Synchronous streaming** in the profile |
| Units: mm/s | Profile **Speed unit** (mm/min or mm/s): it only changes how the speeds of the profile and the Start dialog are shown |
| Camera presets, network cameras, head alignment | **Camera presets**, **Network camera (URL)**, and **Automatic alignment with marks**; see [Camera](10-camara.md) |
| (does not exist in LightBurn) | **Machine > Review design…** (`Ctrl/Cmd+Alt+R`): twelve kinds of findings with one-click fixes; see [Review design](07b-revisar-diseno.md) |
| Job history / log | **Help > Job Log…**: date, machine, document, estimated and actual time, and result of each job sent |
| Galvo: Frame with red light, Start | **Machine** panel with a galvo board (framing with a rectangle, outline, convex hull, or loose objects, with live adjustment) and `tlaku galvo`; experimental, see [Galvo](13c-galvo.md) and [Galvo safety](13d-galvo-seguridad.md) |
| 3D Slice, Cylinder Correction, Taper Warp | With a galvo machine: cylinder and cone correction in the profile and 3D slice engraving in the layer editor; see [3D slice engraving, cylinder, and cone](12b-grabado-3d-y-cilindro.md) |
| Preview | **Preview** |
| Wireframe | **Outlines Only** |
| Undo History | **History** tab |
| Convert to Path (text) | **Convert to paths** in the text panel and in the context menu |

Tlaku's windows are panel tabs, which can be reordered and **pulled out into a floating window** (see [Guides, tabs, start point, and project](03e-guias-puentes-y-proyecto.md)). The toolbars at the top cannot be moved.

## Shortcuts

Tlaku's scheme is **its own**: it does not reproduce LightBurn's, and you can change any shortcut (see [Preferences, shortcuts, and autosave](02b-preferencias-y-atajos.md)); there is not yet a "LightBurn-style" profile. The LightBurn column comes from its official shortcut table (LightBurn 2.1 documentation); that table has contradictions, and where I **found no source** I write **(verify in your version)**.

| Action | Tlaku | LightBurn |
|---|---|---|
| New, open, save | `Ctrl/Cmd+N`, `O`, `S` | `Ctrl+N`, `O`, `S` |
| Save as | `Ctrl/Cmd+Shift+S` | `Ctrl+Shift+S` |
| Import design | `Ctrl/Cmd+I` | `Ctrl+I` |
| Export | G-code: `Ctrl/Cmd+Shift+E`; SVG, DXF, PNG, PDF, and AI have no shortcut | `Alt+X` (export) |
| Print | `Ctrl/Cmd+P` | `Ctrl+P` |
| Undo / Redo | `Ctrl/Cmd+Z` / `Ctrl/Cmd+Shift+Z` or `Ctrl/Cmd+Y` | `Ctrl+Z` / `Ctrl+Shift+Z` |
| Copy, cut, paste | `Ctrl/Cmd+C`, `X`, `V` | same |
| Select all, invert selection | `Ctrl/Cmd+A`, `Ctrl/Cmd+Shift+I` | `Ctrl+A`, `Ctrl+Shift+I` |
| Duplicate | `Ctrl/Cmd+D` | `Ctrl+D` |
| Paste in place | `Ctrl/Cmd+Shift+V` | `Alt+V` |
| Group / Ungroup | `Ctrl/Cmd+G` / `Ctrl/Cmd+Shift+G` | `Ctrl+G` / `Ctrl+U` |
| Flip horizontal / vertical | `Alt+H` or `Ctrl/Cmd+Shift+H` / `Alt+V` | `Ctrl+Shift+H` / `Ctrl+Shift+V` (and `H` / `V` with the canvas focused) |
| Mirror across a line | `Alt+Shift+M` | `Ctrl+Shift+M` |
| Rotate 90° | `[` / `]` | `.` and `,` (with the canvas focused) |
| Rotate by two points | `Alt+T` | `Ctrl+2` (with the canvas focused) |
| Draw order | `PageUp` / `PageDown`; with `Ctrl`, to the extreme | `PageUp` / `PageDown`; with `Ctrl`, to the extreme (the table itself contradicts itself about which goes up and which goes down) |
| Select | `V` (also `Esc`) | `Esc` |
| Line, rectangle, ellipse, text | `L`, `R`, `E`, `T` (single letters) | `Ctrl+L`, `Ctrl+R`, `Ctrl+E`, `Ctrl+T` |
| Edit nodes | `N` | `Ctrl` + the backtick key |
| Pan | `H` | (verify in your version) |
| Polygon, star | `G`, `Shift+G` | (verify in your version) |
| Measure | `M` | `Alt+M` or `Ctrl+M` (the documentation says both) |
| Manual tabs / start point / trim shapes | `U` / `Q` / `X` | (verify in your version) |
| Boolean assistant / trim at last click | `Ctrl/Cmd+B` / `Ctrl/Cmd+K` | (verify in your version) |
| Warp on canvas / fillet corner by click | `Shift+W` / `Shift+F` | (verify in your version) |
| Review design | `Ctrl/Cmd+Alt+R` | (does not exist) |
| New window | `Ctrl/Cmd+Shift+N` | (verify in your version) |
| Offset | `Alt+O` | `Alt+O` |
| Weld | `Alt+W` | `Ctrl+W` |
| Union / Subtract / Intersection | `Alt+U` / `Alt+D` / no shortcut | `Alt` with the keypad `+`, `-`, and `*` signs |
| Join paths (Auto-Join) | no shortcut | `Alt+J` |
| Remove duplicates | no shortcut | `Alt+D` |
| Preview | `P` | `Alt+P` |
| Zoom, fit to bed | `Ctrl/Cmd+=` and `-`, `Ctrl/Cmd+0` | `Ctrl+=` and `-`, `Ctrl+0` |
| Outlines only | `W` | `Alt+Shift+W` |
| Move with arrow keys | 1 mm; `Shift` 10 mm; `Alt` 0.1 mm (configurable) | 5 mm; `Shift` large step; `Ctrl` small step (configurable) |
| Align with the keyboard | no shortcut | `Alt`+arrows |
| Frame | `F` | (verify in your version) |
| Start | `Ctrl/Cmd+Enter` | (verify in your version) |
| Pause / Stop | `Ctrl/Cmd+Shift+P` / `Ctrl/Cmd+.` | (verify in your version) |
| Pick layer by color | `Alt+1` to `Alt+0` | (verify in your version) |
| Show layers, machine, console, history | `Ctrl/Cmd+1`, `2`, `3`, `4` | (verify in your version) |
| Shortcut list | `F1` or `Ctrl/Cmd+/` | (verify in your version) |

Differences that bite:

- In Tlaku the tool keys are **single letters** (no `Ctrl`) and they do not work while you are typing in a field. What in LightBurn is `Ctrl+L` (line) is `L` in Tlaku; and `Ctrl/Cmd+L` is **Lock**.
- **`Alt+V`** pastes in place in LightBurn, but **flips vertically** in Tlaku (there, paste in place is `Ctrl/Cmd+Shift+V`).
- **`Alt+D`** removes duplicates in LightBurn; in Tlaku it is **Subtract**.
- **`Alt`+arrows** aligns in LightBurn; in Tlaku it moves the selection 0.1 mm.
- **`Ctrl/Cmd+W`** welds in LightBurn; in Tlaku it is `Alt+W`.

## What is missing or different

Done today in Tlaku: layers by color with Line, Fill, and Image modes, preview with cut order and optimization settings, GRBL connection over USB and over the network (TCP and WebSocket), macros, Z and U axes, saved positions, `$$` settings with backup, console, camera, materials, rotary and Z axis, tracing, booleans and their assistant, arrays (also virtual ones and along a path), docking, nesting, trimming, breaking apart, optimizing, rounding corners, cutting with a blade, resizing slots, warping, guides, notes and templates, art library, image mask, convert to bitmap, tracing several images, text (with single-line fonts, curved and welded), [variable text](04b-texto-variable.md) (serial, CSV, date and time), QR and barcodes, polygon and star, Bézier curves, measuring, flipping, draw order and locking, manual tabs, start point and direction, context menu, preferences, configurable shortcuts, autosave, advanced cut settings (perforation, lead-in and lead-out, overcut, pauses, cut-through, dot mode, ramp, additional operations, per-object priority and power scale, flood fill, angle increment, untouched image), cutting only the selection, outline framing, positioning the laser, low-power and looping framing, live speed and power adjustments, tab power, camera with fisheye, head-mounted camera, center finder, configuration bundles, Ruida (experimental), Print & Cut, tiling with automatic feeder, repeating jobs with the feeder, warping with handles, one-click rounding, floating panels, image presets and preview, network cameras, [Review design](07b-revisar-diseno.md), the job log, the galvo in the app (experimental), and exporting and importing the formats in [Import and export](03b-importar-y-exportar.md) (also `.rd`, `.egv`, and export to `.ai`).

Missing, and I do not promise it if it is not in the code:

- **Print & Cut and tiling are already there** (see [Print & Cut](09c-print-and-cut.md) and [Tiling and automatic feeder](09d-tiling-y-alimentador.md)), but they were tested only with the virtual machine. **Tiling is something LightBurn does not have**: there a job larger than the bed is split by hand. Print & Cut in Tlaku supports up to 4 marks, an affine fit, measuring with the camera, and images **rotate with the registration**.
- **Of the feeder:** the Start dialog repeats a job N times with an advance, **only with GRBL machines**; the built-in feeder of a DSP controller (Ruida, Trocen) is not driven.
- **Of the design:** progressive scaling when copying along a path, **Cut shapes** grouping the pieces into inside and outside, temporary alignment guides, and variable text with layer-setting values or "baking" (achieved by converting the text to paths).
- **Of the cut:** dragging tabs already placed and per-color and per-machine defaults (there is a single set). **Layer** speeds are still in mm/min (only the profile and the Start dialog can show them in mm/s).
- **Of the machine:** "device and control units" as in LightBurn (there is only the speed unit), the per-layer U setting (there is a per-job U offset in the profile) and the second laser's offset; autofocus. With Ruida the controller's settings are not written and its documents are not deleted or run (`.rd` files do open as a design).
- **Of beginner mode:** in Tlaku it only hides the layer editor's advanced settings; it does not hide tools or menus as LightBurn's does.
- Individual objects cannot be hidden (layers can), there is no selection with `Tab`, and no skewing.
- The **inches** in the preferences apply in the numeric bar, the status bar, the rulers (tenths of an inch), Measure, and the length fields of the Arrange and Nest parts dialogs; fields accept fractions (`3/8"`, `1 1/2 in`) and suffixes (`5mm`, `50%`). Speeds stay in mm/min, and the fields of layers, machine areas, and camera stay in mm.
- **Galvo (fiber, UV):** **experimental** support for EZCAD2's LMCV4 board, from the app (adding with verification without the laser, profile, framing, overscan, required inputs) and from the command line (`tlaku galvo`), tested only against a simulated board. Read [Galvo safety](13d-galvo-seguridad.md) first. There is no pause (stop only), no 9-point lens calibration, and no `.cor` files, and EZCAD3/DLC and BSL are not supported. Trocen and TopWisdom are not supported. GRBL over the network (TCP and WebSocket) is there; Ruida and the K40 with M2 Nano are experimental and neither has been tested with a real machine: see [Supported controllers](13-controladoras.md).
- There is only one guides layer (the one in the color palette); the ruler guides are separate.
- **New window** has not been tested in a real window (see [Guides, tabs, start point, and project](03e-guias-puentes-y-proyecto.md)).

## How to move a design

There are two paths: **export from LightBurn to an open format** (what works today in the app) or **bring in the full `.lbrn` or `.lbrn2` project**, with its layers and settings (below).

### With SVG, DXF, or PDF (works today)

1. In the program where you drew it, save or export the design as **SVG** (best, keeps the curves), **DXF**, or **PDF**. If you made it in LightBurn, check in your version which export formats it offers.
2. In Tlaku open **File > Import Design…** (`Ctrl/Cmd+I`), or **Import with options…** if you need to choose the scale or the DPI.
3. The strokes arrive **grouped by color**: each color becomes a Line-mode layer. Change the mode to Fill where you want to engrave.
4. **Set speed, power, and passes again.** Cut settings do not travel in SVG, DXF, or PDF. You can start from the **Materials** tab (see [Materials](11-materiales.md)).
5. Check the size in the numeric bar. If it does not match the original, try another DPI in the import options (Illustrator uses 72) or see [Troubleshooting](15-solucion-de-problemas.md).
6. Create or choose your machine ([Connecting your machine](08-conectar-maquina.md)) and look at the **Preview** before sending.

### With a LightBurn project (`.lbrn` and `.lbrn2`)

Tlaku opens LightBurn projects with their **layers and settings**: use **File > Import Design…** or drag the file onto the window.

**What is kept**

- **Shapes:** rectangles (with their corner radius), ellipses, paths with curves, groups, **text**, and embedded **images**. Each shape stays at the same position on the bed. Nested groups are flattened into a single group; locked objects stay locked.
- **Images:** the embedded image is extracted to a file in a `NAME_imagenes` folder next to the project (or, if it cannot be written there, to the temporary folder, and it says so). If a layer has images and vectors at the same time, the images go to a layer of their own.
- **Layers** (`C00` to `C29`): the name, the **mode** (line, fill, offset fill, image), the **speed** (from mm/s to mm/min), the **maximum and minimum power**, the **passes**, the **interval** (or the DPI), the **angle**, crosshatch, **bidirectional**, **overscan**, **kerf**, **air**, the **Z offset and Z per pass**, whether the layer is output to the machine and whether it is hidden, the **tabs**, and, for images, the **dithering method** and the halftone.
- **Execution order:** layers are ordered by LightBurn's **priority**.
- **Guides:** the `T1` and `T2` tool layers become guides, which never go out to the machine.
- **Colors:** LightBurn does not publish the exact RGB of its 30-color palette; Tlaku uses **approximate** colors, distinct per layer. Each LightBurn layer gets a different color and keeps its name and its order, but do not expect the same shade.

**What it warns about**

Nothing is lost silently. On import, a notice lists what could not be brought in:

- **Shapes of an unsupported type:** they are skipped and the notice says **how many and of what type**.
- **Text:** if the file does not include the letters' outlines, Tlaku generates them with your fonts (and warns if the font is not installed and it used another); LightBurn's **letter and line spacing** is not imported.
- **Layer settings with no equivalent in Tlaku**, only if they were active: pulses per inch (PPI), laser frequency, second laser, constant power, blower speed, and per-object fill. They are not translated; you have to decide by hand what to do.
- **Advanced cut settings that are translated** (since wave 3): perforation, dot mode, flood fill, overcut, power ramp, start and end pauses, cut-through, and **sub-layers** (several definitions with the same index become **Additional operations**). Several conversions were inferred from public documentation (for example, the cut-through time comes from the start or end pause, the ramp starts at 0 %, and overscan is interpreted as a percentage of the speed), so **verify each layer** with the preview.
- **Lead-in and lead-out:** LightBurn does not store them in the layer in a known way, so they are left off.
- **Fill + line** in a single layer: only the fill is imported (create another line layer if you want the outline).
- **Manual tabs:** the position of the tabs you placed by hand in LightBurn is not imported; here they are distributed automatically according to the layer settings. If it matters to you where they end up, place your own with the **Manual tabs** tool (`U`).
- **Layers cited but not defined** in the file, an unknown layer or image mode, or a file saved with a newer LightBurn version than Tlaku knows (it tries to read it anyway).

> **Warning:** even if the settings arrive, **check them before cutting**. The overscan unit conversion was inferred from public documentation, and none of this has yet been tested with a real machine. Review each layer and look at the **Preview**.

> **Tip:** try the **virtual machine** before your first real job: it uses no material and shows you how the framing, pauses, and alarms look.

> **Warning:** none of the machine connection has been tested with real hardware. As long as LightBurn keeps working with your laser, keep it as plan B.

Next: [Troubleshooting](15-solucion-de-problemas.md)
