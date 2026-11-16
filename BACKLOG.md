# Backlog

## Goals

### coordinated-goal

La persona define un goal con su peer, y un coordinator lo construye con implementers y reviewers en
worktrees, lo integra con un merge local y la persona lo ve avanzar en el trabajo en curso. Define
el formato de `work/` en el repo de federación. Segundo, porque es el corazón del modelo operativo y
lo que reemplaza a tmux y al plugin actual.

### verified-changes

Un change que rompe un contrato, un check o su huella declarada queda bloqueado antes del merge, con
la evidencia a la vista. Tercero, porque sin esta garantía la autonomía de los agentes no es
confiable.

### codex-engine

Un goal corre con agentes en Codex usando el plan de ChatGPT de la persona. Cuarto, porque prueba
que la jerarquía y el MCP no dependen de un proveedor.

### team-of-two

Dos personas, cada una con su peer, coordinan un cambio sobre el mismo activo a través de un repo
de federación remoto. Quinto, porque el equipo de la persona se suma pronto.

### release-candidate

Los agentes preparan una versión candidata de un activo y su dueño la promueve con una acción que
dispara el mecanismo de despliegue de la organización.

### forge-proposals

Un PR o un issue de GitHub sobre un activo público entra como propuesta, y la respuesta vuelve por
GitHub.

### adopt-existing

Una organización adopta un repo existente sin modificarlo, y lo que no adoptó queda como activo
externo.

### remote-core

La app se conecta a un runtime que corre en un servidor, con los agentes trabajando ahí.

## Loose entries

### Repositorio

- **knowledge-checks**: los checks de conocimiento que faltan: cada boundary tiene su header, los
  comandos y rutas que nombran `AGENTS.md` y cada `README.md` existen, y cada `Scope:` de
  `BACKLOG.md` nombra un punto que existe. Scope: project. Surgió en el goal `asset-conversation`.

### Dirección

- **lateral-communication-rules**: capture. Las reglas de cuándo hablan dos pares de un mismo
  nivel, qué pueden acordar entre ellos y cómo queda registrado. Decidido: los pares hablan directo
  y lo que excede su autonomía sube. Surgió en el capture `foundation`.
- **adoption-design**: capture. El detalle de la adopción, para decidirlo con un caso real cuando la
  app exista: cómo se reparte el conocimiento previo de un repo en sus niveles, la propuesta de
  topología al terminar, la transición de los archivos de conocimiento viejos y los proyectos sin
  conocimiento escrito. Decidido: la adopción es gradual, no modifica los repos y termina con una
  propuesta de topología. Propuestas sin decidir: criterios de topología (lo que cambia siempre
  junto va junto; dueños, visibilidad o ritmos de versión distintos separan) y un `AGENTS.md`
  generado en cada repo adoptado, con visibilidad por parte del conocimiento. Surgió en el capture
  `foundation`.

### Conocimiento

- **knowledge-size-caps**: un tope de tamaño para el conocimiento de cada nivel (organización,
  producto, activo), verificado por un check, para que la lectura obligatoria no crezca sin límite.
  Extends: federation. Surgió en el capture `foundation`.
- **conversation-portability**: copiar las conversaciones de una persona a otra máquina, porque hoy
  quedan solo en su runtime. Extends: kernel. Surgió al definir el goal `asset-conversation`.
- **organization-identity**: el dueño de un activo es la identidad de git dentro del repo de
  federación de esa organización, no la global del runtime, porque una persona puede usar correos
  distintos por carpeta. Extends: kernel. Surgió en el goal `asset-conversation`.
- **remote-asset-location**: el kernel une el repo y la carpeta de un activo suponiendo una ruta
  local; esa unión pasa a `federation` cuando el repo sea una URL remota. Scope: federation/catalog.
  Surgió en el goal `asset-conversation`.
- **moved-asset-session**: un activo que se mueve de carpeta ya no puede retomar la sesión de
  `claude` guardada, y su peer falla cada mensaje hasta que esa sesión se limpie. Scope:
  kernel/peer. Surgió en el goal `asset-conversation`.

### Runtime

- **runtime-lifecycle**: el resto del ciclo de vida decidido en `AGENTS.md`: el runtime se apaga
  solo tras un rato sin agentes trabajando ni clientes conectados, se muestra en la barra del
  sistema mientras tiene agentes trabajando, la app reemplaza un runtime de otra versión sin agentes
  y avisa si tiene agentes, y el instalador trae el runtime como sidecar de Tauri. También: un
  segundo runtime sobre la misma carpeta de datos se niega a arrancar mientras el publicado responde;
  el runtime que arranca la app escribe su stderr en la carpeta de datos para que la app muestre por
  qué terminó, y la app lo recoge al cerrarse; y los íconos `.icns` de macOS al empaquetar. Extends:
  runtime. Surgió al definir el goal `asset-conversation`.

### Interfaz

- **app-messages**: los errores que ve la persona están en el idioma de la app y dicen qué hacer,
  como el que aparece al abrir como organización la carpeta de un activo, hoy en inglés y sin
  indicación. Scope: desktop/organization-setup. Surgió en el recorrido final del goal
  `asset-conversation`.
- **peer-markdown**: las respuestas del peer se muestran con su Markdown formateado, no con los `**`
  a la vista. Scope: desktop/asset-conversation. Surgió en el recorrido final del goal
  `asset-conversation`.

### Motores

- **claude-remote-sign-in**: iniciar sesión en Claude cuando el runtime corre en un servidor remoto,
  donde el navegador de la persona no puede volver al `claude` de esa máquina y `claude auth login`
  pide pegar un código; dónde se pega ese código se decide entonces. Scope: claude/sign-in. Surgió
  en el change `claude-sign-in`.
- **claude-managed-install**: que la app instale y maneje su propia copia del `claude` oficial en una
  versión exacta, en vez de usar la que tiene la persona. Extends: claude. Surgió al definir el goal
  `asset-conversation`.
- **codex-remote-plan**: usar el plan de ChatGPT con Codex desde un servidor remoto. El programa de
  OpenAI para apps open source cubre apps locales; para un servidor hay que llenar su formulario de
  interés. Extends: engine. Prerrequisito que solo ve la persona: OpenAI aprobó el uso remoto.
  Surgió en el capture `foundation`.

### Publicación

- **reserve-package-names**: reservar el nombre `federa` en crates.io y npm con un paquete vacío,
  antes de que alguien lo tome. Scope: project. Lo publica la persona, porque sale de su máquina.
  Surgió en el capture `foundation`.
