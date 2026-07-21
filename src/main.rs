use std::sync::{Arc, Mutex};
use std::path::PathBuf;
use serde::{Serialize, Deserialize};

#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
enum Forma {
    Cubo,
    Cuboide,
    PirTriangular,
    PirCuadrada,
    Esfera,
}

fn default_shape_data(forma: Forma) -> (Vec<[f32; 3]>, Vec<Vec<usize>>) {
    match forma {
        Forma::Cubo => (
            vec![
                [-0.5, -0.5,  0.5], [ 0.5, -0.5,  0.5], [ 0.5,  0.5,  0.5], [-0.5,  0.5,  0.5],
                [-0.5, -0.5, -0.5], [ 0.5, -0.5, -0.5], [ 0.5,  0.5, -0.5], [-0.5,  0.5, -0.5],
            ],
            vec![
                vec![0, 1, 2, 3], vec![5, 4, 7, 6], vec![4, 0, 3, 7],
                vec![1, 5, 6, 2], vec![3, 2, 6, 7], vec![4, 5, 1, 0],
            ],
        ),
        Forma::Cuboide => (
            vec![
                [-1.0, -0.5,  0.25], [ 1.0, -0.5,  0.25], [ 1.0,  0.5,  0.25], [-1.0,  0.5,  0.25],
                [-1.0, -0.5, -0.25], [ 1.0, -0.5, -0.25], [ 1.0,  0.5, -0.25], [-1.0,  0.5, -0.25],
            ],
            vec![
                vec![0, 1, 2, 3], vec![5, 4, 7, 6], vec![4, 0, 3, 7],
                vec![1, 5, 6, 2], vec![3, 2, 6, 7], vec![4, 5, 1, 0],
            ],
        ),
        Forma::PirTriangular => (
            vec![
                [0.0, 0.5, 0.0],    // apex
                [-0.5, -0.5, 0.5],  // base 0
                [0.5, -0.5, 0.5],   // base 1
                [0.0, -0.5, -0.5],  // base 2
            ],
            vec![
                vec![1, 2, 3],     // base
                vec![0, 1, 2],     // side 1
                vec![0, 2, 3],     // side 2
                vec![0, 3, 1],     // side 3
            ],
        ),
        Forma::PirCuadrada => (
            vec![
                [0.0, 0.5, 0.0],     // apex
                [-0.5, -0.5, 0.5],   // base 0
                [0.5, -0.5, 0.5],    // base 1
                [0.5, -0.5, -0.5],   // base 2
                [-0.5, -0.5, -0.5],  // base 3
            ],
            vec![
                vec![0, 1, 2],     // side 1
                vec![0, 2, 3],     // side 2
                vec![0, 3, 4],     // side 3
                vec![0, 4, 1],     // side 4
                vec![1, 2, 3, 4],  // base
            ],
        ),
        Forma::Esfera => (vec![], vec![]),
    }
}

struct SharedState {
    forma: Forma,
    pixeles: Vec<Vec<u8>>,
    cara_sel: usize,
    color: [u8; 4],
    tam_pincel: usize,
    dirty: bool,
    mensaje: String,
    tex_size: usize,
    res_dirty: bool,
    paleta: [[u8; 4]; 8],
    fill_color: [u8; 4],
    cubo_escala: f32,
    cuboide_ancho: f32,
    cuboide_alto: f32,
    cuboide_profundo: f32,
    esfera_radio: f32,
    esfera_segmentos: usize,
    project_name: String,
    shape_dirty: bool,
    nuevo_tex_size: usize,
    shape_vertices: Vec<[f32; 3]>,
    shape_faces: Vec<Vec<usize>>,
}

#[derive(Serialize, Deserialize)]
struct ProjectData {
    forma: Forma,
    pixeles: Vec<Vec<u8>>,
    tex_size: usize,
    paleta: [[u8; 4]; 8],
    fill_color: [u8; 4],
    cubo_escala: f32,
    cuboide_ancho: f32,
    cuboide_alto: f32,
    cuboide_profundo: f32,
    esfera_radio: f32,
    esfera_segmentos: usize,
    shape_vertices: Vec<[f32; 3]>,
    shape_faces: Vec<Vec<usize>>,
}

fn crear_pixeles(color: &[u8; 4], tex_size: usize, count: usize) -> Vec<Vec<u8>> {
    (0..count).map(|_| {
        let mut p = vec![0u8; tex_size * tex_size * 4];
        for px in p.chunks_exact_mut(4) { px.copy_from_slice(color); }
        p
    }).collect()
}

fn face_count(forma: Forma) -> usize {
    match forma {
        Forma::Esfera => 1,
        Forma::PirTriangular => 4,
        Forma::PirCuadrada => 5,
        _ => 6,
    }
}

impl SharedState {
    fn new(_colores_ini: &[[u8; 4]; 6], tex_size: usize, paleta: [[u8; 4]; 8]) -> Self {
        let (shape_vertices, shape_faces) = default_shape_data(Forma::Cubo);
        let pixeles = crear_pixeles(&[128, 128, 128, 255], tex_size, 6);
        Self {
            forma: Forma::Cubo,
            pixeles,
            cara_sel: 0, color: [0, 0, 0, 255], tam_pincel: 1,
            dirty: true, mensaje: String::new(), tex_size, res_dirty: false,
            paleta, fill_color: [128, 128, 128, 255],
            cubo_escala: 1.0, cuboide_ancho: 2.0, cuboide_alto: 1.0, cuboide_profundo: 0.5,
            esfera_radio: 1.0, esfera_segmentos: 24,
            project_name: String::from("mi_proyecto"),
            shape_dirty: true, nuevo_tex_size: tex_size,
            shape_vertices, shape_faces,
        }
    }

    fn cambiar_forma(&mut self, nueva: Forma) {
        if self.forma == nueva { return; }
        let (verts, faces) = default_shape_data(nueva);
        self.forma = nueva;
        self.shape_vertices = verts;
        self.shape_faces = faces;
        self.cara_sel = 0;
        self.shape_dirty = true;
        self.dirty = true;
        let count = face_count(nueva);
        if self.pixeles.len() != count {
            let old_count = self.pixeles.len();
            let default_color = if old_count > 0 {
                let mut c = [0u8; 4];
                c.copy_from_slice(&self.pixeles[0][..4]);
                c
            } else {
                [128, 128, 128, 255]
            };
            self.pixeles = crear_pixeles(&default_color, self.tex_size, count);
        }
    }
}

fn redimensionar_pixeles(pixeles: &mut Vec<Vec<u8>>, old_size: usize, new_size: usize) {
    for cara in pixeles.iter_mut() {
        let mut nuevos = vec![0u8; new_size * new_size * 4];
        for y in 0..new_size {
            for x in 0..new_size {
                let oy = y.min(old_size - 1);
                let ox = x.min(old_size - 1);
                let src = (oy * old_size + ox) * 4;
                let dst = (y * new_size + x) * 4;
                nuevos[dst..dst+4].copy_from_slice(&cara[src..src+4]);
            }
        }
        *cara = nuevos;
    }
}

fn desktop_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    for dir in &["Escritorio", "Desktop", "桌面"] {
        let p = PathBuf::from(&home).join(dir);
        if p.is_dir() { return p; }
    }
    PathBuf::from(&home)
}

fn exportar_obj(state: &SharedState) -> String {
    let dir = desktop_dir().join("modelo_3d");
    std::fs::create_dir_all(&dir).ok();
    match state.forma {
        Forma::Esfera => exportar_obj_esfera(state, &dir),
        _ => exportar_obj_mesh(state, &dir),
    }
}

fn exportar_obj_mesh(state: &SharedState, dir: &PathBuf) -> String {
    let nfaces = state.pixeles.len();
    for i in 0..nfaces {
        let path = dir.join(format!("cara_{}.png", i + 1));
        let img = image::RgbaImage::from_raw(state.tex_size as u32, state.tex_size as u32, state.pixeles[i].clone());
        if let Some(img) = img { img.save(&path).ok(); }
    }
    let mut mtl = String::new();
    for i in 0..nfaces {
        mtl.push_str(&format!("newmtl cara_{}\nmap_Kd cara_{}.png\n\n", i + 1, i + 1));
    }
    std::fs::write(dir.join("modelo.mtl"), &mtl).ok();

    let mut obj = String::new();
    obj.push_str("# Exportado de Modelador 3D\nmtllib modelo.mtl\n\n");
    for v in &state.shape_vertices {
        obj.push_str(&format!("v {} {} {}\n", v[0], v[1], v[2]));
    }
    obj.push_str("\nvt 0 0\nvt 1 0\nvt 1 1\nvt 0 1\n\n");
    for (i, face) in state.shape_faces.iter().enumerate() {
        obj.push_str(&format!("usemtl cara_{}\n", i + 1));
        match face.len() {
            3 => {
                obj.push_str(&format!("f {}/1 {}/2 {}/3\n", face[0] + 1, face[1] + 1, face[2] + 1));
            }
            4 => {
                obj.push_str(&format!(
                    "f {}/1 {}/2 {}/3\nf {}/1 {}/3 {}/4\n",
                    face[0] + 1, face[1] + 1, face[2] + 1,
                    face[0] + 1, face[2] + 1, face[3] + 1,
                ));
            }
            _ => {}
        }
    }
    std::fs::write(dir.join("modelo.obj"), &obj).ok();
    format!("Exportado a: {}", dir.display())
}

fn exportar_obj_esfera(state: &SharedState, dir: &PathBuf) -> String {
    // export texture
    let path = dir.join("esfera.png");
    let img = image::RgbaImage::from_raw(state.tex_size as u32, state.tex_size as u32, state.pixeles[0].clone());
    if let Some(img) = img { img.save(&path).ok(); }

    let mut mtl = String::new();
    mtl.push_str("newmtl esfera\nmap_Kd esfera.png\n\n");
    std::fs::write(dir.join("modelo.mtl"), &mtl).ok();

    let seg = state.esfera_segmentos;
    let r = state.esfera_radio;
    let mut obj = String::new();
    obj.push_str("# Exportado de Modelador 3D\nmtllib modelo.mtl\n\n");

    // vertices with UVs
    for lat in 0..=seg {
        let theta = lat as f32 * std::f32::consts::PI / seg as f32;
        let sin_t = theta.sin();
        let cos_t = theta.cos();
        for lon in 0..=seg {
            let phi = lon as f32 * 2.0 * std::f32::consts::PI / seg as f32;
            let x = phi.cos() * sin_t * r;
            let y = cos_t * r;
            let z = phi.sin() * sin_t * r;
            obj.push_str(&format!("v {} {} {}\n", x, y, z));
        }
    }
    obj.push_str("\n");
    for lat in 0..=seg {
        for lon in 0..=seg {
            let u = lon as f32 / seg as f32;
            let v = lat as f32 / seg as f32;
            obj.push_str(&format!("vt {} {}\n", u, v));
        }
    }
    obj.push_str("\n");
    for lat in 0..seg {
        for lon in 0..seg {
            let a = lat * (seg + 1) + lon + 1;
            let b = a + seg + 1;
            obj.push_str(&format!("f {}/{} {}/{} {}/{}\n", a, a, b, b, a+1, a+1));
            obj.push_str(&format!("f {}/{} {}/{} {}/{}\n", a+1, a+1, b, b, b+1, b+1));
        }
    }
    std::fs::write(dir.join("modelo.obj"), &obj).ok();
    format!("Exportado a: {}", dir.display())
}

fn exportar_proyecto(state: &SharedState) -> String {
    let dir = desktop_dir().join("modelador_proyectos");
    std::fs::create_dir_all(&dir).ok();
    let data = ProjectData {
        forma: state.forma,
        pixeles: state.pixeles.clone(),
        tex_size: state.tex_size,
        paleta: state.paleta,
        fill_color: state.fill_color,
        cubo_escala: state.cubo_escala,
        cuboide_ancho: state.cuboide_ancho,
        cuboide_alto: state.cuboide_alto,
        cuboide_profundo: state.cuboide_profundo,
        esfera_radio: state.esfera_radio,
        esfera_segmentos: state.esfera_segmentos,
        shape_vertices: state.shape_vertices.clone(),
        shape_faces: state.shape_faces.clone(),
    };
    let json = serde_json::to_string_pretty(&data).unwrap();
    let path = dir.join(format!("{}.json", state.project_name));
    std::fs::write(&path, &json).ok();
    format!("Proyecto guardado: {}", path.display())
}

fn importar_proyecto(state: &mut SharedState, nombre: &str) -> String {
    let dir = desktop_dir().join("modelador_proyectos");
    let path = dir.join(format!("{}.json", nombre));
    let json = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) => return format!("Error al leer: {}", e),
    };
    let data: ProjectData = match serde_json::from_str(&json) {
        Ok(d) => d,
        Err(e) => return format!("Error al parsear: {}", e),
    };
    state.forma = data.forma;
    state.pixeles = data.pixeles;
    state.tex_size = data.tex_size;
    state.nuevo_tex_size = data.tex_size;
    state.paleta = data.paleta;
    state.fill_color = data.fill_color;
    state.cubo_escala = data.cubo_escala;
    state.cuboide_ancho = data.cuboide_ancho;
    state.cuboide_alto = data.cuboide_alto;
    state.cuboide_profundo = data.cuboide_profundo;
    state.esfera_radio = data.esfera_radio;
    state.esfera_segmentos = data.esfera_segmentos;
    state.shape_vertices = data.shape_vertices;
    state.shape_faces = data.shape_faces;
    state.project_name = nombre.to_string();
    state.cara_sel = 0;
    state.shape_dirty = true;
    state.dirty = true;
    state.res_dirty = true;
    format!("Proyecto cargado: {}", path.display())
}

fn listar_proyectos() -> Vec<String> {
    let dir = desktop_dir().join("modelador_proyectos");
    if !dir.is_dir() { return Vec::new(); }
    let mut nombres = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.extension().map(|e| e == "json").unwrap_or(false) {
                if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
                    nombres.push(stem.to_string());
                }
            }
        }
    }
    nombres.sort();
    nombres
}

fn main() {
    let colores_ini: [[u8; 4]; 6] = [
        [255, 100, 100, 255], [100, 255, 100, 255],
        [100, 100, 255, 255], [255, 255, 100, 255],
        [255, 100, 255, 255], [100, 255, 255, 255],
    ];

    let paleta_ini: [[u8; 4]; 8] = [
        [0, 0, 0, 255], [255, 0, 0, 255], [0, 200, 0, 255],
        [0, 0, 255, 255], [255, 255, 0, 255], [255, 0, 255, 255],
        [0, 200, 200, 255], [255, 255, 255, 255],
    ];

    let state = Arc::new(Mutex::new(SharedState::new(&colores_ini, 8, paleta_ini)));

    let state_3d = state.clone();
    let hilo_3d = std::thread::spawn(move || usar_kiss3d(state_3d));

    let app = UiApp {
        state,
        show_color_picker: false, picker_color: [0, 0, 0],
        custom_colors: Vec::new(), tab: 0,
        editing_paleta: None, show_fill_picker: false,
        proyectos: listar_proyectos(),
    };
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size(egui::vec2(400.0, 560.0)),
        ..Default::default()
    };
    eframe::run_native("Pintura - Modelador 3D", opts, Box::new(|_| Ok(Box::new(app)))).ok();

    hilo_3d.join().ok();
}

// ============================================================
// UI (egui)
// ============================================================
struct UiApp {
    state: Arc<Mutex<SharedState>>,
    show_color_picker: bool,
    picker_color: [u8; 3],
    custom_colors: Vec<[u8; 4]>,
    tab: usize,
    editing_paleta: Option<usize>,
    show_fill_picker: bool,
    proyectos: Vec<String>,
}

impl eframe::App for UiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.selectable_label(self.tab == 0, "Lienzo").clicked() { self.tab = 0; }
                if ui.selectable_label(self.tab == 1, "Forma").clicked() { self.tab = 1; }
                if ui.selectable_label(self.tab == 2, "Avanzado").clicked() { self.tab = 2; }
                if ui.selectable_label(self.tab == 3, "Proyecto").clicked() { self.tab = 3; }
            });
            ui.separator();

            let state_arc = self.state.clone();
            let mut state = state_arc.lock().unwrap();

            let num_keys = [
                egui::Key::Num1, egui::Key::Num2, egui::Key::Num3, egui::Key::Num4,
                egui::Key::Num5, egui::Key::Num6, egui::Key::Num7, egui::Key::Num8,
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
    fn ui_lienzo(&mut self, ui: &mut egui::Ui, state: &mut SharedState) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            match state.forma {
                Forma::Esfera => {
                    ui.label("Textura de la esfera (proyección equirrectangular):");
                }
                _ => {
                    ui.horizontal(|ui| {
                        for i in 0..state.pixeles.len() {
                            let label = format!("Cara {}", i + 1);
                            if ui.selectable_label(state.cara_sel == i, &label).clicked() {
                                state.cara_sel = i; state.dirty = true;
                            }
                        }
                    });
                }
            }

            ui.separator();

            let tex_size = state.tex_size;
            let target = 280.0f32;
            let celda = (target / tex_size as f32).max(10.0).min(35.0);
            let (resp, painter) = ui.allocate_painter(
                egui::Vec2::new(tex_size as f32 * celda, tex_size as f32 * celda),
                egui::Sense::click_and_drag(),
            );
            let pixeles = &state.pixeles[state.cara_sel.min(state.pixeles.len() - 1)];
            for y in 0..tex_size {
                for x in 0..tex_size {
                    let i = (y * tex_size + x) * 4;
                    let min = egui::pos2(resp.rect.min.x + x as f32 * celda, resp.rect.min.y + y as f32 * celda);
                    let rect = egui::Rect::from_min_max(min, egui::pos2(min.x + celda, min.y + celda));
                    let c = egui::Color32::from_rgb(pixeles[i], pixeles[i+1], pixeles[i+2]);
                    painter.rect_filled(rect, 0.0, c);
                    painter.rect_stroke(rect, 0.0, egui::Stroke::new(1.0, egui::Color32::GRAY));
                }
            }

            if let Some(pos) = resp.hover_pos() {
                let px = ((pos.x - resp.rect.min.x) / celda) as usize;
                let py = ((pos.y - resp.rect.min.y) / celda) as usize;

                if px < tex_size && py < tex_size {
                    let is_painting = ui.input(|i| i.pointer.any_down());

                    ui.label(format!("Pixel: ({}, {})", px, py));

                    let center = egui::pos2(
                        resp.rect.min.x + (px as f32 + 0.5) * celda,
                        resp.rect.min.y + (py as f32 + 0.5) * celda,
                    );
                    let r = state.tam_pincel as f32 * celda / 2.0;
                    if r > 0.0 {
                        painter.circle_stroke(center, r, egui::Stroke::new(2.0, egui::Color32::WHITE));
                    }

                    if is_painting {
                        let t = state.tam_pincel;
                        let color = state.color;
                        let cs = state.cara_sel.min(state.pixeles.len() - 1);
                        for y in py.saturating_sub(t/2)..(py + (t+1)/2).min(tex_size) {
                            for x in px.saturating_sub(t/2)..(px + (t+1)/2).min(tex_size) {
                                let i = (y * tex_size + x) * 4;
                                state.pixeles[cs][i..i+4].copy_from_slice(&color);
                            }
                        }
                        state.dirty = true;
                    }

                    if resp.secondary_clicked() {
                        let i = (py * tex_size + px) * 4;
                        let cs = state.cara_sel.min(state.pixeles.len() - 1);
                        state.color = [
                            state.pixeles[cs][i], state.pixeles[cs][i+1],
                            state.pixeles[cs][i+2], 255,
                        ];
                    }
                }
            }

            ui.separator();

            // Paleta
            ui.label("Color:");
            let paleta = state.paleta;
            ui.horizontal(|ui| {
                for &c in &paleta {
                    let color = egui::Color32::from_rgb(c[0], c[1], c[2]);
                    let (id, rect) = ui.allocate_space(egui::vec2(24.0, 24.0));
                    let resp = ui.interact(rect, id, egui::Sense::click());
                    ui.painter().rect_filled(rect, 2.0, color);
                    ui.painter().rect_stroke(rect, 2.0, egui::Stroke::new(1.0, egui::Color32::DARK_GRAY));
                    if resp.clicked() { state.color = c; }
                }
                if ui.button("+").clicked() {
                    self.show_color_picker = true;
                    self.picker_color = [state.color[0], state.color[1], state.color[2]];
                }
            });

            if self.show_color_picker {
                let mut open = true;
                egui::Window::new("Selector de color")
                    .open(&mut open)
                    .default_width(300.0)
                    .show(ui.ctx(), |ui| {
                        ui.spacing_mut().slider_width = ui.available_width() - 10.0;
                        let mut c32 = egui::Color32::from_rgb(self.picker_color[0], self.picker_color[1], self.picker_color[2]);
                        if egui::color_picker::color_picker_color32(ui, &mut c32, egui::color_picker::Alpha::Opaque) {
                            self.picker_color = [c32[0], c32[1], c32[2]];
                            state.color = [c32[0], c32[1], c32[2], 255];
                        }
                    });
                if !open {
                    self.show_color_picker = false;
                    let color = state.color;
                    if !self.custom_colors.iter().any(|&c| c == color) {
                        self.custom_colors.push(color);
                    }
                }
            }

            if !self.custom_colors.is_empty() {
                ui.label("Colores guardados (clic para usar, clic derecho para quitar):");
                ui.horizontal(|ui| {
                    let mut quitar = None;
                    for (i, &c) in self.custom_colors.iter().enumerate() {
                        let color = egui::Color32::from_rgb(c[0], c[1], c[2]);
                        let (id, rect) = ui.allocate_space(egui::vec2(24.0, 24.0));
                        let resp = ui.interact(rect, id, egui::Sense::click());
                        ui.painter().rect_filled(rect, 2.0, color);
                        ui.painter().rect_stroke(rect, 2.0, egui::Stroke::new(1.0, egui::Color32::DARK_GRAY));
                        if resp.clicked() { state.color = c; }
                        if resp.secondary_clicked() { quitar = Some(i); }
                    }
                    if let Some(i) = quitar { self.custom_colors.remove(i); }
                });
            }

            ui.separator();

            ui.add(egui::Slider::new(&mut state.tam_pincel, 0..=8).text("Pincel"));

            if ui.button("Limpiar cara").clicked() {
                let c = state.fill_color;
                let cs = state.cara_sel.min(state.pixeles.len() - 1);
                for px in state.pixeles[cs].chunks_exact_mut(4) { px.copy_from_slice(&c); }
                state.dirty = true;
            }

            ui.separator();

            if ui.button("Exportar OBJ").clicked() {
                let msg = exportar_obj(state);
                state.mensaje = msg;
            }
            if !state.mensaje.is_empty() {
                ui.label(&state.mensaje);
            }
        });
    }

    fn ui_forma(&mut self, ui: &mut egui::Ui, state: &mut SharedState) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.heading("Forma geométrica");
            ui.separator();

            ui.horizontal(|ui| {
                if ui.selectable_label(state.forma == Forma::Cubo, "Cubo").clicked() {
                    state.cambiar_forma(Forma::Cubo);
                }
                if ui.selectable_label(state.forma == Forma::Cuboide, "Cuboide").clicked() {
                    state.cambiar_forma(Forma::Cuboide);
                }
                if ui.selectable_label(state.forma == Forma::PirTriangular, "Pir. Triáng.").clicked() {
                    state.cambiar_forma(Forma::PirTriangular);
                }
                if ui.selectable_label(state.forma == Forma::PirCuadrada, "Pir. Cuadrada").clicked() {
                    state.cambiar_forma(Forma::PirCuadrada);
                }
                if ui.selectable_label(state.forma == Forma::Esfera, "Esfera").clicked() {
                    state.cambiar_forma(Forma::Esfera);
                }
            });

            ui.separator();

            match state.forma {
                Forma::Cubo => {
                    let old = state.cubo_escala;
                    ui.horizontal(|ui| {
                        ui.label("Escala:");
                        if ui.button("−").clicked() { state.cubo_escala = (state.cubo_escala - 0.1).max(0.1); }
                        ui.label(format!("{:.1}", state.cubo_escala));
                        if ui.button("+").clicked() { state.cubo_escala = (state.cubo_escala + 0.1).min(5.0); }
                    });
                    if state.cubo_escala != old {
                        let s = state.cubo_escala / 2.0;
                        let v = &mut state.shape_vertices;
                        v[0] = [-s, -s,  s]; v[1] = [ s, -s,  s]; v[2] = [ s,  s,  s]; v[3] = [-s,  s,  s];
                        v[4] = [-s, -s, -s]; v[5] = [ s, -s, -s]; v[6] = [ s,  s, -s]; v[7] = [-s,  s, -s];
                    }
                }
                Forma::Cuboide => {
                    let old = (state.cuboide_ancho, state.cuboide_alto, state.cuboide_profundo);
                    ui.horizontal(|ui| {
                        ui.label("Ancho (X):");
                        if ui.button("−").clicked() { state.cuboide_ancho = (state.cuboide_ancho - 0.1).max(0.1); }
                        ui.label(format!("{:.1}", state.cuboide_ancho));
                        if ui.button("+").clicked() { state.cuboide_ancho = (state.cuboide_ancho + 0.1).min(5.0); }
                    });
                    ui.horizontal(|ui| {
                        ui.label("Alto (Y):");
                        if ui.button("−").clicked() { state.cuboide_alto = (state.cuboide_alto - 0.1).max(0.1); }
                        ui.label(format!("{:.1}", state.cuboide_alto));
                        if ui.button("+").clicked() { state.cuboide_alto = (state.cuboide_alto + 0.1).min(5.0); }
                    });
                    ui.horizontal(|ui| {
                        ui.label("Profundo (Z):");
                        if ui.button("−").clicked() { state.cuboide_profundo = (state.cuboide_profundo - 0.1).max(0.1); }
                        ui.label(format!("{:.1}", state.cuboide_profundo));
                        if ui.button("+").clicked() { state.cuboide_profundo = (state.cuboide_profundo + 0.1).min(5.0); }
                    });
                    if (state.cuboide_ancho, state.cuboide_alto, state.cuboide_profundo) != old {
                        let (w, h, d) = (state.cuboide_ancho / 2.0, state.cuboide_alto / 2.0, state.cuboide_profundo / 2.0);
                        let v = &mut state.shape_vertices;
                        v[0] = [-w, -h,  d]; v[1] = [ w, -h,  d]; v[2] = [ w,  h,  d]; v[3] = [-w,  h,  d];
                        v[4] = [-w, -h, -d]; v[5] = [ w, -h, -d]; v[6] = [ w,  h, -d]; v[7] = [-w,  h, -d];
                    }
                }
                Forma::Esfera => {
                    ui.horizontal(|ui| {
                        ui.label("Radio:");
                        if ui.button("−").clicked() { state.esfera_radio = (state.esfera_radio - 0.1).max(0.1); }
                        ui.label(format!("{:.1}", state.esfera_radio));
                        if ui.button("+").clicked() { state.esfera_radio = (state.esfera_radio + 0.1).min(5.0); }
                    });
                    ui.horizontal(|ui| {
                        ui.label("Segmentos:");
                        if ui.button("−").clicked() { state.esfera_segmentos = state.esfera_segmentos.saturating_sub(1).max(8); }
                        ui.label(state.esfera_segmentos.to_string());
                        if ui.button("+").clicked() { state.esfera_segmentos = (state.esfera_segmentos + 1).min(64); }
                    });
                    ui.label("Más segmentos = superficie más suave.");
                }
                _ => {}
            }

            if state.forma != Forma::Esfera {
                ui.separator();
                ui.label("Vértices (arrastrar para editar):");
                let mut changed = false;
                for (i, v) in state.shape_vertices.iter_mut().enumerate() {
                    ui.horizontal(|ui| {
                        ui.label(format!("V{}:", i));
                        changed |= ui.add(egui::DragValue::new(&mut v[0]).speed(0.01).prefix("x ")).dragged();
                        changed |= ui.add(egui::DragValue::new(&mut v[1]).speed(0.01).prefix("y ")).dragged();
                        changed |= ui.add(egui::DragValue::new(&mut v[2]).speed(0.01).prefix("z ")).dragged();
                    });
                }
                if changed {
                    state.shape_dirty = true;
                    state.dirty = true;
                }
            }

            state.shape_dirty = true;
            state.dirty = true;
        });
    }

    fn ui_avanzado(&mut self, ui: &mut egui::Ui, state: &mut SharedState) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.heading("Lienzo");
            ui.separator();

            ui.label("Tamaño del lienzo:");
            let old_size = state.tex_size;
            let opciones = [8usize, 16, 32];
            ui.horizontal(|ui| {
                for &op in &opciones {
                    ui.radio_value(&mut state.tex_size, op, format!("{}×{}", op, op));
                }
            });
            if state.tex_size != old_size {
                redimensionar_pixeles(&mut state.pixeles, old_size, state.tex_size);
                state.res_dirty = true;
                state.dirty = true;
            }

            ui.separator();

            ui.label("Colores por defecto (clic para editar):");
            let mut edit_idx = None;
            ui.horizontal(|ui| {
                for (i, &c) in state.paleta.iter().enumerate() {
                    let color = egui::Color32::from_rgb(c[0], c[1], c[2]);
                    let (id, rect) = ui.allocate_space(egui::vec2(24.0, 24.0));
                    let resp = ui.interact(rect, id, egui::Sense::click());
                    ui.painter().rect_filled(rect, 2.0, color);
                    ui.painter().rect_stroke(rect, 2.0, egui::Stroke::new(1.0, egui::Color32::DARK_GRAY));
                    if resp.clicked() { edit_idx = Some(i); }
                }
            });
            if let Some(idx) = edit_idx { self.editing_paleta = Some(idx); }

            if let Some(idx) = self.editing_paleta {
                let mut open = true;
                let mut c32 = egui::Color32::from_rgb(state.paleta[idx][0], state.paleta[idx][1], state.paleta[idx][2]);
                egui::Window::new(format!("Editar color #{}", idx + 1))
                    .open(&mut open)
                    .default_width(300.0)
                    .show(ui.ctx(), |ui| {
                        ui.spacing_mut().slider_width = ui.available_width() - 10.0;
                        if egui::color_picker::color_picker_color32(ui, &mut c32, egui::color_picker::Alpha::Opaque) {
                            state.paleta[idx] = [c32[0], c32[1], c32[2], 255];
                        }
                    });
                if !open { self.editing_paleta = None; }
            }

            ui.separator();

            // fill color
            ui.label("Color de relleno (Limpiar cara):");
            let mut c32 = egui::Color32::from_rgb(state.fill_color[0], state.fill_color[1], state.fill_color[2]);
            let (id, rect) = ui.allocate_space(egui::vec2(24.0, 24.0));
            let resp = ui.interact(rect, id, egui::Sense::click());
            ui.painter().rect_filled(rect, 2.0, c32);
            ui.painter().rect_stroke(rect, 2.0, egui::Stroke::new(1.0, egui::Color32::DARK_GRAY));
            if resp.clicked() { self.show_fill_picker = true; }

            if self.show_fill_picker {
                let mut win_open = true;
                egui::Window::new("Color de relleno")
                    .open(&mut win_open)
                    .default_width(300.0)
                    .show(ui.ctx(), |ui| {
                        ui.spacing_mut().slider_width = ui.available_width() - 10.0;
                        if egui::color_picker::color_picker_color32(ui, &mut c32, egui::color_picker::Alpha::Opaque) {
                            state.fill_color = [c32[0], c32[1], c32[2], 255];
                        }
                    });
                if !win_open { self.show_fill_picker = false; }
            }
        });
    }

    fn ui_proyecto(&mut self, ui: &mut egui::Ui, state: &mut SharedState) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.heading("Proyecto");
            ui.separator();

            ui.horizontal(|ui| {
                ui.label("Nombre:");
                ui.text_edit_singleline(&mut state.project_name);
            });

            ui.horizontal(|ui| {
                if ui.button("Guardar proyecto").clicked() {
                    let msg = exportar_proyecto(state);
                    state.mensaje = msg;
                    self.proyectos = listar_proyectos();
                }
                if ui.button("Cargar proyecto").clicked() {
                    self.proyectos = listar_proyectos();
                }
            });

            ui.separator();
            ui.label("Proyectos guardados:");
            let proyectos = self.proyectos.clone();
            if proyectos.is_empty() {
                ui.label("(ninguno)");
            } else {
                egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
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
    use std::rc::Rc;
    use std::cell::RefCell;
    use kiss3d::window::Window;
    use kiss3d::camera::ArcBall;
    use kiss3d::event::{Key, Action};
    use kiss3d::context::Context;
    use kiss3d::resource::{TextureManager, Mesh};
    use kiss3d::nalgebra as na;
    use kiss3d::scene::SceneNode;
    use kiss3d::resource::vertex_index::VertexIndex;
    use image::RgbaImage;

    let mut window = Window::new("Modelador 3D");
    let mut camara = ArcBall::new(
        na::Point3::new(3.0, 2.5, 3.0),
        na::Point3::new(0.0, 0.0, 0.0),
    );

    let mut grupo = window.add_group();

    // Dynamic face nodes (rebuilt on shape change)
    let mut caras_nodos: Vec<SceneNode> = Vec::new();
    // sphere node
    let mut esfera_nodo: Option<SceneNode> = None;
    let mut texturas: Vec<Rc<kiss3d::context::Texture>> = Vec::new();

    fn tex_id() -> String {
        static C: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        format!("t_{}", C.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
    }

    fn crear_textura(pixeles: &[u8], tex_size: usize) -> Rc<kiss3d::context::Texture> {
        let mut img = RgbaImage::from_raw(tex_size as u32, tex_size as u32, pixeles.to_vec()).unwrap();
        image::imageops::flip_horizontal_in_place(&mut img);
        let dyn_img = image::DynamicImage::ImageRgba8(img);
        let tex = TextureManager::get_global_manager(|tm| tm.add_image(dyn_img.clone(), &tex_id()));
        let ctxt = Context::get();
        ctxt.bind_texture(Context::TEXTURE_2D, Some(&tex));
        ctxt.tex_parameteri(Context::TEXTURE_2D, Context::TEXTURE_MAG_FILTER, Context::NEAREST as i32);
        ctxt.tex_parameteri(Context::TEXTURE_2D, Context::TEXTURE_MIN_FILTER, Context::NEAREST as i32);
        tex
    }

    fn subir_textura(tex: &kiss3d::context::Texture, pixeles: &[u8], tex_size: usize) {
        let mut img = RgbaImage::from_raw(tex_size as u32, tex_size as u32, pixeles.to_vec()).unwrap();
        image::imageops::flip_horizontal_in_place(&mut img);
        let ctxt = Context::get();
        ctxt.bind_texture(Context::TEXTURE_2D, Some(tex));
        ctxt.tex_sub_image2d(Context::TEXTURE_2D, 0, 0, 0, tex_size as i32, tex_size as i32, Context::RGBA, Some(img.as_raw()));
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
                uvs.push(Point2::new(lon as f32 / segmentos as f32, lat as f32 / segmentos as f32));
            }
        }
        for lat in 0..segmentos {
            for lon in 0..segmentos {
                let first = lat * (segmentos + 1) + lon;
                let second = first + segmentos + 1;
                indices.push(Point3::new(first as VertexIndex, second as VertexIndex, (first + 1) as VertexIndex));
                indices.push(Point3::new((first + 1) as VertexIndex, second as VertexIndex, (second + 1) as VertexIndex));
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

    // Initial build
    {
        let state_ = state.lock().unwrap();
        if state_.forma != Forma::Esfera {
            let verts = &state_.shape_vertices;
            for (i, face) in state_.shape_faces.iter().enumerate() {
                let mesh = crear_mesh_cara(verts, face);
                let mut nodo = grupo.add_mesh(mesh, na::Vector3::new(1.0, 1.0, 1.0));
                let tex = crear_textura(&state_.pixeles[i], state_.tex_size);
                nodo.set_texture(tex.clone());
                nodo.set_lines_color(Some(na::Point3::new(0.0, 0.0, 0.0)));
                caras_nodos.push(nodo);
                texturas.push(tex);
            }
        } else {
            let mesh = crear_esfera_mesh(state_.esfera_radio, state_.esfera_segmentos);
            let mut nodo = grupo.add_mesh(mesh, na::Vector3::new(1.0, 1.0, 1.0));
            let tex = crear_textura(&state_.pixeles[0], state_.tex_size);
            nodo.set_texture(tex);
            esfera_nodo = Some(nodo);
        }
    }

    let mut prev_forma = Forma::Cubo;

    while window.render_with_camera(&mut camara) {
        let mut state_ = state.lock().unwrap();

        // Handle shape/vertex changes
        if state_.shape_dirty || state_.forma != prev_forma {
            let forma = state_.forma;
            match forma {
                Forma::Esfera => {
                    for nodo in &mut caras_nodos { nodo.set_visible(false); }
                    if esfera_nodo.is_none() || prev_forma != forma {
                        let mesh = crear_esfera_mesh(state_.esfera_radio, state_.esfera_segmentos);
                        let mut nuevo = grupo.add_mesh(mesh, na::Vector3::new(1.0, 1.0, 1.0));
                        let tex = crear_textura(&state_.pixeles[0], state_.tex_size);
                        nuevo.set_texture(tex);
                        esfera_nodo = Some(nuevo);
                    }
                }
                _ => {
                    if let Some(ref mut nodo) = esfera_nodo { nodo.set_visible(false); }
                    caras_nodos.clear();
                    texturas.clear();
                    for (i, face) in state_.shape_faces.iter().enumerate() {
                        let mesh = crear_mesh_cara(&state_.shape_vertices, face);
                        let mut nodo = grupo.add_mesh(mesh, na::Vector3::new(1.0, 1.0, 1.0));
                        let tex = crear_textura(&state_.pixeles[i], state_.tex_size);
                        nodo.set_texture(tex.clone());
                        nodo.set_lines_color(Some(na::Point3::new(0.0, 0.0, 0.0)));
                        caras_nodos.push(nodo);
                        texturas.push(tex);
                    }
                    state_.res_dirty = false;
                    state_.dirty = false;
                }
            }
            state_.shape_dirty = false;
            prev_forma = forma;
            drop(state_);
            continue;
        }

        // Normal update for current shape
        match state_.forma {
            Forma::Esfera => {
                if state_.dirty || state_.res_dirty {
                    if let Some(ref mut nodo) = esfera_nodo {
                        let tex = crear_textura(&state_.pixeles[0], state_.tex_size);
                        nodo.set_texture(tex);
                    }
                    state_.res_dirty = false;
                    state_.dirty = false;
                }
            }
            _ => {
                if state_.res_dirty {
                    let tex_size = state_.tex_size;
                    texturas.clear();
                    for i in 0..caras_nodos.len() {
                        let tex = crear_textura(&state_.pixeles[i], tex_size);
                        caras_nodos[i].set_texture(tex.clone());
                        caras_nodos[i].set_lines_color(Some(na::Point3::new(0.0, 0.0, 0.0)));
                        texturas.push(tex);
                    }
                    state_.res_dirty = false;
                    state_.dirty = false;
                } else if state_.dirty {
                    for i in 0..texturas.len() {
                        subir_textura(&texturas[i], &state_.pixeles[i], state_.tex_size);
                    }
                    state_.dirty = false;
                }
            }
        }

        let cara_sel = state_.cara_sel;
        let forma = state_.forma;
        drop(state_);

        // Highlight selected face
        match forma {
            Forma::Esfera => {
                if let Some(ref mut nodo) = esfera_nodo {
                    nodo.set_lines_color(Some(na::Point3::new(1.0, 1.0, 1.0)));
                }
            }
            _ => {
                for i in 0..caras_nodos.len() {
                    caras_nodos[i].set_lines_color(Some(if i == cara_sel {
                        na::Point3::new(1.0, 1.0, 1.0)
                    } else {
                        na::Point3::new(0.0, 0.0, 0.0)
                    }));
                }
            }
        }

        // Movement/rotation controls
        let v = 0.05;
        for (k, d) in [(Key::Left, [-v,0.0,0.0]), (Key::Right, [v,0.0,0.0]),
                        (Key::Up, [0.0,v,0.0]), (Key::Down, [0.0,-v,0.0]),
                        (Key::W, [0.0,0.0,-v]), (Key::S, [0.0,0.0,v])] {
            if window.get_key(k) == Action::Press {
                grupo.append_translation(&na::Translation3::new(d[0], d[1], d[2]));
            }
        }

        let mut rr = [0.0; 3];
        for (k, i, s) in [(Key::Q,1,-0.03),(Key::E,1,0.03),(Key::R,0,-0.03),
                          (Key::T,0,0.03),(Key::Y,2,0.03),(Key::U,2,-0.03)] {
            if window.get_key(k) == Action::Press { rr[i] += s; }
        }
        if rr[0] != 0.0 || rr[1] != 0.0 || rr[2] != 0.0 {
            grupo.prepend_to_local_rotation(&na::UnitQuaternion::from_euler_angles(rr[0], rr[1], rr[2]));
        }

        if window.get_key(Key::Equals) == Action::Press {
            let s = grupo.data().local_scale();
            grupo.set_local_scale(s.x * 1.01, s.y * 1.01, s.z * 1.01);
        }
        if window.get_key(Key::Minus) == Action::Press {
            let s = grupo.data().local_scale();
            let n = (s.x * 0.99).max(0.1);
            grupo.set_local_scale(n, n, n);
        }
    }
}
