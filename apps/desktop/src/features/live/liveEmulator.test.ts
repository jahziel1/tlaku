import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import type { JobPreview, StatusReport } from '@/api/types';
import { FALLBACK_DEVICE } from '@/lib/device';
import { createAppStore } from '@/store';
import { LiveService, useLive } from './service';

/**
 * Reproduce con el seguimiento real de la interfaz lo que el emulador GRBL (`tlaku-emu`, máquina
 * `virtual:falcon2`) informó al correr trabajos de verdad: el G-code que enviaría la app, con tres
 * perfiles (origen abajo a la izquierda, origen arriba a la derecha e intercambio X/Y). El rastro
 * lo escribe `machine/tests/live_view.rs` (`UPDATE_LIVE_TRACE=1 cargo test -p tlaku-desktop live_view`),
 * que además comprueba que cada reporte cae sobre la ruta de la vista previa.
 */

interface Run {
  name: string;
  speed: number;
  device: { origin: 'bottom_left' | 'top_right' | 'top_left' | 'bottom_right'; bed: [number, number]; swap_xy: boolean };
  preview: { moves: number[]; move_times: number[]; total_s: number; layers: { color: string; mode: string; seconds: number }[] };
  statuses: [number, StatusReport['state'], [number, number], number][];
  progress: [number, number, number][];
  jobs: [number, string][];
}

const trace = JSON.parse(readFileSync(join(__dirname, 'fixtures/emulator-trace.json'), 'utf8')) as { runs: Run[] };

function replay(run: Run) {
  let ms = 0;
  const store = createAppStore();
  const device = { ...FALLBACK_DEVICE, id: 'emu', origin: run.device.origin, bed: run.device.bed, swap_xy: run.device.swap_xy };
  const preview: JobPreview = {
    layers: run.preview.layers.map((l) => ({ ...l, summary: '', travel_saved_pct: null, explanations: [] })),
    moves: run.preview.moves,
    move_times: run.preview.move_times,
    total_s: run.preview.total_s,
    bounds: [[0, 0], [0, 0]],
    order_note: null,
    warnings: [],
  };
  store.setState({ devices: [device], activeDeviceId: 'emu', preview, previewStatus: 'ready', previewStale: false });
  store.getState().setPref('liveView', true);
  useLive.setState({ jobSeq: 0, route: 'none', source: null, fraction: 0, emitting: false, ended: null });
  const svc = new LiveService(store, { now: () => ms });
  const stop = svc.start();

  type Ev = { at: number; run: () => void };
  const events: Ev[] = [];
  for (const [at, state, work, spindle] of run.statuses) {
    events.push({ at, run: () => store.getState().onStatus({ state, machine_pos: work, work_pos: work, feed: 0, spindle, buffer: { planner: 15, rx: 127 } }) });
  }
  for (const [at, acked, total] of run.progress) events.push({ at, run: () => store.getState().onProgress({ acked, total }) });
  for (const [at, state] of run.jobs) {
    events.push({ at, run: () => store.getState().onJob({ state: state.toLowerCase() as 'started', message: '', position_lost: false }) });
  }
  events.sort((a, b) => a.at - b.at);

  const fractions: number[] = [];
  const sources: (string | null)[] = [];
  const speed = run.speed;
  for (const ev of events) {
    // El reloj de la máquina real corre `speed` veces más lento que el del emulador acelerado.
    const target = ev.at * speed;
    while (ms + 16 <= target) {
      ms += 16;
      const f = svc.frame(ms);
      if (f) fractions.push(f.fraction);
    }
    ms = target;
    ev.run();
    sources.push(useLive.getState().source);
  }
  for (let i = 0; i < 20; i++) {
    ms += 16;
    const f = svc.frame(ms);
    if (f) fractions.push(f.fraction);
  }
  return { svc, fractions, sources, stop };
}

describe('rastro del emulador GRBL (tlaku-emu)', () => {
  it('el rastro existe y trae los tres perfiles', () => {
    expect(trace.runs.map((r) => r.name)).toHaveLength(3);
    for (const r of trace.runs) expect(r.statuses.length).toBeGreaterThan(15);
  });

  for (const run of trace.runs) {
    it(`${run.name}: la vista llega al 100 % y el cabezal acaba en la última posición`, () => {
      const { svc, fractions, sources, stop } = replay(run);
      const route = svc.route!;
      expect(route).not.toBeNull();
      const f = svc.frame()!;
      expect(f.fraction).toBe(1);
      expect(f.mode).toBe('done');
      expect(f.x).toBeCloseTo(route.model.pos[2 * route.count]!, 3);
      expect(f.y).toBeCloseTo(route.model.pos[2 * route.count + 1]!, 3);
      expect(useLive.getState().fraction).toBe(1);
      expect(useLive.getState().ended).toBe('finished');
      // Nunca retrocede mientras la máquina avanza.
      expect(Math.min(...fractions.slice(1).map((v, i) => v - fractions[i]!))).toBeGreaterThanOrEqual(0);
      // El avance salió de la posición real (no hizo falta caer al progreso del envío ni al reloj).
      const seen = sources.filter((s) => s !== null);
      expect(seen.length).toBeGreaterThan(10);
      expect(seen.filter((s) => s === 'position').length / seen.length).toBeGreaterThan(0.9);
      expect(seen).not.toContain('clock');
      stop();
    });
  }
});
