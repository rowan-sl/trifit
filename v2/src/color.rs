use image::Rgb;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ColorChannel {
    R,
    G,
    B,
}
pub use ColorChannel::*;

pub trait ColorIndex {
    type Component;
    fn channel(self, channel: ColorChannel) -> Self::Component;
}

impl ColorIndex for Rgb<u8> {
    type Component = u8;
    fn channel(self, channel: ColorChannel) -> Self::Component {
        self.0[match channel {
            R => 0,
            G => 1,
            B => 2,
        }]
    }
}
