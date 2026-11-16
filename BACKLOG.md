# Backlog

## Goals

### asset-conversation

La persona registra un activo en su organización y conversa con su peer sobre él. Done when: abres
Federa y creas una organización en una carpeta nueva, que pasa a ser su repo de federación;
registras como activo un repo local que tiene un `.federa/` con su alcance; le preguntas a tu peer
qué hace ese activo y su respuesta usa lo que dice ese `.federa/`; cierras la app, la vuelves a
abrir y la conversación sigue en el activo. Primero, porque cruza todas las capas (cliente,
protocolo, runtime, kernel, repo de federación, `.federa/`, motor de Claude con la suscripción de la
persona y herramientas MCP) y saca el riesgo técnico más grande: manejar el binario oficial
`claude` desde Rust. Trae además la base del proyecto. Surgió en el capture `foundation`; sus
decisiones de dirección están en `AGENTS.md`.

- **desktop-asset-view**: full. Outcome: la persona hace el recorrido del goal desde la app. In
  scope: `apps/desktop` con Tauri 2, React, TypeScript y shadcn, módulos por funcionalidad y los
  tipos del protocolo generados; arranca el runtime si no hay uno corriendo y se conecta con su
  token; crear la organización, registrar el activo, verlo en el mapa y conversar con el peer dentro
  del activo; el botón "Conectar con Claude" cuando el runtime no tiene la sesión iniciada. Out of
  scope: empaquetar el runtime en el instalador; las demás superficies. Done when: el Done when del
  goal, hecho a mano por la persona. Extends: desktop. After: runtime-server, claude-sign-in.

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

- **no-commit-scopes**: short. commitlint rechaza todo commit con scope (`scope-empty` siempre), y
  la línea de Repositorio de `AGENTS.md` dice Conventional Commits sin scope, verificado por
  commitlint. Scope: project. Corre apenas termina el goal `asset-conversation` y antes del primer
  push, junto con reescribir la historia de `main` para quitar los scopes existentes, que hace el
  peer con una rama de respaldo. Surgió durante el goal `asset-conversation`.

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

### Runtime

- **runtime-lifecycle**: el resto del ciclo de vida decidido en `AGENTS.md`: el runtime se apaga
  solo tras un rato sin agentes trabajando ni clientes conectados, se muestra en la barra del
  sistema mientras tiene agentes trabajando, la app reemplaza un runtime de otra versión sin agentes
  y avisa si tiene agentes, y el instalador trae el runtime como sidecar de Tauri. Extends: runtime.
  Surgió al definir el goal `asset-conversation`.

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
