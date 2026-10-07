mod acceptance {
    mod body;
    mod evaluation;
    mod history;
    mod marker;
    mod plan;
    mod report;
    mod validation;
}

use concord_core::acceptance::{
    Comment, Judgment, Marker, Reference, Target, Verification, digest,
};

fn declaration(target: Target) -> Marker {
    Marker::Declaration {
        target,
        promise: "Merge the bounded change with its applicable checks settled.".into(),
    }
}

fn comment(node: &str, marker: &Marker) -> Comment {
    Comment {
        node: node.into(),
        body: format!("Acceptance evidence\n\n{}", marker.render().unwrap()),
    }
}

fn reference(comment: &Comment) -> Reference {
    Reference {
        node: comment.node.clone(),
        digest: digest(&comment.body),
    }
}

fn closure(declaration: &Comment, target: Target) -> Marker {
    Marker::Closure {
        target,
        declaration: reference(declaration),
        judgment: Judgment::Satisfied,
        verification: Verification::Manual,
        review: digest("exact observed Issue review basis"),
        evidence: vec!["https://github.com/PerishLab/concord/pull/123".into()],
        remaining: Vec::new(),
        release: None,
    }
}
