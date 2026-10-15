# The interface

A tour of the Tlaku window: what is in each area and which keyboard shortcuts save you time. Shortcuts are written with `Ctrl/Cmd`: `Cmd` on macOS, `Ctrl` on Windows and Linux.

![Tlaku window with the Layers panel docked on the right](../../../apps/desktop/docs/screenshots/18-pestanas-ancho.png)

## Top bar

From left to right:

- The **File**, **Edit**, **Tools**, **View**, **Machine**, **Arrange** and **Help** menus. Each command shows its shortcut; if it is not available, it explains why.
- File buttons: new, open, save, import and export G-code. Next to them, **Undo** and **Redo**.
- The project name (**Untitled** until you save it) and the **Unsaved** notice when there are changes.
- The theme button (sun or moon) and, on the right, the three job buttons: **Preview**, **Frame** and **Start**. Frame and Start are enabled only with a connected machine and something to send.

The **Help** menu opens this **User Manual** (with search), the list of **Keyboard Shortcuts**, **What's New**, the **Job Log…** (the jobs you sent to a machine from this computer, see [Sending jobs safely](09-enviar-con-seguridad.md#job-log)), **Report a Problem…**, **Donate to the Project…** and **About Tlaku**. Preferences are under **Edit > Preferences…** (`Ctrl/Cmd+,`).

![The user manual open inside the application, with the table of contents on the left and the search box](../../../apps/desktop/docs/screenshots/32-manual.png)

## Tool palette

The column on the left, from top to bottom:

| Tool | Key |
|---|---|
| Select | `V` (also `Esc`) |
| Edit Nodes | `N` |
| Draw Line | `L` |
| Rectangle | `R` |
| Ellipse | `E` |
| Polygon | `G` |
| Star | `Shift+G` |
| Text | `T` |
| Pan (move the view) | `H` |
| Measure | `M` |
| Manual tabs | `U` |
| Start point | `Q` |
| Trim shapes | `X` |

Single-key shortcuts work across the whole window, except while you are typing in a field. **Measure** shows distances, angles and data about a shape. How to use each tool is covered in [Drawing and editing](03-dibujar-y-editar.md); the last three are covered in [Guides, tabs, start point and project](03e-guias-puentes-y-proyecto.md) and [Advanced shapes and warping](03c-formas-y-deformar.md).

## Canvas and rulers

The canvas shows the bed of the active machine (its name and size in mm at the top left) with rulers in millimeters. The origin is at the bottom left and Y grows upward.

- **Zoom:** mouse wheel (centered on the cursor), pinch on the trackpad, or the buttons at the bottom right (zoom out, zoom in, fit to bed). `Ctrl/Cmd+=` and `Ctrl/Cmd+-` zoom in and out; `Ctrl/Cmd+0` fits to the bed and `Ctrl/Cmd+Shift+0` fits to the design.
- **Pan the view:** middle button, `Space` + drag, two fingers on the trackpad or the Pan tool.
- **Snapping:** three switches at the bottom left: **Snapping** (`S`), **Snap to grid** and **Snap to objects and bed** (nodes, midpoints, centers and intersections). Hold `Alt` while dragging to suspend it for that gesture only.
- **Context menu:** right-clicking the canvas opens the most-used actions (see [Drawing and editing](03-dibujar-y-editar.md)).
- **Guides:** drag from a ruler to create a guide that attracts objects (see [Guides, tabs, start point and project](03e-guias-puentes-y-proyecto.md)); double-clicking a ruler lists them all.
- **Ghosts:** objects with a **virtual array** show their copies as faint outlines (see [Copies, virtual arrays and nesting](03d-copias-y-nesting.md)).
- **Outlines Only** (`W`) draws objects without fill.

## Numeric bar

Above the canvas, with objects selected, type exact values: **X**, **Y**, **Width**, **Height**, **Rotation** (in degrees, counterclockwise) and the **Rotation step** (the one used with `Shift` when rotating). The nine-point grid on the left chooses the **reference point**: the origin of X and Y, the scale anchor and the center of rotation. The lock keeps the proportions. The fields accept expressions: `100/3`, `+5` (add), `*2` (multiply), `50%`, `5in`. With **Edit Nodes**, this bar becomes the node bar (see [Drawing and editing](03-dibujar-y-editar.md)). Images cannot be rotated.

## Docked panel

On the right there is a tabbed panel. Drag its left edge to change the width; if it gets narrow, the tabs switch to icons only.

| Tab | Shortcut | What it is for |
|---|---|---|
| Layers | `Ctrl/Cmd+1` | cut settings and execution order |
| Machine | `Ctrl/Cmd+2` | connection, status, moving the head, job |
| Console | `Ctrl/Cmd+3` | the machine's commands and responses |
| History | `Ctrl/Cmd+4` | undo and redo steps |
| Camera | `Ctrl/Cmd+5` | see [Camera](10-camara.md) |
| Materials | `Ctrl/Cmd+6` | see [Materials](11-materiales.md) |
| Notes | `Ctrl/Cmd+7` | the project's notebook; it is saved with the project and is not sent to the machine |

You can **reorder** the tabs by dragging them (or with `Alt+Shift+←` and `→`), hide the ones you do not use and restore the order with the **Customize tabs** button; the order is remembered. More in [Guides, tabs, start point and project](03e-guias-puentes-y-proyecto.md).

The **History** panel lists every action (up to 200 steps). Click a step to go back to that point; what you undid can be recovered as long as you do not perform another action. The trash button clears the history without changing the design.

## Color palette and status bar

At the bottom, the strip of **layer colors**: 16 colors and the **Guides**. With objects selected, choosing a color moves them to that layer; with nothing selected, it makes that layer active for whatever you draw. `Alt+1` to `Alt+0` choose the first ten and `Alt+G` the guides. Guides are never sent to the machine.

The status bar shows the cursor position in mm, the zoom, the number of selected objects and their size, the total number of objects, whether the project is saved, the connected machine (with the **Virtual** label if it is an emulator) and the job's estimated time.

## Keyboard shortcuts

Press `?`, `F1` or `Ctrl/Cmd+/` to open the full list, with a search box (it shows the factory shortcuts). All of them can be changed: see [Preferences, shortcuts and autosave](02b-preferencias-y-atajos.md). The most used:

| Action | Shortcut |
|---|---|
| New, open, save | `Ctrl/Cmd+N`, `Ctrl/Cmd+O`, `Ctrl/Cmd+S` |
| New window | `Ctrl/Cmd+Shift+N` |
| New tab, close tab | `Ctrl/Cmd+T`, `Ctrl/Cmd+W` |
| Next, previous tab | `Ctrl+Tab`, `Ctrl+Shift+Tab` (see [Document tabs](03f-pestanas-de-documentos.md)) |
| Import design | `Ctrl/Cmd+I` |
| Undo, redo | `Ctrl/Cmd+Z`, `Ctrl/Cmd+Shift+Z` (or `Ctrl/Cmd+Y`) |
| Copy, cut, paste | `Ctrl/Cmd+C`, `Ctrl/Cmd+X`, `Ctrl/Cmd+V` |
| Select all, invert | `Ctrl/Cmd+A`, `Ctrl/Cmd+Shift+I` |
| Duplicate, delete | `Ctrl/Cmd+D`, `Delete` or `Backspace` |
| Group, ungroup | `Ctrl/Cmd+G`, `Ctrl/Cmd+Shift+G` |
| Lock, unlock all | `Ctrl/Cmd+L`, `Ctrl/Cmd+Shift+L` |
| Flip horizontal, vertical | `Alt+H`, `Alt+V` |
| Rotate 90° counterclockwise, clockwise | `[`, `]` |
| Bring to front, send to back | `Ctrl/Cmd+PageUp`, `Ctrl/Cmd+PageDown` |
| Boolean assistant, trim at last click | `Ctrl/Cmd+B`, `Ctrl/Cmd+K` |
| Break apart subpaths, optimize shapes, cut with a blade | `Alt+B`, `Alt+Shift+O`, `Alt+Shift+C` |
| Preferences | `Ctrl/Cmd+,` |
| Print (PDF at 1:1 scale) | `Ctrl/Cmd+P` |
| Move the selection | arrows: 1 mm; with `Shift` 10 mm; with `Alt` 0.1 mm |
| Preview | `P` |
| Frame, start | `F`, `Ctrl/Cmd+Enter` |

![Keyboard shortcuts dialog with the search box](../../../apps/desktop/docs/screenshots/09-atajos.png)

## Accessibility

Tlaku can be used without a mouse and with a screen reader. The canvas is graphical, but everything around it has keyboard support and a name.

- **Skip to canvas.** When you start tabbing, the **Skip to canvas** link appears; with `Enter` it moves focus to the canvas without going through all the menus and bars.
- **Dialogs.** Focus enters the dialog, `Tab` and `Shift+Tab` do not leave it, `Esc` closes it and focus returns to the control that opened it. Panels that are not modal (review design, art library, print and cut, tiling) also return focus when they close.
- **Menus.** `F10` moves focus to the menu bar; `↓` or `Enter` opens a menu, the arrows move through the entries and between neighboring menus, `Home` and `End` jump to the ends, typing a letter jumps to the entry that starts with it, and `Esc` closes and returns focus. The right-click menu and each layer's menu work the same way.
- **Toolbars.** The tool palette and the bars at the top are a single `Tab` stop; inside, you move with the arrows.
- **Side panel.** The tabs are a tab list: `←`/`→`, `Home` and `End` switch tabs; `Alt+Shift+←/→` reorders them. The customize button opens a menu that closes with `Esc`.
- **Floating panels.** Focus the panel's title bar: the arrows move it and `Shift` + arrows resize it. The corner handle resizes with the arrows.
- **Numeric fields.** `↑` and `↓` add and subtract one step; with `Shift` the step is 10 times larger and with `Alt`, 10 times smaller. `Enter` confirms and `Esc` reverts.
- **Simple mode.** When you change step, focus moves to the step title; buttons are at least 44 px.

**Screen reader**

- Icon-only buttons have a name, and the layer color swatches are announced by their name (they also show their number, so as not to depend on color).
- The canvas has a description (table, size and number of objects) and, when you select, a summary is announced: how many objects, size and lower-left corner. It updates a moment after each change so as not to read every step of a drag.
- Notices are announced on their own; warnings and errors with high priority. The job announces every status change (running, paused, finished, canceled, error) and its progress every 25 %.
- Lists and tables (layers, machines, marks, variable data) have a name.

**Display**

- Text meets a 4.5:1 contrast ratio and control borders 3:1, in both the light and the dark theme.
- Focus is always visible with an outline.
- With the system "reduce motion" option, animations are turned off.
- The phone's remote-control page follows the same rules: large buttons, announced status and progress, and a fixed **Stop** bar.

## Light or dark theme

The sun or moon button in the top bar (or **View > Switch Light/Dark Theme**) toggles between the two themes. Tlaku remembers your choice; by default it opens in dark. The same setting is in **Preferences > Appearance**, along with the canvas grid.

Next: [Preferences, shortcuts and autosave](02b-preferencias-y-atajos.md)
