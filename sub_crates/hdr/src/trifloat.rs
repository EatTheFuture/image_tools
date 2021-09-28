//! Encoding/decoding for unsigned 32-bit trifloat numbers.
//!
//! The encoding uses 8 bits of mantissa per number, and 8 bits for the shared
//! exponent.  The bit layout is [mantissa 1, mantissa 2, mantissa 3, exponent].
//! The exponent is stored as an unsigned integer with a bias of 128.
//!
//! This is compatible with the RGBE format of the Radiance .hdr image file format.

/// Calculates 2.0^exp using IEEE bit fiddling.
///
/// Only works for integer exponents in the range [-126, 127]
/// due to IEEE 32-bit float limits.
#[inline(always)]
fn fiddle_exp2(exp: i32) -> f32 {
    use std::f32;
    f32::from_bits(((exp + 127) as u32) << 23)
}

/// Calculates a floor(log2(n)) using IEEE bit fiddling.
///
/// Because of IEEE floating point format, infinity and NaN
/// floating point values return 128, and subnormal numbers always
/// return -127.  These particular behaviors are not, of course,
/// mathemetically correct, but are actually desireable for the
/// calculations in this library.
#[inline(always)]
fn fiddle_log2(n: f32) -> i32 {
    use std::f32;
    ((f32::to_bits(n) >> 23) & 0b1111_1111) as i32 - 127
}

const EXP_BIAS: i32 = 128;

/// Encodes three floating point values into an unsigned 32-bit trifloat.
///
/// Warning: negative values and NaN's are _not_ supported by the trifloat
/// format.  There are debug-only assertions in place to catch such
/// values in the input floats.
#[inline]
pub fn encode(floats: [f32; 3]) -> [u8; 4] {
    debug_assert!(
        floats[0] >= 0.0
            && floats[1] >= 0.0
            && floats[2] >= 0.0
            && !floats[0].is_nan()
            && !floats[1].is_nan()
            && !floats[2].is_nan(),
        "trifloat::unsigned32::encode(): encoding to unsigned tri-floats only \
         works correctly for positive, non-NaN numbers, but the numbers passed \
         were: ({}, {}, {})",
        floats[0],
        floats[1],
        floats[2]
    );

    let largest = floats[0].max(floats[1].max(floats[2]));

    if largest <= 1.0e-32 {
        [0, 0, 0, 0]
    } else {
        let e = fiddle_log2(largest).max(-EXP_BIAS).min(255 - EXP_BIAS);
        let inv_multiplier = fiddle_exp2(-e + 7);
        let x = (floats[0] * inv_multiplier).min(255.0) as u8;
        let y = (floats[1] * inv_multiplier).min(255.0) as u8;
        let z = (floats[2] * inv_multiplier).min(255.0) as u8;

        [x, y, z, (e + EXP_BIAS) as u8]
    }
}

/// Decodes an unsigned 32-bit trifloat into three full floating point numbers.
///
/// This operation is lossless and cannot fail.
#[inline]
pub fn decode(trifloat: [u8; 4]) -> [f32; 3] {
    let multiplier = fiddle_exp2(trifloat[3] as i32 - EXP_BIAS - 7);

    [
        trifloat[0] as f32 * multiplier,
        trifloat[1] as f32 * multiplier,
        trifloat[2] as f32 * multiplier,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(floats: [f32; 3]) -> [f32; 3] {
        decode(encode(floats))
    }

    #[test]
    fn all_zeros() {
        let fs = [0.0f32, 0.0f32, 0.0f32];

        let tri = encode(fs);
        let fs2 = decode(tri);

        assert_eq!(tri, [0, 0, 0, 0]);
        assert_eq!(fs, fs2);
    }

    #[test]
    fn powers_of_two() {
        let fs = [8.0f32, 64.0f32, 0.5f32];
        assert_eq!(fs, round_trip(fs));
    }

    #[test]
    fn accuracy_01() {
        let mut n = 1.0;
        for _ in 0..128 {
            let [x, _, _] = round_trip([n, 0.0, 0.0]);
            assert_eq!(n, x);
            n += 1.0 / 128.0;
        }
    }

    #[test]
    #[should_panic]
    fn accuracy_02() {
        let mut n = 1.0;
        for _ in 0..256 {
            let [x, _, _] = round_trip([n, 0.0, 0.0]);
            assert_eq!(n, x);
            n += 1.0 / 256.0;
        }
    }

    #[test]
    fn integers() {
        for n in 0..=256 {
            let [x, _, _] = round_trip([n as f32, 0.0, 0.0]);
            assert_eq!(n as f32, x);
        }
    }

    #[test]
    fn precision_floor() {
        let fs = [7.0f32, 257.0f32, 1.0f32];
        assert_eq!([6.0, 256.0, 0.0], round_trip(fs));
    }

    #[test]
    #[should_panic]
    fn nans_01() {
        encode([std::f32::NAN, 0.0, 0.0]);
    }

    #[test]
    #[should_panic]
    fn nans_02() {
        encode([0.0, std::f32::NAN, 0.0]);
    }

    #[test]
    #[should_panic]
    fn nans_03() {
        encode([0.0, 0.0, std::f32::NAN]);
    }

    #[test]
    #[should_panic]
    fn negative_01() {
        encode([-1.0, 0.0, 0.0]);
    }

    #[test]
    #[should_panic]
    fn negative_02() {
        encode([0.0, -1.0, 0.0]);
    }

    #[test]
    #[should_panic]
    fn negative_03() {
        encode([0.0, 0.0, -1.0]);
    }

    #[test]
    fn negative_04() {
        encode([-0.0, -0.0, -0.0]);
    }
}
