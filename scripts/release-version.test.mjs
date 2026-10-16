// Pruebas de scripts/release-version.mjs: `node --test scripts/*.test.mjs`
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import { botonesReadme, cask, sustituir } from './release-version.mjs';

const leer = (rel) => readFileSync(new URL(`../${rel}`, import.meta.url), 'utf8');

test('los botones de los dos README cambian a la versión nueva', () => {
  for (const f of ['README.md', 'README.es.md']) {
    const t = botonesReadme(leer(f), '9.8.7', f);
    assert.equal([...t.matchAll(/releases\/download\/v9\.8\.7\/Tlaku_9\.8\.7_/g)].length, 4);
    assert.doesNotMatch(t, /releases\/download\/v0\.\d+\.\d+\//);
  }
});

test('el cask cambia versión y SHA256 y conserva lo demás', () => {
  const sha = 'a'.repeat(64);
  const t = cask(leer('packaging/homebrew/Casks/tlaku.rb'), '9.8.7', sha);
  assert.match(t, /^ {2}version "9\.8\.7"$/m);
  assert.match(t, new RegExp(`^ {2}sha256 "${sha}"$`, 'm'));
  assert.match(t, /Tlaku_#\{version\}_universal\.dmg/);
});

test('sustituir falla si el texto no tiene lo esperado', () => {
  assert.throws(() => sustituir('nada', /version "x"/, 'y', 'prueba'), /esperaba 1/);
  assert.throws(() => botonesReadme('sin botones', '1.0.0', 'README'), /esperaba 4/);
});
