# Community material settings

[Español](README.es.md) · English

Material settings shared by people who tested them on their own machines. Each file in this folder is
**one contribution** (one GitHub issue) and ends up in the app's material library, shown as
**Community (unverified)**.

> These are one person's values on one machine. Lasers, lenses, focus, air and materials differ.
> **Always run a test on scrap first**, starting with less power than the listed value.

## How a setting gets into the library

1. **Issue.** Someone opens the [Share material settings](https://github.com/jahziel1/tlaku/issues/new?template=material_settings.yml)
   form (or uses *Share this setting…* on one of their own presets in the app's Materials panel, or *Share
   a setting of mine…* in the community browser; both open the form already filled in, and nothing is
   sent until the person publishes it on GitHub). They confirm they tested the values and agree to publish
   them under CC0.
2. **Review.** A maintainer checks that the values make sense for the machine and the material, that the
   result description and photo match, and that nothing unsafe is suggested (for example a cut that
   needs a power the machine cannot give, or a material that must not be lasered).
3. **Pull request.** The maintainer generates the file from the issue and opens a pull request:

   ```sh
   node scripts/community/issue-to-preset.mjs 42        # reads issue #42 with `gh`, writes community/materials/42.json
   cargo test -p tlaku-doc community                     # validates every file in this folder
   ```

   The script converts mm/s to mm/min when the contributor wrote the unit, picks the machine class from
   the laser type and watts, and records the issue URL as the `source`.
4. **Library and catalog.** Once merged, the file reaches people in two ways:
   - It is compiled into the next app version (`crates/tlaku-doc/build.rs`) and appears in the Materials
     panel with the *Community (unverified)* tag, next to the factory settings.
   - The **Site** workflow (`.github/workflows/pages.yml`) compiles every file in this folder into the
     published catalog `https://jahziel1.github.io/tlaku/catalog/v1/index.json`, **without waiting for a new
     app version**. The app downloads it only when the person opens *Browse the community…* in the
     Materials panel (see [the catalog](#the-published-catalog)).

## Reviewing a contribution (maintainers)

Nothing enters the library without a person reading it. Before opening the pull request:

- [ ] The values make sense for the machine and the laser power (the validator also checks the class
      against the watts). A cut that needs more power than the machine gives is rejected.
- [ ] The result description and the photo match the values.
- [ ] Nothing unsafe is suggested (materials that must not be lasered: PVC and vinyl, chrome-tanned
      leather, coated fiberglass...; no instruction to skip the test).
- [ ] The text is the contributor's own tests, not a table copied from a copyrighted manual or site.
- [ ] `notes` has no links, advertising or personal data (the app shows it to everyone).
- [ ] `machine_model` is set **only** if the machine the person wrote clearly names a model of the app's
      machine catalog (`apps/desktop/src/features/devices/catalog*.ts`): use its `id`
      (`--machine-model creality-falcon2-22w`). If unsure, leave it out: the class is still compared.
- [ ] There is no almost identical contribution already (same material, thickness, operation, machine class).

The pull request adds `community/materials/<issue>.json` and nothing else. CI validates it
(`cargo test -p tlaku-doc community` and the catalog build).

### Confirmations ("verified by N people")

A confirmation is a **different** person saying they tried the same values on their own machine.

1. The person comments on the original issue: machine, laser power, material and what came out
   (a photo helps). A reaction (thumbs up) does not count.
2. A maintainer reads the comment and adds an entry to `confirmations` of that contribution's file in a
   pull request:

   ```json
   "confirmations": [
     { "by": "github-user", "machine": "Machine as they wrote it", "machine_class": "diode_20",
       "date": "2026-10-20", "source": "https://github.com/jahziel1/tlaku/issues/42#issuecomment-123456" }
   ]
   ```

3. The validator rejects a confirmation by the contributor themselves and the same person twice. The
   catalog counts the entries (`verified_by`) and the app shows *Verified by N people* with the list.

"Verified" means *other people say it worked for them*. It is not a safety certification: the warning to
test on scrap first is always shown.

### "Same machine" is declared, not proven

Nobody can prove which machine someone owns. What the catalog records (`machine`, `laser_watts`,
`machine_class`, and `machine_model` when a maintainer could match the text to the app's catalog) is what
the person **declared**. The app compares your active machine with it (same model, same class, same laser
type with another power, other laser) and says so; anything other than the same model is shown as "only a
rough guide". Even the same model can differ in lens, focus, air and material.

## The published catalog

`catalog/v1/index.json` is generated by `cargo run -p tlaku-doc --example community_catalog -- --out <file>`
(the Site workflow does it; CI checks it on every change). Format (schema version 1):

```json
{
  "schema": "tlaku.community-materials",
  "version": 1,
  "generated_at": "2026-10-15",
  "entries": [ /* one object per contribution, sorted by id */ ]
}
```

Each entry has the fields of a contribution file (below) plus the calculated `operation`
(`cut`, `score`, `engrave`, `fill` or `image`, from `role` and `mode`), the effective `mode`
(`line`, `fill` or `image`) and `verified_by`. The Rust types are `CatalogIndex`/`CatalogEntry` in
`crates/tlaku-doc/src/community_catalog.rs` (TypeScript bindings are generated from them). The version
is part of the path: a change that would break older apps is published as `catalog/v2/` and `v1/` stays.
Fields added later to `v1` are optional and older apps ignore them.

The app **validates the catalog before showing it** and rejects it whole on the first problem: size up
to 4 MiB, up to 5000 entries, power above 0 and up to 100 %, positive and bounded speeds, passes, thickness,
interval and frequency in range, laser class against watts, the CC0 license, the issue address and
consistency of the calculated fields. A catalog is data only: nothing in it is run, and applying a
setting always goes through the library or an explicit confirmation to change a layer. It never sends
anything to the machine. See `docs/app-api/materiales.md` for the network rules.

There are **no contributions yet**, so the catalog is published empty and the app says so and invites
people to share one. Example of a file (**invented for documentation; it is not a real setting and must
not be used as one**):

```json
{
  "id": "community.basswood.3.cut.diode_20.999",
  "material": "basswood",
  "thickness": 3,
  "role": "cut",
  "machine_class": "diode_20",
  "laser_watts": 22,
  "machine": "EXAMPLE machine 22 W",
  "speed": 500,
  "power": 100,
  "passes": 2,
  "air": true,
  "notes": "EXAMPLE, not a real setting.",
  "source": "https://github.com/jahziel1/tlaku/issues/999",
  "confidence": "forum",
  "license": "CC0-1.0"
}
```

## File format

One JSON object per file, named `<issue number>.json`. It uses the same compact format as
[`crates/tlaku-doc/data/materials-default.json`](../../crates/tlaku-doc/data/materials-default.json),
plus provenance fields:

| Field | Meaning |
|---|---|
| `id` | `community.<material>.<thickness>.<operation>.<class>.<issue>`; unique |
| `material`, `material_name` | Material key (`basswood`, `mdf`...). If the key is not in the factory library, `material_name` holds the name |
| `thickness` | mm; omitted for surface engraving |
| `role` | `cut`, `engrave` or `score` |
| `machine_class`, `laser_watts` | Laser type and power class (`diode_20`...), and the real optical watts; they must agree |
| `machine` | Machine the values were tested on |
| `speed` | **mm/min** |
| `power` | 1 to 100 % |
| `passes`, `interval`, `air`, `frequency_khz` | Optional extras (`interval` in mm) |
| `notes` | Result and focus, free text (up to 400 characters) |
| `source` | URL of the issue: `https://github.com/jahziel1/tlaku/issues/<number>` |
| `confidence` | Always `forum` (community, unverified) |
| `license` | Always `CC0-1.0` |
| `contributor` | GitHub user name (optional) |
| `date` | Day the issue was opened, `YYYY-MM-DD` (optional; the script fills it) |
| `tlaku_version` | Tlaku version the values were tested with, `0.1.0` (optional; the form fills it) |
| `machine_model` | `id` of the model in the app's machine catalog (optional; set by the reviewer, declared) |
| `confirmations` | People who confirmed it, see above (optional; added by the reviewer) |

The validator (`cargo test -p tlaku-doc community`) checks fields, units, sensible ranges, unique ids, the
issue URL, the license and the confirmations. A file that fails it cannot be merged because CI runs the same test.

## License

The values in this folder are released by their authors under
[CC0 1.0](https://creativecommons.org/publicdomain/zero/1.0/) (public domain), so the library can be
redistributed with Tlaku. The code of the project remains MIT.

## Removing a contribution

Open an issue or ask in the original one. Deleting the file removes it from the next release (copies
that were already distributed cannot be recalled, which is why CC0 is required).
