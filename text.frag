#version 330 core
in vec2 TextCoord;
out vec4 FragColor;

uniform sampler2D text;

void main() {
    vec4 color = texture(text, TextCoord);
    if (color.a < 0.1)
        discard;
    FragColor = color;
}
