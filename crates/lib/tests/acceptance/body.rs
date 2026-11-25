use concord_core::acceptance::{checkboxes, sections};

#[test]
fn markup() {
    let body = "## Acceptance\n- [x] real\n```md\n- [ ] example\n## Outcome\nfake\n```\n> - [ ] quotation\n    \n## Outcome\nactual\n";
    let sections = sections(body);
    assert_eq!(sections["outcome"], "actual");
    assert_eq!(checkboxes(&sections["acceptance"]), (1, 0));
}

#[test]
fn duplicate() {
    let sections = sections(
        "## Acceptance\n- [ ] first\n## Acceptance\n- [x] second\n## Acceptance\n- [x] third\n",
    );
    assert!(sections["acceptance"].is_empty());
    assert_eq!(checkboxes(&sections["acceptance"]), (0, 0));
}

#[test]
fn quoted() {
    let sections = sections("> ## Acceptance\n> - [x] quoted\n\n## Outcome\nactual\n");
    assert!(!sections.contains_key("acceptance"));
    assert_eq!(checkboxes("[x] not a task list\n- [ ] real\n"), (1, 1));
}

#[test]
fn nested() {
    let sections =
        sections("## Acceptance\n- [x] parent\n  - [ ] child\n### Detail\n- [x] detail\n");
    assert_eq!(checkboxes(&sections["acceptance"]), (3, 1));
}

#[test]
fn forms() {
    let task = sections(
        "### Outcome\nDeliver parser\n\n### Scope\nSection parser\n\n### Acceptance\n- [x] H3 parsed\n#### Detail\n- [ ] extra\n\n### Non-goals\nNo relaxation\n",
    );
    assert_eq!(task["outcome"], "Deliver parser");
    assert_eq!(task["scope"], "Section parser");
    assert_eq!(task["non-goals"], "No relaxation");
    assert_eq!(checkboxes(&task["acceptance"]), (2, 1));
    assert!(!task.contains_key("detail"));

    let feature = sections(
        "### Problem\nForm rejected\n\n### Outcome\nAccept forms\n\n### Acceptance\n- [x] covered\n\n### Non-goals\nNo change\n",
    );
    assert_eq!(feature["problem"], "Form rejected");
    assert_eq!(feature["outcome"], "Accept forms");
    assert_eq!(feature["non-goals"], "No change");
    assert_eq!(checkboxes(&feature["acceptance"]), (1, 0));

    let bug = sections(
        "### Problem\nRefused\n\n### Evidence\nError output\n\n### Expected outcome\nAccepted cleanly\n\n### Acceptance\n- [ ] fixed\n\n### Non-goals\nNo bypass\n",
    );
    assert_eq!(bug["problem"], "Refused");
    assert_eq!(bug["evidence"], "Error output");
    assert_eq!(bug["expected outcome"], "Accepted cleanly");
    assert_eq!(bug["non-goals"], "No bypass");
    assert_eq!(checkboxes(&bug["acceptance"]), (1, 1));
}

#[test]
fn repeated() {
    let sections = sections(
        "### Acceptance\n- [ ] first\n### Acceptance\n- [x] second\n### Outcome\nfirst\n### Outcome\nsecond\n",
    );
    assert!(sections["acceptance"].is_empty());
    assert_eq!(checkboxes(&sections["acceptance"]), (0, 0));
    assert!(sections["outcome"].is_empty());
}

#[test]
fn ignored() {
    let body = "> ## Outcome\n> quoted\n\n```md\n## Outcome\nfenced\n### Acceptance\n- [ ] fake\n```\n\n> ### Acceptance\n> - [x] quoted\n\n### Outcome\nactual\n";
    let parsed = sections(body);
    assert_eq!(parsed["outcome"], "actual");
    assert!(!parsed.contains_key("acceptance"));
    assert!(sections("> ### Outcome\n> quoted\n").is_empty());
}

#[test]
fn precedence() {
    let parsed = sections("### Intro\nignored\n\n## Outcome\nactual\n### Detail\nkept\n");
    assert!(!parsed.contains_key("intro"));
    assert!(!parsed.contains_key("detail"));
    assert_eq!(parsed["outcome"], "actual\n### Detail\nkept");
}
