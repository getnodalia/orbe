// SPDX-License-Identifier: MIT
// Orbe example project "Lago sereno": a valley carved from a noise texture. The centre is a basin
// (the lake sits at y = 0) and the ground rises into mountains towards the edges.
shader_type spatial;

uniform sampler2D relieve : filter_linear, repeat_disable;
uniform float tamano = 400.0;
uniform float altura = 70.0;
uniform float radio_lago = 80.0;

uniform vec3 color_orilla : source_color = vec3(0.36, 0.31, 0.30);
uniform vec3 color_hierba : source_color = vec3(0.12, 0.20, 0.20);
uniform vec3 color_roca : source_color = vec3(0.20, 0.18, 0.30);
uniform vec3 color_nieve : source_color = vec3(0.95, 0.86, 0.88);

varying float h;
varying float pendiente;

float altura_en(vec2 p) {
	float ruido = texture(relieve, p / tamano + 0.5).r;
	float d = length(p);
	float subida = smoothstep(radio_lago, tamano * 0.46, d);
	float montana = pow(ruido, 1.6) * altura * 1.4 * subida * subida;
	return mix(-5.0, 1.2, smoothstep(0.0, radio_lago + 18.0, d)) + montana;
}

void vertex() {
	vec2 p = VERTEX.xz;
	h = altura_en(p);
	VERTEX.y = h;
	float e = 1.2;
	NORMAL = normalize(vec3(altura_en(p - vec2(e, 0.0)) - altura_en(p + vec2(e, 0.0)), 2.0 * e,
			altura_en(p - vec2(0.0, e)) - altura_en(p + vec2(0.0, e))));
	pendiente = 1.0 - NORMAL.y;
}

void fragment() {
	vec3 suelo = mix(color_orilla, color_hierba, smoothstep(0.5, 5.0, h));
	suelo = mix(suelo, color_roca, smoothstep(6.0, 24.0, h) * 0.85 + smoothstep(0.12, 0.4, pendiente) * 0.5);
	float nieve = smoothstep(40.0, 56.0, h - pendiente * 40.0);
	suelo = mix(suelo, color_nieve, nieve);
	ALBEDO = suelo;
	ROUGHNESS = 0.92;
}
