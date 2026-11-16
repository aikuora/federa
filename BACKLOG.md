# Backlog

## Goals

### asset-conversation

La persona registra un activo en su organización y conversa con su peer sobre él. Done when: abres
Federa, creas una organización en una carpeta nueva, que pasa a ser su repo de federación; registras
como activo un repo local que tiene un `.federa/` con su alcance; le preguntas a tu peer qué hace
ese activo y su respuesta usa lo que dice el `.federa/`; cierras la app, la vuelves a abrir y la
conversación sigue en el activo. Primero, porque cruza todas las capas (cliente, protocolo, núcleo,
repo de federación, `.federa/`, motor de Claude con la suscripción de la persona y MCP) y saca el
riesgo técnico más grande: manejar el binario oficial `claude` desde Rust por stdio. Trae además la
base del proyecto. Surgió en el capture `foundation`.

- **project-base**: el workspace de Cargo y el de pnpm, los comandos del proyecto para construir,
  testear y correr los checks, y los checks en pre-commit y en un CI de GitHub Actions: formato,
  clippy, tests, escaneo de secretos, Conventional Commits en commit-msg, licencias de dependencias
  en Rust y TypeScript, `.claude/worktrees/` fuera de toda herramienta, el tope de `AGENTS.md`, y la
  estructura de cada `README.md`. Trae el `LICENSE` Apache-2.0. Scope: project. Decide con la
  persona: qué herramienta corre los hooks de git. Propuesta: lefthook, que la persona ya usa.
- **core-protocol**: el núcleo como crate que expone el protocolo de comandos y eventos, con los
  tipos TypeScript generados desde Rust. Define desde ya los comandos y eventos de organización,
  activo y conversación que usa este goal, para que el cliente y el núcleo avancen en paralelo.
  Extends: core. After: project-base. Decide con la persona: cómo viaja el protocolo entre cliente y
  núcleo. Propuesta: el núcleo como proceso aparte desde el inicio, porque el mismo protocolo tiene
  que servir con un núcleo remoto.
- **federation-store**: crear y abrir un repo de federación, y registrar un activo en el catálogo
  leyendo su `.federa/`. Extends: federation. After: project-base. Decide con la persona: el formato
  de los archivos del catálogo y de `.federa/`, que es un schema que otros repos van a guardar.
- **claude-engine**: el adaptador que lanza el `claude` oficial y le habla JSON por stdio, con una
  versión fijada, tests de protocolo contra esa versión y el sandbox siempre activo. Extends:
  engines. After: project-base. Decide con la persona: qué versión de `claude` se fija, y qué ve la
  persona cuando no inició sesión. Propuesta: la app le indica iniciarla con `/login` en el flujo
  oficial, sin tocar credenciales.
- **core-mcp**: el servidor MCP del núcleo, con su primera herramienta: leer el conocimiento de un
  activo. Extends: core. After: core-protocol, federation-store. Decide con la persona: el
  transporte MCP entre el núcleo y cada agente.
- **peer-on-asset**: el núcleo abre un peer en el motor de Claude, con su rol y conectado al MCP, y
  guarda la conversación en el activo. Extends: core. After: claude-engine, core-mcp. Decide con la
  persona: dónde se guardan las conversaciones. Propuesta: en el núcleo de la persona, porque una
  conversación es de una persona, y lo que se decide en ella pasa al repo de federación como
  decisión.
- **desktop-asset-view**: la app Tauri con React y shadcn, donde la persona crea la organización,
  registra el activo, lo ve en el mapa y conversa con su peer dentro de él. Extends: desktop. After:
  core-protocol.

### coordinated-goal

La persona define un goal con su peer, y un coordinator lo construye con implementers y reviewers en
worktrees, lo integra con un merge local y la persona lo ve avanzar en el trabajo en curso.
Segundo, porque es el corazón del modelo operativo y lo que reemplaza a tmux y al plugin actual.

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

La app se conecta a un núcleo que corre en un servidor, con los agentes trabajando ahí.

## Loose entries

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

### Motores

- **codex-remote-plan**: usar el plan de ChatGPT con Codex desde un servidor remoto. El programa de
  OpenAI para apps open source cubre apps locales; para un servidor hay que llenar su formulario de
  interés. Extends: engines. Prerrequisito que solo ve la persona: OpenAI aprobó el uso remoto.
  Surgió en el capture `foundation`.

### Publicación

- **reserve-package-names**: reservar el nombre `federa` en crates.io y npm con un paquete vacío,
  antes de que alguien lo tome. Scope: project. Lo publica la persona, porque sale de su máquina.
  Surgió en el capture `foundation`.
