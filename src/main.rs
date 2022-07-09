pub mod vec2;
pub mod utils;

use std::{time::Instant, cmp::Ordering};

use anyhow::Result;
use image::{Rgb, RgbImage, RgbaImage, DynamicImage};
use macroquad::prelude::*;

use vec2::F64x2;
use utils::{get_color_in_triangle, score, score_for_group, average};

/// Notes on formatting
///
/// cords into an image are stored as f64, with a constant range of 0.0-1000.0 (mapped to image)
///
const _A: () = ();

// size of the image to render
const IMAGE_SIZE: u32 = 900;
// size of triangles
const TRI_SIZE: f64 = 5.0;
// iterations to perform
const ITERATIONS: i32 = 100;
// filepath of the input image
// const IMAGE_NAME: &str = "img/gandalf.gif";
// const FORMAT: InputFormat = InputFormat::Gif;
// const IMAGE_NAME: &str = "img/ted-hiking-002.jpg";
// const IMAGE_NAME: &str = "img/wild_potatoes.jpeg";
// const IMAGE_NAME: &str = "img/aroura_sky.jpg";
const IMAGE_NAME: &str = "img/fire.jpg";
const FORMAT: InputFormat = InputFormat::Image;
const RANDOM_CHANCE_QTY: usize = 30;
const SHIFT_AMNT: f64 = 0.5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InputFormat {
    Gif,
    Image,
}

#[macroquad::main("trifit - test")]
async fn main() -> Result<()> {
    clear_background(BLACK);
    let mut input_image;
    match FORMAT {
        InputFormat::Gif => {
            use std::fs::File;
            let mut decoder = gif::DecodeOptions::new();
            // Configure the decoder such that it will expand the image to RGBA.
            decoder.set_color_output(gif::ColorOutput::RGBA);
            // Read the file header
            let file = File::open("gandalf.gif").unwrap();
            let mut decoder = decoder.read_info(file).unwrap();
            let first_frame = decoder.read_next_frame().unwrap().unwrap();
            let img = RgbaImage::from_raw(first_frame.width as u32, first_frame.height as u32, first_frame.buffer.to_vec()).unwrap();
            input_image = DynamicImage::ImageRgba8(img).to_rgb8();
        }
        InputFormat::Image => {
            input_image = image::open(IMAGE_NAME)?.to_rgb8();
        }
    }
    enum Axis { X, Y }
    let current_axis: (u32, u32) = (input_image.width(), input_image.height());
    let larger = match current_axis.0.cmp(&current_axis.1) {
        Ordering::Greater => Axis::X,
        Ordering::Equal => Axis::X,
        Ordering::Less => Axis::Y
    };
    match larger {
        Axis::X => {
            let factor = IMAGE_SIZE as f64 / current_axis.0 as f64;
            let new_height = (factor * current_axis.1 as f64) as u32;
            println!("Original: {current_axis:?}, new: ({IMAGE_SIZE}, {new_height})");
            input_image = image::imageops::resize(&input_image, IMAGE_SIZE, new_height, image::imageops::Lanczos3);
            // input_image = RgbImage::from_pixel(IMAGE_SIZE, IMAGE_SIZE, Rgb([0; 3]));
            // input_image.copy_from(&scaled, 0, (IMAGE_SIZE - new_height) / 2).unwrap();
        }
        Axis::Y => {
            let factor = IMAGE_SIZE as f64 / current_axis.1 as f64;
            let new_width = (factor * current_axis.0 as f64) as u32;
            println!("Original: {current_axis:?}, new: ({new_width}, {IMAGE_SIZE})");
            input_image = image::imageops::resize(&input_image, new_width, IMAGE_SIZE, image::imageops::Lanczos3);
            // input_image = RgbImage::from_pixel(IMAGE_SIZE, IMAGE_SIZE, Rgb([0; 3]));
            // input_image.copy_from(&scaled, (IMAGE_SIZE - new_width) / 2, 0).unwrap();
        }
    }

    let mut image = Image::gen_image_color(
        input_image.width() as u16,
        input_image.height() as u16,
        WHITE,
    );
    for (x, y, pxl) in input_image.enumerate_pixels() {
        image.set_pixel(x, y, Color::from_rgba(pxl.0[0], pxl.0[1], pxl.0[2], 255));
    }
    let bg = Texture2D::from_image(&image);

    // let (_, _, buffer) = generate_regular_points(IMAGE_SIZE, IMAGE_SIZE, 50.0);
    // let buffer = buffer.into_iter().flatten().collect::<Vec<_>>();
    let mut tris = Triangles::new(image.width() as u32, image.height() as u32, TRI_SIZE);


    let mut i = 0;
    loop {
        let starttime0 = Instant::now();
        if i < ITERATIONS {
            for (x, y, _) in tris.clone().into_iter_verts() {
                optimize_one(&input_image, &mut tris, (x, y));
            }
            i += 1;
        }
        let endtime0 = Instant::now();
        let opt_dur = endtime0 - starttime0;
        let starttime = Instant::now();
        draw_texture(bg, 40.0, 40.0, WHITE);
        // for (s_x, s_y, vert) in tris.clone().into_iter_verts() {
        //     let color = if tris.vert_is_edge(s_x, s_y) {
        //         RED
        //     } else {
        //         if s_y % 2 == 1 {
        //             GREEN
        //         } else {
        //             BLUE
        //         }
        //     };
        //     draw_circle(vert.x as f32 + 40.0, vert.y as f32 + 40.0, 5.0, color);
        // }

        for (x, y, _) in tris.clone().into_iter_verts() {
            tris.triangles_around_point(x, y)
                .into_iter()
                .for_each(|mut t| {
                    let colors = get_color_in_triangle(&input_image, t);

                    // let avg = average(&colors);
                    // let color = Color::from_rgba(avg.0[0], avg.0[1], avg.0[2], 255);

                    let score = score(&colors);
                    assert!(0.0 <= score && score <= 255.0);
                    // println!("Score: {}", score as u8);
                    t = t.offset(40.0, 40.0);
                    if i < ITERATIONS {
                        let color = Color::from_rgba((score as u8).saturating_mul(3), 0, 0, 255);
                        t.draw_outline(3.0, color);
                    } else {
                        let Rgb([r, g, b]) = average(&colors);
                        let color = Color::from_rgba(r, g, b, 255);
                        t.draw(color);
                    }
                });
            }

        // test of get_rel:
        // draw_circle(tris.get_vert(7, 7).x as f32 + 40.0, tris.get_vert(7, 7).y as f32 + 40.0, 5.0, PURPLE);
        // let draw = |pos| {
        //     let vert = tris.get_rel(7, 7, pos);
        //     draw_circle(vert.x as f32 + 40.0, vert.y as f32 + 40.0, 5.0, PINK);
        // };
        // draw(RelVertPos::UpRight);
        let endtime = Instant::now();
        println!("Frame:");
        if i < ITERATIONS {
            println!("    optimizer iteration {}", i);
            println!("    optimization took {:?}", opt_dur);
        } else {
            println!("    optimization done!");
        }
        println!("    visualization drawing took {:?}", endtime-starttime);
        next_frame().await;
    }
    // Ok(())
}

pub fn optimize_one(image: &RgbImage, tris: &mut Triangles, xy: (u32, u32)) {
    // println!("e");
    if tris.vert_is_edge(xy.0, xy.1) {
        // println!("edge");
        return;
    }
    let group = tris.triangles_around_point(xy.0, xy.1);
    let original_score = score_for_group(image, &group);
    // println!("Opt: ");
    // println!("    Original score: {original_score}");

    let perms = [
        (0, 1),//   up
        (1, 1),//   up right
        (1, 0),//   right
        (1, -1),//  down right
        (0, -1),//  down
        (-1, -1),// down left
        (-1, 0),//  left
        (-1, 1),//  up left
    ].map(|(x, y)| (x as f64 * SHIFT_AMNT, y as f64 * SHIFT_AMNT));
    let (dx, dy, best_score) = perms.into_iter()
        .map(|(dx, dy)| {
            let original = *tris.get_vert(xy.0, xy.1);
            let at = tris.get_vert_mut(xy.0, xy.1);
            at.x += dx;
            at.y += dy;
            let group = tris.triangles_around_point(xy.0, xy.1);
            let new_score = score_for_group(image, &group);
            // println!("    possible new score: {new_score}");
            *tris.get_vert_mut(xy.0, xy.1) = original;
            (dx, dy, new_score)
        })
        .min_by(|(_, _, a), (_, _, b)| a.partial_cmp(b).unwrap()).unwrap();

    if best_score < original_score || rand::gen_range(0, RANDOM_CHANCE_QTY) == 1 {
        // println!("yay");
        let at = tris.get_vert_mut(xy.0, xy.1);
        at.x += dx;
        at.y += dy;
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Triangles {
    vbuf: Vec<Vec<F64x2>>,
    scale_size: (u32, u32), // MAY NOT CORRISPOND TO vbuf sizes!
    real_size: (u32, u32),
    size_of_chunk: f64,
}

impl Triangles {
    pub fn new(width: u32, height: u32, size: f64) -> Self {
        let (scale_width, scale_height, buffer) = generate_regular_points(width, height, size);
        Self {
            vbuf: buffer,
            scale_size: (scale_width, scale_height),
            real_size: (width, height),
            size_of_chunk: size,
        }
    }

    pub fn triangles_around_point(&self, x: u32, y: u32) -> Vec<Triangle> {
        self.triangle_locations_around_point(x, y)
            .into_iter()
            .map(|p| {
                Triangle(
                    *self.get_vert(p[0].0, p[0].1),
                    *self.get_vert(p[1].0, p[1].1),
                    *self.get_vert(p[2].0, p[2].1),
                )
            })
            .collect()
    }

    pub fn triangle_locations_around_point(&self, x: u32, y: u32) -> Vec<[(u32, u32); 3]> {
        use RelVertPos::*;

        let get_if_exists = |o1, o2| {
            let a = self.pos_rel(x, y, o1)?;
            let b = self.pos_rel(x, y, o2)?;
            Some((a, b))
        };
        let perms = [
            (UpLeft, UpRight),
            (UpRight, Right),
            (Right, DownRight),
            (DownRight, DownLeft),
            (DownLeft, Left),
            (Left, UpLeft),
        ];
        perms
            .into_iter()
            .map(|p| get_if_exists(p.0, p.1))
            .filter(|o| o.is_some())
            .map(|o| o.unwrap())
            .map(|(b, c)| [(x, y), b, c])
            .collect()
    }

    pub fn pos_rel(&self, x: u32, y: u32, pos: RelVertPos) -> Option<(u32, u32)> {
        use RelVertPos::*;
        let y = match pos {
            UpLeft | UpRight => y.checked_sub(1)?,
            DownLeft | DownRight => y + 1,
            Left | Right => y,
        };
        let x = match pos {
            Left => x.checked_sub(1)?,
            Right => x + 1,
            UpLeft | DownLeft => x.checked_sub(if y % 2 == 1 { 1 } else { 0 })?,
            UpRight | DownRight => x + if y % 2 == 0 { 1 } else { 0 },
        };
        self.try_get_vert(x, y)?;
        Some((x, y))
    }

    pub fn vert_is_edge(&self, x: u32, y: u32) -> bool {
        let o = if y % 2 == 1 { 0 } else { 1 };
        x == 0 || x >= self.scale_size.0 + o || y == 0 || y >= self.scale_size.1
    }

    /// x and y are in SCALE units
    pub fn get_vert(&self, x: u32, y: u32) -> &F64x2 {
        &self.vbuf[y as usize][x as usize]
    }

    /// x and y are in SCALE units
    pub fn try_get_vert(&self, x: u32, y: u32) -> Option<&F64x2> {
        Some(self.vbuf.get(y as usize)?.get(x as usize)?)
    }

    /// x and y are in SCALE units
    pub fn get_vert_mut(&mut self, x: u32, y: u32) -> &mut F64x2 {
        &mut self.vbuf[y as usize][x as usize]
    }

    /// x and y are in SCALE units
    pub fn try_get_vert_mut(&mut self, x: u32, y: u32) -> Option<&mut F64x2> {
        Some(self.vbuf.get_mut(y as usize)?.get_mut(x as usize)?)
    }

    pub fn into_iter_verts(self) -> impl Iterator<Item = (u32, u32, F64x2)> {
        let mut tmp = vec![];
        for (scale_y, row) in self.vbuf.into_iter().enumerate() {
            for (scale_x, vert) in row.into_iter().enumerate() {
                tmp.push((scale_x as u32, scale_y as u32, vert));
            }
        }
        tmp.into_iter()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RelVertPos {
    UpRight,
    Right,
    DownRight,
    DownLeft,
    Left,
    UpLeft,
}

/// Note: some points will end up `(size/2.0)` away from the size set (in x axis)
pub fn generate_regular_points(width: u32, height: u32, size: f64) -> (u32, u32, Vec<Vec<F64x2>>) {
    assert!(size.is_sign_positive());
    assert!(size.is_normal());
    let real_scale_width = (width as f64 / size) as u32;
    let scale_width = real_scale_width + 1;
    let scale_height = (height as f64 / size) as u32;
    // start at (0, 0)
    // x -> width
    // y -> height
    let mut x: u32 = 0;
    let mut y: u32 = 0;
    let mut buf: Vec<Vec<F64x2>> = vec![];
    let mut current_row: Vec<F64x2> = vec![];
    loop {
        let offset: f64 = if y % 2 == 1 {
            if x > real_scale_width {
                x = 0;
                y += 1;
                buf.push(current_row);
                current_row = vec![];
                continue;
            }
            0.0
        } else {
            -size / 2.0
        };

        let vertex = F64x2::new((x as f64 * size) + offset, y as f64 * size);
        // println!("{vertex:?}");
        current_row.push(vertex);

        x += 1;
        if x > scale_width {
            x = 0;
            y += 1;
            buf.push(current_row);
            current_row = vec![];
        }
        if y > scale_height {
            buf.push(current_row);
            break;
        }
    }
    (real_scale_width, scale_height, buf)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Triangle(F64x2, F64x2, F64x2);

impl Triangle {
    pub fn offset(mut self, x: f64, y: f64) -> Self {
        self.0.x += x;
        self.1.x += x;
        self.2.x += x;
        self.0.y += y;
        self.1.y += y;
        self.2.y += y;
        self
    }

    pub fn draw_outline(&self, thickness: f32, color: Color) {
        draw_triangle_lines(
            self.0.into(),
            self.1.into(),
            self.2.into(),
            thickness,
            color,
        )
    }

    pub fn draw(&self, color: Color) {
        draw_triangle(
            self.0.into(),
            self.1.into(),
            self.2.into(),
            color,
        )
    }
}

impl Into<Vec2> for F64x2 {
    fn into(self) -> Vec2 {
        Vec2::new(self.x as f32, self.y as f32)
    }
}
