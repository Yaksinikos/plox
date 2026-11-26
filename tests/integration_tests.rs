#[cfg(test)]
mod integration_tests {
    use std::path::PathBuf;
    use std::{fs::create_dir_all, io::Write};

    use log::warn;
    use plox::{parser::*, sorter::*, *};
    use rand::rng;
    use rand::seq::SliceRandom;
    use rules::{EWarningRule, TWarningRule};
    use semver::Version;

    fn init() {
        let env = env_logger::Env::default()
            .default_filter_or(log_level_to_str(ELogLevel::Debug))
            .default_write_style_or("always");
        let _ = env_logger::Builder::from_env(env).is_test(true).try_init();
    }

    /// Test output folder inside the target dir
    fn out_dir() -> PathBuf {
        let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("integration_tests");
        create_dir_all(&dir).expect("could not create dir");
        dir
    }

    fn new_stable_full_sorter() -> Sorter {
        Sorter::new(sorter::ESortType::StableFull, 1000)
    }

    fn clean_mods(plugins: &[PluginData], warning_rules: &[EWarningRule]) -> Vec<PluginData> {
        let mut mods_to_remove = vec![];
        let mut warning_rules = warning_rules.to_vec();
        for rule in &mut warning_rules {
            // only conflict rules
            if let EWarningRule::Conflict(conflict) = rule
                && conflict.eval(plugins)
            {
                // remove mods
                // switch on the len of conflict.conflicts
                let groups_size = conflict.conflicts.len();
                if groups_size == 2 {
                    // remove all mods of group 1
                    for mod_name in &conflict.conflicts[0] {
                        // add if not already in
                        if !mods_to_remove.contains(mod_name) {
                            mods_to_remove.push(mod_name.clone());
                        }
                    }
                } else {
                    // TODO do nothing for now
                    //warn!("groups_size: {}", groups_size);
                }
            }
        }

        // log
        warn!("removing mods: {:?}", mods_to_remove.len());
        for mod_name in &mods_to_remove {
            warn!("\t{mod_name}");
        }

        // remove mods
        let mut mods_cpy = plugins.to_vec();
        mods_cpy.retain(|x| !mods_to_remove.contains(&x.name));

        mods_cpy
    }

    /// Checks the result against the order rules plox keeps (contradicting rules are dropped)
    fn order_ok(result: &[String], mods: &[PluginData], parser: &Parser) -> bool {
        let data = get_graph_data(mods, &parser.order_rules, &parser.warning_rules);
        check_graph_order(result, &data)
    }

    #[test]
    fn test_read_mods() {
        init();

        let mods_path = "./tests/modlist.txt";
        let mods_data = read_file_as_list(mods_path, &None);
        assert_eq!(
            mods_data.iter().map(|s| s.name.clone()).collect::<Vec<_>>(),
            vec![
                "a.archive",
                "b.archive",
                "c.archive",
                "d.archive",
                "e.archive"
            ]
        );
    }

    #[test]
    fn test_parse_order() {
        init();

        let mut parser = new_tes3_parser();
        parser
            .init_from_file("./tests/plox/rules_order.txt")
            .expect("failed rule parsing");

        assert_eq!(8, parser.order_rules.len());

        let mods = debug_get_mods_from_order_rules(&parser.order_rules);

        match new_unstable_sorter().topo_sort(
            ESupportedGame::Morrowind,
            &mods,
            &parser.order_rules,
            &parser.warning_rules,
        ) {
            Ok(result) => {
                assert!(
                    order_ok(&result, &mods, &parser),
                    "stable(true) order is wrong"
                );
            }
            Err(e) => panic!("Error: {e}"),
        }

        match new_stable_sorter().topo_sort(
            ESupportedGame::Morrowind,
            &mods,
            &parser.order_rules,
            &parser.warning_rules,
        ) {
            Ok(result) => {
                assert!(
                    order_ok(&result, &mods, &parser),
                    "stable(true) order is wrong"
                );
            }
            Err(e) => panic!("Error: {e}"),
        }
    }

    #[test]
    fn test_parse_notes() {
        init();

        {
            let mut parser = new_cyberpunk_parser();
            parser
                .init_from_file("./tests/plox/rules_note_passing.txt")
                .expect("failed rule parsing");
            assert_eq!(11, parser.warning_rules.len());
        }

        {
            let mut parser = new_tes3_parser();
            parser
                .init_from_file("./tests/plox/rules_note_failing.txt")
                .expect("failed rule parsing");
            assert_eq!(0, parser.warning_rules.len());
        }
    }

    #[test]
    fn test_parse_conflicts() {
        init();

        let mut parser = new_tes3_parser();
        parser
            .init_from_file("./tests/plox/rules_conflict.txt")
            .expect("failed rule parsing");
        assert_eq!(6, parser.warning_rules.len());
    }

    #[test]
    fn test_parse_requires() {
        init();

        let mut parser = new_tes3_parser();
        parser
            .init_from_file("./tests/plox/rules_requires.txt")
            .expect("failed rule parsing");
        assert_eq!(1, parser.warning_rules.len());
    }

    #[test]
    fn test_dump_rules() -> std::io::Result<()> {
        init();

        {
            let mut parser = new_tes3_parser();
            parser.init_from_file("./tests/mlox/mlox_base.txt")?;

            {
                let file = std::fs::File::create(out_dir().join("base_rules.json"))
                    .expect("file create failed");
                serde_json::to_writer_pretty(file, &parser.warning_rules)
                    .expect("serialize failed");
            }

            {
                let file = std::fs::File::create(out_dir().join("base_rules_order.json"))
                    .expect("file create failed");
                serde_json::to_writer_pretty(file, &parser.order_rules).expect("serialize failed");
            }
        }

        {
            let mut parser = new_tes3_parser();
            parser.init_from_file("./tests/mlox/mlox_user.txt")?;

            {
                let file = std::fs::File::create(out_dir().join("user_rules.json"))
                    .expect("file create failed");
                serde_json::to_writer_pretty(file, &parser.warning_rules)
                    .expect("serialize failed");
            }

            {
                let file = std::fs::File::create(out_dir().join("user_rules_order.json"))
                    .expect("file create failed");
                serde_json::to_writer_pretty(file, &parser.order_rules).expect("serialize failed");
            }

            Ok(())
        }
    }

    #[test]
    fn test_dump_display_rules() -> std::io::Result<()> {
        init();

        {
            let mut parser = new_tes3_parser();
            parser.init_from_file("./tests/mlox/mlox_base.txt")?;

            {
                let mut file = std::fs::File::create(out_dir().join("base_rules.txt"))
                    .expect("file create failed");
                for rule in parser.warning_rules {
                    writeln!(file, "{rule}").expect("could not write to file");
                }
            }
        }

        {
            let mut parser = new_tes3_parser();
            parser.init_from_file("./tests/mlox/mlox_user.txt")?;

            {
                let mut file = std::fs::File::create(out_dir().join("user_rules.txt"))
                    .expect("file create failed");
                for rule in parser.warning_rules {
                    writeln!(file, "{rule}").expect("could not write to file");
                }
            }

            Ok(())
        }
    }

    #[test]
    fn graphviz() -> std::io::Result<()> {
        init();

        let mut parser = new_tes3_parser();
        parser.init_from_file("./tests/mlox/mlox_user.txt")?;

        let mut mods = debug_get_mods_from_order_rules(&parser.order_rules);
        mods = clean_mods(&mods, &parser.warning_rules);

        let mut rng = rng();
        mods.shuffle(&mut rng);

        let data = sorter::get_graph_data(&mods, &parser.order_rules, &parser.warning_rules);
        let g = sorter::build_graph(&data);

        {
            let viz = petgraph::dot::Dot::with_config(&g, &[petgraph::dot::Config::EdgeNoLabel]);
            // write to file

            let mut file =
                std::fs::File::create(out_dir().join("graphviz.dot")).expect("file create failed");
            std::io::Write::write_all(&mut file, format!("{viz:?}").as_bytes())
                .expect("write failed");
        }

        Ok(())
    }

    #[test]
    fn test_mlox_user_rules_stable() -> std::io::Result<()> {
        init();

        let mut parser = new_tes3_parser();
        parser.init_from_file("./tests/mlox/mlox_user.txt")?;

        let mut mods = debug_get_mods_from_order_rules(&parser.order_rules);
        mods = clean_mods(&mods, &parser.warning_rules);

        let mut rng = rng();
        mods.shuffle(&mut rng);

        match new_stable_sorter().topo_sort(
            ESupportedGame::Morrowind,
            &mods,
            &parser.order_rules,
            &parser.warning_rules,
        ) {
            Ok(result) => {
                assert!(
                    order_ok(&result, &mods, &parser),
                    "stable(true) order is wrong"
                );
            }
            Err(e) => {
                panic!("Error: {e}")
            }
        }

        Ok(())
    }

    #[test]
    fn test_mlox_user_rules_unstable() -> std::io::Result<()> {
        init();

        let mut parser = new_tes3_parser();
        parser.init_from_file("./tests/mlox/mlox_user.txt")?;

        let mut mods = debug_get_mods_from_order_rules(&parser.order_rules);
        mods = clean_mods(&mods, &parser.warning_rules);

        let mut rng = rng();
        mods.shuffle(&mut rng);

        match new_unstable_sorter().topo_sort(
            ESupportedGame::Morrowind,
            &mods,
            &parser.order_rules,
            &parser.warning_rules,
        ) {
            Ok(result) => {
                assert!(
                    order_ok(&result, &mods, &parser),
                    "stable(true) order is wrong"
                );
            }
            Err(e) => panic!("Error: {e}"),
        }

        Ok(())
    }

    #[test]
    fn test_mlox_base_rules_stable() -> std::io::Result<()> {
        init();

        let mut parser = new_tes3_parser();
        parser.init_from_file("./tests/mlox/mlox_base.txt")?;

        let mut mods = debug_get_mods_from_order_rules(&parser.order_rules);
        mods = clean_mods(&mods, &parser.warning_rules);

        let mut rng = rng();
        mods.shuffle(&mut rng);

        match new_stable_sorter().topo_sort(
            ESupportedGame::Morrowind,
            &mods,
            &parser.order_rules,
            &parser.warning_rules,
        ) {
            Ok(result) => {
                assert!(
                    order_ok(&result, &mods, &parser),
                    "stable(true) order is wrong"
                );
            }
            Err(e) => {
                panic!("Error: {e}")
            }
        }

        Ok(())
    }

    #[test]
    fn test_mlox_base_rules_unstable() -> std::io::Result<()> {
        init();

        let mut parser = new_tes3_parser();
        parser.init_from_file("./tests/mlox/mlox_base.txt")?;

        let mut mods = debug_get_mods_from_order_rules(&parser.order_rules);
        mods = clean_mods(&mods, &parser.warning_rules);

        let mut rng = rng();
        mods.shuffle(&mut rng);

        match new_unstable_sorter().topo_sort(
            ESupportedGame::Morrowind,
            &mods,
            &parser.order_rules,
            &parser.warning_rules,
        ) {
            Ok(result) => {
                assert!(
                    order_ok(&result, &mods, &parser),
                    "stable(true) order is wrong"
                );
            }
            Err(e) => panic!("Error: {e}"),
        }

        Ok(())
    }

    #[test]
    fn test_mlox_rules_stable() -> std::io::Result<()> {
        init();

        let mut parser = new_tes3_parser();
        parser.parse("./tests/mlox")?;

        let mut mods = debug_get_mods_from_order_rules(&parser.order_rules);
        mods = clean_mods(&mods, &parser.warning_rules);

        let mut rng = rng();
        mods.shuffle(&mut rng);

        warn!("MODS: {}", mods.len());

        match new_stable_sorter().topo_sort(
            ESupportedGame::Morrowind,
            &mods,
            &parser.order_rules,
            &parser.warning_rules,
        ) {
            Ok(result) => {
                assert!(
                    order_ok(&result, &mods, &parser),
                    "stable(true) order is wrong"
                );
            }
            Err(e) => {
                panic!("Error: {e}")
            }
        }

        Ok(())
    }

    #[test]
    fn test_mlox_rules_unstable() -> std::io::Result<()> {
        init();

        let mut parser = new_tes3_parser();
        parser.parse("./tests/mlox")?;

        let mut mods = debug_get_mods_from_order_rules(&parser.order_rules);
        mods = clean_mods(&mods, &parser.warning_rules);

        let mut rng = rng();
        mods.shuffle(&mut rng);

        warn!("MODS: {}", mods.len());

        match new_unstable_sorter().topo_sort(
            ESupportedGame::Morrowind,
            &mods,
            &parser.order_rules,
            &parser.warning_rules,
        ) {
            Ok(result) => {
                assert!(
                    order_ok(&result, &mods, &parser),
                    "stable(true) order is wrong"
                );
            }
            Err(e) => panic!("Error: {e}"),
        }

        Ok(())
    }

    #[test]
    fn test_optimized_sort() -> std::io::Result<()> {
        init();

        let mut parser = parser::new_tes3_parser();
        parser.init_from_file("./tests/mlox/mlox_base.txt")?;
        let mut mods = debug_get_mods_from_order_rules(&parser.order_rules);

        // fixed seed, so a failure can be reproduced
        let mut rng = <rand::rngs::StdRng as rand::SeedableRng>::seed_from_u64(0x706c_6f78);
        mods.shuffle(&mut rng);
        let mods = mods.into_iter().take(100).collect::<Vec<_>>();

        let full_result = new_stable_full_sorter()
            .topo_sort(
                ESupportedGame::Morrowind,
                &mods,
                &parser.order_rules,
                &parser.warning_rules,
            )
            .expect("rules contain a cycle");
        let opt_result = sorter::new_stable_sorter()
            .topo_sort(
                ESupportedGame::Morrowind,
                &mods,
                &parser.order_rules,
                &parser.warning_rules,
            )
            .expect("opt rules contain a cycle");

        // both sorters must satisfy every rule; plugins without a rule between them may be
        // ordered differently, so the results are not compared with each other
        assert!(
            order_ok(&full_result, &mods, &parser),
            "full order is wrong"
        );
        assert!(order_ok(&opt_result, &mods, &parser), "opt order is wrong");
        let mut full_sorted = full_result;
        let mut opt_sorted = opt_result;
        full_sorted.sort();
        opt_sorted.sort();
        assert_eq!(
            full_sorted, opt_sorted,
            "sorters returned different plugins"
        );

        Ok(())
    }

    #[test]
    fn test_optimized_sort_time() -> std::io::Result<()> {
        init();

        let mut parser = parser::new_tes3_parser();
        parser.init_from_file("./tests/mlox/mlox_base.txt")?;
        let mut mods = debug_get_mods_from_order_rules(&parser.order_rules);

        let mut rng = rng();
        let mut times = vec![];
        for n in [64, 128, 256, 512 /* 1024 , 2048 */] {
            mods.shuffle(&mut rng);
            let max = std::cmp::min(n, mods.len() - 1);
            let mods_rnd = mods.clone().into_iter().take(max).collect::<Vec<_>>();

            let now = std::time::Instant::now();
            sorter::new_stable_sorter()
                .topo_sort(
                    ESupportedGame::Morrowind,
                    &mods_rnd,
                    &parser.order_rules,
                    &parser.warning_rules,
                )
                .expect("error: ");
            let elapsed = now.elapsed().as_secs();

            times.push((n, elapsed));
        }

        let mut msg = String::new();
        for (n, t) in &times {
            msg += format!("{n},{t}\n").as_str();
        }

        // log to file
        // let mut file = File::create("unit_log.txt").expect("could not create log file");
        // file.write_all(msg.as_bytes()).expect("write error");

        // assert
        for (_n, t) in times {
            assert!(t < 4);
        }

        Ok(())
    }

    #[test]
    fn test_gather_mods() {
        init();

        let root_path = "./tests";

        let mods = gather_mods(&root_path, ESupportedGame::Cyberpunk, &None);
        assert_eq!(
            mods.iter().map(|s| s.name.clone()).collect::<Vec<_>>(),
            vec![
                "a.archive".to_owned(),
                "b.archive".into(),
                "c.archive".into()
            ]
        );
    }

    #[test]
    fn test_parse_header() {
        init();

        {
            let plugin_test_path = PathBuf::from("tests").join("test2.esp");
            let header = parse_header(&plugin_test_path).expect("failed to parse header");

            // check some things
            assert_eq!(
                header.description,
                "The main data file For Morrowind with version 5.3"
            );
            // check master files
            assert!(header.masters.is_none());

            // check version
            let got = get_version(
                plugin_test_path.file_name().unwrap().to_str().unwrap(),
                &Some(header.description),
            );
            let expected = Version::new(5, 3, 0);
            assert_eq!(got.unwrap(), expected);
        }

        {
            let plugin_test_path = PathBuf::from("tests").join("test 1.1.esp");
            let header = parse_header(&plugin_test_path).expect("failed to parse header");

            // check some things
            assert_eq!(
                header.description,
                "The main data file for BloodMoon.\r\n(requires Morrowind.esm to run)"
            );
            // check master files
            assert_eq!(
                header.masters.unwrap(),
                vec![("Morrowind.esm".to_string(), 79_837_557_u64),]
            );

            // check version
            let got = get_version(
                plugin_test_path.file_name().unwrap().to_str().unwrap(),
                &Some(header.description),
            );
            let expected = Version::new(1, 1, 0);
            assert_eq!(got.unwrap(), expected);
        }
    }
}
