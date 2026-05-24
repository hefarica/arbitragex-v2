FROM mcr.microsoft.com/devcontainers/javascript-node:1-20-bookworm

ENV DEBIAN_FRONTEND=noninteractive \
    NEXT_TELEMETRY_DISABLED=1

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates \
        curl \
        git \
        jq \
        openssh-client \
        procps \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /workspace
