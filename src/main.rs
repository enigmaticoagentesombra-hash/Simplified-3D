use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
enum FormaTipo {
    Cubo,
    Cuboide,
    PirCuadrada,
    Esfera,
    Cilindro,
    Cono,
    Capsula,
    Plano,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
struct GrupoData {
    nombre: String,
    posicion: [f32; 3],
    rotacion: [f32; 3],
    escala: [f32; 3],
}

impl GrupoData {
    fn new() -> Self {
        Self {
            nombre: "Grupo".into(),
            posicion: [0.0, 0.0, 0.0],
            rotacion: [0.0, 0.0, 0.0],
            escala: [1.0, 1.0, 1.0],
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
enum MaterialTipo {
    Plastico,
    Metal,
    Mate,
    Espejo,
}

impl MaterialTipo {
    fn factor_pgr(&self) -> (f32, f32) {
        match self {
            MaterialTipo::Plastico => (0.0, 0.7),
            MaterialTipo::Metal => (1.0, 0.4),
            MaterialTipo::Mate => (0.0, 1.0),
            MaterialTipo::Espejo => (1.0, 0.05),
        }
    }
}

// ============================================================
// Material personalizado (kiss3d `Material` trait) para dar
// reflectividad diferenciada por material en el viewport.
// Reimplementa el render del ObjectMaterial por defecto con un
// fragment shader que añade especular + brillo + un reflejo
// "falso" de cielo (fresnel) para metal/molde azul/mirror.
// ============================================================
use kiss3d::camera::Camera;
use kiss3d::context::Context;
use kiss3d::light::Light;
use kiss3d::nalgebra as ka;
use kiss3d::resource::vertex_index::VERTEX_INDEX_TYPE;
use kiss3d::resource::{Effect, Material, Mesh, ShaderAttribute, ShaderUniform};
use kiss3d::scene::{InstancesBuffer, ObjectData};
use ka::{Isometry3, Matrix3, Matrix4, Point2, Point3, Vector3};
use std::cell::RefCell;
use std::rc::Rc;

const PBR_VERTEX_SRC: &str = r###"#version 100
attribute vec3 position;
attribute vec2 tex_coord;
attribute vec3 normal;
attribute vec3 inst_tra;
attribute vec4 inst_color;
attribute vec3 inst_def_0;
attribute vec3 inst_def_1;
attribute vec3 inst_def_2;
uniform mat3 ntransform, scale;
uniform mat4 proj, view, transform;
uniform vec3 light_position;
varying vec3 local_light_position;
varying vec2 tex_coord_v;
varying vec3 normalInterp;
varying vec3 vertPos;
varying vec4 vertColor;
void main(){
    mat3 deformation = mat3(inst_def_0, inst_def_1, inst_def_2);
    vec4 pt = vec4(inst_tra, 0.0) + transform * vec4(deformation * scale * position, 1.0);
    gl_Position = proj * view * pt;
    vec4 vertPos4 = view * pt;
    vertPos = vec3(vertPos4) / vertPos4.w;
    normalInterp = mat3(view) * ntransform * normal;
    tex_coord_v = tex_coord;
    local_light_position = (view * vec4(light_position, 1.0)).xyz;
    vertColor = inst_color;
}
"###;

const PBR_FRAGMENT_SRC: &str = r###"#version 100
#ifdef GL_FRAGMENT_PRECISION_HIGH
   precision highp float;
#else
   precision mediump float;
#endif
varying vec3 local_light_position;
varying vec2 tex_coord_v;
varying vec3 normalInterp;
varying vec3 vertPos;
varying vec4 vertColor;
uniform vec3 color;
uniform sampler2D tex;
uniform float u_spec;
uniform float u_shine;
uniform float u_mirror;
uniform float u_alpha;
void main() {
  vec3 normal = normalize(normalInterp);
  vec3 lightDir = normalize(local_light_position - vertPos);
  float lambert = clamp(dot(lightDir, normal), 0.0, 1.0);
  vec3 viewDir = normalize(-vertPos);
  float ndv = clamp(dot(viewDir, normal), 0.0, 1.0);
  float specular = 0.0;
  if(lambert > 0.0) {
    vec3 halfDir = normalize(lightDir + viewDir);
    specular = pow(max(dot(halfDir, normal), 0.0), u_shine);
  }
  vec3 skyA = vec3(0.05, 0.06, 0.09);
  vec3 skyB = vec3(0.66, 0.75, 0.95);
  vec3 sky = mix(skyA, skyB, clamp(normal.y * 0.5 + 0.5, 0.0, 1.0));
  float fresnel = pow(1.0 - ndv, 3.0) * u_mirror;
  vec3 baseColor = vertColor.rgb * color;
  vec3 albedo = texture2D(tex, tex_coord_v).rgb;
  vec3 lit = albedo * baseColor * (0.45 + 0.95 * lambert);
  vec3 col = mix(lit, sky, fresnel);
  col += vec3(u_spec) * specular;
  gl_FragColor = vec4(col, u_alpha);
}
"###;

struct Matpbr {
    effect: Effect,
    pos: ShaderAttribute<Point3<f32>>,
    normal: ShaderAttribute<Vector3<f32>>,
    tex_coord: ShaderAttribute<Point2<f32>>,
    inst_tra: ShaderAttribute<Point3<f32>>,
    inst_color: ShaderAttribute<[f32; 4]>,
    inst_def0: ShaderAttribute<Vector3<f32>>,
    inst_def1: ShaderAttribute<Vector3<f32>>,
    inst_def2: ShaderAttribute<Vector3<f32>>,
    light: ShaderUniform<Point3<f32>>,
    color: ShaderUniform<Point3<f32>>,
    transform: ShaderUniform<Matrix4<f32>>,
    scale: ShaderUniform<Matrix3<f32>>,
    ntransform: ShaderUniform<Matrix3<f32>>,
    proj: ShaderUniform<Matrix4<f32>>,
    view: ShaderUniform<Matrix4<f32>>,
    u_spec: ShaderUniform<f32>,
    u_shine: ShaderUniform<f32>,
    u_mirror: ShaderUniform<f32>,
    u_alpha: ShaderUniform<f32>,
    spec: f32,
    shine: f32,
    mirror: f32,
    alpha: f32,
}

impl Matpbr {
    fn new(spec: f32, shine: f32, mirror: f32, alpha: f32) -> Option<Matpbr> {
        let mut effect = Effect::new_from_str(PBR_VERTEX_SRC, PBR_FRAGMENT_SRC);
        effect.use_program();
        Some(Matpbr {
            pos: effect.get_attrib("position")?,
            normal: effect.get_attrib("normal")?,
            tex_coord: effect.get_attrib("tex_coord")?,
            inst_tra: effect.get_attrib("inst_tra")?,
            inst_color: effect.get_attrib("inst_color")?,
            inst_def0: effect.get_attrib("inst_def_0")?,
            inst_def1: effect.get_attrib("inst_def_1")?,
            inst_def2: effect.get_attrib("inst_def_2")?,
            light: effect.get_uniform("light_position")?,
            color: effect.get_uniform("color")?,
            transform: effect.get_uniform("transform")?,
            scale: effect.get_uniform("scale")?,
            ntransform: effect.get_uniform("ntransform")?,
            proj: effect.get_uniform("proj")?,
            view: effect.get_uniform("view")?,
            u_spec: effect.get_uniform("u_spec")?,
            u_shine: effect.get_uniform("u_shine")?,
            u_mirror: effect.get_uniform("u_mirror")?,
            u_alpha: effect.get_uniform("u_alpha")?,
            spec,
            shine,
            mirror,
            alpha,
            effect,
        })
    }

    fn activate(&mut self) {
        self.effect.use_program();
        self.pos.enable();
        self.normal.enable();
        self.tex_coord.enable();
        self.inst_tra.enable();
        self.inst_color.enable();
        self.inst_def0.enable();
        self.inst_def1.enable();
        self.inst_def2.enable();
    }

    fn deactivate(&mut self) {
        self.pos.disable();
        self.normal.disable();
        self.tex_coord.disable();
        self.inst_tra.disable();
        self.inst_color.disable();
        self.inst_def0.disable();
        self.inst_def1.disable();
        self.inst_def2.disable();
    }
}

impl Material for Matpbr {
    fn render(
        &mut self,
        pass: usize,
        transform: &Isometry3<f32>,
        scale: &Vector3<f32>,
        camera: &mut dyn Camera,
        light: &Light,
        data: &ObjectData,
        instances: &mut InstancesBuffer,
        mesh: &mut Mesh,
    ) {
        let context = Context::get();
        self.activate();
        camera.upload(pass, &mut self.proj, &mut self.view);
        let pos = match *light {
            Light::Absolute(ref p) => *p,
            Light::StickToCamera => camera.eye(),
        };
        self.light.upload(&pos);
        let ftransform = transform.to_homogeneous();
        let fntransform = transform.rotation.to_rotation_matrix().into_inner();
        let fscale = Matrix3::from_diagonal(&Vector3::new(scale.x, scale.y, scale.z));
        let instance_count = instances.len() as i32;
        {
            self.transform.upload(&ftransform);
            self.ntransform.upload(&fntransform);
            self.scale.upload(&fscale);
            mesh.bind(&mut self.pos, &mut self.normal, &mut self.tex_coord);
            self.inst_tra.bind(&mut instances.positions);
            kiss3d::verify!(context.vertex_attrib_divisor(self.inst_tra.id(), 1));
            self.inst_color.bind(&mut instances.colors);
            kiss3d::verify!(context.vertex_attrib_divisor(self.inst_color.id(), 1));
            self.inst_def0.bind_sub_buffer(&mut instances.deformations, 2, 0);
            kiss3d::verify!(context.vertex_attrib_divisor(self.inst_def0.id(), 1));
            self.inst_def1.bind_sub_buffer(&mut instances.deformations, 2, 1);
            kiss3d::verify!(context.vertex_attrib_divisor(self.inst_def1.id(), 1));
            self.inst_def2.bind_sub_buffer(&mut instances.deformations, 2, 2);
            kiss3d::verify!(context.vertex_attrib_divisor(self.inst_def2.id(), 1));

            self.u_spec.upload(&self.spec);
            self.u_shine.upload(&self.shine);
            self.u_mirror.upload(&self.mirror);
            self.u_alpha.upload(&self.alpha);
            let transparente = self.alpha < 0.999;
            if transparente {
                kiss3d::verify!(context.enable(Context::BLEND));
                kiss3d::verify!(context.blend_func_separate(
                    Context::SRC_ALPHA,
                    Context::ONE_MINUS_SRC_ALPHA,
                    Context::SRC_ALPHA,
                    Context::ONE_MINUS_SRC_ALPHA,
                ));
            }

            kiss3d::verify!(context.active_texture(Context::TEXTURE0));
            kiss3d::verify!(context.bind_texture(Context::TEXTURE_2D, Some(&**data.texture())));

            if data.surface_rendering_active() {
                self.color.upload(data.color());
                if data.backface_culling_enabled() {
                    kiss3d::verify!(context.enable(Context::CULL_FACE));
                } else {
                    kiss3d::verify!(context.disable(Context::CULL_FACE));
                }
                let _ = kiss3d::verify!(context.polygon_mode(Context::FRONT_AND_BACK, Context::FILL));
                kiss3d::verify!(context.draw_elements_instanced(
                    Context::TRIANGLES, mesh.num_pts() as i32, VERTEX_INDEX_TYPE, 0, instance_count
                ));
            }
            if transparente {
                kiss3d::verify!(context.disable(Context::BLEND));
                self.u_alpha.upload(&1.0);
            }

            if data.lines_width() != 0.0 {
                self.color.upload(data.lines_color().unwrap_or(data.color()));
                kiss3d::verify!(context.disable(Context::CULL_FACE));
                kiss3d::ignore!(context.line_width(data.lines_width()));
                if kiss3d::verify!(context.polygon_mode(Context::FRONT_AND_BACK, Context::LINE)) {
                    kiss3d::verify!(context.draw_elements_instanced(
                        Context::TRIANGLES, mesh.num_pts() as i32, VERTEX_INDEX_TYPE, 0, instance_count
                    ));
                } else {
                    mesh.bind_edges();
                    kiss3d::verify!(context.draw_elements_instanced(
                        Context::LINES, mesh.num_pts() as i32 * 2, VERTEX_INDEX_TYPE, 0, instance_count
                    ));
                }
                context.line_width(1.0);
            }

            if data.points_size() != 0.0 {
                self.color.upload(data.color());
                kiss3d::verify!(context.disable(Context::CULL_FACE));
                context.point_size(data.points_size());
                if kiss3d::verify!(context.polygon_mode(Context::FRONT_AND_BACK, Context::POINT)) {
                    kiss3d::verify!(context.draw_elements_instanced(
                        Context::TRIANGLES, mesh.num_pts() as i32, VERTEX_INDEX_TYPE, 0, instance_count
                    ));
                } else {
                    kiss3d::verify!(context.draw_elements_instanced(
                        Context::POINTS, mesh.num_pts() as i32, VERTEX_INDEX_TYPE, 0, instance_count
                    ));
                }
                context.point_size(1.0);
            }
        }
        kiss3d::verify!(context.vertex_attrib_divisor(self.inst_tra.id(), 0));
        kiss3d::verify!(context.vertex_attrib_divisor(self.inst_color.id(), 0));
        kiss3d::verify!(context.vertex_attrib_divisor(self.inst_def0.id(), 0));
        kiss3d::verify!(context.vertex_attrib_divisor(self.inst_def1.id(), 0));
        kiss3d::verify!(context.vertex_attrib_divisor(self.inst_def2.id(), 0));
        mesh.unbind();
        self.deactivate();
    }
}

fn material_pbr(tipo: MaterialTipo, transparencia: f32) -> Option<Rc<RefCell<Box<dyn Material + 'static>>>> {
    let (spec, shine, mirror) = match tipo {
        MaterialTipo::Plastico => (0.35, 24.0, 0.08),
        MaterialTipo::Metal => (0.42, 90.0, 0.5),
        MaterialTipo::Mate => (0.04, 2.0, 0.0),
        MaterialTipo::Espejo => (0.35, 300.0, 0.85),
    };
    let alpha = (1.0 - transparencia).clamp(0.15, 1.0);
    Some(Rc::new(RefCell::new(Box::new(Matpbr::new(spec, shine, mirror, alpha)?))))
}

fn default_shape_data(tipo: FormaTipo) -> (Vec<[f32; 3]>, Vec<Vec<usize>>) {
    match tipo {
        FormaTipo::Cubo => (
            vec![
                [-0.5, -0.5, 0.5],
                [0.5, -0.5, 0.5],
                [0.5, 0.5, 0.5],
                [-0.5, 0.5, 0.5],
                [-0.5, -0.5, -0.5],
                [0.5, -0.5, -0.5],
                [0.5, 0.5, -0.5],
                [-0.5, 0.5, -0.5],
            ],
            vec![
                vec![0, 1, 2, 3],
                vec![5, 4, 7, 6],
                vec![4, 0, 3, 7],
                vec![1, 5, 6, 2],
                vec![3, 2, 6, 7],
                vec![4, 5, 1, 0],
            ],
        ),
        FormaTipo::Cuboide => (
            vec![
                [-1.0, -0.5, 0.25],
                [1.0, -0.5, 0.25],
                [1.0, 0.5, 0.25],
                [-1.0, 0.5, 0.25],
                [-1.0, -0.5, -0.25],
                [1.0, -0.5, -0.25],
                [1.0, 0.5, -0.25],
                [-1.0, 0.5, -0.25],
            ],
            vec![
                vec![0, 1, 2, 3],
                vec![5, 4, 7, 6],
                vec![4, 0, 3, 7],
                vec![1, 5, 6, 2],
                vec![3, 2, 6, 7],
                vec![4, 5, 1, 0],
            ],
        ),
        FormaTipo::PirCuadrada => (
            vec![
                [0.0, 0.5, 0.0],
                [-0.5, -0.5, 0.5],
                [0.5, -0.5, 0.5],
                [0.5, -0.5, -0.5],
                [-0.5, -0.5, -0.5],
            ],
            vec![
                vec![0, 1, 2],
                vec![0, 2, 3],
                vec![0, 3, 4],
                vec![0, 4, 1],
                vec![4, 3, 2, 1],
            ],
        ),
        FormaTipo::Esfera
        | FormaTipo::Cilindro
        | FormaTipo::Cono
        | FormaTipo::Capsula
        | FormaTipo::Plano => (vec![], vec![]),
    }
}

fn face_count(tipo: FormaTipo) -> usize {
    match tipo {
        FormaTipo::Cilindro => 6,
        FormaTipo::Cono => 5,
        FormaTipo::Esfera | FormaTipo::Capsula | FormaTipo::Plano => 1,
        FormaTipo::PirCuadrada => 5,
        _ => 6,
    }
}

fn crear_pixeles(color: &[u8; 4], tex_size: usize, count: usize) -> Vec<Vec<u8>> {
    (0..count)
        .map(|_| {
            let mut p = vec![0u8; tex_size * tex_size * 4];
            for px in p.chunks_exact_mut(4) {
                px.copy_from_slice(color);
            }
            p
        })
        .collect()
}

struct MallaDatos {
    pos: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    normals: Vec<[f32; 3]>,
    indices: Vec<u32>,
}

fn push_cuad(
    indices: &mut Vec<u32>,
    a: u32,
    b: u32,
    c: u32,
    d: u32,
) {
    indices.extend_from_slice(&[a, c, b, c, d, b]);
}

fn malla_esfera(radio: f32, seg: usize) -> MallaDatos {
    let seg = seg.max(3);
    let mut m = MallaDatos {
        pos: Vec::new(),
        uvs: Vec::new(),
        normals: Vec::new(),
        indices: Vec::new(),
    };
    m.pos.push([0.0, radio, 0.0]);
    m.uvs.push([0.5, 1.0]);
    m.normals.push([0.0, 1.0, 0.0]);
    let mut ring_start = Vec::new();
    for lat in 1..seg {
        let theta = lat as f32 * std::f32::consts::PI / seg as f32;
        let (sin_t, cos_t) = theta.sin_cos();
        ring_start.push(m.pos.len() as u32);
        for lon in 0..=seg {
            let phi = lon as f32 * 2.0 * std::f32::consts::PI / seg as f32;
            let (sin_p, cos_p) = phi.sin_cos();
            let p = [cos_p * sin_t * radio, cos_t * radio, sin_p * sin_t * radio];
            m.pos.push(p);
            m.uvs.push([lon as f32 / seg as f32, 1.0 - lat as f32 / seg as f32]);
            let l = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt().max(1e-12);
            m.normals.push([p[0] / l, p[1] / l, p[2] / l]);
        }
    }
    let south = m.pos.len() as u32;
    m.pos.push([0.0, -radio, 0.0]);
    m.uvs.push([0.5, 0.0]);
    m.normals.push([0.0, -1.0, 0.0]);
    let first = ring_start[0];
    for lon in 0..seg {
        let l = lon as u32;
        m.indices.extend_from_slice(&[0, first + l + 1, first + l]);
    }
    for lat in 0..ring_start.len() - 1 {
        let r0 = ring_start[lat];
        let r1 = ring_start[lat + 1];
        for lon in 0..seg {
            let l = lon as u32;
            push_cuad(&mut m.indices, r0 + l, r1 + l, r0 + l + 1, r1 + l + 1);
        }
    }
    let last = *ring_start.last().unwrap();
    for lon in 0..seg {
        let l = lon as u32;
        m.indices.extend_from_slice(&[south, last + l, last + l + 1]);
    }
    m
}

fn mallas_cilindro(radio: f32, alto: f32, seg: usize) -> Vec<MallaDatos> {
    let seg = seg.max(4);
    let q = (seg / 4).max(2);
    let n = q * 4;
    let pi = std::f32::consts::PI;
    let y_top = alto / 2.0;
    let y_bot = -alto / 2.0;
    let mut out = Vec::with_capacity(6);
    for (sign, y) in [(1.0f32, y_top), (-1.0, y_bot)] {
        let mut m = MallaDatos {
            pos: Vec::new(),
            uvs: Vec::new(),
            normals: Vec::new(),
            indices: Vec::new(),
        };
        m.pos.push([0.0, y, 0.0]);
        m.uvs.push([0.5, 0.5]);
        m.normals.push([0.0, sign, 0.0]);
        for lon in 0..=n {
            let phi = lon as f32 / n as f32 * 2.0 * pi;
            let (sin_p, cos_p) = phi.sin_cos();
            m.pos.push([cos_p * radio, y, sin_p * radio]);
            m.uvs.push([0.5 + 0.5 * cos_p, 0.5 + 0.5 * sin_p]);
            m.normals.push([0.0, sign, 0.0]);
        }
        for lon in 0..n {
            let a = (1 + lon) as u32;
            if sign > 0.0 {
                m.indices.extend_from_slice(&[0, a + 1, a]);
            } else {
                m.indices.extend_from_slice(&[0, a, a + 1]);
            }
        }
        out.push(m);
    }
    for k in 0..4 {
        let phi0 = k as f32 * 0.5 * pi;
        let mut m = MallaDatos {
            pos: Vec::new(),
            uvs: Vec::new(),
            normals: Vec::new(),
            indices: Vec::new(),
        };
        for lon in 0..=q {
            let phi = phi0 + lon as f32 / q as f32 * 0.5 * pi;
            let (sin_p, cos_p) = phi.sin_cos();
            m.pos.push([cos_p * radio, y_top, sin_p * radio]);
            m.uvs.push([lon as f32 / q as f32, 1.0]);
            m.normals.push([cos_p, 0.0, sin_p]);
        }
        for lon in 0..=q {
            let phi = phi0 + lon as f32 / q as f32 * 0.5 * pi;
            let (sin_p, cos_p) = phi.sin_cos();
            m.pos.push([cos_p * radio, y_bot, sin_p * radio]);
            m.uvs.push([lon as f32 / q as f32, 0.0]);
            m.normals.push([cos_p, 0.0, sin_p]);
        }
        let q32 = (q + 1) as u32;
        for lon in 0..q {
            let l = lon as u32;
            push_cuad(&mut m.indices, l, l + q32, l + 1, l + q32 + 1);
        }
        out.push(m);
    }
    out
}

fn mallas_cono(radio: f32, alto: f32, seg: usize) -> Vec<MallaDatos> {
    let seg = seg.max(4);
    let q = (seg / 4).max(2);
    let n = q * 4;
    let pi = std::f32::consts::PI;
    let y_top = alto / 2.0;
    let y_bot = -alto / 2.0;
    let slant = (radio * radio + alto * alto).sqrt().max(1e-12);
    let nx = alto / slant;
    let ny = radio / slant;
    let mut out = Vec::with_capacity(5);
    let mut m = MallaDatos {
        pos: Vec::new(),
        uvs: Vec::new(),
        normals: Vec::new(),
        indices: Vec::new(),
    };
    m.pos.push([0.0, y_bot, 0.0]);
    m.uvs.push([0.5, 0.5]);
    m.normals.push([0.0, -1.0, 0.0]);
    for lon in 0..=n {
        let phi = lon as f32 / n as f32 * 2.0 * pi;
        let (sin_p, cos_p) = phi.sin_cos();
        m.pos.push([cos_p * radio, y_bot, sin_p * radio]);
        m.uvs.push([0.5 + 0.5 * cos_p, 0.5 + 0.5 * sin_p]);
        m.normals.push([0.0, -1.0, 0.0]);
    }
    for lon in 0..n {
        let a = (1 + lon) as u32;
        m.indices.extend_from_slice(&[0, a, a + 1]);
    }
    out.push(m);
    for k in 0..4 {
        let phi0 = k as f32 * 0.5 * pi;
        let mut m = MallaDatos {
            pos: Vec::new(),
            uvs: Vec::new(),
            normals: Vec::new(),
            indices: Vec::new(),
        };
        m.pos.push([0.0, y_top, 0.0]);
        m.uvs.push([0.5, 1.0]);
        m.normals.push([0.0, 1.0, 0.0]);
        for lon in 0..=q {
            let phi = phi0 + lon as f32 / q as f32 * 0.5 * pi;
            let (sin_p, cos_p) = phi.sin_cos();
            m.pos.push([cos_p * radio, y_bot, sin_p * radio]);
            m.uvs.push([lon as f32 / q as f32, 0.0]);
            m.normals.push([nx * cos_p, ny, nx * sin_p]);
        }
        for lon in 0..q {
            let a = (1 + lon) as u32;
            m.indices.extend_from_slice(&[0, a + 1, a]);
        }
        out.push(m);
    }
    out
}

fn malla_capsula(ancho: f32, alto: f32, seg: usize) -> MallaDatos {
    let seg = seg.max(3);
    let mut m = MallaDatos {
        pos: Vec::new(),
        uvs: Vec::new(),
        normals: Vec::new(),
        indices: Vec::new(),
    };
    let radio = (ancho / 2.0).min(alto / 2.0);
    let mid = (alto - 2.0 * radio).max(0.0);
    let y_mid_top = mid / 2.0;
    let y_mid_bot = -mid / 2.0;
    let dome_seg = (seg / 2).max(2);
    let mut rings: Vec<(f32, f32, f32)> = Vec::new();
    rings.push((y_mid_top + radio, 0.0, 1.0));
    for j in 1..dome_seg {
        let t = j as f32 / dome_seg as f32 * std::f32::consts::FRAC_PI_2;
        rings.push((y_mid_top + radio * t.cos(), radio * t.sin(), 1.0 - 0.5 * j as f32 / dome_seg as f32));
    }
    rings.push((y_mid_top, radio, 0.5));
    if mid > 0.0 {
        rings.push((y_mid_bot, radio, 0.5));
    }
    for j in 1..dome_seg {
        let t = (dome_seg - j) as f32 / dome_seg as f32 * std::f32::consts::FRAC_PI_2;
        rings.push((y_mid_bot - radio * t.cos(), radio * t.sin(), 0.5 - 0.5 * j as f32 / dome_seg as f32));
    }
    rings.push((y_mid_bot - radio, 0.0, 0.0));
    let mut ring_start = Vec::new();
    for &(y, r, v) in rings.iter() {
        ring_start.push(m.pos.len() as u32);
        let (n_r, n_y) = if r <= 1e-6 {
            if y > 0.0 { (0.0, 1.0) } else { (0.0, -1.0) }
        } else if y >= y_mid_top - 1e-6 {
            ((r / radio).min(1.0), ((y - y_mid_top) / radio).clamp(-1.0, 1.0))
        } else if y <= y_mid_bot + 1e-6 {
            ((r / radio).min(1.0), ((y - y_mid_bot) / radio).clamp(-1.0, 1.0))
        } else {
            (1.0, 0.0)
        };
        for lon in 0..=seg {
            let phi = lon as f32 * 2.0 * std::f32::consts::PI / seg as f32;
            let (sin_p, cos_p) = phi.sin_cos();
            m.pos.push([cos_p * r, y, sin_p * r]);
            m.uvs.push([lon as f32 / seg as f32, v]);
            m.normals.push([n_r * cos_p, n_y, n_r * sin_p]);
        }
    }
    for i in 0..rings.len() - 1 {
        let a = ring_start[i];
        let b = ring_start[i + 1];
        let top_pole = rings[i].1 <= 1e-6;
        let bot_pole = rings[i + 1].1 <= 1e-6;
        if top_pole {
            for lon in 0..seg {
                let l = lon as u32;
                m.indices.extend_from_slice(&[a, b + l + 1, b + l]);
            }
        } else if bot_pole {
            for lon in 0..seg {
                let l = lon as u32;
                m.indices.extend_from_slice(&[b, a + l, a + l + 1]);
            }
        } else {
            for lon in 0..seg {
                let l = lon as u32;
                push_cuad(&mut m.indices, a + l, b + l, a + l + 1, b + l + 1);
            }
        }
    }
    m
}

fn malla_plano(ancho: f32, alto: f32) -> MallaDatos {
    let w = ancho / 2.0;
    let h = alto / 2.0;
    MallaDatos {
        pos: vec![
            [-w, -h, 0.0],
            [w, -h, 0.0],
            [w, h, 0.0],
            [-w, h, 0.0],
        ],
        uvs: vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
        normals: vec![[0.0, 0.0, 1.0]; 4],
        indices: vec![0, 1, 2, 0, 2, 3],
    }
}

fn malla_forma(forma: &FormaData) -> Option<MallaDatos> {
    match forma.tipo {
        FormaTipo::Esfera => Some(malla_esfera(forma.esfera_radio, forma.segmentos)),
        FormaTipo::Capsula => {
            Some(malla_capsula(forma.capsula_ancho, forma.capsula_alto, forma.segmentos))
        }
        FormaTipo::Plano => Some(malla_plano(forma.plano_ancho, forma.plano_alto)),
        _ => None,
    }
}

fn mallas_forma(forma: &FormaData) -> Option<Vec<MallaDatos>> {
    match forma.tipo {
        FormaTipo::Cilindro => {
            Some(mallas_cilindro(forma.cilindro_radio, forma.cilindro_alto, forma.segmentos))
        }
        FormaTipo::Cono => Some(mallas_cono(forma.cono_radio, forma.cono_alto, forma.segmentos)),
        _ => malla_forma(forma).map(|m| vec![m]),
    }
}

#[derive(Clone, Serialize, Deserialize)]
struct FormaData {
    nombre: String,
    tipo: FormaTipo,
    posicion: [f32; 3],
    rotacion: [f32; 3],
    cubo_escala: f32,
    cuboide_ancho: f32,
    cuboide_alto: f32,
    cuboide_profundo: f32,
    esfera_radio: f32,
    segmentos: usize,
    cilindro_radio: f32,
    cilindro_alto: f32,
    cono_radio: f32,
    cono_alto: f32,
    capsula_ancho: f32,
    capsula_alto: f32,
    plano_ancho: f32,
    plano_alto: f32,
    pyramid_scale: f32,
    shape_vertices: Vec<[f32; 3]>,
    shape_faces: Vec<Vec<usize>>,
    cara_sel: usize,
    pixeles: Vec<Vec<u8>>,
    material: MaterialTipo,
    #[serde(default)]
    grupo: Option<usize>,
#[serde(default)]
transparencia: f32,
    #[serde(default)]
    bloqueada: bool,
    #[serde(default)]
    oculta: bool,
}

impl FormaData {
    fn new(tipo: FormaTipo, tex_size: usize, nombre: String) -> Self {
        let (shape_vertices, shape_faces) = default_shape_data(tipo);
        let count = face_count(tipo);
        let pixeles = crear_pixeles(&[128, 128, 128, 255], tex_size, count);
        Self {
            nombre,
            tipo,
            posicion: [0.0, 0.0, 0.0],
            rotacion: [0.0, 0.0, 0.0],
            cubo_escala: 1.0,
            cuboide_ancho: 2.0,
            cuboide_alto: 1.0,
            cuboide_profundo: 0.5,
            esfera_radio: 1.0,
            segmentos: 24,
            cilindro_radio: 0.5,
            cilindro_alto: 1.0,
            cono_radio: 0.5,
            cono_alto: 1.0,
            capsula_ancho: 0.6,
            capsula_alto: 1.0,
            plano_ancho: 2.0,
            plano_alto: 2.0,
            pyramid_scale: 1.0,
            shape_vertices,
            shape_faces,
            cara_sel: 0,
            material: MaterialTipo::Plastico,
            grupo: None,
            transparencia: 0.0,
            bloqueada: false,
            oculta: false,
            pixeles,
        }
    }

    fn actualizar_vertices(&mut self) {
        match self.tipo {
            FormaTipo::Cubo => {
                let s = self.cubo_escala / 2.0;
                let v = &mut self.shape_vertices;
                v[0] = [-s, -s, s];
                v[1] = [s, -s, s];
                v[2] = [s, s, s];
                v[3] = [-s, s, s];
                v[4] = [-s, -s, -s];
                v[5] = [s, -s, -s];
                v[6] = [s, s, -s];
                v[7] = [-s, s, -s];
            }
            FormaTipo::Cuboide => {
                let (w, h, d) = (
                    self.cuboide_ancho / 2.0,
                    self.cuboide_alto / 2.0,
                    self.cuboide_profundo / 2.0,
                );
                let v = &mut self.shape_vertices;
                v[0] = [-w, -h, d];
                v[1] = [w, -h, d];
                v[2] = [w, h, d];
                v[3] = [-w, h, d];
                v[4] = [-w, -h, -d];
                v[5] = [w, -h, -d];
                v[6] = [w, h, -d];
                v[7] = [-w, h, -d];
            }
            FormaTipo::PirCuadrada => {
                let base = default_shape_data(self.tipo);
                let s = self.pyramid_scale;
                for (i, v) in self.shape_vertices.iter_mut().enumerate() {
                    v[0] = base.0[i][0] * s;
                    v[1] = base.0[i][1] * s;
                    v[2] = base.0[i][2] * s;
                }
            }
            FormaTipo::Esfera
            | FormaTipo::Cilindro
            | FormaTipo::Cono
            | FormaTipo::Capsula
            | FormaTipo::Plano => {}
        }
    }
}

struct SharedState {
    formas: Vec<FormaData>,
    forma_activa: usize,
    color: [u8; 4],
    tam_pincel: usize,
    dirty: bool,
    mensaje: String,
    tex_size: usize,
    res_dirty: bool,
    paleta: [[u8; 4]; 8],
    fill_color: [u8; 4],
    project_name: String,
    project_path: Option<PathBuf>,
    shape_dirty: bool,
    activa_dirty: bool,
    nuevo_tex_size: usize,
    grupos: Vec<GrupoData>,
}

impl SharedState {
    fn new(_colores_ini: &[[u8; 4]; 6], tex_size: usize, paleta: [[u8; 4]; 8]) -> Self {
        let forma = FormaData::new(FormaTipo::Cubo, tex_size, "Forma 1".into());
        Self {
            formas: vec![forma],
            forma_activa: 0,
            color: [0, 0, 0, 255],
            tam_pincel: 1,
            dirty: true,
            mensaje: String::new(),
            tex_size,
            res_dirty: false,
            paleta,
            fill_color: [128, 128, 128, 255],
            project_name: String::from("mi_proyecto"),
            project_path: None,
            shape_dirty: true,
            activa_dirty: true,
            nuevo_tex_size: tex_size,
            grupos: Vec::new(),
        }
    }

    fn forma(&self) -> &FormaData {
        &self.formas[self.forma_activa]
    }

    fn crear_grupo(&mut self) -> usize {
        let n = self.grupos.len() + 1;
        let mut g = GrupoData::new();
        g.nombre = format!("Grupo {}", n);
        self.grupos.push(g);
        self.grupos.len() - 1
    }

    fn borrar_grupo(&mut self, idx: usize) {
        if idx >= self.grupos.len() {
            return;
        }
        self.grupos.remove(idx);
        for f in self.formas.iter_mut() {
            f.grupo = match f.grupo {
                Some(g) if g == idx => None,
                Some(g) if g > idx => Some(g - 1),
                other => other,
            };
        }
    }

    fn asignar_grupo(&mut self, forma_idx: usize, grupo: Option<usize>) {
        if let Some(f) = self.formas.get_mut(forma_idx) {
            f.grupo = grupo;
        }
    }
}

fn redimensionar_pixeles(pixeles: &mut [Vec<u8>], old_size: usize, new_size: usize) {
    for cara in pixeles.iter_mut() {
        let mut nuevos = vec![0u8; new_size * new_size * 4];
        for y in 0..new_size {
            for x in 0..new_size {
                let oy = y.min(old_size - 1);
                let ox = x.min(old_size - 1);
                let src = (oy * old_size + ox) * 4;
                let dst = (y * new_size + x) * 4;
                nuevos[dst..dst + 4].copy_from_slice(&cara[src..src + 4]);
            }
        }
        *cara = nuevos;
    }
}

fn desktop_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    for dir in &["Escritorio", "Desktop", "桌面"] {
        let p = PathBuf::from(&home).join(dir);
        if p.is_dir() {
            return p;
        }
    }
    PathBuf::from(&home)
}

fn sanitize_filename(s: &str) -> String {
    s.chars().map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect()
}

fn align4(n: usize) -> usize {
    (n + 3) & !3
}

fn push_buffer_view(
    buffer: &mut Vec<u8>,
    views: &mut Vec<serde_json::Value>,
    data: &[u8],
    target: Option<u32>,
) -> usize {
    while !buffer.len().is_multiple_of(4) {
        buffer.push(0);
    }
    let offset = buffer.len();
    buffer.extend_from_slice(data);
    let mut view = serde_json::json!({
        "buffer": 0,
        "byteOffset": offset,
        "byteLength": data.len(),
    });
    if let Some(t) = target {
        view["target"] = serde_json::json!(t);
    }
    views.push(view);
    views.len() - 1
}

fn push_accessor(
    accessors: &mut Vec<serde_json::Value>,
    view: usize,
    component_type: u32,
    count: usize,
    ty: &str,
    min: Option<[f32; 3]>,
    max: Option<[f32; 3]>,
) -> usize {
    let mut acc = serde_json::json!({
        "bufferView": view,
        "componentType": component_type,
        "count": count,
        "type": ty,
    });
    if let (Some(mi), Some(ma)) = (min, max) {
        acc["min"] = serde_json::json!([mi[0], mi[1], mi[2]]);
        acc["max"] = serde_json::json!([ma[0], ma[1], ma[2]]);
    }
    accessors.push(acc);
    accessors.len() - 1
}

fn add_png_image(
    buffer: &mut Vec<u8>,
    views: &mut Vec<serde_json::Value>,
    images: &mut Vec<serde_json::Value>,
    pixeles: &[u8],
    tex_size: u32,
    nombre: &str,
) -> Result<usize, String> {
    let img = image::RgbaImage::from_raw(tex_size, tex_size, pixeles.to_vec())
        .ok_or_else(|| "textura inválida".to_string())?;
    let mut cursor = std::io::Cursor::new(Vec::new());
    img.write_to(&mut cursor, image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    let png = cursor.into_inner();
    let view = push_buffer_view(buffer, views, &png, None);
    images.push(serde_json::json!({
        "bufferView": view,
        "mimeType": "image/png",
        "name": nombre,
    }));
    Ok(images.len() - 1)
}

#[allow(clippy::too_many_arguments)]
fn add_material_gltf(
    buffer: &mut Vec<u8>,
    views: &mut Vec<serde_json::Value>,
    images: &mut Vec<serde_json::Value>,
    textures: &mut Vec<serde_json::Value>,
    materials: &mut Vec<serde_json::Value>,
    pixeles: &[u8],
    tex_size: u32,
    nombre: &str,
    material: MaterialTipo,
    transparencia: f32,
) -> Result<usize, String> {
    let img = add_png_image(buffer, views, images, pixeles, tex_size, nombre)?;
    let tex = textures.len();
    textures.push(serde_json::json!({ "source": img, "sampler": 0 }));
    let (metallic, roughness) = material.factor_pgr();
    let alpha = (1.0 - transparencia).clamp(0.0, 1.0);
    let mut pbr = serde_json::json!({
        "baseColorTexture": { "index": tex },
        "metallicFactor": metallic,
        "roughnessFactor": roughness,
    });
    if transparencia > 0.0 {
        pbr["baseColorFactor"] = serde_json::json!([1.0, 1.0, 1.0, alpha]);
    }
    let mut mtl = serde_json::json!({
        "name": nombre,
        "pbrMetallicRoughness": pbr,
    });
    if transparencia > 0.0 {
        mtl["alphaMode"] = serde_json::json!("BLEND");
    }
    let mat = materials.len();
    materials.push(mtl);
    Ok(mat)
}

fn normal_de_cara(v: &[[f32; 3]]) -> [f32; 3] {
    let (a, b, c) = (v[0], v[1], v[2]);
    let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let w = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
    let n = [
        u[1] * w[2] - u[2] * w[1],
        u[2] * w[0] - u[0] * w[2],
        u[0] * w[1] - u[1] * w[0],
    ];
    let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-12);
    [n[0] / len, n[1] / len, n[2] / len]
}

fn euler_a_quaternion(roll: f32, pitch: f32, yaw: f32) -> [f32; 4] {
    let (sr, cr) = (roll * 0.5).sin_cos();
    let (sp, cp) = (pitch * 0.5).sin_cos();
    let (sy, cy) = (yaw * 0.5).sin_cos();
    [
        sr * cp * cy - cr * sp * sy,
        cr * sp * cy + sr * cp * sy,
        cr * cp * sy - sr * sp * cy,
        cr * cp * cy + sr * sp * sy,
    ]
}

fn add_primitive_gltf(
    buffer: &mut Vec<u8>,
    views: &mut Vec<serde_json::Value>,
    accessors: &mut Vec<serde_json::Value>,
    positions: &[[f32; 3]],
    uvs: &[[f32; 2]],
    normals: &[[f32; 3]],
    indices: &[u32],
) -> Result<serde_json::Value, String> {
    let mut pos_bytes = Vec::new();
    for p in positions {
        for c in p {
            pos_bytes.extend_from_slice(&c.to_le_bytes());
        }
    }
    let mut uv_bytes = Vec::new();
    for uv in uvs {
        for c in uv {
            uv_bytes.extend_from_slice(&c.to_le_bytes());
        }
    }
    let mut nor_bytes = Vec::new();
    for n in normals {
        for c in n {
            nor_bytes.extend_from_slice(&c.to_le_bytes());
        }
    }
    let mut idx_bytes = Vec::new();
    for i in indices {
        idx_bytes.extend_from_slice(&i.to_le_bytes());
    }
    let view_pos = push_buffer_view(buffer, views, &pos_bytes, Some(34962));
    let view_uv = push_buffer_view(buffer, views, &uv_bytes, Some(34962));
    let view_nor = push_buffer_view(buffer, views, &nor_bytes, Some(34962));
    let view_idx = push_buffer_view(buffer, views, &idx_bytes, Some(34963));
    let mut min = [f32::MAX; 3];
    let mut max = [f32::MIN; 3];
    for p in positions {
        for k in 0..3 {
            min[k] = min[k].min(p[k]);
            max[k] = max[k].max(p[k]);
        }
    }
    let a_pos =
        push_accessor(accessors, view_pos, 5126, positions.len(), "VEC3", Some(min), Some(max));
    let a_uv = push_accessor(accessors, view_uv, 5126, uvs.len(), "VEC2", None, None);
    let a_nor = push_accessor(accessors, view_nor, 5126, normals.len(), "VEC3", None, None);
    let a_idx = push_accessor(accessors, view_idx, 5125, indices.len(), "SCALAR", None, None);
    Ok(serde_json::json!({
        "attributes": {
            "POSITION": a_pos,
            "NORMAL": a_nor,
            "TEXCOORD_0": a_uv,
        },
        "indices": a_idx,
        "mode": 4,
    }))
}

fn exportar_glb(state: &SharedState, path: &Path) -> Result<String, String> {
    let tex_size = state.tex_size as u32;
    let mut buffer: Vec<u8> = Vec::new();
    let mut views: Vec<serde_json::Value> = Vec::new();
    let mut accessors: Vec<serde_json::Value> = Vec::new();
    let mut images: Vec<serde_json::Value> = Vec::new();
    let mut textures: Vec<serde_json::Value> = Vec::new();
    let mut materials: Vec<serde_json::Value> = Vec::new();
    let mut nodes: Vec<serde_json::Value> = Vec::new();
    let mut meshes: Vec<serde_json::Value> = Vec::new();
    let mut scene_nodes: Vec<u32> = Vec::new();

    for forma in &state.formas {
        let base = sanitize_filename(&forma.nombre);
        let mut prims: Vec<serde_json::Value> = Vec::new();
        match forma.tipo {
            FormaTipo::Esfera
            | FormaTipo::Cilindro
            | FormaTipo::Cono
            | FormaTipo::Capsula
            | FormaTipo::Plano => {
                let mallas = mallas_forma(forma).ok_or_else(|| "forma sin malla".to_string())?;
                for (i, malla) in mallas.iter().enumerate() {
                    let nombre_mtl = if mallas.len() > 1 {
                        format!("{}_face_{}", base, i)
                    } else {
                        base.clone()
                    };
                    let mat = add_material_gltf(
                        &mut buffer, &mut views, &mut images, &mut textures, &mut materials,
                        &forma.pixeles[i.min(forma.pixeles.len() - 1)], tex_size, &nombre_mtl,
                        forma.material, forma.transparencia,
                    )?;
                    if forma.tipo == FormaTipo::Plano {
                        materials[mat]["doubleSided"] = serde_json::json!(true);
                    }
                    let mut prim = add_primitive_gltf(
                        &mut buffer, &mut views, &mut accessors,
                        &malla.pos, &malla.uvs, &malla.normals, &malla.indices,
                    )?;
                    prim["material"] = serde_json::json!(mat);
                    prims.push(prim);
                }
            }
            _ => {
                let mut centro = [0.0f32; 3];
                for v in &forma.shape_vertices {
                    for (c, vk) in centro.iter_mut().zip(v.iter()) {
                        *c += vk;
                    }
                }
                let n_verts = forma.shape_vertices.len().max(1) as f32;
                for c in &mut centro {
                    *c /= n_verts;
                }
                for (i, face) in forma.shape_faces.iter().enumerate() {
                    let positions: Vec<[f32; 3]> =
                        face.iter().map(|&ix| forma.shape_vertices[ix]).collect();
                    let mut normal = normal_de_cara(&positions);
                    let mut fc = [0.0f32; 3];
                    for p in &positions {
                        for (fc_k, p_k) in fc.iter_mut().zip(p.iter()) {
                            *fc_k += p_k;
                        }
                    }
                    let n_fc = positions.len().max(1) as f32;
                    for fc_k in &mut fc {
                        *fc_k /= n_fc;
                    }
                    if (fc[0] - centro[0]) * normal[0]
                        + (fc[1] - centro[1]) * normal[1]
                        + (fc[2] - centro[2]) * normal[2]
                        < 0.0
                    {
                        for n in &mut normal {
                            *n = -*n;
                        }
                    }
                    let uvs = match face.len() {
                        3 => vec![[0.0, 0.0], [1.0, 0.0], [0.5, 1.0]],
                        _ => vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
                    };
                    let normals = vec![normal; positions.len()];
                    let indices: Vec<u32> = match face.len() {
                        3 => vec![0, 1, 2],
                        4 => vec![0, 1, 2, 0, 2, 3],
                        _ => return Err("cara con más de 4 vértices".into()),
                    };
                    let nombre_mtl = format!("{}_face_{}", base, i);
                    let mat = add_material_gltf(
                        &mut buffer, &mut views, &mut images, &mut textures, &mut materials,
                        &forma.pixeles[i], tex_size, &nombre_mtl, forma.material,
                        forma.transparencia,
                    )?;
                    let mut prim = add_primitive_gltf(
                        &mut buffer, &mut views, &mut accessors,
                        &positions, &uvs, &normals, &indices,
                    )?;
                    prim["material"] = serde_json::json!(mat);
                    prims.push(prim);
                }
            }
        }
        let mesh_idx = meshes.len();
        meshes.push(serde_json::json!({
            "name": forma.nombre,
            "primitives": prims,
        }));
        let q = euler_a_quaternion(forma.rotacion[0], forma.rotacion[1], forma.rotacion[2]);
        let node_idx = nodes.len();
        nodes.push(serde_json::json!({
            "name": forma.nombre,
            "translation": [forma.posicion[0], forma.posicion[1], forma.posicion[2]],
            "rotation": q,
            "mesh": mesh_idx,
        }));
        scene_nodes.push(node_idx as u32);
    }

    let data = ProjectData {
        formas: state.formas.clone(),
        forma_activa: state.forma_activa,
        tex_size: state.tex_size,
        paleta: state.paleta,
        fill_color: state.fill_color,
        grupos: state.grupos.clone(),
    };
    let root = serde_json::json!({
        "asset": { "version": "2.0", "generator": "Simplified 3D 0.2.0" },
        "scene": 0,
        "scenes": [{ "nodes": scene_nodes }],
        "nodes": nodes,
        "meshes": meshes,
        "materials": materials,
        "textures": textures,
        "samplers": [{
            "magFilter": 9728,
            "minFilter": 9728,
            "wrapS": 33071,
            "wrapT": 33071,
        }],
        "images": images,
        "accessors": accessors,
        "bufferViews": views,
        "buffers": [{ "byteLength": buffer.len() }],
        "extras": data,
    });

    let json_str = serde_json::to_string(&root).map_err(|e| e.to_string())?;
    let json_len = align4(json_str.len());
    let bin_len = align4(buffer.len());

    let mut out: Vec<u8> = Vec::new();
    out.extend_from_slice(b"glTF");
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&((12 + 8 + json_len + 8 + bin_len) as u32).to_le_bytes());
    out.extend_from_slice(&(json_len as u32).to_le_bytes());
    out.extend_from_slice(b"JSON");
    out.extend_from_slice(json_str.as_bytes());
    out.extend(std::iter::repeat_n(b' ', json_len - json_str.len()));
    out.extend_from_slice(&(bin_len as u32).to_le_bytes());
    out.extend_from_slice(b"BIN\0");
    out.extend_from_slice(&buffer);
    out.extend(std::iter::repeat_n(0u8, bin_len - buffer.len()));

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(path, &out).map_err(|e| e.to_string())?;
    Ok(format!("Guardado en: {}", path.display()))
}

#[derive(Serialize, Deserialize)]
struct ProjectData {
    formas: Vec<FormaData>,
    forma_activa: usize,
    tex_size: usize,
    paleta: [[u8; 4]; 8],
    fill_color: [u8; 4],
    #[serde(default)]
    grupos: Vec<GrupoData>,
}

fn importar_glb(path: &Path) -> Result<ProjectData, String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    if bytes.len() < 12 || &bytes[0..4] != b"glTF" {
        return Err("El archivo no es un .glb".into());
    }
    let mut offset = 12usize;
    let mut json_str = String::new();
    while offset + 8 <= bytes.len() {
        let len = u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize;
        let ctype = u32::from_le_bytes(bytes[offset + 4..offset + 8].try_into().unwrap());
        offset += 8;
        if offset + len > bytes.len() {
            return Err("GLB corrupto".into());
        }
        if ctype == 0x4E4F534A {
            json_str = String::from_utf8_lossy(&bytes[offset..offset + len])
                .trim()
                .to_string();
        }
        offset += len;
    }
    let root: serde_json::Value = serde_json::from_str(&json_str).map_err(|e| e.to_string())?;
    let extras = root
        .get("extras")
        .ok_or_else(|| "No es un proyecto de Simplified 3D (falta extras)".to_string())?;
    serde_json::from_value(extras.clone())
        .map_err(|e| format!("Error al leer el proyecto: {}", e))
}

fn aplicar_proyecto(state: &mut SharedState, data: ProjectData, nombre: String, path: PathBuf) {
    state.formas = data.formas;
    state.forma_activa = data.forma_activa.min(state.formas.len().saturating_sub(1));
    state.tex_size = data.tex_size;
    state.nuevo_tex_size = data.tex_size;
    state.paleta = data.paleta;
    state.fill_color = data.fill_color;
    state.grupos = data.grupos;
    state.project_name = nombre;
    state.project_path = Some(path);
    state.shape_dirty = true;
    state.activa_dirty = true;
    state.dirty = true;
    state.res_dirty = true;
}

fn listar_proyectos() -> Vec<String> {
    let dir = desktop_dir().join("modelador_proyectos");
    if !dir.is_dir() {
        return Vec::new();
    }
    let mut nombres = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.extension().is_some_and(|e| e == "glb")
                && let Some(stem) = p.file_stem().and_then(|s| s.to_str())
            {
                nombres.push(stem.to_string());
            }
        }
    }
    nombres.sort();
    nombres
}

fn main() {
    let colores_ini: [[u8; 4]; 6] = [
        [255, 100, 100, 255],
        [100, 255, 100, 255],
        [100, 100, 255, 255],
        [255, 255, 100, 255],
        [255, 100, 255, 255],
        [100, 255, 255, 255],
    ];
    let paleta_ini: [[u8; 4]; 8] = [
        [0, 0, 0, 255],
        [255, 0, 0, 255],
        [0, 200, 0, 255],
        [0, 0, 255, 255],
        [255, 255, 0, 255],
        [255, 0, 255, 255],
        [0, 200, 200, 255],
        [255, 255, 255, 255],
    ];

    let state = Arc::new(Mutex::new(SharedState::new(&colores_ini, 8, paleta_ini)));
    let state_3d = state.clone();
    let hilo_3d = std::thread::spawn(move || usar_kiss3d(state_3d));

    let app = UiApp {
        state,
        show_color_picker: false,
        picker_color: [0, 0, 0],
        custom_colors: Vec::new(),
        tab: 0,
        subtab: 0,
        grupo_activo: 0,
        autosave_pend: false,
        autosave_start: 0.0,
        prev_dirty: false,
        autosave_activo: false,
        editing_paleta: None,
        show_fill_picker: false,
        proyectos: listar_proyectos(),
        btn_start: 0.0,
        btn_last: 0.0,
        lang: Lang::Es,
    };
    let icon_img = image::load_from_memory(include_bytes!("../Icono/Simplified3D.png"))
        .map(|i| i.into_rgba8());
    let icon = icon_img.map(|img| {
        let w = img.width();
        let h = img.height();
        egui::IconData {
            rgba: img.into_raw(),
            width: w,
            height: h,
        }
    });
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size(egui::vec2(400.0, 560.0))
            .with_icon(icon.unwrap_or_default()),
        ..Default::default()
    };
    eframe::run_native(
        "Simplified 3D",
        opts,
        Box::new(|_| Ok(Box::new(app))),
    )
    .ok();

    hilo_3d.join().ok();
}

// ============================================================
// UI (egui)
// ============================================================
#[derive(Clone, Copy, PartialEq)]
enum Lang { Es, En, Fr }

struct Texts {
    lienzo: &'static str,
    forma: &'static str,
    avanzado: &'static str,
    proyecto: &'static str,
    pintando: &'static str,
    cara: &'static str,
    pixel: &'static str,
    color: &'static str,
    selector_color: &'static str,
    colores_guardados: &'static str,
    pincel: &'static str,
    limpiar_cara: &'static str,
    llenar_todo: &'static str,
    bloquear: &'static str,
    desbloquear: &'static str,
    ocultar: &'static str,
    mostrar: &'static str,
    bloqueada: &'static str,
    formas: &'static str,
    cubo: &'static str,
    cuboide: &'static str,
    piramide: &'static str,
    esfera: &'static str,
    cilindro: &'static str,
    cono: &'static str,
    capsula: &'static str,
    plano: &'static str,
    agregar: &'static str,
    forma_activa: &'static str,
    nombre: &'static str,
    posicion: &'static str,
    rotacion: &'static str,
    x: &'static str,
    y: &'static str,
    z: &'static str,
    escala: &'static str,
    ancho: &'static str,
    alto: &'static str,
    profundo: &'static str,
    radio: &'static str,
    radio_base: &'static str,
    segmentos: &'static str,
    material: &'static str,
    mat_plastico: &'static str,
    mat_metal: &'static str,
    mat_mate: &'static str,
    mat_espejo: &'static str,
    transparencia: &'static str,
    basico: &'static str,
    agrupaciones: &'static str,
    nuevo_grupo: &'static str,
    anadir_a_grupo: &'static str,
    quitar_del_grupo: &'static str,
    miembros: &'static str,
    bloquear_direccion: &'static str,
    tam_lienzo: &'static str,
    colores_paleta: &'static str,
    color_relleno: &'static str,
    guardar: &'static str,
    guardar_cambios: &'static str,
    cargar: &'static str,
    proyectos_guardados: &'static str,
    ninguno: &'static str,
    autoguardado: &'static str,
}

const TEXTS: [Texts; 3] = [
    Texts {
        lienzo: "Lienzo", forma: "Forma", avanzado: "Avanzado", proyecto: "Proyecto",
        pintando: "Pintando:", cara: "Cara", pixel: "Pixel:",
        color: "Color:", selector_color: "Selector de color",
        colores_guardados: "Colores guardados (clic para usar, clic derecho para quitar):",
        pincel: "Pincel", limpiar_cara: "Limpiar cara", llenar_todo: "Llenar todo",
        bloquear: "Bloquear", desbloquear: "Desbloquear", ocultar: "Ocultar", mostrar: "Mostrar",
        bloqueada: "Forma bloqueada (desbloquéala para editarla)",
        formas: "Formas", cubo: "Cubo", cuboide: "Cuboide", piramide: "Pirámide",
        esfera: "Esfera", cilindro: "Cilindro", cono: "Cono", capsula: "Cápsula",
        plano: "Plano", agregar: "Agregar", forma_activa: "Forma activa",
        nombre: "Nombre:", posicion: "Posición:", rotacion: "Rotación (grados):",
        x: "X:", y: "Y:", z: "Z:", escala: "Escala:", ancho: "Ancho (X):",
        alto: "Alto (Y):", profundo: "Profundo (Z):", radio: "Radio:",
        radio_base: "Radio base:", segmentos: "Segmentos:", tam_lienzo: "Tamaño del lienzo:",
        material: "Material:", mat_plastico: "Plástico", mat_metal: "Metal",
        mat_mate: "Mate", mat_espejo: "Espejo",
        transparencia: "Transparencia:",
        basico: "Básico", agrupaciones: "Agrupaciones",
        nuevo_grupo: "Nuevo grupo",
        anadir_a_grupo: "Agregar forma al grupo", quitar_del_grupo: "Quitar", miembros: "Miembros:",
        bloquear_direccion: "El grupo mueve todas sus formas juntas",
        colores_paleta: "Colores por defecto (clic para editar):",
        color_relleno: "Color de relleno (Limpiar cara):",
        guardar: "Guardar proyecto", guardar_cambios: "Guardar cambios",
        cargar: "Cargar proyecto", proyectos_guardados: "Proyectos guardados:",
        ninguno: "(ninguno)", autoguardado: "Autoguardado:",
    },
    Texts {
        lienzo: "Canvas", forma: "Shape", avanzado: "Advanced", proyecto: "Project",
        pintando: "Painting:", cara: "Face", pixel: "Pixel:",
        color: "Color:", selector_color: "Color picker",
        colores_guardados: "Saved colors (click to use, right-click to remove):",
        pincel: "Brush", limpiar_cara: "Clear face", llenar_todo: "Fill all",
        bloquear: "Lock", desbloquear: "Unlock", ocultar: "Hide", mostrar: "Show",
        bloqueada: "Shape locked (unlock it to edit)",
        formas: "Shapes", cubo: "Cube", cuboide: "Cuboid", piramide: "Pyramid",
        esfera: "Sphere", cilindro: "Cylinder", cono: "Cone", capsula: "Capsule",
        plano: "Plane", agregar: "Add", forma_activa: "Active shape",
        nombre: "Name:", posicion: "Position:", rotacion: "Rotation (degrees):",
        x: "X:", y: "Y:", z: "Z:", escala: "Scale:", ancho: "Width (X):",
        alto: "Height (Y):", profundo: "Depth (Z):", radio: "Radius:",
        radio_base: "Base radius:", segmentos: "Segments:", tam_lienzo: "Canvas size:",
        material: "Material:", mat_plastico: "Plastic", mat_metal: "Metal",
        mat_mate: "Matte", mat_espejo: "Mirror",
        transparencia: "Transparency:",
        basico: "Basic", agrupaciones: "Groups",
        nuevo_grupo: "New group",
        anadir_a_grupo: "Add shape to group", quitar_del_grupo: "Remove", miembros: "Members:",
        bloquear_direccion: "The group moves all its shapes together",
        colores_paleta: "Default colors (click to edit):",
        color_relleno: "Fill color (Clear face):",
        guardar: "Save project", guardar_cambios: "Save changes",
        cargar: "Load project", proyectos_guardados: "Saved projects:",
        ninguno: "(none)", autoguardado: "Autosave:",
    },
    Texts {
        lienzo: "Toile", forma: "Forme", avanzado: "Avancé", proyecto: "Projet",
        pintando: "Peinture:", cara: "Face", pixel: "Pixel:",
        color: "Couleur:", selector_color: "Sélecteur de couleur",
        colores_guardados: "Couleurs sauvegardées (clic pour utiliser, clic droit pour retirer):",
        pincel: "Pinceau", limpiar_cara: "Effacer la face", llenar_todo: "Remplir tout",
        bloquear: "Verrouiller", desbloquear: "Déverrouiller", ocultar: "Masquer", mostrar: "Afficher",
        bloqueada: "Forme verrouillée (déverrouillez-la pour éditer)",
        formas: "Formes", cubo: "Cube", cuboide: "Cuboïde", piramide: "Pyramide",
        esfera: "Sphère", cilindro: "Cylindre", cono: "Cône", capsula: "Capsule",
        plano: "Plan", agregar: "Ajouter", forma_activa: "Forme active",
        nombre: "Nom:", posicion: "Position:", rotacion: "Rotation (degrés):",
        x: "X:", y: "Y:", z: "Z:", escala: "Échelle:", ancho: "Largeur (X):",
        alto: "Hauteur (Y):", profundo: "Profondeur (Z):", radio: "Rayon:",
        radio_base: "Rayon de base:", segmentos: "Segments:", tam_lienzo: "Taille de la toile:",
        material: "Matériau:", mat_plastico: "Plastique", mat_metal: "Métal",
        mat_mate: "Mat", mat_espejo: "Miroir",
        transparencia: "Transparence:",
        basico: "Basique", agrupaciones: "Groupes",
        nuevo_grupo: "Nouveau groupe",
        anadir_a_grupo: "Ajouter la forme au groupe", quitar_del_grupo: "Retirer", miembros: "Membres:",
        bloquear_direccion: "Le groupe déplace toutes ses formes ensemble",
        colores_paleta: "Couleurs par défaut (clic pour éditer):",
        color_relleno: "Couleur de remplissage (Effacer la face):",
        guardar: "Sauvegarder le projet", guardar_cambios: "Sauvegarder les modifications",
        cargar: "Charger le projet", proyectos_guardados: "Projets sauvegardés:",
        ninguno: "(aucun)", autoguardado: "Autosave :",
    },
];
struct UiApp {
    state: Arc<Mutex<SharedState>>,
    show_color_picker: bool,
    picker_color: [u8; 3],
    custom_colors: Vec<[u8; 4]>,
    tab: usize,
    subtab: usize,
    grupo_activo: usize,
    autosave_pend: bool,
    autosave_start: f64,
    prev_dirty: bool,
    autosave_activo: bool,
    editing_paleta: Option<usize>,
    show_fill_picker: bool,
    proyectos: Vec<String>,
    btn_start: f64,
    btn_last: f64,
    lang: Lang,
}

impl UiApp {
    fn tx(&self) -> &'static Texts {
        &TEXTS[self.lang as usize]
    }
}

impl eframe::App for UiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Autosave: si hay un proyecto ya guardado y hubo cambios, se guarda
        // solo tras un pequeño retraso de "debounce" para no escribir seguido
        let now = ctx.input(|i| i.time);
        {
            let mut st = self.state.lock().unwrap();
            if st.project_path.is_some() && !self.prev_dirty && st.dirty {
                self.autosave_pend = true;
                self.autosave_start = now;
            }
            self.prev_dirty = st.dirty;
            if self.autosave_activo
                && self.autosave_pend
                && !st.dirty
                && now - self.autosave_start >= 1.5
                && let Some(path) = st.project_path.clone()
                && let Ok(_msg) = exportar_glb(&st, &path)
            {
                self.autosave_pend = false;
                st.mensaje = "Autoguardado".to_string();
            }
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.selectable_label(self.tab == 0, self.tx().lienzo).clicked() {
                    self.tab = 0;
                }
                if ui.selectable_label(self.tab == 1, self.tx().forma).clicked() {
                    self.tab = 1;
                }
                if ui.selectable_label(self.tab == 2, self.tx().avanzado).clicked() {
                    self.tab = 2;
                }
                if ui.selectable_label(self.tab == 3, self.tx().proyecto).clicked() {
                    self.tab = 3;
                }
            });
            ui.separator();

            let state_arc = self.state.clone();
            let mut state = state_arc.lock().unwrap();

            let num_keys = [
                egui::Key::Num1,
                egui::Key::Num2,
                egui::Key::Num3,
                egui::Key::Num4,
                egui::Key::Num5,
                egui::Key::Num6,
                egui::Key::Num7,
                egui::Key::Num8,
            ];
            for (i, key) in num_keys.iter().enumerate() {
                if ui.input(|input| input.key_pressed(*key)) {
                    state.color = state.paleta[i];
                }
            }

            match self.tab {
                0 => self.ui_lienzo(ui, &mut state),
                1 => self.ui_forma_tab(ui, &mut state),
                2 => self.ui_avanzado(ui, &mut state),
                3 => self.ui_proyecto(ui, &mut state),
                _ => {}
            }
        });
        ctx.request_repaint();
    }
}

impl UiApp {
    fn btn_held(&mut self, ui: &egui::Ui, response: &egui::Response) -> bool {
        if response.is_pointer_button_down_on() {
            let now = ui.input(|i| i.time);
            if self.btn_start == 0.0 {
                self.btn_start = now;
                self.btn_last = now;
                return true;
            }
            let held = now - self.btn_start;
            let since_last = now - self.btn_last;
            if (held < 0.5 && since_last >= 0.4) || (held >= 0.5 && since_last >= 0.08) {
                self.btn_last = now;
                return true;
            }
            false
        } else {
            self.btn_start = 0.0;
            self.btn_last = 0.0;
            false
        }
    }

    fn btn_repeat(&mut self, ui: &mut egui::Ui, text: &str) -> bool {
        let btn = ui.add(egui::Button::new(text));
        self.btn_held(ui, &btn)
    }

    fn ui_lienzo(&mut self, ui: &mut egui::Ui, state: &mut SharedState) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.label(format!("{} {}", self.tx().pintando, state.forma().nombre));
            ui.separator();

            let a = state.forma_activa;
            let bloqueada = state.formas[a].bloqueada;
            let forma = &mut state.formas[a];

            match forma.tipo {
                FormaTipo::Cubo | FormaTipo::Cuboide | FormaTipo::PirCuadrada
                | FormaTipo::Cilindro | FormaTipo::Cono => {
                    ui.horizontal(|ui| {
                        for i in 0..forma.pixeles.len() {
                            let label = format!("{} {}", self.tx().cara, i + 1);
                            if ui.selectable_label(forma.cara_sel == i, &label).clicked() {
                                forma.cara_sel = i;
                                state.dirty = true;
                            }
                        }
                    });
                }
                FormaTipo::Esfera => {
                    ui.label("Textura de la esfera (proyección equirrectangular):");
                }
                _ => {}
            }

            ui.separator();

            let tex_size = state.tex_size;
            let target = 280.0f32;
            let celda = (target / tex_size as f32).clamp(10.0, 35.0);
            let (resp, painter) = ui.allocate_painter(
                egui::Vec2::new(tex_size as f32 * celda, tex_size as f32 * celda),
                egui::Sense::click_and_drag(),
            );
            let pixeles = &state.formas[a].pixeles[state.formas[a]
                .cara_sel
                .min(state.formas[a].pixeles.len() - 1)];
            for y in 0..tex_size {
                for x in 0..tex_size {
                    let i = (y * tex_size + x) * 4;
                    let min = egui::pos2(
                        resp.rect.min.x + x as f32 * celda,
                        resp.rect.min.y + y as f32 * celda,
                    );
                    let rect =
                        egui::Rect::from_min_max(min, egui::pos2(min.x + celda, min.y + celda));
                    let c = egui::Color32::from_rgb(pixeles[i], pixeles[i + 1], pixeles[i + 2]);
                    painter.rect_filled(rect, 0.0, c);
                    painter.rect_stroke(rect, 0.0, egui::Stroke::new(1.0, egui::Color32::GRAY));
                }
            }

            if let Some(pos) = resp.hover_pos() {
                let px = ((pos.x - resp.rect.min.x) / celda) as usize;
                let py = ((pos.y - resp.rect.min.y) / celda) as usize;
                if px < tex_size && py < tex_size {
                    let is_painting = ui.input(|i| i.pointer.any_down());
                    ui.label(format!("{} ({}, {})", self.tx().pixel, px, py));

                    let center = egui::pos2(
                        resp.rect.min.x + (px as f32 + 0.5) * celda,
                        resp.rect.min.y + (py as f32 + 0.5) * celda,
                    );
                    let r = state.tam_pincel as f32 * celda / 2.0;
                    if r > 0.0 {
                        painter.circle_stroke(
                            center,
                            r,
                            egui::Stroke::new(2.0, egui::Color32::WHITE),
                        );
                    }

                    if is_painting && !bloqueada {
                        let t = state.tam_pincel;
                        let color = state.color;
                        let cs = state.formas[a]
                            .cara_sel
                            .min(state.formas[a].pixeles.len() - 1);
                        for y in py.saturating_sub(t / 2)..(py + t.div_ceil(2)).min(tex_size) {
                            for x in px.saturating_sub(t / 2)..(px + t.div_ceil(2)).min(tex_size) {
                                let i = (y * tex_size + x) * 4;
                                state.formas[a].pixeles[cs][i..i + 4].copy_from_slice(&color);
                            }
                        }
                        state.dirty = true;
                    }

                    if resp.secondary_clicked() {
                        let i = (py * tex_size + px) * 4;
                        let cs = state.formas[a]
                            .cara_sel
                            .min(state.formas[a].pixeles.len() - 1);
                        state.color = [
                            state.formas[a].pixeles[cs][i],
                            state.formas[a].pixeles[cs][i + 1],
                            state.formas[a].pixeles[cs][i + 2],
                            255,
                        ];
                    }
                }
            }

            ui.separator();

            ui.label(self.tx().color);
            let paleta = state.paleta;
            ui.horizontal(|ui| {
                for &c in &paleta {
                    let color = egui::Color32::from_rgb(c[0], c[1], c[2]);
                    let (id, rect) = ui.allocate_space(egui::vec2(24.0, 24.0));
                    let resp = ui.interact(rect, id, egui::Sense::click());
                    ui.painter().rect_filled(rect, 2.0, color);
                    ui.painter().rect_stroke(
                        rect,
                        2.0,
                        egui::Stroke::new(1.0, egui::Color32::DARK_GRAY),
                    );
                    if resp.clicked() {
                        state.color = c;
                    }
                }
                if ui.button("+").clicked() {
                    self.show_color_picker = true;
                    self.picker_color = [state.color[0], state.color[1], state.color[2]];
                }
            });

            if self.show_color_picker {
                let mut open = true;
                egui::Window::new(self.tx().selector_color)
                    .open(&mut open)
                    .default_width(300.0)
                    .show(ui.ctx(), |ui| {
                        ui.spacing_mut().slider_width = ui.available_width() - 10.0;
                        let mut c32 = egui::Color32::from_rgb(
                            self.picker_color[0],
                            self.picker_color[1],
                            self.picker_color[2],
                        );
                        if egui::color_picker::color_picker_color32(
                            ui,
                            &mut c32,
                            egui::color_picker::Alpha::Opaque,
                        ) {
                            self.picker_color = [c32[0], c32[1], c32[2]];
                            state.color = [c32[0], c32[1], c32[2], 255];
                        }
                    });
                if !open {
                    self.show_color_picker = false;
                    let color = state.color;
                    if !self.custom_colors.contains(&color) {
                        self.custom_colors.push(color);
                    }
                }
            }

            if !self.custom_colors.is_empty() {
                ui.label(self.tx().colores_guardados);
                ui.horizontal(|ui| {
                    let mut quitar = None;
                    for (i, &c) in self.custom_colors.iter().enumerate() {
                        let color = egui::Color32::from_rgb(c[0], c[1], c[2]);
                        let (id, rect) = ui.allocate_space(egui::vec2(24.0, 24.0));
                        let resp = ui.interact(rect, id, egui::Sense::click());
                        ui.painter().rect_filled(rect, 2.0, color);
                        ui.painter().rect_stroke(
                            rect,
                            2.0,
                            egui::Stroke::new(1.0, egui::Color32::DARK_GRAY),
                        );
                        if resp.clicked() {
                            state.color = c;
                        }
                        if resp.secondary_clicked() {
                            quitar = Some(i);
                        }
                    }
                    if let Some(i) = quitar {
                        self.custom_colors.remove(i);
                    }
                });
            }

            ui.separator();
            ui.add(egui::Slider::new(&mut state.tam_pincel, 0..=8).text(self.tx().pincel));

            ui.horizontal(|ui| {
                if ui.add_enabled(!bloqueada, egui::Button::new(self.tx().limpiar_cara)).clicked()
                {
                    let c = state.fill_color;
                    let cs = state.formas[a]
                        .cara_sel
                        .min(state.formas[a].pixeles.len() - 1);
                    for px in state.formas[a].pixeles[cs].chunks_exact_mut(4) {
                        px.copy_from_slice(&c);
                    }
                    state.dirty = true;
                }
                if ui.add_enabled(!bloqueada, egui::Button::new(self.tx().llenar_todo)).clicked() {
                    let c = state.color;
                    for cara in state.formas[a].pixeles.iter_mut() {
                        for px in cara.chunks_exact_mut(4) {
                            px.copy_from_slice(&c);
                        }
                    }
                    state.dirty = true;
                }
            });

            if !state.mensaje.is_empty() {
                ui.label(&state.mensaje);
            }
        });
    }

    fn ui_forma_tab(&mut self, ui: &mut egui::Ui, state: &mut SharedState) {
        ui.horizontal(|ui| {
            if ui.selectable_label(self.subtab == 0, self.tx().basico).clicked() {
                self.subtab = 0;
            }
            if ui.selectable_label(self.subtab == 1, self.tx().agrupaciones).clicked() {
                self.subtab = 1;
            }
        });
        ui.separator();
        match self.subtab {
            0 => self.ui_forma(ui, state),
            1 => self.ui_grupos(ui, state),
            _ => {}
        }
    }

    fn ui_forma(&mut self, ui: &mut egui::Ui, state: &mut SharedState) {
        ui.spacing_mut().interact_size.y = 28.0;
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.heading(self.tx().formas);
            ui.separator();

            let mut borrar: Option<usize> = None;
            for i in 0..state.formas.len() {
                let activa = state.forma_activa == i;
                ui.horizontal(|ui| {
                    if ui
                        .selectable_label(activa, &state.formas[i].nombre)
                        .clicked()
                    {
                        state.forma_activa = i;
                        state.activa_dirty = true;
                    }
                    let badge = match state.formas[i].tipo {
                        FormaTipo::Cubo => self.tx().cubo,
                        FormaTipo::Cuboide => self.tx().cuboide,
                        FormaTipo::PirCuadrada => self.tx().piramide,
                        FormaTipo::Esfera => self.tx().esfera,
                        FormaTipo::Cilindro => self.tx().cilindro,
                        FormaTipo::Cono => self.tx().cono,
                        FormaTipo::Capsula => self.tx().capsula,
                        FormaTipo::Plano => self.tx().plano,
                    };
                    ui.label(badge);
                    if ui.button("X").clicked() && state.formas.len() > 1 {
                        borrar = Some(i);
                    }
                    if ui.button("+").clicked() {
                        let mut f = state.formas[i].clone();
                        f.nombre = format!("{} (copia)", f.nombre);
                        f.posicion[0] += 1.5;
                        state.formas.insert(i + 1, f);
                        state.forma_activa = i + 1;
                        state.shape_dirty = true;
                        state.activa_dirty = true;
                    }
                });
            }
            if let Some(i) = borrar {
                if i < state.forma_activa {
                    state.forma_activa -= 1;
                } else if i == state.forma_activa {
                    state.forma_activa =
                        state.forma_activa.min(state.formas.len().saturating_sub(2));
                }
                state.formas.remove(i);
                state.shape_dirty = true;
                state.activa_dirty = true;
            }

            // Add button at bottom, with dropdown
            ui.horizontal(|ui| {
                egui::menu::menu_button(ui, egui::RichText::new(self.tx().agregar).size(18.0), |ui| {
                    fn add_forma(state: &mut SharedState, tipo: FormaTipo) {
                        let n = state.formas.len() + 1;
                        let mut f = FormaData::new(tipo, state.tex_size, format!("Forma {}", n));
                        f.posicion[0] = state.formas.len() as f32 * 1.5;
                        state.formas.push(f);
                        state.forma_activa = state.formas.len() - 1;
                        state.shape_dirty = true;
                        state.activa_dirty = true;
                    }
                    if ui.button(self.tx().cubo).clicked() { add_forma(state, FormaTipo::Cubo); ui.close_menu(); }
                    if ui.button(self.tx().cuboide).clicked() { add_forma(state, FormaTipo::Cuboide); ui.close_menu(); }
                    if ui.button(self.tx().piramide).clicked() { add_forma(state, FormaTipo::PirCuadrada); ui.close_menu(); }
                    if ui.button(self.tx().esfera).clicked() { add_forma(state, FormaTipo::Esfera); ui.close_menu(); }
                    if ui.button(self.tx().cilindro).clicked() { add_forma(state, FormaTipo::Cilindro); ui.close_menu(); }
                    if ui.button(self.tx().cono).clicked() { add_forma(state, FormaTipo::Cono); ui.close_menu(); }
                    if ui.button(self.tx().capsula).clicked() { add_forma(state, FormaTipo::Capsula); ui.close_menu(); }
                    if ui.button(self.tx().plano).clicked() { add_forma(state, FormaTipo::Plano); ui.close_menu(); }
                });
            });

            ui.separator();
            ui.heading(self.tx().forma_activa);
            ui.separator();

            if state.formas.is_empty() {
                return;
            }
            let a = state.forma_activa;
            let forma = &mut state.formas[a];
            let mut changed = false;

            ui.horizontal(|ui| {
                if ui.button(if forma.bloqueada { self.tx().desbloquear } else { self.tx().bloquear }).clicked()
                {
                    forma.bloqueada = !forma.bloqueada;
                }
                if ui.button(if forma.oculta { self.tx().mostrar } else { self.tx().ocultar })
                    .clicked()
                {
                    forma.oculta = !forma.oculta;
                }
            });
            ui.separator();

            if forma.bloqueada {
                ui.label(self.tx().bloqueada);
                return;
            }

            ui.horizontal(|ui| {
                ui.label("Nombre:");
                ui.text_edit_singleline(&mut forma.nombre);
            });

            ui.separator();

            // Posición
            ui.label(self.tx().posicion);
            ui.horizontal(|ui| {
                let change = |v: &mut f32, delta: f32| {
                    *v = ((*v + delta) * 10.0).round() / 10.0;
                };
                ui.label(self.tx().x);
                if self.btn_repeat(ui, "−") { change(&mut forma.posicion[0], -0.1); }
                ui.add(egui::DragValue::new(&mut forma.posicion[0]).speed(0.05).range(-10.0..=10.0));
                if self.btn_repeat(ui, "+") { change(&mut forma.posicion[0], 0.1); }
                ui.label(self.tx().y);
                if self.btn_repeat(ui, "−") { change(&mut forma.posicion[1], -0.1); }
                ui.add(egui::DragValue::new(&mut forma.posicion[1]).speed(0.05).range(-10.0..=10.0));
                if self.btn_repeat(ui, "+") { change(&mut forma.posicion[1], 0.1); }
                ui.label(self.tx().z);
                if self.btn_repeat(ui, "−") { change(&mut forma.posicion[2], -0.1); }
                ui.add(egui::DragValue::new(&mut forma.posicion[2]).speed(0.05).range(-10.0..=10.0));
                if self.btn_repeat(ui, "+") { change(&mut forma.posicion[2], 0.1); }
            });

            ui.separator();

            // Rotación (grados en UI, radianes internos)
            ui.label(self.tx().rotacion);
            let mut rot_deg = [
                forma.rotacion[0].to_degrees(),
                forma.rotacion[1].to_degrees(),
                forma.rotacion[2].to_degrees(),
            ];
            let old_rot = rot_deg;
            ui.horizontal(|ui| {
                let change = |v: &mut f32, delta: f32| {
                    *v = ((*v + delta) * 10.0).round() / 10.0;
                };
                ui.label(self.tx().x);
                if self.btn_repeat(ui, "−") { change(&mut rot_deg[0], -1.0); }
                ui.add(egui::DragValue::new(&mut rot_deg[0]).speed(0.5).range(-180.0..=180.0).suffix("°"));
                if self.btn_repeat(ui, "+") { change(&mut rot_deg[0], 1.0); }
                ui.label(self.tx().y);
                if self.btn_repeat(ui, "−") { change(&mut rot_deg[1], -1.0); }
                ui.add(egui::DragValue::new(&mut rot_deg[1]).speed(0.5).range(-180.0..=180.0).suffix("°"));
                if self.btn_repeat(ui, "+") { change(&mut rot_deg[1], 1.0); }
                ui.label(self.tx().z);
                if self.btn_repeat(ui, "−") { change(&mut rot_deg[2], -1.0); }
                ui.add(egui::DragValue::new(&mut rot_deg[2]).speed(0.5).range(-180.0..=180.0).suffix("°"));
                if self.btn_repeat(ui, "+") { change(&mut rot_deg[2], 1.0); }
            });
            if rot_deg != old_rot {
                forma.rotacion = [
                    rot_deg[0].to_radians(),
                    rot_deg[1].to_radians(),
                    rot_deg[2].to_radians(),
                ];
            }

            ui.separator();

            // Parámetros según tipo
            match forma.tipo {
                FormaTipo::Cubo => {
                    let old = forma.cubo_escala;
                    ui.horizontal(|ui| {
                        ui.label(self.tx().escala);
                        if self.btn_repeat(ui, "−") {
                            forma.cubo_escala = (forma.cubo_escala - 0.1).max(0.1);
                        }
                        ui.add(egui::DragValue::new(&mut forma.cubo_escala).speed(0.05).range(0.1..=5.0));
                        if self.btn_repeat(ui, "+") {
                            forma.cubo_escala = (forma.cubo_escala + 0.1).min(5.0);
                        }
                    });
                    if forma.cubo_escala != old {
                        forma.actualizar_vertices();
                        changed = true;
                    }
                }
                FormaTipo::Cuboide => {
                    let old = (
                        forma.cuboide_ancho,
                        forma.cuboide_alto,
                        forma.cuboide_profundo,
                    );
                    ui.horizontal(|ui| {
                        ui.label(self.tx().ancho);
                        if self.btn_repeat(ui, "−") {
                            forma.cuboide_ancho = (forma.cuboide_ancho - 0.1).max(0.1);
                        }
                        ui.add(egui::DragValue::new(&mut forma.cuboide_ancho).speed(0.05).range(0.1..=5.0));
                        if self.btn_repeat(ui, "+") {
                            forma.cuboide_ancho = (forma.cuboide_ancho + 0.1).min(5.0);
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label(self.tx().alto);
                        if self.btn_repeat(ui, "−") {
                            forma.cuboide_alto = (forma.cuboide_alto - 0.1).max(0.1);
                        }
                        ui.add(egui::DragValue::new(&mut forma.cuboide_alto).speed(0.05).range(0.1..=5.0));
                        if self.btn_repeat(ui, "+") {
                            forma.cuboide_alto = (forma.cuboide_alto + 0.1).min(5.0);
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label(self.tx().profundo);
                        if self.btn_repeat(ui, "−") {
                            forma.cuboide_profundo = (forma.cuboide_profundo - 0.1).max(0.1);
                        }
                        ui.add(egui::DragValue::new(&mut forma.cuboide_profundo).speed(0.05).range(0.1..=5.0));
                        if self.btn_repeat(ui, "+") {
                            forma.cuboide_profundo = (forma.cuboide_profundo + 0.1).min(5.0);
                        }
                    });
                    if (
                        forma.cuboide_ancho,
                        forma.cuboide_alto,
                        forma.cuboide_profundo,
                    ) != old
                    {
                        forma.actualizar_vertices();
                        changed = true;
                    }
                }
                FormaTipo::PirCuadrada => {
                    let old = forma.pyramid_scale;
                    ui.horizontal(|ui| {
                        ui.label(self.tx().escala);
                        if self.btn_repeat(ui, "−") {
                            forma.pyramid_scale = (forma.pyramid_scale - 0.1).max(0.1);
                        }
                        ui.add(egui::DragValue::new(&mut forma.pyramid_scale).speed(0.05).range(0.1..=5.0));
                        if self.btn_repeat(ui, "+") {
                            forma.pyramid_scale = (forma.pyramid_scale + 0.1).min(5.0);
                        }
                    });
                    if forma.pyramid_scale != old {
                        forma.actualizar_vertices();
                        changed = true;
                    }
                }
                FormaTipo::Esfera => {
                    let old_r = forma.esfera_radio;
                    let old_s = forma.segmentos;
                    ui.horizontal(|ui| {
                        ui.label(self.tx().radio);
                        if self.btn_repeat(ui, "−") {
                            forma.esfera_radio = (forma.esfera_radio - 0.1).max(0.1);
                        }
                        ui.add(egui::DragValue::new(&mut forma.esfera_radio).speed(0.05).range(0.1..=5.0));
                        if self.btn_repeat(ui, "+") {
                            forma.esfera_radio = (forma.esfera_radio + 0.1).min(5.0);
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label(self.tx().segmentos);
                        ui.add(egui::Slider::new(&mut forma.segmentos, 8..=64).text(""));
                    });
                    if forma.esfera_radio != old_r || forma.segmentos != old_s {
                        changed = true;
                    }
                }
                FormaTipo::Cilindro => {
                    let old_r = forma.cilindro_radio;
                    let old_h = forma.cilindro_alto;
                    let old_s = forma.segmentos;
                    ui.horizontal(|ui| {
                        ui.label(self.tx().radio);
                        if self.btn_repeat(ui, "−") {
                            forma.cilindro_radio = (forma.cilindro_radio - 0.1).max(0.1);
                        }
                        ui.add(egui::DragValue::new(&mut forma.cilindro_radio).speed(0.05).range(0.1..=5.0));
                        if self.btn_repeat(ui, "+") {
                            forma.cilindro_radio = (forma.cilindro_radio + 0.1).min(5.0);
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label(self.tx().alto);
                        if self.btn_repeat(ui, "−") {
                            forma.cilindro_alto = (forma.cilindro_alto - 0.1).max(0.1);
                        }
                        ui.add(egui::DragValue::new(&mut forma.cilindro_alto).speed(0.05).range(0.1..=5.0));
                        if self.btn_repeat(ui, "+") {
                            forma.cilindro_alto = (forma.cilindro_alto + 0.1).min(5.0);
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label(self.tx().segmentos);
                        ui.add(egui::Slider::new(&mut forma.segmentos, 8..=64).text(""));
                    });
                    if forma.cilindro_radio != old_r
                        || forma.cilindro_alto != old_h
                        || forma.segmentos != old_s
                    {
                        changed = true;
                    }
                }
                FormaTipo::Cono => {
                    let old_r = forma.cono_radio;
                    let old_h = forma.cono_alto;
                    let old_s = forma.segmentos;
                    ui.horizontal(|ui| {
                        ui.label(self.tx().radio_base);
                        if self.btn_repeat(ui, "−") {
                            forma.cono_radio = (forma.cono_radio - 0.1).max(0.1);
                        }
                        ui.add(egui::DragValue::new(&mut forma.cono_radio).speed(0.05).range(0.1..=5.0));
                        if self.btn_repeat(ui, "+") {
                            forma.cono_radio = (forma.cono_radio + 0.1).min(5.0);
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label(self.tx().alto);
                        if self.btn_repeat(ui, "−") {
                            forma.cono_alto = (forma.cono_alto - 0.1).max(0.1);
                        }
                        ui.add(egui::DragValue::new(&mut forma.cono_alto).speed(0.05).range(0.1..=5.0));
                        if self.btn_repeat(ui, "+") {
                            forma.cono_alto = (forma.cono_alto + 0.1).min(5.0);
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label(self.tx().segmentos);
                        ui.add(egui::Slider::new(&mut forma.segmentos, 8..=64).text(""));
                    });
                    if forma.cono_radio != old_r
                        || forma.cono_alto != old_h
                        || forma.segmentos != old_s
                    {
                        changed = true;
                    }
                }
                FormaTipo::Capsula => {
                    let old_w = forma.capsula_ancho;
                    let old_h = forma.capsula_alto;
                    let old_s = forma.segmentos;
                    ui.horizontal(|ui| {
                        ui.label(self.tx().ancho);
                        if self.btn_repeat(ui, "−") {
                            forma.capsula_ancho = (forma.capsula_ancho - 0.1).max(0.1);
                        }
                        ui.add(egui::DragValue::new(&mut forma.capsula_ancho).speed(0.05).range(0.1..=5.0));
                        if self.btn_repeat(ui, "+") {
                            forma.capsula_ancho = (forma.capsula_ancho + 0.1).min(5.0);
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label(self.tx().alto);
                        if self.btn_repeat(ui, "−") {
                            forma.capsula_alto = (forma.capsula_alto - 0.1).max(0.1);
                        }
                        ui.add(egui::DragValue::new(&mut forma.capsula_alto).speed(0.05).range(0.1..=5.0));
                        if self.btn_repeat(ui, "+") {
                            forma.capsula_alto = (forma.capsula_alto + 0.1).min(5.0);
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label(self.tx().segmentos);
                        ui.add(egui::Slider::new(&mut forma.segmentos, 8..=64).text(""));
                    });
                    if forma.capsula_ancho != old_w
                        || forma.capsula_alto != old_h
                        || forma.segmentos != old_s
                    {
                        changed = true;
                    }
                }
                FormaTipo::Plano => {
                    let old_w = forma.plano_ancho;
                    let old_h = forma.plano_alto;
                    ui.horizontal(|ui| {
                        ui.label(self.tx().ancho);
                        if self.btn_repeat(ui, "−") {
                            forma.plano_ancho = (forma.plano_ancho - 0.1).max(0.1);
                        }
                        ui.add(egui::DragValue::new(&mut forma.plano_ancho).speed(0.05).range(0.1..=5.0));
                        if self.btn_repeat(ui, "+") {
                            forma.plano_ancho = (forma.plano_ancho + 0.1).min(5.0);
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label(self.tx().alto);
                        if self.btn_repeat(ui, "−") {
                            forma.plano_alto = (forma.plano_alto - 0.1).max(0.1);
                        }
                        ui.add(egui::DragValue::new(&mut forma.plano_alto).speed(0.05).range(0.1..=5.0));
                        if self.btn_repeat(ui, "+") {
                            forma.plano_alto = (forma.plano_alto + 0.1).min(5.0);
                        }
                    });
                    if forma.plano_ancho != old_w || forma.plano_alto != old_h {
                        changed = true;
                    }
                }
            }

            ui.separator();
            ui.label(self.tx().material);
            let mat_old = forma.material;
            let mat = forma.material;
            ui.horizontal(|ui| {
                if ui.selectable_label(mat == MaterialTipo::Plastico, self.tx().mat_plastico).clicked() {
                    forma.material = MaterialTipo::Plastico;
                }
                if ui.selectable_label(mat == MaterialTipo::Metal, self.tx().mat_metal).clicked() {
                    forma.material = MaterialTipo::Metal;
                }
                if ui.selectable_label(mat == MaterialTipo::Mate, self.tx().mat_mate).clicked() {
                    forma.material = MaterialTipo::Mate;
                }
                if ui.selectable_label(mat == MaterialTipo::Espejo, self.tx().mat_espejo).clicked() {
                    forma.material = MaterialTipo::Espejo;
                }
            });
            if forma.material != mat_old {
                state.shape_dirty = true;
                state.dirty = true;
            }

            ui.label(self.tx().transparencia);
            if ui
                .add(egui::Slider::new(&mut forma.transparencia, 0.0..=1.0).show_value(true))
                .changed()
            {
                state.shape_dirty = true;
                state.dirty = true;
            }

            if changed {
                state.shape_dirty = true;
                state.dirty = true;
            }
        });
    }

    fn ui_grupos(&mut self, ui: &mut egui::Ui, state: &mut SharedState) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.heading(self.tx().agrupaciones);
            ui.separator();

            // Lista de grupos
            let mut borrar: Option<usize> = None;
            for i in 0..state.grupos.len() {
                ui.horizontal(|ui| {
                    if ui
                        .selectable_label(self.grupo_activo == i, &state.grupos[i].nombre)
                        .clicked()
                    {
                        self.grupo_activo = i;
                    }
                    if ui.button("X").clicked() {
                        borrar = Some(i);
                    }
                });
            }
            if let Some(i) = borrar {
                state.borrar_grupo(i);
                if i < self.grupo_activo {
                    self.grupo_activo -= 1;
                }
                state.shape_dirty = true;
                state.dirty = true;
            }
            if ui.button(self.tx().nuevo_grupo).clicked() {
                self.grupo_activo = state.crear_grupo();
            }

            ui.separator();

            if state.grupos.is_empty() {
                ui.label(self.tx().ninguno);
                return;
            }
            let g = self.grupo_activo.min(state.grupos.len() - 1);
            self.grupo_activo = g;
            let grupo = &mut state.grupos[g];

            ui.heading(&grupo.nombre);
            ui.label(self.tx().nombre);
            ui.text_edit_singleline(&mut grupo.nombre);

            ui.separator();
            ui.label(self.tx().bloquear_direccion);

            // Posición del grupo
            ui.label(self.tx().posicion);
            ui.horizontal(|ui| {
                let change = |v: &mut f32, delta: f32| *v = ((*v + delta) * 10.0).round() / 10.0;
                ui.label(self.tx().x);
                if self.btn_repeat(ui, "−") { change(&mut grupo.posicion[0], -0.1); }
                ui.add(egui::DragValue::new(&mut grupo.posicion[0]).speed(0.05).range(-10.0..=10.0));
                if self.btn_repeat(ui, "+") { change(&mut grupo.posicion[0], 0.1); }
                ui.label(self.tx().y);
                if self.btn_repeat(ui, "−") { change(&mut grupo.posicion[1], -0.1); }
                ui.add(egui::DragValue::new(&mut grupo.posicion[1]).speed(0.05).range(-10.0..=10.0));
                if self.btn_repeat(ui, "+") { change(&mut grupo.posicion[1], 0.1); }
                ui.label(self.tx().z);
                if self.btn_repeat(ui, "−") { change(&mut grupo.posicion[2], -0.1); }
                ui.add(egui::DragValue::new(&mut grupo.posicion[2]).speed(0.05).range(-10.0..=10.0));
                if self.btn_repeat(ui, "+") { change(&mut grupo.posicion[2], 0.1); }
            });

            ui.separator();

            // Rotación del grupo (grados)
            ui.label(self.tx().rotacion);
            let mut rot_deg = [
                grupo.rotacion[0].to_degrees(),
                grupo.rotacion[1].to_degrees(),
                grupo.rotacion[2].to_degrees(),
            ];
            let old_rot = rot_deg;
            ui.horizontal(|ui| {
                let change = |v: &mut f32, delta: f32| {
                    *v = ((*v + delta) * 10.0).round() / 10.0;
                };
                ui.label(self.tx().x);
                if self.btn_repeat(ui, "−") { change(&mut rot_deg[0], -1.0); }
                ui.add(egui::DragValue::new(&mut rot_deg[0]).speed(0.5).range(-180.0..=180.0).suffix("°"));
                if self.btn_repeat(ui, "+") { change(&mut rot_deg[0], 1.0); }
                ui.label(self.tx().y);
                if self.btn_repeat(ui, "−") { change(&mut rot_deg[1], -1.0); }
                ui.add(egui::DragValue::new(&mut rot_deg[1]).speed(0.5).range(-180.0..=180.0).suffix("°"));
                if self.btn_repeat(ui, "+") { change(&mut rot_deg[1], 1.0); }
                ui.label(self.tx().z);
                if self.btn_repeat(ui, "−") { change(&mut rot_deg[2], -1.0); }
                ui.add(egui::DragValue::new(&mut rot_deg[2]).speed(0.5).range(-180.0..=180.0).suffix("°"));
                if self.btn_repeat(ui, "+") { change(&mut rot_deg[2], 1.0); }
            });
            if rot_deg != old_rot {
                grupo.rotacion = [
                    rot_deg[0].to_radians(),
                    rot_deg[1].to_radians(),
                    rot_deg[2].to_radians(),
                ];
                state.dirty = true;
            }

            ui.separator();

            // Escala del grupo
            ui.label(self.tx().escala);
            let old_escala = grupo.escala;
            ui.horizontal(|ui| {
                let change = |v: &mut f32, delta: f32| {
                    *v = ((*v + delta) * 100.0).round() / 100.0;
                };
                ui.label(self.tx().x);
                if self.btn_repeat(ui, "−") { change(&mut grupo.escala[0], -0.05); }
                ui.add(egui::DragValue::new(&mut grupo.escala[0]).speed(0.01).range(0.1..=10.0));
                if self.btn_repeat(ui, "+") { change(&mut grupo.escala[0], 0.05); }
                ui.label(self.tx().y);
                if self.btn_repeat(ui, "−") { change(&mut grupo.escala[1], -0.05); }
                ui.add(egui::DragValue::new(&mut grupo.escala[1]).speed(0.01).range(0.1..=10.0));
                if self.btn_repeat(ui, "+") { change(&mut grupo.escala[1], 0.05); }
                ui.label(self.tx().z);
                if self.btn_repeat(ui, "−") { change(&mut grupo.escala[2], -0.05); }
                ui.add(egui::DragValue::new(&mut grupo.escala[2]).speed(0.01).range(0.1..=10.0));
                if self.btn_repeat(ui, "+") { change(&mut grupo.escala[2], 0.05); }
            });
            if grupo.escala != old_escala {
                state.dirty = true;
            }

            ui.separator();

            // Miembros del grupo
            ui.label(self.tx().miembros);
            let miembro_count = state
                .formas
                .iter()
                .filter(|f| f.grupo == Some(g))
                .count();
            if miembro_count == 0 {
                ui.label(self.tx().ninguno);
            }
            for fi in 0..state.formas.len() {
                if state.formas[fi].grupo == Some(g) {
                    ui.horizontal(|ui| {
                        ui.label(&state.formas[fi].nombre);
                        if ui.button(self.tx().quitar_del_grupo).clicked() {
                            state.asignar_grupo(fi, None);
                            state.shape_dirty = true;
                            state.dirty = true;
                        }
                    });
                }
            }

            ui.separator();

            // Añadir formas del grupo (las que no tienen grupo)
            ui.label(self.tx().anadir_a_grupo);
            for fi in 0..state.formas.len() {
                if state.formas[fi].grupo.is_none() {
                    ui.horizontal(|ui| {
                        ui.label(&state.formas[fi].nombre);
                        if ui.button("+").clicked() {
                            state.asignar_grupo(fi, Some(g));
                            state.shape_dirty = true;
                            state.dirty = true;
                        }
                    });
                }
            }

            ui.separator();
            ui.separator();
        });
    }

    fn ui_avanzado(&mut self, ui: &mut egui::Ui, state: &mut SharedState) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.heading(self.tx().lienzo);
            ui.separator();

            ui.label(self.tx().tam_lienzo);
            let old_size = state.tex_size;
            let opciones = [8usize, 16, 32];
            ui.horizontal(|ui| {
                for &op in &opciones {
                    ui.radio_value(&mut state.tex_size, op, format!("{}×{}", op, op));
                }
            });
            if state.tex_size != old_size {
                for forma in &mut state.formas {
                    redimensionar_pixeles(&mut forma.pixeles, old_size, state.tex_size);
                }
                state.res_dirty = true;
                state.dirty = true;
            }

            ui.separator();

            ui.label(self.tx().colores_paleta);
            let mut edit_idx = None;
            ui.horizontal(|ui| {
                for (i, &c) in state.paleta.iter().enumerate() {
                    let color = egui::Color32::from_rgb(c[0], c[1], c[2]);
                    let (id, rect) = ui.allocate_space(egui::vec2(24.0, 24.0));
                    let resp = ui.interact(rect, id, egui::Sense::click());
                    ui.painter().rect_filled(rect, 2.0, color);
                    ui.painter().rect_stroke(
                        rect,
                        2.0,
                        egui::Stroke::new(1.0, egui::Color32::DARK_GRAY),
                    );
                    if resp.clicked() {
                        edit_idx = Some(i);
                    }
                }
            });
            if let Some(idx) = edit_idx {
                self.editing_paleta = Some(idx);
            }

            if let Some(idx) = self.editing_paleta {
                let mut open = true;
                let mut c32 = egui::Color32::from_rgb(
                    state.paleta[idx][0],
                    state.paleta[idx][1],
                    state.paleta[idx][2],
                );
                egui::Window::new(format!("{} #{}", self.tx().selector_color, idx + 1))
                    .open(&mut open)
                    .default_width(300.0)
                    .show(ui.ctx(), |ui| {
                        ui.spacing_mut().slider_width = ui.available_width() - 10.0;
                        if egui::color_picker::color_picker_color32(
                            ui,
                            &mut c32,
                            egui::color_picker::Alpha::Opaque,
                        ) {
                            state.paleta[idx] = [c32[0], c32[1], c32[2], 255];
                        }
                    });
                if !open {
                    self.editing_paleta = None;
                }
            }

            ui.separator();

            ui.label(self.tx().color_relleno);
            let mut c32 = egui::Color32::from_rgb(
                state.fill_color[0],
                state.fill_color[1],
                state.fill_color[2],
            );
            let (id, rect) = ui.allocate_space(egui::vec2(24.0, 24.0));
            let resp = ui.interact(rect, id, egui::Sense::click());
            ui.painter().rect_filled(rect, 2.0, c32);
            ui.painter()
                .rect_stroke(rect, 2.0, egui::Stroke::new(1.0, egui::Color32::DARK_GRAY));
            if resp.clicked() {
                self.show_fill_picker = true;
            }

            if self.show_fill_picker {
                let mut win_open = true;
                egui::Window::new(self.tx().color_relleno)
                    .open(&mut win_open)
                    .default_width(300.0)
                    .show(ui.ctx(), |ui| {
                        ui.spacing_mut().slider_width = ui.available_width() - 10.0;
                        if egui::color_picker::color_picker_color32(
                            ui,
                            &mut c32,
                            egui::color_picker::Alpha::Opaque,
                        ) {
                            state.fill_color = [c32[0], c32[1], c32[2], 255];
                        }
                    });
                if !win_open {
                    self.show_fill_picker = false;
                }
            }

            ui.separator();
            ui.label("Language:");
            ui.horizontal(|ui| {
                if ui.selectable_label(self.lang == Lang::Es, "Español").clicked() { self.lang = Lang::Es; }
                if ui.selectable_label(self.lang == Lang::En, "English").clicked() { self.lang = Lang::En; }
                if ui.selectable_label(self.lang == Lang::Fr, "Français").clicked() { self.lang = Lang::Fr; }
            });
        });
    }

    fn ui_proyecto(&mut self, ui: &mut egui::Ui, state: &mut SharedState) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.heading(self.tx().proyecto);
            ui.separator();

            ui.horizontal(|ui| {
                ui.label(format!("{}:", self.tx().nombre));
                ui.text_edit_singleline(&mut state.project_name);
            });

            ui.horizontal(|ui| {
                if ui.button(self.tx().guardar).clicked() {
                    let nombre = if state.project_name.trim().is_empty() {
                        "mi_proyecto".to_string()
                    } else {
                        state.project_name.trim().to_string()
                    };
                    let path = desktop_dir()
                        .join("modelador_proyectos")
                        .join(format!("{}.glb", sanitize_filename(&nombre)));
                    match exportar_glb(state, &path) {
                        Ok(msg) => state.mensaje = msg,
                        Err(e) => state.mensaje = format!("Error: {}", e),
                    }
                    state.project_path = Some(path);
                    self.proyectos = listar_proyectos();
                }
                if ui.button(self.tx().guardar_cambios).clicked() {
                    if let Some(ref path) = state.project_path {
                        match exportar_glb(state, path) {
                            Ok(msg) => state.mensaje = msg,
                            Err(e) => state.mensaje = format!("Error: {}", e),
                        }
                    } else {
                        state.mensaje = format!("Primero usa \"{}\".", self.tx().guardar);
                    }
                }
                if ui.button(self.tx().cargar).clicked() {
                    self.proyectos = listar_proyectos();
                }
            });

            ui.separator();
            ui.horizontal(|ui| {
                ui.label(self.tx().autoguardado);
                if ui.selectable_label(self.autosave_activo, "ON").clicked() {
                    self.autosave_activo = true;
                    self.autosave_pend = false;
                }
                if ui.selectable_label(!self.autosave_activo, "OFF").clicked() {
                    self.autosave_activo = false;
                    self.autosave_pend = false;
                }
            });

            ui.separator();
            ui.label(self.tx().proyectos_guardados);
            let proyectos = self.proyectos.clone();
            if proyectos.is_empty() {
                ui.label(self.tx().ninguno);
            } else {
                egui::ScrollArea::vertical()
                    .max_height(200.0)
                    .show(ui, |ui| {
                        for nombre in &proyectos {
                            if ui.selectable_label(false, nombre).clicked() {
                                let path = desktop_dir()
                                    .join("modelador_proyectos")
                                    .join(format!("{}.glb", nombre));
                                match importar_glb(&path) {
                                    Ok(data) => {
                                        let nombre = nombre.clone();
                                        state.mensaje =
                                            format!("Proyecto cargado: {}", path.display());
                                        aplicar_proyecto(state, data, nombre, path);
                                    }
                                    Err(e) => state.mensaje = e,
                                }
                            }
                        }
                    });
            }

            ui.separator();
            if !state.mensaje.is_empty() {
                ui.label(&state.mensaje);
            }
        });
    }
}

// ============================================================
// 3D (kiss3d)
// ============================================================
fn usar_kiss3d(state: Arc<Mutex<SharedState>>) {
    use image::RgbaImage;
    use kiss3d::camera::ArcBall;
    use kiss3d::context::Context;
    use kiss3d::event::{Action, Key};
    use kiss3d::light::Light;
    use kiss3d::nalgebra as na;
    use kiss3d::resource::vertex_index::VertexIndex;
    use kiss3d::resource::{Mesh, TextureManager};
    use kiss3d::scene::SceneNode;
    use kiss3d::window::Window;
    use std::cell::RefCell;
    use std::rc::Rc;

    let mut window = Window::new("Simplified 3D");
    window.set_light(Light::StickToCamera);
    let mut camara = ArcBall::new(
        na::Point3::new(3.0, 2.5, 3.0),
        na::Point3::new(0.0, 0.0, 0.0),
    );

    let mut root = window.add_group();

    // Sky dome background
    {
        let mut sky_group = root.add_group();
        let sky_tex_size = 64usize;
        let mut grad = vec![0u8; sky_tex_size * sky_tex_size * 4];
        for y in 0..sky_tex_size {
            let t = y as f32 / (sky_tex_size - 1) as f32;
            let (r, g, b) = if t < 0.5 {
                let u = t * 2.0;
                (30.0 + 130.0 * u, 80.0 + 130.0 * u, 180.0 + 75.0 * u)
            } else {
                let u = (t - 0.5) * 2.0;
                (
                    160.0 * (1.0 - u) + 80.0 * u,
                    210.0 * (1.0 - u) + 140.0 * u,
                    255.0 * (1.0 - u) + 180.0 * u,
                )
            };
            let (r, g, b) = (r as u8, g as u8, b as u8);
            for x in 0..sky_tex_size {
                let i = (y * sky_tex_size + x) * 4;
                grad[i] = r;
                grad[i + 1] = g;
                grad[i + 2] = b;
                grad[i + 3] = 255;
            }
        }
        let mut sky_datos = malla_esfera(50.0, 32);
        for n in sky_datos.normals.iter_mut() {
            n[0] = -n[0];
            n[1] = -n[1];
            n[2] = -n[2];
        }
        let sky_mesh = mesh_desde_datos(&sky_datos);
        let mut sky_node = sky_group.add_mesh(sky_mesh, na::Vector3::new(1.0, 1.0, 1.0));
        let sky_tex = crear_textura(&grad, sky_tex_size);
        sky_node.set_texture(sky_tex);
        sky_node.enable_backface_culling(false);
    }

    struct ShapeGroup {
        group: SceneNode,
        nodes: Vec<SceneNode>,
        textures: Vec<Rc<kiss3d::context::Texture>>,
    }

    let mut shape_groups: Vec<ShapeGroup> = vec![];
    let mut grupo_nodes: Vec<SceneNode> = vec![];
    fn tex_id() -> String {
        static C: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        format!("t_{}", C.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
    }

    fn crear_textura(pixeles: &[u8], tex_size: usize) -> Rc<kiss3d::context::Texture> {
        let mut img =
            RgbaImage::from_raw(tex_size as u32, tex_size as u32, pixeles.to_vec()).unwrap();
        image::imageops::flip_vertical_in_place(&mut img);
        let dyn_img = image::DynamicImage::ImageRgba8(img);
        let tex = TextureManager::get_global_manager(|tm| tm.add_image(dyn_img.clone(), &tex_id()));
        let ctxt = Context::get();
        ctxt.bind_texture(Context::TEXTURE_2D, Some(&tex));
        ctxt.tex_parameteri(
            Context::TEXTURE_2D,
            Context::TEXTURE_MAG_FILTER,
            Context::NEAREST as i32,
        );
        ctxt.tex_parameteri(
            Context::TEXTURE_2D,
            Context::TEXTURE_MIN_FILTER,
            Context::NEAREST as i32,
        );
        tex
    }

    fn subir_textura(tex: &kiss3d::context::Texture, pixeles: &[u8], tex_size: usize) {
        let mut img =
            RgbaImage::from_raw(tex_size as u32, tex_size as u32, pixeles.to_vec()).unwrap();
        image::imageops::flip_vertical_in_place(&mut img);
        let ctxt = Context::get();
        ctxt.bind_texture(Context::TEXTURE_2D, Some(tex));
        ctxt.tex_sub_image2d(
            Context::TEXTURE_2D,
            0,
            0,
            0,
            tex_size as i32,
            tex_size as i32,
            Context::RGBA,
            Some(img.as_raw()),
        );
    }

    fn mesh_desde_datos(m: &MallaDatos) -> Rc<RefCell<Mesh>> {
        let vertices: Vec<na::Point3<f32>> =
            m.pos.iter().map(|p| na::Point3::new(p[0], p[1], p[2])).collect();
        let normals: Vec<na::Vector3<f32>> =
            m.normals.iter().map(|n| na::Vector3::new(n[0], n[1], n[2])).collect();
        let uvs: Vec<na::Point2<f32>> =
            m.uvs.iter().map(|u| na::Point2::new(u[0], u[1])).collect();
        let indices: Vec<na::Point3<VertexIndex>> = m
            .indices
            .chunks_exact(3)
            .map(|t| na::Point3::new(t[0] as VertexIndex, t[1] as VertexIndex, t[2] as VertexIndex))
            .collect();
        let mesh = Mesh::new(vertices, indices, Some(normals), Some(uvs), false);
        Rc::new(RefCell::new(mesh))
    }

    fn crear_mesh_cara(verts: &[[f32; 3]], face: &[usize]) -> Rc<RefCell<Mesh>> {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        let mut uvs = Vec::new();
        match face.len() {
            3 => {
                for &idx in face {
                    let v = verts[idx];
                    vertices.push(na::Point3::new(v[0], v[1], v[2]));
                }
                uvs.push(na::Point2::new(0.0, 0.0));
                uvs.push(na::Point2::new(1.0, 0.0));
                uvs.push(na::Point2::new(0.5, 1.0));
                indices.push(na::Point3::new(0u32 as VertexIndex, 1, 2));
            }
            4 => {
                for &idx in face {
                    let v = verts[idx];
                    vertices.push(na::Point3::new(v[0], v[1], v[2]));
                }
                uvs.push(na::Point2::new(0.0, 0.0));
                uvs.push(na::Point2::new(1.0, 0.0));
                uvs.push(na::Point2::new(1.0, 1.0));
                uvs.push(na::Point2::new(0.0, 1.0));
                indices.push(na::Point3::new(0u32 as VertexIndex, 1, 2));
                indices.push(na::Point3::new(0u32 as VertexIndex, 2, 3));
            }
            _ => {}
        }
        let mesh = Mesh::new(vertices, indices, None, Some(uvs), false);
        Rc::new(RefCell::new(mesh))
    }

    fn rebuild_shapes(state: &SharedState, root: &mut SceneNode) -> (Vec<ShapeGroup>, Vec<SceneNode>) {
        let mut grupo_nodes = Vec::new();
        for g in &state.grupos {
            let mut n = root.add_group();
            n.set_local_translation(na::Translation3::new(g.posicion[0], g.posicion[1], g.posicion[2]));
            n.set_local_rotation(na::UnitQuaternion::from_euler_angles(
                g.rotacion[0], g.rotacion[1], g.rotacion[2],
            ));
            n.set_local_scale(g.escala[0], g.escala[1], g.escala[2]);
            grupo_nodes.push(n);
        }

        let mut groups = Vec::new();
        for forma in &state.formas {
            let mat = material_pbr(forma.material, forma.transparencia)
                .expect("no se pudo crear material PBR");
            let parent = match forma.grupo {
                Some(gf) if gf < grupo_nodes.len() => &mut grupo_nodes[gf],
                _ => root,
            };
            let mut group = parent.add_group();
            group.set_local_translation(na::Translation3::new(
                forma.posicion[0],
                forma.posicion[1],
                forma.posicion[2],
            ));
            group.set_local_rotation(
                na::UnitQuaternion::from_euler_angles(forma.rotacion[0], forma.rotacion[1], forma.rotacion[2]),
            );
            let mut nodes = Vec::new();
            let mut textures = Vec::new();
            match forma.tipo {
                FormaTipo::Cubo | FormaTipo::Cuboide | FormaTipo::PirCuadrada => {
                    for (_i, face) in forma.shape_faces.iter().enumerate() {
                        let mesh = crear_mesh_cara(&forma.shape_vertices, face);
                        let mut node = group.add_mesh(mesh, na::Vector3::new(1.0, 1.0, 1.0));
                        let tex = crear_textura(&forma.pixeles[_i], state.tex_size);
                        node.set_texture(tex.clone());
                        node.set_lines_color(Some(na::Point3::new(0.0, 0.0, 0.0)));
                        node.set_material(mat.clone());
                        nodes.push(node);
                        textures.push(tex);
                    }
                }
                _ => {
                    if let Some(mallas) = mallas_forma(forma) {
                        for (i, malla) in mallas.iter().enumerate() {
                            let mesh = mesh_desde_datos(malla);
                            let mut node =
                                group.add_mesh(mesh, na::Vector3::new(1.0, 1.0, 1.0));
                            if forma.tipo == FormaTipo::Plano {
                                node.enable_backface_culling(false);
                            }
                            let tex =
                                crear_textura(&forma.pixeles[i.min(forma.pixeles.len() - 1)], state.tex_size);
                            node.set_texture(tex.clone());
                            node.set_lines_color(Some(na::Point3::new(0.0, 0.0, 0.0)));
                            node.set_material(mat.clone());
                            nodes.push(node);
                            textures.push(tex);
                        }
                    }
                }
            }
            groups.push(ShapeGroup {
                group,
                nodes,
                textures,
            });
        }
        (groups, grupo_nodes)
    }

    let (sg0, gn0) = rebuild_shapes(&state.lock().unwrap(), &mut root);
    shape_groups = sg0;
    grupo_nodes = gn0;

    let mut prev_count = state.lock().unwrap().formas.len();
    let mut prev_grupos = state.lock().unwrap().grupos.len();

    while window.render_with_camera(&mut camara) {
        let mut state_ = state.lock().unwrap();

        if state_.shape_dirty
            || state_.formas.len() != prev_count
            || state_.grupos.len() != prev_grupos
        {
            for mut sg in shape_groups.drain(..) {
                sg.group.unlink();
            }
            for mut gn in grupo_nodes.drain(..) {
                gn.unlink();
            }
            let (sg, gn) = rebuild_shapes(&state_, &mut root);
            shape_groups = sg;
            grupo_nodes = gn;
            state_.shape_dirty = false;
            prev_count = state_.formas.len();
            prev_grupos = state_.grupos.len();
        }

        // Update group transforms each frame
        for (gi, g) in state_.grupos.iter().enumerate() {
            if gi < grupo_nodes.len() {
                grupo_nodes[gi]
                    .set_local_translation(na::Translation3::new(g.posicion[0], g.posicion[1], g.posicion[2]));
                grupo_nodes[gi]
                    .set_local_rotation(na::UnitQuaternion::from_euler_angles(
                        g.rotacion[0], g.rotacion[1], g.rotacion[2],
                    ));
                grupo_nodes[gi].set_local_scale(g.escala[0], g.escala[1], g.escala[2]);
            }
        }

        // Update positions and rotations each frame
        for (i, forma) in state_.formas.iter().enumerate() {
            if i < shape_groups.len() {
                shape_groups[i]
                    .group
                    .set_local_translation(na::Translation3::new(
                        forma.posicion[0],
                        forma.posicion[1],
                        forma.posicion[2],
                    ));
                shape_groups[i]
                    .group
                    .set_local_rotation(
                        na::UnitQuaternion::from_euler_angles(forma.rotacion[0], forma.rotacion[1], forma.rotacion[2]),
                    );
                shape_groups[i].group.set_visible(!forma.oculta);
            }
        }

        // Texture updates for active shape
        let a = state_.forma_activa;
        if a < shape_groups.len() {
            let sg = &mut shape_groups[a];
            let f = &state_.formas[a];
            if state_.dirty || state_.res_dirty {
                match f.tipo {
                    FormaTipo::Cubo | FormaTipo::Cuboide | FormaTipo::PirCuadrada
                    | FormaTipo::Cilindro | FormaTipo::Cono => {
                        if state_.res_dirty {
                            sg.textures.clear();
                            for i in 0..sg.nodes.len() {
                                let tex = crear_textura(&f.pixeles[i], state_.tex_size);
                                sg.nodes[i].set_texture(tex.clone());
                                sg.nodes[i].set_lines_color(Some(na::Point3::new(0.0, 0.0, 0.0)));
                                sg.textures.push(tex);
                            }
                        } else if state_.dirty {
                            for i in 0..sg.textures.len() {
                                subir_textura(&sg.textures[i], &f.pixeles[i], state_.tex_size);
                            }
                        }
                    }
                    _ => {
                        if let Some(node) = sg.nodes.first_mut() {
                            let tex = crear_textura(&f.pixeles[0], state_.tex_size);
                            node.set_texture(tex);
                        }
                    }
                }
                state_.dirty = false;
                state_.res_dirty = false;
            }
        }

        // Highlight active shape (cyan edges) and selected face (white edges)
        for (i, sg) in shape_groups.iter_mut().enumerate() {
            let is_active = i == a;
            let f = &state_.formas[i];
            for (j, node) in sg.nodes.iter_mut().enumerate() {
                let color = if is_active && j == f.cara_sel {
                    na::Point3::new(1.0, 1.0, 1.0) // white = selected face
                } else if is_active {
                    na::Point3::new(0.0, 1.0, 1.0) // cyan = active shape
                } else {
                    na::Point3::new(0.0, 0.0, 0.0) // black = inactive
                };
                node.set_lines_color(Some(color));
            }
        }

        drop(state_);

        // Movement/rotation controls (affects all shapes as a group)
        let v = 0.05;
        for (k, d) in [
            (Key::Left, [-v, 0.0, 0.0]),
            (Key::Right, [v, 0.0, 0.0]),
            (Key::Up, [0.0, v, 0.0]),
            (Key::Down, [0.0, -v, 0.0]),
            (Key::W, [0.0, 0.0, -v]),
            (Key::S, [0.0, 0.0, v]),
        ] {
            if window.get_key(k) == Action::Press {
                root.append_translation(&na::Translation3::new(d[0], d[1], d[2]));
            }
        }

        let mut rr = [0.0; 3];
        for (k, i, s) in [
            (Key::Q, 1, -0.03),
            (Key::E, 1, 0.03),
            (Key::R, 0, -0.03),
            (Key::T, 0, 0.03),
            (Key::Y, 2, 0.03),
            (Key::U, 2, -0.03),
        ] {
            if window.get_key(k) == Action::Press {
                rr[i] += s;
            }
        }
        if rr[0] != 0.0 || rr[1] != 0.0 || rr[2] != 0.0 {
            root.prepend_to_local_rotation(&na::UnitQuaternion::from_euler_angles(
                rr[0], rr[1], rr[2],
            ));
        }

        if window.get_key(Key::Equals) == Action::Press {
            let s = root.data().local_scale();
            root.set_local_scale(s.x * 1.01, s.y * 1.01, s.z * 1.01);
        }
        if window.get_key(Key::Minus) == Action::Press {
            let s = root.data().local_scale();
            let n = (s.x * 0.99).max(0.1);
            root.set_local_scale(n, n, n);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_glb() {
        let paleta: [[u8; 4]; 8] = [[0, 0, 0, 255]; 8];
        let colores: [[u8; 4]; 6] = [[128, 128, 128, 255]; 6];
        let mut state = SharedState::new(&colores, 8, paleta);
        state.formas[0].nombre = "Cubo pintado".into();
        state.formas[0].posicion = [0.5, -1.0, 2.0];
        state.formas[0].rotacion = [0.2, 0.5, -0.3];
        state.formas.push(FormaData::new(FormaTipo::Esfera, 8, "Mi esfera".into()));
        state.formas[1].esfera_radio = 1.5;
        state.formas[1].segmentos = 12;
        state.formas[1].posicion = [1.0, 2.0, 3.0];
        state.formas[1].pixeles[0][0..4].copy_from_slice(&[255, 0, 0, 255]);
        for (tipo, nombre) in [
            (FormaTipo::Cilindro, "Cilindro"),
            (FormaTipo::Cono, "Cono"),
            (FormaTipo::Capsula, "Cápsula"),
            (FormaTipo::Plano, "Plano"),
        ] {
            state.formas.push(FormaData::new(tipo, 8, nombre.into()));
        }
        state.formas[2].cilindro_radio = 0.7;
        state.formas[2].cilindro_alto = 2.0;
        state.formas[3].cono_radio = 0.9;
        state.formas[3].cono_alto = 1.5;
        state.formas[4].capsula_ancho = 1.2;
        state.formas[4].capsula_alto = 2.4;
        state.formas[5].plano_ancho = 3.0;
        state.formas[5].plano_alto = 1.5;

        let dir = std::env::temp_dir().join("simplified3d_test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test.glb");
        exportar_glb(&state, &path).expect("exportar_glb");

        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(&bytes[0..4], b"glTF");
        let data = importar_glb(&path).expect("importar_glb");

        assert_eq!(data.formas.len(), 6);
        assert_eq!(data.formas[0].nombre, "Cubo pintado");
        assert_eq!(data.formas[0].posicion, [0.5, -1.0, 2.0]);
        assert_eq!(data.formas[0].rotacion, [0.2, 0.5, -0.3]);
        assert_eq!(data.formas[1].nombre, "Mi esfera");
        assert_eq!(data.formas[1].esfera_radio, 1.5);
        assert_eq!(data.formas[1].pixeles[0], state.formas[1].pixeles[0]);
        assert_eq!(data.formas[2].cilindro_radio, 0.7);
        assert_eq!(data.formas[2].cilindro_alto, 2.0);
        assert_eq!(data.formas[3].cono_radio, 0.9);
        assert_eq!(data.formas[3].cono_alto, 1.5);
        assert_eq!(data.formas[4].capsula_ancho, 1.2);
        assert_eq!(data.formas[4].capsula_alto, 2.4);
        assert_eq!(data.formas[5].plano_ancho, 3.0);
        assert_eq!(data.formas[5].plano_alto, 1.5);
        assert_eq!(data.tex_size, 8);
    }

    #[test]
    fn mallas_validas() {
        for tipo in [
            FormaTipo::Esfera,
            FormaTipo::Cilindro,
            FormaTipo::Cono,
            FormaTipo::Capsula,
            FormaTipo::Plano,
        ] {
            let f = FormaData::new(tipo, 8, "x".into());
            let mallas = mallas_forma(&f).expect("malla");
            assert_eq!(mallas.len(), face_count(tipo), "{tipo:?}: nº de caras");
            for (ic, m) in mallas.iter().enumerate() {
                let tag = format!("{tipo:?} cara {ic}");
                assert!(!m.pos.is_empty(), "{tag}: sin posiciones");
                assert_eq!(m.pos.len(), m.uvs.len(), "{tag}: uvs");
                assert_eq!(m.pos.len(), m.normals.len(), "{tag}: normales");
                assert_eq!(m.indices.len() % 3, 0, "{tag}: indices");
                let max_idx = *m.indices.iter().max().unwrap() as usize;
                assert!(max_idx < m.pos.len(), "{tag}: índice fuera de rango");
                for n in &m.normals {
                    let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
                    assert!((l - 1.0).abs() < 1e-4, "{tag}: normal no unitaria {l}");
                }
                let indices = &m.indices;
                for tri in indices.chunks_exact(3) {
                    let (a, b, c) = (m.pos[tri[0] as usize], m.pos[tri[1] as usize], m.pos[tri[2] as usize]);
                    let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
                    let w = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
                    let n = [
                        u[1] * w[2] - u[2] * w[1],
                        u[2] * w[0] - u[0] * w[2],
                        u[0] * w[1] - u[1] * w[0],
                    ];
                    let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
                    assert!(l > 1e-6, "{tag}: triángulo degenerado");
                    let vn = m.normals[tri[0] as usize];
                    let dot = n[0] * vn[0] + n[1] * vn[1] + n[2] * vn[2];
                    assert!(dot > 0.0, "{tag}: triángulo invertido (winding): {:?}", tri);
                }
            }
        }
    }

    #[test]
    fn quaternion_es_unidad() {
        for (r, p, y) in [(0.0, 0.0, 0.0), (1.0, 2.0, -3.0), (0.1, -0.4, 2.7)] {
            let q = euler_a_quaternion(r, p, y);
            let len = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt();
            assert!((len - 1.0).abs() < 1e-6, "no unit: {}", len);
        }
    }
}
