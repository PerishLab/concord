use super::super::authority::{self, Authorities};
use super::{Authority, Candidate, Context, Mode, Plan, Request, gate, model, refused};
use crate::Result;

pub(super) fn prepare<A: Authorities>(
    context: &Context,
    request: &Request,
    pull: &plumb::delivery::Narrative,
) -> Result<(Authority, Candidate)> {
    let input = plumb::delivery::Request {
        root: &context.path,
        repository: &request.snapshot.repository,
        issue: &request.snapshot,
        observed: request.observed,
        base: &request.base,
        pull,
    };
    match request.authority {
        Mode::Plumb => {
            let (accepted, warrant) = authority::select::<A>(
                &context.path,
                &context.boundary.head,
                "concord.delivery.authority",
            )?;
            let plan = plumb::delivery::prepare(input, &accepted).map_err(refused)?;
            let authority = Authority::Plumb {
                warrant,
                guard: plan.guard.clone(),
            };
            Ok((authority, plan.into()))
        }
        Mode::WharfNative => {
            model::admit(&context.member.integration)?;
            let plan = plumb::delivery::native::prepare(input, &gate::Wharf).map_err(refused)?;
            let authority = Authority::WharfNative {
                evidence: plan.evidence.clone(),
            };
            Ok((authority, plan.into()))
        }
    }
}

pub(super) fn revalidate<A: Authorities>(
    context: &Context,
    plan: &Plan,
    snapshot: &plumb::delivery::Snapshot,
    observed: u64,
) -> Result<()> {
    let input = plumb::delivery::Request {
        root: &context.path,
        repository: &plan.delivery.repository,
        issue: snapshot,
        observed,
        base: &plan.delivery.base,
        pull: &plan.delivery.pull,
    };
    match &plan.authority {
        Authority::Plumb { warrant, guard } => {
            let accepted = authority::keep::<A>(warrant, "concord.delivery.authority")?;
            plumb::delivery::revalidate(input, &plan.delivery.guard(guard), &accepted)
                .map_err(refused)?;
        }
        Authority::WharfNative { evidence } => {
            model::admit(&context.member.integration)?;
            plumb::delivery::native::revalidate(
                input,
                &plan.delivery.native(evidence),
                &gate::Wharf,
            )
            .map_err(refused)?;
        }
    }
    Ok(())
}
