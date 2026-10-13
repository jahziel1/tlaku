import { renderToString } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { Modal } from './Modal';

describe('Modal común', () => {
  it('pinta un diálogo accesible con las clases y los data-* de su área', () => {
    const html = renderToString(
      <Modal labelId="t1" onClose={() => {}} backdropClassName="bk" cardClassName="card" cardData={{ 'data-size': 'sm', 'data-tone': undefined }}>
        <h2 id="t1">Hola</h2>
      </Modal>,
    );
    expect(html).toContain('role="dialog"');
    expect(html).toContain('aria-modal="true"');
    expect(html).toContain('aria-labelledby="t1"');
    expect(html).toContain('class="bk"');
    expect(html).toContain('data-size="sm"');
    expect(html).not.toContain('data-tone');
    expect(html).toContain('tabindex="-1"');
  });
});
