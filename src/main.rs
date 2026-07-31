use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
enum FormaTipo {
    Cubo,
    Cuboide,
    PirCuadrada,
    Esfera,
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
        FormaTipo::Esfera => (vec![], vec![]),
    }
}

fn face_count(tipo: FormaTipo) -> usize {
    match tipo {
        FormaTipo::Esfera => 1,
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
    esfera_segmentos: usize,
    pyramid_scale: f32,
    shape_vertices: Vec<[f32; 3]>,
    shape_faces: Vec<Vec<usize>>,
    cara_sel: usize,
    pixeles: Vec<Vec<u8>>,
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
            esfera_segmentos: 24,
            pyramid_scale: 1.0,
            shape_vertices,
            shape_faces,
            cara_sel: 0,
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
            FormaTipo::Esfera => {}
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
        }
    }

    fn forma(&self) -> &FormaData {
        &self.formas[self.forma_activa]
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

fn save_textures_to_dir(state: &SharedState, dir: &Path) {
    for forma in state.formas.iter() {
        let base = sanitize_filename(&forma.nombre);
        let tex_size = state.tex_size as u32;
        match forma.tipo {
            FormaTipo::Esfera => {
                let tex_path = dir.join(format!("{}.png", base));
                let mut img = image::RgbaImage::from_raw(
                    tex_size, tex_size,
                    forma.pixeles[0].clone(),
                ).unwrap();
                image::imageops::flip_vertical_in_place(&mut img);
                img.save(&tex_path).ok();
            }
            _ => {
                for i in 0..forma.pixeles.len() {
                    let tex_path = dir.join(format!("{}_face_{}.png", base, i));
                    let mut img = image::RgbaImage::from_raw(
                        tex_size, tex_size,
                        forma.pixeles[i].clone(),
                    ).unwrap();
                    image::imageops::flip_vertical_in_place(&mut img);
                    img.save(&tex_path).ok();
                }
            }
        }
    }
}

fn save_obj_to_dir(state: &SharedState, dir: &Path) {
    let mut mtl = String::new();
    mtl.push_str("# MTL exportado de Modelador 3D\n");
    for forma in state.formas.iter() {
        let base = sanitize_filename(&forma.nombre);
        match forma.tipo {
            FormaTipo::Esfera => {
                mtl.push_str(&format!("newmtl {}\n", base));
                mtl.push_str("Ns 0.0\nKa 1.0 1.0 1.0\nKd 1.0 1.0 1.0\nKs 0.0 0.0 0.0\nd 1.0\nillum 2\n");
                mtl.push_str(&format!("map_Kd {}.png\n", base));
                mtl.push('\n');
            }
            _ => {
                for i in 0..forma.pixeles.len() {
                    mtl.push_str(&format!("newmtl {}_face_{}\n", base, i));
                    mtl.push_str("Ns 0.0\nKa 1.0 1.0 1.0\nKd 1.0 1.0 1.0\nKs 0.0 0.0 0.0\nd 1.0\nillum 2\n");
                    mtl.push_str(&format!("map_Kd {}_face_{}.png\n", base, i));
                    mtl.push('\n');
                }
            }
        }
    }
    std::fs::write(dir.join("modelo.mtl"), &mtl).ok();

    let mut obj = String::new();
    obj.push_str("# Exportado de Modelador 3D\nmtllib modelo.mtl\n\n");
    let mut base_v = 1usize;
    let mut base_vt = 1usize;
    for forma in state.formas.iter() {
        let base = sanitize_filename(&forma.nombre);
        obj.push_str(&format!("o {}\n", forma.nombre));
        match forma.tipo {
            FormaTipo::Esfera => {
                let r = forma.esfera_radio;
                let seg = forma.esfera_segmentos;
                for lat in 0..=seg {
                    let theta = lat as f32 * std::f32::consts::PI / seg as f32;
                    let sin_t = theta.sin();
                    let cos_t = theta.cos();
                    for lon in 0..=seg {
                        let phi = lon as f32 * 2.0 * std::f32::consts::PI / seg as f32;
                        let (x, y, z) = (phi.cos() * sin_t * r, cos_t * r, phi.sin() * sin_t * r);
                        obj.push_str(&format!("v {} {} {}\n", x, y, z));
                    }
                }
                for lat in 0..=seg {
                    for lon in 0..=seg {
                        let u = lon as f32 / seg as f32;
                        let v = lat as f32 / seg as f32;
                        obj.push_str(&format!("vt {} {}\n", u, v));
                    }
                }
                for lat in 0..seg {
                    for lon in 0..seg {
                        let a = lat * (seg + 1) + lon;
                        let b = a + seg + 1;
                        obj.push_str(&format!("usemtl {}\n", base));
                        obj.push_str(&format!(
                            "f {}/{} {}/{} {}/{}\n",
                            a + base_v,
                            a + base_vt,
                            b + base_v,
                            b + base_vt,
                            a + 1 + base_v,
                            a + 1 + base_vt
                        ));
                        obj.push_str(&format!(
                            "f {}/{} {}/{} {}/{}\n",
                            a + 1 + base_v,
                            a + 1 + base_vt,
                            b + base_v,
                            b + base_vt,
                            b + 1 + base_v,
                            b + 1 + base_vt
                        ));
                    }
                }
                let count = (seg + 1) * (seg + 1);
                base_v += count * 2;
                base_vt += count;
            }
            _ => {
                let pos = forma.posicion;
                for v in &forma.shape_vertices {
                    obj.push_str(&format!(
                        "v {} {} {}\n",
                        v[0] + pos[0],
                        v[1] + pos[1],
                        v[2] + pos[2]
                    ));
                }
                obj.push_str("\nvt 0 0\nvt 1 0\nvt 1 1\nvt 0 1\n");
                for (i, face) in forma.shape_faces.iter().enumerate() {
                    obj.push_str(&format!("usemtl {}_face_{}\n", base, i));
                    match face.len() {
                        3 => obj.push_str(&format!(
                            "f {}/{} {}/{} {}/{}\n",
                            face[0] + base_v,
                            base_vt,
                            face[1] + base_v,
                            base_vt + 1,
                            face[2] + base_v,
                            base_vt + 2
                        )),
                        4 => {
                            obj.push_str(&format!(
                                "f {}/{} {}/{} {}/{}\n",
                                face[0] + base_v,
                                base_vt,
                                face[1] + base_v,
                                base_vt + 1,
                                face[2] + base_v,
                                base_vt + 2
                            ));
                            obj.push_str(&format!(
                                "f {}/{} {}/{} {}/{}\n",
                                face[0] + base_v,
                                base_vt,
                                face[2] + base_v,
                                base_vt + 2,
                                face[3] + base_v,
                                base_vt + 3
                            ));
                        }
                        _ => {}
                    }
                }
                base_v += forma.shape_vertices.len();
                base_vt += 4;
            }
        }
        obj.push('\n');
    }
    std::fs::write(dir.join("modelo.obj"), &obj).ok();
}

fn guardar_proyecto_dir(state: &SharedState, dir: &Path) -> String {
    std::fs::create_dir_all(dir).ok();
    save_textures_to_dir(state, dir);
    let data = ProjectData {
        formas: state.formas.clone(),
        forma_activa: state.forma_activa,
        tex_size: state.tex_size,
        paleta: state.paleta,
        fill_color: state.fill_color,
    };
    let json = serde_json::to_string_pretty(&data).unwrap();
    std::fs::write(dir.join(format!("{}.json", state.project_name)), &json).ok();
    save_obj_to_dir(state, dir);
    format!("Guardado en: {}", dir.display())
}

fn importar_proyecto(state: &mut SharedState, nombre: &str) -> String {
    let dir = desktop_dir().join("modelador_proyectos").join(nombre);
    let json_path = dir.join(format!("{}.json", nombre));
    let json = match std::fs::read_to_string(&json_path) {
        Ok(s) => s,
        Err(e) => return format!("Error al leer: {}", e),
    };
    let data: ProjectData = match serde_json::from_str(&json) {
        Ok(d) => d,
        Err(e) => return format!("Error al parsear: {}", e),
    };
    state.formas = data.formas;
    for forma in state.formas.iter_mut() {
        let base = sanitize_filename(&forma.nombre);
        match forma.tipo {
            FormaTipo::Esfera => {
                let tex_path = dir.join(format!("{}.png", base));
                if let Ok(img) = image::open(&tex_path) {
                    let mut rgba = img.to_rgba8();
                    image::imageops::flip_vertical_in_place(&mut rgba);
                    forma.pixeles[0] = rgba.into_raw();
                }
            }
            _ => {
                for i in 0..forma.pixeles.len() {
                    let tex_path = dir.join(format!("{}_face_{}.png", base, i));
                    if let Ok(img) = image::open(&tex_path) {
                        let mut rgba = img.to_rgba8();
                        image::imageops::flip_vertical_in_place(&mut rgba);
                        forma.pixeles[i] = rgba.into_raw();
                    }
                }
            }
        }
    }
    state.forma_activa = data.forma_activa.min(state.formas.len().saturating_sub(1));
    state.tex_size = data.tex_size;
    state.nuevo_tex_size = data.tex_size;
    state.paleta = data.paleta;
    state.fill_color = data.fill_color;
    state.project_name = nombre.to_string();
    state.project_path = Some(dir);
    state.shape_dirty = true;
    state.activa_dirty = true;
    state.dirty = true;
    state.res_dirty = true;
    format!("Proyecto cargado: {}", json_path.display())
}

#[derive(Serialize, Deserialize)]
struct ProjectData {
    formas: Vec<FormaData>,
    forma_activa: usize,
    tex_size: usize,
    paleta: [[u8; 4]; 8],
    fill_color: [u8; 4],
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
            if p.is_dir() && let Some(stem) = p.file_name().and_then(|s| s.to_str())
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
    formas: &'static str,
    cubo: &'static str,
    cuboide: &'static str,
    piramide: &'static str,
    esfera: &'static str,
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
    segmentos: &'static str,
    tam_lienzo: &'static str,
    colores_paleta: &'static str,
    color_relleno: &'static str,
    guardar: &'static str,
    guardar_cambios: &'static str,
    cargar: &'static str,
    proyectos_guardados: &'static str,
    ninguno: &'static str,
}

const TEXTS: [Texts; 3] = [
    Texts {
        lienzo: "Lienzo", forma: "Forma", avanzado: "Avanzado", proyecto: "Proyecto",
        pintando: "Pintando:", cara: "Cara", pixel: "Pixel:",
        color: "Color:", selector_color: "Selector de color",
        colores_guardados: "Colores guardados (clic para usar, clic derecho para quitar):",
        pincel: "Pincel", limpiar_cara: "Limpiar cara",
        formas: "Formas", cubo: "Cubo", cuboide: "Cuboide", piramide: "Pirámide",
        esfera: "Esfera", agregar: "Agregar", forma_activa: "Forma activa",
        nombre: "Nombre:", posicion: "Posición:", rotacion: "Rotación (grados):",
        x: "X:", y: "Y:", z: "Z:", escala: "Escala:", ancho: "Ancho (X):",
        alto: "Alto (Y):", profundo: "Profundo (Z):", radio: "Radio:",
        segmentos: "Segmentos:", tam_lienzo: "Tamaño del lienzo:",
        colores_paleta: "Colores por defecto (clic para editar):",
        color_relleno: "Color de relleno (Limpiar cara):",
        guardar: "Guardar proyecto", guardar_cambios: "Guardar cambios",
        cargar: "Cargar proyecto", proyectos_guardados: "Proyectos guardados:",
        ninguno: "(ninguno)",
    },
    Texts {
        lienzo: "Canvas", forma: "Shape", avanzado: "Advanced", proyecto: "Project",
        pintando: "Painting:", cara: "Face", pixel: "Pixel:",
        color: "Color:", selector_color: "Color picker",
        colores_guardados: "Saved colors (click to use, right-click to remove):",
        pincel: "Brush", limpiar_cara: "Clear face",
        formas: "Shapes", cubo: "Cube", cuboide: "Cuboid", piramide: "Pyramid",
        esfera: "Sphere", agregar: "Add", forma_activa: "Active shape",
        nombre: "Name:", posicion: "Position:", rotacion: "Rotation (degrees):",
        x: "X:", y: "Y:", z: "Z:", escala: "Scale:", ancho: "Width (X):",
        alto: "Height (Y):", profundo: "Depth (Z):", radio: "Radius:",
        segmentos: "Segments:", tam_lienzo: "Canvas size:",
        colores_paleta: "Default colors (click to edit):",
        color_relleno: "Fill color (Clear face):",
        guardar: "Save project", guardar_cambios: "Save changes",
        cargar: "Load project", proyectos_guardados: "Saved projects:",
        ninguno: "(none)",
    },
    Texts {
        lienzo: "Toile", forma: "Forme", avanzado: "Avancé", proyecto: "Projet",
        pintando: "Peinture:", cara: "Face", pixel: "Pixel:",
        color: "Couleur:", selector_color: "Sélecteur de couleur",
        colores_guardados: "Couleurs sauvegardées (clic pour utiliser, clic droit pour retirer):",
        pincel: "Pinceau", limpiar_cara: "Effacer la face",
        formas: "Formes", cubo: "Cube", cuboide: "Cuboïde", piramide: "Pyramide",
        esfera: "Sphère", agregar: "Ajouter", forma_activa: "Forme active",
        nombre: "Nom:", posicion: "Position:", rotacion: "Rotation (degrés):",
        x: "X:", y: "Y:", z: "Z:", escala: "Échelle:", ancho: "Largeur (X):",
        alto: "Hauteur (Y):", profundo: "Profondeur (Z):", radio: "Rayon:",
        segmentos: "Segments:", tam_lienzo: "Taille de la toile:",
        colores_paleta: "Couleurs par défaut (clic pour éditer):",
        color_relleno: "Couleur de remplissage (Effacer la face):",
        guardar: "Sauvegarder le projet", guardar_cambios: "Sauvegarder les modifications",
        cargar: "Charger le projet", proyectos_guardados: "Projets sauvegardés:",
        ninguno: "(aucun)",
    },
];
struct UiApp {
    state: Arc<Mutex<SharedState>>,
    show_color_picker: bool,
    picker_color: [u8; 3],
    custom_colors: Vec<[u8; 4]>,
    tab: usize,
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
                1 => self.ui_forma(ui, &mut state),
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
            let forma = &mut state.formas[a];

            match forma.tipo {
                FormaTipo::Esfera => {
                    ui.label("Textura de la esfera (proyección equirrectangular):");
                }
                _ => {
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

                    if is_painting {
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

            if ui.button(self.tx().limpiar_cara).clicked() {
                let c = state.fill_color;
                let cs = state.formas[a]
                    .cara_sel
                    .min(state.formas[a].pixeles.len() - 1);
                for px in state.formas[a].pixeles[cs].chunks_exact_mut(4) {
                    px.copy_from_slice(&c);
                }
                state.dirty = true;
            }

            if !state.mensaje.is_empty() {
                ui.label(&state.mensaje);
            }
        });
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
                    let old_s = forma.esfera_segmentos;
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
                        ui.add(egui::Slider::new(&mut forma.esfera_segmentos, 8..=64).text(""));
                    });
                    if forma.esfera_radio != old_r || forma.esfera_segmentos != old_s {
                        changed = true;
                    }
                }
            }

            if changed {
                state.shape_dirty = true;
                state.dirty = true;
            }
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
                    let dir = desktop_dir()
                        .join("modelador_proyectos")
                        .join(&state.project_name);
                    let msg = guardar_proyecto_dir(state, &dir);
                    state.mensaje = msg;
                    state.project_path = Some(dir);
                    self.proyectos = listar_proyectos();
                }
                if ui.button(self.tx().guardar_cambios).clicked() {
                    if let Some(ref dir) = state.project_path {
                        let msg = guardar_proyecto_dir(state, dir);
                        state.mensaje = msg;
                    } else {
                        state.mensaje = format!("Primero usa \"{}\".", self.tx().guardar);
                    }
                }
                if ui.button(self.tx().cargar).clicked() {
                    self.proyectos = listar_proyectos();
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
                                let msg = importar_proyecto(state, nombre);
                                state.mensaje = msg;
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
        let sky_mesh = crear_esfera_mesh(50.0, 32);
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

    fn crear_esfera_mesh(radio: f32, segmentos: usize) -> Rc<RefCell<Mesh>> {
        use na::{Point2, Point3};
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        let mut uvs = Vec::new();
        let pi = std::f32::consts::PI;
        for lat in 0..=segmentos {
            let theta = lat as f32 * pi / segmentos as f32;
            let sin_t = theta.sin();
            let cos_t = theta.cos();
            for lon in 0..=segmentos {
                let phi = lon as f32 * 2.0 * pi / segmentos as f32;
                let x = phi.cos() * sin_t * radio;
                let y = cos_t * radio;
                let z = phi.sin() * sin_t * radio;
                vertices.push(na::Point3::new(x, y, z));
                uvs.push(Point2::new(
                    lon as f32 / segmentos as f32,
                    lat as f32 / segmentos as f32,
                ));
            }
        }
        for lat in 0..segmentos {
            for lon in 0..segmentos {
                let first = lat * (segmentos + 1) + lon;
                let second = first + segmentos + 1;
                indices.push(Point3::new(
                    first as VertexIndex,
                    second as VertexIndex,
                    (first + 1) as VertexIndex,
                ));
                indices.push(Point3::new(
                    (first + 1) as VertexIndex,
                    second as VertexIndex,
                    (second + 1) as VertexIndex,
                ));
            }
        }
        let mesh = Mesh::new(vertices, indices, None, Some(uvs), false);
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

    fn rebuild_shapes(state: &SharedState, root: &mut SceneNode) -> Vec<ShapeGroup> {
        let mut groups = Vec::new();
        for forma in &state.formas {
            let mut group = root.add_group();
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
                FormaTipo::Esfera => {
                    let mesh = crear_esfera_mesh(forma.esfera_radio, forma.esfera_segmentos);
                    let mut node = group.add_mesh(mesh, na::Vector3::new(1.0, 1.0, 1.0));
                    let tex = crear_textura(&forma.pixeles[0], state.tex_size);
                    node.set_texture(tex.clone());
                    nodes.push(node);
                    textures.push(tex);
                }
                _ => {
                    for (_i, face) in forma.shape_faces.iter().enumerate() {
                        let mesh = crear_mesh_cara(&forma.shape_vertices, face);
                        let mut node = group.add_mesh(mesh, na::Vector3::new(1.0, 1.0, 1.0));
                        let tex = crear_textura(&forma.pixeles[_i], state.tex_size);
                        node.set_texture(tex.clone());
                        node.set_lines_color(Some(na::Point3::new(0.0, 0.0, 0.0)));
                        nodes.push(node);
                        textures.push(tex);
                    }
                }
            }
            groups.push(ShapeGroup {
                group,
                nodes,
                textures,
            });
        }
        groups
    }

    shape_groups = rebuild_shapes(&state.lock().unwrap(), &mut root);

    let mut prev_count = state.lock().unwrap().formas.len();

    while window.render_with_camera(&mut camara) {
        let mut state_ = state.lock().unwrap();

        if state_.shape_dirty || state_.formas.len() != prev_count {
            for mut sg in shape_groups.drain(..) {
                sg.group.unlink();
            }
            shape_groups = rebuild_shapes(&state_, &mut root);
            state_.shape_dirty = false;
            prev_count = state_.formas.len();
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
            }
        }

        // Texture updates for active shape
        let a = state_.forma_activa;
        if a < shape_groups.len() {
            let sg = &mut shape_groups[a];
            let f = &state_.formas[a];
            if state_.dirty || state_.res_dirty {
                match f.tipo {
                    FormaTipo::Esfera => {
                        if let Some(node) = sg.nodes.first_mut() {
                            let tex = crear_textura(&f.pixeles[0], state_.tex_size);
                            node.set_texture(tex);
                        }
                    }
                    _ => {
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
