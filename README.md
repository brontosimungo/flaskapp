# 🌊 NockPool by SWPSCo

<img width="624" height="206" alt="Nockpool logo" src="https://github.com/user-attachments/assets/cab9f6bd-0279-4d17-9c90-485954464394" />


#### **Important note**: We recommend installing via the [NockPool Launcher](https://github.com/SWPSCO/nockpool-miner-launcher) application, which will automatically keep your miner up to date!

This repo includes code and binaries necessary to participate in [Nockpool](https://nockpool.com), the premier [Nockchain](https://nockchain.org) mining pool, with your Linux or Apple Silicon machines.

### Downloads

👉 Get the latest prebuilt bundle from **GitHub Releases → Latest** [Go to bwming github](https://github.com/bwmining/nockpool-addons/releases).

- Example asset name: `nockpool-addons_v0.1.0_linux-x86_64_zen5.tar.gz`
- Inside the tarball:
  - `miner.jam`
  - `libzkvm_jetpack.so`
  - `README.md`
  - `LICENSE`

Verify integrity:

```bash
sha256sum -c SHA256SUMS
```

---

### Install

You can download the prebuilt binaries in the release tab. The Linux bins are SLSA3 attested -- we recommend [verifying](https://github.com/slsa-framework/slsa-verifier).

### Run

### 2) Place the bundle next to the miner (or anywhere you prefer)

```bash
cd nockpool-miner # OR where you installed nockpool-miner

mkdir -p addons && cd addons
# download release tar.gz here, then:
tar -xzf nockpool-addons_v0.1.3_linux-amd_zen4_x86_64.tar.gz
ls -1
# → miner.jam, libzkvm_jetpack.so, README.md, LICENSE
```

### 3) Run with the provided helper script (recommended)

Use `nockpool-run.sh` (provided in nockpool-miner) to launch the miner with sane defaults.


> NOTE: ONLY if you didn't extract file next to the miner binary.
The script exports `MINER_JAM_PATH` where MINER_JAM_PATH is the full path to the miner.jam file and ensures `LIB_DIR` contains the directory of `libzkvm_jetpack.so` 


```bash
chmod +x ./nockpool-run.sh

# HELP for full options possibility
./nockpool-run.sh

# start in foreground
./nockpool-run.sh start \
  --max-threads 16 --jam addons/miner.jam --lib-dir ./addons \
  --account-token nockacct_************************ \
  

# start in background (daemon)
./nockpool-run.sh start --daemon \
  --max-threads 16 --jam ./addons/miner.jam --lib-dir ./addons \
  --account-token nockacct_************************


# check status
./nockpool-run.sh status

# follow logs
./nockpool-run.sh logs

# stop
./nockpool-run.sh stop
```

---

## Compatibility / ABI

The `miner.jam` and `libzkvm_jetpack.so` are built against specific upstream commits/ABI. If versions drift, the miner may refuse to start, or undefined behavior may occur.

- **CPU**: current builds target AMD Zen 4 / Zen 5 (Ryzen 7xxx and 9xxx). Other CPUs may work but are not supported in this bundle yet.
- **OS**: Linux x86_64 (glibc toolchains). macOS builds can be added later.
- **Miner**: tested with the upstream `nockpool-miner` at pinned commits (see release notes).

Release notes include the exact upstream commit hashes used for the build.

---

## Environment variables (script)

- `ACCESS_TOKEN` — default pool account token (can be overridden by CLI `--account-token`)
- `MAX_THREADS` — default thread count (overridden by `--threads`)
- `MINER_JAM_PATH` — path to `miner.jam` (overridden by `--jam`)
- `LIB_DIR` — directory containing `libzkvm_jetpack.so` (overridden by `--lib-dir`)
- `MINER_BIN` — path to the miner binary (default: `./nockpool-miner`)
- `PIDFILE` — PID file path (default: `.nockpool-miner.pid`)
- `LOG_FILE` — log file path (default: `nockpool.log`)
- `EXTRA_ARGS` — extra flags passed verbatim to the miner

---

## FAQ

#### How do I get started?

1. Create an account at [nockpool.com](https://nockpool.com)
2. Generate an **account token** in your dashboard (recommended)
3. Use the account token with `--account-token` flag
4. The miner will automatically create and manage device keys for you

#### What's the difference between account tokens and device keys?

- **Account tokens** (`nockacct_*`): Long-lived tokens that can create and manage multiple device keys.
- **Device keys** (`nock_*`): Individual mining tokens created automatically by account tokens. One per mining device.

#### Where do I get tokens?

Create an account at [nockpool.com](https://nockpool.com) and generate account tokens in your dashboard.

#### How many threads should I use?

Logical cores times two minus 4 is a good rule of thumb. E.g., if you have a 16 core Ryzen capable of 32 threads, 28 would be a good target.

#### How much memory do I need?

As much as you can get! Recommended 8GB + 2.5 per thread.

#### How do I use custom jets?

Just swap out the `zkvm-jetpack` dependency in `Cargo.toml`.

--- 

### Building

Clone repo:

```
git clone https://github.com/SWPSCO/nockpool-miner
```

Build:

```bash
cargo build --release
```

Run: 

```bash
# With account token (recommended)
target/release/nockpool-miner --account-token nockacct_youraccounttokenhere

# Or with device key
target/release/nockpool-miner --key nockpool_yourdevicekeyhere123
```

## Command Line Options

| Flag | Environment Variable | Default | Description |
|---|---|---|---|
| `--account-token` | `NOCKPOOL_ACCOUNT_TOKEN` | - | Account token for generating mining tokens (recommended). |
| `--key` | `KEY` | - | Direct device key for authentication. |
| `--api-url` | `NOCKPOOL_API_URL` | `https://nockpool.com` | Base URL for NockPool API (for development). |
| `--max-threads` | `MAX_THREADS` | (all available threads - 2) | Set the maximum number of threads to use for mining. |
| `--server-address` | `SERVER_ADDRESS` | `quiver.nockpool.com:27016` | The `ip:port` of the nockpool server. |
| `--client-address` | `CLIENT_ADDRESS` | `0.0.0.0:27017` | The `ip:port` of the quiver client. |
| `--network-only` | `NETWORK_ONLY` | `false` | Mine only for network shares. |
| `--insecure` | `INSECURE` | `false` | Use insecure connection to the nockpool server. |
| `--benchmark` | `BENCHMARK` | `false` | Run benchmarking tool. Ignores all other arguments. |
| `--clear-key` | - | `false` | Clear stored mining key and exit. |

**Note:** Either `--account-token` or `--key` must be provided (but not both).
