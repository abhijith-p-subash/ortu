# FAQ & Troubleshooting

## General

**Is my data sent anywhere?**
No. Ortu is local-first — history is stored only on your device. The sole network
activity is checking GitHub Releases for updates.

**Where is my data stored?**
In a SQLite database under the OS app-data directory for `com.ortu.clipboard`.
See [CONFIGURATION.md](CONFIGURATION.md#storage-locations).

**Does Ortu capture passwords?**
On macOS, not when the password manager marks the copy as confidential —
1Password, Bitwarden, KeePassXC, Strongbox and Enpass do, and Ortu skips those
by default (Settings → Privacy → *Skip content marked confidential*).
Otherwise it captures whatever is copied. If you enable *Auto-mask detected secrets*,
likely secrets are masked and encrypted at rest. You can also click the header
status pill to **pause capture** entirely while handling sensitive data. See
[PRIVACY_AND_SECURITY.md](PRIVACY_AND_SECURITY.md).

## History & retention

**My history disappeared after a reboot — is that a bug?**
No — that's the default *On reboot* retention mode, which clears ungrouped &
unpinned items on each OS reboot. Pin items or add them to a group to keep them,
or change the mode in Settings → History. See
[CONFIGURATION.md](CONFIGURATION.md#history-retention).

**Does restarting the app clear history?**
No. Only an actual OS reboot triggers the *On reboot* cleanup; relaunching the
app does not.

**How do I keep everything permanently?**
Set Settings → History → *Keep history for* to **Forever**.

## Shortcuts

**Can I change the hotkeys?**
Yes — the three global hotkeys (open popup, copy-to-stack, paste-next) are
rebindable in Settings → Global Shortcuts, with a *Restore defaults* button.
In-app navigation keys are currently fixed. See [SHORTCUTS.md](SHORTCUTS.md).

**A shortcut won't bind / does nothing.**
Another app or the OS may already own that combination. Ortu rejects combos it
can't register; pick a different one.

## macOS

**"Ortu" is damaged and can't be opened.**
This happens with unsigned builds. Clear the quarantine attribute (see the
README's Gatekeeper section).

**Paste into other apps doesn't work.**
Grant Accessibility permission: System Settings → Privacy & Security →
Accessibility → enable Ortu. The app links you there when needed.

## Windows

**The app won't start / shows a blank window.**
Ensure the WebView2 Runtime is installed (most Windows 10/11 systems have it).

## Linux

**Build fails with missing libraries.**
Install the WebKitGTK/GTK dev packages listed in
[DEVELOPMENT.md](DEVELOPMENT.md#prerequisites).

## Updates

**How do updates work?**
Ortu checks GitHub Releases and shows a notice when a newer version exists — in
the main window and in the quick popup. Clicking **Download update** opens the
installer for your OS and architecture; you install it the same way you did the
first time. Ortu does not replace itself in place. See
[BUILD_AND_RELEASE.md](BUILD_AND_RELEASE.md#update-flow).

**Why doesn't Ortu install updates by itself?**
In-app installation needs signed builds to be reliable, and Ortu is not yet code
signed on macOS or Windows. Until that is in place, the download-and-install
path is the one that actually works. See
[CODE_SIGNING_POLICY.md](CODE_SIGNING_POLICY.md).

## Still stuck?

Search [existing issues](https://github.com/abhijith-p-subash/ortu/issues) or
open a new one with your OS, app version, and steps to reproduce.
