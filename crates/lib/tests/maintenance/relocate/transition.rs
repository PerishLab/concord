use super::{
    Intent, Ready, Relocate, Result, Seat, control, files, git, journal, member, receipt, refuse,
};
use std::fs;
use std::os::unix::fs::PermissionsExt;

impl Relocate {
    pub(super) async fn stage(&self) -> Result<()> {
        control::absent(&self.home.join("workspace"))?;
        control::absent(&journal::root(&self.home))?;
        control::absent(&self.home.join("state/cutover"))?;
        control::absent(&self.frozen())?;
        let files = files::inventory(&self.source.join(".concord"))?;
        control::supported(&files)?;
        let payload = journal::payload(&self.source)?;
        if let Some(payload) = &payload {
            payload.links(&self.source.join(".issues"))?;
        }
        let intent = Intent {
            schema: "concord.member-relocation/v1".into(),
            source: self.source.clone(),
            home: self.home.clone(),
            files,
            payload,
        };
        let state = self.home.join("state");
        if !state.try_exists()? {
            fs::create_dir(&state)?;
        }
        files::directory(&state)?;
        fs::create_dir(journal::root(&self.home))?;
        fs::set_permissions(journal::root(&self.home), fs::Permissions::from_mode(0o700))?;
        journal::write(&journal::root(&self.home).join("intent.json"), &intent)?;
        fs::rename(self.source.join(".concord/sudo"), self.frozen())?;
        journal::unchanged(&intent)?;
        let staged = journal::stage(&self.home);
        fs::create_dir(&staged)?;
        drop(Seat::new(&staged).bootstrap().await?);
        fs::copy(
            self.source.join(".concord/estate.sqlite3"),
            staged.join("workspace/estate.sqlite3"),
        )?;
        fs::copy(self.frozen(), staged.join("workspace/sudo"))?;
        let estate = Seat::new(&staged).open().await?;
        if !estate.structure().await?.agrees() {
            return Err(refuse("source control model refuses"));
        }
        let members = member::capture(&estate, &self.source).await?;
        let nodes = estate
            .issues()
            .await?
            .into_iter()
            .map(|anchor| anchor.node)
            .collect::<Vec<_>>();
        if let Some(payload) = &intent.payload {
            payload.qualify(&nodes, &members)?;
        }
        for integration in estate.integrations().await? {
            member::repository(&integration).await?;
        }
        let space = estate.identity().to_string();
        drop(estate);
        fs::rename(
            staged.join("workspace/sudo"),
            journal::possession(&self.home),
        )?;
        let ready = Ready {
            space,
            files: control::inventory(&staged.join("workspace"))?,
            members,
            git: git::Git::new(&self.home)?.tool()?,
        };
        journal::unchanged(&intent)?;
        journal::write(&journal::root(&self.home).join("ready.json"), &ready)?;
        Ok(())
    }

    pub(super) async fn activate(&self) -> Result<()> {
        let intent = journal::intent(&self.source, &self.home)?;
        journal::controls(&intent)?;
        let ready = super::Ready::read(&self.home)?;
        let workspace = self.home.join("workspace");
        let staged = journal::stage(&self.home);
        if !workspace.try_exists()? {
            journal::unchanged(&intent)?;
            control::exact(&staged, &ready)?;
            control::absent(&staged.join("workspace/sudo"))?;
            control::authority(&intent, &journal::possession(&self.home))?;
            for member in &ready.members {
                member
                    .verify(
                        &self.source,
                        &member.path(&self.source.join(".issues")),
                        false,
                    )
                    .await?;
            }
            fs::rename(staged.join("workspace"), &workspace)?;
        } else {
            control::absent(&staged.join("workspace"))?;
        }
        control::exact(&self.home, &ready)?;
        let old = self.source.join(".issues");
        let target = workspace.join("issues");
        if let Some(payload) = &intent.payload {
            if old.try_exists()? {
                control::absent(&target)?;
                control::absent(&workspace.join("sudo"))?;
                payload.check(&old)?;
                fs::rename(&old, &target)?;
            }
            payload.check(&target)?;
        } else {
            control::absent(&old)?;
            control::absent(&target)?;
        }
        let published = workspace.join("sudo").try_exists()?;
        for member in &ready.members {
            member::repository(&member.integration).await?;
            if published {
                member
                    .verify(&self.source, &member.path(&target), true)
                    .await?;
            } else {
                member.repair(&self.source, &target).await?;
            }
        }
        if let Some(payload) = &intent.payload {
            payload.check(&target)?;
        }
        control::exact(&self.home, &ready)?;
        if published {
            control::absent(&journal::possession(&self.home))?;
            control::authority(&intent, &workspace.join("sudo"))?;
        } else {
            control::authority(&intent, &journal::possession(&self.home))?;
            fs::rename(journal::possession(&self.home), workspace.join("sudo"))?;
        }
        control::validate(&self.home, &ready).await
    }

    pub(super) fn abort(&self) -> Result<()> {
        control::absent(&self.home.join("workspace"))?;
        let intent = journal::intent(&self.source, &self.home)?;
        if self.frozen().try_exists()? {
            journal::unchanged(&intent)?;
        } else if files::inventory(&self.source.join(".concord"))? != intent.files
            || journal::payload(&self.source)? != intent.payload
        {
            return Err(refuse("source changed before freeze"));
        }
        control::discard(&intent)?;
        let possession = journal::possession(&self.home);
        if possession.try_exists()? {
            control::authority(&intent, &possession)?;
            fs::remove_file(possession)?;
        }
        if self.frozen().try_exists()? {
            fs::rename(self.frozen(), self.source.join(".concord/sudo"))?;
        }
        control::cleanup(&self.home)
    }

    pub(super) async fn finish(&self) -> Result<()> {
        if receipt::completed(self).await? {
            return Ok(());
        }
        let intent = journal::intent(&self.source, &self.home)?;
        let ready = super::Ready::read(&self.home)?;
        control::validate(&self.home, &ready).await?;
        control::authority(&intent, &self.home.join("workspace/sudo"))?;
        control::absent(&journal::possession(&self.home))?;
        control::absent(&self.source.join(".issues"))?;
        if let Some(payload) = &intent.payload {
            payload.check(&self.home.join("workspace/issues"))?;
        }
        for member in &ready.members {
            member
                .verify(
                    &self.source,
                    &member.path(&self.home.join("workspace/issues")),
                    true,
                )
                .await?;
        }
        receipt::erase(&intent)?;
        let staged = journal::stage(&self.home);
        if staged.try_exists()? {
            fs::remove_dir(staged)?;
        }
        receipt::publish(self, &ready.space)?;
        receipt::cleanup(&self.home)
    }
}
