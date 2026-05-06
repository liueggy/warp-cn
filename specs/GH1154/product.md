# Hide Warp from the Dock and app switcher on macOS
GitHub: #1154
Related: #5309
Figma: none provided
## Summary
Warp should offer a macOS setting that lets users hide Warp from the Dock and Command-Tab app switcher while keeping Warp available in the background. When Warp is hidden from the Dock, Warp must automatically expose a minimal status-bar icon so users can bring Warp back, open windows, access settings, and quit without relying only on a global hotkey.
The setting is not limited to users with the dedicated hotkey window enabled. It is a general macOS app-presence preference that is especially useful for users who primarily access Warp through the dedicated hotkey window.
## Problem
Users who use Warp as a background hotkey terminal do not need a persistent Dock icon or Command-Tab entry. The current behavior adds visual clutter and makes it easy to accidentally click the Dock icon, which opens a separate normal Warp window instead of showing the dedicated hotkey window. Users also want Warp to be able to run at login in the background so the global hotkey works immediately after startup.
## Goals
- Let macOS users hide Warp from the Dock.
- Hide Warp from the macOS Command-Tab app switcher whenever it is hidden from the Dock.
- Keep Warp discoverable and controllable through a status-bar icon whenever the Dock icon is hidden.
- Keep the setting usable even when the dedicated hotkey window is disabled.
- Preserve existing default behavior for users who do not change the setting.
- Avoid trapping users in a state where Warp is running but has no visible affordance to reopen or configure it.
## Non-goals
- Changing global hotkey keybinding support or allowed key combinations.
- Replacing normal Warp windows with the dedicated hotkey window when the Dock icon is clicked while the Dock icon remains visible.
- Adding a configurable status-bar-only mode independent from Dock visibility.
- Adding Windows or Linux support in this change.
- Changing Dock icon artwork, app icon selection, or channel-specific branding.
## User experience
### Settings
1. A macOS-only setting named “Show Warp in Dock and app switcher” appears in Settings, near other app behavior controls such as global hotkey and start-at-login.
2. The setting defaults to on.
3. When the setting is on, Warp behaves as it does today: it appears in the Dock and Command-Tab switcher, and no status-bar icon is shown solely for this feature.
4. When the setting is off, Warp disappears from the Dock and Command-Tab switcher.
5. The setting should be searchable with terms such as “dock”, “app switcher”, “cmd tab”, “command tab”, “menu bar”, “status bar”, “hotkey”, and “background”.
### Status-bar fallback
6. Whenever Warp is hidden from the Dock, a minimal Warp status-bar icon appears in the macOS menu bar.
7. The status-bar icon provides a menu with at least:
   - Show Dedicated Hotkey Window, when the dedicated hotkey window is enabled.
   - New Window.
   - Settings.
   - Show Warp in Dock and app switcher.
   - Quit Warp.
8. If the dedicated hotkey window is disabled, the status-bar menu omits or disables the dedicated-hotkey item rather than showing an action that cannot work.
9. Selecting “Show Warp in Dock and app switcher” restores the Dock and Command-Tab entry and removes the status-bar icon once the app is visible in the Dock again.
10. Selecting New Window opens and focuses a normal Warp window.
11. Selecting Settings opens and focuses Warp settings.
12. Selecting Quit Warp follows Warp’s existing quit behavior, including warning dialogs for running processes when applicable.
### Hotkey window behavior
13. Existing dedicated hotkey window behavior remains unchanged: the global hotkey opens, focuses, hides, and restores the hotkey window according to current hotkey settings.
14. Hiding the Dock icon does not require the dedicated hotkey window to be enabled.
15. If the dedicated hotkey window is enabled and Warp starts with only a previously restored hotkey window, Warp should not create an extra normal window solely because the restored hotkey window starts hidden. The status-bar icon and global hotkey provide access.
16. If Warp is hidden from the Dock and no hotkey is configured, the user can still open a normal window from the status-bar menu.
### Startup and existing windows
17. Toggling the setting off does not close existing Warp windows.
18. Toggling the setting on does not create a new terminal window by itself.
19. Start-at-login behavior remains controlled by the existing login-item setting.
20. When Warp launches at login with Dock visibility off, Warp may run without a visible terminal window as long as the status-bar icon is visible.
21. Opening files, URLs, or explicit commands that normally create or focus Warp windows should continue to do so even when the Dock icon is hidden.
### Platform behavior
22. The setting is only shown on macOS.
23. On non-macOS platforms, Warp behavior is unchanged.
24. If macOS cannot apply the Dock visibility change at runtime, Warp should fail safely by keeping the Dock icon visible and avoid leaving the user without both Dock and status-bar access.
## Success criteria
- A fresh macOS install shows Warp in the Dock and Command-Tab switcher by default.
- Turning off “Show Warp in Dock and app switcher” removes Warp from both the Dock and Command-Tab switcher.
- Turning off the setting shows a status-bar icon.
- The status-bar menu can open a new Warp window.
- The status-bar menu can open Settings.
- The status-bar menu can show the dedicated hotkey window when dedicated hotkey mode is enabled.
- The status-bar menu can restore Warp to the Dock and Command-Tab switcher.
- The status-bar menu can quit Warp through existing quit flows.
- The dedicated hotkey still toggles the hotkey window while Warp is hidden from the Dock.
- When only a hidden/restored hotkey window exists at startup and Dock visibility is off, Warp does not create an extra normal window solely to make something visible.
- Users who leave the setting on see no change in Dock, Command-Tab, startup, or hotkey behavior.
## Validation
- Manual macOS: toggle the setting off, verify Warp disappears from Dock and Command-Tab and the status-bar icon appears.
- Manual macOS: use the status-bar menu to open a normal window, open Settings, and quit Warp.
- Manual macOS: enable the dedicated hotkey window, hide the Dock icon, and verify the global hotkey opens and hides the dedicated window.
- Manual macOS: with only the dedicated hotkey window restored and Dock visibility off, relaunch Warp and verify it does not open an extra normal terminal window.
- Manual macOS: toggle the setting back on from the status-bar menu and verify the Dock and Command-Tab entry return and the status-bar icon disappears.
- Manual macOS: verify opening a Warp URL or file still opens/focuses the appropriate Warp window while Dock visibility is off.
- Regression: verify non-macOS builds do not show the setting and existing global hotkey behavior remains unchanged.
## Open questions
- The exact status-bar icon artwork is not specified by design. The implementation should use the existing Warp app/icon asset unless design provides a dedicated template icon before implementation.
