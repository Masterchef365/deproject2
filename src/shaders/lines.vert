in vec3 v_pos;
in vec3 v_color;
out vec4 f_color;

uniform mat4 u_view;
uniform mat4 u_projection;
uniform vec2 u_spread;
uniform float u_ptsize;

void main() {
    vec3 pos = v_pos;
    pos.z = (pos.z - u_spread.x) * u_spread.y + u_spread.x;
    gl_Position = u_projection * u_view * vec4(pos, 1.);
    gl_PointSize = u_ptsize;
    f_color = vec4(v_color, 1.);
}

