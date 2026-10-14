# 3D slice engraving, cylinder, and cone

Three features for jobs that are not flat: **3D slice engraving** (a relief from a grayscale image), **cylinder correction** (marking on a mug or a bottle with a galvo laser, without a rotary), and **cone correction** (marking on a glass that widens).

> **Status: they live in the engine.** These three features exist in Tlaku's engine and have automated tests, but **there are no command-line flags for them**. In the app, **cylinder and cone correction** and split marking are in the **Galvo** group of a galvo machine's profile, and **3D slice engraving** is in the layer editor of a galvo machine (see [Galvo (experimental)](13c-galvo.md)). This page explains what they do and how they are calculated, so you know what to expect. **Nothing was tested with a real machine**, and cylinder and cone correction is a **geometric model of our own**, not verified with hardware.

## 3D slice engraving

### The idea

A **grayscale** image is interpreted as a **depth map**: **pure white** is the surface (not engraved) and the **darker** it is, the deeper. Instead of varying the power, the relief is achieved with **several passes** of black and white: each pass engraves only the points *darker* than a threshold, and the threshold goes from white to black in equal steps. The deepest points are engraved in more passes, so they end up deeper.

With **N slices**, pass `k` (starting at 0) uses the threshold `(N − k) / N`. With 4 slices:

| Slice | Threshold | What it engraves |
|---|---|---|
| 1 | 100 % | Everything that is not pure white |
| 2 | 75 % | The points darker than 75 % brightness |
| 3 | 50 % | Those darker than 50 % |
| 4 | 25 % | Only the very dark |

Pure black is engraved in **all four slices**: it is the deepest. A relief with 16 slices has 16 depth steps.

### Settings

| Setting | What it does |
|---|---|
| **Slices** | From 2 to 256. More slices, a smoother relief and a longer job |
| **Angle per slice** | Degrees added to the scan angle on each slice (0 = the same on all). A different angle on each pass spreads the lines and smooths the bottom |
| **Z per slice** | If the layer uses the Z axis with a step per pass, each slice goes down by that step: the focus follows the depth |

Each slice comes out as its own block in the job and in the preview (with the name "image · slice 3/16"). If the layer has pauses, they go only before the first slice and after the last. The deepest slices are often empty if the image has no blacks: Tlaku summarizes this in a single warning ("image: 5 of 16 3D slices have no point to engrave") and it is not an error.

### How to prepare the image

1. Work in **gray, 8 bits**. 16-bit depth maps are not yet taken advantage of: there are only 8 bits (256 levels).
2. **White = surface, black = bottom.** If your map is the other way around, invert the image.
3. A smooth gradient gives a better result than a photo with a lot of detail: it is a relief, not a photo.
4. Start with **few slices** (8 or 16) on scrap material and compare with the model.

This engraving **also works on gantry machines** (for example, a CO2 or a diode): in the engine, the slices are normal threshold passes. The galvo is not required to use it.

## Cylinder correction

### The problem

A galvo marks on a focus **plane**. If you put a mug in, the surface **curves** and moves away from the mirror at the sides: without correction, a 40 mm square comes out distorted, and out of focus toward the edges. With cylinder correction, you draw the design **flat** and Tlaku calculates where the beam has to land so that the mark ends up where you drew it on the surface.

### The data it asks for

| Setting | What it is |
|---|---|
| **Diameter** | The mug's, in mm |
| **Mirror distance** | From the galvo's mirrors to the nearest line of the surface (the top of a convex cylinder). With the piece **in focus**, it is your lens's focal distance |
| **Axis** | **X** if the cylinder runs along X (the surface curves in Y, like a rotary), or **Y** |
| **Inner surface (ring from the inside)** | To mark the inside of a ring instead of the outside |

The **cylinder's axis** passes through the **center of the field** (the center of the bed), and the coordinate that runs **along the curvature** is measured in **millimeters of arc on the surface**, not in flat millimeters. That means that, on a 70 mm mug (220 mm circumference), a design 60 mm wide occupies 60 mm *of surface* (a bit more than a quarter turn), not 60 mm of projection.

### An example with numbers

A mug 70 mm in diameter (radius 35 mm), with the mirrors 160 mm from the top. To mark a point at arc distance `c` from the center, Tlaku projects it in perspective from the mirror:

| Arc on the mug | Where the beam lands in the field | Scale along the axis |
|---|---|---|
| 0 mm (the top) | 0.00 mm | 100 % |
| 10 mm | 9.78 mm | 99.1 % |
| 20 mm | 18.29 mm | 96.6 % |
| 30 mm | 24.60 mm | 93.0 % |
| 40 mm | 28.23 mm | 88.7 % |

Notice that 20 mm of surface land 18.29 mm from the center: the beam has to go **less far** than drawn because the surface curves away. Near the top the difference is minimal; at the sides, each millimeter of surface takes up less and less of the field, until at about 49 mm of arc (almost 80° on this mug: there the tangent ray from the mirror grazes the surface) it can **no longer be seen from above** and points are clipped to the edge, with a warning. In practice, **do not mark beyond about 60° on each side of the top** (about 36 mm of arc on this mug): the beam arrives at a very grazing angle and the mark becomes weak and blurry.

With a **concave** cylinder the opposite happens: the surface gets closer to the mirror toward the edges, and 20 mm of arc land at 19.61 mm.

### What it does not correct

- It is a **geometric model of our own**: projection from a single mirror, without considering that there are two separate mirrors or the actual shape of the lens's field. **It is not verified with hardware.** Run a grid test on a cylinder of the same diameter and measure.
- It **does not correct the focus.** The laser focused at the top is out of focus at the sides; with very curved surfaces, use a large cylinder or a lens with more depth of field.
- To mark on a cylinder with a gantry laser, the way is the rotary: see [Rotary and Z axis](12-rotativo-y-eje-z.md).

## Cone correction

For a glass or a goblet that widens. You draw with the **larger diameter** and the width shrinks toward the thinner end.

| Setting | What it is |
|---|---|
| **Top end** and **Bottom end** diameters | In mm |
| **Length** | From one end of the cone to the other, along its axis, centered on the bed |
| **Axis** | The same as the cylinder's (if you use both, they must match) |
| **The top end is at +X or +Y** | Whether the "top" end is on the positive side of the axis |

At each position along the axis, Tlaku scales the curvature coordinate by the ratio between the diameter there and the larger diameter, so that the same angle covers fewer millimeters of surface where the cone is thinner. With a cone 70 mm at the top, 50 mm at the bottom, and 100 mm long:

| Position on the axis | Diameter | Factor on the width |
|---|---|---|
| Top end (+50 mm) | 70 mm | 100 % |
| Center (0 mm) | 60 mm | 85.7 % |
| Bottom end (−50 mm) | 50 mm | 71.4 % |

The cone **works only in millimeters** and, like the cylinder, **does not correct the focal distance**. It can be used alone (without cylinder correction): then it only scales the width; or together with it, and then the cylinder's local radius is the cone's at each point.

Back to: [Rotary and Z axis](12-rotativo-y-eje-z.md). See also: [Galvo (experimental)](13c-galvo.md). Next: [Supported controllers](13-controladoras.md)
