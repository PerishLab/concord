use plumb::seat::release::Authority;

use super::super::model::Header;
use super::super::plan::{Advice, Status};
use super::super::read::authority::manifest;
use crate::args::acceptance::Observe;

pub(super) async fn read(args: &Observe, header: &Header) -> Advice {
    let first = manifest(args, header).await;
    let second = manifest(args, header).await;
    let mut advice = Advice {
        repository: header.repository.clone(),
        manifest: None,
        status: Status::Unknown,
        message: String::new(),
    };
    if let (Ok(first), Ok(second)) = (first, second)
        && first == second
    {
        classify(&mut advice, first);
    }
    advice.message = advice.status.message().into();
    advice
}

fn valid(value: &str) -> bool {
    value.len() == 40
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn classify(advice: &mut Advice, blob: Option<super::super::read::authority::Blob>) {
    let Some(blob) = blob else {
        advice.status = Status::Missing;
        return;
    };
    if !valid(&blob.oid) {
        return;
    }
    advice.manifest = Some(blob.oid);
    advice.status = match Authority::declared(&blob.text) {
        Ok(Some(_)) => Status::Declared,
        Ok(None) => Status::Missing,
        Err(_) => Status::Invalid,
    };
}
