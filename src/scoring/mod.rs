use std::cmp::{self, Ordering};

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

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Score {
    /// has no info (eg: a triangle with no pixels covered)
    is_none: bool,
    /// distance between colors, 0-100 (0-perfect match, 100=perfectly disarrayed (impossible?))
    average_color_distance: f64,
    /// range of 0-100 (0=line, 100=square)
    squareness: f64,
}

impl Score {
    pub fn average(scores: &[Self]) -> Self {
        if scores.is_empty() {
            return Self::none();
        }
        Self {
            is_none: false,
            average_color_distance: scores
                .iter()
                .filter(|score| !score.is_none)
                .map(|score| score.average_color_distance)
                .sum::<f64>()
                / scores.len() as f64,
            squareness: scores.iter()
                .filter(|score| !score.is_none)
                .map(|score| score.squareness)
                .sum::<f64>()
                / scores.len() as f64,
        }
    }

    pub fn none() -> Self {
        Self { is_none: true, average_color_distance: 0.0, squareness: 0.0 }
    }

    /// 0..100, 0=worst, 100=best
    pub fn score_value(&self) -> f64 {
        if self.is_none {
            return 0.0;
        }
        /*
        idea here is to mostly not care about squareness untill it passes some threshold (~0.5),
         and then start to care a LOT after that
        */
        // old version, much more simple (also more agressive at the start / lower at the end)
        // let weighted_squareness = 1.0 - (3.0 * ((self.squareness / 100.0) - 1.0)).exp(); // 1-e^{3(x-1)}

        // new version, more complex (does not get smaller as early, and does not end at zero)
        //
        // convert so 100=line, and 0=square (so that for good values (smaller) it gives more controll to the color part)
        // then scales into the 0.1 range
        let s = (100.0 - self.squareness) / 100.0;
        let weighted_squareness = (1.528
            * ((1.0 - (3.0 * (0.9 * s - 1.0)).exp())
                * (0.5 + (1.0 / (1.0 + (-s + 0.5).exp())) / 2.0)))
            .min(1.0);
        /*
        the two values (squareness and color distance) are combined by
        multiplication, as it makes it scale nicely. color_dist is not scaled down to 0..1,
        as to improve accuracy

        also converts from 100=worst, 0=best to 100=best, 0=worst for the color dist
        */
        (100.0 - self.average_color_distance) * weighted_squareness
    }

    pub fn cmp(&self, rhs: &Score) -> Ordering {
        self.score_value().partial_cmp(&rhs.score_value()).unwrap()
    }
}

pub fn score(
    triangle: Triangle,
    colors: &Vec<Rgb<u8>>,
    image: &RgbImage,
    tri_size: f64,
    scheme: ScoringScheme,
) -> Score {
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
            let _ret = base + size_score;
            //     let r = deviations.iter().sum::<f64>() / deviations.len() as f64;
            //     if r.is_nan() {
            //         0.0
            //     } else {
            //         r
            //     }
            eprintln!("This mode is temporarily broken, use avg-with-shape-weight instead!");
            todo!()
        }
        ScoringScheme::AvgWithShapeWeight => {
            if colors.is_empty() {
                return Score::none();
            }

            let minx = min(min(triangle.0.x, triangle.1.x), triangle.2.x);
            let maxx = max(max(triangle.0.x, triangle.1.x), triangle.2.x);
            let miny = min(min(triangle.0.y, triangle.1.y), triangle.2.y);
            let maxy = max(max(triangle.0.y, triangle.1.y), triangle.2.y);
            let width = maxx - minx;
            let height = maxy - miny;

            fn color_dist(a: Rgb<u8>, b: Rgb<u8>) -> f64 {
                let r = (a.0[0] - b.0[0]) as f64;
                let g = (a.0[1] - b.0[1]) as f64;
                let b = (a.0[2] - b.0[2]) as f64;
                (r.powi(2) + g.powi(2) + b.powi(2)).sqrt()
            }

            let avg = average(colors);
            let average_color_distance = colors
                .iter()
                .map(|color| color_dist(*color, avg))
                .sum::<f64>()
                / colors.len() as f64
                / 25.5; /* scale into range of 0..100 */
            assert!(0.0 <= average_color_distance && average_color_distance <= 100.0);

            let squareness = {
                let m = max(width, height);
                2.0 * ((((100.0 * (width + height)) / m) / 2.0) - 50.0)
            };
            assert!(0.0 <= squareness && squareness <= 100.0);

            Score {
                is_none: false,
                average_color_distance,
                squareness,
            }
        }
    }
}

pub fn score_for_group(
    image: &RgbImage,
    group: &Vec<Triangle>,
    tri_size: f64,
    scheme: ScoringScheme,
) -> Score {
    let scores = group.iter().map(|t| {
        score(
            *t,
            &get_color_in_triangle(image, *t),
            image,
            tri_size,
            scheme,
        )
    });
    Score::average(&scores.collect::<Vec<_>>())
}
