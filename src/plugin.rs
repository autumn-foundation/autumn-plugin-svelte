//! [`SveltePlugin`]: installs the loader and the app bundles.
//!
//! Each bundle mounts under `/static/_plugins/<namespace>/` through
//! `AppBuilder::plugin_assets`. No configuration, no startup hooks.

use std::borrow::Cow;

use autumn_web::app::AppBuilder;
use autumn_web::assets::PluginAssets;
use autumn_web::plugin::Plugin;
use autumn_web::plugin_contract::PluginContract;

use crate::assets::SVELTE_ASSETS;

/// The crate name. [`SveltePlugin`] with no bundles uses it as its plugin
/// name, and every [`SveltePlugin`] declares it in its contract.
pub const PLUGIN_NAME: &str = env!("CARGO_PKG_NAME");

/// Installs Svelte islands in an Autumn app.
///
/// ```rust,no_run
/// use autumn_plugin_svelte::SveltePlugin;
/// use autumn_web::assets::PluginAssets;
///
/// static ISLANDS: PluginAssets =
///     PluginAssets::from_files("app-islands", &[("islands.js", b"/* build */")]);
///
/// # async fn run() {
/// autumn_web::app()
///     .plugin(SveltePlugin::new().bundle(&ISLANDS))
///     .run()
///     .await;
/// # }
/// ```
///
/// The plugin name includes the bundle namespaces, for example
/// `autumn-plugin-svelte[app-islands]`. Thus a library crate and the app can
/// each install a `SveltePlugin` with their own bundles. Two plugins with the
/// same bundles are one plugin: Autumn skips the second.
#[derive(Debug, Default)]
#[must_use]
pub struct SveltePlugin {
    bundles: Vec<&'static PluginAssets>,
}

impl SveltePlugin {
    /// Makes the plugin with no app bundles.
    pub const fn new() -> Self {
        Self {
            bundles: Vec::new(),
        }
    }

    /// Adds an app bundle of compiled Svelte components.
    ///
    /// The same bundle two times is harmless.
    ///
    /// # Panics
    ///
    /// Panics when the bundle namespace is `svelte`: the loader bundle uses
    /// it. Autumn also stops at start-up when two different bundles use one
    /// namespace.
    pub fn bundle(mut self, bundle: &'static PluginAssets) -> Self {
        assert!(
            bundle.namespace() != SVELTE_ASSETS.namespace(),
            "the namespace `svelte` belongs to {PLUGIN_NAME}; give the app bundle another namespace"
        );
        if !self.bundles.iter().any(|b| std::ptr::eq(*b, bundle)) {
            self.bundles.push(bundle);
        }
        self
    }
}

impl Plugin for SveltePlugin {
    fn name(&self) -> Cow<'static, str> {
        if self.bundles.is_empty() {
            return Cow::Borrowed(PLUGIN_NAME);
        }
        let mut namespaces: Vec<&str> = self.bundles.iter().map(|b| b.namespace()).collect();
        namespaces.sort_unstable();
        namespaces.dedup();
        Cow::Owned(format!("{PLUGIN_NAME}[{}]", namespaces.join(",")))
    }

    fn contract(&self) -> Option<PluginContract> {
        Some(
            PluginContract::new(PLUGIN_NAME)
                .plugin_version(env!("CARGO_PKG_VERSION"))
                .autumn_web("0.8"),
        )
    }

    fn build(self, app: AppBuilder) -> AppBuilder {
        self.bundles
            .into_iter()
            .fold(app.plugin_assets(&SVELTE_ASSETS), AppBuilder::plugin_assets)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets::LOADER_JS;
    use autumn_web::assets::{PLUGIN_ASSETS_ROUTE_MARKER, asset_url};
    use autumn_web::plugin_conformance::{ConformanceConfig, run_conformance};
    use autumn_web::route_listing::{RouteClassification, RouteSource};
    use autumn_web::test::{TestApp, TestClient};

    const JS: &str = "text/javascript; charset=utf-8";
    const IMMUTABLE: &str = "public, max-age=31536000, immutable";
    const REVALIDATE: &str = "public, max-age=0, must-revalidate";

    static APP: PluginAssets = PluginAssets::from_files(
        "svelte-plugin-test",
        &[("islands.js", b"window.app = 1;"), ("islands.css", b".a{}")],
    );

    fn client() -> TestClient {
        TestApp::new()
            .plugin(SveltePlugin::new().bundle(&APP))
            .build()
    }

    #[tokio::test]
    async fn loader_serves_at_fingerprinted_url() {
        let response = client().get(&SVELTE_ASSETS.url(LOADER_JS)).send().await;
        response
            .assert_ok()
            .assert_header("content-type", JS)
            .assert_header("cache-control", IMMUTABLE);
        assert!(response.text().contains("autumnSvelte"));
    }

    #[tokio::test]
    async fn loader_plain_url_revalidates_with_etag() {
        let client = client();
        let plain = "/static/_plugins/svelte/islands.js";
        let response = client.get(plain).send().await;
        response
            .assert_ok()
            .assert_header("cache-control", REVALIDATE);
        let etag = response.header("etag").expect("etag").to_owned();
        client
            .get(plain)
            .header("if-none-match", &etag)
            .send()
            .await
            .assert_status(304);
    }

    #[tokio::test]
    async fn app_bundle_serves_at_fingerprinted_urls() {
        let client = client();
        for asset in APP.iter() {
            let response = client.get(asset.url()).send().await;
            response
                .assert_ok()
                .assert_header("cache-control", IMMUTABLE)
                .assert_header("content-type", asset.content_type());
            assert_eq!(response.body.as_slice(), asset.bytes());
        }
    }

    #[tokio::test]
    async fn unknown_and_stale_paths_are_not_found() {
        let client = client();
        for path in [
            "/static/_plugins/svelte/islands.00000000.js",
            "/static/_plugins/svelte/nope.js",
            "/static/_plugins/svelte-plugin-test/nope.js",
        ] {
            client.get(path).send().await.assert_status(404);
        }
    }

    #[tokio::test]
    async fn asset_url_resolves_both_bundles() {
        let _client = client();
        assert_eq!(
            asset_url("_plugins/svelte/islands.js"),
            SVELTE_ASSETS.url(LOADER_JS)
        );
        assert_eq!(
            asset_url("_plugins/svelte-plugin-test/islands.js"),
            APP.url("islands.js")
        );
    }

    #[test]
    fn routes_are_public_plugin_routes() {
        let plugin = SveltePlugin::new().bundle(&APP);
        let name = plugin.name().into_owned();
        let app = autumn_web::app().plugin(plugin);
        let infos = app.plugin_route_infos().expect("route infos");
        let asset_routes: Vec<_> = infos
            .iter()
            .filter(|info| info.path.starts_with("/static/_plugins/svelte"))
            .collect();
        // Loader: 1 file. App: 2 files. Two URLs per file.
        assert_eq!(asset_routes.len(), 6, "{infos:?}");
        for info in asset_routes {
            assert_eq!(info.method, "GET");
            assert_eq!(info.classification, RouteClassification::Public);
            assert_eq!(info.middleware, [PLUGIN_ASSETS_ROUTE_MARKER]);
            assert_eq!(info.source, RouteSource::Plugin(name.clone()));
        }
    }

    #[test]
    fn plugin_passes_conformance() {
        let app = autumn_web::app().plugin(SveltePlugin::new());
        let infos = app.plugin_route_infos().expect("route infos");
        let report = run_conformance(&ConformanceConfig::new(PLUGIN_NAME), &infos);
        assert!(report.passed(), "{}", report.to_text_report());
    }

    #[test]
    fn plugin_declares_autumn_web_0_8_contract() {
        let contract = SveltePlugin::new().contract().expect("contract");
        let app = autumn_web::app().plugin(SveltePlugin::new());
        let contracts = app.plugin_contracts();
        assert_eq!(contracts.len(), 1, "{contracts:?}");
        assert_eq!(contract.plugin, PLUGIN_NAME);
        assert_eq!(
            contract.plugin_version.as_deref(),
            Some(env!("CARGO_PKG_VERSION"))
        );
        assert_eq!(contract.autumn_web.as_deref(), Some("0.8"));
        assert!(contract.experimental_surfaces.is_empty());
    }

    #[tokio::test]
    async fn installing_twice_is_harmless() {
        let client = TestApp::new()
            .plugin(SveltePlugin::new().bundle(&APP))
            .plugin(SveltePlugin::new().bundle(&APP))
            .build();
        client
            .get(&SVELTE_ASSETS.url(LOADER_JS))
            .send()
            .await
            .assert_ok();
        client.get(&APP.url("islands.js")).send().await.assert_ok();
    }

    static OTHER: PluginAssets = PluginAssets::from_files(
        "svelte-plugin-other",
        &[("islands.js", b"window.other = 1;")],
    );

    #[test]
    fn name_lists_the_bundle_namespaces() {
        assert_eq!(SveltePlugin::new().name(), PLUGIN_NAME);
        assert_eq!(PLUGIN_NAME, env!("CARGO_PKG_NAME"));
        assert_eq!(
            SveltePlugin::new()
                .bundle(&OTHER)
                .bundle(&APP)
                .bundle(&APP)
                .name(),
            "autumn-plugin-svelte[svelte-plugin-other,svelte-plugin-test]"
        );
    }

    #[tokio::test]
    async fn two_plugins_with_different_bundles_both_serve() {
        let client = TestApp::new()
            .plugin(SveltePlugin::new().bundle(&APP))
            .plugin(SveltePlugin::new().bundle(&OTHER))
            .build();
        client.get(&APP.url("islands.js")).send().await.assert_ok();
        client
            .get(&OTHER.url("islands.js"))
            .send()
            .await
            .assert_ok();
        client
            .get(&SVELTE_ASSETS.url(LOADER_JS))
            .send()
            .await
            .assert_ok();
    }

    #[test]
    fn two_plugins_pass_conformance_under_their_names() {
        let first = SveltePlugin::new().bundle(&APP);
        let second = SveltePlugin::new().bundle(&OTHER);
        let names = [first.name(), second.name()];
        let app = autumn_web::app().plugin(first).plugin(second);
        let infos = app.plugin_route_infos().expect("route infos");
        for name in names {
            let report = run_conformance(&ConformanceConfig::new(name.as_ref()), &infos);
            assert!(report.passed(), "{}", report.to_text_report());
        }
        assert!(
            app.plugin_contracts()
                .iter()
                .all(|c| c.plugin == PLUGIN_NAME)
        );
    }

    #[test]
    #[should_panic(expected = "the namespace `svelte` belongs to autumn-plugin-svelte")]
    fn bundle_in_the_loader_namespace_is_refused() {
        static CLASH: PluginAssets = PluginAssets::from_files("svelte", &[("x.js", b"")]);
        let _ = SveltePlugin::new().bundle(&CLASH);
    }

    #[tokio::test]
    async fn plugin_with_no_bundles_serves_the_loader() {
        let client = TestApp::new().plugin(SveltePlugin::new()).build();
        client
            .get(&SVELTE_ASSETS.url(LOADER_JS))
            .send()
            .await
            .assert_ok();
    }

    #[tokio::test]
    async fn same_bundle_given_twice_is_harmless() {
        let client = TestApp::new()
            .plugin(SveltePlugin::new().bundle(&APP).bundle(&APP))
            .build();
        client.get(&APP.url("islands.js")).send().await.assert_ok();
    }
}
