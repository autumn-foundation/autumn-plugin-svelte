//! The loader bundle, embedded at compile time.

use autumn_web::assets::PluginAssets;

/// URL namespace of [`SVELTE_ASSETS`]: `/static/_plugins/svelte/`.
///
/// An app bundle must use a different namespace.
pub const ASSETS_NAMESPACE: &str = "svelte";

/// Logical path of the island loader in [`SVELTE_ASSETS`].
pub const LOADER_JS: &str = "islands.js";

/// The plugin bundle. It holds only the island loader.
///
/// [`SveltePlugin`](crate::SveltePlugin) installs it. Use it directly only
/// to make URLs or tags yourself:
///
/// ```rust
/// use autumn_plugin_svelte::SVELTE_ASSETS;
///
/// let url = SVELTE_ASSETS.url("islands.js");
/// assert!(url.starts_with("/static/_plugins/svelte/islands."), "{url}");
/// ```
pub static SVELTE_ASSETS: PluginAssets = PluginAssets::from_files(
    ASSETS_NAMESPACE,
    &[(LOADER_JS, include_bytes!("../assets/islands.js"))],
);

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine as _;
    use sha2::{Digest as _, Sha384};

    fn sri(bytes: &[u8]) -> String {
        let digest = Sha384::digest(bytes);
        format!(
            "sha384-{}",
            base64::engine::general_purpose::STANDARD.encode(digest)
        )
    }

    fn loader() -> String {
        String::from_utf8(
            SVELTE_ASSETS
                .get(LOADER_JS)
                .expect("loader")
                .bytes()
                .to_vec(),
        )
        .expect("utf-8")
    }

    #[test]
    fn bundle_holds_only_the_loader() {
        let files: Vec<&str> = SVELTE_ASSETS
            .iter()
            .map(autumn_web::assets::PluginAsset::logical_path)
            .collect();
        assert_eq!(files, [LOADER_JS]);
        assert_eq!(SVELTE_ASSETS.mount_path(), "/static/_plugins/svelte");
    }

    #[test]
    fn loader_url_is_fingerprinted_and_sri_matches_bytes() {
        let asset = SVELTE_ASSETS.get(LOADER_JS).expect("loader");
        let hash = asset
            .url()
            .strip_prefix("/static/_plugins/svelte/islands.")
            .and_then(|rest| rest.strip_suffix(".js"))
            .expect("fingerprinted url");
        assert_eq!(hash.len(), 8);
        assert!(hash.bytes().all(|b| b.is_ascii_hexdigit()));
        assert_eq!(asset.integrity(), sri(asset.bytes()));
        assert_eq!(asset.content_type(), "text/javascript; charset=utf-8");
    }

    #[test]
    fn loader_keeps_the_public_contract() {
        let js = loader();
        for needle in [
            "autumnSvelte",
            "data-svelte-island",
            "data-svelte-props",
            "data-svelte-mount",
            "data-svelte-state",
            "data-svelte-fallback",
            "MutationObserver",
            "IntersectionObserver",
            "requestIdleCallback",
            "autumn:svelte:mount",
            "autumn:svelte:unmount",
            "autumn:svelte:error",
        ] {
            assert!(js.contains(needle), "loader uses `{needle}`");
        }
    }

    #[test]
    fn loader_has_no_unsafe_sinks() {
        let js = loader();
        for sink in [
            "eval(",
            "new Function",
            "innerHTML",
            "outerHTML",
            "document.write",
        ] {
            assert!(!js.contains(sink), "loader must not use `{sink}`");
        }
    }
}
