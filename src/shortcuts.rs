//! Keyboard shortcuts: which keys do what, and matching key presses.
//!
//! As in the Speech Output Engine, the shortcuts are kept clear of the keys
//! screen readers and people rely on:
//! * no single letters, digits or punctuation, which people type and screen
//!   readers use for quick navigation (WCAG 2.1.4);
//! * no Alt or Option, because Ctrl+Alt is AltGr on Windows and Option types
//!   characters on a Mac;
//! * no Tab, Enter, Space, arrow keys or editing shortcuts such as Ctrl+C.
//!
//! Modifiers must match exactly, so typing "đ" with AltGr+D never deletes.
//! All the shortcuts can be turned off on the Settings tab.

use egui::{Event, Key, Modifiers};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Platform {
    Windows,
    Mac,
    Linux,
}

impl Platform {
    #[cfg(test)]
    pub const ALL: [Platform; 3] = [Platform::Windows, Platform::Mac, Platform::Linux];

    pub const CURRENT: Platform = if cfg!(target_os = "macos") {
        Platform::Mac
    } else if cfg!(target_os = "windows") {
        Platform::Windows
    } else {
        Platform::Linux
    };
}

/// Modifier keys. `command` is Cmd on a Mac and Ctrl elsewhere. `ctrl` is
/// the Control key on a Mac, and is never set elsewhere.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Mods {
    pub command: bool,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
}

impl Mods {
    /// The modifiers held for a key press.
    fn pressed(m: Modifiers, platform: Platform) -> Self {
        match platform {
            Platform::Mac => Mods { command: m.mac_cmd, ctrl: m.ctrl, shift: m.shift, alt: m.alt },
            _ => Mods { command: m.ctrl || m.command, ctrl: false, shift: m.shift, alt: m.alt },
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Shortcut {
    pub mods: Mods,
    pub key: Key,
}

impl Shortcut {
    const fn key(key: Key) -> Self {
        Shortcut { mods: Mods { command: false, ctrl: false, shift: false, alt: false }, key }
    }

    /// Cmd on a Mac, Ctrl elsewhere.
    const fn command(key: Key) -> Self {
        Shortcut { mods: Mods { command: true, ctrl: false, shift: false, alt: false }, key }
    }

    /// The Control key on every platform, as in Ctrl+Tab.
    fn ctrl(key: Key, shift: bool, platform: Platform) -> Self {
        let mac = platform == Platform::Mac;
        Shortcut { mods: Mods { command: !mac, ctrl: mac, shift, alt: false }, key }
    }

    /// As shown on buttons and lists, such as "Ctrl+N" or "Esc".
    pub fn short(self, platform: Platform) -> String {
        let mac = platform == Platform::Mac;
        let mut parts = Vec::new();
        if self.mods.ctrl {
            parts.push(t!("key-ctrl"));
        }
        if self.mods.command {
            parts.push(if mac { t!("key-cmd") } else { t!("key-ctrl") });
        }
        if self.mods.alt {
            parts.push(if mac { t!("key-option") } else { t!("key-alt") });
        }
        if self.mods.shift {
            parts.push(t!("key-shift"));
        }
        let key = match self.key {
            Key::Escape => t!("key-esc"),
            Key::OpenBracket => "[".to_owned(),
            Key::CloseBracket => "]".to_owned(),
            key => key.name().to_owned(),
        };
        parts.push(key);
        parts.join("+")
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Action {
    /// Start a new entry, or a new category on the Categories tab.
    New,
    /// Save the entry or category being edited.
    Save,
    /// Edit the selected entry.
    Edit,
    /// Delete the selected entry or category.
    Delete,
    /// Go to the box that finds entries.
    Find,
    PreviousMonth,
    NextMonth,
    ThisMonth,
    /// Say how much has been spent this month against the budget.
    Summary,
    Import,
    Export,
    /// Ctrl+1 goes to the first tab, Ctrl+2 to the second and so on.
    GoToTab,
    NextTab,
    PreviousTab,
    Help,
}

impl Action {
    pub fn label(self) -> String {
        match self {
            Action::New => t!("action-new"),
            Action::Save => t!("action-save"),
            Action::Edit => t!("action-edit"),
            Action::Delete => t!("action-delete"),
            Action::Find => t!("action-find"),
            Action::PreviousMonth => t!("action-previous-month"),
            Action::NextMonth => t!("action-next-month"),
            Action::ThisMonth => t!("action-this-month"),
            Action::Summary => t!("action-summary"),
            Action::Import => t!("action-import"),
            Action::Export => t!("action-export"),
            Action::GoToTab => t!("action-go-to-tab"),
            Action::NextTab => t!("action-next-tab"),
            Action::PreviousTab => t!("action-previous-tab"),
            Action::Help => t!("action-help"),
        }
    }
}

/// What a key press asks the app to do.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Command {
    Run(Action),
    /// Go to the tab with this index, counting from 0.
    Tab(usize),
    /// Close the open dropdown list, keeping focus on its dropdown.
    CloseList,
    /// Escape: stop editing, or clear the form.
    Cancel,
}

/// What the window looks like when a key is pressed.
#[derive(Clone, Copy, Debug, Default)]
pub struct KeyContext {
    /// A dropdown list is open, so keys belong to it.
    pub popup_open: bool,
    /// The index of the tab button that has focus, if one has.
    pub focused_tab: Option<usize>,
    pub tab_count: usize,
}

#[derive(Clone, Debug)]
pub struct Keymap {
    platform: Platform,
    enabled: bool,
    bindings: Vec<(Action, Shortcut)>,
}

impl Keymap {
    pub fn new(enabled: bool, platform: Platform) -> Self {
        use Shortcut as S;
        let bindings = vec![
            (Action::New, S::command(Key::N)),
            (Action::Save, S::command(Key::S)),
            (Action::Edit, S::key(Key::F2)),
            (Action::Delete, S::command(Key::D)),
            (Action::Find, S::command(Key::F)),
            (Action::PreviousMonth, S::key(Key::F3)),
            // A Mac's function keys control the computer unless Fn is held,
            // so the months also have keys that need no Fn.
            (Action::PreviousMonth, S::command(Key::OpenBracket)),
            (Action::NextMonth, S::key(Key::F4)),
            (Action::NextMonth, S::command(Key::CloseBracket)),
            (Action::ThisMonth, S::key(Key::F5)),
            (Action::Summary, S::key(Key::F7)),
            (Action::Import, S::command(Key::I)),
            (Action::Export, S::command(Key::E)),
            (Action::GoToTab, S::command(Key::Num1)),
            (Action::NextTab, S::ctrl(Key::Tab, false, platform)),
            (Action::PreviousTab, S::ctrl(Key::Tab, true, platform)),
            (Action::Help, S::key(Key::F1)),
        ];
        Keymap { platform, enabled, bindings }
    }

    /// " (Ctrl+N)" to follow a button label, or nothing if `action` has no
    /// shortcut at the moment.
    pub fn hint(&self, action: Action) -> String {
        if !self.enabled {
            return String::new();
        }
        self.bindings
            .iter()
            .find(|(a, _)| *a == action)
            .map(|(a, s)| format!(" ({})", self.describe(*a, *s)))
            .unwrap_or_default()
    }

    fn describe(&self, action: Action, shortcut: Shortcut) -> String {
        if action == Action::GoToTab {
            let last = Shortcut { key: Key::Num5, ..shortcut };
            return t!("shortcuts-range", first = shortcut.short(self.platform), last = last.short(self.platform));
        }
        shortcut.short(self.platform)
    }

    /// Takes the key presses that are shortcuts out of `events`, so the
    /// controls never see them, and returns what they ask for. `ready` says
    /// whether an action can run now; when it can't, its key is left alone
    /// for the controls.
    ///
    /// While a tab has focus, the Left and Right arrows, Home and End move
    /// between tabs, as in the ARIA tabs pattern. Escape is always taken:
    /// egui would otherwise take focus away from the control, leaving
    /// screen reader users nowhere. It closes an open list, or cancels.
    /// Typed text is never touched. Held keys only repeat moving between
    /// tabs and months.
    pub fn take(&self, events: &mut Vec<Event>, cx: KeyContext, ready: impl Fn(Action) -> bool) -> Vec<Command> {
        let mut commands = Vec::new();
        events.retain(|event| {
            let Event::Key { key, pressed: true, repeat, modifiers, .. } = event else { return true };
            if *key == Key::Escape && modifiers.is_none() {
                if !*repeat {
                    commands.push(if cx.popup_open { Command::CloseList } else { Command::Cancel });
                }
                return false;
            }
            match self.command_for(*key, *modifiers, cx, &ready) {
                Some(Command::Run(action))
                    if *repeat
                        && !matches!(
                            action,
                            Action::NextTab | Action::PreviousTab | Action::PreviousMonth | Action::NextMonth
                        ) =>
                {
                    false
                }
                Some(command) => {
                    commands.push(command);
                    false
                }
                None => true,
            }
        });
        commands
    }

    fn command_for(&self, key: Key, modifiers: Modifiers, cx: KeyContext, ready: &impl Fn(Action) -> bool) -> Option<Command> {
        if let Some(i) = cx.focused_tab
            && modifiers.is_none()
            && cx.tab_count > 0
        {
            let last = cx.tab_count - 1;
            let to = match key {
                Key::ArrowLeft => Some(if i == 0 { last } else { i - 1 }),
                Key::ArrowRight => Some(if i >= last { 0 } else { i + 1 }),
                Key::Home => Some(0),
                Key::End => Some(last),
                _ => None,
            };
            if let Some(to) = to {
                return Some(Command::Tab(to));
            }
        }
        if cx.popup_open || !self.enabled {
            return None;
        }
        let pressed = Mods::pressed(modifiers, self.platform);
        for (action, binding) in &self.bindings {
            if binding.mods != pressed {
                continue;
            }
            if *action == Action::GoToTab {
                if let Some(n) = digit(key)
                    && (1..=cx.tab_count).contains(&n)
                {
                    return Some(Command::Tab(n - 1));
                }
            } else if binding.key == key && ready(*action) {
                return Some(Command::Run(*action));
            }
        }
        None
    }

    /// The quick reference shown by F1.
    pub fn reference(&self) -> String {
        let mut out = String::new();
        if self.enabled {
            let mut listed: Vec<(Action, Vec<String>)> = Vec::new();
            for (action, shortcut) in &self.bindings {
                let text = self.describe(*action, *shortcut);
                match listed.iter_mut().find(|(a, _)| a == action) {
                    Some((_, keys)) => keys.push(text),
                    None => listed.push((*action, vec![text])),
                }
            }
            for (action, keys) in listed {
                let keys = keys.into_iter().reduce(|first, second| t!("shortcuts-either", first = first, second = second));
                out.push_str(&t!("shortcuts-line", action = action.label(), keys = keys.unwrap_or_default()));
                out.push('\n');
            }
        } else {
            out.push_str(&t!("shortcuts-off-note"));
            out.push('\n');
        }
        out.push('\n');
        out.push_str(&t!("shortcuts-always"));
        out
    }
}

/// The digit on a key, for Ctrl+1 to Ctrl+5.
fn digit(key: Key) -> Option<usize> {
    let name = key.name();
    (name.len() == 1).then(|| name.parse().ok()).flatten()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(key: Key, modifiers: Modifiers) -> Event {
        Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers }
    }

    fn cx() -> KeyContext {
        KeyContext { popup_open: false, focused_tab: None, tab_count: 5 }
    }

    #[test]
    fn shortcuts_are_unique_and_never_plain_characters() {
        for p in Platform::ALL {
            let map = Keymap::new(true, p);
            for (i, (action, s)) in map.bindings.iter().enumerate() {
                assert!(!s.mods.alt, "{action:?} uses Alt");
                let typed = !s.mods.command && !s.mods.ctrl;
                assert!(!(typed && s.key.name().chars().count() == 1), "{action:?} is a typed key");
                assert!(!map.bindings[i + 1..].iter().any(|(_, other)| other == s), "{} twice", s.short(p));
            }
        }
    }

    #[test]
    fn ctrl_letters_run_but_altgr_types() {
        let map = Keymap::new(true, Platform::Windows);
        let altgr = Modifiers { alt: true, ctrl: true, command: true, ..Modifiers::NONE };
        let mut events = vec![press(Key::D, altgr), Event::Text("đ".into()), press(Key::N, Modifiers::COMMAND)];
        let commands = map.take(&mut events, cx(), |_| true);
        assert_eq!(commands, vec![Command::Run(Action::New)]);
        assert_eq!(events.len(), 2, "AltGr+D and its text are left for the text box");
    }

    #[test]
    fn tabs_and_months() {
        let map = Keymap::new(true, Platform::Windows);
        let mut events = vec![
            press(Key::Num3, Modifiers::COMMAND),
            press(Key::Num9, Modifiers::COMMAND),
            press(Key::Tab, Modifiers::COMMAND),
            press(Key::F3, Modifiers::NONE),
            press(Key::CloseBracket, Modifiers::COMMAND),
        ];
        let commands = map.take(&mut events, cx(), |_| true);
        assert_eq!(
            commands,
            vec![
                Command::Tab(2),
                Command::Run(Action::NextTab),
                Command::Run(Action::PreviousMonth),
                Command::Run(Action::NextMonth)
            ]
        );
        assert_eq!(events.len(), 1, "Ctrl+9 is left alone with only five tabs");
    }

    #[test]
    fn arrows_move_along_focused_tabs() {
        let map = Keymap::new(false, Platform::Windows);
        let on_tab = KeyContext { focused_tab: Some(0), ..cx() };
        let mut events = vec![press(Key::ArrowLeft, Modifiers::NONE), press(Key::End, Modifiers::NONE)];
        assert_eq!(map.take(&mut events, on_tab, |_| true), vec![Command::Tab(4), Command::Tab(4)]);
        let mut events = vec![press(Key::ArrowLeft, Modifiers::NONE)];
        assert!(map.take(&mut events, cx(), |_| true).is_empty(), "only while a tab has focus");
    }

    #[test]
    fn escape_is_always_taken() {
        let map = Keymap::new(false, Platform::Windows);
        let mut events = vec![press(Key::Escape, Modifiers::NONE)];
        assert_eq!(map.take(&mut events, cx(), |_| true), vec![Command::Cancel]);
        assert!(events.is_empty());
        let open = KeyContext { popup_open: true, ..cx() };
        let mut events = vec![press(Key::Escape, Modifiers::NONE)];
        assert_eq!(map.take(&mut events, open, |_| true), vec![Command::CloseList]);
    }

    #[test]
    fn off_means_off_and_unready_keys_are_left() {
        let off = Keymap::new(false, Platform::Windows);
        let mut events = vec![press(Key::S, Modifiers::COMMAND)];
        assert!(off.take(&mut events, cx(), |_| true).is_empty());
        assert_eq!(events.len(), 1);
        assert_eq!(off.hint(Action::Save), "");

        let on = Keymap::new(true, Platform::Mac);
        assert_eq!(on.hint(Action::Save), " (Cmd+S)");
        assert_eq!(on.hint(Action::GoToTab), " (Cmd+1 to Cmd+5)");
        let mut events = vec![press(Key::D, Modifiers::MAC_CMD)];
        assert!(on.take(&mut events, cx(), |a| a != Action::Delete).is_empty());
        assert_eq!(events.len(), 1);
    }
}
