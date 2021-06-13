use anyhow::{Result, Context};

fn usage(program_name: &str) -> String {
    format!(
        "Usage:
    {0} pack <resourcepack.zip> <textures.png> (--create_manifest)
    {0} unpack <textures.png> <resourcepack.zip> <manifest.json> <template_resourcepack.zip>",
        program_name
    )
}

fn main() -> Result<()> {
    let mut args = std::env::args();
    let program_name = args.next().expect("No program name");
    let usage = usage(&program_name);
    let operation = args.next().context(usage)?;
    println!("Hello, world!");
    Ok(())
}