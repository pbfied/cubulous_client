pub mod rt_renderer;
pub mod rt_pipeline;
pub mod rt_accel;
pub mod rt_canvas;
pub mod rt_descriptor;
pub mod rt_ubo;
mod rt_frame;

pub fn add(left: usize, right: usize) -> usize {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
