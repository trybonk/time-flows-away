/// Other Shell children can sit between these windows. Correct relative order
/// matters; requiring the icon view to be the immediate neighbor caused churn.
pub fn between(siblings: &[usize], wallpaper: usize, icons: usize, worker: usize) -> bool {
    let index = |handle| siblings.iter().position(|candidate| *candidate == handle);
    match (index(icons), index(wallpaper), index(worker)) {
        (Some(top), Some(middle), Some(bottom)) => top < middle && middle < bottom,
        _ => false,
    }
}
#[cfg(test)]
mod tests {
    use super::between;
    #[test]
    fn unrelated_children_do_not_require_reordering() {
        assert!(between(&[1, 8, 2, 9, 3], 2, 1, 3));
        assert!(between(&[1, 2, 3], 2, 1, 3));
    }
    #[test]
    fn wallpaper_above_icons_requires_repair() {
        assert!(!between(&[2, 1, 3], 2, 1, 3));
    }
    #[test]
    fn shell_background_above_wallpaper_requires_repair() {
        assert!(!between(&[1, 3, 2], 2, 1, 3));
    }
    #[test]
    fn missing_shell_layers_are_not_accepted() {
        assert!(!between(&[1, 2], 2, 1, 3));
        assert!(!between(&[1, 3], 2, 1, 3));
    }
}
