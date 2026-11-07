use std::fmt::Write;
use axum::response::IntoResponse;
use axum::{Json, Router};
use axum::routing::{get, post};
use datastar::prelude::{PatchElements, PatchSignals};
use maud::{html, Markup};
use percent_encoding::{utf8_percent_encode, AsciiSet, CONTROLS};
use serde_json::json;
use tower_http::compression::{predicate::SizeAbove, CompressionLayer};
use crate::sse_response;
use crate::pages::layout;
use crate::services::qr_code::prelude::*;

/// The light border scanners need around a code, in modules.
const QUIET_ZONE: usize = 4;

/// What can't appear literally in a data URI: `#` would end it, and `%` would start an escape.
const DATA_URI_ESCAPES: &AsciiSet = &CONTROLS.add(b' ').add(b'"').add(b'#').add(b'%').add(b'<').add(b'>');

/// The `_qr_stats` signal when there's no code to describe. Stats hide while `version` is 0.
const NO_STATS: &str = r#"{"version":0,"error_correction_level":"","mask":0,"encoding_modes":[],"segments":[]}"#;

/// Responses smaller than this aren't worth compressing, in bytes.
const MIN_COMPRESSED_SIZE: u64 = 256;

fn qr_page() -> Markup {
    layout(
        "QR Code Generator",
        "",
        html!{ link rel="stylesheet" href="/assets/qr.css"; },
        html!{
            div class="qr-layout" {
                (sidebar())
                // `#qr-code` is replaced by every patch, so centring has to live on its parent.
                div class="qr-preview" {
                    div id="qr-code" {}
                }
            }
            (stats())
        }
    )
}

fn stats() -> Markup {
    html! {
        dl id="qr-stats" class="papered" style="display: none" data-show="$_qr_stats.version" {
            dt { "Version" }
            dd data-text="$_qr_stats.version" {}
            dt { "Error correction" }
            dd data-text="$_qr_stats.error_correction_level" {}
            dt { "Mask" }
            dd data-text="$_qr_stats.mask" {}
            dt { "Encoding modes" }
            dd data-text="$_qr_stats.encoding_modes.join(', ')" {}
        }
    }
}

/// One path for the whole symbol, with a rectangle per horizontal run of dark modules.
fn qr_path(qr_code: &QRMatrix) -> String {
    let size = qr_code.size();
    let mut path = String::new();
    for row in 0..size {
        let mut col = 0;
        while col < size {
            if !qr_code.is_dark(row, col) {
                col += 1;
                continue;
            }
            let start = col;
            while col < size && qr_code.is_dark(row, col) {
                col += 1;
            }
            let (x, y) = (start + QUIET_ZONE, row + QUIET_ZONE);
            write!(path, "M{x} {y}h{}v1H{x}z", col - start).expect("writing to a String can't fail");
        }
    }
    path
}

fn qr_svg(qr_code: &QRMatrix) -> Markup {
    let full_size = qr_code.size() + QUIET_ZONE * 2;
    html! {
        svg xmlns="http://www.w3.org/2000/svg"
            viewBox=(format!("0 0 {full_size} {full_size}"))
            shape-rendering="crispEdges"
        {
            rect width="100%" height="100%" fill="#fff" {}
            path d=(qr_path(qr_code)) fill="#000" {}
        }
    }
}

/// The code as an `<img>`, because browsers offer "Save image" on images
fn qr_image(qr_code: &QRMatrix) -> Markup {
    let full_size = qr_code.size() + QUIET_ZONE * 2;
    let svg = qr_svg(qr_code).into_string();
    html! {
        img id="qr-code" alt="QR code"
            src=(format!("data:image/svg+xml,{}", utf8_percent_encode(&svg, DATA_URI_ESCAPES)))
            style=(format!("--qr-modules: {full_size}"));
    }
}

async fn render_qr_code(Json(payload): Json<QRData>) -> impl IntoResponse {
    let (element, stats) = match create_qr_code(payload) {
        Ok(qr_code) => (qr_image(&qr_code.matrix), json!({ "_qr_stats": qr_code.stats }).to_string()),
        Err(e) => {
            tracing::warn!("Error creating qr code: {e:?}");
            let message = html! { p id="qr-code" { "Couldn't create a QR code with these settings." } };
            (message, format!(r#"{{"_qr_stats":{NO_STATS}}}"#))
        }
    };
    sse_response(vec![PatchElements::new(element).into(), PatchSignals::new(stats).into()])
}

fn help(text: &str) -> Markup {
    html! {
        span class="help" role="note" tabindex="0" aria-label=(text) data-on:click__prevent="void 0" {
            span aria-hidden="true" { "?" }
            span class="help-text" aria-hidden="true" { (text) }
        }
    }
}

fn sidebar() -> Markup {
    html! {
        div class="sidebar papered" {
            form class="sidebar-controls"
                data-signals=(format!("{{text: 'Test', error_correction_level: 'L', encoding_mode: 'Auto', version: 'Auto', mask: 'Auto', _multiline: false, _qr_stats: {NO_STATS}}}"))
                data-on:"input__debounce.300ms"="@post('/qr/generate')"
                data-on:submit="@post('/qr/generate')"
                data-init="@post('/qr/generate')"
            {
                label class="field" {
                    span class="field-label" {
                        "Text"
                        (help("What the code will contain. Longer text needs a larger code."))
                    }
                    input type="text" data-bind="text" data-show="!$_multiline";
                    textarea rows="4" data-bind="text" style="display: none" data-show="$_multiline" {}
                }
                label class="toggle" {
                    input type="checkbox" data-bind="_multiline"
                        data-on:change=r#"$_multiline || ($text = $text.replace(/\n/g, ' '))"#;
                    " Multiline"
                }
                label class="field" {
                    span class="field-label" {
                        "Error correction"
                        (help("How much damage the code can survive: about 7% at L, 15% at M, 25% at Q and 30% at H. Higher levels make the code larger."))
                    }
                    select data-bind:"error_correction_level" {
                        option value="L" {"L"}
                        option value="M" {"M"}
                        option value="Q" {"Q"}
                        option value="H" {"H"}
                    }
                }
                label class="field" {
                    span class="field-label" {
                        "Encoding mode"
                        (help("How characters are stored. Numeric and Alphanumeric are compact but only allow some characters, Byte allows anything, and Kanji is for Japanese text. Auto picks the best single mode, and Segmented mixes modes."))
                    }
                    select data-bind:"encoding_mode" {
                        @for mode in [ "Auto", "Numeric", "Alphanumeric", "Byte", "Kanji", "Segmented"] {
                            option value=(mode) {(mode)};
                        }
                    }
                }
                label class="field" {
                    span class="field-label" {
                        "Version"
                        (help("The code's size, from 1 (21×21 modules) to 40 (177×177). Auto picks the smallest that fits."))
                    }
                    select data-bind:"version" {
                        option value="Auto" {"Auto"}
                        @for version in 1..=40 {
                            option value=(version) {(version)};
                        }
                    }
                }
                label class="field" {
                    span class="field-label" {
                        "Mask"
                        (help("A pattern that breaks up areas scanners find hard to read. Auto picks the one with the lowest penalty score."))
                    }
                    select data-bind:"mask" {
                        option value="Auto" {"Auto"}
                        @for mask in 0..=7 {
                            option value=(mask) {(mask)};
                        }
                    }
                }
            }
        }
    }
}

pub fn router<S: Clone + Send + Sync + 'static>() -> Router<S> {
    Router::new()
        .route("/generator", get(qr_page()))
        .route("/generate", post(render_qr_code))
        .layer(CompressionLayer::new().compress_when(SizeAbove::new(MIN_COMPRESSED_SIZE)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::{to_bytes, Body};
    use axum::http::{header, Request, Response};
    use tower::ServiceExt;

    fn request_json(text: &str, encoding_mode: &str, version: &str) -> String {
        format!(r#"{{"text": "{text}", "error_correction_level": "L", "encoding_mode": "{encoding_mode}", "version": "{version}", "mask": "2"}}"#)
    }

    /// Sends `json` through the real router, optionally accepting gzip.
    async fn post(json: String, gzip: bool) -> Response<Body> {
        let mut request = Request::post("/generate").header(header::CONTENT_TYPE, "application/json");
        if gzip {
            request = request.header(header::ACCEPT_ENCODING, "gzip");
        }
        router::<()>().oneshot(request.body(Body::from(json)).unwrap()).await.unwrap()
    }

    async fn body_bytes(response: Response<Body>) -> Vec<u8> {
        to_bytes(response.into_body(), usize::MAX).await.unwrap().to_vec()
    }

    async fn body_text(json: String) -> String {
        String::from_utf8(body_bytes(post(json, false).await).await).unwrap()
    }

    fn code(text: &str) -> QRMatrix {
        create_qr_code(serde_json::from_str(&request_json(text, "Auto", "Auto")).unwrap()).unwrap().matrix
    }

    #[test]
    fn path_covers_exactly_the_dark_modules() {
        let qr_code = code("HELLO WORLD");
        let size = qr_code.size();
        let mut covered = vec![false; size * size];

        for rectangle in qr_path(&qr_code).split_terminator('z') {
            let (position, rest) = rectangle.trim_start_matches('M').split_once('h').unwrap();
            let (x, y) = position.split_once(' ').unwrap();
            let (width, closing) = rest.split_once("v1H").unwrap();
            assert_eq!(closing, x, "a rectangle must close back at its start");

            let x: usize = x.parse().unwrap();
            let y: usize = y.parse().unwrap();
            let width: usize = width.parse().unwrap();
            for col in x..x + width {
                let index = (y - QUIET_ZONE) * size + (col - QUIET_ZONE);
                assert!(!covered[index], "module {index} is drawn twice");
                covered[index] = true;
            }
        }

        for row in 0..size {
            for col in 0..size {
                assert_eq!(covered[row * size + col], qr_code.is_dark(row, col), "module ({row}, {col})");
            }
        }
    }

    #[test]
    fn svg_shapes_are_closed_and_the_view_includes_the_quiet_zone() {
        let qr_code = code("HELLO");
        let svg = qr_svg(&qr_code).into_string();
        assert!(svg.contains("</rect>"));
        assert!(svg.contains("</path>"));
        let full_size = qr_code.size() + QUIET_ZONE * 2;
        assert!(svg.contains(&format!(r#"viewBox="0 0 {full_size} {full_size}""#)));
    }

    #[test]
    fn image_tells_the_stylesheet_its_module_count_including_the_quiet_zone() {
        let small = qr_image(&code("HELLO")).into_string();
        assert!(small.contains(r#"style="--qr-modules: 29""#), "version 1 is 21 modules + 8");

        let json = request_json("HELLO", "Auto", "40");
        let large = qr_image(&create_qr_code(serde_json::from_str(&json).unwrap()).unwrap().matrix).into_string();
        assert!(large.contains(r#"style="--qr-modules: 185""#), "version 40 is 177 modules + 8");
    }

    #[tokio::test]
    async fn generate_patches_in_an_image() {
        let body = body_text(request_json("HELLO", "Auto", "1")).await;
        assert!(body.contains("datastar-patch-elements"));
        assert!(body.contains(r#"<img id="qr-code""#));
        assert!(!body.contains("<svg"), "the SVG must only be inside the data URI");
    }

    #[test]
    fn image_data_uri_decodes_to_the_exact_svg() {
        let qr_code = code("HELLO WORLD");
        let image = qr_image(&qr_code).into_string();
        let (_, rest) = image.split_once(r#"src="data:image/svg+xml,"#).unwrap();
        let (encoded, _) = rest.split_once('"').unwrap();

        for raw in [' ', '#', '<', '>'] {
            assert!(!encoded.contains(raw), "{raw:?} must be escaped");
        }
        let decoded = percent_encoding::percent_decode_str(encoded).decode_utf8().unwrap();
        assert_eq!(decoded, qr_svg(&qr_code).into_string());
    }

    #[test]
    fn saved_svg_declares_its_namespace_so_it_works_as_a_standalone_file() {
        assert!(qr_svg(&code("HELLO")).into_string().contains(r#"xmlns="http://www.w3.org/2000/svg""#));
    }

    #[tokio::test]
    async fn generate_shows_a_message_for_text_the_mode_cannot_encode() {
        let body = body_text(request_json("abc", "Numeric", "Auto")).await;
        assert!(body.contains(r#"<p id="qr-code">"#));
    }

    #[tokio::test]
    async fn generate_gzips_the_event_stream_when_accepted() {
        let response = post(request_json("HELLO", "Auto", "10"), true).await;
        assert_eq!(response.headers()[header::CONTENT_TYPE], "text/event-stream");
        assert_eq!(response.headers()[header::CONTENT_ENCODING], "gzip");
        assert_eq!(body_bytes(response).await[..2], [0x1f, 0x8b], "gzip magic number");
    }

    #[tokio::test]
    async fn generate_is_uncompressed_when_gzip_is_not_accepted() {
        let response = post(request_json("HELLO", "Auto", "10"), false).await;
        assert!(!response.headers().contains_key(header::CONTENT_ENCODING));
    }

    /// The `_qr_stats` object in a response body.
    fn returned_stats(body: &str) -> serde_json::Value {
        let line = body.lines().find(|line| line.starts_with("data: signals ")).unwrap();
        let signals: serde_json::Value = serde_json::from_str(line.trim_start_matches("data: signals ")).unwrap();
        signals["_qr_stats"].clone()
    }

    #[tokio::test]
    async fn generate_sends_the_stats_of_the_code() {
        let stats = returned_stats(&body_text(request_json("HELLO", "Auto", "1")).await);
        assert_eq!(stats, json!({
            "version": 1,
            "error_correction_level": "L",
            "mask": 2,
            "encoding_modes": ["Alphanumeric"],
            "segments": [{ "mode": "Alphanumeric", "start_bit": 0, "end_bit": 41, "header_bits": 13 }],
        }));
    }

    #[tokio::test]
    async fn generate_clears_the_stats_on_error() {
        let stats = returned_stats(&body_text(request_json("abc", "Numeric", "Auto")).await);
        assert_eq!(stats, serde_json::from_str::<serde_json::Value>(NO_STATS).unwrap());
    }

    #[test]
    fn no_stats_declares_the_same_fields_as_real_stats() {
        let real = create_qr_code(serde_json::from_str(&request_json("HELLO", "Auto", "1")).unwrap()).unwrap().stats;
        let real = serde_json::to_value(real).unwrap();
        let empty: serde_json::Value = serde_json::from_str(NO_STATS).unwrap();
        let keys = |value: &serde_json::Value| value.as_object().unwrap().keys().cloned().collect::<Vec<_>>();
        assert_eq!(keys(&real), keys(&empty));
    }

    #[test]
    fn stats_are_declared_on_the_page_and_never_posted_back() {
        let form = sidebar().into_string();
        assert!(form.contains("_qr_stats:"), "the underscore keeps it out of requests");
        assert!(stats().into_string().contains(r#"data-show="$_qr_stats.version""#));
    }

    #[test]
    fn multiline_toggle_swaps_two_boxes_bound_to_the_same_text() {
        let form = sidebar().into_string();
        assert!(form.contains("_multiline: false"));
        assert!(form.contains(r#"<input type="text" data-bind="text" data-show="!$_multiline">"#));
        assert!(form.contains(r#"data-bind="text" style="display: none" data-show="$_multiline"></textarea>"#));
        assert!(form.contains(r#"<input type="checkbox" data-bind="_multiline""#));
        assert!(form.contains(r#"$text.replace(/\n/g, ' ')"#));
    }

    #[tokio::test]
    async fn generate_accepts_newlines_in_the_text() {
        let body = body_text(request_json(r"line one\nline two", "Auto", "Auto")).await;
        assert!(body.contains(r#"<img id="qr-code""#));
        assert_eq!(returned_stats(&body)["encoding_modes"], json!(["Byte"]));
    }

    #[test]
    fn every_field_has_a_help_tip_that_does_not_activate_it() {
        let form = sidebar().into_string();
        let fields = form.matches(r#"<label class="field""#).count();
        assert_eq!(fields, 5);
        assert_eq!(form.matches(r#"class="help""#).count(), fields);
        assert_eq!(form.matches(r#"data-on:click__prevent="void 0""#).count(), fields);
    }

    #[test]
    fn form_debounces_input_and_handles_submit() {
        let form = sidebar().into_string();
        assert!(form.contains(r#"data-on:input__debounce.300ms="@post('/qr/generate')""#));
        assert!(form.contains(r#"data-on:submit="@post('/qr/generate')""#));
        let form_tag = form.split("<form").nth(1).unwrap().split('>').next().unwrap();
        assert!(!form_tag.contains("data-on:change"), "input already covers the dropdowns");
    }
}
