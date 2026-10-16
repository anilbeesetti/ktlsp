use ktlsp_oracle_replay::recorder::confinement as child_confinement;

fn confinement_abi(profile:&child_confinement::Profile) -> Result<u32> {
    let abi=child_confinement::abi()?;
    if abi<profile.minimum_abi{return Err("actual host Landlock ABI lacks mandatory filesystem/signal/abstract-Unix custody; no START".into());}
    Ok(abi)
}
fn measure_child_confinement(dir:&Path,a:&Admission,e:&mut Evidence)->Result<()> {
    let grant:RootGrant=load(&dir.join("root-authorization.json"))?;
    let profile=child_confinement::profile();
    let landlock_abi=confinement_abi(&profile)?;
    let measurement=ktlsp_oracle_replay::authority::ConfinementMeasurement{
        schema_version:1,scope:"PR174_HOST_CHILD_CONFINEMENT_PROFILE".into(),kind:"child-confinement-preflight".into(),
        nonce:a.nonce.clone(),boot_id:a.boot_id.clone(),daemon_id:a.daemon_id.clone(),
        profile,landlock_abi,containment_template_sha256:containment_template_sha256()?,
        source_head:a.run.source_head.clone(),source_tree:a.run.source_tree.clone(),host_binary_sha256:a.host_binary_sha256.clone(),adapter_sha256:a.adapter_sha256.clone(),peer_sha256:grant.peer_sha256.clone(),
    };
    measurement.validate(&grant,&a.nonce,&a.boot_id,&a.daemon_id)?;
    side_json(e,&dir.join("child-confinement-profile.json"),&measurement)?;
    e.append(&serde_json::to_value(&measurement).map_err(|e|e.to_string())?,false)
}
