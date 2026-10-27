Luma Mail for Windows x64 (early access)

Extract the entire ZIP into a writable directory and run luma-mail.exe.
Windows 10/11 x64 and the Microsoft Edge WebView2 Evergreen Runtime are required.
The runtime is already present on most current Windows installations. Official installer:
https://developer.microsoft.com/microsoft-edge/webview2/

No MSYS2, Rust or development toolchain is needed on your testing PC.
This preview is not Authenticode-signed; verify the release SHA-256 before running.
The app does not require administrator access or install a background service.
Linux remains the primary target.

Sign in once and choose the optional Gmail settings, Contacts and limited Drive features.
Tokens are stored in Windows Credential Manager. Mail stays online in Gmail.
Settings and the optional bounded cache use the per-user luma-mail directories.
A zero cache budget disables retained mail, attachment, thumbnail and photo caching.
Temporary browser/codec metadata is separate from the mail cache; no browser history,
mail body, attachment, local draft or contact database is intentionally persisted there.
Save/download only writes attachments when you explicitly choose a destination.
Close hides to the native Windows tray when enabled. Right-click the star to Open or Quit.
Notifications use Windows shell notifications and obey Windows notification settings.
The built-in visual signature editor is saved online in Gmail, including changes in Gmail web.

The bundle contains replaceable LGPL/GPL runtime libraries, their licence notices and
THIRD-PARTY.json with exact package versions/source-package locations. Luma source:
https://github.com/SleepyZzed/luma-mail
