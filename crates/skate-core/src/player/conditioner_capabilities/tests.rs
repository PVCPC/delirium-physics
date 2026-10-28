use super::ConditionerCapabilityContext;

#[test]
fn complete_source_capability_branch_table() {
    let source_words = [
        0x0000_01c0,
        0x0000_01c0,
        0x0000_01c0,
        0xffff_fff7,
        0xffff_f7f7,
        0xffff_f7f7,
        0x0000_01c0,
        0xffff_fff7,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
    ];
    for (combination, expected) in source_words.into_iter().enumerate() {
        let context = ConditionerCapabilityContext {
            in_front_end: combination & 8 != 0,
            hall_of_meat_enabled: combination & 4 != 0,
            challenge_query_active: combination & 2 != 0,
            challenge_configuration_enabled: combination & 1 != 0,
        };
        assert_eq!(context.capabilities(), expected, "{context:?}");
    }
}
