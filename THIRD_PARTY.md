# Código de terceros

Código copiado o adaptado de otros proyectos con licencia compatible con MIT. Cada entrada indica
el proyecto, la licencia, el aviso de copyright y dónde se usa en Tlaku. Las dependencias de
`Cargo.toml` y `package.json` no se listan aquí: sus licencias las verifica `cargo deny`.

| Proyecto | Licencia | Copyright | Usado en |
|---|---|---|---|
| [MeerK40t](https://github.com/meerk40t/meerk40t) (`meerk40t/ruida/`, `test/test_ruida.py`) | MIT | Copyright (c) 2021 meerk40t | `crates/tlaku-ruida`: el código se leyó y se reescribió desde cero (no hay código copiado). Lo que sí se reproduce son **datos del protocolo**: los ocho pares magic / `DA 00 05 7E` swizzleado de `test_magic_keys` (vectores en `src/swizzle.rs`, pruebas), las constantes de comandos y la fórmula de la suma `E5 05` (`src/rd.rs`, `FileChecksum::SumAll`) |
| [ruida-pa](https://github.com/StevenIsaacs/ruida-pa) (`protocols/ruida/ruida_protocol.py`, `README.md`, `docs/designs/EffectiveMinPower.md`) | MIT | Copyright (c) 2025 Steve Isaacs | `crates/tlaku-ruida`: código reescrito; se reproducen **datos del protocolo**: nombres y valores de comandos y direcciones de memoria, bits de estado, IDs de tarjeta (`src/session.rs`, `src/cmd.rs`, `src/emu.rs`) y dos vectores de capturas de LightBurn del README: el datagrama `0261 d4890df7` (`src/packet.rs`, pruebas) y el mensaje `E5 05 00 04 53 78 79` (`src/cmd.rs`, pruebas) |

## Solo referencia: no se copió nada

Proyectos que se leyeron para contrastar hechos del protocolo pero cuya licencia no permite copiar
su código. No hay código, archivos ni capturas suyos en el repositorio; los datos que Tlaku usa
(por ejemplo la escala de potencia `% * 16383 / 100`) son hechos del protocolo comprobados con sus
ejemplos y descritos con palabras propias en [docs/hardware/ruida.md](docs/hardware/ruida.md).

| Proyecto | Licencia | Qué se miró |
|---|---|---|
| [jnweiger/ruida-laser](https://github.com/jnweiger/ruida-laser) | GPL-2.0 | `Nova35_rect.pcapng` y `novalokal.pcapng` (checksum y ACK de capturas reales), tres `.rd` de RDWorks (orden de los comandos, nombre de elemento) y `laser_percentages/codes.txt` (escala de potencia) |
| [lyonlabs/ruida](https://crates.io/crates/ruida) | MIT | Plantilla PyO3 sin protocolo: no aporta nada |
