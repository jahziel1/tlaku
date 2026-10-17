# Print & Cut

You print a design on paper, vinyl, or wood with your printer and you want the laser to cut **exactly** along the outline, even if you placed the sheet a little crooked or shifted on the bed. That is **Print & Cut**: some **registration marks** are printed along with the design, you tell Tlaku where they really ended up on the machine, and Tlaku moves, rotates (and if necessary stretches) the whole job so it lands on what was printed.

> **Warning:** Print & Cut was tested with the virtual machine and with a simulated printed sheet (a few millimeters off and slightly rotated). **This feature has not been tested with a real laser or a real camera** (other parts of Tlaku were tested on a Falcon2 Pro, but not Print & Cut). Do the first tests with scrap paper, the power at minimum or the laser disabled, and your hand near the emergency stop.

## When it is useful

- Stickers, labels, and photo frames: the printer puts down the color and the laser cuts the outline.
- Pieces that already come printed or engraved and have to be cut or engraved again in their place.
- Any job on two different machines (the printer and the laser) that has to line up.

It is not suitable for a material that moves between one mark and the next: **do not move the sheet while you measure**.

## Before you start

1. **Connect the machine and home it.** Print & Cut works with the bed's absolute coordinates, so Tlaku needs to know where the head is. When you accept the registration, the job switches by itself to **Start from: Absolute coordinates** and the app tells you.
2. **Do not move the job's origin** after measuring.
3. **With a rotary active you cannot measure with the laser** (the head position is no longer a flat point on the bed).
4. If you are going to measure with the camera, have the camera **calibrated and aligned** with the bed and its image visible on the canvas ([Camera](10-camara.md)).

## Step by step: some stickers

The example: a sheet of stickers with a red cut outline, to cut with the laser.

### 1. Design and add the marks

1. Draw or import the artwork and the cut outline (the outline goes on a line layer).
2. **Machine > Print & Cut…** opens the wizard, which floats over the canvas and does not block it: you can keep selecting objects while you use it. It has three steps: **Marks**, **Measure**, and **Result**.
3. In **Add registration marks** choose the **shape** (**Cross** or **Circle**) and the **count** (2, 3, or 4 marks) and press **Add registration marks**. Tlaku places the marks at the corners of the design, 8 mm from its edge: with 2 they go diagonally, with 3 in an L, and with 4 at all four corners.
4. The marks go on the **guides layer**: they are **not cut** and do not go out to the machine, but they are printed with the design. The farther apart they are, the more precise the measured rotation is.

If you already have your own marks (for example, those in a file someone sent you), instead of adding marks select on the canvas **one object or one group per mark** and press **Use the selection as marks**. You need 2 to 4, more than 10 mm apart from each other: if they are closer, the wizard does not let you continue because the rotation cannot be measured that way.

### 2. Print at actual size

**File > Print…** (`Ctrl/Cmd+P`) generates a 1:1 scale PDF and opens it in the system viewer; if you only want to print the artwork and the marks, choose **selection only** in the dialog. Print it at **actual size (100 %)** and **without fit to page** (see [Import and export](03b-importar-y-exportar.md)). If the printer scales the sheet, Tlaku will notice later as a scale different from 100 %.

### 3. Place the sheet and measure each mark

Put the sheet on the bed, **secure it** (tape or magnets), and go to the **Measure** step. Choose how to measure:

**With the laser.** For each mark:

1. **Go to the mark (approximate)** takes the head to where the mark would be if the sheet were exactly in its place.
2. Fine-tune with the **arrows** (with the same steps as the Machine tab) until the laser spot lands on the **center of the cross**. To see where the head is pointing use the **red pointer** if your machine has one or the **test fire at minimum power** (Machine tab). With the laser on: eye protection, and nothing flammable or reflective under the beam.
3. Press **Save position**. The wizard moves on to the next mark.

**With the camera** (if one is aligned and visible): press **Pick on the image** and click the center of each mark on the camera image. The position comes from the camera's alignment, so the measurement is only as good as that alignment.

### 4. Check the result

The **Result** step shows a table with **Design (mm)**, **Measured (mm)**, and **Error (mm)** for each mark, and the summary: **Mean error**, **Maximum error**, **Rotation**, **Scale**, and **Offset**.

Tlaku considers the registration **good** if the maximum error is **0.3 mm or less** and there are no warnings. If the error is low but there are warnings, it says to review them before accepting; if it is higher, it says to check the saved positions or **Measure again**.

If everything is fine, **Accept**. From that moment the preview, the framing, and the job come out moved according to the registration, and the wizard shows "Registration active" with the mean error, rotation, and scale. **Reset** (or **Machine > Remove the Print & Cut registration**) removes it and the job goes back to coming out where the design says.

### 5. Cut

Check the **preview**, which already shows the job moved according to the registration, **frame** with the laser off to confirm that the rectangle falls on the sheet, and send the job as usual ([Sending jobs safely](09-enviar-con-seguridad.md)).

## What the residual error means

After measuring, Tlaku looks for the transformation (an offset, a rotation, and, depending on the fit type, a scale) that brings **each design mark as close as possible to where you measured it**. The **residual error** is what is left over: the distance, in millimeters, between where each mark **should** be with the chosen transformation and where you **said** it was.

- A **small** error (below 0.3 mm) means your measurements and the printed sheet are consistent with each other: they can be described with a single move.
- A **large** error means **something does not add up**: you measured a mark badly, you moved the sheet between one measurement and another, the printer stretched the paper, or a mark is not the one you thought. Do not accept it: **measure again**. Look first at the mark with the largest error.
- **It is not the accuracy of the cut.** It is a measure of consistency. The cut can fail for other reasons (backlash, the center of the cross that you picked by eye), and a low error does not detect them.

### How many marks and what type of fit

| Fit type | What it corrects | Marks |
|---|---|---|
| **No scaling** | Offset and rotation only (assumes the print came out at actual size) | 2 or more |
| **Uniform scale** | Offset, rotation, and an equal scale in X and Y | 2 or more |
| **Affine** | All of the above, plus different stretching on each axis and skew | 3 or more, not in a straight line |
| **Automatic** | With 2 marks, uniform scale; with 3 or more, affine | — |

**An important detail with 2 marks.** With two marks and uniform scale, there is always an offset, a rotation, and a scale that join them **exactly**: the error comes out almost zero **even if you measured badly**. With two marks the error no longer protects you; look at the **scale warning** then and, above all, use **3 or 4 marks** to get a real check. With **No scaling** and 2 marks, the difference between the printed distance and the measured one is spread out as error, which does reveal an odd scale.

The warnings Tlaku gives:

- **Scale off from 100 % by more than 0.5 %:** the print may be distorted, the printer scaled the sheet, or one of the positions is bad.
- **Mirrored print:** the physical marks are in a different order than the design's. Check that you measured mark 1 on mark 1, and so on.
- **Stretched differently on each axis** (only with the affine fit): the paper deformed, or the printer prints longer in one direction.

## Limits

- **Images rotate with the registration.** If the sheet is crooked, the image to engrave is rotated with it (it is resampled, so with a large rotation it loses a bit of sharpness). Its mask moves with it. With a minimal rotation (less than 0.006°) it is engraved aligned to the axes and nothing changes.
- You cannot measure with the laser with the rotary active.
- The registration is part of the session's **job options**, not of the project file: it is not saved with it, and if you close the app or move the sheet, you have to measure again.
- It was tested with the virtual machine; with a real laser, start with the power at minimum.

Next: [Tiling and automatic feeder](09d-tiling-y-alimentador.md)
