use super::{Client, observe};
use serde_json::{Value, json};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

pub(super) struct World {
    address: SocketAddr,
    timeout: u64,
    state: Arc<Mutex<Value>>,
    stop: Arc<AtomicBool>,
}

impl Drop for World {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(self.address);
    }
}

impl World {
    pub(super) fn new(mode: &str) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let state = Arc::new(Mutex::new(json!({
            "active": 0, "peak": 0, "open": false, "events": [], "confirmed": []
        })));
        let stop = Arc::new(AtomicBool::new(false));
        let timeout = if mode == "timeout" { 1 } else { 5 };
        let mode = mode.to_owned();
        let shared = Arc::clone(&state);
        let flag = Arc::clone(&stop);
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                if flag.load(Ordering::SeqCst) {
                    break;
                }
                let mode = mode.clone();
                let shared = Arc::clone(&shared);
                std::thread::spawn(move || serve(stream, &mode, &shared));
            }
        });
        Self {
            address,
            timeout,
            state,
            stop,
        }
    }

    pub(super) async fn run(&self, count: usize, pages: u16) -> (concord_core::Result<()>, Value) {
        let client = self.client();
        let items = (1..=count).map(item).collect::<Vec<_>>();
        let field = json!({"id":"F_attached", "authority":"issue", "dataType":"SINGLE_SELECT", "issueField":{"fullDatabaseId":"42"}});
        let mut collections = json!({"fields":{"nodes":[field]},"items":{"nodes":items}});
        let result = observe(&client, &mut collections, pages).await;
        (result, collections)
    }

    pub(super) fn state(&self) -> Value {
        self.state.lock().unwrap().clone()
    }

    pub(super) fn client(&self) -> Client {
        Client {
            timeout: self.timeout,
            loopback: Some(self.address),
        }
    }
}

fn serve(mut stream: TcpStream, mode: &str, state: &Arc<Mutex<Value>>) {
    let Some((path, body)) = read(&mut stream) else {
        return;
    };
    let (status, payload) = if path == "/graphql" {
        graphql(&body, mode, state)
    } else {
        rest(&path, mode, state)
    };
    let encoded = payload.as_bytes();
    let header = format!(
        "HTTP/1.1 {status} OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        encoded.len()
    );
    let _ = stream.write_all(header.as_bytes());
    let _ = stream.write_all(encoded);
}

fn read(stream: &mut TcpStream) -> Option<(String, Vec<u8>)> {
    let mut head = Vec::new();
    let mut byte = [0u8; 1];
    while !head.ends_with(b"\r\n\r\n") {
        if stream.read(&mut byte).ok()? == 0 {
            return None;
        }
        head.push(byte[0]);
    }
    let text = String::from_utf8_lossy(&head);
    let path = text.lines().next()?.split_whitespace().nth(1)?.to_owned();
    let length = text
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse::<usize>().ok())
                .flatten()
        })
        .unwrap_or(0);
    let mut body = vec![0u8; length];
    stream.read_exact(&mut body).ok()?;
    Some((path, body))
}

fn rest(path: &str, mode: &str, state: &Arc<Mutex<Value>>) -> (u16, String) {
    let number: usize = path
        .split("/issues/")
        .nth(1)
        .unwrap()
        .split('/')
        .next()
        .unwrap()
        .parse()
        .unwrap();
    let page: usize = path.rsplit_once("page=").unwrap().1.parse().unwrap();
    if page == 1 {
        record(state, "start", number);
    }
    record(state, "page", number);
    if mode == "parallel" && number <= 4 {
        until(state, |value| value["open"] == true);
    }
    if mode == "refused" && number == 1 {
        record(state, "refused", number);
        return (500, json!({"message":"refused"}).to_string());
    }
    if mode == "timeout" {
        std::thread::sleep(Duration::from_secs(3));
    }
    let value = match mode {
        "partial" => json!({}),
        "repeat" => {
            if page == 1 {
                json!((1..=100).map(field).collect::<Vec<_>>())
            } else {
                json!([field(42)])
            }
        }
        "type" => json!([{"issue_field_id":42, "data_type":"text"}]),
        "pages" => {
            if page == 1 {
                json!((1..=100).map(field).collect::<Vec<_>>())
            } else {
                json!([])
            }
        }
        "empty" => json!([]),
        _ => {
            let mut value = field(42);
            value["value"] = json!(number);
            json!([value])
        }
    };
    (200, value.to_string())
}

fn graphql(body: &[u8], mode: &str, state: &Arc<Mutex<Value>>) -> (u16, String) {
    let request: Value = serde_json::from_slice(body).unwrap();
    let number: usize = request["variables"]["id"]
        .as_str()
        .unwrap()
        .split('_')
        .next_back()
        .unwrap()
        .parse()
        .unwrap();
    if mode == "parallel" && number <= 4 {
        until(state, |value| {
            let confirmed = value["confirmed"].as_array().unwrap();
            ((number + 1)..=4).all(|index| confirmed.contains(&json!(index)))
        });
    }
    record(state, "confirmed", number);
    let mut node =
        json!({"id":format!("I_{number}"), "updated":"unchanged", "repository":{"id":"R_repo"}});
    match mode {
        "node" => node["id"] = json!("I_foreign"),
        "repository" => node["repository"]["id"] = json!("R_foreign"),
        "stamp" => node["updated"] = json!("changed"),
        _ => {}
    }
    let mut reply = json!({"data":{"node":node}});
    if mode == "graphql" {
        reply["errors"] = json!([{"message":"partial response"}]);
    }
    (200, reply.to_string())
}

fn record(state: &Arc<Mutex<Value>>, stage: &str, number: usize) {
    let mut value = state.lock().unwrap();
    value["events"]
        .as_array_mut()
        .unwrap()
        .push(json!([stage, number]));
    if stage == "start" {
        let active = value["active"].as_u64().unwrap() + 1;
        let peak = value["peak"].as_u64().unwrap().max(active);
        value["active"] = json!(active);
        value["peak"] = json!(peak);
        if active == 4 {
            value["open"] = json!(true);
        }
    }
    if stage == "confirmed" || stage == "refused" {
        let active = value["active"].as_u64().unwrap() - 1;
        value["active"] = json!(active);
    }
    if stage == "confirmed" {
        value["confirmed"]
            .as_array_mut()
            .unwrap()
            .push(json!(number));
    }
}

fn until(state: &Arc<Mutex<Value>>, predicate: impl Fn(&Value) -> bool) {
    let end = Instant::now() + Duration::from_secs(2);
    while !predicate(&state.lock().unwrap()) {
        if Instant::now() >= end {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn field(number: usize) -> Value {
    json!({"issue_field_id":number, "data_type":"single_select", "value":number})
}

fn item(number: usize) -> Value {
    let kind = match number % 3 {
        0 => "Auto",
        1 => "Feature",
        _ => "Unknown",
    };
    json!({"content":{
        "__typename":"Issue", "id":format!("I_{number}"), "number":number,
        "updatedAt":"unchanged", "repository":{"id":"R_repo", "nameWithOwner":"PerishLab/concord"},
        "issueType":{"name":kind}
    }})
}
