// canvas_v2/shaders.wgsl.rs —— CanvasV2 单管线着色器（WGSL 源）。
//
// 参任务 c1-canvas-core 设计 1：一个实例流 + 一条管线，片元按 kind 求有符号距离
// d（像素），覆盖率 cov = clamp(0.5 − d, 0, 1)，去 MSAA。颜色语义与三套旧着色器
// 逐字一致：SOLID_SHADER 的 sRGB→线性 + 预乘输出、TEXT_SHADER 的浓度补偿
// （与 CPU `TextRenderTuning::tune_coverage` 同公式，测试 `gpu_formula_matches_cpu_lut`
// 守住）、IMAGE_SHADER 的 tint / opacity 规则。
// C1.5 增量重画新增 kind 6（清除实例，走无混合管线）与 blit_vs / blit_fs（target 上屏）。
// 描边语义核对（任务书要求）：`triangulate_stroke` 为**向内**描边（外沿 = 矩形边界，
// 内沿 = 内缩 thickness），故 sd_stroke 用 max(d, −(d+t)) 原样成立；圆弧
// `triangulate_arc` 的 radius 是**中心线**半径（内外各 t/2），故环带以 R 为中心线，
// 与任务书 sd_ring 草案（R − t/2 为中心）不同，以现有语义为准（报告已记）。
// 采样：字形 / 图片走 `textureSampleLevel(…, 0)` —— 图集无 mip，1:1 采样与
// `textureSample` 逐 texel 一致，且避免 instance 输入上的非 uniform 控制流校验错误。

pub const CANVAS2_SHADER: &str = r#"
fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    let low = c / vec3<f32>(12.92);
    let high = pow((c + vec3<f32>(0.055)) / vec3<f32>(1.055), vec3<f32>(2.4));
    return select(high, low, c <= vec3<f32>(0.04045));
}
// 圆角矩形 SDF：p 相对矩形中心（物理像素），half = 半宽高，r = 圆角半径。
fn sd_rrect(p: vec2<f32>, half: vec2<f32>, r: f32) -> f32 {
    let q = abs(p) - (half - vec2<f32>(r));
    return length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - r;
}
// 向内描边：带内 [-t, 0]（外沿 = 矩形边界，与 triangulate_stroke 一致）。
fn sd_stroke(p: vec2<f32>, half: vec2<f32>, r: f32, t: f32) -> f32 {
    let d = sd_rrect(p, half, r);
    return max(d, -(d + t));
}
// 圆弧环（中心线半径 R、厚 t、张角 a0→a1 弧度）。角度系：0 = 正上、顺时针，
// 点角度 phi = atan2(p.x, -p.y)（与 triangulate_arc 的 (sin, -cos) 映射一致）。
// 张角 >= 2π 退化为纯环；否则环带与「两条半径射线夹的楔形」相交。
fn sd_arc(p: vec2<f32>, a0: f32, a1: f32, big_r: f32, t: f32) -> f32 {
    let two_pi = 6.283185307179586;
    let sw = abs(a1 - a0);
    let ring = abs(length(p) - big_r) - 0.5 * t;
    if (sw >= two_pi - 1e-3) {
        return ring;
    }
    let m = 0.5 * (a0 + a1);
    let h = 0.5 * sw;
    let phi = atan2(p.x, -p.y);
    // 归一化角差到 [-π, π)。
    var dphi = phi - m;
    dphi = dphi - two_pi * round(dphi / two_pi);
    if (abs(dphi) <= h) {
        return ring;
    }
    // 楔形外：到弧两端点（圆盘）的距离。
    let e0 = big_r * vec2<f32>(sin(a0), -cos(a0));
    let e1 = big_r * vec2<f32>(sin(a1), -cos(a1));
    return min(length(p - e0), length(p - e1)) - 0.5 * t;
}
// 三角形 SDF（Inigo Quilez sdTriangle，外正内负）。
fn sd_triangle(p: vec2<f32>, p0: vec2<f32>, p1: vec2<f32>, p2: vec2<f32>) -> f32 {
    let e0 = p1 - p0;
    let e1 = p2 - p1;
    let e2 = p0 - p2;
    let v0 = p - p0;
    let v1 = p - p1;
    let v2 = p - p2;
    let pq0 = v0 - e0 * clamp(dot(v0, e0) / dot(e0, e0), 0.0, 1.0);
    let pq1 = v1 - e1 * clamp(dot(v1, e1) / dot(e1, e1), 0.0, 1.0);
    let pq2 = v2 - e2 * clamp(dot(v2, e2) / dot(e2, e2), 0.0, 1.0);
    let s = sign(e0.x * e2.y - e0.y * e2.x);
    let d0 = vec2<f32>(dot(pq0, pq0), s * (pq0.x * e0.y - pq0.y * e0.x));
    let d1 = vec2<f32>(dot(pq1, pq1), s * (pq1.x * e1.y - pq1.y * e1.x));
    let d2 = vec2<f32>(dot(pq2, pq2), s * (pq2.x * e2.y - pq2.y * e2.x));
    let d = min(min(d0, d1), d2);
    return -sqrt(d.x) * sign(d.y);
}

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) kf: vec2<u32>,
    @location(1) rect: vec4<f32>,
    @location(2) p: vec4<f32>,
    @location(3) q: vec4<f32>,
    @location(4) color: vec4<f32>,
    @location(5) clip: vec4<f32>,
};
// 绑定组：整帧只绑一次。字形图集（R8）+ 图片图集（RGBA8 sRGB）+ 采样器 +
// uniform (W, H, contrast, gamma)。独立纹理图片复用同布局的另一个绑定组
// （binding 0/1 挂 1×1 空纹理、binding 1 挂独立纹理），shader 无感知。
@group(0) @binding(0) var glyph_tex: texture_2d<f32>;
@group(0) @binding(1) var img_tex: texture_2d<f32>;
@group(0) @binding(2) var samp: sampler;
@group(0) @binding(3) var<uniform> u: vec4<f32>;

@vertex
fn vs(
    @builtin(vertex_index) vi: u32,
    @location(0) kf: vec2<u32>,
    @location(1) rect: vec4<f32>,
    @location(2) p: vec4<f32>,
    @location(3) q: vec4<f32>,
    @location(4) color: vec4<f32>,
    @location(5) clip: vec4<f32>,
) -> VsOut {
    // 六顶点展开（与旧 push_quad 同序：00,10,11 / 00,11,01）。naga 不允许
    // 局部数组运行时索引，用算术展开：t = 三角形序，i = 三角形内顶点序。
    let t = vi / 3u;
    let i = vi % 3u;
    // 顶点表：t=0 三角形 x=(0,1,1) y=(0,0,1)；t=1 三角形 x=(0,1,0) y=(0,1,1)。
    let c = vec2<f32>(
        select(f32(min(i, 1u)), f32(select(0u, 1u, i == 1u)), t == 1u),
        select(f32(i / 2u), f32(min(i, 1u)), t == 1u),
    );
    let x0 = rect.x - 1.0;
    let y0 = rect.y - 1.0;
    let x1 = rect.x + rect.z + 1.0;
    let y1 = rect.y + rect.w + 1.0;
    let px = mix(x0, x1, c.x);
    let py = mix(y0, y1, c.y);
    var out: VsOut;
    out.pos = vec4<f32>((px / u.x) * 2.0 - 1.0, 1.0 - (py / u.y) * 2.0, 0.0, 1.0);
    out.kf = kf;
    out.rect = rect;
    out.p = p;
    out.q = q;
    out.color = color;
    out.clip = clip;
    return out;
}

@fragment
fn fs(in: VsOut) -> @location(0) vec4<f32> {
    // 硬裁剪：像素中心在裁剪矩形外 → discard（等价旧 scissor 语义；clip 已在 CPU 侧
    // 按 scissor_rect 同一 floor/ceil 量化成整数物理像素边界）。
    if (
        in.pos.x < in.clip.x || in.pos.y < in.clip.y
            || in.pos.x >= in.clip.z || in.pos.y >= in.clip.w
    ) {
        discard;
    }
    let kind = in.kf.x;
    var cov = 0.0;
    if (kind == 4u) {
        // 字形：图集 R8 覆盖率 + 浓度补偿（与 TEXT_SHADER 同公式，contrast / gamma 来自 u.zw）。
        // uv 按未外扩 rect 归一化：1:1 时像素中心精确落在 texel 中心；rect 外扩环带
        // 落在图集 1 px 空隙（覆盖率 0），正是字形位图外的自然过渡。
        var uv = in.q.xy + (in.q.zw - in.q.xy) * vec2<f32>(
            (in.pos.x - in.rect.x) / in.rect.z,
            (in.pos.y - in.rect.y) / in.rect.w,
        );
        cov = textureSampleLevel(glyph_tex, samp, uv, 0.0).r;
        if (u.z != 0.0 || u.w != 1.0) {
            let luma = clamp(
                0.2126 * in.color.r + 0.7152 * in.color.g + 0.0722 * in.color.b, 0.0, 1.0,
            );
            let gamma_eff = max(u.w * (1.0 + 0.30 * (luma - 0.5)), 0.05);
            let g = pow(cov, 1.0 / gamma_eff);
            cov = clamp(0.5 + (g - 0.5) * (1.0 + u.z), 0.0, 1.0);
        }
    } else if (kind == 5u) {
        // 图片：RGBA 图集 / 独立纹理，tint 与 opacity 规则与 IMAGE_SHADER 逐字一致。
        var uv = in.q.xy + (in.q.zw - in.q.xy) * vec2<f32>(
            (in.pos.x - in.rect.x) / in.rect.z,
            (in.pos.y - in.rect.y) / in.rect.w,
        );
        let src = textureSampleLevel(img_tex, samp, uv, 0.0);
        let is_tint = in.color.r != 1.0 || in.color.g != 1.0 || in.color.b != 1.0;
        let rgb = select(src.rgb * src.a, srgb_to_linear(in.color.rgb) * src.a, is_tint);
        return vec4<f32>(rgb * in.color.a, src.a * in.color.a);
    } else if (kind == 0u) {
        // 填充圆角矩形。
        let half = vec2<f32>(in.rect.z, in.rect.w) * 0.5;
        let ctr = in.rect.xy + half;
        cov = clamp(0.5 - sd_rrect(in.pos.xy - ctr, half, in.p.x), 0.0, 1.0);
    } else if (kind == 1u) {
        // 向内描边圆角矩形。
        let half = vec2<f32>(in.rect.z, in.rect.w) * 0.5;
        let ctr = in.rect.xy + half;
        cov = clamp(0.5 - sd_stroke(in.pos.xy - ctr, half, in.p.x, in.p.y), 0.0, 1.0);
    } else if (kind == 2u) {
        // 圆弧环（中心在 p.xy、中心线半径 p.z、厚 p.w；起止角 q.xy 弧度）。
        cov = clamp(0.5 - sd_arc(in.pos.xy - in.p.xy, in.q.x, in.q.y, in.p.z, in.p.w), 0.0, 1.0);
    } else if (kind == 6u) {
        // C1.5 增量清除实例：覆盖损伤矩形 D，片元恒输出 0。
        // 该 kind 走 `pipeline_replace`（blend: None）——直接写入 vec4(0) 而不与旧值混合，
        // 等价于「只在 D 内做 LoadOp::Clear」；scissor 已限到 D，故只清 D。
        return vec4<f32>(0.0);
    } else {
        // 三角形（p = p0.xy p1.xy，q.x/q.y = p2.xy；物理像素）。
        let p0 = vec2<f32>(in.p.x, in.p.y);
        let p1 = vec2<f32>(in.p.z, in.p.w);
        let p2 = vec2<f32>(in.q.x, in.q.y);
        cov = clamp(0.5 - sd_triangle(in.pos.xy, p0, p1, p2), 0.0, 1.0);
    }
    return vec4<f32>(srgb_to_linear(in.color.rgb) * in.color.a * cov, in.color.a * cov);
}

// C1.5：blit 呈现路径 —— 交换链不支持 COPY_DST（无法 copy_texture_to_texture）时，
// 用全屏三角形采样常驻 target 上屏。复用 binding 0（此处语义为 target 纹理），
// 采样器仍是 binding 2；blit 绑定组由 CPU 侧构造（见 canvas_v2.rs `make_blit_bind_group`）。
struct BlitOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};
@vertex
fn blit_vs(@builtin(vertex_index) vi: u32) -> BlitOut {
    // 全屏三角形：vi=0 → (0,0) 左上，vi=1 → (2,0)，vi=2 → (0,2)。
    let x = f32((vi << 1u) & 2u);
    let y = f32(vi & 2u);
    var out: BlitOut;
    out.pos = vec4<f32>(x * 2.0 - 1.0, 1.0 - y * 2.0, 0.0, 1.0);
    out.uv = vec2<f32>(x, y);
    return out;
}
@fragment
fn blit_fs(in: BlitOut) -> @location(0) vec4<f32> {
    // target 与交换链同格式（sRGB）：采样解码 → 写入再编码，A/B 两档走同一往返，可逐位比较。
    return textureSampleLevel(glyph_tex, samp, in.uv, 0.0);
}
"#;
