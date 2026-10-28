use std::collections::HashMap;
use std::sync::LazyLock;
static NAMES: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| {
    include_str!("npc_names_ptbr.tsv").lines().filter_map(|line| line.split_once('\t')).collect()
});
pub fn display_name(name: &str) -> String {
    let base = name.split('#').next().unwrap_or(name);
    NAMES.get(base).copied().unwrap_or(base).to_owned()
}
#[cfg(test)] mod tests {
    use super::*;
    #[test] fn portuguese_names_keep_personal_names_and_hide_suffixes() {
        assert_eq!(display_name("Kafra Employee#nv"), "Funcionária Kafra");
        assert_eq!(display_name("Test Examiner#nv1"), "Examinador");
        assert_eq!(display_name("Hanson#nv"), "Hanson");
        assert_eq!(display_name("Somatology Instructor"), "Instrutor de Atributos");
    }
}
