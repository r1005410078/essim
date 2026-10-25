//! 工程入口预约与窗口归属；不持有协议、磁盘或原生窗口对象。
use std::{collections::HashMap, path::PathBuf};
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Owner {
    Opening(String),
    Open(String),
    Closing(String),
}
#[derive(Default)]
pub(crate) struct WindowIndex {
    paths: HashMap<PathBuf, Owner>,
}
impl WindowIndex {
    pub(crate) fn reserve(&mut self, path: PathBuf, label: &str) -> Result<(), Owner> {
        if let Some(owner) = self.paths.get(&path) {
            return Err(owner.clone());
        }
        self.paths.insert(path, Owner::Opening(label.into()));
        Ok(())
    }
    pub(crate) fn owner(&self, path: &std::path::Path) -> Option<Owner> {
        self.paths.get(path).cloned()
    }
    pub(crate) fn publish(&mut self, path: PathBuf, label: &str) {
        self.paths.insert(path, Owner::Open(label.into()));
    }
    pub(crate) fn closing(&mut self, label: &str) {
        for owner in self.paths.values_mut() {
            if matches!(owner, Owner::Open(v) if v == label) {
                *owner = Owner::Closing(label.into());
            }
        }
    }
    pub(crate) fn release_path(&mut self, path: &std::path::Path, label: &str) {
        if matches!(self.paths.get(path), Some(Owner::Opening(v)) if v == label) {
            self.paths.remove(path);
        }
    }
    pub(crate) fn release(&mut self, label: &str) {
        self.paths.retain(|_, v| match v {
            Owner::Opening(v) | Owner::Open(v) | Owner::Closing(v) => v != label,
        });
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn same_path_is_reserved_once_and_failed_open_can_retry() {
        let mut index = WindowIndex::default();
        let path = PathBuf::from("/a/project.workspace");
        assert!(index.reserve(path.clone(), "a").is_ok());
        assert_eq!(
            index.reserve(path.clone(), "b"),
            Err(Owner::Opening("a".into()))
        );
        index.release("a");
        assert!(index.reserve(path.clone(), "b").is_ok());
        index.publish(path.clone(), "b");
        assert_eq!(index.owner(&path), Some(Owner::Open("b".into())));
        index.closing("b");
        assert_eq!(index.reserve(path, "c"), Err(Owner::Closing("b".into())));
    }
    #[test]
    fn same_ids_in_different_directories_are_independent() {
        let mut index = WindowIndex::default();
        assert!(index.reserve("/a/project.workspace".into(), "a").is_ok());
        assert!(index.reserve("/b/project.workspace".into(), "b").is_ok());
        index.release("a");
        assert!(index
            .owner(std::path::Path::new("/b/project.workspace"))
            .is_some());
    }
    #[cfg(unix)]
    #[test]
    fn symbolic_link_and_real_entry_share_reservation() {
        let root = std::env::temp_dir().join(format!("essim-window-alias-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let entry = root.join("project.workspace");
        std::fs::write(&entry, b"test").unwrap();
        let alias = root.join("alias.workspace");
        let _ = std::fs::remove_file(&alias);
        std::os::unix::fs::symlink(&entry, &alias).unwrap();
        let mut index = WindowIndex::default();
        index.reserve(entry.canonicalize().unwrap(), "a").unwrap();
        assert_eq!(
            index.reserve(alias.canonicalize().unwrap(), "b"),
            Err(Owner::Opening("a".into()))
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}
