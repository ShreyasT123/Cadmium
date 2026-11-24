use slotmap::new_key_type;

new_key_type! {
    pub struct WidgetId;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Key {
    Character(String),
    Backspace,
    Delete,
    Enter,
    Tab,
    Escape,
    Space,
    ArrowLeft,
    ArrowRight,
    ArrowUp,
    ArrowDown,
    Home,
    End,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerButton {
    Left,
    Right,
    Middle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Modifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub super_key: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventPropagation {
    Continue,
    Stop,
}

#[derive(Debug, Clone)]
pub enum UiEvent {
    PointerDown {
        x: f32,
        y: f32,
        button: PointerButton,
    },
    PointerUp {
        x: f32,
        y: f32,
        button: PointerButton,
    },
    PointerMove {
        x: f32,
        y: f32,
    },
    Scroll {
        delta_x: f32,
        delta_y: f32,
    },
    FocusGained,
    FocusLost,
    KeyDown {
        key: Key,
        modifiers: Modifiers,
    },
}

#[derive(Default)]
pub struct FocusManager {
    pub focused: Option<WidgetId>,
    pub hovered: Option<WidgetId>,
    pub captured: Option<WidgetId>,
    pub focus_order: Vec<WidgetId>,
}

impl FocusManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_focus(&mut self, id: Option<WidgetId>) -> bool {
        if self.focused == id {
            return false;
        }
        self.focused = id;
        true
    }

    pub fn cycle_next(&mut self) -> Option<WidgetId> {
        if self.focus_order.is_empty() {
            return None;
        }
        let next = match self
            .focused
            .and_then(|f| self.focus_order.iter().position(|&id| id == f))
        {
            Some(idx) => self.focus_order[(idx + 1) % self.focus_order.len()],
            None => self.focus_order[0],
        };
        self.focused = Some(next);
        Some(next)
    }

    pub fn cycle_prev(&mut self) -> Option<WidgetId> {
        if self.focus_order.is_empty() {
            return None;
        }
        let len = self.focus_order.len();
        let prev = match self
            .focused
            .and_then(|f| self.focus_order.iter().position(|&id| id == f))
        {
            Some(idx) => self.focus_order[(idx + len - 1) % len],
            None => self.focus_order[len - 1],
        };
        self.focused = Some(prev);
        Some(prev)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use slotmap::SlotMap;

    #[test]
    fn focus_cycling_wraps_cleanly() {
        let mut sm: SlotMap<WidgetId, ()> = SlotMap::with_key();
        let a = sm.insert(());
        let b = sm.insert(());
        let c = sm.insert(());

        let mut fm = FocusManager::new();
        fm.focus_order = vec![a, b, c];
        assert_eq!(fm.cycle_next(), Some(a));
        assert_eq!(fm.cycle_next(), Some(b));
        assert_eq!(fm.cycle_next(), Some(c));
        assert_eq!(fm.cycle_next(), Some(a));
        assert_eq!(fm.cycle_prev(), Some(c));
    }
}
