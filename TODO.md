# OxiPulse — Mejoras Pendientes

> Generado: 2026-07-30 · Revisión completa de `oxi-pulse`, `oxi-pulse-homepage`, `oxi-pulse-install`

---

## 🔴 Críticas

- [x] **1. CHANGELOG desactualizado** — Solo documenta hasta `v0.1.13`, faltan 24 releases hasta `v0.3.8`. Reconstruir desde git log.
- [x] **2. CODE_OF_CONDUCT.md** — Referenciado en README L295 pero no existe. Crear el archivo.
- [x] **3. SECURITY.md** — Referenciado en README L266 pero no existe. Crear Security Policy.
- [x] **4. Issue templates genéricos** — Los templates hablan de "Browser", "Smartphone", "iOS". Reescribir para server monitoring (OS, versión agente, logs, config).

---

## 🟡 Importantes

- [x] **5. CI workflow para PRs** — No hay CI en pull requests. Crear workflow con `cargo build`, `cargo test`, `cargo clippy`, `cargo fmt --check` en Linux + Windows.
- [x] **6. README: versión de Rust incorrecta** — Dice "Requires Rust 1.70+" pero `edition = "2024"` requiere Rust 1.85+. Corregir.
- [x] **7. Tests para config** — `config/mod.rs` no tiene tests. Cubrir: parsing TOML, env var overrides, mode `local_agent`, valores por defecto.
- [x] **8. Tests para collector** — `metrics/collector.rs` cubierto con unit test de recolección y creación.
- [x] **9. Rotación de logs en Windows** — `tracing_appender::rolling::daily` crea archivos sin límite. Añadir limpieza de logs antiguos (ej. max 7 días).
- [x] **10. Graceful shutdown: flush OTLP** — Al recibir shutdown no se llama `provider.shutdown()`. Puede perder la última batch de métricas.

---

## 🟢 Nice-to-have

- [x] **11. Reutilizar reqwest client en phone_home** — Se crea un `Client` nuevo en cada ping. Crear una vez y reutilizar para connection pooling.
- [x] **12. Nuevas métricas: system uptime** — `system.uptime` recolectado y exportado vía OTLP.
- [x] **13. Nuevas métricas: CPU cores** — `system.cpu.count` recolectado y exportado vía OTLP.
- [x] **14. Nuevas métricas: swap** — `system.memory.swap.used` y `total` recolectados y exportados vía OTLP.
- [x] **15. Nuevas métricas: load average** — `system.cpu.load_average.1m`, `5m`, `15m` recolectadas y exportadas vía OTLP.
- [x] **16. Eliminar `#[allow(dead_code)]`** — `timestamp_unix_ms` en collector.rs es ahora un campo público activo.
- [x] **17. Install worker: error handling** — Añadido bloque try/catch, HTTP status 502/500 en fallos de GitHub y caché s-maxage=60.



---

## 🌐 Homepage (`oxi-pulse-homepage`)

- [x] **18. Deploy** — Publicado en Cloudflare Workers.
- [x] **19. Dominio** — Vinculado y apuntado el dominio `oxipulse.dev`.
- [x] **20. Changelog en Frontend** — Actualizado `lib/changelog.ts` con la versión v0.3.8 y sincronizado con el agente.
- [ ] **21. Internacionalización** — Desestimado por el momento.

