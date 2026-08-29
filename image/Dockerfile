# paddock sandbox image. Profile-independent: egress policy is written to the
# root-owned /etc/paddock by paddock (on the host) at every start and read by
# init-firewall.sh. The agent runs as `node` and cannot touch either.
#
# Adapted from the reference Claude Code devcontainer:
# https://github.com/anthropics/claude-code/tree/main/.devcontainer
ARG NODE_VERSION=22
FROM node:${NODE_VERSION}-bookworm

ARG TZ=UTC
ENV TZ="$TZ"
ARG CLAUDE_CODE_VERSION=latest
ARG GIT_DELTA_VERSION=0.18.2
ARG ZSH_IN_DOCKER_VERSION=1.2.0

RUN apt-get update && apt-get install -y --no-install-recommends \
  less git procps sudo fzf zsh man-db unzip gnupg2 gh \
  iptables ipset iproute2 dnsutils aggregate jq nano vim ripgrep \
  && apt-get clean && rm -rf /var/lib/apt/lists/*

# corepack: the exact package manager version is activated at container
# creation from the repo's package.json#packageManager.
RUN corepack enable

RUN mkdir -p /usr/local/share/npm-global && chown -R node:node /usr/local/share

# Shell history persists on a per-container volume.
RUN mkdir /commandhistory && touch /commandhistory/.bash_history && chown -R node /commandhistory
ENV PADDOCK=1

# Claude config, pnpm store, turbo cache: node-owned, volume-backed.
RUN mkdir -p /home/node/.claude /home/node/.local/share/pnpm/store /home/node/.cache/turbo && \
  chown -R node:node /home/node/.claude /home/node/.local /home/node/.cache

RUN ARCH=$(dpkg --print-architecture) && \
  wget -q "https://github.com/dandavison/delta/releases/download/${GIT_DELTA_VERSION}/git-delta_${GIT_DELTA_VERSION}_${ARCH}.deb" && \
  dpkg -i "git-delta_${GIT_DELTA_VERSION}_${ARCH}.deb" && rm "git-delta_${GIT_DELTA_VERSION}_${ARCH}.deb"

USER node
ENV NPM_CONFIG_PREFIX=/usr/local/share/npm-global
ENV PATH=$PATH:/usr/local/share/npm-global/bin
ENV SHELL=/bin/zsh
ENV EDITOR=nano
ENV VISUAL=nano
ENV LANG=C.UTF-8

RUN sh -c "$(wget -O- https://github.com/deluan/zsh-in-docker/releases/download/v${ZSH_IN_DOCKER_VERSION}/zsh-in-docker.sh)" -- \
  -p git -p fzf \
  -a "source /usr/share/doc/fzf/examples/key-bindings.zsh" \
  -a "source /usr/share/doc/fzf/examples/completion.zsh" \
  -a "export PROMPT_COMMAND='history -a' && export HISTFILE=/commandhistory/.bash_history" \
  -x

RUN npm install -g @anthropic-ai/claude-code@${CLAUDE_CODE_VERSION}

# Privileged helpers. sudo is limited to exactly these two scripts; policy in
# /etc/paddock is root-owned and written from the host.
USER root
COPY init-firewall.sh init-volumes.sh /usr/local/bin/
RUN mkdir -p /etc/paddock && chmod 0755 /etc/paddock && \
  chmod 0755 /usr/local/bin/init-firewall.sh /usr/local/bin/init-volumes.sh && \
  echo "node ALL=(root) NOPASSWD: /usr/local/bin/init-firewall.sh, /usr/local/bin/init-volumes.sh" > /etc/sudoers.d/paddock && \
  chmod 0440 /etc/sudoers.d/paddock
USER node
WORKDIR /home/node
CMD ["sleep", "infinity"]
