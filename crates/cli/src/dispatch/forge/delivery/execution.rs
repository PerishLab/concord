use crate::dispatch::Completion;
use concord_core::issue_delivery::execution::{Failure, Permit};
use concord_core::{Error, Result};
use serde_json::json;
use std::future::Future;

pub async fn run(
    permit: Permit,
    action: impl Future<Output = Result<Completion>>,
) -> Result<Completion> {
    finish(permit.during(|| action).await)
}

fn finish(
    output: std::result::Result<Result<Completion>, Failure<Result<Completion>>>,
) -> Result<Completion> {
    match output {
        Ok(output) => output,
        Err(Failure::Release { output, error }) => {
            let business = business(output);
            Err(Error::detailed(
                "concord.delivery.execution",
                format!(
                    "land execution release is unconfirmed: {error}; preserve the business result and inspect exact provider state before resuming"
                ),
                json!({"business": business}),
            ))
        }
        Err(Failure::Admission(error)) => Err(Error::typed(
            "concord.delivery.execution",
            format!("land execution admission refused: {error}"),
        )),
    }
}

fn business(output: Result<Completion>) -> serde_json::Value {
    match output {
        Ok(completion) => {
            json!({"exit": completion.code(), "output": "any emitted business report remains on stdout"})
        }
        Err(error) => {
            json!({"code": error.code(), "message": error.message(), "details": error.details()})
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release() {
        for output in [
            Ok(Completion::Pending),
            Err(Error::detailed(
                "business.refused",
                "preserve",
                json!({"evidence": 7}),
            )),
        ] {
            let error = finish(Err(Failure::Release {
                output,
                error: std::io::Error::other("injected release failure").into(),
            }))
            .expect_err("unknown release never returns business success");
            assert_eq!(error.code(), "concord.delivery.execution");
            let business = &error.details().unwrap()["business"];
            assert!(
                business["exit"] == 3
                    || (business["code"] == "business.refused"
                        && business["details"]["evidence"] == 7)
            );
        }
    }
}
