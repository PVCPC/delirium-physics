use std::sync::OnceLock;
static LONG:OnceLock<bool>=OnceLock::new();
pub(super) fn enabled()->bool {*LONG.get_or_init(||std::env::var("DLM_LONG_TRANSITIONS").as_deref()==Ok("1"))}
fn pick(resets:usize,legacy_every:usize,long:bool)->f32 {
    if long {if resets%4==0 {28.} else {0.}}
    else if resets%legacy_every==0 {0.} else {28.}
}
pub(super) fn offset(resets:usize,legacy_every:usize)->f32 {pick(resets,legacy_every,enabled())}
