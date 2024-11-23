#version 330 core
layout (location = 0) in vec4 vertext; // position, textCoords
out vec2 TextCoord;

void main() {
    gl_Position = vec4(vertex.xy, 0.0, 1.0);
    TextCoord = vertex.zw; // Pass texture coordinates
}
