# Ws-Gonzale
Ws Gonzale is aimed to be a async ws-server with "presence" and shared data.

Right now it's just in it's most naive representation and the Wsframe reader isn't in it's final form yet.

I Also didn't want to just copy another framework.

## Checkout the example
If you wanna run a simple WS server
```
cargo run --example life-cycle
```

## Benchmark
### Rust benchmark
```
cargo run bench
```
### Using artillery
Start the test server first
```
cargo run --example life-cycle
```
and then in another window run
```
artillery run benches/artillery.yml
```

## FAQ

### Why not use X library?
This is just to play around with so I can get used to multithreading environments coming from a NodeJS background.

There are a lot of great Rust developers out there; but sometimes a bit less intermediate approach can help teach people some basic concepts.

### Where is this project going?
See Issues

### Where does the name come from?
Speedy gonzales

### WS Protocol
This is the most lazy implementation of the WS protocols. Right now it's just supporting handshakes and sending basic messages.
https://tools.ietf.org/html/rfc6455

### Can I contribute?
Always