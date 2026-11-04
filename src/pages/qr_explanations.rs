use maud::{html, Markup};
use crate::pages::layout;

pub fn qr_page() -> Markup {
    layout(
        "QR Code Encoder",
        html!(
            link rel="stylesheet" href="/static/qr.css" {}
        ),
        html!{
            
        }
    )
}