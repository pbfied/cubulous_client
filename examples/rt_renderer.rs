use winit::event_loop::EventLoop;
use rt_renderer::rt_renderer::RtRenderer;

fn main() {
    // Generic window setup
    let event_loop = EventLoop::new();

    let renderer = RtRenderer::new(&event_loop);

    renderer.run_blocking(event_loop);
}