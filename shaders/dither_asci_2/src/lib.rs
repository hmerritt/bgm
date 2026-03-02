#![cfg_attr(target_arch = "spirv", no_std)]

use spirv_std::glam::{Vec2, Vec3, Vec4};
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
pub fn vs_main(#[spirv(vertex_index)] vertex_index: i32, #[spirv(position)] out_pos: &mut Vec4) {
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

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn mix_f32(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn get_char_mask(mut intensity: f32) -> i32 {
    intensity = intensity.clamp(0.0, 1.0);
    let idx = (intensity * 7.0) as i32;
    match idx {
        0 => 0,
        1 => 2,
        2 => 34,
        3 => 328,
        4 => 2976,
        5 => 28662,
        6 => 63903,
        _ => 65535,
    }
}

fn sample_character(local_uv: Vec2, mask: i32) -> f32 {
    let p = (local_uv * 4.0).floor();
    if p.x < 0.0 || p.x > 3.0 || p.y < 0.0 || p.y > 3.0 {
        return 0.0;
    }
    let bit_pos = p.x as i32 + (p.y as i32) * 4;
    ((mask >> bit_pos) & 1) as f32
}

fn render_ascii_luminance(frag_coord: Vec2, resolution: Vec2, time: f32, mouse: Vec2) -> f32 {
    let font_size = 8.0;
    let grid_scaled = frag_coord / font_size;
    let grid_pos = grid_scaled.floor();
    let local_uv = grid_scaled.fract();
    let grid_center = grid_pos * font_size + Vec2::splat(font_size * 0.5);

    let mut uv = grid_center / resolution;
    uv = uv * 2.0 - Vec2::splat(1.0);
    uv.x *= resolution.x / resolution.y;

    let phase_time = time * 0.8;
    let v1 = (uv.x * 6.0 + phase_time).sin();
    let v2 = (uv.y * 4.0 - phase_time).sin();
    let v3 = (uv.x * uv.y * 5.0 + phase_time * 1.5).sin();

    let mut pattern_intensity = (v1 + v2 + v3) / 3.0;
    pattern_intensity = pattern_intensity * 0.5 + 0.5;

    let mut mouse_pos = mouse;
    if mouse_pos.x <= 0.0 && mouse_pos.y <= 0.0 {
        mouse_pos = resolution * 0.5;
    }

    let mouse_dist = grid_center.distance(mouse_pos);
    let interaction_radius = 100.0;
    let destruction_multiplier = smoothstep(0.0, interaction_radius, mouse_dist);
    let final_intensity = pattern_intensity * destruction_multiplier;

    let char_mask = get_char_mask(final_intensity);
    sample_character(local_uv, char_mask) * final_intensity
}

fn get_glass_height(uv: Vec2) -> f32 {
    let sheared = Vec2::new(uv.x + uv.y * 0.15, uv.x * -0.15 + uv.y) * 4.0;
    let p = sheared.fract() - Vec2::splat(0.5);
    let d = p.x.abs().max(p.y.abs());
    smoothstep(0.48, 0.38, d)
}

fn get_normal(uv: Vec2) -> Vec3 {
    let e = Vec2::new(0.002, 0.0);
    let h = get_glass_height(uv);
    let hx = get_glass_height(uv + e);
    let hy = get_glass_height(uv + Vec2::new(e.y, e.x));
    Vec3::new(h - hx, h - hy, 0.02).normalize()
}

#[spirv(fragment)]
pub fn fs_main(
    #[spirv(frag_coord)] frag_coord: Vec4,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] uniforms: &ShaderUniforms,
    output: &mut Vec4,
) {
    let resolution = Vec2::new(
        uniforms.resolution.x.max(1.0),
        uniforms.resolution.y.max(1.0),
    );
    let frag_xy = Vec2::new(frag_coord.x, frag_coord.y);
    let uv = frag_xy / resolution;
    let mouse = Vec2::new(uniforms.mouse.x, uniforms.mouse.y);

    let height = get_glass_height(uv);
    let normal = get_normal(uv);

    let refraction_strength = 40.0;
    let refraction_offset = Vec2::new(normal.x, normal.y) * refraction_strength;

    let lum_r = render_ascii_luminance(
        frag_xy - refraction_offset * 1.0,
        resolution,
        uniforms.time_seconds,
        mouse,
    );
    let lum_g = render_ascii_luminance(
        frag_xy - refraction_offset * 1.05,
        resolution,
        uniforms.time_seconds,
        mouse,
    );
    let lum_b = render_ascii_luminance(
        frag_xy - refraction_offset * 1.15,
        resolution,
        uniforms.time_seconds,
        mouse,
    );

    let base_color = Vec3::new(0.15, 0.85, 0.4) * 1.5;

    let mut col = Vec3::new(
        mix_f32(0.02, base_color.x, lum_r),
        mix_f32(0.05, base_color.y, lum_g),
        mix_f32(0.02, base_color.z, lum_b),
    );

    let light_dir = Vec3::new(0.5, 0.8, 1.0).normalize();
    let view_dir = Vec3::new(0.0, 0.0, 1.0);
    let half_vector = (light_dir + view_dir).normalize();
    let specular = normal.dot(half_vector).max(0.0).powf(64.0);

    col += Vec3::splat(specular * smoothstep(0.1, 0.9, height));
    col *= Vec3::splat(mix_f32(0.1, 1.0, smoothstep(0.0, 0.1, height)));

    *output = Vec4::new(col.x, col.y, col.z, 1.0);
}
