mod app;
mod init;
mod rig_config;
mod state;
mod tracker_thread;
mod update;

fn main() {
    env_logger::init();
    let event_loop = winit::event_loop::EventLoop::new().unwrap();
    let mut app = app::App::new();
    event_loop.run_app(&mut app).unwrap();
}
