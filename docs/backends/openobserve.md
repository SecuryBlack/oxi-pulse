# Send host metrics to OpenObserve

OpenObserve receives OTLP/gRPC on port 5081. It needs Basic auth and the organization as headers.

## 1. Start OpenObserve

```bash
docker run -d --name openobserve --restart unless-stopped \
  -p 5080:5080 -p 5081:5081 \
  -v openobserve-data:/data -e ZO_DATA_DIR=/data \
  -e ZO_ROOT_USER_EMAIL=root@example.com -e ZO_ROOT_USER_PASSWORD='<password>' \
  public.ecr.aws/zinclabs/openobserve:latest
```

5080 is the UI and HTTP API; 5081 is gRPC, which the OpenObserve quickstart does not publish.

## 2. Point OxiPulse at it

The `authorization` header is `Basic` followed by `email:password` in base64:

```bash
printf '%s' 'root@example.com:<password>' | base64
```

In `/etc/oxipulse/config.toml` (Windows: `C:\ProgramData\oxipulse\config.toml`):

```toml
endpoint = "http://<openobserve-host>:5081"

[headers]
authorization = "Basic <base64 from above>"
organization = "default"
```

Then `sudo systemctl restart oxipulse`. With Docker, pass the same as `-e OXIPULSE_HEADERS="authorization=Basic <base64>,organization=default"`.

Better than the root user: create a service account in OpenObserve (**IAM** → **Service Accounts**) and use its email and token.

## 3. See the metrics

Each metric is its own stream, with dots turned into underscores: `system_cpu_utilization`, `system_memory_usage`, and so on. Open **Metrics**, pick a stream and filter or group by `host_name`.

To check from the command line:

```bash
curl -u 'root@example.com:<password>' \
  "http://<openobserve-host>:5080/api/default/streams?type=metrics&fetchSchema=false"
```
