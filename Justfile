# PIM dev workflow recipes.
#
# Governs the local Zitadel + PIM services loop. Pairs with:
#   - compose.yml          (stack definition, D11-D15)
#   - bootstrap/steps.yaml (Zitadel FirstInstance + admin PAT, D12/D14)
#   - bootstrap/dev.toml   (declarative tenant spec, ADR-0005)
#   - .env.local.example   (three-layer config Layer C, ADR-0012)
#
# Full rationale in:
#   docs/decisions/0005-bootstrap-local-zitadel-with-pim-bootstrap-tool.md
#   docs/decisions/0012-three-layer-config-split-by-sensitivity.md
#   docs/decisions/0013-dev-prod-parity-via-declarative-bootstrap.md
#
# All recipes assume the repo root as CWD. `just --list` to browse.

set shell := ["bash", "-euo", "pipefail", "-c"]
set dotenv-load := false

# Compose CLI. Override with `COMPOSE=docker\ compose just dev-up` if needed.
compose := env_var_or_default("COMPOSE", "podman compose")

# Named volume where Zitadel FirstInstance drops the admin PAT (see
# bootstrap/steps.yaml PatPath). compose.yml declares this as a top-level
# `zitadel-bootstrap` volume; podman prefixes it with the project name.
volume := env_var_or_default("PIM_ZITADEL_BOOTSTRAP_VOLUME", "pim_zitadel-bootstrap")

# Default: list recipes.
default:
    @just --list

# --- Dev lifecycle ---------------------------------------------------------

# First run mints ZITADEL_MASTERKEY, boots postgres + zitadel-api, and copies
# the admin PAT from the zitadel-bootstrap volume into .env.local. Subsequent
# calls skip masterkey regeneration if already set.
#
# Boot the IdP stack and mint masterkey + admin PAT into .env.local (idempotent).
dev-up:
    #!/usr/bin/env bash
    set -euo pipefail

    if [ ! -f .env.local ]; then
        echo "→ creating .env.local from .env.local.example"
        cp .env.local.example .env.local
    fi

    # Source current values (tolerate blanks).
    set +u
    source .env.local
    set -u

    if [ -z "${ZITADEL_MASTERKEY:-}" ]; then
        echo "→ minting ZITADEL_MASTERKEY (32 chars, immutable after first boot)"
        # `openssl rand -base64 24` ≈ 32 chars; strip any padding/newline noise
        # and trim to exactly 32 bytes so Zitadel's strict length check passes.
        key=$(openssl rand -base64 32 | tr -d '\n=+/' | cut -c1-32)
        if [ ${#key} -ne 32 ]; then
            echo "!! masterkey length ${#key} != 32" >&2
            exit 1
        fi
        # In-place upsert without requiring gnu sed.
        python3 - "$key" <<'PY'
    import pathlib, sys
    key = sys.argv[1]
    p = pathlib.Path(".env.local")
    out, seen = [], False
    for line in p.read_text().splitlines():
        if line.startswith("ZITADEL_MASTERKEY="):
            out.append(f"ZITADEL_MASTERKEY={key}")
            seen = True
        else:
            out.append(line)
    if not seen:
        out.append(f"ZITADEL_MASTERKEY={key}")
    p.write_text("\n".join(out) + "\n")
    PY
    fi

    echo "→ starting postgres + zitadel-api (app profile disabled)"
    {{compose}} up -d postgres zitadel-api

    # Wait for zitadel-api to be healthy before trying to pull the PAT.
    echo "→ waiting for zitadel-api to mint admin PAT (up to 120s)"
    deadline=$(( $(date +%s) + 120 ))
    while : ; do
        if {{compose}} exec -T zitadel-api test -s /zitadel/bootstrap/pim-admin.pat 2>/dev/null; then
            break
        fi
        if [ "$(date +%s)" -gt "$deadline" ]; then
            echo "!! timed out waiting for /zitadel/bootstrap/pim-admin.pat" >&2
            echo "   inspect: {{compose}} logs zitadel-api" >&2
            exit 1
        fi
        sleep 2
    done

    pat=$({{compose}} exec -T zitadel-api cat /zitadel/bootstrap/pim-admin.pat | tr -d '\r\n')
    if [ -z "$pat" ]; then
        echo "!! admin PAT is empty" >&2
        exit 1
    fi

    echo "→ writing ZITADEL_ADMIN_PAT into .env.local"
    python3 - "$pat" <<'PY'
    import pathlib, sys
    pat = sys.argv[1]
    p = pathlib.Path(".env.local")
    out, seen = [], False
    for line in p.read_text().splitlines():
        if line.startswith("ZITADEL_ADMIN_PAT="):
            out.append(f"ZITADEL_ADMIN_PAT={pat}")
            seen = True
        else:
            out.append(line)
    if not seen:
        out.append(f"ZITADEL_ADMIN_PAT={pat}")
    p.write_text("\n".join(out) + "\n")
    PY

    echo "✓ dev-up complete. Next: just dev-bootstrap"

# Ensures the declarative tenant spec (bootstrap/dev.toml) is realised in the
# running Zitadel, then applies the dev seed. Reads ZITADEL_ADMIN_PAT from
# .env.local.
#
# Run pim-bootstrap against the running Zitadel, then seed dev users.
dev-bootstrap:
    #!/usr/bin/env bash
    set -euo pipefail
    set +u; source .env.local; set -u
    if [ -z "${ZITADEL_ADMIN_PAT:-}" ]; then
        echo "!! ZITADEL_ADMIN_PAT is empty; did you run just dev-up?" >&2
        exit 1
    fi
    export ZITADEL_ADMIN_PAT
    cargo run --quiet -p pim-bootstrap -- \
        bootstrap --config bootstrap/dev.toml
    cargo run --quiet -p pim-bootstrap -- \
        seed --config bootstrap/seed.dev.toml

# Read-only drift check. Safe to run against dev or prod configs.
dev-diff config="bootstrap/dev.toml":
    #!/usr/bin/env bash
    set -euo pipefail
    set +u; source .env.local; set -u
    export ZITADEL_ADMIN_PAT
    cargo run --quiet -p pim-bootstrap -- diff --config {{config}}

# Bring up the PIM services after bootstrap has generated their configs.
dev-apps:
    {{compose}} --profile app up -d --build

# Stop everything but preserve volumes (data survives across restarts).
dev-down:
    {{compose}} --profile app down

# Drops containers AND volumes (postgres-data + zitadel-bootstrap). The next
# `just dev-up` will regenerate ZITADEL_MASTERKEY and mint a fresh PAT. Also
# clears the symmetric secrets from .env.local so they can't drift.
#
# Wipe the stack (containers + volumes) and clear generated secrets.
dev-reset:
    #!/usr/bin/env bash
    set -euo pipefail
    {{compose}} --profile app down -v
    if [ -f .env.local ]; then
        echo "→ clearing generated values in .env.local"
        python3 - <<'PY'
    import pathlib
    p = pathlib.Path(".env.local")
    cleared = {"ZITADEL_MASTERKEY", "ZITADEL_ADMIN_PAT",
               "USER_SERVICE__ZITADEL_SERVICE_ACCOUNT_TOKEN"}
    out = []
    for line in p.read_text().splitlines():
        key, _, _ = line.partition("=")
        if key.strip() in cleared:
            out.append(f"{key.strip()}=")
        else:
            out.append(line)
    p.write_text("\n".join(out) + "\n")
    PY
    fi
    echo "✓ dev-reset complete. Next: just dev-up"

# Tail logs for the core IdP stack.
dev-logs service="zitadel-api":
    {{compose}} logs -f {{service}}

# --- Prod helpers ----------------------------------------------------------

# Requires the caller to export PIM_BOOTSTRAP_ADMIN_KEY_FILE pointing at the
# JWT profile JSON.
#
# Dry-run the prod config so operators can eyeball the plan.
prod-diff config="bootstrap/prod.toml":
    cargo run --quiet -p pim-bootstrap -- diff --config {{config}}
