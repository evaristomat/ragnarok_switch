//! Every GRF resource path the client reads, in one place.
//!
//! Values are GRF entry names: forward slashes, Korean folder names spelled the
//! way the archive stores them. Paths built at runtime are functions here too,
//! so no `data/` literal needs to live anywhere else.

/// Declares path constants and the module's `ALL` slice from one list, so the
/// two cannot drift.
#[macro_export]
macro_rules! paths {
    ($($(#[$attr:meta])* $name:ident = $value:literal;)*) => {
        $($(#[$attr])* pub const $name: &str = $value;)*

        /// Every path declared in this module.
        pub const ALL: &[&str] = &[$($value),*];
    };
}

/// Every path the registry states outright. Paths assembled at runtime are not
/// here — those come from the builder functions.
pub fn all_static_paths() -> Vec<&'static str> {
    const MODULES: &[&[&str]] = &[
        dir::ALL,
        font::ALL,
        grf::ALL,
        lua::ALL,
        sprite::ALL,
        sprite::effect::ALL,
        sprite::item::ALL,
        sprite::monster::ALL,
        sprite::pet_accessory::ALL,
        sprite::player::ALL,
        table::ALL,
        texture::ALL,
        texture::effect::ALL,
        ui::ALL,
        ui::basic::ALL,
        ui::basic::seekparty::ALL,
        ui::illust::ALL,
        ui::item::ALL,
        ui::login::ALL,
        ui::menu_icon::ALL,
        ui::minimap::ALL,
        ui::refining::ALL,
        ui::styleshop::ALL,
        ui::statuswnd::ALL,
    ];
    MODULES.iter().flat_map(|m| m.iter().copied()).collect()
}

/// Directory prefixes, for listing or stripping — not resources themselves.
pub mod dir {
    crate::paths! {
        DATA = "data/";
        SPRITE_ACCESSORY = "data/sprite/악세사리/";
        SPRITE_MONSTER = "data/sprite/몬스터/";
        SPRITE_NPC = "data/sprite/npc/";
        SPRITE_PLAYER = "data/sprite/인간족/";
        SPRITE_SHIELD = "data/sprite/방패/";
        STR_EFFECT = "data/texture/effect/";
        TEXTURE = "data/texture/";
        UI_TEXTURE = "data/texture/유저인터페이스/";
        UI_TEXTURE_EN = "data/texture/userinterface/";
    }
}

/// TrueType fonts shipped in the archive.
pub mod font {
    crate::paths! {
        NANUM_BARUN_GOTHIC = "data/Font/NanumBarunGothic.ttf";
        NANUM_BARUN_GOTHIC_BOLD = "data/Font/NanumBarunGothicBold.ttf";
    }
}

/// GRF archives.
pub mod grf {
    crate::paths! {
        DEFAULT_ARCHIVE = "data/data.grf";
    }
}

/// Head-attachment offset tables, one per job.
pub mod imf {
    pub fn of(base: &str) -> String {
        format!("data/imf/{base}.imf")
    }

    pub fn for_job(job: &str, sex: &str) -> String {
        format!("data/imf/{job}_{sex}.imf")
    }
}

/// Lua/Lub identity tables. `.lua` and `.lub` are alternate encodings of
/// the same table; readers try one then the other.
pub mod lua {
    crate::paths! {
        ACCESSORY_ID_514_LUB = "data/luafiles514/lua files/datainfo/accessoryid.lub";
        ACCESSORY_ID_LUA = "data/lua files/datainfo/accessoryid.lua";
        ACCESSORY_ID_LUB = "data/lua files/datainfo/accessoryid.lub";
        ACCESSORY_NAME_514_LUB = "data/luafiles514/lua files/datainfo/accname.lub";
        ACCESSORY_NAME_LUA = "data/lua files/datainfo/accname.lua";
        ACCESSORY_NAME_LUB = "data/lua files/datainfo/accname.lub";
        JOB_IDENTITY_514_LUB = "data/luafiles514/lua files/datainfo/jobidentity.lub";
        JOB_NAME_514_LUB = "data/luafiles514/lua files/datainfo/jobname.lub";
        JOB_NAME_LUB = "data/lua files/datainfo/jobname.lub";
        JOB_IDENTITY_LUA = "data/lua files/datainfo/jobidentity.lua";
        JOB_IDENTITY_LUB = "data/lua files/datainfo/jobidentity.lub";
        NPC_IDENTITY_514_LUB = "data/luafiles514/lua files/datainfo/npcidentity.lub";
        NPC_IDENTITY_LUA = "data/lua files/datainfo/npcidentity.lua";
        NPC_IDENTITY_LUB = "data/lua files/datainfo/npcidentity.lub";
        // kRO keeps the current item table outside the GRF (System/itemInfo_true.lub); copied into the loose files
        // here, it is read before the GRF's older one
        ITEM_INFO_TRUE_514_LUB = "data/luafiles514/lua files/datainfo/iteminfo_true.lub";
        ITEM_INFO_514_LUB = "data/luafiles514/lua files/datainfo/iteminfo.lub";
        // iRO's own System/iteminfo.lub (iRO Wiki's clean client): its items kRO's table lacks
        ITEM_INFO_IRO_514_LUB = "data/luafiles514/lua files/datainfo/iteminfo_iro.lub";
        // ROenglishRE's SystemEN/LuaFiles514/itemInfo.lua compiled with luac 5.1: its items no official table has
        ITEM_INFO_EN_514_LUB = "data/luafiles514/lua files/datainfo/iteminfo_en.lub";
        WEAPON_TABLE_514_LUB = "data/luafiles514/lua files/datainfo/weapontable.lub";
        STYLING_SHOP_514_LUB = "data/luafiles514/lua files/stylingshop/stylingshopinfo.lub";
        SIGN_BOARD_LIST_514_LUB = "data/luafiles514/lua files/signboardlist.lub";
        // garments: the view rAthena sends names a folder of sprites per job and sex; the layer tables say which
        // frames draw it behind the body (`JOBID`, which they index by, comes from the skill tree's list)
        ROBE_ID_514_LUB = "data/luafiles514/lua files/datainfo/spriterobeid.lub";
        ROBE_NAME_514_LUB = "data/luafiles514/lua files/datainfo/spriterobename.lub";
        JOB_INHERIT_LIST_514_LUB = "data/luafiles514/lua files/skillinfoz/jobinheritlist.lub";
        // skills: SP per level and which levels the player picks (`SKILL_INFO_LIST`, its keys from `SKID`)
        SKILL_ID_514_LUB = "data/luafiles514/lua files/skillinfoz/skillid.lub";
        SKILL_INFO_LIST_514_LUB = "data/luafiles514/lua files/skillinfoz/skillinfolist.lub";
        ROBE_LAYER_JOBS_514_LUB = "data/luafiles514/lua files/spreditinfo/_new_2dlayerdir_f.lub";
        ROBE_LAYERS_FEMALE_514_LUB = "data/luafiles514/lua files/spreditinfo/_new_biglayerdir_female.lub";
        ROBE_LAYERS_MALE_514_LUB = "data/luafiles514/lua files/spreditinfo/_new_biglayerdir_male.lub";
    }
}

/// Map geometry: one `.rsw`/`.gnd`/`.gat` triple per map.
pub mod map {
    pub fn rsw(map_name: &str) -> String {
        format!("data/{map_name}.rsw")
    }

    pub fn gnd(map_name: &str) -> String {
        format!("data/{map_name}.gnd")
    }

    pub fn gat(map_name: &str) -> String {
        format!("data/{map_name}.gat")
    }

    /// For names already carrying their extension, as RSW headers store them.
    pub fn file(file_name: &str) -> String {
        format!("data/{file_name}")
    }
}

/// 3D monster models and their animation clips.
pub mod model {
    pub fn mob(file_name: &str) -> String {
        format!("data/model/3dmob/{file_name}")
    }

    pub fn mob_animation(bone_type: u32, suffix: &str) -> String {
        format!("data/model/3dmob_bone/{bone_type}_{suffix}.gr2")
    }
}

/// The race a player job belongs to: each keeps its sprites and palettes in its own folders.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Race {
    Human,
    /// The Summoner line (Doram, `도람족`)
    Doram,
}

impl Race {
    fn sprites(self) -> &'static str {
        match self {
            Race::Human => "인간족",
            Race::Doram => "도람족",
        }
    }

    fn head_palettes(self) -> &'static str {
        match self {
            Race::Human => "머리",
            Race::Doram => "도람족/머리",
        }
    }

    fn body_palettes(self) -> &'static str {
        match self {
            Race::Human => "몸",
            Race::Doram => "도람족/body",
        }
    }
}

/// Sprite recolour palettes.
pub mod palette {
    use super::Race;

    pub fn head(race: Race, head_name: &str, sex: &str, palette_id: u16) -> String {
        format!(
            "data/palette/{}/머리{head_name}_{sex}_{palette_id}.pal",
            race.head_palettes()
        )
    }

    pub fn body(race: Race, job: &str, sex: &str, palette_id: u16) -> String {
        format!(
            "data/palette/{}/{job}_{sex}_{palette_id}.pal",
            race.body_palettes()
        )
    }
}

/// BGM tracks and sound effects.
pub mod sound {
    pub fn bgm(track: &str) -> String {
        format!("data/wav/bgm/{track}")
    }

    pub fn sfx(name: &str) -> String {
        format!("data/wav/{name}")
    }
}

/// Sprites outside the actor trees.
pub mod sprite {
    crate::paths! {
        CURSORS_ACT = "data/sprite/cursors.act";
        CURSORS_SPR = "data/sprite/cursors.spr";
        SHADOW = "data/sprite/shadow";
        SHADOW_ACT = "data/sprite/shadow.act";
        SHADOW_SPR = "data/sprite/shadow.spr";
        SLOTMACHINE_ACT = "data/sprite/slotmachine.act";
        SLOTMACHINE_SPR = "data/sprite/slotmachine.spr";
    }

    /// Headgear sprites, drawn per sex.
    pub mod accessory {
        pub fn of(sex: &str, suffix: &str) -> String {
            format!("data/sprite/악세사리/{sex}/{sex}{suffix}")
        }
    }

    /// The `이팩트` tree — visual-effect sprites.
    pub mod effect {
        crate::paths! {
            ANGEL = "data/sprite/이팩트/천사";
            ANGEL_FEATHER = "data/sprite/이팩트/천사날개깃털";
            ANGEL_WINGS = "data/sprite/이팩트/천사날개";
            AQUA_BENEDICTA = "data/sprite/이팩트/성수뜨기";
            BAT = "data/sprite/이팩트/박쥐";
            BLESSING = "data/sprite/이팩트/축복";
            BLOODLUST = "data/sprite/이팩트/블러드러스트";
            BOOK_OF_CREATINGSTAR = "data/sprite/이팩트/book_of_creatingstar/book_of_creatingstar";
            CASTLING = "data/sprite/이팩트/캐슬링";
            CCONFINE = "data/sprite/이팩트/cconfine";
            CHIMNEY_SMOKE = "data/sprite/이팩트/굴뚝연기";
            CHRISTMAS = "data/sprite/이팩트/크리스마스";
            DARKBREATH = "data/sprite/이팩트/darkbreath";
            DEMONSTRATION = "data/sprite/이팩트/데몬스트레이션";
            DESPERADO = "data/sprite/이팩트/데스페라도";
            EARTHQUAKE = "data/sprite/이팩트/어스퀘이크";
            EF_SNOW = "data/sprite/이팩트/ef_snow";
            EMOTION_ACT = "data/sprite/이팩트/emotion.act";
            EMOTION_SPR = "data/sprite/이팩트/emotion.spr";
            FALCON = "data/sprite/이팩트/매";
            FALCON2 = "data/sprite/이팩트/매2";
            FAST = "data/sprite/이팩트/fast";
            FIREBALL = "data/sprite/이팩트/fireball";
            FIREWORK_BIRTHDAY = "data/sprite/이팩트/폭죽_생일";
            FIREWORK_CHRISTMAS = "data/sprite/이팩트/폭죽_크리스마스";
            FIREWORK_LOVE = "data/sprite/이팩트/폭죽_러브";
            FIREWORK_VALENTINE = "data/sprite/이팩트/폭죽_발렌타인";
            FIREWORK_WHITE_DAY = "data/sprite/이팩트/폭죽_화이트데이";
            FVOICE = "data/sprite/이팩트/fvoice";
            GHOST = "data/sprite/이팩트/유령";
            HANBOK_ANGEL_BODY = "data/sprite/이팩트/한복천사(본체)";
            HANBOK_ANGEL_WINGS = "data/sprite/이팩트/한복천사(날개)";
            HUNDRED_SPEAR = "data/sprite/이팩트/헌드레드스피어";
            ICE_BLOCK = "data/sprite/이팩트/얼음땡";
            ISSEN = "data/sprite/이팩트/일섬";
            ITEM_CLOUD = "data/sprite/이팩트/item_cloud";
            ITEM_CURSE = "data/sprite/이팩트/item_curse";
            ITEM_RAIN = "data/sprite/이팩트/item_rain";
            ITEM_THUNDER = "data/sprite/이팩트/item_thunder";
            ITEM_ZZZ = "data/sprite/이팩트/item_zzz";
            KAEN = "data/sprite/이팩트/화염진";
            KASUMIKIRI = "data/sprite/이팩트/안개베기";
            KIRIKAGE = "data/sprite/이팩트/그림자베기";
            MAGICAL_BULLET = "data/sprite/이팩트/매지컬불릿";
            MAPLE_LEAF = "data/sprite/이팩트/단풍";
            MASHOFA = "data/sprite/이팩트/mashofa";
            MSG_ACT = "data/sprite/이팩트/msg.act";
            MSG_SPR = "data/sprite/이팩트/msg.spr";
            M_EF01 = "data/sprite/이팩트/m_ef01";
            M_EF02 = "data/sprite/이팩트/m_ef02";
            M_EF03 = "data/sprite/이팩트/m_ef03";
            M_EF04 = "data/sprite/이팩트/m_ef04";
            M_EF05 = "data/sprite/이팩트/m_ef05";
            M_EF06 = "data/sprite/이팩트/m_ef06";
            M_EF07 = "data/sprite/이팩트/m_ef07";
            NUMBER_ACT = "data/sprite/이팩트/숫자.act";
            NUMBER_SPR = "data/sprite/이팩트/숫자.spr";
            ORCFACE = "data/sprite/이팩트/orcface";
            PARTICLE1 = "data/sprite/이팩트/particle1";
            PARTICLE2 = "data/sprite/이팩트/particle2";
            PARTICLE3 = "data/sprite/이팩트/particle3";
            PARTICLE4 = "data/sprite/이팩트/particle4";
            PARTICLE5 = "data/sprite/이팩트/particle5";
            PARTICLE6 = "data/sprite/이팩트/particle6";
            PARTICLE7 = "data/sprite/이팩트/particle7";
            POISONHIT = "data/sprite/이팩트/poisonhit";
            RANKFONT_ACT = "data/sprite/이팩트/rankfont.act";
            RANKFONT_SPR = "data/sprite/이팩트/rankfont.spr";
            RAPID_SHOWER = "data/sprite/이팩트/래피드샤워";
            SAKURA01 = "data/sprite/이팩트/sakura01";
            SIGHT = "data/sprite/이팩트/sight";
            SILENT_BREEZE = "data/sprite/이팩트/silent_brz";
            SMOKE = "data/sprite/이팩트/smoke";
            SOUL_FAIRY = "data/sprite/이팩트/soul_fairy/soul_fairy";
            SOUL_FALCON = "data/sprite/이팩트/soul_falcon/soul_falcon";
            SOUL_SHADOW = "data/sprite/이팩트/soul_shadow/soul_shadow";
            SPEAR = "data/sprite/이팩트/창";
            SPREAD_ATTACK = "data/sprite/이팩트/스프레드";
            STATUS_CURSE = "data/sprite/이팩트/status-curse";
            STATUS_SLEEP = "data/sprite/이팩트/status-sleep";
            STATUS_STUN = "data/sprite/이팩트/status-stun";
            STOP = "data/sprite/이팩트/스톱";
            TATAMI_FLIP = "data/sprite/이팩트/다다미 뒤집기";
            TIMEFONT_ACT = "data/sprite/이팩트/timefont.act";
            TIMEFONT_SPR = "data/sprite/이팩트/timefont.spr";
            TORCH_01 = "data/sprite/이팩트/torch_01";
            TRACKING = "data/sprite/이팩트/트래킹";
            TRIPLE_ACTION = "data/sprite/이팩트/트리플액션";
            VALLENTINE = "data/sprite/이팩트/vallentine";
            WATERBALL = "data/sprite/이팩트/waterball";
            WIDE_WEB = "data/sprite/이팩트/wideweb";
            WINK = "data/sprite/이팩트/wink";
        }

        /// kRO's carts by design, each as an adult's and a baby's (its table at 0xd320b0, read through its sprite
        /// switch 0x7752b0): 0 the Super Novice's, 1 to 9 the Merchant's own and the purchasable ones, 10 to 12 Cart
        /// Decoration's, 13 the Madogear's. Babies have their own from design 6; before it they push the adult's.
        const CARTS: [(&str, &str); 14] = [
            ("슈노손수레", "슈노손수레"),
            ("손수레", "손수레"),
            ("손수레1", "손수레1"),
            ("손수레2", "손수레2"),
            ("손수레3", "손수레3"),
            ("손수레4", "손수레4"),
            ("손수레5", "손수레5베이비"),
            ("손수레6", "손수레6베이비"),
            ("손수레7", "손수레7베이비"),
            ("손수레8", "손수레8베이비"),
            ("선물상자카트", "선물상자카트_미니"),
            ("포링싣은카트", "포링싣은카트_미니"),
            ("포링카트", "포링카트_미니"),
            ("마도카트", "마도카트_미니"),
        ];

        /// The cart of `design` (none past kRO's table), a baby's when `baby`
        pub fn cart(design: u8, baby: bool) -> Option<String> {
            let (adult, small) = CARTS.get(usize::from(design))?;
            Some(format!(
                "data/sprite/이팩트/{}",
                if baby { small } else { adult }
            ))
        }
    }

    /// Homunculus sprites.
    pub mod homun {
        pub fn of(name: &str) -> String {
            format!("data/sprite/homun/{name}")
        }
    }

    /// Dropped-item sprites.
    pub mod item {
        crate::paths! {
            BLIND_SPEAR = "data/sprite/아이템/블라인드스피어";
            BLUE_GEMSTONE = "data/sprite/아이템/블루젬스톤";
            FLARE_SPEAR = "data/sprite/아이템/플레어스피어";
            FREEZING_SPEAR = "data/sprite/아이템/프리징스피어";
            LIGHTNING_SPEAR = "data/sprite/아이템/라이트닝스피어";
            POISON_SPEAR = "data/sprite/아이템/포이즌스피어";
            RED_GEMSTONE = "data/sprite/아이템/레드젬스톤";
            YELLOW_GEMSTONE = "data/sprite/아이템/옐로우젬스톤";
        }

        pub fn of(name: &str) -> String {
            format!("data/sprite/아이템/{name}")
        }
    }

    /// Monster sprites.
    pub mod monster {
        crate::paths! {
            SKEL_ARCHER_ARROW = "data/sprite/몬스터/skel_archer_arrow";
            STAPO = "data/sprite/몬스터/stapo";
        }

        pub fn of(name: &str) -> String {
            format!("data/sprite/몬스터/{name}")
        }
    }

    /// NPC sprites.
    pub mod npc {
        pub fn of(name: &str) -> String {
            format!("data/sprite/npc/{name}")
        }
    }

    /// Pet headgear animations, keyed by the pet mob.
    pub mod pet_accessory {
        crate::paths! {
            BACSOJIN = "data/sprite/몬스터/BACSOJIN_동그란머리장식.act";
            BAPHOMET = "data/sprite/몬스터/BAPHOMET_뼉다구모자.act";
            BON_GUN = "data/sprite/몬스터/bon_gun_영환도사검.act";
            CHOCHO = "data/sprite/몬스터/chocho_방독면.act";
            CIVIL_SERVANT = "data/sprite/몬스터/CIVIL_SERVANT_금빛귀걸이.act";
            DESERT_WOLF_B = "data/sprite/몬스터/DESERT_WOLF_B_우주복머리.act";
            DEVIRUCHI = "data/sprite/몬스터/DEVIRUCHI_젖꼭지.act";
            DOKEBI = "data/sprite/몬스터/DOKEBI_아후로머리.act";
            DULLAHAN = "data/sprite/몬스터/DULLAHAN_죽음의고리.act";
            GOBLIN_LEADER = "data/sprite/몬스터/GOBLIN_LEADER_멋진휘장.act";
            GOLEM = "data/sprite/몬스터/GOLEM_태엽.act";
            IMP = "data/sprite/몬스터/IMP_뿔보호대.act";
            INCUBUS = "data/sprite/몬스터/INCUBUS_무도회가면.act";
            ISIS = "data/sprite/몬스터/isis_클레오파트라머리띠.act";
            LEAF_CAT = "data/sprite/몬스터/LEAF_CAT_초록복주머니.act";
            LOLI_RURI = "data/sprite/몬스터/LOLI_RURI_패션안경.act";
            LUNATIC = "data/sprite/몬스터/lunatic_리본.act";
            MARIONETTE = "data/sprite/몬스터/MARIONETTE_별모양머리띠.act";
            MEDUSA = "data/sprite/몬스터/MEDUSA_여왕의코로넷.act";
            MIYABI_NINGYO = "data/sprite/몬스터/MIYABI_NINGYO_여름부채.act";
            MUNAK = "data/sprite/몬스터/munak_요술봉.act";
            NIGHTMARE_TERROR = "data/sprite/몬스터/NIGHTMARE_TERROR_지옥의뿔.act";
            ORK_WARRIOR = "data/sprite/몬스터/ork_warrior_꽃.act";
            PECOPECO = "data/sprite/몬스터/pecopeco_냄비.act";
            PETIT = "data/sprite/몬스터/PETIT_별.act";
            PICKY = "data/sprite/몬스터/picky_알껍질.act";
            PORING = "data/sprite/몬스터/poring_책가방.act";
            ROCKER = "data/sprite/몬스터/rocker_메뚜기안경.act";
            SAVAGE_BABE = "data/sprite/몬스터/savage_babe_레이스.act";
            SHINOBI = "data/sprite/몬스터/SHINOBI_두루마기용술.act";
            SMOKIE = "data/sprite/몬스터/smokie_머플러.act";
            SOHEE = "data/sprite/몬스터/SOHEE_방울.act";
            SPORE = "data/sprite/몬스터/spore_원주민치마.act";
            STONE_SHOOTER = "data/sprite/몬스터/STONE_SHOOTER_아프로헤어.act";
            SUCCUBUS = "data/sprite/몬스터/SUCCUBUS_검은나비가면.act";
            WHISPER = "data/sprite/몬스터/WHISPER_영혼고리_.act";
            WICKED_NYMPH = "data/sprite/몬스터/WICKED_NYMPH_옥노리개.act";
            YOYO = "data/sprite/몬스터/yoyo_머리띠.act";
        }
    }

    /// Player body, head and weapon sprites.
    pub mod player {
        use crate::Race;

        crate::paths! {
            GRAND_PECO_CRUSADER_FEMALE = "data/sprite/인간족/몸통/여/신페코크루세이더_여";
            GRAND_PECO_CRUSADER_MALE = "data/sprite/인간족/몸통/남/신페코크루세이더_남";
            LORD_PECO_MALE = "data/sprite/인간족/몸통/남/로드페코_남";
            PECO_KNIGHT_MALE = "data/sprite/인간족/몸통/남/페코페코_기사_남";
            PECO_PALADIN_MALE = "data/sprite/인간족/몸통/남/페코팔라딘_남";
        }

        pub fn body(race: Race, job: &str, sex: &str) -> String {
            format!("data/sprite/{}/몸통/{sex}/{job}_{sex}", race.sprites())
        }

        pub fn head(race: Race, head_name: &str, sex: &str) -> String {
            format!(
                "data/sprite/{}/머리통/{sex}/{head_name}_{sex}",
                race.sprites()
            )
        }

        pub fn weapon(race: Race, job: &str, sex: &str, suffix: &str) -> String {
            format!("data/sprite/{}/{job}/{job}_{sex}{suffix}", race.sprites())
        }

        /// Weapon art keyed by item id, preferred over [`weapon`] when present.
        pub fn weapon_by_item(race: Race, job: &str, sex: &str, item_id: u16) -> String {
            format!("data/sprite/{}/{job}/{job}_{sex}_{item_id}", race.sprites())
        }

        /// A Madogear's own swing trail (`마도기어` the robot, `마도아머` the suit).
        pub fn madogear_trail(name: &str, sex: &str) -> String {
            format!("data/sprite/인간족/마도기어/{name}_{sex}_검광")
        }

        pub fn gm_body(sex: &str) -> String {
            format!("data/sprite/인간족/몸통/{sex}/운영자_{sex}")
        }

        pub fn gm_weapon(sex: &str) -> String {
            format!("data/sprite/인간족/운영자/운영자_{sex}_검")
        }

        /// The name table already carries the sex/type sub-path.
        pub fn mercenary_body(name: &str) -> String {
            format!("data/sprite/인간족/몸통/{name}")
        }

        pub fn mercenary_weapon(base: &str, weapon_char: char) -> String {
            format!("data/sprite/인간족/용병/{base}_{weapon_char}")
        }
    }

    /// Garments (`LOOK_ROBE`), drawn per job and sex.
    pub mod robe {
        pub fn of(robe: &str, job: &str, sex: &str) -> String {
            format!("data/sprite/로브/{robe}/{sex}/{job}_{sex}")
        }
    }

    /// Shield sprites, drawn per job and sex.
    pub mod shield {
        pub fn of(job: &str, sex: &str, shield: impl std::fmt::Display) -> String {
            format!("data/sprite/방패/{job}/{job}_{sex}_{shield}")
        }
    }
}

/// Flat `data/*.txt` lookup tables.
pub mod table {
    crate::paths! {
        CARD_ILLUSTRATION_NAME = "data/num2cardillustnametable.txt";
        CARD_POSTFIX_NAME = "data/cardpostfixnametable.txt";
        CARD_PREFIX_NAME = "data/cardprefixnametable.txt";
        FOG_PARAMETER = "data/fogparametertable.txt";
        IDENTIFIED_ITEM_DESC = "data/idnum2itemdesctable.txt";
        IDENTIFIED_ITEM_NAME = "data/idnum2itemdisplaynametable.txt";
        IDENTIFIED_ITEM_RESOURCE = "data/idnum2itemresnametable.txt";
        INDOOR_RSW = "data/indoorrswtable.txt";
        ITEM_SLOT_COUNT = "data/itemslotcounttable.txt";
        MAP_NAME = "data/mapnametable.txt";
        MAP_POSITION = "data/mappostable.txt";
        MP3_NAME = "data/mp3nametable.txt";
        MSG_STRING = "data/msgstringtable.txt";
        PET_TALK = "data/pettalktable.xml";
        QUEST_DISPLAY = "data/questid2display.txt";
        RES_NAME = "data/resnametable.txt";
        // rAthena's shield views (scripts/import-english.py), for shields iteminfo has no ClassNum for
        SHIELD_VIEW = "data/shieldviewtable.txt";
        // ...and headgear views, for the icon of a headgear iteminfo lacks
        HEADGEAR_VIEW = "data/headgearviewtable.txt";
        // ...and rAthena's cards, for the icon of a card iteminfo lacks
        RATHENA_CARDS = "data/rathenacardtable.txt";
        SKILL_DESC = "data/skilldesctable.txt";
        // the Renewal descriptions over the classic ones (scripts/import-english.py); skilldesctable.txt stays
        // classic for roBrowser
        SKILL_DESC_RENEWAL = "data/skilldesctable_renewal.txt";
        SKILL_NAME = "data/skillnametable.txt";
        SKILL_SP_AMOUNT = "data/leveluseskillspamount.txt";
        SKILL_TREE = "data/skilltreeview.txt";
        UNIDENTIFIED_ITEM_DESC = "data/num2itemdesctable.txt";
        UNIDENTIFIED_ITEM_NAME = "data/num2itemdisplaynametable.txt";
        UNIDENTIFIED_ITEM_RESOURCE = "data/num2itemresnametable.txt";
    }

    pub fn book(item_id: impl std::fmt::Display) -> String {
        format!("data/book/{item_id}.txt")
    }
}

/// Textures outside the UI tree.
pub mod texture {
    crate::paths! {
        GRID = "data/texture/grid.tga";
    }

    pub fn named(name: &str) -> String {
        format!("data/texture/{name}")
    }

    pub fn water(water_type: i32, frame: usize) -> String {
        format!("data/texture/워터/water{water_type}{frame:02}.jpg")
    }

    /// Effect textures and `.str` animation scripts.
    pub mod effect {
        crate::paths! {
            ALPHABET = "data/texture/effect/alpabet.bmp";
        }

        pub fn named(name: &str) -> String {
            format!("data/texture/effect/{name}")
        }

        pub fn str_file(name: &str) -> String {
            format!("data/texture/effect/{name}.str")
        }
    }
}

/// The `유저인터페이스` tree — every window, button and icon texture.
pub mod ui {
    crate::paths! {
        BTN_ADD = "data/texture/유저인터페이스/btn_add.bmp";
        SIGN_BOARD_BG = "data/texture/유저인터페이스/signboard/bg_signboard.bmp";
        SHOW_DAMAGE_BG = "data/texture/유저인터페이스/showdamage/bg_damage.bmp";
        SHOW_DAMAGE_CELL = "data/texture/유저인터페이스/showdamage/btn_damage.bmp";
        SHOW_DAMAGE_CELL_APPLIED = "data/texture/유저인터페이스/showdamage/btn_damage_pick.bmp";
        SHOW_DAMAGE_CELL_PICKED = "data/texture/유저인터페이스/showdamage/btn_damage_press.bmp";
        TIPBOX_CHECKBOX = "data/texture/유저인터페이스/tipbox/checkbox.bmp";
        SWAP_BG_GENERAL = "data/texture/유저인터페이스/swap_equipment/equipwin_bg2_change.bmp";
        SWAP_BG_COSTUME = "data/texture/유저인터페이스/swap_equipment/equipwin_special_change.bmp";
        SWAP_CHANGE = "data/texture/유저인터페이스/swap_equipment/btn_change2_normal.bmp";
        SWAP_CHANGE_OVER = "data/texture/유저인터페이스/swap_equipment/btn_change2_over.bmp";
        SWAP_CHANGE_PRESS = "data/texture/유저인터페이스/swap_equipment/btn_change2_press.bmp";
        SWAP_CHANGE_DISABLE = "data/texture/유저인터페이스/swap_equipment/btn_change2_disable.bmp";
        SWAP_MARK_BG = "data/texture/유저인터페이스/swap_equipment/bg_change.bmp";
        SWAP_MARK_ICON = "data/texture/유저인터페이스/swap_equipment/ico_change.bmp";
        EQUIP_PROPS_BG = "data/texture/유저인터페이스/equipmentpropertieswnd/bg.bmp";
        EQUIP_PROPS_TAB_TOTAL_OFF = "data/texture/유저인터페이스/equipmentpropertieswnd/tab_total_off.bmp";
        EQUIP_PROPS_TAB_TOTAL_ON = "data/texture/유저인터페이스/equipmentpropertieswnd/tab_total_on.bmp";
        EQUIP_PROPS_TAB_CATEGORY1_OFF = "data/texture/유저인터페이스/equipmentpropertieswnd/tab_category1_off.bmp";
        EQUIP_PROPS_TAB_CATEGORY1_ON = "data/texture/유저인터페이스/equipmentpropertieswnd/tab_category1_on.bmp";
        EQUIP_PROPS_TAB_CATEGORY2_OFF = "data/texture/유저인터페이스/equipmentpropertieswnd/tab_category2_off.bmp";
        EQUIP_PROPS_TAB_CATEGORY2_ON = "data/texture/유저인터페이스/equipmentpropertieswnd/tab_category2_on.bmp";
        EQUIP_PROPS_TAB_CATEGORY3_OFF = "data/texture/유저인터페이스/equipmentpropertieswnd/tab_category3_off.bmp";
        EQUIP_PROPS_TAB_CATEGORY3_ON = "data/texture/유저인터페이스/equipmentpropertieswnd/tab_category3_on.bmp";
        EQUIP_PROPS_TAB_CATEGORY4_OFF = "data/texture/유저인터페이스/equipmentpropertieswnd/tab_category4_off.bmp";
        EQUIP_PROPS_TAB_CATEGORY4_ON = "data/texture/유저인터페이스/equipmentpropertieswnd/tab_category4_on.bmp";
        EQUIP_PROPS_TAB_CATEGORY5_OFF = "data/texture/유저인터페이스/equipmentpropertieswnd/tab_category5_off.bmp";
        EQUIP_PROPS_TAB_CATEGORY5_ON = "data/texture/유저인터페이스/equipmentpropertieswnd/tab_category5_on.bmp";
        EQUIP_PROPS_TAB_CATEGORY6_OFF = "data/texture/유저인터페이스/equipmentpropertieswnd/tab_category6_off.bmp";
        EQUIP_PROPS_TAB_CATEGORY6_ON = "data/texture/유저인터페이스/equipmentpropertieswnd/tab_category6_on.bmp";
        EQUIP_PROPS_TAB_CATEGORY7_OFF = "data/texture/유저인터페이스/equipmentpropertieswnd/tab_category7_off.bmp";
        EQUIP_PROPS_TAB_CATEGORY7_ON = "data/texture/유저인터페이스/equipmentpropertieswnd/tab_category7_on.bmp";
        TIPBOX_CHECKBOX_CHECK = "data/texture/유저인터페이스/tipbox/checkbox_check.bmp";
        BTN_ADD_A = "data/texture/유저인터페이스/btn_add_a.bmp";
        BTN_ADD_B = "data/texture/유저인터페이스/btn_add_b.bmp";
        BTN_APPLY = "data/texture/유저인터페이스/btn_apply.bmp";
        BTN_APPLY_A = "data/texture/유저인터페이스/btn_apply_a.bmp";
        BTN_APPLY_B = "data/texture/유저인터페이스/btn_apply_b.bmp";
        BTN_BACK = "data/texture/유저인터페이스/btn_back.bmp";
        BTN_BACK_A = "data/texture/유저인터페이스/btn_back_a.bmp";
        BTN_BACK_B = "data/texture/유저인터페이스/btn_back_b.bmp";
        BTN_BUY = "data/texture/유저인터페이스/btn_buy.bmp";
        BTN_BUY_A = "data/texture/유저인터페이스/btn_buy_a.bmp";
        BTN_BUY_B = "data/texture/유저인터페이스/btn_buy_b.bmp";
        BTN_CANCEL = "data/texture/유저인터페이스/btn_cancel.bmp";
        BTN_CANCEL_A = "data/texture/유저인터페이스/btn_cancel_a.bmp";
        BTN_CANCEL_B = "data/texture/유저인터페이스/btn_cancel_b.bmp";
        BTN_CLOSE = "data/texture/유저인터페이스/btn_close.bmp";
        BTN_CLOSE_A = "data/texture/유저인터페이스/btn_close_a.bmp";
        BTN_CLOSE_B = "data/texture/유저인터페이스/btn_close_b.bmp";
        BTN_DEL = "data/texture/유저인터페이스/btn_del.bmp";
        BTN_DEL_A = "data/texture/유저인터페이스/btn_del_a.bmp";
        BTN_DEL_B = "data/texture/유저인터페이스/btn_del_b.bmp";
        BTN_EDIT = "data/texture/유저인터페이스/btn_edit.bmp";
        BTN_EDIT_A = "data/texture/유저인터페이스/btn_edit_a.bmp";
        BTN_EDIT_B = "data/texture/유저인터페이스/btn_edit_b.bmp";
        BTN_EXCHANGE = "data/texture/유저인터페이스/btn_exchange.bmp";
        BTN_EXCHANGE_A = "data/texture/유저인터페이스/btn_exchange_a.bmp";
        BTN_EXCHANGE_B = "data/texture/유저인터페이스/btn_exchange_b.bmp";
        BTN_EXCHANGE_DIS = "data/texture/유저인터페이스/btn_exchange_dis.bmp";
        BTN_FEED = "data/texture/유저인터페이스/btn_feed.bmp";
        BTN_FEED_A = "data/texture/유저인터페이스/btn_feed_a.bmp";
        BTN_FEED_B = "data/texture/유저인터페이스/btn_feed_b.bmp";
        BTN_FIRED = "data/texture/유저인터페이스/btn_fired.bmp";
        BTN_FIRED_A = "data/texture/유저인터페이스/btn_fired_a.bmp";
        BTN_FIRED_B = "data/texture/유저인터페이스/btn_fired_b.bmp";
        BTN_MAKE = "data/texture/유저인터페이스/btn_make.bmp";
        BTN_MAKE_A = "data/texture/유저인터페이스/btn_make_a.bmp";
        BTN_MAKE_B = "data/texture/유저인터페이스/btn_make_b.bmp";
        BTN_NEXT = "data/texture/유저인터페이스/btn_next.bmp";
        BTN_NEXT_A = "data/texture/유저인터페이스/btn_next_a.bmp";
        BTN_NEXT_B = "data/texture/유저인터페이스/btn_next_b.bmp";
        BTN_OK = "data/texture/유저인터페이스/btn_ok.bmp";
        BTN_OK_A = "data/texture/유저인터페이스/btn_ok_a.bmp";
        BTN_OK_B = "data/texture/유저인터페이스/btn_ok_b.bmp";
        BTN_OK_DIS = "data/texture/유저인터페이스/btn_ok_dis.bmp";
        BTN_RESET = "data/texture/유저인터페이스/btn_reset.bmp";
        BTN_RESET_A = "data/texture/유저인터페이스/btn_reset_a.bmp";
        BTN_RESET_B = "data/texture/유저인터페이스/btn_reset_b.bmp";
        BTN_RESIZE = "data/texture/유저인터페이스/btn_resize.bmp";
        BTN_REWRITE = "data/texture/유저인터페이스/btn_rewrite.bmp";
        BTN_REWRITE_A = "data/texture/유저인터페이스/btn_rewrite_a.bmp";
        BTN_REWRITE_B = "data/texture/유저인터페이스/btn_rewrite_b.bmp";
        BTN_SELL = "data/texture/유저인터페이스/btn_sell.bmp";
        BTN_SELL_A = "data/texture/유저인터페이스/btn_sell_a.bmp";
        BTN_SELL_B = "data/texture/유저인터페이스/btn_sell_b.bmp";
        BTN_SKILL = "data/texture/유저인터페이스/btn_skill.bmp";
        BTN_SKILL_A = "data/texture/유저인터페이스/btn_skill_a.bmp";
        BTN_SKILL_B = "data/texture/유저인터페이스/btn_skill_b.bmp";
        BTN_USE = "data/texture/유저인터페이스/btn_use.bmp";
        BTN_USE_A = "data/texture/유저인터페이스/btn_use_a.bmp";
        BTN_USE_B = "data/texture/유저인터페이스/btn_use_b.bmp";
        BTN_VIEW = "data/texture/유저인터페이스/btn_view.bmp";
        BTN_VIEW_A = "data/texture/유저인터페이스/btn_view_a.bmp";
        BTN_VIEW_B = "data/texture/유저인터페이스/btn_view_b.bmp";
        CHAT_CLOSE = "data/texture/유저인터페이스/chat_close.bmp";
        CHAT_OPEN = "data/texture/유저인터페이스/chat_open.bmp";
        CHECKBOX_0 = "data/texture/유저인터페이스/checkbox_0.bmp";
        CHECKBOX_1 = "data/texture/유저인터페이스/checkbox_1.bmp";
        EMPTY_CARD_SLOT = "data/texture/유저인터페이스/empty_card_slot.bmp";
        ESC_01A = "data/texture/유저인터페이스/esc_01a.bmp";
        ESC_01B = "data/texture/유저인터페이스/esc_01b.bmp";
        ESC_01C = "data/texture/유저인터페이스/esc_01c.bmp";
        ESC_02A = "data/texture/유저인터페이스/esc_02a.bmp";
        ESC_02B = "data/texture/유저인터페이스/esc_02b.bmp";
        ESC_02C = "data/texture/유저인터페이스/esc_02c.bmp";
        ESC_03A = "data/texture/유저인터페이스/esc_03a.bmp";
        ESC_03B = "data/texture/유저인터페이스/esc_03b.bmp";
        ESC_03C = "data/texture/유저인터페이스/esc_03c.bmp";
        ESC_04A = "data/texture/유저인터페이스/esc_04a.bmp";
        ESC_04B = "data/texture/유저인터페이스/esc_04b.bmp";
        ESC_04C = "data/texture/유저인터페이스/esc_04c.bmp";
        ESC_05A = "data/texture/유저인터페이스/esc_05a.bmp";
        ESC_05B = "data/texture/유저인터페이스/esc_05b.bmp";
        ESC_05C = "data/texture/유저인터페이스/esc_05c.bmp";
        ESC_06A = "data/texture/유저인터페이스/esc_06a.bmp";
        ESC_06B = "data/texture/유저인터페이스/esc_06b.bmp";
        ESC_06C = "data/texture/유저인터페이스/esc_06c.bmp";
        ESC_07A = "data/texture/유저인터페이스/esc_07a.bmp";
        ESC_07B = "data/texture/유저인터페이스/esc_07b.bmp";
        ESC_07C = "data/texture/유저인터페이스/esc_07c.bmp";
        ESC_08A = "data/texture/유저인터페이스/esc_08a.bmp";
        ESC_08B = "data/texture/유저인터페이스/esc_08b.bmp";
        ESC_08C = "data/texture/유저인터페이스/esc_08c.bmp";
        RADIOBTN_OFF = "data/texture/유저인터페이스/radiobtn_off.bmp";
        RADIOBTN_ON = "data/texture/유저인터페이스/radiobtn_on.bmp";
        RAG_TITLE = "data/texture/유저인터페이스/rag_title.bmp";
        RAG_TITLE2 = "data/texture/유저인터페이스/rag_title2.bmp";
        RAG_TITLE3 = "data/texture/유저인터페이스/rag_title3.bmp";
        SCROLL0BAR_DOWN = "data/texture/유저인터페이스/scroll0bar_down.bmp";
        SCROLL0BAR_MID = "data/texture/유저인터페이스/scroll0bar_mid.bmp";
        SCROLL0BAR_UP = "data/texture/유저인터페이스/scroll0bar_up.bmp";
        SCROLL0DOWN = "data/texture/유저인터페이스/scroll0down.bmp";
        SCROLL0MID = "data/texture/유저인터페이스/scroll0mid.bmp";
        SCROLL0UP = "data/texture/유저인터페이스/scroll0up.bmp";
        SCROLL1LEFT = "data/texture/유저인터페이스/scroll1left.bmp";
        SCROLL1RIGHT = "data/texture/유저인터페이스/scroll1right.bmp";
        SHOP = "data/texture/유저인터페이스/shop.bmp";
        SYSBOX_BG = "data/texture/유저인터페이스/sysbox_bg.bmp";
        SYSBOX_LD = "data/texture/유저인터페이스/sysbox_ld.bmp";
        SYSBOX_LM = "data/texture/유저인터페이스/sysbox_lm.bmp";
        SYSBOX_LU = "data/texture/유저인터페이스/sysbox_lu.bmp";
        SYSBOX_MD = "data/texture/유저인터페이스/sysbox_md.bmp";
        SYSBOX_MU = "data/texture/유저인터페이스/sysbox_mu.bmp";
        SYSBOX_RD = "data/texture/유저인터페이스/sysbox_rd.bmp";
        SYSBOX_RM = "data/texture/유저인터페이스/sysbox_rm.bmp";
        SYSBOX_RU = "data/texture/유저인터페이스/sysbox_ru.bmp";
        WIN_MSGBOX = "data/texture/유저인터페이스/win_msgbox.bmp";
        WORLDMAP = "data/texture/유저인터페이스/worldmap.bmp";
    }

    /// Shared window chrome and the in-game HUD.
    pub mod basic {
        crate::paths! {
            ARW_LEFT = "data/texture/유저인터페이스/basic_interface/arw_left.bmp";
            ARW_RIGHT = "data/texture/유저인터페이스/basic_interface/arw_right.bmp";
            ARW_RIGHT_ON = "data/texture/유저인터페이스/basic_interface/arw_right_on.bmp";
            BASEWIN_BG = "data/texture/유저인터페이스/basic_interface/basewin_bg.bmp";
            BASEWIN_BG2 = "data/texture/유저인터페이스/basic_interface/basewin_bg2.bmp";
            BASEWIN_MINI = "data/texture/유저인터페이스/basic_interface/basewin_mini.bmp";
            BTNBAR_MID2 = "data/texture/유저인터페이스/basic_interface/btnbar_mid2.bmp";
            BTN_CANCEL2 = "data/texture/유저인터페이스/basic_interface/cancel2.bmp";
            BTN_CANCEL2_A = "data/texture/유저인터페이스/basic_interface/cancel2_a.bmp";
            BTN_CLOSE = "data/texture/유저인터페이스/basic_interface/btn_close.bmp";
            BTN_CLOSE2 = "data/texture/유저인터페이스/basic_interface/close2.bmp";
            BTN_CLOSE2_A = "data/texture/유저인터페이스/basic_interface/close2_a.bmp";
            BTN_CLOSE_A = "data/texture/유저인터페이스/basic_interface/btn_close_a.bmp";
            BTN_CLOSE_B = "data/texture/유저인터페이스/basic_interface/btn_close_b.bmp";
            BTN_DEL = "data/texture/유저인터페이스/basic_interface/del.bmp";
            BTN_DEL_A = "data/texture/유저인터페이스/basic_interface/del_a.bmp";
            BTN_DIALOG_OFF = "data/texture/유저인터페이스/basic_interface/btn_dialog_off.bmp";
            BTN_DIALOG_ON = "data/texture/유저인터페이스/basic_interface/btn_dialog_on.bmp";
            BTN_EQUIP_OFF = "data/texture/유저인터페이스/basic_interface/btn_equip_off.bmp";
            BTN_EQUIP_ON = "data/texture/유저인터페이스/basic_interface/btn_equip_on.bmp";
            BTN_FRIEND_OFF = "data/texture/유저인터페이스/basic_interface/btn_friend_off.bmp";
            BTN_FRIEND_ON = "data/texture/유저인터페이스/basic_interface/btn_friend_on.bmp";
            BTN_ITEMS_OFF = "data/texture/유저인터페이스/basic_interface/btn_items_off.bmp";
            BTN_ITEMS_ON = "data/texture/유저인터페이스/basic_interface/btn_items_on.bmp";
            BTN_MAP_OFF = "data/texture/유저인터페이스/basic_interface/btn_map_off.bmp";
            BTN_MAP_ON = "data/texture/유저인터페이스/basic_interface/btn_map_on.bmp";
            BTN_OFF = "data/texture/유저인터페이스/basic_interface/btn_off.bmp";
            BTN_E_CHANGE_A = "data/texture/유저인터페이스/basic_interface/btn_e_change_a.bmp";
            BTN_E_CHANGE_B = "data/texture/유저인터페이스/basic_interface/btn_e_change_b.bmp";
            BTNBAR_LEFT = "data/texture/유저인터페이스/basic_interface/btnbar_left.bmp";
            BTNBAR_MID = "data/texture/유저인터페이스/basic_interface/btnbar_mid.bmp";
            BTNBAR_RIGHT = "data/texture/유저인터페이스/basic_interface/btnbar_right.bmp";
            ITEMWIN_LEFT = "data/texture/유저인터페이스/basic_interface/itemwin_left.bmp";
            BTN_E_ABILITY_A = "data/texture/유저인터페이스/basic_interface/btn_e_ability_a.bmp";
            BTN_E_ABILITY_B = "data/texture/유저인터페이스/basic_interface/btn_e_ability_b.bmp";
            BTN_E_OFF_A = "data/texture/유저인터페이스/basic_interface/btn_e_off_a.bmp";
            BTN_E_OFF_B = "data/texture/유저인터페이스/basic_interface/btn_e_off_b.bmp";
            BTN_E_OFF_C = "data/texture/유저인터페이스/basic_interface/btn_e_off_c.bmp";
            BTN_OPTION_OFF = "data/texture/유저인터페이스/basic_interface/btn_option_off.bmp";
            BTN_OPTION_ON = "data/texture/유저인터페이스/basic_interface/btn_option_on.bmp";
            BTN_REMAIL = "data/texture/유저인터페이스/basic_interface/remail.bmp";
            BTN_REMAIL_A = "data/texture/유저인터페이스/basic_interface/remail_a.bmp";
            BTN_RETURN = "data/texture/유저인터페이스/basic_interface/return.bmp";
            BTN_RETURN_A = "data/texture/유저인터페이스/basic_interface/return_a.bmp";
            BTN_SEND = "data/texture/유저인터페이스/basic_interface/send.bmp";
            BTN_SEND_A = "data/texture/유저인터페이스/basic_interface/send_a.bmp";
            BTN_SKILL_OFF = "data/texture/유저인터페이스/basic_interface/btn_skill_off.bmp";
            BTN_SKILL_ON = "data/texture/유저인터페이스/basic_interface/btn_skill_on.bmp";
            BTN_STATUS_OFF = "data/texture/유저인터페이스/basic_interface/btn_status_off.bmp";
            BTN_STATUS_ON = "data/texture/유저인터페이스/basic_interface/btn_status_on.bmp";
            COLLECTION_BG = "data/texture/유저인터페이스/basic_interface/collection_bg.bmp";
            COPARISON_DISABLE_CARD_SLOT = "data/texture/유저인터페이스/basic_interface/coparison_disable_card_slot.bmp";
            DIALOG_BG = "data/texture/유저인터페이스/basic_interface/dialog_bg.bmp";
            DIALOG_BTN0 = "data/texture/유저인터페이스/basic_interface/dialog_btn0.bmp";
            DIALOG_BTN1 = "data/texture/유저인터페이스/basic_interface/dialog_btn1.bmp";
            DIALOG_BTN2 = "data/texture/유저인터페이스/basic_interface/dialog_btn2.bmp";
            DIALSCR_DOWN = "data/texture/유저인터페이스/basic_interface/dialscr_down.bmp";
            DIALSCR_UP = "data/texture/유저인터페이스/basic_interface/dialscr_up.bmp";
            ENVELOP = "data/texture/유저인터페이스/basic_interface/envelop.bmp";
            EQUIPWIN_BG = "data/texture/유저인터페이스/basic_interface/equipwin_bg.bmp";
            EQUIPWIN_BG2 = "data/texture/유저인터페이스/basic_interface/equipwin_bg2.bmp";
            EQUIPWIN_SPECIAL = "data/texture/유저인터페이스/basic_interface/equipwin_special.bmp";
            EXCHANGE_BG2 = "data/texture/유저인터페이스/basic_interface/exchange_bg2.bmp";
            GRP_LEADER = "data/texture/유저인터페이스/basic_interface/grp_leader.bmp";
            GRP_ONLINE = "data/texture/유저인터페이스/basic_interface/grp_online.bmp";
            GZEBLUE_LEFT = "data/texture/유저인터페이스/basic_interface/gzeblue_left.bmp";
            GZEBLUE_MID = "data/texture/유저인터페이스/basic_interface/gzeblue_mid.bmp";
            GZEBLUE_RIGHT = "data/texture/유저인터페이스/basic_interface/gzeblue_right.bmp";
            GZERED_LEFT = "data/texture/유저인터페이스/basic_interface/gzered_left.bmp";
            GZERED_MID = "data/texture/유저인터페이스/basic_interface/gzered_mid.bmp";
            GZERED_RIGHT = "data/texture/유저인터페이스/basic_interface/gzered_right.bmp";
            GZE_BG1 = "data/texture/유저인터페이스/basic_interface/gze_bg1.bmp";
            HOMUNINFO_BG = "data/texture/유저인터페이스/basic_interface/homuninfo_bg.bmp";
            ITEMWIN_MID = "data/texture/유저인터페이스/basic_interface/itemwin_mid.bmp";
            ITEM_INVERT = "data/texture/유저인터페이스/basic_interface/item_invert.bmp";
            LV_UP_OFF = "data/texture/유저인터페이스/basic_interface/lv_up_off.bmp";
            LV_UP_ON = "data/texture/유저인터페이스/basic_interface/lv_up_on.bmp";
            MAILLIST1_BG = "data/texture/유저인터페이스/basic_interface/maillist1_bg.bmp";
            MAILLIST2_BG = "data/texture/유저인터페이스/basic_interface/maillist2_bg.bmp";
            MAILLIST3_BG = "data/texture/유저인터페이스/basic_interface/maillist3_bg.bmp";
            MESBTN_010 = "data/texture/유저인터페이스/basic_interface/mesbtn_010.bmp";
            MESBTN_02 = "data/texture/유저인터페이스/basic_interface/mesbtn_02.bmp";
            MESBTN_04 = "data/texture/유저인터페이스/basic_interface/mesbtn_04.bmp";
            MESBTN_05 = "data/texture/유저인터페이스/basic_interface/mesbtn_05.bmp";
            MESBTN_08 = "data/texture/유저인터페이스/basic_interface/mesbtn_08.bmp";
            MESBTN_09 = "data/texture/유저인터페이스/basic_interface/mesbtn_09.bmp";
            QUEST_WINDOW = "data/texture/유저인터페이스/basic_interface/quest_window.bmp";
            SHORTITEM_BG = "data/texture/유저인터페이스/basic_interface/shortitem_bg.bmp";
            SKILL_UP_A = "data/texture/유저인터페이스/basic_interface/skill_up_a.bmp";
            SKILL_UP_B = "data/texture/유저인터페이스/basic_interface/skill_up_b.bmp";
            SKILL_UP_C = "data/texture/유저인터페이스/basic_interface/skill_up_c.bmp";
            STATWIN0_BG = "data/texture/유저인터페이스/basic_interface/statwin0_bg.bmp";
            SYS_BASE_OFF = "data/texture/유저인터페이스/basic_interface/sys_base_off.bmp";
            SYS_BASE_ON = "data/texture/유저인터페이스/basic_interface/sys_base_on.bmp";
            SYS_CLOSE_OFF = "data/texture/유저인터페이스/basic_interface/sys_close_off.bmp";
            SYS_CLOSE_ON = "data/texture/유저인터페이스/basic_interface/sys_close_on.bmp";
            SYS_MINI_OFF = "data/texture/유저인터페이스/basic_interface/sys_mini_off.bmp";
            SYS_MINI_ON = "data/texture/유저인터페이스/basic_interface/sys_mini_on.bmp";
            TAB_ITM_01 = "data/texture/유저인터페이스/basic_interface/tab_itm_01.bmp";
            TAB_ITM_02 = "data/texture/유저인터페이스/basic_interface/tab_itm_02.bmp";
            TAB_ITM_03 = "data/texture/유저인터페이스/basic_interface/tab_itm_03.bmp";
            TAB_QUE_01 = "data/texture/유저인터페이스/basic_interface/tab_que_01.bmp";
            TAB_QUE_02 = "data/texture/유저인터페이스/basic_interface/tab_que_02.bmp";
            TAB_QUE_03 = "data/texture/유저인터페이스/basic_interface/tab_que_03.bmp";
            TITLEBAR_FIX = "data/texture/유저인터페이스/basic_interface/titlebar_fix.bmp";
            TITLEBAR_MID = "data/texture/유저인터페이스/basic_interface/titlebar_mid.bmp";
            TXTBOX_BTN_A = "data/texture/유저인터페이스/basic_interface/txtbox_btn_a.bmp";
            TXTBOX_BTN_B = "data/texture/유저인터페이스/basic_interface/txtbox_btn_b.bmp";
            TXTBOX_BTN_C = "data/texture/유저인터페이스/basic_interface/txtbox_btn_c.bmp";
            W_BASEWIN_BG2 = "data/texture/유저인터페이스/basic_interface/w_basewin_bg2.bmp";
        }

        /// Party-recruitment board buttons.
        pub mod seekparty {
            crate::paths! {
                BTN_CLEAR_A = "data/texture/유저인터페이스/basic_interface/seekparty/btn_clear_a.bmp";
                BTN_CLEAR_B = "data/texture/유저인터페이스/basic_interface/seekparty/btn_clear_b.bmp";
                BTN_CLEAR_C = "data/texture/유저인터페이스/basic_interface/seekparty/btn_clear_c.bmp";
            }
        }
    }

    /// The Renewal status window (2020 and later): base and trait panels, the trait toggle.
    pub mod statuswnd {
        crate::paths! {
            EXPAND_OFF_NORMAL = "data/texture/유저인터페이스/statuswnd/expand_off_normal.bmp";
            EXPAND_OFF_PRESS = "data/texture/유저인터페이스/statuswnd/expand_off_press.bmp";
            EXPAND_ON_NORMAL = "data/texture/유저인터페이스/statuswnd/expand_on_normal.bmp";
            EXPAND_ON_PRESS = "data/texture/유저인터페이스/statuswnd/expand_on_press.bmp";
            W_EX_STATWIN_BG = "data/texture/유저인터페이스/statuswnd/w_ex_statwin_bg.bmp";
            W_STATWIN_BG = "data/texture/유저인터페이스/statuswnd/w_statwin_bg.bmp";
        }
    }

    /// The Renewal menu under the basic info window: its toggle strip, panel and buttons.
    pub mod menu_icon {
        crate::paths! {
            BG_MENU = "data/texture/유저인터페이스/menu_icon/bg_menu.tga";
            BT_EQUIP = "data/texture/유저인터페이스/menu_icon/bt_equip.bmp";
            BT_EQUIP_PRESS = "data/texture/유저인터페이스/menu_icon/bt_equip_press.bmp";
            BT_GUILD = "data/texture/유저인터페이스/menu_icon/bt_guild.bmp";
            BT_GUILD_PRESS = "data/texture/유저인터페이스/menu_icon/bt_guild_press.bmp";
            BT_ITEM = "data/texture/유저인터페이스/menu_icon/bt_item.bmp";
            BT_ITEM_PRESS = "data/texture/유저인터페이스/menu_icon/bt_item_press.bmp";
            BT_MAP = "data/texture/유저인터페이스/menu_icon/bt_map.bmp";
            BT_MAP_PRESS = "data/texture/유저인터페이스/menu_icon/bt_map_press.bmp";
            BT_MENU_CLOSE_NORMAL = "data/texture/유저인터페이스/menu_icon/bt_menu_close_normal.bmp";
            BT_MENU_CLOSE_OVER = "data/texture/유저인터페이스/menu_icon/bt_menu_close_over.bmp";
            BT_MENU_CLOSE_PRESS = "data/texture/유저인터페이스/menu_icon/bt_menu_close_press.bmp";
            BT_MENU_NORMAL = "data/texture/유저인터페이스/menu_icon/bt_menu_normal.bmp";
            BT_MENU_OVER = "data/texture/유저인터페이스/menu_icon/bt_menu_over.bmp";
            BT_MENU_PRESS = "data/texture/유저인터페이스/menu_icon/bt_menu_press.bmp";
            BT_OPTION = "data/texture/유저인터페이스/menu_icon/bt_option.bmp";
            BT_OPTION_PRESS = "data/texture/유저인터페이스/menu_icon/bt_option_press.bmp";
            BT_PARTY = "data/texture/유저인터페이스/menu_icon/bt_party.bmp";
            BT_PARTY_PRESS = "data/texture/유저인터페이스/menu_icon/bt_party_press.bmp";
            BT_QUEST = "data/texture/유저인터페이스/menu_icon/bt_quest.bmp";
            BT_QUEST_PRESS = "data/texture/유저인터페이스/menu_icon/bt_quest_press.bmp";
            BT_SKILL = "data/texture/유저인터페이스/menu_icon/bt_skill.bmp";
            BT_SKILL_PRESS = "data/texture/유저인터페이스/menu_icon/bt_skill_press.bmp";
            BT_STATUS = "data/texture/유저인터페이스/menu_icon/bt_status.bmp";
            BT_STATUS_PRESS = "data/texture/유저인터페이스/menu_icon/bt_status_press.bmp";
        }
    }

    /// Card illustrations.
    pub mod cardbmp {
        pub fn named(name: &str) -> String {
            format!("data/texture/유저인터페이스/cardbmp/{name}.bmp")
        }
    }

    /// Item collection illustrations.
    pub mod collection {
        pub fn named(name: &str) -> String {
            format!("data/texture/유저인터페이스/collection/{name}.bmp")
        }
    }

    /// Pet illustrations, keyed by the pet mob.
    pub mod illust {
        crate::paths! {
            PET_ALICE = "data/texture/유저인터페이스/illust/펫_ALICE.bmp";
            PET_BABY_DESERT_WOLF = "data/texture/유저인터페이스/illust/펫_데져트울프새끼.bmp";
            PET_BACSOJIN = "data/texture/유저인터페이스/illust/펫_BACSOJIN.bmp";
            PET_BAPHOMET_JR = "data/texture/유저인터페이스/illust/펫_바포메트.bmp";
            PET_BONGUN = "data/texture/유저인터페이스/illust/펫_본건.bmp";
            PET_CHONCHON = "data/texture/유저인터페이스/illust/펫_촌촌.bmp";
            PET_CHRISTMAS_SNOW_RABBIT = "data/texture/유저인터페이스/illust/펫_크리스마스_눈토끼.bmp";
            PET_CHUNG_E = "data/texture/유저인터페이스/illust/펫_청이.bmp";
            PET_CIVIL_SERVANT = "data/texture/유저인터페이스/illust/펫_CIVIL_SERVANT.bmp";
            PET_DELETER = "data/texture/유저인터페이스/illust/펫_지상딜리터.bmp";
            PET_DEVIRUCHI = "data/texture/유저인터페이스/illust/펫_데비루치.bmp";
            PET_DIABOLIC = "data/texture/유저인터페이스/illust/펫_디아볼릭.bmp";
            PET_DOKEBI = "data/texture/유저인터페이스/illust/펫_도깨비.bmp";
            PET_DROPS = "data/texture/유저인터페이스/illust/펫_드롭프스.bmp";
            PET_DULLAHAN = "data/texture/유저인터페이스/illust/펫_DULLAHAN.bmp";
            PET_GOBLIN_DAGGER = "data/texture/유저인터페이스/illust/펫_고블린_단검.bmp";
            PET_GOBLIN_EVENT = "data/texture/유저인터페이스/illust/펫_고블린_이벤트.bmp";
            PET_GOBLIN_FLAIL = "data/texture/유저인터페이스/illust/펫_고블린_플레일.bmp";
            PET_GOBLIN_HAMMER = "data/texture/유저인터페이스/illust/펫_고블린_해머.bmp";
            PET_GOBLIN_LEADER = "data/texture/유저인터페이스/illust/펫_GOBLIN_LEADER.bmp";
            PET_GOLEM = "data/texture/유저인터페이스/illust/펫_GOLEM.bmp";
            PET_HUNTER_FLY = "data/texture/유저인터페이스/illust/펫_헌터플라이.bmp";
            PET_IMP = "data/texture/유저인터페이스/illust/펫_IMP.bmp";
            PET_INCUBUS = "data/texture/유저인터페이스/illust/펫_INCUBUS.bmp";
            PET_ISIS = "data/texture/유저인터페이스/illust/펫_이시스.bmp";
            PET_J_TAINI = "data/texture/유저인터페이스/illust/펫_j_taini.bmp";
            PET_LEAF_CAT = "data/texture/유저인터페이스/illust/펫_LEAF_CAT.bmp";
            PET_LOLI_RURI = "data/texture/유저인터페이스/illust/펫_LOLI_RURI.BMP";
            PET_LUNATIC = "data/texture/유저인터페이스/illust/펫_루나틱.bmp";
            PET_MARIONETTE = "data/texture/유저인터페이스/illust/펫_MARIONETTE.bmp";
            PET_MEDUSA = "data/texture/유저인터페이스/illust/펫_MEDUSA.bmp";
            PET_MIYABI_NINGYO = "data/texture/유저인터페이스/illust/펫_MIYABI_NINGYO.bmp";
            PET_MUNAK = "data/texture/유저인터페이스/illust/펫_무낙.bmp";
            PET_NIGHTMARE_TERROR = "data/texture/유저인터페이스/illust/펫_NIGHTMARE_TERROR.bmp";
            PET_ORC_WARRIOR = "data/texture/유저인터페이스/illust/펫_오크워리어.bmp";
            PET_PECOPECO = "data/texture/유저인터페이스/illust/펫_페코페코.bmp";
            PET_PETITE = "data/texture/유저인터페이스/illust/펫_쁘띠.bmp";
            PET_PICKY = "data/texture/유저인터페이스/illust/펫_픽키.bmp";
            PET_POISON_SPORE = "data/texture/유저인터페이스/illust/펫_포이즌스포아.bmp";
            PET_POPORING = "data/texture/유저인터페이스/illust/펫_포포링.bmp";
            PET_PORING = "data/texture/유저인터페이스/illust/펫_포링.bmp";
            PET_RICE_CAKE = "data/texture/유저인터페이스/illust/펫_떡_이벤트.bmp";
            PET_ROCKER = "data/texture/유저인터페이스/illust/펫_로커.bmp";
            PET_SAVAGE_BABE = "data/texture/유저인터페이스/illust/펫_세비지베베.bmp";
            PET_SHINOBI = "data/texture/유저인터페이스/illust/펫_SHINOBI.bmp";
            PET_SMOKIE = "data/texture/유저인터페이스/illust/펫_스모키.bmp";
            PET_SOHEE = "data/texture/유저인터페이스/illust/펫_소희.bmp";
            PET_SPORE = "data/texture/유저인터페이스/illust/펫_스포아.bmp";
            PET_STEEL_CHONCHON = "data/texture/유저인터페이스/illust/펫_스틸촌촌.bmp";
            PET_STONE_SHOOTER = "data/texture/유저인터페이스/illust/펫_STONE_SHOOTER.bmp";
            PET_SUCCUBUS = "data/texture/유저인터페이스/illust/펫_SUCCUBUS.bmp";
            PET_WANDERER = "data/texture/유저인터페이스/illust/펫_배회하는자.bmp";
            PET_WHISPER = "data/texture/유저인터페이스/illust/펫_WHISPER.BMP";
            PET_WICKED_NYMPH = "data/texture/유저인터페이스/illust/펫_WICKED_NYMPH.bmp";
            PET_YOYO = "data/texture/유저인터페이스/illust/펫_요요.bmp";
            PET_ZHERLTHSH = "data/texture/유저인터페이스/illust/펫_ZHERLTHSH.bmp";
        }

        pub fn named(name: &str) -> String {
            format!("data/texture/유저인터페이스/illust/{name}.bmp")
        }
    }

    /// Inventory item icons.
    pub mod item {
        crate::paths! {
            CAT_PAW_HAIRPIN = "data/texture/유저인터페이스/item/고양이발머리핀.bmp";
        }

        pub fn icon(name: &str) -> String {
            format!("data/texture/유저인터페이스/item/{name}.bmp")
        }
    }

    /// The refine window (rAthena's refine UI): kRO's forge art, frames in play order.
    pub mod refining {
        crate::paths! {
            WAIT_0 = "data/texture/유저인터페이스/refining_renewal/bg_refining_wait_00.bmp";
            WAIT_1 = "data/texture/유저인터페이스/refining_renewal/bg_refining_wait_01.bmp";
            WAIT_2 = "data/texture/유저인터페이스/refining_renewal/bg_refining_wait_02.bmp";
            WAIT_3 = "data/texture/유저인터페이스/refining_renewal/bg_refining_wait_03.bmp";
            READY_A_0 = "data/texture/유저인터페이스/refining_renewal/bg_refininga_ready_00.bmp";
            READY_A_1 = "data/texture/유저인터페이스/refining_renewal/bg_refininga_ready_01.bmp";
            READY_A_2 = "data/texture/유저인터페이스/refining_renewal/bg_refininga_ready_02.bmp";
            READY_B_0 = "data/texture/유저인터페이스/refining_renewal/bg_refiningb_ready_00.bmp";
            READY_B_1 = "data/texture/유저인터페이스/refining_renewal/bg_refiningb_ready_01.bmp";
            READY_B_2 = "data/texture/유저인터페이스/refining_renewal/bg_refiningb_ready_02.bmp";
            PROCESS_A_0 = "data/texture/유저인터페이스/refining_renewal/bg_refininga_process_00.bmp";
            PROCESS_A_1 = "data/texture/유저인터페이스/refining_renewal/bg_refininga_process_01.bmp";
            PROCESS_A_2 = "data/texture/유저인터페이스/refining_renewal/bg_refininga_process_02.bmp";
            PROCESS_A_3 = "data/texture/유저인터페이스/refining_renewal/bg_refininga_process_03.bmp";
            PROCESS_A_4 = "data/texture/유저인터페이스/refining_renewal/bg_refininga_process_04.bmp";
            PROCESS_A_5 = "data/texture/유저인터페이스/refining_renewal/bg_refininga_process_05.bmp";
            PROCESS_A_6 = "data/texture/유저인터페이스/refining_renewal/bg_refininga_process_06.bmp";
            PROCESS_A_7 = "data/texture/유저인터페이스/refining_renewal/bg_refininga_process_07.bmp";
            PROCESS_A_8 = "data/texture/유저인터페이스/refining_renewal/bg_refininga_process_08.bmp";
            PROCESS_B_0 = "data/texture/유저인터페이스/refining_renewal/bg_refiningb_process_00.bmp";
            PROCESS_B_1 = "data/texture/유저인터페이스/refining_renewal/bg_refiningb_process_01.bmp";
            PROCESS_B_2 = "data/texture/유저인터페이스/refining_renewal/bg_refiningb_process_02.bmp";
            PROCESS_B_3 = "data/texture/유저인터페이스/refining_renewal/bg_refiningb_process_03.bmp";
            PROCESS_B_4 = "data/texture/유저인터페이스/refining_renewal/bg_refiningb_process_04.bmp";
            PROCESS_B_5 = "data/texture/유저인터페이스/refining_renewal/bg_refiningb_process_05.bmp";
            PROCESS_B_6 = "data/texture/유저인터페이스/refining_renewal/bg_refiningb_process_06.bmp";
            PROCESS_B_7 = "data/texture/유저인터페이스/refining_renewal/bg_refiningb_process_07.bmp";
            PROCESS_B_8 = "data/texture/유저인터페이스/refining_renewal/bg_refiningb_process_08.bmp";
            PROCESS_9 = "data/texture/유저인터페이스/refining_renewal/bg_refining_process_09.bmp";
            PROCESS_10 = "data/texture/유저인터페이스/refining_renewal/bg_refining_process_10.bmp";
            PROCESS_11 = "data/texture/유저인터페이스/refining_renewal/bg_refining_process_11.bmp";
            PROCESS_12 = "data/texture/유저인터페이스/refining_renewal/bg_refining_process_12.bmp";
            PROCESS_13 = "data/texture/유저인터페이스/refining_renewal/bg_refining_process_13.bmp";
            SUCCESS_0 = "data/texture/유저인터페이스/refining_renewal/bg_refining_success_00.bmp";
            SUCCESS_1 = "data/texture/유저인터페이스/refining_renewal/bg_refining_success_01.bmp";
            SUCCESS_2 = "data/texture/유저인터페이스/refining_renewal/bg_refining_success_02.bmp";
            SUCCESS_3 = "data/texture/유저인터페이스/refining_renewal/bg_refining_success_03.bmp";
            SUCCESS_4 = "data/texture/유저인터페이스/refining_renewal/bg_refining_success_04.bmp";
            SUCCESS_5 = "data/texture/유저인터페이스/refining_renewal/bg_refining_success_05.bmp";
            SUCCESS_6 = "data/texture/유저인터페이스/refining_renewal/bg_refining_success_06.bmp";
            SUCCESS_7 = "data/texture/유저인터페이스/refining_renewal/bg_refining_success_07.bmp";
            SUCCESS_8 = "data/texture/유저인터페이스/refining_renewal/bg_refining_success_08.bmp";
            SUCCESS_9 = "data/texture/유저인터페이스/refining_renewal/bg_refining_success_09.bmp";
            SUCCESS_10 = "data/texture/유저인터페이스/refining_renewal/bg_refining_success_10.bmp";
            SUCCESS_11 = "data/texture/유저인터페이스/refining_renewal/bg_refining_success_11.bmp";
            SUCCESS_12 = "data/texture/유저인터페이스/refining_renewal/bg_refining_success_12.bmp";
            SUCCESS_13 = "data/texture/유저인터페이스/refining_renewal/bg_refining_success_13.bmp";
            SUCCESS_14 = "data/texture/유저인터페이스/refining_renewal/bg_refining_success_14.bmp";
            SUCCESS_15 = "data/texture/유저인터페이스/refining_renewal/bg_refining_success_15.bmp";
            SUCCESS_16 = "data/texture/유저인터페이스/refining_renewal/bg_refining_success_16.bmp";
            FAIL_0 = "data/texture/유저인터페이스/refining_renewal/bg_refining_fail_00.bmp";
            FAIL_1 = "data/texture/유저인터페이스/refining_renewal/bg_refining_fail_01.bmp";
            FAIL_2 = "data/texture/유저인터페이스/refining_renewal/bg_refining_fail_02.bmp";
            FAIL_3 = "data/texture/유저인터페이스/refining_renewal/bg_refining_fail_03.bmp";
            FAIL_4 = "data/texture/유저인터페이스/refining_renewal/bg_refining_fail_04.bmp";
            FAIL_5 = "data/texture/유저인터페이스/refining_renewal/bg_refining_fail_05.bmp";
            FAIL_6 = "data/texture/유저인터페이스/refining_renewal/bg_refining_fail_06.bmp";
            FAIL_7 = "data/texture/유저인터페이스/refining_renewal/bg_refining_fail_07.bmp";
            FAIL_8 = "data/texture/유저인터페이스/refining_renewal/bg_refining_fail_08.bmp";
            FAIL_9 = "data/texture/유저인터페이스/refining_renewal/bg_refining_fail_09.bmp";
            FAIL_10 = "data/texture/유저인터페이스/refining_renewal/bg_refining_fail_10.bmp";
            FAIL_11 = "data/texture/유저인터페이스/refining_renewal/bg_refining_fail_11.bmp";
            FAIL_12 = "data/texture/유저인터페이스/refining_renewal/bg_refining_fail_12.bmp";
            FAIL_13 = "data/texture/유저인터페이스/refining_renewal/bg_refining_fail_13.bmp";
            FAIL_14 = "data/texture/유저인터페이스/refining_renewal/bg_refining_fail_14.bmp";
            FAIL_15 = "data/texture/유저인터페이스/refining_renewal/bg_refining_fail_15.bmp";
            FAIL_16 = "data/texture/유저인터페이스/refining_renewal/bg_refining_fail_16.bmp";
            FAIL_17 = "data/texture/유저인터페이스/refining_renewal/bg_refining_fail_17.bmp";
            FAIL_18 = "data/texture/유저인터페이스/refining_renewal/bg_refining_fail_18.bmp";
            FAIL_19 = "data/texture/유저인터페이스/refining_renewal/bg_refining_fail_19.bmp";
            BT_REFINE_DISABLE = "data/texture/유저인터페이스/refining_renewal/bt_refining_disable.bmp";
            BT_REFINE_NORMAL = "data/texture/유저인터페이스/refining_renewal/bt_refining_normal.bmp";
            BT_REFINE_PRESS = "data/texture/유저인터페이스/refining_renewal/bt_refining_press.bmp";
            BT_AGAIN_SUCCESS_DISABLE = "data/texture/유저인터페이스/refining_renewal/bt_refining_again_success_disable.bmp";
            BT_AGAIN_SUCCESS_NORMAL = "data/texture/유저인터페이스/refining_renewal/bt_refining_again_success_normal.bmp";
            BT_AGAIN_SUCCESS_PRESS = "data/texture/유저인터페이스/refining_renewal/bt_refining_again_success_press.bmp";
            BT_RETURN_SUCCESS_NORMAL = "data/texture/유저인터페이스/refining_renewal/bt_refining_return_success_normal.bmp";
            BT_RETURN_SUCCESS_PRESS = "data/texture/유저인터페이스/refining_renewal/bt_refining_return_success_press.bmp";
            BT_AGAIN_FAIL_DISABLE = "data/texture/유저인터페이스/refining_renewal/bt_refining_again_fail_disable.bmp";
            BT_AGAIN_FAIL_NORMAL = "data/texture/유저인터페이스/refining_renewal/bt_refining_again_fail_normal.bmp";
            BT_AGAIN_FAIL_PRESS = "data/texture/유저인터페이스/refining_renewal/bt_refining_again_fail_press.bmp";
            BT_RETURN_FAIL_NORMAL = "data/texture/유저인터페이스/refining_renewal/bt_refining_return_fail_normal.bmp";
            BT_RETURN_FAIL_PRESS = "data/texture/유저인터페이스/refining_renewal/bt_refining_return_fail_press.bmp";
            ICO_NOTICE = "data/texture/유저인터페이스/refining_renewal/ico_notice.bmp";
            SLOT_ITEM_OFF = "data/texture/유저인터페이스/refining_renewal/slot_item_off.bmp";
            SLOT_BLUE_0 = "data/texture/유저인터페이스/refining_renewal/slot_select_blue0.bmp";
            SLOT_BLUE_1 = "data/texture/유저인터페이스/refining_renewal/slot_select_blue1.bmp";
            SLOT_BLUE_2 = "data/texture/유저인터페이스/refining_renewal/slot_select_blue2.bmp";
            SLOT_BLUE_3 = "data/texture/유저인터페이스/refining_renewal/slot_select_blue3.bmp";
            SLOT_RED = "data/texture/유저인터페이스/refining_renewal/slot_select_red.bmp";
        }
    }

    /// The stylist window (`styleshop/`), as kRO's 2022 client draws it
    pub mod styleshop {
        crate::paths! {
            TAB_HAIR_ON = "data/texture/유저인터페이스/styleshop/tab_hair_on.bmp";
            TAB_HAIR_OFF = "data/texture/유저인터페이스/styleshop/tab_hair_off.bmp";
            TAB_HAIR_OVER = "data/texture/유저인터페이스/styleshop/tab_hair_off_over.bmp";
            TAB_ACCESSORIES_ON = "data/texture/유저인터페이스/styleshop/tab_accessories_on.bmp";
            TAB_ACCESSORIES_OFF = "data/texture/유저인터페이스/styleshop/tab_accessories_off.bmp";
            TAB_ACCESSORIES_OVER = "data/texture/유저인터페이스/styleshop/tab_accessories_off_over.bmp";
            TAB_COSTUME_ON = "data/texture/유저인터페이스/styleshop/tab_2ndcostume_on.bmp";
            TAB_COSTUME_OFF = "data/texture/유저인터페이스/styleshop/tab_2ndcostume_off.bmp";
            TAB_COSTUME_OVER = "data/texture/유저인터페이스/styleshop/tab_2ndcostume_off_over.bmp";
            TAB_COSTUME_DISABLE = "data/texture/유저인터페이스/styleshop/tab_2ndcostume_disable.bmp";
            TAB_FAVORITE_ON = "data/texture/유저인터페이스/styleshop/tab_favorite_on.bmp";
            TAB_FAVORITE_OFF = "data/texture/유저인터페이스/styleshop/tab_favorite_off.bmp";
            TAB_FAVORITE_OVER = "data/texture/유저인터페이스/styleshop/tab_favorite_off_over.bmp";
            ARROW_L = "data/texture/유저인터페이스/styleshop/arrow_chr_l_upper_out.bmp";
            ARROW_L_OVER = "data/texture/유저인터페이스/styleshop/arrow_chr_l_upper_over.bmp";
            ARROW_R = "data/texture/유저인터페이스/styleshop/arrow_chr_r_upper_out.bmp";
            ARROW_R_OVER = "data/texture/유저인터페이스/styleshop/arrow_chr_r_upper_over.bmp";
            ROTATE_L = "data/texture/유저인터페이스/styleshop/btn_rotatearrow_l_out.bmp";
            ROTATE_L_OVER = "data/texture/유저인터페이스/styleshop/btn_rotatearrow_l_over.bmp";
            ROTATE_L_PRESS = "data/texture/유저인터페이스/styleshop/btn_rotatearrow_l_press.bmp";
            ROTATE_R = "data/texture/유저인터페이스/styleshop/btn_rotatearrow_r_out.bmp";
            ROTATE_R_OVER = "data/texture/유저인터페이스/styleshop/btn_rotatearrow_r_over.bmp";
            ROTATE_R_PRESS = "data/texture/유저인터페이스/styleshop/btn_rotatearrow_r_press.bmp";
            FAVORITE_OFF = "data/texture/유저인터페이스/styleshop/btn_favorite_off.bmp";
            FAVORITE_OFF_OVER = "data/texture/유저인터페이스/styleshop/btn_favorite_off_over.bmp";
            FAVORITE_ON = "data/texture/유저인터페이스/styleshop/btn_favorite_on.bmp";
            RESET = "data/texture/유저인터페이스/styleshop/btn_refresh_out.bmp";
            RESET_OVER = "data/texture/유저인터페이스/styleshop/btn_refresh_over.bmp";
            RESET_PRESS = "data/texture/유저인터페이스/styleshop/btn_refresh_press.bmp";
            PALETTE_BG = "data/texture/유저인터페이스/styleshop/bg_styling_color.bmp";
            PALETTE_ICON = "data/texture/유저인터페이스/styleshop/icon_color_off.bmp";
            COLOR_NONE = "data/texture/유저인터페이스/styleshop/color00_off.bmp";
            COLOR_NONE_ON = "data/texture/유저인터페이스/styleshop/color00_on.bmp";
            SHADOW_CHR = "data/texture/유저인터페이스/styleshop/shadow_chr_off.bmp";
            SHADOW_ITEM = "data/texture/유저인터페이스/styleshop/shadow_item_off.bmp";
            SHADOW_ITEM_ON = "data/texture/유저인터페이스/styleshop/shadow_item_on.bmp";
            CHECK_ON = "data/texture/유저인터페이스/styleshop/check_able.bmp";
            CHECK_OFF = "data/texture/유저인터페이스/styleshop/notcheck.bmp";
            CHECK_DISABLE = "data/texture/유저인터페이스/styleshop/check_disable.bmp";
            ICON_ZENY = "data/texture/유저인터페이스/styleshop/icon_zeny.bmp";
            LINE_ROW = "data/texture/유저인터페이스/styleshop/line_dotted_1px_light.bmp";
            LINE_COLUMN = "data/texture/유저인터페이스/styleshop/line_dotted_2px.bmp";
            BUY = "data/texture/유저인터페이스/styleshop/btn_buy_out.bmp";
            BUY_OVER = "data/texture/유저인터페이스/styleshop/btn_buy_over.bmp";
            BUY_PRESS = "data/texture/유저인터페이스/styleshop/btn_buy_press.bmp";
            BUY_DISABLE = "data/texture/유저인터페이스/styleshop/btn_buy_disable.bmp";
            COLOR_1 = "data/texture/유저인터페이스/styleshop/color01_off.bmp";
            COLOR_1_ON = "data/texture/유저인터페이스/styleshop/color01_on.bmp";
            COLOR_1_OVER = "data/texture/유저인터페이스/styleshop/color01_off_over.bmp";
            COLOR_2 = "data/texture/유저인터페이스/styleshop/color02_off.bmp";
            COLOR_2_ON = "data/texture/유저인터페이스/styleshop/color02_on.bmp";
            COLOR_2_OVER = "data/texture/유저인터페이스/styleshop/color02_off_over.bmp";
            COLOR_3 = "data/texture/유저인터페이스/styleshop/color03_off.bmp";
            COLOR_3_ON = "data/texture/유저인터페이스/styleshop/color03_on.bmp";
            COLOR_3_OVER = "data/texture/유저인터페이스/styleshop/color03_off_over.bmp";
            COLOR_4 = "data/texture/유저인터페이스/styleshop/color04_off.bmp";
            COLOR_4_ON = "data/texture/유저인터페이스/styleshop/color04_on.bmp";
            COLOR_4_OVER = "data/texture/유저인터페이스/styleshop/color04_off_over.bmp";
            COLOR_5 = "data/texture/유저인터페이스/styleshop/color05_off.bmp";
            COLOR_5_ON = "data/texture/유저인터페이스/styleshop/color05_on.bmp";
            COLOR_5_OVER = "data/texture/유저인터페이스/styleshop/color05_off_over.bmp";
            COLOR_6 = "data/texture/유저인터페이스/styleshop/color06_off.bmp";
            COLOR_6_ON = "data/texture/유저인터페이스/styleshop/color06_on.bmp";
            COLOR_6_OVER = "data/texture/유저인터페이스/styleshop/color06_off_over.bmp";
            COLOR_7 = "data/texture/유저인터페이스/styleshop/color07_off.bmp";
            COLOR_7_ON = "data/texture/유저인터페이스/styleshop/color07_on.bmp";
            COLOR_7_OVER = "data/texture/유저인터페이스/styleshop/color07_off_over.bmp";
            COLOR_8 = "data/texture/유저인터페이스/styleshop/color08_off.bmp";
            COLOR_8_ON = "data/texture/유저인터페이스/styleshop/color08_on.bmp";
            COLOR_8_OVER = "data/texture/유저인터페이스/styleshop/color08_off_over.bmp";
        }
    }

    /// Login, server-select and character-create screens.
    pub mod login {
        crate::paths! {
            BOX_SELECT = "data/texture/유저인터페이스/login_interface/box_select.bmp";
            BTN_CONNECT = "data/texture/유저인터페이스/login_interface/btn_connect.bmp";
            BTN_CONNECT_A = "data/texture/유저인터페이스/login_interface/btn_connect_a.bmp";
            BTN_CONNECT_B = "data/texture/유저인터페이스/login_interface/btn_connect_b.bmp";
            BTN_EXIT = "data/texture/유저인터페이스/login_interface/btn_exit.bmp";
            BTN_EXIT_A = "data/texture/유저인터페이스/login_interface/btn_exit_a.bmp";
            BTN_EXIT_B = "data/texture/유저인터페이스/login_interface/btn_exit_b.bmp";
            CHK_SAVEOFF = "data/texture/유저인터페이스/login_interface/chk_saveoff.bmp";
            CHK_SAVEON = "data/texture/유저인터페이스/login_interface/chk_saveon.bmp";
            NAME_EDIT = "data/texture/유저인터페이스/login_interface/name-edit.bmp";
            WIN_LOGIN = "data/texture/유저인터페이스/login_interface/win_login.bmp";
            WIN_MAKE2 = "data/texture/유저인터페이스/login_interface/win_make2.bmp";
            WIN_SELECT = "data/texture/유저인터페이스/login_interface/win_select.bmp";
            WIN_SERVICE = "data/texture/유저인터페이스/login_interface/win_service.bmp";
        }
    }

    /// Minimap frame and per-map minimap images.
    pub mod minimap {
        crate::paths! {
            MAP_ARROW = "data/texture/유저인터페이스/map/map_arrow.bmp";
            MAP_MINUS0 = "data/texture/유저인터페이스/map/map_minus0.bmp";
            MAP_MINUS1 = "data/texture/유저인터페이스/map/map_minus1.bmp";
            MAP_PLUS0 = "data/texture/유저인터페이스/map/map_plus0.bmp";
            MAP_PLUS1 = "data/texture/유저인터페이스/map/map_plus1.bmp";
            PRONTERA = "data/texture/유저인터페이스/map/prontera.bmp";
        }

        pub fn of(map_name: &str) -> String {
            format!("data/texture/유저인터페이스/map/{map_name}.bmp")
        }
    }
}

#[cfg(test)]
mod tests {
    /// `all_static_paths` lists modules by hand, so a new module can be missed.
    /// Each declared path is one assignment line inside a `paths!` block.
    #[test]
    fn every_declared_path_is_enumerated() {
        let declared = include_str!("lib.rs")
            .lines()
            .filter(|l| l.contains(" = \"data/"))
            .count();
        let enumerated = super::all_static_paths();
        assert_eq!(
            declared,
            enumerated.len(),
            "a paths! module is missing from all_static_paths()"
        );

        let mut sorted = enumerated.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), enumerated.len(), "duplicate path in registry");
    }
}
