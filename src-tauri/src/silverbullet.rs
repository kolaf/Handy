//! SilverBullet (https://silverbullet.md) as the home of meeting notes, with the project a meeting belongs to as context.
//!
//! A SilverBullet "space" is a folder of Markdown files with a small HTTP API: `GET /.fs` lists the files, `GET|PUT /.fs/<path>`
//! reads and writes one, authenticated with `Authorization: Bearer <token>`. A project is a page whose frontmatter has
//! `tags: project`; its tasks are `* [ ] ...` lines, and tasks on other pages that link to it (`[[Project]]`) belong to it too
//! (SilverBullet's own "Linked Tasks" widget shows them on the project page).
//!
//! What this module does, and does not:
//! - it only ever READS existing pages and CREATES new ones (create-only: `If-None-Match: *`, so nothing is overwritten); it never
//!   edits, ticks or deletes anything that exists;
//! - everything that came from a language model or from speech is neutralised before it is written, because a page of a space can
//!   run code (`${...}` expressions, `<!-- #lua -->` directives, space-lua code blocks) in the browser of whoever opens it.

use futures_util::{stream, StreamExt};
use reqwest::{Client, StatusCode};
use serde::Deserialize;
use std::time::Duration;

const MAX_PAGES_SCANNED: usize = 400;
const MAX_PAGE_BYTES: u64 = 200_000;
const MAX_CONTEXT_CHARS: usize = 12_000;
const MAX_TASKS: usize = 80;
const MAX_LINE_CHARS: usize = 300;
const MAX_ITEMS: usize = 25;

pub struct Space {
    base: String,
    token: String,
    http: Client,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FileMeta {
    pub name: String,
    #[serde(default)]
    pub size: u64,
}

#[derive(Debug, PartialEq)]
pub enum CreateError {
    /// A page with that name exists already (nothing was written).
    Exists,
    Other(String),
}

impl Space {
    /// `url` is the space including its path, for example `http://100.64.0.1:3000/notes`.
    pub fn new(url: &str, token: &str) -> Result<Space, String> {
        let base = url.trim().trim_end_matches('/').to_string();
        if !(base.starts_with("http://") || base.starts_with("https://")) {
            return Err("The SilverBullet address must start with http:// or https://".into());
        }
        if token.trim().is_empty() {
            return Err("No SilverBullet token is set.".into());
        }
        let http = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Space {
            base,
            token: token.trim().to_string(),
            http,
        })
    }

    /// The address of a page in the browser.
    pub fn page_url(&self, page: &str) -> String {
        format!("{}/{}", self.base, encode_path(page))
    }

    fn file_url(&self, file: &str) -> String {
        format!("{}/.fs/{}", self.base, encode_path(file))
    }

    fn explain(status: StatusCode) -> String {
        match status.as_u16() {
            401 | 403 => format!("SilverBullet refused the token (HTTP {status}); check the token and its access to the space"),
            404 => "not found (check the space address, for example http://host:3000/notes)".into(),
            _ => format!("SilverBullet answered HTTP {status}"),
        }
    }

    pub async fn list(&self) -> Result<Vec<FileMeta>, String> {
        let response = self
            .http
            .get(format!("{}/.fs", self.base))
            .bearer_auth(&self.token)
            .header("X-Sync-Mode", "true")
            .send()
            .await
            .map_err(|e| format!("Cannot reach SilverBullet: {e}"))?;
        if !response.status().is_success() {
            return Err(Self::explain(response.status()));
        }
        response
            .json::<Vec<FileMeta>>()
            .await
            .map_err(|e| format!("Unexpected answer from SilverBullet: {e}"))
    }

    pub async fn read(&self, file: &str) -> Result<String, String> {
        let response = self
            .http
            .get(self.file_url(file))
            .bearer_auth(&self.token)
            .header("X-Sync-Mode", "true")
            .send()
            .await
            .map_err(|e| format!("Cannot reach SilverBullet: {e}"))?;
        if !response.status().is_success() {
            return Err(format!("{file}: {}", Self::explain(response.status())));
        }
        response.text().await.map_err(|e| e.to_string())
    }

    /// Creates `file` (for example `Meeting Notes/2026-10-07 Title.md`) only if it does not exist.
    pub async fn create(&self, file: &str, content: &str) -> Result<(), CreateError> {
        let response = self
            .http
            .put(self.file_url(file))
            .bearer_auth(&self.token)
            .header("X-Sync-Mode", "true")
            .header("If-None-Match", "*")
            .header("Content-Type", "text/markdown")
            .body(content.to_string())
            .send()
            .await
            .map_err(|e| CreateError::Other(format!("Cannot reach SilverBullet: {e}")))?;
        match response.status() {
            s if s.is_success() => Ok(()),
            StatusCode::PRECONDITION_FAILED => Err(CreateError::Exists),
            s => Err(CreateError::Other(Self::explain(s))),
        }
    }

    /// Creates a page under `name`, or under `name (2)`, `name (3)` ... if that exists. Returns the page name used.
    pub async fn create_unique(&self, name: &str, content: &str) -> Result<String, String> {
        for n in 1..=20 {
            let candidate = if n == 1 {
                name.to_string()
            } else {
                format!("{name} ({n})")
            };
            match self.create(&format!("{candidate}.md"), content).await {
                Ok(()) => return Ok(candidate),
                Err(CreateError::Exists) => continue,
                Err(CreateError::Other(e)) => return Err(e),
            }
        }
        Err(format!("Too many pages already named '{name}'"))
    }

    /// Pages of the space that may hold project or task text: not the library, not system pages, not the meeting folder.
    fn candidates(files: &[FileMeta], skip_folder: &str) -> Vec<String> {
        let skip = format!("{}/", skip_folder.trim_matches('/'));
        files
            .iter()
            .filter(|f| {
                f.name.ends_with(".md")
                    && f.size <= MAX_PAGE_BYTES
                    && !f.name.starts_with("Library/")
                    && !f.name.starts_with('_')
                    && !f.name.starts_with(&skip)
            })
            .map(|f| f.name.clone())
            .take(MAX_PAGES_SCANNED)
            .collect()
    }

    async fn read_many(&self, files: Vec<String>) -> Vec<(String, String)> {
        stream::iter(files)
            .map(|file| async move {
                let text = self.read(&file).await.ok();
                (file, text)
            })
            .buffer_unordered(8)
            .filter_map(|(file, text)| async move { text.map(|t| (page_name(&file), t)) })
            .collect()
            .await
    }

    /// The projects of the space: pages whose frontmatter tags include `project`.
    pub async fn projects(&self, skip_folder: &str) -> Result<Vec<String>, String> {
        let files = self.list().await?;
        let pages = self.read_many(Self::candidates(&files, skip_folder)).await;
        let mut names: Vec<String> = pages
            .into_iter()
            .filter(|(_, text)| {
                page_tags(text)
                    .iter()
                    .any(|t| t.eq_ignore_ascii_case("project"))
            })
            .map(|(name, _)| name)
            .collect();
        names.sort();
        Ok(names)
    }

    /// The project page, its open tasks and the open tasks elsewhere that link to it.
    pub async fn project_context(
        &self,
        project: &str,
        skip_folder: &str,
    ) -> Result<ProjectContext, String> {
        let page_text = self.read(&format!("{project}.md")).await?;
        let mut tasks: Vec<Task> = open_tasks(&page_text)
            .into_iter()
            .map(|(state, text)| Task {
                page: project.to_string(),
                state,
                text,
            })
            .collect();
        let files = self.list().await?;
        let others: Vec<String> = Self::candidates(&files, skip_folder)
            .into_iter()
            .filter(|f| page_name(f) != project)
            .collect();
        for (page, text) in self.read_many(others).await {
            for line in text.lines() {
                if let Some((state, task)) = parse_task_line(line) {
                    if links_to(line, project)
                        && !tasks.iter().any(|t| t.text == task && t.page == page)
                    {
                        tasks.push(Task {
                            page: page.clone(),
                            state,
                            text: task,
                        });
                    }
                }
            }
        }
        tasks.truncate(MAX_TASKS);
        Ok(ProjectContext {
            name: project.to_string(),
            page_text: page_text.chars().take(MAX_CONTEXT_CHARS).collect(),
            tasks,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Task {
    /// The page the task is written on.
    pub page: String,
    /// The text between the brackets: " " for an ordinary open task, or a custom state such as "IN PROGRESS".
    pub state: String,
    pub text: String,
}

#[derive(Debug, Clone)]
pub struct ProjectContext {
    pub name: String,
    pub page_text: String,
    pub tasks: Vec<Task>,
}

impl ProjectContext {
    /// The text that goes into the prompts. It is the user's own notes, but still data, not instructions.
    pub fn for_prompt(&self) -> String {
        let mut out = format!(
            "Project page \"{}\":\n{}\n\nOpen tasks of the project (page: task):\n",
            self.name,
            self.page_text.trim()
        );
        if self.tasks.is_empty() {
            out.push_str("(none)\n");
        }
        for t in &self.tasks {
            out.push_str(&format!("- {}: {}\n", t.page, t.text));
        }
        out
    }
}

// ---- reading pages ----------------------------------------------------------------------------------------------------------

pub fn page_name(file: &str) -> String {
    file.strip_suffix(".md").unwrap_or(file).to_string()
}

/// Percent-encodes a path for the URL, keeping the `/` between folders and the name.
pub fn encode_path(path: &str) -> String {
    let mut out = String::with_capacity(path.len() + 8);
    for byte in path.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

fn frontmatter(text: &str) -> Option<&str> {
    let rest = text
        .strip_prefix("---\n")
        .or_else(|| text.strip_prefix("---\r\n"))?;
    let end = rest.find("\n---")?;
    Some(&rest[..end])
}

/// The `tags` of a page from its frontmatter: `tags: project`, `tags: [project, x]`, `tags: project, x` or a `- project` list.
pub fn page_tags(text: &str) -> Vec<String> {
    let Some(front) = frontmatter(text) else {
        return Vec::new();
    };
    let clean = |s: &str| {
        s.trim()
            .trim_matches(|c| c == '"' || c == '\'' || c == '#')
            .trim()
            .to_string()
    };
    let mut tags = Vec::new();
    let mut lines = front.lines().peekable();
    while let Some(line) = lines.next() {
        let Some(value) = line.strip_prefix("tags:") else {
            continue;
        };
        let value = value.trim();
        if value.is_empty() {
            while let Some(next) = lines.peek() {
                let trimmed = next.trim_start();
                if let Some(item) = trimmed.strip_prefix("- ") {
                    tags.push(clean(item));
                    lines.next();
                } else {
                    break;
                }
            }
        } else {
            let value = value.trim_start_matches('[').trim_end_matches(']');
            tags.extend(value.split(',').map(clean));
        }
    }
    tags.retain(|t| !t.is_empty());
    tags
}

/// `* [ ] text`, `- [x] text` or `* [IN PROGRESS] text` as (state, text); the state is what is between the brackets.
pub fn parse_task_line(line: &str) -> Option<(String, String)> {
    let trimmed = line.trim_start();
    let rest = trimmed
        .strip_prefix("* ")
        .or_else(|| trimmed.strip_prefix("- "))?;
    let rest = rest.strip_prefix('[')?;
    let end = rest.find(']')?;
    let state = &rest[..end];
    let after = &rest[end + 1..];
    // `[link](url)` is not a task: the closing bracket must be followed by a space.
    if !after.starts_with(char::is_whitespace) {
        return None;
    }
    let text = after.trim();
    if text.is_empty() || state.contains(':') {
        return None;
    }
    Some((state.to_string(), text.to_string()))
}

/// Tasks that are not done (`[x]` is done; any other state counts as open).
pub fn open_tasks(text: &str) -> Vec<(String, String)> {
    text.lines()
        .filter_map(parse_task_line)
        .filter(|(state, _)| !state.eq_ignore_ascii_case("x"))
        .collect()
}

/// Does this line link to `page` (`[[page]]`, `[[page|alias]]`, `[[page#heading]]`)?
pub fn links_to(line: &str, page: &str) -> bool {
    let needle = format!("[[{}", page.to_lowercase());
    let lower = line.to_lowercase();
    let mut from = 0;
    while let Some(pos) = lower[from..].find(&needle) {
        let after = from + pos + needle.len();
        match lower[after..].chars().next() {
            Some(']') | Some('|') | Some('#') | Some('@') => return true,
            _ => from = after,
        }
    }
    false
}

// ---- what a language model may add -------------------------------------------------------------------------------------------

#[derive(Debug, Default, PartialEq)]
pub struct Actions {
    /// New tasks the meeting produced, as plain text.
    pub tasks: Vec<String>,
    /// Existing open tasks that the meeting suggests are done: (the task as it is on its page, why).
    pub completed: Vec<(Task, String)>,
    /// New information for the project.
    pub info: Vec<String>,
}

/// Makes text safe to put into a page: a space can run code from page content, so expression, directive and code-block syntax is
/// broken up (the text stays readable).
pub fn neutralize(text: &str) -> String {
    text.replace("${", "$ {")
        .replace("<!--", "<! --")
        .replace("```", "'''")
        .replace("~~~", "---")
}

/// One safe line of text: no line breaks, no bullets, no links or hashtags (the page builds its own), bounded length.
pub fn clean_line(text: &str) -> String {
    let single: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut s = single.trim().to_string();
    for prefix in ["* [ ] ", "- [ ] ", "* ", "- ", "[ ] "] {
        if let Some(rest) = s.strip_prefix(prefix) {
            s = rest.trim().to_string();
        }
    }
    let s = neutralize(&s)
        .replace("[[", "")
        .replace("]]", "")
        .replace('#', "");
    s.chars()
        .take(MAX_LINE_CHARS)
        .collect::<String>()
        .trim()
        .to_string()
}

fn normalize(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// The language model's JSON reply as `Actions`. Only what passes the checks is kept; a completed task has to be one that really
/// exists (the text on the page is used, not the model's wording).
pub fn parse_actions(reply: &str, open: &[Task]) -> Actions {
    let (Some(start), Some(end)) = (reply.find('{'), reply.rfind('}')) else {
        return Actions::default();
    };
    if end < start {
        return Actions::default();
    }
    let Ok(serde_json::Value::Object(map)) = serde_json::from_str(&reply[start..=end]) else {
        return Actions::default();
    };
    let strings = |key: &str| -> Vec<String> {
        map.get(key)
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str())
                    .map(clean_line)
                    .filter(|s| s.chars().count() >= 3)
                    .take(MAX_ITEMS)
                    .collect()
            })
            .unwrap_or_default()
    };
    let mut actions = Actions {
        tasks: strings("proposed_tasks"),
        info: strings("new_information"),
        completed: Vec::new(),
    };
    if let Some(items) = map.get("possibly_completed").and_then(|v| v.as_array()) {
        for item in items.iter().take(MAX_ITEMS) {
            let (Some(task), evidence) = (
                item.get("task").and_then(|v| v.as_str()),
                item.get("evidence").and_then(|v| v.as_str()).unwrap_or(""),
            ) else {
                continue;
            };
            let wanted = normalize(task);
            if wanted.chars().count() < 8 {
                continue;
            }
            let found = open.iter().find(|t| {
                let have = normalize(&t.text);
                have == wanted || have.contains(&wanted) || wanted.contains(&have)
            });
            if let Some(found) = found {
                if !actions.completed.iter().any(|(t, _)| t == found) {
                    actions
                        .completed
                        .push((found.clone(), clean_line(evidence)));
                }
            }
        }
    }
    actions
}

// ---- the pages -----------------------------------------------------------------------------------------------------------------

/// A page title that follows SilverBullet's name rules (no `|`, `@`, `#`, `[[`, `]]`, `/`, leading dot) and is short.
pub fn safe_title(title: &str) -> String {
    let cleaned: String = title
        .chars()
        .map(|c| match c {
            '|' | '@' | '#' | '[' | ']' | '/' | '\\' | ':' | '?' | ';' | '*' | '"' | '<' | '>'
            | '{' | '}' | '$' | '`' => ' ',
            c if c.is_control() => ' ',
            c => c,
        })
        .collect();
    let single = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    let single: String = single
        .trim_matches(|c: char| c == '.' || c == ' ')
        .chars()
        .take(80)
        .collect();
    if single.trim().is_empty() {
        "Meeting".to_string()
    } else {
        single.trim().to_string()
    }
}

fn yaml_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

pub struct MeetingPage<'a> {
    pub title: &'a str,
    pub date: &'a str,
    pub written_at: &'a str,
    pub project: Option<&'a str>,
    pub minutes: &'a str,
    pub actions: &'a Actions,
    pub transcript_page: Option<&'a str>,
    pub tag: &'a str,
    pub files: usize,
    /// A note about something that did not work (the project context could not be read, for example).
    pub warning: Option<&'a str>,
}

/// The meeting page. Existing pages are never edited: the project is only linked, so SilverBullet's own "Linked Tasks" widget
/// shows the proposed tasks on the project page.
pub fn render_meeting_page(p: &MeetingPage) -> String {
    let mut out = String::new();
    out.push_str("---\ntags: meeting\n");
    if let Some(project) = p.project {
        out.push_str(&format!("project: {}\n", yaml_string(project)));
    }
    out.push_str(&format!("date: {}\ncreatedBy: Handy\n---\n\n", p.date));
    out.push_str(&format!("# {}\n\n", p.title));
    out.push_str(&format!(
        "> Written by Handy on {} from {} recording file(s). The minutes and the sections marked \"proposed\" come from a language \
         model reading the transcript: check them. Handy created this page and did not change any existing page or task.\n\n",
        p.written_at, p.files
    ));
    if let Some(warning) = p.warning {
        out.push_str(&format!("> **Note:** {}\n\n", clean_line(warning)));
    }
    out.push_str(&neutralize(p.minutes.trim()));
    out.push_str("\n\n");
    let link = p.project.map(|n| format!(" [[{n}]]")).unwrap_or_default();
    let tag = p.tag.trim().trim_start_matches('#');
    if !p.actions.tasks.is_empty() {
        out.push_str("## Proposed tasks (from the meeting, not reviewed)\n\n");
        for task in &p.actions.tasks {
            out.push_str(&format!(
                "* [ ] {task}{link}{}\n",
                if tag.is_empty() {
                    String::new()
                } else {
                    format!(" #{tag}")
                }
            ));
        }
        out.push('\n');
    }
    if !p.actions.completed.is_empty() {
        out.push_str(
            "## Possibly completed (existing tasks; nothing was changed, tick them yourself)\n\n",
        );
        for (task, evidence) in &p.actions.completed {
            let why = if evidence.is_empty() {
                String::new()
            } else {
                format!(": {evidence}")
            };
            out.push_str(&format!(
                "* \"{}\" (on [[{}]]){why}\n",
                clean_line(&task.text),
                task.page
            ));
        }
        out.push('\n');
    }
    if !p.actions.info.is_empty() {
        let about = p.project.map(|n| format!(" [[{n}]]")).unwrap_or_default();
        out.push_str(&format!(
            "## Proposed new information for{about} (not added to the project page)\n\n"
        ));
        for line in &p.actions.info {
            out.push_str(&format!("* {line}\n"));
        }
        out.push('\n');
    }
    if let Some(transcript) = p.transcript_page {
        out.push_str(&format!("## Transcript\n\n[[{transcript}]]\n"));
    }
    out
}

pub fn render_transcript_page(
    title: &str,
    date: &str,
    meeting_page: &str,
    transcript: &str,
) -> String {
    format!(
        "---\ntags: transcript\ndate: {date}\ncreatedBy: Handy\n---\n\n# Transcript: {title}\n\nPart of [[{meeting_page}]]. Written by Handy from the recording; \
         it may contain mistakes.\n\n{}\n",
        neutralize(transcript.trim())
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task(page: &str, text: &str) -> Task {
        Task {
            page: page.into(),
            state: " ".into(),
            text: text.into(),
        }
    }

    #[test]
    fn the_saga_page_is_a_project_with_one_open_task() {
        let page = "---\ntags: project\nstatus: active\npriority: high\n---\n\n# Basic Design \n\n* [ ] Implement the crypto management system as a front end. The system owns the mapping, SPI.";
        assert_eq!(page_tags(page), vec!["project"]);
        let tasks = open_tasks(page);
        assert_eq!(tasks.len(), 1);
        assert!(tasks[0].1.starts_with("Implement the crypto"));
    }

    #[test]
    fn tags_come_in_several_yaml_shapes() {
        assert_eq!(
            page_tags("---\ntags: [project, security]\n---\nx"),
            vec!["project", "security"]
        );
        assert_eq!(page_tags("---\ntags: \"project\"\n---\nx"), vec!["project"]);
        assert_eq!(
            page_tags("---\nstatus: a\ntags:\n- meeting\n- project\nother: 1\n---\nx"),
            vec!["meeting", "project"]
        );
        assert!(page_tags("no frontmatter").is_empty());
        assert!(page_tags("---\ntitle: x\n---\n").is_empty());
    }

    #[test]
    fn task_lines_and_states() {
        assert_eq!(
            parse_task_line("* [ ] Do it"),
            Some((" ".into(), "Do it".into()))
        );
        assert_eq!(
            parse_task_line("  - [x] Done"),
            Some(("x".into(), "Done".into()))
        );
        assert_eq!(
            parse_task_line("* [IN PROGRESS] Work"),
            Some(("IN PROGRESS".into(), "Work".into()))
        );
        assert_eq!(parse_task_line("* [link](url) text"), None);
        assert_eq!(parse_task_line("plain text"), None);
        let open = open_tasks("* [ ] a\n* [x] b\n* [X] c\n* [WAITING] d");
        assert_eq!(
            open.iter().map(|t| t.1.as_str()).collect::<Vec<_>>(),
            vec!["a", "d"]
        );
    }

    #[test]
    fn links_to_a_page_in_all_wikilink_forms() {
        assert!(links_to("* [ ] Send it [[Saga]]", "Saga"));
        assert!(links_to("see [[saga|the project]]", "Saga"));
        assert!(links_to("see [[Saga#Design]]", "Saga"));
        assert!(!links_to("see [[Saga Extra]]", "Saga"));
        assert!(!links_to("no link to Saga", "Saga"));
    }

    #[test]
    fn page_names_and_urls() {
        assert_eq!(page_name("Meeting Notes/x.md"), "Meeting Notes/x");
        assert_eq!(
            encode_path("Meeting Notes/2026-10-07 Æble.md"),
            "Meeting%20Notes/2026-10-07%20%C3%86ble.md"
        );
        let space = Space::new("http://h:3000/notes/", "t").unwrap();
        assert_eq!(
            space.page_url("Meeting Notes/a b"),
            "http://h:3000/notes/Meeting%20Notes/a%20b"
        );
        assert!(Space::new("h:3000", "t").is_err());
        assert!(Space::new("http://h", "").is_err());
    }

    #[test]
    fn titles_follow_the_name_rules() {
        assert_eq!(
            safe_title("Q3 plan: budget | risks #1 [[x]]"),
            "Q3 plan budget risks 1 x"
        );
        assert_eq!(safe_title("../.hidden"), "hidden");
        assert_eq!(safe_title("  "), "Meeting");
        assert_eq!(safe_title("a${evil}b"), "a evil b");
        assert!(safe_title(&"x".repeat(300)).chars().count() <= 80);
    }

    #[test]
    fn text_that_could_run_is_neutralised() {
        let bad = "Result: ${editor.flashNotification('x')}\n<!-- #lua os.execute('x') -->\n```space-lua\nrun()\n```";
        let safe = neutralize(bad);
        assert!(!safe.contains("${") && !safe.contains("<!--") && !safe.contains("```"));
        let line = clean_line("* [ ] Fix it ${boom} [[Other]] #urgent\nsecond line");
        assert_eq!(line, "Fix it $ {boom} Other urgent second line");
    }

    #[test]
    fn the_model_reply_is_checked_before_use() {
        let open = vec![
            task(
                "Saga",
                "Implement the crypto management system as a front end",
            ),
            task("Meeting Notes/x", "Review the SPI draft"),
        ];
        let reply = r#"Here: {"proposed_tasks": ["Write the key rotation spec", "* [ ] Ask Kari ${x}", ""],
            "possibly_completed": [{"task": "review the spi draft", "evidence": "It was reviewed and approved"},
                                   {"task": "Invented task that does not exist", "evidence": "n/a"}],
            "new_information": ["Deadline moved to March"]}"#;
        let a = parse_actions(reply, &open);
        assert_eq!(
            a.tasks,
            vec!["Write the key rotation spec", "Ask Kari $ {x}"]
        );
        assert_eq!(a.completed.len(), 1);
        assert_eq!(a.completed[0].0.page, "Meeting Notes/x");
        assert_eq!(a.info, vec!["Deadline moved to March"]);
        assert_eq!(parse_actions("no json", &open), Actions::default());
        assert_eq!(parse_actions("{}", &open), Actions::default());
    }

    #[test]
    fn the_meeting_page_links_the_project_and_marks_what_is_proposed() {
        let actions = Actions {
            tasks: vec!["Write the key rotation spec".into()],
            completed: vec![(
                task("Saga", "Review the SPI draft"),
                "approved in the meeting".into(),
            )],
            info: vec!["Deadline moved to March".into()],
        };
        let page = render_meeting_page(&MeetingPage {
            title: "Key management",
            date: "2026-10-07",
            written_at: "2026-10-07 10:00",
            project: Some("Saga"),
            minutes: "## Decisions\n- Use ${bad} keys",
            actions: &actions,
            transcript_page: Some("Meeting Notes/2026-10-07 Key management transcript"),
            tag: "fromMeeting",
            files: 2,
            warning: None,
        });
        assert!(page.starts_with("---\ntags: meeting\nproject: \"Saga\"\ndate: 2026-10-07\n"));
        assert!(page.contains("* [ ] Write the key rotation spec [[Saga]] #fromMeeting\n"));
        assert!(page.contains(
            "Possibly completed (existing tasks; nothing was changed, tick them yourself)"
        ));
        assert!(page.contains("* \"Review the SPI draft\" (on [[Saga]]): approved in the meeting"));
        assert!(
            page.contains("Proposed new information for [[Saga]] (not added to the project page)")
        );
        assert!(page.contains("[[Meeting Notes/2026-10-07 Key management transcript]]"));
        assert!(!page.contains("${bad}"));
        // an existing task is only ever quoted, never written as a task line
        assert!(!page.contains("* [ ] Review the SPI draft"));
    }

    #[test]
    fn a_meeting_without_a_project_has_no_link() {
        let page = render_meeting_page(&MeetingPage {
            title: "Standup",
            date: "2026-10-07",
            written_at: "now",
            project: None,
            minutes: "Notes",
            actions: &Actions {
                tasks: vec!["Do x".into()],
                ..Default::default()
            },
            transcript_page: None,
            tag: "",
            files: 1,
            warning: Some("The project could not be read."),
        });
        assert!(page.contains("* [ ] Do x\n") && !page.contains("[["));
        assert!(page.contains("> **Note:** The project could not be read."));
    }

    #[tokio::test]
    async fn create_only_and_unique_names_against_a_server() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        // first PUT: 412 (exists), second PUT: 200
        let server = tokio::spawn(async move {
            let mut seen = Vec::new();
            for status in ["412 Precondition Failed", "200 OK"] {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut buffer = vec![0u8; 16384];
                let mut got = Vec::new();
                loop {
                    let n = socket.read(&mut buffer).await.unwrap();
                    got.extend_from_slice(&buffer[..n]);
                    if n == 0 || got.windows(4).any(|w| w == b"\r\n\r\n") && got.ends_with(b"body")
                    {
                        break;
                    }
                }
                socket
                    .write_all(
                        format!(
                            "HTTP/1.1 {status}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                        )
                        .as_bytes(),
                    )
                    .await
                    .unwrap();
                seen.push(String::from_utf8_lossy(&got).to_string());
            }
            seen
        });
        let space = Space::new(&format!("http://127.0.0.1:{port}/notes"), "secret").unwrap();
        let used = space
            .create_unique("Meeting Notes/2026 Plan", "body")
            .await
            .unwrap();
        assert_eq!(used, "Meeting Notes/2026 Plan (2)");
        let seen = server.await.unwrap();
        assert!(seen[0].starts_with("PUT /notes/.fs/Meeting%20Notes/2026%20Plan.md "));
        assert!(seen[1].starts_with("PUT /notes/.fs/Meeting%20Notes/2026%20Plan%20%282%29.md "));
        let lower = seen[0].to_lowercase();
        assert!(
            lower.contains("authorization: bearer secret")
                && lower.contains("if-none-match: *")
                && lower.contains("x-sync-mode: true")
        );
    }

    /// Against a real SilverBullet space; run by hand:
    /// `SB_URL=http://host:3000/notes SB_TOKEN=... [SB_WRITE=1] cargo test --lib live_space -- --ignored --nocapture`
    #[tokio::test]
    #[ignore]
    async fn live_space() {
        let (url, token) = (
            std::env::var("SB_URL").unwrap(),
            std::env::var("SB_TOKEN").unwrap(),
        );
        let space = Space::new(&url, &token).unwrap();
        let projects = space.projects("Meeting Notes").await.unwrap();
        println!("projects: {projects:?}");
        let project = std::env::var("SB_PROJECT")
            .unwrap_or_else(|_| projects.first().cloned().unwrap_or_default());
        let ctx = space
            .project_context(&project, "Meeting Notes")
            .await
            .unwrap();
        println!("{}", ctx.for_prompt());
        if std::env::var("SB_WRITE").is_err() {
            return;
        }
        let actions = Actions {
            tasks: vec![
                "Write the key rotation specification (test)".into(),
                "Ask the vendor about SPI support (test)".into(),
            ],
            completed: ctx
                .tasks
                .first()
                .map(|t| vec![(t.clone(), "the test pretends it was finished".to_string())])
                .unwrap_or_default(),
            info: vec!["The deadline moved to March (test)".into()],
        };
        let name = "Meeting Notes/2026-10-07 Handy test (delete me)";
        let transcript_name = format!("{name} transcript");
        let page = render_meeting_page(&MeetingPage {
            title: "Handy test (delete me)",
            date: "2026-10-07",
            written_at: "2026-10-07 test",
            project: Some(&project),
            minutes: "## Summary\nThis page was created by a Handy test. Delete it.\n\n## Decisions\n- Nothing real.",
            actions: &actions,
            transcript_page: Some(&transcript_name),
            tag: "fromMeeting",
            files: 1,
            warning: None,
        });
        let used = space.create_unique(name, &page).await.unwrap();
        let transcript = render_transcript_page(
            "Handy test (delete me)",
            "2026-10-07",
            &used,
            "[00:00] Speaker 1: This is a test transcript.",
        );
        let used_t = space
            .create_unique(&transcript_name, &transcript)
            .await
            .unwrap();
        println!(
            "created: {} and {}",
            space.page_url(&used),
            space.page_url(&used_t)
        );
        let back = space.read(&format!("{used}.md")).await.unwrap();
        assert!(back.contains("#fromMeeting"));
    }
}
