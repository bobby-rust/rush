#version 330 core

out vec4 FragColor;

in vec2 TexCoords;

uniform sampler2D text;

void main() {
    //vec4 sampled = texture(text, TexCoords);
    //FragColor = vec4(sampled.r, sampled.r, sampled.r, 1.0);
    FragColor = vec4(1.0, 1.0, 1.0, 1.0);
}
