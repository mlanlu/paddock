# Egress policy sets

Each `.txt` file lists one hostname per line. Blank lines and `#` comments are
ignored. Sets are additive: the selected agent's set (`codex` or `claude`) is
combined with the profile's sets and extra domains. `open` disables the
firewall. There is no deny list because a hostname can share an IP with an
allowed hostname; see the [threat model](../README.md#egress-policy-modes-that-compose).

`codex.txt` lists the endpoints for ChatGPT sign-in, device authentication,
and OpenAI API use. Verify those flows when Docker is available; service
endpoints can change.
`claude.txt` covers Claude Code's service hosts. Only the selected agent's set
is added automatically. The host can override a set in
`~/.config/paddock/policies/`; the sandbox cannot reach that directory.
