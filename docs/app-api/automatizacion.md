# Contrato app-motor: Automatización por UDP

Parte de [el contrato app-motor](README.md). Servidor UDP local con comandos de texto.

### Automatización por UDP (M54)

Un servidor local opcional para que otros programas o scripts manejen la app con comandos de texto.

- **Apagado por omisión.** Se activa en Preferencias ▸ Automatización (`automationEnabled`), con el puerto
  `automationPort` (1024 a 65535, 47801 por omisión) y la casilla `automationAllowStart`.
- **Solo escucha en `127.0.0.1`.** Ningún otro equipo de la red puede enviarle nada.
- Comando `automation_configure` · `{ config: { enabled, port, allow_start } }` → `{ running, port, error }`
  (la interfaz lo llama al arrancar y cada vez que cambian esas preferencias). Un puerto ocupado o menor
  de 1024 no es un error del comando: vuelve `running: false` con el motivo en `error`.
- Eventos hacia la ventana: `automation://load` `{ path }` y `automation://start` `{ confirmed }`. La interfaz
  es dueña del documento y de las opciones del trabajo, así que `LOAD` y `START` se piden a ella.

**Protocolo.** Un comando por datagrama UDP, texto UTF-8 en una sola línea (máximo 4096 bytes; un `\n` o
`\r\n` final se ignora). La palabra no distingue mayúsculas. La respuesta es un datagrama de una línea
terminada en `\n`, enviado al origen.

| Comando | Respuesta | Qué hace |
|---|---|---|
| `PING` | `PONG` | Comprueba que la app escucha |
| `LOAD <ruta>` | `OK loading` | Pide abrir un proyecto `.tlaku` o importar el diseño, como si se soltara sobre la ventana. La ruta debe ser **absoluta** y de un archivo que existe (el resto de la línea es la ruta, con espacios). El resultado de la importación se avisa en la interfaz |
| `START` | `OK starting` o `PENDING confirm_in_app: ...` | Ver seguridad |
| `STATUS` | `STATUS connected=0` o `STATUS connected=1 state=<idle\|run\|hold\|alarm...> x=<mm> y=<mm>` | Estado de la máquina y posición de trabajo |
| `STOP` | `OK` | Detiene el trabajo (feed hold y soft reset; sin trabajo, cancela el jog). Siempre se atiende si hay máquina conectada |

Errores: `ERR <código>: <texto en español>`. Códigos: `not_connected`, `busy`, `not_found`, `bad_argument`,
`bad_text`, `too_long`, `empty`, `unknown_command`, `failed`.

**Seguridad de `START`:**

1. Exige máquina conectada y en reposo (`idle`); si no, `ERR not_connected` o `ERR busy`, y no pasa nada.
2. Por omisión **no inicia**: abre el diálogo de inicio de la interfaz y responde `PENDING confirm_in_app`;
   el usuario revisa y confirma allí. Se puede consultar el avance con `STATUS`.
3. Solo con la preferencia explícita «permitir iniciar desde automatización» (`automationAllowStart`,
   apagada por omisión) responde `OK starting` y la interfaz inicia sin preguntar, y aun así, si la lista de
   comprobación del diálogo de inicio es obligatoria (`jobChecklistRequired`), se pregunta en el diálogo.
   La interfaz vuelve a comprobar que haya algo que enviar y que la máquina esté lista.

Ejemplo desde una terminal: `echo PING | nc -u -w1 127.0.0.1 47801`.
