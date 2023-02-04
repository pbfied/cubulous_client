use tobj;
use crate::renderer::vertex::Vertex;


pub(crate) fn load_model(path: &str) -> (Vec<Vertex>, Vec<u32>) {
    let mut vertex_vec: Vec<Vertex> = Vec::new();
    let mut index_vec: Vec<u32> = Vec::new();
    let (models, _) =
        tobj::load_obj(path, &tobj::LoadOptions::default()).unwrap();
    for m in models.iter() {
        for n in 0..(m.mesh.positions.len() / 3) { // Push the vertices/texcords for each face
            let pos: [f32; 3] = [
                *m.mesh.positions.get((3 * n + 0) as usize).unwrap(),
                *m.mesh.positions.get((3 * n + 1) as usize).unwrap(),
                *m.mesh.positions.get((3 * n + 2) as usize).unwrap()
            ];
            let tex_cord: [f32; 2] = [
                *m.mesh.texcoords.get((2 * n) as usize).unwrap(),
                1.0 - *m.mesh.texcoords.get((2 * n + 1) as usize).unwrap()
            ];
            let color: [f32; 3] = [1.0, 1.0, 1.0];
            vertex_vec.push(Vertex {
                pos,
                color,
                texCoord: tex_cord,
            });
        }
        index_vec = m.mesh.indices.clone()
    }

    (vertex_vec, index_vec)
}