# Camera

Put a camera over the bed and Tlaku shows its image as the canvas backdrop, so you can place the design right on the material. For it to match you need to correct the lens, align the image with the bed, and, if you want precision, enter the material's thickness.

> **Warning:** the camera was tested with simulated cameras and photos, not with a real machine's camera. Besides serving as a reference backdrop, the image can be saved in the document, traced to vectors, assembled into a mosaic with a head-mounted camera, and used to find the center of a round object. **Network cameras** (a snapshot or MJPEG URL on your local network), named **camera presets**, and a **manual adjustment** of the backdrop are also supported. Anything marked *to be validated with real hardware* was tested only with synthetic data.

## The Camera panel

Open the **Camera** tab (`Ctrl/Cmd+5`). From top to bottom it has: video source, **Camera presets**, live view, **Canvas backdrop**, **Manual backdrop adjustment**, **Bed image**, **Lens**, **Align with engraved marks**, **Material height**, **Bed alignment**, **Head-mounted camera**, and **Center finder**. The same commands appear in the **Machine** menu: **Turn on camera**, **Show camera backdrop on the canvas**, **Freeze the camera frame**, **Calibrate camera lens…**, **Align camera with engraved marks…**, **Capture bed image**, **Vectorize from camera…**, **Update the camera overlay**, and **Center finder (camera)…**.

![Camera panel with the live view, the backdrop, the lens, and the engraved marks](../../../apps/desktop/docs/screenshots/17-panel-camara.png)

1. Choose the **Camera** (by default, the system default) and the **Resolution**. Press **Turn on camera**; you will see "Streaming 640 × 480 px" or whatever applies.
2. If it fails, the panel says why. On macOS, if there is no permission, open System Settings, Privacy & Security, Camera, enable Tlaku, and turn it on again. If it says it is in use, close the other application that has it.

> **Tip:** always use the same resolution. The calibration and the alignment are saved for that resolution; if it changes, the app warns you and they must be repeated.

The configuration (calibration and alignment) is saved and kept across restarts.

### Network camera

Under **Camera type** choose **Network camera (URL)** and paste the address of an HTTP *snapshot* (JPEG) or of an MJPEG stream, for example `http://192.168.1.20/snapshot.jpg`. Tlaku requests frames from the camera (a few per second), so the backdrop is not as smooth as a USB webcam's.

- **It must be on your local network** (`192.168.x.x`, `10.x.x.x`...) and start with `http://`. Tlaku does not connect to internet addresses or to `https://`.
- If the camera asks for a username and password, put them in the URL (`http://user:password@192.168.1.20/...`; Basic authentication, not Digest). **They are stored only on this computer**, in the camera's configuration file (`camera.json`, in plain text: protect that folder like any file with passwords). In the panel they are shown with the password hidden and are **never** included in a `.tlakubundle` bundle.
- If there is no image, the panel shows the reason the camera gave (password rejected, address outside the local network, not responding...).

### Camera presets

Save with a name the source (local camera or network URL), the resolution, and the manual backdrop adjustment, and return to them with one click; if the camera was on, it reopens with the chosen preset. Up to 32; saving with a name that already exists updates it.

## Step 1: calibrate the lens

Almost all cameras bend the image, especially at the edges. Calibrating measures that distortion and corrects it. Press **Calibrate lens…** (or **Recalibrate…**).

![Calibration wizard with the checkerboard detected and the results](../../../apps/desktop/docs/screenshots/16-camara-calibracion.png)

1. Print the **Printable checkerboard** (9 × 6 inner corners) with **Print…**, **Save PDF…**, or **Save SVG…**. Choose the square size and the sheet (**A4 landscape** or **Letter landscape**). Print it at 100 % and check with a ruler that a square measures what it says. Stick it onto a flat, rigid surface.
2. Turn on the camera and show the checkerboard tilted to different sides and at the corners of the frame. When **Checkerboard detected** appears, press **Capture view**.
3. Gather at least 6 varied views (the absolute minimum is 3). The wizard shows the **Field coverage** and tells you which areas are missing ("Not covered: upper left...").
4. Press **Compute**. The **Reprojection error** is rated like this: under 0.5 px **excellent**, 0.5 to 1 **good**, 1 to 2 **fair**, and 2 or more **poor, worth repeating** (repeat with the checkerboard flat, with no reflections, still, and with more angles).
5. If the result works for you, **Use and save**. **Discard result** throws it away.

Calibrate at the same resolution you are going to work with. **Clear calibration** deletes it.

## Step 2: align with the bed

Aligning tells Tlaku which part of the image is which part of the bed. There are three methods; without alignment the backdrop is not shown.

### With engraved marks (recommended)

Engrave crosses at known positions and the camera aligns itself, measuring its accuracy in mm.

1. In **Align with engraved marks** choose the **Number of marks**: **9 marks (measures accuracy)** or **4 marks (quick)**.
2. Press **Open marks document**. A new document opens, with a low-power layer and the crosses over 85 % of the bed; the lower left corner carries a circle to fix the orientation.
3. Engrave that document on a piece of cardboard fixed to the bed and **do not move it afterward**.
4. Without moving the cardboard or the camera, turn on the camera until you can see the bed and press **Capture and detect…**. In the wizard, leave the **Mark height** at 0 if they are on the bed (or the material's thickness) and press **Capture and detect**.
5. Check the photo: green is a mark that was used, amber has a high error, red is discarded, dotted red was not found. The **Fit error** is rated from excellent (under 0.3 mm) to poor (1 mm or more). With 9 marks you also see the **True accuracy, leaving one mark out**; with fewer than 5 it cannot be measured.
6. If it is good, **Accept alignment** (at least 4 used marks are needed). If not, **Discard and retry**.

### By hand, with 4 points

In **Bed alignment**, mark on the image the 4 corners of a sheet of known size (or of the bed), in this order, seen from the front of the machine: lower left, lower right, upper right, upper left. Drag the points to adjust them and type their coordinates in mm, or press **Full bed**, **A4 sheet**, or **Letter sheet** to fill them in. Then **Apply alignment**. It tells you the error of the points in mm. **Remove points** and **Clear alignment** undo.

### With the head, no engraving (fixed camera)

For a camera fixed above the bed (like the one in the lid of a Creality Falcon2 Pro). **Nothing is engraved and no material is used**: with the laser **off**, Tlaku moves the head to a grid of known bed points, takes a photo at each, finds the head in the photos and works out how the image maps to the bed.

1. In **Align with the head (no engraving)** press **Align with the head…**. If there is no lens calibration, Tlaku estimates an approximate one; for better accuracy calibrate the lens first (Step 1).
2. In **Options** (all optional): the **Grid of points** (3 × 3 to 5 × 5), the **Bed margin**, **How the head is located** (automatic: by its **green LED** if it has one; or by **image difference**, which tracks the silhouette even if the gantry moves), the **parallax correction** (the heights of the camera and of the head point above the material) and a fixed **offset** between the tracked point and the laser.
3. Clear the bed, close the lid, tick the confirmation and press **Start alignment**. The machine must be connected, idle and homed. The head visits the points (the laser is **never** turned on); you see the progress per point with a thumbnail of each photo and can **Stop** between two moves.
4. If the camera sees only part of the bed (usual for a lid camera: the head does not show in the front rows, or only peeks out at a corner), points out of view are dropped and Tlaku **repeats a finer grid where it did see the head**, up to three more rounds.
5. In the **Result** you see the error per point in mm, the quality (**Good** with a cross-validation error up to 1 mm; **Acceptable** up to 2.5 mm; **Poor**), how many photos did not show the head, which points were **dropped** for not agreeing, and **which zone of the bed the points cover**: outside that zone the alignment is an extrapolation and may fail. **Adjust the offset** recomputes with the same photos without moving anything.
6. If it is good, **Save alignment**: it is stored as the camera alignment (the same `camera.json`) and the backdrop uses it. If there was no lens calibration, the approximate lens is saved with the alignment.

**Parallax and offset.** The tracked head point is higher than the material and the camera sees it shifted away from its vertical; with both heights the alignment is valid at material level. The LED is not over the nozzle either: that fixed offset can be typed in or, better, computed with **a single reference point**: fire a test mark at a known position, open **Fine-tune with a reference point**, type its coordinates, press **Mark on the image**, click the center of the mark and **Apply reference point**. The alignment shifts so that point matches.

## Step 3: using the backdrop

With the alignment done, tick **Show the camera behind the design**. Adjust the **Opacity** and, if you want, **Freeze frame**: the frame stays as the backdrop even if you turn off the camera, useful for leaving the material in place and designing without the camera on. If the saved alignment is from an image of a different size, the app asks you to align again.

## Camera preferences

The preferences of **each camera** (device, resolution, opacity, grayscale, update when a job finishes, and the manual backdrop adjustment) are in this panel and are saved in the camera's local profile. The **general** ones are in **Edit > Preferences… > General > Camera**: **Reopen the camera on startup** (if the backdrop was visible when you closed, the camera turns on by itself; on by default) and **Maximum image width** (320 to 1920 px, 1280 by default: the width the frame is reduced to for drawing it and for detecting marks and circles; wider gives more detail and asks more of the computer).

## Material height

The camera sees the material's surface, which is closer than the bed, so the backdrop shifts according to the thickness. In **Material height** type the **Material thickness** (from 0 to 300 mm) and the backdrop is corrected. The app says how reliable the correction is:

- **Estimated from the lens** (approximate), if there is only one alignment.
- **Measured with marks** at the heights you have captured. To measure it, repeat the marks on a board of another height (at least 2 mm different from the previous ones) and type that height in the wizard. If your thickness falls outside the measured range, it warns you that extrapolating is less reliable.

## Fisheye lenses

The 5 MP cameras that come on the lid of many CO2 lasers are fisheye: the distortion is so strong that the normal model (Brown-Conrady) ends up with errors of several millimeters at the corners even though the center looks fine. In the calibration wizard, **Lens model** offers:

- **Automatic (recommended)**: fits both models and chooses the one that best predicts the views it set aside, not just the one with the lowest reprojection error. A normal lens stays with Brown-Conrady.
- **Brown-Conrady** and **Fisheye (Kannala-Brandt)**, to fix one.

The result shows the chosen model and a table with the fit and the error on held-out views for each model. With fisheye, take checkerboard views at the corners of the image too. The corrected image of a fisheye shows a wider field, with fewer pixels per millimeter at the center. *To be validated with real hardware: it was tested with synthetic cameras, not with a real lens.*

> **Example:** your lid camera leaves the bed's corners curved and with errors of several millimeters even though the center looks fine. Calibrate with the **Automatic** model and take checkerboard views at the corners as well. In the project's synthetic test (a 140° fisheye), the normal model looked good with 0.56 px of reprojection error but was off by up to 108 mm on the plane; the fisheye model, 3.2 mm. That is why the wizard compares with held-out views and not only with the reprojection error.

## Manual backdrop adjustment

If the alignment came out almost right but not quite, **Manual backdrop adjustment** (with the camera frozen and a saved alignment) lets you fine-tune by hand: tick **Set the resolution by hand** and change the **dots per mm (DPM)** (below you see the ones the alignment gives), and move the backdrop with the **X offset** and **Y** in mm. The adjustment is applied on top of the alignment, without changing it, and also to the bed image you capture. **Remove the adjustment** goes back to the alignment. It is not used with the head-mounted camera.

## Capturing the bed image

With the camera aligned, **Bed image** saves what the camera sees inside the document:

- **Capture bed image** saves the photo, already corrected and aligned, as an image in the document at the exact place where it is on the bed (with the correction for the material's thickness). If the project is not saved, the PNG is left in the temporary folder and the app tells you.
- **Vectorize from camera** does the same and immediately opens the trace dialog on that image (see [Images](06-imagenes.md)).

> **Example:** you put a hand drawing in black ink on paper on the bed, capture the image, and trace it: you get a path that you can cut or engrave on top of the real piece, in its exact place.

In **Canvas backdrop** (next to the opacity) there are two more settings: **Grayscale background** (to see the design on top better; the image you capture also comes out in gray) and **Update the background when a job finishes** (refreshes the frozen frame, or repeats the head-mounted camera's sweep).

## Head-mounted camera

If your camera is mounted on the machine's head, tick **The camera is mounted on the head**: the image moves with the machine's position. You need to calibrate the offset between the camera and the laser:

1. **Generate mark** opens a cross at the center of the bed (you can change its position). Engrave it on a piece of cardboard without moving it, in absolute coordinates.
2. Turn on the camera, connect the machine, move the head until you see the cross, and press **Measure with the camera**. Click the center of the cross on the image.
3. Press **Next position**: the head moves a few millimeters and you click again. With 3 points or more the offset, the scale, and the error are calculated; **Save calibration** keeps them.

**Update the overlay** sweeps the bed in a grid with the laser off, takes a photo at each point, and joins them with blending at the edges; the mosaic stays as a fixed backdrop. Before moving the machine the app asks for confirmation and shows how many positions there are; the sweep can be cancelled. In **Scan area** you choose to sweep the **full bed**, **the selection**, or **an area drawn on the canvas**: press **Draw area on the canvas** and drag a rectangle (Esc cancels); it is marked with dotted lines and its size, and the sweep, the capture, and "update when a job finishes" cover only that area. The same sweep is used in **Capture bed image**. *To be validated with real hardware: tested with the virtual machine and synthetic photos.*

**Automatic alignment with marks.** Instead of clicking the cross, **Automatic alignment with marks** calculates the offset and the scale from a single photo: **Generate the marks as a document** opens a grid of 9 crosses (at least 60 mm per side; you can change the side and the corner) and the first one carries a circle. Engrave them on a piece of cardboard without moving it, move the head until you see the marks with the circle in view, hold it still, and press **Detect marks**. With 4 marks or more the calibration comes out (you can repeat from another position to refine it); **Save calibration** keeps it. *To be validated with real hardware.*

## Center finder

**Center finder** finds the center of a round object with the camera: mark **3 edge points** on the image, or choose **Detect the circle** and click inside the object. It tells you the center and the radius in mm and offers **Move the head to the center** (with the laser off) and **Job origin here** (sets the user origin; then choose "User origin" in "Start from"). *To be validated with real hardware.*

> **Example:** you want to engrave a logo at the center of a round coaster. Turn on the camera with the alignment done, open **Center finder**, choose **Detect the circle**, click inside the coaster, and look at the center and the radius. Press **Job origin here**, and in **Start from** choose **User origin** with the job point at the **center**: the logo ends up centered. If detection fails (it needs to see at least 45 % of the outline), use the **3 edge points**. This finder does not touch the head: it relies on the camera's alignment, so it is only as accurate as that. If you have no camera, or want to measure with the machine itself, use the next one.

## Center finder with the head

**Machine > Center finder with the head…** finds the center of a round object from three edge points measured by the **machine itself**, with no camera. There are two modes:

- **With a probe or touch plate (G38).** Only with **GRBL** controllers that have a probe wired to the probe input. Put the head **outside** the object, closer to its edge than the travel, and choose which way to touch (up, down, left, or right). Before moving, Tlaku shows the travel, the speed, and the exact G-code (`M5`, `G91`, `G38.3 X30 Y0 F100`, `G90`) and asks you to confirm. The head then moves **slowly with the laser off** until the probe makes contact, stops, and backs off a few millimeters. Repeat from three different sides: at the third point you see the center and the radius.
- **With the pointer.** Without a probe: take the red pointer (or the head) to a point on the edge with **Move** and press **Save point**; repeat at three well-spread points. You confirm each point. If the profile has a pointer offset, it is taken into account.

Then, as with the camera: **Move the head to the center** (laser off) and **Job origin here**. **Remove the last one** and **Start over** fix a badly placed point.

**Safety.** The factory values are cautious: 100 mm/min speed (maximum 500), 30 mm travel (maximum 100), and 2 mm retract. The laser stays off the whole time, it only moves with the machine idle, and the engine rejects the touch if there was no confirmation. If there is no contact over the whole travel, Tlaku says so ("No contact within 30 mm") and **does not leave the machine in alarm** (it uses `G38.3`, which does not cause one); if the probe was already active at the start, the controller raises `ALARM:4` and you must move the head away and unlock. The contact is taken where the probe tip is: first measure an object of known diameter to learn how much it deviates. *To be validated with real hardware: it was tested with the GRBL emulator, which simulates the probe (`G38.2` to `G38.5`).* The app's virtual GRBL machines include a 30 mm diameter disc centered at X100 Y100 so you can try it without a machine.

> **Example:** you want to center an engraving on a 25 mm coin stuck to the bed. Put the head 10 mm to the left of the edge, choose **To the right**, confirm, and repeat from below and from above. If the diameter it shows is not 25 mm (±0.5 mm), your probe has an offset: write it down.

Next: [Materials](11-materiales.md)
