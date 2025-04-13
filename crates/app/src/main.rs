mod app;
mod auto_blink;
mod drawio;
mod init;
mod lua_avatar;
mod mascot;
mod notion;
mod rig_config;
mod state;
mod terminal;
mod tracker_thread;
mod update;
mod user_prefs;

fn main() {
    env_logger::init();
    pipeline_logger::init_console(log::Level::Debug);
    let event_loop = winit::event_loop::EventLoop::new().unwrap();
    let mut app = app::App::new();
    event_loop.run_app(&mut app).unwrap();
}
