# Review design

Before sending a job it is worth having someone look at the design calmly: shapes that do not close, repeated lines, things outside the bed. **Review design** does that reading for you and tells you what it found, **without changing anything** until you press a button. It is a fixed analysis (it does not use artificial intelligence), so the same design always gives the same list. Shortcuts are written with `Ctrl/Cmd`: `Cmd` on macOS, `Ctrl` on Windows and Linux.

It opens with **Machine > Review design…** (`Ctrl/Cmd+Alt+R`). It is disabled if the design has no objects (**There are no objects to review.**).

![The Review design panel with seven findings: two serious (a layer with no output and an object off the bed) and several warnings, each with its buttons](../../../apps/desktop/docs/screenshots/64-revisar-diseno.png)

## How to read the panel

At the top it says how many things it found (**I found 7 things to review**) or, if there is nothing, **All good**. Each finding is a card with:

- A **title** with the count and the severity: **Serious** (worth looking at before sending), **Warning** or **Detail**. Serious ones come first.
- An **explanation** of why it matters, with the design's real numbers (the gap, the repeated millimeters, the extra nodes).
- Up to three buttons: **the fix** (if there is one), **Select** and **Ignore**.

The panel **keeps itself up to date**: when you move, delete or fix something, it rereads the design a moment later. **Review again** (the arrows button, at the top) forces a new reading. Each fix is **a single Undo step** (`Ctrl/Cmd+Z`), and if the design changed while it was being fixed, the panel says so (**The design changed while it was being fixed. Review it again.**).

## What it detects

There are twelve types of finding. The thresholds are the factory ones and cannot be changed from the interface.

| Finding | What it means | Fix |
|---|---|---|
| **Open paths that almost close** | An outline with a gap of **0.5 mm or less** between its ends: the laser would leave a stretch uncut and a fill would not cover the shape. | **Close** |
| **Duplicate or overlapping lines** | Repeated stretches or ones placed on top of each other (tolerance of 0.01 mm): the laser goes over twice, burns more and takes longer. It says how many millimeters of stroke are extra. | **Remove duplicates** |
| **Pending joins between paths** | Paths whose ends are **0.1 mm or less** apart but are not joined: the laser switches off and travels between them. | **Join** |
| **Tiny fragments** | Objects measuring **less than 0.2 mm** on a side: leftovers from an import that the laser cannot make properly. | **Delete** |
| **Extra nodes** | Objects from which optimizing would remove **2 nodes or more** without changing the shape (maximum change of 0.02 mm). | **Optimize** |
| **Objects off the bed** (**Serious**) | They lie **completely** outside the work area: the machine cannot reach them. | **Select** only |
| **Objects that stick out past the bed edge** (**Serious**) | A **part** lies outside the work area and would not come out right. | **Select** only |
| **Texts not converted to paths** | While they are text they depend on the font and cannot be joined or trimmed; taken to another program they can look different. If a text does not have an outline yet, it warns that it would not reach the machine. | **Convert to paths** |
| **Layer with no output that has objects** (**Serious**) | The layer has objects but its output is off: they will not be cut or engraved. | **Turn output on** |
| **Hidden layer with objects** | It is not visible on the canvas, but if its output is on it would still be sent to the machine. | **Show layer** |
| **Images with more resolution than needed** | The image has more than **double** the dpi the layer asks for: extra detail that is not noticeable and takes longer. It is only a warning. | None |
| **Images with low resolution** | The image has less than **half** the dpi the layer asks for: it will look pixelated. It is only a warning. | None |

Some rules worth knowing:

- Shape findings (close, join, duplicates, tiny, nodes) are calculated **layer by layer**: two different layers are not mixed.
- **Hidden objects**, layers with output off and **guides** are not part of the geometric analysis, and **locked objects** are not offered for fixes that change their path (they cannot be touched).
- Findings that are **select-only** (off the bed) are deliberately not fixed on their own: moving or deleting is your decision. The panel says so: **This is only a heads-up: there is no fix button.**

## Fix, Select and Ignore

- **The fix button** (**Close**, **Join**, **Remove duplicates**, **Delete**, **Optimize**, **Convert to paths**, **Turn output on**, **Show layer**) applies that fix to the finding's objects, with the same operations as the **Arrange** menu (see [Advanced shapes and warping](03c-formas-y-deformar.md)). When done, a notice says what it did (**Done: …**).
- **Select** selects the finding's objects on the canvas so you can look at them, move them or delete them yourself. It tells you how many (**3 objects selected**).
- **Ignore** hides the finding because you know it is on purpose (for example, a test layer that is turned off). Ignored ones stay at the end, under **Ignored (n)**, with **Show again** to bring them back. **Ignore only lasts while the application is open**: on restart, they appear again.

If only ignored findings remain, the panel says **All good** and reminds you that you ignored some.

### Fix everything safe

The **Fix everything safe** button does in a single step the fixes that **delete and move nothing**: **closing** almost-closed paths, **joining** those that almost touch and **removing duplicates**. It is a single **Undo** step. It does not touch tiny fragments, nodes, texts or layers: you choose those one by one. If there is none of that, it says **There is nothing that can be fixed safely.**

> **Example:** you import a DXF of a part and the preview takes longer than usual. **Review design** says **Duplicate or overlapping lines in 14 objects (320 mm of repeated stroke)** and **6 pending joins**. **Fix everything safe** leaves a single line where there were two and joins the outlines; the estimated time goes down and the cut comes out continuous. If you do not like it, `Ctrl/Cmd+Z`.

## The warning when starting a job

When you open the **Start** dialog, Tlaku reviews the design silently. If it finds something **serious** (objects off the bed, layers with no output that have objects), it adds a yellow line: **Review design found 1 thing worth a look before sending**, with a **Look** link that closes the dialog and opens the panel. The warning **does not block** sending: it is just a question so you do not find out after it has already cut.

![The start dialog with the Review design warning and the Look link](../../../apps/desktop/docs/screenshots/71-inicio-revisar-aviso.png)

## In simple mode

In the **Review** step of [simple mode](01b-modo-sencillo.md), the same findings are written in simple words (**Some lines sit on top of each other. The laser would go over them twice and burn too much.**) with a fix button (**Close them**, **Remove the repeats**, **Join them**, **Delete them**, **Simplify them**, **Convert them**, **Show it**) and **Show me** to select them. To avoid repetition, it leaves out layers with no output (in simple mode, turning off a color is your decision) and what that mode already warns about on its own (unconverted texts and objects off the bed).

## What it does not do

- **It is not a guarantee.** Its saying **All good** does not mean the job is well tuned: it does not look at speed, power or focus. For that there are the [preview](07-vista-previa.md) and the checklist in [Sending jobs safely](09-enviar-con-seguridad.md).
- It does not fix on its own what requires a decision (moving, deleting, turning layers off).
- It does not change the thresholds from the interface.
- It looks at the design as it is **now**: it does not remember what you saw before, except what you ignored in this session.

Next: [Cost and price per piece](07c-costos-y-precio.md)
