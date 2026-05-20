use ab_glyph::{FontRef, PxScale};
use anyhow::{Error, bail};
use image::{DynamicImage, GenericImageView, Pixel, RgbaImage};
use imageproc::drawing::{draw_filled_rect_mut, draw_text_mut, text_size};
use imageproc::rect::Rect;
use opencv::imgproc::COLOR_BGR2RGB;
use std::path::Path;
use image::imageops;
use clap::Parser;
use opencv::videoio::{self, VideoCaptureTraitConst};
use opencv::prelude::*;
use std::thread;

const CHAR: &str = "@";
const BACK_DARKEN: f32= 0.7;
const OUTPUT_WIDTH: u32 = 80; 
const CHAR_SIZE: f32 = 23.0;
const VCODEC: &str = "libx264";
const CHAR_LIST: [&str; 3] = ["#", "%", "@"];

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long, default_value_t=OUTPUT_WIDTH)]
    width: u32,

    #[arg(short, long, default_value_t=CHAR_SIZE)]
    char_size: f32,

    #[arg(short, long, default_value_t=BACK_DARKEN)]
    back_darken: f32,

    #[arg(short, long, default_value_t=VCODEC.to_string())]
    vcodec: String,

    #[arg(short, long)]
    in_media: String,

    #[arg(short, long)]
    out_media: String,
}

// code pulled out of https://github.com/jerry73204/rust-cv-convert because the crate was being stinky
// ----------------------

pub use element_type::*;

use crate::mat_ext::MatExt;
use crate::try_to::TryToCv;
mod element_type {
    pub trait OpenCvElement {
        const DEPTH: i32;
    }

    impl OpenCvElement for u8 {
        const DEPTH: i32 = opencv::core::CV_8U;
    }

    impl OpenCvElement for i8 {
        const DEPTH: i32 = opencv::core::CV_8S;
    }

    impl OpenCvElement for u16 {
        const DEPTH: i32 = opencv::core::CV_16U;
    }

    impl OpenCvElement for i16 {
        const DEPTH: i32 = opencv::core::CV_16S;
    }

    impl OpenCvElement for i32 {
        const DEPTH: i32 = opencv::core::CV_32S;
    }

    impl OpenCvElement for f32 {
        const DEPTH: i32 = opencv::core::CV_32F;
    }

    impl OpenCvElement for f64 {
        const DEPTH: i32 = opencv::core::CV_64F;
    }
}

mod try_to {
    /// Fallible type conversion that is analogous to [TryInto](std::convert::TryInto).
    pub trait TryToCv<T> {
        type Error;

        fn try_to_cv(&self) -> Result<T, Self::Error>;
    }
}

impl try_to::TryToCv<image::DynamicImage> for opencv::core::Mat {
    type Error = Error;

    fn try_to_cv(&self) -> Result<image::DynamicImage, Self::Error> {
        let rows = self.rows();
        let cols = self.cols();

        let depth = self.depth();
        let n_channels = self.channels();
        let width = cols as u32;
        let height = rows as u32;

        let image: image::DynamicImage = match (depth, n_channels) {
            (opencv::core::CV_8U, 3) => mat_to_image_buffer_rgb::<u8>(self, width, height).into(),
            (opencv::core::CV_16U, 3) => mat_to_image_buffer_rgb::<u16>(self, width, height).into(),
            (opencv::core::CV_32F, 3) => mat_to_image_buffer_rgb::<f32>(self, width, height).into(),
            _ => bail!("Mat of type {} is not supported", self.type_name()),
        };

        Ok(image)
    }
}

mod mat_ext {
    use std::slice;

use anyhow::ensure;

use super::*;

    pub trait MatExt {
        type Error;
        fn size_with_depth(&self) -> Vec<usize>;

        fn numel(&self) -> usize {
            self.size_with_depth().iter().product()
        }

        fn as_slice<T>(&self) -> Result<&[T], Self::Error>
        where
            T: OpenCvElement;

        fn type_name(&self) -> String;
    }

    impl MatExt for opencv::core::Mat {
        type Error = Error;

        fn size_with_depth(&self) -> Vec<usize> {
            let size = self.mat_size();
            let size = size.iter().map(|&dim| dim as usize);
            let channels = self.channels() as usize;
            size.chain([channels]).collect()
        }

        fn as_slice<T>(&self) -> Result<&[T], Self::Error>
        where
            T: OpenCvElement,
        {
            ensure!(self.depth() == T::DEPTH, "element type mismatch");
            ensure!(self.is_continuous(), "Mat data must be continuous");

            let numel = self.numel();
            let ptr = self.ptr(0)? as *const T;

            let slice = unsafe { slice::from_raw_parts(ptr, numel) };
            Ok(slice)
        }

        fn type_name(&self) -> String {
            opencv::core::type_to_string(self.typ()).unwrap()
        }
        
        fn numel(&self) -> usize {
            self.size_with_depth().iter().product()
        }
    }
}

fn mat_to_image_buffer_rgb<T>(
    mat: &opencv::core::Mat,
    width: u32,
    height: u32,
) -> image::ImageBuffer<image::Rgb<T>, Vec<T>>
where
    T: image::Primitive + OpenCvElement + DataType,
    image::Rgb<T>: image::Pixel<Subpixel = T>,
{
    type Image<T> = image::ImageBuffer<image::Rgb<T>, Vec<T>>;

    match mat.as_slice::<T>() {
        Ok(slice) => Image::<T>::from_vec(width, height, slice.to_vec()).unwrap(),
        Err(_) => Image::<T>::from_fn(width, height, |col, row| {
            let opencv::core::Point3_::<T> { x, y, z } = *mat.at_2d(row as i32, col as i32).unwrap();
            image::Rgb([x, y, z])
        }),
    }
}

// ----------------------
// end of cv-convert code

fn bright_map_char(bright: u8) -> &'static str {
    let pos = (bright as f32 * (CHAR_LIST.len() - 1) as f32) / 255.0;
    return CHAR_LIST[pos.round() as usize];
}

fn proc_img(back_darken: f32, in_media: DynamicImage, sample_width: u32, sample_height: u32, w: u32, h: u32, scale: PxScale, font: FontRef, output: &Path) {
    let mut image = RgbaImage::new(w * sample_width, h * sample_height);

    let mut x: i32 = 0;
    let mut y: i32 = 0;
    
    for height in 0..sample_height {
        for width in 0..sample_width {
            let col = in_media.get_pixel(width, height).to_rgba();
            // col.to_luma()[0]
            let _char = bright_map_char(col.to_luma()[0]);
            
            let mut back_col = col.clone();
            back_col[0] = (back_col[0] as f32 * back_darken) as u8;
            back_col[1] = (back_col[1] as f32 * back_darken) as u8;
            back_col[2] = (back_col[2] as f32 * back_darken) as u8;

            draw_filled_rect_mut(&mut image, Rect::at(x, y).of_size(w, h), back_col);

            draw_text_mut(&mut image, col, x, y, scale, &font, _char);
            x += w as i32;
        }
        y += h as i32;
        x = 0;
    }
    image.save(output).unwrap();
}

fn main() {
    let args = Args::parse();

    // figure out character dimensions
    let font = FontRef::try_from_slice(include_bytes!("../VT323-Regular.ttf")).unwrap();
    let mut scale = PxScale {
        x: args.char_size,
        y: args.char_size,
    };
    let (w, h) = text_size(scale, &font, CHAR);
    if w < h {
        let nw = h as f32 / w as f32;
        scale = PxScale {
            x: h as f32 * (nw),
            y: h as f32,
        };
    }
    let (w, h) = text_size(scale, &font, CHAR);

    // TODO: make this less dumb
    let video_extensions: [&str; 4] = [".mp4", ".webm", ".mkv", ".mov"];
    
    if video_extensions.iter().any(|&s| args.in_media.ends_with(s)) {
        let mut capture = videoio::VideoCapture::from_file(&args.in_media, videoio::CAP_ANY).unwrap();

        if !capture.is_opened().unwrap_or(false) {
            panic!("Unable to open the video file!");
        }

        let input_sample_width = capture.get(videoio::CAP_PROP_FRAME_WIDTH).unwrap();
        let input_sample_height = capture.get(videoio::CAP_PROP_FRAME_HEIGHT).unwrap();
        println!("{} - {} ", input_sample_width, input_sample_height);
        
        let fps = capture.get(videoio::CAP_PROP_FPS).unwrap();
        let frames = capture.get(videoio::CAP_PROP_FRAME_COUNT).unwrap() as u32;

        println!("{} / {}", frames, fps);

        let mut frame = Mat::default();

        // ----------------------------------------

        std::fs::create_dir("./rascii_frames").unwrap();

        let mut threads = Vec::new();

        for i in 0..frames {
            capture.read(&mut frame).unwrap();

            // color space conversion
            let mut new_frame = Mat::default();
            opencv::imgproc::cvt_color_def(&frame, &mut new_frame, COLOR_BGR2RGB).unwrap();

            // clone variables for threading
            let _back_darken = args.back_darken.clone();
            let _w = w.clone();
            let _h = h.clone();
            let _scale = scale.clone();
            let _font = font.clone();

            // spawn conversion thread
            let handle = thread::spawn(move || {
                let mut _img = new_frame.try_to_cv().unwrap();
                _img = _img.resize(args.width, args.width, imageops::FilterType::Nearest);
                let sample_width = _img.width();
                let sample_height = _img.height();
                let path = format!("./rascii_frames/{i}.png");
                proc_img(_back_darken, _img, sample_width, sample_height, _w, _h, _scale, _font, Path::new(&path));
            });
            threads.push(handle);
        }

        for thread in threads {
            thread.join().unwrap();
        }

        let codec = args.vcodec;
        let orig_video = args.in_media;
        let out_video = args.out_media;

        // reprocess frames into video
        std::process::Command::new("ffmpeg").args(["-loglevel", "quiet", "-hide_banner", "-nostats", "-r", &format!("{fps}"), "-i", "./rascii_frames/%01d.png", "-vcodec", &format!("{codec}"), "-crf", "0", "-y", "temp.mp4"]).output().unwrap();
        std::process::Command::new("ffmpeg").args(["-loglevel", "quiet", "-hide_banner", "-nostats", "-i", "temp.mp4", "-i", &format!("{orig_video}"), "-vcodec", &format!("{codec}"), "-map", "0:v", "-map", "1:a?", "-crf", "0", "-y", &format!("{out_video}")]).output().unwrap();
        
        std::fs::remove_dir_all("./rascii_frames").unwrap();
    }
    else {
        let in_im = Path::new(&args.in_media);
        let path = Path::new(&args.out_media);
        let mut sample = image::open(in_im).unwrap();
        sample = sample.resize(args.width, args.width, imageops::FilterType::Nearest);
        let sample_height = sample.height();
        let sample_width = sample.width();
        proc_img(args.back_darken, sample, sample_width, sample_height, w, h, scale, font, &path);
    }
}