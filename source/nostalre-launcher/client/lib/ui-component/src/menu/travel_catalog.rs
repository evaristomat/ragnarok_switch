// RagSWITCH Pre-Renewal: destinations from active classic NPC/warp/spawn scripts.
// Original Warper by Euphy and rAthena contributors, GPL.
use super::travel::{Destination, TravelGroup};
pub(super) static GROUPS: &[TravelGroup] = &[
    TravelGroup {
        category: 0,
        label: "Acampamento da Expedição",
        destinations: &[
            Destination {
                label: "Acampamento da Expedição",
                map: "mid_camp",
                x: 180, y: 240, go: Some(26),
            },
        ],
    },
    TravelGroup {
        category: 0,
        label: "Acampamento de Thor",
        destinations: &[
            Destination {
                label: "Acampamento de Thor",
                map: "thor_camp",
                x: 246, y: 68, go: None,
            },
        ],
    },
    TravelGroup {
        category: 0,
        label: "Alberta",
        destinations: &[
            Destination {
                label: "Alberta",
                map: "alberta",
                x: 192, y: 147, go: Some(4),
            },
        ],
    },
    TravelGroup {
        category: 0,
        label: "Aldebaran",
        destinations: &[
            Destination {
                label: "Aldebaran",
                map: "aldebaran",
                x: 140, y: 131, go: Some(6),
            },
        ],
    },
    TravelGroup {
        category: 0,
        label: "Amatsu",
        destinations: &[
            Destination {
                label: "Amatsu",
                map: "amatsu",
                x: 198, y: 84, go: Some(10),
            },
        ],
    },
    TravelGroup {
        category: 0,
        label: "Ayothaya",
        destinations: &[
            Destination {
                label: "Ayothaya",
                map: "ayothaya",
                x: 151, y: 117, go: Some(18),
            },
        ],
    },
    TravelGroup {
        category: 0,
        label: "Brasilis",
        destinations: &[
            Destination {
                label: "Brasilis",
                map: "brasilis",
                x: 182, y: 239, go: Some(29),
            },
        ],
    },
    TravelGroup {
        category: 0,
        label: "Comodo",
        destinations: &[
            Destination {
                label: "Comodo",
                map: "comodo",
                x: 209, y: 143, go: Some(8),
            },
        ],
    },
    TravelGroup {
        category: 0,
        label: "Einbech",
        destinations: &[
            Destination {
                label: "Einbech",
                map: "einbech",
                x: 70, y: 95, go: Some(21),
            },
        ],
    },
    TravelGroup {
        category: 0,
        label: "Einbroch",
        destinations: &[
            Destination {
                label: "Einbroch",
                map: "einbroch",
                x: 64, y: 200, go: Some(19),
            },
        ],
    },
    TravelGroup {
        category: 0,
        label: "Geffen",
        destinations: &[
            Destination {
                label: "Geffen",
                map: "geffen",
                x: 119, y: 59, go: Some(2),
            },
        ],
    },
    TravelGroup {
        category: 0,
        label: "Hugel",
        destinations: &[
            Destination {
                label: "Hugel",
                map: "hugel",
                x: 96, y: 145, go: Some(22),
            },
        ],
    },
    TravelGroup {
        category: 0,
        label: "Ilha Esquecida (dia)",
        destinations: &[
            Destination {
                label: "Ilha Esquecida (dia)",
                map: "nameless_i",
                x: 256, y: 215, go: None,
            },
        ],
    },
    TravelGroup {
        category: 0,
        label: "Ilha Esquecida (noite)",
        destinations: &[
            Destination {
                label: "Ilha Esquecida (noite)",
                map: "nameless_n",
                x: 256, y: 215, go: None,
            },
        ],
    },
    TravelGroup {
        category: 0,
        label: "Izlude",
        destinations: &[
            Destination {
                label: "Izlude",
                map: "izlude",
                x: 128, y: 114, go: Some(5),
            },
        ],
    },
    TravelGroup {
        category: 0,
        label: "Jawaii",
        destinations: &[
            Destination {
                label: "Jawaii",
                map: "jawaii",
                x: 249, y: 127, go: Some(17),
            },
        ],
    },
    TravelGroup {
        category: 0,
        label: "Juno (Yuno)",
        destinations: &[
            Destination {
                label: "Juno (Yuno)",
                map: "yuno",
                x: 157, y: 51, go: Some(9),
            },
        ],
    },
    TravelGroup {
        category: 0,
        label: "Kunlun",
        destinations: &[
            Destination {
                label: "Kunlun",
                map: "gonryun",
                x: 160, y: 120, go: Some(11),
            },
        ],
    },
    TravelGroup {
        category: 0,
        label: "Lighthalzen",
        destinations: &[
            Destination {
                label: "Lighthalzen",
                map: "lighthalzen",
                x: 158, y: 92, go: Some(20),
            },
        ],
    },
    TravelGroup {
        category: 0,
        label: "Luoyang",
        destinations: &[
            Destination {
                label: "Luoyang",
                map: "louyang",
                x: 217, y: 40, go: Some(14),
            },
        ],
    },
    TravelGroup {
        category: 0,
        label: "Lutie",
        destinations: &[
            Destination {
                label: "Lutie",
                map: "xmas",
                x: 147, y: 134, go: Some(7),
            },
        ],
    },
    TravelGroup {
        category: 0,
        label: "Manuk",
        destinations: &[
            Destination {
                label: "Manuk",
                map: "manuk",
                x: 282, y: 138, go: Some(27),
            },
        ],
    },
    TravelGroup {
        category: 0,
        label: "Morocc",
        destinations: &[
            Destination {
                label: "Morocc",
                map: "morocc",
                x: 156, y: 93, go: Some(1),
            },
        ],
    },
    TravelGroup {
        category: 0,
        label: "Moscovia",
        destinations: &[
            Destination {
                label: "Moscovia",
                map: "moscovia",
                x: 223, y: 184, go: Some(25),
            },
        ],
    },
    TravelGroup {
        category: 0,
        label: "Niflheim",
        destinations: &[
            Destination {
                label: "Niflheim",
                map: "niflheim",
                x: 21, y: 153, go: Some(13),
            },
        ],
    },
    TravelGroup {
        category: 0,
        label: "Payon",
        destinations: &[
            Destination {
                label: "Payon",
                map: "payon",
                x: 162, y: 233, go: Some(3),
            },
        ],
    },
    TravelGroup {
        category: 0,
        label: "Prontera",
        destinations: &[
            Destination {
                label: "Prontera",
                map: "prontera",
                x: 156, y: 191, go: Some(0),
            },
        ],
    },
    TravelGroup {
        category: 0,
        label: "Rachel",
        destinations: &[
            Destination {
                label: "Rachel",
                map: "rachel",
                x: 130, y: 110, go: Some(23),
            },
        ],
    },
    TravelGroup {
        category: 0,
        label: "Splendide",
        destinations: &[
            Destination {
                label: "Splendide",
                map: "splendide",
                x: 201, y: 147, go: Some(28),
            },
        ],
    },
    TravelGroup {
        category: 0,
        label: "Umbala",
        destinations: &[
            Destination {
                label: "Umbala",
                map: "umbala",
                x: 89, y: 157, go: Some(12),
            },
        ],
    },
    TravelGroup {
        category: 0,
        label: "Veins",
        destinations: &[
            Destination {
                label: "Veins",
                map: "veins",
                x: 216, y: 123, go: Some(24),
            },
        ],
    },
    TravelGroup {
        category: 1,
        label: "Abadia Amaldiçoada",
        destinations: &[
            Destination {
                label: "Andar 1",
                map: "abbey01",
                x: 51, y: 14, go: None,
            },
            Destination {
                label: "Andar 2",
                map: "abbey02",
                x: 150, y: 11, go: None,
            },
            Destination {
                label: "Andar 3",
                map: "abbey03",
                x: 120, y: 10, go: None,
            },
        ],
    },
    TravelGroup {
        category: 1,
        label: "Calabouço de Geffen",
        destinations: &[
            Destination {
                label: "Andar 1",
                map: "gef_dun00",
                x: 104, y: 99, go: None,
            },
            Destination {
                label: "Andar 2",
                map: "gef_dun01",
                x: 115, y: 236, go: None,
            },
            Destination {
                label: "Andar 3",
                map: "gef_dun02",
                x: 106, y: 132, go: None,
            },
            Destination {
                label: "Andar 4",
                map: "gef_dun03",
                x: 203, y: 200, go: None,
            },
        ],
    },
    TravelGroup {
        category: 1,
        label: "Caverna de Amatsu",
        destinations: &[
            Destination {
                label: "Andar 1",
                map: "ama_dun01",
                x: 228, y: 11, go: None,
            },
            Destination {
                label: "Andar 2",
                map: "ama_dun02",
                x: 34, y: 41, go: None,
            },
            Destination {
                label: "Andar 3",
                map: "ama_dun03",
                x: 119, y: 14, go: None,
            },
        ],
    },
    TravelGroup {
        category: 1,
        label: "Caverna de Brasilis",
        destinations: &[
            Destination {
                label: "Andar 1",
                map: "bra_dun01",
                x: 87, y: 47, go: None,
            },
            Destination {
                label: "Andar 2",
                map: "bra_dun02",
                x: 262, y: 262, go: None,
            },
        ],
    },
    TravelGroup {
        category: 1,
        label: "Caverna de Byalan",
        destinations: &[
            Destination {
                label: "Andar 1",
                map: "iz_dun00",
                x: 168, y: 168, go: None,
            },
            Destination {
                label: "Andar 2",
                map: "iz_dun01",
                x: 253, y: 252, go: None,
            },
            Destination {
                label: "Andar 3",
                map: "iz_dun02",
                x: 236, y: 204, go: None,
            },
            Destination {
                label: "Andar 4",
                map: "iz_dun03",
                x: 32, y: 63, go: None,
            },
            Destination {
                label: "Andar 5",
                map: "iz_dun04",
                x: 26, y: 27, go: None,
            },
        ],
    },
    TravelGroup {
        category: 1,
        label: "Caverna de Gelo",
        destinations: &[
            Destination {
                label: "Andar 1",
                map: "ice_dun01",
                x: 157, y: 14, go: None,
            },
            Destination {
                label: "Andar 2",
                map: "ice_dun02",
                x: 151, y: 155, go: None,
            },
            Destination {
                label: "Andar 3",
                map: "ice_dun03",
                x: 149, y: 22, go: None,
            },
            Destination {
                label: "Andar 4",
                map: "ice_dun04",
                x: 33, y: 158, go: None,
            },
        ],
    },
    TravelGroup {
        category: 1,
        label: "Caverna de Magma",
        destinations: &[
            Destination {
                label: "Andar 1",
                map: "mag_dun01",
                x: 126, y: 68, go: None,
            },
            Destination {
                label: "Andar 2",
                map: "mag_dun02",
                x: 47, y: 30, go: None,
            },
        ],
    },
    TravelGroup {
        category: 1,
        label: "Caverna de Payon",
        destinations: &[
            Destination {
                label: "Andar 1",
                map: "pay_dun00",
                x: 21, y: 183, go: None,
            },
            Destination {
                label: "Andar 2",
                map: "pay_dun01",
                x: 19, y: 33, go: None,
            },
            Destination {
                label: "Andar 3",
                map: "pay_dun02",
                x: 19, y: 63, go: None,
            },
            Destination {
                label: "Andar 4",
                map: "pay_dun03",
                x: 155, y: 159, go: None,
            },
            Destination {
                label: "Andar 5",
                map: "pay_dun04",
                x: 201, y: 204, go: None,
            },
        ],
    },
    TravelGroup {
        category: 1,
        label: "Caverna dos Orcs",
        destinations: &[
            Destination {
                label: "Andar 1",
                map: "orcsdun01",
                x: 32, y: 170, go: None,
            },
            Destination {
                label: "Andar 2",
                map: "orcsdun02",
                x: 21, y: 185, go: None,
            },
        ],
    },
    TravelGroup {
        category: 1,
        label: "Cavernas de Comodo",
        destinations: &[
            Destination {
                label: "Andar 1",
                map: "beach_dun",
                x: 266, y: 67, go: None,
            },
            Destination {
                label: "Andar 2",
                map: "beach_dun2",
                x: 255, y: 244, go: None,
            },
            Destination {
                label: "Andar 3",
                map: "beach_dun3",
                x: 23, y: 260, go: None,
            },
        ],
    },
    TravelGroup {
        category: 1,
        label: "Esfinge",
        destinations: &[
            Destination {
                label: "Andar 1",
                map: "in_sphinx1",
                x: 288, y: 9, go: None,
            },
            Destination {
                label: "Andar 2",
                map: "in_sphinx2",
                x: 149, y: 81, go: None,
            },
            Destination {
                label: "Andar 3",
                map: "in_sphinx3",
                x: 210, y: 54, go: None,
            },
            Destination {
                label: "Andar 4",
                map: "in_sphinx4",
                x: 10, y: 222, go: None,
            },
            Destination {
                label: "Andar 5",
                map: "in_sphinx5",
                x: 100, y: 99, go: None,
            },
        ],
    },
    TravelGroup {
        category: 1,
        label: "Esgotos de Prontera",
        destinations: &[
            Destination {
                label: "Andar 1",
                map: "prt_sewb1",
                x: 131, y: 247, go: None,
            },
            Destination {
                label: "Andar 2",
                map: "prt_sewb2",
                x: 19, y: 19, go: None,
            },
            Destination {
                label: "Andar 3",
                map: "prt_sewb3",
                x: 180, y: 169, go: None,
            },
            Destination {
                label: "Andar 4",
                map: "prt_sewb4",
                x: 100, y: 92, go: None,
            },
        ],
    },
    TravelGroup {
        category: 1,
        label: "Floresta Encantada de Moscovia",
        destinations: &[
            Destination {
                label: "Andar 1",
                map: "mosk_dun01",
                x: 189, y: 48, go: None,
            },
            Destination {
                label: "Andar 2",
                map: "mosk_dun02",
                x: 165, y: 30, go: None,
            },
            Destination {
                label: "Andar 3",
                map: "mosk_dun03",
                x: 32, y: 135, go: None,
            },
        ],
    },
    TravelGroup {
        category: 1,
        label: "Formigueiro Infernal",
        destinations: &[
            Destination {
                label: "Andar 1",
                map: "anthell01",
                x: 35, y: 262, go: None,
            },
            Destination {
                label: "Andar 2",
                map: "anthell02",
                x: 168, y: 170, go: None,
            },
        ],
    },
    TravelGroup {
        category: 1,
        label: "Fábrica de Brinquedos",
        destinations: &[
            Destination {
                label: "Andar 1",
                map: "xmas_dun01",
                x: 205, y: 15, go: None,
            },
            Destination {
                label: "Andar 2",
                map: "xmas_dun02",
                x: 129, y: 133, go: None,
            },
        ],
    },
    TravelGroup {
        category: 1,
        label: "Fábrica de Robôs",
        destinations: &[
            Destination {
                label: "Andar 1",
                map: "kh_dun01",
                x: 28, y: 226, go: None,
            },
            Destination {
                label: "Andar 2",
                map: "kh_dun02",
                x: 41, y: 198, go: None,
            },
        ],
    },
    TravelGroup {
        category: 1,
        label: "Gefenia",
        destinations: &[
            Destination {
                label: "Andar 1",
                map: "gefenia01",
                x: 40, y: 103, go: None,
            },
            Destination {
                label: "Andar 2",
                map: "gefenia02",
                x: 203, y: 34, go: None,
            },
            Destination {
                label: "Andar 3",
                map: "gefenia03",
                x: 266, y: 168, go: None,
            },
            Destination {
                label: "Andar 4",
                map: "gefenia04",
                x: 130, y: 272, go: None,
            },
        ],
    },
    TravelGroup {
        category: 1,
        label: "Glast Heim",
        destinations: &[
            Destination {
                label: "Entrada",
                map: "glast_01",
                x: 370, y: 304, go: None,
            },
            Destination {
                label: "Castle 1",
                map: "gl_cas01",
                x: 199, y: 29, go: None,
            },
            Destination {
                label: "Castle 2",
                map: "gl_cas02",
                x: 104, y: 25, go: None,
            },
            Destination {
                label: "Chivalry 1",
                map: "gl_knt01",
                x: 150, y: 15, go: None,
            },
            Destination {
                label: "Chivalry 2",
                map: "gl_knt02",
                x: 157, y: 287, go: None,
            },
            Destination {
                label: "Churchyard",
                map: "gl_chyard",
                x: 147, y: 15, go: None,
            },
            Destination {
                label: "Culvert 1",
                map: "gl_sew01",
                x: 258, y: 255, go: None,
            },
            Destination {
                label: "Culvert 2",
                map: "gl_sew02",
                x: 108, y: 291, go: None,
            },
            Destination {
                label: "Culvert 3",
                map: "gl_sew03",
                x: 171, y: 283, go: None,
            },
            Destination {
                label: "Culvert 4",
                map: "gl_sew04",
                x: 68, y: 277, go: None,
            },
            Destination {
                label: "St. Abbey",
                map: "gl_church",
                x: 156, y: 7, go: None,
            },
            Destination {
                label: "Staircase Dungeon",
                map: "gl_step",
                x: 12, y: 7, go: None,
            },
            Destination {
                label: "Underground Cave 1",
                map: "gl_dun01",
                x: 133, y: 271, go: None,
            },
            Destination {
                label: "Underground Cave 2",
                map: "gl_dun02",
                x: 224, y: 274, go: None,
            },
            Destination {
                label: "Underground Prison 1",
                map: "gl_prison",
                x: 14, y: 70, go: None,
            },
            Destination {
                label: "Underground Prison 2",
                map: "gl_prison1",
                x: 150, y: 14, go: None,
            },
        ],
    },
    TravelGroup {
        category: 1,
        label: "Ilha das Tartarugas",
        destinations: &[
            Destination {
                label: "Entrada",
                map: "tur_dun01",
                x: 154, y: 49, go: None,
            },
            Destination {
                label: "Turtle Dungeon 1",
                map: "tur_dun02",
                x: 148, y: 261, go: None,
            },
            Destination {
                label: "Turtle Dungeon 2",
                map: "tur_dun03",
                x: 132, y: 189, go: None,
            },
            Destination {
                label: "Turtle Dungeon 3",
                map: "tur_dun04",
                x: 100, y: 192, go: None,
            },
        ],
    },
    TravelGroup {
        category: 1,
        label: "Juperos",
        destinations: &[
            Destination {
                label: "Entrada",
                map: "jupe_cave",
                x: 140, y: 51, go: None,
            },
            Destination {
                label: "Juperos 1",
                map: "juperos_01",
                x: 53, y: 247, go: None,
            },
            Destination {
                label: "Juperos 2",
                map: "juperos_02",
                x: 37, y: 63, go: None,
            },
            Destination {
                label: "Núcleo",
                map: "jupe_core",
                x: 150, y: 285, go: None,
            },
        ],
    },
    TravelGroup {
        category: 1,
        label: "Labirinto da Floresta",
        destinations: &[
            Destination {
                label: "Andar 1",
                map: "prt_maze01",
                x: 99, y: 31, go: None,
            },
            Destination {
                label: "Andar 2",
                map: "prt_maze02",
                x: 93, y: 20, go: None,
            },
            Destination {
                label: "Andar 3",
                map: "prt_maze03",
                x: 182, y: 88, go: None,
            },
        ],
    },
    TravelGroup {
        category: 1,
        label: "Laboratório de Somatologia",
        destinations: &[
            Destination {
                label: "Bio Lab 1",
                map: "lhz_dun01",
                x: 150, y: 288, go: None,
            },
            Destination {
                label: "Bio Lab 2",
                map: "lhz_dun02",
                x: 150, y: 18, go: None,
            },
            Destination {
                label: "Bio Lab 3",
                map: "lhz_dun03",
                x: 140, y: 134, go: None,
            },
        ],
    },
    TravelGroup {
        category: 1,
        label: "Lago do Abismo",
        destinations: &[
            Destination {
                label: "Andar 1",
                map: "abyss_01",
                x: 261, y: 272, go: None,
            },
            Destination {
                label: "Andar 2",
                map: "abyss_02",
                x: 275, y: 270, go: None,
            },
            Destination {
                label: "Andar 3",
                map: "abyss_03",
                x: 116, y: 27, go: None,
            },
        ],
    },
    TravelGroup {
        category: 1,
        label: "Minas de Carvão",
        destinations: &[
            Destination {
                label: "Andar 1",
                map: "mjo_dun01",
                x: 52, y: 17, go: None,
            },
            Destination {
                label: "Andar 2",
                map: "mjo_dun02",
                x: 381, y: 343, go: None,
            },
            Destination {
                label: "Andar 3",
                map: "mjo_dun03",
                x: 302, y: 262, go: None,
            },
        ],
    },
    TravelGroup {
        category: 1,
        label: "Minas de Einbech",
        destinations: &[
            Destination {
                label: "Andar 1",
                map: "ein_dun01",
                x: 22, y: 14, go: None,
            },
            Destination {
                label: "Andar 2",
                map: "ein_dun02",
                x: 292, y: 290, go: None,
            },
        ],
    },
    TravelGroup {
        category: 1,
        label: "Navio Fantasma",
        destinations: &[
            Destination {
                label: "Andar 1",
                map: "treasure01",
                x: 69, y: 24, go: None,
            },
            Destination {
                label: "Andar 2",
                map: "treasure02",
                x: 102, y: 27, go: None,
            },
        ],
    },
    TravelGroup {
        category: 1,
        label: "Pirâmides",
        destinations: &[
            Destination {
                label: "Pyramids 1",
                map: "moc_pryd01",
                x: 192, y: 9, go: None,
            },
            Destination {
                label: "Pyramids 2",
                map: "moc_pryd02",
                x: 10, y: 192, go: None,
            },
            Destination {
                label: "Pyramids 3",
                map: "moc_pryd03",
                x: 100, y: 92, go: None,
            },
            Destination {
                label: "Pyramids 4",
                map: "moc_pryd04",
                x: 181, y: 11, go: None,
            },
            Destination {
                label: "Basement 1",
                map: "moc_pryd05",
                x: 94, y: 96, go: None,
            },
            Destination {
                label: "Basement 2",
                map: "moc_pryd06",
                x: 192, y: 8, go: None,
            },
        ],
    },
    TravelGroup {
        category: 1,
        label: "Raízes de Yggdrasil",
        destinations: &[
            Destination {
                label: "Andar 1",
                map: "nyd_dun01",
                x: 61, y: 239, go: None,
            },
            Destination {
                label: "Andar 2",
                map: "nyd_dun02",
                x: 60, y: 271, go: None,
            },
        ],
    },
    TravelGroup {
        category: 1,
        label: "Santuário de Rachel",
        destinations: &[
            Destination {
                label: "Andar 1",
                map: "ra_san01",
                x: 140, y: 11, go: None,
            },
            Destination {
                label: "Andar 2",
                map: "ra_san02",
                x: 32, y: 21, go: None,
            },
            Destination {
                label: "Andar 3",
                map: "ra_san03",
                x: 8, y: 149, go: None,
            },
            Destination {
                label: "Andar 4",
                map: "ra_san04",
                x: 204, y: 218, go: None,
            },
            Destination {
                label: "Andar 5",
                map: "ra_san05",
                x: 150, y: 9, go: None,
            },
        ],
    },
    TravelGroup {
        category: 1,
        label: "Subterrâneo de Umbala",
        destinations: &[
            Destination {
                label: "Carpenter's Shop in the Tree",
                map: "um_dun01",
                x: 42, y: 31, go: None,
            },
            Destination {
                label: "Passage to a Foreign World",
                map: "um_dun02",
                x: 48, y: 30, go: None,
            },
            Destination {
                label: "Hvergermil's Fountain",
                map: "yggdrasil01",
                x: 204, y: 78, go: None,
            },
        ],
    },
    TravelGroup {
        category: 1,
        label: "Templo de Ayothaya",
        destinations: &[
            Destination {
                label: "Ancient Shrine Maze",
                map: "ayo_dun01",
                x: 275, y: 19, go: None,
            },
            Destination {
                label: "Inside Ancient Shrine",
                map: "ayo_dun02",
                x: 24, y: 26, go: None,
            },
        ],
    },
    TravelGroup {
        category: 1,
        label: "Templo de Kunlun",
        destinations: &[
            Destination {
                label: "Andar 1",
                map: "gon_dun01",
                x: 153, y: 53, go: None,
            },
            Destination {
                label: "Andar 2",
                map: "gon_dun02",
                x: 28, y: 113, go: None,
            },
            Destination {
                label: "Andar 3",
                map: "gon_dun03",
                x: 68, y: 16, go: None,
            },
        ],
    },
    TravelGroup {
        category: 1,
        label: "Templo de Odin",
        destinations: &[
            Destination {
                label: "Odin Temple 1",
                map: "odin_tem01",
                x: 298, y: 167, go: None,
            },
            Destination {
                label: "Odin Temple 2",
                map: "odin_tem02",
                x: 224, y: 149, go: None,
            },
            Destination {
                label: "Odin Temple 3",
                map: "odin_tem03",
                x: 266, y: 280, go: None,
            },
        ],
    },
    TravelGroup {
        category: 1,
        label: "Torre de Thanatos",
        destinations: &[
            Destination {
                label: "Andar 1",
                map: "tha_t01",
                x: 150, y: 39, go: None,
            },
            Destination {
                label: "Andar 2",
                map: "tha_t02",
                x: 150, y: 136, go: None,
            },
            Destination {
                label: "Andar 3",
                map: "tha_t03",
                x: 220, y: 158, go: None,
            },
            Destination {
                label: "Andar 4",
                map: "tha_t04",
                x: 59, y: 143, go: None,
            },
            Destination {
                label: "Andar 5",
                map: "tha_t05",
                x: 62, y: 11, go: None,
            },
            Destination {
                label: "Andar 6",
                map: "tha_t06",
                x: 89, y: 221, go: None,
            },
            Destination {
                label: "Andar 7",
                map: "tha_t07",
                x: 35, y: 166, go: None,
            },
            Destination {
                label: "Andar 8",
                map: "tha_t08",
                x: 93, y: 148, go: None,
            },
            Destination {
                label: "Andar 9",
                map: "tha_t09",
                x: 29, y: 107, go: None,
            },
            Destination {
                label: "Andar 10",
                map: "tha_t10",
                x: 159, y: 138, go: None,
            },
            Destination {
                label: "Andar 11",
                map: "tha_t11",
                x: 19, y: 20, go: None,
            },
            Destination {
                label: "Andar 12",
                map: "tha_t12",
                x: 130, y: 52, go: None,
            },
        ],
    },
    TravelGroup {
        category: 1,
        label: "Torre do Relógio",
        destinations: &[
            Destination {
                label: "Clock Tower 1",
                map: "c_tower1",
                x: 199, y: 159, go: None,
            },
            Destination {
                label: "Clock Tower 2",
                map: "c_tower2",
                x: 148, y: 283, go: None,
            },
            Destination {
                label: "Clock Tower 3",
                map: "c_tower3",
                x: 65, y: 147, go: None,
            },
            Destination {
                label: "Clock Tower 4",
                map: "c_tower4",
                x: 56, y: 155, go: None,
            },
            Destination {
                label: "Basement 1",
                map: "alde_dun01",
                x: 297, y: 25, go: None,
            },
            Destination {
                label: "Basement 2",
                map: "alde_dun02",
                x: 127, y: 169, go: None,
            },
            Destination {
                label: "Basement 3",
                map: "alde_dun03",
                x: 277, y: 178, go: None,
            },
            Destination {
                label: "Basement 4",
                map: "alde_dun04",
                x: 268, y: 74, go: None,
            },
        ],
    },
    TravelGroup {
        category: 1,
        label: "Tumba de Louyang",
        destinations: &[
            Destination {
                label: "The Royal Tomb",
                map: "lou_dun01",
                x: 218, y: 196, go: None,
            },
            Destination {
                label: "Inside the Royal Tomb",
                map: "lou_dun02",
                x: 282, y: 20, go: None,
            },
            Destination {
                label: "Suei Long Gon",
                map: "lou_dun03",
                x: 165, y: 38, go: None,
            },
        ],
    },
    TravelGroup {
        category: 1,
        label: "Vulcão de Thor",
        destinations: &[
            Destination {
                label: "Andar 1",
                map: "thor_v01",
                x: 21, y: 228, go: None,
            },
            Destination {
                label: "Andar 2",
                map: "thor_v02",
                x: 75, y: 205, go: None,
            },
            Destination {
                label: "Andar 3",
                map: "thor_v03",
                x: 34, y: 272, go: None,
            },
        ],
    },
    TravelGroup {
        category: 2,
        label: "Campos de Amatsu",
        destinations: &[
            Destination {
                label: "Área 1",
                map: "ama_fild01",
                x: 190, y: 197, go: None,
            },
        ],
    },
    TravelGroup {
        category: 2,
        label: "Campos de Ayothaya",
        destinations: &[
            Destination {
                label: "Área 1",
                map: "ayo_fild01",
                x: 173, y: 134, go: None,
            },
            Destination {
                label: "Área 2",
                map: "ayo_fild02",
                x: 212, y: 150, go: None,
            },
        ],
    },
    TravelGroup {
        category: 2,
        label: "Campos de Brasilis",
        destinations: &[
            Destination {
                label: "Área 1",
                map: "bra_fild01",
                x: 74, y: 32, go: None,
            },
        ],
    },
    TravelGroup {
        category: 2,
        label: "Campos de Comodo",
        destinations: &[
            Destination {
                label: "Área 1",
                map: "cmd_fild01",
                x: 180, y: 178, go: None,
            },
            Destination {
                label: "Área 2",
                map: "cmd_fild02",
                x: 231, y: 160, go: None,
            },
            Destination {
                label: "Área 3",
                map: "cmd_fild03",
                x: 191, y: 172, go: None,
            },
            Destination {
                label: "Área 4",
                map: "cmd_fild04",
                x: 228, y: 194, go: None,
            },
            Destination {
                label: "Área 5",
                map: "cmd_fild05",
                x: 224, y: 203, go: None,
            },
            Destination {
                label: "Área 6",
                map: "cmd_fild06",
                x: 190, y: 223, go: None,
            },
            Destination {
                label: "Área 7",
                map: "cmd_fild07",
                x: 234, y: 177, go: None,
            },
            Destination {
                label: "Área 8",
                map: "cmd_fild08",
                x: 194, y: 175, go: None,
            },
            Destination {
                label: "Área 9",
                map: "cmd_fild09",
                x: 172, y: 172, go: None,
            },
        ],
    },
    TravelGroup {
        category: 2,
        label: "Campos de Einbroch",
        destinations: &[
            Destination {
                label: "Área 1",
                map: "ein_fild01",
                x: 142, y: 225, go: None,
            },
            Destination {
                label: "Área 2",
                map: "ein_fild02",
                x: 182, y: 141, go: None,
            },
            Destination {
                label: "Área 3",
                map: "ein_fild03",
                x: 187, y: 228, go: None,
            },
            Destination {
                label: "Área 4",
                map: "ein_fild04",
                x: 185, y: 173, go: None,
            },
            Destination {
                label: "Área 5",
                map: "ein_fild05",
                x: 216, y: 173, go: None,
            },
            Destination {
                label: "Área 6",
                map: "ein_fild06",
                x: 195, y: 148, go: None,
            },
            Destination {
                label: "Área 7",
                map: "ein_fild07",
                x: 272, y: 220, go: None,
            },
            Destination {
                label: "Área 8",
                map: "ein_fild08",
                x: 173, y: 214, go: None,
            },
            Destination {
                label: "Área 9",
                map: "ein_fild09",
                x: 207, y: 174, go: None,
            },
            Destination {
                label: "Área 10",
                map: "ein_fild10",
                x: 196, y: 200, go: None,
            },
        ],
    },
    TravelGroup {
        category: 2,
        label: "Campos de Geffen",
        destinations: &[
            Destination {
                label: "Área 0",
                map: "gef_fild00",
                x: 46, y: 199, go: None,
            },
            Destination {
                label: "Área 1",
                map: "gef_fild01",
                x: 213, y: 204, go: None,
            },
            Destination {
                label: "Área 2",
                map: "gef_fild02",
                x: 195, y: 212, go: None,
            },
            Destination {
                label: "Área 3",
                map: "gef_fild03",
                x: 257, y: 192, go: None,
            },
            Destination {
                label: "Área 4",
                map: "gef_fild04",
                x: 188, y: 171, go: None,
            },
            Destination {
                label: "Área 5",
                map: "gef_fild05",
                x: 166, y: 263, go: None,
            },
            Destination {
                label: "Área 6",
                map: "gef_fild06",
                x: 248, y: 158, go: None,
            },
            Destination {
                label: "Área 7",
                map: "gef_fild07",
                x: 195, y: 191, go: None,
            },
            Destination {
                label: "Área 8",
                map: "gef_fild08",
                x: 186, y: 183, go: None,
            },
            Destination {
                label: "Área 9",
                map: "gef_fild09",
                x: 221, y: 117, go: None,
            },
            Destination {
                label: "Área 10",
                map: "gef_fild10",
                x: 178, y: 218, go: None,
            },
            Destination {
                label: "Área 11",
                map: "gef_fild11",
                x: 136, y: 328, go: None,
            },
            Destination {
                label: "Área 12",
                map: "gef_fild12",
                x: 240, y: 181, go: None,
            },
            Destination {
                label: "Área 13",
                map: "gef_fild13",
                x: 235, y: 235, go: None,
            },
            Destination {
                label: "Área 14",
                map: "gef_fild14",
                x: 211, y: 185, go: None,
            },
        ],
    },
    TravelGroup {
        category: 2,
        label: "Campos de Hugel",
        destinations: &[
            Destination {
                label: "Área 1",
                map: "hu_fild01",
                x: 268, y: 101, go: None,
            },
            Destination {
                label: "Área 2",
                map: "hu_fild02",
                x: 222, y: 193, go: None,
            },
            Destination {
                label: "Área 3",
                map: "hu_fild03",
                x: 232, y: 185, go: None,
            },
            Destination {
                label: "Área 4",
                map: "hu_fild04",
                x: 252, y: 189, go: None,
            },
            Destination {
                label: "Área 5",
                map: "hu_fild05",
                x: 196, y: 106, go: None,
            },
            Destination {
                label: "Área 6",
                map: "hu_fild06",
                x: 216, y: 220, go: None,
            },
            Destination {
                label: "Área 7",
                map: "hu_fild07",
                x: 227, y: 197, go: None,
            },
        ],
    },
    TravelGroup {
        category: 2,
        label: "Campos de Juno",
        destinations: &[
            Destination {
                label: "Área 1",
                map: "yuno_fild01",
                x: 189, y: 224, go: None,
            },
            Destination {
                label: "Área 2",
                map: "yuno_fild02",
                x: 192, y: 207, go: None,
            },
            Destination {
                label: "Área 3",
                map: "yuno_fild03",
                x: 221, y: 157, go: None,
            },
            Destination {
                label: "Área 4",
                map: "yuno_fild04",
                x: 226, y: 199, go: None,
            },
            Destination {
                label: "Área 5",
                map: "yuno_fild05",
                x: 223, y: 177, go: None,
            },
            Destination {
                label: "Área 6",
                map: "yuno_fild06",
                x: 187, y: 232, go: None,
            },
            Destination {
                label: "Área 7",
                map: "yuno_fild07",
                x: 231, y: 174, go: None,
            },
            Destination {
                label: "Área 8",
                map: "yuno_fild08",
                x: 196, y: 203, go: None,
            },
            Destination {
                label: "Área 9",
                map: "yuno_fild09",
                x: 183, y: 214, go: None,
            },
            Destination {
                label: "Área 10",
                map: "yuno_fild10",
                x: 200, y: 124, go: None,
            },
            Destination {
                label: "Área 11",
                map: "yuno_fild11",
                x: 195, y: 226, go: None,
            },
            Destination {
                label: "Área 12",
                map: "yuno_fild12",
                x: 210, y: 304, go: None,
            },
        ],
    },
    TravelGroup {
        category: 2,
        label: "Campos de Kunlun",
        destinations: &[
            Destination {
                label: "Área 1",
                map: "gon_fild01",
                x: 220, y: 227, go: None,
            },
        ],
    },
    TravelGroup {
        category: 2,
        label: "Campos de Lighthalzen",
        destinations: &[
            Destination {
                label: "Área 1",
                map: "lhz_fild01",
                x: 240, y: 179, go: None,
            },
            Destination {
                label: "Área 2",
                map: "lhz_fild02",
                x: 185, y: 235, go: None,
            },
            Destination {
                label: "Área 3",
                map: "lhz_fild03",
                x: 240, y: 226, go: None,
            },
        ],
    },
    TravelGroup {
        category: 2,
        label: "Campos de Luoyang",
        destinations: &[
            Destination {
                label: "Área 1",
                map: "lou_fild01",
                x: 229, y: 187, go: None,
            },
        ],
    },
    TravelGroup {
        category: 2,
        label: "Campos de Lutie",
        destinations: &[
            Destination {
                label: "Área 1",
                map: "xmas_fild01",
                x: 115, y: 145, go: None,
            },
        ],
    },
    TravelGroup {
        category: 2,
        label: "Campos de Manuk",
        destinations: &[
            Destination {
                label: "Área 1",
                map: "man_fild01",
                x: 35, y: 236, go: None,
            },
            Destination {
                label: "Área 2",
                map: "man_fild02",
                x: 35, y: 262, go: None,
            },
            Destination {
                label: "Área 3",
                map: "man_fild03",
                x: 84, y: 365, go: None,
            },
        ],
    },
    TravelGroup {
        category: 2,
        label: "Campos de Mjolnir",
        destinations: &[
            Destination {
                label: "Área 1",
                map: "mjolnir_01",
                x: 204, y: 120, go: None,
            },
            Destination {
                label: "Área 2",
                map: "mjolnir_02",
                x: 175, y: 193, go: None,
            },
            Destination {
                label: "Área 3",
                map: "mjolnir_03",
                x: 208, y: 213, go: None,
            },
            Destination {
                label: "Área 4",
                map: "mjolnir_04",
                x: 179, y: 180, go: None,
            },
            Destination {
                label: "Área 5",
                map: "mjolnir_05",
                x: 181, y: 240, go: None,
            },
            Destination {
                label: "Área 6",
                map: "mjolnir_06",
                x: 195, y: 270, go: None,
            },
            Destination {
                label: "Área 7",
                map: "mjolnir_07",
                x: 235, y: 202, go: None,
            },
            Destination {
                label: "Área 8",
                map: "mjolnir_08",
                x: 188, y: 215, go: None,
            },
            Destination {
                label: "Área 9",
                map: "mjolnir_09",
                x: 205, y: 144, go: None,
            },
            Destination {
                label: "Área 10",
                map: "mjolnir_10",
                x: 245, y: 223, go: None,
            },
            Destination {
                label: "Área 11",
                map: "mjolnir_11",
                x: 180, y: 206, go: None,
            },
            Destination {
                label: "Área 12",
                map: "mjolnir_12",
                x: 196, y: 208, go: None,
            },
        ],
    },
    TravelGroup {
        category: 2,
        label: "Campos de Moscovia",
        destinations: &[
            Destination {
                label: "Área 1",
                map: "mosk_fild01",
                x: 82, y: 104, go: None,
            },
            Destination {
                label: "Área 2",
                map: "mosk_fild02",
                x: 131, y: 147, go: None,
            },
        ],
    },
    TravelGroup {
        category: 2,
        label: "Campos de Niflheim",
        destinations: &[
            Destination {
                label: "Área 1",
                map: "nif_fild01",
                x: 215, y: 229, go: None,
            },
            Destination {
                label: "Área 2",
                map: "nif_fild02",
                x: 167, y: 234, go: None,
            },
        ],
    },
    TravelGroup {
        category: 2,
        label: "Campos de Prontera",
        destinations: &[
            Destination {
                label: "Área 0",
                map: "prt_fild00",
                x: 208, y: 227, go: None,
            },
            Destination {
                label: "Área 1",
                map: "prt_fild01",
                x: 190, y: 206, go: None,
            },
            Destination {
                label: "Área 2",
                map: "prt_fild02",
                x: 240, y: 206, go: None,
            },
            Destination {
                label: "Área 3",
                map: "prt_fild03",
                x: 190, y: 143, go: None,
            },
            Destination {
                label: "Área 4",
                map: "prt_fild04",
                x: 307, y: 252, go: None,
            },
            Destination {
                label: "Área 5",
                map: "prt_fild05",
                x: 239, y: 213, go: None,
            },
            Destination {
                label: "Área 6",
                map: "prt_fild06",
                x: 185, y: 188, go: None,
            },
            Destination {
                label: "Área 7",
                map: "prt_fild07",
                x: 193, y: 194, go: None,
            },
            Destination {
                label: "Área 8",
                map: "prt_fild08",
                x: 187, y: 218, go: None,
            },
            Destination {
                label: "Área 9",
                map: "prt_fild09",
                x: 210, y: 183, go: None,
            },
            Destination {
                label: "Área 10",
                map: "prt_fild10",
                x: 195, y: 149, go: None,
            },
            Destination {
                label: "Área 11",
                map: "prt_fild11",
                x: 198, y: 164, go: None,
            },
        ],
    },
    TravelGroup {
        category: 2,
        label: "Campos de Rachel",
        destinations: &[
            Destination {
                label: "Área 1",
                map: "ra_fild01",
                x: 192, y: 162, go: None,
            },
            Destination {
                label: "Área 2",
                map: "ra_fild02",
                x: 235, y: 166, go: None,
            },
            Destination {
                label: "Área 3",
                map: "ra_fild03",
                x: 202, y: 206, go: None,
            },
            Destination {
                label: "Área 4",
                map: "ra_fild04",
                x: 202, y: 208, go: None,
            },
            Destination {
                label: "Área 5",
                map: "ra_fild05",
                x: 225, y: 202, go: None,
            },
            Destination {
                label: "Área 6",
                map: "ra_fild06",
                x: 202, y: 214, go: None,
            },
            Destination {
                label: "Área 7",
                map: "ra_fild07",
                x: 263, y: 196, go: None,
            },
            Destination {
                label: "Área 8",
                map: "ra_fild08",
                x: 217, y: 201, go: None,
            },
            Destination {
                label: "Área 9",
                map: "ra_fild09",
                x: 87, y: 121, go: None,
            },
            Destination {
                label: "Área 10",
                map: "ra_fild10",
                x: 277, y: 181, go: None,
            },
            Destination {
                label: "Área 11",
                map: "ra_fild11",
                x: 221, y: 185, go: None,
            },
            Destination {
                label: "Área 12",
                map: "ra_fild12",
                x: 175, y: 200, go: None,
            },
            Destination {
                label: "Área 13",
                map: "ra_fild13",
                x: 174, y: 197, go: None,
            },
        ],
    },
    TravelGroup {
        category: 2,
        label: "Campos de Splendide",
        destinations: &[
            Destination {
                label: "Área 1",
                map: "spl_fild01",
                x: 175, y: 186, go: None,
            },
            Destination {
                label: "Área 2",
                map: "spl_fild02",
                x: 236, y: 184, go: None,
            },
            Destination {
                label: "Área 3",
                map: "spl_fild03",
                x: 188, y: 204, go: None,
            },
        ],
    },
    TravelGroup {
        category: 2,
        label: "Campos de Umbala",
        destinations: &[
            Destination {
                label: "Área 1",
                map: "um_fild01",
                x: 217, y: 206, go: None,
            },
            Destination {
                label: "Área 2",
                map: "um_fild02",
                x: 223, y: 221, go: None,
            },
            Destination {
                label: "Área 3",
                map: "um_fild03",
                x: 237, y: 215, go: None,
            },
            Destination {
                label: "Área 4",
                map: "um_fild04",
                x: 202, y: 197, go: None,
            },
        ],
    },
    TravelGroup {
        category: 2,
        label: "Campos de Veins",
        destinations: &[
            Destination {
                label: "Área 1",
                map: "ve_fild01",
                x: 186, y: 175, go: None,
            },
            Destination {
                label: "Área 2",
                map: "ve_fild02",
                x: 196, y: 370, go: None,
            },
            Destination {
                label: "Área 3",
                map: "ve_fild03",
                x: 222, y: 45, go: None,
            },
            Destination {
                label: "Área 4",
                map: "ve_fild04",
                x: 51, y: 250, go: None,
            },
            Destination {
                label: "Área 5",
                map: "ve_fild05",
                x: 202, y: 324, go: None,
            },
            Destination {
                label: "Área 6",
                map: "ve_fild06",
                x: 150, y: 223, go: None,
            },
            Destination {
                label: "Área 7",
                map: "ve_fild07",
                x: 149, y: 307, go: None,
            },
        ],
    },
    TravelGroup {
        category: 2,
        label: "Deserto de Sograt",
        destinations: &[
            Destination {
                label: "Sograt Desert 20",
                map: "moc_fild20",
                x: 207, y: 202, go: None,
            },
            Destination {
                label: "Sograt Desert 21",
                map: "moc_fild21",
                x: 31, y: 195, go: None,
            },
            Destination {
                label: "Sograt Desert 22",
                map: "moc_fild22",
                x: 38, y: 195, go: None,
            },
            Destination {
                label: "Sograt Desert 1",
                map: "moc_fild01",
                x: 219, y: 205, go: None,
            },
            Destination {
                label: "Sograt Desert 2",
                map: "moc_fild02",
                x: 177, y: 206, go: None,
            },
            Destination {
                label: "Sograt Desert 3",
                map: "moc_fild03",
                x: 194, y: 182, go: None,
            },
            Destination {
                label: "Sograt Desert 7",
                map: "moc_fild07",
                x: 224, y: 170, go: None,
            },
            Destination {
                label: "Sograt Desert 11",
                map: "moc_fild11",
                x: 198, y: 216, go: None,
            },
            Destination {
                label: "Sograt Desert 12",
                map: "moc_fild12",
                x: 156, y: 187, go: None,
            },
            Destination {
                label: "Sograt Desert 13",
                map: "moc_fild13",
                x: 185, y: 263, go: None,
            },
            Destination {
                label: "Sograt Desert 16",
                map: "moc_fild16",
                x: 206, y: 228, go: None,
            },
            Destination {
                label: "Sograt Desert 17",
                map: "moc_fild17",
                x: 208, y: 238, go: None,
            },
            Destination {
                label: "Sograt Desert 18",
                map: "moc_fild18",
                x: 209, y: 223, go: None,
            },
            Destination {
                label: "Sograt Desert 19",
                map: "moc_fild19",
                x: 85, y: 97, go: None,
            },
        ],
    },
    TravelGroup {
        category: 2,
        label: "Florestas de Payon",
        destinations: &[
            Destination {
                label: "Área 1",
                map: "pay_fild01",
                x: 158, y: 206, go: None,
            },
            Destination {
                label: "Área 2",
                map: "pay_fild02",
                x: 151, y: 219, go: None,
            },
            Destination {
                label: "Área 3",
                map: "pay_fild03",
                x: 205, y: 148, go: None,
            },
            Destination {
                label: "Área 4",
                map: "pay_fild04",
                x: 186, y: 247, go: None,
            },
            Destination {
                label: "Área 5",
                map: "pay_fild05",
                x: 134, y: 204, go: None,
            },
            Destination {
                label: "Área 6",
                map: "pay_fild06",
                x: 193, y: 235, go: None,
            },
            Destination {
                label: "Área 7",
                map: "pay_fild07",
                x: 200, y: 177, go: None,
            },
            Destination {
                label: "Área 8",
                map: "pay_fild08",
                x: 137, y: 189, go: None,
            },
            Destination {
                label: "Área 9",
                map: "pay_fild09",
                x: 201, y: 224, go: None,
            },
            Destination {
                label: "Área 10",
                map: "pay_fild10",
                x: 160, y: 205, go: None,
            },
            Destination {
                label: "Área 11",
                map: "pay_fild11",
                x: 194, y: 150, go: None,
            },
        ],
    },
];
