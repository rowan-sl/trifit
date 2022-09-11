use std::cmp::Ordering;

use image::Rgb;

use super::{ScoreInfo, get_color_in_triangle, min, max, average};

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
    pub fn none() -> Self {
        Self { is_none: true, average_color_distance: 0.0, squareness: 0.0 }
    }
}

impl super::Score for Score {
    fn score_for<'a>(info: super::ScoreInfo<'a>) -> Self {
        let ScoreInfo { triangle, image, tri_size: _ } = info;
        let colors = get_color_in_triangle(image, triangle);

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
            let r = a.0[0] as f64 - b.0[0] as f64;
            let g = a.0[1] as f64 - b.0[1] as f64;
            let b = a.0[2] as f64 - b.0[2] as f64;
            (r.powi(2) + g.powi(2) + b.powi(2)).sqrt()
        }

        let avg = average(&colors);
        let average_color_distance = colors
            .iter()
            .map(|color| color_dist(*color, avg))
            .sum::<f64>()
            / colors.len() as f64
            / 25.5; /* scale into range of 0..100 */
        if !(0.0 <= average_color_distance && average_color_distance <= 100.0) {
            warn!("average color distance out of range! (value: {average_color_distance})");
        }

        let squareness = {
            let m = max(width, height);
            2.0 * ((((100.0 * (width + height)) / m) / 2.0) - 50.0)
        };
        if !(0.0 <= squareness && squareness <= 100.0) {
            warn!("squareness out of range! (value: {squareness})");
        }

        Score {
            is_none: false,
            average_color_distance,
            squareness,
        }
    }

    fn average(scores: &[Self]) -> Self {
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

    fn cmp(&self, rhs: &Score) -> Ordering {
        self.score_value().partial_cmp(&rhs.score_value()).unwrap()
    }

    /// 0..100, 0=worst, 100=best
    fn score_value(&self) -> f64 {
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
}