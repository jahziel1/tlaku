# Sending jobs safely

The recommended order for sending a design to the machine: review the preview, choose what gets sent, place the job, frame, confirm, and watch. Tlaku blocks everything that could move the laser without you asking.

> **Warning:** none of this has been tested with a real machine. The entire workflow (framing, starting, pausing, stopping, alarms) was validated against the emulator and the app's virtual machines. Practice first with a **virtual machine** ([Connecting your machine](08-conectar-maquina.md)) and do your first real job on scrap material. Always keep your machine's physical emergency stop within reach.

## What Tlaku checks before transmitting

When you start or frame, the app queries the machine's state (`?`) and **transmits nothing** if it is not idle (**Ready**). In alarm, hold, door open, running, or any other state, it explains why and what to do (**The machine did not accept the job** appears in the **Job** section, and the message is also kept in the console). It also checks the G-code against the board's buffer and warns you if:

- laser mode is off (`$32=0`): this blocks sending a job, because the laser would stay on during travel moves;
- the profile's max S does not match the machine's `$30`: warning, the power would come out scaled;
- the firmware reports `nan` values in the work offsets: warning (Tlaku does not use G54 to G59).

The app **never** sends `$X`, `$H`, or `~` on its own: each one is requested with a button that explains the risk and asks for confirmation.

## 1. What gets sent

### What gets sent: selection only

In the **Job** section (and also in the Start dialog and in the preview bar) are the **What gets sent** checkboxes:

- **Selection only:** the preview, framing, the job, and the G-code export include only what you have selected on the canvas. It is useful for repeating a single piece that came out badly without cutting everything again, or for testing a detail on a scrap. It shows how many objects are selected. With nothing selected the checkbox is disabled and says why (**Select one or more objects on the canvas to use this option**). Anything hidden, anything on a layer with output off, and anything the layer does not accept because of its mode is not output either.
- **Use the selection's origin:** needs the previous one and only counts with **Current position** or **User origin**. When off, the job's anchor point is taken from the box of the **whole design**, and the selected piece is cut *where it belongs* relative to the rest (for example, to repeat piece 3 exactly where it was). When on, the anchor comes from the selection's box: the selected piece is placed as if it were the whole design (for example, to cut it on another scrap, starting where you left the head).

> **Tip:** if you change the selection, the preview recalculates by itself. Before starting, check in the Start dialog that the summary (layers, time, where it lands) is for the piece you want.

## 2. Placing the job: Start from

In the **Machine** tab, **Job** section, **Start from** decides where the design lands on the bed. The same section has the **Optimization settings…** button (how the job is ordered and traveled; see [Preview and cut order](07-vista-previa.md)) and the framing buttons.

| Mode | What it does |
|---|---|
| **Absolute coordinates** | The job comes out at the place on the bed where you see it on the canvas |
| **Current position** | It is anchored wherever the head is when you start (needs the machine connected) |
| **User origin** | It is anchored at the point you saved with **Set origin here** |

With the last two you also choose the **job point that is anchored**, from nine (a 3 by 3 grid): the four corners, the middle of each side, and the center. For example, with "center" the center of the design lands on the head.

Below you see an illustration of the bed with the job, the head, and the origins, and a sentence with coordinates: "The job will cover X from 60.0 to 280.0 mm and Y from 70.0 to 300.0 mm of the bed." If anything goes off, it says on which side and by how many millimeters ("It goes off the bed: on the left ...") and the start dialog will not let you confirm.

### Moving the head and setting the origin

1. In **Move the head** use the arrow pad. A click moves one step (by default `0.1`, `1`, `10`, or `50 mm`); holding it down moves continuously and stops when you release. With focus on the pad, the arrow keys move one step, `Shift` multiplies it by 10, and `Alt` divides it by 10. From any panel: `Ctrl/Cmd+Alt+arrows`.
2. Adjust the **Speed** (in mm/min; it cannot exceed the profile's max speed).
3. When the head is where you want the job to start, press **Set origin here**. Only the app stores it; nothing is sent to the machine. **Clear the user origin** removes it.
4. The pad's center button, and the **Go to work origin (X0 Y0)** button, take the head to the work zero or, if there is a user origin, to that origin (the button then says **Go to user origin**).

**Home ($H)…** is for machines with limit switches; before moving it reminds you to clear the area. If the profile has **Offer homing ($H) when connecting** ticked (see [Connecting your machine](08-conectar-maquina.md#profile-settings-worth-knowing)), connecting opens that same confirmation: **it is never sent on its own**, and you can cancel it.

Below the pad, if the profile has a Z or U axis, their **Up** and **Down** buttons appear; there are also **Focus**, the **Saved positions**, and the **Test fire** (which turns the laser on and asks for double confirmation). All of that, and the **Macros**, is in [Advanced machine control](09b-control-avanzado.md).

### Positioning the laser with a click

**Position the laser** (button in **Move the head**, or the **Machine** menu) saves you from clicking the pad ten times: while it is active, **a click on the bed on the canvas sends the head to that point**, with the laser off (`M5`), at the **Move the head** speed. The cursor changes to a crosshair. It is useful, for example, to bring the head over a corner of the workpiece and set the origin there, or to check a camera mark.

- It turns off with `Esc`, by pressing the button again, or when the machine disconnects; and it turns itself off when a job starts, so a click cannot move the head in the middle of a cut. While it is active, the canvas does not select or drag anything and a label next to the cursor shows the point's coordinates.
- It needs the machine connected and idle, and a flat bed: **with the rotary active it is disabled**, because Y is the workpiece's rotation and not a point on the bed.
- If you click outside the bed, it warns you ("The point X …, Y … is outside the bed") and nothing moves.
- **Bring the red pointer to the point.** If your machine's profile has a **Red pointer offset** (the pointer does not land exactly on the beam; see [Connecting your machine](08-conectar-maquina.md#profile-settings-worth-knowing)), the **Bring the red pointer to the point** checkbox appears next to the button. With it ticked, the click sends the **pointer**, not the beam, there: the head goes to the point minus the offset. It is what you want for putting the pointer on a corner of the workpiece. With the offset at 0 and 0, the checkbox does not appear.

### Move steps and finish position

- **Move steps:** the field under the pad stores up to six values between 0.01 and 500 mm, separated by `;` or spaces (for example `0.05; 0.5; 5; 25`). They are remembered between sessions. The circular-arrow button returns to the default steps. Useful if you adjust focus with 0.1 mm steps or center a large piece with 100 mm steps.
- **Finish position:** decides what the head does when the job **finishes successfully**: **Stay** (the default) where it ended, **Return to origin** (to the user origin if there is one; otherwise to the work zero), or **Go to a point** (X and Y in bed mm, as on the canvas). **Go to the finish position now** makes that move by hand, with the laser off. If the job is cancelled or fails, the head does not move on its own. If your machine's profile already sends the head to the origin at the end, the app reminds you: change it in the device manager if you prefer it to stay. Example: **Go to a point** with X = 0 and Y = the bed height leaves the head in the corner, away from the workpiece, so you can take it out safely.

## 3. Framing

**Frame** (`F`, or the top-bar button) traces, with the laser **off**, the rectangle that the whole job occupies, overscan included. It is for checking that the design lands where you want and within the material. It runs at the speed you have in **Move the head**, with `M5` and `S0`.

If the profile has a **Red pointer offset**, framing with the laser **off** compensates for it automatically: so it is the **pointer**, not the beam, that traces the job's edge. (Framing with the laser on at low power does not compensate for it, because there what you see is the beam.)

**Frame outline** (button next to Frame and in the **Machine** menu) is "rubber band" framing: instead of the rectangle it traces the job's **convex hull**, like a rubber band stretched around the pieces, also with the laser off and at the same speed. It is better for long, diagonal pieces, where the rectangle travels over a lot of empty bed and does not show where the piece will really land. It still counts the overscan and respects **Selection only**, the job origin, and the rotary.

> **Tip:** the rectangle is for the whole set, not for each piece. And even though the laser is off, keep your safety glasses on and your hands clear. Framing can also be repeated in a loop until you press **Stop** and, if your machine's dialect allows it, traced with the laser on at very low power (up to 20 %) to see it on the material: see [Advanced machine control](09b-control-avanzado.md).

## 4. Starting

1. Press **Start** (`Ctrl/Cmd+Enter`). It never goes out directly: the **Laser ready and area clear?** dialog opens.
2. Read it carefully. There are three fixed warnings, to be read: nothing is in the way of the head's travel; you are wearing the right safety glasses for your laser; you will stay next to the machine, with a fire extinguisher nearby, until it finishes.
3. Below is the **Before sending** **checklist**, with checkboxes that you tick yourself: **Air assist is connected**, **Focus is set**, **The material is secured**, **The extractor is on**, and **The lid is closed**. With **Configure** you choose which ones appear (useful for removing "Air assist is connected" if your machine has no air) and turn on **Require it to be ticked before sending**: then **Start job** is not enabled until you tick all the visible checkboxes, and it says how many are missing. Without that option (the default) the checklist is optional. The checkboxes are cleared for every job, so they do not get ticked out of habit; what you choose to show and require is remembered. **Show all** puts them back.
4. The dialog summarizes the **Estimated time**, the **Layers** (power, speed, and passes of each), the **Max power**, and where the job will land, plus the calculation's warnings.
5. The dialog repeats **Selection only** and **Use the selection's origin**, in case you want to change them before starting.
6. **Cancel** is the button with focus. **Start job** is enabled only when the summary has been calculated and the job fits on the bed; otherwise it says "Wait for the job summary and fix what is marked in red before you can start."

Among the calculation's warnings, four can show up that are worth understanding:

- **Review design found N things worth a look before sending** (with a **Look** link): something serious in the design, such as objects off the bed or a layer with no output. It does not block sending. See [Review design](07b-revisar-diseno.md).
- **The serial link cannot keep up.** With USB serial (GRBL, Marlin, Smoothieware), the cable carries at most `baud rate ÷ 10` bytes per second (at 115,200 baud, about 11,520). If at its densest stretch the job asks for more lines per second than fit (very fine curves at high speed, or a bitmap with millions of segments), the machine **runs out of lines, pauses, and can leave marks** on the material. The warning says how many lines per second it asks for, how many fit, and to **what fraction of the speed** that layer would have to be lowered. What to do: lower the speed of the layers with the most detail, simplify the design (**Optimize shapes**, [Review design](07b-revisar-diseno.md)), or use a faster link (native USB, network, or a higher baud rate if your firmware supports it). It does not block sending; it is also noted in the console. Tlaku also **merges nearly collinear cuts** in the G-code (with a tolerance of about 6 µm) to write fewer lines without changing the path.
- **Layer faster than the machine.** If a layer asks for, say, 48,000 mm/min and the profile's max speed is 12,000, the warning says the machine will run at its maximum and the result will come out **darker or more burned** than expected. Lower the layer's speed or fix the profile's max speed if it is set wrong. It does not change the output: the controller limits the feed on its own.
- **Ruida warnings** (cap of 36,000 mm/min and more than 128 layers): see [Ruida and K40](13b-ruida-y-k40.md).

![Confirmation dialog with the job summary](../../../apps/desktop/docs/screenshots/05-maquina-iniciar.png)

![Start dialog with the Before sending checklist and the Selection only and Use the selection's origin checkboxes](../../../apps/desktop/docs/screenshots/36-checklist-inicio.png)

During the job you see the progress (percentage, **Lines**, **Elapsed**, and **Remaining**) in the **Machine** tab.

> **With a Ruida controller** (experimental) the dialog is different: it says whether the job will be **saved as a document** in the controller's memory (the normal case, and it **does not start**: you start it from its panel) or whether it **will run as soon as it arrives**. See [Ruida and K40](13b-ruida-y-k40.md).

> **If the design has variable text:** before sending, Tlaku resolves the variables with the values of that moment. If any of them is not understood, the job **does not go out** and the warning says which one. When it finishes successfully, the serial number and the CSV row advance on their own (if you turned that on); a cancelled or failed job does not advance. See [Variable text](04b-texto-variable.md).

## Pausing and stopping

| Action | What it does |
|---|---|
| **Pause** (`Ctrl/Cmd+Shift+P`) | Feed hold: stops the movement. The same button becomes **Resume** |
| **Stop** (`Ctrl/Cmd+.`) | Feed hold, waits for it to brake, and soft reset of the controller; the job ends and the laser is left off |

The app indicates that the laser is off while paused, but that has not yet been verified with a real machine: during a long pause, do not leave a diode laser focused on the material.

If **Stop** happens while the machine is moving, the position may have been lost. **The position may have been lost** appears: with limit switches do **Home ($H)…**; without them, set the origin again by hand. With Marlin and Smoothieware, cancelling always marks the position as possibly lost.

If you close the app with a job running, it is cancelled in the same way, but Tlaku keeps the checkpoint and offers to continue when you open it again (see [Continue an interrupted job](#continue-an-interrupted-job)).

## Live view of the job

While a job runs, the **Live view** checkbox (**Machine** tab, **Job** section) opens a **floating window** ("picture in picture") inside the app that shows what the machine is doing, **without a camera**: the **whole bed** with what has already been cut or engraved in dark (its layer color, charred), what is left faint (cuts as a dashed line), the head as a crosshair and, while the laser is emitting, a glowing red dot. It **does not cover or change your design**: the canvas stays exactly the same underneath (with the usual editing rules during a job). Below the bed the window shows the **percentage**, the **state** (running, paused, finished; with the **laser on** or **off**) and the **elapsed** and **left** times.

- **Move and adjust it.** Drag the **title bar** anywhere inside the app window (it cannot leave it) and resize it from the **bottom right corner** (it has a minimum size). **Enlarge** takes it to almost the whole canvas area and **Restore size** puts it back where it was; double-clicking the bar does the same. With the keyboard, focus the bar and use the **arrow keys** to move it and **Shift + arrow keys** to resize it. It remembers its position and size between sessions and rearranges itself if you shrink the app window.
- **Look at the bed your way.** The bed fits itself in the space of the window. Inside it, the **mouse wheel** zooms in and out, **dragging** pans it, and the **Fit the bed** button (or the **0** key) brings it back whole; with the keyboard: **+** and **-** to zoom and the **arrow keys** to pan. This zoom belongs to the window only: it does not move the design's zoom.
- **Closing** the window (the **X**) turns the live view off: it is the same checkbox as in the **Job** panel.
- **Open in a separate window.** The title bar button opens the live view in **another system window**, for example to put it on a second monitor, and the floating window hides while it is open. **Back to the floating window** (or closing it) brings it back if the view is still on. There is only one separate window at a time. It is **read only**: it has no access to the machine or to your files, it only receives progress and the path from the main window.

- **Off by default**, and your last choice is remembered. When off it does not locate or animate anything. You can turn it on and off **at any time, also in the middle of a job**: when you turn it on it shows everything already done at once.
- **It follows the real machine, not the clock.** It takes the position the machine reports (about 4 to 5 times a second), finds it on the preview path respecting the profile's origin, X/Y swap and rotary, and moves the head smoothly between reports. If the machine is slower or faster than the estimate it corrects without jumping; on **pause** the animation stops, and if the job is **stopped or fails** it stays frozen where it was. When it finishes it shows 100 % with the head at the last position of the path.
- **If the position does not match** the path (for example, a profile with a mirrored rotary) it uses the send progress; on machines that do not report their position while working (K40, galvo) the view is a **time estimate** and the text says so.
- **View only.** It sends nothing to the machine: pause and stop stay where they always were. A job **resumed** from a checkpoint has no live view.
- **Accessibility:** the window is a non-modal dialog with a name; the bed has a text description with the percentage and the state, updated without bothering the screen reader (the state is announced every 10 %); every button has a name and works with the keyboard.
- With jobs of tens of thousands of moves (image engraving) it stays smooth: what is already burned accumulates in a layer and what is pending is drawn only once (the layers are rebuilt if you zoom in a lot). On the phone you get the same view; see [Control from the phone](09e-control-desde-el-celular.md#live-view-on-the-phone).

## Live adjustments: speed and power

The **Live adjustments** section, at the end of the **Machine** tab, changes the **Speed** and the **Power** of the running job as a percentage of what was programmed, **without touching the G-code or recalculating it**. Each has the **-10**, **-1**, **+1**, and **+10** buttons, a percentage field (from 10 % to 200 %), and a button to go **Back to 100 %**. The current value is the one the controller reports; until it has reported one (it sends it from time to time) "—" is shown.

![Machine tab with What gets sent, the framing buttons, the Optimization settings, and the Live adjustments for speed and power](../../../apps/desktop/docs/screenshots/37-ajustes-en-vivo.png)

What it is for, with an example: you start cutting plywood and see that in one area it does not cut through. Instead of stopping everything and recalculating, you lower the **Speed** to 80 % while it passes that area (more energy per millimeter) and put it back at 100 % afterward. Or you notice the engraving comes out too dark: lower the **Power** to 90 %. Live adjustments do not change your layers: if you want the change to be permanent, adjust the layer.

- **GRBL and its derivatives only** (GRBL 1.1, grblHAL, FluidNC, GRBL-LPC): they use real-time commands that Marlin and Smoothieware do not have. With those controllers the controls are disabled and say why.
- They need the machine connected. They work with or without a job running; rapid (travel) movement is not adjusted.
- Above 100 % the laser works with more energy than you programmed. Raise it little by little and never take your eyes off the machine.

## Alarms

If the machine goes into alarm, a red banner appears in the **Machine** tab: **The machine is in alarm**, with the code (`ALARM:N`), its explanation, and **What to do**, depending on whether your machine has homing enabled (`$22`). The buttons are **Clear alarm ($X)…** and **Home ($H)…**: each opens a dialog with its risk and a checklist, and only sends when you confirm. `$X` unlocks without checking the position; `$H` moves the head to the limit switches, so use it only if your machine has them. The causes of each code are in [Troubleshooting](15-solucion-de-problemas.md).

## Safe console

The **Console** tab (`Ctrl/Cmd+3`) is the log of what was sent and received, with a line for typing commands.

- Read-only commands (`$I`, `$$`, `$#`, `$G`, `$N`, `M5`, and `status`) go out without asking. There are one-click shortcuts: `$$`, `$I`, `$G`, `$#`, and `status`.
- Everything else (moving, turning the laser on, changing a setting, `$X`, `$H`) opens **Confirm command**, which shows "This exact line will be sent:" with the line already normalized, and the risk. Only **Send to the machine** sends it. `$RST` and `$SLP` carry an extra warning because they cannot be undone.
- `?`, `!`, and `~` are rejected when typed by hand: use the names `status`, `hold`, `resume`, and `reset`.
- During a job only `status`, `hold`, `resume`, and `reset` are accepted.
- With the up and down arrows you browse the history; `Tab` completes the command.
- `error:N` and `ALARM:N` replies arrive with their explanation. **Hide status reports** clears the noise of the periodic `?` queries, and you can copy or clear the log.

## Continue an interrupted job

If the power goes out, the app closes, the computer restarts, or the connection drops in the middle of an engraving, you do not have to start over: Tlaku can **continue from where it stopped**. It works with GRBL, Marlin, and Smoothieware (which receive the G-code line by line). Other controllers have their own rules, in the table below.

### What the app saves while it works

- **When a job starts** it saves a *checkpoint* in the application's data folder (subfolder `interrupted-job/`): the exact G-code that was sent, the machine profile, the project path, and the job origin.
- **While sending**, about once a second it records the last line the machine confirmed with `ok`, the head position from the latest status report, and the machine state at that moment (absolute or relative mode, units, `M3` or `M4`, last power `S` and speed `F`, Z, and air). It does this on another thread and atomically, so it does not slow down the sending and a power cut in the middle of the write does not leave a broken file.
- **It is deleted** when the job finishes successfully, when you cancel on purpose with **Stop**, or when you connect a different machine.
- **It is kept** if there is an error or an alarm, if the connection is lost, if you close the app, or if the power goes out.

### When it offers to continue

When you open the app, and also when the connection is lost in the middle of a job, a notice appears: **The last job was interrupted at N %. Continue?** The percentage comes from **where the head is on the path** (the engraving path done up to that point over the total), not from the lines the machine acknowledged: with long lines almost all of them are acknowledged at the start, because they fit in its memory, and say nothing about the real progress. It has three buttons:

| Button | What it does |
|---|---|
| **Continue** | Opens the resume dialog with the restart point Tlaku calculated |
| **Choose the point** | Opens the same dialog so you can move the restart point yourself |
| **Discard** | Deletes the checkpoint; you will no longer be able to continue that job |

If you leave it for later, the [Job history](#job-history) (**Help > Job History…**) marks interrupted jobs with their percentage and has a **Continue** button as long as the checkpoint still exists.

### If the machine kept going on its own

This is the most important limit: **the machine can keep executing what it has in its memory** after Tlaku closes. A job with long lines (an 80 mm square, for example) fits entirely in its buffer: if the app dies after 4 seconds, the board finishes the job by itself. If Tlaku offered to continue from the saved position, it would go over what is already done again.

That is why the checkpoint also stores the last line **sent** (not only the one acknowledged with `ok`) and, when you reconnect, Tlaku compares the machine's position now with the one it saved:

- **If the machine is where it was**, there is nothing to warn about.
- **If it moved and ended at the end of the job**, the notice says the machine kept executing what it had in its memory and seems to have finished. **Discard** becomes the main button: check the piece first, because continuing would repeat what is done.
- **If it moved and is in the middle**, it suggests continuing from where it is now (not from where it was when the checkpoint was saved), with the warning that the machine kept going on its own and how much (lines and millimetres).
- **If it cannot be known** (there was an alarm, the board restarted or lost power, and its position is not reliable) or no machine is connected, the usual behaviour is kept but with the warning that the machine may have kept executing up to N lines (about M mm) it already had. Check the piece before continuing.

A board that restarts is left at (0, 0) with the interpreter feed `F` at zero; one that finished by itself keeps the `F` of the G-code. Tlaku uses that difference (`$G`) so it does not mistake a power cut for a finished job.

If the machine is still running the job when you **connect**, it does not accept commands: Tlaku says so (**The machine is running a job (state Run)...**) instead of blaming the port. Wait for it to finish, or stop it with its own button, and connect again.

### Where to restart

In GRBL, `ok` only means that the line went into the board's buffer, not that it has already run: the board can have up to 127 bytes or 15 blocks ahead of the head (the Falcon2 accepts 127 blocks). That is why the last confirmed line is almost never the last one that was engraved.

- **GRBL.** Tlaku compares the last saved position with the geometry of the confirmed lines, finds the line the head was running, and **steps back cautiously**: by default 3 lines or 1.5 mm, whichever goes further back (you can change it in the dialog); the lines rule only counts for short segments (each line is worth at most 2 mm), so on a long 80 mm side the step back is 1.5 mm and not the whole side. It never steps back past a `G0` travel move. In an image engraving it steps back to the start of the **scan row** in progress.
- **Marlin and Smoothieware.** They have no `?`, so the position is not polled during the job. The point is estimated only from the confirmed lines and steps back a fixed stretch (about 24 motion lines).

In the resume dialog the preview paints **what is already done** with a solid line in one color and **what is left** with a dashed line in another. A bar lets you move the restart point: backward to repeat a longer stretch, forward to repeat less.

### Mandatory steps to resume

All of the following is mandatory, and the app's engine enforces it, not just the interface: if it is not met, the resume button does not enable.

1. **Home, or confirm that nothing moved.**
   - If the machine has homing (`$22=1`), you have to reference it with **Home ($H)…**. A board that restarted (when the port was closed and reopened, or because of the power cut) or that lost its position no longer knows where it is; without homing done, it will not resume.
   - If the board **has no homing**, Tlaku asks you to confirm that the head and the material did not move and that the origin is the same. The warning is clear: if that is not so, the result will come out shifted. Marlin and Smoothieware do not answer `$$`, so with them this confirmation is always asked for.
2. **Framing the whole job.** Tlaku traces the rectangle of the saved G-code with the laser off and asks you to confirm that the material is still in place.
3. **The usual checklist**, the same one from the [Start](#4-starting) dialog.
4. **Generated preamble.** Before the job's lines, Tlaku generates a safe start, in this order: `M5` (laser off), `G90` and `G21`, assist air, a Z adjustment if needed (only with a Z axis), `G0` to the start point **with the laser off**, and only then the `M3` or `M4` mode with its `S` and `F` restored. The job's lines follow from the chosen point. The usual preflight runs on that G-code, and the engine verifies that the preamble contains no move with the laser on.

### What happens depending on the machine

| Machine | Continue |
|---|---|
| **GRBL, Marlin, Smoothieware** | Complete. The profile must put the `S` power inside the motion lines and `G0` must turn the laser off. The **GRBL-M3** profile without laser mode does not meet this |
| **K40** (M2 Nano, LHYMICRO-GL) | **It cannot be continued.** The board receives the job in 30-byte packets and does not report where it is, so there is no safe way to know where it stopped. You have to send the job again; the app only warns |
| **Ruida** | The job lives in the controller. If the app restarts, the notice explains that the machine keeps going on its own and that you can check its status by connecting |
| **Galvo** | Does not apply yet; the app only warns |

### Limits worth knowing

- **The material and the head must not move**, and the origin must be the same. If anything moved, the result will come out shifted.
- **The machine can keep executing what it has in its memory.** If the app dies, what it already delivered to the board keeps running until it empties. Tlaku detects this when you reconnect if it can compare positions (see [If the machine kept going on its own](#if-the-machine-kept-going-on-its-own)); if it cannot, it warns you. Always check the piece before continuing.
- **The restart repeats a short stretch.** The saved position can be up to about 1.2 seconds old (200 ms of the `?` polling plus the save interval), so up to about 1 second of travel is repeated, plus the step back you configured. That stretch is engraved twice and shows as a **slightly darker mark**; in images a **faint mark at the seam** may remain.
- **A power cut can make the motors lose steps.** That is why homing and framing are not a formality: check by eye before accepting.
- **If the G-code uses `G92`, `G10`, or `G28`** it cannot be resumed midway, because they change the origin or the reference position.
- **A resumed job also leaves a checkpoint**, so it can be interrupted and continued several times.
- **Assist air and Z are restored**, but check the focus before continuing.

> **Tip:** the first time, try it on scrap material and with a virtual machine ([Connecting your machine](08-conectar-maquina.md)). Like the rest of the manual, none of this has been tested with a real machine yet; the power-cut test is in the hardware validation list.

## Job history

**Help > Job History…** (formerly "Job Log") shows the jobs you **sent to a machine from this computer**: **Date**, **Machine**, **Document**, **Material**, **Estimate** (the time the preview calculated at the start; **—** if there was no up-to-date one), **Actual** (the time working, without counting pauses) and **Result**: **Finished**, **Cancelled**, **Failed**, **Interrupted** or **Running**. It is useful to compare how long it really took with the estimate, to remember what was cut and with which settings, and to **repeat it**.

![The Job history with four jobs: two finished, one cancelled, and one failed](../../../apps/desktop/docs/screenshots/69-registro-de-trabajos.png)

- A job is logged **when it starts** and completed **when it finishes, is cancelled or fails**; if the application closes in the middle of a job it stays as **Interrupted**. A pause and its resume do not count as new jobs.
- It also keeps the **layers with their settings**, the **material**, the **area**, a **thumbnail**, the origin **tab** and your **notes**; it lets you **search and filter**, **Repeat with these settings** and **export to CSV**. All that, and where it is stored (only on this computer, in the application data folder, up to 1000 jobs by default), is in [Job queue and job history](09f-cola-e-historial-de-trabajos.md).
- **Clear the log** empties it (it asks for confirmation and cannot be undone).
- A job that was interrupted (power cut, app closed, lost connection) is marked as interrupted with its percentage and has the **Continue** button as long as the checkpoint exists. See [Continuing an interrupted job](#continuing-an-interrupted-job).
- It does not record G-code exports or what you send from the console.

For several documents in a row, the [job queue](09f-cola-e-historial-de-trabajos.md) walks you from one to the next and **never sends the next one by itself**.

## Exporting G-code

If you prefer to send with another program, **File > Export G-code…** (`Ctrl/Cmd+Shift+E`) saves a `.gcode` file with the same calculation and the same job options (including **Selection only**, the origin, and the [Optimization settings](07-vista-previa.md)). (To get the **design** out as SVG, DXF, PNG, or PDF, see [Import and export](03b-importar-y-exportar.md).) When done it shows the lines and the estimated time. It is disabled if no layer with objects has output turned on. If you use **Current position**, you need the machine connected to export; without it use **Absolute coordinates**.

From the command line, the `tlaku` program does the same and more: `tlaku send` (with the same checks), `tlaku frame`, `tlaku cmd`, and `tlaku plan`. See the project README.

Next: [Advanced machine control](09b-control-avanzado.md)
