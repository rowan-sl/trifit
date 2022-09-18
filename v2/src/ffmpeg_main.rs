extern crate ffmpeg_next as ffmpeg;

use ffmpeg::format::{input, Pixel};
use ffmpeg::media::Type;
use ffmpeg::software::scaling::{context::Context, flag::Flags};
use ffmpeg::util::frame::video::Video;

use anyhow::Result;
use image::RgbImage;

use std::time::Instant;
use std::{env, thread};

fn main() -> Result<()> {
    ffmpeg::init().unwrap();

    let numthreads = 1;
    let (frame_sender, frame_quque) = flume::bounded::<RgbImage>(numthreads + 1);
    let (metadata_sender, metadata) = flume::bounded(1);

    let ffmthread: thread::JoinHandle<Result<()>> = thread::spawn(move || {
        if let Ok(mut ictx) = input(&env::args().nth(1).expect("Cannot open file.")) {
            let input = ictx
                .streams()
                .best(Type::Video)
                .ok_or(ffmpeg::Error::StreamNotFound)?;
            let video_stream_index = input.index();

            let context_decoder = ffmpeg::codec::context::Context::from_parameters(input.parameters())?;
            let mut decoder = context_decoder.decoder().video()?;

            metadata_sender.send((
                decoder.height(),
                decoder.width(),
                decoder.aspect_ratio(),
                decoder.format(),
                decoder.frame_rate(),
            ))?;

            let mut scaler = Context::get(
                decoder.format(),
                decoder.width(),
                decoder.height(),
                Pixel::RGB24,
                decoder.width(),
                decoder.height(),
                Flags::BILINEAR,
            )?;

            let done = std::cell::RefCell::new(false);

            let mut receive_and_process_decoded_frames =
                |decoder: &mut ffmpeg::decoder::Video| -> Result<(), ffmpeg::Error> {
                    let mut decoded = Video::empty();
                    while decoder.receive_frame(&mut decoded).is_ok() {
                        let mut rgb_frame = Video::empty();
                        scaler.run(&decoded, &mut rgb_frame)?;
                        // save_file(&rgb_frame, frame_index).unwrap();
                        let image = RgbImage::from_vec(rgb_frame.width(), rgb_frame.height(), rgb_frame.data(0).to_vec()).unwrap();
                        if let Err(..) = frame_sender.send(image) {
                            *done.borrow_mut() = true;
                            break;
                        }
                    }
                    Ok(())
                };

            for (stream, packet) in ictx.packets() {
                if stream.index() == video_stream_index {
                    decoder.send_packet(&packet)?;
                    receive_and_process_decoded_frames(&mut decoder)?;
                    if *done.borrow() { break }
                }
            }
            decoder.send_eof()?;
            receive_and_process_decoded_frames(&mut decoder)?;
        }

        Ok(())
    });

    let (
        height,
        width,
        aspect_ratio,
        format,
        frame_rate,
    ) = metadata.recv()?;

    // let mut octx = ffmpeg::format::output(&"out.mp4")?;
    // let global_header = octx.format().flags().contains(ffmpeg::format::Flags::GLOBAL_HEADER);

    // let mut ost = octx.add_stream(ffmpeg::encoder::find(ffmpeg::codec::Id::AV1))?;
    // let mut encoder = ffmpeg::codec::context::Context::from_parameters(ost.parameters())?.encoder().video()?;

    // encoder.set_height(height);
    // encoder.set_width(width);
    // encoder.set_aspect_ratio(aspect_ratio);
    // encoder.set_format(format);
    // encoder.set_frame_rate(frame_rate);
    // if let Some(frame_rate) = frame_rate {
    //     encoder.set_time_base(frame_rate.invert());
    // }
    // if global_header {
    //     encoder.set_flags(ffmpeg::codec::Flags::GLOBAL_HEADER);
    // }

    // ost.set_parameters(&encoder);
    // ffmpeg::format::context::output::dump(&octx, 0, Some(&"out.mp4"));
    // octx.write_header()?;

    let mut count = 0;
    let startt = Instant::now();
    while let Ok(frame) = frame_quque.recv() {
        count += 1;
    }
    let endt = Instant::now();
    let dur = endt-startt;
    println!("got {count} frames in {dur:?}, or an average of {} frames/s", count as f64 / dur.as_secs_f64());

    ffmthread.join().unwrap()?;

    Ok(())
}
