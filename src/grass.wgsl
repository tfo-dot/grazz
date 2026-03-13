struct Uniforms {
    time: f32,
    mower_x: f32,
    growth_factor: f32,
    _padding: f32,
};
@group(0) @binding(0) var<uniform> uniforms: Uniforms;

// Input from our Rust Vertex struct
struct VertexInput {
    @location(0) position: vec2<f32>,
};

// Input from our Rust Instance struct
struct InstanceInput {
    @location(1) offset: vec2<f32>,
    @location(2) scale: vec2<f32>,
    @location(3) sway_phase: f32,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv_y: f32, // Pass Y position to fragment shader for gradients
};

@vertex
fn vs_main(model: VertexInput, instance: InstanceInput) -> VertexOutput {
    var pos = model.position;
    
    // 1. Determine the height of this specific blade
    var current_height_multiplier = uniforms.growth_factor;
    
    // If the mower is sweeping and has passed this blade's X offset, cut it!
    if (uniforms.mower_x > -1.9 && instance.offset.x < uniforms.mower_x) {
        current_height_multiplier = 0.2;
    }

    // 2. Calculate the wind sway (Shorter grass sways less)
    let sway = sin(uniforms.time * 2.0 + instance.sway_phase) * 0.05 * pos.y * current_height_multiplier;
    pos.x += sway;

    // 3. Apply the scale, factoring in the growth/cut
    pos.x = (pos.x * instance.scale.x) + instance.offset.x;
    pos.y = (pos.y * instance.scale.y * current_height_multiplier) + instance.offset.y;

    var out: VertexOutput;
    out.clip_position = vec4<f32>(pos, 0.0, 1.0);
    out.uv_y = model.position.y; 
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Simple gradient: darker green at the root, lighter green at the tip
    let dark_green = vec3<f32>(0.05, 0.3, 0.05);
    let light_green = vec3<f32>(0.3, 0.8, 0.2);
    
    let final_color = mix(dark_green, light_green, in.uv_y);
    
    return vec4<f32>(final_color, 1.0);
}