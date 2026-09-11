

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
    levels: array<vec4<f32>, 128>,
    col: vec4<f32>,
    params: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> imm: BufferVisUniforms;

fn smin(d1: f32, d2: f32, k: f32) -> f32 {
    let h = clamp(0.5 + 0.5 * (d2 - d1) / k, 0.0, 1.0);
    return mix(d2, d1, h) - k * h * (1.0 - h);
}
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
fn fs_main(@location(0) uv: vec2<f32>) -> @location(0) vec4<f32> {
    let num_bars = 512.0;

    let current_bar = i32(floor(uv.x * num_bars));

    var dist = 1e5;

    for (var offset: i32 = -1; offset <= 1; offset += 1) {
        let bar_idx = clamp(current_bar + offset, 0, 511);
        let u_bar_idx = u32(bar_idx);

        let height = clamp(get_level(u_bar_idx), 0.0, 1.0);

        let center_x = (f32(u_bar_idx) + 0.5) / num_bars;
        let bar_p = uv - vec2<f32>(center_x, 0.5);

        let half_width = (1.0 / num_bars) * 0.5;
        let half_height = (height * 0.95) * 0.5;

        dist = smin(dist, sdf_box(bar_p, vec2<f32>(half_width, half_height)), 0.01);
    }

    let fw = max(fwidth(dist), 0.0005);

    let mask = 1.0 - smoothstep(-fw, fw, dist);

    let a = 0.5 * mask;
    let rgb = imm.col.rgb * a;

    return vec4<f32>(rgb, a);
}
