# Building and signing Zap for macOS

This guide covers producing a distributable `Zap.app` / `Zap.dmg` on a Mac,
including signing with an organization's own Apple Developer ID certificate.

CI (`.github/workflows/zap_release.yml`) builds the published DMGs with an
ad-hoc signature, which is why they trip Gatekeeper's "unidentified developer"
dialog. Everything below is about producing a build that does not.

---

## 1. Set up the machine

```sh
script/macos/bootstrap --build-only
```

`--build-only` installs what a release build needs (Xcode toolchain, Rust,
`cargo-bundle`, `cargo-about`, `create-dmg`) and skips the test harness plus the
PowerShell/Docker/gcloud tooling that only matters for cross-platform
development. Drop the flag on a full development machine.

The first `release-lto` build is slow — CI allows up to six hours for it — and
cargo's cache is per-target-triple, so building both architectures roughly
doubles the wall clock.

---

## 2. Pick a signing mode

| Mode | Flag | Result |
| --- | --- | --- |
| Ad-hoc / development | `--selfsign` | Runs on the machine that built it. Gatekeeper blocks it elsewhere. |
| Developer ID | `--localsign` | Distributable within the org; still blocked once the file carries a download quarantine flag. |
| Developer ID + notarized | `--localsign --notarize` | Downloads and opens cleanly anywhere. |

### What certificate is required

Only a **Developer ID Application** certificate works for distribution outside
the App Store. Check what the machine has:

```sh
security find-identity -v -p codesigning
```

- `Developer ID Application: <Org> (TEAMID)` → use `--localsign`.
- `Apple Development: …` → only good for `--selfsign`; it cannot be notarized.
- `Apple Distribution: …` → App Store only, not usable here.

`--localsign` picks the single Developer ID Application certificate in the
keychain and derives the Team ID from its name. Narrow the choice when several
are installed:

```sh
APPLE_TEAM_ID=ABCDE12345 script/bundle --channel oss --arch aarch64 --localsign
```

or name the identity outright, by common name or SHA-1 fingerprint:

```sh
script/bundle --channel oss --arch aarch64 --localsign "Developer ID Application: Acme Inc (ABCDE12345)"
```

The resolved Team ID is passed to the build through `ZAP_APPLE_TEAM_ID` and read
back by `warp_core::macos::APPLE_TEAM_ID`, which autoupdate uses to validate the
signature of anything it downloads. Signing identity and binary therefore stay
in agreement without patching any source file.

Identity resolution runs before the build, so a missing or ambiguous certificate
fails in seconds rather than after an hour of compiling.

---

## 3. Build

Apple Silicon:

```sh
script/bundle --channel oss --arch aarch64 --localsign
```

Intel (cross-compiles fine from an Apple Silicon Mac):

```sh
script/bundle --channel oss --arch x86_64 --dmg-name-suffix intel --localsign
```

Artifacts land in `target/release-lto/bundle/osx/`:

- `Zap.app` — signed application bundle
- `Zap.dmg` — signed disk image (suffixed when `--dmg-name-suffix` is passed)

Useful extras: `-o` opens the output directory when the build finishes,
`--debug` swaps the LTO profile for a debug build when iterating on the bundling
itself, and `--check-only` compiles without producing a bundle.

Verify a finished bundle:

```sh
codesign --verify --deep --strict --verbose=2 target/release-lto/bundle/osx/Zap.app
```

---

## 4. Notarize (needed for distribution)

macOS refuses to open a downloaded application that Apple has not notarized,
even when it carries a valid Developer ID signature. A build copied over SSH or
AirDrop has no quarantine flag and runs without this step; anything fetched over
HTTP does not.

Store the credentials once per machine, so no password has to be passed through
the environment:

```sh
xcrun notarytool store-credentials zap-notary --apple-id <apple-id> --team-id <TEAMID> --password <app-specific-password>
```

The password is an [app-specific password](https://support.apple.com/en-us/HT204397),
not the Apple ID's own password. Then:

```sh
ZAP_NOTARY_KEYCHAIN_PROFILE=zap-notary script/bundle --channel oss --arch aarch64 --localsign --notarize
```

The bundle is signed, packed into a DMG, the DMG is signed and submitted, and
the resulting ticket is stapled to it. Submission usually takes a few minutes;
`notarytool --wait` blocks until Apple returns a verdict and fails the build if
the verdict is not "Accepted".

`WARP_NOTARIZATION_APPLE_ID` and `WARP_NOTARIZATION_PASSWORD` are honoured as a
fallback when no keychain profile is set, which is how CI supplies credentials.

---

## 5. Notes and gotchas

**Entitlements.** Signed builds use `script/Entitlements.plist` with the
hardened runtime. Unlike `script/Debug-Entitlements.plist` it does not grant
`get-task-allow`, which notarization rejects, nor
`disable-library-validation` — every nested binary in the bundle is signed with
the same identity instead. If a signed build ever fails to launch with a library
validation error in Console, that key is the escape hatch, but prefer signing
the offending binary.

**Nested code.** Plug-ins, frameworks and helpers are signed bottom-up before
the enclosing `.app`, rather than with `codesign --deep`. Apple deprecated
`--deep` because it applies the main application's entitlements to every nested
binary.

**Rosetta.** An Intel DMG built on Apple Silicon cannot be smoke-tested locally
without Rosetta 2 (`softwareupdate --install-rosetta`).

**Data directories.** These follow the release channel, not the bundle
identifier, so a locally built `oss` bundle shares state with an installed Zap
release of the same channel. Test against a throwaway profile if that matters.

**Notifications.** macOS only registers an application with the notification
centre when it has a stable code signature (a `cdhash`). Any of the signing
modes above satisfies that; a completely unsigned build does not.
