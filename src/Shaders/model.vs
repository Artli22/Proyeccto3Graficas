#version 330
in vec3 vertexPosition;
in vec4 vertexColor;
uniform mat4 mvp;
uniform vec2 angles;
out vec3 position;
out vec4 color;
void main() {
    float sy = sin(angles.x), cy = cos(angles.x);
    float sp = sin(angles.y), cp = cos(angles.y);
    vec3 p = vertexPosition;
    p = vec3(p.x*cy + p.z*sy, p.y, -p.x*sy + p.z*cy);
    p = vec3(p.x, p.y*cp - p.z*sp, p.y*sp + p.z*cp);
    position = p;
    color = vertexColor;
    // Coordenadas homogeneas: no dividir por w aqui.
    // GPU: assembly -> clipping de los seis planos -> division -> viewport.
    gl_Position = mvp * vec4(p, 1.0);
}
