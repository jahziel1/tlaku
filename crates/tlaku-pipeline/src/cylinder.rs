//! Corrección de cilindro y de cono (M46) para marcar con el galvo sobre superficies curvas.
//!
//! El diseño se dibuja plano. Con la coordenada a lo largo de la curvatura (`c`) en **mm de arco
//! sobre la superficie** y la coordenada a lo largo del eje del cilindro (`a`) en mm, ambas
//! medidas desde el centro de la cama (por donde pasa el eje del cilindro), se calcula dónde debe
//! caer el haz en el campo plano para que la marca quede donde se dibujó:
//!
//! - Ángulo del punto: `theta = c / R`, con `R` el radio de referencia.
//! - Altura de la superficie bajo la cima: convexo `R (1 - cos theta)` (más lejos del espejo),
//!   cóncavo `-R (1 - cos theta)` (más cerca). La distancia del espejo al punto es
//!   `h = D + esa altura`, con `D = mirror_distance_mm`.
//! - Proyección en perspectiva desde el espejo sobre el plano del campo (a distancia `D`):
//!   `c' = D * R sin(theta) / h` y `a' = D * a / h`. En el centro (`theta = 0`, `h = D`) es la
//!   identidad, como pide una lente F-theta enfocada en la cima.
//!
//! **Horizonte.** Un punto solo se puede marcar si el espejo lo ve. En un cilindro cóncavo (el
//! espejo dentro del tubo) se ve toda la pared hasta los 90 grados de arco. En uno convexo el rayo
//! tangente desde el espejo toca la superficie donde `cos theta = r / (r + D)` (con `r` el radio
//! local); más allá, la proyección `c'` baja otra vez (se pliega: dos puntos del diseño caerían en
//! el mismo lugar del campo) y el punto no es alcanzable. Por eso el ángulo se recorta a
//! `min(90 grados, acos(r / (r + D)))` y el punto se cuenta como recortado. En ese borde la
//! proyección vale `r sqrt(D / (D + 2r))`.
//!
//! Es un modelo geométrico propio (sala limpia: no se tomó de ningún programa) y **no está
//! verificado con hardware**; ignora el acoplamiento entre los dos espejos y la forma real del
//! campo de la lente.
//!
//! El cono (Taper Warp) escala la coordenada de curvatura con el diámetro local relativo al
//! mayor: `k(a) = d(a) / d_max`, de modo que un mismo ángulo abarca menos mm de superficie donde
//! el cono es más delgado. Si el cilindro también está activo, el radio local es `R * k(a)` y el
//! ángulo sigue siendo `c / R`.
//!
//! Como la transformación no es lineal, los tramos largos se subdividen cada [`MAX_STEP_MM`] mm
//! antes de proyectarlos.

use tlaku_core::Point;
use tlaku_core::toolpath::Move;
use tlaku_doc::{CylinderAxis, GalvoDevice};
use tlaku_i18n::tr;

/// Largo máximo (mm, en el diseño) de un tramo antes de proyectarlo.
pub(crate) const MAX_STEP_MM: f64 = 1.0;

/// Más allá de este ángulo (rad) la superficie ya no se ve desde arriba: el punto se recorta.
const MAX_THETA: f64 = std::f64::consts::FRAC_PI_2;

/// Tolerancia al decidir si un ángulo llegó al horizonte (los redondeos de `c / R`).
const HORIZON_EPS: f64 = 1e-9;

/// Ángulo máximo (rad) visible desde el espejo para un cilindro de radio local `r` a la distancia
/// `distance` (ver la nota «Horizonte» del módulo).
fn horizon(r: f64, distance: f64, concave: bool) -> f64 {
    if concave {
        MAX_THETA
    } else {
        (r / (r + distance)).acos().min(MAX_THETA)
    }
}

#[derive(Debug, Clone, Copy)]
struct Cyl {
    radius: f64,
    distance: f64,
    concave: bool,
    axis: CylinderAxis,
}

#[derive(Debug, Clone, Copy)]
struct Cone {
    axis: CylinderAxis,
    /// Diámetros en los extremos negativo y positivo del eje.
    d_neg: f64,
    d_pos: f64,
    half_len: f64,
}

impl Cone {
    fn d_max(&self) -> f64 {
        self.d_neg.max(self.d_pos)
    }

    /// Cociente diámetro local / diámetro mayor a la distancia `a` del centro (fuera del cono se
    /// queda en el extremo).
    fn ratio(&self, a: f64) -> f64 {
        let t = ((a + self.half_len) / (2.0 * self.half_len)).clamp(0.0, 1.0);
        (self.d_neg + (self.d_pos - self.d_neg) * t) / self.d_max()
    }
}

/// Transformación de cilindro y/o cono del perfil, centrada en `center`.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Projection {
    center: Point,
    cyl: Option<Cyl>,
    cone: Option<Cone>,
}

fn positive(name: &str, v: f64) -> Result<(), String> {
    if v.is_finite() && v > 0.0 {
        Ok(())
    } else {
        Err(tr!(
            "pipeline.cylinder.must_be_positive",
            name = name,
            value = v
        ))
    }
}

impl Projection {
    /// La transformación que pide el perfil galvo (`cylinder` y `taper`), o `None` si ninguna
    /// está activa. `center` es el centro de la cama (por donde pasa el eje del cilindro).
    pub fn from_device(g: &GalvoDevice, center: Point) -> Result<Option<Projection>, String> {
        let cyl = if g.cylinder.enabled {
            let c = &g.cylinder;
            positive(&tr!("pipeline.cylinder.cyl_diameter"), c.diameter_mm)?;
            positive(
                &tr!("pipeline.cylinder.cyl_mirror_distance"),
                c.mirror_distance_mm,
            )?;
            Some(Cyl {
                radius: c.diameter_mm / 2.0,
                distance: c.mirror_distance_mm,
                concave: c.concave,
                axis: c.axis,
            })
        } else {
            None
        };
        let cone = if g.taper.enabled {
            let t = &g.taper;
            positive(&tr!("pipeline.cylinder.cone_top"), t.top_diameter_mm)?;
            positive(&tr!("pipeline.cylinder.cone_bottom"), t.bottom_diameter_mm)?;
            positive(&tr!("pipeline.cylinder.cone_length"), t.length_mm)?;
            let (d_neg, d_pos) = if t.top_at_positive {
                (t.bottom_diameter_mm, t.top_diameter_mm)
            } else {
                (t.top_diameter_mm, t.bottom_diameter_mm)
            };
            Some(Cone {
                axis: t.axis,
                d_neg,
                d_pos,
                half_len: t.length_mm / 2.0,
            })
        } else {
            None
        };
        if let (Some(c), Some(k)) = (&cyl, &cone)
            && c.axis != k.axis
        {
            return Err(tr!("pipeline.cylinder.axis_mismatch"));
        }
        Ok((cyl.is_some() || cone.is_some()).then_some(Projection { center, cyl, cone }))
    }

    fn axis(&self) -> CylinderAxis {
        self.cyl
            .map(|c| c.axis)
            .or(self.cone.map(|k| k.axis))
            .unwrap_or_default()
    }

    /// Punto del campo plano que marca el punto `p` del diseño. El `bool` es `false` si el
    /// punto estaba fuera de lo visible desde el espejo y se recortó al borde.
    pub fn apply(&self, p: Point) -> (Point, bool) {
        let (dx, dy) = (p.x - self.center.x, p.y - self.center.y);
        // `a`: a lo largo del eje; `c`: a lo largo de la curvatura.
        let (a, c) = match self.axis() {
            CylinderAxis::X => (dx, dy),
            CylinderAxis::Y => (dy, dx),
        };
        let k = self.cone.map_or(1.0, |k| k.ratio(a));
        let (a2, c2, ok) = match self.cyl {
            None => (a, c * k, true),
            Some(cy) => {
                let r_ref = cy.radius;
                let r = r_ref * k;
                let raw = c / r_ref;
                let limit = horizon(r, cy.distance, cy.concave);
                let theta = raw.clamp(-limit, limit);
                let drop = r * (1.0 - theta.cos());
                let h = if cy.concave {
                    cy.distance - drop
                } else {
                    cy.distance + drop
                };
                // En un cóncavo muy cerrado el punto puede quedar a la altura del espejo.
                let h = h.max(cy.distance * 1e-3);
                let s = cy.distance / h;
                (
                    a * s,
                    r * theta.sin() * s,
                    (raw - theta).abs() <= HORIZON_EPS,
                )
            }
        };
        let q = match self.axis() {
            CylinderAxis::X => Point::new(self.center.x + a2, self.center.y + c2),
            CylinderAxis::Y => Point::new(self.center.x + c2, self.center.y + a2),
        };
        (q, ok)
    }

    /// Proyecta todos los movimientos subdividiendo los cortes largos. Devuelve cuántos puntos
    /// se recortaron por no verse desde el espejo.
    pub fn project_moves(&self, moves: &mut Vec<Move>) -> usize {
        let mut clipped = 0;
        let mut out = Vec::with_capacity(moves.len());
        // Posición de la cabeza en coordenadas del diseño.
        let mut head: Option<Point> = None;
        for m in moves.iter() {
            match *m {
                Move::Travel { to } => {
                    let (q, ok) = self.apply(to);
                    clipped += usize::from(!ok);
                    out.push(Move::Travel { to: q });
                    head = Some(to);
                }
                Move::Dwell { to, ms, power_pct } => {
                    let (q, ok) = self.apply(to);
                    clipped += usize::from(!ok);
                    out.push(Move::Dwell {
                        to: q,
                        ms,
                        power_pct,
                    });
                    head = Some(to);
                }
                Move::Cut { to, power_pct } => {
                    let from = head.unwrap_or(to);
                    let n = (from.dist(to) / MAX_STEP_MM).ceil().max(1.0) as usize;
                    for j in 1..=n {
                        let t = j as f64 / n as f64;
                        let p =
                            Point::new(from.x + (to.x - from.x) * t, from.y + (to.y - from.y) * t);
                        let (q, ok) = self.apply(p);
                        clipped += usize::from(!ok);
                        out.push(Move::Cut { to: q, power_pct });
                    }
                    head = Some(to);
                }
            }
        }
        *moves = out;
        clipped
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tlaku_doc::{CylinderCorrection, TaperWarp};

    fn device(cyl: CylinderCorrection, taper: TaperWarp) -> GalvoDevice {
        GalvoDevice {
            cylinder: cyl,
            taper,
            ..GalvoDevice::default()
        }
    }

    fn cylinder(diameter: f64, distance: f64, axis: CylinderAxis, concave: bool) -> Projection {
        let g = device(
            CylinderCorrection {
                enabled: true,
                diameter_mm: diameter,
                mirror_distance_mm: distance,
                axis,
                concave,
            },
            TaperWarp::default(),
        );
        Projection::from_device(&g, Point::new(50.0, 50.0))
            .unwrap()
            .unwrap()
    }

    #[test]
    fn disabled_is_none() {
        let g = GalvoDevice::default();
        assert!(
            Projection::from_device(&g, Point::new(0.0, 0.0))
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn center_line_is_identity() {
        let p = cylinder(60.0, 160.0, CylinderAxis::X, false);
        let (q, ok) = p.apply(Point::new(80.0, 50.0));
        assert!(ok);
        assert!(
            (q.x - 80.0).abs() < 1e-9 && (q.y - 50.0).abs() < 1e-9,
            "{q:?}"
        );
    }

    #[test]
    fn near_axis_the_arc_length_is_preserved() {
        // Cerca de la cima, c' = c con error de tercer orden.
        let p = cylinder(60.0, 160.0, CylinderAxis::X, false);
        let (q, _) = p.apply(Point::new(50.0, 50.0 + 1.0));
        assert!((q.y - 51.0).abs() < 1e-3, "{q:?}");
    }

    #[test]
    fn convex_compresses_the_flanks() {
        // Un tramo de arco de 10 mm a un lado ocupa menos de 10 mm en el campo plano: la
        // superficie se aleja del espejo y baja de la vista.
        let p = cylinder(60.0, 160.0, CylinderAxis::X, false);
        let c0 = p.apply(Point::new(50.0, 50.0 + 20.0)).0.y - 50.0;
        let c1 = p.apply(Point::new(50.0, 50.0 + 30.0)).0.y - 50.0;
        assert!(c1 > c0, "debe seguir creciendo (monótona)");
        assert!(c1 - c0 < 10.0, "tramo comprimido: {}", c1 - c0);
        // Y el campo plano nunca pasa del radio proyectado.
        assert!(c1 < 30.0);
    }

    #[test]
    fn concave_is_the_opposite_of_convex_in_scale() {
        let cx = cylinder(60.0, 160.0, CylinderAxis::X, false);
        let cv = cylinder(60.0, 160.0, CylinderAxis::X, true);
        let a = cx.apply(Point::new(50.0, 50.0 + 25.0)).0.y - 50.0;
        let b = cv.apply(Point::new(50.0, 50.0 + 25.0)).0.y - 50.0;
        assert!(b > a, "la pared interior se acerca al espejo: {a} {b}");
    }

    #[test]
    fn matches_the_closed_form() {
        // Verificación con la fórmula escrita a mano: R = 30, D = 150, c = 20.
        let p = cylinder(60.0, 150.0, CylinderAxis::X, false);
        let theta: f64 = 20.0 / 30.0;
        let h = 150.0 + 30.0 * (1.0 - theta.cos());
        let expected = 150.0 * 30.0 * theta.sin() / h;
        let (q, ok) = p.apply(Point::new(50.0, 70.0));
        assert!(ok);
        assert!(
            (q.y - 50.0 - expected).abs() < 1e-9,
            "{} vs {expected}",
            q.y
        );
        // La coordenada a lo largo del eje se escala con D / h.
        let (q2, _) = p.apply(Point::new(60.0, 70.0));
        assert!((q2.x - 50.0 - 10.0 * 150.0 / h).abs() < 1e-9);
    }

    #[test]
    fn y_axis_swaps_the_roles() {
        let px = cylinder(60.0, 160.0, CylinderAxis::X, false);
        let py = cylinder(60.0, 160.0, CylinderAxis::Y, false);
        let a = px.apply(Point::new(63.0, 71.0)).0;
        let b = py.apply(Point::new(71.0, 63.0)).0;
        // Con X el punto (63, 71) es a=13, c=21; con Y el (71, 63) es a=13, c=21 también.
        assert!((a.x - 50.0 - (b.y - 50.0)).abs() < 1e-9);
        assert!((a.y - 50.0 - (b.x - 50.0)).abs() < 1e-9);
    }

    #[test]
    fn beyond_the_horizon_is_clipped() {
        let p = cylinder(20.0, 100.0, CylinderAxis::X, false);
        // Un arco de 25 mm en un cilindro de radio 10 pasa de 90 grados.
        let (_, ok) = p.apply(Point::new(50.0, 75.0));
        assert!(!ok);
        let mut moves = vec![
            Move::Travel {
                to: Point::new(50.0, 50.0),
            },
            Move::Cut {
                to: Point::new(50.0, 80.0),
                power_pct: 10.0,
            },
        ];
        assert!(p.project_moves(&mut moves) > 0);
    }

    #[test]
    fn long_cuts_are_subdivided_and_keep_power() {
        let p = cylinder(60.0, 160.0, CylinderAxis::X, false);
        let mut moves = vec![
            Move::Travel {
                to: Point::new(30.0, 50.0),
            },
            Move::Cut {
                to: Point::new(70.0, 50.0),
                power_pct: 33.0,
            },
        ];
        assert_eq!(p.project_moves(&mut moves), 0);
        assert_eq!(moves.len(), 1 + 40);
        assert!(moves[1..].iter().all(|m| matches!(
            m,
            Move::Cut { power_pct, .. } if *power_pct == 33.0
        )));
        // Una línea paralela al eje sobre la cima se queda recta y de largo 40.
        let last = moves.last().unwrap().to();
        assert!((last.x - 70.0).abs() < 1e-9 && (last.y - 50.0).abs() < 1e-9);
    }

    #[test]
    fn taper_scales_curvature_by_local_diameter() {
        let g = device(
            CylinderCorrection::default(),
            TaperWarp {
                enabled: true,
                top_diameter_mm: 80.0,
                bottom_diameter_mm: 40.0,
                length_mm: 100.0,
                axis: CylinderAxis::X,
                top_at_positive: true,
            },
        );
        let p = Projection::from_device(&g, Point::new(0.0, 0.0))
            .unwrap()
            .unwrap();
        // Extremo superior (+X): diámetro mayor, sin cambio. Inferior: mitad.
        let top = p.apply(Point::new(50.0, 10.0)).0;
        let bottom = p.apply(Point::new(-50.0, 10.0)).0;
        let mid = p.apply(Point::new(0.0, 10.0)).0;
        assert!((top.y - 10.0).abs() < 1e-9);
        assert!((bottom.y - 5.0).abs() < 1e-9);
        assert!((mid.y - 7.5).abs() < 1e-9);
        // Sin tocar la coordenada a lo largo del eje.
        assert!((top.x - 50.0).abs() < 1e-9 && (bottom.x + 50.0).abs() < 1e-9);
        // Con el lado superior al revés, se invierte.
        let g2 = device(
            CylinderCorrection::default(),
            TaperWarp {
                top_at_positive: false,
                ..g.taper.clone()
            },
        );
        let p2 = Projection::from_device(&g2, Point::new(0.0, 0.0))
            .unwrap()
            .unwrap();
        assert!((p2.apply(Point::new(50.0, 10.0)).0.y - 5.0).abs() < 1e-9);
    }

    #[test]
    fn invalid_parameters_and_axis_mismatch_are_rejected() {
        let mut g = device(
            CylinderCorrection {
                enabled: true,
                diameter_mm: 0.0,
                ..CylinderCorrection::default()
            },
            TaperWarp::default(),
        );
        let e = Projection::from_device(&g, Point::new(0.0, 0.0)).unwrap_err();
        assert!(e.contains("diámetro"), "{e}");
        g.cylinder.diameter_mm = 50.0;
        g.taper.enabled = true;
        g.taper.axis = CylinderAxis::Y;
        let e = Projection::from_device(&g, Point::new(0.0, 0.0)).unwrap_err();
        assert!(e.contains("mismo eje"), "{e}");
    }
    /// Casos de referencia calculados a mano con la fórmula del módulo (ver su documentación):
    /// `theta = c / R`, `h = D +- R (1 - cos theta)` (más para convexo, menos para cóncavo),
    /// `s = D / h`, `c' = R sin(theta) s`, `a' = a s`. Los dos primeros usan ángulos con valores
    /// exactos (90 y 60 grados) para que la cuenta se pueda hacer sin calculadora; el resto son
    /// la misma cuenta con 10 cifras. Si alguien cambia el modelo, estos números se rompen a
    /// propósito: el modelo es propio y sin verificar con hardware, y cambiarlo exige decidirlo.
    #[test]
    fn hand_computed_reference_points() {
        let near = |got: f64, want: f64, what: &str| {
            assert!((got - want).abs() < 1e-8, "{what}: {got} contra {want}");
        };
        // 1. Cóncavo, R = 10, D = 100, c = pi R / 2 (90 grados): cos = 0, sin = 1,
        //    h = 100 - 10 = 90, s = 100 / 90. c' = 10 x 100 / 90 = 11,111...; a = 9 -> a' = 10.
        let p = cylinder(20.0, 100.0, CylinderAxis::X, true);
        let c = std::f64::consts::PI * 10.0 / 2.0;
        let (q, ok) = p.apply(Point::new(50.0 + 9.0, 50.0 + c));
        assert!(ok);
        near(q.x - 50.0, 10.0, "1 a'");
        near(q.y - 50.0, 100.0 / 9.0, "1 c'");
        // 2. Convexo, R = 30, D = 90, c = pi R / 3 (60 grados): cos = 1/2, sin = raíz(3)/2,
        //    drop = 15, h = 105, s = 90 / 105 = 6/7. a = 21 -> a' = 18 exacto;
        //    c' = 30 x (raíz(3)/2) x 6/7 = 22,2692246...
        let p = cylinder(60.0, 90.0, CylinderAxis::X, false);
        let c = std::f64::consts::PI * 30.0 / 3.0;
        let (q, _) = p.apply(Point::new(50.0 + 21.0, 50.0 + c));
        near(q.x - 50.0, 18.0, "2 a'");
        near(q.y - 50.0, 22.269_224_668_7, "2 c'");
        // 3. Cóncavo, mismos datos: drop = -15, h = 75, s = 90 / 75 = 6/5. a = 15 -> a' = 18;
        //    c' = 30 x (raíz(3)/2) x 6/5 = 31,1769145...
        let p = cylinder(60.0, 90.0, CylinderAxis::X, true);
        let (q, _) = p.apply(Point::new(50.0 + 15.0, 50.0 + c));
        near(q.x - 50.0, 18.0, "3 a'");
        near(q.y - 50.0, 31.176_914_536_2, "3 c'");
        // 4. Convexo, R = 30, D = 150, c = 20, a = 10: theta = 2/3, h = 156,4233822,
        //    a' = 9,5893592, c' = 17,7893105.
        let p = cylinder(60.0, 150.0, CylinderAxis::X, false);
        let (q, _) = p.apply(Point::new(60.0, 70.0));
        near(q.x - 50.0, 9.589_359_206_6, "4 a'");
        near(q.y - 50.0, 17.789_310_492_4, "4 c'");
        // 5. Cóncavo, mismos datos: h = 143,5766178, a' = 10,4473836, c' = 19,3810396.
        let p = cylinder(60.0, 150.0, CylinderAxis::X, true);
        let (q, _) = p.apply(Point::new(60.0, 70.0));
        near(q.x - 50.0, 10.447_383_583_3, "5 a'");
        near(q.y - 50.0, 19.381_039_587_1, "5 c'");
        // 6. Simétrico: el lado negativo da lo opuesto en la curvatura (c) y lo mismo a lo largo.
        let p = cylinder(60.0, 150.0, CylinderAxis::X, false);
        let (q, _) = p.apply(Point::new(60.0, 30.0));
        near(q.x - 50.0, 9.589_359_206_6, "6 a'");
        near(q.y - 50.0, -17.789_310_492_4, "6 c'");
    }

    /// El horizonte de un cilindro convexo es el rayo tangente desde el espejo: `cos theta = R /
    /// (R + D)`. Ahí la proyección vale `R sqrt(D / (D + 2R))` (se deduce de la fórmula: h = D
    /// (D + 2R) / (D + R) y sin theta = sqrt(D (D + 2R)) / (D + R)). Con R = 10 y D = 80 son
    /// 10 x sqrt(0,8) = 8,94427191 y a = 10 se escala por s = D / h = 0,9 a 9. Más allá el punto
    /// se recorta a ese borde y se cuenta como no visible; justo antes sí se ve.
    #[test]
    fn the_convex_horizon_is_the_tangent_ray() {
        let p = cylinder(20.0, 80.0, CylinderAxis::X, false);
        let edge = 10.0 * (80.0_f64 / 100.0).sqrt();
        let theta_t = (10.0_f64 / 90.0).acos();
        let (q, ok) = p.apply(Point::new(60.0, 50.0 + 10.0 * theta_t));
        assert!(ok);
        assert!((q.y - 50.0 - edge).abs() < 1e-9, "{}", q.y - 50.0);
        assert!((q.x - 50.0 - 9.0).abs() < 1e-9, "{}", q.x - 50.0);
        for c in [15.0, 25.0, 1000.0] {
            let (q, ok) = p.apply(Point::new(50.0, 50.0 + c));
            assert!(!ok, "c = {c}");
            assert!((q.y - 50.0 - edge).abs() < 1e-9, "c = {c}: {}", q.y - 50.0);
        }
        // Antes del horizonte la proyección crece sin plegarse.
        let mut prev = 0.0;
        for i in 1..=14 {
            let (q, ok) = p.apply(Point::new(50.0, 50.0 + f64::from(i)));
            assert!(ok && q.y - 50.0 > prev, "c = {i}");
            prev = q.y - 50.0;
        }
        // Un cilindro cóncavo ve la pared hasta los 90 grados.
        let p = cylinder(20.0, 80.0, CylinderAxis::X, true);
        let (_, ok) = p.apply(Point::new(50.0, 50.0 + std::f64::consts::PI * 10.0 / 2.0));
        assert!(ok);
        let (_, ok) = p.apply(Point::new(50.0, 50.0 + 17.0));
        assert!(!ok);
    }

    /// Cono más cilindro (cálculo a mano). Cilindro de 60 mm (R = 30) a D = 150, cono de 60 mm
    /// en el extremo +X a 30 mm en el -X, largo 100, eje X. El factor del cono es 0,5 en a = -50
    /// y 0,75 en a = 0; el ángulo sigue siendo c / R. Con c = 15 (theta = 0,5) en a = -50:
    /// r = 15, drop = 15 (1 - cos 0,5) = 1,836261, h = 151,836261, s = 0,987906;
    /// a' = -49,39530 y c' = 15 sin(0,5) s = 7,104413. En a = 0 con k = 0,75: c' = 10,592567.
    #[test]
    fn cone_and_cylinder_combine_with_the_local_radius() {
        let g = device(
            CylinderCorrection {
                enabled: true,
                diameter_mm: 60.0,
                mirror_distance_mm: 150.0,
                axis: CylinderAxis::X,
                concave: false,
            },
            TaperWarp {
                enabled: true,
                top_diameter_mm: 60.0,
                bottom_diameter_mm: 30.0,
                length_mm: 100.0,
                axis: CylinderAxis::X,
                top_at_positive: true,
            },
        );
        let p = Projection::from_device(&g, Point::new(0.0, 0.0))
            .unwrap()
            .unwrap();
        let (q, ok) = p.apply(Point::new(-50.0, 15.0));
        assert!(ok);
        assert!((q.y - 7.104_412_679).abs() < 1e-8, "{}", q.y);
        assert!(
            (q.x + 50.0 * 150.0 / 151.836_261_572).abs() < 1e-6,
            "{}",
            q.x
        );
        let (q, _) = p.apply(Point::new(0.0, 15.0));
        assert!((q.y - 10.592_567_374).abs() < 1e-8, "{}", q.y);
        // Extremo grueso: k = 1 y es el cilindro solo (cálculo 6 de arriba, a lo largo = 50).
        let (q, _) = p.apply(Point::new(50.0, 15.0));
        assert!((q.y - 14.039_041_461).abs() < 1e-8, "{}", q.y);
        assert!((q.x - 48.805_081_398).abs() < 1e-8, "{}", q.x);
    }

    /// La cuadrícula de referencia de la vista previa: el centro y su eje no se mueven; todo lo
    /// demás queda dentro de ±`R sqrt(D / (D + 2R))` en la curvatura (convexo).
    #[test]
    fn the_projected_grid_stays_inside_the_horizon_square() {
        let p = cylinder(60.0, 160.0, CylinderAxis::X, false);
        let limit = 30.0 * (160.0_f64 / 220.0).sqrt();
        for i in -20..=20 {
            let c = f64::from(i) * 2.5;
            let (q, _) = p.apply(Point::new(50.0, 50.0 + c));
            assert!((q.y - 50.0).abs() <= limit + 1e-9);
            assert!((q.x - 50.0).abs() < 1e-12, "el eje no se mueve");
        }
    }
}
