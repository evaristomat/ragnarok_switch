use crate::banner::BannerKind;
use crate::boss_info::BossInfoKind;
use crate::companion::{ElementalPar, SummonPar};
use crate::guild::{GuildBanEntry, GuildMember, GuildPosition, GuildRelation, GuildSkill};
use crate::inventory::{EquipmentItemData, NormalItemData};
use crate::item::Item;
use crate::mail::{MailEntry, OpenedMail};
use crate::minimap_mark::MarkAction;
use crate::monster_info::MonsterInfo;
use crate::show_digit::ShowDigitMode;
use crate::targeting::MapProperties;
use models::enums::action::ActionType;
use models::enums::skill::SkillTargetType;
use models::enums::skill_enums::SkillEnum;
use models::enums::vanish::VanishType;

#[derive(Debug)]
pub enum GameEvent {
    LoginAccepted {
        account_id: u32,
        login_id1: i32,
        login_id2: u32,
        sex: u8,
        servers: Vec<ServerInfo>,
        /// For the web server (guild emblems); empty before 0x0ac4
        web_auth_token: String,
    },
    LoginRefused {
        error_code: u8,
    },
    CharServerConnectRefused {
        error_code: u8,
    },
    CharacterListReceived {
        characters: Vec<CharacterInfo>,
    },
    ZoneServerConnectInfo {
        char_id: u32,
        map_name: String,
        ip: u32,
        port: i16,
    },
    /// The destination map is hosted by a different zone server: reconnect there
    /// and re-enter.
    ZoneServerChanged {
        map_name: String,
        ip: u32,
        port: i16,
    },
    AccessibleMapsReceived {
        maps: Vec<AccessibleMap>,
    },
    MapEntered {
        x: u16,
        y: u16,
        dir: u8,
        tick: u32,
    },
    MapChanged {
        map_name: String,
        x: i16,
        y: i16,
    },
    MapPropertyChanged(MapProperties),
    ServerTick {
        server_tick: u32,
        local_send_time_ms: u32,
    },
    Disconnected(String),
    RequestLogin {
        username: String,
        password: String,
    },
    SelectLoginServer {
        index: usize,
    },
    RequestSelectServer {
        index: usize,
    },
    RequestSelectCharacter {
        slot: u8,
    },
    RequestCreateCharacter {
        slot: u8,
    },
    RequestMakeCharacter {
        name: String,
        slot: u8,
        hair_style: u16,
        hair_color: u16,
        /// Starting job (Novice 0, Summoner 4218) and sex (0 female, 1 male).
        job: u16,
        sex: u8,
    },
    CharacterCreated {
        character: CharacterInfo,
    },
    CharacterCreateFailed {
        error_code: u8,
    },
    CancelCreateCharacter,
    RequestDeleteCharacterReserve {
        gid: u32,
    },
    RequestDeleteCharacterConfirm {
        gid: u32,
        birthdate: String,
    },
    RequestDeleteCharacterCancel {
        gid: u32,
    },
    CharacterDeleteReserved {
        gid: u32,
        result: u32,
        delete_reserved_date: i32,
    },
    CharacterDeleted {
        gid: u32,
        result: u32,
    },
    CharacterDeleteCancelled {
        gid: u32,
        result: u32,
    },
    RequestMove {
        x: u16,
        y: u16,
    },
    PlayerMoved {
        start_x: u16,
        start_y: u16,
        dest_x: u16,
        dest_y: u16,
        start_time: u32,
    },
    BackToLogin,
    BackToServerSelect,
    BackToCharacterSelect,
    ReturnToSavePoint,
    RequestStandingResurrection,
    RequestMapRecoveryWarp,
    RestartAck,
    QuitGame,
    DisconnectAck {
        allowed: bool,
    },
    EntitySpawned {
        gid: u32,
        aid: u32,
        job: u16,
        speed: u16,
        sex: u8,
        head: u16,
        weapon: u32,
        shield: u32,
        /// The garment's view, 0 for none
        robe: u16,
        head_top: u16,
        head_mid: u16,
        head_bottom: u16,
        hair_color: u16,
        /// The clothes' dye (the body palette), 0 for none
        cloth_color: u16,
        x: u16,
        y: u16,
        direction: u8,
        body_state: i16,
        health_state: i16,
        effect_state: i32,
        base_level: i16,
        is_boss: bool,
        posture: u8,
        guild_id: u32,
        guild_emblem_version: i32,
        is_new_entry: bool,
    },
    EntityMoved {
        gid: u32,
        start_x: u16,
        start_y: u16,
        dest_x: u16,
        dest_y: u16,
        start_time: u32,
    },
    EntityVanished {
        gid: u32,
        vanish_type: VanishType,
    },
    EntityStopMove {
        gid: u32,
        x: u16,
        y: u16,
    },
    EntityHighJumped {
        gid: u32,
        x: u16,
        y: u16,
    },
    EntityAction {
        gid: u32,
        target_gid: u32,
        action: ActionType,
        damage: i32,
        left_damage: i32,
        attack_mt: i32,
        attacked_mt: i32,
        start_time: u32,
        count: i16,
    },
    EntityDirectionChanged {
        gid: u32,
        head_dir: u8,
        dir: u8,
    },
    EntityNameReceived {
        gid: u32,
        name: String,
    },
    /// Reply to a name lookup by char id; the char need not be on the map.
    CharNameReceived {
        char_id: u32,
        name: String,
    },
    EntityNamesReceived {
        gid: u32,
        name: String,
        /// Monsters carry their `show_mob_info` string here instead of a party name.
        party_name: String,
        guild_name: String,
        position_name: String,
        /// The title the player wears (`titletable.lub`'s id), 0 for none
        title_id: i32,
    },
    /// The achievements whose reward the character has claimed, from the full list (0x0a23): kRO's client lists
    /// their titles (0xcfe3d0, the entry's `rewarded`)
    AchievementsRewarded {
        achievement_ids: Vec<u32>,
    },
    /// The server's answer to a title change (0x0a2f): worn when `ok`, else refused (not one of the character's)
    TitleChangeAnswered {
        ok: bool,
        title_id: i32,
    },
    /// The swap equipment set, whole (0x0a9b, at map load): each item's inventory index and locations
    EquipSwitchList {
        entries: Vec<(u16, u32)>,
    },
    /// The answer to adding an item to the set (0x0a98): `flag` 0 taken; else rAthena's equip result (1 too low a
    /// level, 2 refused) or kRO's 3, which its client words "There is no item to replace." (0xd1b620)
    EquipSwitchAdded {
        index: u16,
        location: u32,
        flag: u16,
    },
    /// The answer to taking an item out of the set (0x0a9a): `failed` 0 taken out, 1 refused (0xd1c1b0)
    EquipSwitchRemoved {
        index: u16,
        failed: u16,
    },
    /// The answer to CHANGE (0x0a9d): rAthena's 0 for swapped and for an empty set alike, kRO's 1 and others for
    /// refusals (0xd1b890)
    EquipSwitchAnswered {
        failed: u16,
    },
    EntityHpChanged {
        gid: u32,
        hp: u32,
        max_hp: u32,
    },
    EntityOptionChanged {
        gid: u32,
        body_state: i16,
        health_state: i16,
        effect_state: i32,
    },
    EntityOpt3Changed {
        gid: u32,
        effect_state: i32,
        base_level: i32,
        opt3: i32,
    },
    PlayEffectOnEntity {
        gid: u32,
        effect_id: i32,
        value: Option<i32>,
    },
    PlayMiscEffectOnEntity {
        gid: u32,
        code: u8,
    },
    SpiritsChanged {
        gid: u32,
        count: u8,
    },
    /// 0x08cf: a Kagerou's or Oboro's charms, 0 to 10; `element` 1 water, 2 earth, 3 fire, 4 wind, 0 with none left
    CharmsChanged {
        gid: u32,
        element: u8,
        count: u8,
    },
    /// 0x0b73: a Soul Reaper's souls, 0 to 20
    SoulEnergyChanged {
        gid: u32,
        count: u8,
    },
    /// 0x0440: a Rune Knight's Millennium Shields left, 0 when the last one broke
    MillenniumShieldChanged {
        gid: u32,
        count: u8,
    },
    PvpRankingChanged {
        account_id: u32,
        ranking: i32,
        total: i32,
    },
    BladeStop {
        src_gid: u32,
        dest_gid: u32,
        active: bool,
    },
    /// Who a Crusader, Royal Guard or mercenary guards with Devotion (0x01cf, `clif_devotion`): every link it has, at
    /// most five, as kRO's client draws them
    DevotionLinks {
        gid: u32,
        targets: Vec<u32>,
    },
    /// The player's elemental and its gauges (0x081d, `clif_elemental_info`)
    ElementalInit {
        gid: u32,
        hp: u32,
        max_hp: u32,
        sp: u32,
        max_sp: u32,
    },
    /// One of the elemental's gauges (0x081e, `clif_elemental_updatestatus`)
    ElementalParChanged {
        par: ElementalPar,
        value: u32,
    },
    /// A summon of the player's and its HP (0x0b6b, `clif_summon_init`: an ABR or Bionic coming into view)
    SummonHpInit {
        gid: u32,
        hp: u32,
        max_hp: u32,
    },
    /// One of a summon's gauges (0x0b6c, `clif_summon_hp_bar` on damage and heal)
    SummonHpChanged {
        gid: u32,
        par: SummonPar,
        value: u32,
    },
    /// A unit the player's Crimson Marker marks, at its cell; -1, -1 when the mark ends early (0x09c1,
    /// `clif_crimson_marker`)
    CrimsonMarker {
        aid: u32,
        x: i16,
        y: i16,
    },
    /// How many Shadow Scars a unit carries (0x0b68, `clif_enchantingshadow_spirit`: sent as one is added)
    ShadowScars {
        gid: u32,
        amount: u16,
    },
    /// How long a unit's combo pose holds (kRO's 0x01d2 handler at 0x7ba170)
    ComboDelay {
        gid: u32,
        delay_ms: u32,
    },
    StatusEffectChanged {
        gid: u32,
        efst: i16,
        active: bool,
        remain_ms: u32,
        val1: i32,
    },
    EntityResurrected {
        gid: u32,
    },
    MvpReward {
        gid: u32,
    },
    MvpFeedback {
        kind: MvpFeedbackKind,
    },
    FamePointsGained {
        kind: FameKind,
        point: i32,
        total: i32,
    },
    PvpPointsReceived {
        win: i32,
        lose: i32,
        point: i32,
    },
    ParameterChanged {
        var_id: u16,
        value: i64,
    },
    Recovery {
        var_id: u16,
        amount: i32,
    },
    ExpGained {
        aid: u32,
        amount: i64,
        is_base: bool,
        is_quest: bool,
    },
    StatusChanged {
        status_type: u32,
        base: i32,
        bonus: i32,
    },
    AttackRangeChanged {
        range: i16,
    },
    /// Attack refused as out of range; carries where both actors actually stand
    /// and the range the refusal was measured against.
    AttackFailedForDistance {
        target_gid: u32,
        target_x: u16,
        target_y: u16,
        x: u16,
        y: u16,
        range: i16,
    },
    EntitySpriteChanged {
        gid: u32,
        sprite_type: u8,
        value: u32,
        value2: u32,
    },
    SkillCasting {
        gid: u32,
        target_gid: u32,
        skill: SkillEnum,
        property: u32,
        delay_ms: u32,
        x: i16,
        y: i16,
    },
    SkillCastCancel {
        gid: u32,
    },
    SkillFailed {
        skill: Option<SkillEnum>,
        cause: u8,
        /// What the cause refers to: the level, the amount, the skill needed...
        btype: u32,
        /// The missing item, for `USESKILL_FAIL_NEED_ITEM`
        item_id: u32,
    },
    ActionFailure {
        error_code: i16,
    },
    SkillPostDelay {
        skill: SkillEnum,
        delay_ms: u32,
    },
    AfterCastDelay {
        delay_ms: u32,
    },
    SkillDamage {
        skill: SkillEnum,
        src_gid: u32,
        target_gid: u32,
        damage: i32,
        attack_mt: i32,
        attacked_mt: i32,
        count: i16,
        level: i16,
        action: ActionType,
        start_time: u32,
    },
    SkillNoDamage {
        skill: SkillEnum,
        src_gid: u32,
        target_gid: u32,
        /// The packet's skill-level field. For AL_HEAL this carries the heal
        /// amount, which selects the green-heal size.
        level: i32,
    },
    GroundSkill {
        skill: SkillEnum,
        src_gid: u32,
        level: i16,
        x: i16,
        y: i16,
    },
    /// Sense (WZ_ESTIMATION) result. `info.name` is left empty by the network
    /// layer and resolved from the world entities on the client side.
    MonsterInfoReceived {
        info: MonsterInfo,
    },
    SkillUnitEntered {
        aid: u32,
        creator_aid: u32,
        x: i16,
        y: i16,
        unit_id: u16,
        is_visible: bool,
    },
    SkillUnitDisappeared {
        aid: u32,
    },
    GraffitiEntered {
        aid: u32,
        creator_aid: u32,
        x: i16,
        y: i16,
        message: String,
    },
    MapCellChanged {
        x: i16,
        y: i16,
        cell_type: i32,
    },
    SkillUnitUpdated {
        aid: u32,
    },
    EntityEmotion {
        gid: u32,
        emotion_type: u8,
    },
    ChatMessage {
        gid: u32,
        message: String,
    },
    OwnChatMessage {
        message: String,
    },
    RankingReceived {
        ranking: Ranking,
    },
    BroadcastMessage {
        message: String,
        color: [f32; 4],
        banner: BannerKind,
    },
    StarSkillNotice {
        map_name: String,
        monster_id: i32,
        star: u8,
        result: u8,
    },
    StarPlaceRequest {
        which: i8,
    },
    RequestAgreeStarPlace {
        which: i8,
    },
    RequestSendChat {
        message: String,
    },
    RequestEmotion {
        emote_type: u8,
    },
    NpcDialogText {
        npc_id: u32,
        text: String,
    },
    /// 0x08d6: the script's `clear`.
    NpcDialogClear {
        npc_id: u32,
    },
    NpcDialogNext {
        npc_id: u32,
    },
    NpcDialogClose {
        npc_id: u32,
    },
    NpcDialogMenu {
        npc_id: u32,
        items: Vec<String>,
    },
    NpcInputNumber {
        npc_id: u32,
    },
    NpcInputString {
        npc_id: u32,
    },
    NpcDealTypeSelect {
        npc_id: u32,
    },
    RequestNpcContact {
        npc_id: u32,
    },
    RequestNpcNext {
        npc_id: u32,
    },
    RequestNpcClose {
        npc_id: u32,
    },
    RequestNpcMenuSelect {
        npc_id: u32,
        choice: u8,
    },
    WarpList {
        skill: SkillEnum,
        destinations: Vec<String>,
    },
    RequestSelectWarppoint {
        skill: SkillEnum,
        map_name: String,
    },
    RequestNpcInputNumber {
        npc_id: u32,
        value: i32,
    },
    RequestNpcInputString {
        npc_id: u32,
        text: String,
    },
    RequestNpcDealType {
        npc_id: u32,
        deal_type: u8,
    },
    NpcShopBuyList {
        npc_id: u32,
        items: Vec<(u32, i32, i32, u8)>,
    },
    NpcShopSellList {
        npc_id: u32,
        items: Vec<(i16, i32, i32)>,
    },
    NpcShopBuyResult {
        result: u8,
    },
    /// A market shop opened (0x0b7a): a buy shop whose items have a stock
    NpcMarketList {
        items: Vec<MarketItem>,
    },
    /// A market purchase's answer (0x0b4e): the items and amounts bought; `success` false when nothing was
    NpcMarketBought {
        success: bool,
        items: Vec<(u32, u16)>,
    },
    NpcShopSellResult {
        result: u8,
    },
    RequestNpcShopBuy {
        items: Vec<(i16, u32)>,
    },
    /// The stylist opened (0x0ae2, UI type 1)
    StylistOpened,
    /// The stylist's answer (0x0a47): a refused change keeps the window open
    StylistResult {
        success: bool,
    },
    /// Changes the looks picked (0x0afc)
    RequestStylistBuy {
        choice: crate::stylist::StyleChoice,
    },
    RequestStylistClose,
    /// Buys `(count, item id)` from the open market shop (0x09d6)
    RequestNpcMarketBuy {
        items: Vec<(i16, u32)>,
    },
    /// A barter shop opened (0x0b78, `extended` 0x0b79): items paid for with other items (and zeny when extended)
    NpcBarterList {
        extended: bool,
        items: Vec<BarterItem>,
    },
    /// Trades for items at the open barter shop (0x0b0f, `extended` 0x0b57); the answer is the shop's 0x00ca
    RequestNpcBarterBuy {
        extended: bool,
        items: Vec<BarterPurchase>,
    },
    RequestNpcShopSell {
        items: Vec<(i16, i16)>,
    },
    RequestNpcShopClose,
    ChatRoomUpsert {
        owner_aid: u32,
        room_id: u32,
        max_count: i16,
        cur_count: i16,
        atype: u8,
        title: String,
    },
    ChatRoomDestroy {
        room_id: u32,
    },
    ChatRoomEntered {
        room_id: u32,
        members: Vec<crate::chat_room::ChatRoomMember>,
    },
    ChatRoomJoinRefused {
        result: u8,
    },
    ChatRoomCreateResult {
        flag: u8,
    },
    ChatRoomMemberJoined {
        name: String,
        cur_count: i16,
    },
    ChatRoomMemberLeft {
        name: String,
        cur_count: i16,
        kicked: bool,
    },
    ChatRoomOwnerChanged {
        name: String,
    },
    RequestJoinChatRoom {
        room_id: u32,
    },
    ToggleChatRoomCreate,
    RequestCreateChatRoom {
        title: String,
        limit: i16,
        public: bool,
        password: String,
    },
    RequestChangeChatRoom {
        title: String,
        limit: i16,
        public: bool,
        password: String,
    },
    RequestLeaveChatRoom,
    RequestEditChatRoomSettings,
    RequestKickChatMember {
        name: String,
    },
    RequestChangeChatOwner {
        name: String,
    },
    RequestOpenChatMemberMenu {
        name: String,
        x: f32,
        y: f32,
    },
    InventoryNormalItems {
        items: Vec<NormalItemData>,
    },
    InventoryEquipmentItems {
        items: Vec<EquipmentItemData>,
    },
    InventoryItemPickup {
        index: u16,
        item_id: u32,
        count: u16,
        item_type: u8,
        is_identified: bool,
        is_damaged: bool,
        refining_level: u8,
        grade: u8,
        options: [crate::item::ItemOption; 5],
        slot: [u32; 4],
        location: u32,
        result: u8,
    },
    InventoryUseItemResult {
        index: u16,
        count: i16,
        success: bool,
    },
    InventoryEquipResult {
        index: u16,
        wear_location: u32,
        view_id: u16,
        success: bool,
    },
    InventoryArrowEquipped {
        index: u16,
    },
    InventoryUnequipResult {
        index: u16,
        wear_location: u32,
        success: bool,
    },
    /// 0x0b18: the inventory's slots, its expansion included.
    InventorySlots {
        slots: u16,
    },
    /// 0x0ade: the weight percent from which a character is overweight.
    OverweightPercent {
        percent: i32,
    },
    InventoryItemRemoved {
        index: u16,
        count: i16,
    },
    RequestUseItem {
        index: u16,
    },
    RequestEquipItem {
        index: u16,
        location: u32,
    },
    RequestUnequipItem {
        index: u16,
    },
    RequestDropItem {
        index: u16,
        count: i16,
    },
    CartNormalItems {
        items: Vec<NormalItemData>,
    },
    CartEquipmentItems {
        items: Vec<EquipmentItemData>,
    },
    CartItemAdded {
        index: u16,
        item_id: u32,
        count: i16,
        item_type: u8,
        is_identified: bool,
        is_damaged: bool,
        refining_level: u8,
        slot: [u32; 4],
    },
    CartItemRemoved {
        index: u16,
        count: i16,
    },
    CartCountInfo {
        cur_weight: i32,
        max_weight: i32,
        cur_count: i16,
        max_count: i16,
    },
    CartOff,
    StorageNormalItems {
        items: Vec<NormalItemData>,
    },
    StorageEquipItems {
        items: Vec<EquipmentItemData>,
    },
    StorageOpened {
        cur: i16,
        max: i16,
    },
    StorageItemAdded {
        index: u16,
        item_id: u32,
        count: i16,
        item_type: u8,
        is_identified: bool,
        is_damaged: bool,
        refining_level: u8,
        slot: [u32; 4],
    },
    StorageItemRemoved {
        index: u16,
        amount: i16,
    },
    StorageClosed,
    /// 0x23a: the storage the player just opened is password gated.
    StoragePasswordRequest {
        prompt: StoragePasswordPrompt,
    },
    /// 0x23c: how the check or the change went, plus the wrong-attempt count.
    StoragePasswordResult {
        outcome: StoragePasswordOutcome,
        error_count: i16,
    },
    /// The player accepted setting a first password: open the dialog in set mode.
    OpenStoragePasswordPrompt {
        prompt: StoragePasswordPrompt,
    },
    /// Dialog → 0x23b.
    RequestStoragePassword {
        change: bool,
        password: String,
        new_password: String,
    },

    ExchangeRequested {
        name: String,
        gid: u32,
        level: i16,
    },
    ExchangeAckResult {
        result: u8,
        level: i16,
    },
    ExchangeItemAdded {
        item_id: u32,
        item_type: u8,
        count: i32,
        is_identified: bool,
        is_damaged: bool,
        refining_level: u8,
        slot: [u32; 4],
    },
    ExchangeAddResult {
        index: u16,
        result: u8,
    },
    ExchangeConcluded {
        who: u8,
    },
    ExchangeCanceled,
    ExchangeCompleted {
        result: u8,
    },
    ExchangeUndo,
    RequestExchangeItem {
        target_aid: u32,
    },
    RespondExchangeRequest {
        accept: bool,
    },
    RequestAddExchangeItem {
        index: u16,
        count: i32,
    },
    RequestConcludeExchange,
    RequestCancelExchange,
    RequestExecExchange,

    MailWindow {
        open: bool,
    },
    MailInboxReceived {
        entries: Vec<MailEntry>,
    },
    MailOpened {
        mail: OpenedMail,
    },
    MailDeleteAck {
        mail_id: u32,
        ok: bool,
    },
    MailGetItemAck {
        result: u8,
    },
    MailAddItemAck {
        index: u16,
        ok: bool,
    },
    MailSendAck {
        ok: bool,
    },
    MailNewReceived {
        mail_id: u32,
        title: String,
        sender: String,
    },
    MailReturnAck {
        mail_id: u32,
        ok: bool,
    },
    RequestMailList,
    RequestMailOpen {
        mail_id: u32,
    },
    RequestMailDelete {
        mail_id: u32,
    },
    RequestMailGetItem {
        mail_id: u32,
    },
    RequestMailResetItem {
        ty: u8,
    },
    RequestMailAddItem {
        index: u16,
        amount: u32,
    },
    RequestMailSend {
        to: String,
        title: String,
        body: String,
    },
    RequestMailReturn {
        mail_id: u32,
        sender: String,
    },

    RequestMoveItemBodyToStore {
        index: u16,
        count: i16,
    },
    RequestDepositItem {
        index: u16,
    },
    RequestWithdrawItem {
        index: u16,
    },
    RequestMoveItemStoreToBody {
        index: u16,
        count: i16,
    },
    RequestCloseStorage,
    RequestMoveItemBodyToCart {
        index: u16,
        count: i16,
    },
    RequestMoveItemCartToBody {
        index: u16,
        count: i16,
    },
    RequestMoveItemStoreToCart {
        index: u16,
        count: i16,
    },
    RequestMoveItemCartToStore {
        index: u16,
        count: i16,
    },
    RequestCartOff,
    RequestSetCartPick {
        pick_equip: bool,
        pick_usable: bool,
        pick_etc: bool,
    },
    RequestChangeCart {
        num: i16,
    },
    /// 0x097f: the carts Cart Decoration offers (`clif_SelectCart`: 10 to 12), with the account id the answer repeats
    CartDecorationList {
        identity: u32,
        carts: Vec<u8>,
    },
    /// 0x08e2: an NPC's navigation guide (`navigateto`)
    NavigationGuide {
        navigation: crate::navigation::Navigation,
    },
    /// 0x0980: the cart picked from that list
    RequestSelectCart {
        identity: u32,
        cart: u8,
    },
    ToggleCart,
    RequestCardInsertList {
        card_index: u16,
    },
    RequestCardInsert {
        card_index: u16,
        equip_index: u16,
    },
    CardInsertItemList {
        card_index: u16,
        equip_indices: Vec<u16>,
    },
    CardInsertResult {
        equip_index: u16,
        card_index: u16,
        result: u8,
    },
    FloorItemAppeared {
        id: u32,
        item_id: u32,
        is_identified: bool,
        x: i16,
        y: i16,
        sub_x: u8,
        sub_y: u8,
        count: i16,
        is_falling: bool,
    },
    FloorItemDisappeared {
        id: u32,
    },
    RequestPickupItem {
        id: u32,
    },
    SkillListReceived {
        skills: Vec<SkillInfo>,
    },
    SkillUpdated {
        skill: SkillEnum,
        level: i16,
        sp_cost: i16,
        attack_range: i16,
        upgradable: bool,
        /// 0x07e1's: the server makes a combo's next skill self-targeted while
        /// its window is open, and sets it back after (Asura Strike, Flying Kick)
        target_type: Option<SkillTargetType>,
    },
    SkillAdded {
        skill: SkillInfo,
    },
    /// 0x0441: a skill leaves the tree (a quest skill forgotten, an item's skill taken off)
    SkillRemoved {
        skill: SkillEnum,
    },
    RequestSkillLevelUp {
        skill: SkillEnum,
    },
    RequestStatChange {
        status_id: u16,
        amount: u8,
    },
    /// Raise a trait stat (POW to CRT) by `amount`
    RequestTraitStatChange {
        status_id: u16,
        amount: u16,
    },
    ToggleStatusWindow,
    /// A window a hotkey toggles, opened from somewhere else (the basic info window's menu)
    RunHotkeyAction {
        action: crate::keybinding::HotkeyAction,
    },
    /// A window the frameless menu opens (a `Toggle*` event or `RunHotkeyAction`): sent on only while that
    /// window is closed, so the menu never closes one it promised to show
    OpenWindow(Box<GameEvent>),
    HotkeyListReceived {
        slots: Vec<(i8, u32, i16)>,
    },
    RequestHotkeyChange {
        index: u16,
        is_skill: bool,
        id: u32,
        count: i16,
    },
    RequestUseSkill {
        skill: SkillEnum,
        level: i16,
    },
    RequestCompanionUseSkill {
        is_mercenary: bool,
        skill: SkillEnum,
        level: i16,
    },
    /// A skill granted by a consumable, to be armed exactly like a click in the
    /// skill window. The cast metadata rides along because the player has not
    /// learned the skill and so it is absent from the skill list.
    AutoCastSkill {
        skill: SkillEnum,
        level: i16,
        sp_cost: i16,
        attack_range: i16,
        skill_target_type: SkillTargetType,
    },
    ShowItemInfo {
        index: u16,
    },
    ShowItemInfoDirect {
        item: Box<Item>,
    },
    ShowCardInfo {
        item_id: u32,
    },
    ShowCardIllustration {
        item_id: u32,
    },
    ReadBook {
        item_id: u32,
    },
    RequestRemoveOption,
    /// Take off everything worn in `location` at once (0x0bf5)
    RequestUnequipAll {
        location: u32,
    },
    /// Wear one of the character's titles, 0 for none (0x0a2e)
    RequestChangeTitle {
        title_id: i32,
    },
    /// Put an inventory item into the swap equipment set for `location` (0x0a97)
    RequestEquipSwitchAdd {
        index: u16,
        location: u32,
    },
    /// Take an item out of the swap equipment set (0x0a99)
    RequestEquipSwitchRemove {
        index: u16,
    },
    /// CHANGE: wear the whole set in place of the worn equipment (0x0a9c)
    RequestEquipSwitch,
    /// Swap one item: a set item for the worn one of its place, or a worn one into the set (0x0ace)
    RequestEquipSwitchSingle {
        index: u16,
    },
    /// The Damage Indicator tab's Apply: draw damage numbers with this skin and throw them this way, and keep both
    /// (kRO's client sends nothing: 0x8c0291)
    ApplyDamageIndicator {
        skin: u32,
        direction: crate::damage_number::DamageDirection,
    },
    ShowSystemMessage {
        message: String,
    },
    WhisperSettingResult {
        allow: bool,
        result: u8,
        all: bool,
    },
    MemoResult {
        result: u8,
    },
    ProgressBarStarted {
        duration_secs: u32,
    },
    ProgressBarCancelled,
    ServerMsg {
        msg_id: u16,
    },
    /// 0x09cd: a msgstringtable line in the server's colour (0x00BBGGRR)
    ServerMsgColor {
        msg_id: u16,
        color: u32,
    },
    /// 0x07e2: a msgstringtable line with its `%d` filled in
    ServerMsgValue {
        msg_id: u16,
        value: i32,
    },
    /// 0x07e6: a msgstringtable line after a skill's name; the id is raw, as any skill can send one
    ServerSkillMsg {
        skill_id: u16,
        msg_id: u16,
    },
    MapInfoNotice {
        atype: i16,
    },
    ServerColoredMessage {
        account_id: u32,
        color: u32,
        message: String,
    },
    /// Answer to `/who`, sent to the requester alone.
    UserCount {
        count: i32,
    },
    /// 0x014a: outcome of a manner-point adjustment, or the notice that our own
    /// chat got blocked.
    MannerPointResult {
        result: u32,
    },
    /// 0x014b: who adjusted our manner points, and in which direction.
    MannerPointGiven {
        positive: bool,
        other_name: String,
    },
    /// 0x0214: the `/check` stat block of another character.
    GmStatusReceived {
        status: Box<crate::gm::GmStatus>,
    },
    /// 0x01e0: account name behind an account id.
    AccountNameReceived {
        aid: u32,
        name: String,
    },
    SkillMsg {
        msg_no: i32,
    },
    /// The equipment at this inventory index becomes account-bound when worn.
    BindOnEquipNotice {
        index: u16,
    },
    /// A Talkie Box was stepped on: its text belongs to the skill unit `aid`.
    TalkboxContents {
        aid: u32,
        message: String,
    },
    ShowDigit {
        mode: ShowDigitMode,
        value: i32,
    },
    BossInfoReceived {
        kind: BossInfoKind,
        x: u16,
        y: u16,
        respawn_hour: u16,
        respawn_minute: u16,
        name: String,
    },
    DialogClosed,
    ToggleInventory,
    ToggleEquipment,
    /// The Equipment Attributes window, from the equipment window's footer
    ToggleEquipmentProperties,
    ToggleSkills,
    ToggleEmotionWindow,
    ToggleShortcutList,
    ShortcutBindingsChanged(Vec<String>),
    ToggleMinimap,
    ToggleSoundOptions,
    SoundSettingsChanged {
        settings: crate::display::SoundSettings,
        persist: bool,
    },
    ToggleGraphicOptions,
    ToggleSystemMenu,
    ToggleHotkeyConfig,
    /// The menu's Keys page: every binding, after one changed
    KeyBindingsChanged(Box<crate::keybinding::KeyBindings>),
    /// Graphic Options' "Frameless HUD" box (`docs/nostalre-hud.md`)
    HudLayoutChanged {
        layout: crate::display::HudLayout,
    },
    GraphicsSettingsChanged {
        settings: crate::display::GraphicsSettings,
        persist: bool,
    },
    SoundEffect {
        name: String,
        act: u8,
        term_ms: u32,
        gid: u32,
    },
    PartyCreateResult {
        result: u8,
    },
    PartyMemberList {
        name: String,
        members: Vec<PartyMemberData>,
    },
    PartyMemberAdded {
        aid: u32,
        name: String,
        map: String,
        leader: bool,
        online: bool,
        x: u16,
        y: u16,
        class: u16,
        base_level: u16,
    },
    /// 0x0ab2: a member died or was revived.
    PartyMemberDead {
        aid: u32,
        dead: bool,
    },
    /// 0x0abd: a member's job or base level changed.
    PartyMemberJob {
        aid: u32,
        class: u16,
        base_level: u16,
    },
    PartyMemberRemoved {
        aid: u32,
        name: String,
        result: u8,
    },
    PartyMemberHp {
        aid: u32,
        hp: u32,
        max_hp: u32,
    },
    /// Coordinates are signed: the server sends -1,-1 when the member leaves
    /// our map, which clears their marker.
    PartyMemberPosition {
        aid: u32,
        x: i16,
        y: i16,
    },
    PartyInviteReceived {
        party_grid: u32,
        party_name: String,
    },
    PartyInviteResult {
        name: String,
        answer: u8,
    },
    PartyConfigChanged {
        exp_option: u32,
        item_pickup_rule: u8,
        item_division_rule: u8,
    },
    SelfConfigChanged {
        kind: SelfConfigKind,
        enabled: bool,
    },
    /// A script's placement of the NPC dialog (`setdialogsize`, `setdialogpos`, `setdialogpospercent`, from 20220504)
    NpcDialogPlacement(DialogPlacement),
    /// A map's special popup (rAthena's `specialpopup` mapflag, from 20221005): the id of `spopup.lub`'s entry
    SpecialPopup {
        id: i32,
    },
    /// Someone got a rare item, announced to every player (0x07fd)
    SpecialItemObtained {
        player: String,
        item_id: u32,
        source: SpecialItemSource,
        /// The item's refine (from 20220518; rAthena sends 0 so far)
        refine: u32,
    },
    /// An NPC's music (a script's `playBGM`): kRO plays `bgm\<name>.mp3` in place of the map's
    NpcMusic {
        name: String,
    },
    /// The answer to taking everything off at once (0x0bae); the items come off one by one as their own unequips
    UnequipAllAnswered {
        refused: bool,
    },
    RequestSetConfig {
        kind: SelfConfigKind,
        enabled: bool,
    },
    /// A party member on our map picked up an item (shown when the party shares items).
    PartyItemPickup {
        aid: u32,
        item_id: u32,
        identified: bool,
        refine: u8,
    },
    /// As the sender typed it, "Name : text".
    PartyChatMessage {
        message: String,
    },
    GuildChatMessage {
        message: String,
    },
    WhisperReceived {
        sender: String,
        message: String,
    },
    WhisperAck {
        result: u8,
    },
    RequestSendWhisper {
        name: String,
        message: String,
    },
    TogglePartyWindow,
    RequestPartyInvite {
        target_aid: u32,
    },
    RespondPartyInvite {
        party_grid: u32,
        accept: bool,
    },
    RequestLeaveParty,
    RequestExpelMember {
        aid: u32,
        name: String,
    },
    RequestPartyExpOption {
        exp_share: bool,
    },
    SendPartyChat {
        message: String,
    },
    RequestPartyCreate {
        name: String,
        item_pickup_rule: u8,
        item_division_rule: u8,
    },
    RequestChangePartyLeader {
        aid: u32,
    },
    RequestPartyInviteByName {
        name: String,
    },
    /// Open the party helper child window. mode: 0=create, 1=invite, 2=setup.
    ShowPartyHelper {
        mode: u8,
    },

    // --- Friends ---
    FriendListReceived {
        friends: Vec<FriendData>,
    },
    FriendStateChanged {
        aid: u32,
        gid: u32,
        online: bool,
    },
    FriendAddResult {
        result: u8,
        aid: u32,
        gid: u32,
        name: String,
    },
    FriendRemoved {
        aid: u32,
        gid: u32,
    },
    FriendRequestReceived {
        req_aid: u32,
        req_gid: u32,
        name: String,
    },
    ToggleFriendWindow,
    RequestAddFriend {
        name: String,
    },
    RespondFriendRequest {
        req_aid: u32,
        req_gid: u32,
        accept: bool,
    },
    RequestDeleteFriend {
        aid: u32,
        gid: u32,
    },
    RequestWhisper {
        name: String,
    },

    // --- Guild ---
    GuildMenuFlag {
        flag: i32,
    },
    GuildInfo {
        gdid: u32,
        name: String,
        level: i32,
        exp: i32,
        max_exp: i32,
        member_num: i32,
        max_member_num: i32,
        avg_level: i32,
        point: i32,
        honor: i32,
        virtue: i32,
        master_name: String,
        manage_land: String,
        emblem_version: i32,
    },
    GuildMembers {
        members: Vec<GuildMember>,
    },
    GuildPositions {
        positions: Vec<GuildPosition>,
    },
    GuildPositionNames {
        names: Vec<(i32, String)>,
    },
    GuildMemberPositionsChanged {
        entries: Vec<(u32, u32, i32)>,
    },
    /// Signed for the same reason as `PartyMemberPosition`.
    GuildMemberPosition {
        aid: u32,
        x: i16,
        y: i16,
    },
    GuildMemberOnline {
        aid: u32,
        gid: u32,
        online: bool,
        appearance: Option<GuildMemberAppearance>,
    },
    GuildSkills {
        point: i16,
        skills: Vec<GuildSkill>,
    },
    GuildBanList {
        entries: Vec<GuildBanEntry>,
    },
    GuildNotice {
        subject: String,
        body: String,
    },
    GuildRelations {
        relations: Vec<GuildRelation>,
    },
    GuildEmblem {
        gdid: u32,
        version: i32,
        bmp: Vec<u8>,
        /// 0x0152 holds the current emblem when it has one (a web upload empties it), the web server may hold an
        /// older one: the map server's replaces the web server's
        from_map_server: bool,
    },
    /// The web server stored our emblem as `version` (Some), or refused it (None: `bmp` goes to the map server).
    GuildEmblemUploaded {
        gdid: u32,
        version: Option<u32>,
        bmp: Vec<u8>,
    },
    GuildIdentityUpdated {
        gdid: u32,
        emblem_version: i32,
        right: i32,
        is_master: bool,
        name: String,
    },
    GuildCreateResult {
        result: u8,
    },
    /// A member left: their character id (rAthena sends no name from 20161019).
    GuildMemberLeft {
        gid: u32,
        reason: String,
    },
    GuildMemberExpelled {
        gid: u32,
        reason: String,
    },
    /// A unit's guild position title (0x0afd, sent to the area when it changes).
    EntityGuildPositionChanged {
        aid: u32,
        position: String,
    },
    EntityGuildChanged {
        aid: u32,
        gdid: u32,
        emblem_version: i32,
    },
    GuildDisbandResult {
        reason: i32,
    },
    GuildInviteReceived {
        gdid: u32,
        name: String,
    },
    RespondGuildInvite {
        gdid: u32,
        accept: bool,
    },
    GuildAllyRequestReceived {
        aid: u32,
        name: String,
    },
    RespondGuildAlly {
        aid: u32,
        accept: bool,
    },
    GuildAllyResult {
        answer: u8,
    },
    GuildHostileResult {
        result: u8,
    },
    GuildJoinResult {
        answer: u8,
    },
    GuildRelationDeleted {
        gdid: u32,
        relation: i32,
    },
    /// UI → client: window opened, fetch all guild tab data.
    RequestGuildInfoBurst,
    RequestGuildMenu {
        atype: i32,
    },
    /// UI → client: right-clicked a guild member row; open the player menu here.
    ShowGuildMemberMenu {
        aid: u32,
        gid: u32,
        name: String,
        x: f32,
        y: f32,
    },
    RequestSetGuildNotice {
        subject: String,
        body: String,
    },
    RequestGuildLeave,
    RequestGuildExpel {
        aid: u32,
        gid: u32,
        name: String,
    },
    RequestChangeMemberPosition {
        aid: u32,
        gid: u32,
        position_id: i32,
    },
    RequestChangePositionInfo {
        positions: Vec<GuildPosition>,
    },
    RequestUpgradeGuildSkill {
        skill: SkillEnum,
    },
    RequestGuildInvite {
        target_aid: u32,
    },
    RequestGuildAlly {
        target_aid: u32,
    },
    RequestGuildHostile {
        target_aid: u32,
    },
    RequestDeleteGuildRelation {
        gdid: u32,
        relation: i32,
    },
    ConfirmedSkillTalkbox {
        skill: SkillEnum,
        level: i16,
        x: i16,
        y: i16,
        message: String,
    },
    ConfirmedGuildExpel {
        aid: u32,
        gid: u32,
        name: String,
        reason: String,
    },
    ConfirmedGuildLeave,
    ConfirmedDeleteGuildRelation {
        gdid: u32,
        relation: i32,
    },
    /// UI → client: open the emblem picker for the configured folder.
    RequestSelectEmblem,
    /// UI → client: upload the chosen emblem BMP file.
    RequestUploadEmblem {
        path: String,
    },

    /// A mark the server put on the minimap (`ZC_COMPASS`): the town guide's
    /// directions and quest arrows.
    MinimapMark {
        id: u8,
        action: MarkAction,
        x: u16,
        y: u16,
        color: u32,
    },

    /// UI → client: load a world-map texture from the GRF on demand. The client
    /// answers with `WorldMapWindow::texture_loaded`.
    RequestWorldMapTexture {
        path: String,
    },

    // --- Skill-triggered production / selection windows ---
    ItemIdentifyList {
        indices: Vec<u16>,
    },
    ItemIdentifyResult {
        index: i16,
        ok: bool,
    },
    RequestIdentifyItem {
        index: i16,
    },
    MakingArrowList {
        item_ids: Vec<u32>,
    },
    RequestMakingArrow {
        item_id: u32,
    },
    MakableItemList {
        item_ids: Vec<u32>,
    },
    MakingItemResult {
        result: i16,
    },
    RequestMakingItem {
        item_id: u32,
        materials: [u32; 3],
    },
    /// 0x025a: what Mix Cooking, Create Bomb, the pharmacies and Manufacture Machine can make; `mk_type` is the
    /// list's kind, sent back with the pick
    CookingList {
        mk_type: u16,
        item_ids: Vec<u32>,
    },
    /// 0x025b: the pick from a 0x025a list, item 0 to close it (the server keeps the menu until then)
    RequestCooking {
        mk_type: u16,
        item_id: u32,
    },
    WeaponRefineList {
        items: Vec<RefineItemRow>,
    },
    WeaponRefineResult {
        result: i32,
        item_id: u32,
    },
    ItemRefiningResult {
        index: u16,
        refine: u8,
        result: i16,
    },
    /// An NPC opened the refine window (0x0aa0)
    RefineWindowOpened,
    /// What refining the item at `index` costs (0x0aa2): the materials to choose from, and how many Blacksmith
    /// Blessings guard it (0: none can at this level)
    RefineMaterials {
        index: u16,
        blessings: u8,
        materials: Vec<RefineMaterial>,
    },
    /// Someone's refine announced to all (0x0ada)
    RefineAnnounced {
        name: String,
        item_id: u32,
        refine: u8,
        success: bool,
    },
    RequestRefineSelect {
        index: u16,
    },
    RequestRefine {
        index: u16,
        material: u32,
        blessing: bool,
    },
    RequestRefineClose,
    RequestWeaponRefine {
        index: i32,
    },
    RepairItemList {
        items: Vec<RefineItemRow>,
    },
    RepairItemResult {
        index: i16,
        ok: bool,
    },
    RequestRepairItem {
        index: i16,
        item_id: u32,
        refine: u8,
        cards: [u32; 4],
    },
    AutoSpellList {
        skills: Vec<SkillEnum>,
    },
    RequestSelectAutoSpell {
        skill: Option<SkillEnum>,
    },
    /// 0x0442: the skills a menu skill offers (`why` 1, Auto Shadow Spell: the copied ones)
    SkillSelectList {
        why: i32,
        skills: Vec<SkillEnum>,
    },
    /// 0x0443: the pick, or `None` to close the menu
    RequestSelectSkill {
        why: i32,
        skill: Option<SkillEnum>,
    },
    /// 0x07e3: the item list window of Change Material (kind 0) and Four Spirit Analysis (its level, 1 or 2)
    ItemListWindowOpened {
        kind: i32,
    },
    /// 0x07e4: the items and amounts chosen in it (inventory index, amount), or its Cancel (`confirmed` false)
    RequestItemListWindow {
        kind: i32,
        confirmed: bool,
        items: Vec<(u16, u16)>,
    },

    // --- Vending ---
    OpenVendingSetup {
        max_items: i16,
    },
    RequestOpenStore {
        shop_name: String,
        items: Vec<(i16, i16, i32)>,
    },
    RequestCancelVendingSetup,
    RequestCloseStore,
    VendingOwnStock {
        items: Vec<VendorItem>,
    },
    VendingBoardShown {
        aid: u32,
        name: String,
    },
    VendingBoardHidden {
        aid: u32,
    },
    RequestBuyFromVendor {
        aid: u32,
    },
    VendingShopList {
        aid: u32,
        unique_id: u32,
        items: Vec<VendorItem>,
    },
    RequestPurchaseFromVendor {
        aid: u32,
        unique_id: u32,
        items: Vec<(i16, i16)>,
    },
    VendingPurchaseResult {
        index: i16,
        curcount: i16,
        result: u8,
    },
    VendingStockDecrement {
        index: i16,
        count: i16,
    },
    VendingOpenResult {
        result: u8,
    },

    // --- Homunculus / Mercenary ---
    HomunPropertyReceived {
        property: HomunculusProperty,
    },
    /// 0x0230: state 0 = pre-init (carries the companion GID), 1 = intimacy, 2 = hunger.
    CompanionStateChanged {
        state: i8,
        gid: u32,
        data: i32,
    },
    HomunFeedResult {
        success: bool,
        item_id: u32,
    },
    MercenaryInfoReceived {
        info: MercenaryInfo,
        is_init: bool,
    },
    MercenaryParamChanged {
        var: u16,
        value: i32,
    },
    /// 0x0ba5: 64-bit (experience).
    HomunParamChanged {
        var: u16,
        value: i64,
    },
    HomunSkillList {
        skills: Vec<SkillInfo>,
    },
    HomunSkillUpdate {
        skill: SkillEnum,
        level: i16,
        sp_cost: i16,
        attack_range: i16,
        upgradable: bool,
    },
    MercenarySkillList {
        skills: Vec<SkillInfo>,
    },
    RequestCompanionMove {
        gid: u32,
        x: i32,
        y: i32,
    },
    RequestCompanionAttack {
        gid: u32,
        target_gid: u32,
    },
    RequestCompanionMoveToOwner {
        gid: u32,
    },
    /// 0 = info, 1 = feed, 2 = delete (permanent).
    RequestHomunMenu {
        command: u8,
    },
    /// User asked to delete the homunculus; needs confirmation before it fires
    /// `RequestHomunMenu { command: 2 }`.
    RequestHomunDelete,
    /// Rest button: self-cast the Rest skill to vaporize the homunculus.
    RequestHomunRest,
    /// 1 = info, 2 = dismiss.
    RequestMercenaryCommand {
        command: i8,
    },
    RequestRenameHomun {
        name: String,
    },
    ToggleHomunculusWindow,
    ToggleMercenaryWindow,
    ToggleMercenarySkillWindow,
    ToggleHomunSkillWindow,
    /// Standby button: toggle the companion between follow and hold.
    ToggleCompanionStandby {
        is_mercenary: bool,
    },
    /// Patrol button: arm the destination pick, or stop an ongoing patrol.
    ToggleCompanionPatrol {
        is_mercenary: bool,
    },
    ToggleCompanionAiConfig,
    SaveCompanionAiConfig,
    RevertCompanionAiConfig,
    ResetCompanionAiConfig,

    // --- Pet ---
    /// 0x19e: server armed pet-capture targeting.
    PetCaptureStart,
    /// 0x1a0: capture attempt resolved.
    PetCaptureResult {
        ok: bool,
    },
    /// 0x1a2: full pet info; refreshes pet data without opening the window.
    PetProperty {
        property: PetProperty,
    },
    /// 0x1a3: feed attempt resolved.
    PetFeedResult {
        ok: bool,
        food_item_id: u32,
    },
    /// 0x1a4: incremental pet state (ty: 0 init/1 intimacy/2 hunger/3 accessory/4 performance/5 marker).
    PetStateChanged {
        ty: i8,
        gid: u32,
        data: i32,
    },
    /// 0x1a6: eggs available to hatch (inventory indices).
    PetEggList {
        indices: Vec<u16>,
    },
    /// 0x1aa: pet emote / talk broadcast.
    PetAct {
        gid: u32,
        data: i32,
    },
    /// 0x19f: send capture attempt against the picked mob.
    RequestTryCapture {
        gid: u32,
    },
    /// 0x1a1: pet command (cSub 0 info / 1 feed / 2 perform / 3 return-to-egg / 4 unequip).
    RequestPetCommand {
        csub: i8,
    },
    /// 0x1a5: rename the pet (allowed once).
    RequestRenamePet {
        name: String,
    },
    /// 0x1a7: hatch the chosen egg.
    RequestSelectPetEgg {
        index: u16,
    },
    /// 0x1a9: owner-generated pet emote / talk.
    RequestPetAct {
        data: i32,
    },
    /// Feed command chosen: opens the confirm dialog before sending cSub=1.
    RequestPetFeed,
    TogglePetWindow,

    // --- Quest ---
    /// 0x09f8: every quest with its objectives; replaces the log.
    QuestListReceived {
        quests: Vec<crate::quest::Quest>,
    },
    /// 0x09f9: a quest was received (its totals follow in 0x08fe).
    QuestAdded {
        quest: crate::quest::Quest,
    },
    /// 0x2b4: a quest was removed (also means "completed").
    QuestRemoved {
        quest_id: u32,
    },
    /// 0x09fa, 0x08fe: hunt progress.
    QuestHuntUpdated {
        entries: Vec<crate::quest::QuestHuntEntry>,
    },
    /// 0x2b7: active-flag ack.
    QuestActiveChanged {
        quest_id: u32,
        active: bool,
    },
    /// 0x446: over-NPC quest marker.
    QuestNpcMarker {
        npc_id: u32,
        x: u16,
        y: u16,
        effect: i16,
        color: u8,
    },
    /// UI: right-click a row → send CZ_ACTIVE_QUEST.
    RequestToggleQuestActive {
        quest_id: u32,
        active: bool,
    },
    /// UI: View button → open the detail window for the quest.
    OpenQuestDetail {
        quest_id: u32,
    },
    ToggleQuestWindow,
    ShowQuestWindow,

    // --- Marriage ---
    /// 0x1e6: partner's name (empty = no partner), broadcast when someone finishes
    /// casting WE_CALLPARTNER.
    CoupleNameReceived {
        name: String,
    },
    /// 0x1ea: wedding celebration on an actor → confetti effect + sound.
    WeddingCelebration {
        account_id: u32,
    },
    /// 0x1e2: `name` proposes marriage; the reply carries their aid and gid back.
    MarriageProposed {
        proposer_aid: u32,
        proposer_gid: u32,
        name: String,
    },
    /// 0x1e4: the wedding NPC hands the player a target cursor — the next click on
    /// another player proposes to them.
    MarriageProposalArmed,
    /// Proposal cursor click → 0x1e5.
    RequestMarriage {
        target_aid: u32,
    },
    /// Proposal dialog → 0x1e3.
    RespondMarriageProposal {
        accept: bool,
    },
    /// 0x205: you are divorced from `name`.
    Divorced {
        name: String,
    },
    /// 0x1b3: NPC cutin illustration; `position` 0/1/2 = bottom left/middle/right,
    /// 255 = clear.
    NpcCutin {
        image: String,
        position: u8,
    },

    // --- Adoption ---
    /// 0x1f6: a married couple wishes to adopt the local player. `father_aid`/
    /// `mother_aid` are the two parents' account ids, echoed back unchanged on reply.
    AdoptionRequested {
        father_aid: u32,
        mother_aid: u32,
        name: String,
    },
    /// 0x216: adoption failure notice to the requesting parent.
    AdoptionMessage {
        msg_no: i32,
    },
    RequestAdoption {
        target_aid: u32,
    },
    RespondAdoptionRequest {
        accept: bool,
    },

    /// Picked from the context menu; a minus point asks for confirmation first.
    RequestMannerPoint {
        target_aid: u32,
        positive: bool,
    },
    GiveMannerPoint {
        target_aid: u32,
        positive: bool,
    },
    RequestAccountName {
        aid: u32,
    },

    Acknowledged,
}

/// Where a script puts the NPC dialog: rAthena checks 0 to 32767 for the size and the pixel position, 0 to 100 for
/// the percentages
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogPlacement {
    /// 0x0ba2
    Size { width: i32, height: i32 },
    /// 0x0ba3, in pixels
    Pixels { x: i32, y: i32 },
    /// 0x0bb5, in percent of the screen
    Percent { x: i32, y: i32 },
}

/// Where an announced item came from: 0x07fd's type (rAthena's `BROADCASTING_SPECIAL_ITEM_OBTAIN`)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpecialItemSource {
    /// Opened from this box item
    Box(u32),
    /// Dropped by this monster
    Monster(String),
    /// Given by an NPC
    Npc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelfConfigKind {
    RefusePartyInvite,
    OpenEquipmentWindow,
    Call,
    PetAutofeed,
    HomunculusAutofeed,
    /// rAthena's `CONFIG_DISABLE_SHOWCOSTUMES` (from 20230419): enabled hides the costumes
    HideCostumes,
}

impl SelfConfigKind {
    /// Value carried by the `config` field of CZ_CONFIG / ZC_CONFIG.
    pub fn config_id(self) -> i32 {
        match self {
            SelfConfigKind::OpenEquipmentWindow => 0,
            SelfConfigKind::Call => 1,
            SelfConfigKind::PetAutofeed => 2,
            SelfConfigKind::HomunculusAutofeed => 3,
            SelfConfigKind::HideCostumes => 5,
            SelfConfigKind::RefusePartyInvite => -1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoragePasswordPrompt {
    NotSetYet,
    Required,
    LockedOut,
}

impl StoragePasswordPrompt {
    pub fn from_info(info: i16) -> Option<Self> {
        match info {
            0 => Some(StoragePasswordPrompt::NotSetYet),
            1 => Some(StoragePasswordPrompt::Required),
            8 => Some(StoragePasswordPrompt::LockedOut),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoragePasswordOutcome {
    ChangeOk,
    ChangeFailed,
    CheckOk,
    CheckFailed,
    LockedOut,
}

impl StoragePasswordOutcome {
    pub fn from_result(result: i16) -> Option<Self> {
        match result {
            4 => Some(StoragePasswordOutcome::ChangeOk),
            5 => Some(StoragePasswordOutcome::ChangeFailed),
            6 => Some(StoragePasswordOutcome::CheckOk),
            7 => Some(StoragePasswordOutcome::CheckFailed),
            8 => Some(StoragePasswordOutcome::LockedOut),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MvpFeedbackKind {
    Item { item_id: u32 },
    ItemDropped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FameKind {
    Blacksmith,
    Alchemist,
    Taekwon,
}

impl FameKind {
    /// rAthena's `e_rank`: 0 to 2; 3 (/pk) keeps no fame.
    pub fn from_rank_type(rank_type: i16) -> Option<Self> {
        match rank_type {
            0 => Some(FameKind::Blacksmith),
            1 => Some(FameKind::Alchemist),
            2 => Some(FameKind::Taekwon),
            _ => None,
        }
    }

    /// The ranking's type in rAthena's requests and replies.
    pub fn rank_type(self) -> i16 {
        match self {
            FameKind::Blacksmith => 0,
            FameKind::Alchemist => 1,
            FameKind::Taekwon => 2,
        }
    }

    /// The name in its point line and the title of its ranking.
    fn names(self) -> (&'static str, &'static str) {
        match self {
            FameKind::Blacksmith => ("Blacksmith", "Top 10 Blacksmiths"),
            FameKind::Alchemist => ("Alchemist", "Top 10 Alchemists"),
            FameKind::Taekwon => ("TaeKwon", "Top 10 TaeKwon"),
        }
    }

    pub fn point_line(self, point: i32, total: i32) -> String {
        format!(
            "[{} Point] You gained {point} point(s), for a total of {total}.",
            self.names().0
        )
    }
}

/// A fame ranking (0x0af6): character ids and points, best first. It is shown once every name is known.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ranking {
    pub kind: FameKind,
    pub entries: Vec<(u32, i32)>,
}

impl Ranking {
    /// The chat lines, or `None` while a name is missing.
    pub fn lines(&self, names: &crate::char_name::CharNameCache) -> Option<Vec<String>> {
        let mut lines = vec![format!("== {} ==", self.kind.names().1)];
        if self.entries.is_empty() {
            lines.push("  (no entries)".to_string());
        }
        for (i, (char_id, point)) in self.entries.iter().enumerate() {
            lines.push(format!("  {}. {} - {point}", i + 1, names.get(*char_id)?));
        }
        Some(lines)
    }
}

#[derive(Debug, Clone)]
pub struct HomunculusProperty {
    pub name: String,
    pub renamed: bool,
    pub vaporized: bool,
    pub level: i16,
    pub hunger: i16,
    pub intimacy: i16,
    pub accessory: u16,
    pub atk: i16,
    pub matk: i16,
    pub hit: i16,
    pub critical: i16,
    pub def: i16,
    pub mdef: i16,
    pub flee: i16,
    pub aspd: i16,
    pub hp: u32,
    pub max_hp: u32,
    pub sp: u32,
    pub max_sp: u32,
    pub exp: i64,
    pub max_exp: i64,
    pub skill_points: i16,
    pub atk_range: i16,
}

#[derive(Debug, Clone)]
pub struct PetProperty {
    pub name: String,
    pub renamed: bool,
    pub level: i16,
    pub hunger: i16,
    pub intimacy: i16,
    pub accessory: u16,
    pub job: i16,
}

#[derive(Debug, Clone)]
pub struct MercenaryInfo {
    pub gid: u32,
    pub name: String,
    pub level: i16,
    pub atk: i16,
    pub matk: i16,
    pub hit: i16,
    pub critical: i16,
    pub def: i16,
    pub mdef: i16,
    pub flee: i16,
    pub aspd: i16,
    pub atk_range: i16,
    pub hp: u32,
    pub max_hp: u32,
    pub sp: u32,
    pub max_sp: u32,
    pub expire_date: i32,
    pub faith: i16,
    pub calls: i32,
    pub kills: i32,
}

/// One item a market shop sells: its price and how many it has left
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MarketItem {
    pub item_id: u32,
    pub item_type: u8,
    pub price: u32,
    /// `None`: no limit
    pub stock: Option<u32>,
}

/// One item a barter shop trades: its place in the shop, how many are left (`None`: no limit) and what one costs
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BarterItem {
    pub item_id: u32,
    pub item_type: u8,
    pub shop_index: u32,
    pub stock: Option<u32>,
    pub zeny: u32,
    /// The refine the item comes with (an expanded barter's, from 20250402); 0 otherwise
    pub refine: u32,
    pub costs: Vec<BarterCost>,
}

/// An item a barter takes for one of its own, at least at `refine`
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BarterCost {
    pub item_id: u32,
    pub refine: u16,
    pub amount: u32,
}

/// One line of a barter purchase; `inv_index` is the paying item's inventory index (simple barters only)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BarterPurchase {
    pub item_id: u32,
    pub shop_index: u32,
    pub amount: u32,
    pub inv_index: u16,
}

/// One material the refine window offers: its item, the success chance in percent, the zeny it costs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RefineMaterial {
    pub item_id: u32,
    pub chance: u8,
    pub zeny: u32,
}

#[derive(Debug, Clone)]
pub struct RefineItemRow {
    pub index: i16,
    pub item_id: u32,
    pub refine: u8,
    pub cards: [u32; 4],
}

#[derive(Debug, Clone)]
pub struct VendorItem {
    pub index: i16,
    pub item_id: u32,
    pub amount: i16,
    pub price: i32,
    pub refine: u8,
    pub is_identified: bool,
    pub is_damaged: bool,
    pub item_type: u8,
}

#[derive(Debug, Clone)]
pub struct PartyMemberData {
    pub aid: u32,
    pub name: String,
    pub map: String,
    pub leader: bool,
    pub online: bool,
    pub class: u16,
    pub base_level: u16,
}

#[derive(Debug, Clone, Copy)]
pub struct GuildMemberAppearance {
    pub sex: i16,
    pub head: i16,
    pub head_palette: i16,
}

#[derive(Debug, Clone)]
pub struct FriendData {
    pub aid: u32,
    pub gid: u32,
    pub name: String,
    pub online: bool,
}

#[derive(Debug, Clone)]
pub struct SkillInfo {
    pub skill: SkillEnum,
    pub level: i16,
    pub sp_cost: i16,
    pub attack_range: i16,
    pub upgradable: bool,
    pub skill_target_type: SkillTargetType,
}

impl SkillInfo {
    pub fn icon_path(&self) -> String {
        crate::skill::skill_icon_path(self.skill)
    }
}

#[derive(Debug, Clone)]
pub struct ServerInfo {
    pub ip: u32,
    pub port: i16,
    pub name: String,
    pub user_count: u16,
}

#[derive(Debug, Clone)]
pub struct AccessibleMap {
    pub status: u32,
    pub name: String,
}

impl CharacterInfo {
    /// A look of the character changed in play (`ZC_SPRITE_CHANGE`'s hair, colours and headgear): kept, so the
    /// character comes back in it when it enters a map server again
    pub fn apply_look(&mut self, sprite_type: u8, value: u16) {
        use crate::entity::LookType;
        match LookType::from_sprite_type(sprite_type) {
            Some(LookType::Hair) => self.head = value,
            Some(LookType::HeadBottom) => self.head_bottom = value,
            Some(LookType::HeadTop) => self.head_top = value,
            Some(LookType::HeadMid) => self.head_mid = value,
            Some(LookType::HairColor) => self.hair_color = value,
            Some(LookType::ClothesColor) => self.cloth_color = value,
            Some(LookType::Robe) => self.robe = value,
            // the hands follow the equipment, the job the job change
            _ => {}
        }
    }
}

#[derive(Debug, Clone)]
pub struct CharacterInfo {
    pub gid: u32,
    pub name: String,
    pub class: u16,
    pub base_level: u16,
    pub base_exp: u64,
    pub job_level: u32,
    pub map: String,
    pub slot: i8,
    pub head: u16,
    pub hair_color: u16,
    /// The clothes' palette (`bodypalette`): a dye kept with the character
    pub cloth_color: u16,
    pub weapon: u16,
    pub head_top: u16,
    pub head_mid: u16,
    pub head_bottom: u16,
    pub shield: u16,
    /// The garment's view, 0 for none
    pub robe: u16,
    pub sex: u8,
    pub hp: u32,
    pub max_hp: u32,
    pub sp: u32,
    pub max_sp: u32,
    pub str: u8,
    pub agi: u8,
    pub vit: u8,
    pub int: u8,
    pub dex: u8,
    pub luk: u8,
    pub effect_state: i32,
    pub zeny: i32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_look_changed_in_play_is_kept_with_the_character() {
        let mut character = CharacterInfo {
            gid: 1,
            name: "Probette".into(),
            class: 4260,
            base_level: 1,
            base_exp: 0,
            job_level: 1,
            map: "prt_in".into(),
            slot: 0,
            head: 2,
            hair_color: 1,
            cloth_color: 0,
            weapon: 0,
            head_top: 0,
            head_mid: 0,
            head_bottom: 0,
            shield: 0,
            robe: 0,
            sex: 0,
            hp: 0,
            max_hp: 0,
            sp: 0,
            max_sp: 0,
            str: 1,
            agi: 1,
            vit: 1,
            int: 1,
            dex: 1,
            luk: 1,
            effect_state: 0,
            zeny: 0,
        };
        character.apply_look(1, 33);
        character.apply_look(7, 4);
        character.apply_look(2, 1201);
        assert_eq!(
            (character.head, character.cloth_color, character.weapon),
            (33, 4, 0)
        );
    }
    use crate::char_name::CharNameCache;

    #[test]
    fn a_ranking_waits_for_every_name() {
        let ranking = Ranking {
            kind: FameKind::Alchemist,
            entries: vec![(150_001, 40), (150_005, 7)],
        };
        let mut names = CharNameCache::default();
        names.insert(150_001, "SpikeHuman".into());
        assert_eq!(ranking.lines(&names), None);
        names.insert(150_005, "Probette".into());
        assert_eq!(
            ranking.lines(&names).unwrap(),
            vec![
                "== Top 10 Alchemists ==",
                "  1. SpikeHuman - 40",
                "  2. Probette - 7"
            ]
        );
    }
}
