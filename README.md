# Sistema solar explorable - Software Renderer

Arturo Lima - 24683 

El siguiente proyecto tiene como objetivo demostrar un sistema solar y poder viajar entre diferentes planetas; dejando la mayoria de responsabilidades de ejecucion a la GPU local y evitar depender de librerias externas, siendo Raylib la unica excepcion. 

## Evidencia de funcionamietno

### Enlace al video del funcionamiento del programa: 

### Fotografias del programa: 
- Normal: 
<img width="1756" height="1106" alt="image" src="https://github.com/user-attachments/assets/381c5d40-46fb-4a3c-9b2c-e83412c564bc" />

- Carmesi:
  <img width="1721" height="1095" alt="image" src="https://github.com/user-attachments/assets/0c6deff1-219e-4397-8de7-bb71e79448dc" />

- Clasico: 
<img width="1681" height="1107" alt="image" src="https://github.com/user-attachments/assets/acf3854e-a2bd-4a6d-991c-c37fc6b0b515" />

## Cómo ejecutarlo

Se necesita de la instalacion de Rust con `cargo` y Raylib, preferiblemente ver - "5.5.1" 

En el caso de Windows es necesario instalar y configurar CMake para el correcto funcionamietno de Raylib.

El programa se ejecuta con el comando: 
```
cargo run --release
```

## Controles

| Tecla / ratón | Acción |
|---|---|
| `W` / `S` (o flechas arriba/abajo) | Subir / bajar la nariz |
| `A` / `D` (o flechas izquierda/derecha) | Girar a la izquierda / derecha |
| `O`| Orientar la nave a la superficie del planeta mas cercano, ayuda en el aterrizaje |
| `Espacio` | Acelerar (al soltar, la nave frena) |
| `1` / `2` / `3` | Potencia del motor: 30 % / 60 % / 100 % |
| `4` / `5` | Viaje rapido a Unicron / Mogo |
| `R` | Reiniciar (la nave hace un viaje rapido a la Tierra) |
| `F` | Mostrar / ocultar el contador de FPS |
| `Esc` | Salir |

## Elementos visisbles en el programa
- Sol
- Nave espacial
- Escombros de naves abandonadas
- 9 planetas estandares 
- 2 planetas personalizados 
- Linea petrova 
- Dashboard / Hud de la nave 
- Constelación Sancor

## Caracteristicas aplicadas de la rubrica de instruccioens dentro del PDF
|Caracteristica | Ubicacion |
|---|---|
| Planeta distinguible que orbitan, rotan y se observan sin que el programa tenga bajones de rednimietno | `src/planeta.rs` |
| Shader propido con efecto visualmente distinguible | `src/planeta.rs`, `src/sol.rs` |
| Shader animado | `src/sol.rs` |
| Etapa propia de generacion/subdivision procedural de geometria | `src/planeta.rs`|
| Modo control local que facilita el aterrizaje de la nave | `Cohete/src/controles.rs` |
| Aceleracion, frenado y giro progresivamente independiente | `Cohete/src/controles.rs` |
| Atraccion gravitacional de los planetas que modifican la trayectoria de la nave | `src/planeta.rs` | 
| Colisiones basicas que impiden atravesar los planetas | `src/aterrizaje.rs` | 
| Descartar objetos fuera de vista antes de renderizarlos | `src/planeta.rs`, `src/main.rs` |
| Usar distintas cantidades de detalle segun la distancia entre la nave y el planeta | `src/planeta.rs`, `src/sol.rs` | 
| Coherencia visual y claridad de la experiencia | `src/hud.rs`, `src/mapa.rs`, `src/sonido.rs` |
| Claridad de la experiencia de los controles | `src/hud.rs`, `src/mapa.rs`, `Cohete/src/Controles.rs` |


## Estructura del código

- `src/main.rs`: bucle principal y composición de la escena.
- `src/pipeline.rs`, `src/triangle.rs`, `src/framebuffer.rs`: pipeline propio (modelo, vista y
  proyección, clipping homogéneo, rasterizado con z-buffer, franjas en paralelo).
- `src/fragment.rs`, `src/luz.rs`, `src/sol.rs`: fragment shaders e iluminación.
- `src/planeta.rs`: cuerpos celestes procedurales, órbitas y niveles de detalle.
- `src/vuelo.rs`, `src/aterrizaje.rs`: física de vuelo, cámara de seguimiento, asistente y colisión.
- `src/hud.rs`, `src/sonido.rs`: interfaz (con mapa estelar cenital del sistema) y audio.
- `src/modelo_planeta.rs`: carga de cuerpos desde `.obj` (Mogo, Unicron) y niveles de detalle.
- `Cohete/`: carga de OBJ y lectura de controles.
- `src/assets/`: modelos (`.obj`) y efectos de sonido (`soundeffects/`).

## Uso de IA y recursos externos

- Se usó un asistente de IA (GitHub Copilot) para  apoyarse en la planeacion y revision del código (planetas, sol, aterrizaje). 
- Librerías: raylib 5.5.1 (crate `raylib`) para ventana, entrada, audio, decodificación de imagen
  y presentación del framebuffer.

