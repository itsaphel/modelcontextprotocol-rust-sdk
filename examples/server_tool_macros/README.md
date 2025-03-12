Examples using the `#[tool]` macro to define functions as tools, as opposed to the more manual approach in the `servers` example.

## Stateless server

Run with: `cargo run -p mcp-server-macros-examples --example stateless-server`

Test by sending messages like:

```
{"jsonrpc":"2.0","id":1,"method":"tools/list"}

{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"calculator","arguments":{"x":1,"y":2,"operation":"add"}}}

{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"calculator","arguments":{"x":4,"y":5,"operation":"multiply"}}}
```