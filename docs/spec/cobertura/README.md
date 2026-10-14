# 04 - Cobertura funcional real de Tlaku frente al inventario de LightBurn

Este documento se partió en un archivo por área (antes era un solo archivo de 128 KB). Aquí están el resumen y los totales; el detalle de cada fila está en los archivos de la tabla de abajo. La nota de auditoría, cómo leer las tablas, el historial por ola y las discrepancias vigentes están en [auditoria.md](auditoria.md). Los totales de abajo los produce un script que lee las filas de `diseno.md`, `capas.md` y `maquinas.md` (auditoría final de la ola 6D, 2026-10-14).

## 1. Resumen

### Por prioridad

|  | Hecho | Parcial | Solo motor/CLI | Falta | No aplica | Total |
|---|---:|---:|---:|---:|---:|---:|
| P0 | 68 | 0 | 0 | 0 | 0 | 68 |
| P1 | 86 | 2 | 0 | 0 | 0 | 88 |
| P2 | 59 | 2 | 0 | 0 | 0 | 61 |
| Total | 213 | 4 | 0 | 0 | 0 | 217 |

### Por área

|  | Hecho | Parcial | Solo motor/CLI | Falta | No aplica | Total |
|---|---:|---:|---:|---:|---:|---:|
| Diseño y edición | 100 | 0 | 0 | 0 | 0 | 100 |
| Capas, corte e imagen | 60 | 0 | 0 | 0 | 0 | 60 |
| Máquinas y avanzadas | 53 | 4 | 0 | 0 | 0 | 57 |

### Por área y prioridad

| Área | Prio | Hecho | Parcial | Solo motor/CLI | Falta | No aplica | Total |
|---|---|---:|---:|---:|---:|---:|---:|
| Diseño y edición | P0 | 32 | 0 | 0 | 0 | 0 | 32 |
| Diseño y edición | P1 | 39 | 0 | 0 | 0 | 0 | 39 |
| Diseño y edición | P2 | 29 | 0 | 0 | 0 | 0 | 29 |
| Capas, corte e imagen | P0 | 22 | 0 | 0 | 0 | 0 | 22 |
| Capas, corte e imagen | P1 | 23 | 0 | 0 | 0 | 0 | 23 |
| Capas, corte e imagen | P2 | 15 | 0 | 0 | 0 | 0 | 15 |
| Máquinas y avanzadas | P0 | 14 | 0 | 0 | 0 | 0 | 14 |
| Máquinas y avanzadas | P1 | 24 | 2 | 0 | 0 | 0 | 26 |
| Máquinas y avanzadas | P2 | 15 | 2 | 0 | 0 | 0 | 17 |

Tras la ola 6E (2026-10-14): **213 hechas, 4 parciales, 0 solo motor/CLI, 0 faltantes** de 217 (P0: 68 / 0 / 0 / 0). La ola 6E pasó a Hecho M23, M24, M26, M30, M32, M43, M46, M48 y M51 (máquinas), D18, D19, D35, D58, D60 y C35 (diseño y capas) y M52 (multilenguaje); D90 y M22 quedan Hecho con `.sc` y `.mage` fuera (formatos propietarios sin especificación pública). Antes de la parte de diseño y capas de la ola 6E: 197 / 20 / 0 / 0. Tras la auditoría de la ola 6D, el mismo día: 195 / 22 / 0 / 0. Antes de ella (tras la ola 6C, con las filas ya recontadas): 182 / 25 / 10 / 0 (P0: 65 / 3 / 0 / 0); el resumen de entonces decía 180 / 27 / 10 / 0 porque D04 y D12 habían pasado a Hecho en las filas sin recalcular los totales. Antes de la ola 5.5: 175 hechas, 27 parciales, 10 solo motor/CLI y 2 faltantes (P0: 64 / 4 / 0 / 0). Antes de la ola 5: 173 hechas, 26 parciales, 0 solo motor/CLI y 15 faltantes. Antes de la ola 4: 165 hechas, 23 parciales, 3 solo motor/CLI y 23 faltantes (P0: 62 / 6 / 0 / 0). Antes de la ola 3: 143 / 22 / 3 / 46. Antes de la ola 2: 120 / 27 / 6 / 61. Antes de la ola 1: 88 / 31 / 6 / 89 (P0: 53 / 13 / 1 / 1). El detalle de lo que cambió en la ola 6 está en [auditoria.md](auditoria.md).

**Lectura rápida.** De 217 funciones, 213 están hechas (98 %) y 4 son parciales; ninguna falta ni queda solo en el motor o el CLI. **Todas las P0 (el MVP) están hechas: 68 de 68.** En P1 hay 86 de 88 hechas y en P2, 59 de 61. Por área: diseño y edición 100 de 100, capas, corte e imagen 60 de 60 y máquinas y avanzadas 53 de 57. Las 4 parciales son: M25, M28, M40, M47 (detalle en la sección «Parciales que quedan» de [auditoria.md](auditoria.md)); todas necesitan hardware o documentación que no es pública. Las 10 funciones del galvo que estaban como "solo motor/CLI" se reclasificaron en la ola 6D: 6 pasaron a Hecho (M41, M42, M44, M45, C47 y C59) y 4 a Parcial (M30, M40, M43 y M46), porque la app ya las ofrece. **"Hecho" sigue sin significar probado en una máquina real:** no hay ninguna prueba con hardware real. Lo de Ruida, el K40, el galvo, la cámara (ojo de pez, cámara de red, cámara en el cabezal, buscador de centro), Print & Cut, el tiling, el alimentador, el control desde el celular y los ajustes nuevos de GRBL (DTR, envío síncrono, intercambio X/Y, desfase del puntero y home al conectar) se probó solo con emuladores, máquinas virtuales e imágenes sintéticas. La lista de lo que hay que validar, en orden de riesgo, está en [../../validacion-hardware.md](../../validacion-hardware.md).

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
