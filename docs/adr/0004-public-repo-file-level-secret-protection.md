# The dotfiles repo is public; secrets are protected at the file level, not the repo level

A private repo requires git credentials (SSH key or token) on every new device before the first clone — but those credentials can't themselves be fetched from an encrypted repo without already being able to clone it, a circular bootstrap problem. mysh's repo is public instead: only `.age`-suffixed files are protected (via the shared Passphrase, ADR-0003); everything else — repo structure, plain config content, machine names, which Packages are installed — is visible to anyone. This also lets the one-line bootstrap install script live inside the repo itself, so there's exactly one address to remember to set up a brand-new device.

**Consequence — treat as a hard boundary:** nothing sensitive may ever be written to a non-`.age` file. Repo structure and plain config content must be assumed to be world-readable at all times.
