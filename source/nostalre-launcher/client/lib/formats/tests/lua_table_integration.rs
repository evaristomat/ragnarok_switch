use nostalre_formats::lua_table;

#[test]
fn parse_accessory_lua_files() {
    let id_bytes = include_bytes!("fixtures/accessoryid.lua");
    let name_bytes = include_bytes!("fixtures/accname.lua");

    let id_content = lua_table::decode_euc_kr(id_bytes);
    let name_content = lua_table::decode_euc_kr(name_bytes);

    let table = lua_table::build_accessory_table(&id_content, &name_content);

    assert_eq!(table.len(), 859);

    assert_eq!(table.get(&1).map(|s| s.as_str()), Some("_고글"));
    assert_eq!(table.get(&17).map(|s| s.as_str()), Some("_리본"));
    assert_eq!(table.get(&14).map(|s| s.as_str()), Some("_캡"));
}

#[test]
fn parse_idnum2itemresnametable() {
    let data = include_bytes!("fixtures/idnum2itemresnametable.txt");

    let table = lua_table::parse_item_res_table(data);

    assert_eq!(table.len(), 2);

    assert_eq!(table.get(&501).map(|s| s.as_str()), Some("빨간포션"));
    assert_eq!(table.get(&664).map(|s| s.as_str()), Some("선물상자_1"));
}

#[test]
fn parse_accessory_lub_lua50() {
    let table = lua_table::build_accessory_table_from_lub(
        include_bytes!("fixtures/acid_lua50.lub"),
        include_bytes!("fixtures/aname_lua50.lub"),
    )
    .expect("lua 5.0 accessory chunks");

    assert_eq!(table.len(), 859);
    assert_eq!(table.get(&1).map(|s| s.as_str()), Some("_고글"));
    assert_eq!(table.get(&14).map(|s| s.as_str()), Some("_캡"));
    assert_eq!(table.get(&17).map(|s| s.as_str()), Some("_리본"));
}

#[test]
fn parse_accessory_lub_lua51() {
    let table = lua_table::build_accessory_table_from_lub(
        include_bytes!("fixtures/acid_lua51.lub"),
        include_bytes!("fixtures/aname_lua51.lub"),
    )
    .expect("lua 5.1 accessory chunks");

    assert_eq!(table.len(), 859);
    assert_eq!(table.get(&1).map(|s| s.as_str()), Some("_고글"));
    assert_eq!(table.get(&14).map(|s| s.as_str()), Some("_캡"));
    assert_eq!(table.get(&17).map(|s| s.as_str()), Some("_리본"));
}

#[test]
fn parse_npcidentity_lub() {
    let lua50 = lua_table::parse_jt_identity_lub(include_bytes!("fixtures/npcid_lua50.lub"))
        .expect("lua 5.0 identity chunk");
    let lua51 = lua_table::parse_jt_identity_lub(include_bytes!("fixtures/npcid_lua51.lub"))
        .expect("lua 5.1 identity chunk");

    for table in [&lua50, &lua51] {
        assert_eq!(table.len(), 2105);
        assert_eq!(table.get(&1002).map(|s| s.as_str()), Some("JT_PORING"));
        assert_eq!(table.get(&1885).map(|s| s.as_str()), Some("JT_GOPINICH"));
    }
}

#[test]
fn job_name_table_resolves_homunculus_sprite_names() {
    let identity = include_bytes!("fixtures/npcid_lua51.lub").as_slice();
    let job_name = include_bytes!("fixtures/jname_lua51.lub").as_slice();

    let names = lua_table::parse_job_name_lub(&[identity, job_name]).expect("jobname chunk");

    assert_eq!(names.len(), 2104);
    // The identity constants are JT_MER_*; the sprite files are not.
    assert_eq!(names.get(&6001).map(|s| s.as_str()), Some("LIF"));
    assert_eq!(names.get(&6003).map(|s| s.as_str()), Some("FILIR"));
    assert_eq!(names.get(&6011).map(|s| s.as_str()), Some("FILIR_H"));
    assert_eq!(names.get(&1002).map(|s| s.as_str()), Some("Poring"));
}

#[test]
fn item_info_reads_past_16_bit_ids() {
    let items = lua_table::parse_item_info(include_bytes!("fixtures/iteminfo_lua51.lub"))
        .expect("iteminfo chunk");
    assert_eq!(items.len(), 2);
    assert_eq!(items[&1201].class_num, Some(1));
    assert_eq!(items[&1201].unidentified_name.as_deref(), Some("Dagger"));
    let sword = &items[&500001];
    assert_eq!(sword.class_num, Some(2));
    assert_eq!(sword.identified_name.as_deref(), Some("Boost Sword"));
    assert_eq!(sword.identified_resource.as_deref(), Some("boost_sword"));
}

/// The same table compiled by a 64-bit `luac`, whose string lengths are 8 bytes
#[test]
fn item_info_reads_a_64_bit_luac_chunk() {
    let items = lua_table::parse_item_info(include_bytes!("fixtures/iteminfo_lua51_64bit.lub"))
        .expect("64-bit iteminfo chunk");
    assert_eq!(items.len(), 2);
    assert_eq!(
        items[&500001].identified_resource.as_deref(),
        Some("boost_sword")
    );
}

#[test]
fn weapon_table_takes_the_classic_item_or_the_base_view() {
    let table = lua_table::parse_weapon_table(include_bytes!("fixtures/weapontable_lua51.lub"))
        .expect("weapontable chunk");
    assert_eq!(table.expansions.len(), 2);
    assert_eq!(table.expansions[&52], 1410);
    assert_eq!(table.expansions[&99], 4);
    // only the view not named after an item has art of its own
    assert_eq!(table.art_names.len(), 1);
    assert_eq!(table.art_names[&99], "_odd");
}

#[test]
fn garment_names_come_by_view() {
    let names = lua_table::parse_robe_names(
        include_bytes!("fixtures/robe_ids_lua51.lub"),
        include_bytes!("fixtures/robe_names_lua51.lub"),
    )
    .expect("robe name chunks");
    assert_eq!(names.len(), 2);
    assert_eq!(names[&1], "천사날개");
    assert_eq!(names[&2], "Bag");
}

#[test]
fn garment_layers_take_jobid_from_a_chunk_the_vm_stops_in() {
    let jobid = include_bytes!("fixtures/robe_jobid_lua51.lub");
    let jobs = include_bytes!("fixtures/robe_layer_jobs_lua51.lub");
    let female = include_bytes!("fixtures/robe_layers_f_lua51.lub");
    let male = include_bytes!("fixtures/robe_layers_m_lua51.lub");
    let layers =
        lua_table::parse_robe_layers(jobid, jobs, female, male).expect("robe layer chunks");
    assert_eq!(layers.jobs[&4008], 7);
    // the female table first, then the male
    assert_eq!(layers.behind[0][&7][&0], vec![0, 1]);
    assert_eq!(layers.behind[0][&7][&8], vec![2]);
    assert_eq!(layers.behind[1][&7][&0], vec![5]);
    assert_eq!(layers.behind[1][&8][&16], vec![0]);
    // without JOBID nothing can be read, and it says why
    let error = lua_table::parse_robe_layers(&[], jobs, female, male).unwrap_err();
    assert_eq!(error.to_string(), "unexpected end of chunk");
    let error = lua_table::parse_robe_layers(
        include_bytes!("fixtures/robe_ids_lua51.lub"),
        jobs,
        female,
        male,
    )
    .unwrap_err();
    assert_eq!(error.to_string(), "no table JOBID");
}

#[test]
fn sign_board_rows_come_in_order_with_their_holes() {
    let rows = lua_table::parse_sign_board_list(include_bytes!("fixtures/signboardlist_lua51.lub"))
        .expect("sign board chunk");

    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0].map, "Default");
    assert_eq!(rows[0].color.as_deref(), Some("#0x009EBDFF"));
    assert_eq!(
        rows[1],
        lua_table::SignBoardRow {
            map: "prontera".into(),
            x: 146,
            y: 89,
            height: 0,
            kind: 1,
            image: Some("유저인터페이스\\information\\over_kafra.bmp".into()),
            text: None,
            color: None,
        }
    );
    assert_eq!(
        (rows[2].height, rows[2].kind, rows[2].text.as_deref()),
        (15, 3, Some("시험 간판"))
    );
}

#[test]
fn skill_info_list_gives_sp_per_level_of_the_skills_with_a_level_picker() {
    let ids = include_bytes!("fixtures/skillid_lua51.lub").as_slice();
    let list = include_bytes!("fixtures/skillinfolist_lua51.lub").as_slice();

    let skills = lua_table::parse_skill_info_sp_amounts(&[ids, list]).expect("skill chunks");

    assert_eq!(skills.len(), 3);
    assert_eq!(skills["SM_BASH"], vec![7, 7, 7, 12, 12]);
    assert_eq!(skills["AL_HEAL"], vec![10, 15, 20]);
    assert_eq!(skills["AL_TELEPORT"], Vec::<i16>::new());
    assert!(!skills.contains_key("SM_SWORD"));
}

#[test]
fn skill_info_list_needs_the_ids_it_indexes() {
    let list = include_bytes!("fixtures/skillinfolist_lua51.lub").as_slice();

    assert!(lua_table::parse_skill_info_sp_amounts(&[list]).is_err());
}
