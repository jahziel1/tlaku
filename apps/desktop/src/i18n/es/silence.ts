/**
 * Aviso de la placa que no contesta (aviso temprano, sin perder la conexión). Todas las claves
 * empiezan con `silence.`.
 */
export const silence = {
  // ---- la placa no contesta ----
  'silence.warn.title': 'La máquina no contesta desde hace {n} s…',
  'silence.warn.hint': 'Tlaku sigue esperando: a veces la placa se detiene unos segundos y vuelve sola. El aviso se quita cuando conteste.',
  'silence.warn.job': 'El trabajo continuará solo cuando conteste. Si algo va mal, usa Detener.',

  // ---- más de un minuto en pleno trabajo ----
  'silence.serious.title': 'Lleva más de un minuto sin contestar',
  'silence.serious.text': 'Revisa la máquina; si sigue moviéndose y no responde, usa el paro de emergencia. Tlaku no la desconecta por esto.',

  'silence.stop': 'Detener',
  'silence.disconnect': 'Desconectar',
};
