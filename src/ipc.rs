use anyhow::{Context, Result, bail};
use serde::Deserialize;
use serde_json::Value;
use std::{
    io::{BufRead, BufReader, Read, Write},
    os::unix::net::UnixStream,
    time::Duration,
};

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Monitor {
    pub name: String,
    #[serde(default)]
    pub active: bool,
    #[serde(default)]
    pub layout_symbol: String,
    #[serde(default)]
    pub active_tags: Vec<u32>,
}

#[derive(Deserialize)]
struct Monitors {
    monitors: Vec<Monitor>,
}

#[derive(Clone, Debug)]
pub struct Target {
    pub monitor: String,
    pub tags: Vec<u32>,
}

#[derive(Clone, Debug)]
pub struct Ipc {
    pub path: std::path::PathBuf,
}

impl Ipc {
    pub fn from_env() -> Result<Self> {
        Ok(Self { path: std::env::var_os("MANGO_INSTANCE_SIGNATURE").context("MANGO_INSTANCE_SIGNATURE is unset. Start this app inside your MangoWM session.")?.into() })
    }
    fn connect(&self) -> Result<UnixStream> {
        UnixStream::connect(&self.path)
            .with_context(|| format!("Cannot connect to MangoWM at {}", self.path.display()))
    }
    pub fn request(&self, command: &str) -> Result<Value> {
        let mut socket = self.connect()?;
        socket.set_read_timeout(Some(Duration::from_secs(2)))?;
        socket.set_write_timeout(Some(Duration::from_secs(2)))?;
        writeln!(socket, "{command}")?;
        let mut line = String::new();
        BufReader::new(socket)
            .take(1_048_576)
            .read_line(&mut line)?;
        let response: Value =
            serde_json::from_str(&line).context("Invalid MangoWM IPC response")?;
        if let Some(error) = response.get("error") {
            bail!("MangoWM: {error}");
        }
        Ok(response)
    }
    pub fn monitors(&self) -> Result<Vec<Monitor>> {
        Ok(serde_json::from_value::<Monitors>(self.request("get all-monitors")?)?.monitors)
    }
    pub fn target(&self, pointer: bool) -> Result<Target> {
        let monitors = self.monitors()?;
        let pointer_name = if pointer {
            self.request("get cursorpos")
                .ok()
                .and_then(|v| v["monitor"].as_str().map(String::from))
        } else {
            None
        };
        let monitor = pointer_name
            .as_ref()
            .and_then(|name| monitors.iter().find(|m| &m.name == name))
            .or_else(|| monitors.iter().find(|m| m.active))
            .context("No active MangoWM monitor")?;
        Ok(Target {
            monitor: monitor.name.clone(),
            tags: monitor.active_tags.clone(),
        })
    }
    pub fn apply(&self, target: &Target, layout: &str) -> Result<()> {
        let expected = crate::layout::by_name(layout).context("Unknown layout")?;
        validate_token(&target.monitor)?;
        let monitors = self.monitors()?;
        let monitor = monitors
            .iter()
            .find(|m| m.name == target.monitor)
            .context("The invoked monitor was disconnected")?;
        if monitor.active_tags != target.tags {
            bail!("The active tags changed. Reopen the picker to select their layout.");
        }
        let previous = monitors.iter().find(|m| m.active).map(|m| m.name.clone());
        let switched = !monitor.active;
        if switched {
            self.dispatch(&format!("focusmon,{}", target.monitor))?;
        }
        let result = (|| {
            let current = self.monitors()?;
            if !current
                .iter()
                .any(|m| m.name == target.monitor && m.active && m.active_tags == target.tags)
            {
                bail!("Monitor focus or active tags changed. Reopen the picker.");
            }
            self.dispatch(&format!("setlayout,{layout}"))?;
            let updated = self.monitors()?;
            let actual = updated
                .iter()
                .find(|m| m.name == target.monitor)
                .context("Monitor disconnected during selection")?;
            if !actual.layout_symbol.eq_ignore_ascii_case(expected.symbol) {
                bail!("MangoWM did not apply {layout}");
            }
            Ok(())
        })();
        if switched && let Some(previous) = previous {
            validate_token(&previous)?;
            self.dispatch(&format!("focusmon,{previous}"))?;
        }
        result
    }
    fn dispatch(&self, command: &str) -> Result<()> {
        let response = self.request(&format!("dispatch {command}"))?;
        if response["success"] != true {
            bail!("MangoWM did not acknowledge {command}");
        }
        Ok(())
    }
    pub fn watch(&self, mut on_update: impl FnMut(Vec<Monitor>) -> bool) -> Result<()> {
        let mut socket = self.connect()?;
        socket.set_write_timeout(Some(Duration::from_secs(2)))?;
        socket.write_all(b"watch all-monitors\n")?;
        let mut reader = BufReader::new(socket);
        loop {
            let mut line = String::new();
            if reader.by_ref().take(1_048_576).read_line(&mut line)? == 0 {
                bail!("MangoWM disconnected");
            }
            let value: Value = serde_json::from_str(&line)?;
            if let Some(error) = value.get("error") {
                bail!("MangoWM: {error}");
            }
            let state: Monitors = serde_json::from_value(value)?;
            if !on_update(state.monitors) {
                return Ok(());
            }
        }
    }
}

fn validate_token(token: &str) -> Result<()> {
    if token.is_empty()
        || !token
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_.:".contains(c))
    {
        bail!("Invalid monitor name");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_real_monitor_state() {
        let value = r#"{"monitors":[{"name":"DP-2","active":true,"layout_symbol":"CT","active_tags":[3],"tags":[]},{"name":"HDMI-A-1","active":false,"layout_symbol":"T","active_tags":[1]}]}"#;
        let state: Monitors = serde_json::from_str(value).unwrap();
        assert_eq!(state.monitors[0].active_tags, vec![3]);
        assert_eq!(
            crate::layout::by_symbol(&state.monitors[0].layout_symbol)
                .unwrap()
                .name,
            "center_tile"
        );
    }
    #[test]
    fn rejects_command_delimiters() {
        assert!(validate_token("DP-2").is_ok());
        for bad in ["", "DP-2,spawn,evil", "DP-2\n", "DP 2"] {
            assert!(validate_token(bad).is_err());
        }
    }
    #[test]
    fn request_handles_server_errors_and_partial_writes() {
        use std::os::unix::net::UnixListener;
        let path =
            std::env::temp_dir().join(format!("mango-tray-test-{}.sock", std::process::id()));
        let listener = UnixListener::bind(&path).unwrap();
        let worker = std::thread::spawn(move || {
            for response in ["{\"success\":true}\n", "{\"error\":\"unknown function\"}\n"] {
                let (mut stream, _) = listener.accept().unwrap();
                let mut command = String::new();
                BufReader::new(stream.try_clone().unwrap())
                    .read_line(&mut command)
                    .unwrap();
                assert_eq!(command, "get version\n");
                for byte in response.bytes() {
                    stream.write_all(&[byte]).unwrap();
                }
            }
        });
        let ipc = Ipc { path: path.clone() };
        assert_eq!(ipc.request("get version").unwrap()["success"], true);
        assert!(
            ipc.request("get version")
                .unwrap_err()
                .to_string()
                .contains("unknown function")
        );
        worker.join().unwrap();
        std::fs::remove_file(path).unwrap();
    }
    fn mock(
        name: &str,
        responses: Vec<(&'static str, &'static str)>,
    ) -> (Ipc, std::thread::JoinHandle<()>) {
        use std::os::unix::net::UnixListener;
        let path =
            std::env::temp_dir().join(format!("mango-tray-{}-{name}.sock", std::process::id()));
        let listener = UnixListener::bind(&path).unwrap();
        let cleanup = path.clone();
        let worker = std::thread::spawn(move || {
            for (expected, response) in responses {
                let (mut stream, _) = listener.accept().unwrap();
                let mut command = String::new();
                BufReader::new(stream.try_clone().unwrap())
                    .read_line(&mut command)
                    .unwrap();
                assert_eq!(command.trim_end(), expected);
                writeln!(stream, "{response}").unwrap();
            }
            std::fs::remove_file(cleanup).unwrap();
        });
        (Ipc { path }, worker)
    }

    #[test]
    fn applies_to_invoked_monitor_and_restores_focus() {
        let before = r#"{"monitors":[{"name":"DP-1","active":true,"active_tags":[1],"layout_symbol":"M"},{"name":"DP-2","active":false,"active_tags":[3],"layout_symbol":"CT"}]}"#;
        let focused = r#"{"monitors":[{"name":"DP-1","active":false,"active_tags":[1],"layout_symbol":"M"},{"name":"DP-2","active":true,"active_tags":[3],"layout_symbol":"CT"}]}"#;
        let after = r#"{"monitors":[{"name":"DP-1","active":false,"active_tags":[1],"layout_symbol":"M"},{"name":"DP-2","active":true,"active_tags":[3],"layout_symbol":"T"}]}"#;
        let (ipc, worker) = mock(
            "monitor",
            vec![
                ("get all-monitors", before),
                ("dispatch focusmon,DP-2", r#"{"success":true}"#),
                ("get all-monitors", focused),
                ("dispatch setlayout,tile", r#"{"success":true}"#),
                ("get all-monitors", after),
                ("dispatch focusmon,DP-1", r#"{"success":true}"#),
            ],
        );
        ipc.apply(
            &Target {
                monitor: "DP-2".into(),
                tags: vec![3],
            },
            "tile",
        )
        .unwrap();
        worker.join().unwrap();
    }

    #[test]
    fn changed_tags_refuse_selection_without_dispatch() {
        let (ipc, worker) = mock(
            "tags",
            vec![(
                "get all-monitors",
                r#"{"monitors":[{"name":"DP-2","active":true,"active_tags":[4],"layout_symbol":"CT"}]}"#,
            )],
        );
        let error = ipc
            .apply(
                &Target {
                    monitor: "DP-2".into(),
                    tags: vec![3],
                },
                "tile",
            )
            .unwrap_err();
        assert!(error.to_string().contains("active tags changed"));
        worker.join().unwrap();
    }

    #[test]
    fn failed_selection_still_restores_monitor_focus() {
        let before = r#"{"monitors":[{"name":"DP-1","active":true,"active_tags":[1]},{"name":"DP-2","active":false,"active_tags":[3]}]}"#;
        let focused = r#"{"monitors":[{"name":"DP-1","active":false,"active_tags":[1]},{"name":"DP-2","active":true,"active_tags":[3]}]}"#;
        let (ipc, worker) = mock(
            "restore",
            vec![
                ("get all-monitors", before),
                ("dispatch focusmon,DP-2", r#"{"success":true}"#),
                ("get all-monitors", focused),
                ("dispatch setlayout,tile", r#"{"error":"failed"}"#),
                ("dispatch focusmon,DP-1", r#"{"success":true}"#),
            ],
        );
        assert!(
            ipc.apply(
                &Target {
                    monitor: "DP-2".into(),
                    tags: vec![3]
                },
                "tile"
            )
            .is_err()
        );
        worker.join().unwrap();
    }
}
