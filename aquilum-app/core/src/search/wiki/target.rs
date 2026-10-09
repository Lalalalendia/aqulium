use crate::search::paths::{relative_slash_path, strip_markdown_extension};
use std::path::{Path, PathBuf};

pub fn target_key(target: &str) -> (String, &'static str) {
    let clean = target.trim().replace('\\', "/");
    let clean = clean.split('#').next().unwrap_or_default();
    let clean = strip_markdown_extension(clean);
    (
        clean.to_lowercase(),
        if clean.contains('/') { "path" } else { "name" },
    )
}

pub fn relative_key(root: &Path, path: &Path) -> String {
    strip_markdown_extension(&relative_slash_path(root, path)).to_lowercase()
}

pub fn title_key(path: &Path) -> String {
    path.file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase()
}

pub fn relative_target(root: &Path, path: &Path, path_style: bool) -> String {
    if !path_style {
        return path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
    }
    strip_markdown_extension(&relative_slash_path(root, path)).to_owned()
}

pub fn select_candidate(root: &Path, source: &Path, paths: &[String]) -> Option<PathBuf> {
    let borrowed = paths.iter().map(String::as_str).collect::<Vec<_>>();
    pick_candidate(root, source, &borrowed).map(PathBuf::from)
}

pub fn pick_candidate<'a>(
    root: &Path,
    source: &Path,
    paths: &[&'a str],
) -> Option<&'a str> {
    let parent = source.parent();
    paths.iter().copied().min_by(|left, right| {
        candidate_rank(root, parent, left)
            .cmp(&candidate_rank(root, parent, right))
            .then_with(|| left.cmp(right))
    })
}

fn candidate_rank(root: &Path, parent: Option<&Path>, value: &str) -> (bool, usize) {
    let path = Path::new(value);
    (
        path.parent() != parent,
        path.strip_prefix(root).unwrap_or(path).components().count(),
    )
}
