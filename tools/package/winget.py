#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Write the winget manifests for a published release.

    python tools/package/winget.py v0.1.23 SHA256SUMS OUT
    python tools/package/winget.py --self-test

Reads the hashes of the two MSIs from the release's SHA256SUMS, so nothing is
downloaded and the hash winget checks is the one the project signed. Writes
the three files winget-pkgs expects, in the folder layout it expects:

    OUT/manifests/t/tilas01/VeilVoice/0.1.23/
        tilas01.VeilVoice.yaml
        tilas01.VeilVoice.installer.yaml
        tilas01.VeilVoice.locale.en-GB.yaml

Check them with `winget validate --manifest <that folder>`, try them with
`winget install --manifest <that folder>`, then propose them to
microsoft/winget-pkgs. After the first version is accepted, the release
workflow can propose each later one by itself; see docs/PACKAGING.md.

Verify SHA256SUMS against its signature before running this. A manifest
built from an unchecked hash list carries whatever that list says.

# In plain words

Writes the small description files that let people install VeilVoice with
`winget install tilas01.VeilVoice`.
"""

import os
import re
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
sys.path.insert(0, HERE)
import msi  # noqa: E402

IDENTIFIER = "tilas01.VeilVoice"
SCHEMA = "1.10.0"
LOCALE = "en-GB"
RELEASES = "https://github.com/tilas01/veilvoice/releases"


def upgrade_codes():
    """Each scope's UpgradeCode, read from the installer source."""
    with open(msi.SOURCE, encoding="utf-8") as handle:
        source = handle.read()
    codes = {}
    for scope in msi.SCOPES:
        block = re.search(r'Scope\) = "%s" \?>(.*?)<\?(?:elseif|else|endif)' % scope,
                          source, re.S)
        found = block and re.search(r'UpgradeCode = "([0-9A-F-]{36})"', block.group(1))
        if not found:
            raise ValueError(f"no UpgradeCode for the {scope} package in {msi.SOURCE}")
        codes[scope] = found.group(1)
    return codes


def hashes(sums_text, version):
    """The SHA-256 of each MSI, from the text of SHA256SUMS."""
    wanted = {msi.msi_name(version, scope): scope for scope in msi.SCOPES}
    found = {}
    for line in sums_text.splitlines():
        parts = line.strip().split(None, 1)
        if len(parts) != 2 or not re.fullmatch(r"[0-9a-fA-F]{64}", parts[0]):
            continue
        name = parts[1].lstrip("*")
        if name in wanted:
            found[wanted[name]] = parts[0].upper()
    missing = [msi.msi_name(version, s) for s in msi.SCOPES if s not in found]
    if missing:
        raise ValueError(f"SHA256SUMS does not list {', '.join(missing)}")
    return found


def manifests(version, sums_text):
    digests = hashes(sums_text, version)
    codes = upgrade_codes()
    tag = f"v{version}"
    header = lambda kind: f"# yaml-language-server: $schema=https://aka.ms/winget-manifest.{kind}.{SCHEMA}.schema.json\n\n"

    version_yaml = header("version") + (
        f"PackageIdentifier: {IDENTIFIER}\n"
        f"PackageVersion: {version}\n"
        f"DefaultLocale: {LOCALE}\n"
        "ManifestType: version\n"
        f"ManifestVersion: {SCHEMA}\n")

    installers = []
    for scope in msi.SCOPES:
        installers.append(
            "- Architecture: x64\n"
            f"  Scope: {scope}\n"
            f"  InstallerUrl: {RELEASES}/download/{tag}/{msi.msi_name(version, scope)}\n"
            f"  InstallerSha256: {digests[scope]}\n"
            f"  ProductCode: '{{{msi.product_code(version, scope)}}}'\n"
            "  AppsAndFeaturesEntries:\n"
            f"  - UpgradeCode: '{{{codes[scope]}}}'\n")
    installer_yaml = header("installer") + (
        f"PackageIdentifier: {IDENTIFIER}\n"
        f"PackageVersion: {version}\n"
        "InstallerType: wix\n"
        "UpgradeBehavior: install\n"
        "Commands:\n- veilvoice\n"
        "Installers:\n" + "".join(installers) +
        "ManifestType: installer\n"
        f"ManifestVersion: {SCHEMA}\n")

    locale_yaml = header("defaultLocale") + (
        f"PackageIdentifier: {IDENTIFIER}\n"
        f"PackageVersion: {version}\n"
        f"PackageLocale: {LOCALE}\n"
        "Publisher: tilas01\n"
        "PublisherUrl: https://github.com/tilas01\n"
        "PublisherSupportUrl: https://github.com/tilas01/veilvoice/issues\n"
        "PackageName: VeilVoice\n"
        "PackageUrl: https://tilas01.github.io/veilvoice/\n"
        "License: GPL-3.0-or-later\n"
        "LicenseUrl: https://github.com/tilas01/veilvoice/blob/main/LICENSE\n"
        "ShortDescription: Removes the biometric voiceprint from a recording and keeps the words, fully offline.\n"
        "Description: |-\n"
        "  VeilVoice changes a recorded voice so that neither software nor a listener can tell\n"
        "  who was speaking, while the words stay clear enough to transcribe. It works entirely\n"
        "  offline: there is no account, no upload and no networking code.\n"
        "  This installs the desktop application and the veilvoice command.\n"
        "  Written and maintained by tilas01, with much of the code drafted using Claude Code,\n"
        "  Anthropic's generative AI coding tool. tilas01 audits the code and signs every release.\n"
        "Moniker: veilvoice\n"
        "Tags:\n- privacy\n- voice\n- audio\n- anonymisation\n- offline\n- open-source\n"
        f"ReleaseNotesUrl: {RELEASES}/tag/{tag}\n"
        "ManifestType: defaultLocale\n"
        f"ManifestVersion: {SCHEMA}\n")

    return {
        f"{IDENTIFIER}.yaml": version_yaml,
        f"{IDENTIFIER}.installer.yaml": installer_yaml,
        f"{IDENTIFIER}.locale.{LOCALE}.yaml": locale_yaml,
    }


def write(version, sums_path, out):
    with open(sums_path, encoding="utf-8") as handle:
        files = manifests(version, handle.read())
    folder = os.path.join(out, "manifests", IDENTIFIER[0].lower(),
                          *IDENTIFIER.split("."), version)
    os.makedirs(folder, exist_ok=True)
    for name, text in files.items():
        with open(os.path.join(folder, name), "w", encoding="utf-8", newline="\n") as handle:
            handle.write(text)
    return folder


def self_test():
    failures = 0

    def check(ok, what):
        nonlocal failures
        print(("ok   " if ok else "FAIL ") + what)
        failures += not ok

    codes = upgrade_codes()
    check(set(codes) == {"user", "machine"} and codes["user"] != codes["machine"],
          "each installer has its own upgrade code, read from the source")
    sums = ("a" * 64 + "  veilvoice-v0.1.23-windows-x86_64-user.msi\n"
            + "b" * 64 + " *veilvoice-v0.1.23-windows-x86_64-machine.msi\n"
            + "c" * 64 + "  veilvoice-v0.1.23-windows-x86_64.zip\n")
    files = manifests("0.1.23", sums)
    installer = files[f"{IDENTIFIER}.installer.yaml"]
    check("A" * 64 in installer and "B" * 64 in installer,
          "both hashes come from SHA256SUMS, upper case as winget writes them")
    check(installer.count("Scope: ") == 2, "one installer per scope")
    check(msi.product_code("0.1.23", "user") in installer,
          "the product code is the one the installer was built with")
    check("/download/v0.1.23/veilvoice-v0.1.23-windows-x86_64-machine.msi" in installer,
          "the download address is the release's")
    check("\t" not in "".join(files.values()), "no tabs, which YAML does not allow")
    check(all(text.endswith("\n") for text in files.values()), "every file ends in a newline")
    try:
        manifests("0.1.24", sums)
        check(False, "a hash list for another version is refused")
    except ValueError:
        check(True, "a hash list for another version is refused")
    with tempfile.TemporaryDirectory() as scratch:
        path = os.path.join(scratch, "SHA256SUMS")
        with open(path, "w") as handle:
            handle.write(sums)
        folder = write("0.1.23", path, scratch)
        check(folder.endswith(os.path.join("manifests", "t", "tilas01", "VeilVoice", "0.1.23")),
              "the folder is laid out as winget-pkgs expects")
        check(len(os.listdir(folder)) == 3, "three manifest files")
    print(f"\n{failures} failure(s)" if failures else "\nall checks pass")
    return 1 if failures else 0


def main(argv):
    if argv[1:] == ["--self-test"]:
        return self_test()
    if len(argv) != 4:
        print("usage: winget.py vX.Y.Z SHA256SUMS OUT, or winget.py --self-test")
        return 2
    try:
        folder = write(argv[1].lstrip("v"), argv[2], argv[3])
    except ValueError as why:
        print(why, file=sys.stderr)
        return 1
    print(f"wrote {folder}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
