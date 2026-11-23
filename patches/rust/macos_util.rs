// MacosUtil.disableCloseWindowMenuItem: greys out "Close" in the macOS Window menu.
//
// The original reaches the menu through the ca.weblite Objective-C bridge (Client, Proxy), which is a third-party
// library this build does not link. Calling it would throw NoSuchMethodError on every macOS start-up. This patch
// skips the menu change and logs it once, so the Close item stays enabled. Nothing else is affected: on other
// platforms the original returns at once (IS_MACOS is false), and this patch does the same.
//
// Known gap: the Close item is not greyed out on macOS until the bridge is ported to the runtime's objc helpers
// (monolith/rt/desk/cocoa.rs).

pub fn disable_close_window_menu_item(_orig: impl Fn() -> rt::Res<()>) -> rt::Res<()> {
    if !cfg!(target_os = "macos") {
        return Ok(());
    }
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        eprintln!("monolith: macOS Window menu: 'Close' is not disabled (ca.weblite bridge not ported)");
    });
    Ok(())
}
