use crate::session::Session;

fn get_status_icon(status: &str) -> &str {
    match status {
        "active" => "🟢",
        "waiting" => "🟡",
        "stopped" => "⚪",
        _ => "❓",
    }
}

fn format_cwd(cwd: &str) -> String {
    if let Some(home) = std::env::var("HOME").ok() {
        cwd.replace(&home, "~")
    } else {
        cwd.to_string()
    }
}

fn get_status_label(status: &str) -> &str {
    match status {
        "active" => "実行中",
        "waiting" => "承認待ち",
        "stopped" => "完了",
        _ => "不明",
    }
}

fn truncate_text(text: &str, max_chars: usize) -> String {
    let char_count = text.chars().count();
    if char_count <= max_chars {
        text.to_string()
    } else {
        let truncated: String = text.chars().take(max_chars.saturating_sub(3)).collect();
        format!("{}...", truncated)
    }
}

fn format_relative_time(timestamp_str: &str) -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    if let Ok(parsed) = chrono::DateTime::parse_from_rfc3339(timestamp_str) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let modified_ts = parsed.timestamp();
        let diff = now - modified_ts;

        if diff < 60 {
            return "たった今".to_string();
        } else if diff < 3600 {
            return format!("{}分前", diff / 60);
        } else if diff < 86400 {
            return format!("{}時間前", diff / 3600);
        } else {
            return format!("{}日前", diff / 86400);
        }
    }

    "不明".to_string()
}

pub fn display_sessions(sessions: &[Session]) {
    println!("\n📋 Claude Codeセッション一覧\n");

    for session in sessions {
        let icon = get_status_icon(&session.status);
        let status_label = get_status_label(&session.status);
        let cwd = format_cwd(&session.cwd);

        println!(
            "{} {:<10} {}  (pane:{})",
            icon, status_label, cwd, session.pane_id
        );

        // notification_messageがあれば表示
        if let Some(ref msg) = session.notification_message {
            println!("   └─ {}", msg);
        }

        // summaryまたはfirst_promptがあれば表示
        if let Some(ref summary) = session.summary {
            println!("   └─ \"{}\"", truncate_text(summary, 60));
        } else if let Some(ref first_prompt) = session.first_prompt {
            println!("   └─ \"{}\"", truncate_text(first_prompt, 60));
        }

        // メッセージ数、メモリ使用量、Gitブランチ、最終更新時刻を表示
        let mut meta_parts = vec![];

        if let Some(count) = session.message_count {
            meta_parts.push(format!("{}msg", count));
        }

        if let Some(mem_kb) = session.memory_usage_kb {
            let mem_mb = mem_kb / 1024;
            if mem_mb >= 1024 {
                meta_parts.push(format!("{:.1}GB", mem_mb as f64 / 1024.0));
            } else {
                meta_parts.push(format!("{}MB", mem_mb));
            }
        }

        if let Some(ref branch) = session.git_branch {
            meta_parts.push(format!("@{}", branch));
        }

        if let Some(ref modified) = session.modified {
            meta_parts.push(format_relative_time(modified));
        }

        if !meta_parts.is_empty() {
            println!("   └─ {}", meta_parts.join(" · "));
        }

        println!();
    }

    println!("合計: {}セッション\n", sessions.len());
}
