#[macro_use]
extern crate log;
#[macro_use]
extern crate anyhow;

use anyhow::Result;

use imagerep::BlockedImage;

// this will be configurable by a CLI later
const IMAGE_PATH: &str = "../img/ted-hiking-002.jpg";
const INPUT_CFG: InputConfig = InputConfig {
    pixels_per_block: 5,
};
pub struct InputConfig {
    /// size (in pixels) of each 'block' that the image is broken up into (blocks are like pixels, but contains
    /// information about the individual subpixels they contain)
    pub pixels_per_block: u32,
}
const OUTPUT_PATH: &str = "out.png";
const OUTPUT_CFG: OutputConfig = OutputConfig {};
pub struct OutputConfig {}

fn main() -> Result<()> {
    pretty_env_logger::init();
    let image = BlockedImage::new(
        &image::open(IMAGE_PATH)?.to_rgb8(),
        INPUT_CFG.pixels_per_block,
    );

    Ok(())
}

pub mod tris {
    use std::mem;

    use static_assertions::const_assert_eq;

    /// since this type shares the same size and align as `u128`
    /// and contains no uninit bytes (because of explicit padding taking up the structs full size)
    /// it may be safely transmuted between `Self` <-> `u128`
    ///
    /// it is safe to create a `mem::zeroed` of this type
    #[repr(C)]
    #[derive(Default, Debug, Clone, Copy, PartialEq)]
    struct PointRaw {
        //TODO possibly shrink this to a u64 vs u128, but for now the possiblity is kept open to be able to include more data in the future
        pos: (f32, f32),
        /// 8 bc (4+4) bytes of space is taken, this pads the struct to exactly 16 bytes in size (no unninit bytes)
        __pad: [u8; 8],
        /// this field can go anywhere, and has size zero. causes `PointRaw` to be aligned to `u128`
        /// NOTE: this is NOT necessary for `transmute::<PointRaw, u128>` but it does make `*mut PointRaw as *mut u128` valid
        __alignas: [PointRawStoreT; 0],
    }
    type PointRawStoreT = u128;
    const_assert_eq!(mem::size_of::<PointRawStoreT>(), 16);
    const_assert_eq!(mem::size_of::<PointRaw>(), 16);
    const_assert_eq!(mem::align_of::<PointRawStoreT>(), 8);
    const_assert_eq!(mem::align_of::<PointRaw>(), 8);

    impl PointRaw {
        pub const unsafe fn from_storet(x: PointRawStoreT) -> Self {
            mem::transmute(x)
        }

        pub const fn into_storet(self) -> PointRawStoreT {
            unsafe { mem::transmute(self) }
        }
    }

    impl From<PointRawStoreT> for PointRaw {
        fn from(x: PointRawStoreT) -> Self {
            unsafe { PointRaw::from_storet(x) } // not perfect, but no ub
        }
    }

    impl Into<PointRawStoreT> for PointRaw {
        fn into(self) -> PointRawStoreT {
            self.into_storet()
        }
    }

    impl AsRef<PointRaw> for PointRawStoreT {
        fn as_ref<'a>(&'a self) -> &'a PointRaw {
            unsafe { &*(self as *const Self as *const PointRaw) }
        }
    }

    impl From<(f32, f32)> for PointRaw {
        fn from(pos: (f32, f32)) -> Self {
            Self {
                pos,
                ..Default::default()
            }
        }
    }

    impl From<[f32; 2]> for PointRaw {
        fn from(pos: [f32; 2]) -> Self {
            Self {
                pos: (pos[0], pos[1]),
                ..Default::default()
            }
        }
    }
}

pub mod imagerep {
    use anyhow::Result;
    use image::{Rgb, RgbImage};
    use itertools::Itertools;

    use crate::imageops::avg_color;

    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    pub struct BlockedImage {
        width: u32,
        height: u32,
        pixels_per_block: u32,
        buffer: Vec<Block>,
    }

    impl BlockedImage {
        pub fn new(image: &RgbImage, pixels_per_block: u32) -> BlockedImage {
            let image = image::imageops::resize(
                image,
                image.width() + (pixels_per_block - image.width() % pixels_per_block), // scale to next multiple of `pixels_per_block`. required for next step
                image.height() + (pixels_per_block - image.height() % pixels_per_block),
                image::imageops::Nearest,
            );
            let (width, height) = (image.width(), image.height());
            // le abomination
            let buffer = image
                .rows()
                .chunks(pixels_per_block as usize)
                .into_iter()
                .map(|rows|
                    rows.map(|row|
                        row.chunks(pixels_per_block as usize)
                ))
                .map(|rows| {
                    let mut buffer: Vec<Option<Vec<Rgb<u8>>>> = vec![None; (width / pixels_per_block) as usize /* exact multiple bc of scaling above */];
                    for row in rows {
                        for (i, block_chunk) in row.into_iter().enumerate() {
                            buffer.get_mut(i)
                                .unwrap()
                                .get_or_insert(vec![])
                                .extend(block_chunk);
                        }
                    }
                    buffer
                        .into_iter()
                        .map(Option::unwrap)
                })
                .flatten()
                .map(|block_pixels| {
                    Block {
                        avg_color: avg_color(block_pixels.as_slice()).unwrap() // block must have some pixels in it
                    }
                })
                .collect::<Vec<_>>();
            BlockedImage {
                width: width / pixels_per_block,
                height: height / pixels_per_block,
                pixels_per_block,
                buffer,
            }
        }

        pub fn width(&self) -> u32 {
            self.width
        }

        pub fn height(&self) -> u32 {
            self.height
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    pub struct Block {
        pub avg_color: Rgb<u8>,
    }

    // not used currently, may be usefull later
    pub fn load_image(data: &[u8], file_ext: &str) -> Result<RgbImage> {
        let format = match (
            image::guess_format(data),
            image::ImageFormat::from_extension(file_ext),
        ) {
            (Err(_), Some(format)) => {
                warn!("guessing image format failed, continuing with file extension");
                format
            }
            (Ok(guessed_format), Some(ext_format)) => {
                if guessed_format != ext_format {
                    warn!("guessed image format ({:?}) is not the same as the file extension ({:?}), continuing with the guessed format", guessed_format.extensions_str(), ext_format.extensions_str());
                }
                guessed_format
            }
            (Ok(format), None) => format,
            (Err(_), None) => bail!("Unknown image format!"),
        };
        Ok(image::load_from_memory_with_format(data, format)?.to_rgb8())
    }
}

pub mod imageops {
    use image::{GenericImage, Rgb, RgbImage};
    use std::cmp;

    pub fn scale(
        unscaled: RgbImage,
        scale_to: u32, /* size of the largest output dimension */
    ) -> (u32, u32, RgbImage, RgbImage) {
        enum Axis {
            X,
            Y,
        }
        let current_axis: (u32, u32) = (unscaled.width(), unscaled.height());
        let larger = match current_axis.0.cmp(&current_axis.1) {
            cmp::Ordering::Greater => Axis::X,
            cmp::Ordering::Equal => Axis::X,
            cmp::Ordering::Less => Axis::Y,
        };

        let image_size = scale_to;

        match larger {
            Axis::X => {
                let factor = image_size as f64 / current_axis.0 as f64;
                let new_height = (factor * current_axis.1 as f64) as u32;
                // println!("Original: {current_axis:?}, new: ({image_size}, {new_height})");
                let scaled = image::imageops::resize(
                    &unscaled,
                    image_size,
                    new_height,
                    image::imageops::Lanczos3,
                );
                let mut final_image = RgbImage::from_pixel(image_size, image_size, Rgb([0; 3]));
                final_image
                    .copy_from(&scaled, 0, (image_size - new_height) / 2)
                    .unwrap();
                (image_size, new_height, scaled, final_image)
            }
            Axis::Y => {
                let factor = image_size as f64 / current_axis.1 as f64;
                let new_width = (factor * current_axis.0 as f64) as u32;
                // println!("Original: {current_axis:?}, new: ({new_width}, {image_size})");
                let scaled = image::imageops::resize(
                    &unscaled,
                    new_width,
                    image_size,
                    image::imageops::Lanczos3,
                );
                let mut final_image = RgbImage::from_pixel(image_size, image_size, Rgb([0; 3]));
                final_image
                    .copy_from(&scaled, (image_size - new_width) / 2, 0)
                    .unwrap();
                (new_width, image_size, scaled, final_image)
            }
        }
    }

    pub fn avg_color(pixels: &[Rgb<u8>]) -> Option<Rgb<u8>> {
        if pixels.is_empty() {
            None?
        }
        let [r, g, b] = pixels
            .iter()
            .map(|&Rgb(rgb)| rgb.map(|c| c as f64))
            .fold([0f64; 3], |[ar, ag, ab], [r, g, b]| {
                [ar + r, ag + g, ab + b]
            });
        let l = pixels.len() as f64;
        Some(Rgb([r / l, g / l, b / l].map(|c| c.clamp(0.0, 255.0) as u8)))
    }
}
