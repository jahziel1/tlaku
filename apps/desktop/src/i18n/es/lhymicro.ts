/**
 * Textos del área `lhymicro` (K40 con placa Lihuiyu M2 Nano por USB, EXPERIMENTAL). Todas las
 * claves empiezan con `lhymicro.`. Los avisos del codificador llegan del motor como datos
 * (`kind`, capa, valor) y se escriben aquí, en el idioma de la interfaz.
 */
export const lhymicro = {
  // ---- común ---------------------------------------------------------------------------
  'lhymicro.experimental': 'Experimental',
  'lhymicro.experimental.note':
    'Soporte K40 experimental: se probó contra una placa simulada, no con una placa real. Prueba primero con el láser deshabilitado o muy débil y ten el paro de emergencia al alcance.',
  'lhymicro.virtual.name': 'K40 (M2 Nano)',
  'lhymicro.port.usb': 'K40 · placa USB {n}',
  'lhymicro.power.note':
    'La placa M2 Nano de serie no controla la potencia por software: se ajusta con la perilla del K40. Tlaku ignora el porcentaje de potencia de las capas.',

  // ---- perfil de la máquina ------------------------------------------------------------
  'lhymicro.dev.group': 'Placa del K40 (M2 Nano)',
  'lhymicro.dev.board': 'Modelo de placa',
  'lhymicro.dev.board.help':
    'Cambia las ecuaciones de velocidad. La M2 es la de serie en casi todos los K40; usa otra solo si la letra impresa en tu placa es otra (M, M1, M3, A, B, B1 o B2).',
  'lhymicro.dev.board.default': '{board} (la de serie)',
  'lhymicro.dev.origin': 'Origen y home',
  'lhymicro.dev.origin.help':
    'El home de un K40 de serie está arriba a la izquierda: elige «Arriba a la izquierda» en Origen. Si tu cabezal busca otra esquina, elige esa. Es una suposición hasta probar con hardware: haz primero un saltito con el láser apagado.',
  'lhymicro.dev.maxRate':
    'En un K40 la velocidad máxima de corte es el mayor de los dos topes de velocidad (8400 mm/min son 140 mm/s): una capa más rápida se rechaza al enviar.',
  'lhymicro.dev.connection':
    'Conexión por USB (CH341, 1A86:5512): la placa se elige en el panel Máquina. Cierra K40 Whisperer, MeerK40t y LaserDRW antes de conectar.',
  'lhymicro.dev.hidden':
    'Un perfil K40 no usa G-code, S máxima, baudios ni macros, ni los ejes Z y U: esos campos no aplican.',

  // ---- panel Máquina -------------------------------------------------------------------
  'lhymicro.panel.title': 'K40 con M2 Nano',
  'lhymicro.panel.needConnect': 'Conéctate a la placa para ver su estado.',
  'lhymicro.panel.noG':
    'Esta máquina no usa G-code: la consola, los ajustes en vivo, los ajustes «$» de GRBL, el eje Z y el aire no aplican.',
  'lhymicro.panel.board': 'Placa',
  'lhymicro.panel.status': 'Estado de la placa',
  'lhymicro.panel.position': 'Posición',
  'lhymicro.panel.position.known': 'Contada desde el último home',
  'lhymicro.panel.position.unknown': 'Desconocida: haz home',
  'lhymicro.panel.positionNote':
    'La placa no informa su posición real: Tlaku la cuenta desde el home. Si mueves el carro a mano o se pierde un paso, vuelve a hacer home.',
  'lhymicro.panel.unknownWarn':
    'No se sabe dónde está el cabezal: no se moverá nada hasta que hagas home. Enviar un trabajo hace home primero.',
  'lhymicro.panel.lowPower':
    'La placa informa energía insuficiente (239): revisa el paro de emergencia, la tapa y que la fuente del láser esté encendida.',
  'lhymicro.panel.moshiboard':
    'Las Moshiboard comparten el USB de la M2 Nano pero no son compatibles: si al conectar aparece ese aviso, tu máquina lleva una.',
  'lhymicro.status.206': 'Lista',
  'lhymicro.status.238': 'Ocupada',
  'lhymicro.status.236': 'Terminó',
  'lhymicro.status.207': 'Error de CRC',
  'lhymicro.status.239': 'Sin energía',
  'lhymicro.status.204': 'Terminó (M3)',
  'lhymicro.status.0': 'Sin respuesta',
  'lhymicro.status.other': 'Código {code}',
  'lhymicro.console.reason':
    'La placa M2 Nano del K40 no usa G-code, así que aquí no se pueden mandar líneas. El registro muestra lo que se envía y se recibe.',
  'lhymicro.override.reason':
    'Los ajustes en vivo de velocidad y potencia son de GRBL: en el K40 la velocidad va dentro del trabajo y la potencia se ajusta con la perilla de la máquina.',
  'lhymicro.mset.reason': 'Los ajustes «$» son de GRBL: la M2 Nano no los tiene.',
  'lhymicro.axes.reason': 'La M2 Nano no tiene eje Z, eje U ni control del aire asistido: enciéndelo a mano.',
  'lhymicro.fire.reason':
    'El disparo de prueba no está disponible en el K40: la M2 Nano no controla la potencia por software, así que no hay forma segura de dar un pulso corto.',
  'lhymicro.frame.laserOff':
    'El K40 encuadra siempre con el láser apagado (recorre el rectángulo del trabajo con saltos); no hay encuadre con el láser encendido.',
  'lhymicro.jog.note': 'Los saltos van a la velocidad de salto de la placa: la velocidad de arriba no se usa.',

  // ---- diálogo de inicio ---------------------------------------------------------------
  'lhymicro.start.title': 'K40 con M2 Nano (experimental)',
  'lhymicro.start.computing': 'Preparando el trabajo para la placa…',
  'lhymicro.start.error': 'No se pudo preparar el trabajo para la placa: {error}',
  'lhymicro.start.homeFirst':
    'No se sabe dónde está el cabezal: irá primero al home (finales de carrera) y después se grabará.',
  'lhymicro.start.summary':
    '{packets} paquetes · {burn} mm grabados · {travel} mm de viajes · estimado {time}',
  'lhymicro.start.warnings': 'Lo que la placa no hará como lo pediste',
  'lhymicro.start.noWarnings': 'El codificador no encontró nada que la placa ignore.',
  'lhymicro.start.confirm': 'Enviar a la máquina',
  'lhymicro.warn.power_ignored':
    '{layer}: pide {value} % de potencia, pero la placa no la controla: saldrá con la que marque la perilla del K40.',
  'lhymicro.warn.gray_thresholded':
    '{layer}: los niveles de gris se binarizan (cualquier potencia mayor que 0 quema). Usa un tramado para simular grises.',
  'lhymicro.warn.air_assist_ignored': '{layer}: el aire asistido no se controla desde la M2 Nano; enciéndelo a mano.',
  'lhymicro.warn.z_ignored': '{layer}: el eje Z no existe en la M2 Nano y se ignora.',
  'lhymicro.warn.dwell_skipped': '{layer}: se omite una pausa de {value} ms (la placa no hace pausas en el sitio).',
  'lhymicro.warn.dwell_fires': '{layer}: se omite un pulso de {value} ms (la placa no hace pulsos en el sitio).',
  'lhymicro.warn.block': 'bloque {block}',

  // ---- detener sin confirmación ---------------------------------------------------------
  'lhymicro.stop.title': 'La placa no confirmó la parada',
  'lhymicro.stop.text':
    'No hubo confirmación de que el cabezal se detuviera ni de que el láser se apagara: puede seguir en marcha o con el láser encendido. Usa el paro de emergencia del K40 o abre la tapa ahora.',
  'lhymicro.stop.dismiss': 'Entendido',

  // ---- home ------------------------------------------------------------------------------
  'lhymicro.cmd.home': 'Hacer home del K40…',
  'lhymicro.home.title': 'Hacer home del K40',
  'lhymicro.home.risk':
    'El home mueve el cabezal hasta los finales de carrera (arriba a la izquierda en un K40 de serie) con el láser apagado. Despeja el área y no pongas las manos en el carro.',
  'lhymicro.home.check1': 'El área de movimiento está despejada: el cabezal irá hasta los finales de carrera.',
  'lhymicro.home.check2': 'Tu K40 tiene finales de carrera (el de serie sí). Sin ellos, el cabezal chocaría con el marco: no lo uses.',
  'lhymicro.home.send': 'Hacer home',
  'lhymicro.home.exact': 'A la placa se le enviará la orden de home (IPP):',
  'lhymicro.positionLost.text':
    'El último trabajo se detuvo o falló y la placa no informa su posición: se perdió la cuenta. Haz home antes de mover el cabezal o enviar el siguiente trabajo.',
  'lhymicro.positionLost.home': 'Hacer home del K40…',
  'lhymicro.positionLost.reconnect':
    'La última vez, el trabajo se detuvo con el cabezal en movimiento y la posición se perdió. Haz home antes de trabajar.',

  // ---- asistente «Agregar máquina» -----------------------------------------------------
  'lhymicro.wizard.title': 'K40 con M2 Nano por USB',
  'lhymicro.wizard.intro':
    'La placa M2 Nano de serie no habla por puerto serie: Tlaku la busca por USB (1A86:5512) sin enviarle nada.',
  'lhymicro.wizard.found': 'Se encontró {n} placa(s) de K40 por USB.',
  'lhymicro.wizard.none':
    'No se encontró ninguna placa de K40 por USB. Comprueba el cable y que la máquina esté encendida y pulsa «Actualizar». También puedes saltar la detección y elegir «K40 con M2 Nano».',
  'lhymicro.wizard.use': 'Usar K40 con M2 Nano',
  'lhymicro.wizard.board': 'Modelo de placa del K40',
  'lhymicro.wizard.board.help':
    'La M2 es la de serie. Cámbiala solo si la placa lleva otra letra impresa; puedes cambiarlo después en el perfil.',
  'lhymicro.wizard.defaults':
    'Cama 300 × 200 mm, home arriba a la izquierda y 140 mm/s de velocidad máxima son valores de partida habituales: ajústalos en el perfil si tu K40 es distinto.',
  'lhymicro.note.power':
    'La M2 Nano de serie no controla la potencia por software: se ajusta con la perilla del K40. Tlaku ignora el porcentaje de potencia de las capas y lo avisa antes de enviar.',
  'lhymicro.note.position':
    'La placa no informa su posición: Tlaku la cuenta desde el home, así que hay que hacer home antes de mover el cabezal (enviar un trabajo lo hace solo).',
  'lhymicro.note.moshiboard':
    'Las Moshiboard comparten el USB de esta placa pero no son compatibles: si Tlaku lo detecta al conectar, lo dirá.',
} as const;
