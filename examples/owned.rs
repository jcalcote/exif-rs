//! Ordinary allocating extraction: cargo run --example owned -- image.jpg
use std::{env, fs, io};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = env::args()
        .nth(1)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "usage: owned <media-file>"))?;
    let input = fs::read(path)?;
    let metadata = exif_inspect::extract(&input)?;
    eprintln!("{:?}: {:?}", metadata.status(), metadata.diagnostic());
    for field in metadata.fields() {
        println!("{} / {}: {}", field.directory, field.name, field.value);
    }
    Ok(())
}
