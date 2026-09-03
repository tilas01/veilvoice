# Arch packaging

Two package definitions, both of which compile VeilVoice on the machine that
installs it. No binary is downloaded by either.

| File | Package | Builds from |
| --- | --- | --- |
| `PKGBUILD` | `veilvoice` | the tagged release tarball |
| `PKGBUILD-git` | `veilvoice-git` | the `main` branch |

## Building one here

```sh
cd packaging/aur
makepkg -si            # the release package
makepkg -si -p PKGBUILD-git   # the live one
```

`makepkg` runs the workspace test suite as part of the build, which is
entirely offline and needs no audio device. A build that fails its tests does
not produce a package.

## Keeping `.SRCINFO` in step

The AUR reads `.SRCINFO`, not the `PKGBUILD`, so the two have to agree or the
web interface shows something the package does not do:

```sh
updpkgsums                       # fill in the release tarball's hash
makepkg --printsrcinfo > .SRCINFO
```

Both are checked by `tools/verify.py`, which fails if the version, the
dependency list or the installed binaries drift apart from the rest of the
tree.

## What gets installed

`/usr/bin/veilvoice` and `/usr/bin/veilvoice-gui`, a desktop entry, an icon,
the manual pages generated from each program's own `--help`, and the contents
of `docs/`.

There is no `veilvoice-verify` binary. It was one until 0.1.18 and is now part
of `veilvoice`: run `veilvoice verify` to check a release, or use the Verify
tab in the desktop application.
