use super::super::evaluate::World;
use serde_json::json;

#[test]
fn metadata() {
    let value =
        json!({"nodes":[{"field":{"id":"IFSS_phase"},"optionId":"IFSSO_v","name":"Verify"}]});
    for stamp in ["2026-09-30T00:00:00Z", "2026-10-01T00:00:00Z"] {
        let mut world = World::new();
        for page in [&mut world.page, &mut world.fresh, &mut world.relations] {
            let issue = &mut page["data"]["repository"]["issue"];
            issue["updated"] = stamp.into();
            issue["issueFieldValues"] = value.clone();
        }
        let current = world.read();
        assert_eq!(current["verdict"], "satisfied");
        assert_eq!(current["checks"][2]["fact"]["verdict"], "satisfied");
        world.fresh["data"]["repository"]["issue"]["edited"] = "2026-10-02T00:00:00Z".into();
        assert_eq!(world.read()["checks"][2]["fact"]["verdict"], "unmet");
    }
}
