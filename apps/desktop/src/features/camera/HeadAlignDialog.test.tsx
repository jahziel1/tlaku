import { renderToString } from 'react-dom/server';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { CameraAlignment, HeadAlignReport } from '@/api/types';
import { commands } from '@/commands';
import { setLanguage, t } from '@/i18n';
import { ConfigView, ResultView, RunningView, issueText, type ConfigViewProps, type ResultViewProps } from './HeadAlignDialog';
import { HeadAlignView, type HeadAlignViewProps } from './HeadAlignSection';
import { DEFAULT_HEAD_ALIGN, useHeadAlign, type ShotInfo } from './headAlignState';

vi.mock('./session', () => ({
  grabCorrectedPng: async () => null,
  correctedSpace: () => [800, 600],
  framesRendered: () => 0,
  startCamera: async () => undefined,
}));

const COVERED: [number, number, number, number] = [150, 140, 250, 270];

function report(over: Partial<HeadAlignReport> = {}): HeadAlignReport {
  return {
    method: 'led',
    image_size: [800, 600],
    frames: [
      { machine_mm: [60, 60], px: null, failure: 'no_change', confidence: 0, error_mm: null, used: false },
      { machine_mm: [200, 207.5], px: [401.5, 300.2], failure: null, confidence: 0.9, error_mm: 0.31, used: true },
      { machine_mm: [150, 150], px: [330, 380], failure: null, confidence: 0.8, error_mm: 4.2, used: false },
      { machine_mm: [250, 270], px: null, failure: 'touches_border', confidence: 0, error_mm: null, used: false },
    ],
    seen_mm: [[200, 207.5]],
    detected: 2,
    failed: 2,
    fit: {
      to_bed: [[1, 0, 0], [0, 1, 0], [0, 0, 1]],
      lens: null,
      points: [],
      rms_mm: 0.21,
      max_mm: 0.4,
      loo_rms_mm: 0.34,
      loo_max_mm: 0.52,
      used: 9,
      dropped: 1,
      covered_mm: COVERED,
      quality: 'good',
      offset_mm: [0, 0],
      parallax_scale: 1,
      nadir_mm: [0, 0],
      issues: [],
    },
    fit_error: null,
    alignment: null,
    lens: null,
    covered_mm: COVERED,
    issues: [
      { kind: 'not_detected', count: 2 },
      { kind: 'partial_coverage', covered_mm: COVERED, area_mm: [20, 20, 380, 395] },
    ],
    ...over,
  };
}

beforeEach(() => {
  setLanguage('es');
  useHeadAlign.getState().closeDialog();
});

const noop = () => undefined;

function config(over: Partial<ConfigViewProps> = {}): ConfigViewProps {
  return { settings: DEFAULT_HEAD_ALIGN, set: noop, confirmed: false, setConfirmed: noop, error: null, blocked: null, hasLens: false, camStatus: 'live', onStart: noop, onStartCamera: noop, ...over };
}

function result(over: Partial<ResultViewProps> = {}): ResultViewProps {
  return { report: report(), shots: [], saved: false, error: null, settings: DEFAULT_HEAD_ALIGN, set: noop, hasLens: true, busy: false, blocked: null, onRecalc: noop, onSave: noop, onRepeat: noop, onDiscard: noop, ...over };
}

function section(over: Partial<HeadAlignViewProps> = {}): HeadAlignViewProps {
  return { alignment: null, headMode: false, bed: [400, 415], px: null, refBed: [0, 0], marking: false, busy: false, onOpen: noop, onRefBed: noop, onMark: noop, onApply: noop, ...over };
}

describe('preparación', () => {
  it('explica qué pasa, avisa que el cabezal se moverá y pide confirmar antes de empezar', () => {
    const html = renderToString(<ConfigView {...config()} />);
    expect(html).toContain(t('camera.headAlign.explain'));
    expect(html).toContain(t('camera.headAlign.warning'));
    // El botón de iniciar está deshabilitado (accesible) mientras no se confirme.
    expect(html).toMatch(/aria-disabled="true"[^>]*title="Confirma que despejaste la cama y cerraste la tapa\."[^>]*>(?:<svg.*?<\/svg>)?Iniciar alineación/s);
    expect(html).toContain(t('camera.headAlign.confirm.needed'));
    // Sin calibración de lente se avisa que se estimará una.
    expect(html).toContain(t('camera.headAlign.noLens'));
  });

  it('con la confirmación el botón queda disponible; con un motivo de bloqueo, no', () => {
    const ok = renderToString(<ConfigView {...config({ confirmed: true, hasLens: true })} />);
    expect(ok).toMatch(/aria-disabled="false"[^>]*>(?:<svg.*?<\/svg>)?Iniciar alineación/s);
    expect(ok).not.toContain(t('camera.headAlign.confirm.needed'));
    expect(ok).not.toContain(t('camera.headAlign.noLens'));
    const blocked = renderToString(<ConfigView {...config({ confirmed: true, blocked: 'camera.reason.notLive', camStatus: 'off' })} />);
    expect(blocked).toMatch(/aria-disabled="true"/);
    expect(blocked).toContain(t('camera.reason.notLive'));
    // Sin cámara se ofrece encenderla.
    expect(blocked).toContain(t('camera.marks.startCamera'));
  });

  it('las opciones tienen etiquetas y las alturas aparecen al elegir el paralaje', () => {
    expect(renderToString(<ConfigView {...config()} />)).not.toContain(t('camera.headAlign.cameraHeight'));
    const html = renderToString(<ConfigView {...config({ settings: { ...DEFAULT_HEAD_ALIGN, cameraHeightMm: 250, featureHeightMm: 40 } })} />);
    expect(html).toContain(t('camera.headAlign.cameraHeight'));
    expect(html).toContain(t('camera.headAlign.featureHeight'));
    for (const id of ['cam-ha-grid', 'cam-ha-method']) expect(html).toContain(`for="${id}"`);
    expect(html).toContain('3 × 3 (9 puntos)');
  });

  it('un error de la corrida anterior se anuncia', () => {
    expect(renderToString(<ConfigView {...config({ error: 'La máquina entró en alarma' })} />)).toMatch(/role="alert"[^>]*>La máquina entró en alarma/);
  });

  it('el texto sale en inglés al cambiar de idioma', () => {
    setLanguage('en');
    const html = renderToString(<ConfigView {...config()} />);
    setLanguage('es');
    expect(html).toContain('Start alignment');
    expect(html).not.toContain('Iniciar alineación');
  });
});

describe('recorrido', () => {
  const shots: ShotInfo[] = [
    { key: 'r0-0', mm: [60, 60], round: 0, state: 'shot', thumb: 'data:image/jpeg;base64,AAAA' },
    { key: 'r0-1', mm: [200, 60], round: 0, state: 'moving', thumb: null },
    { key: 'r0-2', mm: [340, 60], round: 0, state: 'pending', thumb: null },
  ];

  it('muestra el avance por punto con su estado y miniatura, y permite detener', () => {
    const html = renderToString(<RunningView shots={shots} round={1} cancelRequested={false} onStop={noop} />);
    expect(html).toContain('Foto 1 de 3');
    expect(html).toContain('role="meter"');
    expect(html).toContain('aria-valuenow="1"');
    expect(html).toContain('data:image/jpeg;base64,AAAA');
    expect(html).toContain('Moviendo el cabezal');
    expect(html).toContain('Pendiente');
    expect(html).toContain(t('camera.headAlign.laserOff'));
    expect(html).toContain('ronda de afinado 1');
    expect(html).toContain(t('camera.headAlign.stop'));
    // La lista de puntos está etiquetada y el avance se anuncia con educación.
    expect(html).toContain('aria-live="polite"');
    expect(html).toContain(`aria-label="${t('camera.headAlign.shots')}"`);
  });

  it('detenerse deshabilita el botón y lo dice', () => {
    const html = renderToString(<RunningView shots={shots} round={0} cancelRequested onStop={noop} />);
    expect(html).toContain(t('camera.headAlign.stopping'));
    expect(html).toMatch(/<button[^>]*disabled/);
  });
});

describe('resultado', () => {
  it('muestra el error, la calidad, qué punto no se vio y por qué, y los avisos', () => {
    const html = renderToString(<ResultView {...result()} />);
    expect(html).toContain('data-grade="good"');
    expect(html).toContain('0,34 mm');
    expect(html).toContain('Buena');
    expect(html).toContain('Cabezal localizado en 2 de 4 fotos');
    expect(html).toContain(t('camera.headAlign.usedLed'));
    // Tabla por punto: no visto, visto con su error, descartado y cortado por el borde.
    expect(html).toContain(`aria-label="${t('a11y.table.headAlign')}"`);
    expect(html).toContain('No se ve el cabezal');
    expect(html).toContain('0,31 mm');
    expect(html).toContain('Descartado');
    expect(html).toContain('Cabezal cortado por el borde');
    // Avisos y zona cubierta.
    expect(html).toContain('2 foto(s) sin el cabezal a la vista');
    expect(html).toContain('Fuera de esa zona la alineación es una extrapolación');
    expect(html).toContain('X 150–250 mm, Y 140–270 mm');
    // Se puede guardar.
    expect(html).toMatch(/aria-disabled="false"[^>]*>(?:<svg.*?<\/svg>)?Guardar alineación/s);
  });

  it('sin alineación el guardado está deshabilitado y se explica', () => {
    const html = renderToString(<ResultView {...result({ report: report({ fit: null, fit_error: 'quedan 1 puntos', detected: 1, issues: [] }), blocked: 'noFit' })} />);
    expect(html).toContain(t('camera.headAlign.noFit'));
    expect(html).toContain('quedan 1 puntos');
    expect(html).toContain(t('camera.headAlign.saveBlocked.noFit'));
    expect(html).toMatch(/aria-disabled="true"[^>]*title="No hay alineación que guardar\."/);
    // Repetir y descartar siempre están.
    expect(html).toContain(t('camera.headAlign.repeat'));
    expect(html).toContain(t('camera.headAlign.discard'));
  });

  it('con una lente estimada avisa que se guardará junto con la alineación', () => {
    const lens = { width: 800, height: 600, fx: 800, fy: 800, cx: 399.5, cy: 299.5, k1: -0.1, k2: 0, p1: 0, p2: 0, k3: 0, model: 'brown_conrady' as const, k4: 0, view_zoom: 1, rms_error_px: 0, views_used: 0 };
    expect(renderToString(<ResultView {...result({ report: report({ lens }), hasLens: false })} />)).toContain(t('camera.headAlign.lensWillSave'));
  });

  it('los avisos tienen texto en español e inglés y no son idénticos', () => {
    const issues = report({
      issues: [
        { kind: 'dropped', count: 2 },
        { kind: 'few_points', used: 6 },
        { kind: 'high_error', loo_rms_mm: 3.2 },
        { kind: 'lens_estimated', model: 'fisheye', rms_before_mm: 5.1, rms_after_mm: 0.7 },
      ],
    }).issues;
    const es = issues.map(issueText);
    setLanguage('en');
    const en = issues.map(issueText);
    setLanguage('es');
    expect(es).toHaveLength(4);
    es.forEach((text, i) => {
      expect(text.length).toBeGreaterThan(10);
      expect(text).not.toContain('camera.headAlign');
      expect(en[i]).not.toBe(text);
    });
  });
});

describe('sección del panel y comando', () => {
  const alignment: CameraAlignment = { image_size: [800, 600], image_px: [[0, 0], [1, 0], [1, 1], [0, 1]], bed_mm: [[0, 0], [1, 0], [1, 1], [0, 1]], to_bed: [[1, 0, 0], [0, 1, 0], [0, 0, 1]], rms_mm: 0.1, method: 'head_sweep', samples: [{ height_mm: 0, image_px: [], bed_mm: [], loo_rms_mm: 0.4 }] };

  it('explica la opción y ofrece abrir el asistente', () => {
    const html = renderToString(<HeadAlignView {...section()} />);
    expect(html).toContain(t('camera.headAlign.title'));
    expect(html).toContain(t('camera.headAlign.help'));
    expect(html).toContain(t('camera.headAlign.open'));
    // Sin alineación no hay ajuste con punto de referencia.
    expect(html).not.toContain(t('camera.headAlign.ref.title'));
  });

  it('con una alineación guardada aparece el ajuste con un punto de referencia', () => {
    const html = renderToString(<HeadAlignView {...section({ alignment })} />);
    expect(html).toContain('Alineación hecha con el cabezal (error de validación cruzada 0,40 mm)');
    expect(html).toContain(t('camera.headAlign.ref.title'));
    expect(html).toContain(t('camera.headAlign.ref.none'));
    expect(html).toMatch(/aria-disabled="true"[^>]*title="Marca primero el disparo/);
    const marked = renderToString(<HeadAlignView {...section({ alignment, px: [401.5, 300.25], marking: true })} />);
    expect(marked).toContain('aria-pressed="true"');
    expect(marked).toContain('Marcado en el píxel (401,5, 300,3).');
    expect(marked).toMatch(/aria-disabled="false"[^>]*>Aplicar punto de referencia/);
  });

  it('con la cámara en el cabezal la opción se explica y se deshabilita', () => {
    const html = renderToString(<HeadAlignView {...section({ headMode: true })} />);
    expect(html).toMatch(/aria-disabled="true"/);
    expect(html).toContain(t('camera.headAlign.reason.headMode'));
  });

  it('el comando del menú abre el asistente', async () => {
    await commands.run('camera.alignHead');
    expect(useHeadAlign.getState().open).toBe(true);
    expect(useHeadAlign.getState().phase).toBe('config');
    useHeadAlign.getState().closeDialog();
  });
});
