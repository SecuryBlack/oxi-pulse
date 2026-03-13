Documento de Especificación de Producto: OxiPulse (v0.1.0 - MVP)
1. Visión General del Proyecto
OxiPulse es un agente de monitorización de servidores Open Source diseñado para ser ultraligero, seguro y altamente escalable. El objetivo principal es proporcionar una alternativa a los agentes de telemetría pesados, garantizando un consumo mínimo de recursos en la máquina del cliente.

OxiPulse es un agente genérico con backend configurable: el binario es único y público, y el usuario decide a qué backend OTLP envía sus métricas. Esto permite tres modelos de uso:
- SaaS gestionado (SecuryBlack): el panel genera un script de instalación con el endpoint y token preconfigurados.
- Self-hosted: el usuario despliega su propio ingestor OTLP y apunta el agente a él.
- Compatible con terceros: cualquier backend que hable OTLP estándar (Grafana Cloud, Datadog, etc.).

El agente no tiene ningún endpoint por defecto hardcodeado. SecuryBlack compite en producto, no en lock-in del agente.

El sistema se compone del Agente OxiPulse (público, open source) y una infraestructura central opcional (privada, SaaS de SecuryBlack) que recibe, almacena y visualiza las métricas.

2. Componentes del Sistema (El "Qué" construir)
El equipo debe desarrollar y conectar los siguientes 5 bloques principales:

Bloque A: El Agente OxiPulse (Rust)
Es el ejecutable que corre en el servidor del usuario. Debe ser un binario estático que actúe como un servicio en segundo plano.

Recolección de Métricas (MVP): Debe leer de forma nativa el uso de CPU (%), RAM (Total/Usada), Disco (Total/Usado en /) y Red (Bytes In/Out).

Frecuencia y Protocolo: Debe agrupar estas métricas y enviarlas cada 10 segundos al servidor central usando OpenTelemetry (OTLP sobre gRPC o HTTP). Se usarán los crates opentelemetry y opentelemetry-otlp (ambos Apache-2.0). El timestamp de los datos debe ser generado en el momento de la lectura, no del envío. El endpoint OTLP del colector se configura vía variable de entorno o fichero de configuración, nunca hardcodeado en el binario.

Resiliencia (Modo Offline): Si el servidor central no responde, el agente no debe colapsar ni descartar los datos. Debe guardarlos en un búfer local temporal y enviarlos en bloque cuando recupere la conexión.

Autoupdate: El agente debe ser capaz de consultar diariamente el repositorio de GitHub Releases, detectar si hay una nueva versión, descargar el binario, reemplazar el actual en disco y salir (exit) limpiamente para que el sistema operativo lo reinicie. Se implementará usando el crate self_update (licencia MIT/Apache-2.0, compatible con Apache 2.0).

Bloque B: Sistema de Instalación y Distribución
Mecanismo para que la adopción por parte del usuario sea fricción cero. Existen dos variantes de script:

Variante genérica (repo público de OxiPulse): Para cualquier usuario que quiera instalar el agente y configurarlo manualmente o apuntarlo a su propio backend.

Script Genérico Linux: curl -fsSL https://install.oxipulse.io | bash
Script Genérico Windows: irm https://install.oxipulse.io/windows | iex
El script pide interactivamente el endpoint OTLP y el token, o los acepta como argumentos.

Variante SecuryBlack (generada desde el panel SaaS): El panel genera un comando personalizado con el endpoint y token de SecuryBlack preinyectados, listo para pegar en el servidor del cliente. El usuario nunca ve ni configura nada manualmente.

Script SecuryBlack Linux: curl -fsSL https://install.oxipulse.io | bash -s -- --endpoint ingest.securyblack.com --token <TOKEN>
Script SecuryBlack Windows: irm https://install.oxipulse.io/windows | iex -Endpoint ingest.securyblack.com -Token <TOKEN>

Responsabilidades comunes de ambos scripts:
Linux: 1. Detectar arquitectura (x86_64, ARM64) y distribución. 2. Descargar binario desde GitHub Releases. 3. Escribir config.toml. 4. Crear y habilitar servicio systemd (Restart=always).
Windows: 1. Detectar arquitectura. 2. Descargar binario desde GitHub Releases. 3. Escribir config.toml. 4. Registrar como Windows Service nativo (New-Service) con reinicio automático.

CI/CD (GitHub Actions): Tubería automatizada que, al crear una nueva Release, compile el binario de Rust para las diferentes arquitecturas (linux/x86_64, linux/arm64, windows/x86_64) y lo publique automáticamente en GitHub Releases.

Bloque C: El Ingestor de Telemetría (Rust)
Es el "portero" de nuestra infraestructura central. Su único objetivo es absorber tráfico masivo sin afectar al resto de la aplicación.

Recepción OTLP: Debe exponer un endpoint OTLP (OpenTelemetry Protocol) para recibir las conexiones de los miles de agentes OxiPulse. Compatible con cualquier agente que hable OTLP estándar.

Seguridad (Auth): Debe validar los Tokens (JWT o API Keys) que llegan en las cabeceras (headers) de las peticiones OTLP antes de procesar el payload. Debe rechazar conexiones inválidas instantáneamente.

Batching (Agrupación): No debe escribir en la base de datos por cada petición individual. Debe agrupar las métricas en memoria durante breves intervalos (ej. 1-2 segundos) y hacer inserciones masivas en la base de datos de series temporales.

Actualización de Estado (Heartbeat invisible): Al recibir cualquier métrica válida de un servidor, debe actualizar asíncronamente en una caché rápida (Redis) el campo last_seen de ese servidor con la fecha actual.

Bloque D: Almacenamiento (Datastore)
La capa de persistencia optimizada para el caso de uso.

Base de Datos Principal (Relacional): PostgreSQL/MySQL para guardar usuarios, facturación, y la relación de qué servidores pertenecen a qué cuenta.

TSDB (Base de Datos de Series Temporales): Un motor específico (TimescaleDB o ClickHouse) para almacenar exclusivamente el histórico de métricas con alta compresión.

Caché de Estado: Redis para mantener el estado "En línea/Fuera de línea" de los servidores en tiempo real (last_seen).

Bloque E: API Core y Panel de Control (FastAPI + Frontend)
El cerebro de la lógica de negocio y la cara visible para el cliente.

Autenticación y Registro: Gestión de usuarios y generación de los Tokens que usarán los agentes OxiPulse.

API de Lectura de Métricas: Endpoints optimizados que consulten la TSDB y devuelvan agregaciones de datos al frontend (ej: "uso medio de CPU en las últimas 24h").

Gestión de Estado de Servidores: Un endpoint que consulte Redis para devolver al frontend qué servidores están en verde 🟢 (vistos recientemente) y cuáles en rojo 🔴 (sin conexión).

(Nota: El sistema de monitores Cron/Backups se mantendrá aquí en Python, separado de OxiPulse).

3. Licenciamiento y Open Source
Agente OxiPulse (Repositorio Público): Licencia Apache 2.0. Todas las dependencias deben ser compatibles (MIT, Apache-2.0, BSD). Ninguna dependencia GPL o LGPL.

Infraestructura Core (Ingestor, API, Frontend): Mantenido en repositorios privados (código cerrado) o bajo licencia restrictiva (AGPLv3) si se decide hacer la plataforma entera Open Source en el futuro.

Higiene del Repositorio Público (Protección SecuryBlack):
- Ninguna URL, dominio ni IP de infraestructura de SecuryBlack hardcodeada en el código. El endpoint OTLP se configura siempre externamente (variable de entorno OXIPULSE_ENDPOINT o config file).
- Ninguna referencia a sistemas internos de SecuryBlack (nombres de servidores, proyectos, clientes, credenciales).
- El .gitignore debe excluir explícitamente .env, *.key, *.pem, config.toml (ficheros de config local).
- Las GitHub Actions de CI/CD usarán únicamente Secrets de GitHub, nunca valores en texto plano en los workflows.
- Revisión obligatoria de cada commit antes de push al repo público para detectar fugas accidentales.
- El README debe presentar OxiPulse como proyecto independiente y Open Source, sin mencionar SecuryBlack a menos que sea una decisión deliberada de marketing.

4. Hitos de Entrega (Roadmap MVP)
Fase 1 (El Contrato): Definición del esquema de métricas OTLP que enviará el Agente y recibirá el Ingestor. Validación del pipeline end-to-end con un colector OTLP de prueba (ej. otelcol en local).

Fase 2 (Tubería de datos): Agente Rust enviando datos simulados + Ingestor Rust recibiéndolos e imprimiéndolos en consola.

Fase 3 (Persistencia): Ingestor guardando en la TSDB + FastAPI exponiendo esos datos en JSON.

Fase 4 (Distribución): Autoupdater funcional (self_update) + Script Linux curl|bash estilo Coolify + Script Windows irm|iex + CI/CD cross-compilation.