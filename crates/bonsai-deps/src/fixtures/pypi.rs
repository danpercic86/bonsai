//! PyPI manifest / lock / config fixtures (E2 PyPI row, E3 pip / uv rows).

/// Every requirements line shape: pins, ranges, extras, markers, hashes,
/// comments, continuations, options, editable / path / URL / VCS lines.
pub(crate) const REQUIREMENTS: &str = "\u{FEFF}# top comment\r\n\
flask==3.0.2\r\n\
requests[socks] >= 2.31, < 3 ; python_version >= \"3.8\"  # trailing comment\r\n\
Django==4.2.* \\\r\n\
    --hash=sha256:0000\r\n\
-r base.txt\r\n\
--constraint constraints.txt\r\n\
--pre\r\n\
-e git+https://git.corp.invalid/zz.git#egg=zz-sentinel-editable\r\n\
-e ./local-lib\r\n\
./vendor/zz_local-1.0.tar.gz#egg=zz-local-path\r\n\
https://files.corp.invalid/zz_url-1.0-py3-none-any.whl#egg=zz-url\r\n\
zz-at-git @ git+https://git.corp.invalid/zz-at.git@v1\r\n\
zz-at-file @ file:///srv/zz\rnumpy\n";

/// A file whose own `--index-url` routes every registry dep.
pub(crate) const REQUIREMENTS_CORP_INDEX: &str =
    "--index-url https://u:zz-sentinel-token@pypi.corp.invalid/simple\n\
zz-sentinel-idx==1.0\n\
-e ./local-lib#egg=zz-local\n";

/// `--extra-index-url` to pypi.org is still public.
pub(crate) const REQUIREMENTS_PUBLIC_INDEX: &str =
    "-i https://pypi.org/simple\n--extra-index-url=https://pypi.org/simple/\nsix\n";

/// A PEP 621 + PEP 735 + uv project.
pub(crate) const PYPROJECT_UV: &str = r#"
[project]
name = "zz-app"
dependencies = [
  "httpx>=0.27",
  "attrs==23.2.0",
  "zz-pinned-idx",
  "zz-ws-member",
  "zz-src-git ; sys_platform == 'linux'",
]

[project.optional-dependencies]
cli = ["click>=8"]

[dependency-groups]
dev = ["pytest==8.0.0", { include-group = "lint" }]
lint = ["ruff"]

[tool.uv]
dev-dependencies = ["mypy"]

[tool.uv.sources]
zz-pinned-idx = { index = "Corp" }
zz_ws_member = { workspace = true }
zz-src-git = [{ git = "https://git.corp.invalid/g.git", marker = "sys_platform == 'linux'" }]

[[tool.uv.index]]
name = "corp"
url = "https://u:zz-sentinel-token@uv.corp.invalid/simple"
explicit = true
"#;

pub(crate) const UV_LOCK: &str = r#"
version = 1
requires-python = ">=3.11"

[[package]]
name = "httpx"
version = "0.27.2"
source = { registry = "https://pypi.org/simple" }

[[package]]
name = "zz-app"
version = "0.1.0"
source = { editable = "." }

[[package]]
name = "zz-pinned-idx"
version = "2.0.0"
source = { registry = "https://uv.corp.invalid/simple" }

[[package]]
name = "zz-trans-private"
version = "1.0.0"
source = { registry = "https://uv.corp.invalid/simple" }

[[package]]
name = "zz-flat"
version = "1.0.0"
source = { registry = "../wheels" }

[[package]]
name = "zz-ws-member"
source = { virtual = "packages/member" }
"#;

/// A Poetry project with an explicit and a primary source.
pub(crate) const PYPROJECT_POETRY: &str = r#"
[tool.poetry]
name = "zz-poetry-app"

[tool.poetry.dependencies]
python = "^3.11"
requests = "2.31.0"
pendulum = "^3.0"
zz-explicit = { version = "^1", source = "corp-explicit" }
zz-poetry-git = { git = "https://git.corp.invalid/p.git" }
zz-poetry-path = { path = "../lib" }
multi = [{ version = "<2", python = "<3.12" }, { version = ">=2", python = ">=3.12" }]
extra-opt = { version = "^1", optional = true }

[tool.poetry.group.test.dependencies]
pytest = "^8"

[[tool.poetry.source]]
name = "corp-explicit"
url = "https://poetry.corp.invalid/simple"
priority = "explicit"
"#;

pub(crate) const POETRY_LOCK: &str = r#"
[[package]]
name = "requests"
version = "2.31.0"

[[package]]
name = "zz-explicit"
version = "1.4.0"

[package.source]
type = "legacy"
url = "https://poetry.corp.invalid/simple"
reference = "corp-explicit"

[[package]]
name = "zz-poetry-git"
version = "0.1.0"

[package.source]
type = "git"
url = "https://git.corp.invalid/p.git"
reference = "HEAD"
resolved_reference = "abc"

[metadata]
lock-version = "2.0"
"#;

/// `pip.ini` with a continuation-line `extra-index-url` and a credential.
/// (`\x20` keeps the indent that a `\`-newline in the literal would eat.)
pub(crate) const PIP_CONF: &str = "\u{FEFF}[global]\r\n\
timeout = 60\r\n\
client_cert = /home/u/zz-sentinel-token.pem\r\n\
extra_index_url =\r\n\
\x20   https://pypi.org/simple\r\n\
\x20   https://u:zz-sentinel-token@pip.corp.invalid/simple\r\n\
; comment\r\n\
[install]\r\n\
trusted-host = pip.corp.invalid\r\n";

/// A uv config with a default private index.
pub(crate) const UV_TOML_DEFAULT: &str = r#"
[[index]]
name = "corp"
url = "https://uv-default.corp.invalid/simple"
default = true
"#;
