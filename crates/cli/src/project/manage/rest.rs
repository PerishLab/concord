use super::super::{fault, wire::Client};
use crate::runtime::transport::Request;
use concord_core::Result;
use serde_json::{Value, json};

pub(super) async fn fields(client: &Client<'_>, route: &str, pages: u16) -> Result<Vec<Value>> {
    let mut endpoint = format!("{route}?per_page=100");
    let mut nodes = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for _ in 0..pages {
        if !seen.insert(endpoint.clone()) {
            return Err(fault(
                "changed",
                "REST field pagination repeated its cursor",
            ));
        }
        let reply = Request::new(client.command, client.timeout, 4 * 1024 * 1024)
            .args(["api", "--method", "GET", "--include", &endpoint])
            .run()
            .await
            .map_err(|error| fault("provider", error.to_string()))?;
        let (headers, value) = decode(&reply.stdout)?;
        let values = value
            .as_array()
            .ok_or_else(|| fault("schema", "native REST fields are unreadable"))?;
        nodes.extend(values.iter().cloned());
        match continuation(&headers, route)? {
            Some(next) => endpoint = next,
            None => return Ok(nodes),
        }
    }
    Err(fault(
        "incomplete",
        "REST field pagination budget exhausted; no write is prepared",
    ))
}

pub(super) async fn create(client: &Client<'_>, route: &str, input: &Value) -> Result<Value> {
    let input = serde_json::to_vec(input).expect("native view request");
    let reply = Request::new(client.command, client.timeout, 4 * 1024 * 1024)
        .args(["api", "--method", "POST", route, "--input", "-"])
        .input(&input)
        .run()
        .await
        .map_err(|error| {
            fault(
                "indeterminate",
                format!("view creation outcome is unknown; recover before another write: {error}"),
            )
        })?;
    let value: Value = serde_json::from_slice(&reply.stdout).map_err(|error| {
        fault(
            "indeterminate",
            format!("view creation returned unreadable data: {error}"),
        )
    })?;
    let value = value.get("value").unwrap_or(&value);
    if !value["node_id"].is_string() {
        return Err(fault(
            "indeterminate",
            "created view identity is unreadable; recover without replay",
        ));
    }
    Ok(json!({"view": value["node_id"]}))
}

fn decode(bytes: &[u8]) -> Result<(String, Value)> {
    let text =
        std::str::from_utf8(bytes).map_err(|_| fault("schema", "REST headers are not UTF-8"))?;
    let (headers, body) = text
        .split_once("\r\n\r\n")
        .or_else(|| text.split_once("\n\n"))
        .ok_or_else(|| fault("schema", "REST field response omitted headers"))?;
    let value = serde_json::from_str(body).map_err(|error| {
        fault(
            "schema",
            format!("REST field response is unreadable: {error}"),
        )
    })?;
    Ok((headers.into(), value))
}

fn continuation(headers: &str, route: &str) -> Result<Option<String>> {
    for line in headers.lines() {
        let Some((name, links)) = line.split_once(':') else {
            continue;
        };
        if name.eq_ignore_ascii_case("link") {
            return next(links, route);
        }
    }
    Ok(None)
}

fn next(links: &str, route: &str) -> Result<Option<String>> {
    for link in links.split(',') {
        if !link.contains("rel=\"next\"") {
            continue;
        }
        let url = link
            .trim()
            .split_once('>')
            .map(|pair| pair.0.trim_start_matches('<'))
            .ok_or_else(|| fault("schema", "unreadable REST continuation link"))?;
        let endpoint = url
            .strip_prefix("https://api.github.com/")
            .ok_or_else(|| fault("identity", "foreign REST continuation host"))?;
        if endpoint.split('?').next() != Some(route) || endpoint.contains('#') {
            return Err(fault(
                "identity",
                "REST continuation escaped the qualified Project fields",
            ));
        }
        return Ok(Some(endpoint.into()));
    }
    Ok(None)
}
