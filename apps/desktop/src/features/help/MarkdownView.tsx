import type { ReactNode } from 'react';
import { t } from '@/i18n';
import { CopyButton } from './CopyButton';
import { classifyLink, type Block, type Inline } from './markdown';
import { resolveImage } from './pages';

export interface MarkdownActions {
  /** ¿Existe esa página del manual? Un enlace a una que no existe se muestra como texto. */
  hasPage: (file: string) => boolean;
  onPage: (file: string, anchor: string) => void;
  onAnchor: (anchor: string) => void;
}

const NO_ACTIONS: MarkdownActions = { hasPage: () => false, onPage: () => {}, onAnchor: () => {} };

function Inlines({ nodes, actions }: { nodes: Inline[]; actions: MarkdownActions }) {
  return (
    <>
      {nodes.map((n, i) => (
        <InlineNode key={i} node={n} actions={actions} />
      ))}
    </>
  );
}

function InlineNode({ node, actions }: { node: Inline; actions: MarkdownActions }): ReactNode {
  switch (node.t) {
    case 'text':
      return node.v;
    case 'code':
      return <code>{node.v}</code>;
    case 'strong':
      return <strong><Inlines nodes={node.c} actions={actions} /></strong>;
    case 'em':
      return <em><Inlines nodes={node.c} actions={actions} /></em>;
    case 'image':
      return <Figure alt={node.alt} src={node.src} inline />;
    case 'link': {
      const target = classifyLink(node.href);
      const body = <Inlines nodes={node.c} actions={actions} />;
      if (target.kind === 'page' && actions.hasPage(target.file)) {
        return (
          <a
            href={`#${target.file}`}
            className="md__link"
            onClick={(e) => {
              e.preventDefault();
              actions.onPage(target.file, target.anchor);
            }}
          >
            {body}
          </a>
        );
      }
      if (target.kind === 'anchor') {
        return (
          <a
            href={`#${target.anchor}`}
            className="md__link"
            onClick={(e) => {
              e.preventDefault();
              actions.onAnchor(target.anchor);
            }}
          >
            {body}
          </a>
        );
      }
      if (target.kind === 'external') {
        // Un <a href> externo navegaría la ventana de la app fuera de ella: se muestra como texto copiable.
        return (
          <span className="md__ext">
            {body} <code className="md__url">{target.url}</code> <CopyButton text={target.url} label={t('help.copyLink')} />
          </span>
        );
      }
      return body;
    }
  }
}

function Figure({ alt, src, inline }: { alt: string; src: string; inline?: boolean }) {
  const url = resolveImage(src);
  if (!url) return <span className="md__missing">{t('help.manual.imageMissing', { alt })}</span>;
  const img = <img className="md__img" src={url} alt={alt} loading="lazy" />;
  if (inline) return img;
  return (
    <figure className="md__figure">
      {img}
      {alt && <figcaption>{alt}</figcaption>}
    </figure>
  );
}

function Blocks({ blocks, actions }: { blocks: Block[]; actions: MarkdownActions }) {
  return (
    <>
      {blocks.map((b, i) => (
        <BlockNode key={i} block={b} actions={actions} />
      ))}
    </>
  );
}

function BlockNode({ block, actions }: { block: Block; actions: MarkdownActions }): ReactNode {
  switch (block.t) {
    case 'heading': {
      const Tag = `h${Math.min(block.level + 2, 6)}` as 'h3' | 'h4' | 'h5' | 'h6';
      return (
        <Tag id={`md-${block.id}`} className={`md__h md__h${Math.min(block.level, 4)}`}>
          <Inlines nodes={block.c} actions={actions} />
        </Tag>
      );
    }
    case 'p':
      return <p><Inlines nodes={block.c} actions={actions} /></p>;
    case 'image':
      return <Figure alt={block.alt} src={block.src} />;
    case 'hr':
      return <hr className="md__hr" />;
    case 'code':
      return <pre className="md__pre"><code>{block.v}</code></pre>;
    case 'quote':
      return <blockquote className="md__quote"><Blocks blocks={block.c} actions={actions} /></blockquote>;
    case 'list': {
      const Tag = block.ordered ? 'ol' : 'ul';
      return (
        <Tag className="md__list">
          {block.items.map((it, i) => (
            <li key={i}>
              <Inlines nodes={it.c} actions={actions} />
              <Blocks blocks={it.sub} actions={actions} />
            </li>
          ))}
        </Tag>
      );
    }
    case 'table':
      return (
        <div className="md__tablewrap">
          <table className="md__table">
            <thead>
              <tr>
                {block.head.map((cell, i) => (
                  <th key={i} style={block.align[i] ? { textAlign: block.align[i]! } : undefined} scope="col">
                    <Inlines nodes={cell} actions={actions} />
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {block.rows.map((row, r) => (
                <tr key={r}>
                  {row.map((cell, i) => (
                    <td key={i} style={block.align[i] ? { textAlign: block.align[i]! } : undefined}>
                      <Inlines nodes={cell} actions={actions} />
                    </td>
                  ))}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      );
  }
}

/** Pinta un documento Markdown ya interpretado. Sin HTML crudo: todo sale de elementos de React. */
export function MarkdownView({ blocks, actions = NO_ACTIONS }: { blocks: Block[]; actions?: MarkdownActions }) {
  return (
    <div className="md">
      <Blocks blocks={blocks} actions={actions} />
    </div>
  );
}
