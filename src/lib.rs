// A thin library target that exists only so external harnesses (the criterion benches in
// benches/) can link against the crate. `pages`, `game_ttt`, `sessions`, and `AppState` stay
// declared directly in main.rs, since they're coupled to each other via `crate::` there.
pub mod services;
