use opengl_graphics::GlGraphics;

use crate::colors::Color;
use crate::vec2::F64x2;

#[derive(Debug, Clone, PartialEq)]
pub struct Triangles {
    vbuf: Vec<Vec<F64x2>>,
    scale_size: (u32, u32), // MAY NOT CORRISPOND TO vbuf sizes!
    real_size: (u32, u32),
    size_of_chunk: f64,
}

impl Triangles {
    pub fn new(width: u32, height: u32, size: f64) -> Self {
        let (scale_width, scale_height, buffer) = generate_regular_points(width, height, size);
        Self {
            vbuf: buffer,
            scale_size: (scale_width, scale_height),
            real_size: (width, height),
            size_of_chunk: size,
        }
    }

    pub fn triangles_around_point(&self, x: u32, y: u32) -> Vec<Triangle> {
        self.triangle_locations_around_point(x, y)
            .into_iter()
            .map(|p| {
                Triangle(
                    *self.get_vert(p[0].0, p[0].1),
                    *self.get_vert(p[1].0, p[1].1),
                    *self.get_vert(p[2].0, p[2].1),
                )
            })
            .collect()
    }

    pub fn triangle_locations_around_point(&self, x: u32, y: u32) -> Vec<[(u32, u32); 3]> {
        use RelVertPos::*;

        let get_if_exists = |o1, o2| {
            let a = self.pos_rel(x, y, o1)?;
            let b = self.pos_rel(x, y, o2)?;
            Some((a, b))
        };
        let perms = [
            (UpLeft, UpRight),
            (UpRight, Right),
            (Right, DownRight),
            (DownRight, DownLeft),
            (DownLeft, Left),
            (Left, UpLeft),
        ];
        perms
            .into_iter()
            .map(|p| get_if_exists(p.0, p.1))
            .filter(|o| o.is_some())
            .map(|o| o.unwrap())
            .map(|(b, c)| [(x, y), b, c])
            .collect()
    }

    pub fn pos_rel(&self, x: u32, y: u32, pos: RelVertPos) -> Option<(u32, u32)> {
        use RelVertPos::*;
        let y = match pos {
            UpLeft | UpRight => y.checked_sub(1)?,
            DownLeft | DownRight => y + 1,
            Left | Right => y,
        };
        let x = match pos {
            Left => x.checked_sub(1)?,
            Right => x + 1,
            UpLeft | DownLeft => x.checked_sub(if y % 2 == 1 { 1 } else { 0 })?,
            UpRight | DownRight => x + if y % 2 == 0 { 1 } else { 0 },
        };
        self.try_get_vert(x, y)?;
        Some((x, y))
    }

    pub fn vert_is_edge(&self, x: u32, y: u32) -> bool {
        let o = if y % 2 == 1 { 0 } else { 1 };
        x == 0 || x >= self.scale_size.0 + o || y == 0 || y >= self.scale_size.1
    }

    /// x and y are in SCALE units
    pub fn get_vert(&self, x: u32, y: u32) -> &F64x2 {
        &self.vbuf[y as usize][x as usize]
    }

    /// x and y are in SCALE units
    pub fn try_get_vert(&self, x: u32, y: u32) -> Option<&F64x2> {
        Some(self.vbuf.get(y as usize)?.get(x as usize)?)
    }

    /// x and y are in SCALE units
    pub fn get_vert_mut(&mut self, x: u32, y: u32) -> &mut F64x2 {
        &mut self.vbuf[y as usize][x as usize]
    }

    /// x and y are in SCALE units
    pub fn try_get_vert_mut(&mut self, x: u32, y: u32) -> Option<&mut F64x2> {
        Some(self.vbuf.get_mut(y as usize)?.get_mut(x as usize)?)
    }

    pub fn into_iter_verts(self) -> impl Iterator<Item = (u32, u32, F64x2)> {
        let mut tmp = vec![];
        for (scale_y, row) in self.vbuf.into_iter().enumerate() {
            for (scale_x, vert) in row.into_iter().enumerate() {
                tmp.push((scale_x as u32, scale_y as u32, vert));
            }
        }
        tmp.into_iter()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RelVertPos {
    UpRight,
    Right,
    DownRight,
    DownLeft,
    Left,
    UpLeft,
}

/// Note: some points will end up `(size/2.0)` away from the size set (in x axis)
pub fn generate_regular_points(width: u32, height: u32, size: f64) -> (u32, u32, Vec<Vec<F64x2>>) {
    assert!(size.is_sign_positive());
    assert!(size.is_normal());
    let real_scale_width = (width as f64 / size) as u32;
    let scale_width = real_scale_width + 1;
    let scale_height = (height as f64 / size) as u32;
    // start at (0, 0)
    // x -> width
    // y -> height
    let mut x: u32 = 0;
    let mut y: u32 = 0;
    let mut buf: Vec<Vec<F64x2>> = vec![];
    let mut current_row: Vec<F64x2> = vec![];
    loop {
        let offset: f64 = if y % 2 == 1 {
            if x > real_scale_width {
                x = 0;
                y += 1;
                buf.push(current_row);
                current_row = vec![];
                continue;
            }
            0.0
        } else {
            -size / 2.0
        };

        let vertex = F64x2::new((x as f64 * size) + offset, y as f64 * size);
        // println!("{vertex:?}");
        current_row.push(vertex);

        x += 1;
        if x > scale_width {
            x = 0;
            y += 1;
            buf.push(current_row);
            current_row = vec![];
        }
        if y > scale_height {
            buf.push(current_row);
            break;
        }
    }
    (real_scale_width, scale_height, buf)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Triangle(pub F64x2, pub F64x2, pub F64x2);

impl Triangle {
    pub fn offset(mut self, x: f64, y: f64) -> Self {
        self.0.x += x;
        self.1.x += x;
        self.2.x += x;
        self.0.y += y;
        self.1.y += y;
        self.2.y += y;
        self
    }

    pub fn draw_outline(
        &self,
        thickness: f64,
        color: Color,
        c: &graphics::Context,
        gl: &mut GlGraphics,
    ) {
        use graphics::line;
        line(
            color,
            thickness,
            [self.0.x, self.0.y, self.1.x, self.1.y],
            c.transform,
            gl,
        );
        line(
            color,
            thickness,
            [self.1.x, self.1.y, self.2.x, self.2.y],
            c.transform,
            gl,
        );
        line(
            color,
            thickness,
            [self.2.x, self.2.y, self.0.x, self.0.y],
            c.transform,
            gl,
        );

        // draw_triangle_lines(
        //     self.0.into(),
        //     self.1.into(),
        //     self.2.into(),
        //     thickness,
        //     color,
        // )
    }

    pub fn draw(&self, color: Color, c: &graphics::Context, gl: &mut GlGraphics) {
        use graphics::{DrawState, Polygon};
        Polygon::new(color).draw(
            &[
                [self.0.x, self.0.y],
                [self.1.x, self.1.y],
                [self.2.x, self.2.y],
            ][..],
            &DrawState::default(),
            c.transform,
            gl,
        );
        // draw_triangle(
        //     self.0.into(),
        //     self.1.into(),
        //     self.2.into(),
        //     color,
        // )
    }
}
