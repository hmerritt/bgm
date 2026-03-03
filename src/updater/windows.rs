use super::{UpdateTrigger, UpdaterEvent, UpdaterStatus};
use crate::config::UpdaterConfig;
use crate::installer;
use crate::version::BINARY_FILENAME;
use anyhow::{Context, Result};
use serde::Deserialize;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::Duration;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

#[derive(Debug, Clone)]
pub struct RestartContext {
    update_exe: PathBuf,
    app_exe_name: String,
    relaunch_args: Vec<String>,
}

pub struct UpdaterRuntime {
    status: UpdaterStatus,
    check_interval: Option<Duration>,
    trigger_tx: Option<UnboundedSender<UpdateTrigger>>,
    event_rx: Option<UnboundedReceiver<UpdaterEvent>>,
    restart_context: Option<RestartContext>,
}

impl UpdaterRuntime {
    pub fn status(&self) -> UpdaterStatus {
        self.status
    }

    pub fn check_interval(&self) -> Option<Duration> {
        self.check_interval
    }

    pub fn request_check(&self, trigger: UpdateTrigger) -> bool {
        self.trigger_tx
            .as_ref()
            .map(|tx| tx.send(trigger).is_ok())
            .unwrap_or(false)
    }

    pub fn take_event_receiver(&mut self) -> Option<UnboundedReceiver<UpdaterEvent>> {
        self.event_rx.take()
    }

    pub fn restart_context(&self) -> Option<RestartContext> {
        self.restart_context.clone()
    }
}

#[derive(Debug, Clone)]
struct WorkerContext {
    update_exe: PathBuf,
    feed_url: String,
}

pub fn initialize(config: &UpdaterConfig, relaunch_args: Vec<String>) -> UpdaterRuntime {
    if !config.enabled {
        return UpdaterRuntime {
            status: UpdaterStatus::Disabled,
            check_interval: None,
            trigger_tx: None,
            event_rx: None,
            restart_context: None,
        };
    }

    let update_exe = match installer::locate_update_exe() {
        Ok(path) => path,
        Err(error) => {
            tracing::info!(
                error = %error,
                "self-updater unavailable: not running from a Squirrel install"
            );
            return UpdaterRuntime {
                status: UpdaterStatus::Unsupported,
                check_interval: None,
                trigger_tx: None,
                event_rx: None,
                restart_context: None,
            };
        }
    };

    let worker_context = WorkerContext {
        update_exe: update_exe.clone(),
        feed_url: config.feed_url.clone(),
    };
    let restart_context = RestartContext {
        update_exe,
        app_exe_name: format!("{BINARY_FILENAME}.exe"),
        relaunch_args,
    };

    let (trigger_tx, trigger_rx) = unbounded_channel();
    let (event_tx, event_rx) = unbounded_channel();
    tokio::spawn(run_worker(worker_context, trigger_rx, event_tx));

    UpdaterRuntime {
        status: UpdaterStatus::Idle,
        check_interval: Some(config.check_interval),
        trigger_tx: Some(trigger_tx),
        event_rx: Some(event_rx),
        restart_context: Some(restart_context),
    }
}

pub fn restart_installed_app(context: &RestartContext) -> Result<()> {
    let relaunch_args = quote_windows_args(&context.relaunch_args);
    let mut variants = Vec::new();

    if relaunch_args.is_empty() {
        variants.push(vec![
            "--processStart".to_string(),
            context.app_exe_name.clone(),
        ]);
        variants.push(vec![format!("--processStart={}", context.app_exe_name)]);
    } else {
        variants.push(vec![
            "--processStart".to_string(),
            context.app_exe_name.clone(),
            "--process-start-args".to_string(),
            relaunch_args.clone(),
        ]);
        variants.push(vec![
            "--processStart".to_string(),
            context.app_exe_name.clone(),
            format!("--process-start-args={relaunch_args}"),
        ]);
        variants.push(vec![
            format!("--processStart={}", context.app_exe_name),
            "--process-start-args".to_string(),
            relaunch_args.clone(),
        ]);
        variants.push(vec![
            format!("--processStart={}", context.app_exe_name),
            format!("--process-start-args={relaunch_args}"),
        ]);
    }

    run_update_variants(
        &context.update_exe,
        &variants,
        "restart updated application",
    )?;

    Ok(())
}

async fn run_worker(
    context: WorkerContext,
    mut trigger_rx: UnboundedReceiver<UpdateTrigger>,
    event_tx: UnboundedSender<UpdaterEvent>,
) {
    while let Some(trigger) = trigger_rx.recv().await {
        tracing::debug!(?trigger, "processing app update trigger");
        if event_tx
            .send(UpdaterEvent::Status(UpdaterStatus::Checking))
            .is_err()
        {
            break;
        }

        match check_for_update(context.clone()).await {
            Ok(false) => {
                if event_tx
                    .send(UpdaterEvent::Status(UpdaterStatus::UpToDate))
                    .is_err()
                {
                    break;
                }
            }
            Ok(true) => {
                if event_tx
                    .send(UpdaterEvent::Status(UpdaterStatus::UpdateAvailable))
                    .is_err()
                {
                    break;
                }
                if event_tx
                    .send(UpdaterEvent::Status(UpdaterStatus::Installing))
                    .is_err()
                {
                    break;
                }
                match install_update(context.clone()).await {
                    Ok(()) => {
                        if event_tx
                            .send(UpdaterEvent::Status(UpdaterStatus::InstalledPendingRestart))
                            .is_err()
                        {
                            break;
                        }
                        if event_tx.send(UpdaterEvent::InstallReady).is_err() {
                            break;
                        }
                    }
                    Err(error) => {
                        tracing::warn!(error = %error, "failed to install app update");
                        if event_tx
                            .send(UpdaterEvent::Status(UpdaterStatus::Error))
                            .is_err()
                        {
                            break;
                        }
                    }
                }
            }
            Err(error) => {
                tracing::warn!(error = %error, "failed to check for app update");
                if event_tx
                    .send(UpdaterEvent::Status(UpdaterStatus::Error))
                    .is_err()
                {
                    break;
                }
            }
        }
    }
}

async fn check_for_update(context: WorkerContext) -> Result<bool> {
    tokio::task::spawn_blocking(move || check_for_update_sync(&context))
        .await
        .context("self-updater worker panicked while checking for updates")?
}

async fn install_update(context: WorkerContext) -> Result<()> {
    tokio::task::spawn_blocking(move || install_update_sync(&context))
        .await
        .context("self-updater worker panicked while installing update")?
}

fn check_for_update_sync(context: &WorkerContext) -> Result<bool> {
    let variants = [
        vec![format!("--checkForUpdate={}", context.feed_url)],
        vec!["--checkForUpdate".to_string(), context.feed_url.clone()],
    ];
    let output = run_update_variants(&context.update_exe, &variants, "check for updates")?;
    parse_check_for_update_output(&output.stdout)
}

fn install_update_sync(context: &WorkerContext) -> Result<()> {
    let variants = [
        vec![format!("--update={}", context.feed_url)],
        vec!["--update".to_string(), context.feed_url.clone()],
    ];
    run_update_variants(&context.update_exe, &variants, "install updates")?;
    Ok(())
}

fn run_update_variants(
    update_exe: &Path,
    variants: &[Vec<String>],
    action: &str,
) -> Result<Output> {
    let mut failures = Vec::new();

    for args in variants {
        let output = Command::new(update_exe)
            .args(args)
            .output()
            .with_context(|| format!("failed to execute {}", update_exe.display()))?;

        if output.status.success() {
            return Ok(output);
        }

        failures.push(format!(
            "args=[{}] status={} stderr={}",
            args.join(" "),
            output
                .status
                .code()
                .map(|value| value.to_string())
                .unwrap_or_else(|| "<signal>".to_string()),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    anyhow::bail!(
        "failed to {action} via {}: {}",
        update_exe.display(),
        failures.join(" | ")
    );
}

#[derive(Debug, Deserialize)]
struct CheckForUpdateResponse {
    #[serde(default, rename = "releasesToApply", alias = "ReleasesToApply")]
    releases_to_apply: Vec<Value>,
}

fn parse_check_for_update_output(stdout: &[u8]) -> Result<bool> {
    let text = String::from_utf8_lossy(stdout);
    if let Some(payload) = extract_check_payload(&text) {
        let parsed: CheckForUpdateResponse = serde_json::from_str(payload)
            .with_context(|| format!("invalid --checkForUpdate payload: {payload}"))?;
        return Ok(!parsed.releases_to_apply.is_empty());
    }

    let lowered = text.to_ascii_lowercase();
    if lowered.contains("no updates") || lowered.contains("up to date") {
        return Ok(false);
    }

    anyhow::bail!("unable to parse --checkForUpdate output: {}", text.trim());
}

fn extract_check_payload(stdout: &str) -> Option<&str> {
    let trimmed = stdout.trim();
    if trimmed.starts_with('{') && trimmed.ends_with('}') {
        return Some(trimmed);
    }

    for line in stdout.lines().rev() {
        let line = line.trim();
        if line.starts_with('{') && line.ends_with('}') {
            return Some(line);
        }
    }

    let start = stdout.find('{')?;
    let end = stdout.rfind('}')?;
    if end <= start {
        return None;
    }
    Some(stdout[start..=end].trim())
}

fn quote_windows_args(args: &[String]) -> String {
    args.iter()
        .map(|arg| quote_windows_arg(arg))
        .collect::<Vec<_>>()
        .join(" ")
}

fn quote_windows_arg(arg: &str) -> String {
    if arg.is_empty() {
        return "\"\"".to_string();
    }
    if !arg.chars().any(|ch| ch.is_whitespace() || ch == '"') {
        return arg.to_string();
    }

    let mut quoted = String::from("\"");
    let mut backslashes = 0_usize;

    for ch in arg.chars() {
        if ch == '\\' {
            backslashes += 1;
            continue;
        }

        if ch == '"' {
            quoted.push_str(&"\\".repeat(backslashes * 2 + 1));
            quoted.push('"');
            backslashes = 0;
            continue;
        }

        if backslashes > 0 {
            quoted.push_str(&"\\".repeat(backslashes));
            backslashes = 0;
        }
        quoted.push(ch);
    }

    if backslashes > 0 {
        quoted.push_str(&"\\".repeat(backslashes * 2));
    }
    quoted.push('"');
    quoted
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_check_payload_from_single_line_json() {
        let payload = br#"{"releasesToApply":[{"Version":"1.2.4"}]}"#;
        assert!(parse_check_for_update_output(payload).unwrap());
    }

    #[test]
    fn parse_check_payload_from_multi_line_output() {
        let payload = br#"Downloading RELEASES
Done
{"releasesToApply":[]}"#;
        assert!(!parse_check_for_update_output(payload).unwrap());
    }

    #[test]
    fn quote_windows_arg_handles_spaces_and_quotes() {
        assert_eq!(quote_windows_arg("abc"), "abc");
        assert_eq!(quote_windows_arg("a b"), "\"a b\"");
        assert_eq!(quote_windows_arg(r#"a "b""#), r#""a \"b\"""#);
    }
}
