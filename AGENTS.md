# Modelador 3D - Proyecto

App en Rust para modelar y pintar assets 3D simples.

## Estado actual
- Cubo formado por 6 caras independientes (quads)
- Cada cara tiene textura pintable con resolución variable (8×8, 16×16, 32×32)
- Ventana 3D (kiss3d) con cámara orbital
- Ventana UI (eframe/egui) con dos pestañas: Lienzo y Avanzado
- Exportación a OBJ + PNG

## Controles 3D
- Flechas: mover X/Y
- W/S: mover Z
- Q/E: rotar Y | R/T: rotar X | Y/U: rotar Z
- +/-: escalar

## UI - Pestaña Lienzo
- Selector de cara (6 caras)
- Lienzo de pintura con coordenadas del píxel bajo el mouse
- Preview de pincel (círculo guía)
- Pintado con clic izquierdo, pincel variable (0-8)
- Gotero: clic derecho en un píxel para tomar su color
- Paleta de 8 colores + selector personalizado + colores guardados
- Slider de tamaño de pincel
- Botón "Limpiar cara" (rellena la cara con el color de relleno)
- Botón "Exportar OBJ"

## UI - Pestaña Avanzado
- Selector de resolución: 8×8, 16×16, 32×32
- Editor de colores de la paleta (8 colores editables)
- Selector de color de relleno (usado por "Limpiar cara")
- Scroll vertical en ambas pestañas

## Atajos
- Teclas 1-8: seleccionar color de paleta al instante

## Arquitectura
- `SharedState` con Arc<Mutex<>> compartido entre hilos
- Hilo secundario: kiss3d (render 3D)
- Hilo principal: eframe (UI de pintura)
- Las texturas 3D se voltean horizontalmente al subirse a GPU para coincidir con el lienzo
- Al cambiar resolución se hace tile del contenido actual

## Dependencias
kiss3d = "0.36", image = "0.24", egui = "0.28", eframe = "0.28"

## Próximos pasos potenciales
- Más formas geométricas
- Pinceles, texturas desde archivo
- Más atajos de teclado
