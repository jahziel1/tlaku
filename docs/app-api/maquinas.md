# Contrato app-motor: Máquinas: valores por tipo, sondeo y máquinas virtuales

Parte de [el contrato app-motor](README.md). Comandos de máquinas de la fase 4, oleada C (`device_defaults`, `probe_port`, `simulate_fault`) y la sonda de contacto del buscador de centro (`probe_touch`, ola 6E).

### Máquinas (fase 4, oleada C)

| Comando | Argumentos | Resultado |
|---|---|---|
| `device_defaults` | `kind` | `DeviceProfile` con el dialecto, S máxima, modo de potencia y baudios del tipo |
| `rotary_test` | `device`, `speed`, `power` | `Document` con una línea de exactamente una vuelta |
| `read_lbdev` | `path` | `LbdevImport { devices: DeviceProfile[], notices: LbdevNotice[] }`. Lee un perfil de LightBurn (`.lbdev`, hasta 2 MB) y propone perfiles nuevos (id y nombre que no chocan con los guardados). **No guarda nada ni toca la máquina**: la interfaz guarda con `save_device` lo que el usuario confirma. Los avisos llevan un `kind` (`not_imported`, `invalid_value`, `missing_value`, `unsupported_controller`, `skipped_entry`, `simulator_values`, `script_code`, `z_axis_check`, `origin_inferred`, `connection_type`, `ruida_host`), el dispositivo y un `detail`; el texto lo pone la interfaz. Error si el JSON es inválido o ningún dispositivo es importable |
| `probe_port` | `port`, `baud` | `ProbeResult` (familia, firmware, banner, `$I`/`M115`, `$$`). Solo escribe `?`, `0x18`, `$I`, `$$` y `M115`; nunca mueve la máquina ni cambia ajustes |
| `simulate_fault` | `fault` | Inyecta una falla (llama, inclinación, alarma 3, desconexión, corte de luz `power_cut`). Solo con máquina virtual |
| `interrupted_job` | (ninguno) | `InterruptedJob \| null`: el trabajo del que queda punto de control (`id`, `kind`, `percent` (por posición de la cabeza sobre el recorrido, no por líneas con `ok`), `done_lines`/`total_lines` (dato interno), `mpos`, `can_resume`, `note`, y si hay máquina conectada en reposo, `machine` (`not_checked`/`unknown`/`unmoved`/`moved`/`probably_finished`), `ahead_lines`/`ahead_mm`, `recommend_discard`, `warning`: la máquina pudo seguir sola con lo que tenía en su búfer). `null` si no hay o si un trabajo se está transmitiendo ahora. Ruida, K40 y galvo solo dejan un marcador (`can_resume: false`) |
| `interrupted_job_view` | `back_lines?`, `back_mm?` (snake_case) | `ResumeView`: recorrido del G-code guardado (`moves` como `JobPreview`, `move_lines`), `suggested` (línea, `at`, línea en ejecución, `mismatch`, `row_restart`, `z_delta`) y avisos. Con la máquina conectada en reposo se compara su MPos de ahora con la guardada: si siguió sola, el punto sale de la posición de ahora (y `warnings[0]` lo explica). El punto sale de comparar la última MPos con las líneas confirmadas y retroceder `back_lines` líneas o `back_mm` mm (lo que quede más atrás), sin cruzar un `G0` |
| `discard_interrupted_job` | (ninguno) | `void`. Borra el punto de control |
| `continue_check` | (ninguno) | `ResumeReadiness { homing, homed, state }`: `$22` (`null` si no contesta `$$`) y si ya se hizo `$H` desde que la placa perdió la posición |
| `continue_frame` | `speed` | `void`. Encuadra el rectángulo del G-code guardado con el láser apagado |
| `continue_job` | `args: ResumeStart` | `void`. Continúa desde `args.line`. Exige: homing hecho si `$22=1`, o `confirm_unmoved` si no hay homing; `framed_ok`; `checklist_ok`; el preflight de siempre. Sin eso falla con «Nada se envió». Envía preámbulo (`M5`, `G0` al punto, luego `M3`/`M4` con su `S` y `F`) + resto del trabajo |
| `probe_touch` | `dx`, `dy`, `feed`, `retract`, `confirmed` (snake_case) | `ProbeTouch { hit, machine_pos, work_pos }` (ola 6E, M48). Un toque de sonda con el cabezal: `M5`, `G91`, `G38.3 X{dx} Y{dy} F{feed}`, retroceso de `retract` mm si tocó y `G90`. `dx`/`dy` en mm de trabajo. Límites del motor: recorrido de 0,01 a 100 mm, velocidad de hasta 500 mm/min y retroceso de 0 a 20 mm. Rechaza `confirmed: false`, otra familia que GRBL, la máquina fuera de `Idle` y un trabajo en curso. `hit: false` (sin contacto en el recorrido) no es un error; si la sonda ya estaba activa al empezar, falla con `ALARM:4` explicado |

`list_ports` agrega al final los puertos virtuales `virtual:falcon2`, `virtual:grbl11`,
`virtual:grblhal`, `virtual:fluidnc`, `virtual:marlin` y `virtual:smoothie` (emulador
`tlaku-emu`). La velocidad del tiempo va en el id: `virtual:falcon2@5`.

El envío depende de la familia del perfil: GRBL con conteo de caracteres; Marlin y Smoothie línea a
línea, sin posición durante el trabajo, y cancelar siempre marca `JobEvent.position_lost`.
`MachineState` incluye `other` para estados que no conoce (por ejemplo `Tool` de grblHAL), con el
nombre original en `StatusReport.other_state`.
