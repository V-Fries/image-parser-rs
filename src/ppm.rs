use core::str;
use std::{
    collections::TryReserveError, error::Error, fmt::Display, fs::File, io::Read,
    num::ParseIntError, str::Utf8Error,
};

use crate::{
    image::{Pixel, Rgba, DEFAULT_ALPHA_VALUE},
    Image,
};

pub struct PpmFilePath<'a>(pub &'a str);

#[derive(Debug)]
pub enum ImagesFromPpmFileError {
    FailedToOpenFile(std::io::Error),
    FailedToReadFile(std::io::Error),

    FormatNotFound,
    NoWhitespaceAfterFormat,
    FormatNotSupported,

    WidthNotFound,
    NoWhitespaceAfterWidth,
    WidthIsNotAUtf8String(Utf8Error),
    WidthIsNotAUsize(ParseIntError),

    HeightNotFound,
    NoWhitespaceAfterHeight,
    HeightIsNotAUtf8String(Utf8Error),
    HeightIsNotAUsize(ParseIntError),

    WidthMulHeightOverflowsUsize,
    SizeMulColorByteCountOverflows,

    MaxvalNotFound,
    NoWhitespaceAfterMaxval,
    MaxvalIsNotAUtf8String(Utf8Error),
    MaxvalIsNotAU16(ParseIntError),
    MaxvalCantBe0,

    FailedToAllocateImageData(TryReserveError),
    LessThanSizePixelsFoundInFile,
}

impl Display for ImagesFromPpmFileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl Error for ImagesFromPpmFileError {}

impl<'a> TryFrom<PpmFilePath<'a>> for Vec<Image> {
    type Error = ImagesFromPpmFileError;

    fn try_from(file_path: PpmFilePath) -> Result<Self, Self::Error> {
        let mut file = File::open(file_path.0).map_err(ImagesFromPpmFileError::FailedToOpenFile)?;
        let mut file_content = Vec::new();
        file.read_to_end(&mut file_content)
            .map_err(ImagesFromPpmFileError::FailedToReadFile)?;

        parse_ppm_file(&file_content)
    }
}

impl<'a> TryFrom<PpmFilePath<'a>> for Image {
    type Error = ImagesFromPpmFileError;

    fn try_from(file_path: PpmFilePath) -> Result<Self, Self::Error> {
        Ok(Vec::try_from(file_path)?.into_iter().next().expect(
            "Vec::<image_parser::Image>::try_from::(PpmFile) Returned an empty vec!
             (This should never happen, there is an error in the lib)",
        ))
    }
}

fn parse_ppm_file(file_content: &[u8]) -> Result<Vec<Image>, ImagesFromPpmFileError> {
    let mut images = Vec::new();

    if file_content.is_empty() {
        return Err(ImagesFromPpmFileError::FormatNotFound);
    }

    let mut cursor = 0;
    while cursor < file_content.len() {
        let (bytes_read, image) = parse_image(&file_content[cursor..])?;

        images.push(image);

        match get_content_start_index(file_content, cursor + bytes_read) {
            Some(index) => cursor = index,
            None => break,
        }
    }

    Ok(images)
}

fn parse_image(file_content: &[u8]) -> Result<(usize, Image), ImagesFromPpmFileError> {
    let mut start =
        get_content_start_index(file_content, 0).ok_or(ImagesFromPpmFileError::FormatNotFound)?;
    let mut end = get_content_end_index(file_content, start)
        .ok_or(ImagesFromPpmFileError::NoWhitespaceAfterFormat)?;
    let format = &file_content[start..end];

    start =
        get_content_start_index(file_content, end).ok_or(ImagesFromPpmFileError::WidthNotFound)?;
    end = get_content_end_index(file_content, start)
        .ok_or(ImagesFromPpmFileError::NoWhitespaceAfterWidth)?;
    let width = str::from_utf8(&file_content[start..end])
        .map_err(ImagesFromPpmFileError::WidthIsNotAUtf8String)?
        .parse::<usize>()
        .map_err(ImagesFromPpmFileError::WidthIsNotAUsize)?;

    start =
        get_content_start_index(file_content, end).ok_or(ImagesFromPpmFileError::HeightNotFound)?;
    end = get_content_end_index(file_content, start)
        .ok_or(ImagesFromPpmFileError::NoWhitespaceAfterHeight)?;
    let height = str::from_utf8(&file_content[start..end])
        .map_err(ImagesFromPpmFileError::HeightIsNotAUtf8String)?
        .parse::<usize>()
        .map_err(ImagesFromPpmFileError::HeightIsNotAUsize)?;

    let size = width
        .checked_mul(height)
        .ok_or(ImagesFromPpmFileError::WidthMulHeightOverflowsUsize)?;

    start =
        get_content_start_index(file_content, end).ok_or(ImagesFromPpmFileError::MaxvalNotFound)?;
    end = find_index(file_content, start, |elem| (elem as char).is_whitespace())
        .ok_or(ImagesFromPpmFileError::NoWhitespaceAfterMaxval)?;
    let maxval = str::from_utf8(&file_content[start..end])
        .map_err(ImagesFromPpmFileError::MaxvalIsNotAUtf8String)?
        .parse::<u16>()
        .map_err(ImagesFromPpmFileError::MaxvalIsNotAU16)?;
    if maxval == 0 {
        return Err(ImagesFromPpmFileError::MaxvalCantBe0);
    }

    start = end + 1;
    let (bytes_read, image) = match format {
        b"P6" => read_image(&file_content[start..], width, height, size, maxval)?,
        _ => return Err(ImagesFromPpmFileError::FormatNotSupported),
    };
    Ok((start + bytes_read, image))
}

fn read_image(
    raw_image_data: &[u8],
    width: usize,
    height: usize,
    size: usize,
    maxval: u16,
) -> Result<(usize, Image), ImagesFromPpmFileError> {
    let mut image_data = Vec::<Pixel>::new();
    image_data
        .try_reserve_exact(size)
        .map_err(ImagesFromPpmFileError::FailedToAllocateImageData)?;

    // TODO consider handling the case of maxval 255
    let bytes_read = if maxval < 256 {
        read_image_from_u8_maxval(raw_image_data, size, maxval as u8, &mut image_data)?
    } else {
        read_image_from_u16_maxval(raw_image_data, size, maxval, &mut image_data)?
    };

    Ok((bytes_read, Image::new(width, height, image_data)))
}

fn read_image_from_u8_maxval(
    raw_image_data: &[u8],
    size: usize,
    maxval: u8,
    image_data: &mut Vec<Pixel>,
) -> Result<usize, ImagesFromPpmFileError> {
    const SIZE_OF_U8_COLOR: usize = 3;
    let limit = size
        .checked_mul(SIZE_OF_U8_COLOR)
        .ok_or(ImagesFromPpmFileError::SizeMulColorByteCountOverflows)?;

    if raw_image_data.len() < limit {
        return Err(ImagesFromPpmFileError::LessThanSizePixelsFoundInFile);
    }

    for i in (2..limit).step_by(SIZE_OF_U8_COLOR) {
        image_data.push(Pixel {
            rgba: Rgba {
                r: convert_u8_maxval_color(raw_image_data[i - 2], maxval),
                g: convert_u8_maxval_color(raw_image_data[i - 1], maxval),
                b: convert_u8_maxval_color(raw_image_data[i], maxval),
                a: DEFAULT_ALPHA_VALUE,
            },
        });
    }

    Ok(limit)
}

fn read_image_from_u16_maxval(
    raw_image_data: &[u8],
    size: usize,
    maxval: u16,
    image_data: &mut Vec<Pixel>,
) -> Result<usize, ImagesFromPpmFileError> {
    const SIZE_OF_U16_COLOR: usize = 6;
    let limit = size
        .checked_mul(SIZE_OF_U16_COLOR)
        .ok_or(ImagesFromPpmFileError::SizeMulColorByteCountOverflows)?;

    if raw_image_data.len() < limit {
        return Err(ImagesFromPpmFileError::LessThanSizePixelsFoundInFile);
    }

    for i in (5..limit).step_by(SIZE_OF_U16_COLOR) {
        let r = raw_image_data[i - 4] as u16 | ((raw_image_data[i - 5] as u16) << 8);
        let g = raw_image_data[i - 2] as u16 | ((raw_image_data[i - 3] as u16) << 8);
        let b = raw_image_data[i] as u16 | ((raw_image_data[i - 1] as u16) << 8);
        image_data.push(Pixel {
            rgba: Rgba {
                r: convert_u16_maxval_color(r, maxval),
                g: convert_u16_maxval_color(g, maxval),
                b: convert_u16_maxval_color(b, maxval),
                a: DEFAULT_ALPHA_VALUE,
            },
        });
    }

    Ok(limit)
}

fn convert_u8_maxval_color(color: u8, maxval: u8) -> u8 {
    ((color as f64) / (maxval as f64) * 255.) as u8
}

fn convert_u16_maxval_color(color: u16, maxval: u16) -> u8 {
    ((color as f64) / (maxval as f64) * 255.) as u8
}

fn get_content_start_index(slice: &[u8], skip: usize) -> Option<usize> {
    let mut skip = find_index(slice, skip, |elem| !(elem as char).is_whitespace())?;
    while slice[skip] == b'#' {
        skip = find_index(slice, skip + 1, |elem| elem == b'\n')?;
        skip = find_index(slice, skip + 1, |elem| !(elem as char).is_whitespace())?;
    }
    Some(skip)
}

fn get_content_end_index(slice: &[u8], skip: usize) -> Option<usize> {
    find_index(slice, skip, |elem| {
        (elem as char).is_whitespace() || elem == b'#'
    })
}

fn find_index(slice: &[u8], skip: usize, mut find_op: impl FnMut(u8) -> bool) -> Option<usize> {
    slice.iter().enumerate().skip(skip).find_map(
        |(i, elem)| {
            if find_op(*elem) {
                Some(i)
            } else {
                None
            }
        },
    )
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn single_image() {
        let data: [Pixel; 4 * 4] = [
            42,
            594,
            4543,
            65478,
            56309043,
            547789421,
            909545472,
            u32::MAX,
            u32::MIN,
            56348903,
            2429,
            589409022,
            986953,
            436557,
            2134646,
            3474632,
        ]
        .map(|e| {
            let mut pixel: Pixel = e.into();
            pixel.rgba_mut().a = DEFAULT_ALPHA_VALUE;
            pixel
        });
        let mut file: Vec<u8> = Vec::new();
        file.extend_from_slice(b"P6 4 4 255 ");
        push_pixel_data(&mut file, &data);
        let expected = Image::new(4, 4, data);
        let res = parse_ppm_file(&file).unwrap();
        assert_eq!(res.len(), 1);
        assert_eq!(expected, res[0])
    }

    #[test]
    fn multiple_images() {
        let data_1: [Pixel; 4 * 3] = [
            42,
            594,
            4543,
            65478,
            56309043,
            547789421,
            909545472,
            u32::MAX,
            u32::MIN,
            56348903,
            2429,
            589409022,
        ]
        .map(|e| {
            let mut pixel: Pixel = e.into();
            pixel.rgba_mut().a = DEFAULT_ALPHA_VALUE;
            pixel
        });

        let data_2: [Pixel; 2 * 3] = [43, 346, 647642, 436887, 90863643, 437437474].map(|e| {
            let mut pixel: Pixel = e.into();
            pixel.rgba_mut().a = DEFAULT_ALPHA_VALUE;
            pixel
        });

        let mut file: Vec<u8> = Vec::new();
        file.extend_from_slice(b"P6   4 \n\n 3   255 ");
        push_pixel_data(&mut file, &data_1);
        file.extend_from_slice(b"P6\t2 #test\n3\n# Hey\n255 ");
        push_pixel_data(&mut file, &data_2);

        let expected = [Image::new(4, 3, data_1), Image::new(2, 3, data_2)];
        let res = parse_ppm_file(&file).unwrap();
        assert_eq!(res.len(), expected.len());
        assert_eq!(expected[0], res[0]);
        assert_eq!(expected[1], res[1]);
    }

    #[test]
    fn empty_file() {
        let res = parse_ppm_file(b"").unwrap_err();
        match res {
            ImagesFromPpmFileError::FormatNotFound => {}
            _ => panic!("Expected ImageFromPpmFileError::FormatNotFound found {res}"),
        };

        let res = parse_ppm_file(b"                    ").unwrap_err();
        match res {
            ImagesFromPpmFileError::FormatNotFound => {}
            _ => panic!("Expected ImageFromPpmFileError::FormatNotFound found {res}"),
        };
    }

    #[test]
    fn bad_format() {
        let res = parse_ppm_file(b"").unwrap_err();
        match res {
            ImagesFromPpmFileError::FormatNotFound => {}
            _ => panic!("Expected ImageFromPpmFileError::FormatNotFound found {res}"),
        };

        let res = parse_ppm_file(b"htre4 4 5 4654 ").unwrap_err();
        match res {
            ImagesFromPpmFileError::FormatNotSupported => {}
            _ => panic!("Expected ImageFromPpmFileError::FormatNotSupported found {res}"),
        };

        let res = parse_ppm_file(b"htre4").unwrap_err();
        match res {
            ImagesFromPpmFileError::NoWhitespaceAfterFormat => {}
            _ => panic!("Expected ImageFromPpmFileError::NoWhitespaceAfterFormat found {res}"),
        };
    }

    #[test]
    fn bad_width() {
        let res = parse_ppm_file(b"P6 4f3 5 255 ").unwrap_err();
        match res {
            ImagesFromPpmFileError::WidthIsNotAUsize(_) => {}
            _ => panic!("Expected ImageFromPpmFileError::WidthIsNotAUsize found {res}"),
        };

        let res = parse_ppm_file(b"P6 f 5 255 ").unwrap_err();
        match res {
            ImagesFromPpmFileError::WidthIsNotAUsize(_) => {}
            _ => panic!("Expected ImageFromPpmFileError::WidthIsNotAUsize found {res}"),
        };

        let res = parse_ppm_file(b"P6 42f 5 255 ").unwrap_err();
        match res {
            ImagesFromPpmFileError::WidthIsNotAUsize(_) => {}
            _ => panic!("Expected ImageFromPpmFileError::WidthIsNotAUsize found {res}"),
        };

        let res = parse_ppm_file(b"P6 -42 5 255 ").unwrap_err();
        match res {
            ImagesFromPpmFileError::WidthIsNotAUsize(_) => {}
            _ => panic!("Expected ImageFromPpmFileError::WidthIsNotAUsize found {res}"),
        };

        let res = parse_ppm_file(b"P6 99999999999999999999999999999 2 4 ").unwrap_err();
        match res {
            ImagesFromPpmFileError::WidthIsNotAUsize(_) => {}
            _ => panic!("Expected ImageFromPpmFileError::WidthIsNotAUsize found {res}"),
        };

        let res = parse_ppm_file(b"P6 42").unwrap_err();
        match res {
            ImagesFromPpmFileError::NoWhitespaceAfterWidth => {}
            _ => panic!("Expected ImageFromPpmFileError::NoWhitespaceAfterWidth found {res}"),
        };

        let res = parse_ppm_file(b"P6 ").unwrap_err();
        match res {
            ImagesFromPpmFileError::WidthNotFound => {}
            _ => panic!("Expected ImageFromPpmFileError::WidthNotFound found {res}"),
        };
    }

    #[test]
    fn bad_height() {
        let res = parse_ppm_file(b"P6 5 4f3 255 ").unwrap_err();
        match res {
            ImagesFromPpmFileError::HeightIsNotAUsize(_) => {}
            _ => panic!("Expected ImageFromPpmFileError::HeightIsNotAUsize found {res}"),
        };

        let res = parse_ppm_file(b"P6 5 f 255 ").unwrap_err();
        match res {
            ImagesFromPpmFileError::HeightIsNotAUsize(_) => {}
            _ => panic!("Expected ImageFromPpmFileError::HeightIsNotAUsize found {res}"),
        };

        let res = parse_ppm_file(b"P6 5 42f 255 ").unwrap_err();
        match res {
            ImagesFromPpmFileError::HeightIsNotAUsize(_) => {}
            _ => panic!("Expected ImageFromPpmFileError::HeightIsNotAUsize found {res}"),
        };

        let res = parse_ppm_file(b"P6 5 -42 255 ").unwrap_err();
        match res {
            ImagesFromPpmFileError::HeightIsNotAUsize(_) => {}
            _ => panic!("Expected ImageFromPpmFileError::HeightIsNotAUsize found {res}"),
        };

        let res = parse_ppm_file(b"P6 5 99999999999999999999999999999 255 ").unwrap_err();
        match res {
            ImagesFromPpmFileError::HeightIsNotAUsize(_) => {}
            _ => panic!("Expected ImageFromPpmFileError::HeightIsNotAUsize found {res}"),
        };

        let res = parse_ppm_file(b"P6 42 5").unwrap_err();
        match res {
            ImagesFromPpmFileError::NoWhitespaceAfterHeight => {}
            _ => panic!("Expected ImageFromPpmFileError::NoWhitespaceAfterHeight found {res}"),
        };

        let res = parse_ppm_file(b"P6 42 ").unwrap_err();
        match res {
            ImagesFromPpmFileError::HeightNotFound => {}
            _ => panic!("Expected ImageFromPpmFileError::HeightNotFound found {res}"),
        };
    }

    #[test]
    fn number_overflow() {
        let res = parse_ppm_file(format!("P6 {} 2 256 ", usize::MAX).as_bytes()).unwrap_err();
        match res {
            ImagesFromPpmFileError::WidthMulHeightOverflowsUsize => {}
            _ => {
                panic!("Expected ImageFromPpmFileError::WidthMulHeightOverflowsUsize found {res}")
            }
        };

        // Not sure how to test SizeMulColorByteCountOverflows since FailedToAllocateImageData
        // happens before
    }

    #[test]
    fn allocation_failure() {
        let res = parse_ppm_file(format!("P6 {} 1 256 ", usize::MAX).as_bytes()).unwrap_err();
        match res {
            ImagesFromPpmFileError::FailedToAllocateImageData(_) => {}
            _ => panic!("Expected ImageFromPpmFileError::FailedToAllocateImageData found {res}"),
        };
    }

    #[test]
    fn bad_maxval() {
        let res = parse_ppm_file(b"P6 4 2 2f55 ").unwrap_err();
        match res {
            ImagesFromPpmFileError::MaxvalIsNotAU16(_) => {}
            _ => panic!("Expected ImageFromPpmFileError::MaxvalIsNotAU16 found {res}"),
        };

        let res = parse_ppm_file(b"P6 4 2 f ").unwrap_err();
        match res {
            ImagesFromPpmFileError::MaxvalIsNotAU16(_) => {}
            _ => panic!("Expected ImageFromPpmFileError::MaxvalIsNotAU16 found {res}"),
        };

        let res = parse_ppm_file(b"P6 4 2 255f ").unwrap_err();
        match res {
            ImagesFromPpmFileError::MaxvalIsNotAU16(_) => {}
            _ => panic!("Expected ImageFromPpmFileError::MaxvalIsNotAU16 found {res}"),
        };

        let res = parse_ppm_file(b"P6 4 2 -255 ").unwrap_err();
        match res {
            ImagesFromPpmFileError::MaxvalIsNotAU16(_) => {}
            _ => panic!("Expected ImageFromPpmFileError::MaxvalIsNotAU16 found {res}"),
        };

        let res = parse_ppm_file(b"P6 4 2 999999999999999 ").unwrap_err();
        match res {
            ImagesFromPpmFileError::MaxvalIsNotAU16(_) => {}
            _ => panic!("Expected ImageFromPpmFileError::MaxvalIsNotAU16 found {res}"),
        };

        let res = parse_ppm_file(b"P6 4 2 255").unwrap_err();
        match res {
            ImagesFromPpmFileError::NoWhitespaceAfterMaxval => {}
            _ => panic!("Expected ImageFromPpmFileError::NoWhitespaceAfterMaxval found {res}"),
        };

        let res = parse_ppm_file(b"P6 4 2 ").unwrap_err();
        match res {
            ImagesFromPpmFileError::MaxvalNotFound => {}
            _ => panic!("Expected ImageFromPpmFileError::MaxvalNotFound found {res}"),
        };

        let res = parse_ppm_file(b"P6 4 2 0 ").unwrap_err();
        match res {
            ImagesFromPpmFileError::MaxvalCantBe0 => {}
            _ => panic!("Expected ImageFromPpmFileError::MaxvalCantBe0 found {res}"),
        };
    }

    #[test]
    fn not_enought_pixel_data() {
        let res = parse_ppm_file(b"P6 1 1 255 rg").unwrap_err();
        match res {
            ImagesFromPpmFileError::LessThanSizePixelsFoundInFile => {}
            _ => {
                panic!("Expected ImageFromPpmFileError::LessThanSizePixelsFoundInFile found {res}")
            }
        };

        let res = parse_ppm_file(b"P6 1 1 256 rrggb").unwrap_err();
        match res {
            ImagesFromPpmFileError::LessThanSizePixelsFoundInFile => {}
            _ => {
                panic!("Expected ImageFromPpmFileError::LessThanSizePixelsFoundInFile found {res}")
            }
        };
    }

    fn push_pixel_data(file: &mut Vec<u8>, pixels: &[Pixel]) {
        for pixel in pixels {
            file.push(pixel.rgba().r);
            file.push(pixel.rgba().g);
            file.push(pixel.rgba().b);
        }
    }
}
