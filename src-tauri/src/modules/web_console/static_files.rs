use std::path::{Component, Path, PathBuf};

use super::INDEX_HTML;

pub(super) fn find_frontend_dist() -> Option<PathBuf> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let candidates = [
        manifest_dir.join("../dist"),
        std::env::current_dir().ok()?.join("dist"),
        std::env::current_dir().ok()?.join("../dist"),
    ];

    candidates
        .into_iter()
        .map(|path| normalize_path(&path))
        .find(|path| path.join(INDEX_HTML).exists())
}

fn normalize_path(path: &Path) -> PathBuf {
    path.components().fold(PathBuf::new(), |mut acc, part| {
        match part {
            Component::CurDir => {}
            Component::ParentDir => {
                acc.pop();
            }
            other => acc.push(other.as_os_str()),
        }
        acc
    })
}

pub(super) fn resolve_static_path(root: &Path, request_path: &str) -> Result<PathBuf, String> {
    let path = if request_path == "/" || request_path.is_empty() {
        INDEX_HTML.to_string()
    } else {
        request_path.trim_start_matches('/').to_string()
    };
    let decoded =
        urlencoding::decode(&path).map_err(|err| format!("invalid URL encoding: {}", err))?;
    let mut result = PathBuf::from(root);
    for segment in decoded.split('/') {
        if segment.is_empty() {
            continue;
        }
        if segment == "." || segment == ".." || segment.contains('\\') {
            return Err("invalid static path".to_string());
        }
        result.push(segment);
    }
    Ok(result)
}

pub(super) fn content_type_for_path(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" => "application/json; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        _ => "application/octet-stream",
    }
}
