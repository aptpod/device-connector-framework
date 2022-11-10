# Device Connector Framework

Device Connector Framework is a development framework provided by aptpod to develop Device Connector.

## Document

https://docs.intdash.jp/terminal-system/device-connector/device-connector-framework/v2.0.0/

## How to try

Device Connector is written in Rust, so please install Rust compilation tools. You can use [rustup](https://rustup.rs/) to install Rust.

Build Device Connector.

```sh
git clone https://github.com/aptpod/device-connector-framework.git
cd device-connector-framework
cargo build --release -p device-connector-run
```

Prepare configuration file for Device Connector, and save it as `conf.yaml`.

```yaml
tasks:
  - id: 1
    element: text-src
    conf:
      text: "Hello, World!"
      interval_ms: 100

  - id: 2
    element: stdout-sink
    from:
      - - 1
    conf:
      separator: "\n"
```

And run.

```
./target/release/device-connector-run --config conf.yaml
```
