use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Down,
    Up,
    Top,
    Bottom,
    HalfDown,
    HalfUp,
    Open,
    Back,
    ToggleFocus,
    Jump(usize),
    Search,
    PlayPause,
    Next,
    Previous,
    SeekForward,
    SeekBack,
    VolumeUp,
    VolumeDown,
    Shuffle,
    Repeat,
    Append,
    PlayNext,
    Remove,
    MoveDown,
    MoveUp,
    Clear,
    Help,
    DebugLine,
    Quit,
}

impl Action {
    pub fn describe(self) -> &'static str {
        match self {
            Action::Down => "move down",
            Action::Up => "move up",
            Action::Top => "top",
            Action::Bottom => "bottom",
            Action::HalfDown => "half page down",
            Action::HalfUp => "half page up",
            Action::Open => "open",
            Action::Back => "back",
            Action::ToggleFocus => "toggle sidebar / list",
            Action::Jump(_) => "jump to section",
            Action::Search => "search",
            Action::PlayPause => "play / pause",
            Action::Next => "next track",
            Action::Previous => "previous / restart",
            Action::SeekForward => "seek +10s",
            Action::SeekBack => "seek -10s",
            Action::VolumeUp => "volume up",
            Action::VolumeDown => "volume down",
            Action::Shuffle => "shuffle",
            Action::Repeat => "repeat off/all/one",
            Action::Append => "add to queue",
            Action::PlayNext => "play next",
            Action::Remove => "queue: remove",
            Action::MoveDown => "queue: move down",
            Action::MoveUp => "queue: move up",
            Action::Clear => "queue: clear",
            Action::Help => "help",
            Action::DebugLine => "memory usage",
            Action::Quit => "quit",
        }
    }
}

pub struct Binding {
    pub code: KeyCode,
    pub mods: KeyModifiers,
    pub action: Action,
}

const fn key(c: char, action: Action) -> Binding {
    Binding {
        code: KeyCode::Char(c),
        mods: KeyModifiers::NONE,
        action,
    }
}

const fn code(code: KeyCode, action: Action) -> Binding {
    Binding {
        code,
        mods: KeyModifiers::NONE,
        action,
    }
}

const fn ctrl(c: char, action: Action) -> Binding {
    Binding {
        code: KeyCode::Char(c),
        mods: KeyModifiers::CONTROL,
        action,
    }
}

pub const BINDINGS: &[Binding] = &[
    key('j', Action::Down),
    code(KeyCode::Down, Action::Down),
    key('k', Action::Up),
    code(KeyCode::Up, Action::Up),
    key('g', Action::Top),
    code(KeyCode::Home, Action::Top),
    key('G', Action::Bottom),
    code(KeyCode::End, Action::Bottom),
    ctrl('d', Action::HalfDown),
    code(KeyCode::PageDown, Action::HalfDown),
    ctrl('u', Action::HalfUp),
    code(KeyCode::PageUp, Action::HalfUp),
    key('l', Action::Open),
    code(KeyCode::Right, Action::Open),
    code(KeyCode::Enter, Action::Open),
    key('h', Action::Back),
    code(KeyCode::Left, Action::Back),
    code(KeyCode::Backspace, Action::Back),
    code(KeyCode::Tab, Action::ToggleFocus),
    key('1', Action::Jump(0)),
    key('2', Action::Jump(1)),
    key('3', Action::Jump(2)),
    key('4', Action::Jump(3)),
    key('5', Action::Jump(4)),
    key('/', Action::Search),
    key(' ', Action::PlayPause),
    key('>', Action::Next),
    key('<', Action::Previous),
    key('.', Action::SeekForward),
    key(',', Action::SeekBack),
    key('+', Action::VolumeUp),
    key('-', Action::VolumeDown),
    key('s', Action::Shuffle),
    key('r', Action::Repeat),
    key('a', Action::Append),
    key('n', Action::PlayNext),
    key('d', Action::Remove),
    key('J', Action::MoveDown),
    key('K', Action::MoveUp),
    key('D', Action::Clear),
    key('?', Action::Help),
    code(KeyCode::F(12), Action::DebugLine),
    key('q', Action::Quit),
];

pub fn lookup(k: KeyEvent) -> Option<Action> {
    let mut mods = k.modifiers;
    // terminals report shifted characters like `G` or `?` with SHIFT set
    if matches!(k.code, KeyCode::Char(_)) {
        mods.remove(KeyModifiers::SHIFT);
    }
    BINDINGS
        .iter()
        .find(|b| b.code == k.code && b.mods == mods)
        .map(|b| b.action)
}

pub fn key_name(b: &Binding) -> String {
    let name = match b.code {
        KeyCode::Char(' ') => "Space".into(),
        KeyCode::Char(c) => c.to_string(),
        KeyCode::Down => "↓".into(),
        KeyCode::Up => "↑".into(),
        KeyCode::Left => "←".into(),
        KeyCode::Right => "→".into(),
        KeyCode::Home => "Home".into(),
        KeyCode::End => "End".into(),
        KeyCode::PageDown => "PgDn".into(),
        KeyCode::PageUp => "PgUp".into(),
        KeyCode::Enter => "Enter".into(),
        KeyCode::Backspace => "Backspace".into(),
        KeyCode::Tab => "Tab".into(),
        KeyCode::Esc => "Esc".into(),
        KeyCode::F(n) => format!("F{n}"),
        other => format!("{other:?}"),
    };
    if b.mods.contains(KeyModifiers::CONTROL) {
        format!("Ctrl-{name}")
    } else {
        name
    }
}
