# Community material settings

[Español](README.es.md) · English

Material settings shared by people who tested them on their own machines. Each file in this folder is
**one contribution** (one GitHub issue) and ends up in the app's material library, shown as
**Community (unverified)**.

> These are one person's values on one machine. Lasers, lenses, focus, air and materials differ.
> **Always run a test on scrap first**, starting with less power than the listed value.

## How a setting gets into the library

1. **Issue.** Someone opens the [Share material settings](https://github.com/jahziel1/tlaku/issues/new?template=material_settings.yml)
   form (or uses *Share…* on one of their own presets in the app's Materials panel, which opens the form
   already filled in). They confirm they tested the values and agree to publish them under CC0.
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
4. **Library.** Once merged, the file is compiled into the app (`crates/tlaku-doc/build.rs`) and appears in
   the Materials panel with the *Community (unverified)* tag, next to the factory settings.

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

The validator (`cargo test -p tlaku-doc community`) checks fields, units, sensible ranges, unique ids, the
issue URL and the license. A file that fails it cannot be merged because CI runs the same test.

## License

The values in this folder are released by their authors under
[CC0 1.0](https://creativecommons.org/publicdomain/zero/1.0/) (public domain), so the library can be
redistributed with Tlaku. The code of the project remains MIT.

## Removing a contribution

Open an issue or ask in the original one. Deleting the file removes it from the next release (copies
that were already distributed cannot be recalled, which is why CC0 is required).
