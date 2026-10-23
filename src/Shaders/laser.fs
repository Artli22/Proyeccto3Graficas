#version 330
// Perfil analitico del laser original, trasladado de laser.rs a GPU.
uniform vec4 endpoints;
uniform vec2 inverseDepth;
uniform vec2 viewport;
uniform vec2 screenSize;
uniform float power;
uniform float nearPlane;
uniform float farPlane;
out vec4 finalColor;
void main() {
    vec2 pixel = vec2(gl_FragCoord.x, viewport.y-gl_FragCoord.y)*screenSize/viewport;
    vec2 a = endpoints.xy;
    vec2 delta = endpoints.zw-a;
    float lengthSquared = dot(delta,delta);
    if (lengthSquared < 0.000001) discard;
    float t = clamp(dot(pixel-a,delta)/lengthSquared,0.0,1.0);
    float distanceToBeam = length(pixel-(a+delta*t));
    float inv = mix(inverseDepth.x,inverseDepth.y,t);
    float scale = screenSize.y*inv;
    float glowRadius = 0.85*scale;
    if (distanceToBeam >= glowRadius) discard;
    float fade = 1.0-0.5*t;
    float g = 1.0-distanceToBeam/glowRadius;
    // Misma cuantizacion a alfa entero de la implementacion CPU original.
    float ag = floor(190.0*g*g*power*fade);
    if (ag <= 3.0) ag = 0.0;
    ag /= 255.0;
    float coreRadius = max(0.16*scale,1.0);
    float ac = 0.0;
    if (distanceToBeam < coreRadius) {
        ac = floor(235.0*sqrt(1.0-distanceToBeam/coreRadius)*power*fade);
        if (ac <= 3.0) ac = 0.0;
        ac /= 255.0;
    }
    // Composicion equivalente a pintar primero halo y despues nucleo.
    float alpha = ac+ag*(1.0-ac);
    if (alpha <= 0.0) discard;
    vec3 halo = vec3(255.0,40.0,30.0)/255.0;
    vec3 core = vec3(255.0,225.0,215.0)/255.0;
    finalColor = vec4((core*ac+halo*ag*(1.0-ac))/alpha,alpha);
    gl_FragDepth = farPlane/(farPlane-nearPlane)
                - (farPlane*nearPlane/(farPlane-nearPlane))*inv;
}
