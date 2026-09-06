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
    levels: array<f32, 32>,
    primary_col: vec4<f32>
};

var<immediate> imm: SpectrumUniforms;

@fragment
fn fs_main(@location(0) frag_position: vec2<f32>) -> @location(0) vec4<f32> {
    return imm.primary_col;
}
