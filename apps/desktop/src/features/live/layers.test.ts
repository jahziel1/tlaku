import { describe, expect, it } from 'vitest';
import { fakeCanvasFactory, type FakeCanvas } from './fakeCanvas';
import { charred, layerGeometry, LiveLayers, MAX_SIDE } from './layers';
import { buildLiveRoute, posAtTime } from './route';
import { previewOf, rasterFill, squareCut, type Seg } from './testRoutes';

const style = { fallbackCut: '#c0392b', fallbackEngrave: '#2e86c1', travel: '#6b5a48' };

function layers(p = squareCut(), showTravel = false, maxSide = MAX_SIDE) {
  const made: FakeCanvas[] = [];
  const route = buildLiveRoute(p)!;
  const l = new LiveLayers(route, style, fakeCanvasFactory(made), { showTravel, maxSide });
  return { l, route, pending: made[0]!, burned: made[1]! };
}

describe('geometría de las capas', () => {
  it('cubre la caja del trabajo con margen y no pasa del tope de píxeles por lado', () => {
    const route = buildLiveRoute(rasterFill(10, 5000, 0.1))!; // 5 m de ancho
    const g = layerGeometry(route);
    expect(g.width).toBeLessThanOrEqual(MAX_SIDE);
    expect(g.height).toBeLessThanOrEqual(MAX_SIDE);
    expect(g.scale).toBeLessThan(1);
    const small = layerGeometry(buildLiveRoute(squareCut(20))!);
    expect(small.scale).toBeGreaterThan(10);
  });

  it('charred oscurece el color de la capa sin perder su tono', () => {
    const c = charred('#ff0000');
    const [r, g, b] = c.match(/\d+/g)!.map(Number);
    expect(r).toBeLessThan(120);
    expect(r).toBeGreaterThan(g!);
    expect(b!).toBeLessThan(60);
    expect(charred('no-es-color')).toMatch(/^rgb\(/);
  });
});

describe('lo quemado se acumula de forma incremental', () => {
  it('traza solo lo nuevo en cada avance y no repite lo ya trazado', () => {
    const { l, route, burned } = layers();
    const p = posAtTime(route, route.total * 0.5);
    expect(l.advance(p.move, p.frac, 1000)).toBe(true);
    const lines = burned.calls.lineTo;
    expect(lines).toBeGreaterThan(0);
    // Mismo punto otra vez: no se traza nada.
    expect(l.advance(p.move, p.frac, 1000)).toBe(true);
    expect(burned.calls.lineTo).toBe(lines);
    // Un poco más adelante: solo el tramo nuevo (una línea, en el mismo movimiento).
    const q = posAtTime(route, route.total * 0.5 + 0.2);
    l.advance(q.move, q.frac, 1000);
    expect(burned.calls.lineTo - lines).toBeLessThanOrEqual(2);
    expect(l.burnedTo).toEqual({ move: q.move, frac: q.frac });
    // Nunca se borra mientras avanza.
    expect(burned.calls.clearRect).toBe(0);
  });

  it('el desplazamiento no se dibuja (o muy tenue, si se pide) y no ensucia lo quemado', () => {
    const noTravel = layers();
    noTravel.l.advance(0, 1, 1000); // solo el viaje inicial
    expect(noTravel.burned.calls.lineTo).toBe(0);
    const withTravel = layers(squareCut(), true);
    withTravel.l.advance(0, 1, 1000);
    expect(withTravel.burned.calls.lineTo).toBe(1);
    expect(withTravel.burned.calls.styles.at(-1)).toBe('#6b5a48');
  });

  it('usa el color de la capa oscurecido; sin capas, el neutro', () => {
    const { l, route, burned } = layers();
    l.advance(route.count - 1, 1, 1000);
    expect(burned.calls.styles.at(-1)).toBe(charred('#ff0000'));
    const segs: Seg[] = [[0, 5, 5, 0], [1, 50, 5, 90]];
    const p = previewOf(segs);
    p.layers = [];
    const made: FakeCanvas[] = [];
    const bare = new LiveLayers(buildLiveRoute(p)!, style, fakeCanvasFactory(made));
    bare.advance(1, 1, 1000);
    expect(made[1]!.calls.styles.at(-1)).toBe(charred('#c0392b', 0.55));
  });

  it('un trabajo enorme se pone al día por tandas dentro del presupuesto de cada cuadro', () => {
    const { l, route } = layers(rasterFill(12000, 60, 0.05));
    expect(route.count).toBeGreaterThan(40000);
    let t = 0;
    let frames = 0;
    // Reloj que avanza 1 ms por consulta: con 3 ms de presupuesto cada llamada hace pocas tandas.
    const now = () => t++;
    while (!l.advance(route.count - 1, 1, 3, now)) frames++;
    expect(frames).toBeGreaterThanOrEqual(2);
    expect(l.burnedTo.move).toBe(route.count - 1);
  });

  it('muchos trazos seguidos de la misma capa van en un solo trazado', () => {
    const { l, route, burned } = layers(rasterFill(1500, 60, 0.05));
    l.advance(route.count - 1, 1, 100000);
    expect(burned.calls.stroke).toBeLessThan(route.count / 100);
    expect(burned.calls.lineTo).toBeGreaterThanOrEqual(1500);
  });

  it('cada trazo empieza con su moveTo (también con los desplazamientos tenues intercalados)', () => {
    for (const showTravel of [false, true]) {
      const { l, route, burned } = layers(rasterFill(40, 30, 0.5), showTravel);
      l.advance(route.count - 1, 1, 1e6);
      // 40 pasadas de grabado: cada una necesita su moveTo; con los desplazamientos, una más por cada uno.
      expect(burned.calls.moveTo).toBeGreaterThanOrEqual(showTravel ? 79 : 40);
      expect(burned.calls.lineTo).toBeGreaterThanOrEqual(showTravel ? 79 : 40);
      expect(burned.calls.moveTo).toBeLessThanOrEqual(burned.calls.lineTo);
    }
  });

  it('si la máquina retrocede borra lo quemado y lo vuelve a trazar hasta el nuevo punto', () => {
    const { l, route, burned } = layers();
    l.advance(route.count - 1, 1, 1000);
    const p = posAtTime(route, route.total * 0.3);
    l.advance(p.move, p.frac, 1000);
    expect(burned.calls.clearRect).toBe(1);
    expect(l.burnedTo).toEqual({ move: p.move, frac: p.frac });
  });
});

describe('lo pendiente se pre-renderiza una vez', () => {
  it('renderPending termina por tandas y después no vuelve a trazar', () => {
    const { l, pending } = layers(rasterFill(3000, 60, 0.05));
    expect(l.pendingDone).toBe(false);
    let t = 0;
    let calls = 0;
    while (!l.renderPending(2, () => t++)) calls++;
    expect(calls).toBeGreaterThan(0);
    expect(l.pendingDone).toBe(true);
    const drawn = pending.calls.lineTo;
    expect(drawn).toBeGreaterThan(0);
    expect(l.renderPending(1000)).toBe(true);
    expect(pending.calls.lineTo).toBe(drawn);
  });

  it('un cuadro de pantalla son solo dos drawImage, sin importar el tamaño del trabajo', () => {
    const { l } = layers(rasterFill(3000, 60, 0.05));
    const made: FakeCanvas[] = [];
    const screen = fakeCanvasFactory(made)();
    l.blit(screen.getContext('2d')!, { zoom: 3, panX: 20, panY: 400 });
    expect(made[0]!.calls.drawImage).toBe(2);
  });
});
