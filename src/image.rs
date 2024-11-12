use std::{
    fmt::Debug,
    ops::{Index, IndexMut},
};

pub const DEFAULT_ALPHA_VALUE: u8 = 0;

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

#[repr(C)]
#[derive(Clone, Copy, Eq)]
pub union Pixel {
    pub rgba: Rgba,
    pub color: u32,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Image {
    data: Box<[Pixel]>,

    width: usize,
    height: usize,
}

impl Image {
    pub fn new(width: usize, height: usize, data: impl Into<Box<[Pixel]>>) -> Self {
        let data = data.into();

        assert!(
            width
                .checked_mul(height)
                .expect("Image::new() width * height overflowed")
                == data.len()
        );

        Self {
            width,
            height,
            data,
        }
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }
}

impl Index<usize> for Image {
    type Output = Pixel;

    fn index(&self, index: usize) -> &Self::Output {
        &self.data[index]
    }
}

impl IndexMut<usize> for Image {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.data[index]
    }
}

impl Pixel {
    pub fn color(&self) -> u32 {
        unsafe { self.color }
    }

    pub fn rgba(&self) -> Rgba {
        unsafe { self.rgba }
    }

    pub fn color_mut(&mut self) -> &mut u32 {
        unsafe { &mut self.color }
    }

    pub fn rgba_mut(&mut self) -> &mut Rgba {
        unsafe { &mut self.rgba }
    }
}

impl PartialEq for Pixel {
    fn eq(&self, other: &Self) -> bool {
        self.color() == other.color()
    }
}

impl Debug for Pixel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Pixel {{ color: {} }}", self.color())
    }
}

impl From<u32> for Pixel {
    fn from(color: u32) -> Self {
        Self { color }
    }
}

impl From<Rgba> for Pixel {
    fn from(rgba: Rgba) -> Self {
        Self { rgba }
    }
}
