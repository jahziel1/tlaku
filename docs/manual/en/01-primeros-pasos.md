# Getting started

On this page you go from zero to a ready job: you install Tlaku, import a design, check its layers, look at the preview, test it with a virtual machine and save the project. You do not need a laser to follow along.

> **Heads up:** nothing in Tlaku has been tested with a real machine yet. What you see here works against the emulator and the app's virtual machines. When you connect a real laser, do it under supervision and at low power.

## 1. Install

The installers are published on the repository's **Releases** page:

| System | File |
|---|---|
| macOS 11 or later | `.dmg`: open it and drag Tlaku to Applications |
| Windows 10 or later | `.exe` (NSIS installer) or `.msi` |
| Linux | `.deb` or `.AppImage` |

The application is not signed yet, so the system will warn you the first time. On macOS, try to open it once and then go to **System Settings > Privacy & Security > Open Anyway** (on macOS 14 or earlier, right-click Tlaku and choose **Open** also works); on Windows, choose **More info** and then **Run anyway**. On Linux, to use the serial port add your user to the `dialout` group (see [Troubleshooting](15-solucion-de-problemas.md)).

## 2. Open it and get to know the window

When you open Tlaku you will see the machine bed empty, with the message **The bed is empty**. At the top are the menus and buttons, on the left the tools, on the right the tabbed panel, and at the bottom the strip of layer colors. The details are in [The interface](02-interfaz.md).

![Tlaku main window with an imported design, the Layers panel on the right and the color strip at the bottom](../../../apps/desktop/docs/screenshots/01-principal-oscuro.png)

## 3. Import a design

1. Choose **File > Import Design…** (`Ctrl/Cmd+I`), or drag the file onto the window.
2. Accepted formats: SVG, DXF, PDF, AI, HPGL (`.plt`), G-code, and the images PNG, JPG, BMP, WebP, TIFF, GIF and TGA.
3. Tlaku creates **one layer for each color** in the file. An image goes to its own gray layer, in **Image** mode.
4. If the importer warns about something (shapes it could not read, for example), a persistent notice appears: read it and check that the design is complete.

If you need to choose the scale, the DPI, the unit of a DXF or the page of a PDF, use **File > Import with options…**. All the formats and their options are in [Importing and exporting](03b-importar-y-exportar.md). LightBurn projects (`.lbrn`, `.lbrn2`) can also be imported: see [Coming from LightBurn](14-desde-lightburn.md). More about images in [Images](06-imagenes.md).

## 4. Check the layers

Open the **Layers** tab (`Ctrl/Cmd+1`). Each row is an operation: color, mode, speed, power and its label (**Engrave**, **Line** or **Cut**).

- Click a row: it becomes the active layer. `Enter` or double-click opens its settings.
- The eye hides the layer on the canvas only; the bolt decides whether it is **sent to the machine**.
- A new layer starts with cautious values: 1000 mm/min, 20 % power and 1 pass. They are not values for any particular material: adjust them yourself.

All the parameters are in [Layers and cut settings](05-capas-y-ajustes.md).

## 5. Place the design

Select with the **Select** tool (`V`) and move with the mouse, with the arrow keys (1 mm; `Shift` 10 mm; `Alt` 0.1 mm) or type exact values in the numeric bar at the top (X, Y, Width, Height, Rotation). Check that everything stays inside the bed: if something sticks out, the preview tells you.

## 6. See the preview

Press **Preview** (or `P`). Tlaku calculates the path, draws it on the canvas and shows the estimated time. Play back the path with the play button or with `K`. Details in [Preview and cut order](07-vista-previa.md).

## 7. Test with the virtual machine

This is how you go through the whole flow without hardware:

1. Open the **Machine** tab (`Ctrl/Cmd+2`).
2. Under **Port** choose **Virtual machine · Falcon2 Pro** (the virtual ones appear at the end of the list).
3. Press **Connect**. You will see the **Virtual** label in the panel and in the status bar.
4. Press **Frame** (`F`) and then **Start** (`Ctrl/Cmd+Enter`). Start never sends straight away: first **Laser ready and area clear?** appears with a checklist; only **Start job** sends it.

![Machine panel connected to a virtual machine, with the option to simulate faults](../../../apps/desktop/docs/screenshots/25-maquina-virtual.png)

Nothing is engraved: the laser is simulated. To connect a real machine, see [Connecting your machine](08-conectar-maquina.md) and [Sending jobs safely](09-enviar-con-seguridad.md).

## 8. Save

- **File > Save** (`Ctrl/Cmd+S`) saves the project as `.tlaku`. **Save As…** is `Ctrl/Cmd+Shift+S`.
- **File > Export G-code…** (`Ctrl/Cmd+Shift+E`) writes the job as G-code, in case you want to send it with another program. It needs at least one layer with output and objects.
- To take your **design** to another program, use **File > Export SVG…**, **DXF…**, **PNG image…** or **PDF…** (see [Importing and exporting](03b-importar-y-exportar.md)).
- If you close with unsaved changes, Tlaku asks whether you want to save.
- **Autosave:** while you work, Tlaku saves a recovery copy every minute (if there are changes). If the application closes abruptly, it offers to **recover** your work the next time you open it. It does not replace saving: see [Preferences, shortcuts and autosave](02b-preferencias-y-atajos.md).

Next: [Simple mode](01b-modo-sencillo.md)
