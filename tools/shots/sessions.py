#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Recordings of the programs actually doing something.

    tools/shots/sessions.py --record   # run them and write the transcripts
    tools/shots/sessions.py --check    # verify the transcripts are current

# Why this exists

The website's demonstration replayed ten `--help` screens. Those are real, and
they are the least interesting real thing the programs produce: a help screen
tells a reader what the flags are called and nothing about what happens when
you use one. Somebody deciding whether to trust this needs to see it work.

Everything here is a transcript of a real run. Not a plausible-looking
imitation, not a designer's idea of what the output would be: the bytes the
program wrote, captured from a terminal, with the same passphrases typed at the
same prompts a person would type them at.

# Why a pty rather than a pipe

The first attempt ran each command with its output piped, which is how the help
screens are captured, and three of the four sessions came out as refusals.
`veilvoice keygen` and `veilvoice anonymise` ask for a passphrase and check
whether there is a terminal to ask on; with a pipe there is not, so what got
recorded was the program correctly declining to run.

That refusal is worth showing and it is one of the sessions below. It is not
worth showing four times. So the recorder allocates a pseudo-terminal, which is
what a person has, and types at the prompts.

# What cannot be the same twice

A real run contains figures that are properties of the machine and the moment:
how much faster than realtime it processed the audio, and the millisecond range
the modulation seed rolls at, which is drawn fresh each time on purpose. The
committed transcript keeps the real ones, because it is a transcript. `--check`
normalises those spans on both sides before comparing, so a re-run on a
different machine does not fail for being a different machine, and every other
byte still has to match.

The list of what is normalised is short, explicit, and right here rather than
in a comment somewhere: anything not on it must be identical.

# The one that cannot be re-run offline

The verifier session checks a published release, which means the archive, the
hash list and the signature have to exist and be downloaded. That is a
maintainer step and it needs the network, so `--check` does not re-run it.
Instead it holds the transcript to what the repository can prove about it: the
fingerprint in it must be the fingerprint of the signing key committed here,
the version in it must be the workspace version, and it must contain the
verdict lines the verifier is documented to print. A stale or invented
transcript fails all three.

Pure standard library.
"""

from __future__ import annotations

import argparse
import io
import os
import pty
import re
import select
import shutil
import struct
import subprocess
import sys
import tempfile
import time
import wave
import math

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", ".."))
OUT = os.path.join(ROOT, "assets", "screenshots")

# The width a recorded terminal is. The same figure the help-screen drawings
# use, so the two sit beside each other without one of them wrapping oddly.
COLUMNS = 88

# Spans that are a property of the machine or the moment rather than of the
# program. Normalised on both sides before `--check` compares; the committed
# transcript keeps whatever the real run produced.
VOLATILE = [
    # "125.9x realtime" is how fast this processor happened to be.
    (re.compile(r"\d+(?:\.\d+)?x realtime"), "<speed>x realtime"),
    # "1371-1973 ms" is drawn from the CSPRNG before every roll, on purpose:
    # a fixed period would be a period an observer could measure.
    (re.compile(r"\d+-\d+ ms, drawn fresh"), "<range> ms, drawn fresh"),
    # Argon2id timings, where a build prints them.
    (re.compile(r"in \d+(?:\.\d+)? ?m?s\b"), "in <duration>"),
]


def steady(text: str) -> str:
    """A transcript with the parts that cannot repeat replaced."""
    for pattern, replacement in VOLATILE:
        text = pattern.sub(replacement, text)
    return text


def binary(name: str) -> str | None:
    target = os.environ.get("CARGO_TARGET_DIR") or os.path.join(ROOT, "target")
    exe = name + (".exe" if os.name == "nt" else "")
    for profile in ("release", "debug"):
        path = os.path.join(target, profile, exe)
        if os.path.exists(path):
            return path
    return None


def sample_wav(path: str, seconds: float = 3.0, rate: int = 16000) -> None:
    """A synthetic voiced tone to veil.

    Generated rather than committed, for the same reason every other picture
    in this repository is generated: a binary blob in the tree is a thing
    nobody can check. It is a harmonic series over a wobbling fundamental,
    which is close enough to a voiced vowel for the engine to find a pitch in
    and is nobody's actual voice, which matters for a file that ships.
    """
    frames = []
    for n in range(int(rate * seconds)):
        t = n / rate
        f0 = 120 + 12 * math.sin(2 * math.pi * 0.7 * t)
        value = 0.0
        for harmonic, amplitude in enumerate([1.0, 0.6, 0.45, 0.3, 0.22, 0.15], start=1):
            value += amplitude * math.sin(2 * math.pi * f0 * harmonic * t)
        edge = min(t, seconds - t)
        envelope = min(1.0, edge / 0.2)
        frames.append(int(max(-1.0, min(1.0, value / 3.0)) * envelope * 20000))
    with wave.open(path, "w") as handle:
        handle.setnchannels(1)
        handle.setsampwidth(2)
        handle.setframerate(rate)
        handle.writeframes(b"".join(struct.pack("<h", f) for f in frames))


def run_in_terminal(argv, cwd, typed=(), limit=90.0) -> str:
    """Run a command on a pseudo-terminal, typing at its prompts.

    `typed` is sent one line at a time, each after the program has gone quiet,
    which is what "wait for the prompt" means without parsing the prompt. A
    program that asks for nothing gets nothing.
    """
    pid, fd = pty.fork()
    if pid == 0:  # pragma: no cover - the child never returns
        os.chdir(cwd)
        os.environ["COLUMNS"] = str(COLUMNS)
        os.environ["LINES"] = "40"
        # A terminal that claims no capabilities, so nothing writes cursor
        # movement or colour into a file meant to be read as text.
        os.environ["TERM"] = "dumb"
        os.environ["NO_COLOR"] = "1"
        os.execv(argv[0], argv)

    out = bytearray()
    queue = list(typed)
    deadline = time.time() + limit
    quiet = 0.0
    while time.time() < deadline:
        ready, _, _ = select.select([fd], [], [], 0.3)
        if ready:
            try:
                chunk = os.read(fd, 4096)
            except OSError:
                break
            if not chunk:
                break
            out += chunk
            quiet = 0.0
            continue
        quiet += 0.3
        if queue and quiet >= 0.9:
            os.write(fd, queue.pop(0).encode() + b"\n")
            quiet = 0.0
        elif not queue and quiet >= 2.0:
            break
    try:
        os.waitpid(pid, 0)
    except ChildProcessError:
        pass
    os.close(fd)
    return out.decode("utf-8", "replace").replace("\r\n", "\n")


def clean_transcript(text: str) -> str:
    """Trailing spaces off, one trailing newline, no blank run longer than one."""
    lines = [line.rstrip() for line in text.split("\n")]
    tidied = []
    for line in lines:
        if not line and tidied and not tidied[-1]:
            continue
        tidied.append(line)
    while tidied and not tidied[-1]:
        tidied.pop()
    return "\n".join(tidied) + "\n"


PASSPHRASE = "correct horse battery staple"

# Every session: what it is called, what it is for, the steps, and how it is
# checked. `rerun` sessions are reproduced here on demand; `witnessed` ones
# need a published release and are held to what this repository can prove.
SESSIONS = [
    {
        "name": "anonymise",
        "programme": "veilvoice",
        "title": "Veiling one recording",
        "note": "The whole point of the program, on a three second recording, "
                "with the result sealed to a key so nothing is typed.",
        "how": "rerun",
        "steps": [
            {"show": "veilvoice keygen",
             "argv": ["keygen"],
             "typed": [PASSPHRASE, PASSPHRASE]},
            {"show": "veilvoice anonymise interview.wav -o veiled.veil --encrypt-to veilvoice.pub",
             "argv": ["anonymise", "interview.wav", "-o", "veiled.veil",
                      "--encrypt-to", "veilvoice.pub"]},
        ],
        "files": {"interview.wav": "sample"},
    },
    {
        "name": "refusal",
        "programme": "veilvoice",
        "title": "What it does with no terminal to ask on",
        "note": "The same command in a script, where nobody can type a "
                "passphrase. It stops, says why, and writes nothing.",
        "how": "rerun",
        "pipe": True,
        "steps": [
            {"show": "veilvoice anonymise interview.wav -o veiled.wav",
             "argv": ["anonymise", "interview.wav", "-o", "veiled.wav"]},
        ],
        "files": {"interview.wav": "sample"},
    },
    {
        "name": "unencrypted",
        "programme": "veilvoice",
        "title": "Asking for it in the clear, and being told what that means",
        "note": "The escape hatch exists. It says in full what you are giving "
                "up before it uses it.",
        "how": "rerun",
        "pipe": True,
        "steps": [
            {"show": "veilvoice anonymise interview.wav -o veiled.wav --encrypt false --yes",
             "argv": ["anonymise", "interview.wav", "-o", "veiled.wav",
                      "--encrypt", "false", "--yes"]},
        ],
        "files": {"interview.wav": "sample"},
    },
    {
        "name": "info",
        "programme": "veilvoice",
        "title": "What this build can do",
        "note": "Every version, whether live audio is available on this "
                "machine, and the network answer.",
        "how": "rerun",
        "pipe": True,
        "steps": [
            {"show": "veilvoice info", "argv": ["info"]},
        ],
        "files": {},
    },
    {
        "name": "verify",
        "programme": "veilvoice-verify",
        "title": "Checking a download",
        "note": "The published release, its signed hash list, and the same "
                "question asked again of your own GnuPG.",
        "how": "witnessed",
        "pipe": True,
        "steps": [
            {"show": "veilvoice-verify auto .", "argv": ["auto", "."]},
        ],
        "files": {},
    },
]


def make_files(where, wanted):
    for name, kind in wanted.items():
        if kind == "sample":
            sample_wav(os.path.join(where, name))
        else:
            raise SystemExit("unknown fixture kind %r" % kind)


def record_one(session, release_dir=None):
    """Run one session and return its transcript."""
    exe = binary(session["programme"])
    if exe is None:
        raise SystemExit(
            "no `%s` build found. Run:\n"
            "    cargo build --release -p veilvoice-cli -p veilvoice-verify"
            % session["programme"])

    if session["how"] == "witnessed":
        if not release_dir:
            raise SystemExit(
                "the %s session checks a published release.\n"
                "  Download the archive, SHA256SUMS, SHA256SUMS.asc and the\n"
                "  signing key into one folder and pass --release <FOLDER>."
                % session["name"])
        where = release_dir
        temporary = None
    else:
        temporary = tempfile.mkdtemp(prefix="veilvoice-session-")
        where = temporary
        make_files(where, session["files"])

    try:
        parts = []
        for step in session["steps"]:
            parts.append("$ " + step["show"])
            argv = [exe] + step["argv"]
            if session.get("pipe"):
                # No terminal on purpose: this is what a script sees, and for
                # three of these it is also simply the shorter way to the same
                # bytes because nothing is asked.
                done = subprocess.run(argv, cwd=where, capture_output=True, check=False)
                text = (done.stdout or b"") + (done.stderr or b"")
                parts.append(text.decode("utf-8", "replace").replace("\r\n", "\n"))
            else:
                parts.append(run_in_terminal(argv, where, step.get("typed", ())))
        return clean_transcript("\n".join(parts))
    finally:
        if temporary:
            shutil.rmtree(temporary, ignore_errors=True)


def path_for(name):
    return os.path.join(OUT, "session-%s.txt" % name)


def read(path):
    with io.open(path, encoding="utf-8") as handle:
        return handle.read()


def workspace_version():
    text = read(os.path.join(ROOT, "Cargo.toml"))
    found = re.search(r'(?m)^version = "([0-9]+\.[0-9]+\.[0-9]+)"$', text)
    return found.group(1) if found else None


def signing_fingerprint():
    """The fingerprint of the key this project signs with.

    Read from `FINGERPRINT` in `veilvoice-check`, which is the constant the
    verifier itself compares the embedded key against. That is the source
    rather than a copy of it: a transcript naming a different key fails
    against the same value the program uses, not against a second written
    down somewhere for humans.
    """
    text = read(os.path.join(ROOT, "crates", "veilvoice-check", "src", "lib.rs"))
    found = re.search(r'pub const FINGERPRINT: &str = "([0-9A-F]{40})"', text)
    return found.group(1) if found else None


def check_witnessed(name, transcript):
    """What this repository can prove about a transcript it cannot re-run."""
    problems = []
    fingerprint = signing_fingerprint()
    version = workspace_version()
    if not fingerprint:
        problems.append("no 40-character fingerprint found in README.md to compare against")
    elif fingerprint not in transcript:
        problems.append("does not name the signing key %s that README.md publishes" % fingerprint)
    if version and version not in transcript:
        problems.append("does not mention the workspace version %s, so it is from an older release"
                        % version)
    for required in ("ok    signature over the hash list is good",
                     "ok    sha256 matches",
                     "INTACT."):
        if required not in transcript:
            problems.append("does not contain %r, which the verifier prints on a pass" % required)
    return problems


def record(release_dir):
    os.makedirs(OUT, exist_ok=True)
    for session in SESSIONS:
        if session["how"] == "witnessed" and not release_dir:
            print("  skipped  %-12s needs --release <FOLDER>" % session["name"])
            continue
        transcript = record_one(session, release_dir)
        with io.open(path_for(session["name"]), "w", encoding="utf-8", newline="\n") as handle:
            handle.write(transcript)
        print("  recorded %-12s %d lines" % (session["name"], transcript.count("\n")))
    return 0


def check():
    problems = []
    for session in SESSIONS:
        path = path_for(session["name"])
        if not os.path.exists(path):
            problems.append("%s: never recorded" % os.path.relpath(path, ROOT))
            continue
        committed = read(path)
        if session["how"] == "witnessed":
            for problem in check_witnessed(session["name"], committed):
                problems.append("%s: %s" % (os.path.relpath(path, ROOT), problem))
            continue
        fresh = record_one(session)
        if steady(fresh) != steady(committed):
            problems.append(
                "%s is not what the program prints now.\n"
                "    Run: tools/shots/sessions.py --record"
                % os.path.relpath(path, ROOT))
    if problems:
        for problem in problems:
            print("  " + problem, file=sys.stderr)
        return 1
    print("  %d recorded sessions match the programs" % len(SESSIONS))
    return 0


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("--record", action="store_true", help="run the sessions and write them down")
    parser.add_argument("--check", action="store_true", help="verify the transcripts are current")
    parser.add_argument("--release", metavar="FOLDER",
                        help="a folder holding a downloaded release, for the verifier session")
    args = parser.parse_args()
    if args.record:
        return record(args.release)
    return check()


if __name__ == "__main__":
    raise SystemExit(main())
