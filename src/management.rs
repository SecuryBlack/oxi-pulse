//! Handlers registrados en el intake de comandos. Hoy solo `update_now` — ver
//! el mismo comando en ferro-sentry (`management::commands`) para el diseño
//! original: dispara `sb_agent_core::updater::check_now` de inmediato en vez
//! de esperar al chequeo diario, para el botón "Actualizar" de la app.

use sb_agent_core::command_intake::{CommandOutcome, CommandRegistry};

pub fn register(registry: &CommandRegistry) {
    registry.register("update_now", move |_payload, _progress| async move {
        handle_update_now().await
    });
}

async fn handle_update_now() -> CommandOutcome {
    let cfg = sb_agent_core::updater::UpdaterConfig::new(
        "securyblack",
        "oxi-pulse",
        "oxipulse",
        env!("CARGO_PKG_VERSION"),
    );

    let result = tokio::task::spawn_blocking(move || sb_agent_core::updater::check_now(&cfg)).await;

    match result {
        Ok(Ok(true)) => {
            // Mismo margen que ferro-sentry: deja que esta respuesta salga
            // por el intake antes de que el reinicio (necesario para cargar
            // el binario nuevo que `self_update` ya dejó en disco) corte la
            // conexión.
            std::thread::spawn(|| {
                std::thread::sleep(std::time::Duration::from_secs(2));
                std::process::exit(0);
            });
            CommandOutcome::ok(serde_json::json!({ "updated": true, "previous_version": env!("CARGO_PKG_VERSION") }).to_string())
        }
        Ok(Ok(false)) => CommandOutcome::ok(
            serde_json::json!({ "updated": false, "current_version": env!("CARGO_PKG_VERSION") })
                .to_string(),
        ),
        Ok(Err(e)) => CommandOutcome::failed(format!("update check failed: {e}")),
        Err(e) => CommandOutcome::failed(format!("update task panicked: {e}")),
    }
}
