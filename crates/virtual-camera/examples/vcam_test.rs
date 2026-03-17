/// Standalone test: send 150 frames (5 seconds @ 30fps) of solid color
/// to the KalidoKit virtual camera device.
///
/// Run: cargo run -p virtual-camera --example vcam_test

fn main() {
    env_logger::init();

    #[cfg(target_os = "macos")]
    {
        use virtual_camera::{MacOsVirtualCamera, VirtualCamera};

        let width: u32 = 1280;
        let height: u32 = 720;
        let frame_count = 150;

        println!("[vcam_test] Creating virtual camera...");
        let mut vcam = MacOsVirtualCamera::new();

        println!("[vcam_test] Starting (discovering device & acquiring buffer queue)...");
        match vcam.start() {
            Ok(()) => println!("[vcam_test] Started successfully!"),
            Err(e) => {
                eprintln!("[vcam_test] Failed to start: {e}");
                std::process::exit(1);
            }
        }

        println!(
            "[vcam_test] Sending {} frames ({}x{})...",
            frame_count, width, height
        );
        let pixel_count = (width * height) as usize;

        for i in 0..frame_count {
            // Cycle through red → green → blue every 50 frames
            let mut rgba = vec![0u8; pixel_count * 4];
            let phase = i % 150;
            let (r, g, b) = if phase < 50 {
                (255u8, 0u8, 0u8)
            } else if phase < 100 {
                (0u8, 255u8, 0u8)
            } else {
                (0u8, 0u8, 255u8)
            };

            for pixel in rgba.chunks_exact_mut(4) {
                pixel[0] = r;
                pixel[1] = g;
                pixel[2] = b;
                pixel[3] = 255;
            }

            match vcam.send_frame(&rgba, width, height) {
                Ok(()) => {
                    if i % 30 == 0 {
                        println!(
                            "[vcam_test] Frame {}/{} sent (RGB={},{},{})",
                            i + 1,
                            frame_count,
                            r,
                            g,
                            b
                        );
                    }
                }
                Err(e) => {
                    eprintln!("[vcam_test] send_frame error at frame {}: {e}", i);
                    break;
                }
            }

            std::thread::sleep(std::time::Duration::from_millis(33)); // ~30fps
        }

        println!("[vcam_test] Stopping...");
        vcam.stop();
        println!("[vcam_test] Done! Check Zoom/FaceTime/QuickTime for the virtual camera output.");
    }

    #[cfg(not(target_os = "macos"))]
    {
        eprintln!("This test is macOS only.");
    }
}
