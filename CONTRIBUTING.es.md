# Cómo contribuir

[English](CONTRIBUTING.md) · Español

Gracias por querer ayudar. Estas reglas protegen al proyecto (licencia MIT) y mantienen el código
fácil de revisar. Al participar aceptas el [Código de conducta](CODE_OF_CONDUCT.es.md). No hace falta
programar para ayudar: probar con una máquina real, compartir ajustes de material y traducir son
aportes muy valiosos (ver abajo).

## Sala limpia

El comportamiento esperado se documenta a partir de **fuentes públicas** (manuales, foros,
tutoriales, estándares, documentación de protocolos). Por eso:

- No copies código, textos, íconos, imágenes ni capturas de pantalla de LightBurn ni de otro
  programa comercial.
- No lo descompiles ni lo desensambles, ni analices sus archivos o su tráfico para reproducir
  su implementación.
- Si una función se basa en una fuente pública, cítala en la especificación (`docs/spec/`) o en el
  comentario del código. Lo no verificado se marca como "(no verificado)" o "(inferido)".
- Escribe tu propia implementación a partir de esa descripción.

## Licencias

- **Dependencias.** Solo se aceptan licencias permisivas: MIT, Apache-2.0, BSD-2-Clause,
  BSD-3-Clause, ISC, Zlib, Unicode-3.0, 0BSD, Unlicense y MPL-2.0 (copyleft por archivo, compatible
  con distribuir un binario MIT). Con varias licencias alternativas basta con que una sea permisiva.
  GPL, LGPL y AGPL como única opción están prohibidas. `cargo deny check licenses sources`
  lo comprueba (configuración en [deny.toml](deny.toml)) y corre en la CI.
- **Código de terceros.** Puedes leer código abierto de cualquier licencia para entender un
  protocolo o tomar ideas: los datos de un protocolo y las ideas no tienen derechos de autor.
  - **Copiar o adaptar** código solo se permite si su licencia es compatible con MIT (MIT,
    Apache-2.0, BSD, ISC, Zlib). Conserva su aviso de copyright, cita el origen en un comentario y
    agrégalo a [THIRD_PARTY.md](THIRD_PARTY.md).
  - **Código GPL/LGPL/AGPL** (GRBL, LaserGRBL, Marlin, etc.): leer sí, copiar o traducir línea por
    línea no. Escribe tu propia implementación; si te basaste en él para entender algo, cítalo como
    referencia.
  - Esto no aplica a LightBurn ni a otros programas comerciales: ver "Sala limpia".
- Lo que aportes se publica bajo licencia MIT.

## Estilo

- Comentarios y documentación en **español**, concisos y directos.
- Identificadores (funciones, tipos, variables, archivos) en **inglés**.
- Unidades en milímetros, origen abajo a la izquierda, Y hacia arriba
  (ver [docs/arquitectura.md](docs/arquitectura.md)).
- Formato con `rustfmt` y sin warnings de `clippy`.
- El motor no depende de la interfaz: todo se debe poder ejecutar y probar desde la línea de comandos.

## Requisitos de un PR

Antes de abrirlo, en la raíz del repositorio:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

- Los comandos anteriores deben pasar (la CI los corre en Linux, macOS y Windows). Si tocas la
  interfaz, también `npx tsc --noEmit && npx eslint . && npx vitest run` dentro de `apps/desktop`.
- Toda función nueva trae pruebas; los cambios de salida (G-code, rellenos, rasterizado) se
  cubren con archivos dorados.
- Si añades una dependencia, indica su licencia y ejecuta `cargo deny check licenses sources`.
- Explica en la descripción qué cambia y por qué, y la fuente pública si se trata de comportamiento
  de otro programa. La plantilla del PR trae la lista de verificación (pruebas, sala limpia, licencias,
  idiomas es/en y DCO).
- Todo texto de la interfaz va en español **y** en inglés (ver «Traducir»).
- Si el cambio altera lo que se envía a la máquina, explica cómo se conservan las revisiones de
  seguridad (preflight, confirmaciones, paro).

## Archivos dorados

Algunas pruebas comparan su salida con archivos aprobados en `crates/*/tests/fixtures`
(`tlaku-grbl`, `tlaku-fill`, `tlaku-raster`). Si un cambio de salida es intencional:

```sh
UPDATE_GOLDEN=1 cargo test --workspace
git diff crates/*/tests/fixtures      # revisa cada diferencia
cargo test --workspace                # debe pasar sin UPDATE_GOLDEN
```

Incluye los dorados modificados en el PR y explica por qué cambiaron. Nunca los regeneres para
"hacer pasar" una prueba sin entender la diferencia.

## Certificado de origen del desarrollador (DCO)

No hay CLA. Cada commit debe llevar una línea `Signed-off-by` que indica que aceptas el
[Developer Certificate of Origin 1.1](https://developercertificate.org/): que escribiste el cambio
o tienes derecho a enviarlo bajo la licencia MIT del proyecto. Se añade con:

```sh
git commit -s -m "Mensaje del commit"
```

Esto agrega `Signed-off-by: Tu Nombre <tu@correo>` con el nombre y correo de tu configuración de
git. Para firmar commits ya hechos: `git rebase --signoff main` o `git commit --amend -s`.

## Aportar ajustes de la máquina

Hay dos formas de ayudar sin escribir código, y las dos usan formularios de GitHub:

- **Probar Tlaku en tu máquina real.** Todo se ha probado solo con emuladores. Sigue la
  [lista de validación](docs/validacion-hardware.md) en orden, de lo más seguro a lo más arriesgado, y
  cuenta el resultado con el formulario
  [Reporte de prueba en una máquina real](https://github.com/jahziel1/tlaku/issues/new?template=machine_report.yml).
  Los resultados buenos también sirven. Usa protección ocular, material de desecho y no dejes la
  máquina sola.
- **Compartir ajustes de material.** Con el formulario
  [Compartir ajustes de material](https://github.com/jahziel1/tlaku/issues/new?template=material_settings.yml)
  (o «Compartir…» en el panel Materiales de la app). Solo comparte valores que probaste tú, y confirma
  que los publicas con licencia **CC0**. Tras la revisión entran a la biblioteca como «Comunidad (sin
  verificar)». El proceso y el formato están en [community/materials](community/materials/README.es.md).
  No se acepta pegar tablas copiadas de manuales o sitios con derechos de autor: solo tus propias
  pruebas.

Si eres quien mantiene el proyecto y quieres convertir un issue en un aporte:
`node scripts/community/issue-to-preset.mjs <número>` y después `cargo test -p tlaku-doc community`.

## Traducir

La interfaz está en **español e inglés**, y ambos son obligatorios en cada cambio:

- Textos de la interfaz (TypeScript): `apps/desktop/src/i18n/es/<área>.ts` y `.../en/<área>.ts`. Cada
  clave existe en los dos y los textos **no son copias palabra por palabra**: escribe cada idioma como
  se diría de forma natural.
- Mensajes del motor (Rust): `crates/tlaku-i18n/locales/{es,en}/<crate>.txt` (ver el README de ese
  crate).
- Documentación: cada archivo principal está en inglés (`X.md`) y en español (`X.es.md`), y cada uno
  enlaza al otro arriba. El manual vive en `docs/manual/` (español) y `docs/manual/en/` (inglés), con
  los mismos nombres de archivo. Si cambias uno, actualiza el otro o abre un issue con la etiqueta
  `translation`.
- ¿Quieres agregar un idioma nuevo? Abre primero una [discusión](https://github.com/jahziel1/tlaku/discussions)
  para coordinar.
- Escribe a mano o con una herramienta, pero **revisa** el resultado: una traducción automática sin
  revisar no se acepta, sobre todo en textos de seguridad.
