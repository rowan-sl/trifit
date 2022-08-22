use std::ops::{
    Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Rem, RemAssign, Sub, SubAssign,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct F64x2 {
    pub x: f64,
    pub y: f64,
}

impl F64x2 {
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    pub const fn splat(x: f64) -> Self {
        Self::new(x, x)
    }

    pub const fn zero() -> Self {
        F64x2::new(0.0, 0.0)
    }

    /// cross product of two vectors
    pub fn cross_2v(self, other: Self) -> f64 {
        self.x * other.y - self.y * other.x
    }

    /// cross product of a vector and a scalar
    pub fn cross_vs(self, s: f64) -> Self {
        Self {
            x: s * self.y,
            y: -s * self.x,
        }
    }

    /// sign inverted cross product of a vector and a scalar
    pub fn cross_vs_inverted(self, s: f64) -> Self {
        -self.cross_vs(s)
    }
}

impl Add for F64x2 {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self {
            x: self.x + other.x,
            y: self.y + other.y,
        }
    }
}

impl AddAssign for F64x2 {
    fn add_assign(&mut self, other: Self) {
        self.x += other.x;
        self.y += other.y;
    }
}

impl Sub for F64x2 {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        Self {
            x: self.x - other.x,
            y: self.y - other.y,
        }
    }
}

impl SubAssign for F64x2 {
    fn sub_assign(&mut self, other: Self) {
        self.x -= other.x;
        self.y -= other.y;
    }
}

impl Div for F64x2 {
    type Output = Self;

    fn div(self, other: Self) -> Self {
        Self {
            x: self.x / other.x,
            y: self.y / other.y,
        }
    }
}

impl DivAssign for F64x2 {
    fn div_assign(&mut self, other: Self) {
        self.x /= other.x;
        self.y /= other.y;
    }
}

impl Rem for F64x2 {
    type Output = Self;

    fn rem(self, other: Self) -> Self {
        Self {
            x: self.x % other.x,
            y: self.y % other.y,
        }
    }
}

impl RemAssign for F64x2 {
    fn rem_assign(&mut self, other: Self) {
        self.x %= other.x;
        self.y %= other.y;
    }
}

impl Mul for F64x2 {
    type Output = Self;

    fn mul(self, other: Self) -> Self {
        Self {
            x: self.x * other.x,
            y: self.y * other.y,
        }
    }
}

impl MulAssign for F64x2 {
    fn mul_assign(&mut self, other: Self) {
        self.x *= other.x;
        self.y *= other.y;
    }
}

impl Neg for F64x2 {
    type Output = Self;

    fn neg(self) -> Self {
        Self {
            x: -self.x,
            y: -self.y,
        }
    }
}

impl Add<f64> for F64x2 {
    type Output = Self;

    fn add(self, other: f64) -> Self {
        Self {
            x: self.x + other,
            y: self.y + other,
        }
    }
}

impl AddAssign<f64> for F64x2 {
    fn add_assign(&mut self, other: f64) {
        self.x += other;
        self.y += other;
    }
}

impl Sub<f64> for F64x2 {
    type Output = Self;

    fn sub(self, other: f64) -> Self {
        Self {
            x: self.x - other,
            y: self.y - other,
        }
    }
}

impl SubAssign<f64> for F64x2 {
    fn sub_assign(&mut self, other: f64) {
        self.x -= other;
        self.y -= other;
    }
}

impl Div<f64> for F64x2 {
    type Output = Self;

    fn div(self, other: f64) -> Self {
        Self {
            x: self.x / other,
            y: self.y / other,
        }
    }
}

impl DivAssign<f64> for F64x2 {
    fn div_assign(&mut self, other: f64) {
        self.x /= other;
        self.y /= other;
    }
}

impl Rem<f64> for F64x2 {
    type Output = Self;

    fn rem(self, other: f64) -> Self {
        Self {
            x: self.x % other,
            y: self.y % other,
        }
    }
}

impl RemAssign<f64> for F64x2 {
    fn rem_assign(&mut self, other: f64) {
        self.x %= other;
        self.y %= other;
    }
}

impl Mul<f64> for F64x2 {
    type Output = Self;

    fn mul(self, other: f64) -> Self {
        Self {
            x: self.x * other,
            y: self.y * other,
        }
    }
}

impl MulAssign<f64> for F64x2 {
    fn mul_assign(&mut self, other: f64) {
        self.x *= other;
        self.y *= other;
    }
}

impl From<[f64; 2]> for F64x2 {
    fn from(val: [f64; 2]) -> Self {
        Self {
            x: val[0],
            y: val[1],
        }
    }
}
