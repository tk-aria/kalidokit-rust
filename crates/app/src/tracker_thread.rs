use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;

use image::DynamicImage;
use solver::types::VideoInfo;
use tracker::holistic::HolisticTracker;
use tracker::HolisticResult;

/// A handle to the background tracker thread.
///
/// Frames are sent to the worker thread for ML inference, and results are
/// received back without blocking the main render loop.
pub struct TrackerThread {
    frame_sender: mpsc::SyncSender<(DynamicImage, VideoInfo)>,
    result_receiver: mpsc::Receiver<HolisticResult>,
    /// When true, only face detection runs (pose/hand skipped).
    face_only: Arc<AtomicBool>,
}

impl TrackerThread {
    /// Spawn a new tracker worker thread that owns the given `HolisticTracker`.
    ///
    /// The worker receives frames on an internal channel, runs `detect()`,
    /// and sends results back.  Both channels have a buffer size of 1 so that
    /// stale frames are dropped rather than queued.
    #[allow(dead_code)]
    pub fn new(tracker: HolisticTracker) -> Self {
        Self::new_with_mode(tracker, false)
    }

    /// Spawn a tracker worker with explicit face_only mode.
    pub fn new_with_mode(tracker: HolisticTracker, face_only_initial: bool) -> Self {
        let face_only = Arc::new(AtomicBool::new(face_only_initial));
        let face_only_clone = face_only.clone();

        // Buffer size 1: if the tracker is still busy, try_send will fail
        // and the main loop simply drops that frame.
        let (frame_sender, frame_receiver) = mpsc::sync_channel::<(DynamicImage, VideoInfo)>(1);
        let (result_sender, result_receiver) = mpsc::sync_channel::<HolisticResult>(1);

        thread::Builder::new()
            .name("tracker-worker".into())
            .spawn(move || {
                log::info!("Tracker worker thread started");
                while let Ok((frame, _video_info)) = frame_receiver.recv() {
                    let is_face_only = face_only_clone.load(Ordering::Relaxed);
                    match tracker.detect_with_mode(&frame, is_face_only) {
                        Ok(result) => {
                            // If the main thread hasn't consumed the previous result yet,
                            // just drop the older one and replace it.
                            let _ = result_sender.try_send(result);
                        }
                        Err(e) => {
                            log::warn!("Tracker detection failed: {e}");
                        }
                    }
                }
                log::info!("Tracker worker thread exiting");
            })
            .expect("failed to spawn tracker worker thread");

        Self {
            frame_sender,
            result_receiver,
            face_only,
        }
    }

    /// Set face-only mode at runtime.
    #[allow(dead_code)]
    pub fn set_face_only(&self, face_only: bool) {
        self.face_only.store(face_only, Ordering::Relaxed);
    }

    /// Send a frame to the tracker thread for processing.
    ///
    /// This is non-blocking: if the tracker is still busy with the previous
    /// frame, this frame is silently dropped.
    pub fn send_frame(&self, frame: DynamicImage, video_info: VideoInfo) {
        // try_send: drop the frame if the channel is full (tracker busy)
        let _ = self.frame_sender.try_send((frame, video_info));
    }

    /// Try to receive a tracking result without blocking.
    ///
    /// Returns `Some(result)` if the tracker thread has finished processing
    /// a frame, or `None` if no new result is available yet.
    pub fn try_recv_result(&self) -> Option<HolisticResult> {
        self.result_receiver.try_recv().ok()
    }
}
