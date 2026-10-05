//! Human output is concise; machine output is one complete JSON value.

use anyhow::{Context, Result};
use serde::Serialize;
use std::io::{self, IsTerminal, Write};

/// Per-command output policy, including terminal-aware color handling.
pub struct Output {
    /// JSON output disables decorative status lines and color.
    pub json: bool,
}

impl Output {
    /// Emit one JSON value or a concise success message to stdout.
    pub fn success<T: Serialize>(&self, value: &T, message: &str) -> Result<()> {
        if self.json {
            return self.value(value);
        }
        let color = io::stdout().is_terminal()
            && std::env::var_os("NO_COLOR").is_none()
            && std::env::var("TERM").map_or(true, |term| term != "dumb");
        let line = if color {
            format!("\x1b[32m{}\x1b[0m\n", message)
        } else {
            format!("{}\n", message)
        };
        let mut stdout = anstream::AutoStream::new(io::stdout(), anstream::ColorChoice::Auto);
        stdout.write_all(line.as_bytes()).context("write stdout")
    }

    /// Serialize as a single pretty JSON document, without interleaved diagnostics.
    pub fn value<T: Serialize>(&self, value: &T) -> Result<()> {
        let mut stdout = io::stdout().lock();
        serde_json::to_writer_pretty(&mut stdout, value).context("write JSON output")?;
        stdout.write_all(b"\n").context("write stdout")
    }

    /// Write a sanitized human table, never allowing package text to emit escapes.
    pub fn table(&self, rows: &[(String, String, String)]) -> Result<()> {
        if rows.is_empty() {
            return self.success(&Vec::<String>::new(), "No skills.");
        }
        let mut stdout = io::stdout().lock();
        writeln!(stdout, "{:<38} {:<12} DESCRIPTION", "SKILL", "SHA256")?;
        for (name, hash, description) in rows {
            writeln!(
                stdout,
                "{:<38} {:<12} {}",
                clean(name),
                hash.get(..12).unwrap_or(hash),
                clean(description)
            )?;
        }
        Ok(())
    }
}

/// Escape/control sequences have no place in terminal-visible untrusted text.
fn clean(text: &str) -> String {
    text.chars().filter(|c| !c.is_control()).collect()
}
