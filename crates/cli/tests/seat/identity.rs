use super::spawn;

#[test]
fn identity() {
    let scratch = tempfile::tempdir().expect("scratch");
    let command = spawn::concord(scratch.path());
    let source = std::path::Path::new(command.get_program());
    let bytes = std::fs::read(source).expect("executable");
    let (origin, held) = plumb::identity::inspect(&bytes).expect("reserved identity");
    assert_eq!(origin.prefix, "CONCORD");
    assert!(held.is_none());
    for marker in ["v0.13.0-beta.2", "v0.13.0"] {
        let binding = plumb::identity::Binding {
            product: "concord".into(),
            marker: marker.into(),
            digest: "a".repeat(64),
            commit: "b".repeat(40),
            workload: "c".repeat(64),
        };
        let bound = plumb::identity::bind(&bytes, &binding).expect("bind copy");
        assert_eq!(
            plumb::identity::inspect(&bound).expect("read binding"),
            (origin.clone(), Some(binding.clone()))
        );
        assert_eq!(plumb::identity::bind(&bound, &binding).unwrap(), bound);
        let mut other = binding;
        other.marker = "v0.13.1".into();
        assert!(plumb::identity::bind(&bound, &other).is_err());
        #[cfg(target_os = "linux")]
        probe(scratch.path(), source, marker, &bound);
    }
    assert_eq!(std::fs::read(source).unwrap(), bytes);
}

#[cfg(target_os = "linux")]
fn probe(scratch: &std::path::Path, source: &std::path::Path, marker: &str, bytes: &[u8]) {
    let path = scratch.join(marker);
    std::fs::write(&path, bytes).expect("bound copy");
    std::fs::set_permissions(&path, std::fs::metadata(source).unwrap().permissions())
        .expect("executable permissions");
    let output = spawn::image(scratch, &path)
        .arg("--version")
        .output()
        .expect("run bound copy");
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        format!("concord {marker}")
    );
}
