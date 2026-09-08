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

struct DecayVisUniforms {
    feedback: vec2<f32>,
    time_s: vec2<f32>,
    flags: vec4<f32>,
    color_primary: vec4<f32>,
    color_secondary: vec4<f32>,
    grid: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> imm: DecayVisUniforms;

const COLOR_PRIMARY_MUTED = vec3<f32>(219.0 / 255.0, 168.0 / 255.0, 0.0);
const COLOR_SECONDARY_MUTED = vec3<f32>(0.0, 53.0 / 255.0, 102.0 / 255.0);

fn sdf_box(p: vec2<f32>, b: vec2<f32>) -> f32 {
    let d = abs(p) - b;
    return length(max(d, vec2<f32>(0.0))) + min(max(d.x, d.y), 0.0);
}

/// Composite a single opaque bar onto the accumulating color/alpha (alpha-over).
fn add_bar(
    uv: vec2<f32>,
    center_x: f32,
    y_center: f32,
    half_width: f32,
    h: f32,
    alpha: f32,
    color: vec3<f32>,
    acc_rgb: ptr<function, vec3<f32>>,
    acc_a: ptr<function, f32>
) {
    if (h <= 0.0) {
        return;
    }
    let d = sdf_box(uv - vec2<f32>(center_x, y_center), vec2<f32>(half_width, h * 0.5));
    let mask = 1.0 - smoothstep(-fwidth(d), fwidth(d), d);
    let add_a = mask * alpha;

    let new_a = add_a + *acc_a * (1.0 - add_a);
    if (new_a > 0.0) {
        var rgb = (*acc_rgb * *acc_a * (1.0 - add_a) + color * add_a) / new_a;
        if (add_a <= 0.0) {
            rgb = *acc_rgb;
        }
        *acc_rgb = rgb;
    }
    *acc_a = new_a;
}

@fragment
fn fs_main(@location(0) frag_position: vec2<f32>) -> @location(0) vec4<f32> {
    let uv = frag_position * 0.5 + vec2<f32>(0.5);

    let visible_seconds: f32 = 3.0;
    let half_w: f32 = 0.006;

    var out_rgb = vec3<f32>(0.0);
    var out_a = 0.0;

    let is_stereo = imm.flags.x > 0.5;
    let is_ping_pong = imm.flags.y > 0.5;
    let fb_l = clamp(imm.feedback.x, 0.0, 1.0);
    let fb_r = clamp(imm.feedback.y, 0.0, 1.0);
    let ts_l = max(imm.time_s.x, 0.0);
    let ts_r = max(imm.time_s.y, 0.0);

    let bar_s = max(imm.grid.x, 0.0);
    let grid_half_w: f32 = 0.0015;
    let grid_a: f32 = 0.5;

    let grid_top_is_bar = imm.flags.z > 0.5;
    let grid_bot_is_bar = imm.flags.w > 0.5 || (!is_stereo && imm.flags.z > 0.5);

    var sp_top: f32 = 1.0;
    var sp_bot: f32 = 1.0;
    var col_top = COLOR_SECONDARY_MUTED;
    var col_bot = COLOR_SECONDARY_MUTED;
    if (grid_top_is_bar) {
        sp_top = bar_s;
        col_top = COLOR_PRIMARY_MUTED;
    }
    if (grid_bot_is_bar) {
        sp_bot = bar_s;
        col_bot = COLOR_PRIMARY_MUTED;
    }

    for (var k: u32 = 0u; k < 64u; k++) {
        let x = (f32(k) * sp_top) / visible_seconds;
        if (x > 1.0) {
            break;
        }
        add_bar(uv, x, 0.25, grid_half_w, 0.5, grid_a, col_top, &out_rgb, &out_a);
    }
    for (var k: u32 = 0u; k < 64u; k++) {
        let x = (f32(k) * sp_bot) / visible_seconds;
        if (x > 1.0) {
            break;
        }
        add_bar(uv, x, 0.75, grid_half_w, 0.5, grid_a, col_bot, &out_rgb, &out_a);
    }

    if (is_ping_pong) {
        let step = ts_l + ts_r;
        for (var i: u32 = 0u; i < 16u; i++) {
            let base = (f32(i) * step) / visible_seconds;
            if (base > 1.0) {
                break;
            }
            let x_l = base + ts_l / visible_seconds;
            let x_r = base + ts_r / visible_seconds;

            let d0 = pow(fb_l, f32(i * 2u)) * 0.5;
            let d1 = pow(fb_l, f32(i * 2u + 1u)) * 0.5;
            let d2 = pow(fb_r, f32(i * 2u)) * 0.5;
            let d3 = pow(fb_r, f32(i * 2u + 1u)) * 0.5;

            add_bar(uv, base,     0.5 + d0 * 0.5, half_w, d0, 0.8, imm.color_primary.rgb, &out_rgb, &out_a);
            add_bar(uv, x_l,      0.5 - d1 * 0.5, half_w, d1, 0.8, imm.color_primary.rgb, &out_rgb, &out_a);
            add_bar(uv, base,     0.5 - d2 * 0.5, half_w, d2, 0.8, imm.color_secondary.rgb, &out_rgb, &out_a);
            add_bar(uv, x_r,      0.5 + d3 * 0.5, half_w, d3, 0.8, imm.color_secondary.rgb, &out_rgb, &out_a);
        }
    } else {
        let num_echoes: u32 = 64u;
        for (var i: u32 = 0u; i < num_echoes; i++) {
            let x_l = (f32(i) * ts_l) / visible_seconds;
            let x_r = (f32(i) * ts_r) / visible_seconds;
            if (!is_stereo) {
                if (x_l > 1.0) {
                    break;
                }
            } else {
                if (x_r > 1.0 && x_l > 1.0) {
                    break;
                }
            }

            if (is_stereo) {
                let h_l = pow(fb_l, f32(i)) * 0.5;
                let h_r = pow(fb_r, f32(i)) * 0.5;

                add_bar(uv, x_l, 0.5 - h_l * 0.5, half_w, h_l, 0.8, imm.color_primary.rgb, &out_rgb, &out_a);
                add_bar(uv, x_r, 0.5 + h_r * 0.5, half_w, h_r, 0.8, imm.color_secondary.rgb, &out_rgb, &out_a);
            } else {
                let h_l = pow(fb_l, f32(i)) * 0.5;
                add_bar(uv, x_l, 0.5 - h_l * 0.5, half_w, h_l, 0.8, imm.color_primary.rgb, &out_rgb, &out_a);
                add_bar(uv, x_l, 0.5 + h_l * 0.5, half_w, h_l, 0.8, imm.color_secondary.rgb, &out_rgb, &out_a);
            }
        }
    }

    return vec4<f32>(out_rgb, out_a);
}
