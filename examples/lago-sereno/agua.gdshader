// SPDX-License-Identifier: MIT
// Orbe example project "Lago sereno": still, dark water with faint ripples that mirrors the sky.
shader_type spatial;
render_mode specular_schlick_ggx;

uniform sampler2D ondas : source_color, filter_linear_mipmap, repeat_enable;
uniform vec3 color_profundo : source_color = vec3(0.07, 0.17, 0.30);

void fragment() {
	vec2 uv = UV * 14.0;
	vec3 a = texture(ondas, uv + vec2(TIME * 0.006, TIME * 0.003)).rgb;
	vec3 b = texture(ondas, uv * 1.7 - vec2(TIME * 0.004, 0.0)).rgb;
	NORMAL_MAP = normalize((a + b) * 0.5);
	NORMAL_MAP_DEPTH = 0.12;
	ALBEDO = color_profundo;
	ROUGHNESS = 0.03;
	SPECULAR = 0.95;
	ALPHA = 0.93;
}
