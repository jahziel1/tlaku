# How to contribute

English · [Español](CONTRIBUTING.es.md)

Thank you for wanting to help. These rules protect the project (MIT license) and keep the code easy
to review. By taking part you accept the [Code of Conduct](CODE_OF_CONDUCT.md). You do not need to
code to help: testing on a real machine, sharing material settings and translating are very valuable
contributions (see below).

## Clean room

Expected behavior is documented from **public sources** (manuals, forums, tutorials, standards,
protocol documentation). Therefore:

- Do not copy code, text, icons, images or screenshots from LightBurn or any other commercial
  program.
- Do not decompile or disassemble it, nor analyze its files or its traffic to reproduce its
  implementation.
- If a feature is based on a public source, cite it in the specification (`docs/spec/`) or in the code
  comment. Anything not verified is marked "(no verificado)" or "(inferido)" (unverified / inferred).
- Write your own implementation from that description.

## Licenses

- **Dependencies.** Only permissive licenses are accepted: MIT, Apache-2.0, BSD-2-Clause,
  BSD-3-Clause, ISC, Zlib, Unicode-3.0, 0BSD, Unlicense and MPL-2.0 (per-file copyleft, compatible
  with distributing an MIT binary). With several alternative licenses it is enough that one is
  permissive. GPL, LGPL and AGPL as the only option are forbidden. `cargo deny check licenses sources`
  checks it (configuration in [deny.toml](deny.toml)) and runs in CI.
- **Third-party code.** You may read open source code of any license to understand a protocol or take
  ideas: protocol data and ideas are not copyrightable.
  - **Copying or adapting** code is only allowed if its license is compatible with MIT (MIT,
    Apache-2.0, BSD, ISC, Zlib). Keep its copyright notice, cite the origin in a comment and add it to
    [THIRD_PARTY.md](THIRD_PARTY.md).
  - **GPL/LGPL/AGPL code** (GRBL, LaserGRBL, Marlin, etc.): reading is fine, copying or translating
    line by line is not. Write your own implementation; if you relied on it to understand something,
    cite it as a reference.
  - This does not apply to LightBurn or other commercial programs: see "Clean room".
- What you contribute is published under the MIT license. The exception is community material
  settings, which are CC0 (see "Contributing machine settings").

## Style

- Comments and documentation in the code base are written in **Spanish**, concise and direct. New
  user-facing documentation should exist in English and Spanish (see "Translating").
- Identifiers (functions, types, variables, files) in **English**.
- Units in millimeters, origin at the bottom left, Y pointing up (see
  [docs/arquitectura.md](docs/arquitectura.md), in Spanish).
- Format with `rustfmt` and no `clippy` warnings.
- The engine does not depend on the interface: everything must be runnable and testable from the
  command line.

## Pull request requirements

Before opening one, from the repository root:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

- The commands above must pass (CI runs them on Linux, macOS and Windows). If you touch the
  interface, also `npx tsc --noEmit && npx eslint . && npx vitest run` inside `apps/desktop`.
- Every new feature comes with tests; output changes (G-code, fills, rasterization) are covered with
  golden files.
- If you add a dependency, state its license and run `cargo deny check licenses sources`.
- Explain in the description what changes and why, and the public source if it is about the behavior
  of another program. The PR template carries the checklist (tests, clean room, licenses, es/en
  languages and DCO).
- Every interface text goes in Spanish **and** English (see "Translating").
- If the change alters what is sent to the machine, explain how the safety checks (preflight,
  confirmations, stop) are kept.

## Golden files

Some tests compare their output with approved files in `crates/*/tests/fixtures` (`tlaku-grbl`,
`tlaku-fill`, `tlaku-raster`). If an output change is intentional:

```sh
UPDATE_GOLDEN=1 cargo test --workspace
git diff crates/*/tests/fixtures      # review every difference
cargo test --workspace                # must pass without UPDATE_GOLDEN
```

Include the modified golden files in the PR and explain why they changed. Never regenerate them just
to "make a test pass" without understanding the difference.

## Developer Certificate of Origin (DCO)

There is no CLA. Every commit must carry a `Signed-off-by` line stating that you accept the
[Developer Certificate of Origin 1.1](https://developercertificate.org/): that you wrote the change or
have the right to submit it under the project's MIT license. Add it with:

```sh
git commit -s -m "Commit message"
```

This adds `Signed-off-by: Your Name <you@example.com>` using the name and email from your git
configuration. To sign commits already made: `git rebase --signoff main` or `git commit --amend -s`.

## Contributing machine settings

There are two ways to help without writing code, and both use GitHub forms:

- **Test Tlaku on your real machine.** Everything has only been tested with emulators. Follow the
  [validation list](docs/validacion-hardware.md) (in Spanish) in order, from safest to riskiest, and
  report the result with the
  [Real machine test report](https://github.com/jahziel1/tlaku/issues/new?template=machine_report.yml)
  form. Good results are useful too. Use eye protection, scrap material and do not leave the machine
  alone.
- **Share material settings.** With the
  [Share material settings](https://github.com/jahziel1/tlaku/issues/new?template=material_settings.yml)
  form (or "Share…" in the app's Materials panel). Only share values you tested yourself, and confirm
  you publish them under the **CC0** license. After review they join the library as "Community
  (unverified)". The process and the format are in [community/materials](community/materials/README.md).
  Pasting tables copied from copyrighted manuals or sites is not accepted: only your own tests.

If you are the maintainer and want to turn an issue into a contribution:
`node scripts/community/issue-to-preset.mjs <number>` and then `cargo test -p tlaku-doc community`.

## Translating

The interface is in **Spanish and English**, and both are required in every change:

- Interface texts (TypeScript): `apps/desktop/src/i18n/es/<area>.ts` and `.../en/<area>.ts`. Every key
  exists in both and the texts are **not word-for-word copies**: write each language the way it would
  naturally be said.
- Engine messages (Rust): `crates/tlaku-i18n/locales/{es,en}/<crate>.txt` (see that crate's README).
- Documentation: each main file exists in English (`X.md`) and Spanish (`X.es.md`), and each links to
  the other at the top. The manual lives in `docs/manual/` (Spanish) and `docs/manual/en/` (English),
  with the same file names. If you change one, update the other or open an issue with the
  `translation` label.
- Want to add a new language? Open a [discussion](https://github.com/jahziel1/tlaku/discussions) first
  to coordinate.
- Write by hand or with a tool, but **review** the result: unreviewed machine translation is not
  accepted, especially for safety texts.
