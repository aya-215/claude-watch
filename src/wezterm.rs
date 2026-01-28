use anyhow::{anyhow, Context, Result};
use std::process::Command;

pub fn jump_to_pane(pane_id: &str) -> Result<()> {
    let wezterm = "/mnt/c/Program Files/WezTerm/wezterm.exe";

    let status = Command::new(wezterm)
        .args(["cli", "activate-pane", "--pane-id", pane_id])
        .status()
        .context("WezTermコマンドの実行に失敗")?;

    if !status.success() {
        return Err(anyhow!("WezTermのpane {}へのジャンプに失敗しました", pane_id));
    }

    println!("✅ Pane {} にジャンプしました", pane_id);
    Ok(())
}
