#version 150
in vec3 position;
in vec3 vColor;
out vec3 fColor;

void main() {
    fColor = vColor;
    gl_Position = vec4(position, 1.0);
}