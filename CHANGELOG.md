# Changelog

## v0.2.0

- **Memories:** guardás cualquier parte de la pantalla como referencia (`M` o "Guardar memory") y se la
  pasás al agente desde un buscador con miniaturas (`Ctrl+Alt+M`) o nombrándola en un comentario
  (`@tarjeta-linear`). Tools nuevas: `koon_list_memories` y `koon_get_memory`.
- **Captura inteligente:** con "Marcar área" o "Guardar memory", un click detecta el elemento bajo el
  cursor. La rueda del mouse agranda la selección al elemento que lo contiene, o la achica. No usa ningún
  modelo y tarda unos 10 ms.
- **Comentar seguido:** `Shift+Enter` guarda y deja la herramienta lista para el próximo comentario.
- **El agente recibe el feedback antes:** `koon_wait_feedback` entrega 1,5 s después de tu última marca
  (antes, 3 s) y nunca mientras escribís.
- **Reabrir:** un click en el "✓ …" del agente reabre la marca para contestarle.
- La píldora recuerda dónde la dejaste.
- Sin archivos sueltos en los assets del release.

## v0.1.0

Primera versión de koon: un widget flotante para señalar la pantalla y pasarle el feedback a tu agente
de código por MCP.

- **Píldora flotante** con dock de herramientas que nace de ella: comentar, dibujar y marcar áreas
  sobre la pantalla viva, en cualquier monitor.
- **Hilos de conversación por marca**: el agente puede preguntar (`koon_ask`), responder
  (`koon_reply`) y cerrar con un mensaje (`koon_resolve`).
- **Servidor MCP** (`koon-mcp`) para Claude Code, Codex o cualquier agente compatible.
- **Captura silenciosa** del escritorio en GNOME (Mutter ScreenCast), sin diálogos ni sonido.
- Las marcas abiertas se recargan al iniciar koon.
- Motor de UI propio y liviano (`koon-ui`); binario de ~8 MB.

Probado en Linux (GNOME sobre Wayland, dos monitores). El binario de Linux requiere Ubuntu 24.04 o equivalente (PipeWire 1.0+). Los binarios de Windows y macOS se publican
como vista previa: la captura y el recorte de input todavía son específicos de Linux.
