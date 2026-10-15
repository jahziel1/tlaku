import type { live as Source } from '../es/live';

/** English for the `live` area. Missing keys fall back to Spanish (see `i18n/index.ts`). */
export const live: { [K in keyof typeof Source]?: string } = {
  'live.toggle': 'Live view',
  'live.toggle.hint': 'Draws on top of the design what the machine is doing: what is already burned in dark, what is left faint, and the head, following the real position the machine reports. View only: it sends nothing to the machine. When off it uses no resources; you can turn it on at any time, also in the middle of a job.',
  'live.hud.title': 'Live view',
  'live.hud.percent': '{percent}%',
  'live.hud.elapsed': 'Elapsed',
  'live.hud.remaining': 'Left',
  'live.hud.hide': 'Back to design',
  'live.hud.hideTip': 'Turn the live view off and see only the design again',
  'live.canvas.aria': 'Live view of the job: {percent}%, {state}. {source}',
  'live.state.waiting': 'waiting for the machine',
  'live.state.running': 'running',
  'live.state.paused': 'paused',
  'live.state.finished': 'finished',
  'live.state.cancelled': 'stopped',
  'live.state.error': 'failed',
  'live.laser.on': 'laser on',
  'live.source.position': 'Based on the real position of the machine.',
  'live.source.progress': 'Based on the send progress (the position did not match the toolpath).',
  'live.source.clock': 'Estimated from time: this machine does not report its position while working.',
  'live.route.computing': 'Computing the toolpath…',
  'live.route.unavailable': 'The live view is not available for this job (for example, a resumed job).',
  'live.route.none': 'Start a job to see here what the machine is doing.',
};
