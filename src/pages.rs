pub(crate) mod toys;
pub(crate) mod portfolio;
pub(crate) mod todo;

use maud::{html, Markup, DOCTYPE};

pub fn layout(title: &str, content: Markup) -> Markup {
    html! {
        (DOCTYPE)
        html {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1.0";
                title { (title) }
                script type="module" src="https://cdn.jsdelivr.net/gh/starfederation/datastar@v1.0.2/bundles/datastar.js" {}
                style {
                    "body { --ink: #1a1a1a; --paper: #fafaf8; --line: #cfcfc8; font-family: system-ui, sans-serif; max-width: 600px; margin: 2rem auto; padding: 0 1rem; background-color: var(--paper); color: var(--ink); }"
                    "form {display: inline}"
                    "input[type=text] { padding: 0.4rem; font-size: 1rem; }"
                    "button { padding: 0.4rem 0.8rem; font-size: 1rem; cursor: pointer; background: var(--paper); color: var(--ink); border: 1px solid var(--line); transition: background-color 120ms ease, border-color 120ms ease, transform 60ms ease; }"
                    "button:hover { border-color: var(--ink); background: #f0f0ec; }"
                    "button:active { transform: scale(0.97); }"
                    "code { background: #f0f0ec; border: 1px solid var(--line); padding: 0.1rem 0.35rem; font-size: 0.9em; }"
                    ".todo { display: flex; align-items: center; gap: 0.5rem; padding: 0.5rem 0; border-bottom: 1px solid #eee; }"
                    ".todo .title { flex: 1; }"
                    ".completed .title { text-decoration: line-through; color: #999; }"

                    /* --- portfolio --- */
                    "#portfolio { display: flex; flex-direction: column; gap: 1.5rem; }"
                    "#portfolio-header h1 { margin-bottom: 0.25rem; }"
                    "#portfolio-tagline { color: #55554f; margin-top: 0; }"
                    ".demo-card { border: 1px solid var(--line); padding: 1rem 1.25rem; display: flex; flex-direction: column; gap: 0.5rem; }"
                    ".demo-card h2 { margin: 0; text-transform: uppercase; letter-spacing: 0.1em; font-size: 0.95rem; }"
                    ".demo-sub { color: #55554f; font-size: 0.9rem; margin: 0; }"
                    "#clock-value { font-size: 1.75rem; font-weight: 700; font-variant-numeric: tabular-nums; letter-spacing: 0.05em; }"
                    "#backend-badge { font-weight: 700; border-bottom: 2px solid var(--ink); }"

                    /* --- toys / pages --- */
                    "#toys, #ttt { justify-content: center; align-items: center; display: flex; flex-direction: column; gap: 1rem; }"
                    "#toy-header, #game-header { text-transform: uppercase; letter-spacing: 0.15em; font-size: 1.1rem; font-weight: 600; color: var(--ink, #1a1a1a); margin: 0 0 0.25rem 0; }"
                    "#toy-container { display: flex; flex-direction: column; align-items: center; gap: 0.5rem; width: 100%; }"

                    "button.game-selector { display: block; width: 220px; background: var(--paper, #fafaf8); color: var(--ink, #1a1a1a); border: 1px solid var(--line, #cfcfc8); padding: 0.6rem 1rem; margin: 0; text-align: center; font-size: 0.95rem; letter-spacing: 0.02em; transition: background-color 120ms ease, border-color 120ms ease, transform 60ms ease; }"
                    "button.game-selector:hover { border-color: var(--ink, #1a1a1a); background: #f0f0ec; }"
                    "button.game-selector:active { transform: scale(0.97); background: var(--ink, #1a1a1a); color: var(--paper, #fafaf8); }"

                    /* --- tic-tac-toe --- */
                    "#ttt-board { display: grid; grid-template-columns: repeat(3, 1fr); grid-template-rows: repeat(3, 1fr); width: 240px; height: 240px; gap: 2px; background: var(--ink, #1a1a1a); border: 2px solid var(--ink, #1a1a1a); }"
                    ".ttt-cell { height: 100%; width: 100%; display: flex; align-items: center; justify-content: center; background: var(--paper, #fafaf8); border: none; margin: 0; padding: 0; font-size: 1.5rem; line-height: 1; }"
                    "button.ttt-cell { color: #b8b8b0; font-weight: 400; transition: background-color 120ms ease, color 120ms ease, transform 60ms ease; }"
                    "button.ttt-cell:hover { background: #f0f0ec; color: var(--ink, #1a1a1a); }"
                    "button.ttt-cell:active { transform: scale(0.94); background: #e4e4de; }"
                    "div.ttt-cell { font-weight: 700; color: var(--ink, #1a1a1a); animation: pop 140ms ease; }"
                    "@keyframes pop { from { transform: scale(0.6); opacity: 0; } to { transform: scale(1); opacity: 1; } }"

                    "#ttt-result { font-size: 1.15rem; font-weight: 600; letter-spacing: 0.05em; text-transform: uppercase; color: var(--ink, #1a1a1a); animation: rise 160ms ease; }"
                    "@keyframes rise { from { transform: translateY(4px); opacity: 0; } to { transform: translateY(0); opacity: 1; } }"
                }
            }
            body {
                (content)
            }
        }
    }
}


fn home_button() -> Markup {
    html! {
        p { a href="/" { "← Home" } }
    }
}

