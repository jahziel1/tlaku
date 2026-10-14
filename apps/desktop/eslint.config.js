import js from '@eslint/js';
import jsxA11y from 'eslint-plugin-jsx-a11y';
import reactHooks from 'eslint-plugin-react-hooks';
import globals from 'globals';
import tseslint from 'typescript-eslint';

export default tseslint.config(
  { ignores: ['dist', 'node_modules', 'public', 'src-tauri/target', 'src-tauri/gen', 'src/bindings'] },
  js.configs.recommended,
  ...tseslint.configs.recommended,
  {
    files: ['**/*.{ts,tsx}'],
    languageOptions: { ecmaVersion: 2023, globals: { ...globals.browser } },
    plugins: { 'react-hooks': reactHooks, 'jsx-a11y': jsxA11y },
    rules: {
      ...jsxA11y.flatConfigs.recommended.rules,
      // Accesibilidad: contenedores (diálogos, grupos, listas) que reciben por burbujeo las teclas de sus
      // hijos enfocables (flechas, Esc): es el patrón de teclado compuesto de WAI-ARIA, no un control falso.
      // Los clics y el ratón en elementos no interactivos siguen prohibidos.
      'jsx-a11y/no-noninteractive-element-interactions': ['error', {
        body: ['onError', 'onLoad'],
        iframe: ['onError', 'onLoad'],
        img: ['onError', 'onLoad'],
        ...Object.fromEntries(
          ['div', 'section', 'aside', 'header', 'form', 'fieldset', 'nav', 'ul', 'ol', 'dialog'].map((el) => [el, ['onKeyDown', 'onKeyUp', 'onKeyPress']]),
        ),
      }],
      // Las zonas con desplazamiento deben alcanzarse con el teclado (WCAG 2.1.1): región, registro y paneles de pestañas.
      'jsx-a11y/no-noninteractive-tabindex': ['error', { tags: [], roles: ['tabpanel', 'region', 'log'], allowExpressionValues: true }],
      // Las etiquetas envuelven el control y su texto a varios niveles (`label > span > strong`).
      'jsx-a11y/label-has-associated-control': ['error', { assert: 'either', depth: 5 }],
      // Solo en elementos del DOM: un componente propio puede recibir `autoFocus` para pasarlo a su <input>.
      'jsx-a11y/no-autofocus': ['error', { ignoreNonDOM: true }],
      'react-hooks/rules-of-hooks': 'error',
      'react-hooks/exhaustive-deps': 'warn',
      '@typescript-eslint/consistent-type-imports': ['error', { prefer: 'type-imports' }],
      '@typescript-eslint/no-unused-vars': ['error', { argsIgnorePattern: '^_', varsIgnorePattern: '^_' }],
      '@typescript-eslint/no-explicit-any': 'error',
      eqeqeq: ['error', 'always', { null: 'ignore' }],
      'no-console': ['warn', { allow: ['warn', 'error'] }],
      // Todo texto visible sale de src/i18n; las cadenas literales en JSX se revisan a mano.
    },
  },
  {
    files: ['*.config.{js,ts}', 'vite.config.ts', 'scripts/**/*.mjs'],
    languageOptions: { globals: { ...globals.node } },
  },
);
