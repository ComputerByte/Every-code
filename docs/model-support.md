# Modern model support

Evidence checked on 2026-10-10 against the fork base
`e7d4a5d7c7183eaa6b433f7f9b5e9aa04336cc5c`. This change preserves its
Responses-Lite include fix. No production credentials or unconfigured defaults
were modified. Merge and deployment require operator approval.

## Acceptance status

The 2026-10-13 repair restores CLI-family read-only defaults for configured role
names, one bounded OAuth refresh after an official ChatGPT WebSocket handshake
401, and automatic compaction for eligible context errors. Explicit error codes
take precedence over message wording. Permanent model/parameter errors remain
terminal; Responses-Lite serialization and exact model IDs are preserved.
Deterministic tests exercise actual subprocess arguments, refreshed requests,
HTTP/WS compaction with retained conversation state, and GPT-6.1 create/wait/result
delegation to all three Claude 5.5 models, including child failure reporting.
Fresh independent review compared both fork main and the pre-repair PR head:
**GO for all three repairs and PR #2 merge readiness**, within the mocked
validation scope; **NO-GO for unattended VexOrch replacement**.

These repairs do not establish a sandbox. Global Claude `bypassPermissions`, MCP
access, filesystem isolation, process cancellation, and unattended orchestration
remain separate operational limitations. Merge readiness does not authorize
installation, deployment, or an unattended VexOrch replacement.

Original modernization review: **Claude Opus 5.5 executable resolution verified; overall interactive Claude gap remains as documented**.
The executable resolution discrepancy between older Homebrew Claude (`/opt/homebrew/bin/claude` 2.1.191) and modern npm Claude (`/Users/byte/.npm-global/bin/claude` 2.1.296) has been repaired through explicit agent command configuration without hardcoding operator paths in portable application defaults.
Claude Opus 5.5 authenticated execution, streaming, tool calling, and live child delegation from GPT-6.1 Sol are verified. Sonnet 5.5 and Haiku 5.5 remain verified. Opus 5.5 is enabled in the operator's agent configuration.
As previously audited, Claude execution is via CLI agents; a native Anthropic Messages transport for the interactive main agent remains an external gap outside CLI delegation. PR #2 remains a draft for operator review; installation and deployment require separate authorization.

## Support matrix

“Documented” means the identifier and capabilities appear in the provider's
own documentation; it does not imply every account has access. “Mocked” means
Every Code's real request builder/transport was exercised against a local server.

| Exact ID | Public context / output tokens | Reasoning / effort | Execution evidence |
| --- | --- | --- | --- |
| `gpt-6.1-sol` | 1,050,000 / 128,000 | low, medium, high, xhigh, max | HTTP/WS Lite + public Responses mocked; account discovery; authenticated ModelClient and built CLI returned MODEL_OK; actual built-CLI child delegation returned DELEGATION_OK |
| `gpt-6-astra` | 1,050,000 / 128,000 | low, medium, high, xhigh, max | Official docs; account discovery; HTTP/WS Lite + public Responses mocked; inference not smoke tested |
| `gpt-6-sol` | 1,050,000 / 128,000 | none, low, medium, high, xhigh, max | Official docs; account discovery; HTTP/WS Lite + public Responses mocked; inference not smoke tested |
| `gpt-6-luna` | 1,050,000 / 128,000 | none, low, medium, high, xhigh, max | Official docs; account discovery; HTTP/WS Lite + public Responses mocked; inference not smoke tested |
| `gpt-5.6-sol` | 1,050,000 / 128,000 | none, low, medium, high, xhigh, max | Official docs; account discovery; HTTP/WS Lite + public Responses mocked; inference not smoke tested |
| `gpt-5.6-terra` | 1,050,000 / 128,000 | none, low, medium, high, xhigh, max | Official docs; account discovery; HTTP/WS Lite + public Responses mocked; inference not smoke tested |
| `gpt-5.6-luna` | 1,050,000 / 128,000 | none, low, medium, high, xhigh, max | Official docs; account discovery; HTTP/WS Lite + public Responses mocked; inference not smoke tested |
| `claude-opus-5-5` | 1,000,000 / 128,000 | low, medium, high, xhigh, max; always adaptive thinking | Official ID; exact delegation arguments mocked; authenticated modern CLI (2.1.296 via `/Users/byte/.npm-global/bin/claude`) returned MODEL_OK; streaming + tool calling passed (OPUS_TEST_OK); real child delegation from GPT-6.1 Sol returned OPUS_DELEGATION_OK; enabled in agent configuration |
| `claude-sonnet-5-5` | 1,000,000 / 128,000 | low, medium, high, xhigh, max; adaptive by default | Exact delegation mocked; authenticated installed CLI returned MODEL_OK under exact modelUsage ID; streaming + Read tool call returned MODEL_TOOL_OK; real child delegation from GPT-6.1 Sol returned SONNET_DELEGATION_OK |
| `claude-haiku-5-5` | 1,000,000 / 128,000 | low, medium, high, xhigh, max; adaptive by default | Exact delegation mocked; authenticated installed CLI returned MODEL_OK under exact modelUsage ID; real child delegation from GPT-6.1 Sol returned HAIKU_DELEGATION_OK |

For public OpenAI models, documented maximum input is 922,000 tokens. OAuth is a
separate backend contract: observed normal context is 272,000 and maximum context
872,000; Every Code caps OAuth context at 872,000 and compaction at 784,800. These
limits were read from the backend/bundled catalog, not established with a costly
context saturation test. OAuth omits public `none` choices; its advertised `ultra`
is not newly enabled by this change. Public `minimal` is not an advertised modern
model effort. Extra third-party variants, `gpt-6-terra`, and unknown first-party
GPT-6/5.6 IDs are not advertised or substituted.

GPT-6.1 Sol remains its exact ID everywhere. A live request using the bundled
0.153.0 client floor failed with model-not-supported. The same authorized account,
model and payload succeeded with the released 0.162.1 Codex protocol. The verified
floor applies only to OAuth backend requests and OAuth discovery; public API and
custom-provider headers retain their existing version behavior. Local account
discovery with 0.162.1 lists all seven exact OpenAI IDs. Discovery with the older
version omitted GPT-6.1, GPT-6 Sol and GPT-6 Luna.

Claude execution is through the existing external Claude Code agent integration.
There is no native Anthropic Messages transport in this application. Claude Code
owns authentication, adaptive thinking, native request parameters, stream parsing,
and tool serialization. The change does not invent Anthropic thinking budgets or
send OpenAI reasoning fields to Claude. A Claude model selected for the native
OpenAI transport fails with guidance to use a Claude agent or configured compatible
proxy. Third-party Claude names are not treated as official Anthropic IDs.

## Architecture and audit map

The literal inventory in [model-support-inventory.json](model-support-inventory.json)
covers tracked Rust and JS source, scripts, tests, configuration, prompts,
documentation and the read-only mirror.
It includes historical fixtures and filename candidates, not just executable IDs;
these are not all availability claims. `codex-rs` is a read-only upstream mirror.

| Concern | Actual owners and decisions |
| --- | --- |
| Catalog / interactive picker | `core/src/modern_models.rs`, `common/src/model_presets.rs`, `tui/src/{chatwidget,remote_model_presets}.rs`; one modern registry, exact IDs, GPT-6.1 first in curated order, no first-party modern advertisements on an undiscovered custom endpoint |
| Model metadata / limits | `core/src/{model_family,config,reasoning}.rs`; public specifications separated from observed OAuth limits; modern custom endpoints get conservative provider metadata rather than the OpenAI catalog |
| Authentication / transport | `core/src/{auth,model_provider_info,default_client,client}.rs`, `core/src/remote_models/mod.rs`, `code-version/src/lib.rs`; existing OAuth and API-key mechanisms retained, no new authentication integration |
| Lite request behavior | `core/src/client_common.rs` and `client.rs`; encrypted reasoning include only, store false, parallel false, all_turns reasoning context, normal HTTP and WS paths; disabling summary preserves effort/context |
| Agents / aliases / handoffs | `core/src/{agent_defaults,agent_tool,slash_commands}.rs`, `core/src/config/defaults.rs`; exact modern agent model args, explicit --model/-m overrides, distinct configured role names, supported Claude effort flags; older Claude version aliases retain exact older IDs |
| Coordinator / background routes | `code-auto-drive-core/src/auto_coordinator.rs`, `core/src/config_types.rs`; GPT-6.1 route with low through max; actual API IDs extracted from agent args instead of code-prefixed agent slugs; configured unavailable models diagnose; invalid-model errors do not switch models |
| Retry boundaries | `core/src/{error,client}.rs`, `core/src/codex/streaming.rs`; model/parameter 400 errors terminal in request, WS, and outer turn loops; WS 404/405 transport fallback remains available |
| Other providers | Existing Gemini, Antigravity, Copilot, Qwen, Ollama, Azure and OpenRouter adapters remain in their existing execution families; shared names do not establish modern OpenAI proxy capabilities |

Existing main/review defaults (GPT-5.5) and coordinator default (GPT-5.1) are retained
to preserve established routing. For new configurations, GPT-6.1 Sol is the
recommended general coding, planning, investigation and review choice after
account validation. Sonnet 5.5 is the recommended available Claude delegation
choice; Opus 5.5 requires an operator-managed CLI update before use.

Upstream investigation used GitHub read-only APIs, without adding or writing an
upstream remote: #615's Lite store=false behavior is already present; open #618's
removal of codex-lite was independently merged in the fork's base; #621's
provider-aware compatibility/catalog work is substantially present, with Terra
extended-context support corrected here; issue #625's unbounded invalid-request
loop is addressed across request and outer retry boundaries. No blind cherry-pick
was performed. The fork's embedded model minimum version was additionally found
stale by live execution; the read-only mirror was left untouched.

## Role configuration

Existing profiles, review/planning settings, Auto Drive model settings, named
`[[agents]]` and `[subagents]` commands remain configurable. For example:

```toml
model = "gpt-6.1-sol"
review_model = "gpt-6.1-sol"
model_reasoning_effort = "max"

[auto_drive]
model = "gpt-6.1-sol"
model_reasoning_effort = "high"

[[agents]]
name = "investigation"
command = "coder"
args = ["--model", "gpt-6.1-sol"]
read-only = true

[[agents]]
name = "review"
command = "claude"
args = ["--model", "claude-sonnet-5-5", "--effort", "high"]
read-only = true

[[agents]]
name = "claude-opus-5-5"
command = "/Users/byte/.npm-global/bin/claude"
enabled = true

[[agents]]
name = "claude-sonnet-5-5"
command = "/Users/byte/.npm-global/bin/claude"
enabled = true

[[agents]]
name = "claude-haiku-5-5"
command = "/Users/byte/.npm-global/bin/claude"
enabled = true
```

When multiple installations of external agent CLIs exist on PATH (for example an
older Homebrew installation and a newer npm-global installation), specify the exact
binary path via `command` in `config.toml`. Application defaults remain portable
(`claude`) and do not embed operator-specific filesystem paths. Opus 5.5 requires
Claude Code >=2.1.280; after validation with an authorized compatible CLI, configure
and enable the agent with `enabled = true`.

## macOS Apple Silicon build and installation

Install Xcode Command Line Tools, Git, Node/pnpm if building JS components, and a
Rust toolchain satisfying the lockfile's MSRV. The checkout pins Rust 1.90 but the
current locked dependencies require newer Rust; this validation used existing
Homebrew Rust 1.96 through a repository-local rustup toolchain, without changing
an installed binary or global toolchain. Build from the fork feature branch:

```sh
git clone https://github.com/ComputerByte/Every-code.git
cd Every-code
git checkout feat/modern-model-support
rustup toolchain install stable
RUSTUP_TOOLCHAIN=stable CARGO_NET_GIT_FETCH_WITH_CLI=true PROFILE=dev ./build-fast.sh
./code-rs/target/debug/code --version
./code-rs/target/debug/code exec -m gpt-6.1-sol -c model_reasoning_effort=low "Reply with MODEL_OK"
```

Select a compatible newer toolchain using `RUSTUP_TOOLCHAIN` if required. The build
prints the binary artifact path and creates repository-local links. For an optimized
build, use `PROFILE=release`. After operator approval, install the printed artifact
under the desired name/path (for example `~/.local/bin/coder`) and ensure delegated
`[[agents]].command` points to it. Do not overwrite an installed binary before
approval. Claude Opus 5.5 separately requires Claude Code >=2.1.280; this work did
not update it. Opus 5.5 is disabled in the default agent set; after verifying CLI
version and account access, configure a named Claude agent with
`args = ["--model", "claude-opus-5-5"]` and `enabled = true`.

The ignored live regression can be run with an explicitly authorized existing
OAuth file, read into externally managed in-memory authentication with refresh and
credential writes disabled:

```sh
EVERY_CODE_SMOKE_AUTH_PATH=/path/to/existing/auth.json \
  cargo test --manifest-path code-rs/Cargo.toml -p code-core --lib \
  authenticated_gpt_6_1_sol_executes_without_substitution -- --ignored
```

Do not enable request debug logging during authenticated validation. Public
OpenAI API-key inference, live OAuth WebSocket/tool round trips, parallel/background
cross-provider agent handoffs, and context saturation remain separately unverified.
Exact GPT-6.1 child create/wait/result cycle and real delegation to Claude Opus 5.5,
Sonnet 5.5, and Haiku 5.5 were verified live through the built CLI.

## Regression repair validation (2026-10-13)

Compared with fork main `e7d4a5d7c7183eaa6b433f7f9b5e9aa04336cc5c` and
the pre-repair PR head `ef5d05449e8024875867cbe4eec3e6815e1b6f93`:

| Check | Result |
| --- | --- |
| Core unit suite, including role arguments, OAuth/provider cases and header redaction | 815 passed; 3 ignored |
| Real HTTP/WS context recovery and rejection fixtures | 14 passed |
| GPT-6.1 coordinator to three Claude models plus failed child | 1 passed |
| TUI / Auto Drive / common / version unit suites | 232 / 84 / 29 / 12 passed |
| Existing selected integration suites | 33 passed; 2 failed |
| Required Apple Silicon dev build / whitespace check | Passed without compiler warnings; rustfmt prohibited |
| Strict core Clippy, all targets | Failed on existing diagnostics; exact base also fails; no repair-added diagnostics |

Both integration failures (`repeated_tool_loop` and the blocked user-prompt hook)
reproduce on the exact fork base. Global Git URL rewriting also reproduces a
unit-fixture failure on that base; the serial core suite excludes global Git
configuration. Optional Auto Drive all-features tests pass but emit four inherited
fault-injection compiler warnings; its normal-feature suite is warning-free.
Strict Clippy still reports the seven previously audited PR diagnostics, plus
`items_after_test_module` under Rust 1.96. No full-workspace green claim is made.
Authenticated live checks in the next section are historical; this repair uses
isolated mocks and does not refresh production credentials.

## Original modernization validation (2026-10-10)

| Check | Final result |
| --- | --- |
| `code-auto-drive-core` unit suite | 84 passed |
| `code-common` unit suite | 29 passed |
| `code-core` unit suite | 808 passed; 3 ignored |
| `code-tui` unit suite | 232 passed |
| `code-version` unit suite | 12 passed |
| Provider discovery, delegation wake, tool selection integration suites | 7 passed |
| Required `PROFILE=dev ./build-fast.sh` | Passed without warnings on Apple Silicon / Rust 1.96; includes Rust type checking |
| `git diff --check` | Passed; rustfmt was not run, as repository AGENTS.md prohibits it |
| Strict Clippy | Failed on two unchanged `git-tooling/src/ghost_commits.rs` redundant closures (lines 87/92); reproduced on the exact base commit |
| Authenticated GPT-6.1 | Exact ModelClient HTTP request, final built CLI MODEL_OK, and real child create/wait/result cycle passed; no credential changes |
| Authenticated Claude | Exact Sonnet/Haiku/Opus 5.5 passed; Sonnet and Opus streaming + tool use passed; child delegation from GPT-6.1 Sol to Opus 5.5, Sonnet 5.5, and Haiku 5.5 passed live; Opus 5.5 enabled in agent configuration |

The three ignored core tests are the opt-in authenticated check and two existing
long-running unified-exec tests. The opt-in authenticated check was additionally
run successfully with authorized existing OAuth credentials. No JavaScript source
changed; JS-specific build/typecheck suites were not run.

Tests were run serially to isolate existing tests that mutate process environment.
The global Git URL rewrite was excluded for unit fixtures expecting HTTPS URLs.
Initial stale expectations for context limits/routes, a test JWT fixture, and the
routing editor's fixed cursor index were repaired; final relevant suites have no
failures. The strict-Clippy baseline failure remains, and is not reported as green.

The built binary validated after the Opus gate and resolution has SHA-256
`a61f1b9318afc3bbad847d983f3a7d009952531e1530208baf3f800f936a0f1c`.
Sanitized local validation receipts remain outside tracked source; credentials,
account files and authenticated request logs are not included in this change.

## Sources

- [OpenAI GPT-6.1 Sol](https://developers.openai.com/api/docs/models/gpt-6.1-sol)
- [OpenAI GPT-6 Astra](https://developers.openai.com/api/docs/models/gpt-6-astra)
- [OpenAI GPT-6 Sol](https://developers.openai.com/api/docs/models/gpt-6-sol)
- [OpenAI GPT-6 Luna](https://developers.openai.com/api/docs/models/gpt-6-luna)
- [OpenAI GPT-5.6 Sol](https://developers.openai.com/api/docs/models/gpt-5.6-sol)
- [OpenAI GPT-5.6 Terra](https://developers.openai.com/api/docs/models/gpt-5.6-terra)
- [OpenAI GPT-5.6 Luna](https://developers.openai.com/api/docs/models/gpt-5.6-luna)
- [Released Codex 0.162.1](https://github.com/openai/codex/releases/tag/rust-v0.162.1)
- [Anthropic model overview](https://platform.claude.com/docs/en/models/overview)
- [Anthropic effort interface](https://platform.claude.com/docs/en/build-with-claude/effort)
- [Claude Code model configuration](https://code.claude.com/docs/en/model-config)
- [Claude Code CLI reference](https://code.claude.com/docs/en/cli-reference)
- [Upstream #615](https://github.com/just-every/code/pull/615), [#618](https://github.com/just-every/code/pull/618), [#621](https://github.com/just-every/code/pull/621), [#625](https://github.com/just-every/code/issues/625)
