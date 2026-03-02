//! Software 3D renderer for Minecraft player skins.
//!
//! Takes a full 64x64 Minecraft skin texture (PNG bytes) and renders an isometric
//! 3D view of the player model. The output is a PNG image suitable for display
//! via `png_render_cache`.

use image::{GenericImageView, ImageFormat, Rgba, RgbaImage};
use std::io::Cursor;

/// Default rotation angles for the initial view.
pub const DEFAULT_YAW: f64 = 25.0;
pub const DEFAULT_PITCH: f64 = -20.0;

// ── Math types ──────────────────────────────────────────────────────────────

#[derive(Clone, Copy)]
struct V3 {
    x: f64,
    y: f64,
    z: f64,
}

impl V3 {
    fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }
}

/// 3×3 rotation matrix stored as rows.
struct Mat3([[f64; 3]; 3]);

impl Mat3 {
    /// Build a combined rotation: first rotate `ry` radians around Y, then `rx` around X.
    fn rotation_yx(ry: f64, rx: f64) -> Self {
        let (sy, cy) = ry.sin_cos();
        let (sx, cx) = rx.sin_cos();
        // Rx * Ry
        Self([[cy, 0.0, sy], [sx * sy, cx, -sx * cy], [-cx * sy, sx, cx * cy]])
    }

    fn transform(&self, v: V3) -> V3 {
        let r = &self.0;
        V3 {
            x: r[0][0] * v.x + r[0][1] * v.y + r[0][2] * v.z,
            y: r[1][0] * v.x + r[1][1] * v.y + r[1][2] * v.z,
            z: r[2][0] * v.x + r[2][1] * v.y + r[2][2] * v.z,
        }
    }
}

// ── Quad / face definitions ─────────────────────────────────────────────────

/// A textured quad (face) ready for rendering.
struct Quad {
    /// 4 corners in 3D (order: top-left, top-right, bottom-right, bottom-left as seen from outside).
    verts: [V3; 4],
    /// Absolute texture pixel coordinates for each corner.
    uvs: [(f64, f64); 4],
    /// Outward face normal (before rotation).
    normal: V3,
}

/// Generate the 6 face quads for an axis-aligned box.
///
/// - `min`/`max`: box corners in model space.
/// - `tx`, `ty`: top-left corner of this box's texture region in the skin.
/// - `w`, `h`, `d`: box width (X), height (Y), depth (Z) – must equal max-min.
fn box_quads(min: V3, max: V3, tx: f64, ty: f64, w: f64, h: f64, d: f64) -> [Quad; 6] {
    let (x0, y0, z0) = (min.x, min.y, min.z);
    let (x1, y1, z1) = (max.x, max.y, max.z);

    [
        // Front face (+Z) – texture region at (tx+d, ty+d) size w×h
        Quad {
            verts: [
                V3::new(x0, y1, z1),
                V3::new(x1, y1, z1),
                V3::new(x1, y0, z1),
                V3::new(x0, y0, z1),
            ],
            uvs: [
                (tx + d, ty + d),
                (tx + d + w, ty + d),
                (tx + d + w, ty + d + h),
                (tx + d, ty + d + h),
            ],
            normal: V3::new(0.0, 0.0, 1.0),
        },
        // Back face (-Z) – texture at (tx+2d+w, ty+d) size w×h
        Quad {
            verts: [
                V3::new(x1, y1, z0),
                V3::new(x0, y1, z0),
                V3::new(x0, y0, z0),
                V3::new(x1, y0, z0),
            ],
            uvs: [
                (tx + 2.0 * d + w, ty + d),
                (tx + 2.0 * d + 2.0 * w, ty + d),
                (tx + 2.0 * d + 2.0 * w, ty + d + h),
                (tx + 2.0 * d + w, ty + d + h),
            ],
            normal: V3::new(0.0, 0.0, -1.0),
        },
        // Right face (-X, player's right) – texture at (tx, ty+d) size d×h
        Quad {
            verts: [
                V3::new(x0, y1, z1),
                V3::new(x0, y1, z0),
                V3::new(x0, y0, z0),
                V3::new(x0, y0, z1),
            ],
            uvs: [(tx + d, ty + d), (tx, ty + d), (tx, ty + d + h), (tx + d, ty + d + h)],
            normal: V3::new(-1.0, 0.0, 0.0),
        },
        // Left face (+X, player's left) – texture at (tx+d+w, ty+d) size d×h
        Quad {
            verts: [
                V3::new(x1, y1, z0),
                V3::new(x1, y1, z1),
                V3::new(x1, y0, z1),
                V3::new(x1, y0, z0),
            ],
            uvs: [
                (tx + 2.0 * d + w, ty + d),
                (tx + d + w, ty + d),
                (tx + d + w, ty + d + h),
                (tx + 2.0 * d + w, ty + d + h),
            ],
            normal: V3::new(1.0, 0.0, 0.0),
        },
        // Top face (+Y) – texture at (tx+d, ty) size w×d
        Quad {
            verts: [
                V3::new(x0, y1, z0),
                V3::new(x1, y1, z0),
                V3::new(x1, y1, z1),
                V3::new(x0, y1, z1),
            ],
            uvs: [(tx + d, ty), (tx + d + w, ty), (tx + d + w, ty + d), (tx + d, ty + d)],
            normal: V3::new(0.0, 1.0, 0.0),
        },
        // Bottom face (-Y) – texture at (tx+d+w, ty) size w×d
        Quad {
            verts: [
                V3::new(x1, y0, z0),
                V3::new(x0, y0, z0),
                V3::new(x0, y0, z1),
                V3::new(x1, y0, z1),
            ],
            uvs: [
                (tx + d + w, ty),
                (tx + d + 2.0 * w, ty),
                (tx + d + 2.0 * w, ty + d),
                (tx + d + w, ty + d),
            ],
            normal: V3::new(0.0, -1.0, 0.0),
        },
    ]
}

// ── Body part definitions ───────────────────────────────────────────────────

struct BodyPartDef {
    min: V3,
    max: V3,
    tx: f64,
    ty: f64,
    w: f64,
    h: f64,
    d: f64,
}

/// Returns all body parts (base layer + overlay layer).
/// The overlay boxes are 0.5 units larger on each side.
fn player_model() -> Vec<BodyPartDef> {
    // Player model dimensions (units = skin pixels):
    //   Legs:  y  0..12,   4 wide, 4 deep
    //   Body:  y 12..24,   8 wide, 4 deep
    //   Arms:  y 12..24,   4 wide, 4 deep
    //   Head:  y 24..32,   8 wide, 8 deep
    //
    // Centered at x=0, z=0.

    let e = 0.5; // overlay expansion

    vec![
        // ── Base layers ──
        // Head
        BodyPartDef {
            min: V3::new(-4.0, 24.0, -4.0),
            max: V3::new(4.0, 32.0, 4.0),
            tx: 0.0,
            ty: 0.0,
            w: 8.0,
            h: 8.0,
            d: 8.0,
        },
        // Body
        BodyPartDef {
            min: V3::new(-4.0, 12.0, -2.0),
            max: V3::new(4.0, 24.0, 2.0),
            tx: 16.0,
            ty: 16.0,
            w: 8.0,
            h: 12.0,
            d: 4.0,
        },
        // Right Arm
        BodyPartDef {
            min: V3::new(-8.0, 12.0, -2.0),
            max: V3::new(-4.0, 24.0, 2.0),
            tx: 40.0,
            ty: 16.0,
            w: 4.0,
            h: 12.0,
            d: 4.0,
        },
        // Left Arm
        BodyPartDef {
            min: V3::new(4.0, 12.0, -2.0),
            max: V3::new(8.0, 24.0, 2.0),
            tx: 32.0,
            ty: 48.0,
            w: 4.0,
            h: 12.0,
            d: 4.0,
        },
        // Right Leg
        BodyPartDef {
            min: V3::new(-4.0, 0.0, -2.0),
            max: V3::new(0.0, 12.0, 2.0),
            tx: 0.0,
            ty: 16.0,
            w: 4.0,
            h: 12.0,
            d: 4.0,
        },
        // Left Leg
        BodyPartDef {
            min: V3::new(0.0, 0.0, -2.0),
            max: V3::new(4.0, 12.0, 2.0),
            tx: 16.0,
            ty: 48.0,
            w: 4.0,
            h: 12.0,
            d: 4.0,
        },
        // ── Overlay layers (slightly expanded boxes) ──
        // Head overlay (hat)
        BodyPartDef {
            min: V3::new(-4.0 - e, 24.0 - e, -4.0 - e),
            max: V3::new(4.0 + e, 32.0 + e, 4.0 + e),
            tx: 32.0,
            ty: 0.0,
            w: 8.0,
            h: 8.0,
            d: 8.0,
        },
        // Body overlay
        BodyPartDef {
            min: V3::new(-4.0 - e, 12.0 - e, -2.0 - e),
            max: V3::new(4.0 + e, 24.0 + e, 2.0 + e),
            tx: 16.0,
            ty: 32.0,
            w: 8.0,
            h: 12.0,
            d: 4.0,
        },
        // Right Arm overlay
        BodyPartDef {
            min: V3::new(-8.0 - e, 12.0 - e, -2.0 - e),
            max: V3::new(-4.0 + e, 24.0 + e, 2.0 + e),
            tx: 40.0,
            ty: 32.0,
            w: 4.0,
            h: 12.0,
            d: 4.0,
        },
        // Left Arm overlay
        BodyPartDef {
            min: V3::new(4.0 - e, 12.0 - e, -2.0 - e),
            max: V3::new(8.0 + e, 24.0 + e, 2.0 + e),
            tx: 48.0,
            ty: 48.0,
            w: 4.0,
            h: 12.0,
            d: 4.0,
        },
        // Right Leg overlay
        BodyPartDef {
            min: V3::new(-4.0 - e, 0.0 - e, -2.0 - e),
            max: V3::new(0.0 + e, 12.0 + e, 2.0 + e),
            tx: 0.0,
            ty: 32.0,
            w: 4.0,
            h: 12.0,
            d: 4.0,
        },
        // Left Leg overlay
        BodyPartDef {
            min: V3::new(0.0 - e, 0.0 - e, -2.0 - e),
            max: V3::new(4.0 + e, 12.0 + e, 2.0 + e),
            tx: 0.0,
            ty: 48.0,
            w: 4.0,
            h: 12.0,
            d: 4.0,
        },
    ]
}

// ── Rasterization ───────────────────────────────────────────────────────────

/// 2D edge function: positive when `p` is to the left of edge `a→b`.
#[inline]
fn edge(ax: f64, ay: f64, bx: f64, by: f64, px: f64, py: f64) -> f64 {
    (bx - ax) * (py - ay) - (by - ay) * (px - ax)
}

/// Alpha-blend `src` over `dst` (premultiplied-style, simple).
#[inline]
fn alpha_blend(dst: &mut Rgba<u8>, src: Rgba<u8>) {
    let sa = src[3] as u16;
    if sa == 0 {
        return;
    }
    if sa == 255 {
        *dst = src;
        return;
    }
    let da = dst[3] as u16;
    let inv_sa = 255 - sa;
    let out_a = sa + (da * inv_sa) / 255;
    if out_a == 0 {
        return;
    }
    for i in 0..3 {
        dst[i] = ((src[i] as u16 * sa + dst[i] as u16 * da * inv_sa / 255) / out_a) as u8;
    }
    dst[3] = out_a as u8;
}

/// Rasterize a single triangle with texture mapping and z-buffering.
fn rasterize_triangle(
    // Screen-space vertices (x, y, z for depth)
    v0: (f64, f64, f64),
    v1: (f64, f64, f64),
    v2: (f64, f64, f64),
    // Texture coordinates (absolute pixel coords in skin)
    uv0: (f64, f64),
    uv1: (f64, f64),
    uv2: (f64, f64),
    skin: &image::DynamicImage,
    output: &mut RgbaImage,
    zbuf: &mut [f64],
    out_w: u32,
    out_h: u32,
) {
    let skin_w = skin.width();
    let skin_h = skin.height();

    // Bounding box (clamped to output)
    let min_x = v0.0.min(v1.0).min(v2.0).floor().max(0.0) as i32;
    let max_x = v0.0.max(v1.0).max(v2.0).ceil().min(out_w as f64 - 1.0) as i32;
    let min_y = v0.1.min(v1.1).min(v2.1).floor().max(0.0) as i32;
    let max_y = v0.1.max(v1.1).max(v2.1).ceil().min(out_h as f64 - 1.0) as i32;

    let area = edge(v0.0, v0.1, v1.0, v1.1, v2.0, v2.1);
    if area.abs() < 0.001 {
        return; // degenerate
    }
    let inv_area = 1.0 / area;

    for py in min_y..=max_y {
        for px in min_x..=max_x {
            let cx = px as f64 + 0.5;
            let cy = py as f64 + 0.5;

            let w0 = edge(v1.0, v1.1, v2.0, v2.1, cx, cy) * inv_area;
            let w1 = edge(v2.0, v2.1, v0.0, v0.1, cx, cy) * inv_area;
            let w2 = 1.0 - w0 - w1;

            if w0 >= 0.0 && w1 >= 0.0 && w2 >= 0.0 {
                let z = w0 * v0.2 + w1 * v1.2 + w2 * v2.2;
                let idx = (py as u32 * out_w + px as u32) as usize;

                if z > zbuf[idx] {
                    let u = w0 * uv0.0 + w1 * uv1.0 + w2 * uv2.0;
                    let v = w0 * uv0.1 + w1 * uv1.1 + w2 * uv2.1;

                    // Nearest-neighbor sampling (crisp Minecraft pixels)
                    let tx = (u.floor() as u32).min(skin_w - 1);
                    let ty = (v.floor() as u32).min(skin_h - 1);
                    let pixel = skin.get_pixel(tx, ty);

                    if pixel[3] > 0 {
                        zbuf[idx] = z;
                        let dst = output.get_pixel_mut(px as u32, py as u32);
                        alpha_blend(dst, pixel);
                    }
                }
            }
        }
    }
}

/// Rasterize a quad (4 vertices) as two triangles.
fn rasterize_quad(
    verts: &[(f64, f64, f64); 4],
    uvs: &[(f64, f64); 4],
    skin: &image::DynamicImage,
    output: &mut RgbaImage,
    zbuf: &mut [f64],
    out_w: u32,
    out_h: u32,
) {
    // Triangle 1: v0, v1, v2
    rasterize_triangle(verts[0], verts[1], verts[2], uvs[0], uvs[1], uvs[2], skin, output, zbuf, out_w, out_h);
    // Triangle 2: v0, v2, v3
    rasterize_triangle(verts[0], verts[2], verts[3], uvs[0], uvs[2], uvs[3], skin, output, zbuf, out_w, out_h);
}

// ── Public API ──────────────────────────────────────────────────────────────

/// Render a Minecraft skin into an isometric 3D view, returning raw RGBA pixel data.
///
/// - `skin_png_bytes`: the full 64×64 (or 64×32 legacy) skin texture as PNG.
/// - `out_width`, `out_height`: desired output image dimensions.
/// - `yaw_deg`: Y-axis rotation in degrees (horizontal spin).
/// - `pitch_deg`: X-axis rotation in degrees (vertical tilt; negative = looking from above).
///
/// Returns the rendered `RgbaImage`, or `None` on failure.
pub fn render_skin_3d_raw(
    skin_png_bytes: &[u8],
    out_width: u32,
    out_height: u32,
    yaw_deg: f64,
    pitch_deg: f64,
) -> Option<RgbaImage> {
    let skin = image::load_from_memory_with_format(skin_png_bytes, ImageFormat::Png).ok()?;

    // Handle legacy 64×32 skins by only using the top portion body parts
    let is_legacy = skin.height() < 64;

    let rot = Mat3::rotation_yx(yaw_deg.to_radians(), pitch_deg.to_radians());

    // Center of model vertically (player is 32 units tall, from y=0 to y=32)
    let center_y = 16.0;

    // Gather all quads from all body parts
    let parts = player_model();
    let mut projected_quads: Vec<([(f64, f64, f64); 4], [(f64, f64); 4], f64)> = Vec::new();

    for (i, part) in parts.iter().enumerate() {
        // Skip overlay left limbs on legacy skins (they don't exist in 64×32)
        if is_legacy && i >= 6 {
            // Overlay layers: indices 6..12
            // For legacy skins, only head overlay (index 6) works (at 32,0)
            // Skip body/arm/leg overlays that reference rows >= 32
            if i > 6 {
                continue;
            }
        }
        // Skip left limb base layers on legacy skins (use right limb mirrored – but for
        // simplicity we just skip; they'd sample garbage from the texture)
        if is_legacy && (i == 3 || i == 5) {
            continue;
        }

        let quads = box_quads(part.min, part.max, part.tx, part.ty, part.w, part.h, part.d);

        for quad in &quads {
            // Rotate normal to check visibility
            let rn = rot.transform(quad.normal);
            // Camera looks along -Z, so faces with normal.z > 0 face the camera
            if rn.z <= 0.0 {
                continue;
            }

            // Transform vertices
            let mut screen_verts = [(0.0, 0.0, 0.0); 4];
            let mut avg_z = 0.0;
            for (j, v) in quad.verts.iter().enumerate() {
                let centered = V3::new(v.x, v.y - center_y, v.z);
                let rv = rot.transform(centered);
                screen_verts[j] = (rv.x, -rv.y, rv.z); // flip Y for screen coords (Y down)
                avg_z += rv.z;
            }
            avg_z /= 4.0;

            projected_quads.push((screen_verts, quad.uvs, avg_z));
        }
    }

    // Sort back-to-front (painter's algorithm): smaller Z = further from camera = draw first
    projected_quads.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal));

    // Compute bounding box of all projected vertices to determine scale + offset
    let mut min_x = f64::MAX;
    let mut max_x = f64::MIN;
    let mut min_y = f64::MAX;
    let mut max_y = f64::MIN;
    for (verts, _, _) in &projected_quads {
        for &(x, y, _) in verts {
            min_x = min_x.min(x);
            max_x = max_x.max(x);
            min_y = min_y.min(y);
            max_y = max_y.max(y);
        }
    }

    if max_x <= min_x || max_y <= min_y {
        return None;
    }

    let model_w = max_x - min_x;
    let model_h = max_y - min_y;
    let padding = 4.0; // pixels of padding on each side
    let avail_w = out_width as f64 - 2.0 * padding;
    let avail_h = out_height as f64 - 2.0 * padding;
    let scale = (avail_w / model_w).min(avail_h / model_h);
    let offset_x = padding + (avail_w - model_w * scale) / 2.0 - min_x * scale;
    let offset_y = padding + (avail_h - model_h * scale) / 2.0 - min_y * scale;

    // Create output image (transparent background)
    let mut output = RgbaImage::new(out_width, out_height);
    let mut zbuf = vec![f64::MIN; (out_width * out_height) as usize];

    // Rasterize each quad
    for (verts, uvs, _) in &projected_quads {
        let mut sv = [(0.0, 0.0, 0.0); 4];
        for i in 0..4 {
            sv[i] = (verts[i].0 * scale + offset_x, verts[i].1 * scale + offset_y, verts[i].2);
        }
        rasterize_quad(&sv, uvs, &skin, &mut output, &mut zbuf, out_width, out_height);
    }

    Some(output)
}

/// Render a Minecraft skin into an isometric 3D view, returning PNG-encoded bytes.
///
/// Convenience wrapper around [`render_skin_3d_raw`] that encodes the result to PNG.
pub fn render_skin_3d(
    skin_png_bytes: &[u8],
    out_width: u32,
    out_height: u32,
    yaw_deg: f64,
    pitch_deg: f64,
) -> Option<Vec<u8>> {
    let output = render_skin_3d_raw(skin_png_bytes, out_width, out_height, yaw_deg, pitch_deg)?;
    let mut png_bytes = Vec::new();
    output.write_to(&mut Cursor::new(&mut png_bytes), ImageFormat::Png).ok()?;
    Some(png_bytes)
}
