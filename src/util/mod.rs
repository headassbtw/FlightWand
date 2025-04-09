pub mod logger;

pub fn modifier(input: &[f32; 4]) -> [f32; 4] {
    let id = nalgebra::Quaternion::new(0.0, 0.0, 0.6, -1.0).normalize();
    let quat = nalgebra::Quaternion::new(input[3], input[0], input[1], input[2]);
    let res = quat * id * quat.conjugate();
    let res = res.coords.clone_owned();

    [res.x, res.y, res.z, 1.0]
}
