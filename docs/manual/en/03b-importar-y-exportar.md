# Importing and exporting

Which files can come into Tlaku, with which options, and in which formats you can take your design out to use it in another program, share it or print it at full size. Shortcuts are written with `Ctrl/Cmd`: `Cmd` on macOS, `Ctrl` on Windows and Linux.

## Importing

There are two ways:

- **File > Import Design…** (`Ctrl/Cmd+I`) or **dragging** the file onto the window: it imports **without asking**, with automatic options. It creates one layer for each color, selects what was imported and adjusts the view.
- **File > Import with options…**: the same, but it first opens a dialog with the options below. You can choose **several files** at once.

If the importer could not bring in something, a **persistent notice** appears saying what was missing: read it before sending anything to the machine. Tlaku prefers to warn you rather than silently lose something.

**If nothing comes in, the message says why.** When a file has nothing to import, the error does not stop at "contains no paths": it keeps the cause the importer already knows and says what to do. For example, **"3 HATCH and 12 TEXT were skipped"** in a DXF (convert them to outlines in your CAD program and export again), or an SVG whose content is text or shapes without an outline (convert them to paths in your editor and save again), or a file with an image that can be imported as an image. If your file does not come in, read the whole message before looking for another format.

### Formats you can import

| Format | Extensions | What it brings |
|---|---|---|
| SVG | `.svg` | Bézier curves, groups, clips and text (with the installed fonts). One layer per color. An SVG that only contains embedded images is imported as an image (see below). |
| DXF | `.dxf` | Lines, arcs, circles, ellipses, splines and blocks, with the file's units. Text is ignored with a notice. |
| Images | `.png`, `.jpg`, `.jpeg`, `.bmp`, `.webp`, `.tif`, `.tiff`, `.gif`, `.tga` | An image on its own **Image** mode layer (see [Images](06-imagenes.md)). |
| PDF | `.pdf` | The **paths** of one page, in millimeters, one layer per color. |
| Adobe Illustrator | `.ai` | Same as a PDF, if it was saved with **Create PDF Compatible File** (the `.ai` files Tlaku exports already are). |
| HPGL (plotters and vinyl cutters) | `.plt`, `.hpgl`, `.hpg` | The strokes, **one layer per pen**. |
| G-code | `.nc`, `.gcode`, `.gc`, `.ngc` | The strokes with the laser on, as geometry. |
| Ruida (CO2) | `.rd` | The cuts of a saved job from a Ruida controller, **one layer per layer in the file**, with its speed, power, color and air assist. |
| K40 (M2 Nano) | `.egv` | The laser-on movements of a LHYMICRO-GL job, **one layer per speed**. |

Notes by format:

- **PDF and AI.** Only paths are imported. Text with fonts (convert it to curves in the source program), embedded images, gradients, patterns and clips are **not** imported; Tlaku warns you. If the PDF has several pages, it imports the first one and says so; choose another in the options. An old AI (PostScript, older than Illustrator CS) cannot be read: the app explains how to convert it (save it from Illustrator with **Create PDF Compatible File**, or export it to PDF or SVG).
- **HPGL.** The format does not declare its scale; Tlaku assumes **40 units per millimeter** (0.025 mm, the usual value) and warns you. If the size does not match, correct it with the **Scale** in the import options.
- **G-code.** Only the engraving moves count: `G1`, `G2` and `G3` with the laser on (`M3` or `M4` and `S` greater than 0). Rapid travels (`G0`) are not drawn. By default **one layer per power level** is made (up to 8, from lowest to highest); choose **A single layer** in the options if you prefer just one. It understands `G90`/`G91`, `G20`/`G21` and arcs with `I J` or with `R`. It is useful for recovering a design from a G-code, not for reproducing the original's speed settings.
- **SVG with images only.** If the SVG has no paths but does have **embedded images** (which some programs do when they "export" a photo), Tlaku extracts them to files next to the SVG (`name.tlaku-img1.png`, `name.tlaku-img2.png`…) and imports them as **images** at their position and size within the SVG, on an **Image** mode layer. It tells you so. Keep those files next to the project; if the folder cannot be written to, they go to the temporary folder. To cut or engrave outlines you need an SVG with paths.
- **Images.** The window cannot display TIFF or TGA: Tlaku converts them to a PNG (`name.tlaku.png`) next to the original and the design uses that copy, which you must keep next to the project; if that folder cannot be written to, the copy goes to the temporary folder and you have to import the image again after restarting. In both cases it warns you. The size comes from the resolution (96 dpi by default, or the one you set in **Preferences** or in the import options).
- **Very large images.** Tlaku does not load the whole photo into memory: it reads it by rows (PNG) or by strips (TIFF) and reduces it to the grid the engraving needs, so a bitmap of 60 to 100 megapixels no longer exhausts memory. **Large TIFFs** (87 Mpx or more) and **YCbCr TIFFs** (the color mode of many scanners and some cameras) now load. With the other formats (JPEG, BMP, WebP, GIF, TGA) there is a generous memory limit; if an image exceeds it, the message suggests saving it as PNG from your editor and importing it again. For that order of size, see [Performance with large designs](15b-rendimiento.md).
- **Ruida (`.rd`).** The file on disk is obfuscated and does not say which board it is for: Tlaku tries the known keys and keeps the one that gives a valid job (it tells you if it is not the usual one). What the format does **not** store: the machine origin (it is assumed to be **top left with Y pointing down** and Y is flipped relative to the default profile's bed; if your machine is different, the drawing may come out shifted or mirrored: move or mirror it) and, on fill or image layers, the power that changes within the layer (it is imported with the **max power** and without the overscan segments). Commands it does not recognize are ignored with a notice. It is useful for **recovering a design** from an `.rd`, not for reproducing the job as is.
- **K40 (`.egv`).** Only the movements with the laser on count. The format **does not store power** (the layers come out with the default power: adjust it; on a K40 it is set with the knob), nor the board model (an **M2**, the most common, is assumed in order to convert the speed codes) nor the origin (home is assumed to be top left). The rows of a raster-mode engraving are not imported as vectors, and you are told how many were skipped.
- **G-code with power 0.** If **all** of the file's feed moves come with power 0 (`M3 S0`), there is nothing to draw. Instead of failing, Tlaku opens the **The G-code has no powered cuts** dialog: you enter the **S** value with which those feed moves are taken (500 to start, on the usual GRBL scale where 1000 is 100 %) and press **Import with that power**. It also counts the moves the file makes with `G1 S0`, so check the result.
- **LightBurn projects** (`.lbrn`, `.lbrn2`). They are imported with their layers and settings. What they bring and what is flagged is in [Coming from LightBurn](14-desde-lightburn.md).

### Import options

The **Import options** dialog shows only the ones that make sense for the chosen files:

| Option | What it does |
|---|---|
| **Scale (%)** | Multiplies the whole design. 100 leaves it unchanged. Useful for correcting an HPGL or an SVG of a different size. |
| **Resolution (dpi) for unitless SVGs and images** | How many pixels make an inch when the file does not say. By default it is the **Default** from [Preferences](02b-preferencias-y-atajos.md) (96 out of the box). Illustrator usually uses 72: if the size comes out different from the original, try 72. |
| **Unit for a DXF without units** | **Millimeters (warning)**, **Inches** or **Centimeters**. It only counts if the DXF does not declare its units. |
| **Group everything imported** | Joins everything into a single group so you can move it as one piece. |
| **Group by containment** | Groups each piece with its holes and engravings (what is inside its outline). It cannot be combined with the previous option. |
| **Target layer for vectors** | **One layer per color (automatic)** or an existing layer to put all the vectors in. |
| **PDF or AI page** | Which page to import, starting from 1. |
| **G-code** | **One layer per power level** or **A single layer**. |

**Import** applies the options to all the chosen files; **Cancel** imports nothing.

**Remembered options.** By default, the dialog starts with what you chose last time: resolution, DXF unit, grouping and target layer (the layer is only kept if the design still has it). The **scale** and the **PDF page** always start from scratch, because they are almost never repeated. In **Edit > Preferences… > Files** you can change the **Default resolution**, turn off **Remember the import dialog options** or press **Forget the remembered options**. Importing without the dialog (`Ctrl/Cmd+I` or dragging) uses the default resolution but not the other options.

## Exporting

With something visible in the design, the **File** menu offers:

- **Export G-code…** (`Ctrl/Cmd+Shift+E`): the job for the machine. See [Sending jobs safely](09-enviar-con-seguridad.md).
- **Export SVG…**, **Export DXF…**, **Export PNG image…**, **Export PDF…** and **Export AI…**: your **design** in formats that other programs open. Without visible objects, these commands are disabled and say **There are no objects to export.**

All five open a dialog; then **Export…** asks where to save, and when it finishes the notice gives the path, the size of the drawing in mm and the file size. Hidden objects and those on hidden layers are **not** exported.

![Export SVG dialog with the export options and the choice to export only the selection](../../../apps/desktop/docs/screenshots/30-exportar.png)

In all of them, **What to export** lets you choose **The whole design** or **Selection only** (with how many objects). A text comes out as paths; if it does not yet have a generated outline, it is skipped and you are told.

| Format | Options | What to know |
|---|---|---|
| **SVG** | **Images** (**Embed**, **Link** or **Do not include**), **Margin**, **Stroke width**, **Keep the bed coordinates**, **Fill the shapes on fill layers** | Real measurements in mm (1 unit = 1 mm), Bézier curves as they are and **one group per layer**, with the layer's color. By default the page is the size of the design; with **Keep the bed coordinates**, the bed origin is the corner of the SVG. |
| **DXF** | **Curves** (**Exact curves (SPLINE)** or **Flatten to polylines**), **Tolerance**, **Save the exact color (DXF 2004)** | In mm, with **one layer per color**. Paths made only of curves come out as exact SPLINEs; those that mix lines and curves come out as a flattened polyline. Without the exact color, the nearest ACI color is kept. |
| **PNG** | **Resolution (dpi)**, **Margin**, **Stroke width**, **Transparent background**, **Fill the shapes on fill layers** | The design drawn with each layer's color. 300 dpi by default; there is a size cap so that an absurd value does not exhaust memory. |
| **PDF** | **Page** (**Fit to design**, **A4**, **A3**, **Letter**, **Legal**), **Margin**, **Stroke width**, **Include images**, **Fill the shapes on fill layers** | **Vector** PDF at **1:1 scale**: 1 mm of the design is 1 mm on paper. |
| **AI** | The same as PDF | An `.ai` file at 1:1 scale with **one layer per color**. **It is a PDF 1.5 with layers, not a native Illustrator document**: Illustrator opens it and keeps the layers and colors, but it carries nothing specific to its format (symbols, effects, brushes). For other programs, SVG is still the safest output. |

> **Tip:** SVG and DXF are the output for taking parts to other programs or to another cutter, and the best way not to depend on a single program. PDF and AI open in Illustrator and in almost any vector editor; AI adds the color layers.

### Printing

**File > Print…** (`Ctrl/Cmd+P`) opens the same dialog as **Export PDF…** and, when you accept, generates the PDF at 1:1 scale and **opens it in the system viewer**. From there print it at **actual size (100 %)** and without **fit to page**; otherwise the template comes out at a different scale. Tlaku does not print directly to the printer.

### Copying and pasting between programs

**Copy** and **Cut** put the selection on the system clipboard as SVG; **Edit > Copy as SVG** (`Ctrl/Cmd+Alt+C`) does the same and tells you it did. **Paste** (`Ctrl/Cmd+V`) reads the system clipboard:

- An **SVG** from another program (Inkscape, Illustrator and similar) comes in as a drawing; you are told if elements that are not shapes (text, images, references) were skipped.
- Any **text** comes in as a text object.
- What Tlaku itself copied is pasted from its internal clipboard and **keeps** layers, editable text and shapes.

If the system blocks access to the clipboard, Tlaku says so and keeps working with its internal clipboard.

Next: [Advanced shapes and warping](03c-formas-y-deformar.md)
