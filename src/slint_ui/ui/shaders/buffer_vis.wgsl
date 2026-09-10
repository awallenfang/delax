

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var output: VertexOutput;
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 3.0, -1.0),
        vec2<f32>(-1.0,  3.0)
    );

    let pos = positions[vertex_index];
    output.position = vec4<f32>(pos, 0.0, 1.0);
    output.uv = pos * 0.5 + vec2<f32>(0.5);
    return output;
}

struct BufferVisUniforms {
    levels_dry: array<vec4<f32>, 32>,
    levels_wet: array<vec4<f32>, 32>,
    primary_col: vec4<f32>,
    secondary_col: vec4<f32>,
    params: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> imm: BufferVisUniforms;

fn smin(d1: f32, d2: f32, k: f32) -> f32 {
    let h = clamp(0.5 + 0.5 * (d2 - d1) / k, 0.0, 1.0);
    return mix(d2, d1, h) - k * h * (1.0 - h);
}
fn get_levels(index: u32) -> vec2<f32> {
    let vec_idx = index / 4u;
    let comp_idx = index % 4u;

    let dry_val = imm.levels_dry[vec_idx][comp_idx];
    let wet_val = imm.levels_wet[vec_idx][comp_idx];

    return vec2<f32>(dry_val * (1.0 - imm.params.x), wet_val * imm.params.x);
}

fn sdf_box(p: vec2<f32>, b: vec2<f32>) -> f32 {
    let d = abs(p) - b;
    return length(max(d, vec2<f32>(0.0))) + min(max(d.x, d.y), 0.0);
}

@fragment
fn fs_main(@location(0) uv: vec2<f32>) -> @location(0) vec4<f32> {
    let num_bars = 128.0;
    
    let current_bar = i32(floor(uv.x * num_bars));

    var dist_dry = 1e5;
    var dist_wet = 1e5;

    for (var offset: i32 = -1; offset <= 1; offset += 1) {
        let bar_idx = clamp(current_bar + offset, 0, 127);
        let u_bar_idx = u32(bar_idx);

        let levels = get_levels(u_bar_idx);
        let height_dry = clamp(levels.x, 0.0, 1.0);
        let height_wet = clamp(levels.y, 0.0, 1.0);

        let center_x = (f32(u_bar_idx) + 0.5) / num_bars;
        let bar_p = uv - vec2<f32>(center_x, 0.5);

        let half_width = (1.0 / num_bars) * 0.5;
        let half_height_dry = (height_dry * 0.95) * 0.5;
        let half_height_wet = (height_wet * 0.95) * 0.5;

        dist_dry = smin(dist_dry, sdf_box(bar_p, vec2<f32>(half_width, half_height_dry)), 0.01);
        dist_wet = smin(dist_wet, sdf_box(bar_p, vec2<f32>(half_width, half_height_wet)), 0.01);
    }

    let fw_dry = max(fwidth(dist_dry), 0.0005);
    let fw_wet = max(fwidth(dist_wet), 0.0005);

    let mask_dry = 1.0 - smoothstep(-fw_dry, fw_dry, dist_dry);
    let mask_wet = 1.0 - smoothstep(-fw_wet, fw_wet, dist_wet);

    let dry_a = 0.5 * mask_dry;
    let wet_a = 0.5 * mask_wet;

    let dry_rgb = imm.primary_col.rgb * dry_a;
    let wet_rgb = imm.secondary_col.rgb * wet_a;

    let out_a = wet_a + dry_a * (1.0 - wet_a);
    let out_rgb = wet_rgb + dry_rgb * (1.0 - wet_a);

    let final_rgb = select(vec3<f32>(0.0), out_rgb / out_a, out_a > 0.0);

    return vec4<f32>(final_rgb, out_a);
}