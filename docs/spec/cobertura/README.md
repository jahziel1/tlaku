# 04 - Cobertura funcional real de Tlaku frente al inventario de LightBurn

Este documento se partió en un archivo por área (antes era un solo archivo de 128 KB). Aquí están el resumen y los totales; el detalle de cada fila está en los archivos de la tabla de abajo. La nota de auditoría, cómo leer las tablas, el historial por ola y las discrepancias vigentes están en [auditoria.md](auditoria.md). Los totales de abajo los produce un script que lee las filas de `diseno.md`, `capas.md` y `maquinas.md` (auditoría final de la ola 6D, 2026-10-14).

## 1. Resumen

### Por prioridad

|  | Hecho | Parcial | Solo motor/CLI | Falta | No aplica | Total |
|---|---:|---:|---:|---:|---:|---:|
| P0 | 68 | 0 | 0 | 0 | 0 | 68 |
| P1 | 79 | 9 | 0 | 0 | 0 | 88 |
| P2 | 48 | 13 | 0 | 0 | 0 | 61 |
| Total | 195 | 22 | 0 | 0 | 0 | 217 |

### Por área

|  | Hecho | Parcial | Solo motor/CLI | Falta | No aplica | Total |
|---|---:|---:|---:|---:|---:|---:|
| Diseño y edición | 94 | 6 | 0 | 0 | 0 | 100 |
| Capas, corte e imagen | 59 | 1 | 0 | 0 | 0 | 60 |
| Máquinas y avanzadas | 42 | 15 | 0 | 0 | 0 | 57 |

### Por área y prioridad

| Área | Prio | Hecho | Parcial | Solo motor/CLI | Falta | No aplica | Total |
|---|---|---:|---:|---:|---:|---:|---:|
| Diseño y edición | P0 | 32 | 0 | 0 | 0 | 0 | 32 |
| Diseño y edición | P1 | 37 | 2 | 0 | 0 | 0 | 39 |
| Diseño y edición | P2 | 25 | 4 | 0 | 0 | 0 | 29 |
| Capas, corte e imagen | P0 | 22 | 0 | 0 | 0 | 0 | 22 |
| Capas, corte e imagen | P1 | 22 | 1 | 0 | 0 | 0 | 23 |
| Capas, corte e imagen | P2 | 15 | 0 | 0 | 0 | 0 | 15 |
| Máquinas y avanzadas | P0 | 14 | 0 | 0 | 0 | 0 | 14 |
| Máquinas y avanzadas | P1 | 20 | 6 | 0 | 0 | 0 | 26 |
| Máquinas y avanzadas | P2 | 8 | 9 | 0 | 0 | 0 | 17 |

Tras la auditoría de la ola 6D (2026-10-14): **195 hechas, 22 parciales, 0 solo motor/CLI y 0 faltantes** de 217 (P0: 68 / 0 / 0 / 0). Antes de ella (tras la ola 6C, con las filas ya recontadas): 182 / 25 / 10 / 0 (P0: 65 / 3 / 0 / 0); el resumen de entonces decía 180 / 27 / 10 / 0 porque D04 y D12 habían pasado a Hecho en las filas sin recalcular los totales. Antes de la ola 5.5: 175 hechas, 27 parciales, 10 solo motor/CLI y 2 faltantes (P0: 64 / 4 / 0 / 0). Antes de la ola 5: 173 hechas, 26 parciales, 0 solo motor/CLI y 15 faltantes. Antes de la ola 4: 165 hechas, 23 parciales, 3 solo motor/CLI y 23 faltantes (P0: 62 / 6 / 0 / 0). Antes de la ola 3: 143 / 22 / 3 / 46. Antes de la ola 2: 120 / 27 / 6 / 61. Antes de la ola 1: 88 / 31 / 6 / 89 (P0: 53 / 13 / 1 / 1). El detalle de lo que cambió en la ola 6 está en [auditoria.md](auditoria.md).

**Lectura rápida.** De 217 funciones, 195 están hechas (90 %), 22 son parciales y ninguna falta ni queda solo en el motor o el CLI. **Todas las P0 (el MVP) están hechas: 68 de 68.** En P1 hay 79 de 88 hechas (90 %) y en P2, 48 de 61 (79 %). Por área: diseño y edición 94 de 100, capas, corte e imagen 59 de 60 y máquinas y avanzadas 42 de 57. Las 22 parciales son: D18, D19, D35, D58, D60, D90, C35, M22, M23, M24, M25, M26, M28, M30, M32, M40, M43, M46, M47, M48, M51 y M52 (detalle y criterio para la beta en la sección «Parciales que quedan» de [auditoria.md](auditoria.md)). Las 10 funciones del galvo que estaban como "solo motor/CLI" se reclasificaron en la ola 6D: 6 pasaron a Hecho (M41, M42, M44, M45, C47 y C59) y 4 a Parcial (M30, M40, M43 y M46), porque la app ya las ofrece. **"Hecho" sigue sin significar probado en una máquina real:** no hay ninguna prueba con hardware real. Lo de Ruida, el K40, el galvo, la cámara (ojo de pez, cámara de red, cámara en el cabezal, buscador de centro), Print & Cut, el tiling, el alimentador, el control desde el celular y los ajustes nuevos de GRBL (DTR, envío síncrono, intercambio X/Y, desfase del puntero y home al conectar) se probó solo con emuladores, máquinas virtuales e imágenes sintéticas. La lista de lo que hay que validar, en orden de riesgo, está en [../../validacion-hardware.md](../../validacion-hardware.md).

## Archivos por área

| Archivo | Contenido | Filas |
|---|---|---|
| [diseno.md](diseno.md) | 2.1 Diseño y edición (inventario 01) | D01 a D100 |
| [capas.md](capas.md) | 2.2 Capas, corte e imagen (inventario 02) | C01 a C60 |
| [maquinas.md](maquinas.md) | 2.3 Máquinas y funciones avanzadas (inventario 03) | M1 a M57 |
| [usuario-real.md](usuario-real.md) | 2.4 Lo que un usuario real necesita y el inventario no cubre bien (no cuenta en los totales) | |
| [lanzamiento.md](lanzamiento.md) | 3. Lo que falta para un primer lanzamiento público | |
| [diferenciales.md](diferenciales.md) | 4. Lo que Tlaku tiene y LightBurn no | |
| [auditoria.md](auditoria.md) | Nota de auditoría, cómo leer, historial por ola y discrepancias | |
