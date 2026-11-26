#[cfg(test)]
mod openmw_tests {
    use std::fs;

    use openmw_config::OpenMWConfiguration;
    use plox::openmw_mods;

    fn write_plugin(dir: &std::path::Path, name: &str) {
        fs::create_dir_all(dir).unwrap();
        fs::copy("./tests/test2.esp", dir.join(name)).unwrap();
    }

    #[test]
    fn test_data_dir_with_ampersand() {
        let tmp = tempfile::tempdir().unwrap();
        let data = tmp.path().join("Mods & Fixes");
        write_plugin(&data, "Fix.esp");

        let escaped = data.display().to_string().replace('&', "&&");
        fs::write(
            tmp.path().join("openmw.cfg"),
            format!("data=\"{escaped}\"\ncontent=Fix.esp\n"),
        )
        .unwrap();

        let cfg = OpenMWConfiguration::new(Some(tmp.path().to_path_buf())).unwrap();
        let mods = openmw_mods(&cfg);
        assert_eq!(
            mods.iter().map(|m| m.name.as_str()).collect::<Vec<_>>(),
            vec!["Fix.esp"]
        );
    }

    #[test]
    fn test_content_lookup_is_case_insensitive_and_later_dirs_win() {
        let tmp = tempfile::tempdir().unwrap();
        let first = tmp.path().join("a");
        let second = tmp.path().join("b");
        write_plugin(&first, "Plugin.esp");
        write_plugin(&second, "PLUGIN.ESP");

        fs::write(
            tmp.path().join("openmw.cfg"),
            format!(
                "data=\"{}\"\ndata=\"{}\"\ncontent=plugin.esp\n",
                first.display(),
                second.display()
            ),
        )
        .unwrap();

        let cfg = OpenMWConfiguration::new(Some(tmp.path().to_path_buf())).unwrap();
        let mods = openmw_mods(&cfg);
        assert_eq!(mods.len(), 1);
        assert!(mods[0].name.eq_ignore_ascii_case("plugin.esp"));
    }

    #[test]
    fn test_save_content_keeps_other_lines() {
        let tmp = tempfile::tempdir().unwrap();
        let cfg_path = tmp.path().join("openmw.cfg");
        fs::write(
            &cfg_path,
            "fallback-archive=Morrowind.bsa\ncontent=a.esp\ncontent=b.esp\nno-sound=0\n",
        )
        .unwrap();

        let mut cfg = OpenMWConfiguration::new(Some(tmp.path().to_path_buf())).unwrap();
        cfg.set_content_files(Some(vec!["b.esp".into(), "a.esp".into()]));
        cfg.save_user().unwrap();

        let reloaded = OpenMWConfiguration::new(Some(tmp.path().to_path_buf())).unwrap();
        let content = reloaded
            .content_files_iter()
            .map(|f| f.value_str().to_owned())
            .collect::<Vec<_>>();
        assert_eq!(content, vec!["b.esp", "a.esp"]);

        let text = fs::read_to_string(&cfg_path).unwrap();
        assert!(text.contains("fallback-archive=Morrowind.bsa"), "{text}");
        assert!(text.contains("no-sound=0"), "{text}");
    }

    #[test]
    fn test_save_load_order_backs_up_user_cfg() {
        let tmp = tempfile::tempdir().unwrap();
        let cfg_path = tmp.path().join("openmw.cfg");
        let original = "content=a.esp\ncontent=b.esp\n";
        fs::write(&cfg_path, original).unwrap();

        let mut cfg = OpenMWConfiguration::new(Some(tmp.path().to_path_buf())).unwrap();
        plox::save_openmw_load_order(
            &mut cfg,
            &["b.esp".to_owned(), "a.esp".to_owned()],
            Some(jiff::civil::date(2026, 11, 26)),
        )
        .unwrap();

        let backup = tmp.path().join("openmw_26-11-2026.cfg");
        assert_eq!(fs::read_to_string(&backup).unwrap(), original);
        let text = fs::read_to_string(&cfg_path).unwrap();
        assert!(
            text.find("content=b.esp") < text.find("content=a.esp"),
            "{text}"
        );
    }
}
