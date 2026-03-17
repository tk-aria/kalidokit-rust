//! Integration tests for `video_decoder::open()`.
//!
//! These tests exercise error paths since no test MP4 fixture exists yet.

use video_decoder::*;

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

#[test]
fn open_nonexistent_returns_error() {
    let result = open(
        "/nonexistent.mp4",
        dummy_wgpu_output(),
        SessionConfig::default(),
    );
    match result {
        Err(VideoError::FileNotFound(p)) => assert!(p.contains("nonexistent")),
        Err(other) => panic!("expected FileNotFound, got {:?}", other),
        Ok(_) => panic!("expected error for nonexistent file"),
    }
}

#[test]
fn open_non_mp4_extension() {
    let dir = std::env::temp_dir().join("vd_integration_open");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("test.txt");
    std::fs::write(&path, "not video").unwrap();

    let result = open(
        path.to_str().unwrap(),
        dummy_wgpu_output(),
        SessionConfig::default(),
    );
    assert!(result.is_err(), "opening a .txt file should fail");

    std::fs::remove_file(&path).ok();
}

#[test]
fn open_corrupt_mp4_returns_demux_error() {
    let dir = std::env::temp_dir().join("vd_integration_open_corrupt");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("corrupt.mp4");
    std::fs::write(&path, b"not a real mp4 file contents").unwrap();

    let result = open(
        path.to_str().unwrap(),
        dummy_wgpu_output(),
        SessionConfig::default(),
    );
    match result {
        Err(VideoError::Demux(_)) => {} // expected
        Err(other) => panic!("expected Demux error, got {:?}", other),
        Ok(_) => panic!("expected error for corrupt mp4"),
    }

    std::fs::remove_file(&path).ok();
}

#[test]
fn session_config_default_values() {
    let cfg = SessionConfig::default();
    assert!(cfg.looping);
    assert!(cfg.allow_software_fallback);
    assert_eq!(cfg.decode_buffer_size, 4);
    assert!(cfg.preferred_backend.is_none());
}

#[test]
fn open_with_no_software_fallback() {
    let dir = std::env::temp_dir().join("vd_integration_nosw");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("dummy.mp4");
    std::fs::write(&path, b"fake mp4 data").unwrap();

    let config = SessionConfig {
        allow_software_fallback: false,
        ..SessionConfig::default()
    };

    let result = open(path.to_str().unwrap(), dummy_wgpu_output(), config);
    assert!(result.is_err());

    std::fs::remove_file(&path).ok();
}
