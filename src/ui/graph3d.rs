use eframe::egui_glow;
use egui::{Align2, Color32, FontId, Rounding, mutex::Mutex};
use egui_glow::glow;
use nalgebra_glm::TVec3;
use std::sync::Arc;

pub struct Graph3D {
    gfx: Option<Arc<Mutex<GFX>>>,
    angle: (f32, f32),
}

struct GFX {
    program: glow::Program,
    vertex_array: glow::VertexArray,
    vertex_buffer: glow::Buffer,
}

const VERTEX_SHADER_SOURCE: &str = r#"
    const vec4 colors[3] = vec4[3](
        vec4(1.0, 0.0, 0.0, 1.0),
        vec4(0.0, 1.0, 0.0, 1.0),
        vec4(0.0, 0.0, 1.0, 1.0)
    );
    in vec4 aPos;
    out vec4 v_color;
    uniform float u_angle;
    uniform mat4 u_matrix;
    void main() {
        float prog = gl_VertexID / 100.0;
        prog /= 2.0;
        v_color = colors[gl_VertexID % 3] * (0.5 + prog);
        if (gl_VertexID == 0 || gl_VertexID == 1) {
            v_color = vec4(1.0, 0.0, 0.0, 1.0);
        } else if (gl_VertexID == 2 || gl_VertexID == 3) {
            v_color = vec4(0.0, 0.0, 1.0, 1.0);
        } else if (gl_VertexID == 4 || gl_VertexID == 5) {
            v_color = vec4(0.0, 1.0, 0.0, 1.0);
        }
        if (gl_VertexID < 6 && (gl_VertexID % 2) == 1) {
            v_color *= 0.5;
        }
        gl_Position = vec4(aPos.xyz, 1.0);
        gl_PointSize = 4.0;
        //gl_Position.x *= cos(u_angle);
        gl_Position = u_matrix * gl_Position;
    }
"#;

const FRAGMENT_SHADER_SOURCE: &str = r#"
    precision mediump float;
    in vec4 v_color;
    out vec4 out_color;
    void main() {
        out_color = v_color;
    }
"#;

impl GFX {
    pub fn new<'a>(cc: &'a eframe::CreationContext<'a>) -> Option<Arc<Mutex<Self>>> {
        unsafe {
            let gl = cc.gl.as_ref()?;
            use glow::HasContext as _;

            let shader_version = egui_glow::ShaderVersion::get(gl);

            let program = gl.create_program().expect("Cannot create program");

            if !shader_version.is_new_shader_interface() {
                log::warn!("Custom 3D painting hasn't been ported to {:?}", shader_version);
                return None;
            }

            let shader_sources =
                [(glow::VERTEX_SHADER, VERTEX_SHADER_SOURCE), (glow::FRAGMENT_SHADER, FRAGMENT_SHADER_SOURCE)];

            let shaders: Vec<_> = shader_sources
                .iter()
                .map(|(shader_type, shader_source)| {
                    let shader = gl.create_shader(*shader_type).expect("Cannot create shader");
                    gl.shader_source(shader, &format!("{}\n{}", shader_version.version_declaration(), shader_source));
                    gl.compile_shader(shader);
                    assert!(
                        gl.get_shader_compile_status(shader),
                        "Failed to compile custom_3d_glow {shader_type}: {}",
                        gl.get_shader_info_log(shader)
                    );

                    gl.attach_shader(program, shader);
                    shader
                })
                .collect();

            gl.link_program(program);
            assert!(gl.get_program_link_status(program), "{}", gl.get_program_info_log(program));

            for shader in shaders {
                gl.detach_shader(program, shader);
                gl.delete_shader(shader);
            }

            let vertex_array = gl.create_vertex_array().expect("Cannot create vertex array");
            let vertex_buffer = gl.create_buffer().expect("Cannot create vertex buffer");

            gl.bind_vertex_array(Some(vertex_array));
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(vertex_buffer));
            let stride = 4 * std::mem::size_of::<f32>() as i32;
            gl.vertex_attrib_pointer_f32(0, 3, glow::FLOAT, false, stride, 0);
            gl.enable_vertex_attrib_array(0);
            gl.bind_buffer(glow::ARRAY_BUFFER, None);
            gl.bind_vertex_array(None);

            Some(Arc::new(Mutex::new(Self { program, vertex_array, vertex_buffer })))
        }
    }

    fn paint(&self, gl: &glow::Context, angle: (f32, f32), buffer: [[f32; 4]; 100]) {
        use glow::HasContext as _;
        unsafe {
            let proj = nalgebra_glm::perspective_fov(std::f32::consts::PI / 6.0, 10.0, 10.0, 1.0, 100.0);

            let mut buffer1: [f32; 406] = [0.0; 406];
            buffer1[0] = 1.0;
            buffer1[4] = -1.0;

            buffer1[10] = 1.0;
            buffer1[14] = -1.0;

            buffer1[17] = 1.0;
            buffer1[21] = -1.0;
            let mut i = 6;
            while i < buffer.len() {
                buffer1[(i * 4) + 0] = buffer[i][0];
                buffer1[(i * 4) + 1] = buffer[i][1];
                buffer1[(i * 4) + 2] = buffer[i][2];
                buffer1[(i * 4) + 3] = buffer[i][3];
                i += 1;
            }

            let view = nalgebra_glm::look_at(
                &TVec3::new(f32::cos(angle.0) * 4.0, f32::sin(angle.1) * 4.0, f32::sin(angle.0) * 4.0),
                &TVec3::new(0.0, 0.0, 0.0),
                &TVec3::new(0.0, 1.0, 0.0),
            );

            gl.use_program(Some(self.program));
            gl.uniform_matrix_4_f32_slice(
                gl.get_uniform_location(self.program, "u_matrix").as_ref(),
                false,
                (proj * view).data.as_slice(),
            );
            gl.bind_vertex_array(Some(self.vertex_array));
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.vertex_buffer));
            gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, &buffer1.align_to::<u8>().1, glow::STATIC_DRAW);

            gl.line_width(1.0);
            gl.draw_arrays(glow::LINES, 0, 6);
            gl.line_width(2.0);
            gl.draw_arrays(glow::LINE_STRIP, 6, 99);
            gl.enable(glow::VERTEX_PROGRAM_POINT_SIZE);
            gl.draw_arrays(glow::POINTS, 105, 1);
            gl.disable(glow::VERTEX_PROGRAM_POINT_SIZE);
        }
    }
}

impl Graph3D {
    pub fn new<'a>(cc: &'a eframe::CreationContext<'a>) -> Self { Self { gfx: GFX::new(cc), angle: (0.0, 0.0) } }

    pub fn draw(&mut self, buffer: &[[f32; 4]; 100], ui: &mut egui::Ui) {
        let (rect, response) =
            ui.allocate_exact_size(egui::Vec2::splat(ui.spacing().interact_size.y * 10.0), egui::Sense::drag());

        ui.painter().rect(rect, Rounding::ZERO, Color32::BLACK, ui.visuals().noninteractive().bg_stroke);

        let gfx = match self.gfx.clone() {
            Some(gfx) => gfx,
            None => {
                ui.painter().text(
                    rect.center(),
                    Align2::CENTER_CENTER,
                    "Unavailable",
                    FontId::proportional(ui.spacing().interact_size.y),
                    Color32::WHITE,
                );
                return;
            }
        };

        self.angle.0 += response.drag_motion().x * 0.01;
        self.angle.1 += response.drag_motion().y * 0.01;
        // Clone locals so we can move them into the paint callback:
        let angle = self.angle;
        let buffer = buffer.clone();
        let cb = egui_glow::CallbackFn::new(move |_info, painter| {
            gfx.lock().paint(painter.gl(), angle, buffer);
        });

        let callback = egui::PaintCallback {
            rect: rect.shrink(ui.visuals().noninteractive().bg_stroke.width),
            callback: Arc::new(cb),
        };
        ui.painter().add(callback);
    }
}
