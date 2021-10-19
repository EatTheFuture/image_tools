//! Reads and writes color transform lookup tables in various formats.

use std::io::Write;

pub fn write_spi1d<W: Write>(out: &mut W, range: (f32, f32), table: &[f32]) -> std::io::Result<()> {
    out.write_all(b"Version 1\n")?;
    out.write_all(format!("From {:0.7} {:0.7}\n", range.0, range.1).as_bytes())?;
    out.write_all(format!("Length {}\n", table.len()).as_bytes())?;
    out.write_all(b"Components 1\n")?;
    out.write_all(b"{\n")?;
    for n in table {
        out.write_all(format!("  {:0.7}\n", n).as_bytes())?;
    }
    out.write_all(b"}\n")?;

    Ok(())
}

pub fn write_cube_1d<W: Write>(
    out: &mut W,
    range: (f32, f32),
    table_r: &[f32],
    table_g: &[f32],
    table_b: &[f32],
) -> std::io::Result<()> {
    assert!(table_r.len() == table_g.len() && table_r.len() == table_b.len());

    out.write_all(b"TITLE \"untitled\"\n")?;
    out.write_all(
        format!(
            "DOMAIN_MIN {:0.7} {:0.7} {:0.7}\n",
            range.0, range.0, range.0
        )
        .as_bytes(),
    )?;
    out.write_all(
        format!(
            "DOMAIN_MAX {:0.7} {:0.7} {:0.7}\n",
            range.1, range.1, range.1
        )
        .as_bytes(),
    )?;
    out.write_all(format!("LUT_1D_SIZE {}\n", table_r.len()).as_bytes())?;

    for ((r, g), b) in table_r
        .iter()
        .copied()
        .zip(table_g.iter().copied())
        .zip(table_b.iter().copied())
    {
        out.write_all(format!("{:0.7} {:0.7} {:0.7}\n", r, g, b).as_bytes())?;
    }

    Ok(())
}
