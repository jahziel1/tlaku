import { renderToStaticMarkup } from 'react-dom/server';
import { beforeEach, describe, expect, it } from 'vitest';
import { commands, ALL_COMMANDS, MENUS } from '@/commands';
import { buildMenus } from '@/features/shell/menuModel';
import { es } from '@/i18n';
import { AboutDialog, DonateDialog, HelpDialogHost, ManualReader, ReportDialog } from './HelpDialogs';
import { useHelpUi } from './helpStore';
import { classifyLink, type Block, type Inline } from './markdown';
import { MarkdownView } from './MarkdownView';
import { findPage, indexPages, INDEX_FILE, manualPages, NEWS_FILE, resolveImage, thirdPartyBlocks } from './pages';
import { systemInfo } from './clipboard';
import { PROJECT_LINKS } from './links';

const HELP_IDS = ['help.manual', 'help.shortcuts', 'help.news', 'help.jobLog', 'help.report', 'help.donate', 'help.about'];

describe('menú Ayuda', () => {
  it('todos los comandos de Ayuda existen y el menú los muestra en orden', () => {
    for (const id of HELP_IDS) expect(commands.has(id), id).toBe(true);
    expect(ALL_COMMANDS.filter((c) => c.id.startsWith('help.')).map((c) => c.id).sort()).toEqual([...HELP_IDS].sort());
    const help = buildMenus(commands, MENUS).at(-1)!;
    expect(help.id).toBe('help');
    expect(help.items.flatMap((i) => (i.kind === 'command' ? [i.id] : []))).toEqual(HELP_IDS);
  });
});

describe('diálogos de ayuda', () => {
  beforeEach(() => useHelpUi.getState().close());

  it('sin diálogo abierto no pinta nada', () => {
    expect(renderToStaticMarkup(<HelpDialogHost />)).toBe('');
  });

  it('cada comando deja abierto su diálogo', async () => {
    const expected: Record<string, string> = {
      'help.manual': 'manual',
      'help.news': 'news',
      'help.jobLog': 'jobLog',
      'help.report': 'report',
      'help.donate': 'donate',
      'help.about': 'about',
    };
    for (const [id, dialog] of Object.entries(expected)) {
      useHelpUi.getState().close();
      await commands.run(id);
      expect(useHelpUi.getState().dialog, id).toBe(dialog);
    }
  });

  // El render en servidor ve el estado inicial del store: se pintan los diálogos sueltos.
  it('los diálogos llevan su título', () => {
    expect(renderToStaticMarkup(<ManualReader title={es['help.manual.title']} titleId="m" />)).toContain(es['help.manual.title']);
    expect(renderToStaticMarkup(<ManualReader title={es['help.news.title']} titleId="n" file={NEWS_FILE} single />)).toContain('Novedades');
    expect(renderToStaticMarkup(<ReportDialog />)).toContain(es['help.report.title']);
    expect(renderToStaticMarkup(<DonateDialog />)).toContain(es['help.donate.title']);
  });

  it('Acerca de dice la licencia, el significado del nombre y los créditos', () => {
    const html = renderToStaticMarkup(<AboutDialog />);
    expect(html).toContain('MIT');
    expect(html).toContain('tlacuilo');
    expect(html).toContain('MeerK40t'); // de THIRD_PARTY.md
    expect(thirdPartyBlocks().length).toBeGreaterThan(0);
  });

  it('reportar y donar no inventan enlaces: si faltan, lo dicen', () => {
    if (PROJECT_LINKS.issues === null) expect(renderToStaticMarkup(<ReportDialog />)).toContain(es['help.linkPending']);
    if (PROJECT_LINKS.donate === null) expect(renderToStaticMarkup(<DonateDialog />)).toContain(es['help.linkPending']);
    expect(systemInfo('1.2.3', { platform: 'MacIntel', userAgent: 'UA' })).toBe('Tlaku 1.2.3\nSistema: MacIntel\nWebview: UA');
    expect(systemInfo(null)).toContain('versión desconocida');
  });

  it('el manual abre en el índice y se puede abrir en otra página', () => {
    useHelpUi.getState().openManual();
    expect(useHelpUi.getState().page).toBe(INDEX_FILE);
    const second = indexPages()[1];
    useHelpUi.getState().openManual(second.file, 'x');
    expect(useHelpUi.getState()).toMatchObject({ dialog: 'manual', page: second.file, anchor: 'x' });
  });
});

describe('visor', () => {
  it('no deja pasar HTML y muestra los enlaces externos como texto copiable', () => {
    const blocks: Block[] = [
      { t: 'p', c: [{ t: 'text', v: '<img src=x onerror=alert(1)>' }, { t: 'link', href: 'https://example.org', c: [{ t: 'text', v: 'sitio' }] }] },
    ];
    const html = renderToStaticMarkup(<MarkdownView blocks={blocks} />);
    expect(html).not.toContain('<img');
    expect(html).toContain('&lt;img');
    expect(html).not.toContain('href="https://example.org"');
    expect(html).toContain('https://example.org');
  });
});

/** El manual real: lo que se publica en docs/manual/ tiene que funcionar dentro de la app. */
describe('manual incluido', () => {
  const pages = manualPages();
  const walkInline = (nodes: Inline[], out: Inline[]) => {
    for (const n of nodes) {
      out.push(n);
      if (n.t === 'strong' || n.t === 'em' || n.t === 'link') walkInline(n.c, out);
    }
  };
  const inlinesOf = (blocks: Block[]): Inline[] => {
    const out: Inline[] = [];
    for (const b of blocks) {
      if (b.t === 'heading' || b.t === 'p') walkInline(b.c, out);
      else if (b.t === 'image') out.push({ t: 'image', alt: b.alt, src: b.src });
      else if (b.t === 'quote') out.push(...inlinesOf(b.c));
      else if (b.t === 'list') for (const it of b.items) { walkInline(it.c, out); out.push(...inlinesOf(it.sub)); }
      else if (b.t === 'table') for (const row of [b.head, ...b.rows]) for (const cell of row) walkInline(cell, out);
    }
    return out;
  };

  it('hay índice, páginas con título y un solo h1 cada una', () => {
    expect(findPage(INDEX_FILE)).toBeDefined();
    expect(pages.length).toBeGreaterThanOrEqual(17);
    for (const p of pages) {
      expect(p.blocks.filter((b) => b.t === 'heading' && b.level === 1), p.file).toHaveLength(1);
      expect(p.title.length, p.file).toBeGreaterThan(2);
    }
  });

  it('todos los enlaces internos y las capturas existen', () => {
    for (const p of pages) {
      for (const n of inlinesOf(p.blocks)) {
        if (n.t === 'image') expect(resolveImage(n.src), `${p.file}: ${n.src}`).not.toBeNull();
        if (n.t !== 'link') continue;
        const target = classifyLink(n.href);
        if (target.kind === 'page') expect(findPage(target.file), `${p.file}: ${n.href}`).toBeDefined();
        if (target.kind === 'none') throw new Error(`${p.file}: enlace que la app no sabe abrir: ${n.href}`);
      }
    }
  });

  it('las secciones a las que se enlaza existen', () => {
    for (const p of pages) {
      for (const n of inlinesOf(p.blocks)) {
        if (n.t !== 'link') continue;
        const target = classifyLink(n.href);
        if (target.kind !== 'page' || !target.anchor) continue;
        const dest = findPage(target.file)!;
        const ids = dest.blocks.flatMap((b) => (b.t === 'heading' ? [b.id] : []));
        expect(ids, `${p.file}: ${n.href}`).toContain(target.anchor);
      }
    }
  });

  it('el manual completo se dibuja sin errores', () => {
    for (const p of pages) expect(renderToStaticMarkup(<MarkdownView blocks={p.blocks} />).length, p.file).toBeGreaterThan(50);
  });
});
