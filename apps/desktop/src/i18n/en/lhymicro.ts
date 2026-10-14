import type { lhymicro as Source } from '../es/lhymicro';

/** English for the `lhymicro` area. Missing keys fall back to Spanish (see `i18n/index.ts`). */
export const lhymicro: { [K in keyof typeof Source]?: string } = {
  // ---- common ------------------------------------------------------------------------------
  'lhymicro.experimental': 'Experimental',
  'lhymicro.experimental.note':
    'Experimental K40 support: it was tested against a simulated board, not a real one. Try it first with the laser disabled or very weak, and keep the emergency stop within reach.',
  'lhymicro.virtual.name': 'K40 (M2 Nano)',
  'lhymicro.port.usb': 'K40 · USB board {n}',
  'lhymicro.power.note':
    'The stock M2 Nano board does not control power by software: it is set with the K40\'s knob. Tlaku ignores the layers\' power percentage.',

  // ---- machine profile ---------------------------------------------------------------------
  'lhymicro.dev.group': 'K40 board (M2 Nano)',
  'lhymicro.dev.board': 'Board model',
  'lhymicro.dev.board.help':
    'It changes the speed equations. The M2 is the stock board on almost every K40; use another one only if the letter printed on your board is different (M, M1, M3, A, B, B1 or B2).',
  'lhymicro.dev.board.default': '{board} (stock)',
  'lhymicro.dev.origin': 'Origin and home',
  'lhymicro.dev.origin.help':
    'The home of a stock K40 is at the top left: pick "Top left" under Origin. If your head seeks another corner, pick that one. It is an assumption until tested on hardware: do a small move with the laser off first.',
  'lhymicro.dev.maxRate':
    'On a K40 the maximum cutting speed is the larger of the two speed limits (8400 mm/min is 140 mm/s): a faster layer is rejected when sending.',
  'lhymicro.dev.connection':
    'USB connection (CH341, 1A86:5512): the board is picked in the Machine panel. Close K40 Whisperer, MeerK40t and LaserDRW before connecting.',
  'lhymicro.dev.hidden':
    'A K40 profile does not use G-code, max S, baud rate or macros, nor the Z and U axes: those fields do not apply.',

  // ---- Machine panel -----------------------------------------------------------------------
  'lhymicro.panel.title': 'K40 with M2 Nano',
  'lhymicro.panel.needConnect': 'Connect to the board to see its status.',
  'lhymicro.panel.noG':
    'This machine does not use G-code: the console, live overrides, GRBL "$" settings, the Z axis and air assist do not apply.',
  'lhymicro.panel.board': 'Board',
  'lhymicro.panel.status': 'Board status',
  'lhymicro.panel.position': 'Position',
  'lhymicro.panel.position.known': 'Counted from the last home',
  'lhymicro.panel.position.unknown': 'Unknown: do a home',
  'lhymicro.panel.positionNote':
    'The board does not report its real position: Tlaku counts it from home. If you move the carriage by hand or a step is lost, home again.',
  'lhymicro.panel.unknownWarn':
    'The head position is unknown: nothing will move until you home. Sending a job homes first.',
  'lhymicro.panel.lowPower':
    'The board reports insufficient power (239): check the emergency stop, the lid and that the laser power supply is on.',
  'lhymicro.panel.moshiboard':
    'Moshiboards share the M2 Nano\'s USB id but are not compatible: if that warning shows when connecting, your machine has one.',
  'lhymicro.status.206': 'Ready',
  'lhymicro.status.238': 'Busy',
  'lhymicro.status.236': 'Finished',
  'lhymicro.status.207': 'CRC error',
  'lhymicro.status.239': 'No power',
  'lhymicro.status.204': 'Finished (M3)',
  'lhymicro.status.0': 'No reply',
  'lhymicro.status.other': 'Code {code}',
  'lhymicro.console.reason':
    'The K40\'s M2 Nano board does not use G-code, so lines cannot be sent here. The log shows what is sent and received.',
  'lhymicro.override.reason':
    'Live speed and power overrides are a GRBL feature: on the K40 the speed is part of the job and the power is set with the machine\'s knob.',
  'lhymicro.mset.reason': 'The "$" settings are a GRBL feature: the M2 Nano has none.',
  'lhymicro.axes.reason': 'The M2 Nano has no Z axis, no U axis and no air assist control: turn the air on by hand.',
  'lhymicro.fire.reason':
    'Test fire is not available on the K40: the M2 Nano does not control power by software, so there is no safe way to give a short pulse.',
  'lhymicro.frame.laserOff':
    'The K40 always frames with the laser off (it walks the job\'s rectangle with moves); there is no framing with the laser on.',
  'lhymicro.jog.note': 'Moves go at the board\'s jump speed: the speed above is not used.',

  // ---- start dialog ------------------------------------------------------------------------
  'lhymicro.start.title': 'K40 with M2 Nano (experimental)',
  'lhymicro.start.computing': 'Preparing the job for the board…',
  'lhymicro.start.error': 'The job could not be prepared for the board: {error}',
  'lhymicro.start.homeFirst':
    'The head position is unknown: it will go to home (limit switches) first and then burn.',
  'lhymicro.start.summary':
    '{packets} packets · {burn} mm burned · {travel} mm of travel · estimated {time}',
  'lhymicro.start.warnings': 'What the board will not do as you asked',
  'lhymicro.start.noWarnings': 'The encoder found nothing that the board ignores.',
  'lhymicro.start.confirm': 'Send to the machine',
  'lhymicro.warn.power_ignored':
    '{layer}: asks for {value} % power, but the board does not control it: it will burn at whatever the K40\'s knob says.',
  'lhymicro.warn.gray_thresholded':
    '{layer}: gray levels are binarized (any power above 0 burns). Use a halftone dither to simulate grays.',
  'lhymicro.warn.air_assist_ignored': '{layer}: air assist cannot be controlled from the M2 Nano; turn it on by hand.',
  'lhymicro.warn.z_ignored': '{layer}: the Z axis does not exist on the M2 Nano and is ignored.',
  'lhymicro.warn.dwell_skipped': '{layer}: a {value} ms pause is skipped (the board cannot pause in place).',
  'lhymicro.warn.dwell_fires': '{layer}: a {value} ms pulse is skipped (the board cannot pulse in place).',
  'lhymicro.warn.block': 'block {block}',

  // ---- unconfirmed stop --------------------------------------------------------------------
  'lhymicro.stop.title': 'The board did not confirm the stop',
  'lhymicro.stop.text':
    'There was no confirmation that the head stopped or that the laser turned off: it may still be moving or firing. Use the K40\'s emergency stop or open the lid now.',
  'lhymicro.stop.dismiss': 'Got it',

  // ---- home --------------------------------------------------------------------------------
  'lhymicro.cmd.home': 'Home the K40…',
  'lhymicro.home.title': 'Home the K40',
  'lhymicro.home.risk':
    'Homing moves the head to the limit switches (top left on a stock K40) with the laser off. Clear the area and keep your hands off the carriage.',
  'lhymicro.home.check1': 'The movement area is clear: the head will travel to the limit switches.',
  'lhymicro.home.check2': 'Your K40 has limit switches (the stock one does). Without them the head would hit the frame: do not use this.',
  'lhymicro.home.send': 'Home',
  'lhymicro.home.exact': 'The board will be sent the home command (IPP):',
  'lhymicro.positionLost.text':
    'The last job stopped or failed and the board does not report its position: the count was lost. Home before moving the head or sending the next job.',
  'lhymicro.positionLost.home': 'Home the K40…',
  'lhymicro.positionLost.reconnect':
    'Last time the job stopped with the head moving and the position was lost. Home before working.',

  // ---- "Add machine" wizard ----------------------------------------------------------------
  'lhymicro.wizard.title': 'K40 with M2 Nano over USB',
  'lhymicro.wizard.intro':
    'The stock M2 Nano board has no serial port: Tlaku looks for it on USB (1A86:5512) without sending it anything.',
  'lhymicro.wizard.found': 'Found {n} K40 board(s) on USB.',
  'lhymicro.wizard.none':
    'No K40 board was found on USB. Check the cable and that the machine is on, then press "Refresh". You can also skip detection and pick "K40 with M2 Nano".',
  'lhymicro.wizard.use': 'Use K40 with M2 Nano',
  'lhymicro.wizard.board': 'K40 board model',
  'lhymicro.wizard.board.help':
    'The M2 is the stock board. Change it only if your board has another letter printed on it; you can change it later in the profile.',
  'lhymicro.wizard.defaults':
    'A 300 × 200 mm bed, a top-left home and a 140 mm/s maximum speed are typical starting values: adjust them in the profile if your K40 is different.',
  'lhymicro.note.power':
    'The stock M2 Nano does not control power by software: it is set with the K40\'s knob. Tlaku ignores the layers\' power percentage and warns about it before sending.',
  'lhymicro.note.position':
    'The board does not report its position: Tlaku counts it from home, so home before moving the head (sending a job does it by itself).',
  'lhymicro.note.moshiboard':
    'Moshiboards share this board\'s USB id but are not compatible: if Tlaku detects one when connecting, it will say so.',
};
