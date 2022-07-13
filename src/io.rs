use std::{cmp, fs::OpenOptions, io::Write};

use image::{DynamicImage, GenericImage, Rgb, RgbImage, RgbaImage};

use crate::{
    triangle::{RelVertPos, Triangle, Triangles},
    utils::{average, get_color_in_triangle},
    Args, OutputFormat,
};

pub fn save(tris: &Triangles, image: &RgbImage, args: &Args) {
    if let Some(out_file) = args.output.clone() {
        match args.format.as_ref().unwrap() {
            OutputFormat::Svg => {
                let svg = make_svg(tris, image, args);
                OpenOptions::new()
                    .create(true)
                    .write(true)
                    .truncate(true)
                    .open(&out_file)
                    .unwrap()
                    .write_all(svg.as_bytes())
                    .unwrap();
            }
            OutputFormat::Image => {
                let svg = make_svg(tris, image, args); // lies and deceit! (its svgs all the way down)
                let tree = usvg::Tree::from_str(&svg, &usvg::Options::default().to_ref()).unwrap();
                let mut bytes = vec![0u8; (image.width() * image.height() * 4) as usize];
                let pixmap = tiny_skia::PixmapMut::from_bytes(
                    bytes.as_mut_slice(),
                    image.width(),
                    image.height(),
                )
                .unwrap();
                resvg::render(
                    &tree,
                    usvg::FitTo::Original,
                    tiny_skia::Transform::default(),
                    pixmap,
                );
                let image = RgbaImage::from_vec(image.width(), image.height(), bytes).unwrap();
                image.save(&out_file).unwrap();
            }
            OutputFormat::Mindustry => {
                type Inst = (Rgb<u8>, Triangle);
                let mut map = std::collections::HashMap::<Rgb<u8>, Vec<Triangle>>::new();
                for (rgb, tri) in tris.clone()
                    .into_iter_verts()
                    .map(|v| [(true, v), (false, v)])
                    .flatten()
                    .map::<Option<Inst>, _>(|(flipflop, (rx, ry, tri))| {
                        let (rx1, ry1): (u32, u32);
                        let (rx2, ry2): (u32, u32);
                        if flipflop {
                            (rx1, ry1) = tris.pos_rel(rx, ry, RelVertPos::DownRight)?;
                            (rx2, ry2) = tris.pos_rel(rx, ry, RelVertPos::DownLeft)?;
                        } else {
                            (rx1, ry1) = tris.pos_rel(rx, ry, RelVertPos::Right)?;
                            (rx2, ry2) = tris.pos_rel(rx, ry, RelVertPos::DownRight)?;
                        }
                        let verts = (tri, *tris.get_vert(rx1, ry1), *tris.get_vert(rx2, ry2));
                        let color = average(&get_color_in_triangle(
                            image,
                            Triangle(verts.0, verts.1, verts.2),
                        ));
                        Some((
                            color,
                            Triangle(verts.0, verts.1, verts.2)
                        ))
                    })
                    .filter(|i| i.is_some())
                    .map(|i| i.unwrap())
                {
                    if map.contains_key(&rgb) {
                        map.get_mut(&rgb).unwrap().push(tri);
                    } else {
                        map.insert(rgb, vec![tri]);
                    }
                }

                let path = out_file.with_extension("").to_str().unwrap().to_string();
                let mut res = String::new();
                let mut count = 0usize;
                let mut icount = 0usize;
                let mut fcount = 0usize;
                for (color, locations) in map {
                    res.push_str(&format!(
                        "draw color {r} {g} {b} 0 0 0\n",
                        r = color.0[0],
                        g = color.0[1],
                        b = color.0[2],
                    ));
                    count += 1;
                    icount += 1;
                    for mut tri in locations {
                        tri.0.y = image.height() as f64 - tri.0.y + ((args.image_size - image.height()) as f64 / 2.0);
                        tri.1.y = image.height() as f64 - tri.1.y + ((args.image_size - image.height()) as f64 / 2.0);
                        tri.2.y = image.height() as f64 - tri.2.y + ((args.image_size - image.height()) as f64 / 2.0);
                        // println!("{x} {y}");
                        res.push_str(&format!("draw triangle {} {} {} {} {} {}\n", tri.0.x, tri.0.y, tri.1.x, tri.1.y, tri.2.x, tri.2.y));
                        count += 1;
                        icount += 1;
                        // println!("{count}");
                        if count >= 990 {
                            res.push_str("drawflush display1\n");
                            OpenOptions::new().write(true).create(true).truncate(true).open(format!("{path}{fcount}.mlog")).unwrap().write_all(&res.as_bytes()).unwrap();
                            res.clear();
                            res.push_str(&format!(
                                "draw color {r} {g} {b} 0 0 0\n",
                                r = color.0[0],
                                g = color.0[1],
                                b = color.0[2],
                            ));
                            count = 1;
                            fcount += 1;
                        }
                        if icount > 250 {
                            res.push_str("drawflush display1\n");
                            res.push_str(&format!(
                                "draw color {r} {g} {b} 0 0 0\n",
                                r = color.0[0],
                                g = color.0[1],
                                b = color.0[2],
                            ));
                            icount = 0;
                        }
                    }
                }
                res.push_str("drawflush display1\n");
                OpenOptions::new().write(true).create(true).truncate(true).open(format!("{path}{fcount}.mlog")).unwrap().write_all(&res.as_bytes()).unwrap();
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
            let verts = (tri, *tris.get_vert(rx1, ry1), *tris.get_vert(rx2, ry2));
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
        let rgb = dyn_img.to_rgb8();
        Ok::<_, image::ImageError>(rgb)
    })();

    match (gif_decoder, image_decoder) {
        // (Ok(..), Ok(..)) => unreachable!("Input cannot be an image and a gif!"),
        // the image crate can load the first image of a gif, so it will return Ok
        // we are not using that functionality here because eventually i may try and make this decode/encode gifs (the full thing not just 1st frame)
        (Ok(mut gif_decoder), Err(..)) | (Ok(mut gif_decoder), Ok(..)) => {
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

pub fn scale_image(unscaled: RgbImage, args: &Args) -> (u32, u32, RgbImage, RgbImage) {
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
