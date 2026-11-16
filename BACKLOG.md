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

- **project-base**: short. Outcome: el repo tiene su base de trabajo y sus checks corren en cada
  commit y en CI. In scope: proto con las versiones de toolchains; moon para tareas y hooks de git,
  instalados una sola vez desde el checkout principal y nunca sincronizados desde un worktree,
  porque moon escribe la ruta de los hooks en la configuración que comparten todos; workspaces de
  Cargo y pnpm que toman todo `crates/*` y `apps/*`, así ningún change paralelo edita el mismo
  manifiesto; rustfmt, clippy, cargo-deny (licencias, vulnerabilidades y la dirección de
  dependencias entre crates de `AGENTS.md`); TypeScript 7 con Vite+ (Vite, Vitest, Oxlint y Oxfmt en
  un `vite.config.ts` raíz); commitlint en `commit-msg`; betterleaks; license-checker para
  TypeScript; un check de 300 líneas por archivo de código sin contar tests; un check de tope de
  `AGENTS.md`; un check de la estructura de cada `README.md`; `.claude/worktrees/` fuera de toda
  herramienta; GitHub Actions corriendo las mismas tareas de moon; el `LICENSE` Apache-2.0.
  Escribe en `AGENTS.md` el stack de desarrollo y los comandos del proyecto, y sube su tope a 500
  líneas. Out of scope: crates y apps, que traen los demás changes. Done when: un commit con un
  mensaje fuera de Conventional Commits se rechaza, un secreto de prueba se rechaza, y la tarea de
  checks de moon pasa en local y en CI. Scope: project. Decidido: el stack de arriba; la
  configuración de moon y de Vite+ de `/Users/ccosming/Github/aikuora/aikuora` sirve de referencia,
  sin copiar sus reglas propias.
- **protocol-contract**: full. Outcome: existe el contrato entre clientes y runtime. In scope: el
  crate `protocol` con los comandos y eventos de crear y abrir una organización, registrar un activo,
  listar activos, y conversar con el peer de un activo (enviar mensaje, recibir la respuesta en
  partes, traer la conversación guardada); el saludo inicial donde cliente y runtime declaran la
  versión del protocolo; la generación de los tipos TypeScript desde Rust. Out of scope: el
  transporte, que trae `runtime-server`. Done when: los tipos TypeScript generados compilan y un
  check falla si quedan desalineados con Rust. Extends: protocol. After: project-base.
- **federation-store**: full. Outcome: Federa crea y abre un repo de federación y registra un activo
  en su catálogo. In scope: el crate `federation`, detrás de su interfaz; `organization/knowledge.md`;
  `assets/<activo>.toml` con dueño, ubicación (repo y carpeta), etapa y si está gestionado; leer el
  `.federa/asset.toml` y el `.federa/knowledge.md` de un activo. Out of scope: productos, que siguen
  implícitos; `work/`; contratos entre activos. Done when: un test crea un repo de federación, le
  registra un activo de prueba con su `.federa/` y lo vuelve a leer. Extends: federation. After:
  project-base. Decidido: TOML para lo estructurado y Markdown para el conocimiento, con
  `schema = 1` en cada TOML; se reescribe en su lugar hasta la primera versión publicada.
- **claude-engine**: full. Outcome: el kernel puede abrir un agente en Claude y conversar con él a
  través del contrato de motores. In scope: el crate `engine` con el contrato común (abrir un agente
  con su rol e instrucciones, mandarle un mensaje, recibir sus eventos, cerrarlo); el crate `claude`,
  que lanza el binario oficial con `-p --input-format stream-json --output-format stream-json
  --verbose`, con el sandbox siempre activo; versión mínima `2.1.311`, aceptando las más nuevas sin
  aviso y avisando por debajo; lee solo los campos que usa e ignora el resto, y falla con un error
  claro cuando falta uno que necesita; detecta la sesión no iniciada por `error:
  "authentication_failed"` y devuelve el aviso de iniciarla con `/login` en el flujo oficial; tests
  sin red con grabaciones reales de `2.1.311` de abrir sesión, dos turnos, una herramienta MCP y la
  sesión no iniciada. Out of scope: Codex; que la app instale su propia copia de `claude`. Done
  when: los tests sin red pasan, y un test en vivo, que corre la persona, conversa dos turnos con su
  suscripción. Extends: engine, claude. After: project-base. Background verificado al comparar
  `2.1.311` con `2.1.325`: entre ellas solo aparecieron tres campos opcionales; `system/init` se
  repite en cada turno; el modelo a veces llama a `ToolSearch` antes de una herramienta MCP y a veces
  no, y el adaptador acepta ambos caminos; la entrada es
  `{"type":"user","message":{"role":"user","content":"<texto>"}}`; la sesión no iniciada se simula
  sin tocar el llavero real con un `CLAUDE_CONFIG_DIR` vacío, porque el nombre de la entrada del
  llavero depende de esa carpeta. La grabación nunca lee, copia ni imprime credenciales.
- **kernel-asset**: full. Outcome: el kernel crea y abre organizaciones y registra activos. In
  scope: el crate `kernel` con los módulos `organization` y `asset`, que atienden los comandos de
  `protocol` usando `federation`. Out of scope: conversaciones y herramientas MCP. Done when: un test
  crea una organización y registra un activo usando solo comandos del protocolo. Extends: kernel.
  After: protocol-contract, federation-store.
- **runtime-server**: full. Outcome: el runtime corre como proceso propio y atiende clientes. In
  scope: `apps/runtime`, que conecta `kernel`, `federation` y `claude`, sirve el protocolo por
  WebSocket solo en `127.0.0.1`, con un token en un archivo que solo el usuario puede leer, y
  responde el saludo de versión. Out of scope: apagado por inactividad, ícono en la barra del
  sistema y reemplazo de otra versión. Done when: un cliente de prueba se conecta con el token, se
  rechaza sin él, y crea una organización. Extends: runtime. After: kernel-asset.
- **kernel-tools**: full. Outcome: los agentes usan herramientas de Federa por MCP. In scope: el
  runtime atiende MCP por HTTP; cada agente recibe un token propio con su rol, su sesión, su activo
  y su goal, y el kernel sabe quién llama en cada pedido; el runtime agrega al sandbox del agente el
  permiso para llegar a él y nada más; la primera herramienta, leer el conocimiento del activo del
  agente. Out of scope: herramientas de comunicación entre agentes. Done when: un test llama a la
  herramienta con el token de un agente y recibe el conocimiento de su activo, y sin token se
  rechaza. Extends: kernel, runtime. After: runtime-server.
- **peer-on-asset**: full. Outcome: la persona conversa con su peer sobre un activo y la
  conversación queda guardada. In scope: el kernel abre el peer en el motor de Claude, con su rol y
  conectado a las herramientas MCP, y responde a los comandos de conversación del protocolo; las
  conversaciones son privadas y se guardan en la carpeta de datos del runtime de la persona, pegadas
  al activo. Out of scope: decisiones que pasan al repo de federación; otros roles de agente. Done
  when: un test en vivo, que corre la persona, pregunta qué hace un activo de prueba y la respuesta
  usa su `.federa/knowledge.md`; reiniciar el runtime y traer la conversación la devuelve completa.
  Extends: kernel. After: claude-engine, kernel-tools.
- **desktop-asset-view**: full. Outcome: la persona hace el recorrido del goal desde la app. In
  scope: `apps/desktop` con Tauri 2, React, TypeScript y shadcn, módulos por funcionalidad y los
  tipos del protocolo generados; arranca el runtime si no hay uno corriendo y se conecta con su
  token; crear la organización, registrar el activo, verlo en el mapa y conversar con el peer dentro
  del activo. Out of scope: empaquetar el runtime en el instalador; las demás superficies. Done when:
  el Done when del goal, hecho a mano por la persona. Extends: desktop. After: protocol-contract.

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
