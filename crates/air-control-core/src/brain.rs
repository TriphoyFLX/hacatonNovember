//! Turns a messy spoken line into one allowlisted command, or a short answer.
//!
//! The model never runs a shell. A phrase is executed only when
//! `intent_from_phrase` already accepts it.

use std::path::Path;

use serde_json::Value;

use crate::commands::{intent_from_phrase, voice_phrase_catalog, CommandSpec};

pub enum BrainEvent {
    Say(String),
    Run { heard: String, command: CommandSpec },
}

pub fn interpret(heard: &str) -> BrainEvent {
    let heard = heard.trim();
    if heard.is_empty() {
        return BrainEvent::Say("Ничего не расслышала.".into());
    }
    let Some(key) = api_key() else {
        return BrainEvent::Say("Ключ помощника не найден.".into());
    };
    match complete(&key, heard) {
        Ok(content) => decide(heard, &content),
        Err(message) => BrainEvent::Say(message),
    }
}

pub(crate) fn decide(heard: &str, content: &str) -> BrainEvent {
    let Some(json) = extract_json(content) else {
        return BrainEvent::Say("Не разобрала ответ.".into());
    };
    let Ok(value) = serde_json::from_str::<Value>(json) else {
        return BrainEvent::Say("Не разобрала ответ.".into());
    };
    if let Some(phrase) = value.get("phrase").and_then(Value::as_str) {
        let phrase: String = phrase.chars().filter(|ch| !ch.is_control()).take(100).collect();
        let phrase = phrase.trim();
        if let Some(command) = intent_from_phrase(phrase) {
            if shellish(&command) {
                return BrainEvent::Say(
                    "Терминал и удаление файлов я не выполняю.".into(),
                );
            }
            return BrainEvent::Run {
                heard: heard.to_string(),
                command,
            };
        }
    }
    if let Some(say) = value.get("say").and_then(Value::as_str) {
        let say = clip(say, 280);
        if !say.is_empty() {
            return BrainEvent::Say(say);
        }
    }
    BrainEvent::Say("Не поняла. Спросите что-нибудь или скажите команду своими словами.".into())
}

fn shellish(command: &CommandSpec) -> bool {
    let text = match command {
        CommandSpec::TypeText { text } | CommandSpec::NewNote { text } => text.to_lowercase(),
        _ => return false,
    };
    const BAD: &[&str] = &[
        "rm -rf",
        "rm -fr",
        "sudo ",
        "mkfs",
        "shutdown",
        "reboot",
        ":(){",
        "dd if=",
        "/dev/sd",
        "chmod 777",
        "format c:",
    ];
    BAD.iter().any(|token| text.contains(token))
}

fn clip(text: &str, limit: usize) -> String {
    text.chars()
        .filter(|ch| !ch.is_control())
        .take(limit)
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn extract_json(content: &str) -> Option<&str> {
    let start = content.find('{')?;
    let end = content.rfind('}')?;
    (end > start).then_some(&content[start..=end])
}

fn system_prompt() -> String {
    format!(
        "Ты Ася, голосовой помощник AirControl. Пользователь сказал фразу по-русски, часто коряво.\n\
Верни один JSON-объект и ничего больше.\n\
Если это просьба сделать что-то на компьютере, верни {{\"phrase\":\"каноническая команда\"}}.\n\
Фраза должна быть обычными словами из списка, чтобы локальный разбор её принял:\n{}\n\
Примеры: «мотни экран книзу» → {{\"phrase\":\"вниз\"}}. «погугли погоду в омске» → {{\"phrase\":\"загугли погода в омске\"}}.\n\
Если это вопрос или болтовня, верни {{\"say\":\"короткий ответ по-русски\"}}. Не выдумывай точное время, погоду и новости: скажи, что для этого есть команды «скажи погоду» и «загугли».\n\
Если просят терминал, shell, rm, удалить файлы, пароли или действие вне списка, верни {{\"say\":\"Я отвечаю и выполняю только команды AirControl. Терминал и удаление файлов не трогаю.\"}}\n\
phrase короче 80 символов, say короче 240. Без markdown.",
        voice_phrase_catalog()
    )
}

fn complete(key: &str, user: &str) -> Result<String, String> {
    let body = serde_json::json!({
        "model": "openai/gpt-4o-mini",
        "temperature": 0.2,
        "max_tokens": 180,
        "response_format": {"type": "json_object"},
        "messages": [
            {"role": "system", "content": system_prompt()},
            {"role": "user", "content": user}
        ]
    });
    let mut child = std::process::Command::new("curl")
        .args([
            "-fsS",
            "--max-time",
            "12",
            "https://openrouter.ai/api/v1/chat/completions",
            "-H",
            "Content-Type: application/json",
            "-H",
            &format!("Authorization: Bearer {key}"),
            "-H",
            "HTTP-Referer: http://localhost",
            "-H",
            "X-Title: AirControl",
            "-d",
            "@-",
        ])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|_| "Нет curl, помощник молчит.".to_string())?;
    {
        use std::io::Write;
        let stdin = child.stdin.as_mut().ok_or_else(|| "Нет связи.".to_string())?;
        stdin
            .write_all(body.to_string().as_bytes())
            .map_err(|_| "Нет связи.".to_string())?;
    }
    let output = child
        .wait_with_output()
        .map_err(|_| "Помощник не ответил.".to_string())?;
    if !output.status.success() {
        return Err("Помощник не ответил.".into());
    }
    let raw = String::from_utf8_lossy(&output.stdout);
    let value: Value = serde_json::from_str(&raw).map_err(|_| "Помощник не ответил.".to_string())?;
    value["choices"][0]["message"]["content"]
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| "Помощник не ответил.".into())
}

fn api_key() -> Option<String> {
    if let Some(key) = clean_key(&std::env::var("OPENROUTER_API_KEY").unwrap_or_default()) {
        return Some(key);
    }
    let mut dirs = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        dirs.push(cwd);
    }
    if let Ok(exe) = std::env::current_exe() {
        let mut cursor = exe.parent().map(Path::to_path_buf);
        for _ in 0..6 {
            let Some(dir) = cursor else {
                break;
            };
            let parent = dir.parent().map(Path::to_path_buf);
            dirs.push(dir);
            cursor = parent;
        }
    }
    for dir in dirs {
        if let Some(key) = read_dotenv(&dir.join(".env")) {
            return Some(key);
        }
    }
    None
}

fn read_dotenv(path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    for line in text.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("OPENROUTER_API_KEY=") else {
            continue;
        };
        if let Some(key) = clean_key(rest) {
            return Some(key);
        }
    }
    None
}

fn clean_key(raw: &str) -> Option<String> {
    let key = raw.trim().trim_matches('"').trim_matches('\'').trim();
    (!key.is_empty()).then(|| key.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phrase_must_be_allowlisted() {
        let event = decide("мотни вниз", r#"{"phrase":"вниз"}"#);
        assert!(matches!(
            event,
            BrainEvent::Run {
                command: CommandSpec::PageScroll { down: true, .. },
                ..
            }
        ));
    }

    #[test]
    fn shell_is_not_executed() {
        let event = decide("удали все", r#"{"phrase":"напиши rm -rf /"}"#);
        assert!(matches!(event, BrainEvent::Say(_)));
    }

    #[test]
    fn unknown_phrase_is_not_run() {
        let event = decide("сделай", r#"{"phrase":"rm -rf /"}"#);
        assert!(matches!(event, BrainEvent::Say(_)));
    }

    #[test]
    fn question_is_text() {
        let event = decide("кто ты", r#"{"say":"Я Ася, помогаю с командами."}"#);
        match event {
            BrainEvent::Say(text) => assert!(text.contains("Ася")),
            BrainEvent::Run { .. } => panic!("вопрос не должен стать командой"),
        }
    }
}
