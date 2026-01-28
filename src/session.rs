use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Deserialize, Clone)]
pub struct Session {
    pub session_id: String,
    pub pane_id: String,
    pub cwd: String,
    pub status: String,
    pub notification_message: Option<String>,
    #[allow(dead_code)]
    pub notification_type: Option<String>,
    pub updated: u64,
    #[serde(skip)]
    pub summary: Option<String>,
    #[serde(skip)]
    pub first_prompt: Option<String>,
    #[serde(skip)]
    pub message_count: Option<u32>,
    #[serde(skip)]
    pub git_branch: Option<String>,
    #[serde(skip)]
    pub modified: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SessionsIndex {
    entries: Vec<SessionIndexEntry>,
}

#[derive(Debug, Deserialize)]
struct SessionIndexEntry {
    #[serde(rename = "sessionId")]
    session_id: String,
    summary: Option<String>,
    #[serde(rename = "firstPrompt")]
    first_prompt: Option<String>,
    #[serde(rename = "messageCount")]
    message_count: Option<u32>,
    #[serde(rename = "gitBranch")]
    git_branch: Option<String>,
    modified: Option<String>,
    #[allow(dead_code)]
    #[serde(rename = "projectPath")]
    project_path: Option<String>,
}

fn get_sessions_dir() -> Result<PathBuf> {
    let home = std::env::var("HOME").context("HOME環境変数が見つかりません")?;
    Ok(PathBuf::from(home).join(".claude/sessions"))
}

pub fn load_sessions() -> Result<Vec<Session>> {
    let sessions_dir = get_sessions_dir()?;
    let mut sessions = Vec::new();

    if !sessions_dir.exists() {
        return Ok(sessions);
    }

    for entry in fs::read_dir(&sessions_dir)
        .context("セッションディレクトリの読み込みに失敗")?
    {
        let entry = entry?;
        let path = entry.path();

        if path.extension().and_then(|s| s.to_str()) == Some("json") {
            let content = fs::read_to_string(&path)
                .with_context(|| format!("ファイル読み込みエラー: {:?}", path))?;
            let session: Session = serde_json::from_str(&content)
                .with_context(|| format!("JSONパースエラー: {:?}", path))?;
            sessions.push(session);
        }
    }

    Ok(sessions)
}

fn get_active_pane_ids() -> Result<HashSet<String>> {
    let wezterm = "/mnt/c/Program Files/WezTerm/wezterm.exe";

    let output = Command::new(wezterm)
        .args(["cli", "list", "--format", "json"])
        .output()
        .context("WezTermのペイン一覧取得に失敗")?;

    if !output.status.success() {
        return Err(anyhow::anyhow!("WezTerm cliコマンドが失敗しました"));
    }

    let json_str = String::from_utf8(output.stdout)
        .context("WezTerm出力のUTF-8変換に失敗")?;

    let panes: Vec<serde_json::Value> = serde_json::from_str(&json_str)
        .context("WezTerm JSON解析に失敗")?;

    let pane_ids: HashSet<String> = panes
        .iter()
        .filter_map(|p| p["pane_id"].as_u64())
        .map(|id| id.to_string())
        .collect();

    Ok(pane_ids)
}

pub fn filter_active_sessions(sessions: Vec<Session>) -> Result<Vec<Session>> {
    let active_pane_ids = get_active_pane_ids()?;

    // pane_idごとに最新のセッションだけを保持
    let mut pane_to_session: HashMap<String, Session> = HashMap::new();

    for session in sessions {
        if !active_pane_ids.contains(&session.pane_id) {
            continue;
        }

        // 既存のセッションより新しければ更新
        if let Some(existing) = pane_to_session.get(&session.pane_id) {
            if session.updated > existing.updated {
                pane_to_session.insert(session.pane_id.clone(), session);
            }
        } else {
            pane_to_session.insert(session.pane_id.clone(), session);
        }
    }

    let mut filtered: Vec<Session> = pane_to_session.into_values().collect();

    // タイムスタンプでソート（新しい順）
    filtered.sort_by(|a, b| b.updated.cmp(&a.updated));

    Ok(filtered)
}

pub fn find_session_by_id<'a>(sessions: &'a [Session], session_id: &str) -> Option<&'a Session> {
    sessions.iter().find(|s| s.session_id == session_id)
}

fn cwd_to_project_path(cwd: &str) -> String {
    // cwdから.claude/projectsのディレクトリ名を生成
    // 例: "/home/aya/.dotfiles" -> "-home-aya--dotfiles"
    // '/' と '.' の両方を '-' に置き換える
    cwd.replace('/', "-").replace('.', "-")
}

fn load_sessions_index(cwd: &str) -> Result<HashMap<String, SessionIndexEntry>> {
    let home = std::env::var("HOME").context("HOME環境変数が見つかりません")?;
    let project_dir_name = cwd_to_project_path(cwd);
    let index_path = Path::new(&home)
        .join(".claude/projects")
        .join(&project_dir_name)
        .join("sessions-index.json");

    if !index_path.exists() {
        return Ok(HashMap::new());
    }

    let content = fs::read_to_string(&index_path)
        .with_context(|| format!("sessions-index.json読み込みエラー: {:?}", index_path))?;

    let index: SessionsIndex = serde_json::from_str(&content)
        .with_context(|| format!("sessions-index.json解析エラー: {:?}", index_path))?;

    let map: HashMap<String, SessionIndexEntry> = index
        .entries
        .into_iter()
        .map(|entry| (entry.session_id.clone(), entry))
        .collect();

    Ok(map)
}

pub fn enrich_sessions_with_index(sessions: &mut [Session]) -> Result<()> {
    // cwdごとにsessions-index.jsonを読み込む
    let mut cwd_to_index: HashMap<String, HashMap<String, SessionIndexEntry>> = HashMap::new();

    for session in sessions.iter() {
        if !cwd_to_index.contains_key(&session.cwd) {
            let index = load_sessions_index(&session.cwd).unwrap_or_default();
            cwd_to_index.insert(session.cwd.clone(), index);
        }
    }

    // 各セッションにsummary、first_prompt、その他の情報を追加
    for session in sessions.iter_mut() {
        if let Some(index) = cwd_to_index.get(&session.cwd) {
            if let Some(entry) = index.get(&session.session_id) {
                session.summary = entry.summary.clone();
                session.first_prompt = entry.first_prompt.clone();
                session.message_count = entry.message_count;
                session.git_branch = entry.git_branch.clone();
                session.modified = entry.modified.clone();
            }
        }
    }

    Ok(())
}
