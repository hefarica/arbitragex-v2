# Evidence Index

| Phase | Source | Operation | Result | Exit | Duration ms | Note |
|---|---|---|---|---|---|---|
| Repository | local | open-existing-repository | C:\Users\HFRC\Desktop\arbitragex-v2-main (17) | 0 |  | No checkout, reset, pull, clean or write operation performed. |
| Git metadata | local | git status --porcelain=v1 | M .claude/settings.json<br> M .claude/settings.local.json | 0 | 522.0 |  |
| Git metadata | local | git rev-parse HEAD | aa28fee4ff421bc907b16358418cb1f5505887c4 | 0 | 28.5 |  |
| Git metadata | local | git branch --show-current | main | 0 | 31.4 |  |
| Git metadata | local | git remote get-url origin | https://github.com/hefarica/arbitragex-v2.git | 0 | 28.8 |  |
| Git metadata | local | git log -1 '--format=%H\|%cI\|%s' | aa28fee4ff421bc907b16358418cb1f5505887c4\|2026-07-15T22:29:32-05:00\|feat(live-testnet-v2): SSE endpoint, executor stub, E2E token from env, CI secrets | 0 | 31.6 |  |
| Repository inventory | local | walk-repository | 46144 files, 11427 directories, 56 routes | 0 | 10904.4 |  |
| Compose | local | parse .claude/worktrees/agent-a7a72813e0989b73b/.devcontainer/docker-compose.yml | 1 services | 0 |  |  |
| Compose | local | parse .claude/worktrees/agent-a7a72813e0989b73b/arbitragex-v2-main/.devcontainer/docker-compose.yml | 1 services | 0 |  |  |
| Compose | local | parse .claude/worktrees/agent-a7a72813e0989b73b/arbitragex-v2-main/docker-compose.edge.yml | 7 services | 0 |  |  |
| Compose | local | parse .claude/worktrees/agent-a7a72813e0989b73b/arbitragex-v2-main/docker/compose.dev.yml | 23 services | 0 |  |  |
| Compose | local | parse .claude/worktrees/agent-a7a72813e0989b73b/arbitragex-v2-main/docker/compose.loopback.override.yml | 11 services | 0 |  |  |
| Compose | local | parse .claude/worktrees/agent-a7a72813e0989b73b/arbitragex-v2-main/docker/compose.noports.override.yml | 13 services | 0 |  |  |
| Compose | local | parse .claude/worktrees/agent-a7a72813e0989b73b/arbitragex-v2-main/docker/compose.prod.yml | 21 services | 0 |  |  |
| Compose | local | parse .claude/worktrees/agent-a7a72813e0989b73b/arbitragex-v2-main/docker/compose.staging.override.yml | 6 services | 0 |  |  |
| Compose | local | parse .claude/worktrees/agent-a7a72813e0989b73b/arbitragex-v2-main/docs/blueprints/enterprise_package/docker-compose.yml | 3 services | 0 |  |  |
| Compose | local | parse .claude/worktrees/agent-a7a72813e0989b73b/docker-compose.edge.yml | 7 services | 0 |  |  |
| Compose | local | parse .claude/worktrees/agent-a7a72813e0989b73b/docker/compose.dev.yml | 23 services | 0 |  |  |
| Compose | local | parse .claude/worktrees/agent-a7a72813e0989b73b/docker/compose.loopback.override.yml | 11 services | 0 |  |  |
| Compose | local | parse .claude/worktrees/agent-a7a72813e0989b73b/docker/compose.noports.override.yml | 13 services | 0 |  |  |
| Compose | local | parse .claude/worktrees/agent-a7a72813e0989b73b/docker/compose.prod.yml | 22 services | 0 |  |  |
| Compose | local | parse .claude/worktrees/agent-a7a72813e0989b73b/docker/compose.staging.override.yml | 6 services | 0 |  |  |
| Compose | local | parse .claude/worktrees/agent-a7a72813e0989b73b/docs/blueprints/enterprise_package/docker-compose.yml | 3 services | 0 |  |  |
| Compose | local | parse .claude/worktrees/agent-ab798a43a6ebcddca/.devcontainer/docker-compose.yml | 1 services | 0 |  |  |
| Compose | local | parse .claude/worktrees/agent-ab798a43a6ebcddca/arbitragex-v2-main/.devcontainer/docker-compose.yml | 1 services | 0 |  |  |
| Compose | local | parse .claude/worktrees/agent-ab798a43a6ebcddca/arbitragex-v2-main/docker-compose.edge.yml | 7 services | 0 |  |  |
| Compose | local | parse .claude/worktrees/agent-ab798a43a6ebcddca/arbitragex-v2-main/docker/compose.dev.yml | 23 services | 0 |  |  |
| Compose | local | parse .claude/worktrees/agent-ab798a43a6ebcddca/arbitragex-v2-main/docker/compose.loopback.override.yml | 11 services | 0 |  |  |
| Compose | local | parse .claude/worktrees/agent-ab798a43a6ebcddca/arbitragex-v2-main/docker/compose.noports.override.yml | 13 services | 0 |  |  |
| Compose | local | parse .claude/worktrees/agent-ab798a43a6ebcddca/arbitragex-v2-main/docker/compose.prod.yml | 21 services | 0 |  |  |
| Compose | local | parse .claude/worktrees/agent-ab798a43a6ebcddca/arbitragex-v2-main/docker/compose.staging.override.yml | 6 services | 0 |  |  |
| Compose | local | parse .claude/worktrees/agent-ab798a43a6ebcddca/arbitragex-v2-main/docs/blueprints/enterprise_package/docker-compose.yml | 3 services | 0 |  |  |
| Compose | local | parse .claude/worktrees/agent-ab798a43a6ebcddca/docker-compose.edge.yml | 7 services | 0 |  |  |
| Compose | local | parse .claude/worktrees/agent-ab798a43a6ebcddca/docker/compose.dev.yml | 23 services | 0 |  |  |
| Compose | local | parse .claude/worktrees/agent-ab798a43a6ebcddca/docker/compose.loopback.override.yml | 11 services | 0 |  |  |
| Compose | local | parse .claude/worktrees/agent-ab798a43a6ebcddca/docker/compose.noports.override.yml | 13 services | 0 |  |  |
| Compose | local | parse .claude/worktrees/agent-ab798a43a6ebcddca/docker/compose.prod.yml | 22 services | 0 |  |  |
| Compose | local | parse .claude/worktrees/agent-ab798a43a6ebcddca/docker/compose.staging.override.yml | 6 services | 0 |  |  |
| Compose | local | parse .claude/worktrees/agent-ab798a43a6ebcddca/docs/blueprints/enterprise_package/docker-compose.yml | 3 services | 0 |  |  |
| Compose | local | parse .claude/worktrees/agent-ac4deafb48d86bfc4/.devcontainer/docker-compose.yml | 1 services | 0 |  |  |
| Compose | local | parse .claude/worktrees/agent-ac4deafb48d86bfc4/arbitragex-v2-main/.devcontainer/docker-compose.yml | 1 services | 0 |  |  |
| Compose | local | parse .claude/worktrees/agent-ac4deafb48d86bfc4/arbitragex-v2-main/docker-compose.edge.yml | 7 services | 0 |  |  |
| Compose | local | parse .claude/worktrees/agent-ac4deafb48d86bfc4/arbitragex-v2-main/docker/compose.dev.yml | 23 services | 0 |  |  |
| Compose | local | parse .claude/worktrees/agent-ac4deafb48d86bfc4/arbitragex-v2-main/docker/compose.loopback.override.yml | 11 services | 0 |  |  |
| Compose | local | parse .claude/worktrees/agent-ac4deafb48d86bfc4/arbitragex-v2-main/docker/compose.noports.override.yml | 13 services | 0 |  |  |
| Compose | local | parse .claude/worktrees/agent-ac4deafb48d86bfc4/arbitragex-v2-main/docker/compose.prod.yml | 21 services | 0 |  |  |
| Compose | local | parse .claude/worktrees/agent-ac4deafb48d86bfc4/arbitragex-v2-main/docker/compose.staging.override.yml | 6 services | 0 |  |  |
| Compose | local | parse .claude/worktrees/agent-ac4deafb48d86bfc4/arbitragex-v2-main/docs/blueprints/enterprise_package/docker-compose.yml | 3 services | 0 |  |  |
| Compose | local | parse .claude/worktrees/agent-ac4deafb48d86bfc4/docker-compose.edge.yml | 7 services | 0 |  |  |
| Compose | local | parse .claude/worktrees/agent-ac4deafb48d86bfc4/docker/compose.dev.yml | 23 services | 0 |  |  |
| Compose | local | parse .claude/worktrees/agent-ac4deafb48d86bfc4/docker/compose.loopback.override.yml | 11 services | 0 |  |  |
| Compose | local | parse .claude/worktrees/agent-ac4deafb48d86bfc4/docker/compose.noports.override.yml | 13 services | 0 |  |  |
| Compose | local | parse .claude/worktrees/agent-ac4deafb48d86bfc4/docker/compose.prod.yml | 22 services | 0 |  |  |
| Compose | local | parse .claude/worktrees/agent-ac4deafb48d86bfc4/docker/compose.staging.override.yml | 6 services | 0 |  |  |
| Compose | local | parse .claude/worktrees/agent-ac4deafb48d86bfc4/docs/blueprints/enterprise_package/docker-compose.yml | 3 services | 0 |  |  |
| Compose | local | parse .claude/worktrees/pr4-opportunities/.devcontainer/docker-compose.yml | 1 services | 0 |  |  |
| Compose | local | parse .claude/worktrees/pr4-opportunities/docker-compose.edge.yml | 7 services | 0 |  |  |
| Compose | local | parse .claude/worktrees/pr4-opportunities/docker/compose.dev.yml | 23 services | 0 |  |  |
| Compose | local | parse .claude/worktrees/pr4-opportunities/docker/compose.loopback.override.yml | 11 services | 0 |  |  |
| Compose | local | parse .claude/worktrees/pr4-opportunities/docker/compose.noports.override.yml | 13 services | 0 |  |  |
| Compose | local | parse .claude/worktrees/pr4-opportunities/docker/compose.prod.yml | 22 services | 0 |  |  |
| Compose | local | parse .claude/worktrees/pr4-opportunities/docker/compose.staging.override.yml | 6 services | 0 |  |  |
| Compose | local | parse .claude/worktrees/pr4-opportunities/docs/blueprints/enterprise_package/docker-compose.yml | 3 services | 0 |  |  |
| Compose | local | parse .claude/worktrees/wf_4d31ee79-009-1/.devcontainer/docker-compose.yml | 1 services | 0 |  |  |
| Compose | local | parse .claude/worktrees/wf_4d31ee79-009-1/docker-compose.edge.yml | 7 services | 0 |  |  |
| Compose | local | parse .claude/worktrees/wf_4d31ee79-009-1/docker/compose.dev.yml | 23 services | 0 |  |  |
| Compose | local | parse .claude/worktrees/wf_4d31ee79-009-1/docker/compose.loopback.override.yml | 11 services | 0 |  |  |
| Compose | local | parse .claude/worktrees/wf_4d31ee79-009-1/docker/compose.noports.override.yml | 13 services | 0 |  |  |
| Compose | local | parse .claude/worktrees/wf_4d31ee79-009-1/docker/compose.prod.yml | 22 services | 0 |  |  |
| Compose | local | parse .claude/worktrees/wf_4d31ee79-009-1/docker/compose.staging.override.yml | 6 services | 0 |  |  |
| Compose | local | parse .claude/worktrees/wf_4d31ee79-009-1/docs/blueprints/enterprise_package/docker-compose.yml | 3 services | 0 |  |  |
| Compose | local | parse .devcontainer/docker-compose.yml | 1 services | 0 |  |  |
| Compose | local | parse docker-compose.edge.yml | 7 services | 0 |  |  |
| Compose | local | parse docker-compose.yml | 3 services | 0 |  |  |
| Compose | local | parse docker/compose.dev.yml | 23 services | 0 |  |  |
| Compose | local | parse docker/compose.hotpath-test.yml | 7 services | 0 |  |  |
| Compose | local | parse docker/compose.loopback.override.yml | 11 services | 0 |  |  |
| Compose | local | parse docker/compose.noports.override.yml | 13 services | 0 |  |  |
| Compose | local | parse docker/compose.prod.yml | 22 services | 0 |  |  |
| Compose | local | parse docker/compose.staging.override.yml | 6 services | 0 |  |  |
| Compose | local | parse docs/blueprints/enterprise_package/docker-compose.yml | 3 services | 0 |  |  |
| VPS metadata | ssh:root@195.201.235.70 | ssh -p 22 root@195.201.235.70 <READ_ONLY_COMMAND> | OS	Ubuntu 24.04<br>UNAME	Linux 6.8.0-90-generic x86_64 GNU/Linux<br>UPTIME	up 10 weeks, 5 days, 4 hours, 19 minutes<br>DISK	/dev/sda1        157197504 122356708  28396176      82% /<br>MEMORY	16372305920 3447930880 12924375040<br>BRANCH	DIRTY	0<br>DOCKER	29.4.2<br>COMPOSE	5.1.3 | 0 | 3095.1 | Remote command validated against read-only denylist. |
| Docker inventory | ssh:root@195.201.235.70 | ssh -p 22 root@195.201.235.70 <READ_ONLY_COMMAND> | nt. Designed for performance and the S3 API, it is 100% open-source. MinIO is ideal for large, private cloud environments with stringent security requirements and delivers mission-critical availability across a diverse range of workloads.,distribution-scope=public,io.buildah.version=1.29.0,io.k8s.description=Very small image which doesn't install the package manager.,io.k8s.display-name=Ubi9-micro,io.openshift.expose-services=,maintainer=MinIO Inc \u003cdev@min.io\u003e,name=MinIO,release=RELEASE.2024-11-07T00-52-20Z,summary=MinIO is a High Performance Object Storage, API compatible with Amazon S3 cloud storage service.,url=https://access.redhat.com/containers/#/registry.access.redhat.com/ubi9/ubi-micro/images/9.4-15,vcs-ref=cd5996c9b8b99b546584696465f8f39ec682078c,vcs-type=git,vendor=MinIO Inc \u003cdev@min.io\u003e,version=RELEASE.2024-11-07T00-52-20Z","LocalVolumes":"1","Mounts":"arbitragex-v2_minio_data","Names":"arbitragex-v2-minio-1","Networks":"arbitragex-v2_arbx-net","Platform":{"architecture":"amd64","os":"linux"},"Ports":"127.0.0.1:9000-9001-\u003e9000-9001/tcp","RunningFor":"2 months ago","Size":"57.3kB (virtual 168MB)","State":"running","Status":"Up 2 months (healthy)"} | 0 | 2816.0 | Remote command validated against read-only denylist. |
| Docker safe inspect | ssh:root@195.201.235.70 | ssh -p 22 root@195.201.235.70 <READ_ONLY_COMMAND> | .152025642Z","ExitCode":0,"Output":"  % Total    % Received % Xferd  Average Speed   Time    Time     Time  Current\n                                 Dload  Upload   Total   Spent    Left  Speed\n\r  0     0    0     0    0     0      0      0 --:--:-- --:--:-- --:--:--     0\r  0     0    0     0    0     0      0      0 --:--:-- --:--:-- --:--:--     0\n"}]}}	{"arbitragex-v2_arbx-net":{"IPAMConfig":null,"Links":null,"Aliases":["arbitragex-v2-minio-1","minio"],"DriverOpts":null,"GwPriority":0,"NetworkID":"95606e6a95f5252995b124d516a0b91f510f2a3e4e2beaad59ecb9dd464600f4","EndpointID":"7bec42b453d6efb370a3e022b9cd20eeaedd2f1007b05cbcdfb710d18cba405d","Gateway":"172.18.0.1","IPAddress":"172.18.0.4","MacAddress":"62:86:33:2c:65:45","IPPrefixLen":16,"IPv6Gateway":"","GlobalIPv6Address":"","GlobalIPv6PrefixLen":0,"DNSNames":["arbitragex-v2-minio-1","minio","1ed18b4815b3"]}}	[{"Type":"volume","Name":"arbitragex-v2_minio_data","Source":"/var/lib/docker/volumes/arbitragex-v2_minio_data/_data","Destination":"/data","Driver":"local","Mode":"rw","RW":true,"Propagation":""}]	sha256:ac591851803a79aee64bc37f66d77c56b0a4b6e12d9e5356380f4105510f2332	minio	/opt/arbitragex-v2/docker/compose.prod.yml | 0 | 5489.0 | Remote command validated against read-only denylist. |
| Docker networks | ssh:root@195.201.235.70 | ssh -p 22 root@195.201.235.70 <READ_ONLY_COMMAND> | {"CreatedAt":"2026-05-02 16:05:54.630710119 +0000 UTC","Driver":"bridge","ID":"95606e6a95f5","IPv4":"true","IPv6":"false","Internal":"false","Labels":"com.docker.compose.config-hash=201d2540c9b989951b58978ccba209e620fe8a13e1c2f1e4b1a810dd0b6bafcb,com.docker.compose.network=arbx-net,com.docker.compose.project=arbitragex-v2,com.docker.compose.version=5.1.3","Name":"arbitragex-v2_arbx-net","Scope":"local"}<br>{"CreatedAt":"2026-05-02 14:04:32.547781691 +0000 UTC","Driver":"bridge","ID":"44aa2498fe70","IPv4":"true","IPv6":"false","Internal":"false","Labels":"","Name":"bridge","Scope":"local"}<br>{"CreatedAt":"2026-05-02 14:04:32.542709805 +0000 UTC","Driver":"host","ID":"589ddb4aa6de","IPv4":"true","IPv6":"false","Internal":"false","Labels":"","Name":"host","Scope":"local"}<br>{"CreatedAt":"2026-05-02 14:04:32.536102674 +0000 UTC","Driver":"null","ID":"9fd10542f8a9","IPv4":"true","IPv6":"false","Internal":"false","Labels":"","Name":"none","Scope":"local"} | 0 | 2669.7 | Remote command validated against read-only denylist. |
| Docker volumes | ssh:root@195.201.235.70 | ssh -p 22 root@195.201.235.70 <READ_ONLY_COMMAND> | ocal","Size":"N/A","Status":"N/A"}<br>{"Availability":"N/A","Driver":"local","Group":"N/A","Labels":"com.docker.compose.config-hash=5e2115d8e7d7852f3b103529a6310df50a856a559059180db172a6d17b673852,com.docker.compose.project=arbx-frontend-devcontainer,com.docker.compose.version=5.1.3,com.docker.compose.volume=arbx_node_modules","Links":"N/A","Mountpoint":"/var/lib/docker/volumes/arbx-frontend-devcontainer_arbx_node_modules/_data","Name":"arbx-frontend-devcontainer_arbx_node_modules","Scope":"local","Size":"N/A","Status":"N/A"}<br>{"Availability":"N/A","Driver":"local","Group":"N/A","Labels":"com.docker.volume.anonymous=","Links":"N/A","Mountpoint":"/var/lib/docker/volumes/bf8060edef13940abcb5b33fb8f0603ee0bd31399b42926534df7893037ee9a2/_data","Name":"bf8060edef13940abcb5b33fb8f0603ee0bd31399b42926534df7893037ee9a2","Scope":"local","Size":"N/A","Status":"N/A"}<br>{"Availability":"N/A","Driver":"local","Group":"N/A","Labels":"com.docker.volume.anonymous=","Links":"N/A","Mountpoint":"/var/lib/docker/volumes/dd9ee63fd7de51045d4ecfb8f3db262ea5a8b7175e22ee17040fdb72648d9a8b/_data","Name":"dd9ee63fd7de51045d4ecfb8f3db262ea5a8b7175e22ee17040fdb72648d9a8b","Scope":"local","Size":"N/A","Status":"N/A"} | 0 | 2548.9 | Remote command validated against read-only denylist. |
| VPS file parity | ssh:root@195.201.235.70 | ssh -p 22 root@195.201.235.70 <READ_ONLY_COMMAND> | bash: -c: line 1: unexpected EOF while looking for matching `'' | 2 | 2815.0 | Remote command validated against read-only denylist. |