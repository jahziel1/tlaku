# Tlaku Desktop

Aplicación de escritorio de Tlaku: **Tauri v2 + Vite + React 19 + TypeScript (strict) + zustand**.
La interfaz es dueña del documento que se edita; el motor (Rust) recibe el documento completo y
devuelve resultados. El contrato está en [`docs/app-api/README.md`](../../docs/app-api/README.md).

Estado: la interfaz completa funciona contra una máquina y un motor **simulados** (mock) o contra
el motor real: `src-tauri` implementa todos los comandos y eventos del contrato sobre
`tlaku-pipeline` y `tlaku-grbl`. Ver "Estado" al final.

![Vista principal](docs/screenshots/01-principal-oscuro.png)

Más capturas en `docs/screenshots/` (se regeneran con `npm run screenshots`): editor de ajustes de
capa (`03`), vista previa con resultados (`04`), máquina conectada con el diálogo de iniciar (`05`) y
el trabajo en curso (`06`), consola (`07`), historial (`08`), atajos (`09`) y tema claro (`10`). De la
fase 3: editor de nodos (`11`), panel de texto (`12`), diferencia A menos B (`13`), arreglo circular
(`14`), vectorizado con vista previa (`15`), asistente de calibración de la cámara (`16`), panel de
cámara (`17`) y pestañas del panel acoplado con texto (`18`; con poco ancho pasan a solo ícono).

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

**Modo Tauri (app nativa).** Usa `invoke`/`listen` reales: motor, archivos, perfiles de dispositivo
(`devices.json` en la carpeta de configuración) y la conexión GRBL. Las imágenes del diseño se
muestran con `imageUrl(path)` de `src/api` (protocolo `asset://`).

```bash
npm run tauri dev                              # ventana nativa con recarga en caliente
npm run tauri build -- --debug --no-bundle     # compila la app completa sin empaquetar
```

Desde la raíz del repositorio, el crate se compila con `cargo build -p tlaku-desktop`.

La implementación de la API se elige con `VITE_API=mock|tauri` (por defecto `mock`). Los scripts
`dev:tauri` y `build:tauri` usan `--mode tauri`, que carga `.env.tauri` con `VITE_API=tauri`.
Con `VITE_DEMO=off` el modo mock arranca con la cama vacía en lugar del diseño de ejemplo.

| Script | Qué hace |
|---|---|
| `npm run dev` | Vite en modo mock |
| `npm run build` | `tsc --noEmit` y `vite build` |
| `npm run typecheck` | TypeScript estricto |
| `npm run lint` | ESLint |
| `npm test` | Vitest (registro de comandos, store, mock, lienzo, capas, máquina, vista previa, historial, shell) |
| `npm run readme` | Regenera la tabla de atajos de este README desde el registro de comandos |
| `npm run screenshots` | Capturas en `docs/screenshots/` (necesita `npm run dev` en modo mock en marcha, el script no lo arranca, y Chrome; sin dependencias extra) |

## Estructura

```
apps/desktop/
  src-tauri/            Crate `tlaku-desktop` (miembro del workspace de Cargo)
    capabilities/       Permisos mínimos de Tauri v2 (eventos, diálogos, rutas, ventana)
    src/                engine (motor y archivos), machine (conexión GRBL), commands (invoke)
  src/
    api/                Contrato con el motor
      api.ts            Interfaz `LaserApi`: TODOS los comandos y eventos de docs/app-api/
      tauri.ts          Implementación real (invoke / listen)
      mock/             Implementación simulada en memoria (+ pruebas)
      types.ts          Reexporta src/bindings/ y define JobArgs y EventMap
      index.ts          getApi(): elige mock o tauri según VITE_API
    bindings/           Tipos generados por ts-rs desde tlaku-doc. NO editar a mano
    store/              zustand en slices: document, selection, view, machine, preview, devices (+ notices)
    commands/           Registro central de comandos, atajos y menús (id, etiqueta, atajo, acción)
      definitions/      Un archivo por categoría
    i18n/               es.ts (comunes) y es/<area>.ts (canvas, layers, machine, preview, history) + t()
    lib/                Geometría, viewport (mm <-> px), perfil de dispositivo y coordenadas
                        (`workToDoc`/`docToWork`), paleta y ajustes de capa, utilidades
    ui/                 Componentes reutilizables (NumberField, Modal: diálogo accesible común)
    styles/             Tokens CSS (tema oscuro y claro) y hojas globales; cada área trae su propio .css
    features/           Una carpeta por área, con un dueño cada una (ver abajo)
  scripts/              Utilidades de desarrollo (capturas)
  docs/screenshots/     Capturas de la interfaz
```

Dueños de cada área de `src/features/` (lo que es suyo y lo que expone en su `index.ts`):

| Área | Es dueña de | Expone |
|---|---|---|
| `shell/` | Layout de la ventana, barra de menús y barra superior, paleta de herramientas, panel acoplado (pestañas), barra de estado, avisos, diálogo de atajos; monta `MachineDialogHost` y el aviso de alarma una sola vez | `AppShell` |
| `canvas/` | Lienzo (interfaz `Renderer`, Canvas2D), reglas, zoom y paneo, selección, transformaciones, herramientas de dibujo, edición de nodos, ajuste magnético, dibujo de la vista previa y del cabezal | `CanvasView`, `createRenderer` |
| `layers/` | Lista de capas, editor de ajustes de capa (modo, velocidad, potencia, relleno, imagen), orden de ejecución y franja de colores | `LayersPanel`, `ColorPalette` |
| `machine/` | Conexión, estado, mover el cabezal, origen e "Iniciar desde", trabajo, consola, gestor de dispositivos, alarma y sus diálogos | `MachinePanel`, `ConsolePanel`, `MachineDialogHost`, `AlarmBanner` |
| `preview/` | Vista previa del recorrido, reproducción, resultados por capa y recálculo con debounce | `PreviewBar`, `usePreviewSync` |
| `history/` | Panel Historial (deshacer/rehacer con etiquetas), archivos (abrir, guardar, importar, exportar, recientes), arrastrar y soltar, aviso de cambios sin guardar, título de la ventana | `HistoryPanel`, `PromptHost`, `DropOverlay` y ganchos de ventana |
| `text/` | Panel de texto flotante sobre el lienzo, lista de fuentes, contorno del texto al día con el motor (`text_outline`) y convertir a trayectos | `TextPanel`, `loadFonts`, `useFonts`, `convertTextToPaths` |
| `arrange/` | Menú Organizar: booleanas, offset, alinear y distribuir, centrar y arreglos (rejilla y circular) con sus diálogos | `ArrangeDialogHost`, `useArrangeUi` |
| `trace/` | Diálogo de vectorizado de imágenes con vista previa y colocación del resultado | `TraceDialogHost` |
| `camera/` | Cámara: fuente de video, corrección de lente en WebGL, asistente de calibración, alineación con la cama y fondo del lienzo | `CameraPanel`, `CameraDialogHost` |

Los `store/*` son compartidos: cada slice solo escribe en lo suyo. Los textos de cada área viven en
`src/i18n/es/<area>.ts`.

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
6. **Todo texto visible sale del diccionario** (`src/i18n/es.ts` y `src/i18n/es/<area>.ts`) con
   `t('clave')`. Las claves son tipadas.
   Los mensajes que vienen del motor (advertencias, resúmenes) ya llegan en español.
7. **Colores solo con tokens** (`var(--accent)`, ...). Nada de hex en los componentes. El lienzo
   lee los `--cv-*` con `readCanvasPalette()`. Todo control nuevo debe verse bien en los dos temas
   y tener foco visible.
8. **Cambiar de renderer** (por ejemplo a WebGL) es implementar `Renderer`
   (`features/canvas/renderer.ts`) y devolverlo desde `createRenderer()`. La escena (`Scene`)
   ya trae todo lo necesario; las reglas y la cuadrícula no dependen del renderer.
9. **Unidades y coordenadas:** mm, mm/min y %. El documento tiene origen abajo a la izquierda y Y
   hacia arriba; solo `lib/viewport.ts` convierte a píxeles de pantalla (Y hacia abajo). La
   posición del cabezal (`work_pos`), `JobOptions.user_origin` y "ir al origen" están en
   coordenadas de la **máquina** (las que muestra GRBL): la interfaz las guarda y las envía tal
   cual, y solo para dibujarlas las pasa al documento con `workToDoc` (`lib/device.ts`) según
   `DeviceProfile.origin`. El motor hace la conversión inversa al calcular el trabajo.
10. Antes de entregar: `npm run typecheck && npm run lint && npm test && npm run build`.
    Las pruebas del motor simulado y del store no dependen del reloj real salvo por temporizadores
    cortos; si agregas flujos con tiempo, usa las opciones de `createMockApi({ ... })`.

## Atajos de teclado

Se definen una sola vez en `src/commands/definitions/` y se muestran en menús, tooltips y en el
diálogo de atajos (`F1`, `Ctrl+/` o `?`). `Cmd` en macOS, `Ctrl` en Windows y Linux. Los conflictos
de atajos son un error al arrancar y lo verifica una prueba.

Reglas (respuesta a la queja del foro de atajos contradictorios y que "no hacen nada"):

- **Funcionan en toda la ventana**, no solo con el lienzo enfocado. Las teclas de una sola letra solo
  se ignoran mientras se escribe en un campo de texto.
- **Una tecla, un significado.** No hay letras que cambien de función según lo que haya bajo el cursor.
- **Cmd/Ctrl se reserva para acciones de archivo, edición y vista.** No se sobrecarga como
  modificador de pasos finos: el paso fino usa `Alt`, el grande `Mayús`.
- Un atajo de un comando deshabilitado no hace nada, y el tooltip del botón dice por qué.

**Editor de nodos (`N`).** Con un objeto vectorial seleccionado muestra sus nodos y manejadores. Clic
elige un nodo (Mayús+clic suma o quita; clic en un segmento elige sus dos nodos; arrastrar sobre vacío
dibuja una caja de nodos). Arrastrar mueve los nodos elegidos y los manejadores (Alt rompe la
simetría); doble clic en un segmento agrega un nodo sin cambiar la forma. La barra de nodos fija la X
y la Y exactas (en mm de documento) y reúne tipo de nodo, segmento y trayecto. El objeto conserva su
`transform`: los puntos se editan en coordenadas locales. **Deshacer y rehacer funcionan siempre**, con
nodos elegidos o sin ellos. Con la herramienta de nodos, `Supr`/`⌫` y las flechas actúan sobre los
nodos elegidos (si no hay ninguno, siguen borrando o moviendo el objeto); las demás teclas de nodos
(`I`, `B`, `J`, `O`, `Mayús+L`, `Mayús+U`, `Mayús+E`, `Mayús+S`, `Mayús+M`) solo valen con esa herramienta
y cada una hace una sola cosa, sin depender de lo que haya bajo el cursor. `Esc` vuelve a Seleccionar.
Un nodo "esquina" pedido a mano se recuerda solo durante la sesión (el documento no guarda el tipo de nodo:
al reabrir, manejadores alineados se leen como suaves).

**Texto (`T`).** Clic en vacío crea un texto "Texto" de 10 mm en la capa activa; clic sobre un texto lo
selecciona; el doble clic sobre un texto con Seleccionar pasa a la herramienta de texto. El lienzo sube
`textFocusToken` (`store/view.ts`) para que el panel de texto tome el foco.

**Imagen de cámara.** Si `cameraBackdrop` existe, el lienzo la dibuja bajo la cuadrícula y los objetos,
proyectada sobre sus cuatro esquinas (malla de triángulos con una afín por triángulo; ver
`features/canvas/backdrop.ts`). Sin cámara no cuesta nada.

La tabla sale del registro de comandos y una prueba (`src/features/shell/readme.test.ts`) falla si
se desactualiza; para regenerarla, `npm run readme`. Formato Windows/Linux (en macOS, `Ctrl` es `⌘`,
`Alt` es `⌥` y `Mayús` es `⇧`).

<!-- shortcuts:start (generado: npm run readme) -->

| Grupo | Comando | Atajo |
|---|---|---|
| Archivo | Nuevo proyecto | `Ctrl+N` |
|  | Abrir proyecto… | `Ctrl+O` |
|  | Guardar | `Ctrl+S` |
|  | Guardar como… | `Ctrl+Mayús+S` |
|  | Importar diseño… | `Ctrl+I` |
|  | Exportar G-code… | `Ctrl+Mayús+E` |
|  | Imprimir… | `Ctrl+P` |
|  | Nueva ventana | `Ctrl+Mayús+N` |
| Edición | Deshacer | `Ctrl+Z` |
|  | Rehacer | `Ctrl+Mayús+Z` o `Ctrl+Y` |
|  | Seleccionar todo | `Ctrl+A` |
|  | Invertir selección | `Ctrl+Mayús+I` |
|  | Duplicar | `Ctrl+D` |
|  | Eliminar | `Supr` o `⌫` |
|  | Copiar | `Ctrl+C` |
|  | Cortar | `Ctrl+X` |
|  | Copiar como SVG | `Ctrl+Alt+C` |
|  | Pegar | `Ctrl+V` |
|  | Pegar en el mismo lugar | `Ctrl+Mayús+V` |
|  | Agrupar | `Ctrl+G` |
|  | Desagrupar | `Ctrl+Mayús+G` |
|  | Mover a la izquierda (1 mm) | `←` |
|  | Mover a la izquierda (10 mm) | `Mayús+←` |
|  | Mover a la izquierda (0,1 mm) | `Alt+←` |
|  | Mover a la derecha (1 mm) | `→` |
|  | Mover a la derecha (10 mm) | `Mayús+→` |
|  | Mover a la derecha (0,1 mm) | `Alt+→` |
|  | Mover hacia arriba (1 mm) | `↑` |
|  | Mover hacia arriba (10 mm) | `Mayús+↑` |
|  | Mover hacia arriba (0,1 mm) | `Alt+↑` |
|  | Mover hacia abajo (1 mm) | `↓` |
|  | Mover hacia abajo (10 mm) | `Mayús+↓` |
|  | Mover hacia abajo (0,1 mm) | `Alt+↓` |
|  | Elegir capa de la paleta | `Alt+1` |
|  | Elegir capa de la paleta | `Alt+2` |
|  | Elegir capa de la paleta | `Alt+3` |
|  | Elegir capa de la paleta | `Alt+4` |
|  | Elegir capa de la paleta | `Alt+5` |
|  | Elegir capa de la paleta | `Alt+6` |
|  | Elegir capa de la paleta | `Alt+7` |
|  | Elegir capa de la paleta | `Alt+8` |
|  | Elegir capa de la paleta | `Alt+9` |
|  | Elegir capa de la paleta | `Alt+0` |
|  | Elegir capa de la paleta | `Alt+G` |
|  | Ajustes de la capa activa… | `Alt+Intro` |
|  | Preferencias… | `Ctrl+,` |
| Herramientas | Seleccionar | `V` o `Esc` |
|  | Editar nodos | `N` |
|  | Dibujar línea | `L` |
|  | Rectángulo | `R` |
|  | Elipse | `E` |
|  | Polígono | `G` |
|  | Estrella | `Mayús+G` |
|  | Texto | `T` |
|  | Mano (mover la vista) | `H` |
|  | Medir | `M` |
|  | Puentes manuales | `U` |
|  | Punto de inicio | `Q` |
|  | Recortar formas | `X` |
|  | Insertar nodo en el punto medio | `I` |
|  | Segmento a recta | `Mayús+L` |
|  | Segmento a curva | `Mayús+U` |
|  | Nodo en esquina | `Mayús+E` |
|  | Nodo suave | `Mayús+S` |
|  | Nodo simétrico | `Mayús+M` |
|  | Romper el trayecto en el nodo | `B` |
|  | Unir extremos | `J` |
|  | Cerrar trayecto | `O` |
|  | Cerrar polilínea | `C` |
| Organizar | Unión | `Alt+U` |
|  | Diferencia (A menos B)… | `Alt+D` |
|  | Soldar | `Alt+W` |
|  | Offset… | `Alt+O` |
|  | Centrar en la cama | `Alt+C` |
|  | Voltear horizontal | `Alt+H` o `Ctrl+Mayús+H` |
|  | Voltear vertical | `Alt+V` |
|  | Espejo sobre una línea | `Alt+Mayús+M` |
|  | Girar y escalar por dos puntos | `Alt+T` |
|  | Girar 90° a la izquierda | `[` |
|  | Girar 90° a la derecha | `]` |
|  | Girar un paso a la izquierda | `Mayús+[` |
|  | Girar un paso a la derecha | `Mayús+]` |
|  | Girar 5° a la izquierda | `Alt+[` |
|  | Girar 5° a la derecha | `Alt+]` |
|  | Traer al frente | `Ctrl+RePág` o `Ctrl+Mayús+]` |
|  | Enviar al fondo | `Ctrl+AvPág` o `Ctrl+Mayús+[` |
|  | Subir un nivel | `RePág` o `Ctrl+]` |
|  | Bajar un nivel | `AvPág` o `Ctrl+[` |
|  | Cortar antes (prioridad -1) | `Alt+Mayús+[` |
|  | Cortar después (prioridad +1) | `Alt+Mayús+]` |
|  | Bloquear | `Ctrl+L` |
|  | Desbloquear todo | `Ctrl+Mayús+L` |
|  | Asistente booleano… | `Ctrl+B` |
|  | Recortar en el último clic | `Ctrl+K` |
|  | Separar subtrayectos… | `Alt+B` |
|  | Optimizar formas… | `Alt+Mayús+O` |
|  | Cortar formas con una cuchilla… | `Alt+Mayús+C` |
| Vista | Acercar | `Ctrl+=` o `Ctrl+Mayús+=` |
|  | Alejar | `Ctrl+-` |
|  | Ajustar a la cama | `Ctrl+0` |
|  | Ajustar al diseño | `Ctrl+Mayús+0` |
|  | Solo contornos | `W` |
|  | Ajuste magnético | `S` |
|  | Vista previa | `P` |
|  | Reproducir el recorrido | `K` |
|  | Reproducción más lenta | `,` |
|  | Reproducción más rápida | `.` |
| Máquina | Encuadrar | `F` |
|  | Iniciar | `Ctrl+Intro` |
|  | Pausar | `Ctrl+Mayús+P` |
|  | Detener | `Ctrl+.` |
|  | Mover a la izquierda | `Ctrl+Alt+←` |
|  | Mover a la derecha | `Ctrl+Alt+→` |
|  | Mover hacia arriba | `Ctrl+Alt+↑` |
|  | Mover hacia abajo | `Ctrl+Alt+↓` |
| Paneles y ayuda | Mostrar historial | `Ctrl+4` |
|  | Atajos de teclado… | `Ctrl+/` o `F1` o `Mayús+/` |
|  | Notas del proyecto | `Ctrl+7` |
|  | Mostrar capas | `Ctrl+1` |
|  | Mostrar máquina | `Ctrl+2` |
|  | Mostrar consola | `Ctrl+3` |
|  | Panel de cámara | `Ctrl+5` |
|  | Panel de materiales | `Ctrl+6` |
| Lienzo (gestos) | Zoom en el cursor | `Rueda del ratón` |
|  | Zoom con el trackpad | `Pellizco` |
|  | Mover la vista | `Botón central, Espacio + arrastrar o dos dedos` |

<!-- shortcuts:end -->

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
- La franja de colores identifica cada capa por su nombre (Rojo, Azul...) y marca las que están en
  uso; la última muestra, punteada, es la capa de guías, que nunca se envía a la máquina.

## Notas de dependencias

- Todo en `package.json` está fijado sin `^` y con versiones publicadas hace más de dos semanas.
  Licencias MIT, ISC, Apache-2.0 y OFL-1.1 (solo las fuentes).
- **TypeScript 6.0.3** y no 7.x: `typescript-eslint` 8.71 declara compatibilidad hasta `<6.1`.
- **`.npmrc` con `legacy-peer-deps=true`**: con npm 11.4 la resolución de los peers opcionales de
  Vitest falla (`Cannot read properties of null (reading 'edgesOut')`). Revisar al actualizar npm.

## Estado

Hecho y cubierto por pruebas:

- Lienzo: selección con ratón, transformaciones, herramientas de dibujo, edición de nodos, ajuste
  magnético, medir, reglas, zoom y paneo; vista previa del recorrido con reproducción y resultados.
- Capas: lista con orden de ejecución, editor completo de ajustes, franja de colores.
- Máquina: conexión, estado, mover el cabezal, origen de usuario con los 9 puntos, "Iniciar desde",
  encuadrar, iniciar con diálogo de confirmación, pausa/detener, consola con confirmación de
  comandos peligrosos, alarmas con explicación, gestor de dispositivos.
- Historial visible de deshacer/rehacer, cortar/copiar/pegar, archivos recientes y
  diálogo de atajos generado del registro.
- Documento versión 2 (fase 3): las trayectorias son curvas Bézier (`BezPath`; elipses y esquinas
  redondeadas son cúbicas reales y el lienzo las dibuja con `bezierCurveTo`, nítidas a cualquier
  zoom; el SVG importado conserva sus curvas), grupos planos (`group`: Ctrl+G / Ctrl+Mayús+G; un
  clic en un objeto del grupo elige todo el grupo y Alt+clic solo ese objeto) y objetos de texto
  (se dibuja su `outline`; sin contorno generado se ve un marcador con el texto en fuente del sistema).
  Los proyectos de la versión 1 se migran al abrirlos.

Pendiente:

- Renderer WebGL para el lienzo (hoy Canvas2D; la interfaz `Renderer` ya lo permite).
- Aislar el cálculo de la vista previa del motor real para que solo corra cuando hace falta
  (hoy se recalcula con debounce ante cualquier cambio del documento).
- Idiomas distintos del español (el diccionario ya está separado por área).
- Empaquetado e instaladores firmados (hoy `tauri build --no-bundle`).
