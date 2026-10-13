# Arquitectura

El motor es una biblioteca en Rust organizada en crates pequeños. Ninguno conoce la interfaz:
todo lo que haga la aplicación se puede ejecutar y probar desde la línea de comandos.
Los crates intermedios (`geom`, `plan`, `fill`, `raster`, `sim`) dependen solo de `tlaku-core`;
`tlaku-pipeline` los une y es lo que usan el CLI y la app de escritorio (ver [app-api.md](app-api.md)).

## Pipeline

```mermaid
flowchart TD
    SVG[tlaku-svg<br/>SVG] --> JOB
    DXF[tlaku-dxf<br/>DXF] --> JOB
    IMG[imagen PNG/JPEG/BMP] --> RASTER

    JOB["Job: capas con polilíneas<br/>(tlaku-core)"] --> GEOM["tlaku-geom<br/>booleanas, offset, kerf, auto-join"]
    GEOM --> PLAN["tlaku-plan<br/>orden de corte explicable"]
    GEOM --> FILL["tlaku-fill<br/>relleno por barrido"]
    RASTER["tlaku-raster<br/>ajustes, dithering, barrido"]

    PLAN --> TP
    FILL --> TP
    RASTER --> TP
    JOB -. "modo Line directo:<br/>Toolpath::from_line_job" .-> TP

    TP["Toolpath: bloques de Travel y Cut<br/>(tlaku-core)"] --> SIM["tlaku-sim<br/>tiempo, overscan, muestreo"]
    TP --> GCODE["tlaku-grbl::gcode<br/>G-code M3/M4"]
    GCODE --> STREAM["tlaku-grbl::streamer + serial<br/>envío por conteo de caracteres"]
    SIM -. "estimación / vista previa" .-> OUT[(usuario)]
    STREAM --> MACHINE[(máquina GRBL)]
```

Los drivers futuros (Ruida, galvo) también partirán de un `Toolpath`.

## Responsabilidad de cada crate

| Crate | Responsabilidad |
|---|---|
| `tlaku-core` | Tipos base (`Point`, `Polyline`, `Rgb`), capas, `CutSettings`, `Job`, orden de corte básico (`optimize`) y `Toolpath`. |
| `tlaku-svg` | SVG a curvas en mm, una capa por color; voltea Y al convertir a `Job`. Aplica `clip-path` de verdad (con `tlaku-geom`): las formas rellenas se intersectan y los trazos se parten; `mask` se aproxima como recorte binario y siempre avisa. |
| `tlaku-dxf` | DXF a polilíneas en mm con color ACI/true color; no voltea Y; cada omisión genera una advertencia. |
| `tlaku-geom` | Booleanas y soldar, offset, compensación de kerf, auto-join, quitar segmentos duplicados (rejilla espacial), puentes (`tabs`), regla par-impar de anidamiento. Trabaja sobre polilíneas. |
| `tlaku-plan` | Decide el orden de las figuras (capa, grupo, prioridad, interior primero, recorrido corto) y explica cada paso con un `Reason`. Determinista. "Interior primero" incluye solapes: A va antes que B si más de la mitad de A cae dentro de B (muestreo fijo); en solapes mutuos gana la de menor área, así que no hay ciclos. |
| `tlaku-fill` | Relleno vectorial por barrido (ángulo, serpentina, crosshatch, overscan) o por contornos concéntricos (`FillPattern::Offset`); produce un `ToolpathBlock`. |
| `tlaku-raster` | Imagen a bitmap de potencia (ajustes, remuestreo, dithering/umbral, semitono, líneas de periódico, boceto) y barrido a `ToolpathBlock` con cualquier ángulo (0° conserva el camino original). |
| `tlaku-sim` | Estima el tiempo con aceleración por eje y desviación de unión, calcula el overscan (`v²/2a`) y muestrea la trayectoria. |
| `tlaku-grbl` | G-code GRBL 1.1 a partir de un `Toolpath`, puertos serie, envío con conteo de caracteres, pausa y cancelación, y la seguridad previa al envío (`preflight`: solo se transmite en reposo, con consejos para el CLI o la consola de la app; `frame`: G-code del encuadre). |
| `tlaku-doc` | Tipos del contrato con la app (`Document`, `LayerSettings`, `DeviceProfile`, `JobOptions`, `JobPreview`...) con `serde` y `ts-rs`, más transformaciones afines y cajas envolventes. No depende del resto del motor. |
| `tlaku-pipeline` | De `Document` + `DeviceProfile` + `JobOptions` a `ComputedJob` (`Toolpath`, vista previa, avisos): validación de capas, orden automático, kerf, plan, puentes, relleno, imágenes, eje Z, corrección de barrido, colocación (`start_from`, `job_origin`), límites de la cama, rotativo y G-code. También `import_file` y `material_test`. |
| `tlaku-cli` | Binario `tlaku`; traduce los argumentos a un `Document` y delega en `tlaku-pipeline`. |
| `tlaku-desktop` (`apps/desktop/src-tauri`) | App Tauri: comandos y eventos de [app-api.md](app-api.md) sobre `tlaku-pipeline` y `tlaku-grbl`; la conexión vive en un hilo dedicado. |

## Contrato de `Toolpath`

`Toolpath` (en `tlaku-core/src/toolpath.rs`) es lo único que ven los drivers y el simulador:
una lista de `ToolpathBlock`. Cada bloque comparte velocidad (`speed_mm_min`), asistencia de aire
(`air_assist`) y modo de potencia (`PowerMode`), y contiene una lista de `Move`:

| Movimiento | Significado |
|---|---|
| `Move::Travel { to }` | Desplazamiento rápido con el láser apagado (G0 en GRBL). |
| `Move::Cut { to, power_pct }` | Movimiento a la velocidad del bloque con el láser a `power_pct` (0-100). |

- `Cut` con `power_pct == 0` es **overscan**: el cabezal acelera o frena sin disparar. En GRBL sale
  como G1 con `S0`; el muestreo del simulador lo marca con el láser apagado.
- Los movimientos son segmentos rectos desde la posición anterior.
- `PowerMode::Dynamic` (M4) deja que la controladora escale la potencia con la velocidad real;
  `Constant` (M3) la mantiene fija.
- Un driver solo traduce movimientos a su protocolo: no decide orden, relleno ni overscan.
- `z_mm` (opcional) es la Z absoluta al empezar el bloque (0 = foco); GRBL la emite como `G0 Z`.

## Decisiones del taller (fase 4)

- **Puentes:** se calculan después del kerf y del planificador (el orden de corte no cambia con o
  sin puentes). Cada hueco es `Cut` a potencia 0, no `Travel`: así la máquina no frena en cada
  puente. Nunca ocupan más de la mitad de un contorno; si no caben, se reducen con aviso.
- **Eje Z:** solo con `DeviceProfile.z_axis`. Con `z_step != 0` y varias pasadas, la capa se divide
  en un bloque por pasada (una fila de vista previa por pasada). El simulador ignora Z. El signo de
  +Z depende de la máquina.
- **Rotativo:** el documento y la vista previa quedan en mm de superficie de la pieza; la
  conversión a mm de la controladora se hace solo al generar G-code y encuadre, en Y absoluta
  (sin `G92`). F se reescala por tramo para que la velocidad sobre la superficie sea la pedida.
  Con rotativo no se valida Y contra la cama; se avisa si el diseño excede la circunferencia.
- **Corrección de barrido:** posproceso sobre bloques fill (patrón de barrido) e image
  bidireccionales; las líneas de vuelta se desplazan en el sentido del barrido de ida, con el
  desfase interpolado por velocidad.
- **Prueba de materiales:** `material_test` genera un `Document` normal (una capa por celda y otra
  de rótulos con una fuente de trazo sencillo propia), así que pasa por el mismo pipeline que
  cualquier diseño.

## Convenciones

- **Unidades:** milímetros; velocidades en mm/min; potencia de 0 a 100 %; ángulos en grados en
  los ajustes y radianes en el cálculo interno.
- **Sistema de coordenadas:** el de la máquina, con origen abajo a la izquierda y Y hacia arriba.
- **SVG** tiene Y hacia abajo, por lo que se voltea al convertir a `Job` (`--no-flip` lo desactiva
  en el CLI). **DXF** ya es Y hacia arriba y no se voltea.
- **Imágenes:** la fila 0 de un bitmap es la de arriba; el barrido hace la conversión al sistema
  de la máquina.
- **Polilíneas cerradas** no repiten el primer punto al final. La pertenencia a una región sigue la
  regla par-impar: profundidad 0 = pieza, 1 = agujero, 2 = isla dentro del agujero.
- **Kerf:** `kerf_offset_mm` es cuánto se desplaza cada trayectoria (normalmente la mitad del ancho
  del corte). **Positivo = la pieza sale del tamaño dibujado**: los contornos exteriores se
  agrandan y los agujeros se encogen. Negativo es lo contrario; cero no cambia nada.
- **Errores:** las entradas degeneradas (vacías, con NaN, de área cero) se ignoran sin pánico y lo
  que no se puede importar se reporta con advertencias, nunca en silencio.
- **Pruebas:** la salida observable (G-code, rellenos, bitmaps) se verifica con archivos dorados;
  ver [CONTRIBUTING.md](../CONTRIBUTING.md).
