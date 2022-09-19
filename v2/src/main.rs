#[macro_use]
extern crate log;
#[macro_use]
extern crate anyhow;

use anyhow::Result;

use imagerep::BlockedImage;

// this will be configurable by a CLI later
const IMAGE_PATH: &str = "../img/ted-hiking-002.jpg";
const INPUT_CFG: InputConfig = InputConfig {
    pixels_per_block: 150,
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
    let blocked = BlockedImage::new(
        &image::open(IMAGE_PATH)?.to_rgb8(),
        INPUT_CFG.pixels_per_block,
    );

    blocked.render_original_res().save("out.png")?;

    // let atomic = tris::AtomicU64::new(0);
    // let a = &atomic;
    // let b = a;

    // thread::scope(|scope| {
    //     scope.spawn(move || drop(a));
    //     scope.spawn(move || drop(b));
    // });

    // thread::spawn(move || drop(a));
    // thread::spawn(move || drop(b));

    Ok(())
}

pub mod tris {
    pub use std::sync::atomic::AtomicU64;
    use std::{
        cell::UnsafeCell,
        mem::{self, ManuallyDrop},
        sync::atomic,
    };
    #[cfg(not(target_has_atomic = "64"))]
    compile_error!("no atomic u64s?");

    use static_assertions::const_assert_eq;

    /// since this type shares the same size and align as `PointRawStoreT`
    /// and contains no uninit bytes (because of explicit padding taking up the structs full size)
    /// it may be safely transmuted between `Self` <-> `PointRawStoreT`
    ///
    /// it is safe to create a `mem::zeroed` of this type
    #[repr(C)]
    #[derive(Default, Debug, Clone, Copy, PartialEq)]
    pub struct PointRaw {
        pos: (f32, f32),
        /// 0 bc (4+4) bytes of space is taken, this pads the struct to exactly 8 bytes in size (no unninit bytes)
        __pad: [u8; 0],
        /// this field can go anywhere, and has size zero. causes `PointRaw` to be aligned to `PointRawStoreT`
        /// NOTE: this is NOT necessary for `transmute::<PointRaw, PointRawStoreT>` but it does make `*mut PointRaw as *mut PointRawStoreT` valid
        __alignas: [PointRawStoreT; 0],
    }
    pub type PointRawStoreT = u64;
    const_assert_eq!(mem::size_of::<PointRawStoreT>(), 8);
    const_assert_eq!(mem::size_of::<PointRaw>(), 8);
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

    union MaybeAtomicU64 {
        // dont actually have to do anything special to drop, as in bolth cases here the type is not
        // `Copy` due to UnsafeCell (which does not add any special drop requirements)
        atomic: ManuallyDrop<AtomicU64>,
        nonatomic: ManuallyDrop<UnsafeCell<u64>>,
    }

    pub struct MaybeAtomicPoint {
        inner: MaybeAtomicU64,
    }

    impl MaybeAtomicPoint {
        pub const fn new(atomic: bool) -> Self {
            let inner = if atomic {
                MaybeAtomicU64 {
                    atomic: ManuallyDrop::new(AtomicU64::new(0)),
                }
            } else {
                MaybeAtomicU64 {
                    nonatomic: ManuallyDrop::new(UnsafeCell::new(0)),
                }
            };
            Self { inner }
        }

        /// # Saftey
        /// - is_atomic must match the atomicity of this value when it was created.
        /// - if it is not atomic, the current thread must be the only one accessing it at this time
        pub unsafe fn load(&self, is_atomic: bool) -> (f32, f32) {
            PointRaw::from_storet(if is_atomic {
                self.inner.atomic.load(atomic::Ordering::Relaxed)
            } else {
                self.inner.nonatomic.get().read()
            })
            .pos
        }

        /// # Saftey
        /// - is_atomic must match the atomicity of this value when it was created.
        /// - if it is not atomic, the current thread must be the only one accessing it at this time
        pub unsafe fn store(&self, is_atomic: bool, pos: (f32, f32)) {
            let as_storet = PointRaw::from(pos).into_storet();
            if is_atomic {
                self.inner
                    .atomic
                    .store(as_storet, atomic::Ordering::Relaxed);
            } else {
                self.inner.nonatomic.get().write(as_storet)
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

        pub fn into_blocks(self) -> Vec<Block> {
            self.buffer
        }

        pub fn ppb(&self) -> u32 {
            self.pixels_per_block
        }

        pub fn render_original_res(&self) -> RgbImage {
            // TODO remove this bit once there is a proper get() function
            let img = RgbImage::from_vec(
                self.width(),
                self.height(),
                self
                    .clone()
                    .into_blocks()
                    .into_iter()
                    .flat_map(|block| block.avg_color.0)
                    .collect::<Vec<_>>(),
            )
            .unwrap();
            RgbImage::from_fn(
                self.width() * self.ppb(),
                self.height() * self.ppb(),
                |x, y| {
                    *img.get_pixel(x / self.ppb(), y / self.ppb())
                },
            )
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
