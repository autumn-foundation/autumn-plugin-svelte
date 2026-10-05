//! Tag helpers for the page `<head>`.

use autumn_web::assets::PluginAssets;
use maud::Markup;

use crate::assets::{LOADER_JS, SVELTE_ASSETS};

/// Renders the `<script defer>` tag of the island loader.
///
/// The tag has the fingerprinted URL, `integrity` and
/// `crossorigin="anonymous"`. Put it in the `<head>`.
///
/// ```rust
/// let html = autumn_plugin_svelte::svelte_script().into_string();
/// assert!(html.contains("/static/_plugins/svelte/islands."), "{html}");
/// assert!(html.contains("defer"), "{html}");
/// ```
#[must_use]
pub fn svelte_script() -> Markup {
    SVELTE_ASSETS.deferred_script_tag(LOADER_JS)
}

/// Renders the tags of an app bundle: one `<link rel="stylesheet">` for each
/// `.css` file, then one `<script defer>` for each `.js` file.
///
/// Files are in logical-path order. Other files (fonts, `.mjs`, source maps)
/// get no tag. Each tag has the fingerprinted URL and `integrity`.
///
/// Build the JavaScript as a classic script (Vite `formats: ["iife"]`). A
/// `<script defer>` tag cannot load an ES module.
#[must_use]
pub fn svelte_bundle(bundle: &PluginAssets) -> Markup {
    let paths = |ext: &'static str| {
        bundle
            .iter()
            .map(autumn_web::assets::PluginAsset::logical_path)
            .filter(move |path| path.ends_with(ext))
    };
    maud::html! {
        @for path in paths(".css") { (bundle.stylesheet_tag(path)) }
        @for path in paths(".js") { (bundle.deferred_script_tag(path)) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets::{LOADER_JS, SVELTE_ASSETS};

    static APP: PluginAssets = PluginAssets::from_files(
        "svelte-tags-test",
        &[
            ("b.js", b"b"),
            ("a.js", b"a"),
            ("chunk.mjs", b"m"),
            ("style.css", b"s"),
            ("font.woff2", b"f"),
            ("a.js.map", b"{}"),
        ],
    );

    #[test]
    fn loader_tag_is_deferred_with_sri() {
        let html = svelte_script().into_string();
        let asset = SVELTE_ASSETS.get(LOADER_JS).unwrap();
        assert_eq!(
            html,
            format!(
                r#"<script src="{}" integrity="{}" crossorigin="anonymous" defer></script>"#,
                asset.url(),
                asset.integrity()
            )
        );
    }

    #[test]
    fn bundle_tags_put_css_first_then_js_in_path_order() {
        let html = svelte_bundle(&APP).into_string();
        let expected = [
            APP.stylesheet_tag("style.css").into_string(),
            APP.deferred_script_tag("a.js").into_string(),
            APP.deferred_script_tag("b.js").into_string(),
        ]
        .concat();
        assert_eq!(html, expected);
    }

    #[test]
    fn bundle_tags_skip_other_files() {
        let html = svelte_bundle(&APP).into_string();
        for skipped in ["chunk.", "font.", ".map"] {
            assert!(!html.contains(skipped), "{skipped} has no tag: {html}");
        }
    }

    #[test]
    fn empty_bundle_renders_nothing() {
        static EMPTY: PluginAssets = PluginAssets::from_files("svelte-tags-empty", &[]);
        assert_eq!(svelte_bundle(&EMPTY).into_string(), "");
    }
}
