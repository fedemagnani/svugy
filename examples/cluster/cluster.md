---
title: A Kubernetes cluster
subtitle: click a namespace, then a deployment
styles:
  pod: (color=#3478f6, caption=pod)
  store: (shape=cylinder, color=#d5dcea)
  probe: (line=dotted)
---

```description
Requests enter through [the ingress](#ingress), reach [web](#web) or [api](#api), and end in [Postgres](#postgres). Click a namespace to open it, then a deployment to see its pods.
```

## Components
- users: Users (caption=browsers · apps)
- cluster: Cluster (caption=prod · eu-west-1)
  - ingress: Ingress (shape=slab, caption=TLS · routing)
  - shop: shop (caption=namespace, layout=column)

    ```description
    The stateless services. Each deployment scales its pods on CPU.
    ```

    - web: web (caption=Deployment, layout=row)
      - web-1 (pod)
      - web-2 (pod)
      - web-3 (pod)
    - api: api (caption=Deployment, layout=row)
      - api-1 (pod)
      - api-2 (pod)
  - data: data (caption=namespace)
    - postgres: Postgres (caption=StatefulSet, layout=row)
      - pg-0: pg-0 (store, caption=primary)
      - pg-1: pg-1 (store, caption=replica)
    - redis: Redis (store, caption=sessions · cache)
  - observability: observability (caption=namespace)
    - prometheus: Prometheus (caption=scrapes metrics)
    - grafana: Grafana (caption=dashboards)

## Edges
- users -> ingress: HTTPS (traveler=envelope)
- ingress -> web: routes /
- ingress -> api: routes /api
- web -> api: calls (traveler=document)
- api -> pg-0: writes (traveler=document)
- pg-0 -> pg-1 (probe)
- api -> redis: reads sessions (traveler=key)
- prometheus -> web: scrapes (probe)
- prometheus -> api: scrapes (probe)
- grafana -> prometheus: queries
