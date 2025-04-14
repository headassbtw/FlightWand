pub mod logger;

pub fn modifier(input: &[f32; 4], up: [f32; 3]) -> [f32; 4] {
    let id = nalgebra::Quaternion::new(0.0, up[0], up[1], up[2]).normalize();
    let quat = nalgebra::Quaternion::new(input[3], input[0], input[1], input[2]);
    let res = quat * id * quat.conjugate();
    let res = res.coords.normalize().clone_owned();

    // we don't use the fourth coordinate for anything this function touches,
    // set it to something out of the graph's bounds
    [res.x, res.y, res.z, -2.0]
}
