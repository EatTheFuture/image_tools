mod trifloat;

use std::io::Write;

pub fn write_hdr<W: Write>(
    out: &mut W,
    image: &[[f32; 3]],
    width: usize,
    height: usize,
) -> std::io::Result<()> {
    assert_eq!(image.len(), width * height);

    out.write_all(b"#?RADIANCE\n");
    out.write_all(b"FORMAT=32-bit_rle_rgbe\n\n");
    out.write_all(format!("-Y {} +X {}\n", height, width).as_bytes());
    for pixel in image.iter() {
        out.write_all(&trifloat::encode(*pixel));
    }

    Ok(())
}
