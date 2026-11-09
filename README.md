# Sistema solar explorable - Software Renderer

Proyecto final de Gráficas por Computadora (2026). Un sistema solar que se explora desde una nave,
renderizado completamente en CPU con un pipeline propio escrito en Rust. Raylib solo se usa para la
ventana, la lectura de entradas, el audio, la decodificación del PNG del mapa y para mostrar el
framebuffer; no se usa ninguna de sus funciones de dibujo 3D, cámara, modelos, materiales, shaders
de GPU ni física.

## Cómo ejecutarlo

Requisitos: Rust (edición 2021) con `cargo`. La primera compilación construye raylib desde código
fuente, así que necesita CMake y un compilador de C/C++ (y libclang para los bindings).

```
cargo run --release
```

Usa siempre `--release` para jugar: el rasterizador por software es mucho más lento en debug.

## Controles

| Tecla / ratón | Acción |
|---|---|
| `W` / `S` (o flechas arriba/abajo) | Subir / bajar la nariz |
| `A` / `D` (o flechas izquierda/derecha) | Girar a la izquierda / derecha |
| Botón izquierdo del ratón + arrastrar | Orientar la nave |
| `Espacio` | Acelerar (al soltar, la nave frena) |
| `1` / `2` / `3` | Potencia del motor: 30 % / 60 % / 100 % |
| `4` / `5` | Colocar la nave cerca de Unicron / Mogo |
| Rueda del ratón, `+`, `-` / `Q` | Distancia de la cámara |
| `R` | Reiniciar (la nave vuelve a aparecer cerca de Terra) |
| `Esc` | Salir |

Con control de juego: palanca izquierda para orientar, botón inferior para acelerar y gatillos
superiores para la cámara.

**Aterrizaje.** Apunta a cualquier cuerpo y avanza: cerca de él un asistente limita la velocidad
según la altura para llegar al suelo a unos 400 km/h. Al tocar el suelo por debajo de 800 km/h la
nave se posa; entonces `A`/`D` la giran sobre el terreno y `Espacio` despega. El sol no se puede
atravesar ni aterrizar en él.

## Qué hay en la escena

- **Sol** en el origen: esfera emisora de luz, con shader propio animado (ver más abajo). Todos los
  cuerpos y la nave se iluminan desde su posición, con atenuación según la distancia.
- **Nave** modelada por nosotros (`src/assets/Intento1Lab3.obj`), con propulsor de fuego, láser a
  alta velocidad y sonidos sincronizados.
- **Diez cuerpos celestes** que orbitan el sol y rotan sobre su eje en función del tiempo
  transcurrido: Mercurio, Venus, Terra, Luna (orbita a Terra), Marte, Júpiter, Saturno (con
  anillos), Urano, Neptuno y Plutón. Mercurio y Plutón son pequeños a propósito y la Luna respeta
  la proporción real de tamaño con Terra. Distancias y tamaños de los gigantes están comprimidos
  para poder recorrer el sistema.
- **HUD** estilo automóvil con instrumentos de aviación: velocímetro con barra segmentada, lista
  de cuerpos cercanos, cronómetro y odómetro, mapa orbital en la esquina superior derecha,
  horizonte artificial y radar de altitud.

## Características del menú que solicitamos evaluar y dónde verlas

| Característica | Dónde verla | Archivo |
|---|---|---|
| Planetas distinguibles que orbitan, rotan y se observan de cerca (10 cuerpos) | Volar hacia cualquiera; cada uno tiene color, relieve y rasgos propios | `src/planeta.rs` (`SistemaPlanetario::nuevo`) |
| Shader propio con efecto distinguible: el sol | Superficie de plasma con manchas solares y oscurecimiento del borde | `src/sol.rs` (`color_estrella`) |
| Shader propio con efecto distinguible: iluminación desde el sol | Lado diurno y nocturno de planetas y nave, que cambia al orbitar | `src/fragment.rs`, `src/luz.rs` |
| Shader animado | Plasma del sol que deriva y late con el tiempo | `src/sol.rs` |
| Deformación de la geometría de un planeta mediante ruido | Montañas y mares de Terra, relieve de la Luna, Marte, Mercurio y Plutón | `src/planeta.rs` (`fbm`, `radio_en`) |
| Etapa propia de generación/subdivisión procedural de geometría antes de rasterizar | Al iniciar: icosaedro subdividido recursivamente en esferas | `src/planeta.rs` (`icosaedro`, `subdividir`, `construir_malla`) |
| Aceleración, frenado y giro progresivos independientes de la velocidad del equipo | Acelerar y soltar `Espacio`; la integración usa el tiempo transcurrido | `src/vuelo.rs` (`integrate_speed`) |
| Colisión básica que impide atravesar los cuerpos | Volar a máxima velocidad contra un planeta, la Luna o el sol | `src/aterrizaje.rs` |
| Modo de control local cerca de un cuerpo | Asistente de descenso, arrastre con el movimiento del cuerpo y aterrizaje | `src/aterrizaje.rs` |
| Descartar objetos fuera de la vista antes de rasterizar | Cuerpos fuera del frustum no se procesan; caras traseras descartadas | `src/planeta.rs` (`proyeccion_esfera`), `src/main.rs` (`agregar_malla`) |
| Niveles de detalle según la distancia | Los cuerpos lejanos usan mallas con menos triángulos | `src/planeta.rs` (`malla_lod`), `src/sol.rs` |
| Coherencia visual y claridad de la experiencia | HUD, mapa, sonidos y controles descritos arriba | `src/hud.rs`, `src/mapa.rs`, `src/sonido.rs` |

Pendiente de la rúbrica (no implementado): modelos propios colocados sobre un planeta y su
distribución procedural, mapeo de normales, atracción gravitatoria y los contadores de triángulos
procesados que piden el descarte y los niveles de detalle para demostrarse.

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

- Se usó un asistente de IA (GitHub Copilot) como apoyo para escribir y revisar parte del código
  (planetas, sol, aterrizaje, HUD, sonido). Los integrantes deben poder explicar el código entregado.
- Librerías: raylib 5.5.1 (crate `raylib`) para ventana, entrada, audio, decodificación de imagen
  y presentación del framebuffer.
- Recursos externos: COMPLETAR el origen y la licencia de los
  efectos de sonido de `src/assets/soundeffects/` y del modelo `SancorSeguros1.obj`.
