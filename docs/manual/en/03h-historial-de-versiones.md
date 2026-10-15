# Version history

Saving overwrites the file, and **Undo** is lost when you close. **Version history** keeps how the project looked in the past so you can **go back to an earlier state even after saving**. Everything is **local**: versions never leave your computer.

Open it with **File > Version history…**. **File > Save version…** stores a named version.

## What is kept and when

| Kind | When it is created | What it stores |
|---|---|---|
| **Saved** | Every time you save a project that already had a file | What the file held **just before** being overwritten (with the file's date). The first save creates none: there was nothing to keep yet |
| **Automatic** | Every few minutes (5 by default) while you edit, only if there are unsaved changes | The document as it is on screen, even if you have not saved it |
| **Named** | **File > Save version…** | The document as it is now (including unsaved changes) and the name you type. **Never deleted automatically** |
| **Before restoring** | When you restore by replacing the document (see below) | The state you had, so you can go back |

- **Unchanged content is not repeated**: if the content equals the newest version, no other one is stored. If you save a named version of a document that has not changed since the last one, that version takes your name.
- Projects **without a file** ("Untitled") have no history: the recovery copy protects them (see [Document tabs](03f-pestanas-de-documentos.md#recovery-and-session)). As soon as you save them, they start having history.
- Each tab has its own history (that of the file it shows). Saving a background tab keeps the version of **its** file.
- Versions are compressed (gzip): a typical project takes a few KB per version.

> **Images do not travel inside the project**: the `.tlaku` stores their **path**. An old version recovers the design as it was, but if you moved, deleted or changed the image file, the version will show the new image or will not find it. The same goes for the CSV files of variable data.

## The dialog

On the left, the list of versions, **newest first**: thumbnail, name or date, kind (saved, automatic, named, before restoring), number of objects and layers, and size. When you pick a version, on the right you see:

- a large **preview** (it does not touch your document);
- **what restoring it would change**, compared with the open document: how many objects you added since would be lost, how many you deleted would come back, how many would go back to how they were, and the layers, notes, guides or device that would change. If the file is not open in any tab, there is nothing to compare with;
- the buttons below.

### Restore

- **Restore as a new tab**: opens the version in a tab **with no file** and with unsaved changes. Your document is not touched. Save it under another name to keep it.
- **Replace the open document…**: asks for confirmation and replaces the document of the tab that has that file. First it stores what you had as a **"Before restoring"** version, and the replacement is **a single Undo step**: **Undo** takes you back, and if you already closed or saved, that version is still in the list. If that backup cannot be stored (for example, the disk is full), nothing is replaced. Only available if the file is open in a tab.
- The document is **unsaved** after replacing: the file does not change until you save. When you save, what the file held becomes a "Saved" version.

### Other actions

- **Highlight what changes on the canvas**: selects in the open document the objects you added or modified since that version (the ones that would be lost or changed on restore).
- **Delete this version**: asks for confirmation and removes it for good.
- **History of another file…**: pick a `.tlaku` and open its history **even if it is not open** (you can only restore it as a new tab). If you keep versions in the app data folder, the **Other projects with history** list shows every project that has versions, including ones you **moved or renamed** (a file is identified by its path: moving it starts a new history and the old one stays in that list).

## Where they are stored

By default, in the **application data folder**, in a `versiones` subfolder with one folder per project:

| System | Folder |
|---|---|
| macOS | `~/Library/Application Support/io.github.jahziel1.tlaku/versiones/` |
| Windows | `%APPDATA%\io.github.jahziel1.tlaku\versiones\` |
| Linux | `~/.local/share/io.github.jahziel1.tlaku/versiones/` |

This does not clutter your folders and does not travel by mistake with the project when you email it or sync it to a cloud (versions hold the whole design, for example a client's). If you prefer them to **travel with the file**, in **History settings** pick **Next to the file**: they are stored in a hidden `.tlaku-versions` folder beside the project. When you change the location, the earlier history stays visible; only new versions go to the chosen place. To **delete a project's whole history** use **Delete this project's whole history…** (asks for confirmation).

## History settings

They are changed in the dialog itself (**History settings**) and applied with **Apply**:

| Setting | Default | What it does |
|---|---|---|
| Keep the previous version on every save and take snapshots | Yes | Off, no versions are stored on their own (named ones still work) |
| Where to store | App data folder | Or next to the file, in `.tlaku-versions` |
| Automatic snapshot every | 5 min | `0` = never |
| Keep every version from the last | 24 h | |
| Then keep one per day up to | 30 days ago | The newest of each day stays |
| Then keep one per week up to | 26 weeks ago | Older than that, automatic ones are deleted |
| Size cap per project | 200 MB | What they take on disk |
| Version cap per project | 500 | |

After every new version, **cleanup** runs: the age tiers are applied and then the caps (oldest first). Cleanup **never deletes the newest version or named versions**, so if only those remain a cap may be exceeded. Days and weeks are counted in UTC.

## If something fails

- **Saving the project does not depend on the history.** If a version cannot be written (full disk, no permission), Tlaku tells you and **saves the project anyway**.
- Writes are atomic: a power cut mid-write never leaves half a version; leftovers clean themselves up.
- If a project's history looks empty in the dialog but there are files in the folder, check the chosen location: the dialog looks in both.
- A project **larger than 128 MB** is not stored in the history.

## What it does not do

- It is not branching or merging version control: it is one timeline per file.
- It does not protect image files or external CSV files (see above).
- It does not sync between computers: everything is local. With **Next to the file**, whatever your cloud syncs is up to your cloud.
- It does not store **machine settings** or preferences changes; only the project.
