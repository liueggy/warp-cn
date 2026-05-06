# Hide Warp from the Dock and app switcher on macOS — Technical Spec
## Problem
Warp currently runs as a regular macOS application, so it appears in the Dock and Command-Tab app switcher even when a user primarily uses the dedicated hotkey window. Supporting a background hotkey workflow requires a macOS-only setting that switches Warp between regular and accessory app presentation, plus a status-bar fallback so users can recover access when the Dock icon is hidden.
## Relevant code
- `app/src/terminal/general_settings.rs:8` — `GeneralSettings` defines app-wide user settings such as start at login, restore session, and quit-on-last-window-closed.
- `app/src/settings_view/features_page.rs (5285-5483)` — Settings > Features renders the global hotkey controls.
- `app/src/settings_view/features_page.rs (4497-4586)` — Settings > Features renders start-at-login and quit-on-last-window-closed app behavior controls.
- `app/src/lib.rs (929-950)` — macOS app builder setup wires activate-on-launch, dev icon, menu bar, and Dock menu.
- `app/src/lib.rs (2355-2364)` — startup subscribes to login-item setting changes after settings are initialized.
- `app/src/root_view.rs (279-314)` — root global actions include open-new and hotkey-window actions.
- `app/src/root_view.rs (614-811)` — restored hotkey windows are recreated hidden; today a normal window is opened when only a hotkey window restores.
- `app/src/root_view.rs (1275-1449)` — hotkey-window open/hide state machine.
- `app/src/app_menus.rs (70-96)` — app menu bar and Dock menu are built from Warp menu abstractions.
- `crates/warpui/src/platform/mac/app.rs (94-126)` — macOS `AppExt` currently supports activate-on-launch, app icon, menu bar, and Dock menu builders.
- `crates/warpui/src/platform/mac/app.rs (264-305)` — macOS launch creates the main menu and Dock menu during app launch.
- `crates/warpui/src/platform/mac/objc/app.m (298-307)` — Dock/Finder reopen currently creates a new normal window when no visible windows exist.
- `crates/warpui/src/platform/mac/window.rs (103-130)` and `crates/warpui/src/platform/mac/objc/window.m (904-933)` — macOS app/window activation and focus helpers.
- `crates/warpui_core/src/platform/mod.rs (193-265)` — platform delegate trait where app-level platform hooks can be added.
- `crates/warpui_core/src/core/app.rs (2102-2130)` and `(3978-4032)` — `AppContext` delegates OS-level operations to the platform delegate.
## Current state
Warp has two global-hotkey modes: dedicated hotkey window and show/hide all windows. `KeysSettings` stores the dedicated hotkey window settings under `global_hotkey.dedicated_window.*`, and `RootView` registers global shortcuts after app restoration.
On macOS, the dedicated hotkey window uses `WindowStyle::Pin` and is tracked as a quake-mode window. On restore, Warp recreates that window hidden. If the restored app state contains no normal windows, `open_from_restored` creates a normal window so startup never leaves the user with no visible Warp UI. That fallback conflicts with a background hotkey workflow once Warp has a status-bar access point.
The macOS platform layer already owns AppKit integration for app launch, menus, global hotkeys, activation, and window ordering. It does not currently expose a status-bar item or runtime app activation-policy API. The codebase has no existing `NSStatusItem` abstraction.
## Proposed changes
### 1. Add a macOS Dock visibility setting
Add a public macOS-only setting to `GeneralSettings`, for example:
- Rust field: `show_dock_icon`
- Generated setting type: `ShowDockIcon`
- Default: `true`
- Supported platforms: `SupportedPlatforms::MAC`
- Suggested TOML path: `general.show_dock_icon`
- Description: “Whether Warp appears in the macOS Dock and app switcher.”
Prefer `SyncToCloud::Globally(RespectUserSyncSetting::Yes)` to match other user-facing appearance/app behavior preferences unless maintainers decide Dock visibility should be device-local. The setting must be ignored on non-macOS platforms.
### 2. Add settings UI
Add a macOS-only `SettingsWidget` in `app/src/settings_view/features_page.rs` near the existing global hotkey and start-at-login controls.
The row should:
- Label the toggle “Show Warp in Dock and app switcher”.
- Use `ShowDockIcon::storage_key()` and `ShowDockIcon::sync_to_cloud()` for the local-only/sync indicator pattern.
- Dispatch a new `FeaturesPageAction` that toggles `GeneralSettings::show_dock_icon`.
- Include search terms for Dock, Command-Tab, app switcher, menu bar, status bar, background, and hotkey.
### 3. Introduce a platform presentation API
Add a small app-level platform API rather than hard-coding AppKit calls in app code. A possible shape:
- In `crates/warpui_core/src/platform/mod.rs`, add an enum such as `AppPresentationMode` with `Regular` and `Accessory`.
- Extend `platform::Delegate` with a default no-op method such as `set_app_presentation_mode(&self, mode: AppPresentationMode, status_menu: Option<Menu>)`.
- Add an `AppContext` wrapper method in `crates/warpui_core/src/core/app.rs` so app code can apply presentation without reaching into the delegate directly.
Only the macOS delegate needs a real implementation. Linux, Windows, tests, and headless delegates should no-op.
### 4. Implement macOS activation policy and status item
In the macOS platform layer:
- Use `NSApplicationActivationPolicyRegular` when `show_dock_icon == true`.
- Use `NSApplicationActivationPolicyAccessory` when `show_dock_icon == false` so Warp is removed from the Dock and Command-Tab switcher.
- Create an `NSStatusItem` when entering accessory mode.
- Remove the `NSStatusItem` when returning to regular mode.
- Keep all AppKit calls on the main thread.
Implementation locations:
- Add Objective-C helpers in `crates/warpui/src/platform/mac/objc/app.m` to set activation policy and own a singleton status item.
- Add Rust FFI wrappers in `crates/warpui/src/platform/mac/delegate.rs`.
- Reuse or lightly extend `crates/warpui/src/platform/mac/menus.rs` so the status item can use existing `Menu` / `CustomMenuItem` callbacks rather than a parallel callback system.
The status item menu should be rebuilt when relevant state changes so labels and enabled states remain correct. For v1, rebuilding when `show_dock_icon`, global-hotkey mode, or settings page actions change is sufficient.
### 5. Build the status-bar menu in app code
Add an `app_menus::status_item_menu(ctx: &mut AppContext) -> Menu` builder. Suggested items:
- Show Dedicated Hotkey Window, when `KeysSettings::global_hotkey_mode(ctx)` is `GlobalHotkeyMode::QuakeMode`.
- New Window, dispatching `root_view:open_new` and `workspace:save_app`.
- Settings, dispatching the existing settings action.
- Show Warp in Dock and app switcher, setting `GeneralSettings::show_dock_icon` to `true`.
- Quit Warp, using `ctx.terminate_app(TerminationMode::Cancellable, None)` or the existing standard quit path if one is available through menu abstractions.
For the hotkey item, retrieve `GlobalResourceHandlesProvider` from `AppContext` and dispatch `root_view:toggle_quake_mode_window` with those handles, matching the action signature.
### 6. Apply presentation during startup and on setting changes
After settings are initialized in `app/src/lib.rs`, apply the initial Dock visibility:
- If `show_dock_icon` is true, set regular presentation and ensure no status item remains.
- If false, set accessory presentation and install the status menu.
Subscribe to `GeneralSettings` changes in the same area that currently subscribes to login-item changes. When `show_dock_icon` changes, re-apply presentation.
Also re-apply or rebuild the status menu when global-hotkey mode changes, because the dedicated-hotkey item depends on `KeysSettings::global_hotkey_mode(ctx)`.
### 7. Adjust restored hotkey-window startup fallback
Update `root_view::open_from_restored` so the “only a hidden hotkey window restored” fallback respects the new setting.
Current behavior:
- Restore hidden hotkey window.
- If `normal_window_count == 0`, always open a normal window.
Desired behavior:
- If `normal_window_count == 0` and Dock visibility is on, keep existing behavior.
- If `normal_window_count == 0`, Dock visibility is off, and a hotkey window was restored, do not create an extra normal window.
- If there are no restored windows at all, keep existing `launch` fallback that opens a new window unless product review explicitly decides that Dock-hidden startup should be fully windowless.
Track whether a hotkey window was restored with a local boolean in `open_from_restored`. This keeps the behavior narrow and avoids changing first-launch or empty-state startup semantics.
### 8. Optional feature flag / PR split
The change touches macOS AppKit behavior and startup behavior, so it is reasonable to put the implementation behind a dogfood feature flag such as `FeatureFlag::ConfigurableDockIcon` if reviewers want incremental rollout. A practical split is:
1. Add setting, status item infrastructure, and activation-policy switching behind the flag.
2. Add startup fallback refinement and broader validation.
3. Promote or remove the flag after macOS validation.
If maintainers prefer one PR, keep the scope contained by limiting behavior changes to macOS and preserving defaults.
## End-to-end flow
### User hides Warp from the Dock
1. User opens Settings and turns off “Show Warp in Dock and app switcher”.
2. `FeaturesPageAction` updates `GeneralSettings::show_dock_icon`.
3. The settings subscription in `app/src/lib.rs` calls the presentation helper.
4. macOS delegate sets activation policy to accessory.
5. macOS delegate creates a status item using `app_menus::status_item_menu`.
6. Warp disappears from the Dock and Command-Tab switcher.
### User restores Warp to the Dock
1. User opens the Warp status-bar menu.
2. User selects “Show Warp in Dock and app switcher”.
3. The menu callback sets `GeneralSettings::show_dock_icon` to true.
4. The settings subscription sets activation policy back to regular.
5. The status item is removed.
6. Warp appears in the Dock and Command-Tab switcher again.
### User starts Warp with only the dedicated hotkey window restored
1. Startup restores the dedicated hotkey window hidden, as today.
2. `open_from_restored` sees no normal windows and sees a restored hotkey window.
3. If Dock visibility is off, it does not create a normal window.
4. The status item and global hotkey remain available to open Warp.
## Risks and mitigations
- AppKit activation-policy edge cases: `setActivationPolicy` can fail or behave differently across macOS versions. Log failures, keep regular presentation if applying accessory mode fails, and ensure status item creation happens before or together with hiding the Dock icon.
- User loses access: Always show the status item when Dock visibility is off. Include a menu item to restore Dock visibility.
- Menu callback lifetime: Reuse existing `Menu` and `CustomMenuItem` callback plumbing so callbacks continue to execute through `AppCallbackDispatcher`.
- Startup regressions: Keep the startup fallback narrow; only suppress the extra normal window when a hidden hotkey window was actually restored and Dock visibility is off.
- Quit semantics: Route status-menu Quit through existing termination flow so running-session warnings and update/relaunch behavior are preserved.
- Tests in non-macOS CI: Keep non-macOS platform implementations as no-ops and gate macOS-only UI with `cfg(target_os = "macos")` or supported-platform metadata.
## Testing and validation
- Add unit coverage for the setting default and supported platform if the settings macro test utilities are available.
- Add or update settings-view tests for the macOS-only row if the settings page has platform-specific test coverage.
- Add Rust tests around the restored-state helper if `open_from_restored` logic can be extracted into a pure decision function.
- Manual macOS validation:
  - Toggle off and verify Dock plus Command-Tab removal.
  - Confirm the status item appears and each menu item works.
  - Toggle back on from the status item and verify Dock plus Command-Tab restoration.
  - Enable dedicated hotkey mode and verify global hotkey toggling still works in accessory mode.
  - Relaunch with only a restored hidden hotkey window and Dock visibility off; verify no extra normal window opens.
  - Verify quit warnings still appear for running sessions when quitting from the status menu.
- Regression validation:
  - Build non-macOS targets to ensure the new delegate method and setting UI compile as no-ops.
  - Verify users with the default setting see no Dock, app switcher, startup, or hotkey behavior changes.
## Follow-ups
- Add a dedicated template status-bar icon if design provides one.
- Consider a separate “show status-bar icon even when Dock icon is visible” preference if requested.
- Consider making Dock-click behavior open the dedicated hotkey window while the Dock icon is visible, but keep that separate from this Dock-hiding feature.
