//! Our English for the sign boards' texts: the pinned data writes them in Korean and nothing of rAthena's names them
//! (our own wording; the towns and places by their usual English names). Keyed by the data's text, blanks trimmed.

const ENGLISH: &[(&str, &str)] = &[
    // the table's default, for a board without a text
    ("입력안하셨습니다", "No text"),
    ("메가 부스터 지원", "Mega Booster Support"),
    ("부스터일루시온인챈트", "Booster Illusion Enchant"),
    ("부스터오토매틱인챈트", "Booster Automatic Enchant"),
    ("모험가 편의센터", "Adventurer Service Center"),
    ("마일리지 스톤교환", "Mileage Stone Exchange"),
    ("차원 수호 기념관", "Dimension Guardian Memorial"),
    ("180Lv미만 출입금지", "No Entry Below Lv 180"),
    ("190Lv미만 출입금지", "No Entry Below Lv 190"),
    ("표본 환경 - 혹한", "Specimen Habitat: Frost"),
    ("표본 환경 - 화염", "Specimen Habitat: Flame"),
    ("표본 환경 - 초원", "Specimen Habitat: Meadow"),
    ("표본 환경 - 죽음", "Specimen Habitat: Death"),
    ("표본 환경 - 성전", "Specimen Habitat: Sanctum"),
    ("표본 환경 - 맹독", "Specimen Habitat: Venom"),
    ("표본 환경 - 영혼", "Specimen Habitat: Soul"),
    ("표본환경-맹독", "Specimen Habitat: Venom"),
    ("표본환경-혹한", "Specimen Habitat: Frost"),
    ("바이오스피어-대지", "Biosphere: Earth"),
    ("바이오스피어-화산", "Biosphere: Volcano"),
    ("파라마켓", "Para Market"),
    ("기본 장비 지원", "Basic Gear Support"),
    ("중급 고급 무기 지원", "Weapon Support"),
    ("고급 방어구 지원", "Armor Support"),
    ("쉐도우 장비 대여", "Shadow Gear Rental"),
    ("낙원단 공간이동사", "Eden Teleporter"),
    ("1차 전직 안내", "First Job Guide"),
    ("여객선 승선", "Board the Ship"),
    ("알베르타행", "To Alberta"),
    ("알베르타 행", "To Alberta"),
    ("난파선행", "To the Shipwreck"),
    ("이즈루드행", "To Izlude"),
    ("자와이행", "To Jawaii"),
    ("루티에행", "To Lutie"),
    ("오딘신전 행", "To Odin Temple"),
    ("휘겔 행", "To Hugel"),
    ("용지성 행", "To Louyang"),
    ("아마쯔 행", "To Amatsu"),
    ("아요타야 행", "To Ayothaya"),
    ("브라질리스 행", "To Brasilis"),
    ("쿤룬 행", "To Kunlun"),
    ("포트말라야 행", "To Port Malaya"),
    ("데와타 행", "To Dewata"),
    ("모스코비아 행", "To Moscovia"),
    ("락 릿지 행", "To Rock Ridge"),
    ("거북섬 행", "To Turtle Island"),
    ("라자냐 행", "To Lasagna"),
    ("말랑도 행", "To Malangdo"),
    ("소드 클랜", "Sword Clan"),
    ("아크완드 클랜", "Arch Wand Clan"),
    ("골든메이스 클랜", "Golden Mace Clan"),
    ("크로스보우 클랜", "Crossbow Clan"),
    ("결혼 신청", "Marriage"),
    ("독립 도우미", "Independence Helper"),
    ("비공정 출구", "Airship Exit"),
    ("유노-라헬 행", "To Juno, Rachel"),
    ("유노-이즈루드 행", "To Juno, Izlude"),
    ("이즈루드-라헬 행", "To Izlude, Rachel"),
    ("슈발츠발드 국내선", "Schwarzwald Line"),
    ("등급강화소", "Grade Enhancement"),
    ("미용 쿠폰,코스튬 변경 티켓", "Beauty, Costume Tickets"),
    ("일루시온 패스", "Illusion Pass"),
    ("현자의 유산 패스", "Sage's Legacy Pass"),
    ("기도의 방향 패스", "Way of Prayer Pass"),
];

/// `text` in our English, its leading blanks kept (they place it on the board); `None` for a text we have none for
pub fn english(text: &str) -> Option<String> {
    let trimmed = text.trim();
    let lead = &text[..text.len() - text.trim_start().len()];
    ENGLISH
        .iter()
        .find(|(korean, _)| *korean == trimmed)
        .map(|(_, english)| format!("{lead}{english}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_text_is_found_with_its_blanks_and_kept_leading() {
        assert_eq!(english("  소드 클랜 ").as_deref(), Some("  Sword Clan"));
        assert_eq!(english("모르는 글"), None);
    }

    #[test]
    fn each_korean_text_is_listed_once() {
        let mut seen = std::collections::HashSet::new();
        for (korean, english) in ENGLISH {
            assert!(seen.insert(korean), "{korean} twice");
            assert!(
                !english.is_empty() && english.is_ascii(),
                "{korean}: {english}"
            );
        }
    }
}
