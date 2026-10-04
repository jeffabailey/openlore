//! Page renderers (maud). Each page is chrome + body, as in ADR-032.
//! Copy constants here are the single source of truth for the app's text.

use maud::{html, Markup, DOCTYPE};

/// The name Bluesky shows when the app asks for permission.
pub const APP_NAME: &str = "OpenLore review";

/// What the app is for, in one line.
pub const APP_PURPOSE: &str =
    "Review the philosophy claims OpenLore suggests from your public GitHub repos, \
     and publish only the ones you approve to your own Bluesky repo.";

/// The shared page chrome. `body` is the page-specific fragment.
fn page(title: &str, body: Markup) -> Markup {
    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { (title) " · " (APP_NAME) }
            }
            body {
                header { strong { (APP_NAME) } }
                main { (body) }
            }
        }
    }
}

/// The public landing page.
pub fn landing_page() -> String {
    page(
        "Welcome",
        html! {
            h1 { (APP_NAME) }
            p { (APP_PURPOSE) }
        },
    )
    .into_string()
}
