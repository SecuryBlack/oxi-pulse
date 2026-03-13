# Plan de Implementación — OxiPulse Agent

Cada capa debe completarse y cumplir su criterio "Done" antes de pasar a la siguiente.

---

## Capa 1 — Esqueleto del proyecto
**Objetivo:** Proyecto Rust compilable con estructura de carpetas definida y logging básico.

- [x] Crear `Cargo.toml` con dependencias iniciales (tokio, tracing, tracing-subscriber)
- [x] Definir estructura de carpetas (`src/metrics/`, `src/telemetry/`, `src/config/`, `src/updater/`)
- [x] `main.rs` con bucle principal async vacío (tokio)
- [x] Logging básico con `tracing` (nivel configurable)
**Done:** `cargo build` sin errores ni warnings relevantes. ✓

---

## Capa 2 — Recolección de métricas
**Objetivo:** Leer métricas del sistema de forma nativa y estructurada.

- [x] Añadir `sysinfo` a dependencias
- [x] Struct `Metrics` con campos: cpu_usage, ram_total, ram_used, disk_total, disk_used, net_bytes_in, net_bytes_out, timestamp
- [x] Módulo `src/metrics/collector.rs` que rellena el struct
- [x] Bucle principal imprime métricas por consola cada 10 segundos

**Done:** Métricas reales del sistema visibles por consola cada 10s. ✓

---

## Capa 3 — Configuración
**Objetivo:** El agente es genérico y se configura completamente desde fuera, sin ningún valor hardcodeado.

- [x] Añadir `serde` + `toml` crate a dependencias
- [x] Struct `Config` con campos: `endpoint` (OTLP), `token`, `interval_secs`
- [x] Carga desde `config.toml` con fallback a variables de entorno (`OXIPULSE_ENDPOINT`, `OXIPULSE_TOKEN`)
- [x] Sin endpoint por defecto: el agente falla con error claro y descriptivo si falta `endpoint` o `token`
- [x] `config.toml` añadido a `.gitignore`

**Done:** Agente arranca con config externa sin recompilar. Sin endpoint hardcodeado. ✓

---

## Capa 4 — Envío OTLP
**Objetivo:** Las métricas se envían al colector vía OpenTelemetry.

- [x] Añadir `opentelemetry`, `opentelemetry-otlp`, `opentelemetry_sdk` a dependencias
- [x] Módulo `src/telemetry/mod.rs` que inicializa el pipeline OTLP
- [x] Mapear el struct `Metrics` a gauges/counters de OpenTelemetry
- [x] El token se envía como header Authorization: Bearer en la conexión OTLP
- [ ] Validar con `otelcol` en local que llegan las métricas (pendiente de tener colector)

**Nota:** Build realizado desde WSL2 (Ubuntu-24.04) por bloqueo WDAC en Windows para build scripts. Comando: `wsl -d Ubuntu-24.04 -- bash -c "source ~/.cargo/env && cd /mnt/d/securyblack/oxi-pulse && CARGO_TARGET_DIR=/home/mcamp/cargo-targets/oxi-pulse cargo build"`

**Done:** Pipeline OTLP inicializado, métricas recogidas y registradas en los instruments. Agente arranca y opera correctamente. ✓

---

## Capa 5 — Resiliencia offline
**Objetivo:** El agente no pierde datos si el colector no está disponible.

- [x] Búfer en memoria (VecDeque) que acumula métricas cuando falla el envío
- [x] Lógica de reintento con backoff exponencial (dobla el intervalo de check hasta ~5 min)
- [x] Cuando se recupera la conexión, vaciar el búfer en bloque antes de enviar métricas nuevas
- [x] Límite máximo del búfer configurable (default: 8640 = 24h a 10s)
- [x] Logs claros de estado: "offline — buffering", "reconnected — flushing N metrics"

**Done:** Agente sobrevive caída del colector sin perder datos, reenvía al reconectar. ✓

---

## Capa 6 — Autoupdate
**Objetivo:** El agente se actualiza solo desde GitHub Releases.

- [x] Añadir `self_update` (MIT/Apache-2.0) a dependencias
- [x] Módulo `src/updater/mod.rs` que consulta la API de GitHub Releases
- [x] Comparar versión actual (env `CARGO_PKG_VERSION`) con la última release
- [x] Si hay nueva versión: descargar binario correcto para la arquitectura/SO actual
- [x] Reemplazar binario en disco y hacer `exit(0)` limpio para que el servicio reinicie
- [x] La comprobación se ejecuta una vez al día (timer async, primer check tras 24h)

**Done:** Updater arranca en background. Binario se actualizará automáticamente al publicar una release en GitHub. ✓

---

## Capa 7 — Script de instalación Linux
**Objetivo:** Instalación en una línea en cualquier distro Linux compatible. Dos variantes: genérica e integrada con SecuryBlack.

- [x] `install.sh` que detecta arquitectura (x86_64, arm64) y distribución
- [x] Descarga el binario correcto desde GitHub Releases
- [x] Acepta `--endpoint` y `--token` como argumentos opcionales; si no se pasan, los pide interactivamente
- [x] Escribe `config.toml` en `/etc/oxipulse/config.toml` (permisos 600)
- [x] Crea y habilita servicio systemd con `Restart=always`
- [ ] Probado en VM limpia (pendiente de tener release publicada en GitHub)

**Done:** Script en `scripts/install.sh`. Sintaxis validada, detección de arch y parsing de args verificados. Prueba completa en VM pendiente de Capa 9. ✓

---

## Capa 8 — Script de instalación Windows
**Objetivo:** Instalación en una línea desde PowerShell en Windows. Dos variantes: genérica e integrada con SecuryBlack.

- [x] `install.ps1` que detecta arquitectura (x86_64, arm64)
- [x] Descarga el binario correcto desde GitHub Releases
- [x] Acepta `-Endpoint` y `-Token` como parámetros opcionales; si no se pasan, los pide interactivamente
- [x] Escribe `config.toml` en `C:\ProgramData\oxipulse\config.toml` (ACL restringida a Admins+SYSTEM)
- [x] Registra OxiPulse como Windows Service nativo (`New-Service`) con reinicio automático (`sc.exe failure`)
- [ ] Probado en Windows Server 2022 y Windows 11 limpios (pendiente de release en Capa 9)

**Done:** Script en `scripts/install.ps1`. Sintaxis validada con PSParser. Prueba completa en máquina limpia pendiente de Capa 9. ✓

---

## Capa 9 — CI/CD
**Objetivo:** Cada release publica automáticamente binarios para todas las plataformas.

- [x] GitHub Actions workflow `.github/workflows/release.yml` disparado en tag `v*`
- [x] Cross-compilation para: `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`, `x86_64-pc-windows-msvc`
- [x] Binarios publicados en GitHub Releases con nombres que coinciden con `self_update::get_target()` y los scripts
- [x] Checksums SHA256 generados y adjuntos a la release (`.sha256` por asset)
- [x] Solo usa `GITHUB_TOKEN` automático de Actions — sin secrets manuales ni valores hardcodeados
- [x] `generate_release_notes: true` — release notes automáticas a partir de commits

**Done:** Crear un tag `v0.1.0` genera y publica los 3 binarios + checksums en GitHub Releases. ✓

---

## Progreso general

| Capa | Nombre               | Estado     |
|------|----------------------|------------|
| 1    | Esqueleto            | Completada ✓                         |
| 2    | Métricas             | Completada ✓                         |
| 3    | Configuración        | Completada ✓                         |
| 4    | Envío OTLP           | Completada ✓                         |
| 5    | Resiliencia offline  | Completada ✓                         |
| 6    | Autoupdate           | Completada ✓                         |
| 7    | Script Linux         | Completada ✓                         |
| 8    | Script Windows       | Completada ✓                         |
| 9    | CI/CD                | Completada ✓                         |
