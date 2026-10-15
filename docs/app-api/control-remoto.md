# Contrato app-motor: Control remoto desde el celular

Parte de [el contrato app-motor](README.md). Servidor HTTP, seguridad, API y aviso al terminar.

## Control remoto desde el celular (ola 5.5)

Un servidor HTTP opcional (`apps/desktop/src-tauri/src/remote/`) deja ver y controlar el trabajo desde el
navegador del celular, sin instalar nada. La PC muestra un código QR; el celular lo abre y carga **una sola
página** (`page.html`, embebida en el ejecutable con `include_str!`, sin CDN ni dependencia de Vite) que habla
con la API de abajo. La página está en español e inglés según el idioma del navegador.

### Seguridad

- **Apagado de fábrica** (`remoteEnabled`, Preferencias ▸ Control remoto). Un paquete `.tlakuprefs` o
  `.tlakubundle` importado nunca lo enciende (`NEVER_IMPORTED` en `features/settings/bundle.ts`).
- **Solo red local, nunca internet.** Escucha en **una** dirección IPv4 privada (RFC 1918 o enlace local):
  la que se elige en Preferencias (`remoteIp`) o, sin elección, la recomendada (192.168.x antes que 10.x antes que
  172.16/12; las interfaces de contenedores, VPN y máquinas virtuales van al final). Jamás en `0.0.0.0`.
  Además cada petición se rechaza con 403 si el par que la manda no es una dirección local (un reenvío de
  puertos del router, por ejemplo). El encabezado `Host` debe ser una IP (defensa contra el DNS rebinding).
- **Clave aleatoria de 128 bits** (`getrandom`), distinta en cada arranque del servidor, que viaja en el
  **fragmento** de la URL del QR (`http://<ip>:<puerto>/#<clave>`): el navegador nunca manda el fragmento al
  servidor ni lo deja en el historial (la página lo borra de la barra al leerlo). La página la entrega una vez a
  `POST /api/login` y recibe una **cookie de sesión** (`tlaku_session`, otro valor aleatorio, `HttpOnly`,
  `SameSite=Strict`). Alternativa para scripts: la clave en cada petición con `Authorization: Bearer <clave>` o
  `X-Tlaku-Token` (esas peticiones comparten una sesión por dirección y aparato).
- **Autenticación en cada petición** salvo la página estática (`GET /`) y `POST /api/login`. Los secretos se
  comparan en **tiempo constante** (`auth::ct_eq`) y la búsqueda de sesión recorre todas.
- **Límite de intentos:** 5 claves incorrectas en 60 s desde una dirección la bloquean 120 s (429 con
  `Retry-After`), incluso si luego manda la correcta. El bloqueo también alcanza a la clave por encabezado.
- **Regenerar la clave** (botón en la PC) invalida el QR anterior y **expulsa a todos los clientes**. La PC
  lista quién está conectado (dirección, aparato, actividad) y puede **desconectar** a uno (`remote_kick`).
  Las sesiones sin actividad caducan a los 30 min; máximo 8 sesiones y 16 conexiones en vivo.
- **Sin CORS:** no se emite ningún encabezado `Access-Control-*`. Las acciones (`POST`) exigen la marca
  `X-Requested-With` (o la clave en encabezado) y, si el navegador manda `Origin`, que sea este mismo servidor.
- Respuestas con `Cache-Control: no-store`, `X-Content-Type-Options: nosniff`, `Referrer-Policy: no-referrer`,
  `X-Frame-Options: DENY` y una política de contenido (`default-src 'none'`; solo el script y los estilos
  de la propia página).
- **Pausar, reanudar y detener: siempre.** Van directo a la máquina (`Machine::pause/resume/cancel`), sin
  pasar por la ventana: si la ventana se congela, detener sigue funcionando. Detener no mira el estado,
  el encuadre ni el checklist. El galvo no tiene pausa (`caps.pause = false`), solo detener.
- **Iniciar** exige todo esto a la vez: el checklist completo del celular (*estoy junto a la máquina*, *área
  despejada*, *aire o extractor encendidos*; con la cola guiada esperando, además *ya cambié el material o la pieza*, ver «Cola guiada»); máquina conectada y en reposo; sin trabajo en curso; la ventana
  de la PC informando (si calla más de 12 s, `pc_offline`); algo que enviar; **que el trabajo se haya
  encuadrado en la PC después del último cambio** del documento, de la máquina o de las opciones del trabajo
  (`selectFramed` en `store/machine.ts`: la marca guarda las referencias de `doc`, `jobOptions`, el
  dispositivo activo y el evento de conexión, más la selección si el trabajo es «solo la selección»; el
  store es inmutable, así que cualquier cambio la invalida; reconectar también). Un galvo (haz invisible)
  nunca se inicia desde el celular. El servidor **no inicia nada**: pide a la ventana (`remote://start`) que
  inicie con su ruta de siempre (`startJob`, con el preflight), y la ventana vuelve a comprobar el
  encuadre antes de responder (`remote_start_result`); así la lógica no se duplica. Si falla, el celular
  recibe el motivo. Si la ventana no contesta en 60 s, `timeout`.
- **No hay HTTPS.** En una red local no se puede conseguir un certificado de confianza para una IP privada;
  usar uno autofirmado haría que el navegador del celular mostrara una advertencia que los usuarios
  aprenden a ignorar. Por eso la clave va en el fragmento de la URL y la sesión en una cookie aparte, pero
  quien pueda **espiar la red local** (un Wi-Fi sin cifrar, por ejemplo) puede ver el tráfico y la cookie:
  úsalo solo en una red de confianza. La cookie no lleva `Secure` por la misma razón.

### API HTTP

Cuerpos y respuestas en JSON UTF-8. Errores: `{ "ok": false, "code": "...", "message": "..." }` (`message` en
español; la página traduce por `code`). Todo lo de `/api/*` excepto `login` exige credenciales (401 si faltan).

| Ruta | Respuesta |
|---|---|
| `GET /` | La página del celular (pública, estática). |
| `POST /api/login` · `{ "token": "<clave>" }` | 200 `{ ok: true }` + `Set-Cookie`; 401 `bad_token`; 429 `locked`. |
| `GET /api/state` | Estado completo (abajo). |
| `GET /api/events` | **SSE** (`text/event-stream`): un evento `state` (el mismo JSON de `/api/state`) al conectar, en cada cambio (con un pequeño agrupado de 150 ms) y cada 10 s como latido. Se cierra sola si el cliente es expulsado. |
| `GET /api/route` | `{ version, bed: [w, h] \| null, d }`: recorrido del trabajo como trazado SVG en mm del documento (Y arriba). |
| `GET /api/live` | **Vista en vivo** (solo lectura): `{ version, bed, total, layers: ["#rrggbb"], runs: [{ k: 0\|1, l, w, p: [x, y, t, ...] }], decimated }`, la ruta del trabajo simplificada (abajo). `version: 0` y sin trazos si no hay. La página solo la pide al encender su interruptor. |
| `GET /api/camera.jpg` | Último cuadro de la cámara de la PC (JPEG); 404 `no_camera` si no hay uno de los últimos 20 s. |
| `POST /api/pause`, `/api/resume`, `/api/stop` | 200 `{ ok: true }`; 409 con `not_connected`, `no_pause` (galvo) o `failed` (la máquina lo rechazó). |
| `POST /api/start` · `{ "checklist": { "nearby": true, "clear": true, "air": true, "material": false } }` (`material` solo cuenta si la cola espera) | 200 si la ventana inició el trabajo; 409 con `checklist`, `checklist_material`, `pc_offline`, `not_connected`, `job_active`, `not_idle`, `nothing_to_send`, `galvo`, `not_framed`, `busy_request` o `failed` (el motivo del preflight va en `message`); 504 `timeout`. |

Estado (`/api/state` y eventos `state`):

```ts
interface RemoteState {
  v: 1;
  pc_online: boolean;                       // la ventana informó hace menos de 12 s
  machine: { connected: boolean; kind?: 'grbl'|'ruida'|'lhy'|'galvo'; virtual?: boolean; state?: string };  // del motor
  device: string; doc: string;
  job: { phase: 'idle'|'running'|'paused'|'finished'|'cancelled'|'error'; message: string;
         progress: number /* 0..1 */; elapsed_s: number; remaining_s: number | null; total_s: number | null;
         layer: { index: number; count: number; color: string; mode: string } | null;
         finished_seq: number; error_seq: number };   // contadores: el aviso al terminar se dispara al subir
  head: [number, number] | null; bed: [number, number] | null;   // mm del documento
  caps: { pause: boolean; pause_reason: string | null; stop: boolean };
  start: { allowed: boolean; reason: string | null; message: string | null };
  route_v: number; camera_v: number;        // 0 = no hay; la página los compara para volver a pedir
  live_v: number;                           // versión de `GET /api/live`; 0 = no hay ruta para la vista en vivo
  live: { t: number; running: boolean; retreat_seq: number; src: 'position'|'progress'|'clock' } | null;  // dónde está la máquina sobre esa ruta
  queue: RemoteQueue | null;                // la cola guiada de trabajos (solo lectura); null si no hay
}

interface RemoteQueue {
  state: 'idle'|'awaiting'|'running'|'paused'|'stopped'|'done';
  done: number; total: number;              // trabajos (copias) terminados y total
  eta_s: number | null;                     // segundos que faltan; null si no se conocen todos
  current: { name: string; copies: number; done: number; status: string } | null;
  copy: number;                             // copia en curso o por confirmar del trabajo actual (desde 1)
  message: string;                          // por qué se detuvo (`stopped`)
  items: { name: string; copies: number; done: number; status: 'pending'|'done'|'skipped'|'failed' }[];  // hasta 12
}
```

### Cola guiada

La cola de trabajos (`features/queue` en la PC) viaja en `RemoteSnapshot.queue` y el servidor la reenvía tal cual en el
estado (`queue`; limpia el texto, acota a 12 trabajos y a los seis estados). El celular **solo la ve**: no hay ninguna
ruta `/api/queue`. **La cola nunca avanza sola**: al terminar un trabajo queda en `awaiting` hasta que una persona
confirme el siguiente, en la PC (diálogo «Siguiente trabajo de la cola», que abre el diálogo de inicio normal) o desde el
celular con la misma ruta que «iniciar»:

- el checklist del celular lleva una casilla más (`checklist.material`, *ya cambié el material o la pieza*): con
  `queue.state == "awaiting"` en el último informe de la PC, `POST /api/start` sin ella responde 409
  `checklist_material`; para un inicio normal no cuenta;
- el documento debe estar **encuadrado en la PC** y ser el de la pestaña activa: mientras la cola espera confirmar
  otro documento, la ventana informa `framed: false` (`not_framed`) y `handleRemoteStart` responde `queue_other_tab`
  (`remote.reason.queue_other_tab`);
- todo lo demás de «Iniciar» (máquina en reposo, sin trabajo en curso, ventana informando, sin galvo) sigue igual y la
  ventana vuelve a comprobarlo; el trabajo se inicia con `startJob` (preflight incluido) y la cola solo lo observa
  (pasa a `running` cuando la máquina empieza un trabajo de la pestaña esperada).

### Lo que informa la ventana

El estado del trabajo sale de los eventos `machine://*` que el store (`store/machine.ts`) ya sigue; la ventana
lo resume y se lo manda al servidor (`features/remote/`), como mucho 4 veces por segundo y solo si cambió
(más un latido cada 4 s). Si la ventana se cierra, el servidor lo nota (`pc_online: false`) y bloquea iniciar.

| Comando (`invoke`; todos con `rename_all = "snake_case"`) | Argumentos | Resultado |
|---|---|---|
| `remote_configure` | `config: { enabled, port, ip: string \| null }` | `RemoteStatus` |
| `remote_status` | — | `RemoteStatus` |
| `remote_regenerate` | — | `RemoteStatus` con el QR nuevo; los clientes quedan expulsados |
| `remote_kick` | `id` | `RemoteStatus` |
| `remote_update` | `snapshot: RemoteSnapshot` | — |
| `remote_set_route` | `route: { bed, d } \| null` | — (error si pasa de 400 KB o trae caracteres que no son de un trazado) |
| `remote_set_live` | `route: { bed, total, layers, runs, decimated } \| null` | — (error si no es válida o pasa de 320 KB / 30 000 trazos / 60 000 vértices; ver «Vista en vivo») |
| `remote_camera_frame` | `jpeg_b64` (vacío = cámara apagada) | `boolean`: hay algún celular esperándolo. Si es `false` la ventana no manda más hasta que haya clientes |
| `remote_start_result` | `result: { id, ok, code?, message? }` | — |

`RemoteStatus = { running, port, ip, url, full_url, qr_svg, error, addrs: [{ ip, name, recommended }], clients: [{ id, ip, agent, since_ms, idle_s, connected }] }`.
`full_url` lleva la clave (es la del QR); `qr_svg` es el QR de `full_url` como SVG (lo genera `tlaku-codes`;
las formas vienen con Y hacia arriba y se voltean para dibujarlas). `RemoteSnapshot` está en `api/api.ts`; el
servidor limpia y acota todo lo que recibe (texto de 120 caracteres, números finitos, etc.).

Eventos hacia la ventana: `remote://clients` `{ clients }` (entró, salió, caducó o lo desconectaron),
`remote://start` `{ id, client }` (un celular pidió iniciar con su checklist completo) y `remote://action`
`{ action: 'pause' | 'resume' | 'stop', client }` (para avisar en la PC y cortar el bucle de encuadre).

### Vista en vivo (solo lectura)

La simulación dibujada del trabajo que sigue a la máquina real (`features/live`). En el celular **no añade
ninguna acción**: es una ruta `GET` más, con el mismo token y las mismas reglas de red local que el resto;
las rutas `POST/PUT/DELETE` de `/api/live` no existen. Lo que viaja:

- **La ruta, una vez por trabajo** (`remote_set_live`, la arma `features/live/remoteRoute.ts`). Son los **trazos
  continuos con láser** (`k` 0 = corte, 1 = grabado; `l` = capa en `layers`; `w` = ancho en mm) con `p` =
  `[x, y, t, ...]`, es decir, la posición en mm del documento (Y arriba) y el **tiempo de trabajo** (s) en cada
  vértice. Los desplazamientos y el overscan no van: la página mueve la cabeza en línea recta del final de un
  trazo al inicio del siguiente usando los tiempos. Cada trazo se simplifica con Douglas-Peucker (tolerancia
  en mm: 0,1 o 1/4000 de la diagonal de la cama, la mayor). **Tope: 300 000 bytes de JSON** (el servidor
  acepta hasta 320 KB, 30 000 trazos y 60 000 vértices). Si no cabe: primero sube la tolerancia (hasta 1/500
  de la diagonal) y después, para rellenos e imágenes, manda **líneas agrupadas** (una de cada `s` pasadas,
  dibujada `s` veces más ancha con el paso entre líneas estimado de los datos; `decimated: true`); los cortes
  no se agrupan salvo como último recurso. Con más de 3 000 000 de movimientos no se manda ruta y la página
  sigue mostrando el recorrido de siempre. Cada ruta nueva sube `live_v`; al empezar otro trabajo se manda
  `null` (se borra) y luego la nueva.
- **La ubicación, en cada informe** (`RemoteSnapshot.live`, ~4 por segundo): `t` es el tiempo de trabajo en
  que la PC **ubicó la posición real** que reporta la máquina sobre esa misma ruta; `running` es falso en
  Hold/pausa; `retreat_seq` sube si la máquina retrocedió de verdad; `src` es `position`, `progress` (la
  posición no encajó con el recorrido y se usa el progreso del envío) o `clock` (máquinas que no reportan
  posición mientras trabajan: K40, galvo; la página lo dice). Solo se informa mientras haya al menos un
  celular conectado; sin celulares la PC no sigue el trabajo.
- **La página interpola** el cabezal entre informes con la misma lógica que la PC (`features/live/tracker.ts`;
  el bloque `live-core` de `page.html` es una copia que una prueba fija) y dibuja en un `<canvas>`: capa
  pendiente pre-renderizada una vez, capa quemada incremental. El interruptor se guarda en `localStorage`
  (`tlaku.remote.live`, con try/catch); apagado no baja la ruta ni anima nada, y al encenderlo (también a
  mitad del trabajo) baja la ruta una vez y se pone al día con lo ya hecho.

### Cámara

La cámara vive en el webview de la PC. Mientras haya clientes conectados y la cámara esté encendida
(`cameraStatus === 'live'`), la ventana manda cada 2,5 s un JPEG reducido (640 px de ancho, calidad 0,6; unos
30 a 60 KB) tomado del lienzo ya corregido de lente, con `remote_camera_frame`. Sin clientes no se manda nada
y el servidor ni siquiera guarda lo que reciba. La página muestra la imagen y la recarga cuando sube `camera_v`.

### Aviso al terminar

La página muestra un aviso destacado a pantalla completa («¡Trabajo terminado!» o «El trabajo falló»),
vibra (`navigator.vibrate`, donde el navegador lo permita; Safari de iOS no), suena un pitido (Web Audio,
una vez que el usuario tocó la página), parpadea el título de la pestaña y mantiene la pantalla encendida
mientras trabaja (`wakeLock`). El aviso se dispara con los contadores `finished_seq`/`error_seq`, que sube el
servidor al ver el paso de «en curso» a «terminado»/«falló»: no depende de que el celular estuviera
mirando en ese instante. Al entrar a una sesión ya empezada no se avisa de lo que pasó antes.

### Probarlo con un celular

1. Preferencias ▸ Control remoto ▸ activar. Debe decir «Escuchando en 192.168.x.x:47802».
2. Con el celular en la misma red Wi-Fi, escanear el QR con la cámara y abrir el enlace.
3. Conectar una máquina (o una `virtual:*`), cargar un diseño y **encuadrar** (Máquina ▸ Encuadrar).
4. En el celular: Iniciar trabajo ▸ marcar las tres casillas ▸ Iniciar. Probar Pausar, Reanudar y Detener
   (toque doble). Cambiar algo en el diseño en la PC: Iniciar vuelve a quedar deshabilitado con el motivo.
5. «Regenerar clave» en la PC: el celular pasa a «Te desconectaron». «Desconectar» quita solo a uno.
