use image::{GenericImageView, ImageFormat, Pixel, RgbaImage};
use schema::minecraft_profile::SkinVariant;

#[derive(Clone, Copy, Debug)]
struct V3 {
    x: f64,
    y: f64,
    z: f64,
}

impl V3 {
    const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    fn dot(&self, other: V3) -> f64 {
        self.x * other.x + self.y * other.y + self.z * other.z
    }
}

/// 3×3 rotation matrix stored as rows.
struct Mat3([[f64; 3]; 3]);

impl Mat3 {
    fn rotation_yx(ry: f64, rx: f64) -> Self {
        let (sy, cy) = ry.sin_cos();
        let (sx, cx) = rx.sin_cos();
        Self([[cy, 0.0, sy], [sx * sy, cx, -sx * cy], [-cx * sy, sx, cx * cy]])
    }

    fn rotation_x(rx: f64) -> Self {
        let (sx, cx) = rx.sin_cos();
        Self([[1.0, 0.0, 0.0], [0.0, cx, -sx], [0.0, sx, cx]])
    }

    fn transform(&self, v: V3) -> V3 {
        let r = &self.0;
        V3 {
            x: r[0][0] * v.x + r[0][1] * v.y + r[0][2] * v.z,
            y: r[1][0] * v.x + r[1][1] * v.y + r[1][2] * v.z,
            z: r[2][0] * v.x + r[2][1] * v.y + r[2][2] * v.z,
        }
    }

    fn transform_with_offset(&self, v: V3, o: V3) -> V3 {
        let r = &self.0;
        let x = v.x - o.x;
        let y = v.y - o.y;
        let z = v.z - o.z;
        V3 {
            x: r[0][0] * x + r[0][1] * y + r[0][2] * z + o.x,
            y: r[1][0] * x + r[1][1] * y + r[1][2] * z + o.y,
            z: r[2][0] * x + r[2][1] * y + r[2][2] * z + o.z,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum BodyPartType {
    Head,
    Body,
    RightArm,
    LeftArm,
    RightLeg,
    LeftLeg,
    HeadOverlay,
    BodyOverlay,
    RightArmOverlay,
    LeftArmOverlay,
    RightLegOverlay,
    LeftLegOverlay,
    Cape,
}

impl BodyPartType {
    pub const fn inflate(self) -> f64 {
        match self {
            Self::HeadOverlay |
                Self::RightArmOverlay |
                Self::LeftArmOverlay |
                Self::RightLegOverlay |
                Self::LeftLegOverlay =>
            {
                0.25
            },
            Self::BodyOverlay => 0.24,
            _ => 0.0,
        }
    }

    pub const fn allow_transparency(self) -> bool {
        match self {
            Self::HeadOverlay |
                Self::RightArmOverlay |
                Self::LeftArmOverlay |
                Self::RightLegOverlay |
                Self::LeftLegOverlay |
                Self::BodyOverlay =>
            {
                true
            },
            _ => false,
        }
    }

    pub const fn sway_time_mult(self) -> f64 {
        if let Self::Cape = self {
            1.0
        } else {
            4.0
        }
    }

    pub const fn sway_strength(self) -> f64 {
        match self {
            Self::RightArm |
                Self::RightArmOverlay |
                Self::LeftLeg |
                Self::LeftLegOverlay =>
            {
                1.0
            },
            Self::LeftArm |
                Self::LeftArmOverlay |
                Self::RightLeg |
                Self::RightLegOverlay =>
            {
                -1.0
            },
            Self::Cape => {
                0.25
            },
            _ => 0.0,
        }
    }

    pub const fn pitch_offset(self) -> f64 {
        if let Self::Cape = self {
            18.75_f64.to_radians()
        } else {
            0.0
        }
    }
}

struct BodyPartDef {
    min: V3,
    max: V3,
    pivot: Option<V3>,
    tx: f64,
    ty: f64,
    flip_x: bool,
    part_type: BodyPartType,
}

impl BodyPartDef {
    pub const fn to_quads(&self) -> [Quad; 6] {
        let inflate = self.part_type.inflate();
        let allow_transparency = self.part_type.allow_transparency();

        let w = self.max.x - self.min.x;
        let h = self.max.y - self.min.y;
        let d = self.max.z - self.min.z;
        let x0 = self.min.x - inflate;
        let y0 = self.min.y - inflate;
        let z0 = self.min.z - inflate;
        let x1 = self.max.x + inflate;
        let y1 = self.max.y + inflate;
        let z1 = self.max.z + inflate;
        let tx = self.tx;
        let ty = self.ty;
        let flip_x = self.flip_x;
        let flip_z = matches!(self.part_type, BodyPartType::Cape);

        let mut quads = [
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
                allow_transparency,
            }.flip_uv_horz(flip_x),
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
                allow_transparency,
            }.flip_uv_horz(flip_x),
            // Right face (-X, player's right) – texture at (tx, ty+d) size d×h
            Quad {
                verts: [
                    V3::new(x0, y1, z1),
                    V3::new(x0, y1, z0),
                    V3::new(x0, y0, z0),
                    V3::new(x0, y0, z1),
                ],
                uvs: [
                    (tx + d, ty + d),
                    (tx, ty + d),
                    (tx, ty + d + h),
                    (tx + d, ty + d + h)
                ],
                normal: V3::new(-1.0, 0.0, 0.0),
                allow_transparency,
            }.flip_uv_horz(flip_z),
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
                allow_transparency,
            }.flip_uv_horz(flip_z),
            // Top face (+Y) – texture at (tx+d, ty) size w×d
            Quad {
                verts: [
                    V3::new(x0, y1, z0),
                    V3::new(x1, y1, z0),
                    V3::new(x1, y1, z1),
                    V3::new(x0, y1, z1),
                ],
                uvs: [
                    (tx + d, ty),
                    (tx + d + w, ty),
                    (tx + d + w, ty + d),
                    (tx + d, ty + d)
                ],
                normal: V3::new(0.0, 1.0, 0.0),
                allow_transparency,
            }.flip_uv_horz(flip_x).flip_uv_vert(flip_z),
            // Bottom face (-Y) – texture at (tx+d+w, ty) size w×d
            Quad {
                verts: [
                    V3::new(x1, y0, z0),
                    V3::new(x0, y0, z0),
                    V3::new(x0, y0, z1),
                    V3::new(x1, y0, z1),
                ],
                uvs: [
                    (tx + d + 2.0 * w, ty),
                    (tx + d + w, ty),
                    (tx + d + w, ty + d),
                    (tx + d + 2.0 * w, ty + d),
                ],
                normal: V3::new(0.0, -1.0, 0.0),
                allow_transparency,
            }.flip_uv_horz(flip_x).flip_uv_vert(flip_z),
        ];

        if flip_x {
            unsafe {
                std::ptr::swap(&raw mut quads[2].uvs, &raw mut quads[3].uvs);
            }
        }
        if flip_z {
            unsafe {
                std::ptr::swap(&raw mut quads[0].uvs, &raw mut quads[1].uvs);
            }
        }

        quads
    }

    fn add_projected_quads(&self, projected_quads: &mut Vec<ProjectedQuad>, rot: &Mat3, light0: V3, light1: V3, sway_progress: f64) {
        let mut quads = self.to_quads();
        let sway_strength = self.part_type.sway_strength();
        let sway_time_mult = self.part_type.sway_time_mult();
        let pitch_offset = self.part_type.pitch_offset();

        let pitch = -15.0_f64.to_radians() * sway_strength * (sway_progress * std::f64::consts::TAU * sway_time_mult).sin() + pitch_offset;

        if pitch != 0.0 {
            let transform = Mat3::rotation_x(pitch);
            for quad in &mut quads {
                let pivot = self.pivot.unwrap_or_else(|| {
                    V3::new(
                        self.min.x/2.0 + self.max.x/2.0,
                        self.min.y/2.0 + self.max.y/2.0,
                        self.min.z/2.0 + self.max.z/2.0,
                    )
                });
                for vert in &mut quad.verts {
                    *vert = transform.transform_with_offset(*vert, pivot);
                };
                quad.normal = transform.transform(quad.normal);
            }
        }

        for quad in &quads {
            let mut rn = rot.transform(quad.normal);

            if rn.z <= 0.0 {
                if !quad.allow_transparency {
                    // Cull back facing quads
                    continue;
                } else {
                    // Flip for correct lighting
                    rn.x *= -1.0;
                    rn.y *= -1.0;
                    rn.z *= -1.0;
                }
            }

            let dot0 = rn.dot(light0).clamp(0.0, 1.0);
            let dot1 = rn.dot(light1).clamp(0.0, 1.0);
            let accum = ((dot0 + dot1).min(1.0) * 0.4 + 0.6).clamp(0.0, 1.0);
            let shade = (accum * 255.0) as u8;

            // Transform vertices
            let mut screen_verts = [V3::new(0.0, 0.0, 0.0); 4];
            let mut avg_z = 0.0;
            for (j, v) in quad.verts.iter().enumerate() {
                let mut rv = rot.transform(*v);
                rv.y *= -1.0; // flip for screenspace (y down)
                screen_verts[j] = rv;
                avg_z += rv.z;
            }
            avg_z /= 4.0;

            projected_quads.push(ProjectedQuad {
                verts: screen_verts,
                uvs: quad.uvs,
                avg_z,
                allow_transparency: quad.allow_transparency,
                shade,
                part_type: self.part_type
            });
        }
    }
}

static PLAYER_MODEL: &'static [BodyPartDef] = &[
    // Head
    BodyPartDef {
        min: V3::new(-4.0, 8.0, -4.0),
        max: V3::new(4.0, 16.0, 4.0),
        pivot: None,
        tx: 0.0,
        ty: 0.0,
        flip_x: false,
        part_type: BodyPartType::Head,
    },
    // Body
    BodyPartDef {
        min: V3::new(-4.0, -4.0, -2.0),
        max: V3::new(4.0, 8.0, 2.0),
        pivot: None,
        tx: 16.0,
        ty: 16.0,
        flip_x: false,
        part_type: BodyPartType::Body,
    },
    // Right Arm
    BodyPartDef {
        min: V3::new(-8.0, -4.0, -2.0),
        max: V3::new(-4.0, 8.0, 2.0),
        pivot: Some(V3::new(-4.0, 8.0, 0.0)),
        tx: 40.0,
        ty: 16.0,
        flip_x: false,
        part_type: BodyPartType::RightArm,
    },
    // Left Arm
    BodyPartDef {
        min: V3::new(4.0, -4.0, -2.0),
        max: V3::new(8.0, 8.0, 2.0),
        pivot: Some(V3::new(4.0, 8.0, 0.0)),
        tx: 32.0,
        ty: 48.0,
        flip_x: false,
        part_type: BodyPartType::LeftArm,
    },
    // Right Leg
    BodyPartDef {
        min: V3::new(-4.0, -16.0, -2.0),
        max: V3::new(0.0, -4.0, 2.0),
        pivot: Some(V3::new(-2.0, -4.0, 0.0)),
        tx: 0.0,
        ty: 16.0,
        flip_x: false,
        part_type: BodyPartType::RightLeg,
    },
    // Left Leg
    BodyPartDef {
        min: V3::new(0.0, -16.0, -2.0),
        max: V3::new(4.0, -4.0, 2.0),
        pivot: Some(V3::new(2.0, -4.0, 0.0)),
        tx: 16.0,
        ty: 48.0,
        flip_x: false,
        part_type: BodyPartType::LeftLeg,
    },
    // Head overlay (hat)
    BodyPartDef {
        min: V3::new(-4.0, 8.0, -4.0),
        max: V3::new(4.0, 16.0, 4.0),
        pivot: None,
        tx: 32.0,
        ty: 0.0,
        flip_x: false,
        part_type: BodyPartType::HeadOverlay,
    },
    // Body overlay
    BodyPartDef {
        min: V3::new(-4.0, -4.0, -2.0),
        max: V3::new(4.0, 8.0, 2.0),
        pivot: None,
        tx: 16.0,
        ty: 32.0,
        flip_x: false,
        part_type: BodyPartType::BodyOverlay,
    },
    // Right Arm overlay
    BodyPartDef {
        min: V3::new(-8.0, -4.0, -2.0),
        max: V3::new(-4.0, 8.0, 2.0),
        pivot: Some(V3::new(-4.0, 8.0, 0.0)),
        tx: 40.0,
        ty: 32.0,
        flip_x: false,
        part_type: BodyPartType::RightArmOverlay,
    },
    // Left Arm overlay
    BodyPartDef {
        min: V3::new(4.0, -4.0, -2.0),
        max: V3::new(8.0, 8.0, 2.0),
        pivot: Some(V3::new(4.0, 8.0, 0.0)),
        tx: 48.0,
        ty: 48.0,
        flip_x: false,
        part_type: BodyPartType::LeftArmOverlay,
    },
    // Right Leg overlay
    BodyPartDef {
        min: V3::new(-4.0, -16.0, -2.0),
        max: V3::new(0.0, -4.0, 2.0),
        pivot: Some(V3::new(-2.0, -4.0, 0.0)),
        tx: 0.0,
        ty: 32.0,
        flip_x: false,
        part_type: BodyPartType::RightLegOverlay,
    },
    // Left Leg overlay
    BodyPartDef {
        min: V3::new(0.0, -16.0, -2.0),
        max: V3::new(4.0, -4.0, 2.0),
        pivot: Some(V3::new(2.0, -4.0, 0.0)),
        tx: 0.0,
        ty: 48.0,
        flip_x: false,
        part_type: BodyPartType::LeftLegOverlay,
    },
];

static SLIM_PLAYER_MODEL: &'static [BodyPartDef] = &[
    // Head
    BodyPartDef {
        min: V3::new(-4.0, 8.0, -4.0),
        max: V3::new(4.0, 16.0, 4.0),
        pivot: None,
        tx: 0.0,
        ty: 0.0,
        flip_x: false,
        part_type: BodyPartType::Head,
    },
    // Body
    BodyPartDef {
        min: V3::new(-4.0, -4.0, -2.0),
        max: V3::new(4.0, 8.0, 2.0),
        pivot: None,
        tx: 16.0,
        ty: 16.0,
        flip_x: false,
        part_type: BodyPartType::Body,
    },
    // Right Arm
    BodyPartDef {
        min: V3::new(-7.0, -4.0, -2.0),
        max: V3::new(-4.0, 8.0, 2.0),
        pivot: Some(V3::new(-4.0, 8.0, 0.0)),
        tx: 40.0,
        ty: 16.0,
        flip_x: false,
        part_type: BodyPartType::RightArm,
    },
    // Left Arm
    BodyPartDef {
        min: V3::new(4.0, -4.0, -2.0),
        max: V3::new(7.0, 8.0, 2.0),
        pivot: Some(V3::new(4.0, 8.0, 0.0)),
        tx: 32.0,
        ty: 48.0,
        flip_x: false,
        part_type: BodyPartType::LeftArm,
    },
    // Right Leg
    BodyPartDef {
        min: V3::new(-4.0, -16.0, -2.0),
        max: V3::new(0.0, -4.0, 2.0),
        pivot: Some(V3::new(-2.0, -4.0, 0.0)),
        tx: 0.0,
        ty: 16.0,
        flip_x: false,
        part_type: BodyPartType::RightLeg,
    },
    // Left Leg
    BodyPartDef {
        min: V3::new(0.0, -16.0, -2.0),
        max: V3::new(4.0, -4.0, 2.0),
        pivot: Some(V3::new(2.0, -4.0, 0.0)),
        tx: 16.0,
        ty: 48.0,
        flip_x: false,
        part_type: BodyPartType::LeftLeg,
    },
    // Head overlay (hat)
    BodyPartDef {
        min: V3::new(-4.0, 8.0, -4.0),
        max: V3::new(4.0, 16.0, 4.0),
        pivot: None,
        tx: 32.0,
        ty: 0.0,
        flip_x: false,
        part_type: BodyPartType::HeadOverlay,
    },
    // Body overlay
    BodyPartDef {
        min: V3::new(-4.0, -4.0, -2.0),
        max: V3::new(4.0, 8.0, 2.0),
        pivot: None,
        tx: 16.0,
        ty: 32.0,
        flip_x: false,
        part_type: BodyPartType::BodyOverlay,
    },
    // Right Arm overlay
    BodyPartDef {
        min: V3::new(-7.0, -4.0, -2.0),
        max: V3::new(-4.0, 8.0, 2.0),
        pivot: Some(V3::new(-4.0, 8.0, 0.0)),
        tx: 40.0,
        ty: 32.0,
        flip_x: false,
        part_type: BodyPartType::RightArmOverlay,
    },
    // Left Arm overlay
    BodyPartDef {
        min: V3::new(4.0, -4.0, -2.0),
        max: V3::new(7.0, 8.0, 2.0),
        pivot: Some(V3::new(4.0, 8.0, 0.0)),
        tx: 48.0,
        ty: 48.0,
        flip_x: false,
        part_type: BodyPartType::LeftArmOverlay,
    },
    // Right Leg overlay
    BodyPartDef {
        min: V3::new(-4.0, -16.0, -2.0),
        max: V3::new(0.0, -4.0, 2.0),
        pivot: Some(V3::new(-2.0, -4.0, 0.0)),
        tx: 0.0,
        ty: 32.0,
        flip_x: false,
        part_type: BodyPartType::RightLegOverlay,
    },
    // Left Leg overlay
    BodyPartDef {
        min: V3::new(0.0, -16.0, -2.0),
        max: V3::new(4.0, -4.0, 2.0),
        pivot: Some(V3::new(2.0, -4.0, 0.0)),
        tx: 0.0,
        ty: 48.0,
        flip_x: false,
        part_type: BodyPartType::LeftLegOverlay,
    },
];

static LEGACY_PLAYER_MODEL: &'static [BodyPartDef] = &[
    // Head
    BodyPartDef {
        min: V3::new(-4.0, 8.0, -4.0),
        max: V3::new(4.0, 16.0, 4.0),
        pivot: None,
        tx: 0.0,
        ty: 0.0,
        flip_x: false,
        part_type: BodyPartType::Head,
    },
    // Body
    BodyPartDef {
        min: V3::new(-4.0, -4.0, -2.0),
        max: V3::new(4.0, 8.0, 2.0),
        pivot: None,
        tx: 16.0,
        ty: 16.0,
        flip_x: false,
        part_type: BodyPartType::Body,
    },
    // Right Arm
    BodyPartDef {
        min: V3::new(-8.0, -4.0, -2.0),
        max: V3::new(-4.0, 8.0, 2.0),
        pivot: Some(V3::new(-4.0, 8.0, 0.0)),
        tx: 40.0,
        ty: 16.0,
        flip_x: false,
        part_type: BodyPartType::RightArm,
    },
    // Left Arm
    BodyPartDef {
        min: V3::new(4.0, -4.0, -2.0),
        max: V3::new(8.0, 8.0, 2.0),
        pivot: Some(V3::new(4.0, 8.0, 0.0)),
        tx: 40.0,
        ty: 16.0,
        flip_x: true,
        part_type: BodyPartType::LeftArm,
    },
    // Right Leg
    BodyPartDef {
        min: V3::new(-4.0, -16.0, -2.0),
        max: V3::new(0.0, -4.0, 2.0),
        pivot: Some(V3::new(-2.0, -4.0, 0.0)),
        tx: 0.0,
        ty: 16.0,
        flip_x: false,
        part_type: BodyPartType::RightLeg,
    },
    // Left Leg
    BodyPartDef {
        min: V3::new(0.0, -16.0, -2.0),
        max: V3::new(4.0, -4.0, 2.0),
        pivot: Some(V3::new(2.0, -4.0, 0.0)),
        tx: 0.0,
        ty: 16.0,
        flip_x: true,
        part_type: BodyPartType::LeftLeg,
    },
    // Head overlay (hat)
    BodyPartDef {
        min: V3::new(-4.0, 8.0, -4.0),
        max: V3::new(4.0, 16.0, 4.0),
        pivot: None,
        tx: 32.0,
        ty: 0.0,
        flip_x: false,
        part_type: BodyPartType::HeadOverlay,
    },
];

struct Quad {
    verts: [V3; 4],
    uvs: [(f64, f64); 4],
    normal: V3,
    allow_transparency: bool,
}

impl Quad {
    pub const fn flip_uv_horz(mut self, flip: bool) -> Self {
        if flip {
            self.uvs.swap(0, 1);
            self.uvs.swap(2, 3);
        }
        self
    }

    pub const fn flip_uv_vert(mut self, flip: bool) -> Self {
        if flip {
            self.uvs.swap(0, 3);
            self.uvs.swap(1, 2);
        }
        self
    }
}

struct ProjectedQuad {
    verts: [V3; 4],
    uvs: [(f64, f64); 4],
    avg_z: f64,
    allow_transparency: bool,
    shade: u8,
    part_type: BodyPartType,
}

/// 2D edge function: positive when `p` is to the left of edge `a→b`.
#[inline]
fn edge(ax: f64, ay: f64, bx: f64, by: f64, px: f64, py: f64) -> f64 {
    (bx - ax) * (py - ay) - (by - ay) * (px - ax)
}

/// Rasterize a single triangle with texture mapping and z-buffering.
fn rasterize_triangle(
    // Screen-space vertices (x, y, z for depth)
    v0: V3,
    v1: V3,
    v2: V3,
    // Texture coordinates (absolute pixel coords in skin)
    uv0: (f64, f64),
    uv1: (f64, f64),
    uv2: (f64, f64),
    skin: &image::DynamicImage,
    output: &mut RgbaImage,
    zbuf: &mut [f64],
    allow_transparency: bool,
    shade: u8,
) {
    let skin_w = skin.width();
    let skin_h = skin.height();
    let out_w = output.width();
    let out_h = output.height();

    // Bounding box (clamped to output)
    let min_x = v0.x.min(v1.x).min(v2.x).floor().max(0.0) as i32;
    let max_x = v0.x.max(v1.x).max(v2.x).ceil().min(out_w as f64 - 1.0) as i32;
    let min_y = v0.y.min(v1.y).min(v2.y).floor().max(0.0) as i32;
    let max_y = v0.y.max(v1.y).max(v2.y).ceil().min(out_h as f64 - 1.0) as i32;

    let area = edge(v0.x, v0.y, v1.x, v1.y, v2.x, v2.y);
    if area.abs() < 0.001 {
        return; // degenerate
    }
    let inv_area = 1.0 / area;

    for py in min_y..=max_y {
        for px in min_x..=max_x {
            let cx = px as f64 + 0.5;
            let cy = py as f64 + 0.5;

            let w0 = edge(v1.x, v1.y, v2.x, v2.y, cx, cy) * inv_area;
            let w1 = edge(v2.x, v2.y, v0.x, v0.y, cx, cy) * inv_area;
            let w2 = 1.0 - w0 - w1;

            if w0 >= 0.0 && w1 >= 0.0 && w2 >= 0.0 {
                let z = w0 * v0.z + w1 * v1.z + w2 * v2.z;
                let idx = (py as u32 * out_w + px as u32) as usize;

                if z > zbuf[idx] {
                    let u = w0 * uv0.0 + w1 * uv1.0 + w2 * uv2.0;
                    let v = w0 * uv0.1 + w1 * uv1.1 + w2 * uv2.1;

                    let tx = (u.floor() as u32).min(skin_w - 1);
                    let ty = (v.floor() as u32).min(skin_h - 1);
                    let mut pixel = skin.get_pixel(tx, ty);

                    pixel[0] = ((pixel[0] as u16 * shade as u16) / 255) as u8;
                    pixel[1] = ((pixel[1] as u16 * shade as u16) / 255) as u8;
                    pixel[2] = ((pixel[2] as u16 * shade as u16) / 255) as u8;

                    if !allow_transparency {
                        pixel[3] = 0xFF;
                        output.put_pixel(px as u32, py as u32, pixel);
                        zbuf[idx] = z;
                    } else if pixel[3] > 0 {
                        output.get_pixel_mut(px as u32, py as u32).blend(&pixel);
                        zbuf[idx] = z;
                    }
                }
            }
        }
    }
}

impl ProjectedQuad {
    fn rasterize(
        &self,
        skin: &image::DynamicImage,
        output: &mut RgbaImage,
        zbuf: &mut [f64],
    ) {
        // Triangle 1: v0, v1, v2
        rasterize_triangle(self.verts[0], self.verts[1], self.verts[2],
           self.uvs[0], self.uvs[1], self.uvs[2],
           skin, output, zbuf, self.allow_transparency, self.shade);
        // Triangle 2: v0, v2, v3
        rasterize_triangle(self.verts[0], self.verts[2], self.verts[3],
            self.uvs[0], self.uvs[2], self.uvs[3],
            skin, output, zbuf, self.allow_transparency, self.shade);
    }
}

fn collect_quads(
    is_legacy: bool,
    is_slim: bool,
    add_cape: bool,
    yaw_deg: f64,
    pitch_deg: f64,
    sway_progress: f64,
) -> Vec<ProjectedQuad> {
    let rot = Mat3::rotation_yx(yaw_deg.to_radians(), pitch_deg.to_radians());

    let parts = if is_legacy {
        LEGACY_PLAYER_MODEL
    } else if is_slim {
        SLIM_PLAYER_MODEL
    } else {
        PLAYER_MODEL
    };
    let mut projected_quads: Vec<ProjectedQuad> = Vec::new();

    let light0 = V3::new(0.16169041669088866, 0.8084520834544432, -0.5659164584181102);
    let light1 = V3::new(-0.16169041669088866, 0.8084520834544432, 0.5659164584181102);

    for part in parts.iter() {
        part.add_projected_quads(&mut projected_quads, &rot, light0, light1, sway_progress);
    }

    if add_cape {
        BodyPartDef {
            min: V3::new(-5.0, -8.0, -3.0),
            max: V3::new(5.0, 8.0, -2.0),
            pivot: Some(V3::new(0.0, 8.0, -2.0)),
            tx: 0.0,
            ty: 0.0,
            flip_x: false,
            part_type: BodyPartType::Cape,
        }.add_projected_quads(&mut projected_quads, &rot, light0, light1, sway_progress);
    }

    projected_quads
}

pub fn determine_skin_variant(skin_png: &[u8]) -> Option<SkinVariant> {
    let skin = image::load_from_memory_with_format(skin_png, ImageFormat::Png).ok()?;
    let is_legacy = skin.height() == 32;
    if !is_legacy && skin.get_pixel(54, 20)[3] < 20 {
        Some(SkinVariant::Slim)
    } else {
        Some(SkinVariant::Classic)
    }
}

pub fn render_skin_3d(
    skin_png_bytes: &[u8],
    cape_png_bytes: Option<&[u8]>,
    variant: SkinVariant,
    out_width: u32,
    out_height: u32,
    yaw_deg: f64,
    pitch_deg: f64,
    sway_progress: f64,
    y_offset: f64,
    zoom: f64,
) -> Option<RgbaImage> {
    let skin = image::load_from_memory_with_format(skin_png_bytes, ImageFormat::Png).ok()?;
    let cape = cape_png_bytes.map(|cape| image::load_from_memory_with_format(cape, ImageFormat::Png).ok()).flatten();

    let is_legacy = skin.height() == 32;
    if skin.width() != 64 {
        return None;
    }
    if skin.height() != 64 && !is_legacy {
        return None;
    }
    let is_slim = match variant {
        SkinVariant::Classic => false,
        SkinVariant::Slim => true,
        SkinVariant::Other => !is_legacy && skin.get_pixel(54, 20)[3] < 20,
    };

    let mut projected_quads = collect_quads(is_legacy, is_slim, cape.is_some(), yaw_deg, pitch_deg, sway_progress);

    // Sort back-to-front (painter's algorithm): smaller Z = further from camera = draw first
    projected_quads.sort_by(|a, b| a.avg_z.partial_cmp(&b.avg_z).unwrap_or(std::cmp::Ordering::Equal));

    let scale = (out_width as f64 / MAX_WIDTH_AT_ANY_ANGLE).min(out_height as f64 / MAX_HEIGHT_AT_ANY_ANGLE) * zoom;
    let offset_x = out_width as f64 / 2.0;
    let offset_y = out_height as f64 / 2.0 + y_offset * scale;

    // Create output image (transparent background)
    let mut output = RgbaImage::new(out_width, out_height);
    let mut zbuf = vec![f64::MIN; (out_width * out_height) as usize];

    // Rasterize each quad
    for mut projected_quad in projected_quads {
        let verts = &mut projected_quad.verts;
        for i in 0..4 {
            verts[i] = V3::new(verts[i].x * scale + offset_x, verts[i].y * scale + offset_y, verts[i].z);
        }
        if projected_quad.part_type == BodyPartType::Cape {
            if let Some(cape) = &cape {
                projected_quad.rasterize(&cape, &mut output, &mut zbuf);
            }
        } else {
            projected_quad.rasterize(&skin, &mut output, &mut zbuf);
        }
    }

    Some(output)
}

// Constants calculated by brute force
const MAX_CAPE_ANGLE_SWAY_PROGRESS: f64 = 3.0/4.0;
const MAX_WIDTH_AT_ANY_ANGLE: f64 = 20.407198535851574; // yaw=60.65789523301863, pitch=0
const MAX_HEIGHT_AT_ANY_ANGLE: f64 = 34.65183977799737; // yaw=45, pitch=20.29798422703834
pub const ASPECT_RATIO: f64 = MAX_WIDTH_AT_ANY_ANGLE / MAX_HEIGHT_AT_ANY_ANGLE;

// Debug function used to brute force the max bounds of the model
#[cfg(debug_assertions)]
pub fn brute_force_bounds() {
    let mut best_yaw = 0.0;
    let mut best_pitch = 0.0;
    let mut max_w = 0.0;
    let mut scale = 90.0;
    let yaw_acc = 128;
    let pitch_acc = 128;
    let mut yaw_offset = 0.0;
    let mut pitch_offset = 0.0;

    log::info!("Calculating largest width");
    loop {
        log::info!("Scale: {scale}");
        for y in 0..=yaw_acc {
            for p in 0..=pitch_acc {
                let yaw = y as f64 / yaw_acc as f64 * scale + yaw_offset;
                let pitch = p as f64 / pitch_acc as f64 * scale + pitch_offset;

                let projected_quads = collect_quads(false, true, true, yaw, pitch, MAX_CAPE_ANGLE_SWAY_PROGRESS);
                let mut w = f64::MIN;
                for quad in &projected_quads {
                    for vert in &quad.verts {
                        w = w.max(vert.x.abs());
                    }
                }

                if w*2.0 > max_w {
                    max_w = w*2.0;
                    best_yaw = yaw;
                    best_pitch = pitch;
                } else if w*2.0 == max_w && best_yaw.abs()+best_pitch.abs() > yaw.abs()+pitch.abs() {
                    best_yaw = yaw;
                    best_pitch = pitch;
                }
            }
        }
        if scale == 90.0 {
            scale = 32.0;
        } else {
            scale /= 2.0;
        }
        let new_yaw_offset = best_yaw - scale/2.0;
        let new_pitch_offset = best_pitch - scale/2.0;
        if new_yaw_offset == yaw_offset || new_pitch_offset == pitch_offset {
            break;
        }
        yaw_offset = new_yaw_offset;
        pitch_offset = new_pitch_offset;
        log::info!("Best angle: {best_yaw:?}, {best_pitch:?}");
        log::info!("Width: {max_w:?}");
    }

    let best_w_yaw = best_yaw;
    let best_w_pitch = best_pitch;

    best_yaw = 0.0;
    best_pitch = 0.0;
    let mut max_h = 0.0;
    scale = 90.0;
    yaw_offset = 0.0;
    pitch_offset = 0.0;

    log::info!("Calculating largest height");
    loop {
        log::info!("Scale: {scale}");
        for y in 0..=yaw_acc {
            for p in 0..=pitch_acc {
                let yaw = y as f64 / yaw_acc as f64 * scale + yaw_offset;
                let pitch = p as f64 / pitch_acc as f64 * scale + pitch_offset;

                let projected_quads = collect_quads(false, true, true, yaw, pitch, MAX_CAPE_ANGLE_SWAY_PROGRESS);
                let mut h = f64::MIN;
                for quad in &projected_quads {
                    for vert in &quad.verts {
                        h = h.max(vert.y.abs());
                    }
                }

                if h*2.0 > max_h {
                    max_h = h*2.0;
                    best_yaw = yaw;
                    best_pitch = pitch;
                } else if h*2.0 == max_h && best_yaw.abs()+best_pitch.abs() > yaw.abs()+pitch.abs() {
                    best_yaw = yaw;
                    best_pitch = pitch;
                }
            }
        }
        if scale == 90.0 {
            scale = 32.0;
        } else {
            scale /= 2.0;
        }
        let new_yaw_offset = best_yaw - scale/2.0;
        let new_pitch_offset = best_pitch - scale/2.0;
        if new_yaw_offset == yaw_offset || new_pitch_offset == pitch_offset {
            break;
        }
        yaw_offset = new_yaw_offset;
        pitch_offset = new_pitch_offset;
        log::info!("Best angle: {best_yaw:?}, {best_pitch:?}");
        log::info!("Height: {max_h:?}");
    }

    log::info!("Largest width = {max_w:?} (yaw={best_w_yaw:?}, pitch={best_w_pitch:?}");
    log::info!("Largest height = {max_h:?} (yaw={best_yaw:?}, pitch={best_pitch:?}");
}
