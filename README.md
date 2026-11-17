# LinusWare launcher

Replaces the [old whiptail launcher](https://git.catapis.uk/linusware/main/src/commit/2b7533cf146ca7dd41901b3f974c6aeca2bb83f7/launcher.sh)


## Build(Dev)
```sh
cargo build --release
```

## Release
```sh
./dist.sh
```
builds and signs it. needs `rsign` (`cargo install rsign2`) and the key in `~/.minisign/linusware.key`, public half is `assets/linusware.pub`. the launcher only takes updates signed with that key, so dont lose it

## Install(User)
```bash
curl -fsSL https://linusware.org/install.sh | bash
```

![mrpenis](assets/mrpenis.jpg)
