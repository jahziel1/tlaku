# Contrato app-motor: Ruida por UDP (experimental)

Parte de [el contrato app-motor](README.md). Controladoras Ruida por UDP.

### Ruida por UDP (EXPERIMENTAL)

Una controladora Ruida (CO2) se conecta con un perfil `kind = "ruida"`. Solo está probada contra el
emulador UDP de `tlaku-ruida`, no con una máquina real (ver `docs/hardware/ruida.md`, sección 15).

- `connect` recibe además `device` (el perfil activo). Con `kind = "ruida"` el motor ignora `baud` y usa
  `device.connection.host` (UDP, puertos 50200 y 40200), `device.ruida.magic` (`null` = detectarlo con el
  sondeo de solo lectura) y la cama y el origen del perfil. El `port` solo cuenta para `virtual:ruida[@<velocidad>]`,
  una máquina virtual que arranca el emulador UDP en `127.0.0.1` y se conecta a él por UDP real (`list_ports` la
  ofrece al final). `ConnectResult.firmware` es `Ruida RDC6442S · tarjeta 0x65106510`, y el evento de conexión lleva
  `port = "udp://<IP>"`. Un perfil Ruida sin IP solo conecta a la máquina virtual.
- Los eventos son los mismos `machine://*`. El estado llega cada ~500 ms (nunca durante una subida):
  `work_pos` y `machine_pos` son los mm desde el home hacia dentro de la cama, o sea las coordenadas del
  origen del perfil (`origin` = esquina del home); `feed`, `spindle` y `buffer` van en 0 y no hay `overrides`.
  Con un trabajo en marcha, un sondeo sin respuesta no es una conexión perdida (algunas controladoras ignoran
  la red mientras ejecutan); fuera de uno, tres seguidos sí.
- `start_job` genera el `.rd` (`tlaku_pipeline::export_rd_for_device`) y lo sube con progreso (`machine://progress`
  cuenta paquetes). Con `ruida.run_after_upload = false` (predeterminado) queda como documento `TLAKUnnn` en la
  memoria y no arranca: `machine://job` termina en `finished` con el aviso; con `true` se ejecuta y el hilo sigue el
  estado hasta que la máquina vuelve a reposo (`finished`). Si la subida falla antes del primer paquete, el comando
  devuelve el error y no hay eventos; si falla después, `machine://job` `error` (y `D8 01` si se ejecutaba). Se rechaza
  si la cama que informa la máquina no coincide con la del perfil.
- `pause_job` / `resume_job` / `cancel_job` son `D8 02` / `D8 03` / `D8 01` (cancelar también interrumpe una subida).
  `machine_realtime` traduce `hold`, `resume`, `reset` y `jog_cancel` a esos mismos; `status` no hace nada.
  `jog` y `jog_to` usan saltos `D9` con el láser apagado; `frame_job` y `frame_hull` recorren el rectángulo del trabajo
  con cinco saltos y no bloquean (ignoran `power_pct`: no hay encuadre con el láser encendido).
- Lo que es de GRBL (`machine_command`, `set_override`, ajustes `$`, macros, `jog_axis`, `fire_test`,
  `simulate_fault`) devuelve el error `Esta función es solo de GRBL...`.
- `ruida_info` → `RuidaInfo` y `ruida_files` → `RuidaFile[]`: solo lectura (M26 y M24). `RuidaInfo` incluye (ola 6E) `board_version`, `previous_work`, `control_mode` y las funciones `focus`, `z_return_docking`, `air_assist_mode`.

**Ajustes, documentos y respaldos (ola 6E; EXPERIMENTAL: sin probar con una controladora real).** Todo lo que cambia la máquina pasa por las protecciones de `tlaku_ruida::session` (ver `hardware/ruida.md` sección 16): lista cerrada de direcciones con rango, respaldo previo con fecha en `<configuración>/ruida-backups/`, `confirmed = true` explícito, máquina en reposo y verificación por relectura. Se rechazan con un trabajo o un encuadre en marcha. **Los comandos que escriben** (`ruida_settings_apply`, `ruida_settings_restore`, `ruida_calibrate`, `ruida_file_delete`, `ruida_file_run` y `ruida_prepare_store` con `overwrite = true`) llevan además `experimental_writes: bool`, el valor de la preferencia `ruidaExperimentalWrites` (apagada de fábrica); con `false` el motor las rechaza sin enviar nada.

| Comando | Argumentos | Resultado |
|---|---|---|
| `ruida_settings_read` | — | `RuidaControllerSettings` (modelo, tarjeta y los 24 ajustes con clave, dirección, grupo, unidad, valor, rango y nombre en las fuentes) |
| `ruida_settings_preview` | `changes: RuidaSettingChange[]` (`key`, `value`) | `RuidaSettingsPlan`: solo lo que cambia, con valor actual y nuevo; no escribe |
| `ruida_settings_preview_file` | `json` (texto de un `.tlakuruida`) | `RuidaSettingsPlan`; avisa si es de otra tarjeta |
| `ruida_settings_apply` | `changes`, `confirmed` | `RuidaApplyReport` (cambios, ruta del respaldo, `verified[]`, `write_error`, `ok`, `restore_advised`) |
| `ruida_settings_restore` | `json`, `confirmed` | `RuidaApplyReport`; rechaza un respaldo de otra tarjeta |
| `ruida_settings_export` | — | texto `.tlakuruida` (la interfaz lo guarda con `write_settings_backup`, que acepta esa extensión) |
| `ruida_calibrate` | `key` (`x_step`...), `commanded_mm`, `measured_mm` | `RuidaCalibration` (paso actual y propuesto); no escribe |
| `ruida_backups` / `ruida_backup_text` | — / `name` | `RuidaBackup[]` (más reciente primero) / texto de un respaldo |
| `ruida_prepare_store` | `name` (`null` = automático), `overwrite` | fija el nombre del próximo trabajo guardado y si puede reemplazar a uno igual; vale para un solo envío y caduca a los 120 s; la interfaz lo llama al confirmar el inicio |
| `ruida_file_delete` / `ruida_file_run` | `index`, `name`, `confirmed` | `RuidaDocResult`; el motor relee el nombre del documento y no actúa si no coincide; nunca borra «todos» |

`start_job` con un nombre repetido y sin permiso de reemplazar falla sin tocar nada; con el nombre automático elige un `TLAKUnnn` libre. GRBL: `grbl_settings_autobackup(text)` guarda el respaldo automático previo de los ajustes `$` en `<configuración>/grbl-backups/` y devuelve la ruta.

**Despacho.** `AppState` guarda una sola conexión, `machine::active::ActiveMachine` (`Grbl`, `Ruida` o `Lhy`), y los
comandos pasan por `machine::active::Machine` (`is_closed`, `work_pos`, `jog`, `jog_to`, pausa, reanudar,
cancelar y `realtime`); lo exclusivo de GRBL usa `Machine::grbl()` (`AppState::handle`), lo de Ruida
`Machine::ruida()` y lo del K40 `Machine::lhy()`. Agregar un respaldo nuevo es una variante en cada enum: el compilador marca los
`match` que faltan. `start_job`, `frame_job` y `frame_hull` despachan por respaldo antes de la
ruta GRBL, porque cada respaldo calcula cosas distintas.
