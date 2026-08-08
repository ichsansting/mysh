# 03 — Does the merged repo need to be public?

**Type:** grilling

**Status:** resolved

## Question

`ichsansting/mysh` and `ichsansting/dotfiles` are both currently **private** on
GitHub. ADR-0004 ("The dotfiles repo is public; secrets are protected at the file
level, not the repo level") requires the Source repo to be public so a brand-new
device can bootstrap with zero pre-placed credentials. Merging the profile into mysh
means the combined repo inherits that requirement. Is the user OK making
`ichsansting/mysh` public?

## Answer

Yes — make it public. Matches ADR-0004 as designed: tool code and `profile/` both
become world-readable; only `.age` files stay protected. This is the only way the
one-line `curl | sh` bootstrap works without a pre-placed SSH key/token on the fresh
device.

## Comments
