import { ReviewPanel } from './ReviewPanel';
import { useReviewUi } from './store';

export { ReviewNotice } from './ReviewNotice';
export { SimpleReview } from './SimpleReview';
export { closeReview, openReview, useReviewUi } from './store';

/** Punto de montaje de «Revisar diseño» (lo monta el shell una sola vez): pinta el panel si está abierto. */
export function ReviewHost() {
  const open = useReviewUi((s) => s.open);
  return open ? <ReviewPanel /> : null;
}
