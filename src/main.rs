use anyhow::{bail, Context, Result};
use zip::ZipArchive;
use std::{fs::File, io::prelude::*};

fn usage(program_name: &str) -> String {
    format!(
        "Usage:
    {0} pack <resourcepack.zip> <textures.png> <atlas.json> (--create_atlas)
    {0} unpack <textures.png> <resourcepack.zip> <atlas.json> <template_resourcepack.zip>",
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
    atlas_dir: &str,
    make_atlas: bool,
) -> Result<()> {
    let res_pack_file = File::open(res_pack_dir).context("Failed to open resource pack")?;
    let mut res_pack_archive =
        zip::ZipArchive::new(res_pack_file).context("Failed to open resource pack zip file")?;

    let atlas = if make_atlas {
        create_atlas(&mut res_pack_archive, res_pack_dir.to_string())?
    } else {
        todo!("Load atlas")
    };

    dbg!(atlas);

    Ok(())
}

/// Width of image patches
const TEX_WIDTH: u32 = 16;
/// Size of image patches in bytes
const TEX_SIZE: u32 = TEX_WIDTH * TEX_WIDTH * 3 * 4;

/// Check png info to see if it is compatible
fn check_info(info: &png::OutputInfo) -> bool {
    info.width == TEX_WIDTH
        && info.height == TEX_WIDTH
        && info.bit_depth == png::BitDepth::Eight
        && matches!(info.color_type, png::ColorType::RGB | png::ColorType::RGBA)
}

/// Check a png file to see if it is compatible
fn check_texture<R: Read>(reader: &mut R) -> Result<bool> {
    let decoder = png::Decoder::new(reader);
    let (info, _) = decoder.read_info()?;
    Ok(check_info(&info))
}

fn create_atlas<R: Read + Seek>(archive: &mut ZipArchive<R>, pack_name: String) -> Result<Atlas> {
    let mut good_names = vec![];
    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let name = file.name().to_string();
        let is_file = file.is_file();
        if is_file && path_filter(&name) && check_texture(&mut file)? {
            good_names.push(name);
        }
    }

    let side_length = (good_names.len() as f32).sqrt().ceil() as u32;

    let mut squares = vec![];
    'outer: for y in 0..side_length {
        for x in 0..side_length {
            let name = match good_names.pop() {
                Some(n) => n,
                None => break 'outer,
            };
            squares.push(AtlasSquare {
                name,
                x,
                y
            });
        }
    }

    Ok(Atlas {
        pack_name,
        side_length,
        squares,
    })
}

#[derive(Debug, Clone)]
struct Atlas {
    pack_name: String,
    side_length: u32,
    squares: Vec<AtlasSquare>,
}

#[derive(Debug, Clone)]
struct AtlasSquare {
    name: String,
    x: u32,
    y: u32,
}

