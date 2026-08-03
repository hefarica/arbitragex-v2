# arbx-socket-proxy — least-privilege Docker socket proxy.
#
# Distroless Node, nonroot, no shell, no package.json, no npm install.
# server.js has zero runtime deps (Node built-in http only), so the image is
# just the runtime + one file — minimal supply-chain attack surface.
#
# Built via compose: context = ./socket-proxy (relative to docker/).
# The socket gid is injected at runtime via compose `group_add: ["${DOCKER_GID}"]`.

FROM gcr.io/distroless/nodejs20-debian12:nonroot

WORKDIR /app
COPY server.js ./

# 2375 is arbx-net-internal only (compose maps NO host port).
EXPOSE 2375

ENTRYPOINT ["/nodejs/bin/node", "/app/server.js"]
