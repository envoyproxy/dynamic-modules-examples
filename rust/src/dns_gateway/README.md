# DNS Gateway

Envoy dynamic module filters that intercept DNS queries and route TCP connections to external
domains via virtual IP allocation.

![DNS Gateway diagram](diagram.png)

## Prerequisites

Requires iptables/nftables rules to redirect application traffic to Envoy:

- **DNS**: UDP port 53 redirected to Envoy's DNS listener (e.g. port 15053)
- **TCP**: Outbound connections redirected to Envoy's TCP listener (e.g. port 15001)

## How it works

1. **`dns_gateway`** (UDP listener filter) — Intercepts DNS queries. If the queried domain matches
   a configured pattern, allocates a virtual IP from a private subnet and responds with an A record.
   Caches the mapping from virtual IP to domain and metadata. Non-matching queries pass through.

2. **`cache_lookup`** (network filter) — On new TCP connections, looks up the destination virtual IP
   in the shared cache and sets the resolved domain and metadata as Envoy
   [filter state](https://www.envoyproxy.io/docs/envoy/latest/intro/arch_overview/advanced/data_sharing_between_filters#primitives)
   for use in routing.

```
 Application
     |  DNS query: "bucket-1.aws.com"
     v
 dns_gateway
     |  matches "*.aws.com", allocates 10.10.0.1, responds with A record
     v
 Application
     |  TCP connect to 10.10.0.1:443
     v
 cache_lookup
     |  resolves 10.10.0.1 -> domain="bucket-1.aws.com", metadata.cluster="aws"
     v
 tcp_proxy
     |  routes to upstream cluster using filter state
     v
 External service (bucket-1.aws.com)
```

## Filter state

`cache_lookup` sets the following keys, accessible via `%FILTER_STATE(...)%`:

| Key                                | Example                          |
| ---------------------------------- | -------------------------------- |
| `envoy.dns_gateway.domain`         | `bucket-1.aws.com`               |
| `envoy.dns_gateway.metadata.<key>` | value from matched domain config |

Usage in Envoy config:

- `%FILTER_STATE(envoy.dns_gateway.domain:PLAIN)%`
- `%FILTER_STATE(envoy.dns_gateway.metadata.cluster:PLAIN)%`
- `%FILTER_STATE(envoy.dns_gateway.metadata.auth_token:PLAIN)%`

## Domain matching

- **Exact**: `"example.com"` — matches only `example.com`
- **Wildcard**: `"*.aws.com"` — matches any subdomain (e.g. `bucket-1.aws.com`,
  `sub.api.aws.com`) but not `aws.com` itself

## Configuration reference

### `dns_gateway`

| Field                | Type    | Description                                                      |
| -------------------- | ------- | ---------------------------------------------------------------- |
| `base_ip`            | string  | Base IPv4 address for virtual IP allocation (e.g. `"10.10.0.0"`) |
| `prefix_len`         | integer | CIDR prefix length (1-32). A `/24` gives 256 IPs.                |
| `domains`            | array   | Domain matchers                                                  |
| `domains[].domain`   | string  | Exact (`"example.com"`) or wildcard (`"*.example.com"`) pattern  |
| `domains[].metadata` | object  | String key-value pairs exposed via filter state                  |

### `cache_lookup`

No configuration. Use `filter_config: {}`.

## Manual testing

End-to-end test with docker-compose.

Create the following files:

**docker-compose.yml**:

```yaml
services:
  envoy:
    image: <your-envoy-image>
    network_mode: host
    volumes:
      - ./envoy.yaml:/etc/envoy/envoy.yaml
    command: ["envoy", "-c", "/etc/envoy/envoy.yaml", "-l", "debug"]

upstream-1:
    image: python:3.12-slim
    network_mode: host
    volumes:
      - ./upstream_1.py:/app/server.py
    command: ["python3", "/app/server.py"]

  upstream-2:
    image: python:3.12-slim
    network_mode: host
    volumes:
      - ./upstream_2.py:/app/server.py
    command: ["python3", "/app/server.py"]
```

**upstream_1.py** (port 18001):

```python
from http.server import HTTPServer, BaseHTTPRequestHandler

class Handler(BaseHTTPRequestHandler):
    def do_CONNECT(self):
        print(f"\nCONNECT {self.path}")
        for key, value in self.headers.items():
            print(f"  {key}: {value}")

        self.send_response(200)
        self.end_headers()

        request = self.connection.recv(4096)

        body = f"cluster_1\nCONNECT: {self.path}\n"
        for key, value in self.headers.items():
            body += f"{key}: {value}\n"

        resp = f"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nX-Upstream: cluster_1\r\nContent-Length: {len(body)}\r\n\r\n{body}"
        self.connection.sendall(resp.encode())

HTTPServer(("0.0.0.0", 18001), Handler).serve_forever()
```

**upstream_2.py** (port 18002):

```python
from http.server import HTTPServer, BaseHTTPRequestHandler

class Handler(BaseHTTPRequestHandler):
    def do_CONNECT(self):
        print(f"\nCONNECT {self.path}")
        for key, value in self.headers.items():
            print(f"  {key}: {value}")

        self.send_response(200)
        self.end_headers()

        request = self.connection.recv(4096)

        body = f"cluster_2\nCONNECT: {self.path}\n"
        for key, value in self.headers.items():
            body += f"{key}: {value}\n"

        resp = f"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nX-Upstream: cluster_2\r\nContent-Length: {len(body)}\r\n\r\n{body}"
        self.connection.sendall(resp.encode())

HTTPServer(("0.0.0.0", 18002), Handler).serve_forever()
```

**envoy.yaml**:

```yaml
static_resources:
  listeners:
    - name: dns_listener
      address:
        socket_address:
          address: 0.0.0.0
          port_value: 15053
          protocol: UDP
      listener_filters:
        - name: envoy.filters.udp_listener.dynamic_modules
          typed_config:
            "@type": type.googleapis.com/envoy.extensions.filters.udp.dynamic_modules.v3.DynamicModuleUdpListenerFilter
            dynamic_module_config:
              name: connectivity_envoy_module
              do_not_close: true
            filter_name: dns_gateway
            filter_config:
              "@type": type.googleapis.com/google.protobuf.Struct
              value:
                base_ip: "10.10.0.0"
                prefix_len: 24
                domains:
                  - domain: "*.aws.com"
                    metadata:
                      cluster: cluster_1
                      auth_token: "abc123"
                  - domain: "example.com"
                    metadata:
                      cluster: cluster_2
                      auth_token: "def456"
        - name: envoy.filters.udp_listener.dns_filter
          typed_config:
            "@type": type.googleapis.com/envoy.extensions.filters.udp.dns_filter.v3.DnsFilterConfig
            stat_prefix: dns_fallback
            client_config:
              max_pending_lookups: 256
              dns_resolution_config:
                resolvers:
                  - socket_address:
                      protocol: TCP
                      address: 172.20.0.10
                      port_value: 53
                dns_resolver_options:
                  no_default_search_domain: true
                  use_tcp_for_dns_lookups: true
            server_config:
              inline_dns_table: {}

    - name: tcp_listener
      address:
        socket_address:
          address: 0.0.0.0
          port_value: 15001
      listener_filters:
        - name: envoy.filters.listener.original_dst
          typed_config:
            "@type": type.googleapis.com/envoy.extensions.filters.listener.original_dst.v3.OriginalDst
      filter_chains:
        - filters:
            - name: envoy.filters.network.dynamic_modules
              typed_config:
                "@type": type.googleapis.com/envoy.extensions.filters.network.dynamic_modules.v3.DynamicModuleNetworkFilter
                dynamic_module_config:
                  name: connectivity_envoy_module
                  do_not_close: true
                filter_name: cache_lookup
                filter_config: {}
            # Setting an upstream cluster directly in the TCP proxy tunneling config with FILTER_STATE(...)
            # is not supported. Instead, write the value of FILTER_STATE(...) to 'envoy.tcp_proxy.cluster'
            - name: envoy.filters.network.set_filter_state
              typed_config:
                "@type": type.googleapis.com/envoy.extensions.filters.network.set_filter_state.v3.Config
                on_new_connection:
                  - object_key: envoy.tcp_proxy.cluster
                    format_string:
                      text_format_source:
                        inline_string: "%FILTER_STATE(envoy.dns_gateway.metadata.cluster:PLAIN)%"
            - name: envoy.filters.network.tcp_proxy
              typed_config:
                "@type": type.googleapis.com/envoy.extensions.filters.network.tcp_proxy.v3.TcpProxy
                stat_prefix: egress
                cluster: default
                tunneling_config:
                  hostname: "%FILTER_STATE(envoy.dns_gateway.domain:PLAIN)%"
                  headers_to_add:
                    - header:
                        key: "X-Auth-Token"
                        value: "%FILTER_STATE(envoy.dns_gateway.metadata.auth_token:PLAIN)%"

  clusters:
    - name: cluster_1
      type: STATIC
      load_assignment:
        cluster_name: cluster_1
        endpoints:
          - lb_endpoints:
              - endpoint:
                  address:
                    socket_address:
                      address: 127.0.0.1
                      port_value: 18001

    - name: cluster_2
      type: STATIC
      load_assignment:
        cluster_name: cluster_2
        endpoints:
          - lb_endpoints:
              - endpoint:
                  address:
                    socket_address:
                      address: 127.0.0.1
                      port_value: 18002
```

### 2. Start

```bash
docker-compose up
```

### 3. Set up iptables redirect

```bash
# Redirect DNS (UDP 53) to Envoy's DNS listener
sudo iptables -t nat -A OUTPUT -p udp --dport 53 -j DNAT --to-destination 127.0.0.1:15053

# Redirect TCP to virtual IPs (10.10.0.0/24) to Envoy's TCP listener
sudo iptables -t nat -A OUTPUT -p tcp -d 10.10.0.0/24 -j DNAT --to-destination 127.0.0.1:15001
```

### 4. Test

```bash
# Will allocate sequentially increasing virtual IPs
dig one.s3.aws.com
dig two.s3.aws.com
dig example.com

# Unmatched domain, will defer to external DNS
dig github.com

# Will reach cluster_1
curl http://s3.aws.com./

# Will reach cluster_2
curl http://example.com./

# See logs for upstream-1 and upstream-2
docker-compose logs upstream-1
docker-compose logs upstream-2
```

### 5. Clean up iptables

```bash
sudo iptables -t nat -D OUTPUT -p udp --dport 53 -j DNAT --to-destination 127.0.0.1:15053
sudo iptables -t nat -D OUTPUT -p tcp -d 10.10.0.0/24 -j DNAT --to-destination 127.0.0.1:15001
```
