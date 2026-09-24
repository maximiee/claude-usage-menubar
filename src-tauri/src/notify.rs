//! macOS-Benachrichtigungen bei hoher Auslastung.
//!
//! Regel aus dem Plan: je Schwelle **einmal pro Fenster**. „Fenster“ heisst
//! hier der Zeitraum bis zum nächsten Reset — steht ein neues `resets_at` an,
//! darf erneut gemeldet werden.

use std::collections::HashMap;
use std::sync::Mutex;

use tauri::AppHandle;
use tauri_plugin_notification::NotificationExt;

use crate::usage::{Limit, ResetsAt};

/// Schwellen in aufsteigender Reihenfolge.
const SCHWELLEN: [u8; 2] = [80, 95];

/// Merkt sich je (Limit, Schwelle), für welches Reset-Fenster schon
/// gemeldet wurde.
#[derive(Default)]
pub struct Gemeldet(Mutex<HashMap<String, String>>);

/// Kennzeichnung des aktuellen Fensters. Ohne `resets_at` bleibt nur ein
/// fester Wert — dann wird je Programmlauf genau einmal gemeldet.
fn fenster(limit: &Limit) -> String {
    match &limit.resets_at {
        Some(ResetsAt::Text(s)) => s.clone(),
        Some(ResetsAt::Epoch(n)) => n.to_string(),
        None => "ohne-reset".to_string(),
    }
}

/// Liefert die Meldungen, die für diesen Stand fällig sind, und merkt sie vor.
/// Getrennt vom Versand, damit die Logik ohne macOS testbar bleibt.
pub fn faellige_meldungen(gemeldet: &Gemeldet, limits: &[Limit]) -> Vec<(String, String)> {
    let Ok(mut karte) = gemeldet.0.lock() else {
        return Vec::new();
    };

    let mut raus = Vec::new();
    for limit in limits {
        let w = fenster(limit);
        for schwelle in SCHWELLEN {
            if limit.percent < f64::from(schwelle) {
                continue;
            }
            let schluessel = format!("{}|{}", limit.kind, schwelle);
            if karte.get(&schluessel) == Some(&w) {
                continue; // in diesem Fenster schon gemeldet
            }
            karte.insert(schluessel, w.clone());
            raus.push((
                format!("{} bei {} %", limit.label, limit.percent.round() as i64),
                match schwelle {
                    95 => "Das Limit ist fast erreicht.".to_string(),
                    _ => "Die Auslastung steigt.".to_string(),
                },
            ));
        }
    }
    raus
}

/// Versendet, was fällig ist. Fehler beim Versand sind kein Grund,
/// irgendetwas abzubrechen — Benachrichtigungen sind Beiwerk.
pub fn melde(app: &AppHandle, gemeldet: &Gemeldet, limits: &[Limit]) {
    for (titel, text) in faellige_meldungen(gemeldet, limits) {
        let _ = app
            .notification()
            .builder()
            .title(titel)
            .body(text)
            .show();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn limit(kind: &str, percent: f64, reset: Option<&str>) -> Limit {
        Limit {
            kind: kind.to_string(),
            label: kind.to_string(),
            percent,
            resets_at: reset.map(|r| ResetsAt::Text(r.to_string())),
            severity: None,
        }
    }

    #[test]
    fn unter_der_schwelle_passiert_nichts() {
        let g = Gemeldet::default();
        assert!(faellige_meldungen(&g, &[limit("session", 79.9, Some("A"))]).is_empty());
    }

    #[test]
    fn meldet_je_fenster_nur_einmal() {
        let g = Gemeldet::default();
        let l = vec![limit("session", 82.0, Some("A"))];

        assert_eq!(faellige_meldungen(&g, &l).len(), 1, "erste Meldung");
        assert!(faellige_meldungen(&g, &l).is_empty(), "keine Wiederholung");
    }

    #[test]
    fn neues_fenster_meldet_erneut() {
        let g = Gemeldet::default();
        assert_eq!(faellige_meldungen(&g, &[limit("session", 82.0, Some("A"))]).len(), 1);
        assert_eq!(
            faellige_meldungen(&g, &[limit("session", 82.0, Some("B"))]).len(),
            1,
            "nach dem Reset darf wieder gemeldet werden"
        );
    }

    #[test]
    fn beide_schwellen_melden_getrennt() {
        let g = Gemeldet::default();
        // Sprung von 0 auf 96 loest beide Schwellen auf einmal aus.
        assert_eq!(faellige_meldungen(&g, &[limit("session", 96.0, Some("A"))]).len(), 2);
        assert!(faellige_meldungen(&g, &[limit("session", 96.0, Some("A"))]).is_empty());
    }

    #[test]
    fn limits_werden_getrennt_gezaehlt() {
        let g = Gemeldet::default();
        let l = vec![
            limit("session", 82.0, Some("A")),
            limit("weekly_all", 82.0, Some("B")),
        ];
        assert_eq!(faellige_meldungen(&g, &l).len(), 2);
    }
}
