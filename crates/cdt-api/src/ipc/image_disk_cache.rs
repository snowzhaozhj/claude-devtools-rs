//! Image disk cache：把 base64 image data 内容寻址落盘到本地 `cache_dir`，返回
//! `asset://localhost/<absolute_path>` 让 Tauri webview 通过 asset protocol 直接读，
//! 避免 IPC payload inline data URI 膨胀。
//!
//! 详 change `unify-fs-direct-calls` design D4：disk cache 路径永远走本地 fs
//! （`~/.cache/`），与 SSH source 是否远端无关——SSH 端的 image asset 拉到 Local
//! 后 cache 在本地复用是合理的（避免每次显示都 SFTP 拉一次）。本 module 路径走
//! ALLOWLIST 而非 fs trait，因为 cache 写入永远是 Local 业务。

use std::path::Path;

/// 失败 fallback：返回一个空 `data:` URI 占位。前端 `<img>` 加载会显示
/// broken-image，不阻塞 session 渲染。
pub(super) fn empty_data_uri() -> String {
    "data:application/octet-stream;base64,".to_owned()
}

/// 完整 `data:` URI（落盘失败 / cache 目录未注入时 fallback）。
pub(super) fn format_data_uri(media_type: &str, base64_data: &str) -> String {
    format!("data:{media_type};base64,{base64_data}")
}

pub(super) fn media_type_to_ext(mime: &str) -> &'static str {
    match mime {
        "image/png" => "png",
        "image/jpeg" | "image/jpg" => "jpg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        "image/svg+xml" => "svg",
        _ => "bin",
    }
}

/// SHA256 内容寻址 + 落盘到 cache 目录，返回 `asset://localhost/<absolute_path>`。
/// 失败时 fallback 返回 `data:` URI。
///
/// `tokio::fs::*` 直调走 ALLOWLIST 豁免（design D4）：`cache_dir` 永远 Local，与
/// active `FsKind` 无关。
pub(super) async fn materialize_image_asset(
    cache_dir: &Path,
    media_type: &str,
    base64_data: &str,
) -> String {
    use base64::Engine;
    use sha2::Digest;

    let bytes = match base64::engine::general_purpose::STANDARD.decode(base64_data) {
        Ok(b) => b,
        Err(e) => {
            tracing::warn!(target: "cdt_api::image", error = %e, "base64 decode failed");
            return format_data_uri(media_type, base64_data);
        }
    };

    let mut hasher = sha2::Sha256::new();
    hasher.update(&bytes);
    let digest = hasher.finalize();
    let hash_hex: String = digest.iter().take(8).fold(String::new(), |mut acc, b| {
        use std::fmt::Write;
        let _ = write!(acc, "{b:02x}");
        acc
    });

    let ext = media_type_to_ext(media_type);
    let file_path = cache_dir.join(format!("{hash_hex}.{ext}"));

    if let Err(e) = tokio::fs::create_dir_all(cache_dir).await {
        tracing::warn!(target: "cdt_api::image", error = %e, dir = %cache_dir.display(), "create cache dir failed");
        return format_data_uri(media_type, base64_data);
    }

    if tokio::fs::metadata(&file_path).await.is_err() {
        if let Err(e) = tokio::fs::write(&file_path, &bytes).await {
            tracing::warn!(target: "cdt_api::image", error = %e, path = %file_path.display(), "write image cache failed");
            return format_data_uri(media_type, base64_data);
        }
    }

    // Windows 上 `file_path.display()` 含 `\`，Tauri asset protocol 按 POSIX URI
    // 解析 —— 手动归一为 `/` 保证 `asset://localhost/C:/Users/...` 格式。
    let url_path = file_path.to_string_lossy().replace('\\', "/");
    format!("asset://localhost/{url_path}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn media_type_to_ext_known_and_unknown() {
        assert_eq!(media_type_to_ext("image/png"), "png");
        assert_eq!(media_type_to_ext("image/jpeg"), "jpg");
        assert_eq!(media_type_to_ext("image/gif"), "gif");
        assert_eq!(media_type_to_ext("image/webp"), "webp");
        assert_eq!(media_type_to_ext("application/x-future"), "bin");
    }
}
