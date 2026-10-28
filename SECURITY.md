# Security policy

Keyloom is an offline desktop app. It makes no network connections, listens on no network port, and keeps imported songs and its library on your computer. Its main inputs are MIDI files and MIDI devices.

## Supported versions

Security fixes target the latest published release, including the 0.4.2 beta once it is published. Older builds are not maintained. Check the [release list](https://github.com/mythicruntime-lab/keyloom/releases), which includes beta prereleases as well as stable releases.

## Reporting a vulnerability

Please don't publish vulnerability details in an issue. When private vulnerability reporting is enabled, use the repository's **Security** tab and choose **Report a vulnerability** to send a private report to the maintainer.

If that button is unavailable, open an issue asking the maintainer to enable private vulnerability reporting. Include no exploit details, triggering files or private data in that public request; wait for a private reporting route before sharing them.

Include:

- the Keyloom version, and whether you used the installer or the portable ZIP;
- your Windows version;
- steps to reproduce, and what you expected to happen;
- if a file triggers it, a MIDI file you made yourself rather than a copyrighted song.

Keyloom has a single maintainer, so a reply may take a while.

## What to report here

- A MIDI file that can run code or corrupt memory when it's imported, played, edited or exported.
- Imported content that can read, change or delete files outside Keyloom's data folder, or change an original MIDI file.
- Anything that makes Keyloom connect to the network, load remote content, or run scripts that didn't ship with it.

These aren't vulnerabilities:

- Windows SmartScreen warnings. Release builds aren't code-signed; check a download against its `SHA256SUMS.txt` instead.
- Problems that need someone who can already run programs as you or edit Keyloom's data folder.
- An ordinary crash, freeze or wrong result. Use the **Bug report** issue form for those.
