<div align="center">

<img src="https://github.com/rustbase.png?size=115">
    
<h1>
Rustbase
</h1>

A noSQL key-value database cross-platform program written in [Rust](https://www.rust-lang.org/)

</div>

<br />

# ⚠️ Warning
This is a work in progress.

# Supported platforms
| **Platform** | **is supported?** |
| ------------ | ----------------- |
| Windows      | Yes               |
| Linux        | Yes               |
| macOS        | No                |

# Download
You can download the latest version of Rustbase [here](https://github.com/rustbase/rustbase/releases)

# Development Dependencies
Because of `tonic` dependency, we need some extra dependencies to compile the program.

## Ubuntu
```bash
sudo apt update && sudo apt upgrade -y
sudo apt install -y protobuf-compiler libprotobuf-dev
```

## Alpine Linux
```bash
sudo apk add protoc protobuf-dev
```

## MacOS
```bash
brew install protobuf
```

[Reference](https://github.com/hyperium/tonic#dependencies)



# 🔗 Contribute
[Click here](./CONTRIBUTING.md) to see how to Contribute

Join our [Discord server](https://discord.gg/m5ZzWPumbd) to get help and discuss features.

# Components
- [Config](./src/config/)
- [Query](./src/query/)
- [Server](./src/server/)
    * [Cache](./src/server/cache/)
    * [Engine](./src/server/engine/)
    * [Route](./src/server/route/)
- [Utils](./src/utils/)

# Authors
<div align="center">

| [<img src="https://github.com/pedrinfx.png?size=115" width=115><br><sub>@pedrinfx</sub>](https://github.com/pedrinfx) |
| :-------------------------------------------------------------------------------------------------------------------: |


</div>

# License
[MIT License](./LICENSE)


