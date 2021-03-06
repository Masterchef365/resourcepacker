use anyhow::{bail, ensure, Context, Result};
use serde::{Deserialize, Serialize};
use std::{fs::File, io::prelude::*};
use zip::ZipArchive;

fn usage(program_name: &str) -> String {
    format!(
        "Usage:
    {0} pack <textures.png> <atlas.json> <backup_resourcepack.zip> <resourcepack.zip>
        If <backup_resourcepack> and <resourcepack> are equal, the atlas will be created.

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
            args.next().with_context(usage)?.as_str(),
        ),
        "unpack" => todo!(),
        _ => bail!("{}", usage()),
    }
}

fn path_filter(path: &str) -> bool {
    path.starts_with("assets/minecraft/textures/block/") && path.ends_with(".png")
}

fn pack(
    texture_out_dir: &str,
    atlas_dir: &str,
    backup_pack_dir: &str,
    pack_dir: &str,
) -> Result<()> {
    let bkp_pack_file =
        File::open(backup_pack_dir).context("Failed to open backup resource pack")?;
    let mut bkp_pack_zip =
        zip::ZipArchive::new(bkp_pack_file).context("Failed to open resource pack zip file")?;

    let pack_file = File::open(pack_dir).context("Failed to open resource pack")?;
    let mut pack_zip =
        zip::ZipArchive::new(pack_file).context("Failed to open resource pack zip file")?;

    let make_atlas = backup_pack_dir == pack_dir;
    let atlas: Atlas = if make_atlas {
        let atlas = create_atlas(&mut pack_zip, pack_dir.to_string())?;
        let mut file = File::create(atlas_dir).context("Failed to create atlas file")?;
        serde_json::to_writer(&mut file, &atlas).context("Failed to serialize atlas")?;
        atlas
    } else {
        let mut file = File::open(atlas_dir).context("Failed to open atlas file")?;
        serde_json::from_reader(&mut file).context("Failed to parse atlas")?
    };

    let megatexture = compile_megatexture(&mut pack_zip, &atlas, &mut bkp_pack_zip)?;
    let mut out_file = File::create(texture_out_dir)?;
    write_texture_rgb(&mut out_file, &megatexture)?;

    Ok(())
}

/// Width of image patches
const TEX_WIDTH: u32 = 16;
/// Size of image patches in bytes
const TEX_CHANNELS: u32 = 3;
/// Allow up to MAX_FAIL_PERCENT * 100. % of images to be loaded from the backup texture
const MAX_FAIL_RATE: f32 = 0.05;

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
            squares.push(AtlasSquare { name, x, y });
        }
    }

    Ok(Atlas {
        pack_name,
        side_length,
        squares,
    })
}

struct RgbImage {
    data: Vec<u8>,
    /// Width in pixels
    width: u32,
}

impl RgbImage {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            data: vec![0; (width * height * TEX_CHANNELS) as usize],
            width,
        }
    }

    pub fn blit(&mut self, x: u32, y: u32, other: &RgbImage) {
        let (my_width, my_height) = self.dimensions();
        assert!(
            x < my_width && y < my_height,
            "Attempt to blit outside image boundaries"
        );
        // TODO: More asserts!

        for (row_idx, row) in other.data.chunks_exact(other.row_stride()).enumerate() {
            let off = self.row_stride() * (row_idx + y as usize) + (x * TEX_CHANNELS) as usize;
            self.data[off..off + other.row_stride()].copy_from_slice(row);
        }
    }

    /// Row width in bytes
    pub fn row_stride(&self) -> usize {
        (self.width * TEX_CHANNELS) as _
    }

    pub fn dimensions(&self) -> (u32, u32) {
        (
            self.width,
            self.data.len() as u32 / (self.width * TEX_CHANNELS),
        )
    }
}

fn rgba_to_rgb(data: Vec<u8>) -> Vec<u8> {
    let mut out = vec![];
    for chunk in data.chunks_exact(4) {
        out.push(chunk[0]);
        out.push(chunk[1]);
        out.push(chunk[2]);
    }
    out
}

fn read_texture_rgb<R: Read>(reader: &mut R) -> Result<RgbImage> {
    let decoder = png::Decoder::new(reader);
    let (info, mut reader) = decoder.read_info()?;

    let n_channels = match info.color_type {
        png::ColorType::RGB => 3,
        png::ColorType::RGBA => 4,
        other => bail!("Unsupported color type {:?}", other),
    };

    let mut data = vec![0; (info.width * info.height * n_channels) as usize];
    reader.next_frame(&mut data)?;

    let rgb_data = match info.color_type {
        png::ColorType::RGB => data,
        png::ColorType::RGBA => rgba_to_rgb(data),
        _ => unreachable!(),
    };

    Ok(RgbImage {
        data: rgb_data,
        width: info.width,
    })
}

fn write_texture_rgb<W: Write>(writer: &mut W, image: &RgbImage) -> Result<()> {
    let (width, height) = image.dimensions();
    let mut encoder = png::Encoder::new(writer, width, height);
    encoder.set_color(png::ColorType::RGB);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header()?;
    writer.write_image_data(&image.data)?;
    Ok(())
}

fn compile_megatexture<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    atlas: &Atlas,
    backup: &mut ZipArchive<R>,
) -> Result<RgbImage> {
    // Create megatexture
    let mut megatexture =
        RgbImage::new(TEX_WIDTH * atlas.side_length, TEX_WIDTH * atlas.side_length);

    // Blit squares onto the texture
    let mut failures = vec![];
    for square in &atlas.squares {
        // If a texture isn't found, it's loaded from the backup archive
        let texture = load_square(archive, square);
        let texture = match texture {
            Ok(tex) => tex,
            Err(_) => {
                //eprintln!("Error loading {}, loading from backup", &square.name);
                failures.push(square.name.clone());
                load_square(backup, square).context("Error loading backup square")?
            }
        };

        megatexture.blit(square.x * TEX_WIDTH, square.y * TEX_WIDTH, &texture);
    }

    let fail_rate = failures.len() as f32 / atlas.squares.len() as f32;
    if fail_rate > MAX_FAIL_RATE {
        //eprintln!("Missing: {:?}", failures);
        bail!("{}/{} textures defaulted to the backup, which is greater than the prescribed maximum {}%. Quitting!", failures.len(), atlas.squares.len(), MAX_FAIL_RATE * 100.);
    }

    Ok(megatexture)
}

fn load_square<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    square: &AtlasSquare,
) -> Result<RgbImage> {
    let mut file = archive
        .by_name(&square.name)
        .with_context(|| format!("Archive missing {}", &square.name))?;
    let texture = read_texture_rgb(&mut file)
        .with_context(|| format!("Error reading texture {}", &square.name))?;
    let (width, height) = texture.dimensions();
    ensure!(
        width == TEX_WIDTH && height == TEX_WIDTH,
        "Textures must be {0}x{0}; {1} is not.",
        TEX_WIDTH,
        square.name
    );
    Ok(texture)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Atlas {
    pub pack_name: String,
    pub side_length: u32,
    /// Atlas squares. Note: Always in left-right top-bottom order!
    pub squares: Vec<AtlasSquare>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AtlasSquare {
    pub name: String,
    pub x: u32,
    pub y: u32,
}