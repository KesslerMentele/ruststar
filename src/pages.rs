pub(crate) mod toys;
pub(crate) mod portfolio;
pub(crate) mod todo;
pub(crate) mod qr_generator;

use maud::{html, Markup, DOCTYPE};

pub fn layout(title: &str, head:Markup, content: Markup) -> Markup {
    html! {
        (DOCTYPE)
        html {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1.0";
                title { (title) }
                link rel="stylesheet" href="/assets/root.css";
                (head)
                script type="module" src="https://cdn.jsdelivr.net/gh/starfederation/datastar@v1.0.2/bundles/datastar.js" {}
            }
            body id="root" {
                (header())
                main class="content-container" {
                    (content)
                }
            }
        }
    }
}

fn header() -> Markup {
    html! {
        nav class="header papered" {
            (home_button())
            header {
                    h1 { "Kessler Mentele" }
                    p id="tagline" { "Backend-leaning developer." }
                }
            div class="header-links" {

            }
        }
    }
}

fn home_button() -> Markup {
    html! {
       a href="/" { "Home" }
    }
}

