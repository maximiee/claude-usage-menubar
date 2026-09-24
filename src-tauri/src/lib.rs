mod credentials;
mod notify;
mod tray;
mod usage;

use std::sync::Mutex;
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager, WindowEvent};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_positioner::{Position, WindowExt};

use usage::{Payload, UsageData};

/// Nicht unter 5 Minuten — siehe Sicherheitsregeln in docs/PLAN.md.
const POLL_INTERVAL: Duration = Duration::from_secs(5 * 60);
/// Obergrenze für den Backoff nach HTTP 429.
const POLL_INTERVAL_MAX: Duration = Duration::from_secs(30 * 60);

/// Letztes Ergebnis, damit ein frisch geöffnetes Fenster sofort etwas anzeigt.
#[derive(Default)]
struct AppState {
    /// Was zuletzt angezeigt wurde — kann auch ein Fehler sein.
    last: Mutex<Option<Payload>>,
    /// Letzter *erfolgreicher* Abruf. Überlebt Fehler, damit bei Netzproblemen
    /// weiter Zahlen im Fenster stehen statt einer leeren Fläche.
    last_good: Mutex<Option<UsageData>>,
    /// Welche Schwellen in welchem Reset-Fenster schon gemeldet wurden.
    gemeldet: notify::Gemeldet,
}

/// Verarbeitet einen Abruf. Schlägt er fehl und liegt ein früherer guter
/// Stand vor, wird dieser als veraltet markiert weitergezeigt, statt die
/// Anzeige durch eine Fehlermeldung zu ersetzen.
pub(crate) fn handle_result(app: &AppHandle, fetched: usage::Fetched) {
    let payload = match &fetched.payload {
        Payload::Ok { data, .. } => {
            if let Some(state) = app.try_state::<AppState>() {
                if let Ok(mut guard) = state.last_good.lock() {
                    *guard = Some(data.clone());
                }
                notify::melde(app, &state.gemeldet, &data.limits);
            }
            fetched.payload
        }
        Payload::Err { error, .. } => {
            let vorher = app
                .try_state::<AppState>()
                .and_then(|state| state.last_good.lock().ok().and_then(|g| g.clone()));
            match vorher {
                Some(data) => Payload::stale(data, error.clone()),
                None => fetched.payload,
            }
        }
    };

    apply(app, &payload);
}

/// Übernimmt ein Ergebnis: Cache aktualisieren, Menüleiste beschriften,
/// Frontend benachrichtigen.
pub(crate) fn apply(app: &AppHandle, payload: &Payload) {
    if let Some(state) = app.try_state::<AppState>() {
        if let Ok(mut guard) = state.last.lock() {
            *guard = Some(payload.clone());
        }
    }
    tray::set_title(app, &tray_title(payload));
    let _ = app.emit("usage-updated", payload.clone());
}

/// Linksklick aufs Symbol: Popover zeigen oder verstecken.
pub(crate) fn toggle_window(app: &AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };

    if window.is_visible().unwrap_or(false) {
        let _ = window.hide();
    } else {
        // Vor dem Einblenden unter das Symbol schieben, sonst blitzt das
        // Fenster kurz an der alten Stelle auf.
        let _ = window.move_window(Position::TrayBottomCenter);
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn tray_title(payload: &Payload) -> String {
    match payload {
        Payload::Ok { data, .. } => match usage::session_percent(&data.limits) {
            Some(percent) => format!("{} %", percent.round() as i64),
            None => tray::TITLE_ERROR.to_string(),
        },
        Payload::Err { .. } => tray::TITLE_ERROR.to_string(),
    }
}

/// Sofort abrufen, danach alle 5 Minuten. Nach einem 429 wird das Intervall
/// verdoppelt (höchstens 30 Minuten) und bei Erfolg wieder zurückgesetzt.
fn spawn_poller(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut interval = POLL_INTERVAL;
        loop {
            let fetched = usage::fetch_usage().await;
            interval = if fetched.rate_limited {
                (interval * 2).min(POLL_INTERVAL_MAX)
            } else {
                POLL_INTERVAL
            };
            handle_result(&app, fetched);
            tokio::time::sleep(interval).await;
        }
    });
}

/// Läuft die App beim Login mit?
#[tauri::command]
fn get_autostart(app: AppHandle) -> bool {
    app.autolaunch().is_enabled().unwrap_or(false)
}

/// Autostart ein- oder ausschalten. Gibt den Zustand zurück, der danach
/// tatsächlich gilt — nicht den gewünschten.
#[tauri::command]
fn set_autostart(app: AppHandle, aktiv: bool) -> Result<bool, String> {
    let starter = app.autolaunch();
    let ergebnis = if aktiv { starter.enable() } else { starter.disable() };
    ergebnis.map_err(|e| format!("Autostart liess sich nicht ändern: {e}"))?;
    Ok(starter.is_enabled().unwrap_or(aktiv))
}

/// Zwischengespeichertes Ergebnis, ohne neuen Abruf.
#[tauri::command]
fn get_last(state: tauri::State<'_, AppState>) -> Option<Payload> {
    state.last.lock().ok().and_then(|guard| guard.clone())
}

/// Abruf auf Knopfdruck.
#[tauri::command]
async fn refresh_now(app: AppHandle) -> Option<Payload> {
    let fetched = usage::fetch_usage().await;
    handle_result(&app, fetched);
    // Den Stand zurückgeben, der tatsächlich angezeigt wird — der kann
    // dank stale-Rückfall von dem abweichen, was der Abruf lieferte.
    app.try_state::<AppState>()
        .and_then(|state| state.last.lock().ok().and_then(|g| g.clone()))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_positioner::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            get_last,
            refresh_now,
            get_autostart,
            set_autostart
        ])
        .on_window_event(|window, event| {
            // Schliessen beendet die App nicht, es versteckt nur das Popover.
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .setup(|app| {
            // Kein Dock-Symbol: die App lebt ausschliesslich in der Menüleiste.
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            tray::build(app.handle())?;
            spawn_poller(app.handle().clone());
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;
    use usage::{Limit, Payload};

    fn limit(kind: &str, percent: f64) -> Limit {
        Limit {
            kind: kind.to_string(),
            label: kind.to_string(),
            percent,
            resets_at: None,
            severity: None,
        }
    }

    #[test]
    fn titel_zeigt_gerundete_session_prozent() {
        let p = Payload::success(vec![limit("weekly_all", 13.0), limit("session", 41.6)]);
        assert_eq!(tray_title(&p), "42 %");
    }

    #[test]
    fn titel_zeigt_warnzeichen_bei_fehler() {
        assert_eq!(tray_title(&Payload::failure("kaputt")), tray::TITLE_ERROR);
    }

    #[test]
    fn titel_zeigt_warnzeichen_ohne_limits() {
        assert_eq!(tray_title(&Payload::success(vec![])), tray::TITLE_ERROR);
    }

    #[test]
    fn backoff_verdoppelt_bis_zur_obergrenze() {
        let mut interval = POLL_INTERVAL;
        for _ in 0..10 {
            interval = (interval * 2).min(POLL_INTERVAL_MAX);
        }
        assert_eq!(interval, POLL_INTERVAL_MAX);
        assert!(POLL_INTERVAL >= Duration::from_secs(5 * 60));
    }
}
