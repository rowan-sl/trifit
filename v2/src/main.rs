pub mod color;
pub mod vec2;

use std::{
    path::PathBuf,
    cmp, iter
};

use anyhow::Result;
use clap::Parser as _;
use image::{RgbImage, Rgb, GenericImage, DynamicImage};
use glutin_window::GlutinWindow;
use opengl_graphics::{GlGraphics, OpenGL, Texture, TextureSettings};
use palette::FromColor;
use piston::{
    event_loop::{EventSettings, Events},
    window::WindowSettings,
    RenderEvent, Size, UpdateEvent,
};

// use color::{ColorChannel::*, ColorIndex};
use vec2::F64x2;

#[derive(Debug, Clone, clap::Parser)]
#[clap(author, version, about, long_about = None)]
pub struct Args {
    /// Input file to read from
    file: PathBuf,
    /// (optional) output file to use
    #[clap(long, short)]
    output: Option<PathBuf>,
}

fn main() -> Result<()> {
    let args = Args::parse();
    const SCALE: u32 = 1024;

    let input_image = load_image(args.file);
    let (_, _, input_image, _padded_image) = scale_image(input_image, SCALE);

    // println!("{}", color_distance(Rgb([0; 3]), Rgb([255; 3])));

    let result_image = RgbImage::from_fn(input_image.width(), input_image.height(), |x, y| {
        let average_color_in = |iter: &mut dyn Iterator<Item = (i32 ,i32)>| {
            let (len, mut not_scaled) = iter
                .filter_map(|(dx, dy)| Some((u32::try_from(x as i32 + dx).ok()?, u32::try_from(y as i32 + dy).ok()?)))
                .filter_map(|(ox, oy)| input_image.get_pixel_checked(ox, oy))
                .fold((0u64, [0u64, 0u64, 0u64]), |(len, mut acc), Rgb([r, g, b])| {
                    acc[0] += *r as u64;
                    acc[1] += *g as u64;
                    acc[2] += *b as u64;
                    (len + 1, acc)
                });
            if len == 0 {
                return [0; 3]
            }
            not_scaled[0] /= len;
            not_scaled[1] /= len;
            not_scaled[2] /= len;
            not_scaled
        };
        let block = |w: u32, h: u32| -> Vec<(i32, i32)> {
            (0..h).map(|y| iter::zip(0..w, iter::repeat(y))).flatten().map(|(x, y)| (x as i32 - (w / 2) as i32, y as i32 - (h / 2) as i32)).collect()
        };

        const BLOCK_SIZE: u32 = 30;
        let current_color = average_color_in(&mut block(BLOCK_SIZE, BLOCK_SIZE).into_iter()).map(|i| u8::try_from(i).unwrap());
        let surrounding_color = [
            average_color_in(&mut block(BLOCK_SIZE, BLOCK_SIZE).into_iter().map(|(dx, dy)| (dx, dy - BLOCK_SIZE as i32))), // up
            average_color_in(&mut block(BLOCK_SIZE, BLOCK_SIZE).into_iter().map(|(dx, dy)| (dx + BLOCK_SIZE as i32, dy))), // right
            average_color_in(&mut block(BLOCK_SIZE, BLOCK_SIZE).into_iter().map(|(dx, dy)| (dx, dy + BLOCK_SIZE as i32))), // down
            average_color_in(&mut block(BLOCK_SIZE, BLOCK_SIZE).into_iter().map(|(dx, dy)| (dx - BLOCK_SIZE as i32, dy))), // left
        ]
        .iter()
        .fold([0u64; 3], |acc, i| [acc[0] + i[0], acc[1] + i[1], acc[2] + i[2]])
        .map(|i| u8::try_from(i / 4).unwrap());

        Rgb([((color_distance(Rgb(current_color), Rgb(surrounding_color)) * 255.0) / color_distance(Rgb([0; 3]), Rgb([255; 3]))) as u8; 3])

        // Rgb(current_color.map(|x| x as u8))
        // Rgb([
        //     u8::try_from((surrounding_color[0] as i32 - current_color[0] as i32).abs()).unwrap(),
        //     u8::try_from((surrounding_color[1] as i32 - current_color[1] as i32).abs()).unwrap(),
        //     u8::try_from((surrounding_color[2] as i32 - current_color[2] as i32).abs()).unwrap(),
        // ])
        // todo!()
    });
    result_image.save("out2.png")?;

    return Ok(());

    // Change this to OpenGL::V2_1 if not working.
    let opengl = OpenGL::V4_5;

    // Create an Glutin window.
    let mut window: GlutinWindow = WindowSettings::new("trifit", [10, 10])
        .graphics_api(opengl)
        .size(Size {
            width:  input_image.width() as f64 + 80.0,
            height: input_image.height() as f64 + 80.0,
        })
        .resizable(false)
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

    let bg_texture = Texture::from_image(
        &DynamicImage::ImageRgb8(input_image.clone()).to_rgba8(),
        &TextureSettings::new(),
    );

    while let Some(e) = events.next(&mut window) {
        if let Some(render_args) = e.render_args() {
            use graphics::clear;

            gl.draw(render_args.viewport(), |c, gl| {
                clear(rgba(0, 0, 0, 1.0), gl);
                graphics::Image::new()
                    .rect(rectangle_by_points(
                        F64x2::splat(40.0),
                        F64x2::splat(40.0) + F64x2::new(input_image.width() as f64, input_image.height() as f64),
                    ))
                    .draw(
                        &bg_texture,
                        &graphics::DrawState::default(),
                        c.transform,
                        gl,
                    );
            });
        }
        if let Some(_u_args) = e.update_args() { }
    }

    // let max_dist = 100.0;
    //
    // let &start_color = input_image.get_pixel(0,0);
    // // assuming the coordinate space of x+ = right, y+ = down, angle 0 = facing right, and going from 0,0
    // let angle_dist_to_xy = |angle: f64, dist: f64| {
    //     (
    //         angle.cos() * dist,
    //         angle.sin() * dist,
    //     )
    // };
    //
    // let mut points: Vec<(f64, f64)> = vec![]; // (angle, dist) from 0,0 (0 angle = facing right)
    // for raw_angle in 0..1000 {
    //     let angle = (raw_angle as f64 * (PI / 2.0)) / 1000.0;
    //     let mut dist = -0.5;
    //     loop {
    //         dist += 0.5;
    //         let (x, y) = angle_dist_to_xy(angle, dist);
    //         let (x, y) = (x.round() as u32, y.round() as u32);
    //         if x >= input_image.width()
    //         || y >= input_image.height() {
    //             break
    //         }
    //         if color_distance(start_color, *input_image.get_pixel(x, y)) > max_dist {
    //             break
    //         }
    //     }
    //     points.push((angle, dist))
    // }
    //
    // let mut output_image = input_image.clone();
    // for (angle, dist) in points {
    //     let (x, y) = angle_dist_to_xy(angle, dist);
    //     let (x, y) = (x.round().min(input_image.width() as f64 - 1.0) as u32, y.round().min(input_image.width() as f64 - 1.0) as u32);
    //
    //     *output_image.get_pixel_mut(x, y) = Rgb([255, 0, 0])
    // }
    //
    // output_image.save("out.png")?;

    // let all_colors = input_image.pixels().copied().collect::<Vec<_>>();
    //
    // let palette_size = 32;
    //
    // let mut conditions = rscolorq::Params::new();
    // conditions.palette_size(palette_size);
    // conditions.dithering_level(0.0001);
    // conditions.verify_parameters()?;
    // let mut quantized_image = rscolorq::Matrix2d::new(input_image.width() as usize, input_image.height() as usize);
    //
    // let image = rscolorq::Matrix2d::from_vec(
    //     input_image.pixels()
    //         .map(|&c| rscolorq::color::Rgb {
    //             red: c[0] as f64 / 255.0,
    //             green: c[1] as f64 / 255.0,
    //             blue: c[2] as f64 / 255.0,
    //         })
    //         .collect(),
    //     input_image.width() as usize,
    //     input_image.height() as usize,
    // );
    // let mut palette = Vec::with_capacity(palette_size as usize);
    // rscolorq::spatial_color_quant(&image, &mut quantized_image, &mut palette, &conditions)?;
    // let palette = palette
    //     .iter()
    //     .map(|&c| {
    //         let color = 255.0 * c;
    //         [
    //             color.red.round() as u8,
    //             color.green.round() as u8,
    //             color.blue.round() as u8,
    //         ]
    //     })
    //     .collect::<Vec<[u8; 3]>>();
    //
    // let final_image = RgbImage::from_vec(input_image.width(), input_image.height(), quantized_image.iter()
    //     .map(|i| palette[*i as usize])
    //     .flatten()
    //     .collect::<Vec<_>>()).unwrap();
    //
    // final_image.save("out.png")?;

    Ok(())
}

pub fn load_image(file: PathBuf) -> RgbImage {
    let path = file.canonicalize().expect("invalid path!");
    assert!(path.exists(), "input file must exist!");

    let dyn_img = image::open(path).expect("input file must be an image!");
    dyn_img.to_rgb8()
}

pub fn scale_image(unscaled: RgbImage, scale_to: u32) -> (u32, u32, RgbImage, RgbImage) {
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

pub fn color_distance(c1: Rgb<u8>, c2: Rgb<u8>) -> f64 {
    let r = c1.0[0] as f64 - c2.0[0] as f64;
    let g = c1.0[1] as f64 - c2.0[1] as f64;
    let b = c1.0[2] as f64 - c2.0[2] as f64;
    (r.powi(2) + g.powi(2) + b.powi(2)).sqrt()
}

/// 0-360, 0-1, 0-1, 0-1
pub fn hsla(h: f32, s: f32, l: f32, a: f32) -> [f32; 4] {
    let converted = palette::rgb::Rgb::from_color(palette::Hsl::new(h, s, l)).into_components();
    [converted.0, converted.1, converted.2, a]
}

pub fn rgba(r: u8, g: u8, b: u8, a: f32) -> [f32; 4] {
    [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, a]
}

pub fn rectangle_by_points(c0: F64x2, c1: F64x2) -> [f64; 4] {
    graphics::rectangle::rectangle_by_corners(c0.x, c0.y, c1.x, c1.y)
}

