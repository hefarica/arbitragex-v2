# SKILL: Cloud-Native Infrastructure & Kubernetes for Trading
**Level:** PhD Cloud Computing | SRE/DevOps Grandmaster
**Specialty:** Container Orchestration & Auto-Scaling

## AGENT DIRECTIVE
La nube es tu data center infinito. Kubernetes es tu sistema operativo. **Zero downtime** es la meta.

## CORE KNOWLEDGE
- **Containers:** Docker, containerd
- **Kubernetes:** Pods, Services, Deployments, HPA
- **Service Mesh:** Istio, Linkerd
- **GitOps:** ArgoCD, Flux

## KUBERNETES DEPLOYMENT
```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: hft-strategy-alpha
spec:
  replicas: 3
  template:
    spec:
      nodeSelector:
        workload-type: hft
      containers:
        - name: strategy
          image: trading/hft-alpha:v2.3.1
          resources:
            requests: {memory: "4Gi", cpu: "2000m"}
            limits: {memory: "8Gi", cpu: "4000m"}
          livenessProbe:
            httpGet: {path: /health, port: 8080}
            periodSeconds: 5
```

## AUTO-SCALING
```yaml
apiVersion: autoscaling/v2
kind: HorizontalPodAutoscaler
spec:
  scaleTargetRef: {name: hft-alpha}
  minReplicas: 2
  maxReplicas: 10
  metrics:
    - type: Resource
      resource: {name: cpu, target: {averageUtilization: 70}}
```

## DISASTER RECOVERY
```
Multi-region: us-east-1 (primary), eu-west-1 (secondary)
RTO: < 5 minutos
RPO: < 1 minuto
etcd snapshots cada 6 horas
Cross-region replication de persistent volumes
```
