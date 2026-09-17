# Sandbox image

The image installs both Codex and Claude. Each container has separate named
volumes at `/home/node/.codex` and `/home/node/.claude`, owned by `node`. The
host's agent login directories are never mounted. `paddock rm` leaves these
volumes in place; `paddock reset` removes them. An existing container must be
removed and recreated to gain the Codex volume and updated image.

The image does not contain the live egress policy. The host CLI writes it into
root-owned `/etc/paddock` and runs `init-firewall.sh` before an agent starts.
The agent can run the firewall helper through sudo but cannot edit its inputs.
