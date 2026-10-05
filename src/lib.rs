//! Svelte 5 islands for Autumn, in Maud + htmx pages.
//!
//! The server renders an [`Island`]. The browser loader (`islands.js`)
//! mounts a compiled Svelte component into it.
//!
//! ```rust,no_run
//! use autumn_plugin_svelte::{Island, MountWhen, SveltePlugin, svelte_bundle, svelte_script};
//! use autumn_web::assets::PluginAssets;
//! use autumn_web::prelude::*;
//!
//! // Your Vite build output, compiled into the binary.
//! static ISLANDS: PluginAssets = PluginAssets::from_files(
//!     "app-islands",
//!     &[("islands.js", b"/* vite build output */")],
//! );
//!
//! #[get("/")]
//! async fn index() -> AutumnResult<Markup> {
//!     let counter = Island::new("Counter")
//!         .props(&serde_json::json!({ "start": 3 }))?
//!         .fallback(html! { p { "Counter: 3" } });
//!     Ok(html! {
//!         head { (svelte_script()) (svelte_bundle(&ISLANDS)) }
//!         body {
//!             (counter)
//!             (Island::new("Chart").mount_when(MountWhen::Visible))
//!         }
//!     })
//! }
//!
//! # async fn run() {
//! autumn_web::app()
//!     .plugin(SveltePlugin::new().bundle(&ISLANDS))
//!     .routes(routes![index])
//!     .run()
//!     .await;
//! # }
//! ```
//!
//! The Vite entry file registers the components:
//!
//! ```js
//! import { mount, unmount } from "svelte";
//! import Counter from "./Counter.svelte";
//!
//! let queue = window.autumnSvelte;
//! if (!Array.isArray(queue) && queue?.loader !== true) {
//!   queue = window.autumnSvelte = []; // an id="autumnSvelte" element can clobber it
//! }
//! queue.push({ mount, unmount, components: { Counter } });
//! ```
//!
//! # Limits
//!
//! - No server-side render of the component. The fallback is the first paint.
//! - You need Node to build components. You do not need Node at run time.
//! - Props go in an HTML attribute. Keep them small.

mod assets;
mod island;
mod plugin;
mod tags;

pub use assets::{ASSETS_NAMESPACE, LOADER_JS, SVELTE_ASSETS};
pub use island::{Island, JsonKind, MountWhen, PropsError};
pub use plugin::{PLUGIN_NAME, SveltePlugin};
pub use tags::{svelte_bundle, svelte_script};
