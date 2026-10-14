import {
  ArrowDown,
  ArrowUp,
  Eye,
  EyeOff,
  Image as ImageIcon,
  MousePointerClick,
  Pencil,
  PaintBucket,
  Pin,
  PinOff,
  Settings2,
  SlidersHorizontal,
  Spline,
  Zap,
  ZapOff,
  Ellipsis,
  GripVertical,
  SquareDashed,
  ClipboardPaste,
  Copy,
  Trash2,
} from 'lucide-react';
import { useEffect, useMemo, useRef, useState, type KeyboardEvent, type PointerEvent as ReactPointerEvent } from 'react';
import { useShallow } from 'zustand/react/shallow';
import type { LayerOrder } from '@/api/types';
import { t, tn } from '@/i18n';
import { countByLayer, layerFeatures, type LayerCount } from '@/lib/layers';
import { colorName, isGuideColor, readableOn } from '@/lib/palette';
import { selectActiveDevice, useStore } from '@/store';
import { copyLayerSettings, deleteLayerWithObjects, lockedInLayer, moveLayer, openEditorFor, pasteLayerSettings, selectLayerObjects } from './actions';
import { LayerEditor } from './LayerEditor';
import { focusLayerRow, useLayerUi } from './layerUi';
import { buildExecution, localOrderNote, type ExecEntry } from './order';
import { RowMenu, type MenuItem } from './RowMenu';

const MODE_ICON = { line: Spline, fill: PaintBucket, image: ImageIcon } as const;

/** Conteos por capa como valores primitivos: el panel solo se repinta si cambian de verdad. */
function useLayerCounts(): Map<string, LayerCount> {
  const layers = useStore((s) => s.doc.layers);
  const flat = useStore(
    useShallow((s) => {
      const m = countByLayer(s.doc);
      return s.doc.layers.flatMap((l) => {
        const c = m.get(l.color.toLowerCase());
        return [c?.total ?? 0, c?.live ?? 0];
      });
    }),
  );
  return useMemo(() => new Map(layers.map((l, i) => [l.color.toLowerCase(), { total: flat[2 * i] ?? 0, live: flat[2 * i + 1] ?? 0 }])), [layers, flat]);
}

/** Resumen de velocidad, potencia y pasadas de una capa para su fila. */
function summary(e: ExecEntry): string {
  const l = e.layer;
  const power = l.mode === 'image' && l.min_power > 0 ? `${l.min_power}–${l.power} %` : `${l.power} %`;
  const parts = [`${l.speed} mm/min`, power];
  if (l.mode !== 'line') parts.push(`${l.interval} mm`);
  if (l.passes > 1) parts.push(`×${l.passes}`);
  return parts.join(' · ');
}

interface RowProps {
  entry: ExecEntry;
  active: boolean;
  canMoveUp: boolean;
  canMoveDown: boolean;
  dropMark: 'before' | 'after' | null;
  dragging: boolean;
  onGripDown: (e: ReactPointerEvent) => void;
}

function LayerRow({ entry, active, canMoveUp, canMoveDown, dropMark, dragging, onGripDown }: RowProps) {
  const { layer } = entry;
  const guide = isGuideColor(layer.color);
  const renaming = useLayerUi((s) => s.renaming === layer.color);
  const [menu, setMenu] = useState<DOMRect | null>(null);
  const menuBtn = useRef<HTMLButtonElement>(null);
  const Icon = MODE_ICON[layer.mode];
  const hasZ = useStore((s) => selectActiveDevice(s).z_axis === true);
  const copied = useLayerUi((s) => s.copied);
  const lockedCount = useStore((s) => lockedInLayer(s.doc, layer.color));
  const features = layerFeatures(layer, hasZ);
  const subCount = guide ? 0 : (layer.sub_layers?.length ?? 0);
  const pinned = layer.order !== null;
  const name = layer.name || colorName(layer.color);

  const openMenu = (rect?: DOMRect) => setMenu(rect ?? menuBtn.current?.getBoundingClientRect() ?? new DOMRect());
  const closeMenu = (restoreFocus: boolean) => {
    setMenu(null);
    if (restoreFocus) menuBtn.current?.focus();
  };

  const st = () => useStore.getState();
  const move = (delta: number) => {
    if (entry.position === null) return;
    moveLayer(st(), layer.color, entry.position - 1 + delta);
    focusLayerRow(layer.color);
  };

  const items: MenuItem[] = [
    { id: 'edit', label: t('layers.menu.edit'), icon: <Settings2 size={14} />, hint: 'Intro', onSelect: () => openEditorFor(st(), layer.color) },
    {
      id: 'select',
      label: t('layers.menu.select'),
      icon: <MousePointerClick size={14} />,
      disabledReason: entry.count.total === 0 ? t('layers.reason.noObjects') : undefined,
      onSelect: () => selectLayerObjects(st(), layer.color),
    },
    {
      id: 'copy',
      label: t('layers.menu.copy'),
      icon: <Copy size={14} />,
      disabledReason: guide ? t('layers.reason.guideNoSettings') : undefined,
      onSelect: () => copyLayerSettings(st(), layer.color),
    },
    {
      id: 'paste',
      label: t('layers.menu.paste'),
      icon: <ClipboardPaste size={14} />,
      disabledReason: guide ? t('layers.reason.guideNoSettings') : !copied ? t('layers.reason.nothingCopied') : undefined,
      onSelect: () => pasteLayerSettings(st(), layer.color),
    },
    { id: 'rename', label: t('layers.menu.rename'), icon: <Pencil size={14} />, hint: 'F2', onSelect: () => useLayerUi.getState().startRename(layer.color) },
    {
      id: 'up',
      label: t('layers.menu.up'),
      icon: <ArrowUp size={14} />,
      disabledReason: !entry.runs ? t('layers.reason.notRunning') : !canMoveUp ? t('layers.reason.alreadyFirst') : undefined,
      hint: 'Alt+↑',
      onSelect: () => move(-1),
    },
    {
      id: 'down',
      label: t('layers.menu.down'),
      icon: <ArrowDown size={14} />,
      disabledReason: !entry.runs ? t('layers.reason.notRunning') : !canMoveDown ? t('layers.reason.alreadyLast') : undefined,
      hint: 'Alt+↓',
      onSelect: () => move(1),
    },
    pinned
      ? { id: 'unpin', label: t('layers.menu.unpin'), icon: <PinOff size={14} />, onSelect: () => st().updateLayer(layer.color, { order: null }) }
      : {
          id: 'pin',
          label: t('layers.menu.pin'),
          icon: <Pin size={14} />,
          disabledReason: !entry.runs ? t('layers.reason.notRunning') : undefined,
          onSelect: () => st().updateLayer(layer.color, { order: entry.position }),
        },
    {
      id: 'delete',
      label: t('layers.menu.delete'),
      icon: <Trash2 size={14} />,
      disabledReason: lockedCount > 0 ? t('layers.reason.lockedObjects') : undefined,
      onSelect: () => void deleteLayerWithObjects(useStore.getState, layer.color),
    },
  ];

  const tag = entry.skip
    ? t(`layers.skip.${entry.skip}`)
    : t(`layers.role.${entry.role}`);
  const tagWhy = entry.skip
    ? t(`layers.skip.${entry.skip}.why`)
    : entry.reason
      ? t('layers.role.cut.why', { why: t(`layers.cutReason.${entry.reason}`) })
      : t(`layers.role.${entry.role}.why`);

  return (
    // El clic derecho es un atajo para ratón: con teclado el mismo menú se abre con el botón de la fila.
    // eslint-disable-next-line jsx-a11y/no-noninteractive-element-interactions
    <li
      className="lyr-row"
      data-active={active}
      data-hidden={!layer.visible}
      data-guide={guide}
      data-run={entry.runs}
      data-dragging={dragging}
      data-drop={dropMark ?? undefined}
      data-layer={layer.color}
      onContextMenu={(e) => {
        e.preventDefault();
        openMenu(new DOMRect(e.clientX, e.clientY, 0, 0));
      }}
    >
      <button
        type="button"
        className="lyr-grip"
        aria-label={entry.runs ? t('layers.drag') : t('layers.reason.notRunning')}
        aria-disabled={!entry.runs}
        title={entry.runs ? t('layers.dragHint') : t('layers.reason.notRunning')}
        tabIndex={-1}
        onPointerDown={entry.runs ? onGripDown : undefined}
      >
        <GripVertical size={14} aria-hidden="true" />
      </button>
      <span className="lyr-pos num" title={entry.position !== null ? t('layers.position', { n: entry.position }) : t('layers.reason.notRunning')}>
        {entry.position ?? '–'}
        {pinned && <Pin size={9} aria-label={t('layers.pinned', { n: layer.order ?? 0 })} />}
      </span>
      {renaming ? (
        <div className="lyr-main lyr-main--edit">
          <Swatch color={layer.color} guide={guide} />
          <RenameInput color={layer.color} value={layer.name} />
        </div>
      ) : (
        <button
          type="button"
          className="lyr-main"
          data-layer-main={layer.color}
          aria-current={active ? 'true' : undefined}
          title={t('layers.rowHint')}
          onClick={() => st().setActiveLayer(layer.color)}
          onDoubleClick={() => openEditorFor(st(), layer.color)}
          onKeyDown={(e) => {
            if (e.key === 'Enter' && !e.altKey && !e.ctrlKey && !e.metaKey) {
              e.preventDefault();
              openEditorFor(st(), layer.color);
            }
          }}
        >
          <Swatch color={layer.color} guide={guide} />
          <span className="lyr-text">
            <span className="lyr-name">
              <span className="lyr-name__text">{name}</span>
              <span className="lyr-count num" title={tn('layers.objects', entry.count.total)}>
                {entry.count.total}
                <span className="sr-only"> {t('layers.objectsShort')}</span>
              </span>
            </span>
            <span className="lyr-meta num" title={guide ? undefined : t(`layers.mode.${layer.mode}`)}>
              {guide ? (
                t('layers.guideMeta')
              ) : (
                <>
                  <Icon size={11} aria-hidden="true" />
                  <span className="sr-only">{t(`layers.mode.${layer.mode}`)}: </span>
                  {summary(entry)}
                  {features.map((f) => (
                    <span key={f} className="lyr-feature" title={t(`layers.feature.${f}.why`)}>
                      {t(`layers.feature.${f}`)}
                    </span>
                  ))}
                  {subCount > 0 && (
                    <span className="lyr-feature" title={t('layers.sub.badge.why', { n: subCount })}>
                      {t('layers.sub.badge', { n: subCount })}
                    </span>
                  )}
                </>
              )}
            </span>
            <span className="lyr-tag" data-kind={entry.skip ? 'skip' : entry.role} title={tagWhy}>
              {tag}
            </span>
          </span>
        </button>
      )}
      <button
        type="button"
        className="icon-btn"
        aria-pressed={layer.visible}
        aria-label={layer.visible ? t('layers.hide') : t('layers.show')}
        title={layer.visible ? t('layers.hide') : t('layers.show')}
        onClick={() => st().updateLayer(layer.color, { visible: !layer.visible })}
      >
        {layer.visible ? <Eye size={15} aria-hidden="true" /> : <EyeOff size={15} aria-hidden="true" />}
      </button>
      <button
        type="button"
        className="icon-btn"
        aria-pressed={layer.output && !guide}
        aria-disabled={guide}
        aria-label={guide ? t('layers.outputGuide') : layer.output ? t('layers.outputOff') : t('layers.outputOn')}
        title={guide ? t('layers.outputGuide') : layer.output ? t('layers.outputOff') : t('layers.outputOn')}
        onClick={() => !guide && st().updateLayer(layer.color, { output: !layer.output })}
      >
        {layer.output && !guide ? <Zap size={15} aria-hidden="true" /> : <ZapOff size={15} aria-hidden="true" />}
      </button>
      <button
        ref={menuBtn}
        type="button"
        className="icon-btn"
        aria-haspopup="menu"
        aria-expanded={menu !== null}
        aria-label={t('layers.menu.open', { name })}
        title={t('layers.menu.open', { name })}
        onClick={() => (menu ? closeMenu(true) : openMenu())}
      >
        <Ellipsis size={15} aria-hidden="true" />
      </button>
      {menu && <RowMenu items={items} anchor={menu} label={t('layers.menu.label', { name })} onClose={closeMenu} />}
    </li>
  );
}

function Swatch({ color, guide }: { color: string; guide: boolean }) {
  return (
    <span className="lyr-swatch" data-guide={guide} style={{ '--sw': color, '--ink': readableOn(color) } as React.CSSProperties} aria-hidden="true">
      {guide && <SquareDashed size={14} />}
    </span>
  );
}

function RenameInput({ color, value }: { color: string; value: string }) {
  const [text, setText] = useState(value);
  const ref = useRef<HTMLInputElement>(null);
  useEffect(() => {
    ref.current?.focus();
    ref.current?.select();
  }, []);
  const finish = (save: boolean) => {
    const name = text.trim();
    if (save && name !== '' && name !== value) useStore.getState().updateLayer(color, { name });
    useLayerUi.getState().stopRename();
    focusLayerRow(color);
  };
  const empty = text.trim() === '';
  return (
    <input
      ref={ref}
      className="input lyr-rename"
      value={text}
      maxLength={60}
      aria-label={t('layers.rename')}
      aria-invalid={empty}
      onChange={(e) => setText(e.target.value)}
      onBlur={() => finish(!empty)}
      onKeyDown={(e) => {
        e.stopPropagation();
        if (e.key === 'Enter') {
          if (!empty) finish(true);
        } else if (e.key === 'Escape') finish(false);
      }}
    />
  );
}

/** Orden de ejecución: la propia lista de capas, de arriba (primero) a abajo (último). */
export function LayersPanel() {
  const layers = useStore((s) => s.doc.layers);
  const activeLayer = useStore((s) => s.activeLayer);
  const mode = useStore((s) => s.jobOptions.layer_order);
  const engineNote = useStore((s) => (s.previewStatus === 'ready' ? (s.preview?.order_note ?? null) : null));
  const editing = useLayerUi((s) => s.editing);
  const counts = useLayerCounts();
  const listRef = useRef<HTMLUListElement>(null);
  const [drag, setDrag] = useState<{ color: string; from: number; to: number } | null>(null);

  const entries = useMemo(() => buildExecution(layers, counts, mode), [layers, counts, mode]);
  const running = entries.filter((e) => e.runs).length;
  const note = engineNote ?? localOrderNote(entries, mode);
  const editor = editing ? layers.find((l) => l.color === editing) : undefined;

  // Arrastrar con el puntero: se sigue en la ventana para no perder el gesto al salir de la fila.
  useEffect(() => {
    if (!drag) return;
    const rowsY = () => [...(listRef.current?.querySelectorAll<HTMLElement>('li[data-run="true"]') ?? [])].map((el) => el.getBoundingClientRect());
    const onMove = (e: PointerEvent) => {
      const rects = rowsY();
      let to = rects.length - 1;
      for (let i = 0; i < rects.length; i++) {
        if (e.clientY < rects[i]!.top + rects[i]!.height / 2) {
          to = i;
          break;
        }
      }
      setDrag((d) => (d && d.to !== to ? { ...d, to } : d));
    };
    const onUp = () => {
      if (drag.to !== drag.from) moveLayer(useStore.getState(), drag.color, drag.to);
      setDrag(null);
      focusLayerRow(drag.color);
    };
    const onKey = (e: globalThis.KeyboardEvent) => {
      if (e.key === 'Escape') setDrag(null);
    };
    window.addEventListener('pointermove', onMove);
    window.addEventListener('pointerup', onUp, { once: true });
    window.addEventListener('keydown', onKey);
    return () => {
      window.removeEventListener('pointermove', onMove);
      window.removeEventListener('pointerup', onUp);
      window.removeEventListener('keydown', onKey);
    };
  }, [drag]);

  const onListKey = (e: KeyboardEvent<HTMLUListElement>) => {
    const target = e.target as HTMLElement;
    const color = target.dataset.layerMain;
    if (color === undefined) return;
    const mains = [...(listRef.current?.querySelectorAll<HTMLElement>('[data-layer-main]') ?? [])];
    const i = mains.indexOf(target);
    if (e.altKey && (e.key === 'ArrowUp' || e.key === 'ArrowDown')) {
      e.preventDefault(); // Alt+flechas mueven la selección en el lienzo: aquí reordenan la capa
      const entry = entries.find((x) => x.layer.color === color);
      if (!entry || entry.position === null) return;
      moveLayer(useStore.getState(), color, entry.position - 1 + (e.key === 'ArrowUp' ? -1 : 1));
      focusLayerRow(color);
    } else if (e.key === 'ArrowDown' || e.key === 'ArrowUp' || e.key === 'Home' || e.key === 'End') {
      e.preventDefault();
      const n = e.key === 'Home' ? 0 : e.key === 'End' ? mains.length - 1 : Math.min(mains.length - 1, Math.max(0, i + (e.key === 'ArrowDown' ? 1 : -1)));
      mains[n]?.focus();
    } else if (e.key === 'F2') {
      e.preventDefault();
      useLayerUi.getState().startRename(color);
    }
  };

  if (editor) return <LayerEditor layer={editor} />;

  return (
    <div className="panel-scroll">
      {layers.length === 0 ? (
        <p className="panel-empty">{t('layers.empty')}</p>
      ) : (
        <>
          <div className="lyr-head">
            <div className="lyr-head__row">
              <span id="lyr-order-label" className="lyr-head__title">
                <SlidersHorizontal size={12} aria-hidden="true" /> {t('layers.order.title')}
              </span>
              <OrderModeToggle mode={mode} />
            </div>
            <p className="lyr-note" aria-live="polite">
              {note ?? t('layers.order.note.single')}
            </p>
          </div>
          <ul
            ref={listRef}
            className="lyr-list"
            aria-label={t('layers.list', { n: running })}
            data-dragging={drag !== null}
            onKeyDown={onListKey}
          >
            {entries.map((entry) => {
              const idx = entry.position === null ? -1 : entry.position - 1;
              const mark = drag && idx === drag.to && drag.to !== drag.from ? (drag.to > drag.from ? 'after' : 'before') : null;
              return (
                <LayerRow
                  key={entry.layer.color}
                  entry={entry}
                  active={entry.layer.color.toLowerCase() === activeLayer?.toLowerCase()}
                  canMoveUp={idx > 0}
                  canMoveDown={idx >= 0 && idx < running - 1}
                  dropMark={mark}
                  dragging={drag?.color === entry.layer.color}
                  onGripDown={(e) => {
                    e.preventDefault();
                    setDrag({ color: entry.layer.color, from: idx, to: idx });
                  }}
                />
              );
            })}
          </ul>
          <p className="lyr-foot">{t('layers.keysHint')}</p>
        </>
      )}
    </div>
  );
}

function OrderModeToggle({ mode }: { mode: LayerOrder }) {
  const setJobOptions = useStore((s) => s.setJobOptions);
  const opts: LayerOrder[] = ['auto', 'file'];
  return (
    <div className="lyr-seg lyr-seg--sm" role="radiogroup" aria-labelledby="lyr-order-label">
      {opts.map((o) => (
        <label key={o} title={t(`layers.order.mode.${o}.why`)}>
          <input type="radio" name="layer-order" checked={mode === o} onChange={() => setJobOptions({ layer_order: o })} />
          <span>{t(`layers.order.mode.${o}`)}</span>
        </label>
      ))}
    </div>
  );
}
