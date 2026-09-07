struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) frag_position: vec2<f32>,
};

@vertex
fn vs_main(
    @builtin(vertex_index) vertex_index: u32
) -> VertexOutput {
    var output: VertexOutput;

    // Full-screen triangle covering [-1, 1] clip space
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
    levels: array<vec4<f32>, 8>, // 32 floats packed as 8 x vec4 (fits std140/immediate alignment)
    primary_col: vec4<f32>,
};

var<immediate> imm: SpectrumUniforms;

// Helper to access flattened float index from packed vec4 array
fn get_level(index: u32) -> f32 {
    let vec_idx = index / 4u;
    let comp_idx = index % 4u;
    return imm.levels[vec_idx][comp_idx];
}

// Signed Distance Field (SDF) for a rounded box
fn sdf_rounded_box(p: vec2<f32>, b: vec2<f32>, r: f32) -> f32 {
    let q = abs(p) - b + vec2<f32>(r);
    return length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - r;
}

@fragment
fn fs_main(@location(0) frag_position: vec2<f32>) -> @location(0) vec4<f32> {
    // Map clip-space UV [-1.0, 1.0] to normalized UV [0.0, 1.0]
    let uv = frag_position * 0.5 + vec2<f32>(0.5);

    // Grid layout parameters
    let num_bars: f32 = 32.0;
    let gap_ratio: f32 = 0.2; // 20% gap between bars
    let corner_radius: f32 = 0.01;

    // Determine bar index (0..31) and local x-coordinate within the bar cell [0, 1]
    let cell_x = uv.x * num_bars;
    let bar_index = u32(clamp(floor(cell_x), 0.0, num_bars - 1.0));
    let local_x = fract(cell_x);

    let normalized_height = clamp(get_level(bar_index), 0.0, 1.0);

    // Compute center and half-extents for the rounded box SDF
    let bar_width = (1.0 - gap_ratio) / num_bars;
    let half_width = bar_width * 0.5;
    let half_height = (normalized_height * 0.95) * 0.5; // Scale height slightly to prevent clipping top

    // Center of the current bar in normalized UV space
    let bar_center = vec2<f32>(
        (f32(bar_index) + 0.5) / num_bars,
        half_height + 0.025 // Small bottom margin
    );

    // Calculate signed distance
    let dist = sdf_rounded_box(uv - bar_center, vec2<f32>(half_width, half_height), corner_radius);

    // Anti-aliased alpha blending
    let smoothing = fwidth(dist);
    let alpha = 1.0 - smoothstep(-smoothing, smoothing, dist);

    // Output primary color with SDF mask applied
    return vec4<f32>(imm.primary_col.rgb, imm.primary_col.a * alpha);
}