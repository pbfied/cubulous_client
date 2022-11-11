pub mod renderer;

use winit::event_loop::EventLoop;

use renderer::renderer::CubulousRenderer;


fn hello_triangle() {
    // Generic window setup
    let event_loop = EventLoop::new();

    let renderer = CubulousRenderer::new(&event_loop);

    renderer.run_blocking(event_loop);
}

fn main() {
    hello_triangle();

    // App::new()
    //     .add_system()
    //     .run();
}
