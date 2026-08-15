use super::{Estate, Flow, Graph, Life, Role, Weight, World, fault};
use crate::{Error, Result};
use serde::Serialize;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Finding {
    pub code: String,
    pub subject: String,
    pub message: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Agreement {
    pub target: String,
    pub revision: i64,
    pub faults: Vec<Finding>,
    pub observations: Vec<Finding>,
}

impl Agreement {
    pub fn agrees(&self) -> bool {
        self.faults.is_empty()
    }

    pub(super) fn fault(
        &mut self,
        code: &str,
        subject: impl Into<String>,
        message: impl Into<String>,
    ) {
        self.faults.push(Finding {
            code: code.to_string(),
            subject: subject.into(),
            message: message.into(),
        });
    }

    pub(super) fn observe(
        &mut self,
        code: &str,
        subject: impl Into<String>,
        message: impl Into<String>,
    ) {
        self.observations.push(Finding {
            code: code.to_string(),
            subject: subject.into(),
            message: message.into(),
        });
    }
}

impl Estate {
    #[locus::trace(with = crate::observation::view())]
    pub async fn inspect(&self, task: Option<&str>, domain: Option<&str>) -> Result<Agreement> {
        if task.is_some() && domain.is_some() {
            return Err(Error::typed(
                "concord.audit.scope",
                "audit accepts a Task or Domain, not both",
            ));
        }
        let world = World::load(self).await?;
        let selected = select(&world, task, domain)?;
        let graph = self.graph(true).await?;
        let target = task
            .or(domain)
            .map(str::to_string)
            .unwrap_or_else(|| self.space.display().to_string());
        let mut report = Agreement {
            target,
            revision: graph.revision,
            faults: Vec::new(),
            observations: Vec::new(),
        };
        reservations(self, &world, &mut report).await?;
        network(self, &graph, &mut report).await?;
        facts(self, &selected, &mut report).await;
        super::territory::inspect(self, &selected, &mut report).await?;
        report.faults.sort_by_key(order);
        report.observations.sort_by_key(order);
        Ok(report)
    }

    pub(super) async fn ensure(&self) -> Result<()> {
        let report = self.inspect(None, None).await?;
        if report.agrees() {
            return Ok(());
        }
        Err(Error::typed(
            "concord.audit.refused",
            format!("estate has {} agreement fault(s)", report.faults.len()),
        ))
    }
}

fn select(world: &World, task: Option<&str>, domain: Option<&str>) -> Result<Vec<super::Node>> {
    if let Some(task) = task {
        return world.node(task).cloned().map(|task| vec![task]);
    }
    if let Some(domain) = domain {
        if world.domain(domain).is_none() {
            return Err(Error::typed(
                "concord.domain.absent",
                format!("unknown managed domain {domain}"),
            ));
        }
        return Ok(world
            .nodes
            .iter()
            .filter(|task| task.domain == domain)
            .cloned()
            .collect());
    }
    Ok(world.nodes.clone())
}

async fn reservations(state: &Estate, world: &World, report: &mut Agreement) -> Result<()> {
    let rows = state.core.live("Reservation").await.map_err(fault)?;
    for task in &world.nodes {
        let domain = world.domain(&task.domain).expect("Task Domain is loaded");
        let held = rows.iter().any(|row| {
            row.int("domain") == Some(domain) && row.text("name") == Some(task.name.as_str())
        });
        if !held {
            report.fault(
                "reservation.missing",
                task.identity(),
                "current Task coordinate has no lifetime Reservation",
            );
        }
    }
    for row in rows {
        if row.text("name").is_none()
            || !world.domains.contains_key(&row.int("domain").unwrap_or(-1))
        {
            report.fault(
                "reservation.malformed",
                format!("Reservation#{}", row.key()),
                "Reservation has malformed name or Domain",
            );
        }
    }
    Ok(())
}

async fn network(plane: &Estate, graph: &Graph, report: &mut Agreement) -> Result<()> {
    for edge in &graph.edges {
        let source = graph.nodes.iter().find(|node| node.key == edge.source);
        let target = graph.nodes.iter().find(|node| node.key == edge.target);
        if source.is_none_or(|node| node.life == Life::Retired)
            || target.is_none_or(|node| node.life == Life::Retired)
        {
            report.fault(
                "dependency.retired",
                format!("Dependency#{}", edge.key),
                "live dependency endpoint is absent or retired",
            );
        }
        if edge.weight == Weight::Unknown {
            report.observe(
                "dependency.unknown",
                format!("Dependency#{}", edge.key),
                "dependency weight is unknown",
            );
        }
        if let (Some(source), Some(target)) = (source, target)
            && source.domain != target.domain
        {
            report.observe(
                "dependency.cross_domain",
                format!("{} -> {}", source.identity(), target.identity()),
                "dependency crosses Domain boundaries",
            );
        }
    }
    for cycle in graph.cycles() {
        report.fault(
            "dependency.cycle",
            "graph",
            format!("direct dependency cycle: {cycle:?}"),
        );
    }
    for node in &graph.nodes {
        let wanted = graph.reach(node.key, Flow::Out, Weight::Unknown);
        let found = plane
            .reach(&node.identity())
            .await?
            .into_iter()
            .map(|node| node.key)
            .collect::<Vec<_>>();
        if wanted != found {
            report.fault(
                "dependency.closure",
                node.identity(),
                "materialized closure differs from the direct graph",
            );
        }
    }
    Ok(())
}

async fn facts(state: &Estate, tasks: &[super::Node], report: &mut Agreement) {
    for task in tasks {
        let identity = task.identity();
        current(state, task, report).await;
        if task.life == Life::Active {
            phases(state, &identity, report).await;
        }
    }
}

async fn current(plane: &Estate, task: &super::Node, report: &mut Agreement) {
    let identity = task.identity();
    let current = match plane.current(&identity).await {
        Ok(current) => current,
        Err(error) => {
            report.fault(error.code(), identity, error.message());
            return;
        }
    };
    if task.life != Life::Active {
        return;
    }
    let missing = [
        (Role::Goal, "Goal"),
        (Role::Focus, "Focus"),
        (Role::Next, "Next"),
    ]
    .into_iter()
    .filter(|(role, _)| !current.facts.iter().any(|fact| fact.role == *role));
    for (_, name) in missing {
        report.observe(
            "task.missing_fact",
            task.identity(),
            format!("active Task has no {name}"),
        );
    }
}

async fn phases(seat: &Estate, identity: &str, report: &mut Agreement) {
    let phases = match seat.phases(identity).await {
        Ok(phases) => phases,
        Err(error) => {
            report.fault(error.code(), identity, error.message());
            return;
        }
    };
    let missing = phases.into_iter().filter(|phase| {
        !phase
            .entries
            .iter()
            .any(|entry| entry.part == super::Part::Outcome)
    });
    for phase in missing {
        report.observe(
            "phase.missing_outcome",
            format!("{identity}#{}", phase.number),
            "migrated legacy Phase has no Outcome",
        );
    }
}

fn order(finding: &Finding) -> (String, String, String) {
    (
        finding.code.clone(),
        finding.subject.clone(),
        finding.message.clone(),
    )
}
