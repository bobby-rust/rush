#version 330 core

layout (location = 0) in vec3 aPos; // position attribute
layout (location = 1) in vec3 aColor; // color attribute

out vec3 color;

void main() { 
    //gl_Position = vec4(aPos.x, -aPos.y, aPos.z, 1.0); // the same as aPos.x, aPos.y, aPos.z, 1.0
    gl_Position = vec4(aPos, 1.0);
    color = aColor;
}
