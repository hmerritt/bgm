#![cfg_attr(target_arch = "spirv", no_std)]

use spirv_std::glam::{Vec2, Vec4};
use spirv_std::num_traits::Float;
use spirv_std::spirv;

#[repr(C)]
pub struct ShaderUniforms {
    pub time_seconds: f32,
    pub frame_index: u32,
    pub mouse_enabled: u32,
    pub _padding: u32,
    pub resolution: Vec4,
    pub mouse: Vec4,
}

#[spirv(vertex)]
pub fn vs_main(
    #[spirv(vertex_index)] vertex_index: i32,
    #[spirv(position)] out_pos: &mut Vec4,
) {
    let x = match vertex_index {
        0 => -1.0,
        1 => 3.0,
        _ => -1.0,
    };
    let y = match vertex_index {
        0 => -1.0,
        1 => -1.0,
        _ => 3.0,
    };
    *out_pos = Vec4::new(x, y, 0.0, 1.0);
}

#[spirv(fragment)]
pub fn fs_main(
    #[spirv(frag_coord)] frag_coord: Vec4,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] uniforms: &ShaderUniforms,
    output: &mut Vec4,
) {
    let resolution = Vec2::new(uniforms.resolution.x.max(1.0), uniforms.resolution.y.max(1.0));
    let uv = Vec2::new(frag_coord.x / resolution.x, frag_coord.y / resolution.y);
    let center = uv - Vec2::splat(0.5);
    let dist = center.length();

    let mut mouse_mix = 0.0;
    if uniforms.mouse_enabled != 0 {
        let mouse = Vec2::new(uniforms.mouse.x / resolution.x, uniforms.mouse.y / resolution.y);
        mouse_mix = (uv - mouse).length().clamp(0.0, 1.0);
    }

    let t = uniforms.time_seconds;
    let wave = (12.0 * dist - t * 2.5).sin() * 0.5 + 0.5;
    let r = (uv.x + t * 0.08).sin() * 0.5 + 0.5;
    let g = (uv.y + t * 0.11).cos() * 0.5 + 0.5;
    let b = (wave + 0.2 * (1.0 - mouse_mix)).clamp(0.0, 1.0);

    *output = Vec4::new(r, g, b, 1.0);
}
