// Fog's band (M2 spec 2.6.3): mist on the floor, soft at its two edges and
// drifting slowly along the band. Drawn in arena units from the fragment's
// world position, so the wisps keep their size whatever the band's width.

import bevy_sprite_render::mesh2d::vertex_output::VertexOutput;
import bevy_sprite_render::mesh2d::view_bindings::globals;

// The mist where it is thickest; alpha is how much of the floor it covers.
@group(constants::MATERIAL_BIND_GROUP) @binding(0) var<uniform> color: vec4<f32>;
// Half the band's width, in arena units. The band is centred on x = 0.
@group(constants::MATERIAL_BIND_GROUP) @binding(1) var<uniform> half_width: f32;

// How far inside an edge the mist reaches its full density, in arena units:
// about a ball across, so a ball is lost as it goes in.
const EDGE: f32 = 2.0;
// The size of the large wisps and of the small ones, in arena units.
const LARGE_WISP: f32 = 14.0;
const SMALL_WISP: f32 = 5.0;
// How fast the large wisps drift along the band, in arena units a second.
// Slow: nothing in the mist changes faster than once in several seconds.
// The drift runs on the renderer's clock, so it goes on through a pause,
// and starts over once an hour, when that clock wraps.
const DRIFT: f32 = 2.5;
// The mist's density in its thinnest wisp, as a share of its thickest.
const THINNEST: f32 = 0.3;

// A number in 0..1 for each whole-numbered cell, the same every frame.
fn cell(at: vec2<f32>) -> f32 {
    return fract(sin(dot(at, vec2<f32>(127.1, 311.7))) * 43758.5453);
}

// Smooth noise in 0..1: the cells' numbers, eased between neighbours.
fn wisps(at: vec2<f32>) -> f32 {
    let corner = floor(at);
    let within = fract(at);
    let eased = within * within * (3.0 - 2.0 * within);
    let below = mix(cell(corner), cell(corner + vec2<f32>(1.0, 0.0)), eased.x);
    let above = mix(
        cell(corner + vec2<f32>(0.0, 1.0)),
        cell(corner + vec2<f32>(1.0, 1.0)),
        eased.x,
    );
    return mix(below, above, eased.y);
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let at = in.world_position.xy;
    let inside_the_edge = half_width - abs(at.x);
    let edge = smoothstep(0.0, min(EDGE, half_width), inside_the_edge);
    let drift = vec2<f32>(0.0, globals.time * DRIFT);
    let large = wisps((at + drift) / LARGE_WISP);
    let small = wisps((at - 1.7 * drift) / SMALL_WISP);
    let density = mix(THINNEST, 1.0, mix(large, small, 0.3));
    return vec4<f32>(color.rgb, color.a * edge * density);
}
