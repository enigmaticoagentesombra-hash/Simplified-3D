# Modelador 3D - Proyecto

App en Rust para modelar y pintar assets 3D simples.

## Estado actual
- 5 formas geométricas: Cubo, Cuboide, Pirámide Triangular, Pirámide Cuadrada, Esfera
- Cubo y Cuboide con parámetros de escala/dimensiones
- Cada forma tiene caras independientes con textura pintable
- Resolución de textura variable (8×8, 16×16, 32×32)
- Ventana 3D (kiss3d) con cámara orbital
- Ventana UI (eframe/egui) con 4 pestañas: Lienzo, Forma, Avanzado, Proyecto
- Exportación a OBJ + PNG
- Guardado/carga de proyectos en JSON

## Controles 3D
- Flechas: mover X/Y
- W/S: mover Z
- Q/E: rotar Y | R/T: rotar X | Y/U: rotar Z
- +/-: escalar

## UI - Pestaña Lienzo
- Selector de cara (cantidad variable según forma)
- Lienzo de pintura con coordenadas del píxel bajo el mouse
- Preview de pincel (círculo guía)
- Pintado con clic izquierdo, pincel variable (0-8)
- Gotero: clic derecho en un píxel para tomar su color
- Paleta de 8 colores + selector personalizado + colores guardados
- Slider de tamaño de pincel
- Botón "Limpiar cara" (rellena la cara con el color de relleno)
- Botón "Exportar OBJ"

## UI - Pestaña Forma
- Selector de forma: Cubo, Cuboide, Pir. Triáng., Pir. Cuadrada, Esfera
- Parámetros específicos: Escala (cubo), Ancho/Alto/Profundo (cuboide), Radio/Segmentos (esfera)
- Editor de vértices arrastrable (para formas no-esfera)

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

## Arquitectura
- `SharedState` con Arc<Mutex<>> compartido entre hilos
- Hilo secundario: kiss3d (render 3D con SceneNode dinámicos)
- Hilo principal: eframe (UI de pintura)
- Las texturas 3D se voltean horizontalmente al subirse a GPU
- Al cambiar resolución se redimensionan los píxeles
- Cada forma tiene vértices y caras definidos en `shape_vertices` / `shape_faces`
- El render 3D construye mallas por cara usando `add_mesh`
- Serialización con serde para guardar proyectos

## Dependencias
kiss3d = "0.36", image = "0.24", egui = "0.28", eframe = "0.28", serde, serde_json
