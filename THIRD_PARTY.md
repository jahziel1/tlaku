# Código de terceros

Código copiado o adaptado de otros proyectos con licencia compatible con MIT. Cada entrada indica
el proyecto, la licencia, el aviso de copyright y dónde se usa en Tlaku. Las dependencias de
`Cargo.toml` y `package.json` no se listan aquí: sus licencias las verifica `cargo deny`.

| Proyecto | Licencia | Copyright | Usado en |
|---|---|---|---|
| [MeerK40t](https://github.com/meerk40t/meerk40t) (`meerk40t/ruida/`, `test/test_ruida.py`) | MIT | Copyright (c) 2021 meerk40t | `crates/tlaku-ruida`: el código se leyó y se reescribió desde cero (no hay código copiado). Lo que sí se reproduce son **datos del protocolo**: los ocho pares magic / `DA 00 05 7E` swizzleado de `test_magic_keys` (vectores en `src/swizzle.rs`, pruebas), las constantes de comandos y la fórmula de la suma `E5 05` (`src/rd.rs`, `FileChecksum::SumAll`) |
| [ruida-pa](https://github.com/StevenIsaacs/ruida-pa) (`protocols/ruida/ruida_protocol.py`, `README.md`, `docs/designs/EffectiveMinPower.md`) | MIT | Copyright (c) 2025 Steve Isaacs | `crates/tlaku-ruida`: código reescrito; se reproducen **datos del protocolo**: nombres y valores de comandos y direcciones de memoria, bits de estado, IDs de tarjeta (`src/session.rs`, `src/cmd.rs`, `src/emu.rs`) y dos vectores de capturas de LightBurn del README: el datagrama `0261 d4890df7` (`src/packet.rs`, pruebas) y el mensaje `E5 05 00 04 53 78 79` (`src/cmd.rs`, pruebas) |
| [lbrn2-to-svg](https://github.com/jlucaso1/lbrn2-to-svg) | MIT | Copyright (c) 2025 João Lucas de Oliveira Lopes | `crates/tlaku-lbrn` e `import_lbrn`: código reescrito desde cero; solo se reproducen **datos del formato** (nombres de elementos y atributos, caché `VertID`/`PrimID`, `LineClosed`, `BackupPath` del texto) |
| [lbrnts](https://github.com/tscircuit/lbrnts) | MIT | Copyright (c) 2025 tscircuit Inc | `crates/tlaku-lbrn` e `import_lbrn`: solo **datos del formato** (campos de `CutSetting`, `FormatVersion` 0/1). Ningún archivo suyo está en el repositorio; la tabla de colores de capa es aproximada y propia, y los fixtures de `crates/tlaku-pipeline/tests/fixtures/lbrn/` son originales de Tlaku |
| [lightburn-tester](https://github.com/MarcinZukowski/lightburn-tester) y [ducktape](https://github.com/agentydragon/ducktape) | (no se copia nada) | — | Solo **nombres de elementos** de `CutSetting` (`perfLen`, `perfSkip`, `dotTime`, `dotSpacing`, `rampOuter`, `throughPower`, `enableCutThroughStart/End`, `startDelay`, `endDelay`, `floodFill`) para `import_lbrn/layers.rs`. Datos del formato; no se copió código |
| [Hershey Fonts](http://emergent.unpythonic.net/software/hershey) (archivos `*.jhf` tomados de [kamalmostafa/hershey-fonts](https://github.com/kamalmostafa/hershey-fonts), carpeta `hershey-fonts/`; solo los datos, nada del código GPL-2.0 de ese repositorio) | Aviso de uso propio: libre para cualquier fin, también comercial, si se distribuyen los reconocimientos y no se usa el formato de NTIS | Copyright 1967 Dr. A. V. Hershey, James Hurt | `crates/tlaku-text/data/hershey/` (15 fuentes de un trazo: Roman Simplex/Duplex/Triplex, Futura, Times, Script, Cursive y Gothic) y `src/linefont.rs`. Reconocimientos obligatorios: *The Hershey Fonts were originally created by Dr. A. V. Hershey while working at the U. S. National Bureau of Standards. The format of the Font data in this distribution was originally created by James Hurt, Cognition, Inc., 900 Technology Park Drive, Billerica, MA 01821.* El aviso completo está en `crates/tlaku-text/data/hershey/NOTICE.txt` y la interfaz lo cita junto al selector de fuentes. Los datos se convierten a trayectorias propias de Tlaku, que es una conversión a otro formato distinto del de NTIS, como permite la condición 2 |
| [ezdxf](https://github.com/mozman/ezdxf) (`src/ezdxf/fonts/shapefile.py`) | MIT | Copyright (c) 2022-2026 Manfred Moitzi | `crates/tlaku-text/src/shx.rs`: se leyó para entender la disposición binaria de los `.shx` de AutoCAD (`shapes` y `unifont`) y el cálculo de arcos fraccionarios; el lector de Tlaku está escrito desde cero (no hay código copiado). Los significados de los códigos de forma salen de la documentación pública de Autodesk (*Shape Description Codes*) |

## Solo referencia: no se copió nada

Proyectos que se leyeron para contrastar hechos del protocolo pero cuya licencia no permite copiar
su código. No hay código, archivos ni capturas suyos en el repositorio; los datos que Tlaku usa
(por ejemplo la escala de potencia `% * 16383 / 100`) son hechos del protocolo comprobados con sus
ejemplos y descritos con palabras propias en [docs/hardware/ruida.md](docs/hardware/ruida.md).

| Proyecto | Licencia | Qué se miró |
|---|---|---|
| [jnweiger/ruida-laser](https://github.com/jnweiger/ruida-laser) | GPL-2.0 | `Nova35_rect.pcapng` y `novalokal.pcapng` (checksum y ACK de capturas reales), tres `.rd` de RDWorks (orden de los comandos, nombre de elemento) y `laser_percentages/codes.txt` (escala de potencia) |
| [lyonlabs/ruida](https://crates.io/crates/ruida) | MIT | Plantilla PyO3 sin protocolo: no aporta nada |
| [SVGnest](https://github.com/Jack000/SVGnest) | MIT | Idea de acomodar con polígonos de no-ajuste (NFP) y buscar sobre el orden y las rotaciones. No se copió código: `crates/tlaku-nest` es una implementación propia (suma de Minkowski por partes convexas, regiones libres incrementales y recocido simulado) |
| [Deepnest](https://github.com/deepnest-next/deepnest) | Sin licencia clara en la API de GitHub (NOASSERTION; tratar como GPL) | Solo se leyó la descripción general del enfoque; no se copió ni tradujo código |
