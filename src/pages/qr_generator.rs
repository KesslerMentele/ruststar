use std::sync::Arc;
use axum::response::IntoResponse;
use axum::{Json, Router};
use axum::routing::{get, post};
use maud::{html, Markup};
use crate::AppState;
use crate::pages::layout;
use crate::services::qr_code::prelude::*;

fn qr_page() -> Markup {
    layout(
        "QR Generator",
        html!{ link rel="stylesheet" href="/assets/qr_generator.css"; },
        html!{
            (sidebar())
        }
    )
}

fn render_qr_code(Json(payload): Json<QRData>) -> impl IntoResponse {
    let code = create_qr_code(payload);
    
}

fn sidebar() -> Markup {
    html! {
        div class="sidebar" {
            form class="sidebar-controls"
                data-signals="{data: 'Test', errorCorrectionLevel: 'L', encodingMode: 'AutoDetect', version: 'AutoDetect', mask: 'choose best'}"
                data-on:change="@post('/qr/')"
            {
                input type="text" data-bind="data";
                select data-bind:"errorCorrectionLevel" {
                    option value="L" {"L"}
                    option value="M" {"M"}
                    option value="Q" {"Q"}
                    option value="H" {"H"}
                }
                select data-bind:"encodingMode" {
                    @for mode in [ "Numeric", "Alphanumeric", "Byte", "Kanji", "AutoDetect", "Segmented"] {
                        option value=(mode) {(mode)};
                    }
                }
                select data-bind:"version" {
                    @for version in 1..=40 {
                        option value=(version) {(version)};
                    }
                    option value="AutoDetect" {"AutoDetect"}
                }
                select data-bind:"mask" {
                    @for mask in 0..=7 {
                        option value=(mask) {(mask)};
                    }
                    option value="choose best" {"Choose Best"}
                }
            }
        }
    }
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/generator", get(qr_page()))
        .route("/generate", post(render_qr_code))
}

// The page must generate bindings for:
// Text input field -> data str
// dropdown -> error_correction_level: L M Q H, user must choose
// dropdown -> encoding_mode: default autodetect
// dropdown -> version: default auto
// dropdown -> mask: default auto