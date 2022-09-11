use lazysort::SortedBy;

use super::{ScoreInfo, get_color_in_triangle, average};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Score(f64 /* smaller is better */);

impl super::Score for Score {
    fn score_for<'a>(info: super::ScoreInfo<'a>) -> Self {
        let ScoreInfo { triangle, image, tri_size } = info;
        let colors = get_color_in_triangle(image, triangle);

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

        let avg = average(&colors);
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
            .nth(std::cmp::min(
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
        let ret = base + size_score;
        //     let r = deviations.iter().sum::<f64>() / deviations.len() as f64;
        //     if r.is_nan() {
        //         0.0
        //     } else {
        //         r
        //     }

        Score(ret)
    }

    fn average(scores: &[Self]) -> Self {
        Score(if !scores.is_empty() { scores.iter().copied().map(|Score(score)| score).sum::<f64>() / scores.len() as f64 } else { 0.0 })
    }

    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        other.0.partial_cmp(&self.0).unwrap()
    }

    /// 0..100, 0=worst, 100=best
    fn score_value(&self) -> f64 {
        100.0 - (self.0 / 2.5).clamp(0.0, 100.0) // may be wrong
    }
}
