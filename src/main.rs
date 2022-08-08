#[macro_use]
extern crate log;

pub mod colors;
pub mod io;
pub mod scoring;
pub mod triangle;
pub mod vec2;

use std::{
    path::PathBuf,
    sync::{
        atomic::{self, AtomicBool},
        Arc,
    },
    thread::{self, JoinHandle},
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
use scoring::{
    average, get_color_in_triangle, point_in_triangle, rectangle_by_points, score, score_for_group,
};
use triangle::Triangles;
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

    #[clap(help = "file to output to")]
    output: Option<PathBuf>,

    #[clap(long, action, help = "do not display visualizations")]
    no_visuals: bool,

    #[clap(
        long,
        action,
        help = "exit early when no changes occur during an iteration"
    )]
    exit_early: bool,

    #[clap(long, short, arg_enum, value_parser, help = "output format to use")]
    format: Option<OutputFormat>,

    #[clap(
        long,
        arg_enum,
        value_parser,
        help = "method of scoring triangles",
        default_value = "percentile-with-size-weight"
    )]
    scoring: ScoringScheme,
}

#[derive(Debug, Clone, ValueEnum)]
pub enum OutputFormat {
    Svg,
    Image,
    Mindustry,
}
#[derive(Debug, Clone, Copy, ValueEnum, Default)]
pub enum ScoringScheme {
    /// percentile based system that is weighted against small triangles
    #[default]
    PercentileWithSizeWeight,
    ColorspaceOptimized,
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

    let (
        raw_image,
        padded_image,
        (w, h),
        mut recvd_tris,
        mut recvd_iteration,
        proc_thread_comm,
        proc_thread_kill,
        mut proc_thread,
    ) = run_for_image(args.clone());

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

                                let score =
                                    score(t, &colors, &raw_image, args.tri_size, args.scoring);
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
                match proc_thread_comm.try_recv() {
                    // handle incoming updates from proc thread, as well as errors from that thread
                    // this also handles saving the image on exit
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
                            if args.output.is_some() {
                                save(
                                    &recvd_tris,
                                    &raw_image,
                                    args.image_size,
                                    args.output.clone().unwrap(),
                                    args.format.clone().unwrap(),
                                );
                            }
                        }
                    }
                }
            }
        }
    } else {
        if args.output.is_none() {
            warn!("no outputs (visualization or file) are set, so this will take a lot of time to do nothing")
        }
        // run untill computation is done, and then save the image
        loop {
            match proc_thread_comm.recv() {
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
                        if args.output.is_some() {
                            save(
                                &recvd_tris,
                                &raw_image,
                                args.image_size,
                                args.output.clone().unwrap(),
                                args.format.clone().unwrap(),
                            );
                        }
                    }
                    break;
                }
            }
        }
    }

    // send kill signal
    proc_thread_kill.store(true, atomic::Ordering::Relaxed);
    // wait for exit
    loop {
        let _ = proc_thread_comm.try_recv();
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

fn run_for_image(
    args: Args,
) -> (
    RgbImage,
    RgbImage,
    (u32, u32),
    Triangles,
    usize,
    flume::Receiver<(usize, Triangles)>,
    Arc<AtomicBool>,
    Option<JoinHandle<()>>,
) {
    let unscaled = load_image(args.file.clone());
    // scale the image to the size specified in the args, while retainging the aspect ratio
    let (w, h, raw_image, padded_image) = scale_image(unscaled, args.image_size);

    // create the starting grid of triangles
    let original_tris = Triangles::new(
        w + (args.tri_size - w as f64 % args.tri_size.ceil()) as u32,
        h + (args.tri_size - h as f64 % args.tri_size.ceil()) as u32,
        args.tri_size,
    );
    // variables to be filled in by the processing thread
    let recvd_tris = original_tris.clone();
    let recvd_iteration = 0usize;

    // communication between the processing and display threads
    let proc_thread_comm = flume::bounded::<(usize, Triangles)>(2);
    let proc_thread_kill = Arc::new(AtomicBool::new(false));
    let proc_thread_kill2 = proc_thread_kill.clone();

    // copy of inputs for proc thread
    let raw_image2 = raw_image.clone();
    let args2 = args.clone();
    let proc_thread = Some(thread::spawn(move || {
        let image = raw_image2;
        let args = args2;
        let mut tris = original_tris;
        let mut last_tris = tris.clone();
        // counts the number of steps left
        let mut iteration: usize = 0;

        'main: loop {
            if iteration >= args.iterations {
                break;
            }
            let starttime = Instant::now();

            // randomly iterate through the verticies of the grid
            let mut verts = tris.clone().into_iter_verts().collect::<Vec<_>>();
            verts.shuffle(&mut rand::thread_rng());
            for (x, y, _) in verts {
                // for each vertex, run a optimization on it that shifts it to the best nearby position, if there is one.
                optimize_one(&image, &mut tris, (x, y), &args);
                if proc_thread_kill2.load(atomic::Ordering::Relaxed) {
                    break 'main;
                }
            }
            iteration += 1;
            let endtime = Instant::now();
            let opt_dur = endtime - starttime;
            // report back to the display thread with progress to be shown
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
        }
    }));

    (
        raw_image,
        padded_image,
        (w, h),
        recvd_tris,
        recvd_iteration,
        proc_thread_comm.1,
        proc_thread_kill,
        proc_thread,
    )
}

/// finds a new optimal position for a vertex in the grid of triangles
pub fn optimize_one(image: &RgbImage, tris: &mut Triangles, xy: (u32, u32), args: &Args) {
    let shift_amnt = args.shift;
    let randomness = args.randomness;
    // do not move edge verts
    if tris.vert_is_edge(xy.0, xy.1) {
        return;
    }
    // get the triangles around the current point
    let group = tris.triangles_around_point(xy.0, xy.1);
    // and score for that group
    let original_score = score_for_group(image, &group, args.tri_size, args.scoring);

    // possible movements of the point (all directions, and up to some number of steps in that direction)
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
        (1..=4/* number of steps in each direction */) // TODO: make this configurable
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
            let new_score = score_for_group(image, &group, args.tri_size, args.scoring);
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
