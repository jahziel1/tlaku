/**
 * Vista en vivo del trabajo (`features/live`): la simulación dibujada que sigue a la máquina real,
 * en la PC y en el celular (la página del celular trae sus propios textos en `remote/page.html`).
 */
export const live = {
  'live.toggle': 'Vista en vivo',
  'live.toggle.hint': 'Dibuja sobre el diseño lo que hace la máquina: lo ya quemado en oscuro, lo que falta tenue y el cabezal, siguiendo la posición real que reporta la máquina. Solo mira: no envía nada a la máquina. Apagada, no consume recursos; puedes encenderla en cualquier momento, también a mitad de un trabajo.',
  'live.hud.title': 'Vista en vivo',
  'live.hud.percent': '{percent} %',
  'live.hud.elapsed': 'Transcurrido',
  'live.hud.remaining': 'Restante',
  'live.hud.hide': 'Volver al diseño',
  'live.hud.hideTip': 'Apaga la vista en vivo y vuelve a ver solo el diseño',
  'live.canvas.aria': 'Vista en vivo del trabajo: {percent} %, {state}. {source}',
  'live.state.waiting': 'esperando a la máquina',
  'live.state.running': 'en marcha',
  'live.state.paused': 'en pausa',
  'live.state.finished': 'terminado',
  'live.state.cancelled': 'detenido',
  'live.state.error': 'con error',
  'live.laser.on': 'láser encendido',
  'live.source.position': 'Según la posición real de la máquina.',
  'live.source.progress': 'Según el progreso del envío (la posición no encajó con el recorrido).',
  'live.source.clock': 'Estimado por el tiempo: esta máquina no reporta su posición mientras trabaja.',
  'live.route.computing': 'Calculando el recorrido…',
  'live.route.unavailable': 'La vista en vivo no está disponible para este trabajo (por ejemplo, una reanudación).',
  'live.route.none': 'Inicia un trabajo para ver aquí lo que hace la máquina.',
} as const;
