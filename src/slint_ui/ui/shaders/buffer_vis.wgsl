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
    levels_dry: array<vec4<f32>, 128>,
    levels_wet: array<vec4<f32>, 128>,
    primary_col: vec4<f32>,
    secondary_col: vec4<f32>,
    num_bars: u32
};

var<immediate> imm: BufferVisUniforms;

fn get_levels(index: u32) -> vec2<f32> {
    let vec_idx = index / 4u;
    let comp_idx = index % 4u;
    return vec2(imm.levels_dry[vec_idx][comp_idx], imm.levels_wet[vec_idx][comp_idx]);
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
    let local_x = fract(cell_x);

    let normalized_heights = clamp(get_levels(bar_index), 0.0, 1.0);

    let bar_width = (1.0 - gap_ratio) / num_bars;
    let half_width = bar_width * 0.5;
    let half_height_dry = (normalized_heights.x * 0.95) * 0.5;
    let half_height_wet = (normalized_heights.y * 0.95) * 0.5;

    let bar_center = vec2<f32>(
        (f32(bar_index) + 0.5) / num_bars,
        half_height + 0.025
    );

    let dist_dry = sdf_box(uv - bar_center, vec2<f32>(half_width, half_height_dry));
    let dist_wet = sdf_box(uv - bar_center, vec2<f32>(half_width, half_height_wet));

    let smoothing_dry = fwidth(dist_dry);
    let smoothing_wet = fwidth(dist_wet);
    let alpha_dry = 0.5 - smoothstep(-smoothing_dry, smoothing_dry, dist_dry);
    let alpha_wet = 0.5 - smoothstep(-smoothing_wet, smoothing_wet, dist_wet);


    let out_col = vec4<f32>(imm.primary_col.rgb, imm.primary_col.a * alpha_dry) + vec4<f32>(imm.secondary_col.rgb, imm.primary_col.a * alpha_wet);
    return vec4<f32>(imm.primary_col.rgb, imm.primary_col.a * alpha);
}