# Copies, virtual arrays and nesting

How to repeat parts and arrange them to make the most of the material: copy along a path, the **virtual array** (copies that are not objects), docking parts to each other, automatic grouping by containment and **nesting** (automatic arrangement). The commands are in the **Arrange** menu (the grouping one, in **Edit**). Each one is a single **Undo** step. The usual grid and circular arrays are in [Drawing and editing](03-dibujar-y-editar.md).

## Copy along a path

**Copy along a path…** spreads copies of the selection along another path, like beads on a necklace.

1. Select what you want to copy **and** the path (it can be open or closed).
2. In the dialog, **Which one is the path** says which object is the guide (the list marks each one's role: **Path** or **Copy**). By default it is the last one in the selection.
3. Choose the **Spacing**:
   - **By number of copies** (8 by default), or
   - **By distance** between copies (10 mm by default); the dialog shows the **path length**.
4. **Rotate to follow the path tangent** points each copy in the direction of the path; the **Extra rotation** is added to that direction. The shape is taken as pointing to the right.
5. **Keep the originals** (off by default): when off, the originals are removed and only the copies remain; when on, they stay where they were in addition to the copies.
6. The preview shows the copies on the path. The maximum is 2000 copies.

**Example: a garland of stars along a curve.** Draw a small star and a curve with the line tool. Select both, open the command, set **By number of copies** to 12 and turn off rotation so they all stay upright (or turn it on so they follow the curve). **Example: ventilation holes along an arc**: a 4 mm circle, a guide arc and **By distance** of 9 mm.

Images cannot be rotated: if you include them, turn off rotation or remove them. There is no progressive scale along the path.

## Virtual array

A **virtual array** makes a single object be engraved or cut several times **without creating the objects**. The object is still edited as one; the copies show as faint **ghosts** on the canvas and are generated when the job is sent.

1. Select **one** object (if it belongs to a group, ungroup it or weld it first).
2. **Virtual array…**
3. Choose the type:
   - **Grid**: **Columns**, **Rows** and the **Horizontal gap** and **vertical** gap between the edges of each copy (negative = they overlap). 3 × 3 and 5 mm by default.
   - **Circular (around a center)**: total copies, **Total angle** (360° by default), center X and Y and whether the copies **rotate** toward the center.
4. **Assign array.** To change it later, open it again (**Update array**).

To remove it or make it real there are two commands: **Remove virtual array** and **Convert array into objects** (each copy becomes a real object).

**Example: 40 coasters on a sheet.** Draw a single coaster (an outline, not a group), give it a virtual array of 5 columns and 8 rows with a 3 mm gap, and you have the complete job without 40 objects in the document. If you change the original's shape, all 40 copies change at once. With **Convert array into objects** you can touch a single copy.

![An object with a virtual array: the copies appear as dotted ghosts around the original](../../../apps/desktop/docs/screenshots/42-arreglo-virtual.png)

> **Heads up:** the canvas draws up to 600 ghosts at a time, but the job carries all the copies (up to 2000). **Fit to design**, the size of the selection and the out-of-bed warning **do not count the copies**: look at the [preview](07-vista-previa.md) to check that all of them fit on the bed. The copies of an image are engraved, but the canvas does not draw its ghost.

## Dock and move together

**Dock and move together…** sticks parts to each other without measuring them by hand.

1. Select the **reference shape** and the ones that will move. In the dialog, **Reference shape (stays put)** chooses which one stays still (the first by default); the list marks **Fixed** and **Moves**.
2. Choose the **Mode**:
   - **Slide to the edge**: each shape slides in the chosen **Direction** (left, right, up or down) until it touches the reference. Only the ones facing it advance.
   - **Pull toward the reference**: each shape advances in a straight line toward the center of the reference until it touches it.
3. **Gap**: the separation left when touching (it can be negative, so they overlap).
4. **Move all together**: they keep their relative position and move as a block.
5. The preview marks the reference and the new positions. Shapes that do not reach the reference stay where they are and you are told how many.

**Example: bring parts closer to save travel.** You see four loose parts to the left of a panel. With the panel as the reference, **Slide to the edge** to the right and a 2 mm gap: they end up stuck to the panel with 2 mm of separation.

It works with **bounding boxes** (not the real outline) and does not dock to the edges of the bed (for that there are **Align relative to** the bed and **Center on bed**).

## Auto-group by containment

**Edit > Auto-group by containment** forms one group for each part, with the holes and engravings that lie inside its outer outline, as the engine understands it (even-odd depth). An island inside a hole is a new part.

**Example: a DXF with 30 parts and their holes.** When you import it, everything comes in loose: moving a part leaves the holes behind. Select everything and use the command: each part ends up in its group with its holes, and you can now move it, align it or use fill by groups and **Finish each group before the next** (see [Layers and cut settings](05-capas-y-ajustes.md)). If you already know you need it, turn on **Group by containment** in **File > Import with options…** and the file arrives already grouped.

It requires at least two closed shapes. It uses the largest closed outline of each object; what is already grouped, open lines, texts and images are left out.

## Nesting: arranging parts

**Arrange > Nest parts…** places the selected parts to waste as little as possible: it is useful for getting many parts out of a sheet or fitting the design to a piece of material that has already been used.

### What goes in

- Each **loose object** or whole **group** counts as one part. Locked or hidden objects are not moved.
- **Copies**: in the parts list each one has its number of copies (or **Copies of all**). Up to 2000 parts in total.
- **Container**: **Bed** (with its size) or **Material: an object of the document**. The material object has to be a closed shape; it is not moved or nested, the parts fit inside its outline and avoid its holes.

### Options

| Option | What it does |
|---|---|
| **Spacing between parts** | Minimum distance between parts (1 mm by default). Set at least the width of your cut (kerf) plus a little margin. |
| **Edge margin** | Minimum distance to the edge of the container (0 by default). |
| **Rotations** | **None**, **0 / 90 / 180 / 270°** (default), **Every N degrees** (**Rotation step**, 15° by default) or **Free**. More rotations make better use of the material and take longer. |
| **Allow mirroring** | Parts can be flipped. Turn it off if the part has a good side. |
| **Use holes of other parts** | Small parts can fall inside the holes of large ones. |
| **Strategy** | **Gravity** (compacts toward a corner) or **Bottom left**. |
| **Time limit** | From 1 to 600 s (20 by default). It is the most time spent searching for a better arrangement. |
| **Seed** | A different number, a different variant of the arrangement; useful to test whether it comes out better. |

![Nesting dialog with the parts, the container, the options and the preview with the utilization](../../../apps/desktop/docs/screenshots/39-nesting.png)

### How to use it

1. Select the parts and open the command. The preview shows how they are now.
2. Choose the container and the options. Press **Start**.
3. While it calculates, the preview updates and the window shows the **utilization** (area of the parts against that of the container), how many parts are **placed** and the time. **Stop** cuts the search and keeps the best arrangement up to that moment; if it reaches the time limit, it tells you.
4. If some part did not fit, the window lists it (**Did not fit**). Change the container, the rotations or the spacing.
5. When you like it, **Apply**: the parts move (and the copies are created). It is a single **Undo** step. **Cancel** changes nothing.

**Example: 24 acrylic keychains on a 300 × 200 mm sheet.** Draw a keychain, select it, and in the dialog set **Copies** to 24, spacing of 2 mm, rotations of 0 / 90 / 180 / 270° and **Start**. In a few seconds you see the utilization (for example, 71 %) and the parts laid out; if two do not fit, switch to 15° rotations and try again. **Apply** and check the [preview](07-vista-previa.md).

**Example: making use of an irregular scrap.** Import or draw the outline of the scrap (for example, with holes from previous cuts) on a layer with no output, choose **Material: an object of the document** and nesting places the parts inside, avoiding the holes.

> **Tip:** the engine works with a slightly simplified version of each outline, but always a conservative one (never smaller than the real part), so the parts do not overlap and stay at least at the requested spacing. Leave a spacing larger than the kerf, and if the material has a grain, turn off mirroring and do not use free rotation: Tlaku does not know the grain, you decide which rotations are acceptable.

> **Heads up:** the arrangement does not change the cutting order; that is decided by the [planner](07-vista-previa.md) as usual. Nesting only places the parts, without splitting the job across several sheets.

Next: [Guides, tabs, start point and project](03e-guias-puentes-y-proyecto.md)
