# Modelador 3D - Proyecto

App en Rust para modelar y pintar assets 3D simples.

## Estado actual
- 4 tipos de forma: Cubo, Cuboide, Pirámide Cuadrada, Esfera (Pirámide Triangular eliminada)
- Múltiples formas independientes en la escena, cada una con su propio nombre, posición y texturas
- Cada forma tiene caras independientes con textura pintable
- Resolución de textura variable (8×8, 16×16, 32×32)
- Ventana 3D (kiss3d) con cámara orbital
- Ventana UI (eframe/egui) con 4 pestañas: Lienzo, Forma, Avanzado, Proyecto
- Exportación a OBJ + PNG (todas las formas como objetos separados)
- Guardado/carga de proyectos en JSON

## Controles 3D (afectan toda la escena)
- Flechas: mover X/Y
- W/S: mover Z
- Q/E: rotar Y | R/T: rotar X | Y/U: rotar Z
- +/-: escalar

## UI - Pestaña Lienzo
- Selector de cara (cantidad variable según forma y tipo)
- Lienzo de pintura con coordenadas del píxel bajo el mouse
- Preview de pincel (círculo guía)
- Pintado con clic izquierdo, pincel variable (0-8)
- Gotero: clic derecho en un píxel para tomar su color
- Paleta de 8 colores + selector personalizado + colores guardados
- Botones ± para tamaño de pincel
- Botón "Limpiar cara" (rellena la cara con el color de relleno)
- Botón "Exportar OBJ"

## UI - Pestaña Forma
- Lista de todas las formas con nombre y tipo
- Botón X para eliminar forma
- Botón + en la parte inferior derecha (menú contextual) para añadir: Cubo, Cuboide, Pirámide, Esfera
- Selección de forma activa (clic en el nombre)
- Controles de posición: X/Y/Z con botones ± (paso 0.1, mantén presionado para repetir) + DragValue editable con teclado
- Controles de rotación: X/Y/Z en grados con botones ± (paso 1°, mantén presionado para repetir) + DragValue editable con teclado
- Parámetros específicos según tipo: Escala (cubo), Ancho/Alto/Profundo (cuboide), Radio/Segmentos (esfera), Escala (pirámide) — todos con botones ± (con repetición al mantener) + DragValue editable

## UI - Pestaña Avanzado
- Selector de resolución: 8×8, 16×16, 32×32
- Editor de colores de la paleta (8 colores editables)
- Selector de color de relleno (usado por "Limpiar cara")
- Scroll vertical en todas las pestañas

## UI - Pestaña Proyecto
- Guardar y cargar proyectos (JSON)
- Nombre de proyecto editable
- Lista de proyectos guardados

## Atajos
- Teclas 1-8: seleccionar color de paleta al instante

## Visualización 3D
- La forma activa se dibuja con bordes cian `[0,1,1]`
- La cara seleccionada de la forma activa tiene bordes blancos `[1,1,1]`
- Las formas inactivas tienen bordes negros `[0,0,0]`
- Cada forma tiene su propio grupo en el grafo de escena con traslación individual
- Al cambiar parámetros, las mallas se reconstruyen (old nodes se desvinculan con `unlink()`)

## Arquitectura
- `FormaTipo` enum: Cubo, Cuboide, PirCuadrada, Esfera
- `FormaData` struct: nombre, tipo, posición [x,y,z], parámetros por tipo, vértices/derivados, pixeles por cara, cara_sel
- `SharedState` con `formas: Vec<FormaData>`, `forma_activa: usize`, y `shape_dirty: bool`
- `Arc<Mutex<>>` compartido entre hilos
- Hilo secundario: kiss3d (render 3D con SceneNode dinámicos, función `rebuild_shapes()`)
- Hilo principal: eframe (UI de pintura)
- Las texturas 3D se voltean verticalmente al subirse a GPU (`flip_vertical_in_place`) para corregir coordenadas Y (UI top-left → OpenGL bottom-left)
- Al cambiar resolución se redimensionan los píxeles
- Exportación OBJ escribe cada forma como objeto separado con desplazamiento de posición
- Serialización con serde para guardar/cargar proyectos (toda la `Vec<FormaData>`)

## Dependencias
kiss3d = "0.37", image = "0.24", egui = "0.28", eframe = "0.28", serde, serde_json
