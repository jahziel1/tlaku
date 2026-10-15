/**
 * Notice for a board that is not answering (early warning, the connection is not dropped). Every
 * key starts with `silence.`.
 */
export const silence = {
  // ---- the board is not answering ----
  'silence.warn.title': 'The machine has not answered for {n} s…',
  'silence.warn.hint': 'Tlaku keeps waiting: the board sometimes stalls for a few seconds and comes back on its own. This notice goes away when it answers.',
  'silence.warn.job': 'The job will carry on by itself once it answers. If something looks wrong, use Stop.',

  // ---- more than a minute in the middle of a job ----
  'silence.serious.title': 'It has been silent for over a minute',
  'silence.serious.text': 'Check the machine; if it keeps moving and does not respond, use the emergency stop. Tlaku does not disconnect it because of this.',

  'silence.stop': 'Stop',
  'silence.disconnect': 'Disconnect',
};
