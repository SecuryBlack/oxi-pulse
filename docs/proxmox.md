# OxiPulse on Proxmox VE

Proxmox VE is Debian, so OxiPulse installs on it with the `.deb` package: on the node itself, inside LXC containers and inside VMs. Every install sends the metrics of the machine it runs on, with its own `host.name`.

## Where to install it

| Where | What you get |
|---|---|
| **The node** | CPU, memory, swap and network of the physical host, every disk and ZFS dataset, including each container's `subvol-*`. One agent covers the node and the disk usage of all its containers. |
| **An LXC container** | The container's own view: its memory and CPU limits, its root disk, its network, its uptime and its OS. |
| **A VM** | A normal Linux or Windows host. Install as on any other server ([installation](../README.md#installation)). |

Install on the node to watch the hardware. Add it to the containers and VMs whose memory, CPU or disk you want to see from inside.

## On the node

As root in the node shell (Proxmox VE 8 and 9, amd64). Replace the version with the [latest one](https://github.com/securyblack/oxi-pulse/releases/latest):

```bash
curl -fsSLO https://github.com/securyblack/oxi-pulse/releases/download/v0.5.4/oxipulse_0.5.4_amd64.deb
apt install ./oxipulse_0.5.4_amd64.deb
nano /etc/oxipulse/config.toml      # set endpoint (and token if your backend needs one)
systemctl enable --now oxipulse
```

Updates come with the next `.deb`; the package turns off the agent's self-update.

What to expect from the node:

- **ZFS**: `rpool`, `rpool/data` and every `rpool/data/subvol-<id>-disk-<n>` appear as filesystems in `system.filesystem.usage`, each with its own size (the container's disk quota) and usage. Filter by `device` in your dashboards if you only want the pool.
- **Network**: besides the physical NICs and `vmbr*` bridges, Proxmox creates `veth<id>i<n>` (containers), `tap<id>i<n>` (VMs) and `fwbr`/`fwpr`/`fwln` (firewall) interfaces. They show the traffic of each guest. Filter by `device` to keep only `vmbr*` or the NICs.
- **VM disks** on LVM-thin or ZFS zvols are block devices, not mounted filesystems, so they do not appear. Install inside the VM to see its disk usage.

## Inside an LXC container

Works in unprivileged containers (the Proxmox default), no extra options needed. From the node, for container 101:

```bash
pct push 101 oxipulse_0.5.4_amd64.deb /root/oxipulse.deb
pct exec 101 -- apt install -y /root/oxipulse.deb
pct exec 101 -- sed -i 's|^endpoint = ""|endpoint = "http://collector.example.com:4317"|' /etc/oxipulse/config.toml
pct exec 101 -- systemctl enable --now oxipulse
```

Or download and install the `.deb` (Debian, Ubuntu) or `.rpm` (Rocky, Alma, Fedora) from the container's own shell, or use the install script on any other distribution.

Proxmox runs LXCFS, so the container sees its own resources and OxiPulse reports them:

| Metric | Inside the container |
|---|---|
| `system.memory.limit` | The container's memory limit, not the node's RAM |
| `system.cpu.logical.count` | The container's CPU cores limit |
| `system.uptime` | Time since the container started |
| `system.filesystem.usage` | The container's root disk and mount points |
| `system.network.io` | The container's interfaces |
| `system.paging.usage` | Only if the container has swap |
| `system.cpu.load_average.*` | The node's load average, as `uptime` shows inside the container |

Tested with an unprivileged Debian 12 container (512 MiB, 2 cores) under LXC with LXCFS: memory limit, core count, uptime, network and OS were the container's.

## Alpine and other containers without systemd

The `.deb`/`.rpm` packages need systemd. In containers without it (Alpine with OpenRC), run the binary from the [pre-built tarball](https://github.com/securyblack/oxi-pulse/releases/latest) under your init system, with the same `/etc/oxipulse/config.toml`.
