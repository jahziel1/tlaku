# Cómo contribuir

Gracias por querer ayudar. Estas reglas protegen al proyecto (licencia MIT) y mantienen el código
fácil de revisar.

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

- Los comandos anteriores deben pasar (la CI los corre en Linux, macOS y Windows).
- Toda función nueva trae pruebas; los cambios de salida (G-code, rellenos, rasterizado) se
  cubren con archivos dorados.
- Si añades una dependencia, indica su licencia y ejecuta `cargo deny check licenses sources`.
- Explica en la descripción qué cambia y por qué, y la fuente pública si se trata de comportamiento
  de otro programa.

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
