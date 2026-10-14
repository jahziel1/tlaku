import { useStore } from '@/store';
import { useInteraction } from './interaction';
import { TIP_H, TIP_W } from './tipPlacement';

/** Medidas en vivo junto al cursor mientras se arrastra o se dibuja. */
export function CanvasTip() {
  const tip = useInteraction((s) => s.tip);
  const width = useStore((s) => s.viewport.width);
  const height = useStore((s) => s.viewport.height);
  if (!tip) return null;
  const flipX = tip.x + TIP_W > width;
  const flipY = tip.y + TIP_H > height;
  return (
    <div
      className="canvas-tip num"
      style={{
        left: tip.x,
        top: tip.y,
        transform: `translate(${flipX ? 'calc(-100% - 32px)' : '0'}, ${flipY ? 'calc(-100% - 36px)' : '0'})`,
      }}
      aria-hidden="true"
    >
      {tip.text}
    </div>
  );
}
