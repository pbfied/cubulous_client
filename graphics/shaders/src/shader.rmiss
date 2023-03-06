#version 460
#extension GL_EXT_ray_tracing : require

layout(location = 0) rayPayloadInEXT vec3 hitValue;
layout( push_constant ) uniform constants {
    vec4 clear_color;
} pcs;

void main()
{
    hitValue = pcs.clear_color.xyz;
//    hitValue = vec3(0.3, 0.3, 0.3);
}