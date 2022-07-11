#[macro_use]
extern crate log;

pub mod colors;
pub mod utils;
pub mod vec2;

use std::{
    cmp,
    path::PathBuf,
    sync::{
        atomic::{self, AtomicBool},
        Arc,
    },
    thread,
    time::Instant, fs::OpenOptions, io::Write,
};

use anyhow::Result;
use clap::{ArgGroup, Parser, ValueEnum};
use glutin_window::GlutinWindow;
use graphics::line;
use image::{DynamicImage, GenericImage, Rgb, RgbImage, RgbaImage};
use opengl_graphics::{GlGraphics, OpenGL, Texture, TextureSettings};
use piston::{
    event_loop::{EventSettings, Events},
    window::WindowSettings,
    RenderEvent, Size, UpdateEvent,
};

use colors::*;
use rand::Rng;
use utils::{average, get_color_in_triangle, rectangle_by_points, score, score_for_group};
use vec2::F64x2;

#[derive(Debug, Clone, Parser)]
#[clap(author, version, about, long_about = None)]
#[clap(group(
            ArgGroup::new("out")
                .required(false)
                .args(&["output"])
                .requires("format"),
        ))]
pub struct Args {
    file: PathBuf,

    #[clap(long, help = "size of image to render")]
    image_size: u32,

    #[clap(long, help = "size of each triangle")]
    tri_size: f64,

    #[clap(long, help = "num of iterations to perform")]
    iterations: usize,

    #[clap(long, help = "ammount to move each vertex each iteration")]
    shift: f64,

    #[clap(
        long,
        help = "chance that a new option will be chosen even if it is worse",
        default_value = "0"
    )]
    randomness: usize,

    output: Option<PathBuf>,

    #[clap(long, action)]
    no_visuals: bool,

    #[clap(long, short, arg_enum, value_parser)]
    format: Option<OutputFormat>,
}

#[derive(Debug, Clone, ValueEnum)]
pub enum OutputFormat {
    Svg,
    Image,
    Mindustry,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InputFormat {
    Gif,
    Image,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    pretty_env_logger::formatted_builder()
        .filter_level(log::LevelFilter::Trace)
        .init();

    info!("Initialized");

    // Change this to OpenGL::V2_1 if not working.
    let opengl = OpenGL::V4_5;

    // Create an Glutin window.
    let mut window: GlutinWindow = WindowSettings::new("limeon", [200, 200])
        .graphics_api(opengl)
        .size(Size {
            width: 1_000.0,
            height: 700.0,
        })
        .vsync(true)
        .controllers(true)
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

    let unscaled = load_image(&args);
    let (w, h, raw_image, padded_image) = scale_image(unscaled, &args);
    let bg_texture = Texture::from_image(
        &DynamicImage::ImageRgb8(padded_image).to_rgba8(),
        &TextureSettings::new(),
    );

    let original_tris = Triangles::new(
        w + (args.tri_size - w as f64 % args.tri_size.ceil()) as u32,
        h + (args.tri_size - h as f64 % args.tri_size.ceil()) as u32,
        args.tri_size,
    );
    let mut recvd_tris = original_tris.clone();
    let mut recvd_iteration = 0usize;

    let proc_thread_comm = flume::bounded::<(usize, Triangles)>(2);
    let proc_thread_kill = Arc::new(AtomicBool::new(false));
    let proc_thread_kill2 = proc_thread_kill.clone();

    let raw_image2 = raw_image.clone();
    let args2 = args.clone();
    let mut proc_thread = Some(thread::spawn(move || {
        let image = raw_image2;
        let args = args2;
        let mut tris = original_tris;
        let mut iteration: usize = 0;

        'main: loop {
            let starttime = Instant::now();
            for (x, y, _) in tris.clone().into_iter_verts() {
                optimize_one(&image, &mut tris, (x, y), args.shift, args.randomness);
                if proc_thread_kill2.load(atomic::Ordering::Relaxed) {
                    break 'main;
                }
            }
            iteration += 1;
            let endtime = Instant::now();
            let opt_dur = endtime - starttime;
            proc_thread_comm
                .0
                .send((iteration, tris.clone()))
                .expect("Processing thread exiting -- main thread panic detected");
            println!("Optimizer step");
            println!("    iteration #{iteration}");
            println!("    took {opt_dur:?}");
            if iteration >= args.iterations {
                break;
            }
        }
    }));

    while let Some(e) = events.next(&mut window) {
        if let Some(render_args) = e.render_args() {
            use graphics::clear;

            gl.draw(render_args.viewport(), |c, gl| {
                clear(rgba(0, 0, 0, 1.0), gl);
                graphics::Image::new()
                    .rect(rectangle_by_points(
                        F64x2::splat(40.0),
                        F64x2::splat(40.0) + F64x2::splat(args.image_size as f64),
                    ))
                    .draw(
                        &bg_texture,
                        &graphics::DrawState::default(),
                        c.transform,
                        gl,
                    );

                for (x, y, _) in recvd_tris.clone().into_iter_verts() {
                    recvd_tris
                        .triangles_around_point(x, y)
                        .into_iter()
                        .for_each(|mut t| {
                            let colors = get_color_in_triangle(&raw_image, t);

                            // let avg = average(&colors);
                            // let color = Color::from_rgba(avg.0[0], avg.0[1], avg.0[2], 255);

                            let score = score(&colors);
                            assert!(0.0 <= score && score <= 255.0);
                            // println!("Score: {}", score as u8);
                            t = t.offset(40.0, 40.0);
                            t = t.offset(
                                (args.image_size - w) as f64 / 2.0,
                                (args.image_size - h) as f64 / 2.0,
                            );
                            if recvd_iteration < args.iterations {
                                let color = rgba((score as u8).saturating_mul(3), 0, 0, 1.0);
                                // let color = RED;
                                t.draw_outline(2.0, color, &c, gl);
                            } else {
                                let Rgb([r, g, b]) = average(&colors);
                                let color = rgba(r, g, b, 1.0);
                                t.draw(color, &c, gl);
                            }
                        });
                }
            });
        }

        if let Some(_u_args) = e.update_args() {
            match proc_thread_comm.1.try_recv() {
                Ok(values) => {
                    (recvd_iteration, recvd_tris) = values;
                }
                Err(flume::TryRecvError::Empty) => {}
                Err(flume::TryRecvError::Disconnected) => {
                    if let Some(proc_thread) = proc_thread.take() {
                        println!("Procesing thread exiting");
                        match proc_thread.join() {
                            Ok(..) => {}
                            Err(err) => std::panic::panic_any(err),
                        }
                        save(&recvd_tris, &raw_image, &args);
                    }
                }
            }
        }
    }

    proc_thread_kill.store(true, atomic::Ordering::Relaxed);
    loop {
        let _ = proc_thread_comm.1.try_recv();
        if let Some(proc_thread) = proc_thread.as_ref() {
            if proc_thread.is_finished() {
                break;
            }
        } else {
            break;
        }
    }
    Ok(())
}

pub fn save(tris: &Triangles, image: &RgbImage, args: &Args) {
    if let Some(out_file) = args.output.clone() {
        match args.format.as_ref().unwrap() {
            OutputFormat::Svg => {
                let svg = make_svg(tris, image, args);
                OpenOptions::new().create(true).write(true).truncate(true).open(&out_file).unwrap().write_all(svg.as_bytes()).unwrap();
            }
            OutputFormat::Image => {
                let svg = make_svg(tris, image, args);// lies and deceit! (its svgs all the way down)
                let tree = usvg::Tree::from_str(&svg, &usvg::Options::default().to_ref()).unwrap();
                let mut bytes = vec![0u8; (image.width() * image.height() * 4) as usize];
                let pixmap = tiny_skia::PixmapMut::from_bytes(bytes.as_mut_slice(), image.width(), image.height()).unwrap();
                resvg::render(&tree, usvg::FitTo::Original, tiny_skia::Transform::default(), pixmap);
                let image = RgbaImage::from_vec(image.width(), image.height(), bytes).unwrap();
                image.save(&out_file).unwrap();
            }
            OutputFormat::Mindustry => {
                todo!()
            }
        }
        println!("Saved to {out_file:?}");
    }
}

pub fn make_svg(tris: &Triangles, image: &RgbImage, args: &Args) -> String {
    use svg::{node::element::Polygon, Document};

    let nodes = tris
        .clone()
        .into_iter_verts()
        .map(|v| [(true, v), (false, v)])
        .flatten()
        .map::<Option<Polygon>, _>(|(flipflop, (rx, ry, tri))| {
            let (rx1, ry1): (u32, u32);
            let (rx2, ry2): (u32, u32);
            if flipflop {
                (rx1, ry1) = tris.pos_rel(rx, ry, RelVertPos::DownRight)?;
                (rx2, ry2) = tris.pos_rel(rx, ry, RelVertPos::DownLeft)?;
            } else {
                (rx1, ry1) = tris.pos_rel(rx, ry, RelVertPos::Right)?;
                (rx2, ry2) = tris.pos_rel(rx, ry, RelVertPos::DownRight)?;
            }
            let verts = (
                tri,
                *tris.get_vert(rx1, ry1),
                *tris.get_vert(rx2, ry2),
            );
            let colors = average(&get_color_in_triangle(
                image,
                Triangle(verts.0, verts.1, verts.2),
            ));
            Some(
                Polygon::new()
                    .set(
                        "fill",
                        format!("rgb({}, {}, {})", colors.0[0], colors.0[1], colors.0[2]),
                    )
                    .set(
                        "stroke",
                        format!("rgb({}, {}, {})", colors.0[0], colors.0[1], colors.0[2]),
                    )
                    .set(
                        "points",
                        format!(
                            "{},{} {},{} {},{}",
                            verts.0.x, verts.0.y, verts.1.x, verts.1.y, verts.2.x, verts.2.y
                        ),
                    ),
            )
        });
    let mut doc = Document::new().set("viewBox", (0, 0, args.image_size, args.image_size));
    for node in nodes {
        if let Some(node) = node {
            doc = doc.add(node);
        }
    }
    doc.to_string()
}

pub fn load_image(args: &Args) -> RgbImage {
    let path = args.file.canonicalize().expect("invalid path!");
    assert!(path.exists(), "input file must exist!");
    // let extension = path.extension().expect("File does not have an extension").to_str().expect("File extension must be valid UTF-8");
    let gif_decoder = {
        use std::fs::File;
        let mut decoder = gif::DecodeOptions::new();
        // Configure the decoder such that it will expand the image to RGBA.
        decoder.set_color_output(gif::ColorOutput::RGBA);
        // Read the file header
        let file = File::open(&path).expect("Cannot open input file!");
        decoder.read_info(file)
    };

    let image_decoder = (|| {
        let dyn_img = image::open(path)?;
        Ok::<_, image::ImageError>(dyn_img.to_rgb8())
    })();

    match (gif_decoder, image_decoder) {
        (Ok(..), Ok(..)) => unreachable!("Input cannot be an image and a gif!"),
        (Ok(mut gif_decoder), Err(..)) => {
            let first_frame = gif_decoder.read_next_frame().unwrap().unwrap();
            let img = RgbaImage::from_raw(
                first_frame.width as u32,
                first_frame.height as u32,
                first_frame.buffer.to_vec(),
            )
            .unwrap();
            DynamicImage::ImageRgba8(img).to_rgb8()
        }
        (Err(..), Ok(image)) => image,
        (Err(..), Err(..)) => panic!("Input is not a gif or an image"),
    }
}

fn scale_image(unscaled: RgbImage, args: &Args) -> (u32, u32, RgbImage, RgbImage) {
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

    let image_size = args.image_size;

    match larger {
        Axis::X => {
            let factor = image_size as f64 / current_axis.0 as f64;
            let new_height = (factor * current_axis.1 as f64) as u32;
            println!("Original: {current_axis:?}, new: ({image_size}, {new_height})");
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
            println!("Original: {current_axis:?}, new: ({new_width}, {image_size})");
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

pub fn optimize_one(
    image: &RgbImage,
    tris: &mut Triangles,
    xy: (u32, u32),
    shift_amnt: f64,
    randomness: usize,
) {
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
        (0, 1),   //   up
        (1, 1),   //   up right
        (1, 0),   //   right
        (1, -1),  //  down right
        (0, -1),  //  down
        (-1, -1), // down left
        (-1, 0),  //  left
        (-1, 1),  //  up left
    ]
    .map(|(x, y)| (x as f64 * shift_amnt, y as f64 * shift_amnt));
    let (dx, dy, best_score) = perms
        .into_iter()
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
        .min_by(|(_, _, a), (_, _, b)| a.partial_cmp(b).unwrap())
        .unwrap();

    if best_score < original_score
        || if randomness != 0 {
            rand::thread_rng().gen_range(0..randomness) == 1
        } else {
            false
        }
    {
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

    pub fn draw_outline(
        &self,
        thickness: f64,
        color: Color,
        c: &graphics::Context,
        gl: &mut GlGraphics,
    ) {
        line(
            color,
            thickness,
            [self.0.x, self.0.y, self.1.x, self.1.y],
            c.transform,
            gl,
        );
        line(
            color,
            thickness,
            [self.1.x, self.1.y, self.2.x, self.2.y],
            c.transform,
            gl,
        );
        line(
            color,
            thickness,
            [self.2.x, self.2.y, self.0.x, self.0.y],
            c.transform,
            gl,
        );

        // draw_triangle_lines(
        //     self.0.into(),
        //     self.1.into(),
        //     self.2.into(),
        //     thickness,
        //     color,
        // )
    }

    pub fn draw(&self, color: Color, c: &graphics::Context, gl: &mut GlGraphics) {
        use graphics::{DrawState, Polygon};
        Polygon::new(color).draw(
            &[
                [self.0.x, self.0.y],
                [self.1.x, self.1.y],
                [self.2.x, self.2.y],
            ][..],
            &DrawState::default(),
            c.transform,
            gl,
        );
        // draw_triangle(
        //     self.0.into(),
        //     self.1.into(),
        //     self.2.into(),
        //     color,
        // )
    }
}
