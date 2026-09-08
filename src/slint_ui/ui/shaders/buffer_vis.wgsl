struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) frag_position: vec2<f32>,
};

@vertex
fn vs_main(
    @builtin(vertex_index) vertex_index: u32
) -> VertexOutput {
    var output: VertexOutput;

    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0,  3.0),
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 3.0, -1.0)
    );

    let pos = positions[vertex_index];
    output.position = vec4<f32>(pos.x, -pos.y, 0.0, 1.0);
    output.frag_position = pos;
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

fn get_levels(index: u32) -> vec2<f32> {
    let vec_idx = index / 4u;
    let comp_idx = index % 4u;

    let dry_val = imm.levels_dry[vec_idx][comp_idx];
    let wet_val = imm.levels_wet[vec_idx][comp_idx];

    return vec2(dry_val * (1. - imm.params.x), wet_val*imm.params.x);
}

fn sdf_box(p: vec2<f32>, b: vec2<f32>) -> f32 {
    let d = abs(p) - b;
    return length(max(d, vec2<f32>(0.0))) + min(max(d.x, d.y), 0.0);
}

@fragment
fn fs_main(@location(0) frag_position: vec2<f32>) -> @location(0) vec4<f32> {
    let uv = frag_position * 0.5 + vec2<f32>(0.5);

    let num_bars: f32 = 128.;
    let gap_ratio: f32 = 0.2;

    let cell_x = uv.x * num_bars;
    let bar_index = u32(clamp(floor(cell_x), 0.0, num_bars - 1.0));

    let levels = get_levels(bar_index);
    let height_dry = clamp(levels.x, 0.0, 1.0);
    let height_wet = clamp(levels.y, 0.0, 1.0);

    let bar_width = (1.0 - gap_ratio) / num_bars;
    let half_width = bar_width * 0.5;
    let half_height_dry = (height_dry * 0.95) * 0.5;
    let half_height_wet = (height_wet * 0.95) * 0.5;

    let center_x = (f32(bar_index) + 0.5) / num_bars;
    let center = vec2<f32>(center_x, 0.5);

    let dist_dry = sdf_box(uv - center, vec2<f32>(half_width, half_height_dry));
    let dist_wet = sdf_box(uv - center, vec2<f32>(half_width, half_height_wet));

    let mask_dry = 1.0 - smoothstep(-fwidth(dist_dry), fwidth(dist_dry), dist_dry);
    let mask_wet = 1.0 - smoothstep(-fwidth(dist_wet), fwidth(dist_wet), dist_wet);

    // Apply the SDF masks scaled to 0.5 base alpha
    let dry_color = vec4<f32>(imm.primary_col.rgb, 0.5 * mask_dry);
    let wet_color = vec4<f32>(imm.secondary_col.rgb, 0.5 * mask_wet);

    // Standard Alpha-Over Blend (wet composite over dry)
    let out_a = wet_color.a + dry_color.a * (1.0 - wet_color.a);

    var out_rgb = vec3<f32>(0.0);
    if (out_a > 0.0) {
        out_rgb = (wet_color.rgb * wet_color.a + dry_color.rgb * dry_color.a * (1.0 - wet_color.a)) / out_a;
    }

    return vec4<f32>(out_rgb, out_a);
}