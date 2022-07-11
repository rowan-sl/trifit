use image::GenericImageView;

use crate::*;

pub fn rectangle_by_points(c0: F64x2, c1: F64x2) -> [f64; 4] {
    graphics::rectangle::rectangle_by_corners(c0.x, c0.y, c1.x, c1.y)
}

pub fn get_color_in_triangle(image: &RgbImage, triangle: Triangle) -> Vec<Rgb<u8>> {
    fn sign(p1: F64x2, p2: F64x2, p3: F64x2) -> f64 {
        return (p1.x - p3.x) * (p2.y - p3.y) - (p2.x - p3.x) * (p1.y - p3.y);
    }

    fn point_in_triangle(pt: F64x2, v1: F64x2, v2: F64x2, v3: F64x2) -> bool {
        let d1 = sign(pt, v1, v2);
        let d2 = sign(pt, v2, v3);
        let d3 = sign(pt, v3, v1);

        let has_neg = (d1 < 0.0) || (d2 < 0.0) || (d3 < 0.0);
        let has_pos = (d1 > 0.0) || (d2 > 0.0) || (d3 > 0.0);

        !(has_neg && has_pos)
    }

    fn min(a: f64, b: f64) -> f64 {
        if a < b {
            a
        } else {
            b
        }
    }

    fn max(a: f64, b: f64) -> f64 {
        if a > b {
            a
        } else {
            b
        }
    }

    let minx = min(min(triangle.0.x, triangle.1.x), triangle.2.x)
        .floor()
        .clamp(0.0, image.width() as f64) as u32;
    let maxx = max(max(triangle.0.x, triangle.1.x), triangle.2.x)
        .ceil()
        .clamp(0.0, image.width() as f64) as u32;
    let miny = min(min(triangle.0.y, triangle.1.y), triangle.2.y)
        .floor()
        .clamp(0.0, image.height() as f64) as u32;
    let maxy = max(max(triangle.0.y, triangle.1.y), triangle.2.y)
        .ceil()
        .clamp(0.0, image.height() as f64) as u32;
    let width = maxx - minx;
    let height = maxy - miny;
    // println!("img: {}x{}", image.width(), image.height());
    // println!("    sub image coords: ({}, {})", minx, miny);
    // println!("    sub image demensions: {}x{}", width, height);

    // for (x, y, pxl) in image.view(minx, miny, width, height).enumerate_pixels()
    image
        .view(minx, miny, width, height)
        .to_image()
        .enumerate_pixels()
        .map(|(x, y, px)| (x + minx, y + miny, px))
        .map(|(x, y, pxl)| {
            if point_in_triangle(
                F64x2::new(x as f64, y as f64),
                triangle.0,
                triangle.1,
                triangle.2,
            ) {
                Some(*pxl)
            } else {
                None
            }
        })
        .filter(|o| o.is_some())
        .map(|o| o.unwrap())
        .collect()
}

pub fn average(colors: &Vec<Rgb<u8>>) -> Rgb<u8> {
    let sum = colors.iter().fold(Rgb([0u128; 3]), |acc, x| {
        Rgb([
            acc[0] + x[0] as u128,
            acc[1] + x[1] as u128,
            acc[2] + x[2] as u128,
        ])
    });
    Rgb([
        u8::try_from(sum.0[0].checked_div(colors.len() as u128).unwrap_or(0)).unwrap(),
        u8::try_from(sum.0[1].checked_div(colors.len() as u128).unwrap_or(0)).unwrap(),
        u8::try_from(sum.0[2].checked_div(colors.len() as u128).unwrap_or(0)).unwrap(),
    ])
}

pub fn score(colors: &Vec<Rgb<u8>>) -> f64 {
    let avg = average(colors);
    let deviations: Vec<f64> = colors
        .iter()
        .map(|c| {
            ((avg.0[0] as f64 - c.0[0] as f64).abs()
                + (avg.0[1] as f64 - c.0[1] as f64).abs()
                + (avg.0[2] as f64 - c.0[2] as f64).abs())
                / 3.0
        })
        .collect();
    let r = deviations.iter().sum::<f64>() / deviations.len() as f64;
    if r.is_nan() {
        0.0
    } else {
        r
    }
}

pub fn score_for_group(image: &RgbImage, group: &Vec<Triangle>) -> f64 {
    let scores: Vec<f64> = group
        .iter()
        .map(|t| score(&get_color_in_triangle(image, *t)))
        .collect();
    // println!("scores: {scores:?}");
    let r = scores.iter().sum::<f64>() / scores.len() as f64;
    if r.is_nan() {
        // println!("nan");
        0.0
    } else {
        r
    }
}
