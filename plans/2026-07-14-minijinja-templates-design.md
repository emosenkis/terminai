# Minijinja Templates Design

## Goal

Replace Handlebars with Minijinja for prompt and agent argument rendering,
while making the default prompt fully extensible and allowing argument
expressions to expand to zero or more command-line arguments.

## Configuration model

Agent `args` and `extra-args` entries accept either a string or an expression
object:

```yaml
args:
  - --static
  - "--cwd={{ cwd }}"
  - expr: '["--mcp-config", mcp_url] if uses_mcp else []'
```

String entries are rendered as Minijinja templates and produce exactly one
argument, including when the rendered value is empty. Expression entries are
compiled and evaluated as Minijinja expressions and must produce an array of
strings. An empty array omits the entry. Undefined values and wrong result
types are errors with the source argument included in the error context.

The existing template data remains available: `cwd`, `mcp_url`,
`tool_command`, `mcp_command`, `mcp_port`, `context_prompt`, `uses_mcp`, and
`uses_tool_cli`. JSON and TOML string encoding are exposed as filters so values
can be written as `{{ context_prompt|toml }}` or `{{ mcp_url|json }}`.

Agent configs and presets can set `prompt-template`. It inherits through the
preset chain and defaults to `default.jinja`.

## Prompt templates

The bundled `config/general.yaml` is replaced by `config/default.jinja`. The
bundled prompt is divided into named Jinja blocks for the base introduction,
general rules, MCP instructions, CLI introduction, and CLI fallback
instructions. User templates can extend it and override only the blocks they
need.

Templates are resolved by one Minijinja environment with strict undefined
behavior and no auto-escaping. Template names use forward-slash-separated,
relative paths and cannot escape the Terminai XDG config directory.

The loader provides two special names:

- `default.jinja` loads `$XDG_CONFIG_HOME/terminai/default.jinja` when present,
  otherwise it loads the bundled default.
- `builtin/default.jinja` always loads the bundled default.

All other names load from `$XDG_CONFIG_HOME/terminai/<name>`. This permits a
user default to extend `{% extends "builtin/default.jinja" %}`, while a selected
non-default template can extend `{% extends "default.jinja" %}` and receive the
user-shadowed default automatically.

## Rendering flow

`build_launch_plan` first resolves the preset and direct agent overrides,
including the prompt template name. It creates the Minijinja environment,
renders the selected prompt with the resolved MCP and CLI flags, then renders
or evaluates each argument using the same context. The rendered prompt is
available to argument templates as `context_prompt`.

The loader reads templates at launch-plan construction time, so changes in the
XDG directory are reflected in newly launched agents without process-global
template state.

## Errors and compatibility

Handlebars syntax and the custom `args`, `arg`, and `OMIT` helpers are removed.
Bundled YAML is migrated to Jinja syntax and expression objects. Missing
templates, invalid inheritance, invalid expressions, undefined variables,
non-array expression results, and non-string array elements all fail launch
plan construction with contextual errors.

The old unused `initial-prompt` field is replaced by `prompt-template` rather
than retaining two competing prompt customization mechanisms.

## Verification

Tests cover bundled presets, Jinja string rendering and filters, conditional
argument expansion, invalid expression result types, strict undefined values,
prompt block overrides, XDG default shadowing, explicit built-in inheritance,
custom-template inheritance through the shadowed default, template traversal
rejection, and preset/direct template selection. The generated schema and
configuration documentation are refreshed after the Rust tests pass.
