//! Geometría del campo: milímetros a unidades de 16 bits y corrección paramétrica del campo.
//!
//! Las posiciones de la tarjeta son enteros de 16 bits con el centro en `0x8000`; el campo de la
//! lente F-theta (`field_mm`) cubre las 65536 posiciones de cada eje (docs/hardware/galvo.md,
//! secciones 2.3 y 3.1: Bryce Schroeder y la documentación de LightBurn). Una unidad vale
//! `field_mm / 65536` (1,68 micras en 110 mm).
//!
//! La corrección se aplica **en el anfitrión y antes de cuantizar** (sección 3.2: así no se
//! depende del formato `.cor`). La parametrización es **propia de Tlaku**: no pretende ser
//! idéntica a la de EZCAD ni a la de LightBurn, que no la documentan. Pasos, sobre coordenadas
//! con origen en el centro del campo y `h = field_mm / 2`:
//!
//! 1. escala por eje, luego giro de `rotation_deg` (antihorario) alrededor del centro;
//! 2. barril/cojín: `x' = x (1 + bx r²)`, `y' = y (1 + by r²)` con `r² = u² + v²` y `(u, v) =
//!    (x, y) / h`; con `b > 0` las orillas se alejan del centro;
//! 3. trapecio: `x' = x (1 + tx v)`, `y' = y (1 + ty u)`;
//! 4. sesgo: `x' = x + kx v h`, `y' = y + ky u h`;
//! 5. desplazamiento `offset` (mm);
//! 6. intercambio de ejes (`swap_xy`) y negación por eje (`invert`), que describen cómo está
//!    montado el cabezal;
//! 7. la tabla de un `.cor` de EZCAD, si el perfil trae `cor_file`: se suma, en unidades de la
//!    tarjeta, al punto ya asignado a los ejes (ver [`crate::cor`]; lectura inferida, sin verificar
//!    con hardware). Se aplica **en el anfitrión**: nunca se escribe a la tarjeta.
//!
//! La inversa (para ida y vuelta y vistas previas) se resuelve con Newton sobre la función
//! directa, que es suave y monótona con los límites permitidos.

use crate::cor::CorGrid;
use std::fmt;
use std::path::Path;
use std::sync::Arc;
use tlaku_doc::FieldCorrection;
use tlaku_i18n::tr;

/// Posiciones por eje.
pub const AXIS_STEPS: f64 = 65536.0;
/// Centro de cada eje.
pub const CENTER: u16 = 0x8000;
/// Margen (en unidades) por el que un punto en el borde exacto del campo se satura en vez de
/// rechazarse: el borde derecho (+field/2) cae en 65536 y se recorta a 65535 (1 unidad).
const EDGE_SLACK: f64 = 1.0;

/// Límite de barril, trapecio y sesgo: con más, la función deja de ser monótona en el campo.
pub const MAX_NONLINEAR: f64 = 0.3;
const MIN_SCALE: f64 = 0.1;
const MAX_SCALE: f64 = 10.0;

#[derive(Debug, Clone, PartialEq)]
pub enum CalibError {
    BadField(f64),
    NonFinite(String),
    BadScale(f64),
    NonlinearTooLarge {
        what: String,
        value: f64,
    },
    /// El punto corregido cae fuera de las 65536 posiciones del eje.
    OutOfField {
        axis: char,
        mm: f64,
    },
    /// El archivo `.cor` del perfil no se pudo leer o no se puede aplicar (mensaje de `cor`).
    Cor(String),
}

impl fmt::Display for CalibError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CalibError::BadField(v) => {
                write!(f, "{}", tr!("galvo.calib.bad_field", v = v))
            }
            CalibError::NonFinite(w) => write!(f, "{}", tr!("galvo.calib.non_finite", what = w)),
            CalibError::BadScale(v) => {
                write!(
                    f,
                    "{}",
                    tr!(
                        "galvo.calib.bad_scale",
                        v = v,
                        min = MIN_SCALE,
                        max = MAX_SCALE
                    )
                )
            }
            CalibError::NonlinearTooLarge { what, value } => write!(
                f,
                "{}",
                tr!(
                    "galvo.calib.nonlinear_too_large",
                    what = what,
                    value = value,
                    max = MAX_NONLINEAR
                )
            ),
            CalibError::OutOfField { axis, mm } => write!(
                f,
                "{}",
                tr!(
                    "galvo.calib.out_of_field",
                    axis = axis,
                    mm = format!("{mm:.3}")
                )
            ),
            CalibError::Cor(e) => write!(f, "{}", tr!("galvo.calib.cor", error = e)),
        }
    }
}

impl std::error::Error for CalibError {}

/// Unidades de la tarjeta por milímetro.
pub fn units_per_mm(field_mm: f64) -> f64 {
    AXIS_STEPS / field_mm
}

/// Cuantiza milímetros desde el centro del campo a una posición de 16 bits. Falla fuera del
/// campo; un punto en el borde exacto se satura.
pub fn mm_to_units(mm: f64, field_mm: f64, axis: char) -> Result<u16, CalibError> {
    if !(mm.is_finite() && field_mm.is_finite() && field_mm > 0.0) {
        return Err(CalibError::NonFinite(tr!("galvo.calib.what_position")));
    }
    let u = f64::from(CENTER) + mm * units_per_mm(field_mm);
    if !(-0.5..=65535.0 + EDGE_SLACK + 1e-9).contains(&u) {
        return Err(CalibError::OutOfField { axis, mm });
    }
    Ok(u.round().clamp(0.0, 65535.0) as u16)
}

/// Milímetros desde el centro del campo para una posición de 16 bits.
pub fn units_to_mm(u: u16, field_mm: f64) -> f64 {
    (f64::from(u) - f64::from(CENTER)) / units_per_mm(field_mm)
}

/// Corrección del campo ya validada, con la directa y la inversa.
#[derive(Debug, Clone)]
pub struct FieldMap {
    field_mm: f64,
    c: FieldCorrection,
    sin: f64,
    cos: f64,
    /// Tabla del `.cor` del perfil, ya leída.
    cor: Option<Arc<CorGrid>>,
}

impl FieldMap {
    /// Identidad.
    pub fn identity(field_mm: f64) -> Result<Self, CalibError> {
        Self::new(field_mm, &FieldCorrection::default())
    }

    /// Valida la corrección y, si el perfil trae `cor_file`, lee ese archivo (solo lectura).
    pub fn new(field_mm: f64, c: &FieldCorrection) -> Result<Self, CalibError> {
        let map = Self::without_cor(field_mm, c)?;
        match c.cor_file.as_deref().map(str::trim) {
            Some(path) if !path.is_empty() => {
                let grid = CorGrid::load(Path::new(path), c.cor_transpose, c.cor_invert)
                    .map_err(|e| CalibError::Cor(format!("{path}: {e}")))?;
                Ok(map.with_cor(grid))
            }
            _ => Ok(map),
        }
    }

    /// Añade la tabla de un `.cor` ya leída (para pruebas y para quien la carga por su cuenta).
    pub fn with_cor(mut self, grid: CorGrid) -> Self {
        self.cor = Some(Arc::new(grid));
        self
    }

    /// ¿Se aplica la tabla de un `.cor`?
    pub fn has_cor(&self) -> bool {
        self.cor.is_some()
    }

    /// Como [`new`](Self::new) pero sin leer el `.cor` aunque el perfil lo nombre.
    pub fn without_cor(field_mm: f64, c: &FieldCorrection) -> Result<Self, CalibError> {
        if !(field_mm.is_finite() && field_mm > 0.0) {
            return Err(CalibError::BadField(field_mm));
        }
        let finite = |v: f64, w: String| {
            if v.is_finite() {
                Ok(())
            } else {
                Err(CalibError::NonFinite(w))
            }
        };
        for v in c.scale {
            finite(v, tr!("galvo.calib.what_scale"))?;
            if !(MIN_SCALE..=MAX_SCALE).contains(&v) {
                return Err(CalibError::BadScale(v));
            }
        }
        for v in c.offset {
            finite(v, tr!("galvo.calib.what_offset"))?;
        }
        finite(c.rotation_deg, tr!("galvo.calib.what_rotation"))?;
        for (vals, what) in [
            (c.bulge, tr!("galvo.calib.what_barrel")),
            (c.trapezoid, tr!("galvo.calib.what_trapezoid")),
            (c.skew, tr!("galvo.calib.what_skew")),
        ] {
            for v in vals {
                finite(v, what.clone())?;
                if v.abs() > MAX_NONLINEAR {
                    return Err(CalibError::NonlinearTooLarge { what, value: v });
                }
            }
        }
        let rad = c.rotation_deg.to_radians();
        Ok(FieldMap {
            field_mm,
            c: c.clone(),
            sin: rad.sin(),
            cos: rad.cos(),
            cor: None,
        })
    }

    pub fn field_mm(&self) -> f64 {
        self.field_mm
    }

    /// Pasos 1 a 5: del diseño (mm desde el centro) al diseño corregido, antes de los ejes.
    fn correct(&self, x: f64, y: f64) -> (f64, f64) {
        let h = self.field_mm / 2.0;
        let (x, y) = (x * self.c.scale[0], y * self.c.scale[1]);
        let (x, y) = (x * self.cos - y * self.sin, x * self.sin + y * self.cos);
        let (u, v) = (x / h, y / h);
        let r2 = u * u + v * v;
        let (x, y) = (
            x * (1.0 + self.c.bulge[0] * r2),
            y * (1.0 + self.c.bulge[1] * r2),
        );
        let (u, v) = (x / h, y / h);
        let (x, y) = (
            x * (1.0 + self.c.trapezoid[0] * v),
            y * (1.0 + self.c.trapezoid[1] * u),
        );
        let (u, v) = (x / h, y / h);
        let (x, y) = (x + self.c.skew[0] * v * h, y + self.c.skew[1] * u * h);
        (x + self.c.offset[0], y + self.c.offset[1])
    }

    /// Paso 6: asignación a los ejes del galvo.
    fn to_axes(&self, x: f64, y: f64) -> (f64, f64) {
        let (mut a, mut b) = if self.c.swap_xy { (y, x) } else { (x, y) };
        if self.c.invert[0] {
            a = -a;
        }
        if self.c.invert[1] {
            b = -b;
        }
        (a, b)
    }

    fn undo_axes(&self, a: f64, b: f64) -> (f64, f64) {
        let a = if self.c.invert[0] { -a } else { a };
        let b = if self.c.invert[1] { -b } else { b };
        if self.c.swap_xy { (b, a) } else { (a, b) }
    }

    /// Desplazamiento (mm) que suma la tabla del `.cor` a la posición `(a, b)` de los ejes.
    fn cor_shift(&self, a: f64, b: f64) -> (f64, f64) {
        let Some(grid) = &self.cor else {
            return (0.0, 0.0);
        };
        let upm = units_per_mm(self.field_mm);
        let c = f64::from(CENTER);
        let (dx, dy) = grid.delta_units(c + a * upm, c + b * upm);
        (dx / upm, dy / upm)
    }

    /// Del diseño (mm desde el centro) a mm de los ejes del galvo.
    pub fn forward(&self, p: (f64, f64)) -> (f64, f64) {
        let (x, y) = self.correct(p.0, p.1);
        let (a, b) = self.to_axes(x, y);
        let (sa, sb) = self.cor_shift(a, b);
        (a + sa, b + sb)
    }

    /// Inversa de [`forward`](Self::forward). `None` si Newton no converge (no pasa con los
    /// límites que acepta `new`).
    pub fn inverse(&self, q: (f64, f64)) -> Option<(f64, f64)> {
        // Primero se quita la tabla del `.cor` (suave y pequeña: punto fijo).
        let q = if self.cor.is_some() {
            let mut p = q;
            for _ in 0..50 {
                let s = self.cor_shift(p.0, p.1);
                p = (q.0 - s.0, q.1 - s.1);
            }
            let s = self.cor_shift(p.0, p.1);
            if (p.0 + s.0 - q.0).abs().max((p.1 + s.1 - q.1).abs()) > 1e-9 {
                return None;
            }
            p
        } else {
            q
        };
        let (tx, ty) = self.undo_axes(q.0, q.1);
        // Punto de partida: quitar el desplazamiento y suponer lineal.
        let mut x = tx - self.c.offset[0];
        let mut y = ty - self.c.offset[1];
        for _ in 0..60 {
            let (fx, fy) = self.correct(x, y);
            let (ex, ey) = (fx - tx, fy - ty);
            if ex.abs().max(ey.abs()) < 1e-12 {
                return Some((x, y));
            }
            // Jacobiano por diferencias centrales.
            let e = 1e-6 * self.field_mm.max(1.0);
            let (axp, ayp) = self.correct(x + e, y);
            let (axm, aym) = self.correct(x - e, y);
            let (bxp, byp) = self.correct(x, y + e);
            let (bxm, bym) = self.correct(x, y - e);
            let (j11, j21) = ((axp - axm) / (2.0 * e), (ayp - aym) / (2.0 * e));
            let (j12, j22) = ((bxp - bxm) / (2.0 * e), (byp - bym) / (2.0 * e));
            let det = j11 * j22 - j12 * j21;
            if det.abs() < 1e-12 {
                return None;
            }
            x -= (j22 * ex - j12 * ey) / det;
            y -= (-j21 * ex + j11 * ey) / det;
        }
        let (fx, fy) = self.correct(x, y);
        ((fx - tx).abs().max((fy - ty).abs()) < 1e-9).then_some((x, y))
    }

    /// Diseño (mm desde el centro) a posición de 16 bits: corrige y cuantiza.
    pub fn to_units(&self, p: (f64, f64)) -> Result<(u16, u16), CalibError> {
        let (a, b) = self.forward(p);
        Ok((
            mm_to_units(a, self.field_mm, 'X')?,
            mm_to_units(b, self.field_mm, 'Y')?,
        ))
    }

    /// Posición de 16 bits a diseño (mm desde el centro), deshaciendo la corrección.
    pub fn from_units(&self, u: (u16, u16)) -> Option<(f64, f64)> {
        self.inverse((
            units_to_mm(u.0, self.field_mm),
            units_to_mm(u.1, self.field_mm),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn center_is_0x8000_and_scale_follows_the_lens() {
        assert_eq!(mm_to_units(0.0, 110.0, 'X').unwrap(), 0x8000);
        // 110 mm / 65536 = 1,678 micras por unidad.
        assert!((1000.0 / units_per_mm(110.0) - 1.678).abs() < 1e-3);
        assert!((1000.0 / units_per_mm(150.0) - 2.289).abs() < 1e-3);
        assert!((1000.0 / units_per_mm(200.0) - 3.052).abs() < 1e-3);
        assert!((1000.0 / units_per_mm(300.0) - 4.578).abs() < 1e-3);
        // Media lente a cada lado.
        assert_eq!(mm_to_units(-55.0, 110.0, 'X').unwrap(), 0);
        assert_eq!(mm_to_units(55.0, 110.0, 'X').unwrap(), 65535);
        // Un cuarto del campo.
        assert_eq!(mm_to_units(27.5, 110.0, 'X').unwrap(), 0x8000 + 16384);
    }

    #[test]
    fn points_outside_the_field_are_rejected_not_clamped() {
        assert!(matches!(
            mm_to_units(55.01, 110.0, 'X'),
            Err(CalibError::OutOfField { axis: 'X', .. })
        ));
        assert!(mm_to_units(-55.01, 110.0, 'Y').is_err());
        assert!(mm_to_units(f64::NAN, 110.0, 'X').is_err());
        assert!(mm_to_units(1.0, 0.0, 'X').is_err());
    }

    #[test]
    fn quantization_round_trip_error_is_half_a_unit() {
        let half = 0.5 / units_per_mm(110.0) + 1e-12;
        for i in 0..200 {
            let mm = -54.9 + 109.8 * f64::from(i) / 199.0;
            let u = mm_to_units(mm, 110.0, 'X').unwrap();
            assert!((units_to_mm(u, 110.0) - mm).abs() <= half, "{mm}");
        }
    }

    #[test]
    fn identity_changes_nothing() {
        let m = FieldMap::identity(150.0).unwrap();
        for p in [(0.0, 0.0), (10.0, -20.0), (-70.0, 70.0)] {
            let q = m.forward(p);
            assert!((q.0 - p.0).abs() < 1e-12 && (q.1 - p.1).abs() < 1e-12);
        }
        assert_eq!(m.to_units((0.0, 0.0)).unwrap(), (0x8000, 0x8000));
    }

    #[test]
    fn axis_flags_map_to_the_galvo_axes() {
        let c = FieldCorrection {
            invert: [true, false],
            ..Default::default()
        };
        let m = FieldMap::new(100.0, &c).unwrap();
        assert_eq!(m.forward((10.0, 20.0)), (-10.0, 20.0));
        let c = FieldCorrection {
            swap_xy: true,
            ..Default::default()
        };
        let m = FieldMap::new(100.0, &c).unwrap();
        assert_eq!(m.forward((10.0, 20.0)), (20.0, 10.0));
        let c = FieldCorrection {
            rotation_deg: 90.0,
            ..Default::default()
        };
        let m = FieldMap::new(100.0, &c).unwrap();
        let q = m.forward((10.0, 0.0));
        assert!(q.0.abs() < 1e-9 && (q.1 - 10.0).abs() < 1e-9);
        let c = FieldCorrection {
            scale: [2.0, 0.5],
            offset: [1.0, -1.0],
            ..Default::default()
        };
        let m = FieldMap::new(100.0, &c).unwrap();
        assert_eq!(m.forward((3.0, 4.0)), (7.0, 1.0));
    }

    fn full() -> FieldCorrection {
        FieldCorrection {
            scale: [1.02, 0.97],
            offset: [0.7, -0.4],
            rotation_deg: 1.3,
            invert: [false, true],
            swap_xy: true,
            bulge: [0.02, -0.015],
            trapezoid: [0.01, -0.02],
            skew: [0.005, 0.008],
            cor_file: None,
            cor_transpose: false,
            cor_invert: false,
        }
    }

    #[test]
    fn forward_then_inverse_returns_the_point() {
        let m = FieldMap::new(110.0, &full()).unwrap();
        let mut worst = 0.0f64;
        for i in -5..=5 {
            for j in -5..=5 {
                let p = (f64::from(i) * 9.0, f64::from(j) * 9.0);
                let q = m.forward(p);
                let back = m.inverse(q).expect("converge");
                worst = worst.max((back.0 - p.0).abs()).max((back.1 - p.1).abs());
            }
        }
        assert!(worst < 1e-9, "error de ida y vuelta {worst} mm");
    }

    #[test]
    fn quantized_round_trip_stays_within_a_unit() {
        let m = FieldMap::new(110.0, &full()).unwrap();
        let unit = 1.0 / units_per_mm(110.0);
        for i in -4..=4 {
            for j in -4..=4 {
                let p = (f64::from(i) * 10.0, f64::from(j) * 10.0);
                let u = m.to_units(p).unwrap();
                let back = m.from_units(u).unwrap();
                // La cuantización agrega media unidad por eje; la corrección la estira poco.
                assert!(
                    (back.0 - p.0).abs() < unit && (back.1 - p.1).abs() < unit,
                    "{p:?} -> {u:?} -> {back:?}"
                );
            }
        }
    }

    #[test]
    fn the_correction_is_monotonic_along_lines() {
        let m = FieldMap::new(110.0, &full()).unwrap();
        // Con swap_xy, el eje X del diseño controla el eje Y del galvo (negado).
        let mut prev = m.forward((-50.0, 10.0));
        for i in 1..=100 {
            let x = -50.0 + f64::from(i);
            let cur = m.forward((x, 10.0));
            // El eje galvo Y baja al subir x (invert[1]) y el eje X se mueve poco.
            assert!(cur.1 < prev.1, "x = {x}");
            prev = cur;
        }
    }

    #[test]
    fn nonlinear_terms_move_the_corners_the_expected_way() {
        let c = FieldCorrection {
            bulge: [0.05, 0.05],
            ..Default::default()
        };
        let m = FieldMap::new(100.0, &c).unwrap();
        let (x, y) = m.forward((40.0, 40.0));
        assert!(x > 40.0 && y > 40.0, "las esquinas se alejan");
        let (cx, _) = m.forward((1.0, 0.0));
        assert!((cx - 1.0).abs() < 1e-3, "el centro casi no se mueve");
    }

    #[test]
    fn invalid_parameters_are_rejected() {
        let bad = |f: &dyn Fn(&mut FieldCorrection)| {
            let mut c = FieldCorrection::default();
            f(&mut c);
            FieldMap::new(110.0, &c).unwrap_err()
        };
        assert!(matches!(
            bad(&|c| c.scale[0] = 0.0),
            CalibError::BadScale(_)
        ));
        assert!(matches!(
            bad(&|c| c.scale[1] = 50.0),
            CalibError::BadScale(_)
        ));
        assert!(matches!(
            bad(&|c| c.bulge[0] = 0.5),
            CalibError::NonlinearTooLarge { .. }
        ));
        assert!(matches!(
            bad(&|c| c.skew[1] = f64::NAN),
            CalibError::NonFinite(_)
        ));
        assert!(matches!(
            bad(&|c| c.rotation_deg = f64::INFINITY),
            CalibError::NonFinite(_)
        ));
        assert!(matches!(
            FieldMap::new(-1.0, &FieldCorrection::default()),
            Err(CalibError::BadField(_))
        ));
    }
    use crate::cor::{CorFormat, CorTable, GRID};

    /// Tabla de 65 x 65 con el mismo desplazamiento (dx, dy) en todos los nodos.
    fn flat_cor(dx: f64, dy: f64) -> CorGrid {
        let t = CorTable {
            format: CorFormat::Float,
            scale: 1.0,
            dx: vec![dx; GRID * GRID],
            dy: vec![dy; GRID * GRID],
        };
        CorGrid::from_table(&t, false, false).unwrap()
    }

    /// Cálculo a mano: en un campo de 100 mm hay 65536 / 100 = 655,36 unidades por mm. Una tabla
    /// que desplaza todos los nodos (655,36, -1310,72) unidades suma (+1, -2) mm a cada punto de
    /// los ejes; el centro pasa de 0x8000 a 32768 + 655,36 = 33423,36, que se cuantiza a 33423,
    /// y en Y a 32768 - 1310,72 = 31457,28, que se cuantiza a 31457.
    #[test]
    fn the_cor_table_is_added_in_card_units_before_quantizing() {
        let m = FieldMap::without_cor(100.0, &FieldCorrection::default())
            .unwrap()
            .with_cor(flat_cor(655.36, -1310.72));
        assert!(m.has_cor());
        let q = m.forward((10.0, 20.0));
        assert!(
            (q.0 - 11.0).abs() < 1e-9 && (q.1 - 18.0).abs() < 1e-9,
            "{q:?}"
        );
        assert_eq!(m.to_units((0.0, 0.0)).unwrap(), (33423, 31457));
        // Sin tabla, el centro sigue siendo 0x8000.
        let plain = FieldMap::without_cor(100.0, &FieldCorrection::default()).unwrap();
        assert_eq!(plain.to_units((0.0, 0.0)).unwrap(), (0x8000, 0x8000));
        assert!(!plain.has_cor());
    }

    /// La tabla actúa sobre los ejes de la tarjeta, o sea después de intercambiarlos y negarlos:
    /// con `swap_xy` el desplazamiento dx de la tabla mueve el eje que antes era Y del diseño.
    #[test]
    fn the_cor_table_acts_on_the_card_axes_after_swapping() {
        let c = FieldCorrection {
            swap_xy: true,
            ..FieldCorrection::default()
        };
        let m = FieldMap::without_cor(100.0, &c)
            .unwrap()
            .with_cor(flat_cor(655.36, 0.0));
        // (10, 20) se vuelve (20, 10) en los ejes y la tabla suma 1 mm al primero.
        let q = m.forward((10.0, 20.0));
        assert!(
            (q.0 - 21.0).abs() < 1e-9 && (q.1 - 10.0).abs() < 1e-9,
            "{q:?}"
        );
    }

    #[test]
    fn the_inverse_undoes_the_cor_table() {
        // Una tabla que no es plana: crece con la columna.
        let t = CorTable {
            format: CorFormat::Float,
            scale: 1.0,
            dx: (0..GRID * GRID).map(|i| (i % GRID) as f64 * 4.0).collect(),
            dy: (0..GRID * GRID).map(|i| (i / GRID) as f64 * -3.0).collect(),
        };
        let m = FieldMap::without_cor(110.0, &full())
            .unwrap()
            .with_cor(CorGrid::from_table(&t, false, false).unwrap());
        for i in -4..=4 {
            for j in -4..=4 {
                let p = (f64::from(i) * 9.0, f64::from(j) * 9.0);
                let back = m.inverse(m.forward(p)).expect("converge");
                assert!(
                    (back.0 - p.0).abs() < 1e-8 && (back.1 - p.1).abs() < 1e-8,
                    "{p:?} {back:?}"
                );
            }
        }
    }

    #[test]
    fn a_profile_with_a_missing_cor_file_is_an_error_not_a_silent_skip() {
        let c = FieldCorrection {
            cor_file: Some("/no/existe/lente.cor".into()),
            ..FieldCorrection::default()
        };
        let e = FieldMap::new(110.0, &c).unwrap_err();
        assert!(matches!(e, CalibError::Cor(_)), "{e:?}");
        // Una ruta vacía o en blanco equivale a no tener archivo.
        for blank in ["", "   "] {
            let c = FieldCorrection {
                cor_file: Some(blank.into()),
                ..FieldCorrection::default()
            };
            assert!(!FieldMap::new(110.0, &c).unwrap().has_cor());
        }
    }
    /// Los mismos números están fijados en `apps/desktop/src/features/machine/galvoCalib9.test.ts`:
    /// la interfaz tiene una copia del modelo (para el asistente de nueve puntos) y las dos pruebas
    /// juntas impiden que se separen.
    #[test]
    fn the_ts_mirror_numbers_are_pinned() {
        let m = FieldMap::without_cor(110.0, &full()).unwrap();
        for (p, want) in [
            ((0.0, 0.0), (-0.4, -0.7)),
            ((38.5, 38.5), (37.048_664_921, -40.312_345_662)),
            ((-38.5, 19.25), (17.144_920_998, 39.518_921_697)),
            ((10.0, -30.0), (-28.923_944_697, -11.425_716_223)),
        ] {
            let q = m.forward(p);
            assert!(
                (q.0 - want.0).abs() < 1e-8 && (q.1 - want.1).abs() < 1e-8,
                "{p:?} -> {q:?}"
            );
        }
    }
}
