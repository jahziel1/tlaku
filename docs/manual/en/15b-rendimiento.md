# Performance with large designs

What to expect from Tlaku when the design is not a sample plate but a map, an SVG exported from a mapping program, an 80-megapixel photo, or a DXF with a hundred thousand lines. The figures come from the project's stress tests (178 real and synthetic files, plus canvas tests with 1,000 to 100,000 objects). They are **measured with the application in simulated mode, with the emulator, and with Chrome painting in software**: the painting times are pessimistic compared with a real graphics card, and the engine's calculation is representative. None of this has been measured with a real machine, and your computer may be slower or faster.

## In one sentence

Up to **10,000 objects** the canvas has no noticeable cost; with **50,000** it stays smooth for looking around and moving one object; with **100,000** it works, but moving **everything** at once takes about 0.4 seconds and the application uses about 650 MB of memory. Calculating a map with tens of thousands of paths takes **a second or a little more**. An image of **60 to 100 megapixels** is imported without exhausting memory. What becomes the limit before Tlaku does is the **USB cable**: a very detailed job can ask for more lines per second than fit at 115,200 baud.

## The canvas

| Design | What it feels like |
|---|---|
| Up to 10,000 objects | Everything instant: loading, moving, zooming, selecting. |
| 50,000 objects | Loads in about 0.3 s; panning and zooming stay smooth. Moving the whole selection looks like about 7 frames per second and committing the move takes about 0.3 s. Acceptable. |
| 100,000 objects | Loads in about 0.6 s; a click takes about 0.2 s. **Moving everything at once takes about 0.4 s** (before the performance improvements, 0.57 s) and the application takes up about 650 MB. This is the point where you notice it. |
| A single path of 100,000 nodes | No measurable cost. |
| A single path of a million nodes | Painting is the most expensive part (about 0.4 s on load). A click used to take 0.3 s and after the improvements about 50 ms; dragging the whole path was the slowest (about 3 frames per second before the improvements). |

So that the canvas does not drag with designs like these, Tlaku paints with a **reusable bitmap** when there are more than about 12,000 visible segments, uses a **spatial index** for clicking, hovering, and rectangle selection (it does not walk through all the objects), and calculates only the **changes** when editing. Practical tips:

- **Hide or lock** what you are not going to touch, and work on a small selection: moving 10 objects out of 100,000 costs tens of milliseconds.
- A file with hundreds of thousands of objects is almost always a **drawing exported from a map or a CAD program** with one piece per stroke. If you can, join strokes (**Arrange > Join paths**) and simplify (**Optimize shapes**) before going on.
- **Review design** ([Review design](07b-revisar-diseno.md)) finds duplicate lines and extra nodes, which are what inflate these files the most; **Fix everything safe** cleans them in one step.

## Calculating the job (preview, start, export)

This is what takes time when you press **Preview** or **Start**. The time depends on the number of paths and nodes, not on the size in millimeters.

- **Maps and illustrations with tens of thousands of paths.** The cut order with **Cut inner shapes first** (cutting the holes before the outline) was the bottleneck: a topographic map with 35,413 paths and almost 400,000 nodes used to take about 30 s. Now it takes **1 to 1.5 s**, with the same order, the same estimated time, and the same path. A PDF with a few very long paths (about 300,000 nodes) went from 14 s to under 1 s.
- **Millions of loose lines.** **Reduce empty travel** with a million separate lines used to take about 40 s; now, about 5 s with the same result. With 100,000 lines, under a second.
- **Large images.** Tlaku reads the image **by rows or in strips** and reduces it to the engraving grid before processing it, so importing a 60 to 108-megapixel photo takes **less than 75 MB** (before, 1.3 to 2.3 GB) and calculating the job takes 2.5 to 6 s (before, 17 to 28 s). Large TIFFs and YCbCr ones load. More than 100 megapixels was not tested: a bitmap of that size at 0.1 mm is a job of millions of lines.
- **Estimated time.** Against the emulator, the estimate came out within a few percent of the simulated time on the real files. With a real machine it may vary more (real acceleration, the firmware); treat it as a guide.

## The G-code and the cable

A map with four million nodes produces **millions of lines and tens of megabytes of G-code** (the Germany map from the tests: about 4.3 million lines unoptimized, 3.0 million with the merging of nearly collinear cuts, which Tlaku applies by itself without changing the path).

At **115,200 baud** the cable passes about 11,500 bytes per second, about 600 lines per second of 19 bytes. A map at 1,000 mm/min can ask for 70 % of that, and **above about 1,400 mm/min the machine would run out of lines and stop waiting**, leaving marks on the material. That is why the **Start** dialog warns when the link cannot keep up (see [Sending jobs safely](09-enviar-con-seguridad.md)), with the speed to lower that layer to. With native USB or over the network the limit is much higher. Sending 250,000 lines over USB serial already takes minutes.

## What to do if your design is heavy

1. **Simplify before sending:** **Review design**, **Optimize shapes**, and **Remove duplicates** reduce nodes and lines without changing how it looks.
2. **Lower the speed of the most detailed layers** or use **Selection only** to test in parts.
3. **Use a fast link** if your controller allows it (native USB, network): the serial cable is the first thing to run out.
4. **Test with the preview** before starting and look at the estimated time: if it takes tens of seconds to calculate, the machine will also have a lot to read.
5. **Images:** you do not need more resolution than the layer's interval. **Review design** warns about images with **more resolution than needed**; reducing them saves processing time without changing the engraving.

## How it was measured

The tests live in the repository: the `tlaku-bench` program for the engine (`scripts/run-stress.sh`) and `apps/desktop/scripts/canvas-stress.mjs` for the canvas. The full results, file by file, are in `docs/pruebas-estres.md` in the repository. If you have a design that feels slow on your computer, **Help > Report a Problem…** with its size and what took time helps improve it.

Next: [Glossary](16-glosario.md)
