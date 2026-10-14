# Preferences, shortcuts and autosave

How to set Tlaku up your way: the **Preferences** window, the **keyboard shortcut** editor, the `.tlakuprefs` files for moving your settings to another computer and the **recovery copies** that protect your work if the application closes abruptly. Shortcuts are written with `Ctrl/Cmd`: `Cmd` on macOS, `Ctrl` on Windows and Linux.

## Opening the preferences

Choose **Edit > Preferences…** or press `Ctrl/Cmd+,`. The window has seven sections on the left and **changes are saved automatically**: there is no OK button, only **Close**.

![Preferences window with the General, Editing, Appearance, Files, Automation and Shortcuts sections (the screenshot predates the Remote control section)](../../../apps/desktop/docs/screenshots/29-preferencias.png)

At the bottom there are three buttons: **Import…** and **Export…** (see below) and **Reset**, which returns all preferences to their factory values after asking for confirmation. Reset **does not touch your keyboard shortcuts**.

## What you can adjust

| Section | Setting | Default | Range |
|---|---|---|---|
| General | **Language** | The system's (Spanish or English) | Español or English; it changes instantly, without losing the document. The engine's warnings and errors appear in the chosen language |
| Editing | **Length units** | Millimeters (mm) | mm or inches (in) |
| Editing | **Normal step** for the arrow keys | 1 mm | 0.001 to 100 mm |
| Editing | **Step with Shift** | 10 mm | 0.001 to 500 mm |
| Editing | **Step with Alt** | 0.1 mm | 0.001 to 10 mm |
| Editing | **Rotation step with Shift** | 15° | 1° to 180° |
| Editing | **Snapping distance** | 8 px | 1 to 40 px |
| Editing | **Offset when duplicating and pasting** | 5 mm | 0 to 200 mm |
| Editing | **Return to Select after drawing a shape** | Yes | Yes or no. When on, after finishing a rectangle, an ellipse, a polygon, a star or a line, the tool switches to **Select** and leaves the shape selected; when off, the tool stays active so you can draw another |
| Appearance | **Theme** | Dark | Dark or Light |
| Appearance | **Canvas grid** | Automatic (based on zoom) | Automatic or a fixed size from 1 to 100 mm |
| Appearance | **Grid contrast** | Standard | Soft, Standard or High |
| Appearance | **Window layout** | Panel on the right, tools on the left | Left or right for each, and **Reset layout**. See [Guides, tabs, start point and project](03e-guias-puentes-y-proyecto.md) |
| Files | **Save recovery copies automatically** | Yes | Yes or no |
| Files | **Interval between copies** | 60 s | 10 to 3600 s |
| Files | **Recent files to remember** | 10 | 0 to 30 |
| Files | **Default resolution** (Importing files) | 96 dpi | 10 to 2400 dpi |
| Files | **Remember the import dialog options** | Yes | Yes or no |
| Automation | **Turn on UDP automation** | No | Yes or no. See [Network connection and automation](08b-red-y-automatizacion.md) |
| Automation | **UDP port** | 47801 | 1024 to 65535 (appears when you turn it on) |
| Automation | **Allow starting from automation** | No | Yes or no. **When on, any program on your computer can fire the laser without your confirmation** |
| Remote control | **Turn on remote control from your phone** | No | Yes or no. Shows a QR code to open a page on your phone, over the local network. See [Control from your phone](09e-control-desde-el-celular.md) |
| Remote control | **Port** and **Network address** | 47802, automatic | Port 1024 to 65535; the address is chosen manually only if the computer is on several networks |

![The interface in English: panels and menus translated after changing the language in Preferences; in this screenshot, taken before the engine's messages were translated, the layer-order summary appears in Spanish](../../../apps/desktop/docs/screenshots/38-english.png)

Things worth knowing:

- **Units.** The design is always stored in millimeters; the unit you choose changes how lengths are shown and typed. It applies to the numeric bar (X, Y, width and height), the status bar, the rulers and the grid (in inches, in tenths), the **Measure** tool (see [Drawing and editing](03-dibujar-y-editar.md)), the length fields in the Arrange dialogs (arrays, offset, align and nest parts) and those in this window. The fields accept the unit as you type ("5 mm", "0.25 in", `1/8"`, "1 1/2 in", `50%`). Speeds stay in mm/min, and the layer fields (interval, kerf, Z offset...), the machine areas and the camera ones stay in mm.
- **Arrow-key steps.** They change the three arrow-key steps (see [Drawing and editing](03-dibujar-y-editar.md)).
- **Rotation step.** It is the one used by rotating with `Shift` while dragging the handle, by the commands **Rotate One Step Counterclockwise/Clockwise** and by the **Rotation step** field in the numeric bar.
- **Grid.** **Automatic** adjusts the spacing to the zoom; with a **fixed size** (1 to 100 mm), the minor grid is that size and the major grid is ten times larger; when zoomed far out it is hidden so the view does not get cluttered. **Grid contrast** (**Soft**, **Standard**, **High**) decides how strongly its lines show over the bed: raise it to **High** if you cannot see it well on your screen or with the light theme, and lower it to **Soft** if it distracts you while drawing.
- **Return to Select.** This is what lets you move or scale a figure right after drawing it without changing tools. Turn it off if you draw many figures of the same type in a row; `Esc` also returns to **Select** at any time.
- **Importing files.** The **Default resolution** is the one used to import unitless SVGs and images when you do not pick another in **Import with options…** (96 is the usual value in drawing programs; Illustrator uses 72). With **Remember the import dialog options**, that dialog starts with what you chose last time (resolution, DXF unit, grouping and target layer); the **scale** and the **PDF page** always start fresh. **Forget the remembered options** erases them. See [Importing and exporting](03b-importar-y-exportar.md).
- **Theme.** It is the same setting as the sun or moon button in the top bar.
- **Recent files.** The **Clear the recent files list** button empties the **File > Open Recent** list. With 0, none are remembered.

### Working in inches

If your measurements come in inches (US plans, 3/4" lumber), change **Length units** to **Inches** and the program talks to you in that unit while the design is still stored in millimeters.

1. **Edit > Preferences… > Editing > Length units**: inches (`in`).
2. Select an object: in the numeric bar, **X**, **Y**, **Width** and **Height** appear in inches, as do the rulers (with tenth-of-an-inch marks) and the status bar.
3. Type whichever way is comfortable. With inches on, a number without a unit is inches, and you can use fractions and mix units: `3/8"`, `1 1/2 in`, `2.25`, `5mm` (converted), `50%` (half of the current value) or a calculation such as `(10+2)*4`.

> **Example:** a part of 3 1/2 × 2 inches. Select the rectangle, type `3 1/2` in **Width** and `2` in **Height**. Internally it is 88.9 × 50.8 mm. To leave 1/8" of spacing when nesting parts, type `1/8"` in **Spacing between parts** in **Nest parts**.

What **stays in millimeters** even if you choose inches: speeds (always mm/min), the layer fields (interval, kerf, Z offset...), the machine and camera areas and the material thickness. If you see a figure in millimeters in one of those fields, it is not a bug: that field does not change unit yet.

If you have two Tlaku windows open, a preference change in one **is reflected in the other** immediately.

Preferences live in a `preferences.json` file inside the application's configuration folder, not in the web window, so they survive clearing the embedded browser's data.

## Keyboard shortcuts

The factory shortcuts are in [The interface](02-interfaz.md) and in the dialog opened with `F1`, `?` or `Ctrl/Cmd+/`. To change them, open **Edit > Edit keyboard shortcuts…** or the **Shortcuts** section of Preferences.

![Keyboard shortcuts dialog with the search box](../../../apps/desktop/docs/screenshots/09-atajos.png)

The list groups commands by category and has a search box (**Search for a command or a shortcut**). On each command:

1. Press **Change the shortcut** (the pencil) and, when it says **Press the key combination…**, type the new one. `Esc` cancels.
2. **Add another shortcut** (the plus sign) gives a command several combinations.
3. **Remove the shortcut** leaves it with **No shortcut**.
4. **Restore the default** goes back to that command's original combination.

A command with a changed shortcut is marked **Custom**, and the top shows how many you have.

If the combination is already used by another command, Tlaku does not silently override it: it says "X" is already the shortcut for "Y" and offers **Take it away and assign it here** or **Cancel**.

Also:

- **Restore the defaults** (at the bottom of the panel) removes all your changes, after confirming.
- **Export shortcuts…** and **Import shortcuts…** save and load only the shortcuts, useful as a backup.
- Tool keys are single letters and do not work while you are typing in a field, just like the factory shortcuts.
- The `F1` dialog and the tables in this manual **always show the factory shortcuts**, not the ones you have changed.

## Moving your settings to another computer

In the Preferences window, **Export…** saves a `.tlakuprefs` file with all your preferences **and** your shortcuts. **Import…** loads it on another computer (or after reinstalling). The file also carries the preferences that are changed outside this window: the **defaults for new layers** and **Beginner mode** (see [Layers and cut settings](05-capas-y-ajustes.md)) and the setup of the **Before sending** list in the Start dialog (see [Sending jobs safely](09-enviar-con-seguridad.md)). The **Optimization settings** and the **Finish position** are remembered between sessions, but do not travel in this file. To carry your **machines** (with macros and positions), your **material library**, your **cameras** and your **own art** too, use a `.tlakubundle` bundle (**File > Export settings bundle…**; see [Advanced machine control](09b-control-avanzado.md)).

When importing, Tlaku tells you how many preferences came in and for how many commands the shortcuts were. If the file comes from another version:

- Preferences that this version does not understand are **ignored**, and you are told how many there were.
- Shortcuts for commands that do not exist in this version **are kept**, in case they arrive later, and you are told how many there are.

A file that is not a Tlaku preferences bundle is rejected with a clear message. Materials have their own file (`.tlakumat`, see [Materials](11-materiales.md)); devices, cameras and your own art travel in the `.tlakubundle` bundle.

## Autosave and recovery

While you work, Tlaku saves a **recovery copy** of your document. It does not replace your `.tlaku` file: **Save** is still what keeps your work, and the copy never overwrites your project.

- Every 60 seconds by default (adjustable in **Preferences > Files**, from 10 to 3600 s), and **only if there are unsaved changes** since the last copy.
- The copy is in the application's data folder, in a `recuperacion` subfolder. At most 10 copies are kept.
- It is deleted automatically when you save the project, open another one or close the application normally.
- It can be turned off with **Save recovery copies automatically**. If the copy cannot be written, Tlaku warns you once.

If Tlaku closes abruptly (a power cut, a crash, a forced quit), the next time you open it **Unsaved work found** appears, with one row per document: its name, **Copy from** the date and time, and the source project (or **Document that was never saved**). For each one:

- **Recover** opens the copy as your current document, marked as **Unsaved** (if you already had something with changes, Tlaku asks first). After recovering, **save it** to keep it.
- **Discard** deletes it.
- **Discard all** deletes them all (asks for confirmation; it cannot be undone).
- **Decide later** closes the window without touching anything.

> **Tip:** autosave saves you from an unexpected shutdown, not from your own mistake. To undo a change, use **Undo** and the **History** panel (see [The interface](02-interfaz.md)).

Next: [Drawing and editing](03-dibujar-y-editar.md)
