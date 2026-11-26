#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clip_stack_intersects_and_restores_parent_clip() {
        let mut dl = DrawList::new(1.0);
        dl.push_clip(Rect::new(10.0, 10.0, 200.0, 200.0));
        dl.push_clip(Rect::new(150.0, 50.0, 100.0, 100.0));
        assert_eq!(
            dl.clip_stack.last().copied(),
            Some(Rect::new(150.0, 50.0, 60.0, 100.0))
        );
        dl.pop_clip();
        assert_eq!(
            dl.clip_stack.last().copied(),
            Some(Rect::new(10.0, 10.0, 200.0, 200.0))
        );
        dl.pop_clip();
        assert_eq!(dl.clip_stack.last().copied(), None);
    }
}
