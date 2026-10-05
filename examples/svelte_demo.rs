//! Svelte demo: a small Autumn app with Svelte 5 islands.
//!
//! ```sh
//! cargo run --example svelte_demo
//! ```
//!
//! Open <http://127.0.0.1:3000>. The page has:
//!
//! - a `Counter` island with server props,
//! - a `Clock` island that htmx adds and removes (the loader unmounts it, so
//!   its timer stops),
//! - a `Counter` island that mounts only when you scroll to it.
//!
//! The components are in `frontend/src/`. Their build output is in
//! `examples/islands/`, so this example needs no Node.

use autumn_plugin_svelte::{Island, MountWhen, SveltePlugin, svelte_bundle, svelte_script};
use autumn_web::assets::PluginAssets;
use autumn_web::prelude::*;

/// The Vite build output of `frontend/`, compiled into the binary.
static ISLANDS: PluginAssets = PluginAssets::from_files(
    "demo-islands",
    &[
        ("islands.js", include_bytes!("islands/islands.js")),
        ("islands.css", include_bytes!("islands/islands.css")),
    ],
);

#[autumn_web::main]
async fn main() {
    autumn_web::app()
        .plugin(SveltePlugin::new().bundle(&ISLANDS))
        .routes(routes![index, clock_on, clock_off])
        .run()
        .await;
}

fn layout(content: &Markup) -> Markup {
    html! {
        (maud::DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { "Autumn + Svelte islands" }
                script src="/static/js/htmx.min.js" defer {}
                (svelte_script())
                (svelte_bundle(&ISLANDS))
            }
            body { main { (content) } }
        }
    }
}

#[get("/")]
async fn index() -> AutumnResult<Markup> {
    let counter = Island::new("Counter")
        .props(&serde_json::json!({ "start": 3, "label": "Clicks" }))?
        .fallback(html! { p { "Clicks: 3 (enable JavaScript to change it)" } });
    let lazy = Island::new("Counter")
        .props(&serde_json::json!({ "start": 100, "label": "Lazy" }))?
        .mount_when(MountWhen::Visible)
        .fallback(html! { p { "Lazy: 100" } });
    Ok(layout(&html! {
        h1 { "Svelte islands in an Autumn page" }
        section {
            h2 { "Counter (props from Rust)" }
            (counter)
        }
        section {
            h2 { "Clock (htmx adds and removes it)" }
            div #clock-slot { (clock_controls(false)) }
        }
        div style="height: 120vh" { p { "Scroll down." } }
        section {
            h2 { "Counter (mounts when visible)" }
            (lazy)
        }
    }))
}

fn clock_controls(on: bool) -> Markup {
    html! {
        @if on {
            button hx-get="/clock/off" hx-target="#clock-slot" { "Remove clock" }
            (Island::new("Clock").fallback(html! { "Clock" }))
        } @else {
            button hx-get="/clock/on" hx-target="#clock-slot" { "Add clock" }
        }
    }
}

#[get("/clock/on")]
async fn clock_on() -> Markup {
    clock_controls(true)
}

#[get("/clock/off")]
async fn clock_off() -> Markup {
    clock_controls(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use autumn_web::test::{TestApp, TestClient};

    fn client() -> TestClient {
        TestApp::new()
            .plugin(SveltePlugin::new().bundle(&ISLANDS))
            .routes(routes![index, clock_on, clock_off])
            .build()
    }

    #[tokio::test]
    async fn index_renders_islands_and_tags() {
        let client = client();
        let html = client.get("/").send().await.assert_ok().text();
        assert_eq!(html.matches(r#"data-svelte-island="Counter""#).count(), 2);
        assert!(html.contains(r#"data-svelte-mount="visible""#), "{html}");
        assert!(html.contains(&ISLANDS.url("islands.js")), "{html}");
        assert!(html.contains(&ISLANDS.url("islands.css")), "{html}");
        assert!(html.contains("/static/_plugins/svelte/islands."), "{html}");
        for asset in ISLANDS.iter() {
            client.get(asset.url()).send().await.assert_ok();
        }
    }

    #[tokio::test]
    async fn clock_fragments_toggle() {
        let client = client();
        let on = client.get("/clock/on").send().await.assert_ok().text();
        assert!(on.contains(r#"data-svelte-island="Clock""#), "{on}");
        let off = client.get("/clock/off").send().await.assert_ok().text();
        assert!(!off.contains("data-svelte-island"), "{off}");
    }
}
