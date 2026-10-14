# Drawing and editing

How to draw shapes, select them, transform them, measure them and combine them. Everything you do can be undone (`Ctrl/Cmd+Z`) and is recorded in the **History** panel. Shortcuts are written with `Ctrl/Cmd`: `Cmd` on macOS, `Ctrl` on Windows and Linux; all of them can be changed (see [Preferences, shortcuts and autosave](02b-preferencias-y-atajos.md)).

## Drawing tools

What you draw goes to the **active layer** (the one marked in the Layers panel or in the color strip). If the active layer is an image layer, it goes to the first line or fill layer. While you drag, a label shows the live measurement.

| Tool | Key | How to use it |
|---|---|---|
| Draw Line | `L` | Click by click for a polyline. **Click and drag** when placing a node pulls out its handles and the next segment comes out curved (see "Bézier curves" below). A click on the first node closes the shape; so does `C`. `Enter`, double-click or right-click leave it open; `Backspace` removes the last point; `Shift` locks 45° angles. |
| Rectangle | `R` | Drag. `Shift` makes a square. |
| Ellipse | `E` | Drag. `Shift` makes a circle. |
| Polygon | `G` | Drag from the center. See "Polygon and star". |
| Star | `Shift+G` | Drag from the center. See "Polygon and star". |
| Text | `T` | Click on an empty spot to create a text (see [Text](04-texto.md)). |
| Measure | `M` | See "Measure". |
| Manual tabs | `U` | Click on an outline to add or remove a tab. See [Guides, tabs, start point and project](03e-guias-puentes-y-proyecto.md). |
| Start point | `Q` | Click on a closed outline to set where the cut begins; `Shift`+click reverses the direction. See [Guides, tabs, start point and project](03e-guias-puentes-y-proyecto.md). |
| Trim shapes | `X` | With crossing shapes selected, click on a segment to delete it. See [Advanced shapes and warping](03c-formas-y-deformar.md). |

With **Rectangle** active, the **Corner radius** field (in mm) appears in the numeric bar for the rectangles you draw next. There is an **Art library** with ready-made drawings (arrows, frames, hearts, boxes with joints...) under **Tools > Art library**: see [Guides, tabs, start point and project](03e-guias-puentes-y-proyecto.md).

### Bézier curves

The **Draw Line** tool also draws curves, like the pen of a vector editor:

- A **click** places a corner node; the segment arriving at it is straight.
- **Click and drag** when placing a node pulls out its handles: the outgoing handle follows the pointer and the incoming one is its mirror image, so the node is smooth and the next segment comes out curved. `Shift` locks 45° on the handles too.
- As you get close, the cursor **snaps** to nodes, midpoints, centers and intersections of what is already there (hold `Alt` to suspend it).
- Afterwards you can fine-tune any node with **Edit Nodes** (see below).

### Polygon and star

Choose **Polygon** (`G`) or **Star** (`Shift+G`) in the palette or under **Tools**, click on the center and drag: the distance to the pointer sets the **radius** and its direction the **rotation** (with `Shift`, the shape ends up with a point facing up). The parameters appear in the numeric bar before drawing and, once the shape is drawn, in the **Shape Properties** window:

| Shape | Parameters |
|---|---|
| Polygon | **Sides** (3 to 100), **Radius**, corner **Rounding** (mm) |
| Star | **Points** (3 to 100), outer radius, inner radius as a percentage of the outer one, and **Rounding** |

A triangle, a pentagon or an octagon is a polygon with 3, 5 or 8 sides.

The **Shape Properties** window (**View > Shape Properties**) applies to the selected rectangle, ellipse, polygon and star; with several shapes of the same type, the change affects all of them. As long as you do not edit a shape's nodes, it remains "a shape" with its parameters; when you edit nodes, join or trim, it becomes an ordinary path. **The parameters are saved in the project:** when you reopen it, the shapes are still editable. Scaling a shape changes its parameters (a scaled rectangle changes its width and height); if the scale is not proportional and the figure stops being regular (polygon, star or rounded rectangle), it becomes a path.

### Cut priority and power scale per object

With any object or selection (also several at once, and not only shapes), the **Object properties** window (the same as **Shape Properties**; open it with **View > Shape Properties**) offers two **Cut** fields. Empty means "automatic" and 100 % respectively; if the selected objects have different values, the field shows **Mixed**.

| Field | What it does | Example |
|---|---|---|
| **Cut priority** | An integer; lower = cut earlier, within its layer. It accepts `+1` and `-1` to add to or subtract from what each object already had. | On a sheet with fine details and large parts, give priority 1 to the large parts and leave the fine details without priority (it counts as 0, and lower is cut earlier): the details are cut first, while the sheet is still firmly in place. |
| **Power scale** | Percentage of the layer's power that this object uses (0 to 100; empty = 100 %). | A name engraved more lightly than the rest without creating another layer: that object with a power scale of 60 % inside the same engraving layer. |

- **Priority** only orders within the layer, applies on **Line** layers and **only counts if the Optimization settings respect it** (they do by default; see [Preview and cut order](07-vista-previa.md)). The precedence is always layer, group and priority.
- **Power scale** applies on **Line** and **Fill** layers (not on Image ones). If a fill has objects with different scales, they are filled separately, once per scale, and the preview warns you; if two objects with different scales overlap, the common area is engraved twice. A result above 100 % is limited to 100 % and also flagged; with 0 % the object is not filled.
- **Arrange > Cut earlier / Cut later** (`Alt+Shift+[` and `Alt+Shift+]`) subtract or add 1 to the priority of the selected objects (no priority counts as 0), and **View > Show cut priority** draws a small number on every object that has a priority, so you can check them at a glance.
- **Groups** (`Ctrl/Cmd+G`) also count when cutting: with **Finish each group before the next** in the [Optimization settings](07-vista-previa.md), the laser finishes a whole group before moving to another, and a layer's **By group** fill fills them together (see [Layers and cut settings](05-capas-y-ajustes.md)). The outermost group is used.

## Measure

With **Measure** (`M`):

- **Drag between two points** to see the **distance**, the **angle** and the **Δx** and **Δy** differences. `Shift` locks the angle to 0°, 45° or 90°.
- **Click on a shape** to see its data: the **Outline** (closed, open or mixed), the **Nodes**, the **Segments** (how many straight and how many curved), the **Size**, the **Perimeter** and the **Area** (with holes subtracted). If the shape is a circle, also its **Diameter** and its **Center**.

Measurements use the unit from the preferences (mm or inches). Measuring changes nothing in the design.

## Select

The **Select** tool (`V`) does all this:

- Click selects an object; `Shift`+click adds it to or removes it from the selection.
- Dragging over empty space draws a box. Left to right (solid line) selects only what is **completely inside**; right to left (dashed line), everything that **touches** the box.
- `Ctrl/Cmd+A` selects everything; `Ctrl/Cmd+Shift+I` inverts the selection.
- `Esc` cancels the drag in progress; with no drag, it clears the selection.

### Special selections

They are in the **Edit** menu (and several in the context menu):

| Command | What it selects |
|---|---|
| **Select All in Layer** | All the objects in the active layer (or the selected object's layer). |
| **Select Open Shapes** | Paths that are not closed. |
| **Select Open Shapes on Fill Layers** | Open ones that are on fill layers, where they cannot be filled. |
| **Select Shapes Inside the Selection** | What lies inside the selected shape. |
| **Select Smaller Shapes** | What has less area than the selection. |
| **Select Circles by Size…** | Circles whose diameter falls in a range; with the same value as minimum and maximum, an exact diameter (screw holes, for example). |

If nothing matches, Tlaku says so in a notice. Locked objects are not selected.

## Move, scale and rotate

- **Move:** drag the object, use the arrow keys (1 mm; `Shift` 10 mm; `Alt` 0.1 mm, adjustable in Preferences) or type **X** and **Y** in the numeric bar. `Shift` while dragging limits the movement to horizontal or vertical. If you grab near a node, midpoint or center of the selection, that point is the one that snaps.
- **Scale:** drag a handle. On the corners it keeps the proportions; `Shift` releases them; `Alt` scales from the center. In the numeric bar, **Width** and **Height** with the lock on scale both.
- **Rotate:** with a single object selected, a second click on it switches the handles to rotate mode. Drag the top knob; `Shift` rotates in 15° steps (the **Rotation step** in the numeric bar, adjustable); `Alt` rotates around the reference point instead of the center. Or type the degrees in **Rotation**.

Images can be moved and scaled but cannot be rotated (the rotation commands are disabled and say why): rotate the image in an editor before importing it.

### Quick rotations from the keyboard

They rotate the selection around its center, and repeat if you hold the key down:

| Command | Key |
|---|---|
| Rotate 90° counterclockwise / clockwise | `[` / `]` |
| Rotate one step counterclockwise / clockwise (15° by default) | `Shift+[` / `Shift+]` |
| Rotate 5° counterclockwise / clockwise | `Alt+[` / `Alt+]` |

`[` rotates counterclockwise and `]` clockwise. There are no 10° or 45° rotations from the keyboard: `.` and `,` are already the preview speed.

### Flip, mirror and two-point rotation

**Arrange** menu:

- **Flip Horizontal** (`Alt+H` or `Ctrl/Cmd+Shift+H`) and **Flip Vertical** (`Alt+V`): reflect the selection over the center of its box. `Ctrl/Cmd+Shift+V` is **Paste in Place**, which is why vertical uses `Alt`.
- **Mirror Across a Line** (`Alt+Shift+M`): creates a **mirrored copy** across a line that you mark, and keeps the original. There are two ways: select the objects and, last, a **two-point straight line** (which will be the axis); or, with only the objects selected, the command asks you to **click two points** (or drag) to draw the mirror line. `Esc` cancels.
- **Rotate and Scale by Two Points** (`Alt+T`): click on the **pivot**, then drag from a **reference point** to where it should end up. The selection rotates so that the reference point arrives there; with `Shift` it also **scales**. A label shows the degrees (and the percentage). `Esc` cancels.

### Transform switches

In **Arrange** there are switches to prevent accidents: **Allow Move**, **Allow Scale** and **Allow Rotate**. When off, dragging does nothing and a notice says where to turn them back on. **Scale from Center** and **Keep Aspect Ratio** fix those handle modes without having to hold a key.

## Draw order and locking

- **Draw order** (**Arrange** menu): the last item in the list is drawn on top, and that decides what you see when two shapes cover each other.

| Command | Shortcut |
|---|---|
| Bring to Front | `Ctrl/Cmd+PageUp` or `Ctrl/Cmd+Shift+]` |
| Bring Forward | `PageUp` or `Ctrl/Cmd+]` |
| Send Backward | `PageDown` or `Ctrl/Cmd+[` |
| Send to Back | `Ctrl/Cmd+PageDown` or `Ctrl/Cmd+Shift+[` |

  Commands that would change nothing (for example, bringing forward what is already on top) are disabled and say why.
- **Lock** (`Ctrl/Cmd+L`): the object cannot be selected with the mouse, nor moved, scaled, rotated or deleted. **Unlock All** (`Ctrl/Cmd+Shift+L`) frees all locked objects and leaves them selected so you can see which ones they were. Hiding individual objects is not possible yet; for that, use the layer's visibility.

## Edit Nodes

Choose **Edit Nodes** (`N`) and click on a path to see its nodes and handles.

![Node editor with a rounded outline, three nodes chosen and the node bar at the top](../../../apps/desktop/docs/screenshots/11-editor-de-nodos.png)

- Click chooses a node; `Shift`+click adds or removes; click on a segment chooses its two nodes; dragging over empty space chooses with a box.
- Drag to move the chosen nodes (`Shift` in one direction only, `Alt` without snapping). The arrow keys also move them and `Delete` removes them. The bar sets exact **X** and **Y**.
- Double-click on a segment adds a node without changing the shape.

The node keys only work with this tool and nodes chosen:

| Action | Key |
|---|---|
| Insert a node at the midpoint | `I` |
| Segment to line / to curve | `Shift+L` / `Shift+U` |
| Node to corner / smooth / symmetric | `Shift+E` / `Shift+S` / `Shift+M` |
| Break the path at the node | `B` |
| Join endpoints | `J` |
| Close path | `O` |

Texts and images have no nodes: convert the text to paths to edit it this way.

## Group

`Ctrl/Cmd+G` groups two or more objects; `Ctrl/Cmd+Shift+G` ungroups. A group moves, scales and aligns as a single object. Groups can be **nested**: grouping a group with another object creates a group inside a group. A click always chooses the outermost group, and **ungrouping removes one level at a time**: to touch a member, ungroup until you reach it. If you import parts with their holes loose, **Edit > Auto-group by containment** builds one group per part (see [Copies, virtual arrays and nesting](03d-copias-y-nesting.md)).

## Arrange menu

### Booleans

They work with **closed shapes** of vectors (and text with an outline); not with images or open paths (close them first with `C` or with **Close paths…**).

| Command | Shortcut | Result |
|---|---|---|
| Union | `Alt+U` | A single shape with all the area. |
| Subtract (A minus B)… | `Alt+D` | A minus the area of the others. A dialog lets you choose which one is A. |
| Intersection | | Only what they share. |
| Exclusive OR (XOR) | | What is in one shape but not in both. |
| Weld | `Alt+W` | Fuses overlapping shapes into one region. A shape that ends up entirely inside another counts as a hole. |

Intersection and XOR with exactly two shapes are applied directly; with more, they ask you to choose A.

If you are unsure which operation you need, the **Boolean assistant…** (`Ctrl/Cmd+B`) shows a thumbnail of each one's result before applying it (see [Advanced shapes and warping](03c-formas-y-deformar.md)).

![Subtract dialog: A minus B for choosing the primary shape](../../../apps/desktop/docs/screenshots/13-diferencia.png)

### Offset

**Offset…** (`Alt+O`) creates a parallel outline at a fixed distance. Positive grows outward (holes shrink); negative grows inward. Choose the **Corners** (**Round**, **Mitered**, **Beveled**) and whether to **Keep the original**. There is a preview in the dialog.

### Join, close and remove duplicates

They open a dialog with a **Tolerance** in mm (initial value in parentheses). They only work with vector paths: not with images or with unconverted text. Join and close also require at least one open path.

- **Join paths…** (0.05): joins open paths whose endpoints are within that distance, even if they belong to different objects (on the same layer).
- **Close paths…** (0.5): closes each open path whose end is near its start.
- **Remove duplicates…** (0.01): deletes repeated or overlapping segments in each layer, so the laser does not cut the same line twice.

### Align and distribute

The six align commands (left, horizontal center, right, top, vertical center, bottom) require at least two objects, or one if the reference is the bed. **Distribute** with equal spacing requires three. **Align relative to** chooses the reference: the selection, an object or the bed. **Center on bed** is `Alt+C`. **Align and distribute…** opens everything in one dialog.

### Match sizes

**Match Width** and **Match Height** give all selected objects the width or height of the **first** one in the selection. The **(keep proportions)** variants also scale the other side so they are not distorted. They require at least two objects.

### Arrays

- **Grid array…**: **Columns** and **Rows**, with **Spacing** measured between edges or between centers.
- **Circular array…**: **Total pieces** (including the original), **Total angle**, **Radius** and center; it can rotate the copies toward the center.

Both show a preview and warn if the array goes beyond the bed. The maximum is 2000 pieces, and you can **Group the result**.

These arrays create real objects. To repeat a single object without filling the document there is a **Virtual array…**, and to spread copies along a curve, **Copy along a path…**; there are also **Dock and move together…** and **nesting** (automatic arrangement of parts on the sheet). All of that is in [Copies, virtual arrays and nesting](03d-copias-y-nesting.md).

### More shape operations

Trim, break apart, optimize nodes, fillet corners, cut with a blade, resize slots, rubber-band outline, tangent circle and **Warp** are in [Advanced shapes and warping](03c-formas-y-deformar.md).

## Context menu

**Right-click** on the canvas opens a menu with the most-used items, without going to the menu bar. Each entry shows its shortcut and, if it is not available, is grayed out.

![Context menu on a selected object, with cut, copy, group, lock and the submenus for order, flip, align and select](../../../apps/desktop/docs/screenshots/28-menu-contextual.png)

- **On a selected object:** **Cut**, **Copy**, **Copy as SVG**, **Paste**, **Duplicate**, **Delete**, **Group**, **Ungroup**, **Lock** and the submenus **Draw order**, **Flip and rotate**, **Align** and **Select**; with a single object, also **Offset…**. With a text, **Edit text** and **Convert to paths** appear; with an image, **Trace image…**.
- **On the background:** **Paste**, **Paste in Place**, **Select All**, **Invert Selection** and, if there are locked objects, **Unlock All**, plus **Fit to Bed** and **Fit to Design**.

A right-click on an object that was not selected selects it before opening the menu. The menu does **not** open while you are drawing a polyline (there the right-click ends it), while you are dragging (nor a node) or during the two-point rotation or the mirror. In the layer list, each row has its own menu (the **…** button).

## Snapping

`S` turns it on or off. It attracts to the visible grid and to the **nodes, midpoints, centers and intersections** of the other objects, and to the edges and centers of the bed, within the distance you set in Preferences (8 screen pixels by default). The **Snap to grid** and **Snap to objects and bed** switches (bottom left of the canvas) choose what to snap to, and a different indicator marks whether the point is a node, a midpoint, a center or an intersection. `Alt` while dragging suspends it for that gesture.

## Copy, paste and undo

`Ctrl/Cmd+C`, `X` and `V` copy, cut and paste; **Paste in Place** is `Ctrl/Cmd+Shift+V`; `Ctrl/Cmd+D` duplicates. What you paste and duplicate comes out offset (5 mm by default, adjustable in Preferences).

Copying and pasting **between Tlaku and other programs** works with SVG: you can paste into Tlaku a drawing copied from Inkscape or Illustrator, and paste into them what you copy here. The details are in [Importing and exporting](03b-importar-y-exportar.md). **Undo** and **Redo** step through the last 200 steps kept by the **History**.

Next: [Importing and exporting](03b-importar-y-exportar.md)
