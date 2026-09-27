# Notas de mejoras

Lo que fui encontrando al probar hook a fondo. Cada punto dice qué problema resuelve y por qué suma al
trabajo con agentes. Están ordenadas por valor, de mayor a menor.

## 1. El agente ve el estado de tu tanda en vivo

Hoy `hook_wait_feedback` espera a que dejes de escribir y entrega todo junto. Se podría entregar la
**primera marca apenas la guardás** y seguir mandando las siguientes como mensajes nuevos (el MCP
soporta notificaciones de progreso). Así el agente empieza a leer código mientras vos seguís marcando, y
el tiempo total baja mucho en tandas largas.

## 2. Memories más precisas: estados y movimiento

La memory guarda una imagen. Para "hacelo como esto" suele faltar:
- **el hover y el estado activo:** guardar 2 o 3 capturas con un atajo mientras pasás el mouse;
- **un clip corto** (2 a 3 s) cuando lo que gusta es una animación;
- **radios y espaciados estimados** desde la imagen. La paleta de colores dominantes ya se calcula al
  guardar la memory y le llega al agente.

## 3. Comparar el resultado contra la memory

Después de que el agente aplica un cambio "como @tarjeta-linear", hook podría capturar la zona nueva y
mostrarla lado a lado con la memory, con una marca de "no se parece acá" que vuelve al agente. Cerrar ese
ciclo es lo que hace precisa la reproducción.

## 4. Marcas que siguen al contenido

Si scrolleás o movés la ventana después de marcar, el pin queda flotando en el lugar viejo. Anclar cada
marca a la ventana (y, en la web, al elemento) evita malentendidos con el agente y hace que el hilo siga
teniendo sentido minutos después.

## 5. Voz

Hecho: dictado de comentarios con `V` (cpal + whisper.cpp local, modelo `small` cuantizado). Queda:
transcripción parcial mientras hablás (hoy transcribe al cortar), usar la GPU (Metal/Vulkan) para bajar
los ~2-3 s de espera, y que la píldora reaccione a la voz con el dock cerrado.

**Benchmark (27/09/2026).** 8 frases de dictado reales (128 palabras, voz rioplatense a ritmo normal:
voseo, nombres propios, términos en inglés), CPU de 12 núcleos sin GPU, clips de 10 s:

| Configuración | Errores por palabra | Espera |
|---|---|---|
| `small`, sin contexto | 31% | 3,6 s |
| `small`, contexto con voseo | 27% | 4,0 s |
| `small`, contexto + `audio_ctx` a la duración (el defecto hoy) | 26% | 1,5 s |
| `large-v3-turbo`, contexto | 11% | 18,5 s |
| `large-v3-turbo`, contexto + `audio_ctx` a la duración | 14% | 5,9 s |

- El modelo pesa más que el contexto: `turbo` erra la mitad. El contexto arregla nombres ("Claude",
  "header") y el voseo ("interpretá", "podés").
- Recortar el silencio de los bordes empeoró todo (cortaba palabras en grabaciones con picos): descartado.
- Queda: probar `turbo` con Vulkan en la RX 580 (debería bajar a 1-2 s y volverse el defecto) y rehacer
  la tabla con cada cambio. Las grabaciones y el script están fuera del repo, en
  `~/.local/share/hook/bench/` (es la voz del usuario).

**Servidor de voz.** `voice_server` manda el audio a una URL compatible con la API de transcripción de
OpenAI o con `whisper-server` de whisper.cpp. Queda levantar ese servidor con GPU y medir `large-v3` y otros
motores (por ejemplo Canary) por calidad pura, sumando la latencia de red. Si lo usa gente de afuera,
necesita autenticación y una revisión de seguridad antes de exponerlo.

## 6. Contexto automático de la app marcada

Junto con la captura, mandar al agente el **nombre de la app y el título de la ventana** (y la URL si es un
navegador). Con eso el agente sabe qué proyecto y qué ruta tocar sin preguntar.

## Encontrado al probar

- **La suite de interfaz no corre en un X anidado** (Xephyr no tiene DRI3 y wgpu no puede presentar), así
  que toma el mouse del escritorio real. Convendría un modo de prueba con render fuera de pantalla e
  inyección de eventos directa al `Board`, sin X, para correrla en CI.
- **La prueba de estrés depende del playground "Café Orbital" abierto.** Podría abrirlo sola.

## Límites de la captura inteligente

- Los elementos sin borde ni cambio de fondo (por ejemplo, texto suelto sobre el mismo fondo) solo se
  seleccionan a nivel de su contenedor. Se podría sumar la accesibilidad del sistema (AT-SPI en Linux) para
  leer los límites reales de cada elemento cuando la app los expone.
- Las esquinas muy redondeadas pueden correr un lado 1 o 2 px.
