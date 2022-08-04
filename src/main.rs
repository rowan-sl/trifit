#[macro_use]
extern crate log;

pub mod colors;
pub mod io;
pub mod triangle;
pub mod scoring;
pub mod vec2;

use std::{
    path::PathBuf,
    sync::{
        atomic::{self, AtomicBool},
        Arc,
    },
    thread,
    time::Instant,
};

use anyhow::Result;
use clap::{ArgGroup, Parser, ValueEnum};
use glutin_window::GlutinWindow;
use image::{DynamicImage, Rgb, RgbImage};
use opengl_graphics::{GlGraphics, OpenGL, Texture, TextureSettings};
use piston::{
    event_loop::{EventSettings, Events},
    window::WindowSettings,
    RenderEvent, Size, UpdateEvent,
};

use colors::*;
use io::{load_image, save, scale_image};
use rand::{prelude::SliceRandom, Rng};
use triangle::Triangles;
use scoring::{
    average, get_color_in_triangle, point_in_triangle, rectangle_by_points, score, score_for_group,
};
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

    #[clap(help="file to output to")]
    output: Option<PathBuf>,

    #[clap(long, action, help = "do not display visualizations")]
    no_visuals: bool,

    #[clap(long, action, help = "exit early when no changes occur during an iteration")]
    exit_early: bool,

    #[clap(long, short, arg_enum, value_parser, help = "output format to use")]
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

    let unscaled = load_image(args.file.clone());
    let (w, h, raw_image, padded_image) = scale_image(unscaled, args.image_size);

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
        let mut last_tris = tris.clone();
        let mut iteration: usize = 0;

        'main: loop {
            let starttime = Instant::now();

            let mut verts = tris.clone().into_iter_verts().collect::<Vec<_>>();
            verts.shuffle(&mut rand::thread_rng());
            for (x, y, _) in verts {
                optimize_one(&image, &mut tris, (x, y), &args);
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
            if args.exit_early {
                if tris == last_tris {
                    println!("No more work to do, finishing early");
                    proc_thread_comm
                        .0
                        .send((usize::MAX /* signals that all iterations are complete, even if they are not */, tris.clone()))
                        .expect("Processing thread exiting -- main thread panic detected");
                    break;
                } else {
                    last_tris = tris.clone();
                }
            }
            if iteration >= args.iterations {
                break;
            }
        }
    }));

    if !args.no_visuals {
        // Change this to OpenGL::V2_1 if not working.
        let opengl = OpenGL::V4_5;

        // Create an Glutin window.
        let mut window: GlutinWindow = WindowSettings::new("trifit", [10, 10])
            .graphics_api(opengl)
            .size(Size {
                width: args.image_size as f64 + 80.0,
                height: args.image_size as f64 + 80.0,
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
            &DynamicImage::ImageRgb8(padded_image).to_rgba8(),
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

                                let score = score(&colors, &raw_image, args.tri_size);
                                assert!(
                                    0.0 <= score && score <= 255.0 * 3.0,
                                    "Score was too large/small! (score: {score})"
                                );
                                // println!("Score: {}", score as u8);
                                t = t.offset(40.0, 40.0);
                                t = t.offset(
                                    (args.image_size - w) as f64 / 2.0,
                                    (args.image_size - h) as f64 / 2.0,
                                );
                                if recvd_iteration < args.iterations {
                                    let color = rgba(
                                        if score <= 255.0 { score as u8 } else { 0 },
                                        if score <= 255.0 * 2.0 && score > 255.0 {
                                            (score - 255.0) as u8
                                        } else {
                                            0
                                        },
                                        if score <= 255.0 * 3.0 && score > 255.0 * 2.0 {
                                            (score - 255.0 * 2.0) as u8
                                        } else {
                                            0
                                        },
                                        1.0,
                                    );
                                    // let color = RED;
                                    t.draw_outline(2.0, color, &c, gl);
                                } else {
                                    let color;
                                    if colors.is_empty() {
                                        color = rgba(0, 0, 0, 0.0);
                                    } else {
                                        let Rgb([r, g, b]) = average(&colors);
                                        color = rgba(r, g, b, 1.0);
                                    }
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
                            save(&recvd_tris, &raw_image, args.image_size, args.output.clone().unwrap(), args.format.clone().unwrap());
                        }
                    }
                }
            }
        }
    } else {
        if args.output.is_none() {
            warn!("no outputs (visualization or file) are set, so this will take a lot of time to do nothing")
        }
        loop {
            match proc_thread_comm.1.recv() {
                Ok(values) => {
                    let _ = values;
                }
                Err(flume::RecvError::Disconnected) => {
                    if let Some(proc_thread) = proc_thread.take() {
                        println!("Procesing thread exiting");
                        match proc_thread.join() {
                            Ok(..) => {}
                            Err(err) => std::panic::panic_any(err),
                        }
                        save(&recvd_tris, &raw_image, args.image_size, args.output.clone().unwrap(), args.format.clone().unwrap());
                    }
                    break;
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

pub fn optimize_one(image: &RgbImage, tris: &mut Triangles, xy: (u32, u32), args: &Args) {
    let shift_amnt = args.shift;
    let randomness = args.randomness;
    // println!("e");
    if tris.vert_is_edge(xy.0, xy.1) {
        // println!("edge");
        return;
    }
    let group = tris.triangles_around_point(xy.0, xy.1);
    let original_score = score_for_group(image, &group, args.tri_size);
    // println!("Opt: ");
    // println!("    Original score: {original_score}");

    let perms = [
        (0.0, 1.0), //   up
        // (0.5, 1.0),
        (1.0, 1.0), //   up right
        // (1.0, 0.5),
        (1.0, 0.0), //   right
        // (1.0, -0.5),
        (1.0, -1.0), //  down right
        // (0.5, -1.0),
        (0.0, -1.0), //  down
        // (-0.5, -1.0),
        (-1.0, -1.0), // down left
        // (-1.0, -0.5),
        (-1.0, 0.0), //  left
        // (-1.0, 0.5),
        (-1.0, 1.0), //  up left
                     // (-0.5, 1.0),
    ]
    .into_iter()
    .map(|(x, y)| (x * shift_amnt, y * shift_amnt))
    .map(|(x, y)| {
        (1..=4)
            .map(|i| (x * i as f64, y * i as f64))
            .collect::<Vec<_>>()
    })
    .flatten();

    let tris2 = tris.clone();

    let (dx, dy, best_score) = perms
        .filter(|(dx, dy)| {
            let new = *tris2.get_vert(xy.0, xy.1) + F64x2::new(*dx, *dy);
            tris2
                .triangles_around_point(xy.0, xy.1)
                .into_iter()
                .map(|t| point_in_triangle(new, t.0, t.1, t.2))
                .any(|x| x)
        })
        .map(|(dx, dy)| {
            let original = *tris.get_vert(xy.0, xy.1);
            let at = tris.get_vert_mut(xy.0, xy.1);
            at.x += dx;
            at.y += dy;
            let group = tris.triangles_around_point(xy.0, xy.1);
            let new_score = score_for_group(image, &group, args.tri_size);
            // println!("    possible new score: {new_score}");
            *tris.get_vert_mut(xy.0, xy.1) = original;
            (dx, dy, new_score)
        })
        .min_by(|(_, _, a), (_, _, b)| a.partial_cmp(b).unwrap())
        .unwrap_or((0f64, 0f64, 0f64));

    if best_score < original_score
        || if randomness != 0 {
            rand::thread_rng().gen_bool(1.0 / randomness as f64)
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
