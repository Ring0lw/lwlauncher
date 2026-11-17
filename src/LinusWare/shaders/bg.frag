uniform vec2 u_res;
uniform float u_scale;
uniform float u_time;
uniform vec2 u_mouse;
uniform float u_motion;
uniform vec4 u_logo;
out vec4 o;

const float SP = 22.0;
const vec3 BG = vec3(0.0196, 0.0588, 0.0314);
const vec3 LINE = vec3(0.022, 0.27, 0.11);
const vec3 GLINT = vec3(0.30, 1.0, 0.58);

float h11(float n) { return fract(sin(n * 127.1 + 31.7) * 43758.5453); }
float h21(vec2 p) { return fract(sin(dot(p, vec2(12.9898, 78.233))) * 43758.5453); }

float lineY(float k, float x, float t) {
    float a = h11(k), b = h11(k + 17.0), c = h11(k + 41.0);
    float y = k * SP + (a - 0.5) * 7.0;
    y += sin(x * (0.0055 + 0.004 * b) + t * (0.22 + 0.18 * c) + a * 6.283) * (3.5 + 5.0 * b);
    y += sin(x * (0.012 + 0.006 * a) - t * (0.15 + 0.12 * b) + c * 6.283) * (1.5 + 2.5 * c);
    vec2 m = u_mouse - vec2(x, y);
    float f = exp(-dot(m, m) / (2.0 * 62.0 * 62.0));
    float s = m.y / sqrt(m.y * m.y + 16.0 * 16.0);
    y -= s * f * 13.0 * u_motion;
    return y;
}

void main() {
    vec2 p = vec2(gl_FragCoord.x, u_res.y - gl_FragCoord.y) / u_scale;
    float W = u_res.x / u_scale;
    float t = u_time;
    vec3 col = BG;

    vec2 lc = p - u_logo.xy;
    col += vec3(0.005, 0.035, 0.014) * u_logo.w * exp(-dot(lc, lc) / (2.0 * 80.0 * 80.0));

    float aa = 0.9 / u_scale;
    float k0 = floor(p.y / SP);
    float lines = 0.0;
    float glint = 0.0;
    for (int j = -2; j <= 2; j++) {
        float k = k0 + float(j);
        float y = lineY(k, p.x, t);
        float dy = (lineY(k, p.x + 1.0, t) - lineY(k, p.x - 1.0, t)) * 0.5;
        float d = abs(p.y - y) / sqrt(1.0 + dy * dy);
        float cov = 1.0 - smoothstep(0.55 - aa, 0.55 + aa, d);
        vec2 m = u_mouse - vec2(p.x, y);
        float near = exp(-dot(m, m) / (2.0 * 80.0 * 80.0)) * u_motion;
        float inten = (0.35 + 0.65 * h11(k + 5.0)) * (1.0 + 0.9 * near);
        lines = max(lines, cov * inten);

        float span = W + 360.0;
        float speed = 45.0 + 70.0 * h11(k + 9.0);
        float ph = t * speed / span + h11(k + 3.0);
        float on = step(0.62, h11(k * 3.1 + floor(ph) * 7.7));
        float gx = fract(ph) * span - 180.0;
        float dx = gx - p.x;
        float trail = dx > 0.0 ? pow(max(1.0 - dx / 130.0, 0.0), 2.0) : exp(-dx * dx / 18.0);
        float halo = exp(-d * d / 10.0) * exp(-dx * dx / 900.0);
        glint += on * (cov * trail + 0.15 * halo);
    }
    col = mix(col, LINE, clamp(lines, 0.0, 1.0));
    col += GLINT * clamp(glint, 0.0, 1.0) * 0.45;

    vec2 uv = p / vec2(W, u_res.y / u_scale) - 0.5;
    col *= 1.0 - 0.35 * smoothstep(0.35, 0.85, length(uv * vec2(1.1, 1.3)));
    col += (h21(gl_FragCoord.xy + fract(t) * 91.0) - 0.5) / 255.0;
    o = vec4(col, 1.0);
}
