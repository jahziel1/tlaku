import { useEffect } from 'react';
import { useStore } from '@/store';
import { BatchDialogHost } from './BatchDialog';
import { BitmapDialogHost } from './BitmapDialog';
import { TraceDialog } from './TraceDialog';
import { useTraceDialog } from './traceStore';

/**
 * Dibuja el diálogo "Vectorizar imagen" cuando hay una imagen abierta en `traceStore`, y también
 * "Vectorizar varios archivos" y "Convertir a mapa de bits". Lo monta el shell una sola vez. Si la
 * imagen desaparece del documento (deshacer, borrar), se cierra.
 */
export function TraceDialogHost() {
  const imageId = useTraceDialog((s) => s.imageId);
  const close = useTraceDialog((s) => s.close);
  const obj = useStore((s) => (imageId ? s.doc.objects.find((o) => o.id === imageId) : undefined));
  const image = obj?.kind.type === 'image' ? obj.kind : null;

  useEffect(() => {
    if (imageId && !image) close();
  }, [imageId, image, close]);

  return (
    <>
      {imageId && image && <TraceDialog key={imageId} imageId={imageId} image={image} />}
      <BatchDialogHost />
      <BitmapDialogHost />
    </>
  );
}
