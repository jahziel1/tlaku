<!-- English first, español abajo. Borra lo que no aplique / Delete what does not apply. -->

## What does this change and why? / ¿Qué cambia y por qué?

<!-- EN: Short description. If it reproduces the behavior of another program, link the public source. -->
<!-- ES: Descripción corta. Si reproduce el comportamiento de otro programa, enlaza la fuente pública. -->

Closes / Cierra #

## Checklist / Lista de verificación

**Tests / Pruebas**
- [ ] `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace` pass. / Pasan los tres comandos anteriores.
- [ ] UI changes: `npx tsc --noEmit && npx eslint . && npx vitest run` pass (in `apps/desktop`). / Cambios de interfaz: pasan `tsc`, `eslint` y `vitest`.
- [ ] New features come with tests; output changes (G-code, fills, raster) are covered by golden files and I explain why they changed. / Las funciones nuevas traen pruebas; los cambios de salida se cubren con archivos dorados y explico por qué cambiaron.
- [ ] Not tested on a real machine, or tested on: ______ (say which, honestly). / No probado con una máquina real, o probado en: ______ (dilo con honestidad).

**Clean room / Sala limpia**
- [ ] I did not copy code, text, icons, images or screenshots from LightBurn or other commercial software, and I did not decompile or analyze its files or traffic. / No copié código, textos, íconos, imágenes ni capturas de LightBurn ni de otro programa comercial, ni lo descompilé ni analicé sus archivos o su tráfico.

**Licenses / Licencias**
- [ ] New dependencies have a permissive license and `cargo deny check licenses sources` passes. / Las dependencias nuevas tienen licencia permisiva y `cargo deny` pasa.
- [ ] Third-party code I adapted is MIT/Apache/BSD/ISC/Zlib, keeps its notice and is listed in `THIRD_PARTY.md`. No GPL/LGPL/AGPL code was copied or translated line by line. / El código de terceros que adapté es compatible, conserva su aviso y está en `THIRD_PARTY.md`. No copié ni traduje línea por línea código GPL/LGPL/AGPL.

**Languages / Idiomas**
- [ ] Every UI text exists in `es` and `en` (not word-for-word copies of each other). / Todo texto de la interfaz existe en `es` y `en`.
- [ ] Documentation I changed is updated in both languages (`X.md` and `X.es.md`), or I opened an issue for the missing translation. / La documentación que cambié está en ambos idiomas, o abrí un issue para la traducción faltante.

**Safety / Seguridad**
- [ ] This does not change what is sent to the machine, or I explain how the safety checks (preflight, confirmations, stop) are kept. / No cambia lo que se envía a la máquina, o explico cómo se mantienen las revisiones de seguridad.

**DCO**
- [ ] Every commit is signed off (`git commit -s`) under the [Developer Certificate of Origin 1.1](https://developercertificate.org/). / Cada commit lleva `Signed-off-by` (`git commit -s`).
