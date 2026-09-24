//! Ruft den (inoffiziellen) Nutzungs-Endpunkt ab und bringt beide bekannten
//! Antwortformate auf ein einheitliches Modell.
//!
//! Der Endpunkt ist undokumentiert und kann sich jederzeit ändern. Deshalb:
//! Unbekanntes wird übersprungen statt zu einem Absturz zu führen, und wenn
//! am Ende nichts Brauchbares übrig bleibt, gibt es eine klare Meldung.

use serde::{Deserialize, Serialize};

use crate::credentials;

const USAGE_URL: &str = "https://api.anthropic.com/api/oauth/usage";
const BETA_HEADER: &str = "oauth-2025-04-20";
const USER_AGENT: &str = concat!("claude-usage-menubar/", env!("CARGO_PKG_VERSION"));
const TIMEOUT_SECONDS: u64 = 15;

/// `resets_at` kommt laut Plan als ISO-8601-String oder als Unix-Zeit.
/// Beides wird unverändert durchgereicht; das Frontend kann beides lesen.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ResetsAt {
    Text(String),
    Epoch(f64),
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Limit {
    /// Rohe Art aus der Antwort ("session", "weekly_all", "five_hour", …).
    /// Nötig, um die Session-Zeile zuverlässig zu finden, ohne auf
    /// Anzeigetexte zu matchen — die Reihenfolge im Array ist nicht garantiert.
    pub kind: String,
    pub label: String,
    pub percent: f64,
    pub resets_at: Option<ResetsAt>,
    /// Vom Server mitgelieferte Einstufung ("normal", …), falls vorhanden.
    /// Wird in M4 für die Balkenfarbe genutzt, mit den festen Schwellen als Rückfall.
    pub severity: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct UsageData {
    pub limits: Vec<Limit>,
    pub fetched_at: String,
    /// `true`, wenn die Werte aus einem früheren Abruf stammen, weil der
    /// aktuelle fehlgeschlagen ist. Alte Zahlen zu zeigen ist besser als gar
    /// keine — aber sie müssen als alt erkennbar sein.
    pub stale: bool,
    /// Grund für `stale`, zur Anzeige im Fenster.
    pub notice: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum Payload {
    Ok { ok: bool, data: UsageData },
    Err { ok: bool, error: String },
}

impl Payload {
    pub fn success(limits: Vec<Limit>) -> Self {
        Payload::Ok {
            ok: true,
            data: UsageData {
                limits,
                fetched_at: chrono::Utc::now().to_rfc3339(),
                stale: false,
                notice: None,
            },
        }
    }

    /// Ein früherer Stand, als veraltet gekennzeichnet. `fetched_at` bleibt
    /// bewusst der alte Zeitpunkt — er sagt aus, wie alt die Zahlen sind.
    pub fn stale(previous: UsageData, reason: impl Into<String>) -> Self {
        Payload::Ok {
            ok: true,
            data: UsageData {
                stale: true,
                notice: Some(reason.into()),
                ..previous
            },
        }
    }

    pub fn failure(error: impl Into<String>) -> Self {
        Payload::Err {
            ok: false,
            error: error.into(),
        }
    }
}

// ---------------------------------------------------------------------------
// Rohformate
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct RawLimit {
    kind: String,
    /// Kommt im `limits`-Array als Integer, im flachen Format als Float.
    /// `f64` liest beides.
    percent: f64,
    #[serde(default)]
    resets_at: Option<ResetsAt>,
    #[serde(default)]
    scope: Option<RawScope>,
    #[serde(default)]
    severity: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawScope {
    #[serde(default)]
    model: Option<RawModel>,
}

#[derive(Debug, Deserialize)]
struct RawModel {
    #[serde(default)]
    display_name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FlatEntry {
    utilization: f64,
    #[serde(default)]
    resets_at: Option<ResetsAt>,
}

/// Whitelist für das flache Format, in Anzeigereihenfolge.
///
/// Bewusst eine Whitelist: die Antwort enthält zusätzlich rund zwei Dutzend
/// interne Codenamen-Keys (`tangelo`, `nimbus_quill`, `copper_kite`, …), die
/// nichts im Fenster zu suchen haben. Siehe Befund zu M0 in docs/PLAN.md.
const FLAT_KEYS: &[(&str, &str)] = &[
    ("five_hour", "Session (5 Std.)"),
    ("seven_day", "Woche (alle Modelle)"),
    ("seven_day_sonnet", "Woche (Sonnet)"),
    ("seven_day_opus", "Woche (Opus)"),
];

// ---------------------------------------------------------------------------
// Normalisierung
// ---------------------------------------------------------------------------

/// Wandelt eine Antwort in die einheitliche Limit-Liste um.
///
/// Zuerst wird das `limits`-Array ausgewertet, sonst greift das flache Format.
/// Eine leere Liste gilt als Formatänderung und wird zum Fehler.
pub fn normalize(body: &str) -> Result<Vec<Limit>, String> {
    let root: serde_json::Value = serde_json::from_str(body).map_err(|_| {
        "Die Antwort war kein gültiges JSON. Das Format hat sich vermutlich geändert."
            .to_string()
    })?;

    let mut limits = from_limits_array(&root);
    if limits.is_empty() {
        limits = from_flat_keys(&root);
    }

    if limits.is_empty() {
        return Err("Es liessen sich keine Limits aus der Antwort lesen. \
                    Das Format hat sich vermutlich geändert."
            .to_string());
    }

    Ok(limits)
}

fn from_limits_array(root: &serde_json::Value) -> Vec<Limit> {
    let Some(entries) = root.get("limits").and_then(|v| v.as_array()) else {
        return Vec::new();
    };

    entries
        .iter()
        .filter_map(|entry| serde_json::from_value::<RawLimit>(entry.clone()).ok())
        .map(|raw| Limit {
            label: label_for_kind(&raw.kind, raw.scope.as_ref()),
            kind: raw.kind,
            percent: raw.percent,
            resets_at: raw.resets_at,
            severity: raw.severity,
        })
        .collect()
}

fn label_for_kind(kind: &str, scope: Option<&RawScope>) -> String {
    match kind {
        "session" => "Session".to_string(),
        "weekly_all" => "Woche (alle Modelle)".to_string(),
        "weekly_scoped" => {
            let model = scope
                .and_then(|s| s.model.as_ref())
                .and_then(|m| m.display_name.as_deref())
                .unwrap_or("Modell");
            format!("Woche ({model})")
        }
        // Unbekannte Arten werden absichtlich mitgenommen statt verworfen:
        // ein neu eingeführtes Limit soll sichtbar sein, nicht stillschweigend fehlen.
        other => other.to_string(),
    }
}

fn from_flat_keys(root: &serde_json::Value) -> Vec<Limit> {
    FLAT_KEYS
        .iter()
        .filter_map(|(key, label)| {
            let value = root.get(*key)?;
            if value.is_null() {
                return None; // `null`-Einträge überspringen
            }
            let entry: FlatEntry = serde_json::from_value(value.clone()).ok()?;
            Some(Limit {
                kind: (*key).to_string(),
                label: (*label).to_string(),
                percent: entry.utilization,
                resets_at: entry.resets_at,
                severity: None,
            })
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Abruf
// ---------------------------------------------------------------------------

/// Ergebnis eines Abrufs. `rate_limited` steuert den Backoff in der
/// Poll-Schleife — dafür den Fehlertext zu parsen wäre zu brüchig.
pub struct Fetched {
    pub payload: Payload,
    pub rate_limited: bool,
}

struct FetchError {
    message: String,
    rate_limited: bool,
}

impl From<String> for FetchError {
    fn from(message: String) -> Self {
        FetchError { message, rate_limited: false }
    }
}

/// Prozentwert der Session-Zeile für den Titel in der Menüleiste.
/// Fällt auf das erste Limit zurück, falls keine Session-Art dabei ist.
pub fn session_percent(limits: &[Limit]) -> Option<f64> {
    limits
        .iter()
        .find(|l| matches!(l.kind.as_str(), "session" | "five_hour"))
        .or_else(|| limits.first())
        .map(|l| l.percent)
}

/// Einmaliger Abruf. Gibt immer ein `Payload` zurück, wirft nie.
pub async fn fetch_usage() -> Fetched {
    match fetch_usage_inner().await {
        Ok(limits) => Fetched {
            payload: Payload::success(limits),
            rate_limited: false,
        },
        Err(e) => Fetched {
            payload: Payload::failure(e.message),
            rate_limited: e.rate_limited,
        },
    }
}

async fn fetch_usage_inner() -> Result<Vec<Limit>, FetchError> {
    let token = credentials::load_access_token()?;

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(TIMEOUT_SECONDS))
        .build()
        .map_err(|e| format!("HTTP-Client liess sich nicht erstellen: {e}"))?;

    let response = client
        .get(USAGE_URL)
        .header("Authorization", format!("Bearer {}", token.expose()))
        .header("anthropic-beta", BETA_HEADER)
        .header("User-Agent", USER_AGENT)
        .send()
        .await
        .map_err(|e| {
            if e.is_timeout() {
                "Zeitüberschreitung bei der Anfrage. Besteht eine Internetverbindung?".to_string()
            } else if e.is_connect() {
                "Keine Verbindung zu api.anthropic.com. Besteht eine Internetverbindung?"
                    .to_string()
            } else {
                "Die Anfrage ist fehlgeschlagen. Besteht eine Internetverbindung?".to_string()
            }
        })?;

    // Token ab hier nicht mehr nötig.
    drop(token);

    let status = response.status();
    if !status.is_success() {
        return Err(FetchError {
            message: message_for_status(status.as_u16()),
            rate_limited: status.as_u16() == 429,
        });
    }

    let body = response
        .text()
        .await
        .map_err(|_| "Die Antwort liess sich nicht lesen.".to_string())?;

    normalize(&body).map_err(FetchError::from)
}

fn message_for_status(status: u16) -> String {
    match status {
        401 | 403 => "Die Anmeldung ist abgelaufen. Öffne kurz Claude Code, \
                      dann erneuert sich die Anmeldung von selbst."
            .to_string(),
        429 => "Das Abruflimit des Endpunkts greift gerade. Er erlaubt nur etwa \
                einen Abruf alle paar Minuten. Die App versucht es von selbst \
                erneut — „Aktualisieren“ hilft jetzt nicht."
            .to_string(),
        500..=599 => format!("Der Dienst antwortet gerade nicht (Status {status})."),
        other => format!("Unerwartete Antwort vom Server (Status {other})."),
    }
}

// ---------------------------------------------------------------------------
// Tests (ohne Netzwerk)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const NEUES_FORMAT: &str = r#"{
      "limits": [
        { "kind": "session", "percent": 42.0, "resets_at": "2026-09-24T18:00:00+00:00" },
        { "kind": "weekly_all", "percent": 13.0, "resets_at": "2026-09-26T12:00:00+00:00" },
        { "kind": "weekly_scoped", "percent": 20.0, "resets_at": "2026-09-26T12:00:00+00:00",
          "scope": { "model": { "display_name": "Opus" } } }
      ]
    }"#;

    const ALTES_FORMAT: &str = r#"{
      "five_hour":        { "utilization": 35.0, "resets_at": "2026-02-06T22:00:00+00:00" },
      "seven_day":        { "utilization": 14.0, "resets_at": "2026-02-10T22:00:00+00:00" },
      "seven_day_sonnet": { "utilization": 39.0, "resets_at": "2026-02-10T22:00:00+00:00" },
      "seven_day_opus":   null
    }"#;

    /// Gekürzte, aber strukturgetreue echte Antwort aus dem M0-Test:
    /// beide Formate gleichzeitig, dazu unbekannte Codenamen-Keys.
    const ECHTE_ANTWORT: &str = r#"{
      "five_hour": { "utilization": 10.0, "resets_at": "2026-09-24T13:50:00.681785+00:00" },
      "seven_day": { "utilization": 1.0, "resets_at": "2026-09-26T12:00:00.681806+00:00" },
      "seven_day_opus": null,
      "seven_day_sonnet": null,
      "tangelo": null,
      "iguana_necktie": null,
      "nimbus_quill": { "utilization": 0.0, "resets_at": null },
      "copper_kite": null,
      "limits": [
        { "kind": "session", "group": "session", "percent": 10, "severity": "normal",
          "resets_at": "2026-09-24T13:50:00.681785+00:00", "scope": null, "is_active": true },
        { "kind": "weekly_all", "group": "weekly", "percent": 1, "severity": "normal",
          "resets_at": "2026-09-26T12:00:00.681806+00:00", "scope": null, "is_active": false }
      ],
      "spend": { "percent": 9 }
    }"#;

    #[test]
    fn neues_format_wird_gelesen() {
        let limits = normalize(NEUES_FORMAT).expect("muss lesbar sein");
        assert_eq!(limits.len(), 3);
        assert_eq!(limits[0].label, "Session");
        assert_eq!(limits[0].percent, 42.0);
        assert_eq!(limits[1].label, "Woche (alle Modelle)");
        assert_eq!(limits[2].label, "Woche (Opus)");
        assert_eq!(
            limits[0].resets_at,
            Some(ResetsAt::Text("2026-09-24T18:00:00+00:00".to_string()))
        );
    }

    #[test]
    fn altes_format_wird_gelesen_und_null_uebersprungen() {
        let limits = normalize(ALTES_FORMAT).expect("muss lesbar sein");
        // seven_day_opus ist null und faellt weg
        assert_eq!(limits.len(), 3);
        assert_eq!(limits[0].label, "Session (5 Std.)");
        assert_eq!(limits[0].percent, 35.0);
        assert_eq!(limits[1].label, "Woche (alle Modelle)");
        assert_eq!(limits[2].label, "Woche (Sonnet)");
        assert!(limits.iter().all(|l| l.label != "Woche (Opus)"));
    }

    #[test]
    fn echte_antwort_nutzt_das_limits_array_und_ignoriert_codenamen() {
        let limits = normalize(ECHTE_ANTWORT).expect("muss lesbar sein");
        assert_eq!(limits.len(), 2, "nur die Eintraege aus limits[]");
        assert_eq!(limits[0].label, "Session");
        assert_eq!(limits[0].percent, 10.0, "Integer muss als f64 ankommen");
        assert_eq!(limits[0].severity.as_deref(), Some("normal"));
        assert_eq!(limits[1].label, "Woche (alle Modelle)");
        // Die internen Codenamen duerfen nirgends auftauchen.
        for verboten in ["tangelo", "nimbus_quill", "copper_kite", "iguana_necktie", "spend"] {
            assert!(
                limits.iter().all(|l| l.label != verboten),
                "{verboten} darf nicht als Limit erscheinen"
            );
        }
    }

    #[test]
    fn flacher_fallback_ignoriert_unbekannte_keys() {
        // Kein limits-Array -> Fallback greift, Codenamen bleiben trotzdem draussen.
        let body = r#"{
          "five_hour": { "utilization": 7.5, "resets_at": 1760000000 },
          "nimbus_quill": { "utilization": 99.0, "resets_at": null },
          "tangelo": { "utilization": 50.0, "resets_at": null }
        }"#;
        let limits = normalize(body).expect("muss lesbar sein");
        assert_eq!(limits.len(), 1);
        assert_eq!(limits[0].label, "Session (5 Std.)");
        assert_eq!(limits[0].resets_at, Some(ResetsAt::Epoch(1760000000.0)));
    }

    #[test]
    fn unbekannte_art_im_array_bleibt_sichtbar() {
        let body = r#"{ "limits": [ { "kind": "monthly_new", "percent": 5 } ] }"#;
        let limits = normalize(body).expect("muss lesbar sein");
        assert_eq!(limits.len(), 1);
        assert_eq!(limits[0].label, "monthly_new");
        assert_eq!(limits[0].resets_at, None);
    }

    #[test]
    fn kind_wird_in_beiden_formaten_mitgefuehrt() {
        let neu = normalize(NEUES_FORMAT).unwrap();
        assert_eq!(neu[0].kind, "session");
        assert_eq!(neu[2].kind, "weekly_scoped");

        let alt = normalize(ALTES_FORMAT).unwrap();
        assert_eq!(alt[0].kind, "five_hour");
        assert_eq!(alt[1].kind, "seven_day");
    }

    #[test]
    fn session_percent_findet_die_richtige_zeile() {
        // Session steht hier bewusst NICHT an erster Stelle.
        let body = r#"{ "limits": [
            { "kind": "weekly_all", "percent": 13 },
            { "kind": "session", "percent": 42 }
        ] }"#;
        let limits = normalize(body).unwrap();
        assert_eq!(session_percent(&limits), Some(42.0));

        // Flaches Format: five_hour zaehlt als Session.
        let alt = normalize(ALTES_FORMAT).unwrap();
        assert_eq!(session_percent(&alt), Some(35.0));

        // Ohne Session-Art: Rueckfall auf das erste Limit.
        let ohne = normalize(r#"{ "limits": [ { "kind": "weekly_all", "percent": 7 } ] }"#).unwrap();
        assert_eq!(session_percent(&ohne), Some(7.0));

        assert_eq!(session_percent(&[]), None);
    }

    #[test]
    fn leere_antwort_ergibt_formatfehler() {
        let err = normalize(r#"{ "limits": [], "tangelo": null }"#).unwrap_err();
        assert!(err.contains("Format"), "unerwartete Meldung: {err}");
    }

    #[test]
    fn kaputtes_json_ergibt_formatfehler() {
        let err = normalize("kein json").unwrap_err();
        assert!(err.contains("Format"), "unerwartete Meldung: {err}");
    }

    #[test]
    fn stale_behaelt_werte_und_zeitpunkt_und_nennt_den_grund() {
        let frisch = Payload::success(vec![Limit {
            kind: "session".to_string(),
            label: "Session".to_string(),
            percent: 42.0,
            resets_at: None,
            severity: None,
        }]);

        let Payload::Ok { data: vorher, .. } = frisch else {
            panic!("success muss Ok liefern");
        };
        let zeitpunkt = vorher.fetched_at.clone();

        let veraltet = Payload::stale(vorher, "Keine Verbindung zu api.anthropic.com.");
        let Payload::Ok { data, .. } = veraltet else {
            panic!("stale muss Ok liefern, damit die Zahlen sichtbar bleiben");
        };

        assert!(data.stale);
        assert_eq!(data.limits[0].percent, 42.0, "Werte bleiben erhalten");
        assert_eq!(
            data.fetched_at, zeitpunkt,
            "der alte Zeitpunkt muss stehen bleiben — er sagt, wie alt die Zahlen sind"
        );
        assert_eq!(
            data.notice.as_deref(),
            Some("Keine Verbindung zu api.anthropic.com.")
        );
    }

    #[test]
    fn frischer_abruf_ist_nicht_stale() {
        let p = Payload::success(vec![]);
        let Payload::Ok { data, .. } = p else { panic!() };
        assert!(!data.stale);
        assert!(data.notice.is_none());
    }

    #[test]
    fn statusmeldungen_sind_verstaendlich() {
        assert!(message_for_status(401).contains("Claude Code"));
        assert!(message_for_status(429).contains("Abruflimit"));
        assert!(message_for_status(503).contains("503"));
        assert!(message_for_status(418).contains("418"));
    }

    #[test]
    fn payload_serialisiert_wie_im_datenmodell() {
        let ok = Payload::success(vec![Limit {
            kind: "session".to_string(),
            label: "Session".to_string(),
            percent: 42.0,
            resets_at: Some(ResetsAt::Text("2026-09-24T18:00:00+00:00".to_string())),
            severity: Some("normal".to_string()),
        }]);
        let json: serde_json::Value = serde_json::to_value(&ok).unwrap();
        assert_eq!(json["ok"], serde_json::json!(true));
        assert_eq!(json["data"]["limits"][0]["label"], "Session");
        assert!(json["data"]["fetched_at"].is_string());

        let err = Payload::failure("kaputt");
        let json: serde_json::Value = serde_json::to_value(&err).unwrap();
        assert_eq!(json["ok"], serde_json::json!(false));
        assert_eq!(json["error"], "kaputt");
    }
}
