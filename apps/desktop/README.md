# LaserSoft Desktop

Aplicación de escritorio de LaserSoft: **Tauri v2 + Vite + React 19 + TypeScript (strict) + zustand**.
La interfaz es dueña del documento que se edita; el motor (Rust) recibe el documento completo y
devuelve resultados. El contrato está en [`docs/app-api.md`](../../docs/app-api.md).

Estado: **oleada A** (esqueleto). La interfaz funciona completa contra una máquina y un motor
**simulados**; el crate `src-tauri` todavía no depende de los crates del motor (oleada B).

![Vista principal](docs/screenshots/01-principal-oscuro.png)

## Cómo correrla

Requisitos: Node 22.13 o superior, npm 11, Rust estable y, para la app nativa, las herramientas
de Tauri de tu sistema (en macOS, Xcode).

```bash
cd apps/desktop
npm install            # el .npmrc activa legacy-peer-deps (ver "Notas de dependencias")
```

**Modo mock (navegador, sin Rust).** Es el modo por defecto: se desarrolla la interfaz con datos
simulados (diseño de ejemplo, vista previa con movimientos, máquina que emite estado y progreso).

```bash
npm run dev            # http://localhost:1420
```

**Modo Tauri (app nativa).** Usa `invoke`/`listen` reales. Hoy solo existe el comando `app_version`;
el resto de comandos del contrato devuelven error hasta que se conecte el motor.

```bash
npm run tauri dev                              # ventana nativa con recarga en caliente
npm run tauri build -- --debug --no-bundle     # compila la app completa sin empaquetar
```

Desde la raíz del repositorio, el crate se compila con `cargo build -p lasersoft-desktop`.

La implementación de la API se elige con `VITE_API=mock|tauri` (por defecto `mock`). Los scripts
`dev:tauri` y `build:tauri` usan `--mode tauri`, que carga `.env.tauri` con `VITE_API=tauri`.
Con `VITE_DEMO=off` el modo mock arranca con la cama vacía en lugar del diseño de ejemplo.

| Script | Qué hace |
|---|---|
| `npm run dev` | Vite en modo mock |
| `npm run build` | `tsc --noEmit` y `vite build` |
| `npm run typecheck` | TypeScript estricto |
| `npm run lint` | ESLint |
| `npm test` | Vitest (registro de comandos, store, mock, viewport) |
| `npm run screenshots` | Capturas en `docs/screenshots/` (necesita `npm run dev` y Chrome; sin dependencias extra) |

## Estructura

```
apps/desktop/
  src-tauri/            Crate `lasersoft-desktop` (miembro del workspace de Cargo)
    capabilities/       Permisos mínimos de Tauri v2 (eventos y diálogos de archivo)
  src/
    api/                Contrato con el motor
      api.ts            Interfaz `LaserApi`: TODOS los comandos y eventos de docs/app-api.md
      tauri.ts          Implementación real (invoke / listen)
      mock/             Implementación simulada en memoria (+ pruebas)
      types.ts          Reexporta src/bindings/ y define lo que aún no tiene binding
      index.ts          getApi(): elige mock o tauri según VITE_API
    bindings/           Tipos generados por ts-rs desde laser-doc. NO editar a mano
    store/              zustand en slices: document, selection, view, machine, preview, devices (+ notices)
    commands/           Registro central de comandos y atajos (id, etiqueta, atajo, acción)
      definitions/      Un archivo por categoría
    i18n/               es.ts (diccionario) y t()
    lib/                Geometría, viewport (mm <-> px), paleta de capas, utilidades
    ui/                 Componentes de interfaz reutilizables (NumberField)
    styles/             Tokens CSS (tema oscuro y claro) y hojas por zona
    features/
      shell/            Layout, menús, barra superior, paleta de herramientas, panel acoplado, estado
      canvas/           Lienzo: interfaz Renderer, Canvas2D, reglas, zoom y paneo
      layers/           Lista de capas, editor rápido y franja de colores
      machine/          Conexión, estado, mover cabezal, trabajo y consola
      preview/          Vista previa del recorrido y recálculo con debounce
      history/          Deshacer/rehacer (en el slice document) y archivos: abrir, guardar, importar, exportar
  scripts/              Utilidades de desarrollo (capturas)
  docs/screenshots/     Capturas de la interfaz
```

### Reglas para quienes sigan trabajando aquí

1. **Cada carpeta de `src/features/` tiene un dueño.** Quien trabaje en `canvas` no edita
   `layers`, y así. Si necesitas algo de otra carpeta, usa su `index.ts` o pide el cambio.
2. **No escribas en `src/bindings/`**: lo genera el motor. Si falta un tipo, defínelo temporalmente
   en `src/api/types.ts` con un `TODO(bindings)`.
3. **Nada llama a `invoke` ni a `listen` directamente.** Todo pasa por `getApi()` (`src/api`). Si
   agregas un comando al contrato, agrégalo a `LaserApi`, a `COMMAND_NAMES`, a `tauri.ts` y al mock.
4. **Estado compartido en `src/store/`**, un archivo por slice. Un slice puede leer a otro con
   `get()`, pero solo escribe en el suyo (las acciones de otro slice se llaman, no se reimplementan).
   En componentes, selecciona valores primitivos o usa `useShallow`: el cursor cambia 60 veces por
   segundo y no debe repintar paneles.
5. **Comandos, menús, botones y atajos salen de `src/commands/`.** Un comando nuevo va en
   `definitions/<categoria>.ts`; el registro lanza un error al arrancar si un id o un atajo se repite.
   Los botones usan `useCommand(id)` o `<CommandButton id>`: etiqueta, atajo y motivo de
   deshabilitado salen de ahí. Un comando no disponible devuelve la clave del motivo
   (`enabled: (s) => true | 'reason.xxx'`): **nunca un botón gris sin explicación**.
6. **Todo texto visible sale de `src/i18n/es.ts`** con `t('clave')`. Las claves son tipadas.
   Los mensajes que vienen del motor (advertencias, resúmenes) ya llegan en español.
7. **Colores solo con tokens** (`var(--accent)`, ...). Nada de hex en los componentes. El lienzo
   lee los `--cv-*` con `readCanvasPalette()`. Todo control nuevo debe verse bien en los dos temas
   y tener foco visible.
8. **Cambiar de renderer** (por ejemplo a WebGL) es implementar `Renderer`
   (`features/canvas/renderer.ts`) y devolverlo desde `createRenderer()`. La escena (`Scene`)
   ya trae todo lo necesario; las reglas y la cuadrícula no dependen del renderer.
9. **Unidades:** mm, mm/min y %. Origen abajo a la izquierda y Y hacia arriba en el documento; solo
   `lib/viewport.ts` convierte a píxeles de pantalla (Y hacia abajo).
10. Antes de entregar: `npm run typecheck && npm run lint && npm test && npm run build`.
    Las pruebas del motor simulado y del store no dependen del reloj real salvo por temporizadores
    cortos; si agregas flujos con tiempo, usa las opciones de `createMockApi({ ... })`.

## Atajos de teclado

Se definen una sola vez en `src/commands/definitions/` y se muestran en menús y tooltips.
`Cmd` en macOS, `Ctrl` en Windows y Linux. Los conflictos de atajos son un error al arrancar y lo
verifica una prueba.

Reglas (respuesta a la queja del foro de atajos contradictorios y que "no hacen nada"):

- **Funcionan en toda la ventana**, no solo con el lienzo enfocado. Las teclas de una sola letra solo
  se ignoran mientras se escribe en un campo de texto.
- **Una tecla, un significado.** No hay letras que cambien de función según lo que haya bajo el cursor.
- **Cmd/Ctrl se reserva para acciones de archivo, edición y vista.** No se sobrecarga como
  modificador de pasos finos: el paso fino usa `Alt`, el grande `Mayús`.
- Un atajo de un comando deshabilitado no hace nada, y el tooltip del botón dice por qué.

| Grupo | Atajo | Comando |
|---|---|---|
| Archivo | `Mod+N` / `Mod+O` | Nuevo / Abrir proyecto |
| | `Mod+S` / `Mod+Shift+S` | Guardar / Guardar como |
| | `Mod+I` / `Mod+Shift+E` | Importar diseño / Exportar G-code |
| Edición | `Mod+Z` / `Mod+Shift+Z` o `Mod+Y` | Deshacer / Rehacer |
| | `Mod+A` / `Mod+D` | Seleccionar todo / Duplicar |
| | `Supr` o `Retroceso` | Eliminar |
| | `Mod+C` `Mod+X` `Mod+V` `Mod+G` `Mod+Shift+G` | Reservados (cortar, copiar, pegar, agrupar, desagrupar) |
| | Flechas / `Mayús`+flechas / `Alt`+flechas | Mover 1 mm / 10 mm / 0,1 mm |
| Herramientas | `V` o `Esc` / `N` | Seleccionar (`Esc` además quita la selección) / Editar nodos |
| | `L` `R` `E` `T` | Línea, rectángulo, elipse, texto |
| | `H` / `M` | Mano / Medir |
| Vista | `Mod+=` / `Mod+-` | Acercar / Alejar |
| | `Mod+0` / `Mod+Shift+0` | Ajustar a la cama / al diseño (o a la selección) |
| | `W` / `P` | Solo contornos / Vista previa |
| Máquina | `F` | Encuadrar |
| | `Mod+Enter` | Iniciar |
| | `Mod+Shift+P` / `Mod+.` | Pausar o reanudar / Detener |
| Paneles | `Mod+1` `Mod+2` `Mod+3` | Capas / Máquina / Consola |

En el lienzo (no son comandos del registro): rueda = zoom en el cursor; pellizco = zoom; dos dedos
en el trackpad = paneo; botón central, `Espacio` + arrastrar o la herramienta Mano = paneo.

## Diseño visual

Identidad propia de "banco de trabajo": instrumentación precisa, no una copia de ninguna otra
herramienta.

- **Tema oscuro por defecto** (los talleres suelen tener poca luz) y tema claro. El tema se guarda y
  se aplica antes de pintar, sin destello. Los tokens están en `src/styles/tokens.css`.
- **Paleta:** grafito azulado (`#151920` panel, `#0e1115` fondo) y un solo acento, el violeta de un
  diodo de 450 nm (`#8793ff` en oscuro, `#4650e0` en claro; ≥ 5:1 con su texto). Verde, ámbar y
  rojo solo para estado. Texto principal ≥ 12:1; secundario ≥ 7:1.
- **Lienzo de alto contraste:** cama más clara que el área de trabajo, cuadrícula de dos niveles que
  se adapta al zoom (1/10 mm, 1/10 cm...), marcas de registro en las esquinas y el origen de la
  máquina en el color de acento. Reglas en mm con cursor.
- **Tipografía empaquetada localmente** (sin red): Archivo variable (eje de ancho; la interfaz usa
  94 % y la marca 118 %) y Spline Sans Mono para todo número (coordenadas, tiempos, ajustes), con
  cifras tabulares para que no bailen al cambiar. Licencia OFL-1.1.
- Íconos de `lucide-react` (ISC). Foco visible en todo control; se respeta `prefers-reduced-motion`.
- La franja de colores numera las capas y marca las que están en uso; T1/T2 (rayadas) son capas de
  herramienta que nunca se envían a la máquina.

## Notas de dependencias

- Todo en `package.json` está fijado sin `^` y con versiones publicadas hace más de dos semanas.
  Licencias MIT, ISC, Apache-2.0 y OFL-1.1 (solo las fuentes).
- **TypeScript 6.0.3** y no 7.x: `typescript-eslint` 8.71 declara compatibilidad hasta `<6.1`.
- **`.npmrc` con `legacy-peer-deps=true`**: con npm 11.4 la resolución de los peers opcionales de
  Vitest falla (`Cannot read properties of null (reading 'edgesOut')`). Revisar al actualizar npm.
- `cargo deny check licenses` rechaza hoy `target-lexicon` (licencia `Apache-2.0 WITH LLVM-exception`,
  permisiva) que llega con Tauri: hay que permitir esa expresión en `deny.toml`.

## Pendiente (siguientes oleadas)

- Conectar `src-tauri` con los crates del motor e implementar los comandos del contrato.
- Reemplazar en `api/types.ts` lo que aún no tiene binding (`StatusReport`, eventos, `ExportResult`).
- Canvas: selección con ratón, transformaciones, herramientas de dibujo, edición de nodos, WebGL.
- Editor completo de capas, gestor de dispositivos, origen de 9 puntos y "iniciar desde".
- Historial visible de deshacer (los pasos ya se guardan con etiqueta en `past`/`future`).
- Cortar, copiar, pegar, agrupar y los atajos reservados.
- Con el motor real, `usePreviewSync` debe calcular solo cuando haga falta.
