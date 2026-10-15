/**
 * Interpolación del cabezal entre reportes de la máquina (~4-5 Hz) para que se mueva fluido.
 *
 * El tiempo de trabajo `t` (segundos sobre la ruta de la vista previa) que se DIBUJA va siempre
 * detrás o junto a lo que dice la máquina más una extrapolación corta:
 *
 *  - cada reporte trae el tiempo `tLoc` en que la posición real quedó ubicada en la ruta;
 *  - entre reportes se extrapola a la velocidad medida (`rate`, tiempo de ruta por segundo real),
 *    como mucho `MAX_AGE_S` segundos reales: si los reportes se detienen, la cabeza se detiene;
 *  - el tiempo dibujado se acerca a ese objetivo con un filtro de primer orden (`TAU_S`) y NUNCA
 *    retrocede, salvo que el ubicador avise de un retroceso real de la máquina;
 *  - en pausa (Hold) se detiene en seco; cancelado o con error queda congelado donde estaba;
 *    al terminar va al final de la ruta.
 *
 * Esta lógica la copia el celular en `src-tauri/src/remote/page.html` (bloque `live-core`); una
 * prueba fija que las dos den lo mismo.
 */

export const TAU_S = 0.12;
export const MAX_AGE_S = 0.8;
export const RATE_MIN = 0.25;
export const RATE_MAX = 20;
/** Veces la velocidad normal a la que se alcanza un objetivo que quedó lejos. */
export const CATCH_UP = 8;
/** Si el objetivo queda más lejos que esto (s de trabajo) se salta directo. */
export const SNAP_AHEAD_S = 6;
/** Cuadro más largo que se acepta (s): tras una pausa del navegador no se salta de golpe. */
export const MAX_DT_S = 0.1;

export type TrackerMode = 'running' | 'hold' | 'frozen' | 'done';

export interface TrackerReport {
  /** Tiempo de trabajo (s) en que se ubicó la posición real. */
  t: number;
  /** La máquina está ejecutando (no en Hold ni en pausa). */
  running: boolean;
  /** La máquina retrocedió de verdad: se salta atrás. */
  retreat?: boolean;
}

const clamp = (v: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, v));

export class LiveTracker {
  /** Tiempo de trabajo dibujado (s). */
  t = 0;
  mode: TrackerMode = 'hold';
  /** Velocidad medida: segundos de ruta por segundo real. */
  rate = 1;
  private tLoc = 0;
  private wLoc = 0;
  private seen = false;
  private lastWall = 0;
  private stalled = 0;

  constructor(readonly total: number) {}

  /** Último tiempo ubicado por la máquina (s): lo que se le cuenta al celular. */
  get located(): number {
    return this.tLoc;
  }

  /** Ya llegó algún reporte. */
  get started(): boolean {
    return this.seen;
  }

  /** Un reporte ubicado en la ruta. `wall` en segundos de reloj. */
  report(r: TrackerReport, wall: number): void {
    if (this.mode === 'frozen' || this.mode === 'done') return;
    const t = clamp(r.t, 0, this.total);
    if (!this.seen) {
      // Primer reporte (o vista activada a mitad del trabajo): se salta directo, sin animar desde cero.
      this.seen = true;
      this.t = t;
      this.lastWall = wall;
    } else {
      const dw = wall - this.wLoc;
      if (r.running && this.mode === 'running' && dw >= 0.08 && dw <= 3) {
        const dt = t - this.tLoc;
        if (dt > 1e-3) {
          this.rate = clamp(this.rate * 0.6 + (dt / dw) * 0.4, RATE_MIN, RATE_MAX);
          this.stalled = 0;
        } else if (++this.stalled >= 2) {
          // En marcha pero sin avanzar (un dwell, el planificador vacío): no se extrapola igual.
          this.rate = Math.max(RATE_MIN, this.rate * 0.5);
        }
      }
      if (r.retreat && t < this.t) this.t = t;
      else if (t - this.t > SNAP_AHEAD_S) this.t = t;
    }
    this.tLoc = r.retreat ? t : Math.max(t, this.tLoc);
    this.wLoc = wall;
    this.mode = r.running ? 'running' : 'hold';
  }

  /** Avanza al instante `wall` (s) y devuelve el tiempo de trabajo a dibujar. */
  step(wall: number): number {
    const dt = clamp(wall - this.lastWall, 0, MAX_DT_S);
    this.lastWall = wall;
    if (this.mode === 'frozen') return this.t;
    if (this.mode === 'done') {
      this.t = this.total;
      return this.t;
    }
    if (!this.seen) return this.t;
    const target = this.mode === 'running' ? this.tLoc + this.rate * clamp(wall - this.wLoc, 0, MAX_AGE_S) : this.tLoc;
    if (target > this.t) {
      const lag = (target - this.t) * (1 - Math.exp(-dt / TAU_S));
      const cap = dt * Math.max(this.rate, 1) * CATCH_UP;
      this.t = Math.min(this.total, this.t + Math.min(lag, cap));
      // La cola del filtro exponencial se cierra: en pausa la cabeza queda exactamente quieta.
      if (target - this.t < 1e-3) this.t = Math.min(this.total, target);
    }
    return this.t;
  }

  /** El trabajo se canceló o falló: la cabeza queda donde está. */
  freeze(): void {
    this.mode = 'frozen';
  }

  /** El trabajo terminó: la cabeza llega al final de la ruta. */
  finish(): void {
    this.mode = 'done';
    this.t = this.total;
    // Aunque no hubiera llegado ningún reporte (vista encendida después de terminar), ya hay qué dibujar.
    this.seen = true;
  }
}
