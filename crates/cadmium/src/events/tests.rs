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
