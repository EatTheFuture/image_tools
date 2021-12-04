use std::{env, fs::File, io::Write, path::Path};

#[derive(Copy, Clone)]
struct Chromaticities {
    r: (f64, f64),
    g: (f64, f64),
    b: (f64, f64),
    w: (f64, f64),
}

const REC709_CHROMA: Chromaticities = Chromaticities {
    r: (0.640, 0.330),
    g: (0.300, 0.600),
    b: (0.150, 0.060),
    w: (0.3127, 0.3290),
};

const REC2020_CHROMA: Chromaticities = Chromaticities {
    r: (0.708, 0.292),
    g: (0.170, 0.797),
    b: (0.131, 0.046),
    w: (0.3127, 0.3290),
};

const ACES_2065_1_CHROMA: Chromaticities = Chromaticities {
    r: (0.73470, 0.26530),
    g: (0.00000, 1.00000),
    b: (0.00010, -0.07700),
    w: (0.32168, 0.33767),
};

const ACES_CG_CHROMA: Chromaticities = Chromaticities {
    r: (0.713, 0.293),
    g: (0.165, 0.830),
    b: (0.128, 0.044),
    w: (0.32168, 0.33767),
};

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();

    // Rec709
    {
        let chroma = REC709_CHROMA;
        let dest_path = Path::new(&out_dir).join("rec709_inc.rs");
        let mut f = File::create(&dest_path).unwrap();
        write_conversion_matrices("rec709", chroma, &mut f).unwrap();
    }

    // Rec2020
    {
        let chroma = REC2020_CHROMA;
        let dest_path = Path::new(&out_dir).join("rec2020_inc.rs");
        let mut f = File::create(&dest_path).unwrap();
        write_conversion_matrices("rec2020", chroma, &mut f).unwrap();
    }

    // ACES2065-1
    {
        let chroma = ACES_2065_1_CHROMA;
        let dest_path = Path::new(&out_dir).join("aces_2065_1_inc.rs");
        let mut f = File::create(&dest_path).unwrap();
        write_conversion_matrices("aces_2065_1", chroma, &mut f).unwrap();
    }

    // ACEScg
    {
        let chroma = ACES_CG_CHROMA;
        let dest_path = Path::new(&out_dir).join("aces_cg_inc.rs");
        let mut f = File::create(&dest_path).unwrap();
        write_conversion_matrices("aces_cg", chroma, &mut f).unwrap();
    }

    //--------
    // Test that our multiply function behaves as expected.
    // (Unit tests don't work inside build scripts, so we're
    // putting it here instead.)
    {
        let rec709_mat = rgb_to_xyz(REC709_CHROMA, 1.0);
        let aces_mat = rgb_to_xyz(ACES_2065_1_CHROMA, 1.0);
        let combined_mat = multiply(rec709_mat, aces_mat);

        let vec1 = [0.0, 0.0, 0.0];
        let vec2 = [1.0, 0.0, 0.0];
        let vec3 = [0.0, 1.0, 0.0];
        let vec4 = [0.0, 0.0, 1.0];
        let vec5 = [1.0, 1.0, 1.0];

        assert!(
            vec_max_diff(
                multiply_vec_mat(multiply_vec_mat(vec1, rec709_mat), aces_mat),
                multiply_vec_mat(vec1, combined_mat),
            ) < 0.000_000_000_000_001
        );
        assert!(
            vec_max_diff(
                multiply_vec_mat(multiply_vec_mat(vec2, rec709_mat), aces_mat),
                multiply_vec_mat(vec2, combined_mat),
            ) < 0.000_000_000_000_001
        );
        assert!(
            vec_max_diff(
                multiply_vec_mat(multiply_vec_mat(vec3, rec709_mat), aces_mat),
                multiply_vec_mat(vec3, combined_mat),
            ) < 0.000_000_000_000_001
        );
        assert!(
            vec_max_diff(
                multiply_vec_mat(multiply_vec_mat(vec4, rec709_mat), aces_mat),
                multiply_vec_mat(vec4, combined_mat),
            ) < 0.000_000_000_000_001
        );
        assert!(
            vec_max_diff(
                multiply_vec_mat(multiply_vec_mat(vec5, rec709_mat), aces_mat),
                multiply_vec_mat(vec5, combined_mat),
            ) < 0.000_000_000_000_001
        );
    }
}

/// Generates conversion matrices for the given color space chromaticities.
fn write_conversion_matrices(
    space_name: &str,
    chroma: Chromaticities,
    f: &mut File,
) -> std::io::Result<()> {
    // Utility matrices.
    let aces_to_xyz = rgb_to_xyz(ACES_2065_1_CHROMA, 1.0);
    let xyz_to_aces = inverse(aces_to_xyz);
    let adapt_d65_to_e = chromatic_adaptation_matrix((0.3127, 0.3290), (1.0 / 3.0, 1.0 / 3.0));

    // Matrices we're actually going to write out.
    let to_xyz = rgb_to_xyz(chroma, 1.0);
    let from_xyz = inverse(to_xyz);
    let to_xyz_d65 = multiply(to_xyz, adapt_d65_to_e);
    let from_xyz_d65 = inverse(to_xyz_d65);
    let to_aces = multiply(to_xyz, xyz_to_aces);
    let from_aces = multiply(aces_to_xyz, from_xyz);

    f.write_all(format!("pub mod {} {{\n", space_name).as_bytes())?;
    write_matrix("TO_XYZ", to_xyz, f)?;
    write_matrix("FROM_XYZ", from_xyz, f)?;
    write_matrix("TO_XYZ_D65", to_xyz_d65, f)?;
    write_matrix("FROM_XYZ_D65", from_xyz_d65, f)?;
    write_matrix("TO_ACES_2056_1", to_aces, f)?;
    write_matrix("FROM_ACES_2065_1", from_aces, f)?;
    f.write_all(b"\n}\n")?;

    Ok(())
}

fn write_matrix(name: &str, matrix: [[f64; 3]; 3], f: &mut File) -> std::io::Result<()> {
    f.write_all(
        format!(
            r#"
    pub const {}: [[f32; 3]; 3] = [
        [{:.10}, {:.10}, {:.10}],
        [{:.10}, {:.10}, {:.10}],
        [{:.10}, {:.10}, {:.10}],
    ];
"#,
            name,
            matrix[0][0],
            matrix[0][1],
            matrix[0][2],
            matrix[1][0],
            matrix[1][1],
            matrix[1][2],
            matrix[2][0],
            matrix[2][1],
            matrix[2][2]
        )
        .as_bytes(),
    )?;

    Ok(())
}

/// Port of the RGBtoXYZ function from the ACES CTL reference implementation.
/// See lib/IlmCtlMath/CtlColorSpace.cpp in the CTL reference implementation.
///
/// This takes the chromaticities of an RGB colorspace and generates a
/// transform matrix from that space to XYZ.
///
/// * `chroma` is the chromaticities.
/// * `y` is the XYZ "Y" value that should map to RGB (1,1,1)
///
/// Note: the generated matrix will *not* do any chromatic adaptation.
/// It simply maps RGB colors to their absolute coordinates in XYZ space.
/// So, for example, unless the whitepoint of the RGB space is E, then
/// RGB = 1,1,1 will not map to XYZ = 1,1,1.
fn rgb_to_xyz(chroma: Chromaticities, y: f64) -> [[f64; 3]; 3] {
    // X and Z values of RGB value (1, 1, 1), or "white".
    let x = chroma.w.0 * y / chroma.w.1;
    let z = (1.0 - chroma.w.0 - chroma.w.1) * y / chroma.w.1;

    // Scale factors for matrix rows.
    let d = chroma.r.0 * (chroma.b.1 - chroma.g.1)
        + chroma.b.0 * (chroma.g.1 - chroma.r.1)
        + chroma.g.0 * (chroma.r.1 - chroma.b.1);

    let sr = (x * (chroma.b.1 - chroma.g.1)
        - chroma.g.0 * (y * (chroma.b.1 - 1.0) + chroma.b.1 * (x + z))
        + chroma.b.0 * (y * (chroma.g.1 - 1.0) + chroma.g.1 * (x + z)))
        / d;

    let sg = (x * (chroma.r.1 - chroma.b.1)
        + chroma.r.0 * (y * (chroma.b.1 - 1.0) + chroma.b.1 * (x + z))
        - chroma.b.0 * (y * (chroma.r.1 - 1.0) + chroma.r.1 * (x + z)))
        / d;

    let sb = (x * (chroma.g.1 - chroma.r.1)
        - chroma.r.0 * (y * (chroma.g.1 - 1.0) + chroma.g.1 * (x + z))
        + chroma.g.0 * (y * (chroma.r.1 - 1.0) + chroma.r.1 * (x + z)))
        / d;

    // Assemble the matrix.
    let mut mat = [[0.0; 3]; 3];

    mat[0][0] = sr * chroma.r.0;
    mat[0][1] = sg * chroma.g.0;
    mat[0][2] = sb * chroma.b.0;

    mat[1][0] = sr * chroma.r.1;
    mat[1][1] = sg * chroma.g.1;
    mat[1][2] = sb * chroma.b.1;

    mat[2][0] = sr * (1.0 - chroma.r.0 - chroma.r.1);
    mat[2][1] = sg * (1.0 - chroma.g.0 - chroma.g.1);
    mat[2][2] = sb * (1.0 - chroma.b.0 - chroma.b.1);

    mat
}

/// Creates a matrix to chromatically adapt CIE 1931 XYZ colors
/// from one whitepoint to another.
///
/// This uses the Hunt-Pointer-Estevez matrices and the Von Kries transform.
///
/// - `src_wp`: the xy chromaticity coordinates of the white point to convert from.
/// - `dst_wp`: the xy chromaticity coordinates of the white point to convert to.
fn chromatic_adaptation_matrix(src_wp: (f64, f64), dst_wp: (f64, f64)) -> [[f64; 3]; 3] {
    // The Hunt-Pointer-Estevez transformation matrix and its inverse.
    const TO_LMS_HUNT: [[f64; 3]; 3] = [
        [0.38971, 0.68898, -0.07868],
        [-0.22981, 1.18340, 0.04641],
        [0.0, 0.0, 1.0],
    ];
    const FROM_LMS_HUNT: [[f64; 3]; 3] = [
        [1.910196834052035, -1.1121238927878747, 0.20190795676749937],
        [0.3709500882486886, 0.6290542573926132, -0.0000080551421843],
        [0.0, 0.0, 1.0],
    ];

    // Compute the whitepoints' XYZ values.
    let src_wp_xyz = [
        src_wp.0 / src_wp.1,
        1.0,
        (1.0 - src_wp.0 - src_wp.1) / src_wp.1,
    ];
    let dst_wp_xyz = [
        dst_wp.0 / dst_wp.1,
        1.0,
        (1.0 - dst_wp.0 - dst_wp.1) / dst_wp.1,
    ];

    // Compute the whitepoints' LMS values.
    let src_wp_lms = multiply_vec_mat(src_wp_xyz, TO_LMS_HUNT);
    let dst_wp_lms = multiply_vec_mat(dst_wp_xyz, TO_LMS_HUNT);

    // Incorperate the ratio between the whitepoints into the LMS -> XYZ matrix.
    let wp_ratio = [
        dst_wp_lms[0] / src_wp_lms[0],
        dst_wp_lms[1] / src_wp_lms[1],
        dst_wp_lms[2] / src_wp_lms[2],
    ];
    let adapted_lms_to_xyz = [
        [
            FROM_LMS_HUNT[0][0] * wp_ratio[0],
            FROM_LMS_HUNT[0][1] * wp_ratio[1],
            FROM_LMS_HUNT[0][2] * wp_ratio[2],
        ],
        [
            FROM_LMS_HUNT[1][0] * wp_ratio[0],
            FROM_LMS_HUNT[1][1] * wp_ratio[1],
            FROM_LMS_HUNT[1][2] * wp_ratio[2],
        ],
        [
            FROM_LMS_HUNT[2][0] * wp_ratio[0],
            FROM_LMS_HUNT[2][1] * wp_ratio[1],
            FROM_LMS_HUNT[2][2] * wp_ratio[2],
        ],
    ];

    // Combine with the xyz -> lms matrix.
    multiply(TO_LMS_HUNT, adapted_lms_to_xyz)
}

/// Calculates the inverse of the given 3x3 matrix.
///
/// Ported to Rust from `gjInverse()` in IlmBase's Imath/ImathMatrix.h
fn inverse(m: [[f64; 3]; 3]) -> [[f64; 3]; 3] {
    let mut s = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    let mut t = m;

    // Forward elimination
    for i in 0..2 {
        let mut pivot = i;
        let mut pivotsize = t[i][i];

        if pivotsize < 0.0 {
            pivotsize = -pivotsize;
        }

        for j in (i + 1)..3 {
            let mut tmp = t[j][i];

            if tmp < 0.0 {
                tmp = -tmp;
            }

            if tmp > pivotsize {
                pivot = j;
                pivotsize = tmp;
            }
        }

        if pivotsize == 0.0 {
            panic!("Cannot invert singular matrix.");
        }

        if pivot != i {
            for j in 0..3 {
                let mut tmp = t[i][j];
                t[i][j] = t[pivot][j];
                t[pivot][j] = tmp;

                tmp = s[i][j];
                s[i][j] = s[pivot][j];
                s[pivot][j] = tmp;
            }
        }

        for j in (i + 1)..3 {
            let f = t[j][i] / t[i][i];

            for k in 0..3 {
                t[j][k] -= f * t[i][k];
                s[j][k] -= f * s[i][k];
            }
        }
    }

    // Backward substitution
    for i in (0..3).rev() {
        let f = t[i][i];

        if t[i][i] == 0.0 {
            panic!("Cannot invert singular matrix.");
        }

        for j in 0..3 {
            t[i][j] /= f;
            s[i][j] /= f;
        }

        for j in 0..i {
            let f = t[j][i];

            for k in 0..3 {
                t[j][k] -= f * t[i][k];
                s[j][k] -= f * s[i][k];
            }
        }
    }

    s
}

/// Multiplies two matrices together.
///
/// The result is a matrix that is equivalent to first
/// multiplying by `a` and then by `b`.
fn multiply(a: [[f64; 3]; 3], b: [[f64; 3]; 3]) -> [[f64; 3]; 3] {
    let mut c = [[0.0f64; 3]; 3];

    c[0][0] = (b[0][0] * a[0][0]) + (b[0][1] * a[1][0]) + (b[0][2] * a[2][0]);
    c[0][1] = (b[0][0] * a[0][1]) + (b[0][1] * a[1][1]) + (b[0][2] * a[2][1]);
    c[0][2] = (b[0][0] * a[0][2]) + (b[0][1] * a[1][2]) + (b[0][2] * a[2][2]);

    c[1][0] = (b[1][0] * a[0][0]) + (b[1][1] * a[1][0]) + (b[1][2] * a[2][0]);
    c[1][1] = (b[1][0] * a[0][1]) + (b[1][1] * a[1][1]) + (b[1][2] * a[2][1]);
    c[1][2] = (b[1][0] * a[0][2]) + (b[1][1] * a[1][2]) + (b[1][2] * a[2][2]);

    c[2][0] = (b[2][0] * a[0][0]) + (b[2][1] * a[1][0]) + (b[2][2] * a[2][0]);
    c[2][1] = (b[2][0] * a[0][1]) + (b[2][1] * a[1][1]) + (b[2][2] * a[2][1]);
    c[2][2] = (b[2][0] * a[0][2]) + (b[2][1] * a[1][2]) + (b[2][2] * a[2][2]);

    c
}

fn multiply_vec_mat(a: [f64; 3], b: [[f64; 3]; 3]) -> [f64; 3] {
    let mut c = [0.0f64; 3];

    c[0] = (a[0] * b[0][0]) + (a[1] * b[0][1]) + (a[2] * b[0][2]);
    c[1] = (a[0] * b[1][0]) + (a[1] * b[1][1]) + (a[2] * b[1][2]);
    c[2] = (a[0] * b[2][0]) + (a[1] * b[2][1]) + (a[2] * b[2][2]);

    c
}

//-------------------------------------------------------------
// Only used for testing.

fn vec_max_diff(a: [f64; 3], b: [f64; 3]) -> f64 {
    let x = (a[0] - b[0]).abs();
    let y = (a[1] - b[1]).abs();
    let z = (a[2] - b[2]).abs();

    x.max(y.max(z))
}
