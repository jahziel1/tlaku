# Contrato app-motor: Máquinas: valores por tipo, sondeo y máquinas virtuales

Parte de [el contrato app-motor](README.md). Comandos de máquinas de la fase 4, oleada C (`device_defaults`, `probe_port`, `simulate_fault`).

### Máquinas (fase 4, oleada C)

| Comando | Argumentos | Resultado |
|---|---|---|
| `device_defaults` | `kind` | `DeviceProfile` con el dialecto, S máxima, modo de potencia y baudios del tipo |
| `rotary_test` | `device`, `speed`, `power` | `Document` con una línea de exactamente una vuelta |
| `probe_port` | `port`, `baud` | `ProbeResult` (familia, firmware, banner, `$I`/`M115`, `$$`). Solo escribe `?`, `0x18`, `$I`, `$$` y `M115`; nunca mueve la máquina ni cambia ajustes |
| `simulate_fault` | `fault` | Inyecta una falla (llama, inclinación, alarma 3, desconexión). Solo con máquina virtual |

`list_ports` agrega al final los puertos virtuales `virtual:falcon2`, `virtual:grbl11`,
`virtual:grblhal`, `virtual:fluidnc`, `virtual:marlin` y `virtual:smoothie` (emulador
`tlaku-emu`). La velocidad del tiempo va en el id: `virtual:falcon2@5`.

El envío depende de la familia del perfil: GRBL con conteo de caracteres; Marlin y Smoothie línea a
línea, sin posición durante el trabajo, y cancelar siempre marca `JobEvent.position_lost`.
`MachineState` incluye `other` para estados que no conoce (por ejemplo `Tool` de grblHAL), con el
nombre original en `StatusReport.other_state`.
