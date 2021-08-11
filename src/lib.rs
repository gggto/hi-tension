//! `hi-tension` (contraction of high tension) is a Rust crate designed for *basic*
//! but *fast* network communication between scientific applications. The focus is
//! on transferring large unsized arrays of `f64` with maximum throughput and
//! minimum latency.
//!
//! # Usage
//!
//! ```rust
//! use hi_tension::{hiread, hiwrite, hidelimiter};
//!
//! // Here we use a TcpStream but anything implementing Read and Write will do
//! use std::net::TcpStream;
//! let mut stream = TcpStream::connect("127.0.0.1:34254");
//! // Of course, here you need a server on the other side. Please look at the
//! // examples to get a testing one.
//!
//! // Let's allocate a small 8 MB array
//! let data = vec![0.0; 1_000_000];
//!
//! // Of course you can go much higher, your RAM is the limit.
//! // let data = vec![0.0; 1_000_000_000]; // 8 GB
//!
//! // Sending data over the socket is done through calling hiwrite, and then
//! // hidelimiter to signal your array is done.
//! hiwrite(&mut stream, &data)?;
//! hidelimiter(&mut stream);
//!
//! // You may send your data in multible packets
//! hiwrite(&mut stream, &data[..500_000])?;
//! hiwrite(&mut stream, &data[500_000..])?;
//! hidelimiter(&mut stream);
//! // This is useful for example if you are calculating your data while
//! // transferring it.
//!
//! // To receive an array, simply call hiread
//! let vec = hiread(&mut stream)?;
//! ```
//!
//! # Rough protocol description
//!
//! The `hi-tension` protocol accepts 2 kinds of messages:
//! - *Simple Text Messages*, for contextual communication and custom remote
//!   procedure calls defined by the client application.
//! - *High Tension Messages*, for fast data transfert.
//!
//! Currently, this library only implements *High Tension Messages*, since *Simple
//! Text Messages* are easily done through `writeln!` calls, but that may change in
//! the future.
//!
//! *High Tension Messages* are packets of `f64` (double precision floating points),
//! separated by the magic NaN value `0x7ff800100400a05b`. A NaN value was chosen
//! because:
//! 1. They are not supposed to appear in valid calculations.
//! 2. In the case one appears there is a `1/16777214` chance that it is exactly
//!    `0x7ff800100400a05b`, which is less than a probability of 0.000006 %.
//!
//! Endianness is assumed to be *little-endian*, but no checks are performed. Be
//! careful if you use this on ARM devices.
//!
//! ## Acknowlegments
//!
//! After a *High Tension Message* is sent, the sender must wait for a newline `\n`
//! sent by the receiver, to ensure succesfull reception.
//!
//! *Simple Text Messages* are newline `\n` separated UTF-8 packets.

use std::io::{Read, Result, Write};

const DELIMITER_NAN: [u8; 8] = [0x5b, 0xa0, 0x00, 0x04, 0x10, 0x00, 0xf8, 0x7f];
const DEFAULT_SIZE: usize = 100_000_000;

fn as_u8_slice<T>(v: &[T]) -> &[u8] {
    unsafe {
        std::slice::from_raw_parts(v.as_ptr() as *const u8, v.len() * std::mem::size_of::<T>())
    }
}

fn as_u8_slice_mut<T>(v: &mut [T]) -> &mut [u8] {
    unsafe {
        std::slice::from_raw_parts_mut(v.as_ptr() as *mut u8, v.len() * std::mem::size_of::<T>())
    }
}

/// Read a *High Tension Message* from the `stream`.
///
/// This function is blocking.
///
/// During operation, this function allocates space greedily by doubling buffer
/// size each time more space is needed. Since message size is unknown, this
/// minimize the number of allocations required, but may induce excessive RAM
/// consumption. Extra space is released when the function returns.
///
/// # Examples
///
/// Basic usage:
///
/// ```no_run
/// use std::net::TcpStream;
/// let stream = TcpStream::connect("127.0.0.1:34567")
///
/// let data = hiread(&mut stream);
/// ```
pub fn hiread<S: Read + Write>(stream: &mut S) -> Result<Vec<f64>> {
    let mut i = 0;
    let mut size = DEFAULT_SIZE;
    let mut buf = vec![0.0; size];
    let mut buf_view = as_u8_slice_mut(&mut buf);
    loop {
        if i == size * 8 {
            drop(buf_view);
            size *= 2;
            buf.resize(size, 0.0);
            buf_view = as_u8_slice_mut(&mut buf);
        }

        i += stream.read(&mut buf_view[i..])?;

        if buf_view[i - 8..i] == DELIMITER_NAN {
            stream.write(b"\n")?;
            stream.flush()?;
            break;
        }
    }
    size = i / 8 - 1;
    buf.truncate(size);
    Ok(buf)
}

/// Send a `data` slice as a *High Tension Message* into the `stream`.
///
/// This function is blocking.
///
/// Your message shall be ended by calling [`hidelimiter`] on the stream. You
/// may call `hiwrite` more than one time, if you need to send the data piece by
/// piece (e.g. if you calculate the data while sending it.).
///
/// [`hidelimiter`]: fn.hidelimiter.html
///
/// # Examples
///
/// Basic usage:
///
/// ```no_run
/// use std::net::TcpStream;
/// let stream = TcpStream::connect("127.0.0.1:34567")
///
/// let data = vec![0.0; 1_000_000]; // 8 MB
/// // Of course you can go much higher, your RAM is the limit.
/// // let data = vec![0.0; 1_000_000_000]; // 8 GB
///
/// hiwrite(&mut stream, &data)?;
/// hidelimiter(&mut stream);
///
/// // You may send your data in multible packets
/// hiwrite(&mut stream, &data[..500_000])?;
/// hiwrite(&mut stream, &data[500_000..])?;
/// hidelimiter(&mut stream);
/// ```
pub fn hiwrite<W: Write>(stream: &mut W, data: &[f64]) -> Result<()> {
    let mut i = 0;
    let slice = as_u8_slice(&data[i..]);
    loop {
        i += stream.write(&slice[i..])?;

        if i == slice.len() {
            break;
        }
    }
    Ok(())
}

/// Signal the ending of a *High Tension Message* to the other end of the
/// `stream`.
///
/// Takes care of reception acknowledgements from the other side. This function
/// is blocking.
///
/// This function is generally used after one or more calls to [`hiwrite`].
///
/// [`hiwrite`]: fn.hiwrite.html
///
/// # Examples
///
/// Basic usage:
///
/// ```no_run
/// use std::net::TcpStream;
/// let stream = TcpStream::connect("127.0.0.1:34567")
///
/// let data = vec![0.0; 1_000_000]; // 8 MB
///
/// hiwrite(&mut stream, &data)?;
/// hidelimiter(&mut stream);
/// ```
pub fn hidelimiter<S: Read + Write>(stream: &mut S) -> Result<()> {
    stream.write(&DELIMITER_NAN)?;
    stream.flush()?;
    stream.read_exact(&mut [0])
}
