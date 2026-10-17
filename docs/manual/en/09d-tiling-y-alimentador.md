# Tiling and automatic feeder

**Tiling** splits a job that is **larger than the bed** into tiles that do fit, sends them one at a time, and tells you how to move the material between one tile and the next. It is useful for a 1.2 m sign on a 40 cm laser, a mural of interlocking pieces, or a roll of fabric. **LightBurn has no tiling**: there it is solved by hand, by splitting the vectors and registering each piece with Print & Cut.

The **automatic feeder** is a belt or a set of rollers that advance the material by themselves. Tlaku uses it inside tiling so you do not have to move the material by hand between tiles.

> **Warning:** tiling and the feeder were tested with the virtual machine and with automated tests. **These features have not been tested with a real machine** (other parts of Tlaku were tested on a Falcon2 Pro, but not tiling or the feeder). The tiles line up with material placed by hand or with a feeder: the precision of each joint depends on how well you place the material, not on Tlaku. Start with a test design on cardboard and with the laser at low power.

## When to use it

- The design **does not fit on the bed** in one of the two axes. Then the Start dialog and the **Job** section of the Machine tab show a notice with the design's and the bed's dimensions ("…: it does not fit in one go. You can send it in tiles") and the **Split into tiles…** button.
- Your machine has pass-through (a door or a slot so the material comes out the front or the back), or the material can be moved on the bed and fixed again.
- You accept that each joint will depend on how you place the material: for pieces that have to fit to a tenth of a millimeter, it is better for each piece to fit entirely on the bed.

If it only goes over by a little (a couple of centimeters), first check whether rotating the design 90° or shrinking it is enough.

## Step by step: a 600 × 300 mm sign on a 400 × 415 bed

1. Import the design and, if you only want a part, select it (tiling respects the job's **Selection only**).
2. **Machine > Split into tiles…** opens the panel (also the button in the notice above). It shows the size of the design and of the bed.
3. Adjust the options:

| Option | What it does | Default |
|---|---|---|
| **Tile width** and **Tile height** | Maximum size of each tile | The bed minus the margin on each side (390 × 405 mm with a 400 × 415 bed and a 5 mm margin) |
| **Overlap** | How much of the design is repeated at each joint | 5 mm |
| **Bed margin** | Space between the bed's edge and the tile (overscan needs it) | 5 mm |
| **Order** | **By rows** (always left to right) or **Serpentine** (each odd row reversed: less distance traveled) | By rows |
| **Alignment marks in the overlap** | Small crosses in the overlap zone, engraved in both tiles | On |

4. The canvas draws the **tile plan** and the panel says how many there are: here, **2 tiles (2 columns × 1 rows)**: the first covers 0 to 390 mm of width and the second 385 to 600 mm; the 5 mm from 385 to 390 are repeated.
5. **Start.** The wizard moves on to **Tile 1 of 2: place the material like this…** and tells you which area of the design must be on the bed and that its **lower left corner** must be at the origin plus the margin.
6. Place the material, check with a frame that it is where it should be, and press **Send tile 1**. When it finishes, the wizard moves on to tile 2 and asks you to place the material.
7. Move the material, **line up the crosses** (see below), and press **Send tile 2**. At the end it says **Done** and how many tiles were sent and how many were skipped.

Tiles are calculated in the bed's absolute coordinates: **home first** and do not change the origin between tiles.

## The overlap and the alignment marks

The **overlap** is the strip that is cut or engraved in **both** tiles. It is there so that a placement error of a millimeter or two does not leave a gap: whatever falls in the strip is repeated. But what falls in the strip is cut or engraved twice: in a cut it is barely noticeable, and in a fill engraving the strip ends up more strongly marked. Therefore:

- Use an overlap that is **small but larger than your placement error**: 3 to 5 mm is a reasonable starting point. With zero there are no crosses.
- For **cutting**, the overlap does not usually cause problems: the stroke already cut is not seen again. For **fill engraving**, the strip shows; try a lower power.

The **alignment marks** are small crosses (80 % of the overlap in size, with a maximum of 6 mm) placed in the overlap zones, **also engraved in both tiles**. They act as a reference: after moving the material, the second tile will engrave the crosses again, and if the material is well placed they will land **on top of those from the first tile**. When they do, the lines coincide; if you see two crosses, move the material until they join.

- The crosses are on their own layer ("Alignment marks", light blue `#00a0ff`, light engraving at 3000 mm/min and 12 % power). **They are engraved over the design**, so if they bother you, use a larger overlap and low power, or turn them off.
- If you already have a layer of that color, Tlaku uses your settings as they are; that way you can give them your own power and speed.
- To see where a cross lands without burning anything, use your machine's red pointer or framing.

The fine alignment of the material is **physical**: use stops, squares, or a template, and tape the material down. It is what gives the joint the most quality.

## Several columns: the guided pause

When the design is wider than the bed there are **several columns**, and the material has to be moved **by hand** from one to the next (the feeder only moves one axis). When a tile finishes and the next one is in another column or row, the wizard stops and opens a dialog, for example **"Move the material to column 2 of 2"**, which says which area of the design must sit on the bed and brings the **registration marks**:

- These are the alignment crosses that were **already engraved** in the overlap and fall inside the next tile. Each one is shown with its position in the design.
- **Bring the pointer** moves the head (laser off) to the point on the bed where that cross must land once the material is placed correctly; if your machine has a red pointer and its offset is in the profile, it is the pointer that goes there. Move the material by hand until the pointer lands on the engraved cross.
- **It is placed: send tile n** closes the dialog and sends the tile. **Skip** marks it done without sending it, and **Later** closes the dialog so you can place the material without hurry (the panel's **Send tile n** button is still there).

There are registration marks only if you used an **overlap greater than zero** and left the **alignment marks** on; otherwise the dialog says so and you will have to align to the edge of the material or to stops. With the **Serpentine** order there are fewer pauses: the first row goes left to right and the next one right to left, so when the row changes the next tile is in the **same column**.

## Sending in tiles

For each tile the wizard shows:

- **Tile n of total: place the material like this…**, with the area of the design that falls on the bed.
- **Send tile n**: sends it to the machine; while it goes out you will see **Sending tile n of total…** and you can **Cancel this tile**.
- **Skip tile n**: marks it as done without sending it (for example, if you already cut that tile).
- A **plan** where you tap a tile to **repeat it or jump to it**.

If a tile does not go out (error, cancellation, the machine is not ready), the wizard says **Tile n did not go out**, with the reason, and lets you **Retry tile n** or **Go back**. The tile stays pending; it is not lost.

The engine trims each tile before planning: strokes are split at the tile's edge (no line is left half done), fills are trimmed as a region, and images are masked.

## Resuming

If the app closes, the power goes out, or the connection is cut in the middle, the progress is saved on your computer. When you open **Split into tiles…** with the **same design, the same options, and the same machine**, the wizard offers **Resume** ("There is an unfinished tiled job (3 of 6 tiles done)") or **Discard**. The tile that was in progress asks for confirmation again before being sent. If you change the design or the options, the saved progress no longer corresponds and is not offered.

## Automatic feeder

If your machine has a feeder (a conveyor belt or rollers that move the material with a motor), you can tell Tlaku how to advance it and tiling will do it by itself between tiles.

### Setting it up

**Machine > Manage Devices…**, choose the profile, and find the **Automatic feeder** group:

1. Tick **I have an automatic feeder**.
2. **Advance G-code:** one command per line; `{mm}` is replaced with the distance of each advance. The default is:

   ```
   G91
   G0 Y{mm}
   G90
   ```

   (relative, move, absolute). Adjust it to the axis your feeder moves. Below you see **"With this advance it would send: …"** with the resulting G-code.
3. **Default advance:** the distance (300 mm by default) when no other is known. Between tiles the tile height minus the overlap is used.
4. **Save changes.**

Tlaku validates the G-code (it cannot be empty, it must contain `{mm}`, it accepts no other placeholder, and no line may be longer than 120 characters) and the advance (positive and at most 5000 mm). With a Ruida controller or a K40 the group does not appear: they do not use G-code.

### How tiling uses it

The feeder only advances the material between tiles **of the same column**: when the design fits across the width and is split along the length, or, with several columns and the **Serpentine** order, when going from one row to the next in the same column. When the next tile is in **another column**, a single-axis advance is not enough: the wizard stops with the [guided pause](#several-columns-the-guided-pause) and you move the material. With the by-rows order and several columns the panel warns "The feeder is only used when the tiles run in a single direction (one column)".

With a single column, the flow changes:

1. The first tile **does ask for confirmation**: you place the material and the wizard tells you "after that the feeder advances by itself with: …" and the exact G-code.
2. When a tile finishes, the app waits for the machine to return to idle, sends the advance (**Advancing the material…**), waits again, and sends the next tile **without asking for confirmation**.
3. If the advance fails ("The feeder could not advance the material" or "the machine did not return to idle"), the wizard stops at that tile and lets you **Retry**.

**Example.** A roll of fabric: a 300 × 1000 mm design on a 400 × 415 bed. With the 5 mm margin and the 5 mm overlap you get **3 tiles in one column** (405, 405, and 200 mm tall). Between the first and the second the feeder advances **400 mm** (405 minus 5 of overlap), and between the second and the third **195 mm** (the last one is shorter).

> **Before using it, test the advance.** Send a short advance by hand (for example `G91`, `G0 Y10`, `G90`) and check **which way the material moves**. The material has to advance in the direction that puts the next part of the design under the laser. If it goes the other way, change the sign in the G-code (`G0 Y-{mm}`). Tlaku cannot know how your feeder is mounted. Also check that the three lines leave the machine in absolute mode (`G90`) when they finish.

## Repeating a job with the feeder

It also works without tiling: to engrave the same design several times on a roll or a belt, in the Start dialog tick **Repeat the job and advance the material between repeats** (it only appears with a GRBL machine that has the feeder enabled and valid in the profile).

1. Set the **Repeats** (from 2 to 99) and the **Advance between repeats**. It starts with the profile's advance; **Use the job height** makes it equal to your design's height.
2. Below you see the summary with the G-code that will be sent between one repeat and the next.
3. **Start the repeats** confirms **only the first**: the others start by themselves when the material has finished advancing and the machine has returned to idle. A panel shows how many have run; **Stop** cuts the one that is engraving and does not let any more go out.

If an advance or a job fails, the batch stops at that point and the panel says why. As with tiling, **test the advance by hand first** and do not leave the machine unattended.

## What it does not do (yet)

- There is no tiling from the command line; it belongs to the app's wizard.
- With several columns the material is repositioned **by hand**: the wizard only guides you (pause and registration marks); it does not measure whether it ended up well placed. Check by eye that the crosses coincide before sending.
- It does **not** use the built-in feeder of a DSP controller (Ruida, Trocen): there is no documented way to request the advance from it, and moving an axis with invented commands is not safe. With Ruida and K40 the **Repeat with the feeder** section does not appear.
- It does not split whole shapes when choosing the tile's edge: a shape that crosses the edge is cut in two parts, one in each tile.
- None of this has been tested with a real feeder, nor with the material moved by hand between columns.

Next: [Remote control from your phone](09e-control-desde-el-celular.md)
