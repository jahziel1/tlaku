# Advanced shapes and warping

Tools for changing the shape of a drawing that is already made: trim where shapes cross, break apart, simplify nodes, round corners (also with one click), split with a blade, adjust joint slots, wrap with an outline, warp (also with handles on the canvas) and combine with a Boolean operations assistant. All of them are in the **Arrange** menu and each one is a single **Undo** step (`Ctrl/Cmd+Z`). Shortcuts are written with `Ctrl/Cmd`: `Cmd` on macOS, `Ctrl` on Windows and Linux, and they can be changed (see [Preferences, shortcuts and autosave](02b-preferencias-y-atajos.md)).

The basics of drawing, aligning and simple Booleans are in [Drawing and editing](03-dibujar-y-editar.md).

> **Heads up:** the operations on this page work on **vectors** (not images) and do not touch locked objects. A text must first be converted to paths (see [Text](04-texto.md)). If a command is grayed out, hover over it: it says why.

## Trim shapes

Removes the stretch of a stroke that lies between two crossings, like erasing the excess of a cross.

1. Select the objects that cross.
2. Choose the **Trim shapes** tool (`X`) in the palette.
3. Move the pointer over the stroke: the segment that would be deleted is **highlighted**.
4. Click. The segment disappears. With `Shift`+click the segment is **kept** as a separate object.

**Example: a cross made of two rectangles.** Draw a horizontal rectangle and a vertical one that cross, and select them. With `X`, click on each of the four segments that lie inside the other rectangle: only the outline of the cross remains, without interior lines that the laser would cut unnecessarily. (To join whole shapes, **Weld** is more direct.)

The **Trim at last click** command (`Ctrl/Cmd+K`) repeats the trim at the last point where you clicked on the canvas. Endpoints that touch count as crossings. The leftover pieces are not joined automatically between different objects; if you want them joined, use **Join paths…**.

## Break apart subpaths

**Break apart subpaths…** (`Alt+B`) turns an object that has several outlines into one object per outline. Curves do not change.

- **Keep holes with their outer outline:** a frame with its hole stays a single object; only loose pieces are separated. Use it when you want to move each piece but not take the holes apart.

**Example: a DXF with all the parts in a single path.** You import it and see that you cannot move one part without moving the others. Select it, **Break apart subpaths…** with the holes kept, and now each part moves on its own. The window tells you how many objects became how many.

It does not split into loose segments: the unit is the outline.

## Optimize shapes

**Optimize shapes…** (`Alt+Shift+O`) reduces a path's nodes without it straying from the original shape by more than the **Tolerance**.

| Setting | What it does |
|---|---|
| **Tolerance** | Maximum error allowed, in mm (0.05 by default). Between 0.01 and 0.05 it hardly changes the shape; high values distort it. |
| **Turn dense runs into smooth curves** | Fits curves to the runs without corners (a circle made of hundreds of segments) and merges them into one. |
| **Corner angle** | A vertex whose turn exceeds this angle (35° by default) is kept as a sharp corner. |

The dialog shows the **nodes before and after** and the percentage saved. It is useful for paths traced from a photo or DXFs exported with thousands of points: the file weighs less, the preview runs faster and the laser movement comes out smoother.

![Optimize shapes dialog with the tolerance, the corner angle and the node count before and after](../../../apps/desktop/docs/screenshots/47-optimizar-formas.png)

> **Tip:** start with 0.05 mm and look at the preview. If the result looks the same and the nodes drop to half or less, apply it. With **Turn dense runs into smooth curves**, a circumference of 400 segments becomes a few curves.

It does not fit circular arcs (it uses cubic curves) and does not treat a group as a single unit.

## Round corners

**Round corners…** modifies the corners where **two straight lines** meet. Curves are not touched.

| Type | Result |
|---|---|
| **Rounded** | An arc tangent to the two lines. The **Radius** (3 mm by default) is that of the arc. |
| **Chamfer** | A straight cut at the **Cut distance** from the corner, measured along each line. |
| **Bite** | A concave arc centered on the corner, biting into it. |

- If two corners compete for the same line, the radius is **reduced** so they fit; the window warns you. If they do not fit with any radius, they are left sharp.
- **Only the corners of the selected nodes:** with the **Edit Nodes** tool (`N`) you choose some nodes and round only those corners.

**Example: a card with rounded corners.** Draw an 85 × 55 mm rectangle, select it, **Round corners…** with 3 mm: the four corners get an exact arc. (For a single rectangle there is also the **Corner radius** field when drawing it.)

### Round with one click (`Shift+F`)

To touch **a single corner** without selecting nodes, use the **Fillet corner by click** tool (`Shift+F`, or **Tools**). When you activate it, a bar appears above the canvas:

- **Round** or **Revert** (the mode).
- The **corner type** (**Rounded**, **Chamfer** or **Bite**) and its **Radius** (or **Distance**, for the chamfer): the same values as in the dialog above.

With **Round** mode, move the pointer over a line-to-line vertex: a preview shows how it will look and a label says **Click: round this corner (radius 12 mm)**. **One click** rounds only that corner, and it is a single **Undo** step. If you click where there is no two-line corner, the tool says so (**There is no line-to-line corner there. Click on a vertex.**).

![The Click-to-fillet bar, one corner already rounded and the preview of the next one](../../../apps/desktop/docs/screenshots/66-redondear-clic.png)

**Reverting a corner.** With **Revert** mode (or **holding `Alt`** while clicking), a click on a **rounded** or **bitten** corner turns it back into a point. It works if the arc is **exact** (the one made by this tool or the dialog). **A chamfer cannot be reverted**: it is just one more straight segment and there is no way to know which the original corner was; for that, **Undo**. `Alt` also switches direction in the other mode: with **Revert** active, `Alt`+click rounds again.

## Cut shapes with a blade

**Cut shapes with a blade…** (`Alt+Shift+C`) splits shapes with another closed shape that acts as a blade.

1. Select the blade and the shapes you want to split.
2. In the dialog you choose **which one is the blade** (the list shows each object's role: **Blade** or **Shape**).
3. Decide whether to **Close the pieces with the cutting line** and whether to **Keep the blade**.
4. The window tells you how many shapes are cut and into how many pieces.

- **Closed** (default): each piece is closed by the cutting line. It is what you want if you are going to **fill** them.
- **Open:** only the outline is split, without adding the cutting line. It is the right choice for **Line** layers, where cutting the blade's line twice would burn it too much.

**Example: splitting a circle into two halves for a keychain.** Draw the circle and a rectangle covering its right half. With the rectangle as the blade, the pieces are: what is inside and what is outside. The pieces come out as loose objects, selected; they are not grouped into two groups. It does not split images.

## Resize slots

**Resize slots…** changes the width of joint slots so they fit the **real** thickness of the material. Every board you buy measures slightly different from what the label says.

| Setting | What it does |
|---|---|
| **Current slot width** | The width it has now (3 mm by default). The window lists the **widths found** in the selection, with how many slots there are of each. |
| **Search tolerance** | How much difference is allowed when recognizing a slot (0.15 mm by default). |
| **New width** | What it is changed to (3.2 mm by default): normally the measured thickness plus the clearance the joint needs. |
| **Which wall stays put** | **Neither: it grows from the center**, **The first one** or **The last one**. |
| **Notches, holes, tabs** | Which patterns are recognized: notches in the edge of a part and rectangular holes, and, if you tick it, tabs of the same width. |

**Example: a 3 mm wooden box.** You downloaded a design with slots for 3 mm, but your board measures 3.2 mm with the caliper. Select all the pieces, leave **Current slot width** at 3, type **New width** 3.2 and apply: the slots get wider. Check the preview of the count ("N slots would change.").

It only recognizes **sharp 90° corners** and works with polylines: the object's curves are flattened. If the change would leave an inverted edge on a part, that part is not touched and you are told.

## Rubber band outline

**Rubber band outline…** creates a new closed shape that wraps the selection, like stretching a rubber band around the pieces. It stays on the active layer.

- **Convex (tightest):** the convex hull, without going into the gaps.
- **Concave (follows the gaps):** the outline goes inward as long as its edges are longer than the **Maximum edge length** (20 mm by default). With a large value it ends up the same as the convex one.
- **Keep the original objects** (on by default).

**Example: a support for several loose pieces.** Select the pieces and create the convex outline with a little **Offset…** afterwards: you get the base of a plate that wraps them. It also works for drawing the box around a cutting zone.

## Tangent circle

**Tangent circle…** adds circles that touch shapes without crossing them.

- With **two** shapes (or lines): type the **Radius** (10 mm by default) and you get the circles of that radius that touch both.
- With **three**: the radius comes from the shapes (the circles that touch all three).

The dialog lists the **candidates** with their radius and center, shows them in the preview and adds the ones you mark as real circles. If there are none, try another radius. Circles that contain a shape touching it from the inside are not calculated.

**Example: a drain hole that touches two walls.** With the two walls as lines, ask for a 6 mm radius and choose the candidate that falls in the corner.

## Warp

**Warp…** puts the selection in an **envelope** of four corners and four curved edges and really deforms it: the curves are recalculated, not just their control points moved, with an error under 0.01 mm.

1. Select one or more objects.
2. Under **Quick warps** choose a preset and adjust the **Strength** (in %, 30 by default): **Arch up**, **Arch down**, **Bulge**, **Pinch**, **Perspective** or **Slant**.
3. If you want to fine-tune, open **Corners and edges (advanced)**: how much each corner moves in mm (X to the right, Y upward) and how much each edge bends at its midpoint (positive, outward).
4. Look at the **Preview** (the original shape dotted and the warped one solid) and press **Warp**. **Reset** sets everything back to zero.

**Example: an arched sign.** Type "CAFÉ", convert it to paths, and apply **Arch up** at 30 %: the word curves like a smile without being distorted. For text that is already curved along a circle there is also the **Bend** option in the text panel.

![Warp dialog with the presets, the strength and the preview of a warped star](../../../apps/desktop/docs/screenshots/41-deformar.png)

### Warp with handles on the canvas (`Shift+W`)

To warp by eye, **Warp on canvas** (`Shift+W`, or the **Edit on canvas** button in the dialog, which closes it and leaves the mesh over the design) puts a **control mesh** over the selection and warps the design **live** as you drag. First select vector objects; if there is no selection, the tool says so and waits.

A bar above the canvas chooses the type:

| Type | Handles | What it does |
|---|---|---|
| **Bilinear** | 4 corners | Moves the corners; the edges stay straight. Good for slanting or stretching a shape. |
| **Perspective** | 4 corners | Like a photo of a sheet seen from the side: straight lines stay straight. The four corners must form a convex quadrilateral. |
| **4×4 mesh** | 16 points | The surface passes through each point: it lets you bend, ripple and twist. |

![The 4×4-point mesh over the plate and the gear, with several points dragged; the original shape is dotted](../../../apps/desktop/docs/screenshots/65-deformar-lienzo.png)

- **Dragging a handle** warps live. **`Shift`** while dragging moves only in a straight line (horizontal or vertical). Changing type keeps the shape you have.
- **Enter** or **Apply** writes the result, with the curves recalculated exactly, in **a single Undo step**. While it calculates, the button says **Calculating…**.
- **Reset** returns the mesh to its place; **Cancel** or **`Esc`** leaves without changing anything (in the middle of a drag, `Esc` returns the handle).
- For very precise work, the **Warp…** dialog above, with exact millimeters at each corner, is still the way to type the numbers.

> **Heads up:** the live preview is a quick approximation; the final result is recalculated by the engine when you apply. The result **replaces** the originals; if you do not like it, **Undo**. A selection with no width or no height (a straight line) cannot be warped.

## Boolean assistant

The **Boolean assistant…** (`Ctrl/Cmd+B`) shows **the result** of each operation before applying it, in thumbnails.

1. Select two or more closed shapes.
2. In the dialog choose which is the primary shape (**A**); the rest form **B**.
3. Each card shows the result of **Union**, **A minus B**, **Intersection**, **Symmetric difference** and **Weld**, with how many pieces come out (or **No result**).
4. Click the card you want to apply.

![Boolean assistant with the cards for union, A minus B, intersection, symmetric difference and weld](../../../apps/desktop/docs/screenshots/40-asistente-booleano.png)

**Example: a key with a bow.** Draw the key's body (A) and a circle at the tip (B). On the cards you see at a glance what happens if you unite them (the key with a bump), if you subtract the circle (the key with the hole made) and if you intersect them (only the small shared piece). You click **A minus B**.

The individual commands (**Union**, **Subtract…**, etc.) still exist in the same menu and with their shortcuts (see [Drawing and editing](03-dibujar-y-editar.md)).

Next: [Copies, virtual arrays and nesting](03d-copias-y-nesting.md)
