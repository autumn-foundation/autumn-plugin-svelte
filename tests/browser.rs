//! Browser tests: the island loader in real headless Chromium.
//!
//! Run: `cargo test --test browser -- --ignored --test-threads=1`.
//! The tests need Chromium (see `autumn_web::system_test`).

// Test helpers stop the test on a failure.
#![allow(clippy::expect_used, clippy::panic)]

use std::time::Duration;

use autumn_plugin_svelte::{Island, MountWhen, SveltePlugin, svelte_bundle, svelte_script};
use autumn_web::assets::PluginAssets;
use autumn_web::prelude::*;
use autumn_web::reexports::axum;
use autumn_web::reexports::axum::extract::Path;
use autumn_web::system_test::{Page, SystemTest, SystemTestRunner};
use autumn_web::test::TestApp;
use serde::de::DeserializeOwned;

/// The contract-level fake bundle.
static FAKE: PluginAssets = PluginAssets::from_files(
    "svelte-fake",
    &[("fake.js", include_bytes!("fixtures/fake.js"))],
);

/// The real Svelte 5 bundle from `frontend/`.
static REAL: PluginAssets = PluginAssets::from_files(
    "svelte-real",
    &[
        (
            "islands.js",
            include_bytes!("../examples/islands/islands.js"),
        ),
        (
            "islands.css",
            include_bytes!("../examples/islands/islands.css"),
        ),
    ],
);

fn head() -> Markup {
    html! {
        script src="/static/js/htmx.min.js" defer {}
        (svelte_script())
        (svelte_bundle(&FAKE))
        (svelte_bundle(&REAL))
    }
}

fn page(body: &Markup) -> Markup {
    html! {
        (maud::DOCTYPE)
        html { head { (head()) } body { (body) } }
    }
}

fn echo(id: &str, key: &str) -> Island {
    Island::new("Echo")
        .id(id)
        .props(&serde_json::json!({ "k": key }))
        .expect("props")
        .fallback(html! { i { "fallback " (key) } })
}

#[get("/real")]
async fn real() -> AutumnResult<Markup> {
    let counter = Island::new("Counter")
        .id("counter")
        .props(&serde_json::json!({ "start": 3, "label": "Hits" }))?
        .fallback(html! { p { "Hits: 3 (server)" } });
    Ok(page(&html! { (counter) }))
}

#[get("/order")]
async fn order() -> Markup {
    // App bundle first, loader second, loader again.
    html! {
        (maud::DOCTYPE)
        html {
            head { (svelte_bundle(&FAKE)) (svelte_script()) (svelte_script()) }
            body { (echo("e", "order")) }
        }
    }
}

#[get("/swap")]
async fn swap() -> Markup {
    page(&html! {
        button #next hx-get="/fragment/2" hx-target="#slot" { "Next" }
        div #slot { (echo("i1", "1")) }
        div #other {}
    })
}

#[get("/fragment/{n}")]
async fn fragment(Path(n): Path<u32>) -> Markup {
    html! { (echo(&format!("i{n}"), &n.to_string())) }
}

#[get("/lazy")]
async fn lazy() -> Markup {
    page(&html! {
        (echo("idle", "idle").mount_when(MountWhen::Idle))
        div style="height: 5000px" {}
        (echo("visible", "visible").mount_when(MountWhen::Visible))
        (echo("gone", "gone").mount_when(MountWhen::Visible))
    })
}

#[get("/errors")]
async fn errors() -> Markup {
    page(&html! {
        (Island::new("Boom").id("boom").fallback(html! { "boom fallback" }))
        (Island::new("Partial").id("partial").fallback(html! { "partial fallback" }))
        div #badjson data-svelte-island="Echo" data-svelte-props="{bad" { "bad json fallback" }
        div #array data-svelte-island="Echo" data-svelte-props="[1]" { "array fallback" }
    })
}

#[get("/names")]
async fn names() -> Markup {
    page(&html! {
        (Island::new("Later").id("later").fallback(html! { "later fallback" }))
        (Island::new("__proto__").id("proto"))
        (Island::new("constructor").id("ctor"))
        (Island::new("hasOwnProperty").id("own"))
        (echo("ok", "ok"))
    })
}

#[get("/clock")]
async fn clock() -> Markup {
    page(&html! {
        button #clear hx-get="/empty" hx-target="#slot" { "Clear" }
        div #slot { (Island::new("Clock").id("clock")) }
    })
}

#[get("/empty")]
async fn empty() -> Markup {
    html! { p #cleared { "cleared" } }
}

fn app() -> TestApp {
    TestApp::new()
        .plugin(SveltePlugin::new().components(&FAKE).components(&REAL))
        .routes(routes![
            real, order, swap, fragment, lazy, errors, names, clock, empty
        ])
}

/// Serves the app on a free port and opens Chromium on it.
async fn start() -> SystemTestRunner {
    let router = app().build().into_router();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move {
        axum::serve(listener, router).await.expect("serve");
    });
    SystemTest::attach(format!("http://{addr}"))
        .await
        .expect("Chromium")
}

/// Runs `js` and drops the result.
async fn run(page: &Page, js: &str) {
    page.evaluate(js).await.expect("evaluate");
}

async fn eval<T: DeserializeOwned>(page: &Page, js: &str) -> T {
    page.evaluate(js)
        .await
        .expect("evaluate")
        .into_value()
        .expect("decode")
}

/// Polls `js` until it is `true`. Fails after 10 seconds.
async fn wait_for(page: &Page, js: &str) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        if page
            .evaluate(js)
            .await
            .ok()
            .and_then(|r| r.into_value::<bool>().ok())
            == Some(true)
        {
            return;
        }
        assert!(tokio::time::Instant::now() < deadline, "timed out: {js}");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

async fn state(page: &Page, id: &str) -> String {
    eval(
        page,
        &format!("document.getElementById('{id}').getAttribute('data-svelte-state')"),
    )
    .await
}

async fn log(page: &Page) -> Vec<String> {
    eval(page, "window.__svelteLog || []").await
}

async fn mounted(page: &Page, id: &str) {
    page.expect_attribute(&format!("#{id}"), "data-svelte-state", "mounted")
        .await
        .expect("mounted");
}

// AC5: a real Svelte 5 component, under the default CSP.
#[tokio::test]
#[ignore = "requires Chromium"]
async fn real_svelte_component_mounts_with_props_and_reacts() {
    let runner = start().await;
    let page = runner.page().await.expect("page");
    page.visit("/real").await.expect("visit");
    mounted(&page, "counter").await;
    page.expect_text("Hits: 3").await.expect("props");
    let text: String = eval(&page, "document.getElementById('counter').textContent").await;
    assert!(!text.contains("(server)"), "fallback is hidden: {text}");
    page.click("#counter button[aria-label=Increase]")
        .await
        .expect("click");
    page.expect_text("Hits: 4").await.expect("reactive");
    let display: String = eval(
        &page,
        "getComputedStyle(document.querySelector('#counter .counter')).display",
    )
    .await;
    assert_eq!(display, "inline-flex", "component CSS from svelte_bundle");
    page.expect_no_console_errors()
        .await
        .expect("clean console");

    let response = app().build().get("/real").send().await;
    let csp = response
        .header("content-security-policy")
        .expect("CSP header")
        .to_owned();
    assert!(csp.contains("script-src 'self'"), "{csp}");
    assert!(!csp.contains("unsafe-eval"), "{csp}");
}

// AC6 with real Svelte: htmx removes a component with a timer.
#[tokio::test]
#[ignore = "requires Chromium"]
async fn htmx_swap_unmounts_a_real_svelte_component() {
    let runner = start().await;
    let page = runner.page().await.expect("page");
    page.visit("/clock").await.expect("visit");
    mounted(&page, "clock").await;
    let has_time: bool = eval(&page, "!!document.querySelector('#clock time.clock')").await;
    assert!(has_time, "Svelte rendered the clock");
    page.click("#clear").await.expect("click");
    page.expect_text("cleared").await.expect("swapped");
    wait_for(
        &page,
        "window.__svelteLog.includes('autumn:svelte:unmount:Clock')",
    )
    .await;
    let left: u32 = eval(&page, "document.querySelectorAll('time.clock').length").await;
    assert_eq!(left, 0);
    page.expect_no_console_errors()
        .await
        .expect("clean console");
}

// AC9: script order does not matter; one element mounts one time.
#[tokio::test]
#[ignore = "requires Chromium"]
async fn order_and_repeat_scans_mount_once() {
    let runner = start().await;
    let page = runner.page().await.expect("page");
    page.visit("/order").await.expect("visit");
    mounted(&page, "e").await;
    let loader: bool = eval(&page, "window.autumnSvelte.loader === true").await;
    assert!(loader, "the queue is now the loader API");
    run(
        &page,
        "autumnSvelte.scan(document); autumnSvelte.scan(document.body); \
         autumnSvelte.scan(document.getElementById('e'))",
    )
    .await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    let mounts = log(&page)
        .await
        .into_iter()
        .filter(|l| l.starts_with("mount:"))
        .count();
    assert_eq!(mounts, 1);
    let echoes: u32 = eval(&page, "document.querySelectorAll('#e .echo').length").await;
    assert_eq!(echoes, 1);
    page.expect_no_console_errors()
        .await
        .expect("clean console");
}

// AC6: htmx swaps mount new islands and unmount old ones.
#[tokio::test]
#[ignore = "requires Chromium"]
async fn htmx_swap_mounts_new_and_unmounts_old() {
    let runner = start().await;
    let page = runner.page().await.expect("page");
    page.visit("/swap").await.expect("visit");
    mounted(&page, "i1").await;
    page.click("#next").await.expect("click");
    mounted(&page, "i2").await;
    wait_for(
        &page,
        "window.__svelteLog.includes('destroy:{\"k\":\"1\"}')",
    )
    .await;
    let log = log(&page).await;
    assert!(
        log.contains(&"autumn:svelte:unmount:Echo".to_owned()),
        "{log:?}"
    );
    assert!(log.contains(&"mount:{\"k\":\"2\"}".to_owned()), "{log:?}");
    page.expect_no_console_errors()
        .await
        .expect("clean console");
}

// A moved island keeps its instance (morph, `appendChild`).
#[tokio::test]
#[ignore = "requires Chromium"]
async fn moved_island_keeps_its_instance() {
    let runner = start().await;
    let page = runner.page().await.expect("page");
    page.visit("/swap").await.expect("visit");
    mounted(&page, "i1").await;
    run(
        &page,
        "document.getElementById('other').appendChild(document.getElementById('i1'))",
    )
    .await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    let parent: String = eval(&page, "document.getElementById('i1').parentElement.id").await;
    assert_eq!(parent, "other");
    let log = log(&page).await;
    assert!(!log.iter().any(|l| l.starts_with("destroy:")), "{log:?}");
    assert_eq!(state(&page, "i1").await, "mounted");
}

// htmx history restore puts back mounted markup. It mounts again, clean.
#[tokio::test]
#[ignore = "requires Chromium"]
async fn restored_markup_mounts_again_from_the_fallback() {
    let runner = start().await;
    let page = runner.page().await.expect("page");
    page.visit("/swap").await.expect("visit");
    mounted(&page, "i1").await;
    // The same thing htmx does on history restore: serialize, then parse.
    run(
        &page,
        "(function () { var s = document.getElementById('slot'); \
           var copy = document.createRange().createContextualFragment(s.getHTML()); \
           s.replaceChildren(copy); })()",
    )
    .await;
    wait_for(
        &page,
        "window.__svelteLog.filter(function (l) { return l.indexOf('mount:') === 0; }).length === 2",
    )
    .await;
    mounted(&page, "i1").await;
    let echoes: u32 = eval(&page, "document.querySelectorAll('#i1 .echo').length").await;
    assert_eq!(echoes, 1, "no stale component DOM");
    let fallback: String = eval(
        &page,
        "document.querySelector('#i1 > template[data-svelte-fallback]').content.textContent",
    )
    .await;
    assert_eq!(fallback, "fallback 1");
}

// AC7: `idle` and `visible` wait for their trigger.
#[tokio::test]
#[ignore = "requires Chromium"]
async fn idle_and_visible_wait_for_their_trigger() {
    let runner = start().await;
    let page = runner.page().await.expect("page");
    page.visit("/lazy").await.expect("visit");
    mounted(&page, "idle").await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(state(&page, "visible").await, "pending");
    run(
        &page,
        "document.getElementById('gone').remove(); \
         document.getElementById('visible').scrollIntoView()",
    )
    .await;
    mounted(&page, "visible").await;
    run(&page, "window.scrollTo(0, document.body.scrollHeight)").await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    let log = log(&page).await;
    assert!(!log.iter().any(|l| l.contains("gone")), "{log:?}");
    page.expect_no_console_errors()
        .await
        .expect("clean console");
}

// AC8: a failed mount keeps the fallback and reports the error.
#[tokio::test]
#[ignore = "requires Chromium"]
async fn failed_mounts_keep_the_fallback_and_report() {
    let runner = start().await;
    let page = runner.page().await.expect("page");
    page.visit("/errors").await.expect("visit");
    for (id, text) in [
        ("boom", "boom fallback"),
        ("partial", "partial fallback"),
        ("badjson", "bad json fallback"),
        ("array", "array fallback"),
    ] {
        page.expect_attribute(&format!("#{id}"), "data-svelte-state", "error")
            .await
            .expect("error state");
        let shown: String = eval(
            &page,
            &format!("document.getElementById('{id}').textContent"),
        )
        .await;
        assert_eq!(shown, text, "#{id} shows only its fallback");
    }
    let log = log(&page).await;
    for name in ["Boom", "Partial", "Echo"] {
        assert!(
            log.contains(&format!("autumn:svelte:error:{name}")),
            "{name}: {log:?}"
        );
    }
    assert!(
        !page.console_errors().is_empty(),
        "the loader logs the errors"
    );
}

// Unknown names stay pending. A late registration mounts them.
#[tokio::test]
#[ignore = "requires Chromium"]
async fn unknown_and_prototype_names_stay_pending() {
    let runner = start().await;
    let page = runner.page().await.expect("page");
    page.visit("/names").await.expect("visit");
    mounted(&page, "ok").await;
    for id in ["later", "proto", "ctor", "own"] {
        assert_eq!(state(&page, id).await, "pending", "#{id}");
    }
    run(
        &page,
        "autumnSvelte.push({ \
           mount: function (C, o) { return C(o.target, o.props); }, \
           unmount: function () {}, \
           components: { Later: function (t) { t.append('later mounted'); return {}; } } \
         })",
    )
    .await;
    mounted(&page, "later").await;
    page.expect_text("later mounted").await.expect("late mount");
    page.expect_no_console_errors()
        .await
        .expect("clean console");
}
