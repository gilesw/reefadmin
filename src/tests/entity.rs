use super::*;

#[test]
fn labels_from_names() {
    assert_eq!(default_label("rule_templates"), "Rule templates");
    assert_eq!(default_singular("rule_templates"), "rule template");
    assert_eq!(default_singular("categories"), "category");
    assert_eq!(default_singular("address"), "address");
}
