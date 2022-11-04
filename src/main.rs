pub mod renderer;

use std::fs::File;
use std::path::{Path, PathBuf};
use std::io::Read;

use winit::event_loop::EventLoop;

use renderer::renderer::CubulousRenderer;


fn hello_triangle() {
    // Generic window setup
    let event_loop = EventLoop::new();

    let renderer = CubulousRenderer::new(&event_loop);

    CubulousRenderer::run_blocking(renderer.window_id(), event_loop);
}

fn main() {
    hello_triangle();

    // App::new()
    //     .add_system()
    //     .run();
}
