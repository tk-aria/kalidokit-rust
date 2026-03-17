//! Decode an MP4 video to PNG frames using the macOS VideoToolbox backend.
//!
//! This example uses `AppleVideoSession` (AVFoundation / VideoToolbox)
//! for hardware-accelerated H.264 decoding on macOS.
//!
//! # Usage
//!
//! ```sh
//! cargo run -p video-decoder --example decode_to_png_apple -- <input.mp4> [output_dir]
//! ```

#[cfg(target_os = "macos")]
fn main() -> anyhow::Result<()> {
    use std::time::Duration;

    use video_decoder::backend::apple::AppleVideoSession;
    use video_decoder::handle::NativeHandle;
    use video_decoder::session::{OutputTarget, SessionConfig};
    use video_decoder::types::{ColorSpace, FrameStatus, PixelFormat};
    use video_decoder::VideoSession;

    env_logger::init();

    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: decode_to_png_apple <input.mp4> [output_dir]");
        std::process::exit(1);
    }

    let input = &args[1];
    let out_dir = args.get(2).map(|s| s.as_str()).unwrap_or("frames_apple");

    std::fs::create_dir_all(out_dir)?;

    let output = OutputTarget {
        native_handle: NativeHandle::Metal {
            texture: std::ptr::null_mut(),
            device: std::ptr::null_mut(),
        },
        format: PixelFormat::Rgba8Srgb,
        width: 0,
        height: 0,
        color_space: ColorSpace::default(),
    };

    let config = SessionConfig::default();
    let mut session = AppleVideoSession::new(input, output, &config)?;

    let info = session.info().clone();
    println!(
        "Video: {}x{}, {:.1} fps, {:.1}s, codec: {:?}, backend: {:?}",
        info.width,
        info.height,
        info.fps,
        info.duration.as_secs_f64(),
        info.codec,
        info.backend,
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
            FrameStatus::Waiting => continue,
            FrameStatus::EndOfStream => {
                println!("End of stream after {} frames", frame_count);
                break;
            }
        }
    }

    println!(
        "Decoded {} frames to {}/ using {:?}",
        frame_count, out_dir, info.backend
    );
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("This example requires macOS (VideoToolbox backend).");
    std::process::exit(1);
}
