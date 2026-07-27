use anyhow::Result;
use objc2_foundation::NSBundle;

/// Apple Developer Team ID used for code signing and validation.
///
/// `script/macos/bundle` exports `ZAP_APPLE_TEAM_ID` with the team of the
/// certificate the bundle is actually signed with, so a fork signing with its
/// own Developer ID does not have to patch this constant. Without that variable
/// the value falls back to the upstream Warp team, matching the historical
/// behaviour. Cargo tracks `option_env!` inputs, so changing the variable
/// triggers a rebuild.
///
/// This has to stay in sync with the signing identity: `verify_code_signature`
/// in `app/src/autoupdate/mac.rs` requires the downloaded bundle's leaf
/// certificate to carry this team in `subject.OU`.
pub const APPLE_TEAM_ID: &str = match option_env!("ZAP_APPLE_TEAM_ID") {
    Some(team_id) => team_id,
    None => "2BBY89MBSN",
};

/// Get the path to the macOS `.app` bundle.
pub fn get_bundle_path() -> Result<String> {
    let bundle = NSBundle::mainBundle();
    let path = bundle.bundlePath();
    Ok(path.to_string())
}
