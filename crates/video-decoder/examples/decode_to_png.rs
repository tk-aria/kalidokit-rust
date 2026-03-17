//! Decode an MP4 video to PNG frames using the software decoder.
//!
//! This example uses the software backend (`SwVideoSession`) directly
//! to decode H.264 NAL units on the CPU and write RGBA frames as PNG files.
//!
//! # Usage
//!
//! ```sh
//! cargo run -p video-decoder --example decode_to_png -- <input.mp4> [output_dir]
//! ```

use std::time::Duration;

use video_decoder::backend::software::SwVideoSession;
use video_decoder::handle::NativeHandle;
use video_decoder::session::{OutputTarget, SessionConfig};
use video_decoder::types::{ColorSpace, FrameStatus, PixelFormat};
use video_decoder::VideoSession;

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: decode_to_png <input.mp4> [output_dir]");
        std::process::exit(1);
    }

    let input = &args[1];
    let out_dir = args.get(2).map(|s| s.as_str()).unwrap_or("frames");

    std::fs::create_dir_all(out_dir)?;

    // We use a dummy Wgpu output target since the SW backend stores RGBA
    // in a CPU-side buffer accessible via `frame_rgba()`.
    let output = OutputTarget {
        native_handle: NativeHandle::Wgpu {
            queue: std::ptr::null(),
            texture_id: 0,
        },
        format: PixelFormat::Rgba8Srgb,
        width: 0, // will be determined from the video
        height: 0,
        color_space: ColorSpace::default(),
    };

    let config = SessionConfig::default();
    let mut session = SwVideoSession::new(input, output, &config)?;

    let info = session.info().clone();
    println!(
        "Video: {}x{}, {:.1} fps, {:.1}s, codec: {:?}",
        info.width,
        info.height,
        info.fps,
        info.duration.as_secs_f64(),
        info.codec,
    );

    let max_frames: usize = 10;
    let mut frame_count: usize = 0;
    let dt = if info.fps > 0.0 {
        Duration::from_secs_f64(1.0 / info.fps)
    } else {
        Duration::from_millis(33)
    };

    while frame_count < max_frames {
        match session.decode_frame(dt)? {
            FrameStatus::NewFrame => {
                let rgba = session.frame_rgba();
                let w = session.info().width;
                let h = session.info().height;

                let path = format!("{}/frame_{:04}.png", out_dir, frame_count);
                image::save_buffer(&path, rgba, w, h, image::ColorType::Rgba8)?;
                println!("Wrote {}", path);
                frame_count += 1;
            }
            FrameStatus::Waiting => {
                // Decoder needs more data; continue feeding.
                continue;
            }
            FrameStatus::EndOfStream => {
                println!("End of stream after {} frames", frame_count);
                break;
            }
        }
    }

    println!("Decoded {} frames to {}/", frame_count, out_dir);
    Ok(())
}
