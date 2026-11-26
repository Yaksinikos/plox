#[cfg(test)]
mod tes3_tests {
    use std::fs;

    use plox::{find_case_insensitive, gather_tes3_mods, update_tes3};

    /// A game folder with lower-case `data files` and `morrowind.ini`, as often found on Linux
    fn game_folder() -> tempfile::TempDir {
        let tmp = tempfile::tempdir().unwrap();
        let data = tmp.path().join("data files");
        fs::create_dir_all(&data).unwrap();
        for name in ["Morrowind.esm", "Test.esp", "Inactive.esp"] {
            fs::copy("./tests/test2.esp", data.join(name)).unwrap();
        }
        fs::write(
            tmp.path().join("morrowind.ini"),
            "[General]\nFoo=1\n[Game Files]\nGameFile0=morrowind.esm\nGameFile1=test.esp\n",
        )
        .unwrap();
        tmp
    }

    #[test]
    fn test_finds_folders_case_insensitively() {
        let tmp = game_folder();
        let found = find_case_insensitive(tmp.path(), "Data Files");
        assert!(found.is_dir());
        assert!(find_case_insensitive(tmp.path(), "Morrowind.ini").is_file());
        assert_eq!(
            find_case_insensitive(tmp.path(), "missing.txt"),
            tmp.path().join("missing.txt")
        );
    }

    #[test]
    fn test_gather_matches_ini_case_insensitively() {
        let tmp = game_folder();
        let mut names = gather_tes3_mods(&tmp.path())
            .into_iter()
            .map(|m| m.name)
            .collect::<Vec<_>>();
        names.sort();
        assert_eq!(names, vec!["Morrowind.esm", "Test.esp"]);
    }

    #[test]
    fn test_update_writes_names_as_on_disk() {
        let tmp = game_folder();
        update_tes3(
            tmp.path(),
            &["morrowind.esm".to_owned(), "test.esp".to_owned()],
            false,
            None,
        )
        .unwrap();

        let ini = fs::read_to_string(find_case_insensitive(tmp.path(), "Morrowind.ini")).unwrap();
        assert!(ini.contains("Foo=1"), "{ini}");
        assert!(ini.contains("GameFile0=Morrowind.esm"), "{ini}");
        assert!(ini.contains("GameFile1=Test.esp"), "{ini}");
        assert_eq!(
            fs::read_dir(tmp.path()).unwrap().count(),
            2,
            "no backup expected"
        );
    }

    #[test]
    fn test_update_backs_up_ini_before_writing() {
        let tmp = game_folder();
        let original = fs::read_to_string(tmp.path().join("morrowind.ini")).unwrap();
        let date = jiff::civil::date(2026, 11, 26);

        update_tes3(
            tmp.path(),
            &["test.esp".to_owned(), "morrowind.esm".to_owned()],
            false,
            Some(date),
        )
        .unwrap();

        let backup = tmp.path().join("morrowind_26-11-2026.ini");
        assert_eq!(fs::read_to_string(&backup).unwrap(), original);
        let ini = fs::read_to_string(tmp.path().join("morrowind.ini")).unwrap();
        assert!(ini.contains("GameFile0=Test.esp"), "{ini}");
    }
}
