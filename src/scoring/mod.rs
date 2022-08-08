use std::cmp;

use image::{Rgb, RgbImage};
use lazysort::SortedBy;

use crate::{triangle::Triangle, vec2::F64x2, ScoringScheme};

#[inline(always)]
fn min(a: f64, b: f64) -> f64 {
    if a < b {
        a
    } else {
        b
    }
}

#[inline(always)]
fn max(a: f64, b: f64) -> f64 {
    if a > b {
        a
    } else {
        b
    }
}

pub fn rectangle_by_points(c0: F64x2, c1: F64x2) -> [f64; 4] {
    graphics::rectangle::rectangle_by_corners(c0.x, c0.y, c1.x, c1.y)
}

pub fn point_in_triangle(pt: F64x2, v1: F64x2, v2: F64x2, v3: F64x2) -> bool {
    #[inline(always)]
    fn sign(p1: F64x2, p2: F64x2, p3: F64x2) -> f64 {
        (p1.x - p3.x) * (p2.y - p3.y) - (p2.x - p3.x) * (p1.y - p3.y)
    }

    let d1 = sign(pt, v1, v2);
    let d2 = sign(pt, v2, v3);
    let d3 = sign(pt, v3, v1);

    let has_neg = (d1 < 0.0) || (d2 < 0.0) || (d3 < 0.0);
    let has_pos = (d1 > 0.0) || (d2 > 0.0) || (d3 > 0.0);

    !(has_neg && has_pos)
}

pub fn get_color_in_triangle(image: &RgbImage, triangle: Triangle) -> Vec<Rgb<u8>> {
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
    SubImageIterator::new(image, minx, miny, width, height)
        .map(|(x, y, pxl)| {
            if point_in_triangle(
                F64x2::new((x + minx) as f64, (y + miny) as f64),
                triangle.0,
                triangle.1,
                triangle.2,
            ) {
                Some(pxl)
            } else {
                None
            }
        })
        .filter(Option::is_some)
        .map(Option::unwrap)
        .collect()
}

pub struct SubImageIterator<'a> {
    image: &'a RgbImage,
    xy: (u32, u32),
    wh: (u32, u32),
    current_xy: (u32, u32),
    done: bool,
}

impl<'a> SubImageIterator<'a> {
    pub fn new(img: &'a RgbImage, x: u32, y: u32, width: u32, height: u32) -> Self {
        assert!(x + width <= img.width());
        assert!(y + height <= img.height());
        let done = width == 0 || height == 0;
        SubImageIterator {
            image: img,
            xy: (x, y),
            wh: (width, height),
            current_xy: (0, 0),
            done,
        }
    }

    fn get(&self, x: u32, y: u32) -> Rgb<u8> {
        *self.image.get_pixel(x + self.xy.0, y + self.xy.1)
    }
}

impl<'a> Iterator for SubImageIterator<'a> {
    type Item = (u32, u32, Rgb<u8>);
    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            None
        } else {
            let v = self.get(self.current_xy.0, self.current_xy.1);
            self.current_xy.0 += 1;
            if self.current_xy.0 >= self.wh.0 {
                self.current_xy.0 = 0;
                self.current_xy.1 += 1;
                if self.current_xy.1 >= self.wh.1 {
                    self.done = true;
                }
            }
            Some((self.current_xy.0, self.current_xy.1, v))
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        if let Ok(v) = usize::try_from(self.wh.0 * self.wh.1) {
            (v, Some(v))
        } else {
            (0, None)
        }
    }
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

pub fn score(
    _triangle: Triangle,
    colors: &Vec<Rgb<u8>>,
    image: &RgbImage,
    tri_size: f64,
    scheme: ScoringScheme,
) -> f64 {
    match scheme {
        ScoringScheme::PercentileWithSizeWeight => {
            let w = image.width() + (tri_size - image.width() as f64 % tri_size.ceil()) as u32;
            let h = image.height() + (tri_size - image.height() as f64 % tri_size.ceil()) as u32;
            let appt = (image.width() * image.height())
                / ((w as f64 / tri_size) * (h as f64 / tri_size)) as u32;

            fn max(a: f64, b: f64) -> f64 {
                if a > b {
                    a
                } else {
                    b
                }
            }

            let avg = average(colors);
            let base = colors
                .iter()
                .map(|c| {
                    max(
                        max(
                            (avg.0[0] as f64 - c.0[0] as f64).abs(),
                            (avg.0[1] as f64 - c.0[1] as f64).abs(),
                        ),
                        (avg.0[2] as f64 - c.0[2] as f64).abs(),
                    )
                })
                .sorted_by(|a, b| b.total_cmp(a))
                .nth(cmp::min(
                    appt as usize / 20, // 5%
                    colors.len().saturating_sub(1),
                ))
                .unwrap_or(0.0);
            // let size_score = (deviations.len() - std::cmp::min(appt as usize / 20 /* 5% */, deviations.len())) as f64 * 1.0 /* weight value */;
            // let size_score = if (appt as f64 * 0.03) as usize > deviations.len() {
            //     (appt as f64 * 0.03) as usize - deviations.len()
            // } else { 0 } as f64;
            // let size_score = 1.0 / (((cmp::min(deviations.len() as u32 + 1, (appt as f64 / 1.0) as u32) as f64 * 10.0)
            // / (appt as f64 / 1.0)) * 2.0);
            let size_score = ((appt as f64 / (colors.len() as f64 + 1.0)) / appt as f64) * 255.0;
            // println!("{}", size_score);
            if base + size_score > 255.0 * 3.0 {
                println!("{base} {size_score} {} {}", colors.len(), appt);
            }
            base + size_score
            //     let r = deviations.iter().sum::<f64>() / deviations.len() as f64;
            //     if r.is_nan() {
            //         0.0
            //     } else {
            //         r
            //     }
        }
        ScoringScheme::ColorspaceOptimized => {
            todo!()
        }
    }
}

pub fn score_for_group(
    image: &RgbImage,
    group: &Vec<Triangle>,
    tri_size: f64,
    scheme: ScoringScheme,
) -> f64 {
    let scores = group.iter().map(|t| {
        score(
            *t,
            &get_color_in_triangle(image, *t),
            image,
            tri_size,
            scheme,
        )
    });
    // println!("scores: {scores:?}");
    let r = scores.sum::<f64>() / group.len() as f64;
    if r.is_nan() {
        // println!("nan");
        0.0
    } else {
        r
    }
}
