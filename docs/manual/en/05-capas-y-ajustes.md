# Layers and cut settings

In Tlaku each color is a **layer**, and each layer is an operation with its own settings: how much it moves, with what power and in what way. This page explains the **Layers** panel, the settings editor (with its advanced sections), additional operations and the order in which they run.

> **Heads up:** a new layer starts with 1000 mm/min, 20 % power and 1 pass. These are starting values, not for any material. Always test on a scrap before cutting the good piece (the [material test](11-materiales.md) helps find the values).

## The Layers panel

It opens with `Ctrl/Cmd+1`. Each row shows the color and name, how many objects it has, a summary (speed, power, interval, passes) and a label: **Engrave**, **Line** or **Cut**. If the layer has additional operations you will also see a badge such as **+2 ops**. On the right there are three buttons:

- The **eye** shows or hides the layer on the canvas. It does not change what is sent.
- The **bolt** decides whether the layer is **sent to the machine**. Turn it off to test the rest without deleting it.
- The **three dots** open a menu (also with a right-click on the row): **Layer settings…**, **Select all objects on this layer**, **Rename**, **Run earlier**, **Run later**, **Pin to this position**, **Copy settings**, **Paste settings** and **Delete layer and its objects…**.

With the keyboard: arrows to move through the list, `Enter` or double-click to open the settings, `F2` to rename and `Alt+↑`/`Alt+↓` to reorder. `Alt+Enter` opens the active layer's settings from anywhere.

To move objects to a layer, select them and choose a color from the strip below. The **Guides** layer (the last, dotted square) serves as a reference while designing and is never sent to the machine; it has no cut settings, so you cannot copy or paste onto it.

### Copying and pasting settings, and deleting a layer

- **Copy settings** saves all the layer's settings (including its additional operations) and **Paste settings** puts them on another layer in a single **Undo** step. The target layer keeps its color, its name, whether it is visible, whether it goes out to the machine and its pinned position. It is the quick way to give ten cut layers the same starting values. They also exist as commands (**Copy settings of the active layer**, **Paste settings into the active layer**, **Delete the active layer and its objects…**) that are not in the menus, but appear in **Edit > Edit keyboard shortcuts…**, where you can assign a key to them.
- **Delete layer and its objects…** asks for confirmation and deletes the layer together with everything inside it (also one **Undo** step). If any object on the layer is locked, it will not let you delete and tells you: unlock it first.

## Settings editor

Open the editor with `Enter` on a row. At the top, the **Help** button shows or hides an explanation under each parameter. Invalid fields are marked in red and are not applied; each change has its own **Undo** step.

![Editor of a cut layer with speed, power, passes and air assist](../../../apps/desktop/docs/screenshots/03-editor-de-capa.png)

### Advanced settings and beginner mode

The advanced settings (advanced cutting, flood fill and grouping, advanced image) live in **collapsible sections** that open by themselves when the layer already uses something from them, with the **in use** badge. That way the editor shows speed, power, passes and interval first.

**Beginner mode** is no longer a switch in the layer editor: **it is [simple mode](01b-modo-sencillo.md)**, which besides hiding the advanced settings hides the menus, the tools and the panels, and guides you step by step. Turn it on with the button in the top bar or in **Preferences > General > Interface mode**. If you had Beginner mode on and had never chosen a mode, Tlaku starts in simple mode; if you had already chosen one, your choice is kept.

### Layer mode

| Mode | What it does |
|---|---|
| **Line** | Follows the outlines of the objects: for cutting or scoring. |
| **Fill** | Scans the inside of closed shapes with parallel lines: for engraving surfaces. |
| **Image** | Engraves photos and bitmaps by varying the power dot by dot (see [Images](06-imagenes.md)). |

### Speed and power

| Setting | Range and unit | Notes |
|---|---|---|
| **Speed** | 1 to 60000 mm/min | Slower leaves more energy in each millimeter. It warns if it exceeds the active machine's maximum. |
| **Power** | 0 to 100 % | In Image mode it is called **Max power**. |
| **Min power** | 0 to 100 % | Only on **Image** layers: it is the power of the lightest tones. It cannot exceed the maximum. On Line and Fill layers it does not exist: the field does not appear. See the note below. |
| **Passes** | 1 to 1000 | Two passes at half power usually cut cleaner than one at full power. |
| **Air assist** | yes or no | Turns on the machine's air during the layer. |

> **Heads up:** min power only changes the G-code on **Image** layers. On a **Line** layer with dynamic power (M4) the editor does not offer the field and explains why: it is the controller itself that lowers the power at corners according to the actual speed, and GRBL does not let you set a minimum for it from the G-code. To avoid the more burned marks at corners use the **Power ramp** and the **Lead-in to the contour** below.

### Outline: kerf and tabs (Line mode)

- **Kerf** (-5 to 5 mm): compensates for what the beam eats from the material. With a positive value the path shifts outward on parts and inward on holes, so the part comes out at the drawn size. It only affects closed outlines.
- **Tabs**: with **Leave tabs uncut** the layer leaves small stretches uncut so the part does not come loose. You define the **Tab width**, the **Count** per closed shape (or a fixed **Spacing** in mm; with 0 the count is used) and whether **Inner shapes are skipped**. Tabs appear as gaps in the preview. They are spread automatically, or you place them yourself with the **Manual tabs** tool (`U`): an object with manual tabs ignores its layer's automatic ones (see [Guides, tabs, start point and project](03e-guias-puentes-y-proyecto.md)). The **Tab power (%)** (right below) decides what the laser does over the tab gap: with **0** (default) the laser does not touch it; with a higher value it **scores the tab without cutting through it**.

![Tabs section of the layer editor](../../../apps/desktop/docs/screenshots/21-capa-puentes.png)

**Tab power.** It is an **absolute** percentage of the machine's power (from 0 to 100), not a percentage of the layer, and it **never exceeds the object's cutting power**: if you set 60 % on a layer that cuts at 40 %, the tab is scored at 40 %. It applies to all the layer's tabs, automatic or manual. In the preview, tabs with power are drawn as scoring and those at 0 as laser off. While scoring the tab the laser keeps the cutting speed, without slowing down. With perforation, the tab gap is also crossed with that power.

**Example: a part that comes out with a tap.** You cut 3 mm plywood at 80 % with 4 tabs. With **Tab power** at `0` the part stays held by four whole points that you have to break or cut with a utility knife. Try on a scrap with `10` or `15`: the laser grooves each tab without going through it and, if the value is good, the part comes loose with a gentle tap. The right value depends on the material and the laser: look for the lowest one that grooves enough, without perforating.

### Advanced cutting (Line mode)

The collapsible **Advanced cutting** section gathers the settings that change *how* each stroke is traced. It opens by itself if the layer already uses any (and then shows the **in use** badge). All of them start off, so they change nothing until you turn them on. Almost all apply only to **closed outlines**; open ones are cut as usual and the preview warns how many were left without the setting.

![Advanced cutting section of the layer editor with perforation, arc lead-in and pauses](../../../apps/desktop/docs/screenshots/33-corte-avanzado.png)

**Order in which they are applied.** First kerf and cutting order; then tabs; then dot mode or perforation; then overcut, lead-out and lead-in; then the ramp; and finally the pauses and cut-through. The resulting stroke is: lead-in, contour, overcut, lead-out.

| Setting | What it does | Ranges |
|---|---|---|
| **Perforation (cut and skip)** | Cuts one stretch (**Cut length**), skips another (**Skip length**) and repeats along the stroke. | 0.01 to 100 mm each stretch. Starts at 1 mm and 1 mm. |
| **Lead-in to the contour** / **Lead-out from the contour** | The laser starts (or ends) outside the part, not on the edge. **Shape:** **Straight** or **Arc**; **Length**; **Angle** relative to the contour (straight) or **Arc sweep** (arc). | Length 0.01 to 50 mm. Starts at 1 mm and 45°. |
| **Overcut** | When closing a contour, the laser keeps cutting this stretch past the starting point. | 0 to 50 mm. 0 = off. |
| **Pause at start** / **Pause at end** | Waits, with the laser on, at the start or end of each stroke. | 0 to 10,000 ms. 0 = no pause. |
| **Cut-through** | Starts each closed contour with extra power (**Start power**) for a few milliseconds (**Start duration**). | Power 0 to 100 %; time 0 to 10,000 ms. Starts at 100 % and 100 ms. |
| **Dot mode** | Instead of a continuous line, fires a dot every certain distance: **Shot duration** and **Dot spacing**. | Shot 0.01 to 1000 ms; spacing 0.01 to 50 mm. Starts at 1 ms and 0.5 mm. |
| **Power ramp** | Power rises at the start of each stroke and drops at the end: **Ramp length**, **Start power** (% of the layer's power) and **Outer contours only**. | Length 0.01 to 50 mm; start 0 to 100 %. Starts at 1 mm and 0 %. |

**Practical examples**

- **Perforation for folds in cardboard or card stock.** Draw the fold line on a separate Line layer, turn on **Perforation** and set 2 mm cut and 1 mm skipped (the field's summary says "3 mm pattern: 2 cut and 1 skipped"). The paper stays joined by little bridges and folds cleanly without tearing. Adjust the lengths with a scrap: more skipped length folds more easily but resists less.
- **Lead-in to the contour so no mark is left on the edge.** On starting, the laser leaves a slightly more burned dot (it stays a moment in the same place). With a 2 mm straight **Lead-in**, the mark ends up in the waste, outside the part, and the laser "enters" the edge already in motion. If the part has a very tight corner, try the **Arc** lead-in, which arrives tangent to the contour. On a hole the lead-in goes toward the inside of the hole, which is what gets thrown away. If the lead-in does not fit without crossing the shape, Tlaku shortens it (to half and then to a quarter) and, if it still does not fit, omits it and warns you.
- **Lead-out from the contour.** The same idea at the end: the laser moves away toward the waste instead of switching off over the edge. Combine lead-in and lead-out for a mark-free cut.
- **Overcut to close a cut properly.** In thick material, the point where the contour closes sometimes ends up with a thread uncut. With an **Overcut** of 0.5 to 1 mm the laser goes past over what is already cut and the closure comes out clean. It cannot exceed one full lap of the perimeter.
- **Pause at start and cut-through in thick wood or acrylic.** The laser takes time to go through the material; if it moves immediately, the start is left half done. A **Pause at start** of 50 to 200 ms gives it time. **Cut-through** does the same with extra power and only on closed contours, to go through without leaving an over-burned hole. When both are present, the start pause goes first and then the cut-through. **Pause at end** finishes off the last point.
- **Dot mode for marking or dotted perforation.** It is good for decorative dotted lines or for marking the spot for a manual perforation. Dot mode replaces the stroke: a 20 mm line with 0.5 mm spacing fires about 40 dots.
- **Power ramp for rubber stamps and bevels.** Instead of entering at full power at once, the power rises gradually at the start of each stroke and drops at the end, over the length you choose. With **Outer contours only** the ramp is applied only to the outside contours, so the part is beveled outward and the holes are not touched. It is the setting for rubber stamps and parts with a sloped edge.

**Combinations that do not work.** If two settings contradict each other, the more specific one wins and the other is ignored with a notice in the preview; the editor also marks the conflict:

- **Dot mode** with **Perforation** (the dots are already intermittent), with **Ramp** (each dot fires at the layer's power), with lead-in, lead-out or overcut (there is no line to lengthen) or with pauses and cut-through (each dot already has its pause). If one is already on, the editor blocks the other with the reason.
- **Lead-in, lead-out and overcut** do not apply to a shape with **tabs**, because it is no longer a whole closed contour.

> **Tip:** pauses use the `G4` instruction. If your controller counts time in milliseconds rather than seconds (Marlin, Smoothieware), change it in the **G-code dialect** of your machine's profile ([Connecting your machine](08-conectar-maquina.md)); a wrong unit makes the pause a thousand times longer or shorter. With dynamic power (M4), Tlaku switches to M3 only during the pause, because M4 turns the laser off when the head is still.

### Scanning (Fill and Image)

| Setting | What it does |
|---|---|
| **Interval** (0.01 to 5 mm) | Distance between scan lines. Less interval, more density and more time. Below you see the equivalent in DPI and in lines per cm: they are the same data (DPI = 25.4 / interval). |
| **Angle** | Direction of the lines, in degrees counterclockwise (0 = horizontal). |
| **Crosshatch** | Fills twice, the second at 90°. Fill only. |
| **Bidirectional** | Engraves back and forth: almost twice as fast. If the machine has backlash, turn it off so the lines do not misalign. |
| **Automatic overscan** / **Overscan** (0 to 50 mm) | Distance the head travels outside the part, with the laser off, to reach speed before marking. In automatic it is calculated from the speed and the acceleration. |
| **Angle increment** (-360 to 360°) | Degrees added to the angle on each pass. It only shows with 2 or more passes; with offset fill it is not used. |

**Example, angle increment.** To engrave wood with a lot of contrast, two passes with an **Angle increment** of 90° make the second cross the first, like a crosshatch but spreading the energy over two passes. With small increments (for example 15° with 3 passes) the scan streaks are spread out and are much less noticeable on smooth surfaces.

### Fill: style and grouping (Fill mode)

The collapsible **Fill** section has two selectors:

- **Fill:** **Scan** (parallel lines), **Offset** (concentric rings inward; then angle, crosshatch, bidirectional and overscan are not used) or **Flood fill**. *Flood fill* calculates a route that travels each region of the drawing separately without crossing the empty spaces; it saves a lot of travel in designs with many separate islands (loose letters, for example), but it is more sensitive to the machine's mechanical backlash.
- **Grouping:** **All together** scans all the layer's shapes at once (fastest); **Shape by shape** finishes each shape before moving to the next (good for slow engraving, to see one complete piece before the next, or in scattered designs); **By group** fills each document group together (the ones you make with **Group**).

### Advanced image (Image mode)

The collapsible **Advanced image** section has three settings, explained with examples in [Images](06-imagenes.md): **Send untouched (pass-through)** (the image is sent as is, without dithering or tone adjustments), **Edge enhancement** (radius and strength) and **Remove specks**.

### Z axis

The **Z axis** group (**Z offset** and **Step down per pass**, in mm) only appears if the active machine has a motorized Z axis, or if the layer already carries Z values. If there is no Z axis, those values are ignored when sending and the editor offers **Reset Z to 0**. More in [Rotary and Z axis](12-rotativo-y-eje-z.md).

## Additional operations (sub-layers)

Sometimes the same shape needs two different jobs: filling it and then cutting its outline. Instead of duplicating the drawing in two colors, the layer can carry **additional operations** (also called sub-layers): extra settings that run on **the same objects**, in order, right after the main operation.

![Additional operations section of the layer editor, with a line operation and a fill one](../../../apps/desktop/docs/screenshots/34-operaciones-adicionales.png)

They are managed in the editor's **Additional operations** section:

1. Choose the **Mode of the new operation** (Line, Fill or Image) and press **Add operation**. It starts with the layer's speed, power and interval.
2. Press its header ("Operation 1: Line") to edit it with the same fields as the layer.
3. With the arrows you move it up or down, and with the trash you delete it.

There is a maximum of **11 additional operations** (12 with the main one), the same as the engine admits. Operations use the layer's color, name, visibility and output switch: if the layer does not go out to the machine, none of its operations do. An object that an operation does not accept because of its mode (an image in a Line operation, for example) is ignored in that operation and the preview warns you.

**Example: fill and then outline.** Draw a closed logo on a **Fill** layer (engraving at 3000 mm/min and 25 %). Add a **Line** operation at 400 mm/min and 80 %, with **Lead-in to the contour**. The machine first engraves the interior and then cuts the edge, without you having to duplicate anything or keep two copies of the drawing in sync. Another use: a second low-power Line operation that goes over the edge and darkens it.

In the preview each operation appears as its own block, with a name such as "#ff0000 · 2/3" (its position among that layer's operations). In the layer list, the **+N ops** badge reminds you that the layer has operations. If automatic order is on, an operation is ordered by its own role (engraving or cut), but never before its layer.

## Priority and power per object

Besides the layer settings, each object can have its own **Cut priority** and **Power scale**. They are edited in the **Object properties** window (see [Drawing and editing](03-dibujar-y-editar.md)). Priority applies on **Line** layers; power scale, on **Line** and **Fill** layers (not on Image ones):

- **Cut priority:** an integer; lower is cut earlier, within its layer. Empty is automatic. It is good for cutting the small, delicate things first, or for making sure an outer frame is the last thing to come loose.
- **Power scale:** the percentage of the layer's power that this object uses (0 to 100; empty = 100 %). It is good, for example, for engraving a softer line inside a cut layer, or a lighter detail inside a fill, without creating another layer. In a fill, objects with different scales are filled separately (once per scale) and the preview warns you. If the result would go over 100 % it is limited to 100 % and you are warned.

Priority is only respected if the **Optimization settings** have it turned on (see [Preview and cut order](07-vista-previa.md)).

## Defaults for new layers

The **Defaults** group, at the end of the editor, has three levels. When you create a layer, the most specific one that exists wins:

1. **This color on this machine** (**Save for this color on [machine]**).
2. **This color on any machine** (**Save for this color (all machines)**).
3. **The general set** (**Save as defaults for new layers**).

If there is none, the layer starts with the factory values (1000 mm/min and 50 %). Each button takes this layer's settings (except the color, name, visibility, output and position); existing layers do not change. Saving again for the same color and machine replaces what was there. A line under the title says which level applies today for this layer and this machine. **Remove this color's values…** deletes the matching level, and **Reset** puts the general set back to the factory values (it does not touch the color ones). The machine is the active one in the machine list: if you switch machines, new layers start with what you saved for the new one. There are also commands (**Save the active layer as defaults**, **… for its color on this machine**, **… for its color (all machines)** and **Reset the defaults for new layers**) in **Edit > Edit keyboard shortcuts…**.

They are saved with the application's preferences, not with the project, and travel in the `.tlakuprefs` file. To carry specific settings for a material, use the [material library](11-materiales.md).

## Execution order

The layer list itself is the order, from top (first) to bottom (last). The number on each row is its position.

- **Automatic** (the default mode): engravings first (Fill and Image), then lines and finally cuts, so the part does not move before engraving is finished. A line layer counts as **Cut** if it has a kerf other than 0, or power of 80 % or more with several passes.
- **From file**: respects the list as it is.

Switch between the two with the **Automatic** / **From file** selector in the panel. To reorder, drag the row's handle, use `Alt+↑`/`Alt+↓` or **Run earlier** and **Run later**. **Pin to this position** (or **Pin the execution position** in the editor) forces a layer to occupy a place; two layers cannot ask for the same one.

A layer without a position does not run, and its label says why: **Guide**, **No output** or **No objects**. Hover over the label to see the explanation. How the path is ordered *within* each layer, and the options that change it (respecting layers, groups and priority, removing repeated stretches, hiding mechanical backlash), is in [Preview and cut order](07-vista-previa.md).

Next: [Images](06-imagenes.md)
