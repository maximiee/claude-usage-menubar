//! Liest das OAuth-Token von Claude Code aus dem macOS-Schlüsselbund.
//!
//! Sicherheitsregeln (siehe docs/PLAN.md):
//! - Das Token wird nur im Speicher gehalten, nie auf die Platte geschrieben,
//!   nie geloggt und nie ans Frontend geschickt.
//! - Kein eigener Token-Refresh. Claude Code rotiert das Token selbst.

use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

/// Name des Schlüsselbund-Eintrags. Zentral hier, damit eine Änderung
/// seitens Claude Code nur an einer Stelle nachgezogen werden muss.
const KEYCHAIN_SERVICE: &str = "Claude Code-credentials";

/// Hülle um das Token, die ein versehentliches Loggen verhindert.
/// `Debug` gibt bewusst nur einen Platzhalter aus.
pub struct Secret(String);

impl Secret {
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Secret(***)")
    }
}

/// Holt das Token aus dem Schlüsselbund und prüft das Ablaufdatum.
/// Fehlertexte sind bewusst auf Deutsch und sagen, was zu tun ist.
pub fn load_access_token() -> Result<Secret, String> {
    let raw = read_keychain_entry()?;
    let parsed: serde_json::Value = serde_json::from_str(&raw)
        .map_err(|_| "Der Schlüsselbund-Eintrag enthält kein gültiges JSON. \
                      Melde dich in Claude Code neu an.".to_string())?;

    let oauth = parsed.get("claudeAiOauth").ok_or_else(|| {
        "Im Schlüsselbund fehlt der Abschnitt „claudeAiOauth“. Das Format hat sich \
         vermutlich geändert.".to_string()
    })?;

    // Ablaufdatum prüfen, bevor wir eine Anfrage verschwenden.
    if let Some(expires_at_ms) = oauth.get("expiresAt").and_then(|v| v.as_f64()) {
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as f64)
            .unwrap_or(0.0);
        if now_ms > 0.0 && expires_at_ms <= now_ms {
            return Err("Die Anmeldung ist abgelaufen. Öffne kurz Claude Code, \
                        dann erneuert sich die Anmeldung von selbst.".to_string());
        }
    }

    let token = oauth
        .get("accessToken")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            "Im Schlüsselbund fehlt „accessToken“. Melde dich in Claude Code neu an.".to_string()
        })?;

    if token.is_empty() {
        return Err("Das gespeicherte Token ist leer. Melde dich in Claude Code neu an.".to_string());
    }

    Ok(Secret(token.to_string()))
}

/// Ruft `/usr/bin/security` auf und übersetzt die bekannten Fehlerfälle.
fn read_keychain_entry() -> Result<String, String> {
    let output = Command::new("/usr/bin/security")
        .args(["find-generic-password", "-s", KEYCHAIN_SERVICE, "-w"])
        .output()
        .map_err(|e| format!("„/usr/bin/security“ liess sich nicht starten: {e}"))?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if stdout.is_empty() {
            return Err("Der Schlüsselbund-Eintrag ist leer. Melde dich in Claude Code neu an."
                .to_string());
        }
        return Ok(stdout);
    }

    // stderr enthält nie das Token, nur die Fehlermeldung von `security`.
    let stderr = String::from_utf8_lossy(&output.stderr).to_lowercase();
    if stderr.contains("could not be found") || output.status.code() == Some(44) {
        Err("Kein Claude-Code-Login gefunden. Starte Claude Code im Terminal und \
             melde dich mit deinem Abo an (nicht mit einem API-Schlüssel).".to_string())
    } else if stderr.contains("user interaction is not allowed")
        || stderr.contains("user canceled")
        || stderr.contains("interaction not allowed")
    {
        Err("Der Zugriff auf den Schlüsselbund wurde abgelehnt. Erlaube ihn beim nächsten \
             Nachfragen, oder gib den Zugriff in der Schlüsselbundverwaltung frei: Eintrag \
             „Claude Code-credentials“ → Zugriff → diese App hinzufügen.".to_string())
    } else {
        Err(format!(
            "Der Schlüsselbund liess sich nicht lesen (Code {}).",
            output.status.code().unwrap_or(-1)
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_verraet_sich_nicht_im_debug_output() {
        let s = Secret("geheim-abc-123".to_string());
        assert_eq!(format!("{s:?}"), "Secret(***)");
        assert!(!format!("{s:?}").contains("geheim"));
        assert_eq!(s.expose(), "geheim-abc-123");
    }
}
