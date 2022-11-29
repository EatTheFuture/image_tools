fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).unwrap();

    let _ = image_fmt::load(std::fs::File::open(path)?)?;

    return Ok(());
}
