# Contributing to Faber

Faber's runtime is developed and tested inside Docker; do not `cargo run` it on
the host. [DEVELOPMENT.md](DEVELOPMENT.md) has the full loop, the test suites
and the disposable VM. The short path:

```bash
./scripts/dev.sh up      # API on http://localhost:3000/api/v1, key just-a-test-api-key
./scripts/dev.sh check   # rustfmt and Clippy
./scripts/dev.sh test
./scripts/dev.sh down
```

Security-relevant changes should keep [SECURITY.md](SECURITY.md) and
[SECURITY_TEST_MATRIX.md](SECURITY_TEST_MATRIX.md) true. Planned work and what
is out of scope are in [ROADMAP.md](ROADMAP.md).

## README images

Faber has no screen of its own, so the README images are real terminal
sessions against a running instance, composed into windows from
`assets/screenshots/source/`:

- `session/*.cmds` are the commands, `session/jobs/` and the source files are
  their inputs, and `session/rec.sh` replays a session and prints the
  transcript.
- `render.py` turns the transcripts in `shots/` (not committed) into
  `hero.html`, `contained.html`, `judge.html` and `guardrails.html`, which share
  `style.css`.

To remake them, build an image with Python added (the sessions use it), start
it on a spare port with `MEMORY_MAX=128M`, and record:

```bash
docker build -f docker/prod/Dockerfile -t faber-readme .
printf 'FROM faber-readme\nRUN apt-get update && apt-get install -y --no-install-recommends python3\n' \
  | docker build -t faber-readme:py -
docker run -d --name faber-readme --init --privileged --cgroupns=host \
  -v /sys/fs/cgroup:/sys/fs/cgroup:rw --read-only --tmpfs /tmp:size=64m \
  -e API_KEY="$FABER_KEY" -e MEMORY_MAX=128M -p 127.0.0.1:41500:3000 faber-readme:py

cd assets/screenshots/source
mkdir -p shots
for s in compile contained judge guardrails; do
  FABER_KEY="$FABER_KEY" session/rec.sh session/$s.cmds > shots/$s.txt
done
python3 render.py
```

Render each page with headless Chrome at 1600 x 1000 CSS pixels and 2x scale,
then convert to WebP:

```bash
for n in hero contained judge guardrails; do
  google-chrome-stable --headless=new --hide-scrollbars --force-device-scale-factor=2 \
    --window-size=1600,1000 --screenshot=/tmp/$n.png "file://$PWD/$n.html"
  cwebp -q 82 /tmp/$n.png -o ../$n.webp
done
docker rm -f faber-readme
```
