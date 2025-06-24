# moz-merino-ext

A collection of Python extensions for Mozilla's [`merino`](https://github.com/mozilla-services/merino-py) implemented in Rust using [`PyO3`](https://pyo3.rs).

## Building and Testing

This project is managed by [`uv`](https://docs.astral.sh/uv/) and [`maturin`](https://www.maturin.rs/index.html). To set up the development environment:

```shell
make install
```

To build the extension and install it in a local virtual environment:

```shell
make dev
```

To run all the tests:

```shell
make test
```

Or to run build and test altogether:

```shell
make dev-test
```

See all other utilities:

```shell
make help
```
