#[macro_use]
extern crate log;
#[macro_use]
extern crate anyhow;

pub mod swsrasv;

use anyhow::Result;
use glutin_window::GlutinWindow;
use image::DynamicImage;
use opengl_graphics::{GlGraphics, OpenGL, Texture, TextureSettings};
use piston::{
    event_loop::{EventSettings, Events},
    window::WindowSettings,
    RenderEvent,
};

use crate::{imagerep::BlockedImage, tris::point::MaybeAtomicPoint};

// this will be configurable by a CLI later
const IMAGE_PATH: &str = "../img/ted-hiking-002.jpg";
const INPUT_CFG: InputConfig = InputConfig {
    pixels_per_block: 50,
    verts_per_block: 1,
};
pub struct InputConfig {
    /// size (in pixels) of each 'block' that the image is broken up into (blocks are like pixels, but contains
    /// information about the individual subpixels they contain)
    pub pixels_per_block: u32,
    /// number of vericies per each block
    pub verts_per_block: u32,
}
const OUTPUT_PATH: &str = "out.png";
const OUTPUT_CFG: OutputConfig = OutputConfig {};
pub struct OutputConfig {}

#[tokio::main]
async fn main() -> Result<()> {
    pretty_env_logger::init();

    let raw_image = &image::open(IMAGE_PATH)?;
    let blocked = BlockedImage::new(&raw_image.to_rgb8(), INPUT_CFG.pixels_per_block);

    let rendered_blocked = blocked.render_original_res();
    rendered_blocked.save("out.png")?;
    let rendered_blocked = DynamicImage::ImageRgb8(rendered_blocked).into_rgba8();

    //? code for creating tri grid

    // in block to enforece/make clear what is being used where
    let (multfn, verts) = {
        // (0, 0) is top left corner
        // offset per "block" is 5 (regulates percision and stuff)
        const OFFSET_PER_BLOCK: f64 = 5.0;
        // let max_coords = (
        //     OFFSET_PER_BLOCK * blocked.width() as f64,
        //     OFFSET_PER_BLOCK * blocked.height() as f64,
        // );
        let verts_per_block = INPUT_CFG.verts_per_block; // true only for width
        // offset for x (=0.5) for each row.
        let offset_width_unit = 60.0f64.to_radians().cos();
        // dy for changing rows
        let inc_height_unit = 60.0f64.to_degrees().sin();
        let inc_width_unit = 2.0 * offset_width_unit;

        // val to add each y inc
        let inc_height_scaled = (inc_height_unit / verts_per_block as f64) * OFFSET_PER_BLOCK; // scale same ammount as width (verts per block does not hold here)
        // val to add each x inc
        let inc_width_scaled = (inc_width_unit / verts_per_block as f64) * OFFSET_PER_BLOCK;
        // val to alternatingly subtract from x/leave for each row
        let offset_width_scaled = (offset_width_unit / verts_per_block as f64) * OFFSET_PER_BLOCK;

        let mut verts: Vec<MaybeAtomicPoint> = vec![];

        let max_y_idx = ((verts_per_block * blocked.height()) as f64 / inc_height_unit).ceil() as u32;
        // add y % 2 to this to get real x offset
        let max_x_idx = verts_per_block * blocked.width();
        for y in 0..=max_y_idx {
            for x in 0..=(max_x_idx + y % 2) {
                let is_atomic = false;
                let atomic = MaybeAtomicPoint::new(is_atomic);
                // TODO: calculate pos with multiplication to avoid accumulating error
                unsafe { atomic.store(is_atomic, (x as f64 * inc_width_scaled - offset_width_scaled * (y % 2) as f64, y as f64 * inc_height_scaled)) }
                verts.push(atomic);
            }
        }
        (
            move |rect_size: (f64, f64)| {
                let xmult = ((rect_size.0 / verts_per_block as f64) / blocked.width() as f64) / OFFSET_PER_BLOCK;
                let ymult = ((rect_size.1 / verts_per_block as f64) / blocked.height() as f64) / OFFSET_PER_BLOCK;
                (xmult, ymult)
            },
            verts
        )
    };

    //? rendering code

    pub fn rgba(r: u8, g: u8, b: u8, a: f32) -> [f32; 4] {
        [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, a]
    }

    // Change this to OpenGL::V2_1 if not working.
    let opengl = OpenGL::V4_5;

    // Create an Glutin window.
    let mut window: GlutinWindow = WindowSettings::new("trifit v2", [10, 10])
        .graphics_api(opengl)
        .vsync(true)
        .build()
        .unwrap();

    let mut gl = GlGraphics::new(opengl);

    let mut events = Events::new({
        let mut es = EventSettings::new();
        es.lazy = false;
        es.ups_reset = 20;
        es.ups = 10;
        es
    });

    const PADDING: f64 = 30.0;
    let bg_texture = Texture::from_image(&rendered_blocked, &TextureSettings::new());
    let bg_ratio_wth = rendered_blocked.width() as f64 / rendered_blocked.height() as f64; // w/h ratio
    let bg_ratio_htw = rendered_blocked.height() as f64 / rendered_blocked.width() as f64; // h/w ratio

    while let Some(e) = events.next(&mut window) {
        if let Some(render_args) = e.render_args() {
            use graphics::clear;

            gl.draw(render_args.viewport(), |c, gl| {
                clear(rgba(0, 0, 0, 1.0), gl);

                let [target_w, target_h] = render_args.window_size;
                let (target_w, target_h, offset_w, offset_h) = (target_w - 2.0 * PADDING, target_h - 2.0 * PADDING, PADDING, PADDING);

                let h_by_w = bg_ratio_htw * target_w;
                let w_by_h = bg_ratio_wth * target_h;
                let rect_size = if h_by_w <= target_h {
                    (target_w, h_by_w)
                } else {
                    // assert!(w_by_h <= target_w);
                    (w_by_h, target_h)
                };
                let padding = (
                    (target_w - rect_size.0) / 2.0,
                    (target_h - rect_size.1) / 2.0,
                );
                graphics::Image::new()
                    .rect(graphics::rectangle::rectangle_by_corners(
                        padding.0 + offset_w,
                        padding.1 + offset_h,
                        rect_size.0 + padding.0 + offset_w,
                        rect_size.1 + padding.1 + offset_h,
                    ))
                    .draw(
                        &bg_texture,
                        &graphics::DrawState::default(),
                        c.transform,
                        gl,
                    );

                let (xmult, ymult) = multfn(rect_size);
                for point in &verts {
                    // saftey: yes
                    let (x, y) = unsafe { point.load(false) };
                    let (dx, dy) = padding;
                    graphics::Ellipse::new(rgba(255, 36, 255, 0.7)).draw(
                        graphics::rectangle::centered_square(x*xmult + dx + offset_w, y*ymult + dy + offset_h, 3.0),
                        &graphics::DrawState::default(),
                        c.transform,
                        gl,
                    );
                }
            });
        }
    }

    // println!("{:?}", unsafe { verts[10].load(false) });

    // let atomic = tris::AtomicU64::new(0);
    // let a = &atomic;
    // let b = a;

    // thread::scope(|scope| {
    //     scope.spawn(move || drop(a));
    //     scope.spawn(move || drop(b));
    // });

    Ok(())
}

pub mod tris {
    pub mod point {
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
            // store if it is atomic or not, to catch bugs
            debug_is_atomic: bool, // TODO: remove this once validated.
        }

        // use f64 for computation, and f32 for storage
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
                Self {
                    inner,
                    debug_is_atomic: atomic,
                }
            }

            /// # Saftey
            /// - is_atomic must match the atomicity of this value when it was created.
            /// - if it is not atomic, the current thread must be the only one accessing it at this time
            pub unsafe fn load(&self, is_atomic: bool) -> (f64, f64) {
                assert_eq!(self.debug_is_atomic, is_atomic);
                let point = PointRaw::from_storet(if is_atomic {
                    self.inner.atomic.load(atomic::Ordering::Relaxed)
                } else {
                    self.inner.nonatomic.get().read()
                })
                .pos;
                (point.0 as f64, point.1 as f64)
            }

            /// # Saftey
            /// - is_atomic must match the atomicity of this value when it was created.
            /// - if it is not atomic, the current thread must be the only one accessing it at this time
            pub unsafe fn store(&self, is_atomic: bool, pos: (f64, f64)) {
                assert_eq!(self.debug_is_atomic, is_atomic);
                let as_storet = PointRaw::from((pos.0 as f32, pos.1 as f32)).into_storet();
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
            //TODO see if it can be removed
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
                self.clone()
                    .into_blocks()
                    .into_iter()
                    .flat_map(|block| block.avg_color.0)
                    .collect::<Vec<_>>(),
            )
            .unwrap();
            RgbImage::from_fn(
                self.width() * self.ppb(),
                self.height() * self.ppb(),
                |x, y| *img.get_pixel(x / self.ppb(), y / self.ppb()),
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
    use image::Rgb;
    // use image::{GenericImage, Rgb, Pixel, ImageBuffer};
    // use std::cmp;

    // pub fn scale<P: Pixel + 'static>(
    //     unscaled: &ImageBuffer::<P, Vec<<P as Pixel>::Subpixel>>,
    //     scaled_x: u32,
    //     scaled_y: u32,
    // // ) -> (u32, u32, ImageBuffer::<P, Vec<<P as Pixel>::Subpixel>>, ImageBuffer::<P, Vec<<P as Pixel>::Subpixel>>) {
    // ) -> ImageBuffer::<P, Vec<<P as Pixel>::Subpixel>> {
    //     //? new
    //     // let mut output_image = ImageBuffer::default();

    //     // let scaled_dims = if scaled_x > scaled_y {
    //     //     // out x larger, resize to y dim
    //     //     ((scaled_x as f64 * (unscaled.height() as f64 / scaled_y as f64)).floor() as u32, scaled_y)
    //     // } else {
    //     //     // out y larger, resize to x dim
    //     //     dbg!(scaled_x, dbg!(scaled_y as f64 * (unscaled.width() as f64 / scaled_x as f64)).floor() as u32)
    //     // };
    //     // let offset = (
    //     //     (scaled_x - scaled_dims.0) / 2,
    //     //     (scaled_y - scaled_dims.1) / 2,
    //     // );
    //     // println!("offset: {offset:?}, scaled_xy: ({scaled_x},{scaled_y}), scaled_dims: {scaled_dims:?}");

    //     // let scaled_image = image::imageops::resize(
    //     //     unscaled,
    //     //     output_image.width() - offset.0,
    //     //     output_image.height() - offset.1,
    //     //     image::imageops::FilterType::CatmullRom,
    //     // );

    //     // output_image.copy_from(&scaled_image, offset.0, offset.1).unwrap();

    //     // output_image
    //     //? old
    //     // enum Axis {
    //     //     X,
    //     //     Y,
    //     // }
    //     // let current_axis: (u32, u32) = (unscaled.width(), unscaled.height());
    //     // let larger = match current_axis.0.cmp(&current_axis.1) {
    //     //     cmp::Ordering::Greater => Axis::X,
    //     //     cmp::Ordering::Equal => Axis::X,
    //     //     cmp::Ordering::Less => Axis::Y,
    //     // };

    //     // let default_pixel = *unscaled.get_pixel(0, 0);

    //     // match larger {
    //     //     Axis::X => {
    //     //         let factor = image_size as f64 / current_axis.0 as f64;
    //     //         let new_height = (factor * current_axis.1 as f64) as u32;
    //     //         // println!("Original: {current_axis:?}, new: ({image_size}, {new_height})");
    //     //         let scaled = image::imageops::resize(
    //     //             unscaled,
    //     //             image_size,
    //     //             new_height,
    //     //             image::imageops::Lanczos3,
    //     //         );
    //     //         let mut final_image = ImageBuffer::<P, Vec<<P as Pixel>::Subpixel>>::from_pixel(image_size, image_size, default_pixel);
    //     //         final_image
    //     //             .copy_from(&scaled, 0, (image_size - new_height) / 2)
    //     //             .unwrap();
    //     //         (image_size, new_height, scaled, final_image)
    //     //     }
    //     //     Axis::Y => {
    //     //         let factor = image_size as f64 / current_axis.1 as f64;
    //     //         let new_width = (factor * current_axis.0 as f64) as u32;
    //     //         // println!("Original: {current_axis:?}, new: ({new_width}, {image_size})");
    //     //         let scaled = image::imageops::resize(
    //     //             unscaled,
    //     //             new_width,
    //     //             image_size,
    //     //             image::imageops::Lanczos3,
    //     //         );
    //     //         let mut final_image = ImageBuffer::<P, Vec<<P as Pixel>::Subpixel>>::from_pixel(image_size, image_size, default_pixel);
    //     //         final_image
    //     //             .copy_from(&scaled, (image_size - new_width) / 2, 0)
    //     //             .unwrap();
    //     //         (new_width, image_size, scaled, final_image)
    //     //     }
    //     // }
    // }

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
