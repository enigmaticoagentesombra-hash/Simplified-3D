# Modelador 3D - Proyecto

App en Rust para modelar y pintar assets 3D simples.

## Estado actual
- 8 tipos de forma: Cubo, Cuboide, Pirámide Cuadrada, Esfera, Cilindro, Cono, Cápsula, Plano (Pirámide Triangular eliminada)
- Múltiples formas independientes en la escena, cada una con su propio nombre, posición y texturas
- Cada forma tiene caras independientes con textura pintable
- Resolución de textura variable (8×8, 16×16, 32×32)
- Ventana 3D (kiss3d) con cámara orbital
- Ventana UI (eframe/egui) con 4 pestañas: Lienzo, Forma, Avanzado, Proyecto
- Guardado/carga de proyectos en un solo archivo `.glb` (mallas + texturas embebidas + metadatos de edición en `extras`)
- El `.glb` se abre directamente en Blender/Unity; también la app lo vuelve a cargar para seguir editando

## Controles 3D (afectan toda la escena)
- Flechas: mover X/Y
- W/S: mover Z
- Q/E: rotar Y | R/T: rotar X | Y/U: rotar Z
- +/-: escalar

## UI - Pestaña Lienzo
- Selector de cara: Cubo/Cuboide (6), Pirámide (5), Cilindro (6 = tapa, base, 4 lados), Cono (5 = base, 4 lados); Esfera, Cápsula y Plano tienen 1 cara pintable
- Lienzo de pintura con coordenadas del píxel bajo el mouse
- Preview de pincel (círculo guía)
- Pintado con clic izquierdo, pincel variable (0-8)
- Gotero: clic derecho en un píxel para tomar su color
- Paleta de 8 colores + selector personalizado + colores guardados
- Botones ± para tamaño de pincel
- Botón "Limpiar cara" (rellena la cara con el color de relleno) y "Llenar todo" (rellena TODAS las caras con el color seleccionado), ambos deshabilitados si la forma está bloqueada

## UI - Pestaña Forma
Tiene 2 subpestañas (selectable_labels con `self.subtab`: 0 Básico, 1 Agrupaciones)

### Subpestaña Básico
- Lista de todas las formas con nombre y tipo
- Botón X para eliminar forma
- Botón + en la parte inferior derecha (menú contextual) para añadir: Cubo, Cuboide, Pirámide, Esfera, Cilindro, Cono, Cápsula, Plano
- Selección de forma activa (clic en el nombre)
- Controles de posición: X/Y/Z con botones ± (paso 0.1, mantén presionado para repetir) + DragValue editable con teclado
- Controles de rotación: X/Y/Z en grados con botones ± (paso 1°, mantén presionado para repetir) + DragValue editable con teclado
- Parámetros específicos según tipo (todos con botones ± + DragValue): Escala (cubo), Ancho/Alto/Profundo (cuboide), Escala (pirámide), Radio/Segmentos (esfera), Radio/Alto/Segmentos (cilindro), Radio base/Alto/Segmentos (cono), Ancho/Alto/Segmentos (cápsula), Ancho/Alto (plano)
- Selector de material (Plástico/Metal/Mate/Espejo)
- Slider de Transparencia (0 opaco → 1 transparente); se aplica a cualquier material
- Botones Bloquear/Ocultar por forma (`FormaData.bloqueada/oculta`). Bloqueada: la edición (nombre/posición/rotación/parámetros/material/transparencia), el pintado y Limpiar/Llenar todo se deshabilitan; se muestra aviso. Ocultas: se sacan del render (`set_visible(!oculta)` por frame sobre el nodo de la forma; sin rebuild, los índices de `shape_groups` no cambian)

### Subpestaña Agrupaciones
- `GrupoData { nombre, posicion, rotacion(grados→rad), escala }`; `FormaData.grupo: Option<usize>` (índice en `SharedState.grupos`) y `SharedState.grupos: Vec<GrupoData>`
- Crear grupo (`crear_grupo`), borrar grupo (`borrar_grupo` reindexa y desagrupa miembros), renombrar
- Añadir/quitar formas a/de un grupo (`asignar_grupo(forma_idx, Option<usize>)`)
- Editar posición/rotación/escala del grupo (se aplica a todas sus formas, que se mueven/rotan/escalan juntas)
- Las subpestañas se renderizan en `ui_forma_tab`; el contenido Básico es `ui_forma` y el de grupos `ui_grupos`

## UI - Pestaña Avanzado
- Selector de resolución: 8×8, 16×16, 32×32
- Editor de colores de la paleta (8 colores editables)
- Selector de color de relleno (usado por "Limpiar cara")
- Scroll vertical en todas las pestañas

## UI - Pestaña Proyecto
- Guardar y cargar proyectos como un solo `.glb` en `~/Desktop/modelador_proyectos/<nombre>.glb`
- Nombre de proyecto editable (se sanitiza para el nombre de archivo)
- "Guardar cambios" sobrescribe el mismo `.glb`
- Autosave: si hay un proyecto ya guardado (`project_path`), EL autoguardado está activo (`autosave_activo`, por defecto desactivado, con toggle ON/OFF en la pestaña Proyecto) y `state.dirty` hace un cambio, se exporta automáticamente tras un debounce de 1.5s (se detecta el flanco de `dirty` con `prev_dirty`; campos `autosave_pend`/`autosave_start`/`prev_dirty` en `UiApp`)
- Lista de proyectos guardados (solo archivos `.glb`)
- Los proyectos antiguos (carpeta JSON+PNG+OBJ+MTL) ya no se listan ni se cargan

## Atajos
- Teclas 1-8: seleccionar color de paleta al instante

## Tooltips
- **Actualmente desactivados/eliminados** (se reañadirán después): no hay `on_hover_text` ni configuración de `tooltip_delay`
- Si se reañaden: usar `Response::on_hover_text` (posiciona relativo al widget; para widgets grandes usar `on_hover_text_at_pointer` para que aparezca bajo el cursor), configurar `ctx.style_mut(|s| s.interaction.tooltip_delay = 1.0)` y `show_tooltips_only_when_still = false` (el default true cancela los globos con micro-movimientos del ratón), y añadir claves `_tip` en `Texts`/`TEXTS` (es/en/fr)
- El lienzo 3D (ventana kiss3d separada) NO puede llevar tooltips egui (es otra ventana nativa, sin hover de widgets)

## Visualización 3D
- La forma activa se dibuja con bordes cian `[0,1,1]`
- La cara seleccionada de la forma activa tiene bordes blancos `[1,1,1]`
- Las formas inactivas tienen bordes negros `[0,0,0]`
- Cada forma tiene su propio grupo en el grafo de escena con traslación individual
- Al cambiar parámetros, las mallas se reconstruyen (old nodes se desvinculan con `unlink()`)

## Materiales
- `MaterialTipo` enum: Plastico, Metal, Mate, Espejo (campo `material` en `FormaData`; por defecto `Plastico`)
- Selector en la pestaña Forma (selectable_labels); al cambiar se setea `shape_dirty` para reconstruir (re-asigna material)
- Reflectividad por material en el GLB: `MaterialTipo::factor_pgr() -> (metallic, roughness)`: plástico (0.0,0.7), metal (1.0,0.4), mate (0.0,1.0), espejo (1.0,0.05) → escrito en `pbrMetallicRoughness`
- Viewport: la función `material_pbr(tipo)` crea un `Matpbr` (implementa `kiss3d::resource::Material`), una copia del render de `ObjectMaterial` pero con un fragment shader custom. Cada node recibe su material vía `node.set_material(mat.clone())` en `rebuild_shapes()`
- El shader custom (`PBR_VERTEX_SRC`/`PBR_FRAGMENT_SRC` embebidos como string const) añade 3 uniforms: `u_spec` (cobertura especular), `u_shine` (brillo/exponente) y `u_mirror` (fuerza de un reflejo "falso de cielo" vía fresnel ~pow(1-ndv,3) muestreando un gradiente de cielo en `normal.y`). La textura se modula y se mezcla con ese cielo
- Parámetros por material: plástico (0.35, 24, 0.08), metal (0.42, 90, 0.5), mate (0.04, 2, 0.0), espejo (0.35, 300, 0.85). Orden de reflectividad: mate < plástico < metal < espejo (espejo con el fresnel más fuerte y brillo más agudo)
- El material custom reusa los `verify!`/`ignore!` de kiss3d (`#[macro_export]`) y los mismos atributos instanced (inst_tra/inst_color/inst_def_*) que `ObjectMaterial`; dibuja superficie y líneas (bordes de selección)
- Transparencia: `FormaData.transparencia: f32` (0 opaco a 1). El shader tiene uniform `u_alpha`; si alpha<0.999 se activa blending (`enable(BLEND)` + `blend_func_separate(SRC_ALPHA, ONE_MINUS_SRC_ALPHA,...)`) solo durante la superficie y se apaga (con `u_alpha=1`) al pintar las líneas, para que los bordes de forma/quidades se vean nítidos. `material_pbr(tipo, transparencia)` y la llamada en `rebuild_shapes` pasan `forma.transparencia`; el valor en pantalla se clampa a `alpha>=0.15` para no perder la forma
- Export GLB: si `transparencia>0` se añade `baseColorFactor:[1,1,1,1-transp]` y `alphaMode:"BLEND"` al material

## Arquitectura
- `FormaTipo` enum: Cubo, Cuboide, PirCuadrada, Esfera, Cilindro, Cono, Capsula, Plano
- `FormaData` struct: nombre, tipo, posición [x,y,z], parámetros por tipo (`segmentos` compartido por esfera/cilindro/cono/cápsula, `esfera_radio`, `cilindro_radio/alto`, `cono_radio/alto`, `capsula_ancho/alto`, `plano_ancho/alto`), vértices/derivados, pixeles por cara, cara_sel, `material`
- `SharedState` con `formas: Vec<FormaData>`, `forma_activa: usize`, y `shape_dirty: bool`, `grupos: Vec<GrupoData>`
- Grupos: `FormaData.grupo: Option<usize>`; en el grafo de escena cada grupo es un nodo (root→grupo→forma). `rebuild_shapes` devuelve `(Vec<ShapeGroup>, Vec<SceneNode>)` (formas + nodos de grupo). Por frame se actualizan los transforms de las formas y de los grupos (posición/rotación/escala) desde el estado
- `Arc<Mutex<>>` compartido entre hilos
- Hilo secundario: kiss3d (render 3D con SceneNode dinámicos, función `rebuild_shapes()`)
- Hilo principal: eframe (UI de pintura)
- Las texturas 3D se voltean verticalmente al subirse a GPU (`flip_vertical_in_place`) para corregir coordenadas Y (UI top-left → OpenGL bottom-left)
- Al cambiar resolución se redimensionan los píxeles
- Exportación GLB: cada forma = un nodo (translation + rotation, cuaternión = `from_euler_angles` de nalgebra); cubo/cuboide/pirámide = un primitive por cara con su material/textura; esfera/cápsula/plano = un primitive por malla; cilindro y cono = un primitive por cara (tapa/base/lados) con su textura; el plano usa `doubleSided: true`
- Mallas por procedimiento (`MallaDatos` con pos/uvs/normals/indices): `malla_esfera`, `malla_capsula`, `malla_plano`, `mallas_cilindro` (6 caras), `mallas_cono` (5 caras); los polos de esfera y cápsula usan abanicos de triángulos (sin triángulos degenerados); las normales laterales del cono son `[alto/slant, radio/slant]` rotadas; los triángulos quedan con winding hacia afuera (CCW desde fuera) para el backface culling de kiss3d (`push_cuad` usa la diagonal `[a,c,b,c,d,b]`)
- Orientación de UV vertical consistente: v=0 abajo, v=1 arriba (cilindro/cono/plano/esfera/cápsula); las UV de las tapas/base usan proyección circular `[0.5+0.5cos, 0.5+0.5sin]`
- La sky dome usa `malla_esfera(50,32)` con normales negadas (hacia adentro) para verse iluminada desde dentro
- `Mesh::new` de kiss3d recibe normales como `na::Vector3<f32>`; `mesh_desde_datos` construye el mesh (también usado por la sky dome)
- Texturas embebidas como PNG en el chunk BIN; las imágenes NO se voltean (convención glTF: v=0 arriba)
- El estado editable completo (`ProjectData` con `formas`/pixeles) viaja en `extras` del GLB para poder reabrir y editar
- Formato GLB v2: header 12B + chunk JSON (padding espacios) + chunk BIN (padding ceros); índices u32, componentes f32; el length total del header debe ser `as u32`
- Test `roundtrip_glb` verifica exportar→importar y validez del header; `mallas_validas` verifica índices, normales unitarias y sin triángulos degenerados

## Dependencias
kiss3d = "0.36", image = "0.24", egui = "0.28", eframe = "0.28", serde, serde_json
