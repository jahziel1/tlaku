# Cost and price per piece

If you sell what you make (Etsy, Mercado Libre, craft fairs, custom orders), **Cost and price** tells you what a piece costs to make and what to sell it for so that, after the platform fee and taxes, you keep the profit you want. It uses the figures Tlaku already works out for the job (time, laser-on time, area and cut length), so it updates by itself when you change the design.

It is a **quoting aid, not a promise**: the time is an estimate and you type the prices. **Everything stays on your computer; nothing is sent to the internet** (there is not even an exchange rate). Shortcuts are written with `Ctrl/Cmd`: `Cmd` on macOS and `Ctrl` on Windows and Linux.

## Opening it

- The **Cost and price…** button in the **Machine** tab (Job section), next to **Optimization settings…**.
- The same button in the **preview** bar.
- The menu **Machine > Cost and price…**.

A dialog opens with your inputs on the left and the **result** on the right: the **suggested price per piece** in large type, the cost, the breakdown and the price of the batch. While it is open the result follows the job: if the design, the layers, the active machine or the selection change, it recalculates.

## What it asks for

### Machine time

Choose which time to use:

- **Tlaku estimate**: the one from the preview. It is an estimate and tends to run short against reality; the result warns about it ("Uncalibrated time estimate").
- **Estimate calibrated with your jobs**: the estimate multiplied by how long your finished jobs on that machine really took. It uses the [job history](09f-cola-e-historial-de-trabajos.md) (the median of real time ÷ estimate over the last 20 jobs) and needs **at least 3**. The dialog shows the factor ("jobs took 1.35x the estimate").
- **Real time of the last job**: the time of the last finished job on that machine, not counting pauses. Handy if you always repeat the same piece.

If you pick an option and there is not enough history yet, it is disabled and the estimate is used (the dialog says so).

### Machine and electricity

These values **are remembered per machine**: when you switch the active machine, its own values appear, and a new machine starts at zero.

- **Machine cost per hour**: what you want to recover for the purchase, the workshop and upkeep for each hour of work. It is charged on the whole time of the run.
- **Price** and **lifetime** of the tube (CO2) or the diode, in hours with the laser on. **Wear** is price ÷ lifetime, and **it is charged only for the time the laser is on** (cutting and engraving, not rapid moves). With a lifetime of 0 it is not counted.
- **Power draw of the machine** and **of accessories** (exhaust fan, air compressor, water chiller), in W, and the **price per kWh**. Electricity is (machine W + accessory W) ÷ 1000 × hours × price per kWh.

### Material

Three ways to charge for it:

- **By area (rectangle around the design)**: the area of the box that surrounds everything the job outputs, plus **waste**. It is the simplest, and if you nest pieces there will be material left over; the result warns about that.
- **By real area of the shapes**: the sum of the areas of the **closed** shapes (a ring or the "o" of a text subtracts its hole). Open strokes have no area; if the design has no closed shape the surrounding box is used, with a warning. Two objects that overlap are counted separately.
- **By sheet (pieces per sheet)**: sheet price ÷ the pieces that come out of each sheet with your layout. It is the fairest if you arrange them with [nesting](03d-copias-y-nesting.md). It shows how many **sheets are needed** for the quantity.

In the first two you type the **sheet price and size** (width × height in mm) and the cost is the used area × (1 + waste) × price ÷ sheet area.

**Material profiles.** Type a name and press **Save profile** to remember the price, the size, the waste, the pieces per sheet and the way of charging; pick it later from the **Material profile** list. Saving under a name that already exists replaces it. **Delete profile** asks for confirmation. There can be up to 100 profiles.

### Labor and extras

- **Labor rate per hour**.
- **Batch setup** (minutes of design, settings and tests): paid **once** and spread over all the pieces.
- **Finishing per piece** (minutes of sanding, gluing, assembling).
- **Packaging and supplies per piece**: a fixed cost (bag, card, glue).

### Sale

- **Currency**: by default the one for the system region (`es-MX` gives Mexican pesos; with no region, MXN in Spanish and USD in English). Amounts show with your region's symbol and format. Currencies with no cents (CLP, PYG) round to the whole unit.
- **Quantity to sell** and **pieces per run of the design**: if the design already holds 12 keychains, enter 12, and time, electricity and material are shared among them. The **Use the N in the design** button counts the pieces for you (each group counts as one and the copies of a [virtual array](03d-copias-y-nesting.md) add up).
- **Profit margin** and what it is taken on (the cost or the sale price).
- **Platform fee** and **taxes**, in %.
- **Round the price up to**: the cent, 0.50, 1, 5 or 10.

## How it is calculated

Everything is per piece. Each line of the cost is rounded to the currency's cent, so the breakdown adds up exactly.

```
cost = machine + wear + electricity + material + finishing + setup ÷ quantity + supplies
```

The **price** is worked out from the cost `C`, the margin `m`, the fee `c` and the taxes `t` (all as fractions: 30 % is 0.30). The fee and the taxes are charged on **the final price** (what the customer pays), not on the cost; that is why they are **divided out** instead of added. What you keep is `price × (1 − c − t)`, and that must cover the cost plus your profit:

```
margin on cost:   price = C × (1 + m) ÷ (1 − c − t)
margin on price:  price = C ÷ (1 − c − t − m)
```

The dialog has a **How it is calculated** section with the formula and **your numbers** already filled in (`$60.00 × (1 + 0.3) ÷ (1 − 0.1 − 0) = 86.6667 → $86.67`). Then the price is **always rounded up** to the chosen step, so the real profit never drops below the one you asked for.

> **Example.** A 1 h run (30 min with the laser on), a 100 × 100 mm box on a 600 × 400 mm sheet that costs 120, machine at 60 per hour, a 1,200 tube with 2,000 h of life, 120 W at 2.50 per kWh, labor at 100 per hour with 30 min of setup and 6 min of finishing, 2 for packaging and 10 pieces: machine 60 + wear 0.30 + electricity 0.30 + material 5 + finishing 10 + setup 5 + packaging 2 = **82.60** of cost. With a 30 % margin on cost and a 10 % fee: 82.60 × 1.3 ÷ 0.9 = 119.31 → **119.32** per piece (1,193.20 for the batch), of which 11.93 is the fee and 24.79 your profit. Adding "30 % + 10 %" to the cost would give 118.18, which falls short.

**Taxes added on top of the price** (16 % VAT): the box is the percentage **of the final price**. If your tax is added on top, type 16 ÷ 1.16 = **13.79 %**.

If the fee, the taxes (and the margin, when it is taken on the price) add up to **100 % or more**, no price covers them: the result says **No price is possible** and what to lower.

## Warnings it shows

- **Uncalibrated time estimate** (usually lower than the real one) and **distance-based approximate time** (when the engine did not give the exact times).
- **The material is worked out from the rectangle around the design**, **there are no closed shapes**, **the sheet size is missing** or **the pieces per sheet**.
- **There is no job time or area yet**: with no objects in layers with output enabled only fixed expenses count.

With **Selection only** on (in the job panel), the calculator also counts only what is selected.

## Copying and saving the breakdown

At the bottom of the dialog:

- **Copy breakdown**: text ready to paste into a message, a quote or a note, with the job, the cost by line, the price and the batch.
- **Copy CSV** and **Save CSV…**: the same rows as `Section,Item,Value,Unit`, in the interface language. Numbers use a **decimal point and no thousands separator** so any spreadsheet reads them; the file carries the UTF-8 mark so Excel shows accents correctly. If your spreadsheet expects semicolons (some regional settings), use **Data > Text to Columns** and choose the comma.

## What is remembered

In this computer's `localStorage` (key `tlaku.costs`), without leaving it: each machine's values, the material profiles, the currency, the rates, the margin, the fee, the taxes, the rounding, the quantity and the time source. They do **not** travel in the project or in preference bundles.

## What it does not do

- **It does not promise a price**: the time is an estimate and it does not know what your hour is worth or what your platform charges. Look at the breakdown and adjust.
- It does not know material prices or platform fees, and it does not look up exchange rates.
- It does not work out how many pieces fit on a sheet: [nesting](03d-copias-y-nesting.md) does that; you type the result in **Pieces per sheet**.
- Real area does not subtract what overlaps between different objects.

Next: [Connecting your machine](08-conectar-maquina.md)
