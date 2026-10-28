// RagSWITCH Pre-Renewal: no AP/traits; all playable classic jobs cap at Base 99.
pub fn has_ap(_job: u16) -> bool { false }
pub fn max_base_level(_job: u16) -> u16 { 99 }
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn classic_level_and_ap_limits() {
        for job in [0, 1, 7, 4008, 23, 24, 25, 4046, 4049] {
            assert_eq!(max_base_level(job), 99);
            assert!(!has_ap(job));
        }
    }
}
