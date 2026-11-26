#[cfg(test)]
mod unit_tests {

    use plox::{
        rules::Order,
        sorter::{self, Sorter},
        *,
    };

    fn init() {
        let env = env_logger::Env::default()
            .default_filter_or(log_level_to_str(ELogLevel::Debug))
            .default_write_style_or("always");
        let _ = env_logger::Builder::from_env(env).is_test(true).try_init();
    }

    fn new_stable_full_sorter() -> Sorter {
        Sorter::new(sorter::ESortType::StableFull, 1000)
    }

    #[test]
    fn test_cycle() {
        init();

        let order = [
            Order::from("a", "b").into(),
            Order::from("b", "c").into(),
            Order::from("d", "e").into(),
            Order::from("b", "a").into(),
        ];

        let mods: Vec<PluginData> = ["a", "b", "c", "d", "e", "f", "g"]
            .iter()
            .map(|e| PluginData::new(e.to_string(), 0))
            .collect();

        // the contradicting rule `b -> a` comes last and is dropped
        let data = sorter::get_graph_data(&mods, &order, &[]);
        assert_eq!(data.skipped.len(), 1);

        for mut sorter in [
            sorter::new_unstable_sorter(),
            new_stable_full_sorter(),
            sorter::new_stable_sorter(),
        ] {
            let result = sorter
                .topo_sort(ESupportedGame::Morrowind, &mods, &order, &[])
                .unwrap_or_else(|e| panic!("Error: {e}"));
            let pos = |n: &str| result.iter().position(|r| r == n);
            assert!(pos("a") < pos("b"), "first rule must win: {result:?}");
            assert!(pos("b") < pos("c"));
            assert!(pos("d") < pos("e"));
        }
    }

    #[test]
    fn test_ordering() {
        init();

        let order = [
            Order::from("b", "a").into(),
            Order::from("b", "c").into(),
            Order::from("d", "e").into(),
            Order::from("e", "c").into(),
            Order::from("test.archive", "test2.archive").into(),
        ];

        let mods: Vec<PluginData> = ["d", "e", "f", "g", "a", "b", "c"]
            .iter()
            .map(|e| PluginData::new(e.to_string(), 0))
            .collect();

        match sorter::new_unstable_sorter().topo_sort(ESupportedGame::Morrowind, &mods, &order, &[])
        {
            Ok(result) => {
                assert!(check_order(&result, &order), "stable(true) order is wrong");
            }
            Err(e) => panic!("Error: {e}"),
        }

        match new_stable_full_sorter().topo_sort(ESupportedGame::Morrowind, &mods, &order, &[]) {
            Ok(result) => {
                assert!(check_order(&result, &order), "stable(true) order is wrong");
            }
            Err(e) => panic!("Error: {e}"),
        }

        match sorter::new_stable_sorter().topo_sort(ESupportedGame::Morrowind, &mods, &order, &[]) {
            Ok(result) => {
                assert!(check_order(&result, &order), "stable(true) order is wrong");
            }
            Err(e) => panic!("Error: {e}"),
        }
    }
}
