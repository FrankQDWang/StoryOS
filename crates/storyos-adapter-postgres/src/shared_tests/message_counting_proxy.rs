//! A TCP proxy that counts the client messages of the database protocol, to measure round trips.

use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::{Arc, Mutex};

const SSL_REQUEST: [u8; 4] = 80_877_103_u32.to_be_bytes();

/// The client messages that went through a [`message_counting_proxy`]. A round trip is a Sync or
/// a simple Query, which wait for an answer. The Sync after the Close of a dropped statement waits
/// for nothing, so it is no round trip. One Execute runs one statement; one simple Query can run
/// more than one.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct SentMessages {
    pub(crate) round_trips: usize,
    pub(crate) simple_queries: usize,
    pub(crate) parses: usize,
    pub(crate) executes: usize,
}

/// A TCP proxy in front of the database that counts the client messages of its connections.
pub(crate) fn message_counting_proxy(database_url: &str) -> (String, Arc<Mutex<SentMessages>>) {
    let config = database_url.parse::<tokio_postgres::Config>().unwrap();
    let port = config.get_ports()[0];
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let proxy_port = listener.local_addr().unwrap().port();
    let messages = Arc::new(Mutex::new(SentMessages::default()));
    let counted = Arc::clone(&messages);
    std::thread::spawn(move || {
        for client in listener.incoming() {
            let (Ok(mut client), Ok(mut server)) =
                (client, TcpStream::connect(("127.0.0.1", port)))
            else {
                return;
            };
            let (mut client_read, mut server_write) =
                (client.try_clone().unwrap(), server.try_clone().unwrap());
            let counted = Arc::clone(&counted);
            std::thread::spawn(move || {
                count_and_forward(&mut client_read, &mut server_write, &counted);
                close_both(&client_read, &server_write);
            });
            std::thread::spawn(move || {
                let _copied = std::io::copy(&mut server, &mut client);
                close_both(&server, &client);
            });
        }
    });
    let proxy_url = database_url.replacen(&format!(":{port}/"), &format!(":{proxy_port}/"), 1);
    (proxy_url, messages)
}

// When one side ends, the other side sees the end too.
fn close_both(first: &TcpStream, second: &TcpStream) {
    let _first = first.shutdown(Shutdown::Both);
    let _second = second.shutdown(Shutdown::Both);
}

// The count goes up before the message goes on, so a client that has its answer sees the count.
fn count_and_forward(
    client: &mut TcpStream,
    server: &mut TcpStream,
    messages: &Mutex<SentMessages>,
) {
    let read_message = |from: &mut TcpStream, header: &mut [u8]| {
        from.read_exact(header).ok()?;
        let length = u32::from_be_bytes(header[header.len() - 4..].try_into().ok()?);
        let mut body = vec![0; usize::try_from(length).ok()? - 4];
        from.read_exact(&mut body).ok()?;
        Some(body)
    };
    // An SSLRequest can come before the startup message; neither has a type byte.
    loop {
        let mut header = [0; 4];
        let Some(body) = read_message(client, &mut header) else {
            return;
        };
        if server.write_all(&[&header[..], &body].concat()).is_err() {
            return;
        }
        if body.get(..4) != Some(&SSL_REQUEST[..]) {
            break;
        }
    }
    let mut previous = 0;
    loop {
        let mut header = [0; 5];
        let Some(body) = read_message(client, &mut header) else {
            return;
        };
        if let Ok(mut counts) = messages.lock() {
            match header[0] {
                b'Q' => {
                    counts.simple_queries += 1;
                    counts.round_trips += 1;
                }
                b'S' if previous != b'C' => counts.round_trips += 1,
                b'P' => counts.parses += 1,
                b'E' => counts.executes += 1,
                _ => {}
            }
        }
        previous = header[0];
        if server.write_all(&[&header[..], &body].concat()).is_err() {
            return;
        }
    }
}
