//! OpenDeck action for the FilmCraft command line interface.
//!
//! Arguments are stored as a JSON string array and sent directly to the CLI.
//! Deliberately never invoking a shell preserves paths, JSON parameters, and user input exactly.

use std::{path::PathBuf, process::Stdio, time::Duration};

use openaction::*;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use simplelog::{ColorChoice, Config, LevelFilter, TermLogger, TerminalMode};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::TcpStream,
    process::Command,
};

const DEFAULT_EXECUTABLE: &str = "filmcraft-cli";
const CATALOG_ARGUMENTS: &[&str] = &["commands", "--json"];
const DEFAULT_ARGUMENTS: &str = r#"["commands","--json"]"#;
const MAX_ARGUMENTS: usize = 128;
const MAX_ARGUMENT_BYTES: usize = 16 * 1024;
const MAX_CATALOG_BYTES: usize = 2 * 1024 * 1024;

#[derive(Clone, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
struct CommandSettings {
    /// CLI executable or absolute path. Empty uses the app's conventional CLI name.
    executable: String,
    /// JSON string array, e.g. ["exec", "timeline.play", "{}"].
    arguments: String,
    /// Optional directory in which to run the command.
    working_directory: String,
    /// A user-visible title shown before the optional command output.
    title: String,
    /// Keep stdout/stderr (trimmed) on the Stream Deck key after a successful run.
    show_output: bool,
    /// Wall-clock limit for a process. Zero uses 30 seconds; maximum is 300 seconds.
    timeout_seconds: u64,
}

impl Default for CommandSettings {
    fn default() -> Self {
        Self {
            executable: String::new(),
            arguments: DEFAULT_ARGUMENTS.to_string(),
            working_directory: String::new(),
            title: String::new(),
            show_output: false,
            timeout_seconds: 30,
        }
    }
}

impl CommandSettings {
    fn executable(&self) -> &str {
        let executable = self.executable.trim();
        if executable.is_empty() {
            DEFAULT_EXECUTABLE
        } else {
            executable
        }
    }

    fn timeout(&self) -> Duration {
        Duration::from_secs(if self.timeout_seconds == 0 {
            30
        } else {
            self.timeout_seconds.clamp(1, 300)
        })
    }

    fn title(&self) -> Option<String> {
        (!self.title.trim().is_empty()).then(|| self.title.trim().to_string())
    }
}

#[derive(Debug)]
enum ExecutionError {
    Configuration(String),
    Process(String),
    TimedOut(u64),
    Failed { code: Option<i32>, output: String },
}

impl std::fmt::Display for ExecutionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Configuration(message) | Self::Process(message) => f.write_str(message),
            Self::TimedOut(seconds) => write!(f, "Timed out after {seconds} seconds"),
            Self::Failed { code, output } => {
                write!(
                    f,
                    "FilmCraft CLI exited with {}",
                    code.map_or_else(|| "a signal".into(), |code| format!("code {code}"))
                )?;
                if !output.is_empty() {
                    write!(f, ": {output}")?;
                }
                Ok(())
            }
        }
    }
}

fn parse_arguments(raw: &str) -> Result<Vec<String>, ExecutionError> {
    let parsed: Value = serde_json::from_str(raw.trim()).map_err(|error| {
        ExecutionError::Configuration(format!(
            "Arguments must be a JSON array of strings: {error}"
        ))
    })?;
    let values = parsed.as_array().ok_or_else(|| {
        ExecutionError::Configuration("Arguments must be a JSON array of strings.".into())
    })?;
    if values.is_empty() {
        return Err(ExecutionError::Configuration(
            "Add at least one CLI argument, for example [\"commands\", \"--json\"].".into(),
        ));
    }
    if values.len() > MAX_ARGUMENTS {
        return Err(ExecutionError::Configuration(format!(
            "At most {MAX_ARGUMENTS} arguments are allowed."
        )));
    }
    values
        .iter()
        .map(|value| {
            let value = value.as_str().ok_or_else(|| {
                ExecutionError::Configuration(
                    "Every argument must be a string; numbers and JSON values must be quoted."
                        .into(),
                )
            })?;
            if value.len() > MAX_ARGUMENT_BYTES {
                return Err(ExecutionError::Configuration(format!(
                    "An argument is longer than {MAX_ARGUMENT_BYTES} bytes."
                )));
            }
            Ok(value.to_string())
        })
        .collect()
}

fn summary(stdout: &[u8], stderr: &[u8]) -> String {
    let output = if !stderr.is_empty() { stderr } else { stdout };
    let text = String::from_utf8_lossy(output).replace(['\n', '\r'], " ");
    let text = text.trim();
    let mut end = text.len().min(120);
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    let suffix = if text.len() > end { "…" } else { "" };
    format!("{}{}", &text[..end], suffix)
}

async fn execute(
    settings: &CommandSettings,
    arguments: Vec<String>,
) -> Result<String, ExecutionError> {
    let mut command = Command::new(settings.executable());
    command
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    if !settings.working_directory.trim().is_empty() {
        command.current_dir(PathBuf::from(settings.working_directory.trim()));
    }
    let seconds = settings.timeout().as_secs();
    let output = tokio::time::timeout(settings.timeout(), command.output())
        .await
        .map_err(|_| ExecutionError::TimedOut(seconds))?
        .map_err(|error| {
            ExecutionError::Process(format!(
                "Could not start {}: {error}",
                settings.executable()
            ))
        })?;
    let output_summary = summary(&output.stdout, &output.stderr);
    if output.status.success() {
        Ok(output_summary)
    } else {
        Err(ExecutionError::Failed {
            code: output.status.code(),
            output: output_summary,
        })
    }
}

async fn report(
    instance: &Instance,
    settings: &CommandSettings,
    result: Result<String, ExecutionError>,
    to_inspector: bool,
) {
    match result {
        Ok(output) => {
            let title = if settings.show_output && !output.is_empty() {
                output.clone()
            } else {
                settings.title().unwrap_or_else(|| "Done".to_string())
            };
            let _ = instance.set_title(Some(title), None).await;
            let _ = instance.show_ok().await;
            if to_inspector {
                let _ = instance
                    .send_to_property_inspector(
                        json!({"kind":"result", "ok":true, "message": output}),
                    )
                    .await;
            }
        }
        Err(error) => {
            let message = error.to_string();
            log::warn!("FilmCraft CLI action failed: {message}");
            let _ = instance.set_title(Some("Error".to_string()), None).await;
            let _ = instance.show_alert().await;
            if to_inspector {
                let _ = instance
                    .send_to_property_inspector(
                        json!({"kind":"result", "ok":false, "message": message}),
                    )
                    .await;
            }
        }
    }
}

async fn run_action(instance: &Instance, settings: &CommandSettings, to_inspector: bool) {
    let result = parse_arguments(&settings.arguments).and_then(|arguments| {
        // Validation above is synchronous; the process work stays on the async runtime.
        Ok(arguments)
    });
    let result = match result {
        Ok(arguments) => execute(settings, arguments).await,
        Err(error) => Err(error),
    };
    report(instance, settings, result, to_inspector).await;
}

fn catalog_items(stdout: &str) -> Vec<Value> {
    let Ok(value) = serde_json::from_str::<Value>(stdout) else {
        return Vec::new();
    };
    let items = value
        .as_array()
        .cloned()
        .or_else(|| value.get("commands").and_then(Value::as_array).cloned())
        .unwrap_or_default();
    items
        .into_iter()
        .filter_map(|item| {
            if let Some(id) = item.as_str() {
                return Some(json!({"id": id, "label": id}));
            }
            let id = ["id", "command", "tool", "name"]
                .into_iter()
                .find_map(|key| item.get(key).and_then(Value::as_str))?;
            let label = item
                .get("label")
                .or_else(|| item.get("name"))
                .and_then(Value::as_str)
                .unwrap_or(id);
            Some(json!({"id": id, "label": label}))
        })
        .collect()
}

async fn send_catalog(instance: &Instance, settings: &CommandSettings) {
    let mut command = Command::new(settings.executable());
    command
        .args(CATALOG_ARGUMENTS)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    if !settings.working_directory.trim().is_empty() {
        command.current_dir(PathBuf::from(settings.working_directory.trim()));
    }
    let result = tokio::time::timeout(Duration::from_secs(15), command.output()).await;
    let reply = match result {
        Ok(Ok(output)) if output.status.success() && output.stdout.len() <= MAX_CATALOG_BYTES => {
            let text = String::from_utf8_lossy(&output.stdout);
            let items = catalog_items(&text);
            if items.is_empty() {
                json!({"kind":"catalog", "ok":false, "message":"The installed CLI did not return a JSON command catalog."})
            } else {
                json!({"kind":"catalog", "ok":true, "items":items})
            }
        }
        Ok(Ok(output)) => {
            json!({"kind":"catalog", "ok":false, "message":format!("Could not read the command catalog: {}", summary(&output.stdout, &output.stderr))})
        }
        Ok(Err(error)) => {
            json!({"kind":"catalog", "ok":false, "message":format!("Could not start {}: {error}", settings.executable())})
        }
        Err(_) => {
            json!({"kind":"catalog", "ok":false, "message":"Timed out while loading the command catalog."})
        }
    };
    let _ = instance.send_to_property_inspector(reply).await;
}

/// Settings for a control that drives the FilmCraft window already running on this machine.
/// The control server is deliberately loopback-only, so an OpenDeck profile can never turn this
/// plugin into a network command runner.
#[derive(Clone, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
struct EditControlSettings {
    control_host: String,
    control_port: u16,
    key_action: String,
    press_action: String,
    dial_action: String,
    dial_step: f64,
    title: String,
}

impl Default for EditControlSettings {
    fn default() -> Self {
        Self {
            control_host: "127.0.0.1".into(),
            control_port: 9876,
            key_action: "playPause".into(),
            press_action: "shuttleStop".into(),
            dial_action: "jog".into(),
            dial_step: 1.0,
            title: "FilmCraft".into(),
        }
    }
}

impl EditControlSettings {
    fn host(&self) -> &str {
        let host = self.control_host.trim();
        if host.is_empty() {
            "127.0.0.1"
        } else {
            host
        }
    }

    fn step(&self) -> f64 {
        if self.dial_step.is_finite() {
            self.dial_step.abs().clamp(0.01, 100.0)
        } else {
            1.0
        }
    }
}

fn loopback_host(host: &str) -> bool {
    matches!(
        host.to_ascii_lowercase().as_str(),
        "127.0.0.1" | "::1" | "localhost"
    )
}

async fn film_control_call(
    settings: &EditControlSettings,
    method: &str,
    params: Value,
) -> Result<Value, String> {
    let host = settings.host();
    if !loopback_host(host) {
        return Err("The FilmCraft control host must be localhost, 127.0.0.1, or ::1.".into());
    }
    if settings.control_port == 0 {
        return Err("Set a valid FilmCraft control port (normally 9876).".into());
    }
    let stream = tokio::time::timeout(
        Duration::from_secs(3),
        TcpStream::connect((host, settings.control_port)),
    )
    .await
    .map_err(|_| "Timed out connecting to FilmCraft. Start it with --control 9876.".to_string())?
    .map_err(|error| format!("Could not connect to FilmCraft control: {error}"))?;
    let (reader, mut writer) = stream.into_split();
    let request = json!({"id": 1, "method": method, "params": params});
    writer
        .write_all(request.to_string().as_bytes())
        .await
        .map_err(|error| format!("Could not send FilmCraft control request: {error}"))?;
    writer
        .write_all(b"\n")
        .await
        .map_err(|error| format!("Could not finish FilmCraft control request: {error}"))?;
    let mut line = String::new();
    let read = tokio::time::timeout(
        Duration::from_secs(5),
        BufReader::new(reader).read_line(&mut line),
    )
    .await
    .map_err(|_| "FilmCraft did not answer the control request in time.".to_string())?
    .map_err(|error| format!("Could not read FilmCraft control response: {error}"))?;
    if read == 0 || line.len() > 128 * 1024 {
        return Err("FilmCraft returned an empty or oversized control response.".into());
    }
    let response: Value = serde_json::from_str(&line)
        .map_err(|error| format!("FilmCraft returned invalid control JSON: {error}"))?;
    if response.get("ok").and_then(Value::as_bool) == Some(true) {
        Ok(response.get("result").cloned().unwrap_or(Value::Null))
    } else {
        Err(response
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or("FilmCraft rejected the control request.")
            .to_string())
    }
}

fn engine(command: &'static str, params: Value) -> (&'static str, Value) {
    (
        "engine.execute",
        json!({"command": command, "params": params}),
    )
}

fn film_key_request(action: &str) -> Result<(&'static str, Value), String> {
    Ok(match action {
        "playPause" => ("ui.playback", json!({"action":"toggle"})),
        "cut" => engine("sequence.addEdit", json!({})),
        "rippleDelete" => engine("edit.rippleDelete", json!({})),
        "addMarker" => engine("markers.add", json!({})),
        "previousMarker" => engine("markers.goPrev", json!({})),
        "nextMarker" => engine("markers.goNext", json!({})),
        "previousEdit" => engine("playhead.prevEdit", json!({})),
        "nextEdit" => engine("playhead.nextEdit", json!({})),
        "shuttleForward" => (
            "ui.menu.invoke",
            json!({"id":"playback.forward", "params":{}}),
        ),
        "shuttleReverse" => (
            "ui.menu.invoke",
            json!({"id":"playback.reverse", "params":{}}),
        ),
        "shuttleStop" => ("ui.playback", json!({"action":"stop"})),
        "undo" => engine("edit.undo", json!({})),
        "redo" => engine("edit.redo", json!({})),
        other => return Err(format!("Unknown FilmCraft key action `{other}`.")),
    })
}

fn selected_lumetri_value(sequence: &Value, parameter: &str) -> Option<(u64, usize, f64)> {
    let selected = sequence.get("selection")?.as_array()?.first()?.as_u64()?;
    for track_type in ["video", "audio"] {
        for track in sequence.get(track_type)?.as_array()? {
            for item in track.get("items")?.as_array()? {
                if item.get("clip").and_then(Value::as_u64) != Some(selected) {
                    continue;
                }
                for (index, effect) in item.get("effects")?.as_array()?.iter().enumerate() {
                    if effect.get("effect").and_then(Value::as_str) != Some("lumetri") {
                        continue;
                    }
                    let value = effect
                        .get("params")?
                        .get(parameter)?
                        .get("value")?
                        .as_str()?;
                    let number = value
                        .strip_prefix("Float(")?
                        .strip_suffix(')')?
                        .parse::<f64>()
                        .ok()?;
                    return Some((selected, index, number));
                }
            }
        }
    }
    None
}

async fn film_dial_request(settings: &EditControlSettings, ticks: i16) -> Result<Value, String> {
    if ticks == 0 {
        return Ok(Value::Null);
    }
    let scaled = f64::from(ticks) * settings.step();
    match settings.dial_action.as_str() {
        "jog" => {
            let (method, params) =
                engine("playhead.step", json!({"frames": scaled.round() as i64}));
            film_control_call(settings, method, params).await
        }
        "shuttle" => {
            let speed = scaled.signum() * scaled.abs().ceil().clamp(1.0, 8.0);
            film_control_call(
                settings,
                "ui.playback",
                json!({"action":"play", "speed":speed}),
            )
            .await
        }
        "audioGain" => {
            film_control_call(
                settings,
                "engine.execute",
                json!({"command":"clip.audioGain", "params":{"mode":"adjust", "db":scaled}}),
            )
            .await
        }
        "exposure" | "contrast" | "saturation" => {
            let sequence = film_control_call(
                settings,
                "engine.execute",
                json!({"command":"sequence.inspect", "params":{}}),
            )
            .await?;
            let (clip, effect, current) =
                selected_lumetri_value(&sequence, settings.dial_action.as_str()).ok_or_else(
                    || {
                        format!(
                    "Select a clip with a Lumetri {} parameter before turning this encoder.",
                    settings.dial_action
                )
                    },
                )?;
            film_control_call(settings, "engine.execute", json!({"command":"effects.setParam", "params":{"clip":clip, "effect":effect, "param":settings.dial_action, "value":current + scaled, "merge":"opendeck-dial"}})).await
        }
        other => Err(format!("Unknown FilmCraft dial action `{other}`.")),
    }
}

async fn report_edit_result(
    instance: &Instance,
    title: &str,
    result: Result<Value, String>,
    inspector: bool,
) {
    match result {
        Ok(value) => {
            let _ = instance.set_title(Some(title.to_string()), None).await;
            let _ = instance.show_ok().await;
            if inspector {
                let _ = instance.send_to_property_inspector(json!({"kind":"result", "ok":true, "message":summary(value.to_string().as_bytes(), b"")})).await;
            }
        }
        Err(message) => {
            log::warn!("FilmCraft edit control failed: {message}");
            let _ = instance.set_title(Some("Error".to_string()), None).await;
            let _ = instance.show_alert().await;
            if inspector {
                let _ = instance
                    .send_to_property_inspector(
                        json!({"kind":"result", "ok":false, "message":message}),
                    )
                    .await;
            }
        }
    }
}

struct FilmCraftEditControl;

#[async_trait]
impl Action for FilmCraftEditControl {
    const UUID: &'static str = "wt.luizgustavo.getcraft.filmcraft.editControl";
    type Settings = EditControlSettings;

    async fn key_down(
        &self,
        instance: &Instance,
        settings: &Self::Settings,
    ) -> OpenActionResult<()> {
        let result = film_key_request(&settings.key_action)
            .and_then(|(method, params)| Ok((method, params)));
        let result = match result {
            Ok((method, params)) => film_control_call(settings, method, params).await,
            Err(error) => Err(error),
        };
        report_edit_result(instance, &settings.title, result, false).await;
        Ok(())
    }

    async fn dial_down(
        &self,
        instance: &Instance,
        settings: &Self::Settings,
    ) -> OpenActionResult<()> {
        let result = match film_key_request(&settings.press_action) {
            Ok((method, params)) => film_control_call(settings, method, params).await,
            Err(error) => Err(error),
        };
        report_edit_result(instance, &settings.title, result, false).await;
        Ok(())
    }

    async fn dial_rotate(
        &self,
        instance: &Instance,
        settings: &Self::Settings,
        ticks: i16,
        _pressed: bool,
    ) -> OpenActionResult<()> {
        let result = film_dial_request(settings, ticks).await;
        report_edit_result(instance, &settings.title, result, false).await;
        Ok(())
    }

    async fn will_appear(
        &self,
        instance: &Instance,
        settings: &Self::Settings,
    ) -> OpenActionResult<()> {
        let _ = instance.set_title(Some(settings.title.clone()), None).await;
        Ok(())
    }

    async fn send_to_plugin(
        &self,
        instance: &Instance,
        settings: &Self::Settings,
        payload: &Value,
    ) -> OpenActionResult<()> {
        let effective = payload
            .get("settings")
            .and_then(|value| serde_json::from_value::<EditControlSettings>(value.clone()).ok())
            .unwrap_or_else(|| settings.clone());
        if payload.get("request").and_then(Value::as_str) == Some("test") {
            let result = film_key_request(&effective.key_action)
                .and_then(|(method, params)| Ok((method, params)));
            let result = match result {
                Ok((method, params)) => film_control_call(&effective, method, params).await,
                Err(error) => Err(error),
            };
            report_edit_result(instance, &effective.title, result, true).await;
        }
        Ok(())
    }
}

struct RunFilmCraftCli;

#[async_trait]
impl Action for RunFilmCraftCli {
    const UUID: &'static str = "wt.luizgustavo.getcraft.filmcraft.run";
    type Settings = CommandSettings;

    async fn key_down(
        &self,
        instance: &Instance,
        settings: &Self::Settings,
    ) -> OpenActionResult<()> {
        run_action(instance, settings, false).await;
        Ok(())
    }

    async fn dial_down(
        &self,
        instance: &Instance,
        settings: &Self::Settings,
    ) -> OpenActionResult<()> {
        self.key_down(instance, settings).await
    }

    async fn will_appear(
        &self,
        instance: &Instance,
        settings: &Self::Settings,
    ) -> OpenActionResult<()> {
        if let Some(title) = settings.title() {
            let _ = instance.set_title(Some(title), None).await;
        }
        Ok(())
    }

    async fn send_to_plugin(
        &self,
        instance: &Instance,
        settings: &Self::Settings,
        payload: &Value,
    ) -> OpenActionResult<()> {
        let effective = payload
            .get("settings")
            .and_then(|value| serde_json::from_value::<CommandSettings>(value.clone()).ok())
            .unwrap_or_else(|| settings.clone());
        match payload.get("request").and_then(Value::as_str) {
            Some("catalog") => send_catalog(instance, &effective).await,
            Some("test") => run_action(instance, &effective, true).await,
            _ => {}
        }
        Ok(())
    }
}

#[tokio::main]
async fn main() -> OpenActionResult<()> {
    let _ = TermLogger::init(
        LevelFilter::Info,
        Config::default(),
        TerminalMode::Stdout,
        ColorChoice::Never,
    );
    register_action(FilmCraftEditControl).await;
    register_action(RunFilmCraftCli).await;
    run(std::env::args().collect()).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_a_string_argument_vector() {
        assert_eq!(
            parse_arguments(r#"["exec","timeline.play","{}"]"#).unwrap(),
            ["exec", "timeline.play", "{}"]
        );
    }

    #[test]
    fn rejects_non_string_arguments() {
        assert!(parse_arguments(r#"["exec", 4]"#).is_err());
    }

    #[test]
    fn stderr_is_preferred_in_a_failure_summary() {
        assert_eq!(summary(b"normal", b"bad\nnews"), "bad news");
    }

    #[test]
    fn has_a_usable_default_command() {
        assert_eq!(
            parse_arguments(&CommandSettings::default().arguments).unwrap(),
            CATALOG_ARGUMENTS
                .iter()
                .map(|value| (*value).to_string())
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn honors_a_one_second_timeout() {
        assert_eq!(
            CommandSettings {
                timeout_seconds: 1,
                ..Default::default()
            }
            .timeout(),
            Duration::from_secs(1)
        );
    }
}
