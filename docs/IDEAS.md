# Ideas para hook

Ideas que queremos explorar. Se van refinando a medida que iteramos; cada una lleva el problema que
resuelve, cómo imaginamos la experiencia y qué hace falta para que sea precisa.

## Memories: guardar inspiración y pedir "algo así"

**La idea.** Navegás cualquier web, ves algo que te gusta (un botón, una tarjeta, una animación, una
paleta, un layout) y con hook lo guardás como una *memory*. Más tarde, trabajando en tu proyecto, le
pedís al agente "hacé este header como la memory *nav-vercel*" o marcás un elemento propio y decís
"que se parezca a *tarjeta-linear*". El agente recibe la memory completa y la reproduce con mucha
precisión, adaptada a tu código y a tu diseño.

**Experiencia.**
1. Con hook abierto, elegís **Guardar memory** en el dock y señalás el elemento (o marcás un área).
2. hook muestra qué va a guardar (el elemento resaltado) y le ponés un nombre y, si querés, una nota:
   "me gusta cómo rebota al hover".
3. La memory queda en tu biblioteca: un panel con miniaturas, nombre, origen y etiquetas.
4. Al pedir un cambio, la mencionás por nombre o la arrastrás sobre una marca. El agente la usa como
   referencia exacta.

**Qué guardar para que sea preciso** (no alcanza con una captura):
- **Imagen**: recorte del elemento en alta resolución, y en estados clave (reposo, hover, activo) si es
  interactivo.
- **Estructura**: el subárbol del DOM, limpio (sin scripts ni atributos de tracking).
- **Estilos computados** de cada nodo: tipografía (familia, pesos, tamaños, interlineado, tracking),
  colores, bordes, radios, sombras, espaciados, layout (flex/grid, gaps, alineaciones), tamaños reales.
- **Movimiento**: transiciones, keyframes y curvas de easing; opcionalmente un video corto del hover o
  la animación.
- **Recursos**: fuentes, íconos (SVG inline), imágenes de fondo y degradados.
- **Contexto**: URL, título, fecha, tamaño de viewport y modo claro u oscuro.
- **Tokens derivados**: una lectura resumida (paleta, escala tipográfica, radios, sombras) para que el
  agente pueda adaptarla a los tokens de tu proyecto en vez de copiar valores sueltos.

**Cómo encaja con lo que ya hay.**
- La extracción del subárbol, los estilos y los recursos tiene que resolverse sin extensión de navegador
  (por ejemplo, con la accesibilidad del sistema o con la imagen). Es una pregunta abierta.
- El **almacenamiento** de hook (`~/.local/share/hook/`) suma una carpeta `memories/` con un JSON por
  memory, más sus imágenes y recursos, que funciona como caché local de la copia en la nube.
- El **MCP** suma tools como `hook_list_memories`, `hook_get_memory(nombre)` y, del lado de hook, una
  forma de referenciar memories desde un comentario (`@nav-vercel`).

**Precisión: cómo la vamos a medir.** El agente reproduce la memory y hook compara el resultado contra
el original: capturas lado a lado y diferencias en estilos computados clave (colores, tamaños,
espaciados, tipografía). Iteramos hasta que la diferencia sea mínima.

**Nube.** Las memories se sincronizan en la nube con la cuenta de cada persona, para tenerlas en
cualquier máquina y en cualquier agente. Son privadas por defecto: solo las ve su dueño, salvo que
decida compartirlas.

**Uso.** Las memories son inspiración. Como con cualquier servicio de almacenamiento (Google Drive,
Dropbox), qué guarda cada persona y cómo lo usa (derechos de autor, licencias de fuentes o imágenes) es
responsabilidad de quien usa hook.

## Tomar ideas por voz

**La idea.** Que anotar una idea sea tan rápido como decirla. Mantenés un atajo (o tocás la píldora),
hablás ("idea: que hook guarde memories de otras webs…") y soltás. hook transcribe localmente, limpia
el texto y lo guarda como idea, sin que dejes lo que estás haciendo.

**Experiencia.**
- Las barras verdes de la píldora reaccionan a tu voz mientras hablás, y al soltar aparece un
  "✓ Idea guardada" corto.
- Si mientras hablás señalás algo en pantalla, la idea queda asociada a eso (una captura o un elemento),
  igual que una marca.
- Las ideas se ordenan solas: el agente las agrupa por tema, detecta duplicadas y las suma a
  `docs/IDEAS.md` (o a una biblioteca de ideas en hook), con fecha y el audio original por si hace falta
  volver a escucharlo.
- Después podés pedirle al agente "desarrollá la idea de memories" y parte del texto ya ordenado.

**Qué hace falta.** Micrófono con `cpal`, transcripción local (whisper), un atajo de "mantener para
hablar" y una tool del MCP para que el agente lea y ordene las ideas capturadas.

## Controlar hook con la voz

**La idea.** Manejar hook hablando, sin tocar el mouse:
- "Mové la píldora a la izquierda de la pantalla", "ponela abajo en el otro monitor".
- "Ocultala, necesito ver", "mostrala", "cerrá hook".
- "Dejá un comentario sobre el botón de enviar: que sea más grande", "marcá el área del menú".
- "Leeme lo que respondió el agente", "resolvé la marca 2", "borrá todas las marcas".

**Ocultarse con gracia.** Cuando le pedís que se oculte, la píldora no desaparece de golpe: se vuelve
translúcida estilo *glass* (desenfoque y brillo sutil del fondo), se achica y se desvanece con una
animación suave. Al volver, hace el camino inverso.

**Cómo imaginamos que funciona.** La voz se transcribe localmente y un intérprete (reglas simples para
comandos frecuentes, y el agente para lo más libre) la traduce en acciones de hook: mover, ocultar,
abrir el dock, crear o editar marcas. Para "dejá un comentario sobre X", hook ubica X con la
accesibilidad del sistema o con la imagen de la pantalla, muestra dónde lo va a poner y lo confirma
rápido ("¿acá?") antes de guardarlo.

**Qué hace falta.** Lo mismo que la toma de ideas por voz (micrófono, transcripción local, atajo para
hablar), más una capa de comandos con acciones bien definidas y una forma confiable de encontrar
elementos en pantalla a partir de una descripción.

## Varias apps y proyectos a la vez

**El problema.** Es normal trabajar con 3 o 4 apps que no tienen nada que ver entre sí, cada una con su
propio agente. Hoy las marcas son del escritorio: no saben a qué app pertenecen, y cualquier agente
conectado recibe todas.

**Lo que habría que resolver** (por ahora lo dejamos de lado para no sumar complejidad):
- **Asociar cada marca a la ventana que tiene debajo**: app, título, proceso y proyecto.
  - Apps nativas: el proceso de la ventana indica en qué carpeta corre (el repo).
  - Apps web: el puerto de la URL (`localhost:5173`) lleva al proceso que lo sirve y a su carpeta. Es
    agnóstico al stack (Vite, Next, Rails, Go…).
- **Que cada agente reciba solo lo suyo**: el MCP filtra por la carpeta donde corre el agente.
- **Que la marca acompañe a su ventana**: si se mueve, se tapa o se minimiza.
- En GNOME Wayland, ver las ventanas de otras apps requiere la extensión compañera (`gnome/`), que se
  instala una vez y pide cerrar sesión.

**Preguntas abiertas.** ¿Qué pasa con una marca sobre una app sin proyecto conocido? ¿Un agente puede
pedir marcas de otro proyecto? ¿Cómo se ve en hook a qué app pertenece cada marca?

## Anclaje agnóstico de marcas

Para que las marcas no se corran cuando cambia lo que hay debajo, sin depender de ningún framework ni
herramienta de build:
1. **Seguimiento visual**: se guarda un parche de imagen alrededor de la marca y se lo busca en cada
   captura nueva, para mover la marca adonde fue a parar el contenido. Funciona con cualquier app.
2. **Accesibilidad del sistema** para apps nativas: AT-SPI, UI Automation, AX.

Descartado: extensiones de navegador y plugins de herramientas de build. hook tiene que funcionar sin
integrarse con nada.

## Motor compartido entre proyectos

**El problema.** sherpa, hook y lumen dibujan su UI con variantes del mismo motor. Copiar partes
(como hoy hace `hook-ui`) sirve para empezar liviano, pero no escala: cada arreglo hay que repetirlo.

**La idea.** Un motor compartido, modular, del que cada proyecto toma solo lo que usa: módulos o
features para texto, íconos (solo los usados, elegidos en build), widgets y el painter de GPU. Así el
binario de cada app queda chico y los arreglos se hacen una sola vez.

## Otras ideas en el horizonte

- **Voz y señalar**: mantener un atajo, hablar y apuntar con el mouse ("esto más grande… y esto
  sacalo"). hook graba la voz y el recorrido del cursor, transcribe localmente y arma una marca por cada
  cosa señalada.
- **Hilos de conversación por marca**: preguntas y respuestas entre vos y el agente sobre cada marca.
  *(Hecho.)*
- **Wayland puro en GNOME** con una extensión compañera, y soporte para macOS y Windows.
