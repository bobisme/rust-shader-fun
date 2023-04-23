struct VertexOutput {
    // @location(0) color: vec4<f32>,
    @builtin(position) position: vec4<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) in_vertex_index: u32) -> VertexOutput {
    let x = f32(i32(in_vertex_index) - 1);
    let y = f32(i32(in_vertex_index & 1u) * 2 - 1);
    var result: VertexOutput;
    // result.color = color;
    // result.color = vec4<f32>(color.x, color.y, color.z, 1.0);
    result.position = vec4<f32>(x, y, 0.0, 1.0);
    return result;
}

@group(0)
@binding(0) 
var<uniform> color: vec4<f32>;

@fragment
fn fs_main(vertex: VertexOutput) -> @location(0) vec4<f32> {
    return color;
    // return vec4<f32>(color.x, color.y, color.z, color.w);
}