# Hosting the VeilVoice website yourself

The site is static files in [`website/`](../website). It has no build step and
nothing server-side, so any web server that serves a directory can host it, the
same way GitHub Pages does at <https://tilas01.github.io/veilvoice/>.

Two ready-made ways:

| For | Use |
| --- | --- |
| A quick local look, or the repository going down | `python3 tools/site/serve.py` |
| A real server, a mirror, an internal copy | [`deploy/nginx.conf`](nginx.conf) |

## The development script

```sh
python3 tools/site/serve.py           # http://localhost:8000
python3 tools/site/serve.py --check   # start, fetch every page, report, exit
```

`--check` is what proves the site is servable and every page returns 200; it is
wired into `tools/verify.py` so a broken page is caught before it is pushed.

## Under nginx

Edit the `root` in [`nginx.conf`](nginx.conf) to point at your checkout's
`website/` directory, include the file, `nginx -t`, reload. Caddy, Apache and
anything else that serves a directory work identically; the config just writes
the few content-type and header details down for the server people most often
reach for.

## Why static matters here

A site that can be served by copying a folder is a site that can be mirrored,
audited and trusted. The one number on it that must never be stale -- the
signing-key fingerprint you check a release against -- is in the HTML, so a
mirror shows exactly what the source says, with nothing in between to get it
wrong.
