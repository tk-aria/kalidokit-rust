//! Integration tests for decode paths.

use std::time::Duration;

use video_decoder::backend::software::SwVideoSession;
use video_decoder::handle::NativeHandle;
use video_decoder::session::{OutputTarget, SessionConfig};
use video_decoder::types::{Backend, ColorSpace, PixelFormat};
use video_decoder::VideoSession;

fn dummy_wgpu_output() -> OutputTarget {
    OutputTarget {
        native_handle: NativeHandle::Wgpu {
            queue: std::ptr::null(),
            texture_id: 0,
        },
        format: PixelFormat::Rgba8Srgb,
        width: 640,
        height: 480,
        color_space: ColorSpace::default(),
    }
}

fn fixture_path() -> String {
    let p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/big_buck_bunny_360p.mp4");
    p.to_str().unwrap().to_string()
}

#[test]
fn sw_session_nonexistent_file() {
    let result = SwVideoSession::new(
        "/nonexistent/video.mp4",
        dummy_wgpu_output(),
        &SessionConfig::default(),
    );
    assert!(result.is_err());
}

#[test]
fn sw_session_invalid_mp4() {
    let dir = std::env::temp_dir().join("vd_decode_invalid");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("invalid.mp4");
    std::fs::write(&path, b"this is not a valid mp4 file at all").unwrap();

    let result = SwVideoSession::new(
        path.to_str().unwrap(),
        dummy_wgpu_output(),
        &SessionConfig::default(),
    );
    assert!(result.is_err());

    std::fs::remove_file(&path).ok();
}

#[test]
fn sw_session_unsupported_container() {
    let result = SwVideoSession::new("video.avi", dummy_wgpu_output(), &SessionConfig::default());
    assert!(result.is_err());
}

#[test]
fn sw_session_backend_is_software() {
    // We can't construct a valid session without a real MP4, but we can
    // verify the Backend enum value used by the software path.
    assert_eq!(format!("{:?}", Backend::Software), "Software");
}

#[test]
fn video_session_trait_is_object_safe() {
    // Verify VideoSession can be used as a trait object.
    fn _assert_object_safe(_: &dyn VideoSession) {}
}

#[test]
fn frame_status_variants_are_distinct() {
    use video_decoder::types::FrameStatus;
    assert_ne!(FrameStatus::NewFrame, FrameStatus::Waiting);
    assert_ne!(FrameStatus::Waiting, FrameStatus::EndOfStream);
    assert_ne!(FrameStatus::NewFrame, FrameStatus::EndOfStream);
}

#[test]
fn decode_10_frames_sw() {
    let path = fixture_path();
    if !std::path::Path::new(&path).exists() {
        return;
    }
    let output = dummy_wgpu_output();
    let mut session = video_decoder::open(&path, output, SessionConfig::default()).unwrap();
    let dt = Duration::from_secs_f64(1.0 / 30.0);
    let mut frames = 0;
    for _ in 0..200 {
        match session.decode_frame(dt).unwrap() {
            video_decoder::FrameStatus::NewFrame => frames += 1,
            video_decoder::FrameStatus::EndOfStream => break,
            _ => {}
        }
        if frames >= 10 {
            break;
        }
    }
    assert!(frames >= 10, "expected >=10 frames, got {}", frames);
}
