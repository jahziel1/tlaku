/** `jsdom` no trae tipos propios: solo se declara lo que usan las pruebas que abren la página del celular. */
declare module 'jsdom' {
  export class JSDOM {
    constructor(html?: string, options?: Record<string, unknown>);
    readonly window: Window & typeof globalThis;
  }
}
