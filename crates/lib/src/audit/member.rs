use super::*;

struct Check<'a> {
    task: &'a Legacy,
    member: &'a crate::Member,
    path: &'a Path,
    source: &'a Path,
}

struct Inspection<'a, 'b> {
    audit: &'a mut Audit,
    check: Check<'b>,
}

#[locus::trace(with = crate::observation::view())]
pub(super) fn inspect(
    audit: &mut Audit,
    task: &Legacy,
    member: &crate::Member,
    version: u32,
) -> Result<()> {
    let path = task.member(&member.name);
    if !path.is_dir() {
        audit.fault("presence", &path, "declared member path is missing");
        return Ok(());
    }
    let source = match task.source(&member.source) {
        Ok(source) => source,
        Err(error) => {
            audit.fault("source", &path, error.to_string());
            return Ok(());
        }
    };
    let origin = git::at(&source).identity();
    let seat = git::at(&path).seat();
    let mut inspection = Inspection {
        audit,
        check: Check {
            task,
            member,
            path: &path,
            source: &source,
        },
    };
    inspection.identities(&origin, &seat);
    if source.is_dir() && !git::at(&source).registered(&path)? {
        inspection.audit.fault(
            "worktree",
            &path,
            "member path is absent from source Git worktree metadata",
        );
    }
    inspection.branch(seat);
    inspection.boundary(version);
    Ok(())
}

impl Inspection<'_, '_> {
    fn identities(&mut self, source: &Result<std::path::PathBuf>, seat: &Result<git::Seat>) {
        match (source, seat) {
            (Ok(source), Ok(seat)) if source != &seat.identity => self.audit.fault(
                "identity",
                self.check.path,
                format!(
                    "member Git identity {} differs from source {}",
                    seat.identity.display(),
                    source.display()
                ),
            ),
            (Err(error), _) => self
                .audit
                .fault("source", self.check.source, error.to_string()),
            (_, Err(error)) => self
                .audit
                .fault("worktree", self.check.path, error.to_string()),
            _ => {}
        }
    }

    fn branch(&mut self, seat: Result<git::Seat>) {
        match seat {
            Ok(seat) => {
                let expected = self.check.member.branch(&self.check.task.task().name);
                if seat.branch != expected {
                    self.audit.fault(
                        "branch",
                        self.check.path,
                        format!(
                            "member branch {} differs from registry {expected}",
                            seat.branch
                        ),
                    );
                }
            }
            Err(error) => self
                .audit
                .fault("branch", self.check.path, error.to_string()),
        }
    }

    fn boundary(&mut self, version: u32) {
        if version < 2 {
            return;
        }
        let head = git::at(self.check.path).head();
        let valid =
            head.as_ref().ok().is_some_and(|head| {
                self.check.member.boundary.as_ref().is_some_and(|proof| {
                    crate::boundary::valid(proof, &self.check.member.write, head)
                })
            });
        if !valid {
            self.audit.observe(
                "boundary",
                self.check.path,
                "member boundary proof is absent or stale",
            );
        }
    }
}
