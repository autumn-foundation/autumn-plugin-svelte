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

/// An ES-module bundle.
static MODULE: PluginAssets = PluginAssets::from_files(
    "svelte-module",
    &[("module.mjs", include_bytes!("fixtures/module.mjs"))],
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
        (svelte_bundle(&MODULE))
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
            body {
                // A named element clobbers `window.autumnSvelte`.
                div #autumnSvelte {}
                (echo("e", "order"))
            }
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

#[get("/probe.js")]
async fn probe_js() -> ([(&'static str, &'static str); 1], &'static str) {
    (
        [("content-type", "text/javascript")],
        include_str!("fixtures/probe.js"),
    )
}

#[get("/lazy")]
async fn lazy() -> Markup {
    html! {
        (maud::DOCTYPE)
        html {
            head { script src="/probe.js" {} (head()) }
            body { (lazy_body()) }
        }
    }
}

fn lazy_body() -> Markup {
    html! {
        (echo("idle", "idle").mount_when(MountWhen::Idle))
        div style="height: 5000px" {}
        (echo("visible", "visible").mount_when(MountWhen::Visible))
        (echo("gone", "gone").mount_when(MountWhen::Visible))
        (echo("idle-gone", "idle-gone").mount_when(MountWhen::Idle))
    }
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
        (Island::new("Module").id("module").props(&serde_json::json!({ "k": "m" })).expect("props"))
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

#[get("/morph")]
async fn morph() -> Markup {
    page(&html! {
        script src="/static/js/idiomorph.min.js" defer {}
        div hx-ext="morph" {
            button #morph hx-get="/morph/fragment" hx-target="#m1" hx-swap="morph:outerHTML" { "Morph" }
            button #preserve hx-get="/preserve/fragment" hx-target="#pslot" { "Preserve" }
            (echo("m1", "1"))
            div #pslot { (echo("p1", "p").id("p1")) }
        }
    })
}

#[get("/morph/fragment")]
async fn morph_fragment() -> Markup {
    html! { (echo("m1", "9")) }
}

#[get("/preserve/fragment")]
async fn preserve_fragment() -> Markup {
    html! { div #p1 hx-preserve data-svelte-island="Echo" { "new fallback" } }
}

#[get("/history")]
async fn history() -> Markup {
    page(&html! {
        button #go hx-get="/fragment/2" hx-target="#slot" hx-push-url="/history/2" { "Go" }
        div #slot { (echo("i1", "1")) }
        div style="height: 5000px" {}
        (echo("visible", "visible").mount_when(MountWhen::Visible))
        (Island::new("Nope").id("nope").fallback(html! { "nope fallback" }))
        (Island::new("Boom").id("boom").fallback(html! { "boom fallback" }))
    })
}

#[get("/ignore")]
async fn ignore() -> Markup {
    page(&html! {
        (echo("outside", "outside"))
        div #user data-svelte-ignore {
            div #injected data-svelte-island="Echo" data-svelte-props=r#"{"k":"pwn"}"# { "user text" }
        }
    })
}

#[get("/nest")]
async fn nest() -> Markup {
    page(&html! { (Island::new("Nest").id("nest")) })
}

#[get("/revive.js")]
async fn revive_js() -> ([(&'static str, &'static str); 1], &'static str) {
    (
        [("content-type", "text/javascript")],
        "window.__revived = true;",
    )
}

#[get("/foreign")]
async fn foreign() -> Markup {
    page(&html! {
        div #foreign data-svelte-island="Echo" data-svelte-state="mounted" {
            template data-svelte-fallback {
                "foreign fallback"
                script src="/revive.js" {}
            }
            span { "stale" }
        }
    })
}

fn app() -> TestApp {
    TestApp::new()
        .plugin(
            SveltePlugin::new()
                .bundle(&FAKE)
                .bundle(&REAL)
                .bundle(&MODULE),
        )
        .routes(routes![
            real,
            order,
            swap,
            fragment,
            lazy,
            errors,
            names,
            clock,
            empty,
            morph,
            morph_fragment,
            preserve_fragment,
            history,
            ignore,
            nest,
            revive_js,
            foreign,
            probe_js
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
    let script_src: Vec<&str> = csp
        .split(';')
        .map(str::trim)
        .filter(|d| d.starts_with("script-src"))
        .collect();
    assert_eq!(script_src, ["script-src 'self'"], "{csp}");
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

// AC7: `idle` and `visible` wait for their trigger; teardown cancels it.
#[tokio::test]
#[ignore = "requires Chromium"]
async fn idle_and_visible_wait_for_their_trigger() {
    let runner = start().await;
    let page = runner.page().await.expect("page");
    page.visit("/lazy").await.expect("visit");
    wait_for(
        &page,
        "!!window.autumnSvelte && window.autumnSvelte.loader === true",
    )
    .await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(state(&page, "idle").await, "pending", "idle waits");
    assert_eq!(state(&page, "visible").await, "pending", "visible waits");
    run(
        &page,
        "document.getElementById('gone').remove(); \
         document.getElementById('idle-gone').remove();",
    )
    .await;
    wait_for(
        &page,
        "window.__disconnects === 1 && window.__idleCancels === 1",
    )
    .await;
    run(&page, "window.__runIdle()").await;
    mounted(&page, "idle").await;
    run(&page, "document.getElementById('visible').scrollIntoView()").await;
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
    mounted(&page, "module").await;
    page.expect_text("module m")
        .await
        .expect("ES module bundle");
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

// Review fix: htmx history restore keeps the fallback of pending and
// failed islands.
#[tokio::test]
#[ignore = "requires Chromium"]
async fn history_restore_keeps_pending_and_error_fallbacks() {
    let runner = start().await;
    let page = runner.page().await.expect("page");
    page.visit("/history").await.expect("visit");
    mounted(&page, "i1").await;
    page.expect_attribute("#boom", "data-svelte-state", "error")
        .await
        .expect("error");
    page.click("#go").await.expect("click");
    mounted(&page, "i2").await;
    run(&page, "history.back()").await;
    wait_for(
        &page,
        "!!document.getElementById('i1') && \
         document.getElementById('i1').getAttribute('data-svelte-state') === 'mounted'",
    )
    .await;
    page.expect_attribute("#boom", "data-svelte-state", "error")
        .await
        .expect("error again");
    for (id, state_value, text) in [
        ("visible", "pending", "fallback visible"),
        ("nope", "pending", "nope fallback"),
        ("boom", "error", "boom fallback"),
    ] {
        assert_eq!(state(&page, id).await, state_value, "#{id}");
        let shown: String = eval(
            &page,
            &format!("document.getElementById('{id}').textContent"),
        )
        .await;
        assert_eq!(shown, text, "#{id} keeps its fallback");
    }
}

// Review fix: an in-place morph remounts with the new props; hx-preserve
// keeps the instance.
#[tokio::test]
#[ignore = "requires Chromium"]
async fn morph_remounts_with_new_props_and_preserve_keeps_instance() {
    let runner = start().await;
    let page = runner.page().await.expect("page");
    page.visit("/morph").await.expect("visit");
    mounted(&page, "m1").await;
    mounted(&page, "p1").await;
    page.click("#morph").await.expect("click");
    wait_for(
        &page,
        "window.__svelteLog.includes('destroy:{\"k\":\"1\"}') && \
         window.__svelteLog.includes('mount:{\"k\":\"9\"}')",
    )
    .await;
    mounted(&page, "m1").await;
    let echoes: Vec<String> = eval(
        &page,
        "Array.from(document.querySelectorAll('#m1 .echo')).map(function (e) { return e.textContent; })",
    )
    .await;
    assert_eq!(echoes, ["{\"k\":\"9\"}"]);
    page.click("#preserve").await.expect("click");
    tokio::time::sleep(Duration::from_millis(300)).await;
    let log = log(&page).await;
    assert!(
        !log.contains(&"destroy:{\"k\":\"p\"}".to_owned()),
        "{log:?}"
    );
    assert_eq!(state(&page, "p1").await, "mounted");
    page.expect_no_console_errors()
        .await
        .expect("clean console");
}

// Review fix: a registration from inside mount() does not mount twice.
#[tokio::test]
#[ignore = "requires Chromium"]
async fn reentrant_registration_mounts_once() {
    let runner = start().await;
    let page = runner.page().await.expect("page");
    page.visit("/names").await.expect("visit");
    mounted(&page, "ok").await;
    run(
        &page,
        "window.__n = 0; autumnSvelte.push({ \
           mount: function (C, o) { return C(o.target); }, unmount: function () {}, \
           components: { Later: function (t) { \
             window.__n++; \
             autumnSvelte.push({ mount: function () { return {}; }, unmount: function () {}, \
               components: { Other: function () {} } }); \
             t.append('L' + window.__n); return {}; } } })",
    )
    .await;
    mounted(&page, "later").await;
    let result: (u32, String) = eval(
        &page,
        "[window.__n, document.getElementById('later').textContent]",
    )
    .await;
    assert_eq!(result, (1, "L1".to_owned()));
}

// Review fix: remove, register, re-add in one task still mounts.
#[tokio::test]
#[ignore = "requires Chromium"]
async fn remove_register_readd_in_one_task_mounts() {
    let runner = start().await;
    let page = runner.page().await.expect("page");
    page.visit("/names").await.expect("visit");
    mounted(&page, "ok").await;
    run(
        &page,
        "var e = document.getElementById('later'); var p = e.parentNode; e.remove(); \
         autumnSvelte.push({ mount: function (C, o) { return C(o.target); }, unmount: function () {}, \
           components: { Later: function (t) { t.append('later mounted'); return {}; } } }); \
         p.appendChild(e);",
    )
    .await;
    mounted(&page, "later").await;
    page.expect_text("later mounted").await.expect("mounted");
}

// Review fix: teardown clears the state; a rejected unmount is handled.
#[tokio::test]
#[ignore = "requires Chromium"]
async fn teardown_clears_state_and_handles_async_unmount() {
    let runner = start().await;
    let page = runner.page().await.expect("page");
    page.visit("/names").await.expect("visit");
    mounted(&page, "ok").await;
    run(
        &page,
        "autumnSvelte.push({ mount: function (C, o) { return C(o.target); }, \
           unmount: function () { return Promise.reject(new Error('async unmount')); }, \
           components: { Later: function (t) { t.append('x'); return {}; } } })",
    )
    .await;
    mounted(&page, "later").await;
    run(
        &page,
        "window.__ok = document.getElementById('ok'); window.__ok.remove(); \
         window.__later = document.getElementById('later'); window.__later.remove();",
    )
    .await;
    wait_for(
        &page,
        "window.__svelteLog.includes('destroy:{\"k\":\"ok\"}')",
    )
    .await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    let states: (bool, bool) = eval(
        &page,
        "[window.__ok.hasAttribute('data-svelte-state'), \
          window.__later.hasAttribute('data-svelte-state')]",
    )
    .await;
    assert_eq!(states, (false, false));
    let log = log(&page).await;
    assert!(!log.contains(&"unhandledrejection".to_owned()), "{log:?}");
    assert!(
        page.console_errors()
            .iter()
            .any(|e| e.contains("failed to unmount")),
        "{:?}",
        page.console_errors()
    );
}

// Security review fix: islands inside `data-svelte-ignore` never mount.
#[tokio::test]
#[ignore = "requires Chromium"]
async fn ignore_boundary_blocks_injected_islands() {
    let runner = start().await;
    let page = runner.page().await.expect("page");
    page.visit("/ignore").await.expect("visit");
    mounted(&page, "outside").await;
    run(
        &page,
        "var d = document.createElement('div'); d.id = 'late'; \
         d.setAttribute('data-svelte-island', 'Echo'); \
         document.getElementById('user').appendChild(d); \
         autumnSvelte.scan(document);",
    )
    .await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    let untouched: (bool, bool) = eval(
        &page,
        "[document.getElementById('injected').hasAttribute('data-svelte-state'), \
          document.getElementById('late').hasAttribute('data-svelte-state')]",
    )
    .await;
    assert_eq!(untouched, (false, false));
    let log = log(&page).await;
    assert!(!log.iter().any(|l| l.contains("pwn")), "{log:?}");
    page.expect_no_console_errors()
        .await
        .expect("clean console");
}

// Security review fix: self-nesting islands stop at the depth limit.
#[tokio::test]
#[ignore = "requires Chromium"]
async fn nested_islands_stop_at_the_depth_limit() {
    let runner = start().await;
    let page = runner.page().await.expect("page");
    page.visit("/nest").await.expect("visit");
    wait_for(
        &page,
        "document.querySelectorAll('[data-svelte-state=error]').length === 1",
    )
    .await;
    let mounted_count: u32 = eval(
        &page,
        "document.querySelectorAll('[data-svelte-state=mounted]').length",
    )
    .await;
    assert_eq!(mounted_count, 16);
}

// Security review fix: a fallback template from markup does not run scripts.
#[tokio::test]
#[ignore = "requires Chromium"]
async fn foreign_fallback_template_does_not_revive_scripts() {
    let runner = start().await;
    let page = runner.page().await.expect("page");
    page.visit("/foreign").await.expect("visit");
    mounted(&page, "foreign").await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    let revived: bool = eval(&page, "window.__revived === true").await;
    assert!(!revived, "the template script must not run");
    let text: String = eval(
        &page,
        "document.querySelector('#foreign > template').content.textContent",
    )
    .await;
    assert_eq!(text, "foreign fallback");
}

// The first registration of a name stays; a second one is an error.
#[tokio::test]
#[ignore = "requires Chromium"]
async fn duplicate_registration_keeps_the_first() {
    let runner = start().await;
    let page = runner.page().await.expect("page");
    page.visit("/names").await.expect("visit");
    mounted(&page, "ok").await;
    run(
        &page,
        "autumnSvelte.push({ mount: function (C, o) { return C(o.target); }, unmount: function () {}, \
           components: { Echo: function (t) { t.append('impostor'); return {}; } } }); \
         var d = document.createElement('div'); d.id = 'second'; \
         d.setAttribute('data-svelte-island', 'Echo'); \
         d.setAttribute('data-svelte-props', '{\"k\":\"second\"}'); \
         document.body.appendChild(d);",
    )
    .await;
    mounted(&page, "second").await;
    let text: String = eval(&page, "document.getElementById('second').textContent").await;
    assert_eq!(text, "{\"k\":\"second\"}");
    assert!(
        page.console_errors()
            .iter()
            .any(|e| e.contains("already registered")),
        "{:?}",
        page.console_errors()
    );
}
