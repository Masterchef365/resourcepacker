use anyhow::{bail, Context, Result};
use std::{fs::File, io::prelude::*};

fn usage(program_name: &str) -> String {
    format!(
        "Usage:
    {0} pack <resourcepack.zip> <textures.png> <manifest.json> (--create_manifest)
    {0} unpack <textures.png> <resourcepack.zip> <manifest.json> <template_resourcepack.zip>",
        program_name
    )
}

fn main() -> Result<()> {
    let mut args = std::env::args();
    let program_name = args.next().expect("No program name");
    let usage = || usage(&program_name);
    let operation = args.next().with_context(usage)?;

    match operation.as_str() {
        "pack" => pack(
            args.next().with_context(usage)?.as_str(),
            args.next().with_context(usage)?.as_str(),
            args.next().with_context(usage)?.as_str(),
            args.next().is_some(),
        ),
        "unpack" => todo!(),
        _ => bail!("{}", usage()),
    }
}

fn path_filter(path: &str) -> bool {
    path.starts_with("assets/minecraft/textures/block/") && path.ends_with(".png")
}

fn pack(
    res_pack_dir: &str,
    texture_out_dir: &str,
    manifest_dir: &str,
    create_manifest: bool,
) -> Result<()> {
    let res_pack_file = File::open(res_pack_dir).context("Failed to open resource pack")?;
    let mut res_pack_archive =
        zip::ZipArchive::new(res_pack_file).context("Failed to open resource pack zip file")?;

    for i in 0..res_pack_archive.len() {
        let mut file = res_pack_archive.by_index(i)?;
        let name = file.name();
        if file.is_file() && path_filter(&name) {
            println!("{}", file.name());
            let tex = read_block_texture(&mut file)?;
        }
    }

    Ok(())
}

/// Read a 16x16 texture in RGB format
fn read_block_texture<R: Read>(reader: &mut R) -> Result<Vec<u8>> {
    let decoder = png::Decoder::new(reader);
    let (info, mut reader) = decoder.read_info()?;
    println!("{:?}", info);
    Ok(vec![])
}