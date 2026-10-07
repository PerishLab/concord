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
