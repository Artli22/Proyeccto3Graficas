#version 330
in vec3 position;
in vec4 color;
uniform vec3 eye;
uniform vec3 engineLight;
uniform float enginePower;
out vec4 finalColor;
void main() {
    // Derivadas de la posicion interpolada: normal plana de cada cara.
    vec3 n = normalize(cross(dFdx(position), dFdy(position)));
    vec3 v = normalize(eye - position);
    if (dot(n,v) < 0.0) n = -n;
    vec3 l = normalize(vec3(-0.6, 0.7, 0.5));
    float ndl = max(dot(n,l), 0.0);
    float diffuse = 0.30 + 0.70*ndl;
    float specular = ndl > 0.0 ? 0.85*pow(max(dot(n,normalize(l+v)),0.0),40.0) : 0.0;
    vec3 delta = engineLight-position;
    float distanceToLight = length(delta);
    if (distanceToLight > 0.0001 && distanceToLight < 9.0)
        diffuse = min(1.0, diffuse + max(dot(n,delta/distanceToLight),0.0)*enginePower*0.6/(1.0+distanceToLight*distanceToLight*0.05));
    vec3 rgb = clamp(color.rgb*diffuse + vec3(specular),0.0,1.0);
    if (color.r > 0.99 && color.g < 0.01 && color.b > 0.99) rgb = color.rgb;
    finalColor = vec4(rgb, color.a);
}
