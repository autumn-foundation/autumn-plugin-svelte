//! [`Island`]: the server side of one Svelte island.

use maud::{Markup, Render};
use serde::Serialize;

/// When the loader mounts an island.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum MountWhen {
    /// Mount when the loader starts. This is the default.
    #[default]
    Load,
    /// Mount when the browser is idle.
    Idle,
    /// Mount when the island comes into the viewport.
    Visible,
}

impl MountWhen {
    /// The `data-svelte-mount` value. `None` for [`MountWhen::Load`].
    const fn attr(self) -> Option<&'static str> {
        match self {
            Self::Load => None,
            Self::Idle => Some("idle"),
            Self::Visible => Some("visible"),
        }
    }
}

/// The error from [`Island::props`].
///
/// It converts to `AutumnError` (status 500), so `?` works in a handler.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum PropsError {
    /// The value does not serialize to JSON.
    #[error("island props do not serialize to JSON: {0}")]
    Serialize(#[from] serde_json::Error),
    /// The value is JSON, but not a JSON object.
    #[error("island props must be a JSON object, not {0}")]
    NotAnObject(&'static str),
}

/// One Svelte island: an element that the loader mounts a component into.
///
/// It renders as:
///
/// ```html
/// <div data-svelte-island="Counter" data-svelte-props="{...}"
///      data-svelte-mount="visible" id="..." class="...">fallback</div>
/// ```
///
/// The loader replaces the fallback with the component. Without JavaScript,
/// or when the mount fails, the fallback stays.
///
/// ```rust
/// use autumn_plugin_svelte::{Island, MountWhen};
/// use maud::{Render, html};
///
/// let island = Island::new("Counter")
///     .props(&serde_json::json!({ "start": 3 }))?
///     .mount_when(MountWhen::Visible)
///     .fallback(html! { p { "Counter: 3" } });
/// let html = island.render().into_string();
/// assert!(html.starts_with(r#"<div data-svelte-island="Counter""#));
/// # Ok::<(), autumn_plugin_svelte::PropsError>(())
/// ```
#[derive(Debug, Clone)]
#[must_use]
pub struct Island {
    name: String,
    props: Option<String>,
    when: MountWhen,
    fallback: Option<Markup>,
    id: Option<String>,
    class: Option<String>,
}

impl Island {
    /// Makes an island for the component that the app bundle registers as
    /// `name`.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            props: None,
            when: MountWhen::Load,
            fallback: None,
            id: None,
            class: None,
        }
    }

    /// Sets the component props. The value must serialize to a JSON object.
    ///
    /// # Errors
    ///
    /// - [`PropsError::Serialize`] when `serde_json` cannot serialize the
    ///   value (for example, a map with non-string keys).
    /// - [`PropsError::NotAnObject`] when the JSON is not an object.
    pub fn props<T: Serialize + ?Sized>(mut self, props: &T) -> Result<Self, PropsError> {
        // Serialize once and keep the field order. The first byte of JSON
        // text tells its kind.
        let json = serde_json::to_string(props)?;
        let kind = match json.as_bytes().first() {
            Some(b'{') => None,
            Some(b'[') => Some("an array"),
            Some(b'"') => Some("a string"),
            Some(b'n') => Some("null"),
            Some(b't' | b'f') => Some("a boolean"),
            _ => Some("a number"),
        };
        if let Some(kind) = kind {
            return Err(PropsError::NotAnObject(kind));
        }
        self.props = Some(json);
        Ok(self)
    }

    /// Sets when the loader mounts the island. The default is
    /// [`MountWhen::Load`].
    pub const fn mount_when(mut self, when: MountWhen) -> Self {
        self.when = when;
        self
    }

    /// Sets the server content. The page shows it before the mount, without
    /// JavaScript, and after a failed mount.
    pub fn fallback(mut self, content: Markup) -> Self {
        self.fallback = Some(content);
        self
    }

    /// Sets the `id` attribute.
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Sets the `class` attribute.
    pub fn class(mut self, class: impl Into<String>) -> Self {
        self.class = Some(class.into());
        self
    }
}

impl Render for Island {
    fn render(&self) -> Markup {
        maud::html! {
            div data-svelte-island=(self.name)
                data-svelte-props=[self.props.as_deref()]
                data-svelte-mount=[self.when.attr()]
                id=[self.id.as_deref()]
                class=[self.class.as_deref()]
            {
                @if let Some(fallback) = &self.fallback { (fallback) }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use maud::html;
    use proptest::prelude::*;
    use std::collections::BTreeMap;

    /// Reverses Maud attribute escaping.
    fn unescape(s: &str) -> String {
        s.replace("&quot;", "\"")
            .replace("&#39;", "'")
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&amp;", "&")
    }

    /// The raw (escaped) value of `attr` in `html`.
    fn attr<'a>(html: &'a str, attr: &str) -> Option<&'a str> {
        let start = html.find(&format!(" {attr}=\""))? + attr.len() + 3;
        let end = html[start..].find('"')? + start;
        Some(&html[start..end])
    }

    #[test]
    fn minimal_island_renders_only_the_name() {
        let html = Island::new("Counter").render().into_string();
        assert_eq!(html, r#"<div data-svelte-island="Counter"></div>"#);
    }

    #[test]
    fn props_render_as_json_attribute() {
        #[derive(Serialize)]
        struct Props {
            start: i32,
            label: &'static str,
        }
        let html = Island::new("Counter")
            .props(&Props {
                start: 3,
                label: "Hits",
            })
            .unwrap()
            .render()
            .into_string();
        let raw = attr(&html, "data-svelte-props").expect("props attribute");
        assert_eq!(unescape(raw), r#"{"start":3,"label":"Hits"}"#);
    }

    #[test]
    fn props_escape_markup() {
        let html = Island::new("X")
            .props(&serde_json::json!({ "s": "\"><script>alert(1)</script>&'" }))
            .unwrap()
            .render()
            .into_string();
        assert!(!html.contains("<script"), "{html}");
        let raw = attr(&html, "data-svelte-props").expect("props attribute");
        let value: serde_json::Value = serde_json::from_str(&unescape(raw)).unwrap();
        assert_eq!(value["s"], "\"><script>alert(1)</script>&'");
    }

    #[test]
    fn non_object_props_are_refused() {
        for (value, kind) in [
            (serde_json::json!(5), "a number"),
            (serde_json::json!([1]), "an array"),
            (serde_json::json!("s"), "a string"),
            (serde_json::json!(null), "null"),
            (serde_json::json!(true), "a boolean"),
        ] {
            let err = Island::new("X").props(&value).unwrap_err();
            assert!(
                matches!(err, PropsError::NotAnObject(k) if k == kind),
                "{err}"
            );
            assert_eq!(
                err.to_string(),
                format!("island props must be a JSON object, not {kind}")
            );
        }
    }

    #[test]
    fn unserializable_props_are_refused() {
        let mut map = BTreeMap::new();
        map.insert(vec![1_u8], 1);
        let err = Island::new("X").props(&map).unwrap_err();
        assert!(matches!(err, PropsError::Serialize(_)), "{err}");
    }

    #[test]
    fn props_error_converts_to_autumn_error() {
        fn handler() -> autumn_web::AutumnResult<Island> {
            Ok(Island::new("X").props(&5)?)
        }
        let err = handler().unwrap_err();
        assert_eq!(
            err.status(),
            autumn_web::reexports::http::StatusCode::INTERNAL_SERVER_ERROR
        );
    }

    #[test]
    fn mount_strategy_renders_except_for_load() {
        let render = |when| Island::new("X").mount_when(when).render().into_string();
        assert!(!render(MountWhen::Load).contains("data-svelte-mount"));
        assert!(render(MountWhen::Idle).contains(r#"data-svelte-mount="idle""#));
        assert!(render(MountWhen::Visible).contains(r#"data-svelte-mount="visible""#));
        assert_eq!(MountWhen::default(), MountWhen::Load);
    }

    #[test]
    fn fallback_id_and_class_render() {
        let html = Island::new("Counter")
            .id("c1")
            .class("card wide")
            .fallback(html! { p { "Counter: 3" } })
            .render()
            .into_string();
        assert_eq!(
            html,
            r#"<div data-svelte-island="Counter" id="c1" class="card wide"><p>Counter: 3</p></div>"#
        );
    }

    #[test]
    fn island_renders_inside_html_macro() {
        let page = html! { main { (Island::new("A")) } }.into_string();
        assert_eq!(page, r#"<main><div data-svelte-island="A"></div></main>"#);
    }

    #[test]
    fn last_setter_wins() {
        let html = Island::new("X")
            .props(&serde_json::json!({"a": 1}))
            .unwrap()
            .props(&serde_json::json!({"b": 2}))
            .unwrap()
            .mount_when(MountWhen::Idle)
            .mount_when(MountWhen::Visible)
            .render()
            .into_string();
        assert_eq!(
            unescape(attr(&html, "data-svelte-props").unwrap()),
            r#"{"b":2}"#
        );
        assert!(html.contains(r#"data-svelte-mount="visible""#));
        assert!(!html.contains("idle"));
    }

    proptest! {
        /// Any string props survive the attribute round trip.
        #[test]
        fn props_round_trip(map in proptest::collection::btree_map(".*", ".*", 0..6)) {
            let html = Island::new("X").props(&map).unwrap().render().into_string();
            let raw = attr(&html, "data-svelte-props").unwrap();
            prop_assert!(!raw.contains('<') && !raw.contains('>'));
            let back: BTreeMap<String, String> = serde_json::from_str(&unescape(raw)).unwrap();
            prop_assert_eq!(back, map);
        }

        /// No name, id or class escapes its attribute.
        #[test]
        fn attributes_never_break_out(name in ".*", id in ".*", class in ".*") {
            let html = Island::new(name.clone()).id(id.clone()).class(class.clone())
                .render().into_string();
            prop_assert!(html.starts_with("<div data-svelte-island=\""));
            prop_assert!(html.ends_with("\"></div>"));
            prop_assert_eq!(html.matches('<').count(), 2);
            prop_assert_eq!(unescape(attr(&html, "data-svelte-island").unwrap()), name);
            prop_assert_eq!(unescape(attr(&html, "id").unwrap()), id);
            prop_assert_eq!(unescape(attr(&html, "class").unwrap()), class);
        }
    }
}
