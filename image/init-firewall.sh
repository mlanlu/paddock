#!/bin/bash
# Default-deny egress firewall for paddock sandboxes. Re-applied on every start.
#
# Policy is read from /etc/paddock (root-owned, written by paddock on the host
# via docker exec — never by the agent):
#   mode                 "enforce" (default) or "open" (no restrictions)
#   allowed-domains.txt  one hostname per line, resolved to IPv4 here
#   github               "1" to allow GitHub's published IP ranges (api.github.com/meta)
#   host-ports.txt       TCP port ranges on host.docker.internal, e.g. 54321:54329
#
# FAIL-CLOSED: DROP policies and the fixed rules go in first; the allowlist is
# populated last. If anything below fails, the sandbox is left with DNS only.
set -euo pipefail
IFS=$'\n\t'

P=/etc/paddock
MODE=$(cat "$P/mode" 2>/dev/null || echo enforce)

DOCKER_DNS_RULES=$(iptables-save -t nat | grep "127\.0\.0\.11" || true)
iptables -F; iptables -X
iptables -t nat -F; iptables -t nat -X
iptables -t mangle -F; iptables -t mangle -X
ipset destroy allowed-domains 2>/dev/null || true
if [ -n "$DOCKER_DNS_RULES" ]; then
    iptables -t nat -N DOCKER_OUTPUT 2>/dev/null || true
    iptables -t nat -N DOCKER_POSTROUTING 2>/dev/null || true
    echo "$DOCKER_DNS_RULES" | xargs -L 1 iptables -t nat
fi

if [ "$MODE" = "open" ]; then
    iptables -P INPUT ACCEPT; iptables -P OUTPUT ACCEPT; iptables -P FORWARD DROP
    echo "firewall: OPEN — no egress restrictions"
    exit 0
fi

# 1. Closed first.
iptables -P INPUT DROP; iptables -P FORWARD DROP; iptables -P OUTPUT DROP
iptables -A INPUT -i lo -j ACCEPT
iptables -A OUTPUT -o lo -j ACCEPT
iptables -A OUTPUT -p udp --dport 53 -j ACCEPT
iptables -A INPUT -p udp --sport 53 -j ACCEPT
iptables -A INPUT -m state --state ESTABLISHED,RELATED -j ACCEPT
iptables -A OUTPUT -m state --state ESTABLISHED,RELATED -j ACCEPT

# Container subnet: lets published ports / docker-proxy reach us.
GW=$(ip route | grep default | cut -d" " -f3)
[ -n "$GW" ] || { echo "ERROR: no default route"; exit 1; }
SUBNET=$(echo "$GW" | sed "s/\.[0-9]*$/.0\/24/")
iptables -A INPUT -s "$SUBNET" -j ACCEPT
iptables -A OUTPUT -d "$SUBNET" -j ACCEPT

# Docker host, only on the configured TCP port ranges.
HOST_IPS=$(getent ahostsv4 host.docker.internal | awk '{print $1}' | sort -u || true)
if [ -s "$P/host-ports.txt" ] && [ -n "$HOST_IPS" ]; then
    while read -r range; do
        range="$(echo "${range%%#*}" | tr -d '[:space:]')"
        [ -z "$range" ] && continue
        while read -r ip; do
            iptables -A OUTPUT -d "$ip" -p tcp -m multiport --dports "$range" -j ACCEPT
        done < <(echo "$HOST_IPS")
        echo "allowed: host.docker.internal tcp $range"
    done < "$P/host-ports.txt"
fi

# The allowlist rule + explicit reject exist before the set is populated.
ipset create allowed-domains hash:net
iptables -A OUTPUT -m set --match-set allowed-domains dst -j ACCEPT
iptables -A OUTPUT -j REJECT --reject-with icmp-admin-prohibited

# 2. Populate the allowlist.
if [ "$(cat "$P/github" 2>/dev/null)" = "1" ]; then
    gh_ranges=$(curl -s --connect-timeout 10 https://api.github.com/meta || true)
    if [ -z "$gh_ranges" ] || ! echo "$gh_ranges" | jq -e '.web and .api and .git' >/dev/null 2>&1; then
        echo "ERROR: could not fetch GitHub IP ranges (network?) — sandbox stays closed"; exit 1
    fi
    while read -r cidr; do
        [[ "$cidr" =~ ^[0-9]{1,3}(\.[0-9]{1,3}){3}/[0-9]{1,2}$ ]] || { echo "ERROR: bad CIDR from GitHub meta: $cidr"; exit 1; }
        ipset add allowed-domains "$cidr" -exist
    done < <(echo "$gh_ranges" | jq -r '(.web + .api + .git)[]' | aggregate -q)
    echo "allowed: GitHub ranges"
fi

failed=0
if [ -s "$P/allowed-domains.txt" ]; then
    while read -r line; do
        domain="$(echo "${line%%#*}" | tr -d '[:space:]')"
        [ -z "$domain" ] && continue
        ips=$(dig +noall +answer +time=3 +tries=2 A "$domain" | awk '$4 == "A" {print $5}')
        if [ -z "$ips" ]; then echo "WARN: could not resolve $domain"; failed=1; continue; fi
        while read -r ip; do
            [[ "$ip" =~ ^[0-9]{1,3}(\.[0-9]{1,3}){3}$ ]] || { echo "ERROR: bad IP for $domain: $ip"; exit 1; }
            ipset add allowed-domains "$ip" -exist
        done < <(echo "$ips")
        echo "allowed: $domain"
    done < "$P/allowed-domains.txt"
fi

# 3. Verify.
if curl -s --connect-timeout 5 https://example.com >/dev/null 2>&1; then
    echo "ERROR: verification failed — https://example.com is reachable"; exit 1
fi
echo "firewall: enforced (example.com blocked)"
[ "$failed" = 0 ] || { echo "ERROR: some domains did not resolve — run \`paddock firewall\` once the network is back"; exit 1; }
