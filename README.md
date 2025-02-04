# Dynamic Modules Examples

> Envoy Version: [foobarhash]

This repository hosts examples of dynamic modules for [Envoy] to extend its functionality. 
The high level documentation is available [here][High Level Doc]. In short, a dynamic module is a shared library 
that can be loaded into Envoy at runtime to add custom functionality, for example, a new HTTP filter.

It is a new way to extend Envoy without the need to recompile it just like the existing mechanisms
like Lua filters, Wasm filters, or External Processors.

Currently, the only language supported is Rust, so this repository contains examples of dynamic modules written in Rust.
Future examples will be added in other languages once the support is available.

[foobarhash]: https://github.com/envoyproxy/envoy/tree/foobarhash
[Envoy]: https://github.com/envoyproxy/envoy
[High Level Doc]: https://todo.com
