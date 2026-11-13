---
styles:
  pod: (color=#3478f6, caption=pod)
  store: (shape=cylinder, color=#d5dcea)
  probe: (line=dotted)
---

## Components
- users: Users (caption=browsers · apps)
- cluster: Cluster (caption=prod · eu-west-1)
  - ingress: Ingress (shape=slab, caption=TLS · routing)
  - shop: shop (caption=namespace)

    ```description
    The stateless services. Each deployment scales its pods on CPU.
    ```

    - web: web (caption=Deployment)
      - web-1 (pod)
      - web-2 (pod)
      - web-3 (pod)
    - api: api (caption=Deployment)

      ````description
      Two replicas behind the `api` Service; a pod receives traffic only while `/ready` answers.
      ```yaml
      apiVersion: apps/v1
      kind: Deployment
      metadata:
        name: api
        namespace: shop
      spec:
        replicas: 2
        selector:
          matchLabels: { app: api }
        template:
          metadata:
            labels: { app: api }
          spec:
            containers:
              - name: api
                image: shop/api:1.4.2
                readinessProbe:
                  httpGet: { path: /ready, port: 8080 }
      ```
      ````

      - api-1 (pod)
      - api-2 (pod)
  - data: data (caption=namespace)
    - postgres: Postgres (caption=StatefulSet)
      - pg-0: pg-0 (store, caption=primary)
      - pg-1: pg-1 (store, caption=replica)
    - redis: Redis (store, caption=sessions · cache)
  - observability: observability (caption=namespace)
    - prometheus: Prometheus (caption=scrapes metrics)
    - grafana: Grafana (caption=dashboards)

## Workflows
### request: A request is served (subtitle=from the browser to Postgres)
How one HTTPS request crosses the cluster. Click a namespace to open it.

#### enter: The ingress terminates TLS
[The ingress](#ingress) is the only public endpoint: it decrypts the request and picks a route.
- users -> ingress: HTTPS (traveler=envelope)

#### route: The ingress routes by path
Pages go to [web](#web), `/api` calls from apps go straight to [api](#api).
- ingress -> web: routes /
- ingress -> api: routes /api

#### call: web calls the API
[web](#web) renders the page from the data [api](#api) returns.
- web -> api: calls (traveler=document)

#### state: The API reads the session and writes the order
Sessions live in [Redis](#redis); orders go to [the primary](#pg-0), the only pod that accepts writes.
- api -> redis: reads sessions (traveler=key)
- api -> pg-0: writes (traveler=document)

  ````description
  One transaction per order:
  ```sql
  INSERT INTO orders (id, user_id, total_cents)
  VALUES ($1, $2, $3)
  RETURNING created_at;
  ```
  ````

#### replicate: The replica follows the primary
[pg-1](#pg-1) streams every write from [pg-0](#pg-0), ready to take over if it fails.
- pg-0 -> pg-1: streams the WAL (line=dotted)

### probe: A pod fails its readiness probe (subtitle=a Service routes only to ready pods)
The kubelet probes every pod; a pod that stops answering leaves its Service until it recovers. Click [web](#web) to see its pods.

- web (layout=row)

#### spread: The ingress spreads requests over web's pods
The `web` Service balances across all three pods.
- ingress -> web-1: routes /
- ingress -> web-2: routes /
- ingress -> web-3: routes /

#### fail: web-2 fails its probe
[web-2](#web-2) stops answering its readiness probe, so the Service drops it. Nothing restarts it unless its liveness probe fails too.
- web-2

#### reroute: The other pods absorb the traffic
[web-1](#web-1) and [web-3](#web-3) take every request until [web-2](#web-2) passes its probe again.
- ingress -> web-1[0]
- ingress -> web-3[0]

### metrics: Prometheus watches the services (subtitle="scrape, store, chart")
[Prometheus](#prometheus) pulls metrics from the deployments; [Grafana](#grafana) only reads Prometheus. Click a deployment to see the pods it scrapes.

#### scrape: Prometheus scrapes each deployment
On every interval Prometheus reads each pod's `/metrics` endpoint and stores the samples.
- prometheus -> web: scrapes (probe)
- prometheus -> api: scrapes (probe)
- web-1
- web-2
- web-3
- api-1
- api-2

#### chart: Grafana charts the series
Each dashboard panel is a query to [Prometheus](#prometheus).
- grafana -> prometheus: queries
