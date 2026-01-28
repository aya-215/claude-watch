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

        println!();
    }

    println!("合計: {}セッション\n", sessions.len());
}
