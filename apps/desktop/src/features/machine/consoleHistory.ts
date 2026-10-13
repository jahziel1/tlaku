/** Historial de comandos de la consola: flecha arriba/abajo, sin repetidos y con el borrador intacto. */
export class ConsoleHistory {
  private list: string[];
  private cursor = -1;
  private draft = '';

  constructor(
    private readonly limit = 50,
    initial: readonly string[] = [],
  ) {
    this.list = initial.slice(0, limit);
  }

  /** Del más reciente al más antiguo. */
  items(): readonly string[] {
    return this.list;
  }

  /** Registra lo enviado: el más reciente va primero y un repetido sube en vez de duplicarse. */
  push(text: string): void {
    const t = text.trim();
    this.cursor = -1;
    this.draft = '';
    if (!t) return;
    this.list = [t, ...this.list.filter((h) => h !== t)].slice(0, this.limit);
  }

  /** Flecha arriba: el comando anterior. `current` es lo escrito hasta ahora (se recupera al bajar). */
  prev(current: string): string {
    if (this.list.length === 0) return current;
    if (this.cursor === -1) this.draft = current;
    this.cursor = Math.min(this.list.length - 1, this.cursor + 1);
    return this.list[this.cursor]!;
  }

  /** Flecha abajo: el siguiente; al pasar el más reciente vuelve el borrador. */
  next(current: string): string {
    if (this.cursor === -1) return current;
    this.cursor -= 1;
    return this.cursor === -1 ? this.draft : this.list[this.cursor]!;
  }

  /** Se llama al editar a mano: el recorrido empieza de nuevo desde el borrador. */
  reset(): void {
    this.cursor = -1;
    this.draft = '';
  }
}
