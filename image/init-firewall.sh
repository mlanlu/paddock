#!/bin/bash
# Default-deny egress firewall for paddock sandboxes. Re-applied on every start.
#
# Allowed:
#   - GitHub IP ranges (https://api.github.com/meta)
#   - domains in /etc/paddock/allowed-domains.txt (resolved at start)
#   - the Docker host (host.docker.internal) on the TCP ranges in /etc/paddock/host-ports.txt
#   - DNS, loopback, the container's own subnet (so published ports keep working)
# Everything else is REJECTed.
set -euo pipefail
IFS=$'\n\t'

ALLOWLIST=/etc/paddock/allowed-domains.txt
HOST_PORTS=/etc/paddock/host-ports.txt

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

iptables -A OUTPUT -p udp --dport 53 -j ACCEPT
iptables -A INPUT -p udp --sport 53 -j ACCEPT
iptables -A OUTPUT -p tcp --dport 22 -j ACCEPT
iptables -A INPUT -p tcp --sport 22 -m state --state ESTABLISHED -j ACCEPT
iptables -A INPUT -i lo -j ACCEPT
iptables -A OUTPUT -o lo -j ACCEPT

ipset create allowed-domains hash:net

gh_ranges=$(curl -s https://api.github.com/meta)
if [ -z "$gh_ranges" ] || ! echo "$gh_ranges" | jq -e '.web and .api and .git' >/dev/null; then
    echo "ERROR: could not fetch GitHub IP ranges"; exit 1
fi
while read -r cidr; do
    [[ "$cidr" =~ ^[0-9]{1,3}(\.[0-9]{1,3}){3}/[0-9]{1,2}$ ]] || { echo "ERROR: bad CIDR from GitHub meta: $cidr"; exit 1; }
    ipset add allowed-domains "$cidr" -exist
done < <(echo "$gh_ranges" | jq -r '(.web + .api + .git)[]' | aggregate -q)
echo "allowed: GitHub ranges"

while read -r line; do
    domain="$(echo "${line%%#*}" | tr -d '[:space:]')"
    [ -z "$domain" ] && continue
    ips=$(dig +noall +answer A "$domain" | awk '$4 == "A" {print $5}')
    if [ -z "$ips" ]; then echo "ERROR: failed to resolve $domain"; exit 1; fi
    while read -r ip; do
        [[ "$ip" =~ ^[0-9]{1,3}(\.[0-9]{1,3}){3}$ ]] || { echo "ERROR: bad IP for $domain: $ip"; exit 1; }
        ipset add allowed-domains "$ip" -exist
    done < <(echo "$ips")
    echo "allowed: $domain"
done < "$ALLOWLIST"

GW=$(ip route | grep default | cut -d" " -f3)
[ -n "$GW" ] || { echo "ERROR: no default route"; exit 1; }
SUBNET=$(echo "$GW" | sed "s/\.[0-9]*$/.0\/24/")
iptables -A INPUT -s "$SUBNET" -j ACCEPT
iptables -A OUTPUT -d "$SUBNET" -j ACCEPT

# Docker host, only on the configured TCP port ranges.
HOST_IPS=$(getent ahostsv4 host.docker.internal | awk '{print $1}' | sort -u || true)
if [ -s "$HOST_PORTS" ] && [ -n "$HOST_IPS" ]; then
    while read -r range; do
        range="$(echo "${range%%#*}" | tr -d '[:space:]')"
        [ -z "$range" ] && continue
        while read -r ip; do
            iptables -A OUTPUT -d "$ip" -p tcp -m multiport --dports "$range" -j ACCEPT
        done < <(echo "$HOST_IPS")
        echo "allowed: host.docker.internal tcp $range"
    done < "$HOST_PORTS"
fi

iptables -P INPUT DROP
iptables -P FORWARD DROP
iptables -P OUTPUT DROP
iptables -A INPUT -m state --state ESTABLISHED,RELATED -j ACCEPT
iptables -A OUTPUT -m state --state ESTABLISHED,RELATED -j ACCEPT
iptables -A OUTPUT -m set --match-set allowed-domains dst -j ACCEPT
iptables -A OUTPUT -j REJECT --reject-with icmp-admin-prohibited

if curl --connect-timeout 5 https://example.com >/dev/null 2>&1; then
    echo "ERROR: firewall verification failed — https://example.com is reachable"; exit 1
fi
if ! curl --connect-timeout 5 https://api.github.com/zen >/dev/null 2>&1; then
    echo "ERROR: firewall verification failed — https://api.github.com unreachable"; exit 1
fi
echo "firewall ok: example.com blocked, api.github.com reachable"
