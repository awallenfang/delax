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

struct SpectrumUniforms {
    levels: array<vec4<f32>, 8>,
    primary_col: vec4<f32>,
};

var<immediate> imm: SpectrumUniforms;

fn get_level(index: u32) -> f32 {
    let vec_idx = index / 4u;
    let comp_idx = index % 4u;
    return imm.levels[vec_idx][comp_idx];
}

fn sdf_box(p: vec2<f32>, b: vec2<f32>) -> f32 {
    let d = abs(p) - b;
    return length(max(d, vec2<f32>(0.0))) + min(max(d.x, d.y), 0.0);
}

@fragment
fn fs_main(@location(0) frag_position: vec2<f32>) -> @location(0) vec4<f32> {
    let uv = frag_position * 0.5 + vec2<f32>(0.5);

    let num_bars: f32 = 32.0;
    let gap_ratio: f32 = 0.2;

    let cell_x = uv.x * num_bars;
    let bar_index = u32(clamp(floor(cell_x), 0.0, num_bars - 1.0));
    let local_x = fract(cell_x);

    let normalized_height = clamp(get_level(bar_index), 0.0, 1.0);

    let bar_width = (1.0 - gap_ratio) / num_bars;
    let half_width = bar_width * 0.5;
    let half_height = (normalized_height * 0.95) * 0.5;

    let bar_center = vec2<f32>(
        (f32(bar_index) + 0.5) / num_bars,
        half_height + 0.025
    );

    let dist = sdf_box(uv - bar_center, vec2<f32>(half_width, half_height));

    let smoothing = fwidth(dist);
    let alpha = 1.0 - smoothstep(-smoothing, smoothing, dist);

    return vec4<f32>(imm.primary_col.rgb, imm.primary_col.a * alpha);
}