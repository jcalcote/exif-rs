//! Explicit admission/storage: cargo run --example arenas -- image.jpg
use exif_inspect::{Extractor, Prefix};
use std::{env, fs, io, sync::atomic::AtomicBool};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = env::args()
        .nth(1)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "usage: arenas <media-file>"))?;
    let input = fs::read(path)?;
    let extractor = Extractor::default();
    let plan = extractor.plan(input.len())?;
    let split = input.len() / 2;
    let parts = [&input[..split], &input[split..]];
    let prefix = Prefix::new(&parts, plan.input_bytes)?;
    let mut scratch = vec![0; plan.workspace_bytes];
    let mut output = vec![0; plan.output_bytes];
    let cancel = AtomicBool::new(false);
    let result = extractor.extract(prefix, &mut scratch, &mut output, &cancel)?;
    let descriptor = result.descriptor();
    eprintln!("{:?}: {:?}", result.status(), result.diagnostic());
    // The descriptor can be retained beside output. Reborrow validates storage
    // before yielding structured strings; it retains no source/scratch view.
    for field in descriptor.fields(&output)? {
        println!("{} / {}: {}", field.directory, field.name, field.value);
    }
    Ok(())
}
