use super::super::super::{Estate, fault, upgrade};
use super::super::{bridge, legacy, released};
use super::{Migration, Plan, RECEIPT, Receipt, Rollback, SCHEMA, SOURCE, Survey, TARGET};
use super::{align, exact, storage, validate};
use crate::{Error, Result};
use fs2::FileExt;
use keel::adapt::db::Sqlite;
use std::fs::File;
use std::path::Path;

impl Migration<'_> {
    pub async fn survey(&self) -> Result<Survey> {
        let guard = self.guard()?;
        let survey = self.inspect().await;
        unlock(&guard)?;
        survey
    }

    pub async fn prepare(&self, fingerprint: &str) -> Result<Plan> {
        let guard = self.guard()?;
        let survey = self.inspect().await?;
        exact("source fingerprint", fingerprint, &survey.fingerprint)?;
        let stage = survey.stage.join("estate.sqlite3");
        let created = !stage.is_file();
        let staged = if !created {
            storage::evidence(&stage)?
        } else {
            storage::copy(&self.seat.database(), &stage)?;
            if let Err(error) = self.evolve(&stage).await {
                let _ = std::fs::remove_file(&stage);
                return Err(error);
            }
            storage::evidence(&stage)?
        };
        if let Err(error) = self.verify(&stage).await {
            if created {
                let _ = std::fs::remove_file(&stage);
            }
            return Err(error);
        }
        let plan = Plan {
            schema: SCHEMA.to_string(),
            survey,
            staged,
        };
        unlock(&guard)?;
        Ok(plan)
    }

    pub async fn apply(&self, plan: &Plan) -> Result<Receipt> {
        validate(plan)?;
        let guard = self.guard()?;
        let survey = self.inspect().await?;
        exact(
            "source fingerprint",
            &plan.survey.fingerprint,
            &survey.fingerprint,
        )?;
        exact(
            "stage path",
            &plan.staged.path,
            &survey.stage.join("estate.sqlite3"),
        )?;
        let staged = storage::evidence(&plan.staged.path)?;
        exact(
            "staged database digest",
            &plan.staged.digest,
            &staged.digest,
        )?;
        self.verify(&plan.staged.path).await?;
        let backup = survey.backup.join("estate.sqlite3");
        let backup = if backup.exists() {
            storage::evidence(&backup)?
        } else {
            storage::copy(&self.seat.database(), &backup)?
        };
        exact("backup digest", &survey.database.digest, &backup.digest)?;
        let record = survey
            .stage
            .parent()
            .expect("migration stage has a fingerprint parent")
            .join("receipt.json");
        let receipt = Receipt {
            schema: RECEIPT.to_string(),
            source: survey,
            database: super::Evidence {
                path: self.seat.database(),
                bytes: staged.bytes,
                digest: staged.digest,
            },
            backup,
            record,
        };
        storage::record(&receipt)?;
        if let Err(error) = storage::activate(&plan.staged.path, &self.seat.database()) {
            let _ = std::fs::remove_file(&receipt.record);
            return Err(error);
        }
        if let Err(error) = self.verify(&self.seat.database()).await {
            storage::restore(&receipt.backup.path, &self.seat.database())?;
            let _ = std::fs::remove_file(&receipt.record);
            return Err(Error::typed(
                "concord.migration.activation",
                format!("activated estate failed verification and was rolled back: {error}"),
            ));
        }
        let current = storage::evidence(&self.seat.database())?;
        exact(
            "activated database digest",
            &receipt.database.digest,
            &current.digest,
        )?;
        unlock(&guard)?;
        Ok(receipt)
    }

    pub async fn rollback(&self, receipt: &Receipt) -> Result<Rollback> {
        self.receipt(receipt)?;
        let guard = self.guard()?;
        let current = storage::evidence(&self.seat.database())?;
        exact(
            "activated database digest",
            &receipt.database.digest,
            &current.digest,
        )?;
        let backup = storage::evidence(&receipt.backup.path)?;
        exact(
            "backup digest",
            &receipt.source.database.digest,
            &backup.digest,
        )?;
        storage::restore(&backup.path, &self.seat.database())?;
        self.legacy(&self.seat.database()).await?;
        let restored = storage::evidence(&self.seat.database())?;
        let rollback = Rollback {
            schema: RECEIPT.to_string(),
            source: SOURCE.to_string(),
            restored,
        };
        unlock(&guard)?;
        Ok(rollback)
    }

    async fn inspect(&self) -> Result<Survey> {
        storage::regular(&self.seat.root)?;
        storage::regular(&self.seat.database())?;
        storage::regular(&self.seat.sudo())?;
        storage::settled(&self.seat.database())?;
        self.legacy(&self.seat.database()).await?;
        let database = storage::evidence(&self.seat.database())?;
        let sudo = storage::evidence(&self.seat.sudo())?;
        let fingerprint = storage::fingerprint(&database, &sudo);
        let root = self.seat.root.join("migration").join(TARGET);
        let stage = root.join(&fingerprint).join("stage");
        let backup = root.join(&fingerprint).join("backup");
        let required = database
            .bytes
            .checked_mul(3)
            .and_then(|bytes| bytes.checked_add(sudo.bytes * 2))
            .ok_or_else(|| Error::typed("concord.migration.space", "migration size overflows"))?;
        let available = fs2::available_space(&self.seat.root)?;
        if available < required {
            return Err(Error::typed(
                "concord.migration.space",
                format!("migration requires {required} bytes, only {available} are available"),
            ));
        }
        Ok(Survey {
            schema: SCHEMA.to_string(),
            source: SOURCE.to_string(),
            target: TARGET.to_string(),
            fingerprint,
            database,
            sudo,
            stage,
            backup,
            required,
            available,
        })
    }

    async fn legacy(&self, database: &Path) -> Result<()> {
        let sudo = std::fs::read_to_string(self.seat.sudo())?;
        let wire = Sqlite::file(database).await.map_err(fault)?;
        let held = keel::bootstrap(legacy(), wire).map_err(fault)?;
        let core = held.seal(&sudo).await.map_err(|error| {
            Error::typed(
                "concord.migration.source",
                format!("estate is not the exact {SOURCE} source model: {error}"),
            )
        })?;
        let spaces = core.live("Space").await.map_err(fault)?;
        if spaces.len() != 1 {
            return Err(Error::typed(
                "concord.migration.source",
                format!("source estate needs one Space, found {}", spaces.len()),
            ));
        }
        Ok(())
    }

    async fn evolve(&self, database: &Path) -> Result<()> {
        bind(database, bridge).await?;
        bind(database, released).await?;
        Ok(())
    }

    async fn verify(&self, database: &Path) -> Result<()> {
        let sudo = std::fs::read_to_string(self.seat.sudo())?;
        let wire = Sqlite::file(database).await.map_err(fault)?;
        let held = keel::bootstrap(released(), wire).map_err(fault)?;
        let core = held.seal(&sudo).await.map_err(upgrade)?;
        let estate = Estate {
            core,
            space: self.seat.space.clone(),
            anchors: false,
        };
        estate.verify().await?;
        let agreement = estate.inspect(None, None).await?;
        if !agreement.agrees() {
            return Err(Error::detailed(
                "concord.migration.agreement",
                "staged estate audit found agreement faults",
                serde_json::to_value(agreement).map_err(|error| Error::new(error.to_string()))?,
            ));
        }
        Ok(())
    }

    fn guard(&self) -> Result<File> {
        let path = self.seat.space.join(".concord.lock");
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)?;
        crate::path::at(&path).mode(0o600)?;
        file.lock_exclusive()?;
        Ok(file)
    }
}

pub(in crate::estate::model) async fn bind(
    database: &Path,
    model: fn() -> keel::Graph,
) -> Result<keel::Core<Sqlite>> {
    for attempt in 0..8 {
        align();
        let wire = Sqlite::file(database).await.map_err(fault)?;
        match keel::bind(model(), wire).await {
            Ok(core) => {
                return Ok(core);
            }
            Err(keel::adapt::Error::Adapt(note))
                if note == "lease is not the past" && attempt < 7 => {}
            Err(error) => return Err(upgrade(error)),
        }
    }
    unreachable!("bounded migration retries either return or fail")
}

fn unlock(guard: &File) -> Result<()> {
    FileExt::unlock(guard).map_err(|error| {
        Error::typed(
            "concord.migration.unlock",
            format!("cannot release Space migration lock: {error}"),
        )
    })
}
