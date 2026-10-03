//! A numbered prompt picker in a small floating window, like SuperWhisper's mode list.
//!
//! The shortcut (or `handy --prompt-picker`) opens the window; it lists the post-processing prompts with numbers. Pressing
//! a number key or clicking an entry selects that prompt, Escape (or the shortcut again) closes it, and it closes itself
//! after a few seconds. The window never takes focus, so the app being dictated into keeps its selection. While it is
//! open the number keys and Escape are registered as temporary global shortcuts (they are swallowed, so no digit is
//! typed into the document); they are released when it closes.

use crate::actions::{set_prompt_by_id, ShortcutAction};
use crate::settings::{get_settings, ShortcutBinding};
use log::{info, warn};
use serde::Serialize;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, WebviewWindowBuilder};

pub const MAX_ITEMS: usize = 9;
const WINDOW_LABEL: &str = "prompt_picker";
const WIDTH: f64 = 340.0;
const HEADER: f64 = 46.0;
const ROW: f64 = 40.0;
const PADDING: f64 = 16.0;
const AUTO_CLOSE: Duration = Duration::from_secs(12);

static OPEN: AtomicBool = AtomicBool::new(false);
static GENERATION: AtomicU64 = AtomicU64::new(0);
/// Prompt ids shown, in order; entry `n - 1` is chosen by number `n`.
static SHOWN: Mutex<Vec<String>> = Mutex::new(Vec::new());
/// What the open picker shows. The window is created on first use and its page may not have loaded yet when the
/// "picker-open" event is sent, so the page also asks for this when it starts.
static CURRENT: Mutex<Option<OpenPayload>> = Mutex::new(None);

#[derive(Serialize, Clone, specta::Type)]
struct Item {
    number: usize,
    id: String,
    name: String,
}

#[derive(Serialize, Clone, specta::Type)]
pub struct OpenPayload {
    items: Vec<Item>,
    selected: Option<String>,
}

fn window_height(count: usize) -> f64 {
    HEADER + ROW * count as f64 + PADDING
}

/// The prompts to list: the first nine, in settings order.
fn pick_items(prompts: &[(String, String)]) -> Vec<Item> {
    prompts
        .iter()
        .take(MAX_ITEMS)
        .enumerate()
        .map(|(i, (id, name))| Item {
            number: i + 1,
            id: id.clone(),
            name: name.clone(),
        })
        .collect()
}

fn temp_binding(id: &str, key: &str) -> ShortcutBinding {
    ShortcutBinding {
        id: id.to_string(),
        name: id.to_string(),
        description: String::new(),
        default_binding: key.to_string(),
        current_binding: key.to_string(),
    }
}

fn temp_bindings(count: usize) -> Vec<ShortcutBinding> {
    let mut list: Vec<ShortcutBinding> = (1..=count)
        .map(|n| temp_binding(&format!("picker_{n}"), &n.to_string()))
        .collect();
    list.push(temp_binding("picker_close", "escape"));
    list
}

fn ensure_window(app: &AppHandle) -> Option<tauri::WebviewWindow> {
    if let Some(window) = app.get_webview_window(WINDOW_LABEL) {
        return Some(window);
    }
    let (tx, rx) = std::sync::mpsc::channel();
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        let mut builder = WebviewWindowBuilder::new(
            &handle,
            WINDOW_LABEL,
            tauri::WebviewUrl::App("src/picker/index.html".into()),
        )
        .title("Prompts")
        .resizable(false)
        .inner_size(WIDTH, window_height(MAX_ITEMS))
        .shadow(false)
        .maximizable(false)
        .minimizable(false)
        .closable(false)
        .accept_first_mouse(true)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .transparent(true)
        .focusable(false)
        .focused(false)
        .visible(false);
        if let Some(data_dir) = crate::portable::data_dir() {
            builder = builder.data_directory(data_dir.join("webview"));
        }
        let _ = tx.send(builder.build().ok());
    });
    rx.recv_timeout(Duration::from_secs(5)).ok().flatten()
}

/// Shows the window centred on the monitor under the mouse. Runs on the main thread.
fn place_and_show(app: &AppHandle, window: &tauri::WebviewWindow, count: usize) {
    let height = window_height(count);
    let _ = window.set_size(tauri::Size::Logical(tauri::LogicalSize {
        width: WIDTH,
        height,
    }));
    let place = |window: &tauri::WebviewWindow| {
        if let Some(monitor) = crate::overlay::get_monitor_with_cursor(app) {
            let scale = monitor.scale_factor();
            let x =
                monitor.position().x as f64 + (monitor.size().width as f64 - WIDTH * scale) / 2.0;
            let y =
                monitor.position().y as f64 + (monitor.size().height as f64 - height * scale) / 3.0;
            let _ = window.set_position(tauri::Position::Physical(tauri::PhysicalPosition {
                x: x as i32,
                y: y as i32,
            }));
        } else {
            let _ = window.center();
        }
    };
    place(window);
    let _ = window.show();
    // The first placement can be undone by a DPI change when the window moves between monitors.
    place(window);
    #[cfg(target_os = "windows")]
    crate::overlay::force_overlay_topmost(window);
}

pub fn is_open() -> bool {
    OPEN.load(Ordering::SeqCst)
}

/// Opens the picker, or closes it when it is already open.
pub fn toggle(app: &AppHandle) {
    if is_open() {
        close(app);
    } else {
        open(app);
    }
}

fn open(app: &AppHandle) {
    let settings = get_settings(app);
    let prompts: Vec<(String, String)> = settings
        .post_process_prompts
        .iter()
        .filter(|p| !crate::extras::is_transform_prompt(&p.id))
        .map(|p| (p.id.clone(), p.name.clone()))
        .collect();
    let items = pick_items(&prompts);
    if items.is_empty() {
        crate::learn::announce(app, "reformat-setup", String::new());
        return;
    }
    let Some(window) = ensure_window(app) else {
        warn!("Prompt picker: the window could not be created");
        return;
    };
    if OPEN.swap(true, Ordering::SeqCst) {
        return;
    }
    let generation = GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    if let Ok(mut shown) = SHOWN.lock() {
        *shown = items.iter().map(|i| i.id.clone()).collect();
    }

    let payload = OpenPayload {
        items: items.clone(),
        selected: settings.post_process_selected_prompt_id.clone(),
    };
    if let Ok(mut current) = CURRENT.lock() {
        *current = Some(payload.clone());
    }
    let _ = window.emit("picker-open", payload);
    let handle = app.clone();
    let win = window.clone();
    let count = items.len();
    let _ = app.run_on_main_thread(move || place_and_show(&handle, &win, count));

    for binding in temp_bindings(items.len()) {
        if let Err(err) = crate::shortcut::register_shortcut(app, binding.clone()) {
            warn!(
                "Prompt picker: could not register '{}': {}",
                binding.current_binding, err
            );
        }
    }
    info!("Prompt picker opened with {} prompts", items.len());

    // Close by itself if nothing is chosen.
    let handle = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(AUTO_CLOSE);
        if is_open() && GENERATION.load(Ordering::SeqCst) == generation {
            close(&handle);
        }
    });
}

pub fn close(app: &AppHandle) {
    if !OPEN.swap(false, Ordering::SeqCst) {
        return;
    }
    if let Ok(mut current) = CURRENT.lock() {
        *current = None;
    }
    let count = SHOWN.lock().map(|s| s.len()).unwrap_or(MAX_ITEMS);
    for binding in temp_bindings(count) {
        let _ = crate::shortcut::unregister_shortcut(app, binding);
    }
    if let Some(window) = app.get_webview_window(WINDOW_LABEL) {
        let _ = window.emit("picker-close", ());
        let win = window.clone();
        let _ = app.run_on_main_thread(move || {
            let _ = win.hide();
        });
    }
}

/// Chooses the prompt with this number (1-based) and closes the picker.
pub fn choose(app: &AppHandle, number: usize) {
    if !is_open() {
        return;
    }
    let id = SHOWN
        .lock()
        .ok()
        .and_then(|shown| shown.get(number.wrapping_sub(1)).cloned());
    close(app);
    match id {
        Some(id) => set_prompt_by_id(app, &id),
        None => warn!("Prompt picker: no prompt with number {}", number),
    }
}

// Shortcut events arrive on the thread that owns the keyboard hook, and (un)registering talks to that same thread, so
// every action hands its work to a new thread.

pub(crate) struct PickerToggleAction;

impl ShortcutAction for PickerToggleAction {
    fn start(&self, app: &AppHandle, _binding_id: &str, _shortcut_str: &str) {
        let app = app.clone();
        std::thread::spawn(move || toggle(&app));
    }

    fn stop(&self, _app: &AppHandle, _binding_id: &str, _shortcut_str: &str) {}
}

pub(crate) struct PickerChoiceAction {
    pub number: usize,
}

impl ShortcutAction for PickerChoiceAction {
    fn start(&self, app: &AppHandle, _binding_id: &str, _shortcut_str: &str) {
        let app = app.clone();
        let number = self.number;
        std::thread::spawn(move || choose(&app, number));
    }

    fn stop(&self, _app: &AppHandle, _binding_id: &str, _shortcut_str: &str) {}
}

pub(crate) struct PickerCloseAction;

impl ShortcutAction for PickerCloseAction {
    fn start(&self, app: &AppHandle, _binding_id: &str, _shortcut_str: &str) {
        let app = app.clone();
        std::thread::spawn(move || close(&app));
    }

    fn stop(&self, _app: &AppHandle, _binding_id: &str, _shortcut_str: &str) {}
}

/// The window's own click handler.
#[tauri::command]
#[specta::specta]
pub fn picker_select(app: AppHandle, number: usize) {
    std::thread::spawn(move || choose(&app, number));
}

/// The list to show right now, or nothing when the picker is closed (asked for by the page when it starts).
#[tauri::command]
#[specta::specta]
pub fn picker_state() -> Option<OpenPayload> {
    CURRENT.lock().ok().and_then(|current| current.clone())
}

#[tauri::command]
#[specta::specta]
pub fn picker_close(app: AppHandle) {
    std::thread::spawn(move || close(&app));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prompts(n: usize) -> Vec<(String, String)> {
        (0..n)
            .map(|i| (format!("id{i}"), format!("Name {i}")))
            .collect()
    }

    #[test]
    fn items_are_numbered_from_one_and_capped_at_nine() {
        let items = pick_items(&prompts(12));
        assert_eq!(items.len(), MAX_ITEMS);
        assert_eq!(items[0].number, 1);
        assert_eq!(items[0].id, "id0");
        assert_eq!(items[8].number, 9);
        assert!(pick_items(&[]).is_empty());
    }

    #[test]
    fn temporary_bindings_are_the_digits_and_escape() {
        let b = temp_bindings(3);
        let keys: Vec<(&str, &str)> = b
            .iter()
            .map(|b| (b.id.as_str(), b.current_binding.as_str()))
            .collect();
        assert_eq!(
            keys,
            vec![
                ("picker_1", "1"),
                ("picker_2", "2"),
                ("picker_3", "3"),
                ("picker_close", "escape")
            ]
        );
    }

    #[test]
    fn window_grows_with_the_number_of_prompts() {
        assert!(window_height(9) > window_height(3));
    }
}
