# hi-tension

`hi-tension` (contraction of high tension) is a Rust crate designed for *basic*
but *fast* network communication between scientific applications. The focus is
on transferring large unsized arrays of `f64` with maximum throughput and
minimum latency.

## Usage

Add this to your `Cargo.toml`:
```toml
[dependencies]
hi-tension = "0.1.0"
```

Using the library is quite simple:
```rust
use hi_tension::{hiread, hiwrite, hidelimiter};

// Here we use a TcpStream but anything implementing Read and Write will do
use std::net::TcpStream;
let mut stream = TcpStream::connect("127.0.0.1:34254");
// Of course, here you need a server on the other side. Please look at the
// examples to get a testing one.

// Let's allocate a small 8 MB array
let data = vec![0.0; 1_000_000];

// Of course you can go much higher, your RAM is the limit.
// let data = vec![0.0; 1_000_000_000]; // 8 GB

// Sending data over the socket is done through calling hiwrite, and then
// hidelimiter to signal your array is done.
hiwrite(&mut stream, &data)?;
hidelimiter(&mut stream);

// You may send your data in multible packets
hiwrite(&mut stream, &data[..500_000])?;
hiwrite(&mut stream, &data[500_000..])?;
hidelimiter(&mut stream);
// This is useful for example if you are calculating your data while
// transferring it.

// To receive an array, simply call hiread
let vec = hiread(&mut stream)?;
```

## Rough protocol description

The `hi-tension` protocol accepts 2 kinds of messages:
- *Simple Text Messages*, for contextual communication and custom remote
  procedure calls defined by the client application.
- *High Tension Messages*, for fast data transfert.

Currently, this library only implements *High Tension Messages*, since *Simple
Text Messages* are easily done through `writeln!` calls, but that may change in
the future.

*High Tension Messages* are packets of `f64` (double precision floating points),
separated by the magic NaN value `0x7ff800100400a05b`. A NaN value was chosen
because:
1. They are not supposed to appear in valid calculations.
2. In the case one appears there is a `1/16777214` chance that it is exactly
   `0x7ff800100400a05b`, which is less than a probability of 0.000006 %.

Endianness is assumed to be *little-endian*, but no checks are performed. Be
careful if you use this on ARM devices.

### Acknowlegments

After a *High Tension Message* is sent, the sender must wait for a newline `\n`
sent by the receiver, to ensure succesfull reception.

*Simple Text Messages* are newline `\n` separated UTF-8 packets.
