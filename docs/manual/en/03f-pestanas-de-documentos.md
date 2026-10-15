# Document tabs

Open **several projects in the same window**, each in its own tab, like in a browser. LightBurn does not have this (it opens one project per window, and its *Pages* are something else: sheets inside one project). A tab is a complete document: its design, its layers and cut settings, its **Undo** history, its file and its saved state. Shortcuts are written with `Ctrl/Cmd`: `Cmd` on macOS, `Ctrl` on Windows and Linux.

## The tab bar

It sits above the canvas. Each tab shows:

- the **file name**, or **Untitled 1**, **Untitled 2**... if it has not been saved yet;
- an **orange dot** when it has unsaved changes (a screen reader announces it as "has unsaved changes");
- a **printer** if the job running on the machine was started from that tab (see [There is only one machine](#there-is-only-one-machine));
- an **X** to close it. Middle-clicking, `Del` on the focused tab and `Ctrl/Cmd+W` also close it.

The **+** button (or `Ctrl/Cmd+T`) opens an empty tab. Drag a tab to **reorder** it. Right-click (or the menu key, or `Shift+F10`) opens its menu:

| Option | What it does |
|---|---|
| Close tab | Like the X; if it has changes, it asks first |
| Close other tabs | Leaves only this one. If several have changes, a single question with the list |
| Duplicate tab | Copies the document to a new tab on its right. The copy **has no file** and counts as having unsaved changes: save it to keep it |
| Move left / Move right | Reorders without dragging (works with the keyboard) |
| Move to a new window | Moves the tab to another Tlaku window (see below). Not available if it is the only tab |

> **There is no** "Show in folder" yet: the app is not allowed to open the system file explorer.

With many tabs the bar scrolls, and the active one always stays in view. In [simple mode](01b-modo-sencillo.md) the bar only appears if there is already more than one tab.

## What belongs to each tab and what belongs to the whole window

| Per tab | Per window |
|---|---|
| The design, the layers and the cut settings | The **clipboard**: copy in one tab and paste in another (with the settings of the layers it uses) |
| The **Undo** and **Redo** history | The **machine** (connection, status, job in progress) and the active machine profile |
| The file and whether there are unsaved changes | The **preferences** and the shortcuts |
| The selection, the active layer and the selected nodes | The active tool, the theme and the panel layout |
| The zoom and the pan | The live view |
| The preview (visible, progress, result) and the **job options** (origin, layer order, optimization) | |

A new tab inherits the job options of the one you had (so you do not repeat the job origin in every document), without the selection or the tiling.

## Opening files

- **Open project** (`Ctrl/Cmd+O`) and **recent files** open the project in a **new tab**. If the active tab is **empty and has no changes**, it is replaced instead of leaving an empty tab. If the project is **already open**, Tlaku activates its tab and tells you.
- **New project** (`Ctrl/Cmd+N`) opens an empty tab; nothing needs to be saved first, because no document is lost. **New from template**, the camera marks, the material test and the rotary test do the same: each in its own tab.
- **Dragging design files** onto the canvas or **Import** (`Ctrl/Cmd+I`) adds them to the **active** tab, as always. If you switch tabs while importing, the result goes into the tab where you asked for it, not the one you are looking at now.
- The same goes for saving, exporting, tracing, nesting, inserting art and the other operations that take a while: they end up in their tab of origin. If you closed that tab meanwhile, Tlaku warns you and does not write to another one.

## Shortcuts

| Action | Shortcut |
|---|---|
| New tab | `Ctrl/Cmd+T` |
| Close tab | `Ctrl/Cmd+W` |
| Next / previous tab | `Ctrl+Tab` / `Ctrl+Shift+Tab` (on macOS it is also `Ctrl`, because `Cmd+Tab` belongs to the system) |
| Move around the bar (with focus on it) | `←` `→` (they activate the tab), `Home`, `End` |
| Close the focused tab | `Del` |

There is no `Cmd+1..9` or `Cmd+Option+←/→`: they already have another owner (`Ctrl/Cmd+1..7` open the side panels and `Ctrl/Cmd+Alt+arrows` jog the machine head), and Tlaku does not reassign existing shortcuts. You can assign whatever you like to the tab commands in **Preferences > Shortcuts**.

> **macOS:** the system's default menu has **Close Window** on `Cmd+W`. Tlaku removes it so that `Cmd+W` closes the tab; the window closes with its red button.

## Closing with changes

- **A tab with changes:** closing it shows **Unsaved changes** with **Save**, **Don't save** and **Cancel**. If it was a background tab, it is activated so you can see which one it is. **Save** writes that tab's document; if saving fails, the tab does not close.
- **Closing the last tab** leaves a new empty tab (the window does not close).
- **Closing the window or quitting** with a single dirty tab: the usual question. With **several**: an **Unsaved changes in N tabs** dialog with the **list** of names and the buttons **Save all** (saved one by one; if one fails or you cancel the file picker, it stops and nothing closes), **Discard all** and **Cancel**.
- **Close other tabs** does the same with the other tabs.
- **File > Save all tabs** saves every tab with changes.

## Recovery and session

- **One recovery copy per tab** with changes (each with its own name, for example "Untitled 2"). It is deleted when you save that tab, go back to the saved state with **Undo**, or close it. Up to 40 copies are kept across all windows.
- After an unexpected shutdown, **Unsaved work found** lists each copy; each **Recover** opens it in **its own tab** (marked as unsaved), and **Recover all** opens them all at once. If the source project is already open and has no changes, the copy takes that tab instead of duplicating it. With several windows, each one only offers orphan copies: those of other open windows are not mixed in.
- **Reopen tabs at startup** (**Preferences > General**, on by default): when Tlaku starts, the projects that were open in tabs when it closed are opened again and the one that was active is active again. **Untitled** tabs are not reopened (if they had unsaved work, recovery offers it). If a file no longer exists, you are told and the rest continue. Only the main window saves and restores the session.

## There is only one machine

The machine, its connection and the job in progress belong to the whole window; tabs do not duplicate them.

- A job **uses its own copy**: switching tabs, or editing another one, does not change what is being cut. If you close the tab of origin, the job continues.
- The tab you started the job from carries a **printer** while it runs. The **live view** (and the phone's) keeps showing that job even if you switch tabs, and the progress, the estimated time and the current layer are those of that job.
- **Preview**, **Frame** and **Start** act on the **active** tab, and say so: the toolbar for those buttons is called "Job of *name*" when there are several tabs, and the start dialog says **"name" will be sent to *machine***.
- When a job finishes successfully, the automatic advance of variable text (serial number and CSV row) is applied in the tab of origin, not the one you are looking at.

## Moving a tab to a new window

**Move to a new window** opens another Tlaku window (see [New window](03e-guias-puentes-y-proyecto.md#new-window)) with that document, its unsaved changes and its file, and closes the tab here. The document travels as a recovery copy with its own id: if the new window never picks it up, the copy stays available for recovery the next time you open Tlaku. If the window cannot be opened, the tab stays where it was.

## Memory

Every **Undo** step keeps the geometry that the action replaced, so several tabs with large documents multiply the memory. The **active** tab keeps its full history (up to 200 steps); a **background** tab keeps the **30 most recent steps** and a budget of about 200 thousand geometry points (a tenth of the active one, about 10 MB). Only old history is trimmed: the document and its saved state do not change. Measured with a document of 6,000 objects of 40 points and 150 edits per tab, a background tab's history goes from about 243 thousand points to 96 thousand. Five such tabs take about 350 MB in total instead of about 410 MB.

## Accessibility

The bar is a `tablist` with one `tab` per document, and the design area is its `tabpanel`. Only one tab is a `Tab` stop; `←`/`→` move through them and activate them. Each tab's name includes whether it has unsaved changes or a job in progress. Texts are in Spanish and English.

Next: [Text](04-texto.md)
