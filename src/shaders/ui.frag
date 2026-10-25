in vec2 v_local;
in vec2 v_uv;
in vec2 v_px;
flat in vec4 v_clip;
flat in vec2 v_half;
flat in vec4 v_fill;
flat in vec4 v_border;
flat in vec4 v_glow;
flat in vec4 v_extra;
flat in vec4 v_p0;
flat in vec4 v_p1;
flat in vec4 v_p2;
uniform sampler2D u_atlas;
uniform sampler2D u_image;
uniform sampler2D u_fx;
uniform float u_time;
uniform float u_scale;
out vec4 o;

float box(vec2 p, vec2 b, float r) {
    vec2 q = abs(p) - b + r;
    return length(max(q, 0.0)) + min(max(q.x, q.y), 0.0) - r;
}

void main() {
    if (v_px.x < v_clip.x || v_px.y < v_clip.y || v_px.x > v_clip.z || v_px.y > v_clip.w) discard;
    int kind = int(v_p0.w + 0.5);
    if (kind == 1) {
        float c = texture(u_atlas, v_uv).r;
        float lum = dot(v_fill.rgb / max(v_fill.a, 0.001), vec3(0.2126, 0.7152, 0.0722));
        float a = lum > 0.25 ? pow(c, 1.0 / 1.55) : 1.0 - pow(1.0 - c, 1.0 / 1.3);
        o = v_fill * a;
        return;
    }
    if (kind == 2) { o = texture(u_image, v_uv) * v_fill.a; return; }
    if (kind == 3) { o = v_fill * texture(u_fx, v_uv).r; return; }

    float r = v_p0.x, bw = v_p0.y, gr = v_p0.z;
    float d = box(v_local, v_half, r);
    float aa = 0.8;

    if (kind == 4) {
        float rad = length(v_local);
        float ring = abs(rad - (v_half.x - bw * 0.5)) - bw * 0.5;
        float a = atan(v_local.y, v_local.x) - v_p1.x;
        a = mod(a, 6.28318);
        float arc = smoothstep(0.0, 0.25, a) * (1.0 - smoothstep(v_p1.y - 0.25, v_p1.y, a));
        o = v_fill * (1.0 - smoothstep(-aa, aa, ring)) * arc;
        return;
    }

    float inside = 1.0 - smoothstep(-aa, aa, d);
    vec4 fill = v_fill;

    float fx = (v_local.x + v_half.x) / max(2.0 * v_half.x, 1.0);
    if (v_p1.y > 0.0) {
        float edge = 1.0 / max(2.0 * v_half.x, 1.0);
        float m = 1.0 - smoothstep(v_p1.y - edge, v_p1.y + edge, fx);
        vec4 pc = v_extra;
        if (v_p1.z > 0.0) {
            float s = fract((v_local.x + v_local.y - v_p1.z * 30.0 * u_scale) / (18.0 * u_scale));
            pc.rgb += pc.a * 0.10 * smoothstep(0.45, 0.55, s);
        }
        fill = mix(fill, pc, m);
    }
    if (v_p1.x > -1.0) {
        float band = (v_local.x - v_local.y * 0.6) / max(2.0 * v_half.x, 1.0) + 0.5;
        float s = exp(-pow((band - v_p1.x) * 7.0, 2.0));
        fill.rgb += vec3(1.0) * 0.22 * s * fill.a;
    }
    if (v_p2.w > 0.0) {
        float rd = length(v_local - v_p2.xy);
        float ring = 1.0 - smoothstep(v_p2.z - 1.5, v_p2.z, rd);
        fill.rgb += vec3(1.0) * v_p2.w * ring * fill.a;
    }

    vec4 col = fill * inside;
    if (bw > 0.0) {
        float b = 1.0 - smoothstep(-aa, aa, abs(d + bw * 0.5) - bw * 0.5);
        col = mix(col, v_border, b * v_border.a > 0.0 ? b : 0.0);
    }
    if (gr > 0.0 && d > -1.0) {
        float g = exp(-pow(max(d, 0.0) / gr, 2.0) * 2.5) * (1.0 - inside);
        col += v_glow * g;
    }
    o = col;
}
