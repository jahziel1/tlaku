# Text

How to create text, choose its font (system or single-line), curve it, weld its letters, write it along a path and convert it to editable paths; and how to insert QR codes and barcodes. Tlaku's text is calculated from the fonts installed on your system and from the single-line fonts it includes.

## Creating a text

1. Choose the **Text** tool (`T`).
2. Click on an empty spot on the canvas. A sample text (10 mm, Arial if installed; otherwise the first one available) appears on the active layer, already selected.
3. Type in the **Content** field of the panel that floats next to the text. `Enter` adds a line.

To edit an existing text, double-click it with **Select**, or select it: the **Text** panel appears whenever the selection is a single text. You can move, scale and rotate a text like any object (see [Drawing and editing](03-dibujar-y-editar.md)).

![Text panel next to a selected text, with content, font, style, size and the On path option](../../../apps/desktop/docs/screenshots/12-panel-de-texto.png)

> **Tip:** text goes to a layer like any other object. On a **Line** layer the outline of the letters is traced; on a **Fill** layer their interior is engraved. A text cannot be used on an **Image** layer (see [Layers and cut settings](05-capas-y-ajustes.md)).

## Panel options

| Option | What it does |
|---|---|
| **Content** | The text, with several lines if you want. |
| **Font** | A search box (**Search fonts**) over the installed families. If there are many, type to narrow the list. |
| **Bold**, **Italic** | Request those variants of the font. |
| **Align left**, **Center**, **Align right** | Alignment of the lines relative to each other. |
| **Size** | In mm. It is the size of the em, not the height of the capital letter: an "H" comes out shorter than the value you type. |
| **Letter spacing** | Extra separation between letters, in mm (negative values allowed). |
| **Line spacing** | In multiples of the size; it starts at 1.2. |
| **Bend** | Degrees the text spans along an arc (see "Curved text"). |
| **Weld letters** | Joins letters that touch into a single outline (see "Welded text"). |
| **Insert variable** | Puts a variable in braces (serial number, CSV data, date or time) where the cursor is. See [Variable text](04b-texto-variable.md). |

> **Heads up:** Tlaku never fakes a bold or an italic. If the family does not have that variant, it uses the closest one that exists and says so in the **Outline warnings**. If the font you chose is not installed (for example, when opening a project on another computer), it uses another one and warns you: "The font '...' is not installed; '...' is used instead." Check the warning before sending anything.

While the outline is being calculated you will see **Generating outline…**. If it fails, the panel shows the reason.

## Text on a path

To make the text follow a line or a curve:

1. First draw the guide stroke (a line, polyline, ellipse...). Without a vector object in the design, the checkbox cannot be ticked: "Draw a line or a curve so you can type along it."
2. Select the text and tick **On path**.
3. In **Guide object** choose the stroke.
4. Adjust the **Offset** (in mm, along the path) and tick **Flip** to write on the other side.

If you delete the guide object, the panel warns you: choose another or untick **On path**. The guide stroke is still a normal object and will be cut or engraved according to its layer; put it on a layer with no output (or in **Guides**) if you only want it as a reference.

## Convert to paths

**Convert to paths** replaces the text with vector paths. Use it when you need to edit the nodes of the letters, or when you are going to move the project to another computer without depending on the font.

- Afterwards you can no longer change the font or the content: it is one step with **Undo** if you regret it.
- It is disabled while the outline is being generated, or if the text does not have an outline yet (type something with an installed font).

A text with an outline already works with the Booleans and the **Offset** in the **Arrange** menu. On the other hand, **Edit Nodes** and **Join paths…**, **Close paths…** and **Remove duplicates…** only work with paths: convert the text first.

## Curved text

The **Bend** field in the panel arranges the text along an **arc**, without needing to draw a guide path:

- It is typed in degrees and is **what the longest line spans** along the arc. **Positive** arches it upward (the center of the arc is below, like a rainbow); **negative** downward (like a smile); **0** leaves it straight.
- It accepts up to **360°** (a full turn); if you type more, it limits it and warns you.
- With several lines, all of them curve around the same center. If the curvature is so tight that the radius is smaller than the text height, the letters get squashed and the panel warns you.
- It is ignored while the text is **On path**: they are two ways of curving and the path takes precedence. The panel says so.

A curved text is still a text: you can change its content or its bend later.

## Welded text

Italic and calligraphy fonts have letters that overlap: on a **Line** layer the laser traces each full outline and passes twice where they cross. Tick **Weld letters** and the letters that touch or overlap are **fused into a single outline**.

- It only works with outline fonts (the system ones). With a single-line font there are no outlines to join and the panel warns you.
- The same can be done by hand with **Convert to paths** and then **Weld** (`Alt+W`), but the text stops being editable. With the checkbox, the text stays text.

## Single-line fonts

A normal font draws the **outline** of each letter; a **single-line** (single-stroke) font draws only the **center stroke**. They are good for engraving thin text quickly, so the laser passes only once per letter, and for plotters and lettering cuts.

Tlaku includes **15 single-line Hershey fonts**, with nothing to install: Roman Simplex, Duplex and Triplex; Futura Light and Medium; Times Roman, Bold, Italic and Italic Bold; Script Simplex and Complex; Cursive; and Gothic English, German and Italian. In the font selector they appear in their own section, **Single-line fonts**, below **System fonts**, and the panel reminds you what they are for.

How to use them:

1. Create or select a text and choose a font from the **Single-line fonts** section.
2. Put the text on a layer in **Line** mode. The result is **open paths**: on a Fill layer there is nothing to fill.
3. **Size**, **Letter spacing**, **Line spacing**, alignment and **Bend** work the same. The capital letter measures 72 % of the size you type. These fonts **have no bold or italic**, and cannot be welded.

### Importing SHX fonts

If you already have AutoCAD **SHX** fonts, choose **Tools > Import SHX font…** (or the **Import SHX font…** button in the Text panel), tick one or several `.shx` files and they are added to the single-line section. They are kept for future sessions; when finished, a notice lists the names added and, if something fails, says why. Tlaku reads the `shapes` (1.0 and 1.1) and `unifont` formats; `bigfont` fonts (Chinese, Japanese, Korean) are not supported yet.

> **Heads up:** SHX fonts have their own license. Use only the ones you have the right to use. The Hershey data included in Tlaku is free to use with its acknowledgments, which are cited next to the selector and in **Help > About Tlaku**.

## QR codes and barcodes

**Tools > Insert code…** opens a dialog that generates a code as **vector shapes** ready to engrave or cut, with a live preview.

![Insert code dialog with the type, the content, error correction and the size](../../../apps/desktop/docs/screenshots/31-codigo-qr.png)

### Types

| Group | Type | What it accepts |
|---|---|---|
| Two-dimensional | **QR** | Any text or link. |
| Two-dimensional | **Data Matrix** | Any text; good for marking small parts. |
| Barcodes | **Code 128** | ASCII text: letters, numbers and symbols. |
| Barcodes | **Code 39** | Uppercase letters, numbers and `- . space $ / + %`. |
| Barcodes | **EAN-13** | 12 digits, or 13 with the check digit. |
| Barcodes | **EAN-8** | 7 digits, or 8 with the check digit. |
| Barcodes | **UPC-A** | 11 digits, or 12 with the check digit. |
| Barcodes | **ITF (interleaved 2 of 5)** | Digits only; with an odd count a check digit is added. |
| Barcodes | **Codabar** | Digits and `- $ / : . +`; starts and ends with A, B, C or D. |

If the content is not valid for the type, the dialog explains why and does not let you insert.

### Options

| Option | What it does |
|---|---|
| **Content** | What the code encodes. |
| **Error correction** (QR only) | **L** 7 %, **M** 15 % (default), **Q** 25 % or **H** 30 %: how much damage it tolerates and still reads. More correction, denser code. |
| **Code side length** (QR and Data Matrix) | Side length in mm. 30 mm by default. |
| **Bar width** and **Bar height** (barcodes) | Size of the code in mm. |
| **Margin** | Blank zone around it; it is only drawn when inverted, as the border of the background. |
| **Human-readable text below** (barcodes) | Adds the digits or characters below the bars, with the **Text font** you choose. |
| **Invert (engrave the background)** | Delivers the background with the code as a gap. Useful on dark materials; **not all readers read an inverted code**. |

The preview shows the modules and the final size in mm. **Insert** puts the code at the **center of the view** (or of the bed, if the view is outside it) and on the active layer; the notice gives the dimensions. At the bottom of the dialog you can see which layer it will go to.

> **Heads up:** the code is a set of **already-generated paths**, not an editable code: to change the content or the type, delete the inserted one and insert another (or use **Undo**). Nor is it a variable code: serial numbers and CSV data (see [Variable text](04b-texto-variable.md)) work in **texts**, not inside a QR or a barcode. **Always check with a real reader** that the engraved code reads, with your material and at your size: a module that is too small or low-contrast will not read even if the design is right.

Next: [Variable text](04b-texto-variable.md)
