//! Symbol in der Menüleiste: Titel mit der Session-Auslastung, Linksklick
//! zeigt/versteckt das Popover, Rechtsklick öffnet das Menü.

use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::AppHandle;

pub const TRAY_ID: &str = "main";

/// Titel, solange noch kein Abruf durch ist.
const TITLE_PENDING: &str = "…";
/// Titel bei jedem Fehler. `⚠︎` ist schmal und fällt trotzdem auf.
pub const TITLE_ERROR: &str = "⚠︎";

pub fn build(app: &AppHandle) -> tauri::Result<()> {
    let refresh = MenuItem::with_id(app, "refresh", "Aktualisieren", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Beenden", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&refresh, &quit])?;

    // Template-Icon: rein schwarz mit Alpha. macOS färbt es selbst um,
    // damit es in heller wie dunkler Menüleiste sitzt.
    let icon = tauri::image::Image::from_bytes(include_bytes!("../icons/tray-template.png"))?;

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon)
        .icon_as_template(true)
        .menu(&menu)
        .title(TITLE_PENDING)
        // Voreinstellung ist `true` — damit würde der Linksklick das Menü
        // öffnen statt das Fenster zu zeigen.
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "refresh" => {
                let handle = app.clone();
                tauri::async_runtime::spawn(async move {
                    let fetched = crate::usage::fetch_usage().await;
                    crate::handle_result(&handle, fetched);
                });
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            // Der Positioner merkt sich hier die Lage des Symbols.
            tauri_plugin_positioner::on_tray_event(tray.app_handle(), &event);

            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                crate::toggle_window(tray.app_handle());
            }
        })
        .build(app)?;

    Ok(())
}

/// Setzt den Text in der Menüleiste. Läuft ins Leere, falls das Icon
/// (noch) nicht existiert — kein Grund, die App zu beenden.
pub fn set_title(app: &AppHandle, text: &str) {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let _ = tray.set_title(Some(text));
    }
}
