# Variable text

To engrave serial numbers, names from a list or today's date without typing each part by hand, a text can carry **variables** in braces. The text you save in the project is the **template** (`Part {serial:4}`); when previewing, sending or exporting, Tlaku replaces it with the values of that moment (`Part 0007`) and generates the outline of the letters with them. The saved project always keeps the template.

## Writing variables

Select a text and use the **Insert variable** menu in the text panel: it puts the variable where the cursor is. You can also type them by hand.

| Variable | What it puts |
|---|---|
| `{serial}` (or `{serie}`) | The current serial number. `{serial:6}` pads it with zeros to 6 digits; `{serial:hex}` and `{serial:HEX}` write it in hexadecimal (`{serial:hex4}` with 4 digits). |
| `{csv:Name}` | The data from the **Name** column in the current CSV row. The column number starting from 1 is also accepted: `{csv:2}`. With several CSVs, `{csv.source:Name}` picks the source (see below). |
| `{row}` (or `{fila}`) | The number of the current CSV row, starting from 1. `{row.source}` is the one that source gets. |
| `{date}` (or `{fecha}`) | Today's date, `dd/MM/yyyy` by default. `{date:format}` writes it your way. |
| `{time}` (or `{hora}`) | The time, `HH:mm` by default. `{time:format}` changes the format. |
| `{copy}` (or `{copia}`) | The copy number, starting from 1. |

The names work in Spanish and in English, and capitalization does not matter.

**Offset.** Any serial, CSV, row or copy variable accepts `@N` at the end: `{csv:Name@2}` is the data from two rows ahead, `{serial@3}` the number three steps ahead. It is what **One copy per row** uses (below).

**Literal braces.** To write a brace, double it: `{{` gives `{` and `}}` gives `}`. So `{{serial}}` is engraved as `{serial}`.

**Date and time format.**

| Mark | Meaning |
|---|---|
| `d`, `dd`, `ddd`, `dddd` | Day (1 or 2 digits), abbreviated and full weekday name. |
| `M`, `MM`, `MMM`, `MMMM` | Month (1 or 2 digits), abbreviated and full. |
| `yy`, `yyyy` (or `aa`, `aaaa`) | Year with 2 or 4 digits. |
| `H`, `HH` / `h`, `hh` | Hour from 0 to 23 / from 1 to 12. |
| `m`, `mm` / `s`, `ss` | Minutes / seconds. |
| `AP` / `ap` | AM or PM / am or pm. |
| `'text'` | Text as is: `{date:'Made on' MMMM d, yyyy}` gives `Made on September 3, 2025`. |

A lowercase `m` means **minutes** if it sits next to an hour or to seconds (`HH:mm`, `mm:ss`) and **month** in all other cases: `{date:dd/mm/yyyy}` also works. Month and day names come out in the interface language.

**If something is not understood** (a name that does not exist, an unclosed brace, a column that is not in the CSV) that part is left written as is, the text panel warns you and **sending is blocked** until it is fixed: a half-written text is never engraved.

## The "Variable text" dialog

It opens from **Tools > Variable text…** or from the button in the text panel.

![Variable text dialog with the serial number, the CSV table and One copy per row](../../../apps/desktop/docs/screenshots/43-texto-variable.png)

- **Serial number.** The **next number**, the **step** (how much it increases per job) and the minimum **digits** (pads with zeros). With **Previous** and **Next** you move the number one step.
- **Data sources (CSV).** **Choose CSV…** (or **Add another CSV…**), **Reload** and **Remove this source**; with several files see "Several CSVs in one project". UTF-8 files are accepted (with or without BOM), with comma, semicolon or tab separator (detected automatically), fields in double quotes and line breaks inside a field. The first row is the headers. Up to 100,000 rows.
- **The table** shows the rows and highlights the current one, the one that will be engraved in the next job. A click on a row chooses it; you can also use **Previous row**, **Next row**, **To the start** or **Go to row** (**Preview by row** group).
- **Advance.** With **Advance when a job finishes** on, when a job finishes successfully the serial goes up one step and the CSV moves to the next row. A canceled job or one with an error does not advance. The advance can be undone. If the job used N copies (see below) it advances N steps.
- **End of the CSV.** When the last row is engraved the app warns you. With the row already past the end, the next job does not go out (it would say there is no data) until you choose another row or go back to the start, so the last label is not repeated by mistake.
- **Show the template on the canvas.** By default the canvas shows the current values. This option (also in the text panel) shows the braces, so you can edit them. When sending, the values always go out.
- **Objects with variables in the design.** Lists each template (from texts, images and codes) with what would be engraved now and any problems it has.
- **Check data.** Lists the CSV rows with missing data (see below).

> **Heads up:** the serial and the row live in the project. For the next number to survive closing the program, **save the project** after engraving (the advance marks the project as modified).

## One copy per row

With a label selected (for example the text and its frame), **One copy per row** creates the number of copies you indicate in a grid (by default, the rows remaining in the CSV). Each copy takes the next row: in the third one, `{csv:Name}` becomes `{csv:Name@2}`, and likewise with `{serial}`. That way you engrave many labels in **a single job**. It is a single undo step.

## Several CSVs in one project

A project can use **several CSV files at once**: for example one with people and another with products. In the dialog, **Data sources (CSV)** lists each file with a short **name** (by default the file name without extension; letters, numbers, `_` and `-` only). **Add another CSV…** adds more; **Change file…**, **Make main** and **Remove this source** manage them. If you rename a source, the texts, images and codes that used the old name are updated automatically.

Each variable picks its source by name: `{csv.products:Name}` is the **Name** column of the source `products`, and `{row.products}` is the row it gets. Without a name (`{csv:Name}`) it is the **main source**, the first one in the list. The **Insert variable** menu offers the columns of every source already read.

**What happens when they have different numbers of rows.** There is a single rule: **the main source rules**.

- The current row and the number of rows in the job are the main source's; the other sources are read with **the same row**. Past the main source's last row the data has run out for all of them.
- A **shorter** secondary source **repeats from the start** (with 3 rows, row 4 of the main source reads its row 1). If you would rather not repeat, choose **Block sending** on that source: past its last row sending stops and the notice names the source and the row.
- A **longer** secondary source is cut: the extra rows are not used.
- The dialog flags each case as soon as the files are read ("has 2 rows and the main source 3: it repeats from the start").

## Variable images and logos

Select an **image** and, in its properties, tick **Variable image (the file comes from a column)**. Type the path template (`{csv:Logo}`) or pick it with **Column with the path**.

- The path in the cell can be **absolute** or **relative to the folder of the CSV** the column comes from (`logos/mug.png`).
- When previewing, sending or exporting, the image is replaced by each row's file and **fitted into the box of the marked image**: **Whole, centered** keeps it complete (there may be spare space) and **Fill and crop** covers the box and crops what sticks out. The box is the size the sample image has in the design.
- The image keeps its **layer**, so it is engraved with that layer's **image mode** (threshold, dither, grayscale...). Set up the sample image with the layer color you want.
- If a file is missing, the cell is empty or the file is not a readable image, **sending is blocked** and the notice says which one, in which row and in which template. Nothing else is ever engraved by mistake.
- With **One copy per row** each copy takes the logo of its row.

## Variable QR and barcodes

In **Tools > Insert code…** the **Content** can contain variables: `https://shop.example/{csv:Id}`, or `{csv.products:Sku}` in a barcode. The dialog's preview uses the current row (and the **Insert CSV column** picker adds the variable). The inserted code remembers its settings and is **generated again when sending** with each row's data, centered where it was. If a row's content ends up empty or the generator rejects it (an EAN-13 with too few digits, for example), sending is blocked and the row is named. Loose braces without a variable (JSON, for example) are encoded as they are; to write a brace next to a variable, double it (`{{`).

## Preview by row and data check

- **Preview by row.** The group of that name shows **Row X of N** and has **Previous row**, **Next row**, **To the start** and **Go to row**. The current row is the one you see on the canvas, in the job preview and in what is sent. With several sources, the **Show source** selector shows each one's table with the row it gets highlighted.
- **Check data.** The **Check all rows** button goes through the CSV with what the design uses and **lists the rows with missing data**: empty cells, images that do not exist or have an empty path, codes without content and rows past the end of a source set to "block". A click on a row makes it the current row so you can see it. A column or source that does not exist is reported only once.
- **When sending.** The rows of that job are always checked: missing image files and codes that cannot be generated block sending; empty cells in a text do not block it but you are warned with the row.

## In the project

Sources are saved in the `.tlaku` with a **path relative** to the project's folder when the CSV is in that folder or near it (up to three levels up), so you can move the whole folder; if it is far away the absolute path is saved. When the project is opened the paths become absolute again. A project from an earlier version, with a single CSV, opens with that file as the main source. Image paths in cells are resolved against the CSV's folder, so they travel with it.

## Practical examples

**Twenty numbered labels in a single job.** Type `Part {serial:4}` inside a frame, select the text and the frame, and in the **Variable text** dialog set the **Next number** to 1. Use **One copy per row** with 20 copies and 5 **per row**: you get `Part 0001` through `Part 0020`. With **Advance when a job finishes**, when the job finishes successfully the next number becomes 21; save the project to keep it.

**One label per person.** In a spreadsheet make the columns **Name** and **Role**, save it as CSV, and in Tlaku choose that file with **Choose CSV…**. With the text `{csv:Name}` and, below it, `{csv:Role}`, each job engraves one person and moves to the next row. To engrave all of them at once, **One copy per row** with as many copies as rows (by default it proposes the ones remaining).

**Manufacturing date on each part.** `Made on {date:MMMM d, yyyy}` engraves, for example, "Made on October 13, 2026" with the date of the day you send the job, without touching anything.

**A label with a logo and a QR per product.** A CSV `products.csv` with the columns **Product**, **Logo** (image path) and **Sku**. Put a text `{csv:Product}`, a sample image marked as variable with `{csv:Logo}` and a QR with `https://shop.example/{csv:Sku}`. Select the three, use **One copy per row** and send: each label comes out with its name, its logo and its QR. Beforehand, **Check all rows** tells you whether any logo does not exist.

**A brace that really is a brace.** `Code {{A}}` is engraved as `Code {A}`.

## What is missing or different

- Variables work in **texts**, in **images** (the file path) and in **QR and barcodes** (the content). Not in shape fills or in vectors.
- On the canvas, a variable image or code shows those of **the current row** (like the text), as long as the CSV is loaded and the file exists; otherwise you see the sample. This is only what you see: the project keeps the sample, and selecting or editing the object works on it. **Show the template on the canvas** shows the sample again. The job preview and what is sent use the same row data. Check the row in the preview before engraving.
- Secondary sources are read with the same row number as the main one; there is no row of their own per source and no key-based join (as in a database).
- There is no row selection (range, filter) or bulk find and replace; nor QR codes with their own numbering (use `{serial}` in the content for that).
- There is no variable mode for layer settings (speed, power) and no "baking" the text into fixed values; to fix a text, convert it to paths.
- Tlaku's syntax is its own: LightBurn files with variable text are not translated automatically.
- None of this has been tested with a real machine yet; it was tested with the emulator and the virtual machine.

Next: [Layers and cut settings](05-capas-y-ajustes.md)
