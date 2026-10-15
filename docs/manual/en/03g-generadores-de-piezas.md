# Part generators: boxes, trays, living hinges, gears, joints and the fit test

**Tools > Part generators…** draws laser-cut parts for you: a **finger-joint box** with dividers and legs, a **tray or drawer**, a **rounded box** with a flexible wall, a **living hinge**, involute **gears**, loose **joints** (dovetail and tab and slot) and a **fit test piece** to calibrate your machine. You enter the dimensions, watch the parts update live, and **Insert** drops them into the design as regular vector objects that you can edit, move or nest like any other.

Shortcuts are written with `Ctrl/Cmd`: `Cmd` on macOS and `Ctrl` on Windows and Linux. Dimensions use the unit from **Preferences** (mm or inches); everything is mm inside.

## The dialog

On the left you choose the **generator**; in the middle are its **parameters** (with the unit next to each dimension) and on the right the **preview**, which updates as you type. A value that does not work is marked in red with the reason (for example "the box is too small for fingers of this thickness") and **Insert** stays off until you fix it. Below the preview:

- **Cut layer**: the line layer that receives the cuts. By default the active layer (if it is a line layer) or the first line layer.
- **Layer for engraved labels** (only if you ask for labels): an existing layer, **No labels**, or a **new layer** ("Part labels", line mode, low power and higher speed so it **engraves without cutting through**). Check its power against your material.
- The generator's **warnings** (fingers narrower than the material, very thin bridges, and so on).

**Insert** adds one cut object per part (and one engraving object per labelled part), **all in one group**, in the active tab, inside the bed and **without covering what is already there**: parts are arranged in rows as wide as the bed and the set goes to the first free spot. If there is no free spot it lands in the corner and a notice says so. Everything is **one undo step**.

## Finger-joint box

Panels join with **alternating fingers**: on every edge, one panel has a finger where the other has a notch, and the depth is the material **thickness**. Dimensions come out exact.

- **Type**: *Closed* (6 panels), *Open* (no lid, 5) or *With sliding lid*: the lid slides in slots in the side panels and goes in from behind; the back panel is lower. A gap of two thicknesses is left in front of the lid (a block on each side panel holds up the strip above), and the side panels get a slot one thickness tall plus the **lid play**.
- **Dimensions are** *Outside* or *Inside*. With inside dimensions the finished box is two thicknesses larger per side (and, on the open one, one thickness taller; on the sliding-lid one, the inside height is the one **under the lid**).
- **Material thickness**: your sheet's. Measure it with calipers; a "3 mm" sheet often measures 2.8 to 3.2.
- **Finger width**: the one you prefer. Tlaku **adjusts** it on every edge to an **odd** number of equal cells, so fingers are symmetric and both ends of an edge belong to the same panel. A finger narrower than the thickness breaks easily: the generator warns you.
- **Fit clearance** (slot − finger): see below. **0** is nominal; positive is looser, negative is tighter.
- **Engrave each panel name**: labels every panel (FRONT, BACK, LEFT, RIGHT, BOTTOM, LID) on its **outer face**, reading correctly when you place the piece with that face up. If it does not fit between the fingers it is left out with a notice.

## Box dividers, plinth and legs

In the box, the **Dividers** section splits the inside into compartments.

- **Dividers across the width (split X)** and **across the depth (split Y)**: how many sheets the box has in each direction (0 to 10). With 2 and 1 you get six compartments. Dividers go the **full inside height** (in the closed box they reach the lid; in the sliding-lid box, up to the lid; in the open box, to the rim).
- **Compartment widths**: empty = all equal. If you type sizes (`40, 60`; `;` or spaces also work) they apply to the **first** compartments and the remaining ones share what is left equally; if you type **all** of them they must add up to the cavity minus the dividers (the field tells you how much). A compartment cannot be narrower than two thicknesses.
- **How they are held**: with *Tabs in slots* every divider has tabs, exactly as thick as the wall or base that receives them, that go into **slots** in the side walls, the base and the lid (if any); the tabs use the same finger layout as the box and always leave a plain stretch between the slot and the edge. With *Loose* there are no slots in the walls: every divider is shortened by the **loose divider clearance** in length and height and drops in between the walls (a dab of glue holds it if needed).
- **Half-lap crossings**: where an X divider crosses a Y divider, the X one loses its top half and the Y one its bottom half, in slots one thickness wide (plus clearance) and **exactly half the height** deep. You assemble them by sliding one into the other, and they neither overlap nor leave a gap. There are no tabs at crossings.
- The box **fit clearance** applies here too: slots are the tab (or the thickness of the crossing divider) plus that clearance, split between both sides. At 0 it is nominal; the layer compensates the kerf, just like the fingers.

The **Plinth or legs** section raises the box. It is a stack of **flat layers one material thickness thick** that you **glue** together and under the base (they have no joints, so the box does not change): *Plinth* is a frame set back one thickness from the edge of the base and *Four legs* is four squares per layer, one leg at each corner. The **height** is rounded to a number of layers (up to 10) and the notice tells you how many layers and how many mm you get.

## Tray and drawer

The **Tray and drawer** generator uses the same open finger-joint box (same edge rules, clearance and kerf) and adds handles and, if you want, a front and an outer case. Dimensions are those of the **body**.

- **Open tray**: five low-sided panels. With **Handle** set to *Rounded notch* the left and right sides get a U-shaped cutout with a rounded bottom, open at the top edge; with *Pill-shaped hole* it is a slot near the edge. **Handle width** and **notch depth / hole height** are validated so they do not touch the corner or base fingers.
- **Drawer with oversized front**: the body plus a **plain front** that overhangs on every side, top and bottom, by the amount set in **Front overhang on each side**. It is **glued on the outside** of the front wall, centred, and covers the opening of the cabinet. The handle goes on the front, and the same cutout goes through the body's front wall behind it so a finger can get in.
- **Drawer and outer case**: it also produces a case open at the front whose inside is the drawer plus the **drawer-to-case clearance** on every side and at the back; it is the open finger-joint box with swapped axes (five pieces: bottom, top, two sides and back). The drawer slides in through the front and its front stays outside; the generator warns if the front does not cover the gap (overhang minus clearance under 2 mm).

## Kerf and clearance: two different things

The laser removes material: the beam has a width (the **kerf**, 0.1 to 0.3 mm depending on machine and material). That is why a "perfect" joint in the drawing comes out loose or tight when cut.

- Parts are drawn **nominal**: fingers of the exact width and notches of the exact depth, no kerf.
- The **layer compensates the kerf**: in **Layers** (cut layer editor) there is a **Kerf** field. With a positive value the layer grows outer contours and shrinks holes and notches, so the **physical** piece comes out the drawn size. **For joints to fit, turn on kerf in the cut layer** with the value you measured (see "Calibrating"). The dialog reminds you, and if the layer already has kerf it tells you it is being used.
- The **fit clearance** is a fine adjustment of slot against finger drawn into the geometry (it widens the notches and narrows the fingers equally on both pieces). Use it **or** the kerf for the same effect, not both: the generator warns you if you set a clearance. The usual way: measured kerf on the layer and clearance 0; if the fit is a hair loose or tight, adjust the clearance by 0.02 to 0.05 mm.

## Calibrating with the fit test piece

The **Fit test** is a **plate with thirteen slots** of different widths and a **tab** (a tongue with a handle). Cut it with **the same layer and the same material** as your box and it tells you which clearance works for you.

1. Choose the mode:
   - **Slot against a cut finger**: the one for box fingers. Slots are the finger width plus a step (−0.30 … +0.30 mm with the factory values), and the tab is a finger of that width.
   - **Slot against the material edge**: for tab-and-slot joints. Slots are the **thickness** plus a step and the tab goes in **edgewise**.
2. Enter the **thickness** (or finger width), the **step** (0.05 mm is a good start; raise it if your beam is wide) and how many slots on each side (6 cover up to ±0.30 mm). Leave **Engrave the values** on: every slot gets its offset engraved (`-0.10`, `0.00`, `+0.05`…).
3. In the cut layer set the **kerf** you use (or 0 if you do not know it yet) and **Insert**. Cut with the **real layer and material**. Do not use that sheet for anything else first: thickness varies from sheet to sheet.
4. Try the tab in each slot. Look for the one it enters **with a slight press**: no forcing, no wobble, not falling out. Its engraved value is what you were looking for.
5. With finger mode you have two ways to apply the result (use **one**):
   - Enter the value as the box **Fit clearance** and leave the layer kerf at 0; or
   - Leave the clearance at 0 and set the layer **kerf** to **a quarter** of the value, without the sign. Reason: the Kerf field is how far each line moves (half the beam width), and in a finger-in-slot joint the beam eats material on both sides, so slot minus finger opens by **two beam widths**: if slot `-0.20` fits, your beam is about 0.10 mm and the layer kerf should be **0.05 mm**. Repeat the test with that kerf set: slot `0.00` should now fit.

A negative value that fits is normal: it means your cut leaves slots **wider** and fingers **narrower** than drawn. Write the value down per **material and thickness**: each one has its own.

## Living hinge

An area of **staggered cuts** lets wood, plywood or thin acrylic bend. Choose the **cut shape**: *Staggered straight cuts* (the classic pattern), *Wave*, *Zigzag*, *Brick* or *Diamond*; the **direction** (cuts run along X or along Y; the material curves the other way); the **size of the area**; the **length of each cut**, the **bridge** left uncut between two cuts, the **row spacing** and the **margin** without cuts next to the edge. With wave or zigzag, also the **height** and **wavelength**. Even rows start on a cut and odd rows on a bridge (half a step offset) and everything is symmetric about the centre.

- **Corner radius** rounds the outline; **Include the outline as a cut** removes it if you only want the pattern inside a piece you draw yourself (move the group where you need it).
- More bridges and wider-spaced rows = stiffer; longer cuts and closer rows = more flexible. Real flex depends on the material and the grain: **test a scrap** before cutting the final piece. Bridges under 1 mm or rows closer than 1 mm break easily and the generator warns you.
- **Brick**: staggered rows like the straight cuts, but each cut has a **short end tick** at each end (a ⊓ shape, like the joints of a brick wall). It also flexes when twisted. The tick is half the row spacing and leaves at least 0.3 mm of material against the neighbouring row; if it does not fit, the generator asks for more spacing.
- **Diamond**: a **lattice of diamonds**. Each diamond side is a cut and the bridge is left at the vertices. Here the **cut length** is the diamond width (its diagonal along the cuts), the **row spacing** is **half of its other diagonal** (taller diamonds with more spacing) and the **bridge** is the material that joins the diamonds at each vertex. If the bridge is so long that it eats the side, or the sides end up closer than 0.3 mm at a vertex, the generator marks the field and explains why.
- Every cut is an **open line** with no width; the layer kerf does not change them (the bridges already allow for the beam eating a little).

## Gears

**Involute spur gears**, ready to cut. Under **What to generate** choose: *One gear*, *Gear pair* (two that mesh), *Pinion and ring gear* (internal gear: the pinion turns inside a toothed ring), *Rack* or *Pinion and rack*.

- **Tooth size:** by **module** (mm; pitch diameter is `m·z` and outside diameter `m·(z+2)`) or by **diametral pitch** (teeth per inch: `m = 25.4/DP`). Module 2 and 20 teeth gives a 40 mm pitch / 44 mm outside gear. Two gears only mesh with **the same module and pressure angle**; the generator uses one for the whole set.
- **Pressure angle:** 14.5° (legacy), **20° (standard)** or 25° (sturdier tooth with a wider root).
- **Center distance:** for a pair it is `m·(z1+z2)/2`; for the ring gear, `m·(z2−z1)/2`. With **Meshed, as a single object** the parts come out already placed at that distance with one tooth in the other's gap (one object, to see the mesh; move the group wherever you need it). With **Separate** you get one part per gear, in rows, to cut or move independently.
- **Backlash** and **tip clearance:** the backlash thins each tooth by half the value at the pitch circle, so the pair has that play between flanks **without changing the center distance**; 0.1 mm is a starting point for 3 mm wood, but it depends on your kerf and machine (with the layer kerf on, the requested backlash is what you really get). Tip clearance (0.25 of the module by default) leaves the root free for the other gear's tip.
- **Tooth root:** with **fillet** (stronger) or **plain**. **Curve tolerance** (0.01 mm) is how far the polygon may stray from the exact involute; lower = more points.
- **Center bore:** round, with a **keyway** (width and depth) or a **D shaft** (one flat); 0 = no bore. The second gear of a pair has its own diameter. A bore leaving less than 1 mm of wall under the tooth roots is rejected.
- **Lightening:** **round holes** or **spokes** (openings between constant-width arms), in the number you ask for, between the hub and the tooth roots; **minimum material** sets the hub and rim wall and the arm width. If it does not fit, it is left out with a warning.
- **Ring gear:** the outer **rim thickness** is separate. **Rack:** number of teeth and body thickness under the root; the length is `teeth × π·m`.

**Warnings:** with few teeth there is **undercut** (the tooth root weakens and the gear may bind): the practical minimum is `2/sin²α` (17 teeth at 20°, 32 at 14.5°, 11 at 25°) and the warning suggests it. It also warns about very thin tips, a **contact ratio** below 1.2 (the pair meshes roughly) and a ring gear with fewer than 8 more teeth than the pinion. Teeth that end in a point or a bore that does not fit are errors and mark the field. The generator does not know your bed size: the dialog warns when the set does not fit.

Teeth lose accuracy with modules under 1 mm in wood or acrylic (the beam is about the size of the tooth): start with module 2 or more and cut a test pair first.

## Joints: dovetail and tab and slot

**Loose** parts to join two plates along an edge: use them as a test or in your own designs (the boxes already come with their fingers). Choose the **joint type**; a pair of plates comes out and is inserted like any other generator. The **edge length** and the **depth** of each plate are those of the sample piece; copy only the shape of the joint into your design.

- **Flat dovetail (same plane)**: both plates lie in the same plane, like jigsaw pieces. One plate has trapezoidal **tails** at equal spacing and the other the **sockets** of the same trapezoid. Parameters: **number of tails**, **neck width** (at the edge), **depth**, **angle** (how much the tail widens; 8 to 15° is usual, a larger angle locks more) and **clearance**. The clearance (socket − tail) leaves half on each side: with **0** the two plates together fill the whole rectangle with no gaps or overlaps. They assemble by **sliding** one plate over the other along the edge; the shape stops them being pulled apart sideways but not sliding: fix with a stop or a drop of glue. If the tails do not fit (no material left between sockets) or the socket goes past the plate depth, the generator flags it.
- **Tab and slot (perpendicular)**: one plate stands on edge on the face of the other. The first has **tabs** of the requested **width** and a length equal to the **thickness**; the second has the **through slots** of `(tab + clearance) × (thickness + clearance)`, at the **distance from the edge** you choose. The tabs are evenly spaced and centred. A tab narrower than the thickness breaks easily and the generator warns you.

Both use the **fit clearance** the same way as the box: with the kerf on the layer, leave it at 0; otherwise calibrate it with the **Fit test** (its "Slot against the material edge" mode is made for tabs).

## Rounded box

**Base and lid with rounded corners and a single wall** that wraps around and **bends at the corners** thanks to a living hinge, joined with fingers to the base and the lid. **Three parts** come out: *Base*, *Lid* (if you ask for it) and *Wall with hinge*.

- **Dimensions**: **outer width**, **depth** and **height**, **corner radius** (at least the thickness plus 2 mm and no more than half the shorter side) and **thickness**. **With lid** off leaves the box open (base and wall).
- **Wall length**: it is the **perimeter of the thickness mid-line** of the wall (the contour moved half a thickness inwards, with radius `R − thickness/2`): when the strip is curved that line neither stretches nor compresses, so it keeps its length. The outer contour is `2π × thickness/2` longer and the inner one shorter. The dialog tells you the exact length.
- **Hinge**: only on the **arcs** of the four corners; the straight sides stay flat and uncut. The cuts are vertical (parallel to the bending axis) with the pattern, length, bridge, spacing and margin you choose, as in the loose hinge (brick and diamond too).
- **Fingers**: the contour is split into an **even** number of equal cells along the mid-line. Even cells are **wall fingers** (they go into notches in the base and lid) and odd cells are plate tabs. On the curves the notches are ring sectors as wide as the thickness; their area equals that of the flat finger. The **clearance** widens each notch by half per side.
- **Wall seam**: it starts and ends at the centre of the front side. With **dovetail** (one or two, depending on the height) the two ends lock together; with **butt** they stay straight and are glued. The seam tails stick out past the length of the strip. If the straight stretch at the front is too short for the seam, the generator tells you.

**Important:** the corner hinge is **not tested in real material** with this geometry. Before cutting the box, cut a **test hinge** with the same pattern and check that it bends to the corner radius without breaking; a very small radius calls for longer cuts and thinner bridges. In simulated mode (without the engine) the rounded box preview is only **approximate**.

## Regenerate

Parameters are saved **in the project**. Selecting the group shows the **Generated parts [Regenerate…]** bar over the canvas (also **Tools > Regenerate parts…**): it reopens the dialog with the saved parameters and **replaces the parts in the same place** (same corner, same group). It works like parametric shapes: if you delete parts or edit their nodes, the link is broken and the dialog warns that regenerating redoes the whole set. A project saved by an earlier version opens just the same: whatever is missing takes its factory value.

## Limits

- Boxes are **flat panels with fingers**; rounded corners are the **Rounded box**, with a single wall with a hinge. Dividers and plinth use nominal sizes and clearance; the fit test gives you the value for your material. The loose joints are not yet applied to an edge of an object in your design: they come out as sample pieces with the length you ask for. The **sliding lid** is a simple solution: make sure the strip above the side panels (two thicknesses) holds up in your material.
- Geometry and fit are checked by automated tests, but the **real fit depends on your machine and material**: always cut the test piece and a small box first.
- Everything is computed on your computer; nothing is sent to the internet.
