# Rubik's cage analysis

## Webapp build

Install [webassembly target and trunk](https://yew.rs/docs/getting-started/introduction#install-webassembly-target).

```
RUSTFLAGS='--cfg getrandom_backend="wasm_js"'
trunk build --release
```

or `trunk serve` to serve with hot reloading.
