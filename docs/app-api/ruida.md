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
- `ruida_info` → `RuidaInfo` y `ruida_files` → `RuidaFile[]`: solo lectura (M26 y M24).

**Despacho.** `AppState` guarda una sola conexión, `machine::active::ActiveMachine` (`Grbl`, `Ruida` o `Lhy`), y los
comandos pasan por `machine::active::Machine` (`is_closed`, `work_pos`, `jog`, `jog_to`, pausa, reanudar,
cancelar y `realtime`); lo exclusivo de GRBL usa `Machine::grbl()` (`AppState::handle`), lo de Ruida
`Machine::ruida()` y lo del K40 `Machine::lhy()`. Agregar un respaldo nuevo es una variante en cada enum: el compilador marca los
`match` que faltan. `start_job`, `frame_job` y `frame_hull` despachan por respaldo antes de la
ruta GRBL, porque cada respaldo calcula cosas distintas.
