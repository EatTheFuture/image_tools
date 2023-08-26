#![allow(dead_code)]

/// A cubic bezier curve in the unit square, with the first and last
/// control points implicitly being (0,0) and (1,1).
///
/// `p2` and `p3` are the "handles" of the curve, allowing its shape to
/// be tweaked.  Both `p2` and `p3` must be within the unit square (i.e.
/// all coordinates within [0,1]).
///
/// This basically serves as a tweakable sigmoid function.
pub fn unit_cubic_bezier(x: f64, p2: [f64; 2], p3: [f64; 2]) -> f64 {
    if x <= 1.0e-10 {
        0.0
    } else if x >= 0.999_999_999 {
        1.0
    } else {
        let t = unit_cubic_bezier_1d_inv(x, p2[0], p3[0]);
        unit_cubic_bezier_1d(t, p2[1], p3[1])
    }
}

/// One dimension of a cubic bezier curve, with the first and last
/// control points implicitly always at (0, 0) and (1, 1).
///
/// - `t` is the evaluation parameter, and should always be in the
///   interval [0, 1].
/// - `b` and `c` are the 2nd and 3rd control points, and should also
///   always be in [0, 1].
#[inline(always)]
fn unit_cubic_bezier_1d(t: f64, b: f64, c: f64) -> f64 {
    debug_assert!(
        t >= 0.0 && t <= 1.0,
        "t must be within the interval [0.0, 1.0]"
    );
    debug_assert!(
        b >= 0.0 && b <= 1.0,
        "b must be within the interval [0.0, 1.0]"
    );
    debug_assert!(
        c >= 0.0 && c <= 1.0,
        "c must be within the interval [0.0, 1.0]"
    );
    let t2 = t * t;
    let t3 = t2 * t;
    let t_inv = 1.0 - t;
    let t_inv2 = t_inv * t_inv;

    (3.0 * b * t * t_inv2) + (3.0 * c * t2 * t_inv) + t3
}

/// Inverse of `unit_cubic_bezier_1d()`.
///
/// Like `unit_cubic_bezier_1d()`, all parameters should always be in
/// the interval [0, 1].
#[inline(always)]
fn unit_cubic_bezier_1d_inv(x: f64, b: f64, c: f64) -> f64 {
    debug_assert!(
        x >= 0.0 && x <= 1.0,
        "x must be within the interval [0.0, 1.0]"
    );
    debug_assert!(
        b >= 0.0 && b <= 1.0,
        "b must be within the interval [0.0, 1.0]"
    );
    debug_assert!(
        c >= 0.0 && c <= 1.0,
        "c must be within the interval [0.0, 1.0]"
    );

    // Find the solution that's in the interval [0, 1].  With the
    // constraints on `x`, `b`, and `c`, there should be precisely one.
    let roots = find_cubic_bezier_roots(x, [0.0, b, c, 1.0]);
    for root in roots {
        if root >= 0.0 && root <= 1.0 {
            // Found the solution.
            return root;
        }
    }

    // If we reach here, that means there's no solution.  However, since
    // the end points of the curve are fixed at 0.0 and 1.0, this isn't
    // possible: there will always be at least one real solution as long
    // as `x` is within [0, 1] as documented.
    panic!("No solution found.  Ensure that all parameters are within the interval [0.0, 1.0].")
}

/// Find the real roots at `x` of a cubic polynomial with Bernstein
/// coefficients [pa, pb, pc, pd].
///
/// There can be up to three distinct real roots, hence returning an
/// array of three numbers.  When there are fewer than three roots, the
/// real roots are put first in the array, and NaN's fill the remaining
/// array elements.
///
/// Note that it is possible for there to be *no* real roots, in which
/// case the entire array will be NaN's.
///
/// From https://pomax.github.io/bezierinfo/index.html#yforx
#[inline(always)]
fn find_cubic_bezier_roots(x: f64, bernstein_coefficients: [f64; 4]) -> [f64; 3] {
    use std::f64::consts::TAU;

    fn is_approximately_zero(a: f64) -> bool {
        a.abs() < 1.0e-14
    }

    // Convert the Bernstein coefficients to standard polynomial
    // coefficients.
    let (a, b, c, d) = {
        let pa = bernstein_coefficients[0];
        let pb = bernstein_coefficients[1];
        let pc = bernstein_coefficients[2];
        let pd = bernstein_coefficients[3];
        (
            -pa + (pb * 3.0) - (pc * 3.0) + pd,
            (pa * 3.0) - (pb * 6.0) + (pc * 3.0),
            -(pa * 3.0) + (pb * 3.0),
            pa - x,
        )
    };

    // Use Cardano's formula for finding the roots of a depressed cubic
    // curve.
    if is_approximately_zero(a) {
        // It's either a lower-order curve or there's no solution.  In
        // either case we have to handle it specially to avoid what would
        // be a divide-by-zero in the cubic solution.
        if is_approximately_zero(b) {
            if is_approximately_zero(c) {
                // There are no solutions.
                [f64::NAN; 3]
            } else {
                // Linear solution.
                [-d / c, f64::NAN, f64::NAN]
            }
        } else {
            // Quadratic solution.
            let q = (c * c - 4.0 * b * d).sqrt();
            [(q - c) / (b * 2.0), (-c - q) / (b * 2.0), f64::NAN]
        }
    } else {
        // Cubic solution.
        let b = b / a;
        let c = c / a;
        let d = d / a;
        let b3 = b / 3.0;
        let p = ((3.0 * c) - (b * b)) / 3.0;
        let p3 = p / 3.0;
        let q = ((2.0 * b * b * b) - (9.0 * b * c) + (27.0 * d)) / 27.0;
        let q2 = q / 2.0;
        let discriminant = (q2 * q2) + (p3 * p3 * p3);

        // Case 1: one distinct real root.
        if discriminant > 0.0 {
            let sd = discriminant.sqrt();
            let u1 = (-q2 + sd).cbrt();
            let v1 = (q2 + sd).cbrt();

            [u1 - v1 - b3, f64::NAN, f64::NAN]
        }
        // Case 2: two distinct real roots.
        else if discriminant == 0.0 {
            let u1 = if q2 < 0.0 { (-q2).cbrt() } else { -(q2.cbrt()) };

            [2.0 * u1 - b3, -u1 - b3, f64::NAN]
        }
        // Case 3: three distinct real roots.
        else {
            let mp3 = -p / 3.0;
            let r = (mp3 * mp3 * mp3).sqrt();
            let t = -q / (2.0 * r);
            let cosphi = t.clamp(-1.0, 1.0);
            let phi = cosphi.acos();
            let crtr = r.cbrt();
            let t1 = 2.0 * crtr;

            [
                t1 * (phi / 3.0).cos() - b3,
                t1 * ((phi + TAU) / 3.0).cos() - b3,
                t1 * ((phi + 2.0 * TAU) / 3.0).cos() - b3,
            ]
        }
    }
}
