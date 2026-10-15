import { frameAt, type LiveFrame } from './follow';
import type { RouteShape } from './route';
import type { LiveReading } from './service';
import { EMPTY_SNAPSHOT, sameSnapshot, type LiveSnapshot, type LiveSource } from './source';
import { LiveTracker } from './tracker';
import { decodeRoute, decodeState } from './wire';

/**
 * Fuente de la ventana aparte: no sigue a la máquina (no tiene acceso a ella). Recibe de la
 * ventana principal la ruta y la foto del estado (`wire.ts`) y arma su propio `LiveTracker`
 * con los reportes, de modo que el cabezal se mueve igual de fluido que en la ventana principal.
 */
export class RemoteLiveSource implements LiveSource {
  private snap: LiveSnapshot = EMPTY_SNAPSHOT;
  private effective: LiveSnapshot = EMPTY_SNAPSHOT;
  private shape: RouteShape | null = null;
  private shapeSeq = 0;
  private tracker: LiveTracker | null = null;
  private reading: LiveReading | null = null;
  private retreatSeen = 0;
  private spindle = 0;
  private readonly listeners = new Set<() => void>();

  /** `now`: reloj en ms (el de `performance.now` por omisión). */
  constructor(private readonly now: () => number = () => performance.now()) {}

  /** Llegó un mensaje `route`. Devuelve si era válido. */
  applyRoute(text: string): boolean {
    const got = decodeRoute(text);
    if (!got) return false;
    this.shape = got.route;
    this.shapeSeq = got.seq;
    this.tracker = new LiveTracker(got.route.total);
    this.retreatSeen = 0;
    if (this.reading) this.applyReading(this.reading);
    this.changed();
    return true;
  }

  /** Llegó un mensaje `state`. Devuelve si era válido. */
  applyState(text: string): boolean {
    const got = decodeState(text);
    if (!got) return false;
    this.snap = got.snap;
    // Otro trabajo: la ruta anterior ya no sirve (la nueva llega en su propio mensaje).
    if (this.shape && got.snap.jobSeq !== this.shapeSeq) {
      this.shape = null;
      this.tracker = null;
      this.retreatSeen = 0;
    }
    if (got.reading) {
      this.reading = got.reading;
      this.applyReading(got.reading);
    }
    this.changed();
    return true;
  }

  private applyReading(r: LiveReading): void {
    this.spindle = r.spindle;
    const tracker = this.tracker;
    if (!tracker || r.jobSeq !== this.shapeSeq) return;
    if (r.mode === 'done') tracker.finish();
    else if (r.mode === 'frozen') tracker.freeze();
    else tracker.report({ t: r.t, running: r.mode === 'running', retreat: r.retreatSeq > this.retreatSeen }, this.now() / 1000);
    this.retreatSeen = r.retreatSeq;
  }

  private changed(): void {
    const hasRoute = !!this.shape && this.shapeSeq === this.snap.jobSeq;
    // Mientras la ruta no llega, se dice «calculando» aunque la principal ya la tenga lista.
    const next: LiveSnapshot = this.snap.route === 'ready' && !hasRoute ? { ...this.snap, route: 'computing' } : this.snap;
    if (!sameSnapshot(this.effective, next)) this.effective = next;
    this.listeners.forEach((fn) => fn());
  }

  snapshot(): LiveSnapshot {
    return this.effective;
  }

  route(): RouteShape | null {
    return this.effective.route === 'ready' ? this.shape : null;
  }

  frame(): LiveFrame | null {
    if (!this.shape || !this.tracker || this.effective.route !== 'ready') return null;
    return frameAt(this.shape, this.tracker, this.spindle, this.now());
  }

  subscribe(fn: () => void): () => void {
    this.listeners.add(fn);
    return () => this.listeners.delete(fn);
  }
}
