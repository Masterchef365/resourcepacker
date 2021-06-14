use anyhow::{bail, ensure, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    ffi::OsStr,
    fs::File,
    io::prelude::*,
    path::{Path, PathBuf},
};
use zip::ZipArchive;

fn usage(program_name: &str) -> String {
    format!(
        "Usage:
    {0} atlas <resourcepack dir> <atlas.json>
    {0} pack <resourcepack dir> <atlas.json> <texture dir>
    {0} unpack <image.png> <resourcepack.zip> <atlas.json>",
        program_name
    )
}

fn main() -> Result<()> {
    let mut args = std::env::args();
    let program_name = args.next().expect("No program name");
    let usage = || usage(&program_name);
    let operation = args.next().with_context(usage)?;

    match operation.as_str() {
        "atlas" => atlas(
            args.next().with_context(usage)?.as_str(),
            args.next().with_context(usage)?.as_str(),
            //args.next().with_context(usage)?.parse()?,
        ),
        "pack" => pack(
            args.next().with_context(usage)?.as_str(),
            args.next().with_context(usage)?.as_str(),
            args.next().with_context(usage)?.as_str(),
        ),
        "unpack" => todo!(),
        _ => bail!("{}", usage()),
    }
}

fn atlas(resourcepacks_dir: &str, atlas_path: &str) -> Result<()> {
    let atlas = build_atlas(resourcepacks_dir)?;
    atlas.save(atlas_path)?;
    println!(
        "Built atlas contains {0} squares for a total texture dimension of {1}x{1}",
        atlas.squares.len(),
        atlas.side_length * TEX_WIDTH
    );
    Ok(())
}

fn pack(resourcepacks_dir: &str, atlas_path: &str, texture_dir: &str) -> Result<()> {
    let texture_dir = Path::new(texture_dir);
    std::fs::create_dir(texture_dir).context("Failed to create texture dir")?;

    let atlas = Atlas::load(atlas_path)?;
    for dir in zipfiles_from(resourcepacks_dir)? {

        let mut archive = ZipArchive::new(File::open(&dir)?)?;

        let texture = compile_megatexture(&mut archive, &atlas)?;

        let name = dir.file_stem().unwrap().to_str().unwrap();
        let texture_path = texture_dir.join(format!("{}.png", name));

        write_texture_rgb(File::create(&texture_path)?, &texture)?;

        println!("Finished writing {}", texture_path.to_str().unwrap());
    }
    Ok(())
}

fn print_ok<T, E: std::fmt::Display>(r: Result<T, E>) -> Option<T> {
    match r {
        Err(e) => {
            eprintln!("{}", e);
            None
        }
        Ok(s) => Some(s),
    }
}

fn zipfiles_from(path: impl AsRef<Path>) -> Result<impl Iterator<Item=PathBuf>> {
    Ok(std::fs::read_dir(path)?
        .filter_map(|dir| print_ok(dir))
        .map(|d| d.path())
        .filter(|p| p.is_file() && p.extension() == Some(OsStr::new("zip"))))
}

fn build_atlas(resourcepacks_dir: &str) -> Result<Atlas> {
    let texture_counts = count_textures(resourcepacks_dir)?;
    let most_common = *texture_counts.values().max().context("Empty dataset")?;
    let texture_names = texture_counts
        .into_iter()
        .filter_map(|(name, count)| (count == most_common).then(|| name))
        .collect();
    Ok(Atlas::new(texture_names))
}

fn count_textures(resourcepacks_dir: &str) -> Result<HashMap<String, u32>> {
    let mut texture_counts: HashMap<String, u32> = HashMap::new();
    for pack_path in zipfiles_from(resourcepacks_dir)? {
        let names = match valid_texture_names(&pack_path) {
            Ok(n) => n,
            Err(e) => {
                eprintln!("Error opening {}, {}", pack_path.to_str().unwrap(), e);
                continue;
            }
        };

        for name in names {
            *texture_counts.entry(name).or_insert(0) += 1;
        }
    }

    Ok(texture_counts)
}

fn valid_texture_names(pack_path: impl AsRef<Path>) -> Result<Vec<String>> {
    let file = File::open(pack_path).context("Failed to open resource pack")?;
    let mut archive =
        zip::ZipArchive::new(file).context("Failed to open resource pack zip file")?;

    let mut good_names = vec![];

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let name = file.name().to_string();
        let is_file = file.is_file();

        if is_file && path_filter(&name) && check_texture(&mut file)? {
            good_names.push(name);
        }
    }
    Ok(good_names)
}

fn path_filter(path: &str) -> bool {
    path.starts_with("assets/minecraft/textures/block/") && path.ends_with(".png")
}

/*
fn pack(
    res_pack_dir: &str,
    texture_out_dir: &str,
    atlas_dir: &str,
    make_atlas: bool,
) -> Result<()> {
    let res_pack_file = File::open(res_pack_dir).context("Failed to open resource pack")?;
    let mut res_pack_archive =
        zip::ZipArchive::new(res_pack_file).context("Failed to open resource pack zip file")?;

    let atlas: Atlas = if make_atlas {
        let atlas = create_atlas(&mut res_pack_archive, res_pack_dir.to_string())?;
        let mut file = File::create(atlas_dir).context("Failed to create atlas file")?;
        serde_json::to_writer(&mut file, &atlas).context("Failed to serialize atlas")?;
        atlas
    } else {
        let mut file = File::open(atlas_dir).context("Failed to open atlas file")?;
        serde_json::from_reader(&mut file).context("Failed to parse atlas")?
    };

    let megatexture = compile_megatexture(&mut res_pack_archive, &atlas)?;
    let mut out_file = File::create(texture_out_dir)?;
    write_texture_rgb(&mut out_file, &megatexture)?;

    Ok(())
}
*/

/// Width of image patches
const TEX_WIDTH: u32 = 16;
/// Size of image patches in bytes
const TEX_CHANNELS: u32 = 3;

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

/*
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
}
*/

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

fn write_texture_rgb<W: Write>(writer: W, image: &RgbImage) -> Result<()> {
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
) -> Result<RgbImage> {
    // Create megatexture
    let mut megatexture =
        RgbImage::new(TEX_WIDTH * atlas.side_length, TEX_WIDTH * atlas.side_length);

    // Blit squares onto the texture
    for square in &atlas.squares {
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
        megatexture.blit(square.x * TEX_WIDTH, square.y * TEX_WIDTH, &texture);
    }

    Ok(megatexture)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Atlas {
    pub side_length: u32,
    /// Atlas squares. Note: Always in left-right top-bottom order!
    pub squares: Vec<AtlasSquare>,
}

impl Atlas {
    pub fn new(mut names: Vec<String>) -> Self {
        let side_length = (names.len() as f32).sqrt().ceil() as u32;

        let mut squares = vec![];
        'outer: for y in 0..side_length {
            for x in 0..side_length {
                let name = match names.pop() {
                    Some(n) => n,
                    None => break 'outer,
                };
                squares.push(AtlasSquare { name, x, y });
            }
        }

        Self {
            side_length,
            squares,
        }
    }

    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let mut file = File::open(path).context("Failed to open atlas file")?;
        Ok(serde_json::from_reader(&mut file).context("Failed to parse atlas")?)
    }

    pub fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        let mut file = File::create(path).context("Failed to create atlas file")?;
        Ok(serde_json::to_writer(&mut file, self).context("Failed to serialize atlas")?)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AtlasSquare {
    pub name: String,
    pub x: u32,
    pub y: u32,
}